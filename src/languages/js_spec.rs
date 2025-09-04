use crate::ir::{Symbol, SymbolId, SymbolKind, TextRange};
use crate::ir::reference::{RefKind, UnresolvedRef};
use crate::languages::LanguageAnalyzer;
use crate::ts_core::{load_javascript_spec, compile_queries_javascript, QueryRunner, Capture};
use crate::languages::path::{normalize_path_like, resolve_module_path};
use crate::languages::util::{line_offsets, byte_to_line};

pub struct SpecJsAnalyzer {
    queries: crate::ts_core::CompiledQueries,
    runner: QueryRunner,
}

impl SpecJsAnalyzer {
    pub fn new() -> Self {
        let spec = load_javascript_spec();
        let queries = compile_queries_javascript(&spec).expect("compile js queries");
        let runner = QueryRunner::new_javascript();
        Self { queries, runner }
    }
}

// Import line_offsets from util
// fn line_lookup(src: &str) -> Vec<usize> { ... }

impl LanguageAnalyzer for SpecJsAnalyzer {
    fn language(&self) -> &'static str { "javascript" }

    fn symbols_in_file(&self, path: &str, source: &str) -> Vec<Symbol> {
        let offs = line_offsets(source);
        let mut out = Vec::new();
        for caps in self.runner.run_captures(source, &self.queries.decl) {
            if let Some(nc) = caps.iter().find(|c| c.name == "name") {
                let name = std::str::from_utf8(&source.as_bytes()[nc.start..nc.end]).unwrap_or("");
                if name.is_empty() { continue; }
                let kind = match caps.iter().find(|c| c.name == "decl").map(|d| d.kind.as_str()) {
                    Some("class_declaration") => SymbolKind::Struct,
                    Some("method_definition") => SymbolKind::Method,
                    _ => SymbolKind::Function,
                };
                let sl = byte_to_line(&offs, nc.start);
                let el = byte_to_line(&offs, nc.end.saturating_sub(1)).max(sl);
                out.push(Symbol { id: SymbolId::new("javascript", path, &kind, name, sl), name: name.to_string(), kind, file: path.to_string(), range: TextRange { start_line: sl, end_line: el }, language: "javascript".to_string() });
            }
        }
        out
    }

    fn unresolved_refs(&self, path: &str, source: &str) -> Vec<UnresolvedRef> {
        let offs = line_offsets(source);
        let mut out = Vec::new();
        for caps in self.runner.run_captures(source, &self.queries.calls) {
            // prefer property_identifier if present else identifier
            let name_cap = caps.iter().find(|c| c.name == "name");
            if let Some(n) = name_cap {
                let name = std::str::from_utf8(&source.as_bytes()[n.start..n.end]).unwrap_or("").to_string();
                if name.is_empty() { continue; }
                // consider member call by checking if any capture kind is member_expression
                let is_method = caps.iter().any(|c| c.kind == "member_expression");
                let ln = byte_to_line(&offs, n.start);
                let qual = caps.iter().find(|c| c.name == "qual").map(|q| std::str::from_utf8(&source.as_bytes()[q.start..q.end]).unwrap_or("").to_string());
                out.push(UnresolvedRef { name, kind: RefKind::Call, file: path.to_string(), line: ln, qualifier: qual.filter(|s| !s.is_empty()), is_method });
            }
        }
        out
    }

    fn imports_in_file(&self, path: &str, source: &str) -> std::collections::HashMap<String, String> {
        use regex::Regex;
        let mut map = std::collections::HashMap::new();
        let re_from = Regex::new(r#"(?m)^\s*import\s+(.+?)\s+from\s+['\"]([^'\"]+)['\"]"#).unwrap();
        let re_require = Regex::new(r#"(?m)require\s*\(\s*['\"]([^'\"]+)['\"]\s*\)"#).unwrap();
        for cap in re_from.captures_iter(source) {
            let head = cap.get(1).unwrap().as_str().trim();
            let raw = cap.get(2).unwrap().as_str();
            if let Some(norm) = normalize_es_module_path(path, raw) {
                // glob prefixes for module and module/index
                map.insert(format!("__glob__{}", norm.clone()), norm.clone());
                map.insert(format!("__glob__{}", format!("{}/index", norm)), format!("{}/index", norm));
                // namespace import: * as A
                if let Some(ns) = head.strip_prefix("* as ") {
                    let alias = ns.trim();
                    map.insert(alias.to_string(), norm.clone());
                }
                // default import: A from 'mod'
                if head.starts_with(|c: char| c.is_alphabetic() || c == '_' || c == '$') && !head.starts_with('{') {
                    if let Some(first) = head.split(',').next() {
                        let alias = first.trim();
                        if !alias.is_empty() { map.insert(alias.to_string(), format!("{}::default", norm)); }
                    }
                }
                // named imports: { a as b, c }
                if head.starts_with('{') {
                    let inner = head.trim().trim_start_matches('{').trim_end_matches('}');
                    for seg in inner.split(',') {
                        let seg = seg.trim(); if seg.is_empty() { continue; }
                        if let Some((orig, alias)) = seg.split_once(" as ") {
                            map.insert(alias.trim().to_string(), format!("{}::{}", norm, orig.trim()));
                        } else {
                            map.insert(seg.to_string(), format!("{}::{}", norm, seg));
                        }
                    }
                }
            }
        }
        for cap in re_require.captures_iter(source) {
            let raw = cap.get(1).unwrap().as_str();
            if let Some(norm) = normalize_es_module_path(path, raw) {
                map.insert(format!("__glob__{}", norm.clone()), norm.clone());
                map.insert(format!("__glob__{}", format!("{}/index", norm)), format!("{}/index", norm));
            }
        }
        map
    }
}

fn normalize_es_module_path(cur_file: &str, raw: &str) -> Option<String> {
    // Supported JS/TS extensions
    let exts = [".js", ".mjs", ".cjs", ".ts", ".tsx"];
    resolve_module_path(cur_file, raw, &exts)
}
