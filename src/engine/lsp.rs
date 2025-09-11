// moved from src/engine/lsp/mod.rs (flattened)
use crate::{ChangedOutput, FileChanges, LanguageMode, ImpactOptions, ImpactOutput};
use log::{debug, info, trace, warn};
use serde_json::json;

/// Minimal capability matrix placeholder for future LSP probing.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct CapabilityMatrix {
    pub call_hierarchy: bool,
    pub references: bool,
    pub definition: bool,
    pub document_symbol: bool,
    pub workspace_symbol: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LspConfig {
    pub strict: bool,
    pub dump_capabilities: bool,
    pub mock: bool,
    pub mock_caps: Option<super::CapsHint>,
}

/// Stub LSP session. Will later speak JSON-RPC over stdio.
pub struct LspSession {
    _cfg: LspConfig,
    pub capabilities: CapabilityMatrix,
    child: Option<std::process::Child>,
    stdin: Option<std::process::ChildStdin>,
    stdout: Option<std::process::ChildStdout>,
    next_id: std::sync::atomic::AtomicU64,
}

impl LspSession {
    pub fn new(lang: LanguageMode, cfg: LspConfig) -> anyhow::Result<Self> {
        info!("lsp: initializing session (strict={}, mock={})", cfg.strict, cfg.mock);
        // Test/CI escape hatch: disable real LSP usage when requested
        if std::env::var("DIMPACT_DISABLE_REAL_LSP").ok().as_deref() == Some("1") && !cfg.mock {
            anyhow::bail!("real LSP disabled by DIMPACT_DISABLE_REAL_LSP=1")
        }
        // Test hook: allow mocking LSP availability without real servers.
        if cfg.mock {
            let caps = if let Some(h) = cfg.mock_caps { CapabilityMatrix {
                call_hierarchy: h.call_hierarchy,
                references: h.references,
                definition: h.definition,
                document_symbol: h.document_symbol,
                workspace_symbol: h.workspace_symbol,
            }} else { CapabilityMatrix { call_hierarchy: true, references: true, definition: true, document_symbol: true, workspace_symbol: true } };
            return Ok(Self {
                _cfg: cfg,
                capabilities: caps,
                child: None,
                stdin: None,
                stdout: None,
                next_id: std::sync::atomic::AtomicU64::new(1),
            });
        }
        // Try to spawn a server for the given language
        let cmd = match lang {
            LanguageMode::Rust => Some(("rust-analyzer", vec![] as Vec<&str>)),
            LanguageMode::Ruby => Some(("ruby-lsp", vec![] as Vec<&str>)),
            LanguageMode::Javascript | LanguageMode::Typescript | LanguageMode::Tsx => Some(("typescript-language-server", vec!["--stdio"])),
            LanguageMode::Auto => None, // unknown until a file is opened; skip
        };
        let Some((exe, args)) = cmd else { anyhow::bail!("lsp server not determined for language") };
        let mut child = std::process::Command::new(exe)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()?;

        let mut stdin = child.stdin.take().ok_or_else(|| anyhow::anyhow!("no stdin"))?;
        let mut stdout = child.stdout.take().ok_or_else(|| anyhow::anyhow!("no stdout"))?;

        // Send initialize request with workspace root to help servers (e.g. rust-analyzer)
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let root_uri = path_to_uri(&cwd);
        let init = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "processId": std::process::id(),
                "rootUri": root_uri,
                "workspaceFolders": [ { "uri": root_uri, "name": "workspace" } ],
                "capabilities": {},
                "trace": "off",
            }
        });
        let buf = encode_jsonrpc_message(&init);
        use std::io::Write;
        stdin.write_all(&buf)?;

        // Read response with a small timeout
        use std::io::Read;
        let mut acc: Vec<u8> = Vec::new();
        let start = std::time::Instant::now();
        // Allow more time for real servers to initialize
        let timeout = std::time::Duration::from_millis(2000);
        loop {
            let mut tmp = [0u8; 4096];
            match stdout.read(&mut tmp) {
                Ok(0) => break,
                Ok(n) => {
                    acc.extend_from_slice(&tmp[..n]);
                    if let Ok((val, _used)) = decode_jsonrpc_message(&acc) {
                        // parse capabilities if present
                        let caps = val.get("result").and_then(|r| r.get("capabilities")).cloned().unwrap_or(json!({}));
                        let m = CapabilityMatrix {
                            call_hierarchy: caps.get("callHierarchyProvider").is_some(),
                            references: caps.get("referencesProvider").is_some(),
                            definition: caps.get("definitionProvider").is_some(),
                            document_symbol: caps.get("documentSymbolProvider").is_some(),
                            workspace_symbol: caps.get("workspaceSymbolProvider").is_some(),
                        };
                        // Log capabilities
                        info!("lsp: capabilities: {}", serde_json::to_string(&m).unwrap_or_default());
                        // Best-effort send initialized notification
                        let initialized = json!({"jsonrpc":"2.0","method":"initialized","params":{}});
                        let _ = stdin.write_all(&encode_jsonrpc_message(&initialized));
                        // Keep session handles to allow future requests
                        return Ok(Self {
                            _cfg: cfg,
                            capabilities: m,
                            child: Some(child),
                            stdin: Some(stdin),
                            stdout: Some(stdout),
                            next_id: std::sync::atomic::AtomicU64::new(2),
                        });
                    }
                }
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::WouldBlock {
                        // spin
                    } else {
                        break;
                    }
                }
            }
            if start.elapsed() > timeout { break; }
        }
        let _ = child.kill();
        anyhow::bail!("lsp initialize timeout or invalid response")
    }

    fn next_request_id(&self) -> u64 {
        self.next_id.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }

    #[allow(dead_code)]
    pub fn request(&mut self, method: &str, params: serde_json::Value, timeout_ms: u64) -> anyhow::Result<serde_json::Value> {
        if self._cfg.mock || self.stdin.is_none() || self.stdout.is_none() {
            anyhow::bail!("lsp request not available (mock or no io)")
        }
        let id = self.next_request_id();
        debug!("lsp: request id={} method={}", id, method);
        let req = json!({"jsonrpc":"2.0","id": id, "method": method, "params": params});
        let buf = encode_jsonrpc_message(&req);
        use std::io::Write;
        self.stdin.as_mut().unwrap().write_all(&buf)?;

        use std::io::Read;
        let mut acc: Vec<u8> = Vec::new();
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis(timeout_ms);
        loop {
            let mut tmp = [0u8; 8192];
            let n = self.stdout.as_mut().unwrap().read(&mut tmp)?;
            if n == 0 { anyhow::bail!("lsp server closed") }
            acc.extend_from_slice(&tmp[..n]);
            while let Ok((val, used)) = decode_jsonrpc_message(&acc) {
                acc.drain(..used);
                if val.get("id").and_then(|v| v.as_u64()) == Some(id) {
                    if val.get("error").is_some() { warn!("lsp: error for method {} id={}", method, id); anyhow::bail!("lsp error response") }
                    trace!("lsp: response id={} method={}", id, method);
                    return Ok(val.get("result").cloned().unwrap_or(json!({})));
                }
            }
            if start.elapsed() > timeout { anyhow::bail!("lsp request timeout") }
        }
    }

    #[allow(dead_code)]
    pub fn notify(&mut self, method: &str, params: serde_json::Value) -> anyhow::Result<()> {
        if self._cfg.mock || self.stdin.is_none() { return Ok(()); }
        debug!("lsp: notify method={}", method);
        let notif = json!({"jsonrpc":"2.0","method": method, "params": params});
        let buf = encode_jsonrpc_message(&notif);
        use std::io::Write;
        self.stdin.as_mut().unwrap().write_all(&buf)?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn shutdown(mut self) {
        if self._cfg.mock { return; }
        if let (Some(mut _stdin), Some(mut child)) = (self.stdin.take(), self.child.take()) {
            use std::io::Write;
            let _ = _stdin.write_all(&encode_jsonrpc_message(&json!({"jsonrpc":"2.0","id":9999,"method":"shutdown"})));
            let _ = _stdin.write_all(&encode_jsonrpc_message(&json!({"jsonrpc":"2.0","method":"exit"})));
            let _ = child.kill();
        }
    }

    /// Best-effort capability probe to validate server actually handles methods.
    /// For now, this only operates in mock mode by reflecting mock_caps; real server probing is TODO.
    pub fn probe_update(&mut self) {
        if let Some(h) = self._cfg.mock_caps {
            self.capabilities.call_hierarchy &= h.call_hierarchy;
            self.capabilities.references &= h.references;
            self.capabilities.definition &= h.definition;
            self.capabilities.document_symbol &= h.document_symbol;
            self.capabilities.workspace_symbol &= h.workspace_symbol;
        }
    }

    pub fn probe_files(&mut self, files: &[String]) {
        if self._cfg.mock { return; }
        if self.stdin.is_none() || self.stdout.is_none() { return; }
        // pick first Rust file, if any
        let rust = files.iter().find(|p| p.ends_with(".rs"));
        if let Some(path) = rust {
            let p = std::path::Path::new(path);
            if let Ok(abs) = std::fs::canonicalize(p) {
                let uri = path_to_uri(&abs);
                // Probe documentSymbol
                let _ = self.request("textDocument/documentSymbol", json!({"textDocument": {"uri": uri}}), 400)
                    .map(|_| { self.capabilities.document_symbol = true; });
                // Probe prepareCallHierarchy with a dummy position 0,0
                let _ = self.request("textDocument/prepareCallHierarchy", json!({"textDocument": {"uri": uri}, "position": {"line": 0, "character": 0}}), 400)
                    .map(|_| { self.capabilities.call_hierarchy = true; });
                // Probe references/definition quickly
                let pos = json!({"line": 0, "character": 0});
                let _ = self.request("textDocument/references", json!({"textDocument": {"uri": uri}, "position": pos, "context": {"includeDeclaration": true}}), 300)
                    .map(|_| { self.capabilities.references = true; });
                let _ = self.request("textDocument/definition", json!({"textDocument": {"uri": uri}, "position": pos}), 300)
                    .map(|_| { self.capabilities.definition = true; });
            }
        }
    }

    fn req_prepare_call_hierarchy(&mut self, uri: &str, line0: u32, character0: u32) -> anyhow::Result<Vec<serde_json::Value>> {
        let params = json!({"textDocument": {"uri": uri}, "position": {"line": line0, "character": character0}});
        let v = self.request("textDocument/prepareCallHierarchy", params, 700)?;
        Ok(v.as_array().cloned().unwrap_or_default())
    }

    fn req_incoming_calls(&mut self, item: &serde_json::Value) -> anyhow::Result<Vec<serde_json::Value>> {
        let params = json!({"item": item});
        let v = self.request("callHierarchy/incomingCalls", params, 1200)?;
        Ok(v.as_array().cloned().unwrap_or_default())
    }

    fn req_outgoing_calls(&mut self, item: &serde_json::Value) -> anyhow::Result<Vec<serde_json::Value>> {
        let params = json!({"item": item});
        let v = self.request("callHierarchy/outgoingCalls", params, 1200)?;
        Ok(v.as_array().cloned().unwrap_or_default())
    }

    fn req_document_symbol(&mut self, uri: &str) -> anyhow::Result<Vec<serde_json::Value>> {
        let params = json!({"textDocument": {"uri": uri}});
        let v = self.request("textDocument/documentSymbol", params, 800)?;
        Ok(v.as_array().cloned().unwrap_or_default())
    }

    fn req_definition(&mut self, uri: &str, line0: u32, character0: u32) -> anyhow::Result<Vec<serde_json::Value>> {
        let params = json!({"textDocument": {"uri": uri}, "position": {"line": line0, "character": character0}});
        let v = self.request("textDocument/definition", params, 800)?;
        let mut out = Vec::new();
        if let Some(arr) = v.as_array() {
            out.extend(arr.clone());
        } else if v.get("targetUri").is_some() {
            out.push(v.clone());
        }
        Ok(out)
    }

    fn req_references(&mut self, uri: &str, line0: u32, character0: u32) -> anyhow::Result<Vec<serde_json::Value>> {
        let params = json!({"textDocument": {"uri": uri}, "position": {"line": line0, "character": character0}, "context": {"includeDeclaration": false}});
        let v = self.request("textDocument/references", params, 1200)?;
        Ok(v.as_array().cloned().unwrap_or_default())
    }
}

