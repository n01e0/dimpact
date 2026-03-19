# G3-2: PDG / propagation 改善用 fixed evaluation set

このメモは、G3 で PDG / propagation を改善するときに毎回戻って比較する
**固定評価セット** を決めるためのもの。

G3-1 の棚卸しで見えた通り、現状の PDG path は

- Rust/Ruby の file-local DFG augmentation
- call-site bridge / alias / minimal function summary
- 最終的には `Reference` に潰して既存 `compute_impact()` へ戻す

という境界で止まっている。

したがって G3-2 では、いきなり多言語・project-wide の大きいセットを作るより、
**今の実装境界にちゃんと刺さる 3〜5 ケース** を固定する方がよい。

今回の結論は、**5 ケース** を採用すること。

- Rust: 2 ケース
- Ruby: 3 ケース

Python / Go / Java / JS / TS は、現状の PDG path では DFG 部分が本質的に立たないため、
G3-2 の fixed set には入れない。
そこを混ぜると「PDG の改善を見たい評価面」ではなく
「通常 impact path とほぼ同じ比較面」になってしまう。

- machine-readable set: `docs/g3-2-pdg-eval-set.json`

## 1. この評価セットで見るもの

このセットで見たいのは、主に次の 4 種類。

1. **call-site bridge の FN**
   - 引数 use → callee symbol → 代入先 def の橋渡しが落ちていないか
2. **alias / reassignment の FN / FP**
   - 最新 def を使うべき所で stale def を辿っていないか
3. **return-flow / temp alias の FN**
   - Ruby の `v = callee; v + 1` 系で summary/bridge が痩せすぎないか
4. **dynamic Ruby 周辺の FP guard**
   - propagation を足した結果、関係ない target まで smear していないか

## 2. 固定ルール

この set では、case ごとに比較する lane を固定する。

### 共通 lane

- baseline: 通常 impact path
- pdg: `--with-pdg`
- propagation: `--with-propagation`

### 観測方法

- **graph-shape が本題**のケースは `-f dot`
- **symbol-level の過不足**を見たいケースは `-f json --with-edges`

理由:

- 現状は PDG の説明力が `ImpactOutput` へ十分残っていない
- そのため G3-2 時点では、`dot` を一次観測面にするケースが必要

### 更新ルール

この 5 ケースは G3 の作業中に**入れ替えない**。
追加はよいが、置換する場合は「なぜ古いケースを外すのか」を別メモに残す。

## 3. 採用ケース一覧

| case_id | lang | source | kind | primary view | ねらい |
| --- | --- | --- | --- | --- | --- |
| rust-callsite-summary-bridge | rust | `tests/cli_pdg_propagation.rs` を mirror | FN | dot | 引数 use → callee → 受け側 def bridge |
| rust-alias-reassignment-latest-def | rust | `src/dfg.rs` test shape を mirror | FN/FP | dot | alias chain と latest-def 優先 |
| ruby-callees-chain-alias-return | ruby | `tests/fixtures/ruby/analyzer_hard_cases_callees_chain_alias_return.rb` | FN | dot + json | temp alias / return-flow chain |
| ruby-dynamic-alias-define-method-no-leak | ruby | `tests/fixtures/ruby/analyzer_hard_cases_dynamic_alias_define_method.rb` | FP | json | alias / define_method から unrelated target へ漏れない |
| ruby-dynamic-send-public-send-target-separation | ruby | `tests/fixtures/ruby/analyzer_hard_cases_dynamic_send_public_send.rb` | FN/FP | json | dynamic target separation を壊さない |

## 4. 各ケースの固定意図

## 4.1 `rust-callsite-summary-bridge`

### Source

`tests/cli_pdg_propagation.rs` の `setup_repo()` を mirror した最小 fixture。

### Why this case

これは今の propagation path が既に狙っている
**call-site bridge + function summary bridge** の最小確認面。

現状の実装で言うと、次が効いてほしい。

- callsite `use(x)` → callee symbol
- callee summary input / impacted node
- impacted node → callsite `def(y)`
- summary connected 時の `use(x) -> def(y)` shortcut

### Fixed compare

- direction: `callees`
- primary command:
  - baseline: `dimpact impact --direction callees --format dot`
  - pdg: `dimpact impact --direction callees --with-pdg --format dot`
  - propagation: `dimpact impact --direction callees --with-propagation --format dot`

### What counts as failure

- propagation lane で callee まわりの bridge edge が落ちる → **FN**
- propagation lane で unrelated node を広く結びすぎる → **FP**

### Why keep it

最小形なので、G3-3 / G3-4 で bridge を触ったときに壊れ方を最短で見つけやすい。

## 4.2 `rust-alias-reassignment-latest-def`

### Source

`src/dfg.rs` の unit test `rust_alias_chain_and_reassignment_prefers_latest_def()` を mirror した fixture。

### Why this case

これは PDG/propagation 以前に、DFG の質が悪いと全部崩れるタイプのケース。

見たい論点は 2 つ。

1. `a -> b -> d` の alias chain が残るか
2. `let c = a` が **line 1 の `a` ではなく line 3 の再代入 `a`** を使うか

### Fixed compare

- direction: `callees`
- primary command:
  - pdg: `dimpact impact --direction callees --with-pdg --format dot`
  - propagation: `dimpact impact --direction callees --with-propagation --format dot`

