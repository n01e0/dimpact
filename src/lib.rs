pub mod cache;
pub mod dfg;
pub mod diff;
pub mod engine;
pub mod impact;
pub mod ir;
pub mod languages;
pub mod mapping;
pub mod render;
pub mod schema;
pub mod ts_core;

pub use dfg::{DataFlowGraph, DependencyKind, DfgBuilder, DfgEdge, DfgNode};
pub use diff::{Change, ChangeKind, DiffParseError, FileChanges, parse_unified_diff};
pub use engine::EngineConfig;
pub use engine::{AnalysisEngine, EngineKind};
pub use impact::{
    ImpactAffectedModule, ImpactDepthBucket, ImpactDirection, ImpactOptions, ImpactOutput,
    ImpactRiskLevel, ImpactRiskSummary, ImpactSliceBridgeKind, ImpactSliceCandidateLane,
    ImpactSliceCandidateScoringSummary, ImpactSliceCandidateSourceKind,
    ImpactSliceCandidateSupportMetadata, ImpactSliceEvidenceKind, ImpactSliceFileMetadata,
    ImpactSliceNegativeEvidenceKind, ImpactSlicePlannerKind, ImpactSlicePruneReason,
    ImpactSlicePrunedCandidate, ImpactSliceReasonKind, ImpactSliceReasonMetadata,
    ImpactSliceScopes, ImpactSliceScoreTuple, ImpactSliceSelectionSummary,
    ImpactSliceSupportEdgeCertainty, ImpactSummary, ImpactWitness, ImpactWitnessHop,
    ImpactWitnessSliceContext, ImpactWitnessSliceFileContext, ImpactWitnessSliceRankingBasis,
    ImpactWitnessSliceSelectedVsPrunedReason, attach_slice_selection_summary, build_project_graph,
    compute_impact, path_is_ignored,
};
pub use ir::{Symbol, SymbolId, SymbolKind, TextRange};
pub use languages::LanguageKind;
pub use mapping::{ChangedOutput, LanguageMode, compute_changed_symbols};
pub use render::{dfg_to_dot, to_dot, to_html};
pub use schema::{
    ImpactSchemaEdgeDetail, ImpactSchemaGraphMode, ImpactSchemaLayout, ImpactSchemaProfile,
    JSON_SCHEMA_DRAFT_URL, JSON_SCHEMA_FORMAT, JSON_SCHEMA_MAJOR_VERSION, JSON_SCHEMA_NAMESPACE,
    JSON_SCHEMA_ROOT, ResolvedSchemaProfile, SchemaCommand, SchemaOutputFormat, SchemaProfile,
    SchemaProfileInput, SchemaProfileResolveError, SchemaRegistryError, find_registered_schema,
    list_registered_schemas, read_schema_document, registered_schema_profiles,
    resolve_schema_profile,
};