// --- Minimal JSON-RPC 2.0 framing helpers (Content-Length based) ---

pub(crate) fn encode_jsonrpc_message(value: &serde_json::Value) -> Vec<u8> {
    let body = value.to_string();
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    let mut out = header.into_bytes();
    out.extend_from_slice(body.as_bytes());
    out
}

pub(crate) fn decode_jsonrpc_message(input: &[u8]) -> anyhow::Result<(serde_json::Value, usize)> {
    // Find header terminator CRLFCRLF
    let mut idx = None;
    for i in 0..input.len().saturating_sub(3) {
        if &input[i..i+4] == b"\r\n\r\n" { idx = Some(i); break; }
    }
    let Some(hdr_end) = idx else { anyhow::bail!("incomplete header") };
    let header = std::str::from_utf8(&input[..hdr_end]).map_err(|e| anyhow::anyhow!(e))?;
    let mut content_len: Option<usize> = None;
    for line in header.split("\r\n") {
        if let Some(rest) = line.strip_prefix("Content-Length:") {
            let n = rest.trim().parse::<usize>().map_err(|e| anyhow::anyhow!(e))?;
            content_len = Some(n);
        }
    }
    let len = content_len.ok_or_else(|| anyhow::anyhow!("missing Content-Length"))?;
    let body_start = hdr_end + 4;
    if input.len() < body_start + len { anyhow::bail!("incomplete body") }
    let body = &input[body_start..body_start+len];
    let value: serde_json::Value = serde_json::from_slice(body)?;
    Ok((value, body_start + len))
}

#[derive(Default)]
pub struct LspEngine {
    cfg: super::EngineConfig,
    fallback: super::ts::TsEngine,
}

impl LspEngine {
    pub fn new(cfg: super::EngineConfig) -> Self { Self { cfg, fallback: super::ts::TsEngine } }
}

impl super::AnalysisEngine for LspEngine {
    fn changed_symbols(&self, diffs: &[FileChanges], lang: LanguageMode) -> anyhow::Result<ChangedOutput> {
        info!(
            "engine.lsp.changed_symbols: strict={}, files={}",
            self.cfg.lsp_strict,
            diffs.len()
        );
        // Hybrid policy: 非strict では変更点抽出はTSに委譲（堅牢）
        if !self.cfg.lsp_strict {
            if self.cfg.dump_capabilities {
                // ベストエフォートでcapabilitiesをダンプ
                let lsp_cfg = LspConfig { strict: false, dump_capabilities: true, mock: self.cfg.mock_lsp, mock_caps: self.cfg.mock_caps };
                match LspSession::new(lang, lsp_cfg) {
                    Ok(mut s) => { s.probe_update(); eprintln!("{}", serde_json::to_string(&s.capabilities).unwrap_or_else(|_| "{}".to_string())); },
                    Err(_) => { eprintln!("{}", serde_json::to_string(&CapabilityMatrix::default()).unwrap_or_else(|_| "{}".to_string())); }
                }
            }
            return self.fallback.changed_symbols(diffs, lang);
        }
        let lsp_cfg = LspConfig { strict: self.cfg.lsp_strict, dump_capabilities: self.cfg.dump_capabilities, mock: self.cfg.mock_lsp, mock_caps: self.cfg.mock_caps };
        match LspSession::new(lang, lsp_cfg) {
            Ok(mut _sess) => {
                _sess.probe_update();
                let files_list: Vec<String> = diffs.iter().filter_map(|fc| fc.new_path.clone()).collect();
                _sess.probe_files(&files_list);
                if self.cfg.dump_capabilities {
                    eprintln!("{}", serde_json::to_string(&_sess.capabilities).unwrap_or_else(|_| "{}".to_string()));
                }
                // Strategy selection
                match decide_changed_strategy(&_sess.capabilities) {
                    ChangedStrategy::DocumentSymbol | ChangedStrategy::WorkspaceSymbol => {
                        let out = lsp_changed_symbols(&mut _sess, diffs, lang)?;
                        if out.changed_symbols.is_empty() {
                            if self.cfg.lsp_strict { anyhow::bail!("lsp documentSymbol returned no symbols; strict mode") }
                            else { self.fallback.changed_symbols(diffs, lang) }
                        } else { Ok(out) }
                    }
                    ChangedStrategy::TsFallback => {
                        if self.cfg.lsp_strict { anyhow::bail!("lsp: no suitable symbol capability; strict mode") }
                        else { self.fallback.changed_symbols(diffs, lang) }
                    }
                }
            }
            Err(e) => {
                if self.cfg.dump_capabilities {
                    eprintln!("{}", serde_json::to_string(&CapabilityMatrix::default()).unwrap_or_else(|_| "{}".to_string()));
                }
                if self.cfg.lsp_strict { Err(e) } else { self.fallback.changed_symbols(diffs, lang) }
            }
        }
    }

