use crate::dfg::{DataFlowGraph, DependencyKind};
use crate::impact::ImpactOutput;
use crate::ir::SymbolKind;

fn esc_dot(s: &str) -> String {
    s.replace('"', "\\\"").replace('\n', " ")
}

fn parse_symbol_id(id: &str) -> Option<(String, String, String, String, u32)> {
    // lang:file:kind:name:line
    let parts: Vec<&str> = id.split(':').collect();
    if parts.len() < 5 {
        return None;
    }
    let lang = parts[0].to_string();
    let kind = parts[2].to_string();
    let name = parts[3].to_string();
    let line: u32 = parts[4].parse().ok()?;
    // Rejoin file (in case it contained colons in rare environments)
    let file = parts[1].to_string();
    Some((lang, file, kind, name, line))
}

/// Convert a DataFlowGraph (PDG) to GraphViz dot format.
pub fn dfg_to_dot(graph: &DataFlowGraph) -> String {
    use std::fmt::Write as _;
    let mut buf = String::new();
    buf.push_str("digraph pdg {\n");
    buf.push_str("  rankdir=LR;\n  node [shape=oval, fontname=\"monospace\"];\n");
    // Nodes
    for node in &graph.nodes {
        let label = format!(
            "{}\n{}:{}",
            esc_dot(&node.name),
            esc_dot(&node.file),
            node.line
        );
        let _ = writeln!(buf, "  \"{}\" [label=\"{}\"];", esc_dot(&node.id), label);
    }
    // Edges
    for edge in &graph.edges {
        let style = match edge.kind {
            DependencyKind::Data => "solid",
            DependencyKind::Control => "dashed",
        };
        let _ = writeln!(
            buf,
            "  \"{}\" -> \"{}\" [style={}];",
            esc_dot(&edge.from),
            esc_dot(&edge.to),
            style
        );
    }
    buf.push_str("}\n");
    buf
}
// Unit tests for PDG dot rendering
#[cfg(test)]
mod dfg_render_tests {
    use super::*;
    use crate::dfg::{DataFlowGraph, DependencyKind, DfgEdge, DfgNode};

    #[test]
    fn test_dfg_to_dot_empty() {
        let graph = DataFlowGraph {
            nodes: Vec::new(),
            edges: Vec::new(),
        };
        let dot = dfg_to_dot(&graph);
        assert!(dot.starts_with("digraph pdg"));
    }

    #[test]
    fn test_dfg_to_dot_simple() {
        let node = DfgNode {
            id: "n1".to_string(),
            name: "x".to_string(),
            file: "f.rs".to_string(),
            line: 1,
        };
        let edge = DfgEdge {
            from: "n1".to_string(),
            to: "n1".to_string(),
            kind: DependencyKind::Data,
        };
        let graph = DataFlowGraph {
            nodes: vec![node.clone()],
            edges: vec![edge],
        };
        let dot = dfg_to_dot(&graph);
        assert!(dot.contains("\"n1\""));
        assert!(dot.contains("solid"));
    }
}

#[cfg(test)]
mod impact_render_tests {
    use super::*;
    use crate::impact::ImpactOutput;
    use crate::ir::reference::{RefKind, Reference};
    use crate::ir::{Symbol, SymbolId, SymbolKind, TextRange};

    fn mk_sym(file: &str, name: &str, line: u32) -> Symbol {
        let kind = SymbolKind::Function;
        Symbol {
            id: SymbolId::new("rust", file, &kind, name, line),
            name: name.to_string(),
            kind,
            file: file.to_string(),
            range: TextRange {
                start_line: line,
                end_line: line,
            },
            language: "rust".to_string(),
        }
    }

