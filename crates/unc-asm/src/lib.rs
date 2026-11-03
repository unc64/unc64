//! UNC 64 toolchain: assembler, disassembler, reference docs, tapes, BASIC listings, and the
//! loader that puts an assembled program into a machine the same way in every host.

pub mod analysis;
pub mod basic;
pub mod asm;
pub mod disasm;
pub mod doc;
mod json;
pub mod lang;
pub mod tape;

pub use asm::{AsmError, Assembly, DEFAULT_ORIGIN, LineInfo, ListLine, Segment, assemble, assemble_with};
pub use analysis::{Analysis, analysis_json, analyze, analyze_with};
pub use disasm::disasm;

use unc_core::cpu::{CpuState, flag};
use unc_core::{Machine, RamSize};

/// Loads a program and sets the defined start state (manual chapter 6, "Where the program
/// starts"): PC = the start address, SP = $FF, P = $24 (I set, so interrupts are off),
/// A = X = Y = 0. Memory and I/O are left as they are. Returns the start address.
pub fn load(m: &mut Machine, asm: &Assembly) -> u16 {
    take_cursor_away(m);
    for s in &asm.segments {
        m.load(s.start, &s.bytes);
    }
    let start = asm.start().unwrap_or(DEFAULT_ORIGIN);
    let c = &mut m.cpu;
    c.pc = start;
    c.sp = 0xFF;
    c.p = flag::I | flag::U;
    c.a = 0;
    c.x = 0;
    c.y = 0;
    c.state = CpuState::Running;
    start
}

/// A program is loaded into a machine waiting at BASIC's `READY.`, where the line editor
/// shows its cursor. The program didn't ask for it, so put back what was under it, as the
/// ROM's CUROFF does when BASIC stops waiting (manual chapter 13).
fn take_cursor_away(m: &mut Machine) {
    const CURX: u16 = 0x0200;
    const CURY: u16 = 0x0201;
    const RUNFLG: u16 = 0x0204;
    const CURSAV: u16 = 0x020F;
    let (x, y) = (m.peek(CURX) as u16, m.peek(CURY) as u16);
    if m.peek(RUNFLG) != 0 || x >= 40 || y >= 25 {
        return;
    }
    let cell = unc_core::machine::SCREEN + y * 40 + x;
    if m.peek(cell) == 0xA0 {
        let under = m.peek(CURSAV);
        m.poke(cell, under);
    }
}

/// A machine as the hosts give it to a program: switched on, booted to BASIC
/// (`Machine::boot`), with `asm` loaded and about to run from its start.
pub fn boot(ram: RamSize, asm: &Assembly) -> Machine {
    let mut m = Machine::boot(ram);
    load(&mut m, asm);
    m
}
