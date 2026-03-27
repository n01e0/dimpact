#![allow(deprecated)]

use predicates::prelude::*;

fn fetch_schema_document(schema_id: &str) -> serde_json::Value {
    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert = cmd
        .arg("schema")
        .arg("--id")
        .arg(schema_id)
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref()).to_string();
    serde_json::from_str(&stdout).expect("valid schema document")
}

#[test]
fn schema_list_json_reports_registered_ids() {
    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert = cmd.arg("schema").arg("--list").assert().success();

    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref()).to_string();
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("valid schema list json");
    let items = value.as_array().expect("schema list should be an array");

    assert_eq!(items.len(), 15);
    assert!(items.iter().any(|item| {
        item.get("schema_id")
            == Some(&serde_json::Value::String(
                "dimpact:json/v1/diff/default".to_string(),
            ))
    }));
    assert!(items.iter().any(|item| {
        item.get("schema_id")
            == Some(&serde_json::Value::String(
                "dimpact:json/v1/impact/per_seed/with_edges/propagation".to_string(),
            ))
    }));
}

#[test]
fn schema_id_json_returns_registered_document() {
    let value = fetch_schema_document("dimpact:json/v1/diff/default");

    assert_eq!(
        value.get("$id"),
        Some(&serde_json::Value::String(
            "dimpact:json/v1/diff/default".to_string(),
        ))
    );
    assert_eq!(
        value.pointer("/x-dimpact/status"),
        Some(&serde_json::Value::String("concrete".to_string()))
    );
}

#[test]
fn changed_diff_and_id_schemas_are_concrete_and_model_current_shapes() {
    let changed = fetch_schema_document("dimpact:json/v1/changed/default");
    assert_eq!(
        changed.pointer("/x-dimpact/status"),
        Some(&serde_json::Value::String("concrete".to_string()))
    );
    assert_eq!(
        changed.pointer("/required"),
        Some(&serde_json::json!(["_schema", "json_schema", "data"]))
    );
    assert_eq!(
        changed.pointer("/properties/data/required"),
        Some(&serde_json::json!(["changed_files", "changed_symbols"]))
    );
    assert_eq!(
        changed.pointer("/$defs/symbol/properties/kind/enum"),
        Some(&serde_json::json!([
            "function", "method", "struct", "enum", "trait", "module"
        ]))
    );

    let diff = fetch_schema_document("dimpact:json/v1/diff/default");
    assert_eq!(
        diff.pointer("/x-dimpact/status"),
        Some(&serde_json::Value::String("concrete".to_string()))
    );
    assert_eq!(
        diff.get("type"),
        Some(&serde_json::Value::String("object".to_string()))
    );
    assert_eq!(
        diff.pointer("/properties/data/type"),
        Some(&serde_json::Value::String("array".to_string()))
    );
    assert_eq!(
        diff.pointer("/$defs/change_kind/enum"),
        Some(&serde_json::json!(["added", "removed", "context"]))
    );
    assert_eq!(
        diff.pointer("/$defs/file_changes/required"),
        Some(&serde_json::json!(["old_path", "new_path", "changes"]))
    );

    let id = fetch_schema_document("dimpact:json/v1/id/default");
    assert_eq!(
        id.pointer("/x-dimpact/status"),
        Some(&serde_json::Value::String("concrete".to_string()))
    );
    assert_eq!(
        id.get("type"),
        Some(&serde_json::Value::String("object".to_string()))
    );
    assert_eq!(
        id.pointer("/properties/data/type"),
        Some(&serde_json::Value::String("array".to_string()))
    );
    assert_eq!(
        id.pointer("/properties/data/items/required"),
        Some(&serde_json::json!(["id", "symbol"]))
    );
    assert_eq!(
        id.pointer("/$defs/text_range/required"),
        Some(&serde_json::json!(["start_line", "end_line"]))
    );
}

#[test]
fn impact_default_schema_is_concrete_and_models_summary_only_shape() {
    let value = fetch_schema_document("dimpact:json/v1/impact/default/summary_only/call_graph");

    assert_eq!(
        value.pointer("/x-dimpact/status"),
        Some(&serde_json::Value::String("concrete".to_string()))
    );
    assert_eq!(
        value.pointer("/properties/data/properties/edges/maxItems"),
        Some(&serde_json::Value::Number(0.into()))
    );
    assert_eq!(
        value.pointer("/$defs/edge_provenance/enum"),
        Some(&serde_json::json!(["call_graph"]))
    );
    assert_eq!(
        value.pointer("/$defs/reference/required"),
        Some(&serde_json::json!([
            "from",
            "to",
            "kind",
            "file",
            "line",
            "certainty",
            "confidence",
            "provenance"
        ]))
    );
    assert_eq!(
        value.pointer("/$defs/impact_summary/required"),
        Some(&serde_json::json!(["by_depth", "affected_modules", "risk"]))
    );
    assert!(
        value
            .pointer("/$defs/impact_witness/properties/bridge_execution_family")
            .is_none()
    );
}

