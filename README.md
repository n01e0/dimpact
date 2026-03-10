# dimpact

Current version: `0.5.1`

Fast, language-aware impact analysis for changed code. Feed it a git diff or seed it with specific symbols; get back changed symbols, impacted symbols, and optional reference edges.

Highlights
- Tree‑Sitter engine by default (Auto): robust and fast
- LSP engine (GA): capability‑driven with TS fallback in non‑strict
- Flexible seeding: pass Symbol IDs or JSON
- Symbol ID generator: resolve IDs from file/line/name, with filters

Quick Start
- Build: `cargo build --release`
- Show parsed diff:
  - `git diff --no-ext-diff | dimpact diff -f json`
- Changed symbols:
  - `git diff --no-ext-diff | dimpact changed --lang auto --engine auto -f json`
- Impact from a diff (callers, with edges):
  - `git diff --no-ext-diff | dimpact impact --direction callers --with-edges -f json`
- Impact from seeds (no diff needed):
  - `dimpact impact --seed-symbol 'rust:src/lib.rs:fn:foo:12' --direction callers -f json`
  - `dimpact impact --seed-json '["typescript:src/a.ts:fn:run:10"]' -f json`
- Impact per changed-symbol grouping (direction=both, with edges):
  - `git diff --no-ext-diff | dimpact impact --per-seed --direction both --with-edges -f json`

Symbol ID generator
- Generate candidate IDs from file/line/name; filter by kind; print as JSON/YAML or plain IDs.
- Examples:
  - List IDs in a file: `dimpact id --path src/lib.rs --raw`
  - Narrow by line: `dimpact id --path src/lib.rs --line 120 -f json`
  - Narrow by name and kind, print single‑line raw ID: `dimpact id --path src/lib.rs --name foo --kind fn --raw`
  - Workspace search by name: `dimpact id --name foo -f json`

CLI Overview
- Subcommands:
  - `diff`: parse unified diff from stdin
  - `changed`: compute changed symbols from stdin diff
  - `impact`: compute impact from stdin diff or seeds (supports diff- and seed-based analyses)
  - `id`: generate Symbol IDs from file/line/name
  - `cache`: build/update/stats/clear the incremental cache
  - `completions`: generate shell completion script
- Seeds:
  - `--seed-symbol LANG:PATH:KIND:NAME:LINE` (repeatable)
  - `--seed-json <json|string|path|->` accepts array of strings or objects
  - When seeds are present, language is inferred from seeds (mixed languages error)
- Output formats: `-f json|yaml|dot|html`
  - When confidence filtering is applied (`--min-confidence` and/or `--exclude-dynamic-fallback`), JSON/YAML output includes a `confidence_filter` block with:
    - `min_confidence`
    - `exclude_dynamic_fallback`
    - `input_edge_count`
    - `kept_edge_count`
  
- Impact Options (subcommand `impact`):
  - `--direction callers|callees|both` : traversal direction (default: callers)
  - `--max-depth N`             : max traversal depth (default: 100)
  - `--with-edges`              : include reference edges in output
  - `--min-confidence LEVEL`    : confidence threshold (`confirmed|inferred|dynamic-fallback`)
  - `--exclude-dynamic-fallback`: exclude `dynamic_fallback` edges from traversal/output
  - `--ignore-dir DIR`          : ignore directories by relative prefix (repeatable)
  - `--with-pdg`                : use PDG-based dependence analysis (Rust/Ruby for DFG)
  - `--with-propagation`        : enable symbolic propagation across variables and functions (implies PDG)
  - `--engine auto|ts|lsp`      : analysis engine (default: auto)
  - `--auto-policy compat|strict-if-available` : policy for `--engine auto` (default: compat)
  - `--engine-lsp-strict`       : strict mode for LSP engine
  - `--engine-dump-capabilities`: dump engine capabilities to stderr
  - `--seed-symbol LANG:PATH:KIND:NAME:LINE` : seed symbols by ID (repeatable)
  - `--seed-json PATH|'-'|JSON` : seed symbols via JSON array or file or stdin
  - `--per-seed`              : group impact per changed/seed symbol; when `--direction both`, outputs separate caller and callee results

