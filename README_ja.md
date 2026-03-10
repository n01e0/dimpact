# dimpact

現在のバージョン: `0.5.2`

変更が加えられたコードに対する、高速かつ多言語対応(予定)の影響解析ツール。git diff を入力するか、特定のシンボルをシードとして与えることで、変更されたシンボル、その変更によって影響を受けるシンボル、および必要に応じて参照エッジを取得できます。

## ハイライト
- デフォルトで Tree‑Sitter エンジン（Auto）：堅牢かつ高速
- LSP エンジン（GA）：機能駆動、strict モードでない場合は TS にフォールバック
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
  - confidence フィルタ（`--min-confidence` / `--exclude-dynamic-fallback`）適用時、JSON/YAML 出力に `confidence_filter` ブロックが追加されます:
    - `min_confidence`
    - `exclude_dynamic_fallback`
    - `input_edge_count`
    - `kept_edge_count`

### Impact オプション（`impact` サブコマンド）
- `--direction callers|callees|both` : 方向 (既定: callers)
- `--max-depth N`               : 最大探索深度 (既定: 100)
- `--with-edges`                : 参照エッジを出力に含める
- `--min-confidence LEVEL`      : confidence 閾値（`confirmed|inferred|dynamic-fallback`）
- `--exclude-dynamic-fallback`  : `dynamic_fallback` エッジを探索/出力から除外
- `--op-profile PROFILE`        : 運用プリセット（`balanced|precision-first`）
- `--ignore-dir DIR`            : 相対パスプレフィックスでディレクトリを無視（繰り返し可）
- `--with-pdg`                  : PDG ベースの依存解析を使用 (Rust/Ruby の DFG)
- `--with-propagation`          : 変数・関数をまたいだシンボリック伝播を有効化 (PDG を含む)
- `--engine auto|ts|lsp`        : 分析エンジン (既定: auto)
- `--auto-policy compat|strict-if-available` : `--engine auto` 用ポリシー (既定: compat)
- `--engine-lsp-strict`         : strict モードで LSP を実行（フォールバックなし）
- `--engine-dump-capabilities`  : エンジンの機能一覧を stderr に出力
- `--seed-symbol LANG:PATH:KIND:NAME:LINE` : ID ベースのシード (繰り返し可)
- `--seed-json PATH|'-'|JSON`   : JSON 配列やファイル・stdin でシード
- `--per-seed`                  : 変更/シードごとに結果をグループ化; `--direction both` 時は caller/callee 別出力

## 運用 confidence プロファイル（`--op-profile`）
- `balanced`
  - `--min-confidence inferred` を適用（推奨の標準運用モード）
  - 日常運用での recall/precision バランスを重視
- `precision-first`
  - `--min-confidence confirmed` + `--exclude-dynamic-fallback` を適用
  - CI/レビューゲートなど誤判定コストが高い場面向け
- 優先順位:
  - 明示指定フラグ（`--min-confidence` / `--exclude-dynamic-fallback`）がプロファイル既定値を上書きします
- 典型コマンド:
  - balanced:
    - `git diff --no-ext-diff | dimpact impact --direction callers --with-edges --op-profile balanced -f json`
  - precision-first:
    - `git diff --no-ext-diff | dimpact impact --direction callers --with-edges --op-profile precision-first -f json`
  - 取りこぼし調査（recall優先）:
    - `git diff --no-ext-diff | dimpact impact --direction callers --with-edges --min-confidence dynamic-fallback -f json`
- 確認ポイント:
  - JSON/YAML の `confidence_filter.input_edge_count` と `confidence_filter.kept_edge_count` を比較し、想定どおりに除外されたか確認します。

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
- Auto (`--engine auto`) はポリシーで挙動を切り替え可能
  - `compat` (既定): 互換挙動を維持（auto は TS 経路を選択）
  - `strict-if-available`: LSP 経路を優先し、capability/session が不足する場合は理由付きログを出して TS にフォールバック
- LSP (GA): `--engine lsp`
  - `--engine-lsp-strict`: LSP 課題時に TS にフォールバックしない
  - `--engine-dump-capabilities`: LSP 機能一覧を stderr に出力

## Auto policy の運用
- 優先順位: CLI (`--auto-policy`) > 環境変数 (`DIMPACT_AUTO_POLICY`) > 既定値 (`compat`)
- 典型コマンド:
  - 互換デフォルトを明示する場合:
    - `git diff --no-ext-diff | dimpact impact --engine auto --auto-policy compat -f json`
  - strict-if-available を使う場合:
    - `git diff --no-ext-diff | dimpact impact --engine auto --auto-policy strict-if-available -f json`
  - 環境変数で既定を切り替える場合:
    - `export DIMPACT_AUTO_POLICY=strict-if-available`

