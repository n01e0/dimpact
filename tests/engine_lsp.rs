use dimpact::engine::CapsHint;
use serial_test::serial;
use std::collections::BTreeSet;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

struct ScopedEnvVar {
    key: &'static str,
    prev: Option<String>,
}

impl ScopedEnvVar {
    fn set(key: &'static str, value: &str) -> Self {
        let prev = std::env::var(key).ok();
        // SAFETY: tests using this helper are serial and restore previous value in Drop.
        unsafe {
            std::env::set_var(key, value);
        }
        Self { key, prev }
    }
}

impl Drop for ScopedEnvVar {
    fn drop(&mut self) {
        // SAFETY: tests using this helper are serial and restore previous value in Drop.
        unsafe {
            if let Some(prev) = &self.prev {
                std::env::set_var(self.key, prev);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }
}

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

fn detect_python_lsp_server_for_tests() -> Option<&'static str> {
    let has = |exe: &str| {
        Command::new(exe)
            .arg("--help")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    };

    if let Ok(preferred) = std::env::var("DIMPACT_PYTHON_LSP") {
        let p = preferred.trim().to_ascii_lowercase();
        let cand = match p.as_str() {
            "pyright" | "pyright-langserver" => "pyright-langserver",
            "basedpyright" | "basedpyright-langserver" => "basedpyright-langserver",
            "pylsp" => "pylsp",
            _ => "",
        };
        if !cand.is_empty() && has(cand) {
            return Some(cand);
        }
    }

    if has("pyright-langserver") {
        return Some("pyright-langserver");
    }
    if has("basedpyright-langserver") {
        return Some("basedpyright-langserver");
    }
    if has("pylsp") {
        return Some("pylsp");
    }
    None
}

fn require_python_lsp_server_for_lane(lane: &str) {
    if let Some(server) = detect_python_lsp_server_for_tests() {
        eprintln!(
            "fail-fast preflight: lane={} python_lsp_selected={} (selection logic preserved)",
            lane, server
        );
    } else {
        panic!(
            "fail-fast preflight: lane={} cause=server python LSP server not found (pyright-langserver/basedpyright-langserver/pylsp)",
            lane
        );
    }
}

fn has_gopls() -> bool {
    Command::new("gopls")
        .arg("version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn has_jdtls() -> bool {
    Command::new("jdtls")
        .arg("--help")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn require_gopls_for_lane(lane: &str) {
    if !has_gopls() {
        panic!(
            "fail-fast preflight: lane={} cause=server gopls not found",
            lane
        );
    }
}

fn require_jdtls_for_lane(lane: &str) {
    if !has_jdtls() {
        panic!(
            "fail-fast preflight: lane={} cause=server jdtls not found",
            lane
        );
    }
}

fn has_typescript_lsp_server() -> bool {
    Command::new("typescript-language-server")
        .arg("--help")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn require_typescript_lsp_server_for_lane(lane: &str) {
    if !has_typescript_lsp_server() {
        panic!(
            "fail-fast preflight: lane={} cause=server typescript-language-server not found",
            lane
        );
    }
}

fn has_ruby_lsp_server() -> bool {
    Command::new("ruby-lsp")
        .arg("--help")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn require_ruby_lsp_server_for_lane(lane: &str) {
    if !has_ruby_lsp_server() {
        panic!(
            "fail-fast preflight: lane={} cause=server ruby-lsp not found",
            lane
        );
    }
}

fn should_run_strict_lsp_e2e() -> bool {
    std::env::var("DIMPACT_E2E_STRICT_LSP").ok().as_deref() == Some("1")
}

fn require_strict_lsp_env_gate_for_lane(lane: &str) -> bool {
    match std::env::var("DIMPACT_E2E_STRICT_LSP") {
        Ok(v) if v == "1" => true,
        Ok(v) if v == "0" => {
            eprintln!("skip: lane={} has DIMPACT_E2E_STRICT_LSP=0", lane);
            false
        }
        Ok(v) => {
            panic!(
                "fail-fast preflight: lane={} cause=env invalid DIMPACT_E2E_STRICT_LSP={} (expected 1 or 0)",
                lane, v
            );
        }
        Err(_) => {
            eprintln!("skip: set DIMPACT_E2E_STRICT_LSP=1 to run strict LSP e2e tests");
            false
        }
    }
}

fn require_env_gate_callers_lane(lane: &str, lane_var: &str, hint: &str) -> bool {
    match std::env::var("DIMPACT_E2E_STRICT_LSP") {
        Ok(v) if v == "1" => return true,
        Ok(v) if v == "0" => {}
        Ok(v) => {
            panic!(
                "fail-fast preflight: lane={} cause=env invalid DIMPACT_E2E_STRICT_LSP={} (expected 1 or 0)",
                lane, v
            );
        }
        Err(_) => {}
    }

    match std::env::var(lane_var) {
        Ok(v) if v == "1" => true,
        Ok(v) => {
            panic!(
                "fail-fast preflight: lane={} cause=env invalid {}={} (expected 1; to skip leave unset)",
                lane, lane_var, v
            );
        }
        Err(_) => {
            eprintln!("skip: set {} to run strict LSP e2e tests", hint);
            false
        }
    }
}

fn should_run_typescript_strict_lsp_e2e() -> bool {
    std::env::var("DIMPACT_E2E_STRICT_LSP_TYPESCRIPT")
        .ok()
        .as_deref()
        == Some("1")
        || should_run_strict_lsp_e2e()
}

fn should_run_javascript_strict_lsp_e2e() -> bool {
    std::env::var("DIMPACT_E2E_STRICT_LSP_JAVASCRIPT")
        .ok()
        .as_deref()
        == Some("1")
        || should_run_strict_lsp_e2e()
}

fn should_run_tsx_strict_lsp_e2e() -> bool {
    std::env::var("DIMPACT_E2E_STRICT_LSP_TSX").ok().as_deref() == Some("1")
        || should_run_strict_lsp_e2e()
}

fn should_run_ruby_strict_lsp_e2e() -> bool {
    std::env::var("DIMPACT_E2E_STRICT_LSP_RUBY").ok().as_deref() == Some("1")
        || should_run_strict_lsp_e2e()
}

fn should_run_python_strict_lsp_e2e() -> bool {
    std::env::var("DIMPACT_E2E_STRICT_LSP_PYTHON")
        .ok()
        .as_deref()
        == Some("1")
        || should_run_strict_lsp_e2e()
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

fn classify_fail_fast_cause(err: &str) -> &'static str {
    let m = err.to_ascii_lowercase();
    if m.contains("not found")
        || m.contains("no such file")
        || m.contains("failed to spawn")
        || m.contains("initialize timeout")
        || m.contains("server")
    {
        "server"
    } else if m.contains("capability")
        || m.contains("documentsymbol")
        || m.contains("workspace symbol")
        || m.contains("callhierarchy")
        || m.contains("references")
        || m.contains("definition")
    {
        "capability"
    } else {
        "logic"
    }
}

fn fail_fast_with_triage<T>(lane: &str, phase: &str, res: anyhow::Result<T>) -> T {
    match res {
        Ok(v) => v,
        Err(e) => {
            let msg = e.to_string();
            let cause = classify_fail_fast_cause(&msg);
            eprintln!(
                "fail-fast-triage: lane={} phase={} cause={} error={}",
                lane, phase, cause, msg
            );
            panic!(
                "fail-fast: lane={} phase={} cause={} error={}",
                lane, phase, cause, msg
            );
        }
    }
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

fn setup_repo_python_callers_chain_fixture() -> (TempDir, std::path::PathBuf) {
    let initial = "def bar():\n    return 1\n\ndef foo():\n    return bar()\n\ndef main():\n    return foo()\n";
    let updated = "def bar():\n    x = 1\n    return x\n\ndef foo():\n    return bar()\n\ndef main():\n    return foo()\n";
    setup_repo_single_file("main.py", initial, updated)
}

fn setup_repo_python_callees_chain_fixture() -> (TempDir, std::path::PathBuf) {
    let initial = "def bar():\n    return 1\n\ndef baz():\n    return 2\n\ndef foo():\n    return bar() + baz()\n\ndef main():\n    return foo()\n";
    let updated = "def bar():\n    return 1\n\ndef baz():\n    return 2\n\ndef foo():\n    x = 1\n    return bar() + baz() + x\n\ndef main():\n    return foo()\n";
    setup_repo_single_file("main.py", initial, updated)
}

fn setup_repo_go_callers_chain_fixture() -> (TempDir, std::path::PathBuf) {
    let initial = "package main\n\nfunc bar() {}\n\nfunc foo() {\n    bar()\n}\n\nfunc main() {\n    foo()\n}\n";
    let updated = "package main\n\nfunc bar() {\n    x := 1\n    _ = x\n}\n\nfunc foo() {\n    bar()\n}\n\nfunc main() {\n    foo()\n}\n";
    setup_repo_single_file("main.go", initial, updated)
}

fn setup_repo_java_callers_chain_fixture() -> (TempDir, std::path::PathBuf) {
    let initial = "class Main {\n    static void bar() {}\n\n    static void foo() {\n        bar();\n    }\n\n    static void entry() {\n        foo();\n    }\n}\n";
    let updated = "class Main {\n    static void bar() {\n        int x = 1;\n    }\n\n    static void foo() {\n        bar();\n    }\n\n    static void entry() {\n        foo();\n    }\n}\n";
    setup_repo_single_file("Main.java", initial, updated)
}

fn setup_repo_java_callees_chain_fixture() -> (TempDir, std::path::PathBuf) {
    let initial = "class Main {\n    static void c() {}\n\n    static void b() {\n        c();\n    }\n\n    static void entry() {\n        b();\n    }\n}\n";
    let updated = "class Main {\n    static void c() {}\n\n    static void b() {\n        int x = 1;\n        c();\n    }\n\n    static void entry() {\n        b();\n    }\n}\n";
    setup_repo_single_file("Main.java", initial, updated)
}

fn setup_repo_java_both_chain_fixture() -> (TempDir, std::path::PathBuf) {
    let initial = "class Main {\n    static void bar() {}\n\n    static void foo() {\n        bar();\n    }\n\n    static void entry() {\n        foo();\n    }\n}\n";
    let updated = "class Main {\n    static void bar() {}\n\n    static void foo() {\n        int x = 1;\n        bar();\n    }\n\n    static void entry() {\n        foo();\n    }\n}\n";
    setup_repo_single_file("Main.java", initial, updated)
}

fn setup_repo_go_callees_chain_fixture() -> (TempDir, std::path::PathBuf) {
    let initial =
        "package main\n\nfunc c() {}\n\nfunc b() {\n    c()\n}\n\nfunc a() {\n    b()\n}\n";
    let updated = "package main\n\nfunc c() {}\n\nfunc b() {\n    x := 1\n    _ = x\n    c()\n}\n\nfunc a() {\n    b()\n}\n";
    setup_repo_single_file("main.go", initial, updated)
}

fn setup_repo_go_both_chain_fixture() -> (TempDir, std::path::PathBuf) {
    let initial = "package main\n\nfunc bar() {}\n\nfunc foo() {\n    bar()\n}\n\nfunc main() {\n    foo()\n}\n";
    let updated = "package main\n\nfunc bar() {}\n\nfunc foo() {\n    x := 1\n    _ = x\n    bar()\n}\n\nfunc main() {\n    foo()\n}\n";
    setup_repo_single_file("main.go", initial, updated)
}

fn setup_repo_go_real_lsp_e2e_fixture() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    fs::write(path.join("go.mod"), "module lsp-go-fixture\n\ngo 1.22\n").unwrap();
    fs::write(
        path.join("main.go"),
        "package main\n\nfunc bar() int {\n\treturn 1\n}\n\nfunc foo() int {\n\treturn bar()\n}\n\nfunc main() {\n\t_ = foo()\n}\n",
    )
    .unwrap();

    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    fs::write(
        path.join("main.go"),
        "package main\n\nfunc bar() int {\n\tx := 1\n\treturn x\n}\n\nfunc foo() int {\n\treturn bar()\n}\n\nfunc main() {\n\t_ = foo()\n}\n",
    )
    .unwrap();

    (dir, path)
}

fn setup_repo_java_real_lsp_e2e_fixture() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    fs::create_dir_all(path.join("src/main/java")).unwrap();
    fs::write(
        path.join("pom.xml"),
        "<project xmlns=\"http://maven.apache.org/POM/4.0.0\"\n         xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"\n         xsi:schemaLocation=\"http://maven.apache.org/POM/4.0.0 https://maven.apache.org/xsd/maven-4.0.0.xsd\">\n  <modelVersion>4.0.0</modelVersion>\n  <groupId>dev.dimpact</groupId>\n  <artifactId>lsp-java-fixture</artifactId>\n  <version>0.1.0</version>\n  <properties>\n    <maven.compiler.source>17</maven.compiler.source>\n    <maven.compiler.target>17</maven.compiler.target>\n  </properties>\n</project>\n",
    )
    .unwrap();
    fs::write(
        path.join("src/main/java/Main.java"),
        "class Main {\n    static int bar() {\n        return 1;\n    }\n\n    static int foo() {\n        return bar();\n    }\n\n    public static void main(String[] args) {\n        foo();\n    }\n}\n",
    )
    .unwrap();

    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    fs::write(
        path.join("src/main/java/Main.java"),
        "class Main {\n    static int bar() {\n        int x = 1;\n        return x;\n    }\n\n    static int foo() {\n        return bar();\n    }\n\n    public static void main(String[] args) {\n        foo();\n    }\n}\n",
    )
    .unwrap();

    (dir, path)
}

fn setup_repo_typescript_real_lsp_e2e_fixture() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    fs::write(
        path.join("package.json"),
        "{\n  \"name\": \"lsp-typescript-fixture\",\n  \"private\": true,\n  \"version\": \"0.1.0\"\n}\n",
    )
    .unwrap();
    fs::write(
        path.join("tsconfig.json"),
        "{\n  \"compilerOptions\": {\n    \"target\": \"ES2020\",\n    \"module\": \"commonjs\",\n    \"strict\": true\n  }\n}\n",
    )
    .unwrap();
    fs::write(
        path.join("main.ts"),
        "function bar(): number {\n  return 1;\n}\n\nfunction foo(): number {\n  return bar();\n}\n\nfunction main(): number {\n  return foo();\n}\n",
    )
    .unwrap();

    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    fs::write(
        path.join("main.ts"),
        "function bar(): number {\n  const x = 1;\n  return x;\n}\n\nfunction foo(): number {\n  return bar();\n}\n\nfunction main(): number {\n  return foo();\n}\n",
    )
    .unwrap();

    (dir, path)
}

fn setup_repo_tsx_real_lsp_e2e_fixture() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    fs::write(
        path.join("package.json"),
        "{\n  \"name\": \"lsp-tsx-fixture\",\n  \"private\": true,\n  \"version\": \"0.1.0\"\n}\n",
    )
    .unwrap();
    fs::write(
        path.join("tsconfig.json"),
        "{\n  \"compilerOptions\": {\n    \"target\": \"ES2020\",\n    \"module\": \"commonjs\",\n    \"jsx\": \"react-jsx\",\n    \"strict\": true\n  }\n}\n",
    )
    .unwrap();
    fs::write(
        path.join("app.tsx"),
        "function bar() {\n  return 1;\n}\n\nfunction View() {\n  return <div>{bar()}</div>;\n}\n\nexport function App() {\n  return <View />;\n}\n",
    )
    .unwrap();

    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    fs::write(
        path.join("app.tsx"),
        "function bar() {\n  const x = 1;\n  return x;\n}\n\nfunction View() {\n  return <div>{bar()}</div>;\n}\n\nexport function App() {\n  return <View />;\n}\n",
    )
    .unwrap();

    (dir, path)
}

fn setup_repo_tsx_callees_real_lsp_e2e_fixture() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    fs::write(
        path.join("package.json"),
        "{\n  \"name\": \"lsp-tsx-callees-fixture\",\n  \"private\": true,\n  \"version\": \"0.1.0\"\n}\n",
    )
    .unwrap();
    fs::write(
        path.join("tsconfig.json"),
        "{\n  \"compilerOptions\": {\n    \"target\": \"ES2020\",\n    \"module\": \"commonjs\",\n    \"jsx\": \"react-jsx\",\n    \"strict\": true\n  }\n}\n",
    )
    .unwrap();
    fs::write(
        path.join("app.tsx"),
        "function bar(): number {\n  return 1;\n}\n\nfunction baz(): number {\n  return 2;\n}\n\nfunction View(): JSX.Element {\n  return <div>{bar() + baz()}</div>;\n}\n\nexport function App(): JSX.Element {\n  return <View />;\n}\n",
    )
    .unwrap();

    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    fs::write(
        path.join("app.tsx"),
        "function bar(): number {\n  return 1;\n}\n\nfunction baz(): number {\n  return 2;\n}\n\nfunction View(): JSX.Element {\n  const x = 1;\n  return <div>{bar() + baz() + x}</div>;\n}\n\nexport function App(): JSX.Element {\n  return <View />;\n}\n",
    )
    .unwrap();

    (dir, path)
}

fn setup_repo_typescript_callees_real_lsp_e2e_fixture() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    fs::write(
        path.join("package.json"),
        "{\n  \"name\": \"lsp-typescript-callees-fixture\",\n  \"private\": true,\n  \"version\": \"0.1.0\"\n}\n",
    )
    .unwrap();
    fs::write(
        path.join("tsconfig.json"),
        "{\n  \"compilerOptions\": {\n    \"target\": \"ES2020\",\n    \"module\": \"commonjs\",\n    \"strict\": true\n  }\n}\n",
    )
    .unwrap();
    fs::write(
        path.join("main.ts"),
        "function bar(): number {\n  return 1;\n}\n\nfunction baz(): number {\n  return 2;\n}\n\nfunction foo(): number {\n  return bar() + baz();\n}\n\nfunction main(): number {\n  return foo();\n}\n",
    )
    .unwrap();

    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    fs::write(
        path.join("main.ts"),
        "function bar(): number {\n  return 1;\n}\n\nfunction baz(): number {\n  return 2;\n}\n\nfunction foo(): number {\n  const x = 1;\n  return bar() + baz() + x;\n}\n\nfunction main(): number {\n  return foo();\n}\n",
    )
    .unwrap();

