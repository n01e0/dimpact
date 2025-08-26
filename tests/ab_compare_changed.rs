#![cfg(feature = "ts")]
use assert_cmd::cargo::CommandCargoExt;
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

#[test]
fn ab_compare_changed_smoke() {
    let dir = TempDir::new().unwrap();
    let repo = dir.path().to_path_buf();
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "tester@example.com"]);
    git(&repo, &["config", "user.name", "Tester"]);
    fs::write(repo.join("main.rs"), "fn foo() {}\nfn bar() {}\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "init", "-q"]);
    fs::write(repo.join("main.rs"), "fn foo() { let _x=1; }\nfn bar() {}\n").unwrap();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();

    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    cmd.current_dir(&repo)
        .arg("--mode").arg("changed")
        .arg("--lang").arg("rust")
        .arg("--ab-compare")
        .arg("--format").arg("json")
        .write_stdin(diff);
    let assert = cmd.assert().success();
    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref());
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(v["only_in_regex"].is_array());
    assert!(v["only_in_ts"].is_array());
}

