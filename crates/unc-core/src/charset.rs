//! UNSCII, the character ROM: 256 glyphs of 8 x 8 pixels, 8 bytes each. The first 128 are
//! the set the screen shows as it is switched on (graphics and ASCII, lowercase included);
//! the second 128 are for 256-glyph mode.
//!
//! The source is the "graph paper" file rom/unscii.txt, parsed here at compile time, so
//! editing a glyph is editing a text file. A malformed file is a compile error.

/// Glyphs `$00-$FF`, row by row; bit 7 of each byte is the leftmost pixel.
pub static CHAR_ROM: [u8; 2048] = parse(include_str!("../../../rom/unscii.txt"));

const fn hex(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'A'..=b'F' => c - b'A' + 10,
        b'a'..=b'f' => c - b'a' + 10,
        _ => panic!("unscii.txt: bad hex digit in a glyph header"),
    }
}

const fn parse(src: &str) -> [u8; 2048] {
    let s = src.as_bytes();
    let mut out = [0u8; 2048];
    let mut seen = [false; 256];
    let mut code: usize = 0;
    let mut row: usize = 8;
    let mut i = 0;
    while i < s.len() {
        let start = i;
        while i < s.len() && s[i] != b'\n' {
            i += 1;
        }
        let mut end = i;
        if end > start && s[end - 1] == b'\r' {
            end -= 1;
        }
        i += 1;
        if end == start || s[start] == b';' {
            continue;
        }
        if s[start] == b'$' {
            if row != 8 {
                panic!("unscii.txt: a glyph has fewer than 8 rows");
            }
            if end - start < 3 {
                panic!("unscii.txt: glyph header needs two hex digits");
            }
            code = (hex(s[start + 1]) * 16 + hex(s[start + 2])) as usize;
            if seen[code] {
                panic!("unscii.txt: a glyph is defined twice");
            }
            seen[code] = true;
            row = 0;
            continue;
        }
        if row >= 8 {
            panic!("unscii.txt: a glyph has more than 8 rows, or a row has no header");
        }
        if end - start != 8 {
            panic!("unscii.txt: every row is exactly 8 pixels");
        }
        let mut byte = 0u8;
        let mut c = 0;
        while c < 8 {
            byte <<= 1;
            match s[start + c] {
                b'#' => byte |= 1,
                b'.' => {}
                _ => panic!("unscii.txt: pixels are '#' (ink) or '.' (paper)"),
            }
            c += 1;
        }
        out[code * 8 + row] = byte;
        row += 1;
    }
    if row != 8 {
        panic!("unscii.txt: the last glyph has fewer than 8 rows");
    }
    let mut k = 0;
    while k < 256 {
        if !seen[k] {
            panic!("unscii.txt: every glyph $00-$FF must be defined");
        }
        k += 1;
    }
    out
}
