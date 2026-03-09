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

fn setup_repo_triple() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    let src = r#"fn c() {}
fn b() { c(); }
fn a() { b(); }
"#;
    fs::write(path.join("main.rs"), src).unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    // modify b body
    let src2 = r#"fn c() {}
fn b() { let _k = 1; c(); }
fn a() { b(); }
"#;
    fs::write(path.join("main.rs"), src2).unwrap();
    (dir, path)
}

#[test]
fn cli_impact_direction_callees() {
    let (_tmp, repo) = setup_repo_triple();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();

    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    cmd.current_dir(&repo)
        .arg("--mode")
        .arg("impact")
        .arg("--direction")
        .arg("callees")
        .arg("--lang")
        .arg("rust")
        .arg("--format")
        .arg("json")
        .write_stdin(diff.clone());
    let assert = cmd.assert().success();
    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref());
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let impacted = v["impacted_symbols"].as_array().unwrap();
    let names: Vec<&str> = impacted
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"c"));
}

#[test]
fn cli_impact_max_depth_limits() {
    // change b, callees depth 0 should not include c
    let (_tmp, repo) = setup_repo_triple();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();

    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    cmd.current_dir(&repo)
        .arg("--mode")
        .arg("impact")
        .arg("--direction")
        .arg("callees")
        .arg("--max-depth")
        .arg("0")
        .arg("--lang")
        .arg("rust")
        .arg("--format")
        .arg("json")
        .write_stdin(diff);
    let assert = cmd.assert().success();
    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref());
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let impacted = v["impacted_symbols"].as_array().unwrap();
    let names: Vec<&str> = impacted
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert!(!names.contains(&"c"));
}

#[test]
fn cli_impact_min_confidence_filters_inferred_edges() {
    let (_tmp, repo) = setup_repo_triple();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();

    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert = cmd
        .current_dir(&repo)
        .arg("--mode")
        .arg("impact")
        .arg("--direction")
        .arg("callers")
        .arg("--with-edges")
        .arg("--lang")
        .arg("rust")
        .arg("--format")
        .arg("json")
        .write_stdin(diff.clone())
        .assert()
        .success();
    let v: serde_json::Value = serde_json::from_slice(assert.get_output().stdout.as_ref()).unwrap();
    let impacted = v["impacted_symbols"].as_array().unwrap();
    assert!(!impacted.is_empty());

    let mut strict = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let strict_assert = strict
        .current_dir(&repo)
        .arg("--mode")
        .arg("impact")
        .arg("--direction")
        .arg("callers")
        .arg("--with-edges")
        .arg("--min-confidence")
        .arg("confirmed")
        .arg("--lang")
        .arg("rust")
        .arg("--format")
        .arg("json")
        .write_stdin(diff)
        .assert()
        .success();
    let v2: serde_json::Value =
        serde_json::from_slice(strict_assert.get_output().stdout.as_ref()).unwrap();
    let impacted2 = v2["impacted_symbols"].as_array().unwrap();
    let edges2 = v2["edges"].as_array().unwrap();

    assert!(impacted2.is_empty());
    assert!(edges2.is_empty());
}

#[test]
fn cli_impact_exclude_dynamic_fallback_matches_min_confidence_inferred() {
    let (_tmp, repo) = setup_repo_triple();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();

    let mut a = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let a_out = a
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
        .write_stdin(diff.clone())
        .assert()
        .success();
    let va: serde_json::Value = serde_json::from_slice(a_out.get_output().stdout.as_ref()).unwrap();

    let mut b = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let b_out = b
        .current_dir(&repo)
        .arg("--mode")
        .arg("impact")
        .arg("--direction")
        .arg("callers")
        .arg("--with-edges")
        .arg("--exclude-dynamic-fallback")
        .arg("--lang")
        .arg("rust")
        .arg("--format")
        .arg("json")
        .write_stdin(diff)
        .assert()
        .success();
    let vb: serde_json::Value = serde_json::from_slice(b_out.get_output().stdout.as_ref()).unwrap();

    let impacted_a: std::collections::BTreeSet<&str> = va["impacted_symbols"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| s["name"].as_str())
        .collect();
    let impacted_b: std::collections::BTreeSet<&str> = vb["impacted_symbols"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| s["name"].as_str())
        .collect();
    assert_eq!(impacted_a, impacted_b);

    let edges_a = va["edges"].as_array().unwrap().len();
    let edges_b = vb["edges"].as_array().unwrap().len();
    assert_eq!(edges_a, edges_b);
}

#[test]
fn cli_impact_json_reports_confidence_filter_summary() {
    let (_tmp, repo) = setup_repo_triple();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();

    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert = cmd
        .current_dir(&repo)
        .arg("--mode")
        .arg("impact")
        .arg("--direction")
        .arg("callers")
        .arg("--with-edges")
        .arg("--exclude-dynamic-fallback")
        .arg("--lang")
        .arg("rust")
        .arg("--format")
        .arg("json")
        .write_stdin(diff)
        .assert()
        .success()
        .stderr(predicate::str::contains("confidence filter applied"));

    let v: serde_json::Value = serde_json::from_slice(assert.get_output().stdout.as_ref()).unwrap();
    let cf = &v["confidence_filter"];
    assert_eq!(cf["exclude_dynamic_fallback"], true);
    assert!(
        cf["input_edge_count"].as_u64().unwrap_or(0) >= cf["kept_edge_count"].as_u64().unwrap_or(0)
    );
}

#[test]
fn cli_impact_yaml_reports_confidence_filter_summary() {
    let (_tmp, repo) = setup_repo_triple();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();

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
    let cf = v
        .get("confidence_filter")
        .and_then(|x| x.as_mapping())
        .expect("confidence_filter mapping in yaml output");
    let min_key = serde_yaml::Value::from("min_confidence");
    let exclude_key = serde_yaml::Value::from("exclude_dynamic_fallback");
    assert_eq!(cf.get(&min_key).and_then(|x| x.as_str()), Some("inferred"));
    assert_eq!(cf.get(&exclude_key).and_then(|x| x.as_bool()), Some(false));
}