    (dir, path)
}

fn setup_repo_javascript_real_lsp_e2e_fixture() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    fs::write(
        path.join("package.json"),
        "{\n  \"name\": \"lsp-javascript-fixture\",\n  \"private\": true,\n  \"version\": \"0.1.0\"\n}\n",
    )
    .unwrap();
    fs::write(
        path.join("main.js"),
        "function bar() {\n  return 1;\n}\n\nfunction foo() {\n  return bar();\n}\n\nfunction main() {\n  return foo();\n}\n",
    )
    .unwrap();

    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    fs::write(
        path.join("main.js"),
        "function bar() {\n  const x = 1;\n  return x;\n}\n\nfunction foo() {\n  return bar();\n}\n\nfunction main() {\n  return foo();\n}\n",
    )
    .unwrap();

    (dir, path)
}

fn setup_repo_javascript_callees_real_lsp_e2e_fixture() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    fs::write(
        path.join("package.json"),
        "{\n  \"name\": \"lsp-javascript-callees-fixture\",\n  \"private\": true,\n  \"version\": \"0.1.0\"\n}\n",
    )
    .unwrap();
    fs::write(
        path.join("main.js"),
        "function bar() {\n  return 1;\n}\n\nfunction baz() {\n  return 2;\n}\n\nfunction foo() {\n  return bar() + baz();\n}\n\nfunction main() {\n  return foo();\n}\n",
    )
    .unwrap();

    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    fs::write(
        path.join("main.js"),
        "function bar() {\n  return 1;\n}\n\nfunction baz() {\n  return 2;\n}\n\nfunction foo() {\n  const x = 1;\n  return bar() + baz() + x;\n}\n\nfunction main() {\n  return foo();\n}\n",
    )
    .unwrap();

    (dir, path)
}

fn setup_repo_ruby_real_lsp_e2e_fixture() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    fs::write(path.join("Gemfile"), "source \"https://rubygems.org\"\n").unwrap();
    fs::write(
        path.join("main.rb"),
        "def bar\n  1\nend\n\ndef foo\n  bar\nend\n\ndef main\n  foo\nend\n",
    )
    .unwrap();

    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    fs::write(
        path.join("main.rb"),
        "def bar\n  x = 1\n  x\nend\n\ndef foo\n  bar\nend\n\ndef main\n  foo\nend\n",
    )
    .unwrap();

    (dir, path)
}

fn setup_repo_ruby_callees_real_lsp_e2e_fixture() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    fs::write(path.join("Gemfile"), "source \"https://rubygems.org\"\n").unwrap();
    fs::write(
        path.join("main.rb"),
        "def bar\n  1\nend\n\ndef baz\n  2\nend\n\ndef foo\n  bar + baz\nend\n\ndef main\n  foo\nend\n",
    )
    .unwrap();

    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    fs::write(
        path.join("main.rb"),
        "def bar\n  1\nend\n\ndef baz\n  2\nend\n\ndef foo\n  x = 1\n  bar + baz + x\nend\n\ndef main\n  foo\nend\n",
    )
    .unwrap();

    (dir, path)
}

fn setup_repo_python_real_lsp_e2e_fixture() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().to_path_buf();
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "tester@example.com"]);
    git(&path, &["config", "user.name", "Tester"]);

    fs::write(
        path.join("pyproject.toml"),
        "[project]\nname = \"lsp-python-fixture\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        path.join("main.py"),
        "def bar():\n    return 1\n\ndef foo():\n    return bar()\n\ndef main():\n    return foo()\n",
    )
    .unwrap();

    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "init", "-q"]);

    fs::write(
        path.join("main.py"),
        "def bar():\n    x = 1\n    return x\n\ndef foo():\n    return bar()\n\ndef main():\n    return foo()\n",
    )
    .unwrap();

    (dir, path)
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
    let _guard = ScopedEnvVar::set("DIMPACT_DISABLE_REAL_LSP", "1");
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
        .expect_err("strict + no symbol caps should error");
    std::env::set_current_dir(cwd).unwrap();

    let msg = err.to_string();
    assert!(msg.contains("changed_symbols capability missing"));
    assert!(msg.contains("language=Rust"));
    assert!(msg.contains("required=document_symbol or workspace_symbol"));
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
        .expect_err("strict + no impact caps should error");
    std::env::set_current_dir(cwd).unwrap();

    let msg = err.to_string();
    assert!(msg.contains("impact capability missing"));
    assert!(msg.contains("language=Rust"));
    assert!(msg.contains("direction=Callers"));
    assert!(msg.contains("required=call_hierarchy or (references/definition)"));
}

