use serde::{Deserialize, Serialize};

/// Kind of dependency edge in data/control flow graph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
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
        // SSA-like ordered definition records: variable -> [(line, def_id)]
        let mut def_records_by_name: HashMap<String, Vec<(u32, String)>> = HashMap::new();
        let mut param_def_records_by_name: HashMap<String, Vec<(u32, String)>> = HashMap::new();
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
                                        def_records_by_name
                                            .entry(name.to_string())
                                            .or_default()
                                            .push((sl, node_id.clone()));
                                        param_def_records_by_name
                                            .entry(name.to_string())
                                            .or_default()
                                            .push((sl, node_id.clone()));
                                    }
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
                                def_records_by_name
                                    .entry(name.to_string())
                                    .or_default()
                                    .push((sl, node_id.clone()));
                            }
                            def_lines_by_name
                                .entry(name.to_string())
                                .or_default()
                                .insert(sl);
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
        // First pass: collect let-definitions (including `let mut x = ...`).
        let re_let_def =
            regex::Regex::new(r"^\s*let(?:\s+mut)?\s+([A-Za-z_][A-Za-z0-9_]*)\b").unwrap();
        for (idx, line) in source.lines().enumerate() {
            let line_no = (idx + 1) as u32;
            let Some(cap) = re_let_def.captures(line) else {
                continue;
            };
            let Some(name) = cap.get(1).map(|m| m.as_str()) else {
                continue;
            };
            let node_id = format!("{}:def:{}:{}", path, name, line_no);
            if seen_node_ids.insert(node_id.clone()) {
                nodes.push(DfgNode {
                    id: node_id.clone(),
                    name: name.to_string(),
                    file: path.to_string(),
                    line: line_no,
                });
                def_records_by_name
                    .entry(name.to_string())
                    .or_default()
                    .push((line_no, node_id.clone()));
            }
            // Track definition line
            def_lines_by_name
                .entry(name.to_string())
                .or_default()
                .insert(line_no);
        }
        for records in def_records_by_name.values_mut() {
            records.sort_by_key(|(line, _)| *line);
        }
        let control_ranges = collect_control_ranges_rust(source);

        // Lightweight alias propagation: a = b; / let a = b;
        // Add conservative data edge def(b) -> def(a) at assignment lines.
        let re_alias_let = regex::Regex::new(
            r"^\s*let(?:\s+mut)?\s+([A-Za-z_][A-Za-z0-9_]*)(?:\s*:[^=]+)?\s*=\s*([A-Za-z_][A-Za-z0-9_]*)\b",
        )
        .unwrap();
        let re_alias_assign =
            regex::Regex::new(r"^\s*([A-Za-z_][A-Za-z0-9_]*)\s*=\s*([A-Za-z_][A-Za-z0-9_]*)\b")
                .unwrap();
        for (idx, line) in source.lines().enumerate() {
            let line_no = (idx + 1) as u32;
            let pair = if let Some(cap) = re_alias_let.captures(line) {
                Some((
                    cap.get(1).unwrap().as_str().to_string(),
                    cap.get(2).unwrap().as_str().to_string(),
                ))
            } else {
                re_alias_assign.captures(line).map(|cap| {
                    (
                        cap.get(1).unwrap().as_str().to_string(),
                        cap.get(2).unwrap().as_str().to_string(),
                    )
                })
            };
            let Some((lhs, rhs)) = pair else {
                continue;
            };
            if lhs == rhs {
                continue;
            }
            let Some(lhs_def_id) = def_records_by_name.get(&lhs).and_then(|recs| {
                recs.iter()
                    .rev()
                    .find(|(ln, _)| *ln == line_no)
                    .map(|(_, id)| id.clone())
            }) else {
                continue;
            };
            let rhs_defs =
                reaching_def_ids_ssa_like(&rhs, line_no, &def_records_by_name, &control_ranges);
            for rhs_def_id in rhs_defs {
                edges.push(DfgEdge {
                    from: rhs_def_id,
                    to: lhs_def_id.clone(),
                    kind: DependencyKind::Data,
                });
            }
        }

        // Second pass: collect uses and link to reaching defs (SSA-like)
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
                let reaching = reaching_def_ids_ssa_like_with_same_line_params(
                    token,
                    line_no,
                    &def_records_by_name,
                    &param_def_records_by_name,
                    &control_ranges,
                );
                if !reaching.is_empty() {
                    let node_id = format!("{}:use:{}:{}", path, token, line_no);
                    if seen_node_ids.insert(node_id.clone()) {
                        nodes.push(DfgNode {
                            id: node_id.clone(),
                            name: token.to_string(),
                            file: path.to_string(),
                            line: line_no,
                        });
                    }
                    for def_id in reaching {
                        edges.push(DfgEdge {
                            from: def_id,
                            to: node_id.clone(),
                            kind: DependencyKind::Data,
                        });
                    }
                }
            }
        }
        // Add control dependency nodes/edges from precomputed control ranges.
        let data_node_count = nodes.len();
        for (start_ln, end_ln) in &control_ranges {
            let ctrl_id = format!("{}:ctrl:{}:{}", path, start_ln, end_ln);
            if seen_node_ids.insert(ctrl_id.clone()) {
                nodes.push(DfgNode {
                    id: ctrl_id.clone(),
                    name: "control".to_string(),
                    file: path.to_string(),
                    line: *start_ln,
                });
            }
            for nd in &nodes[..data_node_count] {
                if nd.line >= *start_ln && nd.line <= *end_ln {
                    edges.push(DfgEdge {
                        from: ctrl_id.clone(),
                        to: nd.id.clone(),
                        kind: DependencyKind::Control,
                    });
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
        let mut def_records_by_name: HashMap<String, Vec<(u32, String)>> = HashMap::new();
        let mut def_lines_by_name: HashMap<String, HashSet<u32>> = HashMap::new();
        let mut seen_node_ids: HashSet<String> = HashSet::new();
        let reserved = [
            "if", "else", "elsif", "end", "return", "def", "class", "module",
        ];

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
                            def_records_by_name
                                .entry(name.to_string())
                                .or_default()
                                .push((line_no, node_id));
                        }
                        def_lines_by_name
                            .entry(name.to_string())
                            .or_default()
                            .insert(line_no);
                    }
                }
            }
        }

        for records in def_records_by_name.values_mut() {
            records.sort_by_key(|(line, _)| *line);
        }
        let control_ranges = collect_control_ranges_ruby(source);

        // Capture assignments and their RHS uses, then define LHS (SSA-like ordering).
        let assign_re =
            Regex::new(r"^\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*=\s*([a-zA-Z_][a-zA-Z0-9_]*)").unwrap();
        for (idx, line) in source.lines().enumerate() {
            let line_no = (idx + 1) as u32;
            if let Some(cap) = assign_re.captures(line) {
                let lhs = cap.get(1).unwrap().as_str();
                let rhs = cap.get(2).unwrap().as_str();

                let reaching_rhs =
                    reaching_def_ids_ssa_like(rhs, line_no, &def_records_by_name, &control_ranges);
                if !reaching_rhs.is_empty() {
                    let use_id = format!("{}:use:{}:{}", path, rhs, line_no);
                    if seen_node_ids.insert(use_id.clone()) {
                        nodes.push(DfgNode {
                            id: use_id.clone(),
                            name: rhs.to_string(),
                            file: path.to_string(),
                            line: line_no,
                        });
                    }
                    for def_id in &reaching_rhs {
                        edges.push(DfgEdge {
                            from: def_id.clone(),
                            to: use_id.clone(),
                            kind: DependencyKind::Data,
                        });
                    }
                }

                // LHS definition
                let def_id = format!("{}:def:{}:{}", path, lhs, line_no);
                if seen_node_ids.insert(def_id.clone()) {
                    nodes.push(DfgNode {
                        id: def_id.clone(),
                        name: lhs.to_string(),
                        file: path.to_string(),
                        line: line_no,
                    });
                    def_records_by_name
                        .entry(lhs.to_string())
                        .or_default()
                        .push((line_no, def_id.clone()));
                }
                def_lines_by_name
                    .entry(lhs.to_string())
                    .or_default()
                    .insert(line_no);

                // Lightweight alias propagation: a = b => def(b) -> def(a)
                if lhs != rhs {
                    for rhs_def_id in &reaching_rhs {
                        edges.push(DfgEdge {
                            from: rhs_def_id.clone(),
                            to: def_id.clone(),
                            kind: DependencyKind::Data,
                        });
                    }
                }
            }
        }

        for records in def_records_by_name.values_mut() {
            records.sort_by_key(|(line, _)| *line);
        }

        // Capture return uses
        let return_re = Regex::new(r"return\s+([a-zA-Z_][a-zA-Z0-9_]*)").unwrap();
        for (idx, line) in source.lines().enumerate() {
            let line_no = (idx + 1) as u32;
            if let Some(cap) = return_re.captures(line) {
                let name = cap.get(1).unwrap().as_str();
                if def_lines_by_name
                    .get(name)
                    .is_some_and(|s| s.contains(&line_no))
                {
                    continue;
                }
                let node_id = format!("{}:use:{}:{}", path, name, line_no);
                if seen_node_ids.insert(node_id.clone()) {
                    nodes.push(DfgNode {
                        id: node_id.clone(),
                        name: name.to_string(),
                        file: path.to_string(),
                        line: line_no,
                    });
                }
                let reaching =
                    reaching_def_ids_ssa_like(name, line_no, &def_records_by_name, &control_ranges);
                for def_id in reaching {
                    edges.push(DfgEdge {
                        from: def_id,
                        to: node_id.clone(),
                        kind: DependencyKind::Data,
                    });
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
                let reaching = reaching_def_ids_ssa_like(
                    token,
                    line_no,
                    &def_records_by_name,
                    &control_ranges,
                );
                if reaching.is_empty() {
                    continue;
                }
                let use_id = format!("{}:use:{}:{}", path, token, line_no);
                if seen_node_ids.insert(use_id.clone()) {
                    nodes.push(DfgNode {
                        id: use_id.clone(),
                        name: token.to_string(),
                        file: path.to_string(),
                        line: line_no,
                    });
                }
                for def_id in reaching {
                    edges.push(DfgEdge {
                        from: def_id,
                        to: use_id.clone(),
                        kind: DependencyKind::Data,
                    });
                }
            }
        }

        // Add control dependency nodes/edges from precomputed control ranges.
        let data_count = nodes.len();
        for (start_ln, end_ln) in &control_ranges {
            let ctrl_id = format!("{}:ctrl:{}:{}", path, start_ln, end_ln);
            if seen_node_ids.insert(ctrl_id.clone()) {
                nodes.push(DfgNode {
                    id: ctrl_id.clone(),
                    name: "control".to_string(),
                    file: path.to_string(),
                    line: *start_ln,
                });
            }
            for nd in &nodes[..data_count] {
                if nd.line >= *start_ln && nd.line <= *end_ln {
                    edges.push(DfgEdge {
                        from: ctrl_id.clone(),
                        to: nd.id.clone(),
                        kind: DependencyKind::Control,
                    });
                }
            }
        }

        DataFlowGraph { nodes, edges }
    }
}
// Insert at PdgBuilder
use crate::ir::reference::Reference;

