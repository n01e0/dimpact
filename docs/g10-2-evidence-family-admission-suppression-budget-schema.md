# G10-2: evidence family ごとの admission / suppression / budget schema

対象: bounded slice planner / Ruby narrow fallback / witness / slice summary

このメモは、G10-1 で固定した

- admission / pruning / stop を evidence 側へ前倒ししたい
- same-path duplicate / same-family sibling / fallback-only noise を compare 前にも扱いたい
- raw file count だけでなく family-aware に budget を切りたい

という方針を受けて、
**G10 以降の planner control に使う coarse-grained な evidence family schema** を定義するためのもの。

machine-readable companion: `docs/g10-2-evidence-family-admission-suppression-budget-schema.json`

ここで言う schema は、raw enum を直ちに全部置き換えるという意味ではない。
G10-2 の役割はまず、

- どの raw evidence kind をどの family に集約して control 判断へ渡すか
- admission / suppression / budget の各段で family をどう読むか
- G10-3 以降の runtime 実装がどの profile shape を共有すべきか

を固定することにある。

---

## design input

主に次を見た。

- `docs/g9-2-evidence-normalization-rules.md`
- `docs/g9-2-evidence-normalization-rules.json`
- `docs/g10-1-g9-admission-pruning-stop-inventory-and-evidence-budget-memo.md`
- `docs/g5-3-bounded-project-slice-policy.md`
- `src/bin/dimpact.rs`
  - `tier2_scoring_summary()`
  - `ruby_require_relative_scoring_summary()`
  - `ruby_narrow_fallback_scoring_summary()`
  - `compare_tier2_candidates()`
  - `plan_bounded_slice()`
- `src/impact.rs`
  - selected/pruned witness reason builders

---

## 1. Goal / Non-goal

## Goal

- raw evidence kind の上に **policy 用の evidence family** を定義する
- candidate ごとに `admission / suppression / budget` を読むための最小 profile shape を固定する
- graph-first / weak require-relative / narrow fallback を同じ vocabulary で比較できるようにする
- G10-3 以降の runtime 実装が family-local representative selection を導入できるようにする
- G10-6 の witness / slice summary が dropped-before-admit や family-budget drop を短く言えるようにする

## Non-goal

- G10-2 で compare 実装をただちに差し替えること
- G10-2 で既存 enum を全面改名すること
- G10-2 で public JSON を full debug trace 化すること
- G10-2 で per-language の例外を無制限に増やすこと

---

## 2. なぜ evidence kind だけでは足りないか

G9 で `primary / support / fallback / negative` という category は揃った。
ただし planner control に使うには、まだ粒度が細かすぎる。

たとえば runtime は raw kind として

- `param_to_return_flow`
- `callsite_position_hint`
- `explicit_require_relative_load`
- `noisy_return_hint`

のような単位を持てるが、G10 で必要なのはしばしばもっと coarse な判断である。

- これは **return continuity 系** の代表候補か
- これは **fallback provenance 系** があるだけで semantic continuity は薄いか
- これは **helper/noise suppressor 系** が強すぎるか
- これは同 family の既存代表を update するのか、しないのか

そのため G10 では、raw kind と compare tuple の間に
**evidence family** という policy bucket を置くのが自然である。

---

## 3. 基本用語

## 3.1 category

G9-2 で定義した 4 category をそのまま使う。

- `primary`
- `support`
- `fallback`
- `negative`

## 3.2 evidence kind

runtime が現在 materialize する具体的な raw label。

例:

- `param_to_return_flow`
- `require_relative_edge`
- `callsite_position_hint`
- `noisy_return_hint`

## 3.3 evidence family

G10 planner control が admission / suppression / budget を読むための **coarse-grained policy bucket**。

family は次を満たすべきである。

- raw kind より粗い
- witness でも読める短い意味を保つ
- graph-first と fallback の両方で使っても意味がぶれにくい
- same-family representative selection に使える

## 3.4 candidate family

candidate が閉じようとしている continuity / completion family。
G10-2 では次を canonical にする。

- `return_continuation`
- `alias_continuation`
- `weak_require_relative_continuation`
- `module_companion_fallback`

