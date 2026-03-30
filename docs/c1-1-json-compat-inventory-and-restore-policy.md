# C1-1: schema 導入で壊れた JSON compatibility 面の棚卸しと復元方針

このメモは、C1 の前提として **schema 導入でどこが壊れたか** を棚卸しし、
**どの contract をどこまで戻すか** を先に固定するためのもの。

結論だけ先に書くと、今回戻すべきなのは **`diff` / `changed` / `impact` / `impact --per-seed` / `id -f json` の通常 JSON 出力** であり、
`schema` subcommand は残す。
ただし schema layer は **通常出力の envelope ではなく、通常出力を説明する help / lookup layer** として扱い直す。

---

## 1. break point と restore target

schema 系の導入は段階的に入ったが、**JSON compatibility を実際に壊した commit は `3904eab` (`feat: embed schema metadata in json output`)** だった。

その直前の **`d25f87a` (`feat: add changed diff and id schemas`)** は、
すでに `schema --list` / `schema --id` / `schema resolve ...` と concrete schema files を持っていた一方で、
**通常 JSON 出力はまだ旧 top-level shape のまま** だった。

つまり C1 で戻したい target は、雑に言うと:

- schema CLI は **`d25f87a` 以降のまま維持**
- 通常 JSON 出力は **`d25f87a` 時点の payload shape へ戻す**

という組み合わせになる。

これは「schema 機能を捨てる」のではなく、
**runtime output への envelope 埋め込みだけを撤回する** という整理。

---

## 2. 壊れた user-visible JSON surface

compatibility を壊した面は 5 つ。

| surface | `3904eab` 前 | `3904eab` 後 | 破壊の本質 |
| --- | --- | --- | --- |
| `diff -f json` | top-level array | `{ _schema, json_schema, data: [...] }` | array consumer が全滅 |
| `changed -f json` | top-level object (`changed_files`, `changed_symbols`) | envelope object | root field 参照が 1 段深くなる |
| `impact -f json` | top-level object (`changed_symbols`, `impacted_symbols`, `summary`, ...) | envelope object | root field 参照が 1 段深くなる |
| `impact --per-seed -f json` | top-level array | envelope object | array consumer が全滅 |
| `id -f json` | top-level array | envelope object | array consumer が全滅 |

### 2.1 実装上の break point

`src/bin/dimpact.rs` で `JsonOutputEnvelope` / `print_json_output()` が追加され、
JSON 出力が一律で:

```json
{
  "_schema": { "id": "..." },
  "json_schema": "...",
  "data": ...
}
```

に包まれるようになった。

この変更は `run_diff()` / `run_changed()` / `run_id()` だけでなく、
`print_impact_output()` と `impact --per-seed` の grouped 出力経路にも入っているので、
**JSON の主要 surface 全部が同時に変わった**。

### 2.2 何が「互換 break」なのか

今回の break は field 追加ではなく、**top-level shape の変更**。

- object surface では `payload.foo` が `payload.data.foo` になる
- array surface では `payload[0]` が `payload.data[0]` になる
- `_schema` / `json_schema` / `data` が required になり、旧 parser がそのままでは読めない

特に `diff` / `impact --per-seed` / `id` は元が top-level array なので、
JSON を array 前提で読む consumer にはほぼ確実に breaking change だった。

---

## 3. runtime 以外で巻き込まれた compatibility 面

`3904eab` は runtime だけでなく、**それに追随する unwrap / schema-envelope 前提** を複数箇所へ広げている。
C1 で戻すべきなのは runtime だけではない。

## 3.1 test helper / CLI regression 群

追加・変更された代表面:

- `tests/json_output.rs`
  - `parse_payload()` / `parse_payload_slice()` で `data` を剥がす helper を追加
  - `schema_id()` / `schema_path()` で runtime envelope を assertion する helper を追加
- `tests/cli_changed.rs`
- `tests/cli_impact.rs`
- `tests/cli_integration.rs`
- `tests/changed_impacted_golden_baseline.rs`
- `tests/cli_impact_*` 系
- `tests/cli_go.rs` / `tests/cli_java.rs` / `tests/cli_python*.rs`
- `tests/cli_pdg_propagation.rs`

ここでの問題は 2 種ある。

1. **旧 payload shape を直接見なくなった**
   - helper が `data` を剥がしてしまうので、default JSON が envelope を持つ前提でも test が通る
2. **runtime が `_schema` / `json_schema` を出すこと自体を contract 化してしまった**
   - `cli_changed.rs` / `cli_impact.rs` / `cli_integration.rs` などで schema id/path を assert している

