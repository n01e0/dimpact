# G14 rollup: runtime stitched-chain representatives, representative-based witness explanation

## Summary

G14 finished the shift from file-first stitched continuation bookkeeping to a small runtime representative that drives compare, duplicate suppression, budget, and explanation.

The implementation landed in reviewable steps:

- `#604` add the representative projection scaffold
- `#605` wire representative projection into ranking
- `#606` suppress same-chain duplicates by representative key
- `#607` add representative family and per-chain budget controls
- this follow-up threads representative explanation into witness output and locks the remaining mixed-family / duplicate-locality regressions

## What changed

### 1. The planner now keeps a chain-shaped explanation next to selected and pruned slice entries

Selected `slice_selection.files[*].reasons[*]` and `slice_selection.pruned_candidates[*]` can now carry compact representative explanation metadata:

- closure target key
- family bucket
- duplicate key
- budget key
- winner / loser reason codes
- compact winning chain steps
- compact supporting steps
- negative chain signals

This keeps the output surface aligned with the runtime object that actually won, instead of reconstructing explanation only from raw file reason fields later.

### 2. Selected-vs-pruned matching can stay on the winning representative even when locality differs

The old witness matching path depended mostly on file-local reason identity such as `via_symbol_id`, `via_path`, and bridge kind.
That was too brittle for same-chain duplicates that described the same closure from different localities.

Now witness selected-vs-pruned reasoning can fall back to representative metadata, so a loser that differs in locality can still be explained as a weaker duplicate of the same winning chain.

### 3. Winning-chain provenance now prefers the representative winner over step-union reconstruction

`winning_bridge_execution_chain_compact` and `observed_supporting_steps_compact` now prefer the selected representative explanation when it exists.
This fixes the main Ruby mixed-family overstatement from G13:

- the winning chain stays focused on the actual alias/return representative
- `require_relative` support remains visible as support-only context
- `bridge_execution_family` follows the representative winner instead of being inflated by every observed step on the path

## Locked regressions

The follow-up adds explicit regressions for the last two G14 failure families:

- `selected_vs_pruned_prefers_representative_explanation_for_same_chain_duplicate`
  - duplicate-locality case
  - verifies same-chain losers still match the selected winner when the raw file-local reason fields differ
- `build_bridge_execution_provenance_compact_prefers_representative_winning_chain`
  - mixed-family case
  - verifies winning-chain output stays representative-first while supporting require-relative provenance stays separate

## Why G14 matters

Before G14, bounded planning could already find useful stitched files, but the runtime still mixed together:

- file candidate identity
- chain identity
- family budget
- witness explanation

That made some outcomes correct but hard to explain, and some explanations overstated mixed/support-only steps as if they had actually won.

After G14:

- compare happens on representative-aware keys
- duplicate suppression is chain-aware
- budget is family-aware
- witness explanation is winner-first

The bounded planner stays small, but the reported winner is now much closer to the real stitched chain the runtime selected.

## Remaining limits

G14 does not try to enumerate every possible stitched chain or replace the existing bounded file caps.
It keeps the current bounded planner and makes the representative path authoritative for the chain decisions that already exist.
