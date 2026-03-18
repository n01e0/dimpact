# `by_depth` summary の設計と schema (G1-3)

対象: `dimpact impact`

このメモは、`impact` 出力へ追加する `by_depth` summary について、
**出力 schema / 深さの意味 / 既存出力との整合 / 実装時の内部データ要件** を固めるための設計メモ。

G1-3 では実装はまだ入れず、G1-4 でそのまま実装に落とせる粒度まで決める。

## 1. 目的

`impacted_symbols` は一覧としては便利だが、件数が増えると「どれが近い影響で、どれが波及影響か」が読みにくい。

`by_depth` はそれを補う summary で、
**seed changed symbol から impacted symbol までの最短 hop 数** を使って、
impact を深さごとに集計する。

狙いは次の 3 つ。

1. `depth=1` の direct hit をすぐ読めるようにする
2. `depth>=2` の transitive hit をまとめて把握できるようにする
3. 後続の `risk` 算出で再利用できる土台を作る

## 2. 置き場所

`by_depth` は CLI 専用 wrapper ではなく、`ImpactOutput` 本体の `summary` に載せる。

理由:
- 通常 JSON/YAML と `--per-seed` で同じ位置に置ける
- 将来 `risk` / `affected_modules` / `affected_processes` を同じ箱に足せる
- HTML/DOT は最初は無視しても、後で取り込める

### 2.1 追加後の概念 shape

```rust
pub struct ImpactOutput {
    pub changed_symbols: Vec<Symbol>,
    pub impacted_symbols: Vec<Symbol>,
    pub impacted_files: Vec<String>,
    pub edges: Vec<Reference>,
    pub impacted_by_file: HashMap<String, Vec<Symbol>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<ImpactSummary>,
}
```

```rust
pub struct ImpactSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub by_depth: Option<Vec<ImpactDepthBucket>>,
}
```

```rust
pub struct ImpactDepthBucket {
    pub depth: usize,
    pub symbol_count: usize,
    pub file_count: usize,
}
```

## 3. JSON / YAML schema

### 3.1 通常出力

通常の `impact -f json|yaml` では、トップレベルに `summary.by_depth` を追加する。

```json
{
  "changed_symbols": [
    {
      "id": "rust:src/lib.rs:fn:foo:10",
      "name": "foo",
      "kind": "function",
      "file": "src/lib.rs",
      "range": { "start_line": 10, "end_line": 18 },
      "language": "rust"
    }
  ],
  "impacted_symbols": [
    {
      "id": "rust:src/lib.rs:fn:bar:25",
      "name": "bar",
      "kind": "function",
      "file": "src/lib.rs",
      "range": { "start_line": 25, "end_line": 31 },
      "language": "rust"
    }
  ],
  "impacted_files": ["src/lib.rs"],
  "edges": [],
  "impacted_by_file": {
    "src/lib.rs": [
      {
        "id": "rust:src/lib.rs:fn:bar:25",
        "name": "bar",
        "kind": "function",
        "file": "src/lib.rs",
        "range": { "start_line": 25, "end_line": 31 },
        "language": "rust"
      }
    ]
  },
  "summary": {
    "by_depth": [
      { "depth": 1, "symbol_count": 1, "file_count": 1 }
    ]
  }
}
```

### 3.2 confidence filter 併用時

`confidence_filter` は現状どおりトップレベル sibling とし、
`summary.by_depth` は **filter 適用後の出力に対応する集計** とする。

