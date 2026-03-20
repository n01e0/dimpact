# G7 rollup: bridge candidate scoring, scope split, Ruby bounded fallback tuning, and minimal selected-vs-pruned witness reasons

## What landed

This document closes out the G7 work around bounded slice bridge selection.

G7 was not about widening dimpact from bounded slice into project-wide closure.
It was about making the existing bounded planner **choose better, explain itself better, and regress less easily**.

The practical goal was:

**separate bridge-family labels from actual ranking material, move planner comparison onto a reviewable scoring profile, split cache/DFG/explanation responsibilities, improve one real Rust misselection and one real Ruby bounded fallback weakness, and surface a minimal selected-vs-pruned reason in witness output.**

In practice, G7 landed in six layers:

- first, inventory the remaining bridge-candidate misselection patterns and define a public scoring schema
- next, implement score-based bridge comparison in the planner and fix one real Rust case
- then, define and minimally implement the planner scope split so explanation scope is no longer just a side effect of selection
- next, improve one real Ruby bounded `require_relative` / alias / return-flow competition case
- then, connect witness output to selected-vs-pruned reasoning and lock the behavior with regressions
- finally, update the READMEs so the public mental model matches the actual G7 planner behavior

## Merged PRs

- `#486` docs: add G7 bridge scoring memo
- `#487` docs: define G7 bridge scoring schema
- `#488` feat: score bridge candidates in bounded slice planner
- `#489` docs: define planner scope split policy
- `#490` feat: split bounded slice explanation scope
- `#491` feat: improve ruby bridge scoring
- `#492` feat: explain witness bridge candidate choices
- `#493` test: lock bridge scoring regression fixtures
- `#494` docs: explain bridge scoring and scope split

---

## What changed in practice

## 1. Bridge selection moved from coarse kind ordering to a reviewable scoring profile

Before G7, Tier 2 bridge comparison was still too close to:

- coarse `bridge_kind`
- call line
- lexical path tie-break

That was enough to fix a narrow same-side case in G6, but not enough to explain or stabilize more complex competition patterns.

G7 changed the public contract first, then the runtime:

- `bridge_kind` remains a short family label
- `scoring` now carries the actual compare basis
  - `source_kind`
  - `lane`
  - `primary_evidence_kinds`
  - `secondary_evidence_kinds`
  - `score_tuple`

Relevant docs:

- `docs/g7-1-bridge-candidate-scoring-design-memo.md`
- `docs/g7-2-bridge-scoring-schema.md`
- `docs/g7-2-bridge-scoring-schema.json`

The key shift is conceptual as much as technical:

- stop letting `bridge_kind` act as both label and score
- expose the actual ranking dimensions so selected-vs-pruned decisions are reviewable in output, tests, and docs

## 2. The planner now compares bridge candidates by source/lane/evidence before call position

`#488` landed the runtime side of that scoring model.

In practice, Tier 2 bridge selection now prefers, in order:

- stronger source category
- stronger lane
- stronger primary evidence count
- stronger secondary evidence count
- stronger call-position hint
- deterministic lexical tie-break

This matters because the G7 planner no longer primarily answers:

> which candidate had a nicer name or happened later?

It now answers something closer to:

> which candidate better closes the intended continuation under a bounded, explainable comparison profile?

That does **not** make the planner perfect.
It does make the ranking much less opaque and much easier to regression-test.

## 3. Scope split is now explicit: cache freshness, local DFG build, and user-facing explanation are different jobs

G6 exposed file scopes, but the runtime still bundled most selection through a single `select_path()` shape.
In practice that meant `explanation=true` often happened as a side effect of selecting a file at all.

G7 fixed this in two phases:

- `#489` documented the policy and invariants
- `#490` implemented the minimal split in the accumulator/runtime

The intended roles are now explicit:

- `cache_update`
  - execution preparation / freshness
- `local_dfg`
  - local flow materialization
- `explanation`
  - user-facing retained file

Important runtime consequences:

- `local_dfg` and `explanation` both imply `cache_update`
- `local_dfg` and `explanation` can diverge
- a file can remain in `summary.slice_selection.files[*]` with `explanation=false`
- pruned candidates stay in `pruned_candidates[*]` and do **not** become explanation files

This matters because G7 made it possible to shrink explanation scope without necessarily shrinking internal execution scope in the same blunt way.

## 4. One real Rust bridge misselection is now fixed under the new scoring contract

G7 did not stop at docs/schema work.
It used the new scoring shape to fix a real Rust misselection.

The landed regression is a same-side competition where an adapter/helper-style candidate could win too easily over the semantically relevant completion.

