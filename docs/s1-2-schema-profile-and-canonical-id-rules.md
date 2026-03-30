# S1-2: `SchemaProfile` / canonical schema id のルール

このメモは、S1-1 で棚卸しした current JSON surface を前提に、
後続の `schema resolve` / `schema --id` / `_schema.id` 埋め込みで共有するための
**`SchemaProfile` の定義** と **canonical schema id の命名規則** を固定する。

> Historical note: C1 の JSON compatibility restore 以降、通常の `diff` / `changed` / `impact` / `id` の JSON 出力は payload-only のままで、`_schema.id` / `json_schema` / `data` wrapper を runtime に埋め込まない。ここでの `_schema.id` / `json_schema` / envelope に関する議論は、schema layer をどう設計していたかを示す設計メモとして読むこと。

対象:

- `diff -f json`
- `changed -f json`
- `impact -f json`
- `impact --per-seed -f json`
- `id -f json` (`--raw` は除外)

非対象:

- YAML / DOT / HTML
- runtime 実装そのもの
- schema document の中身の詳細

---

## 1. ここで固定したいこと

この task で決めるべきことは 4 つだけ。

1. `SchemaProfile` は **何を表す型か**
2. CLI 引数から **どう正規化して profile を決めるか**
3. profile から **どう canonical schema id を作るか**
4. その id を **いつ変えるべきか**

S1-1 で見えた重要点は次だった。

- JSON surface は 5 種ある
- `impact` だけが複数の profile 軸を持つ
- `--direction` や confidence filter 系は output に影響するが、schema family を分けるほどではない
- top-level array surface があるので、後で `_schema.id` を埋めるには envelope が要る

この task では、その判断を **Rust の型と id 規則に落ちる粒度** にする。

---

## 2. 決定: `SchemaProfile` は「payload family」を表す

`SchemaProfile` は、CLI の全引数を丸ごと保存する型ではない。

**同じ JSON schema を共有できる payload family を表す正規化後の型**とする。

つまり:

- content を変えるだけの flag は持たない
- stderr だけに効く flag は持たない
- user-visible schema family を変える差分だけを持つ

この前提で、`SchemaProfile` は **non-impact 用の固定 family** と
**impact 用の 3 軸 family** を表せば十分。

---

## 3. 決定: `SchemaProfile` の Rust 形

最終的な実装形としては、次の enum + nested struct がよい。

```rust
pub enum SchemaProfile {
    DiffDefault,
    ChangedDefault,
    IdDefault,
    Impact(ImpactSchemaProfile),
}

pub struct ImpactSchemaProfile {
    pub layout: ImpactSchemaLayout,
    pub edge_detail: ImpactSchemaEdgeDetail,
    pub graph_mode: ImpactSchemaGraphMode,
}

pub enum ImpactSchemaLayout {
    Default,
    PerSeed,
}

pub enum ImpactSchemaEdgeDetail {
    SummaryOnly,
    WithEdges,
}

pub enum ImpactSchemaGraphMode {
    CallGraph,
    Pdg,
    Propagation,
}
```

### 3.1 なぜ enum にするか

S1-1 では `subcommand + optional axes` でも整理できたが、
実装としては enum の方が自然。

理由:

- `diff` / `changed` / `id` に無意味な `None` field を持たせずに済む
- `impact` だけが variant を持つことが型で表現できる
- `match` で schema file path / id slug を組み立てやすい
- unsupported surface (`id --raw`, `-f yaml`) と schema-backed surface を分けやすい

### 3.2 ここで `format` を profile に入れない理由

S1 の対象は JSON schema 体系だけなので、`SchemaProfile` 自体には `format` を持たせない。

代わりに resolver の入口で:

- `format=json` のときだけ profile 解決を許可
- それ以外は unsupported surface としてエラー

にする。

つまり **format は profile の一部ではなく、profile 解決の前提条件**。

---

## 4. 決定: CLI → `SchemaProfile` の正規化ルール

## 4.1 全体ルール

CLI から profile を決めるときは、
**schema family を変える差分だけを正規化**する。

以下は profile に入れない。

- `--direction`
- `--max-depth`
- `--min-confidence`
- `--exclude-dynamic-fallback`
- `--op-profile`
- `--seed-symbol`
- `--seed-json`
- `--lang`
- `--engine`
- `--auto-policy`
- `--engine-lsp-strict`
- `--ignore-dir`
- `--engine-dump-capabilities`

