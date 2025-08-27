# Repository Guidelines

このリポジトリは Rust 製の CLI ツール「dimpact」です。git の差分を解析し、変更行→シンボル→影響範囲を算出します。先輩、最小の手順で気持ちよくコントリビュートできるようにまとめましたっ。

## Project Structure & Module Organization
- `src/`: コア実装
  - `bin/dimpact.rs`: CLI エントリーポイント（stdin の diff を受け取ります）
  - `diff.rs`: 統一 diff パーサ
  - `mapping.rs`: 変更行→シンボル対応づけ
  - `impact.rs`: 参照グラフ構築と影響分析
  - `languages/*`: 言語アナライザ（Rust/Ruby, tree‑sitter ベース）
  - `ir.rs`: シンボル/参照の中間表現
- `resources/specs/*.yml`: tree‑sitter クエリ定義
- `tests/`: 統合テスト（CLI を実行して検証）
- `scripts/`: 補助スクリプト（`scripts/dimpact-e2e.sh` など）

## Build, Test, and Development Commands
- ビルド: `cargo build`
- テスト一式: `cargo test`
- 単体テスト例: `cargo test --test cli_impact_mod_rs`
- 実行（JSON 出力）: `git diff --no-ext-diff | cargo run --quiet --bin dimpact -- --format json`
- 変更シンボル: `... --mode changed --lang auto`
- 影響解析: `... --mode impact --direction callers --max-depth 2 --with-edges`

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
- 解析対象はカレントディレクトリ配下の `*.rs`/`*.rb`。`.git` と `target` は自動除外。
- CLI は常に stdin の diff を期待します（端末入力のみはエラー）。スクリプトやパイプでの実行を推奨ですっ。

