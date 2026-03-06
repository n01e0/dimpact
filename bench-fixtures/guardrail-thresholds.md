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
