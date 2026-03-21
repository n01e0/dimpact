# G9-2: evidence normalization rules (`primary / support / fallback / negative`)

対象: bounded slice planner / Ruby narrow fallback / witness explanation

このメモは、G9-1 で整理した

- planner は lexical proxy と semantic fact が混ざっている
- fallback はより factual だが planner と同じ vocabulary で比較されていない
- witness は winning-side の差分だけを抜いていて、losing-side や suppressing signal をほぼ持っていない

というズレに対して、
**G9 以降で evidence をどの category で扱うか** を先に固定するためのもの。

machine-readable companion: `docs/g9-2-evidence-normalization-rules.json`

design input:

- `docs/g8-2-bridge-scoring-evidence-schema.md`
- `docs/g9-1-g8-evidence-usage-inventory-and-gap-memo.md`

---

## 1. Goal / Non-goal

## Goal

- evidence を `primary / support / fallback / negative` の 4 category に正規化する
- planner / fallback / witness が同じ category vocabulary を共有できるようにする
- 既存 G8 evidence kind を「そのまま残すもの」「split するもの」「compat alias に落とすもの」に分ける
- G9-3 以降の scoring 実装と G9-6 の witness explanation が、同じ normalized contract を参照できるようにする

## Non-goal

- G9-2 で runtime compare 関数を実装変更すること
- G9-2 で enum を直ちに増減すること
- G9-2 で full proof trace や multi-path witness を設計すること
- lexical hint を完全廃止すること

---

## 2. 正規化の基本原則

## 2.1 `source_kind` / `lane` は evidence ではない

G8 では `source_kind` と `lane` が実質的に evidence の一部のように読める場面があったが、
G9 ではまずこれを切り分ける。

- `source_kind`
  - candidate が **どの収集経路** から来たか
- `lane`
  - candidate が **どの continuity / completion family** を閉じようとしているか
- `evidence`
  - candidate を選ぶ/落とすための根拠

したがって、`graph_second_hop` や `narrow_fallback` は ranking dimension ではあるが、
`primary / support / fallback / negative` の 4 category には入れない。

## 2.2 evidence は「観測 fact」と「選択上の役割」で分類する

同じ runtime 事実でも、G9 では次の観点で置き場を決める。

1. その fact は **candidate continuity を直接示すか**
2. その fact は **strength / certainty / tie-break** を補強するだけか
3. その fact は **fallback candidate を bounded に許可する理由** か
4. その fact は **候補を suppress / demote / losing-side 化する理由** か

この役割の違いを明示しない限り、planner / fallback / witness は同じ単語を使っても同じ意味にならない。

## 2.3 lexical proxy は primary へ直接入れない

G9 で最も重要な rule はこれである。

- 名前や path 由来の hint だけで `primary` を立てない
- lexical hint は `support` または `negative` に落とす
- primary はあくまで「continuity / selection を直接観測した fact」に寄せる

これにより、G8 の

- `return_flow`
- `assigned_result`
- `alias_chain`

が name/path proxy で立っていた部分を、正規化後は別 category へ分離できる。

---

## 3. 4 category の定義

## 3.1 Primary

### 定義

`primary` は、candidate がその lane で選ばれるべきことを直接示す **continuity fact / semantic fact** である。

### ルール

- candidate-local に観測できること
- ranking/witness の両方で同じ意味で読めること
- lexical naming だけでは立たないこと
- fallback candidate の admission reason とは分けること
- shared でも losing-side でも意味が変わらないこと

### G9 での canonical 役割

- planner では最も重要な正の signal
- witness では `winning_primary_evidence` / `losing_primary_evidence` の基礎になる
- fallback candidate であっても continuity fact が観測できるなら primary へ載せてよい

### 典型例

- `param_to_return_flow`
- 実観測された `return_passthrough`
- 実観測された `result_assignment_continuity`
- 実観測された `local_alias_flow`

## 3.2 Support

### 定義

`support` は、primary や candidate の信頼度を補強するが、**それだけで candidate を正当化しない** strength/provenance/tie-break signal である。

### ルール

- 単独では candidate admission の理由にならない
- 単独では `primary` を代替しない
- shared/diff の両方で witness に出せること
- provenance / certainty / positional strength / weak structural hint をここへ置く

### G9 での canonical 役割

- planner では primary の次に比較される補強 signal
- witness では `winning_support` / `losing_support` の基礎になる
- negative とは別に「正の補強」であることを保つ

### 典型例

