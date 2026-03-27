#![allow(deprecated)]

use predicates::prelude::*;

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
    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert = cmd
        .arg("schema")
        .arg("--id")
        .arg("dimpact:json/v1/diff/default")
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref()).to_string();
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("valid schema document");

    assert_eq!(
        value.get("$id"),
        Some(&serde_json::Value::String(
            "dimpact:json/v1/diff/default".to_string(),
        ))
    );
    assert_eq!(
        value.pointer("/x-dimpact/status"),
        Some(&serde_json::Value::String("placeholder".to_string()))
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
