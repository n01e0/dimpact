# G12-2: bounded continuation の multi-input / alias-result stitching failure fixed eval set

このメモは、G12 で戻り先にする **current-main 向け fixed evaluation set** を決めるためのもの。

paired machine-readable set: `docs/g12-2-bounded-continuation-failure-eval-set.json`

G11 までで、bounded continuation path は少なくとも次の control case を通せるようになった。

- Rust two-hop wrapper return continuation
  - `tests/cli_pdg_propagation.rs::pdg_propagation_extends_two_hop_wrapper_return_through_rust_bridge_continuation_scope`
- Ruby two-hop require_relative wrapper return continuation
  - `tests/cli_pdg_propagation.rs::pdg_propagation_extends_two_hop_require_relative_wrapper_return_scope`
- imported result -> caller alias chain の基本ケース
  - `tests/cli_pdg_propagation.rs::pdg_propagation_extends_imported_result_into_caller_alias_chain`

そのため G12-2 では、もう「two-hop continuation file を scope に入れられるか」中心ではなく、
**G12-1 で整理した current weakness をそのまま evaluation case に落とす** ことを主眼にする。

今回の set では **5 ケース** を採用する。

- Rust: 4 ケース
- Ruby: 1 ケース

---

## 1. この set で見たい current frontier

G12-2 で固定したい frontier は次の 5 種類。

1. **nested multi-input continuation**
2. **reordered / partial input binding continuation**
3. **wrapper + caller をまたぐ alias-result stitching**
4. **alias family continuation beyond tier2**
5. **require_relative + alias-result mixed stitching**

ここで重要なのは、G12 の failure surface がもう

- callee file を scope に入れられるか
- direct boundary を 1 hop 越えられるか

だけではないこと。

current main は、single-input の return continuation や基本的な imported-result alias にはかなり強い。
いま本当に固定すべきなのは、

- **どの input がどの summary input に bind されたか**
- **selected alias family が実際に stitch execution へ落ちるか**
- **family-aware continuation / budgeting を持たないせいでどこが痩せるか**

である。

---

## 2. 固定ルール

## 2.1 lane

この set の比較 lane は次で固定する。

- baseline: 通常 impact path
- pdg: `--with-pdg`
- propagation: `--with-propagation`

ただし primary comparison は case ごとに固定する。
G12 の frontier は多くが **propagation / stitching contract の不足** なので、
primary lane は propagation になることが多い。

## 2.2 engine

G12-2 の主題は engine 差分ではない。
したがって engine は引き続き安定面として `ts` に固定する。

- `--engine ts`
- auto-policy / strict LSP 差分はこの set では扱わない

## 2.3 primary view

観測の主画面は case ごとに次を使い分ける。

- **bridge / stitch の成立そのもの** を見たい: `-f dot`
- **slice selection / pruned candidate / budget** を見たい: `-f json --with-edges`

## 2.4 current-main 記述の扱い

この set では、既存 green case 以外は fixture 化前の段階なので、
「current main で必ず再現済み」ではなく
**current code contract から見て failure になりやすい shape** として記述する。

したがって各 case には、次を残す。

- repro layout
- mutation
- primary command
- expected current-main gap
- later success の形

---

## 3. 採用ケース一覧

| case_id | lang | failure kind | primary view | ねらい |
| --- | --- | --- | --- | --- |
| rust-nested-two-arg-summary-continuation | rust | FN | dot | nested multi-input continuation で relevant arg だけを caller result まで戻せるか |
| rust-reordered-partial-input-binding-continuation | rust | FN | dot | reordered / partial input binding でも summary continuation を閉じられるか |
| rust-wrapper-caller-double-alias-result-stitching | rust | FN | dot | imported result が wrapper alias と caller alias をまたいで out まで閉じるか |
| rust-alias-family-continuation-beyond-tier2 | rust | FN/scope | json | `boundary_alias_continuation` family を tier2 の先へ 1 hop continuation できるか |
| ruby-require-relative-alias-result-mixed-stitching | ruby | FN | dot | require_relative split と alias-result stitching の混合 shape を閉じられるか |

