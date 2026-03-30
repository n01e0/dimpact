#![allow(deprecated)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

fn run_json(args: &[&str]) -> (String, serde_json::Value) {
    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert = cmd.args(args).assert().success();
    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref()).to_string();
    let value = serde_json::from_str(&stdout).expect("valid json output");
    (stdout, value)
}

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

fn setup_repo() -> (TempDir, PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let repo = dir.path().to_path_buf();
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "tester@example.com"]);
    git(&repo, &["config", "user.name", "Tester"]);

    fs::write(
        repo.join("main.rs"),
        "fn root() {\n    leaf();\n}\n\nfn leaf() {\n    println!(\"one\");\n}\n",
    )
    .expect("write initial rust file");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "init", "-q"]);

    fs::write(
        repo.join("main.rs"),
        "fn root() {\n    leaf();\n}\n\nfn leaf() {\n    println!(\"one\");\n    println!(\"two\");\n}\n",
    )
    .expect("write modified rust file");

    (dir, repo)
}

fn run_json_in_repo(repo: &Path, args: &[&str], stdin: Option<&str>) -> Value {
    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    cmd.current_dir(repo).args(args);
    if let Some(stdin) = stdin {
        cmd.write_stdin(stdin.to_owned());
    }
    let assert = cmd.assert().success();
    serde_json::from_slice(assert.get_output().stdout.as_ref()).expect("valid json output")
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
        "snapshot_scope": {
            "captures": [
                "schema --list",
                "schema --id <schema-id>",
                "schema resolve <command> [flags...]",
            ],
            "excludes": [
                "runtime JSON output from diff -f json",
                "runtime JSON output from changed -f json",
                "runtime JSON output from impact -f json",
                "runtime JSON output from id -f json",
            ],
        },
        "schema_count": schema_list.as_array().expect("schema list array").len(),
        "schema_list": schema_list,
        "resolve_cases": actual_resolve_cases,
        "documents": actual_documents,
    });

    assert_eq!(actual, expected);
}

#[test]
fn runtime_json_outputs_stay_separate_from_schema_subcommand_surfaces() {
    let (_tmp, repo) = setup_repo();
    let diff_text = String::from_utf8(git(&repo, &["diff", "--no-ext-diff", "--unified=0"]).stdout)
        .expect("diff text");

    let diff = run_json_in_repo(&repo, &["diff", "-f", "json"], Some(&diff_text));
    assert!(
        diff.is_array(),
        "diff -f json should stay a top-level array"
    );

    let changed = run_json_in_repo(
        &repo,
        &["changed", "--lang", "rust", "-f", "json"],
        Some(&diff_text),
    );
    let changed_root = changed
        .as_object()
        .expect("changed -f json should stay a top-level object");
    assert!(
        !changed_root.contains_key("_schema")
            && !changed_root.contains_key("json_schema")
            && !changed_root.contains_key("data"),
        "changed -f json should not embed schema subcommand metadata"
    );

    let impact = run_json_in_repo(
        &repo,
        &[
            "impact",
            "--direction",
            "callers",
            "--lang",
            "rust",
            "-f",
            "json",
        ],
        Some(&diff_text),
    );
    let impact_root = impact
        .as_object()
        .expect("impact -f json should stay a top-level object");
    assert!(
        !impact_root.contains_key("_schema")
            && !impact_root.contains_key("json_schema")
            && !impact_root.contains_key("data")
            && impact_root.get("$id").is_none()
            && impact_root.get("x-dimpact").is_none(),
        "impact -f json should stay separate from schema document surfaces"
    );

    let per_seed = run_json_in_repo(
        &repo,
        &[
            "impact",
            "--direction",
            "callers",
            "--lang",
            "rust",
            "--per-seed",
            "-f",
            "json",
        ],
        Some(&diff_text),
    );
    assert!(
        per_seed.is_array(),
        "impact --per-seed -f json should stay a top-level array"
    );

    let id = run_json_in_repo(
        &repo,
        &["id", "--path", "main.rs", "--line", "5", "-f", "json"],
        None,
    );
    assert!(id.is_array(), "id -f json should stay a top-level array");

    let schema_doc = run_json_in_repo(
        &repo,
        &[
            "schema",
            "--id",
            "dimpact:json/v1/impact/default/summary_only/call_graph",
        ],
        None,
    );
    assert_eq!(
        schema_doc.get("$id"),
        Some(&Value::String(
            "dimpact:json/v1/impact/default/summary_only/call_graph".to_string(),
        ))
    );
    assert_eq!(
        schema_doc.pointer("/x-dimpact/status"),
        Some(&Value::String("concrete".to_string()))
    );
}
