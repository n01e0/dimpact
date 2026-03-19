#![allow(deprecated)]
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

fn setup_repo_module_groups() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let repo = dir.path().to_path_buf();

    fs::create_dir_all(repo.join("alpha")).unwrap();
    fs::create_dir_all(repo.join("beta")).unwrap();

    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "tester@example.com"]);
    git(&repo, &["config", "user.name", "Tester"]);

    fs::write(
        repo.join("main.rs"),
        r#"mod alpha;
mod beta;
mod leaf;

fn root_one() {
    crate::leaf::leaf();
}

fn main() {
    crate::alpha::first::alpha_one();
    crate::alpha::second::alpha_two();
    crate::beta::first::beta_one();
    root_one();
}
"#,
    )
    .unwrap();
    fs::write(
        repo.join("alpha/mod.rs"),
        "pub mod first;\npub mod second;\n",
    )
    .unwrap();
    fs::write(
        repo.join("alpha/first.rs"),
        "pub fn alpha_one() { crate::leaf::leaf(); }\n",
    )
    .unwrap();
    fs::write(
        repo.join("alpha/second.rs"),
        "pub fn alpha_two() { crate::leaf::leaf(); }\n",
    )
    .unwrap();
    fs::write(repo.join("beta/mod.rs"), "pub mod first;\n").unwrap();
    fs::write(
        repo.join("beta/first.rs"),
        "pub fn beta_one() { crate::leaf::leaf(); }\n",
    )
    .unwrap();
    fs::write(repo.join("leaf.rs"), "pub fn leaf() {}\n").unwrap();

    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "init", "-q"]);

    fs::write(repo.join("leaf.rs"), "pub fn leaf() { let _x = 1; }\n").unwrap();

    (dir, repo)
}

fn diff_text(repo: &std::path::Path) -> String {
    let diff_out = git(repo, &["diff", "--no-ext-diff", "--unified=0"]);
    String::from_utf8(diff_out.stdout).unwrap()
}

fn run_impact_json(repo: &std::path::Path, diff: &str, extra_args: &[&str]) -> serde_json::Value {
    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    cmd.current_dir(repo)
        .arg("--mode")
        .arg("impact")
        .arg("--direction")
        .arg("callers")
        .arg("--lang")
        .arg("rust")
        .arg("--format")
        .arg("json");
    for arg in extra_args {
        cmd.arg(arg);
    }
    let assert = cmd.write_stdin(diff.to_owned()).assert().success();
    serde_json::from_slice(assert.get_output().stdout.as_ref()).unwrap()
}

fn run_impact_yaml(repo: &std::path::Path, diff: &str) -> serde_yaml::Value {
    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert = cmd
        .current_dir(repo)
        .arg("--mode")
        .arg("impact")
        .arg("--direction")
        .arg("callers")
        .arg("--lang")
        .arg("rust")
        .arg("--format")
        .arg("yaml")
        .write_stdin(diff.to_owned())
        .assert()
        .success();
    serde_yaml::from_slice(assert.get_output().stdout.as_ref()).unwrap()
}

fn affected_modules_json(output: &serde_json::Value) -> Vec<(String, u64, u64)> {
    output["summary"]["affected_modules"]
        .as_array()
        .expect("summary.affected_modules array")
        .iter()
        .map(|module| {
            (
                module["module"].as_str().expect("module").to_string(),
                module["symbol_count"].as_u64().expect("symbol_count"),
                module["file_count"].as_u64().expect("file_count"),
            )
        })
        .collect()
}

fn affected_modules_yaml(output: &serde_yaml::Value) -> Vec<(String, u64, u64)> {
    let summary = output
        .get("summary")
        .and_then(|value| value.as_mapping())
        .expect("summary mapping");
    let modules = summary
        .get(serde_yaml::Value::from("affected_modules"))
        .and_then(|value| value.as_sequence())
        .expect("affected_modules sequence");

    modules
        .iter()
        .map(|module| {
            let module = module.as_mapping().expect("affected module mapping");
            (
                module
                    .get(serde_yaml::Value::from("module"))
                    .and_then(|value| value.as_str())
                    .expect("module")
                    .to_string(),
                module
                    .get(serde_yaml::Value::from("symbol_count"))
                    .and_then(|value| value.as_u64())
                    .expect("symbol_count"),
                module
                    .get(serde_yaml::Value::from("file_count"))
                    .and_then(|value| value.as_u64())
                    .expect("file_count"),
            )
        })
        .collect()
}

#[test]
fn cli_impact_json_includes_affected_modules_summary() {
    let (_tmp, repo) = setup_repo_module_groups();
    let diff = diff_text(&repo);

    let output = run_impact_json(&repo, &diff, &[]);

    assert_eq!(
        affected_modules_json(&output),
        vec![
            ("alpha".to_string(), 2, 2),
            ("(root)".to_string(), 2, 1),
            ("beta".to_string(), 1, 1),
        ]
    );
}

#[test]
fn cli_impact_per_seed_nests_affected_modules_under_output_summary() {
    let (_tmp, repo) = setup_repo_module_groups();
    let diff = diff_text(&repo);

    let output = run_impact_json(&repo, &diff, &["--per-seed"]);

    let grouped = output.as_array().expect("per-seed top-level array");
    assert_eq!(grouped.len(), 1);
    assert_eq!(grouped[0]["changed_symbol"]["name"].as_str(), Some("leaf"));
    assert_eq!(
        affected_modules_json(&grouped[0]["impacts"][0]["output"]),
        vec![
            ("alpha".to_string(), 2, 2),
            ("(root)".to_string(), 2, 1),
            ("beta".to_string(), 1, 1),
        ]
    );
}

#[test]
fn cli_impact_yaml_includes_affected_modules_summary() {
    let (_tmp, repo) = setup_repo_module_groups();
    let diff = diff_text(&repo);

    let output = run_impact_yaml(&repo, &diff);

    assert_eq!(
        affected_modules_yaml(&output),
        vec![
            ("alpha".to_string(), 2, 2),
            ("(root)".to_string(), 2, 1),
            ("beta".to_string(), 1, 1),
        ]
    );
}
