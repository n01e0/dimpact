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

fn setup_repo_from_fixture(before: &str, after: &str) -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    fs::write(path.join("main.rs"), before).unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    fs::write(path.join("main.rs"), after).unwrap();
    (dir, path)
}

fn fixture_repo() -> (TempDir, std::path::PathBuf) {
    let before = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/rust/analyzer_hard_cases_confidence_compare.rs"
    ));
    let after = before.replacen("x + 1", "x + 2", 1);
    setup_repo_from_fixture(before, &after)
}

fn direct_plus_three_transitive_repo() -> (TempDir, std::path::PathBuf) {
    let before = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/rust/impact_summary_risk_direct_plus_three_transitive.rs"
    ));
    let after = before.replacen("let _ = base + 1;", "let _ = base + 2;", 1);
    setup_repo_from_fixture(before, &after)
}

fn diff_text(repo: &std::path::Path) -> String {
    let diff_out = git(repo, &["diff", "--no-ext-diff", "--unified=0"]);
    String::from_utf8(diff_out.stdout).unwrap()
}

#[test]
fn cli_impact_risk_json_reports_fixture_summary() {
    let (_tmp, repo) = fixture_repo();
    let diff = diff_text(&repo);

    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert = cmd
        .current_dir(&repo)
        .arg("--mode")
        .arg("impact")
        .arg("--direction")
        .arg("callers")
        .arg("--with-edges")
        .arg("--min-confidence")
        .arg("inferred")
        .arg("--lang")
        .arg("rust")
        .arg("--format")
        .arg("json")
        .write_stdin(diff)
        .assert()
        .success();

    let v: serde_json::Value = serde_json::from_slice(assert.get_output().stdout.as_ref()).unwrap();
    let risk = &v["summary"]["risk"];
    assert_eq!(risk["level"], "medium");
    assert_eq!(risk["direct_hits"].as_u64(), Some(1));
    assert_eq!(risk["transitive_hits"].as_u64(), Some(1));
    assert_eq!(risk["impacted_files"].as_u64(), Some(1));
    assert_eq!(risk["impacted_symbols"].as_u64(), Some(2));
}

#[test]
fn cli_impact_risk_yaml_reports_fixture_summary() {
    let (_tmp, repo) = fixture_repo();
    let diff = diff_text(&repo);

    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert = cmd
        .current_dir(&repo)
        .arg("--mode")
        .arg("impact")
        .arg("--direction")
        .arg("callers")
        .arg("--with-edges")
        .arg("--min-confidence")
        .arg("inferred")
        .arg("--lang")
        .arg("rust")
        .arg("--format")
        .arg("yaml")
        .write_stdin(diff)
        .assert()
        .success();

    let v: serde_yaml::Value = serde_yaml::from_slice(assert.get_output().stdout.as_ref()).unwrap();
    let summary = v
        .get("summary")
        .and_then(|value| value.as_mapping())
        .expect("summary mapping");
    let risk = summary
        .get(serde_yaml::Value::from("risk"))
        .and_then(|value| value.as_mapping())
        .expect("risk mapping");

    let level_key = serde_yaml::Value::from("level");
    let direct_key = serde_yaml::Value::from("direct_hits");
    let transitive_key = serde_yaml::Value::from("transitive_hits");
    let files_key = serde_yaml::Value::from("impacted_files");
    let symbols_key = serde_yaml::Value::from("impacted_symbols");

    assert_eq!(
        risk.get(&level_key).and_then(|value| value.as_str()),
        Some("medium")
    );
    assert_eq!(
        risk.get(&direct_key).and_then(|value| value.as_u64()),
        Some(1)
    );
    assert_eq!(
        risk.get(&transitive_key).and_then(|value| value.as_u64()),
        Some(1)
    );
    assert_eq!(
        risk.get(&files_key).and_then(|value| value.as_u64()),
        Some(1)
    );
    assert_eq!(
        risk.get(&symbols_key).and_then(|value| value.as_u64()),
        Some(2)
    );
}

#[test]
fn cli_impact_risk_json_promotes_direct_plus_three_transitive_to_high() {
    let (_tmp, repo) = direct_plus_three_transitive_repo();
    let diff = diff_text(&repo);

    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert = cmd
        .current_dir(&repo)
        .arg("--mode")
        .arg("impact")
        .arg("--direction")
        .arg("callers")
        .arg("--with-edges")
        .arg("--min-confidence")
        .arg("inferred")
        .arg("--lang")
        .arg("rust")
        .arg("--format")
        .arg("json")
        .write_stdin(diff)
        .assert()
        .success();

    let v: serde_json::Value = serde_json::from_slice(assert.get_output().stdout.as_ref()).unwrap();
    let risk = &v["summary"]["risk"];
    assert_eq!(risk["level"], "high");
    assert_eq!(risk["direct_hits"].as_u64(), Some(1));
    assert_eq!(risk["transitive_hits"].as_u64(), Some(3));
    assert_eq!(risk["impacted_files"].as_u64(), Some(1));
    assert_eq!(risk["impacted_symbols"].as_u64(), Some(4));

    let by_depth = v["summary"]["by_depth"].as_array().expect("by_depth array");
    let buckets: Vec<(u64, u64)> = by_depth
        .iter()
        .map(|bucket| {
            (
                bucket["depth"].as_u64().expect("depth"),
                bucket["symbol_count"].as_u64().expect("symbol_count"),
            )
        })
        .collect();
    assert_eq!(buckets, vec![(1, 1), (2, 1), (3, 1), (4, 1)]);
}

#[test]
fn cli_impact_risk_yaml_promotes_direct_plus_three_transitive_to_high() {
    let (_tmp, repo) = direct_plus_three_transitive_repo();
    let diff = diff_text(&repo);

    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert = cmd
        .current_dir(&repo)
        .arg("--mode")
        .arg("impact")
        .arg("--direction")
        .arg("callers")
        .arg("--with-edges")
        .arg("--min-confidence")
        .arg("inferred")
        .arg("--lang")
        .arg("rust")
        .arg("--format")
        .arg("yaml")
        .write_stdin(diff)
        .assert()
        .success();

    let v: serde_yaml::Value = serde_yaml::from_slice(assert.get_output().stdout.as_ref()).unwrap();
    let summary = v
        .get("summary")
        .and_then(|value| value.as_mapping())
        .expect("summary mapping");
    let risk = summary
        .get(serde_yaml::Value::from("risk"))
        .and_then(|value| value.as_mapping())
        .expect("risk mapping");

    let level_key = serde_yaml::Value::from("level");
    let direct_key = serde_yaml::Value::from("direct_hits");
    let transitive_key = serde_yaml::Value::from("transitive_hits");
    let files_key = serde_yaml::Value::from("impacted_files");
    let symbols_key = serde_yaml::Value::from("impacted_symbols");

    assert_eq!(
        risk.get(&level_key).and_then(|value| value.as_str()),
        Some("high")
    );
    assert_eq!(
        risk.get(&direct_key).and_then(|value| value.as_u64()),
        Some(1)
    );
    assert_eq!(
        risk.get(&transitive_key).and_then(|value| value.as_u64()),
        Some(3)
    );
    assert_eq!(
        risk.get(&files_key).and_then(|value| value.as_u64()),
        Some(1)
    );
    assert_eq!(
        risk.get(&symbols_key).and_then(|value| value.as_u64()),
        Some(4)
    );
}
