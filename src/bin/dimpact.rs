use clap::{Parser, Subcommand, ValueEnum};
use dimpact::{parse_unified_diff, DiffParseError};
use dimpact::{ChangedOutput, LanguageMode};
use dimpact::{ImpactDirection, ImpactOptions, ImpactOutput};
use dimpact::engine::{EngineKind, make_engine};
use dimpact::EngineConfig;
use is_terminal::IsTerminal;
use std::io::{self, Read};
use env_logger::Env;
use std::fs;

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Json,
    Yaml,
    Dot,
    Html,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Mode {
    Diff,
    Changed,
    Impact,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum LangOpt { Auto, Rust, Ruby, Javascript, Typescript, Tsx }

#[derive(Debug, Clone, Copy, ValueEnum)]
enum DirectionOpt { Callers, Callees, Both }

#[derive(Debug, Clone, Copy, ValueEnum)]
enum EngineOpt { Auto, Ts, Lsp }

#[derive(Debug, Clone, Copy, ValueEnum)]
enum KindOpt {
    #[value(alias = "function")] Fn,
    Method,
    Struct,
    Enum,
    Trait,
    #[value(alias = "module")] Mod,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CacheScopeOpt { Local, Global }


#[derive(Debug, Parser)]
#[command(name = "dimpact", version, about = "Analyze git diff and serialize changes")] 
struct Args {
    /// Output format (json or yaml)
    #[arg(short = 'f', long = "format", value_enum, default_value_t = OutputFormat::Json)]
    format: OutputFormat,

    /// Deprecated: use subcommands (diff/changed/impact/id) instead
    #[arg(long = "mode", value_enum, default_value_t = Mode::Diff, hide = true)]
    mode: Mode,

    /// Language mode for symbol extraction / detection
    #[arg(long = "lang", value_enum, default_value_t = LangOpt::Auto)]
    lang: LangOpt,

    /// Impact direction: callers, callees or both (when mode=impact)
    #[arg(long = "direction", value_enum, default_value_t = DirectionOpt::Callers)]
    direction: DirectionOpt,

    /// Max traversal depth for impact (0 = only changed, 1 = neighbors)
    #[arg(long = "max-depth")]
    max_depth: Option<usize>,

    /// Include reference edges in impact output
    #[arg(long = "with-edges", default_value_t = false)]
    with_edges: bool,

    /// Analysis engine: auto (default), ts, lsp
    #[arg(long = "engine", value_enum, default_value_t = EngineOpt::Auto)]
    engine: EngineOpt,

    /// LSP strict mode: do not fallback to TS on failure
    #[arg(long = "engine-lsp-strict", default_value_t = false)]
    engine_lsp_strict: bool,

    /// Dump detected LSP capabilities (diagnostic)
    #[arg(long = "engine-dump-capabilities", default_value_t = false)]
    engine_dump_capabilities: bool,

    /// Seed Symbol IDs to compute impact from (repeatable)
    /// Format: {LANG}:{PATH}:{KIND}:{NAME}:{LINE}
    /// KIND: fn|method|struct|enum|trait|mod
    #[arg(long = "seed-symbol")]
    seed_symbols: Vec<String>,

    /// Seed symbols as JSON (string, file path, or '-' for stdin)
    /// Accepts: ["LANG:PATH:KIND:NAME:LINE", ...] or
    ///          [{"lang":"rust","path":"src/lib.rs","kind":"fn","name":"foo","line":12}, ...]
    #[arg(long = "seed-json")]
    seed_json: Option<String>,
    /// Subcommands
    #[command(subcommand)]
    cmd: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Show parsed diff from stdin
    Diff,
    /// Show changed symbols from diff
    Changed{
        #[arg(long = "lang", value_enum, default_value_t = LangOpt::Auto)]
        lang: LangOpt,
        /// Analysis engine: auto (TS default), ts, lsp (experimental)
        #[arg(long = "engine", value_enum, default_value_t = EngineOpt::Auto)]
        engine: EngineOpt,
        #[arg(long = "engine-lsp-strict", default_value_t = false)]
        engine_lsp_strict: bool,
        #[arg(long = "engine-dump-capabilities", default_value_t = false)]
        engine_dump_capabilities: bool,
    },
    /// Compute impact from diff or seeds
    Impact{
        #[arg(long = "lang", value_enum, default_value_t = LangOpt::Auto)]
        lang: LangOpt,
        #[arg(long = "direction", value_enum, default_value_t = DirectionOpt::Callers)]
        direction: DirectionOpt,
        #[arg(long = "max-depth")] max_depth: Option<usize>,
        #[arg(long = "with-edges", default_value_t = false)] with_edges: bool,
        /// Analysis engine: auto (TS default), ts, lsp (experimental)
        #[arg(long = "engine", value_enum, default_value_t = EngineOpt::Auto)] engine: EngineOpt,
        #[arg(long = "engine-lsp-strict", default_value_t = false)] engine_lsp_strict: bool,
        #[arg(long = "engine-dump-capabilities", default_value_t = false)] engine_dump_capabilities: bool,
        #[arg(long = "seed-symbol")] seed_symbols: Vec<String>,
        #[arg(long = "seed-json")] seed_json: Option<String>,
    },
    /// Generate a Symbol ID from file, line and name
    Id{
        /// Target file path (optional; if omitted, searches workspace)
        #[arg(long = "path")] path: Option<String>,
        /// 1-based line number within the symbol (optional; effective only with --path)
        #[arg(long = "line")] line: Option<u32>,
        /// Symbol name (e.g. function/method/struct name) (optional)
        #[arg(long = "name")] name: Option<String>,
        /// Language override (auto by default)
        #[arg(long = "lang", value_enum, default_value_t = LangOpt::Auto)] lang: LangOpt,
        /// Optional kind filter to narrow candidates
        #[arg(long = "kind", value_enum)] kind: Option<KindOpt>,
        /// If exactly one candidate, print plain ID
        #[arg(long = "raw", default_value_t = false)] raw: bool,
    },
    /// Manage incremental analysis cache
    Cache{
        #[command(subcommand)] cmd: CacheCmd,
    },
}

#[derive(Debug, Subcommand)]
enum CacheCmd {
    /// Build or rebuild cache for workspace
    Build{
        /// Cache scope: local (repo) or global (XDG_CONFIG_HOME)
        #[arg(long = "scope", value_enum, default_value_t = CacheScopeOpt::Local)]
        scope: CacheScopeOpt,
        /// Override cache directory (takes precedence over scope)
        #[arg(long = "dir")] dir: Option<String>,
    },
    /// Show cache stats (files/symbols/edges)
    Stats{
        #[arg(long = "scope", value_enum, default_value_t = CacheScopeOpt::Local)]
        scope: CacheScopeOpt,
        #[arg(long = "dir")] dir: Option<String>,
    },
    /// Clear cache (delete DB file)
    Clear{
        #[arg(long = "scope", value_enum, default_value_t = CacheScopeOpt::Local)]
        scope: CacheScopeOpt,
        #[arg(long = "dir")] dir: Option<String>,
    },
}

fn main() -> anyhow::Result<()> {
    // Initialize logger once; default level comes from RUST_LOG
    let _ = env_logger::Builder::from_env(Env::default().default_filter_or(""))
        .format_timestamp(None)
        .try_init();
    let args = Args::parse();

    // Prefer subcommands if provided; fallback to deprecated --mode
    if let Some(cmd) = args.cmd {
        match cmd {
            Command::Diff => run_diff(args.format),
            Command::Changed{ lang, engine, engine_lsp_strict, engine_dump_capabilities } => {
                run_changed(args.format, lang, engine, engine_lsp_strict, engine_dump_capabilities)
            }
            Command::Impact{ lang, direction, max_depth, with_edges, engine, engine_lsp_strict, engine_dump_capabilities, seed_symbols, seed_json } => {
                run_impact(args.format, lang, direction, max_depth, with_edges, engine, engine_lsp_strict, engine_dump_capabilities, seed_symbols, seed_json)
            }
            Command::Id{ path, line, name, lang, kind, raw } => run_id(args.format, path.as_deref(), line, name.as_deref(), lang, kind, raw),
            Command::Cache{ cmd } => run_cache(cmd),
        }?;
        return Ok(());
    }

    match args.mode {
        Mode::Diff => {
            run_diff(args.format)?;
        }
        Mode::Changed => {
            run_changed(args.format, args.lang, args.engine, args.engine_lsp_strict, args.engine_dump_capabilities)?;
        }
        Mode::Impact => {
            run_impact(args.format, args.lang, args.direction, args.max_depth, args.with_edges, args.engine, args.engine_lsp_strict, args.engine_dump_capabilities, args.seed_symbols, args.seed_json)?;
        }
    }

    Ok(())
}

fn run_cache(cmd: CacheCmd) -> anyhow::Result<()> {
    match cmd {
        CacheCmd::Build{ scope, dir } => {
            let scope = match scope { CacheScopeOpt::Local => dimpact::cache::CacheScope::Local, CacheScopeOpt::Global => dimpact::cache::CacheScope::Global };
            let path_override = dir.as_deref().map(std::path::Path::new);
            let mut db = dimpact::cache::open(scope, path_override)?;
            let st = dimpact::cache::build_all(&mut db.conn)?;
            eprintln!("cache build: files={} symbols={} edges={}", st.files, st.symbols, st.edges);
        }
        CacheCmd::Stats{ scope, dir } => {
            let scope = match scope { CacheScopeOpt::Local => dimpact::cache::CacheScope::Local, CacheScopeOpt::Global => dimpact::cache::CacheScope::Global };
            let path_override = dir.as_deref().map(std::path::Path::new);
            let db = dimpact::cache::open(scope, path_override)?;
            let st = dimpact::cache::stats(&db.conn)?;
            println!("{{\"files\":{},\"symbols\":{},\"edges\":{}}}", st.files, st.symbols, st.edges);
        }
        CacheCmd::Clear{ scope, dir } => {
            let scope = match scope { CacheScopeOpt::Local => dimpact::cache::CacheScope::Local, CacheScopeOpt::Global => dimpact::cache::CacheScope::Global };
            let path_override = dir.as_deref().map(std::path::Path::new);
            let paths = dimpact::cache::resolve_paths(scope, path_override, None)?;
            dimpact::cache::clear(&paths)?;
            eprintln!("cache cleared: {}", paths.db.display());
        }
    }
    Ok(())
}

// A/B compare helpers removed in TS-only mode

fn read_diff_from_stdin() -> anyhow::Result<String> {
    if std::io::stdin().is_terminal() {
        anyhow::bail!("no stdin detected: please pipe `git diff` output into dimpact");
    }
    let mut s = String::new();
    io::stdin().read_to_string(&mut s)?;
    Ok(s)
}

fn parse_seed_symbol(s: &str) -> anyhow::Result<dimpact::Symbol> {
    // Format: {LANG}:{PATH}:{KIND}:{NAME}:{LINE}
    let parts: Vec<&str> = s.splitn(5, ':').collect();
    if parts.len() != 5 {
        anyhow::bail!("invalid seed symbol format: {}", s);
    }
    let lang = parts[0];
    let file = parts[1];
    let kind_str = parts[2];
    let name = parts[3];
    let line: u32 = parts[4].parse().map_err(|_| anyhow::anyhow!("invalid LINE in seed symbol: {}", parts[4]))?;

    let kind = match kind_str {
        "fn" | "function" => dimpact::SymbolKind::Function,
        "method" => dimpact::SymbolKind::Method,
        "struct" => dimpact::SymbolKind::Struct,
        "enum" => dimpact::SymbolKind::Enum,
        "trait" => dimpact::SymbolKind::Trait,
        "mod" | "module" => dimpact::SymbolKind::Module,
        other => anyhow::bail!("unknown KIND in seed symbol: {}", other),
    };

    let id = dimpact::SymbolId::new(lang, file, &kind, name, line);
    let sym = dimpact::Symbol {
        id,
        name: name.to_string(),
        kind,
        file: file.to_string(),
        range: dimpact::TextRange { start_line: line, end_line: line },
        language: lang.to_string(),
    };
    Ok(sym)
}

fn parse_seed_json_input(arg: &str) -> anyhow::Result<Vec<dimpact::Symbol>> {
    // Determine source: stdin ('-'), file path, or inline JSON
    let content = if arg == "-" {
        let mut s = String::new();
        io::stdin().read_to_string(&mut s)?;
        s
    } else if std::fs::metadata(arg).map(|m| m.is_file()).unwrap_or(false) {
        std::fs::read_to_string(arg)?
    } else {
        arg.to_string()
    };
    parse_seed_json(&content)
}

fn parse_seed_json(content: &str) -> anyhow::Result<Vec<dimpact::Symbol>> {
    let v: serde_json::Value = serde_json::from_str(content)
        .map_err(|e| anyhow::anyhow!("failed to parse seed JSON: {}", e))?;
    let arr = v.as_array().ok_or_else(|| anyhow::anyhow!("seed JSON must be an array"))?;
    let mut out: Vec<dimpact::Symbol> = Vec::with_capacity(arr.len());
    for item in arr {
        if let Some(s) = item.as_str() {
            out.push(parse_seed_symbol(s)?);
            continue;
        }
        if let Some(obj) = item.as_object() {
            // If { id: "..." } provided
            if let Some(serde_json::Value::String(id)) = obj.get("id") {
                out.push(parse_seed_symbol(id)?);
                continue;
            }
            let lang = obj.get("lang").or_else(|| obj.get("language"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("seed object missing 'lang'"))?;
            let file = obj.get("path").or_else(|| obj.get("file"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("seed object missing 'path' or 'file'"))?;
            let kind_str = obj.get("kind").and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("seed object missing 'kind'"))?;
            let name = obj.get("name").and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("seed object missing 'name'"))?;
            let line = obj.get("line")
                .or_else(|| obj.get("start_line"))
                .and_then(|v| v.as_u64())
                .ok_or_else(|| anyhow::anyhow!("seed object missing 'line' or 'start_line'"))? as u32;

            let kind = match kind_str {
                "fn" | "function" => dimpact::SymbolKind::Function,
                "method" => dimpact::SymbolKind::Method,
                "struct" => dimpact::SymbolKind::Struct,
                "enum" => dimpact::SymbolKind::Enum,
                "trait" => dimpact::SymbolKind::Trait,
                "mod" | "module" => dimpact::SymbolKind::Module,
                other => anyhow::bail!("unknown KIND in seed object: {}", other),
            };
            let id = dimpact::SymbolId::new(lang, file, &kind, name, line);
            out.push(dimpact::Symbol {
                id,
                name: name.to_string(),
                kind,
                file: file.to_string(),
                range: dimpact::TextRange { start_line: line, end_line: line },
                language: lang.to_string(),
            });
            continue;
        }
        anyhow::bail!("seed JSON elements must be strings or objects");
    }
    Ok(out)
}

fn run_diff(fmt: OutputFormat) -> anyhow::Result<()> {
    let diff_text = read_diff_from_stdin()?;
    let files = match parse_unified_diff(&diff_text) {
        Ok(f) => f,
        Err(DiffParseError::MissingHeader) => Vec::new(),
        Err(e) => return Err(anyhow::anyhow!(e)),
    };
    match fmt {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&files)?),
        OutputFormat::Yaml => print!("{}", serde_yaml::to_string(&files)?),
        OutputFormat::Dot | OutputFormat::Html => anyhow::bail!("format not supported for 'diff': use json|yaml"),
    }
    Ok(())
}