#[test]
#[serial]
fn lsp_engine_strict_impact_from_symbols_errors_when_caps_missing() {
    let (_tmp, repo) = setup_repo_basic();

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
        direction: dimpact::ImpactDirection::Callees,
        max_depth: Some(3),
        with_edges: Some(false),
        ignore_dirs: Vec::new(),
    };

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let err = engine
        .impact_from_symbols(&changed, dimpact::LanguageMode::Rust, &opts)
        .expect_err("strict + no impact_from_symbols caps should error");
    std::env::set_current_dir(cwd).unwrap();

    let msg = err.to_string();
    assert!(msg.contains("impact_from_symbols capability missing"));
    assert!(msg.contains("language=Rust"));
    assert!(msg.contains("direction=Callees"));
    assert!(msg.contains("required=call_hierarchy or (definition/references)"));
}

#[test]
#[serial]
fn lsp_engine_non_strict_impact_from_symbols_falls_back_when_lsp_unavailable() {
    // Ensure this path exercises fallback from LSP session init failure
    let _guard = ScopedEnvVar::set("DIMPACT_DISABLE_REAL_LSP", "1");
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
fn lsp_engine_strict_mock_go_callers_fixture_runs() {
    let (_tmp, repo) = setup_repo_go_callers_chain_fixture();

    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();
    let files = dimpact::parse_unified_diff(&diff).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].new_path.as_deref(), Some("main.go"));

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
    let changed = vec![dimpact::Symbol {
        id: dimpact::SymbolId::new("go", "main.go", &dimpact::SymbolKind::Function, "bar", 3),
        name: "bar".to_string(),
        kind: dimpact::SymbolKind::Function,
        file: "main.go".to_string(),
        range: dimpact::TextRange {
            start_line: 3,
            end_line: 6,
        },
        language: "go".to_string(),
    }];

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let out1 = engine
        .impact_from_symbols(&changed, dimpact::LanguageMode::Auto, &opts)
        .unwrap();
    let out2 = engine
        .impact_from_symbols(&changed, dimpact::LanguageMode::Auto, &opts)
        .unwrap();
    std::env::set_current_dir(cwd).unwrap();

    let changed1: BTreeSet<String> = out1
        .changed_symbols
        .iter()
        .map(|s| s.name.clone())
        .collect();
    let changed2: BTreeSet<String> = out2
        .changed_symbols
        .iter()
        .map(|s| s.name.clone())
        .collect();
    let impacted1 = impacted_name_set(&out1);
    let impacted2 = impacted_name_set(&out2);

    assert_eq!(changed1, BTreeSet::from(["bar".to_string()]));
    assert_eq!(changed1, changed2, "changed_symbols should be stable");
    assert_eq!(impacted1, impacted2, "impacted_symbols should be stable");
    assert!(impacted1.is_empty());
}

#[test]
#[serial]
fn lsp_engine_strict_mock_java_callers_fixture_runs() {
    let (_tmp, repo) = setup_repo_java_callers_chain_fixture();

    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();
    let files = dimpact::parse_unified_diff(&diff).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].new_path.as_deref(), Some("Main.java"));

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
    let changed = vec![dimpact::Symbol {
        id: dimpact::SymbolId::new("java", "Main.java", &dimpact::SymbolKind::Method, "bar", 2),
        name: "bar".to_string(),
        kind: dimpact::SymbolKind::Method,
        file: "Main.java".to_string(),
        range: dimpact::TextRange {
            start_line: 2,
            end_line: 4,
        },
        language: "java".to_string(),
    }];

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let out1 = engine
        .impact_from_symbols(&changed, dimpact::LanguageMode::Auto, &opts)
        .unwrap();
    let out2 = engine
        .impact_from_symbols(&changed, dimpact::LanguageMode::Auto, &opts)
        .unwrap();
    std::env::set_current_dir(cwd).unwrap();

    let changed1: BTreeSet<String> = out1
        .changed_symbols
        .iter()
        .map(|s| s.name.clone())
        .collect();
    let changed2: BTreeSet<String> = out2
        .changed_symbols
        .iter()
        .map(|s| s.name.clone())
        .collect();
    let impacted1 = impacted_name_set(&out1);
    let impacted2 = impacted_name_set(&out2);

    assert_eq!(changed1, BTreeSet::from(["bar".to_string()]));
    assert_eq!(changed1, changed2, "changed_symbols should be stable");
    assert_eq!(impacted1, impacted2, "impacted_symbols should be stable");
    assert!(impacted1.is_empty());
}

#[test]
#[serial]
fn lsp_engine_strict_mock_java_callees_fixture_runs() {
    let (_tmp, repo) = setup_repo_java_callees_chain_fixture();

    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();
    let files = dimpact::parse_unified_diff(&diff).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].new_path.as_deref(), Some("Main.java"));

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
    let changed = vec![dimpact::Symbol {
        id: dimpact::SymbolId::new("java", "Main.java", &dimpact::SymbolKind::Method, "b", 4),
        name: "b".to_string(),
        kind: dimpact::SymbolKind::Method,
        file: "Main.java".to_string(),
        range: dimpact::TextRange {
            start_line: 4,
            end_line: 7,
        },
        language: "java".to_string(),
    }];

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let out1 = engine
        .impact_from_symbols(&changed, dimpact::LanguageMode::Auto, &opts)
        .unwrap();
    let out2 = engine
        .impact_from_symbols(&changed, dimpact::LanguageMode::Auto, &opts)
        .unwrap();
    std::env::set_current_dir(cwd).unwrap();

    let changed1: BTreeSet<String> = out1
        .changed_symbols
        .iter()
        .map(|s| s.name.clone())
        .collect();
    let changed2: BTreeSet<String> = out2
        .changed_symbols
        .iter()
        .map(|s| s.name.clone())
        .collect();
    let impacted1 = impacted_name_set(&out1);
    let impacted2 = impacted_name_set(&out2);

    assert_eq!(changed1, BTreeSet::from(["b".to_string()]));
    assert_eq!(changed1, changed2, "changed_symbols should be stable");
    assert_eq!(impacted1, impacted2, "impacted_symbols should be stable");
}

#[test]
#[serial]
fn lsp_engine_strict_mock_java_both_fixture_runs() {
    let (_tmp, repo) = setup_repo_java_both_chain_fixture();

    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();
    let files = dimpact::parse_unified_diff(&diff).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].new_path.as_deref(), Some("Main.java"));

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
    let changed = vec![dimpact::Symbol {
        id: dimpact::SymbolId::new("java", "Main.java", &dimpact::SymbolKind::Method, "foo", 4),
        name: "foo".to_string(),
        kind: dimpact::SymbolKind::Method,
        file: "Main.java".to_string(),
        range: dimpact::TextRange {
            start_line: 4,
            end_line: 7,
        },
        language: "java".to_string(),
    }];

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let out1 = engine
        .impact_from_symbols(&changed, dimpact::LanguageMode::Auto, &opts)
        .unwrap();
    let out2 = engine
        .impact_from_symbols(&changed, dimpact::LanguageMode::Auto, &opts)
        .unwrap();
    std::env::set_current_dir(cwd).unwrap();

    let changed1: BTreeSet<String> = out1
        .changed_symbols
        .iter()
        .map(|s| s.name.clone())
        .collect();
    let changed2: BTreeSet<String> = out2
        .changed_symbols
        .iter()
        .map(|s| s.name.clone())
        .collect();
    let impacted1 = impacted_name_set(&out1);
    let impacted2 = impacted_name_set(&out2);

    let changed_ids1: Vec<String> = out1
        .changed_symbols
        .iter()
        .map(|s| s.id.0.clone())
        .collect();
    let changed_ids2: Vec<String> = out2
        .changed_symbols
        .iter()
        .map(|s| s.id.0.clone())
        .collect();
    let impacted_ids1: Vec<String> = out1
        .impacted_symbols
        .iter()
        .map(|s| s.id.0.clone())
        .collect();
    let impacted_ids2: Vec<String> = out2
        .impacted_symbols
        .iter()
        .map(|s| s.id.0.clone())
        .collect();

    let mut changed_ids1_sorted = changed_ids1.clone();
    changed_ids1_sorted.sort();
    changed_ids1_sorted.dedup();
    let mut changed_ids2_sorted = changed_ids2.clone();
    changed_ids2_sorted.sort();
    changed_ids2_sorted.dedup();
    let mut impacted_ids1_sorted = impacted_ids1.clone();
    impacted_ids1_sorted.sort();
    impacted_ids1_sorted.dedup();
    let mut impacted_ids2_sorted = impacted_ids2.clone();
    impacted_ids2_sorted.sort();
    impacted_ids2_sorted.dedup();

    assert_eq!(changed1, BTreeSet::from(["foo".to_string()]));
    assert_eq!(changed1, changed2, "changed_symbols should be stable");
    assert_eq!(impacted1, impacted2, "impacted_symbols should be stable");
    assert_eq!(
        changed_ids1, changed_ids2,
        "changed_symbols order should be stable"
    );
    assert_eq!(
        impacted_ids1, impacted_ids2,
        "impacted_symbols order should be stable"
    );
    assert_eq!(
        changed_ids1, changed_ids1_sorted,
        "changed_symbols should be sorted/deduped"
    );
    assert_eq!(
        changed_ids2, changed_ids2_sorted,
        "changed_symbols should be sorted/deduped"
    );
    assert_eq!(
        impacted_ids1, impacted_ids1_sorted,
        "impacted_symbols should be sorted/deduped"
    );
    assert_eq!(
        impacted_ids2, impacted_ids2_sorted,
        "impacted_symbols should be sorted/deduped"
    );
}

#[test]
#[serial]
fn lsp_engine_strict_mock_java_callees_fixture_with_refs_only_caps() {
    let (_tmp, repo) = setup_repo_java_callees_chain_fixture();

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
    let changed = vec![dimpact::Symbol {
        id: dimpact::SymbolId::new("java", "Main.java", &dimpact::SymbolKind::Method, "b", 4),
        name: "b".to_string(),
        kind: dimpact::SymbolKind::Method,
        file: "Main.java".to_string(),
        range: dimpact::TextRange {
            start_line: 4,
            end_line: 7,
        },
        language: "java".to_string(),
    }];

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let out1 = engine
        .impact_from_symbols(&changed, dimpact::LanguageMode::Auto, &opts)
        .unwrap();
    let out2 = engine
        .impact_from_symbols(&changed, dimpact::LanguageMode::Auto, &opts)
        .unwrap();
    std::env::set_current_dir(cwd).unwrap();

    let changed1: BTreeSet<String> = out1
        .changed_symbols
        .iter()
        .map(|s| s.name.clone())
        .collect();
    let changed2: BTreeSet<String> = out2
        .changed_symbols
        .iter()
        .map(|s| s.name.clone())
        .collect();

    assert_eq!(changed1, BTreeSet::from(["b".to_string()]));
    assert_eq!(changed1, changed2, "changed_symbols should be stable");
    assert_eq!(
        impacted_name_set(&out1),
        impacted_name_set(&out2),
        "impacted_symbols should be stable"
    );
}

#[test]
#[serial]
fn lsp_engine_strict_mock_java_both_fixture_with_refs_only_caps() {
    let (_tmp, repo) = setup_repo_java_both_chain_fixture();

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
    let changed = vec![dimpact::Symbol {
        id: dimpact::SymbolId::new("java", "Main.java", &dimpact::SymbolKind::Method, "foo", 4),
        name: "foo".to_string(),
        kind: dimpact::SymbolKind::Method,
        file: "Main.java".to_string(),
        range: dimpact::TextRange {
            start_line: 4,
            end_line: 7,
        },
        language: "java".to_string(),
    }];

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let out1 = engine
        .impact_from_symbols(&changed, dimpact::LanguageMode::Auto, &opts)
        .unwrap();
    let out2 = engine
        .impact_from_symbols(&changed, dimpact::LanguageMode::Auto, &opts)
        .unwrap();
    std::env::set_current_dir(cwd).unwrap();

    let changed1: BTreeSet<String> = out1
        .changed_symbols
        .iter()
        .map(|s| s.name.clone())
        .collect();
    let changed2: BTreeSet<String> = out2
        .changed_symbols
        .iter()
        .map(|s| s.name.clone())
        .collect();

    assert_eq!(changed1, BTreeSet::from(["foo".to_string()]));
    assert_eq!(changed1, changed2, "changed_symbols should be stable");
    assert_eq!(
        impacted_name_set(&out1),
        impacted_name_set(&out2),
        "impacted_symbols should be stable"
    );
}

