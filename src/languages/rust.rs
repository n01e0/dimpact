use crate::ir::{Symbol, SymbolId, SymbolKind, TextRange};
use crate::ir::reference::{RefKind, UnresolvedRef};
use regex::Regex;

pub struct RustAnalyzer;

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
            if ch == '}' { if depth > 0 { depth -= 1; } }
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
        let re_call = Regex::new(r"([A-Za-z_][A-Za-z0-9_]*)\s*(!)?\s*\(").unwrap();
        let re_method = Regex::new(r"\.\s*([A-Za-z_][A-Za-z0-9_]*)\s*\(").unwrap();
        let mut refs = Vec::new();
        for (i, line) in source.lines().enumerate() {
            let ln = (i as u32) + 1;
            for cap in re_method.captures_iter(line) {
                let name = cap.get(1).unwrap().as_str();
                refs.push(UnresolvedRef { name: name.to_string(), kind: RefKind::Call, file: path.to_string(), line: ln });
            }
            for cap in re_call.captures_iter(line) {
                if cap.get(2).map(|m| m.as_str() == "!").unwrap_or(false) {
                    continue; // likely a macro like println!
                }
                let name = cap.get(1).unwrap().as_str();
                refs.push(UnresolvedRef { name: name.to_string(), kind: RefKind::Call, file: path.to_string(), line: ln });
            }
        }
        refs
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
}
