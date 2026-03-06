use crate::{ChangedOutput, FileChanges, ImpactOptions, ImpactOutput, LanguageMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineKind {
    Auto,
    Ts,
    Lsp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoPolicy {
    Compat,
    StrictIfAvailable,
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

fn parse_auto_policy_env(v: &str) -> Option<AutoPolicy> {
    match v.trim().to_ascii_lowercase().as_str() {
        "compat" => Some(AutoPolicy::Compat),
        "strict-if-available" | "strict_if_available" => Some(AutoPolicy::StrictIfAvailable),
        _ => None,
    }
}

fn auto_policy_from_env_or_default() -> AutoPolicy {
    std::env::var("DIMPACT_AUTO_POLICY")
        .ok()
        .as_deref()
        .and_then(parse_auto_policy_env)
        .unwrap_or(AutoPolicy::Compat)
}

pub fn make_engine_with_auto_policy(
    kind: EngineKind,
    cfg: EngineConfig,
    auto_policy: Option<AutoPolicy>,
) -> Box<dyn AnalysisEngine> {
    match kind {
        EngineKind::Auto => match auto_policy.unwrap_or_else(auto_policy_from_env_or_default) {
            AutoPolicy::Compat => {
                log::info!("engine: kind=Auto policy=compat selected=TS");
                Box::new(self::ts::TsEngine)
            }
            AutoPolicy::StrictIfAvailable => {
                let mut lsp_cfg = cfg;
                // strict-if-available: prefer LSP, but do not hard-fail on unavailability.
                lsp_cfg.lsp_strict = false;
                log::info!(
                    "engine: kind=Auto policy=strict-if-available selected=LSP(prefer) fallback=TS"
                );
                Box::new(self::lsp::LspEngine::new(lsp_cfg))
            }
        },
        EngineKind::Ts => Box::new(self::ts::TsEngine),
        EngineKind::Lsp => {
            log::info!("engine: kind=LSP (GA) strict={}", cfg.lsp_strict);
            Box::new(self::lsp::LspEngine::new(cfg))
        }
    }
}

pub fn make_engine(kind: EngineKind, cfg: EngineConfig) -> Box<dyn AnalysisEngine> {
    make_engine_with_auto_policy(kind, cfg, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_auto_policy_env_accepts_known_values() {
        assert_eq!(parse_auto_policy_env("compat"), Some(AutoPolicy::Compat));
        assert_eq!(
            parse_auto_policy_env("strict-if-available"),
            Some(AutoPolicy::StrictIfAvailable)
        );
        assert_eq!(
            parse_auto_policy_env("strict_if_available"),
            Some(AutoPolicy::StrictIfAvailable)
        );
    }

    #[test]
    fn parse_auto_policy_env_rejects_unknown_values() {
        assert_eq!(parse_auto_policy_env(""), None);
        assert_eq!(parse_auto_policy_env("unknown"), None);
    }
}

// Submodules
pub mod lsp;
pub mod ts;