C1 の観点では、この helper 群は「一時的な追随パッチ」であって、維持対象ではない。
**旧 top-level shape を直接固定する test に戻す**必要がある。

## 3.2 automation / CI scripts

追随パッチが入った代表面:

- `scripts/verify-precision-regression.sh`
  - `parse_json_payload()` で envelope を剥がす fallback を追加
- `.github/workflows/dimpact.yaml`
  - `jq` 側で `(.data // .) as $payload` を使って unwrap

これらは「新旧両対応」に見えるが、C1 の文脈ではむしろ問題で、
**default JSON が envelope でも通ってしまう**ので回帰検知が弱くなる。

C1-6 / C1-7 では、単に runtime を戻すだけでなく、
**unwrap shim 自体も消して旧 top-level shape を前提に戻す**方がよい。

## 3.3 schema document 自体

`resources/schemas/json/v1/**` も `3904eab` で envelope 形へ巻き取られた。

たとえば `d25f87a` 時点の `resources/schemas/json/v1/diff/default.schema.json` は:

- top-level `type: array`
- `items: { "$ref": "#/$defs/file_changes" }`

だったのに、現在は:

- top-level `type: object`
- required: `[_schema, json_schema, data]`
- payload array は `/properties/data`

になっている。

同じことが:

- `changed/default`
- `id/default`
- `impact/default/*`
- `impact/per_seed/*`

の全 concrete schema に起きている。

重要なのは、**payload-only の concrete schema は `d25f87a` 時点ですでに存在していた**こと。
つまり C1-4 / C1-5 は新設計ではなく、
**`3904eab` で包んだ schema docs を payload shape に戻す作業**として整理できる。

## 3.4 schema CLI regression / snapshot

追随して変わった面:

- `tests/cli_schema.rs`
  - `/properties/data/...` を前提に schema document を見ている
- `docs/s1-10-schema-registry-snapshot.json`
  - schema document digest を固定しているので、schema docs を戻せば更新が必要
- `docs/s1-10-rollup-summary.md`
  - envelope を public contract として説明している

`schema --list` / `schema resolve ...` 自体は envelope に依存していないが、
**`schema --id` が返す concrete schema document の意味**は envelope 寄りに寄ってしまっている。

C1 ではここを

- `schema --list` / `schema resolve ...` はそのまま維持
- `schema --id` が返す concrete doc は payload shape を表す

へ戻す必要がある。

## 3.5 README / docs の public message

現行 `README.md` と `docs/s1-10-rollup-summary.md` では、
`_schema` / `json_schema` / `data` envelope を **通常 JSON の public contract** として説明している。

これは C1 の目標と真っ向からぶつかる。

C1 の public message は逆であるべき。

- schema は **lookup / inspection layer**
- 通常 JSON 出力は **従来 payload をそのまま返す**
- help を増やすために data envelope を被せない

README / README_ja / rollup docs はこの方針へ揃え直す必要がある。

---

## 4. restore policy

## 4.1 一番大きい原則

**通常 JSON 出力の contract は payload そのもの** とする。

つまり:

- `diff -f json` は array を返す
- `changed -f json` は `changed_files` / `changed_symbols` を root に持つ object を返す
- `impact -f json` は `changed_symbols` / `impacted_symbols` / `summary` などを root に持つ object を返す
- `impact --per-seed -f json` は array を返す
- `id -f json` は array を返す

`_schema` / `json_schema` / `data` は default JSON stdout へ出さない。

## 4.2 schema layer の役割

schema layer は残す。ただし役割をこう定義し直す。

- `schema --list`: どんな payload family があるか調べる
- `schema --id <id>`: その payload family を表す concrete schema document を取得する
- `schema resolve ...`: CLI flag からどの payload family に当たるか機械的に解決する

つまり schema は **通常出力に同梱される transport metadata** ではなく、
**通常出力を説明する補助面** として扱う。

## 4.3 source of truth

復元の source of truth は `d25f87a` に置くのが自然。

理由:

1. `schema` subcommand 群はもう存在している
2. concrete schema docs もすでに payload-only で揃っている
3. envelope 導入前の最後の commit なので、default JSON compatibility の基準として明確

C1-2 の regression fixture / snapshot も、基本的には **`d25f87a` の shape を固定する** 発想でよい。

## 4.4 versioning に関する扱い

S1-2 では「top-level shape が変わるなら schema major を上げる」と書いている。
それに照らすと `3904eab` の envelope 化自体が強い breaking change だった。

