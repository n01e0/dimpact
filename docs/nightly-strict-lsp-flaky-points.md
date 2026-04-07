# Nightly strict real-LSP flaky points (ALL57-1)

対象: `.github/workflows/nightly-strict-lsp.yml`

このメモは **server install / startup / timeout** 観点で、現状の flaky 要因候補を抽出したもの。
（対策実装は ALL57-2 以降で実施）

Phase A（env/server 起因の fail-fast 移行）以降は、nightly summary で以下を常時追跡する。
- `fail-fast 昇格済み`
- `skip-safe 残件`

## Q54 現在地（Q54-1〜Q54-4 反映）

### Q54-1 再棚卸し（言語×方向）
- 参照: `release-notes/0.5.4-strict-real-lsp-skip-safe-inventory-q54-1.md`
- 集計:
  - skip prints: `19`
  - residual lanes: `18`
  - category totals: `env=15 / server=4 / capability=0 / other=0`
  - actionable residual lanes: `0`

### Q54-2〜Q54-4 昇格結果
- 参照:
  - `release-notes/0.5.4-ts-js-fail-fast-promotion-q54-2.md`
  - `release-notes/0.5.4-go-java-fail-fast-promotion-q54-3.md`
  - `release-notes/0.5.4-ruby-python-fail-fast-promotion-q54-4.md`
- nightly lane policy (`STRICT_LSP_ACTIVE_LANES`) は段階的に
  - `go` → `go,typescript,javascript` → `go,typescript,javascript,java` → `go,typescript,javascript,java,ruby,python`
- 現在は strict real-LSP 対象 6言語（TS/JS/Go/Java/Ruby/Python）すべてが active fail-fast 側。

### 運用上の残件（Q54-5 時点）
- policy-disabled lane はなし。
- 残る skip-safe は運用要因（install/server availability）起因の実行時事象として追跡。
- capability/other の actionable residual は Q54-1 集計時点で 0。

## 1) Server install 由来の flaky 候補

- **latest/snapshot 依存で再現性が揺れる**
  - `ghcr.io/n01e0/dimpact-ci:latest`（L20）
  - `go install .../gopls@latest`（L51）
  - `jdt-language-server-latest.tar.gz`（L63）
  - `npm install -g ...` / `gem install ...`（L37, L42, L82）
- **外部ネットワーク依存に retry がない**
  - npm/go/curl/gem いずれも単発実行で、瞬断・レート制限・mirror揺れ時に失敗しやすい。
- **`$GITHUB_PATH` の反映タイミング依存**
  - Go: `echo ... >> $GITHUB_PATH` の直後に `gopls version` 実行（L52-L53）
  - Java: `echo ... >> $GITHUB_PATH` の直後に `jdtls --help` 実行（L73-L74）
  - 同一 step 内では PATH 反映に依存差が出る可能性がある。

## 2) Server startup 由来の flaky 候補

- **install 後の確認が `--help` のみ**
  - 各 server の起動可否/ハンドシェイク（initialize）を検証していないため、
    `engine_lsp` 実行時に初めて startup failure が顕在化する。
- **全言語を一括で strict E2E 実行**
  - `DIMPACT_E2E_STRICT_LSP=1 cargo test -q --test engine_lsp`（L85-L94）
  - 1言語の startup 遅延/不安定でジョブ全体が赤化しやすい。
- **ウォームアップ不足**
  - server の first-start コスト（index/build cache）を吸収する preflight が無い。

## 3) Timeout 由来の flaky 候補

- **job/step の明示 timeout が無い**
  - install 群・`cargo test` とも `timeout-minutes` / `timeout` wrapper 未設定。
  - ハング時に失敗までの時間が長く、再実行判断が遅れる。
- **逐次セットアップの累積遅延**
  - TS/JS → Python → Go → Java → Ruby の順で serial 実行（L35-L83）。
  - 外部要因で遅延が積み上がると nightly 窓で揺れやすい。
- **言語別のタイムアウト隔離が無い**
  - 言語ごとの切り分け失敗で、timeout 原因の特定に時間がかかる。

## 4) 優先度メモ（抽出時点）

- **High**: latest/snapshot依存、retryなし、全言語一括実行、timeout未設定
- **Medium**: PATH反映タイミング依存、`--help`止まりの起動確認
- **Low**: startup の handshake 実検証不足（`--help` ベースの health-check だけでは取り切れないケース）

## 5) nightly 再現手順（ALL57-4）

### GitHub Actions で手動再実行

