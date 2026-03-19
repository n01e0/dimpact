# G3-1: 現在の PDG / propagation path の実装境界棚卸し

このメモは、`impact` コマンドにある `--with-pdg` / `--with-propagation` の**現在の実装境界**を整理し、
通常の impact path（call graph ベース）とどこが揃っていて、どこがまだズレているかを固定するためのもの。

結論を先に書くと、現状の PDG path は

- **入口と最終到達計算は通常 impact path と共有**している
- しかし中間の graph 構築は **Rust/Ruby の changed/seed file 局所 DFG** に強く寄っている
- さらに最終的には `Reference` に潰して `compute_impact()` へ流しているため、**PDG 固有の dependency kind / provenance / witness がかなり落ちている**

という状態にある。

つまり今の `--with-pdg` / `--with-propagation` は、
**「impact 本体に統合された新しい graph モデル」** というより、
**「一部区間だけ PDG/propagation を差し込んで、最後は既存 impact path に戻している経路」** と捉えるのが正確。

## 1. 通常 impact path が今やっていること

通常 path は大きく 2 系統ある。

1. diff ベース: `engine.impact(&files, lang, &opts)`
2. seed ベース: `engine.impact_from_symbols(&seeds, lang, &opts)`

TS engine の既定経路では、どちらも最終的には

- cache を開く
- workspace 全体から構築/更新された `SymbolIndex` と `Reference` をロードする
- `compute_impact()` で symbol graph を BFS する

という流れになっている。

参照点:

- `src/engine/ts.rs`
- `src/cache.rs:492-515`
- `src/impact.rs:768-930`

この path の特徴は次の通り。

- graph の主単位は **symbol ↔ symbol の `Reference`**
- traversal / `summary.by_depth` / `summary.risk` / `impacted_symbols` は **全部 `compute_impact()` に集約**
- `--per-seed`、`--engine auto|ts|lsp`、confidence filter などの通常オプションはこの path に乗る

要するに、通常 path は **project-wide symbol graph を基準**にしており、impact の本体仕様もこちらを前提に育っている。

## 2. PDG / propagation path が今やっていること

`--with-pdg` または `--with-propagation` が入ると、`impact` CLI は通常 engine path を通らず、
`src/bin/dimpact.rs` 内で専用分岐に入る。

参照点:

- diff ベース: `src/bin/dimpact.rs:1189-1267`
- seed ベース: `src/bin/dimpact.rs:1293-1368`

現在の流れは次の通り。

1. changed symbols あるいは seed symbols は通常通り求める
2. cache から `index` と `refs`（既存 call graph）をロードする
3. changed file / seed file に対してだけ DFG を作る
   - 対応は **Rust (`.rs`) / Ruby (`.rb`) のみ**
4. `PdgBuilder::build()` で DFG + call refs を 1 本の graph にまとめる
5. `--with-propagation` のときだけ `augment_symbolic_propagation()` を追加実行する
6. 最後に PDG edge を `Reference` に潰し直して `compute_impact()` に渡す

参照点:

- `src/dfg.rs:561-715`
- `src/impact.rs:768-930`

この時点で、PDG path も**最終到達計算そのものは通常 impact path と同じ `compute_impact()`** を使っている。

## 3. どこが既に共有化されているか

まず、今の実装にも共有できている部分はある。

### 3.1 changed/seed symbol の入口は共通

- diff なら `compute_changed_symbols()`
- seed なら CLI から解決した `Symbol`

がそのまま seed になる。

つまり「どの symbol を起点に impact を始めるか」は、PDG path だけ別ルールになっていない。

### 3.2 最終 traversal は共通

`compute_impact()` は `Reference` の隣接関係を見て BFS するだけなので、
PDG path でも通常 path でも**最後の reachability ロジック自体は同一**。

これは良い意味で重要で、
G3 で PDG 結果を本体へ寄せる時も、
少なくとも「到達深さ」「summary 生成」「ignore_dir 適用」は同じ土台に乗せやすい。

