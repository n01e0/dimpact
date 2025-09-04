use crate::ir::Symbol;
use crate::ir::reference::UnresolvedRef;

pub trait LanguageAnalyzer {
    fn language(&self) -> &'static str;
    fn symbols_in_file(&self, path: &str, source: &str) -> Vec<Symbol>;
    fn unresolved_refs(&self, path: &str, source: &str) -> Vec<UnresolvedRef>;
    fn imports_in_file(&self, _path: &str, _source: &str) -> std::collections::HashMap<String, String> { Default::default() }
}

pub mod rust;
pub mod rust_ts;
pub mod rust_spec;
pub mod ruby_spec;
pub mod ts_spec;
pub mod js_spec;
pub mod util;
pub mod path;

// TS-only now

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageKind { Auto, Rust, Ruby, Javascript, Typescript, Tsx }

pub fn analyzer_for_path(path: &str, lang: LanguageKind) -> Option<Box<dyn LanguageAnalyzer>> {
    let ext = std::path::Path::new(path).extension().and_then(|s| s.to_str()).unwrap_or("");
    let target = match lang {
        LanguageKind::Rust => "rs",
        LanguageKind::Ruby => "rb",
        LanguageKind::Javascript => "js",
        LanguageKind::Typescript => "ts",
        LanguageKind::Tsx => "tsx",
        LanguageKind::Auto => ext,
    };
    match target {
        "rs" => Some(Box::new(rust_spec::SpecRustAnalyzer::new())),
        "rb" => Some(Box::new(ruby_spec::SpecRubyAnalyzer::new())),
        "js" => Some(Box::new(js_spec::SpecJsAnalyzer::new())),
        "ts" => Some(Box::new(ts_spec::SpecTsAnalyzer::new_ts())),
        "tsx" => Some(Box::new(ts_spec::SpecTsAnalyzer::new_tsx())),
        _ => None,
    }
}
