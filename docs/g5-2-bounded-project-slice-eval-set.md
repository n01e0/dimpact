# G5-2: bounded project-slice 用 2-hop+ multi-file fixed evaluation set

このメモは、G5 で bounded project-slice を入れるときに毎回戻って比較する
**2-hop 以上の multi-file 固定評価セット** を決めるためのもの。

G4-2 の fixed set は正しかった。
ただし G4 の主対象は **direct boundary files** までの最小拡張であり、
そこで効きやすいのは主に

- changed/seed file
- その 1-hop 隣接 file

までだった。

G5-1 で棚卸ししたとおり、G5 の本当の残課題はその先にある。
つまり、今見たいのは

- direct boundary file 自体が wrapper / alias / return-flow の中継点になっているケース
- さらにもう 1 file 足さないと bridge が閉じないケース
- 2-hop 以上の slice を取った瞬間に FP guard が必要になるケース

である。

したがって G5-2 では、G4-2 の 1-hop set を置き換えるのではなく、
**direct-boundary では足りない 2-hop+ ケースだけを別 set として固定する。**

- Rust: 2 ケース
- Ruby: 2 ケース

machine-readable set: `docs/g5-2-bounded-project-slice-eval-set.json`

---

## 1. この評価セットで見るもの

G5-2 で見たいのは、主に次の 4 種類。

1. **bridge-completion 不足の FN**
   - seed/changed file と direct boundary file だけでは足りず、3 file 目で bridge が閉じるか
2. **boundary-side alias continuation の FN**
   - imported result / wrapper result が caller 側 alias chain に 2-hop 以上で繋がるか
3. **bounded slice の explainability**
   - impacted symbol が増えるだけでなく、どの中継 file で bridge が入ったかが観測できるか
4. **2-hop slice 後の FP guard**
   - scope を 3 file に広げても dynamic target separation や no-smear が壊れないか

G4-2 の 1-hop cross-file call-site case は、G5 でも大事な non-regression だが、
この set では **control / baseline** として扱い、主対象にはしない。

---

## 2. 固定ルール

## 2.1 共通 lane

この set でも lane は次で固定する。

- baseline: 通常 impact path
- pdg: `--with-pdg`
- propagation: `--with-propagation`

必要に応じて lane を省略するケースはあるが、
基本は G4-2 と同じ 3 面を前提にする。

## 2.2 engine の扱い

G5-2 の目的は **bounded slice の必要性を固定すること** であり、engine 差分の比較ではない。
したがって engine 差分は混ぜない。

- 比較 lane は `--engine ts` の安定面
- strict LSP / auto-policy の比較は G5-8 側へ分離する

## 2.3 観測方法

- **graph-shape / bridge completion** が本題のケースは `-f dot`
- **symbol-level の FP/FN / witness / edge provenance** を見たいケースは `-f json --with-edges`
- G5 では witness の説明力も重要なので、G4 より `json --with-edges` の比重を少し上げる

## 2.4 G4-2 との関係

G4-2 の set は残す。
G5-2 の set はそれを置き換えない。

役割分担は次のとおり。

- **G4-2**: direct boundary expansion の成否を見る 1-hop set
- **G5-2**: bounded project-slice / bridge-completion の成否を見る 2-hop+ set

---

## 3. 採用ケース一覧

| case_id | lang | hops | kind | primary view | ねらい |
| --- | --- | --- | --- | --- | --- |
| rust-three-file-wrapper-return-completion | rust | 2-hop | FN | dot + json | `main -> adapter -> core` で Tier 2 の bridge completion が要る return-flow |
| rust-three-file-imported-result-alias-continuation | rust | 2-hop | FN/FP | dot + json | `main -> adapter -> value` を経た result が caller 側 alias chain に入るか |
| ruby-three-file-require-relative-alias-return-chain | ruby | 2-hop | FN | dot + json | `app/main -> lib/mid -> lib/leaf` の temp alias / return-flow chain |
| ruby-three-file-dynamic-send-target-separation | ruby | 2-hop | FP | json | `app/main -> lib/router -> lib/targets` に広げても dynamic target separation を壊さない |

4 ケースに絞る理由は単純で、G5 前半でまず必要なのが

- Rust の bridge-completion 2 面
- Ruby の split chain 1 面
- Ruby の FP guard 1 面

だからである。

---

## 4. 各ケースの固定意図

## 4.1 `rust-three-file-wrapper-return-completion`

### Source

- `docs/g4-2-pdg-multi-file-eval-set.md::rust-cross-file-wrapper-return-flow`
- `docs/g5-1-bounded-project-slice-design-memo.md`
- `tests/cli_pdg_propagation.rs::setup_cross_file_wrapper_two_arg_repo`

### Planned layout

- `src/core.rs`
  - `pub fn core(a: i32) -> i32 { a + 1 }`
- `src/adapter.rs`
  - `pub fn wrap(a: i32) -> i32 { let mid = crate::core::core(a); mid }`
- `src/main.rs`
  - `mod core; mod adapter;`
  - `fn caller() { let x = 1; let out = adapter::wrap(x); println!("{}", out); }`

