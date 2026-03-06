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

fn setup_repo_java_hard_cases() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    fs::create_dir_all(path.join("demo")).unwrap();

    let ops = r#"package demo;

public class Ops {
    public static int pick(int v) {
        return v;
    }

    public static String pick(String v) {
        return v;
    }
}
"#;
    let outer = r#"package demo;

public class Outer {
    public static class Inner {
        public static int compute() {
            return 1;
        }
    }
}
"#;
    let main = r#"package demo;

import static demo.Ops.pick;

public class Main {
    static int run() {
        return pick(1) + Outer.Inner.compute();
    }

    static int entry() {
        return run();
    }
}
"#;

    fs::write(path.join("demo/Ops.java"), ops).unwrap();
    fs::write(path.join("demo/Outer.java"), outer).unwrap();
    fs::write(path.join("demo/Main.java"), main).unwrap();

    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    let ops2 = r#"package demo;

public class Ops {
    public static int pick(int v) {
        int x = v + 1;
        return x;
    }

    public static String pick(String v) {
        return v;
    }
}
"#;
    fs::write(path.join("demo/Ops.java"), ops2).unwrap();

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

#[test]
fn cli_mode_changed_reports_java_hard_case_symbol() {
    let (_tmp, repo) = setup_repo_java_hard_cases();
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
        .stdout(predicate::str::contains("\"pick\""))
        .stdout(predicate::str::contains("demo/Ops.java"));

    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref());
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let changed = v["changed_symbols"].as_array().unwrap();
    assert!(changed.iter().any(|s| s["name"].as_str() == Some("pick")));
}

#[test]
fn cli_impact_direction_callers_java_hard_cases() {
    let (_tmp, repo) = setup_repo_java_hard_cases();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();

    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    cmd.current_dir(&repo)
        .arg("--mode")
        .arg("impact")
        .arg("--direction")
        .arg("callers")
        .arg("--lang")
        .arg("java")
        .arg("--format")
        .arg("json")
        .write_stdin(diff);
    let assert = cmd.assert().success();

    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref());
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let changed = v["changed_symbols"].as_array().unwrap();
    assert!(changed.iter().any(|s| s["name"].as_str() == Some("pick")));

    let impacted = v["impacted_symbols"].as_array().unwrap();
    let names: Vec<&str> = impacted.iter().filter_map(|s| s["name"].as_str()).collect();
    assert!(names.contains(&"run"));
}
