//! The keyboard matrix: every key that is down, at once. Rev A. The spec is
//! docs/prg/programmers-reference-guide.md, chapter 11.

use crate::machine::Machine;
use crate::machine::io::{KEY_DOWN, KEY_LEFT};

/// Rows in the matrix, KEYS ($C060) to KEYS + 8.
pub const KEY_ROWS: usize = 9;

/// Where a key code sits in the matrix: (row, bit mask). Codes $20-$5F take rows 0-7,
/// row (code - $20) / 8, bit code AND 7. Row 8 holds the cursor keys left, right, up and
/// down in bits 0-3, RETURN in bit 4, DELETE in bit 5 and STOP in bit 7. Other codes have
/// no place in the matrix.
pub fn key_position(code: u8) -> Option<(usize, u8)> {
    match code {
        0x20..=0x5F => Some(((code - 0x20) as usize >> 3, 1 << (code & 7))),
        KEY_LEFT..=KEY_DOWN => Some((8, 1 << (code - KEY_LEFT))),
        13 => Some((8, 0x10)),
        8 => Some((8, 0x20)),
        3 => Some((8, 0x80)),
        _ => None,
    }
}

impl Machine {
    /// Sets (down) or clears one key in the keyboard matrix, and nothing else. `key` and
    /// `key_up` call it. A host that queues presses, so that typing never outruns KBD,
    /// calls it as soon as the key physically goes down, so the matrix doesn't wait.
    pub fn key_matrix(&mut self, code: u8, down: bool) {
        if let Some((row, bit)) = key_position(code & 0x7F) {
            let r = &mut self.mem.io.keys[row];
            *r = if down { *r | bit } else { *r & !bit };
        }
    }
}
