#![allow(deprecated)]

use serde_json::{Map, Value, json};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

fn git(cwd: &Path, args: &[&str]) -> std::process::Output {
    let mut cmd = Command::new("git");
    cmd.args(args).current_dir(cwd);
    let out = cmd.output().expect("git command failed to spawn");
    if !out.status.success() {
        panic!(
            "git {:?} failed: status {:?}\nstdout:{}\nstderr:{}",
            args,
            out.status,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    out
}

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/json_compat_restore")
}

fn load_fixture(name: &str) -> Value {
    let path = fixture_dir().join(name);
    serde_json::from_str(&fs::read_to_string(path).expect("read fixture")).expect("valid fixture")
}

fn setup_repo() -> (TempDir, PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let repo = dir.path().to_path_buf();
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "tester@example.com"]);
    git(&repo, &["config", "user.name", "Tester"]);

    let fixtures = fixture_dir();
    fs::write(
        repo.join("main.rs"),
        fs::read_to_string(fixtures.join("main_before.rs")).expect("read before fixture"),
    )
    .expect("write before fixture");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "init", "-q"]);

    fs::write(
        repo.join("main.rs"),
        fs::read_to_string(fixtures.join("main_after.rs")).expect("read after fixture"),
    )
    .expect("write after fixture");

    (dir, repo)
}

fn parse_plain_json_output(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).expect("valid json output")
}

fn kind_name(value: &Value) -> Value {
    Value::String(
        match value {
            Value::Null => "null",
            Value::Bool(_) => "bool",
            Value::Number(_) => "number",
            Value::String(_) => "string",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
        }
        .to_string(),
    )
}

fn immediate_shape(value: &Value) -> Value {
    match value {
        Value::Array(items) => json!({
            "type": "array",
            "item": items.first().map(immediate_shape).unwrap_or_else(|| Value::String("empty".to_string())),
        }),
        Value::Object(obj) => {
            let mut shape = Map::new();
            for (key, child) in obj {
                shape.insert(key.clone(), kind_name(child));
            }
            Value::Object(shape)
        }
        _ => kind_name(value),
    }
}

fn assert_fixture_has_no_schema_envelope(value: &Value, name: &str) {
    let root = value
        .as_object()
        .unwrap_or_else(|| panic!("{name} should be an object fixture when checking envelope"));
    assert!(
        !root.contains_key("_schema"),
        "{name} should not carry runtime _schema metadata"
    );
    assert!(
        !root.contains_key("json_schema"),
        "{name} should not carry runtime json_schema metadata"
    );
    assert!(
        !root.contains_key("data"),
        "{name} should not carry a runtime data envelope"
    );
}

#[test]
fn json_compat_restore_fixtures_capture_pre_envelope_top_level_shapes() {
    let diff = load_fixture("diff.json");
    let diff_items = diff
        .as_array()
        .expect("diff fixture should be a top-level array");
    assert!(!diff_items.is_empty(), "diff fixture should not be empty");
    let diff_item = diff_items[0]
        .as_object()
        .expect("diff item should be an object");
    assert_eq!(
        diff_item.keys().cloned().collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "old_path".to_string(),
            "new_path".to_string(),
            "changes".to_string(),
        ]),
        "diff fixture should keep the old top-level array item shape"
    );

    let changed = load_fixture("changed.json");
    assert_fixture_has_no_schema_envelope(&changed, "changed fixture");
    let changed_root = changed
        .as_object()
        .expect("changed fixture should be a top-level object");
    assert_eq!(
        changed_root.keys().cloned().collect::<BTreeSet<_>>(),
        BTreeSet::from(["changed_files".to_string(), "changed_symbols".to_string(),]),
        "changed fixture should keep the old root object shape"
    );

    let impact = load_fixture("impact.json");
    assert_fixture_has_no_schema_envelope(&impact, "impact fixture");
    let impact_root = impact
        .as_object()
        .expect("impact fixture should be a top-level object");
    assert_eq!(
        impact_root.keys().cloned().collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "changed_symbols".to_string(),
            "impacted_symbols".to_string(),
            "impacted_files".to_string(),
            "edges".to_string(),
            "impacted_by_file".to_string(),
            "impacted_witnesses".to_string(),
            "summary".to_string(),
        ]),
        "impact fixture should keep the old root object shape"
    );

    let id = load_fixture("id.json");
    let id_items = id
        .as_array()
        .expect("id fixture should be a top-level array");
    assert!(!id_items.is_empty(), "id fixture should not be empty");
    let id_item = id_items[0]
        .as_object()
        .expect("id item should be an object");
    assert_eq!(
        id_item.keys().cloned().collect::<BTreeSet<_>>(),
        BTreeSet::from(["id".to_string(), "symbol".to_string()]),
        "id fixture should keep the old top-level array item shape"
    );
}

#[test]
fn current_payload_shapes_match_json_compat_restore_fixtures() {
    let (_tmp, repo) = setup_repo();

    let diff_text = String::from_utf8(git(&repo, &["diff", "--no-ext-diff", "--unified=0"]).stdout)
        .expect("diff text");

    let mut diff_cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let diff_assert = diff_cmd
        .current_dir(&repo)
        .arg("diff")
        .arg("-f")
        .arg("json")
        .write_stdin(diff_text.clone())
        .assert()
        .success();
    let diff_actual = parse_plain_json_output(diff_assert.get_output().stdout.as_ref());
    assert_eq!(
        immediate_shape(&diff_actual),
        immediate_shape(&load_fixture("diff.json")),
        "diff payload shape drifted from the pre-envelope fixture"
    );

    let mut changed_cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let changed_assert = changed_cmd
        .current_dir(&repo)
        .arg("changed")
        .arg("--lang")
        .arg("rust")
        .arg("-f")
        .arg("json")
        .write_stdin(diff_text.clone())
        .assert()
        .success();
    let changed_actual = parse_plain_json_output(changed_assert.get_output().stdout.as_ref());
    assert_eq!(
        immediate_shape(&changed_actual),
        immediate_shape(&load_fixture("changed.json")),
        "changed payload shape drifted from the pre-envelope fixture"
    );

    let mut impact_cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let impact_assert = impact_cmd
        .current_dir(&repo)
        .arg("impact")
        .arg("--direction")
        .arg("callers")
        .arg("--lang")
        .arg("rust")
        .arg("-f")
        .arg("json")
        .write_stdin(diff_text)
        .assert()
        .success();
    let impact_actual = parse_plain_json_output(impact_assert.get_output().stdout.as_ref());
    assert_eq!(
        immediate_shape(&impact_actual),
        immediate_shape(&load_fixture("impact.json")),
        "impact payload shape drifted from the pre-envelope fixture"
    );

    let mut id_cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let id_assert = id_cmd
        .current_dir(&repo)
        .arg("id")
        .arg("--path")
        .arg("main.rs")
        .arg("--line")
        .arg("5")
        .arg("-f")
        .arg("json")
        .assert()
        .success();
    let id_actual = parse_plain_json_output(id_assert.get_output().stdout.as_ref());
    assert_eq!(
        immediate_shape(&id_actual),
        immediate_shape(&load_fixture("id.json")),
        "id payload shape drifted from the pre-envelope fixture"
    );
}