#[test]
#[serial]
fn lsp_engine_strict_mock_go_callees_fixture_runs() {
    let (_tmp, repo) = setup_repo_go_callees_chain_fixture();

    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();
    let files = dimpact::parse_unified_diff(&diff).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].new_path.as_deref(), Some("main.go"));

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
    let changed = vec![dimpact::Symbol {
        id: dimpact::SymbolId::new("go", "main.go", &dimpact::SymbolKind::Function, "b", 5),
        name: "b".to_string(),
        kind: dimpact::SymbolKind::Function,
        file: "main.go".to_string(),
        range: dimpact::TextRange {
            start_line: 5,
            end_line: 9,
        },
        language: "go".to_string(),
    }];

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let out1 = engine
        .impact_from_symbols(&changed, dimpact::LanguageMode::Auto, &opts)
        .unwrap();
    let out2 = engine
        .impact_from_symbols(&changed, dimpact::LanguageMode::Auto, &opts)
        .unwrap();
    std::env::set_current_dir(cwd).unwrap();

    let changed1: BTreeSet<String> = out1
        .changed_symbols
        .iter()
        .map(|s| s.name.clone())
        .collect();
    let changed2: BTreeSet<String> = out2
        .changed_symbols
        .iter()
        .map(|s| s.name.clone())
        .collect();
    let impacted1 = impacted_name_set(&out1);
    let impacted2 = impacted_name_set(&out2);

    assert_eq!(changed1, BTreeSet::from(["b".to_string()]));
    assert_eq!(changed1, changed2, "changed_symbols should be stable");
    assert_eq!(impacted1, impacted2, "impacted_symbols should be stable");
}

#[test]
#[serial]
fn lsp_engine_strict_mock_go_both_fixture_runs() {
    let (_tmp, repo) = setup_repo_go_both_chain_fixture();

    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();
    let files = dimpact::parse_unified_diff(&diff).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].new_path.as_deref(), Some("main.go"));

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
    let changed = vec![dimpact::Symbol {
        id: dimpact::SymbolId::new("go", "main.go", &dimpact::SymbolKind::Function, "foo", 5),
        name: "foo".to_string(),
        kind: dimpact::SymbolKind::Function,
        file: "main.go".to_string(),
        range: dimpact::TextRange {
            start_line: 5,
            end_line: 9,
        },
        language: "go".to_string(),
    }];

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let out1 = engine
        .impact_from_symbols(&changed, dimpact::LanguageMode::Auto, &opts)
        .unwrap();
    let out2 = engine
        .impact_from_symbols(&changed, dimpact::LanguageMode::Auto, &opts)
        .unwrap();
    std::env::set_current_dir(cwd).unwrap();

    let changed1: BTreeSet<String> = out1
        .changed_symbols
        .iter()
        .map(|s| s.name.clone())
        .collect();
    let changed2: BTreeSet<String> = out2
        .changed_symbols
        .iter()
        .map(|s| s.name.clone())
        .collect();
    let impacted1 = impacted_name_set(&out1);
    let impacted2 = impacted_name_set(&out2);

    assert_eq!(changed1, BTreeSet::from(["foo".to_string()]));
    assert_eq!(changed1, changed2, "changed_symbols should be stable");
    assert_eq!(impacted1, impacted2, "impacted_symbols should be stable");
}

#[test]
#[serial]
fn lsp_engine_strict_mock_go_callees_fixture_with_refs_only_caps() {
    let (_tmp, repo) = setup_repo_go_callees_chain_fixture();

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
    let changed = vec![dimpact::Symbol {
        id: dimpact::SymbolId::new("go", "main.go", &dimpact::SymbolKind::Function, "b", 5),
        name: "b".to_string(),
        kind: dimpact::SymbolKind::Function,
        file: "main.go".to_string(),
        range: dimpact::TextRange {
            start_line: 5,
            end_line: 9,
        },
        language: "go".to_string(),
    }];

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let out1 = engine
        .impact_from_symbols(&changed, dimpact::LanguageMode::Auto, &opts)
        .unwrap();
    let out2 = engine
        .impact_from_symbols(&changed, dimpact::LanguageMode::Auto, &opts)
        .unwrap();
    std::env::set_current_dir(cwd).unwrap();

    let changed1: BTreeSet<String> = out1
        .changed_symbols
        .iter()
        .map(|s| s.name.clone())
        .collect();
    let changed2: BTreeSet<String> = out2
        .changed_symbols
        .iter()
        .map(|s| s.name.clone())
        .collect();

    assert_eq!(changed1, BTreeSet::from(["b".to_string()]));
    assert_eq!(changed1, changed2, "changed_symbols should be stable");
    assert_eq!(
        impacted_name_set(&out1),
        impacted_name_set(&out2),
        "impacted_symbols should be stable"
    );
}

#[test]
#[serial]
fn lsp_engine_strict_mock_go_both_fixture_with_refs_only_caps() {
    let (_tmp, repo) = setup_repo_go_both_chain_fixture();

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
    let changed = vec![dimpact::Symbol {
        id: dimpact::SymbolId::new("go", "main.go", &dimpact::SymbolKind::Function, "foo", 5),
        name: "foo".to_string(),
        kind: dimpact::SymbolKind::Function,
        file: "main.go".to_string(),
        range: dimpact::TextRange {
            start_line: 5,
            end_line: 9,
        },
        language: "go".to_string(),
    }];

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let out1 = engine
        .impact_from_symbols(&changed, dimpact::LanguageMode::Auto, &opts)
        .unwrap();
    let out2 = engine
        .impact_from_symbols(&changed, dimpact::LanguageMode::Auto, &opts)
        .unwrap();
    std::env::set_current_dir(cwd).unwrap();

    let changed1: BTreeSet<String> = out1
        .changed_symbols
        .iter()
        .map(|s| s.name.clone())
        .collect();
    let changed2: BTreeSet<String> = out2
        .changed_symbols
        .iter()
        .map(|s| s.name.clone())
        .collect();

    assert_eq!(changed1, BTreeSet::from(["foo".to_string()]));
    assert_eq!(changed1, changed2, "changed_symbols should be stable");
    assert_eq!(
        impacted_name_set(&out1),
        impacted_name_set(&out2),
        "impacted_symbols should be stable"
    );
}

#[test]
#[serial]
fn lsp_engine_strict_mock_python_callers_fixture_runs() {
    let (_tmp, repo) = setup_repo_python_callers_chain_fixture();

    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();
    let files = dimpact::parse_unified_diff(&diff).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].new_path.as_deref(), Some("main.py"));

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
    let changed = vec![dimpact::Symbol {
        id: dimpact::SymbolId::new(
            "python",
            "main.py",
            &dimpact::SymbolKind::Function,
            "bar",
            1,
        ),
        name: "bar".to_string(),
        kind: dimpact::SymbolKind::Function,
        file: "main.py".to_string(),
        range: dimpact::TextRange {
            start_line: 1,
            end_line: 3,
        },
        language: "python".to_string(),
    }];

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let out1 = engine
        .impact_from_symbols(&changed, dimpact::LanguageMode::Auto, &opts)
        .unwrap();
    let out2 = engine
        .impact_from_symbols(&changed, dimpact::LanguageMode::Auto, &opts)
        .unwrap();
    std::env::set_current_dir(cwd).unwrap();

    let changed1: BTreeSet<String> = out1
        .changed_symbols
        .iter()
        .map(|s| s.name.clone())
        .collect();
    let changed2: BTreeSet<String> = out2
        .changed_symbols
        .iter()
        .map(|s| s.name.clone())
        .collect();
    let impacted1 = impacted_name_set(&out1);
    let impacted2 = impacted_name_set(&out2);

    assert_eq!(changed1, changed2, "changed_symbols should be stable");
    assert_eq!(impacted1, impacted2, "impacted_symbols should be stable");

    let expected_changed = BTreeSet::from(["bar".to_string()]);
    let expected_impacted: BTreeSet<String> = BTreeSet::new();

    assert_eq!(changed1, expected_changed);
    assert_eq!(impacted1, expected_impacted);
}

#[test]
#[serial]
fn lsp_engine_strict_mock_python_callees_fixture_runs() {
    let (_tmp, repo) = setup_repo_python_callees_chain_fixture();

    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();
    let files = dimpact::parse_unified_diff(&diff).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].new_path.as_deref(), Some("main.py"));

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
    let changed = vec![dimpact::Symbol {
        id: dimpact::SymbolId::new(
            "python",
            "main.py",
            &dimpact::SymbolKind::Function,
            "foo",
            7,
        ),
        name: "foo".to_string(),
        kind: dimpact::SymbolKind::Function,
        file: "main.py".to_string(),
        range: dimpact::TextRange {
            start_line: 7,
            end_line: 9,
        },
        language: "python".to_string(),
    }];

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let out1 = engine
        .impact_from_symbols(&changed, dimpact::LanguageMode::Auto, &opts)
        .unwrap();
    let out2 = engine
        .impact_from_symbols(&changed, dimpact::LanguageMode::Auto, &opts)
        .unwrap();
    std::env::set_current_dir(cwd).unwrap();

    assert_eq!(
        impacted_name_set(&out1),
        impacted_name_set(&out2),
        "strict mock callees fixture should be stable"
    );
}

#[test]
#[serial]
fn lsp_engine_strict_mock_python_both_fixture_runs() {
    let (_tmp, repo) = setup_repo_python_both_chain_fixture();

    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();
    let files = dimpact::parse_unified_diff(&diff).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].new_path.as_deref(), Some("main.py"));

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
    let changed = vec![dimpact::Symbol {
        id: dimpact::SymbolId::new(
            "python",
            "main.py",
            &dimpact::SymbolKind::Function,
            "foo",
            4,
        ),
        name: "foo".to_string(),
        kind: dimpact::SymbolKind::Function,
        file: "main.py".to_string(),
        range: dimpact::TextRange {
            start_line: 4,
            end_line: 6,
        },
        language: "python".to_string(),
    }];

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let out1 = engine
        .impact_from_symbols(&changed, dimpact::LanguageMode::Auto, &opts)
        .unwrap();
    let out2 = engine
        .impact_from_symbols(&changed, dimpact::LanguageMode::Auto, &opts)
        .unwrap();
    std::env::set_current_dir(cwd).unwrap();

    let changed1: BTreeSet<String> = out1
        .changed_symbols
        .iter()
        .map(|s| s.name.clone())
        .collect();
    let changed2: BTreeSet<String> = out2
        .changed_symbols
        .iter()
        .map(|s| s.name.clone())
        .collect();
    let impacted1 = impacted_name_set(&out1);
    let impacted2 = impacted_name_set(&out2);

    assert_eq!(changed1, changed2, "changed_symbols should be stable");
    assert_eq!(impacted1, impacted2, "impacted_symbols should be stable");
}

