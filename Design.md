# LSP ベース解析エンジン設計（動的フォールバック版）

## 背景と目的
- 現状は Tree‑Sitter（TS）＋独自ヒューリスティクスで参照解決し、毎回ワークスペース全体の走査が必要。大規模化でコスト/精度の両面が課題。
- 目的は以下。
  - 参照解決の精度向上（型/モジュール/マクロ等）。
  - サーバ側インデックスの再利用による高速化。
  - 既存 CLI/出力スキーマの互換維持（破壊的変更なし）。

## 方針（動的フォールバック）
- 言語や機能ごとに固定せず、実行時に LSP 能力を検出・プローブして最良経路を選択。利用不可/失敗時は段階的に次善策へフォールバック。
- 新たに「解析エンジン」抽象を導入し、TS 実装と LSP 実装を差し替え可能にする。
- CLI に `--engine auto|ts|lsp` を追加。既定は `auto`（可能なら LSP、無理なら自動で TS）。

## 能力検出とプローブ
- 初期化応答 `initialize` の `serverCapabilities` を CapabilityMatrix に格納。
  - `callHierarchyProvider` / `referencesProvider` / `definitionProvider` / `documentSymbolProvider` 等を確認。
- `client/registerCapability` を処理し、動的登録で能力を更新。
- 機能ごとに軽いプローブを実施（例：`prepareCallHierarchy` を1回呼び `null`/`-32601`/timeout を検出）。

## アーキテクチャ
```
src/
  engine/
    mod.rs           // EngineKind, AnalysisEngine, factory
    ts.rs            // 既存コードの薄ラッパ
    lsp/
      mod.rs         // LspSession, CapabilityMatrix, JSON-RPC
      rust.rs        // rust-analyzer 連携（DocumentSymbol/CallHierarchy/References）
      ruby.rs        // ruby-lsp/solargraph 等（機能差は動的判定）
```

### Engine トレイト（案）
```rust
pub enum EngineKind { Auto, Ts, Lsp }

pub trait AnalysisEngine {
    fn changed_symbols(&self, diffs: &[FileChanges], lang: LanguageMode) -> anyhow::Result<ChangedOutput>;
    fn impact(&self, changed: &[Symbol], opts: &ImpactOptions) -> anyhow::Result<ImpactOutput>;
}
```
- TS 実装は既存関数：`compute_changed_symbols` と `build_project_graph`+`compute_impact`。
- LSP 実装はオンデマンド問い合わせで解決（全量グラフを常に構築しない）。

## アルゴリズム（動的チェーン）
### 変更シンボル抽出
1) LSP が有効かつ `documentSymbol` 成功 → 階層ノードから `SymbolKind`/`TextRange` を生成し、変更行と交差する宣言のみ採用。
2) 失敗時、LSP `workspace/symbol` を補助的に利用（ノイズ可）。
3) それでも不足なら TS の `symbols_in_file` にフォールバック。

### 影響伝播（callers/callees/both）
1) 起点ごとに `prepareCallHierarchy` 成功なら、方向に応じて `incomingCalls`/`outgoingCalls` を BFS。`max_depth` を反復回数に適用。
2) 階層未対応でも `references`/`definition` が使える場合：
   - callers: 参照点を `documentSymbol` で「包含シンボル」にマップして呼び元ノード集合を作る。
   - callees: 定義/実装点から呼び先候補を集約。
3) それでも不可なら TS の `build_project_graph`+`compute_impact` にフォールバック。
4) `with_edges` は得られた関係から `Reference` を構築し直す（LSP 階層→エッジ化、参照→逆引きエッジ化）。

### 多言語の同時処理
- 拡張子単位で LSP セッションをマルチプレックス（Rust→rust‑analyzer、Ruby→ruby‑lsp 等）。
- 各ファイル/機能ごとに能力判定→プローブ→最適経路を選択。言語固定の分岐は行わない。

## セッション管理
- `LspSession` を言語ごとに起動（stdio JSON‑RPC、`Content-Length`）。
- `initialize`/`initialized` の後、必要に応じ `didOpen`（変更ファイルや近傍）を送ると精度が向上するサーバもある。
- タイムアウト/エラー時は `--engine auto` では次の段へフォールバック、`--engine lsp` ではエラー終了（`--engine lsp-strict` 案）。
- 将来は `dimpactd` デーモンで常駐化し、インデックスを再利用。

### 依存と実装方針
- 依存候補: `lsp-types`, `serde_json`（既存）, `crossbeam-channel` もしくはシンプルな同期スレッド実装。
- 非同期ランタイムは初期段階では導入せず、blocking IO + スレッドで十分。

## TS 実装の位置づけ
- 既存の `languages/*` と `impact.rs` をそのまま活用。LSP 未対応の言語/機能のフォールバックとして常に利用可能に保つ。
- CLI `--engine ts` で強制選択できるようにし、再現性を担保。

## CLI/公開API への影響
- 追加フラグ: `--engine auto|ts|lsp`、`--engine-lsp-strict`、`--engine-dump-capabilities`（診断用）。
- 出力スキーマは維持。LSP でも `ImpactOutput` を構築し、`with_edges` は得られた関係で埋める。

