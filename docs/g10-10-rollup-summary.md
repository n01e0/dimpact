# G10 rollup: evidence-budgeted admission, family-aware suppression, compact loser bookkeeping, and non-widening precision gains

## What landed

This document closes out the main G10 work around evidence-budgeted admission for the bounded slice planner.

G9 had already normalized the language around evidence.
G10 was about pushing that normalized evidence **earlier into planner control**.

The practical goal was:

**let normalized evidence influence whether a candidate should exist at all, make duplicate/sibling/fallback losers explicit before normal ranking, tighten one real Rust over-selection case and one real Ruby fallback-noise case, expose compact dropped-before-admit reasons, and prove that the resulting precision gains do not come from scope widening.**

In practice, G10 landed in ten layers:

- first, inventory where G9 still used post-hoc / fixed-count admission, pruning, and stop behavior
- next, define the evidence-family schema that planner control can read
- then, implement suppress-before-admit so obviously weak candidates are pruned before normal side ranking
- next, suppress one real Rust same-family over-selection case
- then, refine Ruby narrow fallback admission so a dynamic-heavy noisy runtime case is filtered earlier
- next, add compact dropped-before-admit reasons to witness / slice summary surfaces
- then, extend the fixed eval set with admission-conflict and budget-exhaustion cases
- next, add regressions that prove the G10 precision wins are not hidden scope widening
- then, update the public READMEs so the mental model matches the new bounded-admission runtime
- finally, summarize the merged G10 surface and identify the next-phase candidates

## Merged PRs

- `#516` docs: add G10 evidence-budget design memo
- `#517` docs: define G10 evidence family schema
- `#518` planner: add suppress-before-admit planner pruning
- `#519` planner: suppress weaker Rust sibling candidates
- `#520` planner: refine Ruby fallback admission for runtime families
- `#521` impact: add compact dropped-before-admit explanations
- `#522` docs: extend eval set for admission conflicts
- `#523` tests: add non-widening regression coverage
- `#524` docs: explain evidence-budgeted admission

---

## What changed in practice

## 1. G10 turned normalized evidence into planner control input

G9 made evidence easier to talk about.
G10 made evidence matter earlier.

Before G10, the bounded slice planner still behaved mostly like this:

- materialize graph-second-hop candidates structurally
- assign lane / score metadata
- compare candidates
- keep only the top few
- explain the loser afterward if possible

That meant several important decisions still happened too late:

- weak helpers could enter the ranking pool even when they were obvious noise
- same-path / same-family losers could disappear without an explicit policy name
- fixed file-count caps still did more work than family-aware admission or suppression

`#516` and `#517` made that gap explicit in docs.
The key conceptual shift is:

- normalized evidence is no longer only ranking metadata
- it is now also the intended control surface for:
  - admission
  - suppression
  - budget
  - stop

This is the core G10 change.
The planner is still bounded, but the reason it keeps a candidate has become more deliberate.

Relevant docs:

- `docs/g10-1-g9-admission-pruning-stop-inventory-and-evidence-budget-memo.md`
- `docs/g10-2-evidence-family-admission-suppression-budget-schema.md`
- `docs/g10-2-evidence-family-admission-suppression-budget-schema.json`

## 2. G10 added an explicit suppress-before-admit stage

`#518` is the runtime heart of G10.

The planner can now prune some candidates **before** the normal side-local ranked selection instead of waiting for a later ranked-out outcome.
That matters because several wrong candidates were never interesting enough to deserve a real slot contest.

The first merged runtime uses are intentionally narrow but important:

- helper/noise candidates that rely on weak lexical or positional support can stop at `suppressed_before_admit`
- weak Ruby `require_relative` continuation candidates can be dropped earlier when the same boundary side already has a stronger semantic-ready candidate
- weaker same-path replacements can be recorded explicitly instead of silently disappearing

This is the first G10 proof that planner precision can improve by saying:

> this candidate should never become a real explanation contender

instead of only saying:

> it entered the pool, then lost later

That change sounds small, but it is the cleanest runtime expression of evidence-budgeted admission.

## 3. Rust same-family sibling over-selection is now explicitly suppressed

`#519` took the new suppress-before-admit machinery and used it on one real Rust over-selection pattern.

Before this change, a later Rust sibling could remain competitive mostly because it lived in the same broad continuation family and still looked plausible under count-based comparison.
In practice, that meant same-family variation could consume attention or slots that should have been reserved for the stronger representative.