    fn impact(&self, diffs: &[FileChanges], lang: LanguageMode, opts: &ImpactOptions) -> anyhow::Result<ImpactOutput> {
        info!(
            "engine.lsp.impact: strict={}, direction={:?}, max_depth={:?}",
            self.cfg.lsp_strict,
            opts.direction,
            opts.max_depth
        );
        if !self.cfg.lsp_strict && self.cfg.dump_capabilities {
            // Print capabilities for diagnostics even if we fallback computation
            let lsp_cfg = LspConfig { strict: false, dump_capabilities: true, mock: self.cfg.mock_lsp, mock_caps: self.cfg.mock_caps };
            match LspSession::new(lang, lsp_cfg) {
                Ok(mut s) => { s.probe_update(); eprintln!("{}", serde_json::to_string(&s.capabilities).unwrap_or_else(|_| "{}".to_string())); },
                Err(_) => { eprintln!("{}", serde_json::to_string(&CapabilityMatrix::default()).unwrap_or_else(|_| "{}".to_string())); }
            }
        }
        if !self.cfg.lsp_strict && !self.cfg.mock_lsp {
            return self.fallback.impact(diffs, lang, opts);
        }
        // Attempt LSP impact; if session init fails, fallback only when not strict
        let lsp_cfg = LspConfig { strict: self.cfg.lsp_strict, dump_capabilities: self.cfg.dump_capabilities, mock: self.cfg.mock_lsp, mock_caps: self.cfg.mock_caps };
        match LspSession::new(lang, lsp_cfg) {
            Ok(mut _sess) => {
                _sess.probe_update();
                let files_list: Vec<String> = diffs.iter().filter_map(|fc| fc.new_path.clone()).collect();
                _sess.probe_files(&files_list);
                if self.cfg.dump_capabilities {
                    eprintln!("{}", serde_json::to_string(&_sess.capabilities).unwrap_or_else(|_| "{}".to_string()));
                }
                // Use callHierarchy BFS when available; else fallback/strict error
                if _sess.capabilities.call_hierarchy {
                    let changed = lsp_changed_symbols(&mut _sess, diffs, lang)?;
                    if _sess._cfg.mock {
                        // In mock mode, fall back to TS graph impact for determinism in tests
                        let (index, refs) = crate::impact::build_project_graph()?;
                        return Ok(crate::impact::compute_impact(&changed.changed_symbols, &index, &refs, opts));
                    }
                    let out = lsp_impact_bfs(&mut _sess, changed.changed_symbols.clone(), opts);
                    match out {
                        Ok(o) if !o.impacted_symbols.is_empty() || changed.changed_symbols.is_empty() => Ok(o),
                        Ok(mut o_empty) => {
                            // LSP内フォールバック: references/definition ベースを strict/非strict を問わず試す（Callers/Both）
                            if matches!(opts.direction, crate::impact::ImpactDirection::Callers | crate::impact::ImpactDirection::Both) && (_sess.capabilities.references || _sess.capabilities.definition) {
                                let out2 = lsp_impact_references(&mut _sess, changed.changed_symbols.clone(), opts)?;
                                if !out2.impacted_symbols.is_empty() { return Ok(out2); }
                                // 両方向の場合はcallee側も補完する
                            }
                            if matches!(opts.direction, crate::impact::ImpactDirection::Callees | crate::impact::ImpactDirection::Both) {
                                let (callees, extra_edges) = scan_callees_for_changed(&mut _sess, &changed.changed_symbols);
                                if !callees.is_empty() {
                                    // マージして返す（strictでもOK）
                                    o_empty.impacted_symbols = callees;
                                    if opts.with_edges.unwrap_or(false) { o_empty.edges = extra_edges; }
                                    // impacted_files再計算
                                    let mut files: Vec<String> = o_empty.impacted_symbols.iter().map(|s| s.file.clone()).collect();
                                    files.sort(); files.dedup();
                                    o_empty.impacted_files = files;
                                    return Ok(o_empty);
                                }
                            }
                            // LSPのみでプロジェクトグラフを構築（TS相当）してimpactを算出（strictでもOK）
                            if o_empty.impacted_symbols.is_empty()
                                && !changed.changed_symbols.is_empty()
                                && let Ok((index, refs)) = lsp_build_project_graph(&mut _sess)
                            {
                                let out2 = crate::impact::compute_impact(&changed.changed_symbols, &index, &refs, opts);
                                return Ok(out2);
                            }
                            // 非strictのみTSフォールバック
                            if self.cfg.lsp_strict { Ok(o_empty) } else { self.fallback.impact(diffs, lang, opts) }
                        }
                        Err(e) => {
                            // まずはLSP内のreferencesルートへ（strictでもOK; Callers/Both）
                            if matches!(opts.direction, crate::impact::ImpactDirection::Callers | crate::impact::ImpactDirection::Both) && (_sess.capabilities.references || _sess.capabilities.definition) {
                                let out2 = lsp_impact_references(&mut _sess, changed.changed_symbols.clone(), opts)?;
                                if !out2.impacted_symbols.is_empty() || self.cfg.lsp_strict { return Ok(out2); }
                            }
                            if matches!(opts.direction, crate::impact::ImpactDirection::Callees | crate::impact::ImpactDirection::Both) {
                                let (callees, extra_edges) = scan_callees_for_changed(&mut _sess, &changed.changed_symbols);
                                if !callees.is_empty() {
                                    let mut files: Vec<String> = callees.iter().map(|s| s.file.clone()).collect();
                                    files.sort(); files.dedup();
                                    let edges = if opts.with_edges.unwrap_or(false) { extra_edges } else { Vec::new() };
                                    let mut impacted_by_file: std::collections::HashMap<String, Vec<crate::ir::Symbol>> = std::collections::HashMap::new();
                                    for s in &callees { impacted_by_file.entry(s.file.clone()).or_default().push(s.clone()); }
                                    for v in impacted_by_file.values_mut() { v.sort_by(|a,b| a.id.0.cmp(&b.id.0)); v.dedup_by(|a,b| a.id.0 == b.id.0); }
                                    return Ok(crate::impact::ImpactOutput { changed_symbols: changed.changed_symbols.clone(), impacted_symbols: callees, impacted_files: files, edges, impacted_by_file });
                                }
                            }
                            // LSPでの全体グラフ構築にトライ
                            if let Ok((index, refs)) = lsp_build_project_graph(&mut _sess) {
                                let out2 = crate::impact::compute_impact(&changed.changed_symbols, &index, &refs, opts);
                                if !out2.impacted_symbols.is_empty() || self.cfg.lsp_strict { return Ok(out2); }
                            }
                            if self.cfg.lsp_strict { Err(e) } else { self.fallback.impact(diffs, lang, opts) }
                        }
                    }
                } else if _sess.capabilities.references || _sess.capabilities.definition {
                    if matches!(opts.direction, crate::impact::ImpactDirection::Callees | crate::impact::ImpactDirection::Both) {
                        if self.cfg.lsp_strict { anyhow::bail!("lsp impact callees/both via references not implemented; strict mode") } else { return self.fallback.impact(diffs, lang, opts); }
                    }
                    let changed = lsp_changed_symbols(&mut _sess, diffs, lang)?;
                    let out = lsp_impact_references(&mut _sess, changed.changed_symbols.clone(), opts)?;
                    Ok(out)
            } else if self.cfg.lsp_strict { anyhow::bail!("lsp: no suitable impact capability; strict mode") } else { self.fallback.impact(diffs, lang, opts) }
            }
            Err(e) => {
                if self.cfg.dump_capabilities {
                    eprintln!("{}", serde_json::to_string(&CapabilityMatrix::default()).unwrap_or_else(|_| "{}".to_string()));
                }
                if self.cfg.lsp_strict { Err(e) } else { self.fallback.impact(diffs, lang, opts) }
            }
        }
    }

    fn impact_from_symbols(&self, changed: &[crate::ir::Symbol], lang: LanguageMode, opts: &ImpactOptions) -> anyhow::Result<ImpactOutput> {
        info!(
            "engine.lsp.impact_from_symbols: strict={}, seeds={} dir={:?}",
            self.cfg.lsp_strict,
            changed.len(),
            opts.direction
        );
        let lsp_cfg = LspConfig { strict: self.cfg.lsp_strict, dump_capabilities: self.cfg.dump_capabilities, mock: self.cfg.mock_lsp, mock_caps: self.cfg.mock_caps };
        let mut sess = LspSession::new(lang, lsp_cfg)?;
        sess.probe_update();
        if self.cfg.dump_capabilities { eprintln!("{}", serde_json::to_string(&sess.capabilities).unwrap_or_else(|_| "{}".to_string())); }
        // prefer callHierarchy BFS
        if sess.capabilities.call_hierarchy {
            let out = lsp_impact_bfs(&mut sess, changed.to_vec(), opts);
            match out {
                Ok(o) if !o.impacted_symbols.is_empty() || changed.is_empty() => Ok(o),
                Ok(o_empty) => {
                    // fall back to full LSP graph
                    if let Ok((index, refs)) = lsp_build_project_graph(&mut sess) { return Ok(crate::impact::compute_impact(changed, &index, &refs, opts)); }
                    Ok(o_empty)
                }
                Err(_) => {
                    if let Ok((index, refs)) = lsp_build_project_graph(&mut sess) { return Ok(crate::impact::compute_impact(changed, &index, &refs, opts)); }
                    anyhow::bail!("lsp impact_from_symbols failed")
                }
            }
        } else if sess.capabilities.references || sess.capabilities.definition {
            let out = lsp_impact_references(&mut sess, changed.to_vec(), opts)?;
            Ok(out)
        } else if self.cfg.lsp_strict { anyhow::bail!("lsp: no suitable capabilities for impact_from_symbols") } else { self.fallback.impact_from_symbols(changed, lang, opts) }
    }
}

