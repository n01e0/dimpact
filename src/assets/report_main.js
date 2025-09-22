(function(){
  function hasCytoscape(){ return typeof window.cytoscape === 'function'; }
  function showBusy(){ const b=document.getElementById('busy'); if(b){ b.style.display='flex'; } }
  function hideBusy(){ const b=document.getElementById('busy'); if(b){ b.style.display='none'; } }

  const N_NODES = (IMPACT_DATA && IMPACT_DATA.nodes ? IMPACT_DATA.nodes.length : 0);
  const N_EDGES = (IMPACT_DATA && IMPACT_DATA.edges ? IMPACT_DATA.edges.length : 0);
  const HEAVY = (N_NODES > 800) || (N_EDGES > 1500);
  const DEGREE = (function(){ const m=new Map(); IMPACT_DATA.nodes.forEach(n=>m.set(n.data.id,0)); IMPACT_DATA.edges.forEach(e=>{ m.set(e.data.source,(m.get(e.data.source)||0)+1); m.set(e.data.target,(m.get(e.data.target)||0)+1); }); return m; })();

  function getFilterState(){
    const kinds = Array.from(document.querySelectorAll('input.kind:checked')).map(x=>x.value);
    return {
      changed: document.getElementById('f_changed').checked,
      impacted: document.getElementById('f_impacted').checked,
      dir: (function(){ const r=document.querySelector('input[name=dir]:checked'); return r?r.value:'undirected'; })(),
      roots: Array.from(document.querySelectorAll('#root-list input[type=checkbox]:checked')).map(x=>x.value),
      kinds,
      depth: (function(){ const v = document.getElementById('f_depth').value; return v===''?null:Math.max(0, parseInt(v,10)||0); })(),
      reach: document.getElementById('f_reach') ? document.getElementById('f_reach').checked : true,
      file: document.getElementById('f_file').value.trim().toLowerCase()
    };
  }

  function buildAdj(dir){
    const adj = new Map();
    IMPACT_DATA.nodes.forEach(n=>{ adj.set(n.data.id, new Set()); });
    IMPACT_DATA.edges.forEach(e=>{
      const s = e.data.source, t = e.data.target;
      if(dir==='callees' || dir==='undirected') (adj.get(s)||new Set()).add(t);
      if(dir==='callers' || dir==='undirected') (adj.get(t)||new Set()).add(s);
    });
    return adj;
  }
  function computeDistances(dir){
    const adj = buildAdj(dir);
    const q=[]; const dist=new Map();
    const roots = getFilterState().roots;
    const src = (roots && roots.length) ? roots : IMPACT_DATA.nodes.filter(n=>n.data.changed).map(n=>n.data.id);
    src.forEach(id=>{ dist.set(id,0); q.push(id); });
    while(q.length){ const u=q.shift(); const d=dist.get(u)||0; const neigh=Array.from(adj.get(u)||[]); for(const v of neigh){ if(!dist.has(v)){ dist.set(v,d+1); q.push(v);} } }
    return dist; // id->distance
  }
  function computeParents(dir){
    const adj = buildAdj(dir);
    const q=[]; const parent=new Map(); const seen=new Set();
    const roots = getFilterState().roots;
    const src = (roots && roots.length) ? roots : IMPACT_DATA.nodes.filter(n=>n.data.changed).map(n=>n.data.id);
    src.forEach(id=>{ seen.add(id); parent.set(id, null); q.push(id); });
    while(q.length){ const u=q.shift(); for(const v of (adj.get(u)||[])){ if(!seen.has(v)){ seen.add(v); parent.set(v,u); q.push(v); } } }
    return parent; // id -> parent id or null for changed
  }
  function computePathPairs(dir){
    const P = computeParents(dir);
    const impacted = IMPACT_DATA.nodes.filter(n=>!n.data.changed).map(n=>n.data.id);
    const pairs = new Set();
    function addPair(a,b){ pairs.add(a+"\t"+b); pairs.add(b+"\t"+a); }
    for(const t of impacted){ if(!P.has(t)) continue; let u=t; let p=P.get(u); while(p){ addPair(u,p); u=p; p=P.get(u); } }
    return pairs;
  }

  const EXPAND = new Set();
  function expandedVisible(dir){ const adj = buildAdj(dir); const vis=new Set(); EXPAND.forEach(id=>{ vis.add(id); (adj.get(id)||[]).forEach(v=>vis.add(v)); }); return vis; }

  const WORKER = (function(){
    try { const blob = new Blob([WORKER_SRC], {type: 'text/javascript'}); return new Worker(URL.createObjectURL(blob)); } catch(e) { return null; }
  })();
  function computeAsync(dir){
    return new Promise((resolve)=>{
      if(!WORKER){ const dist = computeDistances(dir); const PP = computePathPairs(dir); resolve({ dist, pairs: PP }); return; }
      const roots = Array.from(document.querySelectorAll('#root-list input[type=checkbox]:checked')).map(x=>x.value);
      const nodes = IMPACT_DATA.nodes.map(n=>n.data.id);
      const edges = IMPACT_DATA.edges.map(e=>({s:e.data.source, t:e.data.target}));
      const impacted = IMPACT_DATA.nodes.filter(n=>!n.data.changed).map(n=>n.data.id);
      WORKER.onmessage = function(ev){ const d = ev.data||{}; const dist = new Map(d.dist||[]); const pairs = new Set(d.pairs||[]); resolve({ dist, pairs }); };
      WORKER.postMessage({ cmd: 'compute', dir, roots, nodes, edges, impacted });
    });
  }

  async function renderWithCytoscape(){
    const el = document.getElementById('viz'); el.style.display = 'block';
    const cv = document.getElementById('canvas'); if (cv) cv.style.display = 'none';
    const cy = cytoscape({ container: el, elements: IMPACT_DATA, style: [
      { selector: 'node', style: { 'label': 'data(label)', 'font-size': 10, 'text-valign': 'center', 'text-halign': 'center', 'background-color': '#eef', 'border-width': 1, 'border-color': '#bbf', 'width': 22, 'height': 22 }},
      { selector: 'node[changed = true]', style: { 'background-color': '#fee', 'border-color': '#fbb' }},
      { selector: 'edge', style: { 'width': 1, 'line-color': '#ccc', 'target-arrow-color': '#ccc', 'target-arrow-shape': 'triangle', 'curve-style': 'bezier' }},
      { selector: 'edge.path', style: { 'line-color': '#e33', 'target-arrow-color': '#e33', 'width': 2 } }
    ], layout: { name: 'breadthfirst', directed: true } });

    function runLayout(opts){ try{ showBusy(); }catch(_e){} const l = cy.layout(opts); l.on('layoutstop', ()=>{ try{ hideBusy(); }catch(_e){} }); l.run(); }
    document.getElementById('layout-bf').onclick = () => runLayout({name:'breadthfirst', directed:true});
    document.getElementById('layout-grid').onclick = () => runLayout({name:'grid'});
    document.getElementById('layout-cose').onclick = () => runLayout({name:'cose'});

    (async function(){ const f=getFilterState(); const R=await computeAsync(f.dir); const PP=R.pairs; cy.edges().forEach(e=>{ const id=e.data('source')+"\t"+e.data('target'); if(PP.has(id)){ e.addClass('path'); } else { e.removeClass('path'); } }); })();

    const popup = document.getElementById('popup');
    const pTitle = document.getElementById('p-title'); const pId = document.getElementById('p-id');
    const pFile = document.getElementById('p-file'); const pKind = document.getElementById('p-kind'); const pDepth = document.getElementById('p-depth');
    cy.on('tap', 'node', async (evt)=>{
      const d = evt.target.data(); pTitle.textContent = d.label; pId.textContent = d.id; pFile.textContent = d.file + ':' + d.line; pKind.textContent = d.kind + (d.changed? ' (changed)':'');
      const F = getFilterState(); const R = await computeAsync(F.dir); const DD = R.dist; pDepth.textContent = (DD.has(d.id)? DD.get(d.id) : 'n/a'); popup.style.display='block';
      const btnExp = document.getElementById('p-expand'); if(btnExp){ btnExp.onclick = ()=>{ EXPAND.add(d.id); applyFilters(); popup.style.display='none'; }; }
    });
    document.getElementById('p-copy').onclick = ()=>{ navigator.clipboard && navigator.clipboard.writeText(document.getElementById('p-id').textContent); };
    document.getElementById('p-close').onclick = ()=>{ popup.style.display='none'; };

    async function applyFilters(){
      const f = getFilterState(); try{ showBusy(); }catch(_e){}
      const R = await computeAsync(f.dir); const DIST = R.dist; const EXP = expandedVisible(f.dir);
      const visibleNode = new Set();
      cy.nodes().forEach(n=>{
        const d=n.data(); const passChanged = (d.changed && f.changed) || (!d.changed && f.impacted);
        const passKind = f.kinds.includes(String(d.kind||'')); const passFile = f.file==='' || String(d.file||'').toLowerCase().includes(f.file);
        const distVal = DIST.has(d.id)? DIST.get(d.id) : Infinity; const passDepth = (f.depth==null) || (distVal <= f.depth); const passReach = !f.reach || Number.isFinite(distVal);
        const show = passChanged && passKind && passFile && passDepth && passReach || EXP.has(d.id);
        if(show){ n.show(); visibleNode.add(d.id);} else { n.hide(); }
      });
      cy.edges().forEach(e=>{ const s=e.data('source'), t=e.data('target'); if(visibleNode.has(s) && visibleNode.has(t)) e.show(); else e.hide(); });
      const PP2 = R.pairs; cy.edges().forEach(e=>{ const id=e.data('source')+"\t"+e.data('target'); if(PP2.has(id) && e.visible()){ e.addClass('path'); } else { e.removeClass('path'); } });
      try{ hideBusy(); }catch(_e){}
    }
    document.getElementById('apply-filters').onclick = applyFilters;
    document.getElementById('reset-filters').onclick = ()=>{
      document.getElementById('f_changed').checked = true; document.getElementById('f_impacted').checked = true; document.querySelectorAll('input.kind').forEach(x=>x.checked=true);
      document.getElementById('f_depth').value = ''; document.getElementById('f_file').value = '';
      const rUndir = document.querySelector('input[name=dir][value=undirected]'); if(rUndir) rUndir.checked = true; const reach = document.getElementById('f_reach'); if(reach) reach.checked = true;
      const box = document.getElementById('root-list'); if(box){ const inputs = box.querySelectorAll('input[type=checkbox]'); inputs.forEach((el,idx)=>{ el.checked = idx < Math.min(10, inputs.length); }); }
      applyFilters();
    };
    applyFilters();
  }

  async function renderWithCanvas(){
    const cv = document.getElementById('canvas'); const viz = document.getElementById('viz'); viz.style.display = 'none'; cv.style.display = 'block';
    const w = cv.clientWidth || 800, h = cv.clientHeight || 520; cv.width = w; cv.height = h; const ctx = cv.getContext('2d');
    try{ showBusy(); }catch(_e){}; ctx.clearRect(0,0,w,h);
    const F = getFilterState(); const R = await computeAsync(F.dir); const DIST = R.dist; const PP = R.pairs;
    const nodes = IMPACT_DATA.nodes
      .filter(n=>{ const d=n.data; const passChanged = (d.changed && F.changed) || (!d.changed && F.impacted); const passKind = F.kinds.includes(String(d.kind||'')); const passFile = F.file==='' || String(d.file||'').toLowerCase().includes(F.file); const distVal = DIST.has(d.id)? DIST.get(d.id) : Infinity; const passDepth = (F.depth==null) || (distVal <= F.depth); const passReach = !F.reach || Number.isFinite(distVal); return passChanged && passKind && passFile && passDepth && passReach; })
      .map((n,i)=>({ id: n.data.id, label: n.data.label, changed: !!n.data.changed, x:0, y:0 }));
    const N = nodes.length, RAD = Math.max(80, Math.min(w,h)/2 - 40), cx = w/2, cy = h/2;
    for(let i=0;i<N;i++){ const a = (2*Math.PI*i)/N; nodes[i].x = cx + RAD*Math.cos(a); nodes[i].y = cy + RAD*Math.sin(a); }
    IMPACT_DATA.edges.forEach(e=>{ const s = nodes.find(n=>n.id===e.data.source), t = nodes.find(n=>n.id===e.data.target); if(!s||!t) return; const key = e.data.source+"\t"+e.data.target; const onPath = PP.has(key); ctx.beginPath(); ctx.strokeStyle = onPath? '#e33':'#ccc'; ctx.lineWidth = onPath? 2 : 1; ctx.moveTo(s.x, s.y); ctx.lineTo(t.x, t.y); ctx.stroke(); });
    nodes.forEach(n=>{ ctx.beginPath(); ctx.fillStyle = n.changed ? '#fee' : '#eef'; ctx.strokeStyle = n.changed ? '#fbb' : '#bbf'; ctx.arc(n.x, n.y, 12, 0, 2*Math.PI); ctx.fill(); ctx.stroke(); ctx.fillStyle = '#333'; ctx.font = '10px monospace'; ctx.textAlign = 'center'; ctx.fillText(n.label, n.x, n.y-16); });
    cv.onclick = async function(evt){ const rect = cv.getBoundingClientRect(); const x = evt.clientX-rect.left, y = evt.clientY-rect.top; let best=null, bd=1e9; nodes.forEach(n=>{ const dx=n.x-x, dy=n.y-y; const d=dx*dx+dy*dy; if(d<bd){bd=d; best=n;} }); if(best && bd <= (14*14)){ const d = IMPACT_DATA.nodes.find(nn=>nn.data.id===best.id).data; document.getElementById('p-title').textContent = d.label; document.getElementById('p-id').textContent = d.id; document.getElementById('p-file').textContent = d.file+':'+d.line; document.getElementById('p-kind').textContent = d.kind + (d.changed? ' (changed)':''); const RR = await computeAsync(F.dir); const DD = RR.dist; document.getElementById('p-depth').textContent = (DD.has(d.id)? DD.get(d.id) : 'n/a'); document.getElementById('popup').style.display='block'; } };
    try{ hideBusy(); }catch(_e){}
  }

  // Build root selector from data
  function degreeSort(a,b){ return (DEGREE.get(b.data.id)||0) - (DEGREE.get(a.data.id)||0); }
  function buildRootList(){ const box = document.getElementById('root-list'); if(!box) return; box.innerHTML=''; const q=(document.getElementById('root-search')?.value||'').toLowerCase(); const sortBy=document.getElementById('root-sort')?.value||'degree'; let changed=IMPACT_DATA.nodes.filter(n=>!!n.data.changed); if(q){ changed=changed.filter(n=> String(n.data.label||'').toLowerCase().includes(q) || String(n.data.file||'').toLowerCase().includes(q)); } if(sortBy==='label'){ changed.sort((a,b)=> String(a.data.label||'').localeCompare(String(b.data.label||''))); } else { changed.sort(degreeSort); } const LIMIT=200; const items=changed.slice(0,LIMIT); const TOPK=Math.min(10,items.length); items.forEach((n,i)=>{ const id=n.data.id; const label=n.data.label+' — '+(n.data.file||'')+':'+(n.data.line||''); const wrap=document.createElement('label'); wrap.className='small'; wrap.style.marginRight='8px'; const inp=document.createElement('input'); inp.type='checkbox'; inp.value=id; if(i<TOPK) inp.checked=true; wrap.appendChild(inp); wrap.appendChild(document.createTextNode(' '+label)); box.appendChild(wrap); }); const btnTop=document.getElementById('roots-top'); const btnNone=document.getElementById('roots-none'); const btnAll=document.getElementById('roots-all'); if(btnTop) btnTop.onclick=function(){ const inputs=box.querySelectorAll('input[type=checkbox]'); inputs.forEach((el,idx)=>{ el.checked=idx<Math.min(10,inputs.length); }); }; if(btnNone) btnNone.onclick=function(){ box.querySelectorAll('input[type=checkbox]').forEach(el=>{ el.checked=false; }); }; if(btnAll) btnAll.onclick=function(){ box.querySelectorAll('input[type=checkbox]').forEach(el=>{ el.checked=true; }); }; }

  // Wire roots search/sort
  function attachRootControls(){ const rs = document.getElementById('root-search'); if(rs){ rs.oninput = ()=>{ buildRootList(); }; } const rsort = document.getElementById('root-sort'); if(rsort){ rsort.onchange = ()=>{ buildRootList(); }; } }

  // Entry
  (function(){ if(HEAVY){ const depth=document.getElementById('f_depth'); if(depth) depth.value='2'; const reach=document.getElementById('f_reach'); if(reach) reach.checked=true; } buildRootList(); attachRootControls(); })();
  (async function(){ await renderWithCanvas(); if(!HEAVY && hasCytoscape()) { await renderWithCytoscape(); } else if(!HEAVY) { const s=document.createElement('script'); s.src='https://unpkg.com/cytoscape@3/dist/cytoscape.min.js'; s.onload = ()=>{ renderWithCytoscape(); }; document.head.appendChild(s); } })();
})();