## ロギング
`env_logger` を使用。`RUST_LOG=info`（または `debug`/`trace`）で診断ログを有効化。

## LSP strict E2E テスト
- strict LSP の E2E テストは env gate による opt-in 運用（言語サーバー未導入環境でもデフォルトCIを安定させるため）。
- 現在の挙動（Phase A/B 同期済み）:
  - strict レーンを有効化し server preflight を通過した後の失敗は **fail-fast**（`server` / `capability` / `logic`）として扱う。
  - `not-reported` / `unavailable` の skip-safe フォールバックは strict real-LSP レーンから除去済み。
  - 残る skip-safe は運用上の最小残件のみ:
    - `env-gate-disabled`（opt-in gate 未有効）
    - `server-missing`（現状は主に `rust-analyzer` 未導入の Rust レーン）
- Rust strict E2E（`callers` / `callees` / `both`、`rust-analyzer` が必要）:
  - 実行: `DIMPACT_E2E_STRICT_LSP=1 cargo test --test engine_lsp`
  - gate の意味: 未設定 => skip、`1` => 実行、明示的な不正値 => preflight で fail-fast。
- strict real-LSP の対象言語: **TypeScript / TSX / JavaScript / Ruby / Go / Java / Python**
- Go strict E2E（`gopls` が必要）:
  - `DIMPACT_E2E_STRICT_LSP_GO=1 cargo test --test engine_lsp`
  - `DIMPACT_E2E_STRICT_LSP=1` でも Go strict E2E が有効になります。
- Java strict E2E（`jdtls` が必要）:
  - `DIMPACT_E2E_STRICT_LSP_JAVA=1 cargo test --test engine_lsp`
  - `DIMPACT_E2E_STRICT_LSP=1` でも Java strict E2E が有効になります。
- TypeScript strict E2E（`typescript-language-server` が必要）:
  - `DIMPACT_E2E_STRICT_LSP_TYPESCRIPT=1 cargo test --test engine_lsp`
  - `DIMPACT_E2E_STRICT_LSP=1` でも TypeScript strict E2E が有効になります。
- JavaScript strict E2E（`typescript-language-server` が必要）:
  - `DIMPACT_E2E_STRICT_LSP_JAVASCRIPT=1 cargo test --test engine_lsp`
  - `DIMPACT_E2E_STRICT_LSP=1` でも JavaScript strict E2E が有効になります。
- TSX strict E2E（`typescript-language-server` が必要）:
  - `DIMPACT_E2E_STRICT_LSP_TSX=1 cargo test --test engine_lsp`
  - `DIMPACT_E2E_STRICT_LSP=1` でも TSX strict E2E が有効になります。
- Ruby strict E2E（`ruby-lsp` が必要）:
  - `DIMPACT_E2E_STRICT_LSP_RUBY=1 cargo test --test engine_lsp`
  - `DIMPACT_E2E_STRICT_LSP=1` でも Ruby strict E2E が有効になります。
- Python strict E2E（`pyright-langserver` / `basedpyright-langserver` / `pylsp` のいずれかが必要）:
  - `DIMPACT_E2E_STRICT_LSP_PYTHON=1 cargo test --test engine_lsp`
  - `DIMPACT_E2E_STRICT_LSP=1` でも Python strict E2E が有効になります。
- Python LSP サーバー選択:
  - 自動検出順: `pyright-langserver` -> `basedpyright-langserver` -> `pylsp`
  - 明示指定: `DIMPACT_PYTHON_LSP=pyright|basedpyright|pylsp`
- real-LSP サーバー導入クイック手順（ローカル）:
  - TypeScript/TSX/JavaScript: `npm install -g typescript typescript-language-server`
  - Python（pyright）: `npm install -g pyright`
  - Go: `go install golang.org/x/tools/gopls@latest`
  - Ruby: `gem install ruby-lsp --no-document`
  - Java（`jdtls`）: `jdtls` を導入して `PATH` に追加（詳細は下記 CI 設定を参照）
- real-LSP サーバー導入（CI）:
  - `nightly-strict-lsp.yml` で TS/TSX/JS/Python/Go/Java/Ruby の server を導入後に `engine_lsp` strict E2E を実行
  - `bench.yml` で strict-LSP ベンチの各言語ジョブごとに server を導入
- skip-safe 残件レポート:
  - 更新: `scripts/summarize-strict-e2e-skips.sh tests/engine_lsp.rs`
  - 最新成果物: `docs/strict-real-lsp-skip-reasons-v0.4.1.md`

