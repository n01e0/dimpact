# G8-10 closeout note: no extra targeted Rust/Ruby patch needed

## Purpose

This note closes the final G8 cleanup task.
The question for G8-10 was simple:

- is there one more small Rust/Ruby runtime fix that must land before G8 can close cleanly?
- or are the remaining items already better treated as G9 work?

The answer after the merged G8 stack is:

**no additional targeted runtime patch is required to close G8 cleanly.**

---

## What was checked

The closeout decision is based on the merged G8 stack on `main`:

- `#496` evidence inventory / design memo
- `#497` evidence schema extension
- `#498` Rust `param_to_return_flow` improvement
- `#499` Ruby true narrow fallback runtime
- `#500` winning-evidence witness explanation
- `#501` fixed evidence-driven eval set
- `#502` precision regressions that lock non-widening behavior
- `#503` README / README_ja evidence-driven selection guidance
- `#504` G8 rollup summary

and especially on the fixed G8 evaluation surface documented in:

- `docs/g8-6-evidence-driven-eval-set.md`
- `docs/g8-9-rollup-summary.md`

---

## Why no extra patch is needed

## 1. The intended G8 improvement surfaces already landed

The phase goals were:

- strengthen scoring evidence without widening bounded slice scope
- improve at least one Rust misselection
- improve at least one Ruby dynamic-heavy / narrow-fallback case
- surface winning evidence in witness explanation

Those goals are already met on `main`.

Most concretely:

- Rust now has a real `param_to_return_flow` winner case
- Ruby now has a real `module_companion_fallback` / true narrow-fallback case
- witness output now carries `winning_primary_evidence_kinds` and `winning_support`

## 2. The fixed evaluation set already captures the intended G8 boundary

G8 now has a fixed evaluation set for the evidence-driven work.
That set locks:

- Rust semantic evidence beating helper noise
- Ruby true narrow fallback materialization
- winning-evidence witness explanation
- Ruby CLI selected/pruned witness surface

If an extra G8 cleanup patch were necessary, it should be justified by one of those fixed cases still failing or remaining ambiguous.
At closeout time, that is not the situation.

## 3. Precision improvements are already protected against accidental scope growth

The most important strategic risk after G8 was:

- stronger bridge/fallback logic silently widening explanation scope

That is already covered by the G8 regression additions.
The current regressions explicitly lock that:

- stronger winners remain selected
- weaker helpers remain ranked-out metadata when appropriate
- explanation-visible witness paths stay bounded

That means the most important “one more cleanup before closing” concern is already addressed.

## 4. The remaining items are next-phase problems, not phase-close blockers

The main unresolved items after G8 are still things like:

- making more evidence kinds semantic instead of lexical
- deciding whether additional Ruby fallback shapes are worth bounded runtime support
- refining witness explanation beyond one compact winning-evidence surface
- adding lightweight aggregated reporting for evidence-driven eval cases

Those are real follow-ups, but they are **G9 candidates**, not missing G8 completion criteria.

---

## What would have justified one more G8 runtime patch

A final Rust/Ruby patch would have been justified only if at least one of the following had remained true after the merged stack:

- the fixed G8 eval set still had a missing winner/pruned case
- the Ruby narrow-fallback path still failed to materialize in its bounded runtime case
- winning evidence/support still failed to appear in witness output
- stronger evidence still widened explanation scope instead of only improving selection quality

None of those conditions remain true in the merged G8 stack.

---

## Decision

G8 can close cleanly **without** an extra targeted Rust/Ruby runtime patch.

The correct closeout action is therefore:

- keep the merged G8 stack as the completed phase surface
- treat the remaining unresolved items as explicit G9 candidates
- avoid inventing a small late-phase patch that would broaden scope or dilute the phase boundary without fixing a proven G8 blocker

---

## Rule carried forward

If a future follow-up proposes another bounded-slice selection change, it should continue to prove two things together:

1. the selected winner improves for a stronger evidence reason
2. explanation scope does **not** broaden as a side effect

That remains the most important closing lesson from G8.