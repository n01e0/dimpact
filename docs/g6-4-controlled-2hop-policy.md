# G6-4: bounded slice を controlled 2-hop へ拡張する policy

このメモは、G5-3 の bounded project-slice policy を土台にしつつ、
G6 で Tier 2 を **single extra file** から **controlled 2-hop** へ引き上げるための policy を固定するもの。

G6-1 では、現行 builder が

- planner reason は出せるようになり始めたが
- bridge completion は still shallow で
- seed ごとに first acceptable file を 1 回だけ足す作りに近い

という問題を整理した。

G6-2 / G6-3 では、その選択理由を `summary.slice_selection` として露出できるようにした。
したがって G6-4 で次に決めるべきことは、
**"どの 2-hop を bounded のまま許すか"** である。

ここでの狙いは scope を project-wide に広げることではない。
むしろ

- graph-first
- per-seed planning
- file-level reason
- small budget
- deterministic prune

を保ったまま、**boundary side ごとに 2-hop を controlled に許す** ことにある。

machine-readable policy: `docs/g6-4-controlled-2hop-policy.json`

---

## 1. Goal / Non-goal

## Goal

- Tier 2 を「per-seed 1 file」ではなく、**boundary side 単位で制御された 2-hop expansion** として定義する
- `summary.slice_selection` に出る reason / `bridge_kind` / prune diagnostics と矛盾しない selection rule を固定する
- callers / callees / both で **side ledger と stop rule** を分ける
- Ruby `require_relative` 系に対して、graph-first を壊さない narrow companion fallback を定義する
- G6-5 / G6-6 / G6-7 が実装・回帰テストへ落とせる最小 policy を与える

## Non-goal

- 無制限 2-hop closure
- Tier 2 の先をさらに再帰展開すること
- import/path heuristic 主導へ戻すこと
- full witness graph / multi-candidate proof graph をここで設計すること
- Tier 3 fallback を一般の path discovery に昇格させること

---

## 2. G5-3 から何を変えるか

G5-3 のうち、次はそのまま維持する。

- Tier 0 = root files
- Tier 1 = direction-aware direct boundary files
- graph-first / per-seed planning / union execution
- cache update / local DFG / explanation の scope split
- `wrapper_return` / `boundary_alias_continuation` / `require_relative_chain` の bridge kind vocabulary

G6-4 で更新するのは主に次の 4 点である。

1. **Tier 2 の単位を per-seed から per-boundary-side に変える**
2. **call adjacency だけでなく bridge evidence を必要条件にする**
3. **callers / callees / both の stop rule を side ごとに分ける**
4. **Tier 3 companion fallback を Ruby `require_relative` 中心に narrow 化する**

一言で言えば、
G5-3 が `1-hop + 1 completion` だったのに対し、
G6-4 は **`1-hop + controlled side-local 2-hop`** にする。

---

## 3. controlled 2-hop の基本単位: boundary side

G6 では Tier 2 を seed 全体で 1 回だけ持つのではなく、
**boundary side** ごとに ledger を持つ。

概念上は次のような単位を採る。

```rust
struct BoundarySideKey {
    seed_symbol_id: String,
    boundary_symbol_id: String,
    boundary_path: String,
    side: BoundarySide,
}

enum BoundarySide {
    CallerSide,
    CalleeSide,
}
```

ここでの意図は単純である。

- same seed でも boundary は複数ありうる
- `both` では caller-side / callee-side を混ぜたくない
- "seed に 1 個だけ追加" では side ごとの bridge を説明できない

G6-4 では、Tier 2 候補の収集・選択・prune をまず **boundary side ごと** に行い、
そのあと per-seed / union へ折りたたむ。

### Rule

- 1 つの boundary side からは **最大 1 file** だけ Tier 2 として採る
- ただし seed 全体としては複数 side を持ちうる
- `both` では caller-side と callee-side の budget / ranking / prune を共有しない

これにより、"2-hop を許す" と言っても
**無制限 closure ではなく、side-local な 1 extra file** に制限できる。

---

## 4. 採用する Tier policy

## 4.1 Tier 0 / Tier 1 は G5-3 を維持

### Tier 0

- diff mode: changed files
- seed mode: explicit seed files

### Tier 1

