# G6 rollup: explainable bounded slices, controlled 2-hop selection, and stronger Rust/Ruby multi-file paths

## What landed

This document closes out the G6 work around `impact --with-pdg` / `--with-propagation`.

G6 was not about making dimpact project-wide.
It was about taking the bounded-slice footing from G5 and making it more explainable, a bit stronger, and easier to reason about in actual output.

The practical goal was:

**keep the PDG / propagation path bounded, expose why files were selected, extend the planner from shallow one-extra-file behavior into a controlled 2-hop model, improve one real Rust case and one real Ruby case, and connect witness output back to slice-selection context.**

In practice, G6 landed in five layers:

- first, inventory what the old planner still hid and define the missing schema/policy surface
- then, expose file-level slice-selection metadata in CLI output
- next, upgrade the planner to a controlled 2-hop selection model with bounded budgets
- then, use that model to fix one real Rust false negative and one real Ruby multi-file weakness
- finally, connect witness output back to slice-selection context and document the new mental model in the READMEs

## Merged PRs

- `#476` docs: add G6 bounded slice reasoning memo
- `#477` docs: define G6 planner reason schema
- `#478` feat: expose bounded slice selection reasons
- `#479` docs: define G6 controlled 2-hop policy
- `#480` feat: add controlled 2-hop slice selection
- `#481` fix: rank Rust 2-hop wrapper completions by evidence
- `#482` fix: improve Ruby no-paren propagation
- `#483` feat: link witnesses to slice selection context
- `#484` docs: clarify bounded slice reasoning in readmes

---

## What changed in practice

## 1. Bounded slice is no longer a mostly hidden planner

G5 had already introduced the bounded-slice footing, but most of the planner remained internal.
G6 made the key planning decisions visible.

The important shift was:

- stop treating bounded slice as just an implementation detail behind PDG/propagation scope growth
- expose file-level slice-selection metadata directly in output
- make selected vs pruned planner decisions testable and reviewable

The public output now has a distinct file-level planner surface:

- `summary.slice_selection.files[*]`
  - selected file paths
  - scope split (`cache_update` / `local_dfg` / `explanation`)
  - per-seed `reasons[*]`
- `summary.slice_selection.pruned_candidates[*]`
  - ranked-out / budget-pruned alternatives when the planner had to discard candidates

Relevant docs:

- `docs/g6-1-bounded-slice-reasoning-design-memo.md`
- `docs/g6-2-planner-reason-file-metadata-schema.md`
- `docs/g6-2-planner-reason-file-metadata-schema.json`

This matters because G6 gave the PDG/propagation path an explicit answer to a question G5 could not really answer:

**why was this file in scope at all?**

## 2. The planner moved from “one extra file” toward controlled 2-hop selection

G5’s effective Tier 2 behavior was still close to:

- direct boundary
- plus one bridge-completion file per seed

That was useful, but shallow.
G6 replaced that with a more explicit controlled 2-hop model.

The current practical shape is now closer to:

- root changed/seed files
- direct boundary files
- bounded second-hop bridge files selected per boundary side under small budgets

The associated policy is now written down rather than implied:

- boundary-side-local Tier 2 selection
- bounded per-seed / per-side budgets
- bridge-kind-aware vocabulary (`wrapper_return`, `boundary_alias_continuation`, `require_relative_chain`)
- minimal prune diagnostics
- narrow companion-style fallback kept deliberately small

Relevant docs:

- `docs/g6-4-controlled-2hop-policy.md`
- `docs/g6-4-controlled-2hop-policy.json`

This is still intentionally bounded.
G6 did **not** turn dimpact into a recursive whole-project PDG.
It did make the 2-hop boundary more deliberate and more explainable.

## 3. The selected-file reason contract is now fixture-backed

G6 did not stop at schema docs.
`#478` actually wired slice-selection metadata into JSON/YAML output and locked it in CLI regressions.

That means the project now has real fixture-backed expectations for:

- which files were selected into the bounded slice
- why they were selected
- which seed they belonged to
- what kind of bridge/completion they represented
- which alternatives were pruned

This is especially important for `--per-seed` mode, because the planner output now matches the grouped-output mental model much better.

Before G6, `--per-seed` could tell you which symbol spread where, but not which file-level scope decisions were made for that seed.
After G6, that attribution survives into `summary.slice_selection`.

## 4. One real Rust same-side 2-hop false negative was fixed

G6 used the new controlled-2-hop footing to remove one real Rust weakness.

The fixed case was a same-side wrapper-return situation where a lexically earlier helper/noise file could win the Tier 2 competition over the return-relevant leaf file.

In practice, the improvement was:

- same-side Tier 2 candidates are no longer only ordered by simple path tie-breaking
- call-site evidence is now used so the return-relevant completion can beat earlier noise
- the rejected helper candidate still survives as `ranked_out` planner metadata instead of vanishing silently

