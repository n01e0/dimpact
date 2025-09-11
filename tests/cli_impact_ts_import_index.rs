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
fn ts_import_index_callers() {
    let dir = TempDir::new().unwrap();
    let repo = dir.path().to_path_buf();
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "tester@example.com"]);
    git(&repo, &["config", "user.name", "Tester"]);
    std::fs::create_dir_all(repo.join("mod")).unwrap();
    fs::write(repo.join("mod/index.ts"), "export function bar(): void {}\n").unwrap();
    fs::write(repo.join("main.ts"), "import * as A from './mod'\nfunction foo(){ A.bar(); }\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "init", "-q"]);
    fs::write(repo.join("mod/index.ts"), "export function bar(): void { let x=1; }\n").unwrap();
    let diff = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert = cmd.current_dir(&repo)
        .arg("--mode").arg("impact")
        .arg("--direction").arg("callers")
        .arg("--lang").arg("auto")
        .arg("--format").arg("json")
        .write_stdin(String::from_utf8(diff.stdout).unwrap())
        .assert().success();
    let out = String::from_utf8_lossy(assert.get_output().stdout.as_ref());
    assert!(out.contains("\"foo\""), "impact should include foo, got: {}", out);
}
