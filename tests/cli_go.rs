#![allow(deprecated)]
mod json_output;

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

fn setup_repo_go_triple() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    let src = r#"package main

func c() {}

func b() {
    c()
}

func a() {
    b()
}
"#;
    fs::write(path.join("main.go"), src).unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    let src2 = r#"package main

func c() {}

func b() {
    x := 1
    _ = x
    c()
}

func a() {
    b()
}
"#;
    fs::write(path.join("main.go"), src2).unwrap();

    (dir, path)
}

#[test]
fn cli_mode_changed_reports_go_symbol() {
    let (_tmp, repo) = setup_repo_go_triple();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();

    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert = cmd
        .current_dir(&repo)
        .arg("--mode")
        .arg("changed")
        .arg("--lang")
        .arg("go")
        .arg("--format")
        .arg("json")
        .write_stdin(diff)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"changed_symbols\""))
        .stdout(predicate::str::contains("\"b\""))
        .stdout(predicate::str::contains("main.go"));

    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref());
    let v = json_output::parse_payload(&stdout);
    let changed = v["changed_symbols"].as_array().unwrap();
    assert!(changed.iter().any(|s| s["name"].as_str() == Some("b")));
}

#[test]
fn cli_impact_direction_callees_go() {
    let (_tmp, repo) = setup_repo_go_triple();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();

    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    cmd.current_dir(&repo)
        .arg("--mode")
        .arg("impact")
        .arg("--direction")
        .arg("callees")
        .arg("--lang")
        .arg("go")
        .arg("--format")
        .arg("json")
        .write_stdin(diff);
    let assert = cmd.assert().success();

    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref());
    let v = json_output::parse_payload(&stdout);
    let changed = v["changed_symbols"].as_array().unwrap();
    assert!(changed.iter().any(|s| s["name"].as_str() == Some("b")));

    let impacted = v["impacted_symbols"].as_array().unwrap();
    let names: Vec<&str> = impacted.iter().filter_map(|s| s["name"].as_str()).collect();
    assert!(names.contains(&"c"));
}