#[test]
fn impact_variant_schemas_are_concrete_and_capture_profile_specific_constraints() {
    let per_seed = fetch_schema_document("dimpact:json/v1/impact/per_seed/summary_only/call_graph");
    assert_eq!(
        per_seed.pointer("/x-dimpact/status"),
        Some(&serde_json::Value::String("concrete".to_string()))
    );
    assert_eq!(
        per_seed.get("type"),
        Some(&serde_json::Value::String("object".to_string()))
    );
    assert_eq!(
        per_seed.pointer("/properties/data/type"),
        Some(&serde_json::Value::String("array".to_string()))
    );
    assert_eq!(
        per_seed.pointer("/properties/data/items/properties/impacts/maxItems"),
        Some(&serde_json::Value::Number(2.into()))
    );
    assert_eq!(
        per_seed
            .pointer("/properties/data/items/properties/impacts/items/properties/direction/enum"),
        Some(&serde_json::json!(["callers", "callees"]))
    );
    assert_eq!(
        per_seed.pointer(
            "/properties/data/items/properties/impacts/items/properties/output/properties/edges/maxItems"
        ),
        Some(&serde_json::Value::Number(0.into()))
    );

    let with_edges = fetch_schema_document("dimpact:json/v1/impact/default/with_edges/call_graph");
    assert_eq!(
        with_edges.pointer("/x-dimpact/status"),
        Some(&serde_json::Value::String("concrete".to_string()))
    );
    assert!(
        with_edges
            .pointer("/properties/data/properties/edges/maxItems")
            .is_none()
    );
    assert_eq!(
        with_edges.pointer("/$defs/edge_provenance/enum"),
        Some(&serde_json::json!(["call_graph"]))
    );

    let pdg = fetch_schema_document("dimpact:json/v1/impact/default/summary_only/pdg");
    assert_eq!(
        pdg.pointer("/$defs/ref_kind/enum"),
        Some(&serde_json::json!(["call", "data", "control"]))
    );
    assert_eq!(
        pdg.pointer("/$defs/edge_provenance/enum"),
        Some(&serde_json::json!(["call_graph", "local_dfg"]))
    );
    assert_eq!(
        pdg.pointer("/$defs/impact_summary/required"),
        Some(&serde_json::json!([
            "by_depth",
            "affected_modules",
            "risk",
            "slice_selection"
        ]))
    );
    assert!(
        pdg.pointer("/$defs/impact_witness/properties/bridge_execution_family")
            .is_some()
    );

    let propagation =
        fetch_schema_document("dimpact:json/v1/impact/per_seed/with_edges/propagation");
    assert_eq!(
        propagation.pointer("/$defs/edge_provenance/enum"),
        Some(&serde_json::json!([
            "call_graph",
            "local_dfg",
            "symbolic_propagation"
        ]))
    );
    assert!(
        propagation
            .pointer(
                "/properties/data/items/properties/impacts/items/properties/output/properties/edges/maxItems"
            )
            .is_none()
    );
}

#[test]
fn schema_id_rejects_unknown_schema_ids() {
    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    cmd.arg("schema")
        .arg("--id")
        .arg("dimpact:json/v1/does/not/exist")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown schema id"));
}

#[test]
fn schema_resolve_outputs_machine_friendly_profile_info() {
    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert = cmd
        .arg("schema")
        .arg("resolve")
        .arg("impact")
        .arg("--per-seed")
        .arg("--with-edges")
        .arg("--with-propagation")
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref()).to_string();
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("valid resolve json");

    assert_eq!(
        value.get("profile"),
        Some(&serde_json::Value::String(
            "impact/per_seed/with_edges/propagation".to_string(),
        ))
    );
    assert_eq!(
        value.get("schema_id"),
        Some(&serde_json::Value::String(
            "dimpact:json/v1/impact/per_seed/with_edges/propagation".to_string(),
        ))
    );
    assert_eq!(
        value.get("schema_path"),
        Some(&serde_json::Value::String(
            "resources/schemas/json/v1/impact/per_seed/with_edges/propagation.schema.json"
                .to_string(),
        ))
    );
}

#[test]
fn schema_resolve_ignores_content_only_flags_for_profile_resolution() {
    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert = cmd
        .arg("schema")
        .arg("resolve")
        .arg("impact")
        .arg("--direction")
        .arg("both")
        .arg("--max-depth")
        .arg("2")
        .arg("--min-confidence")
        .arg("confirmed")
        .arg("--exclude-dynamic-fallback")
        .arg("--op-profile")
        .arg("precision-first")
        .arg("--seed-symbol")
        .arg("rust:src/lib.rs:fn:foo:12")
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref()).to_string();
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("valid resolve json");

    assert_eq!(
        value.get("profile"),
        Some(&serde_json::Value::String(
            "impact/default/summary_only/call_graph".to_string(),
        ))
    );
}

#[test]
fn schema_resolve_rejects_raw_id_output() {
    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    cmd.arg("schema")
        .arg("resolve")
        .arg("id")
        .arg("--raw")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "schema profile is not available for raw id output",
        ));
}
