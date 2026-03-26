# dimpact

`dimpact` is a CLI for language-aware impact analysis from git diffs or explicit symbol seeds.
It maps changed lines to symbols, builds reference relationships, and reports what code is likely affected.

## What it does

- Parse unified git diffs from stdin
- Detect changed symbols in Rust, Ruby, Python, JavaScript, TypeScript, TSX, Go, and Java
- Compute caller / callee impact from a diff or from seed symbols
- Output JSON, YAML, DOT, or HTML
- Generate Symbol IDs from file / line / name
- Persist analysis data with a local SQLite cache
- Use Tree-Sitter by default, with an LSP engine available when needed

## Installation

### Build from source

```bash
cargo build --release
./target/release/dimpact --help
```

### Install into your Cargo bin directory

```bash
cargo install --path .
dimpact --help
```

## Basic usage

### 1. Parse a diff

```bash
git diff --no-ext-diff | dimpact diff -f json
```

### 2. Show changed symbols

```bash
git diff --no-ext-diff | dimpact changed --lang auto -f json
```

### 3. Compute impact from a diff

```bash
git diff --no-ext-diff | dimpact impact --direction callers --with-edges -f json
```

### 4. Compute impact from explicit seeds

```bash
dimpact impact \
  --seed-symbol 'rust:src/lib.rs:fn:foo:12' \
  --direction callers \
  -f json
```

### 5. Generate Symbol IDs

```bash
dimpact id --path src/lib.rs --name foo --kind fn --raw
```

## Main commands

| Command | Purpose |
| --- | --- |
| `diff` | Parse unified diff input from stdin |
| `changed` | Resolve changed lines to symbols |
| `impact` | Compute callers / callees / both from diff or seeds |
| `id` | Generate Symbol IDs from file, line, and name |
| `cache` | Build, update, inspect, or clear the local cache |
| `completions` | Generate shell completion scripts |

## Useful options

- `--direction callers|callees|both`
- `--with-edges`
- `--max-depth N`
- `--engine auto|ts|lsp`
- `--seed-symbol LANG:PATH:KIND:NAME:LINE`
- `--seed-json <json|path|->`
- `-f json|yaml|dot|html`

## Cache

`dimpact` can persist symbols and reference edges in SQLite for faster repeated analysis.

```bash
dimpact cache build --scope local
dimpact cache stats --scope local
```

Default local cache path:

```text
.dimpact/cache/v1/index.db
```

## Notes

- `diff`, `changed`, and diff-based `impact` expect unified diff input on stdin.
- Seed-based `impact` does not require stdin.
- Tree-Sitter is the default engine.
- LSP mode is available via `--engine lsp`.

## Advanced docs

For LSP graduation notes, strict-LSP workflow details, and design memos, see [`docs/`](docs/).

## License

MIT. See [LICENSE](LICENSE).