    #[test]
    fn to_dot_highlights_path_edges() {
        let a = mk_sym("f.rs", "a", 1);
        let b = mk_sym("f.rs", "b", 2);
        let c = mk_sym("f.rs", "c", 3);
        let edges = vec![
            Reference {
                from: a.id.clone(),
                to: b.id.clone(),
                kind: RefKind::Call,
                file: "f.rs".into(),
                line: 2,
            },
            Reference {
                from: b.id.clone(),
                to: c.id.clone(),
                kind: RefKind::Call,
                file: "f.rs".into(),
                line: 3,
            },
        ];
        let out = ImpactOutput {
            changed_symbols: vec![a.clone()],
            impacted_symbols: vec![b.clone(), c.clone()],
            impacted_files: vec!["f.rs".into()],
            edges: edges.clone(),
            impacted_by_file: std::collections::HashMap::new(),
        };
        let dot = to_dot(&out);
        assert!(
            dot.contains("color=\"#e33\""),
            "expected highlighted path edges"
        );
    }

    #[test]
    fn to_html_embeds_assets() {
        let changed = mk_sym("src/lib.rs", "foo", 10);
        let out = ImpactOutput {
            changed_symbols: vec![changed.clone()],
            impacted_symbols: vec![],
            impacted_files: vec!["src/lib.rs".into()],
            edges: vec![],
            impacted_by_file: std::collections::HashMap::new(),
        };
        let html = super::to_html(&out);
        assert!(html.contains("<!doctype html>"));
        assert!(
            html.contains("const WORKER_SRC = \""),
            "expected worker script to be embedded as JSON string"
        );
        assert!(html.contains("class=\"symbol-select\""));
        assert!(html.contains("symbols-select-all"));
    }
}

/// Compute a set of undirected edge pairs that lie on at least one shortest path
/// from any changed symbol to any impacted symbol, using the provided edges.
fn compute_path_pairs(out: &ImpactOutput) -> std::collections::HashSet<(String, String)> {
    use std::collections::{HashMap, HashSet, VecDeque};
    let mut pairs: HashSet<(String, String)> = HashSet::new();
    if out.edges.is_empty() {
        return pairs;
    }
    // Build undirected adjacency (owned strings for simplicity)
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();
    for e in &out.edges {
        adj.entry(e.from.0.clone())
            .or_default()
            .push(e.to.0.clone());
        adj.entry(e.to.0.clone())
            .or_default()
            .push(e.from.0.clone());
    }
    // Multi-source BFS from changed nodes
    let mut q: VecDeque<String> = VecDeque::new();
    let mut par: HashMap<String, String> = HashMap::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut roots: HashSet<String> = HashSet::new();
    for s in &out.changed_symbols {
        roots.insert(s.id.0.clone());
        if seen.insert(s.id.0.clone()) {
            q.push_back(s.id.0.clone());
        }
    }
    while let Some(u) = q.pop_front() {
        if let Some(nei) = adj.get(&u) {
            for v in nei {
                if seen.insert(v.clone()) {
                    par.insert(v.clone(), u.clone());
                    q.push_back(v.clone());
                }
            }
        }
    }
    // Reconstruct one path per impacted node (to nearest root)
    for t in &out.impacted_symbols {
        let mut cur = t.id.0.clone();
        if !par.contains_key(&cur) && !roots.contains(&cur) {
            continue;
        }
        while let Some(p) = par.get(&cur) {
            pairs.insert((cur.clone(), p.clone()));
            pairs.insert((p.clone(), cur.clone()));
            if roots.contains(p) {
                break;
            }
            cur = p.clone();
        }
    }
    pairs
}

