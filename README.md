# dimpact

Current version: `0.5.3`

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

### Reading the new `summary` output (JSON/YAML)
- `impact` JSON/YAML now includes a `summary` block intended for quick triage before you inspect the full symbol/edge lists.
- Current summary fields:
  - `summary.by_depth`
    - shortest-hop buckets from the changed/seed symbols to impacted symbols
    - `depth=1` is the direct hit set
    - `depth>=2` shows transitive spread
  - `summary.risk`
    - lightweight first-pass triage priority built from direct hits, transitive hits, impacted file count, and impacted symbol count
    - read it as review/CI priority, not as a production-severity prediction or a replacement for inspecting the raw graph
    - rough reading guide:
      - `low`: likely local; start near the changed code
      - `medium`: not huge, but callers or nearby files are worth checking
      - `high`: broad enough that you should assume caller-side spread and inspect the graph early
  - `summary.affected_modules`
    - lightweight path-based grouping of impacted symbols
    - useful for answering “which area of the repo should I inspect next?”
    - entry-like files are normalized for readability: `src/main.rs` / `src/lib.rs` / `src/engine/mod.rs` collapse to their parent path, and root-level entry files collapse to `(root)`
- There is still no `summary.affected_processes`; that remains intentionally deferred until entrypoint heuristics and fixtures are strong enough.
- Typical JSON shape:
  ```json
  {
    "changed_symbols": [...],
    "impacted_symbols": [...],
    "impacted_files": [...],
    "edges": [...],
    "impacted_by_file": {...},
    "impacted_witnesses": {...},
    "summary": {
      "by_depth": [
        { "depth": 1, "symbol_count": 3, "file_count": 2 },
        { "depth": 2, "symbol_count": 7, "file_count": 4 }
      ],
      "risk": {
        "level": "medium",
        "direct_hits": 3,
        "transitive_hits": 7,
        "impacted_files": 4,
        "impacted_symbols": 10
      },
      "affected_modules": [
        { "module": "src/engine", "symbol_count": 4, "file_count": 2 },
        { "module": "(root)", "symbol_count": 2, "file_count": 1 }
      ]
    },
    "confidence_filter": {
      "min_confidence": "inferred",
      "exclude_dynamic_fallback": false,
      "input_edge_count": 20,
      "kept_edge_count": 12
    }
  }
  ```
- Operational intent:
  - start with `by_depth` to separate direct vs transitive impact
  - use `risk` to decide how aggressively to triage the diff (`low` = local-first, `medium` = nearby spread worth checking, `high` = broad-wave suspicion)
  - use `affected_modules` to decide which directories/modules to open next; `(root)` means the repo-root entry area, not a literal module named `main.rs`
  - then inspect `impacted_symbols` / `edges` for the concrete graph details
- `confidence_filter` remains a top-level sibling of `summary`, not a field inside it.
- `impacted_witnesses` now carry a minimal path summary for each impacted symbol:
  - `edge` / `via_symbol_id` still point at the selected last hop
  - `path` expands that witness into one chosen hop-by-hop route from the root changed/seed symbol
  - `provenance_chain` / `kind_chain` make it easier to see where call / data / control / symbolic-propagation edges entered that route
  - `path_compact` / `provenance_chain_compact` / `kind_chain_compact` keep the same route in a more compressed, explanation-oriented form
  - `slice_context.selected_files_on_path` lightly connects that witness route back to the bounded-slice planner, so you can see which selected files were actually on the witness path, which hop indices they covered, and which slice-selection reasons retained them
  - `slice_context.selected_vs_pruned_reasons` adds the minimal "why this won" explanation when a selected bridge candidate beat a ranked-out competitor
  - this is still one selected shortest-path explanation, not an exhaustive proof of every possible route
