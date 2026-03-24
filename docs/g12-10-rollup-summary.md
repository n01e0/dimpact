# G12 rollup: multi-input continuation, alias-result stitching, bridge-execution provenance, and planner/policy alignment

## What landed

This document closes out the main G12 work around bounded continuation after G11.

G11 proved that dimpact could keep one more bounded continuation file in scope for short Rust/Ruby wrapper-return chains.
G12 was the next logical step:
not “open more files,” but make the bounded continuation model better at **connecting the right continuation chain once those files are already selected**.

The practical goal was:

**inventory the weak multi-input / alias-result stitching cases, define the bounded stitching policy, land at least one real Rust improvement and one real Ruby improvement, strengthen bridge-execution provenance, lock the new behavior with regressions, align the bounded planner with the policy, and update the public docs so the runtime model matches what the code actually does.**

In practice, G12 landed in nine layers:

- first, re-baseline the current bounded continuation weaknesses against `main`
- next, define a fixed failure eval set for multi-input continuation / alias-result stitching
- then, write the bounded stitching policy that separates selected scope from execution-chain closure
- next, land one real Rust-side nested multi-input continuation improvement
- then, land one real Ruby-side propagation cleanup for short alias/`require_relative` caller chains
- next, add compact bridge-execution provenance to witness output
- then, expand regression coverage for the improved multi-input / stitching cases
- next, align the bounded planner so alias-family continuation can extend into the same bounded tier-3 surface
- finally, update README / README_ja so the public explanation includes the new scope/provenance story and the still-missing parts

## Merged PRs

- `#537` docs: add G12 bounded continuation design memo
- `#538` docs: add G12 failure eval set
- `#539` docs: define G12 bounded stitching policy
- `#540` pdg: tighten nested multi-input continuation bridges
- `#541` pdg: filter weak duplicate ruby callsite refs
- `#542` impact: add bridge execution provenance
- `#543` tests: expand G12 multi-input regressions
- `#544` planner: extend alias continuation tier3 anchors
- `#545` docs: update G12 continuation scope notes

---

## What changed in practice

## 1. G12 changed the main question from “can scope reach the file?” to “can the selected files be stitched into the right chain?”

`#537` matters because it reset the phase around current `main` instead of the older G11 baseline.
By the time G12 started, the runtime already had:

- bounded slice planning
- direct boundary selection
- bridge completion files
- one-hop continuation files
- selected/pruned reasoning in JSON and witness surfaces

So G12 was not mainly about adding another scope tier.
It was about the harder current gap:

- nested multi-input continuation
- alias-result stitching as its own family
- bridge-execution provenance that explains **which continuation chain actually won**

That was the right reframing.
Without it, the phase would have kept talking about file inclusion while the real misses were already execution-contract misses.

## 2. G12 fixed a concrete failure set instead of treating “stitching is weak” as one blob

`#538` locked five current failure families:

- Rust nested multi-input continuation
- Rust reordered / partial input-binding continuation
- Rust wrapper + caller double alias-result stitching
- Rust alias-family continuation beyond tier 2
- Ruby `require_relative` + alias-result mixed stitching

This matters because it split the remaining work into more honest buckets:

- propagation / binding problems
- alias/stitching problems
- planner continuation-family problems
- witness/provenance problems

That gave the later code changes a much cleaner contract.
G12 no longer needed vague claims like “multi-file continuation is better now.”
It could say which failure family improved, which still only had policy coverage, and which still remained only partially modeled.

## 3. G12 defined bounded stitching as an execution policy, not as more scope growth

`#539` is the design center of the phase.
It fixed the split between:

- **planner responsibility**: choose a bounded file frontier and retain family representatives
- **stitching responsibility**: use that selected frontier to close the continuation/result chain

The important conceptual outcome was:

- selected scope stays bounded
- stitching does not reopen pruned files
- continuation closure is described as a bounded chain of step families

The step vocabulary was fixed around:

- `callsite_input_binding`
- `summary_return_bridge`
- `nested_summary_bridge`
- `alias_result_stitch`
- `require_relative_load`

And the coarse family vocabulary was fixed around:

- `return_continuation`
- `alias_result_stitch`
- `require_relative_continuation`
- `mixed_require_relative_alias_stitch`
- `nested_multi_input_continuation`

That was the most important design outcome of G12.
The phase did **not** choose recursive widening, full binding inference, or project-wide continuation search.
It chose a bounded execution model layered on top of the already-bounded planner.

