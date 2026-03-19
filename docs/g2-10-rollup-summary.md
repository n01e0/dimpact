# G2 rollup: summary evaluation, calibration, and lightweight cleanup

## What landed

This document closes out the G2 follow-up work for the `impact` summary layer.

G1 introduced the summary fields.
G2 was about answering a harder question:
**are those fields actually useful for day-to-day triage, and if not, what should change without overcomplicating the model?**

The rollout stayed intentionally incremental so that evaluation, policy changes, fixture locks, and docs updates could each land in small reviewable PRs.

### Merged PRs

- `#436` docs: define G2 summary evaluation candidates
- `#437` docs: add G2 risk calibration baseline
- `#438` docs: fix G2 summary evaluation set
- `#439` feat: tighten high-risk threshold for transitive spread
- `#440` test: cover high-risk transitive spread regression
- `#441` docs: plan lightweight affected-modules cleanup
- `#442` feat: normalize affected module root labels
- `#443` docs: defer affected processes experimental rollout
- `#444` docs: clarify G2 impact summary guidance

## Evaluation result

G2 fixed the evaluation surface before changing behavior.

### 1. Summary evaluation now has explicit criteria

The project now has written criteria for judging whether the summary is useful:

- `by_depth` should help separate direct vs transitive spread
- `risk` should behave like a triage-priority hint, not a faux severity oracle
- `affected_modules` should help decide what to open next
- the summary should remain internally consistent with raw `impacted_symbols` / `impacted_files`

Relevant docs:

- `docs/g2-1-summary-eval-candidates.md`
- `docs/g2-2-risk-calibration-baseline.md`

### 2. A fixed comparison set now exists

G2 locked a compact evaluation set across:

- low floor / filtered-empty behavior
- medium chain behavior
- high fan-out behavior
- dynamic Python/Ruby boundary cases
- a heavy diff scale anchor

This matters because later `risk` / `affected_modules` changes can now be compared against stable snapshots instead of ad-hoc spot checks.

Relevant artifacts:

- `docs/g2-3-summary-eval-set.md`
- `docs/g2-3-summary-eval-set.json`
- `scripts/collect-summary-eval.py`

### 3. The main practical findings

From the fixed cases, G2 found two concrete problems worth fixing now.

#### 3.1 `risk` was a little late to escalate broad caller spread

The old rule waited until `direct_hits >= 1 && transitive_hits >= 4` before escalating to `high`.
That under-called cases where the change already had a direct caller hit and the caller-side spread was visibly thick.

G2 changed that threshold to:

- `direct_hits >= 1 && transitive_hits >= 3` → `high`

That is still explainable, still rule-based, and better aligned with the fixed triage cases.

#### 3.2 `affected_modules` mixed directory buckets with root file names

The old output could produce lists like:

- `alpha`
- `main.rs`
- `beta`

That was technically valid, but a bit noisy as a reading aid.
G2 kept the path-based grouping model, but normalized entry-like files so the display is more consistent:

- `src/main.rs` / `src/lib.rs` / `src/engine/mod.rs` → parent path
- repo-root entry-like files → `(root)`

This preserves the lightweight model while making the output less awkward to scan.

## Change intent

G2 did **not** try to turn the summary into a second full analysis engine.
The intent was narrower.

### `risk`

Goal:
- reduce obvious underestimation on caller-side spread
- keep the rule explainable and testable
- avoid moving into opaque weighted scoring

Net effect:
- `risk` remains a compact triage hint
- it is slightly more willing to say `high` once direct impact and transitive spread are both present

### `affected_modules`

Goal:
- preserve path-based grouping
- improve naming consistency where the old output was visibly noisy
- avoid namespace heuristics or graph clustering

Net effect:
- the grouping stays lightweight and cheap
- root entry areas are shown more clearly as `(root)`
- entry-like files in subdirectories read more like repo areas than literal filenames

### `affected_processes`

Goal:
- decide whether G2 should start an experimental rollout

Result:
- **no experimental rollout yet**
- design remains deferred until entrypoint heuristics, fixture ground truth, and scope limits are strong enough

This is deliberate.
A wrong process label is more misleading than a coarse module label.

## User-facing result after G2

The `impact` summary layer is now better grounded in actual usage:

- `by_depth` still answers “is this direct or transitive?”
- `risk` is documented and calibrated as triage priority
- `affected_modules` is easier to scan in root-entry cases
- README / README_ja now explain the intended reading order and current limits

In practice, the expected reading flow is:

1. check `summary.by_depth`
2. use `summary.risk` to decide how aggressively to triage
3. use `summary.affected_modules` to choose where to open code next
4. fall through to `impacted_symbols` / `edges` for the concrete graph

## Known limits

### `risk` is still rule-based and intentionally simple

Current `risk` is not a learned score and not a production-severity estimate.
It is a deliberately small heuristic.

Implications:

- some `medium` vs `high` boundary cases will remain arguable
- dynamic-language cases can still feel more expensive to inspect than the raw counts suggest
- module dispersion is not yet folded into `risk`

Relevant baseline doc:

- `docs/g2-2-risk-calibration-baseline.md`

### `affected_modules` is still path-based, not semantic clustering

Current grouping remains intentionally lightweight.
It does **not** attempt:

- namespace-aware labels like `crate::foo` or `pkg.foo`
- graph/community clustering
- repo-specific alias rules
- module-level re-scoring

That is a feature, not an omission-by-accident.
The current tradeoff favors explainability and stable tests over smarter-looking but harder-to-trust grouping.

Relevant design doc:

- `docs/g2-6-affected-modules-lightweight-plan.md`

### `affected_processes` remains deferred

G2 re-checked whether the next step should be an experimental `affected_processes` rollout.
The answer is still no.

Reason:

- entrypoint detection is still too language- and repo-dependent
- the current fixed evaluation set is not yet process-ground-truth oriented
- even as an experimental field, a wrong process label would mislead users too easily

Relevant docs:

- `docs/g1-7-affected-processes-approach.md`
- `docs/g1-8-affected-processes-defer.md`
- `docs/g2-8-affected-processes-decision.md`

## Why this closes G2

G2 was supposed to do more than “add one more field.”
It needed to prove that the summary layer could be evaluated, improved, and explained in a disciplined way.

That now exists:

- evaluation criteria are written down
- fixed comparison cases exist
- one real `risk` calibration change shipped with regression coverage
- one real `affected_modules` usability cleanup shipped with regression coverage
- docs now explain both the intended reading model and the current boundaries
- `affected_processes` has an explicit non-rollout decision rather than vague future intent

So after G2, the summary layer is no longer just “new output.”
It has a repeatable evaluation loop and a clearer operational story.
