//! Common path normalization utilities for language spec modules.
use std::path::{Component, Path};

/// Normalize a path-like value by collapsing '.' and '..' without touching the filesystem.
/// Converts backslashes to forward slashes.
pub fn normalize_path_like(p: &Path) -> String {
    let mut out = std::path::PathBuf::new();
    for comp in p.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out.to_string_lossy().replace('\\', "/")
}

/// Resolve a module specifier to a normalized path-like string.
/// Trims supported extensions, replaces backslashes, collapses '.' and '..'.
pub fn resolve_module_path(cur_file: &str, raw: &str, exts: &[&str]) -> Option<String> {
    let mut s = raw.trim().to_string();
    for &ext in exts {
        if s.ends_with(ext) {
            s.truncate(s.len() - ext.len());
        }
    }
    let s = s.replace('\\', "/");
    if s.starts_with("./") || s.starts_with("../") {
        let base = Path::new(cur_file)
            .parent()
            .unwrap_or_else(|| Path::new("."));
        let joined = base.join(&s);
        Some(normalize_path_like(&joined))
    } else {
        // bare specifier: remove leading slash if any
        Some(s.trim_start_matches('/').to_string())
    }
}
