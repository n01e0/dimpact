# strict real-LSP skip-safe lane inventory (A1)

source: `tests/engine_lsp.rs`
total skip prints: **112**

## 1) language × direction × reason

| language | direction | reason | count | sample |
|---|---|---|---:|---|
| go | all | server-missing | 1 | skip: gopls not found |
| go | both | both-impact-unavailable | 2 | skip: strict Go both impact unavailable in this env: {e} |
|  |  | both-not-reported | 1 | skip: Go LSP did not report both-direction impacts in this environment |
|  |  | changed-symbols-unavailable | 1 | skip: strict Go changed_symbols unavailable in this env: {e} |
|  |  | server-missing | 1 | skip: gopls not found |
| go | callees | callees-impact-unavailable | 2 | skip: strict Go callees impact unavailable in this env: {e} |
|  |  | callees-not-reported | 1 | skip: Go LSP did not report callees in this environment |
|  |  | changed-symbols-unavailable | 1 | skip: strict Go changed_symbols unavailable in this env: {e} |
|  |  | server-missing | 1 | skip: gopls not found |
| go | callers | server-missing | 1 | skip: gopls not found |
| java | all | server-missing | 1 | skip: jdtls not found |
| java | both | both-impact-unavailable | 2 | skip: strict Java both impact unavailable in this env: {e} |
|  |  | both-not-reported | 1 | skip: Java LSP did not report both-direction impacts in this environment |
|  |  | changed-symbols-unavailable | 1 | skip: strict Java changed_symbols unavailable in this env: {e} |
|  |  | server-missing | 1 | skip: jdtls not found |
| java | callees | callees-impact-unavailable | 2 | skip: strict Java callees impact unavailable in this env: {e} |
|  |  | callees-not-reported | 1 | skip: Java LSP did not report callees in this environment |
|  |  | changed-symbols-unavailable | 1 | skip: strict Java changed_symbols unavailable in this env: {e} |
|  |  | server-missing | 1 | skip: jdtls not found |
| java | callers | server-missing | 1 | skip: jdtls not found |
| javascript | all | server-missing | 1 | skip: typescript-language-server not found |
| javascript | both | both-impact-unavailable | 2 | skip: strict JavaScript both impact unavailable in this env: {e} |
|  |  | both-not-reported | 1 | skip: JavaScript LSP did not report both-direction impacts in this environment |
|  |  | changed-symbols-unavailable | 1 | skip: strict JavaScript changed_symbols unavailable in this env: {e} |
|  |  | server-missing | 1 | skip: typescript-language-server not found |
| javascript | callees | callees-impact-unavailable | 2 | skip: strict JavaScript callees impact unavailable in this env: {e} |
|  |  | callees-not-reported | 1 | skip: JavaScript LSP did not report callees in this environment |
|  |  | changed-symbols-unavailable | 1 | skip: strict JavaScript changed_symbols unavailable in this env: {e} |
|  |  | server-missing | 1 | skip: typescript-language-server not found |
| javascript | callers | callers-impact-unavailable | 2 | skip: strict JavaScript callers impact unavailable in this env: {e} |
|  |  | callers-not-reported | 1 | skip: JavaScript LSP did not report callers in this environment |
|  |  | changed-symbols-unavailable | 1 | skip: strict JavaScript changed_symbols unavailable in this env: {e} |
|  |  | server-missing | 1 | skip: typescript-language-server not found |
| python | both | both-impact-unavailable | 2 | skip: strict python both impact unavailable in this env: {e} |
|  |  | both-not-reported | 1 | skip: python LSP did not report both-direction impacts in this environment |
|  |  | changed-symbols-unavailable | 1 | skip: strict python changed_symbols unavailable in this env: {e} |
| python | callees | callees-impact-unavailable | 2 | skip: strict python callees impact unavailable in this env: {e} |
|  |  | callees-not-reported | 1 | skip: python LSP did not report callees in this environment |
|  |  | changed-symbols-unavailable | 1 | skip: strict python changed_symbols unavailable in this env: {e} |
| ruby | all | server-missing | 1 | skip: ruby-lsp not found |
| ruby | both | both-impact-unavailable | 2 | skip: strict Ruby both impact unavailable in this env: {e} |
|  |  | both-not-reported | 1 | skip: Ruby LSP did not report both-direction impacts in this environment |
|  |  | changed-symbols-unavailable | 1 | skip: strict Ruby changed_symbols unavailable in this env: {e} |
|  |  | server-missing | 1 | skip: ruby-lsp not found |
| ruby | callees | callees-impact-unavailable | 2 | skip: strict Ruby callees impact unavailable in this env: {e} |
|  |  | callees-not-reported | 1 | skip: Ruby LSP did not report callees in this environment |
|  |  | changed-symbols-unavailable | 1 | skip: strict Ruby changed_symbols unavailable in this env: {e} |
|  |  | server-missing | 1 | skip: ruby-lsp not found |
| ruby | callers | callers-impact-unavailable | 2 | skip: strict Ruby callers impact unavailable in this env: {e} |
|  |  | callers-not-reported | 1 | skip: Ruby LSP did not report callers in this environment |
|  |  | changed-symbols-unavailable | 1 | skip: strict Ruby changed_symbols unavailable in this env: {e} |
|  |  | server-missing | 1 | skip: ruby-lsp not found |
| rust | both | both-impact-unavailable | 2 | skip: strict rust both impact unavailable in this env: {e} |
|  |  | both-not-reported | 1 | skip: rust LSP did not report both-direction impacts in this environment |
|  |  | env-gate-disabled | 1 | skip: set DIMPACT_E2E_STRICT_LSP=1 to run strict LSP e2e tests |
|  |  | other | 1 | skip: rust-analyzer not available |
| rust | callees | callees-impact-unavailable | 2 | skip: strict rust callees impact unavailable in this env: {e} |
|  |  | callees-not-reported | 1 | skip: rust LSP did not report callees in this environment |
|  |  | env-gate-disabled | 1 | skip: set DIMPACT_E2E_STRICT_LSP=1 to run strict LSP e2e tests |
|  |  | other | 1 | skip: rust-analyzer not available |
| rust | callers | env-gate-disabled | 2 | skip: set DIMPACT_E2E_STRICT_LSP=1 to run strict LSP e2e tests |
|  |  | other | 2 | skip: rust-analyzer not available |
|  |  | strict-lsp-unavailable | 2 | skip: strict LSP unavailable in this environment: {e} |
| tsx | all | server-missing | 1 | skip: typescript-language-server not found |
| tsx | both | both-impact-unavailable | 2 | skip: strict TSX both impact unavailable in this env: {e} |
|  |  | both-not-reported | 1 | skip: TSX LSP did not report both-direction impacts in this environment |
|  |  | changed-symbols-unavailable | 1 | skip: strict TSX changed_symbols unavailable in this env: {e} |
|  |  | server-missing | 1 | skip: typescript-language-server not found |
| tsx | callees | callees-impact-unavailable | 2 | skip: strict TSX callees impact unavailable in this env: {e} |
|  |  | callees-not-reported | 1 | skip: TSX LSP did not report callees in this environment |
|  |  | changed-symbols-unavailable | 1 | skip: strict TSX changed_symbols unavailable in this env: {e} |
|  |  | server-missing | 1 | skip: typescript-language-server not found |
| tsx | callers | callers-impact-unavailable | 2 | skip: strict TSX callers impact unavailable in this env: {e} |
|  |  | callers-not-reported | 1 | skip: TSX LSP did not report callers in this environment |
|  |  | changed-symbols-unavailable | 1 | skip: strict TSX changed_symbols unavailable in this env: {e} |
|  |  | server-missing | 1 | skip: typescript-language-server not found |
| typescript | all | server-missing | 1 | skip: typescript-language-server not found |
| typescript | both | both-impact-unavailable | 2 | skip: strict TypeScript both impact unavailable in this env: {e} |
|  |  | both-not-reported | 1 | skip: TypeScript LSP did not report both-direction impacts in this environment |
|  |  | changed-symbols-unavailable | 1 | skip: strict TypeScript changed_symbols unavailable in this env: {e} |
|  |  | server-missing | 1 | skip: typescript-language-server not found |
| typescript | callees | callees-impact-unavailable | 2 | skip: strict TypeScript callees impact unavailable in this env: {e} |
|  |  | callees-not-reported | 1 | skip: TypeScript LSP did not report callees in this environment |
|  |  | changed-symbols-unavailable | 1 | skip: strict TypeScript changed_symbols unavailable in this env: {e} |
|  |  | server-missing | 1 | skip: typescript-language-server not found |
| typescript | callers | callers-impact-unavailable | 2 | skip: strict TypeScript callers impact unavailable in this env: {e} |
|  |  | callers-not-reported | 1 | skip: TypeScript LSP did not report callers in this environment |
|  |  | changed-symbols-unavailable | 1 | skip: strict TypeScript changed_symbols unavailable in this env: {e} |
|  |  | server-missing | 1 | skip: typescript-language-server not found |

## 2) totals by language

| language | skip prints |
|---|---:|
| go | 12 |
| java | 12 |
| javascript | 16 |
| python | 8 |
| ruby | 16 |
| rust | 16 |
| tsx | 16 |
| typescript | 16 |

## 3) remarks

- This inventory is generated from current `skip:` prints in `tests/engine_lsp.rs`.
- Lanes without skip markers may already be fail-fast migrated or not covered by skip-safe code paths.
