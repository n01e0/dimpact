use crate::ir::Symbol;
use crate::ir::reference::UnresolvedRef;

pub trait LanguageAnalyzer {
    fn language(&self) -> &'static str;
    fn symbols_in_file(&self, path: &str, source: &str) -> Vec<Symbol>;
    fn unresolved_refs(&self, path: &str, source: &str) -> Vec<UnresolvedRef>;
    fn imports_in_file(
        &self,
        _path: &str,
        _source: &str,
    ) -> std::collections::HashMap<String, String> {
        Default::default()
    }
}

pub mod go_spec;
pub mod java_spec;
pub mod js_spec;
pub mod path;
pub mod py_spec;
pub mod ruby_spec;
pub mod rust;
pub mod rust_spec;
pub mod rust_ts;
pub mod ts_spec;
pub mod util;

// TS-only now

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageKind {
    Auto,
    Rust,
    Ruby,
    Javascript,
    Typescript,
    Tsx,
    Go,
    Java,
}

pub fn analyzer_for_path(path: &str, lang: LanguageKind) -> Option<Box<dyn LanguageAnalyzer>> {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let target = match lang {
        LanguageKind::Rust => "rs",
        LanguageKind::Ruby => "rb",
        LanguageKind::Javascript => "js",
        LanguageKind::Typescript => "ts",
        LanguageKind::Tsx => "tsx",
        LanguageKind::Go => "go",
        LanguageKind::Java => "java",
        LanguageKind::Auto => ext,
    };
    match target {
        "rs" => Some(Box::new(rust_spec::SpecRustAnalyzer::new())),
        "rb" => Some(Box::new(ruby_spec::SpecRubyAnalyzer::new())),
        "js" => Some(Box::new(js_spec::SpecJsAnalyzer::new())),
        "ts" => Some(Box::new(ts_spec::SpecTsAnalyzer::new_ts())),
        "tsx" => Some(Box::new(ts_spec::SpecTsAnalyzer::new_tsx())),
        "go" => Some(Box::new(go_spec::SpecGoAnalyzer::new())),
        "java" => Some(Box::new(java_spec::SpecJavaAnalyzer::new())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{LanguageKind, analyzer_for_path};

    #[test]
    fn analyzer_for_path_recognizes_go_java_extensions() {
        assert!(analyzer_for_path("main.go", LanguageKind::Auto).is_some());
        assert!(analyzer_for_path("Main.java", LanguageKind::Auto).is_some());
        assert!(analyzer_for_path("main.any", LanguageKind::Go).is_some());
        assert!(analyzer_for_path("main.any", LanguageKind::Java).is_some());
    }

    #[test]
    fn analyzer_for_path_existing_languages_unchanged() {
        assert!(analyzer_for_path("src/lib.rs", LanguageKind::Auto).is_some());
        assert!(analyzer_for_path("app/main.rb", LanguageKind::Auto).is_some());
        assert!(analyzer_for_path("web/main.js", LanguageKind::Auto).is_some());
        assert!(analyzer_for_path("web/main.ts", LanguageKind::Auto).is_some());
        assert!(analyzer_for_path("web/main.tsx", LanguageKind::Auto).is_some());

        // Explicit language modes should keep previous behavior independent of extension.
        assert!(analyzer_for_path("x.any", LanguageKind::Rust).is_some());
        assert!(analyzer_for_path("x.any", LanguageKind::Ruby).is_some());
        assert!(analyzer_for_path("x.any", LanguageKind::Javascript).is_some());
        assert!(analyzer_for_path("x.any", LanguageKind::Typescript).is_some());
        assert!(analyzer_for_path("x.any", LanguageKind::Tsx).is_some());
    }

    #[test]
    fn analyzer_for_path_uses_go_java_implementations_not_fallback() {
        let go = analyzer_for_path("main.go", LanguageKind::Auto).expect("go analyzer");
        assert_eq!(go.language(), "go");
        let go_syms = go.symbols_in_file("main.go", "package main\nfunc main() {}\n");
        assert!(go_syms.iter().any(|s| s.name == "main"));

        let java = analyzer_for_path("Main.java", LanguageKind::Auto).expect("java analyzer");
        assert_eq!(java.language(), "java");
        let java_syms = java.symbols_in_file(
            "Main.java",
            "class Main {\n    static void main(String[] args) {\n    }\n}\n",
        );
        assert!(java_syms.iter().any(|s| s.name == "Main"));
        assert!(java_syms.iter().any(|s| s.name == "main"));
    }
}
