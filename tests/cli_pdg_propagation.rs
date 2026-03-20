#![allow(deprecated)]
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

fn setup_two_arg_repo() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    let src = r#"fn callee(a: i32, b: i32) -> i32 { b + 1 }
fn caller() {
    let x = 1;
    let y = 2;
    let out = callee(x, y);
    println!("{}", out);
}
"#;
    fs::write(path.join("f.rs"), src).unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    let src2 = r#"fn callee(a: i32, b: i32) -> i32 { b + 1 }
fn caller() {
    let x = 3;
    let y = 2;
    let out = callee(x, y);
    println!("{}", out);
}
"#;
    fs::write(path.join("f.rs"), src2).unwrap();

    (dir, path)
}

fn setup_repo_with_file(
    rel_path: &str,
    src: &str,
    before: &str,
    after: &str,
) -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    let file_path = path.join(rel_path);
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&file_path, src).unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    let updated = src.replacen(before, after, 1);
    assert_ne!(updated, src, "expected fixture mutation to change source");
    fs::write(&file_path, updated).unwrap();

    (dir, path)
}

fn setup_cross_file_callsite_repo() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    fs::write(
        path.join("callee.rs"),
        "pub fn callee(a: i32) -> i32 { a + 1 }\n",
    )
    .unwrap();
    fs::write(
        path.join("main.rs"),
        r#"mod callee;
fn caller() {
    let x = 1;
    let y = callee::callee(x);
    println!("{}", y);
}
"#,
    )
    .unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    fs::write(
        path.join("main.rs"),
        r#"mod callee;
fn caller() {
    let x = 2;
    let y = callee::callee(x);
    println!("{}", y);
}
"#,
    )
    .unwrap();

    (dir, path)
}

fn setup_cross_file_callers_repo() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    fs::write(
        path.join("callee.rs"),
        "pub fn callee(a: i32) -> i32 { a + 1 }\n",
    )
    .unwrap();
    fs::write(
        path.join("main.rs"),
        r#"mod callee;
fn caller() {
    let x = 1;
    let y = callee::callee(x);
    println!("{}", y);
}
"#,
    )
    .unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    fs::write(
        path.join("callee.rs"),
        "pub fn callee(a: i32) -> i32 { a + 2 }\n",
    )
    .unwrap();

    (dir, path)
}

fn setup_cross_file_wrapper_two_arg_repo() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    fs::write(
        path.join("leaf.rs"),
        r#"pub fn source(v: i32) -> i32 {
    v + 1
}
"#,
    )
    .unwrap();
    fs::write(
        path.join("wrapper.rs"),
        r#"use crate::leaf;

pub fn wrap(left: i32, right: i32) -> i32 {
    let mid = leaf::source(right);
    mid
}
"#,
    )
    .unwrap();
    fs::write(
        path.join("main.rs"),
        r#"mod leaf;
mod wrapper;

fn caller() {
    let x = 1;
    let y = 2;
    let out = wrapper::wrap(x, y);
    println!("{}", out);
}
"#,
    )
    .unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    fs::write(
        path.join("main.rs"),
        r#"mod leaf;
mod wrapper;

fn caller() {
    let x = 3;
    let y = 2;
    let out = wrapper::wrap(x, y);
    println!("{}", out);
}
"#,
    )
    .unwrap();

    (dir, path)
}

fn setup_ruby_require_relative_alias_return_repo() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    fs::create_dir_all(path.join("lib")).unwrap();
    fs::create_dir_all(path.join("app")).unwrap();
    fs::write(
        path.join("lib/service.rb"),
        r#"def bounce(value)
  alias_value = value
  return alias_value
end
"#,
    )
    .unwrap();
    fs::write(
        path.join("app/runner.rb"),
        r#"require_relative '../lib/service'

def entry(seed)
  reply = bounce(seed)
  return reply
end
"#,
    )
    .unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    fs::write(
        path.join("lib/service.rb"),
        r#"def bounce(value)
  alias_value = value
  return alias_value.to_s
end
"#,
    )
    .unwrap();

    (dir, path)
}

fn diff_text(repo: &std::path::Path) -> String {
    let diff_out = git(repo, &["diff", "--no-ext-diff", "--unified=0"]);
    String::from_utf8(diff_out.stdout).unwrap()
}

