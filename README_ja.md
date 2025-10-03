# dimpact

変更が加えられたコードに対する、高速かつ多言語対応(予定)の影響解析ツール。git diff を入力するか、特定のシンボルをシードとして与えることで、変更されたシンボル、その変更によって影響を受けるシンボル、および必要に応じて参照エッジを取得できます。

## ハイライト
- デフォルトで Tree‑Sitter エンジン（Auto）：堅牢かつ高速
- LSP エンジン（実験的）：機能駆動、strict モードでない場合は TS にフォールバック
- 柔軟なシード：Symbol ID または JSON を受け付け
- Symbol ID ジェネレータ：ファイル/行/名前 から ID を解決、フィルタ指定可能

## クイックスタート
```bash
# ビルド
cargo build --release

# 変更差分をパース
git diff --no-ext-diff | dimpact diff -f json

# 変更されたシンボル
git diff --no-ext-diff | dimpact changed -f json

# diff に基づく影響解析（callers、エッジ付き）
git diff --no-ext-diff | dimpact impact --direction callers --with-edges -f json

# シードに基づく影響解析（diff 不要）
dimpact impact --seed-symbol 'rust:src/lib.rs:fn:foo:12' --direction callers -f json
dimpact impact --seed-json '["typescript:src/a.ts:fn:run:10"]' -f json

# 変更シンボルごとにグループ化した影響解析（direction=both、エッジ付き）
git diff --no-ext-diff | dimpact impact --per-seed --direction both --with-edges -f json
```

## Symbol ID ジェネレータ
- ファイル/行/名前 から候補となるシンボル ID を生成し、kind で絞り込み、JSON/YAML またはプレーンテキストで出力します。
```bash
# ファイル内のシンボル ID を一覧表示
dimpact id --path src/lib.rs --raw

# 行番号で絞り込み（JSON 形式）
dimpact id --path src/lib.rs --line 120 -f json

# 名前と kind で絞り込み、単一行のプレーン ID を出力
dimpact id --path src/lib.rs --name foo --kind fn --raw

# ワークスペース全体を名前で検索（JSON 形式）
dimpact id --name foo -f json
```

## CLI 概要
- サブコマンド:
  - `diff`: stdin から unified diff をパース
  - `changed`: diff から変更されたシンボルを抽出
  - `impact`: diff またはシードから影響解析を実行
  - `id`: ファイル/行/名前 からシンボル ID を生成
  - `cache`: インクリメンタルキャッシュを build/update/stats/clear
  - `completions`: シェル補完スクリプトを生成
- シード:
  - `--seed-symbol LANG:PATH:KIND:NAME:LINE` （繰り返し指定可）
  - `--seed-json <json|string|path|->` JSON 文字列・ファイル・stdin を受け付け
  - シード指定時は言語をシードから判定（混在はエラー）
- 出力形式: `-f json|yaml|dot|html`

### Impact オプション（`impact` サブコマンド）
- `--direction callers|callees|both` : 方向 (既定: callers)
- `--max-depth N`               : 最大探索深度 (既定: 100)
- `--with-edges`                : 参照エッジを出力に含める
- `--ignore-dir DIR`            : 相対パスプレフィックスでディレクトリを無視（繰り返し可）
- `--with-pdg`                  : PDG ベースの依存解析を使用 (Rust/Ruby の DFG)
- `--with-propagation`          : 変数・関数をまたいだシンボリック伝播を有効化 (PDG を含む)
- `--engine auto|ts|lsp`        : 分析エンジン (既定: auto → TS)
- `--engine-lsp-strict`         : strict モードで LSP を実行（フォールバックなし）
- `--engine-dump-capabilities`  : エンジンの機能一覧を stderr に出力
- `--seed-symbol LANG:PATH:KIND:NAME:LINE` : ID ベースのシード (繰り返し可)
- `--seed-json PATH|'-'|JSON`   : JSON 配列やファイル・stdin でシード
- `--per-seed`                  : 変更/シードごとに結果をグループ化; `--direction both` 時は caller/callee 別出力