`candidate_family` は evidence family そのものではないが、
**どの family budget を使うか** を決める entry point なので schema に含める。

---

## 4. canonical evidence families

G10-2 では、category ごとに少なくとも次の family を canonical にする。

## 4.1 Primary families

### `return_semantic_continuity`

役割:

- return continuation を直接支える semantic continuity

典型 raw source:

- `param_to_return_flow`
- future observed return-passthrough family
- future observed result-assignment continuity when return side を強く閉じる場合

### `alias_semantic_continuity`

役割:

- alias/value/result chain を直接支える semantic continuity

典型 raw source:

- future observed local alias flow
- future observed imported-result alias continuity
- G9 時点の `alias_chain` の semantic 部分

### `fallback_promoted_semantic_continuity`

役割:

- fallback candidate だが、admission 後に semantic continuity を得たことを表す

典型 raw source:

- Ruby narrow fallback candidate が method family / literal target family / local continuity を強く満たした場合

この family は narrow fallback を raw provenance だけで終わらせず、
**fallback candidate にも semantic win path がありうる** ことを表すために置く。

## 4.2 Support families

### `semantic_provenance`

役割:

- candidate が local DFG / propagation / stronger semantic aggregation に支えられていること

典型 raw source:

- `local_dfg_support`
- `symbolic_propagation_support`
- `semantic_support_rank` の元になっている stronger semantic aggregation

### `call_position_strength`

役割:

- late callsite / boundary-side 最終 call のような positional strength

典型 raw source:

- `callsite_position_hint`
- `call_line == side_max_call_line` に対応する positional support

### `positive_lexical_hint`

役割:

- wrapper / alias / path family などの弱い正の structural hint

典型 raw source:

- `name_path_hint` の positive 部分
- `return_flow` / `assigned_result` / `alias_chain` に混ざっていた positive lexical proxy

### `certainty_support`

役割:

- candidate の certainty / provenance strength

典型 raw source:

- `edge_certainty=confirmed|inferred|dynamic_fallback`
- future call-graph certainty split

## 4.3 Fallback families

### `require_relative_provenance`

役割:

- Ruby weak continuation / narrow fallback を bounded に materialize してよい load provenance

典型 raw source:

- `require_relative_edge`
- `explicit_require_relative_load`

### `dynamic_target_provenance`

役割:

- literal `send/public_send` target family が fallback candidate を narrow していること

典型 raw source:

- `dynamic_dispatch_literal_target`

### `companion_path_provenance`

役割:

- companion path / module companion rule による bounded materialization provenance

典型 raw source:

- `companion_file_match`
- `module_companion` の fallback-admission 部分

## 4.4 Negative families

### `helper_noise_suppressor`

役割:

- helper/debug/tmp/noisy-return 系候補を promotion させない suppressor

典型 raw source:

- `noisy_return_hint`
- helper/noise/debug/tmp lexical suppressor

### `fallback_only_suppressor`

役割:

- fallback provenance はあるが semantic continuity が薄く、fallback-only に留めたい候補

典型 raw source:

- fallback family だけがあり primary family が無い状態
- weak require-relative continuation が stronger graph-side semantic family に負ける状態

### `weaker_certainty_suppressor`

役割:

- dynamic/inferred certainty が stronger confirmed side に負けること

典型 raw source:

- `edge_certainty=dynamic_fallback` が stronger certainty loser として観測される場合

### `late_call_only_suppressor`

役割:

- positional strength だけがあり、continuity/fallback provenance が薄い候補

典型 raw source:

- `callsite_position_hint` しか強みがない late-call candidate

---

## 5. raw evidence kind → family mapping

G10-2 では、少なくとも次の coarse mapping を canonical にする。

## 5.1 既存 G9 runtime からの mapping

### primary / support side

- `param_to_return_flow`
  - `primary.return_semantic_continuity`
  - `support.semantic_provenance`
- `return_flow`
  - semantic portion が観測できる時だけ `primary.return_semantic_continuity`
  - lexical portion は `support.positive_lexical_hint`
- `assigned_result`
  - semantic portion が観測できる時だけ `primary.return_semantic_continuity` または `primary.alias_semantic_continuity`
  - lexical portion は `support.positive_lexical_hint`
