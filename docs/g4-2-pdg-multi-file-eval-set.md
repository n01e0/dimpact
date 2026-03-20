# G4-2: multi-file PDG weak-case fixed evaluation set

このメモは、G4 で PDG / propagation を multi-file に広げるときに毎回戻って比較する
**固定評価セット** を決めるためのもの。

G3-2 の fixed set は、当時の実装境界に合わせて file-local の Rust/Ruby ケースへ寄せていた。
それは正しかったが、G4 の主論点は変わっている。

G4 でまず見たいのは、

- caller と callee が別 file に分かれた時の **call-site bridge**
- wrapper / temp alias / assigned result が別 file に分かれた時の **return-flow**
- imported callee の戻り値が caller 側 alias chain に入る時の **alias 連結**
- scope を広げた結果に起こりやすい **cross-file propagation の過剰漏れ**

である。

したがって G4-2 では、
**今の PDG path が multi-file で弱くなりやすい 5 ケース** を固定する。

- Rust: 3 ケース
- Ruby: 2 ケース

machine-readable set: `docs/g4-2-pdg-multi-file-eval-set.json`

---

## 1. この評価セットで見るもの

この set で見たいのは、主に次の 5 種類。

1. **cross-file call-site bridge の FN**
   - changed caller file の `use(x)` から、別 file の callee summary を経て caller 側 `def(y)` へ戻れるか
2. **cross-file return-flow の FN**
   - wrapper file を挟んだ戻り値の流れを、call-only ではなく local data-flow を含めて繋げられるか
3. **cross-file alias continuation の FN**
   - imported callee の結果が caller file 内の alias / reassignment chain に接続されるか
4. **scope expansion 後の witness / edge-shape 劣化**
   - impacted symbol 数だけ増えても、どこで繋がったかが観測できないままになっていないか
5. **cross-file propagation の FP guard**
   - related file を scope に入れた結果、dynamic target separation や no-leak guard を壊していないか

---

## 2. 固定ルール

## 2.1 共通 lane

この set では lane を次で固定する。

- baseline: 通常 impact path
- pdg: `--with-pdg`
- propagation: `--with-propagation`

## 2.2 engine の扱い

G4-2 の目的は **build scope / propagation weakness の固定** であって、engine 差分の比較ではない。
したがって、この set では engine 差分を混ぜない。

- 比較 lane は `--engine ts` 相当の安定面として扱う
- `auto-policy` / strict LSP の差分は **G4-8** に分離する

理由:

- 現状の PDG path は engine abstraction を十分には通っていない
- G4-2 で engine 差分まで混ぜると、scope 不足と engine 差分を分離しにくい

## 2.3 観測方法

- **graph-shape / bridge の有無**が本題のケースは `-f dot`
- **symbol-level の FP/FN guard** を見たいケースは `-f json --with-edges`
- 必要なら両方使うが、case ごとに primary view を固定する

## 2.4 更新ルール

この 5 ケースは G4 前半では**入れ替えない**。
追加はよいが、置換するなら「なぜ古いケースを外すのか」を別メモに残す。

---

## 3. 採用ケース一覧

| case_id | lang | kind | primary view | ねらい |
| --- | --- | --- | --- | --- |
| rust-cross-file-callsite-summary-bridge | rust | FN | dot | caller file → callee file を跨ぐ call-site summary bridge |
| rust-cross-file-wrapper-return-flow | rust | FN | dot + json | wrapper file を挟む return-flow / assigned-result chain |
| rust-cross-file-imported-result-alias-chain | rust | FN/FP | dot | imported callee result が caller 側 alias chain に入るか |
| ruby-cross-file-callees-chain-alias-return | ruby | FN | dot + json | split fixture 上の temp alias / return-flow chain |
| ruby-cross-file-dynamic-send-target-separation | ruby | FP | json | scope 拡張後も dynamic target separation を壊さない |

---

