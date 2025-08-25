use assert_cmd::prelude::*;
use assert_cmd::cargo::CommandCargoExt;
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

    // rust file
    let src = r#"fn foo() {
    println!("one");
}

fn bar() {}
"#;
    fs::write(path.join("main.rs"), src).unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    // modify inside foo
    let src2 = r#"fn foo() {
    println!("one");
    println!("two");
}

fn bar() {}
"#;
    fs::write(path.join("main.rs"), src2).unwrap();

    (dir, path)
}

#[test]
fn cli_mode_changed_reports_rust_symbol() {
    let (_tmp, repo) = setup_repo();
    let diff_out = git(&repo, &["diff", "--no-ext-diff"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();

    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert = cmd
        .current_dir(&repo)
        .arg("--mode").arg("changed")
        .arg("--lang").arg("rust")
        .arg("--format").arg("json")
        .write_stdin(diff)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"changed_symbols\""))
        .stdout(predicate::str::contains("\"foo\""))
        .stdout(predicate::str::contains("main.rs"));

    // parse json to ensure structure
    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref());
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(v["changed_symbols"].is_array());
}

