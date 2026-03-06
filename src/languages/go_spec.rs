use crate::ir::reference::{RefKind, UnresolvedRef};
use crate::ir::{Symbol, SymbolId, SymbolKind, TextRange};
use crate::languages::LanguageAnalyzer;
use crate::languages::util::{byte_to_line, line_offsets};

pub struct SpecGoAnalyzer;

impl SpecGoAnalyzer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SpecGoAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageAnalyzer for SpecGoAnalyzer {
    fn language(&self) -> &'static str {
        "go"
    }

    fn symbols_in_file(&self, path: &str, source: &str) -> Vec<Symbol> {
        use regex::Regex;

        let mut out = Vec::new();
        let offs = line_offsets(source);
        let re_func = Regex::new(
            r"(?m)^[ \t]*func[ \t]*(\([^)]*\)[ \t]*)?([A-Za-z_][A-Za-z0-9_]*)[ \t]*(?:\[[^\]]+\])?[ \t]*\(",
        )
        .unwrap();

        for caps in re_func.captures_iter(source) {
            let Some(full) = caps.get(0) else {
                continue;
            };
            let Some(name_cap) = caps.get(2) else {
                continue;
            };
            let name = name_cap.as_str();
            if name.is_empty() {
                continue;
            }
            let kind = if caps.get(1).is_some() {
                SymbolKind::Method
            } else {
                SymbolKind::Function
            };
            let sl = byte_to_line(&offs, full.start());
            let end_byte = find_decl_block_end(source, full.start()).unwrap_or(full.end());
            let el = byte_to_line(&offs, end_byte.saturating_sub(1)).max(sl);
            out.push(Symbol {
                id: SymbolId::new("go", path, &kind, name, sl),
                name: name.to_string(),
                kind,
                file: path.to_string(),
                range: TextRange {
                    start_line: sl,
                    end_line: el,
                },
                language: "go".to_string(),
            });
        }
        out
    }

    fn unresolved_refs(&self, path: &str, source: &str) -> Vec<UnresolvedRef> {
        use regex::Regex;
        use std::collections::HashSet;

        let mut out = Vec::new();
        let mut seen: HashSet<(u32, String, Option<String>, bool)> = HashSet::new();
        let offs = line_offsets(source);
        let imports = self.imports_in_file(path, source);
        let import_aliases: HashSet<String> = imports.keys().cloned().collect();
        let decl_lines: HashSet<u32> = source
            .lines()
            .enumerate()
            .filter_map(|(i, line)| {
                if line.trim_start().starts_with("func ") {
                    Some((i as u32) + 1)
                } else {
                    None
                }
            })
            .collect();

        let re_qualified_call = Regex::new(
            r"\b([A-Za-z_][A-Za-z0-9_]*(?:\s*\.\s*[A-Za-z_][A-Za-z0-9_]*)+)\s*(?:\[[^\]]+\])?\s*\(",
        )
        .unwrap();
        for cap in re_qualified_call.captures_iter(source) {
            let Some(m) = cap.get(0) else {
                continue;
            };
            let Some(chain) = cap.get(1) else {
                continue;
            };
            let ln = byte_to_line(&offs, m.start());
            if decl_lines.contains(&ln) {
                continue;
            }
            let compact = chain.as_str().replace([' ', '\t', '\n'], "");
            let mut parts = compact.split('.').collect::<Vec<_>>();
            if parts.len() < 2 {
                continue;
            }
            let Some(name) = parts.pop() else {
                continue;
            };
            let qual = parts.join(".");
            let first = qual.split('.').next().unwrap_or("");
            let is_package_qual = parts.len() == 1 && import_aliases.contains(first);
            let is_method = !is_package_qual;
            let key = (ln, name.to_string(), Some(qual.clone()), is_method);
            if !seen.insert(key) {
                continue;
            }
            out.push(UnresolvedRef {
                name: name.to_string(),
                kind: RefKind::Call,
                file: path.to_string(),
                line: ln,
                qualifier: Some(qual),
                is_method,
            });
        }

        let re_bare_call =
            Regex::new(r"\b([A-Za-z_][A-Za-z0-9_]*)\s*(?:\[[^\]]+\])?\s*\(").unwrap();
        let skip = [
            "if", "for", "switch", "select", "return", "func", "go", "defer", "make", "new", "len",
            "cap", "append", "copy", "delete", "close", "panic", "recover", "print", "println",
            "complex", "real", "imag",
        ];
        for cap in re_bare_call.captures_iter(source) {
            let Some(m) = cap.get(0) else {
                continue;
            };
            let Some(name_cap) = cap.get(1) else {
                continue;
            };
            let name = name_cap.as_str();
            if skip.contains(&name) {
                continue;
            }
            // Avoid duplicate with qualified calls like pkg.Foo(...)
            if m.start() > 0 && source.as_bytes()[m.start().saturating_sub(1)] == b'.' {
                continue;
            }
            let ln = byte_to_line(&offs, m.start());
            if decl_lines.contains(&ln) {
                continue;
            }
            let key = (ln, name.to_string(), None, false);
            if !seen.insert(key) {
                continue;
            }
            out.push(UnresolvedRef {
                name: name.to_string(),
                kind: RefKind::Call,
                file: path.to_string(),
                line: ln,
                qualifier: None,
                is_method: false,
            });
        }

        out
    }

    fn imports_in_file(
        &self,
        _path: &str,
        source: &str,
    ) -> std::collections::HashMap<String, String> {
        use regex::Regex;
        use std::collections::HashMap;

        let mut map = HashMap::new();
        let re_single =
            Regex::new(r#"^\s*import\s+(?:([._A-Za-z][A-Za-z0-9_]*)\s+)?["`]([^"`]+)["`]"#)
                .unwrap();
        let re_block_start = Regex::new(r#"^\s*import\s*\("#).unwrap();
        let re_block_item =
            Regex::new(r#"^\s*(?:([._A-Za-z][A-Za-z0-9_]*)\s+)?["`]([^"`]+)["`]"#).unwrap();

        let mut in_block = false;
        for line in source.lines() {
            let trimmed = line.trim();
            if !in_block {
                if re_block_start.is_match(trimmed) {
                    in_block = true;
                    continue;
                }
                if let Some(cap) = re_single.captures(trimmed) {
                    let path = cap.get(2).map(|m| m.as_str()).unwrap_or("");
                    if path.is_empty() {
                        continue;
                    }
                    let alias = cap
                        .get(1)
                        .map(|m| m.as_str().to_string())
                        .unwrap_or_else(|| default_import_alias(path));
                    map.insert(alias, path.replace('/', "::"));
                }
                continue;
            }

            if trimmed.starts_with(')') {
                in_block = false;
                continue;
            }
            if let Some(cap) = re_block_item.captures(trimmed) {
                let path = cap.get(2).map(|m| m.as_str()).unwrap_or("");
                if path.is_empty() {
                    continue;
                }
                let alias = cap
                    .get(1)
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_else(|| default_import_alias(path));
                map.insert(alias, path.replace('/', "::"));
            }
        }
        map
    }
}

fn find_decl_block_end(source: &str, decl_start: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut i = decl_start;
    while i < bytes.len() && bytes[i] != b'{' {
        i += 1;
    }
    if i >= bytes.len() {
        return None;
    }
    let mut depth = 0i32;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'{' {
            depth += 1;
        } else if b == b'}' {
            depth -= 1;
            if depth == 0 {
                return Some(i + 1);
            }
        }
        i += 1;
    }
    Some(bytes.len())
}

fn default_import_alias(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::languages::LanguageAnalyzer;

    #[test]
    fn extract_go_symbols_functions_and_methods() {
        let src = r#"package sample

func Top(a int) int {
    return helper(a)
}

func (s *Service) Handle(v int) error {
    return s.do(v)
}
"#;
        let ana = SpecGoAnalyzer::new();
        let syms = ana.symbols_in_file("pkg/main.go", src);
        assert_eq!(syms.len(), 2);
        assert!(syms.iter().any(|s| {
            s.name == "Top"
                && matches!(s.kind, SymbolKind::Function)
                && s.id.0 == "go:pkg/main.go:fn:Top:3"
                && s.range.end_line >= s.range.start_line
        }));
        assert!(syms.iter().any(|s| {
            s.name == "Handle"
                && matches!(s.kind, SymbolKind::Method)
                && s.id.0 == "go:pkg/main.go:method:Handle:7"
                && s.range.end_line >= s.range.start_line
        }));
    }

    #[test]
    fn extract_go_unresolved_refs_with_method_and_package_calls() {
        let src = r#"package sample
import "fmt"

func run(svc *Service) {
    svc.Handle(1)
    fmt.Println("x")
    plain()
}
"#;
        let ana = SpecGoAnalyzer::new();
        let refs = ana.unresolved_refs("pkg/main.go", src);
        assert!(refs.iter().any(|r| {
            r.name == "Handle"
                && r.qualifier.as_deref() == Some("svc")
                && r.is_method
                && r.line == 5
        }));
        assert!(refs.iter().any(|r| {
            r.name == "Println"
                && r.qualifier.as_deref() == Some("fmt")
                && !r.is_method
                && r.line == 6
        }));
        assert!(refs.iter().any(|r| {
            r.name == "plain" && r.qualifier.is_none() && !r.is_method && r.line == 7
        }));
    }

    #[test]
    fn extract_go_imports_single_and_block() {
        let src = r#"package sample
import "fmt"
import alias "github.com/acme/lib"
import (
    "net/http"
    ioalias "io"
    _ "github.com/acme/hidden"
)
"#;
        let ana = SpecGoAnalyzer::new();
        let imports = ana.imports_in_file("pkg/main.go", src);
        assert_eq!(imports.get("fmt").map(String::as_str), Some("fmt"));
        assert_eq!(
            imports.get("alias").map(String::as_str),
            Some("github.com::acme::lib")
        );
        assert_eq!(imports.get("http").map(String::as_str), Some("net::http"));
        assert_eq!(imports.get("ioalias").map(String::as_str), Some("io"));
        assert_eq!(
            imports.get("_").map(String::as_str),
            Some("github.com::acme::hidden")
        );
    }

    #[test]
    fn go_hard_case_fixture_generics_chained_embedded() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/go/analyzer_hard_cases.go"
        ));
        let ana = SpecGoAnalyzer::new();

        let syms = ana.symbols_in_file("pkg/hard.go", src);
        assert!(syms.iter().any(|s| {
            s.name == "Map"
                && matches!(s.kind, SymbolKind::Function)
                && s.id.0 == "go:pkg/hard.go:fn:Map:34"
        }));
        assert!(syms.iter().any(|s| {
            s.name == "Handle"
                && matches!(s.kind, SymbolKind::Method)
                && s.id.0 == "go:pkg/hard.go:method:Handle:42"
        }));

        let refs = ana.unresolved_refs("pkg/hard.go", src);
        assert!(
            refs.iter()
                .any(|r| { r.name == "Log" && r.qualifier.as_deref() == Some("s") && r.is_method })
        );
        assert!(refs.iter().any(|r| {
            r.name == "Do" && r.qualifier.as_deref() == Some("s.repo.client") && r.is_method
        }));
        assert!(
            refs.iter()
                .any(|r| { r.name == "Map" && r.qualifier.is_none() && !r.is_method })
        );
        assert!(refs.iter().any(|r| {
            r.name == "Background" && r.qualifier.as_deref() == Some("context") && !r.is_method
        }));
    }
}
