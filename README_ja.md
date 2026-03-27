# dimpact

`dimpact` は、git diff や明示的なシンボル指定を入力として、変更の影響範囲を解析する CLI ツールです。
変更行をシンボルへ対応付けし、参照関係をたどって、どのコードが影響を受けそうかを出力します。

## できること

- unified diff を stdin から解析
- Rust / Ruby / Python / JavaScript / TypeScript / TSX / Go / Java の変更シンボルを検出
- diff またはシードシンボルから callers / callees の影響解析を実行
- JSON / YAML / DOT / HTML で出力
- ファイル / 行 / 名前から Symbol ID を生成
- SQLite ベースのローカルキャッシュで解析を高速化
- 既定では Tree-Sitter、必要に応じて LSP エンジンも利用可能

## インストール

### ソースからビルド

```bash
cargo build --release
./target/release/dimpact --help
```

### crates.io から Cargo でインストール

```bash
cargo install dimpact
dimpact --help
```

### Docker でインストール

```bash
docker pull ghcr.io/n01e0/dimpact:latest
docker run --rm ghcr.io/n01e0/dimpact:latest --help
```

カレントリポジトリをコンテナから解析したいときは、作業ツリーを `/work` にマウントして diff を stdin で流します。

```bash
git diff --no-ext-diff | docker run -i --rm -v "$PWD":/work ghcr.io/n01e0/dimpact:latest impact --direction callers --with-edges -f json
```

## 基本的な使い方

### 1. diff をパースする

```bash
git diff --no-ext-diff | dimpact diff -f json
```

### 2. 変更されたシンボルを出す

```bash
git diff --no-ext-diff | dimpact changed --lang auto -f json
```

### 3. diff から影響解析する

```bash
git diff --no-ext-diff | dimpact impact --direction callers --with-edges -f json
```

### 4. シードシンボルから影響解析する

```bash
dimpact impact \
  --seed-symbol 'rust:src/lib.rs:fn:foo:12' \
  --direction callers \
  -f json
```

### 5. Symbol ID を生成する

```bash
dimpact id --path src/lib.rs --name foo --kind fn --raw
```

## 主なコマンド

| コマンド | 役割 |
| --- | --- |
| `diff` | stdin から unified diff をパース |
| `changed` | 変更行をシンボルへ対応付け |
| `impact` | diff またはシードから callers / callees / both を解析 |
| `id` | ファイル・行・名前から Symbol ID を生成 |
| `cache` | キャッシュの build / update / stats / clear |
| `completions` | シェル補完スクリプトを生成 |

## よく使うオプション

- `--direction callers|callees|both`
- `--with-edges`
- `--max-depth N`
- `--engine auto|ts|lsp`
- `--seed-symbol LANG:PATH:KIND:NAME:LINE`
- `--seed-json <json|path|->`
- `-f json|yaml|dot|html`

## キャッシュ

解析済みのシンボルと参照エッジを SQLite に保存して、繰り返し実行を高速化できます。

```bash
dimpact cache build --scope local
dimpact cache stats --scope local
```

既定のローカルキャッシュ保存先:

```text
.dimpact/cache/v1/index.db
```

## 補足

- `diff` / `changed` / diff ベースの `impact` は stdin の unified diff を前提とします。
- シードベースの `impact` は stdin 不要です。
- 既定エンジンは Tree-Sitter です。
- LSP モードは `--engine lsp` で利用できます。

## 詳細ドキュメント

strict-LSP 運用や設計メモなど、詳細な資料は [`docs/`](docs/) を参照してください。

## ライセンス

MIT. 詳細は [LICENSE](LICENSE) を参照してください。
