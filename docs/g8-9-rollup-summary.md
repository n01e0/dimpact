# G8 rollup: evidence-driven selection, true narrow fallback, winning-evidence witness output, and bounded precision guardrails

## What landed

This document closes out the main G8 work around bounded-slice evidence collection and explanation.

G8 was not about making the planner bigger.
The real target was narrower and more useful:

**improve bounded-slice selection quality and witness explanation quality by feeding the planner more semantic evidence, materializing one true narrow-fallback runtime path, and locking the resulting precision gains without widening explanation scope.**

In practice, G8 landed in eight layers:

- first, inventory which evidence kinds were still lexical or schema-only and define the G8 evidence plan
- next, extend the public scoring schema so semantic evidence and support metadata have a stable home
- then, improve one real Rust competition with `param_to_return_flow`
- next, land one real Ruby true narrow-fallback runtime for a dynamic-heavy `method_missing` companion case
- then, surface winning evidence and winning support in witness explanations
- next, define a fixed evaluation set for the new evidence-driven surfaces
- then, strengthen regressions so precision improves without broadening bounded explanation scope
- finally, update the READMEs so the public mental model matches the new evidence-driven planner behavior

## Merged PRs

- `#496` docs: add G8 evidence design memo
- `#497` feat: extend bridge scoring evidence schema
- `#498` feat: strengthen Rust bridge evidence selection
- `#499` feat: add Ruby narrow fallback companion selection
- `#500` feat: add winning evidence to witness explanations
- `#501` docs: add G8 evidence-driven eval set
- `#502` test: add precision regressions for bounded slice winners
- `#503` docs: document evidence-driven selection in READMEs

---

## What changed in practice

## 1. G8 gave the bounded planner a sharper evidence contract

G7 already separated `bridge_kind` from the actual reviewable `scoring` object.
What G8 added was a clearer answer to:

- which evidence kinds are real semantic facts
- which ones are still shallow hints
- how support strength should be represented separately from the evidence kind itself

The key schema/runtime vocabulary added or activated in G8 is:

- semantic evidence such as `param_to_return_flow`
- narrow-fallback evidence such as
  - `explicit_require_relative_load`
  - `companion_file_match`
  - `dynamic_dispatch_literal_target`
- support metadata such as
  - `local_dfg_support`
  - `symbolic_propagation_support`
  - `edge_certainty`

Relevant docs:

- `docs/g8-1-missing-evidence-inventory-and-design-memo.md`
- `docs/g8-2-bridge-scoring-evidence-schema.md`
- `docs/g8-2-bridge-scoring-evidence-schema.json`

The practical result is that G8 no longer treats evidence collection as “just another ranking hint.”
It gives the planner and witness surface a more explicit distinction between:

- observed continuation facts
- support/provenance strength
- compact human-facing winner explanation

## 2. One real Rust misselection now improves for a semantic reason, not just a positional one

`#498` landed the first real G8 runtime use of the new evidence shape.

The important behavior change is:

- a Rust Tier 2 candidate can now materialize `param_to_return_flow`
- that candidate can also carry `support.local_dfg_support = true`
- the planner can therefore prefer the semantically relevant passthrough leaf over a later neutral helper

This matters because the fixed competition is no longer saying:

> the later call won

It is saying something closer to:

> the candidate with stronger parameter-origin continuity won inside the same bounded slice

Locked improvement surface:

- selected: `step.rs`
- pruned: `later.rs`
- ranking basis: `primary_evidence_count`
- winning evidence: `param_to_return_flow`
- winning support: `local_dfg_support`

Relevant docs / coverage:

- `docs/g8-3-rust-param-to-return-evidence.md`
- `tests/cli_pdg_propagation.rs::pdg_slice_selection_prefers_param_passthrough_leaf_over_later_neutral_helper`
- `src/bin/dimpact.rs::collect_rust_tier2_semantic_evidence_detects_param_to_return_flow`