#[test]
#[serial]
fn lsp_engine_strict_mock_python_callees_fixture_with_refs_only_caps() {
    let (_tmp, repo) = setup_repo_python_callees_chain_fixture();

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
    let changed = vec![dimpact::Symbol {
        id: dimpact::SymbolId::new(
            "python",
            "main.py",
            &dimpact::SymbolKind::Function,
            "foo",
            7,
        ),
        name: "foo".to_string(),
        kind: dimpact::SymbolKind::Function,
        file: "main.py".to_string(),
        range: dimpact::TextRange {
            start_line: 7,
            end_line: 9,
        },
        language: "python".to_string(),
    }];

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let out1 = engine
        .impact_from_symbols(&changed, dimpact::LanguageMode::Auto, &opts)
        .unwrap();
    let out2 = engine
        .impact_from_symbols(&changed, dimpact::LanguageMode::Auto, &opts)
        .unwrap();
    std::env::set_current_dir(cwd).unwrap();

    let changed1: BTreeSet<String> = out1
        .changed_symbols
        .iter()
        .map(|s| s.name.clone())
        .collect();
    let changed2: BTreeSet<String> = out2
        .changed_symbols
        .iter()
        .map(|s| s.name.clone())
        .collect();

    assert_eq!(changed1, BTreeSet::from(["foo".to_string()]));
    assert_eq!(changed1, changed2, "changed_symbols should be stable");
    assert_eq!(
        impacted_name_set(&out1),
        impacted_name_set(&out2),
        "impacted_symbols should be stable"
    );
}

#[test]
#[serial]
fn lsp_engine_strict_mock_python_both_fixture_with_refs_only_caps() {
    let (_tmp, repo) = setup_repo_python_both_chain_fixture();

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
    let changed = vec![dimpact::Symbol {
        id: dimpact::SymbolId::new(
            "python",
            "main.py",
            &dimpact::SymbolKind::Function,
            "foo",
            4,
        ),
        name: "foo".to_string(),
        kind: dimpact::SymbolKind::Function,
        file: "main.py".to_string(),
        range: dimpact::TextRange {
            start_line: 4,
            end_line: 6,
        },
        language: "python".to_string(),
    }];

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let out1 = engine
        .impact_from_symbols(&changed, dimpact::LanguageMode::Auto, &opts)
        .unwrap();
    let out2 = engine
        .impact_from_symbols(&changed, dimpact::LanguageMode::Auto, &opts)
        .unwrap();
    std::env::set_current_dir(cwd).unwrap();

    let changed1: BTreeSet<String> = out1
        .changed_symbols
        .iter()
        .map(|s| s.name.clone())
        .collect();
    let changed2: BTreeSet<String> = out2
        .changed_symbols
        .iter()
        .map(|s| s.name.clone())
        .collect();

    assert_eq!(changed1, BTreeSet::from(["foo".to_string()]));
    assert_eq!(changed1, changed2, "changed_symbols should be stable");
    assert_eq!(
        impacted_name_set(&out1),
        impacted_name_set(&out2),
        "impacted_symbols should be stable"
    );
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
fn typescript_real_lsp_e2e_fixture_is_opt_in_gated() {
    if !should_run_typescript_strict_lsp_e2e() {
        eprintln!(
            "skip: set DIMPACT_E2E_STRICT_LSP_TYPESCRIPT=1 (or DIMPACT_E2E_STRICT_LSP=1) to run TypeScript strict LSP e2e fixture"
        );
        return;
    }
    require_typescript_lsp_server_for_lane("typescript/preflight-fixture");

    let (_tmp, repo) = setup_repo_typescript_real_lsp_e2e_fixture();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();
    let files = dimpact::parse_unified_diff(&diff).unwrap();

    assert_eq!(files.len(), 1);
    assert_eq!(files[0].new_path.as_deref(), Some("main.ts"));
    assert!(
        files[0]
            .changes
            .iter()
            .any(|c| matches!(c.kind, dimpact::ChangeKind::Added)),
        "fixture should include added lines for changed detection"
    );

    let tsconfig = fs::read_to_string(repo.join("tsconfig.json")).expect("read tsconfig");
    assert!(tsconfig.contains("\"strict\": true"));
}

#[test]
#[serial]
fn tsx_real_lsp_e2e_fixture_is_opt_in_gated() {
    if !should_run_tsx_strict_lsp_e2e() {
        eprintln!(
            "skip: set DIMPACT_E2E_STRICT_LSP_TSX=1 (or DIMPACT_E2E_STRICT_LSP=1) to run TSX strict LSP e2e fixture"
        );
        return;
    }
    require_typescript_lsp_server_for_lane("tsx/preflight-fixture");

    let (_tmp, repo) = setup_repo_tsx_real_lsp_e2e_fixture();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();
    let files = dimpact::parse_unified_diff(&diff).unwrap();

    assert_eq!(files.len(), 1);
    assert_eq!(files[0].new_path.as_deref(), Some("app.tsx"));
    assert!(
        files[0]
            .changes
            .iter()
            .any(|c| matches!(c.kind, dimpact::ChangeKind::Added)),
        "fixture should include added lines for changed detection"
    );

    let tsconfig = fs::read_to_string(repo.join("tsconfig.json")).expect("read tsconfig");
    assert!(tsconfig.contains("\"jsx\": \"react-jsx\""));
}

#[test]
#[serial]
fn lsp_engine_strict_tsx_callers_chain_e2e_when_available() {
    if !require_env_gate_callers_lane(
        "tsx/callers",
        "DIMPACT_E2E_STRICT_LSP_TSX",
        "DIMPACT_E2E_STRICT_LSP_TSX=1 (or DIMPACT_E2E_STRICT_LSP=1)",
    ) {
        return;
    }
    require_typescript_lsp_server_for_lane("tsx/callers");

    let (_tmp, repo) = setup_repo_tsx_real_lsp_e2e_fixture();
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
    let changed_res = engine.changed_symbols(&files, dimpact::LanguageMode::Auto);
    let out1_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    let out2_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    std::env::set_current_dir(cwd).unwrap();

    let changed = fail_fast_with_triage("tsx/callers", "changed_symbols", changed_res);
    let out1 = fail_fast_with_triage("tsx/callers", "impact#1", out1_res);
    let out2 = fail_fast_with_triage("tsx/callers", "impact#2", out2_res);

    assert!(changed.changed_symbols.iter().any(|s| s.name == "bar"));
    let names1 = impacted_name_set(&out1);
    let names2 = impacted_name_set(&out2);
    assert_eq!(
        names1, names2,
        "strict TSX LSP callers result should be stable"
    );
    assert!(
        names1.contains("View") || names1.contains("App"),
        "fail-fast: lane=tsx/callers phase=assertion cause=logic expected impacted caller 'View' or 'App', got {:?}",
        names1
    );
}

#[test]
#[serial]
fn lsp_engine_strict_tsx_callees_chain_e2e_when_available() {
    if !should_run_tsx_strict_lsp_e2e() {
        eprintln!(
            "skip: set DIMPACT_E2E_STRICT_LSP_TSX=1 (or DIMPACT_E2E_STRICT_LSP=1) to run TSX strict LSP e2e tests"
        );
        return;
    }
    require_typescript_lsp_server_for_lane("tsx/callees");

    let (_tmp, repo) = setup_repo_tsx_callees_real_lsp_e2e_fixture();
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
        direction: dimpact::ImpactDirection::Callees,
        max_depth: Some(5),
        with_edges: Some(false),
        ignore_dirs: Vec::new(),
    };

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let changed_res = engine.changed_symbols(&files, dimpact::LanguageMode::Auto);
    let out1_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    let out2_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    std::env::set_current_dir(cwd).unwrap();

    let changed = fail_fast_with_triage("tsx/callees", "changed_symbols", changed_res);
    let out1 = fail_fast_with_triage("tsx/callees", "impact#1", out1_res);
    let out2 = fail_fast_with_triage("tsx/callees", "impact#2", out2_res);

    assert!(changed.changed_symbols.iter().any(|s| s.name == "View"));
    let names1 = impacted_name_set(&out1);
    let names2 = impacted_name_set(&out2);
    assert_eq!(
        names1, names2,
        "strict TSX LSP callees result should be stable"
    );
    assert!(names1.contains("bar") || names1.contains("baz"));
}

#[test]
#[serial]
fn lsp_engine_strict_tsx_both_chain_e2e_when_available() {
    if !should_run_tsx_strict_lsp_e2e() {
        eprintln!(
            "skip: set DIMPACT_E2E_STRICT_LSP_TSX=1 (or DIMPACT_E2E_STRICT_LSP=1) to run TSX strict LSP e2e tests"
        );
        return;
    }
    require_typescript_lsp_server_for_lane("tsx/both");

    let (_tmp, repo) = setup_repo_tsx_callees_real_lsp_e2e_fixture();
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
        direction: dimpact::ImpactDirection::Both,
        max_depth: Some(5),
        with_edges: Some(false),
        ignore_dirs: Vec::new(),
    };

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let changed_res = engine.changed_symbols(&files, dimpact::LanguageMode::Auto);
    let out1_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    let out2_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    std::env::set_current_dir(cwd).unwrap();

    let changed = fail_fast_with_triage("tsx/both", "changed_symbols", changed_res);
    let out1 = fail_fast_with_triage("tsx/both", "impact#1", out1_res);
    let out2 = fail_fast_with_triage("tsx/both", "impact#2", out2_res);

    assert!(changed.changed_symbols.iter().any(|s| s.name == "View"));
    let names1 = impacted_name_set(&out1);
    let names2 = impacted_name_set(&out2);
    assert_eq!(
        names1, names2,
        "strict TSX LSP both result should be stable"
    );
    assert!(names1.contains("bar") || names1.contains("baz") || names1.contains("App"));
}

#[test]
#[serial]
fn lsp_engine_strict_typescript_callers_chain_e2e_when_available() {
    if !require_env_gate_callers_lane(
        "typescript/callers",
        "DIMPACT_E2E_STRICT_LSP_TYPESCRIPT",
        "DIMPACT_E2E_STRICT_LSP_TYPESCRIPT=1 (or DIMPACT_E2E_STRICT_LSP=1)",
    ) {
        return;
    }
    require_typescript_lsp_server_for_lane("typescript/callers");

    let (_tmp, repo) = setup_repo_typescript_real_lsp_e2e_fixture();
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
    let changed_res = engine.changed_symbols(&files, dimpact::LanguageMode::Auto);
    let out1_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    let out2_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    std::env::set_current_dir(cwd).unwrap();

    let changed = fail_fast_with_triage("typescript/callers", "changed_symbols", changed_res);
    let out1 = fail_fast_with_triage("typescript/callers", "impact#1", out1_res);
    let out2 = fail_fast_with_triage("typescript/callers", "impact#2", out2_res);

    assert!(changed.changed_symbols.iter().any(|s| s.name == "bar"));
    let names1 = impacted_name_set(&out1);
    let names2 = impacted_name_set(&out2);
    assert_eq!(
        names1, names2,
        "strict TypeScript LSP callers result should be stable"
    );
    assert!(
        names1.contains("foo"),
        "fail-fast: lane=typescript/callers phase=assertion cause=logic expected impacted caller 'foo', got {:?}",
        names1
    );
}

#[test]
#[serial]
fn lsp_engine_strict_typescript_callees_chain_e2e_when_available() {
    if !should_run_typescript_strict_lsp_e2e() {
        eprintln!(
            "skip: set DIMPACT_E2E_STRICT_LSP_TYPESCRIPT=1 (or DIMPACT_E2E_STRICT_LSP=1) to run TypeScript strict LSP e2e tests"
        );
        return;
    }
    require_typescript_lsp_server_for_lane("typescript/callees");

    let (_tmp, repo) = setup_repo_typescript_callees_real_lsp_e2e_fixture();
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
        direction: dimpact::ImpactDirection::Callees,
        max_depth: Some(5),
        with_edges: Some(false),
        ignore_dirs: Vec::new(),
    };

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let changed_res = engine.changed_symbols(&files, dimpact::LanguageMode::Auto);
    let out1_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    let out2_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    std::env::set_current_dir(cwd).unwrap();

    let changed = fail_fast_with_triage("typescript/callees", "changed_symbols", changed_res);
    let out1 = fail_fast_with_triage("typescript/callees", "impact#1", out1_res);
    let out2 = fail_fast_with_triage("typescript/callees", "impact#2", out2_res);

    assert!(changed.changed_symbols.iter().any(|s| s.name == "foo"));
    let names1 = impacted_name_set(&out1);
    let names2 = impacted_name_set(&out2);
    assert_eq!(
        names1, names2,
        "strict TypeScript LSP callees result should be stable"
    );
    assert!(names1.contains("bar") || names1.contains("baz"));
}