- `summary.slice_selection` is emitted on the PDG / propagation path and exposes the bounded-slice planner decision itself:
  - `files[*]` lists the selected file-level scope with `cache_update` / `local_dfg` / `explanation` split
  - `files[*].reasons[*]` records the selection reason per seed, including direct-boundary vs bridge-completion distinctions
  - `files[*].reasons[*].scoring` and `pruned_candidates[*].scoring` expose the bridge-candidate score profile (`source_kind`, `lane`, evidence kinds, score tuple), so selected-vs-pruned comparisons are reviewable from JSON/YAML output
  - `pruned_candidates[*]` gives the minimal diagnostics for ranked-out / budget-pruned candidates when the planner had to drop alternatives
  - read the scope split as: `cache_update` = execution preparation, `local_dfg` = local flow materialization, `explanation` = user-facing retained file; `local_dfg` and `explanation` may diverge, and pruned candidates do not become explanation files
- With `--per-seed`, the same summary appears under `impacts[].output.summary` for each changed/seed symbol, and witness data stays nested under each grouped output.
- DOT/HTML output stays compatible; the new summary fields are for JSON/YAML consumption.

### Evidence-driven selection: how to think about it
- The PDG / propagation planner is intentionally **not** trying to include every reachable helper file.
  - the goal is to keep a small bounded explanation slice and choose the strongest local continuation for each boundary side
- G9 sharpens the mental model a bit:
  - `source_kind` and `lane` are still important ranking dimensions, but they are **not** themselves evidence
  - the evidence vocabulary is easiest to read as four normalized categories:
    1. `primary`: direct continuity facts such as `param_to_return_flow`, `return_flow`, `assigned_result`, or `alias_chain` when they represent the actual selected continuation
    2. `support`: strength/provenance signals such as `local_dfg_support`, `symbolic_propagation_support`, `edge_certainty`, or positional hints that help explain trust without justifying selection by themselves
    3. `fallback`: bounded admission reasons for candidates that are intentionally narrow runtime recoveries, such as `explicit_require_relative_load`, `companion_file_match`, `dynamic_dispatch_literal_target`, or weaker `require_relative` continuation facts
    4. `negative`: suppressing signals that keep noisy candidates from winning, such as helper-style return noise or fallback-only losers with weaker certainty
- In practice, the planner still compares selected vs pruned candidates through `summary.slice_selection.files[*].reasons[*].scoring` and `summary.slice_selection.pruned_candidates[*].scoring`, but the intended reading is now:
  - first ask which candidate had the stronger `primary` continuity
  - then use `support` to explain why that continuation was more trustworthy
  - then use `fallback` to explain why a narrow runtime candidate was allowed to exist at all
  - finally use `negative` to explain why a loser stayed ranked out instead of widening the slice
- The important operational idea is still: **better evidence should improve precision without widening scope**.
  - a stronger winner becomes the selected explanation file
  - weaker alternatives stay visible in `pruned_candidates[*]` or `slice_context.selected_vs_pruned_reasons` instead of silently expanding the slice
- Witness output follows the same shape in a compact way:
  - `winning_primary_evidence_kinds` and `winning_support` explain the selected side
  - `losing_side_reason` explains the loser when there is an obvious suppressing reason (for example helper noise, fallback-only status, or weaker `dynamic_fallback` certainty)
- A good reading order for surprising Rust/Ruby PDG output is:
  1. inspect `summary.slice_selection.files[*].reasons[*].scoring`
  2. compare against `summary.slice_selection.pruned_candidates[*].scoring`
  3. finish with `impacted_witnesses[*].slice_context.selected_vs_pruned_reasons` for the shortest human-facing explanation

- Impact Options (subcommand `impact`):
  - `--direction callers|callees|both` : traversal direction (default: callers)
  - `--max-depth N`             : max traversal depth (default: 100)
  - `--with-edges`              : include reference edges in output
  - `--min-confidence LEVEL`    : confidence threshold (`confirmed|inferred|dynamic-fallback`)
  - `--exclude-dynamic-fallback`: exclude `dynamic_fallback` edges from traversal/output
  - `--op-profile PROFILE`      : operational preset (`balanced|precision-first`)
  - `--ignore-dir DIR`          : ignore directories by relative prefix (repeatable)
  - `--with-pdg`                : add local PDG/DFG edges on top of normal impact traversal (Rust/Ruby local DFG)
  - `--with-propagation`        : add symbolic propagation bridges on top of PDG (call-site / summary-oriented heuristics)
  - `--engine auto|ts|lsp`      : analysis engine (default: auto)
  - `--auto-policy compat|strict-if-available` : policy for `--engine auto` (default: compat)
  - `--engine-lsp-strict`       : strict mode for LSP engine
  - `--engine-dump-capabilities`: dump engine capabilities to stderr
  - `--seed-symbol LANG:PATH:KIND:NAME:LINE` : seed symbols by ID (repeatable)
  - `--seed-json PATH|'-'|JSON` : seed symbols via JSON array or file or stdin
  - `--per-seed`              : group impact per changed/seed symbol; when `--direction both`, outputs separate caller and callee results

