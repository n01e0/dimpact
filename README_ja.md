# dimpact

現在のバージョン: `0.5.3`

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

### 新しい `summary` 出力の見方（JSON/YAML）
- `impact` の JSON/YAML には、詳細な symbol/edge 一覧を読む前の一次判断用として `summary` ブロックが含まれます。
- 現在の summary 項目:
  - `summary.by_depth`
    - 変更/シードシンボルから impacted symbol までの最短 hop 数ごとの集計
    - `depth=1` は direct hit
    - `depth>=2` は transitive な波及
  - `summary.risk`
    - direct hit 数、transitive hit 数、影響 file 数、影響 symbol 数から作る軽量な一次トリアージ優先度
    - 本番障害の severity 予測ではなく、レビュー/CI でどれくらい慎重に見るべきかの目安です
    - ざっくりした読み方:
      - `low`: まず局所 diff とみなして変更近傍から見る
      - `medium`: 近い caller や周辺 file まで確認候補に入れる
      - `high`: caller 側へ広く波及している前提で早めにグラフを確認する
  - `summary.affected_modules`
    - impacted symbol を path ベースで軽量にグルーピングしたもの
    - 「次にどのディレクトリ / モジュールを開くべきか」を決めるための補助です
    - 読みやすさのため、`src/main.rs` / `src/lib.rs` / `src/engine/mod.rs` のような entry-like file は親ディレクトリへ畳み、repo root の entry file は `(root)` と表示されます
- 現時点では `summary.affected_processes` はありません。entrypoint 判定の heuristic と fixture を固めるまで意図的に見送っています。
- JSON のイメージ:
  ```json
  {
    "changed_symbols": [...],
    "impacted_symbols": [...],
    "impacted_files": [...],
    "edges": [...],
    "impacted_by_file": {...},
    "impacted_witnesses": {...},
    "summary": {
      "by_depth": [
        { "depth": 1, "symbol_count": 3, "file_count": 2 },
        { "depth": 2, "symbol_count": 7, "file_count": 4 }
      ],
      "risk": {
        "level": "medium",
        "direct_hits": 3,
        "transitive_hits": 7,
        "impacted_files": 4,
        "impacted_symbols": 10
      },
      "affected_modules": [
        { "module": "src/engine", "symbol_count": 4, "file_count": 2 },
        { "module": "(root)", "symbol_count": 2, "file_count": 1 }
      ]
    },
    "confidence_filter": {
      "min_confidence": "inferred",
      "exclude_dynamic_fallback": false,
      "input_edge_count": 20,
      "kept_edge_count": 12
    }
  }
  ```
- 運用上の読み順:
  - まず `by_depth` で direct / transitive を分けて見る
  - 次に `risk` でどれくらい強くトリアージすべきかを判断する（`low` = 局所優先、`medium` = 近傍 caller まで確認、`high` = 広い波及を疑う）
  - `affected_modules` で、次に見るべきディレクトリ / モジュールを絞る。`(root)` は `main.rs` という file 名そのものではなく、repo root の entry 周辺を指します
  - 最後に `impacted_symbols` / `edges` で具体的な伝播経路を確認する
- `confidence_filter` は `summary` の中ではなく、引き続きトップレベル sibling として出ます。
- `impacted_witnesses` には、各 impacted symbol ごとの最小パス要約が入ります:
  - `edge` / `via_symbol_id` は引き続き選ばれた last hop を指します
  - `path` は root changed/seed symbol からそこに至る 1 本の hop-by-hop 経路を出します
  - `provenance_chain` / `kind_chain` で、その経路のどこに call / data / control / symbolic_propagation が入ったかを追いやすくなります
  - `path_compact` / `provenance_chain_compact` / `kind_chain_compact` は、その同じ経路をより説明しやすい圧縮形で返します
  - `slice_context.selected_files_on_path` を見ると、その witness 経路上の file が bounded-slice planner でどう選ばれたか、どの hop index を担当したか、どの seed-specific reason で残ったかを軽く追えます
  - `slice_context.selected_vs_pruned_reasons` には、selected された bridge candidate が ranked-out 候補に勝った最小理由が入ります
  - ただし、これは依然として 1 本の最短経路ベースの説明であり、すべての候補経路を網羅するものではありません
