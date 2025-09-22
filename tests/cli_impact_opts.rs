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

fn setup_repo_triple() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    let src = r#"fn c() {}
fn b() { c(); }
fn a() { b(); }
"#;
    fs::write(path.join("main.rs"), src).unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    // modify b body
    let src2 = r#"fn c() {}
fn b() { let _k = 1; c(); }
fn a() { b(); }
"#;
    fs::write(path.join("main.rs"), src2).unwrap();
    (dir, path)
}

#[test]
fn cli_impact_direction_callees() {
    let (_tmp, repo) = setup_repo_triple();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();

    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    cmd.current_dir(&repo)
        .arg("--mode")
        .arg("impact")
        .arg("--direction")
        .arg("callees")
        .arg("--lang")
        .arg("rust")
        .arg("--format")
        .arg("json")
        .write_stdin(diff.clone());
    let assert = cmd.assert().success();
    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref());
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let impacted = v["impacted_symbols"].as_array().unwrap();
    let names: Vec<&str> = impacted
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"c"));
}

#[test]
fn cli_impact_max_depth_limits() {
    // change b, callees depth 0 should not include c
    let (_tmp, repo) = setup_repo_triple();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();

    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    cmd.current_dir(&repo)
        .arg("--mode")
        .arg("impact")
        .arg("--direction")
        .arg("callees")
        .arg("--max-depth")
        .arg("0")
        .arg("--lang")
        .arg("rust")
        .arg("--format")
        .arg("json")
        .write_stdin(diff);
    let assert = cmd.assert().success();
    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref());
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let impacted = v["impacted_symbols"].as_array().unwrap();
    let names: Vec<&str> = impacted
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert!(!names.contains(&"c"));
}
