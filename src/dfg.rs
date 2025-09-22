use serde::{Deserialize, Serialize};

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
        // Initialize DFG containers
        let mut nodes: Vec<DfgNode> = Vec::new();
        let mut edges: Vec<DfgEdge> = Vec::new();
        // Map variable name -> definition node IDs
        let mut def_ids_by_name: HashMap<String, Vec<String>> = HashMap::new();
        // Map variable name -> set of line numbers where it's defined
        let mut def_lines_by_name: HashMap<String, HashSet<u32>> = HashMap::new();
        let mut seen_node_ids: HashSet<String> = HashSet::new();
        // Interprocedural analysis: parameters and assignments via AST
        {
            // Parse Rust AST to extract definitions
            let mut parser = tree_sitter::Parser::new();
            let lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
            parser.set_language(&lang).expect("set language");
            if let Some(tree) = parser.parse(source, None) {
                let offs = crate::languages::util::line_offsets(source);
                let mut cursor = tree.root_node().walk();
                let mut stack = vec![tree.root_node()];
                while let Some(node) = stack.pop() {
                    // Traverse children
                    for child in node.named_children(&mut cursor) {
                        stack.push(child);
                    }
                    // Function parameters: treat as definitions
                    if node.kind() == "function_item"
                        && let Some(params_node) = node.child_by_field_name("parameters")
                    {
                        for param in params_node.named_children(&mut cursor) {
                            if param.kind() == "parameter"
                                && let Some(pat) = param.child_by_field_name("pattern")
                            {
                                let name = pat.utf8_text(source.as_bytes()).unwrap_or("");
                                if !name.is_empty() {
                                    let sl = crate::languages::util::byte_to_line(
                                        &offs,
                                        pat.start_byte(),
                                    );
                                    let node_id = format!("{}:def:{}:{}", path, name, sl);
                                    if seen_node_ids.insert(node_id.clone()) {
                                        nodes.push(DfgNode {
                                            id: node_id.clone(),
                                            name: name.to_string(),
                                            file: path.to_string(),
                                            line: sl,
                                        });
                                    }
                                    def_ids_by_name
                                        .entry(name.to_string())
                                        .or_default()
                                        .push(node_id);
                                }
                            }
                        }
                    }
                    // Assignment expressions: x = ... as definitions
                    if node.kind() == "assignment_expression"
                        && let Some(lhs) = node.child_by_field_name("left")
                        && lhs.kind() == "identifier"
                    {
                        let name = lhs.utf8_text(source.as_bytes()).unwrap_or("");
                        if !name.is_empty() {
                            let sl = crate::languages::util::byte_to_line(&offs, lhs.start_byte());
                            let node_id = format!("{}:def:{}:{}", path, name, sl);
                            if seen_node_ids.insert(node_id.clone()) {
                                nodes.push(DfgNode {
                                    id: node_id.clone(),
                                    name: name.to_string(),
                                    file: path.to_string(),
                                    line: sl,
                                });
                            }
                            def_ids_by_name
                                .entry(name.to_string())
                                .or_default()
                                .push(node_id);
                        }
                    }
                }
            }
        }
        // Definitions (let) and uses of defined vars
        // Reserved keywords to skip as uses
        let reserved = [
            "let", "mut", "fn", "pub", "self", "super", "crate", "if", "else", "match", "for",
            "while", "loop", "return", "use", "struct", "enum", "trait", "impl", "mod", "as", "in",
            "true", "false",
        ];
        // First pass: collect definitions
        for (idx, line) in source.lines().enumerate() {
            let line_no = (idx + 1) as u32;
            let trimmed = line.trim_start();
            if let Some(rest) = trimmed.strip_prefix("let ") {
                // extract variable name
                if let Some(name) = rest
                    .split(|c: char| !c.is_alphanumeric() && c != '_')
                    .next()
                    && !name.is_empty()
                {
                    let node_id = format!("{}:def:{}:{}", path, name, line_no);
                    if seen_node_ids.insert(node_id.clone()) {
                        nodes.push(DfgNode {
                            id: node_id.clone(),
                            name: name.to_string(),
                            file: path.to_string(),
                            line: line_no,
                        });
                    }
                    def_ids_by_name
                        .entry(name.to_string())
                        .or_default()
                        .push(node_id.clone());
                    // Track definition line
                    def_lines_by_name
                        .entry(name.to_string())
                        .or_default()
                        .insert(line_no);
                }
            }
        }
        // Second pass: collect uses and link to defs
        for (idx, line) in source.lines().enumerate() {
            let line_no = (idx + 1) as u32;
            for token in line.split(|c: char| !c.is_alphanumeric() && c != '_') {
                if token.is_empty() || reserved.contains(&token) {
                    continue;
                }
                // Skip uses on same line as definition
                if def_lines_by_name
                    .get(token)
                    .is_some_and(|lines| lines.contains(&line_no))
                {
                    continue;
                }
                if let Some(def_ids) = def_ids_by_name.get(token) {
                    let node_id = format!("{}:use:{}:{}", path, token, line_no);
                    if seen_node_ids.insert(node_id.clone()) {
                        nodes.push(DfgNode {
                            id: node_id.clone(),
                            name: token.to_string(),
                            file: path.to_string(),
                            line: line_no,
                        });
                    }
                    for def_id in def_ids {
                        edges.push(DfgEdge {
                            from: def_id.clone(),
                            to: node_id.clone(),
                            kind: DependencyKind::Data,
                        });
                    }
                }
            }
        }
        // Now extract control dependencies via Tree-Sitter control queries
        // Load Rust spec and compile control query
        let spec = crate::ts_core::load_rust_spec();
        let compiled =
            crate::ts_core::compile_queries_rust(&spec).expect("compile rust control queries");
        // Query for control nodes
        if let Some(ctrl_q) = &compiled.control {
            let runner = crate::ts_core::QueryRunner::new_rust();
            let offs = crate::languages::util::line_offsets(source);
            // number of data nodes before control nodes are added
            let data_node_count = nodes.len();
            for caps in runner.run_captures(source, ctrl_q) {
                if let Some(c0) = caps.first() {
                    let start_ln = crate::languages::util::byte_to_line(&offs, c0.start);
                    let end_ln =
                        crate::languages::util::byte_to_line(&offs, c0.end.saturating_sub(1));
                    let ctrl_id = format!("{}:ctrl:{}:{}", path, start_ln, end_ln);
                    // add control node if new
                    if seen_node_ids.insert(ctrl_id.clone()) {
                        nodes.push(DfgNode {
                            id: ctrl_id.clone(),
                            name: "control".to_string(),
                            file: path.to_string(),
                            line: start_ln,
                        });
                    }
                    // add control edges to existing data nodes within block
                    for nd in &nodes[..data_node_count] {
                        if nd.line >= start_ln && nd.line <= end_ln {
                            edges.push(DfgEdge {
                                from: ctrl_id.clone(),
                                to: nd.id.clone(),
                                kind: DependencyKind::Control,
                            });
                        }
                    }
                }
            }
        }
        DataFlowGraph { nodes, edges }
    }
}