fn run_impact_json(repo: &std::path::Path, diff: &str, args: &[&str]) -> serde_json::Value {
    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert = cmd
        .current_dir(repo)
        .arg("impact")
        .args(args)
        .write_stdin(diff.to_string())
        .assert()
        .success();
    serde_json::from_slice(&assert.get_output().stdout).expect("json output")
}

fn run_impact_dot(repo: &std::path::Path, diff: &str, args: &[&str]) -> String {
    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert = cmd
        .current_dir(repo)
        .arg("impact")
        .args(args)
        .write_stdin(diff.to_string())
        .assert()
        .success();
    String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 output")
}

#[test]
fn pdg_diff_mode_respects_strict_lsp_engine_selection() {
    let (_tmp, repo) = setup_repo();
    let diff = diff_text(&repo);

    let mut ts_cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    ts_cmd
        .current_dir(&repo)
        .env("DIMPACT_DISABLE_REAL_LSP", "1")
        .arg("impact")
        .args([
            "--engine",
            "ts",
            "--with-pdg",
            "--format",
            "json",
            "--direction",
            "callers",
        ])
        .write_stdin(diff.clone())
        .assert()
        .success();

    for pdg_flag in ["--with-pdg", "--with-propagation"] {
        let mut base_cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
        base_cmd
            .current_dir(&repo)
            .env("DIMPACT_DISABLE_REAL_LSP", "1")
            .arg("impact")
            .args([
                "--engine",
                "lsp",
                "--engine-lsp-strict",
                "--format",
                "json",
                "--direction",
                "callers",
            ])
            .write_stdin(diff.clone())
            .assert()
            .failure()
            .stderr(predicate::str::contains(
                "real LSP disabled by DIMPACT_DISABLE_REAL_LSP=1",
            ));

        let mut pdg_cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
        pdg_cmd
            .current_dir(&repo)
            .env("DIMPACT_DISABLE_REAL_LSP", "1")
            .arg("impact")
            .args([
                "--engine",
                "lsp",
                "--engine-lsp-strict",
                pdg_flag,
                "--format",
                "json",
                "--direction",
                "callers",
            ])
            .write_stdin(diff.clone())
            .assert()
            .failure()
            .stderr(predicate::str::contains(
                "real LSP disabled by DIMPACT_DISABLE_REAL_LSP=1",
            ));
    }
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

#[test]
fn pdg_path_assigns_confirmed_or_inferred_confidence_only() {
    let (_tmp, repo) = setup_repo();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();

    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert = cmd
        .current_dir(&repo)
        .arg("impact")
        .arg("--with-pdg")
        .arg("--with-propagation")
        .arg("--with-edges")
        .arg("--format")
        .arg("json")
        .write_stdin(diff)
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8 output");
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("json output");
    let edges = v["edges"].as_array().expect("edges array");
    assert!(!edges.is_empty(), "expected non-empty edges");

    let certainties: std::collections::BTreeSet<String> = edges
        .iter()
        .filter_map(|e| e["certainty"].as_str().map(|s| s.to_string()))
        .collect();
    let kinds: std::collections::BTreeSet<String> = edges
        .iter()
        .filter_map(|e| e["kind"].as_str().map(|s| s.to_string()))
        .collect();
    let provenances: std::collections::BTreeSet<String> = edges
        .iter()
        .filter_map(|e| e["provenance"].as_str().map(|s| s.to_string()))
        .collect();

    assert!(
        certainties
            .iter()
            .all(|c| c == "confirmed" || c == "inferred"),
        "unexpected certainty values: {:?}",
        certainties
    );
    assert!(
        !certainties.contains("dynamic_fallback"),
        "PDG path should not emit dynamic_fallback certainty"
    );
    assert!(
        kinds.contains("call"),
        "expected merged call edges: {:?}",
        kinds
    );
    assert!(
        kinds.contains("data"),
        "expected merged data edges: {:?}",
        kinds
    );
    assert!(
        provenances.contains("call_graph"),
        "expected call_graph provenance: {:?}",
        provenances
    );
    assert!(
        provenances.contains("symbolic_propagation"),
        "expected symbolic_propagation provenance: {:?}",
        provenances
    );
}

#[test]
fn pdg_propagation_adds_direct_summary_bridge_for_single_line_callee() {
    let (_tmp, repo) = setup_repo();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();

    let mut cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let assert = cmd
        .current_dir(&repo)
        .arg("impact")
        .arg("--with-propagation")
        .arg("--format")
        .arg("dot")
        .write_stdin(diff)
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(assert.get_output().stdout.as_ref());
    assert!(
        stdout.contains("\"f.rs:use:x:4\" -> \"f.rs:def:y:4\""),
        "expected summary bridge from callsite arg use into assigned def, got:\n{}",
        stdout
    );
}

#[test]
fn pdg_propagation_adds_cross_file_summary_bridge_for_direct_callee() {
    let (_tmp, repo) = setup_cross_file_callsite_repo();
    let diff = diff_text(&repo);

    let pdg = run_impact_dot(
        &repo,
        &diff,
        &["--direction", "callees", "--with-pdg", "--format", "dot"],
    );
    let prop = run_impact_dot(
        &repo,
        &diff,
        &[
            "--direction",
            "callees",
            "--with-propagation",
            "--format",
            "dot",
        ],
    );

    assert!(
        !pdg.contains("\"main.rs:use:x:4\" -> \"main.rs:def:y:4\""),
        "plain PDG should not synthesize the cross-file summary bridge, got:\n{}",
        pdg
    );
    assert!(
        prop.contains("\"main.rs:use:x:4\" -> \"main.rs:def:y:4\""),
        "expected propagation to recover the cross-file summary bridge, got:\n{}",
        prop
    );
    assert!(
        prop.contains("\"callee.rs:def:a:1\""),
        "expected callee-file DFG nodes to be present after related-file expansion, got:\n{}",
        prop
    );
}

#[test]
fn pdg_propagation_does_not_leak_irrelevant_two_arg_bridge() {
    let (_tmp, repo) = setup_two_arg_repo();
    let diff = diff_text(&repo);

    let stdout = run_impact_dot(&repo, &diff, &["--with-propagation", "--format", "dot"]);
    assert!(
        stdout.contains("\"f.rs:use:y:5\" -> \"f.rs:def:out:5\""),
        "expected later arg to keep its summary bridge, got:\n{}",
        stdout
    );
    assert!(
        !stdout.contains("\"f.rs:use:x:5\" -> \"f.rs:def:out:5\""),
        "unexpected irrelevant arg bridge leaked into output, got:\n{}",
        stdout
    );
}

#[test]
fn pdg_propagation_maps_multi_file_wrapper_return_without_leaking_irrelevant_arg() {
    let (_tmp, repo) = setup_cross_file_wrapper_two_arg_repo();
    let diff = diff_text(&repo);

    let pdg = run_impact_dot(
        &repo,
        &diff,
        &["--direction", "callees", "--with-pdg", "--format", "dot"],
    );
    let prop = run_impact_dot(
        &repo,
        &diff,
        &[
            "--direction",
            "callees",
            "--with-propagation",
            "--format",
            "dot",
        ],
    );

    assert!(
        !pdg.contains("\"wrapper.rs:def:right:3\""),
        "plain PDG should not expand wrapper-file DFG nodes, got:\n{}",
        pdg
    );
    assert!(
        !pdg.contains("\"main.rs:use:y:7\" -> \"main.rs:def:out:7\""),
        "plain PDG should not synthesize the wrapper return bridge, got:\n{}",
        pdg
    );
    assert!(
        prop.contains("\"wrapper.rs:def:right:3\""),
        "expected propagation to expand wrapper-file DFG nodes, got:\n{}",
        prop
    );
    assert!(
        prop.contains("\"main.rs:use:y:7\" -> \"main.rs:def:out:7\""),
        "expected propagation to bridge the return flow from the relevant arg, got:\n{}",
        prop
    );
    assert!(
        !prop.contains("\"main.rs:use:x:7\" -> \"main.rs:def:out:7\""),
        "unexpected irrelevant arg bridge leaked through wrapper summary, got:\n{}",
        prop
    );
}

#[test]
fn propagation_callers_edges_keep_cross_file_callsite_bridges_but_drop_irrelevant_symbol_fanout() {
    let (_tmp, repo) = setup_cross_file_callers_repo();
    let diff = diff_text(&repo);

    let prop = run_impact_json(
        &repo,
        &diff,
        &[
            "--direction",
            "callers",
            "--with-propagation",
            "--format",
            "json",
            "--with-edges",
        ],
    );
    let edges = prop["edges"].as_array().expect("edges array");
    let pairs: std::collections::BTreeSet<(String, String)> = edges
        .iter()
        .map(|e| {
            (
                e["from"].as_str().unwrap().to_string(),
                e["to"].as_str().unwrap().to_string(),
            )
        })
        .collect();

    assert!(
        pairs.contains(&(
            "rust:main.rs:fn:caller:2".to_string(),
            "main.rs:use:x:4".to_string(),
        )),
        "expected impacted caller to retain the callsite-use bridge: {prop:#}"
    );
    assert!(
        !pairs.contains(&(
            "rust:main.rs:fn:caller:2".to_string(),
            "main.rs:def:x:3".to_string(),
        )),
        "unexpected non-callsite fanout into caller-local seed def leaked into edges: {prop:#}"
    );
    assert!(
        !pairs.contains(&(
            "rust:main.rs:fn:caller:2".to_string(),
            "main.rs:use:y:5".to_string(),
        )),
        "unexpected post-call use fanout leaked into edges: {prop:#}"
    );
}

#[test]
fn pdg_keeps_latest_def_and_avoids_stale_reassignment_edge() {
    let src = r#"fn demo() {
    let mut a = 1;
    let b = a;
    a = 2;
    let c = a;
    let d = b;
    println!("{} {}", c, d);
}
"#;
    let (_tmp, repo) = setup_repo_with_file(
        "f.rs",
        src,
        "println!(\"{} {}\", c, d);",
        "println!(\"{} {}!\", c, d);",
    );
    let diff = diff_text(&repo);

    let pdg = run_impact_dot(
        &repo,
        &diff,
        &["--direction", "callees", "--with-pdg", "--format", "dot"],
    );
    let prop = run_impact_dot(
        &repo,
        &diff,
        &[
            "--direction",
            "callees",
            "--with-propagation",
            "--format",
            "dot",
        ],
    );

    for out in [&pdg, &prop] {
        assert!(
            out.contains("\"f.rs:def:a:2\" -> \"f.rs:def:b:3\""),
            "expected alias edge a:2 -> b:3, got:\n{}",
            out
        );
        assert!(
            out.contains("\"f.rs:def:a:4\" -> \"f.rs:def:c:5\""),
            "expected latest-def edge a:4 -> c:5, got:\n{}",
            out
        );
        assert!(
            out.contains("\"f.rs:def:b:3\" -> \"f.rs:def:d:6\""),
            "expected alias chain edge b:3 -> d:6, got:\n{}",
            out
        );
        assert!(
            !out.contains("\"f.rs:def:a:2\" -> \"f.rs:def:c:5\""),
            "stale a:2 -> c:5 edge should not reappear, got:\n{}",
            out
        );
    }
}

