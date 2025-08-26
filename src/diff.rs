use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DiffParseError {
    #[error("missing diff header")] 
    MissingHeader,
    #[error("invalid hunk header: {0}")]
    InvalidHunkHeader(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileChanges {
    pub old_path: Option<String>,
    pub new_path: Option<String>,
    pub changes: Vec<Change>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Change {
    pub kind: ChangeKind,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Added,
    Removed,
    Context,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HunkRange {
    old_start: u32,
    old_len: u32,
    new_start: u32,
    new_len: u32,
}

impl fmt::Display for HunkRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "-{},{} +{},{}",
            self.old_start, self.old_len, self.new_start, self.new_len
        )
    }
}

/// Parse a unified diff string into a vector of file-level changes.
///
/// This parser is intentionally minimal and supports the common subset:
/// - `diff --git a/.. b/..` headers (optional for parsing)
/// - `--- a/path` and `+++ b/path`
/// - Hunk headers like `@@ -l,s +l,s @@` (s optional)
/// - Line prefixes: `+` added, `-` removed, ` ` context
pub fn parse_unified_diff(input: &str) -> Result<Vec<FileChanges>, DiffParseError> {
    let mut files: Vec<FileChanges> = Vec::new();
    let mut lines = input.lines().peekable();

    let mut cur_old_path: Option<String> = None;
    let mut cur_new_path: Option<String> = None;
    let mut cur_changes: Vec<Change> = Vec::new();

    // helper to flush current file
    let flush_file = |files: &mut Vec<FileChanges>, cur_old_path: &mut Option<String>, cur_new_path: &mut Option<String>, cur_changes: &mut Vec<Change>| {
        if !cur_changes.is_empty() || cur_old_path.is_some() || cur_new_path.is_some() {
            files.push(FileChanges {
                old_path: cur_old_path.take(),
                new_path: cur_new_path.take(),
                changes: std::mem::take(cur_changes),
            });
        }
    };

    // We don't require a global header; we look for file markers and hunks.
    while let Some(line) = lines.next() {
        if line.starts_with("diff --git ") {
            // New file diff section starts. Flush previous.
            flush_file(&mut files, &mut cur_old_path, &mut cur_new_path, &mut cur_changes);
            // Not strictly needed to parse paths here; use ---/+++ for reliable values.
            continue;
        }

        if line.starts_with("--- ") {
            // e.g., --- a/path or --- /dev/null
            let old_path = line[4..].trim();
            cur_old_path = if old_path == "/dev/null" {
                None
            } else {
                Some(strip_a_b_prefix(old_path).to_string())
            };
            // Expect +++ to follow (not strictly enforced here)
            if let Some(next) = lines.next() {
                if next.starts_with("+++ ") {
                    let new_path = next[4..].trim();
                    cur_new_path = if new_path == "/dev/null" {
                        None
                    } else {
                        Some(strip_a_b_prefix(new_path).to_string())
                    };
                } else {
                    // Unexpected but allow; push back by manual handling not supported, so ignore.
                }
            }
            continue;
        }

        if let Some(hunk) = line.strip_prefix("@@ ") {
            // Parse hunk header: -l(,s)? +l(,s)? @@ ...
            let (range, _rest) = parse_hunk_header(hunk)?;

            // Iterate following lines as hunk body until next header/file marker
            let mut old_ln = range.old_start;
            let mut new_ln = range.new_start;

            while let Some(&peek) = lines.peek() {
                if peek.starts_with("@@ ") || peek.starts_with("diff --git ") || peek.starts_with("--- ") {
                    break; // end of hunk/file
                }
                let body = lines.next().unwrap();
                if body.starts_with('+') {
                    cur_changes.push(Change {
                        kind: ChangeKind::Added,
                        old_line: None,
                        new_line: Some(new_ln),
                        content: body[1..].to_string(),
                    });
                    new_ln += 1;
                } else if body.starts_with('-') {
                    cur_changes.push(Change {
                        kind: ChangeKind::Removed,
                        old_line: Some(old_ln),
                        new_line: None,
                        content: body[1..].to_string(),
                    });
                    old_ln += 1;
                } else if body.starts_with(' ') || body.is_empty() {
                    // context line (empty line can appear as context in some diffs)
                    cur_changes.push(Change {
                        kind: ChangeKind::Context,
                        old_line: Some(old_ln),
                        new_line: Some(new_ln),
                        content: body.strip_prefix(' ').unwrap_or(body).to_string(),
                    });
                    old_ln += 1;
                    new_ln += 1;
                } else if body.starts_with('\\') {
                    // "\\ No newline at end of file" — ignore for content but don't advance counters
                } else {
                    // Unknown marker; treat as context to be resilient
                    cur_changes.push(Change {
                        kind: ChangeKind::Context,
                        old_line: Some(old_ln),
                        new_line: Some(new_ln),
                        content: body.to_string(),
                    });
                    old_ln += 1;
                    new_ln += 1;
                }
            }
            continue;
        }
        // Ignore other lines like file mode changes, index lines etc.
    }

    // Flush last file if pending
    flush_file(&mut files, &mut cur_old_path, &mut cur_new_path, &mut cur_changes);

    if files.is_empty() {
        // Not strictly an error; but signal to caller if absolutely nothing parsed
        return Err(DiffParseError::MissingHeader);
    }
    Ok(files)
}

fn strip_a_b_prefix(path: &str) -> &str {
    if let Some(stripped) = path.strip_prefix("a/") {
        stripped
    } else if let Some(stripped) = path.strip_prefix("b/") {
        stripped
    } else {
        path
    }
}

fn parse_hunk_header(h: &str) -> Result<(HunkRange, &str), DiffParseError> {
    // h like: -12,3 +34,2 @@ optional
    let after_minus = h;
    let (old_part, rest1) = split_at_space(after_minus).ok_or_else(|| DiffParseError::InvalidHunkHeader(h.to_string()))?;
    if !old_part.starts_with('-') { return Err(DiffParseError::InvalidHunkHeader(h.to_string())); }
    let (new_part, rest2) = split_at_space(rest1).ok_or_else(|| DiffParseError::InvalidHunkHeader(h.to_string()))?;
    if !new_part.starts_with('+') { return Err(DiffParseError::InvalidHunkHeader(h.to_string())); }

    let old_nums = &old_part[1..];
    let new_nums = &new_part[1..];

    let (old_start, old_len) = parse_start_len(old_nums);
    let (new_start, new_len) = parse_start_len(new_nums);

    // Skip trailing @@ ...
    let rest = rest2.trim_start();
    let rest = if let Some(idx) = rest.find("@@") { &rest[idx + 2..] } else { rest };

    Ok((HunkRange { old_start, old_len, new_start, new_len }, rest))
}

fn split_at_space(s: &str) -> Option<(&str, &str)> {
    let s = s.trim_start();
    if s.is_empty() { return None; }
    if let Some(pos) = s.find(' ') {
        Some((&s[..pos], &s[pos+1..]))
    } else {
        None
    }
}

fn parse_start_len(s: &str) -> (u32, u32) {
    if let Some((a, b)) = s.split_once(',') {
        (a.parse().unwrap_or(0), b.parse().unwrap_or(0))
    } else {
        (s.parse().unwrap_or(0), 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    const SIMPLE_DIFF: &str = r#"diff --git a/foo.txt b/foo.txt
index e69de29..4b825dc 100644
--- a/foo.txt
+++ b/foo.txt
@@ -0,0 +1,3 @@
+line 1
+line 2
+line 3
"#;

    #[test]
    fn parse_added_file() {
        let files = parse_unified_diff(SIMPLE_DIFF).expect("parsed");
        assert_eq!(files.len(), 1);
        let f = &files[0];
        assert_eq!(f.old_path, Some("foo.txt".to_string()));
        assert_eq!(f.new_path, Some("foo.txt".to_string()));
        let added: Vec<_> = f.changes.iter().filter(|c| c.kind == ChangeKind::Added).collect();
        assert_eq!(added.len(), 3);
        assert_eq!(added[0].new_line, Some(1));
        assert_eq!(added[2].new_line, Some(3));
    }

    const MODIFIED_DIFF: &str = r#"diff --git a/bar.rs b/bar.rs
index 1111111..2222222 100644
--- a/bar.rs
+++ b/bar.rs
@@ -1,4 +1,4 @@
 fn main() {
-    println!("hi");
+    println!("hello");
 }
"#;

    #[test]
    fn parse_modified_file() {
        let files = parse_unified_diff(MODIFIED_DIFF).expect("parsed");
        assert_eq!(files.len(), 1);
        let f = &files[0];
        assert_eq!(f.changes.len(), 4); // context, removed, added, context
        assert!(f.changes.iter().any(|c| matches!(c.kind, ChangeKind::Removed)));
        assert!(f.changes.iter().any(|c| matches!(c.kind, ChangeKind::Added)));
    }

    const MULTI_HUNK_DIFF: &str = r#"diff --git a/a.txt b/a.txt
--- a/a.txt
+++ b/a.txt
@@ -1,2 +1,2 @@
 a
-b
+B
@@ -10,0 +11,2 @@
+x
+y
"#;

    #[test]
    fn parse_multi_hunk() {
        let files = parse_unified_diff(MULTI_HUNK_DIFF).expect("parsed");
        let f = &files[0];
        let added: Vec<_> = f.changes.iter().filter(|c| c.kind == ChangeKind::Added).collect();
        assert_eq!(added.len(), 3);
        assert_eq!(added[0].new_line, Some(2));
        assert_eq!(added[1].new_line, Some(11));
        assert_eq!(added[2].new_line, Some(12));
    }
}