#[test]
#[serial]
fn lsp_engine_strict_typescript_both_chain_e2e_when_available() {
    if !should_run_typescript_strict_lsp_e2e() {
        eprintln!(
            "skip: set DIMPACT_E2E_STRICT_LSP_TYPESCRIPT=1 (or DIMPACT_E2E_STRICT_LSP=1) to run TypeScript strict LSP e2e tests"
        );
        return;
    }
    require_typescript_lsp_server_for_lane("typescript/both");

    let (_tmp, repo) = setup_repo_typescript_callees_real_lsp_e2e_fixture();
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
        direction: dimpact::ImpactDirection::Both,
        max_depth: Some(5),
        with_edges: Some(false),
        ignore_dirs: Vec::new(),
    };

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let changed_res = engine.changed_symbols(&files, dimpact::LanguageMode::Auto);
    let out1_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    let out2_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    std::env::set_current_dir(cwd).unwrap();

    let changed = fail_fast_with_triage("typescript/both", "changed_symbols", changed_res);
    let out1 = fail_fast_with_triage("typescript/both", "impact#1", out1_res);
    let out2 = fail_fast_with_triage("typescript/both", "impact#2", out2_res);

    assert!(changed.changed_symbols.iter().any(|s| s.name == "foo"));
    let names1 = impacted_name_set(&out1);
    let names2 = impacted_name_set(&out2);
    assert_eq!(
        names1, names2,
        "strict TypeScript LSP both result should be stable"
    );
    assert!(names1.contains("bar") || names1.contains("baz") || names1.contains("main"));
}

#[test]
#[serial]
fn javascript_real_lsp_e2e_fixture_is_opt_in_gated() {
    if !should_run_javascript_strict_lsp_e2e() {
        eprintln!(
            "skip: set DIMPACT_E2E_STRICT_LSP_JAVASCRIPT=1 (or DIMPACT_E2E_STRICT_LSP=1) to run JavaScript strict LSP e2e fixture"
        );
        return;
    }
    require_typescript_lsp_server_for_lane("javascript/preflight-fixture");

    let (_tmp, repo) = setup_repo_javascript_real_lsp_e2e_fixture();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();
    let files = dimpact::parse_unified_diff(&diff).unwrap();

    assert_eq!(files.len(), 1);
    assert_eq!(files[0].new_path.as_deref(), Some("main.js"));
    assert!(
        files[0]
            .changes
            .iter()
            .any(|c| matches!(c.kind, dimpact::ChangeKind::Added)),
        "fixture should include added lines for changed detection"
    );

    let package_json = fs::read_to_string(repo.join("package.json")).expect("read package.json");
    assert!(package_json.contains("lsp-javascript-fixture"));
}

#[test]
#[serial]
fn lsp_engine_strict_javascript_callers_chain_e2e_when_available() {
    if !require_env_gate_callers_lane(
        "javascript/callers",
        "DIMPACT_E2E_STRICT_LSP_JAVASCRIPT",
        "DIMPACT_E2E_STRICT_LSP_JAVASCRIPT=1 (or DIMPACT_E2E_STRICT_LSP=1)",
    ) {
        return;
    }
    require_typescript_lsp_server_for_lane("javascript/callers");

    let (_tmp, repo) = setup_repo_javascript_real_lsp_e2e_fixture();
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
    let changed_res = engine.changed_symbols(&files, dimpact::LanguageMode::Auto);
    let out1_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    let out2_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    std::env::set_current_dir(cwd).unwrap();

    let changed = fail_fast_with_triage("javascript/callers", "changed_symbols", changed_res);
    let out1 = fail_fast_with_triage("javascript/callers", "impact#1", out1_res);
    let out2 = fail_fast_with_triage("javascript/callers", "impact#2", out2_res);

    assert!(changed.changed_symbols.iter().any(|s| s.name == "bar"));
    let names1 = impacted_name_set(&out1);
    let names2 = impacted_name_set(&out2);
    assert_eq!(
        names1, names2,
        "strict JavaScript LSP callers result should be stable"
    );
    assert!(
        names1.contains("foo"),
        "fail-fast: lane=javascript/callers phase=assertion cause=logic expected impacted caller 'foo', got {:?}",
        names1
    );
}

#[test]
#[serial]
fn lsp_engine_strict_javascript_callees_chain_e2e_when_available() {
    if !should_run_javascript_strict_lsp_e2e() {
        eprintln!(
            "skip: set DIMPACT_E2E_STRICT_LSP_JAVASCRIPT=1 (or DIMPACT_E2E_STRICT_LSP=1) to run JavaScript strict LSP e2e tests"
        );
        return;
    }
    require_typescript_lsp_server_for_lane("javascript/callees");

    let (_tmp, repo) = setup_repo_javascript_callees_real_lsp_e2e_fixture();
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
        direction: dimpact::ImpactDirection::Callees,
        max_depth: Some(5),
        with_edges: Some(false),
        ignore_dirs: Vec::new(),
    };

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let changed_res = engine.changed_symbols(&files, dimpact::LanguageMode::Auto);
    let out1_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    let out2_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    std::env::set_current_dir(cwd).unwrap();

    let changed = fail_fast_with_triage("javascript/callees", "changed_symbols", changed_res);
    let out1 = fail_fast_with_triage("javascript/callees", "impact#1", out1_res);
    let out2 = fail_fast_with_triage("javascript/callees", "impact#2", out2_res);

    assert!(changed.changed_symbols.iter().any(|s| s.name == "foo"));
    let names1 = impacted_name_set(&out1);
    let names2 = impacted_name_set(&out2);
    assert_eq!(
        names1, names2,
        "strict JavaScript LSP callees result should be stable"
    );
    assert!(names1.contains("bar") || names1.contains("baz"));
}

#[test]
#[serial]
fn lsp_engine_strict_javascript_both_chain_e2e_when_available() {
    if !should_run_javascript_strict_lsp_e2e() {
        eprintln!(
            "skip: set DIMPACT_E2E_STRICT_LSP_JAVASCRIPT=1 (or DIMPACT_E2E_STRICT_LSP=1) to run JavaScript strict LSP e2e tests"
        );
        return;
    }
    require_typescript_lsp_server_for_lane("javascript/both");

    let (_tmp, repo) = setup_repo_javascript_callees_real_lsp_e2e_fixture();
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
        direction: dimpact::ImpactDirection::Both,
        max_depth: Some(5),
        with_edges: Some(false),
        ignore_dirs: Vec::new(),
    };

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let changed_res = engine.changed_symbols(&files, dimpact::LanguageMode::Auto);
    let out1_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    let out2_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    std::env::set_current_dir(cwd).unwrap();

    let changed = fail_fast_with_triage("javascript/both", "changed_symbols", changed_res);
    let out1 = fail_fast_with_triage("javascript/both", "impact#1", out1_res);
    let out2 = fail_fast_with_triage("javascript/both", "impact#2", out2_res);

    assert!(changed.changed_symbols.iter().any(|s| s.name == "foo"));
    let names1 = impacted_name_set(&out1);
    let names2 = impacted_name_set(&out2);
    assert_eq!(
        names1, names2,
        "strict JavaScript LSP both result should be stable"
    );
    assert!(names1.contains("bar") || names1.contains("baz") || names1.contains("main"));
}

#[test]
#[serial]
fn ruby_real_lsp_e2e_fixture_is_opt_in_gated() {
    if !should_run_ruby_strict_lsp_e2e() {
        eprintln!(
            "skip: set DIMPACT_E2E_STRICT_LSP_RUBY=1 (or DIMPACT_E2E_STRICT_LSP=1) to run Ruby strict LSP e2e fixture"
        );
        return;
    }
    require_ruby_lsp_server_for_lane("ruby/preflight-fixture");

    let (_tmp, repo) = setup_repo_ruby_real_lsp_e2e_fixture();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();
    let files = dimpact::parse_unified_diff(&diff).unwrap();

    assert_eq!(files.len(), 1);
    assert_eq!(files[0].new_path.as_deref(), Some("main.rb"));
    assert!(
        files[0]
            .changes
            .iter()
            .any(|c| matches!(c.kind, dimpact::ChangeKind::Added)),
        "fixture should include added lines for changed detection"
    );

    let gemfile = fs::read_to_string(repo.join("Gemfile")).expect("read Gemfile");
    assert!(gemfile.contains("rubygems.org"));
}

#[test]
#[serial]
fn lsp_engine_strict_ruby_callers_chain_e2e_when_available() {
    if !require_env_gate_callers_lane(
        "ruby/callers",
        "DIMPACT_E2E_STRICT_LSP_RUBY",
        "DIMPACT_E2E_STRICT_LSP_RUBY=1 (or DIMPACT_E2E_STRICT_LSP=1)",
    ) {
        return;
    }
    require_ruby_lsp_server_for_lane("ruby/callers");

    let (_tmp, repo) = setup_repo_ruby_real_lsp_e2e_fixture();
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
    let changed_res = engine.changed_symbols(&files, dimpact::LanguageMode::Auto);
    let out1_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    let out2_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    std::env::set_current_dir(cwd).unwrap();

    let changed = fail_fast_with_triage("ruby/callers", "changed_symbols", changed_res);
    let out1 = fail_fast_with_triage("ruby/callers", "impact#1", out1_res);
    let out2 = fail_fast_with_triage("ruby/callers", "impact#2", out2_res);

    assert!(changed.changed_symbols.iter().any(|s| s.name == "bar"));
    let names1 = impacted_name_set(&out1);
    let names2 = impacted_name_set(&out2);
    assert_eq!(
        names1, names2,
        "strict Ruby LSP callers result should be stable"
    );
    assert!(
        names1.contains("foo"),
        "fail-fast: lane=ruby/callers phase=assertion cause=logic expected impacted caller 'foo', got {:?}",
        names1
    );
}

#[test]
#[serial]
fn lsp_engine_strict_ruby_callees_chain_e2e_when_available() {
    if !should_run_ruby_strict_lsp_e2e() {
        eprintln!(
            "skip: set DIMPACT_E2E_STRICT_LSP_RUBY=1 (or DIMPACT_E2E_STRICT_LSP=1) to run Ruby strict LSP e2e tests"
        );
        return;
    }
    require_ruby_lsp_server_for_lane("ruby/callees");

    let (_tmp, repo) = setup_repo_ruby_callees_real_lsp_e2e_fixture();
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
        direction: dimpact::ImpactDirection::Callees,
        max_depth: Some(5),
        with_edges: Some(false),
        ignore_dirs: Vec::new(),
    };

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let changed_res = engine.changed_symbols(&files, dimpact::LanguageMode::Auto);
    let out1_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    let out2_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    std::env::set_current_dir(cwd).unwrap();

    let changed = fail_fast_with_triage("ruby/callees", "changed_symbols", changed_res);
    let out1 = fail_fast_with_triage("ruby/callees", "impact#1", out1_res);
    let out2 = fail_fast_with_triage("ruby/callees", "impact#2", out2_res);

    assert!(changed.changed_symbols.iter().any(|s| s.name == "foo"));
    let names1 = impacted_name_set(&out1);
    let names2 = impacted_name_set(&out2);
    assert_eq!(
        names1, names2,
        "strict Ruby LSP callees result should be stable"
    );
    assert!(names1.contains("bar") || names1.contains("baz"));
}