/// Ruby Data Flow Graph builder: supports params, assignments, return, and control dependencies.
pub struct RubyDfgBuilder;

impl DfgBuilder for RubyDfgBuilder {
    fn build(path: &str, source: &str) -> DataFlowGraph {
        use regex::Regex;
        use std::collections::{HashMap, HashSet};
        // Initialize DFG containers
        let mut nodes: Vec<DfgNode> = Vec::new();
        let mut edges: Vec<DfgEdge> = Vec::new();
        let mut def_ids_by_name: HashMap<String, Vec<String>> = HashMap::new();
        let mut def_lines_by_name: HashMap<String, HashSet<u32>> = HashMap::new();
        let mut seen_node_ids: HashSet<String> = HashSet::new();
        let reserved = ["if", "else", "end", "return", "def", "class", "module"];
        // Parse parameters via regex
        let fn_re = Regex::new(r"def\s+\w+\s*\(([^)]*)\)").unwrap();
        for (idx, line) in source.lines().enumerate() {
            let line_no = (idx + 1) as u32;
            if let Some(cap) = fn_re.captures(line) {
                let params = cap.get(1).unwrap().as_str();
                for p in params.split(',') {
                    let name = p
                        .trim()
                        .strip_prefix("mut ")
                        .unwrap_or(p)
                        .split(':')
                        .next()
                        .unwrap_or("");
                    if !name.is_empty() {
                        let node_id = format!("{}:def:{}:{}", path, name, line_no);
                        if seen_node_ids.insert(node_id.clone()) {
                            nodes.push(DfgNode {
                                id: node_id.clone(),
                                name: name.to_string(),
                                file: path.to_string(),
                                line: line_no,
                            });
                        }
                        def_ids_by_name
                            .entry(name.to_string())
                            .or_default()
                            .push(node_id.clone());
                        def_lines_by_name
                            .entry(name.to_string())
                            .or_default()
                            .insert(line_no);
                    }
                }
            }
        }
        // Capture assignments and their RHS uses
        let assign_re =
            Regex::new(r"^\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*=\s*([a-zA-Z_][a-zA-Z0-9_]*)").unwrap();
        for (idx, line) in source.lines().enumerate() {
            let line_no = (idx + 1) as u32;
            if let Some(cap) = assign_re.captures(line) {
                let lhs = cap.get(1).unwrap().as_str();
                let rhs = cap.get(2).unwrap().as_str();
                // LHS definition
                let def_id = format!("{}:def:{}:{}", path, lhs, line_no);
                if seen_node_ids.insert(def_id.clone()) {
                    nodes.push(DfgNode {
                        id: def_id.clone(),
                        name: lhs.to_string(),
                        file: path.to_string(),
                        line: line_no,
                    });
                }
                def_ids_by_name
                    .entry(lhs.to_string())
                    .or_default()
                    .push(def_id.clone());
                def_lines_by_name
                    .entry(lhs.to_string())
                    .or_default()
                    .insert(line_no);
                // RHS use dependency
                if let Some(def_ids) = def_ids_by_name.get(rhs) {
                    let use_id = format!("{}:use:{}:{}", path, rhs, line_no);
                    if seen_node_ids.insert(use_id.clone()) {
                        nodes.push(DfgNode {
                            id: use_id.clone(),
                            name: rhs.to_string(),
                            file: path.to_string(),
                            line: line_no,
                        });
                    }
                    for def_id in def_ids {
                        edges.push(DfgEdge {
                            from: def_id.clone(),
                            to: use_id.clone(),
                            kind: DependencyKind::Data,
                        });
                    }
                }
            }
        }
        // Capture return uses
        let return_re = Regex::new(r"return\s+([a-zA-Z_][a-zA-Z0-9_]*)").unwrap();
        for (idx, line) in source.lines().enumerate() {
            let line_no = (idx + 1) as u32;
            if let Some(cap) = return_re.captures(line) {
                let name = cap.get(1).unwrap().as_str();
                let node_id = format!("{}:use:{}:{}", path, name, line_no);
                if !def_lines_by_name
                    .get(name)
                    .is_some_and(|s| s.contains(&line_no))
                {
                    if seen_node_ids.insert(node_id.clone()) {
                        nodes.push(DfgNode {
                            id: node_id.clone(),
                            name: name.to_string(),
                            file: path.to_string(),
                            line: line_no,
                        });
                    }
                    if let Some(def_ids) = def_ids_by_name.get(name) {
                        for def_id in def_ids {
                            edges.push(DfgEdge {
                                from: def_id.clone(),
                                to: node_id.clone(),
                                kind: DependencyKind::Data,
                            });
                        }
                    }
                }
            }
        }
        // Capture general uses beyond return (assignments, method calls, etc.)
        for (idx, line) in source.lines().enumerate() {
            let line_no = (idx + 1) as u32;
            for token in line.split(|c: char| !c.is_alphanumeric() && c != '_') {
                if token.is_empty() || reserved.contains(&token) {
                    continue;
                }
                // Skip if defined on this line
                if def_lines_by_name
                    .get(token)
                    .is_some_and(|s| s.contains(&line_no))
                {
                    continue;
                }
                if let Some(def_ids) = def_ids_by_name.get(token) {
                    let use_id = format!("{}:use:{}:{}", path, token, line_no);
                    if seen_node_ids.insert(use_id.clone()) {
                        nodes.push(DfgNode {
                            id: use_id.clone(),
                            name: token.to_string(),
                            file: path.to_string(),
                            line: line_no,
                        });
                    }
                    for def_id in def_ids {
                        edges.push(DfgEdge {
                            from: def_id.clone(),
                            to: use_id.clone(),
                            kind: DependencyKind::Data,
                        });
                    }
                }
            }
        }
        // Generic uses: catch variable usages beyond return
        for (idx, line) in source.lines().enumerate() {
            let line_no = (idx + 1) as u32;
            for token in line.split(|c: char| !c.is_alphanumeric() && c != '_') {
                if token.is_empty() || reserved.contains(&token) {
                    continue;
                }
                // Skip if defined on this line
                if def_lines_by_name
                    .get(token)
                    .is_some_and(|s| s.contains(&line_no))
                {
                    continue;
                }
                if let Some(def_ids) = def_ids_by_name.get(token) {
                    let use_id = format!("{}:use:{}:{}", path, token, line_no);
                    if seen_node_ids.insert(use_id.clone()) {
                        nodes.push(DfgNode {
                            id: use_id.clone(),
                            name: token.to_string(),
                            file: path.to_string(),
                            line: line_no,
                        });
                    }
                    for def_id in def_ids {
                        edges.push(DfgEdge {
                            from: def_id.clone(),
                            to: use_id.clone(),
                            kind: DependencyKind::Data,
                        });
                    }
                }
            }
        }
        // Now extract control dependencies via Tree-Sitter
        let spec = crate::ts_core::load_ruby_spec();
        let compiled =
            crate::ts_core::compile_queries_ruby(&spec).expect("compile ruby control queries");
        if let Some(ctrl_q) = &compiled.control {
            let runner = crate::ts_core::QueryRunner::new_ruby();
            let offs = crate::languages::util::line_offsets(source);
            let data_count = nodes.len();
            for caps in runner.run_captures(source, ctrl_q) {
                if let Some(c0) = caps.first() {
                    let start_ln = crate::languages::util::byte_to_line(&offs, c0.start);
                    let end_ln =
                        crate::languages::util::byte_to_line(&offs, c0.end.saturating_sub(1));
                    let ctrl_id = format!("{}:ctrl:{}:{}", path, start_ln, end_ln);
                    if seen_node_ids.insert(ctrl_id.clone()) {
                        nodes.push(DfgNode {
                            id: ctrl_id.clone(),
                            name: "control".to_string(),
                            file: path.to_string(),
                            line: start_ln,
                        });
                    }
                    for nd in &nodes[..data_count] {
                        if nd.line >= start_ln && nd.line <= end_ln {
                            edges.push(DfgEdge {
                                from: ctrl_id.clone(),
                                to: nd.id.clone(),
                                kind: DependencyKind::Control,
                            });
                        }
                    }
                }
            }
        }
        DataFlowGraph { nodes, edges }
    }
}
// Insert at PdgBuilder
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

    /// Augment PDG with symbolic propagation bridges.
    /// Heuristics:
    /// - Connect variable uses at call sites to callee symbols.
    /// - Connect callee symbols to variable definitions at call sites (returned value captured).
    /// - Connect function/method symbols to all variable def/use nodes within their range.
    pub fn augment_symbolic_propagation(
        pdg: &mut DataFlowGraph,
        refs: &[Reference],
        index: &crate::ir::reference::SymbolIndex,
    ) {
        use crate::ir::SymbolKind;
        // Index DFG nodes by (file,line)
        let mut uses_by_loc: std::collections::HashMap<(String, u32), Vec<String>> =
            std::collections::HashMap::new();
        let mut defs_by_loc: std::collections::HashMap<(String, u32), Vec<String>> =
            std::collections::HashMap::new();
        for n in &pdg.nodes {
            let key = (n.file.clone(), n.line);
            if n.id.contains(":use:") {
                uses_by_loc
                    .entry(key.clone())
                    .or_default()
                    .push(n.id.clone());
            }
            if n.id.contains(":def:") {
                defs_by_loc.entry(key).or_default().push(n.id.clone());
            }
        }
        // 1) Call-site bridges
        for r in refs {
            let key = (r.file.clone(), r.line);
            if let Some(uses) = uses_by_loc.get(&key) {
                for u in uses {
                    pdg.edges.push(DfgEdge {
                        from: u.clone(),
                        to: r.to.0.clone(),
                        kind: DependencyKind::Data,
                    });
                }
            }
            if let Some(defs) = defs_by_loc.get(&key) {
                for d in defs {
                    pdg.edges.push(DfgEdge {
                        from: r.to.0.clone(),
                        to: d.clone(),
                        kind: DependencyKind::Data,
                    });
                }
            }
        }
        // 2) Intra-function bridges: symbol -> all DFG nodes within its span
        for s in &index.symbols {
            if !matches!(s.kind, SymbolKind::Function | SymbolKind::Method) {
                continue;
            }
            for n in &pdg.nodes {
                if n.file == s.file && n.line >= s.range.start_line && n.line <= s.range.end_line {
                    pdg.edges.push(DfgEdge {
                        from: s.id.0.clone(),
                        to: n.id.clone(),
                        kind: DependencyKind::Data,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod pdg_tests {
    use super::*;
    use crate::ir::SymbolId;
    use crate::ir::reference::Reference;

    #[test]
    fn build_empty_pdg() {
        let dfg = DataFlowGraph {
            nodes: Vec::new(),
            edges: Vec::new(),
        };
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
        let r = Reference {
            from: from_id.clone(),
            to: to_id.clone(),
            kind: crate::ir::reference::RefKind::Call,
            file: "f.rs".to_string(),
            line: 10,
        };
        let dfg = DataFlowGraph {
            nodes: Vec::new(),
            edges: Vec::new(),
        };
        let pdg = PdgBuilder::build(&dfg, &[r]);
        assert_eq!(pdg.edges.len(), 1);
        let e = &pdg.edges[0];
        assert_eq!(&e.from, &from_id.0);
        assert_eq!(&e.to, &to_id.0);
        assert_eq!(e.kind, DependencyKind::Data);
    }

    #[test]
    fn pdg_includes_control_and_call_edges() {
        // Prepare a simple DFG with one let and one use, plus a call ref
        let mut dfg = DataFlowGraph {
            nodes: Vec::new(),
            edges: Vec::new(),
        };
        // Add a control node
        dfg.nodes.push(DfgNode {
            id: "f.rs:ctrl:2:4".to_string(),
            name: "control".to_string(),
            file: "f.rs".to_string(),
            line: 2,
        });
        // Add a def/use node
        dfg.nodes.push(DfgNode {
            id: "f.rs:def: x:2".to_string(),
            name: "x".to_string(),
            file: "f.rs".to_string(),
            line: 2,
        });
        // No edges in DFG
        let ref_sym = Reference {
            from: crate::ir::SymbolId("rust:f.rs:fn:foo:1".to_string()),
            to: crate::ir::SymbolId("rust:f.rs:fn:bar:1".to_string()),
            kind: crate::ir::reference::RefKind::Call,
            file: "f.rs".to_string(),
            line: 10,
        };
        let pdg = PdgBuilder::build(&dfg, &[ref_sym.clone()]);
        // Check call edge added
        assert!(pdg.edges.iter().any(|e| e.from == ref_sym.from.0
            && e.to == ref_sym.to.0
            && e.kind == DependencyKind::Data));
        // Check control edge remains
        // Control edges were not in DFG; here we only check call edges merge
        // The control node should still be present
        assert!(pdg.nodes.iter().any(|n| n.id == "f.rs:ctrl:2:4"));
    }
}

// Unit tests for DFG
#[cfg(test)]
mod tests {
    use super::*;
    use crate::dfg::RubyDfgBuilder;

    #[test]
    fn build_empty_graph() {
        let dfg = RustDfgBuilder::build("foo.rs", "");
        assert!(dfg.nodes.is_empty(), "expected no nodes in empty source");
        assert!(dfg.edges.is_empty(), "expected no edges in empty source");
    }

    #[test]
    fn build_assignment_definitions() {
        let src = r#"
        let x = 1;
        x = 42;
        let y = x;
        "#;
        let dfg = RustDfgBuilder::build("f.rs", src);
        // Expect multiple definitions of x (initial let and assignment)
        let defs: Vec<_> = dfg
            .nodes
            .iter()
            .filter(|n| n.name == "x" && n.id.contains(":def:"))
            .collect();
        assert!(
            defs.len() >= 2,
            "expected >=2 definitions of x, got {}",
            defs.len()
        );
        // Expect use edges for x
        let uses: Vec<_> = dfg
            .edges
            .iter()
            .filter(|e| e.kind == DependencyKind::Data && e.to.contains(":use:x:"))
            .collect();
        assert!(
            !uses.is_empty(),
            "expected data dependency edges for x usage"
        );
    }

    #[test]
    fn build_return_dependency() {
        let src = r#"
        fn foo() {
            let a = compute();
            return a;
        }
        "#;
        let dfg = RustDfgBuilder::build("f.rs", src);
        // Expect definition for a and use of a in return
        let def_ids: Vec<&String> = dfg
            .nodes
            .iter()
            .filter(|n| n.name == "a" && n.id.contains(":def:"))
            .map(|n| &n.id)
            .collect();
        assert_eq!(
            def_ids.len(),
            1,
            "expected one definition of a, got {}",
            def_ids.len()
        );
        let use_ids: Vec<&String> = dfg
            .nodes
            .iter()
            .filter(|n| n.name == "a" && n.id.contains(":use:"))
            .map(|n| &n.id)
            .collect();
        assert_eq!(
            use_ids.len(),
            1,
            "expected one use of a, got {}",
            use_ids.len()
        );
        // Edge from def a to use a
        let edge_exists = dfg.edges.iter().any(|e| {
            e.kind == DependencyKind::Data && e.from == *def_ids[0] && e.to == *use_ids[0]
        });
        assert!(
            edge_exists,
            "expected data edge from '{}' to '{}'",
            def_ids[0], use_ids[0]
        );
    }

    #[test]
    fn build_control_dependencies() {
        let src = r#"
        let x = 1;
        if x > 0 {
            let y = x;
            let z = y;
        }
        "#;
        let dfg = RustDfgBuilder::build("f.rs", src);
        // Expect at least one control node and control edges
        let ctrl_nodes: Vec<_> = dfg
            .nodes
            .iter()
            .filter(|n| n.id.contains(":ctrl:"))
            .collect();
        assert!(!ctrl_nodes.is_empty(), "expected control nodes");
        let ctrl_edges: Vec<_> = dfg
            .edges
            .iter()
            .filter(|e| e.kind == DependencyKind::Control)
            .collect();
        assert!(!ctrl_edges.is_empty(), "expected control edges");
        // All control edges should originate from control nodes
        for edge in ctrl_edges {
            assert!(
                edge.from.contains(":ctrl:"),
                "control edge should originate from control node"
            );
        }
    }

    #[test]
    fn build_ruby_dfg_simple() {
        let src = r#"
        def foo(a)
            b = a
            if b > 0
                c = b
            end
            return c
        end
        "#;
        let path = "test.rb";
        let dfg = RubyDfgBuilder::build(path, src);
        // Parameter 'a'
        assert!(
            dfg.nodes
                .iter()
                .any(|n| n.name == "a" && n.id.starts_with("test.rb:def:a:")),
            "expected definition node for 'a'"
        );
        // Definition 'b' and 'c'
        assert!(
            dfg.nodes.iter().any(|n| n.id.contains(":def:b:")),
            "expected def node for 'b'"
        );
        assert!(
            dfg.nodes.iter().any(|n| n.id.contains(":def:c:")),
            "expected def node for 'c'"
        );
        // Data dependencies for uses
        assert!(
            dfg.edges
                .iter()
                .any(|e| e.from.contains(":def:a:") && e.to.contains(":use:a:")),
            "expected data edge from def:a to use:a"
        );
        assert!(
            dfg.edges
                .iter()
                .any(|e| e.from.contains(":def:b:") && e.to.contains(":use:b:")),
            "expected data edge from def:b to use:b"
        );
        // Control dependency for if block
        assert!(
            dfg.nodes.iter().any(|n| n.id.contains(":ctrl:")),
            "expected control node"
        );
        assert!(
            dfg.edges.iter().any(|e| e.kind == DependencyKind::Control),
            "expected control edges"
        );
    }
}