- `callers`: incoming call の `from` file
- `callees`: outgoing call の `to` file
- `both`: その union。ただし caller-side / callee-side は別 ledger に載せる

Tier 0 / Tier 1 の reason kind もそのまま維持する。

- `seed_file`
- `changed_file`
- `direct_caller_file`
- `direct_callee_file`

## 4.2 Tier 2 = controlled 2-hop expansion

### Rule

Tier 2 は、Tier 1 で確定した boundary side を起点に、
**bridge evidence を満たす second-hop candidate を side-local に最大 1 file 選ぶ**。

ここで大事なのは次の 2 点。

1. second-hop だからといって call adjacency だけで足してはいけない
2. Tier 2 は boundary side ごとに 1 個までで、seed 全体の small budget も守る

### Accepted bridge kinds

当面の bridge kind は G5 と同じ 3 種類でよい。

1. `wrapper_return`
2. `boundary_alias_continuation`
3. `require_relative_chain`

### Required evidence

Tier 2 候補は、少なくとも次の 2 面を満たした時だけ候補化する。

1. **graph adjacency evidence**
   - boundary symbol から second-hop symbol/file へ base call graph 上の関係がある
2. **bridge-kind evidence**
   - その second hop が wrapper / alias / require-relative のどれを閉じるつもりなのかが説明できる

つまり G6-4 では、
**call adjacency は必要条件だが十分条件ではない。**

### Output

Tier 2 で選ばれた file は

- `summary.slice_selection.files[*].reasons[*].tier = 2`
- `kind = bridge_completion_file`
- `bridge_kind = wrapper_return | boundary_alias_continuation | require_relative_chain`

を持つ。

また落ちた候補は `pruned_candidates[]` に

- `tier = 2`
- 同じ `bridge_kind`
- 対応する `prune_reason`

を残す。

---

## 5. bridge evidence policy

G6-4 では bridge kind を単なる label にせず、
Tier 2 候補化の gate と ranking に使う。

## 5.1 `wrapper_return`

### どういう時に使うか

boundary file が wrapper / adapter / service 的な中継点で、
second-hop file がないと result-return chain が閉じにくい時。

典型:

- `main -> adapter -> core`
- `caller -> service -> serializer`

### 必要な説明

- boundary symbol が seed 側から見て direct boundary である
- second-hop file が boundary symbol の downstream / upstream wrapper return chain を閉じる
- その second hop を取ると witness / propagation / impacted symbol が説明しやすくなる

### Priority

Tier 2 では最優先。

## 5.2 `boundary_alias_continuation`

### どういう時に使うか

boundary file を経由した値や imported result が、
seed file 側または adjacent file 側 alias chain / assigned-result chain に繋がっている時。

典型:

- `main -> adapter -> value`
- `x = wrap(); y = x; out = y`

### 必要な説明

- second-hop file が boundary symbol/result の alias continuation 先として説明できる
- 単なる 2-hop fanout ではなく、value/return continuity がある

### Priority

`wrapper_return` の次。

## 5.3 `require_relative_chain`

### どういう時に使うか

Ruby で direct boundary の `mid.rb` だけでは return-ish flow / alias chain が閉じず、
`leaf.rb` 側までないと multi-file reasoning が痩せる時。

典型:

- `app/main.rb -> lib/mid.rb -> lib/leaf.rb`

### 必要な説明

- graph-first で boundary side は既に `mid.rb` として取れている
- `leaf.rb` は require-relative / alias / return-flow の split chain を閉じる second hop である
- path heuristic だけではなく、boundary side に紐づく Ruby bridge として説明できる

### Priority

`wrapper_return` / `boundary_alias_continuation` の後。
ただし Ruby 側の priority fixture では主役候補になりうる。

---

## 6. direction-aware stop rules

controlled 2-hop は direction ごとに止まり方を変える。

## 6.1 callers

### Rule

- Tier 1 は caller-side boundary を取る
- Tier 2 は **caller-side の second hop** だけを見る
- callee-side の graph branch へは乗り換えない

### Why

callers で欲しいのは、seed に到達する upstream bridge であって、
boundary を越えた先の unrelated callee fanout ではないから。

## 6.2 callees

### Rule

- Tier 1 は callee-side boundary を取る
- Tier 2 は **callee-side の second hop** だけを見る
- caller-side の graph branch へは乗り換えない

