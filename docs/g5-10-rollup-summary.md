# G5 rollup: bounded project-slice planning, explainable witnesses, and a cleaner PDG baseline

## What landed

This document closes out the G5 work around `impact --with-pdg` / `--with-propagation`.

G5 was not about turning dimpact into a whole-project PDG engine.
The narrower goal was more practical:

**replace the old direct-boundary-only expansion with a bounded project-slice footing, improve one real 2-hop multi-file bridge, make witnesses easier to explain, lock the new behavior with regressions, and remove one more engine-consistency mismatch.**

In practice, G5 landed in four layers:

- first, define the bounded-slice problem, the fixed evaluation surface, and the selection policy
- then, ship a minimal bounded slice builder and use that shared scope in both `--with-pdg` and `--with-propagation`
- next, improve one real 3-file bridge case, stabilize witness selection/compression, and expand multi-file regressions
- finally, tighten the PDG diff-mode engine contract and update the public docs to describe the current bounded-slice model plainly

## Merged PRs

- `#466` docs: add G5 bounded project-slice design memo
- `#467` docs: add G5 bounded slice eval set
- `#468` docs: define G5 bounded slice policy
- `#469` feat: add bounded slice PDG scope builder
- `#470` feat: bridge imported result alias chains
- `#471` feat: rank and compact witness paths
- `#472` test: expand multi-file PDG propagation regressions
- `#473` fix: honor selected engine in PDG diff mode
- `#474` docs: clarify bounded slice guidance

---

## What changed in practice

## 1. G5 turned “direct boundary” into a named bounded-slice model

G4 made the remaining weakness visible.
G5 made the replacement model explicit.

The important design shift was:

- stop treating the next step as “just widen PDG a bit more”
- define a **bounded project slice** instead
- keep the scope intentionally small and deterministic:
  - root changed/seed files
  - direct boundary files
  - at most one bridge-completion file
  - optional small companion fallback only when needed

Relevant docs:

- `docs/g5-1-bounded-project-slice-design-memo.md`
- `docs/g5-2-bounded-project-slice-eval-set.md`
- `docs/g5-3-bounded-project-slice-policy.md`
- `docs/g5-3-bounded-project-slice-policy.json`

This matters because G5 stopped framing the next target as project-wide PDG and instead fixed the vocabulary around a smaller, testable, budgeted scope model.

## 2. There is now a G5-specific 2-hop+ evaluation surface

G4’s multi-file set still centered on direct-boundary improvements.
G5 added a new fixed set for cases that need more than one hop of context.

The locked G5 surfaces are:

- Rust three-file wrapper return completion
- Rust three-file imported-result alias continuation
- Ruby three-file `require_relative` alias / return chain
- Ruby three-file dynamic-send target separation guard

Relevant docs:

- `docs/g5-2-bounded-project-slice-eval-set.md`
- `docs/g5-2-bounded-project-slice-eval-set.json`

This gave G5 a stable way to ask three different questions separately:

- is the selected scope still too small?
- did propagation recover the bridge after scope grew?
- did a larger slice start smearing precision?

## 3. `--with-pdg` and `--with-propagation` now share a bounded slice builder

The central code change in G5 was `#469`.

Instead of only widening propagation scope ad hoc, the implementation now builds a minimal bounded slice in `src/bin/dimpact.rs` and uses it as the PDG build scope for both:

- `--with-pdg`
- `--with-propagation`

The current implementation is intentionally small:

- Tier 0: root changed/seed files
- Tier 1: direct boundary files from call-graph refs
- Tier 2: one bridge-completion file per seed
- local DFG scope stays limited to Rust/Ruby files

This does **not** give users project-wide PDG.
It does fix a long-standing asymmetry:

- before G5, `--with-propagation` was the only path that really widened scope
- after G5, plain PDG can also include the extra third file when that file belongs to the selected bounded slice

Locked regressions include the three-file wrapper case, where plain PDG now keeps the third-file leaf/core-side DFG nodes in scope even when it still does not synthesize propagation-only bridges.

Relevant coverage:

- `tests/cli_pdg_propagation.rs::pdg_propagation_maps_multi_file_wrapper_return_without_leaking_irrelevant_arg`
- planner tests in `src/bin/dimpact.rs`

## 4. One real 3-file bridge now lands end-to-end

