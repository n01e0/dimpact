use crate::ir::{Symbol, SymbolId};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum EdgeCertainty {
    #[default]
    Confirmed,
    Inferred,
    DynamicFallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    pub from: SymbolId,
    pub to: SymbolId,
    pub kind: RefKind,
    pub file: String,
    pub line: u32,
    pub certainty: EdgeCertainty,
}

#[derive(Serialize)]
struct ReferenceSer<'a> {
    from: &'a SymbolId,
    to: &'a SymbolId,
    kind: &'a RefKind,
    file: &'a str,
    line: u32,
    certainty: &'a EdgeCertainty,
    confidence: &'a EdgeCertainty,
}

#[derive(Deserialize)]
struct ReferenceDe {
    from: SymbolId,
    to: SymbolId,
    kind: RefKind,
    file: String,
    line: u32,
    #[serde(default)]
    certainty: Option<EdgeCertainty>,
    #[serde(default)]
    confidence: Option<EdgeCertainty>,
}

impl Serialize for Reference {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ReferenceSer {
            from: &self.from,
            to: &self.to,
            kind: &self.kind,
            file: &self.file,
            line: self.line,
            certainty: &self.certainty,
            confidence: &self.certainty,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Reference {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = ReferenceDe::deserialize(deserializer)?;
        Ok(Self {
            from: raw.from,
            to: raw.to,
            kind: raw.kind,
            file: raw.file,
            line: raw.line,
            certainty: raw.certainty.or(raw.confidence).unwrap_or_default(),
        })
    }
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
            by_name
                .entry(s.name.clone())
                .or_insert_with(Vec::new)
                .push(s.clone());
            by_file
                .entry(s.file.clone())
                .or_insert_with(Vec::new)
                .push(s.clone());
        }
        Self {
            symbols,
            by_name,
            by_file,
        }
    }

    pub fn enclosing_symbol(&self, file: &str, line: u32) -> Option<&Symbol> {
        self.by_file
            .get(file)?
            .iter()
            .filter(|s| s.range.start_line <= line && line <= s.range.end_line)
            .min_by_key(|s| {
                let span = s.range.end_line.saturating_sub(s.range.start_line);
                (span, u32::MAX.saturating_sub(s.range.start_line))
            })
    }
}
