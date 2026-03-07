# strict-LSP RPC / time profile snapshot (ALL62-1)

取得日: 2026-03-07 (local run)

## Scope
- Languages: Rust / TypeScript / JavaScript / Ruby / Go / Java / Python
- Command baseline: `scripts/bench-impact-engines.sh --rpc-counts`
- Mode: strict-LSP (`--engine lsp --engine-lsp-strict` path)

## Result summary

| Language | Input | Runs | strict-LSP time (avg/min/max) | changed/impacted | RPC profile (top) | Status |
|---|---|---:|---|---|---|---|
| Rust | `--base v0.4.0` | 3 | `46.707s / 21.790s / 59.670s` | `9 / 0` | `documentSymbol=2098`, `references=780`, `didOpen=85` | OK |
| TypeScript | `bench-fixtures/ts-heavy.diff` | 1 | `1.380s / 1.380s / 1.380s` | `10 / 31` | `incomingCalls=41`, `prepareCallHierarchy=11`, `documentSymbol=2` | OK |
| JavaScript | `bench-fixtures/js-heavy.diff` | 3 | `1.373s / 1.000s / 1.560s` | `0 / 0` | `incomingCalls=41`, `prepareCallHierarchy=11`, `documentSymbol=2` | OK (zero result) |
| Ruby | `bench-fixtures/ruby-heavy.diff` | 3 | N/A | N/A | N/A | BLOCKED (`lsp initialize timeout or invalid response`) |
| Go | `bench-fixtures/go-heavy.diff` | 3 | `0.590s / 0.380s / 0.750s` | `10 / 31` | `incomingCalls=41`, `prepareCallHierarchy=11`, `documentSymbol=2` | OK |
| Java | `bench-fixtures/java-heavy.diff` | 3 | N/A | N/A | N/A | BLOCKED (`lsp initialize timeout or invalid response`) |
| Python | `bench-fixtures/python-heavy.diff` | 3 | `1.127s / 1.120s / 1.130s` | `10 / 31` | `incomingCalls=41`, `prepareCallHierarchy=11`, `documentSymbol=2` | OK |

## Logs

Raw logs are stored next to this file:

- `rust.log`
- `typescript.log` (initial 3-run attempt, timed out)
- `typescript-runs1.log` (successful 1-run fallback)
- `javascript.log`
- `ruby.log`
- `go.log`
- `java.log`
- `python.log`

## Notes for follow-up (Loop 62)

- Rust strict-LSP shows heavy `textDocument/documentSymbol` volume and large wall-time variance.
- TypeScript 3-run measurement timed out once; 1-run fallback succeeded.
- JavaScript returns `changed/impacted = 0/0` despite successful RPC call flow.
- Ruby/Java are currently blocked at initialize timeout and need startup/capability stabilization before optimization work.
