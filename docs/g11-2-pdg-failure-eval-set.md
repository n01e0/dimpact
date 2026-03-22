# G11-2: current multi-file PDG failure fixed evaluation set

このメモは、G11 で戻り先にする **current main 向け fixed evaluation set** を決めるためのもの。

G4-2 では「multi-file にした時に弱くなりやすい代表ケース」を固定した。
ただ、main の PDG path はその後かなり進んでいる。

- bounded slice planner が入った
- `slice_selection_summary` / `pruned_candidates` が出るようになった
- direct boundary の cross-file summary bridge は既に一部回収できている
- Ruby の require_relative fallback も、少なくとも 1-hop の代表ケースは guard/test がある

そのため G11-2 では、昔の「multi-file にすると弱い」をそのままなぞるのではなく、
**main でまだ落ちている failure case** に寄せて fixed set を組み直す。

選定基準は次の 3 つ。

1. **current main で再現すること**
2. 既存の direct-boundary success case ではなく、**今の bounded frontier / propagation の限界**を踏むこと
3. 後続 task で docs / tests / code に落としやすいこと

今回の set では **5 ケース** を採用する。

- Rust: 4 ケース
- Ruby: 1 ケース

machine-readable set: `docs/g11-2-pdg-failure-eval-set.json`

---

## 1. この set で見たい failure family

G11-2 で固定したい failure family は、次の 5 種類。

1. **one-hop completion では足りない wrapper-return continuation**
2. **nested callee が multi-input になると summary continuation が痩せる問題**
3. **wrapper 内 alias chain までは見えるのに caller result へ戻り切れない問題**
4. **Ruby require_relative split で 2-hop continuation が閉じない問題**
5. **bridge completion budget により、妥当な leaf が slice から落ちる問題**

ここで重要なのは、G11 の主論点がもう

- 「callee file を scope に入れられるか」

だけではないこと。

main の実装では、direct boundary file の取り込み自体はかなり前進している。
いま本当に弱いのは、

- その先の continuation をどこまで閉じるか
- multi-input / alias / two-hop のどれを bridge family として扱えるか
- bounded planner の budget と propagation の bridge 形成がどこで頭打ちになるか

である。

---

## 2. 固定ルール

## 2.1 lane

この set の比較 lane は次で固定する。

- baseline: 通常 impact path
- pdg: `--with-pdg`
- propagation: `--with-propagation`

ただし全ケースで baseline を主比較にする必要はない。
current frontier は多くが
**PDG はある程度動くが propagation / continuation が最後まで閉じない**
という形なので、case ごとに primary lane を固定する。

## 2.2 engine

G11-2 の主題は engine 差分ではない。
したがって engine は G4-2 と同様に、まず安定面として `ts` に固定する。

- `--engine ts`
- `auto-policy` / strict LSP 差分はこの set では扱わない

## 2.3 primary view

観測の主画面は case ごとに次を使い分ける。

- **bridge そのもの**を見たい: `-f dot`
- **slice selection / pruned candidate** を見たい: `-f json --with-edges`
- 必要なら両方を見る

## 2.4 current-main 再現の扱い

この set は「理論上弱そう」な case 集ではなく、
**main で実際に落ちる shape** を固定する set とする。

したがって各 case には

- repro layout
- mutation
- primary command
- main で観測した gap

を最低限残す。

---

## 3. 採用ケース一覧

| case_id | lang | failure kind | primary view | ねらい |
| --- | --- | --- | --- | --- |
| rust-two-hop-wrapper-return-continuation | rust | FN | dot | `main -> wrap -> step -> leaf` の 2-hop continuation が caller result まで戻らない |
| rust-nested-two-arg-summary-continuation | rust | FN | dot | nested callee が 2 引数になると relevant arg continuation が caller result に戻らない |
| rust-cross-file-wrapper-return-alias-chain | rust | FN | dot | wrapper 内 alias chain は見えるが imported result が caller result まで閉じない |
| ruby-two-hop-require-relative-return-continuation | ruby | FN | dot | `main -> wrap -> step -> leaf` の require_relative split で 2-hop continuation が閉じない |
| rust-three-boundary-bridge-budget-overflow | rust | FN/scope | json | 3 つの boundary があると per-seed Tier2 budget で 1 つが落ちる |

---

## 4. 各ケースの固定意図

## 4.1 `rust-two-hop-wrapper-return-continuation`

### Layout

- `main.rs`
- `wrap.rs`
- `step.rs`
- `leaf.rs`

```rust
// main.rs
mod wrap;
mod step;
mod leaf;
fn caller() {
    let x = 1;
    let y = wrap::wrap(x);
    println!("{}", y);
}
```

```rust
// wrap.rs
pub fn wrap(a: i32) -> i32 {
    crate::step::step(a)
}
```

