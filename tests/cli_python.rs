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

fn setup_repo_python_call_chain() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    let src = r#"def bar():
    return 1

def foo():
    return bar()

def main():
    return foo()
"#;
    fs::write(path.join("main.py"), src).unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    let src2 = r#"def bar():
    x = 1
    return x

def foo():
    return bar()

def main():
    return foo()
"#;
    fs::write(path.join("main.py"), src2).unwrap();

    (dir, path)
}

#[test]
fn cli_changed_lang_python_uses_python_analyzer() {
    let (_tmp, repo) = setup_repo_python_call_chain();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();

    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert = cmd
        .current_dir(&repo)
        .arg("--mode")
        .arg("changed")
        .arg("--lang")
        .arg("python")
        .arg("--format")
        .arg("json")
        .write_stdin(diff)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"changed_symbols\""))
        .stdout(predicate::str::contains("\"bar\""))
        .stdout(predicate::str::contains("main.py"));

    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref());
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let changed = v["changed_symbols"].as_array().unwrap();
    assert!(changed.iter().any(|s| s["name"].as_str() == Some("bar")));
}

#[test]
fn cli_impact_lang_python_engine_ts_uses_non_lsp_analyzer() {
    let (_tmp, repo) = setup_repo_python_call_chain();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();

    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert = cmd
        .current_dir(&repo)
        .arg("--mode")
        .arg("impact")
        .arg("--engine")
        .arg("ts")
        .arg("--direction")
        .arg("callers")
        .arg("--lang")
        .arg("python")
        .arg("--format")
        .arg("json")
        .write_stdin(diff)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"changed_symbols\""))
        .stdout(predicate::str::contains("\"impacted_symbols\""));

    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref());
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let changed = v["changed_symbols"].as_array().unwrap();
    assert!(changed.iter().any(|s| s["name"].as_str() == Some("bar")));

    let impacted = v["impacted_symbols"].as_array().unwrap();
    let names: Vec<&str> = impacted.iter().filter_map(|s| s["name"].as_str()).collect();
    assert!(names.contains(&"foo"));
}