#[test]
#[serial]
fn lsp_engine_strict_ruby_both_chain_e2e_when_available() {
    if !should_run_ruby_strict_lsp_e2e() {
        eprintln!(
            "skip: set DIMPACT_E2E_STRICT_LSP_RUBY=1 (or DIMPACT_E2E_STRICT_LSP=1) to run Ruby strict LSP e2e tests"
        );
        return;
    }
    require_ruby_lsp_server_for_lane("ruby/both");

    let (_tmp, repo) = setup_repo_ruby_callees_real_lsp_e2e_fixture();
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
        direction: dimpact::ImpactDirection::Both,
        max_depth: Some(5),
        with_edges: Some(false),
        ignore_dirs: Vec::new(),
    };

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let changed_res = engine.changed_symbols(&files, dimpact::LanguageMode::Auto);
    let out1_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    let out2_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    std::env::set_current_dir(cwd).unwrap();

    let changed = fail_fast_with_triage("ruby/both", "changed_symbols", changed_res);
    let out1 = fail_fast_with_triage("ruby/both", "impact#1", out1_res);
    let out2 = fail_fast_with_triage("ruby/both", "impact#2", out2_res);

    assert!(changed.changed_symbols.iter().any(|s| s.name == "foo"));
    let names1 = impacted_name_set(&out1);
    let names2 = impacted_name_set(&out2);
    assert_eq!(
        names1, names2,
        "strict Ruby LSP both result should be stable"
    );
    assert!(names1.contains("bar") || names1.contains("baz") || names1.contains("main"));
}

#[test]
#[serial]
fn go_real_lsp_e2e_fixture_is_opt_in_gated() {
    if !require_env_gate_callers_lane(
        "go/preflight-fixture",
        "DIMPACT_E2E_STRICT_LSP_GO",
        "DIMPACT_E2E_STRICT_LSP_GO=1 (or DIMPACT_E2E_STRICT_LSP=1)",
    ) {
        return;
    }
    require_gopls_for_lane("go/preflight-fixture");

    let (_tmp, repo) = setup_repo_go_real_lsp_e2e_fixture();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();
    let files = dimpact::parse_unified_diff(&diff).unwrap();

    assert_eq!(files.len(), 1);
    assert_eq!(files[0].new_path.as_deref(), Some("main.go"));
    assert!(
        files[0]
            .changes
            .iter()
            .any(|c| matches!(c.kind, dimpact::ChangeKind::Added)),
        "fixture should include added lines for changed detection"
    );

    let gomod = fs::read_to_string(repo.join("go.mod")).expect("read go.mod");
    assert!(gomod.contains("module lsp-go-fixture"));
}

#[test]
#[serial]
fn java_real_lsp_e2e_fixture_is_opt_in_gated() {
    if !require_env_gate_callers_lane(
        "java/preflight-fixture",
        "DIMPACT_E2E_STRICT_LSP_JAVA",
        "DIMPACT_E2E_STRICT_LSP_JAVA=1 (or DIMPACT_E2E_STRICT_LSP=1)",
    ) {
        return;
    }
    require_jdtls_for_lane("java/preflight-fixture");

    let (_tmp, repo) = setup_repo_java_real_lsp_e2e_fixture();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();
    let files = dimpact::parse_unified_diff(&diff).unwrap();

    assert_eq!(files.len(), 1);
    assert_eq!(
        files[0].new_path.as_deref(),
        Some("src/main/java/Main.java")
    );
    assert!(
        files[0]
            .changes
            .iter()
            .any(|c| matches!(c.kind, dimpact::ChangeKind::Added)),
        "fixture should include added lines for changed detection"
    );

    let pom = fs::read_to_string(repo.join("pom.xml")).expect("read pom.xml");
    assert!(pom.contains("<artifactId>lsp-java-fixture</artifactId>"));
}

#[test]
#[serial]
fn lsp_engine_strict_java_callers_chain_e2e_when_available() {
    if !require_env_gate_callers_lane(
        "java/callers",
        "DIMPACT_E2E_STRICT_LSP_JAVA",
        "DIMPACT_E2E_STRICT_LSP_JAVA=1 (or DIMPACT_E2E_STRICT_LSP=1)",
    ) {
        return;
    }
    require_jdtls_for_lane("java/callers");

    let (_tmp, repo) = setup_repo_java_real_lsp_e2e_fixture();
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
    let changed_res = engine.changed_symbols(&files, dimpact::LanguageMode::Auto);
    let out1_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    let out2_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    std::env::set_current_dir(cwd).unwrap();

    let changed = fail_fast_with_triage("java/callers", "changed_symbols", changed_res);
    let out1 = fail_fast_with_triage("java/callers", "impact#1", out1_res);
    let out2 = fail_fast_with_triage("java/callers", "impact#2", out2_res);

    assert!(changed.changed_symbols.iter().any(|s| s.name == "bar"));
    let names1 = impacted_name_set(&out1);
    let names2 = impacted_name_set(&out2);
    assert_eq!(
        names1, names2,
        "strict Java LSP callers result should be stable"
    );
    assert!(
        names1.contains("foo"),
        "fail-fast: lane=java/callers phase=assertion cause=logic expected impacted caller 'foo', got {:?}",
        names1
    );
}

#[test]
#[serial]
fn lsp_engine_strict_java_callees_chain_e2e_when_available() {
    if !require_env_gate_callers_lane(
        "java/callees",
        "DIMPACT_E2E_STRICT_LSP_JAVA",
        "DIMPACT_E2E_STRICT_LSP_JAVA=1 (or DIMPACT_E2E_STRICT_LSP=1)",
    ) {
        return;
    }
    require_jdtls_for_lane("java/callees");

    let (_tmp, repo) = setup_repo_java_callees_chain_fixture();
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
        direction: dimpact::ImpactDirection::Callees,
        max_depth: Some(5),
        with_edges: Some(false),
        ignore_dirs: Vec::new(),
    };

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let changed_res = engine.changed_symbols(&files, dimpact::LanguageMode::Auto);
    let out1_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    let out2_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    std::env::set_current_dir(cwd).unwrap();

    let changed = fail_fast_with_triage("java/callees", "changed_symbols", changed_res);
    let out1 = fail_fast_with_triage("java/callees", "impact#1", out1_res);
    let out2 = fail_fast_with_triage("java/callees", "impact#2", out2_res);

    assert!(changed.changed_symbols.iter().any(|s| s.name == "b"));
    let names1 = impacted_name_set(&out1);
    let names2 = impacted_name_set(&out2);
    assert_eq!(
        names1, names2,
        "strict Java LSP callees result should be stable"
    );
    assert!(names1.contains("c"));
}

#[test]
#[serial]
fn lsp_engine_strict_java_both_chain_e2e_when_available() {
    if !require_env_gate_callers_lane(
        "java/both",
        "DIMPACT_E2E_STRICT_LSP_JAVA",
        "DIMPACT_E2E_STRICT_LSP_JAVA=1 (or DIMPACT_E2E_STRICT_LSP=1)",
    ) {
        return;
    }
    require_jdtls_for_lane("java/both");

    let (_tmp, repo) = setup_repo_java_both_chain_fixture();
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
        direction: dimpact::ImpactDirection::Both,
        max_depth: Some(5),
        with_edges: Some(false),
        ignore_dirs: Vec::new(),
    };

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let changed_res = engine.changed_symbols(&files, dimpact::LanguageMode::Auto);
    let out1_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    let out2_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    std::env::set_current_dir(cwd).unwrap();

    let changed = fail_fast_with_triage("java/both", "changed_symbols", changed_res);
    let out1 = fail_fast_with_triage("java/both", "impact#1", out1_res);
    let out2 = fail_fast_with_triage("java/both", "impact#2", out2_res);

    assert!(changed.changed_symbols.iter().any(|s| s.name == "foo"));
    let names1 = impacted_name_set(&out1);
    let names2 = impacted_name_set(&out2);
    assert_eq!(
        names1, names2,
        "strict Java LSP both result should be stable"
    );
    assert!(names1.contains("bar") || names1.contains("entry"));
}

#[test]
#[serial]
fn lsp_engine_strict_go_callers_chain_e2e_when_available() {
    if !require_env_gate_callers_lane(
        "go/callers",
        "DIMPACT_E2E_STRICT_LSP_GO",
        "DIMPACT_E2E_STRICT_LSP_GO=1 (or DIMPACT_E2E_STRICT_LSP=1)",
    ) {
        return;
    }
    require_gopls_for_lane("go/callers");

    let (_tmp, repo) = setup_repo_go_real_lsp_e2e_fixture();
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
    let changed_res = engine.changed_symbols(&files, dimpact::LanguageMode::Auto);
    let out1_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    let out2_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    std::env::set_current_dir(cwd).unwrap();

    let changed = fail_fast_with_triage("go/callers", "changed_symbols", changed_res);
    let out1 = fail_fast_with_triage("go/callers", "impact#1", out1_res);
    let out2 = fail_fast_with_triage("go/callers", "impact#2", out2_res);

    assert!(changed.changed_symbols.iter().any(|s| s.name == "bar"));
    let names1 = impacted_name_set(&out1);
    let names2 = impacted_name_set(&out2);
    assert_eq!(
        names1, names2,
        "strict Go LSP callers result should be stable"
    );
    assert!(
        names1.contains("foo"),
        "fail-fast: lane=go/callers phase=assertion cause=logic expected impacted caller 'foo', got {:?}",
        names1
    );
}

#[test]
#[serial]
fn lsp_engine_strict_go_callees_chain_e2e_when_available() {
    if !require_env_gate_callers_lane(
        "go/callees",
        "DIMPACT_E2E_STRICT_LSP_GO",
        "DIMPACT_E2E_STRICT_LSP_GO=1 (or DIMPACT_E2E_STRICT_LSP=1)",
    ) {
        return;
    }
    require_gopls_for_lane("go/callees");

    let (_tmp, repo) = setup_repo_go_callees_chain_fixture();
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
        direction: dimpact::ImpactDirection::Callees,
        max_depth: Some(5),
        with_edges: Some(false),
        ignore_dirs: Vec::new(),
    };

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let changed_res = engine.changed_symbols(&files, dimpact::LanguageMode::Auto);
    let out1_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    let out2_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    std::env::set_current_dir(cwd).unwrap();

    let changed = fail_fast_with_triage("go/callees", "changed_symbols", changed_res);
    let out1 = fail_fast_with_triage("go/callees", "impact#1", out1_res);
    let out2 = fail_fast_with_triage("go/callees", "impact#2", out2_res);

    assert!(changed.changed_symbols.iter().any(|s| s.name == "b"));
    let names1 = impacted_name_set(&out1);
    let names2 = impacted_name_set(&out2);
    assert_eq!(
        names1, names2,
        "strict Go LSP callees result should be stable"
    );
    assert!(names1.contains("c"));
}