fn run_changed(fmt: OutputFormat, lang_opt: LangOpt, engine_opt: EngineOpt, lsp_strict: bool, dump_caps: bool) -> anyhow::Result<()> {
    let diff_text = read_diff_from_stdin()?;
    let files = match parse_unified_diff(&diff_text) {
        Ok(f) => f,
        Err(DiffParseError::MissingHeader) => Vec::new(),
        Err(e) => return Err(anyhow::anyhow!(e)),
    };
    let lang = match lang_opt {
        LangOpt::Auto => LanguageMode::Auto,
        LangOpt::Rust => LanguageMode::Rust,
        LangOpt::Ruby => LanguageMode::Ruby,
        LangOpt::Javascript => LanguageMode::Javascript,
        LangOpt::Typescript => LanguageMode::Typescript,
        LangOpt::Tsx => LanguageMode::Tsx,
    };
    let ekind = match engine_opt { EngineOpt::Auto => EngineKind::Auto, EngineOpt::Ts => EngineKind::Ts, EngineOpt::Lsp => EngineKind::Lsp };
    let ecfg = EngineConfig { lsp_strict, dump_capabilities: dump_caps, mock_lsp: std::env::var("DIMPACT_TEST_LSP_MOCK").ok().as_deref() == Some("1"), mock_caps: None };
    let engine = make_engine(ekind, ecfg);
    if dump_caps && !matches!(engine_opt, EngineOpt::Lsp) {
        // For diagnostics under TS/Auto, emit a stub capability matrix to stderr
        eprintln!("{}", serde_json::json!({
            "document_symbol": false,
            "workspace_symbol": false,
            "references": false,
            "definition": false,
            "call_hierarchy": false,
        }));
    }
    log::info!("mode=changed engine={:?} files={} lang={:?}", ekind, files.len(), lang);
    let report: ChangedOutput = engine.changed_symbols(&files, lang)?;
    match fmt {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
        OutputFormat::Yaml => print!("{}", serde_yaml::to_string(&report)?),
        OutputFormat::Dot | OutputFormat::Html => anyhow::bail!("format not supported for 'changed': use json|yaml"),
    }
    Ok(())
}

