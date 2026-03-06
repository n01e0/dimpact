# Go/Java bench guardrail thresholds (initial)

This note records initial `min-changed` / `min-impacted` values for strict-LSP bench guardrails.

## Go (BJ40-1)
- target fixture: `bench-fixtures/go-heavy.diff`
- initial (relaxed) strict-LSP guardrail:
  - `--min-lsp-changed 6`
  - `--min-lsp-impacted 15`

### Rationale
- The fixture is intentionally heavy and currently produces larger counts on TS fallback/local runs.
- Initial values are set conservatively (relaxed) to catch major regressions while avoiding over-tight CI failures in early rollout.
- These are bootstrap values and should be raised after CI trend data stabilizes.

### Repro command template
```bash
scripts/bench-impact-engines.sh \
  --diff-file bench-fixtures/go-heavy.diff \
  --runs 1 \
  --direction callers \
  --lang go \
  --min-lsp-changed 6 \
  --min-lsp-impacted 15
```

## Java (BJ40-2)
- target fixture: `bench-fixtures/java-heavy.diff`
- initial (relaxed) strict-LSP guardrail:
  - `--min-lsp-changed 7`
  - `--min-lsp-impacted 15`

### Rationale
- Local fixture measurement currently reports changed/impacted counts above these values, so this catches large regressions while keeping early CI rollout tolerant.
- The threshold is intentionally conservative and should be tuned upward after stable CI trend data is collected.

### Repro command template
```bash
scripts/bench-impact-engines.sh \
  --diff-file bench-fixtures/java-heavy.diff \
  --runs 1 \
  --direction callers \
  --lang java \
  --min-lsp-changed 7 \
  --min-lsp-impacted 15
```

## Python (NX8-7)
- target fixture: `bench-fixtures/python-heavy.diff`
- initial (relaxed) strict-LSP guardrail:
  - `--min-lsp-changed 3`
  - `--min-lsp-impacted 5`

### Rationale
- Python strict-LSP behavior is environment-dependent and can vary by server implementation/version, so bootstrap thresholds are intentionally conservative.
- Initial values still catch major regressions while reducing false positives during CI rollout.
- Thresholds should be tightened after enough successful CI history is collected.

### Repro command template
```bash
scripts/bench-impact-engines.sh \
  --diff-file bench-fixtures/python-heavy.diff \
  --runs 1 \
  --direction callers \
  --lang python \
  --min-lsp-changed 3 \
  --min-lsp-impacted 5
```

## Threshold update policy (BJ40-4)

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
  - Go: `min-lsp-changed >= 4`, `min-lsp-impacted >= 10`
  - Java: `min-lsp-changed >= 5`, `min-lsp-impacted >= 10`
  - Python: `min-lsp-changed >= 2`, `min-lsp-impacted >= 4`
