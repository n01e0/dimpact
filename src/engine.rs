use crate::{ChangedOutput, FileChanges, ImpactOptions, ImpactOutput, LanguageMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineKind {
    Auto,
    Ts,
    Lsp,
}

pub trait AnalysisEngine {
    fn changed_symbols(
        &self,
        diffs: &[FileChanges],
        lang: LanguageMode,
    ) -> anyhow::Result<ChangedOutput>;
    fn impact(
        &self,
        diffs: &[FileChanges],
        lang: LanguageMode,
        opts: &ImpactOptions,
    ) -> anyhow::Result<ImpactOutput>;
    fn impact_from_symbols(
        &self,
        changed: &[crate::ir::Symbol],
        lang: LanguageMode,
        opts: &ImpactOptions,
    ) -> anyhow::Result<ImpactOutput>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct EngineConfig {
    pub lsp_strict: bool,
    pub dump_capabilities: bool,
    pub mock_lsp: bool,
    pub mock_caps: Option<CapsHint>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CapsHint {
    pub call_hierarchy: bool,
    pub references: bool,
    pub definition: bool,
    pub document_symbol: bool,
    pub workspace_symbol: bool,
}

pub fn make_engine(kind: EngineKind, cfg: EngineConfig) -> Box<dyn AnalysisEngine> {
    match kind {
        EngineKind::Auto => {
            log::info!("engine: kind=Auto (Tree-Sitter default)");
            Box::new(self::ts::TsEngine)
        }
        EngineKind::Ts => Box::new(self::ts::TsEngine),
        EngineKind::Lsp => {
            log::warn!("engine: kind=LSP (experimental) strict={}", cfg.lsp_strict);
            Box::new(self::lsp::LspEngine::new(cfg))
        }
    }
}

// Submodules
pub mod lsp;
pub mod ts;