---

## 4. 各ケースの固定意図

## 4.1 `rust-nested-two-arg-summary-continuation`

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

これは G11-2 でも見えていた current frontier で、
G12 では **multi-input continuation を input binding 問題として解く** ための基準点になる。

### Expected current-main gap

current contract では次が起きやすい。

- `pair.rs` 側 relevant input `b` は見える
- `wrap.rs:def:v` までは戻る
- しかし `main.rs:use:y -> main.rs:def:out` の bridge が最後まで閉じない
- irrelevant arg `x` を混ぜずに relevant input だけを選ぶ contract も弱い

### What later success looks like

- `main.rs:use:y -> main.rs:def:out` が回収される
- `main.rs:use:x -> main.rs:def:out` は増えない
- witness で selected input binding を compact に説明できる

---

## 4.2 `rust-reordered-partial-input-binding-continuation`

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
    let v = crate::pair::pair(b, 1, a);
    v + 1
}
```

```rust
// pair.rs
pub fn pair(left: i32, _lit: i32, right: i32) -> i32 { left + 1 }
```

mutation は `main.rs` の `let y = 2;` → `let y = 3;` に固定する。

### Primary command

```bash
git diff --no-ext-diff --unified=0 | \
  dimpact impact --engine ts --direction callees --with-propagation --format dot
```

### Why this case

G12-1 で分けた通り、multi-input の本体は input count ではなく
**input binding map** である。
この case はその中でも

- reordered positional binding
- literal を挟んだ partial binding

を一番小さく踏む。

### Expected current-main gap

current contract では、`summary.inputs` と `callsite_uses` の zip-by-order に強く依存するため、

- relevant caller input `y` を `pair(left)` に bind できない
- literal slot を無視した partial binding を持てない
- 結果として `main.rs:use:y -> main.rs:def:out` が閉じにくい

### What later success looks like

- reordered binding を明示的に持てる
- literal slot を companion / dropped binding として扱える
- `y` だけが `out` へ戻り、`x` は巻き込まれない

---

## 4.3 `rust-wrapper-caller-double-alias-result-stitching`

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
    let y = wrap::wrap(x);
    let out = y;
    println!("{}", out);
}
```

```rust
// wrap.rs
pub fn wrap(a: i32) -> i32 {
    let tmp = crate::value::make(a);
    let alias = tmp;
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

current main の imported-result alias 成功ケースは、主に wrapper 側で alias chain が閉じる最小面である。
G12 で欲しいのはその先、
**wrapper alias + caller alias の 2 段 stitching** である。

### Expected current-main gap

current contract では次が起きやすい。

- `value -> tmp -> alias` までは local DFG で見える
- しかし `alias -> main.rs:def:y -> main.rs:def:out` の caller-side stitch が generic bridge に依存しやすい
- alias-result stitching 全体を 1 本の family として説明できない

### What later success looks like

- imported result が `tmp -> alias -> y -> out` まで途切れずに閉じる
- witness に alias-result stitch chain が compact に残る
- return continuation と alias-result stitch を区別して説明できる

---

## 4.4 `rust-alias-family-continuation-beyond-tier2`

### Layout

- `main.rs`
- `adapter.rs`
- `value.rs`
- `shared.rs`

```rust
// main.rs
mod adapter;
mod value;
mod shared;
fn caller() {
    let x = 1;
    let out = adapter::wrap(x);
    println!("{}", out);
}
```

```rust
// adapter.rs
pub fn wrap(a: i32) -> i32 {
    crate::value::value(a)
}
```

```rust
// value.rs
pub fn value(a: i32) -> i32 {
    let mid = crate::shared::source(a);
    let alias = mid;
    alias
}
```

```rust
// shared.rs
pub fn source(a: i32) -> i32 { a + 1 }
```

mutation は `main.rs` の `let x = 1;` → `let x = 2;` に固定する。

### Primary command

```bash
git diff --no-ext-diff --unified=0 | \
  dimpact impact --engine ts --direction callees --with-propagation --format json --with-edges