#[test]
fn ruby_chain_fixture_only_gains_symbolic_edges_with_propagation() {
    let src = include_str!("fixtures/ruby/analyzer_hard_cases_callees_chain_alias_return.rb");
    let (_tmp, repo) = setup_repo_with_file("demo/test.rb", src, "v + inc", "(v + inc) + 1");
    let diff = diff_text(&repo);

    let baseline = run_impact_json(
        &repo,
        &diff,
        &[
            "--direction",
            "callers",
            "--lang",
            "ruby",
            "--format",
            "json",
            "--with-edges",
        ],
    );
    let pdg = run_impact_json(
        &repo,
        &diff,
        &[
            "--direction",
            "callers",
            "--lang",
            "ruby",
            "--with-pdg",
            "--format",
            "json",
            "--with-edges",
        ],
    );
    let prop = run_impact_json(
        &repo,
        &diff,
        &[
            "--direction",
            "callers",
            "--lang",
            "ruby",
            "--with-propagation",
            "--format",
            "json",
            "--with-edges",
        ],
    );

    assert_eq!(baseline["edges"].as_array().unwrap().len(), 0);
    assert_eq!(pdg["edges"].as_array().unwrap().len(), 0);

    let prop_edges = prop["edges"].as_array().expect("edges array");
    assert_eq!(
        prop_edges.len(),
        2,
        "expected fixed pair of symbolic edges: {prop:#}"
    );
    assert!(prop_edges.iter().all(|e| e["kind"] == "data"));
    assert!(
        prop_edges
            .iter()
            .all(|e| e["provenance"] == "symbolic_propagation")
    );
    assert!(
        prop_edges
            .iter()
            .any(|e| e["to"] == "demo/test.rb:def:v:14")
    );
    assert!(
        prop_edges
            .iter()
            .any(|e| e["to"] == "demo/test.rb:use:v:16")
    );
}