- `summary.slice_selection` は PDG / propagation path で出力され、bounded-slice planner 自体の判断を見せます:
  - `files[*]` で選ばれた file-level scope と `cache_update` / `local_dfg` / `explanation` の分離を確認できます
  - `files[*].reasons[*]` で direct boundary と bridge completion を含む seed ごとの選定理由を確認できます
  - `files[*].reasons[*].scoring` と `pruned_candidates[*].scoring` を見ると、`source_kind` / `lane` / evidence kind / score tuple まで含めた bridge candidate の比較根拠を JSON/YAML 上で追えます
  - `pruned_candidates[*]` で ranked-out / budget prune された候補の最小診断を確認できます
  - scope split は `cache_update` = 実行準備、`local_dfg` = ローカル flow の materialization、`explanation` = user-facing に残す file、という意図です。`local_dfg` と `explanation` は分かれうる一方、pruned candidate は explanation file には昇格しません
- `--per-seed` 指定時は、各変更/シードシンボルごとの `impacts[].output.summary` 配下に同じ summary が入り、witness も各 grouped output の中にネストされます。
- DOT/HTML 出力は互換維持で、今回の summary は JSON/YAML 利用を主対象としています。

### evidence-driven selection / evidence-budgeted admission の見方
- PDG / propagation planner は、到達可能な helper file を全部 scope に入れることを目指していません。
  - 目的は bounded な explanation slice を保ったまま、boundary side ごとに最も筋の良い continuation を選ぶことです
- G9/G10 では mental model をもう 1 段はっきりさせています:
  - `source_kind` と `lane` は重要な ranking dimension ですが、それ自体は evidence ではありません
  - evidence は、次の 4 category に分けて読むと追いやすいです:
    1. `primary`: `param_to_return_flow` / `return_flow` / `assigned_result` / `alias_chain` のような、選ばれた continuation を直接支える continuity fact
    2. `support`: `local_dfg_support` / `symbolic_propagation_support` / `edge_certainty` / position hint のような、単独では勝ち筋を作らない strength / provenance signal
    3. `fallback`: `explicit_require_relative_load` / `companion_file_match` / `dynamic_dispatch_literal_target` / 弱い `require_relative` continuation のような、narrow runtime candidate を bounded に出してよい理由
    4. `negative`: helper-style な return noise や、弱い certainty の fallback loser のような、noisy candidate を suppress する signal
- 実際の planner 比較は引き続き `summary.slice_selection.files[*].reasons[*].scoring` と `summary.slice_selection.pruned_candidates[*].scoring` に出ますが、読み順としては:
  - まず `primary` で continuity の筋が強いかを見る
  - 次に `support` でその continuity がどれくらい信頼できるかを見る
  - 次に `fallback` で narrow runtime candidate がそもそも出現してよい理由を見る
  - 最後に `negative` で、なぜ loser が ranked-out のまま slice を広げなかったかを見る
- 運用上の大事な考え方は、**良い evidence は scope を広げずに precision を上げるべき**、という点です。
  - 強い候補だけが selected explanation file になる
  - 弱い候補は `pruned_candidates[*]` や `slice_context.selected_vs_pruned_reasons` に残して、黙って slice を広げない
- G10 では、これをさらに 1 段前へ寄せて **evidence-budgeted admission** として扱います:
  - planner は「どの候補が勝つか」だけでなく、「どの弱い候補は ranking pool に入る前に落とすべきか」も判断します
  - helper noise、fallback-only loser、弱い same-family sibling、弱い same-path duplicate は、explanation file に昇格するのではなく `pruned_candidates[*]` に留まるのが期待挙動です
  - 読み分けると便利な prune label は次です:
    - `suppressed_before_admit`: 通常の side-local ranking に入る前に落とした弱い候補
    - `weaker_same_family_sibling`: 同じ continuation family にすでにより強い代表候補がいた
    - `weaker_same_path_duplicate`: 複数の説明が同じ file へ収束したが、より強い代表だけを selected に残した
    - `bridge_budget_exhausted`: ローカル比較は通ったが、最後の per-seed bounded budget で落ちた
  - これは「探索を広げた planner」ではなく「family-aware に bounded admission を厳しくした planner」だと読むのが大事です:
    - admission が厳しくなった
    - loser bookkeeping が明示的になった
    - `selected_files_on_path` を含む witness scope は、loser を記録しても意図的に小さく保たれる
- witness 出力も同じ考え方で圧縮されています:
  - `winning_primary_evidence_kinds` と `winning_support` が selected side を説明する
  - `losing_side_reason` は、helper noise / fallback-only / 弱い `dynamic_fallback` certainty など、loser 側に明確な suppressing reason があるときに短く説明する
