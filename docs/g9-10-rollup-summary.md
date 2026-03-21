# G9 rollup: evidence normalization, suppressing signals, stronger Rust/Ruby bounded selection, and losing-side witness reasons

## What landed

This document closes out the main G9 work around evidence normalization for bounded slice selection.

G9 was not about making the planner broader.
It was about making the planner / fallback / witness surfaces **mean the same thing when they say “evidence”**.

The practical goal was:

**normalize evidence into reviewable categories, let suppressing signals affect planner choice, strengthen one real Rust tie-break and one real Ruby narrow-fallback case, expose a compact losing-side reason in witness output, and lock the resulting precision gains without scope widening.**

In practice, G9 landed in ten layers:

- first, inventory where G8 evidence was actually being used and document the planner / fallback / witness drift
- next, define the normalized evidence contract (`primary / support / fallback / negative`)
- then, let planner scoring use negative / suppressing evidence and fix one real noisy Rust misselection
- next, strengthen Rust same-kind competition with a stronger semantic tie-break
- then, strengthen Ruby true narrow fallback so generic dynamic-send runtime noise is filtered out
- next, add a compact losing-side reason to witness selected-vs-pruned explanations
- then, extend the fixed eval set with evidence-conflict cases
- next, add regressions that prove the precision gains are not just hidden scope widening
- then, update the public READMEs so the mental model matches the runtime
- finally, summarize the merged G9 surface and identify the next-phase candidates

## Merged PRs

- `#506` docs: add G9 evidence usage gap memo
- `#507` docs: define G9 evidence normalization rules
- `#508` planner: penalize noisy return helper candidates
- `#509` planner: strengthen rust semantic tie-breaks
- `#510` planner: tighten ruby dynamic fallback matching
- `#511` impact: add losing-side witness reasons
- `#512` docs: extend eval set with evidence conflicts
- `#513` tests: lock precision gains against scope widening
- `#514` docs: explain evidence normalization and fallback

---

## What changed in practice

## 1. G9 turned “evidence” from a loose umbrella word into a normalized contract

G8 had already improved bounded selection, but it still used the word `evidence` for several different things at once:

- direct continuity facts
- lexical/path proxies
- bounded fallback admission facts
- compact winner-only witness metadata

`#506` and `#507` fixed the design side of that drift.

The important conceptual change is:

- `source_kind` and `lane` remain ranking dimensions
- but evidence itself is now easier to reason about in four categories:
  - `primary`
  - `support`
  - `fallback`
  - `negative`

That matters because the planner, Ruby fallback runtime, and witness explanation no longer need to pretend that all score inputs have the same role.

Relevant docs:

- `docs/g9-1-g8-evidence-usage-inventory-and-gap-memo.md`
- `docs/g9-2-evidence-normalization-rules.md`
- `docs/g9-2-evidence-normalization-rules.json`

## 2. Planner scoring can now use suppressing evidence instead of only positive counts

Before G9, bounded bridge comparison was still mostly additive:

- stronger source/lane
- more primary evidence
- more secondary evidence
- later call position

That made noisy helper-style candidates too easy to keep competitive if they happened to have a plausible name or later callsite.

`#508` changed that by introducing planner-visible negative evidence.

The first concrete runtime use is `negative_evidence_kinds = [noisy_return_hint]`.
That lets the planner say something more precise than:

> both candidates looked similar, so later position or lexical hint broke the tie

It can now say:

> the helper candidate stayed ranked out because it carried explicit suppressing evidence

This is the first G9 proof that “better precision” can come from **penalizing the wrong candidate**, not only from adding more positive facts to the winner.

## 3. Rust same-kind competition is no longer forced to fall back to shallow tie-breaks

One of the clearest G8/G9 gaps was that two Rust candidates could share the same evidence kind family while still differing materially in semantic strength.

Before `#509`, that often collapsed too quickly to:

- secondary evidence count
- call position
- lexical tie-break

`#509` added `semantic_support_rank` so the planner can distinguish:

- two candidates with the same top-level evidence kinds
- but different strength of semantic aggregation underneath

This matters because the planner can now prefer the stronger Rust continuation for a semantic reason **before** falling back to callsite or lexical ordering.

In practical terms, G9 changed one class of answer from:

> these candidates look the same, so the later one won

to:

> these candidates share the same evidence kind family, but one has stronger semantic support

That is exactly the kind of same-kind tie-break improvement G9 was meant to deliver.

## 4. Ruby true narrow fallback is stronger without becoming broader

Ruby was the most important place to prove that fallback can get smarter without turning into broad companion discovery.

`#510` tightened the true narrow-fallback runtime for dynamic-send-heavy cases.

The important runtime change is:

- generic dynamic runtime files are no longer supposed to survive “just in case”
- a runtime companion should usually remain only when it matches a concrete target family or bounded runtime fact

