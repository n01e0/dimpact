use crate::ir::{Symbol, SymbolId};
use crate::ir::reference::{Reference, RefKind, SymbolIndex, UnresolvedRef};
use crate::languages::{LanguageAnalyzer, rust::RustAnalyzer};
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
}

impl Default for ImpactOptions {
    fn default() -> Self { Self { direction: ImpactDirection::Callers, max_depth: Some(100) } }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImpactOutput {
    pub changed_symbols: Vec<Symbol>,
    pub impacted_symbols: Vec<Symbol>,
    pub impacted_files: Vec<String>,
}

/// Build symbol index and resolved reference edges for the current workspace (cwd).
pub fn build_project_graph() -> anyhow::Result<(SymbolIndex, Vec<Reference>)> {
    let analyzer = RustAnalyzer::new(); // for now only rust
    let mut symbols = Vec::new();
    let mut urefs = Vec::new();
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
            if let Some(ext) = path.extension() { if ext != "rs" { continue; } } else { continue; }
            let path_str = path.strip_prefix("./").unwrap_or(path).to_string_lossy().to_string();
            let Ok(src) = fs::read_to_string(path) else { continue; };
            symbols.extend(analyzer.symbols_in_file(&path_str, &src));
            urefs.extend(analyzer.unresolved_refs(&path_str, &src));
        }
    }
    let index = SymbolIndex::build(symbols);
    let refs = resolve_references(&index, &urefs);
    Ok((index, refs))
}

fn resolve_references(index: &SymbolIndex, urefs: &[UnresolvedRef]) -> Vec<Reference> {
    let mut out = Vec::new();
    for r in urefs {
        // find from symbol by containing line
        let Some(from_sym) = index.enclosing_symbol(&r.file, r.line) else { continue };
        // simple name-based resolution
        if let Some(cands) = index.by_name.get(&r.name) {
            for to_sym in cands {
                if !matches!(to_sym.kind, crate::ir::SymbolKind::Function | crate::ir::SymbolKind::Method) {
                    continue;
                }
                out.push(Reference {
                    from: from_sym.id.clone(),
                    to: to_sym.id.clone(),
                    kind: r.kind.clone(),
                    file: r.file.clone(),
                    line: r.line,
                });
            }
        }
    }
    out
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
        if let Some(maxd) = opts.max_depth { if d >= maxd { continue; } }
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

    ImpactOutput { changed_symbols: changed.to_vec(), impacted_symbols, impacted_files }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
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

