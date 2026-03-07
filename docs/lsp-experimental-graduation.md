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

## skip-safe 移行ポリシー（PH65）

strict real-LSP E2E は一律で fail-fast 化せず、言語/方向ごとに段階移行する。

### 1) skip-safe を **維持**する条件

以下のいずれかを満たすレーンは skip-safe 維持。

- `server-missing`（言語サーバー未導入）
- `env-gate-disabled`（実行ゲート未有効）
- `*-not-reported`（LSP が callers/callees/both を返さない）
- `*-unavailable`（changed_symbols / impact が環境依存で不安定）
- nightly で `install/startup/timeout` が継続的に発生

### 2) skip-safe を **解除（fail-fast 化）**する条件

以下を満たしたレーンから fail-fast へ移行。

1. PH65-1 集計で昇格候補に入っている
2. 同一レーンで `changed/impacted` の安定性が確認済み
3. CI 失敗時に原因が `server/capability/logic` で即判別できる
4. 直近運用で重大な startup/timeout 不安定がない

### 3) 移行順序（段階運用）

- Phase-1: callers レーン優先（最低2言語）
- Phase-2: callees
- Phase-3: both

保守性のため、1PRで広げすぎずレーン単位で移行する。

### 4) fail-fast 化後のロールバック条件

fail-fast 化したレーンで以下が連続した場合は一時的に skip-safe へ戻す。

- startup/timeout 系の失敗が連続
- capability 差分で false alarm が多発
- 原因分類ログで `server` 優勢かつ環境要因が解消していない

ロールバック時は原因分類ログと run URL を添えて記録する。

## GA判定 実行結果（GA52-4）

実行日: 2026-03-06 (Asia/Tokyo)

- 実行コマンド:
  - `scripts/verify-lsp-graduation.sh`
- 実行結果:
  - `PASS=62 FAIL=0`

### 補足（同実行内で確認された統合回帰）
- `cargo test -q --test engine_lsp` ✅
- `cargo test -q` ✅
- `cargo clippy -q --all-targets -- -D warnings` ✅

### 判定
- 現時点の GA 判定チェックはすべて pass。
- required-check 相当の CI レーン（`lsp_graduation_check`）と nightly レーンの両方で同スクリプトを実行する構成。