### Operational confidence profiles (`--op-profile`)
- `balanced`
  - applies `--min-confidence inferred` (recommended default operating mode)
  - keeps a practical recall/precision balance for routine analysis
- `precision-first`
  - applies `--min-confidence confirmed` + `--exclude-dynamic-fallback`
  - intended for strict CI/review triage where false positives are costly
- Override precedence:
  - explicit flags (`--min-confidence`, `--exclude-dynamic-fallback`) override profile defaults
- Typical usage:
  - balanced:
    - `git diff --no-ext-diff | dimpact impact --direction callers --with-edges --op-profile balanced -f json`
  - precision-first:
    - `git diff --no-ext-diff | dimpact impact --direction callers --with-edges --op-profile precision-first -f json`
  - recall investigation (intentionally broad):
    - `git diff --no-ext-diff | dimpact impact --direction callers --with-edges --min-confidence dynamic-fallback -f json`
- Validation tip:
  - Compare `confidence_filter.input_edge_count` vs `confidence_filter.kept_edge_count` in JSON/YAML output to confirm expected filtering.

### Recommended `--min-confidence` by language (Q54-10)

Based on Q54-10 re-sampling (`release-notes/0.5.4-confidence-distribution-q54-10.md`), the current operational recommendations are:

| Language | Recommended `--min-confidence` | observed inferred edges | Rationale |
| --- | --- | ---: | --- |
| typescript | `inferred` | 3 | sampled impacted edges are inferred; `confirmed` would drop observed signal |
| tsx | `inferred` | 0 | no impacted edges in sampled corpus; keep global default for consistency |
| rust | `inferred` | 0 | no impacted edges in sampled corpus; keep global default for consistency |
| java | `inferred` | 19 | sampled impacted edges are inferred; `confirmed` would drop observed signal |
| go | `inferred` | 0 | no impacted edges in sampled corpus; keep global default for consistency |
| ruby | `inferred` | 6 | sampled impacted edges are inferred; `confirmed` would drop observed signal |
| python | `inferred` | 12 | sampled impacted edges are inferred; `confirmed` would drop observed signal |

- Recommended global default: `inferred`.
- For strict review/CI triage where false positives are costly, use `--op-profile precision-first` (or `--min-confidence confirmed --exclude-dynamic-fallback`).
  
### PDG / propagation: when to use which
- Bounded slice is the current scope model for the PDG path:
  - the goal is **not** to open the whole repo, but to recover the few nearby files that are most likely needed to explain a short multi-file bridge
  - the current planner is a **controlled 2-hop** model: seed/changed file first, then direct boundary files, then at most one bridge-completion file per boundary side under a small per-seed budget
  - `--with-pdg` and `--with-propagation` now share that same bounded slice planner in diff mode and seed mode
- `--with-pdg`
  - best when you want extra Rust/Ruby local data/control-dependence context around the selected bounded slice
  - useful when plain call-graph impact is too coarse and you want to confirm which nearby file(s) should even be in scope before asking stronger flow questions
  - this is the lighter-weight option when you care more about local explanation than about bridging values across call boundaries
- `--with-propagation`
  - builds on the same bounded slice, then adds symbolic call-site / summary bridges on top
  - best when the real question is "does this value / argument / result continue across the boundary?"
  - use this when you suspect false negatives around alias chains, wrapper return-flow, imported-result continuation, or other short inter-procedural Rust/Ruby bridges
- `--per-seed`
  - works with both normal impact and the PDG / propagation path
  - useful when you want to compare how each changed/seed symbol fans out independently, including per-seed witnesses and compact path summaries