理由は単純で、これらは

- content だけを変える
- optional field の有無で吸収できる
- stderr 専用
- もしくは internal execution detail

だから。

## 4.2 `diff`

- `diff -f json` → `SchemaProfile::DiffDefault`
- deprecated `--mode diff -f json` も同じ profile へ正規化

他に variant 軸は持たない。

## 4.3 `changed`

- `changed -f json` → `SchemaProfile::ChangedDefault`
- deprecated `--mode changed -f json` も同じ profile へ正規化

`--lang` / `--engine` は profile に入れない。

## 4.4 `id`

- `id -f json` → `SchemaProfile::IdDefault`
- `id --raw` は JSON surface ではないので **schema profile なし**

`schema resolve id --raw` は unsupported として弾くべき。

## 4.5 `impact`

`impact` は次の 3 軸だけを正規化する。

### layout

- `--per-seed` あり → `PerSeed`
- それ以外 → `Default`

### edge_detail

- `--with-edges` あり → `WithEdges`
- それ以外 → `SummaryOnly`

### graph_mode

- `--with-propagation` あり → `Propagation`
- else if `--with-pdg` あり → `Pdg`
- else → `CallGraph`

結果として:

```rust
SchemaProfile::Impact(ImpactSchemaProfile {
    layout,
    edge_detail,
    graph_mode,
})
```

を返す。

---

## 5. subtle but important normalization rules

ここは実装でブレやすいので、先に固定しておく。

## 5.1 `edge_detail` は internal compute flag ではなく、user-visible output 契約で決める

`impact` 実装では、confidence filter があると内部的には edge を再計算する。
しかし `--with-edges` が無ければ、最終出力では `edges` を空に戻す path がある。

なので schema profile の `edge_detail` は、
**internal に edge を計算したかどうか**ではなく、
**user-visible payload が edge detail surface として扱われるか**で決める。

つまり:

- `impact --min-confidence confirmed -f json`
  - profile は `SummaryOnly`
- `impact --with-edges --min-confidence confirmed -f json`
  - profile は `WithEdges`

この区別を固定する。

## 5.2 `graph_mode=Propagation` は `--with-pdg` を包含して正規化する

`--with-propagation` は意味的に PDG path を含むが、schema id 上は

- `Pdg`
- `Propagation`

を別 mode として扱う。

したがって:

- `--with-propagation`
- `--with-pdg --with-propagation`

は同じ `graph_mode=Propagation` に正規化する。

## 5.3 `--direction` は profile に入れない

`impact --direction both --per-seed` では `impacts` が 2 要素になりやすいが、
ここは別 schema id にしない。

理由:

- outer layout は同じ `per_seed`
- `direction` は field の enum 値 / array cardinality の範囲で表現できる
- `direction callers` / `callees` / `both` で id を分けると explosion が始まる

したがって `impact/per_seed/...` schema は

- `impacts.len() == 1`
- `impacts.len() == 2`

の両方を許容する前提にする。

## 5.4 confidence filter 系は canonical id に入れない

以下は id に入れない。

- `--min-confidence`
- `--exclude-dynamic-fallback`
- `--op-profile`

理由:

- `confidence_filter` は optional metadata block として schema に載せれば足りる
- `op-profile` は shortcut であって独立 family ではない
- ここを id に入れると practical に使いにくい

### 正規化ルール

resolver は `--op-profile` を受けても、schema id を増やしてはいけない。

たとえば:

- `impact --op-profile balanced -f json`
- `impact --min-confidence inferred -f json`
- `impact -f json`

は全部同じ `SchemaProfile::Impact { layout: Default, edge_detail: SummaryOnly, graph_mode: CallGraph }`
を返す。

違いは **payload content** と **optional `confidence_filter` presence** 側で吸収する。

---

## 6. 決定: profile slug の規則

canonical schema id を作る前に、まず `SchemaProfile` から
**version 非依存の profile slug** を得るルールを固定する。

この slug は:

- `schema resolve` の内部結果
- schema file path
- `_schema` metadata の debug 用表示

などに使い回しやすい。

規則:

- segment はすべて **snake_case**
- segment separator は `/`
- default も省略しない

## 6.1 non-impact

