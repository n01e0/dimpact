# Repository Guidelines

このリポジトリは Rust 製の CLI ツール「dimpact」です。git の差分を解析し、変更行→シンボル→影響範囲を算出します。シード（Symbol ID/JSON）からの直接解析や、Symbol ID 生成も可能です。

## Project Structure & Module Organization
- `src/`: コア実装
  - `bin/dimpact.rs`: CLI エントリーポイント（subcommands: diff/changed/impact/id）
  - `diff.rs`: 統一 diff パーサ
  - `mapping.rs`: 変更行→シンボル対応づけ
  - `impact.rs`: 参照グラフ構築と影響分析
  - `languages/*`: 言語アナライザ（Rust/Ruby/JS/TS/TSX; Tree‑Sitter ベース）
  - `ir.rs`: シンボル/参照の中間表現
  - `engine.rs`: エンジン選択（Auto/Ts/Lsp）
  - `engine/ts.rs`: Tree‑Sitter エンジン
  - `engine/lsp.rs`: LSP エンジン（Experimental）
- `resources/specs/*.yml`: tree‑sitter クエリ定義
- `tests/`: 統合テスト（CLI を実行して検証）
- `scripts/`: 補助スクリプト（`scripts/dimpact-e2e.sh` など）

## Build, Test, and Development Commands
- ビルド: `cargo build`
- テスト一式: `cargo test`
- 単体テスト例: `cargo test --test cli_impact_mod_rs`
- 実行（JSON 出力）: `git diff --no-ext-diff | cargo run --quiet --bin dimpact -- diff -f json`
- 変更シンボル: `git diff --no-ext-diff | cargo run --quiet --bin dimpact -- changed --lang auto`
- 影響解析（diff）: `git diff --no-ext-diff | cargo run --quiet --bin dimpact -- impact --direction callers --max-depth 2 --with-edges`
- 影響解析（シード）: `cargo run --quiet --bin dimpact -- impact --seed-symbol 'rust:src/lib.rs:fn:foo:12' --direction callers`
- ID 生成: `cargo run --quiet --bin dimpact -- id --path src/lib.rs --line 12 --name foo --kind fn --raw`

## Coding Style & Naming Conventions
- Rust 2024 edition。`cargo fmt` に従う 4 スペースインデント推奨。
- Lint: `cargo clippy -- -D warnings` を推奨。
- ファイル/モジュールはスネークケース、型は UpperCamelCase、関数/変数はスネークケース。
- 公開 API は `src/lib.rs` で再公開しており、既存のエクスポートに揃えてください。

## Testing Guidelines
- フレームワーク: Rust 標準テスト＋統合テスト（`assert_cmd`, `predicates`, `tempfile`, `serial_test`）。
- 命名: 統合テストは `tests/*.rs`。CLI 実行と JSON 構造の検証が中心です。
- 追加テストは最小再現のリポ構成を作り、`git diff --no-ext-diff --unified=0` を stdin に流す形で書いてください。

## Commit & Pull Request Guidelines
- 履歴から明確な規約は見当たりません（推測）。以下を推奨します。
  - サブジェクトは英語の命令形で簡潔に（例: "Add Ruby call resolver"）。
  - 本文に背景・設計意図・影響範囲・テスト方針を記載。
- PR には: 目的、変更点一覧、実行例（コマンドと抜粋出力）、関連 Issue を添付。CI で `cargo build && cargo test` が通ることを確認してくださいね。

## Security & Configuration Tips
- 解析対象はカレントディレクトリ配下の `*.rs`/`*.rb`/`*.js`/`*.ts`/`*.tsx`。`.git`/`target`/`node_modules` は自動除外。
- diff/changed/impact(非シード) は stdin の diff を期待します（端末入力のみはエラー）。impact はシード指定時は diff 不要です。

## CLI 概要（重要オプション）
- Subcommands: `diff`, `changed`, `impact`, `id`（従来の `--mode` は非推奨）
- Seeds:
  - `--seed-symbol LANG:PATH:KIND:NAME:LINE`（複数指定可）
  - `--seed-json <json|string|path|->`（配列: 文字列ID or オブジェクト）
  - シードがあれば言語は自動判定（混在はエラー）
- ID 生成: `--path/--line/--name` はいずれも任意。`--kind fn|method|struct|enum|trait|mod` で絞り込み。`--raw` で ID を複数行出力。
- Engine: `--engine auto|ts|lsp`（Auto=TS 既定、LSP=Experimental）。`--engine-lsp-strict`、`--engine-dump-capabilities`（診断）
