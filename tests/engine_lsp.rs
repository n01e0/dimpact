use dimpact::engine::CapsHint;
use serial_test::serial;
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

fn setup_repo_basic() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    fs::write(path.join("main.rs"), "fn bar() {}\nfn foo() { bar(); }\n").unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);
    fs::write(
        path.join("main.rs"),
        "fn bar() { let _x=1; }\nfn foo() { bar(); }\n",
    )
    .unwrap();
    (dir, path)
}

fn has_rust_analyzer() -> bool {
    Command::new("rust-analyzer")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn should_run_strict_lsp_e2e() -> bool {
    std::env::var("DIMPACT_E2E_STRICT_LSP")
        .ok()
        .as_deref()
        == Some("1")
}

fn setup_repo_rust_project(initial_src: &str, updated_src: &str) -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    fs::create_dir_all(path.join("src")).unwrap();
    fs::write(
        path.join("Cargo.toml"),
        "[package]\nname = \"lsp_fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    fs::write(path.join("src/main.rs"), initial_src).unwrap();

    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    fs::write(path.join("src/main.rs"), updated_src).unwrap();
    (dir, path)
}

fn impacted_name_set(out: &dimpact::ImpactOutput) -> BTreeSet<String> {
    out.impacted_symbols.iter().map(|s| s.name.clone()).collect()
}

#[test]
#[serial]
fn lsp_engine_falls_back_changed() {
    let (_tmp, repo) = setup_repo_basic();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();

    let files = dimpact::parse_unified_diff(&diff).unwrap();
    let cfg = dimpact::EngineConfig {
        lsp_strict: false,
        dump_capabilities: false,
        mock_lsp: false,
        mock_caps: None,
    };
    let engine = dimpact::engine::make_engine(dimpact::EngineKind::Lsp, cfg);

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let changed = engine
        .changed_symbols(&files, dimpact::LanguageMode::Rust)
        .unwrap();
    std::env::set_current_dir(cwd).unwrap();

    assert!(changed.changed_symbols.iter().any(|s| s.name == "bar"));
}

#[test]
#[serial]
fn lsp_engine_falls_back_impact_callers() {
    let (_tmp, repo) = setup_repo_basic();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();
    let files = dimpact::parse_unified_diff(&diff).unwrap();

    let cfg = dimpact::EngineConfig {
        lsp_strict: false,
        dump_capabilities: false,
        mock_lsp: false,
        mock_caps: None,
    };
    let engine = dimpact::engine::make_engine(dimpact::EngineKind::Auto, cfg);
    let opts = dimpact::ImpactOptions {
        direction: dimpact::ImpactDirection::Callers,
        max_depth: Some(100),
        with_edges: Some(false),
        ignore_dirs: Vec::new(),
    };

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let out = engine
        .impact(&files, dimpact::LanguageMode::Rust, &opts)
        .unwrap();
    std::env::set_current_dir(cwd).unwrap();

    assert!(out.impacted_symbols.iter().any(|s| s.name == "foo"));
}

#[test]
#[serial]
fn lsp_engine_strict_errors() {
    // Ensure tests do not accidentally use a real server on dev machines
    unsafe {
        std::env::set_var("DIMPACT_DISABLE_REAL_LSP", "1");
    }
    let (_tmp, repo) = setup_repo_basic();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();
    let files = dimpact::parse_unified_diff(&diff).unwrap();

    let cfg = dimpact::EngineConfig {
        lsp_strict: true,
        dump_capabilities: false,
        mock_lsp: false,
        mock_caps: None,
    };
    let engine = dimpact::engine::make_engine(dimpact::EngineKind::Lsp, cfg);

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let err = engine
        .changed_symbols(&files, dimpact::LanguageMode::Rust)
        .err();
    std::env::set_current_dir(cwd).unwrap();

    assert!(
        err.is_some(),
        "strict mode should error when LSP unavailable"
    );
}

#[test]
#[serial]
fn lsp_engine_strict_mock_succeeds() {
    let (_tmp, repo) = setup_repo_basic();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();
    let files = dimpact::parse_unified_diff(&diff).unwrap();

    // Enable mock LSP session (pretend server available)
    let cfg = dimpact::EngineConfig {
        lsp_strict: true,
        dump_capabilities: true,
        mock_lsp: true,
        mock_caps: None,
    };
    let engine = dimpact::engine::make_engine(dimpact::EngineKind::Lsp, cfg);

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let changed = engine
        .changed_symbols(&files, dimpact::LanguageMode::Rust)
        .unwrap();
    let opts = dimpact::ImpactOptions {
        direction: dimpact::ImpactDirection::Callers,
        max_depth: Some(100),
        with_edges: Some(false),
        ignore_dirs: Vec::new(),
    };
    let out = engine
        .impact(&files, dimpact::LanguageMode::Rust, &opts)
        .unwrap();
    std::env::set_current_dir(cwd).unwrap();
    // no global env var modifications

    assert!(changed.changed_symbols.iter().any(|s| s.name == "bar"));
    assert!(out.impacted_symbols.iter().any(|s| s.name == "foo"));
}

#[test]
#[serial]
fn lsp_engine_strict_errors_when_caps_missing() {
    let (_tmp, repo) = setup_repo_basic();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();
    let files = dimpact::parse_unified_diff(&diff).unwrap();

    // Mock LSP available but without document/workspace symbol capabilities
    let caps = CapsHint {
        document_symbol: false,
        workspace_symbol: false,
        call_hierarchy: false,
        references: false,
        definition: false,
    };
    let cfg = dimpact::EngineConfig {
        lsp_strict: true,
        dump_capabilities: false,
        mock_lsp: true,
        mock_caps: Some(caps),
    };
    let engine = dimpact::engine::make_engine(dimpact::EngineKind::Lsp, cfg);

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let err = engine
        .changed_symbols(&files, dimpact::LanguageMode::Rust)
        .err();
    std::env::set_current_dir(cwd).unwrap();
    assert!(err.is_some(), "strict + no caps should error");
}

#[test]
#[serial]
fn lsp_engine_strict_impact_errors_when_caps_missing() {
    let (_tmp, repo) = setup_repo_basic();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();
    let files = dimpact::parse_unified_diff(&diff).unwrap();

    // No callHierarchy/references/definition -> impact should error under strict
    let caps = CapsHint {
        document_symbol: true,
        workspace_symbol: true,
        call_hierarchy: false,
        references: false,
        definition: false,
    };
    let cfg = dimpact::EngineConfig {
        lsp_strict: true,
        dump_capabilities: false,
        mock_lsp: true,
        mock_caps: Some(caps),
    };
    let engine = dimpact::engine::make_engine(dimpact::EngineKind::Lsp, cfg);
    let opts = dimpact::ImpactOptions {
        direction: dimpact::ImpactDirection::Callers,
        max_depth: Some(1),
        with_edges: Some(false),
        ignore_dirs: Vec::new(),
    };

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let err = engine
        .impact(&files, dimpact::LanguageMode::Rust, &opts)
        .err();
    std::env::set_current_dir(cwd).unwrap();
    assert!(err.is_some(), "strict + no impact caps should error");
}

#[test]
#[serial]
fn lsp_engine_non_strict_impact_from_symbols_falls_back_when_lsp_unavailable() {
    // Ensure this path exercises fallback from LSP session init failure
    unsafe {
        std::env::set_var("DIMPACT_DISABLE_REAL_LSP", "1");
    }
    let (_tmp, repo) = setup_repo_basic();
    let cfg = dimpact::EngineConfig {
        lsp_strict: false,
        dump_capabilities: false,
        mock_lsp: false,
        mock_caps: None,
    };
    let engine = dimpact::engine::make_engine(dimpact::EngineKind::Lsp, cfg);
    let changed = vec![dimpact::Symbol {
        id: dimpact::SymbolId::new("rust", "main.rs", &dimpact::SymbolKind::Function, "bar", 1),
        name: "bar".to_string(),
        kind: dimpact::SymbolKind::Function,
        file: "main.rs".to_string(),
        range: dimpact::TextRange {
            start_line: 1,
            end_line: 1,
        },
        language: "rust".to_string(),
    }];
    let opts = dimpact::ImpactOptions {
        direction: dimpact::ImpactDirection::Callers,
        max_depth: Some(3),
        with_edges: Some(false),
        ignore_dirs: Vec::new(),
    };

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let out = engine
        .impact_from_symbols(&changed, dimpact::LanguageMode::Rust, &opts)
        .unwrap();
    std::env::set_current_dir(cwd).unwrap();

    assert!(out.impacted_symbols.iter().any(|s| s.name == "foo"));
}

#[test]
#[serial]
fn lsp_engine_strict_callers_chain_is_stable_when_available() {
    if !should_run_strict_lsp_e2e() {
        eprintln!("skip: set DIMPACT_E2E_STRICT_LSP=1 to run strict LSP e2e tests");
        return;
    }
    if !has_rust_analyzer() {
        eprintln!("skip: rust-analyzer not available");
        return;
    }

    let initial = "fn bar() {}\nfn foo() { bar(); }\nfn main() { foo(); }\n";
    let updated = "fn bar() { let _x = 1; }\nfn foo() { bar(); }\nfn main() { foo(); }\n";
    let (_tmp, repo) = setup_repo_rust_project(initial, updated);

    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();
    let files = dimpact::parse_unified_diff(&diff).unwrap();

    let cfg = dimpact::EngineConfig {
        lsp_strict: true,
        dump_capabilities: false,
        mock_lsp: false,
        mock_caps: None,
    };
    let engine = dimpact::engine::make_engine(dimpact::EngineKind::Lsp, cfg);
    let opts = dimpact::ImpactOptions {
        direction: dimpact::ImpactDirection::Callers,
        max_depth: Some(5),
        with_edges: Some(false),
        ignore_dirs: Vec::new(),
    };

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let out1 = match engine.impact(&files, dimpact::LanguageMode::Rust, &opts) {
        Ok(v) => v,
        Err(e) => {
            std::env::set_current_dir(cwd).unwrap();
            eprintln!("skip: strict LSP unavailable in this environment: {e}");
            return;
        }
    };
    let out2 = engine
        .impact(&files, dimpact::LanguageMode::Rust, &opts)
        .expect("second strict LSP run should succeed");
    std::env::set_current_dir(cwd).unwrap();

    let names1 = impacted_name_set(&out1);
    let names2 = impacted_name_set(&out2);
    assert!(names1.contains("foo"));
    assert!(names1.contains("main"));
    assert_eq!(names1, names2, "strict LSP result should be stable");
}

#[test]
#[serial]
fn lsp_engine_strict_methods_chain_resolves_callers_when_available() {
    if !should_run_strict_lsp_e2e() {
        eprintln!("skip: set DIMPACT_E2E_STRICT_LSP=1 to run strict LSP e2e tests");
        return;
    }
    if !has_rust_analyzer() {
        eprintln!("skip: rust-analyzer not available");
        return;
    }

    let initial = r#"
struct S;
impl S {
    fn bar(&self) {}
    fn foo(&self) { self.bar(); }
}
fn main() { let s = S; s.foo(); }
"#;
    let updated = r#"
struct S;
impl S {
    fn bar(&self) { let _x = 1; }
    fn foo(&self) { self.bar(); }
}
fn main() { let s = S; s.foo(); }
"#;
    let (_tmp, repo) = setup_repo_rust_project(initial, updated);

    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();
    let files = dimpact::parse_unified_diff(&diff).unwrap();

    let cfg = dimpact::EngineConfig {
        lsp_strict: true,
        dump_capabilities: false,
        mock_lsp: false,
        mock_caps: None,
    };
    let engine = dimpact::engine::make_engine(dimpact::EngineKind::Lsp, cfg);
    let opts = dimpact::ImpactOptions {
        direction: dimpact::ImpactDirection::Callers,
        max_depth: Some(5),
        with_edges: Some(false),
        ignore_dirs: Vec::new(),
    };

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let out = match engine.impact(&files, dimpact::LanguageMode::Rust, &opts) {
        Ok(v) => v,
        Err(e) => {
            std::env::set_current_dir(cwd).unwrap();
            eprintln!("skip: strict LSP unavailable in this environment: {e}");
            return;
        }
    };
    std::env::set_current_dir(cwd).unwrap();

    let names = impacted_name_set(&out);
    assert!(names.contains("foo"), "method caller should be detected");
}
