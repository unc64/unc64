//! Lines, LSP positions (UTF-16 columns) and file URIs.

use std::path::{Path, PathBuf};

/// The lines of a text as the assembler reads them: split at `\n`, a `\r` before it dropped.
pub fn lines(text: &str) -> Vec<&str> {
    text.lines().collect()
}

/// The UTF-16 column of byte offset `byte` in `line`.
pub fn utf16_col(line: &str, byte: usize) -> u32 {
    let byte = byte.min(line.len());
    let byte = (0..=byte).rev().find(|&b| line.is_char_boundary(b)).unwrap_or(0);
    line[..byte].encode_utf16().count() as u32
}

/// The byte offset of UTF-16 column `col` in `line`, clamped to the line.
pub fn byte_col(line: &str, col: u32) -> usize {
    let mut units = 0;
    for (i, c) in line.char_indices() {
        if units >= col as usize {
            return i;
        }
        units += c.len_utf16();
    }
    line.len()
}

fn hex_val(b: u8) -> Option<u8> {
    (b as char).to_digit(16).map(|d| d as u8)
}

/// The path of a `file:` URI, `%xx` escapes decoded.
pub fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    // `file:///C:/x` on Windows; `file:///home/x` elsewhere.
    let rest = if rest.len() > 3 && rest.as_bytes()[0] == b'/' && rest.as_bytes()[2] == b':' { &rest[1..] } else { rest };
    let b = rest.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let (Some(h), Some(l)) = (hex_val(b[i + 1]), hex_val(b[i + 2]))
        {
            out.push(h * 16 + l);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    Some(PathBuf::from(String::from_utf8(out).ok()?))
}

/// A `file:` URI for a path, with what needs escaping escaped.
pub fn path_to_uri(path: &Path) -> String {
    let s = path.to_string_lossy().replace('\\', "/");
    let mut out = String::from("file://");
    if !s.starts_with('/') {
        out.push('/');
    }
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"/-_.~:".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_columns() {
        let l = "LDA #'é' ; 😀 x";
        assert_eq!(utf16_col(l, l.find('x').unwrap()), 14);
        assert_eq!(byte_col(l, 14), l.find('x').unwrap());
        assert_eq!(byte_col(l, 99), l.len());
    }

    #[test]
    fn uris() {
        let p = uri_to_path("file:///home/me/my%20games/snake.s").unwrap();
        assert_eq!(p, PathBuf::from("/home/me/my games/snake.s"));
        assert_eq!(path_to_uri(&p), "file:///home/me/my%20games/snake.s");
    }
}
