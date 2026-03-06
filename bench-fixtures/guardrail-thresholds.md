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

## Threshold update policy

### Review timing
- Review thresholds when either condition is met:
  - every 2 weeks, or
  - after 20 additional successful bench runs for the target language/fixture.

### Raise policy (stricter)
- Raise `min-lsp-changed` / `min-lsp-impacted` only when all of the following hold:
  - at least 20 recent successful runs are available,
  - the 10th percentile (p10) for each metric is greater than current threshold + 2,
  - no unexplained dip/regression is observed in the latest 10 successful runs.
- Raise in small steps (`+1` or `+2` max per update) to avoid sudden CI instability.

### Lower policy (more relaxed)
- Lower thresholds only when all of the following hold:
  - guardrail failures occur in 3 or more recent runs,
  - investigation indicates environment/tooling noise (not product regression),
  - rerun with same fixture confirms expected changed/impacted shape is preserved.
- Lower in small steps (`-1` or `-2` max per update).

### Safety floor
- Never lower below these bootstrap floors:
  - TypeScript: `min-lsp-changed >= 4`, `min-lsp-impacted >= 10`
  - JavaScript: `min-lsp-changed >= 4`, `min-lsp-impacted >= 10`
  - Ruby: `min-lsp-changed >= 2`, `min-lsp-impacted >= 4`
  - Go: `min-lsp-changed >= 4`, `min-lsp-impacted >= 10`
  - Java: `min-lsp-changed >= 5`, `min-lsp-impacted >= 10`
  - Python: `min-lsp-changed >= 2`, `min-lsp-impacted >= 4`