## 4. Rust now handles one real nested multi-input continuation case that used to fall short

`#540` is the clearest Rust proof point.
It improved the short shape where:

- a wrapper takes more than one input
- the nested callee also has more than one input
- only one of those inputs is actually relevant to the returned result

Before G12, the nested bridge contract still effectively collapsed back toward single-input assumptions.
After `#540`:

- nested continuation bridging can bind the relevant callsite use into a multi-input nested summary
- caller-result bridging is gated more carefully, so the relevant path can close without dragging an irrelevant arg along with it
- the regression explicitly checks that the relevant `y -> out` style bridge lands while the irrelevant `x -> out` leak does **not** appear

This is a real precision/recall gain.
It did not just widen scope.
It improved the **shape of continuation closure** inside already-selected scope.

## 5. Ruby got a smaller but still useful stitching-side cleanup

`#541` is narrower than the Rust win, but still important.
It improved the Ruby short-chain propagation surface by filtering weak duplicate callsite refs before symbolic stitching.

The practical effect is:

- propagation keeps the real caller-side bridge into the `require_relative`-loaded callee
- propagation avoids bogus duplicate callsite bridges that were being synthesized from later return/param-adjacent refs
- short Ruby alias/`require_relative` caller chains become less noisy and less misleading

This is the right kind of Ruby win for G12.
It is not “Ruby multi-input is solved.”
It is a bounded, regression-backed improvement that makes the current narrow Ruby stitching path less error-prone.

## 6. Witnesses can now explain the selected continuation/stitching family directly

`#542` is the biggest explanation/output gain of the phase.
Before it, witness output already had:

- `path`
- `provenance_chain`
- `kind_chain`
- `slice_context`
- selected-vs-pruned file reasoning

That was strong for:

- why this file was selected
- which path was chosen

But it was still weaker for:

- which continuation family actually closed the result

After `#542`, `ImpactWitness` also carries compact bridge-execution provenance:

- `bridge_execution_family`
- `bridge_execution_chain_compact`

This lets the output explain, in a compact way, whether the chosen route was being carried by:

- `return_continuation`
- `alias_result_stitch`
- `require_relative_continuation`
- mixed require-relative / alias-result behavior
- nested continuation-style behavior

And it surfaces execution-step metadata like:

- `callsite_input_binding`
- `summary_return_bridge`
- `nested_summary_bridge`
- `alias_result_stitch`
- `require_relative_load`

This is a meaningful shift.
G12 made witness explanation stronger not by dumping more raw graph state, but by exposing a compact **selected bridge-execution story**.

## 7. The improved cases are now locked by stronger regression coverage

`#543` matters because G12 would be hard to trust if the wins were only covered by one dot-format assertion or one narrow fixture.

The added regressions lock in:

- Rust nested two-arg continuation behavior across baseline / `--with-pdg` / `--with-propagation`
- the non-leak guarantee for the irrelevant arg
- Ruby `require_relative` caller-alias behavior with explicit PDG vs propagation comparison
- per-seed bridge-selection stability for the improved nested Rust case

This is the correct regression surface for G12.
The phase was not only about “an extra file was selected” or “a single edge appeared.”
It was about:

- the right continuation family being used
- the wrong continuation not leaking
- the better case staying visible in per-seed output
- plain PDG staying narrower than propagation where expected

## 8. The planner and the stitching policy are now more aligned than before

`#544` is the planner-side alignment fix.
At the start of G12, one of the most obvious planner/policy mismatches was:

- the docs/policy already treated alias-result stitching as its own family
- tier-2 planner vocabulary already knew `boundary_alias_continuation`
- but tier-3 continuation anchors were still effectively wrapper-return-only

After `#544`:

- admitted alias-continuation tier-2 candidates can now serve as tier-3 continuation anchors
- bounded continuation can extend one more step in that same alias family
- a dedicated regression proves that the alias-family continuation becomes a real `bridge_continuation_file`

This is small, but strategically important.
It means the planner no longer contradicts the G12 stitching model at its most obvious same-family continuation boundary.

## 9. README / README_ja now describe the actual G12 runtime model

`#545` updated the public docs so they describe the current G12 state honestly.
The important wording shifts are:

- bounded continuation now includes the narrow alias-continuation follow-up that landed in G12
- G12’s concrete propagation gains are called out explicitly
- the new witness-side `bridge_execution_family` / `bridge_execution_chain_compact` metadata is documented
- the remaining limits are made more explicit:
  - no recursive whole-project closure
  - no general expression normalization / arbitrary argument matching
  - reordered/partial binding is still partial
  - wider alias zones and richer family-aware budgets are still incomplete

That matters because G12 changed the mental model more than the CLI surface.
Without these doc updates, the public explanation would lag behind the runtime in exactly the place G12 worked on.

---

## Improvement cases G12 materially helped

The clearest practical wins from G12 are:

### 1. Rust nested multi-input continuation without irrelevant-arg leakage

Improved from:

- nested continuation falling back toward single-input assumptions
- caller-result closure being weak for the relevant arg
- accidental over-propagation risk for the irrelevant arg

To:

- relevant nested input can propagate through the selected nested summary path
- caller-result closure lands for the relevant arg/result chain
- the irrelevant arg stays out of the final caller-result bridge in the locked regression

Locked surface:

- `tests/cli_pdg_propagation.rs`
- baseline / PDG / propagation comparison
- per-seed bridge-selection assertions

### 2. Ruby short `require_relative` caller-alias propagation with less duplicate-callsite noise

Improved from:

- correct short caller-side stitching competing with noisy duplicate callsite refs

To:

- propagation keeps the real caller → callee bridge
- bogus `def:seed` / `use:out` style duplicate bridges stay absent
- plain PDG remains narrower than propagation in the locked regression

Locked surface:

- JSON edge-level assertions in `tests/cli_pdg_propagation.rs`
- explicit PDG vs propagation comparison

### 3. Witness-side bridge-execution provenance for short continuation/stitching routes

Improved from:

- file/path/provenance explanation only

To:

- compact execution-family metadata in `impacted_witnesses`
- compact step-family chain metadata
- representative explanation for wrapper-return, alias-result stitching, and Ruby require-relative load behavior

Locked surface:

- per-seed witness assertions for Rust wrapper continuation
- per-seed witness assertions for imported-result alias stitching
- per-seed witness assertions for Ruby two-hop `require_relative` continuation

### 4. Planner continuation-family alignment for alias follow-up

Improved from:

- alias family visible at tier 2 but effectively blocked from tier-3 continuation anchor use

To:

- same-family alias continuation beyond tier 2 under the bounded planner
- dedicated planner regression proving that `boundary_alias_continuation` can now produce a `bridge_continuation_file`

Locked surface:

- `src/bin/dimpact.rs` planner regression
- imported-result witness case remains green
- wrapper-return continuation case remains green

---

## What G12 did **not** do

This is just as important as what it landed.

G12 did **not** turn the bounded continuation path into:

- a project-wide PDG
- a generic cross-file DFG / SSA system
- a recursive whole-repo continuation search
- a complete argument-binding engine
- a full alias-analysis engine
- a full-parity implementation across non-Rust/Ruby languages

It also did **not** fully solve every failure family from the eval set.
In particular, G12 still did **not** fully solve:

- broad reordered / partial multi-input binding
- wider alias-result stitching across multiple alias zones and richer reassignments
- mixed Ruby `require_relative` + alias-result stitching as a generally closed family
- family-aware continuation budgeting beyond the current small bounded caps
- exhaustive bridge-execution explanation or all competing chain alternatives

That is a healthy outcome.
G12 moved one real layer forward without pretending to have solved the whole continuation space.

---

## Remaining gaps after G12

The most important remaining gaps are now smaller and clearer than before.

## 1. Binding-map quality is still the biggest continuation gap

The strongest remaining technical target is still:

- reordered input binding
- partial input binding with literal/ignored companion slots
- repeated binding and non-trivial mapping beyond simple zip-like recovery

G12 improved one selected nested multi-input case, but it did **not** generalize input binding into a rich first-class map across all short cases.

## 2. Alias-result stitching is better, but still narrower than the policy vocabulary

G12 aligned the vocabulary and improved one planner follow-up, but there is still a gap around:

- wrapper-local alias zone + caller-local alias zone combinations beyond the current narrow success shapes
- broader imported-result stitching when reassignment or non-trivial alias structure appears
- mixed require-relative + alias-result continuation in Ruby beyond narrow cases

This means alias-result stitching remains the next major continuation family after nested multi-input precision.

## 3. Planner budgeting is still intentionally small and somewhat blunt

G12 aligned planner family handling enough to admit alias-family tier-3 follow-up, but it did not fully implement the richer family-aware budgeting described in the policy memo.

