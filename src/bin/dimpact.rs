use clap::{Parser, ValueEnum};
use dimpact::{parse_unified_diff, DiffParseError};
use dimpact::{compute_changed_symbols, ChangedOutput, LanguageMode};
use dimpact::{build_project_graph, compute_impact, ImpactDirection, ImpactOptions, ImpactOutput};
use dimpact::Engine;
use is_terminal::IsTerminal;
use std::io::{self, Read};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Json,
    Yaml,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Mode {
    Diff,
    Changed,
    Impact,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum LangOpt { Auto, Rust }

#[derive(Debug, Clone, Copy, ValueEnum)]
enum DirectionOpt { Callers, Callees, Both }

#[derive(Debug, Clone, Copy, ValueEnum)]
enum EngineOpt { Regex, Ts }

#[derive(Debug, Parser)]
#[command(name = "dimpact", version, about = "Analyze git diff and serialize changes")] 
struct Args {
    /// Output format (json or yaml)
    #[arg(short = 'f', long = "format", value_enum, default_value_t = OutputFormat::Json)]
    format: OutputFormat,

    /// Mode of operation: diff (raw), changed (symbols), impact (TBD)
    #[arg(long = "mode", value_enum, default_value_t = Mode::Diff)]
    mode: Mode,

    /// Language mode for symbol extraction (when mode=changed)
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

    /// Analysis engine: regex or ts (Tree-sitter). Default: ts (falls back to regex if ts feature is not built).
    #[arg(long = "engine", value_enum, default_value_t = EngineOpt::Ts)]
    engine: EngineOpt,

    /// Run A/B comparison between regex and ts engines (mode=changed only)
    #[arg(long = "ab-compare", default_value_t = false)]
    ab_compare: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let diff_text = read_diff_from_stdin()?;
    let files = match parse_unified_diff(&diff_text) {
        Ok(f) => f,
        Err(DiffParseError::MissingHeader) => {
            // Treat empty parse gracefully
            Vec::new()
        }
        Err(e) => return Err(anyhow::anyhow!(e)),
    };

    match args.mode {
        Mode::Diff => {
            match args.format {
                OutputFormat::Json => {
                    let out = serde_json::to_string_pretty(&files)?;
                    println!("{}", out);
                }
                OutputFormat::Yaml => {
                    let out = serde_yaml::to_string(&files)?;
                    print!("{}", out);
                }
            }
        }
        Mode::Changed => {
            if args.ab_compare {
                #[cfg(feature = "ts")]
                {
                    let lang = match args.lang { LangOpt::Auto => LanguageMode::Auto, LangOpt::Rust => LanguageMode::Rust };
                    let regex_report: ChangedOutput = compute_changed_symbols(&files, lang, Engine::Regex)?;
                    let ts_report: ChangedOutput = compute_changed_symbols(&files, lang, Engine::Ts)?;
                    let report = build_ab_report(&regex_report, &ts_report);
                    match args.format {
                        OutputFormat::Json => { println!("{}", serde_json::to_string_pretty(&report)?); }
                        OutputFormat::Yaml => { print!("{}", serde_yaml::to_string(&report)?); }
                    }
                    return Ok(());
                }
                #[cfg(not(feature = "ts"))]
                {
                    anyhow::bail!("--ab-compare requires building with 'ts' feature")
                }
            }
            let lang = match args.lang { LangOpt::Auto => LanguageMode::Auto, LangOpt::Rust => LanguageMode::Rust };
            let engine = match args.engine { EngineOpt::Regex => Engine::Regex, EngineOpt::Ts => Engine::Ts };
            if matches!(args.engine, EngineOpt::Ts) && !cfg!(feature = "ts") {
                eprintln!("warning: --engine ts requested but binary built without 'ts' feature; falling back to regex engine");
            }
            let report: ChangedOutput = compute_changed_symbols(&files, lang, engine)?;
            match args.format {
                OutputFormat::Json => {
                    let out = serde_json::to_string_pretty(&report)?;
                    println!("{}", out);
                }
                OutputFormat::Yaml => {
                    let out = serde_yaml::to_string(&report)?;
                    print!("{}", out);
                }
            }
        }
        Mode::Impact => {
            if args.ab_compare {
                #[cfg(feature = "ts")]
                {
                    let lang = match args.lang { LangOpt::Auto => LanguageMode::Auto, LangOpt::Rust => LanguageMode::Rust };
                    // compute changed with both engines
                    let changed_regex: ChangedOutput = compute_changed_symbols(&files, lang, Engine::Regex)?;
                    let changed_ts: ChangedOutput = compute_changed_symbols(&files, lang, Engine::Ts)?;
                    // build graphs and impact
                    let (index_r, refs_r) = build_project_graph(Engine::Regex)?;
                    let (index_t, refs_t) = build_project_graph(Engine::Ts)?;
                    let direction = match args.direction {
                        DirectionOpt::Callers => ImpactDirection::Callers,
                        DirectionOpt::Callees => ImpactDirection::Callees,
                        DirectionOpt::Both => ImpactDirection::Both,
                    };
                    let opts = ImpactOptions { direction, max_depth: args.max_depth.or(Some(100)), with_edges: Some(args.with_edges) };
                    let impact_r: ImpactOutput = compute_impact(&changed_regex.changed_symbols, &index_r, &refs_r, &opts);
                    let impact_t: ImpactOutput = compute_impact(&changed_ts.changed_symbols, &index_t, &refs_t, &opts);
                    let report = build_ab_impact_report(&impact_r, &impact_t);
                    match args.format {
                        OutputFormat::Json => { println!("{}", serde_json::to_string_pretty(&report)?); }
                        OutputFormat::Yaml => { print!("{}", serde_yaml::to_string(&report)?); }
                    }
                    return Ok(());
                }
                #[cfg(not(feature = "ts"))]
                {
                    anyhow::bail!("--ab-compare requires building with 'ts' feature")
                }
            }
            let lang = match args.lang { LangOpt::Auto => LanguageMode::Auto, LangOpt::Rust => LanguageMode::Rust };
            let engine = match args.engine { EngineOpt::Regex => Engine::Regex, EngineOpt::Ts => Engine::Ts };
            if matches!(args.engine, EngineOpt::Ts) && !cfg!(feature = "ts") {
                eprintln!("warning: --engine ts requested but binary built without 'ts' feature; falling back to regex engine");
            }
            let changed: ChangedOutput = compute_changed_symbols(&files, lang, engine)?;
            let (index, refs) = build_project_graph(engine)?;
            let direction = match args.direction {
                DirectionOpt::Callers => ImpactDirection::Callers,
                DirectionOpt::Callees => ImpactDirection::Callees,
                DirectionOpt::Both => ImpactDirection::Both,
            };
            let opts = ImpactOptions { direction, max_depth: args.max_depth.or(Some(100)), with_edges: Some(args.with_edges) };
            let out: ImpactOutput = compute_impact(&changed.changed_symbols, &index, &refs, &opts);
            match args.format {
                OutputFormat::Json => {
                    let s = serde_json::to_string_pretty(&out)?;
                    println!("{}", s);
                }
                OutputFormat::Yaml => {
                    let s = serde_yaml::to_string(&out)?;
                    print!("{}", s);
                }
            }
        }
    }

    Ok(())
}

#[derive(serde::Serialize)]
struct AbCompareReport<'a> {
    regex: &'a ChangedOutput,
    ts: &'a ChangedOutput,
    only_in_regex: Vec<String>,
    only_in_ts: Vec<String>,
    intersection: usize,
}

