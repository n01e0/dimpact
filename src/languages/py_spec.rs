use crate::ir::reference::UnresolvedRef;
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

    fn unresolved_refs(&self, _path: &str, _source: &str) -> Vec<UnresolvedRef> {
        Vec::new()
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
    fn unresolved_refs_is_empty_for_symbols_phase() {
        let ana = SpecPyAnalyzer::new();
        let refs = ana.unresolved_refs("main.py", "def foo():\n    return bar()\n");
        assert!(refs.is_empty());
    }
}
