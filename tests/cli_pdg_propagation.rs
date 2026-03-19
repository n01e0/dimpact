#![allow(deprecated)]
use predicates::prelude::*;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn git(cwd: &std::path::Path, args: &[&str]) -> std::process::Output {
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

fn setup_repo() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    let src = r#"fn callee(a: i32) -> i32 { a + 1 }
fn caller() {
    let x = 1;
    let y = callee(x);
    println!("{}", y);
}
"#;
    fs::write(path.join("f.rs"), src).unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    // change x assignment to force a diff in caller
    let src2 = r#"fn callee(a: i32) -> i32 { a + 1 }
fn caller() {
    let x = 2;
    let y = callee(x);
    println!("{}", y);
}
"#;
    fs::write(path.join("f.rs"), src2).unwrap();

    (dir, path)
}

fn setup_two_arg_repo() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    let src = r#"fn callee(a: i32, b: i32) -> i32 { b + 1 }
fn caller() {
    let x = 1;
    let y = 2;
    let out = callee(x, y);
    println!("{}", out);
}
"#;
    fs::write(path.join("f.rs"), src).unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    let src2 = r#"fn callee(a: i32, b: i32) -> i32 { b + 1 }
fn caller() {
    let x = 3;
    let y = 2;
    let out = callee(x, y);
    println!("{}", out);
}
"#;
    fs::write(path.join("f.rs"), src2).unwrap();

    (dir, path)
}

#[test]
fn pdg_propagation_adds_var_to_callee_edge() {
    let (_tmp, repo) = setup_repo();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();

    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert = cmd
        .current_dir(&repo)
        .arg("impact")
        .arg("--with-pdg")
        .arg("--with-propagation")
        .arg("--format")
        .arg("dot")
        .write_stdin(diff)
        .assert()
        .success()
        .stdout(predicate::str::contains("rust:f.rs:fn:callee:1"));

    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref());
    // Roughly ensure there's an edge into the callee symbol ID
    assert!(stdout.contains("\"rust:f.rs:fn:callee:1\""));
}

#[test]
fn pdg_path_assigns_confirmed_or_inferred_confidence_only() {
    let (_tmp, repo) = setup_repo();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();

    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert = cmd
        .current_dir(&repo)
        .arg("impact")
        .arg("--with-pdg")
        .arg("--with-propagation")
        .arg("--with-edges")
        .arg("--format")
        .arg("json")
        .write_stdin(diff)
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 output");
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("json output");
    let edges = v["edges"].as_array().expect("edges array");
    assert!(!edges.is_empty(), "expected non-empty edges");

    let certainties: std::collections::BTreeSet<String> = edges
        .iter()
        .filter_map(|e| e["certainty"].as_str().map(|s| s.to_string()))
        .collect();

    assert!(
        certainties
            .iter()
            .all(|c| c == "confirmed" || c == "inferred"),
        "unexpected certainty values: {:?}",
        certainties
    );
    assert!(
        !certainties.contains("dynamic_fallback"),
        "PDG path should not emit dynamic_fallback certainty"
    );
}

#[test]
fn pdg_propagation_adds_direct_summary_bridge_for_single_line_callee() {
    let (_tmp, repo) = setup_repo();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();

    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert = cmd
        .current_dir(&repo)
        .arg("impact")
        .arg("--with-propagation")
        .arg("--format")
        .arg("dot")
        .write_stdin(diff)
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref());
    assert!(
        stdout.contains("\"f.rs:use:x:4\" -> \"f.rs:def:y:4\""),
        "expected summary bridge from callsite arg use into assigned def, got:\n{}",
        stdout
    );
}

#[test]
fn pdg_propagation_does_not_leak_irrelevant_two_arg_bridge() {
    let (_tmp, repo) = setup_two_arg_repo();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();

    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert = cmd
        .current_dir(&repo)
        .arg("impact")
        .arg("--with-propagation")
        .arg("--format")
        .arg("dot")
        .write_stdin(diff)
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref());
    assert!(
        stdout.contains("\"f.rs:use:y:5\" -> \"f.rs:def:out:5\""),
        "expected later arg to keep its summary bridge, got:\n{}",
        stdout
    );
    assert!(
        !stdout.contains("\"f.rs:use:x:5\" -> \"f.rs:def:out:5\""),
        "unexpected irrelevant arg bridge leaked into output, got:\n{}",
        stdout
    );
}