### 3.3 project-wide symbol index は共有

PDG path でも cache から `SymbolIndex` を読むので、
最終的な impacted symbol の解決先は通常 path と同じ symbol universe にいる。

完全な別分析器になっているわけではない。

## 4. 実装境界の本体: PDG はどこまでを見ているか

ここからがズレの本題。

## 4.1 DFG を作るのは changed/seed file だけ

PDG path は project 全体の DFG を作っていない。

- diff ベースでは `changed.changed_files`
- seed ベースでは `seeds` が属する file 集合

に対してだけ DFG を構築している。

参照点:

- `src/bin/dimpact.rs:1206-1220`
- `src/bin/dimpact.rs:1306-1325`

このため、現状の PDG/propagation が見ている data/control flow は
**「変更が入った file 周辺」または「seed を置いた file 周辺」だけ** である。

### 含意

- callee 側が別 file にあると、その file に DFG node が無ければ summary bridge は育たない
- caller 側が別 file でも、そこでは通常 call ref に戻る
- つまり現在の PDG は **project-wide PDG** ではなく、**local DFG + global call graph** である

この境界は、通常 impact path との差としてかなり大きい。
通常 path は最初から最後まで project-wide symbol graph を見ているが、
PDG path は中間だけ file-local に細かくなる。

## 4.2 Rust/Ruby 以外では PDG 部分が実質空振りになる

DFG builder が実行されるのは `.rs` と `.rb` のみ。

参照点:

- `src/bin/dimpact.rs:1206-1219`
- `src/bin/dimpact.rs:1311-1324`

そのため Go / Java / Python / JS / TS / TSX では、`--with-pdg` を付けても
中間 graph に DFG node はほぼ入らず、`PdgBuilder::build()` は実質

- 空の DFG
- そこへ既存 call refs を data edge として流し込む

だけになる。

参照点:

- `src/dfg.rs:563-574`

これはつまり、Rust/Ruby 以外では現在の `--with-pdg` は
**新しい依存情報を増やす経路ではなく、既存 call graph を別表現へ詰め替えているだけ** に近い。

## 4.3 propagation も call-site 局所 heuristic に留まっている

`augment_symbolic_propagation()` は強化としては有効だが、見ている世界はかなり限定されている。

主な heuristic は:

- call site の `use` node → callee symbol
- callee symbol → call site の `def` node
- function summary を使った input -> impacted bridge
- function/method symbol → その span 内 DFG node

参照点:

- `src/dfg.rs:577-715`

ただしこの bridge が効くのは、
**call site と callee summary の双方に必要な DFG node があるとき**だけ。

`build_function_summaries()` も、各 function/method の file 範囲内にある `:def:` / `:use:` node を集めて組み立てるだけなので、
別 file / 非 Rust/Ruby / DFG 未構築 file には summary が立たない。

参照点:

- `src/dfg.rs:717-780` 以降

つまり propagation も今は
**project-wide symbolic propagation** ではなく、
**DFG が立っている file 群の中でだけ太くなる call-site bridge** という理解が正しい。

## 5. 通常 impact path とのズレ

ここからは、実際に気にすべきズレを論点別に並べる。

## 5.1 engine 選択を PDG path がバイパスしている

通常 path は `engine.impact()` / `engine.impact_from_symbols()` を通るため、
`--engine auto|ts|lsp`、`--auto-policy`、strict LSP などの運用方針が効く。

しかし PDG path は `src/bin/dimpact.rs` の分岐で直接 cache + DFG を扱っており、
**engine abstraction を通っていない**。

そのため現状の `--with-pdg` / `--with-propagation` は、
CLI 上は engine option を持っていても、実装上は **TS/LSP 切替とほぼ独立** になっている。

これは通常 impact path との明確なズレ。

### 含意

- LSP path が将来 `confirmed` / richer edge metadata を持っても PDG path には自動反映されない
- auto policy の改善をしても PDG path は別保守になる
- `impact` の「本体経路」が 2 つある状態になっている