- `DiffDefault` → `diff/default`
- `ChangedDefault` → `changed/default`
- `IdDefault` → `id/default`

## 6.2 impact

- `Impact { Default, SummaryOnly, CallGraph }`
  → `impact/default/summary_only/call_graph`
- `Impact { Default, WithEdges, Pdg }`
  → `impact/default/with_edges/pdg`
- `Impact { PerSeed, WithEdges, Propagation }`
  → `impact/per_seed/with_edges/propagation`

### 6.3 なぜ default を省略しないか

`impact/call_graph` のように省略すると、
後で variant 軸が増えたときに backward-compatible な規則が崩れやすい。

`default` を明示しておけば:

- flat で見ても意味が分かる
- file path にしやすい
- `resolve` 出力が機械的になる
- schema snapshot 名も揃う

ので、冗長でも明示した方がよい。

---

## 7. 決定: canonical schema id の文字列規則

canonical schema id は、profile slug に

- namespace
- format
- schema major version

を前置したものにする。

## 7.1 canonical id grammar

```text
schema-id := "dimpact:" format "/v" major "/" profile-slug
format    := "json"
major     := 1 | 2 | ...
```

S1 開始時点では `format=json`, `major=1` のみを使う。

## 7.2 examples

- `dimpact:json/v1/diff/default`
- `dimpact:json/v1/changed/default`
- `dimpact:json/v1/id/default`
- `dimpact:json/v1/impact/default/summary_only/call_graph`
- `dimpact:json/v1/impact/default/with_edges/call_graph`
- `dimpact:json/v1/impact/default/summary_only/pdg`
- `dimpact:json/v1/impact/default/with_edges/pdg`
- `dimpact:json/v1/impact/per_seed/summary_only/propagation`
- `dimpact:json/v1/impact/per_seed/with_edges/propagation`

## 7.3 この形式を選ぶ理由

### shell-friendly

- URL のような `://` が無い
- quoting せずに扱いやすい
- `schema --id <id>` にそのまま入れやすい

### path-friendly

`dimpact:` を外せば、そのまま file path にしやすい。

例:

- id: `dimpact:json/v1/impact/default/with_edges/pdg`
- path suffix: `json/v1/impact/default/with_edges/pdg`

### namespace が明確

将来もし別 schema source を扱うとしても、`dimpact:` prefix で衝突を避けやすい。

---

## 8. 決定: canonical id と schema file path の対応規則

S1-4 以降で `schema --id` を実装するときのために、
**canonical id → repo 内 schema document path** の規則もここで決める。

提案:

```text
resources/schemas/<format>/v<major>/<profile-slug>.schema.json
```

例:

- `dimpact:json/v1/diff/default`
  → `resources/schemas/json/v1/diff/default.schema.json`
- `dimpact:json/v1/changed/default`
  → `resources/schemas/json/v1/changed/default.schema.json`
- `dimpact:json/v1/id/default`
  → `resources/schemas/json/v1/id/default.schema.json`
- `dimpact:json/v1/impact/default/summary_only/call_graph`
  → `resources/schemas/json/v1/impact/default/summary_only/call_graph.schema.json`

### 8.1 この path 規則の利点

- id から機械的に path を引ける
- `schema --list` の列挙も簡単
- snapshot / fixture の置き場所が安定する
- schema source を later task で追加しやすい

### 8.2 `json_schema` field との関係

将来 runtime output に入れる `json_schema` は、
canonical id そのものではなく **schema document locator** 側に使う。

つまり:

- `_schema.id` = canonical schema id
- `json_schema` = その id が指す schema document の path / URL

という役割分担にする。

---

## 9. 決定: `schema resolve` の返り値ルール

S1-5 で `schema resolve <subcommand> ...` を作るとき、
resolver が返すべき核は次の 3 つ。

1. normalized `SchemaProfile`
2. canonical schema id
3. schema document path

最低限の machine-friendly shape イメージ:

```json
{
  "profile": "impact/per_seed/with_edges/propagation",
  "schema_id": "dimpact:json/v1/impact/per_seed/with_edges/propagation",
  "schema_path": "resources/schemas/json/v1/impact/per_seed/with_edges/propagation.schema.json"
}
```

ここで `profile` を string slug でも返しておくと、
内部 Rust enum をそのまま外へ露出しなくて済む。

