use predicates::prelude::*;
use std::fs;
use std::io::Write;
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

    // create initial file and commit
    fs::write(path.join("a.txt"), b"hello\nworld\n").unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    // modify the file: change world -> WORLD and add a new line
    let mut f = fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(path.join("a.txt"))
        .unwrap();
    write!(f, "hello\nWORLD\nNEW\n").unwrap();

    (dir, path)
}

#[test]
fn e2e_git_diff_into_dimpact_json() {
    let (_dir, repo) = setup_repo();

    // produce diff from repo
    let out = git(&repo, &["diff", "--no-ext-diff"]);
    let diff = String::from_utf8(out.stdout).unwrap();
    assert!(!diff.is_empty(), "diff should not be empty");

    // run dimpact and feed diff via stdin
    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert = cmd
        .arg("--format")
        .arg("json")
        .write_stdin(diff)
        .assert()
        .success()
        .stdout(predicate::str::contains("a.txt"));

    // additionally, parse JSON to ensure schema is valid
    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref()).to_string();
    let files: Vec<dimpact::FileChanges> =
        serde_json::from_str(&stdout).expect("valid json schema");
    assert_eq!(files.len(), 1);
    let f = &files[0];
    assert_eq!(f.old_path.as_deref(), Some("a.txt"));
    assert_eq!(f.new_path.as_deref(), Some("a.txt"));
    assert!(
        f.changes
            .iter()
            .any(|c| matches!(c.kind, dimpact::ChangeKind::Removed))
    );
    assert!(
        f.changes
            .iter()
            .any(|c| matches!(c.kind, dimpact::ChangeKind::Added))
    );
}

#[test]
fn e2e_git_diff_into_dimpact_yaml() {
    let (_dir, repo) = setup_repo();
    let out = git(&repo, &["diff", "--no-ext-diff"]);
    let diff = String::from_utf8(out.stdout).unwrap();

    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    cmd.arg("--format").arg("yaml").write_stdin(diff);

    let assert = cmd.assert().success();
    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref());
    assert!(stdout.contains("- old_path:"));
}