## 4. 各ケースの固定意図

## 4.1 `rust-cross-file-callsite-summary-bridge`

### Source

- `tests/cli_pdg_propagation.rs::setup_repo()` を multi-file 化した mirror
- module split の土台は `tests/cli_impact_imports.rs` / `tests/cli_impact_use_paths.rs` の既存 shape を借りる

### Planned layout

- `src/callee.rs`
  - `pub fn callee(a: i32) -> i32 { a + 1 }`
- `src/main.rs`
  - `mod callee;`
  - `fn caller() { let x = 1; let y = callee::callee(x); println!("{}", y); }`

mutation は `main.rs` の `let x = 1;` → `let x = 2;` に固定する。

### Why this case

G3 の最小 call-site summary bridge は single-file では固定できた。
G4 でまず欲しいのは、その同じ論点を
**callee を別 file へ逃がした時にも観測できること**。

今の build scope では local DFG が changed/seed file に寄るため、
このケースは multi-file にした瞬間に弱くなりやすい。

### Fixed compare

- direction: `callees`
- primary commands:
  - baseline: `dimpact impact --engine ts --direction callees --format dot`
  - pdg: `dimpact impact --engine ts --direction callees --with-pdg --format dot`
  - propagation: `dimpact impact --engine ts --direction callees --with-propagation --format dot`

### Current weak expectation

G4 着手時点では、baseline / pdg / propagation で impacted symbol 自体は大きくは崩れなくても、
**propagation 固有の `use(x) -> def(y)` 近い bridge が cross-file では薄い** 状態を想定する。

### What counts as success later

- propagation lane で caller 側 `use(x)` と `def(y)` の間に、callee file を跨いだ summary-connected flow が観測できる
- callee file 側 node / witness が「scope が広がった結果」として見える

### What counts as failure

- single-file では見えた call-site summary bridge が multi-file で消える → **FN**
- caller file 内の無関係 def/use まで広く結ぶ → **FP**

---

## 4.2 `rust-cross-file-wrapper-return-flow`

### Source

- `tests/cli_pdg_propagation.rs::setup_repo()` の発展形として新規 mirror fixture を作る
- 3 file chain にして `main -> adapter -> core` を固定する

### Planned layout

- `src/core.rs`
  - `pub fn core(a: i32) -> i32 { a + 1 }`
- `src/adapter.rs`
  - `pub fn wrap(a: i32) -> i32 { let v = crate::core::core(a); v + 1 }`
- `src/main.rs`
  - `mod core; mod adapter;`
  - `fn caller() { let x = 1; let y = adapter::wrap(x); println!("{}", y); }`

mutation は `main.rs` の `let x = 1;` → `let x = 2;` に固定する。

### Why this case

これは G4 で見たい **return-flow** の最小 multi-file 面。

- changed caller file
- wrapper file にある temp variable `v`
- core file の callee summary
- caller file の assigned result `y`

が 3 file に分かれるので、scope planner が弱いとすぐ call-only へ戻る。

### Fixed compare

- direction: `callees`
- primary commands:
  - baseline: `dimpact impact --engine ts --direction callees --format json --with-edges`
  - pdg: `dimpact impact --engine ts --direction callees --with-pdg --format dot`
  - propagation: `dimpact impact --engine ts --direction callees --with-propagation --format json --with-edges`

### Current weak expectation

G4 着手時点では、call graph 上の `caller -> wrap -> core` は見えても、
**`x -> v -> y` の return-ish flow は薄く、witness も call edge に寄りやすい** と想定する。

### What counts as success later

- propagation lane で `adapter.rs` 内の temp alias / assigned-result が観測できる
- `impacted_witnesses` が plain call だけでなく local_dfg / symbolic_propagation を含む最短経路になる

### What counts as failure

- wrapper file を跨いだ瞬間に data-flow が痩せ、`v` 周辺の bridge が出ない → **FN**
- wrapper file の unrelated node へ広がりすぎる → **FP**