```json
{
  "changed_symbols": [...],
  "impacted_symbols": [...],
  "impacted_files": [...],
  "edges": [...],
  "impacted_by_file": {...},
  "summary": {
    "by_depth": [
      { "depth": 1, "symbol_count": 2, "file_count": 1 },
      { "depth": 2, "symbol_count": 3, "file_count": 2 }
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

### 3.3 `--per-seed`

`--per-seed` では `ImpactOutput` が `impacts[].output` にネストされるので、
`by_depth` も同じ場所に入る。

```json
[
  {
    "changed_symbol": { "id": "..." },
    "impacts": [
      {
        "direction": "callers",
        "output": {
          "changed_symbols": [...],
          "impacted_symbols": [...],
          "impacted_files": [...],
          "edges": [...],
          "impacted_by_file": {...},
          "summary": {
            "by_depth": [
              { "depth": 1, "symbol_count": 2, "file_count": 1 }
            ]
          }
        },
        "confidence_filter": {
          "min_confidence": "confirmed",
          "exclude_dynamic_fallback": true,
          "input_edge_count": 5,
          "kept_edge_count": 2
        }
      }
    ]
  }
]
```

## 4. 深さの意味論

ここは実装前に曖昧さを消しておく。

### 4.1 depth の定義

`depth` は、**seed changed symbol 集合から対象 symbol までの最短 hop 数**。

- seed changed symbols は `depth=0`
- `by_depth` は impacted 側の summary なので、**bucket は `depth>=1` のみを出す**
- hop 数は現在の traversal が辿る edge をそのまま使う

### 4.2 集計対象

`by_depth` の bucket は **最終的に `impacted_symbols` に入った symbol 集合** を対象にする。

つまり:
- `changed_symbols` 自体は bucket に含めない
- `impacted_symbols` に入っていない node は bucket に含めない
- 各 symbol は **最短 depth の bucket に 1 回だけ** 入る

この定義により、次の不変条件が成り立つ。

- `sum(by_depth[*].symbol_count) == impacted_symbols.len()`
- `depth` bucket 同士で symbol は重複しない
- `by_depth` は `impacted_symbols` の別表現であり、別 universe を導入しない

### 4.3 file_count の定義

各 bucket の `file_count` は、その depth bucket に属する symbol の **unique file 数**。

- cumulative file 数ではない
- `impacted_files` 全体の file 数でもない
- bucket 内で 2 symbol が同じ file なら 1 と数える

### 4.4 `--direction` ごとの扱い

#### callers
- 現在の reverse edge traversal の最短距離を使う
- `depth=1` は「changed symbol を直接呼ぶ caller」

#### callees
- 現在の forward edge traversal の最短距離を使う
- `depth=1` は「changed symbol が直接呼ぶ callee」
- 現行挙動では、別 seed から再到達した changed symbol が `impacted_symbols` 側へ入ることがある
  (`src/impact.rs:618` 付近の `reached_changed_via_callees`)
- その場合も、**最終出力の `impacted_symbols` に入っているなら by_depth に含める**
- つまり `by_depth` は「changed symbol 由来か否か」ではなく、**最終 output membership** を正とする

#### both
- 現在の union traversal の最短距離を使う
- `depth=1` は caller / callee のどちらでもよい 1-hop 到達 node
- direction 別内訳までは最初の schema では持たない

## 5. 出力の整列規則

`by_depth` の schema は minimal にするが、順序は固定する。

- bucket は `depth` 昇順
- 同じ depth は 1 bucket のみ
- `symbol_count > 0` の depth だけ出す
- impacted symbol が 0 件なら `by_depth` は空配列 `[]`

この最後の点は重要で、
**「feature が有効だが結果が空」** と **「field 自体が未実装 / 非対応」** を区別しやすくするため、
`by_depth` 実装後は空でも配列として出す方針にする。

## 6. `summary` / `by_depth` の optionality

Rust 側の構造体では将来拡張のため `Option` を使ってよいが、
G1-4 の実装完了後の JSON/YAML 契約としては次を推奨する。

- `summary` は `impact` JSON/YAML では常に出す
- `summary.by_depth` も常に出す
- 結果ゼロ件のときは `summary.by_depth: []`

理由:
- consumer が `summary?.by_depth ?? []` のような分岐を減らせる
- `risk` など後続 field は optional のまま増やせる
- `by_depth` は今回の first-class feature なので、省略より安定出力を優先したい

つまり「内部表現では optional 可、外部契約では常時出力」を採る。

## 7. `with_edges` / confidence filter / ignore_dir との関係

### 7.1 `with_edges=false`

`by_depth` は **`edges` の出力有無に依存しない**。

- `with_edges=false` でも `summary.by_depth` は出す
- depth は traversal 中の内部状態から作る
- 公開 `edges` 配列から復元しない

### 7.2 confidence filter

`apply_confidence_filter()` は filtered edge で再計算しているため (`src/bin/dimpact.rs:152` 以降)、
`by_depth` も **filtered graph に対する再計算結果** を採用する。

順序はこうする。

1. 元の graph で impact 計算
2. confidence filter 適用時は filtered refs で再計算
3. 再計算後の結果から `summary.by_depth` を生成
4. `confidence_filter` block は sibling metadata として付与

### 7.3 ignore_dir

`ignore_dir` により除外された symbol は `impacted_symbols` に残らないので、
`by_depth` もその除外後の集合に一致させる。

つまり `by_depth` は常に **最終 output と同一 universe** を見る。

## 8. 内部実装に必要なデータ

現状の `compute_impact()` は `seen` だけを持っており (`src/impact.rs:563`)、
最終 `ImpactOutput` からは最短 depth を復元できない。

そのため G1-4 実装では、最低限次の内部データが必要。

```rust
struct ImpactTraversalResult {
    impacted_symbols: Vec<Symbol>,
    impacted_files: Vec<String>,
    edges: Vec<Reference>,
    impacted_by_file: HashMap<String, Vec<Symbol>>,
    min_depth_by_symbol_id: HashMap<String, usize>,
}
```

ここで重要なのは `min_depth_by_symbol_id`。

- key: symbol id
- value: changed seed 集合からの最短 hop 数
- changed seed 自体は `0`
- impacted 側へ出すときは `>=1` を使う

この map から `by_depth` を生成する。

## 9. 実装方針の固定

G1-4 でぶれないように、実装方針もこの時点で固定しておく。

### 方針 A: traversal と finalizer を分ける

推奨。

1. traversal は node 集合・edge 集合・`min_depth_by_symbol_id` を返す
2. finalizer はそれを `ImpactOutput` + `summary` に整形する
3. Tree-Sitter / cache 経路と LSP 経路で finalizer を共有する

理由:
- `src/impact.rs:677` と `src/engine/lsp.rs:1256` / `src/engine/lsp.rs:2302` の重複を吸収しやすい
- `risk` など後続 summary も finalizer に寄せられる
- confidence filter 後の再計算にも同じ経路を使える

### 方針 B: `ImpactOutput` を直接組み立てる箇所に都度 `summary` を足す

非推奨。

理由:
- LSP 経路で付け忘れや差異が出やすい
- `by_depth` だけなら動いても、後で `risk` を足すと再び散らばる

## 10. Rust struct の提案

G1-4 実装時の具体形としては、次の程度が最小でよい。

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ImpactSummary {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub by_depth: Vec<ImpactDepthBucket>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImpactDepthBucket {
    pub depth: usize,
    pub symbol_count: usize,
    pub file_count: usize,
}
```