After G7:

- the selected Rust bridge file carries `scoring`
- the losing helper candidate survives as a ranked-out `pruned_candidate`
- the fixture now locks both the winner and the loser’s score profile

Relevant coverage:

- planner test for preferring alias/return-relevant completion over helper noise
- CLI propagation fixture asserting the selected file plus ranked-out metadata

This matters because G7 proved the new scoring model actually changes runtime outcomes, not just schema shape.

## 5. Ruby bounded bridge selection is now less likely to collapse into plain `require_relative` helper preference

G6 improved a Ruby short multi-file case, but `.rb` candidates were still too easy to collapse into a generic `require_relative_chain` bucket.

`#491` improved this in a bounded way.
The important change is:

- Ruby candidates with stronger alias / return-flow semantics are no longer forced to lose to plain `require_relative` helper competition just because they are Ruby files
- weaker fallback-style `require_relative` candidates remain visible as ranked-out metadata instead of silently widening scope

This is still intentionally narrow.
G7 did **not** implement broad Ruby companion discovery or recursive fallback growth.
But it did improve a real bounded competition pattern while preserving the overall philosophy:

- keep scope bounded
- keep fallback narrow
- let semantic completion beat plain helper noise when the evidence is stronger

Relevant coverage:

- planner test for Ruby leaf completion beating later helper noise
- CLI propagation fixture for Ruby `require_relative` / return-flow witness behavior

## 6. Witnesses now carry a minimal selected-vs-pruned explanation

G6 linked witness paths to slice-selection context.
G7 made that linkage slightly more explanatory.

After `#492`, witness `slice_context` can now include a compact selected-vs-pruned reason:

- which pruned path lost
- prune reason
- which bridge kinds were involved
- which ranking basis won
- a short human-facing summary string

This is intentionally minimal.
It is **not** a full ranking trace and it does not mirror the whole score tuple into witness output.
But it does answer a question G6 could not answer directly:

> why did this selected bridge candidate beat the nearby alternative?

That is exactly the right level for G7:

- detailed score profile stays in `summary.slice_selection`
- compact human-facing “why this won” stays in witness `slice_context`

## 7. Regression coverage is now much stronger around scoring, Ruby fallback, and explanation surfaces

`#493` is important because it turned the new behavior into harder-to-accidentally-break fixture surface.

The new regressions lock at least three things more directly than before:

- the exact ranked-out Ruby helper fallback metadata
- the fact that the semantic leaf remains the selected explanation-side winner
- the fact that helper noise stays outside the bounded explanation slice even if it is still reachable in the broader graph

This matters because G7 is mostly about choosing and explaining the right bounded files.
If those choices are not fixture-backed, they drift quickly.

## 8. The README guidance now describes the actual G7 planner

`#494` updated both README variants so the public guidance now says plainly that:

- bridge candidate comparison is score-based, not just kind-based
- `summary.slice_selection` exposes score profiles for selected and pruned candidates
- witness `slice_context.selected_vs_pruned_reasons` exists as a compact explanation layer
- scope split means `cache_update`, `local_dfg`, and `explanation` are different responsibilities
- Ruby fallback is still intentionally narrow even though short semantic completion got better

That matters because by the end of G7, the old docs would have undersold both:

- how much better the planner explanation surface became
- how intentionally bounded the Ruby fallback model still is

---

## Improvement cases G7 materially helped

The clearest practical wins from G7 are:

### 1. Bridge ranking is no longer mostly opaque

Improved from:

- coarse `bridge_kind` + call line + path ordering
- little visibility into why one candidate beat another

To:

- explicit `scoring` on selected and pruned candidates
- score dimensions stable enough to lock in tests and docs
- witness-facing compact “why this won” summaries

### 2. A real Rust bridge misselection is fixed under the new ranking model

Improved from:

- helper-like candidate able to beat the semantically relevant Rust completion too easily

To:

- score-based winner selection
- losing alternative preserved as ranked-out metadata
- selected-vs-pruned difference visible in both planner output and witness output

### 3. A real Ruby bounded helper-vs-semantic competition is improved

Improved from:

- Ruby candidates too easily collapsing toward plain `require_relative` continuation handling

To:

- semantic alias / return-flow completion beating later helper noise in a bounded fixture
- weaker fallback still visible but not silently promoted into explanation scope

### 4. Explanation scope is now slimmer and more honest

Improved from:

- explanation often being a side effect of generic file selection

To:

- explicit split between execution preparation, local materialization, and user-facing retained files
- witness context filtered to explanation-visible files
- better control over what appears in bounded-slice narration

