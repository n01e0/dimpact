# S1-1: current JSON surface inventory and schema profile design memo

このメモは、S1 の最初の土台として **現在の user-visible JSON 出力面** と **shape-affecting flags** を棚卸しし、
後続の `SchemaProfile` / canonical schema id / `schema resolve` 実装で何を正規化すべきかを決めるための設計メモ。

前提:

- S1 の対象は **JSON schema 体系**。YAML / DOT / HTML は今回の canonical schema profile の対象外。
- goal は「各 JSON 出力が自分で schema id を名乗り、CLI からも同じ id で schema を引けること」。
- なのでここでは **現在の payload shape** と **将来 profile に切るべき差分** を分けて整理する。

---

## 1. まず結論

### 1.1 現在の JSON 出力面は 5 つある

JSON として user-visible な surface は、実質この 5 つ。

1. `diff -f json`
2. `changed -f json`
3. `impact -f json` の通常出力
4. `impact --per-seed -f json`
5. `id -f json` (`--raw` は除外)

### 1.2 profile 軸として切る価値が高いのは `impact` の 3 軸

`impact` だけが shape / value-domain の揺れを複数持つ。
最低限 profile 軸として持つ価値が高いのは次の 3 つ。

- `layout`: `default | per_seed`
- `edge_detail`: `summary_only | with_edges`
- `graph_mode`: `call_graph | pdg | propagation`

これで `impact` の canonical profile は素直に整理できる。

### 1.3 ただし「shape-affecting flag」と「profile に切る flag」は同じではない

たとえば以下は output に影響するが、**別 profile に分けなくても 1 schema で吸収できる**。

- `--direction`
- `--min-confidence`
- `--exclude-dynamic-fallback`
- `--op-profile`
- `--seed-symbol` / `--seed-json`
- `--lang` / `--engine`
- `--ignore-dir` / `--max-depth`

特に `confidence_filter` は presence が変わるが、**optional field** として 1 schema に収める方が自然。
`--op-profile` はさらに `--min-confidence` / `--exclude-dynamic-fallback` へ正規化されるので、profile 軸には不要。

### 1.4 top-level array surfaces は `_schema` 埋め込みの障害になる

現在、次の 3 surface は top-level array。

- `diff -f json`
- `impact --per-seed -f json`
- `id -f json`

S1 の goal には JSON 出力へ `_schema.id` と `json_schema` を載せることが含まれるので、
**top-level array のままでは sibling metadata を足せない**。

このため、S1-9 までにどこかで **metadata envelope 方針** を決める必要がある。
一番素直なのは、JSON schema 対象出力をすべて

```json
{
  "_schema": { ... },
  "json_schema": "...",
  "data": ...
}
```

の共通 envelope に寄せる方針。

この memo 時点ではまだ最終決定まではしないが、**array surface がある以上、無包装のまま埋め込む案は厳しい**。

---

## 2. 現在の JSON surface inventory

## 2.1 `diff -f json`

実装:

- `run_diff()` が `Vec<FileChanges>` をそのまま JSON serialize (`src/bin/dimpact.rs`)
- `FileChanges` / `Change` は `src/diff.rs`

現在 shape:

```json
[
  {
    "old_path": "src/lib.rs",
    "new_path": "src/lib.rs",
    "changes": [
      {
        "kind": "added",
        "old_line": null,
        "new_line": 12,
        "content": "..."
      }
    ]
  }
]
```

top-level:

- **array**

payload type:

- `Vec<FileChanges>`

nested reusable types:

- `FileChanges`
  - `old_path: string|null`
  - `new_path: string|null`
  - `changes: Change[]`
- `Change`
  - `kind: added|removed|context`
  - `old_line: integer|null`
  - `new_line: integer|null`
  - `content: string`

notes:

- empty diff は `[]`
- shape-affecting flag は実質なし
- deprecated `--mode diff` は同じ surface とみなしてよい

### profile recommendation

- 1 profile で十分
- 仮名: `diff/default`

---

## 2.2 `changed -f json`

実装:

- `run_changed()` が `ChangedOutput` を JSON serialize (`src/bin/dimpact.rs`)
- `ChangedOutput` は `src/mapping.rs`
- `Symbol` は `src/ir.rs`

現在 shape:

```json
{
  "changed_files": ["src/lib.rs"],
  "changed_symbols": [
    {
      "id": "rust:src/lib.rs:fn:foo:12",
      "name": "foo",
      "kind": "function",
      "file": "src/lib.rs",
      "range": {
        "start_line": 12,
        "end_line": 20
      },
      "language": "rust"
    }
  ]
}
```

