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
fn resolves_module_in_mod_rs() {
    let dir = TempDir::new().unwrap();
    let repo = dir.path().to_path_buf();
    fs::create_dir_all(repo.join("a/b")).unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "tester@example.com"]);
    git(&repo, &["config", "user.name", "Tester"]);
    fs::write(repo.join("a/b/mod.rs"), "pub fn c() {}\n").unwrap();
    fs::write(repo.join("a.rs"), "fn x(){ crate::a::b::c(); }\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "init", "-q"]);
    fs::write(repo.join("a.rs"), "fn x(){ let _k=1; crate::a::b::c(); }\n").unwrap();
    let diff = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    cmd.current_dir(&repo)
        .arg("--mode").arg("impact")
        .arg("--direction").arg("callees")
        .arg("--lang").arg("rust")
        .arg("--format").arg("json")
        .write_stdin(String::from_utf8(diff.stdout).unwrap());
    let assert = cmd.assert().success();
    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref());
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let names: Vec<&str> = v["impacted_symbols"].as_array().unwrap().iter().map(|s| s["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"c"));
}
