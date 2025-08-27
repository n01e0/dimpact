use dimpact::engine::lsp::{CapabilityMatrix, decide_changed_strategy, decide_impact_strategy, ChangedStrategy, ImpactStrategy};

#[test]
fn strategy_prefers_document_symbol() {
    let caps = CapabilityMatrix { document_symbol: true, workspace_symbol: false, call_hierarchy: false, references: false, definition: false };
    assert_eq!(decide_changed_strategy(&caps), ChangedStrategy::DocumentSymbol);
}

#[test]
fn strategy_falls_back_workspace_symbol() {
    let caps = CapabilityMatrix { document_symbol: false, workspace_symbol: true, call_hierarchy: false, references: false, definition: false };
    assert_eq!(decide_changed_strategy(&caps), ChangedStrategy::WorkspaceSymbol);
}

#[test]
fn strategy_ts_when_no_symbol_caps() {
    let caps = CapabilityMatrix::default();
    assert_eq!(decide_changed_strategy(&caps), ChangedStrategy::TsFallback);
}

#[test]
fn impact_prefers_call_hierarchy() {
    let caps = CapabilityMatrix { call_hierarchy: true, ..Default::default() };
    assert_eq!(decide_impact_strategy(&caps), ImpactStrategy::CallHierarchy);
}

#[test]
fn impact_uses_references_if_available() {
    let caps = CapabilityMatrix { references: true, ..Default::default() };
    assert_eq!(decide_impact_strategy(&caps), ImpactStrategy::References);
    let caps = CapabilityMatrix { definition: true, ..Default::default() };
    assert_eq!(decide_impact_strategy(&caps), ImpactStrategy::References);
}