- `alias_chain`
  - semantic portion が観測できる時だけ `primary.alias_semantic_continuity`
  - lexical portion は `support.positive_lexical_hint`
- `callsite_position_hint`
  - `support.call_position_strength`
- `name_path_hint`
  - positive side は `support.positive_lexical_hint`
  - suppressing side は `negative.helper_noise_suppressor`

### fallback side

- `require_relative_edge`
  - `fallback.require_relative_provenance`
- `explicit_require_relative_load`
  - `fallback.require_relative_provenance`
- `dynamic_dispatch_literal_target`
  - `fallback.dynamic_target_provenance`
- `companion_file_match`
  - `fallback.companion_path_provenance`
- `module_companion`
  - fallback-admission 部分は `fallback.companion_path_provenance`
  - semantic continuity へ昇格できる時だけ `primary.fallback_promoted_semantic_continuity`

### negative / certainty side

- `noisy_return_hint`
  - `negative.helper_noise_suppressor`
- `edge_certainty=dynamic_fallback`
  - positive support としては `support.certainty_support`
  - stronger candidate に負けた理由としては `negative.weaker_certainty_suppressor`

## 5.2 mapping rule

重要なのは 1 raw kind = 1 family に固定しないこと。
G10-2 では、同じ raw signal が

- support 側の provenance
- negative 側の suppressor
- fallback candidate の primary 昇格条件

として別段で読まれうる。

ただし **1 stage 内では 1 つの役割だけを採る**。
これにより multi-count inflation を避ける。

---

## 6. candidate admission profile schema

G10-2 で canonical にする profile shape は conceptually 次である。

```text
CandidateAdmissionProfile {
  source_kind
  candidate_family
  primary_families[]
  support_families[]
  fallback_families[]
  negative_families[]
  admission_class
  representative_key
}
```

## 6.1 fields

### `source_kind`

- `graph_second_hop`
- `narrow_fallback`

G9 と同じく non-evidence dimension。

### `candidate_family`

- `return_continuation`
- `alias_continuation`
- `weak_require_relative_continuation`
- `module_companion_fallback`

これは later budget/suppression の family-local grouping に使う。

### `primary_families[]`

candidate が直接 continuity を示す semantic family。
重複は落とし、family 名 lexical order で安定化する。

### `support_families[]`

strength/provenance/tie-break family。
単独で admission reason にはならない。

### `fallback_families[]`

bounded materialization provenance。
特に narrow fallback / weak require-relative continuation では admission 読みの中心になる。

### `negative_families[]`

suppress / demote / stop family。
absence を implicit にせず、明示 family として保持する。

### `admission_class`

G10-2 では少なくとも次の 4 値を canonical にする。

- `semantic_ready`
  - primary family があり、compare pool に素直に入れてよい
- `fallback_ready`
  - fallback provenance が十分で、bounded fallback candidate として pool に入れてよい
- `structural_only`
  - support/fallback はあるが、semantic continuity は弱い
- `suppressed_before_admit`
  - negative が強すぎる、または stronger representative がいて compare pool に入れない

### `representative_key`

同 path / same-family / same-side candidate をまとめる stable grouping key。
少なくとも conceptually 次を含む。

- boundary side identity
- path
- candidate_family
- fallback provenance family set（必要時）

---

## 7. admission rule schema

G10-2 では candidate admission を次のように定義する。

## 7.1 semantic-ready admission

条件:

- `primary_families` が 1 つ以上ある
- `negative_families` が hard suppress gate を満たさない

性質:

- 通常の graph-side candidate はここへ入るのが理想
- fallback candidate でも semantic continuity を得たらここへ昇格してよい

## 7.2 fallback-ready admission

条件:

- `primary_families` は弱い/空でもよい
- `fallback_families` が family contract を満たす
- `negative_families` が hard suppress gate を満たさない

性質:

- narrow fallback / weak require-relative continuation の bounded candidate
- compare に入るが、semantic-ready より弱く読まれうる

## 7.3 structural-only holding state

条件:

- support はある
- fallback provenance も弱いか不十分
- semantic continuity は無い
- negative が hard suppress gate までは行かない

性質:

- runtime 内部では materialize してもよいが、final compare pool へそのまま入れない方がよい
- G10-3 以降では suppress-before-admit の主要対象

