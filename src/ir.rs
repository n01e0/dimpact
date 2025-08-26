use serde::{Deserialize, Serialize};

pub mod reference;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct TextRange {
    pub start_line: u32, // 1-based inclusive
    pub end_line: u32,   // 1-based inclusive
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    Function,
    Method,
    Struct,
    Enum,
    Trait,
    Module,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SymbolId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Symbol {
    pub id: SymbolId,
    pub name: String,
    pub kind: SymbolKind,
    pub file: String,
    pub range: TextRange,
    pub language: String,
}

impl SymbolId {
    pub fn new(lang: &str, file: &str, kind: &SymbolKind, name: &str, start_line: u32) -> Self {
        let k = match kind {
            SymbolKind::Function => "fn",
            SymbolKind::Method => "method",
            SymbolKind::Struct => "struct",
            SymbolKind::Enum => "enum",
            SymbolKind::Trait => "trait",
            SymbolKind::Module => "mod",
        };
        Self(format!("{}:{}:{}:{}:{}", lang, file, k, name, start_line))
    }
}

