use std::fs;
use std::process::Command;

#[test]
fn bench_script_lang_summary_includes_go_java() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let ts_json = tmp.path().join("ts.json");
    let lsp_json = tmp.path().join("lsp.json");

    fs::write(
        &ts_json,
        r#"{
  "changed_symbols": [
    {"language": "go"},
    {"language": "go"},
    {"language": "java"}
  ],
  "impacted_symbols": [
    {"language": "go"},
    {"language": "java"},
    {"language": "java"}
  ]
}
"#,
    )
    .unwrap();

    fs::write(
        &lsp_json,
        r#"{
  "changed_symbols": [
    {"language": "java"},
    {"language": "java"},
    {"language": "go"}
  ],
  "impacted_symbols": [
    {"language": "go"},
    {"language": "java"}
  ]
}
"#,
    )
    .unwrap();

    let repo = env!("CARGO_MANIFEST_DIR");
    let script = format!("{repo}/scripts/bench-impact-engines.sh");
    let out = Command::new("bash")
        .current_dir(repo)
        .arg(script)
        .arg("--summary-ts-json")
        .arg(&ts_json)
        .arg("--summary-lsp-json")
        .arg(&lsp_json)
        .output()
        .expect("run bench-impact-engines.sh");

    assert!(
        out.status.success(),
        "script failed\nstdout:{}\nstderr:{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("[lang-summary]"));
    assert!(stdout.contains("ts changed_by_lang: go:2, java:1"));
    assert!(stdout.contains("ts impacted_by_lang: go:1, java:2"));
    assert!(stdout.contains("lsp(strict) changed_by_lang: go:1, java:2"));
    assert!(stdout.contains("lsp(strict) impacted_by_lang: go:1, java:1"));
}
