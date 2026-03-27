#![allow(deprecated)]
mod json_output;

use std::collections::BTreeSet;
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

fn setup_repo_chain() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    let before = r#"fn leaf() {}
fn mid() { leaf(); }
fn top() { mid(); }
"#;
    fs::write(path.join("main.rs"), before).unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    let after = r#"fn leaf() { let _x = 1; }
fn mid() { leaf(); }
fn top() { mid(); }
"#;
    fs::write(path.join("main.rs"), after).unwrap();
    (dir, path)
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
    json_output::parse_payload_slice(assert.get_output().stdout.as_ref())
}

fn by_depth_tuples(output: &serde_json::Value) -> Vec<(u64, u64, u64)> {
    output["summary"]["by_depth"]
        .as_array()
        .expect("summary.by_depth array")
        .iter()
        .map(|bucket| {
            (
                bucket["depth"].as_u64().expect("depth"),
                bucket["symbol_count"].as_u64().expect("symbol_count"),
                bucket["file_count"].as_u64().expect("file_count"),
            )
        })
        .collect()
}

fn risk_tuple(output: &serde_json::Value) -> (&str, u64, u64, u64, u64) {
    let risk = &output["summary"]["risk"];
    (
        risk["level"].as_str().expect("risk.level"),
        risk["direct_hits"].as_u64().expect("risk.direct_hits"),
        risk["transitive_hits"]
            .as_u64()
            .expect("risk.transitive_hits"),
        risk["impacted_files"]
            .as_u64()
            .expect("risk.impacted_files"),
        risk["impacted_symbols"]
            .as_u64()
            .expect("risk.impacted_symbols"),
    )
}

#[test]
fn cli_impact_by_depth_callers_basic_depth_bucketing() {
    let (_tmp, repo) = setup_repo_chain();
    let diff = diff_text(&repo);

    let v = run_impact_json(&repo, &diff, &["--with-edges"]);

    let impacted_names: BTreeSet<&str> = v["impacted_symbols"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|symbol| symbol["name"].as_str())
        .collect();
    assert_eq!(impacted_names, BTreeSet::from(["mid", "top"]));
    assert_eq!(by_depth_tuples(&v), vec![(1, 1, 1), (2, 1, 1)]);
    assert_eq!(risk_tuple(&v), ("medium", 1, 1, 1, 2));
    assert!(!v["edges"].as_array().unwrap().is_empty());
}

#[test]
fn cli_impact_by_depth_present_without_edges_output() {
    let (_tmp, repo) = setup_repo_chain();
    let diff = diff_text(&repo);

    let v = run_impact_json(&repo, &diff, &[]);

    assert!(v["edges"].as_array().unwrap().is_empty());
    assert_eq!(by_depth_tuples(&v), vec![(1, 1, 1), (2, 1, 1)]);
    assert_eq!(risk_tuple(&v), ("medium", 1, 1, 1, 2));
}

#[test]
fn cli_impact_by_depth_per_seed_nests_under_output() {
    let (_tmp, repo) = setup_repo_chain();
    let diff = diff_text(&repo);

    let v = run_impact_json(&repo, &diff, &["--per-seed"]);

    let grouped = v.as_array().expect("per-seed top-level array");
    assert_eq!(grouped.len(), 1);
    assert_eq!(grouped[0]["changed_symbol"]["name"].as_str(), Some("leaf"));
    let output = &grouped[0]["impacts"][0]["output"];
    assert_eq!(by_depth_tuples(output), vec![(1, 1, 1), (2, 1, 1)]);
    assert_eq!(risk_tuple(output), ("medium", 1, 1, 1, 2));
}

#[test]
fn cli_impact_by_depth_confidence_filter_updates_summary() {
    let (_tmp, repo) = setup_repo_chain();
    let diff = diff_text(&repo);

    let v = run_impact_json(&repo, &diff, &["--min-confidence", "confirmed"]);

    assert!(v["impacted_symbols"].as_array().unwrap().is_empty());
    assert!(v["edges"].as_array().unwrap().is_empty());
    assert!(by_depth_tuples(&v).is_empty());
    assert_eq!(risk_tuple(&v), ("low", 0, 0, 0, 0));
    assert_eq!(v["confidence_filter"]["kept_edge_count"].as_u64(), Some(0));
    assert!(
        v["confidence_filter"]["input_edge_count"]
            .as_u64()
            .unwrap_or(0)
            > 0
    );
}
