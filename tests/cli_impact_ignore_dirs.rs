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
fn ignore_dir_drops_seeds_in_ignored_path() {
    // repo layout:
    //   src/main.js (calls g)
    //   dist/generated.js (defines g) ← changed file (ignored)
    let dir = TempDir::new().unwrap();
    let repo = dir.path().to_path_buf();
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "tester@example.com"]);
    git(&repo, &["config", "user.name", "Tester"]);
    fs::create_dir_all(repo.join("src")).unwrap();
    fs::create_dir_all(repo.join("dist")).unwrap();
    fs::write(repo.join("src/main.js"), "function main() { g(); }\n").unwrap();
    fs::write(repo.join("dist/generated.js"), "function g() { }\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "init", "-q"]);
    // change dist/generated.js
    fs::write(
        repo.join("dist/generated.js"),
        "function g() { let k = 1; }\n",
    )
    .unwrap();
    let diff = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);

    // Without ignore: expect main to be impacted (callers of g)
    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert = cmd
        .current_dir(&repo)
        .arg("--mode")
        .arg("impact")
        .arg("--direction")
        .arg("callers")
        .arg("--lang")
        .arg("auto")
        .arg("--format")
        .arg("json")
        .write_stdin(String::from_utf8(diff.stdout.clone()).unwrap())
        .assert()
        .success();
    let out = String::from_utf8_lossy(assert.get_output().stdout.as_ref());
    assert!(
        out.contains("\"main\""),
        "impact should include main without ignore, got: {}",
        out
    );

    // With ignore: dist should be ignored as seed → no impact
    let mut cmd2 = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert2 = cmd2
        .current_dir(&repo)
        .arg("--mode")
        .arg("impact")
        .arg("--direction")
        .arg("callers")
        .arg("--lang")
        .arg("auto")
        .arg("--format")
        .arg("json")
        .arg("--ignore-dir")
        .arg("dist")
        .write_stdin(String::from_utf8(diff.stdout).unwrap())
        .assert()
        .success();
    let out2 = String::from_utf8_lossy(assert2.get_output().stdout.as_ref());
    let v: serde_json::Value = serde_json::from_str(&out2).unwrap();
    let impacted = v["impacted_symbols"].as_array().unwrap();
    assert!(
        impacted.is_empty(),
        "impacted should be empty when seed is ignored, got: {}",
        out2
    );
}

#[test]
fn ignore_dir_filters_impacted_in_ignored_path() {
    // repo layout:
    //   src/main.js (calls g) ← changed file
    //   dist/generated.js (defines g) ← should be filtered from impact when ignored
    let dir = TempDir::new().unwrap();
    let repo = dir.path().to_path_buf();
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "tester@example.com"]);
    git(&repo, &["config", "user.name", "Tester"]);
    fs::create_dir_all(repo.join("src")).unwrap();
    fs::create_dir_all(repo.join("dist")).unwrap();
    fs::write(repo.join("src/main.js"), "function main() { g(); }\n").unwrap();
    fs::write(repo.join("dist/generated.js"), "function g() { }\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "init", "-q"]);
    // change src/main.js
    fs::write(
        repo.join("src/main.js"),
        "function main() { let x = 1; g(); }\n",
    )
    .unwrap();
    let diff = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);

    // Without ignore: expect g to be impacted (callees of main)
    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert = cmd
        .current_dir(&repo)
        .arg("--mode")
        .arg("impact")
        .arg("--direction")
        .arg("callees")
        .arg("--lang")
        .arg("auto")
        .arg("--format")
        .arg("json")
        .write_stdin(String::from_utf8(diff.stdout.clone()).unwrap())
        .assert()
        .success();
    let out = String::from_utf8_lossy(assert.get_output().stdout.as_ref());
    assert!(
        out.contains("\"g\""),
        "impact should include g without ignore, got: {}",
        out
    );

    // With ignore: dist should be filtered from impacted
    let mut cmd2 = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert2 = cmd2
        .current_dir(&repo)
        .arg("--mode")
        .arg("impact")
        .arg("--direction")
        .arg("callees")
        .arg("--lang")
        .arg("auto")
        .arg("--format")
        .arg("json")
        .arg("--ignore-dir")
        .arg("dist")
        .write_stdin(String::from_utf8(diff.stdout).unwrap())
        .assert()
        .success();
    let out2 = String::from_utf8_lossy(assert2.get_output().stdout.as_ref());
    assert!(
        !out2.contains("\"g\""),
        "impact should NOT include g when dist is ignored, got: {}",
        out2
    );
}
