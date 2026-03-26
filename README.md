# dimpact

Fast, language-aware impact analysis for changed code.

Feed `dimpact` a git diff or a set of seed symbols and it returns changed symbols, impacted symbols, impacted files, and optional edges. The main use case is: **given this diff, what code is affected?**

## Installation

### Build from source

```bash
cargo build --release
```

The binary will be available at:

```bash
./target/release/dimpact
```

### Shell completions

```bash
dimpact completions bash > /tmp/dimpact.bash
source /tmp/dimpact.bash
```

## Usage

### 1. Parse a diff

```bash
git diff --no-ext-diff | dimpact diff -f json
```

### 2. Show changed symbols

```bash
git diff --no-ext-diff | dimpact changed -f json
```

### 3. Run impact analysis from a diff

```bash
git diff --no-ext-diff | dimpact impact --direction callers --with-edges -f json
```

### 4. Run impact analysis from seed symbols

```bash
dimpact impact --seed-symbol 'rust:src/lib.rs:fn:foo:12' --direction callers -f json
dimpact impact --seed-json '["typescript:src/a.ts:fn:run:10"]' -f json
```

### 5. Group impact per changed / seed symbol

```bash
git diff --no-ext-diff | dimpact impact --per-seed --direction both --with-edges -f json
```

### 6. Generate Symbol IDs

```bash
# list ids in a file
dimpact id --path src/lib.rs --raw

# narrow by line
dimpact id --path src/lib.rs --line 120 -f json

# narrow by name + kind
dimpact id --path src/lib.rs --name foo --kind fn --raw
```

## Key features

- **Diff-based impact analysis**
  - changed symbols, impacted symbols, impacted files, and optional edges
- **Seed-based impact analysis**
  - run impact without a diff by passing Symbol IDs or JSON
- **Per-seed grouping**
  - inspect how each changed / seed symbol fans out independently
- **Summary output**
  - `summary.by_depth`
  - `summary.risk`
  - `summary.affected_modules`
- **PDG / propagation augmentation**
  - `--with-pdg`
  - `--with-propagation`
- **Multiple output formats**
  - `json`, `yaml`, `dot`, `html`
- **Engine selection**
  - `--engine auto|ts|lsp`

## Summary output

`impact` JSON/YAML includes a `summary` block for quick triage:

- `summary.by_depth`
  - direct vs transitive spread
- `summary.risk`
  - lightweight triage hint
- `summary.affected_modules`
  - path-based grouping of impacted symbols

Example shape:

```json
{
  "changed_symbols": [...],
  "impacted_symbols": [...],
  "impacted_files": [...],
  "edges": [...],
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
      { "module": "src/engine", "symbol_count": 4, "file_count": 2 }
    ]
  }
}
```

## PDG / propagation

- `--with-pdg`
  - adds local PDG/DFG-style context on top of normal impact traversal
- `--with-propagation`
  - adds propagation bridges on top of PDG
- `--per-seed`
  - works with normal impact and the PDG / propagation path

Current practical scope:

- strongest today in **Rust** and **Ruby**
- still **bounded**, not a full project-wide PDG
- best understood as **normal impact traversal + bounded PDG / propagation augmentation**

## Common options

```text
--direction callers|callees|both
--with-edges
--per-seed
--with-pdg
--with-propagation
--engine auto|ts|lsp
--min-confidence confirmed|inferred|dynamic-fallback
-f json|yaml|dot|html
```

For the full CLI surface, use:

```bash
dimpact --help
dimpact impact --help
dimpact id --help
```

## Limitations

- PDG / propagation is not full project-wide whole-program analysis
- PDG / propagation is currently strongest in Rust and Ruby
- `summary.affected_processes` is not implemented

## strict real-LSP target languages

The README keeps this section intentionally short, but CI expects the target-language env gates to stay discoverable here.

- `DIMPACT_E2E_STRICT_LSP_TYPESCRIPT`
- `DIMPACT_E2E_STRICT_LSP_JAVASCRIPT`
- `DIMPACT_E2E_STRICT_LSP_RUBY`
- `DIMPACT_E2E_STRICT_LSP_GO`
- `DIMPACT_E2E_STRICT_LSP_JAVA`
- `DIMPACT_E2E_STRICT_LSP_PYTHON`

For the full strict LSP / graduation details, see `docs/` and `scripts/verify-lsp-graduation.sh`.

## Docs

Long-form design notes, rollups, evaluation docs, and implementation details live in `docs/`.

## License

MIT