### Operational guide for `--exclude-dynamic-fallback`
- Purpose: remove low-certainty `dynamic_fallback` edges from traversal/output when you want a precision-first view.
- Recommended usage profile:
  - CI / review gate (precision-first):
    - `git diff --no-ext-diff | dimpact impact --direction callers --with-edges --min-confidence inferred --exclude-dynamic-fallback -f json`
  - Recall investigation (intentionally broad):
    - `git diff --no-ext-diff | dimpact impact --direction callers --with-edges --min-confidence dynamic-fallback -f json`
- Practical rules:
  - `--exclude-dynamic-fallback` is effectively equivalent to `--min-confidence inferred` for edge filtering.
  - If `--min-confidence inferred` is already set, adding `--exclude-dynamic-fallback` is explicit but functionally redundant.
  - For strictest triage, use `--min-confidence confirmed` (the flag then has no additional effect).
- Validation tip:
  - Compare `confidence_filter.input_edge_count` vs `confidence_filter.kept_edge_count` in JSON/YAML output to confirm expected filtering.
  
### PDG Visualization
- Generate PDG in `dot` format with `--with-pdg` and `-f dot`:
  ```
  git diff --no-ext-diff | dimpact impact --with-pdg -f dot
  ```

Path highlighting in DOT/HTML
- When running impact with `--with-edges`, the DOT and HTML outputs highlight edges that are on a shortest path from any changed symbol to any impacted symbol.
- This helps visually trace “how” the impact propagates from changes to affected code.
- HTML view provides filters and automatic layout; highlighted path edges are shown in red.

Engine Selection
- Auto (`--engine auto`) supports policy-based operation:
  - `compat` (default): preserves existing behavior (auto resolves to TS path)
  - `strict-if-available`: prefers LSP path; if capability/session is insufficient, falls back to TS with reasoned logs
- LSP (GA): `--engine lsp`
  - `--engine-lsp-strict`: don’t fall back to TS on LSP issues
  - `--engine-dump-capabilities`: emit LSP capabilities JSON to stderr

Auto policy operation
- Priority: CLI (`--auto-policy`) > env (`DIMPACT_AUTO_POLICY`) > default (`compat`)
- Typical commands:
  - keep compatibility default explicitly:
    - `git diff --no-ext-diff | dimpact impact --engine auto --auto-policy compat -f json`
  - prefer strict-if-available path:
    - `git diff --no-ext-diff | dimpact impact --engine auto --auto-policy strict-if-available -f json`
  - set once via environment variable:
    - `export DIMPACT_AUTO_POLICY=strict-if-available`

Logging
- Uses `env_logger`. Set `RUST_LOG=info` (or `debug|trace`) to see diagnostics.

LSP strict E2E tests
- Strict LSP E2E tests are opt-in via env gates (to keep default CI stable on hosts without language servers).
- Current behavior (Phase A/B synced):
  - once a strict lane is gated on and server preflight passes, failures are treated as **fail-fast** (`server` / `capability` / `logic`).
  - `not-reported` / `unavailable` skip-safe fallbacks have been removed from strict real-LSP lanes.
  - remaining skip-safe residual is operational-only:
    - `env-gate-disabled` (opt-in gate not enabled)
    - `server-missing` (currently rust lanes without `rust-analyzer`)
- Rust strict E2E (`callers` / `callees` / `both`, requires `rust-analyzer`):
  - run: `DIMPACT_E2E_STRICT_LSP=1 cargo test --test engine_lsp`
  - gate semantics: unset => skip, `1` => run, explicit invalid value => fail-fast preflight error.
- strict real-LSP target languages: **TypeScript / TSX / JavaScript / Ruby / Go / Java / Python**
- TypeScript strict E2E (requires `typescript-language-server`):
  - `DIMPACT_E2E_STRICT_LSP_TYPESCRIPT=1 cargo test --test engine_lsp`
  - `DIMPACT_E2E_STRICT_LSP=1` also enables TypeScript strict E2E tests.
- JavaScript strict E2E (requires `typescript-language-server`):
  - `DIMPACT_E2E_STRICT_LSP_JAVASCRIPT=1 cargo test --test engine_lsp`
  - `DIMPACT_E2E_STRICT_LSP=1` also enables JavaScript strict E2E tests.
