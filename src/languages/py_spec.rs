use crate::ir::reference::{RefKind, UnresolvedRef};
use crate::ir::{Symbol, SymbolId, SymbolKind, TextRange};
use crate::languages::LanguageAnalyzer;
use crate::languages::util::{byte_to_line, line_offsets};
use crate::ts_core::{QueryRunner, compile_queries_python, load_python_spec};

pub struct SpecPyAnalyzer {
    queries: crate::ts_core::CompiledQueries,
    runner: QueryRunner,
}

impl SpecPyAnalyzer {
    pub fn new() -> Self {
        let spec = load_python_spec();
        let queries = compile_queries_python(&spec).expect("compile python queries");
        let runner = QueryRunner::new_python();
        Self { queries, runner }
    }
}

impl Default for SpecPyAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageAnalyzer for SpecPyAnalyzer {
    fn language(&self) -> &'static str {
        "python"
    }

    fn symbols_in_file(&self, path: &str, source: &str) -> Vec<Symbol> {
        let offs = line_offsets(source);
        let lines: Vec<&str> = source.lines().collect();

        #[derive(Clone)]
        struct Decl {
            name: String,
            decl_kind: String,
            start_line: u32,
            end_line: u32,
        }

        let mut decls = Vec::<Decl>::new();
        for caps in self.runner.run_captures(source, &self.queries.decl) {
            let Some(name_cap) = caps.iter().find(|c| c.name == "name") else {
                continue;
            };
            let Some(decl_cap) = caps.iter().find(|c| c.name == "decl") else {
                continue;
            };

            let name = source[name_cap.start..name_cap.end].trim();
            if name.is_empty() {
                continue;
            }

            let sl = byte_to_line(&offs, decl_cap.start);
            let start_idx = (sl.saturating_sub(1)) as usize;
            let end_line = if start_idx < lines.len() {
                find_python_block_end(&lines, start_idx) as u32
            } else {
                sl
            }
            .max(sl);

            decls.push(Decl {
                name: name.to_string(),
                decl_kind: decl_cap.kind.clone(),
                start_line: sl,
                end_line,
            });
        }

        let class_ranges: Vec<(u32, u32)> = decls
            .iter()
            .filter(|d| d.decl_kind == "class_definition")
            .map(|d| (d.start_line, d.end_line))
            .collect();

        decls
            .into_iter()
            .map(|d| {
                let kind = match d.decl_kind.as_str() {
                    "class_definition" => SymbolKind::Struct,
                    "function_definition" => {
                        if class_ranges
                            .iter()
                            .any(|(s, e)| d.start_line > *s && d.start_line <= *e)
                        {
                            SymbolKind::Method
                        } else {
                            SymbolKind::Function
                        }
                    }
                    _ => SymbolKind::Function,
                };

                Symbol {
                    id: SymbolId::new("python", path, &kind, &d.name, d.start_line),
                    name: d.name,
                    kind,
                    file: path.to_string(),
                    range: TextRange {
                        start_line: d.start_line,
                        end_line: d.end_line,
                    },
                    language: "python".to_string(),
                }
            })
            .collect()
    }

    fn unresolved_refs(&self, path: &str, source: &str) -> Vec<UnresolvedRef> {
        use std::collections::HashSet;

        let offs = line_offsets(source);
        let mut out = Vec::new();
        let mut seen: HashSet<(u32, String, Option<String>, bool)> = HashSet::new();
        let imports = self.imports_in_file(path, source);
        let import_aliases: HashSet<String> = imports.keys().cloned().collect();

        for caps in self.runner.run_captures(source, &self.queries.calls) {
            let Some(name_cap) = caps.iter().find(|c| c.name == "name") else {
                continue;
            };
            let name = source[name_cap.start..name_cap.end].trim();
            if name.is_empty() {
                continue;
            }

            let qual = caps
                .iter()
                .find(|c| c.name == "qual")
                .map(|q| source[q.start..q.end].trim().replace([' ', '\t', '\n'], ""))
                .filter(|q| !q.is_empty());
            let call_cap = caps.iter().find(|c| c.name == "call");
            let ln = if let Some(c) = call_cap {
                byte_to_line(&offs, c.start)
            } else {
                byte_to_line(&offs, name_cap.start)
            };

            let is_method = if let Some(q) = qual.as_deref() {
                let first = q.split('.').next().unwrap_or("");
                !import_aliases.contains(first)
            } else {
                false
            };
            let key = (ln, name.to_string(), qual.clone(), is_method);
            if !seen.insert(key) {
                continue;
            }

            out.push(UnresolvedRef {
                name: name.to_string(),
                kind: RefKind::Call,
                file: path.to_string(),
                line: ln,
                qualifier: qual,
                is_method,
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
        let from_mod = module_path_for_file(path);

        let re_import = Regex::new(r"(?m)^\s*import\s+(.+)$").unwrap();
        let re_from =
            Regex::new(r"(?m)^\s*from\s+([A-Za-z0-9_\.]+|\.+[A-Za-z0-9_\.]*)\s+import\s+(.+)$")
                .unwrap();

        for cap in re_import.captures_iter(source) {
            let rhs = cap.get(1).map(|m| m.as_str()).unwrap_or("").trim();
            for item in rhs.split(',') {
                let item = item.trim();
                if item.is_empty() {
                    continue;
                }
                let (module_raw, alias) = if let Some((m, a)) = item.split_once(" as ") {
                    (m.trim(), a.trim())
                } else {
                    (item, item.split('.').next().unwrap_or(item).trim())
                };
                if module_raw.is_empty() || alias.is_empty() {
                    continue;
                }
                let module_path = module_raw.replace('.', "::");
                map.insert(alias.to_string(), module_path.clone());
                map.insert(format!("__glob__{}", module_path.clone()), module_path);
            }
        }

        for cap in re_from.captures_iter(source) {
            let module_raw = cap.get(1).map(|m| m.as_str()).unwrap_or("").trim();
            let rhs = cap.get(2).map(|m| m.as_str()).unwrap_or("").trim();
            if module_raw.is_empty() || rhs.is_empty() {
                continue;
            }

            let module_path = resolve_from_import_module(&from_mod, module_raw);
            if module_path.is_empty() {
                continue;
            }

            let rhs = rhs
                .trim_start_matches('(')
                .trim_end_matches(')')
                .trim_end_matches(',')
                .trim();

            for item in rhs.split(',') {
                let item = item.trim();
                if item.is_empty() {
                    continue;
                }
                if item == "*" {
                    map.insert(
                        format!("__glob__{}", module_path.clone()),
                        module_path.clone(),
                    );
                    continue;
                }

                let (name, alias) = if let Some((n, a)) = item.split_once(" as ") {
                    (n.trim(), a.trim())
                } else {
                    (item, item)
                };
                if name.is_empty() || alias.is_empty() {
                    continue;
                }
                map.insert(alias.to_string(), format!("{}::{}", module_path, name));
            }
        }

        map
    }
}

fn indentation_width(s: &str) -> usize {
    let mut w = 0usize;
    for ch in s.chars() {
        match ch {
            ' ' => w += 1,
            '\t' => w += 4,
            _ => break,
        }
    }
    w
}

fn find_python_block_end(lines: &[&str], start_idx: usize) -> usize {
    let base = indentation_width(lines[start_idx]);
    for (i, line) in lines.iter().enumerate().skip(start_idx + 1) {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        if indentation_width(line) <= base {
            return i;
        }
    }
    lines.len()
}

fn module_path_for_file(path: &str) -> String {
    let p = std::path::Path::new(path);
    let mut s = p.to_string_lossy().replace('\\', "/");
    if let Some(rest) = s.strip_prefix("./") {
        s = rest.to_string();
    }
    if s.ends_with("/__init__.py") {
        s = s.trim_end_matches("/__init__.py").to_string();
    } else if s.ends_with(".py") {
        s = s.trim_end_matches(".py").to_string();
    }
    s.replace('/', "::")
}

fn resolve_from_import_module(from_mod: &str, module_raw: &str) -> String {
    let dots = module_raw.chars().take_while(|c| *c == '.').count();
    if dots == 0 {
        return module_raw.replace('.', "::");
    }

    let rest = module_raw[dots..].trim_matches('.').replace('.', "::");
    let mut parts: Vec<&str> = from_mod.split("::").filter(|s| !s.is_empty()).collect();
    // current file module -> package scope
    if !parts.is_empty() {
        parts.pop();
    }
    for _ in 1..dots {
        if !parts.is_empty() {
            parts.pop();
        }
    }

    let mut base = parts.join("::");
    if !rest.is_empty() {
        if !base.is_empty() {
            base.push_str("::");
        }
        base.push_str(&rest);
    }
    base
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::languages::LanguageAnalyzer;

    #[test]
    fn extract_python_symbols_functions_classes_and_methods() {
        let src = r#"class Service:
    def handle(self, v):
        return v

def run(x):
    return x
"#;
        let ana = SpecPyAnalyzer::new();
        let syms = ana.symbols_in_file("main.py", src);

        assert!(syms.iter().any(|s| {
            s.name == "Service"
                && matches!(s.kind, SymbolKind::Struct)
                && s.id.0 == "python:main.py:struct:Service:1"
        }));
        assert!(syms.iter().any(|s| {
            s.name == "handle"
                && matches!(s.kind, SymbolKind::Method)
                && s.id.0 == "python:main.py:method:handle:2"
        }));
        assert!(syms.iter().any(|s| {
            s.name == "run"
                && matches!(s.kind, SymbolKind::Function)
                && s.id.0 == "python:main.py:fn:run:5"
        }));
    }

    #[test]
    fn unresolved_refs_extracts_bare_and_qualified_calls() {
        let src = r#"def call_all(obj):
    foo()
    obj.bar()
    self.baz()
"#;
        let ana = SpecPyAnalyzer::new();
        let refs = ana.unresolved_refs("main.py", src);

        assert!(
            refs.iter().any(|r| {
                r.name == "foo" && r.qualifier.is_none() && !r.is_method && r.line == 2
            })
        );
        assert!(refs.iter().any(|r| {
            r.name == "bar" && r.qualifier.as_deref() == Some("obj") && r.is_method && r.line == 3
        }));
        assert!(refs.iter().any(|r| {
            r.name == "baz" && r.qualifier.as_deref() == Some("self") && r.is_method && r.line == 4
        }));
    }

    #[test]
    fn unresolved_refs_marks_import_alias_calls_as_non_method() {
        let src = r#"import importlib
from pkg import mod as local_mod

def run():
    importlib.import_module('x')
    local_mod.run()
"#;
        let ana = SpecPyAnalyzer::new();
        let refs = ana.unresolved_refs("pkg/main.py", src);

        assert!(refs.iter().any(|r| {
            r.name == "import_module" && r.qualifier.as_deref() == Some("importlib") && !r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "run" && r.qualifier.as_deref() == Some("local_mod") && !r.is_method
        }));
    }

    #[test]
    fn imports_extract_alias_from_and_relative_paths() {
        let src = r#"import os
import util.helpers as uh
from pkg.service import run as runner, Client
from .local import fn as local_fn
from ..core import base
from . import sibling
from pkg.star import *
"#;
        let ana = SpecPyAnalyzer::new();
        let im = ana.imports_in_file("pkg/sub/main.py", src);

        assert_eq!(im.get("os").map(String::as_str), Some("os"));
        assert_eq!(im.get("uh").map(String::as_str), Some("util::helpers"));
        assert_eq!(
            im.get("runner").map(String::as_str),
            Some("pkg::service::run")
        );
        assert_eq!(
            im.get("Client").map(String::as_str),
            Some("pkg::service::Client")
        );
        assert_eq!(
            im.get("local_fn").map(String::as_str),
            Some("pkg::sub::local::fn")
        );
        assert_eq!(im.get("base").map(String::as_str), Some("pkg::core::base"));
        assert_eq!(
            im.get("sibling").map(String::as_str),
            Some("pkg::sub::sibling")
        );
        assert_eq!(
            im.get("__glob__pkg::star").map(String::as_str),
            Some("pkg::star")
        );
    }

    #[test]
    fn resolve_from_import_module_handles_relative_levels() {
        assert_eq!(
            resolve_from_import_module("pkg::sub::main", ".local"),
            "pkg::sub::local"
        );
        assert_eq!(
            resolve_from_import_module("pkg::sub::main", "..core"),
            "pkg::core"
        );
        assert_eq!(
            resolve_from_import_module("pkg::sub::main", "pkg.service"),
            "pkg::service"
        );
    }

    #[test]
    fn python_hard_case_fixture_dynamic_call_and_import_edge() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/python/analyzer_hard_cases.py"
        ));
        let ana = SpecPyAnalyzer::new();

        let syms = ana.symbols_in_file("pkg/sub/main.py", src);
        assert!(syms.iter().any(|s| {
            s.name == "DynamicRunner"
                && matches!(s.kind, SymbolKind::Struct)
                && s.id.0 == "python:pkg/sub/main.py:struct:DynamicRunner:7"
        }));
        assert!(syms.iter().any(|s| {
            s.name == "run"
                && matches!(s.kind, SymbolKind::Method)
                && s.id.0 == "python:pkg/sub/main.py:method:run:8"
        }));

        let refs = ana.unresolved_refs("pkg/sub/main.py", src);
        assert!(refs.iter().any(|r| {
            r.name == "import_module" && r.qualifier.as_deref() == Some("importlib") && !r.is_method
        }));
        assert!(
            refs.iter()
                .any(|r| r.name == "imod" && r.qualifier.is_none() && !r.is_method)
        );
        assert!(refs.iter().any(|r| {
            r.name == "load" && r.qualifier.as_deref() == Some("local_loader") && !r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "handle" && r.qualifier.as_deref() == Some("handler") && r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "process" && r.qualifier.as_deref() == Some("extra") && r.is_method
        }));

        let im = ana.imports_in_file("pkg/sub/main.py", src);
        assert_eq!(im.get("importlib").map(String::as_str), Some("importlib"));
        assert_eq!(
            im.get("imod").map(String::as_str),
            Some("importlib::import_module")
        );
        assert_eq!(
            im.get("local_loader").map(String::as_str),
            Some("pkg::sub::plugins::loader")
        );
        assert_eq!(
            im.get("__glob__pkg::sub::plugins").map(String::as_str),
            Some("pkg::sub::plugins")
        );
    }

    #[test]
    fn python_hard_case_fixture_v041_dynamic_call_import_edge() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/python/analyzer_hard_cases_v041.py"
        ));
        let ana = SpecPyAnalyzer::new();

        let syms = ana.symbols_in_file("pkg/sub/main.py", src);
        assert!(syms.iter().any(|s| {
            s.name == "DynamicRunner"
                && matches!(s.kind, SymbolKind::Struct)
                && s.id.0 == "python:pkg/sub/main.py:struct:DynamicRunner:7"
        }));
        assert!(syms.iter().any(|s| {
            s.name == "run"
                && matches!(s.kind, SymbolKind::Method)
                && s.id.0 == "python:pkg/sub/main.py:method:run:8"
        }));

        let refs = ana.unresolved_refs("pkg/sub/main.py", src);
        assert!(
            refs.iter()
                .any(|r| r.name == "imod" && r.qualifier.is_none() && !r.is_method)
        );
        assert!(refs.iter().any(|r| {
            r.name == "import_module" && r.qualifier.as_deref() == Some("il") && !r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "load" && r.qualifier.as_deref() == Some("loader_mod") && !r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "handle" && r.qualifier.as_deref() == Some("handler") && r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "process" && r.qualifier.as_deref() == Some("via_alias") && r.is_method
        }));

        let im = ana.imports_in_file("pkg/sub/main.py", src);
        assert_eq!(im.get("il").map(String::as_str), Some("importlib"));
        assert_eq!(
            im.get("imod").map(String::as_str),
            Some("importlib::import_module")
        );
        assert_eq!(
            im.get("loader_mod").map(String::as_str),
            Some("pkg::sub::plugins::loader")
        );
        assert_eq!(
            im.get("__glob__pkg::sub::plugins").map(String::as_str),
            Some("pkg::sub::plugins")
        );
    }

    #[test]
    fn python_hard_case_fixture_decorator_descriptor_attribute_chain() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/python/analyzer_hard_cases_decorator_descriptor_chain.py"
        ));
        let ana = SpecPyAnalyzer::new();

        let syms = ana.symbols_in_file("pkg/svc.py", src);
        assert!(
            syms.iter()
                .any(|s| s.name == "UpperDescriptor" && matches!(s.kind, SymbolKind::Struct))
        );
        assert!(
            syms.iter()
                .any(|s| s.name == "Service" && matches!(s.kind, SymbolKind::Struct))
        );
        assert!(
            syms.iter()
                .any(|s| s.name == "process" && matches!(s.kind, SymbolKind::Method))
        );
        assert!(
            syms.iter()
                .any(|s| s.name == "traced" && matches!(s.kind, SymbolKind::Function))
        );

        let refs = ana.unresolved_refs("pkg/svc.py", src);
        assert!(
            refs.iter()
                .any(|r| r.name == "w" && r.qualifier.is_none() && !r.is_method)
        );
        assert!(refs.iter().any(|r| {
            r.name == "normalizer" && r.qualifier.as_deref() == Some("self") && r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "send"
                && r.qualifier.as_deref() == Some("self.api.client.dispatcher")
                && r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "strip" && r.qualifier.as_deref() == Some("cleaned") && r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "lower"
                && r.qualifier
                    .as_deref()
                    .map(|q| q.starts_with("cleaned.strip"))
                    .unwrap_or(false)
                && r.is_method
        }));

        let im = ana.imports_in_file("pkg/svc.py", src);
        assert_eq!(im.get("w").map(String::as_str), Some("functools::wraps"));
    }
}
