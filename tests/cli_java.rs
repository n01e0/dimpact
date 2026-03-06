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

fn setup_repo_java_triple() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    let src = r#"class Main {
    static void c() {}

    static void b() {
        c();
    }

    static void a() {
        b();
    }
}
"#;
    fs::write(path.join("Main.java"), src).unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    let src2 = r#"class Main {
    static void c() {}

    static void b() {
        int x = 1;
        c();
    }

    static void a() {
        b();
    }
}
"#;
    fs::write(path.join("Main.java"), src2).unwrap();

    (dir, path)
}

#[test]
fn cli_mode_changed_reports_java_symbol() {
    let (_tmp, repo) = setup_repo_java_triple();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();

    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert = cmd
        .current_dir(&repo)
        .arg("--mode")
        .arg("changed")
        .arg("--lang")
        .arg("java")
        .arg("--format")
        .arg("json")
        .write_stdin(diff)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"changed_symbols\""))
        .stdout(predicate::str::contains("\"b\""))
        .stdout(predicate::str::contains("Main.java"));

    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref());
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let changed = v["changed_symbols"].as_array().unwrap();
    assert!(changed.iter().any(|s| s["name"].as_str() == Some("b")));
}

#[test]
fn cli_impact_direction_callees_java() {
    let (_tmp, repo) = setup_repo_java_triple();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();

    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    cmd.current_dir(&repo)
        .arg("--mode")
        .arg("impact")
        .arg("--direction")
        .arg("callees")
        .arg("--lang")
        .arg("java")
        .arg("--format")
        .arg("json")
        .write_stdin(diff);
    let assert = cmd.assert().success();

    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref());
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let changed = v["changed_symbols"].as_array().unwrap();
    assert!(changed.iter().any(|s| s["name"].as_str() == Some("b")));

    let impacted = v["impacted_symbols"].as_array().unwrap();
    let names: Vec<&str> = impacted.iter().filter_map(|s| s["name"].as_str()).collect();
    assert!(names.contains(&"c"));
}
