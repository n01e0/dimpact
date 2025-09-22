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
fn ruby_require_relative_nested_dirs_callers_impact() {
    let dir = TempDir::new().unwrap();
    let repo = dir.path().to_path_buf();
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "tester@example.com"]);
    git(&repo, &["config", "user.name", "Tester"]);
    fs::create_dir_all(repo.join("lib")).unwrap();
    fs::create_dir_all(repo.join("app")).unwrap();
    // lib/a.rb defines m
    fs::write(repo.join("lib/a.rb"), "def m; end\n").unwrap();
    // app/b.rb requires ../lib/a and calls m
    fs::write(
        repo.join("app/b.rb"),
        "require_relative '../lib/a'\n\ndef foo\n  m\nend\n",
    )
    .unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "init", "-q"]);
    // change m in lib/a.rb
    fs::write(repo.join("lib/a.rb"), "def m; x=1; end\n").unwrap();
    let diff = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    // impact callers should include foo in app/b.rb
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
        .write_stdin(String::from_utf8(diff.stdout).unwrap())
        .assert()
        .success();
    let out = String::from_utf8_lossy(assert.get_output().stdout.as_ref());
    assert!(
        out.contains("\"foo\""),
        "impact should include foo, got: {}",
        out
    );
}
