#![allow(deprecated)]
#![allow(unused)]
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::Value;
use tempfile::TempDir;

fn git(cwd: &Path, args: &[&str]) -> std::process::Output {
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

fn run_changed_and_impacted(
    lang: &str,
    filename: &str,
    before: &str,
    after: &str,
) -> (BTreeSet<String>, BTreeSet<String>) {
    let dir = TempDir::new().unwrap();
    let repo = dir.path().to_path_buf();

    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "tester@example.com"]);
    git(&repo, &["config", "user.name", "Tester"]);

    let p = repo.join(filename);
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&p, before).unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "init", "-q"]);

    fs::write(&p, after).unwrap();

    let diff = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let mut changed_cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let changed_assert = changed_cmd
        .current_dir(&repo)
        .arg("--mode")
        .arg("changed")
        .arg("--lang")
        .arg(lang)
        .arg("--format")
        .arg("json")
        .write_stdin(String::from_utf8(diff.stdout.clone()).unwrap())
        .assert()
        .success();
    let changed_json: Value =
        serde_json::from_slice(changed_assert.get_output().stdout.as_ref()).unwrap();

    let changed_names: BTreeSet<String> = changed_json["changed_symbols"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| s["name"].as_str())
        .map(ToString::to_string)
        .collect();

    let diff2 = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let mut impact_cmd = assert_cmd::Command::cargo_bin("dimpact").unwrap();
    let impact_assert = impact_cmd
        .current_dir(&repo)
        .arg("--mode")
        .arg("impact")
        .arg("--direction")
        .arg("callers")
        .arg("--lang")
        .arg(lang)
        .arg("--format")
        .arg("json")
        .write_stdin(String::from_utf8(diff2.stdout).unwrap())
        .assert()
        .success();
    let impact_json: Value =
        serde_json::from_slice(impact_assert.get_output().stdout.as_ref()).unwrap();

    let impacted_names: BTreeSet<String> = impact_json["impacted_symbols"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| s["name"].as_str())
        .map(ToString::to_string)
        .collect();

    (changed_names, impacted_names)
}

#[test]
fn changed_impacted_golden_baseline_matrix_v73() {
    let ts_before = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/typescript/analyzer_hard_cases_dispatch_overload_optional_chain.ts"
    ));
    let ts_after = ts_before.replace(
        "return typeof v === \"number\" ? v : Number.parseInt(v, 10);",
        "return typeof v === \"number\" ? v : Number.parseInt(v, 10) + 1;",
    );

    let tsx_before = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/tsx/analyzer_hard_cases_component_callback_optional_chain.tsx"
    ));
    let tsx_after = tsx_before.replace(
        "return <section>{handle(props.item)}</section>;",
        "return <section>{handle(props.item)}!</section>;",
    );

    let rust_before = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/rust/analyzer_hard_cases_trait_dispatch_method_value_generic.rs"
    ));
    let rust_after = rust_before.replace(
        "self.worker.handle(first)",
        "self.worker.handle(first.clone())",
    );

    let java_before = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/java/analyzer_hard_cases_lambda_methodref_overload.java"
    ));
    let java_after = java_before.replacen(
        "return Integer.parseInt(s);",
        "return Integer.parseInt(s) + 1;",
        1,
    );

    let go_before = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/go/analyzer_hard_cases_interface_dispatch_method_value_generic_receiver.go"
    ));
    let go_after = go_before.replace(
        "return b.inner.Handle(context.Background())",
        "return b.inner.Handle(context.Background()) // tweak",
    );

    let ruby_before = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/ruby/analyzer_hard_cases_dynamic_send_public_send.rb"
    ));
    let ruby_after = ruby_before.replacen(":ok", ":ok2", 1);

    let python_before = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/python/analyzer_hard_cases_dynamic_getattr_setattr_getattribute.py"
    ));
    let python_after = python_before.replace("payload.strip()", "payload.rstrip()");

    let python_monkey_before = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/python/analyzer_hard_cases_dynamic_monkeypatch_metaclass_protocol.py"
    ));
    let python_monkey_after =
        python_monkey_before.replace("payload.strip().upper()", "payload.strip().lower()");

    let cases = vec![
        (
            "typescript",
            "demo/a.ts",
            ts_before,
            ts_after.as_str(),
            BTreeSet::from(["parse".to_string()]),
            BTreeSet::from(["run".to_string()]),
        ),
        (
            "tsx",
            "demo/a.tsx",
            tsx_before,
            tsx_after.as_str(),
            BTreeSet::from(["Panel".to_string()]),
            BTreeSet::new(),
        ),
        (
            "rust",
            "demo/a.rs",
            rust_before,
            rust_after.as_str(),
            BTreeSet::from(["run".to_string()]),
            BTreeSet::new(),
        ),
        (
            "java",
            "demo/A.java",
            java_before,
            java_after.as_str(),
            BTreeSet::from(["OverloadLab".to_string(), "parse".to_string()]),
            BTreeSet::from(["parse".to_string(), "run".to_string()]),
        ),
        (
            "go",
            "demo/a.go",
            go_before,
            go_after.as_str(),
            BTreeSet::from(["Run".to_string()]),
            BTreeSet::new(),
        ),
        (
            "ruby",
            "demo/a.rb",
            ruby_before,
            ruby_after.as_str(),
            BTreeSet::from(["DynamicDispatch".to_string(), "target_sym".to_string()]),
            BTreeSet::from(["execute".to_string()]),
        ),
        (
            "python",
            "demo/a.py",
            python_before,
            python_after.as_str(),
            BTreeSet::from(["DynamicAccessor".to_string(), "__getattr__".to_string()]),
            BTreeSet::from(["__init__".to_string(), "execute".to_string()]),
        ),
        (
            "python",
            "demo/monkey.py",
            python_monkey_before,
            python_monkey_after.as_str(),
            BTreeSet::from(["patched_run".to_string()]),
            BTreeSet::from(["install_patch".to_string(), "execute".to_string()]),
        ),
    ];

    for (lang, file, before, after, changed_golden, impacted_golden) in cases {
        let (changed, impacted) = run_changed_and_impacted(lang, file, before, after);
        assert_eq!(
            changed, changed_golden,
            "changed golden mismatch for lang={} file={}",
            lang, file
        );
        assert_eq!(
            impacted, impacted_golden,
            "impacted golden mismatch for lang={} file={}",
            lang, file
        );
    }
}