## 5.2 PDG edge を `Reference` に潰す時点で意味情報が大きく落ちる

PDG path の最後では、`DfgEdge` を全部 `Reference` に変換して `compute_impact()` に渡している。

参照点:

- `src/bin/dimpact.rs:1239-1258`
- `src/bin/dimpact.rs:1340-1359`

このとき:

- `kind` は全部 `RefKind::Call`
- `file` は空文字
- `line` は `0`
- certainty は `confirmed_pairs` に入っていなければ `Inferred`

になる。

つまり、PDG が持っていたはずの

- `DependencyKind::Data | Control`
- call-site bridge なのか summary bridge なのか
- どの file / line で生成された edge なのか

といった情報は、`compute_impact()` に入る前にかなり失われる。

### これは何を意味するか

今の PDG path は、到達性の改善には寄与しうるが、
**説明可能性の改善はまだ graph 途中で捨てている**。

G3-5 や G3-6 で edge kind / provenance / witness を整理したい理由は、まさにここにある。

## 5.3 current cache path では confirmed edge も実質立たない

`confirmed_pairs` は元の `refs` の `certainty == Confirmed` を集めているが、
cache から読む `load_graph()` は現在すべての edge を `Inferred` として返している。

参照点:

- `src/bin/dimpact.rs:1229-1238`
- `src/bin/dimpact.rs:1330-1339`
- `src/cache.rs:492-508`

つまり現在の PDG path では、少なくとも cache ベースの通常実行では
**`confirmed_pairs` は空になりやすく、PDG edge はほぼ全部 `inferred` に寄る**。

テスト上は「confirmed または inferred しか出さない」で通っているが、
実装実態としては **ほぼ inferred-only path** と見てよい。

通常 impact path も TS cache では inferred 寄りだが、
PDG path は engine/LSP を通らない分、ここを将来改善しにくい。

## 5.4 DFG node は traversal に使えるが、最終出力の主役にはなっていない

`compute_impact()` は `Reference` の from/to ID を辿るだけなので、
ID が symbol でなく DFG node でも queue には積める。

しかし impacted result を作るときは `SymbolIndex` から引ける ID しか `impacted_symbols` に残さない。

参照点:

- `src/impact.rs:888-891`

つまり DFG node は

- reachability を繋ぐ中継点にはなれる
- でも `impacted_symbols` には出ない

という扱い。

この設計自体は理解できるが、現在の問題は、`--with-edges` で見える edge も最終的には changed/impacted symbol 近傍だけに絞られるため、
**propagation で実際に効いた DFG 中継がユーザ出力にはかなり残りにくい**こと。

参照点:

- `src/impact.rs:897-919`

要するに、PDG/propagation は内部到達には使っているが、
外に見せる path explanation はまだ通常 impact path の器に強く引き戻されている。

## 5.5 DOT 出力の意味が通常 impact と変わる

通常の `-f dot` は `ImpactOutput` を DOT 化する。
しかし `--with-pdg` / `--with-propagation` かつ `-f dot` のときだけ、
CLI は `compute_impact()` 前で早期 return し、**生の PDG を `dfg_to_dot()` で出す**。

参照点:

- `src/bin/dimpact.rs:1225-1227`
- `src/render.rs:24-55`

これは useful ではあるが、通常 impact の DOT とは意味が違う。

- 通常 DOT: changed/impacted symbol を中心にした impact graph
- PDG DOT: DFG node / control edge / call bridge を含む raw PDG

つまり `-f dot` は、`--with-pdg` の有無で**可視化対象が切り替わる**。

この差は README では「PDG 可視化」として触れられているが、
設計としては `impact output の別 view` ではなく、**別 graph dump** になっている。

## 5.6 `--per-seed` は明示的に未対応

`--per-seed` は通常 path では使えるが、PDG/propagation では
明示的にエラーになる。

参照点:

- `src/bin/dimpact.rs:1024-1028`

これは現状として妥当だが、通常 impact path との feature parity は崩れている。

