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

#[test]
fn cli_engine_dump_capabilities_uses_mock() {
    let dir = TempDir::new().unwrap();
    let repo = dir.path().to_path_buf();
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "tester@example.com"]);
    git(&repo, &["config", "user.name", "Tester"]);
    fs::write(repo.join("main.rs"), "fn bar() {}\nfn foo() { bar(); }\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "init", "-q"]);
    fs::write(repo.join("main.rs"), "fn bar() { let _x=1; }\nfn foo() { bar(); }\n").unwrap();
    let diff = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);

    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert = cmd
        .current_dir(&repo)
        .env("DIMPACT_TEST_LSP_MOCK", "1")
        .arg("--mode").arg("impact")
        .arg("--engine").arg("auto")
        .arg("--engine-dump-capabilities")
        .arg("--lang").arg("rust")
        .arg("--format").arg("json")
        .write_stdin(String::from_utf8(diff.stdout).unwrap())
        .assert()
        .success()
        .stderr(predicate::str::contains("document_symbol"))
        .stderr(predicate::str::contains("call_hierarchy"));

    // also ensure normal JSON output to stdout
    let _ = String::from_utf8_lossy(assert.get_output().stdout.as_ref());
}
