# G8-6: evidence-driven fixed evaluation set

このメモは、G8 で入った evidence-driven selection / true narrow fallback / witness explanation を
毎回戻って比較する **固定評価セット** を定義し、
G9 の evidence conflict 系改善に加えて、
G10 で入った **admission conflict / suppress-before-admit / budget exhaustion** をそこへ追加で固定するためのもの。

G5/G7 までは主に bounded slice の広げ方と selected/pruned surface を固定していた。
G8/G9 では scope widening ではなく、
**同じ bounded slice の中で何の evidence が勝敗を決め、どう witness へ出るか** を固定した。
G10 ではその続きとして、
**admit する前に弱い候補を落とすこと、same-family / same-path loser を明示的に残すこと、raw cap を budget exhaustion として区別して残すこと**
を fixed baseline へ足す。

したがって現 HEAD の fixed set は、G8/G9 の 7 ケースを維持しつつ、
G10 の admission conflict / budget exhaustion ケースを追加した **9 ケース** で構成する。

- Rust: 5 ケース
- Ruby: 4 ケース

machine-readable set: `docs/g8-6-evidence-driven-eval-set.json`

---

## 1. この評価セットで見るもの

現行の fixed set で見たいのは主に次の 9 種類。

1. **semantic evidence が positional noise に勝つこと**
   - `param_to_return_flow` が取れた Rust leaf が later helper を押し切れるか
2. **negative / suppressing evidence が noisy helper を落とすこと**
   - return-ish helper noise が lexical hint や later callsite を持っていても selected されないか
3. **same-kind 候補で semantic support の強さが tie-break になること**
   - 同じ evidence kind を持つ Rust 候補同士で、強い semantic aggregation を持つ方が勝てるか
4. **alias 系の明らかに弱い helper が compare 前に suppress されること**
   - alias continuation を壊さず helper noise だけが `suppressed_before_admit` として残るか
5. **same-family sibling が ranked-out ではなく family conflict として残ること**
   - `param_to_return_flow` を持つ leaf が later sibling を `weaker_same_family_sibling` として落とせるか
6. **per-seed raw cap が budget exhaustion として明示されること**
   - side-local loser と seed-wide loser が同じ `ranked_out` に潰れず区別されるか
7. **true narrow fallback が bounded に materialize されること**
   - Ruby `method_missing` companion が graph-first へ滲まず narrow fallback lane として選ばれるか
8. **Ruby fallback-only / dynamic runtime noise が admit 前に落ちること**
   - weak require_relative helper や unrelated runtime family が bounded candidate として残らないか
9. **winner/pruned 差分と compact explanation が witness / slice summary に残ること**
   - `winning_primary_evidence_kinds` / `winning_support` / `compact_explanation` / `summary` が安定しているか

---

## 2. 固定ルール

### 2.1 lane の扱い

この set では、**CLI lane と test lane を両方 first-class に扱う**。

- CLI lane:
  - `impact --with-pdg --format dot`
  - `impact --with-propagation --format json`
- test lane:
  - `cargo test --test cli_pdg_propagation ...`
  - `cargo test --bin dimpact ...`
  - `cargo test --lib ...`

理由:

- Rust / Ruby の competition と filtering は CLI integration で固定するのが最も実態に近い
- true narrow fallback / budget exhaustion / same-path duplicate の raw metadata は planner unit が最も直接
- witness explanation の最小面は `src/impact.rs` unit が最も直接

### 2.2 engine / format

- CLI lane の engine は `ts` 安定面を前提にする
- selected/pruned / scoring / witness explanation が本題のケースは `json`
- selected file の入り方自体を見たいケースだけ `dot` を併用する

### 2.3 更新ルール

この 9 ケースは、現行 evidence-budgeted admission surface の fixed baseline として扱う。
置換するなら、「より直接で、現 HEAD に既に存在する coverage へ置き換える理由」を別メモに残す。

---

## 3. 採用ケース一覧