fn item_to_symbol(item: &serde_json::Value) -> Option<crate::ir::Symbol> {
    let name = item.get("name")?.as_str()?.to_string();
    let kind = map_lsp_symbol_kind(item.get("kind")?.as_u64().unwrap_or(12));
    let uri = item.get("uri").and_then(|v| v.as_str()).or_else(|| item.get("from").and_then(|f| f.get("uri").and_then(|u| u.as_str())) )?;
    let file = uri_to_path(uri);
    let range_v = item.get("selectionRange").or_else(|| item.get("range"))?;
    let sl = range_v.get("start").and_then(|s| s.get("line")).and_then(|n| n.as_u64()).unwrap_or(0) as u32 + 1;
    let el = range_v.get("end").and_then(|s| s.get("line")).and_then(|n| n.as_u64()).unwrap_or(0) as u32;
    Some(crate::ir::Symbol {
        id: crate::ir::SymbolId::new("rust", &file, &kind, &name, sl),
        name,
        kind,
        file,
        range: crate::ir::TextRange { start_line: sl, end_line: el.max(sl) },
        language: "rust".to_string(),
    })
}

fn lsp_impact_bfs(sess: &mut LspSession, changed: Vec<crate::ir::Symbol>, opts: &crate::impact::ImpactOptions) -> anyhow::Result<crate::impact::ImpactOutput> {
    use std::collections::{HashSet, VecDeque};
    let mut q: VecDeque<(serde_json::Value, usize)> = VecDeque::new();
    let mut seen_keys: HashSet<String> = HashSet::new();
    // node set is not used in current algorithm; kept in references variant
    let mut node_map: std::collections::HashMap<String, crate::ir::Symbol> = std::collections::HashMap::new();
    let mut edges: Vec<crate::ir::reference::Reference> = Vec::new();

    // roots: prepareCallHierarchy for each changed symbol
    let mut seeded_roots = 0usize;
    for s in changed.iter() {
        if !s.file.ends_with(".rs") { continue; }
        let abspath = std::fs::canonicalize(&s.file).unwrap_or_else(|_| std::path::PathBuf::from(&s.file));
        let uri = path_to_uri(&abspath);
        if !sess._cfg.mock && let Ok(text) = std::fs::read_to_string(&abspath) {
            let _ = sess.notify("textDocument/didOpen", json!({
                "textDocument": { "uri": uri, "languageId": "rust", "version": 1, "text": text }
            }));
        }
        if matches!(s.kind, crate::ir::SymbolKind::Function | crate::ir::SymbolKind::Method) {
            // Directly seed from the changed callable itself
            let (mut line0, mut ch0) = guess_callable_position(&s.file, s).unwrap_or((s.range.start_line.saturating_sub(1), 0));
            if let Ok(defs) = sess.req_definition(&uri, line0, ch0)
                && let Some(loc) = defs.first()
                && let Some(r) = loc.get("targetSelectionRange").or_else(|| loc.get("range"))
                && let (Some(sl), Some(sc)) = (
                    r.get("start").and_then(|s| s.get("line")).and_then(|n| n.as_u64()),
                    r.get("start").and_then(|s| s.get("character")).and_then(|n| n.as_u64()),
                ) {
                line0 = sl as u32;
                ch0 = sc as u32;
            }
            let mut roots = sess.req_prepare_call_hierarchy(&uri, line0, ch0).unwrap_or_default();
            if roots.is_empty() && ch0 != 0 {
                roots = sess.req_prepare_call_hierarchy(&uri, line0, 0).unwrap_or_default();
            }
            for it in roots {
                let key = format!("{}:{}:{}", it.get("uri").and_then(|u| u.as_str()).unwrap_or(""), it.get("name").and_then(|n| n.as_str()).unwrap_or(""), it.get("kind").and_then(|k| k.as_u64()).unwrap_or(0));
                if seen_keys.insert(key) { q.push_back((it, 0)); seeded_roots += 1; }
            }
        } else {
            // Non-callable changes (enum/struct/module, etc): find their references,
            // then seed BFS from the enclosing callables at those reference sites.
            let defs = sess.req_definition(&uri, s.range.start_line.saturating_sub(1), 0).unwrap_or_default();
            let (def_uri, def_line0) = if let Some(loc) = defs.first() {
                let u = loc.get("uri").or_else(|| loc.get("targetUri")).and_then(|v| v.as_str()).unwrap_or(&uri).to_string();
                let r = loc.get("range").or_else(|| loc.get("targetSelectionRange"));
                let l0 = r.and_then(|rr| rr.get("start")).and_then(|st| st.get("line")).and_then(|n| n.as_u64()).unwrap_or(0) as u32;
                (u, l0)
            } else { (uri.clone(), s.range.start_line.saturating_sub(1)) };
            let refs = sess.req_references(&def_uri, def_line0, 0).unwrap_or_default();
            for loc in refs {
                let loc_uri = loc.get("uri").and_then(|v| v.as_str()).unwrap_or("");
                let file = uri_to_path(loc_uri);
                let line0 = loc.get("range").and_then(|r| r.get("start")).and_then(|st| st.get("line")).and_then(|n| n.as_u64()).unwrap_or(0) as u32;
                let items = sess.req_document_symbol(loc_uri).unwrap_or_default();
                if let Some(caller) = enclosing_symbol_in_doc(&items, &file, line0)
                    && matches!(caller.kind, crate::ir::SymbolKind::Function | crate::ir::SymbolKind::Method)
                {
                        let c_abs = std::fs::canonicalize(&caller.file).unwrap_or_else(|_| std::path::PathBuf::from(&caller.file));
                        let c_uri = path_to_uri(&c_abs);
                        let (l0, ch0) = guess_callable_position(&caller.file, &caller).unwrap_or((caller.range.start_line.saturating_sub(1), 0));
                        let mut roots = sess.req_prepare_call_hierarchy(&c_uri, l0, ch0).unwrap_or_default();
                        if roots.is_empty() && ch0 != 0 {
                            roots = sess.req_prepare_call_hierarchy(&c_uri, l0, 0).unwrap_or_default();
                        }
                        for it in roots {
                            let key = format!("{}:{}:{}", it.get("uri").and_then(|u| u.as_str()).unwrap_or(""), it.get("name").and_then(|n| n.as_str()).unwrap_or(""), it.get("kind").and_then(|k| k.as_u64()).unwrap_or(0));
                            if seen_keys.insert(key) { q.push_back((it, 0)); seeded_roots += 1; }
                        }
                }
            }
        }
    }
    // If no roots prepared, return empty impact (caller will decide LSP内フォールバックやTSフォールバック)
    if seeded_roots == 0 {
        let impacted_symbols: Vec<crate::ir::Symbol> = Vec::new();
        let impacted_files: Vec<String> = Vec::new();
        return Ok(crate::impact::ImpactOutput { changed_symbols: changed, impacted_symbols, impacted_files, edges: Vec::new(), impacted_by_file: std::collections::HashMap::new() });
    }

    while let Some((item, d)) = q.pop_front() {
        let cur_sym = if let Some(sym) = item_to_symbol(&item) { sym } else { continue };
        let cur_id = cur_sym.id.0.clone();
        node_map.entry(cur_id.clone()).or_insert(cur_sym.clone());
        if let Some(maxd) = opts.max_depth && d >= maxd { continue; }

        match opts.direction {
            crate::impact::ImpactDirection::Callers => {
                let mut env = EnqueueEnv { q: &mut q, edges: &mut edges, seen_keys: &mut seen_keys, node_map: &mut node_map };
                for inc in sess.req_incoming_calls(&item).unwrap_or_default() {
                    if let Some(from) = inc.get("from") { enqueue_edge(&mut env, from, &cur_sym, d+1, true); }
                }
                // Supplement callers via references to catch cases callHierarchy misses
                enqueue_callers_via_references(sess, &cur_sym, &mut q, &mut edges, &mut seen_keys, &mut node_map, d+1);
            }
            crate::impact::ImpactDirection::Callees => {
                let mut env = EnqueueEnv { q: &mut q, edges: &mut edges, seen_keys: &mut seen_keys, node_map: &mut node_map };
                for out in sess.req_outgoing_calls(&item).unwrap_or_default() {
                    if let Some(to) = out.get("to") { enqueue_edge(&mut env, to, &cur_sym, d+1, false); }
                }
                // Also scan body to enrich outgoing even when some were found
                let _ = scan_and_enqueue_callees(sess, &cur_sym, &mut q, &mut edges, &mut seen_keys, &mut node_map, d+1);
            }
            crate::impact::ImpactDirection::Both => {
                let mut env = EnqueueEnv { q: &mut q, edges: &mut edges, seen_keys: &mut seen_keys, node_map: &mut node_map };
                for inc in sess.req_incoming_calls(&item).unwrap_or_default() {
                    if let Some(from) = inc.get("from") { enqueue_edge(&mut env, from, &cur_sym, d+1, true); }
                }
                enqueue_callers_via_references(sess, &cur_sym, &mut q, &mut edges, &mut seen_keys, &mut node_map, d+1);
                let mut env2 = EnqueueEnv { q: &mut q, edges: &mut edges, seen_keys: &mut seen_keys, node_map: &mut node_map };
                for out in sess.req_outgoing_calls(&item).unwrap_or_default() {
                    if let Some(to) = out.get("to") { enqueue_edge(&mut env2, to, &cur_sym, d+1, false); }
                }
                let _ = scan_and_enqueue_callees(sess, &cur_sym, &mut q, &mut edges, &mut seen_keys, &mut node_map, d+1);
            }
        }
    }

    let changed_ids: HashSet<String> = changed.iter().map(|s| s.id.0.clone()).collect();
    let mut impacted_symbols: Vec<crate::ir::Symbol> = node_map.values().filter(|s| !changed_ids.contains(&s.id.0)).cloned().collect();
    impacted_symbols.sort_by(|a,b| a.id.0.cmp(&b.id.0));
    impacted_symbols.dedup_by(|a,b| a.id.0 == b.id.0);
    let mut impacted_files: Vec<String> = impacted_symbols.iter().map(|s| s.file.clone()).collect();
    impacted_files.sort(); impacted_files.dedup();
    let edges = if opts.with_edges.unwrap_or(false) { edges } else { Vec::new() };
    let mut impacted_by_file: std::collections::HashMap<String, Vec<crate::ir::Symbol>> = std::collections::HashMap::new();
    for s in &impacted_symbols { impacted_by_file.entry(s.file.clone()).or_default().push(s.clone()); }
    for v in impacted_by_file.values_mut() { v.sort_by(|a,b| a.id.0.cmp(&b.id.0)); v.dedup_by(|a,b| a.id.0 == b.id.0); }
    Ok(crate::impact::ImpactOutput { changed_symbols: changed, impacted_symbols, impacted_files, edges, impacted_by_file })
}

