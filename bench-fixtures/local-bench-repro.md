# Local one-shot bench memo (Go/Java fixtures)

This memo records a one-time local measurement run using the fixed heavy diff fixtures.

## Fixtures
- Go: `bench-fixtures/go-heavy.diff`
- Java: `bench-fixtures/java-heavy.diff`

## Reproduction commands

### 1) Strict-LSP bench script (intended path)
```bash
scripts/bench-impact-engines.sh --diff-file bench-fixtures/go-heavy.diff --runs 1 --direction callers --lang go
scripts/bench-impact-engines.sh --diff-file bench-fixtures/java-heavy.diff --runs 1 --direction callers --lang java
```

### 2) TS-only one-shot fallback (works without language servers)
```bash
/usr/bin/time -f "%e" bash -lc "cargo run -q -- impact --engine ts --lang go --direction callers -f json < bench-fixtures/go-heavy.diff > /tmp/go_ts.json"
/usr/bin/time -f "%e" bash -lc "cargo run -q -- impact --engine ts --lang java --direction callers -f json < bench-fixtures/java-heavy.diff > /tmp/java_ts.json"
```

## Local run result (this run)

### Strict-LSP script attempt
- Go command result: `Error: No such file or directory (os error 2)`
- Java command result: `Error: No such file or directory (os error 2)`
- Local tool check:
  - `gopls`: not found
  - `jdtls`: not found

### TS-only one-shot measurement
- Go (`--lang go`, callers, fixture diff):
  - elapsed: `0.16s`
  - changed_symbols: `10`
  - impacted_symbols: `31`
- Java (`--lang java`, callers, fixture diff):
  - elapsed: `0.18s`
  - changed_symbols: `11`
  - impacted_symbols: `31`

## Notes
- For full TS vs LSP(strict) comparison on Go/Java fixtures, install `gopls` and `jdtls`, then rerun the strict-LSP bench script commands above.