Future work likely still needs to look at:

- per-family representative budgeting
- return vs alias-family coexistence under small caps
- when a valid later candidate should displace an earlier weaker survivor

This is planner work, not just propagation work.
It deserves its own follow-up phase instead of being mixed into a continuation-bridge code tweak.

## 4. Bridge-execution provenance is now useful, but still representative

G12 made a real step forward here, but the current witness story is still stronger for:

- what selected route won
- what coarse bridge family that route represents

than for:

- what all competing execution chains looked like
- exactly how a richer binding map was chosen at each step

A later phase could make the chain metadata richer without losing the compactness that makes the current witness output useful.

## 5. Non-Rust/Ruby parity is still intentionally absent

G12 did the right thing by not broadening language scope.
But the remaining language story is still:

- Rust and Ruby get the meaningful bounded continuation / stitching improvements
- other languages are mostly still the normal impact path with limited local augmentation

That is fine for now, but it should remain explicit as planning continues.

---

## Best next-phase candidates

If G13 follows naturally from G12, the strongest candidates look like this:

## Candidate A: richer input-binding maps for reordered / partial multi-input continuation

Why first:

- it is the clearest remaining false-negative family after the Rust nested case landed
- it is already fixed in the eval set
- it builds directly on G12’s continuation/stitching policy vocabulary

Likely goal:

- move beyond simple order-preserving or near-order-preserving recovery
- represent partial/reordered binding more explicitly
- keep the non-leak guarantee for irrelevant args

## Candidate B: broader alias-result stitching across wrapper + caller chains

Why next:

- G12 aligned the alias-family story but only partially improved runtime behavior
- wrapper-return is still ahead of general alias-result stitching

Likely goal:

- strengthen imported-result -> alias -> caller-result continuity
- cover broader wrapper-local + caller-local alias combinations
- keep the gains tied to bounded same-family continuation instead of widening scope globally

## Candidate C: mixed Ruby `require_relative` + alias-result continuation

Why useful:

- the eval set already names it
- Ruby now has less noisy short-chain propagation, but mixed continuation/stitching closure is still only partially modeled

Likely goal:

- preserve bounded admission
- improve short mixed-chain closure without broad companion expansion
- keep the Ruby story honest and regression-backed

## Candidate D: family-aware continuation budgeting

Why later but important:

- planner continuation is less misaligned than before, but the budget contract is still coarse
- some remaining misses will be about which valid candidate survives, not whether a continuation exists at all

Likely goal:

- preserve boundedness
- allow return and alias-result families to coexist under tighter but less blunt caps
- keep prune reasoning explicit in `summary.slice_selection`

## Candidate E: richer but still compact bridge-execution explanation

Why useful:

- `bridge_execution_family` / `bridge_execution_chain_compact` are a real step up
- the next step is likely “more precise, not much larger” output

Likely goal:

- carry selected binding hints where they can be derived cleanly
- distinguish more mixed-chain cases without dumping raw planner state
- stay compact enough to remain readable in per-seed JSON

---

## Final assessment

G12 was a good continuation phase because it stayed disciplined.
It did not try to solve whole-program continuation.
It improved the bounded model where the real current misses had already moved:

- execution-chain closure
- alias-result stitching as its own family
- witness-side explanation of the chosen continuation route
- planner/policy alignment for the same bounded family surface

The phase succeeded because it moved in a sensible order:

- first inventory the weak shapes
- then lock the failure set
- then define the stitching policy
- then land one Rust improvement and one Ruby improvement
- then strengthen provenance
- then lock regressions
- then align the planner with the policy
- then update the public docs

The net result is:

- bounded continuation is no longer just a wrapper-return scope story
- one real nested multi-input Rust case now closes correctly without irrelevant-arg leakage
- Ruby short caller-side propagation is less noisy in a meaningful `require_relative`/alias shape
- witnesses can now explain the selected continuation/stitching family directly
- the planner now admits one narrow alias-family tier-3 continuation that the policy had already implied
- the remaining unsolved space is now more clearly split into:
  - binding-map quality gaps
  - broader alias-result stitching gaps
  - planner budgeting gaps
  - richer-but-still-compact explanation gaps

That is exactly what a healthy phase should do:

**ship a real bounded continuation/stitching gain, keep the runtime explainable, align the planner with the documented policy, and leave the next unsolved layers smaller and clearer than before.**