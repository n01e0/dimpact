use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use dimpact::DfgBuilder;
use dimpact::EngineConfig;
use dimpact::attach_slice_selection_summary;
use dimpact::cache;
use dimpact::compute_impact;
use dimpact::dfg::{DataFlowGraph, DependencyKind, PdgBuilder, RubyDfgBuilder, RustDfgBuilder};
use dimpact::dfg_to_dot;
use dimpact::engine::{AutoPolicy, EngineKind, make_engine_with_auto_policy};
use dimpact::ir::SymbolId;
use dimpact::ir::reference::{EdgeCertainty, EdgeProvenance, RefKind, Reference, SymbolIndex};
use dimpact::{ChangedOutput, LanguageMode};
use dimpact::{DiffParseError, parse_unified_diff};
use dimpact::{
    ImpactDirection, ImpactOptions, ImpactOutput, ImpactSliceBridgeKind, ImpactSliceCandidateLane,
    ImpactSliceCandidateScoringSummary, ImpactSliceCandidateSourceKind,
    ImpactSliceCandidateSupportMetadata, ImpactSliceEvidenceKind, ImpactSliceFileMetadata,
    ImpactSlicePlannerKind, ImpactSlicePruneReason, ImpactSliceReasonKind,
    ImpactSliceReasonMetadata, ImpactSliceScopes, ImpactSliceScoreTuple,
    ImpactSliceSelectionSummary,
};
use env_logger::Env;
use is_terminal::IsTerminal;
use serde::Serialize;
use std::fs;
use std::io::{self, Read};

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
enum LangOpt {
    Auto,
    Rust,
    Ruby,
    #[value(alias = "py")]
    Python,
    Javascript,
    Typescript,
    Tsx,
    Go,
    Java,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum DirectionOpt {
    Callers,
    Callees,
    Both,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum EngineOpt {
    Auto,
    Ts,
    Lsp,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum AutoPolicyOpt {
    Compat,
    StrictIfAvailable,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ConfidenceOpt {
    Confirmed,
    Inferred,
    #[value(alias = "dynamic_fallback")]
    DynamicFallback,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OperationalProfileOpt {
    #[value(name = "balanced")]
    Balanced,
    #[value(name = "precision-first")]
    PrecisionFirst,
}

impl ConfidenceOpt {
    fn min_rank(self) -> u8 {
        match self {
            Self::DynamicFallback => 0,
            Self::Inferred => 1,
            Self::Confirmed => 2,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Confirmed => "confirmed",
            Self::Inferred => "inferred",
            Self::DynamicFallback => "dynamic_fallback",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct ConfidenceFilterSummary {
    min_confidence: Option<String>,
    exclude_dynamic_fallback: bool,
    input_edge_count: usize,
    kept_edge_count: usize,
}

#[derive(Debug, Serialize)]
struct ImpactOutputRendered<'a> {
    #[serde(flatten)]
    output: &'a ImpactOutput,
    #[serde(skip_serializing_if = "Option::is_none")]
    confidence_filter: Option<&'a ConfidenceFilterSummary>,
}

fn certainty_rank(certainty: EdgeCertainty) -> u8 {
    match certainty {
        EdgeCertainty::DynamicFallback => 0,
        EdgeCertainty::Inferred => 1,
        EdgeCertainty::Confirmed => 2,
    }
}

fn meets_min_confidence(certainty: EdgeCertainty, min: ConfidenceOpt) -> bool {
    certainty_rank(certainty) >= min.min_rank()
}

fn resolve_operational_profile(
    profile: Option<OperationalProfileOpt>,
    min_confidence: Option<ConfidenceOpt>,
    exclude_dynamic_fallback: bool,
) -> (Option<ConfidenceOpt>, bool) {
    let Some(profile) = profile else {
        return (min_confidence, exclude_dynamic_fallback);
    };

    match profile {
        OperationalProfileOpt::Balanced => (
            min_confidence.or(Some(ConfidenceOpt::Inferred)),
            exclude_dynamic_fallback,
        ),
        OperationalProfileOpt::PrecisionFirst => {
            (min_confidence.or(Some(ConfidenceOpt::Confirmed)), true)
        }
    }
}

fn apply_confidence_filter(
    out: ImpactOutput,
    opts: &ImpactOptions,
    min_confidence: Option<ConfidenceOpt>,
    exclude_dynamic_fallback: bool,
    keep_edges_in_output: bool,
) -> (ImpactOutput, Option<ConfidenceFilterSummary>) {
    if min_confidence.is_none() && !exclude_dynamic_fallback {
        return (out, None);
    }

    let input_edge_count = out.edges.len();
    let filtered_refs: Vec<Reference> = out
        .edges
        .iter()
        .filter(|r| {
            min_confidence
                .map(|min| meets_min_confidence(r.certainty.clone(), min))
                .unwrap_or(true)
                && (!exclude_dynamic_fallback
                    || !matches!(r.certainty, EdgeCertainty::DynamicFallback))
        })
        .cloned()
        .collect();

    let mut symbols: Vec<dimpact::ir::Symbol> = Vec::new();
    symbols.extend(out.changed_symbols.clone());
    symbols.extend(out.impacted_symbols.clone());

    let index = SymbolIndex::build(symbols);
    let mut recompute_opts = opts.clone();
    recompute_opts.with_edges = Some(true);
    let mut filtered = compute_impact(
        &out.changed_symbols,
        &index,
        &filtered_refs,
        &recompute_opts,
    );
    let summary = ConfidenceFilterSummary {
        min_confidence: min_confidence.map(|m| m.as_str().to_string()),
        exclude_dynamic_fallback,
        input_edge_count,
        kept_edge_count: filtered.edges.len(),
    };
    if !keep_edges_in_output {
        filtered.edges.clear();
    }
    (filtered, Some(summary))
}

fn print_impact_output(
    fmt: OutputFormat,
    out: &ImpactOutput,
    confidence_filter: Option<&ConfidenceFilterSummary>,
) -> anyhow::Result<()> {
    if let Some(cf) = confidence_filter {
        eprintln!(
            "confidence filter applied: min_confidence={} exclude_dynamic_fallback={} kept_edges={}/{}",
            cf.min_confidence.as_deref().unwrap_or("(none)"),
            cf.exclude_dynamic_fallback,
            cf.kept_edge_count,
            cf.input_edge_count
        );
    }
    match fmt {
        OutputFormat::Json => println!(
            "{}",
            serde_json::to_string_pretty(&ImpactOutputRendered {
                output: out,
                confidence_filter,
            })?
        ),
        OutputFormat::Yaml => print!(
            "{}",
            serde_yaml::to_string(&ImpactOutputRendered {
                output: out,
                confidence_filter,
            })?
        ),
        OutputFormat::Dot => println!("{}", dimpact::to_dot(out)),
        OutputFormat::Html => println!("{}", dimpact::to_html(out)),
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum KindOpt {
    #[value(alias = "function")]
    Fn,
    Method,
    Struct,
    Enum,
    Trait,
    #[value(alias = "module")]
    Mod,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CacheScopeOpt {
    Local,
    Global,
}

#[derive(Debug, Parser)]
#[command(
    name = "dimpact",
    version,
    about = "Analyze git diff and serialize changes"
)]
struct Args {
    /// Output format (json, yaml, dot, html)
    #[arg(short = 'f', long = "format", value_enum, default_value_t = OutputFormat::Json, global = true)]
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
    /// Minimum edge confidence used for impact traversal/output filtering.
    /// confirmed: only confirmed edges
    /// inferred: confirmed + inferred
    /// dynamic-fallback: all edges
    #[arg(long = "min-confidence", value_enum)]
    min_confidence: Option<ConfidenceOpt>,
    /// Exclude dynamic-fallback edges from impact traversal/output.
    #[arg(long = "exclude-dynamic-fallback", default_value_t = false)]
    exclude_dynamic_fallback: bool,
    /// Operational confidence profile preset for impact filtering.
    /// balanced: min-confidence inferred
    /// precision-first: min-confidence confirmed + exclude dynamic-fallback
    #[arg(long = "op-profile", value_enum)]
    op_profile: Option<OperationalProfileOpt>,
    /// Ignore directories (relative prefixes). Repeatable.
    #[arg(long = "ignore-dir")]
    ignore_dir: Vec<String>,

    /// Analysis engine: auto (default), ts, lsp
    #[arg(long = "engine", value_enum, default_value_t = EngineOpt::Auto)]
    engine: EngineOpt,

    /// Auto engine policy: compat (default) or strict-if-available
    #[arg(long = "auto-policy", value_enum, global = true)]
    auto_policy: Option<AutoPolicyOpt>,

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
    /// Group impact per changed/seed symbol; output per-seed results
    #[arg(long = "per-seed", default_value_t = false)]
    per_seed: bool,
    /// Subcommands
    #[command(subcommand)]
    cmd: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Show parsed diff from stdin
    Diff,
    /// Show changed symbols from diff
    Changed {
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
    Impact {
        #[arg(long = "lang", value_enum, default_value_t = LangOpt::Auto)]
        lang: LangOpt,
        #[arg(long = "direction", value_enum, default_value_t = DirectionOpt::Callers)]
        direction: DirectionOpt,
        #[arg(long = "max-depth")]
        max_depth: Option<usize>,
        #[arg(long = "with-edges", default_value_t = false)]
        with_edges: bool,
        /// Minimum edge confidence used for impact traversal/output filtering.
        /// confirmed: only confirmed edges
        /// inferred: confirmed + inferred
        /// dynamic-fallback: all edges
        #[arg(long = "min-confidence", value_enum)]
        min_confidence: Option<ConfidenceOpt>,
        /// Exclude dynamic-fallback edges from impact traversal/output.
        #[arg(long = "exclude-dynamic-fallback", default_value_t = false)]
        exclude_dynamic_fallback: bool,
        /// Operational confidence profile preset for impact filtering.
        /// balanced: min-confidence inferred
        /// precision-first: min-confidence confirmed + exclude dynamic-fallback
        #[arg(long = "op-profile", value_enum)]
        op_profile: Option<OperationalProfileOpt>,
        /// Use PDG-based dependence analysis
        #[arg(long = "with-pdg", default_value_t = false)]
        with_pdg: bool,
        /// Enable symbolic propagation across variables and functions (implies PDG)
        #[arg(long = "with-propagation", default_value_t = false)]
        with_propagation: bool,
        /// Analysis engine: auto (TS default), ts, lsp
        #[arg(long = "engine", value_enum, default_value_t = EngineOpt::Auto)]
        engine: EngineOpt,
        #[arg(long = "engine-lsp-strict", default_value_t = false)]
        engine_lsp_strict: bool,
        #[arg(long = "engine-dump-capabilities", default_value_t = false)]
        engine_dump_capabilities: bool,
        #[arg(long = "seed-symbol")]
        seed_symbols: Vec<String>,
        #[arg(long = "seed-json")]
        seed_json: Option<String>,
        /// Ignore directories (relative prefixes). Repeatable.
        #[arg(long = "ignore-dir")]
        ignore_dir: Vec<String>,
        /// Group impact per changed/seed symbol; output per-seed results
        #[arg(long = "per-seed", default_value_t = false)]
        per_seed: bool,
    },
    /// Generate a Symbol ID from file, line and name
    Id {
        /// Target file path (optional; if omitted, searches workspace)
        #[arg(long = "path")]
        path: Option<String>,
        /// 1-based line number within the symbol (optional; effective only with --path)
        #[arg(long = "line")]
        line: Option<u32>,
        /// Symbol name (e.g. function/method/struct name) (optional)
        #[arg(long = "name")]
        name: Option<String>,
        /// Language override (auto by default)
        #[arg(long = "lang", value_enum, default_value_t = LangOpt::Auto)]
        lang: LangOpt,
        /// Optional kind filter to narrow candidates
        #[arg(long = "kind", value_enum)]
        kind: Option<KindOpt>,
        /// If exactly one candidate, print plain ID
        #[arg(long = "raw", default_value_t = false)]
        raw: bool,
    },
    /// Manage incremental analysis cache
    Cache {
        #[command(subcommand)]
        cmd: CacheCmd,
    },
    /// Generate shell completion script for this CLI
    Completions {
        /// Target shell (bash, zsh, fish, powershell, elvish)
        #[arg(value_enum)]
        shell: CompletionShell,
    },
}

#[derive(Debug, Subcommand)]
enum CacheCmd {
    /// Build or rebuild cache for workspace
    Build {
        /// Cache scope: local (repo) or global (XDG_CONFIG_HOME)
        #[arg(long = "scope", value_enum, default_value_t = CacheScopeOpt::Local)]
        scope: CacheScopeOpt,
        /// Override cache directory (takes precedence over scope)
        #[arg(long = "dir")]
        dir: Option<String>,
    },
    /// Update cache consistency against current workspace (no diff required)
    #[command(alias = "verify")]
    Update {
        #[arg(long = "scope", value_enum, default_value_t = CacheScopeOpt::Local)]
        scope: CacheScopeOpt,
        #[arg(long = "dir")]
        dir: Option<String>,
    },
    /// Show cache stats (files/symbols/edges)
    Stats {
        #[arg(long = "scope", value_enum, default_value_t = CacheScopeOpt::Local)]
        scope: CacheScopeOpt,
        #[arg(long = "dir")]
        dir: Option<String>,
    },
    /// Clear cache (delete DB file)
    Clear {
        #[arg(long = "scope", value_enum, default_value_t = CacheScopeOpt::Local)]
        scope: CacheScopeOpt,
        #[arg(long = "dir")]
        dir: Option<String>,
    },
}

fn main() -> anyhow::Result<()> {
    // Initialize logger once; default level comes from RUST_LOG
    let _ = env_logger::Builder::from_env(Env::default().default_filter_or(""))
        .format_timestamp(None)
        .try_init();
    // Optional parallelism override for rayon (for cache build/update)
    if let Ok(j) = std::env::var("DIMPACT_JOBS")
        && let Ok(n) = j.parse::<usize>()
    {
        let _ = rayon::ThreadPoolBuilder::new()
            .num_threads(n)
            .build_global();
    }

    let args = Args::parse();

    // Prefer subcommands if provided; fallback to deprecated --mode
    if let Some(cmd) = args.cmd {
        match cmd {
            Command::Diff => run_diff(args.format),
            Command::Changed {
                lang,
                engine,
                engine_lsp_strict,
                engine_dump_capabilities,
            } => run_changed(
                args.format,
                lang,
                engine,
                args.auto_policy,
                engine_lsp_strict,
                engine_dump_capabilities,
            ),
            Command::Impact {
                lang,
                direction,
                max_depth,
                with_edges,
                min_confidence,
                exclude_dynamic_fallback,
                op_profile,
                with_pdg,
                with_propagation,
                engine,
                engine_lsp_strict,
                engine_dump_capabilities,
                seed_symbols,
                seed_json,
                ignore_dir,
                per_seed,
            } => run_impact(
                args.format,
                lang,
                direction,
                max_depth,
                with_edges,
                min_confidence,
                exclude_dynamic_fallback,
                op_profile,
                with_pdg,
                with_propagation,
                engine,
                args.auto_policy,
                engine_lsp_strict,
                engine_dump_capabilities,
                seed_symbols,
                seed_json,
                ignore_dir,
                per_seed,
            ),
            Command::Id {
                path,
                line,
                name,
                lang,
                kind,
                raw,
            } => run_id(
                args.format,
                path.as_deref(),
                line,
                name.as_deref(),
                lang,
                kind,
                raw,
            ),
            Command::Cache { cmd } => run_cache(cmd),
            Command::Completions { shell } => run_completions(shell),
        }?;
        return Ok(());
    }

    match args.mode {
        Mode::Diff => {
            run_diff(args.format)?;
        }
        Mode::Changed => {
            run_changed(
                args.format,
                args.lang,
                args.engine,
                args.auto_policy,
                args.engine_lsp_strict,
                args.engine_dump_capabilities,
            )?;
        }
        Mode::Impact => {
            // PDG mode not available in deprecated mode
            run_impact(
                args.format,
                args.lang,
                args.direction,
                args.max_depth,
                args.with_edges,
                args.min_confidence,
                args.exclude_dynamic_fallback,
                args.op_profile,
                false,
                false,
                args.engine,
                args.auto_policy,
                args.engine_lsp_strict,
                args.engine_dump_capabilities,
                args.seed_symbols,
                args.seed_json,
                args.ignore_dir,
                args.per_seed,
            )?;
        }
    }

    Ok(())
}

fn run_cache(cmd: CacheCmd) -> anyhow::Result<()> {
    match cmd {
        CacheCmd::Build { scope, dir } => {
            let scope = match scope {
                CacheScopeOpt::Local => dimpact::cache::CacheScope::Local,
                CacheScopeOpt::Global => dimpact::cache::CacheScope::Global,
            };
            let path_override = dir.as_deref().map(std::path::Path::new);
            let mut db = dimpact::cache::open(scope, path_override)?;
            let st = dimpact::cache::build_all(&mut db.conn)?;
            eprintln!(
                "cache build: files={} symbols={} edges={}",
                st.files, st.symbols, st.edges
            );
        }
        CacheCmd::Update { scope, dir } => {
            let scope = match scope {
                CacheScopeOpt::Local => dimpact::cache::CacheScope::Local,
                CacheScopeOpt::Global => dimpact::cache::CacheScope::Global,
            };
            let path_override = dir.as_deref().map(std::path::Path::new);
            let mut db = dimpact::cache::open(scope, path_override)?;
            let st_before = dimpact::cache::stats(&db.conn)?;
            let st_after = dimpact::cache::verify(&mut db.conn)?;
            eprintln!(
                "cache update: files={} symbols={} edges={} (was files={} symbols={} edges={})",
                st_after.files,
                st_after.symbols,
                st_after.edges,
                st_before.files,
                st_before.symbols,
                st_before.edges
            );
        }
        CacheCmd::Stats { scope, dir } => {
            let scope = match scope {
                CacheScopeOpt::Local => dimpact::cache::CacheScope::Local,
                CacheScopeOpt::Global => dimpact::cache::CacheScope::Global,
            };
            let path_override = dir.as_deref().map(std::path::Path::new);
            let db = dimpact::cache::open(scope, path_override)?;
            let st = dimpact::cache::stats(&db.conn)?;
            println!(
                "{{\"files\":{},\"symbols\":{},\"edges\":{}}}",
                st.files, st.symbols, st.edges
            );
        }
        CacheCmd::Clear { scope, dir } => {
            let scope = match scope {
                CacheScopeOpt::Local => dimpact::cache::CacheScope::Local,
                CacheScopeOpt::Global => dimpact::cache::CacheScope::Global,
            };
            let path_override = dir.as_deref().map(std::path::Path::new);
            let paths = dimpact::cache::resolve_paths(scope, path_override, None)?;
            dimpact::cache::clear(&paths)?;
            eprintln!("cache cleared: {}", paths.db.display());
        }
    }
    Ok(())
}

fn run_completions(shell: CompletionShell) -> anyhow::Result<()> {
    use clap_complete::{generate, shells};
    let mut cmd = Args::command();
    let name = cmd.get_name().to_string();
    match shell {
        CompletionShell::Bash => generate(shells::Bash, &mut cmd, name, &mut std::io::stdout()),
        CompletionShell::Zsh => generate(shells::Zsh, &mut cmd, name, &mut std::io::stdout()),
        CompletionShell::Fish => generate(shells::Fish, &mut cmd, name, &mut std::io::stdout()),
        CompletionShell::PowerShell => {
            generate(shells::PowerShell, &mut cmd, name, &mut std::io::stdout())
        }
        CompletionShell::Elvish => generate(shells::Elvish, &mut cmd, name, &mut std::io::stdout()),
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
    let line: u32 = parts[4]
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid LINE in seed symbol: {}", parts[4]))?;

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
        range: dimpact::TextRange {
            start_line: line,
            end_line: line,
        },
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
    let arr = v
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("seed JSON must be an array"))?;
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
            let lang = obj
                .get("lang")
                .or_else(|| obj.get("language"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("seed object missing 'lang'"))?;
            let file = obj
                .get("path")
                .or_else(|| obj.get("file"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("seed object missing 'path' or 'file'"))?;
            let kind_str = obj
                .get("kind")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("seed object missing 'kind'"))?;
            let name = obj
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("seed object missing 'name'"))?;
            let line = obj
                .get("line")
                .or_else(|| obj.get("start_line"))
                .and_then(|v| v.as_u64())
                .ok_or_else(|| anyhow::anyhow!("seed object missing 'line' or 'start_line'"))?
                as u32;

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
                range: dimpact::TextRange {
                    start_line: line,
                    end_line: line,
                },
                language: lang.to_string(),
            });
            continue;
        }
        anyhow::bail!("seed JSON elements must be strings or objects");
    }
    Ok(out)
}

fn map_auto_policy(opt: AutoPolicyOpt) -> AutoPolicy {
    match opt {
        AutoPolicyOpt::Compat => AutoPolicy::Compat,
        AutoPolicyOpt::StrictIfAvailable => AutoPolicy::StrictIfAvailable,
    }
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
        OutputFormat::Dot | OutputFormat::Html => {
            anyhow::bail!("format not supported for 'diff': use json|yaml")
        }
    }
    Ok(())
}

fn run_changed(
    fmt: OutputFormat,
    lang_opt: LangOpt,
    engine_opt: EngineOpt,
    auto_policy: Option<AutoPolicyOpt>,
    lsp_strict: bool,
    dump_caps: bool,
) -> anyhow::Result<()> {
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
        LangOpt::Python => LanguageMode::Python,
        LangOpt::Javascript => LanguageMode::Javascript,
        LangOpt::Typescript => LanguageMode::Typescript,
        LangOpt::Tsx => LanguageMode::Tsx,
        LangOpt::Go => LanguageMode::Go,
        LangOpt::Java => LanguageMode::Java,
    };
    let ekind = match engine_opt {
        EngineOpt::Auto => EngineKind::Auto,
        EngineOpt::Ts => EngineKind::Ts,
        EngineOpt::Lsp => EngineKind::Lsp,
    };
    let ecfg = EngineConfig {
        lsp_strict,
        dump_capabilities: dump_caps,
        mock_lsp: std::env::var("DIMPACT_TEST_LSP_MOCK").ok().as_deref() == Some("1"),
        mock_caps: None,
    };
    let engine = make_engine_with_auto_policy(ekind, ecfg, auto_policy.map(map_auto_policy));
    if dump_caps && !matches!(engine_opt, EngineOpt::Lsp) {
        // For diagnostics under TS/Auto, emit a stub capability matrix to stderr
        eprintln!(
            "{}",
            serde_json::json!({
                "document_symbol": false,
                "workspace_symbol": false,
                "references": false,
                "definition": false,
                "call_hierarchy": false,
            })
        );
    }
    log::info!(
        "mode=changed engine={:?} files={} lang={:?}",
        ekind,
        files.len(),
        lang
    );
    let report: ChangedOutput = engine.changed_symbols(&files, lang)?;
    match fmt {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
        OutputFormat::Yaml => print!("{}", serde_yaml::to_string(&report)?),
        OutputFormat::Dot | OutputFormat::Html => {
            anyhow::bail!("format not supported for 'changed': use json|yaml")
        }
    }
    Ok(())
}

/// Grouped impact output per changed or seed symbol, with direction info
#[derive(Debug, Serialize)]
struct PerSeedImpact {
    direction: ImpactDirection,
    output: ImpactOutput,
    #[serde(skip_serializing_if = "Option::is_none")]
    confidence_filter: Option<ConfidenceFilterSummary>,
}

#[derive(Debug, Serialize)]
struct PerSeedOutput {
    changed_symbol: dimpact::ir::Symbol,
    impacts: Vec<PerSeedImpact>,
}

#[allow(clippy::too_many_arguments)]
fn strongest_certainty(a: EdgeCertainty, b: EdgeCertainty) -> EdgeCertainty {
    if certainty_rank(a.clone()) >= certainty_rank(b.clone()) {
        a
    } else {
        b
    }
}

fn ref_kind_from_dependency(kind: &DependencyKind) -> RefKind {
    match kind {
        DependencyKind::Data => RefKind::Data,
        DependencyKind::Control => RefKind::Control,
    }
}

fn edge_location_from_nodes(
    from: &str,
    to: &str,
    nodes_by_id: &std::collections::HashMap<String, (String, u32)>,
    callsite_certainty_by_loc: &std::collections::HashMap<(String, u32), EdgeCertainty>,
) -> (String, u32, Option<EdgeCertainty>) {
    let from_loc = nodes_by_id.get(from).cloned();
    let to_loc = nodes_by_id.get(to).cloned();

    let mut derived_certainty = None;
    let mut callsite_loc = None;
    if let Some(loc) = from_loc.clone()
        && let Some(certainty) = callsite_certainty_by_loc.get(&loc)
    {
        derived_certainty = Some(certainty.clone());
        callsite_loc = Some(loc);
    }
    if let Some(loc) = to_loc.clone()
        && let Some(certainty) = callsite_certainty_by_loc.get(&loc)
    {
        derived_certainty = Some(match derived_certainty {
            Some(existing) => strongest_certainty(existing, certainty.clone()),
            None => certainty.clone(),
        });
        if callsite_loc.is_none() {
            callsite_loc = Some(loc);
        }
    }
    if let Some((file, line)) = callsite_loc {
        return (file, line, derived_certainty);
    }
    if let Some((file, line)) = to_loc {
        return (file, line, derived_certainty);
    }
    if let Some((file, line)) = from_loc {
        return (file, line, derived_certainty);
    }
    (String::new(), 0, derived_certainty)
}

struct PdgContext {
    index: SymbolIndex,
    pdg: DataFlowGraph,
    refs: Vec<Reference>,
    slice_selection: ImpactSliceSelectionSummary,
    per_seed_slice_selection: std::collections::BTreeMap<String, ImpactSliceSelectionSummary>,
}

#[derive(Debug, Clone, Default)]
struct Tier2SemanticEvidence {
    param_to_return_flow: bool,
}

fn symbol_id_node_name(node_id: &str) -> Option<&str> {
    let mut parts = node_id.split(':');
    let _file = parts.next()?;
    let _kind = parts.next()?;
    parts.next()
}

fn rust_signature_param_names(source: &str, start_line: u32) -> std::collections::BTreeSet<String> {
    let start_idx = usize::try_from(start_line.saturating_sub(1)).unwrap_or_default();
    let header = source
        .lines()
        .skip(start_idx)
        .take(8)
        .scan(false, |done, line| {
            if *done {
                return None;
            }
            let chunk = line.trim();
            if chunk.contains('{') {
                *done = true;
            }
            Some(chunk.to_string())
        })
        .collect::<Vec<_>>()
        .join(" ");
    let Some(open_paren) = header.find('(') else {
        return std::collections::BTreeSet::new();
    };
    let Some(close_paren) = header[open_paren + 1..].find(')') else {
        return std::collections::BTreeSet::new();
    };
    header[open_paren + 1..open_paren + 1 + close_paren]
        .split(',')
        .filter_map(|raw| {
            let before_type = raw.split(':').next()?.trim();
            if before_type.is_empty() {
                return None;
            }
            let token = before_type
                .split_whitespace()
                .last()?
                .trim_matches('&')
                .trim_matches(|c: char| c == ',' || c == '(' || c == ')');
            if token.is_empty() || token == "self" || token == "_" {
                return None;
            }
            Some(token.to_string())
        })
        .collect()
}

fn collect_rust_tier2_semantic_evidence(
    completion_symbol: &dimpact::Symbol,
) -> Tier2SemanticEvidence {
    if completion_symbol.language != "rust" || !completion_symbol.file.ends_with(".rs") {
        return Tier2SemanticEvidence::default();
    }
    let Ok(src) = fs::read_to_string(&completion_symbol.file) else {
        return Tier2SemanticEvidence::default();
    };
    let param_names = rust_signature_param_names(&src, completion_symbol.range.start_line);
    if param_names.is_empty() {
        return Tier2SemanticEvidence::default();
    }

    let dfg = RustDfgBuilder::build(&completion_symbol.file, &src);
    let node_line_by_id: std::collections::HashMap<_, _> = dfg
        .nodes
        .iter()
        .map(|node| (node.id.clone(), node.line))
        .collect();
    let pdg = PdgBuilder::build(&dfg, &[]);
    let index = SymbolIndex::build(vec![completion_symbol.clone()]);
    let Some(summary) = PdgBuilder::build_function_summaries(&pdg, &index)
        .into_iter()
        .find(|summary| summary.function_id == completion_symbol.id.0)
    else {
        return Tier2SemanticEvidence::default();
    };

    let tail_line_floor = std::cmp::max(
        summary.start_line,
        summary
            .end_line
            .saturating_sub(if summary.end_line > summary.start_line {
                1
            } else {
                0
            }),
    );
    let param_input_ids: std::collections::BTreeSet<_> = summary
        .inputs
        .iter()
        .filter(|input_id| {
            symbol_id_node_name(input_id)
                .map(|name| param_names.contains(name))
                .unwrap_or(false)
        })
        .cloned()
        .collect();
    if param_input_ids.is_empty() {
        return Tier2SemanticEvidence::default();
    }

    let param_to_return_flow = summary.flows.iter().any(|flow| {
        param_input_ids.contains(&flow.input_node_id)
            && flow.impacted_node_ids.iter().any(|node_id| {
                node_line_by_id
                    .get(node_id)
                    .is_some_and(|line| *line >= tail_line_floor)
            })
    });

    Tier2SemanticEvidence {
        param_to_return_flow,
    }
}

fn build_local_dfg_for_paths<'a>(paths: impl IntoIterator<Item = &'a str>) -> DataFlowGraph {
    let mut combined = DataFlowGraph {
        nodes: Vec::new(),
        edges: Vec::new(),
    };
    for path in paths {
        if path.ends_with(".rs") {
            if let Ok(src) = fs::read_to_string(path) {
                let dfg = RustDfgBuilder::build(path, &src);
                combined.nodes.extend(dfg.nodes);
                combined.edges.extend(dfg.edges);
            }
        } else if path.ends_with(".rb")
            && let Ok(src) = fs::read_to_string(path)
        {
            let dfg = RubyDfgBuilder::build(path, &src);
            combined.nodes.extend(dfg.nodes);
            combined.edges.extend(dfg.edges);
        }
    }
    combined
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SliceSelectionTier {
    Root,
    DirectBoundary,
    BridgeCompletion,
}

const PER_BOUNDARY_SIDE_TIER2_FILES_MAX: usize = 1;
const PER_SEED_TIER2_FILES_MAX: usize = 2;

fn slice_selection_tier_value(tier: SliceSelectionTier) -> u8 {
    match tier {
        SliceSelectionTier::Root => 0,
        SliceSelectionTier::DirectBoundary => 1,
        SliceSelectionTier::BridgeCompletion => 2,
    }
}

#[derive(Debug, Default)]
struct BoundedSlicePlan {
    cache_update_paths: Vec<String>,
    local_dfg_paths: Vec<String>,
    slice_selection: ImpactSliceSelectionSummary,
    per_seed_slice_selection: std::collections::BTreeMap<String, ImpactSliceSelectionSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct RelatedCallSymbol {
    symbol_id: String,
    kind: ImpactSliceReasonKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Tier2Candidate {
    path: String,
    via_symbol_id: String,
    via_path: String,
    bridge_kind: Option<ImpactSliceBridgeKind>,
    scoring: ImpactSliceCandidateScoringSummary,
}

#[derive(Debug, Clone, Default)]
struct SliceSelectionFileState {
    scopes: ImpactSliceScopes,
    reasons: std::collections::BTreeSet<ImpactSliceReasonMetadata>,
}

#[derive(Debug, Clone, Default)]
struct SliceSelectionAccumulator {
    cache_update_paths: std::collections::BTreeSet<String>,
    local_dfg_paths: std::collections::BTreeSet<String>,
    files: std::collections::BTreeMap<String, SliceSelectionFileState>,
    pruned_candidates: std::collections::BTreeSet<dimpact::ImpactSlicePrunedCandidate>,
}

impl SliceSelectionAccumulator {
    fn mark_cache_update(&mut self, path: &str) {
        self.cache_update_paths.insert(path.to_string());
        let file_state = self.files.entry(path.to_string()).or_default();
        file_state.scopes.cache_update = true;
    }

    fn mark_local_dfg(&mut self, path: &str) {
        self.mark_cache_update(path);
        if supports_local_dfg(path) {
            self.local_dfg_paths.insert(path.to_string());
            let file_state = self.files.entry(path.to_string()).or_default();
            file_state.scopes.local_dfg = true;
        }
    }

    fn mark_explanation(&mut self, path: &str) {
        self.mark_cache_update(path);
        let file_state = self.files.entry(path.to_string()).or_default();
        file_state.scopes.explanation = true;
    }

    fn add_reason(&mut self, path: &str, reason: ImpactSliceReasonMetadata) {
        self.mark_explanation(path);
        if supports_local_dfg(path) {
            self.mark_local_dfg(path);
        }
        if let Some(file_state) = self.files.get_mut(path) {
            file_state.reasons.insert(reason);
        }
    }

    fn add_pruned_candidate(&mut self, candidate: dimpact::ImpactSlicePrunedCandidate) {
        self.pruned_candidates.insert(candidate);
    }

    fn merge(&mut self, other: &SliceSelectionAccumulator) {
        for path in &other.cache_update_paths {
            self.cache_update_paths.insert(path.clone());
        }
        for path in &other.local_dfg_paths {
            self.local_dfg_paths.insert(path.clone());
        }
        for (path, other_state) in &other.files {
            let state = self.files.entry(path.clone()).or_default();
            state.scopes.cache_update |= other_state.scopes.cache_update;
            state.scopes.local_dfg |= other_state.scopes.local_dfg;
            state.scopes.explanation |= other_state.scopes.explanation;
            state.reasons.extend(other_state.reasons.iter().cloned());
        }
        self.pruned_candidates
            .extend(other.pruned_candidates.iter().cloned());
    }

    fn into_summary(self) -> ImpactSliceSelectionSummary {
        ImpactSliceSelectionSummary {
            planner: ImpactSlicePlannerKind::BoundedSlice,
            files: self
                .files
                .into_iter()
                .map(|(path, state)| ImpactSliceFileMetadata {
                    path,
                    scopes: state.scopes,
                    reasons: state.reasons.into_iter().collect(),
                })
                .collect(),
            pruned_candidates: self.pruned_candidates.into_iter().collect(),
        }
    }
}

fn supports_local_dfg(path: &str) -> bool {
    path.ends_with(".rs") || path.ends_with(".rb")
}

fn is_call_graph_ref(r: &Reference) -> bool {
    r.kind == RefKind::Call && r.provenance == EdgeProvenance::CallGraph
}

fn collect_related_call_symbols(
    symbol_id: &str,
    refs: &[Reference],
    direction: ImpactDirection,
) -> Vec<RelatedCallSymbol> {
    let mut related = std::collections::BTreeSet::new();
    for r in refs.iter().filter(|r| is_call_graph_ref(r)) {
        match direction {
            ImpactDirection::Callers if r.to.0 == symbol_id => {
                related.insert(RelatedCallSymbol {
                    symbol_id: r.from.0.clone(),
                    kind: ImpactSliceReasonKind::DirectCallerFile,
                });
            }
            ImpactDirection::Callees if r.from.0 == symbol_id => {
                related.insert(RelatedCallSymbol {
                    symbol_id: r.to.0.clone(),
                    kind: ImpactSliceReasonKind::DirectCalleeFile,
                });
            }
            ImpactDirection::Both => {
                if r.to.0 == symbol_id {
                    related.insert(RelatedCallSymbol {
                        symbol_id: r.from.0.clone(),
                        kind: ImpactSliceReasonKind::DirectCallerFile,
                    });
                }
                if r.from.0 == symbol_id {
                    related.insert(RelatedCallSymbol {
                        symbol_id: r.to.0.clone(),
                        kind: ImpactSliceReasonKind::DirectCalleeFile,
                    });
                }
            }
            _ => {}
        }
    }
    related.into_iter().collect()
}

fn boundary_follow_direction(kind: ImpactSliceReasonKind) -> ImpactDirection {
    match kind {
        ImpactSliceReasonKind::DirectCallerFile => ImpactDirection::Callers,
        ImpactSliceReasonKind::DirectCalleeFile => ImpactDirection::Callees,
        _ => ImpactDirection::Both,
    }
}

fn bridge_keyword_hint(text: &str) -> bool {
    ["wrap", "wrapper", "adapter", "service"]
        .iter()
        .any(|needle| text.contains(needle))
}

fn return_keyword_hint(text: &str) -> bool {
    [
        "leaf", "source", "core", "callee", "target", "final", "inner",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}

fn alias_keyword_hint(text: &str) -> bool {
    ["alias", "value", "result", "make", "out"]
        .iter()
        .any(|needle| text.contains(needle))
}

fn noise_keyword_hint(text: &str) -> bool {
    ["helper", "noise", "alt", "debug", "tmp"]
        .iter()
        .any(|needle| text.contains(needle))
}

fn has_strong_semantic_tier2_evidence(primary_evidence_kinds: &[ImpactSliceEvidenceKind]) -> bool {
    primary_evidence_kinds.iter().any(|kind| {
        matches!(
            kind,
            ImpactSliceEvidenceKind::AliasChain | ImpactSliceEvidenceKind::ReturnFlow
        )
    })
}

fn ruby_require_relative_scoring_summary(
    completion_file: &str,
    call_line: u32,
    side_max_call_line: u32,
) -> ImpactSliceCandidateScoringSummary {
    let mut primary_evidence_kinds = vec![ImpactSliceEvidenceKind::RequireRelativeEdge];
    let mut secondary_evidence_kinds = Vec::new();
    if call_line == side_max_call_line {
        secondary_evidence_kinds.push(ImpactSliceEvidenceKind::CallsitePositionHint);
    }
    primary_evidence_kinds.sort_by_key(|kind| evidence_kind_key(*kind));
    secondary_evidence_kinds.sort_by_key(|kind| evidence_kind_key(*kind));
    let secondary_evidence_count = u8::try_from(secondary_evidence_kinds.len()).unwrap_or(u8::MAX);
    ImpactSliceCandidateScoringSummary {
        source_kind: ImpactSliceCandidateSourceKind::GraphSecondHop,
        lane: ImpactSliceCandidateLane::RequireRelativeContinuation,
        primary_evidence_kinds,
        secondary_evidence_kinds,
        score_tuple: ImpactSliceScoreTuple {
            source_rank: 0,
            lane_rank: tier2_lane_rank(ImpactSliceCandidateLane::RequireRelativeContinuation),
            primary_evidence_count: 1,
            secondary_evidence_count,
            call_position_rank: call_line,
            lexical_tiebreak: completion_file.to_string(),
        },
        support: None,
    }
}

fn evidence_kind_key(kind: ImpactSliceEvidenceKind) -> &'static str {
    match kind {
        ImpactSliceEvidenceKind::AliasChain => "alias_chain",
        ImpactSliceEvidenceKind::AssignedResult => "assigned_result",
        ImpactSliceEvidenceKind::CallsitePositionHint => "callsite_position_hint",
        ImpactSliceEvidenceKind::CompanionFileMatch => "companion_file_match",
        ImpactSliceEvidenceKind::DynamicDispatchLiteralTarget => "dynamic_dispatch_literal_target",
        ImpactSliceEvidenceKind::ExplicitRequireRelativeLoad => "explicit_require_relative_load",
        ImpactSliceEvidenceKind::ModuleCompanion => "module_companion",
        ImpactSliceEvidenceKind::NamePathHint => "name_path_hint",
        ImpactSliceEvidenceKind::ParamToReturnFlow => "param_to_return_flow",
        ImpactSliceEvidenceKind::RequireRelativeEdge => "require_relative_edge",
        ImpactSliceEvidenceKind::ReturnFlow => "return_flow",
    }
}

fn tier2_lane_rank(lane: ImpactSliceCandidateLane) -> u8 {
    match lane {
        ImpactSliceCandidateLane::ReturnContinuation => 0,
        ImpactSliceCandidateLane::AliasContinuation => 1,
        ImpactSliceCandidateLane::RequireRelativeContinuation => 2,
        ImpactSliceCandidateLane::ModuleCompanionFallback => 3,
    }
}

fn tier2_bridge_kind_for_lane(lane: ImpactSliceCandidateLane) -> Option<ImpactSliceBridgeKind> {
    match lane {
        ImpactSliceCandidateLane::ReturnContinuation => Some(ImpactSliceBridgeKind::WrapperReturn),
        ImpactSliceCandidateLane::AliasContinuation => {
            Some(ImpactSliceBridgeKind::BoundaryAliasContinuation)
        }
        ImpactSliceCandidateLane::RequireRelativeContinuation => {
            Some(ImpactSliceBridgeKind::RequireRelativeChain)
        }
        ImpactSliceCandidateLane::ModuleCompanionFallback => None,
    }
}

fn tier2_scoring_summary(
    boundary_symbol: &dimpact::Symbol,
    boundary_file: &str,
    completion_symbol: &dimpact::Symbol,
    completion_file: &str,
    call_line: u32,
    side_max_call_line: u32,
    semantic_evidence: &Tier2SemanticEvidence,
) -> ImpactSliceCandidateScoringSummary {
    let is_ruby_chain_candidate =
        boundary_file.ends_with(".rb") || completion_file.ends_with(".rb");
    let boundary_name = boundary_symbol.name.to_ascii_lowercase();
    let boundary_path = boundary_file.to_ascii_lowercase();
    let completion_name = completion_symbol.name.to_ascii_lowercase();
    let completion_path = completion_file.to_ascii_lowercase();

    let boundary_wrapper_hint =
        bridge_keyword_hint(boundary_name.as_str()) || bridge_keyword_hint(boundary_path.as_str());
    let completion_return_hint = return_keyword_hint(completion_name.as_str())
        || return_keyword_hint(completion_path.as_str());
    let completion_alias_hint = alias_keyword_hint(completion_name.as_str())
        || alias_keyword_hint(completion_path.as_str());
    let completion_noise_hint = noise_keyword_hint(completion_name.as_str())
        || noise_keyword_hint(completion_path.as_str());
    let has_name_path_hint = boundary_wrapper_hint
        || completion_return_hint
        || completion_alias_hint
        || completion_noise_hint;
    let has_param_to_return_flow = semantic_evidence.param_to_return_flow
        && !completion_return_hint
        && !completion_alias_hint
        && !completion_noise_hint;

    let lane = if completion_return_hint
        || has_param_to_return_flow
        || (boundary_wrapper_hint
            && !completion_alias_hint
            && !completion_noise_hint
            && call_line == side_max_call_line)
    {
        ImpactSliceCandidateLane::ReturnContinuation
    } else {
        ImpactSliceCandidateLane::AliasContinuation
    };

    let mut primary_evidence_kinds = Vec::new();
    match lane {
        ImpactSliceCandidateLane::ReturnContinuation => {
            if completion_return_hint
                || has_param_to_return_flow
                || (!completion_alias_hint && !completion_noise_hint)
            {
                primary_evidence_kinds.push(ImpactSliceEvidenceKind::ReturnFlow);
            }
            if has_param_to_return_flow {
                primary_evidence_kinds.push(ImpactSliceEvidenceKind::ParamToReturnFlow);
            }
            if boundary_wrapper_hint || call_line == side_max_call_line {
                primary_evidence_kinds.push(ImpactSliceEvidenceKind::AssignedResult);
            }
        }
        ImpactSliceCandidateLane::AliasContinuation => {
            if completion_alias_hint {
                primary_evidence_kinds.push(ImpactSliceEvidenceKind::AliasChain);
            }
            if has_param_to_return_flow {
                primary_evidence_kinds.push(ImpactSliceEvidenceKind::ParamToReturnFlow);
            }
            if boundary_wrapper_hint || !completion_noise_hint {
                primary_evidence_kinds.push(ImpactSliceEvidenceKind::AssignedResult);
            }
        }
        ImpactSliceCandidateLane::RequireRelativeContinuation => {
            primary_evidence_kinds.push(ImpactSliceEvidenceKind::RequireRelativeEdge);
        }
        ImpactSliceCandidateLane::ModuleCompanionFallback => {
            primary_evidence_kinds.push(ImpactSliceEvidenceKind::ModuleCompanion);
        }
    }

    if is_ruby_chain_candidate && !has_strong_semantic_tier2_evidence(&primary_evidence_kinds) {
        return ruby_require_relative_scoring_summary(
            completion_file,
            call_line,
            side_max_call_line,
        );
    }

    let mut secondary_evidence_kinds = Vec::new();
    if call_line == side_max_call_line {
        secondary_evidence_kinds.push(ImpactSliceEvidenceKind::CallsitePositionHint);
    }
    if has_name_path_hint {
        secondary_evidence_kinds.push(ImpactSliceEvidenceKind::NamePathHint);
    }
    primary_evidence_kinds.sort_by_key(|kind| evidence_kind_key(*kind));
    primary_evidence_kinds.dedup();
    secondary_evidence_kinds.sort_by_key(|kind| evidence_kind_key(*kind));
    secondary_evidence_kinds.dedup();

    let support = if has_param_to_return_flow {
        Some(ImpactSliceCandidateSupportMetadata {
            local_dfg_support: true,
            ..ImpactSliceCandidateSupportMetadata::default()
        })
    } else {
        None
    };

    ImpactSliceCandidateScoringSummary {
        source_kind: ImpactSliceCandidateSourceKind::GraphSecondHop,
        lane,
        score_tuple: ImpactSliceScoreTuple {
            source_rank: 0,
            lane_rank: tier2_lane_rank(lane),
            primary_evidence_count: u8::try_from(primary_evidence_kinds.len()).unwrap_or(u8::MAX),
            secondary_evidence_count: u8::try_from(secondary_evidence_kinds.len())
                .unwrap_or(u8::MAX),
            call_position_rank: call_line,
            lexical_tiebreak: completion_file.to_string(),
        },
        primary_evidence_kinds,
        secondary_evidence_kinds,
        support,
    }
}

fn compare_tier2_candidates(a: &Tier2Candidate, b: &Tier2Candidate) -> std::cmp::Ordering {
    a.scoring
        .score_tuple
        .source_rank
        .cmp(&b.scoring.score_tuple.source_rank)
        .then_with(|| {
            a.scoring
                .score_tuple
                .lane_rank
                .cmp(&b.scoring.score_tuple.lane_rank)
        })
        .then_with(|| {
            b.scoring
                .score_tuple
                .primary_evidence_count
                .cmp(&a.scoring.score_tuple.primary_evidence_count)
        })
        .then_with(|| {
            b.scoring
                .score_tuple
                .secondary_evidence_count
                .cmp(&a.scoring.score_tuple.secondary_evidence_count)
        })
        .then_with(|| {
            b.scoring
                .score_tuple
                .call_position_rank
                .cmp(&a.scoring.score_tuple.call_position_rank)
        })
        .then_with(|| {
            a.scoring
                .score_tuple
                .lexical_tiebreak
                .cmp(&b.scoring.score_tuple.lexical_tiebreak)
        })
        .then_with(|| a.via_path.cmp(&b.via_path))
        .then_with(|| a.via_symbol_id.cmp(&b.via_symbol_id))
}

fn make_tier2_reason(
    seed_symbol_id: &str,
    candidate: &Tier2Candidate,
) -> ImpactSliceReasonMetadata {
    ImpactSliceReasonMetadata {
        seed_symbol_id: seed_symbol_id.to_string(),
        tier: slice_selection_tier_value(SliceSelectionTier::BridgeCompletion),
        kind: ImpactSliceReasonKind::BridgeCompletionFile,
        via_symbol_id: Some(candidate.via_symbol_id.clone()),
        via_path: Some(candidate.via_path.clone()),
        bridge_kind: candidate.bridge_kind,
        scoring: Some(candidate.scoring.clone()),
    }
}

fn make_tier2_pruned_candidate(
    seed_symbol_id: &str,
    candidate: &Tier2Candidate,
    prune_reason: ImpactSlicePruneReason,
) -> dimpact::ImpactSlicePrunedCandidate {
    dimpact::ImpactSlicePrunedCandidate {
        seed_symbol_id: seed_symbol_id.to_string(),
        path: candidate.path.clone(),
        tier: slice_selection_tier_value(SliceSelectionTier::BridgeCompletion),
        kind: ImpactSliceReasonKind::BridgeCompletionFile,
        via_symbol_id: Some(candidate.via_symbol_id.clone()),
        via_path: Some(candidate.via_path.clone()),
        bridge_kind: candidate.bridge_kind,
        prune_reason,
        scoring: Some(candidate.scoring.clone()),
    }
}

fn plan_bounded_slice(
    cache_update_roots: &[String],
    local_dfg_roots: &[String],
    seeds: &[dimpact::Symbol],
    index: &SymbolIndex,
    refs: &[Reference],
    direction: ImpactDirection,
    root_reason_kind: ImpactSliceReasonKind,
) -> BoundedSlicePlan {
    let mut overall = SliceSelectionAccumulator::default();

    for path in cache_update_roots {
        overall.mark_cache_update(path);
    }
    for path in local_dfg_roots {
        overall.mark_local_dfg(path);
    }
    let symbol_file_by_id: std::collections::HashMap<_, _> = index
        .symbols
        .iter()
        .map(|symbol| (symbol.id.0.clone(), symbol.file.as_str()))
        .collect();
    let symbol_by_id: std::collections::HashMap<_, _> = index
        .symbols
        .iter()
        .map(|symbol| (symbol.id.0.clone(), symbol))
        .collect();
    let mut tier2_semantic_evidence_by_symbol_id = std::collections::HashMap::new();
    let mut per_seed_slice_selection = std::collections::BTreeMap::new();

    for seed in seeds {
        let mut seed_selection = SliceSelectionAccumulator::default();
        let root_reason = ImpactSliceReasonMetadata {
            seed_symbol_id: seed.id.0.clone(),
            tier: slice_selection_tier_value(SliceSelectionTier::Root),
            kind: root_reason_kind,
            via_symbol_id: None,
            via_path: None,
            bridge_kind: None,
            scoring: None,
        };
        seed_selection.add_reason(seed.file.as_str(), root_reason.clone());

        let root_file = seed.file.as_str();
        let direct_boundary_symbols =
            collect_related_call_symbols(seed.id.0.as_str(), refs, direction);
        let direct_boundary_paths: std::collections::BTreeSet<String> = direct_boundary_symbols
            .iter()
            .filter_map(|boundary| symbol_file_by_id.get(&boundary.symbol_id).copied())
            .filter(|path| *path != root_file)
            .map(str::to_string)
            .collect();
        let mut tier2_candidates = Vec::new();

        for boundary in &direct_boundary_symbols {
            let Some(boundary_file) = symbol_file_by_id.get(&boundary.symbol_id).copied() else {
                continue;
            };
            if boundary_file != root_file {
                seed_selection.add_reason(
                    boundary_file,
                    ImpactSliceReasonMetadata {
                        seed_symbol_id: seed.id.0.clone(),
                        tier: slice_selection_tier_value(SliceSelectionTier::DirectBoundary),
                        kind: boundary.kind,
                        via_symbol_id: Some(boundary.symbol_id.clone()),
                        via_path: None,
                        bridge_kind: None,
                        scoring: None,
                    },
                );
            }

            let Some(boundary_symbol) = symbol_by_id.get(&boundary.symbol_id).copied() else {
                continue;
            };

            let side_refs: Vec<_> = refs
                .iter()
                .filter(|r| is_call_graph_ref(r))
                .filter_map(|reference| {
                    let completion_symbol_id = match boundary_follow_direction(boundary.kind) {
                        ImpactDirection::Callers if reference.to.0 == boundary.symbol_id => {
                            Some(reference.from.0.as_str())
                        }
                        ImpactDirection::Callees if reference.from.0 == boundary.symbol_id => {
                            Some(reference.to.0.as_str())
                        }
                        ImpactDirection::Both => {
                            if reference.to.0 == boundary.symbol_id {
                                Some(reference.from.0.as_str())
                            } else if reference.from.0 == boundary.symbol_id {
                                Some(reference.to.0.as_str())
                            } else {
                                None
                            }
                        }
                        _ => None,
                    }?;
                    let completion_file = symbol_file_by_id.get(completion_symbol_id).copied()?;
                    if completion_file == root_file || completion_file == boundary_file {
                        return None;
                    }
                    if direct_boundary_paths.contains(completion_file) {
                        return None;
                    }
                    Some((
                        completion_symbol_id.to_string(),
                        completion_file,
                        reference.line,
                    ))
                })
                .collect();
            let side_max_call_line = side_refs
                .iter()
                .map(|(_, _, line)| *line)
                .max()
                .unwrap_or_default();

            let mut side_candidates = std::collections::BTreeMap::new();
            for (completion_symbol_id, completion_file, call_line) in side_refs {
                let Some(completion_symbol) = symbol_by_id.get(&completion_symbol_id).copied()
                else {
                    continue;
                };
                let semantic_evidence = tier2_semantic_evidence_by_symbol_id
                    .entry(completion_symbol_id.clone())
                    .or_insert_with(|| collect_rust_tier2_semantic_evidence(completion_symbol))
                    .clone();
                let scoring = tier2_scoring_summary(
                    boundary_symbol,
                    boundary_file,
                    completion_symbol,
                    completion_file,
                    call_line,
                    side_max_call_line,
                    &semantic_evidence,
                );
                let candidate = Tier2Candidate {
                    path: completion_file.to_string(),
                    via_symbol_id: boundary.symbol_id.clone(),
                    via_path: boundary_file.to_string(),
                    bridge_kind: tier2_bridge_kind_for_lane(scoring.lane),
                    scoring,
                };
                side_candidates
                    .entry(completion_file.to_string())
                    .and_modify(|existing: &mut Tier2Candidate| {
                        if compare_tier2_candidates(&candidate, existing).is_lt() {
                            *existing = candidate.clone();
                        }
                    })
                    .or_insert(candidate);
            }

            let mut side_candidates: Vec<_> = side_candidates.into_values().collect();
            side_candidates.sort_by(compare_tier2_candidates);
            for candidate in side_candidates
                .iter()
                .skip(PER_BOUNDARY_SIDE_TIER2_FILES_MAX)
            {
                seed_selection.add_pruned_candidate(make_tier2_pruned_candidate(
                    seed.id.0.as_str(),
                    candidate,
                    ImpactSlicePruneReason::RankedOut,
                ));
            }
            tier2_candidates.extend(
                side_candidates
                    .into_iter()
                    .take(PER_BOUNDARY_SIDE_TIER2_FILES_MAX),
            );
        }

        tier2_candidates.sort_by(compare_tier2_candidates);
        let mut selected_tier2_paths = std::collections::BTreeSet::new();
        for candidate in tier2_candidates {
            if selected_tier2_paths.contains(&candidate.path) {
                seed_selection.add_reason(
                    candidate.path.as_str(),
                    make_tier2_reason(seed.id.0.as_str(), &candidate),
                );
                continue;
            }
            if selected_tier2_paths.len() >= PER_SEED_TIER2_FILES_MAX {
                seed_selection.add_pruned_candidate(make_tier2_pruned_candidate(
                    seed.id.0.as_str(),
                    &candidate,
                    ImpactSlicePruneReason::BridgeBudgetExhausted,
                ));
                continue;
            }
            seed_selection.add_reason(
                candidate.path.as_str(),
                make_tier2_reason(seed.id.0.as_str(), &candidate),
            );
            selected_tier2_paths.insert(candidate.path);
        }

        overall.merge(&seed_selection);
        per_seed_slice_selection.insert(seed.id.0.clone(), seed_selection.into_summary());
    }

    let cache_update_paths: Vec<String> = overall.cache_update_paths.iter().cloned().collect();
    let local_dfg_paths: Vec<String> = overall.local_dfg_paths.iter().cloned().collect();
    let slice_selection = overall.into_summary();

    BoundedSlicePlan {
        cache_update_paths,
        local_dfg_paths,
        slice_selection,
        per_seed_slice_selection,
    }
}

fn validate_selected_engine_for_pdg_diff(
    engine: &dyn dimpact::engine::AnalysisEngine,
    files: &[dimpact::FileChanges],
    lang: LanguageMode,
    opts: &ImpactOptions,
) -> anyhow::Result<()> {
    let _ = engine.impact(files, lang, opts)?;
    Ok(())
}

fn build_pdg_context(
    cache_update_paths: &[String],
    local_dfg_paths: &[String],
    seeds: &[dimpact::Symbol],
    direction: ImpactDirection,
    with_propagation: bool,
    root_reason_kind: ImpactSliceReasonKind,
) -> anyhow::Result<PdgContext> {
    let (scope, dir_override) = cache::scope_from_env();
    let mut db = cache::open(scope, dir_override.as_deref())?;
    let st = cache::stats(&db.conn)?;
    if st.symbols == 0 {
        cache::build_all(&mut db.conn)?;
    }

    let mut initial_cache_update_paths: std::collections::BTreeSet<String> =
        cache_update_paths.iter().cloned().collect();
    initial_cache_update_paths.extend(local_dfg_paths.iter().cloned());
    initial_cache_update_paths.extend(seeds.iter().map(|seed| seed.file.clone()));
    let initial_cache_update_paths: Vec<String> = initial_cache_update_paths.into_iter().collect();

    if !initial_cache_update_paths.is_empty() {
        cache::update_paths(&mut db.conn, &initial_cache_update_paths)?;
    }

    let (mut index, mut refs) = cache::load_graph(&db.conn)?;
    let plan = plan_bounded_slice(
        &initial_cache_update_paths,
        local_dfg_paths,
        seeds,
        &index,
        &refs,
        direction,
        root_reason_kind,
    );

    let additional_cache_update_paths: Vec<String> = plan
        .cache_update_paths
        .iter()
        .filter(|path| !initial_cache_update_paths.contains(path))
        .cloned()
        .collect();
    if !additional_cache_update_paths.is_empty() {
        cache::update_paths(&mut db.conn, &additional_cache_update_paths)?;
        let loaded = cache::load_graph(&db.conn)?;
        index = loaded.0;
        refs = loaded.1;
    }

    let combined = build_local_dfg_for_paths(plan.local_dfg_paths.iter().map(String::as_str));
    let mut pdg = PdgBuilder::build(&combined, &refs);
    if with_propagation {
        PdgBuilder::augment_symbolic_propagation(&mut pdg, &refs, &index);
    }
    let pdg_refs = merge_pdg_references(&combined, &pdg, &refs);
    Ok(PdgContext {
        index,
        pdg,
        refs: pdg_refs,
        slice_selection: plan.slice_selection,
        per_seed_slice_selection: plan.per_seed_slice_selection,
    })
}

fn build_grouped_impact_outputs(
    seeds: &[dimpact::Symbol],
    refs: &[Reference],
    index: &SymbolIndex,
    opts: &ImpactOptions,
    min_confidence: Option<ConfidenceOpt>,
    exclude_dynamic_fallback: bool,
    with_edges: bool,
    slice_selection_by_seed: Option<
        &std::collections::BTreeMap<String, ImpactSliceSelectionSummary>,
    >,
) -> Vec<PerSeedOutput> {
    let mut grouped: Vec<PerSeedOutput> = Vec::new();
    for seed in seeds {
        let mut impacts: Vec<PerSeedImpact> = Vec::new();
        let slice_selection =
            slice_selection_by_seed.and_then(|summaries| summaries.get(&seed.id.0));
        if opts.direction == ImpactDirection::Both {
            let mut o = opts.clone();
            o.direction = ImpactDirection::Callers;
            let (mut output, confidence_filter) = apply_confidence_filter(
                compute_impact(std::slice::from_ref(seed), index, refs, &o),
                &o,
                min_confidence,
                exclude_dynamic_fallback,
                with_edges,
            );
            if let Some(slice_selection) = slice_selection {
                attach_slice_selection_summary(&mut output, slice_selection);
            }
            impacts.push(PerSeedImpact {
                direction: ImpactDirection::Callers,
                output,
                confidence_filter,
            });
            let mut o2 = opts.clone();
            o2.direction = ImpactDirection::Callees;
            let (mut output, confidence_filter) = apply_confidence_filter(
                compute_impact(std::slice::from_ref(seed), index, refs, &o2),
                &o2,
                min_confidence,
                exclude_dynamic_fallback,
                with_edges,
            );
            if let Some(slice_selection) = slice_selection {
                attach_slice_selection_summary(&mut output, slice_selection);
            }
            impacts.push(PerSeedImpact {
                direction: ImpactDirection::Callees,
                output,
                confidence_filter,
            });
        } else {
            let (mut output, confidence_filter) = apply_confidence_filter(
                compute_impact(std::slice::from_ref(seed), index, refs, opts),
                opts,
                min_confidence,
                exclude_dynamic_fallback,
                with_edges,
            );
            if let Some(slice_selection) = slice_selection {
                attach_slice_selection_summary(&mut output, slice_selection);
            }
            impacts.push(PerSeedImpact {
                direction: opts.direction,
                output,
                confidence_filter,
            });
        }
        grouped.push(PerSeedOutput {
            changed_symbol: seed.clone(),
            impacts,
        });
    }
    grouped
}

fn merge_pdg_references(
    combined: &DataFlowGraph,
    pdg: &DataFlowGraph,
    refs: &[Reference],
) -> Vec<Reference> {
    let mut merged = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut push_ref = |r: Reference| {
        let key = (
            r.from.0.clone(),
            r.to.0.clone(),
            r.kind.clone(),
            r.file.clone(),
            r.line,
            r.certainty.clone(),
            r.provenance.clone(),
        );
        if seen.insert(key) {
            merged.push(r);
        }
    };

    let nodes_by_id: std::collections::HashMap<String, (String, u32)> = combined
        .nodes
        .iter()
        .map(|n| (n.id.clone(), (n.file.clone(), n.line)))
        .collect();
    let mut callsite_certainty_by_loc: std::collections::HashMap<(String, u32), EdgeCertainty> =
        std::collections::HashMap::new();
    for r in refs {
        let key = (r.file.clone(), r.line);
        callsite_certainty_by_loc
            .entry(key)
            .and_modify(|existing| {
                *existing = strongest_certainty(existing.clone(), r.certainty.clone())
            })
            .or_insert_with(|| r.certainty.clone());
        let mut base_ref = r.clone();
        base_ref.provenance = EdgeProvenance::CallGraph;
        push_ref(base_ref);
    }

    for e in &combined.edges {
        let (file, line, _) =
            edge_location_from_nodes(&e.from, &e.to, &nodes_by_id, &callsite_certainty_by_loc);
        push_ref(Reference {
            from: SymbolId(e.from.clone()),
            to: SymbolId(e.to.clone()),
            kind: ref_kind_from_dependency(&e.kind),
            file,
            line,
            certainty: EdgeCertainty::Inferred,
            provenance: EdgeProvenance::LocalDfg,
        });
    }

    let mut base_edge_keys: std::collections::HashSet<(String, String, DependencyKind)> = combined
        .edges
        .iter()
        .map(|e| (e.from.clone(), e.to.clone(), e.kind.clone()))
        .collect();
    base_edge_keys.extend(
        refs.iter()
            .map(|r| (r.from.0.clone(), r.to.0.clone(), DependencyKind::Data)),
    );

    for e in &pdg.edges {
        let key = (e.from.clone(), e.to.clone(), e.kind.clone());
        if base_edge_keys.contains(&key) {
            continue;
        }
        let (file, line, derived_certainty) =
            edge_location_from_nodes(&e.from, &e.to, &nodes_by_id, &callsite_certainty_by_loc);
        push_ref(Reference {
            from: SymbolId(e.from.clone()),
            to: SymbolId(e.to.clone()),
            kind: ref_kind_from_dependency(&e.kind),
            file,
            line,
            certainty: derived_certainty.unwrap_or(EdgeCertainty::Inferred),
            provenance: EdgeProvenance::SymbolicPropagation,
        });
    }

    merged
}

fn run_impact(
    fmt: OutputFormat,
    lang_opt: LangOpt,
    dir_opt: DirectionOpt,
    max_depth: Option<usize>,
    with_edges: bool,
    min_confidence: Option<ConfidenceOpt>,
    exclude_dynamic_fallback: bool,
    op_profile: Option<OperationalProfileOpt>,
    with_pdg: bool,
    with_propagation: bool,
    engine_opt: EngineOpt,
    auto_policy: Option<AutoPolicyOpt>,
    lsp_strict: bool,
    dump_caps: bool,
    seed_symbols: Vec<String>,
    seed_json: Option<String>,
    ignore_dir: Vec<String>,
    per_seed: bool,
) -> anyhow::Result<()> {
    // Gather seeds
    let mut seeds: Vec<dimpact::Symbol> = Vec::new();
    if let Some(sj) = seed_json.as_ref() {
        let mut from_json = parse_seed_json_input(sj)?;
        seeds.append(&mut from_json);
    }
    if !seed_symbols.is_empty() {
        for s in &seed_symbols {
            seeds.push(parse_seed_symbol(s)?);
        }
    }

    // Determine language: prefer seeds' language when provided
    let lang: LanguageMode = if !seeds.is_empty() {
        let mut langs: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for s in &seeds {
            langs.insert(s.language.to_ascii_lowercase());
        }
        if langs.len() > 1 {
            anyhow::bail!("mixed seed languages not supported: {:?}", langs);
        }
        let seed_lang = langs
            .iter()
            .next()
            .cloned()
            .unwrap_or_else(|| "auto".to_string());
        lang_mode_from_str(&seed_lang)
            .ok_or_else(|| anyhow::anyhow!("unknown seed language: {}", seed_lang))?
    } else {
        match lang_opt {
            LangOpt::Auto => LanguageMode::Auto,
            LangOpt::Rust => LanguageMode::Rust,
            LangOpt::Ruby => LanguageMode::Ruby,
            LangOpt::Python => LanguageMode::Python,
            LangOpt::Javascript => LanguageMode::Javascript,
            LangOpt::Typescript => LanguageMode::Typescript,
            LangOpt::Tsx => LanguageMode::Tsx,
            LangOpt::Go => LanguageMode::Go,
            LangOpt::Java => LanguageMode::Java,
        }
    };
    let direction = match dir_opt {
        DirectionOpt::Callers => ImpactDirection::Callers,
        DirectionOpt::Callees => ImpactDirection::Callees,
        DirectionOpt::Both => ImpactDirection::Both,
    };
    let (min_confidence, exclude_dynamic_fallback) =
        resolve_operational_profile(op_profile, min_confidence, exclude_dynamic_fallback);
    let compute_with_edges = with_edges || min_confidence.is_some() || exclude_dynamic_fallback;
    let opts = ImpactOptions {
        direction,
        max_depth: max_depth.or(Some(100)),
        with_edges: Some(compute_with_edges),
        ignore_dirs: ignore_dir.clone(),
    };
    let ekind = match engine_opt {
        EngineOpt::Auto => EngineKind::Auto,
        EngineOpt::Ts => EngineKind::Ts,
        EngineOpt::Lsp => EngineKind::Lsp,
    };
    let ecfg = EngineConfig {
        lsp_strict,
        dump_capabilities: dump_caps,
        mock_lsp: std::env::var("DIMPACT_TEST_LSP_MOCK").ok().as_deref() == Some("1"),
        mock_caps: None,
    };
    let engine = make_engine_with_auto_policy(ekind, ecfg, auto_policy.map(map_auto_policy));
    if dump_caps && !matches!(engine_opt, EngineOpt::Lsp) {
        eprintln!(
            "{}",
            serde_json::json!({
                "document_symbol": false,
                "workspace_symbol": false,
                "references": false,
                "definition": false,
                "call_hierarchy": false,
            })
        );
    }

    // Per-seed grouping for call-graph or PDG-enhanced impact (diff or seed based)
    if per_seed {
        // Diff-based grouping: seeds := changed symbols
        if seeds.is_empty() {
            let diff_text = read_diff_from_stdin()?;
            let files = match parse_unified_diff(&diff_text) {
                Ok(f) => f,
                Err(DiffParseError::MissingHeader) => Vec::new(),
                Err(e) => return Err(anyhow::anyhow!(e)),
            };
            if with_pdg || with_propagation {
                let changed: ChangedOutput = engine.changed_symbols(&files, lang)?;
                validate_selected_engine_for_pdg_diff(&*engine, &files, lang, &opts)?;
                let pdg = build_pdg_context(
                    &changed.changed_files,
                    &changed.changed_files,
                    &changed.changed_symbols,
                    opts.direction,
                    with_propagation,
                    ImpactSliceReasonKind::ChangedFile,
                )?;
                let grouped = build_grouped_impact_outputs(
                    &changed.changed_symbols,
                    &pdg.refs,
                    &pdg.index,
                    &opts,
                    min_confidence,
                    exclude_dynamic_fallback,
                    with_edges,
                    Some(&pdg.per_seed_slice_selection),
                );
                println!("{}", serde_json::to_string_pretty(&grouped)?);
                return Ok(());
            }

            let changed: ChangedOutput = engine.changed_symbols(&files, lang)?;
            let (scope, dir_override) = cache::scope_from_env();
            let mut db = cache::open(scope, dir_override.as_deref())?;
            let st = cache::stats(&db.conn)?;
            if st.symbols == 0 {
                cache::build_all(&mut db.conn)?;
            }
            if !changed.changed_files.is_empty() {
                cache::update_paths(&mut db.conn, &changed.changed_files)?;
            }
            let (index, refs) = cache::load_graph(&db.conn)?;
            let grouped = build_grouped_impact_outputs(
                &changed.changed_symbols,
                &refs,
                &index,
                &opts,
                min_confidence,
                exclude_dynamic_fallback,
                with_edges,
                None,
            );
            println!("{}", serde_json::to_string_pretty(&grouped)?);
            return Ok(());
        }
        // Seed-based grouping: group per provided seed
        if with_pdg || with_propagation {
            let fileset: std::collections::BTreeSet<String> =
                seeds.iter().map(|s| s.file.clone()).collect();
            let local_dfg_paths: Vec<String> = fileset.into_iter().collect();
            let pdg = build_pdg_context(
                &[],
                &local_dfg_paths,
                &seeds,
                opts.direction,
                with_propagation,
                ImpactSliceReasonKind::SeedFile,
            )?;
            let grouped = build_grouped_impact_outputs(
                &seeds,
                &pdg.refs,
                &pdg.index,
                &opts,
                min_confidence,
                exclude_dynamic_fallback,
                with_edges,
                Some(&pdg.per_seed_slice_selection),
            );
            println!("{}", serde_json::to_string_pretty(&grouped)?);
            return Ok(());
        }

        let (scope, dir_override) = cache::scope_from_env();
        let mut db = cache::open(scope, dir_override.as_deref())?;
        let st = cache::stats(&db.conn)?;
        if st.symbols == 0 {
            cache::build_all(&mut db.conn)?;
        }
        let (index, refs) = cache::load_graph(&db.conn)?;
        let grouped = build_grouped_impact_outputs(
            &seeds,
            &refs,
            &index,
            &opts,
            min_confidence,
            exclude_dynamic_fallback,
            with_edges,
            None,
        );
        println!("{}", serde_json::to_string_pretty(&grouped)?);
        return Ok(());
    }

    // diff-based impact (default when --per-seed not set and no seeds)
    if seeds.is_empty() {
        let diff_text = read_diff_from_stdin()?;
        let files = match parse_unified_diff(&diff_text) {
            Ok(f) => f,
            Err(DiffParseError::MissingHeader) => Vec::new(),
            Err(e) => return Err(anyhow::anyhow!(e)),
        };
        log::info!(
            "mode=impact(diff) engine={:?} files={} lang={:?} dir={:?} max_depth={:?} with_edges={} profile={:?} min_conf={:?} exclude_dynamic_fallback={} pdg={} ignore_dirs={:?}",
            ekind,
            files.len(),
            lang,
            direction,
            opts.max_depth,
            compute_with_edges,
            op_profile,
            min_confidence,
            exclude_dynamic_fallback,
            with_pdg,
            opts.ignore_dirs
        );
        if with_pdg || with_propagation {
            let changed: ChangedOutput = engine.changed_symbols(&files, lang)?;
            validate_selected_engine_for_pdg_diff(&*engine, &files, lang, &opts)?;
            let pdg = build_pdg_context(
                &changed.changed_files,
                &changed.changed_files,
                &changed.changed_symbols,
                opts.direction,
                with_propagation,
                ImpactSliceReasonKind::ChangedFile,
            )?;
            if matches!(fmt, OutputFormat::Dot) {
                println!("{}", dfg_to_dot(&pdg.pdg));
                return Ok(());
            }
            let (mut out, confidence_filter) = apply_confidence_filter(
                compute_impact(&changed.changed_symbols, &pdg.index, &pdg.refs, &opts),
                &opts,
                min_confidence,
                exclude_dynamic_fallback,
                with_edges,
            );
            attach_slice_selection_summary(&mut out, &pdg.slice_selection);
            print_impact_output(fmt, &out, confidence_filter.as_ref())?;
            return Ok(());
        }
        let (out, confidence_filter) = apply_confidence_filter(
            engine.impact(&files, lang, &opts)?,
            &opts,
            min_confidence,
            exclude_dynamic_fallback,
            with_edges,
        );
        print_impact_output(fmt, &out, confidence_filter.as_ref())?;
        return Ok(());
    }

    log::info!(
        "mode=impact(seeds) engine={:?} seeds={} lang={:?} dir={:?} max_depth={:?} with_edges={} profile={:?} min_conf={:?} exclude_dynamic_fallback={} ignore_dirs={:?}",
        ekind,
        seeds.len(),
        lang,
        direction,
        opts.max_depth,
        compute_with_edges,
        op_profile,
        min_confidence,
        exclude_dynamic_fallback,
        opts.ignore_dirs
    );
    if with_pdg || with_propagation {
        let fileset: std::collections::BTreeSet<String> =
            seeds.iter().map(|s| s.file.clone()).collect();
        let local_dfg_paths: Vec<String> = fileset.into_iter().collect();
        let pdg = build_pdg_context(
            &[],
            &local_dfg_paths,
            &seeds,
            opts.direction,
            with_propagation,
            ImpactSliceReasonKind::SeedFile,
        )?;
        let (mut out, confidence_filter) = apply_confidence_filter(
            compute_impact(&seeds, &pdg.index, &pdg.refs, &opts),
            &opts,
            min_confidence,
            exclude_dynamic_fallback,
            with_edges,
        );
        attach_slice_selection_summary(&mut out, &pdg.slice_selection);
        print_impact_output(fmt, &out, confidence_filter.as_ref())?;
        return Ok(());
    }

    let (out, confidence_filter) = apply_confidence_filter(
        engine.impact_from_symbols(&seeds, lang, &opts)?,
        &opts,
        min_confidence,
        exclude_dynamic_fallback,
        with_edges,
    );
    print_impact_output(fmt, &out, confidence_filter.as_ref())?;
    Ok(())
}

fn lang_mode_from_str(s: &str) -> Option<LanguageMode> {
    match s.to_ascii_lowercase().as_str() {
        "rust" => Some(LanguageMode::Rust),
        "ruby" => Some(LanguageMode::Ruby),
        "javascript" | "js" => Some(LanguageMode::Javascript),
        "typescript" | "ts" => Some(LanguageMode::Typescript),
        "tsx" => Some(LanguageMode::Tsx),
        "go" | "golang" => Some(LanguageMode::Go),
        "java" => Some(LanguageMode::Java),
        "python" | "py" => Some(LanguageMode::Python),
        "auto" => Some(LanguageMode::Auto),
        _ => None,
    }
}

fn run_id(
    fmt: OutputFormat,
    path: Option<&str>,
    line: Option<u32>,
    name: Option<&str>,
    lang_opt: LangOpt,
    kind_opt: Option<KindOpt>,
    raw: bool,
) -> anyhow::Result<()> {
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
            LangOpt::Python => dimpact::LanguageKind::Python,
            LangOpt::Javascript => dimpact::LanguageKind::Javascript,
            LangOpt::Typescript => dimpact::LanguageKind::Typescript,
            LangOpt::Tsx => dimpact::LanguageKind::Tsx,
            LangOpt::Go => dimpact::LanguageKind::Go,
            LangOpt::Java => dimpact::LanguageKind::Java,
        };
        let Some(analyzer) = dimpact::languages::analyzer_for_path(fp, lkind) else {
            continue;
        };
        let Ok(source) = fs::read_to_string(fp) else {
            continue;
        };
        let mut syms = analyzer.symbols_in_file(fp, &source);
        all_syms.append(&mut syms);
    }

    if all_syms.is_empty() {
        anyhow::bail!("no symbols found in search scope");
    }

    // Stepwise narrowing: path -> line -> name -> kind (each only if yields results)
    let mut current: Vec<dimpact::Symbol> = all_syms.clone();
    if let Some(p) = path {
        let subset: Vec<_> = current.iter().filter(|s| s.file == p).cloned().collect();
        if !subset.is_empty() {
            current = subset;
        } else {
            current = all_syms.clone();
        }
    }
    if let Some(ln) = line {
        let subset: Vec<_> = current
            .iter()
            .filter(|s| s.range.start_line <= ln && ln <= s.range.end_line)
            .cloned()
            .collect();
        if !subset.is_empty() {
            current = subset;
        }
    }
    if let Some(nm) = name {
        let subset: Vec<_> = current.iter().filter(|s| s.name == nm).cloned().collect();
        if !subset.is_empty() {
            current = subset;
        }
    }
    if let Some(kopt) = kind_opt {
        let want = map_kind_opt(kopt);
        let subset: Vec<_> = current.iter().filter(|s| s.kind == want).cloned().collect();
        if !subset.is_empty() {
            current = subset;
        }
    }

    if current.is_empty() {
        anyhow::bail!("no matching symbol candidates");
    }

    let mut sorted = current;
    sorted.sort_by_key(|s| (s.range.end_line - s.range.start_line, key_of_kind(&s.kind)));

    if raw {
        for s in &sorted {
            println!("{}", s.id.0);
        }
        return Ok(());
    }

    match fmt {
        OutputFormat::Json => {
            let items: Vec<serde_json::Value> = sorted
                .iter()
                .map(|s| {
                    serde_json::json!({
                        "id": s.id.0,
                        "symbol": s,
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&items)?);
        }
        OutputFormat::Yaml => {
            print!("{}", serde_yaml::to_string(&sorted)?);
        }
        OutputFormat::Dot | OutputFormat::Html => {
            anyhow::bail!("format not supported for 'id': use json|yaml or --raw")
        }
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
        LangOpt::Auto => vec!["rs", "rb", "js", "ts", "tsx", "py", "go", "java"],
        LangOpt::Rust => vec!["rs"],
        LangOpt::Ruby => vec!["rb"],
        LangOpt::Python => vec!["py"],
        LangOpt::Javascript => vec!["js"],
        LangOpt::Typescript => vec!["ts"],
        LangOpt::Tsx => vec!["tsx"],
        LangOpt::Go => vec!["go"],
        LangOpt::Java => vec!["java"],
    };
    let root = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    scan_dir(&root, &exts, &mut out)?;
    Ok(out)
}

fn scan_dir(dir: &std::path::Path, exts: &[&str], out: &mut Vec<String>) -> anyhow::Result<()> {
    if let Some(name) = dir.file_name().and_then(|s| s.to_str())
        && [".git", "target", "node_modules"].contains(&name)
    {
        return Ok(());
    }
    let rd = match fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return Ok(()),
    };
    for ent in rd {
        let ent = match ent {
            Ok(e) => e,
            Err(_) => continue,
        };
        let p = ent.path();
        let Ok(ft) = ent.file_type() else { continue };
        if ft.is_dir() {
            scan_dir(&p, exts, out)?;
            continue;
        }
        if !ft.is_file() {
            continue;
        }
        let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("");
        if exts.contains(&ext) {
            out.push(p.to_string_lossy().to_string());
        }
    }
    Ok(())
}

#[allow(dead_code)]
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

#[allow(dead_code)]
fn impact_from_diff(args: Args, files: Vec<dimpact::FileChanges>) -> anyhow::Result<()> {
    let lang = match args.lang {
        LangOpt::Auto => LanguageMode::Auto,
        LangOpt::Rust => LanguageMode::Rust,
        LangOpt::Ruby => LanguageMode::Ruby,
        LangOpt::Python => LanguageMode::Python,
        LangOpt::Javascript => LanguageMode::Javascript,
        LangOpt::Typescript => LanguageMode::Typescript,
        LangOpt::Tsx => LanguageMode::Tsx,
        LangOpt::Go => LanguageMode::Go,
        LangOpt::Java => LanguageMode::Java,
    };
    let direction = match args.direction {
        DirectionOpt::Callers => ImpactDirection::Callers,
        DirectionOpt::Callees => ImpactDirection::Callees,
        DirectionOpt::Both => ImpactDirection::Both,
    };
    let opts = ImpactOptions {
        direction,
        max_depth: args.max_depth.or(Some(100)),
        with_edges: Some(args.with_edges),
        ignore_dirs: args.ignore_dir.clone(),
    };
    let ekind = match args.engine {
        EngineOpt::Auto => EngineKind::Auto,
        EngineOpt::Ts => EngineKind::Ts,
        EngineOpt::Lsp => EngineKind::Lsp,
    };
    let ecfg = EngineConfig {
        lsp_strict: args.engine_lsp_strict,
        dump_capabilities: args.engine_dump_capabilities,
        mock_lsp: false,
        mock_caps: None,
    };
    let engine = make_engine_with_auto_policy(ekind, ecfg, args.auto_policy.map(map_auto_policy));
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
#[derive(Debug, Clone, Copy, ValueEnum)]
enum CompletionShell {
    Bash,
    Zsh,
    Fish,
    PowerShell,
    Elvish,
}

#[cfg(test)]
mod tests {
    use super::*;
    use dimpact::engine::CapsHint;
    use serial_test::serial;
    use std::fs;
    use std::process::Command as ProcessCommand;
    use tempfile::TempDir;

    fn git(cwd: &std::path::Path, args: &[&str]) -> std::process::Output {
        let mut cmd = ProcessCommand::new("git");
        cmd.args(args).current_dir(cwd);
        let out = cmd.output().expect("git command failed to spawn");
        if !out.status.success() {
            panic!(
                "git {:?} failed: status {:?}\nstdout:{}\nstderr:{}",
                args,
                out.status,
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
        }
        out
    }

    fn setup_pdg_engine_repo() -> (TempDir, std::path::PathBuf, Vec<dimpact::FileChanges>) {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().to_path_buf();
        git(&path, &["init", "-q"]);
        git(&path, &["config", "user.email", "tester@example.com"]);
        git(&path, &["config", "user.name", "Tester"]);

        fs::write(
            path.join("main.rs"),
            r#"fn callee(value: i32) -> i32 {
    value + 1
}

fn caller() -> i32 {
    let x = 1;
    callee(x)
}
"#,
        )
        .unwrap();
        git(&path, &["add", "."]);
        git(&path, &["commit", "-m", "init", "-q"]);

        fs::write(
            path.join("main.rs"),
            r#"fn callee(value: i32) -> i32 {
    value + 1
}

fn caller() -> i32 {
    let x = 2;
    callee(x)
}
"#,
        )
        .unwrap();

        let diff_out = git(&path, &["diff", "--no-ext-diff", "--unified=0"]);
        let diff = String::from_utf8(diff_out.stdout).unwrap();
        let files = parse_unified_diff(&diff).expect("parse diff");
        (dir, path, files)
    }

    #[test]
    fn lang_mode_from_str_accepts_python_aliases() {
        assert_eq!(lang_mode_from_str("python"), Some(LanguageMode::Python));
        assert_eq!(lang_mode_from_str("Py"), Some(LanguageMode::Python));
    }

    #[test]
    fn lang_mode_from_str_accepts_go_java_aliases() {
        assert_eq!(lang_mode_from_str("go"), Some(LanguageMode::Go));
        assert_eq!(lang_mode_from_str("golang"), Some(LanguageMode::Go));
        assert_eq!(lang_mode_from_str("java"), Some(LanguageMode::Java));
    }

    #[test]
    fn cli_lang_value_enum_accepts_go_java_python_and_keeps_rust() {
        let a = Args::try_parse_from(["dimpact", "changed", "--lang", "go"])
            .expect("go should be accepted by --lang");
        match a.cmd {
            Some(Command::Changed { lang, .. }) => assert!(matches!(lang, LangOpt::Go)),
            _ => panic!("expected changed subcommand"),
        }

        let b = Args::try_parse_from(["dimpact", "impact", "--lang", "java"])
            .expect("java should be accepted by --lang");
        match b.cmd {
            Some(Command::Impact { lang, .. }) => assert!(matches!(lang, LangOpt::Java)),
            _ => panic!("expected impact subcommand"),
        }

        let c = Args::try_parse_from(["dimpact", "changed", "--lang", "python"])
            .expect("python should be accepted by --lang");
        match c.cmd {
            Some(Command::Changed { lang, .. }) => assert!(matches!(lang, LangOpt::Python)),
            _ => panic!("expected changed subcommand"),
        }

        let d = Args::try_parse_from(["dimpact", "changed", "--lang", "py"])
            .expect("py alias should be accepted by --lang");
        match d.cmd {
            Some(Command::Changed { lang, .. }) => assert!(matches!(lang, LangOpt::Python)),
            _ => panic!("expected changed subcommand"),
        }

        let e = Args::try_parse_from(["dimpact", "changed", "--lang", "rust"])
            .expect("rust should keep working by --lang");
        match e.cmd {
            Some(Command::Changed { lang, .. }) => assert!(matches!(lang, LangOpt::Rust)),
            _ => panic!("expected changed subcommand"),
        }
    }

    #[test]
    fn cli_auto_policy_accepts_strict_if_available() {
        let a = Args::try_parse_from([
            "dimpact",
            "changed",
            "--engine",
            "auto",
            "--auto-policy",
            "strict-if-available",
        ])
        .expect("strict-if-available should be accepted");
        assert!(matches!(
            a.auto_policy,
            Some(AutoPolicyOpt::StrictIfAvailable)
        ));
    }

    #[test]
    fn cli_auto_policy_defaults_to_env_when_unspecified() {
        // SAFETY: tests in this module are single-threaded in this process context.
        unsafe {
            std::env::set_var("DIMPACT_AUTO_POLICY", "strict-if-available");
        }
        let a = Args::try_parse_from(["dimpact", "changed", "--engine", "auto"])
            .expect("auto policy omitted should still parse");
        assert!(a.auto_policy.is_none());
        unsafe {
            std::env::remove_var("DIMPACT_AUTO_POLICY");
        }
    }

    #[test]
    #[serial]
    fn pdg_diff_validation_honors_strict_lsp_impact_capabilities() {
        let (_tmp, repo, files) = setup_pdg_engine_repo();
        let caps = CapsHint {
            document_symbol: true,
            workspace_symbol: true,
            call_hierarchy: false,
            references: false,
            definition: false,
        };
        let engine = make_engine_with_auto_policy(
            EngineKind::Lsp,
            EngineConfig {
                lsp_strict: true,
                dump_capabilities: false,
                mock_lsp: true,
                mock_caps: Some(caps),
            },
            None,
        );
        let opts = ImpactOptions {
            direction: ImpactDirection::Callers,
            max_depth: Some(4),
            with_edges: Some(false),
            ignore_dirs: Vec::new(),
        };

        let cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(&repo).unwrap();
        let err =
            validate_selected_engine_for_pdg_diff(&*engine, &files, LanguageMode::Rust, &opts)
                .expect_err("strict LSP without impact caps should fail for PDG diff validation");
        std::env::set_current_dir(cwd).unwrap();

        let msg = err.to_string();
        assert!(msg.contains("impact capability missing"));
        assert!(msg.contains("direction=Callers"));
    }

    #[test]
    #[serial]
    fn pdg_diff_validation_baseline_matches_selected_engine_when_caps_exist() {
        let (_tmp, repo, files) = setup_pdg_engine_repo();
        let lsp_engine = make_engine_with_auto_policy(
            EngineKind::Lsp,
            EngineConfig {
                lsp_strict: true,
                dump_capabilities: false,
                mock_lsp: true,
                mock_caps: Some(CapsHint {
                    document_symbol: true,
                    workspace_symbol: true,
                    call_hierarchy: true,
                    references: false,
                    definition: false,
                }),
            },
            None,
        );
        let ts_engine = make_engine_with_auto_policy(
            EngineKind::Ts,
            EngineConfig {
                lsp_strict: false,
                dump_capabilities: false,
                mock_lsp: false,
                mock_caps: None,
            },
            None,
        );
        let opts = ImpactOptions {
            direction: ImpactDirection::Callers,
            max_depth: Some(4),
            with_edges: Some(false),
            ignore_dirs: Vec::new(),
        };

        let cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(&repo).unwrap();
        validate_selected_engine_for_pdg_diff(&*lsp_engine, &files, LanguageMode::Rust, &opts)
            .expect("mock LSP with impact caps should pass PDG diff validation");
        validate_selected_engine_for_pdg_diff(&*ts_engine, &files, LanguageMode::Rust, &opts)
            .expect("TS engine should pass the same PDG diff validation baseline");
        std::env::set_current_dir(cwd).unwrap();
    }

    #[test]
    fn merge_pdg_references_preserves_kind_certainty_and_provenance() {
        let combined = DataFlowGraph {
            nodes: vec![
                dimpact::dfg::DfgNode {
                    id: "f.rs:def:x:10".to_string(),
                    name: "x".to_string(),
                    file: "f.rs".to_string(),
                    line: 10,
                },
                dimpact::dfg::DfgNode {
                    id: "f.rs:use:x:10".to_string(),
                    name: "x".to_string(),
                    file: "f.rs".to_string(),
                    line: 10,
                },
                dimpact::dfg::DfgNode {
                    id: "f.rs:def:y:10".to_string(),
                    name: "y".to_string(),
                    file: "f.rs".to_string(),
                    line: 10,
                },
                dimpact::dfg::DfgNode {
                    id: "f.rs:ctrl:9:10".to_string(),
                    name: "control".to_string(),
                    file: "f.rs".to_string(),
                    line: 9,
                },
            ],
            edges: vec![
                dimpact::dfg::DfgEdge {
                    from: "f.rs:def:x:10".to_string(),
                    to: "f.rs:use:x:10".to_string(),
                    kind: DependencyKind::Data,
                },
                dimpact::dfg::DfgEdge {
                    from: "f.rs:ctrl:9:10".to_string(),
                    to: "f.rs:def:y:10".to_string(),
                    kind: DependencyKind::Control,
                },
            ],
        };
        let refs = vec![Reference {
            from: SymbolId("rust:f.rs:fn:caller:9".to_string()),
            to: SymbolId("rust:f.rs:fn:callee:1".to_string()),
            kind: RefKind::Call,
            file: "f.rs".to_string(),
            line: 10,
            certainty: EdgeCertainty::Confirmed,
            provenance: EdgeProvenance::CallGraph,
        }];
        let mut pdg = combined.clone();
        pdg.edges.push(dimpact::dfg::DfgEdge {
            from: refs[0].from.0.clone(),
            to: refs[0].to.0.clone(),
            kind: DependencyKind::Data,
        });
        pdg.edges.push(dimpact::dfg::DfgEdge {
            from: "f.rs:use:x:10".to_string(),
            to: "f.rs:def:y:10".to_string(),
            kind: DependencyKind::Data,
        });

        let merged = merge_pdg_references(&combined, &pdg, &refs);

        assert!(merged.iter().any(|r| {
            r.kind == RefKind::Call
                && r.provenance == EdgeProvenance::CallGraph
                && r.certainty == EdgeCertainty::Confirmed
                && r.file == "f.rs"
                && r.line == 10
        }));
        assert!(merged.iter().any(|r| {
            r.kind == RefKind::Data
                && r.provenance == EdgeProvenance::LocalDfg
                && r.certainty == EdgeCertainty::Inferred
                && r.from.0 == "f.rs:def:x:10"
                && r.to.0 == "f.rs:use:x:10"
        }));
        assert!(merged.iter().any(|r| {
            r.kind == RefKind::Control
                && r.provenance == EdgeProvenance::LocalDfg
                && r.certainty == EdgeCertainty::Inferred
                && r.from.0 == "f.rs:ctrl:9:10"
                && r.to.0 == "f.rs:def:y:10"
        }));
        assert!(merged.iter().any(|r| {
            r.kind == RefKind::Data
                && r.provenance == EdgeProvenance::SymbolicPropagation
                && r.certainty == EdgeCertainty::Confirmed
                && r.file == "f.rs"
                && r.line == 10
                && r.from.0 == "f.rs:use:x:10"
                && r.to.0 == "f.rs:def:y:10"
        }));
    }

    fn test_symbol(id: &str, name: &str, file: &str, line: u32) -> dimpact::Symbol {
        dimpact::Symbol {
            id: SymbolId(id.to_string()),
            name: name.to_string(),
            kind: dimpact::SymbolKind::Function,
            file: file.to_string(),
            range: dimpact::TextRange {
                start_line: line,
                end_line: line + 2,
            },
            language: if file.ends_with(".rb") {
                "ruby".to_string()
            } else {
                "rust".to_string()
            },
        }
    }

    fn call_ref(from: &str, to: &str, file: &str, line: u32) -> Reference {
        Reference {
            from: SymbolId(from.to_string()),
            to: SymbolId(to.to_string()),
            kind: RefKind::Call,
            file: file.to_string(),
            line,
            certainty: EdgeCertainty::Confirmed,
            provenance: EdgeProvenance::CallGraph,
        }
    }

    fn slice_selection_file<'a>(
        summary: &'a ImpactSliceSelectionSummary,
        path: &str,
    ) -> &'a ImpactSliceFileMetadata {
        summary
            .files
            .iter()
            .find(|file| file.path == path)
            .unwrap_or_else(|| panic!("missing slice selection metadata for {path}: {summary:#?}"))
    }

    fn test_tier2_scoring(
        lane: ImpactSliceCandidateLane,
        primary_evidence_kinds: Vec<ImpactSliceEvidenceKind>,
        secondary_evidence_kinds: Vec<ImpactSliceEvidenceKind>,
        call_position_rank: u32,
        lexical_tiebreak: &str,
    ) -> ImpactSliceCandidateScoringSummary {
        ImpactSliceCandidateScoringSummary {
            source_kind: ImpactSliceCandidateSourceKind::GraphSecondHop,
            lane,
            score_tuple: ImpactSliceScoreTuple {
                source_rank: 0,
                lane_rank: tier2_lane_rank(lane),
                primary_evidence_count: u8::try_from(primary_evidence_kinds.len()).unwrap(),
                secondary_evidence_count: u8::try_from(secondary_evidence_kinds.len()).unwrap(),
                call_position_rank,
                lexical_tiebreak: lexical_tiebreak.to_string(),
            },
            primary_evidence_kinds,
            secondary_evidence_kinds,
            support: None,
        }
    }

    #[test]
    fn collect_rust_tier2_semantic_evidence_detects_param_to_return_flow() {
        let dir = TempDir::new().expect("tempdir");
        let file = dir.path().join("step.rs");
        fs::write(
            &file,
            "pub fn step(input: i32) -> i32 {\n    let forwarded = input;\n    forwarded\n}\n",
        )
        .unwrap();
        let file_str = file.to_string_lossy().to_string();
        let symbol = dimpact::Symbol {
            id: SymbolId::new("rust", &file_str, &dimpact::SymbolKind::Function, "step", 1),
            name: "step".to_string(),
            kind: dimpact::SymbolKind::Function,
            file: file_str,
            range: dimpact::TextRange {
                start_line: 1,
                end_line: 4,
            },
            language: "rust".to_string(),
        };

        let evidence = collect_rust_tier2_semantic_evidence(&symbol);
        assert!(evidence.param_to_return_flow);
    }

    #[test]
    fn bounded_slice_plan_selects_direct_boundary_and_single_bridge_completion() {
        let seed = test_symbol("rust:main.rs:fn:caller:1", "caller", "main.rs", 1);
        let wrapper = test_symbol("rust:wrapper.rs:fn:wrap:1", "wrap", "wrapper.rs", 1);
        let leaf = test_symbol("rust:leaf.rs:fn:source:1", "source", "leaf.rs", 1);
        let sibling = test_symbol("rust:side.rs:fn:side:1", "side", "side.rs", 1);
        let index = SymbolIndex::build(vec![
            seed.clone(),
            wrapper.clone(),
            leaf.clone(),
            sibling.clone(),
        ]);
        let refs = vec![
            call_ref(seed.id.0.as_str(), wrapper.id.0.as_str(), "main.rs", 4),
            call_ref(
                wrapper.id.0.as_str(),
                sibling.id.0.as_str(),
                "wrapper.rs",
                3,
            ),
            call_ref(wrapper.id.0.as_str(), leaf.id.0.as_str(), "wrapper.rs", 5),
        ];

        let plan = plan_bounded_slice(
            &[seed.file.clone()],
            &[seed.file.clone()],
            std::slice::from_ref(&seed),
            &index,
            &refs,
            ImpactDirection::Callees,
            ImpactSliceReasonKind::SeedFile,
        );

        assert_eq!(
            plan.cache_update_paths,
            vec![
                "leaf.rs".to_string(),
                "main.rs".to_string(),
                "wrapper.rs".to_string(),
            ]
        );
        assert_eq!(plan.local_dfg_paths, plan.cache_update_paths);
    }

    #[test]
    fn bounded_slice_plan_keeps_bridge_completion_per_boundary_side() {
        let seed = test_symbol("rust:main.rs:fn:caller:1", "caller", "main.rs", 1);
        let left_wrapper = test_symbol(
            "rust:left_wrapper.rs:fn:wrap_left:3",
            "wrap_left",
            "left_wrapper.rs",
            3,
        );
        let right_wrapper = test_symbol(
            "rust:right_wrapper.rs:fn:wrap_right:3",
            "wrap_right",
            "right_wrapper.rs",
            3,
        );
        let left_leaf = test_symbol(
            "rust:left_leaf.rs:fn:source_left:1",
            "source_left",
            "left_leaf.rs",
            1,
        );
        let right_leaf = test_symbol(
            "rust:right_leaf.rs:fn:source_right:1",
            "source_right",
            "right_leaf.rs",
            1,
        );
        let index = SymbolIndex::build(vec![
            seed.clone(),
            left_wrapper.clone(),
            right_wrapper.clone(),
            left_leaf.clone(),
            right_leaf.clone(),
        ]);
        let refs = vec![
            call_ref(seed.id.0.as_str(), left_wrapper.id.0.as_str(), "main.rs", 4),
            call_ref(
                seed.id.0.as_str(),
                right_wrapper.id.0.as_str(),
                "main.rs",
                5,
            ),
            call_ref(
                left_wrapper.id.0.as_str(),
                left_leaf.id.0.as_str(),
                "left_wrapper.rs",
                4,
            ),
            call_ref(
                right_wrapper.id.0.as_str(),
                right_leaf.id.0.as_str(),
                "right_wrapper.rs",
                4,
            ),
        ];

        let plan = plan_bounded_slice(
            &[seed.file.clone()],
            &[seed.file.clone()],
            std::slice::from_ref(&seed),
            &index,
            &refs,
            ImpactDirection::Callees,
            ImpactSliceReasonKind::SeedFile,
        );

        assert_eq!(
            plan.cache_update_paths,
            vec![
                "left_leaf.rs".to_string(),
                "left_wrapper.rs".to_string(),
                "main.rs".to_string(),
                "right_leaf.rs".to_string(),
                "right_wrapper.rs".to_string(),
            ]
        );
        assert_eq!(plan.local_dfg_paths, plan.cache_update_paths);
        assert!(
            plan.slice_selection.pruned_candidates.is_empty(),
            "unexpected pruned candidates: {:?}",
            plan.slice_selection.pruned_candidates
        );

        let left_leaf = slice_selection_file(&plan.slice_selection, "left_leaf.rs");
        assert_eq!(
            left_leaf.reasons,
            vec![ImpactSliceReasonMetadata {
                seed_symbol_id: seed.id.0.clone(),
                tier: 2,
                kind: ImpactSliceReasonKind::BridgeCompletionFile,
                via_symbol_id: Some("rust:left_wrapper.rs:fn:wrap_left:3".to_string()),
                via_path: Some("left_wrapper.rs".to_string()),
                bridge_kind: Some(ImpactSliceBridgeKind::WrapperReturn),
                scoring: Some(test_tier2_scoring(
                    ImpactSliceCandidateLane::ReturnContinuation,
                    vec![
                        ImpactSliceEvidenceKind::AssignedResult,
                        ImpactSliceEvidenceKind::ReturnFlow,
                    ],
                    vec![
                        ImpactSliceEvidenceKind::CallsitePositionHint,
                        ImpactSliceEvidenceKind::NamePathHint,
                    ],
                    4,
                    "left_leaf.rs",
                )),
            }]
        );

        let right_leaf = slice_selection_file(&plan.slice_selection, "right_leaf.rs");
        assert_eq!(
            right_leaf.reasons,
            vec![ImpactSliceReasonMetadata {
                seed_symbol_id: seed.id.0.clone(),
                tier: 2,
                kind: ImpactSliceReasonKind::BridgeCompletionFile,
                via_symbol_id: Some("rust:right_wrapper.rs:fn:wrap_right:3".to_string()),
                via_path: Some("right_wrapper.rs".to_string()),
                bridge_kind: Some(ImpactSliceBridgeKind::WrapperReturn),
                scoring: Some(test_tier2_scoring(
                    ImpactSliceCandidateLane::ReturnContinuation,
                    vec![
                        ImpactSliceEvidenceKind::AssignedResult,
                        ImpactSliceEvidenceKind::ReturnFlow,
                    ],
                    vec![
                        ImpactSliceEvidenceKind::CallsitePositionHint,
                        ImpactSliceEvidenceKind::NamePathHint,
                    ],
                    4,
                    "right_leaf.rs",
                )),
            }]
        );
    }

    #[test]
    fn bounded_slice_plan_prefers_later_wrapper_return_call_over_lexically_earlier_noise() {
        let seed = test_symbol("rust:main.rs:fn:caller:1", "caller", "main.rs", 1);
        let wrapper = test_symbol("rust:wrapper.rs:fn:wrap:3", "wrap", "wrapper.rs", 3);
        let helper = test_symbol("rust:aaa_helper.rs:fn:noise:1", "noise", "aaa_helper.rs", 1);
        let leaf = test_symbol("rust:leaf.rs:fn:source:1", "source", "leaf.rs", 1);
        let index = SymbolIndex::build(vec![
            seed.clone(),
            wrapper.clone(),
            helper.clone(),
            leaf.clone(),
        ]);
        let refs = vec![
            call_ref(seed.id.0.as_str(), wrapper.id.0.as_str(), "main.rs", 5),
            call_ref(wrapper.id.0.as_str(), helper.id.0.as_str(), "wrapper.rs", 4),
            call_ref(wrapper.id.0.as_str(), leaf.id.0.as_str(), "wrapper.rs", 5),
        ];

        let plan = plan_bounded_slice(
            &[seed.file.clone()],
            &[seed.file.clone()],
            std::slice::from_ref(&seed),
            &index,
            &refs,
            ImpactDirection::Callees,
            ImpactSliceReasonKind::SeedFile,
        );

        assert_eq!(
            plan.cache_update_paths,
            vec![
                "leaf.rs".to_string(),
                "main.rs".to_string(),
                "wrapper.rs".to_string(),
            ]
        );
        assert!(
            plan.slice_selection
                .pruned_candidates
                .iter()
                .any(|candidate| {
                    candidate.path == "aaa_helper.rs"
                        && candidate.prune_reason == ImpactSlicePruneReason::RankedOut
                        && candidate.bridge_kind
                            == Some(ImpactSliceBridgeKind::BoundaryAliasContinuation)
                        && candidate.via_symbol_id.as_deref() == Some("rust:wrapper.rs:fn:wrap:3")
                        && candidate.scoring.as_ref().is_some_and(|scoring| {
                            scoring.lane == ImpactSliceCandidateLane::AliasContinuation
                                && scoring.primary_evidence_kinds
                                    == vec![ImpactSliceEvidenceKind::AssignedResult]
                                && scoring.secondary_evidence_kinds
                                    == vec![ImpactSliceEvidenceKind::NamePathHint]
                        })
                })
        );
    }

    #[test]
    fn bounded_slice_plan_prefers_alias_continuation_over_later_adapter_helper_noise() {
        let seed = test_symbol("rust:main.rs:fn:caller:1", "caller", "main.rs", 1);
        let adapter = test_symbol("rust:adapter.rs:fn:wrap:3", "wrap", "adapter.rs", 3);
        let value = test_symbol("rust:value.rs:fn:make:1", "make", "value.rs", 1);
        let helper = test_symbol("rust:zzz_helper.rs:fn:noise:1", "noise", "zzz_helper.rs", 1);
        let index = SymbolIndex::build(vec![
            seed.clone(),
            adapter.clone(),
            value.clone(),
            helper.clone(),
        ]);
        let refs = vec![
            call_ref(seed.id.0.as_str(), adapter.id.0.as_str(), "main.rs", 5),
            call_ref(adapter.id.0.as_str(), value.id.0.as_str(), "adapter.rs", 4),
            call_ref(adapter.id.0.as_str(), helper.id.0.as_str(), "adapter.rs", 5),
        ];

        let plan = plan_bounded_slice(
            &[seed.file.clone()],
            &[seed.file.clone()],
            std::slice::from_ref(&seed),
            &index,
            &refs,
            ImpactDirection::Callees,
            ImpactSliceReasonKind::SeedFile,
        );

        assert_eq!(
            plan.cache_update_paths,
            vec![
                "adapter.rs".to_string(),
                "main.rs".to_string(),
                "value.rs".to_string(),
            ]
        );

        let value_file = slice_selection_file(&plan.slice_selection, "value.rs");
        assert_eq!(
            value_file.reasons,
            vec![ImpactSliceReasonMetadata {
                seed_symbol_id: seed.id.0.clone(),
                tier: 2,
                kind: ImpactSliceReasonKind::BridgeCompletionFile,
                via_symbol_id: Some("rust:adapter.rs:fn:wrap:3".to_string()),
                via_path: Some("adapter.rs".to_string()),
                bridge_kind: Some(ImpactSliceBridgeKind::BoundaryAliasContinuation),
                scoring: Some(test_tier2_scoring(
                    ImpactSliceCandidateLane::AliasContinuation,
                    vec![
                        ImpactSliceEvidenceKind::AliasChain,
                        ImpactSliceEvidenceKind::AssignedResult,
                    ],
                    vec![ImpactSliceEvidenceKind::NamePathHint],
                    4,
                    "value.rs",
                )),
            }]
        );
        assert!(
            plan.slice_selection
                .pruned_candidates
                .iter()
                .any(|candidate| {
                    candidate.path == "zzz_helper.rs"
                        && candidate.prune_reason == ImpactSlicePruneReason::RankedOut
                        && candidate.bridge_kind
                            == Some(ImpactSliceBridgeKind::BoundaryAliasContinuation)
                        && candidate.via_symbol_id.as_deref() == Some("rust:adapter.rs:fn:wrap:3")
                        && candidate.scoring.as_ref().is_some_and(|scoring| {
                            scoring.lane == ImpactSliceCandidateLane::AliasContinuation
                                && scoring.primary_evidence_kinds
                                    == vec![ImpactSliceEvidenceKind::AssignedResult]
                                && scoring.secondary_evidence_kinds
                                    == vec![
                                        ImpactSliceEvidenceKind::CallsitePositionHint,
                                        ImpactSliceEvidenceKind::NamePathHint,
                                    ]
                                && scoring.score_tuple.call_position_rank == 5
                        })
                }),
            "expected later helper noise to lose to the stronger alias continuation candidate: {:#?}",
            plan.slice_selection.pruned_candidates
        );
    }

    #[test]
    fn bounded_slice_plan_prefers_rust_param_passthrough_over_later_neutral_helper() {
        let dir = TempDir::new().expect("tempdir");
        let root = dir.path();
        fs::write(
            root.join("step.rs"),
            "pub fn step(input: i32) -> i32 {\n    let forwarded = input;\n    forwarded\n}\n",
        )
        .unwrap();
        fs::write(
            root.join("later.rs"),
            "pub fn later(drop: i32) -> i32 {\n    let shadow = drop;\n    41\n}\n",
        )
        .unwrap();
        fs::write(
            root.join("wrapper.rs"),
            "use crate::later;\nuse crate::step;\n\npub fn wrap(a: i32) -> i32 {\n    let keep = step::step(a);\n    let _side = later::later(a);\n    keep\n}\n",
        )
        .unwrap();
        fs::write(
            root.join("main.rs"),
            "mod later;\nmod step;\nmod wrapper;\n\nfn caller() {\n    let input = 7;\n    let out = wrapper::wrap(input);\n    println!(\"{}\", out);\n}\n",
        )
        .unwrap();

        let seed = dimpact::Symbol {
            id: SymbolId("rust:main.rs:fn:caller:5".to_string()),
            name: "caller".to_string(),
            kind: dimpact::SymbolKind::Function,
            file: "main.rs".to_string(),
            range: dimpact::TextRange {
                start_line: 5,
                end_line: 9,
            },
            language: "rust".to_string(),
        };
        let wrapper = dimpact::Symbol {
            id: SymbolId("rust:wrapper.rs:fn:wrap:4".to_string()),
            name: "wrap".to_string(),
            kind: dimpact::SymbolKind::Function,
            file: "wrapper.rs".to_string(),
            range: dimpact::TextRange {
                start_line: 4,
                end_line: 8,
            },
            language: "rust".to_string(),
        };
        let step = dimpact::Symbol {
            id: SymbolId("rust:step.rs:fn:step:1".to_string()),
            name: "step".to_string(),
            kind: dimpact::SymbolKind::Function,
            file: "step.rs".to_string(),
            range: dimpact::TextRange {
                start_line: 1,
                end_line: 4,
            },
            language: "rust".to_string(),
        };
        let later = dimpact::Symbol {
            id: SymbolId("rust:later.rs:fn:later:1".to_string()),
            name: "later".to_string(),
            kind: dimpact::SymbolKind::Function,
            file: "later.rs".to_string(),
            range: dimpact::TextRange {
                start_line: 1,
                end_line: 4,
            },
            language: "rust".to_string(),
        };
        let index = SymbolIndex::build(vec![
            seed.clone(),
            wrapper.clone(),
            step.clone(),
            later.clone(),
        ]);
        let refs = vec![
            call_ref(seed.id.0.as_str(), wrapper.id.0.as_str(), "main.rs", 7),
            call_ref(wrapper.id.0.as_str(), step.id.0.as_str(), "wrapper.rs", 5),
            call_ref(wrapper.id.0.as_str(), later.id.0.as_str(), "wrapper.rs", 6),
        ];

        let cwd = std::env::current_dir().expect("cwd");
        std::env::set_current_dir(root).expect("chdir temp repo");
        let plan = plan_bounded_slice(
            &[seed.file.clone()],
            &[seed.file.clone()],
            std::slice::from_ref(&seed),
            &index,
            &refs,
            ImpactDirection::Callees,
            ImpactSliceReasonKind::SeedFile,
        );
        std::env::set_current_dir(cwd).expect("restore cwd");

        let step_file = slice_selection_file(&plan.slice_selection, "step.rs");
        assert_eq!(
            step_file.reasons,
            vec![ImpactSliceReasonMetadata {
                seed_symbol_id: seed.id.0.clone(),
                tier: 2,
                kind: ImpactSliceReasonKind::BridgeCompletionFile,
                via_symbol_id: Some("rust:wrapper.rs:fn:wrap:4".to_string()),
                via_path: Some("wrapper.rs".to_string()),
                bridge_kind: Some(ImpactSliceBridgeKind::WrapperReturn),
                scoring: Some(ImpactSliceCandidateScoringSummary {
                    source_kind: ImpactSliceCandidateSourceKind::GraphSecondHop,
                    lane: ImpactSliceCandidateLane::ReturnContinuation,
                    primary_evidence_kinds: vec![
                        ImpactSliceEvidenceKind::AssignedResult,
                        ImpactSliceEvidenceKind::ParamToReturnFlow,
                        ImpactSliceEvidenceKind::ReturnFlow,
                    ],
                    secondary_evidence_kinds: vec![ImpactSliceEvidenceKind::NamePathHint],
                    score_tuple: ImpactSliceScoreTuple {
                        source_rank: 0,
                        lane_rank: 0,
                        primary_evidence_count: 3,
                        secondary_evidence_count: 1,
                        call_position_rank: 5,
                        lexical_tiebreak: "step.rs".to_string(),
                    },
                    support: Some(ImpactSliceCandidateSupportMetadata {
                        local_dfg_support: true,
                        ..ImpactSliceCandidateSupportMetadata::default()
                    }),
                }),
            }]
        );
        assert!(
            plan.slice_selection
                .pruned_candidates
                .iter()
                .any(|candidate| {
                    candidate.path == "later.rs"
                        && candidate.prune_reason == ImpactSlicePruneReason::RankedOut
                        && candidate.bridge_kind == Some(ImpactSliceBridgeKind::WrapperReturn)
                        && candidate.via_symbol_id.as_deref() == Some("rust:wrapper.rs:fn:wrap:4")
                        && candidate.scoring.as_ref().is_some_and(|scoring| {
                            scoring.lane == ImpactSliceCandidateLane::ReturnContinuation
                                && scoring.primary_evidence_kinds
                                    == vec![
                                        ImpactSliceEvidenceKind::AssignedResult,
                                        ImpactSliceEvidenceKind::ReturnFlow,
                                    ]
                                && scoring.secondary_evidence_kinds
                                    == vec![
                                        ImpactSliceEvidenceKind::CallsitePositionHint,
                                        ImpactSliceEvidenceKind::NamePathHint,
                                    ]
                        })
                }),
            "expected later neutral helper to be ranked out once param-to-return evidence is available: {:#?}",
            plan.slice_selection.pruned_candidates
        );
    }

    #[test]
    fn bounded_slice_plan_prefers_ruby_return_completion_over_later_require_relative_helper_noise()
    {
        let seed = test_symbol(
            "ruby:app/runner.rb:method:entry:3",
            "entry",
            "app/runner.rb",
            3,
        );
        let service = test_symbol(
            "ruby:lib/service.rb:method:bounce:4",
            "bounce",
            "lib/service.rb",
            4,
        );
        let leaf = test_symbol(
            "ruby:lib/leaf.rb:method:finish:1",
            "finish",
            "lib/leaf.rb",
            1,
        );
        let helper = test_symbol(
            "ruby:lib/zzz_helper.rb:method:helper_noise:1",
            "helper_noise",
            "lib/zzz_helper.rb",
            1,
        );
        let index = SymbolIndex::build(vec![
            seed.clone(),
            service.clone(),
            leaf.clone(),
            helper.clone(),
        ]);
        let refs = vec![
            call_ref(
                seed.id.0.as_str(),
                service.id.0.as_str(),
                "app/runner.rb",
                5,
            ),
            call_ref(
                service.id.0.as_str(),
                leaf.id.0.as_str(),
                "lib/service.rb",
                6,
            ),
            call_ref(
                service.id.0.as_str(),
                helper.id.0.as_str(),
                "lib/service.rb",
                7,
            ),
        ];

        let plan = plan_bounded_slice(
            &[seed.file.clone()],
            &[seed.file.clone()],
            std::slice::from_ref(&seed),
            &index,
            &refs,
            ImpactDirection::Callees,
            ImpactSliceReasonKind::SeedFile,
        );

        assert_eq!(
            plan.cache_update_paths,
            vec![
                "app/runner.rb".to_string(),
                "lib/leaf.rb".to_string(),
                "lib/service.rb".to_string(),
            ]
        );

        let leaf_file = slice_selection_file(&plan.slice_selection, "lib/leaf.rb");
        assert_eq!(
            leaf_file.reasons,
            vec![ImpactSliceReasonMetadata {
                seed_symbol_id: seed.id.0.clone(),
                tier: 2,
                kind: ImpactSliceReasonKind::BridgeCompletionFile,
                via_symbol_id: Some("ruby:lib/service.rb:method:bounce:4".to_string()),
                via_path: Some("lib/service.rb".to_string()),
                bridge_kind: Some(ImpactSliceBridgeKind::WrapperReturn),
                scoring: Some(test_tier2_scoring(
                    ImpactSliceCandidateLane::ReturnContinuation,
                    vec![
                        ImpactSliceEvidenceKind::AssignedResult,
                        ImpactSliceEvidenceKind::ReturnFlow,
                    ],
                    vec![ImpactSliceEvidenceKind::NamePathHint],
                    6,
                    "lib/leaf.rb",
                )),
            }]
        );
        assert_eq!(
            plan.slice_selection.pruned_candidates,
            vec![dimpact::ImpactSlicePrunedCandidate {
                seed_symbol_id: seed.id.0.clone(),
                path: "lib/zzz_helper.rb".to_string(),
                tier: 2,
                kind: ImpactSliceReasonKind::BridgeCompletionFile,
                via_symbol_id: Some("ruby:lib/service.rb:method:bounce:4".to_string()),
                via_path: Some("lib/service.rb".to_string()),
                bridge_kind: Some(ImpactSliceBridgeKind::RequireRelativeChain),
                prune_reason: ImpactSlicePruneReason::RankedOut,
                scoring: Some(test_tier2_scoring(
                    ImpactSliceCandidateLane::RequireRelativeContinuation,
                    vec![ImpactSliceEvidenceKind::RequireRelativeEdge],
                    vec![ImpactSliceEvidenceKind::CallsitePositionHint],
                    7,
                    "lib/zzz_helper.rb",
                )),
            }],
            "expected later Ruby helper noise to stay a single ranked-out require_relative fallback: {:#?}",
            plan.slice_selection.pruned_candidates
        );
    }

    #[test]
    fn bounded_slice_plan_records_ranked_out_and_budget_pruned_tier2_candidates() {
        let seed = test_symbol("rust:main.rs:fn:caller:1", "caller", "main.rs", 1);
        let wrap_a = test_symbol("rust:a_wrapper.rs:fn:wrap_a:3", "wrap_a", "a_wrapper.rs", 3);
        let wrap_b = test_symbol("rust:b_wrapper.rs:fn:wrap_b:3", "wrap_b", "b_wrapper.rs", 3);
        let wrap_c = test_symbol("rust:c_wrapper.rs:fn:wrap_c:3", "wrap_c", "c_wrapper.rs", 3);
        let leaf_a = test_symbol("rust:a_leaf.rs:fn:leaf_a:1", "leaf_a", "a_leaf.rs", 1);
        let alt_a = test_symbol("rust:z_alt.rs:fn:alt_a:1", "alt_a", "z_alt.rs", 1);
        let leaf_b = test_symbol("rust:b_leaf.rs:fn:leaf_b:1", "leaf_b", "b_leaf.rs", 1);
        let leaf_c = test_symbol("rust:c_leaf.rs:fn:leaf_c:1", "leaf_c", "c_leaf.rs", 1);
        let index = SymbolIndex::build(vec![
            seed.clone(),
            wrap_a.clone(),
            wrap_b.clone(),
            wrap_c.clone(),
            leaf_a.clone(),
            alt_a.clone(),
            leaf_b.clone(),
            leaf_c.clone(),
        ]);
        let refs = vec![
            call_ref(seed.id.0.as_str(), wrap_a.id.0.as_str(), "main.rs", 4),
            call_ref(seed.id.0.as_str(), wrap_b.id.0.as_str(), "main.rs", 5),
            call_ref(seed.id.0.as_str(), wrap_c.id.0.as_str(), "main.rs", 6),
            call_ref(wrap_a.id.0.as_str(), alt_a.id.0.as_str(), "a_wrapper.rs", 4),
            call_ref(
                wrap_a.id.0.as_str(),
                leaf_a.id.0.as_str(),
                "a_wrapper.rs",
                5,
            ),
            call_ref(
                wrap_b.id.0.as_str(),
                leaf_b.id.0.as_str(),
                "b_wrapper.rs",
                4,
            ),
            call_ref(
                wrap_c.id.0.as_str(),
                leaf_c.id.0.as_str(),
                "c_wrapper.rs",
                4,
            ),
        ];

        let plan = plan_bounded_slice(
            &[seed.file.clone()],
            &[seed.file.clone()],
            std::slice::from_ref(&seed),
            &index,
            &refs,
            ImpactDirection::Callees,
            ImpactSliceReasonKind::SeedFile,
        );

        assert_eq!(
            plan.cache_update_paths,
            vec![
                "a_leaf.rs".to_string(),
                "a_wrapper.rs".to_string(),
                "b_leaf.rs".to_string(),
                "b_wrapper.rs".to_string(),
                "c_wrapper.rs".to_string(),
                "main.rs".to_string(),
            ]
        );
        assert!(
            plan.slice_selection
                .pruned_candidates
                .iter()
                .any(|candidate| {
                    candidate.path == "z_alt.rs"
                        && candidate.prune_reason == ImpactSlicePruneReason::RankedOut
                        && candidate.bridge_kind
                            == Some(ImpactSliceBridgeKind::BoundaryAliasContinuation)
                        && candidate.via_symbol_id.as_deref()
                            == Some("rust:a_wrapper.rs:fn:wrap_a:3")
                })
        );
        assert!(
            plan.slice_selection
                .pruned_candidates
                .iter()
                .any(|candidate| {
                    candidate.path == "c_leaf.rs"
                        && candidate.prune_reason == ImpactSlicePruneReason::BridgeBudgetExhausted
                        && candidate.bridge_kind == Some(ImpactSliceBridgeKind::WrapperReturn)
                        && candidate.via_symbol_id.as_deref()
                            == Some("rust:c_wrapper.rs:fn:wrap_c:3")
                })
        );
    }

    #[test]
    fn bounded_slice_plan_limits_local_dfg_scope_to_rust_and_ruby_files() {
        let seed = test_symbol("rust:main.rs:fn:caller:1", "caller", "main.rs", 1);
        let js = test_symbol("js:helper.js:fn:helper:1", "helper", "helper.js", 1);
        let index = SymbolIndex::build(vec![seed.clone(), js.clone()]);
        let refs = vec![call_ref(seed.id.0.as_str(), js.id.0.as_str(), "main.rs", 4)];

        let plan = plan_bounded_slice(
            &[seed.file.clone()],
            &[seed.file.clone()],
            std::slice::from_ref(&seed),
            &index,
            &refs,
            ImpactDirection::Callees,
            ImpactSliceReasonKind::SeedFile,
        );

        assert!(plan.cache_update_paths.contains(&"helper.js".to_string()));
        assert_eq!(plan.local_dfg_paths, vec!["main.rs".to_string()]);
    }

    #[test]
    fn bounded_slice_plan_keeps_initial_local_dfg_roots_out_of_explanation_scope_without_reasons() {
        let seed = test_symbol("rust:main.rs:fn:caller:1", "caller", "main.rs", 1);
        let callee = test_symbol("rust:callee.rs:fn:callee:1", "callee", "callee.rs", 1);
        let index = SymbolIndex::build(vec![seed.clone(), callee.clone()]);
        let refs = vec![call_ref(
            seed.id.0.as_str(),
            callee.id.0.as_str(),
            "main.rs",
            4,
        )];

        let plan = plan_bounded_slice(
            &[seed.file.clone(), "notes.rs".to_string()],
            &[seed.file.clone(), "notes.rs".to_string()],
            std::slice::from_ref(&seed),
            &index,
            &refs,
            ImpactDirection::Callees,
            ImpactSliceReasonKind::SeedFile,
        );

        let notes = slice_selection_file(&plan.slice_selection, "notes.rs");
        assert_eq!(
            notes.scopes,
            ImpactSliceScopes {
                cache_update: true,
                local_dfg: true,
                explanation: false,
            }
        );
        assert!(
            notes.reasons.is_empty(),
            "notes.rs should stay cache/local-DFG only"
        );

        let main = slice_selection_file(&plan.slice_selection, "main.rs");
        assert!(main.scopes.explanation);
        assert_eq!(
            main.reasons,
            vec![ImpactSliceReasonMetadata {
                seed_symbol_id: seed.id.0.clone(),
                tier: 0,
                kind: ImpactSliceReasonKind::SeedFile,
                via_symbol_id: None,
                via_path: None,
                bridge_kind: None,
                scoring: None,
            }]
        );
    }

    #[test]
    fn cli_op_profile_accepts_balanced_and_precision_first() {
        let a = Args::try_parse_from(["dimpact", "impact", "--op-profile", "balanced"])
            .expect("balanced profile should parse");
        match a.cmd {
            Some(Command::Impact { op_profile, .. }) => {
                assert!(matches!(op_profile, Some(OperationalProfileOpt::Balanced)))
            }
            _ => panic!("expected impact subcommand"),
        }

        let b = Args::try_parse_from(["dimpact", "impact", "--op-profile", "precision-first"])
            .expect("precision-first profile should parse");
        match b.cmd {
            Some(Command::Impact { op_profile, .. }) => assert!(matches!(
                op_profile,
                Some(OperationalProfileOpt::PrecisionFirst)
            )),
            _ => panic!("expected impact subcommand"),
        }
    }

    #[test]
    fn op_profile_defaults_can_be_overridden_explicitly() {
        let (min_a, excl_a) =
            resolve_operational_profile(Some(OperationalProfileOpt::Balanced), None, false);
        assert!(matches!(min_a, Some(ConfidenceOpt::Inferred)));
        assert!(!excl_a);

        let (min_b, excl_b) =
            resolve_operational_profile(Some(OperationalProfileOpt::PrecisionFirst), None, false);
        assert!(matches!(min_b, Some(ConfidenceOpt::Confirmed)));
        assert!(excl_b);

        let (min_c, excl_c) = resolve_operational_profile(
            Some(OperationalProfileOpt::PrecisionFirst),
            Some(ConfidenceOpt::DynamicFallback),
            false,
        );
        assert!(matches!(min_c, Some(ConfidenceOpt::DynamicFallback)));
        assert!(excl_c);
    }

    #[test]
    fn evidence_kind_key_sorts_new_g8_2_evidence_names_lexically() {
        let mut kinds = vec![
            ImpactSliceEvidenceKind::ParamToReturnFlow,
            ImpactSliceEvidenceKind::ExplicitRequireRelativeLoad,
            ImpactSliceEvidenceKind::CompanionFileMatch,
            ImpactSliceEvidenceKind::DynamicDispatchLiteralTarget,
        ];

        kinds.sort_by_key(|kind| evidence_kind_key(*kind));

        assert_eq!(
            kinds,
            vec![
                ImpactSliceEvidenceKind::CompanionFileMatch,
                ImpactSliceEvidenceKind::DynamicDispatchLiteralTarget,
                ImpactSliceEvidenceKind::ExplicitRequireRelativeLoad,
                ImpactSliceEvidenceKind::ParamToReturnFlow,
            ]
        );
    }
}
