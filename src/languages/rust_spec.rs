use crate::ir::{Symbol, SymbolId, SymbolKind, TextRange};
use crate::ir::reference::{RefKind, UnresolvedRef};
use crate::languages::{LanguageAnalyzer, rust::RustAnalyzer};
use crate::languages::rust_ts::RustTsAnalyzer;
use crate::ts_core::{load_rust_spec, compile_queries_rust, QueryRunner, Capture};

pub struct SpecRustAnalyzer {
    spec: crate::ts_core::Spec,
    queries: crate::ts_core::CompiledQueries,
    runner: QueryRunner,
}

impl SpecRustAnalyzer {
    pub fn new() -> Self {
        let spec = load_rust_spec();
        let queries = compile_queries_rust(&spec).expect("compile rust queries");
        let runner = QueryRunner::new_rust();
        Self { spec, queries, runner }
    }
}

fn line_lookup(src: &str) -> Vec<usize> {
    let mut offs = vec![0usize];
    for (i, b) in src.bytes().enumerate() { if b == b'\n' { offs.push(i+1); } }
    offs
}

fn byte_to_line(offs: &[usize], byte: usize) -> u32 {
    match offs.binary_search(&byte) { Ok(i) => (i as u32) + 1, Err(i) => i as u32 }
}

impl LanguageAnalyzer for SpecRustAnalyzer {
    fn language(&self) -> &'static str { "rust" }

    fn symbols_in_file(&self, path: &str, source: &str) -> Vec<Symbol> {
        // For reliability (methods inside impl), reuse the existing TS analyzer's symbol extraction
        RustTsAnalyzer::new().symbols_in_file(path, source)
    }

    fn unresolved_refs(&self, path: &str, source: &str) -> Vec<UnresolvedRef> {
        let mut out = Vec::new();
        let offs = line_lookup(source);
        for caps in self.runner.run_captures(source, &self.queries.calls) {
            let name_cap = caps.iter().find(|c| c.name == "name");
            let qname_cap = caps.iter().find(|c| c.name == "qname");
            let method_cap = caps.iter().find(|c| c.name == "method");
            let ln = byte_to_line(&offs, caps.first().map(|c| c.start).unwrap_or(0));
            if let Some(n) = method_cap.or(name_cap) {
                let name = &source.as_bytes()[n.start..n.end];
                let name = std::str::from_utf8(name).unwrap_or("");
                if name.is_empty() || name.ends_with('!') { continue; }
                out.push(UnresolvedRef { name: name.to_string(), kind: RefKind::Call, file: path.to_string(), line: ln, qualifier: None, is_method: method_cap.is_some() });
                continue;
            }
            if let Some(q) = qname_cap {
                let txt = &source.as_bytes()[q.start..q.end];
                let txt = std::str::from_utf8(txt).unwrap_or("");
                let parts: Vec<&str> = txt.split("::").collect();
                if let Some((last, rest)) = parts.split_last() {
                    let qualifier = if rest.is_empty() { None } else { Some(rest.join("::")) };
                    out.push(UnresolvedRef { name: (*last).to_string(), kind: RefKind::Call, file: path.to_string(), line: ln, qualifier, is_method: false });
                }
            }
        }
        out
    }

    fn imports_in_file(&self, path: &str, source: &str) -> std::collections::HashMap<String, String> {
        // reuse robust regex-based import parser for now
        RustAnalyzer::new().imports_in_file(path, source)
    }
}
