//! BASIC listings on the host (manual chapter 2). The ROM tokenizes, so the host doesn't
//! keep a second tokenizer: a listing is typed into a booted machine line by line, as a
//! person would type it, and the program is read back out of memory. The result is byte
//! for byte what the machine holds, which is what a type-2 tape carries (`tape::basic`).

use std::collections::BTreeSet;

use unc_core::machine::io;
use unc_core::{Machine, RamSize};

/// The longest line BASIC's screen editor takes.
pub const MAX_LINE: usize = 120;

/// Types `listing` into a booted UNC 64 (with the 64K expansion, for room) and returns the
/// program as BASIC keeps it from `$0800`, up to and including its end marker.
///
/// Every line that isn't blank starts with a line number (0-65535). As at the keyboard, a
/// line replaces an earlier one with its number, and a number alone deletes the line. It's
/// typed with CASE set, so small letters arrive as they are: BASIC keeps them inside quotes
/// and after REM and takes them for capitals everywhere else. Anything the UNC can't type
/// (outside `$20-$7E`), or a line over 120 characters, is an error, as is a program too big
/// for memory.
pub fn tokenize(listing: &str) -> Result<Vec<u8>, String> {
    let mut m = Machine::boot(RamSize::K64);
    m.poke(io::CASE, 1);
    let mut want = BTreeSet::new();
    for (i, raw) in listing.lines().enumerate() {
        let n = i + 1;
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(c) = line.chars().find(|c| !(' '..='~').contains(c)) {
            return Err(format!("line {n}: {c:?} isn't a character the UNC can type"));
        }
        if line.len() > MAX_LINE {
            return Err(format!("line {n}: {} characters; BASIC takes at most {MAX_LINE}", line.len()));
        }
        let digits: String = line.chars().take_while(char::is_ascii_digit).collect();
        let number: u16 = digits.parse().map_err(|_| format!("line {n}: doesn't start with a line number (0-65535)"))?;
        if line[digits.len()..].trim().is_empty() {
            want.remove(&number);
        } else {
            want.insert(number);
        }
        for k in line.bytes().chain(*b"\r") {
            type_key(&mut m, k)?;
        }
    }
    type_key(&mut m, b' ')?; // taken only once BASIC is back for the next line: the last is stored
    let program = crate::tape::resident_basic(&m);
    let have: BTreeSet<u16> = crate::tape::check_basic(&program)?.into_iter().collect();
    if have != want {
        return Err(format!("the program doesn't fit in memory: {} of {} lines stored", have.len(), want.len()));
    }
    Ok(program)
}

/// Presses a key and runs the machine until BASIC has taken it.
fn type_key(m: &mut Machine, k: u8) -> Result<(), String> {
    m.key(k);
    for _ in 0..400 {
        m.run_cycles(500);
        if m.peek(io::KBD) & 0x80 == 0 {
            m.key_up(k);
            return Ok(());
        }
    }
    Err("the machine stopped taking keys".into())
}