### Why

callees では downstream continuation を見たいのであって、
second hop を理由に upstream closure へ戻したくないから。

## 6.3 both

### Rule

- caller-side / callee-side の Tier 1 を別々に持つ
- Tier 2 budget も caller-side / callee-side で別 ledger にする
- caller-side candidate が callee-side budget を消費してはいけない

### Why

`both` を 1 つの大きな frontier にすると、
explainability も prune 診断も一気に曖昧になるから。

---

## 7. Tier 2 の scoring / ordering

G6-4 では Tier 2 を side-local に選ぶが、
その中の ordering は deterministic に固定する。

## 7.1 phase ordering

1. Tier 0 を確定
2. Tier 1 を boundary side として確定
3. boundary side ごとに Tier 2 candidate を収集
4. side-local ranking で Tier 2 を 1 つ選ぶ
5. per-seed budget で Tier 2 overflow を prune
6. 必要なら narrow Tier 3 fallback を評価
7. union budget prune

## 7.2 side-local ordering

同じ boundary side の Tier 2 候補は、少なくとも次で並べる。

1. `bridge_kind` priority
2. evidence strength
3. seed からの hop depth（浅い方）
4. certainty / confidence priority
5. lexical path order

### `bridge_kind` priority

1. `wrapper_return`
2. `boundary_alias_continuation`
3. `require_relative_chain`

### evidence strength

ここでは full proof score は要らない。
少なくとも

- graph adjacency + bridge evidence が明確
- graph adjacency はあるが bridge explanation が弱い

を区別できればよい。

G6-5 の最小実装では、side-local first acceptable hit ではなく
**ranked best candidate** を取ることを最低条件にする。

---

## 8. Budget / prune policy

G6-4 の controlled 2-hop は bounded のままでなければ意味がない。
そのため G5 より少し広げつつも、budget を正式に固定する。

## 8.1 初期 budget

G6 の初期値は次を採用する。

- `per_seed_tier1_files_max`: 4
- `per_boundary_side_tier2_files_max`: 1
- `per_seed_tier2_files_max`: 2
- `per_seed_tier3_files_max`: 1
- `union_cache_update_paths_max`: 14
- `union_local_dfg_paths_max`: 10
- `union_explanation_paths_max`: 14

### Why

- G5 の `per_seed_tier2_files_max = 1` では side-local bridge を持てない
- とはいえ 2-hop を自由化すると bounded slice でなくなる
- したがって **1 side = 1 file, 1 seed = at most 2 files** が最初の着地点としてちょうどよい

## 8.2 prune order

超過時は次の順で落とす。

1. Tier 3 fallback
2. 同じ boundary side の低順位 Tier 2 candidate
3. per-seed Tier 2 overflow side
4. Tier 1 の低優先候補
5. Tier 0 は最後まで保持

## 8.3 prune reasons

G6-3 で追加した schema に合わせ、少なくとも次を使い分ける。

- `already_selected`
- `bridge_budget_exhausted`
- `cache_update_budget_exhausted`
- `local_dfg_budget_exhausted`
- `ranked_out`

この区別により、

- candidate は見つかった
- でも side-local ranking で負けた
- もしくは global budget で落ちた

を出力から追える。

## 8.4 no recursive closure

Tier 2 の先をさらに Tier 2.5 / Tier 3 call expansion のように広げない。

G6-4 はあくまで
**direct boundary + controlled second hop** で止める policy であり、
project-wide closure へ向かうものではない。

---

## 9. narrow Tier 3 fallback

Tier 3 は残すが、G5 よりもさらに役割を narrow にする。

## 9.1 Rule

Tier 3 fallback は、次の条件をすべて満たす時だけ許す。

1. boundary side は graph-first に既に選ばれている
2. bridge kind は `require_relative_chain` または module companion 的ケースとして説明できる
3. Tier 2 graph-first candidate が無い、または budget / scope の都合で semantic coverage が痩せる
4. fallback が new frontier を作らず、既存 side に寄生する companion 追加に留まる

## 9.2 Allowed forms

- Ruby: `require_relative` で直接決まる companion / split chain leaf
- Rust: `foo.rs` / `foo/mod.rs` / `mod.rs` の module companion

## 9.3 Disallowed forms