That means G9 improved Ruby fallback in the right direction:

- stronger bounded admission
- less generic noise
- no silent scope broadening

This is strategically important.
G9 did **not** make Ruby fallback more permissive.
It made it more selective and therefore more trustworthy.

## 5. Witness output can now explain a loser, not only a winner

G8 witness improvements were useful but still strongly winner-oriented.
The compact explanation could tell you:

- which ranking basis won
- which winning evidence/support appeared on the selected side

But it could not say much about why the loser remained ranked out.

`#511` added the first compact losing-side explanation surface:

- `losing_side_reason`

The current surface is intentionally small, but it already matters in practice because it can carry things like:

- `negative_evidence=noisy_return_hint`
- fallback-only loser context
- weaker `edge_certainty=dynamic_fallback`

This is the clearest G9 witness improvement.
The system can now answer not only:

> why did this candidate win?

but also, in a compact form:

> why did the loser stay a loser?

## 6. G9 now has a fixed evaluation set for evidence-conflict behavior

`#512` matters because it turned the new G9 surfaces into an explicit regression contract instead of leaving them spread across isolated fixtures.

The fixed set now locks seven evidence-driven cases across Rust and Ruby, including:

- semantic evidence beating positional noise
- negative/suppressing evidence keeping noisy helpers ranked out
- same-kind Rust competition decided by `semantic_support_rank`
- Ruby true narrow fallback materialization
- Ruby dynamic runtime filtering by literal target family
- winning-side plus losing-side witness explanation
- CLI selected/pruned witness stability under Ruby competition

That gives G9 a stable answer to:

- which evidence-conflict surfaces are intentionally improved
- which witness/pruned diagnostics must remain readable
- how to detect regressions that masquerade as “smarter planning” while really broadening scope

## 7. Regression coverage now proves the precision gains are not scope widening in disguise

`#513` is one of the most strategically important G9 changes.

It explicitly locks that recent precision wins do **not** come from widening explanation scope.

The strengthened CLI regressions now assert things like:

- ranked-out Rust losers stay out of `selected_files_on_path`
- generic Ruby runtime noise does not leak into `impacted_witnesses`
- filtered fallback noise does not reappear in explanation context even if it was nearby in the repo

That is the right proof obligation for this phase.
If a planner improvement only works because more files quietly enter scope, it is not evidence normalization — it is just hidden scope growth.

G9 now has stronger protection against that failure mode.

## 8. The README guidance now describes the actual G9 planner/fallback/witness model

`#514` updated both README variants so the public mental model now matches the merged runtime more closely.

The important documentation shifts are:

- `source_kind` / `lane` are described as ranking dimensions, not evidence themselves
- evidence is explained in the normalized categories
  - `primary`
  - `support`
  - `fallback`
  - `negative`
- witness output is described as carrying both winning-side and compact losing-side explanation
- Ruby true narrow fallback is described as bounded admission, not broad rescue

This matters because by the end of G9, the old docs would have been misleading in two ways:

- they would have under-described the new suppressing / losing-side surfaces
- they would have made Ruby fallback sound broader than it actually should be

---

## Improvement cases G9 materially helped

The clearest practical wins from G9 are:

### 1. Rust helper noise can now lose for an explicit suppressing reason

Improved from:

- helper-ish candidates sometimes remaining too competitive because the planner mostly compared positive evidence and late position

To:

- a noisy return helper remaining ranked out because it carries explicit negative evidence

Locked surface:

- pruned `negative_evidence_kinds = [noisy_return_hint]`
- witness `losing_side_reason = negative_evidence=noisy_return_hint`
- compact explanation slice still excludes the helper file itself

### 2. Same-kind Rust candidates can now be separated by stronger semantic support

Improved from:

- same-family candidates collapsing too quickly to later callsite or lexical order

To:

- a stronger Rust continuation winning on `semantic_support_rank`

Locked surface:

- selected `selected_better_by = semantic_support_rank`
- witness summary explicitly mentions stronger semantic support

### 3. Ruby dynamic-send fallback noise is filtered before it pollutes selection/witness surfaces

Improved from:

- generic runtime files being too easy to keep around when dynamic-send evidence existed nearby

To:

- family-specific runtime candidates surviving while generic runtime noise is filtered out before materialization/explanation

Locked surface:

- filtered file absent from `impacted_files`
- filtered file absent from `impacted_witnesses`
- filtered file absent from witness `selected_files_on_path` and slice-selection explanation context

### 4. Witness explanations now expose a minimal losing-side story

Improved from:

- winner-only explanations with little visibility into why a loser stayed ranked out

To:

- compact losing-side explanation for helper noise, fallback-only losers, or weaker certainty

This is the first phase where witness output can explain both:

- what won
- and why the loser did not earn scope

---

## What G9 did **not** do

