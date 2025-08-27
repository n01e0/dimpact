pub mod diff;
pub mod ir;
pub mod languages;
pub mod mapping;
pub mod ts_core;
pub mod impact;
pub mod engine;
pub mod render;

pub use diff::{parse_unified_diff, Change, ChangeKind, DiffParseError, FileChanges};
pub use ir::{Symbol, SymbolId, SymbolKind, TextRange};
pub use mapping::{ChangedOutput, LanguageMode, compute_changed_symbols};
pub use impact::{build_project_graph, compute_impact, ImpactDirection, ImpactOptions, ImpactOutput};
pub use languages::LanguageKind;
pub use engine::{AnalysisEngine, EngineKind};
pub use engine::EngineConfig;
pub use render::{to_dot, to_html};
