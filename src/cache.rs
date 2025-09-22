use anyhow::Context;
use rusqlite::{Connection, params};
use std::fs;
use std::path::{Path, PathBuf};

use crate::ir::reference::{Reference, SymbolIndex, UnresolvedRef};
use crate::ir::{Symbol, SymbolId, SymbolKind, TextRange};
use crate::languages::{LanguageKind, analyzer_for_path};
type SymbolsByPath = std::collections::HashMap<String, Vec<Symbol>>;
type UrefsByPath = std::collections::HashMap<String, Vec<UnresolvedRef>>;
type ImportMapByPath = std::collections::HashMap<String, std::collections::HashMap<String, String>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheScope {
    Local,
    Global,
}

#[derive(Debug, Clone)]
pub struct CachePaths {
    pub dir: PathBuf,
    pub db: PathBuf,
    pub lock: PathBuf,
}

#[derive(Debug)]
pub struct CacheDb {
    pub conn: Connection,
    pub paths: CachePaths,
}

#[derive(Debug, Default, Clone)]
pub struct CacheStats {
    pub files: i64,
    pub symbols: i64,
    pub edges: i64,
}

const SCHEMA_VERSION: &str = "v1";

pub fn resolve_paths(
    scope: CacheScope,
    override_dir: Option<&Path>,
    repo_root: Option<&Path>,
) -> anyhow::Result<CachePaths> {
    if let Some(dir) = override_dir {
        let dir = dir.to_path_buf();
        return Ok(CachePaths {
            db: dir.join("index.db"),
            lock: dir.join(".lock"),
            dir,
        });
    }
    match scope {
        CacheScope::Local => {
            let root = repo_root
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| find_repo_root().unwrap_or_else(|| PathBuf::from(".")));
            let dir = root.join(".dimpact").join("cache").join(SCHEMA_VERSION);
            Ok(CachePaths {
                db: dir.join("index.db"),
                lock: dir.join(".lock"),
                dir,
            })
        }
        CacheScope::Global => {
            let xdg = std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
                .unwrap_or_else(|| PathBuf::from(".config"));
            let root = find_repo_root().unwrap_or_else(|| PathBuf::from("."));
            let key = repo_key(&root);
            let dir = xdg
                .join("dimpact")
                .join("cache")
                .join(SCHEMA_VERSION)
                .join(key);
            Ok(CachePaths {
                db: dir.join("index.db"),
                lock: dir.join(".lock"),
                dir,
            })
        }
    }
}

pub fn open(scope: CacheScope, override_dir: Option<&Path>) -> anyhow::Result<CacheDb> {
    let paths = resolve_paths(scope, override_dir, None)?;
    fs::create_dir_all(&paths.dir)
        .with_context(|| format!("create cache dir: {}", paths.dir.display()))?;
    let mut conn = Connection::open(&paths.db)
        .with_context(|| format!("open cache db: {}", paths.db.display()))?;
    init_db(&mut conn)?;
    Ok(CacheDb { conn, paths })
}

pub fn scope_from_env() -> (CacheScope, Option<PathBuf>) {
    let scope = match std::env::var("DIMPACT_CACHE_SCOPE").ok().as_deref() {
        Some("global") | Some("GLOBAL") => CacheScope::Global,
        _ => CacheScope::Local,
    };
    let dir = std::env::var_os("DIMPACT_CACHE_DIR").map(PathBuf::from);
    (scope, dir)
}

