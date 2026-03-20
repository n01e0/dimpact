# G4-1: PDG build scope / engine consistency の残課題棚卸しと設計メモ

このメモは、G3 完了時点の `impact --with-pdg` / `--with-propagation` について、
**build scope** と **engine consistency** の 2 軸で残っている課題を整理し、
G4 でどの順番で設計・実装を進めるべきかを固定するためのもの。

結論を先に書くと、G3 後の PDG path は

- G3-1 で明示した「local DFG augmentation path」という説明が今も妥当
- G3 で `--per-seed`、edge metadata、minimal witness はかなり改善された
- しかし **どの file に DFG を立てるか** と **どの engine の graph を土台にするか** は、まだ `src/bin/dimpact.rs` 側の専用経路に閉じている

という状態にある。

したがって G4 の本丸は、単に bridge を 1 個ずつ増やすことではなく、
まず **PDG がどこまで build されるか** と **通常 impact の engine policy とどう揃えるか** を小さい設計単位に分解することになる。

---

## 1. G3 時点で既に揃ったもの

G3 で改善済みの点は、G4 の前提として明確に切り分けておく。

### 1.1 出力契約の最低限は前進した

G3 では次が入った。

- `--per-seed` が PDG / propagation でも動く
- `merge_pdg_references()` で `call` / `data` / `control` の kind と provenance を main output 側へある程度持ち込めるようになった
- `compute_impact()` の `impacted_witnesses` で last-hop の minimal witness を返せるようになった

参照:

- `docs/g3-10-rollup-summary.md`
- `src/bin/dimpact.rs` (`merge_pdg_references`, `build_grouped_impact_outputs`)
- `src/impact.rs` (`compute_impact`)

つまり G3-1 時点で強かった「最後に全部 `Reference::Call/Inferred` に潰れる」という問題は、
**完全ではないがそのままではない** ところまで改善されている。

### 1.2 CLI 上の最低限の feature parity も少し進んだ

G3-1 では `--per-seed` 未対応が目立つズレだったが、これは解消済み。
そのため G4 で残る CLI 上の大きなズレは、
**option が見えているのに実装経路が実際には揃っていない部分** に寄っている。

特に問題なのは:

- `--engine auto|ts|lsp`
- `--auto-policy`
- `--engine-lsp-strict`

が PDG mode でも CLI には出てくる一方、実際の PDG graph 構築は engine abstraction に十分乗っていないこと。

---

## 2. 現在の実装の短い再確認

現状の PDG path は、だいたい次の形になっている。

1. diff mode では `compute_changed_symbols()` で changed symbols を求める
2. seed mode では CLI seed をそのまま使う
3. `build_pdg_context()` が cache を開き、`cache::load_graph()` で symbol graph を読む
4. `build_local_dfg_for_paths()` が local DFG を作る
5. `PdgBuilder::build()` が call refs を混ぜて PDG を組む
6. `augment_symbolic_propagation()` で propagation bridge を追加する
7. `merge_pdg_references()` で `Reference` 群へ戻して `compute_impact()` に流す

参照:

- `src/bin/dimpact.rs` (`run_impact`, `build_pdg_context`, `build_local_dfg_for_paths`, `merge_pdg_references`)
- `src/dfg.rs` (`PdgBuilder::build`, `augment_symbolic_propagation`, `build_function_summaries`)
- `src/cache.rs` (`load_graph`)

G3 で出力 metadata は改善されたが、
**graph をどう集めて build するか** の責務はまだ CLI 側に強く残っている。

---

## 3. 残課題の棚卸し: build scope

## 3.1 local DFG build 対象が「changed/seed file そのもの」で止まっている

`build_pdg_context()` に渡している `local_dfg_paths` は、現状だと

- diff mode: `changed.changed_files`
- seed mode: `seeds` が属する file 集合

にほぼ等しい。

つまり今の PDG build scope は
**seed file 単体、または changed file 単体の列挙** であって、
そこから caller / callee / import / defining file へ閉包を広げる規則を持っていない。

### 影響

- callee が別 file にいると、その file に DFG が立たず summary bridge が育たない
- return-flow を跨ぐ改善をしても multi-file 側では再現しにくい
- alias / temporary variable が file を跨いだ瞬間に通常 call graph へ戻りやすい

これは G4 tasklist の

- G4-2: multi-file 固定評価セット
- G4-3: build scope を関連 file を含む最小集合へ広げる
- G4-4: multi-file call-site / return-flow bridge

の前提そのもの。