- Rust/Ruby の PDG 出力が意外だったときは、次の順で読むと追いやすいです:
  1. `summary.slice_selection.files[*].reasons[*].scoring` を見る
  2. `summary.slice_selection.pruned_candidates[*].scoring` と見比べる
  3. admit 前 drop や duplicate/sibling merge だった場合は `pruned_candidates[*].compact_explanation` を見る
  4. 最後に `impacted_witnesses[*].slice_context.selected_vs_pruned_reasons` で人間向けの最短説明を確認する

### Impact オプション（`impact` サブコマンド）
- `--direction callers|callees|both` : 方向 (既定: callers)
- `--max-depth N`               : 最大探索深度 (既定: 100)
- `--with-edges`                : 参照エッジを出力に含める
- `--min-confidence LEVEL`      : confidence 閾値（`confirmed|inferred|dynamic-fallback`）
- `--exclude-dynamic-fallback`  : `dynamic_fallback` エッジを探索/出力から除外
- `--op-profile PROFILE`        : 運用プリセット（`balanced|precision-first`）
- `--ignore-dir DIR`            : 相対パスプレフィックスでディレクトリを無視（繰り返し可）
- `--with-pdg`                  : 通常 impact にローカルな PDG/DFG エッジを追加 (Rust/Ruby のローカル DFG)
- `--with-propagation`          : PDG の上にシンボリック伝播 bridge を追加（call-site / function summary ヒューリスティック）
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

## 言語別 推奨 `--min-confidence`（Q54-10）

Q54-10 の再計測結果（`release-notes/0.5.4-confidence-distribution-q54-10.md`）に基づく、現時点の運用推奨値です。

| 言語 | 推奨 `--min-confidence` | 観測 inferred edge 数 | 理由 |
| --- | --- | ---: | --- |
| typescript | `inferred` | 3 | サンプルの impacted edge が inferred。`confirmed` だと観測シグナルを落とす |
| tsx | `inferred` | 0 | サンプルでは impacted edge なし。運用一貫性のため global default に合わせる |
| rust | `inferred` | 0 | サンプルでは impacted edge なし。運用一貫性のため global default に合わせる |
| java | `inferred` | 19 | サンプルの impacted edge が inferred。`confirmed` だと観測シグナルを落とす |
| go | `inferred` | 0 | サンプルでは impacted edge なし。運用一貫性のため global default に合わせる |
| ruby | `inferred` | 6 | サンプルの impacted edge が inferred。`confirmed` だと観測シグナルを落とす |
| python | `inferred` | 12 | サンプルの impacted edge が inferred。`confirmed` だと観測シグナルを落とす |

- 推奨 global default は `inferred`。
- 誤判定コストが高い厳格レビュー/CI トリアージでは `--op-profile precision-first`（または `--min-confidence confirmed --exclude-dynamic-fallback`）を使用してください。

## PDG / propagation の使い分け
- bounded slice は、いまの PDG path の scope モデルです
  - 目的は repo 全体を開くことではなく、短い multi-file bridge を説明するのに必要な近傍 file だけを拾うことです
  - 現在の planner は **controlled 2-hop** で、seed/changed file → direct boundary file → boundary side ごとに必要な bridge completion file を少数だけ足す、という bounded な project slice を狙っています
  - `--with-pdg` と `--with-propagation` は、diff モードでも seed モードでも、この同じ bounded slice planner を共有します
- `--with-pdg`
  - 選ばれた bounded slice 上で、Rust/Ruby の local data/control dependence を少し厚く見たいとき向けです
  - plain な call graph だけでは粗すぎる Rust/Ruby の alias / control-flow 周辺で特に有効です
  - 「まずどの近傍 file まで scope に入るべきか」を確認したいときの軽めの説明強化パスだと考えてください
- `--with-propagation`
  - 同じ bounded slice の上に call-site / summary bridge を足す拡張です
  - 引数 → callee → 代入先 のような値伝播や、wrapper return-flow / imported result continuation を見たいときに使います
  - local variable flow、alias chain、wrapper return-flow、短い inter-procedural propagation の false negative を疑うときに向いています
- `--per-seed`
  - 通常 impact だけでなく PDG / propagation path でも使えます
  - changed/seed symbol ごとの波及を個別に見たいときに便利で、seed ごとの witness や compact path も追いやすくなります

