//! The UNC tape format, version 1 (manual chapter 14). A tape is one or more programs one
//! after another, each a header, its data and a checksum:
//!
//! | Offset | Size | What |
//! |---|---|---|
//! | 0 | 4 | `UNCT`, the magic the ROM searches for |
//! | 4 | 1 | Version: 1 |
//! | 5 | 1 | Type: 1 = machine code, started at its entry; 2 = a BASIC program |
//! | 6 | 16 | Name: upper case, padded with spaces |
//! | 22 | 2 | Load address, low byte first |
//! | 24 | 2 | Length of the data, low byte first |
//! | 26 | 2 | Entry address, low byte first |
//! | 28 | length | The data |
//! | 28 + length | 1 | Checksum: every byte from the version to the last data byte, XORed |
//!
//! **Type 2, a BASIC program** (Rev A): the data is the program exactly as BASIC keeps
//! it from `$0800`, up to and including its end marker: each line a length byte (the whole
//! line), the line number (low byte first), the text with its tokens and a 0; then a 0
//! length. Load address `$0800`, entry 0 (unused). There are no addresses in it, so it's
//! the same on a UNC 16 and a UNC 64, and no variables: LOAD clears them, as NEW does.
//!
//! A tape image file (`.tap`) is exactly the bytes on the tape, so the version is at byte
//! 4 of every file, and two tapes `cat`ed together make one tape with both programs on it.

use crate::asm::{Assembly, DEFAULT_ORIGIN};
use unc_core::Machine;

pub const MAGIC: &[u8; 4] = b"UNCT";
pub const VERSION: u8 = 1;
/// Machine code: loaded at its load address and started at its entry.
pub const TYPE_MACHINE_CODE: u8 = 1;
/// A BASIC program: its lines and end marker, loaded at `BASIC_START` in place of the
/// resident program. LOAD gives `READY.`; it doesn't RUN it.
pub const TYPE_BASIC: u8 = 2;
/// Where BASIC keeps its program, and where a type-2 program loads.
pub const BASIC_START: u16 = 0x0800;
/// Where BASIC keeps the address of its program's end marker (PRGEND, manual Appendix D).
pub const PRGEND: u16 = 0x0209;
pub const NAME_LEN: usize = 16;
/// Bytes before the data: magic, version, type, name, load, length, entry.
pub const HEADER_LEN: usize = 28;

/// One program on a tape.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Program {
    pub kind: u8,
    /// The name as stored: `NAME_LEN` bytes, upper case, space-padded.
    pub name: [u8; NAME_LEN],
    pub load: u16,
    pub entry: u16,
    pub data: Vec<u8>,
}

impl Program {
    /// The name without its padding.
    pub fn name(&self) -> String {
        String::from_utf8_lossy(&self.name).trim_end().to_string()
    }

    /// The program as it goes on the tape: header, data, checksum.
    pub fn encode(&self) -> Vec<u8> {
        assert!(self.data.len() <= 0xFFFF, "a program on tape is at most 65535 bytes");
        let mut out = Vec::with_capacity(HEADER_LEN + self.data.len() + 1);
        out.extend_from_slice(MAGIC);
        out.push(VERSION);
        out.push(self.kind);
        out.extend_from_slice(&self.name);
        for w in [self.load, self.data.len() as u16, self.entry] {
            out.extend_from_slice(&w.to_le_bytes());
        }
        out.extend_from_slice(&self.data);
        out.push(checksum(&out[MAGIC.len()..]));
        out
    }
}

/// The checksum of a program's bytes from the version on: all of them XORed together.
pub fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0, |c, b| c ^ b)
}

/// A name as the tape stores it: upper case, only characters the UNC can type ($20-$5F),
/// at most 16 of them, padded with spaces.
pub fn tape_name(name: &str) -> [u8; NAME_LEN] {
    let mut out = [b' '; NAME_LEN];
    let chars = name.to_uppercase().chars().filter(|c| (' '..='_').contains(c)).take(NAME_LEN).collect::<Vec<_>>();
    for (o, c) in out.iter_mut().zip(chars) {
        *o = c as u8;
    }
    out
}

/// A tape holding one machine-code program: the assembly from its lowest to its highest
/// byte (gaps filled with 0), started at `start` (or its first byte), named `name`.
pub fn make(asm: &Assembly, name: &str) -> Vec<u8> {
    program(asm, name).encode()
}

/// The program `make` puts on tape.
pub fn program(asm: &Assembly, name: &str) -> Program {
    let used = asm.segments.iter().filter(|s| !s.bytes.is_empty());
    let lo = used.clone().map(|s| s.start).min();
    let hi = used.map(|s| s.start as usize + s.bytes.len() - 1).max();
    let (load, data) = match (lo, hi) {
        (Some(lo), Some(hi)) => (lo, asm.image(lo, hi as u16, 0)),
        _ => (DEFAULT_ORIGIN, Vec::new()),
    };
    Program { kind: TYPE_MACHINE_CODE, name: tape_name(name), load, entry: asm.start().unwrap_or(load), data }
}

