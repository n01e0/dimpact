use crate::ir::reference::{RefKind, UnresolvedRef};
use crate::ir::{Symbol, SymbolId, SymbolKind, TextRange};
use crate::languages::LanguageAnalyzer;
use crate::languages::path::normalize_path_like;
use crate::languages::util::{byte_to_line, line_offsets};
use crate::ts_core::{QueryRunner, compile_queries_ruby, load_ruby_spec};
use regex::Regex;

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

#[derive(Clone, Debug)]
enum DynamicNameExpr {
    Literal(String),
    Ref(String),
    InterpolatedPrefix(String),
    Any(Vec<DynamicNameExpr>),
    MapLookup {
        map_name: String,
        key_expr: Option<String>,
    },
}

#[allow(clippy::too_many_arguments)]
fn parse_dynamic_name_expr(
    raw: &str,
    re_symbol_lit: &Regex,
    re_string_lit: &Regex,
    re_var_ref: &Regex,
    re_string_to_sym: &Regex,
    re_symbol_to_s: &Regex,
    re_interpolated_to_sym: &Regex,
    re_var_cast: &Regex,
    re_map_lookup: &Regex,
) -> Option<DynamicNameExpr> {
    let mut expr = raw.trim();
    while expr.starts_with('(') && expr.ends_with(')') && expr.len() >= 2 {
        expr = expr[1..expr.len() - 1].trim();
    }

    if let Some((lhs, rhs)) = expr.split_once("||") {
        let left = parse_dynamic_name_expr(
            lhs.trim(),
            re_symbol_lit,
            re_string_lit,
            re_var_ref,
            re_string_to_sym,
            re_symbol_to_s,
            re_interpolated_to_sym,
            re_var_cast,
            re_map_lookup,
        );
        let right = parse_dynamic_name_expr(
            rhs.trim(),
            re_symbol_lit,
            re_string_lit,
            re_var_ref,
            re_string_to_sym,
            re_symbol_to_s,
            re_interpolated_to_sym,
            re_var_cast,
            re_map_lookup,
        );
        if let (Some(left_expr), Some(right_expr)) = (left, right) {
            return Some(DynamicNameExpr::Any(vec![left_expr, right_expr]));
        }
    }

    if let Some(cap) = re_symbol_lit.captures(expr) {
        return cap
            .get(1)
            .map(|m| DynamicNameExpr::Literal(m.as_str().to_string()));
    }
    if let Some(cap) = re_string_lit.captures(expr) {
        return cap
            .get(1)
            .map(|m| DynamicNameExpr::Literal(m.as_str().to_string()));
    }
    if let Some(cap) = re_string_to_sym.captures(expr) {
        return cap
            .get(1)
            .map(|m| DynamicNameExpr::Literal(m.as_str().to_string()));
    }
    if let Some(cap) = re_symbol_to_s.captures(expr) {
        return cap
            .get(1)
            .map(|m| DynamicNameExpr::Literal(m.as_str().to_string()));
    }
    if let Some(cap) = re_interpolated_to_sym.captures(expr) {
        return cap
            .get(1)
            .map(|m| DynamicNameExpr::InterpolatedPrefix(m.as_str().to_string()));
    }
    if let Some(cap) = re_map_lookup.captures(expr) {
        let map_name = cap.get(1).map(|m| m.as_str().to_string())?;
        let key_expr = cap.get(2).map(|m| m.as_str().trim().to_string());
        return Some(DynamicNameExpr::MapLookup { map_name, key_expr });
    }
    if let Some(cap) = re_var_ref.captures(expr) {
        return cap
            .get(1)
            .map(|m| DynamicNameExpr::Ref(m.as_str().to_string()));
    }
    if let Some(cap) = re_var_cast.captures(expr) {
        return cap
            .get(1)
            .map(|m| DynamicNameExpr::Ref(m.as_str().to_string()));
    }
    None
}

fn dedup_names(names: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    names
        .into_iter()
        .filter(|n| !n.is_empty())
        .filter(|n| seen.insert(n.clone()))
        .collect()
}

fn strip_ruby_inline_comment(expr: &str) -> String {
    let mut out = String::with_capacity(expr.len());
    let mut in_single = false;
    let mut in_double = false;
    let mut escape = false;

    for ch in expr.chars() {
        if escape {
            out.push(ch);
            escape = false;
            continue;
        }
        if (in_single || in_double) && ch == '\\' {
            out.push(ch);
            escape = true;
            continue;
        }
        if !in_double && ch == '\'' {
            in_single = !in_single;
            out.push(ch);
            continue;
        }
        if !in_single && ch == '"' {
            in_double = !in_double;
            out.push(ch);
            continue;
        }
        if !in_single && !in_double && ch == '#' {
            break;
        }
        out.push(ch);
    }

    out.trim().to_string()
}