```rust
// step.rs
pub fn step(a: i32) -> i32 {
    let v = crate::leaf::leaf(a);
    v + 1
}
```

```rust
// leaf.rs
pub fn leaf(a: i32) -> i32 { a + 1 }
```

mutation は `main.rs` の `let x = 1;` → `let x = 2;` に固定する。

### Primary command

```bash
git diff --no-ext-diff --unified=0 | \
  dimpact impact --engine ts --direction callees --with-propagation --format dot
```

### Why this case

main の propagation は direct boundary + one-hop completion までは持っている。
しかしこの shape は

- `main -> wrap`
- `wrap -> step`
- `step -> leaf`

と **return continuation が 2 hop** 必要になる。

現在の実装では `step.rs:def:v` は見えても、caller 側の `use(x) -> def(y)` へ最後まで戻り切れない。
`leaf.rs` 側 local node も raw call symbol に比べると弱い。

### Current main observation

current main では次が見える。

- `step.rs:def:v:2` は出る
- `rust:wrap.rs:fn:wrap:1 -> main.rs:def:y:6` は出る
- `main.rs:use:x:6 -> main.rs:def:y:6` は出ない
- `leaf.rs:def:a:1` も出ない

### What later success looks like

- propagation lane で caller 側 `use(x)` から `def(y)` まで bridge が戻る
- `leaf.rs` 側 node / witness も completion の一部として見える

---

## 4.2 `rust-nested-two-arg-summary-continuation`

### Layout

- `main.rs`
- `wrap.rs`
- `pair.rs`

```rust
// main.rs
mod wrap;
mod pair;
fn caller() {
    let x = 1;
    let y = 2;
    let out = wrap::wrap(x, y);
    println!("{}", out);
}
```

```rust
// wrap.rs
pub fn wrap(a: i32, b: i32) -> i32 {
    let v = crate::pair::pair(a, b);
    v + 1
}
```

```rust
// pair.rs
pub fn pair(a: i32, b: i32) -> i32 { b + 1 }
```

mutation は `main.rs` の `let y = 2;` → `let y = 3;` に固定する。

### Primary command

```bash
git diff --no-ext-diff --unified=0 | \
  dimpact impact --engine ts --direction callees --with-propagation --format dot
```

### Why this case

single-file の 2 引数 case には既に FP guard がある。
しかし current frontier は、
**nested callee が multi-input になったときの continuation** である。

この shape では caller 側の relevant arg は `y` で、
`wrap` 内には `v` があり、`pair` 側では `b` が効く。

現在の propagation は nested completion を持つが、
summary mapping が multi-input nested case で最後まで閉じない。

### Current main observation

current main では次が見える。

- `pair.rs:def:b:1` は出る
- `wrap.rs:def:v:2` は出る
- `wrap.rs:use:b:2 -> wrap.rs:def:v:2` は出る
- `main.rs:use:y:6 -> main.rs:def:out:6` は出ない

### What later success looks like

- relevant arg `y` だけが `out` まで戻る
- irrelevant arg `x` を巻き込まずに multi-input nested continuation を回収できる

---

## 4.3 `rust-cross-file-wrapper-return-alias-chain`

### Layout

- `main.rs`
- `wrap.rs`
- `value.rs`

```rust
// main.rs
mod wrap;
mod value;
fn caller() {
    let x = 1;
    let out = wrap::wrap(x);
    println!("{}", out);
}
```

```rust
// wrap.rs
pub fn wrap(a: i32) -> i32 {
    let y = crate::value::make(a);
    let alias = y;
    alias
}
```

```rust
// value.rs
pub fn make(a: i32) -> i32 { a + 1 }
```

mutation は `main.rs` の `let x = 1;` → `let x = 2;` に固定する。

### Primary command

```bash
git diff --no-ext-diff --unified=0 | \
  dimpact impact --engine ts --direction callees --with-propagation --format dot
```

### Why this case

これは one-hop continuation より少し嫌らしい。

- imported result `value::make(a)`
- wrapper 内 temp `y`
- wrapper 内 alias `alias`
- caller result `out`

という **return + alias** の複合 shape だから。

current main では wrapper 内の alias chain 自体は見えるが、
その chain が caller 側 `out` まで自然に閉じない。

### Current main observation

current main では次が見える。

- `wrap.rs:def:y:2 -> wrap.rs:def:alias:3` は出る
- `value.rs:def:a:1` も出る
- `main.rs:use:x:5 -> main.rs:def:out:5` は出ない

### What later success looks like

- imported callee result が `y -> alias -> out` まで途切れずに説明できる
- wrapper 内 alias chain だけで終わらず caller result まで閉じる

---

## 4.4 `ruby-two-hop-require-relative-return-continuation`