pub fn to_dot(out: &ImpactOutput) -> String {
    use std::fmt::Write as _;
    let mut buf = String::new();
    buf.push_str("digraph impact {\n");
    buf.push_str("  rankdir=LR;\n  node [shape=box, fontname=\"monospace\"];\n");

    let path_pairs = compute_path_pairs(out);

    // Collect nodes (changed + impacted), de-dup by id
    let mut seen = std::collections::BTreeSet::new();
    for s in &out.changed_symbols {
        if seen.insert(s.id.0.clone()) {
            let _ = writeln!(
                buf,
                "  \"{}\" [label=\"{}\\n{}:{}\", style=filled, fillcolor=\"#fee\"];",
                esc_dot(&s.id.0),
                esc_dot(&s.name),
                esc_dot(&s.file),
                s.range.start_line
            );
        }
    }
    for s in &out.impacted_symbols {
        if seen.insert(s.id.0.clone()) {
            let _ = writeln!(
                buf,
                "  \"{}\" [label=\"{}\\n{}:{}\", style=filled, fillcolor=\"#eef\"];",
                esc_dot(&s.id.0),
                esc_dot(&s.name),
                esc_dot(&s.file),
                s.range.start_line
            );
        }
    }
    // Add context nodes referenced by edges but not in changed/impacted
    for e in &out.edges {
        for id in [e.from.0.as_str(), e.to.0.as_str()] {
            if seen.contains(id) {
                continue;
            }
            let (label, file, line) =
                if let Some((_lang, file, _kind, name, line)) = parse_symbol_id(id) {
                    (esc_dot(&name).to_string(), esc_dot(&file), line)
                } else {
                    (esc_dot(id), String::new(), 0)
                };
            let _ = writeln!(
                buf,
                "  \"{}\" [label=\"{}\\n{}:{}\", style=filled, fillcolor=\"#eee\"];",
                esc_dot(id),
                label,
                file,
                line
            );
            seen.insert(id.to_string());
        }
    }

    if !out.edges.is_empty() {
        for e in &out.edges {
            let highlight = path_pairs.contains(&(e.from.0.clone(), e.to.0.clone()));
            let attrs = if highlight {
                " [color=\"#e33\",penwidth=2]"
            } else {
                ""
            };
            let _ = writeln!(
                buf,
                "  \"{}\" -> \"{}\"{};",
                esc_dot(&e.from.0),
                esc_dot(&e.to.0),
                attrs
            );
        }
    }
    buf.push_str("}\n");
    buf
}

pub fn to_html(out: &ImpactOutput) -> String {
    html::render(out)
}

mod html {
    use super::{h, kind_code, parse_symbol_id};
    use crate::impact::ImpactOutput;
    use serde_json::json;
    use std::collections::BTreeSet;