Relevant coverage:

- planner test for preferring the later wrapper-return candidate
- `tests/cli_pdg_propagation.rs::pdg_slice_selection_prefers_wrapper_return_leaf_over_earlier_noise_candidate`

This matters because it proves the controlled 2-hop layer is not only broader than G5, but also better at choosing the **right** extra file when multiple same-side candidates exist.

## 5. One real Ruby multi-file propagation weakness was improved

G6 also shipped a Ruby-side improvement instead of stopping at Rust-only progress.

The landed change was modest but real:

- Ruby DFG handling now recognizes no-paren method parameter lists well enough for a short multi-file wrapper / `require_relative` / return-flow path to recover parameter flow that used to disappear

That improves one of the specific weak surfaces G6 called out:

- short Ruby alias / return-flow / wrapper continuation chains

while keeping existing guardrails intact around dynamic-send/public-send target separation.

Relevant coverage:

- `src/dfg.rs` unit coverage for Ruby no-paren param flow
- `tests/cli_pdg_propagation.rs::ruby_require_relative_no_paren_wrapper_recovers_caller_arg_and_callee_param_scope`
- existing Ruby guard regressions remaining green

This matters because G6 did not only make the planner more visible; it also paid down one genuine Ruby multi-file false-negative area.

## 6. Witnesses and slice-selection context are now lightly connected

G5 made witness output more readable.
G6 made it more connected.

The new witness-side context is intentionally small:

- `impacted_witnesses[*].slice_context.seed_symbol_id`
- `impacted_witnesses[*].slice_context.selected_files_on_path[*]`
  - `path`
  - `witness_hops`
  - `selection_reasons`
  - `seed_reasons`

This does **not** turn witnesses into a full proof graph.
What it does give users is a better answer to two practical questions:

- why did this witness path go through this file?
- why was this file retained by the bounded-slice planner for this seed?

That is the right level of G6 explainability:

- keep `summary.slice_selection` as the file-selection surface
- keep `impacted_witnesses` as the chosen-path surface
- add a light linkage between them instead of merging the two abstractions

Relevant coverage:

- `src/impact.rs` unit coverage for witness/slice linkage
- CLI JSON/per-seed fixture assertions in `tests/cli_pdg_propagation.rs`

## 7. The README guidance now matches the G6 model, not the G5-only mental model

`#484` updated both README variants to reflect the current state more honestly.

The public docs now say plainly that:

- bounded slice is the PDG/propagation scope model
- the planner is now a controlled 2-hop model rather than a single ad hoc extra file
- `summary.slice_selection` is the place to inspect planner decisions
- `impacted_witnesses[*].slice_context` is the place to connect file selection back to the chosen witness path
- Ruby improved in short bounded cases but still has clear limits on longer ladders / dynamic-heavy flows / broader companion discovery

This matters because by the end of G6, the implementation had moved enough that the old documentation would have become misleading again if left untouched.

---

## Improvement cases G6 materially helped

The clearest practical wins from G6 are:

### 1. File-level bounded-slice selection is now inspectable

Improved from:

- path membership existing mostly as internal state
- no user-visible explanation for root vs boundary vs bridge-completion file selection

To:

- selected file metadata in `summary.slice_selection.files[*]`
- per-seed reasons and bridge-kind metadata
- minimal pruned-candidate diagnostics in `summary.slice_selection.pruned_candidates[*]`

### 2. The bounded slice model is now controlled 2-hop, not just “one extra file if lucky”

Improved from:

- per-seed one-off bridge completion
- shallow/fragile same-side completion behavior

To:

- boundary-side-aware Tier 2 selection
- bounded but more capable second-hop retention
- deterministic prune diagnostics and stronger reviewability

### 3. A real Rust same-side FN is gone

Improved from:

- helper/noise-side same-side candidates sometimes beating the return-relevant completion

To:

- wrapper-return evidence winning the Tier 2 ranking for that short Rust multi-file case
- the rejected alternative still being visible as `ranked_out`

### 4. A real Ruby short multi-file propagation weakness is improved

Improved from:

- Ruby no-paren wrapper parameter flow dropping out of short multi-file propagation

To:

- a bounded `require_relative` / wrapper / return-flow case that now retains the expected caller-arg / callee-param bridge

### 5. Witnesses now answer slightly more than “last hop only”

Improved from:

- witness path and compact witness fields explaining only the chosen route itself

To:

- witness output also being able to say which selected files on that route were retained by the planner and why

---

## What G6 did **not** solve

G6 improved the bounded-slice model a lot, but several boundaries remain deliberate.

## 1. This is still not project-wide PDG

Even after controlled 2-hop selection, the current model is still:

- bounded
- budgeted
- deterministic
- intentionally local

