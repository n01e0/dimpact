# Strict-LSP bench guardrail thresholds

This note records `min-lsp-changed` / `min-lsp-impacted` guardrails and latest remeasurement snapshot.

## ALL58-1 remeasurement snapshot

- primary source: GitHub Actions `bench.yml` run `22776479200` (workflow_dispatch)
- fallback source: local rerun only for Python (CI python install check currently fails before benchmark execution)

| Language | Fixture | Current guardrail | Remeasured changed/impacted | Status | Source |
|---|---|---:|---:|---|---|
| TypeScript | `bench-fixtures/ts-heavy.diff` | `6 / 15` | `0 / 0` | FAIL (below guardrail) | CI run `22776479200` (`bench-typescript-strict-lsp`) |
| JavaScript | `bench-fixtures/js-heavy.diff` | `6 / 15` | `0 / 0` | FAIL (below guardrail) | CI run `22776479200` (`bench-javascript-strict-lsp`) |
| Ruby | `bench-fixtures/ruby-heavy.diff` | `3 / 5` | `N/A` | BLOCKED (`lsp initialize timeout or invalid response`) | CI run `22776479200` (`bench-ruby-strict-lsp`) |
| Go | `bench-fixtures/go-heavy.diff` | `6 / 15` | `10 / 31` | PASS | CI run `22776479200` (`bench-go-strict-lsp`) |
| Java | `bench-fixtures/java-heavy.diff` | `7 / 15` | `N/A` | BLOCKED (`lsp initialize timeout or invalid response`) | CI run `22776479200` (`bench-java-strict-lsp`) |
| Python | `bench-fixtures/python-heavy.diff` | `3 / 5` | `10 / 31` | PASS (fallback local measurement) | local rerun (`scripts/bench-impact-engines.sh --lang python`) |

### Notes from this snapshot
- Go/Python are currently above guardrails with margin.
- TypeScript/JavaScript currently report `0/0` on strict-LSP in CI and fail guardrails.
- Java/Ruby strict-LSP currently timed out during initialize in this snapshot, so no counts were produced.
- Python CI job did not reach benchmark execution because `pyright-langserver --help` exits non-zero in current environment; local rerun was used to keep a current measurement.

## Repro command templates

### TypeScript
```bash
scripts/bench-impact-engines.sh \
  --diff-file bench-fixtures/ts-heavy.diff \
  --runs 1 \
  --direction callers \
  --lang typescript \
  --min-lsp-changed 6 \
  --min-lsp-impacted 15
```

### JavaScript
```bash
scripts/bench-impact-engines.sh \
  --diff-file bench-fixtures/js-heavy.diff \
  --runs 1 \
  --direction callers \
  --lang javascript \
  --min-lsp-changed 6 \
  --min-lsp-impacted 15
```

### Ruby
```bash
scripts/bench-impact-engines.sh \
  --diff-file bench-fixtures/ruby-heavy.diff \
  --runs 1 \
  --direction callers \
  --lang ruby \
  --min-lsp-changed 3 \
  --min-lsp-impacted 5
```

### Go
```bash
scripts/bench-impact-engines.sh \
  --diff-file bench-fixtures/go-heavy.diff \
  --runs 1 \
  --direction callers \
  --lang go \
  --min-lsp-changed 6 \
  --min-lsp-impacted 15
```

### Java
```bash
scripts/bench-impact-engines.sh \
  --diff-file bench-fixtures/java-heavy.diff \
  --runs 1 \
  --direction callers \
  --lang java \
  --min-lsp-changed 7 \
  --min-lsp-impacted 15
```

### Python
```bash
scripts/bench-impact-engines.sh \
  --diff-file bench-fixtures/python-heavy.diff \
  --runs 1 \
  --direction callers \
  --lang python \
  --min-lsp-changed 3 \
  --min-lsp-impacted 5
```

## Threshold update policy (ALL58-2, explicit rule set)

### 1) Review timing (when to evaluate)
- Evaluate threshold updates when either condition is met:
  - every 2 weeks, or
  - after 20 additional successful runs for the target language/fixture.

### 2) Required evidence (must collect before deciding)
For each language/fixture pair, collect the latest window data:
- at least 20 successful runs (if unavailable, mark as `insufficient-sample`),
- guardrail pass/fail counts,
- p10/p50 for `lsp.changed` and `lsp.impacted`,
- latest 10-run trend (dip/outlier presence),
- failure classification (`product-regression` vs `environment/tooling-noise` vs `infra-blocked`).

### 3) Raise conditions (stricten thresholds)
Raise `min-lsp-changed` / `min-lsp-impacted` only if **all** are true:
1. sample size is >= 20 successful runs,
2. `p10(lsp.changed) > current_min_changed + 2`,
3. `p10(lsp.impacted) > current_min_impacted + 2`,
4. no unexplained dip is present in the latest 10 successful runs.

Raise step limits:
- one update max per review window,
- each metric step is `+1` or `+2` (never larger in one change).

### 4) Lower conditions (relax thresholds)
Lower `min-lsp-changed` / `min-lsp-impacted` only if **all** are true:
1. guardrail failures occurred in >= 3 recent runs,
2. investigation classifies root cause as `environment/tooling-noise` (not product regression),
3. same-fixture rerun confirms expected changed/impacted shape is preserved.

Lower step limits:
- each metric step is `-1` or `-2` (never larger in one change),
- never lower below the safety floor.

### 5) No-change / freeze conditions
Keep thresholds unchanged when any of the following holds:
- `insufficient-sample` (successful runs < 20),
- `infra-blocked` (install/startup/timeout prevents reliable measurement),
- active incident without root-cause classification,
- conflicting signals (for example, p10 suggests raise but latest trend has unexplained dips).

### 6) Safety floor (hard lower bound)
Never lower below these bootstrap floors:
- TypeScript: `min-lsp-changed >= 4`, `min-lsp-impacted >= 10`
- JavaScript: `min-lsp-changed >= 4`, `min-lsp-impacted >= 10`
- Ruby: `min-lsp-changed >= 2`, `min-lsp-impacted >= 4`
- Go: `min-lsp-changed >= 4`, `min-lsp-impacted >= 10`
- Java: `min-lsp-changed >= 5`, `min-lsp-impacted >= 10`
- Python: `min-lsp-changed >= 2`, `min-lsp-impacted >= 4`

### 7) Change record format (for PR body / docs)
When thresholds are changed, record:
- target language + fixture,
- old/new threshold pair,
- evidence window (run IDs/date range),
- decision type (`raise` / `lower` / `freeze`) and rule justification,
- rollback trigger condition.
