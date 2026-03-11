# LSP experimental 卒業基準

このドキュメントは `--engine lsp` を experimental 扱いから昇格させるための判断基準を定義する。

## 卒業判定の必須条件

以下を **すべて満たした時点** を卒業候補とする。

1. **strict E2E 言語カバレッジ完了**
   - Go / Java / TypeScript / JavaScript / Ruby / Python の strict real-LSP E2E が存在する
   - callers / callees / both の 3 方向を実装済み
   - Phase A 仕様に従い、env/server 起因は fail-fast 基本、残る skip-safe は理由付きで追跡されている

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
   - nightly / CI summary で `skip-safe 残件` と `fail-fast 昇格済み` が明示される

## 卒業判定時の確認チェックリスト

- [ ] strict E2E: 6言語 × 3方向（callers/callees/both）を実装済み
- [ ] CI で engine_lsp / test / clippy が連続 green
- [ ] bench artifact（txt/json）を確認できる
- [ ] guardrail 失敗ログが不足メトリクスを明示している
- [ ] README / README_ja の実行条件が最新実装と一致している
- [ ] strict real-LSP migration summary に `skip-safe 残件` / `fail-fast 昇格済み` が出力される

## 判定メモ

- 卒業判定は「単発成功」ではなく「継続安定」を重視する。
- 閾値を急に厳格化せず、小刻み調整で CI 安定性を優先する。

## Phase A 仕様（A1-A10）

strict real-LSP E2E の env/server 起因 skip-safe を段階的に fail-fast へ移行する。

### 1) fail-fast 基本ルール

- **server preflight**（`*-lsp` / `gopls` / `jdtls` など）:
  - 未導入は skip せず fail-fast（cause=server）
- **env gate**:
  - 昇格済み callers レーンでは、明示された不正値/無効値を fail-fast（cause=env）
  - Rust は `A6` 時点で「toward fail-fast」運用（不正値は fail-fast、未設定は opt-in skip 維持）

### 2) 段階移行の順序

- callers 優先で昇格（A7）
- 次に callees / both へ拡張
- 1PR で広げすぎず、レーン単位で CI 安定を確認

### 3) skip-safe を残す条件（Phase A 時点）

以下は実装課題として理由付きで残し、Phase B で解消する。

- `*-not-reported`（LSP が callers/callees/both を返さない）
- `*-unavailable`（changed_symbols / impact が環境依存で不安定）
- 非昇格レーンの env 未設定（opt-in gate）

### 4) 可視化と追跡

- 進捗サマリ生成: `scripts/summarize-strict-e2e-migration-progress.sh`
- CI: `engine_lsp_regression` が Step Summary に進捗を追記
- nightly: preflight 後に同サマリを Step Summary へ追記し、
  `nightly-logs/strict-e2e-migration-summary.md` を保存

### 5) fail-fast 化後のロールバック条件

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

## Q54 quality-gates 反映サマリ

- Q54-1 で strict real-LSP skip-safe 残件を再棚卸し（language × direction）し、actionable residual は 0 を確認。
- Q54-2〜Q54-4 で fail-fast 昇格を段階適用し、nightly policy `STRICT_LSP_ACTIVE_LANES` は
  `go,typescript,javascript,java,ruby,python` まで拡張済み。
- これにより、strict real-LSP 対象6言語は policy 上すべて active fail-fast 側。
- 参照:
  - `release-notes/0.5.4-strict-real-lsp-skip-safe-inventory-q54-1.md`
  - `release-notes/0.5.4-ts-js-fail-fast-promotion-q54-2.md`
  - `release-notes/0.5.4-go-java-fail-fast-promotion-q54-3.md`
  - `release-notes/0.5.4-ruby-python-fail-fast-promotion-q54-4.md`