`ImpactOutput.summary` 自体は `Option<ImpactSummary>` にしてもよいが、
外部契約としては最終的に常時出力へ寄せたいので、
実装時には `Default` を使って空 summary を安定生成できる形が扱いやすい。

## 11. テスト観点

G1-4 で最低限必要なテスト観点もここで固定する。

1. **basic callers**
   - 1-hop / 2-hop の bucket が正しく出る
2. **with_edges=false**
   - `edges=[]` でも `summary.by_depth` が出る
3. **confidence filter**
   - filtered 結果に応じて bucket が変わる
4. **per-seed**
   - `impacts[].output.summary.by_depth` に載る
5. **both direction**
   - union traversal の最短距離で bucket 化される
6. **zero impacted**
   - `summary.by_depth: []`
7. **ignore_dir**
   - 除外後の output と一致する bucket 集計になる

## 12. 最終決定

G1-3 時点の決定事項は次のとおり。

- `by_depth` は `ImpactOutput.summary.by_depth` に置く
- bucket schema は **`depth` / `symbol_count` / `file_count`** の最小 3 field
- `depth` は changed seed 集合からの最短 hop 数
- bucket は `depth>=1` の impacted symbol のみを集計する
- 各 symbol は最短 depth の bucket に 1 回だけ入る
- `with_edges=false` でも `by_depth` は出す
- confidence filter 時は filtered graph の再計算結果を正とする
- `--per-seed` では `impacts[].output.summary.by_depth` に載せる
- 実装は traversal と finalizer を分け、LSP と非 LSP で共通化する

この schema なら、G1-4 では **互換性を壊さず最小実装** に集中でき、
G1-5 / G1-6 の `risk` も同じ summary 土台の上に自然に追加できる。