- `call_graph_support`
- `local_dfg_support`
- `symbolic_propagation_support`
- `edge_certainty`
- `callsite_position_hint`
- positive 側の弱い structural/name hint

## 3.3 Fallback

### 定義

`fallback` は、graph-first では拾えない candidate を **bounded に出現させてよい理由** を表す category である。

### ルール

- continuity fact そのものではなく、fallback admission provenance に寄せる
- graph-first candidate と narrow fallback candidate を比較するときも、まず「なぜこの fallback candidate が存在してよいか」を説明する用途に使う
- fallback のみで strong primary を置き換えない
- fallback candidate の witness では、winning/losing 両方の bounded rule を短く読めるようにする

### G9 での canonical 役割

- planner では fallback lane の candidate admission reason
- fallback runtime では raw observation を normalized fallback facts に変換する場所
- witness では `winning_fallback` / `losing_fallback` の素材になる

### 典型例

- `explicit_require_relative_load`
- `companion_file_match`
- `dynamic_dispatch_literal_target`
- weak Ruby continuation 側の `require_relative_edge`

## 3.4 Negative

### 定義

`negative` は、candidate を suppress / demote / fallback-only に留める **負の signal / 抑制 signal** である。

### ルール

- ただ「fact が無い」ことではなく、実際に比較や説明へ使う明示 signal にする
- positive evidence を上書きせず、別 category として持つ
- planner では suppress / rank demotion / promotion cap に使う
- witness では losing-side 理由の短い説明に使う

### G9 での canonical 役割

- helper/noise を primary/support から切り離す
- dynamic fallback が stronger certainty に負けた理由を説明できるようにする
- fallback candidate が出現はしたが selected されなかった理由を表す

### 典型例

- helper/noise/debug/tmp 系 name/path hint
- fallback-only で semantic continuity が無いことを示す suppressing flag
- weaker `edge_certainty`（差分としては support 側に出るが、比較ロジック上は negative にも変換できる）
- late-call-only で他の continuity が無い候補

---

## 4. 正規化ルール

## 4.1 candidate admission rule

candidate を出現させる条件は次で正規化する。

### graph-first candidate

- source は graph から来る
- 選択の正当化は `primary` と `support` で行う
- weak structural continuation (`require_relative_edge` など) は `fallback` 相当として保持してよいが、primary へは入れない

### narrow fallback candidate

- candidate の存在理由は `fallback` に必ず残す
- fallback だけで selected されることは許すが、その場合も witness で「fallback admission に依存した勝ち/負け」を読める必要がある
- fallback candidate に semantic continuity fact が取れた場合だけ、それを `primary` へ追加する

## 4.2 ranking rule

正規化後の比較は conceptually 次の順をとる。

1. `source_kind`
2. `lane`
3. `primary`
4. `support`
5. `fallback`
6. `negative`
7. deterministic lexical tiebreak

ここで重要なのは、`negative` を「最後の雑な補正」にしないこと。
G9-3 の実装では、次のどちらかで扱う。

- candidate promotion 前の suppress gate
- compare の終盤で negative profile を比較する rule

どちらを選んでも、**negative を独立 category として保持する** ことが G9-2 の契約である。

## 4.3 witness rendering rule

witness は正規化後、少なくとも次を surface として持てるようにする。

- winning primary
- winning support
- winning fallback
- losing negative

G8 では winning-side の差分だけだったが、G9-2 では losing-side の簡易理由を category として先に定義する。

## 4.4 shared evidence rule

shared evidence は witness で常に全文出す必要はないが、internal comparison では保持してよい。
ただし summary を作るときは

- winning difference
- losing suppressor
- fallback-only admission

を優先し、shared evidence は省略可能とする。

---

## 5. G8 inventory の category mapping

## 5.1 そのまま primary に残すもの

- `param_to_return_flow`

これは G8 時点でも local DFG / function summary 由来の fact なので、そのまま primary に残してよい。

## 5.2 split して扱うもの

### `return_flow`

G8 では lexical proxy を多く含むため、そのまま primary へ残さない。
正規化後は次に split する。

- observed return continuity -> `primary`
- return-ish naming/path hint -> `support` または `negative`

### `assigned_result`

G8 では wrapper/path hint と混ざっているため split が必要。

- observed result assignment continuity -> `primary`
- wrapper-ish naming/path proxy -> `support`

### `alias_chain`

G8 では alias-ish naming で立つことがあるため split が必要。

- observed local alias continuity -> `primary`
- alias/value/result naming hint -> `support`