---

## 10. 決定: unsupported surface の扱い

schema resolver は「なんでも id を返す」ものにしない。

次は unsupported として明示的に落とす。

- `-f yaml`
- `-f dot`
- `-f html`
- `id --raw`
- 将来 schema 未実装の subcommand / variant

理由:

- S1 の対象は JSON schema 体系だけ
- 無理に近い id を返すと contract が曖昧になる
- `schema resolve` は machine-friendly であるほど、unsupported をはっきり返した方がよい

---

## 11. 決定: versioning / stability ルール

canonical schema id は、単なる表示ラベルではなく
**schema contract の stable identifier** として扱う。

そのため version segment の意味を先に固定する。

## 11.1 `v1` は schema major version

- `v1` は CLI version ではない
- release tag と 1:1 対応しない
- same release の中で schema major は普通変えない

`v1` が変わるのは、**同じ profile slug に対して互換でない schema 変更**が入るときだけ。

## 11.2 `v1` を維持してよい変更

- description / title / comments / examples の修正
- schema file path の内部実装改善（id 自体は同じ）
- analysis quality 改善で payload content が変わること
- optional field がもともと schema に許可されていて、実際の emit 率だけが変わること
- ordering の安定化など、schema contract を壊さない変更

## 11.3 `v2` へ上げるべき変更

- top-level shape が変わる
- required field を追加 / 削除 / rename する
- field type を変える
- enum / domain を互換なく変える
- same schema id なのに consumer が別 parser を要求されるような変更
- object schema を closed contract として持つ前提で、新しい user-visible field を追加する

### 11.3.1 特に注意する点

S1 の schema は **deterministic id** を重視するので、
object schema は基本的に closed contract 前提で考える。

つまり新 field の追加も、基本的には schema major bump 候補になる。

これは少し厳しめだが、

- snapshot test
- canonical id stability
- downstream validator の分かりやすさ

を優先すると、この方が安全。

---

## 12. examples: CLI からどう解決されるか

## 12.1 same profile, different content flags

### A

```bash
dimpact impact -f json
```

→ profile slug:

```text
impact/default/summary_only/call_graph
```

→ schema id:

```text
dimpact:json/v1/impact/default/summary_only/call_graph
```

### B

```bash
dimpact impact --direction both --max-depth 2 --seed-symbol 'rust:src/lib.rs:fn:foo:12' -f json
```

→ **同じ** profile slug / id

理由:

- seed source
- direction
- depth

は schema family を変えないから。

## 12.2 confidence filter stays in the same family

```bash
dimpact impact --min-confidence confirmed --exclude-dynamic-fallback -f json
```

→ profile slug:

```text
impact/default/summary_only/call_graph
```

`confidence_filter` は optional metadata として schema 側で許容する。

## 12.3 visible edge detail changes family

```bash
dimpact impact --with-edges -f json
```

→ profile slug:

```text
impact/default/with_edges/call_graph
```

## 12.4 propagation wins over pdg

```bash
dimpact impact --with-pdg --with-propagation --per-seed --with-edges -f json
```

→ profile slug:

```text
impact/per_seed/with_edges/propagation
```

## 12.5 unsupported example

```bash
dimpact id --raw
```

→ schema resolve result:

- unsupported surface
- no schema id

---

## 13. 最終決定

S1-2 の結論は次。

1. `SchemaProfile` は **payload family を表す enum** とする
2. non-impact は `DiffDefault` / `ChangedDefault` / `IdDefault` の 3 family
3. `impact` は `layout / edge_detail / graph_mode` の 3 軸で表す
4. profile slug は snake_case + `/` 区切りで、default も省略しない
5. canonical schema id は
   - `dimpact:json/v1/<profile-slug>`
   の形式にする
6. schema file path は
   - `resources/schemas/json/v1/<profile-slug>.schema.json`
   に対応づける
7. `direction` / confidence filter / seed source / lang / engine 系は schema id に入れない
8. unsupported surface (`yaml` / `dot` / `html` / `id --raw`) には schema id を割り当てない
9. version segment は schema major として扱い、breaking contract change のときだけ上げる

このルールなら、S1-3 で resolver を実装するときに迷いが少なく、
S1-4 以降の schema file path / `_schema.id` / `json_schema` 埋め込みも機械的につなげられる。