## 設定（YAML）
- 目的: バックエンド選択（LSP/TS）、LSP 起動・プローブ・タイムアウト、解析パラメータ、出力/診断を外部設定可能に。
- 配置優先度（高→低）:
  1) CLI フラグ（最優先）
  2) ワークスペース設定 `dimpact.yml` または `dimpact.yaml`（リポジトリルート）
  3) ユーザー設定 `~/.config/dimpact/config.yml`（存在すれば）
  - マージ規則: マップは深いマージ、配列は上位が下位を置換（重複の曖昧性を避けるため）。

### スキーマ案
```yaml
version: 1

engine:
  default: auto           # auto|ts|lsp
  lsp_strict: false       # true ならフォールバック禁止
  prefer:                 # 言語別の初期選好（実行時は能力で上書き）
    rust: lsp
    ruby: auto
  probe:
    timeout_ms: 1500
    retry: 0

lsp:
  rust:
    cmd: rust-analyzer
    args: []
    env: {}
    initializationOptions:
      cargo:
        features: []
        allTargets: false
        runBuildScripts: false
      procMacro:
        enable: false
    openStrategy:
      openChangedFiles: true
      openContextLines: 20
    requestTimeoutMs: 5000
    concurrency: 8        # BFS 並列上限
    capabilitiesOverride: # サーバの申告を上書き可
      callHierarchy: true

  ruby:
    cmd: ruby-lsp         # 例: ruby-lsp / solargraph
    args: []
    env: {}
    requestTimeoutMs: 5000
    capabilitiesOverride:
      callHierarchy: false

ts:
  enabled: true
  # 将来: クエリ上書きや言語別チューニング用のフック

analysis:
  language: auto          # auto|rust|ruby
  direction: callers      # callers|callees|both（デフォルト値）
  max_depth: 100
  with_edges: false
  include: ["**/*.rs", "**/*.rb"]
  exclude: ["target/**", ".git/**", "**/.*/*"]

output:
  format: json            # json|yaml
  pretty: true
  dumpCapabilities: false # 能力診断の出力
  trace: warn             # off|error|warn|info|debug|trace
```

### 意味と動作
- engine:
  - `default` は基本動作。`prefer` は言語別の初手を示すだけで、実際は能力検出＋プローブで経路を選ぶ。
  - `lsp_strict` 有効時は LSP 失敗で即エラー（自動フォールバックなし）。
  - `probe` は機能プローブのタイムアウト/リトライを制御。
- lsp:
  - サーバ起動コマンド/引数/環境変数、`initializationOptions` を明示可能。
  - `openStrategy` は `didOpen` ポリシー。開くファイルと行数ヒントを制御。
  - `capabilitiesOverride` でサーバ申告の機能を強制的に on/off。
  - `requestTimeoutMs`/`concurrency` は BFS 時の並列・タイムアウト制御に反映。
- analysis/output:
  - CLI 未指定時の既定値として使用。`include/exclude` は WalkDir および LSP 対象フィルタに反映。

### セキュリティと信頼境界
- LSP サーバの実行コマンドや環境変数を設定できるため、未信頼リポではユーザー設定のみ有効・リポ設定は無視する「セーフモード」を検討。
- 将来フラグ: `--safe-config`（ユーザー設定のみ）/ `--no-config`（設定無効）。

### CLI との優先順位
- CLI > リポ `dimpact.yml` > ユーザー設定の順で上書き。
- 例: `--engine lsp --direction callees` は YAML の `engine.default`/`analysis.direction` を無視して実行。

## エラー処理とフォールバック
- サーバ未検出/初期化失敗/タイムアウト：`auto` は次の段へフォールバック、`lsp` 指定時はエラー終了。
- 未対応機能は必ずプローブして判定。`references` や `definition` で代替し、足りない場合のみ TS に切替。

## パフォーマンスと安全性
- 初回は LSP ウォームアップで遅くなり得るが、常駐化で逆転。BFS はオンデマンド問い合わせで全量スキャンを避ける。
- LSP はローカル実行前提。`cargo`/`bundle` 等の読み取りがあるため未信頼リポはサンドボックス推奨。

## テスト計画
- 単体: JSON‑RPC フレーミング、URI/パス/行番号変換、能力行列とプローブの判定、タイムアウト。
- 統合（feature `lsp`）: サーバがある環境でのみ実行。`--engine auto` が能力に応じて正しくフォールバックすることを検証。
- 互換性: 既存 `tests/*.rs` と同等の出力を `--engine lsp` でも満たす（`--direction`/`--max-depth`/`with_edges`）。

## 段階的ロールアウト
- Phase 1: Rust/LSP を実装（動的フォールバック含む）。Ruby はサーバ能力が足りない箇所のみ TS に自動移行。
- Phase 2: セッション再利用（簡易デーモン）、メトリクス収集、`auto` を既定に昇格。
- Phase 3: Ruby/その他言語の機能差吸収と精度改善。

## オープンクエスチョン
- サーバ差異（特に Ruby 系）をどこまで抽象化するか。
- マクロ展開/生成コードの扱い（Rust）。
- workspaceFolders（マルチルート）対応の優先度。
- 将来 DB 方式との併用（事前計算キャッシュ＋オンデマンド問い合わせ）。

---
固定ロジックではなく「能力駆動の動的フォールバック」を中核に据えることで、精度と速度を両立しつつ多言語・多サーバ環境に柔軟に対応します。