- import/path heuristic だけで新しい side を起こすこと
- fallback を起点にさらに Tier 2 candidate を再収集すること
- Ruby companion を broad namespace discovery に使うこと

### Why

Tier 3 を広くしすぎると、controlled 2-hop ではなく
heuristic path expansion に戻ってしまうから。

---

## 10. mode policy

## 10.1 diff mode

- root seed: `changed.changed_symbols`
- Tier 0: `changed.changed_files`
- Tier 1: changed symbols から caller/callee boundary side を作る
- Tier 2: boundary side ごとに controlled second hop を最大 1 file
- Tier 3: narrow fallback のみ

## 10.2 seed mode

- root seed: explicit CLI seeds
- Tier 0: seed files
- Tier 1: seed symbols から caller/callee boundary side を作る
- Tier 2: side-local controlled second hop を最大 1 file
- Tier 3: narrow fallback のみ

## 10.3 per-seed mode

planner は必ず seed ごとに計画し、
実行時だけ union してよい。

ただし出力では必ず次を残す。

- selected Tier 2 reason
- `bridge_kind`
- side-local に落ちた `pruned_candidates`

`--per-seed` では `summary.slice_selection` の inner schema を変えない。
違いは outer nesting だけにする。

---

## 11. G6 acceptance mapping

この policy は、少なくとも次の受け皿になるべきである。

## 11.1 G6-5 最小実装

- existing Tier 2 を first acceptable file ではなく side-local ranked selection に置き換える
- `bridge_kind` を Tier 2 selection に実際に入れる
- per-boundary-side budget を実装する

## 11.2 G6-6 Rust target

- wrapper / adapter / alias continuation の 2-hop case で 1 件 real improvement を取る
- 少なくとも 1 ケースで
  - selected Tier 2 file
  - `bridge_kind`
  - `ranked_out` / budget diagnostics

が fixture と整合すること

## 11.3 G6-7 Ruby target

- `require_relative` split alias / return-flow の 3-file case で 1 件 real improvement を取る
- Ruby 側では
  - graph-first Tier 2
  - narrow companion fallback

のどちらで成立したかを明示できること

---

## 12. 実装挿入点メモ

G6-5 以降の実装では、少なくとも次を行うべきである。

1. `plan_bounded_slice()` を boundary side aware にする
2. Tier 2 candidate collection を side-local に分ける
3. side-local ranking を pure logic として切り出す
4. `ImpactSliceReasonMetadata.bridge_kind` を実際に埋める
5. `pruned_candidates` に `ranked_out` / `bridge_budget_exhausted` を残す
6. `both` で caller-side / callee-side の ledger を分離する

ここで重要なのは、
**selection reason schema は G6-3 で既にあるので、G6-5 は policy を実装に落とすことに集中できる** という点である。

---

## 13. 固定しておきたい判断

### 判断 1

**controlled 2-hop の単位は seed ではなく boundary side にする。**
ただし bounded 性のため `1 side = 1 file` を守る。

### 判断 2

**Tier 2 候補化には call adjacency だけでなく bridge evidence を要求する。**
2-hop fanout は completion とみなさない。

### 判断 3

**`both` は 1 本の大きな frontier にしない。**
caller-side / callee-side を別 ledger として扱う。

### 判断 4

**Tier 3 fallback は Ruby `require_relative` / module companion の narrow assist に留める。**
new frontier discovery には使わない。

### 判断 5

**controlled 2-hop は `summary.slice_selection` の reason / bridge_kind / prune diagnostics と一体で設計する。**
見えない selection policy に戻してはいけない。

---

## 14. 一言まとめ

G6-4 の controlled 2-hop policy は、
**direct boundary の次を seed 単位で雑に 1 file 足すのではなく、boundary side ごとに bridge evidence を満たす second-hop candidate を 1 file だけ選び、callers / callees / both の ledger と budget を分けたまま bounded に運用する** という方針である。

この policy なら

- shallow-boundary を越える 2-hop を少しだけ許せる
- planner reason / `bridge_kind` / prune diagnostics と整合できる
- Ruby `require_relative` 系も graph-first を崩さず narrow fallback で扱える
- G6-5 / G6-6 / G6-7 の実装と回帰テストへ素直に落とせる

ので、G6 の controlled 2-hop 方針として十分実用的である。
