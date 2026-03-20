# G5-3: changed symbols / refs / call graph / cache index から bounded slice を選ぶ policy

このメモは、G5 で入れる **bounded project-slice planner** の selection policy を固定するためのもの。

G4-3 では、PDG build scope を

- seed / changed file
- direct boundary file
- 必要時の bridge completion 1 回

まで広げる方針を置いた。

それは正しかったが、G5 で本当に必要なのは、
それを `expand_related_local_dfg_paths()` 的な ad-hoc path union ではなく、
**changed symbols / refs / call graph / cache index から説明可能な slice plan を組み立てる内部契約**
として定義し直すことだ。

G5-1 では「何が direct-boundary で取りこぼされているか」を棚卸しし、
G5-2 では「2-hop 以上で bounded slice が必要な固定評価セット」を置いた。

G5-3 で決めるべきことは、実装より先に次の 4 点である。

1. **入力として何を使うか**
2. **そこからどの file を候補化するか**
3. **どの file を local DFG / cache update / explanation に載せるか**
4. **なぜその file が選ばれたかを per-seed で保持するか**

結論を先に書くと、G5 の bounded slice policy は

- `changed_symbols` を root seed にする
- `refs` から **base call graph adjacency** を抽出する
- `SymbolIndex` を使って symbol -> file / file -> symbol を安定化する
- まず **direct boundary** を per-seed に選ぶ
- 次に **bridge completion** を 1 回だけ許す
- 最後に必要時だけ **module/require_relative companion fallback** を足す

という 3 段 + budget の policy にするのが筋がよい。

machine-readable policy: `docs/g5-3-bounded-project-slice-policy.json`

---

## 1. Goal / Non-goal

## Goal

- `changed symbols / refs / call graph / cache index` から、
  **bounded project-slice を deterministic に選ぶ** policy を定義する
- G5-2 の 2-hop+ fixed set に対して、
  どの file が Tier 1 / Tier 2 / fallback なのかを説明できるようにする
- `build_pdg_context()` の入力を path list 直渡しから、
  **reason-aware plan object** に置き換えられる設計にする
- `--with-pdg` / `--with-propagation` / `--per-seed` の全てで使える共有 policy を固定する

## Non-goal

- 無制限 project-wide closure
- 全言語で local DFG を build すること
- witness schema の最終形をここで決め切ること
- engine consistency の最終解決（これは G5-8 以降）

---

## 2. 入力として扱うもの

G5 planner は、最低限次の 4 つを一次入力として扱う。

## 2.1 changed symbols

- diff mode では `engine.changed_symbols()` / `compute_changed_symbols()` の出力
- seed mode では CLI から与えられた seed symbols

これは **root seed set** になる。

## 2.2 refs

`cache::load_graph()` から得られる `Vec<Reference>` を使う。

ただし bounded slice の planner では、全部を同じ意味で扱わない。
少なくとも次の 3 面に投影する。

1. **base call graph adjacency**
   - `kind == call`
   - `provenance == call_graph`
2. **boundary evidence**
   - callers / callees の向きを決める symbol 間 adjacency
3. **explanation support**
   - 後で witness や debug reason と結びつける raw edge 情報

G5-3 で重要なのは、
**slice selection の本体は base call graph adjacency に寄せる** という点である。
local_dfg / symbolic_propagation edge は planner の入力本体ではなく、実行後の enrichment 側に置く。

## 2.3 call graph view

`refs` からその場で作る derived view として、少なくとも次を持つ。

- `outgoing_calls_by_symbol`
- `incoming_calls_by_symbol`
- `cross_file_calls_by_file`
- `boundary_symbols_by_seed`

ここでいう call graph は新しい persistent artifact ではなく、
**`refs` を planner 用に見やすく整えた projection** でよい。

## 2.4 cache index (`SymbolIndex`)

`SymbolIndex` は planner で次の役割を持つ。

- symbol id → symbol file の確定
- file → contained symbols の参照
- boundary symbol が属する file の中にどの symbol 群がいるかを見る
- per-seed reason の attribution を安定させる

特に G5 では `SymbolIndex.by_file` が重要で、
**boundary file の中でどの symbol を completion 候補として見るか** の土台になる。

---

## 3. planner contract

G5 では path list を直接広げるのではなく、
少なくとも概念上次のような policy contract を置く。