## 現在の PDG / propagation の限界
- scope はまだ意図的に絞っています
  - 現状は **bounded project slice** であって project-wide closure ではありません
  - Rust/Ruby ではおおむね「root file + direct boundary + いくつかの boundary side ごとの controlled second-hop bridge file」くらいのスコープを狙っています
  - そのため、短い 2-hop bridge には効きますが、再帰的な whole-project expansion はしません
  - それ以外の言語では `--with-pdg` を付けても、実質的には通常の call graph シグナルに寄る場面が多いです
- **project-wide PDG** ではありません
  - 現状は依然として `global call graph + bounded local DFG augmentation` に近い挙動です
  - propagation も whole-program symbolic execution ではなく、call-site / summary 中心の heuristic です
- witness は改善されたが、まだ最小表現です
  - `impacted_witnesses.path` / `provenance_chain` / `kind_chain` で 1 本の multi-hop 説明を見られるようになりました
  - `impacted_witnesses.slice_context` を見ると、その経路上に出てくる selected file が bounded-slice planner でどう選ばれたかを軽く辿れます
  - `*_compact` 系フィールドでその説明を読みやすく圧縮していますが、依然として 1 本の選択経路の要約です
  - 競合する複数経路の列挙や、代替経路が存在しないことの証明まではしません
- `-f dot` の意味が切り替わります
  - 通常の `impact -f dot` は impact graph
  - `impact --with-pdg -f dot` は raw な PDG/DFG 風グラフです
- Ruby は short multi-file case が前進した一方で、限界もまだあります
  - `require_relative` / alias / wrapper-return の短い chain や no-paren wrapper parameter flow は以前より拾いやすくなりました
  - bridge scoring は、semantic に強い alias / return-flow completion を単純な `require_relative` helper noise より優先しつつ、弱い fallback は scope を広げず ranked-out candidate として残す方針です
  - true narrow fallback の考え方は、**広く拾うことではなく bounded に admission すること** です:
    - runtime companion は concrete な target family や bounded runtime fact に結びつくときだけ残る想定です
    - generic な dynamic runtime file は「一応関係ありそう」では残さず、filter される方が正しいです
  - fallback discovery 自体はまだ意図的に narrow で、広い companion expansion を目指してはいません
  - ただし、長い `require_relative` ladder、dynamic-send が強い flow、広い companion discovery はまだ意図的に弱く抑えています
  - Ruby の PDG / propagation は、完全な inter-procedural proof system ではなく、bounded な explainability aid として見るのが安全です
- エンジン統合は改善したが、まだ完全ではありません
  - diff モードの PDG / propagation でも changed-symbol discovery だけでなく strict な impact capability check も選択した engine に揃うようになりました
  - ただし PDG / propagation 自体は、いまも cache 済み graph の上にローカル graph 構築を重ねる経路なので、engine ネイティブな edge richness がそのまま保存されるわけではありません

## 実運用の目安
- repo 全体で安定した caller/callee の答えが欲しいなら、まず通常の `impact` を使う
- 「どの近傍 file まで bounded slice に入るのか」と、その中の Rust/Ruby data/control dependency を見たいなら `--with-pdg` を足す
- 「この値・引数・結果は call 境界を越えて伝播するか？」が本題なら `--with-propagation` まで上げる。特に短い Rust/Ruby の multi-file bridge を見たいときに有効です
- PDG / propagation の結果が意外だったら、まず `summary.slice_selection` で selected / pruned file を見て、その後に `impacted_witnesses[*].slice_context` で why-this-file と why-this-path の対応を追うのが分かりやすいです
- bridge choice がまだ不自然に見える場合は、`files[*].reasons[*].scoring` と `pruned_candidates[*].scoring` を見比べ、最後に `slice_context.selected_vs_pruned_reasons` で人間向けの最小説明を確認してください
- seed ごとに分けて見たいなら、上のどれに対しても `--per-seed` を足し、`impacted_witnesses` と compact witness fields を見る
- Go/Java/Python/JS/TS/TSX では、現時点の `--with-pdg` に大きな上積みを期待しすぎない方が安全です。fixture / regression で確認できる範囲の experimental 機能として扱ってください

## PDG 可視化
`--with-pdg` と `-f dot` を組み合わせて PDG を dot 形式で出力できます。
```bash
git diff --no-ext-diff | dimpact impact --with-pdg -f dot
```
`--with-propagation` と `-f dot` を組み合わせると、伝播 bridge を含んだ graph を出力できます。
```bash
git diff --no-ext-diff | dimpact impact --with-propagation -f dot
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
