use crate::ir::Symbol;
use crate::ir::reference::{EdgeProvenance, RefKind, Reference, SymbolIndex, UnresolvedRef};
use crate::languages::{LanguageKind, analyzer_for_path};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ImpactDirection {
    Callers,
    Callees,
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImpactOptions {
    pub direction: ImpactDirection,
    pub max_depth: Option<usize>,
    pub with_edges: Option<bool>,
    /// Directories to ignore (relative path prefixes). If a symbol's file
    /// path starts with any of these prefixes (or equals to it), the symbol
    /// is excluded from seeds and results.
    #[serde(default)]
    pub ignore_dirs: Vec<String>,
}

impl Default for ImpactOptions {
    fn default() -> Self {
        Self {
            direction: ImpactDirection::Callers,
            max_depth: Some(100),
            with_edges: Some(false),
            ignore_dirs: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ImpactSummary {
    #[serde(default)]
    pub by_depth: Vec<ImpactDepthBucket>,
    #[serde(default)]
    pub affected_modules: Vec<ImpactAffectedModule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk: Option<ImpactRiskSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slice_selection: Option<ImpactSliceSelectionSummary>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ImpactSlicePlannerKind {
    BoundedSlice,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImpactSliceSelectionSummary {
    pub planner: ImpactSlicePlannerKind,
    #[serde(default)]
    pub files: Vec<ImpactSliceFileMetadata>,
    #[serde(default)]
    pub pruned_candidates: Vec<ImpactSlicePrunedCandidate>,
}

impl Default for ImpactSliceSelectionSummary {
    fn default() -> Self {
        Self {
            planner: ImpactSlicePlannerKind::BoundedSlice,
            files: Vec::new(),
            pruned_candidates: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImpactSliceFileMetadata {
    pub path: String,
    pub scopes: ImpactSliceScopes,
    #[serde(default)]
    pub reasons: Vec<ImpactSliceReasonMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ImpactSliceScopes {
    pub cache_update: bool,
    pub local_dfg: bool,
    pub explanation: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ImpactSliceReasonMetadata {
    pub seed_symbol_id: String,
    pub tier: u8,
    pub kind: ImpactSliceReasonKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub via_symbol_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub via_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge_kind: Option<ImpactSliceBridgeKind>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ImpactSliceReasonKind {
    SeedFile,
    ChangedFile,
    DirectCallerFile,
    DirectCalleeFile,
    BridgeCompletionFile,
    ModuleCompanionFile,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ImpactSliceBridgeKind {
    WrapperReturn,
    BoundaryAliasContinuation,
    RequireRelativeChain,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ImpactSlicePrunedCandidate {
    pub seed_symbol_id: String,
    pub path: String,
    pub tier: u8,
    pub kind: ImpactSliceReasonKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub via_symbol_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub via_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge_kind: Option<ImpactSliceBridgeKind>,
    pub prune_reason: ImpactSlicePruneReason,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ImpactSlicePruneReason {
    AlreadySelected,
    BridgeBudgetExhausted,
    CacheUpdateBudgetExhausted,
    LocalDfgBudgetExhausted,
    RankedOut,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImpactDepthBucket {
    pub depth: usize,
    pub symbol_count: usize,
    pub file_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImpactAffectedModule {
    pub module: String,
    pub symbol_count: usize,
    pub file_count: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ImpactRiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImpactRiskSummary {
    pub level: ImpactRiskLevel,
    pub direct_hits: usize,
    pub transitive_hits: usize,
    pub impacted_files: usize,
    pub impacted_symbols: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImpactWitnessHop {
    pub from_symbol_id: String,
    pub to_symbol_id: String,
    pub edge: Reference,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImpactWitnessCompactHop {
    pub from_symbol_id: String,
    pub to_symbol_id: String,
    pub edge: Reference,
    #[serde(default)]
    pub collapsed_hops: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ImpactWitnessSliceContext {
    pub seed_symbol_id: String,
    #[serde(default)]
    pub selected_files_on_path: Vec<ImpactWitnessSliceFileContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImpactWitnessSliceFileContext {
    pub path: String,
    #[serde(default)]
    pub witness_hops: Vec<usize>,
    #[serde(default)]
    pub selection_reasons: Vec<ImpactSliceReasonMetadata>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub seed_reasons: Vec<ImpactSliceReasonMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImpactWitness {
    pub symbol_id: String,
    pub depth: usize,
    pub root_symbol_id: String,
    pub via_symbol_id: String,
    pub edge: Reference,
    #[serde(default)]
    pub path: Vec<ImpactWitnessHop>,
    #[serde(default)]
    pub provenance_chain: Vec<EdgeProvenance>,
    #[serde(default)]
    pub kind_chain: Vec<RefKind>,
    #[serde(default)]
    pub path_compact: Vec<ImpactWitnessCompactHop>,
    #[serde(default)]
    pub provenance_chain_compact: Vec<EdgeProvenance>,
    #[serde(default)]
    pub kind_chain_compact: Vec<RefKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slice_context: Option<ImpactWitnessSliceContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImpactOutput {
    pub changed_symbols: Vec<Symbol>,
    pub impacted_symbols: Vec<Symbol>,
    pub impacted_files: Vec<String>,
    pub edges: Vec<Reference>,
    pub impacted_by_file: std::collections::HashMap<String, Vec<Symbol>>, // file -> impacted symbols in that file
    #[serde(default)]
    pub impacted_witnesses: std::collections::HashMap<String, ImpactWitness>,
    #[serde(default)]
    pub summary: ImpactSummary,
}

pub(crate) fn build_by_depth_summary(
    impacted_symbols: &[Symbol],
    min_depth_by_symbol_id: &HashMap<String, usize>,
) -> Vec<ImpactDepthBucket> {
    let mut buckets: std::collections::BTreeMap<usize, (usize, HashSet<String>)> =
        std::collections::BTreeMap::new();
    for sym in impacted_symbols {
        let Some(depth) = min_depth_by_symbol_id.get(&sym.id.0).copied() else {
            continue;
        };
        if depth == 0 {
            continue;
        }
        let (symbol_count, files) = buckets.entry(depth).or_insert_with(|| (0, HashSet::new()));
        *symbol_count += 1;
        files.insert(sym.file.clone());
    }
    buckets
        .into_iter()
        .map(|(depth, (symbol_count, files))| ImpactDepthBucket {
            depth,
            symbol_count,
            file_count: files.len(),
        })
        .collect()
}

pub(crate) fn build_risk_summary(
    by_depth: &[ImpactDepthBucket],
    impacted_files: usize,
    impacted_symbols: usize,
) -> ImpactRiskSummary {
    let direct_hits = by_depth
        .iter()
        .find(|bucket| bucket.depth == 1)
        .map(|bucket| bucket.symbol_count)
        .unwrap_or(0);
    let transitive_hits = by_depth
        .iter()
        .filter(|bucket| bucket.depth >= 2)
        .map(|bucket| bucket.symbol_count)
        .sum();

    let level = if direct_hits >= 3
        || (direct_hits >= 2 && transitive_hits >= 2)
        || (direct_hits >= 1 && transitive_hits >= 3)
        || impacted_files >= 4
        || impacted_symbols >= 8
    {
        ImpactRiskLevel::High
    } else if direct_hits >= 1
        || transitive_hits >= 3
        || impacted_files >= 2
        || impacted_symbols >= 4
    {
        ImpactRiskLevel::Medium
    } else {
        ImpactRiskLevel::Low
    };

    ImpactRiskSummary {
        level,
        direct_hits,
        transitive_hits,
        impacted_files,
        impacted_symbols,
    }
}

fn is_entry_like_module_file(name: &str) -> bool {
    matches!(
        name,
        "main.rs" | "lib.rs" | "mod.rs" | "index.js" | "index.ts" | "index.tsx" | "__init__.py"
    )
}

fn affected_module_for_file(file: &str) -> String {
    let normalized = file.strip_prefix("./").unwrap_or(file).replace('\\', "/");
    let path = std::path::Path::new(&normalized);
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty() && *parent != std::path::Path::new("."));

    if path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(is_entry_like_module_file)
    {
        return parent
            .map(|parent| parent.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|| "(root)".to_string());
    }

    parent
        .map(|parent| parent.to_string_lossy().replace('\\', "/"))
        .unwrap_or(normalized)
}

pub(crate) fn build_affected_modules_summary(
    impacted_symbols: &[Symbol],
) -> Vec<ImpactAffectedModule> {
    let mut modules: HashMap<String, (usize, HashSet<String>)> = HashMap::new();

    for sym in impacted_symbols {
        let module = affected_module_for_file(&sym.file);
        let (symbol_count, files) = modules
            .entry(module)
            .or_insert_with(|| (0usize, HashSet::new()));
        *symbol_count += 1;
        files.insert(sym.file.clone());
    }

    let mut summary: Vec<ImpactAffectedModule> = modules
        .into_iter()
        .map(|(module, (symbol_count, files))| ImpactAffectedModule {
            module,
            symbol_count,
            file_count: files.len(),
        })
        .collect();
    summary.sort_by(|a, b| {
        b.symbol_count
            .cmp(&a.symbol_count)
            .then_with(|| (a.module == "(root)").cmp(&(b.module == "(root)")))
            .then_with(|| a.module.cmp(&b.module))
    });
    summary
}

fn record_min_depth(
    min_depth_by_symbol_id: &mut HashMap<String, usize>,
    symbol_id: &str,
    depth: usize,
) {
    min_depth_by_symbol_id
        .entry(symbol_id.to_string())
        .and_modify(|current| *current = (*current).min(depth))
        .or_insert(depth);
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WitnessCandidate {
    root_symbol_id: String,
    path: Vec<ImpactWitnessHop>,
}

fn edge_provenance_rank(provenance: &EdgeProvenance) -> usize {
    match provenance {
        EdgeProvenance::SymbolicPropagation => 0,
        EdgeProvenance::LocalDfg => 1,
        EdgeProvenance::CallGraph => 2,
    }
}

fn ref_kind_rank(kind: &RefKind) -> usize {
    match kind {
        RefKind::Data => 0,
        RefKind::Call => 1,
        RefKind::Control => 2,
    }
}

fn reference_sort_key(edge: &Reference) -> (usize, usize, &str, u32, &str, &str) {
    (
        edge_provenance_rank(&edge.provenance),
        ref_kind_rank(&edge.kind),
        edge.file.as_str(),
        edge.line,
        edge.from.0.as_str(),
        edge.to.0.as_str(),
    )
}

fn compact_witness_path(path: &[ImpactWitnessHop]) -> Vec<ImpactWitnessCompactHop> {
    let mut compact: Vec<ImpactWitnessCompactHop> = Vec::new();
    for hop in path {
        if let Some(last) = compact.last_mut()
            && last.edge.provenance == hop.edge.provenance
            && last.edge.kind == hop.edge.kind
            && last.edge.file == hop.edge.file
            && last.edge.line == hop.edge.line
        {
            last.to_symbol_id = hop.to_symbol_id.clone();
            last.collapsed_hops += 1;
            continue;
        }
        compact.push(ImpactWitnessCompactHop {
            from_symbol_id: hop.from_symbol_id.clone(),
            to_symbol_id: hop.to_symbol_id.clone(),
            edge: hop.edge.clone(),
            collapsed_hops: 1,
        });
    }
    compact
}

fn witness_candidate_is_better(candidate: &WitnessCandidate, current: &WitnessCandidate) -> bool {
    let candidate_non_call_graph = candidate
        .path
        .iter()
        .filter(|hop| hop.edge.provenance != EdgeProvenance::CallGraph)
        .count();
    let current_non_call_graph = current
        .path
        .iter()
        .filter(|hop| hop.edge.provenance != EdgeProvenance::CallGraph)
        .count();
    if candidate_non_call_graph != current_non_call_graph {
        return candidate_non_call_graph > current_non_call_graph;
    }

    let candidate_compact_len = compact_witness_path(&candidate.path).len();
    let current_compact_len = compact_witness_path(&current.path).len();
    if candidate_compact_len != current_compact_len {
        return candidate_compact_len < current_compact_len;
    }

    let candidate_sig: Vec<_> = candidate
        .path
        .iter()
        .map(|hop| {
            (
                reference_sort_key(&hop.edge),
                hop.from_symbol_id.as_str(),
                hop.to_symbol_id.as_str(),
            )
        })
        .collect();
    let current_sig: Vec<_> = current
        .path
        .iter()
        .map(|hop| {
            (
                reference_sort_key(&hop.edge),
                hop.from_symbol_id.as_str(),
                hop.to_symbol_id.as_str(),
            )
        })
        .collect();
    candidate_sig < current_sig
}

fn update_witness_candidate(
    witnesses: &mut HashMap<String, WitnessCandidate>,
    symbol_id: &str,
    candidate: WitnessCandidate,
) -> bool {
    match witnesses.get(symbol_id) {
        Some(current)
            if current.path.len() < candidate.path.len()
                || (current.path.len() == candidate.path.len()
                    && !witness_candidate_is_better(&candidate, current)) =>
        {
            false
        }
        _ => {
            witnesses.insert(symbol_id.to_string(), candidate);
            true
        }
    }
}

pub(crate) fn finalize_impact_output(
    changed_symbols: Vec<Symbol>,
    mut impacted_symbols: Vec<Symbol>,
    edges: Vec<Reference>,
    min_depth_by_symbol_id: &HashMap<String, usize>,
    impacted_witnesses: std::collections::HashMap<String, ImpactWitness>,
) -> ImpactOutput {
    impacted_symbols.sort_by(|a, b| a.id.0.cmp(&b.id.0));
    impacted_symbols.dedup_by(|a, b| a.id.0 == b.id.0);

    let mut impacted_files: Vec<String> = impacted_symbols.iter().map(|s| s.file.clone()).collect();
    impacted_files.sort();
    impacted_files.dedup();

    let mut impacted_by_file: std::collections::HashMap<String, Vec<Symbol>> =
        std::collections::HashMap::new();
    for s in &impacted_symbols {
        impacted_by_file
            .entry(s.file.clone())
            .or_default()
            .push(s.clone());
    }
    for v in impacted_by_file.values_mut() {
        v.sort_by(|a, b| a.id.0.cmp(&b.id.0));
        v.dedup_by(|a, b| a.id.0 == b.id.0);
    }
    let by_depth = build_by_depth_summary(&impacted_symbols, min_depth_by_symbol_id);
    let affected_modules = build_affected_modules_summary(&impacted_symbols);
    let risk = build_risk_summary(&by_depth, impacted_files.len(), impacted_symbols.len());

    ImpactOutput {
        changed_symbols,
        impacted_symbols,
        impacted_files,
        edges,
        impacted_by_file,
        impacted_witnesses,
        summary: ImpactSummary {
            by_depth,
            affected_modules,
            risk: Some(risk),
            slice_selection: None,
        },
    }
}

fn record_witness_file_hop(
    ordered_paths: &mut Vec<String>,
    witness_hops_by_path: &mut HashMap<String, Vec<usize>>,
    path: &str,
    hop_index: usize,
) {
    let hops = witness_hops_by_path.entry(path.to_string()).or_default();
    if hops.last().copied() != Some(hop_index) {
        hops.push(hop_index);
    }
    if !ordered_paths.iter().any(|existing| existing == path) {
        ordered_paths.push(path.to_string());
    }
}

fn build_witness_slice_context(
    witness: &ImpactWitness,
    symbol_file_by_id: &HashMap<String, String>,
    slice_selection: &ImpactSliceSelectionSummary,
) -> ImpactWitnessSliceContext {
    let selected_files_by_path: HashMap<&str, &ImpactSliceFileMetadata> = slice_selection
        .files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect();

    let mut ordered_paths: Vec<String> = Vec::new();
    let mut witness_hops_by_path: HashMap<String, Vec<usize>> = HashMap::new();
    for (hop_index, hop) in witness.path.iter().enumerate() {
        if let Some(from_file) = symbol_file_by_id.get(&hop.from_symbol_id) {
            record_witness_file_hop(
                &mut ordered_paths,
                &mut witness_hops_by_path,
                from_file,
                hop_index,
            );
        }
        record_witness_file_hop(
            &mut ordered_paths,
            &mut witness_hops_by_path,
            &hop.edge.file,
            hop_index,
        );
        if let Some(to_file) = symbol_file_by_id.get(&hop.to_symbol_id) {
            record_witness_file_hop(
                &mut ordered_paths,
                &mut witness_hops_by_path,
                to_file,
                hop_index,
            );
        }
    }

    let selected_files_on_path = ordered_paths
        .into_iter()
        .filter_map(|path| {
            let metadata = selected_files_by_path.get(path.as_str())?;
            let seed_reasons: Vec<ImpactSliceReasonMetadata> = metadata
                .reasons
                .iter()
                .filter(|reason| reason.seed_symbol_id == witness.root_symbol_id)
                .cloned()
                .collect();
            Some(ImpactWitnessSliceFileContext {
                witness_hops: witness_hops_by_path
                    .remove(path.as_str())
                    .unwrap_or_default(),
                selection_reasons: metadata.reasons.clone(),
                seed_reasons,
                path,
            })
        })
        .collect();

    ImpactWitnessSliceContext {
        seed_symbol_id: witness.root_symbol_id.clone(),
        selected_files_on_path,
    }
}

pub fn attach_slice_selection_summary(
    output: &mut ImpactOutput,
    slice_selection: &ImpactSliceSelectionSummary,
) {
    output.summary.slice_selection = Some(slice_selection.clone());

    if output.impacted_witnesses.is_empty() {
        return;
    }

    let symbol_file_by_id: HashMap<String, String> = output
        .changed_symbols
        .iter()
        .chain(output.impacted_symbols.iter())
        .map(|symbol| (symbol.id.0.clone(), symbol.file.clone()))
        .collect();

    for witness in output.impacted_witnesses.values_mut() {
        witness.slice_context = Some(build_witness_slice_context(
            witness,
            &symbol_file_by_id,
            slice_selection,
        ));
    }
}

/// Return true if `path` is under any of the given `ignore_dirs` prefixes.
/// Matching is done on normalized, relative paths without leading "./".
pub fn path_is_ignored(path: &str, ignore_dirs: &[String]) -> bool {
    if ignore_dirs.is_empty() {
        return false;
    }
    // Normalize path
    let mut p = path.replace('\\', "/");
    if let Some(stripped) = p.strip_prefix("./") {
        p = stripped.to_string();
    }
    for dir in ignore_dirs {
        if dir.is_empty() {
            continue;
        }
        let mut d = dir.replace('\\', "/");
        if let Some(stripped) = d.strip_prefix("./") {
            d = stripped.to_string();
        }
        if d.ends_with('/') {
            d.pop();
        }
        if p == d || p.starts_with(&(d.clone() + "/")) {
            return true;
        }
    }
    false
}

/// Build symbol index and resolved reference edges for the current workspace (cwd).
pub fn build_project_graph() -> anyhow::Result<(SymbolIndex, Vec<Reference>)> {
    let mut symbols = Vec::new();
    let mut urefs = Vec::new();
    let mut file_imports: std::collections::HashMap<
        String,
        std::collections::HashMap<String, String>,
    > = std::collections::HashMap::new();
    for entry in WalkDir::new(".")
        .into_iter()
        .filter_entry(|e| {
            let p = e.path();
            let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
            !(name == ".git" || name == "target" || name.starts_with('.'))
        })
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path.is_file() {
            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
            if ext != "rs"
                && ext != "rb"
                && ext != "js"
                && ext != "ts"
                && ext != "tsx"
                && ext != "py"
            {
                continue;
            }
            let path_str = path
                .strip_prefix("./")
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();
            let Ok(src) = fs::read_to_string(path) else {
                continue;
            };
            let kind = if ext == "rs" {
                LanguageKind::Rust
            } else if ext == "rb" {
                LanguageKind::Ruby
            } else if ext == "js" {
                LanguageKind::Javascript
            } else if ext == "ts" {
                LanguageKind::Typescript
            } else if ext == "tsx" {
                LanguageKind::Tsx
            } else {
                LanguageKind::Python
            };
            let Some(analyzer) = analyzer_for_path(&path_str, kind) else {
                continue;
            };
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
        let Some(from_sym) = index.enclosing_symbol(&r.file, r.line) else {
            continue;
        };
        // Determine candidate name, considering alias from imports
        let imports = file_imports.get(&r.file).cloned().unwrap_or_default();
        let mut target_name = r.name.as_str();
        let qualifier = r.qualifier.as_deref();
        // normalize qualifier using imports (handle alias on the first segment)
        let from_mod = module_path_for_file(&r.file);
        let norm_qual =
            qualifier.and_then(|q| normalize_qualifier_with_imports(q, &imports, &from_mod));
        let qualifier = norm_qual.as_deref().or(qualifier);
        let mut imported_prefix: Option<String> = None;
        let mut glob_prefixes: Vec<String> = imports
            .iter()
            .filter_map(|(k, v)| {
                if k.starts_with("__glob__") {
                    Some(v.clone())
                } else {
                    None
                }
            })
            .collect();
        if qualifier.is_none()
            && let Some(full) = imports.get(&r.name)
        {
            let prior = full.rsplit_once("::").map(|(p, _)| p).unwrap_or("");
            let ip = if prior.contains("self::")
                || prior.contains("super::")
                || prior.contains("crate::")
            {
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
                if !visited.insert(ip.clone()) {
                    break;
                }
                let mut agg_files: Vec<&String> = file_imports
                    .keys()
                    .filter(|f| file_matches_module_path(f, &ip))
                    .collect();
                if agg_files.len() > 1 {
                    agg_files.sort_by_key(|f| {
                        if f.ends_with("/index.js")
                            || f.ends_with("/index.ts")
                            || f.ends_with("/index.tsx")
                        {
                            0
                        } else {
                            1
                        }
                    });
                }
                let Some(agg_path) = agg_files.first() else {
                    break;
                };
                let Some(exp_map) = file_imports.get(*agg_path) else {
                    break;
                };
                for (k, v) in exp_map.iter() {
                    if k.starts_with("__export_glob__") {
                        glob_prefixes.push(v.clone());
                    }
                }
                let key = format!("__export__{}", target_name);
                if let Some(real) = exp_map.get(&key) {
                    ip = real
                        .rsplit_once("::")
                        .map(|(p, _)| p)
                        .unwrap_or("")
                        .to_string();
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
                let v: Vec<_> = cands
                    .iter()
                    .filter(|s| file_matches_module_path(&s.file, q))
                    .collect();
                if v.is_empty() {
                    cands.iter().collect()
                } else {
                    v
                }
            } else {
                cands.iter().collect()
            };
            best = filtered
                .into_iter()
                .filter(|to_sym| {
                    matches!(
                        to_sym.kind,
                        crate::ir::SymbolKind::Function | crate::ir::SymbolKind::Method
                    )
                })
                .max_by(|a, b| {
                    let score_for = |to_sym: &&crate::ir::Symbol| {
                        let mut best = score_candidate(
                            &r.file,
                            qualifier,
                            imported_prefix.as_deref(),
                            to_sym,
                            r.is_method,
                        );
                        for gp in &glob_prefixes {
                            let s = score_candidate(
                                &r.file,
                                qualifier,
                                Some(gp.as_str()),
                                to_sym,
                                r.is_method,
                            );
                            if s > best {
                                best = s;
                            }
                        }
                        best
                    };
                    let sa = score_for(a);
                    let sb = score_for(b);
                    sa.cmp(&sb)
                        // tie-break: prefer earlier declaration line to reduce
                        // overload-related misses (stable + deterministic)
                        .then_with(|| b.range.start_line.cmp(&a.range.start_line))
                        .then_with(|| a.id.0.cmp(&b.id.0))
                });
        }

        // Fallback: no same-name match → choose best symbol within the imported/qualified module
        if best.is_none() {
            let mut module_hints: Vec<String> = Vec::new();
            if let Some(q) = qualifier {
                module_hints.push(q.to_string());
            }
            if let Some(ip) = &imported_prefix
                && !ip.is_empty()
            {
                module_hints.push(ip.clone());
            }
            for gp in &glob_prefixes {
                if !module_hints.contains(gp) {
                    module_hints.push(gp.clone());
                }
            }
            if !module_hints.is_empty() {
                let cands: Vec<&crate::ir::Symbol> = index
                    .symbols
                    .iter()
                    .filter(|s| {
                        matches!(
                            s.kind,
                            crate::ir::SymbolKind::Function | crate::ir::SymbolKind::Method
                        )
                    })
                    .filter(|s| {
                        module_hints
                            .iter()
                            .any(|mp| file_matches_module_path(&s.file, mp))
                    })
                    .collect();
                if !cands.is_empty() {
                    best = cands.into_iter().max_by(|a, b| {
                        let score_for = |to_sym: &&crate::ir::Symbol| {
                            let mut score = score_candidate(
                                &r.file,
                                qualifier,
                                imported_prefix.as_deref(),
                                to_sym,
                                r.is_method,
                            );
                            for gp in &glob_prefixes {
                                let s = score_candidate(
                                    &r.file,
                                    qualifier,
                                    Some(gp.as_str()),
                                    to_sym,
                                    r.is_method,
                                );
                                if s > score {
                                    score = s;
                                }
                            }
                            score
                        };
                        let sa = score_for(a);
                        let sb = score_for(b);
                        sa.cmp(&sb)
                            .then_with(|| b.range.start_line.cmp(&a.range.start_line))
                            .then_with(|| a.id.0.cmp(&b.id.0))
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
                certainty: crate::ir::reference::EdgeCertainty::Inferred,
                provenance: crate::ir::reference::EdgeProvenance::CallGraph,
            });
        }
    }
    out
}

fn function_is_method_compatible(language: &str) -> bool {
    matches!(language, "ruby" | "python")
}

fn score_candidate(
    from_file: &str,
    qualifier: Option<&str>,
    imported_prefix: Option<&str>,
    cand: &crate::ir::Symbol,
    call_is_method: bool,
) -> i32 {
    let mut score = 0;
    if cand.file == from_file {
        score += 30;
    }
    // same directory
    if std::path::Path::new(&cand.file).parent() == std::path::Path::new(from_file).parent() {
        score += 10;
    }
    if let Some(q) = qualifier
        && file_matches_module_path(&cand.file, q)
    {
        score += 20;
    }
    if let Some(ip) = imported_prefix
        && !ip.is_empty()
        && file_matches_module_path(&cand.file, ip)
    {
        score += 15;
    }
    // prefer method symbol if call site looked like a method.
    // Dynamic languages may represent methods as Function in some paths,
    // so keep a language-scoped fallback.
    if call_is_method {
        if matches!(cand.kind, crate::ir::SymbolKind::Method) {
            score += 25;
        } else if matches!(cand.kind, crate::ir::SymbolKind::Function)
            && function_is_method_compatible(&cand.language)
        {
            score += 20;
        }
    } else if matches!(cand.kind, crate::ir::SymbolKind::Function) {
        score += 5;
    }
    score
}

fn file_matches_module_path(file: &str, module_path: &str) -> bool {
    if module_path.is_empty() {
        return false;
    }
    let base = module_path.replace("::", "/");
    let file_norm = if let Ok(s) = std::path::Path::new(file).strip_prefix("./") {
        s.to_string_lossy()
    } else {
        std::borrow::Cow::from(file)
    };
    // Match either <base> with supported extensions (and Rust mod.rs),
    // JS/TS index files, and Python package entry files.
    file_norm.ends_with(&(base.clone() + ".rs"))
        || file_norm.ends_with(&(base.clone() + ".rb"))
        || file_norm.ends_with(&(base.clone() + ".js"))
        || file_norm.ends_with(&(base.clone() + ".ts"))
        || file_norm.ends_with(&(base.clone() + ".tsx"))
        || file_norm.ends_with(&(base.clone() + ".py"))
        || file_norm.ends_with(&(base.clone() + ".go"))
        || file_norm.ends_with(&(base.clone() + ".java"))
        || file_norm.ends_with(&(base.clone() + "/index.js"))
        || file_norm.ends_with(&(base.clone() + "/index.ts"))
        || file_norm.ends_with(&(base.clone() + "/index.tsx"))
        || file_norm.ends_with(&(base.clone() + "/__init__.py"))
        || file_norm.ends_with(&(base + "/mod.rs"))
}

fn normalize_qualifier_with_imports(
    q: &str,
    imports: &std::collections::HashMap<String, String>,
    from_mod: &str,
) -> Option<String> {
    // Support both Ruby/Rust (::) and JS/TS (.) namespace separators
    let q = q.replace('.', "::");
    // apply alias on first segment, then expand self/super/crate relative to from_mod
    let parts: Vec<&str> = q.split("::").collect();
    if parts.is_empty() {
        return None;
    }
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
    if let Ok(stripped) = p.strip_prefix("./") {
        p = stripped;
    }
    let s = p.to_string_lossy();
    if s.ends_with("/mod.rs") || s.ends_with("/lib.rs") || s.ends_with("/main.rs") {
        let dir = p.parent().unwrap_or_else(|| std::path::Path::new(""));
        return dir.to_string_lossy().replace('/', "::");
    }
    if s.ends_with("/index.js") || s.ends_with("/index.ts") || s.ends_with("/index.tsx") {
        let dir = p.parent().unwrap_or_else(|| std::path::Path::new(""));
        return dir.to_string_lossy().replace('/', "::");
    }
    if s.ends_with("/__init__.py") {
        let dir = p.parent().unwrap_or_else(|| std::path::Path::new(""));
        return dir.to_string_lossy().replace('/', "::");
    }
    if s.ends_with(".rs")
        || s.ends_with(".rb")
        || s.ends_with(".js")
        || s.ends_with(".ts")
        || s.ends_with(".tsx")
        || s.ends_with(".py")
        || s.ends_with(".go")
        || s.ends_with(".java")
    {
        let no_ext = s
            .trim_end_matches(".rs")
            .trim_end_matches(".rb")
            .trim_end_matches(".js")
            .trim_end_matches(".ts")
            .trim_end_matches(".tsx")
            .trim_end_matches(".py")
            .trim_end_matches(".go")
            .trim_end_matches(".java");
        return no_ext.replace('/', "::");
    }
    s.replace('/', "::")
}

fn expand_relative_path(current_mod: &str, path: &str) -> String {
    if path.starts_with("crate::") {
        return path.trim_start_matches("crate::").to_string();
    }
    let mut rem = path;
    let mut base: Vec<&str> = current_mod.split("::").filter(|s| !s.is_empty()).collect();
    if rem.starts_with("self::") {
        rem = rem.trim_start_matches("self::");
    }
    while rem.starts_with("super::") {
        if !base.is_empty() {
            base.pop();
        }
        rem = rem.trim_start_matches("super::");
    }
    if rem.is_empty() {
        return base.join("::");
    }
    if base.is_empty() {
        rem.to_string()
    } else {
        format!("{}::{}", base.join("::"), rem)
    }
}

pub fn compute_impact(
    changed: &[Symbol],
    index: &SymbolIndex,
    refs: &[Reference],
    opts: &ImpactOptions,
) -> ImpactOutput {
    let by_id: HashMap<&str, &Symbol> =
        index.symbols.iter().map(|s| (s.id.0.as_str(), s)).collect();

    // Build adjacency maps
    let mut fwd: HashMap<&str, Vec<&Reference>> = HashMap::new(); // from -> [edge]
    let mut rev: HashMap<&str, Vec<&Reference>> = HashMap::new(); // to -> [edge]
    for e in refs {
        let from = e.from.0.as_str();
        let to = e.to.0.as_str();
        fwd.entry(from).or_default().push(e);
        rev.entry(to).or_default().push(e);
    }
    for edges in fwd.values_mut() {
        edges.sort_by_key(|edge| reference_sort_key(edge));
    }
    for edges in rev.values_mut() {
        edges.sort_by_key(|edge| reference_sort_key(edge));
    }

    let changed_ids: HashSet<String> = changed.iter().map(|s| s.id.0.clone()).collect();

    let mut min_depth_by_symbol_id: HashMap<String, usize> = HashMap::new();
    let mut summary_depth_by_symbol_id: HashMap<String, usize> = HashMap::new();
    let mut witness_candidates_by_symbol_id: HashMap<String, WitnessCandidate> = HashMap::new();
    let mut reached_changed_via_callees: HashSet<String> = HashSet::new();
    let mut q: VecDeque<(String, usize)> = VecDeque::new();
    // Seed queue with non-ignored changed symbols
    for s in changed {
        if !path_is_ignored(&s.file, &opts.ignore_dirs) {
            record_min_depth(&mut min_depth_by_symbol_id, &s.id.0, 0);
            q.push_back((s.id.0.clone(), 0));
        }
    }
    while let Some((cur, d)) = q.pop_front() {
        if min_depth_by_symbol_id
            .get(cur.as_str())
            .is_some_and(|best| d > *best)
        {
            continue;
        }
        if let Some(maxd) = opts.max_depth
            && d >= maxd
        {
            continue;
        }

        let current_root = witness_candidates_by_symbol_id
            .get(cur.as_str())
            .map(|candidate| candidate.root_symbol_id.clone())
            .unwrap_or_else(|| cur.clone());
        let current_path = witness_candidates_by_symbol_id
            .get(cur.as_str())
            .map(|candidate| candidate.path.clone())
            .unwrap_or_default();

        let mut consider_edge = |edge: &Reference, next_symbol_id: &str| {
            let next_depth = d + 1;
            record_min_depth(&mut summary_depth_by_symbol_id, next_symbol_id, next_depth);

            let mut candidate_path = current_path.clone();
            candidate_path.push(ImpactWitnessHop {
                from_symbol_id: cur.clone(),
                to_symbol_id: next_symbol_id.to_string(),
                edge: edge.clone(),
            });
            let witness_updated = update_witness_candidate(
                &mut witness_candidates_by_symbol_id,
                next_symbol_id,
                WitnessCandidate {
                    root_symbol_id: current_root.clone(),
                    path: candidate_path,
                },
            );

            if matches!(opts.direction, ImpactDirection::Callees)
                && changed_ids.contains(next_symbol_id)
                && next_symbol_id != cur
            {
                reached_changed_via_callees.insert(next_symbol_id.to_string());
            }

            let should_enqueue = match min_depth_by_symbol_id.get(next_symbol_id).copied() {
                None => {
                    record_min_depth(&mut min_depth_by_symbol_id, next_symbol_id, next_depth);
                    true
                }
                Some(best_depth) if next_depth < best_depth => {
                    record_min_depth(&mut min_depth_by_symbol_id, next_symbol_id, next_depth);
                    true
                }
                Some(best_depth) if next_depth == best_depth && witness_updated => true,
                _ => false,
            };
            if should_enqueue {
                q.push_back((next_symbol_id.to_string(), next_depth));
            }
        };

        match opts.direction {
            ImpactDirection::Callers => {
                if let Some(edges) = rev.get(cur.as_str()) {
                    for edge in edges {
                        consider_edge(edge, edge.from.0.as_str());
                    }
                }
            }
            ImpactDirection::Callees => {
                if let Some(edges) = fwd.get(cur.as_str()) {
                    for edge in edges {
                        consider_edge(edge, edge.to.0.as_str());
                    }
                }
            }
            ImpactDirection::Both => {
                if let Some(edges) = rev.get(cur.as_str()) {
                    for edge in edges {
                        consider_edge(edge, edge.from.0.as_str());
                    }
                }
                if let Some(edges) = fwd.get(cur.as_str()) {
                    for edge in edges {
                        consider_edge(edge, edge.to.0.as_str());
                    }
                }
            }
        }
    }

    let mut impacted_ids: HashSet<String> = min_depth_by_symbol_id
        .keys()
        .filter(|id| !changed_ids.contains(id.as_str()))
        .cloned()
        .collect();
    if matches!(opts.direction, ImpactDirection::Callees) {
        for id in reached_changed_via_callees {
            impacted_ids.insert(id);
        }
    }

    let mut impacted_symbols: Vec<Symbol> = impacted_ids
        .into_iter()
        .filter_map(|id| by_id.get(id.as_str()).cloned().cloned())
        .collect();
    // Filter out symbols located in ignored directories
    if !opts.ignore_dirs.is_empty() {
        impacted_symbols.retain(|s| !path_is_ignored(&s.file, &opts.ignore_dirs));
    }

    let impacted_witnesses: std::collections::HashMap<String, ImpactWitness> = impacted_symbols
        .iter()
        .filter_map(|sym| {
            let candidate = witness_candidates_by_symbol_id.get(&sym.id.0)?;
            let depth = summary_depth_by_symbol_id
                .get(&sym.id.0)
                .copied()
                .unwrap_or(candidate.path.len());
            let edge = candidate.path.last()?.edge.clone();
            let via_symbol_id = candidate
                .path
                .last()
                .map(|hop| hop.from_symbol_id.clone())
                .unwrap_or_else(|| candidate.root_symbol_id.clone());
            let path = candidate.path.clone();
            let provenance_chain = path.iter().map(|hop| hop.edge.provenance.clone()).collect();
            let kind_chain = path.iter().map(|hop| hop.edge.kind.clone()).collect();
            let path_compact = compact_witness_path(&path);
            let provenance_chain_compact = path_compact
                .iter()
                .map(|hop| hop.edge.provenance.clone())
                .collect();
            let kind_chain_compact = path_compact
                .iter()
                .map(|hop| hop.edge.kind.clone())
                .collect();
            Some((
                sym.id.0.clone(),
                ImpactWitness {
                    symbol_id: sym.id.0.clone(),
                    depth,
                    root_symbol_id: candidate.root_symbol_id.clone(),
                    via_symbol_id,
                    edge,
                    path,
                    provenance_chain,
                    kind_chain,
                    path_compact,
                    provenance_chain_compact,
                    kind_chain_compact,
                    slice_context: None,
                },
            ))
        })
        .collect();

    let edges = if opts.with_edges.unwrap_or(false) {
        // Keep the primary relationship graph for changed+impacted nodes.
        // For callees mode we avoid inbound context edges from outside the explored node set
        // to keep oracle comparison stable (e.g. exclude f10->f09 when f10 is outside scope).
        let impacted_id_set: std::collections::HashSet<&str> =
            impacted_symbols.iter().map(|s| s.id.0.as_str()).collect();
        let node_set: std::collections::HashSet<&str> = changed_ids
            .iter()
            .map(String::as_str)
            .chain(impacted_id_set.iter().copied())
            .collect();
        let callsite_locs: std::collections::HashSet<(String, u32)> = refs
            .iter()
            .filter(|e| {
                e.provenance == crate::ir::reference::EdgeProvenance::CallGraph
                    && node_set.contains(e.from.0.as_str())
                    && node_set.contains(e.to.0.as_str())
            })
            .map(|e| (e.file.clone(), e.line))
            .collect();
        refs.iter()
            .filter(|e| {
                let from = e.from.0.as_str();
                let to = e.to.0.as_str();
                let in_scope = if matches!(opts.direction, ImpactDirection::Callees) {
                    node_set.contains(from) && node_set.contains(to)
                } else {
                    node_set.contains(from) || node_set.contains(to)
                };
                if !in_scope {
                    return false;
                }
                if !matches!(opts.direction, ImpactDirection::Callers) {
                    return true;
                }

                let from_is_symbol = node_set.contains(from);
                let to_is_symbol = node_set.contains(to);
                let is_symbol_local_bridge = from_is_symbol ^ to_is_symbol;
                if !is_symbol_local_bridge {
                    return true;
                }
                if !matches!(
                    e.provenance,
                    crate::ir::reference::EdgeProvenance::LocalDfg
                        | crate::ir::reference::EdgeProvenance::SymbolicPropagation
                ) {
                    return true;
                }

                let symbol_id = if from_is_symbol { from } else { to };
                changed_ids.contains(symbol_id) || callsite_locs.contains(&(e.file.clone(), e.line))
            })
            .cloned()
            .collect()
    } else {
        Vec::new()
    };

    finalize_impact_output(
        changed.to_vec(),
        impacted_symbols,
        edges,
        &summary_depth_by_symbol_id,
        impacted_witnesses,
    )
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
        let bar = index
            .symbols
            .iter()
            .find(|s| s.name == "bar")
            .unwrap()
            .clone();
        let out = compute_impact(&[bar], &index, &refs, &ImpactOptions::default());
        std::env::set_current_dir(cwd).unwrap();
        assert!(out.impacted_symbols.iter().any(|s| s.name == "foo"));
    }

    #[test]
    fn compute_impact_records_direct_witness_for_callers() {
        let changed = Symbol {
            id: crate::ir::SymbolId::new(
                "rust",
                "main.rs",
                &crate::ir::SymbolKind::Function,
                "bar",
                1,
            ),
            name: "bar".to_string(),
            kind: crate::ir::SymbolKind::Function,
            file: "main.rs".to_string(),
            range: crate::ir::TextRange {
                start_line: 1,
                end_line: 1,
            },
            language: "rust".to_string(),
        };
        let impacted = Symbol {
            id: crate::ir::SymbolId::new(
                "rust",
                "main.rs",
                &crate::ir::SymbolKind::Function,
                "foo",
                2,
            ),
            name: "foo".to_string(),
            kind: crate::ir::SymbolKind::Function,
            file: "main.rs".to_string(),
            range: crate::ir::TextRange {
                start_line: 2,
                end_line: 2,
            },
            language: "rust".to_string(),
        };
        let index = SymbolIndex::build(vec![changed.clone(), impacted.clone()]);
        let refs = vec![Reference {
            from: impacted.id.clone(),
            to: changed.id.clone(),
            kind: crate::ir::reference::RefKind::Call,
            file: "main.rs".to_string(),
            line: 2,
            certainty: crate::ir::reference::EdgeCertainty::Confirmed,
            provenance: crate::ir::reference::EdgeProvenance::CallGraph,
        }];

        let out = compute_impact(&[changed.clone()], &index, &refs, &ImpactOptions::default());
        let witness = out
            .impacted_witnesses
            .get(&impacted.id.0)
            .expect("witness for foo");
        assert_eq!(witness.symbol_id, impacted.id.0);
        assert_eq!(witness.depth, 1);
        assert_eq!(witness.root_symbol_id, changed.id.0);
        assert_eq!(witness.via_symbol_id, changed.id.0);
        assert_eq!(witness.edge.kind, crate::ir::reference::RefKind::Call);
        assert_eq!(
            witness.edge.provenance,
            crate::ir::reference::EdgeProvenance::CallGraph
        );
        assert_eq!(witness.path.len(), 1);
        assert_eq!(witness.path[0].from_symbol_id, changed.id.0);
        assert_eq!(witness.path[0].to_symbol_id, impacted.id.0);
        assert_eq!(
            witness.provenance_chain,
            vec![crate::ir::reference::EdgeProvenance::CallGraph]
        );
        assert_eq!(
            witness.kind_chain,
            vec![crate::ir::reference::RefKind::Call]
        );
        assert_eq!(witness.path_compact.len(), 1);
        assert_eq!(witness.path_compact[0].collapsed_hops, 1);
        assert_eq!(
            witness.provenance_chain_compact,
            vec![crate::ir::reference::EdgeProvenance::CallGraph]
        );
        assert_eq!(
            witness.kind_chain_compact,
            vec![crate::ir::reference::RefKind::Call]
        );
    }

    #[test]
    fn compute_impact_keeps_symbolic_propagation_in_witness_edge() {
        let changed = Symbol {
            id: crate::ir::SymbolId::new(
                "rust",
                "main.rs",
                &crate::ir::SymbolKind::Function,
                "seed",
                1,
            ),
            name: "seed".to_string(),
            kind: crate::ir::SymbolKind::Function,
            file: "main.rs".to_string(),
            range: crate::ir::TextRange {
                start_line: 1,
                end_line: 1,
            },
            language: "rust".to_string(),
        };
        let mid = Symbol {
            id: crate::ir::SymbolId::new(
                "rust",
                "main.rs",
                &crate::ir::SymbolKind::Function,
                "mid",
                2,
            ),
            name: "mid".to_string(),
            kind: crate::ir::SymbolKind::Function,
            file: "main.rs".to_string(),
            range: crate::ir::TextRange {
                start_line: 2,
                end_line: 2,
            },
            language: "rust".to_string(),
        };
        let target = Symbol {
            id: crate::ir::SymbolId::new(
                "rust",
                "main.rs",
                &crate::ir::SymbolKind::Function,
                "target",
                3,
            ),
            name: "target".to_string(),
            kind: crate::ir::SymbolKind::Function,
            file: "main.rs".to_string(),
            range: crate::ir::TextRange {
                start_line: 3,
                end_line: 3,
            },
            language: "rust".to_string(),
        };
        let index = SymbolIndex::build(vec![changed.clone(), mid.clone(), target.clone()]);
        let refs = vec![
            Reference {
                from: changed.id.clone(),
                to: mid.id.clone(),
                kind: crate::ir::reference::RefKind::Call,
                file: "main.rs".to_string(),
                line: 2,
                certainty: crate::ir::reference::EdgeCertainty::Confirmed,
                provenance: crate::ir::reference::EdgeProvenance::CallGraph,
            },
            Reference {
                from: mid.id.clone(),
                to: target.id.clone(),
                kind: crate::ir::reference::RefKind::Data,
                file: "main.rs".to_string(),
                line: 3,
                certainty: crate::ir::reference::EdgeCertainty::Inferred,
                provenance: crate::ir::reference::EdgeProvenance::SymbolicPropagation,
            },
        ];
        let opts = ImpactOptions {
            direction: ImpactDirection::Callees,
            max_depth: Some(10),
            with_edges: Some(true),
            ignore_dirs: Vec::new(),
        };

        let out = compute_impact(&[changed.clone()], &index, &refs, &opts);
        let witness = out
            .impacted_witnesses
            .get(&target.id.0)
            .expect("witness for target");
        assert_eq!(witness.depth, 2);
        assert_eq!(witness.root_symbol_id, changed.id.0);
        assert_eq!(witness.via_symbol_id, mid.id.0);
        assert_eq!(witness.edge.kind, crate::ir::reference::RefKind::Data);
        assert_eq!(
            witness.edge.provenance,
            crate::ir::reference::EdgeProvenance::SymbolicPropagation
        );
        assert_eq!(witness.edge.line, 3);
        assert_eq!(witness.edge.file, "main.rs");
        assert_eq!(witness.path.len(), 2);
        assert_eq!(witness.path[0].from_symbol_id, changed.id.0);
        assert_eq!(witness.path[0].to_symbol_id, mid.id.0);
        assert_eq!(witness.path[1].from_symbol_id, mid.id.0);
        assert_eq!(witness.path[1].to_symbol_id, target.id.0);
        assert_eq!(
            witness.provenance_chain,
            vec![
                crate::ir::reference::EdgeProvenance::CallGraph,
                crate::ir::reference::EdgeProvenance::SymbolicPropagation,
            ]
        );
        assert_eq!(
            witness.kind_chain,
            vec![
                crate::ir::reference::RefKind::Call,
                crate::ir::reference::RefKind::Data,
            ]
        );
        assert_eq!(witness.path_compact.len(), 2);
        assert_eq!(
            witness.provenance_chain_compact,
            vec![
                crate::ir::reference::EdgeProvenance::CallGraph,
                crate::ir::reference::EdgeProvenance::SymbolicPropagation,
            ]
        );
        assert_eq!(
            witness.kind_chain_compact,
            vec![
                crate::ir::reference::RefKind::Call,
                crate::ir::reference::RefKind::Data,
            ]
        );
    }

    #[test]
    fn compute_impact_prefers_more_explainable_equal_depth_witness_path() {
        let changed = Symbol {
            id: crate::ir::SymbolId::new(
                "rust",
                "main.rs",
                &crate::ir::SymbolKind::Function,
                "seed",
                1,
            ),
            name: "seed".to_string(),
            kind: crate::ir::SymbolKind::Function,
            file: "main.rs".to_string(),
            range: crate::ir::TextRange {
                start_line: 1,
                end_line: 1,
            },
            language: "rust".to_string(),
        };
        let plain_mid = Symbol {
            id: crate::ir::SymbolId::new(
                "rust",
                "main.rs",
                &crate::ir::SymbolKind::Function,
                "plain_mid",
                2,
            ),
            name: "plain_mid".to_string(),
            kind: crate::ir::SymbolKind::Function,
            file: "main.rs".to_string(),
            range: crate::ir::TextRange {
                start_line: 2,
                end_line: 2,
            },
            language: "rust".to_string(),
        };
        let rich_mid = Symbol {
            id: crate::ir::SymbolId::new(
                "rust",
                "main.rs",
                &crate::ir::SymbolKind::Function,
                "rich_mid",
                3,
            ),
            name: "rich_mid".to_string(),
            kind: crate::ir::SymbolKind::Function,
            file: "main.rs".to_string(),
            range: crate::ir::TextRange {
                start_line: 3,
                end_line: 3,
            },
            language: "rust".to_string(),
        };
        let target = Symbol {
            id: crate::ir::SymbolId::new(
                "rust",
                "main.rs",
                &crate::ir::SymbolKind::Function,
                "target",
                4,
            ),
            name: "target".to_string(),
            kind: crate::ir::SymbolKind::Function,
            file: "main.rs".to_string(),
            range: crate::ir::TextRange {
                start_line: 4,
                end_line: 4,
            },
            language: "rust".to_string(),
        };
        let index = SymbolIndex::build(vec![
            changed.clone(),
            plain_mid.clone(),
            rich_mid.clone(),
            target.clone(),
        ]);
        let refs = vec![
            Reference {
                from: changed.id.clone(),
                to: plain_mid.id.clone(),
                kind: crate::ir::reference::RefKind::Call,
                file: "main.rs".to_string(),
                line: 10,
                certainty: crate::ir::reference::EdgeCertainty::Confirmed,
                provenance: crate::ir::reference::EdgeProvenance::CallGraph,
            },
            Reference {
                from: plain_mid.id.clone(),
                to: target.id.clone(),
                kind: crate::ir::reference::RefKind::Call,
                file: "main.rs".to_string(),
                line: 11,
                certainty: crate::ir::reference::EdgeCertainty::Confirmed,
                provenance: crate::ir::reference::EdgeProvenance::CallGraph,
            },
            Reference {
                from: changed.id.clone(),
                to: rich_mid.id.clone(),
                kind: crate::ir::reference::RefKind::Call,
                file: "main.rs".to_string(),
                line: 12,
                certainty: crate::ir::reference::EdgeCertainty::Confirmed,
                provenance: crate::ir::reference::EdgeProvenance::CallGraph,
            },
            Reference {
                from: rich_mid.id.clone(),
                to: target.id.clone(),
                kind: crate::ir::reference::RefKind::Data,
                file: "main.rs".to_string(),
                line: 13,
                certainty: crate::ir::reference::EdgeCertainty::Inferred,
                provenance: crate::ir::reference::EdgeProvenance::SymbolicPropagation,
            },
        ];
        let opts = ImpactOptions {
            direction: ImpactDirection::Callees,
            max_depth: Some(10),
            with_edges: Some(true),
            ignore_dirs: Vec::new(),
        };

        let out = compute_impact(&[changed], &index, &refs, &opts);
        let witness = out
            .impacted_witnesses
            .get(&target.id.0)
            .expect("witness for target");
        assert_eq!(witness.depth, 2);
        assert_eq!(witness.via_symbol_id, rich_mid.id.0);
        assert_eq!(
            witness.provenance_chain,
            vec![
                crate::ir::reference::EdgeProvenance::CallGraph,
                crate::ir::reference::EdgeProvenance::SymbolicPropagation,
            ]
        );
    }

    #[test]
    fn compute_impact_compacts_redundant_witness_hops() {
        let changed = Symbol {
            id: crate::ir::SymbolId::new(
                "rust",
                "main.rs",
                &crate::ir::SymbolKind::Function,
                "seed",
                1,
            ),
            name: "seed".to_string(),
            kind: crate::ir::SymbolKind::Function,
            file: "main.rs".to_string(),
            range: crate::ir::TextRange {
                start_line: 1,
                end_line: 1,
            },
            language: "rust".to_string(),
        };
        let target = Symbol {
            id: crate::ir::SymbolId::new(
                "rust",
                "main.rs",
                &crate::ir::SymbolKind::Function,
                "target",
                4,
            ),
            name: "target".to_string(),
            kind: crate::ir::SymbolKind::Function,
            file: "main.rs".to_string(),
            range: crate::ir::TextRange {
                start_line: 4,
                end_line: 4,
            },
            language: "rust".to_string(),
        };
        let tmp1 = crate::ir::SymbolId("main.rs:def:a:2".to_string());
        let tmp2 = crate::ir::SymbolId("main.rs:use:a:2".to_string());
        let index = SymbolIndex::build(vec![changed.clone(), target.clone()]);
        let refs = vec![
            Reference {
                from: changed.id.clone(),
                to: tmp1.clone(),
                kind: crate::ir::reference::RefKind::Data,
                file: "main.rs".to_string(),
                line: 22,
                certainty: crate::ir::reference::EdgeCertainty::Confirmed,
                provenance: crate::ir::reference::EdgeProvenance::LocalDfg,
            },
            Reference {
                from: tmp1.clone(),
                to: tmp2.clone(),
                kind: crate::ir::reference::RefKind::Data,
                file: "main.rs".to_string(),
                line: 22,
                certainty: crate::ir::reference::EdgeCertainty::Confirmed,
                provenance: crate::ir::reference::EdgeProvenance::LocalDfg,
            },
            Reference {
                from: tmp2,
                to: target.id.clone(),
                kind: crate::ir::reference::RefKind::Data,
                file: "main.rs".to_string(),
                line: 22,
                certainty: crate::ir::reference::EdgeCertainty::Confirmed,
                provenance: crate::ir::reference::EdgeProvenance::LocalDfg,
            },
        ];
        let opts = ImpactOptions {
            direction: ImpactDirection::Callees,
            max_depth: Some(10),
            with_edges: Some(true),
            ignore_dirs: Vec::new(),
        };

        let out = compute_impact(&[changed], &index, &refs, &opts);
        let witness = out
            .impacted_witnesses
            .get(&target.id.0)
            .expect("witness for target");
        assert_eq!(witness.path.len(), 3);
        assert_eq!(witness.path_compact.len(), 1);
        assert_eq!(
            witness.path_compact[0].from_symbol_id,
            witness.root_symbol_id
        );
        assert_eq!(witness.path_compact[0].to_symbol_id, target.id.0);
        assert_eq!(witness.path_compact[0].collapsed_hops, 3);
        assert_eq!(
            witness.provenance_chain_compact,
            vec![crate::ir::reference::EdgeProvenance::LocalDfg]
        );
        assert_eq!(
            witness.kind_chain_compact,
            vec![crate::ir::reference::RefKind::Data]
        );
    }

    #[test]
    fn attach_slice_selection_summary_links_witness_files_to_selected_slice_files() {
        let seed = Symbol {
            id: crate::ir::SymbolId::new(
                "rust",
                "main.rs",
                &crate::ir::SymbolKind::Function,
                "caller",
                1,
            ),
            name: "caller".to_string(),
            kind: crate::ir::SymbolKind::Function,
            file: "main.rs".to_string(),
            range: crate::ir::TextRange {
                start_line: 1,
                end_line: 1,
            },
            language: "rust".to_string(),
        };
        let mid = Symbol {
            id: crate::ir::SymbolId::new(
                "rust",
                "wrapper.rs",
                &crate::ir::SymbolKind::Function,
                "wrap",
                2,
            ),
            name: "wrap".to_string(),
            kind: crate::ir::SymbolKind::Function,
            file: "wrapper.rs".to_string(),
            range: crate::ir::TextRange {
                start_line: 2,
                end_line: 2,
            },
            language: "rust".to_string(),
        };
        let target = Symbol {
            id: crate::ir::SymbolId::new(
                "rust",
                "callee.rs",
                &crate::ir::SymbolKind::Function,
                "callee",
                3,
            ),
            name: "callee".to_string(),
            kind: crate::ir::SymbolKind::Function,
            file: "callee.rs".to_string(),
            range: crate::ir::TextRange {
                start_line: 3,
                end_line: 3,
            },
            language: "rust".to_string(),
        };
        let index = SymbolIndex::build(vec![seed.clone(), mid.clone(), target.clone()]);
        let refs = vec![
            Reference {
                from: seed.id.clone(),
                to: mid.id.clone(),
                kind: crate::ir::reference::RefKind::Call,
                file: "main.rs".to_string(),
                line: 10,
                certainty: crate::ir::reference::EdgeCertainty::Confirmed,
                provenance: crate::ir::reference::EdgeProvenance::CallGraph,
            },
            Reference {
                from: mid.id.clone(),
                to: target.id.clone(),
                kind: crate::ir::reference::RefKind::Call,
                file: "wrapper.rs".to_string(),
                line: 20,
                certainty: crate::ir::reference::EdgeCertainty::Confirmed,
                provenance: crate::ir::reference::EdgeProvenance::CallGraph,
            },
        ];
        let opts = ImpactOptions {
            direction: ImpactDirection::Callees,
            max_depth: Some(10),
            with_edges: Some(true),
            ignore_dirs: Vec::new(),
        };
        let mut out = compute_impact(&[seed.clone()], &index, &refs, &opts);

        let wrapper_seed_reason = ImpactSliceReasonMetadata {
            seed_symbol_id: seed.id.0.clone(),
            tier: 2,
            kind: ImpactSliceReasonKind::BridgeCompletionFile,
            via_symbol_id: Some(mid.id.0.clone()),
            via_path: None,
            bridge_kind: Some(ImpactSliceBridgeKind::WrapperReturn),
        };
        let wrapper_other_reason = ImpactSliceReasonMetadata {
            seed_symbol_id: "rust:other.rs:fn:other:1".to_string(),
            tier: 1,
            kind: ImpactSliceReasonKind::DirectCalleeFile,
            via_symbol_id: Some(mid.id.0.clone()),
            via_path: None,
            bridge_kind: None,
        };
        let slice_selection = ImpactSliceSelectionSummary {
            planner: ImpactSlicePlannerKind::BoundedSlice,
            files: vec![
                ImpactSliceFileMetadata {
                    path: "main.rs".to_string(),
                    scopes: ImpactSliceScopes {
                        cache_update: true,
                        local_dfg: true,
                        explanation: true,
                    },
                    reasons: vec![ImpactSliceReasonMetadata {
                        seed_symbol_id: seed.id.0.clone(),
                        tier: 0,
                        kind: ImpactSliceReasonKind::ChangedFile,
                        via_symbol_id: None,
                        via_path: None,
                        bridge_kind: None,
                    }],
                },
                ImpactSliceFileMetadata {
                    path: "wrapper.rs".to_string(),
                    scopes: ImpactSliceScopes {
                        cache_update: true,
                        local_dfg: true,
                        explanation: true,
                    },
                    reasons: vec![wrapper_seed_reason.clone(), wrapper_other_reason.clone()],
                },
                ImpactSliceFileMetadata {
                    path: "callee.rs".to_string(),
                    scopes: ImpactSliceScopes {
                        cache_update: true,
                        local_dfg: true,
                        explanation: true,
                    },
                    reasons: vec![ImpactSliceReasonMetadata {
                        seed_symbol_id: seed.id.0.clone(),
                        tier: 1,
                        kind: ImpactSliceReasonKind::DirectCalleeFile,
                        via_symbol_id: Some(target.id.0.clone()),
                        via_path: None,
                        bridge_kind: None,
                    }],
                },
                ImpactSliceFileMetadata {
                    path: "unused.rs".to_string(),
                    scopes: ImpactSliceScopes {
                        cache_update: false,
                        local_dfg: false,
                        explanation: true,
                    },
                    reasons: vec![ImpactSliceReasonMetadata {
                        seed_symbol_id: seed.id.0.clone(),
                        tier: 3,
                        kind: ImpactSliceReasonKind::ModuleCompanionFile,
                        via_symbol_id: None,
                        via_path: Some("main.rs".to_string()),
                        bridge_kind: None,
                    }],
                },
            ],
            pruned_candidates: Vec::new(),
        };

        attach_slice_selection_summary(&mut out, &slice_selection);

        assert_eq!(out.summary.slice_selection, Some(slice_selection.clone()));
        let witness = out
            .impacted_witnesses
            .get(&target.id.0)
            .expect("witness for callee");
        assert_eq!(
            witness.slice_context,
            Some(ImpactWitnessSliceContext {
                seed_symbol_id: seed.id.0.clone(),
                selected_files_on_path: vec![
                    ImpactWitnessSliceFileContext {
                        path: "main.rs".to_string(),
                        witness_hops: vec![0],
                        selection_reasons: vec![ImpactSliceReasonMetadata {
                            seed_symbol_id: seed.id.0.clone(),
                            tier: 0,
                            kind: ImpactSliceReasonKind::ChangedFile,
                            via_symbol_id: None,
                            via_path: None,
                            bridge_kind: None,
                        }],
                        seed_reasons: vec![ImpactSliceReasonMetadata {
                            seed_symbol_id: seed.id.0.clone(),
                            tier: 0,
                            kind: ImpactSliceReasonKind::ChangedFile,
                            via_symbol_id: None,
                            via_path: None,
                            bridge_kind: None,
                        }],
                    },
                    ImpactWitnessSliceFileContext {
                        path: "wrapper.rs".to_string(),
                        witness_hops: vec![0, 1],
                        selection_reasons: vec![wrapper_seed_reason, wrapper_other_reason,],
                        seed_reasons: vec![ImpactSliceReasonMetadata {
                            seed_symbol_id: seed.id.0.clone(),
                            tier: 2,
                            kind: ImpactSliceReasonKind::BridgeCompletionFile,
                            via_symbol_id: Some(mid.id.0.clone()),
                            via_path: None,
                            bridge_kind: Some(ImpactSliceBridgeKind::WrapperReturn),
                        }],
                    },
                    ImpactWitnessSliceFileContext {
                        path: "callee.rs".to_string(),
                        witness_hops: vec![1],
                        selection_reasons: vec![ImpactSliceReasonMetadata {
                            seed_symbol_id: seed.id.0.clone(),
                            tier: 1,
                            kind: ImpactSliceReasonKind::DirectCalleeFile,
                            via_symbol_id: Some(target.id.0.clone()),
                            via_path: None,
                            bridge_kind: None,
                        }],
                        seed_reasons: vec![ImpactSliceReasonMetadata {
                            seed_symbol_id: seed.id.0.clone(),
                            tier: 1,
                            kind: ImpactSliceReasonKind::DirectCalleeFile,
                            via_symbol_id: Some(target.id.0.clone()),
                            via_path: None,
                            bridge_kind: None,
                        }],
                    },
                ],
            })
        );
    }

    #[test]
    fn method_compatibility_accepts_python_and_ruby_function_symbols() {
        assert!(function_is_method_compatible("ruby"));
        assert!(function_is_method_compatible("python"));
        assert!(!function_is_method_compatible("rust"));
    }

    #[test]
    fn score_candidate_prefers_method_but_allows_python_function_fallback() {
        let method = Symbol {
            id: crate::ir::SymbolId::new(
                "python",
                "pkg/a.py",
                &crate::ir::SymbolKind::Method,
                "m",
                1,
            ),
            name: "m".to_string(),
            kind: crate::ir::SymbolKind::Method,
            file: "pkg/a.py".to_string(),
            range: crate::ir::TextRange {
                start_line: 1,
                end_line: 1,
            },
            language: "python".to_string(),
        };
        let py_fn = Symbol {
            id: crate::ir::SymbolId::new(
                "python",
                "pkg/a.py",
                &crate::ir::SymbolKind::Function,
                "m",
                1,
            ),
            name: "m".to_string(),
            kind: crate::ir::SymbolKind::Function,
            file: "pkg/a.py".to_string(),
            range: crate::ir::TextRange {
                start_line: 1,
                end_line: 1,
            },
            language: "python".to_string(),
        };
        let rust_fn = Symbol {
            id: crate::ir::SymbolId::new(
                "rust",
                "src/lib.rs",
                &crate::ir::SymbolKind::Function,
                "m",
                1,
            ),
            name: "m".to_string(),
            kind: crate::ir::SymbolKind::Function,
            file: "src/lib.rs".to_string(),
            range: crate::ir::TextRange {
                start_line: 1,
                end_line: 1,
            },
            language: "rust".to_string(),
        };

        let m_score = score_candidate("pkg/b.py", None, None, &method, true);
        let py_fn_score = score_candidate("pkg/b.py", None, None, &py_fn, true);
        let rust_fn_score = score_candidate("pkg/b.py", None, None, &rust_fn, true);

        assert!(m_score > py_fn_score, "method should score highest");
        assert!(
            py_fn_score > rust_fn_score,
            "python function fallback should be preferred"
        );
    }

    #[test]
    fn file_matches_module_path_supports_java_files() {
        assert!(file_matches_module_path(
            "src/main/java/demo/Outer.java",
            "demo::Outer"
        ));
        assert!(file_matches_module_path("demo/Ops.java", "demo::Ops"));
    }

    #[test]
    fn module_path_for_file_strips_go_and_java_extensions() {
        assert_eq!(
            module_path_for_file("pkg/service/main.go"),
            "pkg::service::main"
        );
        assert_eq!(module_path_for_file("demo/Ops.java"), "demo::Ops");
    }

    #[test]
    fn affected_module_for_file_normalizes_entry_like_labels() {
        assert_eq!(affected_module_for_file("main.rs"), "(root)");
        assert_eq!(affected_module_for_file("src/main.rs"), "src");
        assert_eq!(affected_module_for_file("src/lib.rs"), "src");
        assert_eq!(affected_module_for_file("src/engine/mod.rs"), "src/engine");
        assert_eq!(affected_module_for_file("web/index.ts"), "web");
        assert_eq!(affected_module_for_file("pkg/__init__.py"), "pkg");
        assert_eq!(affected_module_for_file("foo.rs"), "foo.rs");
    }

    #[test]
    fn build_affected_modules_summary_keeps_root_after_named_dirs_on_tie() {
        let make_symbol = |file: &str, name: &str, line: u32| Symbol {
            id: crate::ir::SymbolId::new(
                "rust",
                file,
                &crate::ir::SymbolKind::Function,
                name,
                line,
            ),
            name: name.to_string(),
            kind: crate::ir::SymbolKind::Function,
            file: file.to_string(),
            range: crate::ir::TextRange {
                start_line: line,
                end_line: line,
            },
            language: "rust".to_string(),
        };

        let summary = build_affected_modules_summary(&[
            make_symbol("main.rs", "main", 1),
            make_symbol("main.rs", "root_one", 2),
            make_symbol("alpha/first.rs", "alpha_one", 3),
            make_symbol("alpha/second.rs", "alpha_two", 4),
            make_symbol("beta/first.rs", "beta_one", 5),
        ]);

        let observed: Vec<(String, usize, usize)> = summary
            .into_iter()
            .map(|item| (item.module, item.symbol_count, item.file_count))
            .collect();
        assert_eq!(
            observed,
            vec![
                ("alpha".to_string(), 2, 2),
                ("(root)".to_string(), 2, 1),
                ("beta".to_string(), 1, 1),
            ]
        );
    }

    #[test]
    fn build_risk_summary_marks_small_transitive_only_impact_low() {
        let risk = build_risk_summary(
            &[ImpactDepthBucket {
                depth: 2,
                symbol_count: 1,
                file_count: 1,
            }],
            1,
            1,
        );
        assert_eq!(risk.level, ImpactRiskLevel::Low);
        assert_eq!(risk.direct_hits, 0);
        assert_eq!(risk.transitive_hits, 1);
    }

    #[test]
    fn build_risk_summary_marks_direct_impact_medium() {
        let risk = build_risk_summary(
            &[
                ImpactDepthBucket {
                    depth: 1,
                    symbol_count: 1,
                    file_count: 1,
                },
                ImpactDepthBucket {
                    depth: 2,
                    symbol_count: 1,
                    file_count: 1,
                },
            ],
            1,
            2,
        );
        assert_eq!(risk.level, ImpactRiskLevel::Medium);
        assert_eq!(risk.direct_hits, 1);
        assert_eq!(risk.transitive_hits, 1);
    }

    #[test]
    fn build_risk_summary_marks_direct_plus_three_transitive_impact_high() {
        let risk = build_risk_summary(
            &[
                ImpactDepthBucket {
                    depth: 1,
                    symbol_count: 1,
                    file_count: 1,
                },
                ImpactDepthBucket {
                    depth: 2,
                    symbol_count: 3,
                    file_count: 1,
                },
            ],
            1,
            4,
        );
        assert_eq!(risk.level, ImpactRiskLevel::High);
        assert_eq!(risk.direct_hits, 1);
        assert_eq!(risk.transitive_hits, 3);
    }

    #[test]
    fn build_risk_summary_marks_large_direct_plus_transitive_impact_high() {
        let risk = build_risk_summary(
            &[
                ImpactDepthBucket {
                    depth: 1,
                    symbol_count: 2,
                    file_count: 2,
                },
                ImpactDepthBucket {
                    depth: 2,
                    symbol_count: 2,
                    file_count: 1,
                },
            ],
            3,
            4,
        );
        assert_eq!(risk.level, ImpactRiskLevel::High);
        assert_eq!(risk.direct_hits, 2);
        assert_eq!(risk.transitive_hits, 2);
    }
}