fn run_impact(
    fmt: OutputFormat,
    lang_opt: LangOpt,
    dir_opt: DirectionOpt,
    max_depth: Option<usize>,
    with_edges: bool,
    engine_opt: EngineOpt,
    lsp_strict: bool,
    dump_caps: bool,
    seed_symbols: Vec<String>,
    seed_json: Option<String>,
) -> anyhow::Result<()> {
    // Gather seeds
    let mut seeds: Vec<dimpact::Symbol> = Vec::new();
    if let Some(sj) = seed_json.as_ref() {
        let mut from_json = parse_seed_json_input(sj)?;
        seeds.append(&mut from_json);
    }
    if !seed_symbols.is_empty() {
        for s in &seed_symbols { seeds.push(parse_seed_symbol(s)?); }
    }

    // Determine language: prefer seeds' language when provided
    let lang: LanguageMode = if !seeds.is_empty() {
        let mut langs: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for s in &seeds { langs.insert(s.language.to_ascii_lowercase()); }
        if langs.len() > 1 {
            anyhow::bail!("mixed seed languages not supported: {:?}", langs);
        }
        let seed_lang = langs.iter().next().cloned().unwrap_or_else(|| "auto".to_string());
        lang_mode_from_str(&seed_lang).ok_or_else(|| anyhow::anyhow!("unknown seed language: {}", seed_lang))?
    } else {
        match lang_opt {
            LangOpt::Auto => LanguageMode::Auto,
            LangOpt::Rust => LanguageMode::Rust,
            LangOpt::Ruby => LanguageMode::Ruby,
            LangOpt::Javascript => LanguageMode::Javascript,
            LangOpt::Typescript => LanguageMode::Typescript,
            LangOpt::Tsx => LanguageMode::Tsx,
        }
    };
    let direction = match dir_opt { DirectionOpt::Callers => ImpactDirection::Callers, DirectionOpt::Callees => ImpactDirection::Callees, DirectionOpt::Both => ImpactDirection::Both };
    let opts = ImpactOptions { direction, max_depth: max_depth.or(Some(100)), with_edges: Some(with_edges) };
    let ekind = match engine_opt { EngineOpt::Auto => EngineKind::Auto, EngineOpt::Ts => EngineKind::Ts, EngineOpt::Lsp => EngineKind::Lsp };
    let ecfg = EngineConfig { lsp_strict, dump_capabilities: dump_caps, mock_lsp: std::env::var("DIMPACT_TEST_LSP_MOCK").ok().as_deref() == Some("1"), mock_caps: None };
    let engine = make_engine(ekind, ecfg);
    if dump_caps && !matches!(engine_opt, EngineOpt::Lsp) {
        eprintln!("{}", serde_json::json!({
            "document_symbol": false,
            "workspace_symbol": false,
            "references": false,
            "definition": false,
            "call_hierarchy": false,
        }));
    }

    if seeds.is_empty() {
        // Diff-based
        let diff_text = read_diff_from_stdin()?;
        let files = match parse_unified_diff(&diff_text) {
            Ok(f) => f,
            Err(DiffParseError::MissingHeader) => Vec::new(),
            Err(e) => return Err(anyhow::anyhow!(e)),
        };
        log::info!("mode=impact(diff) engine={:?} files={} lang={:?} dir={:?} max_depth={:?} with_edges={}", ekind, files.len(), lang, direction, opts.max_depth, with_edges);
        let out: ImpactOutput = engine.impact(&files, lang, &opts)?;
        match fmt {
            OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&out)?),
            OutputFormat::Yaml => print!("{}", serde_yaml::to_string(&out)?),
            OutputFormat::Dot => println!("{}", dimpact::to_dot(&out)),
            OutputFormat::Html => println!("{}", dimpact::to_html(&out)),
        }
        return Ok(());
    }

    log::info!("mode=impact(seeds) engine={:?} seeds={} lang={:?} dir={:?} max_depth={:?} with_edges={}", ekind, seeds.len(), lang, direction, opts.max_depth, with_edges);
    let out: ImpactOutput = engine.impact_from_symbols(&seeds, lang, &opts)?;
    match fmt {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&out)?),
        OutputFormat::Yaml => print!("{}", serde_yaml::to_string(&out)?),
        OutputFormat::Dot => println!("{}", dimpact::to_dot(&out)),
        OutputFormat::Html => println!("{}", dimpact::to_html(&out)),
    }
    Ok(())
}