#[test]
#[serial]
fn lsp_engine_strict_go_both_chain_e2e_when_available() {
    if !require_env_gate_callers_lane(
        "go/both",
        "DIMPACT_E2E_STRICT_LSP_GO",
        "DIMPACT_E2E_STRICT_LSP_GO=1 (or DIMPACT_E2E_STRICT_LSP=1)",
    ) {
        return;
    }
    require_gopls_for_lane("go/both");

    let (_tmp, repo) = setup_repo_go_both_chain_fixture();
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
        direction: dimpact::ImpactDirection::Both,
        max_depth: Some(5),
        with_edges: Some(false),
        ignore_dirs: Vec::new(),
    };

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let changed_res = engine.changed_symbols(&files, dimpact::LanguageMode::Auto);
    let out1_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    let out2_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    std::env::set_current_dir(cwd).unwrap();

    let changed = fail_fast_with_triage("go/both", "changed_symbols", changed_res);
    let out1 = fail_fast_with_triage("go/both", "impact#1", out1_res);
    let out2 = fail_fast_with_triage("go/both", "impact#2", out2_res);

    assert!(changed.changed_symbols.iter().any(|s| s.name == "foo"));
    let names1 = impacted_name_set(&out1);
    let names2 = impacted_name_set(&out2);
    assert_eq!(names1, names2, "strict Go LSP both result should be stable");
    assert!(names1.contains("bar") || names1.contains("main"));
}

#[test]
#[serial]
fn python_real_lsp_e2e_fixture_is_opt_in_gated() {
    if !should_run_python_strict_lsp_e2e() {
        eprintln!(
            "skip: set DIMPACT_E2E_STRICT_LSP_PYTHON=1 (or DIMPACT_E2E_STRICT_LSP=1) to run Python strict LSP e2e fixture"
        );
        return;
    }
    require_python_lsp_server_for_lane("python/preflight-fixture");

    let (_tmp, repo) = setup_repo_python_real_lsp_e2e_fixture();
    let diff_out = git(&repo, &["diff", "--no-ext-diff", "--unified=0"]);
    let diff = String::from_utf8(diff_out.stdout).unwrap();
    let files = dimpact::parse_unified_diff(&diff).unwrap();

    assert_eq!(files.len(), 1);
    assert_eq!(files[0].new_path.as_deref(), Some("main.py"));
    assert!(
        files[0]
            .changes
            .iter()
            .any(|c| matches!(c.kind, dimpact::ChangeKind::Added)),
        "fixture should include added lines for changed detection"
    );

    let pyproject = fs::read_to_string(repo.join("pyproject.toml")).expect("read pyproject");
    assert!(pyproject.contains("name = \"lsp-python-fixture\""));
}

#[test]
#[serial]
fn lsp_engine_strict_python_callers_chain_e2e_when_available() {
    if !require_env_gate_callers_lane(
        "python/callers",
        "DIMPACT_E2E_STRICT_LSP_PYTHON",
        "DIMPACT_E2E_STRICT_LSP_PYTHON=1 (or DIMPACT_E2E_STRICT_LSP=1)",
    ) {
        return;
    }
    require_python_lsp_server_for_lane("python/callers");

    let (_tmp, repo) = setup_repo_python_real_lsp_e2e_fixture();
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
    let changed_res = engine.changed_symbols(&files, dimpact::LanguageMode::Auto);
    let out1_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    let out2_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    std::env::set_current_dir(cwd).unwrap();

    let changed = fail_fast_with_triage("python/callers", "changed_symbols", changed_res);
    let out1 = fail_fast_with_triage("python/callers", "impact#1", out1_res);
    let out2 = fail_fast_with_triage("python/callers", "impact#2", out2_res);

    assert!(changed.changed_symbols.iter().any(|s| s.name == "bar"));
    let names1 = impacted_name_set(&out1);
    let names2 = impacted_name_set(&out2);
    assert_eq!(
        names1, names2,
        "strict python LSP callers result should be stable"
    );
    assert!(
        names1.contains("foo"),
        "fail-fast: lane=python/callers phase=assertion cause=logic expected impacted caller 'foo', got {:?}",
        names1
    );
}

#[test]
#[serial]
fn lsp_engine_strict_python_callees_chain_e2e_when_available() {
    if !should_run_python_strict_lsp_e2e() {
        eprintln!(
            "skip: set DIMPACT_E2E_STRICT_LSP_PYTHON=1 (or DIMPACT_E2E_STRICT_LSP=1) to run Python strict LSP e2e tests"
        );
        return;
    }
    require_python_lsp_server_for_lane("python/callees");

    let (_tmp, repo) = setup_repo_python_callees_chain_fixture();
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
        direction: dimpact::ImpactDirection::Callees,
        max_depth: Some(5),
        with_edges: Some(false),
        ignore_dirs: Vec::new(),
    };

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let changed_res = engine.changed_symbols(&files, dimpact::LanguageMode::Auto);
    let out1_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    let out2_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    std::env::set_current_dir(cwd).unwrap();

    let changed = fail_fast_with_triage("python/callees", "changed_symbols", changed_res);
    let out1 = fail_fast_with_triage("python/callees", "impact#1", out1_res);
    let out2 = fail_fast_with_triage("python/callees", "impact#2", out2_res);

    assert!(changed.changed_symbols.iter().any(|s| s.name == "foo"));
    let names1 = impacted_name_set(&out1);
    let names2 = impacted_name_set(&out2);
    assert_eq!(
        names1, names2,
        "strict python LSP callees result should be stable"
    );
    assert!(names1.contains("bar") || names1.contains("baz"));
}

#[test]
#[serial]
fn lsp_engine_strict_python_both_chain_e2e_when_available() {
    if !should_run_python_strict_lsp_e2e() {
        eprintln!(
            "skip: set DIMPACT_E2E_STRICT_LSP_PYTHON=1 (or DIMPACT_E2E_STRICT_LSP=1) to run Python strict LSP e2e tests"
        );
        return;
    }
    require_python_lsp_server_for_lane("python/both");

    let (_tmp, repo) = setup_repo_python_callees_chain_fixture();
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
        direction: dimpact::ImpactDirection::Both,
        max_depth: Some(5),
        with_edges: Some(false),
        ignore_dirs: Vec::new(),
    };

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let changed_res = engine.changed_symbols(&files, dimpact::LanguageMode::Auto);
    let out1_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    let out2_res = engine.impact(&files, dimpact::LanguageMode::Auto, &opts);
    std::env::set_current_dir(cwd).unwrap();

    let changed = fail_fast_with_triage("python/both", "changed_symbols", changed_res);
    let out1 = fail_fast_with_triage("python/both", "impact#1", out1_res);
    let out2 = fail_fast_with_triage("python/both", "impact#2", out2_res);

    assert!(changed.changed_symbols.iter().any(|s| s.name == "foo"));
    let names1 = impacted_name_set(&out1);
    let names2 = impacted_name_set(&out2);
    assert_eq!(
        names1, names2,
        "strict python LSP both result should be stable"
    );
    assert!(names1.contains("bar") || names1.contains("baz") || names1.contains("main"));
}

#[test]
#[serial]
fn lsp_engine_strict_callers_chain_is_stable_when_available() {
    if !require_strict_lsp_env_gate_for_lane("rust/callers") {
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
    let out1_res = engine.impact(&files, dimpact::LanguageMode::Rust, &opts);
    let out2_res = engine.impact(&files, dimpact::LanguageMode::Rust, &opts);
    std::env::set_current_dir(cwd).unwrap();

    let out1 = fail_fast_with_triage("rust/callers", "impact#1", out1_res);
    let out2 = fail_fast_with_triage("rust/callers", "impact#2", out2_res);

    let names1 = impacted_name_set(&out1);
    let names2 = impacted_name_set(&out2);
    assert!(
        names1.contains("foo"),
        "fail-fast: lane=rust/callers phase=assertion cause=logic expected impacted caller 'foo', got {:?}",
        names1
    );
    assert!(
        names1.contains("main"),
        "fail-fast: lane=rust/callers phase=assertion cause=logic expected impacted caller 'main', got {:?}",
        names1
    );
    assert_eq!(names1, names2, "strict LSP result should be stable");
}

#[test]
#[serial]
fn lsp_engine_strict_rust_callees_chain_e2e_when_available() {
    if !require_strict_lsp_env_gate_for_lane("rust/callees") {
        return;
    }
    if !has_rust_analyzer() {
        eprintln!("skip: rust-analyzer not available");
        return;
    }

    let initial = "fn bar() -> i32 { 1 }\nfn baz() -> i32 { 2 }\nfn foo() -> i32 { bar() + baz() }\nfn main() { let _ = foo(); }\n";
    let updated = "fn bar() -> i32 { 1 }\nfn baz() -> i32 { 2 }\nfn foo() -> i32 { let x = 1; bar() + baz() + x }\nfn main() { let _ = foo(); }\n";
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
        direction: dimpact::ImpactDirection::Callees,
        max_depth: Some(5),
        with_edges: Some(false),
        ignore_dirs: Vec::new(),
    };

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let out1_res = engine.impact(&files, dimpact::LanguageMode::Rust, &opts);
    let out2_res = engine.impact(&files, dimpact::LanguageMode::Rust, &opts);
    std::env::set_current_dir(cwd).unwrap();

    let out1 = fail_fast_with_triage("rust/callees", "impact#1", out1_res);
    let out2 = fail_fast_with_triage("rust/callees", "impact#2", out2_res);

    let names1 = impacted_name_set(&out1);
    let names2 = impacted_name_set(&out2);
    assert_eq!(
        names1, names2,
        "strict rust LSP callees result should be stable"
    );
    assert!(
        names1.contains("bar") || names1.contains("baz"),
        "fail-fast: lane=rust/callees phase=assertion cause=logic expected impacted callee 'bar' or 'baz', got {:?}",
        names1
    );
}

#[test]
#[serial]
fn lsp_engine_strict_rust_both_chain_e2e_when_available() {
    if !require_strict_lsp_env_gate_for_lane("rust/both") {
        return;
    }
    if !has_rust_analyzer() {
        eprintln!("skip: rust-analyzer not available");
        return;
    }

    let initial = "fn bar() -> i32 { 1 }\nfn baz() -> i32 { 2 }\nfn foo() -> i32 { bar() + baz() }\nfn main() { let _ = foo(); }\n";
    let updated = "fn bar() -> i32 { 1 }\nfn baz() -> i32 { 2 }\nfn foo() -> i32 { let x = 1; bar() + baz() + x }\nfn main() { let _ = foo(); }\n";
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
        direction: dimpact::ImpactDirection::Both,
        max_depth: Some(5),
        with_edges: Some(false),
        ignore_dirs: Vec::new(),
    };

    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&repo).unwrap();
    let out1_res = engine.impact(&files, dimpact::LanguageMode::Rust, &opts);
    let out2_res = engine.impact(&files, dimpact::LanguageMode::Rust, &opts);
    std::env::set_current_dir(cwd).unwrap();

    let out1 = fail_fast_with_triage("rust/both", "impact#1", out1_res);
    let out2 = fail_fast_with_triage("rust/both", "impact#2", out2_res);

    let names1 = impacted_name_set(&out1);
    let names2 = impacted_name_set(&out2);
    assert_eq!(
        names1, names2,
        "strict rust LSP both result should be stable"
    );
    assert!(
        names1.contains("bar") || names1.contains("baz") || names1.contains("main"),
        "fail-fast: lane=rust/both phase=assertion cause=logic expected impacted symbols to include bar/baz/main, got {:?}",
        names1
    );
}

#[test]
#[serial]
fn lsp_engine_strict_methods_chain_resolves_callers_when_available() {
    if !require_strict_lsp_env_gate_for_lane("rust/method-callers") {
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
    let out_res = engine.impact(&files, dimpact::LanguageMode::Rust, &opts);
    std::env::set_current_dir(cwd).unwrap();

    let out = fail_fast_with_triage("rust/method-callers", "impact#1", out_res);

    let names = impacted_name_set(&out);
    assert!(
        names.contains("foo"),
        "fail-fast: lane=rust/method-callers phase=assertion cause=logic expected impacted caller 'foo', got {:?}",
        names
    );
}