/// A tape holding a BASIC program (type 2), named `name`. `program` is the program as BASIC
/// keeps it from `$0800`, up to and including its end marker (`resident_basic` reads it out
/// of a machine; `crate::basic::tokenize` makes one from a listing).
pub fn basic(name: &str, program: &[u8]) -> Program {
    Program { kind: TYPE_BASIC, name: tape_name(name), load: BASIC_START, entry: 0, data: program.to_vec() }
}

/// The BASIC program in a machine's memory, as a type-2 program carries it: `$0800` up to
/// and including the end marker at PRGEND. Just an end marker (`[0]`) if PRGEND doesn't
/// point at one.
pub fn resident_basic(m: &Machine) -> Vec<u8> {
    let end = u16::from_le_bytes([m.peek(PRGEND), m.peek(PRGEND + 1)]);
    if !(BASIC_START..0xC000).contains(&end) {
        return vec![0];
    }
    let bytes: Vec<u8> = (BASIC_START..=end).map(|a| m.peek(a)).collect();
    if check_basic(&bytes).is_err() {
        return vec![0];
    }
    bytes
}

/// Whether `data` is a BASIC program as type 2 carries it: lines of at least 4 bytes, each
/// ending in 0, and then the end marker as its last byte. Returns the line numbers.
pub fn check_basic(data: &[u8]) -> Result<Vec<u16>, String> {
    let mut at = 0;
    let mut lines = Vec::new();
    loop {
        match data.get(at) {
            None => return Err(format!("byte {at}: no end marker")),
            Some(0) if at + 1 == data.len() => return Ok(lines),
            Some(0) => return Err(format!("byte {at}: bytes after the end marker")),
            Some(&len) if len < 4 => return Err(format!("byte {at}: a line {len} bytes long")),
            Some(&len) => {
                let end = at + len as usize;
                if end >= data.len() || data[end - 1] != 0 {
                    return Err(format!("byte {at}: a line that doesn't end in 0"));
                }
                lines.push(u16::from_le_bytes([data[at + 1], data[at + 2]]));
                at = end;
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TapeError {
    /// No `UNCT` where a program should start.
    NoMagic { offset: usize },
    /// A version this reader doesn't know.
    Version { offset: usize, version: u8 },
    /// The tape ends inside a program.
    Truncated { offset: usize },
    /// The checksum doesn't match: the program's bytes are damaged.
    Checksum { offset: usize, name: String },
    /// A type-2 program that isn't a BASIC program (`check_basic`), or not at `$0800`.
    Basic { offset: usize, name: String, why: String },
}

impl std::fmt::Display for TapeError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            TapeError::NoMagic { offset } => write!(f, "byte {offset}: not a UNC tape (no UNCT)"),
            TapeError::Version { offset, version } => write!(f, "byte {offset}: tape format version {version}, this reader knows 1"),
            TapeError::Truncated { offset } => write!(f, "byte {offset}: the tape ends inside a program"),
            TapeError::Checksum { offset, name } => write!(f, "byte {offset}: {name}: checksum error"),
            TapeError::Basic { offset, name, why } => write!(f, "byte {offset}: {name}: not a BASIC program: {why}"),
        }
    }
}

/// Reads every program on a tape, in order. A BASIC program (type 2) is checked the way the
/// ROM's LOAD checks it: at `$0800`, and a well-formed program (`check_basic`).
pub fn parse(tape: &[u8]) -> Result<Vec<Program>, TapeError> {
    let mut out = Vec::new();
    let mut at = 0;
    while at < tape.len() {
        let rest = &tape[at..];
        if !rest.starts_with(MAGIC) {
            return Err(TapeError::NoMagic { offset: at });
        }
        if rest.len() > 4 && rest[4] != VERSION {
            return Err(TapeError::Version { offset: at, version: rest[4] });
        }
        if rest.len() < HEADER_LEN {
            return Err(TapeError::Truncated { offset: at });
        }
        let word = |i: usize| u16::from_le_bytes([rest[i], rest[i + 1]]);
        let len = word(24) as usize;
        let end = HEADER_LEN + len;
        if rest.len() <= end {
            return Err(TapeError::Truncated { offset: at });
        }
        let p = Program {
            kind: rest[5],
            name: rest[6..22].try_into().unwrap(),
            load: word(22),
            entry: word(26),
            data: rest[HEADER_LEN..end].to_vec(),
        };
        if checksum(&rest[MAGIC.len()..=end]) != 0 {
            return Err(TapeError::Checksum { offset: at, name: p.name() });
        }
        if p.kind == TYPE_BASIC {
            let why = if p.load != BASIC_START { Err(format!("at ${:04X}, not $0800", p.load)) } else { check_basic(&p.data).map(drop) };
            if let Err(why) = why {
                return Err(TapeError::Basic { offset: at, name: p.name(), why });
            }
        }
        out.push(p);
        at += end + 1;
    }
    Ok(out)
}
