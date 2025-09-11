use crate::ir::Symbol;
use crate::ir::reference::{Reference, SymbolIndex, UnresolvedRef};
use crate::languages::{analyzer_for_path, LanguageKind};
use walkdir::WalkDir;
use serde::{Serialize, Deserialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ImpactDirection { Callers, Callees, Both }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImpactOptions {
    pub direction: ImpactDirection,
    pub max_depth: Option<usize>,
    pub with_edges: Option<bool>,
}

impl Default for ImpactOptions {
    fn default() -> Self { Self { direction: ImpactDirection::Callers, max_depth: Some(100), with_edges: Some(false) } }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImpactOutput {
    pub changed_symbols: Vec<Symbol>,
    pub impacted_symbols: Vec<Symbol>,
    pub impacted_files: Vec<String>,
    pub edges: Vec<Reference>,
    pub impacted_by_file: std::collections::HashMap<String, Vec<Symbol>>, // file -> impacted symbols in that file
}

/// Build symbol index and resolved reference edges for the current workspace (cwd).
pub fn build_project_graph() -> anyhow::Result<(SymbolIndex, Vec<Reference>)> {
    let mut symbols = Vec::new();
    let mut urefs = Vec::new();
    let mut file_imports: std::collections::HashMap<String, std::collections::HashMap<String, String>> = std::collections::HashMap::new();
    for entry in WalkDir::new(".")
        .into_iter()
        .filter_entry(|e| {
            let p = e.path();
            let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
            !(name == ".git" || name == "target" || name.starts_with('.'))
        })
        .filter_map(Result::ok) {
        let path = entry.path();
        if path.is_file() {
            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
            if ext != "rs" && ext != "rb" && ext != "js" && ext != "ts" && ext != "tsx" { continue; }
            let path_str = path.strip_prefix("./").unwrap_or(path).to_string_lossy().to_string();
            let Ok(src) = fs::read_to_string(path) else { continue; };
            let kind = if ext == "rs" { LanguageKind::Rust }
                else if ext == "rb" { LanguageKind::Ruby }
                else if ext == "js" { LanguageKind::Javascript }
                else if ext == "ts" { LanguageKind::Typescript }
                else { LanguageKind::Tsx };
            let Some(analyzer) = analyzer_for_path(&path_str, kind) else { continue };
            symbols.extend(analyzer.symbols_in_file(&path_str, &src));
            urefs.extend(analyzer.unresolved_refs(&path_str, &src));
            let im = analyzer.imports_in_file(&path_str, &src);
            file_imports.insert(path_str.clone(), im);
        }
    }
    let index = SymbolIndex::build(symbols);
    let refs = resolve_references(&index, &urefs, &file_imports);
    Ok((index, refs))
}

pub(crate) fn resolve_references(
    index: &SymbolIndex,
    urefs: &[UnresolvedRef],
    file_imports: &std::collections::HashMap<String, std::collections::HashMap<String, String>>,
) -> Vec<Reference> {
    let mut out = Vec::new();
    for r in urefs {
        // find from symbol by containing line
        let Some(from_sym) = index.enclosing_symbol(&r.file, r.line) else { continue };
        // Determine candidate name, considering alias from imports
        let imports = file_imports.get(&r.file).cloned().unwrap_or_default();
        let mut target_name = r.name.as_str();
        let qualifier = r.qualifier.as_deref();
        // normalize qualifier using imports (handle alias on the first segment)
        let from_mod = module_path_for_file(&r.file);
        let norm_qual = qualifier.and_then(|q| normalize_qualifier_with_imports(q, &imports, &from_mod));
        let qualifier = norm_qual.as_deref().or(qualifier);
        let mut imported_prefix: Option<String> = None;
        let mut glob_prefixes: Vec<String> = imports
            .iter()
            .filter_map(|(k,v)| if k.starts_with("__glob__") { Some(v.clone()) } else { None })
            .collect();
        if qualifier.is_none() && let Some(full) = imports.get(&r.name) {
                let prior = full.rsplit_once("::").map(|(p, _)| p).unwrap_or("");
                let ip = if prior.contains("self::") || prior.contains("super::") || prior.contains("crate::") {
                    expand_relative_path(&from_mod, prior)
                } else {
                    prior.to_string()
                };
                imported_prefix = Some(ip);
                target_name = full.rsplit_once("::").map(|(_, n)| n).unwrap_or(full);
        }

        // Re-export fallback: if imported_prefix points to an aggregator module, try to map to the underlying module via its export map
        if let Some(mut ip) = imported_prefix.clone() {
            // resolve through aggregator chain (up to 10 hops, guard cycles)
            let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();
            for _ in 0..10 {
                if !visited.insert(ip.clone()) { break; }
                let mut agg_files: Vec<&String> = file_imports.keys()
                    .filter(|f| file_matches_module_path(f, &ip))
                    .collect();
                if agg_files.len() > 1 {
                    agg_files.sort_by_key(|f| if f.ends_with("/index.js") || f.ends_with("/index.ts") || f.ends_with("/index.tsx") { 0 } else { 1 });
                }
                let Some(agg_path) = agg_files.first() else { break };
                let Some(exp_map) = file_imports.get(*agg_path) else { break };
                for (k, v) in exp_map.iter() { if k.starts_with("__export_glob__") { glob_prefixes.push(v.clone()); } }
                let key = format!("__export__{}", target_name);
                if let Some(real) = exp_map.get(&key) {
                    ip = real.rsplit_once("::").map(|(p, _)| p).unwrap_or("").to_string();
                    imported_prefix = Some(ip.clone());
                    target_name = real.rsplit_once("::").map(|(_, n)| n).unwrap_or(real);
                    continue;
                }
                break;
            }
        }

        // Try candidates by exact name first
        let mut best: Option<&crate::ir::Symbol> = None;
        if let Some(cands) = index.by_name.get(target_name) {
            // If qualifier given, prefer candidates whose module path matches it
            let filtered: Vec<&crate::ir::Symbol> = if let Some(q) = qualifier {
                let v: Vec<_> = cands.iter().filter(|s| file_matches_module_path(&s.file, q)).collect();
                if v.is_empty() { cands.iter().collect() } else { v }
            } else { cands.iter().collect() };
            best = filtered
                .into_iter()
                .filter(|to_sym| matches!(to_sym.kind, crate::ir::SymbolKind::Function | crate::ir::SymbolKind::Method))
                .max_by_key(|to_sym| {
                    let mut best = score_candidate(&r.file, qualifier, imported_prefix.as_deref(), to_sym, r.is_method);
                    for gp in &glob_prefixes {
                        let s = score_candidate(&r.file, qualifier, Some(gp.as_str()), to_sym, r.is_method);
                        if s > best { best = s; }
                    }
                    best
                });
        }

        // Fallback: no same-name match → choose best symbol within the imported/qualified module
        if best.is_none() {
            let mut module_hints: Vec<String> = Vec::new();
            if let Some(q) = qualifier { module_hints.push(q.to_string()); }
            if let Some(ip) = &imported_prefix && !ip.is_empty() { module_hints.push(ip.clone()); }
            for gp in &glob_prefixes { if !module_hints.contains(gp) { module_hints.push(gp.clone()); } }
            if !module_hints.is_empty() {
                let cands: Vec<&crate::ir::Symbol> = index.symbols.iter()
                    .filter(|s| matches!(s.kind, crate::ir::SymbolKind::Function | crate::ir::SymbolKind::Method))
                    .filter(|s| module_hints.iter().any(|mp| file_matches_module_path(&s.file, mp)))
                    .collect();
                if !cands.is_empty() {
                    best = cands.into_iter().max_by_key(|to_sym| {
                        let mut score = score_candidate(&r.file, qualifier, imported_prefix.as_deref(), to_sym, r.is_method);
                        for gp in &glob_prefixes {
                            let s = score_candidate(&r.file, qualifier, Some(gp.as_str()), to_sym, r.is_method);
                            if s > score { score = s; }
                        }
                        score
                    });
                }
            }
        }

        if let Some(to_sym) = best {
            out.push(Reference {
                from: from_sym.id.clone(),
                to: to_sym.id.clone(),
                kind: r.kind.clone(),
                file: r.file.clone(),
                line: r.line,
            });
        }
    }
    out
}

fn score_candidate(from_file: &str, qualifier: Option<&str>, imported_prefix: Option<&str>, cand: &crate::ir::Symbol, call_is_method: bool) -> i32 {
    let mut score = 0;
    if cand.file == from_file { score += 30; }
    // same directory
    if std::path::Path::new(&cand.file).parent() == std::path::Path::new(from_file).parent() { score += 10; }
    if let Some(q) = qualifier && file_matches_module_path(&cand.file, q) { score += 20; }
    if let Some(ip) = imported_prefix && !ip.is_empty() && file_matches_module_path(&cand.file, ip) { score += 15; }
    // prefer method symbol if call site looked like a method
    if call_is_method {
        if matches!(cand.kind, crate::ir::SymbolKind::Method) { score += 25; }
        // Ruby: def は Functionとして表現されることが多いので、メソッド的呼び出しでも許容
        if cand.language == "ruby" && matches!(cand.kind, crate::ir::SymbolKind::Function) { score += 20; }
    } else if matches!(cand.kind, crate::ir::SymbolKind::Function) { score += 5; }
    score
}

fn file_matches_module_path(file: &str, module_path: &str) -> bool {
    if module_path.is_empty() { return false; }
    let base = module_path.replace("::", "/");
    let file_norm = if let Ok(s) = std::path::Path::new(file).strip_prefix("./") { s.to_string_lossy() } else { std::borrow::Cow::from(file) };
    // Match either <base> with supported extensions (and Rust mod.rs), and JS/TS index files
    file_norm.ends_with(&(base.clone() + ".rs"))
        || file_norm.ends_with(&(base.clone() + ".rb"))
        || file_norm.ends_with(&(base.clone() + ".js"))
        || file_norm.ends_with(&(base.clone() + ".ts"))
        || file_norm.ends_with(&(base.clone() + ".tsx"))
        || file_norm.ends_with(&(base.clone() + "/index.js"))
        || file_norm.ends_with(&(base.clone() + "/index.ts"))
        || file_norm.ends_with(&(base.clone() + "/index.tsx"))
        || file_norm.ends_with(&(base + "/mod.rs"))
}

fn normalize_qualifier_with_imports(q: &str, imports: &std::collections::HashMap<String, String>, from_mod: &str) -> Option<String> {
    // Support both Ruby/Rust (::) and JS/TS (.) namespace separators
    let q = q.replace('.', "::");
    // apply alias on first segment, then expand self/super/crate relative to from_mod
    let parts: Vec<&str> = q.split("::").collect();
    if parts.is_empty() { return None; }
    if let Some(mapped) = imports.get(parts[0]) {
        let mut new = mapped.to_string();
        if parts.len() > 1 {
            new.push_str("::");
            new.push_str(&parts[1..].join("::"));
        }
        Some(expand_relative_path(from_mod, &new))
    } else {
        Some(expand_relative_path(from_mod, &q))
    }
}

pub fn module_path_for_file(file: &str) -> String {
    let mut p = std::path::Path::new(file);
    // strip leading ./ if any
    if let Ok(stripped) = p.strip_prefix("./") { p = stripped; }
    let s = p.to_string_lossy();
    if s.ends_with("/mod.rs") || s.ends_with("/lib.rs") || s.ends_with("/main.rs") {
        let dir = p.parent().unwrap_or_else(|| std::path::Path::new(""));
        return dir.to_string_lossy().replace('/', "::");
    }
    if s.ends_with("/index.js") || s.ends_with("/index.ts") || s.ends_with("/index.tsx") {
        let dir = p.parent().unwrap_or_else(|| std::path::Path::new(""));
        return dir.to_string_lossy().replace('/', "::");
    }
    if s.ends_with(".rs") || s.ends_with(".rb") || s.ends_with(".js") || s.ends_with(".ts") || s.ends_with(".tsx") {
        let no_ext = s.trim_end_matches(".rs").trim_end_matches(".rb").trim_end_matches(".js").trim_end_matches(".ts").trim_end_matches(".tsx");
        return no_ext.replace('/', "::");
    }
    s.replace('/', "::")
}

fn expand_relative_path(current_mod: &str, path: &str) -> String {
    if path.starts_with("crate::") { return path.trim_start_matches("crate::").to_string(); }
    let mut rem = path;
    let mut base: Vec<&str> = current_mod.split("::").filter(|s| !s.is_empty()).collect();
    if rem.starts_with("self::") {
        rem = rem.trim_start_matches("self::");
    }
    while rem.starts_with("super::") {
        if !base.is_empty() { base.pop(); }
        rem = rem.trim_start_matches("super::");
    }
    if rem.is_empty() { return base.join("::"); }
    if base.is_empty() { rem.to_string() } else { format!("{}::{}", base.join("::"), rem) }
}

pub fn compute_impact(
    changed: &[Symbol],
    index: &SymbolIndex,
    refs: &[Reference],
    opts: &ImpactOptions,
) -> ImpactOutput {
    let by_id: HashMap<&str, &Symbol> = index.symbols.iter().map(|s| (s.id.0.as_str(), s)).collect();

    // Build adjacency maps
    let mut fwd: HashMap<&str, Vec<&str>> = HashMap::new(); // from -> [to]
    let mut rev: HashMap<&str, Vec<&str>> = HashMap::new(); // to -> [from]
    for e in refs {
        let from = e.from.0.as_str();
        let to = e.to.0.as_str();
        fwd.entry(from).or_default().push(to);
        rev.entry(to).or_default().push(from);
    }

    let mut seen: HashSet<&str> = HashSet::new();
    let mut q: VecDeque<(&str, usize)> = VecDeque::new();
    for s in changed { q.push_back((s.id.0.as_str(), 0)); }
    while let Some((cur, d)) = q.pop_front() {
        if !seen.insert(cur) { continue; }
        if let Some(maxd) = opts.max_depth && d >= maxd { continue; }
        match opts.direction {
            ImpactDirection::Callers => {
                if let Some(nbs) = rev.get(cur) { for &n in nbs { q.push_back((n, d+1)); } }
            }
            ImpactDirection::Callees => {
                if let Some(nbs) = fwd.get(cur) { for &n in nbs { q.push_back((n, d+1)); } }
            }
            ImpactDirection::Both => {
                if let Some(nbs) = rev.get(cur) { for &n in nbs { q.push_back((n, d+1)); } }
                if let Some(nbs) = fwd.get(cur) { for &n in nbs { q.push_back((n, d+1)); } }
            }
        }
    }

    let changed_ids: HashSet<&str> = changed.iter().map(|s| s.id.0.as_str()).collect();
    let mut impacted_symbols: Vec<Symbol> = seen
        .into_iter()
        .filter(|id| !changed_ids.contains(*id))
        .filter_map(|id| by_id.get(id).cloned().cloned())
        .collect();
    impacted_symbols.sort_by(|a,b| a.id.0.cmp(&b.id.0));
    impacted_symbols.dedup_by(|a,b| a.id.0 == b.id.0);

    let mut impacted_files: Vec<String> = impacted_symbols.iter().map(|s| s.file.clone()).collect();
    impacted_files.sort(); impacted_files.dedup();

    let edges = if opts.with_edges.unwrap_or(false) {
        let node_set: std::collections::HashSet<&str> = changed.iter().map(|s| s.id.0.as_str()).chain(by_id.keys().cloned().filter(|id| impacted_symbols.iter().any(|s| s.id.0.as_str()==*id))).collect();
        refs.iter()
            .filter(|e| node_set.contains(e.from.0.as_str()) || node_set.contains(e.to.0.as_str()))
            .cloned()
            .collect()
    } else { Vec::new() };
    let mut impacted_by_file: std::collections::HashMap<String, Vec<Symbol>> = std::collections::HashMap::new();
    for s in &impacted_symbols { impacted_by_file.entry(s.file.clone()).or_default().push(s.clone()); }
    for v in impacted_by_file.values_mut() { v.sort_by(|a,b| a.id.0.cmp(&b.id.0)); v.dedup_by(|a,b| a.id.0 == b.id.0); }
    ImpactOutput { changed_symbols: changed.to_vec(), impacted_symbols, impacted_files, edges, impacted_by_file }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use tempfile::tempdir;

    #[test]
    #[serial]
    fn impact_simple_callers() {
        let td = tempdir().unwrap();
        let f = td.path().join("main.rs");
        let code = r#"fn bar() {}
fn foo() { bar(); }
"#;
        fs::write(&f, code).unwrap();
        let cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(td.path()).unwrap();
        let (index, refs) = build_project_graph().unwrap();
        let bar = index.symbols.iter().find(|s| s.name == "bar").unwrap().clone();
        let out = compute_impact(&[bar], &index, &refs, &ImpactOptions::default());
        std::env::set_current_dir(cwd).unwrap();
        assert!(out.impacted_symbols.iter().any(|s| s.name == "foo"));
    }
}
