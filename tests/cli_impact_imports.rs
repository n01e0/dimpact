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

fn setup_repo_import_case() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    // two files: utils.rs with c(), main.rs using imported c()
    fs::write(path.join("utils.rs"), "pub fn c() {}\n").unwrap();
    fs::write(path.join("main.rs"), "use crate::utils::c;\nfn b() { c(); }\nfn a() {}\n").unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    // modify b()
    fs::write(path.join("main.rs"), "use crate::utils::c;\nfn b() { let _k=1; c(); }\nfn a() {}\n").unwrap();
    (dir, path)
}

#[test]
fn impact_resolves_imported_function() {
    let (_tmp, repo) = setup_repo_import_case();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();
    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert = cmd
        .current_dir(&repo)
        .arg("--mode").arg("impact")
        .arg("--direction").arg("callees")
        .arg("--lang").arg("rust")
        .arg("--format").arg("json")
        .write_stdin(diff)
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref());
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let impacted = v["impacted_symbols"].as_array().unwrap();
    let names: Vec<&str> = impacted.iter().map(|s| s["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"c"));
}

#[test]
fn impact_resolves_qualified_call() {
    let dir = TempDir::new().unwrap();
    let repo = dir.path().to_path_buf();
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "tester@example.com"]);
    git(&repo, &["config", "user.name", "Tester"]);
    fs::write(repo.join("utils.rs"), "pub fn c() {}\n").unwrap();
    fs::write(repo.join("main.rs"), "fn b() { crate::utils::c(); }\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "init", "-q"]);
    fs::write(repo.join("main.rs"), "fn b() { let _x=1; crate::utils::c(); }\n").unwrap();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();
    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    cmd.current_dir(&repo)
        .arg("--mode").arg("impact")
        .arg("--direction").arg("callees")
        .arg("--lang").arg("rust")
        .arg("--format").arg("json")
        .write_stdin(diff);
    let assert = cmd.assert().success();
    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref());
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let names: Vec<&str> = v["impacted_symbols"].as_array().unwrap().iter().map(|s| s["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"c"));
}