- TSX strict E2E (requires `typescript-language-server`):
  - `DIMPACT_E2E_STRICT_LSP_TSX=1 cargo test --test engine_lsp`
  - `DIMPACT_E2E_STRICT_LSP=1` also enables TSX strict E2E tests.
- Ruby strict E2E (requires `ruby-lsp`):
  - `DIMPACT_E2E_STRICT_LSP_RUBY=1 cargo test --test engine_lsp`
  - `DIMPACT_E2E_STRICT_LSP=1` also enables Ruby strict E2E tests.
- Go strict E2E (requires `gopls`):
  - `DIMPACT_E2E_STRICT_LSP_GO=1 cargo test --test engine_lsp`
  - `DIMPACT_E2E_STRICT_LSP=1` also enables Go strict E2E tests.
- Java strict E2E (requires `jdtls`):
  - `DIMPACT_E2E_STRICT_LSP_JAVA=1 cargo test --test engine_lsp`
  - `DIMPACT_E2E_STRICT_LSP=1` also enables Java strict E2E tests.
- Python strict E2E (requires one of `pyright-langserver`, `basedpyright-langserver`, `pylsp`):
  - `DIMPACT_E2E_STRICT_LSP_PYTHON=1 cargo test --test engine_lsp`
  - `DIMPACT_E2E_STRICT_LSP=1` also enables Python strict E2E tests.
- Python LSP server selection:
  - auto-detect order: `pyright-langserver` -> `basedpyright-langserver` -> `pylsp`
  - override with: `DIMPACT_PYTHON_LSP=pyright|basedpyright|pylsp`
- Real-LSP server install quickstart (local):
  - TypeScript/TSX/JavaScript: `npm install -g typescript typescript-language-server`
  - Python (pyright): `npm install -g pyright`
  - Go: `go install golang.org/x/tools/gopls@latest`
  - Ruby: `gem install ruby-lsp --no-document`
  - Java (`jdtls`): install `jdtls` and add it to `PATH` (see CI workflow snippet below)
- Real-LSP server setup in CI:
  - `nightly-strict-lsp.yml` installs TS/TSX/JS/Python/Go/Java/Ruby servers before `engine_lsp` strict E2E
  - `bench.yml` installs per-language servers in each strict-LSP benchmark job
- Skip-safe residual report:
  - refresh: `scripts/summarize-strict-e2e-skips.sh tests/engine_lsp.rs`
  - latest artifact: `docs/strict-real-lsp-skip-reasons-v0.4.1.md`

Known limitations
- strict real-LSP still depends on host/runtime prerequisites (installed servers, project/toolchain state).
- opt-in env gates intentionally keep default CI lean; disabled gates are reported as operational residual, not actionable failures.
- once a lane is gated on and preflight passes, strict paths fail-fast (no skip-safe fallback for `not-reported` / `unavailable`).
- Python call extraction currently focuses on core call forms (`foo()`, `obj.m()`, `self.m()`).
  - Highly dynamic constructs (for example runtime-resolved calls) are outside current guarantees.
- Strict mode requires capability support per phase/direction; otherwise it returns explicit strict errors with language/direction/capability hints.

Python parity status (P-END-*)
- ✅ P-END-1: strict + mock covers `callers` / `callees` / `both` with dedicated Python fixtures/tests.
- ✅ P-END-2: strict + `references/definition` route covers `callers` / `callees` / `both` (no not-implemented branch for these directions).
- ✅ P-END-3: real-LSP opt-in E2E exists with env gating (`DIMPACT_E2E_STRICT_LSP_PYTHON` / `DIMPACT_E2E_STRICT_LSP`).
- ✅ P-END-4: Python strict operation/setup is documented in `README.md` and `README_ja.md`.

Usage Examples
- Callers from a diff (JSON with edges):
  - `git diff --no-ext-diff | dimpact impact --direction callers --with-edges -f json`
- Callees limited to depth 2 (YAML):
  - `git diff --no-ext-diff | dimpact impact --direction callees --max-depth 2 -f yaml`
- Force Tree‑Sitter engine (recommended default):
  - `git diff --no-ext-diff | dimpact impact --engine ts -f json`
- Try LSP engine (GA) with strict mode and capability dump:
  - `git diff --no-ext-diff | dimpact impact --engine lsp --engine-lsp-strict --engine-dump-capabilities -f json`
  - Tip: `RUST_LOG=info` to see more diagnostics
