use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn git(cwd: &Path, args: &[&str]) -> std::process::Output {
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

fn setup_repo_from_python_fixture() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/python/non_lsp");
    let before = fs::read_to_string(fixture_root.join("main_before.py")).expect("read before");
    let after = fs::read_to_string(fixture_root.join("main_after.py")).expect("read after");
    let pyproject =
        fs::read_to_string(fixture_root.join("pyproject.toml")).expect("read pyproject");

    fs::write(path.join("main.py"), before).unwrap();
    fs::write(path.join("pyproject.toml"), pyproject).unwrap();

    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    fs::write(path.join("main.py"), after).unwrap();

    (dir, path)
}

#[test]
fn cli_changed_python_fixture_reports_bar() {
    let (_tmp, repo) = setup_repo_from_python_fixture();
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
        .success();

    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref());
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let changed = v["changed_symbols"].as_array().unwrap();
    assert!(changed.iter().any(|s| s["name"].as_str() == Some("bar")));
}

#[test]
fn cli_impact_python_fixture_reports_callers_chain() {
    let (_tmp, repo) = setup_repo_from_python_fixture();
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
        .success();

    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref());
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let changed = v["changed_symbols"].as_array().unwrap();
    assert!(changed.iter().any(|s| s["name"].as_str() == Some("bar")));

    let impacted = v["impacted_symbols"].as_array().unwrap();
    let names: Vec<&str> = impacted.iter().filter_map(|s| s["name"].as_str()).collect();
    assert!(names.contains(&"foo"));
}
