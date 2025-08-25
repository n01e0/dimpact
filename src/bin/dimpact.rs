use clap::{Parser, ValueEnum};
use dimpact::{parse_unified_diff, DiffParseError};
use dimpact::{compute_changed_symbols, ChangedOutput, LanguageMode};
use dimpact::{build_project_graph, compute_impact, ImpactDirection, ImpactOptions, ImpactOutput};
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
            let lang = match args.lang { LangOpt::Auto => LanguageMode::Auto, LangOpt::Rust => LanguageMode::Rust };
            let report: ChangedOutput = compute_changed_symbols(&files, lang)?;
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
            let lang = match args.lang { LangOpt::Auto => LanguageMode::Auto, LangOpt::Rust => LanguageMode::Rust };
            let changed: ChangedOutput = compute_changed_symbols(&files, lang)?;
            let (index, refs) = build_project_graph()?;
            let out: ImpactOutput = compute_impact(&changed.changed_symbols, &index, &refs, &ImpactOptions::default());
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

fn read_diff_from_stdin() -> anyhow::Result<String> {
    if std::io::stdin().is_terminal() {
        anyhow::bail!("no stdin detected: please pipe `git diff` output into dimpact");
    }
    let mut s = String::new();
    io::stdin().read_to_string(&mut s)?;
    Ok(s)
}