| case_id | lang | kind | primary view | ねらい |
| --- | --- | --- | --- | --- |
| rust-param-to-return-flow-competition | rust | family conflict regression | dot + json + unit | `param_to_return_flow` leaf が later sibling を `weaker_same_family_sibling` で落とす |
| rust-returnish-helper-negative-evidence | rust | suppressing regression | dot + json + unit | `noisy_return_hint` が return-ish helper を ranked-out に留める |
| rust-semantic-support-tiebreak | rust | tie-break regression | dot + json + unit | 同種 evidence の Rust 候補で `semantic_support_rank` が later callsite hint に勝つ |
| rust-alias-helper-suppress-before-admit | rust | admission conflict regression | dot + json + unit | alias continuation を壊さず helper noise を `suppressed_before_admit` で残す |
| rust-tier2-bridge-budget-exhaustion | rust | budget exhaustion regression | planner unit | side-local loser と seed-wide loser を別 prune reason で残す |
| ruby-method-missing-companion-narrow-fallback | ruby | narrow fallback FN | planner unit | `method_missing` companion を true narrow fallback lane で選ぶ |
| ruby-dynamic-runtime-target-family-filter | ruby | admission conflict regression | json + planner unit | generic runtime noise を filtering しつつ same-path duplicate を残す |
| ruby-winning-evidence-source-kind-explanation | ruby | explanation regression | lib unit | selected/pruned の勝ち筋 evidence/support と losing-side reason が witness summary に出る |
| ruby-require-relative-leaf-competition | ruby | admission conflict regression | dot + json + unit | fallback-only helper を `suppressed_before_admit` で残しつつ semantic leaf を選ぶ |

---

## 4. 各ケースの固定意図

### 4.1 `rust-param-to-return-flow-competition`

**Source**

- `docs/g8-3-rust-param-to-return-evidence.md`
- `tests/cli_pdg_propagation.rs::setup_cross_file_param_passthrough_competition_repo`
- `tests/cli_pdg_propagation.rs::pdg_slice_selection_prefers_param_passthrough_leaf_over_later_neutral_helper`
- `src/bin/dimpact.rs::bounded_slice_plan_prefers_rust_param_passthrough_over_later_neutral_helper`
- `src/bin/dimpact.rs::collect_rust_tier2_semantic_evidence_detects_param_to_return_flow`

**Locked outcome**

- selected files は `main.rs`, `step.rs`, `wrapper.rs`
- `later.rs` は `pruned_candidates[*].prune_reason = weaker_same_family_sibling`
- selected は Tier 2 `bridge_completion_file` / `via_symbol_id = rust:wrapper.rs:fn:wrap:4`
- witness では `rust:step.rs:fn:step:1` 側に selected-vs-pruned explanation が付く

**Expected fields**

- selected scoring:
  - `lane = return_continuation`
  - `primary_evidence_kinds = [assigned_result, param_to_return_flow, return_flow]`
  - `support.local_dfg_support = true`
- pruned scoring (`later.rs`):
  - `lane = return_continuation`
  - `primary_evidence_kinds = [assigned_result, return_flow]`
  - `secondary_evidence_kinds = [callsite_position_hint, name_path_hint]`
- witness reason:
  - `prune_reason = weaker_same_family_sibling`
  - `selected_better_by = primary_evidence_count`
  - `winning_primary_evidence_kinds = [param_to_return_flow]`
  - `winning_support.local_dfg_support = true`

**Regression to catch**

- later helper noise が再び selected へ残る
- same-family loser が `weaker_same_family_sibling` で残らなくなる
- `param_to_return_flow` / `local_dfg_support` が witness explanation から落ちる

---

### 4.2 `rust-returnish-helper-negative-evidence`

**Source**

- `src/bin/dimpact.rs::bounded_slice_plan_penalizes_returnish_helper_noise_against_real_return_completion`
- `tests/cli_pdg_propagation.rs::setup_cross_file_returnish_helper_noise_repo`
- `tests/cli_pdg_propagation.rs::pdg_slice_selection_penalizes_returnish_helper_noise_after_later_callsite`
- `src/impact.rs::selected_vs_pruned_reason_derives_losing_side_reason_from_negative_evidence`

**Locked outcome**

- selected files は `leaf.rs`, `main.rs`, `wrapper.rs`
- `zzz_final_helper.rs` は `ranked_out`
- helper 側だけ `negative_evidence_kinds = [noisy_return_hint]` を持つ
- witness では `losing_side_reason = negative_evidence=noisy_return_hint` が出る

**Regression to catch**

- later return-ish helper が call position や lexical hint で再び勝つ
- `negative_evidence_kinds` が pruned metadata から落ちる
- losing-side reason が witness summary から消える

---

### 4.3 `rust-semantic-support-tiebreak`

**Source**

- `src/bin/dimpact.rs::bounded_slice_plan_prefers_stronger_rust_semantic_support_over_later_callsite_hint`
- `tests/cli_pdg_propagation.rs::setup_cross_file_semantic_support_competition_repo`
- `tests/cli_pdg_propagation.rs::pdg_slice_selection_prefers_stronger_rust_semantic_support_over_later_callsite_hint`

