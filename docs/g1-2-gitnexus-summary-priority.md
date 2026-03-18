# GitNexus summary 流用候補の優先順位づけと最小実装順 (G1-2)

対象: `dimpact impact`

このメモは、GitNexus 由来で流用価値がありそうな summary 候補を、
**dimpact の現状実装にどれだけ自然に乗るか** で優先順位づけし、
G1 系タスクでの **最小実装順** を決めるための設計メモ。

前提として現在の `impact` 出力は `ImpactOutput` を核にしており、
Tree-Sitter 経路の `compute_impact()` (`src/impact.rs:542`) と、
CLI wrapper (`src/bin/dimpact.rs:112`, `src/bin/dimpact.rs:152`) と、
LSP 側の複数の `ImpactOutput` 組み立て経路 (`src/engine/lsp.rs:1256`, `src/engine/lsp.rs:2263`) がある。

この構造を前提にすると、候補ごとの優先度は「有用さ」だけでなく、
**今のデータで無理なく作れるか / traversal の内部状態追加が必要か / 既存出力を壊しにくいか** で決まる。

## 1. 候補一覧

今回の候補は tasklist に沿って以下の 4 本とする。

1. `by_depth`
2. `risk`
3. `affected_modules`
4. `affected_processes`

GitNexus 側で価値があるのはどれも同じだが、dimpact では向き不向きがかなり違う。

## 2. 評価軸

優先順位づけには次の 5 軸を使う。

- **利用価値**: `impact` を見た瞬間の判断材料になるか
- **dimpact 適合度**: 既存の call graph / symbol / path 情報で自然に作れるか
- **前提依存**: 先に別の summary を入れないと成立しないか
- **実装コスト**: traversal / schema / tests / docs の増分がどれくらいか
- **ヒューリスティック耐性**: repo 依存・命名依存で壊れやすくないか

## 3. 優先順位の結論

結論だけ先に書くと、dimpact 向けの優先順位はこうするのが自然。

1. **`by_depth`**
2. **`risk`**
3. **`affected_modules`**
4. **`affected_processes`**

理由は単純で、
`by_depth` は traversal の結果をそのまま整理した summary であり、
`risk` はその上に乗る二次集計、
`affected_modules` は path / namespace ベースの軽量 post-process で比較的後付けしやすく、
`affected_processes` は entrypoint 概念や repo 依存 heuristic が強く、今の dimpact には一番重いから。

## 4. 候補ごとの評価

### 4.1 `by_depth` — 最優先

**位置づけ**
- もっとも dimpact らしい summary
- traversal 結果を「何件あったか」から「どの距離に何がいるか」へ変換できる
- 後続の `risk` 計算の土台になる

**価値**
- 直接影響 (`depth=1`) と波及影響 (`depth>=2`) を分けて読める
- 単なる `impacted_symbols` 件数より判断しやすい
- `--direction callers|callees|both` の違いも見えやすい

**dimpact との相性**
- 現在の `compute_impact()` は BFS を使っており (`src/impact.rs:565` 以降)、概念的には最短 depth と相性が良い
- ただし現実には `seen` 集合しか残しておらず (`src/impact.rs:563`)、最終出力に depth map を持っていない
- つまり **価値は高いが、探索中の内部状態を少し拡張する必要がある**

**リスク**
- 既存出力にはない新しい集計なので schema 設計が必要
- `--per-seed` と confidence filter 適用後の shape を合わせる必要がある
- LSP 側の手組み `ImpactOutput` でも同じ summary を載せる必要がある

**結論**
- 最優先で着手する
- ただし最初の版は欲張らず、**depth ごとの件数 + file 数** から始めるのがよい

### 4.2 `risk` — 第2優先

**位置づけ**
- 利用者が最終的に見たい「読む順」を短縮する summary
- ただし raw データではなく、`by_depth` 等の集計を前提にした二次評価

**価値**
- direct / transitive hit 数を 1 つの目安へ圧縮できる
- CI や review triage と相性が良い
- 既存の `confidence_filter` と並べたときに、「どれくらい確からしい・どれくらい広い」を両輪で示せる

**dimpact との相性**
- `ImpactOutput` の既存 field だけでも大まかな件数は出せるが、
  本当に欲しいのは `depth=1` と `depth>=2` の分離
