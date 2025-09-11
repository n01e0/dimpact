use crate::ir::{Symbol, SymbolId, SymbolKind, TextRange};
use crate::ir::reference::{RefKind, UnresolvedRef};
use regex::Regex;

pub struct RustAnalyzer;

impl Default for RustAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl RustAnalyzer {
    pub fn new() -> Self { Self }
}

fn find_block_end(source: &str, start_line_idx: usize, open_brace_on_line: bool) -> usize {
    // Return end line index (0-based) of the block starting at or after start_line_idx.
    // Very naive: counts braces, ignores strings/comments intricacies.
    let mut depth = 0usize;
    let mut started = false;
    for (i, line) in source.lines().enumerate().skip(start_line_idx) {
        for ch in line.chars() {
            if ch == '{' { depth += 1; started = true; }
            if ch == '}' { depth = depth.saturating_sub(1); }
        }
        if open_brace_on_line && i == start_line_idx { // include brace on same line
            if !started { depth += 1; started = true; }
        }
        if started && depth == 0 { return i; }
    }
    // fallback to last line
    source.lines().count().saturating_sub(1)
}

fn mk_symbol(path: &str, lang: &str, name: &str, kind: SymbolKind, start_line: u32, end_line: u32) -> Symbol {
    Symbol {
        id: SymbolId::new(lang, path, &kind, name, start_line),
        name: name.to_string(),
        kind,
        file: path.to_string(),
        range: TextRange { start_line, end_line },
        language: lang.to_string(),
    }
}

impl crate::languages::LanguageAnalyzer for RustAnalyzer {
    fn language(&self) -> &'static str { "rust" }

