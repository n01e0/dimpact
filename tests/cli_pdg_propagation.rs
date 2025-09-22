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

    let src = r#"fn callee(a: i32) -> i32 { a + 1 }
fn caller() {
    let x = 1;
    let y = callee(x);
    println!("{}", y);
}
"#;
    fs::write(path.join("f.rs"), src).unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    // change x assignment to force a diff in caller
    let src2 = r#"fn callee(a: i32) -> i32 { a + 1 }
fn caller() {
    let x = 2;
    let y = callee(x);
    println!("{}", y);
}
"#;
    fs::write(path.join("f.rs"), src2).unwrap();

    (dir, path)
}

#[test]
fn pdg_propagation_adds_var_to_callee_edge() {
    let (_tmp, repo) = setup_repo();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();

    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert = cmd
        .current_dir(&repo)
        .arg("impact")
        .arg("--with-pdg")
        .arg("--with-propagation")
        .arg("--format")
        .arg("dot")
        .write_stdin(diff)
        .assert()
        .success()
        .stdout(predicate::str::contains("rust:f.rs:fn:callee:1"));

    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref());
    // Roughly ensure there's an edge into the callee symbol ID
    assert!(stdout.contains("\"rust:f.rs:fn:callee:1\""));
}
