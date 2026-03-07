# strict real-LSP E2E skip reason matrix (PH65-1)

source: `tests/engine_lsp.rs`
total skip prints: **95**

## 1) language/direction skip aggregation

| lane | reason | count |
|---|---|---:|
| go/all | server-missing | 1 |
| go/both | both-impact-unavailable | 2 |
|  | both-not-reported | 1 |
|  | changed-symbols-unavailable | 1 |
|  | server-missing | 1 |
| go/callees | callees-impact-unavailable | 2 |
|  | callees-not-reported | 1 |
|  | changed-symbols-unavailable | 1 |
|  | server-missing | 1 |
| go/callers | callers-impact-unavailable | 2 |
|  | changed-symbols-unavailable | 1 |
|  | server-missing | 1 |
| java/all | server-missing | 1 |
| java/both | both-impact-unavailable | 2 |
|  | both-not-reported | 1 |
|  | changed-symbols-unavailable | 1 |
|  | server-missing | 1 |
| java/callees | callees-impact-unavailable | 2 |
|  | callees-not-reported | 1 |
|  | changed-symbols-unavailable | 1 |
|  | server-missing | 1 |
| java/callers | callers-impact-unavailable | 2 |
|  | changed-symbols-unavailable | 1 |
|  | server-missing | 1 |
| javascript/all | server-missing | 1 |
| javascript/both | both-impact-unavailable | 2 |
|  | both-not-reported | 1 |
|  | changed-symbols-unavailable | 1 |
|  | server-missing | 1 |
| javascript/callees | callees-impact-unavailable | 2 |
|  | callees-not-reported | 1 |
|  | changed-symbols-unavailable | 1 |
|  | server-missing | 1 |
| javascript/callers | callers-impact-unavailable | 2 |
|  | callers-not-reported | 1 |
|  | changed-symbols-unavailable | 1 |
|  | server-missing | 1 |
| python/both | both-impact-unavailable | 2 |
|  | both-not-reported | 1 |
|  | changed-symbols-unavailable | 1 |
| python/callees | callees-impact-unavailable | 2 |
|  | callees-not-reported | 1 |
|  | changed-symbols-unavailable | 1 |
| python/callers | callers-impact-unavailable | 2 |
|  | changed-symbols-unavailable | 1 |
| ruby/all | server-missing | 1 |
| ruby/both | both-impact-unavailable | 2 |
|  | both-not-reported | 1 |
|  | changed-symbols-unavailable | 1 |
|  | server-missing | 1 |
| ruby/callees | callees-impact-unavailable | 2 |
|  | callees-not-reported | 1 |
|  | changed-symbols-unavailable | 1 |
|  | server-missing | 1 |
| ruby/callers | callers-impact-unavailable | 2 |
|  | callers-not-reported | 1 |
|  | changed-symbols-unavailable | 1 |
|  | server-missing | 1 |
| rust/callers | env-gate-disabled | 2 |
|  | other | 2 |
|  | strict-lsp-unavailable | 2 |
| typescript/all | server-missing | 1 |
| typescript/both | both-impact-unavailable | 2 |
|  | both-not-reported | 1 |
|  | changed-symbols-unavailable | 1 |
|  | server-missing | 1 |
| typescript/callees | callees-impact-unavailable | 2 |
|  | callees-not-reported | 1 |
|  | changed-symbols-unavailable | 1 |
|  | server-missing | 1 |
| typescript/callers | callers-impact-unavailable | 2 |
|  | callers-not-reported | 1 |
|  | changed-symbols-unavailable | 1 |
|  | server-missing | 1 |

## 2) promotion candidates (fail-fast migration candidates)

- `go/callers`: callers lane has no explicit report-gap skip marker in current tests
- `java/callers`: callers lane has no explicit report-gap skip marker in current tests
- `python/callers`: callers lane has no explicit report-gap skip marker in current tests

## 3) hold candidates

- `go/all`: prioritize callers for phase-1 fail-fast migration
- `go/both`: prioritize callers for phase-1 fail-fast migration; contains not-reported skip marker; contains unavailable skip marker
- `go/callees`: prioritize callers for phase-1 fail-fast migration; contains not-reported skip marker; contains unavailable skip marker
- `java/all`: prioritize callers for phase-1 fail-fast migration
- `java/both`: prioritize callers for phase-1 fail-fast migration; contains not-reported skip marker; contains unavailable skip marker
- `java/callees`: prioritize callers for phase-1 fail-fast migration; contains not-reported skip marker; contains unavailable skip marker
- `javascript/all`: prioritize callers for phase-1 fail-fast migration
- `javascript/both`: prioritize callers for phase-1 fail-fast migration; contains not-reported skip marker; contains unavailable skip marker
- `javascript/callees`: prioritize callers for phase-1 fail-fast migration; contains not-reported skip marker; contains unavailable skip marker
- `javascript/callers`: contains not-reported skip marker; contains unavailable skip marker
- `python/both`: prioritize callers for phase-1 fail-fast migration; contains not-reported skip marker; contains unavailable skip marker
- `python/callees`: prioritize callers for phase-1 fail-fast migration; contains not-reported skip marker; contains unavailable skip marker
- `ruby/all`: prioritize callers for phase-1 fail-fast migration
- `ruby/both`: prioritize callers for phase-1 fail-fast migration; contains not-reported skip marker; contains unavailable skip marker
- `ruby/callees`: prioritize callers for phase-1 fail-fast migration; contains not-reported skip marker; contains unavailable skip marker
- `ruby/callers`: contains not-reported skip marker; contains unavailable skip marker
- `rust/callers`: contains unavailable skip marker
- `typescript/all`: prioritize callers for phase-1 fail-fast migration
- `typescript/both`: prioritize callers for phase-1 fail-fast migration; contains not-reported skip marker; contains unavailable skip marker
- `typescript/callees`: prioritize callers for phase-1 fail-fast migration; contains not-reported skip marker; contains unavailable skip marker
- `typescript/callers`: contains not-reported skip marker; contains unavailable skip marker

## 4) screening policy used

- callers lane first (phase-1)
- `env-gate-disabled` / `server-missing` are treated as operational prerequisites
- lanes with `*-not-reported` or `*-unavailable` markers are kept in hold set