#[allow(clippy::too_many_arguments)]
fn resolve_dynamic_name_exprs(
    expr: DynamicNameExpr,
    line: u32,
    assigned_exprs: &std::collections::HashMap<String, Vec<(u32, DynamicNameExpr)>>,
    map_entries: &std::collections::HashMap<String, Vec<(String, String)>>,
    re_symbol_lit: &Regex,
    re_string_lit: &Regex,
    re_var_ref: &Regex,
    re_string_to_sym: &Regex,
    re_symbol_to_s: &Regex,
    re_interpolated_to_sym: &Regex,
    re_var_cast: &Regex,
    re_map_lookup: &Regex,
    depth: u8,
) -> Vec<String> {
    if depth == 0 {
        return Vec::new();
    }

    match expr {
        DynamicNameExpr::Literal(v) => vec![v],
        DynamicNameExpr::InterpolatedPrefix(prefix) => {
            let mut names = Vec::new();
            for entries in map_entries.values() {
                for (k, _) in entries {
                    names.push(format!("{prefix}{k}"));
                }
            }
            dedup_names(names)
        }
        DynamicNameExpr::Any(parts) => {
            let mut names = Vec::new();
            for part in parts {
                names.extend(resolve_dynamic_name_exprs(
                    part,
                    line,
                    assigned_exprs,
                    map_entries,
                    re_symbol_lit,
                    re_string_lit,
                    re_var_ref,
                    re_string_to_sym,
                    re_symbol_to_s,
                    re_interpolated_to_sym,
                    re_var_cast,
                    re_map_lookup,
                    depth - 1,
                ));
            }
            dedup_names(names)
        }
        DynamicNameExpr::Ref(var) => {
            let Some(entries) = assigned_exprs.get(&var) else {
                return Vec::new();
            };
            let Some((assigned_line, assigned_expr)) =
                entries.iter().rev().find(|(ln, _)| *ln <= line)
            else {
                return Vec::new();
            };
            resolve_dynamic_name_exprs(
                assigned_expr.clone(),
                *assigned_line,
                assigned_exprs,
                map_entries,
                re_symbol_lit,
                re_string_lit,
                re_var_ref,
                re_string_to_sym,
                re_symbol_to_s,
                re_interpolated_to_sym,
                re_var_cast,
                re_map_lookup,
                depth - 1,
            )
        }
        DynamicNameExpr::MapLookup { map_name, key_expr } => {
            let Some(entries) = map_entries.get(&map_name) else {
                return Vec::new();
            };
            match key_expr {
                None => dedup_names(entries.iter().map(|(_, v)| v.clone()).collect()),
                Some(key_raw) => {
                    let mut key_candidates = Vec::new();
                    if let Some(kexpr) = parse_dynamic_name_expr(
                        &key_raw,
                        re_symbol_lit,
                        re_string_lit,
                        re_var_ref,
                        re_string_to_sym,
                        re_symbol_to_s,
                        re_interpolated_to_sym,
                        re_var_cast,
                        re_map_lookup,
                    ) {
                        key_candidates = resolve_dynamic_name_exprs(
                            kexpr,
                            line,
                            assigned_exprs,
                            map_entries,
                            re_symbol_lit,
                            re_string_lit,
                            re_var_ref,
                            re_string_to_sym,
                            re_symbol_to_s,
                            re_interpolated_to_sym,
                            re_var_cast,
                            re_map_lookup,
                            depth - 1,
                        );
                    }
                    if key_candidates.is_empty() {
                        dedup_names(entries.iter().map(|(_, v)| v.clone()).collect())
                    } else {
                        let key_set: std::collections::HashSet<String> =
                            key_candidates.into_iter().collect();
                        dedup_names(
                            entries
                                .iter()
                                .filter(|(k, _)| key_set.contains(k))
                                .map(|(_, v)| v.clone())
                                .collect(),
                        )
                    }
                }
            }
        }
    }
}

fn find_method_call_start(text: &str, method_name: &str) -> Option<usize> {
    let mut seek_from = 0usize;
    while seek_from < text.len() {
        let rel = text[seek_from..].find(method_name)?;
        let idx = seek_from + rel;
        let before = text[..idx].chars().next_back();
        let after = text[idx + method_name.len()..].chars().next();
        let is_ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
        if before.is_none_or(|c| !is_ident(c)) && after.is_none_or(|c| !is_ident(c)) {
            return Some(idx);
        }
        seek_from = idx + method_name.len();
    }
    None
}

