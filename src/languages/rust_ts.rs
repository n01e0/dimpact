#![cfg(feature = "ts")]
use crate::ir::{Symbol, SymbolId, SymbolKind, TextRange};
use crate::ir::reference::{RefKind, UnresolvedRef};
use std::cell::RefCell;

pub struct RustTsAnalyzer {
    parser: RefCell<tree_sitter::Parser>,
}

impl RustTsAnalyzer {
    pub fn new() -> Self {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tree_sitter_rust::language()).expect("load ts-rust");
        Self { parser: RefCell::new(parser) }
    }
}

fn line_lookup(src: &str) -> Vec<usize> {
    let mut offs = vec![0usize];
    for (i, b) in src.bytes().enumerate() { if b == b'\n' { offs.push(i+1); } }
    offs
}

fn byte_to_line(offs: &[usize], byte: usize) -> u32 {
    match offs.binary_search(&byte) {
        Ok(i) => (i as u32) + 1,
        Err(i) => i as u32,
    }
}

impl crate::languages::LanguageAnalyzer for RustTsAnalyzer {
    fn language(&self) -> &'static str { "rust" }

    fn symbols_in_file(&self, path: &str, source: &str) -> Vec<Symbol> {
        let tree = self.parser.borrow_mut().parse(source, None).unwrap();
        let root = tree.root_node();
        let offs = line_lookup(source);
        let mut out = Vec::new();
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            let kind = node.kind();
            let s = if kind == "function_item" {
                let name = node.child_by_field_name("name").map(|n| n.utf8_text(source.as_bytes()).unwrap()).unwrap_or("");
                Some((name.to_string(), SymbolKind::Function))
            } else if kind == "struct_item" {
                let name = node.child_by_field_name("name").map(|n| n.utf8_text(source.as_bytes()).unwrap()).unwrap_or("");
                Some((name.to_string(), SymbolKind::Struct))
            } else if kind == "enum_item" {
                let name = node.child_by_field_name("name").map(|n| n.utf8_text(source.as_bytes()).unwrap()).unwrap_or("");
                Some((name.to_string(), SymbolKind::Enum))
            } else if kind == "trait_item" {
                let name = node.child_by_field_name("name").map(|n| n.utf8_text(source.as_bytes()).unwrap()).unwrap_or("");
                Some((name.to_string(), SymbolKind::Trait))
            } else if kind == "impl_item" {
                // methods inside impl
                for i in 0..node.child_count() {
                    let ch = node.child(i).unwrap();
                    if ch.kind() == "function_item" || ch.kind() == "method_definition" {
                        let name_node = ch.child_by_field_name("name");
                        if let Some(nn) = name_node {
                            let name = nn.utf8_text(source.as_bytes()).unwrap();
                            let sl = byte_to_line(&offs, ch.start_byte());
                            let el = byte_to_line(&offs, ch.end_byte().saturating_sub(1));
                            out.push(Symbol {
                                id: SymbolId::new("rust", path, &SymbolKind::Method, name, sl),
                                name: name.to_string(),
                                kind: SymbolKind::Method,
                                file: path.to_string(),
                                range: TextRange { start_line: sl, end_line: el.max(sl) },
                                language: "rust".to_string(),
                            });
                        }
                    }
                }
                None
            } else { None };
            if let Some((name, kind)) = s {
                if !name.is_empty() {
                    let sl = byte_to_line(&offs, node.start_byte());
                    let el = byte_to_line(&offs, node.end_byte().saturating_sub(1));
                    out.push(Symbol {
                        id: SymbolId::new("rust", path, &kind, &name, sl),
                        name,
                        kind,
                        file: path.to_string(),
                        range: TextRange { start_line: sl, end_line: el.max(sl) },
                        language: "rust".to_string(),
                    });
                }
            }
            for i in 0..node.child_count() { stack.push(node.child(i).unwrap()); }
        }
        out
    }

    fn unresolved_refs(&self, path: &str, source: &str) -> Vec<UnresolvedRef> {
        let tree = self.parser.borrow_mut().parse(source, None).unwrap();
        let root = tree.root_node();
        let offs = line_lookup(source);
        let mut out = Vec::new();
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            if node.kind() == "call_expression" {
                let func = node.child_by_field_name("function");
                if let Some(f) = func {
                    let ln = byte_to_line(&offs, node.start_byte());
                    let k = f.kind();
                    if k == "identifier" {
                        let name = f.utf8_text(source.as_bytes()).unwrap().to_string();
                        if name.ends_with('!') { /* macro - ignore */ } else {
                            out.push(UnresolvedRef { name, kind: RefKind::Call, file: path.to_string(), line: ln, qualifier: None, is_method: false });
                        }
                    } else if k == "scoped_identifier" || k == "scoped_type_identifier" || k == "qualified_name" || k == "path_expression" {
                        let txt = f.utf8_text(source.as_bytes()).unwrap();
                        let parts: Vec<&str> = txt.split("::").collect();
                        if let Some((last, rest)) = parts.split_last() {
                            let qualifier = if rest.is_empty() { None } else { Some(rest.join("::")) };
                            out.push(UnresolvedRef { name: (*last).to_string(), kind: RefKind::Call, file: path.to_string(), line: ln, qualifier, is_method: false });
                        }
                    } else if k == "field_expression" {
                        // x.method()
                        if let Some(name_node) = f.child_by_field_name("field") {
                            let name = name_node.utf8_text(source.as_bytes()).unwrap().to_string();
                            out.push(UnresolvedRef { name, kind: RefKind::Call, file: path.to_string(), line: ln, qualifier: None, is_method: true });
                        }
                    }
                }
            }
            for i in 0..node.child_count() { stack.push(node.child(i).unwrap()); }
        }
        out
    }

    fn imports_in_file(&self, path: &str, source: &str) -> std::collections::HashMap<String, String> {
        let tree = self.parser.borrow_mut().parse(source, None).unwrap();
        let root = tree.root_node();
        let mut map = std::collections::HashMap::new();
        let bytes = source.as_bytes();
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            if node.kind() == "use_declaration" {
                // Simplified: flatten single or braced list
                let clause = node.child_by_field_name("argument");
                if let Some(c) = clause {
                    let text = c.utf8_text(bytes).unwrap();
                    if text.contains('{') {
                        // recursively flatten nested braces
                        fn flatten(out: &mut std::collections::HashMap<String,String>, prefix: &str, inner: &str) {
                            let mut depth: i32=0; let mut cur=String::new();
                            let push = |tok: &str, pfx: &str, out: &mut std::collections::HashMap<String,String>| {
                                let tok = tok.trim(); if tok.is_empty() { return; }
                                if tok == "self" {
                                    let alias = pfx.split("::").filter(|s| !s.is_empty()).last().unwrap_or(pfx).trim();
                                    out.insert(alias.to_string(), pfx.to_string());
                                    return;
                                }
                                if tok == "*" {
                                    out.insert(format!("__glob__{}", pfx), pfx.to_string());
                                    return;
                                }
                                if let Some(br) = tok.find('{') {
                                    let newp = format!("{}::{}", pfx, tok[..br].trim().trim_end_matches("::"));
                                    let inn = tok[br+1..].trim_end_matches('}');
                                    flatten(out, &newp, inn);
                                } else if let Some((n,a)) = tok.split_once(" as ") {
                                    out.insert(a.trim().to_string(), format!("{}::{}", pfx, n.trim()));
                                } else {
                                    out.insert(tok.to_string(), format!("{}::{}", pfx, tok));
                                }
                            };
                            for ch in inner.chars() {
                                match ch {
                                    '{' => { depth+=1; cur.push(ch); }
                                    '}' => { depth=depth.saturating_sub(1); cur.push(ch); }
                                    ',' if depth==0 => { let t=cur.trim().to_string(); if !t.is_empty(){ push(&t, prefix, out);} cur.clear(); }
                                    _ => cur.push(ch),
                                }
                            }
                            let t=cur.trim().to_string(); if !t.is_empty(){ push(&t, prefix, out);}    
                        }
                        if let Some((pref, rest)) = text.split_once('{') {
                            let prefix = pref.trim().trim_end_matches("::");
                            let inner = rest.trim().trim_end_matches('}');
                            flatten(&mut map, prefix, inner);
                        }
                        } else {
                            // simple use path [as alias]
                            if let Some((p, a)) = text.split_once(" as ") {
                                let alias = a.trim();
                                map.insert(alias.to_string(), p.trim().to_string());
                            } else {
                                let alias = text.split("::").last().unwrap_or(text).trim();
                                if text.trim_end().ends_with("::*") {
                                    let pfx = text.trim().trim_end_matches("::*");
                                    map.insert(format!("__glob__{}", pfx).to_string(), pfx.to_string());
                                } else {
                                    map.insert(alias.to_string(), text.trim().to_string());
                                }
                            }
                        }
                }
            }
            for i in 0..node.child_count() { stack.push(node.child(i).unwrap()); }
        }
        // mod declarations mapping
        let current_mod = crate::impact::module_path_for_file(path);
        for l in source.lines() {
            let t = l.trim();
            if t.starts_with("mod ") {
                let name = t[4..].trim().trim_end_matches(';').trim();
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
    fn ts_extracts_symbols() {
        let ana = RustTsAnalyzer::new();
        let src = "fn foo() {} struct S;";
        let syms = ana.symbols_in_file("lib.rs", src);
        let names: Vec<_> = syms.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"foo"));
        assert!(names.contains(&"S"));
    }
}