It still does **not** provide:

- recursive multi-hop closure
- whole-repo symbolic execution
- exhaustive multi-path reasoning

That is a feature, not an omission-by-accident.
The whole point of G6 was to improve bounded reasoning without breaking that constraint.

## 2. Bridge-kind evidence is still relatively shallow

G6 introduced better vocabulary and real `bridge_kind` fields, but the scoring/evidence layer is still modest.

The current implementation is strong enough for:

- wrapper-return style second-hop selection
- some alias/continuation distinctions
- short Ruby `require_relative`-style recovery

It is **not** yet a rich proof system for:

- typed bridge classification across many competing candidates
- more semantic companion/fallback ranking
- large mixed alias/return/import chain scoring

## 3. Ruby remains intentionally bounded and conservative

Ruby improved materially in G6, but it still has clear limits.

Still weak / intentionally bounded:

- longer `require_relative` ladders
- broader companion discovery
- dynamic-send-heavy chains
- wider namespace/module heuristics

So the right mental model is still:

- useful bounded multi-file explanation aid
- not a complete inter-procedural Ruby data-flow engine

## 4. Witness linkage is lightweight on purpose

`slice_context` is useful, but it is not a full witness explainer.
It does **not** yet provide:

- all competing paths
- full path-vs-file justification trees
- explicit “why not this file?” narratives beyond pruned candidate metadata
- a synthesized human-readable explanation layer

G6 intentionally stopped at the lighter connection point.

## 5. Planner scopes are still bundled more than ideal internally

The public output now distinguishes `cache_update` / `local_dfg` / `explanation`, which is already a major improvement.
But internally, the planner/runtime boundary can still be decomposed further if a later phase needs cleaner architecture.

---

## Why this closes G6

G6 was about turning the bounded-slice path into something more explainable and a bit stronger, without abandoning its bounded nature.

That goal is now met:

- planner-reason gaps were documented and given a real schema
- slice-selection metadata is emitted in JSON/YAML and fixed in regressions
- controlled 2-hop selection landed in the planner
- one real Rust 2-hop weakness and one real Ruby multi-file weakness were improved
- witness output is now lightly connected back to slice-selection reasons
- README / README_ja explain the current model and its limits in public-facing language

What remains after G6 is evolutionary, not foundational.
The bounded-slice model now has a real explainability contract and a better 2-hop footing.

---

## Next-stage candidates after G6

The best next work is probably **not** “make the planner much wider”.
The more valuable direction is to deepen the same bounded model selectively.

### Candidate 1: stronger bridge-kind scoring before wider scope growth

Highest-value next step.

Reason:
- G6 now exposes enough planner metadata that better scoring can be reviewed and tested cleanly
- same-side candidate competition will likely matter more than simply adding more budget

Concrete targets:
- richer evidence scoring for `wrapper_return` vs alias continuation vs `require_relative_chain`
- better use of call-site position / summary evidence / certainty in Tier 2 ranking
- stronger `ranked_out` diagnostics when several plausible bridge files compete

### Candidate 2: narrow Ruby companion / fallback improvements

High value, but should stay intentionally constrained.

Reason:
- Ruby is still the clearest place where short multi-file explanation is useful but easy to over-widen

Concrete targets:
- narrow companion fallback for selected `require_relative` chain shapes
- preserve dynamic-send/public-send separation while improving short companion-style recoveries
- keep broad namespace/path heuristics out of scope

### Candidate 3: better witness-facing explanation UX without proof-graph explosion

Reason:
- G6 made the file/path link possible
- the next useful step is probably better presentation rather than much more raw graph material

Concrete targets:
- clearer JSON/YAML narration fields for “why this file on this path?”
- possible DOT/HTML annotations based on `slice_context`
- better compact witness summaries that mention selection tier / bridge kind when helpful

### Candidate 4: cleaner internal separation of planner scopes

Reason:
- the public schema now distinguishes `cache_update` / `local_dfg` / `explanation`
- later maintenance will get easier if the runtime side mirrors that separation more directly

Concrete targets:
- reduce planner/runtime coupling inside `src/bin/dimpact.rs`
- make selection/explanation structures easier to reuse in future reporters or debug modes

### Candidate 5: broader language expansion should wait until the bounded model gets sharper

Reason:
- the current Rust/Ruby concentration is still appropriate
- widening language claims before the bounded planner gets sharper would likely create noisy expectations

That suggests the safer order is:

1. deepen bounded selection quality
2. improve witness/explanation presentation
3. only then consider broader language-specific PDG/propagation ambitions

---

## One-sentence summary

G6 turned bounded slice from a mostly internal small-scope trick into a visible, testable, controlled-2-hop planner with real file-level reasons, better Rust/Ruby multi-file recovery, and a lightweight connection from witness paths back to slice-selection decisions.
