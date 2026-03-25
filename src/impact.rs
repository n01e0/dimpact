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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scoring: Option<ImpactSliceCandidateScoringSummary>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ImpactSliceReasonKind {
    SeedFile,
    ChangedFile,
    DirectCallerFile,
    DirectCalleeFile,
    BridgeCompletionFile,
    BridgeContinuationFile,
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
pub struct ImpactSliceCandidateScoringSummary {
    pub source_kind: ImpactSliceCandidateSourceKind,
    pub lane: ImpactSliceCandidateLane,
    pub primary_evidence_kinds: Vec<ImpactSliceEvidenceKind>,
    pub secondary_evidence_kinds: Vec<ImpactSliceEvidenceKind>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub negative_evidence_kinds: Vec<ImpactSliceNegativeEvidenceKind>,
    pub score_tuple: ImpactSliceScoreTuple,
    #[serde(
        default,
        skip_serializing_if = "impact_slice_candidate_support_is_absent"
    )]
    pub support: Option<ImpactSliceCandidateSupportMetadata>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ImpactSliceCandidateSourceKind {
    GraphSecondHop,
    NarrowFallback,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ImpactSliceCandidateLane {
    ReturnContinuation,
    AliasContinuation,
    RequireRelativeContinuation,
    ModuleCompanionFallback,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ImpactSliceEvidenceKind {
    ReturnFlow,
    AssignedResult,
    AliasChain,
    ParamToReturnFlow,
    RequireRelativeEdge,
    ExplicitRequireRelativeLoad,
    ModuleCompanion,
    CompanionFileMatch,
    DynamicDispatchLiteralTarget,
    CallsitePositionHint,
    NamePathHint,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ImpactSliceNegativeEvidenceKind {
    NoisyReturnHint,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ImpactSliceCandidateSupportMetadata {
    #[serde(default, skip_serializing_if = "impact_slice_bool_is_false")]
    pub call_graph_support: bool,
    #[serde(default, skip_serializing_if = "impact_slice_bool_is_false")]
    pub local_dfg_support: bool,
    #[serde(default, skip_serializing_if = "impact_slice_bool_is_false")]
    pub symbolic_propagation_support: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge_certainty: Option<ImpactSliceSupportEdgeCertainty>,
}

impl ImpactSliceCandidateSupportMetadata {
    fn is_empty(&self) -> bool {
        !self.call_graph_support
            && !self.local_dfg_support
            && !self.symbolic_propagation_support
            && self.edge_certainty.is_none()
    }
}

fn impact_slice_bool_is_false(value: &bool) -> bool {
    !*value
}

fn impact_slice_u8_is_zero(value: &u8) -> bool {
    *value == 0
}

fn impact_slice_candidate_support_is_absent(
    support: &Option<ImpactSliceCandidateSupportMetadata>,
) -> bool {
    match support {
        None => true,
        Some(support) => support.is_empty(),
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ImpactSliceSupportEdgeCertainty {
    Confirmed,
    Inferred,
    DynamicFallback,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ImpactSliceScoreTuple {
    pub source_rank: u8,
    pub lane_rank: u8,
    pub primary_evidence_count: u8,
    pub secondary_evidence_count: u8,
    #[serde(default, skip_serializing_if = "impact_slice_u8_is_zero")]
    pub negative_evidence_count: u8,
    #[serde(default, skip_serializing_if = "impact_slice_u8_is_zero")]
    pub semantic_support_rank: u8,
    pub call_position_rank: u32,
    pub lexical_tiebreak: String,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scoring: Option<ImpactSliceCandidateScoringSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compact_explanation: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ImpactSlicePruneReason {
    AlreadySelected,
    BridgeBudgetExhausted,
    CacheUpdateBudgetExhausted,
    LocalDfgBudgetExhausted,
    SuppressedBeforeAdmit,
    WeakerSamePathDuplicate,
    WeakerSameFamilySibling,
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ImpactWitnessSliceRankingBasis {
    SourceKind,
    Lane,
    PrimaryEvidenceCount,
    NegativeEvidenceCount,
    SemanticSupportRank,
    SecondaryEvidenceCount,
    CallsitePosition,
    LexicalTiebreak,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImpactWitnessSliceSelectedVsPrunedReason {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub via_symbol_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub via_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_bridge_kind: Option<ImpactSliceBridgeKind>,
    pub pruned_path: String,
    pub prune_reason: ImpactSlicePruneReason,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pruned_bridge_kind: Option<ImpactSliceBridgeKind>,
    pub selected_better_by: ImpactWitnessSliceRankingBasis,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub winning_primary_evidence_kinds: Option<Vec<ImpactSliceEvidenceKind>>,
    #[serde(
        default,
        skip_serializing_if = "impact_slice_candidate_support_is_absent"
    )]
    pub winning_support: Option<ImpactSliceCandidateSupportMetadata>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub losing_side_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compact_explanation: Option<String>,
    pub summary: String,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selected_vs_pruned_reasons: Vec<ImpactWitnessSliceSelectedVsPrunedReason>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ImpactBridgeExecutionFamily {
    ReturnContinuation,
    AliasResultStitch,
    RequireRelativeContinuation,
    MixedRequireRelativeAliasStitch,
    NestedMultiInputContinuation,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ImpactBridgeExecutionStepFamily {
    CallsiteInputBinding,
    SummaryReturnBridge,
    NestedSummaryBridge,
    AliasResultStitch,
    RequireRelativeLoad,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImpactBridgeExecutionStepCompact {
    pub family: ImpactBridgeExecutionFamily,
    pub step_family: ImpactBridgeExecutionStepFamily,
    pub anchor_symbol_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge_kind: Option<ImpactSliceBridgeKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason_kind: Option<ImpactSliceReasonKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
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
    pub bridge_execution_family: Option<ImpactBridgeExecutionFamily>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bridge_execution_chain_compact: Vec<ImpactBridgeExecutionStepCompact>,
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

fn witness_source_kind_label(kind: ImpactSliceCandidateSourceKind) -> &'static str {
    match kind {
        ImpactSliceCandidateSourceKind::GraphSecondHop => "graph_second_hop",
        ImpactSliceCandidateSourceKind::NarrowFallback => "narrow_fallback",
    }
}

fn witness_lane_label(lane: ImpactSliceCandidateLane) -> &'static str {
    match lane {
        ImpactSliceCandidateLane::ReturnContinuation => "return_continuation",
        ImpactSliceCandidateLane::AliasContinuation => "alias_continuation",
        ImpactSliceCandidateLane::RequireRelativeContinuation => "require_relative_continuation",
        ImpactSliceCandidateLane::ModuleCompanionFallback => "module_companion_fallback",
    }
}

fn witness_evidence_kind_label(kind: ImpactSliceEvidenceKind) -> &'static str {
    match kind {
        ImpactSliceEvidenceKind::ReturnFlow => "return_flow",
        ImpactSliceEvidenceKind::AssignedResult => "assigned_result",
        ImpactSliceEvidenceKind::AliasChain => "alias_chain",
        ImpactSliceEvidenceKind::ParamToReturnFlow => "param_to_return_flow",
        ImpactSliceEvidenceKind::RequireRelativeEdge => "require_relative_edge",
        ImpactSliceEvidenceKind::ExplicitRequireRelativeLoad => "explicit_require_relative_load",
        ImpactSliceEvidenceKind::ModuleCompanion => "module_companion",
        ImpactSliceEvidenceKind::CompanionFileMatch => "companion_file_match",
        ImpactSliceEvidenceKind::DynamicDispatchLiteralTarget => "dynamic_dispatch_literal_target",
        ImpactSliceEvidenceKind::CallsitePositionHint => "callsite_position_hint",
        ImpactSliceEvidenceKind::NamePathHint => "name_path_hint",
    }
}

fn witness_negative_evidence_kind_label(kind: ImpactSliceNegativeEvidenceKind) -> &'static str {
    match kind {
        ImpactSliceNegativeEvidenceKind::NoisyReturnHint => "noisy_return_hint",
    }
}

fn witness_support_edge_certainty_label(
    certainty: ImpactSliceSupportEdgeCertainty,
) -> &'static str {
    match certainty {
        ImpactSliceSupportEdgeCertainty::Confirmed => "confirmed",
        ImpactSliceSupportEdgeCertainty::Inferred => "inferred",
        ImpactSliceSupportEdgeCertainty::DynamicFallback => "dynamic_fallback",
    }
}

fn witness_support_edge_certainty_rank(certainty: Option<ImpactSliceSupportEdgeCertainty>) -> u8 {
    match certainty {
        None => 0,
        Some(ImpactSliceSupportEdgeCertainty::DynamicFallback) => 1,
        Some(ImpactSliceSupportEdgeCertainty::Inferred) => 2,
        Some(ImpactSliceSupportEdgeCertainty::Confirmed) => 3,
    }
}

fn selected_vs_pruned_winning_primary_evidence_kinds(
    selected: &ImpactSliceCandidateScoringSummary,
    pruned: &ImpactSliceCandidateScoringSummary,
) -> Option<Vec<ImpactSliceEvidenceKind>> {
    let pruned_primary_evidence: HashSet<ImpactSliceEvidenceKind> =
        pruned.primary_evidence_kinds.iter().copied().collect();
    let mut seen = HashSet::new();
    let winning_primary_evidence_kinds: Vec<ImpactSliceEvidenceKind> = selected
        .primary_evidence_kinds
        .iter()
        .copied()
        .filter(|kind| !pruned_primary_evidence.contains(kind) && seen.insert(*kind))
        .collect();

    (!winning_primary_evidence_kinds.is_empty()).then_some(winning_primary_evidence_kinds)
}

fn selected_vs_pruned_winning_support(
    selected: &ImpactSliceCandidateScoringSummary,
    pruned: &ImpactSliceCandidateScoringSummary,
) -> Option<ImpactSliceCandidateSupportMetadata> {
    let selected_support = selected.support.as_ref()?;
    let pruned_support = pruned.support.as_ref();
    let winning_support = ImpactSliceCandidateSupportMetadata {
        call_graph_support: selected_support.call_graph_support
            && !pruned_support.is_some_and(|support| support.call_graph_support),
        local_dfg_support: selected_support.local_dfg_support
            && !pruned_support.is_some_and(|support| support.local_dfg_support),
        symbolic_propagation_support: selected_support.symbolic_propagation_support
            && !pruned_support.is_some_and(|support| support.symbolic_propagation_support),
        edge_certainty: selected_support.edge_certainty.filter(|certainty| {
            witness_support_edge_certainty_rank(Some(*certainty))
                > witness_support_edge_certainty_rank(
                    pruned_support.and_then(|support| support.edge_certainty),
                )
        }),
    };

    (!winning_support.is_empty()).then_some(winning_support)
}

fn selected_vs_pruned_summary_labels(labels: &[String]) -> String {
    match labels {
        [] => String::new(),
        [label] => label.clone(),
        [first, second] => format!("{first} + {second}"),
        [first, second, remaining @ ..] => {
            format!("{first} + {second} (+{} more)", remaining.len())
        }
    }
}

fn selected_vs_pruned_summary_support_labels(
    support: &ImpactSliceCandidateSupportMetadata,
) -> Vec<String> {
    let mut labels = Vec::new();
    if support.call_graph_support {
        labels.push("call_graph_support".to_string());
    }
    if support.local_dfg_support {
        labels.push("local_dfg_support".to_string());
    }
    if support.symbolic_propagation_support {
        labels.push("symbolic_propagation_support".to_string());
    }
    if let Some(edge_certainty) = support.edge_certainty {
        labels.push(format!(
            "edge_certainty={}",
            witness_support_edge_certainty_label(edge_certainty)
        ));
    }
    labels
}

#[allow(clippy::too_many_arguments)]
fn selected_vs_pruned_summary(
    selected_path: &str,
    pruned_path: &str,
    basis: ImpactWitnessSliceRankingBasis,
    selected: &ImpactSliceCandidateScoringSummary,
    pruned: &ImpactSliceCandidateScoringSummary,
    winning_primary_evidence_kinds: Option<&[ImpactSliceEvidenceKind]>,
    winning_support: Option<&ImpactSliceCandidateSupportMetadata>,
    losing_side_reason: Option<&str>,
) -> String {
    let reason = match basis {
        ImpactWitnessSliceRankingBasis::SourceKind => format!(
            "{} outranked {}",
            witness_source_kind_label(selected.source_kind),
            witness_source_kind_label(pruned.source_kind)
        ),
        ImpactWitnessSliceRankingBasis::Lane => format!(
            "{} outranked {}",
            witness_lane_label(selected.lane),
            witness_lane_label(pruned.lane)
        ),
        ImpactWitnessSliceRankingBasis::PrimaryEvidenceCount => format!(
            "it had more primary evidence ({} > {})",
            selected.score_tuple.primary_evidence_count, pruned.score_tuple.primary_evidence_count
        ),
        ImpactWitnessSliceRankingBasis::NegativeEvidenceCount => format!(
            "it had less negative evidence ({} < {})",
            selected.score_tuple.negative_evidence_count,
            pruned.score_tuple.negative_evidence_count
        ),
        ImpactWitnessSliceRankingBasis::SemanticSupportRank => format!(
            "it had stronger semantic support ({} > {})",
            selected.score_tuple.semantic_support_rank, pruned.score_tuple.semantic_support_rank
        ),
        ImpactWitnessSliceRankingBasis::SecondaryEvidenceCount => format!(
            "it had more secondary evidence ({} > {})",
            selected.score_tuple.secondary_evidence_count,
            pruned.score_tuple.secondary_evidence_count
        ),
        ImpactWitnessSliceRankingBasis::CallsitePosition => format!(
            "it had a stronger callsite position hint ({} > {})",
            selected.score_tuple.call_position_rank, pruned.score_tuple.call_position_rank
        ),
        ImpactWitnessSliceRankingBasis::LexicalTiebreak => format!(
            "lexical tiebreak favored {} over {}",
            selected_path, pruned_path
        ),
    };
    let mut details = Vec::new();
    if let Some(winning_primary_evidence_kinds) = winning_primary_evidence_kinds {
        let winning_evidence_labels: Vec<String> = winning_primary_evidence_kinds
            .iter()
            .map(|kind| witness_evidence_kind_label(*kind).to_string())
            .collect();
        details.push(format!(
            "winning primary evidence: {}",
            selected_vs_pruned_summary_labels(&winning_evidence_labels)
        ));
    }
    if let Some(winning_support) = winning_support {
        let winning_support_labels = selected_vs_pruned_summary_support_labels(winning_support);
        if !winning_support_labels.is_empty() {
            details.push(format!(
                "winning support: {}",
                selected_vs_pruned_summary_labels(&winning_support_labels)
            ));
        }
    }
    if let Some(losing_side_reason) = losing_side_reason {
        details.push(format!("losing side: {losing_side_reason}"));
    }
    if details.is_empty() {
        format!("selected over {} because {}", pruned_path, reason)
    } else {
        format!(
            "selected over {} because {}; {}",
            pruned_path,
            reason,
            details.join("; ")
        )
    }
}

fn selected_reason_ranking_basis(
    selected: &ImpactSliceCandidateScoringSummary,
    pruned: &ImpactSliceCandidateScoringSummary,
) -> Option<ImpactWitnessSliceRankingBasis> {
    match selected
        .score_tuple
        .source_rank
        .cmp(&pruned.score_tuple.source_rank)
    {
        std::cmp::Ordering::Less => return Some(ImpactWitnessSliceRankingBasis::SourceKind),
        std::cmp::Ordering::Greater => return None,
        std::cmp::Ordering::Equal => {}
    }
    match selected
        .score_tuple
        .lane_rank
        .cmp(&pruned.score_tuple.lane_rank)
    {
        std::cmp::Ordering::Less => return Some(ImpactWitnessSliceRankingBasis::Lane),
        std::cmp::Ordering::Greater => return None,
        std::cmp::Ordering::Equal => {}
    }
    match selected
        .score_tuple
        .primary_evidence_count
        .cmp(&pruned.score_tuple.primary_evidence_count)
    {
        std::cmp::Ordering::Greater => {
            return Some(ImpactWitnessSliceRankingBasis::PrimaryEvidenceCount);
        }
        std::cmp::Ordering::Less => return None,
        std::cmp::Ordering::Equal => {}
    }
    match selected
        .score_tuple
        .negative_evidence_count
        .cmp(&pruned.score_tuple.negative_evidence_count)
    {
        std::cmp::Ordering::Less => {
            return Some(ImpactWitnessSliceRankingBasis::NegativeEvidenceCount);
        }
        std::cmp::Ordering::Greater => return None,
        std::cmp::Ordering::Equal => {}
    }
    match selected
        .score_tuple
        .semantic_support_rank
        .cmp(&pruned.score_tuple.semantic_support_rank)
    {
        std::cmp::Ordering::Greater => {
            return Some(ImpactWitnessSliceRankingBasis::SemanticSupportRank);
        }
        std::cmp::Ordering::Less => return None,
        std::cmp::Ordering::Equal => {}
    }
    match selected
        .score_tuple
        .secondary_evidence_count
        .cmp(&pruned.score_tuple.secondary_evidence_count)
    {
        std::cmp::Ordering::Greater => {
            return Some(ImpactWitnessSliceRankingBasis::SecondaryEvidenceCount);
        }
        std::cmp::Ordering::Less => return None,
        std::cmp::Ordering::Equal => {}
    }
    match selected
        .score_tuple
        .call_position_rank
        .cmp(&pruned.score_tuple.call_position_rank)
    {
        std::cmp::Ordering::Greater => {
            return Some(ImpactWitnessSliceRankingBasis::CallsitePosition);
        }
        std::cmp::Ordering::Less => return None,
        std::cmp::Ordering::Equal => {}
    }
    match selected
        .score_tuple
        .lexical_tiebreak
        .cmp(&pruned.score_tuple.lexical_tiebreak)
    {
        std::cmp::Ordering::Less => Some(ImpactWitnessSliceRankingBasis::LexicalTiebreak),
        std::cmp::Ordering::Greater => None,
        std::cmp::Ordering::Equal => None,
    }
}

fn selected_vs_pruned_losing_side_reason(
    selected: &ImpactSliceCandidateScoringSummary,
    pruned: &ImpactSliceCandidateScoringSummary,
) -> Option<String> {
    let mut labels = Vec::new();
    let mut seen_negative_evidence = HashSet::new();
    for kind in pruned.negative_evidence_kinds.iter().copied() {
        if seen_negative_evidence.insert(kind) {
            labels.push(format!(
                "negative_evidence={}",
                witness_negative_evidence_kind_label(kind)
            ));
        }
    }
    if pruned.source_kind == ImpactSliceCandidateSourceKind::NarrowFallback
        && selected.source_kind != pruned.source_kind
    {
        labels.push(format!(
            "fallback_only={}",
            witness_source_kind_label(pruned.source_kind)
        ));
    } else if pruned.lane == ImpactSliceCandidateLane::ModuleCompanionFallback
        && selected.lane != pruned.lane
    {
        labels.push(format!("fallback_only={}", witness_lane_label(pruned.lane)));
    }
    let pruned_edge_certainty = pruned
        .support
        .as_ref()
        .and_then(|support| support.edge_certainty);
    let selected_edge_certainty = selected
        .support
        .as_ref()
        .and_then(|support| support.edge_certainty);
    if pruned_edge_certainty == Some(ImpactSliceSupportEdgeCertainty::DynamicFallback)
        && witness_support_edge_certainty_rank(selected_edge_certainty)
            > witness_support_edge_certainty_rank(pruned_edge_certainty)
    {
        labels.push("edge_certainty=dynamic_fallback".to_string());
    }

    (!labels.is_empty()).then(|| selected_vs_pruned_summary_labels(&labels))
}

fn selected_reason_matches_pruned_candidate(
    selected_path: &str,
    reason: &ImpactSliceReasonMetadata,
    candidate: &ImpactSlicePrunedCandidate,
) -> bool {
    matches!(
        reason.kind,
        ImpactSliceReasonKind::BridgeCompletionFile | ImpactSliceReasonKind::BridgeContinuationFile
    ) && matches!(
        candidate.prune_reason,
        ImpactSlicePruneReason::RankedOut
            | ImpactSlicePruneReason::SuppressedBeforeAdmit
            | ImpactSlicePruneReason::WeakerSameFamilySibling
    ) && candidate.path != selected_path
        && reason.seed_symbol_id == candidate.seed_symbol_id
        && reason.tier == candidate.tier
        && reason.kind == candidate.kind
        && reason.via_symbol_id == candidate.via_symbol_id
        && reason.via_path == candidate.via_path
}

fn build_selected_vs_pruned_reasons(
    selected_path: &str,
    seed_reasons: &[ImpactSliceReasonMetadata],
    pruned_candidates: &[ImpactSlicePrunedCandidate],
) -> Vec<ImpactWitnessSliceSelectedVsPrunedReason> {
    let mut reasons = Vec::new();

    for reason in seed_reasons {
        let Some(selected_scoring) = reason.scoring.as_ref() else {
            continue;
        };

        for candidate in pruned_candidates.iter().filter(|candidate| {
            selected_reason_matches_pruned_candidate(selected_path, reason, candidate)
        }) {
            let Some(pruned_scoring) = candidate.scoring.as_ref() else {
                continue;
            };
            let Some(selected_better_by) =
                selected_reason_ranking_basis(selected_scoring, pruned_scoring)
            else {
                continue;
            };
            let winning_primary_evidence_kinds =
                selected_vs_pruned_winning_primary_evidence_kinds(selected_scoring, pruned_scoring);
            let winning_support =
                selected_vs_pruned_winning_support(selected_scoring, pruned_scoring);
            let losing_side_reason =
                selected_vs_pruned_losing_side_reason(selected_scoring, pruned_scoring);
            reasons.push(ImpactWitnessSliceSelectedVsPrunedReason {
                via_symbol_id: reason.via_symbol_id.clone(),
                via_path: reason.via_path.clone(),
                selected_bridge_kind: reason.bridge_kind,
                pruned_path: candidate.path.clone(),
                prune_reason: candidate.prune_reason,
                pruned_bridge_kind: candidate.bridge_kind,
                selected_better_by,
                winning_primary_evidence_kinds: winning_primary_evidence_kinds.clone(),
                winning_support: winning_support.clone(),
                losing_side_reason: losing_side_reason.clone(),
                compact_explanation: candidate.compact_explanation.clone(),
                summary: selected_vs_pruned_summary(
                    selected_path,
                    &candidate.path,
                    selected_better_by,
                    selected_scoring,
                    pruned_scoring,
                    winning_primary_evidence_kinds.as_deref(),
                    winning_support.as_ref(),
                    losing_side_reason.as_deref(),
                ),
            });
        }
    }

    reasons
}

fn build_witness_slice_context(
    witness: &ImpactWitness,
    symbol_file_by_id: &HashMap<String, String>,
    slice_selection: &ImpactSliceSelectionSummary,
) -> ImpactWitnessSliceContext {
    let selected_files_by_path: HashMap<&str, &ImpactSliceFileMetadata> = slice_selection
        .files
        .iter()
        .filter(|file| file.scopes.explanation)
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
            let selected_vs_pruned_reasons = build_selected_vs_pruned_reasons(
                path.as_str(),
                &seed_reasons,
                &slice_selection.pruned_candidates,
            );
            Some(ImpactWitnessSliceFileContext {
                witness_hops: witness_hops_by_path
                    .remove(path.as_str())
                    .unwrap_or_default(),
                selection_reasons: metadata.reasons.clone(),
                seed_reasons,
                selected_vs_pruned_reasons,
                path,
            })
        })
        .collect();

    ImpactWitnessSliceContext {
        seed_symbol_id: witness.root_symbol_id.clone(),
        selected_files_on_path,
    }
}

fn bridge_execution_family_for_bridge_kind(
    bridge_kind: ImpactSliceBridgeKind,
) -> ImpactBridgeExecutionFamily {
    match bridge_kind {
        ImpactSliceBridgeKind::WrapperReturn => ImpactBridgeExecutionFamily::ReturnContinuation,
        ImpactSliceBridgeKind::BoundaryAliasContinuation => {
            ImpactBridgeExecutionFamily::AliasResultStitch
        }
        ImpactSliceBridgeKind::RequireRelativeChain => {
            ImpactBridgeExecutionFamily::RequireRelativeContinuation
        }
    }
}

fn bridge_execution_step_family_for_reason(
    reason: &ImpactSliceReasonMetadata,
) -> Option<ImpactBridgeExecutionStepFamily> {
    match (reason.kind, reason.bridge_kind) {
        (
            ImpactSliceReasonKind::BridgeCompletionFile,
            Some(ImpactSliceBridgeKind::WrapperReturn),
        ) => Some(ImpactBridgeExecutionStepFamily::SummaryReturnBridge),
        (
            ImpactSliceReasonKind::BridgeContinuationFile,
            Some(ImpactSliceBridgeKind::WrapperReturn),
        ) => Some(ImpactBridgeExecutionStepFamily::NestedSummaryBridge),
        (
            ImpactSliceReasonKind::BridgeCompletionFile
            | ImpactSliceReasonKind::BridgeContinuationFile,
            Some(ImpactSliceBridgeKind::BoundaryAliasContinuation),
        ) => Some(ImpactBridgeExecutionStepFamily::AliasResultStitch),
        (
            ImpactSliceReasonKind::BridgeCompletionFile
            | ImpactSliceReasonKind::BridgeContinuationFile,
            Some(ImpactSliceBridgeKind::RequireRelativeChain),
        ) => Some(ImpactBridgeExecutionStepFamily::RequireRelativeLoad),
        _ => None,
    }
}

fn bridge_execution_step_summary(
    step_family: ImpactBridgeExecutionStepFamily,
    bridge_kind: Option<ImpactSliceBridgeKind>,
    reason_kind: ImpactSliceReasonKind,
) -> String {
    let action = match step_family {
        ImpactBridgeExecutionStepFamily::CallsiteInputBinding => "entered selected boundary",
        ImpactBridgeExecutionStepFamily::SummaryReturnBridge => "selected bridge completion",
        ImpactBridgeExecutionStepFamily::NestedSummaryBridge => "selected bridge continuation",
        ImpactBridgeExecutionStepFamily::AliasResultStitch => "selected alias-result stitch",
        ImpactBridgeExecutionStepFamily::RequireRelativeLoad => {
            "selected require_relative continuation"
        }
    };
    match bridge_kind {
        Some(bridge_kind) => format!(
            "{} via {} ({})",
            action,
            witness_bridge_kind_label(bridge_kind),
            witness_reason_kind_label(reason_kind)
        ),
        None => format!("{} ({})", action, witness_reason_kind_label(reason_kind)),
    }
}

fn witness_reason_kind_label(kind: ImpactSliceReasonKind) -> &'static str {
    match kind {
        ImpactSliceReasonKind::SeedFile => "seed_file",
        ImpactSliceReasonKind::ChangedFile => "changed_file",
        ImpactSliceReasonKind::DirectCallerFile => "direct_caller_file",
        ImpactSliceReasonKind::DirectCalleeFile => "direct_callee_file",
        ImpactSliceReasonKind::BridgeCompletionFile => "bridge_completion_file",
        ImpactSliceReasonKind::BridgeContinuationFile => "bridge_continuation_file",
        ImpactSliceReasonKind::ModuleCompanionFile => "module_companion_file",
    }
}

fn witness_bridge_kind_label(kind: ImpactSliceBridgeKind) -> &'static str {
    match kind {
        ImpactSliceBridgeKind::WrapperReturn => "wrapper_return",
        ImpactSliceBridgeKind::BoundaryAliasContinuation => "boundary_alias_continuation",
        ImpactSliceBridgeKind::RequireRelativeChain => "require_relative_chain",
    }
}

fn relative_require_target(from_path: &str, to_path: &str) -> Option<String> {
    let from_parent = std::path::Path::new(from_path).parent()?;
    let from_components: Vec<_> = from_parent.components().collect();
    let to_components: Vec<_> = std::path::Path::new(to_path).components().collect();
    let mut shared = 0usize;
    while shared < from_components.len()
        && shared < to_components.len()
        && from_components[shared] == to_components[shared]
    {
        shared += 1;
    }

    let mut parts = Vec::new();
    for _ in shared..from_components.len() {
        parts.push("..".to_string());
    }
    for component in to_components.iter().skip(shared) {
        let part = component.as_os_str().to_string_lossy().to_string();
        if !part.is_empty() {
            parts.push(part);
        }
    }
    let relative = parts.join("/");
    let relative = relative
        .strip_suffix(".rb")
        .unwrap_or(&relative)
        .to_string();
    (!relative.is_empty()).then_some(relative)
}

fn file_has_require_relative_load(from_path: &str, to_path: &str) -> bool {
    if !(from_path.ends_with(".rb") && to_path.ends_with(".rb")) {
        return false;
    }
    let Some(target) = relative_require_target(from_path, to_path) else {
        return false;
    };
    let Ok(contents) = fs::read_to_string(from_path) else {
        return false;
    };
    [
        format!("require_relative \"{target}\""),
        format!("require_relative '{target}'"),
    ]
    .into_iter()
    .any(|needle| contents.contains(&needle))
}

fn build_bridge_execution_chain_compact(
    slice_context: &ImpactWitnessSliceContext,
    provenance_chain: &[EdgeProvenance],
) -> (
    Option<ImpactBridgeExecutionFamily>,
    Vec<ImpactBridgeExecutionStepCompact>,
) {
    let mut bridge_steps = Vec::new();
    let mut seen_steps = HashSet::new();

    for file_context in &slice_context.selected_files_on_path {
        for reason in &file_context.seed_reasons {
            let Some(step_family) = bridge_execution_step_family_for_reason(reason) else {
                continue;
            };
            let Some(bridge_kind) = reason.bridge_kind else {
                continue;
            };
            let Some(anchor_symbol_id) = reason.via_symbol_id.clone() else {
                continue;
            };
            if let Some(anchor_path) = reason.via_path.clone()
                && file_has_require_relative_load(anchor_path.as_str(), file_context.path.as_str())
            {
                let require_relative_step = ImpactBridgeExecutionStepCompact {
                    family: ImpactBridgeExecutionFamily::RequireRelativeContinuation,
                    step_family: ImpactBridgeExecutionStepFamily::RequireRelativeLoad,
                    anchor_symbol_id: anchor_symbol_id.clone(),
                    anchor_path: Some(anchor_path.clone()),
                    bridge_kind: Some(ImpactSliceBridgeKind::RequireRelativeChain),
                    reason_kind: Some(reason.kind),
                    summary: Some(bridge_execution_step_summary(
                        ImpactBridgeExecutionStepFamily::RequireRelativeLoad,
                        Some(ImpactSliceBridgeKind::RequireRelativeChain),
                        reason.kind,
                    )),
                };
                let key = (
                    require_relative_step.family,
                    require_relative_step.step_family,
                    anchor_symbol_id.clone(),
                    require_relative_step.anchor_path.clone(),
                    require_relative_step.bridge_kind,
                    require_relative_step.reason_kind,
                );
                if seen_steps.insert(key) {
                    bridge_steps.push(require_relative_step);
                }
            }

            let family = bridge_execution_family_for_bridge_kind(bridge_kind);
            let step = ImpactBridgeExecutionStepCompact {
                family,
                step_family,
                anchor_symbol_id: anchor_symbol_id.clone(),
                anchor_path: reason
                    .via_path
                    .clone()
                    .or_else(|| Some(file_context.path.clone())),
                bridge_kind: Some(bridge_kind),
                reason_kind: Some(reason.kind),
                summary: Some(bridge_execution_step_summary(
                    step_family,
                    Some(bridge_kind),
                    reason.kind,
                )),
            };
            let key = (
                step.family,
                step.step_family,
                anchor_symbol_id,
                step.anchor_path.clone(),
                step.bridge_kind,
                step.reason_kind,
            );
            if seen_steps.insert(key) {
                bridge_steps.push(step);
            }
        }
    }

    let has_alias_step = bridge_steps
        .iter()
        .any(|step| step.family == ImpactBridgeExecutionFamily::AliasResultStitch);
    let has_require_relative_step = bridge_steps
        .iter()
        .any(|step| step.family == ImpactBridgeExecutionFamily::RequireRelativeContinuation);
    let has_selected_alias_reason = slice_context
        .selected_files_on_path
        .iter()
        .flat_map(|file_context| file_context.seed_reasons.iter())
        .any(|reason| {
            matches!(
                reason.kind,
                ImpactSliceReasonKind::BridgeCompletionFile
                    | ImpactSliceReasonKind::BridgeContinuationFile
            ) && reason.bridge_kind == Some(ImpactSliceBridgeKind::BoundaryAliasContinuation)
        });
    let has_nested_step = bridge_steps
        .iter()
        .any(|step| step.step_family == ImpactBridgeExecutionStepFamily::NestedSummaryBridge);
    let has_return_step = bridge_steps.iter().any(|step| {
        matches!(
            step.step_family,
            ImpactBridgeExecutionStepFamily::SummaryReturnBridge
                | ImpactBridgeExecutionStepFamily::NestedSummaryBridge
        )
    });

    let representative_family = if has_selected_alias_reason {
        Some(ImpactBridgeExecutionFamily::AliasResultStitch)
    } else if has_alias_step && has_require_relative_step {
        Some(ImpactBridgeExecutionFamily::MixedRequireRelativeAliasStitch)
    } else if has_alias_step {
        Some(ImpactBridgeExecutionFamily::AliasResultStitch)
    } else if has_require_relative_step {
        Some(ImpactBridgeExecutionFamily::RequireRelativeContinuation)
    } else if has_nested_step && provenance_chain.contains(&EdgeProvenance::SymbolicPropagation) {
        Some(ImpactBridgeExecutionFamily::NestedMultiInputContinuation)
    } else if has_return_step {
        Some(ImpactBridgeExecutionFamily::ReturnContinuation)
    } else {
        None
    };

    if let (Some(entry_family), Some(boundary_reason)) = (
        representative_family.or_else(|| bridge_steps.first().map(|step| step.family)),
        slice_context
            .selected_files_on_path
            .iter()
            .flat_map(|file_context| {
                file_context
                    .seed_reasons
                    .iter()
                    .map(move |reason| (file_context, reason))
            })
            .find(|(_, reason)| {
                matches!(
                    reason.kind,
                    ImpactSliceReasonKind::DirectCallerFile
                        | ImpactSliceReasonKind::DirectCalleeFile
                ) && reason.via_symbol_id.is_some()
            }),
    ) {
        let (file_context, reason) = boundary_reason;
        let anchor_symbol_id = reason.via_symbol_id.clone().expect("checked is_some");
        let boundary_step = ImpactBridgeExecutionStepCompact {
            family: entry_family,
            step_family: ImpactBridgeExecutionStepFamily::CallsiteInputBinding,
            anchor_symbol_id: anchor_symbol_id.clone(),
            anchor_path: Some(file_context.path.clone()),
            bridge_kind: None,
            reason_kind: Some(reason.kind),
            summary: Some(bridge_execution_step_summary(
                ImpactBridgeExecutionStepFamily::CallsiteInputBinding,
                None,
                reason.kind,
            )),
        };
        let key = (
            boundary_step.family,
            boundary_step.step_family,
            anchor_symbol_id,
            boundary_step.anchor_path.clone(),
            boundary_step.bridge_kind,
            boundary_step.reason_kind,
        );
        if seen_steps.insert(key) {
            bridge_steps.insert(0, boundary_step);
        }
    }

    (representative_family, bridge_steps)
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
        if let Some(slice_context) = witness.slice_context.as_ref() {
            let (bridge_execution_family, bridge_execution_chain_compact) =
                build_bridge_execution_chain_compact(slice_context, &witness.provenance_chain);
            witness.bridge_execution_family = bridge_execution_family;
            witness.bridge_execution_chain_compact = bridge_execution_chain_compact;
        }
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
                    bridge_execution_family: None,
                    bridge_execution_chain_compact: vec![],
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

        let out = compute_impact(
            std::slice::from_ref(&changed),
            &index,
            &refs,
            &ImpactOptions::default(),
        );
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

        let out = compute_impact(std::slice::from_ref(&changed), &index, &refs, &opts);
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
        let mut out = compute_impact(std::slice::from_ref(&seed), &index, &refs, &opts);

        let wrapper_seed_reason = ImpactSliceReasonMetadata {
            seed_symbol_id: seed.id.0.clone(),
            tier: 2,
            kind: ImpactSliceReasonKind::BridgeCompletionFile,
            via_symbol_id: Some(mid.id.0.clone()),
            via_path: None,
            bridge_kind: Some(ImpactSliceBridgeKind::WrapperReturn),
            scoring: None,
        };
        let wrapper_other_reason = ImpactSliceReasonMetadata {
            seed_symbol_id: "rust:other.rs:fn:other:1".to_string(),
            tier: 1,
            kind: ImpactSliceReasonKind::DirectCalleeFile,
            via_symbol_id: Some(mid.id.0.clone()),
            via_path: None,
            bridge_kind: None,
            scoring: None,
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
                        scoring: None,
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
                        scoring: None,
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
                        scoring: None,
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
                            scoring: None,
                        }],
                        seed_reasons: vec![ImpactSliceReasonMetadata {
                            seed_symbol_id: seed.id.0.clone(),
                            tier: 0,
                            kind: ImpactSliceReasonKind::ChangedFile,
                            via_symbol_id: None,
                            via_path: None,
                            bridge_kind: None,
                            scoring: None,
                        }],
                        selected_vs_pruned_reasons: vec![],
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
                            scoring: None,
                        }],
                        selected_vs_pruned_reasons: vec![],
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
                            scoring: None,
                        }],
                        seed_reasons: vec![ImpactSliceReasonMetadata {
                            seed_symbol_id: seed.id.0.clone(),
                            tier: 1,
                            kind: ImpactSliceReasonKind::DirectCalleeFile,
                            via_symbol_id: Some(target.id.0.clone()),
                            via_path: None,
                            bridge_kind: None,
                            scoring: None,
                        }],
                        selected_vs_pruned_reasons: vec![],
                    },
                ],
            })
        );
    }

    #[test]
    fn attach_slice_selection_summary_adds_selected_vs_pruned_witness_reasoning() {
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
                "leaf.rs",
                &crate::ir::SymbolKind::Function,
                "source",
                3,
            ),
            name: "source".to_string(),
            kind: crate::ir::SymbolKind::Function,
            file: "leaf.rs".to_string(),
            range: crate::ir::TextRange {
                start_line: 3,
                end_line: 3,
            },
            language: "rust".to_string(),
        };
        let helper = Symbol {
            id: crate::ir::SymbolId::new(
                "rust",
                "aaa_helper.rs",
                &crate::ir::SymbolKind::Function,
                "noise",
                4,
            ),
            name: "noise".to_string(),
            kind: crate::ir::SymbolKind::Function,
            file: "aaa_helper.rs".to_string(),
            range: crate::ir::TextRange {
                start_line: 4,
                end_line: 4,
            },
            language: "rust".to_string(),
        };
        let index = SymbolIndex::build(vec![
            seed.clone(),
            mid.clone(),
            target.clone(),
            helper.clone(),
        ]);
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
            Reference {
                from: mid.id.clone(),
                to: helper.id.clone(),
                kind: crate::ir::reference::RefKind::Call,
                file: "wrapper.rs".to_string(),
                line: 21,
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
        let mut out = compute_impact(std::slice::from_ref(&seed), &index, &refs, &opts);

        let selected_reason = ImpactSliceReasonMetadata {
            seed_symbol_id: seed.id.0.clone(),
            tier: 2,
            kind: ImpactSliceReasonKind::BridgeCompletionFile,
            via_symbol_id: Some(mid.id.0.clone()),
            via_path: Some("wrapper.rs".to_string()),
            bridge_kind: Some(ImpactSliceBridgeKind::WrapperReturn),
            scoring: Some(ImpactSliceCandidateScoringSummary {
                source_kind: ImpactSliceCandidateSourceKind::GraphSecondHop,
                lane: ImpactSliceCandidateLane::ReturnContinuation,
                primary_evidence_kinds: vec![
                    ImpactSliceEvidenceKind::AssignedResult,
                    ImpactSliceEvidenceKind::ReturnFlow,
                ],
                secondary_evidence_kinds: vec![ImpactSliceEvidenceKind::CallsitePositionHint],
                negative_evidence_kinds: vec![],
                score_tuple: ImpactSliceScoreTuple {
                    source_rank: 0,
                    lane_rank: 0,
                    primary_evidence_count: 2,
                    secondary_evidence_count: 1,
                    negative_evidence_count: 0,
                    semantic_support_rank: 0,
                    call_position_rank: 8,
                    lexical_tiebreak: "leaf.rs".to_string(),
                },
                support: None,
            }),
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
                        scoring: None,
                    }],
                },
                ImpactSliceFileMetadata {
                    path: "wrapper.rs".to_string(),
                    scopes: ImpactSliceScopes {
                        cache_update: true,
                        local_dfg: true,
                        explanation: true,
                    },
                    reasons: vec![ImpactSliceReasonMetadata {
                        seed_symbol_id: seed.id.0.clone(),
                        tier: 1,
                        kind: ImpactSliceReasonKind::DirectCalleeFile,
                        via_symbol_id: Some(mid.id.0.clone()),
                        via_path: None,
                        bridge_kind: None,
                        scoring: None,
                    }],
                },
                ImpactSliceFileMetadata {
                    path: "leaf.rs".to_string(),
                    scopes: ImpactSliceScopes {
                        cache_update: true,
                        local_dfg: true,
                        explanation: true,
                    },
                    reasons: vec![selected_reason.clone()],
                },
            ],
            pruned_candidates: vec![ImpactSlicePrunedCandidate {
                seed_symbol_id: seed.id.0.clone(),
                path: "aaa_helper.rs".to_string(),
                tier: 2,
                kind: ImpactSliceReasonKind::BridgeCompletionFile,
                via_symbol_id: Some(mid.id.0.clone()),
                via_path: Some("wrapper.rs".to_string()),
                bridge_kind: Some(ImpactSliceBridgeKind::BoundaryAliasContinuation),
                prune_reason: ImpactSlicePruneReason::RankedOut,
                scoring: Some(ImpactSliceCandidateScoringSummary {
                    source_kind: ImpactSliceCandidateSourceKind::GraphSecondHop,
                    lane: ImpactSliceCandidateLane::AliasContinuation,
                    primary_evidence_kinds: vec![ImpactSliceEvidenceKind::AssignedResult],
                    secondary_evidence_kinds: vec![ImpactSliceEvidenceKind::NamePathHint],
                    negative_evidence_kinds: vec![],
                    score_tuple: ImpactSliceScoreTuple {
                        source_rank: 0,
                        lane_rank: 1,
                        primary_evidence_count: 1,
                        secondary_evidence_count: 1,
                        negative_evidence_count: 0,
                        semantic_support_rank: 0,
                        call_position_rank: 7,
                        lexical_tiebreak: "aaa_helper.rs".to_string(),
                    },
                    support: None,
                }),
                compact_explanation: None,
            }],
        };

        attach_slice_selection_summary(&mut out, &slice_selection);

        let witness = out
            .impacted_witnesses
            .get(&target.id.0)
            .expect("witness for leaf");
        let leaf_context = witness
            .slice_context
            .as_ref()
            .and_then(|context| {
                context
                    .selected_files_on_path
                    .iter()
                    .find(|file| file.path == "leaf.rs")
            })
            .expect("leaf witness context");
        assert_eq!(leaf_context.seed_reasons, vec![selected_reason]);
        assert_eq!(
            leaf_context.selected_vs_pruned_reasons,
            vec![ImpactWitnessSliceSelectedVsPrunedReason {
                via_symbol_id: Some(mid.id.0.clone()),
                via_path: Some("wrapper.rs".to_string()),
                selected_bridge_kind: Some(ImpactSliceBridgeKind::WrapperReturn),
                pruned_path: "aaa_helper.rs".to_string(),
                prune_reason: ImpactSlicePruneReason::RankedOut,
                pruned_bridge_kind: Some(ImpactSliceBridgeKind::BoundaryAliasContinuation),
                selected_better_by: ImpactWitnessSliceRankingBasis::Lane,
                winning_primary_evidence_kinds: Some(vec![ImpactSliceEvidenceKind::ReturnFlow]),
                winning_support: None,
                losing_side_reason: None,
                compact_explanation: None,
                summary: "selected over aaa_helper.rs because return_continuation outranked alias_continuation; winning primary evidence: return_flow".to_string(),
            }]
        );

        let helper_witness = out
            .impacted_witnesses
            .get(&helper.id.0)
            .expect("witness for helper");
        let helper_paths: Vec<&str> = helper_witness
            .slice_context
            .as_ref()
            .expect("helper witness slice context")
            .selected_files_on_path
            .iter()
            .map(|file| file.path.as_str())
            .collect();
        assert_eq!(helper_paths, vec!["main.rs", "wrapper.rs"]);
    }

    #[test]
    fn selected_vs_pruned_reason_derives_winning_metadata_for_source_kind_explanations() {
        let seed_symbol_id = "ruby:app/runner.rb:method:run:1".to_string();
        let via_symbol_id = "ruby:lib/service.rb:method:bounce:4".to_string();
        let via_path = "lib/service.rb".to_string();
        let reasons = build_selected_vs_pruned_reasons(
            "lib/leaf.rb",
            &[ImpactSliceReasonMetadata {
                seed_symbol_id: seed_symbol_id.clone(),
                tier: 2,
                kind: ImpactSliceReasonKind::BridgeCompletionFile,
                via_symbol_id: Some(via_symbol_id.clone()),
                via_path: Some(via_path.clone()),
                bridge_kind: Some(ImpactSliceBridgeKind::RequireRelativeChain),
                scoring: Some(ImpactSliceCandidateScoringSummary {
                    source_kind: ImpactSliceCandidateSourceKind::GraphSecondHop,
                    lane: ImpactSliceCandidateLane::ModuleCompanionFallback,
                    primary_evidence_kinds: vec![
                        ImpactSliceEvidenceKind::CompanionFileMatch,
                        ImpactSliceEvidenceKind::ExplicitRequireRelativeLoad,
                    ],
                    secondary_evidence_kinds: vec![],
                    negative_evidence_kinds: vec![],
                    score_tuple: ImpactSliceScoreTuple {
                        source_rank: 0,
                        lane_rank: 3,
                        primary_evidence_count: 2,
                        secondary_evidence_count: 0,
                        negative_evidence_count: 0,
                        semantic_support_rank: 0,
                        call_position_rank: 0,
                        lexical_tiebreak: "lib/leaf.rb".to_string(),
                    },
                    support: Some(ImpactSliceCandidateSupportMetadata {
                        symbolic_propagation_support: true,
                        edge_certainty: Some(ImpactSliceSupportEdgeCertainty::Confirmed),
                        ..ImpactSliceCandidateSupportMetadata::default()
                    }),
                }),
            }],
            &[ImpactSlicePrunedCandidate {
                seed_symbol_id,
                path: "lib/helper.rb".to_string(),
                tier: 2,
                kind: ImpactSliceReasonKind::BridgeCompletionFile,
                via_symbol_id: Some(via_symbol_id),
                via_path: Some(via_path),
                bridge_kind: Some(ImpactSliceBridgeKind::RequireRelativeChain),
                prune_reason: ImpactSlicePruneReason::RankedOut,
                scoring: Some(ImpactSliceCandidateScoringSummary {
                    source_kind: ImpactSliceCandidateSourceKind::NarrowFallback,
                    lane: ImpactSliceCandidateLane::ModuleCompanionFallback,
                    primary_evidence_kinds: vec![ImpactSliceEvidenceKind::CompanionFileMatch],
                    secondary_evidence_kinds: vec![],
                    negative_evidence_kinds: vec![],
                    score_tuple: ImpactSliceScoreTuple {
                        source_rank: 1,
                        lane_rank: 3,
                        primary_evidence_count: 1,
                        secondary_evidence_count: 0,
                        negative_evidence_count: 0,
                        semantic_support_rank: 0,
                        call_position_rank: 0,
                        lexical_tiebreak: "lib/helper.rb".to_string(),
                    },
                    support: Some(ImpactSliceCandidateSupportMetadata {
                        edge_certainty: Some(ImpactSliceSupportEdgeCertainty::DynamicFallback),
                        ..ImpactSliceCandidateSupportMetadata::default()
                    }),
                }),
                compact_explanation: None,
            }],
        );

        assert_eq!(
            reasons,
            vec![ImpactWitnessSliceSelectedVsPrunedReason {
                via_symbol_id: Some("ruby:lib/service.rb:method:bounce:4".to_string()),
                via_path: Some("lib/service.rb".to_string()),
                selected_bridge_kind: Some(ImpactSliceBridgeKind::RequireRelativeChain),
                pruned_path: "lib/helper.rb".to_string(),
                prune_reason: ImpactSlicePruneReason::RankedOut,
                pruned_bridge_kind: Some(ImpactSliceBridgeKind::RequireRelativeChain),
                selected_better_by: ImpactWitnessSliceRankingBasis::SourceKind,
                winning_primary_evidence_kinds: Some(vec![
                    ImpactSliceEvidenceKind::ExplicitRequireRelativeLoad,
                ]),
                winning_support: Some(ImpactSliceCandidateSupportMetadata {
                    symbolic_propagation_support: true,
                    edge_certainty: Some(ImpactSliceSupportEdgeCertainty::Confirmed),
                    ..ImpactSliceCandidateSupportMetadata::default()
                }),
                losing_side_reason: Some(
                    "fallback_only=narrow_fallback + edge_certainty=dynamic_fallback"
                        .to_string()
                ),
                compact_explanation: None,
                summary: "selected over lib/helper.rb because graph_second_hop outranked narrow_fallback; winning primary evidence: explicit_require_relative_load; winning support: symbolic_propagation_support + edge_certainty=confirmed; losing side: fallback_only=narrow_fallback + edge_certainty=dynamic_fallback".to_string(),
            }]
        );
    }

    #[test]
    fn selected_vs_pruned_reason_carries_compact_explanation_for_suppressed_before_admit() {
        let seed_symbol_id = "rust:main.rs:fn:caller:1".to_string();
        let via_symbol_id = "rust:wrapper.rs:fn:wrap:4".to_string();
        let reasons = build_selected_vs_pruned_reasons(
            "leaf.rs",
            &[ImpactSliceReasonMetadata {
                seed_symbol_id: seed_symbol_id.clone(),
                tier: 2,
                kind: ImpactSliceReasonKind::BridgeCompletionFile,
                via_symbol_id: Some(via_symbol_id.clone()),
                via_path: Some("wrapper.rs".to_string()),
                bridge_kind: Some(ImpactSliceBridgeKind::WrapperReturn),
                scoring: Some(ImpactSliceCandidateScoringSummary {
                    source_kind: ImpactSliceCandidateSourceKind::GraphSecondHop,
                    lane: ImpactSliceCandidateLane::ReturnContinuation,
                    primary_evidence_kinds: vec![
                        ImpactSliceEvidenceKind::AssignedResult,
                        ImpactSliceEvidenceKind::ReturnFlow,
                    ],
                    secondary_evidence_kinds: vec![ImpactSliceEvidenceKind::NamePathHint],
                    negative_evidence_kinds: vec![],
                    score_tuple: ImpactSliceScoreTuple {
                        source_rank: 0,
                        lane_rank: 0,
                        primary_evidence_count: 2,
                        secondary_evidence_count: 1,
                        negative_evidence_count: 0,
                        semantic_support_rank: 0,
                        call_position_rank: 5,
                        lexical_tiebreak: "leaf.rs".to_string(),
                    },
                    support: None,
                }),
            }],
            &[ImpactSlicePrunedCandidate {
                seed_symbol_id,
                path: "aaa_helper.rs".to_string(),
                tier: 2,
                kind: ImpactSliceReasonKind::BridgeCompletionFile,
                via_symbol_id: Some(via_symbol_id),
                via_path: Some("wrapper.rs".to_string()),
                bridge_kind: Some(ImpactSliceBridgeKind::BoundaryAliasContinuation),
                prune_reason: ImpactSlicePruneReason::SuppressedBeforeAdmit,
                scoring: Some(ImpactSliceCandidateScoringSummary {
                    source_kind: ImpactSliceCandidateSourceKind::GraphSecondHop,
                    lane: ImpactSliceCandidateLane::AliasContinuation,
                    primary_evidence_kinds: vec![ImpactSliceEvidenceKind::AssignedResult],
                    secondary_evidence_kinds: vec![ImpactSliceEvidenceKind::NamePathHint],
                    negative_evidence_kinds: vec![],
                    score_tuple: ImpactSliceScoreTuple {
                        source_rank: 0,
                        lane_rank: 1,
                        primary_evidence_count: 1,
                        secondary_evidence_count: 1,
                        negative_evidence_count: 0,
                        semantic_support_rank: 0,
                        call_position_rank: 4,
                        lexical_tiebreak: "aaa_helper.rs".to_string(),
                    },
                    support: None,
                }),
                compact_explanation: Some(
                    "suppressed_before_admit=helper_noise_suppressor".to_string(),
                ),
            }],
        );

        assert_eq!(
            reasons,
            vec![ImpactWitnessSliceSelectedVsPrunedReason {
                via_symbol_id: Some("rust:wrapper.rs:fn:wrap:4".to_string()),
                via_path: Some("wrapper.rs".to_string()),
                selected_bridge_kind: Some(ImpactSliceBridgeKind::WrapperReturn),
                pruned_path: "aaa_helper.rs".to_string(),
                prune_reason: ImpactSlicePruneReason::SuppressedBeforeAdmit,
                pruned_bridge_kind: Some(ImpactSliceBridgeKind::BoundaryAliasContinuation),
                selected_better_by: ImpactWitnessSliceRankingBasis::Lane,
                winning_primary_evidence_kinds: Some(vec![ImpactSliceEvidenceKind::ReturnFlow]),
                winning_support: None,
                losing_side_reason: None,
                compact_explanation: Some(
                    "suppressed_before_admit=helper_noise_suppressor".to_string(),
                ),
                summary: "selected over aaa_helper.rs because return_continuation outranked alias_continuation; winning primary evidence: return_flow".to_string(),
            }]
        );
    }

    #[test]
    fn selected_vs_pruned_reason_derives_losing_side_reason_from_negative_evidence() {
        let seed_symbol_id = "rust:main.rs:fn:caller:1".to_string();
        let via_symbol_id = "rust:wrapper.rs:fn:wrap:4".to_string();
        let reasons = build_selected_vs_pruned_reasons(
            "leaf.rs",
            &[ImpactSliceReasonMetadata {
                seed_symbol_id: seed_symbol_id.clone(),
                tier: 2,
                kind: ImpactSliceReasonKind::BridgeCompletionFile,
                via_symbol_id: Some(via_symbol_id.clone()),
                via_path: Some("wrapper.rs".to_string()),
                bridge_kind: Some(ImpactSliceBridgeKind::WrapperReturn),
                scoring: Some(ImpactSliceCandidateScoringSummary {
                    source_kind: ImpactSliceCandidateSourceKind::GraphSecondHop,
                    lane: ImpactSliceCandidateLane::ReturnContinuation,
                    primary_evidence_kinds: vec![
                        ImpactSliceEvidenceKind::AssignedResult,
                        ImpactSliceEvidenceKind::ReturnFlow,
                    ],
                    secondary_evidence_kinds: vec![ImpactSliceEvidenceKind::NamePathHint],
                    negative_evidence_kinds: vec![],
                    score_tuple: ImpactSliceScoreTuple {
                        source_rank: 0,
                        lane_rank: 0,
                        primary_evidence_count: 2,
                        secondary_evidence_count: 1,
                        negative_evidence_count: 0,
                        semantic_support_rank: 0,
                        call_position_rank: 5,
                        lexical_tiebreak: "leaf.rs".to_string(),
                    },
                    support: None,
                }),
            }],
            &[ImpactSlicePrunedCandidate {
                seed_symbol_id,
                path: "zzz_final_helper.rs".to_string(),
                tier: 2,
                kind: ImpactSliceReasonKind::BridgeCompletionFile,
                via_symbol_id: Some(via_symbol_id),
                via_path: Some("wrapper.rs".to_string()),
                bridge_kind: Some(ImpactSliceBridgeKind::WrapperReturn),
                prune_reason: ImpactSlicePruneReason::RankedOut,
                scoring: Some(ImpactSliceCandidateScoringSummary {
                    source_kind: ImpactSliceCandidateSourceKind::GraphSecondHop,
                    lane: ImpactSliceCandidateLane::ReturnContinuation,
                    primary_evidence_kinds: vec![
                        ImpactSliceEvidenceKind::AssignedResult,
                        ImpactSliceEvidenceKind::ReturnFlow,
                    ],
                    secondary_evidence_kinds: vec![
                        ImpactSliceEvidenceKind::CallsitePositionHint,
                        ImpactSliceEvidenceKind::NamePathHint,
                    ],
                    negative_evidence_kinds: vec![ImpactSliceNegativeEvidenceKind::NoisyReturnHint],
                    score_tuple: ImpactSliceScoreTuple {
                        source_rank: 0,
                        lane_rank: 0,
                        primary_evidence_count: 2,
                        secondary_evidence_count: 2,
                        negative_evidence_count: 1,
                        semantic_support_rank: 0,
                        call_position_rank: 6,
                        lexical_tiebreak: "zzz_final_helper.rs".to_string(),
                    },
                    support: None,
                }),
                compact_explanation: None,
            }],
        );

        assert_eq!(
            reasons,
            vec![ImpactWitnessSliceSelectedVsPrunedReason {
                via_symbol_id: Some("rust:wrapper.rs:fn:wrap:4".to_string()),
                via_path: Some("wrapper.rs".to_string()),
                selected_bridge_kind: Some(ImpactSliceBridgeKind::WrapperReturn),
                pruned_path: "zzz_final_helper.rs".to_string(),
                prune_reason: ImpactSlicePruneReason::RankedOut,
                pruned_bridge_kind: Some(ImpactSliceBridgeKind::WrapperReturn),
                selected_better_by: ImpactWitnessSliceRankingBasis::NegativeEvidenceCount,
                winning_primary_evidence_kinds: None,
                winning_support: None,
                losing_side_reason: Some("negative_evidence=noisy_return_hint".to_string()),
                compact_explanation: None,
                summary: "selected over zzz_final_helper.rs because it had less negative evidence (0 < 1); losing side: negative_evidence=noisy_return_hint".to_string(),
            }]
        );
    }

    #[test]
    fn selected_vs_pruned_reason_matches_bridge_continuation_candidates() {
        let reasons = build_selected_vs_pruned_reasons(
            "leaf.rs",
            &[ImpactSliceReasonMetadata {
                seed_symbol_id: "rust:main.rs:fn:caller:5".to_string(),
                tier: 3,
                kind: ImpactSliceReasonKind::BridgeContinuationFile,
                via_symbol_id: Some("rust:step.rs:fn:step:3".to_string()),
                via_path: Some("step.rs".to_string()),
                bridge_kind: Some(ImpactSliceBridgeKind::WrapperReturn),
                scoring: Some(ImpactSliceCandidateScoringSummary {
                    source_kind: ImpactSliceCandidateSourceKind::GraphSecondHop,
                    lane: ImpactSliceCandidateLane::ReturnContinuation,
                    primary_evidence_kinds: vec![
                        ImpactSliceEvidenceKind::AssignedResult,
                        ImpactSliceEvidenceKind::ReturnFlow,
                    ],
                    secondary_evidence_kinds: vec![ImpactSliceEvidenceKind::NamePathHint],
                    negative_evidence_kinds: vec![],
                    score_tuple: ImpactSliceScoreTuple {
                        source_rank: 0,
                        lane_rank: 0,
                        primary_evidence_count: 2,
                        secondary_evidence_count: 1,
                        negative_evidence_count: 0,
                        semantic_support_rank: 1,
                        call_position_rank: 5,
                        lexical_tiebreak: "leaf.rs".to_string(),
                    },
                    support: Some(ImpactSliceCandidateSupportMetadata {
                        local_dfg_support: true,
                        ..ImpactSliceCandidateSupportMetadata::default()
                    }),
                }),
            }],
            &[ImpactSlicePrunedCandidate {
                seed_symbol_id: "rust:main.rs:fn:caller:5".to_string(),
                path: "alt_leaf.rs".to_string(),
                tier: 3,
                kind: ImpactSliceReasonKind::BridgeContinuationFile,
                via_symbol_id: Some("rust:step.rs:fn:step:3".to_string()),
                via_path: Some("step.rs".to_string()),
                bridge_kind: Some(ImpactSliceBridgeKind::WrapperReturn),
                prune_reason: ImpactSlicePruneReason::RankedOut,
                scoring: Some(ImpactSliceCandidateScoringSummary {
                    source_kind: ImpactSliceCandidateSourceKind::GraphSecondHop,
                    lane: ImpactSliceCandidateLane::ReturnContinuation,
                    primary_evidence_kinds: vec![ImpactSliceEvidenceKind::AssignedResult],
                    secondary_evidence_kinds: vec![
                        ImpactSliceEvidenceKind::CallsitePositionHint,
                        ImpactSliceEvidenceKind::NamePathHint,
                    ],
                    negative_evidence_kinds: vec![],
                    score_tuple: ImpactSliceScoreTuple {
                        source_rank: 0,
                        lane_rank: 0,
                        primary_evidence_count: 1,
                        secondary_evidence_count: 2,
                        negative_evidence_count: 0,
                        semantic_support_rank: 0,
                        call_position_rank: 6,
                        lexical_tiebreak: "alt_leaf.rs".to_string(),
                    },
                    support: None,
                }),
                compact_explanation: None,
            }],
        );

        assert_eq!(reasons.len(), 1);
        assert_eq!(reasons[0].pruned_path, "alt_leaf.rs");
        assert_eq!(
            reasons[0].selected_better_by,
            ImpactWitnessSliceRankingBasis::PrimaryEvidenceCount
        );
        assert_eq!(
            reasons[0].winning_primary_evidence_kinds,
            Some(vec![ImpactSliceEvidenceKind::ReturnFlow])
        );
    }

    #[test]
    fn selected_vs_pruned_reason_matches_weaker_same_family_sibling_prune_reason() {
        let seed_symbol_id = "rust:main.rs:fn:caller:1".to_string();
        let via_symbol_id = "rust:wrapper.rs:fn:wrap:4".to_string();
        let reasons = build_selected_vs_pruned_reasons(
            "step.rs",
            &[ImpactSliceReasonMetadata {
                seed_symbol_id: seed_symbol_id.clone(),
                tier: 2,
                kind: ImpactSliceReasonKind::BridgeCompletionFile,
                via_symbol_id: Some(via_symbol_id.clone()),
                via_path: Some("wrapper.rs".to_string()),
                bridge_kind: Some(ImpactSliceBridgeKind::WrapperReturn),
                scoring: Some(ImpactSliceCandidateScoringSummary {
                    source_kind: ImpactSliceCandidateSourceKind::GraphSecondHop,
                    lane: ImpactSliceCandidateLane::ReturnContinuation,
                    primary_evidence_kinds: vec![
                        ImpactSliceEvidenceKind::AssignedResult,
                        ImpactSliceEvidenceKind::ParamToReturnFlow,
                        ImpactSliceEvidenceKind::ReturnFlow,
                    ],
                    secondary_evidence_kinds: vec![ImpactSliceEvidenceKind::NamePathHint],
                    negative_evidence_kinds: vec![],
                    score_tuple: ImpactSliceScoreTuple {
                        source_rank: 0,
                        lane_rank: 0,
                        primary_evidence_count: 3,
                        secondary_evidence_count: 1,
                        negative_evidence_count: 0,
                        semantic_support_rank: 2,
                        call_position_rank: 5,
                        lexical_tiebreak: "step.rs".to_string(),
                    },
                    support: Some(ImpactSliceCandidateSupportMetadata {
                        local_dfg_support: true,
                        ..ImpactSliceCandidateSupportMetadata::default()
                    }),
                }),
            }],
            &[ImpactSlicePrunedCandidate {
                seed_symbol_id,
                path: "later.rs".to_string(),
                tier: 2,
                kind: ImpactSliceReasonKind::BridgeCompletionFile,
                via_symbol_id: Some(via_symbol_id),
                via_path: Some("wrapper.rs".to_string()),
                bridge_kind: Some(ImpactSliceBridgeKind::WrapperReturn),
                prune_reason: ImpactSlicePruneReason::WeakerSameFamilySibling,
                scoring: Some(ImpactSliceCandidateScoringSummary {
                    source_kind: ImpactSliceCandidateSourceKind::GraphSecondHop,
                    lane: ImpactSliceCandidateLane::ReturnContinuation,
                    primary_evidence_kinds: vec![
                        ImpactSliceEvidenceKind::AssignedResult,
                        ImpactSliceEvidenceKind::ReturnFlow,
                    ],
                    secondary_evidence_kinds: vec![
                        ImpactSliceEvidenceKind::CallsitePositionHint,
                        ImpactSliceEvidenceKind::NamePathHint,
                    ],
                    negative_evidence_kinds: vec![],
                    score_tuple: ImpactSliceScoreTuple {
                        source_rank: 0,
                        lane_rank: 0,
                        primary_evidence_count: 2,
                        secondary_evidence_count: 2,
                        negative_evidence_count: 0,
                        semantic_support_rank: 0,
                        call_position_rank: 6,
                        lexical_tiebreak: "later.rs".to_string(),
                    },
                    support: None,
                }),
                compact_explanation: None,
            }],
        );

        assert_eq!(
            reasons,
            vec![ImpactWitnessSliceSelectedVsPrunedReason {
                via_symbol_id: Some("rust:wrapper.rs:fn:wrap:4".to_string()),
                via_path: Some("wrapper.rs".to_string()),
                selected_bridge_kind: Some(ImpactSliceBridgeKind::WrapperReturn),
                pruned_path: "later.rs".to_string(),
                prune_reason: ImpactSlicePruneReason::WeakerSameFamilySibling,
                pruned_bridge_kind: Some(ImpactSliceBridgeKind::WrapperReturn),
                selected_better_by: ImpactWitnessSliceRankingBasis::PrimaryEvidenceCount,
                winning_primary_evidence_kinds: Some(vec![
                    ImpactSliceEvidenceKind::ParamToReturnFlow,
                ]),
                winning_support: Some(ImpactSliceCandidateSupportMetadata {
                    local_dfg_support: true,
                    ..ImpactSliceCandidateSupportMetadata::default()
                }),
                losing_side_reason: None,
                compact_explanation: None,
                summary: "selected over later.rs because it had more primary evidence (3 > 2); winning primary evidence: param_to_return_flow; winning support: local_dfg_support".to_string(),
            }]
        );
    }

    #[test]
    fn selected_vs_pruned_reason_ignores_same_path_duplicates_and_budget_exhaustion() {
        let seed_symbol_id = "ruby:app/runner.rb:method:entry:3".to_string();
        let via_symbol_id = "ruby:lib/service.rb:method:bounce:4".to_string();
        let reasons = build_selected_vs_pruned_reasons(
            "lib/route_runtime.rb",
            &[ImpactSliceReasonMetadata {
                seed_symbol_id: seed_symbol_id.clone(),
                tier: 2,
                kind: ImpactSliceReasonKind::BridgeCompletionFile,
                via_symbol_id: Some(via_symbol_id.clone()),
                via_path: Some("lib/service.rb".to_string()),
                bridge_kind: Some(ImpactSliceBridgeKind::BoundaryAliasContinuation),
                scoring: Some(ImpactSliceCandidateScoringSummary {
                    source_kind: ImpactSliceCandidateSourceKind::GraphSecondHop,
                    lane: ImpactSliceCandidateLane::AliasContinuation,
                    primary_evidence_kinds: vec![
                        ImpactSliceEvidenceKind::AliasChain,
                        ImpactSliceEvidenceKind::AssignedResult,
                    ],
                    secondary_evidence_kinds: vec![
                        ImpactSliceEvidenceKind::CallsitePositionHint,
                        ImpactSliceEvidenceKind::NamePathHint,
                    ],
                    negative_evidence_kinds: vec![],
                    score_tuple: ImpactSliceScoreTuple {
                        source_rank: 0,
                        lane_rank: 1,
                        primary_evidence_count: 2,
                        secondary_evidence_count: 2,
                        negative_evidence_count: 0,
                        semantic_support_rank: 0,
                        call_position_rank: 7,
                        lexical_tiebreak: "lib/route_runtime.rb".to_string(),
                    },
                    support: None,
                }),
            }],
            &[
                ImpactSlicePrunedCandidate {
                    seed_symbol_id: seed_symbol_id.clone(),
                    path: "lib/route_runtime.rb".to_string(),
                    tier: 2,
                    kind: ImpactSliceReasonKind::BridgeCompletionFile,
                    via_symbol_id: Some(via_symbol_id.clone()),
                    via_path: Some("lib/service.rb".to_string()),
                    bridge_kind: Some(ImpactSliceBridgeKind::BoundaryAliasContinuation),
                    prune_reason: ImpactSlicePruneReason::WeakerSamePathDuplicate,
                    scoring: Some(ImpactSliceCandidateScoringSummary {
                        source_kind: ImpactSliceCandidateSourceKind::GraphSecondHop,
                        lane: ImpactSliceCandidateLane::AliasContinuation,
                        primary_evidence_kinds: vec![
                            ImpactSliceEvidenceKind::AliasChain,
                            ImpactSliceEvidenceKind::AssignedResult,
                        ],
                        secondary_evidence_kinds: vec![ImpactSliceEvidenceKind::NamePathHint],
                        negative_evidence_kinds: vec![],
                        score_tuple: ImpactSliceScoreTuple {
                            source_rank: 0,
                            lane_rank: 1,
                            primary_evidence_count: 2,
                            secondary_evidence_count: 1,
                            negative_evidence_count: 0,
                            semantic_support_rank: 0,
                            call_position_rank: 4,
                            lexical_tiebreak: "lib/route_runtime.rb".to_string(),
                        },
                        support: None,
                    }),
                    compact_explanation: Some(
                        "suppressed_before_admit=weaker_same_path_duplicate".to_string(),
                    ),
                },
                ImpactSlicePrunedCandidate {
                    seed_symbol_id,
                    path: "lib/fallback_runtime.rb".to_string(),
                    tier: 2,
                    kind: ImpactSliceReasonKind::BridgeCompletionFile,
                    via_symbol_id: Some(via_symbol_id),
                    via_path: Some("lib/service.rb".to_string()),
                    bridge_kind: Some(ImpactSliceBridgeKind::BoundaryAliasContinuation),
                    prune_reason: ImpactSlicePruneReason::BridgeBudgetExhausted,
                    scoring: Some(ImpactSliceCandidateScoringSummary {
                        source_kind: ImpactSliceCandidateSourceKind::GraphSecondHop,
                        lane: ImpactSliceCandidateLane::AliasContinuation,
                        primary_evidence_kinds: vec![
                            ImpactSliceEvidenceKind::AliasChain,
                            ImpactSliceEvidenceKind::AssignedResult,
                        ],
                        secondary_evidence_kinds: vec![ImpactSliceEvidenceKind::NamePathHint],
                        negative_evidence_kinds: vec![],
                        score_tuple: ImpactSliceScoreTuple {
                            source_rank: 0,
                            lane_rank: 1,
                            primary_evidence_count: 2,
                            secondary_evidence_count: 1,
                            negative_evidence_count: 0,
                            semantic_support_rank: 0,
                            call_position_rank: 8,
                            lexical_tiebreak: "lib/fallback_runtime.rb".to_string(),
                        },
                        support: Some(ImpactSliceCandidateSupportMetadata {
                            edge_certainty: Some(ImpactSliceSupportEdgeCertainty::DynamicFallback),
                            ..ImpactSliceCandidateSupportMetadata::default()
                        }),
                    }),
                    compact_explanation: Some("bridge_budget_exhausted".to_string()),
                },
            ],
        );

        assert!(
            reasons.is_empty(),
            "expected same-path duplicates and bridge-budget drops to stay out of witness selected-vs-pruned explanation scope"
        );
    }

    #[test]
    fn selected_vs_pruned_reason_omits_optional_winning_fields_when_absent() {
        let reason = ImpactWitnessSliceSelectedVsPrunedReason {
            via_symbol_id: Some("rust:wrapper.rs:fn:wrap:2".to_string()),
            via_path: Some("wrapper.rs".to_string()),
            selected_bridge_kind: Some(ImpactSliceBridgeKind::WrapperReturn),
            pruned_path: "helper.rs".to_string(),
            prune_reason: ImpactSlicePruneReason::RankedOut,
            pruned_bridge_kind: Some(ImpactSliceBridgeKind::BoundaryAliasContinuation),
            selected_better_by: ImpactWitnessSliceRankingBasis::Lane,
            winning_primary_evidence_kinds: None,
            winning_support: None,
            losing_side_reason: None,
            compact_explanation: None,
            summary:
                "selected over helper.rs because return_continuation outranked alias_continuation"
                    .to_string(),
        };

        let value = serde_json::to_value(&reason).expect("serialize witness reason");
        assert!(value.get("winning_primary_evidence_kinds").is_none());
        assert!(value.get("winning_support").is_none());
        assert!(value.get("losing_side_reason").is_none());
        assert!(value.get("compact_explanation").is_none());
    }

    #[test]
    fn attach_slice_selection_summary_omits_non_explanation_files_from_witness_context() {
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
        let mut out = ImpactOutput {
            changed_symbols: vec![seed.clone()],
            impacted_symbols: vec![mid.clone(), target.clone()],
            impacted_witnesses: HashMap::from([(
                target.id.0.clone(),
                ImpactWitness {
                    symbol_id: target.id.0.clone(),
                    depth: 1,
                    root_symbol_id: seed.id.0.clone(),
                    via_symbol_id: mid.id.0.clone(),
                    edge: Reference {
                        from: mid.id.clone(),
                        to: target.id.clone(),
                        kind: RefKind::Call,
                        line: 11,
                        file: "wrapper.rs".to_string(),
                        certainty: crate::ir::reference::EdgeCertainty::Confirmed,
                        provenance: EdgeProvenance::CallGraph,
                    },
                    path: vec![
                        ImpactWitnessHop {
                            from_symbol_id: seed.id.0.clone(),
                            to_symbol_id: mid.id.0.clone(),
                            edge: Reference {
                                from: seed.id.clone(),
                                to: mid.id.clone(),
                                kind: RefKind::Call,
                                line: 10,
                                file: "main.rs".to_string(),
                                certainty: crate::ir::reference::EdgeCertainty::Confirmed,
                                provenance: EdgeProvenance::CallGraph,
                            },
                        },
                        ImpactWitnessHop {
                            from_symbol_id: mid.id.0.clone(),
                            to_symbol_id: target.id.0.clone(),
                            edge: Reference {
                                from: mid.id.clone(),
                                to: target.id.clone(),
                                kind: RefKind::Call,
                                line: 11,
                                file: "wrapper.rs".to_string(),
                                certainty: crate::ir::reference::EdgeCertainty::Confirmed,
                                provenance: EdgeProvenance::CallGraph,
                            },
                        },
                    ],
                    provenance_chain: vec![],
                    kind_chain: vec![],
                    path_compact: vec![],
                    provenance_chain_compact: vec![],
                    kind_chain_compact: vec![],
                    bridge_execution_family: None,
                    bridge_execution_chain_compact: vec![],
                    slice_context: None,
                },
            )]),
            impacted_files: vec![],
            edges: vec![],
            impacted_by_file: HashMap::new(),
            summary: ImpactSummary::default(),
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
                        kind: ImpactSliceReasonKind::SeedFile,
                        via_symbol_id: None,
                        via_path: None,
                        bridge_kind: None,
                        scoring: None,
                    }],
                },
                ImpactSliceFileMetadata {
                    path: "wrapper.rs".to_string(),
                    scopes: ImpactSliceScopes {
                        cache_update: true,
                        local_dfg: true,
                        explanation: false,
                    },
                    reasons: vec![ImpactSliceReasonMetadata {
                        seed_symbol_id: seed.id.0.clone(),
                        tier: 2,
                        kind: ImpactSliceReasonKind::BridgeCompletionFile,
                        via_symbol_id: Some(mid.id.0.clone()),
                        via_path: None,
                        bridge_kind: Some(ImpactSliceBridgeKind::WrapperReturn),
                        scoring: None,
                    }],
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
                        scoring: None,
                    }],
                },
            ],
            pruned_candidates: Vec::new(),
        };

        attach_slice_selection_summary(&mut out, &slice_selection);

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
                            kind: ImpactSliceReasonKind::SeedFile,
                            via_symbol_id: None,
                            via_path: None,
                            bridge_kind: None,
                            scoring: None,
                        }],
                        seed_reasons: vec![ImpactSliceReasonMetadata {
                            seed_symbol_id: seed.id.0.clone(),
                            tier: 0,
                            kind: ImpactSliceReasonKind::SeedFile,
                            via_symbol_id: None,
                            via_path: None,
                            bridge_kind: None,
                            scoring: None,
                        }],
                        selected_vs_pruned_reasons: vec![],
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
                            scoring: None,
                        }],
                        seed_reasons: vec![ImpactSliceReasonMetadata {
                            seed_symbol_id: seed.id.0.clone(),
                            tier: 1,
                            kind: ImpactSliceReasonKind::DirectCalleeFile,
                            via_symbol_id: Some(target.id.0.clone()),
                            via_path: None,
                            bridge_kind: None,
                            scoring: None,
                        }],
                        selected_vs_pruned_reasons: vec![],
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

    #[test]
    fn scoring_summary_omits_empty_support_metadata_from_json() {
        let scoring = ImpactSliceCandidateScoringSummary {
            source_kind: ImpactSliceCandidateSourceKind::GraphSecondHop,
            lane: ImpactSliceCandidateLane::ReturnContinuation,
            primary_evidence_kinds: vec![ImpactSliceEvidenceKind::ReturnFlow],
            secondary_evidence_kinds: vec![],
            negative_evidence_kinds: vec![],
            score_tuple: ImpactSliceScoreTuple {
                source_rank: 0,
                lane_rank: 0,
                primary_evidence_count: 1,
                secondary_evidence_count: 0,
                negative_evidence_count: 0,
                semantic_support_rank: 0,
                call_position_rank: 3,
                lexical_tiebreak: "leaf.rs".to_string(),
            },
            support: Some(ImpactSliceCandidateSupportMetadata::default()),
        };

        let value = serde_json::to_value(&scoring).expect("serialize scoring summary");

        assert_eq!(
            value,
            serde_json::json!({
                "source_kind": "graph_second_hop",
                "lane": "return_continuation",
                "primary_evidence_kinds": ["return_flow"],
                "secondary_evidence_kinds": [],
                "score_tuple": {
                    "source_rank": 0,
                    "lane_rank": 0,
                    "primary_evidence_count": 1,
                    "secondary_evidence_count": 0,
                    "call_position_rank": 3,
                    "lexical_tiebreak": "leaf.rs"
                }
            })
        );
    }

    #[test]
    fn scoring_summary_round_trips_new_evidence_kinds_and_support_metadata() {
        let scoring = ImpactSliceCandidateScoringSummary {
            source_kind: ImpactSliceCandidateSourceKind::NarrowFallback,
            lane: ImpactSliceCandidateLane::ModuleCompanionFallback,
            primary_evidence_kinds: vec![
                ImpactSliceEvidenceKind::CompanionFileMatch,
                ImpactSliceEvidenceKind::DynamicDispatchLiteralTarget,
                ImpactSliceEvidenceKind::ExplicitRequireRelativeLoad,
                ImpactSliceEvidenceKind::ParamToReturnFlow,
            ],
            secondary_evidence_kinds: vec![ImpactSliceEvidenceKind::NamePathHint],
            negative_evidence_kinds: vec![ImpactSliceNegativeEvidenceKind::NoisyReturnHint],
            score_tuple: ImpactSliceScoreTuple {
                source_rank: 1,
                lane_rank: 3,
                primary_evidence_count: 4,
                secondary_evidence_count: 1,
                negative_evidence_count: 1,
                semantic_support_rank: 2,
                call_position_rank: 0,
                lexical_tiebreak: "demo/helper.rb".to_string(),
            },
            support: Some(ImpactSliceCandidateSupportMetadata {
                call_graph_support: true,
                local_dfg_support: true,
                symbolic_propagation_support: true,
                edge_certainty: Some(ImpactSliceSupportEdgeCertainty::DynamicFallback),
            }),
        };

        let value = serde_json::to_value(&scoring).expect("serialize scoring summary");
        assert_eq!(
            value["primary_evidence_kinds"],
            serde_json::json!([
                "companion_file_match",
                "dynamic_dispatch_literal_target",
                "explicit_require_relative_load",
                "param_to_return_flow"
            ])
        );
        assert_eq!(
            value["support"],
            serde_json::json!({
                "call_graph_support": true,
                "local_dfg_support": true,
                "symbolic_propagation_support": true,
                "edge_certainty": "dynamic_fallback"
            })
        );
        assert_eq!(
            value["negative_evidence_kinds"],
            serde_json::json!(["noisy_return_hint"])
        );
        assert_eq!(value["score_tuple"]["negative_evidence_count"], 1);

        let round_tripped: ImpactSliceCandidateScoringSummary =
            serde_json::from_value(value).expect("deserialize scoring summary");
        assert_eq!(round_tripped, scoring);
    }
}