mutation は `main.rs` の `let x = 1;` → `let x = 2;` に固定する。

### Why this case

これは G5 の **bridge completion** を最も説明しやすい最小面。

- Tier 0: `main.rs`
- Tier 1: `adapter.rs`
- Tier 2: `core.rs`

という構成になり、direct-boundary だけでは
`adapter.rs` の temp / assigned-result は見えても、
`core.rs` 側の summary を足さないと `x -> mid -> out` が閉じにくい。

### Fixed compare

- direction: `callees`
- primary commands:
  - baseline: `dimpact impact --engine ts --direction callees --format json --with-edges`
  - pdg: `dimpact impact --engine ts --direction callees --with-pdg --format dot`
  - propagation: `dimpact impact --engine ts --direction callees --with-propagation --format json --with-edges`

### Current weak expectation

G4 直後の direct-boundary 実装でも、`main -> adapter` までは改善する可能性がある。
しかし **`adapter -> core` を completion として選ばない限り、return-ish flow は call-only explanation に戻りやすい。**

### What counts as success later

- `core.rs` を含む 3-file slice が観測できる
- propagation lane で `main.rs:use:x` から `main.rs:def:out` までに `adapter.rs` と `core.rs` を含む bridge が出る
- witness / edge provenance に local_dfg または symbolic_propagation が入り、plain call-only で終わらない

### What counts as failure

- `adapter.rs` は入っても `core.rs` が slice に入らず、bridge が閉じない → **FN**
- 3-file 目を足した結果、adapter/core の unrelated node へ広がる → **FP**

---

## 4.2 `rust-three-file-imported-result-alias-continuation`

### Source

- `docs/g4-2-pdg-multi-file-eval-set.md::rust-cross-file-imported-result-alias-chain`
- `src/dfg.rs::rust_alias_chain_and_reassignment_prefers_latest_def`
- `tests/cli_impact_imports.rs`

### Planned layout

- `src/value.rs`
  - `pub fn make(a: i32) -> i32 { a + 1 }`
- `src/adapter.rs`
  - `pub fn wrap(a: i32) -> i32 { value::make(a) }`
- `src/main.rs`
  - `mod value; mod adapter;`
  - `fn caller() { let x = 1; let y = adapter::wrap(x); let alias = y; let out = alias; println!("{}", out); }`

必要なら `let alias2 = alias;` を足して alias chain を 2 hop に固定する。
mutation は `main.rs` の `let x = 1;` → `let x = 2;` に固定する。

### Why this case

G4-2 の imported-result alias case は 2 file でも意味があったが、
G5 では **completion を必要とする 3-file 版** に上げた方がよい。

- Tier 0: `main.rs`
- Tier 1: `adapter.rs`
- Tier 2: `value.rs`

この形にすると、見たいものが

- adapter boundary を越えた result summary
- caller 側 alias chain (`y -> alias -> out`)

の両方になり、bounded slice が必要な理由がはっきりする。

### Fixed compare

- direction: `callees`
- primary commands:
  - pdg: `dimpact impact --engine ts --direction callees --with-pdg --format dot`
  - propagation: `dimpact impact --engine ts --direction callees --with-propagation --format json --with-edges`

baseline は補助比較に留める。
本体は call graph ではなく **result-to-alias continuation** だから。

### Current weak expectation

direct-boundary だけだと `main -> adapter` は見えても、
**`value.rs` を completion として取らない限り、imported result が caller alias chain に綺麗に入らない** 状態を想定する。

### What counts as success later

- `value::make(x)` の結果が `adapter::wrap` を経て `y -> alias -> out` に繋がる
- stale alias / broad def-to-def edge を増やさず continuation できる
- witness が adapter boundary を挟んだ path を持つ

### What counts as failure

- imported result が adapter 呼び出しで止まり caller alias chain に入らない → **FN**
- alias chain 外の stale edge や broad fan-out が増える → **FP**

---

## 4.3 `ruby-three-file-require-relative-alias-return-chain`

### Source

- `docs/g4-2-pdg-multi-file-eval-set.md::ruby-cross-file-callees-chain-alias-return`
- `tests/fixtures/ruby/analyzer_hard_cases_callees_chain_alias_return.rb`
- `tests/cli_impact_ruby_require.rs`
- `tests/cli_impact_ruby_require_nested.rs`

### Planned layout

- `lib/leaf.rb`
  - return-ish expression を持つ leaf method
- `lib/mid.rb`
  - `require_relative "leaf"`
  - `v = leaf_call; v + inc` 形の temp alias / return-flow を保持
- `app/main.rb`
  - `require_relative "../lib/mid"`
  - top-level caller chain

mutation は `leaf.rb` か `mid.rb` の `v + inc` を `(v + inc) + 1` へ変える形に固定する。

### Why this case

G4 では Ruby の 2-file split までは改善面があったが、
G5 で bounded slice を測るには **`main -> mid -> leaf` の 3-file chain** にする必要がある。

このケースでは

- direct boundary の `mid.rb`
- completion の `leaf.rb`
- require-relative による path fallback