- そのため、**`risk` 単独先行より `by_depth` 後追いの方が自然**

**リスク**
- スコア式を先に作り込みすぎると調整コストが膨らむ
- レベル名 (`low|medium|high` など) を早く固定しすぎると、将来見直しづらい

**結論**
- 第2優先
- 初期版は **単純なルールベース** にとどめる
  - `direct_hits`
  - `transitive_hits`
  - `impacted_files`
  - `impacted_symbols`
- まず説明可能性を優先し、重み学習や repo 別補正は入れない

### 4.3 `affected_modules` — 第3優先

**位置づけ**
- 「どこが揺れているか」を symbol 群ではなくモジュール単位で見せる summary
- GitNexus 由来の grouping の中では、dimpact に一番移植しやすい

**価値**
- 大きい diff で `impacted_symbols` が長くなるときに読みやすい
- `src/foo/...` のような path まとまりで会話できる
- 実際の修正導線（関連ファイルの追加確認）に繋がりやすい

**dimpact との相性**
- `impacted_symbols` / `impacted_files` は既にある (`src/impact.rs:622`, `src/impact.rs:633`)
- つまり最小版は traversal を触らずに、**後段集計だけで導入できる**
- path prefix、module path、import namespace のどれか 1 つから始めればよい

**リスク**
- どの単位を module と呼ぶかは言語差がある
- Rust/TS/Java/Python/Ruby/Go で厳密に揃えようとすると重い

**結論**
- 第3優先
- 初期版は **path prefix ベース** に限定する
- たとえば `src/foo/bar.rs` → `src/foo` のような軽量ルールから始めるのがよい
- 「module/community grouping の最小版」としては十分価値がある

### 4.4 `affected_processes` — 第4優先 / 条件付き defer 候補

**位置づけ**
- 利用価値は高いが、最も repo 依存で、dimpact の現在地には重い
- 「何の実行経路が影響を受けるか」を言いたい summary

**価値**
- うまく当たれば人間には非常に分かりやすい
- ただし外したときの誤誘導コストも高い

**dimpact との相性**
- 現在の出力は symbol / file / edge が中心で、process / entrypoint の明示モデルはない
- そのため、最小版でも下記のどれかが必要になる
  - entrypoint ファイルの規約定義
  - main / bin / cli / server などへの heuristic
  - call graph からの逆引きルール

**リスク**
- repo ごとに正解が違う
- 誤った group 名が付くと、`risk` よりも強く人を誤誘導する
- tests も fixture 依存が強くなりやすい

**結論**
- 第4優先
- G1-7 時点で軽量導入案が見えなければ、**実装より design note + TODO を優先** する
- 無理に G1 前半へ押し込む候補ではない

## 5. 優先順位表

| 候補 | 優先度 | 利用価値 | 実装コスト | 前提依存 | ヒューリスティック依存 | 判断 |
| --- | --- | --- | --- | --- | --- | --- |
| `by_depth` | 1 | 高い | 中 | なし | 低い | 先に入れる |
| `risk` | 2 | 高い | 低〜中 | `by_depth` ほぼ必須 | 低い | `by_depth` の次 |
| `affected_modules` | 3 | 中〜高 | 低〜中 | なし | 中 | 軽量版を後段で入れる |
| `affected_processes` | 4 | 中〜高 | 中〜高 | なし | 高い | 後回し / 条件付き defer |

## 6. 最小実装順

実装順は candidate 順と完全には同じではなく、**plumbing を先に入れる** のが安全。

### Phase 0: summary 受け皿の整備

最初にやるべき最小作業:

1. `ImpactOutput` に optional `summary` を追加できる形を決める
2. `ImpactOutput` の sort / dedup / file 集計を共通 helper に寄せる
3. confidence filter 適用後の出力にも同じ helper を通す
4. `--per-seed` でも `impacts[].output.summary` に揃えて載るようにする

これは feature というより **今後の実装事故を減らすための plumbing**。
`src/impact.rs:677` の直接構築と、`src/engine/lsp.rs:1256` / `src/engine/lsp.rs:2302` の直接構築を放置したままだと、summary の付け忘れが起きやすい。

