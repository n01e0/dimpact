use crate::ir::reference::{RefKind, UnresolvedRef};
use crate::ir::{Symbol, SymbolId, SymbolKind, TextRange};
use crate::languages::LanguageAnalyzer;
use crate::languages::path::normalize_path_like;
use crate::languages::util::{byte_to_line, line_offsets};
use crate::ts_core::{QueryRunner, compile_queries_ruby, load_ruby_spec};

pub struct SpecRubyAnalyzer {
    queries: crate::ts_core::CompiledQueries,
    runner: QueryRunner,
}

impl SpecRubyAnalyzer {
    pub fn new() -> Self {
        let spec = load_ruby_spec();
        let queries = compile_queries_ruby(&spec).expect("compile ruby queries");
        let runner = QueryRunner::new_ruby();
        Self { queries, runner }
    }
}

impl Default for SpecRubyAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageAnalyzer for SpecRubyAnalyzer {
    fn language(&self) -> &'static str {
        "ruby"
    }

    fn symbols_in_file(&self, path: &str, source: &str) -> Vec<Symbol> {
        // Use TS queries for declarations; classify roughly by node kind
        let offs = line_offsets(source);
        let lines: Vec<&str> = source.lines().collect();
        let mut out = Vec::new();
        for caps in self.runner.run_captures(source, &self.queries.decl) {
            if let Some(nc) = caps.iter().find(|c| c.name == "name") {
                let name = &source[nc.start..nc.end];
                if name.is_empty() {
                    continue;
                }
                // Determine declaration kind from @decl node
                let mut kind = SymbolKind::Function;
                if let Some(decl_node) = caps.iter().find(|c| c.name == "decl") {
                    kind = match decl_node.kind.as_str() {
                        "class" => SymbolKind::Struct,
                        "module" => SymbolKind::Module,
                        "method" | "singleton_method" => SymbolKind::Method,
                        _ => SymbolKind::Function,
                    };
                } else {
                    // Fallback: infer from any captured node kinds
                    for c in &caps {
                        match c.kind.as_str() {
                            "class" => {
                                kind = SymbolKind::Struct;
                                break;
                            }
                            "module" => {
                                kind = SymbolKind::Module;
                                break;
                            }
                            "method" | "singleton_method" => {
                                kind = SymbolKind::Method;
                                break;
                            }
                            _ => {}
                        }
                    }
                }
                let sl = byte_to_line(&offs, nc.start);
                // Expand range to full Ruby block if possible
                let mut el = byte_to_line(&offs, nc.end.saturating_sub(1)).max(sl);
                let start_idx = (sl.saturating_sub(1)) as usize;
                if start_idx < lines.len() {
                    let end_idx = find_ruby_block_end(&lines, start_idx);
                    el = ((end_idx as u32) + 1).max(sl);
                }
                out.push(Symbol {
                    id: SymbolId::new("ruby", path, &kind, name, sl),
                    name: name.to_string(),
                    kind,
                    file: path.to_string(),
                    range: TextRange {
                        start_line: sl,
                        end_line: el,
                    },
                    language: "ruby".to_string(),
                });
            }
        }
        out
    }

    fn unresolved_refs(&self, path: &str, source: &str) -> Vec<UnresolvedRef> {
        let mut out = Vec::new();
        let offs = line_offsets(source);
        let re_sym_call = regex::Regex::new(r":([A-Za-z_][A-Za-z0-9_?!]*)").unwrap();
        for caps in self.runner.run_captures(source, &self.queries.calls) {
            let name_cap = caps.iter().find(|c| c.name == "name");
            if let Some(n) = name_cap {
                let mut name = source[n.start..n.end].to_string();
                if name.is_empty() {
                    continue;
                }
                if (name == "send" || name == "public_send")
                    && let Some(callnode) = caps.iter().find(|c| c.name == "call")
                {
                    let text = &source[callnode.start..callnode.end];
                    if let Some(mat) = re_sym_call.captures(text) {
                        name = mat.get(1).unwrap().as_str().to_string();
                    }
                }
                let ln = if let Some(callnode) = caps.iter().find(|c| c.name == "call") {
                    byte_to_line(&offs, callnode.start)
                } else {
                    byte_to_line(&offs, n.start)
                };
                out.push(UnresolvedRef {
                    name,
                    kind: RefKind::Call,
                    file: path.to_string(),
                    line: ln,
                    qualifier: None,
                    is_method: true,
                });
            }
        }
        // Fallback: paren-less bare call like `m` (no args, no receiver)
        use regex::Regex;
        let re_bare = Regex::new(r"^\s*([a-zA-Z_][A-Za-z0-9_?!]*)").unwrap();
        let seen: std::collections::HashSet<(u32, String)> =
            out.iter().map(|r| (r.line, r.name.clone())).collect();
        for (i, line) in source.lines().enumerate() {
            if let Some(cap) = re_bare.captures(line) {
                let name = cap.get(1).unwrap().as_str();
                let rest = &line[cap.get(0).unwrap().end()..];
                let rest_trim = rest.trim_start();
                if rest_trim.starts_with('=')
                    || rest_trim.starts_with('.')
                    || rest_trim.starts_with("::")
                    || rest_trim.starts_with('(')
                {
                    // likely assignment, receiver call, namespace, or explicit paren-call handled elsewhere
                } else {
                    let ln = (i as u32) + 1;
                    if !seen.contains(&(ln, name.to_string())) {
                        out.push(UnresolvedRef {
                            name: name.to_string(),
                            kind: RefKind::Call,
                            file: path.to_string(),
                            line: ln,
                            qualifier: None,
                            is_method: true,
                        });
                    }
                }
            }
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
        let re_req = Regex::new(r#"^\s*(require|require_relative)\s+['\"]([^'\"]+)['\"]"#).unwrap();
        for line in source.lines() {
            if let Some(cap) = re_req.captures(line) {
                let kind = cap.get(1).unwrap().as_str();
                let raw = cap.get(2).unwrap().as_str();
                // Normalize: strip extension, resolve relative segments for require_relative
                let mut pfx = raw.trim();
                if pfx.ends_with(".rb") {
                    pfx = &pfx[..pfx.len() - 3];
                }
                // Convert Windows backslashes to forward slashes for normalization
                let pfx = pfx.replace('\\', "/");
                let normalized = if kind == "require_relative" {
                    // Resolve relative to the directory of `path`
                    let base_dir = std::path::Path::new(path)
                        .parent()
                        .unwrap_or_else(|| std::path::Path::new("."));
                    let joined = base_dir.join(pfx);
                    let canon = normalize_path_like(&joined);
                    canon
                        .trim_start_matches("./")
                        .trim_start_matches('.')
                        .trim_start_matches('/')
                        .to_string()
                } else {
                    // For plain `require`, keep as-is (minus extension) — load path unknown
                    pfx.trim_start_matches("./")
                        .trim_start_matches('.')
                        .trim_start_matches('/')
                        .to_string()
                };
                if normalized.is_empty() {
                    continue;
                }
                // store as glob prefix path-like (foo/bar)
                map.insert(format!("__glob__{}", normalized), normalized);
            }
        }
        map
    }
}

fn find_ruby_block_end(lines: &[&str], start: usize) -> usize {
    let mut depth = 0i32;
    let re_begin = regex::Regex::new(r"\b(def|class|module)\b").unwrap();
    let re_end = regex::Regex::new(r"\bend\b").unwrap();
    for (idx, line) in lines.iter().enumerate().skip(start) {
        if re_begin.is_match(line) {
            depth += 1;
        }
        if re_end.is_match(line) {
            depth -= 1;
            if depth == 0 {
                return idx;
            }
        }
    }
    lines.len().saturating_sub(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::languages::LanguageAnalyzer;

    #[test]
    fn extract_calls_including_safe_nav_and_send() {
        let src = r#"def m; end
def foo
  a=nil
  a&.m
  m
  self.send(:m)
end
"#;
        let ana = SpecRubyAnalyzer::new();
        let refs = ana.unresolved_refs("a.rb", src);
        let names: Vec<_> = refs.iter().map(|r| r.name.as_str()).collect();
        assert!(names.contains(&"m"));
        // at least 2 occurrences (a&.m and m)
        assert!(names.iter().filter(|&&n| n == "m").count() >= 2);
    }
}