```rust
struct PdgProjectSlicePlan {
    per_seed: Vec<SeedProjectSlicePlan>,
    union_cache_update_paths: Vec<String>,
    union_local_dfg_paths: Vec<String>,
    union_explanation_paths: Vec<String>,
}

struct SeedProjectSlicePlan {
    seed_symbol_id: String,
    cache_update_paths: Vec<String>,
    local_dfg_paths: Vec<String>,
    explanation_paths: Vec<String>,
    reasons_by_path: BTreeMap<String, Vec<PdgSliceReason>>,
    pruned_candidates: Vec<PrunedSliceCandidate>,
}

enum PdgSliceReason {
    SeedFile,
    ChangedFile,
    DirectCallerFile { via_symbol_id: String },
    DirectCalleeFile { via_symbol_id: String },
    BridgeCompletionFile { via_symbol_id: String, via_kind: BridgeKind },
    ModuleCompanionFile { via_path: String },
}

enum BridgeKind {
    WrapperReturn,
    BoundaryAliasContinuation,
    RequireRelativeChain,
}
```

ここで重要なのは型名よりも、次の 5 点である。

1. **per-seed plan が first-class** であること
2. **cache update / local DFG / explanation が分離**されていること
3. **reason が path 単位で残る**こと
4. **pruned candidate も観測可能**であること
5. **union 実行しても per-seed attribution を失わない**こと

---

## 4. 基本方針

## 4.1 Graph-first, seed-first

slice selection は import/path heuristic から始めない。
まず

- changed symbols / explicit seeds
- refs から作った base call graph adjacency
- SymbolIndex

で **symbol boundary** を決め、そのあと file へ落とす。

理由:

- G5-2 の 4 ケースはすべて「どの symbol boundary を 1〜2 段閉じるか」として説明した方が自然
- import 起点にすると broad namespace / unused import / require chain で scope が膨らみやすい
- bounded slice は「関係ありそうな file 全部」ではなく、**今の seed に必要な近傍だけ** を取るための policy だから

## 4.2 per-seed planning, union execution

planner はまず **seed ごとに独立した slice** を組む。

そのあと実行段階で必要なら

- union_cache_update_paths
- union_local_dfg_paths

へ折りたたんでよい。

この順にする理由は、G5-1 で整理した通り
**scope reason を失わないため** である。

## 4.3 call-graph tier と enrichment tier を分ける

planner が扱うのは基本的に **base call graph side** までに留める。

- selection policy: call_graph + SymbolIndex
- PDG build: selected Rust/Ruby files に local DFG を build
- propagation: 同じ slice 上で symbolic propagation を追加

これで `--with-pdg` と `--with-propagation` の違いを
**scope** ではなく **edge augmentation** に寄せて整理できる。

---

## 5. 採用する selection policy

G5 の bounded slice policy は、次の 4 tier を正式採用とする。

## 5.1 Tier 0: root files（changed / seed core）

### Rule

必ず含める。

- diff mode: changed files
- seed mode: explicit seed files

### Data source

- changed symbols
- seed symbols
- SymbolIndex（file 解決）

### Output

- `cache_update_paths`: 含める
- `local_dfg_paths`: `.rs` / `.rb` のみ含める
- `explanation_paths`: 常に含める
- reason: `changed_file` または `seed_file`

### Why

ここは最小核であり、bounded slice の外に出してはいけない。

## 5.2 Tier 1: direction-aware direct boundary files

### Rule

seed symbols を起点に base call graph adjacency を 1-hop だけ辿り、
隣接 symbol の所属 file を direct boundary file として選ぶ。

- `callers`: incoming call の `from` symbol file
- `callees`: outgoing call の `to` symbol file
- `both`: その union

### Data source

- `refs` のうち `kind == call && provenance == call_graph`
- SymbolIndex による symbol id → file 解決

### Output

- `cache_update_paths`: 含める
- `local_dfg_paths`: `.rs` / `.rb` のみ含める
- `explanation_paths`: 含める
- reason:
  - callers なら `direct_caller_file`
  - callees なら `direct_callee_file`

### Why

これは G4 direct-boundary の正式昇格版であり、
G5 でもまず効く最小拡張だから。

## 5.3 Tier 2: bridge completion file

### Rule

Tier 1 で選ばれた boundary file / boundary symbol を起点に、
**bridge を閉じるために必要な file を 1 回だけ** 追加する。

G5 前半では Tier 2 を再帰 closure にしない。
**per seed / per boundary side で 1 追加まで** に固定する。

### Detection classes

G5-2 の acceptance surface に合わせ、最初は次の 3 種類で十分。

#### A. `wrapper_return`

boundary file の symbol がさらに別 file symbol を直接 call していて、
その boundary file が seed 側の assigned result / wrapper return の中継点になっているケース。

典型:

- `main -> adapter -> core`

#### B. `boundary_alias_continuation`

boundary file を経由する imported result が、
seed file 側 alias chain / assigned-result chain に接続されるケース。

典型:

- `main -> adapter -> value`
- `y = adapter::wrap(x); alias = y; out = alias`