- Benchmark policy difference on the same diff (TS fixed vs auto strict-if-available):
  - `scripts/bench-impact-engines.sh --base origin/main --runs 3 --direction callers --lang rust --compare-auto-strict-if-available`
  - fixed diff file: `scripts/bench-impact-engines.sh --diff-file /tmp/dimpact.diff --runs 3 --lang rust --compare-auto-strict-if-available`
  - include second-path RPC method counts: `scripts/bench-impact-engines.sh --base origin/main --runs 1 --rpc-counts --compare-auto-strict-if-available`
  - note: without `--compare-auto-strict-if-available`, the script keeps the original TS vs LSP(strict) comparison mode
  - optional minimum guards for the second path (fail on low counts): `scripts/bench-impact-engines.sh --base origin/main --runs 1 --min-lsp-changed 40 --min-lsp-impacted 15 --compare-auto-strict-if-available`
  - Go strict-LSP bench (requires `gopls`): `scripts/bench-impact-engines.sh --diff-file bench-fixtures/go-heavy.diff --runs 1 --direction callers --lang go --min-lsp-changed 6 --min-lsp-impacted 15`
  - Java strict-LSP bench (requires `jdtls`): `scripts/bench-impact-engines.sh --diff-file bench-fixtures/java-heavy.diff --runs 1 --direction callers --lang java --min-lsp-changed 7 --min-lsp-impacted 15`
  - CI workflow: `Benchmark Impact Engines` (includes rust + Go + Java strict-LSP jobs)
  - Operational cautions (TS/Rust alignment):
    - Existing Rust benchmark operation (`--base origin/main --lang rust`) remains the baseline and is unchanged.
    - Go/Java jobs are additive guardrails using fixed heavy diff fixtures (not a replacement for Rust baseline runs).
    - Threshold values are language/fixture specific; do not compare absolute counts across Rust vs Go/Java.
    - Keep tuning policy conservative (small step updates) to avoid destabilizing existing TS/Rust CI behavior.
- Compare output diff with strict LSP as oracle:
  - `scripts/compare-impact-vs-lsp-oracle.sh --base origin/main --direction callers --lang rust --report-json /tmp/oracle-diff.json`
  - fixed diff file + fail on mismatch: `scripts/compare-impact-vs-lsp-oracle.sh --diff-file /tmp/dimpact.diff --lang rust --with-edges --fail-on-diff`
- Seed via Symbol IDs (no diff needed):
  - `dimpact impact --seed-symbol 'rust:src/lib.rs:fn:foo:12' --direction callers -f json`
- Seed via JSON file:
  - `echo '["typescript:src/a.ts:fn:run:10","typescript:src/b.ts:method:App::start:5"]' > seeds.json`
  - `dimpact impact --seed-json seeds.json --direction both -f json`
- Seed via JSON from stdin (`-`):
  - `printf '[{"lang":"rust","path":"src/lib.rs","kind":"fn","name":"foo","line":12}]' | dimpact impact --seed-json - --direction callers -f json`
- Generate IDs then pipe directly into impact:
  - `dimpact id --path src/lib.rs --name foo --kind fn --raw | dimpact impact --seed-json - --direction callers -f json`
- Search workspace by name and list all candidate IDs:
  - `dimpact id --name initialize --raw`

License
This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
Cache
- Purpose: persist symbols and reference edges to speed up impact analysis.
- Storage: single SQLite DB `index.db` stored in either location:
  - Local (default): `<repo_root>/.dimpact/cache/v1/index.db`
  - Global: `$XDG_CONFIG_HOME/dimpact/cache/v1/<repo_key>/index.db`
Control via subcommands:
  - Build or rebuild the cache: `dimpact cache build --scope local|global [--dir PATH]`
  - Update existing cache (alias `verify`): `dimpact cache update --scope local|global [--dir PATH]`
  - Show cache stats: `dimpact cache stats --scope local|global [--dir PATH]`
  - Clear the cache: `dimpact cache clear --scope local|global [--dir PATH]`
- Impact integration: the TS engine uses the cache by default. On first use it builds the cache; on subsequent runs it updates only changed files.
- Env overrides: `DIMPACT_CACHE_SCOPE=local|global`, `DIMPACT_CACHE_DIR=/custom/dir`.