    fn symbols_in_file(&self, path: &str, source: &str) -> Vec<Symbol> {
        let re_fn = Regex::new(r"^\s*(?:pub\s+)?(?:async\s+)?(?:const\s+)?fn\s+([a-zA-Z_][a-zA-Z0-9_]*)").unwrap();
        let re_struct = Regex::new(r"^\s*(?:pub\s+)?struct\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap();
        let re_enum = Regex::new(r"^\s*(?:pub\s+)?enum\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap();
        let re_trait = Regex::new(r"^\s*(?:pub\s+)?trait\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap();

        let mut symbols = Vec::new();
        let lines: Vec<&str> = source.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            let l = *line;
            if let Some(caps) = re_fn.captures(l) {
                let name = caps.get(1).unwrap().as_str();
                let open_brace_on_line = l.contains('{');
                let end_idx = find_block_end(source, idx, open_brace_on_line);
                symbols.push(mk_symbol(path, "rust", name, SymbolKind::Function, (idx as u32)+1, (end_idx as u32)+1));
                continue;
            }
            if let Some(caps) = re_struct.captures(l) {
                let name = caps.get(1).unwrap().as_str();
                let end_idx = if l.contains('{') { find_block_end(source, idx, true) } else { idx };
                symbols.push(mk_symbol(path, "rust", name, SymbolKind::Struct, (idx as u32)+1, (end_idx as u32)+1));
                continue;
            }
            if let Some(caps) = re_enum.captures(l) {
                let name = caps.get(1).unwrap().as_str();
                let end_idx = if l.contains('{') { find_block_end(source, idx, true) } else { idx };
                symbols.push(mk_symbol(path, "rust", name, SymbolKind::Enum, (idx as u32)+1, (end_idx as u32)+1));
                continue;
            }
            if let Some(caps) = re_trait.captures(l) {
                let name = caps.get(1).unwrap().as_str();
                let end_idx = if l.contains('{') { find_block_end(source, idx, true) } else { idx };
                symbols.push(mk_symbol(path, "rust", name, SymbolKind::Trait, (idx as u32)+1, (end_idx as u32)+1));
                continue;
            }
        }
        symbols
    }

    fn unresolved_refs(&self, path: &str, source: &str) -> Vec<UnresolvedRef> {
        // qualified free fn: a::b::c(
        let re_qcall = Regex::new(r"([A-Za-z_][A-Za-z0-9_]*(?:::[A-Za-z_][A-Za-z0-9_]*)*)\s*\(").unwrap();
        // simple free fn: name(
        let re_call = Regex::new(r"\b([A-Za-z_][A-Za-z0-9_]*)\s*(!)?\s*\(").unwrap();
        // method: .name(
        let re_method = Regex::new(r"\.\s*([A-Za-z_][A-Za-z0-9_]*)\s*\(").unwrap();
        let mut refs = Vec::new();
        for (i, line) in source.lines().enumerate() {
            let ln = (i as u32) + 1;
            // qualified calls first to capture a::b::c(...)
            for cap in re_qcall.captures_iter(line) {
                let full = cap.get(1).unwrap().as_str();
                if full.contains("::") {
                    let mut parts: Vec<&str> = full.split("::").collect();
                    if let Some(last) = parts.pop() {
                        refs.push(UnresolvedRef {
                            name: last.to_string(),
                            kind: RefKind::Call,
                            file: path.to_string(),
                            line: ln,
                            qualifier: Some(parts.join("::")),
                            is_method: false,
                        });
                    }
                }
            }
            for cap in re_method.captures_iter(line) {
                let name = cap.get(1).unwrap().as_str();
                refs.push(UnresolvedRef { name: name.to_string(), kind: RefKind::Call, file: path.to_string(), line: ln, qualifier: None, is_method: true });
            }
            for cap in re_call.captures_iter(line) {
                if cap.get(2).map(|m| m.as_str() == "!").unwrap_or(false) {
                    continue; // likely a macro like println!
                }
                let name = cap.get(1).unwrap().as_str();
                // skip if already recorded as qualified call on same line
                if refs.iter().any(|r| r.line == ln && r.name == name && r.qualifier.is_some()) { continue; }
                refs.push(UnresolvedRef { name: name.to_string(), kind: RefKind::Call, file: path.to_string(), line: ln, qualifier: None, is_method: false });
            }
        }
        refs
    }

    fn imports_in_file(&self, path: &str, source: &str) -> std::collections::HashMap<String, String> {
        fn normalize(s: &str) -> String { s.trim().to_string() }
        fn flatten(items: &str, prefix: &str, out: &mut std::collections::HashMap<String, String>) {
            let mut depth: i32 = 0; let mut cur = String::new();
            let push_item = |tok: &str, prefix: &str, out: &mut std::collections::HashMap<String, String>| {
                let tok = tok.trim(); if tok.is_empty() { return; }
                if tok == "self" {
                    let alias = prefix.split("::").filter(|s| !s.is_empty()).last().unwrap_or(prefix).trim();
                    out.insert(alias.to_string(), normalize(prefix));
                    return;
                }
                if tok == "*" {
                    out.insert(format!("__glob__{}", prefix), prefix.to_string());
                    return;
                }
                if let Some(brace) = tok.find('{') {
                    let pfx = format!("{}::{}", prefix, tok[..brace].trim().trim_end_matches("::"));
                    let inner = tok[brace+1..].trim_end_matches('}');
                    flatten(inner, &pfx, out);
                    return;
                }
                let (name, alias) = if let Some((n,a)) = tok.split_once(" as ") { (n.trim(), a.trim()) } else { (tok, tok) };
                let full = if prefix.is_empty() { name.to_string() } else { format!("{}::{}", prefix, name) };
                out.insert(alias.to_string(), normalize(&full));
            };
            for ch in items.chars() {
                match ch {
                    '{' => { depth+=1; cur.push(ch); }
                    '}' => { depth=depth.saturating_sub(1); cur.push(ch); }
                    ',' if depth==0 => { let t = cur.trim().to_string(); if !t.is_empty() { push_item(&t, prefix, out); } cur.clear(); }
                    _ => cur.push(ch),
                }
            }
            let t = cur.trim().to_string(); if !t.is_empty() { push_item(&t, prefix, out); }
        }
        let mut map = std::collections::HashMap::new();
        for mut line in source.lines().map(|l| l.trim()) {
            if !(line.starts_with("use ") || line.starts_with("pub use ")) { continue; }
            if !line.ends_with(';') { continue; }
            if let Some(stripped) = line.strip_prefix("pub use ") { line = stripped; } else if let Some(stripped) = line.strip_prefix("use ") { line = stripped; }
            line = &line[..line.len()-1];
            if let Some(brace_pos) = line.find('{') {
                let prefix = line[..brace_pos].trim_end_matches("::").trim();
                let rest = &line[brace_pos+1..line.rfind('}').unwrap_or(line.len())];
                flatten(rest, prefix, &mut map);
            } else {
                let (path_spec, alias) = if let Some((p, a)) = line.split_once(" as ") { (p.trim(), a.trim()) } else { (line, line.split("::").last().unwrap_or(line)) };
                if path_spec.ends_with("::*") {
                    let pfx = path_spec.trim_end_matches("::*");
                    map.insert(format!("__glob__{}", pfx), pfx.to_string());
                } else {
                    map.insert(alias.to_string(), normalize(path_spec));
                }
            }
        }
        // mod declarations map: mod foo; -> current_mod::foo
        let current_mod = super::super::impact::module_path_for_file(path);
        for l in source.lines() {
            let t = l.trim();
            if let Some(rest) = t.strip_prefix("mod ") {
                let name = rest.trim().trim_end_matches(';').trim();
                if !name.is_empty() {
                    let mp = if current_mod.is_empty() { name.to_string() } else { format!("{}::{}", current_mod, name) };
                    map.insert(name.to_string(), mp);
                }
            }
        }
        map
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::languages::LanguageAnalyzer;

    #[test]
    fn extract_basic_symbols() {
        let src = r#"pub struct S { x: i32 }

impl S { fn m(&self) {} }

fn foo() {
    println!("hi");
}

enum E { A, B }
"#;
        let ana = RustAnalyzer::new();
        let syms = LanguageAnalyzer::symbols_in_file(&ana, "lib.rs", src);
        let names: Vec<_> = syms.iter().map(|s| (&s.name, &s.kind)).collect();
        assert!(names.iter().any(|(n, _)| **n == "S"));
        assert!(names.iter().any(|(n, _)| **n == "foo"));
        assert!(names.iter().any(|(n, _)| **n == "E"));
        // range sanity: foo spans at least 2 lines
        let foo = syms.iter().find(|s| s.name == "foo").unwrap();
        assert!(foo.range.end_line >= foo.range.start_line);
    }

    #[test]
    fn extract_unresolved_refs_basic() {
        let src = r#"fn foo() { bar(); x.baz(); println!("ok"); }"#;
        let ana = RustAnalyzer::new();
        let refs = ana.unresolved_refs("lib.rs", src);
        let names: Vec<_> = refs.iter().map(|r| r.name.as_str()).collect();
        assert!(names.contains(&"bar"));
        assert!(names.contains(&"baz"));
        assert!(!names.contains(&"println"));
    }

    #[test]
    fn extract_qualified_refs() {
        let src = r#"fn foo() { crate::utils::call(); a::b::c(); }"#;
        let ana = RustAnalyzer::new();
        let refs = ana.unresolved_refs("lib.rs", src);
        assert!(refs.iter().any(|r| r.name == "call" && r.qualifier.as_deref() == Some("crate::utils")));
        assert!(refs.iter().any(|r| r.name == "c" && r.qualifier.as_deref() == Some("a::b")));
    }

    #[test]
    fn parse_imports_variants() {
        let src = r#"use a::b::c;
use x as y;
use a::b::{d, e as f};
"#;
        let ana = RustAnalyzer::new();
        let m = ana.imports_in_file("lib.rs", src);
        assert_eq!(m.get("c").unwrap(), "a::b::c");
        assert_eq!(m.get("y").unwrap(), "x");
        assert_eq!(m.get("d").unwrap(), "a::b::d");
        assert_eq!(m.get("f").unwrap(), "a::b::e");
    }
}