## 3. Ruby now has one true narrow-fallback runtime path instead of only schema vocabulary

Before G8, `source_kind = narrow_fallback` and `lane = module_companion_fallback` were mostly vocabulary.
G8 turned that into one real bounded runtime path.

`#499` added a minimal Ruby narrow-fallback runtime for a dynamic-heavy companion case built around:

- `require_relative`
- `public_send("route_created", payload)`
- `method_missing`
- `respond_to_missing?`

The important change is not “Ruby fallback got broader.”
It is the opposite:

- bounded fallback stays intentionally narrow
- but one real companion case can now materialize as a true fallback candidate with reviewable evidence

Locked selected scoring shape:

- `source_kind = narrow_fallback`
- `lane = module_companion_fallback`
- `primary_evidence_kinds = [companion_file_match, dynamic_dispatch_literal_target, explicit_require_relative_load]`
- `support.edge_certainty = dynamic_fallback`

Relevant coverage:

- `src/bin/dimpact.rs::bounded_slice_plan_selects_ruby_method_missing_companion_as_narrow_fallback`
- `docs/g8-6-evidence-driven-eval-set.md`

This is the clearest G8 proof that bounded Ruby fallback can improve without reopening broad companion expansion.

## 4. Witnesses can now say which evidence actually won

G7 added minimal selected-vs-pruned reasons.
G8 made that surface materially more useful.

After `#500`, witness `slice_context.selected_vs_pruned_reasons` can now carry:

- `winning_primary_evidence_kinds`
- `winning_support`
- stronger summary strings that mention the winning evidence/support, not just the winning ranking basis

This matters because the system can now answer a more operational question:

> why did this file win, in evidence terms?

instead of only:

> which ranking dimension broke the tie?

Relevant coverage:

- `src/impact.rs::selected_vs_pruned_reason_derives_winning_metadata_for_source_kind_explanations`
- `tests/cli_pdg_propagation.rs` selected-vs-pruned witness assertions
- `docs/g8-6-evidence-driven-eval-set.md`

The improvement is still intentionally compact.
G8 did **not** turn witness output into a full proof trace.
But it did make the chosen winner legible in semantic terms.

## 5. G8 now has a fixed evaluation set for evidence-driven behavior

`#501` matters because it turned the new G8 surfaces into an explicit evaluation contract instead of leaving them as scattered fixtures.

The fixed G8 evaluation set locks four areas:

1. Rust semantic evidence beating positional/helper noise
2. Ruby true narrow fallback materialization
3. witness winning-evidence explanation for selected-vs-pruned competition
4. Ruby CLI selected/pruned witness surface under a real competition

Relevant docs:

- `docs/g8-6-evidence-driven-eval-set.md`
- `docs/g8-6-evidence-driven-eval-set.json`

This gives G8 a stable answer to:

- what should improve
- what must not regress
- how to recognize scope widening disguised as “more intelligence”

## 6. Regression coverage now locks precision gains without scope growth

`#502` is important because it locked the intended G8 outcome at the right boundary:

- stronger evidence should improve the selected file
- weaker alternatives should remain visible as ranked-out metadata
- helper noise should not silently become explanation scope

The added regressions strengthen both:

- CLI selected-vs-pruned witness assertions
- library-level witness/slice-context assertions

In practice, the regressions now lock that:

- the winning file keeps its evidence-driven explanation
- the losing helper remains reachable only as ranked-out context where appropriate
- the selected explanation path stays slimmer than the broader reachable graph

This is the most important strategic result of G8 after the runtime fixes themselves.
The phase validated the hypothesis from G8-1:

- several remaining pain points were evidence/ranking/explanation problems
- not raw scope-size problems

## 7. The public README guidance now matches the evidence-driven planner

`#503` updated both README variants to explain that the PDG / propagation planner is:

- bounded by design
- evidence-driven in how it chooses a local continuation
- reviewable through
  - `summary.slice_selection.files[*].reasons[*].scoring`
  - `summary.slice_selection.pruned_candidates[*].scoring`
  - `impacted_witnesses[*].slice_context.selected_vs_pruned_reasons`

