//! Common utilities for language analyzers.
/// Build a vector of byte offsets for each line start.
/// The returned Vec has length = number of lines + 1,
/// where element i is the starting byte index of line i (1-based).
pub fn line_offsets(src: &str) -> Vec<usize> {
    let mut offs = Vec::with_capacity(src.len() / 20 + 2);
    offs.push(0);
    for (i, b) in src.bytes().enumerate() {
        if b == b'\n' {
            offs.push(i + 1);
        }
    }
    offs
}

/// Convert a byte index to a line number (1-based) using precomputed offsets.
pub fn byte_to_line(offs: &[usize], byte: usize) -> u32 {
    match offs.binary_search(&byte) {
        Ok(i) => (i as u32) + 1,
        Err(i) => i as u32,
    }
}
