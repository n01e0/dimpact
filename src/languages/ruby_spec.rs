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

        // Track simple local assignments for send/public_send argument tracing.
        // Example: dyn_sym = :target, dyn_str = "target".
        let re_assign_literal = regex::Regex::new(
            r#"(?m)^\s*([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(?::([A-Za-z_][A-Za-z0-9_?!]*)|[\"']([A-Za-z_][A-Za-z0-9_?!]*)[\"'])\s*$"#,
        )
        .unwrap();
        let mut assigned_literals: std::collections::HashMap<String, Vec<(u32, String)>> =
            std::collections::HashMap::new();
        for caps in re_assign_literal.captures_iter(source) {
            let Some(full) = caps.get(0) else {
                continue;
            };
            let Some(var) = caps.get(1) else {
                continue;
            };
            let Some(value) = caps.get(2).or_else(|| caps.get(3)) else {
                continue;
            };
            let ln = byte_to_line(&offs, full.start());
            assigned_literals
                .entry(var.as_str().to_string())
                .or_default()
                .push((ln, value.as_str().to_string()));
        }
        for entries in assigned_literals.values_mut() {
            entries.sort_by_key(|(ln, _)| *ln);
        }

        let re_send_first_arg =
            regex::Regex::new(r#"(?:^|[^\w])(?:send|public_send)\s*\(\s*([^,\)\n]+)"#).unwrap();
        let re_symbol_lit = regex::Regex::new(r#"^:([A-Za-z_][A-Za-z0-9_?!]*)$"#).unwrap();
        let re_string_lit = regex::Regex::new(r#"^[\"']([A-Za-z_][A-Za-z0-9_?!]*)[\"']$"#).unwrap();
        let re_ident = regex::Regex::new(r#"^([A-Za-z_][A-Za-z0-9_]*)$"#).unwrap();

        let resolve_send_target = |call_text: &str, ln: u32| -> Option<String> {
            let arg_raw = re_send_first_arg
                .captures(call_text)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().trim())?;

            if let Some(cap) = re_symbol_lit.captures(arg_raw) {
                return cap.get(1).map(|m| m.as_str().to_string());
            }
            if let Some(cap) = re_string_lit.captures(arg_raw) {
                return cap.get(1).map(|m| m.as_str().to_string());
            }
            if let Some(cap) = re_ident.captures(arg_raw) {
                let var = cap.get(1).map(|m| m.as_str())?;
                if let Some(cands) = assigned_literals.get(var) {
                    return cands
                        .iter()
                        .rev()
                        .find(|(line, _)| *line <= ln)
                        .map(|(_, v)| v.clone());
                }
            }
            None
        };

        for caps in self.runner.run_captures(source, &self.queries.calls) {
            let name_cap = caps.iter().find(|c| c.name == "name");
            if let Some(n) = name_cap {
                let mut name = source[n.start..n.end].to_string();
                if name.is_empty() {
                    continue;
                }
                let ln = if let Some(callnode) = caps.iter().find(|c| c.name == "call") {
                    byte_to_line(&offs, callnode.start)
                } else {
                    byte_to_line(&offs, n.start)
                };
                if (name == "send" || name == "public_send")
                    && let Some(callnode) = caps.iter().find(|c| c.name == "call")
                {
                    let text = &source[callnode.start..callnode.end];
                    if let Some(resolved) = resolve_send_target(text, ln) {
                        name = resolved;
                    }
                }
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
        let mut seen: std::collections::HashSet<(u32, String)> =
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
                    if seen.insert((ln, name.to_string())) {
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

        // Additional fallback: explicit receiver call forms like `self.m` / `obj.m` / `obj&.m`
        let re_receiver =
            Regex::new(r"(?:^|[^:&\w])(?:self|[a-zA-Z_][A-Za-z0-9_]*)\s*(?:\.|&\.)\s*([a-zA-Z_][A-Za-z0-9_?!]*)").unwrap();
        for (i, line) in source.lines().enumerate() {
            let ln = (i as u32) + 1;
            for cap in re_receiver.captures_iter(line) {
                let name = cap.get(1).unwrap().as_str().to_string();
                if seen.insert((ln, name.clone())) {
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
  self.m
  m
  self.send(:m)
end
"#;
        let ana = SpecRubyAnalyzer::new();
        let refs = ana.unresolved_refs("a.rb", src);
        let names: Vec<_> = refs.iter().map(|r| r.name.as_str()).collect();
        assert!(names.contains(&"m"));
        // at least 3 occurrences (a&.m, self.m, and bare m)
        assert!(names.iter().filter(|&&n| n == "m").count() >= 3);
    }

    #[test]
    fn ruby_dynamic_fixture_send_public_send_symbol_string() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/ruby/analyzer_hard_cases_dynamic_send_public_send.rb"
        ));
        let ana = SpecRubyAnalyzer::new();

        let syms = ana.symbols_in_file("pkg/ruby_dynamic.rb", src);
        assert!(syms.iter().any(|s| {
            s.name == "target_sym"
                && matches!(s.kind, SymbolKind::Method)
                && s.id.0 == "ruby:pkg/ruby_dynamic.rb:method:target_sym:2"
        }));
        assert!(syms.iter().any(|s| {
            s.name == "target_str"
                && matches!(s.kind, SymbolKind::Method)
                && s.id.0 == "ruby:pkg/ruby_dynamic.rb:method:target_str:6"
        }));

        let refs = ana.unresolved_refs("pkg/ruby_dynamic.rb", src);
        let names: Vec<_> = refs.iter().map(|r| r.name.as_str()).collect();
        assert!(names.iter().filter(|&&n| n == "target_sym").count() >= 3);
        assert!(names.iter().filter(|&&n| n == "target_str").count() >= 3);
        // send/public_send targets should be traced from symbol/string args (including local var assignment).
        assert!(!names.contains(&"send"));
        assert!(!names.contains(&"public_send"));
    }

    #[test]
    fn ruby_dynamic_fixture_alias_method_define_method() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/ruby/analyzer_hard_cases_dynamic_alias_define_method.rb"
        ));
        let ana = SpecRubyAnalyzer::new();

        let syms = ana.symbols_in_file("pkg/ruby_dynamic_alias_define.rb", src);
        assert!(syms.iter().any(|s| {
            s.name == "original"
                && matches!(s.kind, SymbolKind::Method)
                && s.id.0 == "ruby:pkg/ruby_dynamic_alias_define.rb:method:original:2"
        }));
        assert!(syms.iter().any(|s| {
            s.name == "execute"
                && matches!(s.kind, SymbolKind::Method)
                && s.id.0 == "ruby:pkg/ruby_dynamic_alias_define.rb:method:execute:17"
        }));

        let refs = ana.unresolved_refs("pkg/ruby_dynamic_alias_define.rb", src);
        let names: Vec<_> = refs.iter().map(|r| r.name.as_str()).collect();
        assert!(names.contains(&"alias_method"));
        assert!(names.contains(&"define_method"));
        assert!(names.contains(&"aliased_sym"));
        assert!(names.contains(&"aliased_str"));
        assert!(names.contains(&"defined_sym"));
        assert!(names.contains(&"defined_str"));
    }

    #[test]
    fn ruby_dynamic_fixture_method_missing_include_prepend() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/ruby/analyzer_hard_cases_dynamic_method_missing_include_prepend.rb"
        ));
        let ana = SpecRubyAnalyzer::new();

        let syms = ana.symbols_in_file("pkg/ruby_dynamic_method_missing.rb", src);
        assert!(syms.iter().any(|s| {
            s.name == "method_missing"
                && matches!(s.kind, SymbolKind::Method)
                && s.id.0 == "ruby:pkg/ruby_dynamic_method_missing.rb:method:method_missing:17"
        }));
        assert!(syms.iter().any(|s| {
            s.name == "respond_to_missing?"
                && matches!(s.kind, SymbolKind::Method)
                && s.id.0 == "ruby:pkg/ruby_dynamic_method_missing.rb:method:respond_to_missing?:22"
        }));
        assert!(syms.iter().any(|s| {
            s.name == "execute"
                && matches!(s.kind, SymbolKind::Method)
                && s.id.0 == "ruby:pkg/ruby_dynamic_method_missing.rb:method:execute:26"
        }));

        let refs = ana.unresolved_refs("pkg/ruby_dynamic_method_missing.rb", src);
        let names: Vec<_> = refs.iter().map(|r| r.name.as_str()).collect();
        assert!(names.contains(&"include"));
        assert!(names.contains(&"prepend"));
        assert!(names.contains(&"dyn_alpha"));
        assert!(names.contains(&"from_included"));
        assert!(names.contains(&"around_before"));
    }
}