### 5. G7 made the planner easier to extend without pretending it is already project-wide

Improved from:

- pressure to “just widen scope again” when ranking/explanation were the actual problem

To:

- a clearer contract for future work:
  - scoring surface
  - scope surface
  - witness surface
  - bounded fallback surface

---

## What G7 did not do

G7 made the bounded planner more deliberate and more explainable.
It did **not** solve every remaining limitation.

Important non-landed items:

- no project-wide recursive closure
- no whole-program symbolic reasoning
- no broad Tier 3 companion/fallback exploration in runtime behavior
- no exhaustive selected-vs-pruned proof trace in witness output
- no attempt to generalize the full PDG/progression surface equally across all languages

In other words, G7 improved **choice quality and explanation quality inside the bounded model**.
It did not change the model into something fundamentally unbounded.

---

## Before / after summary

The shortest honest summary of G7 is:

### Before G7

- bridge candidate choice still leaned too much on coarse kind/name/position heuristics
- scope flags existed but explanation scope was still bundled too tightly with generic selection
- Ruby bounded fallback behavior was still too easy to flatten into generic `require_relative` competition
- witnesses could show selected files, but not the minimal selected-vs-pruned reason

### After G7

- bridge candidate choice is score-profile-based and visible in output
- selected/pruned competition is reviewable in JSON/YAML
- explanation scope is intentionally separated from cache/local DFG responsibilities
- one real Rust and one real Ruby bounded competition case improved
- witness output can now say, minimally, why the chosen bridge beat a ranked-out alternative

---

## Recommended next-stage candidates

G7 leaves the project in a much better place, but it also makes the next bottlenecks clearer.
The best next-stage candidates are probably:

## 1. Make evidence collection more semantic and less name-driven

The current scoring model is good enough to review and regress, but several evidence fields are still fed by relatively shallow heuristics.

Best next step:

- strengthen how `return_flow`, `assigned_result`, and `alias_chain` are derived
- reduce remaining reliance on naming hints as a stand-in for semantics
- improve cross-side competition where multiple candidates have the same lane but different structural support

Why this is next:

- G7 built the score surface
- the next real leverage is improving the evidence feeding that surface

## 2. Land a true runtime lane for narrow fallback / module companion selection

G7 defined the schema/policy vocabulary for fallback lanes more clearly than the runtime currently exploits.

Best next step:

- make Tier 3 / narrow fallback runtime selection more explicit
- let selected/pruned fallback candidates carry the same reviewable surface consistently
- improve Ruby companion-style cases without reopening broad scope growth

Why this is next:

- the docs/schema now name the fallback shape clearly
- runtime behavior is still more limited than that vocabulary suggests

## 3. Extend witness explanation beyond one minimal winning basis

The current witness explanation is intentionally small and appropriate.
But it is probably still the thinnest layer in the end-to-end story.

Best next step:

- optionally include the first two decisive ranking dimensions, not just one
- distinguish ranked-out vs budget-pruned explanations more explicitly in witness text
- expose selected-vs-pruned explanation for more than one bridge file on the path when useful

Why this is next:

- G7 proved the surface is useful
- the remaining work is refinement, not invention from scratch

## 4. Expand the fixed eval set around bridge competition patterns

The new regressions are good, but the remaining risk is still mostly in unseen competition shapes.

Best next step:

- add more cross-side competition fixtures
- add more Ruby mixed alias / return / helper noise fixtures
- add more budget-prune cases that ensure scope split and witness explanation stay aligned

Why this is next:

- scoring systems regress at the edges first
- the project now has a good schema for expressing those edges

## 5. Keep resisting premature project-wide widening

This is less a feature than a strategic recommendation.
G7 strongly suggests that several remaining pain points are still ranking/explanation problems, not raw scope-size problems.

Best next step:

- only widen scope when a fixed eval case proves the bounded model is truly missing necessary context
- otherwise prefer better evidence, better ranking, and better explanation over broader selection

Why this is next:

- G7’s wins came from making bounded choices better, not from making the planner indiscriminately larger

---

## Closing note

G7 is a meaningful maturity step for the bounded slice planner.

It does not make dimpact “done” in multi-file PDG / propagation.
What it does do is move the project from:

- hidden/heuristic bridge choice
- partially bundled scope meaning
- weak explanation of selected-vs-pruned outcomes

to something much more sustainable:

- score-based bridge comparison
- explicit scope responsibilities
- bounded Ruby fallback tuning
- witness-level minimal choice explanation
- fixture-backed non-regression on the new planner surfaces

That is a good place to stop G7 and choose the next step deliberately rather than widening the planner blindly.