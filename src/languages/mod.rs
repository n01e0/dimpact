use crate::ir::Symbol;
use crate::ir::reference::UnresolvedRef;

pub trait LanguageAnalyzer {
    fn language(&self) -> &'static str;
    fn symbols_in_file(&self, path: &str, source: &str) -> Vec<Symbol>;
    fn unresolved_refs(&self, path: &str, source: &str) -> Vec<UnresolvedRef>;
}

pub mod rust;