### Current limits of the PDG / propagation path
- Scope is intentionally bounded today:
  - the planner is still closer to a **bounded project slice** than to project-wide closure
  - current Rust/Ruby scope selection is roughly: root file(s) + direct boundary + controlled second-hop bridge files for a few boundary sides
  - that helps short 2-hop-style bridges, but it still deliberately stops before recursive whole-project expansion
  - other languages still mostly fall back to the normal call-graph signal even if you pass `--with-pdg`
- It is **not** a project-wide PDG yet:
  - current behavior is still closer to `global call graph + bounded local DFG augmentation`
  - propagation is mostly call-site / summary-oriented, not a whole-program symbolic executor
- Witnesses are better, but still intentionally minimal:
  - `impacted_witnesses.path` / `provenance_chain` / `kind_chain` expose one chosen multi-hop explanation
  - `impacted_witnesses.slice_context` now tells you which selected files on that route came from the bounded-slice planner and which seed-specific reasons retained them
  - the `*_compact` witness fields make that explanation easier to read, but they are still summaries of one selected route
  - you still do **not** get all competing paths or a full proof that no alternative route exists
- `-f dot` changes meaning when PDG is enabled:
  - normal `impact -f dot` shows the impact graph
  - `impact --with-pdg -f dot` shows the raw PDG/DFG-style graph instead
- Ruby has improved short multi-file coverage, but still has clear boundaries:
  - short `require_relative` / alias / wrapper-return chains are better than before, including no-paren wrapper parameter flow
  - bridge scoring now tries to keep semantic alias / return-flow completions ahead of plain `require_relative` helper noise, while leaving weaker fallback paths visible as ranked-out candidates instead of silently broadening scope
  - true narrow fallback is intentionally about **bounded admission**, not broad rescue:
    - a runtime companion should usually survive because it matches a concrete target family or bounded runtime fact
    - generic dynamic runtime files are expected to stay filtered out instead of being kept "just in case"
  - fallback discovery is still intentionally narrow: the goal is bounded explanation, not broad Ruby companion expansion
  - longer `require_relative` ladders, dynamic-send-heavy flows, and broader companion discovery are still intentionally under-modeled
  - treat Ruby PDG / propagation as a bounded explanation aid, not as a complete inter-procedural proof system
- Engine integration is improved but still uneven:
  - diff-mode PDG / propagation now honor the selected engine for changed-symbol discovery **and** strict impact-capability validation, so strict-LSP failure semantics stay aligned with plain impact
  - PDG / propagation still layer extra local graph construction on top of cached graph data, so engine-native edge richness is not fully preserved yet

### Practical guidance
- Start with normal `impact` when you want stable repo-wide caller/callee answers.
- Add `--with-pdg` when you want to ask: "which nearby Rust/Ruby files belong in the bounded explanation slice, and what local data/control edges appear there?"
- Escalate to `--with-propagation` when the interesting question is "does this value/argument/result flow across the call boundary?", especially for short Rust/Ruby multi-file bridges.
- When the PDG / propagation output looks surprising, inspect `summary.slice_selection` first to see which files were selected or pruned, then inspect `impacted_witnesses[*].slice_context` to connect that file-level reason back to the chosen witness path.
- If a bridge choice still looks odd, compare `files[*].reasons[*].scoring` against `pruned_candidates[*].scoring`, then check `slice_context.selected_vs_pruned_reasons` for the compact human-facing explanation.
- If you need to review one seed at a time, add `--per-seed` to any of the above and inspect `impacted_witnesses`, especially the compact witness fields, for the chosen path summary.
- If you are working in Go/Java/Python/JS/TS/TSX, do not assume `--with-pdg` adds much today; treat it as experimental unless the fixture/regression says otherwise.

### PDG Visualization
- Generate PDG in `dot` format with `--with-pdg` and `-f dot`:
  ```
  git diff --no-ext-diff | dimpact impact --with-pdg -f dot
  ```
- Generate the propagation-augmented graph with `--with-propagation` and `-f dot`:
  ```
  git diff --no-ext-diff | dimpact impact --with-propagation -f dot
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
