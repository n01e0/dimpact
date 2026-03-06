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
    std::env::var("DIMPACT_E2E_STRICT_LSP").ok().as_deref() == Some("1")
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
    out.impacted_symbols
        .iter()
        .map(|s| s.name.clone())
        .collect()
}

fn setup_repo_single_file(
    filename: &str,
    initial_src: &str,
    updated_src: &str,
) -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    fs::write(path.join(filename), initial_src).unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    fs::write(path.join(filename), updated_src).unwrap();
    (dir, path)
}

fn setup_repo_ruby_both_chain_fixture() -> (TempDir, std::path::PathBuf) {
    let initial = "def bar\nend\n\ndef foo\n  bar\nend\n\ndef main\n  foo\nend\n";
    let updated = "def bar\nend\n\ndef foo\n  x = 1\n  bar\n  x\nend\n\ndef main\n  foo\nend\n";
    setup_repo_single_file("main.rb", initial, updated)
}

fn setup_repo_python_both_chain_fixture() -> (TempDir, std::path::PathBuf) {
    let initial = "def bar():\n    return 1\n\ndef foo():\n    return bar()\n\ndef main():\n    return foo()\n";
    let updated = "def bar():\n    return 1\n\ndef foo():\n    x = 1\n    return bar() + x\n\ndef main():\n    return foo()\n";
    setup_repo_single_file("main.py", initial, updated)
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
fn lsp_engine_strict_mock_typescript_callers_chain() {
    let initial = "function bar() {}\nfunction foo() { bar(); }\n";
    let updated = "function bar() { const x = 1; return x; }\nfunction foo() { bar(); }\n";
    let (_tmp, repo) = setup_repo_single_file("main.ts", initial, updated);

    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();
    let files = dimpact::parse_unified_diff(&diff).unwrap();

    let cfg = dimpact::EngineConfig {
        lsp_strict: true,
        dump_capabilities: false,
        mock_lsp: true,
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
    let changed = engine
        .changed_symbols(&files, dimpact::LanguageMode::Typescript)
        .unwrap();
    let out = engine
        .impact(&files, dimpact::LanguageMode::Typescript, &opts)
        .unwrap();
    std::env::set_current_dir(cwd).unwrap();

    assert!(changed.changed_symbols.iter().any(|s| s.name == "bar"));
    assert!(out.impacted_symbols.iter().any(|s| s.name == "foo"));
}

#[test]
#[serial]
fn lsp_engine_strict_mock_tsx_callers_chain() {
    let initial = "function bar() { return 1; }\nfunction foo() { return bar(); }\nexport default function App() { return <div>{foo()}</div>; }\n";
    let updated = "function bar() { return 2; }\nfunction foo() { return bar(); }\nexport default function App() { return <div>{foo()}</div>; }\n";
    let (_tmp, repo) = setup_repo_single_file("app.tsx", initial, updated);

    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();
    let files = dimpact::parse_unified_diff(&diff).unwrap();

    let cfg = dimpact::EngineConfig {
        lsp_strict: true,
        dump_capabilities: false,
        mock_lsp: true,
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
    let changed = engine
        .changed_symbols(&files, dimpact::LanguageMode::Tsx)
        .unwrap();
    let out = engine
        .impact(&files, dimpact::LanguageMode::Tsx, &opts)
        .unwrap();
    std::env::set_current_dir(cwd).unwrap();

    assert!(changed.changed_symbols.iter().any(|s| s.name == "bar"));
    assert!(out.impacted_symbols.iter().any(|s| s.name == "foo"));
}

#[test]
#[serial]
fn lsp_engine_strict_mock_typescript_callees_chain() {
    let initial = "function bar() { return 1; }\nfunction baz() { return 2; }\nfunction foo() { return bar() + baz(); }\n";
    let updated = "function bar() { return 1; }\nfunction baz() { return 2; }\nfunction foo() { const x = 1; return bar() + baz() + x; }\n";
    let (_tmp, repo) = setup_repo_single_file("main.ts", initial, updated);

    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();
    let files = dimpact::parse_unified_diff(&diff).unwrap();

    let cfg = dimpact::EngineConfig {
        lsp_strict: true,
        dump_capabilities: false,
        mock_lsp: true,
        mock_caps: None,
    };
    let engine = dimpact::engine::make_engine(dimpact::EngineKind::Lsp, cfg);
    let opts = dimpact::ImpactOptions {
        direction: dimpact::ImpactDirection::Callees,
        max_depth: Some(5),
        with_edges: Some(false),
        ignore_dirs: Vec::new(),
    };

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let changed = engine
        .changed_symbols(&files, dimpact::LanguageMode::Typescript)
        .unwrap();
    let out = engine
        .impact(&files, dimpact::LanguageMode::Typescript, &opts)
        .unwrap();
    std::env::set_current_dir(cwd).unwrap();

    assert!(changed.changed_symbols.iter().any(|s| s.name == "foo"));
    assert!(out.impacted_symbols.iter().any(|s| s.name == "bar"));
    assert!(out.impacted_symbols.iter().any(|s| s.name == "baz"));
}

#[test]
#[serial]
fn lsp_engine_strict_mock_javascript_callers_chain() {
    let initial = "function bar() {}\nfunction foo() { bar(); }\n";
    let updated = "function bar() { const x = 1; return x; }\nfunction foo() { bar(); }\n";
    let (_tmp, repo) = setup_repo_single_file("main.js", initial, updated);

    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();
    let files = dimpact::parse_unified_diff(&diff).unwrap();

    let cfg = dimpact::EngineConfig {
        lsp_strict: true,
        dump_capabilities: false,
        mock_lsp: true,
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
    let changed = engine
        .changed_symbols(&files, dimpact::LanguageMode::Javascript)
        .unwrap();
    let out = engine
        .impact(&files, dimpact::LanguageMode::Javascript, &opts)
        .unwrap();
    std::env::set_current_dir(cwd).unwrap();

    assert!(changed.changed_symbols.iter().any(|s| s.name == "bar"));
    assert!(out.impacted_symbols.iter().any(|s| s.name == "foo"));
}

#[test]
#[serial]
fn lsp_engine_strict_mock_javascript_both_chain() {
    let initial = "function bar() { return 1; }\nfunction foo() { return bar(); }\nfunction main() { return foo(); }\n";
    let updated = "function bar() { return 1; }\nfunction foo() { const x = 1; return bar() + x; }\nfunction main() { return foo(); }\n";
    let (_tmp, repo) = setup_repo_single_file("main.js", initial, updated);

    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();
    let files = dimpact::parse_unified_diff(&diff).unwrap();

    let cfg = dimpact::EngineConfig {
        lsp_strict: true,
        dump_capabilities: false,
        mock_lsp: true,
        mock_caps: None,
    };
    let engine = dimpact::engine::make_engine(dimpact::EngineKind::Lsp, cfg);
    let opts = dimpact::ImpactOptions {
        direction: dimpact::ImpactDirection::Both,
        max_depth: Some(5),
        with_edges: Some(false),
        ignore_dirs: Vec::new(),
    };

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let changed = engine
        .changed_symbols(&files, dimpact::LanguageMode::Javascript)
        .unwrap();
    let out = engine
        .impact(&files, dimpact::LanguageMode::Javascript, &opts)
        .unwrap();
    std::env::set_current_dir(cwd).unwrap();

    assert!(changed.changed_symbols.iter().any(|s| s.name == "foo"));
    assert!(out.impacted_symbols.iter().any(|s| s.name == "bar"));
    assert!(out.impacted_symbols.iter().any(|s| s.name == "main"));
}

#[test]
#[serial]
fn lsp_engine_strict_mock_ruby_callers_chain() {
    let initial = "def bar\nend\n\ndef foo\n  bar\nend\n";
    let updated = "def bar\n  x = 1\n  x\nend\n\ndef foo\n  bar\nend\n";
    let (_tmp, repo) = setup_repo_single_file("main.rb", initial, updated);

    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();
    let files = dimpact::parse_unified_diff(&diff).unwrap();

    let cfg = dimpact::EngineConfig {
        lsp_strict: true,
        dump_capabilities: false,
        mock_lsp: true,
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
    let changed = engine
        .changed_symbols(&files, dimpact::LanguageMode::Ruby)
        .unwrap();
    let out = engine
        .impact(&files, dimpact::LanguageMode::Ruby, &opts)
        .unwrap();
    std::env::set_current_dir(cwd).unwrap();

    assert!(changed.changed_symbols.iter().any(|s| s.name == "bar"));
    assert!(out.impacted_symbols.iter().any(|s| s.name == "foo"));
}

#[test]
#[serial]
fn lsp_engine_strict_mock_ruby_both_chain() {
    let (_tmp, repo) = setup_repo_ruby_both_chain_fixture();

    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();
    let files = dimpact::parse_unified_diff(&diff).unwrap();

    let cfg = dimpact::EngineConfig {
        lsp_strict: true,
        dump_capabilities: false,
        mock_lsp: true,
        mock_caps: None,
    };
    let engine = dimpact::engine::make_engine(dimpact::EngineKind::Lsp, cfg);
    let opts = dimpact::ImpactOptions {
        direction: dimpact::ImpactDirection::Both,
        max_depth: Some(5),
        with_edges: Some(false),
        ignore_dirs: Vec::new(),
    };

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let changed = engine
        .changed_symbols(&files, dimpact::LanguageMode::Ruby)
        .unwrap();
    let out = engine
        .impact(&files, dimpact::LanguageMode::Ruby, &opts)
        .unwrap();
    std::env::set_current_dir(cwd).unwrap();

    assert!(changed.changed_symbols.iter().any(|s| s.name == "foo"));
    assert!(out.impacted_symbols.iter().any(|s| s.name == "bar"));
    assert!(out.impacted_symbols.iter().any(|s| s.name == "main"));
}

#[test]
#[serial]
fn lsp_engine_strict_mock_ruby_both_chain_with_refs_only_caps() {
    let (_tmp, repo) = setup_repo_ruby_both_chain_fixture();

    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();
    let files = dimpact::parse_unified_diff(&diff).unwrap();

    let caps = CapsHint {
        document_symbol: true,
        workspace_symbol: true,
        call_hierarchy: false,
        references: true,
        definition: true,
    };
    let cfg = dimpact::EngineConfig {
        lsp_strict: true,
        dump_capabilities: false,
        mock_lsp: true,
        mock_caps: Some(caps),
    };
    let engine = dimpact::engine::make_engine(dimpact::EngineKind::Lsp, cfg);
    let opts = dimpact::ImpactOptions {
        direction: dimpact::ImpactDirection::Both,
        max_depth: Some(5),
        with_edges: Some(false),
        ignore_dirs: Vec::new(),
    };

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let changed = engine
        .changed_symbols(&files, dimpact::LanguageMode::Ruby)
        .unwrap();
    let out = engine
        .impact(&files, dimpact::LanguageMode::Ruby, &opts)
        .unwrap();
    std::env::set_current_dir(cwd).unwrap();

    assert!(changed.changed_symbols.iter().any(|s| s.name == "foo"));
    assert!(out.impacted_symbols.iter().any(|s| s.name == "bar"));
    assert!(out.impacted_symbols.iter().any(|s| s.name == "main"));
}

#[test]
#[serial]
fn lsp_engine_strict_mock_ruby_callees_chain_with_refs_only_caps() {
    let (_tmp, repo) = setup_repo_ruby_both_chain_fixture();

    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();
    let files = dimpact::parse_unified_diff(&diff).unwrap();

    let caps = CapsHint {
        document_symbol: true,
        workspace_symbol: true,
        call_hierarchy: false,
        references: true,
        definition: true,
    };
    let cfg = dimpact::EngineConfig {
        lsp_strict: true,
        dump_capabilities: false,
        mock_lsp: true,
        mock_caps: Some(caps),
    };
    let engine = dimpact::engine::make_engine(dimpact::EngineKind::Lsp, cfg);
    let opts = dimpact::ImpactOptions {
        direction: dimpact::ImpactDirection::Callees,
        max_depth: Some(5),
        with_edges: Some(false),
        ignore_dirs: Vec::new(),
    };

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let changed = engine
        .changed_symbols(&files, dimpact::LanguageMode::Ruby)
        .unwrap();
    let out = engine
        .impact(&files, dimpact::LanguageMode::Ruby, &opts)
        .unwrap();
    std::env::set_current_dir(cwd).unwrap();

    assert!(changed.changed_symbols.iter().any(|s| s.name == "foo"));
    assert!(out.impacted_symbols.iter().any(|s| s.name == "bar"));
    assert!(!out.impacted_symbols.iter().any(|s| s.name == "main"));
}

#[test]
#[serial]
fn lsp_engine_strict_mock_ruby_method_callers_chain() {
    let initial = "class S\n  def bar\n  end\n\n  def foo\n    self.bar\n  end\nend\n";
    let updated =
        "class S\n  def bar\n    x = 1\n    x\n  end\n\n  def foo\n    self.bar\n  end\nend\n";
    let (_tmp, repo) = setup_repo_single_file("main.rb", initial, updated);

    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();
    let files = dimpact::parse_unified_diff(&diff).unwrap();

    let cfg = dimpact::EngineConfig {
        lsp_strict: true,
        dump_capabilities: false,
        mock_lsp: true,
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
    let changed = engine
        .changed_symbols(&files, dimpact::LanguageMode::Ruby)
        .unwrap();
    let out = engine
        .impact(&files, dimpact::LanguageMode::Ruby, &opts)
        .unwrap();
    std::env::set_current_dir(cwd).unwrap();

    assert!(changed.changed_symbols.iter().any(|s| s.name == "bar"));
    assert!(out.impacted_symbols.iter().any(|s| s.name == "foo"));
}

#[test]
#[serial]
fn python_fixture_for_changed_flow_is_prepared() {
    let (_tmp, repo) = setup_repo_python_both_chain_fixture();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();
    let files = dimpact::parse_unified_diff(&diff).unwrap();

    assert_eq!(files.len(), 1);
    let f = &files[0];
    assert_eq!(f.new_path.as_deref(), Some("main.py"));
    assert!(
        f.changes
            .iter()
            .any(|c| matches!(c.kind, dimpact::ChangeKind::Added)),
        "fixture should include added lines for changed detection"
    );
}

#[test]
#[serial]
fn python_fixture_for_impact_flow_has_call_chain() {
    let (_tmp, repo) = setup_repo_python_both_chain_fixture();
    let src = fs::read_to_string(repo.join("main.py")).expect("read fixture source");

    let spec = dimpact::ts_core::load_python_spec();
    let compiled = dimpact::ts_core::compile_queries_python(&spec).expect("compile py queries");
    let runner = dimpact::ts_core::QueryRunner::new_python();

    let decls = runner.run_captures(&src, &compiled.decl);
    let calls = runner.run_captures(&src, &compiled.calls);

    let mut decl_names = BTreeSet::new();
    for caps in &decls {
        if let Some(name_cap) = caps.iter().find(|c| c.name == "name") {
            decl_names.insert(src[name_cap.start..name_cap.end].to_string());
        }
    }

    let mut call_names = BTreeSet::new();
    for caps in &calls {
        if let Some(name_cap) = caps.iter().find(|c| c.name == "name") {
            call_names.insert(src[name_cap.start..name_cap.end].to_string());
        }
    }

    assert!(decl_names.contains("bar"));
    assert!(decl_names.contains("foo"));
    assert!(decl_names.contains("main"));
    assert!(call_names.contains("bar"));
    assert!(call_names.contains("foo"));
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
