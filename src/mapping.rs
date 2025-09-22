use crate::diff::{ChangeKind, FileChanges};
use crate::ir::{Symbol, TextRange};
use crate::languages::{LanguageKind, analyzer_for_path};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageMode {
    Auto,
    Rust,
    Ruby,
    Javascript,
    Typescript,
    Tsx,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChangedOutput {
    pub changed_files: Vec<String>,
    pub changed_symbols: Vec<Symbol>,
}

pub fn compute_changed_symbols(
    diffs: &[FileChanges],
    lang: LanguageMode,
) -> anyhow::Result<ChangedOutput> {
    // Include both new_path (added/modified) and old_path for deletions/renames,
    // so cache can mark removed files as present=0 when they no longer exist.
    let mut changed_files: Vec<String> = Vec::new();
    for fc in diffs {
        if let Some(p) = fc.new_path.clone() {
            changed_files.push(p);
        } else if let Some(op) = fc.old_path.clone() {
            changed_files.push(op);
        }
    }

    let mut changed_lines_by_file: HashMap<String, HashSet<u32>> = HashMap::new();
    for fc in diffs {
        if let Some(path) = &fc.new_path {
            let set = changed_lines_by_file.entry(path.clone()).or_default();
            for ch in &fc.changes {
                // count Added, Removed, and Context lines as changes
                if matches!(ch.kind, ChangeKind::Added)
                    || matches!(ch.kind, ChangeKind::Removed)
                    || matches!(ch.kind, ChangeKind::Context)
                {
                    // use new_line when available, else old_line for removals
                    if let Some(nl) = ch.new_line {
                        set.insert(nl);
                    } else if let Some(ol) = ch.old_line {
                        set.insert(ol);
                    }
                }
            }
        }
    }

    let mut changed_symbols = Vec::new();
    for (path, lines) in changed_lines_by_file.iter() {
        let kind = match lang {
            LanguageMode::Auto => LanguageKind::Auto,
            LanguageMode::Rust => LanguageKind::Rust,
            LanguageMode::Ruby => LanguageKind::Ruby,
            LanguageMode::Javascript => LanguageKind::Javascript,
            LanguageMode::Typescript => LanguageKind::Typescript,
            LanguageMode::Tsx => LanguageKind::Tsx,
        };
        let Some(analyzer) = analyzer_for_path(path, kind) else {
            continue;
        };
        let Ok(source) = fs::read_to_string(path) else {
            continue;
        };
        let symbols = analyzer.symbols_in_file(path, &source);
        for s in symbols {
            if intersects(&s.range, lines) {
                changed_symbols.push(s);
            }
        }
    }

    Ok(ChangedOutput {
        changed_files,
        changed_symbols,
    })
}

fn intersects(range: &TextRange, lines: &HashSet<u32>) -> bool {
    for ln in range.start_line..=range.end_line {
        if lines.contains(&ln) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::parse_unified_diff;
    use serial_test::serial;
    use tempfile::tempdir;

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
        let out = compute_changed_symbols(&parsed, LanguageMode::Rust).unwrap();
        std::env::set_current_dir(cwd).unwrap();

        assert!(out.changed_files.iter().any(|p| p.ends_with("main.rs")));
        assert!(out.changed_symbols.iter().any(|s| s.name == "foo"));
        assert!(!out.changed_symbols.iter().any(|s| s.name == "bar"));
    }
}
