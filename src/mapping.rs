use crate::diff::{ChangeKind, FileChanges};
use crate::ir::{Symbol, TextRange};
use crate::languages::{LanguageAnalyzer, Engine, rust_analyzer};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageMode {
    Auto,
    Rust,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChangedOutput {
    pub changed_files: Vec<String>,
    pub changed_symbols: Vec<Symbol>,
}

pub fn compute_changed_symbols(
    diffs: &[FileChanges],
    lang: LanguageMode,
    engine: Engine,
) -> anyhow::Result<ChangedOutput> {
    let changed_files: Vec<String> = diffs
        .iter()
        .filter_map(|fc| fc.new_path.clone())
        .collect();

    let mut changed_lines_by_file: HashMap<String, HashSet<u32>> = HashMap::new();
    for fc in diffs {
        if let Some(path) = &fc.new_path {
            let set = changed_lines_by_file.entry(path.clone()).or_default();
            for ch in &fc.changes {
                if let Some(nl) = ch.new_line {
                    // count both Added and Context lines; Context lines can help expand range near removals
                    // but primarily Added lines carry the change signal
                    if matches!(ch.kind, ChangeKind::Added) || matches!(ch.kind, ChangeKind::Context) {
                        set.insert(nl);
                    }
                }
            }
        }
    }

    let analyzer: Box<dyn LanguageAnalyzer> = match lang {
        LanguageMode::Auto | LanguageMode::Rust => rust_analyzer(engine),
    };

    let mut changed_symbols = Vec::new();
    for (path, lines) in changed_lines_by_file.iter() {
        // Skip non-rust when mode is Rust
        if let LanguageMode::Rust = lang
            && !path.ends_with(".rs") { continue; }
        let Ok(source) = fs::read_to_string(path) else { continue };
        let symbols = analyzer.symbols_in_file(path, &source);
        for s in symbols {
            if intersects(&s.range, lines) {
                changed_symbols.push(s);
            }
        }
    }

    Ok(ChangedOutput { changed_files, changed_symbols })
}

fn intersects(range: &TextRange, lines: &HashSet<u32>) -> bool {
    for ln in range.start_line..=range.end_line {
        if lines.contains(&ln) { return true; }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::{parse_unified_diff};
    use tempfile::tempdir;
    use serial_test::serial;
    

    #[test]
    #[serial]
    fn changed_symbols_basic() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("main.rs");
        let code = r#"fn foo() {
    println!("one");
}

fn bar() {}
"#;
        fs::write(&file, code).unwrap();

        // Construct a diff snippet that adds a line inside foo()
        let diff = "diff --git a/main.rs b/main.rs\n--- a/main.rs\n+++ b/main.rs\n@@ -1,3 +1,4 @@\n fn foo() {\n-    println!(\"one\");\n+    println!(\"one\");\n+    println!(\"two\");\n }\n".to_string();

        let parsed = parse_unified_diff(&diff).unwrap();
        // Compute with working dir file; change current dir temporarily
        let cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();
        let out = compute_changed_symbols(&parsed, LanguageMode::Rust, crate::languages::Engine::Regex).unwrap();
        std::env::set_current_dir(cwd).unwrap();

        assert!(out.changed_files.iter().any(|p| p.ends_with("main.rs")));
        assert!(out.changed_symbols.iter().any(|s| s.name == "foo"));
        assert!(!out.changed_symbols.iter().any(|s| s.name == "bar"));
    }
}