G9 improved evidence normalization materially, but it still stayed within a bounded scope.
It did **not** attempt to do the following:

- turn the planner into a project-wide PDG / symbolic executor
- rewrite all legacy evidence kinds into brand-new runtime enums in one pass
- make witness output a full proof trace or multi-path comparison report
- make Ruby fallback broad or recursive
- solve evidence normalization for every language beyond the current Rust/Ruby-heavy surfaces

That boundary is intentional.
G9 was a normalization and bounded-selection phase, not a whole-architecture rewrite.

---

## Main lessons from G9

## 1. Better bounded selection often comes from separating roles, not adding more candidates

The most important design lesson is that many planner problems were caused by conflating:

- continuity facts
- support/provenance
- bounded fallback admission
- suppressing signals

Once those roles are made more explicit, the planner gets better without needing to broaden the slice.

## 2. Negative evidence is a first-class precision tool

G9 shows that suppressing signals are not just optional annotations.
They are part of the actual selection contract.

That matters because not every planner improvement should come from “finding more reasons to include.”
Sometimes the correct improvement is “finding a principled reason to keep the wrong candidate out.”

## 3. Fallback quality is mostly about admission discipline

The Ruby work reinforces the right fallback philosophy:

- fallback should be narrow
- runtime admission should be explainable
- generic dynamic noise should be filtered, not retained broadly

This is the opposite of a “keep more files just in case” strategy.

## 4. Witness quality improves when it mirrors ranking roles, not raw internals

G9 did not dump the full planner state into witness output.
Instead, it added the minimum extra surface that maps to the normalized roles:

- winning primary/support
- losing-side reason

That is a good pattern to keep.
It gives users a real explanation without forcing witness output to become a full debug trace.

---

## Next-stage candidates after G9

The remaining items are now much clearer than they were at the start of the phase.
The best next-step candidates are:

## Candidate 1. Finish runtime-side normalization of legacy evidence kinds

Current G9 docs define the normalized categories, but several runtime evidence kinds are still only partially normalized.
The most obvious follow-up is to finish splitting legacy mixed-role kinds such as:

- `return_flow`
- `assigned_result`
- `alias_chain`
- `name_path_hint`

into cleaner runtime roles that more faithfully distinguish:

- direct continuity fact
- weak positive support
- explicit suppressing/noise signal

Why it matters:

- it would reduce the remaining gap between the normalized docs contract and the raw planner metadata
- it would make selected/pruned evidence names easier to interpret at face value

## Candidate 2. Unify weak Ruby require-relative continuation and true narrow fallback on one explicit comparison surface

G9 improved true narrow fallback, but Ruby still has two related continuation shapes:

- weak graph-side `require_relative` continuation
- true narrow fallback with bounded runtime admission

A strong next step would be to make their relationship more explicit in runtime comparison and witness output.

Why it matters:

- it would make Ruby selected-vs-pruned explanations easier to read
- it would reduce the remaining graph-vs-fallback vocabulary mismatch

## Candidate 3. Add richer witness diff without turning witness into a proof trace

G9 added `losing_side_reason`, which is the right first step.
A follow-up could safely add a little more, such as:

- selected/pruned fallback deltas
- selected/pruned secondary-evidence deltas when they are meaningful
- clearer formatting for certainty/support differences

Why it matters:

- it would make explanations slightly less lossy while preserving the current compact design
- it would keep the witness surface aligned with the normalized evidence roles

## Candidate 4. Automate the fixed eval-set reporting path

G9 now has a stronger fixed eval set, but it is still mostly a documentation and fixture contract.
A useful follow-up would be to add a lightweight runner/report surface that summarizes:

- which fixed cases passed
- what selected/pruned surfaces changed
- whether scope-guard assertions still hold

Why it matters:

- it would make evidence-normalization regressions easier to spot during iteration
- it would reduce the chance of silent drift between docs, tests, and runtime behavior

## Candidate 5. Expand evidence-conflict coverage carefully, language by language

Rust and Ruby now have clearer bounded-evidence surfaces than before.
A reasonable next phase could extend the same discipline to adjacent coverage areas, but only when each language has:

- a clear bounded-selection story
- concrete misselection fixtures
- a non-widening regression guard

Why it matters:

- it preserves the strongest G9 lesson: do not confuse better precision with broader scope
- it avoids prematurely generalizing a Rust/Ruby-heavy model into under-modeled languages

---

## Closing summary

G9 succeeded because it did not try to make dimpact universally smarter all at once.
It instead made the current bounded planner **more internally coherent**.

After the merged G9 stack, dimpact is better able to say:

- what kind of evidence it is using
- why one candidate beat another
- why a loser stayed ranked out
- and why a fallback candidate was allowed to exist at all

while still keeping the core bounded-slice rule intact:

**precision should improve because the planner chooses better and explains better, not because explanation scope quietly grows.**
