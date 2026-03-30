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

### 6. 登録済み JSON schema を調べる

```bash
dimpact schema --list
dimpact schema --id dimpact:json/v1/impact/default/summary_only/call_graph
dimpact schema resolve impact --per-seed --with-edges --with-propagation
```

## JSON schema surface

`schema` サブコマンド群は、JSON contract を調べるための help / lookup layer です。通常の JSON 出力 shape 自体は変えません。

つまり:

- `dimpact diff -f json` は従来どおり top-level array を返す
- `dimpact changed -f json` は従来どおり top-level object を返す
- `dimpact impact -f json` は従来どおり top-level object を返す
- `dimpact impact --per-seed -f json` は従来どおり top-level array を返す
- `dimpact id -f json` は従来どおり top-level array を返す

通常の JSON 出力には `_schema` / `json_schema` / `data` の wrapper は埋め込みません。

schema layer を直接使うときは次を使います。

- `dimpact schema --list` — 登録済みの canonical schema id と document path を列挙
- `dimpact schema --id <schema-id>` — 1 つの id に対応する concrete JSON Schema document を取得
- `dimpact schema resolve <subcommand> ...` — その JSON command が対応する canonical profile / id / path を解決

Schema document は [`resources/schemas/json/v1/`](resources/schemas/json/v1/) 配下にあります。

## 主なコマンド

| コマンド | 役割 |
| --- | --- |
| `diff` | stdin から unified diff をパース |
| `changed` | 変更行をシンボルへ対応付け |
| `impact` | diff またはシードから callers / callees / both を解析 |
| `id` | ファイル・行・名前から Symbol ID を生成 |
| `schema` | 登録済み JSON schema の list / resolve / fetch |
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