// Heuristic: scan the function source for simple callsites like `name(` or `path::name(`,
// then resolve definition via LSP and seed call hierarchy from there.
fn scan_and_enqueue_callees(
    sess: &mut LspSession,
    cur_sym: &crate::ir::Symbol,
    q: &mut std::collections::VecDeque<(serde_json::Value, usize)>,
    edges: &mut Vec<crate::ir::reference::Reference>,
    seen_keys: &mut std::collections::HashSet<String>,
    node_map: &mut std::collections::HashMap<String, crate::ir::Symbol>,
    next_depth: usize,
) -> usize {
    use std::io::Read;
    let mut added = 0usize;
    let path = std::path::Path::new(&cur_sym.file);
    let abspath = if path.is_absolute() { path.to_path_buf() } else { std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()) };
    let uri = path_to_uri(&abspath);
    let mut s = String::new();
    if let Ok(mut f) = std::fs::File::open(&abspath) { let _ = f.read_to_string(&mut s); }
    if s.is_empty() { return 0; }
    let start0 = cur_sym.range.start_line.saturating_sub(1) as usize;
    let end0 = cur_sym.range.end_line.saturating_sub(1) as usize;
    let lines: Vec<&str> = s.lines().collect();
    let mut seen_names: std::collections::HashSet<(u32,u32)> = std::collections::HashSet::new();
    for (li, line) in lines.iter().enumerate().take(end0+1).skip(start0) {
        let bytes = line.as_bytes();
        let mut i = 0usize;
        while i < bytes.len() {
            // identifier or path segment
            if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
                let mut last_seg_start = i;
                i += 1;
                while i < bytes.len() {
                    let c = bytes[i];
                    if c.is_ascii_alphanumeric() || c == b'_' { i += 1; continue; }
                    // Rust path ::
                    if i+1 < bytes.len() && c == b':' && bytes[i+1] == b':' {
                        i += 2; last_seg_start = i; continue;
                    }
                    // method call .name
                    if c == b'.' { i += 1; last_seg_start = i; continue; }
                    break;
                }
                // skip whitespace
                let mut j = i; while j < bytes.len() && bytes[j].is_ascii_whitespace() { j += 1; }
                if j < bytes.len() && bytes[j] == b'(' {
                    // crude keyword/macro filter
                    let name = &line[last_seg_start..i];
                    if !(name == "if" || name == "while" || name == "loop" || name == "match" || name == "for" || name == "return" || name == "fn" || name.ends_with('!')) {
                        // avoid self-edge on signature line or recursive detection by name-equality heuristic
                        if name == cur_sym.name && (li as u32 + 1) == cur_sym.range.start_line { i = j; continue; }
                        let line0 = li as u32; let ch0 = last_seg_start as u32;
                        if seen_names.insert((line0, ch0)) {
                            // try definition at name start
                    if let Ok(defs) = sess.req_definition(&uri, line0, ch0) {
                                for loc in defs {
                                    let u = loc.get("uri").or_else(|| loc.get("targetUri")).and_then(|v| v.as_str()).unwrap_or("");
                                    let r = loc.get("range").or_else(|| loc.get("targetSelectionRange"));
                                    if u.is_empty() || r.is_none() { continue; }
                                    let rs = r.unwrap().get("start").and_then(|st| st.get("line")).and_then(|n| n.as_u64()).unwrap_or(0) as u32;
                                    let rc = r.unwrap().get("start").and_then(|st| st.get("character")).and_then(|n| n.as_u64()).unwrap_or(0) as u32;
                                    // prepare hierarchy at callee definition
                                    let mut roots = sess.req_prepare_call_hierarchy(u, rs, rc).unwrap_or_default();
                                    if roots.is_empty() { roots = sess.req_prepare_call_hierarchy(u, rs, 0).unwrap_or_default(); }
                                    for it in roots {
                                        let key = format!("{}:{}:{}", it.get("uri").and_then(|uu| uu.as_str()).unwrap_or(""), it.get("name").and_then(|n| n.as_str()).unwrap_or(""), it.get("kind").and_then(|k| k.as_u64()).unwrap_or(0));
                                        if seen_keys.insert(key) {
                                            // enqueue node and edge cur_sym -> it
                                            q.push_back((it.clone(), next_depth));
                                            if let Some(sym_to) = item_to_symbol(&it) {
                                                node_map.entry(sym_to.id.0.clone()).or_insert(sym_to.clone());
                                                edges.push(crate::ir::reference::Reference { from: cur_sym.id.clone(), to: sym_to.id.clone(), kind: crate::ir::reference::RefKind::Call, file: cur_sym.file.clone(), line: li as u32 + 1 });
                                                added += 1;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                i = j;
                continue;
            }
            i += 1;
        }
    }
    added
}

fn enqueue_callers_via_references(
    sess: &mut LspSession,
    cur_sym: &crate::ir::Symbol,
    q: &mut std::collections::VecDeque<(serde_json::Value, usize)>,
    edges: &mut Vec<crate::ir::reference::Reference>,
    seen_keys: &mut std::collections::HashSet<String>,
    node_map: &mut std::collections::HashMap<String, crate::ir::Symbol>,
    next_depth: usize,
) {
    let uri = path_to_uri(std::path::Path::new(&cur_sym.file));
    let defs = sess.req_definition(&uri, cur_sym.range.start_line.saturating_sub(1), 0).unwrap_or_default();
    let (def_uri, def_line0) = if let Some(loc) = defs.first() {
        let u = loc.get("uri").or_else(|| loc.get("targetUri")).and_then(|v| v.as_str()).unwrap_or(&uri).to_string();
        let r = loc.get("range").or_else(|| loc.get("targetSelectionRange"));
        let l0 = r.and_then(|rr| rr.get("start")).and_then(|st| st.get("line")).and_then(|n| n.as_u64()).unwrap_or(0) as u32;
        (u, l0)
    } else { (uri.clone(), cur_sym.range.start_line.saturating_sub(1)) };
    let refs = sess.req_references(&def_uri, def_line0, 0).unwrap_or_default();
    for loc in refs {
        let loc_uri = loc.get("uri").and_then(|v| v.as_str()).unwrap_or("");
        let file = uri_to_path(loc_uri);
        let line0 = loc.get("range").and_then(|r| r.get("start")).and_then(|st| st.get("line")).and_then(|n| n.as_u64()).unwrap_or(0) as u32;
        let items = sess.req_document_symbol(loc_uri).unwrap_or_default();
        if let Some(caller) = enclosing_symbol_in_doc(&items, &file, line0)
            && matches!(caller.kind, crate::ir::SymbolKind::Function | crate::ir::SymbolKind::Method)
        {
                let c_abs = std::fs::canonicalize(&caller.file).unwrap_or_else(|_| std::path::PathBuf::from(&caller.file));
                let c_uri = path_to_uri(&c_abs);
                let (l0, ch0) = guess_callable_position(&caller.file, &caller).unwrap_or((caller.range.start_line.saturating_sub(1), 0));
                let mut roots = sess.req_prepare_call_hierarchy(&c_uri, l0, ch0).unwrap_or_default();
                if roots.is_empty() && ch0 != 0 { roots = sess.req_prepare_call_hierarchy(&c_uri, l0, 0).unwrap_or_default(); }
                for it in roots {
                    let key = format!("{}:{}:{}", it.get("uri").and_then(|u| u.as_str()).unwrap_or(""), it.get("name").and_then(|n| n.as_str()).unwrap_or(""), it.get("kind").and_then(|k| k.as_u64()).unwrap_or(0));
                    if seen_keys.insert(key) {
                        q.push_back((it.clone(), next_depth));
                        if let Some(sym_from) = item_to_symbol(&it) {
                            node_map.entry(sym_from.id.0.clone()).or_insert(sym_from.clone());
                            edges.push(crate::ir::reference::Reference { from: sym_from.id.clone(), to: cur_sym.id.clone(), kind: crate::ir::reference::RefKind::Call, file: sym_from.file.clone(), line: sym_from.range.start_line });
                        }
                    }
                }
        }
    }
}

// Scan callees for all changed callables and return one-hop callee symbols + edges
fn scan_callees_for_changed(sess: &mut LspSession, changed: &[crate::ir::Symbol]) -> (Vec<crate::ir::Symbol>, Vec<crate::ir::reference::Reference>) {
    let mut out_syms: Vec<crate::ir::Symbol> = Vec::new();
    let mut out_edges: Vec<crate::ir::reference::Reference> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for s in changed.iter().filter(|s| matches!(s.kind, crate::ir::SymbolKind::Function | crate::ir::SymbolKind::Method)) {
        let (syms, edges) = scan_callees_symbols(sess, s);
        for sym in syms {
            if seen.insert(sym.id.0.clone()) { out_syms.push(sym); }
        }
        out_edges.extend(edges);
    }
    (out_syms, out_edges)
}

// One-hop callee extraction using definitions + documentSymbol mapping
fn scan_callees_symbols(sess: &mut LspSession, cur_sym: &crate::ir::Symbol) -> (Vec<crate::ir::Symbol>, Vec<crate::ir::reference::Reference>) {
    use std::io::Read;
    let mut out_syms: Vec<crate::ir::Symbol> = Vec::new();
    let mut out_edges: Vec<crate::ir::reference::Reference> = Vec::new();
    let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    let path = std::path::Path::new(&cur_sym.file);
    let abspath = if path.is_absolute() { path.to_path_buf() } else { std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()) };
    let uri = path_to_uri(&abspath);
    let mut s = String::new();
    if let Ok(mut f) = std::fs::File::open(&abspath) { let _ = f.read_to_string(&mut s); }
    if s.is_empty() { return (out_syms, out_edges); }
    let start0 = cur_sym.range.start_line.saturating_sub(1) as usize;
    let end0 = cur_sym.range.end_line.saturating_sub(1) as usize;
    let lines: Vec<&str> = s.lines().collect();
    for (li, line) in lines.iter().enumerate().take(end0+1).skip(start0) {
        let bytes = line.as_bytes();
        let mut i = 0usize;
        while i < bytes.len() {
            if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
                let mut last_seg_start = i; i += 1;
                while i < bytes.len() {
                    let c = bytes[i];
                    if c.is_ascii_alphanumeric() || c == b'_' { i += 1; continue; }
                    if i+1 < bytes.len() && c == b':' && bytes[i+1] == b':' { i += 2; last_seg_start = i; continue; }
                    if c == b'.' { i += 1; last_seg_start = i; continue; }
                    break;
                }
                let mut j = i; while j < bytes.len() && bytes[j].is_ascii_whitespace() { j += 1; }
                if j < bytes.len() && bytes[j] == b'(' {
                    let name = &line[last_seg_start..i];
                    if !(name == "if" || name == "while" || name == "loop" || name == "match" || name == "for" || name == "return" || name == "fn" || name.ends_with('!')) {
                        if name == cur_sym.name && (li as u32 + 1) == cur_sym.range.start_line { i = j; continue; }
                        let line0 = li as u32; let ch0 = last_seg_start as u32;
                        // defs at callsite
                        let defs = sess.req_definition(&uri, line0, ch0).unwrap_or_default();
                        for loc in defs {
                            let def_uri = loc.get("uri").or_else(|| loc.get("targetUri")).and_then(|v| v.as_str()).unwrap_or("");
                            let def_file = uri_to_path(def_uri);
                            let r = loc.get("range").or_else(|| loc.get("targetSelectionRange"));
                            let def_l0 = r.and_then(|rr| rr.get("start")).and_then(|st| st.get("line")).and_then(|n| n.as_u64()).unwrap_or(0) as u32;
                            let items = sess.req_document_symbol(def_uri).unwrap_or_default();
                            if let Some(sym_to) = enclosing_symbol_in_doc(&items, &def_file, def_l0)
                                && (sym_to.id.0 != cur_sym.id.0)
                                && matches!(sym_to.kind, crate::ir::SymbolKind::Function | crate::ir::SymbolKind::Method)
                                && seen_ids.insert(sym_to.id.0.clone())
                            {
                                out_syms.push(sym_to.clone());
                                out_edges.push(crate::ir::reference::Reference { from: cur_sym.id.clone(), to: sym_to.id.clone(), kind: crate::ir::reference::RefKind::Call, file: cur_sym.file.clone(), line: li as u32 + 1 });
                            }
                        }
                    }
                }
                i = j; continue;
            }
            i += 1;
        }
    }
    (out_syms, out_edges)
}

fn guess_callable_position(file: &str, sym: &crate::ir::Symbol) -> Option<(u32,u32)> {
    use std::io::Read;
    let mut f = std::fs::File::open(file).ok()?;
    let mut s = String::new(); f.read_to_string(&mut s).ok()?;
    let line_idx = (sym.range.start_line.saturating_sub(1)) as usize;
    let line = s.lines().nth(line_idx)?;
    // Try to find exact name token start
    if let Some(pos) = line.find(&sym.name) {
        return Some((sym.range.start_line.saturating_sub(1), pos as u32));
    }
    // Try `fn name` pattern
    let pat = format!("fn {}", sym.name);
    if let Some(pos) = line.find(&pat) {
        return Some((sym.range.start_line.saturating_sub(1), (pos + 3) as u32));
    }
    Some((sym.range.start_line.saturating_sub(1), 0))
}

struct EnqueueEnv<'a> {
    q: &'a mut std::collections::VecDeque<(serde_json::Value, usize)>,
    edges: &'a mut Vec<crate::ir::reference::Reference>,
    seen_keys: &'a mut std::collections::HashSet<String>,
    node_map: &'a mut std::collections::HashMap<String, crate::ir::Symbol>,
}

fn enqueue_edge(
    env: &mut EnqueueEnv,
    next_item: &serde_json::Value,
    cur_sym: &crate::ir::Symbol,
    next_depth: usize,
    is_incoming: bool,
) {
    if let Some(sym) = item_to_symbol(next_item) {
        let key = sym.id.0.clone();
        if env.seen_keys.insert(key.clone()) {
            env.q.push_back((next_item.clone(), next_depth));
        }
        env.node_map.entry(key.clone()).or_insert(sym.clone());
        let (from, to) = if is_incoming { (sym.clone(), cur_sym.clone()) } else { (cur_sym.clone(), sym.clone()) };
        env.edges.push(crate::ir::reference::Reference { from: from.id.clone(), to: to.id.clone(), kind: crate::ir::reference::RefKind::Call, file: from.file.clone(), line: from.range.start_line });
    }
}

fn lsp_impact_references(sess: &mut LspSession, changed: Vec<crate::ir::Symbol>, opts: &crate::impact::ImpactOptions) -> anyhow::Result<crate::impact::ImpactOutput> {
    use std::collections::{HashSet, VecDeque};
    let mut q: VecDeque<(crate::ir::Symbol, usize)> = VecDeque::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut nodes: HashSet<String> = HashSet::new();
    let mut node_map: std::collections::HashMap<String, crate::ir::Symbol> = std::collections::HashMap::new();
    let mut edges: Vec<crate::ir::reference::Reference> = Vec::new();

    for s in changed.iter() { q.push_back((s.clone(), 0)); node_map.insert(s.id.0.clone(), s.clone()); }
    while let Some((sym, d)) = q.pop_front() {
        if let Some(maxd) = opts.max_depth && d >= maxd { continue; }
        let uri = path_to_uri(std::path::Path::new(&sym.file));
        // definition at a precise position (prefer symbol name offset)
        let (line0, ch0) = guess_callable_position(&sym.file, &sym).unwrap_or((sym.range.start_line.saturating_sub(1), 0));
        let defs = sess.req_definition(&uri, line0, ch0).unwrap_or_default();
    let (def_uri, def_line0) = if let Some(loc) = defs.first() {
            let u = loc.get("uri").or_else(|| loc.get("targetUri")).and_then(|v| v.as_str()).unwrap_or(&uri).to_string();
            let r = loc.get("range").or_else(|| loc.get("targetSelectionRange"));
            let l0 = r.and_then(|rr| rr.get("start")).and_then(|s| s.get("line")).and_then(|n| n.as_u64()).unwrap_or(0) as u32;
            (u, l0)
        } else { (uri.clone(), sym.range.start_line.saturating_sub(1)) };
        let refs = sess.req_references(&def_uri, def_line0, 0).unwrap_or_default();
        for loc in refs {
            let loc_uri = loc.get("uri").and_then(|v| v.as_str()).unwrap_or("");
            let file = uri_to_path(loc_uri);
            let line0 = loc.get("range").and_then(|r| r.get("start")).and_then(|s| s.get("line")).and_then(|n| n.as_u64()).unwrap_or(0) as u32;
            // find enclosing symbol via documentSymbol
            let items = sess.req_document_symbol(loc_uri).unwrap_or_default();
            if let Some(caller) = enclosing_symbol_in_doc(&items, &file, line0) {
                let key = caller.id.0.clone();
                node_map.entry(key.clone()).or_insert(caller.clone());
                if seen.insert(format!("edge:{}->{}", caller.id.0, sym.id.0)) { edges.push(crate::ir::reference::Reference { from: caller.id.clone(), to: sym.id.clone(), kind: crate::ir::reference::RefKind::Call, file: caller.file.clone(), line: caller.range.start_line }); }
                if !nodes.contains(&caller.id.0) { nodes.insert(caller.id.0.clone()); q.push_back((caller, d+1)); }
            }
        }
    }
    let changed_ids: HashSet<String> = changed.iter().map(|s| s.id.0.clone()).collect();
    let mut impacted_symbols: Vec<crate::ir::Symbol> = node_map.values().filter(|s| !changed_ids.contains(&s.id.0)).cloned().collect();
    impacted_symbols.sort_by(|a,b| a.id.0.cmp(&b.id.0));
    impacted_symbols.dedup_by(|a,b| a.id.0 == b.id.0);
    let mut impacted_files: Vec<String> = impacted_symbols.iter().map(|s| s.file.clone()).collect();
    impacted_files.sort(); impacted_files.dedup();
    let edges = if opts.with_edges.unwrap_or(false) { edges } else { Vec::new() };
    let mut impacted_by_file: std::collections::HashMap<String, Vec<crate::ir::Symbol>> = std::collections::HashMap::new();
    for s in &impacted_symbols { impacted_by_file.entry(s.file.clone()).or_default().push(s.clone()); }
    for v in impacted_by_file.values_mut() { v.sort_by(|a,b| a.id.0.cmp(&b.id.0)); v.dedup_by(|a,b| a.id.0 == b.id.0); }
    Ok(crate::impact::ImpactOutput { changed_symbols: changed, impacted_symbols, impacted_files, edges, impacted_by_file })
}

fn enclosing_symbol_in_doc(items: &[serde_json::Value], file: &str, line0: u32) -> Option<crate::ir::Symbol> {
    // Walk both DocumentSymbol (with children) and SymbolInformation
    for it in items {
        // DocumentSymbol path
        if let Some(r) = it.get("range")
            && let (Some(s), Some(e)) = (r.get("start"), r.get("end")) {
                let sl = s.get("line").and_then(|n| n.as_u64()).unwrap_or(0) as u32;
                let el = e.get("line").and_then(|n| n.as_u64()).unwrap_or(0) as u32;
                if sl <= line0 && line0 <= el {
                    let name = it.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let kind = map_lsp_symbol_kind(it.get("kind").and_then(|v| v.as_u64()).unwrap_or(12));
                    return Some(crate::ir::Symbol { id: crate::ir::SymbolId::new("rust", file, &kind, name, sl+1), name: name.to_string(), kind, file: file.to_string(), range: crate::ir::TextRange { start_line: sl+1, end_line: el.max(sl)+1 }, language: "rust".to_string() });
                }
        }
        // children
        if let Some(children) = it.get("children").and_then(|v| v.as_array())
            && let Some(sym) = enclosing_symbol_in_doc(children, file, line0) { return Some(sym); }
        // SymbolInformation path
        if let Some(loc) = it.get("location")
            && let Some(r) = loc.get("range") {
                let sl = r.get("start").and_then(|s| s.get("line")).and_then(|n| n.as_u64()).unwrap_or(0) as u32;
                let el = r.get("end").and_then(|s| s.get("line")).and_then(|n| n.as_u64()).unwrap_or(0) as u32;
                if sl <= line0 && line0 <= el {
                    let name = it.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let kind = map_lsp_symbol_kind(it.get("kind").and_then(|v| v.as_u64()).unwrap_or(12));
                    return Some(crate::ir::Symbol { id: crate::ir::SymbolId::new("rust", file, &kind, name, sl+1), name: name.to_string(), kind, file: file.to_string(), range: crate::ir::TextRange { start_line: sl+1, end_line: el.max(sl)+1 }, language: "rust".to_string() });
                }
        }
    }
    None
}

fn lsp_changed_symbols(sess: &mut LspSession, diffs: &[crate::FileChanges], lang: crate::mapping::LanguageMode) -> anyhow::Result<crate::mapping::ChangedOutput> {
    use std::collections::{HashMap, HashSet};
    if sess._cfg.mock {
        return crate::mapping::compute_changed_symbols(diffs, lang);
    }
    // collect changed files and changed line sets
    let mut changed_files: Vec<String> = Vec::new();
    let mut changed_lines_by_file: HashMap<String, HashSet<u32>> = HashMap::new();
    for fc in diffs.iter() {
        if let Some(path) = &fc.new_path { changed_files.push(path.clone()); }
        if let Some(path) = &fc.new_path {
            let set = changed_lines_by_file.entry(path.clone()).or_default();
            for ch in &fc.changes {
                if let Some(nl) = ch.new_line
                    && (matches!(ch.kind, crate::diff::ChangeKind::Added) || matches!(ch.kind, crate::diff::ChangeKind::Context))
                { set.insert(nl); }
            }
        }
    }
    changed_files.sort(); changed_files.dedup();
    let mut symbols: Vec<crate::ir::Symbol> = Vec::new();
    for (path, lines) in changed_lines_by_file.iter() {
        if !path.ends_with(".rs") { continue; }
        let abspath = std::fs::canonicalize(path).unwrap_or(std::path::PathBuf::from(path));
        let uri = path_to_uri(&abspath);
        let text = std::fs::read_to_string(&abspath).unwrap_or_else(|_| String::new());
        // didOpen
        let params = json!({
            "textDocument": {
                "uri": uri,
                "languageId": "rust",
                "version": 1,
                "text": text,
            }
        });
        let _ = sess.notify("textDocument/didOpen", params);
        // documentSymbol
        let params = json!({ "textDocument": { "uri": uri } });
        if let Ok(result) = sess.request("textDocument/documentSymbol", params, 500) {
            // Result can be DocumentSymbol[] or SymbolInformation[]
            if let Some(arr) = result.as_array() {
                for item in arr {
                    collect_symbols_from_item(path, item, &mut symbols, lines);
                }
            }
        }
    }
    Ok(crate::mapping::ChangedOutput { changed_files, changed_symbols: symbols })
}

fn path_to_uri(p: &std::path::Path) -> String {
    let mut s = String::from("file://");
    // crude percent-encoding for spaces only
    let ps = p.canonicalize().unwrap_or_else(|_| p.to_path_buf()).to_string_lossy().replace(' ', "%20");
    if cfg!(target_os = "windows") {
        s.push_str(&ps.replace('\\', "/"));
    } else { s.push_str(&ps); }
    s
}

fn uri_to_path(uri: &str) -> String {
    let raw = if let Some(rest) = uri.strip_prefix("file://") { rest.replace("%20", " ") } else { uri.to_string() };
    // Normalize to workspace-relative if possible
    match std::env::current_dir() {
        Ok(cwd) => {
            let cwd = cwd.canonicalize().unwrap_or(cwd);
            let rawp = std::path::Path::new(&raw);
            if rawp.is_absolute() {
                if let Ok(stripped) = rawp.canonicalize().unwrap_or_else(|_| rawp.to_path_buf()).strip_prefix(&cwd) {
                    let s = stripped.to_string_lossy().to_string();
                    if s.is_empty() { ".".to_string() } else { s }
                } else { raw }
            } else { raw }
        }
        Err(_) => raw,
    }
}

fn collect_symbols_from_item(path: &str, item: &serde_json::Value, out: &mut Vec<crate::ir::Symbol>, changed_lines: &std::collections::HashSet<u32>) {
    // DocumentSymbol form: { name, kind, range{start{line},end{line}}, children? }
    // SymbolInformation form: { name, kind, location{range{...}} }
    let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let kind_num = item.get("kind").and_then(|v| v.as_u64()).unwrap_or(12);
    // Only consider a whitelist of meaningful kinds to avoid picking up locals/variables etc.
    let allowed = matches!(kind_num, 2 | 5 | 6 | 9 | 10 | 12 | 23);
    let (start_line0, end_line0) = if let Some(r) = item.get("range") {
        (r.get("start").and_then(|s| s.get("line")).and_then(|n| n.as_u64()).unwrap_or(0), r.get("end").and_then(|e| e.get("line")).and_then(|n| n.as_u64()).unwrap_or(0))
    } else if let Some(loc) = item.get("location") {
        if let Some(r) = loc.get("range") {
            (r.get("start").and_then(|s| s.get("line")).and_then(|n| n.as_u64()).unwrap_or(0), r.get("end").and_then(|e| e.get("line")).and_then(|n| n.as_u64()).unwrap_or(0))
        } else { (0,0) }
    } else { (0,0) };
    let start_line = (start_line0 as u32) + 1;
    let mut end_line = end_line0 as u32; // LSP end is exclusive; convert to inclusive approx
    if end_line < start_line { end_line = start_line; }
    let kind = map_lsp_symbol_kind(kind_num);
    if allowed && !name.is_empty() && intersects_lines(start_line, end_line, changed_lines) {
        out.push(crate::ir::Symbol {
            id: crate::ir::SymbolId::new("rust", path, &kind, name, start_line),
            name: name.to_string(),
            kind,
            file: path.to_string(),
            range: crate::ir::TextRange { start_line, end_line },
            language: "rust".to_string(),
        });
    }
    if let Some(children) = item.get("children").and_then(|v| v.as_array()) {
        for ch in children { collect_symbols_from_item(path, ch, out, changed_lines); }
    }
}

fn map_lsp_symbol_kind(k: u64) -> crate::ir::SymbolKind {
    match k {
        6 => crate::ir::SymbolKind::Method,      // Method
        12 => crate::ir::SymbolKind::Function,   // Function
        5 => crate::ir::SymbolKind::Struct,      // Class -> Struct-ish
        23 => crate::ir::SymbolKind::Struct,     // Struct (RA specific mapping)
        10 => crate::ir::SymbolKind::Enum,       // Enum
        22 => crate::ir::SymbolKind::Enum,       // EnumMember -> treat as Enum (non-callable)
        9 => crate::ir::SymbolKind::Trait,       // Interface -> Trait-ish
        2 => crate::ir::SymbolKind::Module,      // Namespace/Module
        _ => crate::ir::SymbolKind::Function,
    }
}

fn intersects_lines(start: u32, end: u32, lines: &std::collections::HashSet<u32>) -> bool {
    let mut ln = start; while ln <= end { if lines.contains(&ln) { return true; } ln += 1; } false
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangedStrategy { DocumentSymbol, WorkspaceSymbol, TsFallback }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImpactStrategy { CallHierarchy, References, TsFallback }

pub fn decide_changed_strategy(caps: &CapabilityMatrix) -> ChangedStrategy {
    if caps.document_symbol { ChangedStrategy::DocumentSymbol }
    else if caps.workspace_symbol { ChangedStrategy::WorkspaceSymbol }
    else { ChangedStrategy::TsFallback }
}

pub fn decide_impact_strategy(caps: &CapabilityMatrix) -> ImpactStrategy {
    if caps.call_hierarchy { ImpactStrategy::CallHierarchy }
    else if caps.references || caps.definition { ImpactStrategy::References }
    else { ImpactStrategy::TsFallback }
}

// ---- LSP graph builder (TS相当) ----

fn collect_symbols_all(path: &str, items: &[serde_json::Value], out: &mut Vec<crate::ir::Symbol>) {
    for it in items {
        let name = it.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let kind_num = it.get("kind").and_then(|v| v.as_u64()).unwrap_or(12);
        let allowed = matches!(kind_num, 6 | 12); // Method | Function のみをグラフ対象に
        let (start_line0, end_line0) = if let Some(r) = it.get("range") {
            (r.get("start").and_then(|s| s.get("line")).and_then(|n| n.as_u64()).unwrap_or(0), r.get("end").and_then(|e| e.get("line")).and_then(|n| n.as_u64()).unwrap_or(0))
        } else if let Some(loc) = it.get("location") { if let Some(r) = loc.get("range") {
            (r.get("start").and_then(|s| s.get("line")).and_then(|n| n.as_u64()).unwrap_or(0), r.get("end").and_then(|e| e.get("line")).and_then(|n| n.as_u64()).unwrap_or(0))
        } else { (0,0) } } else { (0,0) };
        let start_line = (start_line0 as u32) + 1;
        let mut end_line = end_line0 as u32;
        if end_line < start_line { end_line = start_line; }
        let kind = map_lsp_symbol_kind(kind_num);
        if allowed && !name.is_empty() {
            out.push(crate::ir::Symbol { id: crate::ir::SymbolId::new("rust", path, &kind, name, start_line), name: name.to_string(), kind, file: path.to_string(), range: crate::ir::TextRange { start_line, end_line }, language: "rust".to_string() });
        }
        if let Some(children) = it.get("children").and_then(|v| v.as_array()) { collect_symbols_all(path, children, out); }
    }
}

fn lsp_build_project_graph(sess: &mut LspSession) -> anyhow::Result<(crate::ir::reference::SymbolIndex, Vec<crate::ir::reference::Reference>)> {
    use walkdir::WalkDir;
    let mut all_symbols: Vec<crate::ir::Symbol> = Vec::new();
    // 1) Collect function/method symbols
    for entry in WalkDir::new(".")
        .into_iter()
        .filter_entry(|e| {
            let p = e.path();
            let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
            !(name == ".git" || name == "target" || name.starts_with('.'))
        })
        .filter_map(Result::ok) {
        let path = entry.path();
        if path.is_file() {
            if path.extension().and_then(|s| s.to_str()) != Some("rs") { continue; }
            let abspath = std::fs::canonicalize(path).unwrap_or(path.to_path_buf());
            let uri = path_to_uri(&abspath);
            let path_str = if let Ok(rel) = path.strip_prefix("./") { rel.to_string_lossy().to_string() } else { path.to_string_lossy().to_string() };
            // didOpen
            let text = std::fs::read_to_string(&abspath).unwrap_or_default();
            let _ = sess.notify("textDocument/didOpen", serde_json::json!({"textDocument": {"uri": uri, "languageId":"rust", "version": 1, "text": text}}));
            // documentSymbol
            if let Ok(items) = sess.req_document_symbol(&uri) {
                collect_symbols_all(&path_str, &items, &mut all_symbols);
            }
        }
    }
    // 2) Build edges via references at callee definitions
    let mut edges: Vec<crate::ir::reference::Reference> = Vec::new();
    for to_sym in &all_symbols {
        let abspath = std::fs::canonicalize(&to_sym.file).unwrap_or_else(|_| std::path::PathBuf::from(&to_sym.file));
        let uri = path_to_uri(&abspath);
        let (line0, ch0) = guess_callable_position(&to_sym.file, to_sym).unwrap_or((to_sym.range.start_line.saturating_sub(1), 0));
        let refs = sess.req_references(&uri, line0, ch0).unwrap_or_default();
        for loc in refs {
            let loc_uri = loc.get("uri").and_then(|v| v.as_str()).unwrap_or("");
            let file = uri_to_path(loc_uri);
            let line0 = loc.get("range").and_then(|r| r.get("start")).and_then(|s| s.get("line")).and_then(|n| n.as_u64()).unwrap_or(0) as u32;
            let items = sess.req_document_symbol(loc_uri).unwrap_or_default();
            if let Some(caller) = enclosing_symbol_in_doc(&items, &file, line0)
                && caller.id.0 != to_sym.id.0 {
                edges.push(crate::ir::reference::Reference { from: caller.id.clone(), to: to_sym.id.clone(), kind: crate::ir::reference::RefKind::Call, file: caller.file.clone(), line: caller.range.start_line });
            }
        }
    }
    let index = crate::ir::reference::SymbolIndex::build(all_symbols);
    Ok((index, edges))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jsonrpc_framing_roundtrip() {
        let v = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {"capabilities": {}}
        });
        let buf = encode_jsonrpc_message(&v);
        let (v2, used) = decode_jsonrpc_message(&buf).expect("decode");
        assert_eq!(v2["method"], "initialize");
        assert_eq!(used, buf.len());
    }

    #[test]
    fn jsonrpc_decode_with_extra_data() {
        let v = json!({"jsonrpc":"2.0","id":1,"result":{"ok":true}});
        let mut buf = encode_jsonrpc_message(&v);
        buf.extend_from_slice(b"trailing");
        let (v2, used) = decode_jsonrpc_message(&buf).expect("decode");
        assert_eq!(v2["result"]["ok"], true);
        assert!(used < buf.len());
    }

    #[test]
    fn mock_session_probes_are_true() {
        let cfg = LspConfig { strict: true, dump_capabilities: false, mock: true, mock_caps: None };
        let sess = LspSession::new(crate::mapping::LanguageMode::Rust, cfg).expect("mock ok");
        assert!(sess.capabilities.document_symbol);
        assert!(sess.capabilities.call_hierarchy);
    }
}