### `name_path_hint`

G9 では 1 個の positive hint として持たない。
少なくとも次へ split する。

- weak positive structural hint -> `support`
- helper/noise/debug/tmp suppressor -> `negative`

## 5.3 fallback に移すもの

- `require_relative_edge`
- `explicit_require_relative_load`
- `companion_file_match`
- `dynamic_dispatch_literal_target`

特に `require_relative_edge` は G8 では graph-second-hop 側の primary に入っているが、
正規化後は **continuity fact というより fallback/structural provenance** として扱う。

## 5.4 support に残すもの

- `call_graph_support`
- `local_dfg_support`
- `symbolic_propagation_support`
- `edge_certainty`
- `callsite_position_hint`

`callsite_position_hint` は G8 でも primary ではなく secondary だったので、
G9 では support として明示的に扱うのが自然である。

## 5.5 compat alias または廃止候補

### `module_companion`

G8 runtime では実質未使用で、役割も `companion_file_match` と重なっている。
したがって G9 では次のどちらかに寄せる。

- compat alias として残し、normalized surface では `fallback.companion_file_match` に畳む
- runtime で未使用なら将来的に削る

G9-2 の時点では **compat alias 扱い** とするのが安全である。

---

## 6. cross-surface contract

## 6.1 Planner contract

planner は normalized された category を少なくとも内部的に分けて扱う。

- `primary`
  - continuity を示す main signal
- `support`
  - provenance / certainty / tie-break 補強
- `fallback`
  - candidate admission provenance
- `negative`
  - suppress / demote / losing-side signal

少なくとも G9-3 以降、planner は lexical proxy だけで primary count を水増ししてはいけない。

## 6.2 Fallback runtime contract

fallback runtime は raw observation をそのまま planner scoring に流さず、
次の 2 段を踏む。

1. boundary/candidate observation を集める
2. それを normalized `fallback` / `support` / 必要なら `primary` に写像する

これにより Ruby narrow fallback と weak require-relative continuation の vocabulary を揃えられる。

## 6.3 Witness contract

witness は selected/pruned comparison から、最低限次を生成できる必要がある。

- winning primary
- winning support
- winning fallback
- losing negative

この contract が入ることで、G9-6 の「losing-side の簡易理由」が無理なく置ける。

---

## 7. 正規化後の最小 example

### graph-first Rust winner

```json
{
  "source_kind": "graph_second_hop",
  "lane": "return_continuation",
  "primary": ["param_to_return_flow"],
  "support": ["local_dfg_support", "callsite_position_hint"],
  "fallback": [],
  "negative": []
}
```

### narrow fallback Ruby candidate

```json
{
  "source_kind": "narrow_fallback",
  "lane": "module_companion_fallback",
  "primary": [],
  "support": ["edge_certainty=dynamic_fallback"],
  "fallback": [
    "explicit_require_relative_load",
    "companion_file_match",
    "dynamic_dispatch_literal_target"
  ],
  "negative": ["fallback_only_without_semantic_continuity"]
}
```

この shape なら witness は次のように短く言える。

- selected over `helper.rb` because winning fallback: `explicit_require_relative_load + companion_file_match`; losing negative: `fallback_only_without_semantic_continuity`

---

## 8. G9 task mapping

## G9-3

- `negative` / suppressing evidence を planner scoring に入れる
- helper/noise 系 hint を positive count から外す

## G9-4

- Rust 側の `return_flow` / `assigned_result` / `alias_chain` を observed fact 寄りに再分類する
- lexical proxy を support/negative へ分ける

## G9-5

- Ruby weak continuation と true narrow fallback を同じ normalized table で扱えるようにする
- `require_relative_edge` を fallback 側へ寄せる

## G9-6

- witness に `winning_fallback` と `losing_negative` を追加する
- runtime の `ModuleCompanionFile` 競合も selected/pruned explanation に乗せる

---

## 9. Conclusion

G8 までで evidence vocabulary は増えたが、surface ごとの意味密度はまだ揃っていない。
G9-2 の正規化 rule は次の一点に尽きる。

**primary は continuity fact、support は strength/provenance、fallback は bounded admission reason、negative は suppress/demotion reasonとして分ける。**

この分離が入ると、

- planner は lexical proxy を primary から外せる
- fallback は graph-first と同じ comparison story に載せられる
- witness は winning-side だけでなく losing-side も同じ vocabulary で説明できる

以後の G9 runtime 変更は、この contract を壊さない範囲で行うべきである。
