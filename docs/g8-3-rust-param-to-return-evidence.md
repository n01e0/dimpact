# G8-3: Rust param-to-return evidence メモ

対象: bounded slice planner の Tier 2 bridge scoring（Rust）

## やったこと

G8-3 では Rust 側で `param_to_return_flow` の evidence 収集を最小実装した。

具体的には、Tier 2 candidate が Rust symbol のときだけ file-local DFG と function summary を使って、
**関数の param が末尾の return / tail expression まで届いているか** を観測する。

観測できた candidate には次を付ける。

- primary evidence: `param_to_return_flow`
- support metadata: `{ "local_dfg_support": true }`

また、この semantic evidence がある candidate は、
`call_line == side_max_call_line` に依存せず `return_continuation` として扱えるようにした。

## before

G7 / G8-2 時点では、neutral name/path の Rust leaf は lexical hint が薄いと

- 早い callsite にある neutral leaf
- 遅い callsite にある neutral helper

の競合で、**後ろにある helper 側が wrapper_return として勝つ** ケースが残っていた。

理由は、`tier2_scoring_summary()` が主に

- name/path hint
- last call position

で return continuation を推定していたため。

## after

`param_to_return_flow` が取れた neutral leaf は

- `lane = return_continuation`
- `primary_evidence_kinds = [assigned_result, param_to_return_flow, return_flow]`
- `support.local_dfg_support = true`

を持てるようになり、
**later helper より primary evidence count で勝てる** ようになった。

## 固定した regression

新しい fixture では次を固定した。

- selected: `step.rs`
- pruned: `later.rs`
- selected_better_by: `primary_evidence_count`

これは「last call が勝つ」ではなく、
**param continuity を local DFG で観測できた candidate が勝つ** ことを示す最小ケースである。

## non-goal

この段階では、Rust Tier 2 全体を local DFG ベースへ置き換えてはいない。

- `assigned_result`
- `alias_chain`
- `return_flow`

の全面 semantic 化は後続 task で続ける。