After `#519`:

- same-family Rust return/alias siblings can be compared earlier within the boundary side
- the weaker sibling can stop at `weaker_same_family_sibling`
- the stronger semantic representative remains selected without widening the explanation slice

This matters because G10 is not only about helper noise.
It is also about **family-local representative selection**.

The practical improvement is that the planner can now say:

> this is not just a loser — it is a weaker sibling of an already-good representative

That is a much better bounded-selection story than generic ranked-out bookkeeping.

## 4. Ruby fallback admission is stricter without becoming broader

`#520` is the Ruby proof point for G10.

G9 had already improved narrow fallback.
G10 tightened it one more step by using stronger boundary-side runtime-family evidence to decide which fallback candidates should survive admission.

The important runtime shift is:

- a dynamic-heavy runtime candidate is no longer kept just because its path or target prefix feels vaguely related
- when the boundary names a concrete runtime constant family, fallback admission now expects candidates to line up with that family
- unrelated runtime noise should be filtered earlier, before it pollutes selected/pruned witness surfaces

That is the correct direction for Ruby fallback.
G10 did **not** make fallback more permissive.
It made fallback more disciplined.

This is strategically important for the whole bounded-slice model:

- better Ruby precision should come from tighter bounded admission
- not from retaining more runtime files “just in case”

## 5. Dropped-before-admit losers now leave a compact public trace

`#521` connected the runtime-side G10 changes back to explanation surfaces.

Before this, the planner could start suppressing candidates earlier, but the public JSON / witness story still leaned too heavily toward ranked-out losers.
That would have made G10 harder to review because some of the new precision work would have happened “off to the side.”

After `#521`, `slice_selection.pruned_candidates[*]` and witness selected-vs-pruned surfaces can carry compact dropped-before-admit labels such as:

- `suppressed_before_admit=helper_noise_suppressor`
- `suppressed_before_admit=fallback_only_suppressor`
- `suppressed_before_admit=weaker_same_path_duplicate`

This is the right shape for G10 witness output.
It does **not** dump the full planner state.
It just preserves the minimum public explanation needed to answer:

- why was this candidate never admitted?
- why did a duplicate or sibling not widen scope?

That is exactly the compact surface G10 was supposed to add.

## 6. The fixed eval set now covers admission conflict and budget exhaustion explicitly

`#522` matters because G10 needed a stronger regression contract than G9.
It was no longer enough to lock only selected-vs-ranked-out behavior.

The fixed evidence-driven eval set now explicitly includes:

- suppress-before-admit helper cases
- Rust same-family sibling suppression
- Ruby fallback admission conflict cases
- same-path duplicate bookkeeping
- seed-wide `bridge_budget_exhausted` cases distinct from side-local losers

That gives G10 a much better answer to:

- which conflicts should stop before ranking
- which losers should remain visible as family/duplicate/budget diagnostics
- how to tell a real bounded-admission improvement from accidental scope growth

This is one of the key reasons G10 is reviewable as a coherent phase instead of a collection of isolated tweaks.

## 7. Regression coverage now proves the new precision gains stay compact

`#523` is the strategic safety net for G10.

It extends the G9 non-widening idea to the new G10 prune surfaces.
The important thing it locks is **not** only that the right candidate wins.
It also locks that the new loser bookkeeping does not silently widen explanation scope.

The strengthened regressions now assert things like:

- weaker same-family Rust losers stay out of `summary.slice_selection.files`
- those losers also stay out of witness `selected_files_on_path`
- same-path duplicate/runtime-family loser bookkeeping in Ruby does not widen witness scope
- `build_selected_vs_pruned_reasons()` ignores same-path duplicates and `bridge_budget_exhausted` cases for witness explanation scope

This is exactly the right proof obligation for G10.
If the new precision only works because more files quietly entered the explanation slice, then the planner did not actually get better at admission.
It just got broader.

G10 now has direct protection against that failure mode.

## 8. The public README guidance now describes the actual G10 planner model

`#524` updated both README variants so the public mental model matches the merged runtime more closely.

The important documentation shifts are:

- evidence-driven selection is now described together with evidence-budgeted admission
- the main new prune labels are documented in a user-readable way:
  - `suppressed_before_admit`
  - `weaker_same_family_sibling`
  - `weaker_same_path_duplicate`
  - `bridge_budget_exhausted`