top-level:

- **object**

payload type:

- `ChangedOutput`

nested reusable types:

- `Symbol`
- `TextRange`
- `SymbolKind`

notes:

- `--lang`, `--engine`, `--auto-policy`, `--engine-lsp-strict` は内容に影響しうるが、shape 自体は固定
- `--engine-dump-capabilities` は stderr 追加であり、stdout JSON schema には含めない

### profile recommendation

- 1 profile で十分
- 仮名: `changed/default`

---

## 2.3 `impact -f json` の通常出力

実装:

- `print_impact_output()` が `ImpactOutputRendered` を JSON serialize (`src/bin/dimpact.rs`)
- `ImpactOutputRendered` は `ImpactOutput` を `flatten` し、必要なら `confidence_filter` を sibling として追加
- `ImpactOutput` / `ImpactSummary` / `ImpactWitness` は `src/impact.rs`
- `Reference` は `src/ir/reference.rs`

現在 shape:

```json
{
  "changed_symbols": [...],
  "impacted_symbols": [...],
  "impacted_files": [...],
  "edges": [...],
  "impacted_by_file": {
    "src/lib.rs": [...]
  },
  "impacted_witnesses": {
    "rust:src/lib.rs:fn:foo:12": {
      "symbol_id": "...",
      "depth": 1,
      "root_symbol_id": "...",
      "via_symbol_id": "...",
      "edge": {...},
      "path": [...],
      "provenance_chain": [...],
      "kind_chain": [...]
    }
  },
  "summary": {
    "by_depth": [...],
    "affected_modules": [...],
    "risk": {
      "level": "medium",
      "direct_hits": 1,
      "transitive_hits": 2,
      "impacted_files": 2,
      "impacted_symbols": 3
    }
  },
  "confidence_filter": {
    "min_confidence": "inferred",
    "exclude_dynamic_fallback": false,
    "input_edge_count": 10,
    "kept_edge_count": 7
  }
}
```

top-level:

- **object**

payload type:

- `ImpactOutputRendered`
  - flattened `ImpactOutput`
  - optional `confidence_filter`

nested reusable types:

- `Symbol`
- `Reference`
- `ImpactWitness`
- `ImpactWitnessHop`
- `ImpactSummary`
- `ImpactDepthBucket`
- `ImpactAffectedModule`
- `ImpactRiskSummary`

important current details:

1. `confidence_filter` は **optional sibling**
2. `summary.risk` は struct 上は optional だが、現行 builder は常に `Some(...)` を入れている
3. `impacted_witnesses` は `--with-edges` なしでも出る
4. `edges` field 自体は常に出るが、中身は `--with-edges` や confidence filter 解決後の状態に依存する
5. `Reference` JSON は `certainty` と `confidence` を **両方 serialize** する

`Reference` の現在 shape はこうなる。

```json
{
  "from": "rust:a.rs:fn:f:1",
  "to": "rust:b.rs:fn:g:3",
  "kind": "call",
  "file": "a.rs",
  "line": 10,
  "certainty": "inferred",
  "confidence": "inferred",
  "provenance": "call_graph"
}
```

この `certainty` / `confidence` 重複は schema 上も明示しておく必要がある。

---

## 2.4 `impact --per-seed -f json`

実装:

- `run_impact()` の `per_seed` branch が `Vec<PerSeedOutput>` を直接 JSON serialize (`src/bin/dimpact.rs`)
- `PerSeedOutput` / `PerSeedImpact` は `src/bin/dimpact.rs`

現在 shape:

```json
[
  {
    "changed_symbol": {
      "id": "rust:src/lib.rs:fn:foo:12",
      "name": "foo",
      "kind": "function",
      "file": "src/lib.rs",
      "range": { "start_line": 12, "end_line": 20 },
      "language": "rust"
    },
    "impacts": [
      {
        "direction": "callers",
        "output": {
          "changed_symbols": [...],
          "impacted_symbols": [...],
          "impacted_files": [...],
          "edges": [...],
          "impacted_by_file": {...},
          "impacted_witnesses": {...},
          "summary": {...}
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

top-level:

- **array**

payload type:

- `Vec<PerSeedOutput>`

nested reusable types:

- `Symbol`
- `PerSeedImpact`
  - `direction`
  - `output: ImpactOutput`
  - optional `confidence_filter`

important current details:

1. `--direction both` のときだけ `impacts` は通常 2 要素 (`callers`, `callees`)
2. それ以外の direction では通常 1 要素
3. `output` の中身は **通常 impact の `ImpactOutput`** と同型
4. `confidence_filter` は top-level ではなく `impacts[]` ごとに optional
5. top-level が array なので、ここも `_schema` 埋め込みには envelope が要る

### profile recommendation

- 別 layout profile として扱うべき
- 仮名: `impact/per_seed`

---

## 2.5 `id -f json`

実装:

- `run_id()` が `[{"id": ..., "symbol": ...}, ...]` を直接構築して JSON serialize (`src/bin/dimpact.rs`)

現在 shape:

```json
[
  {
    "id": "rust:src/lib.rs:fn:foo:12",
    "symbol": {
      "id": "rust:src/lib.rs:fn:foo:12",
      "name": "foo",
      "kind": "function",
      "file": "src/lib.rs",
      "range": {
        "start_line": 12,
        "end_line": 20
      },
      "language": "rust"
    }
  }
]
```

top-level:

- **array**

payload type:

- `Vec<{ id, symbol }>`

notes:

- `--raw` は plain text lines であり JSON schema 対象外
- `--path` / `--line` / `--name` / `--kind` / `--lang` は内容や件数には効くが shape は固定
- ここも top-level array なので `_schema` 埋め込みには envelope が必要

### profile recommendation

- 1 profile で十分
- 仮名: `id/default`

---

## 3. shape-affecting flags inventory

ここでは「output を変える flag」を全部書き出したうえで、
**canonical profile に切るべきか** / **同一 schema の optional or enum で吸収できるか** を分ける。

## 3.1 `impact` flags

| flag | current effect | profile 軸に切るか | 理由 |
| --- | --- | --- | --- |
| `--per-seed` | top-level が `object -> array` に変わる | **yes** | これは layout の違いそのもの |
| `--with-edges` | `edges` の運用契約が変わる。非 empty になりうる前提が変わる | **yes** | key set は同じでも consumer expectation がかなり変わる |
| `--with-pdg` | `Reference.kind` / `provenance` / witness path の value-domain が広がる | **yes** | call-graph-only と semantic contract が違う |
| `--with-propagation` | `symbolic_propagation` provenance を含みうる | **yes** | `--with-pdg` よりさらに domain が広がる |
| `--direction` | default では content 差分、per-seed では `impacts` 要素数が 1 or 2 | no | 1 schema で enum / array cardinality 許容が可能 |
| `--min-confidence` | `confidence_filter` が出ることがある。edge 集合も絞られる | no | optional field + value filtering で表現できる |
| `--exclude-dynamic-fallback` | 同上 | no | 同上 |
| `--op-profile` | 内部で `min-confidence` / `exclude-dynamic-fallback` へ正規化 | no | canonical profile 解決時に正規化すればよい |
| `--seed-symbol` / `--seed-json` | diff-based か seed-based かの source が変わる | no | 現行 JSON shape は同型 |
| `--lang` | 内容差分 | no | shape 不変 |
| `--engine` / `--auto-policy` / `--engine-lsp-strict` | 内容差分 | no | shape 不変 |
| `--ignore-dir` / `--max-depth` | 内容差分 | no | shape 不変 |
| `--engine-dump-capabilities` | stderr 出力追加 | no | stdout schema 対象外 |

### 3.1.1 `--with-pdg` と `--with-propagation` を profile 軸に切る理由

これらは top-level key を増やさないので、一見すると 1 schema でも足りる。
でも実際には次の違いがある。

- `Reference.kind` が `call` だけでなく `data` / `control` を持ちうる
- `Reference.provenance` が `call_graph` だけでなく `local_dfg` / `symbolic_propagation` を持ちうる
- `impacted_witnesses[*].provenance_chain` / `kind_chain` の domain も広がる

つまり **shape というより value-domain の contract が変わる**。
そのため canonical schema profile としては分けた方が downstream に優しい。

### 3.1.2 `--with-edges` を profile 軸に切る理由

`edges` field 自体は常に存在するので、厳密な key-set だけ見ると 1 schema でも表現できる。
それでも分ける価値がある理由は次の通り。

- `--with-edges` なしは「graph summary を見る」モード
- `--with-edges` ありは「edge list も API contract に入れる」モード
- 以後の snapshot / regression でも、`edges` を meaningful field として固定したいケースが増える

要するに、**JSON key の違いというより consumer expectation の違い**。
S1 の goal が machine-friendly schema resolution なら、ここは分けた方が扱いやすい。

### 3.1.3 confidence filter 系は profile 軸に入れない

`--min-confidence` / `--exclude-dynamic-fallback` / `--op-profile` は出力に影響するが、
canonical profile としては split しない方がよい。

理由:

- `confidence_filter` は optional field として schema に載せれば足りる
- `op-profile` は user-facing shortcut であって payload family ではない
- ここを profile 軸にすると schema id が細かく割れすぎる
- downstream が本当に欲しいのは「filter metadata が付くかもしれない」ことであって、profile の細分化ではない

---

## 3.2 `changed` flags

| flag | current effect | profile 軸に切るか | 理由 |
| --- | --- | --- | --- |
| `--lang` | 内容差分 | no | shape 固定 |
| `--engine` / `--auto-policy` / `--engine-lsp-strict` | 内容差分 | no | shape 固定 |
| `--engine-dump-capabilities` | stderr 出力追加 | no | stdout schema 対象外 |

結論:

- `changed` は 1 profile で十分

---

## 3.3 `diff` flags

`diff` は実質 `-f json|yaml` くらいしかない。
JSON schema profile 観点では:

- `format=json` の 1 profile だけで十分

---

## 3.4 `id` flags

| flag | current effect | profile 軸に切るか | 理由 |
| --- | --- | --- | --- |
| `--raw` | JSON ではなく plain text lines になる | schema 対象外 | JSON surface ではない |
| `--path` / `--line` / `--name` / `--kind` / `--lang` | 内容差分 | no | shape 固定 |

結論:

- JSON としては 1 profile で十分
- `--raw` は schema subcommand の対象外にするのが自然

---

## 4. reusable nested shape inventory

schema 実装を進めるときは、top-level profile だけでなく再利用断片も切り出しておくと楽。
現時点で切り出し価値が高いのは次。

### 4.1 common

- `TextRange`
- `SymbolKind`
- `Symbol`

### 4.2 diff

- `ChangeKind`
- `Change`
- `FileChanges`

### 4.3 impact

- `RefKind`
- `EdgeCertainty`
- `EdgeProvenance`
- `Reference`
- `ImpactDepthBucket`
- `ImpactAffectedModule`
- `ImpactRiskSummary`
- `ImpactWitnessHop`
- `ImpactWitness`
- `ImpactSummary`
- `ConfidenceFilterSummary`

### 4.4 one subtle schema detail: `Reference`

`Reference` は現行 serialize で

- `certainty`
- `confidence`

を両方出す。
これは legacy / rename 移行のためには理解できるが、schema としては以下のどちらかを明示しないと曖昧になる。

1. **現行互換優先**: 両方 required にする
2. **移行優先**: `certainty` required, `confidence` optional alias にする

S1-1 時点では、**現在の user-visible output を schema 化する**のが先なので、
まずは **両方 present の現行互換** を前提にしておくのが安全。
整理は後で別タスクに切ればいい。

---

## 5. schema profile の切り方: 提案

## 5.1 `SchemaProfile` は「payload family」を表す

後続タスクでは、CLI flag 全部を profile に入れるのではなく、
**payload family を決める最小軸だけ**を `SchemaProfile` に持たせるのがよい。

提案イメージ:

```text
SchemaProfile {
  subcommand: diff | changed | impact | id,
  impact_layout: default | per_seed | null,
  impact_edge_detail: summary_only | with_edges | null,
  impact_graph_mode: call_graph | pdg | propagation | null,
}
```

ここで `null` は `impact` 以外では未使用という意味。

このくらいに留めると、resolver が安定しやすい。

## 5.2 正規化ルールの提案

CLI から profile へ落とすときの正規化は次でよい。

### `impact`

- `--per-seed` あり → `layout=per_seed`
- それ以外 → `layout=default`
- `--with-propagation` あり → `graph_mode=propagation`
- else if `--with-pdg` あり → `graph_mode=pdg`
- else → `graph_mode=call_graph`
- `--with-edges` あり → `edge_detail=with_edges`
- else → `edge_detail=summary_only`
- `--op-profile` は内部で `min-confidence` / `exclude-dynamic-fallback` へ正規化するが、profile には入れない
- `--direction both` は schema id を分けず、schema 側で `impacts` の長さ 1..2 を許容する

### `diff` / `changed` / `id`

- 追加 profile 軸なし
- それぞれ default 1 種でよい

## 5.3 resulting normalized profile space

S1 scope で normalized profile として必要なのは、**flat enum の列挙ではなく軸の直積**として考えるのが自然。

### non-impact

- `diff/default`
- `changed/default`
- `id/default`

### impact axes

- `layout`: `default | per_seed`
- `edge_detail`: `summary_only | with_edges`
- `graph_mode`: `call_graph | pdg | propagation`

この 3 軸で考えると、`impact` の normalized combination は **最大 12 通り**。

- `default × summary_only × call_graph`
- `default × with_edges × call_graph`
- `default × summary_only × pdg`
- `default × with_edges × pdg`
- `default × summary_only × propagation`
- `default × with_edges × propagation`
- `per_seed × summary_only × call_graph`
- `per_seed × with_edges × call_graph`
- `per_seed × summary_only × pdg`
- `per_seed × with_edges × pdg`
- `per_seed × summary_only × propagation`
- `per_seed × with_edges × propagation`

実際の schema source を 12 個の独立ファイルにする必要はない。
ただし **canonical id 解決**の観点では、この normalized combination を一意に表現できる必要がある。

注意:

- `--with-propagation` は意味的には `pdg` を包含するが、resolver 側では `graph_mode=propagation` として単独正規化すれば十分
- resolver 内部の normalized form は、**layout / edge_detail / graph_mode の 3 軸**で持っておくのがよい
- profile family を flat enum に潰すより、3 軸 struct の方が拡張しやすい

---

## 6. `_schema` 埋め込みと envelope の設計メモ

これは S1-9 の実装論点だが、S1-1 の時点で先に注意しておいた方がいい。

## 6.1 現状の問題

現在の JSON payload には top-level array がある。
そのため、object surface だけに `_schema` を足して、array surface は別扱いにすると、
schema resolution の UX が歪む。

たとえばこうなる。

- `changed`: `{ ..., "_schema": ... }`
- `diff`: `[{...}, {...}]` なので同じ足し方ができない
- `impact --per-seed`: 同じく top-level array
- `id`: 同じく top-level array

これは後でかなり面倒。

## 6.2 推奨方針

JSON schema 対象の出力は、最終的に **共通 envelope** に寄せるのが一番きれい。

候補:

```json
{
  "_schema": {
    "id": "..."
  },
  "json_schema": "...",
  "data": ...
}
```

この形にすると:

- object payload も array payload も同じ位置に metadata を載せられる
- `schema --id` の返り値 schema と runtime output の対応がわかりやすい
- `schema resolve ...` の結果と実 payload を素直に突き合わせられる

## 6.3 まだ決め切らなくてよい点

S1-1 ではまだ以下は open でよい。

- `json_schema` に URL を入れるか、repo-relative path を入れるか
- `_schema` 内に `version` / `profile` / `generated_at` まで入れるか
- schema document 自体が envelope schema なのか payload schema なのか

ただし、**top-level array payload を持つ以上、何らかの envelope が必要**という点は先に共有しておくべき。

---

## 7. 後続タスクへの引き継ぎ

## 7.1 S1-2 で決めるべきこと

- `SchemaProfile` struct の最終 shape
- canonical schema id の文字列規則
- normalized axes から id をどう組み立てるか
- profile family と schema document path の対応規則

## 7.2 S1-3 で実装する resolver の要件

resolver は「全部の CLI flag を覚える」ものではなく、
**schema profile に効く差分だけを canonicalize する**ものにする。

最低限やること:

- subcommand 判定
- `impact` の `layout` / `edge_detail` / `graph_mode` 正規化
- `--op-profile` の内部正規化（ただし schema id には反映しない）
- deprecated `--mode` を subcommand へ正規化

## 7.3 S1-6〜S1-8 の schema 実装順

自然な順番はこれ。

1. `impact/default`
2. `impact/per_seed`
3. `impact/with_edges`
4. `impact/pdg`
5. `impact/propagation`
6. `changed`
7. `diff`
8. `id`

理由:

- 一番 shape 差分が多いのは `impact`
- nested reusable fragments も `impact` が最も多い
- `diff` / `changed` / `id` は後からでも整理しやすい

---

## 8. 最終提案

この S1-1 memo としては、次を採用するのが良い。

1. **現在の JSON surface は 5 つ** と認識する
   - `diff`
   - `changed`
   - `impact`
   - `impact --per-seed`
   - `id`

2. **canonical `SchemaProfile` は payload family を表す最小軸に絞る**
   - `subcommand`
   - `impact.layout`
   - `impact.edge_detail`
   - `impact.graph_mode`

3. **profile に入れない flag を明示する**
   - `direction`
   - confidence filter 系
   - seed source 系
   - lang / engine 系
   - depth / ignore 系

4. **`impact` は `layout / edge_detail / graph_mode` の 3 軸正規化で進める**

5. **`diff` / `impact --per-seed` / `id` が top-level array なので、S1-9 では共通 envelope がほぼ必要**

この整理なら、S1-2 以降で schema id 規則を決めても、不要な profile explosion を避けつつ、
`schema resolve` に必要な決定性も確保できる。
