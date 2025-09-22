use crate::cache;
use crate::{
    ChangedOutput, FileChanges, ImpactOptions, ImpactOutput, LanguageMode, compute_changed_symbols,
    compute_impact,
};

#[derive(Default)]
pub struct TsEngine;

impl super::AnalysisEngine for TsEngine {
    fn changed_symbols(
        &self,
        diffs: &[FileChanges],
        lang: LanguageMode,
    ) -> anyhow::Result<ChangedOutput> {
        compute_changed_symbols(diffs, lang)
    }

    fn impact(
        &self,
        diffs: &[FileChanges],
        lang: LanguageMode,
        opts: &ImpactOptions,
    ) -> anyhow::Result<ImpactOutput> {
        let changed: ChangedOutput = compute_changed_symbols(diffs, lang)?;
        // Open local cache and ensure built; then update changed files incrementally
        let (scope, dir_override) = cache::scope_from_env();
        let mut db = cache::open(scope, dir_override.as_deref())?;
        let st = cache::stats(&db.conn)?;
        if st.symbols == 0 {
            log::info!("cache: empty → build all");
            cache::build_all(&mut db.conn)?;
        }
        if !changed.changed_files.is_empty() {
            log::info!(
                "cache: updating {} changed file(s)",
                changed.changed_files.len()
            );
            cache::update_paths(&mut db.conn, &changed.changed_files)?;
        }
        let (index, refs) = cache::load_graph(&db.conn)?;
        let out = compute_impact(&changed.changed_symbols, &index, &refs, opts);
        Ok(out)
    }

    fn impact_from_symbols(
        &self,
        changed: &[crate::ir::Symbol],
        _lang: LanguageMode,
        opts: &ImpactOptions,
    ) -> anyhow::Result<ImpactOutput> {
        let (scope, dir_override) = cache::scope_from_env();
        let mut db = cache::open(scope, dir_override.as_deref())?;
        let st = cache::stats(&db.conn)?;
        if st.symbols == 0 {
            log::info!("cache: empty → build all");
            cache::build_all(&mut db.conn)?;
        }
        let (index, refs) = cache::load_graph(&db.conn)?;
        let out = compute_impact(changed, &index, &refs, opts);
        Ok(out)
    }
}