#### C. `require_relative_chain`

Ruby で direct boundary の `mid.rb` を越えた leaf file がないと、
return-ish flow / temp alias chain が閉じないケース。

典型:

- `app/main.rb -> lib/mid.rb -> lib/leaf.rb`

### Data source

- Tier 1 で選ばれた boundary symbols/files
- base call graph adjacency
- SymbolIndex.by_file
- 必要最小限の file-local context（symbol containment / same-file neighboring symbols）

### Output

- `cache_update_paths`: 含める
- `local_dfg_paths`: `.rs` / `.rb` のみ含める
- `explanation_paths`: 含める
- reason: `bridge_completion_file { via_symbol_id, via_kind }`

### Why

G5 の bounded slice が direct-boundary と違うのは、ここを explicit に扱う点にある。

## 5.4 Tier 3: companion fallback（module / require-relative）

### Rule

graph-first selection だけでは file 同定が痩せる時だけ、
**すでに選ばれた path に紐づく companion file** を fallback として足す。

対象は当面:

- Rust: `foo.rs` / `foo/mod.rs` / `mod.rs`
- Ruby: `require_relative` で直接決まる companion

### Constraint

- Tier 1 / Tier 2 で選ばれた path に紐づく時だけ許可
- import/path だけを起点に無制限に広げない
- budget 超過時は真っ先に落とす

### Output

- `cache_update_paths`: 必要時のみ含める
- `local_dfg_paths`: `.rs` / `.rb` かつ budget に余裕がある時のみ
- `explanation_paths`: 含めてよい
- reason: `module_companion_file`

---

## 6. Candidate scoring / ordering

G5 では単に集合を作るのでなく、
**候補の優先順位** を固定しておく方がよい。

## 6.1 phase ordering

1. Tier 0 を確定
2. Tier 1 を direct boundary として確定
3. Tier 2 候補を収集
4. Tier 2 を score して 1 つだけ選ぶ
5. 必要なら Tier 3 fallback を評価
6. budget prune

## 6.2 tier priority

優先順は固定する。

1. Tier 0
2. Tier 1
3. Tier 2
4. Tier 3

## 6.3 within-tier ordering

同 tier 内は少なくとも次で deterministic にする。

1. seed からの hop depth
2. reason priority
3. call-graph certainty priority
4. lexical path order

`certainty` はあくまで補助であり、
Tier を逆転させるほど強く使わない。

## 6.4 bridge completion priority

Tier 2 候補が複数ある時は、まず次で優先する。

1. `wrapper_return`
2. `boundary_alias_continuation`
3. `require_relative_chain`

理由:

- Rust wrapper-return は direct-boundary との差が最も説明しやすい
- alias continuation はその次に価値が高い
- require-relative companion は fallback 成分も強いので 3 位に置く

---

## 7. Budget / stop rules

bounded slice は bounded でなければ意味がない。
したがって planner contract には budget を正式に持たせる。

## 7.1 初期 budget

G5 の初期値は次で固定する。

- `per_seed_tier1_files_max`: 4
- `per_seed_tier2_files_max`: 1
- `per_seed_tier3_files_max`: 1
- `union_cache_update_paths_max`: 12
- `union_local_dfg_paths_max`: 8
- `union_explanation_paths_max`: 12

## 7.2 prune order

超過時は次の順で落とす。

1. Tier 3
2. Tier 2
3. Tier 1 の低優先候補
4. Tier 0 は最後まで保持

## 7.3 no recursive closure

Tier 2 の先をさらに Tier 2.5 / Tier 3 call graph closure のように広げない。

G5-3 の目的は **project-wide expansion ではなく、1-hop + 1 completion の policy 固定** だからである。

---

## 8. scope split

G5 planner では、選ばれた path を 3 種類に分けて持つ。

## 8.1 cache update scope

役割:

- base graph を最新化する file 集合

含める対象:

- Tier 0
- Tier 1
- 選ばれた Tier 2
- 必要時の Tier 3

性質:

- 広めでよい
- `local_dfg_scope` の superset であるべき

## 8.2 local DFG scope

役割:

- Rust/Ruby local DFG build に実際に渡す file 集合

含める対象:

- Tier 0 の `.rs` / `.rb`
- Tier 1 の `.rs` / `.rb`
- Tier 2 の `.rs` / `.rb`
- Tier 3 は budget 余裕時のみ

性質:

- より保守的
- `cache_update_scope` より小さくてよい

## 8.3 explanation scope

役割:

- reason / witness / debug 出力で「なぜ入ったか」を返したい path 集合

含める対象:

- 原則、選ばれた全 path
- prune された candidate は別表で保持してもよい