## 既知の制約
- strict real-LSP は引き続きホスト/実行環境の前提（server導入、プロジェクト状態、toolchain）に依存します。
- opt-in env gate はデフォルトCIを軽量化するための運用であり、未有効は actionable failure ではなく運用残件として扱います。
- レーン有効化＋preflight通過後は fail-fast で判定し、`not-reported` / `unavailable` の skip-safe には戻しません。
- Python の call 抽出は現在、主要な呼び出し形（`foo()` / `obj.m()` / `self.m()`）を中心に対応しています。
  - 実行時解決が必要な高動的ケースは、現時点では保証対象外です。
- strict モードでは、phase/方向ごとの capability が不足すると、言語/方向/capability ヒント付きの明示エラーを返します。

## Python parity ステータス（P-END-*）
- ✅ P-END-1: strict + mock で `callers` / `callees` / `both` を Python fixture付きテストでカバー。
- ✅ P-END-2: strict + `references/definition` 経路でも `callers` / `callees` / `both` が動作（未実装分岐なし）。
- ✅ P-END-3: real-LSP opt-in E2E を環境変数ゲート付きで追加済み（`DIMPACT_E2E_STRICT_LSP_PYTHON` / `DIMPACT_E2E_STRICT_LSP`）。
- ✅ P-END-4: Python strict 運用方法を `README.md` / `README_ja.md` に記載済み。

## 使用例
```bash
# 呼び出し元チェーンをエッジ付き JSON で出力
git diff --no-ext-diff | dimpact impact --direction callers --with-edges -f json

# callee チェーンを深さ 2、YAML 形式で出力
git diff --no-ext-diff | dimpact impact --direction callees --max-depth 2 -f yaml

# Tree‑Sitter エンジンを強制 (推奨デフォルト)
git diff --no-ext-diff | dimpact impact --engine ts -f json

# strict モード付き LSP エンジン + 機能一覧ダンプ (GA)
git diff --no-ext-diff | dimpact impact --engine lsp --engine-lsp-strict --engine-dump-capabilities -f json
# Tip: `RUST_LOG=info` で詳細なログを確認

# policy 差分ベンチ（TS固定 vs auto strict-if-available）
scripts/bench-impact-engines.sh --base origin/main --runs 3 --direction callers --lang rust --compare-auto-strict-if-available
# 固定 diff ファイルを使って比較
scripts/bench-impact-engines.sh --diff-file /tmp/dimpact.diff --runs 3 --lang rust --compare-auto-strict-if-available
# 第2経路の RPC メソッド呼び出し回数も出力
scripts/bench-impact-engines.sh --base origin/main --runs 1 --rpc-counts --compare-auto-strict-if-available
# 最小件数ガード（第2経路が閾値未満なら失敗）
scripts/bench-impact-engines.sh --base origin/main --runs 1 --min-lsp-changed 40 --min-lsp-impacted 15 --compare-auto-strict-if-available
# NOTE: `--compare-auto-strict-if-available` を外すと従来どおり TS vs LSP(strict) 比較
# Go strict-LSP ベンチ（`gopls` が必要）
scripts/bench-impact-engines.sh --diff-file bench-fixtures/go-heavy.diff --runs 1 --direction callers --lang go --min-lsp-changed 6 --min-lsp-impacted 15
# Java strict-LSP ベンチ（`jdtls` が必要）
scripts/bench-impact-engines.sh --diff-file bench-fixtures/java-heavy.diff --runs 1 --direction callers --lang java --min-lsp-changed 7 --min-lsp-impacted 15
# CI ワークフロー: Benchmark Impact Engines（rust + Go + Java strict-LSP ジョブを実行）
# 運用上の注意（既存 TS/Rust 運用との整合）
# - Rust 既存ベンチ（`--base origin/main --lang rust`）を基準運用として維持する
# - Go/Java は固定 heavy diff fixture を使う追加 guardrail で、Rust ベースラインの代替ではない
# - 閾値は言語/fixture ごとに別管理し、Rust と Go/Java の絶対件数を直接比較しない
# - 閾値調整は小刻みに行い、既存 TS/Rust CI の安定性を優先する

# strict LSP を oracle とした差分比較（候補エンジンとの差分）
scripts/compare-impact-vs-lsp-oracle.sh --base origin/main --direction callers --lang rust --report-json /tmp/oracle-diff.json
# 固定 diff + 差分があれば失敗
scripts/compare-impact-vs-lsp-oracle.sh --diff-file /tmp/dimpact.diff --lang rust --with-edges --fail-on-diff

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
