use serde::{Serialize, Deserialize};

/// Kind of dependency edge in data/control flow graph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DependencyKind {
    /// Data dependence (definition → use).
    Data,
    /// Control dependence (predicate → statement).
    Control,
}

/// Node in the data flow graph, representing a definition or use.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DfgNode {
    /// Unique identifier for the node.
    pub id: String,
    /// Symbol or variable name.
    pub name: String,
    /// File path where this node is located.
    pub file: String,
    /// Line number of the node (1-based).
    pub line: u32,
}

/// Edge in the data/control flow graph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DfgEdge {
    /// Source node ID.
    pub from: String,
    /// Target node ID.
    pub to: String,
    /// Type of dependency.
    pub kind: DependencyKind,
}

/// Representation of a Data Flow Graph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DataFlowGraph {
    /// All definition/use nodes.
    pub nodes: Vec<DfgNode>,
    /// All dependency edges.
    pub edges: Vec<DfgEdge>,
}

/// Trait for constructing a Data Flow Graph from source code.
pub trait DfgBuilder {
    /// Build a DFG for the given file path and its source content.
    fn build(path: &str, source: &str) -> DataFlowGraph;
}

/// Default Rust DFG builder (stub implementation).
pub struct RustDfgBuilder;

impl DfgBuilder for RustDfgBuilder {
    fn build(path: &str, source: &str) -> DataFlowGraph {
        use std::collections::{HashMap, HashSet};
        // Simple Rust DFG: definitions (let) and uses of defined vars
        let mut nodes: Vec<DfgNode> = Vec::new();
        let mut edges: Vec<DfgEdge> = Vec::new();
        let mut def_ids_by_name: HashMap<String, Vec<String>> = HashMap::new();
        let mut seen_node_ids: HashSet<String> = HashSet::new();
        // Reserved keywords to skip as uses
        let reserved = ["let","mut","fn","pub","self","super","crate","if","else","match","for","while","loop","return","use","struct","enum","trait","impl","mod","as","in","true","false"];
        // First pass: collect definitions
        for (idx, line) in source.lines().enumerate() {
            let line_no = (idx + 1) as u32;
            let trimmed = line.trim_start();
            if let Some(rest) = trimmed.strip_prefix("let ") {
                // extract variable name
                if let Some(name) = rest.split(|c: char| !c.is_alphanumeric() && c != '_').next() {
                    if !name.is_empty() {
                        let node_id = format!("{}:def:{}:{}", path, name, line_no);
                        if seen_node_ids.insert(node_id.clone()) {
                            nodes.push(DfgNode { id: node_id.clone(), name: name.to_string(), file: path.to_string(), line: line_no });
                        }
                        def_ids_by_name.entry(name.to_string()).or_default().push(node_id);
                    }
                }
            }
        }
        // Second pass: collect uses and link to defs
        for (idx, line) in source.lines().enumerate() {
            let line_no = (idx + 1) as u32;
            for token in line.split(|c: char| !c.is_alphanumeric() && c != '_') {
                if token.is_empty() || reserved.contains(&token) { continue; }
                if let Some(def_ids) = def_ids_by_name.get(token) {
                    let node_id = format!("{}:use:{}:{}", path, token, line_no);
                    if seen_node_ids.insert(node_id.clone()) {
                        nodes.push(DfgNode { id: node_id.clone(), name: token.to_string(), file: path.to_string(), line: line_no });
                    }
                    for def_id in def_ids {
                        edges.push(DfgEdge { from: def_id.clone(), to: node_id.clone(), kind: DependencyKind::Data });
                    }
                }
            }
        }
        DataFlowGraph { nodes, edges }
    }
}

use crate::ir::reference::Reference;

/// Builder for Program Dependence Graphs by merging DFG and call graph.
pub struct PdgBuilder;

impl PdgBuilder {
    /// Build a PDG by combining the data/control flow graph and call references.
    pub fn build(dfg: &DataFlowGraph, refs: &[Reference]) -> DataFlowGraph {
        // Start with existing DFG
        let mut pdg = dfg.clone();
        // Add call edges as data dependencies
        for r in refs {
            pdg.edges.push(DfgEdge {
                from: r.from.0.clone(),
                to: r.to.0.clone(),
                kind: DependencyKind::Data,
            });
        }
        pdg
    }
}

#[cfg(test)]
mod pdg_tests {
    use super::*;
    use crate::ir::SymbolId;
    use crate::ir::reference::Reference;

    #[test]
    fn build_empty_pdg() {
        let dfg = DataFlowGraph { nodes: Vec::new(), edges: Vec::new() };
        let refs: Vec<Reference> = Vec::new();
        let pdg = PdgBuilder::build(&dfg, &refs);
        assert!(pdg.nodes.is_empty());
        assert!(pdg.edges.is_empty());
    }

    #[test]
    fn build_pdg_with_refs() {
        // Prepare a dummy reference
        let from_id = SymbolId::new("rust", "f.rs", &crate::ir::SymbolKind::Function, "foo", 1);
        let to_id = SymbolId::new("rust", "f.rs", &crate::ir::SymbolKind::Function, "bar", 5);
        let r = Reference { from: from_id.clone(), to: to_id.clone(), kind: crate::ir::reference::RefKind::Call, file: "f.rs".to_string(), line: 10 };
        let dfg = DataFlowGraph { nodes: Vec::new(), edges: Vec::new() };
        let pdg = PdgBuilder::build(&dfg, &[r]);
        assert_eq!(pdg.edges.len(), 1);
        let e = &pdg.edges[0];
        assert_eq!(&e.from, &from_id.0);
        assert_eq!(&e.to, &to_id.0);
        assert_eq!(e.kind, DependencyKind::Data);
    }
}

// Unit tests for DFG
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_empty_graph() {
        let dfg = RustDfgBuilder::build("foo.rs", "");
        assert!(dfg.nodes.is_empty(), "expected no nodes in empty source");
        assert!(dfg.edges.is_empty(), "expected no edges in empty source");
    }
}
