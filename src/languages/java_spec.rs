use crate::ir::reference::{RefKind, UnresolvedRef};
use crate::ir::{Symbol, SymbolId, SymbolKind, TextRange};
use crate::languages::LanguageAnalyzer;
use crate::languages::util::{byte_to_line, line_offsets};

pub struct SpecJavaAnalyzer;

impl SpecJavaAnalyzer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SpecJavaAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageAnalyzer for SpecJavaAnalyzer {
    fn language(&self) -> &'static str {
        "java"
    }

    fn symbols_in_file(&self, path: &str, source: &str) -> Vec<Symbol> {
        use regex::Regex;
        use std::collections::HashSet;

        let mut out = Vec::new();
        let mut seen_ids: HashSet<String> = HashSet::new();
        let mut class_names: HashSet<String> = HashSet::new();
        let offs = line_offsets(source);

        let re_type = Regex::new(
            r"(?m)^[ \t]*(?:public|protected|private|abstract|final|static|sealed|non-sealed|strictfp|\s)*\b(class|interface|enum)\s+([A-Za-z_][A-Za-z0-9_]*)",
        )
        .expect("valid java type regex");

        for caps in re_type.captures_iter(source) {
            let Some(full) = caps.get(0) else {
                continue;
            };
            let Some(kind_cap) = caps.get(1) else {
                continue;
            };
            let Some(name_cap) = caps.get(2) else {
                continue;
            };
            let kind = match kind_cap.as_str() {
                "class" => SymbolKind::Struct,
                "interface" => SymbolKind::Trait,
                "enum" => SymbolKind::Enum,
                _ => continue,
            };
            let name = name_cap.as_str();
            class_names.insert(name.to_string());

            let sl = byte_to_line(&offs, full.start());
            let end_byte = find_decl_block_end(source, full.start()).unwrap_or(full.end());
            let el = byte_to_line(&offs, end_byte.saturating_sub(1)).max(sl);
            let id = SymbolId::new("java", path, &kind, name, sl);
            if !seen_ids.insert(id.0.clone()) {
                continue;
            }
            out.push(Symbol {
                id,
                name: name.to_string(),
                kind,
                file: path.to_string(),
                range: TextRange {
                    start_line: sl,
                    end_line: el,
                },
                language: "java".to_string(),
            });
        }

        let re_method = Regex::new(
            r"(?m)^[ \t]*(?:@[A-Za-z_][A-Za-z0-9_.]*(?:\([^)]*\))?\s*)*(?:public|protected|private|static|final|abstract|synchronized|native|strictfp|default|\s)*(?:<[^>\n]+>\s*)?(?:[A-Za-z_][A-Za-z0-9_<>\[\].?]*(?:\s+[A-Za-z_][A-Za-z0-9_<>\[\].?]*)*)\s+([A-Za-z_][A-Za-z0-9_]*)\s*\([^;{]*\)\s*(?:throws [^{;]+)?\{",
        )
        .expect("valid java method regex");

        for caps in re_method.captures_iter(source) {
            let Some(full) = caps.get(0) else {
                continue;
            };
            let Some(name_cap) = caps.get(1) else {
                continue;
            };
            let name = name_cap.as_str();
            if is_java_control_like(name) {
                continue;
            }
            let sl = byte_to_line(&offs, full.start());
            let end_byte = find_decl_block_end(source, full.start()).unwrap_or(full.end());
            let el = byte_to_line(&offs, end_byte.saturating_sub(1)).max(sl);
            let id = SymbolId::new("java", path, &SymbolKind::Method, name, sl);
            if !seen_ids.insert(id.0.clone()) {
                continue;
            }
            out.push(Symbol {
                id,
                name: name.to_string(),
                kind: SymbolKind::Method,
                file: path.to_string(),
                range: TextRange {
                    start_line: sl,
                    end_line: el,
                },
                language: "java".to_string(),
            });
        }

        // Constructors: same name as class, no return type.
        let re_ctor = Regex::new(
            r"(?m)^[ \t]*(?:@[A-Za-z_][A-Za-z0-9_.]*(?:\([^)]*\))?\s*)*(?:public|protected|private|\s)+([A-Za-z_][A-Za-z0-9_]*)\s*\([^;{]*\)\s*(?:throws [^{;]+)?\{",
        )
        .expect("valid java ctor regex");

        for caps in re_ctor.captures_iter(source) {
            let Some(full) = caps.get(0) else {
                continue;
            };
            let Some(name_cap) = caps.get(1) else {
                continue;
            };
            let name = name_cap.as_str();
            if !class_names.contains(name) {
                continue;
            }
            let sl = byte_to_line(&offs, full.start());
            let end_byte = find_decl_block_end(source, full.start()).unwrap_or(full.end());
            let el = byte_to_line(&offs, end_byte.saturating_sub(1)).max(sl);
            let id = SymbolId::new("java", path, &SymbolKind::Method, name, sl);
            if !seen_ids.insert(id.0.clone()) {
                continue;
            }
            out.push(Symbol {
                id,
                name: name.to_string(),
                kind: SymbolKind::Method,
                file: path.to_string(),
                range: TextRange {
                    start_line: sl,
                    end_line: el,
                },
                language: "java".to_string(),
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

        let decl_lines: HashSet<u32> = self
            .symbols_in_file(path, source)
            .into_iter()
            .filter(|s| matches!(s.kind, SymbolKind::Method | SymbolKind::Function))
            .map(|s| s.range.start_line)
            .collect();

        let re_qualified_call =
            Regex::new(r"\b([A-Za-z_][A-Za-z0-9_]*(?:\s*\.\s*[A-Za-z_][A-Za-z0-9_]*)+)\s*\(")
                .expect("valid java qualified call regex");
        for caps in re_qualified_call.captures_iter(source) {
            let Some(m) = caps.get(0) else {
                continue;
            };
            let Some(chain_cap) = caps.get(1) else {
                continue;
            };
            let ln = byte_to_line(&offs, m.start());
            if decl_lines.contains(&ln) {
                continue;
            }

            let compact = chain_cap.as_str().replace([' ', '\t', '\n'], "");
            let mut parts = compact.split('.').collect::<Vec<_>>();
            if parts.len() < 2 {
                continue;
            }
            let Some(name) = parts.pop() else {
                continue;
            };
            let qual = parts.join(".");
            let first = qual.split('.').next().unwrap_or("");
            let is_method = if first == "this" || first == "super" {
                true
            } else if import_aliases.contains(first) {
                false
            } else {
                !first
                    .chars()
                    .next()
                    .map(|c| c.is_ascii_uppercase())
                    .unwrap_or(false)
            };

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

        let re_method_ref = Regex::new(
            r"\b([A-Za-z_][A-Za-z0-9_]*(?:\s*\.\s*[A-Za-z_][A-Za-z0-9_]*)*)\s*::\s*([A-Za-z_][A-Za-z0-9_]*)",
        )
        .expect("valid java method ref regex");
        for caps in re_method_ref.captures_iter(source) {
            let Some(m) = caps.get(0) else {
                continue;
            };
            let Some(qual_cap) = caps.get(1) else {
                continue;
            };
            let Some(name_cap) = caps.get(2) else {
                continue;
            };

            let ln = byte_to_line(&offs, m.start());
            if decl_lines.contains(&ln) {
                continue;
            }

            let qual = qual_cap.as_str().replace([' ', '\t', '\n'], "");
            let name = name_cap.as_str();
            let first = qual.split('.').next().unwrap_or("");
            let is_method = if first == "this" || first == "super" {
                true
            } else if import_aliases.contains(first) {
                false
            } else {
                !first
                    .chars()
                    .next()
                    .map(|c| c.is_ascii_uppercase())
                    .unwrap_or(false)
            };

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
            Regex::new(r"\b([A-Za-z_][A-Za-z0-9_]*)\s*\(").expect("valid java bare call regex");
        for caps in re_bare_call.captures_iter(source) {
            let Some(m) = caps.get(0) else {
                continue;
            };
            let Some(name_cap) = caps.get(1) else {
                continue;
            };
            let name = name_cap.as_str();
            if is_java_control_like(name) {
                continue;
            }
            // Skip qualified calls (already handled above).
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
        let re_import =
            Regex::new(r"(?m)^\s*import\s+(static\s+)?([A-Za-z_][A-Za-z0-9_.]*)(\.\*)?\s*;")
                .expect("valid java import regex");

        for caps in re_import.captures_iter(source) {
            let Some(path_cap) = caps.get(2) else {
                continue;
            };
            let path = path_cap.as_str();
            let norm = path.replace('.', "::");
            let wildcard = caps.get(3).is_some();

            if wildcard {
                map.insert(format!("__glob__{norm}"), norm);
                continue;
            }

            let alias = default_import_alias(path);
            if alias.is_empty() {
                continue;
            }

            map.insert(alias, norm);
        }

        map
    }
}

fn is_java_control_like(name: &str) -> bool {
    matches!(
        name,
        "if" | "for"
            | "while"
            | "switch"
            | "catch"
            | "return"
            | "new"
            | "throw"
            | "synchronized"
            | "assert"
            | "try"
            | "do"
            | "else"
    )
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
    path.rsplit('.').next().unwrap_or(path).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_java_symbols_types_methods_and_ctor() {
        let src = r#"package demo;

public class Service {
    public Service() {}

    public void handle(int v) {
        helper(v);
    }

    static int helper(int v) {
        return v;
    }
}

interface Runner {
    void run();
}

enum Mode {
    ON, OFF;
}
"#;
        let ana = SpecJavaAnalyzer::new();
        let syms = ana.symbols_in_file("src/Service.java", src);

        assert!(
            syms.iter()
                .any(|s| s.name == "Service" && matches!(s.kind, SymbolKind::Struct))
        );
        assert!(
            syms.iter()
                .any(|s| s.name == "Runner" && matches!(s.kind, SymbolKind::Trait))
        );
        assert!(
            syms.iter()
                .any(|s| s.name == "Mode" && matches!(s.kind, SymbolKind::Enum))
        );
        assert!(
            syms.iter()
                .any(|s| s.name == "handle" && matches!(s.kind, SymbolKind::Method))
        );
        assert!(
            syms.iter()
                .any(|s| s.name == "helper" && matches!(s.kind, SymbolKind::Method))
        );
    }

    #[test]
    fn extract_java_unresolved_refs_method_and_qualified_calls() {
        let src = r#"package demo;

import java.util.Collections;

class Service {
    void run() {
        this.handle();
        Collections.sort(items);
        helper();
    }

    void handle() {}
}
"#;
        let ana = SpecJavaAnalyzer::new();
        let refs = ana.unresolved_refs("src/Service.java", src);

        assert!(refs.iter().any(|r| {
            r.name == "handle"
                && r.qualifier.as_deref() == Some("this")
                && r.is_method
                && r.line == 7
        }));
        assert!(refs.iter().any(|r| {
            r.name == "sort"
                && r.qualifier.as_deref() == Some("Collections")
                && !r.is_method
                && r.line == 8
        }));
        assert!(refs.iter().any(|r| {
            r.name == "helper" && r.qualifier.is_none() && !r.is_method && r.line == 9
        }));
    }

    #[test]
    fn extract_java_imports_single_static_and_wildcard() {
        let src = r#"package demo;

import java.util.List;
import java.util.*;
import static java.util.Collections.sort;
import static java.util.Collections.*;
"#;
        let ana = SpecJavaAnalyzer::new();
        let imports = ana.imports_in_file("src/Service.java", src);

        assert_eq!(
            imports.get("List").map(String::as_str),
            Some("java::util::List")
        );
        assert_eq!(
            imports.get("__glob__java::util").map(String::as_str),
            Some("java::util")
        );
        assert_eq!(
            imports.get("sort").map(String::as_str),
            Some("java::util::Collections::sort")
        );
        assert_eq!(
            imports
                .get("__glob__java::util::Collections")
                .map(String::as_str),
            Some("java::util::Collections")
        );
    }

    #[test]
    fn extract_java_unresolved_refs_static_import_and_nested_qualified_call() {
        let src = r#"package demo;

import static demo.Ops.pick;

public class Main {
    static int run() {
        return pick(1) + Outer.Inner.compute();
    }
}
"#;
        let ana = SpecJavaAnalyzer::new();
        let refs = ana.unresolved_refs("demo/Main.java", src);

        assert!(
            refs.iter()
                .any(|r| r.name == "pick" && r.qualifier.is_none() && !r.is_method)
        );
        assert!(refs.iter().any(|r| {
            r.name == "compute" && r.qualifier.as_deref() == Some("Outer.Inner") && !r.is_method
        }));
    }

    #[test]
    fn java_hard_case_fixture_overload_static_import_nested() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/java/analyzer_hard_cases.java"
        ));
        let ana = SpecJavaAnalyzer::new();

        let syms = ana.symbols_in_file("demo/Parser.java", src);
        let parse_count = syms
            .iter()
            .filter(|s| s.name == "parse" && matches!(s.kind, SymbolKind::Method))
            .count();
        assert_eq!(parse_count, 2, "expected overloaded parse methods");
        assert!(
            syms.iter()
                .any(|s| s.name == "run" && matches!(s.kind, SymbolKind::Method))
        );

        let refs = ana.unresolved_refs("demo/Parser.java", src);
        assert!(refs.iter().any(|r| {
            r.name == "compute" && r.qualifier.as_deref() == Some("Outer.Inner") && !r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "parseInt" && r.qualifier.as_deref() == Some("Integer") && !r.is_method
        }));
        assert!(
            refs.iter()
                .any(|r| r.name == "emptyList" && r.qualifier.is_none() && !r.is_method)
        );
        assert!(
            refs.iter()
                .any(|r| { r.name == "requireNonNull" && r.qualifier.is_none() && !r.is_method })
        );

        let imports = ana.imports_in_file("demo/Parser.java", src);
        assert_eq!(
            imports.get("emptyList").map(String::as_str),
            Some("java::util::Collections::emptyList")
        );
        assert_eq!(
            imports.get("requireNonNull").map(String::as_str),
            Some("java::util::Objects::requireNonNull")
        );
    }

    #[test]
    fn java_hard_case_fixture_v041_overload_static_import_nested_type() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/java/analyzer_hard_cases_v041.java"
        ));
        let ana = SpecJavaAnalyzer::new();

        let syms = ana.symbols_in_file("demo/Engine.java", src);
        assert!(
            syms.iter()
                .any(|s| s.name == "Engine" && matches!(s.kind, SymbolKind::Struct))
        );
        assert!(
            syms.iter()
                .any(|s| s.name == "Nested" && matches!(s.kind, SymbolKind::Struct))
        );

        let parse_count = syms
            .iter()
            .filter(|s| s.name == "parse" && matches!(s.kind, SymbolKind::Method))
            .count();
        assert_eq!(parse_count, 2, "expected overloaded parse methods");
        assert!(
            syms.iter()
                .any(|s| s.name == "run" && matches!(s.kind, SymbolKind::Method))
        );

        let refs = ana.unresolved_refs("demo/Engine.java", src);
        assert!(refs.iter().any(|r| {
            r.name == "eval" && r.qualifier.as_deref() == Some("Engine.Nested") && !r.is_method
        }));
        assert!(
            refs.iter()
                .any(|r| r.name == "emptyList" && r.qualifier.is_none() && !r.is_method)
        );
        assert!(
            refs.iter()
                .any(|r| { r.name == "requireNonNull" && r.qualifier.is_none() && !r.is_method })
        );

        let imports = ana.imports_in_file("demo/Engine.java", src);
        assert_eq!(
            imports.get("emptyList").map(String::as_str),
            Some("java::util::Collections::emptyList")
        );
        assert_eq!(
            imports.get("requireNonNull").map(String::as_str),
            Some("java::util::Objects::requireNonNull")
        );
    }

    #[test]
    fn java_hard_case_fixture_lambda_method_ref_inner_class_call() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/java/analyzer_hard_cases_lambda_methodref_inner.java"
        ));
        let ana = SpecJavaAnalyzer::new();

        let syms = ana.symbols_in_file("demo/Flow.java", src);
        assert!(
            syms.iter()
                .any(|s| s.name == "Flow" && matches!(s.kind, SymbolKind::Struct))
        );
        assert!(
            syms.iter()
                .any(|s| s.name == "run" && matches!(s.kind, SymbolKind::Method))
        );
        let parse_count = syms
            .iter()
            .filter(|s| s.name == "parse" && matches!(s.kind, SymbolKind::Method))
            .count();
        assert!(
            parse_count >= 2,
            "expected parse methods in outer and inner classes"
        );

        let refs = ana.unresolved_refs("demo/Flow.java", src);
        assert!(refs.iter().any(|r| {
            r.name == "parse" && r.qualifier.as_deref() == Some("this") && r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "compute" && r.qualifier.as_deref() == Some("inner") && r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "parse" && r.qualifier.as_deref() == Some("Outer.Inner") && !r.is_method
        }));
        assert!(
            refs.iter()
                .any(|r| r.name == "parse" && r.qualifier.is_none() && !r.is_method)
        );

        let imports = ana.imports_in_file("demo/Flow.java", src);
        assert_eq!(
            imports.get("Function").map(String::as_str),
            Some("java::util::function::Function")
        );
    }

    #[test]
    fn java_hard_case_fixture_lambda_method_ref_overload_v73() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/java/analyzer_hard_cases_lambda_methodref_overload.java"
        ));
        let ana = SpecJavaAnalyzer::new();

        let syms = ana.symbols_in_file("demo/OverloadLab.java", src);
        assert!(
            syms.iter()
                .any(|s| s.name == "OverloadLab" && matches!(s.kind, SymbolKind::Struct))
        );
        assert!(syms.iter().any(|s| {
            s.name == "run"
                && matches!(s.kind, SymbolKind::Method)
                && s.id.0 == "java:demo/OverloadLab.java:method:run:23"
        }));
        let parse_count = syms
            .iter()
            .filter(|s| s.name == "parse" && matches!(s.kind, SymbolKind::Method))
            .count();
        assert!(parse_count >= 3, "expected overloaded parse methods");

        let refs = ana.unresolved_refs("demo/OverloadLab.java", src);
        assert!(refs.iter().any(|r| {
            r.name == "parse" && r.qualifier.as_deref() == Some("this") && r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "parseStatic"
                && r.qualifier.as_deref() == Some("OverloadLab")
                && !r.is_method
        }));
        assert!(
            refs.iter()
                .any(|r| r.name == "parse" && r.qualifier.is_none() && !r.is_method)
        );

        let imports = ana.imports_in_file("demo/OverloadLab.java", src);
        assert_eq!(
            imports.get("Function").map(String::as_str),
            Some("java::util::function::Function")
        );
        assert_eq!(
            imports.get("BiFunction").map(String::as_str),
            Some("java::util::function::BiFunction")
        );
    }

    #[test]
    fn java_hard_case_fixture_lambda_method_ref_overload_v2() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/java/analyzer_hard_cases_lambda_methodref_overload_v2.java"
        ));
        let ana = SpecJavaAnalyzer::new();

        let syms = ana.symbols_in_file("demo/JavaOverloadLabV2.java", src);
        assert!(syms.iter().any(|s| {
            s.name == "JavaOverloadLabV2" && matches!(s.kind, SymbolKind::Struct)
        }));
        assert!(
            syms.iter()
                .any(|s| s.name == "run" && matches!(s.kind, SymbolKind::Method))
        );
        let decode_count = syms
            .iter()
            .filter(|s| s.name == "decode" && matches!(s.kind, SymbolKind::Method))
            .count();
        assert!(decode_count >= 3, "expected overloaded decode methods");

        let refs = ana.unresolved_refs("demo/JavaOverloadLabV2.java", src);
        assert!(refs.iter().any(|r| {
            r.name == "decode" && r.qualifier.as_deref() == Some("this") && r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "decode" && r.qualifier.as_deref() == Some("codec") && r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "decodeStatic"
                && r.qualifier.as_deref() == Some("JavaOverloadLabV2")
                && !r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "decode" && r.qualifier.is_none() && !r.is_method
        }));

        let imports = ana.imports_in_file("demo/JavaOverloadLabV2.java", src);
        assert_eq!(
            imports.get("Function").map(String::as_str),
            Some("java::util::function::Function")
        );
        assert_eq!(
            imports.get("BiFunction").map(String::as_str),
            Some("java::util::function::BiFunction")
        );
    }
}