#[test]
fn ruby_require_relative_alias_return_only_gains_symbolic_edges_with_propagation() {
    let (_tmp, repo) = setup_ruby_require_relative_alias_return_repo();
    let diff = diff_text(&repo);

    let baseline = run_impact_json(
        &repo,
        &diff,
        &[
            "--direction",
            "callers",
            "--lang",
            "ruby",
            "--format",
            "json",
            "--with-edges",
        ],
    );
    let pdg = run_impact_json(
        &repo,
        &diff,
        &[
            "--direction",
            "callers",
            "--lang",
            "ruby",
            "--with-pdg",
            "--format",
            "json",
            "--with-edges",
        ],
    );
    let prop = run_impact_json(
        &repo,
        &diff,
        &[
            "--direction",
            "callers",
            "--lang",
            "ruby",
            "--with-propagation",
            "--format",
            "json",
            "--with-edges",
        ],
    );

    let data_pairs = |value: &serde_json::Value| -> std::collections::BTreeSet<(String, String)> {
        value["edges"]
            .as_array()
            .expect("edges array")
            .iter()
            .filter(|e| e["kind"] == "data")
            .map(|e| {
                (
                    e["from"].as_str().unwrap().to_string(),
                    e["to"].as_str().unwrap().to_string(),
                )
            })
            .collect()
    };

    assert!(
        data_pairs(&baseline).is_empty(),
        "baseline should keep pure call edges"
    );
    assert!(
        data_pairs(&pdg).is_empty(),
        "plain PDG should not add cross-file alias bridges"
    );

    let prop_data = data_pairs(&prop);
    assert!(
        prop_data.contains(&(
            "app/runner.rb:use:seed:4".to_string(),
            "ruby:lib/service.rb:method:bounce:1".to_string(),
        )),
        "expected propagation to connect caller arg use into required callee: {prop:#}"
    );
    assert!(
        prop_data.contains(&(
            "ruby:lib/service.rb:method:bounce:1".to_string(),
            "app/runner.rb:def:reply:4".to_string(),
        )),
        "expected propagation to connect callee return back into caller def: {prop:#}"
    );
    assert!(
        prop["edges"].as_array().is_some_and(|edges| edges
            .iter()
            .filter(|e| e["kind"] == "data")
            .all(|e| e["provenance"] == "symbolic_propagation")),
        "expected propagation-only data edges to stay tagged as symbolic_propagation: {prop:#}"
    );
}

