use crate::ir::Symbol;
use crate::ir::reference::UnresolvedRef;

pub trait LanguageAnalyzer {
    fn language(&self) -> &'static str;
    fn symbols_in_file(&self, path: &str, source: &str) -> Vec<Symbol>;
    fn unresolved_refs(&self, path: &str, source: &str) -> Vec<UnresolvedRef>;
    fn imports_in_file(&self, _path: &str, _source: &str) -> std::collections::HashMap<String, String> { Default::default() }
}

pub mod rust;
#[cfg(feature = "ts")]
pub mod rust_ts;

#[derive(Debug, Clone, Copy)]
pub enum Engine { Regex, Ts }

pub fn rust_analyzer(engine: Engine) -> Box<dyn LanguageAnalyzer> {
    match engine {
        Engine::Regex => Box::new(rust::RustAnalyzer::new()),
        Engine::Ts => {
            #[cfg(feature = "ts")]
            { return Box::new(rust_ts::RustTsAnalyzer::new()); }
            #[cfg(not(feature = "ts"))]
            { return Box::new(rust::RustAnalyzer::new()); }
        }
    }
}
