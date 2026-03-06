# LSP experimental 卒業基準

このドキュメントは `--engine lsp` を experimental 扱いから昇格させるための判断基準を定義する。

## 卒業判定の必須条件

以下を **すべて満たした時点** を卒業候補とする。

1. **strict E2E 言語カバレッジ完了**
   - Go / Java / TypeScript / JavaScript / Ruby / Python の strict real-LSP E2E が存在する
   - callers / callees / both の 3 方向を skip-safe で検証できる

2. **統合回帰の継続安定**
   - `cargo test -q --test engine_lsp` が継続的に green
   - `cargo test -q` が継続的に green
   - `cargo clippy -q --all-targets -- -D warnings` が継続的に green

3. **bench guardrail の安定運用**
   - Rust baseline + Go/Java/Python strict-LSP bench ジョブが有効
   - JSON/TXT artifact が保存され、失敗時ログで閾値不足が判読可能
   - 閾値更新ポリシー（段階的調整 + safety floor）に従って運用されている

4. **運用ドキュメント整備**
   - README / README_ja に strict E2E 実行条件（言語サーバー + env gate）が反映済み
   - bench 実行手順と注意点（既存 TS/Rust 運用との整合）が反映済み

## 卒業判定時の確認チェックリスト

- [ ] strict E2E: 6言語 × 3方向（callers/callees/both）を実装済み
- [ ] CI で engine_lsp / test / clippy が連続 green
- [ ] bench artifact（txt/json）を確認できる
- [ ] guardrail 失敗ログが不足メトリクスを明示している
- [ ] README / README_ja の実行条件が最新実装と一致している

## 判定メモ

- 卒業判定は「単発成功」ではなく「継続安定」を重視する。
- 閾値を急に厳格化せず、小刻み調整で CI 安定性を優先する。
- language server 未導入環境を考慮し、real-LSP E2E は skip-safe 方針を維持する。