fn lang_mode_from_str(s: &str) -> Option<LanguageMode> {
    match s.to_ascii_lowercase().as_str() {
        "rust" => Some(LanguageMode::Rust),
        "ruby" => Some(LanguageMode::Ruby),
        "javascript" | "js" => Some(LanguageMode::Javascript),
        "typescript" | "ts" => Some(LanguageMode::Typescript),
        "tsx" => Some(LanguageMode::Tsx),
        "auto" => Some(LanguageMode::Auto),
        _ => None,
    }
}

fn run_id(fmt: OutputFormat, path: Option<&str>, line: Option<u32>, name: Option<&str>, lang_opt: LangOpt, kind_opt: Option<KindOpt>, raw: bool) -> anyhow::Result<()> {
    // Determine search scope (single file or workspace)
    if line.is_some() && path.is_none() {
        anyhow::bail!("--line requires --path (cannot use line without file context)");
    }

    let files = collect_candidate_files(path, lang_opt)?;
    let mut all_syms: Vec<dimpact::Symbol> = Vec::new();
    for fp in &files {
        let lkind = match lang_opt {
            LangOpt::Auto => dimpact::LanguageKind::Auto,
            LangOpt::Rust => dimpact::LanguageKind::Rust,
            LangOpt::Ruby => dimpact::LanguageKind::Ruby,
            LangOpt::Javascript => dimpact::LanguageKind::Javascript,
            LangOpt::Typescript => dimpact::LanguageKind::Typescript,
            LangOpt::Tsx => dimpact::LanguageKind::Tsx,
        };
        let Some(analyzer) = dimpact::languages::analyzer_for_path(fp, lkind) else { continue };
        let Ok(source) = fs::read_to_string(fp) else { continue };
        let mut syms = analyzer.symbols_in_file(fp, &source);
        all_syms.append(&mut syms);
    }

    if all_syms.is_empty() {
        anyhow::bail!("no symbols found in search scope");
    }

    // Stepwise narrowing: path -> line -> name -> kind (each only if yields results)
    let mut current: Vec<dimpact::Symbol> = all_syms.clone();
    if let Some(p) = path {
        let subset: Vec<_> = current.iter().cloned().filter(|s| s.file == p).collect();
        if !subset.is_empty() { current = subset; } else { current = all_syms.clone(); }
    }
    if let Some(ln) = line {
        let subset: Vec<_> = current.iter().cloned().filter(|s| s.range.start_line <= ln && ln <= s.range.end_line).collect();
        if !subset.is_empty() { current = subset; }
    }
    if let Some(nm) = name {
        let subset: Vec<_> = current.iter().cloned().filter(|s| s.name == nm).collect();
        if !subset.is_empty() { current = subset; }
    }
    if let Some(kopt) = kind_opt {
        let want = map_kind_opt(kopt);
        let subset: Vec<_> = current.iter().cloned().filter(|s| s.kind == want).collect();
        if !subset.is_empty() { current = subset; }
    }

    if current.is_empty() {
        anyhow::bail!("no matching symbol candidates");
    }

    let mut sorted = current;
    sorted.sort_by_key(|s| (s.range.end_line - s.range.start_line, key_of_kind(&s.kind)));

    if raw {
        for s in &sorted { println!("{}", s.id.0); }
        return Ok(());
    }

    match fmt {
        OutputFormat::Json => {
            let items: Vec<serde_json::Value> = sorted.iter().map(|s| serde_json::json!({
                "id": s.id.0,
                "symbol": s,
            })).collect();
            println!("{}", serde_json::to_string_pretty(&items)?);
        }
        OutputFormat::Yaml => {
            print!("{}", serde_yaml::to_string(&sorted)?);
        }
        OutputFormat::Dot | OutputFormat::Html => anyhow::bail!("format not supported for 'id': use json|yaml or --raw"),
    }
    Ok(())
}