#[test]
fn ruby_alias_define_fixture_keeps_defined_sym_without_leaking_defined_only() {
    let src = include_str!("fixtures/ruby/analyzer_hard_cases_dynamic_alias_define_method.rb");
    let (_tmp, repo) = setup_repo_with_file("demo/test.rb", src, ":ok", ":ko");
    let diff = diff_text(&repo);

    let prop = run_impact_json(
        &repo,
        &diff,
        &[
            "--direction",
            "callers",
            "--lang",
            "ruby",
            "--with-propagation",
            "--format",
            "json",
            "--with-edges",
        ],
    );
    let edges = prop["edges"].as_array().expect("edges array");

    assert!(
        edges
            .iter()
            .any(|e| e["to"] == "ruby:demo/test.rb:method:defined_sym:9")
    );
    assert!(
        !edges
            .iter()
            .any(|e| e["to"] == "ruby:demo/test.rb:method:defined_only:17"),
        "unexpected leak into defined_only: {prop:#}"
    );
}

#[test]
fn ruby_send_fixture_keeps_target_separation_under_propagation() {
    let src = include_str!("fixtures/ruby/analyzer_hard_cases_dynamic_send_public_send.rb");
    let (_tmp, repo) = setup_repo_with_file(
        "demo/test.rb",
        src,
        "send(:target_sym)",
        "send(:target_sym).to_s",
    );
    let diff = diff_text(&repo);

    let prop = run_impact_json(
        &repo,
        &diff,
        &[
            "--direction",
            "callers",
            "--lang",
            "ruby",
            "--with-propagation",
            "--format",
            "json",
            "--with-edges",
        ],
    );
    let targets: std::collections::BTreeSet<String> = prop["edges"]
        .as_array()
        .expect("edges array")
        .iter()
        .filter_map(|e| e["to"].as_str().map(|s| s.to_string()))
        .collect();

    assert_eq!(
        targets,
        std::collections::BTreeSet::from([
            "ruby:demo/test.rb:method:target_sym:2".to_string(),
            "ruby:demo/test.rb:method:target_str:6".to_string(),
        ]),
        "propagation should keep dynamic target separation: {prop:#}"
    );
}