---

## 4.3 `rust-cross-file-imported-result-alias-chain`

### Source

- `src/dfg.rs::rust_alias_chain_and_reassignment_prefers_latest_def()` の alias 論点を multi-file 化した mirror
- module import の土台は `tests/cli_impact_imports.rs` を借りる

### Planned layout

- `src/value.rs`
  - `pub fn make(a: i32) -> i32 { a + 1 }`
- `src/main.rs`
  - `mod value;`
  - `fn caller() { let x = 1; let y = value::make(x); let alias = y; let out = alias; println!("{}", out); }`

必要なら `let alias2 = alias;` を足して alias chain を 2 hop に固定する。
mutation は `main.rs` の `let x = 1;` → `let x = 2;` とする。

### Why this case

G3 で固定した alias / latest-def 論点は single-file の DFG quality を見るには十分だった。
G4 では、そこに
**cross-file の imported callee result が caller 内 alias chain に入る** という 1 段を足したい。

このケースがあると、

- callee file summary が無いせいで `x -> y` が弱いのか
- その後の caller file alias chain が壊れているのか

を切り分けやすい。

### Fixed compare

- direction: `callees`
- primary commands:
  - pdg: `dimpact impact --engine ts --direction callees --with-pdg --format dot`
  - propagation: `dimpact impact --engine ts --direction callees --with-propagation --format dot`

baseline は補助比較に留める。
このケースの本体は call graph ではなく、**cross-file result-to-alias stitching** だから。

### Current weak expectation

G4 着手時点では、caller file 側の alias chain 自体は見えても、
**imported callee result からその chain へ入る橋が弱い** 状態を想定する。

### What counts as success later

- `value::make(x)` の結果が `y -> alias -> out` の chain に綺麗につながる
- stale / unrelated alias edge を増やさずに結果を繋げる

### What counts as failure

- imported result が alias chain に接続されず、call edge だけで終わる → **FN**
- scope 拡張で stale alias / broad def-to-def edge が増える → **FP**

---

## 4.4 `ruby-cross-file-callees-chain-alias-return`

### Source

- `tests/fixtures/ruby/analyzer_hard_cases_callees_chain_alias_return.rb` を split した fixture
- `tests/cli_impact_ruby_require.rs` / `tests/cli_impact_ruby_require_nested.rs` の require-relative shape を借りる

### Planned layout

例:

- `lib/leaf.rb`
  - return-ish expression を持つ末端 method
- `lib/mid.rb`
  - `require_relative "leaf"`
  - `v = leaf_call; v + inc` 形の temp alias / return flow を保持
- `app/main.rb`
  - `require_relative "../lib/mid"`
  - top-level caller chain

mutation は G3 fixture と同様に、leaf か mid の `v + inc` を `(v + inc) + 1` へ変える形に固定する。

### Why this case

G3 の Ruby chain fixture は single-file では良い代表面だったが、
G4 ではそのままでは scope 論点を測れない。

この split 版では、

- require-relative による cross-file call graph
- mid file の temp alias
- leaf file の return-ish flow

が分かれるので、**local DFG + global call graph 境界**を multi-file で観測しやすい。

### Fixed compare

- direction: `callers`
- primary commands:
  - baseline: `dimpact impact --engine ts --direction callers --format json --with-edges`
  - pdg: `dimpact impact --engine ts --direction callers --with-pdg --format dot`
  - propagation: `dimpact impact --engine ts --direction callers --with-propagation --format json --with-edges`

### Current weak expectation

G4 着手時点では、callers chain 自体は baseline でも見えるが、
**mid / leaf にある temp alias / return-flow の説明力が multi-file で弱い** 状態を想定する。

### What counts as success later

- propagation lane で mid file の temp alias と leaf file の return-ish flow が観測できる
- witness が call-only ではなく、どこで data bridge が入ったかを示せる

### What counts as failure

