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
- **Low**: ログ粒度不足（現状でも artifact はあるが、言語別前提の切り分けには不足）

---

次タスク対応先:
- ALL57-2: install/health-check の明示化（失敗理由の可視化）
- ALL57-3: strict E2E 前 preflight（skip-safe + reason）
- ALL57-4: nightly ログ集約と再現手順ドキュメント化
