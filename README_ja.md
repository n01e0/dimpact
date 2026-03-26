# dimpact

変更差分から影響範囲を抽出するための、高速な多言語対応 impact analysis ツール。

`git diff` あるいは seed symbol を入力すると、変更されたシンボル、影響を受けるシンボル、影響ファイル、必要に応じてエッジを返します。主目的は **「この diff でどこが影響を受けるか」** を出すことです。

## Installation

### ソースからビルド

```bash
cargo build --release
```

生成バイナリ:

```bash
./target/release/dimpact
```

### シェル補完

```bash
dimpact completions bash > /tmp/dimpact.bash
source /tmp/dimpact.bash
```

## Usage

### 1. diff をパース

```bash
git diff --no-ext-diff | dimpact diff -f json
```

### 2. changed symbols を出す

```bash
git diff --no-ext-diff | dimpact changed -f json
```

### 3. diff から impact analysis

```bash
git diff --no-ext-diff | dimpact impact --direction callers --with-edges -f json
```

### 4. seed symbol から impact analysis

```bash
dimpact impact --seed-symbol 'rust:src/lib.rs:fn:foo:12' --direction callers -f json
dimpact impact --seed-json '["typescript:src/a.ts:fn:run:10"]' -f json
```

### 5. changed / seed ごとに分けて見る

```bash
git diff --no-ext-diff | dimpact impact --per-seed --direction both --with-edges -f json
```

### 6. Symbol ID を生成

```bash
# ファイル内の id を列挙
dimpact id --path src/lib.rs --raw

# 行で絞る
dimpact id --path src/lib.rs --line 120 -f json

# 名前 + kind で絞る
dimpact id --path src/lib.rs --name foo --kind fn --raw
```

## 主な機能

- **diff ベースの impact analysis**
  - changed symbols / impacted symbols / impacted files / optional edges
- **seed ベースの impact analysis**
  - diff なしでも Symbol ID や JSON で解析可能
- **per-seed grouping**
  - changed / seed ごとの波及を個別に確認できる
- **summary 出力**
  - `summary.by_depth`
  - `summary.risk`
  - `summary.affected_modules`
- **PDG / propagation 補強**
  - `--with-pdg`
  - `--with-propagation`
- **複数出力形式**
  - `json`, `yaml`, `dot`, `html`
- **engine 選択**
  - `--engine auto|ts|lsp`

## summary 出力

`impact` の JSON/YAML には、一次判断用の `summary` が入ります。

- `summary.by_depth`
  - direct / transitive の分離
- `summary.risk`
  - 軽量な triage ヒント
- `summary.affected_modules`
  - impacted symbol の path-based grouping

例:

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
  - 通常の impact traversal に、ローカルな PDG/DFG 風の文脈を足す
- `--with-propagation`
  - PDG の上に propagation bridge を足す
- `--per-seed`
  - 通常 impact でも PDG / propagation でも使える

現在の実用レンジ:

- いま特に強いのは **Rust** と **Ruby**
- まだ **bounded** で、完全な project-wide PDG ではない
- 実態としては **通常 impact traversal + bounded PDG / propagation augmentation** に近い

## よく使うオプション

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

CLI 全体はこれで確認できます。

```bash
dimpact --help
dimpact impact --help
dimpact id --help
```

## Limitations

- PDG / propagation は full project-wide whole-program analysis ではない
- PDG / propagation が特に強いのは現在 Rust / Ruby
- `summary.affected_processes` は未実装

## strict real-LSP の対象言語

README は短く保つけど、このセクションだけは CI が同期確認に使っているので残します。

- `DIMPACT_E2E_STRICT_LSP_TYPESCRIPT`
- `DIMPACT_E2E_STRICT_LSP_JAVASCRIPT`
- `DIMPACT_E2E_STRICT_LSP_RUBY`
- `DIMPACT_E2E_STRICT_LSP_GO`
- `DIMPACT_E2E_STRICT_LSP_JAVA`
- `DIMPACT_E2E_STRICT_LSP_PYTHON`

strict LSP / graduation の詳細は `docs/` と `scripts/verify-lsp-graduation.sh` を参照してください。

## Docs

長い設計メモ、rollup、評価ドキュメント、実装詳細は `docs/` に置いてあります。

## License

MIT