fn extract_call_args(text: &str, method_name: &str, max_args: usize) -> Vec<String> {
    if max_args == 0 {
        return Vec::new();
    }

    let Some(idx) = find_method_call_start(text, method_name) else {
        return Vec::new();
    };
    let rest = text[idx + method_name.len()..].trim_start();
    if rest.is_empty() {
        return Vec::new();
    }

    let mut has_parens = false;
    let body = if let Some(paren_body) = rest.strip_prefix('(') {
        has_parens = true;
        let mut depth = 1i32;
        let mut in_single = false;
        let mut in_double = false;
        let mut escaped = false;
        let mut end = paren_body.len();

        for (i, ch) in paren_body.char_indices() {
            if escaped {
                escaped = false;
                continue;
            }
            if in_single {
                if ch == '\\' {
                    escaped = true;
                } else if ch == '\'' {
                    in_single = false;
                }
                continue;
            }
            if in_double {
                if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    in_double = false;
                }
                continue;
            }

            match ch {
                '\'' => in_single = true,
                '"' => in_double = true,
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = i;
                        break;
                    }
                }
                _ => {}
            }
        }

        &paren_body[..end]
    } else {
        rest
    };

    let mut out = Vec::new();
    let mut start = 0usize;
    let mut depth_paren = 0i32;
    let mut depth_brack = 0i32;
    let mut depth_brace = 0i32;
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;

    let push_segment = |start_idx: usize, end_idx: usize, out: &mut Vec<String>| {
        if out.len() >= max_args {
            return;
        }
        let seg = body[start_idx..end_idx].trim();
        if !seg.is_empty() {
            out.push(seg.to_string());
        }
    };

    for (i, ch) in body.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if in_single {
            if ch == '\\' {
                escaped = true;
            } else if ch == '\'' {
                in_single = false;
            }
            continue;
        }
        if in_double {
            if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_double = false;
            }
            continue;
        }

        match ch {
            '\'' => in_single = true,
            '"' => in_double = true,
            '(' => depth_paren += 1,
            ')' => {
                if depth_paren == 0 && depth_brack == 0 && depth_brace == 0 {
                    push_segment(start, i, &mut out);
                    start = i;
                    break;
                }
                depth_paren -= 1;
            }
            '[' => depth_brack += 1,
            ']' => depth_brack -= 1,
            '{' if !has_parens && depth_paren == 0 && depth_brack == 0 && depth_brace == 0 => {
                push_segment(start, i, &mut out);
                start = i;
                break;
            }
            '{' => depth_brace += 1,
            '}' => depth_brace -= 1,
            ',' if depth_paren == 0 && depth_brack == 0 && depth_brace == 0 => {
                push_segment(start, i, &mut out);
                start = i + ch.len_utf8();
                if out.len() >= max_args {
                    break;
                }
            }
            '\n' | ';' if depth_paren == 0 && depth_brack == 0 && depth_brace == 0 => {
                push_segment(start, i, &mut out);
                start = i;
                break;
            }
            c if !has_parens
                && c.is_whitespace()
                && depth_paren == 0
                && depth_brack == 0
                && depth_brace == 0 =>
            {
                let rem = body[i..].trim_start();
                if rem == "do" || rem.starts_with("do ") {
                    push_segment(start, i, &mut out);
                    start = i;
                    break;
                }
            }
            _ => {}
        }
    }

    if out.len() < max_args {
        let seg = body[start..].trim();
        if !seg.is_empty() {
            out.push(seg.to_string());
        }
    }

    out.truncate(max_args);
    out
}

fn extract_first_send_like_arg(text: &str, method_name: &str) -> Option<String> {
    extract_call_args(text, method_name, 1).into_iter().next()
}

fn extract_ruby_const_path(arg_raw: &str) -> Option<String> {
    let mut s = arg_raw.trim();
    while s.starts_with('(') && s.ends_with(')') && s.len() >= 2 {
        s = s[1..s.len() - 1].trim();
    }
    if let Some(rest) = s.strip_prefix("::") {
        s = rest;
    }
    if s.is_empty() {
        return None;
    }
    let seg_ok = |seg: &str| {
        let mut chars = seg.chars();
        let Some(first) = chars.next() else {
            return false;
        };
        first.is_ascii_uppercase() && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
    };
    if s.split("::").all(seg_ok) {
        Some(s.to_string())
    } else {
        None
    }
}