性質:

- 実行対象と完全一致しなくてもよい
- ただし per-seed attribution は必須

---

## 9. mode ごとの適用方針

## 9.1 diff mode

### root seed

- `changed.changed_symbols`

### Tier 0

- `changed.changed_files`

### Tier 1

- changed symbols から direct callers / callees を file に落とす

### Tier 2

- G5-2 の 4 ケースに対応する bridge-completion candidate を 1 回だけ許す

## 9.2 explicit seed mode

### root seed

- CLI seed symbols

### Tier 0

- seed files

### Tier 1

- seed symbols の incoming / outgoing call adjacency file

### Tier 2

- wrapper-return / alias continuation / Ruby split chain に必要な 1 追加だけを許す

## 9.3 per-seed mode

planner は必ず seed ごとに plan を作る。

必要なら実行時だけ union するが、
`reasons_by_path` と `pruned_candidates` は seed ごとに保持する。

---

## 10. G5-2 fixed set への対応づけ

G5-3 policy が G5-2 の 4 ケースにどう対応するかを固定しておく。

## 10.1 `rust-three-file-wrapper-return-completion`

- Tier 0: `main.rs`
- Tier 1: `adapter.rs`
- Tier 2: `core.rs`
- bridge kind: `wrapper_return`

## 10.2 `rust-three-file-imported-result-alias-continuation`

- Tier 0: `main.rs`
- Tier 1: `adapter.rs`
- Tier 2: `value.rs`
- bridge kind: `boundary_alias_continuation`

## 10.3 `ruby-three-file-require-relative-alias-return-chain`

- Tier 0: changed file / root caller file
- Tier 1: `mid.rb`
- Tier 2: `leaf.rb`
- bridge kind: `require_relative_chain`

## 10.4 `ruby-three-file-dynamic-send-target-separation`

- Tier 0: dispatch / router side file
- Tier 1: adjacent caller or callee boundary
- Tier 2: `targets.rb`（必要なら）
- fallback: companion path resolution may assist, but must not replace graph-first selection

---

## 11. 実装挿入点

G5-3 は policy taskなので、実装そのものではなく挿入点を固定する。

## 11.1 置き換える対象

現状の `src/bin/dimpact.rs` では

- `expand_related_local_dfg_paths()`
- `build_pdg_context()`

が path union を担っている。

G5-4 ではその前に、少なくとも概念上次の pure function を置くべき。

```rust
fn plan_bounded_project_slice(...) -> PdgProjectSlicePlan
```

## 11.2 実行順

1. changed symbols / seeds を確定
2. cache load (`SymbolIndex`, `refs`)
3. call graph projection を作る
4. `plan_bounded_project_slice(...)`
5. union_cache_update_paths を cache update へ渡す
6. union_local_dfg_paths だけ local DFG build に渡す
7. その上で PDG / propagation を実行する

## 11.3 初期テスト面

G5-4 以降で必要になる最低限のテスト面は次。

- planner unit tests
  - seed / refs / index から期待 path set が出るか
- deterministic budget tests
  - prune order が固定か
- G5-2 fixture regressions
  - Tier 2 completion が必要なケースで正しい file を選べるか
- per-seed attribution tests
  - union 実行しても reasons が seed 単位で残るか

---

## 12. 固定しておきたい判断

### 判断 1

**selection policy の一次入力は `changed symbols + refs + projected call graph + SymbolIndex` に置く。**
path heuristic は一次入力にしない。

### 判断 2

**Tier 1 は direct boundary、Tier 2 は bridge completion 1 回だけ。**
bounded slice はここで止める。

### 判断 3

**`--with-pdg` / `--with-propagation` は同じ slice planner を共有する。**
propagation は slice の上で bridge を足すだけにする。

### 判断 4

**planner は per-seed に組み、実行時だけ union してよい。**
reason と prune 情報は per-seed のまま残す。

### 判断 5

**cache update / local DFG / explanation scope は plan object で分離する。**
G5 の bounded slice は path list の ad-hoc union ではない。

---

## 13. 一言まとめ

G5-3 の bounded slice policy は、次の一文に尽きる。

**changed symbols を root seed に、refs から projected call graph を作り、SymbolIndex で symbol/file を安定化したうえで、direct boundary を 1-hop、bridge completion を 1 回だけ選ぶ per-seed / reason-aware / budgeted plan として scope を決める。**

この policy なら

- G5-2 の 2-hop+ fixed set にそのまま対応できる
- `--with-pdg` と `--with-propagation` の scope 層を共有できる
- cache update / local DFG / explanation を分離できる
- G5-4 の最小実装に無理なく落とせる

ので、G5 の bounded project-slice policy として十分実用的である。
