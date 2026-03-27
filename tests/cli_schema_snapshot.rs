#![allow(deprecated)]

use std::fs;
use std::path::PathBuf;

use serde_json::json;
use sha2::{Digest, Sha256};

fn run_json(args: &[&str]) -> (String, serde_json::Value) {
    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert = cmd.args(args).assert().success();
    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref()).to_string();
    let value = serde_json::from_str(&stdout).expect("valid json output");
    (stdout, value)
}

#[test]
fn schema_registry_snapshot_matches_current_outputs() {
    let snapshot_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("docs")
        .join("s1-10-schema-registry-snapshot.json");
    let expected: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&snapshot_path).expect("read snapshot"))
            .expect("parse snapshot");

    let (_, schema_list) = run_json(&["schema", "--list"]);
    let resolve_cases = [
        vec!["diff"],
        vec!["changed", "--lang", "rust"],
        vec!["impact"],
        vec!["impact", "--with-edges"],
        vec!["impact", "--per-seed", "--with-pdg"],
        vec!["impact", "--per-seed", "--with-edges", "--with-propagation"],
        vec!["id"],
    ];

    let actual_resolve_cases: Vec<_> = resolve_cases
        .iter()
        .map(|argv| {
            let args = [vec!["schema", "resolve"], argv.clone()].concat();
            let (_, result) = run_json(&args);
            json!({ "argv": argv, "result": result })
        })
        .collect();

    let actual_documents: Vec<_> = schema_list
        .as_array()
        .expect("schema list array")
        .iter()
        .map(|item| {
            let schema_id = item
                .get("schema_id")
                .and_then(|v| v.as_str())
                .expect("schema id");
            let (stdout, doc) = run_json(&["schema", "--id", schema_id]);
            json!({
                "schema_id": schema_id,
                "schema_path": item.get("schema_path").cloned().expect("schema path"),
                "title": doc.get("title").cloned().expect("title"),
                "status": doc.pointer("/x-dimpact/status").cloned().expect("status"),
                "sha256": format!("{:x}", Sha256::digest(stdout.as_bytes())),
            })
        })
        .collect();

    let actual = json!({
        "generated_by": "scripts/collect-schema-snapshot.py",
        "schema_count": schema_list.as_array().expect("schema list array").len(),
        "schema_list": schema_list,
        "resolve_cases": actual_resolve_cases,
        "documents": actual_documents,
    });

    assert_eq!(actual, expected);
}