## PDG 可視化
`--with-pdg` と `-f dot` を組み合わせて PDG を dot 形式で出力できます。
```bash
git diff --no-ext-diff | dimpact impact --with-pdg -f dot
```

## DOT/HTML での経路ハイライト
- `--with-edges` 指定時、DOT/HTML 出力で変更シンボルから影響シンボルへの最短経路上のエッジがハイライトされます。
- 変更箇所から影響範囲への伝播ルートを視覚的に追いやすくします。
- HTML ビューにはフィルタや自動レイアウト機能があり、ハイライトされた経路エッジは赤色で表示されます。

## エンジン選択
- Auto: デフォルトで Tree‑Sitter エンジン (推奨)
- LSP (実験的): `--engine lsp`
  - `--engine-lsp-strict`: LSP 課題時に TS にフォールバックしない
  - `--engine-dump-capabilities`: LSP 機能一覧を stderr に出力

## ロギング
`env_logger` を使用。`RUST_LOG=info`（または `debug`/`trace`）で診断ログを有効化。

## 使用例
```bash
# 呼び出し元チェーンをエッジ付き JSON で出力
git diff --no-ext-diff | dimpact impact --direction callers --with-edges -f json

# callee チェーンを深さ 2、YAML 形式で出力
git diff --no-ext-diff | dimpact impact --direction callees --max-depth 2 -f yaml

# Tree‑Sitter エンジンを強制 (推奨デフォルト)
git diff --no-ext-diff | dimpact impact --engine ts -f json

# strict モード付き LSP エンジン + 機能一覧ダンプ (実験的)
git diff --no-ext-diff | dimpact impact --engine lsp --engine-lsp-strict --engine-dump-capabilities -f json
# Tip: `RUST_LOG=info` で詳細なログを確認

# Symbol ID でシードし、diff 不要で影響解析
dimpact impact --seed-symbol 'rust:src/lib.rs:fn:foo:12' --direction callers -f json

# JSON ファイルでシード
echo '["typescript:src/a.ts:fn:run:10","typescript:src/b.ts:method:App::start:5"]' > seeds.json
dimpact impact --seed-json seeds.json --direction both -f json

# stdin から JSON でシード
printf '[{"lang":"rust","path":"src/lib.rs","kind":"fn","name":"foo","line":12}]' \\
  | dimpact impact --seed-json - --direction callers -f json

# ID を生成して直接パイプ
dimpact id --path src/lib.rs --name foo --kind fn --raw \\
  | dimpact impact --seed-json - --direction callers -f json

# ワークスペース内を名前で検索し、候補 ID を一覧表示
dimpact id --name initialize --raw
```

## ライセンス
本プロジェクトは MIT ライセンスの下で公開されています。詳細は [LICENSE](LICENSE) ファイルを参照してください。

## キャッシュ
- 目的: 影響解析を高速化するため、シンボルと参照エッジを永続化
- 保存場所: 単一の SQLite DB `index.db` に保存され、以下のいずれかのディレクトリに配置されます:
  - ローカル (既定): `<repo_root>/.dimpact/cache/v1/index.db`
  - グローバル: `$XDG_CONFIG_HOME/dimpact/cache/v1/<repo_key>/index.db`
- サブコマンドで制御:
  - キャッシュのビルド/再構築: `dimpact cache build --scope local|global [--dir PATH]`
  - 既存キャッシュの更新 (alias `verify`): `dimpact cache update --scope local|global [--dir PATH]`
  - キャッシュ統計の表示: `dimpact cache stats --scope local|global [--dir PATH]`
  - キャッシュのクリア: `dimpact cache clear --scope local|global [--dir PATH]`
- 影響解析統合: TS エンジンはキャッシュをデフォルトで使用。初回は自動でビルドし、以降は変更ファイルのみを更新します。
- 環境変数による上書き: `DIMPACT_CACHE_SCOPE=local|global`, `DIMPACT_CACHE_DIR=/custom/dir`