baseline は補助比較に留める。
このケースの本体は call graph ではなく DFG edge quality だから。

### What counts as failure

- `def:a:3 -> def:c:4` が無い → **FN**
- 代わりに `def:a:1 -> def:c:4` の stale edge が残る → **FP**

### Why keep it

G3-3 で alias / reassignment を触るなら、これが一番壊れやすい。
しかも false negative と false positive の両方を同時に見られる。

## 4.3 `ruby-callees-chain-alias-return`

### Source

`tests/fixtures/ruby/analyzer_hard_cases_callees_chain_alias_return.rb`

### Why this case

この fixture は real-corpus 由来の形を保ったまま、

- 線形 call chain
- `v = fNN`
- `v + inc`
- temp alias を挟んだ return-ish flow

を含んでいる。

G3 の Ruby 側で見たい「callees chain + temp alias + return-flow」の代表面としてちょうどよい。

### Fixed compare

- direction: `callers` を基本、必要なら `callees` も併記
- primary commands:
  - baseline: `dimpact impact --direction callers --format json --with-edges`
  - pdg: `dimpact impact --direction callers --with-pdg --format dot`
  - propagation: `dimpact impact --direction callers --with-propagation --format dot`

### What counts as failure

- chain 中盤で temp alias 周辺の data bridge が痩せて witness が作れない → **FN**
- propagation を足したことで chain 外へ広く漏れる → **FP**

### Why keep it

G3-1 で挙げた「local DFG + global call graph」境界の Ruby 側代表として使いやすい。
小さすぎず、でも heavy fixture ほど読みにくくない。

## 4.4 `ruby-dynamic-alias-define-method-no-leak`

### Source

`tests/fixtures/ruby/analyzer_hard_cases_dynamic_alias_define_method.rb`

### Why this case

これは Ruby の dynamic resolver が既に頑張っている領域に、
propagation を足したときの**漏れ方**を見るための guard case。

特に見たいのは:

- `original -> aliased_* -> defined_* -> execute` の筋は維持する
- しかし `defined_only` まで一緒に巻き込まない

### Fixed compare

- direction: `callers`
- primary command:
  - baseline: `dimpact impact --direction callers --format json --with-edges`
  - propagation: `dimpact impact --direction callers --with-propagation --format json --with-edges`

### What counts as failure

- `defined_only` や無関係な dynamic target が impacted 側へ出る → **FP**
- 既存の alias / define_method 解決が propagation によって崩れる → **FN**

### Why keep it

G3-4 で inter-procedural propagation を広げるほど、
こういう「dynamic に強く、でも leak はさせたくない」面が重要になる。

## 4.5 `ruby-dynamic-send-public-send-target-separation`

### Source

`tests/fixtures/ruby/analyzer_hard_cases_dynamic_send_public_send.rb`

### Why this case

`send/public_send` は Ruby で FP が増えやすい地点。
この fixture には

- symbol literal target
- string literal target
- local var 経由 target

が全部入っているので、propagation を足したときに
**target separation が壊れていないか** を見るのに向いている。

### Fixed compare

- direction: `callers`
- primary command:
  - baseline: `dimpact impact --direction callers --format json --with-edges`
  - propagation: `dimpact impact --direction callers --with-propagation --format json --with-edges`

### What counts as failure

- `target_sym` と `target_str` の separation が崩れ、まとめて広く impacted する → **FP**
- local-var 経由 target の橋渡しが消える → **FN**

### Why keep it

Ruby の dynamic dispatch を強くするほど、ここは必ず副作用が出やすい。
G3-4 以降の FP guard として固定しておく価値が高い。

## 5. 今回あえて入れなかったもの

## 5.1 Python / Go / Java / JS / TS

G3-1 の通り、現状の PDG path は Rust/Ruby 以外では DFG 部分が実質空振りになりやすい。
したがって G3-2 の fixed set にこれらを入れると、
「PDG を改善したか」ではなく「通常 impact path との差がほぼないか」を確認するだけになりやすい。

これは G3-2 の目的から外れるので、今回は入れない。

## 5.2 heavy diff

`bench-fixtures/ruby-heavy.diff` のような大きめ diff は、
最終的には欲しいが、G3-2 の最初の fixed set としては重い。

理由:

- graph を読むコストが高い
- どこで漏れた / どこで増えたかの切り分けが遅い
- G3-3 / G3-4 の初期ループでは小さい case の方が壊れ方を局所化しやすい

heavy diff は、G3-7 以降で regression 拡張するときの追加候補に回す。

## 6. 設計メモ

### メモ 1

G3-2 の fixed set は、**今の PDG path の実装境界に合わせて小さく保つ**。

### メモ 2

この段階では、**impact result の symbol 数だけでなく、PDG DOT / edge shape を一次評価面に含める**。

### メモ 3

Rust では bridge / alias quality、Ruby では temp alias / dynamic leak guard を軸に置く。

## 7. 一言まとめ

- G3-2 の fixed evaluation set は 5 ケースで固定する
- Rust 2 / Ruby 3 に寄せ、現状の PDG 実装境界にちゃんと刺さる比較面にする
- graph-shape を見るため、`dot` を一次観測面に含める
- ここで固定した 5 ケースを、G3-3 / G3-4 / G3-7 の回帰面として使う