- chain 中盤で temp alias 周辺の bridge が痩せる → **FN**
- scope 拡張で chain 外へ不要に広がる → **FP**

---

## 4.5 `ruby-cross-file-dynamic-send-target-separation`

### Source

- `tests/fixtures/ruby/analyzer_hard_cases_dynamic_send_public_send.rb` を split した fixture
- require-relative で target 定義 file / dispatch file / caller file を分ける

### Planned layout

例:

- `lib/targets.rb`
  - `target_sym`, `target_str`, その他 target method 群
- `lib/router.rb`
  - `require_relative "targets"`
  - `send(:target_sym)` / `public_send("target_str")` / local var 経由 target
- `app/main.rb`
  - `require_relative "../lib/router"`
  - dispatcher caller

mutation は `router.rb` で `send(:target_sym)` を `send(:target_sym).to_s` へ変える形に固定する。

### Why this case

scope を広げると、FN 改善だけでなく FP も増えやすい。
Ruby の dynamic send/public_send はその典型。

このケースは、multi-file scope を入れた後でも
**target separation を壊さないための guard** として重要。

### Fixed compare

- direction: `callers`
- primary commands:
  - baseline: `dimpact impact --engine ts --direction callers --format json --with-edges`
  - propagation: `dimpact impact --engine ts --direction callers --with-propagation --format json --with-edges`

### Current weak expectation

G4 着手時点では、multi-file scope 自体はまだ弱いため大きな上積みは出にくいはず。
しかし G4-3 / G4-4 で scope planner を広げた後は、ここがすぐ FP guard になる。

### What counts as success later

- `target_sym` と `target_str` の separation を保つ
- local-var mediated target だけを適切に追加し、全 target 一括 smearing を避ける

### What counts as failure

- related files を scope に入れた結果、dynamic target がまとめて広く impacted する → **FP**
- 既存の target-specific resolution が崩れる → **FN**

---

## 5. 今回あえて入れなかったもの

## 5.1 Go / Java / Python / JS / TS / TSX

G4-1 でも整理した通り、現状の PDG path で local DFG enrichment の主戦場は Rust / Ruby である。
したがって G4-2 にこれらを混ぜると、

- multi-file PDG scope の改善を見たいのか
- 通常 impact / engine 差分を見たいのか

が曖昧になりやすい。

これらの比較面は **G4-8 engine baseline** 側で扱うのが自然。

## 5.2 heavy repo-wide diff

G4-2 は fixed evaluation set なので、まずは

- どこで scope が足りないか
- どの file を入れると bridge が立つか
- どこで FP が増えたか

を局所化しやすい small/medium fixture を優先する。

heavy diff は G4-7 以降で regression を増やすときの追加候補に回す。

---

## 6. 設計メモ

### メモ 1

G4-2 の fixed set は、**single-file の G3 set をそのまま引き延ばすためではなく、scope planner の成否を見るため** に置く。

### メモ 2

この段階では、**impact symbol 数** だけでなく、**どの file のどの橋が増えたか** を見たいので、dot と `--with-edges` の両方を使う。

### メモ 3

FN 改善ケースだけでなく、scope expansion 後の FP guard を 1 件入れておく。
G4 ではそれが早い段階から必要になる。

### メモ 4

G4-2 の 5 ケースは、次の後続タスクに直接つながる。

- G4-3: scope planner 設計
- G4-4: multi-file call-site / return-flow bridge 実装
- G4-7: CLI regression 追加
- G4-8: engine baseline 分離

---

## 7. 一言まとめ

- G4-2 の fixed evaluation set は **5 ケース** で固定する
- Rust 3 / Ruby 2 に寄せ、multi-file で弱くなる call-site / return-flow / alias / FP guard を明示的に分ける
- engine 差分は混ぜず、scope と propagation の弱点を先に固定する
- この set を G4-3 / G4-4 / G4-7 の共通評価面として使う