- `pruned_candidates[*].compact_explanation` is documented as the place to look for dropped-before-admit / duplicate / sibling loser labels
- the docs now state more explicitly that these precision gains are bounded, family-aware admission changes — not hidden scope widening

That matters because by the end of G10, the old docs would have under-described the planner in exactly the place that changed most:

- not just how candidates are scored
- but how they are filtered before they can widen the slice at all

---

## Improvement cases G10 materially helped

The clearest practical wins from G10 are:

### 1. Obvious helper noise can now lose before ranked selection

Improved from:

- weak helper-like candidates entering the real side-local ranking pool and only losing afterward

To:

- weak helper-like candidates stopping at `suppressed_before_admit`

Locked surface:

- `pruned_candidates[*].prune_reason = suppressed_before_admit`
- `compact_explanation = suppressed_before_admit=helper_noise_suppressor`
- witness scope remains compact instead of gaining a new explanation file

### 2. Rust same-family sibling competition is now explained as family conflict, not generic ranking loss

Improved from:

- later Rust siblings remaining too competitive inside the same continuation family

To:

- weaker siblings stopping at `weaker_same_family_sibling`

Locked surface:

- selected leaf remains the semantic representative
- pruned sibling is recorded explicitly as same-family loser
- witness `selected_files_on_path` remains pinned to the selected explanation slice

### 3. Ruby dynamic-heavy fallback noise is filtered by runtime-family admission

Improved from:

- runtime-like candidates surviving fallback admission mostly because they looked vaguely route-ish or path-adjacent

To:

- unrelated runtime noise being filtered when it does not match the boundary-side runtime family

Locked surface:

- noisy runtime file absent from `impacted_files`
- noisy runtime file absent from selected explanation scope
- selected runtime file remains, and same-path loser bookkeeping stays compact

### 4. Duplicate and budget losers are now easier to distinguish

Improved from:

- same-path conflicts, side-local losers, and seed-wide budget losers being too easy to mentally blur together

To:

- same-path duplicates staying visible as duplicate suppression
- same-family losers staying visible as sibling suppression
- seed-wide cap losers staying visible as `bridge_budget_exhausted`

This is an important G10 win because it makes bounded planning easier to inspect without broadening the planner itself.

### 5. Dropped-before-admit explanation now has a public minimal surface

Improved from:

- early-pruned candidates being hard to review from output alone

To:

- compact explanation labels showing why a loser never earned admission

This is the missing public-facing half of the runtime G10 work.
Without it, suppress-before-admit would have been real but too invisible.

---

## What G10 did **not** do

G10 materially improved admission discipline, but it still stayed inside a bounded planner architecture.
It did **not** attempt to do the following:

- turn the bounded slice planner into family-budgeted whole-project closure
- replace every raw evidence kind with a fully runtime-enforced evidence-family object model
- make witness output a full proof trace of every admission / suppression / budget decision
- add recursive stop expansion based on evidence families across the whole repo
- generalize the new planner discipline to every language beyond the current Rust/Ruby-heavy PDG surfaces

That boundary is intentional.
G10 was an **admission-control phase**, not a whole-planner rewrite.

---

## Main lessons from G10

## 1. Better bounded precision often comes from refusing to admit the wrong candidate

This is the biggest design lesson.

Many pre-G10 problems were still framed as:

- compare more carefully
- keep better metadata
- explain the loser afterward

G10 shows that the stronger move is often earlier:

- do not let the wrong candidate become a real contender

That is exactly what evidence-budgeted admission is for.

## 2. Duplicate and sibling bookkeeping should be explicit policy, not accidental map behavior

Before G10, same-path suppression and same-family competition already existed informally in planner behavior.
But they were not first-class policy concepts.

G10 shows that once they are made explicit:

- the runtime gets easier to reason about
- witness surfaces get easier to debug
- non-widening regressions become much easier to write

## 3. Budget is more useful when it distinguishes local losers from global losers

`bridge_budget_exhausted` matters because not every loser means the same thing.
Some candidates are weak and should never have been admitted.
Others are locally good but lose to the final bounded budget.

That distinction is important because it tells you whether the next improvement should target:

- admission discipline
- family-local representative selection
- or final seed-wide budget policy

## 4. Witness quality improves when compact explanation follows planner policy names

G10 did not try to mirror every internal planner detail.
Instead, it added compact labels that map to policy-level concepts:

- helper suppressor
- fallback-only suppressor
- same-path duplicate
- same-family sibling
- bridge budget exhaustion

That is a good pattern to keep.
It gives the user a meaningful explanation while keeping the witness surface compact.

## 5. Non-widening regression tests are part of the planner design, not just test polish

G10 reinforced a lesson already visible in G9:

- a planner change is not really “precision improving” unless it also proves that explanation scope stayed bounded for the right reason

In other words, non-widening regressions are not optional after-the-fact cleanup.
They are part of the actual admission contract.

---

## Next-stage candidates after G10

The remaining items are now much clearer than they were at the start of the phase.
The best next-step candidates are:

## Candidate 1. Make family-local budget real in runtime, not only in schema / narrow suppression

G10 introduced the schema and the first explicit family-local suppressions.
But the runtime still does not fully perform representative selection and budget in a unified family-local stage before every final per-seed decision.

A strong next step would be to make the planner explicitly do:

- same-path merge/suppression
- same-family representative selection
- family-local budget
- then final per-seed budget

Why it matters:

- it would reduce remaining raw-count influence
- it would make `bridge_budget_exhausted` more semantically meaningful
- it would finish the core runtime half of the G10-2 design

## Candidate 2. Add evidence-aware stop rules, not only admission-aware pruning

G10 improved admission and loser bookkeeping more than stop behavior.
The planner is still mostly bounded by structural and numeric caps.

A good next step would be to make stop rules more explicit, for example:

- stop when the relevant family representative is already present
- stop when only suppressing leftovers remain
- stop when fallback provenance contracts can no longer be satisfied

Why it matters:

- it would complete the `admission / pruning / stop` triad from G10-1
- it would make boundedness easier to explain as policy, not just constant values

## Candidate 3. Promote the evidence-family profile from docs contract to stable runtime/debug schema

G10-2 defined the profile conceptually, but the runtime still exposes that logic through existing selected/pruned scoring metadata plus compact explanation labels.
A follow-up could introduce a more explicit runtime/debug representation of candidate admission profiles.

Why it matters:

- it would shrink the remaining gap between docs and runtime control surfaces
- it would make family-aware planner debugging easier without requiring a full witness proof trace

## Candidate 4. Expand compact explanation carefully to cover family-budget / stop outcomes

G10 added dropped-before-admit labels successfully.
A reasonable next step would be to add a little more where it genuinely helps, such as:

- clearer `family_budget_exhausted=<family>` labels
- compact stop labels when the planner refuses to widen further
- more explicit selected-vs-pruned fallback family deltas when they are the real reason a candidate lost

Why it matters:

- it would make bounded-admission failures easier to review
- it would keep witness surfaces aligned with the next runtime policy layer

## Candidate 5. Grow evaluation/reporting around admission contracts, not only fixtures

G10 now has better fixed cases and non-widening regressions, but the evaluation/reporting path is still mostly spread across docs and individual tests.
A useful follow-up would be a lightweight report that summarizes:

- which admission-conflict cases passed
- which losers were dropped before admit vs budgeted out later
- whether witness scope stayed compact

Why it matters:

- it would make admission-policy drift easier to spot during iteration
- it would keep docs, tests, and runtime behavior aligned

## Candidate 6. Extend the discipline carefully beyond Rust/Ruby only when bounded contracts are concrete

Rust and Ruby now have clearer bounded-admission surfaces than before.
A future phase could extend the same discipline to adjacent languages or planner paths, but only when each area has:

- a clear bounded-selection story
- concrete misselection fixtures
- explicit non-widening guards
- compact loser bookkeeping that does not force full debug traces into witness output

Why it matters:

- it preserves the strongest G10 lesson: do not confuse better admission with broader scope
- it avoids prematurely generalizing a Rust/Ruby-heavy planner model into under-modeled areas

---

## Closing summary

G10 succeeded because it did not try to make dimpact globally broader.
It instead made the current bounded planner **more selective before ranking, more explicit about loser kinds, and more reviewable when it refused admission**.

After the merged G10 stack, dimpact is better able to say:

- which candidate should never have entered the pool
- when a loser was a duplicate vs a sibling vs a budget casualty
- why a Ruby fallback candidate was allowed or denied admission
- and why the explanation slice stayed compact even though more loser metadata is now visible

while keeping the core bounded-slice rule intact:

**precision should improve because the planner admits better, suppresses earlier, and explains loser kinds more clearly — not because explanation scope quietly grows.**