## 7.4 suppressed-before-admit

条件例:

- `negative.helper_noise_suppressor` があり、`primary_families` も `fallback_families` も薄い
- same `representative_key` に stronger semantic-ready candidate がいる
- same candidate_family の代表を更新しない weaker sibling である
- fallback provenance family が partial で contract を満たさない

性質:

- `pruned_candidates` の ranked_out へ行く前に drop される
- G10-6 で compact reason を返す対象になる

---

## 8. suppression rule schema

G10-2 では suppression を 3 段に分ける。

## 8.1 hard suppress gate

compare pool へ入れる前に落とす gate。

代表例:

- `helper_noise_suppressor` が強く、semantic/fallback family が薄い
- same-path stronger representative が既に確定している
- weak require-relative continuation が stronger semantic family に完全に支配される

これは G10-3 の本体である。

## 8.2 soft demotion

admit はするが、family compare で負けやすくする。

代表例:

- `weaker_certainty_suppressor`
- `fallback_only_suppressor`
- `late_call_only_suppressor`

G9 の compare 負けロジックは、G10 ではこの層へ寄せるのが自然。

## 8.3 representative shadowing

candidate 自体を消さなくても、family representative を更新しない場合。

代表例:

- same path, same candidate_family, same fallback provenance family で weaker
- same candidate_family の sibling だが、既存代表より family coverage を増やさない

これは debug 上は suppression として残したいが、
selected path 側では merge/drop されることがある。

---

## 9. budget schema

G10-2 で budget を読む順は次とする。

1. same-path duplicate suppression
2. same candidate-family representative selection
3. family-local budget
4. cross-family per-seed budget

重要なのは、**raw file count budget を最初に使わない** こと。

## 9.1 same-path duplicate budget

ルール:

- 同じ path に対しては、原則 1 representative を維持する
- ただし reason だけは merge してよい
- family coverage を増やさない weaker duplicate は budget を消費させない

## 9.2 candidate-family local budget

G10-2 の canonical local budget は、boundary side ごとに次とする。

- `return_continuation`: 1 representative
- `alias_continuation`: 1 representative
- `weak_require_relative_continuation`: 1 representative
- `module_companion_fallback`: 1 representative

ただしこれは「必ず 4 件残す」という意味ではない。
**family ごとに strongest representative を 1 つまで残せる** という意味である。

## 9.3 family priority rule

cross-family compare 前の優先は conceptually 次にする。

1. `return_continuation`
2. `alias_continuation`
3. `weak_require_relative_continuation`
4. `module_companion_fallback`

ただし、これは G7/G9 の lane 優先をそのまま固定化するというより、
**semantic-ready family を fallback-ready family より先に代表化する** ための初期 rule と読むべきである。

## 9.4 cross-family per-seed budget

G10-2 では final explanation/build budget はまだ維持してよい。
初期値は既存の bounded planner を引き継ぎ、少なくとも conceptually

- `per_seed_tier2_files_max = 2`

を残してよい。

ただし適用順は変える。

- 先に family representatives を作る
- その後 final per-seed cap に入れる

これにより、same family の薄い variation が先に slot を食う問題を減らせる。

## 9.5 family budget exhausted の条件

次の場合は `family_budget_exhausted` と読める。

- same candidate_family の strongest representative が既に残っている
- 新 candidate は representative を update しない
- 追加で残しても cross-family diversity が増えない

G10-6 の compact reason では、これを short label 化できるようにしておく。

---

## 10. candidate family ごとの canonical contract

## 10.1 `return_continuation`

### admission

望ましい状態:

- `primary.return_semantic_continuity` がある
- `support.semantic_provenance` または `support.call_position_strength` が補強する

### suppression

強く抑えたい状態:

- `negative.helper_noise_suppressor`
- `negative.late_call_only_suppressor`
- lexical positive しかなく semantic continuity が無い

### budget reading

- same side の return family は strongest 1 representative を優先
- helper-noise sibling は compare 前に落とすのが望ましい

## 10.2 `alias_continuation`

### admission

望ましい状態:

- `primary.alias_semantic_continuity` がある
- imported-result / local alias continuity を直接支える