## 3.2 build scope の決定ロジックが明示的な policy になっていない

現状は CLI 分岐の中で

- cache をどの path で update するか
- local DFG をどの path で build するか

をその場で組み立てている。

このため、将来 scope を少しでも広げると、次の問いが散らばりやすい。

- changed file は常に scope に入れるのか
- seed symbol の定義 file だけ入れるのか
- direct callee file まで入れるのか
- direct caller file も必要か
- max depth / file budget / language allowlist をどう切るのか

今のままだと scope 拡張が「if を足す作業」になりやすく、
G4-3 で欲しい「最小集合の方針」がコード構造に現れにくい。

## 3.3 build scope が cache update scope / DFG scope / evaluation scope で分離されていない

PDG には少なくとも 3 種類の scope がある。

1. **cache update scope**: cache を再計算・更新する file 集合
2. **local DFG scope**: 実際に DFG node/edge を立てる file 集合
3. **evaluation scope**: fixture / regression で before/after を比較する対象ケース

現状の実装では 1 と 2 が近い形でそのまま渡されることが多く、
3 は docs / tests に散っている。

この結合のままだと、たとえば
「cache は changed files だけ update したいが、DFG は direct callee file まで含めたい」
のような設計が出たときに、責務が曖昧になりやすい。

G4 ではまず、この 3 つを言葉として分ける必要がある。

## 3.4 言語ごとの build scope の意味が揃っていない

`build_local_dfg_for_paths()` が local DFG を作るのは現在 `.rs` と `.rb` のみ。
つまり scope を広げても、実質的に恩恵が大きいのは Rust / Ruby に限られる。

これは現時点では悪いことではない。
むしろ G4 では **言語 parity を急がない** ほうが安全。

ただし設計上は、少なくとも次を明示すべき。

- scope planner は multi-language でも動くが、DFG builder の有無で enrich 量が変わる
- build scope 拡張 = 全言語で即 parity ではない
- regression は「scope が広がったか」と「bridge が効いたか」を分けて観測する

ここを混ぜると、G4-3 の scope 改善と G4-8 の engine 差分 baseline が同時にぼやける。

## 3.5 raw PDG DOT は scope の観測面として useful だが、理由説明はまだ足りない

`impact --with-pdg -f dot` は raw PDG を返すので、scope が足りない時の観測には役立つ。
ただし現状では

- なぜその file が scope に入ったか
- 逆になぜ入らなかったか
- scope expansion でどの bridge が新規に立ったか

までは出ない。

G4 で scope を広げるなら、少なくとも設計メモとしては
**scope decision を fixture と一緒に追える形にする** 方針を持っておいた方がよい。

---

## 4. 残課題の棚卸し: engine consistency

## 4.1 PDG mode は依然として `AnalysisEngine` を本体経路として通っていない

通常 impact は `make_engine_with_auto_policy()` で engine を選び、
`AnalysisEngine::{changed_symbols, impact, impact_from_symbols}` を通す。

一方で PDG mode は `run_impact()` の中で専用分岐に入り、
`build_pdg_context()` + `compute_impact()` を直接呼ぶ。

つまり現在の `--with-pdg` / `--with-propagation` は、
**出力段では既存 impact を使うが、graph 構築段では engine contract の外側にいる**。

### 影響

- `--engine lsp` を付けても PDG 側の graph source は `cache::load_graph()` に固定される
- `--auto-policy strict-if-available` を付けても PDG 側では strict LSP 優先の意味が通らない
- 将来 LSP が richer certainty / provenance / dynamic target separation を持っても、PDG 側へ自然には流れ込まない

## 4.2 diff mode の changed symbol 決定も engine 非依存経路になっている

PDG diff mode では `engine.changed_symbols()` ではなく `compute_changed_symbols()` を使っている。
これは TS ベースの analyzer 群で symbol を取る経路であり、
engine policy と同じ選択面ではない。

したがって現状の PDG mode では、
**changed symbol 決定** と **base graph 決定** の両方で engine consistency が崩れている。

これは G4-8 の「engine 間 baseline 比較」をやる前に、
少なくとも論点として固定しておくべきポイント。

## 4.3 cache graph の edge semantics は engine richness を保存していない

`cache::load_graph()` は現在、edge をすべて

- `RefKind::Call`
- `EdgeCertainty::Inferred`
- `EdgeProvenance::CallGraph`

として読み戻す。

G3 で `merge_pdg_references()` 側の metadata は改善されたが、
その土台にある base refs が最初から coarse なので、
engine 差分を PDG の前段に反映させるにはまだ弱い。