```

### Why this case

この case では tier2 で `value.rs` が `boundary_alias_continuation` として選ばれても、
**same-family continuation を `shared.rs` まで 1 hop 伸ばしたい**。

G12-1 で切り出した「tier3 continuation が wrapper_return family に寄りすぎている」問題を、
一番小さく固定するためのケースである。

### Expected current-main gap

current planner contract では次が起きやすい。

- `value.rs` は tier2 alias candidate として選ばれうる
- しかし tier3 continuation anchor が wrapper-return 寄りなので `shared.rs` を continuation しにくい
- `summary.slice_selection` でも alias family の先が representative として残りにくい

### What later success looks like

- `shared.rs` が alias-family continuation として選ばれる
- `bridge_kind = boundary_alias_continuation` を保ったまま 1 hop continuation できる
- family-aware budget で alias representative が落ちにくくなる

---

## 4.5 `ruby-require-relative-alias-result-mixed-stitching`

### Layout

- `main.rb`
- `lib/wrap.rb`
- `lib/value.rb`
- `lib/leaf.rb`

```ruby
# main.rb
require_relative "lib/wrap"

def entry
  x = 1
  y = Wrap.wrap(x)
  out = y
  puts out
end
```

```ruby
# lib/wrap.rb
require_relative "value"

module Wrap
  def self.wrap(a)
    tmp = Value.make(a)
    alias_value = tmp
    alias_value
  end
end
```

```ruby
# lib/value.rb
require_relative "leaf"

module Value
  def self.make(a)
    mid = Leaf.leaf(a)
    mid
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

Rust だけでなく Ruby でも、G12 では
**require_relative provenance と alias-result stitching が混ざる shape** を固定しておく必要がある。

既存の two-hop require_relative return control caseだけでは、
Ruby の mixed stitching weakness は見えない。

### Expected current-main gap

current contract では次が起きやすい。

- `require_relative` split の先に leaf method symbol までは見える
- wrapper 側 alias `tmp -> alias_value` も file-local では見える
- しかし `leaf -> mid -> tmp -> alias_value -> y -> out` 全体を 1 本の mixed chain として閉じにくい
- witness でも require_relative continuation と alias-result stitching の合流が見えにくい

### What later success looks like

- Ruby でも `x -> y -> out` が mixed chain として閉じる
- `require_relative_chain` と alias-result stitching の両方が採用 chain に残る
- witness で mixed bridge execution provenance を compact に説明できる

---

## 5. この set を採用する理由

G12-2 では、意図的に current green control case を eval set の中心に置かなかった。
理由は単純で、そこはもう current main で regression 化されているからである。

いま固定すべきなのは、次のような **execution contract frontier** である。

- nested multi-input continuation
- reordered / partial input binding
- wrapper + caller をまたぐ alias-result stitching
- alias family continuation beyond tier2
- Ruby の require_relative + alias-result mixed stitching

この 5 ケースを固定しておくと、後続の G12-3〜G12-7 で

- binding map が効いたのか
- alias-result stitching family が効いたのか
- planner continuation family が広がったのか
- family-aware budget が効いたのか
- witness provenance が強くなったのか

を分けて比較しやすい。

---

## 6. 次にやること

この set を土台に、後続 task では次の順で進めるのがよい。

1. repro layout を tests/fixture に落とす
2. failure case ごとに scope / stitch / witness の観測面を分けて regression 化する
3. Rust で priority 1 件、Ruby で priority 1 件の改善を実装する
4. 最後に bridge execution provenance を compact metadata として出す

要するに G12-2 は、単なるアイデア集ではなく
**G12 の multi-input / alias-result stitching work をぶらさないための fixed failure baseline**
である。