C1 の方針としては、ここでさらに

- payload-only を `v2`
- envelope を `v1`

のように並行維持するより、
**S1 の envelope contract を撤回し、現行 schema ids は payload shape を指すものとして立て直す** 方がスコープに合っている。

この tasklist の goal も

- 通常 JSON の互換復元
- schema CLI の維持
- opt-in envelope の新設は out

であって、二重系の schema family を抱えることではないため。

要するに C1 では、schema version を増やすより
**S1 でやり過ぎた runtime envelope を rollback して、schema layer を help 的な位置に戻す** ことを優先する。

---

## 5. 復元の作業順

## 5.1 C1-2: まず旧 shape を fixture / regression として固定

先にやるべきことは runtime 修正ではなく、**何へ戻すかを test で固定すること**。

対象:

- `diff -f json`
- `changed -f json`
- `impact -f json`
- `impact --per-seed -f json`
- `id -f json`

ここでは `_schema` / `json_schema` / `data` が存在しないことまで含めて固定したい。

## 5.2 C1-3: runtime envelope を撤回

`src/bin/dimpact.rs` の修正点は比較的明確。

- `JsonOutputEnvelope` / `print_json_output()` の経路を外す
- `run_diff()` / `run_changed()` / `run_id()` は payload をそのまま serialize
- `print_impact_output()` も payload 直列化へ戻す
- `impact --per-seed` grouped output も payload 直列化へ戻す

この段階では schema registry 自体は壊さなくてよい。

## 5.3 C1-4 / C1-5: concrete schema docs を payload shape 前提へ戻す

ここもゼロから再設計する必要はあまりなく、
基本は `d25f87a` 時点の concrete schema docs と `tests/cli_schema.rs` を restore baseline にできる。

具体的には:

- `diff/default` / `id/default` / `impact/per_seed/*` を top-level array schema へ戻す
- `changed/default` / `impact/default/*` を top-level object schema へ戻す
- `/properties/data/...` 前提の test を root / items 前提へ戻す
- profile slug / schema resolve の対応は維持する

## 5.4 C1-6 / C1-7: unwrap shim を掃除する

runtime が戻ったあとで、追随用 shim を残すと次の回帰を見逃す。

なので:

- `tests/json_output.rs` の payload unwrap helper を撤去または縮小
- runtime `_schema` / `json_schema` assertion を削除
- `scripts/verify-precision-regression.sh` の unwrap fallback を外す
- `.github/workflows/dimpact.yaml` の `(.data // .)` を旧 shape 前提へ戻す
- `docs/s1-10-schema-registry-snapshot.json` の digest を更新

までやってはじめて「compatibility を戻した」と言える。

## 5.5 C1-8 / C1-9: docs message を入れ替える

最後に README / README_ja / rollup docs を更新して、
誤った public message を残さないようにする。

載せるべき message は次の 1 行に尽きる。

> schema は help / lookup の延長であり、通常 JSON 出力の top-level shape は変えない。

---

## 6. non-goals

今回の復元でやらないもの:

- opt-in envelope の新設
- YAML / HTML / DOT の新仕様化
- schema registry の外部ホスティング
- runtime JSON に別名 metadata field を再導入すること
- envelope / non-envelope の並行 family を増やすこと

C1 はあくまで **default JSON compatibility restore** であって、
新しい transport 層を追加する phase ではない。

---

## 7. task mapping

この memo から見た各 task の役割は次の通り。

- **C1-1**: break 面と restore policy を固定する ← このメモ
- **C1-2**: 旧 top-level shape を regression fixture 化
- **C1-3**: runtime envelope 撤回
- **C1-4**: schema CLI が envelope なしでも成立するよう整理
- **C1-5**: impact variant schema / profile 対応を payload shape 前提で維持
- **C1-6**: CI / scripts / tests の unwrap 追随を剥がす
- **C1-7**: schema snapshot / regression 更新
- **C1-8**: README / README_ja / docs の public message 修正
- **C1-9**: rollup で最終整理

---

## 8. 最終結論

C1 で戻すべきものは単純で、
**schema subcommand は残すが、通常 JSON 出力には二度と envelope を被せない**。

そして実装・test・docs の source of truth は、
`3904eab` 以降の envelope 付き状態ではなく、
**`d25f87a` 時点で既に成立していた payload-only contract** に置くのが最も素直。

これで

- 旧 consumer compatibility
- schema lookup 機能の維持
- task scope の抑制

を同時に満たせる。