fn collect_candidate_files(path: Option<&str>, lang_opt: LangOpt) -> anyhow::Result<Vec<String>> {
    if let Some(p) = path {
        let md = fs::metadata(p);
        if md.as_ref().map(|m| m.is_file()).unwrap_or(false) {
            return Ok(vec![p.to_string()]);
        } else {
            anyhow::bail!("path is not a file: {}", p);
        }
    }
    // Workspace scan by extensions
    let mut out = Vec::new();
    let exts = match lang_opt {
        LangOpt::Auto => vec!["rs","rb","js","ts","tsx"],
        LangOpt::Rust => vec!["rs"],
        LangOpt::Ruby => vec!["rb"],
        LangOpt::Javascript => vec!["js"],
        LangOpt::Typescript => vec!["ts"],
        LangOpt::Tsx => vec!["tsx"],
    };
    let root = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    scan_dir(&root, &exts, &mut out)?;
    Ok(out)
}

fn scan_dir(dir: &std::path::Path, exts: &[&str], out: &mut Vec<String>) -> anyhow::Result<()> {
    if let Some(name) = dir.file_name().and_then(|s| s.to_str()) {
        if [".git","target","node_modules"].contains(&name) {
            return Ok(());
        }
    }
    let rd = match fs::read_dir(dir) { Ok(r) => r, Err(_) => return Ok(()) };
    for ent in rd {
        let ent = match ent { Ok(e) => e, Err(_) => continue };
        let p = ent.path();
        let Ok(ft) = ent.file_type() else { continue };
        if ft.is_dir() { scan_dir(&p, exts, out)?; continue; }
        if !ft.is_file() { continue; }
        let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("");
        if exts.contains(&ext) {
            out.push(p.to_string_lossy().to_string());
        }
    }
    Ok(())
}