特に LSP 側が将来 richer metadata を持っても、
今の cache load contract のままだと PDG build 入り口で flatten されやすい。

## 4.4 engine option が「見えているのに効き方が揃わない」状態が一番危ない

ユーザー視点で一番ややこしいのは、
engine option が存在しないことではなく、**存在するのに PDG mode では意味が薄い** こと。

今の README は通常 impact の engine policy を丁寧に説明しているが、
PDG mode では次のズレが残る。

- `--engine auto --auto-policy strict-if-available` を付けても PDG 自体は strict LSP path にならない
- `--engine lsp --engine-lsp-strict` を付けても、PDG build に必要な base refs / changed symbols は strict LSP contract で生成されない
- diff / seed / per-seed の各 PDG 分岐が CLI 内部の専用経路なので、engine 由来の差分を fixture に固定しづらい

これは実装上のズレであると同時に、ドキュメント上のズレでもある。

## 4.5 現状のテスト面は「PDG の良し悪し」と「engine 差分」を十分に分離できていない

`tests/cli_pdg_propagation.rs` は G3 regression として有効だが、主に見ているのは

- Rust / Ruby の local PDG/propagation improvement
- no-leak guard
- per-seed output

であり、engine 差分ではない。

逆に `tests/engine_lsp.rs` は engine 側の strict E2E を持つが、PDG mode を本格的には見ていない。

そのため今のテスト面では、

- scope が足りなくて拾えないのか
- engine が違って base graph が変わるのか
- propagation bridge 自体が弱いのか

を分けて観測しにくい。

G4-8 で baseline を追加する理由はここにある。

---

## 5. build scope と engine consistency は別問題ではなく、結合問題

G4 で重要なのは、build scope と engine consistency を別々の TODO に見せつつ、
設計としては同時に扱うこと。

理由は単純で、scope だけ広げても base graph が engine ごとに揃わなければ、
「multi-file で拾えた / 拾えない」の意味が曖昧になるから。

逆に engine だけ揃えても、PDG build scope が seed file 単体のままだと、
PDG の上積みが local のままで止まりやすい。

つまり G4 の設計単位としては、

- **base graph provider** をどう決めるか
- **local DFG scope planner** をどう決めるか
- その上で **bridge / witness / merge policy** をどう載せるか

の 3 層に分けるのが自然。

---

## 6. G4 の設計方針

ここでは、G4 で採るべき最小方針を固定する。

## 6.1 いきなり project-wide PDG にしない

G3-1 でも整理した通り、いま必要なのは「全 file の DFG を常時 build する」ことではない。
それをやると

- build cost
- fixture の複雑化
- regression の説明難度

が一気に上がる。

G4 ではまず、
**seed/changed file から 1-hop か 2-hop 程度の関連 file を選ぶ最小 scope planner**
を設ける方がよい。

## 6.2 engine が決めるのは「土台の symbol graph」、PDG が決めるのは「局所 enrich」

責務分離としてはこれが一番わかりやすい。

- engine:
  - changed symbol 決定
  - base symbol graph / base refs 供給
  - auto-policy / strictness / capability handling
- PDG layer:
  - scope planner
  - local DFG build
  - propagation bridge
  - merge policy / witness enrichment

この分け方にすると、`--engine` が PDG mode でも意味を持ちやすい。

## 6.3 scope planner を明示的な struct / policy に切り出す

G4-3 の最小単位としては、たとえば次のような責務を持つ planner が欲しい。

- 入力:
  - seed symbols
  - changed files
  - base refs / symbol index
  - direction
- 出力:
  - `cache_update_paths`
  - `local_dfg_paths`
  - scope に入れた理由の要約

名前は `PdgBuildScope` / `PdgScopePlan` / `PdgAugmentationPlan` など何でもよいが、
重要なのは **CLI 分岐の即席ベクトルではなく、意味を持った計画オブジェクトにすること**。

## 6.4 G4 の最初は Rust / Ruby に寄せたままでよい

scope planner 自体は language-agnostic でも、
実際に DFG enrichment が効くのは当面 Rust / Ruby でよい。

ここで無理に Go / Java / Python / JS / TS / TSX の DFG parity を目標にすると、
G4 の焦点が

- multi-file build scope
- engine consistency
- witness / merge policy

から逸れやすい。

## 6.5 engine baseline は「PDG なし」と「PDG あり」を分けて観測する

G4-8 の baseline では、少なくとも 2 面必要。

