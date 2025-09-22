self.onmessage = function(ev){
  const msg = ev && ev.data ? ev.data : {};
  if(msg.cmd !== 'compute'){ return; }
  const dir = msg.dir || 'undirected';
  const roots = (Array.isArray(msg.roots) && msg.roots.length) ? msg.roots.slice() : [];
  const changed = Array.isArray(msg.changed) ? msg.changed.slice() : [];
  const start = roots.length ? roots : changed;
  const nodes = Array.isArray(msg.nodes) ? msg.nodes.slice() : [];
  const edges = Array.isArray(msg.edges) ? msg.edges.slice() : [];
  const impacted = Array.isArray(msg.impacted) ? msg.impacted.slice() : [];
  const adj = buildAdjacency(dir, nodes, edges);
  const bfs = runBfs(adj, start);
  const pairs = computePairs(adj, bfs.parent, new Set(start), impacted);
  const distArr = Array.from(bfs.dist.entries());
  const pairArr = Array.from(pairs);
  self.postMessage({ dist: distArr, pairs: pairArr });
};

function buildAdjacency(dir, nodes, edges){
  const adj = new Map();
  nodes.forEach((id)=>{ if(!adj.has(id)){ adj.set(id, []); } });
  edges.forEach((edge)=>{
    if(!edge || typeof edge.s !== 'string' || typeof edge.t !== 'string'){ return; }
    const s = edge.s;
    const t = edge.t;
    if(!adj.has(s)){ adj.set(s, []); }
    if(!adj.has(t)){ adj.set(t, []); }
    if(dir === 'callees' || dir === 'undirected'){ adj.get(s).push(t); }
    if(dir === 'callers' || dir === 'undirected'){ adj.get(t).push(s); }
  });
  return adj;
}

function runBfs(adj, roots){
  const queue = [];
  const dist = new Map();
  const parent = new Map();
  const seen = new Set();
  (roots || []).forEach((id)=>{
    if(!seen.has(id)){
      seen.add(id);
      dist.set(id, 0);
      parent.set(id, null);
      queue.push(id);
    }
  });
  while(queue.length){
    const u = queue.shift();
    const base = dist.get(u) || 0;
    const neigh = adj.get(u) || [];
    for(let i=0;i<neigh.length;i++){
      const v = neigh[i];
      if(!seen.has(v)){
        seen.add(v);
        dist.set(v, base + 1);
        parent.set(v, u);
        queue.push(v);
      }
    }
  }
  return { dist, parent };
}

function computePairs(adj, parent, roots, impacted){
  const pairs = new Set();
  if(!impacted || !impacted.length){ return pairs; }
  impacted.forEach((node)=>{
    if(!parent.has(node) && !roots.has(node)){ return; }
    let cur = node;
    while(parent.has(cur)){
      const p = parent.get(cur);
      if(p == null){ break; }
      const key = cur + "\t" + p;
      const rev = p + "\t" + cur;
      pairs.add(key);
      pairs.add(rev);
      if(roots.has(p)){ break; }
      cur = p;
    }
  });
  return pairs;
}
