use serde::{Deserialize, Serialize};
use crate::ir::{Symbol, SymbolId};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RefKind {
    Call,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UnresolvedRef {
    pub name: String,
    pub kind: RefKind,
    pub file: String,
    pub line: u32,
    pub qualifier: Option<String>, // e.g., "a::b" for a::b::name()
    pub is_method: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Reference {
    pub from: SymbolId,
    pub to: SymbolId,
    pub kind: RefKind,
    pub file: String,
    pub line: u32,
}

#[derive(Debug, Default, Clone)]
pub struct SymbolIndex {
    pub symbols: Vec<Symbol>,
    pub by_name: std::collections::HashMap<String, Vec<Symbol>>, // name -> symbols
    pub by_file: std::collections::HashMap<String, Vec<Symbol>>, // file -> symbols
}

impl SymbolIndex {
    pub fn build(symbols: Vec<Symbol>) -> Self {
        let mut by_name = std::collections::HashMap::new();
        let mut by_file = std::collections::HashMap::new();
        for s in symbols.iter() {
            by_name.entry(s.name.clone()).or_insert_with(Vec::new).push(s.clone());
            by_file.entry(s.file.clone()).or_insert_with(Vec::new).push(s.clone());
        }
        Self { symbols, by_name, by_file }
    }

    pub fn enclosing_symbol(&self, file: &str, line: u32) -> Option<&Symbol> {
        self.by_file.get(file)?.iter().find(|s| s.range.start_line <= line && line <= s.range.end_line)
    }
}
