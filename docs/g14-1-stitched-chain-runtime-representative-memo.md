# G14-1 stitched-chain runtime representative memo

## Goal

Define the minimal runtime representative that sits between the current file-local stitched candidate search and the final witness/output layer.

This memo fixes four things explicitly:

- candidate shape
- duplicate key
- budget key
- explanation fields

## Why this layer is needed

G13 established that ranking, duplicate suppression, and boundedness should be driven by a **stitched chain representative**, not by raw file candidates.

Today the planner still compares mostly file-local candidates directly. That makes these cases harder than they need to be:

- same closure described by different wrapper/helper paths
- return-like and alias-like candidates competing for the same closure slot
- mixed/nested explanations overstating support steps as if they were the winner
- file caps accidentally acting as chain caps

The runtime representative should be the smallest object that can answer:

1. what closure this candidate claims to reach
2. whether two candidates are the same chain for suppression purposes
3. what budget bucket this candidate consumes
4. how the winner should be explained

## 1. Candidate shape

### 1.1 Runtime struct

```rust
struct StitchedChainRepresentative {
    seed_symbol_id: String,
    entry_boundary_symbol_id: String,
    anchor_symbol_id: String,

    family: StitchedChainFamily,
    family_bucket: StitchedChainFamilyBucket,
    step_families: Vec<StitchedStepFamily>,

    terminal_symbol_id: Option<String>,
    terminal_path: Option<String>,

    caller_result_symbol_id: Option<String>,
    nested_continuation_symbol_id: Option<String>,

    reaches_caller_result: bool,
    reaches_nested_continuation: bool,
    has_require_relative_load: bool,

    closure_target_key: String,
    anchor_locality_key: String,
    duplicate_key: String,
    budget_key: String,

    winning_bridge_execution_chain_compact: Vec<BridgeExecutionStepCompact>,
    observed_supporting_steps_compact: Vec<BridgeExecutionStepCompact>,

    negative_chain_signals: Vec<String>,
    explanation: StitchedChainExplanation,
}
```

### 1.2 Required field meanings

- `seed_symbol_id`: original seed that started the continuation search
- `entry_boundary_symbol_id`: first boundary symbol that admitted the stitched continuation
- `anchor_symbol_id`: selected continuation anchor that this representative is built around
- `family`: concrete runtime family label, for example `return`, `alias_result`, `mixed`, `nested`
- `family_bucket`: normalized budget/suppression bucket, see below
- `step_families`: ordered set of families actually used by the winning compact chain
- `terminal_symbol_id` / `terminal_path`: where the representative ends if there is no stronger closure target symbol
- `caller_result_symbol_id`: caller-side closure target when the chain really closes on a caller result
- `nested_continuation_symbol_id`: nested continuation target when that is the real closure target
- `reaches_caller_result`: true only when the selected winning chain actually closes to caller result
- `reaches_nested_continuation`: true only when the selected winning chain actually closes through the nested continuation
- `has_require_relative_load`: support signal, not winner proof by itself
- `closure_target_key`: canonical closure target identity used by both duplicate and budget logic
- `anchor_locality_key`: canonical locality identity for the selected anchor
- `winning_bridge_execution_chain_compact`: only the steps that define the winner
- `observed_supporting_steps_compact`: extra observed steps that support the explanation but did not define the winner
- `negative_chain_signals`: penalties/noise markers such as helper-only stitch, duplicate wrapper path, weak mixed labeling

### 1.3 Canonical family buckets

`family` stays expressive, but `family_bucket` is normalized for suppression/budget.

```text
return_result
alias_result
mixed_result
nested_continuation
```

Rules:

- `return` -> `return_result`
- `alias_result` -> `alias_result`
- `mixed` -> `mixed_result`
- `nested` -> `nested_continuation`

Do **not** collapse `return_result` and `alias_result` into the same bucket.
That was the main source of file-cap behaving like chain-cap.

## 2. Duplicate key

### 2.1 Shape

```rust
struct StitchedChainDuplicateKey {
    seed_symbol_id: String,
    entry_boundary_symbol_id: String,
    family_bucket: StitchedChainFamilyBucket,
    closure_target_key: String,
    anchor_locality_key: String,
}
```

Serialized runtime key:

```text
{seed_symbol_id}|{entry_boundary_symbol_id}|{family_bucket}|{closure_target_key}|{anchor_locality_key}
```

### 2.2 Normalization rules

`closure_target_key` should be chosen in this priority order:

1. `caller_result_symbol_id`
2. `nested_continuation_symbol_id`
3. `terminal_symbol_id`
4. `terminal_path`

`anchor_locality_key` should be:

1. anchor symbol id when available
2. otherwise `file:start_line:end_line`

### 2.3 Intended effect

Two candidates are treated as `weaker_same_chain_duplicate` when they differ only in:

- wrapper/helper path detail
- support-step verbosity
- file-local observation path
- mixed vs support-only narration around the same closure target

They should **not** be treated as duplicates when they differ in:

- seed
- entry boundary
- normalized family bucket
- actual closure target
- selected anchor locality

## 3. Budget key

### 3.1 Shape

```rust
struct StitchedChainBudgetKey {
    seed_symbol_id: String,
    entry_boundary_symbol_id: String,
    family_bucket: StitchedChainFamilyBucket,
    closure_target_key: String,
}
```

Serialized runtime key:

```text
{seed_symbol_id}|{entry_boundary_symbol_id}|{family_bucket}|{closure_target_key}
```

### 3.2 Budget interpretation

The runtime should count budget at the representative level, not at raw file-candidate level.

Minimum policy:

- keep existing global bounded caps
- allow at most one retained representative per `budget_key`
- compare candidates sharing the same `budget_key` by winner ranking
- let `return_result` and `alias_result` consume different family buckets

This keeps boundedness explicit while stopping file count from silently deciding chain diversity.

## 4. Explanation fields

### 4.1 Shape

```rust
struct StitchedChainExplanation {
    winner_reason_codes: Vec<String>,
    loser_reason_codes: Vec<String>,

    closure_summary: String,
    family_summary: String,
    budget_summary: String,
    duplicate_summary: Option<String>,

    selected_anchor_symbol_id: String,
    closure_target_key: String,
    family_bucket: StitchedChainFamilyBucket,

    winning_bridge_execution_chain_compact: Vec<BridgeExecutionStepCompact>,
    observed_supporting_steps_compact: Vec<BridgeExecutionStepCompact>,
    negative_chain_signals: Vec<String>,
}
```

### 4.2 Required explanation fields

The winner explanation must make these distinctions explicit:

- **what actually won**
  - `winning_bridge_execution_chain_compact`
- **what was only supporting evidence**
  - `observed_supporting_steps_compact`
- **what closure target was reached**
  - `closure_target_key`
- **which family bucket this consumed**
  - `family_bucket`
- **why weaker alternatives lost**
  - `loser_reason_codes`
- **which negative signals were present but tolerated**
  - `negative_chain_signals`

### 4.3 Reason code vocabulary

Recommended initial winner codes:

- `closure_reaches_caller_result`
- `closure_reaches_nested_continuation`
- `selected_anchor_contributes_to_closure`
- `mixed_chain_has_required_load_support`
- `stronger_family_fit`
- `stronger_local_evidence`

Recommended initial loser codes:

- `weaker_same_chain_duplicate`
- `over_budget_same_family_bucket`
- `weaker_closure_target`
- `anchor_did_not_contribute_to_closure`
- `support_only_require_relative`
- `helper_only_stitch`
- `weaker_mixed_label`

## 5. Compact runtime example

```json
{
  "seed_symbol_id": "ruby:app/service.rb:method:run:12",
  "entry_boundary_symbol_id": "ruby:app/service.rb:method:dispatch:24",
  "anchor_symbol_id": "ruby:app/loader.rb:method:resolve:8",
  "family": "mixed",
  "family_bucket": "mixed_result",
  "step_families": ["alias_result", "mixed"],
  "caller_result_symbol_id": "ruby:app/service.rb:method:run:12",
  "reaches_caller_result": true,
  "reaches_nested_continuation": false,
  "has_require_relative_load": true,
  "closure_target_key": "ruby:app/service.rb:method:run:12",
  "anchor_locality_key": "ruby:app/loader.rb:method:resolve:8",
  "duplicate_key": "ruby:app/service.rb:method:run:12|ruby:app/service.rb:method:dispatch:24|mixed_result|ruby:app/service.rb:method:run:12|ruby:app/loader.rb:method:resolve:8",
  "budget_key": "ruby:app/service.rb:method:run:12|ruby:app/service.rb:method:dispatch:24|mixed_result|ruby:app/service.rb:method:run:12"
}
```

## 6. Decisions

- introduce one runtime representative per stitched winner candidate
- make `duplicate_key` stricter than budget only by adding `anchor_locality_key`
- keep family expressiveness in `family`, but normalize comparisons through `family_bucket`
- separate winner chain from supporting observed steps in explanation/provenance
- treat `require_relative` as support, never as sole closure proof

## 7. Non-goals

This memo does not define:

- the final ranking formula
- graph-isomorphism-level duplicate detection
- project-wide recursive chain enumeration
- output schema changes beyond the explanation/runtime mapping needed here

## 8. Implementation note

The intended insertion point is after current stitched candidate reconstruction and before final winner selection / witness emission.

At runtime:

1. reconstruct candidate
2. normalize into `StitchedChainRepresentative`
3. compute `duplicate_key`
4. compute `budget_key`
5. rank within `budget_key`
6. emit explanation from the retained representative