**Locked outcome**

- selected files は `main.rs`, `steady.rs`, `wrapper.rs`
- `plain.rs` は `ranked_out`
- selected/pruned は両方 `return_continuation` かつ同じ primary evidence kind を持つ
- tie-break は `semantic_support_rank` で決まる

**Regression to catch**

- same-kind 候補が later callsite hint だけで決まる
- `semantic_support_rank` が score tuple から落ちる
- stronger semantic leaf が `plain.rs` に押し負ける

---

### 4.4 `rust-alias-helper-suppress-before-admit`

**Source**

- `src/bin/dimpact.rs::bounded_slice_plan_prefers_alias_continuation_over_later_adapter_helper_noise`
- `tests/cli_pdg_propagation.rs::setup_cross_file_imported_result_alias_competition_repo`
- `tests/cli_pdg_propagation.rs::pdg_slice_selection_prefers_alias_continuation_value_over_later_adapter_helper`
- `src/impact.rs::selected_vs_pruned_reason_carries_compact_explanation_for_suppressed_before_admit`

**Locked outcome**

- selected files は `adapter.rs`, `main.rs`, `value.rs`
- `zzz_helper.rs` は `suppressed_before_admit`
- helper 側には `compact_explanation = suppressed_before_admit=helper_noise_suppressor` が付く
- witness でも同じ compact explanation が selected/pruned 理由へ引き継がれる

**Expected fields**

- selected scoring:
  - `lane = alias_continuation`
  - `primary_evidence_kinds = [alias_chain, assigned_result]`
- pruned scoring:
  - `lane = alias_continuation`
  - `primary_evidence_kinds = [assigned_result]`
  - `secondary_evidence_kinds = [callsite_position_hint, name_path_hint]`
- witness reason:
  - `selected_better_by = primary_evidence_count`
  - `winning_primary_evidence_kinds = [alias_chain]`
  - `compact_explanation = suppressed_before_admit=helper_noise_suppressor`

**Regression to catch**

- alias helper noise が compare pool へ残って rank contest を汚す
- `suppressed_before_admit=helper_noise_suppressor` が pruned/witness metadata から落ちる
- selected file が alias continuation ではなく helper 側へぶれる

---

### 4.5 `rust-tier2-bridge-budget-exhaustion`

**Source**

- `src/bin/dimpact.rs::bounded_slice_plan_records_ranked_out_and_budget_pruned_tier2_candidates`

**Locked outcome**

- selected files は `a_leaf.rs`, `a_wrapper.rs`, `b_leaf.rs`, `b_wrapper.rs`, `c_wrapper.rs`, `main.rs`
- `z_alt.rs` は side-local loser として `suppressed_before_admit`
- `c_leaf.rs` は seed-wide loser として `bridge_budget_exhausted`
- same test の中で admission conflict と final seed budget が別 surface として観測できる

**Expected fields**

- `z_alt.rs`
  - `bridge_kind = boundary_alias_continuation`
  - `via_symbol_id = rust:a_wrapper.rs:fn:wrap_a:3`
- `c_leaf.rs`
  - `bridge_kind = wrapper_return`
  - `via_symbol_id = rust:c_wrapper.rs:fn:wrap_c:3`
  - `prune_reason = bridge_budget_exhausted`

**Regression to catch**

- side-local loser と budget loser が同じ `ranked_out` に潰れる
- `bridge_budget_exhausted` candidate が pruned metadata から落ちる
- final cap を widen してしまい `c_leaf.rs` が selected へ混ざる

---

### 4.6 `ruby-method-missing-companion-narrow-fallback`

**Source**

- `docs/g8-1-missing-evidence-inventory-and-design-memo.md`
- `docs/g8-2-bridge-scoring-evidence-schema.json`
- `src/bin/dimpact.rs::bounded_slice_plan_selects_ruby_method_missing_companion_as_narrow_fallback`
- `tests/fixtures/ruby/analyzer_hard_cases_dynamic_dsl_method_missing_chain_v4.rb`

**Locked outcome**

- selected narrow fallback file は `lib/runtime.rb`
- reason は Tier 3 `module_companion_file`
- `via_path` は temp repo 上の `lib/router.rb`
- `pruned_candidates` は空

**Regression to catch**

- true narrow fallback が materialize されず runtime companion を拾えない
- `companion_file_match` / `dynamic_dispatch_literal_target` / `explicit_require_relative_load` の 3 点が scoring に揃わない
- narrow fallback candidate が graph-first selected reason に化ける