fn build_ab_report<'a>(regex: &'a ChangedOutput, ts: &'a ChangedOutput) -> AbCompareReport<'a> {
    use std::collections::HashSet;
    let set_r: HashSet<&str> = regex.changed_symbols.iter().map(|s| s.id.0.as_str()).collect();
    let set_t: HashSet<&str> = ts.changed_symbols.iter().map(|s| s.id.0.as_str()).collect();
    let only_r: Vec<String> = set_r.difference(&set_t).map(|s| s.to_string()).collect();
    let only_t: Vec<String> = set_t.difference(&set_r).map(|s| s.to_string()).collect();
    let inter = set_r.intersection(&set_t).count();
    AbCompareReport { regex, ts, only_in_regex: only_r, only_in_ts: only_t, intersection: inter }
}

#[derive(serde::Serialize)]
struct AbCompareImpactReport<'a> {
    regex: &'a ImpactOutput,
    ts: &'a ImpactOutput,
    only_in_regex_symbols: Vec<String>,
    only_in_ts_symbols: Vec<String>,
    intersection_symbols: usize,
}

fn build_ab_impact_report<'a>(regex: &'a ImpactOutput, ts: &'a ImpactOutput) -> AbCompareImpactReport<'a> {
    use std::collections::HashSet;
    let set_r: HashSet<&str> = regex.impacted_symbols.iter().map(|s| s.id.0.as_str()).collect();
    let set_t: HashSet<&str> = ts.impacted_symbols.iter().map(|s| s.id.0.as_str()).collect();
    let only_r: Vec<String> = set_r.difference(&set_t).map(|s| s.to_string()).collect();
    let only_t: Vec<String> = set_t.difference(&set_r).map(|s| s.to_string()).collect();
    let inter = set_r.intersection(&set_t).count();
    AbCompareImpactReport { regex, ts, only_in_regex_symbols: only_r, only_in_ts_symbols: only_t, intersection_symbols: inter }
}

fn read_diff_from_stdin() -> anyhow::Result<String> {
    if std::io::stdin().is_terminal() {
        anyhow::bail!("no stdin detected: please pipe `git diff` output into dimpact");
    }
    let mut s = String::new();
    io::stdin().read_to_string(&mut s)?;
    Ok(s)
}