    const TEMPLATE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/assets/report.html"
    ));
    const STYLE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/assets/report.css"
    ));
    const SCRIPT_MAIN: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/assets/report_main.js"
    ));
    const SCRIPT_WORKER: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/assets/impact_worker.js"
    ));

    pub(super) fn render(out: &ImpactOutput) -> String {
        HtmlReportPage { out }.render()
    }

    struct HtmlReportPage<'a> {
        out: &'a ImpactOutput,
    }

    impl<'a> HtmlReportPage<'a> {
        fn render(&self) -> String {
            let mut html = TEMPLATE.to_owned();
            html = html.replace("{{STYLE}}", STYLE);
            html = html.replace(
                "{{COUNT_CHANGED}}",
                &self.out.changed_symbols.len().to_string(),
            );
            html = html.replace(
                "{{COUNT_IMPACTED}}",
                &self.out.impacted_symbols.len().to_string(),
            );
            html = html.replace(
                "{{COUNT_FILES}}",
                &self.out.impacted_files.len().to_string(),
            );
            html = html.replace("{{COUNT_EDGES}}", &self.out.edges.len().to_string());
            html = html.replace("{{CHANGED_LIST}}", &self.render_changed_list());
            html = html.replace("{{IMPACTED_LIST}}", &self.render_impacted_list());
            html = html.replace("{{EDGES_SECTION}}", &self.render_edges_section());
            html = html.replace("{{IMPACT_DATA}}", &escape_script(&self.impact_data_json()));
            html = html.replace("{{WORKER_SRC}}", &self.worker_script_json());
            html = html.replace("{{MAIN_SCRIPT}}", &escape_script(SCRIPT_MAIN));
            html
        }

        fn impact_data_json(&self) -> String {
            let mut nodes = Vec::new();
            let mut seen: BTreeSet<String> = BTreeSet::new();

            for s in &self.out.changed_symbols {
                if seen.insert(s.id.0.clone()) {
                    nodes.push(json!({
                        "data": {
                            "id": s.id.0,
                            "label": s.name,
                            "file": s.file,
                            "line": s.range.start_line,
                            "changed": true,
                            "kind": kind_code(&s.kind),
                        }
                    }));
                }
            }

            for s in &self.out.impacted_symbols {
                if seen.insert(s.id.0.clone()) {
                    nodes.push(json!({
                        "data": {
                            "id": s.id.0,
                            "label": s.name,
                            "file": s.file,
                            "line": s.range.start_line,
                            "changed": false,
                            "kind": kind_code(&s.kind),
                        }
                    }));
                }
            }

            let mut edges = Vec::new();
            for e in &self.out.edges {
                edges.push(json!({
                    "data": {
                        "id": format!("{}->{}", e.from.0, e.to.0),
                        "source": e.from.0,
                        "target": e.to.0,
                    }
                }));

                for id in [&e.from.0, &e.to.0] {
                    if seen.contains(id) {
                        continue;
                    }
                    if let Some((_lang, file, kind, name, line)) = parse_symbol_id(id) {
                        nodes.push(json!({
                            "data": {
                                "id": id,
                                "label": name,
                                "file": file,
                                "line": line,
                                "changed": false,
                                "kind": kind,
                            }
                        }));
                    } else {
                        nodes.push(json!({
                            "data": {
                                "id": id,
                                "label": id,
                                "file": "",
                                "line": 0,
                                "changed": false,
                                "kind": "mod",
                            }
                        }));
                    }
                    seen.insert((*id).to_owned());
                }
            }

            serde_json::to_string(&json!({ "nodes": nodes, "edges": edges }))
                .unwrap_or_else(|_| "{}".to_string())
        }

        fn render_changed_list(&self) -> String {
            let mut buf = String::new();
            for s in &self.out.changed_symbols {
                buf.push_str(&format!(
                    "<li><label><input type=\"checkbox\" class=\"symbol-select\" value=\"{}\" data-role=\"changed\" data-kind=\"{}\" data-changed=\"true\" checked> <code>{}</code> — {} ({}:{})</label></li>\n",
                    h(&s.id.0),
                    kind_code(&s.kind),
                    h(&s.id.0),
                    h(&s.name),
                    h(&s.file),
                    s.range.start_line
                ));
            }
            if buf.is_empty() {
                buf.push_str("<li><em>none</em></li>\n");
            }
            buf
        }

        fn render_impacted_list(&self) -> String {
            let mut buf = String::new();
            for s in &self.out.impacted_symbols {
                buf.push_str(&format!(
                    "<li><label><input type=\"checkbox\" class=\"symbol-select\" value=\"{}\" data-role=\"impacted\" data-kind=\"{}\" data-changed=\"false\" checked> <code>{}</code> — {} ({}:{})</label></li>\n",
                    h(&s.id.0),
                    kind_code(&s.kind),
                    h(&s.id.0),
                    h(&s.name),
                    h(&s.file),
                    s.range.start_line
                ));
            }
            if buf.is_empty() {
                buf.push_str("<li><em>none</em></li>\n");
            }
            buf
        }

        fn render_edges_section(&self) -> String {
            if self.out.edges.is_empty() {
                return String::new();
            }
            let mut buf = String::from(
                "<div class=\"sec card\"><h2>Edges</h2><table><thead><tr><th>From</th><th>To</th></tr></thead><tbody>",
            );
            for e in &self.out.edges {
                buf.push_str(&format!(
                    "<tr><td><code>{}</code></td><td><code>{}</code></td></tr>",
                    h(&e.from.0),
                    h(&e.to.0)
                ));
            }
            buf.push_str("</tbody></table></div>");
            buf
        }

        fn worker_script_json(&self) -> String {
            serde_json::to_string(SCRIPT_WORKER).unwrap_or_else(|_| "\"\"".to_string())
        }
    }

    fn escape_script(src: &str) -> String {
        src.replace("</", "<\\/")
    }
}

fn h(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn kind_code(k: &SymbolKind) -> &'static str {
    match k {
        SymbolKind::Function => "fn",
        SymbolKind::Method => "method",
        SymbolKind::Struct => "struct",
        SymbolKind::Enum => "enum",
        SymbolKind::Trait => "trait",
        SymbolKind::Module => "mod",
    }
}