G5 did not only widen scope.
It also used that scope to improve one real bridge surface.

`#470` improved the Rust imported-result alias case:

- `main -> adapter -> value`
- imported result then continues into `y -> alias -> out`

The added behavior is still deliberately bounded:

- one-hop nested summary completion
- focused on the boundary-side imported-result continuation
- no recursive closure

This matters because it moves one of the G5-2 target cases out of the “designed but not actually recovered” bucket.

Relevant coverage:

- `tests/cli_pdg_propagation.rs::pdg_propagation_extends_imported_result_into_caller_alias_chain`

## 5. Witness selection is now ranked and compressed

G5 also improved explainability.

Before G5, witness selection was mainly “first BFS path wins”.
That was serviceable but unstable in equal-depth tie cases and unnecessarily noisy for user-visible output.

`#471` changed this in two ways:

- equal-depth witnesses now use a deterministic ranking
  - shortest depth stays primary
  - ties prefer more explainable paths
- witness output now has compressed explanation fields:
  - `path_compact`
  - `provenance_chain_compact`
  - `kind_chain_compact`

This is still intentionally conservative:

- one selected shortest-path explanation, not all candidates
- compressed route summary, not a synthetic proof graph

But it makes multi-file output materially easier to read, especially in `--per-seed` JSON.

Relevant code / coverage:

- `src/impact.rs`
- unit tests for ranking and compaction
- CLI JSON assertions in `tests/cli_pdg_propagation.rs`

## 6. Multi-file regression coverage now matches the bounded-slice model better

`#472` expanded the regression surface so G5 behavior is locked more directly.

The suite now covers, in JSON/per-seed form as well as graph-shape form:

- three-file Rust wrapper witness shape under plain PDG
- three-file imported-result alias witness shape under propagation
- Ruby `require_relative` witness shape with compact witness fields
- multi-file witness compaction remaining stable in grouped output

This matters because the guardrails are no longer only “an extra edge appeared somewhere”.
They now also lock:

- which file stayed in scope
- which bridge stayed propagation-only
- which witness route was chosen
- how the compact witness summary looks in per-seed output

## 7. One more PDG diff-mode / selected-engine mismatch is gone

G4 fixed changed-symbol discovery in PDG diff mode.
G5 removed one more mismatch in `#473`.

Before G5-8, PDG/progation diff mode could still skip part of the selected engine’s impact contract after changed-symbol discovery succeeded.
That meant strict mock-LSP / strict LSP capability failures were not fully aligned with plain impact.

After `#473`, PDG diff mode now validates the selected engine’s impact capability before falling into cache/PDG construction.

This does **not** fully unify the PDG path with engine-native edge behavior.
It does close another visible semantic gap:

- strict impact-capability failures now surface consistently on plain impact and PDG diff mode

Relevant coverage:

- `src/bin/dimpact.rs::pdg_diff_validation_honors_strict_lsp_impact_capabilities`
- `src/bin/dimpact.rs::pdg_diff_validation_baseline_matches_selected_engine_when_caps_exist`

## 8. The README now describes the actual bounded-slice model

`#474` updated `README.md` and `README_ja.md` so the public docs now say plainly that:

- the PDG path uses a bounded slice model
- `--with-pdg` and `--with-propagation` share that scope layer
- the scope is still bounded, not project-wide
- compact witness fields exist and are intended for explanation
- diff-mode PDG now honors both selected-engine changed-symbol discovery and strict impact-capability validation

That matters because G5 improved the implementation enough that users need an accurate mental model, but not so much that “whole-project PDG” would be an honest description.

---

## Improvement cases G5 materially helped

The clearest practical wins from G5 are:

### 1. Plain PDG can now carry a bounded three-file scope

Improved from:

- `--with-pdg` staying close to the root file
- third-file bridge-completion scope mostly being propagation-side only

To:

- plain PDG and propagation sharing the same bounded slice selection
- third-file wrapper/leaf/core nodes now appearing in PDG scope when they belong to the bounded slice

### 2. Rust imported-result alias continuation now has a real improved case

Improved from:

- imported-result alias continuation being mainly a planned evaluation surface

To:

- one actual Rust three-file case where the bridge is recovered through `adapter -> value` and the caller-side alias chain remains connected

### 3. Multi-file witness output is more stable and easier to read

Improved from:

- last-hop-heavy witness output with first-seen BFS tie behavior

