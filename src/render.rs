use crate::impact::ImpactOutput;

fn esc_dot(s: &str) -> String {
    s.replace('"', "\\\"").replace('\n', " ")
}

pub fn to_dot(out: &ImpactOutput) -> String {
    use std::fmt::Write as _;
    let mut buf = String::new();
    buf.push_str("digraph impact {\n");
    buf.push_str("  rankdir=LR;\n  node [shape=box, fontname=\"monospace\"];\n");

    // Collect nodes (changed + impacted), de-dup by id
    let mut seen = std::collections::BTreeSet::new();
    for s in &out.changed_symbols {
        if seen.insert(s.id.0.clone()) {
            let _ = writeln!(
                buf,
                "  \"{}\" [label=\"{}\\n{}:{}\", style=filled, fillcolor=\"#fee\"];",
                esc_dot(&s.id.0), esc_dot(&s.name), esc_dot(&s.file), s.range.start_line
            );
        }
    }
    for s in &out.impacted_symbols {
        if seen.insert(s.id.0.clone()) {
            let _ = writeln!(
                buf,
                "  \"{}\" [label=\"{}\\n{}:{}\", style=filled, fillcolor=\"#eef\"];",
                esc_dot(&s.id.0), esc_dot(&s.name), esc_dot(&s.file), s.range.start_line
            );
        }
    }

    if !out.edges.is_empty() {
        for e in &out.edges {
            let _ = writeln!(
                buf,
                "  \"{}\" -> \"{}\";",
                esc_dot(&e.from.0), esc_dot(&e.to.0)
            );
        }
    }
    buf.push_str("}\n");
    buf
}

pub fn to_html(out: &ImpactOutput) -> String {
    // Minimal, dependency-free HTML report. Lists changed/impacted and edges.
    // Self-contained (no external scripts), easy to view in CI artifacts.
    let mut html = String::new();
    html.push_str(r#"<!doctype html><html lang="en"><meta charset="utf-8"><title>dimpact report</title>
<style>
body{font:14px/1.5 -apple-system,BlinkMacSystemFont,Segoe UI,Roboto,Helvetica,Arial,sans-serif;margin:20px;color:#222}
code{background:#f5f5f5;padding:1px 4px;border-radius:3px}
.sec{margin-bottom:24px}
.grid{display:grid;grid-template-columns:1fr 1fr;gap:16px}
.card{border:1px solid #ddd;border-radius:6px;padding:12px}
.muted{color:#666}
table{border-collapse:collapse;width:100%}
th,td{border:1px solid #ddd;padding:6px 8px;text-align:left}
th{background:#fafafa}
</style>
<h1>dimpact report</h1>
"#);
    html.push_str(&format!(
        "<p class=muted>changed: {} symbols • impacted: {} symbols • files: {}</p>",
        out.changed_symbols.len(), out.impacted_symbols.len(), out.impacted_files.len()
    ));
    html.push_str("<div class=grid>");
    html.push_str("<div class=card><h2>Changed Symbols</h2><ul>");
    for s in &out.changed_symbols {
        html.push_str(&format!(
            "<li><code>{}</code> — {} ({}:{})</li>",
            h(&s.id.0), h(&s.name), h(&s.file), s.range.start_line
        ));
    }
    html.push_str("</ul></div>");
    html.push_str("<div class=card><h2>Impacted Symbols</h2><ul>");
    for s in &out.impacted_symbols {
        html.push_str(&format!(
            "<li><code>{}</code> — {} ({}:{})</li>",
            h(&s.id.0), h(&s.name), h(&s.file), s.range.start_line
        ));
    }
    html.push_str("</ul></div>");
    html.push_str("</div>");
    if !out.edges.is_empty() {
        html.push_str("<div class=sec card><h2>Edges</h2><table><thead><tr><th>From</th><th>To</th></tr></thead><tbody>");
        for e in &out.edges {
            html.push_str(&format!(
                "<tr><td><code>{}</code></td><td><code>{}</code></td></tr>",
                h(&e.from.0), h(&e.to.0)
            ));
        }
        html.push_str("</tbody></table></div>");
    }
    html.push_str("</html>");
    html
}

fn h(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