### Phase 1: `by_depth` の最小版

最小版の output はこれで十分。

```json
{
  "summary": {
    "by_depth": [
      { "depth": 1, "symbol_count": 3, "file_count": 2 },
      { "depth": 2, "symbol_count": 7, "file_count": 4 }
    ]
  }
}
```

ここではまだ以下は入れない。

- depth ごとの symbol 一覧
- depth ごとの edge 一覧
- 最短経路の具体列

まずは **件数集計だけ** で良い。これでも十分に判断しやすくなる。

### Phase 2: `risk` の最小版

`by_depth` が入ったら、`risk` は比較的薄く足せる。

```json
{
  "summary": {
    "risk": {
      "level": "medium",
      "direct_hits": 3,
      "transitive_hits": 7,
      "impacted_files": 4,
      "impacted_symbols": 10
    }
  }
}
```

初期版はルールベースでよい。
たとえば:

- direct 0 / transitive 小 → `low`
- direct 1 以上 or file 数が閾値超え → `medium`
- direct 多数 + transitive 多数 → `high`

というように、**人間が説明できる閾値** にとどめる。

### Phase 3: `affected_modules` の最小版

最小版は path grouping だけで始める。

```json
{
  "summary": {
    "affected_modules": [
      { "module": "src/engine", "symbol_count": 4, "file_count": 2 },
      { "module": "src/bin", "symbol_count": 2, "file_count": 1 }
    ]
  }
}
```

この段階では以下を見送る。

- 言語ごとの厳密 module 解釈
- import graph による community 検出
- repository 固有 namespace の手当て

### Phase 4: `affected_processes` の可否判断

ここは「実装する」より先に、「軽量版が本当に成立するか」を見るフェーズにする。

成立条件の目安:
- entrypoint 候補を path 規約からそこそこ安定抽出できる
- false positive をテスト fixture で抑えられる
- module grouping より説明力が上がる

これが満たせないなら、G1 では **設計メモと TODO で止める** 方がよい。

## 7. G1 系タスクへの割り当て

この優先順位を tasklist に落とすと、自然な流れはこうなる。

- **G1-3 / G1-4**
  - `by_depth` schema 決定
  - `by_depth` 実装
  - CLI fixture / golden / README 最低限更新
- **G1-5 / G1-6**
  - `risk` 実装
  - fixture で direct/transitive hit を検証
- **G1-9 / G1-10**
  - `affected_modules` 軽量版を path grouping で導入
- **G1-7 / G1-8**
  - `affected_processes` は軽量導入案の feasibility 判定を先にやる
  - 条件を満たす場合のみ最小版を入れる

tasklist の番号順では process の検討が先に来ているが、
**実装優先度としては `affected_modules` の方が前** と考えるのが妥当。

## 8. 推奨する schema の育て方

summary 全体は最初から feature ごとに拡張できる箱にしておくのがよい。

```json
{
  "summary": {
    "by_depth": [],
    "risk": {},
    "affected_modules": [],
    "affected_processes": []
  }
}
```

ただし実際の初期出力では、未実装 field を無理に空配列で出す必要はない。
Rust 側では `Option` ベースにして、**実装済みのものだけ serialize** する方が安全。

理由:
- 既存 consumer との互換性を保ちやすい
- feature を段階追加しやすい
- `--per-seed` でもネストが肥大化しすぎない

## 9. 最終判断

GitNexus 流用候補のうち、dimpact 向けに本当に先にやるべき順は:

1. **`by_depth`**: traversal に最も近く、利用価値も高い
2. **`risk`**: `by_depth` の上に素直に乗る
3. **`affected_modules`**: path ベースの軽量 grouping が現実的
4. **`affected_processes`**: 有用だが repo 依存が強く、G1 前半では重い

実装順としては、さらにその前に **summary 共通 plumbing** を挟むべき。

つまり G1 系の最小実装順は、実質的には次の 5 段になる。

1. summary 受け皿 / 共通 finalizer
2. `by_depth`
3. `risk`
4. `affected_modules`
5. `affected_processes`（成立時のみ）

この順なら、dimpact の既存出力互換を保ちつつ、
「まず役立つもの」から順に導入できる。