To:

- deterministic shortest-path witness ranking
- compact witness path / provenance / kind summaries in both normal and per-seed output

### 4. Engine-consistency in PDG diff mode is tighter than G4

Improved from:

- changed-symbol discovery honoring the selected engine but later impact validation still drifting

To:

- selected-engine impact capability checks also being honored before PDG diff-mode construction proceeds

### 5. Public guidance now matches the bounded-slice implementation

Improved from:

- README still describing mostly the G4-era mental model

To:

- README / README_ja explaining bounded slice goals, limits, practical use, and compact witness behavior directly

---

## What G5 did **not** solve

G5 was useful, but it left several clear boundaries in place.

## 1. The planner is still minimal and mostly internal

G5 shipped a bounded slice builder, but not a full public/debuggable planner object with:

- per-seed reasons surfaced in CLI output
- explicit prune diagnostics
- explicit cache-update vs explanation-scope reporting

The design docs describe that target more richly than the current runtime output does.

## 2. Bridge completion is still intentionally shallow

The current behavior is still roughly:

- direct boundary
- plus one bridge-completion file

That is enough for short 2-hop-style bridges.
It is not enough for:

- recursive adapter chains
- broader serializer / service / wrapper stacks
- longer return/alias continuation ladders

## 3. Only one priority improvement case was truly “upgraded” end-to-end

G5 improved the Rust imported-result alias case for real.
But other G5-2 target cases are still in different states:

- wrapper three-file scope is materially better
- Ruby split-chain surfaces are better documented and better covered
- Ruby dynamic-send remains primarily a guard case

So the evaluation surface is ahead of the implementation on some Ruby-side bridge-completion goals.

## 4. Engine consistency is better, not complete

G5 removed one more semantic mismatch, but the PDG path still layers:

- cached graph data
- local DFG construction
- symbolic bridge augmentation

on top of the selected engine rather than preserving engine-native semantics end to end.

## 5. Bounded slice still has no user-visible “why this file?” output

The docs and policy note now talk in terms of:

- root files
- direct boundary
- bridge completion
- budget

But the CLI does not yet expose a first-class per-path reason surface that says:

- this file entered as direct boundary
- this file entered as bridge completion
- this candidate was pruned by budget

That remains a major explainability gap between design and runtime output.

---

## G6 candidates

If G6 continues from G5, the next good steps are:

## 1. Surface slice reasons in JSON/debug output

The most obvious next step is to expose the planner’s decisions directly:

- selected paths
- reason per path
- per-seed attribution
- pruned candidates / budget drops

That would close the gap between the bounded-slice policy docs and what users can actually inspect.

## 2. Improve one Ruby 3-file bridge-completion case for real

G5’s strongest real upgrade landed on the Rust imported-result alias case.
A good G6 target is to do the same for one Ruby three-file chain, especially:

- `require_relative` split alias / return-flow

That would move Ruby from “covered and guarded” toward “materially improved”.

## 3. Refine bridge-completion scoring instead of only bounding it

G5 bounded Tier 2 selection.
G6 could make it smarter by ranking completion candidates more explicitly for:

- wrapper-return
- boundary-side alias continuation
- require-relative chain completion

without abandoning the bounded model.

## 4. Separate cache-update scope, local-DFG scope, and explanation scope more explicitly in code

The policy docs already want this separation.
G6 could make the runtime structures line up more closely with that contract.

## 5. Expand engine-consistency baselines beyond strict impact validation

G5-8 fixed one important semantic mismatch.
G6 could compare:

- selected-engine changed symbols
- selected-engine impact path
- PDG/progation path on top of cache + local augmentation

more systematically, especially on Rust/Ruby bounded-slice fixtures.

## 6. Decide whether compact witness output should remain “one route only” or grow into multi-candidate explainability

G5 made witness output more stable.
G6 needs to decide whether that is enough, or whether advanced debug/reporting should surface:

- alternate equal-depth candidates
- path-reason overlays
- compressed path plus selected-slice reasons together

---

## One-line summary

G5 replaced the old direct-boundary-only mental model with a real bounded project-slice footing, used that footing to improve one real 3-file Rust bridge, made witness output more stable and readable, locked the new behavior with multi-file regressions, and removed one more PDG diff-mode engine mismatch — while still deliberately stopping short of project-wide PDG or recursive slice closure.