fn choose_most_specific(mut v: Vec<dimpact::Symbol>) -> dimpact::Symbol {
    v.sort_by_key(|s| (s.range.end_line - s.range.start_line, key_of_kind(&s.kind)));
    v.into_iter().next().unwrap()
}

fn key_of_kind(k: &dimpact::SymbolKind) -> u8 {
    match k {
        dimpact::SymbolKind::Method => 0,
        dimpact::SymbolKind::Function => 1,
        dimpact::SymbolKind::Struct => 2,
        dimpact::SymbolKind::Enum => 3,
        dimpact::SymbolKind::Trait => 4,
        dimpact::SymbolKind::Module => 5,
    }
}

fn map_kind_opt(k: KindOpt) -> dimpact::SymbolKind {
    match k {
        KindOpt::Fn => dimpact::SymbolKind::Function,
        KindOpt::Method => dimpact::SymbolKind::Method,
        KindOpt::Struct => dimpact::SymbolKind::Struct,
        KindOpt::Enum => dimpact::SymbolKind::Enum,
        KindOpt::Trait => dimpact::SymbolKind::Trait,
        KindOpt::Mod => dimpact::SymbolKind::Module,
    }
}

fn impact_from_diff(args: Args, files: Vec<dimpact::FileChanges>) -> anyhow::Result<()> {
    let lang = match args.lang {
        LangOpt::Auto => LanguageMode::Auto,
        LangOpt::Rust => LanguageMode::Rust,
        LangOpt::Ruby => LanguageMode::Ruby,
        LangOpt::Javascript => LanguageMode::Javascript,
        LangOpt::Typescript => LanguageMode::Typescript,
        LangOpt::Tsx => LanguageMode::Tsx,
    };
    let direction = match args.direction {
        DirectionOpt::Callers => ImpactDirection::Callers,
        DirectionOpt::Callees => ImpactDirection::Callees,
        DirectionOpt::Both => ImpactDirection::Both,
    };
    let opts = ImpactOptions { direction, max_depth: args.max_depth.or(Some(100)), with_edges: Some(args.with_edges) };
    let ekind = match args.engine { EngineOpt::Auto => EngineKind::Auto, EngineOpt::Ts => EngineKind::Ts, EngineOpt::Lsp => EngineKind::Lsp };
    let ecfg = EngineConfig { lsp_strict: args.engine_lsp_strict, dump_capabilities: args.engine_dump_capabilities, mock_lsp: false, mock_caps: None };
    let engine = make_engine(ekind, ecfg);
    log::info!(
        "mode=impact(diff) engine={:?} files={} lang={:?} dir={:?} max_depth={:?} with_edges={}",
        ekind,
        files.len(),
        lang,
        direction,
        opts.max_depth,
        args.with_edges
    );
    let out: ImpactOutput = engine.impact(&files, lang, &opts)?;
    match args.format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&out)?),
        OutputFormat::Yaml => print!("{}", serde_yaml::to_string(&out)?),
        OutputFormat::Dot => println!("{}", dimpact::to_dot(&out)),
        OutputFormat::Html => println!("{}", dimpact::to_html(&out)),
    }
    Ok(())
}