1. Actions で `Nightly Strict real-LSP E2E` を `workflow_dispatch` 実行
2. 実行後、artifact `nightly-strict-lsp-execution-logs` を取得
3. 次のログを優先確認
   - `install-*.log`（server install/health-check）
   - `preflight.log` / `engine-lsp-strict-preflight.log`（preflight gate 判定理由）
   - `strict-e2e-migration-summary.md`（`skip-safe 残件` / `fail-fast 昇格済み` の集計）
   - `engine-lsp-strict-e2e.log`（strict E2E 本体）
   - `run-graduation-check.log` / `lsp-graduation-check.log`（graduation check）

### ローカルでの最小再現（workflow相当）

```bash
# repo root
npm install -g typescript typescript-language-server pyright
pyright-langserver --help >/dev/null
typescript-language-server --help >/dev/null

go install golang.org/x/tools/gopls@latest
gopls version >/dev/null

# jdtls/ruby-lsp は環境依存のため、導入済みであれば同様に --help を確認

DIMPACT_E2E_STRICT_LSP=1 \
DIMPACT_E2E_STRICT_LSP_TYPESCRIPT=1 \
DIMPACT_E2E_STRICT_LSP_JAVASCRIPT=1 \
DIMPACT_E2E_STRICT_LSP_PYTHON=1 \
cargo test -q --test engine_lsp
```

### 切り分けのコツ

- install 失敗: `install-*.log` の `::error::` 行を先頭に確認
- migration 判定: `strict-e2e-migration-summary.md` で `skip-safe 残件` / `fail-fast 昇格済み` を確認
- preflight 判定: `engine-lsp-strict-preflight.log` で gate 理由（`enabled=... reason=...`）を確認
- strict 本体失敗: `engine-lsp-strict-e2e.log` の末尾 + `$GITHUB_STEP_SUMMARY` を併読

## 6) 運用フロー（triage / retry / escalation）（ALL63-4）

現在の nightly は、失敗時に以下の順で自動処理する。

### 6.1 triage（自動分類）

1. `scripts/classify-nightly-flaky.sh` が `nightly-logs/*.log` を走査
2. flaky を分類し、retry 実行後は retry 吸収済みの install を再分類する
   - `install`
   - `retry_absorbed`
   - `startup`
   - `logic`
   - `capability`
   - `timeout`
3. 生成物
   - `nightly-flaky-classification.json`
   - `nightly-flaky-classification.md`
4. retry 前の分類は retry policy 判定に使い、retry 後に最終分類を上書きする
   - `nightly-flaky-classification-initial.json/.md`: retry policy 用スナップショット
   - `nightly-flaky-classification.json/.md`: retry 吸収後の最終分類
5. CI summary には初回分類結果が自動追記され、failure triage は最終分類を使う

### 6.2 retry（タイプ別ポリシー）

分類結果をもとに、workflow が再試行可否を自動判定する。

- `install` > 0: **retry 1回**（setup系を best-effort 再実行）
- `startup` > 0: **retry 1回**
- `timeout` > 0: **retry 1回**
- `logic` / `capability` のみ: **retry しない**（非一時要因扱い）

再試行時は `engine_lsp` strict E2E / graduation check を policy-gated で再実行し、
初回 + retry を統合して最終成功判定する。retry 後は Ruby install のような
setup 失敗が回復したケースを `retry_absorbed` へ移し、残った strict failure を
`logic` / `startup` / `timeout` として見直す。

### 6.3 failure summary（原因/言語/再現）

失敗時は `scripts/summarize-nightly-failure.sh` が triage 情報を要約し、
CI summary に以下の表を出す。

- cause type
- language
- evidence（`file:line` + snippet）
- repro step（その場で叩けるコマンド）

生成物:
- `nightly-failure-triage.md`

### 6.4 escalation（人手対応の条件）

次の条件では、retry 成否に関係なく escalation 対象とする。

- 同一カテゴリが 3回以上連続（例: startup timeout が連続）
- `capability` が連続（設定/機能差分の恒久問題の可能性）
- `install` 失敗が複数言語で同時発生（runner/container 側障害の可能性）
- retry 後も `strict_e2e` または `graduation` が失敗

escalation 時の最小提出セット:

1. `nightly-strict-lsp-execution-logs` artifact 一式
2. `nightly-flaky-classification.json`
3. `nightly-failure-triage.md`
4. `strict-e2e-migration-summary.md`
5. 該当 run URL / commit SHA / 失敗カテゴリ

---

フォローアップ候補:
- preflight に startup handshake（initialize）検証を追加
- capability 判定の言語別粒度をさらに細分化