1. 同じ diff / seed に対する通常 impact の engine 差分
2. 同じ diff / seed に対する PDG mode の engine 差分

ここを分けないと、scope 改善で増えた edge と engine 差分で増えた edge が混ざる。

---

## 7. 最小アーキテクチャ案

大改造ではなく、G4 で現実的な最小案を置いておく。

## 7.1 `build_pdg_context()` を「base graph + scope plan + local enrich」に分解する

現状の `build_pdg_context()` は責務が広い。
G4 では少なくとも概念上、次の 3 段に分けたい。

1. **base graph resolve**
   - engine policy に従って `SymbolIndex` / `Reference` を得る
2. **scope plan resolve**
   - seed / changed / refs から local DFG 対象 file 集合を決める
3. **local enrich merge**
   - local DFG build
   - propagation bridge
   - merged refs 生成

この分け方なら、G4-3 / G4-4 / G4-8 を個別に前進させやすい。

## 7.2 diff mode と seed mode で planner 入力だけを変え、PDG pipeline 自体は揃える

今は diff mode と seed mode で分岐が濃い。
ただし本質的に違うのは

- initial seeds をどう得るか
- scope の初期 file 集合が何か

だけで、PDG pipeline 自体は極力共通にした方がよい。

G4 では

- diff mode
- explicit seed mode
- per-seed mode

の 3 つで **同じ planner / builder を通す** ことを目標にしたい。

## 7.3 scope reason を最低限テストできるようにする

最初から user-visible JSON schema に入れる必要はないが、
少なくとも内部的には

- seed file
- direct callee file
- direct caller file
- same-module companion file

のどの理由で file を scope に入れたかを追える方がよい。

これは G4-2/G4-7 の fixture で「なぜこの file まで含めたのか」を説明する助けになる。

## 7.4 base refs の provenance/certainty を将来拡張可能なまま保つ

G4-1 の時点で cache schema を即変更する必要はないが、
少なくとも設計としては

- cache 由来の coarse edge
- engine 由来の richer edge
- local_dfg / symbolic_propagation 由来の augmented edge

を区別できる余地を残すべき。

そうしないと G4-8 で engine 差分を測っても、PDG 入口でまた flatten される。

---

## 8. 推奨する着手順

G4 tasklist に対応づけると、順番は次が自然。

### Step 1: G4-2

multi-file で弱いケースを 3〜5 件に固定する。

狙い:

- scope が狭いせいで拾えないケース
- scope は足りるが bridge が弱いケース
- engine 差分が見えやすいケース

を分ける。

### Step 2: G4-3

`PdgBuildScope` 相当の planner を設計し、
seed file 単体から direct related files を含む最小集合へ広げる方針を決める。

### Step 3: G4-4

scope が広がった状態で、一番効く multi-file bridge を 1 点だけ入れる。
候補は call-site / return-flow のどちらかを優先するのがよい。

### Step 4: G4-8

engine 差分 baseline を追加し、
「scope 改善で勝ったのか」「engine 差分で勝ったのか」を見分けられるようにする。

そのあとで

- G4-5 merge policy
- G4-6 witness/provenance 強化
- G4-7 CLI regression 拡張
- G4-9 docs 更新
- G4-10 rollup

へ繋ぐのが一番自然。

---

## 9. この時点で固定しておきたい判断

### 判断 1

**G4 の最初の敵は「bridge 不足」そのものではなく、「scope planner 不在」と「engine contract からの逸脱」。**

### 判断 2

**PDG を multi-file にしたいなら、先に「どの file まで build するか」を first-class な設計対象にする必要がある。**

### 判断 3

**`--engine` 系 option は PDG mode でも意味が通るべきで、少なくとも “効いているように見えるが実装経路は別” 状態は縮めるべき。**

### 判断 4

**G4 では project-wide PDG を目標にしない。**
まずは Rust/Ruby を中心に、関連 file を含む最小 multi-file augmentation を成立させる。

---

## 10. 一言まとめ

G3 で PDG path はかなり整理されたが、G4 の前に残っている本質的なズレはまだ 2 つある。

1. **build scope が seed/changed file 直打ちのままで、multi-file を計画的に扱えていない**
2. **PDG mode が engine contract の外で graph を組み立てるため、`--engine` / `--auto-policy` / strict LSP と整合していない**

したがって G4 の設計は、

- scope planner の導入
- base graph provider と local enrich の責務分離
- engine baseline と PDG baseline の切り分け

から始めるのが最も筋が良い。