G3-8 の対象がここ。

## 6. 現在の実装境界を一言でまとめると

今の PDG / propagation path は、次の境界で止まっている。

### 入っているもの

- Rust/Ruby changed/seed file の DFG
- その file 周辺での alias / control / call-site bridge
- minimal function summary
- 既存 call graph との接続
- 最終 traversal / summary は既存 impact の仕組みを再利用

### まだ入っていないもの

- project-wide PDG 構築
- engine abstraction への統合
- PDG edge kind / provenance / witness の保持
- per-seed 対応
- non-Rust/Ruby での本質的な PDG 強化
- `ImpactOutput` 側での PDG path explanation の表現

## 7. 設計上の判断: 今の PDG path をどう呼ぶべきか

G3 以降の議論では、今のものを

- 「PDG fully integrated path」

と呼ぶのはやや強すぎる。

現状に即した表現は、むしろ

- **call-graph impact に file-local PDG/propagation を差し込む補助経路**
- **reachability enhancement path**

あたり。

この言い方にしておくと、
「なぜ `Reference` へ潰しているのか」「なぜ per-seed が無いのか」「なぜ DOT だけ別物なのか」の説明が付きやすい。

## 8. G3 の後続タスクにどう繋がるか

この棚卸しから、後続タスクの焦点はかなりはっきりしている。

## 8.1 G3-2 / G3-3 / G3-4

ここではまず、今の file-local PDG/propagation でも改善できる FN/FP を固定し、
Rust/Ruby DFG / summary / alias / call-site bridge の精度を上げるのが筋。

理由:

- ここは既に実装面がある
- 変更の局所性が高い
- いきなり全体統合よりテストで前後比較しやすい

## 8.2 G3-5

ここが本丸。

解くべき問いは
**「PDG path の結果を impact result にどうマージするか」** であって、
単に到達 symbol を増やすだけでは足りない。

最低でも次を決める必要がある。

- edge の論理種別を `Call` 一択のままにするか
- `DependencyKind::Data|Control` を持ち上げるか
- call graph edge と propagation edge の provenance をどう表すか
- `with_edges` で DFG 中継や witness をどう見せるか

## 8.3 G3-6

witness / provenance を最小追加するなら、実装の圧縮点は
**`DfgEdge -> Reference` 変換部** になる可能性が高い。

ここで全部捨てている metadata を最小限でも残さないと、
「PDG でなぜ拾えたか」の説明力は上がらない。

## 8.4 G3-8

`--per-seed` 未対応は単なる未実装というより、
**今の PDG path が既存 impact output schema に綺麗に乗っていない**ことの表れでもある。

per-seed を入れるなら、
結果 grouping 以前に graph/provenance の扱いを少し整理した方がよい。

## 9. いま固定しておきたい設計メモ

最後に、今後の議論でブレないように短く固定しておく。

### メモ 1

**今の `--with-pdg` / `--with-propagation` は、project-wide PDG ではない。**

実態は **global call graph + local DFG augmentation**。

### メモ 2

**PDG path は impact traversal へ統合されているが、impact schema へはまだ十分統合されていない。**

到達計算は共有できているが、edge semantics / witness / provenance は途中で落ちている。

### メモ 3

**G3 の本質は「PDG をもっと賢くすること」だけではなく、「PDG の結果を impact 本体の説明可能な出力へ落とすこと」。**

この後半をやらない限り、PDG path は便利な内部補助経路のまま止まりやすい。

## 10. 一言まとめ

- 現在の PDG / propagation path は、Rust/Ruby の changed/seed file 周辺にだけ DFG を立てる
- そこへ既存 call graph を混ぜ、最後は `compute_impact()` に戻している
- そのため reachability enhancement としては機能しているが、通常 impact path と比べると
  - engine 統合
  - feature parity
  - edge kind / provenance
  - witness 表現
  がまだ揃っていない
- G3 後続では、精度改善と同じくらい **result integration の設計** が重要になる