/// Per-input impact slice inside a function summary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FunctionInputImpact {
    pub input_node_id: String,
    pub impacted_node_ids: Vec<String>,
}

/// Minimal function summary (input -> impact) derived from PDG data edges.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FunctionSummary {
    pub function_id: String,
    pub file: String,
    pub start_line: u32,
    pub end_line: u32,
    pub inputs: Vec<String>,
    pub flows: Vec<FunctionInputImpact>,
}

/// Builder for Program Dependence Graphs by merging DFG and call graph.
pub struct PdgBuilder;

fn push_unique_edge(
    pdg: &mut DataFlowGraph,
    seen_edges: &mut std::collections::HashSet<(String, String, DependencyKind)>,
    from: String,
    to: String,
    kind: DependencyKind,
) {
    if seen_edges.insert((from.clone(), to.clone(), kind.clone())) {
        pdg.edges.push(DfgEdge { from, to, kind });
    }
}

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

        let mut seen_edges: std::collections::HashSet<(String, String, DependencyKind)> = pdg
            .edges
            .iter()
            .map(|e| (e.from.clone(), e.to.clone(), e.kind.clone()))
            .collect();

        // Precompute minimal function summaries and map by callee symbol ID.
        let summaries = Self::build_function_summaries(pdg, index);
        let mut summary_by_fn: std::collections::HashMap<String, FunctionSummary> =
            std::collections::HashMap::new();
        for s in summaries {
            summary_by_fn.insert(s.function_id.clone(), s);
        }

        // 1) Call-site bridges + summary-connected inter-procedural bridges
        for r in refs {
            let key = (r.file.clone(), r.line);
            let callsite_uses = uses_by_loc.get(&key).cloned().unwrap_or_default();
            let callsite_defs = defs_by_loc.get(&key).cloned().unwrap_or_default();

            for u in &callsite_uses {
                push_unique_edge(
                    pdg,
                    &mut seen_edges,
                    u.clone(),
                    r.to.0.clone(),
                    DependencyKind::Data,
                );
            }
            for d in &callsite_defs {
                push_unique_edge(
                    pdg,
                    &mut seen_edges,
                    r.to.0.clone(),
                    d.clone(),
                    DependencyKind::Data,
                );
            }

            // Connect call-site propagation through callee summary (input -> impacted).
            if let Some(summary) = summary_by_fn.get(&r.to.0) {
                let mut summary_connected = false;
                for flow in &summary.flows {
                    if flow.impacted_node_ids.is_empty() {
                        continue;
                    }
                    summary_connected = true;

                    for u in &callsite_uses {
                        push_unique_edge(
                            pdg,
                            &mut seen_edges,
                            u.clone(),
                            flow.input_node_id.clone(),
                            DependencyKind::Data,
                        );
                    }
                    for impacted in &flow.impacted_node_ids {
                        for d in &callsite_defs {
                            push_unique_edge(
                                pdg,
                                &mut seen_edges,
                                impacted.clone(),
                                d.clone(),
                                DependencyKind::Data,
                            );
                        }
                    }
                }

                // Direct summary bridge at call-site: args/use -> assigned defs.
                if summary_connected {
                    for u in &callsite_uses {
                        for d in &callsite_defs {
                            push_unique_edge(
                                pdg,
                                &mut seen_edges,
                                u.clone(),
                                d.clone(),
                                DependencyKind::Data,
                            );
                        }
                    }
                }
            }
        }
        // 2) Intra-function bridges: symbol -> all DFG nodes within its span
        for s in &index.symbols {
            if !matches!(s.kind, SymbolKind::Function | SymbolKind::Method) {
                continue;
            }
            let in_span_node_ids: Vec<String> = pdg
                .nodes
                .iter()
                .filter(|n| {
                    n.file == s.file && n.line >= s.range.start_line && n.line <= s.range.end_line
                })
                .map(|n| n.id.clone())
                .collect();
            for node_id in in_span_node_ids {
                push_unique_edge(
                    pdg,
                    &mut seen_edges,
                    s.id.0.clone(),
                    node_id,
                    DependencyKind::Data,
                );
            }
        }
    }

    /// Build minimal function summaries (input -> impacted nodes) from PDG data edges.
    ///
    /// Input heuristic:
    /// - in-range def nodes with no incoming in-range data edges.
    ///
    /// Impact heuristic:
    /// - nodes reachable from each input via in-range data edges (excluding the input itself).
    pub fn build_function_summaries(
        pdg: &DataFlowGraph,
        index: &crate::ir::reference::SymbolIndex,
    ) -> Vec<FunctionSummary> {
        use crate::ir::SymbolKind;
        use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

        let mut nodes_by_id: HashMap<&str, &DfgNode> = HashMap::new();
        for n in &pdg.nodes {
            nodes_by_id.insert(n.id.as_str(), n);
        }

        let mut out_adj: HashMap<&str, Vec<&str>> = HashMap::new();
        let mut in_adj: HashMap<&str, Vec<&str>> = HashMap::new();
        for e in &pdg.edges {
            if e.kind != DependencyKind::Data {
                continue;
            }
            if nodes_by_id.contains_key(e.from.as_str()) && nodes_by_id.contains_key(e.to.as_str())
            {
                out_adj
                    .entry(e.from.as_str())
                    .or_default()
                    .push(e.to.as_str());
                in_adj
                    .entry(e.to.as_str())
                    .or_default()
                    .push(e.from.as_str());
            }
        }

        let mut summaries = Vec::new();
        for s in &index.symbols {
            if !matches!(s.kind, SymbolKind::Function | SymbolKind::Method) {
                continue;
            }

            let in_range_ids: BTreeSet<String> = pdg
                .nodes
                .iter()
                .filter(|n| {
                    n.file == s.file
                        && n.line >= s.range.start_line
                        && n.line <= s.range.end_line
                        && (n.id.contains(":def:") || n.id.contains(":use:"))
                })
                .map(|n| n.id.clone())
                .collect();
            if in_range_ids.is_empty() {
                continue;
            }
            let in_range_set: HashSet<&str> = in_range_ids.iter().map(|id| id.as_str()).collect();

            let mut param_like_inputs: Vec<String> = in_range_ids
                .iter()
                .filter(|id| id.contains(":def:"))
                .filter(|id| {
                    nodes_by_id
                        .get(id.as_str())
                        .is_some_and(|n| n.line == s.range.start_line)
                })
                .cloned()
                .collect();
            param_like_inputs.sort();
            param_like_inputs.dedup();

            let mut inputs: Vec<String> = if !param_like_inputs.is_empty() {
                // Prefer parameter-like defs on function start line as summary inputs.
                param_like_inputs
            } else {
                // Fallback: entry defs with no incoming in-range data edges.
                in_range_ids
                    .iter()
                    .filter(|id| id.contains(":def:"))
                    .filter(|id| {
                        in_adj
                            .get(id.as_str())
                            .map(|froms| froms.iter().all(|f| !in_range_set.contains(*f)))
                            .unwrap_or(true)
                    })
                    .cloned()
                    .collect()
            };
            inputs.sort();
            inputs.dedup();
            if inputs.is_empty() {
                continue;
            }

            let mut flows = Vec::new();
            for input in &inputs {
                let mut q: VecDeque<&str> = VecDeque::new();
                let mut seen: HashSet<&str> = HashSet::new();
                q.push_back(input.as_str());
                seen.insert(input.as_str());

                let mut impacted: BTreeSet<String> = BTreeSet::new();
                while let Some(cur) = q.pop_front() {
                    for next in out_adj.get(cur).cloned().unwrap_or_default() {
                        if !in_range_set.contains(next) || seen.contains(next) {
                            continue;
                        }
                        seen.insert(next);
                        q.push_back(next);
                        if next != input.as_str() {
                            impacted.insert(next.to_string());
                        }
                    }
                }

                flows.push(FunctionInputImpact {
                    input_node_id: input.clone(),
                    impacted_node_ids: impacted.into_iter().collect(),
                });
            }

            summaries.push(FunctionSummary {
                function_id: s.id.0.clone(),
                file: s.file.clone(),
                start_line: s.range.start_line,
                end_line: s.range.end_line,
                inputs,
                flows,
            });
        }

        summaries
    }
}

