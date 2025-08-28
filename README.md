# dimpact

Fast, language-aware impact analysis for changed code. Feed it a git diff or seed it with specific symbols; get back changed symbols, impacted symbols, and optional reference edges.

Highlights
- Tree‑Sitter engine by default (Auto): robust and fast
- LSP engine (experimental): capability‑driven with TS fallback in non‑strict
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
  - `impact`: compute impact from stdin diff or seeds
  - `id`: generate Symbol IDs from file/line/name
  - `cache`: build/stats/clear the incremental cache
- Seeds:
  - `--seed-symbol LANG:PATH:KIND:NAME:LINE` (repeatable)
  - `--seed-json <json|string|path|->` accepts array of strings or objects
  - When seeds are present, language is inferred from seeds (mixed languages error)
- Output formats: `-f json|yaml|dot|html`

Engine Selection
- Auto: Tree‑Sitter by default (recommended)
- LSP (experimental): `--engine lsp`
  - `--engine-lsp-strict`: don’t fall back to TS on LSP issues
  - `--engine-dump-capabilities`: emit LSP capabilities JSON to stderr

Logging
- Uses `env_logger`. Set `RUST_LOG=info` (or `debug|trace`) to see diagnostics.

Usage Examples
- Callers from a diff (JSON with edges):
  - `git diff --no-ext-diff | dimpact impact --direction callers --with-edges -f json`
- Callees limited to depth 2 (YAML):
  - `git diff --no-ext-diff | dimpact impact --direction callees --max-depth 2 -f yaml`
- Force Tree‑Sitter engine (recommended default):
  - `git diff --no-ext-diff | dimpact impact --engine ts -f json`
- Try LSP engine (experimental) with strict mode and capability dump:
  - `git diff --no-ext-diff | dimpact impact --engine lsp --engine-lsp-strict --engine-dump-capabilities -f json`
  - Tip: `RUST_LOG=info` to see more diagnostics
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
- See repository license if present; otherwise contact maintainers.
Cache
- Purpose: persist symbols and reference edges to speed up impact analysis.
- Storage: single SQLite DB `index.db` stored in either location:
  - Local (default): `<repo_root>/.dimpact/cache/v1/index.db`
  - Global: `$XDG_CONFIG_HOME/dimpact/cache/v1/<repo_key>/index.db`
- Control via subcommands:
  - Build: `dimpact cache build --scope local|global [--dir PATH]`
  - Stats: `dimpact cache stats --scope local|global [--dir PATH]`
  - Clear: `dimpact cache clear --scope local|global [--dir PATH]`
- Impact integration: the TS engine uses the cache by default. On first use it builds the cache; on subsequent runs it updates only changed files.
- Env overrides: `DIMPACT_CACHE_SCOPE=local|global`, `DIMPACT_CACHE_DIR=/custom/dir`.