### Layout

- `main.rb`
- `lib/wrap.rb`
- `lib/step.rb`
- `lib/leaf.rb`

```ruby
# main.rb
require_relative "lib/wrap"

def entry
  x = 1
  y = Wrap.wrap(x)
  puts y
end
```

```ruby
# lib/wrap.rb
require_relative "step"

module Wrap
  def self.wrap(a)
    Step.step(a)
  end
end
```

```ruby
# lib/step.rb
require_relative "leaf"

module Step
  def self.step(a)
    v = Leaf.leaf(a)
    v + 1
  end
end
```

```ruby
# lib/leaf.rb
module Leaf
  def self.leaf(a)
    a + 1
  end
end
```

mutation は `main.rb` の `x = 1` → `x = 2` に固定する。

### Primary command

```bash
git diff --no-ext-diff --unified=0 | \
  dimpact impact --engine ts --lang ruby --direction callees --with-propagation --format dot
```

### Why this case

Rust だけでなく Ruby でも、current frontier は direct require_relative success ではなく
**2-hop continuation** である。

この shape は

- require_relative split
- wrapper module
- temp alias `v`
- downstream leaf

を全部使うので、Ruby 側の continuation の限界をかなり素直に踏む。

### Current main observation

current main では次が見える。

- `lib/step.rb:def:v:5` は出る
- `ruby:lib/leaf.rb:method:leaf:2` symbol は出る
- `main.rb:use:x:4 -> main.rb:def:y:4` は出ない
- `lib/leaf.rb:def:a:3` は出ない

### What later success looks like

- Ruby でも caller 側 `x -> y` の continuation が戻る
- require_relative split の先の leaf local node まで観測できる

---

## 4.5 `rust-three-boundary-bridge-budget-overflow`

### Layout

- `main.rs`
- `a_wrapper.rs` / `a_leaf.rs`
- `b_wrapper.rs` / `b_leaf.rs`
- `c_wrapper.rs` / `c_leaf.rs`

```rust
// main.rs
mod a_wrapper;
mod a_leaf;
mod b_wrapper;
mod b_leaf;
mod c_wrapper;
mod c_leaf;

fn caller() {
    let x = 1;
    let ya = a_wrapper::wrap_a(x);
    let yb = b_wrapper::wrap_b(x);
    let yc = c_wrapper::wrap_c(x);
    println!("{} {} {}", ya, yb, yc);
}
```

各 wrapper は対応する leaf を 1 回だけ呼ぶ最小形にする。
mutation は `main.rs` の `let x = 1;` → `let x = 2;` に固定する。

### Primary command

```bash
git diff --no-ext-diff --unified=0 | \
  dimpact impact --engine ts --direction callees --with-propagation --format json --with-edges
```

### Why this case

これは pure propagation failure というより、
**bounded slice planner の budget failure** を固定するケース。

current main は per-seed Tier2 bridge completion を 2 file までに制限している。
そのため 3 つの boundary が同じ seed から生えると、
3 個目の leaf は妥当でも slice から落ちる。

### Current main observation

current main の `summary.slice_selection` では次が観測できる。

- selected files: `a_leaf.rs`, `a_wrapper.rs`, `b_leaf.rs`, `b_wrapper.rs`, `c_wrapper.rs`, `main.rs`
- pruned candidate: `c_leaf.rs`
- prune reason: `bridge_budget_exhausted`
- `via_symbol_id`: `rust:c_wrapper.rs:fn:wrap_c:1`

### What later success looks like

- budget の持ち方を bridge family / seed frontier に合わせて見直せる
- 少なくとも「3 本目だから無条件に落ちる」形からは抜けられる

---

## 5. この set を採用する理由

G11-2 では、あえて direct boundary の success case を中心には置かなかった。
理由は単純で、そこは main ですでにかなり改善されているから。

いま固定すべきなのは、次のような **current frontier** である。

- one-hop を越える continuation
- nested multi-input continuation
- return と alias が混ざる continuation
- Ruby require_relative split の 2-hop continuation
- bounded planner の budget failure

この 5 ケースを固定しておくと、後続の G11-3〜G11-7 で

- scope の widening が効いたのか
- bridge family が増えたのか
- propagation の continuation が改善したのか
- それとも budget / ranking の見直しが必要なのか

を比較しやすい。

---

## 6. 次にやること

この set を土台に、後続 task では次の順で進めるのがよい。

1. まず case ごとの repro を tests/fixture に落とす
2. slice selection summary の expected shape を固定する
3. propagation edge / witness の before-after を固定する
4. 最後に README / README_ja の current limits を main 実装に合わせて更新する

要するに G11-2 は、単なるアイデア集ではなく
**今の main で本当に痛い multi-file PDG 失敗面を固定するための足場**
である。