---

### 4.7 `ruby-dynamic-runtime-target-family-filter`

**Source**

- `src/bin/dimpact.rs::ruby_narrow_fallback_filters_generic_dynamic_runtime_without_target_family_hint`
- `tests/cli_pdg_propagation.rs::setup_ruby_dynamic_send_runtime_noise_repo`
- `tests/cli_pdg_propagation.rs::pdg_slice_selection_filters_generic_ruby_dynamic_runtime_noise`

**Locked outcome**

- selected files は `app/runner.rb`, `lib/route_runtime.rb`, `lib/service.rb`
- `lib/aaa_runtime.rb` は candidate materialization 前に filtering される
- same selected path へ収束した loser は `weaker_same_path_duplicate` として残る
- `compact_explanation = suppressed_before_admit=weaker_same_path_duplicate` が付く

**Expected fields**

- boundary evidence:
  - `explicit_require_relative_loads` は `lib/aaa_runtime.rb`, `lib/route_runtime.rb` を含む
  - `literal_dynamic_target_hints = [route_, route_created]`
- CLI outcome:
  - `impacted_files` は `lib/aaa_runtime.rb` を含まない
  - selected file は `lib/route_runtime.rb`
  - pruned candidate は `path = lib/route_runtime.rb`, `prune_reason = weaker_same_path_duplicate`

**Regression to catch**

- generic runtime noise が fallback candidate へ残る
- same-path loser bookkeeping が消えて duplicate admission conflict が追えなくなる
- intended runtime (`route_runtime.rb`) まで filtering される

---

### 4.8 `ruby-winning-evidence-source-kind-explanation`

**Source**

- `docs/g8-1-missing-evidence-inventory-and-design-memo.md`
- `docs/g8-2-bridge-scoring-evidence-schema.json`
- `src/impact.rs::selected_vs_pruned_reason_derives_winning_metadata_for_source_kind_explanations`

**Locked outcome**

- selected は `lib/leaf.rb`
- pruned は `lib/helper.rb`
- `selected_better_by = source_kind`
- witness summary には winning evidence / winning support / losing-side reason が揃う

**Regression to catch**

- `winning_primary_evidence_kinds` が selected/pruned 差分から導けなくなる
- `winning_support` が support 差分を落とす
- `losing_side_reason` が fallback-only / dynamic_fallback を落とす

---

### 4.9 `ruby-require-relative-leaf-competition`

**Source**

- `src/bin/dimpact.rs::bounded_slice_plan_prefers_ruby_return_completion_over_later_require_relative_helper_noise`
- `tests/cli_pdg_propagation.rs::setup_ruby_require_relative_competing_leaf_repo`
- `tests/cli_pdg_propagation.rs::pdg_slice_selection_prefers_ruby_require_relative_leaf_over_later_helper_noise`

**Locked outcome**

- selected files は `app/runner.rb`, `lib/leaf.rb`, `lib/service.rb`
- `lib/zzz_helper.rb` は `suppressed_before_admit`
- helper 側には `compact_explanation = suppressed_before_admit=fallback_only_suppressor` が付く
- helper witness の `selected_files_on_path` には `lib/zzz_helper.rb` が入らない

**Expected fields**

- selected scoring:
  - `lane = return_continuation`
  - `primary_evidence_kinds = [assigned_result, return_flow]`
- pruned scoring:
  - `lane = require_relative_continuation`
  - `primary_evidence_kinds = [require_relative_edge]`
  - `secondary_evidence_kinds = [callsite_position_hint]`
- witness reason:
  - `selected_better_by = lane`
  - `winning_primary_evidence_kinds = [assigned_result, return_flow]`
  - `compact_explanation = suppressed_before_admit=fallback_only_suppressor`

**Regression to catch**

- fallback-only helper が compare pool を抜けて semantic leaf を押しのける
- `suppressed_before_admit=fallback_only_suppressor` が pruned/witness metadata から落ちる
- Ruby helper が explanation slice に混ざる

---

## 5. 一言まとめ

G8/G9 までは、主に **何の evidence が selected/pruned の勝敗を決めるか** を固定していた。
G10 でこの fixed set に足したいのは、
**何をそもそも admit しないか、same-family / same-path loser をどう残すか、raw cap を budget exhaustion としてどう区別するか**
である。

したがって現行の eval set は、
**ranking regression の固定セット** であると同時に、
**evidence-budgeted admission / suppression / budget の固定セット** でもある。