#[test]
fn per_seed_diff_mode_supports_propagation() {
    let (_tmp, repo) = setup_repo();
    let diff = diff_text(&repo);

    let grouped = run_impact_json(
        &repo,
        &diff,
        &[
            "--direction",
            "callees",
            "--with-propagation",
            "--with-edges",
            "--per-seed",
            "--format",
            "json",
        ],
    );

    let grouped = grouped.as_array().expect("per-seed top-level array");
    assert_eq!(grouped.len(), 1);
    assert_eq!(
        grouped[0]["changed_symbol"]["name"].as_str(),
        Some("caller")
    );
    let output = &grouped[0]["impacts"][0]["output"];
    assert!(
        output["impacted_symbols"]
            .as_array()
            .is_some_and(|syms| !syms.is_empty()),
        "expected impacted symbols in per-seed propagation output: {grouped:#?}"
    );
    assert!(
        output["edges"].as_array().is_some_and(|edges| edges
            .iter()
            .any(|e| { e["provenance"] == "call_graph" && e["kind"] == "call" })),
        "expected merged call edge in per-seed output: {grouped:#?}"
    );
    assert!(
        output["impacted_witnesses"]
            .as_object()
            .is_some_and(|w| w.contains_key("rust:f.rs:fn:callee:1")),
        "expected per-seed witness nesting for impacted callee: {grouped:#?}"
    );
    let witness = &output["impacted_witnesses"]["rust:f.rs:fn:callee:1"];
    assert_eq!(
        witness["path"][0]["from_symbol_id"].as_str(),
        Some("rust:f.rs:fn:caller:2")
    );
    assert_eq!(
        witness["path"][0]["to_symbol_id"].as_str(),
        Some("rust:f.rs:fn:callee:1")
    );
    assert_eq!(witness["provenance_chain"][0].as_str(), Some("call_graph"));
    assert_eq!(witness["kind_chain"][0].as_str(), Some("call"));
}

#[test]
fn per_seed_seed_mode_supports_pdg() {
    let (_tmp, repo) = setup_repo();

    let grouped = run_impact_json(
        &repo,
        "",
        &[
            "--direction",
            "callees",
            "--seed-symbol",
            "rust:f.rs:fn:caller:2",
            "--with-pdg",
            "--with-edges",
            "--per-seed",
            "--format",
            "json",
        ],
    );

    let grouped = grouped.as_array().expect("per-seed top-level array");
    assert_eq!(grouped.len(), 1);
    assert_eq!(
        grouped[0]["changed_symbol"]["id"].as_str(),
        Some("rust:f.rs:fn:caller:2")
    );
    let output = &grouped[0]["impacts"][0]["output"];
    assert!(
        output["impacted_symbols"]
            .as_array()
            .is_some_and(|syms| !syms.is_empty()),
        "expected impacted symbols in seed-based per-seed PDG output: {grouped:#?}"
    );
    assert!(
        output["edges"].as_array().is_some_and(|edges| edges
            .iter()
            .any(|e| { e["provenance"] == "call_graph" && e["kind"] == "call" })),
        "expected call graph edges in seed-based per-seed PDG output: {grouped:#?}"
    );
}
