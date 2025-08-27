use crate::ir::{Symbol, SymbolId, SymbolKind, TextRange};
use crate::ir::reference::{RefKind, UnresolvedRef};
use crate::languages::LanguageAnalyzer;
use crate::ts_core::{load_javascript_spec, compile_queries_javascript, QueryRunner, Capture};

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

fn line_lookup(src: &str) -> Vec<usize> { let mut offs=vec![0usize]; for (i,b) in src.bytes().enumerate(){ if b==b'\n'{offs.push(i+1);} } offs }
fn byte_to_line(offs: &[usize], byte: usize) -> u32 { match offs.binary_search(&byte){ Ok(i)=>(i as u32)+1, Err(i)=> i as u32 } }

impl LanguageAnalyzer for SpecJsAnalyzer {
    fn language(&self) -> &'static str { "javascript" }

    fn symbols_in_file(&self, path: &str, source: &str) -> Vec<Symbol> {
        let offs = line_lookup(source);
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
        let offs = line_lookup(source);
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
                out.push(UnresolvedRef { name, kind: RefKind::Call, file: path.to_string(), line: ln, qualifier: None, is_method });
            }
        }
        out
    }

    fn imports_in_file(&self, path: &str, source: &str) -> std::collections::HashMap<String, String> {
        use regex::Regex;
        let mut map = std::collections::HashMap::new();
        let re_import = Regex::new(r#"(?m)^\s*import[^{]*from\s+['\"]([^'\"]+)['\"]"#).unwrap();
        let re_require = Regex::new(r#"(?m)require\s*\(\s*['\"]([^'\"]+)['\"]\s*\)"#).unwrap();
        for cap in re_import.captures_iter(source) {
            let raw = cap.get(1).unwrap().as_str();
            if let Some(norm) = normalize_es_module_path(path, raw) {
                map.insert(format!("__glob__{}", norm), norm);
            }
        }
        for cap in re_require.captures_iter(source) {
            let raw = cap.get(1).unwrap().as_str();
            if let Some(norm) = normalize_es_module_path(path, raw) {
                map.insert(format!("__glob__{}", norm), norm);
            }
        }
        map
    }
}

fn normalize_es_module_path(cur_file: &str, raw: &str) -> Option<String> {
    // Handle relative like './x', '../x', or bare 'pkg' (keep as-is without dot/leading slash)
    let mut s = raw.trim().trim_end_matches(".js").trim_end_matches(".mjs").trim_end_matches(".cjs");
    s = s.trim_end_matches(".ts").trim_end_matches(".tsx");
    let s = s.replace('\\', "/");
    if s.starts_with("./") || s.starts_with("../") {
        let base = std::path::Path::new(cur_file).parent().unwrap_or_else(|| std::path::Path::new("."));
        let joined = base.join(s);
        Some(normalize_path_like(&joined))
    } else {
        // bare specifier — return as-is; resolver would need load paths
        Some(s.trim_start_matches('/').to_string())
    }
}

fn normalize_path_like(p: &std::path::Path) -> String {
    use std::path::{Component, PathBuf};
    let mut out = PathBuf::new();
    for comp in p.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => { out.pop(); }
            other => out.push(other.as_os_str()),
        }
    }
    out.to_string_lossy().replace('\\', "/")
}