が同時に出るので、G5 planner の主要論点が一番よく出る。

### Fixed compare

- direction: `callers`
- primary commands:
  - baseline: `dimpact impact --engine ts --direction callers --format json --with-edges`
  - pdg: `dimpact impact --engine ts --direction callers --with-pdg --format dot`
  - propagation: `dimpact impact --engine ts --direction callers --with-propagation --format json --with-edges`

### Current weak expectation

direct-boundary だけでは `mid.rb` までは scope に入っても、
**`leaf.rb` の return-ish flow を completion で取らない限り、temp alias を含む callers chain の説明が薄い** と想定する。

### What counts as success later

- propagation lane で `mid.rb` の temp alias と `leaf.rb` の return-ish flow が一つの chain として見える
- witness / provenance にどこで data bridge が入ったかが出る
- require-relative companion に頼り過ぎず graph-first の slice で説明できる

### What counts as failure

- chain が `mid.rb` 周辺で痩せる、または `leaf.rb` が欠ける → **FN**
- linear chain の外へ不要に広がる → **FP**

---

## 4.4 `ruby-three-file-dynamic-send-target-separation`

### Source

- `docs/g4-2-pdg-multi-file-eval-set.md::ruby-cross-file-dynamic-send-target-separation`
- `tests/fixtures/ruby/analyzer_hard_cases_dynamic_send_public_send.rb`

### Planned layout

- `lib/targets.rb`
  - `target_sym`, `target_str`, その他 target method 群
- `lib/router.rb`
  - `require_relative "targets"`
  - `send(:target_sym)` / `public_send("target_str")` / local-var mediated target
- `app/main.rb`
  - `require_relative "../lib/router"`
  - router caller

mutation は `router.rb` で `send(:target_sym)` を `send(:target_sym).to_s` へ変える形に固定する。

### Why this case

bounded project-slice は FN 改善のために file を増やすが、
同時に **2-hop になった瞬間の FP guard** が必要になる。

Ruby の dynamic send/public_send はその代表面で、
3-file slice にしたときも target separation を壊さないかを先に固定しておく価値が高い。

### Fixed compare

- direction: `callers`
- primary commands:
  - baseline: `dimpact impact --engine ts --direction callers --format json --with-edges`
  - propagation: `dimpact impact --engine ts --direction callers --with-propagation --format json --with-edges`

### Current weak expectation

G4 の direct-boundary では大きな上積みがまだ出なくても、
G5 で `router.rb` から `targets.rb` まで slice を completion すると、このケースはすぐ guard になる。

### What counts as success later

- `target_sym` と `target_str` の separation を保つ
- local-var mediated target だけを適切に含める
- broad cross-file smearing を起こさない

### What counts as failure

- slice 拡張により dynamic target がまとめて impact する → **FP**
- 既存の target-specific resolution が崩れる → **FN**

---

## 5. 今回あえて入れなかったもの

## 5.1 1-hop direct-boundary case

G4-2 にある

- direct callee summary bridge
- 2-file Ruby alias-return

は引き続き重要だが、G5-2 の主目的は **direct-boundary では足りない面だけを固定すること** なので、主対象からは外した。

## 5.2 multi-seed / budget pressure case

per-seed reason や budget 超過は G5 で重要な論点だが、
最初の fixed set としては 1 case あたりの failure mode が混ざりやすい。

それらは G5-3/G5-4 で planner contract が入ったあとに、
別の determinism / budget set として切り出す方がよい。

## 5.3 Go / Java / Python / JS / TS / TSX

G5-2 の目的は bounded slice の必要性を Rust/Ruby で固定することなので、
engine richness や analyzer parity を主題にしやすい言語はここでは外す。

これらは G5-8 の engine consistency baseline 側で扱うのが自然。

---

## 6. 設計メモ

### メモ 1

G5-2 の set は **G4-2 の強化版** ではあるが、役割は違う。
G4-2 が direct boundary 用なら、G5-2 は bridge-completion / bounded slice 用である。

### メモ 2

ここで固定した 4 ケースは、G5-3/G5-4 の planner 実装にそのまま対応づけられる。

- Rust wrapper-return completion
- Rust boundary-side alias continuation
- Ruby 3-file split chain
- Ruby 2-hop FP guard

### メモ 3

G5-2 では **witness の説明力** も観測対象に含める。
G4 では edge の有無が主だったが、G5 では「どの file を経由して bridge が閉じたか」を見たい。

### メモ 4

この set は bounded project-slice の最初の acceptance surface なので、
ケース数は増やし過ぎない。
4 ケースなら、FN と FP の両面を保ったまま説明負荷を抑えられる。

---

## 7. 一言まとめ

- G5-2 の fixed evaluation set は **4 ケース** で固定する
- すべて **2-hop 以上** の Rust/Ruby multi-file case に寄せる
- direct-boundary では足りず、bridge completion または bounded slice が必要な面だけを選ぶ
- G4-2 を非退行 control としつつ、G5-3/G5-4/G5-5 の共通評価面として使う
