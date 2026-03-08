# strict real-LSP E2E skip-safe residual report (F2)

source: `tests/engine_lsp.rs`
total skip prints: **25**

## 1) language/direction skip aggregation

| lane | reason | count |
|---|---|---:|
| go/all | env-gate-disabled | 1 |
| go/both | env-gate-disabled | 1 |
| go/callees | env-gate-disabled | 1 |
| java/all | env-gate-disabled | 1 |
| java/both | env-gate-disabled | 1 |
| java/callees | env-gate-disabled | 1 |
| javascript/all | env-gate-disabled | 1 |
| javascript/both | env-gate-disabled | 1 |
| javascript/callees | env-gate-disabled | 1 |
| python/all | env-gate-disabled | 1 |
| python/both | env-gate-disabled | 1 |
| python/callees | env-gate-disabled | 1 |
| ruby/all | env-gate-disabled | 1 |
| ruby/both | env-gate-disabled | 1 |
| ruby/callees | env-gate-disabled | 1 |
| rust/both | server-missing | 1 |
| rust/callees | server-missing | 1 |
| rust/callers | server-missing | 2 |
| tsx/all | env-gate-disabled | 1 |
| tsx/both | env-gate-disabled | 1 |
| tsx/callees | env-gate-disabled | 1 |
| typescript/all | env-gate-disabled | 1 |
| typescript/both | env-gate-disabled | 1 |
| typescript/callees | env-gate-disabled | 1 |

## 2) actionable residual (non-operational)

- lanes: **0**
- none (0)

## 3) minimal residual with reasons (operational prerequisites)

- lanes: **24**
- `go/all`: env gate opt-in
- `go/both`: env gate opt-in
- `go/callees`: env gate opt-in
- `java/all`: env gate opt-in
- `java/both`: env gate opt-in
- `java/callees`: env gate opt-in
- `javascript/all`: env gate opt-in
- `javascript/both`: env gate opt-in
- `javascript/callees`: env gate opt-in
- `python/all`: env gate opt-in
- `python/both`: env gate opt-in
- `python/callees`: env gate opt-in
- `ruby/all`: env gate opt-in
- `ruby/both`: env gate opt-in
- `ruby/callees`: env gate opt-in
- `rust/both`: server missing on host
- `rust/callees`: server missing on host
- `rust/callers`: server missing on host
- `tsx/all`: env gate opt-in
- `tsx/both`: env gate opt-in
- `tsx/callees`: env gate opt-in
- `typescript/all`: env gate opt-in
- `typescript/both`: env gate opt-in
- `typescript/callees`: env gate opt-in

## 4) policy used

- residual is acceptable when only `env-gate-disabled` / `server-missing` remain
- any other reason is treated as actionable residual