fn init_db(conn: &mut Connection) -> anyhow::Result<()> {
    // Pragmas for WAL and reasonable defaults
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "temp_store", "MEMORY")?;

    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS files (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT NOT NULL UNIQUE,
            lang TEXT NOT NULL,
            digest TEXT,
            mtime INTEGER,
            present INTEGER NOT NULL DEFAULT 1
        );
        CREATE TABLE IF NOT EXISTS symbols (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            sid TEXT NOT NULL,
            file_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            kind TEXT NOT NULL,
            start_line INTEGER NOT NULL,
            end_line INTEGER NOT NULL,
            language TEXT NOT NULL,
            sig_hash TEXT,
            parent_sid TEXT,
            FOREIGN KEY(file_id) REFERENCES files(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_symbols_sid ON symbols(sid);
        CREATE INDEX IF NOT EXISTS idx_symbols_file ON symbols(file_id);
        CREATE INDEX IF NOT EXISTS idx_symbols_name ON symbols(name);

        CREATE TABLE IF NOT EXISTS edges (
            from_sid TEXT NOT NULL,
            to_sid TEXT NOT NULL,
            kind TEXT NOT NULL,
            file_id INTEGER NOT NULL,
            line INTEGER NOT NULL,
            FOREIGN KEY(file_id) REFERENCES files(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_edges_from ON edges(from_sid);
        CREATE INDEX IF NOT EXISTS idx_edges_to ON edges(to_sid);
        CREATE INDEX IF NOT EXISTS idx_edges_file ON edges(file_id);
        "#,
    )?;

    // Record schema version
    conn.execute(
        "INSERT OR REPLACE INTO meta(key, value) VALUES('schema_version', ?1)",
        params![SCHEMA_VERSION],
    )?;
    Ok(())
}

pub fn stats(conn: &Connection) -> anyhow::Result<CacheStats> {
    Ok(CacheStats {
        files: conn
            .query_row("SELECT COUNT(*) FROM files WHERE present=1", [], |r| {
                r.get(0)
            })
            .unwrap_or(0),
        symbols: conn
            .query_row("SELECT COUNT(*) FROM symbols", [], |r| r.get(0))
            .unwrap_or(0),
        edges: conn
            .query_row("SELECT COUNT(*) FROM edges", [], |r| r.get(0))
            .unwrap_or(0),
    })
}

pub fn clear(paths: &CachePaths) -> anyhow::Result<()> {
    if paths.db.exists() {
        fs::remove_file(&paths.db).ok();
    }
    // Keep dir and lock; it's fine
    Ok(())
}

pub fn build_all(conn: &mut Connection) -> anyhow::Result<CacheStats> {
    // Rebuild from scratch using parallel analysis
    let files = list_workspace_files();
    let (symbols, urefs, file_imports) = analyze_paths_parallel(&files);
    let index = SymbolIndex::build(symbols);
    let refs = crate::impact::resolve_references(&index, &urefs, &file_imports);
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM symbols", [])?;
    tx.execute("DELETE FROM edges", [])?;
    tx.execute("DELETE FROM files", [])?;

    // Insert files encountered in symbols
    let mut file_ids: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for s in &index.symbols {
        file_ids.entry(s.file.clone()).or_insert_with(|| {
            let dig = file_digest(&s.file);
            let lang = s.language.clone();
            tx.execute(
                "INSERT INTO files(path, lang, digest, mtime, present) VALUES(?1, ?2, ?3, ?4, 1)",
                params![&s.file, &lang, dig, file_mtime(&s.file)],
            )
            .unwrap();
            tx.last_insert_rowid()
        });
    }

    // Insert symbols
    {
        let mut sym_stmt = tx.prepare("INSERT INTO symbols(sid, file_id, name, kind, start_line, end_line, language, sig_hash, parent_sid) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)")?;
        for s in &index.symbols {
            let file_id = *file_ids.get(&s.file).unwrap();
            sym_stmt.execute(params![
                &s.id.0,
                file_id,
                &s.name,
                kind_to_str(&s.kind),
                s.range.start_line as i64,
                s.range.end_line as i64,
                &s.language,
                sig_hash_for(s),
                Option::<String>::None
            ])?;
        }
    }

    // Insert edges
    {
        let mut edge_stmt = tx.prepare(
            "INSERT INTO edges(from_sid, to_sid, kind, file_id, line) VALUES(?1, ?2, ?3, ?4, ?5)",
        )?;
        for e in &refs {
            // file_id derived from e.file
            let file_id = *file_ids.entry(e.file.clone()).or_insert_with(|| {
                tx.execute(
                    "INSERT INTO files(path, lang, digest, mtime, present) VALUES(?1, ?2, ?3, ?4, 1)",
                    params![&e.file, guess_lang_from_ext(&e.file), file_digest(&e.file), file_mtime(&e.file)],
                ).unwrap();
                tx.last_insert_rowid()
            });
            edge_stmt.execute(params![&e.from.0, &e.to.0, "call", file_id, e.line as i64])?;
        }
    }
    tx.commit()?;
    let st = stats(conn)?;
    Ok(st)
}

/// Verify cache consistency against current workspace without requiring a diff.
/// - Recompute digests for current files and update entries whose digest/present/lang changed
/// - Mark missing files as present=0 and drop their symbols/edges
pub fn verify(conn: &mut Connection) -> anyhow::Result<CacheStats> {
    // Load DB snapshot
    let mut db_files: std::collections::HashMap<String, (String, i64, String)> =
        std::collections::HashMap::new();
    {
        let mut stmt = conn.prepare("SELECT path, digest, present, lang FROM files")?;
        let rows = stmt.query_map([], |r| {
            let p: String = r.get(0)?;
            let dig: String = r.get(1)?;
            let present: i64 = r.get(2)?;
            let lang: String = r.get(3)?;
            Ok((p, dig, present, lang))
        })?;
        for r in rows {
            let (p, dig, pr, lang) = r?;
            db_files.insert(p, (dig, pr, lang));
        }
    }

    // Scan current workspace files
    let fs_files = list_workspace_files();
    let fs_set: std::collections::HashSet<String> = fs_files.iter().cloned().collect();

    // Determine updates for existing files
    let mut to_update: Vec<String> = Vec::new();
    for p in &fs_files {
        let dig = file_digest(p);
        let present_expected: i64 = 1;
        let lang = guess_lang_from_ext(p).to_string();
        match db_files.get(p) {
            None => to_update.push(p.clone()),
            Some((db_dig, db_present, db_lang)) => {
                if db_dig != &dig || *db_present != present_expected || db_lang != &lang {
                    to_update.push(p.clone());
                }
            }
        }
    }
    // Determine updates for files that were present but are now missing
    for (p, (_dig, pr, _lang)) in db_files.iter() {
        if *pr == 1 && !fs_set.contains(p) {
            to_update.push(p.clone());
        }
    }

    // Dedup in case of overlap
    to_update.sort();
    to_update.dedup();
    update_paths(conn, &to_update)
}

pub fn update_paths(conn: &mut Connection, paths: &[String]) -> anyhow::Result<CacheStats> {
    if paths.is_empty() {
        return stats(conn);
    }
    // Analyze changed files in parallel
    let (symbols_by_file, urefs_by_file, imports_by_file) = analyze_specific_paths_parallel(paths);

    // Write symbols in a single transaction
    {
        let tx = conn.transaction()?;
        for p in paths {
            let exists = fs::metadata(p).map(|m| m.is_file()).unwrap_or(false);
            let lang = guess_lang_from_ext(p).to_string();
            tx.execute(
                "INSERT INTO files(path, lang, digest, mtime, present) VALUES(?1, ?2, ?3, ?4, ?5)\n                 ON CONFLICT(path) DO UPDATE SET lang=excluded.lang, digest=excluded.digest, mtime=excluded.mtime, present=excluded.present",
                params![p, &lang, file_digest(p), file_mtime(p), if exists {1} else {0}],
            )?;
            let file_id: i64 =
                tx.query_row("SELECT id FROM files WHERE path=?1", params![p], |r| {
                    r.get(0)
                })?;
            tx.execute("DELETE FROM symbols WHERE file_id=?1", params![file_id])?;
            tx.execute("DELETE FROM edges WHERE file_id=?1", params![file_id])?;
            if let Some(syms) = symbols_by_file.get(p) {
                let mut stmt = tx.prepare("INSERT INTO symbols(sid, file_id, name, kind, start_line, end_line, language, sig_hash, parent_sid) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)")?;
                for s in syms {
                    stmt.execute(params![
                        &s.id.0,
                        file_id,
                        &s.name,
                        kind_to_str(&s.kind),
                        s.range.start_line as i64,
                        s.range.end_line as i64,
                        &s.language,
                        sig_hash_for(s),
                        Option::<String>::None
                    ])?;
                }
            }
        }
        tx.commit()?;
    }

    // Build index including newly inserted symbols
    let index = load_index(conn)?;

    // Insert edges for changed files using prepared unresolved refs/imports
    {
        let tx = conn.transaction()?;
        {
            let mut edge_stmt = tx.prepare("INSERT INTO edges(from_sid, to_sid, kind, file_id, line) VALUES(?1, ?2, ?3, ?4, ?5)")?;
            for p in paths {
                let file_id: i64 =
                    tx.query_row("SELECT id FROM files WHERE path=?1", params![p], |r| {
                        r.get(0)
                    })?;
                let urefs = urefs_by_file.get(p).cloned().unwrap_or_default();
                let imports = imports_by_file.get(p).cloned().unwrap_or_default();
                let refs = crate::impact::resolve_references(
                    &index,
                    &urefs,
                    &std::collections::HashMap::from([(p.clone(), imports)]),
                );
                for e in refs {
                    edge_stmt.execute(params![
                        &e.from.0,
                        &e.to.0,
                        "call",
                        file_id,
                        e.line as i64
                    ])?;
                }
            }
        }
        tx.commit()?;
    }
    stats(conn)
}

// Parallel build helpers
fn list_workspace_files() -> Vec<String> {
    let mut out = Vec::new();
    for entry in walkdir::WalkDir::new(".")
        .into_iter()
        .filter_entry(|e| {
            let p = e.path();
            let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
            !(name == ".git" || name == "target" || name == "node_modules" || name.starts_with('.'))
        })
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path.is_file() {
            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
            if ["rs", "rb", "js", "ts", "tsx"].contains(&ext) {
                let path_str = path
                    .strip_prefix("./")
                    .unwrap_or(path)
                    .to_string_lossy()
                    .to_string();
                out.push(path_str);
            }
        }
    }
    out
}

#[allow(clippy::type_complexity)]
fn analyze_paths_parallel(paths: &[String]) -> (Vec<Symbol>, Vec<UnresolvedRef>, ImportMapByPath) {
    use rayon::prelude::*;
    let results: Vec<(
        Vec<Symbol>,
        Vec<UnresolvedRef>,
        (String, std::collections::HashMap<String, String>),
    )> = paths
        .par_iter()
        .map(|p| {
            let kind = LanguageKind::Auto;
            let Some(analyzer) = analyzer_for_path(p, kind) else {
                return (Vec::new(), Vec::new(), (p.clone(), Default::default()));
            };
            let Ok(src) = fs::read_to_string(p) else {
                return (Vec::new(), Vec::new(), (p.clone(), Default::default()));
            };
            let syms = analyzer.symbols_in_file(p, &src);
            let urefs = analyzer.unresolved_refs(p, &src);
            let im = analyzer.imports_in_file(p, &src);
            (syms, urefs, (p.clone(), im))
        })
        .collect();
    let mut symbols = Vec::new();
    let mut urefs_all = Vec::new();
    let mut imports_map: ImportMapByPath = std::collections::HashMap::new();
    for (syms, urefs, (p, im)) in results {
        symbols.extend(syms);
        urefs_all.extend(urefs);
        imports_map.insert(p, im);
    }
    (symbols, urefs_all, imports_map)
}

#[allow(clippy::type_complexity)]
fn analyze_specific_paths_parallel(
    paths: &[String],
) -> (SymbolsByPath, UrefsByPath, ImportMapByPath) {
    use rayon::prelude::*;
    let results: Vec<(
        String,
        Vec<Symbol>,
        Vec<UnresolvedRef>,
        std::collections::HashMap<String, String>,
    )> = paths
        .par_iter()
        .map(|p| {
            let p = p.clone();
            if !std::path::Path::new(&p).is_file() {
                return (p, Vec::new(), Vec::new(), Default::default());
            }
            let kind = LanguageKind::Auto;
            let Some(analyzer) = analyzer_for_path(&p, kind) else {
                return (p, Vec::new(), Vec::new(), Default::default());
            };
            let Ok(src) = fs::read_to_string(&p) else {
                return (p, Vec::new(), Vec::new(), Default::default());
            };
            let syms = analyzer.symbols_in_file(&p, &src);
            let urefs = analyzer.unresolved_refs(&p, &src);
            let im = analyzer.imports_in_file(&p, &src);
            (p, syms, urefs, im)
        })
        .collect();
    let mut syms_map = std::collections::HashMap::new();
    let mut urefs_map = std::collections::HashMap::new();
    let mut imports_map = std::collections::HashMap::new();
    for (p, syms, urefs, im) in results {
        syms_map.insert(p.clone(), syms);
        urefs_map.insert(p.clone(), urefs);
        imports_map.insert(p, im);
    }
    (syms_map, urefs_map, imports_map)
}

pub fn load_graph(conn: &Connection) -> anyhow::Result<(SymbolIndex, Vec<Reference>)> {
    let index = load_index(conn)?;
    // Edges
    let mut stmt = conn.prepare("SELECT from_sid, to_sid, kind, files.path, line FROM edges JOIN files ON edges.file_id = files.id")?;
    let edge_iter = stmt.query_map([], |row| {
        let from_sid: String = row.get(0)?;
        let to_sid: String = row.get(1)?;
        let _kind: String = row.get(2)?; // currently only call
        let file: String = row.get(3)?;
        let line: i64 = row.get(4)?;
        Ok(Reference {
            from: SymbolId(from_sid),
            to: SymbolId(to_sid),
            kind: crate::ir::reference::RefKind::Call,
            file,
            line: line as u32,
        })
    })?;
    let mut edges = Vec::new();
    for e in edge_iter {
        edges.push(e?);
    }
    Ok((index, edges))
}

fn load_index(conn: &Connection) -> anyhow::Result<SymbolIndex> {
    let mut stmt = conn.prepare("SELECT sid, files.path, symbols.name, symbols.kind, symbols.start_line, symbols.end_line, symbols.language FROM symbols JOIN files ON symbols.file_id = files.id WHERE files.present=1")?;
    let rows = stmt.query_map([], |row| {
        let sid: String = row.get(0)?;
        let file: String = row.get(1)?;
        let name: String = row.get(2)?;
        let kind_s: String = row.get(3)?;
        let start_line: i64 = row.get(4)?;
        let end_line: i64 = row.get(5)?;
        let lang: String = row.get(6)?;
        let kind = match kind_s.as_str() {
            "fn" | "function" => SymbolKind::Function,
            "method" => SymbolKind::Method,
            "struct" => SymbolKind::Struct,
            "enum" => SymbolKind::Enum,
            "trait" => SymbolKind::Trait,
            "mod" | "module" => SymbolKind::Module,
            _ => SymbolKind::Function,
        };
        Ok(Symbol {
            id: SymbolId(sid),
            name,
            kind,
            file,
            range: TextRange {
                start_line: start_line as u32,
                end_line: end_line as u32,
            },
            language: lang,
        })
    })?;
    let mut symbols = Vec::new();
    for r in rows {
        symbols.push(r?);
    }
    Ok(SymbolIndex::build(symbols))
}

fn find_repo_root() -> Option<PathBuf> {
    let mut cur = std::env::current_dir().ok()?;
    loop {
        if cur.join(".git").exists() || cur.join(".hg").exists() || cur.join(".svn").exists() {
            return Some(cur);
        }
        if !cur.pop() {
            break;
        }
    }
    None
}

fn repo_key(root: &Path) -> String {
    let c = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let s = c.to_string_lossy();
    let mut hasher = blake3::Hasher::new();
    hasher.update(s.as_bytes());
    let hash = hasher.finalize();
    let short = &hash.to_hex()[..10];
    let base = root.file_name().and_then(|s| s.to_str()).unwrap_or("repo");
    format!("{}-{}", short, base)
}

fn file_digest(path: &str) -> String {
    match fs::read(path) {
        Ok(bytes) => {
            let mut hasher = blake3::Hasher::new();
            hasher.update(&bytes);
            hasher.finalize().to_hex().to_string()
        }
        Err(_) => String::new(),
    }
}

fn file_mtime(path: &str) -> i64 {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

fn guess_lang_from_ext(path: &str) -> &'static str {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    match ext {
        "rs" => "rust",
        "rb" => "ruby",
        "js" => "javascript",
        "ts" => "typescript",
        "tsx" => "tsx",
        _ => "unknown",
    }
}

fn kind_to_str(k: &SymbolKind) -> &'static str {
    match k {
        SymbolKind::Function => "fn",
        SymbolKind::Method => "method",
        SymbolKind::Struct => "struct",
        SymbolKind::Enum => "enum",
        SymbolKind::Trait => "trait",
        SymbolKind::Module => "mod",
    }
}

fn sig_hash_for(s: &Symbol) -> String {
    // M1: simple placeholder (name+kind). Later: normalized signature+scope chain
    let mut hasher = blake3::Hasher::new();
    hasher.update(s.name.as_bytes());
    hasher.update(kind_to_str(&s.kind).as_bytes());
    hasher.finalize().to_hex().to_string()
}