### suppression

強く抑えたい状態:

- alias-ish naming しかなく semantic continuity が無い
- same path の stronger alias representative が既にある

### budget reading

- alias family も strongest 1 representative を維持
- return family と cross-family compare する前に sibling を落とす

## 10.3 `weak_require_relative_continuation`

### admission

許される状態:

- `fallback.require_relative_provenance` がある
- semantic-ready family が side 内に無い、または補助的 candidate として残す価値がある

### suppression

強く抑えたい状態:

- require-relative provenance しかなく、side 内に stronger semantic family がある
- companion/runtime narrow fallback へ進めず、fallback-only のまま残る

### budget reading

- weak continuation は return/alias family より後順位
- ただし Ruby side で唯一の bounded continuation なら 1 representative を許す

## 10.4 `module_companion_fallback`

### admission

許される状態:

- `fallback.require_relative_provenance` または `fallback.dynamic_target_provenance` と
  `fallback.companion_path_provenance` が bounded admission を支える
- 可能なら `primary.fallback_promoted_semantic_continuity` が追加される

### suppression

強く抑えたい状態:

- generic dynamic runtime で literal target family を持たない
- companion path だけで semantic continuity も target provenance も薄い
- stronger graph-side representative が既にある

### budget reading

- fallback family では strongest 1 representative を維持
- generic runtime noise は family slot を使わせない

---

## 11. stop rule との接続

G10-2 の schema は budget だけでなく stop rule にも効く。

## 11.1 family-representatives-satisfied stop

次の状態なら side をそれ以上広げなくてよい。

- 必要な candidate family の代表が揃った
- 残り候補は weaker sibling / duplicate / suppressing leftover だけ

## 11.2 only-suppressing-leftovers stop

残候補の profile がほぼ

- `helper_noise_suppressor`
- `fallback_only_suppressor`
- `late_call_only_suppressor`

だけなら、その side/family は打ち切ってよい。

## 11.3 fallback-provenance-exhausted stop

Ruby narrow fallback / weak require-relative continuation では、
次を満たしたらそれ以上 widen しない。

- `require_relative_provenance`
- `dynamic_target_provenance`
- `companion_path_provenance`

の contract を満たす候補がもう無い

---

## 12. witness / slice summary への影響

G10-2 の schema を user-facing surface へそのまま全部出す必要はない。
ただし少なくとも次は出せるようにしておくべきである。

## 12.1 selected / pruned winner-loser で出したいもの

- winning primary family
- winning support family
- winning fallback family
- losing negative family

## 12.2 dropped-before-admit で出したいもの

- `suppressed_before_admit=helper_noise_suppressor`
- `suppressed_before_admit=fallback_only_suppressor`
- `suppressed_before_admit=weaker_same_path_duplicate`
- `family_budget_exhausted=<candidate_family>`

つまり G10-2 の family 名は、
**runtime compare 用 metadata であると同時に compact explanation label** でもあるべきである。

---

## 13. 後続タスクへの接続

## G10-3

`suppress-before-admit` 実装。
`hard suppress gate` と `structural_only` の扱いを runtime へ入れる。

## G10-4

Rust duplicate / sibling 過剰選択の抑制。
特に `return_continuation` / `alias_continuation` family の representative shadowing を実装する。

## G10-5

Ruby fallback admission refinement。
`require_relative_provenance` / `dynamic_target_provenance` / `companion_path_provenance` の contract を narrow fallback runtime に入れる。

## G10-6

witness / slice summary へ dropped-before-admit / family-budget labels を追加する。

## G10-7 / G10-8

family budget exhaustion / duplicate suppression / fallback admission mismatch を eval set と regression に固定する。

---

## 14. 一言まとめ

G10-2 の schema で固定したい核心は、
**raw evidence kind をそのまま count するのではなく、policy 用の evidence family に集約して candidate を admit / suppress / budget する** ことである。

より具体的には、
planner はこれから

- raw kind
- normalized category
- policy family
- candidate family representative

という 4 段を経て判断するべきであり、
その結果として
**弱い候補を compare 前に落とし、同 family の strongest representative を残し、bounded なので止まる理由を family 名で説明できる** ようにするのが G10-2 の目的である。
