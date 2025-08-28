use crate::impact::ImpactOutput;
use crate::ir::SymbolKind;

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
    // Enhanced HTML report with optional Cytoscape.js visualization and a built-in canvas fallback.
    // Data embed
    use serde_json::json;
    use std::collections::BTreeSet;
    let mut nodes = vec![];
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for s in &out.changed_symbols {
        if seen.insert(&s.id.0) {
            nodes.push(json!({
                "data": { "id": s.id.0, "label": s.name, "file": s.file, "line": s.range.start_line, "changed": true, "kind": kind_code(&s.kind) }
            }));
        }
    }
    for s in &out.impacted_symbols {
        if seen.insert(&s.id.0) {
            nodes.push(json!({
                "data": { "id": s.id.0, "label": s.name, "file": s.file, "line": s.range.start_line, "changed": false, "kind": kind_code(&s.kind) }
            }));
        }
    }
    let mut edges = vec![];
    for e in &out.edges {
        edges.push(json!({ "data": { "id": format!("{}->{}", e.from.0, e.to.0), "source": e.from.0, "target": e.to.0 } }));
    }
    let data_json = json!({ "nodes": nodes, "edges": edges }).to_string();

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
#viz{height:520px;border:1px solid #ddd;border-radius:6px;margin:12px 0}
#canvas{height:520px;border:1px solid #ddd;border-radius:6px;display:none}
.toolbar{display:flex;gap:8px;align-items:center;margin:8px 0}
.badge{display:inline-block;padding:2px 6px;border-radius:10px;background:#eee;margin-left:6px}
.chip{display:inline-block;padding:0 8px;border-radius:12px;font-size:12px}
.chip.changed{background:#fee;border:1px solid #fbb}
.chip.imp{background:#eef;border:1px solid #bbf}
.filters{display:flex;flex-wrap:wrap;gap:10px;align-items:center;margin:6px 0}
label.small{font-size:12px;color:#555}
.popup{position:fixed;right:16px;bottom:16px;max-width:420px;background:#fff;border:1px solid #ddd;border-radius:8px;box-shadow:0 8px 24px rgba(0,0,0,.12);padding:12px;display:none}
.popup h3{margin:0 0 6px 0;font-size:16px}
.popup .row{margin:4px 0}
.popup .row code{display:inline-block}
.popup .actions{display:flex;gap:8px;margin-top:8px}
</style>
<h1>dimpact report</h1>
"#);
    html.push_str(&format!(
        "<p class=muted>changed: {} symbols • impacted: {} symbols • files: {} <span class=badge>edges: {}</span></p>",
        out.changed_symbols.len(), out.impacted_symbols.len(), out.impacted_files.len(), out.edges.len()
    ));
    // Controls + Graph area
    html.push_str(r#"
<div class=card>
  <div class=toolbar>
    <strong>Graph</strong>
    <span class="chip changed">changed</span>
    <span class="chip imp">impacted</span>
    <button id="layout-bf">layout: breadthfirst</button>
    <button id="layout-grid">layout: grid</button>
    <button id="layout-cose">layout: cose</button>
    <button id="toggle-canvas">fallback canvas</button>
  </div>
  <div class="filters">
    <label class=small><input type=checkbox id=f_changed checked> changed</label>
    <label class=small><input type=checkbox id=f_impacted checked> impacted</label>
    <label class=small>kind:
      <label class=small><input type=checkbox class=kind checked value="fn"> fn</label>
      <label class=small><input type=checkbox class=kind checked value="method"> method</label>
      <label class=small><input type=checkbox class=kind checked value="struct"> struct</label>
      <label class=small><input type=checkbox class=kind checked value="enum"> enum</label>
      <label class=small><input type=checkbox class=kind checked value="trait"> trait</label>
      <label class=small><input type=checkbox class=kind checked value="mod"> mod</label>
    </label>
    <label class=small>max depth: <input type=number id=f_depth min=0 style="width:4em"></label>
    <label class=small>file contains: <input type=text id=f_file placeholder="path or module" style="width:14em"></label>
    <button id=apply-filters>apply</button>
    <button id=reset-filters>reset</button>
  </div>
  <div id="viz"></div>
  <canvas id="canvas"></canvas>
</div>
"#);

    // Lists
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

    // Scripts: try Cytoscape via CDN; fallback to canvas rendering
    html.push_str("<script>const IMPACT_DATA = ");
    html.push_str(&h(&data_json));
    html.push_str(";\n</script>\n");
    html.push_str(r#"
<script>
(function(){
  function hasCytoscape(){ return typeof window.cytoscape === 'function'; }
  function getFilterState(){
    const kinds = Array.from(document.querySelectorAll('input.kind:checked')).map(x=>x.value);
    return {
      changed: document.getElementById('f_changed').checked,
      impacted: document.getElementById('f_impacted').checked,
      kinds,
      depth: (function(){ const v = document.getElementById('f_depth').value; return v===''?null:Math.max(0, parseInt(v,10)||0); })(),
      file: document.getElementById('f_file').value.trim().toLowerCase()
    };
  }
  function computeDistancesUndirected(){
    const adj = new Map();
    IMPACT_DATA.nodes.forEach(n=>{ adj.set(n.data.id, new Set()); });
    IMPACT_DATA.edges.forEach(e=>{ (adj.get(e.data.source)||new Set()).add(e.data.target); (adj.get(e.data.target)||new Set()).add(e.data.source); });
    const q=[]; const dist=new Map();
    IMPACT_DATA.nodes.forEach(n=>{ if(n.data.changed){ dist.set(n.data.id,0); q.push(n.data.id);} });
    while(q.length){ const u=q.shift(); const d=dist.get(u)||0; const neigh=Array.from(adj.get(u)||[]); for(const v of neigh){ if(!dist.has(v)){ dist.set(v,d+1); q.push(v);} } }
    return dist; // id->distance
  }
  const DIST = computeDistancesUndirected();

  function renderWithCytoscape(){
    const el = document.getElementById('viz');
    el.style.display = 'block';
    const cy = cytoscape({
      container: el,
      elements: IMPACT_DATA,
      style: [
        { selector: 'node', style: { 'label': 'data(label)', 'font-size': 10, 'text-valign': 'center', 'text-halign': 'center', 'background-color': '#eef', 'border-width': 1, 'border-color': '#bbf', 'width': 22, 'height': 22 }},
        { selector: 'node[changed = true]', style: { 'background-color': '#fee', 'border-color': '#fbb' }},
        { selector: 'edge', style: { 'width': 1, 'line-color': '#ccc', 'target-arrow-color': '#ccc', 'target-arrow-shape': 'triangle', 'curve-style': 'bezier' }}
      ],
      layout: { name: 'breadthfirst', directed: true }
    });
    document.getElementById('layout-bf').onclick = () => cy.layout({name:'breadthfirst', directed:true}).run();
    document.getElementById('layout-grid').onclick = () => cy.layout({name:'grid'}).run();
    document.getElementById('layout-cose').onclick = () => cy.layout({name:'cose'}).run();

    // Popup on click
    const popup = document.getElementById('popup');
    const pTitle = document.getElementById('p-title');
    const pId = document.getElementById('p-id');
    const pFile = document.getElementById('p-file');
    const pKind = document.getElementById('p-kind');
    const pDepth = document.getElementById('p-depth');
    cy.on('tap', 'node', (evt)=>{
      const d = evt.target.data();
      pTitle.textContent = d.label;
      pId.textContent = d.id;
      pFile.textContent = d.file + ':' + d.line;
      pKind.textContent = d.kind + (d.changed? ' (changed)':'');
      pDepth.textContent = (DIST.has(d.id)? DIST.get(d.id) : 'n/a');
      popup.style.display='block';
    });
    document.getElementById('p-copy').onclick = ()=>{ navigator.clipboard && navigator.clipboard.writeText(pId.textContent); };
    document.getElementById('p-close').onclick = ()=>{ popup.style.display='none'; };

    function applyFilters(){
      const f = getFilterState();
      const visibleNode = new Set();
      cy.nodes().forEach(n=>{
        const d=n.data();
        const passChanged = (d.changed && f.changed) || (!d.changed && f.impacted);
        const passKind = f.kinds.includes(String(d.kind||''));
        const passFile = f.file==='' || String(d.file||'').toLowerCase().includes(f.file);
        const passDepth = (f.depth==null) || ((DIST.get(d.id)||Infinity) <= f.depth);
        const show = passChanged && passKind && passFile && passDepth;
        if(show){ n.show(); visibleNode.add(d.id);} else { n.hide(); }
      });
      cy.edges().forEach(e=>{
        const s=e.data('source'), t=e.data('target');
        if(visibleNode.has(s) && visibleNode.has(t)) e.show(); else e.hide();
      });
    }
    document.getElementById('apply-filters').onclick = applyFilters;
    document.getElementById('reset-filters').onclick = ()=>{
      document.getElementById('f_changed').checked = true;
      document.getElementById('f_impacted').checked = true;
      document.querySelectorAll('input.kind').forEach(x=>x.checked=true);
      document.getElementById('f_depth').value = '';
      document.getElementById('f_file').value = '';
      applyFilters();
    };
    applyFilters();
  }

  function renderWithCanvas(){
    const cv = document.getElementById('canvas');
    const viz = document.getElementById('viz');
    viz.style.display = 'none';
    cv.style.display = 'block';
    const w = cv.clientWidth || 800, h = cv.clientHeight || 520;
    cv.width = w; cv.height = h;
    const ctx = cv.getContext('2d');
    ctx.clearRect(0,0,w,h);
    const F = getFilterState();
    const nodes = IMPACT_DATA.nodes
      .filter(n=>{
        const d=n.data;
        const passChanged = (d.changed && F.changed) || (!d.changed && F.impacted);
        const passKind = F.kinds.includes(String(d.kind||''));
        const passFile = F.file==='' || String(d.file||'').toLowerCase().includes(F.file);
        const passDepth = (F.depth==null) || ((DIST.get(d.id)||Infinity) <= F.depth);
        return passChanged && passKind && passFile && passDepth;
      })
      .map((n,i)=>({ id: n.data.id, label: n.data.label, changed: !!n.data.changed, x:0, y:0 }));
    const N = nodes.length, R = Math.max(80, Math.min(w,h)/2 - 40), cx = w/2, cy = h/2;
    for(let i=0;i<N;i++){ const a = (2*Math.PI*i)/N; nodes[i].x = cx + R*Math.cos(a); nodes[i].y = cy + R*Math.sin(a); }
    // edges
    ctx.strokeStyle = '#ccc'; ctx.lineWidth = 1;
    IMPACT_DATA.edges.forEach(e=>{
      const s = nodes.find(n=>n.id===e.data.source), t = nodes.find(n=>n.id===e.data.target);
      if(!s||!t) return; ctx.beginPath(); ctx.moveTo(s.x, s.y); ctx.lineTo(t.x, t.y); ctx.stroke();
    });
    // nodes
    nodes.forEach(n=>{
      ctx.beginPath(); ctx.fillStyle = n.changed ? '#fee' : '#eef'; ctx.strokeStyle = n.changed ? '#fbb' : '#bbf';
      ctx.arc(n.x, n.y, 12, 0, 2*Math.PI); ctx.fill(); ctx.stroke();
      ctx.fillStyle = '#333'; ctx.font = '10px monospace'; ctx.textAlign = 'center';
      ctx.fillText(n.label, n.x, n.y-16);
    });

    // simple popup via nearest node
    cv.onclick = function(evt){
      const rect = cv.getBoundingClientRect(); const x = evt.clientX-rect.left, y = evt.clientY-rect.top;
      let best=null, bd=1e9; nodes.forEach(n=>{ const dx=n.x-x, dy=n.y-y; const d=dx*dx+dy*dy; if(d<bd){bd=d; best=n;} });
      if(best && bd <= (14*14)){
        const d = IMPACT_DATA.nodes.find(nn=>nn.data.id===best.id).data;
        document.getElementById('p-title').textContent = d.label;
        document.getElementById('p-id').textContent = d.id;
        document.getElementById('p-file').textContent = d.file+':'+d.line;
        document.getElementById('p-kind').textContent = d.kind + (d.changed? ' (changed)':'');
        document.getElementById('p-depth').textContent = (DIST.has(d.id)? DIST.get(d.id) : 'n/a');
        document.getElementById('popup').style.display='block';
      }
    };
  }

  // Toggle
  document.getElementById('toggle-canvas').onclick = renderWithCanvas;

  if(hasCytoscape()) { renderWithCytoscape(); }
  else {
    // try loading from CDN, then render
    const s = document.createElement('script');
    s.src = 'https://unpkg.com/cytoscape@3/dist/cytoscape.min.js';
    s.onload = renderWithCytoscape;
    s.onerror = function(){ renderWithCanvas(); };
    document.head.appendChild(s);
  }
})();
</script>
<div id="popup" class="popup">
  <h3 id="p-title"></h3>
  <div class=row><strong>ID:</strong> <code id="p-id"></code></div>
  <div class=row><strong>File:</strong> <code id="p-file"></code></div>
  <div class=row><strong>Kind:</strong> <code id="p-kind"></code></div>
  <div class=row><strong>Depth:</strong> <code id="p-depth"></code></div>
  <div class=actions>
    <button id="p-copy">copy id</button>
    <button id="p-close">close</button>
  </div>
</div>
"#);

    html.push_str("</html>");
    html
}

fn h(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
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
