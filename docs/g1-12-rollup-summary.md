# G1 rollup: GitNexus-inspired impact summary rollout

## What landed

This document closes out the G1 summary-layer work for `dimpact`.

The rollout was intentionally split into small PRs so each step could stay reviewable and independently reversible.

### Merged PRs

- `#425` docs: prioritize GitNexus summary candidates
- `#426` docs: define by-depth summary schema
- `#427` feat(impact): add by-depth summary
- `#428` feat(impact): add initial risk summary
- `#429` test: verify impact risk summary outputs
- `#430` docs: decide affected-processes approach
- `#431` docs: defer affected-processes implementation
- `#432` docs: decide affected-modules approach
- `#433` feat: add affected modules impact summary
- `#434` docs: explain impact summary output

## User-facing result

`impact` JSON/YAML output now includes a summary-oriented triage layer before the full symbol/edge detail.

Current summary fields:

- `summary.by_depth`
  - separates direct (`depth=1`) from transitive (`depth>=2`) impact
- `summary.risk`
  - provides an initial severity hint based on direct/transitive hits and output size
- `summary.affected_modules`
  - path-based lightweight grouping to help readers decide which directories/modules to inspect next

These summary fields are emitted for normal output and also nest under `impacts[].output.summary` in `--per-seed` mode.

## Example shape

```json
{
  "summary": {
    "by_depth": [
      { "depth": 1, "symbol_count": 2, "file_count": 1 },
      { "depth": 2, "symbol_count": 4, "file_count": 3 }
    ],
    "risk": {
      "level": "medium",
      "direct_hits": 2,
      "transitive_hits": 4,
      "impacted_files": 3,
      "impacted_symbols": 6
    },
    "affected_modules": [
      { "module": "src", "symbol_count": 5, "file_count": 2 },
      { "module": "tests", "symbol_count": 1, "file_count": 1 }
    ]
  }
}
```

## Known constraints

### `affected_processes` is intentionally deferred

G1 explicitly does **not** ship `affected_processes` yet.

Reason:
- process grouping is more repo-specific and heuristic-sensitive than `by_depth`, `risk`, or path-based `affected_modules`
- a wrong process label is more misleading than a slightly coarse count-based summary
- the current graph/output model does not yet carry stable entrypoint/process metadata suitable for default-on output

Current status:
- feasibility and approach were documented in `docs/g1-7-affected-processes-approach.md`
- the actual implementation was deferred in `docs/g1-8-affected-processes-defer.md`

### `affected_modules` is intentionally lightweight

The current implementation is path-based, not community-detection-based.

That is deliberate:
- low implementation risk
- stable and explainable output
- easy to validate in CLI tests

If a future phase needs richer grouping, import/namespace-aware grouping or graph/community-based grouping can be added later.

## Why this closes G1

G1 was about importing the most practical GitNexus ideas into `dimpact` without replacing the core impact engine.

That goal is now met:
- `by_depth` adds direct vs transitive structure
- `risk` adds first-pass prioritization
- `affected_modules` adds a lightweight grouping layer
- docs explain how to read the new output
- `affected_processes` has an explicit defer decision instead of being left ambiguous

So the remaining work after G1 is evolutionary, not foundational.
