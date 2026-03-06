# Nightly strict real-LSP flaky points (ALL57-1)

対象: `.github/workflows/nightly-strict-lsp.yml`

このメモは **server install / startup / timeout** 観点で、現状の flaky 要因候補を抽出したもの。
（対策実装は ALL57-2 以降で実施）

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
   - `preflight.log` / `engine-lsp-strict-preflight.log`（skip-safe 判定理由）
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
- skip-safe 判定: `engine-lsp-strict-preflight.log` で対象言語の `enabled=0 reason=...` を確認
- strict 本体失敗: `engine-lsp-strict-e2e.log` の末尾 + `$GITHUB_STEP_SUMMARY` を併読

---

フォローアップ候補:
- preflight に startup handshake（initialize）検証を追加
- 言語別 timeout と retry 方針の細分化