This matters because by the end of G8, the old mental model would be misleading.
The planner is no longer best described as:

> a thing that sometimes pulls in nearby helper files

It is better described as:

> a bounded planner that tries to choose the strongest local continuation and keep weaker alternatives visible as pruned evidence instead of expanding explanation scope

---

## Improvement cases G8 materially helped

The clearest practical wins from G8 are:

### 1. Rust semantic evidence now beats later helper noise in one real Tier 2 case

Improved from:

- a neutral helper being able to win too easily by later call position or shallow hints

To:

- a passthrough leaf winning because it carries `param_to_return_flow` plus `local_dfg_support`

### 2. Ruby has one real true narrow-fallback runtime case

Improved from:

- narrow fallback being mostly schema vocabulary and design intent

To:

- a bounded `method_missing` companion case selecting a real `module_companion_fallback` candidate with explicit fallback evidence

### 3. Witness explanations now expose the winning evidence/support

Improved from:

- witness reasons only naming the winning ranking basis

To:

- witness reasons also naming the evidence/support that actually made the winner stronger

### 4. G8 proved that precision can improve without widening explanation scope

Improved from:

- a plausible risk that stronger bridge/fallback logic would implicitly broaden bounded slices

To:

- fixture-backed regressions that keep helper noise outside selected explanation paths while preserving ranked-out diagnostics

### 5. The evidence-driven mental model is now documented end-to-end

Improved from:

- design notes existing in docs/tests but not clearly in the user-facing README story

To:

- public guidance that explains how to read selected/pruned scoring and witness reasoning in operational order

---

## What G8 did not do

G8 made the bounded planner more semantic and more explainable.
It did **not** solve every remaining limitation.

Important non-landed items:

- no project-wide recursive closure
- no broad Ruby companion discovery beyond the bounded true narrow-fallback path that now exists
- no full semantic rewrite of all existing evidence kinds (`assigned_result`, `alias_chain`, `return_flow`, etc.)
- no exhaustive witness proof trace with multiple competing routes
- no large cross-language rollout of semantic evidence collection beyond the concrete Rust/Ruby G8 cases
- no benchmark-style quantitative precision dashboard for the whole evidence surface beyond the fixed regression/eval sets

In short, G8 improved **evidence quality and explanation quality inside the bounded model**.
It did not turn the planner into a general inter-procedural proof engine.

---

## Before / after summary

The shortest honest summary of G8 is:

### Before G8

- scoring had a reviewable schema, but too much runtime evidence was still shallow or lexical
- narrow fallback existed more as vocabulary than as a real bounded runtime lane
- witness selected-vs-pruned reasons were useful but too thin to say what evidence actually won
- precision gains were not yet locked as an explicit evidence-driven evaluation surface

### After G8

- semantic/support evidence has a stable public contract
- one real Rust competition improves because of `param_to_return_flow`
- one real Ruby dynamic-heavy companion case now materializes as bounded narrow fallback
- witness output can name winning evidence/support in compact form
- a fixed eval set and stronger regressions protect the new evidence-driven surfaces
- the README now describes the planner as bounded and evidence-driven, not merely heuristic file expansion

---

## Unresolved points carried into the next phase

The main unresolved items after G8 are:

## 1. Evidence collection is still uneven across lanes

G8 proved the value of semantic evidence, but only a small subset of the current evidence vocabulary is truly semantic today.

Most important remaining gap:

- extend semantic collection beyond `param_to_return_flow` and the minimal Ruby narrow-fallback evidence set
- reduce remaining dependence on shallow lexical/name/path hints where they still dominate tie-breaking

## 2. Narrow fallback remains intentionally narrow and still sparse

This is mostly a feature, not a bug.
But the current runtime still covers only a small bounded subset of companion/fallback shapes.

Open question:

- which additional Ruby fallback patterns are genuinely missing necessary bounded context
- and which ones should stay out-of-scope because they would only broaden the planner without improving explanation quality

## 3. Witness explanation is still single-route and intentionally compact

G8 improved witness explanation a lot, but it still stops at one selected route plus one compact selected-vs-pruned explanation.

Remaining limitation:

- it cannot yet explain more than one decisive comparison on a path
- it does not expose alternative witness routes
- it does not distinguish “ranked out” vs “budget pruned” in richer narrative form

## 4. Evaluation is fixed, but not yet aggregated into a lightweight phase metric

G8 now has a fixed eval set and stronger regressions.
What it still lacks is a compact phase-level answer to:

- how many evidence-driven competition cases improved
- how many remain ambiguous
- which unresolved patterns are still outside the current fixed set

## 5. The planner still needs disciplined pressure against scope creep

G8 validated the bounded strategy.
The next risk is accidental erosion of that discipline through “just include one more helper/companion” changes.

The unresolved strategic rule remains:

- widen bounded scope only when a fixed case proves missing necessary context
- otherwise prefer stronger evidence, better ranking, and clearer witness explanation

---

## Recommended G9 candidates

The best next-stage candidates after G8 are probably:

## 1. Make more existing evidence kinds semantic instead of lexical

Best next step:

- strengthen runtime derivation of `assigned_result`, `alias_chain`, and `return_flow`
- keep `name_path_hint` as a tie-breaker, not as a practical primary driver in cases where local structure is available

Why this is next:

- G8 proved semantic evidence can change outcomes cleanly
- the next leverage is broadening that benefit within the existing bounded lanes

## 2. Expand bounded Ruby fallback only where the eval set proves it is worth it

Best next step:

- add one or two carefully chosen Ruby fallback competition cases
- only materialize additional fallback evidence when it helps a fixed bounded case without turning into broad companion expansion

Why this is next:

- G8 now has one true narrow-fallback success case
- the next challenge is deciding which follow-ups improve explanation quality rather than just increasing candidate count

## 3. Refine witness explanation beyond one winning fact when it materially helps review

Best next step:

- optionally expose the first two decisive ranking differences
- improve wording around ranked-out vs budget-pruned alternatives
- consider richer compact witness explanation only when it stays clearly smaller than the raw planner scoring surface

Why this is next:

- G8 showed winning evidence/support is useful
- the remaining work is refinement, not invention

## 4. Add a lightweight rollup/eval report for evidence-driven cases

Best next step:

- build a small report that summarizes the fixed G8 eval cases and their expected winner/pruned shape
- keep it documentation/test-facing, not a large new reporting subsystem

Why this is next:

- the phase now has enough fixed cases to summarize meaningfully
- a lightweight report would help future follow-up tasks avoid repeating manual archaeology

## 5. Keep bounded-slice discipline explicit in every follow-up

Best next step:

- require new evidence/fallback changes to show not only the winner improvement, but also the non-widening behavior for explanation scope
- continue treating `pruned_candidates` and witness selected-vs-pruned output as first-class regression surface

Why this is next:

- G8’s biggest strategic win was proving that better selection does not require broader explanation scope
- future follow-ups should keep that property explicit

---

## Closing note

G8 is a meaningful maturity step for bounded-slice selection.

It does not make dimpact “done” at multi-file PDG / propagation reasoning.
What it does do is move the project from:

- reviewable scoring with still-thin evidence
- fallback vocabulary with limited runtime realization
- minimal witness choice reasons

to something substantially more useful:

- evidence-driven ranking with a sharper semantic/support contract
- one real bounded Ruby narrow-fallback lane
- one real Rust semantic winner case
- witness-level winning evidence/support explanation
- fixed eval/regression surface that protects precision without widening explanation scope
- public docs that describe the bounded planner honestly

That is a good place to stop G8, record the unresolved boundaries clearly, and choose G9 work by evidence value rather than by planner size.