fn snake_case_ruby_const_segment(seg: &str) -> String {
    let mut out = String::new();
    let mut prev_is_lower_or_digit = false;
    for ch in seg.chars() {
        if ch.is_ascii_uppercase() {
            if prev_is_lower_or_digit {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
            prev_is_lower_or_digit = false;
        } else {
            out.push(ch);
            prev_is_lower_or_digit = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        }
    }
    out
}

fn ruby_module_hints(const_path: &str) -> Vec<String> {
    let raw = const_path.trim().trim_start_matches("::");
    if raw.is_empty() {
        return Vec::new();
    }
    let mut out = vec![raw.to_string()];
    out.push(raw.replace("::", "/"));

    let snake_segments: Vec<String> = raw.split("::").map(snake_case_ruby_const_segment).collect();
    if !snake_segments.is_empty() {
        out.push(snake_segments.join("::"));
        out.push(snake_segments.join("/"));
    }

    let mut dedup = std::collections::HashSet::new();
    out.into_iter()
        .filter(|h| dedup.insert(h.clone()))
        .collect()
}

fn collect_module_methods_by_hint(
    source: &str,
) -> std::collections::HashMap<String, std::collections::HashSet<String>> {
    let lines: Vec<&str> = source.lines().collect();
    let re_module =
        Regex::new(r"^\s*module\s+([A-Z][A-Za-z0-9_]*(?:::[A-Z][A-Za-z0-9_]*)*)").unwrap();
    let re_def = Regex::new(r"^\s*def\s+([A-Za-z_][A-Za-z0-9_?!]*)\b").unwrap();

    let mut map: std::collections::HashMap<String, std::collections::HashSet<String>> =
        std::collections::HashMap::new();

    for (start_idx, line) in lines.iter().enumerate() {
        let Some(cap) = re_module.captures(line) else {
            continue;
        };
        let Some(mod_name) = cap.get(1).map(|m| m.as_str()) else {
            continue;
        };
        let end_idx = find_ruby_block_end(&lines, start_idx);
        let mut methods: std::collections::HashSet<String> = std::collections::HashSet::new();
        for inner in lines.iter().take(end_idx + 1).skip(start_idx + 1) {
            if let Some(def_cap) = re_def.captures(inner)
                && let Some(m) = def_cap.get(1)
            {
                methods.insert(m.as_str().to_string());
            }
        }
        if methods.is_empty() {
            continue;
        }

        for hint in ruby_module_hints(mod_name) {
            map.entry(hint).or_default().extend(methods.iter().cloned());
        }
    }

    map
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
        let module_methods_by_hint = collect_module_methods_by_hint(source);
        let mut mixin_hints: std::collections::HashSet<String> = std::collections::HashSet::new();

        // Track simple assignment chains for send/public_send argument tracing.
        // Examples:
        //   dyn_sym = :target
        //   @dyn = "target"
        //   TARGET = :target
        //   alias = dyn_sym
        //   dyn = "target".to_sym
        let re_assign_expr = Regex::new(
            r#"(?m)^\s*([@]{0,2}[A-Za-z_][A-Za-z0-9_]*|[A-Z][A-Za-z0-9_]*)\s*=\s*([^\n]+?)\s*$"#,
        )
        .unwrap();
        let re_symbol_lit = Regex::new(r#"^:([A-Za-z_][A-Za-z0-9_?!]*)$"#).unwrap();
        let re_string_lit = Regex::new(r#"^[\"']([A-Za-z_][A-Za-z0-9_?!]*)[\"']$"#).unwrap();
        let re_var_ref =
            Regex::new(r#"^([@]{0,2}[A-Za-z_][A-Za-z0-9_]*|[A-Z][A-Za-z0-9_]*)$"#).unwrap();
        let re_string_to_sym =
            Regex::new(r#"^[\"']([A-Za-z_][A-Za-z0-9_?!]*)[\"']\.to_sym$"#).unwrap();
        let re_symbol_to_s = Regex::new(r#"^:([A-Za-z_][A-Za-z0-9_?!]*)\.to_s$"#).unwrap();
        let re_interpolated_to_sym =
            Regex::new(r#"^[\"']([A-Za-z_][A-Za-z0-9_?!]*)#\{[^}]+\}[\"']\.to_sym$"#)
                .unwrap();
        let re_var_cast =
            Regex::new(r#"^([@]{0,2}[A-Za-z_][A-Za-z0-9_]*|[A-Z][A-Za-z0-9_]*)\.(?:to_sym|to_s)$"#)
                .unwrap();
        let re_map_lookup = Regex::new(r#"^([A-Z][A-Za-z0-9_]*)\s*\[\s*([^\]]+)\s*\]$"#).unwrap();

        let re_const_hash_block =
            Regex::new(r#"(?ms)([A-Z][A-Za-z0-9_]*)\s*=\s*\{(.*?)\}\s*(?:\.freeze)?"#).unwrap();
        let re_hash_entry = Regex::new(
            r#"(?:\"([A-Za-z_][A-Za-z0-9_?!]*)\"|'([A-Za-z_][A-Za-z0-9_?!]*)'|:([A-Za-z_][A-Za-z0-9_?!]*))\s*=>\s*(?::([A-Za-z_][A-Za-z0-9_?!]*)|\"([A-Za-z_][A-Za-z0-9_?!]*)\"|'([A-Za-z_][A-Za-z0-9_?!]*)')"#,
        )
        .unwrap();
        let mut map_entries: std::collections::HashMap<String, Vec<(String, String)>> =
            std::collections::HashMap::new();
        for caps in re_const_hash_block.captures_iter(source) {
            let Some(const_name) = caps.get(1).map(|m| m.as_str().to_string()) else {
                continue;
            };
            let Some(body) = caps.get(2).map(|m| m.as_str()) else {
                continue;
            };
            let mut entries = Vec::new();
            for ecap in re_hash_entry.captures_iter(body) {
                let key = ecap
                    .get(1)
                    .or_else(|| ecap.get(2))
                    .or_else(|| ecap.get(3))
                    .map(|m| m.as_str().to_string());
                let value = ecap
                    .get(4)
                    .or_else(|| ecap.get(5))
                    .or_else(|| ecap.get(6))
                    .map(|m| m.as_str().to_string());
                if let (Some(k), Some(v)) = (key, value) {
                    entries.push((k, v));
                }
            }
            if !entries.is_empty() {
                map_entries.insert(const_name, entries);
            }
        }

        let mut assigned_exprs: std::collections::HashMap<String, Vec<(u32, DynamicNameExpr)>> =
            std::collections::HashMap::new();
        for caps in re_assign_expr.captures_iter(source) {
            let Some(full) = caps.get(0) else {
                continue;
            };
            let Some(var) = caps.get(1) else {
                continue;
            };
            let Some(rhs) = caps.get(2) else {
                continue;
            };
            let rhs_clean = strip_ruby_inline_comment(rhs.as_str());
            if rhs_clean.is_empty() {
                continue;
            }
            let ln = byte_to_line(&offs, full.start());
            if let Some(expr) = parse_dynamic_name_expr(
                rhs_clean.as_str(),
                &re_symbol_lit,
                &re_string_lit,
                &re_var_ref,
                &re_string_to_sym,
                &re_symbol_to_s,
                &re_interpolated_to_sym,
                &re_var_cast,
                &re_map_lookup,
            ) {
                assigned_exprs
                    .entry(var.as_str().to_string())
                    .or_default()
                    .push((ln, expr));
            }
        }
        for entries in assigned_exprs.values_mut() {
            entries.sort_by_key(|(ln, _)| *ln);
        }

        let resolve_dynamic_names = |arg_raw: &str, ln: u32| -> Vec<String> {
            let Some(expr) = parse_dynamic_name_expr(
                arg_raw,
                &re_symbol_lit,
                &re_string_lit,
                &re_var_ref,
                &re_string_to_sym,
                &re_symbol_to_s,
                &re_interpolated_to_sym,
                &re_var_cast,
                &re_map_lookup,
            ) else {
                return Vec::new();
            };
            dedup_names(resolve_dynamic_name_exprs(
                expr,
                ln,
                &assigned_exprs,
                &map_entries,
                &re_symbol_lit,
                &re_string_lit,
                &re_var_ref,
                &re_string_to_sym,
                &re_symbol_to_s,
                &re_interpolated_to_sym,
                &re_var_cast,
                &re_map_lookup,
                8,
            ))
        };
        let resolve_dynamic_name = |arg_raw: &str, ln: u32| -> Option<String> {
            resolve_dynamic_names(arg_raw, ln).into_iter().next()
        };

        for caps in self.runner.run_captures(source, &self.queries.calls) {
            let name_cap = caps.iter().find(|c| c.name == "name");
            if let Some(n) = name_cap {
                let name = source[n.start..n.end].to_string();
                if name.is_empty() {
                    continue;
                }
                let ln = if let Some(callnode) = caps.iter().find(|c| c.name == "call") {
                    byte_to_line(&offs, callnode.start)
                } else {
                    byte_to_line(&offs, n.start)
                };
                let mut suppress_base_call_ref = false;
                if let Some(callnode) = caps.iter().find(|c| c.name == "call") {
                    let text = &source[callnode.start..callnode.end];

                    if (name == "send" || name == "public_send")
                        && let Some(arg_raw) = extract_first_send_like_arg(text, &name)
                    {
                        let resolved_names = resolve_dynamic_names(&arg_raw, ln);
                        if !resolved_names.is_empty() {
                            suppress_base_call_ref = true;
                            for resolved in resolved_names {
                                out.push(UnresolvedRef {
                                    name: resolved,
                                    kind: RefKind::Call,
                                    file: path.to_string(),
                                    line: ln,
                                    qualifier: None,
                                    is_method: true,
                                });
                            }
                        }
                    }

                    if name == "include" || name == "extend" || name == "prepend" {
                        for arg_raw in extract_call_args(text, &name, 8) {
                            if let Some(mod_path) = extract_ruby_const_path(&arg_raw) {
                                for hint in ruby_module_hints(&mod_path) {
                                    mixin_hints.insert(hint);
                                }
                            }
                        }
                    }

                    // Conservative edge for alias_method: add refs for both alias and original targets.
                    if name == "alias_method" {
                        for arg_raw in extract_call_args(text, "alias_method", 2) {
                            if let Some(resolved) = resolve_dynamic_name(&arg_raw, ln) {
                                out.push(UnresolvedRef {
                                    name: resolved,
                                    kind: RefKind::Call,
                                    file: path.to_string(),
                                    line: ln,
                                    qualifier: None,
                                    is_method: true,
                                });
                            }
                        }
                    }

                    // Conservative edge for define_method: add ref for defined target name.
                    if name == "define_method"
                        && let Some(arg_raw) = extract_call_args(text, "define_method", 1)
                            .into_iter()
                            .next()
                        && let Some(resolved) = resolve_dynamic_name(&arg_raw, ln)
                    {
                        out.push(UnresolvedRef {
                            name: resolved,
                            kind: RefKind::Call,
                            file: path.to_string(),
                            line: ln,
                            qualifier: None,
                            is_method: true,
                        });
                    }
                }
                if !suppress_base_call_ref {
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

        // Bridge include/extend/prepend resolution to call inference:
        // if a call name matches methods found on mixin modules, emit a qualified ref
        // so resolver scoring can prefer the corresponding module path.
        if !mixin_hints.is_empty() && !module_methods_by_hint.is_empty() {
            let mut seen_qualified: std::collections::HashSet<(u32, String, String)> = out
                .iter()
                .filter_map(|r| {
                    r.qualifier
                        .as_ref()
                        .map(|q| (r.line, r.name.clone(), q.clone()))
                })
                .collect();

            for r in out.clone() {
                if !r.is_method || r.qualifier.is_some() {
                    continue;
                }
                if r.name == "include"
                    || r.name == "extend"
                    || r.name == "prepend"
                    || r.name == "method_missing"
                    || r.name == "respond_to_missing?"
                {
                    continue;
                }

                for hint in &mixin_hints {
                    let Some(methods) = module_methods_by_hint.get(hint) else {
                        continue;
                    };
                    if !methods.contains(&r.name) {
                        continue;
                    }
                    if seen_qualified.insert((r.line, r.name.clone(), hint.clone())) {
                        out.push(UnresolvedRef {
                            name: r.name.clone(),
                            kind: RefKind::Call,
                            file: path.to_string(),
                            line: r.line,
                            qualifier: Some(hint.clone()),
                            is_method: true,
                        });
                    }
                }
            }
        }

        // Conservative dynamic-dispatch fallback edges:
        // when unresolved call names match the method_missing/respond_to_missing? naming policy,
        // add fallback refs so downstream impact analysis can keep dynamic paths visible.
        let re_method_missing_def = Regex::new(r"(?m)^\s*def\s+method_missing\b").unwrap();
        let re_respond_to_missing_def = Regex::new(r"(?m)^\s*def\s+respond_to_missing\?").unwrap();
        let has_method_missing = re_method_missing_def.is_match(source);
        let has_respond_to_missing = re_respond_to_missing_def.is_match(source);
        if has_method_missing || has_respond_to_missing {
            let re_method_def = Regex::new(r"(?m)^\s*def\s+([A-Za-z_][A-Za-z0-9_?!]*)\b").unwrap();
            let declared_methods: std::collections::HashSet<String> = re_method_def
                .captures_iter(source)
                .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
                .collect();

            let re_dyn_prefix = Regex::new(
                r#"(?:name(?:\.to_s)?|[A-Za-z_][A-Za-z0-9_]*)\.start_with\?\(\s*(?:\"([^\"]+)\"|'([^']+)')\s*\)"#,
            )
            .unwrap();
            let dyn_prefixes: Vec<String> = re_dyn_prefix
                .captures_iter(source)
                .filter_map(|c| {
                    c.get(1)
                        .or_else(|| c.get(2))
                        .map(|m| m.as_str().to_string())
                })
                .collect();

            let mut seen_method_missing: std::collections::HashSet<u32> = out
                .iter()
                .filter(|r| r.name == "method_missing")
                .map(|r| r.line)
                .collect();
            let mut seen_respond_to_missing: std::collections::HashSet<u32> = out
                .iter()
                .filter(|r| r.name == "respond_to_missing?")
                .map(|r| r.line)
                .collect();

            for r in out.clone() {
                if !r.is_method || r.qualifier.is_some() {
                    continue;
                }
                if r.name == "method_missing" || r.name == "respond_to_missing?" {
                    continue;
                }
                if declared_methods.contains(&r.name) {
                    continue;
                }
                if !dyn_prefixes.is_empty() && !dyn_prefixes.iter().any(|p| r.name.starts_with(p)) {
                    continue;
                }

                if has_method_missing && seen_method_missing.insert(r.line) {
                    out.push(UnresolvedRef {
                        name: "method_missing".to_string(),
                        kind: RefKind::Call,
                        file: path.to_string(),
                        line: r.line,
                        qualifier: None,
                        is_method: true,
                    });
                }

                if has_respond_to_missing && seen_respond_to_missing.insert(r.line) {
                    out.push(UnresolvedRef {
                        name: "respond_to_missing?".to_string(),
                        kind: RefKind::Call,
                        file: path.to_string(),
                        line: r.line,
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
    fn ruby_send_public_send_tracks_assignment_chains_and_reassignments() {
        let src = r#"CONST_SYM = :target_sym
CONST_STR = "target_str"

class DynamicDispatch
  def target_sym
    :ok
  end

  def target_str
    :ok
  end

  def execute
    dyn_sym = CONST_SYM
    send(dyn_sym)

    dyn_alias = dyn_sym
    public_send(dyn_alias)

    dyn_name = CONST_STR
    send(dyn_name)

    dyn_name = "target_sym"
    public_send(dyn_name)
  end
end
"#;
        let ana = SpecRubyAnalyzer::new();

        let refs = ana.unresolved_refs("pkg/ruby_dynamic_chain.rb", src);
        let names: Vec<_> = refs.iter().map(|r| r.name.as_str()).collect();
        let refs_dbg: Vec<_> = refs
            .iter()
            .map(|r| format!("{}@{}", r.name, r.line))
            .collect();

        assert!(
            names.iter().filter(|&&n| n == "target_sym").count() >= 3,
            "names={names:?} refs={refs_dbg:?}"
        );
        assert!(
            names.iter().filter(|&&n| n == "target_str").count() >= 1,
            "names={names:?}"
        );
        assert!(!names.contains(&"send"), "names={names:?}");
        assert!(!names.contains(&"public_send"), "names={names:?}");
    }

    #[test]
    fn extract_call_args_handles_define_method_block_form() {
        let text = "define_method dyn_name.to_s do";
        let args = extract_call_args(text, "define_method", 1);
        assert_eq!(args, vec!["dyn_name.to_s"]);

        let text = "alias_method alias_name.to_sym, original_name";
        let args = extract_call_args(text, "alias_method", 2);
        assert_eq!(args, vec!["alias_name.to_sym", "original_name"]);
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
                && s.id.0 == "ruby:pkg/ruby_dynamic_alias_define.rb:method:execute:21"
        }));

        let refs = ana.unresolved_refs("pkg/ruby_dynamic_alias_define.rb", src);
        let names: Vec<_> = refs.iter().map(|r| r.name.as_str()).collect();
        assert!(names.contains(&"alias_method"));
        assert!(names.contains(&"define_method"));
        assert!(names.contains(&"original"));
        assert!(names.contains(&"aliased_sym"));
        assert!(names.contains(&"aliased_str"));
        assert!(names.contains(&"defined_sym"));
        assert!(names.contains(&"defined_str"));
        assert!(names.contains(&"defined_only"));
    }

    #[test]
    fn ruby_alias_define_tracks_assignment_chain_in_conservative_edges() {
        let src = r#"ORIGINAL_NAME = :original
ALIAS_NAME = "aliased"

class AliasDefineChain
  def original
    :ok
  end

  alias_target = ALIAS_NAME.to_sym
  source_name = ORIGINAL_NAME
  alias_method alias_target, source_name

  define_target = alias_target.to_s
  define_method define_target do
    original
  end

  def execute
    aliased
  end
end
"#;
        let ana = SpecRubyAnalyzer::new();

        let refs = ana.unresolved_refs("pkg/ruby_alias_define_chain.rb", src);
        let names: Vec<_> = refs.iter().map(|r| r.name.as_str()).collect();

        assert!(names.contains(&"alias_method"));
        assert!(names.contains(&"define_method"));
        assert!(names.contains(&"original"));
        assert!(names.contains(&"aliased"));
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
        assert!(refs.iter().any(|r| {
            r.name == "from_included" && r.line == 28 && r.qualifier.is_some() && r.is_method
        }));
        assert!(refs.iter().any(|r| {
            r.name == "around_before" && r.line == 29 && r.qualifier.is_some() && r.is_method
        }));
        let refs_dbg: Vec<_> = refs
            .iter()
            .map(|r| match &r.qualifier {
                Some(q) => format!("{}@{}[{}]", r.name, r.line, q),
                None => format!("{}@{}", r.name, r.line),
            })
            .collect();
        assert!(
            refs.iter().any(|r| {
                r.name == "method_missing" && r.line == 27 && r.qualifier.is_none() && r.is_method
            }),
            "refs={refs_dbg:?}"
        );
        assert!(
            refs.iter().any(|r| {
                r.name == "respond_to_missing?"
                    && r.line == 27
                    && r.qualifier.is_none()
                    && r.is_method
            }),
            "refs={refs_dbg:?}"
        );
    }

    #[test]
    fn ruby_dynamic_fixture_dsl_method_missing_dispatch_chain() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/ruby/analyzer_hard_cases_dynamic_dsl_method_missing_chain.rb"
        ));
        let ana = SpecRubyAnalyzer::new();

        let refs = ana.unresolved_refs("pkg/ruby_dynamic_dsl_chain.rb", src);
        let names: Vec<_> = refs.iter().map(|r| r.name.as_str()).collect();
        let refs_dbg: Vec<_> = refs
            .iter()
            .map(|r| match &r.qualifier {
                Some(q) => format!("{}@{}[{}]", r.name, r.line, q),
                None => format!("{}@{}", r.name, r.line),
            })
            .collect();

        assert!(names.contains(&"handle_create"), "refs={refs_dbg:?}");
        assert!(names.contains(&"handle_update"), "refs={refs_dbg:?}");
        assert!(names.contains(&"dsl_create"), "refs={refs_dbg:?}");
        assert!(names.contains(&"dsl_unknown"), "refs={refs_dbg:?}");
        assert!(!names.contains(&"public_send"), "refs={refs_dbg:?}");
        assert!(
            refs.iter().any(|r| {
                r.name == "method_missing" && r.line == 48 && r.qualifier.is_none() && r.is_method
            }),
            "refs={refs_dbg:?}"
        );
        assert!(
            refs.iter().any(|r| {
                r.name == "respond_to_missing?"
                    && r.line == 48
                    && r.qualifier.is_none()
                    && r.is_method
            }),
            "refs={refs_dbg:?}"
        );
    }

    #[test]
    fn ruby_dynamic_fixture_dsl_method_missing_dispatch_chain_v2() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/ruby/analyzer_hard_cases_dynamic_dsl_method_missing_chain_v2.rb"
        ));
        let ana = SpecRubyAnalyzer::new();

        let refs = ana.unresolved_refs("pkg/ruby_dynamic_dsl_chain_v2.rb", src);
        let names: Vec<_> = refs.iter().map(|r| r.name.as_str()).collect();
        let refs_dbg: Vec<_> = refs
            .iter()
            .map(|r| match &r.qualifier {
                Some(q) => format!("{}@{}[{}]", r.name, r.line, q),
                None => format!("{}@{}", r.name, r.line),
            })
            .collect();

        assert!(names.contains(&"route_create"), "refs={refs_dbg:?}");
        assert!(names.contains(&"route_delete"), "refs={refs_dbg:?}");
        assert!(names.contains(&"emit_created"), "refs={refs_dbg:?}");
        assert!(names.contains(&"emit_deleted"), "refs={refs_dbg:?}");
        assert!(names.contains(&"emit_unknown"), "refs={refs_dbg:?}");
        assert!(!names.contains(&"send"), "refs={refs_dbg:?}");
        assert!(!names.contains(&"public_send"), "refs={refs_dbg:?}");

        assert!(
            refs.iter().any(|r| {
                r.name == "method_missing" && r.line == 81 && r.qualifier.is_none() && r.is_method
            }),
            "refs={refs_dbg:?}"
        );
        assert!(
            refs.iter().any(|r| {
                r.name == "respond_to_missing?"
                    && r.line == 81
                    && r.qualifier.is_none()
                    && r.is_method
            }),
            "refs={refs_dbg:?}"
        );
    }

    #[test]
    fn ruby_dynamic_fixture_resolver_combo_regression() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/ruby/analyzer_hard_cases_dynamic_resolver_combo.rb"
        ));
        let ana = SpecRubyAnalyzer::new();

        let refs = ana.unresolved_refs("pkg/ruby_dynamic_resolver_combo.rb", src);
        let names: Vec<_> = refs.iter().map(|r| r.name.as_str()).collect();
        let refs_dbg: Vec<_> = refs
            .iter()
            .map(|r| match &r.qualifier {
                Some(q) => format!("{}@{}[{}]", r.name, r.line, q),
                None => format!("{}@{}", r.name, r.line),
            })
            .collect();

        assert!(names.contains(&"base_alias"), "refs={refs_dbg:?}");
        assert!(names.contains(&"base_defined"), "refs={refs_dbg:?}");
        assert!(names.contains(&"included_api"), "refs={refs_dbg:?}");
        assert!(names.contains(&"mixed_from_prepend"), "refs={refs_dbg:?}");
        assert!(names.contains(&"class_api"), "refs={refs_dbg:?}");
        assert!(
            refs.iter().any(|r| {
                r.name == "included_api" && r.line == 47 && r.qualifier.is_some() && r.is_method
            }),
            "refs={refs_dbg:?}"
        );
        assert!(
            refs.iter().any(|r| {
                r.name == "mixed_from_prepend"
                    && r.line == 48
                    && r.qualifier.is_some()
                    && r.is_method
            }),
            "refs={refs_dbg:?}"
        );
        assert!(
            refs.iter().any(|r| {
                r.name == "class_api" && r.line == 49 && r.qualifier.is_some() && r.is_method
            }),
            "refs={refs_dbg:?}"
        );
        assert!(
            refs.iter().any(|r| {
                r.name == "method_missing" && r.line == 50 && r.qualifier.is_none() && r.is_method
            }),
            "refs={refs_dbg:?}"
        );
        assert!(
            refs.iter().any(|r| {
                r.name == "respond_to_missing?"
                    && r.line == 50
                    && r.qualifier.is_none()
                    && r.is_method
            }),
            "refs={refs_dbg:?}"
        );
        assert!(!names.contains(&"send"), "refs={refs_dbg:?}");
        assert!(!names.contains(&"public_send"), "refs={refs_dbg:?}");
    }

    #[test]
    fn ruby_extend_module_connects_to_call_inference_via_qualified_ref() {
        let src = r#"module ClassMixin
  def class_api
    :ok
  end
end

class UsesExtend
  extend ClassMixin

  def execute
    self.class_api
  end
end
"#;
        let ana = SpecRubyAnalyzer::new();

        let refs = ana.unresolved_refs("pkg/ruby_extend_inference.rb", src);
        assert!(refs.iter().any(|r| {
            r.name == "class_api" && r.line == 11 && r.qualifier.is_some() && r.is_method
        }));
    }

    #[test]
    fn ruby_respond_to_missing_only_adds_fallback_edge() {
        let src = r#"class RespondOnly
  def respond_to_missing?(name, include_private = false)
    name.to_s.start_with?("dyn_") || super
  end

  def execute
    dyn_beta
  end
end
"#;
        let ana = SpecRubyAnalyzer::new();

        let refs = ana.unresolved_refs("pkg/ruby_dynamic_respond_only.rb", src);
        assert!(refs.iter().any(|r| {
            r.name == "respond_to_missing?" && r.line == 7 && r.qualifier.is_none() && r.is_method
        }));
        assert!(
            !refs
                .iter()
                .any(|r| r.name == "method_missing" && r.line == 7)
        );
    }
}