fn collect_control_ranges_rust(source: &str) -> Vec<(u32, u32)> {
    let spec = crate::ts_core::load_rust_spec();
    let compiled = match crate::ts_core::compile_queries_rust(&spec) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let Some(ctrl_q) = &compiled.control else {
        return Vec::new();
    };

    let runner = crate::ts_core::QueryRunner::new_rust();
    let offs = crate::languages::util::line_offsets(source);
    let mut out = Vec::new();
    for caps in runner.run_captures(source, ctrl_q) {
        if let Some(c0) = caps.first() {
            let start_ln = crate::languages::util::byte_to_line(&offs, c0.start);
            let end_ln = crate::languages::util::byte_to_line(&offs, c0.end.saturating_sub(1));
            out.push((start_ln, end_ln));
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

fn collect_control_ranges_ruby(source: &str) -> Vec<(u32, u32)> {
    let spec = crate::ts_core::load_ruby_spec();
    let compiled = match crate::ts_core::compile_queries_ruby(&spec) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let Some(ctrl_q) = &compiled.control else {
        return Vec::new();
    };

    let runner = crate::ts_core::QueryRunner::new_ruby();
    let offs = crate::languages::util::line_offsets(source);
    let mut out = Vec::new();
    for caps in runner.run_captures(source, ctrl_q) {
        if let Some(c0) = caps.first() {
            let start_ln = crate::languages::util::byte_to_line(&offs, c0.start);
            let end_ln = crate::languages::util::byte_to_line(&offs, c0.end.saturating_sub(1));
            out.push((start_ln, end_ln));
        }
    }

    // Text fallback for if/elsif/else...end ranges to stabilize branch merges.
    let mut if_stack: Vec<u32> = Vec::new();
    for (idx, raw) in source.lines().enumerate() {
        let line_no = (idx + 1) as u32;
        let t = raw.trim_start();
        if t.starts_with("if ") || t == "if" || t.starts_with("unless ") || t == "unless" {
            if_stack.push(line_no);
            continue;
        }
        if t.starts_with("end")
            && let Some(start_ln) = if_stack.pop()
        {
            out.push((start_ln, line_no));
        }
    }

    out.sort_unstable();
    out.dedup();
    out
}

fn reaching_def_ids_ssa_like(
    name: &str,
    use_line: u32,
    def_records_by_name: &std::collections::HashMap<String, Vec<(u32, String)>>,
    control_ranges: &[(u32, u32)],
) -> Vec<String> {
    let Some(records) = def_records_by_name.get(name) else {
        return Vec::new();
    };

    let mut prior: Vec<(u32, String)> = records
        .iter()
        .filter(|(line, _)| *line < use_line)
        .cloned()
        .collect();
    if prior.is_empty() {
        return Vec::new();
    }
    prior.sort_by_key(|(line, _)| *line);

    let latest_line = prior.last().map(|(line, _)| *line).unwrap_or(0);
    let mut selected: std::collections::BTreeSet<String> = prior
        .iter()
        .filter(|(line, _)| *line == latest_line)
        .map(|(_, id)| id.clone())
        .collect();

    // SSA-like + branch stabilization:
    // if the latest reaching def is inside a completed control range before this use,
    // include all defs of the symbol from that control range (branch-merge approximation).
    // Additionally, when only one def is found in that range, include the last pre-range def
    // to model partial assignment paths (e.g. `if cond { x = ... }` without `else`).
    for (start, end) in control_ranges {
        if *end >= use_line || latest_line < *start || latest_line > *end {
            continue;
        }

        let mut in_range_defs: Vec<&String> = Vec::new();
        for (line, id) in &prior {
            if *line >= *start && *line <= *end {
                selected.insert(id.clone());
                in_range_defs.push(id);
            }
        }

        if in_range_defs.len() <= 1
            && let Some((_, pre_id)) = prior.iter().rev().find(|(line, _)| *line < *start)
        {
            selected.insert(pre_id.clone());
        }
    }

    selected.into_iter().collect()
}

fn reaching_def_ids_ssa_like_with_same_line_params(
    name: &str,
    use_line: u32,
    def_records_by_name: &std::collections::HashMap<String, Vec<(u32, String)>>,
    param_def_records_by_name: &std::collections::HashMap<String, Vec<(u32, String)>>,
    control_ranges: &[(u32, u32)],
) -> Vec<String> {
    let reaching = reaching_def_ids_ssa_like(name, use_line, def_records_by_name, control_ranges);
    if !reaching.is_empty() {
        return reaching;
    }

    let Some(param_records) = param_def_records_by_name.get(name) else {
        return Vec::new();
    };

    let mut same_line_params: Vec<String> = param_records
        .iter()
        .filter(|(line, _)| *line == use_line)
        .map(|(_, id)| id.clone())
        .collect();
    same_line_params.sort();
    same_line_params.dedup();
    same_line_params
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
            certainty: crate::ir::reference::EdgeCertainty::Confirmed,
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
            certainty: crate::ir::reference::EdgeCertainty::Confirmed,
        };
        let pdg = PdgBuilder::build(&dfg, std::slice::from_ref(&ref_sym));
        // Check call edge added
        assert!(pdg.edges.iter().any(|e| e.from == ref_sym.from.0
            && e.to == ref_sym.to.0
            && e.kind == DependencyKind::Data));
        // Check control edge remains
        // Control edges were not in DFG; here we only check call edges merge
        // The control node should still be present
        assert!(pdg.nodes.iter().any(|n| n.id == "f.rs:ctrl:2:4"));
    }

    #[test]
    fn propagation_connects_summary_interprocedurally() {
        use crate::ir::{Symbol, SymbolKind, TextRange};

        let mut pdg = DataFlowGraph {
            nodes: vec![
                DfgNode {
                    id: "f.rs:use:x:10".to_string(),
                    name: "x".to_string(),
                    file: "f.rs".to_string(),
                    line: 10,
                },
                DfgNode {
                    id: "f.rs:def:y:10".to_string(),
                    name: "y".to_string(),
                    file: "f.rs".to_string(),
                    line: 10,
                },
                DfgNode {
                    id: "f.rs:def:a:1".to_string(),
                    name: "a".to_string(),
                    file: "f.rs".to_string(),
                    line: 1,
                },
                DfgNode {
                    id: "f.rs:use:a:2".to_string(),
                    name: "a".to_string(),
                    file: "f.rs".to_string(),
                    line: 2,
                },
            ],
            edges: vec![DfgEdge {
                from: "f.rs:def:a:1".to_string(),
                to: "f.rs:use:a:2".to_string(),
                kind: DependencyKind::Data,
            }],
        };

        let refs = vec![Reference {
            from: SymbolId("rust:f.rs:fn:caller:9".to_string()),
            to: SymbolId("rust:f.rs:fn:callee:1".to_string()),
            kind: crate::ir::reference::RefKind::Call,
            file: "f.rs".to_string(),
            line: 10,
            certainty: crate::ir::reference::EdgeCertainty::Confirmed,
        }];
        let index = crate::ir::reference::SymbolIndex::build(vec![Symbol {
            id: SymbolId("rust:f.rs:fn:callee:1".to_string()),
            name: "callee".to_string(),
            kind: SymbolKind::Function,
            file: "f.rs".to_string(),
            range: TextRange {
                start_line: 1,
                end_line: 3,
            },
            language: "rust".to_string(),
        }]);

        PdgBuilder::augment_symbolic_propagation(&mut pdg, &refs, &index);

        assert!(pdg.edges.iter().any(|e| {
            e.kind == DependencyKind::Data && e.from == "f.rs:use:x:10" && e.to == "f.rs:def:a:1"
        }));
        assert!(pdg.edges.iter().any(|e| {
            e.kind == DependencyKind::Data && e.from == "f.rs:use:a:2" && e.to == "f.rs:def:y:10"
        }));
        assert!(pdg.edges.iter().any(|e| {
            e.kind == DependencyKind::Data && e.from == "f.rs:use:x:10" && e.to == "f.rs:def:y:10"
        }));
    }

    #[test]
    fn function_summary_minimal_input_to_impact() {
        use crate::ir::{Symbol, SymbolKind, TextRange};

        let dfg = RustDfgBuilder::build(
            "f.rs",
            "fn foo(a: i32) -> i32 {\n    let b = a;\n    return b;\n}\n",
        );
        let pdg = PdgBuilder::build(&dfg, &[]);

        let foo = Symbol {
            id: SymbolId::new("rust", "f.rs", &SymbolKind::Function, "foo", 1),
            name: "foo".to_string(),
            kind: SymbolKind::Function,
            file: "f.rs".to_string(),
            range: TextRange {
                start_line: 1,
                end_line: 4,
            },
            language: "rust".to_string(),
        };
        let index = crate::ir::reference::SymbolIndex::build(vec![foo]);

        let summaries = PdgBuilder::build_function_summaries(&pdg, &index);
        assert_eq!(summaries.len(), 1);
        let s = &summaries[0];
        assert_eq!(s.function_id, "rust:f.rs:fn:foo:1");
        assert!(s.inputs.iter().any(|id| id.ends_with(":def:a:1")));

        let flow = s
            .flows
            .iter()
            .find(|f| f.input_node_id.ends_with(":def:a:1"))
            .expect("flow for input a");
        assert!(
            flow.impacted_node_ids
                .iter()
                .any(|id| id.ends_with(":def:b:2"))
        );
        assert!(
            flow.impacted_node_ids
                .iter()
                .any(|id| id.ends_with(":use:b:3"))
        );
    }

    #[test]
    fn function_summary_tracks_same_line_parameter_use_in_single_line_function() {
        use crate::ir::{Symbol, SymbolKind, TextRange};

        let dfg = RustDfgBuilder::build("f.rs", "fn foo(a: i32) -> i32 { a + 1 }\n");
        assert!(
            dfg.nodes.iter().any(|n| n.id == "f.rs:use:a:1"),
            "single-line parameter use should be tracked"
        );
        assert!(
            dfg.edges.iter().any(|e| {
                e.kind == DependencyKind::Data && e.from == "f.rs:def:a:1" && e.to == "f.rs:use:a:1"
            }),
            "single-line parameter use should connect back to its parameter def"
        );

        let foo = Symbol {
            id: SymbolId::new("rust", "f.rs", &SymbolKind::Function, "foo", 1),
            name: "foo".to_string(),
            kind: SymbolKind::Function,
            file: "f.rs".to_string(),
            range: TextRange {
                start_line: 1,
                end_line: 1,
            },
            language: "rust".to_string(),
        };
        let index = crate::ir::reference::SymbolIndex::build(vec![foo]);
        let pdg = PdgBuilder::build(&dfg, &[]);
        let summaries = PdgBuilder::build_function_summaries(&pdg, &index);
        let s = summaries.first().expect("summary for foo");
        assert!(s.inputs.iter().any(|id| id.ends_with(":def:a:1")));
        let flow = s
            .flows
            .iter()
            .find(|f| f.input_node_id.ends_with(":def:a:1"))
            .expect("flow for param a");
        assert!(
            flow.impacted_node_ids
                .iter()
                .any(|id| id.ends_with(":use:a:1")),
            "single-line return expression should remain visible in summary impact"
        );
    }

    #[test]
    fn function_summary_prefers_parameter_inputs_over_local_roots() {
        use crate::ir::{Symbol, SymbolKind, TextRange};

        let dfg = RustDfgBuilder::build(
            "f.rs",
            "fn foo(a: i32) -> i32 {\n    let seed = 1;\n    let b = a + seed;\n    return b;\n}\n",
        );
        let pdg = PdgBuilder::build(&dfg, &[]);

        let foo = Symbol {
            id: SymbolId::new("rust", "f.rs", &SymbolKind::Function, "foo", 1),
            name: "foo".to_string(),
            kind: SymbolKind::Function,
            file: "f.rs".to_string(),
            range: TextRange {
                start_line: 1,
                end_line: 5,
            },
            language: "rust".to_string(),
        };
        let index = crate::ir::reference::SymbolIndex::build(vec![foo]);

        let summaries = PdgBuilder::build_function_summaries(&pdg, &index);
        let s = summaries.first().expect("summary for foo");

        assert!(s.inputs.iter().any(|id| id.ends_with(":def:a:1")));
        assert!(
            !s.inputs.iter().any(|id| id.ends_with(":def:seed:2")),
            "parameter-style input selection should suppress local root defs"
        );
    }

    #[test]
    fn function_summary_falls_back_to_root_defs_without_params() {
        use crate::ir::{Symbol, SymbolKind, TextRange};

        let dfg = RustDfgBuilder::build(
            "f.rs",
            "fn bar() -> i32 {\n    let seed = 1;\n    let out = seed;\n    return out;\n}\n",
        );
        let pdg = PdgBuilder::build(&dfg, &[]);

        let bar = Symbol {
            id: SymbolId::new("rust", "f.rs", &SymbolKind::Function, "bar", 1),
            name: "bar".to_string(),
            kind: SymbolKind::Function,
            file: "f.rs".to_string(),
            range: TextRange {
                start_line: 1,
                end_line: 5,
            },
            language: "rust".to_string(),
        };
        let index = crate::ir::reference::SymbolIndex::build(vec![bar]);

        let summaries = PdgBuilder::build_function_summaries(&pdg, &index);
        let s = summaries.first().expect("summary for bar");

        assert!(
            s.inputs.iter().any(|id| id.ends_with(":def:seed:2")),
            "fallback root-def input should remain for parameterless function"
        );
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
        // SSA-like expectation: use at line 4 should connect to latest reaching def (line 3)
        let use_id = "f.rs:use:x:4";
        let incoming: Vec<_> = dfg
            .edges
            .iter()
            .filter(|e| e.kind == DependencyKind::Data && e.to == use_id)
            .map(|e| e.from.clone())
            .collect();
        assert_eq!(incoming.len(), 1, "expected one reaching def for x@line4");
        assert!(
            incoming[0].ends_with(":def:x:3"),
            "expected latest def at line 3, got {}",
            incoming[0]
        );

        // Alias propagation: let y = x should add def(x@3) -> def(y@4)
        assert!(dfg.edges.iter().any(|e| {
            e.kind == DependencyKind::Data
                && e.from.ends_with(":def:x:3")
                && e.to.ends_with(":def:y:4")
        }));
    }

    #[test]
    fn rust_alias_chain_and_reassignment_prefers_latest_def() {
        let src = "let mut a = seed;\nlet b = a;\na = other;\nlet c = a;\nlet d = b;\n";
        let dfg = RustDfgBuilder::build("f.rs", src);

        // `let mut a` should define `a` (not a bogus `mut` symbol).
        assert!(dfg.nodes.iter().any(|n| n.id.ends_with(":def:a:1")));
        assert!(!dfg.nodes.iter().any(|n| n.name == "mut"));

        // Assignment chain: a -> b -> d.
        assert!(dfg.edges.iter().any(|e| {
            e.kind == DependencyKind::Data
                && e.from.ends_with(":def:a:1")
                && e.to.ends_with(":def:b:2")
        }));
        assert!(dfg.edges.iter().any(|e| {
            e.kind == DependencyKind::Data
                && e.from.ends_with(":def:b:2")
                && e.to.ends_with(":def:d:5")
        }));

        // Reassignment: `c = a` should use latest def of a (line 3), not initial line 1.
        assert!(dfg.edges.iter().any(|e| {
            e.kind == DependencyKind::Data
                && e.from.ends_with(":def:a:3")
                && e.to.ends_with(":def:c:4")
        }));
        assert!(!dfg.edges.iter().any(|e| {
            e.kind == DependencyKind::Data
                && e.from.ends_with(":def:a:1")
                && e.to.ends_with(":def:c:4")
        }));
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
    fn rust_reaching_defs_branch_join_includes_both_branch_defs() {
        let src = "let x = 0;\nif cond {\n    x = 1;\n} else {\n    x = 2;\n}\nlet y = x;\n";
        let dfg = RustDfgBuilder::build("f.rs", src);

        let use_id = "f.rs:use:x:7";
        let incoming: std::collections::BTreeSet<_> = dfg
            .edges
            .iter()
            .filter(|e| e.kind == DependencyKind::Data && e.to == use_id)
            .map(|e| e.from.clone())
            .collect();

        assert!(
            incoming.iter().any(|id| id.ends_with(":def:x:3")),
            "expected then-branch def to reach join"
        );
        assert!(
            incoming.iter().any(|id| id.ends_with(":def:x:5")),
            "expected else-branch def to reach join"
        );
        assert!(
            !incoming.iter().any(|id| id.ends_with(":def:x:1")),
            "initial def should be shadowed by branch defs"
        );
    }

    #[test]
    fn rust_reaching_defs_partial_branch_keeps_pre_branch_def() {
        let src = "let x = 0;\nif cond {\n    x = 1;\n}\nlet y = x;\n";
        let dfg = RustDfgBuilder::build("f.rs", src);

        let use_id = "f.rs:use:x:5";
        let incoming: std::collections::BTreeSet<_> = dfg
            .edges
            .iter()
            .filter(|e| e.kind == DependencyKind::Data && e.to == use_id)
            .map(|e| e.from.clone())
            .collect();

        assert!(
            incoming.iter().any(|id| id.ends_with(":def:x:3")),
            "branch def should reach join"
        );
        assert!(
            incoming.iter().any(|id| id.ends_with(":def:x:1")),
            "pre-branch def should remain for non-taken path"
        );
    }

    #[test]
    fn ruby_alias_assignment_adds_def_to_def_edge() {
        let src = "a = seed\nb = a\nreturn b\n";
        let dfg = RubyDfgBuilder::build("test.rb", src);

        assert!(dfg.edges.iter().any(|e| {
            e.kind == DependencyKind::Data
                && e.from.ends_with(":def:a:1")
                && e.to.ends_with(":def:b:2")
        }));
    }

    #[test]
    fn ruby_alias_reassignment_prefers_latest_def() {
        let src = "a = seed\nb = a\na = other\nc = a\n";
        let dfg = RubyDfgBuilder::build("test.rb", src);

        assert!(dfg.edges.iter().any(|e| {
            e.kind == DependencyKind::Data
                && e.from.ends_with(":def:a:1")
                && e.to.ends_with(":def:b:2")
        }));
        assert!(dfg.edges.iter().any(|e| {
            e.kind == DependencyKind::Data
                && e.from.ends_with(":def:a:3")
                && e.to.ends_with(":def:c:4")
        }));
        assert!(!dfg.edges.iter().any(|e| {
            e.kind == DependencyKind::Data
                && e.from.ends_with(":def:a:1")
                && e.to.ends_with(":def:c:4")
        }));
    }

    #[test]
    fn ruby_reaching_defs_partial_branch_keeps_pre_branch_def() {
        let src = "a = seed\nif cond\n  a = other\nend\nb = a\n";
        let dfg = RubyDfgBuilder::build("test.rb", src);

        let use_id = "test.rb:use:a:5";
        let incoming: std::collections::BTreeSet<_> = dfg
            .edges
            .iter()
            .filter(|e| e.kind == DependencyKind::Data && e.to == use_id)
            .map(|e| e.from.clone())
            .collect();

        assert!(
            incoming.iter().any(|id| id.ends_with(":def:a:3")),
            "branch def should reach join"
        );
        assert!(
            incoming.iter().any(|id| id.ends_with(":def:a:1")),
            "pre-branch def should remain for non-taken path"
        );
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
