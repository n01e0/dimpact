use crate::ir::reference::{RefKind, UnresolvedRef};
use crate::ir::{Symbol, SymbolId, SymbolKind, TextRange};
use crate::languages::LanguageAnalyzer;
use crate::languages::path::resolve_module_path;
use crate::languages::util::{byte_to_line, line_offsets};
use crate::ts_core::{QueryRunner, compile_queries_typescript, load_typescript_spec};

pub struct SpecTsAnalyzer {
    queries: crate::ts_core::CompiledQueries,
    runner: QueryRunner,
    tsx: bool,
}

impl SpecTsAnalyzer {
    pub fn new_ts() -> Self {
        let spec = load_typescript_spec();
        let queries = compile_queries_typescript(&spec, false).expect("compile ts queries");
        let runner = QueryRunner::new_typescript(false);
        Self {
            queries,
            runner,
            tsx: false,
        }
    }
    pub fn new_tsx() -> Self {
        let spec = load_typescript_spec();
        let queries = compile_queries_typescript(&spec, true).expect("compile tsx queries");
        let runner = QueryRunner::new_typescript(true);
        Self {
            queries,
            runner,
            tsx: true,
        }
    }
}

impl LanguageAnalyzer for SpecTsAnalyzer {
    fn language(&self) -> &'static str {
        if self.tsx { "tsx" } else { "typescript" }
    }

    fn symbols_in_file(&self, path: &str, source: &str) -> Vec<Symbol> {
        let offs = line_offsets(source);
        let mut out = Vec::new();
        for caps in self.runner.run_captures(source, &self.queries.decl) {
            if let Some(nc) = caps.iter().find(|c| c.name == "name") {
                let name = &source[nc.start..nc.end];
                if name.is_empty() {
                    continue;
                }
                let decl_cap = caps.iter().find(|c| c.name == "decl");
                let kind = match decl_cap.map(|d| d.kind.as_str()) {
                    Some("class_declaration") => SymbolKind::Struct,
                    Some("method_definition") | Some("method_signature") => SymbolKind::Method,
                    _ => SymbolKind::Function,
                };
                let (sl, el) = if let Some(dc) = decl_cap {
                    (
                        byte_to_line(&offs, dc.start),
                        byte_to_line(&offs, dc.end.saturating_sub(1))
                            .max(byte_to_line(&offs, dc.start)),
                    )
                } else {
                    (
                        byte_to_line(&offs, nc.start),
                        byte_to_line(&offs, nc.end.saturating_sub(1))
                            .max(byte_to_line(&offs, nc.start)),
                    )
                };
                out.push(Symbol {
                    id: SymbolId::new(self.language(), path, &kind, name, sl),
                    name: name.to_string(),
                    kind,
                    file: path.to_string(),
                    range: TextRange {
                        start_line: sl,
                        end_line: el,
                    },
                    language: self.language().to_string(),
                });
            }
        }
        // Fallback: CommonJS function expressions for TS allowJs environments
        use regex::Regex;
        fn find_block_end(src: &str, start_idx: usize) -> usize {
            let bytes = src.as_bytes();
            let mut i = start_idx;
            while i < bytes.len() && bytes[i] != b'{' {
                i += 1;
            }
            if i >= bytes.len() {
                return start_idx;
            }
            let mut depth = 0i32;
            while i < bytes.len() {
                let b = bytes[i];
                if b == b'{' {
                    depth += 1;
                } else if b == b'}' {
                    depth -= 1;
                    if depth == 0 {
                        return i + 1;
                    }
                }
                i += 1;
            }
            bytes.len()
        }
        let re_default = Regex::new(r#"(?m)^\s*module\.exports\s*=\s*function\s*\("#).unwrap();
        if let Some(m) = re_default.find(source) {
            let sl = byte_to_line(&offs, m.start());
            let endb = find_block_end(source, m.start());
            let el = byte_to_line(&offs, endb.saturating_sub(1)).max(sl);
            let kind = SymbolKind::Function;
            out.push(Symbol {
                id: SymbolId::new(self.language(), path, &kind, "default", sl),
                name: "default".to_string(),
                kind,
                file: path.to_string(),
                range: TextRange {
                    start_line: sl,
                    end_line: el,
                },
                language: self.language().to_string(),
            });
        }
        let re_named = Regex::new(
            r#"(?m)^\s*(?:module\.exports|exports)\.([A-Za-z_$][\w$]*)\s*=\s*function\s*\("#,
        )
        .unwrap();
        for cap in re_named.captures_iter(source) {
            let m = cap.get(0).unwrap();
            let name = cap.get(1).unwrap().as_str();
            let sl = byte_to_line(&offs, m.start());
            let endb = find_block_end(source, m.start());
            let el = byte_to_line(&offs, endb.saturating_sub(1)).max(sl);
            let kind = SymbolKind::Function;
            out.push(Symbol {
                id: SymbolId::new(self.language(), path, &kind, name, sl),
                name: name.to_string(),
                kind,
                file: path.to_string(),
                range: TextRange {
                    start_line: sl,
                    end_line: el,
                },
                language: self.language().to_string(),
            });
        }
        // Fallback: module.exports = { foo(){}, bar: () => {} }
        if let Some(m) = Regex::new(r#"(?m)^\s*module\.exports\s*=\s*\{"#)
            .unwrap()
            .find(source)
        {
            let start = m.start();
            let endb = find_block_end(source, start);
            let body = &source[start..endb];
            let re_obj_method =
                Regex::new(r#"(?m)\b([A-Za-z_$][\w$]*)\s*\(\s*[^\)]*\s*\)\s*\{"#).unwrap();
            for cap in re_obj_method.captures_iter(body) {
                let name = cap.get(1).unwrap().as_str();
                let s_abs = start + cap.get(0).unwrap().start();
                let sl = byte_to_line(&offs, s_abs);
                let el = byte_to_line(&offs, endb.saturating_sub(1)).max(sl);
                let kind = SymbolKind::Function;
                out.push(Symbol {
                    id: SymbolId::new(self.language(), path, &kind, name, sl),
                    name: name.to_string(),
                    kind,
                    file: path.to_string(),
                    range: TextRange {
                        start_line: sl,
                        end_line: el,
                    },
                    language: self.language().to_string(),
                });
            }
            let re_obj_arrow =
                Regex::new(r#"(?m)\b([A-Za-z_$][\w$]*)\s*:\s*\(?[^\)]*\)?\s*=>\s*\{"#).unwrap();
            for cap in re_obj_arrow.captures_iter(body) {
                let name = cap.get(1).unwrap().as_str();
                let s_abs = start + cap.get(0).unwrap().start();
                let sl = byte_to_line(&offs, s_abs);
                let el = byte_to_line(&offs, endb.saturating_sub(1)).max(sl);
                let kind = SymbolKind::Function;
                out.push(Symbol {
                    id: SymbolId::new(self.language(), path, &kind, name, sl),
                    name: name.to_string(),
                    kind,
                    file: path.to_string(),
                    range: TextRange {
                        start_line: sl,
                        end_line: el,
                    },
                    language: self.language().to_string(),
                });
            }
        }
        // Fallback: class field arrow methods in TS: class A { m = () => { ... } }
        let re_class = Regex::new(r#"(?m)\bclass\s+[A-Za-z_$][\w$]*\s*\{"#).unwrap();
        let re_field_arrow = Regex::new(r#"(?m)(?:\s*(?:public|private|protected|readonly|static|declare|abstract)\s+)*\s*([A-Za-z_$][\w$]*)\s*=\s*\(?[^\)]*\)?\s*(?::[^=]+?)?\s*=>\s*\{"#).unwrap();
        for m in re_class.captures_iter(source) {
            let start = m.get(0).unwrap().start();
            let endb = find_block_end(source, start);
            let body = &source[start..endb];
            for cap in re_field_arrow.captures_iter(body) {
                let name = cap.get(1).unwrap().as_str();
                let s_abs = start + cap.get(0).unwrap().start();
                let mut brace_from = s_abs;
                let bytes = source.as_bytes();
                while brace_from < bytes.len() && bytes[brace_from] != b'{' {
                    brace_from += 1;
                }
                let meth_end = find_block_end(source, brace_from);
                let sl = byte_to_line(&offs, s_abs);
                let el = byte_to_line(&offs, meth_end.saturating_sub(1)).max(sl);
                let kind = SymbolKind::Method;
                out.push(Symbol {
                    id: SymbolId::new(self.language(), path, &kind, name, sl),
                    name: name.to_string(),
                    kind,
                    file: path.to_string(),
                    range: TextRange {
                        start_line: sl,
                        end_line: el,
                    },
                    language: self.language().to_string(),
                });
            }
        }
        out
    }

    fn unresolved_refs(&self, path: &str, source: &str) -> Vec<UnresolvedRef> {
        let offs = line_offsets(source);
        let mut out = Vec::new();
        for caps in self.runner.run_captures(source, &self.queries.calls) {
            let name_cap = caps.iter().find(|c| c.name == "name");
            if let Some(n) = name_cap {
                let name = source[n.start..n.end].to_string();
                if name.is_empty() {
                    continue;
                }
                let is_method = caps.iter().any(|c| c.kind == "member_expression");
                let ln = byte_to_line(&offs, n.start);
                let qual = caps
                    .iter()
                    .find(|c| c.name == "qual")
                    .map(|q| source[q.start..q.end].to_string());
                out.push(UnresolvedRef {
                    name,
                    kind: RefKind::Call,
                    file: path.to_string(),
                    line: ln,
                    qualifier: qual.filter(|s| !s.is_empty()),
                    is_method,
                });
            }
        }
        // Fallback: optional chaining calls (obj?.m()) and identifier calls (foo?.())
        use regex::Regex;
        let re_opt_member =
            Regex::new(r#"([A-Za-z_$][\w$\.]+)\?\.\s*([A-Za-z_$][\w$]*)\s*\("#).unwrap();
        for cap in re_opt_member.captures_iter(source) {
            let q = cap.get(1).unwrap().as_str().to_string();
            let name = cap.get(2).unwrap().as_str().to_string();
            let ln = byte_to_line(&offs, cap.get(0).unwrap().start());
            out.push(UnresolvedRef {
                name,
                kind: RefKind::Call,
                file: path.to_string(),
                line: ln,
                qualifier: Some(q),
                is_method: true,
            });
        }
        let re_opt_ident = Regex::new(r#"\b([A-Za-z_$][\w$]*)\s*\?\.\s*\("#).unwrap();
        for cap in re_opt_ident.captures_iter(source) {
            let name = cap.get(1).unwrap().as_str().to_string();
            let ln = byte_to_line(&offs, cap.get(0).unwrap().start());
            out.push(UnresolvedRef {
                name,
                kind: RefKind::Call,
                file: path.to_string(),
                line: ln,
                qualifier: None,
                is_method: false,
            });
        }
        // Fallback: obj.func?.()
        let re_opt_member2 =
            Regex::new(r#"([A-Za-z_$][\w$\.]+)\.([A-Za-z_$][\w$]*)\s*\?\.\s*\("#).unwrap();
        for cap in re_opt_member2.captures_iter(source) {
            let q = cap.get(1).unwrap().as_str().to_string();
            let name = cap.get(2).unwrap().as_str().to_string();
            let ln = byte_to_line(&offs, cap.get(0).unwrap().start());
            out.push(UnresolvedRef {
                name,
                kind: RefKind::Call,
                file: path.to_string(),
                line: ln,
                qualifier: Some(q),
                is_method: true,
            });
        }
        out
    }

    fn imports_in_file(
        &self,
        path: &str,
        source: &str,
    ) -> std::collections::HashMap<String, String> {
        use regex::Regex;
        let mut map = std::collections::HashMap::new();
        let re_from = Regex::new(r#"(?m)^\s*import\s+(.+?)\s+from\s+['\"]([^'\"]+)['\"]"#).unwrap();
        let re_require = Regex::new(r#"(?m)require\s*\(\s*['\"]([^'\"]+)['\"]\s*\)"#).unwrap();
        let re_export_named =
            Regex::new(r#"(?m)^\s*export\s*\{([^}]+)\}\s*from\s*['\"]([^'\"]+)['\"]"#).unwrap();
        let re_export_all =
            Regex::new(r#"(?m)^\s*export\s*\*\s*from\s*['\"]([^'\"]+)['\"]"#).unwrap();
        let re_req_alias = Regex::new(r#"(?m)^\s*(?:const|let|var)\s+([A-Za-z_$][\w$]*)\s*=\s*require\s*\(\s*['\"]([^'\"]+)['\"]\s*\)"#).unwrap();
        let re_req_destruct = Regex::new(r#"(?m)^\s*(?:const|let|var)\s*\{([^}]+)\}\s*=\s*require\s*\(\s*['\"]([^'\"]+)['\"]\s*\)"#).unwrap();
        for cap in re_from.captures_iter(source) {
            let head = cap.get(1).unwrap().as_str().trim();
            let raw = cap.get(2).unwrap().as_str();
            if let Some(norm) = normalize_ts_module_path(path, raw) {
                // glob prefixes
                map.insert(format!("__glob__{}", norm.clone()), norm.clone());
                let idx = format!("{}/index", norm);
                map.insert(format!("__glob__{}", idx.clone()), idx);
                // namespace import
                if let Some(ns) = head.strip_prefix("* as ") {
                    let alias = ns.trim();
                    map.insert(alias.to_string(), norm.clone());
                }
                // default import
                if head.starts_with(|c: char| c.is_alphabetic() || c == '_' || c == '$')
                    && !head.starts_with('{')
                    && let Some(first) = head.split(',').next()
                {
                    let alias = first.trim();
                    if !alias.is_empty() {
                        map.insert(alias.to_string(), format!("{}::default", norm));
                    }
                }
                // named imports
                if head.starts_with('{') {
                    let inner = head.trim().trim_start_matches('{').trim_end_matches('}');
                    for seg in inner.split(',') {
                        let seg = seg.trim();
                        if seg.is_empty() {
                            continue;
                        }
                        if let Some((orig, alias)) = seg.split_once(" as ") {
                            map.insert(
                                alias.trim().to_string(),
                                format!("{}::{}", norm, orig.trim()),
                            );
                        } else {
                            map.insert(seg.to_string(), format!("{}::{}", norm, seg));
                        }
                    }
                }
            }
        }
        // const X = require('mod')
        for cap in re_req_alias.captures_iter(source) {
            let alias = cap.get(1).unwrap().as_str();
            let raw = cap.get(2).unwrap().as_str();
            if let Some(norm) = normalize_ts_module_path(path, raw) {
                map.insert(alias.to_string(), format!("{}::default", norm));
                map.insert(format!("__glob__{}", norm.clone()), norm.clone());
                let idx = format!("{}/index", norm);
                map.insert(format!("__glob__{}", idx.clone()), idx);
            }
        }
        // const { a, b: c } = require('mod')
        for cap in re_req_destruct.captures_iter(source) {
            let inner = cap.get(1).unwrap().as_str();
            let raw = cap.get(2).unwrap().as_str();
            if let Some(norm) = normalize_ts_module_path(path, raw) {
                for seg in inner.split(',') {
                    let seg = seg.trim();
                    if seg.is_empty() {
                        continue;
                    }
                    if let Some((orig, alias)) = seg.split_once(':') {
                        map.insert(
                            alias.trim().to_string(),
                            format!("{}::{}", norm, orig.trim()),
                        );
                    } else {
                        map.insert(seg.to_string(), format!("{}::{}", norm, seg));
                    }
                }
                map.insert(format!("__glob__{}", norm.clone()), norm.clone());
                let idx = format!("{}/index", norm);
                map.insert(format!("__glob__{}", idx.clone()), idx);
            }
        }
        // export { a as b, c, default as D } from 'mod'
        for cap in re_export_named.captures_iter(source) {
            let inner = cap.get(1).unwrap().as_str();
            let raw = cap.get(2).unwrap().as_str();
            if let Some(norm) = normalize_ts_module_path(path, raw) {
                for seg in inner.split(',') {
                    let seg = seg.trim();
                    if seg.is_empty() {
                        continue;
                    }
                    if let Some((orig, alias)) = seg.split_once(" as ") {
                        let o = orig.trim();
                        let a = alias.trim();
                        map.insert(format!("__export__{}", a), format!("{}::{}", norm, o));
                    } else {
                        let o = seg;
                        map.insert(format!("__export__{}", o), format!("{}::{}", norm, o));
                    }
                }
                map.insert(format!("__export_glob__{}", norm.clone()), norm.clone());
                let idx = format!("{}/index", norm);
                map.insert(format!("__export_glob__{}", idx.clone()), idx);
            }
        }
        // export * from 'mod'
        for cap in re_export_all.captures_iter(source) {
            let raw = cap.get(1).unwrap().as_str();
            if let Some(norm) = normalize_ts_module_path(path, raw) {
                map.insert(format!("__export_glob__{}", norm.clone()), norm.clone());
                let idx = format!("{}/index", norm);
                map.insert(format!("__export_glob__{}", idx.clone()), idx);
            }
        }
        for cap in re_require.captures_iter(source) {
            let raw = cap.get(1).unwrap().as_str();
            if let Some(norm) = normalize_ts_module_path(path, raw) {
                map.insert(format!("__glob__{}", norm.clone()), norm.clone());
                let idx = format!("{}/index", norm);
                map.insert(format!("__glob__{}", idx.clone()), idx);
            }
        }
        map
    }
}

fn normalize_ts_module_path(cur_file: &str, raw: &str) -> Option<String> {
    // Supported TS/JS extensions
    let exts = [".ts", ".tsx", ".mts", ".cts", ".js", ".mjs", ".cjs"];
    resolve_module_path(cur_file, raw, &exts)
}
