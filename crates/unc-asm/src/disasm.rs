//! Disassembler: one instruction at a time, in the assembler's own syntax.

use unc_core::isa::{Mode, OPCODES};

/// Disassembles the instruction at `addr`. Returns its text and length in bytes.
/// Unassigned opcodes come back as `.byte $XX`.
pub fn disasm(read: &dyn Fn(u16) -> u8, addr: u16) -> (String, u16) {
    let op = read(addr);
    let Some(info) = OPCODES[op as usize] else {
        return (format!(".byte ${op:02X}"), 1);
    };
    let b1 = read(addr.wrapping_add(1));
    let w = u16::from_le_bytes([b1, read(addr.wrapping_add(2))]);
    let m = info.op.mnemonic();
    let text = match info.mode {
        Mode::Imp => m.to_string(),
        Mode::Acc => format!("{m} A"),
        Mode::Imm => format!("{m} #${b1:02X}"),
        Mode::Fp => format!("{m} ${b1:02X}"),
        Mode::Fpx => format!("{m} ${b1:02X},X"),
        Mode::Fpy => format!("{m} ${b1:02X},Y"),
        Mode::Abs => format!("{m} ${w:04X}"),
        Mode::Absx => format!("{m} ${w:04X},X"),
        Mode::Absy => format!("{m} ${w:04X},Y"),
        Mode::Ind => format!("{m} (${w:04X})"),
        Mode::Indy => format!("{m} (${b1:02X}),Y"),
        Mode::Rel => format!("{m} ${:04X}", addr.wrapping_add(2).wrapping_add(b1 as i8 as i16 as u16)),
    };
    (text, info.mode.len() as u16)
}
