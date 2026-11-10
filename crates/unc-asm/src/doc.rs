//! Generates the Programmer's Reference Guide's instruction-set appendix (Appendix C) from the opcode table.

use unc_core::isa::{ALL_OPS, Mode, OPCODES, Op};

pub(crate) fn syntax(op: Op, mode: Mode) -> String {
    let m = op.mnemonic();
    match mode {
        Mode::Imp => m.into(),
        Mode::Acc => format!("{m} A"),
        Mode::Imm => format!("{m} #$nn"),
        Mode::Fp => format!("{m} $nn"),
        Mode::Fpx => format!("{m} $nn,X"),
        Mode::Fpy => format!("{m} $nn,Y"),
        Mode::Abs => format!("{m} $nnnn"),
        Mode::Absx => format!("{m} $nnnn,X"),
        Mode::Absy => format!("{m} $nnnn,Y"),
        Mode::Ind => format!("{m} ($nnnn)"),
        Mode::Indy => format!("{m} ($nn),Y"),
        Mode::Rel => format!("{m} label"),
    }
}

pub fn describe(op: Op) -> (&'static str, &'static str) {
    match op {
        Op::Brk => ("Break: push PC and status (B set), jump through $FFFE", "I"),
        Op::Nop => ("No operation", "-"),
        Op::Hlt => ("Halt the processor until reset, or an NMI", "-"),
        Op::Rts => ("Return from subroutine", "-"),
        Op::Rti => ("Return from interrupt: pull status, then PC", "all"),
        Op::Jsr => ("Jump to subroutine: push the return address", "-"),
        Op::Jmp => ("Jump", "-"),
        Op::Beq => ("Branch if equal (Z set)", "-"),
        Op::Bne => ("Branch if not equal (Z clear)", "-"),
        Op::Bcc => ("Branch if carry clear", "-"),
        Op::Bcs => ("Branch if carry set", "-"),
        Op::Bmi => ("Branch if minus (N set)", "-"),
        Op::Bpl => ("Branch if plus (N clear)", "-"),
        Op::Bvc => ("Branch if overflow clear", "-"),
        Op::Bvs => ("Branch if overflow set", "-"),
        Op::Tax => ("A to X", "N Z"),
        Op::Tay => ("A to Y", "N Z"),
        Op::Txa => ("X to A", "N Z"),
        Op::Tya => ("Y to A", "N Z"),
        Op::Txs => ("X to stack pointer", "-"),
        Op::Tsx => ("Stack pointer to X", "N Z"),
        Op::Inx => ("X + 1", "N Z"),
        Op::Iny => ("Y + 1", "N Z"),
        Op::Dex => ("X - 1", "N Z"),
        Op::Dey => ("Y - 1", "N Z"),
        Op::Pha => ("Push A", "-"),
        Op::Pla => ("Pull A", "N Z"),
        Op::Php => ("Push status (with B set)", "-"),
        Op::Plp => ("Pull status", "all"),
        Op::Clc => ("Clear carry", "C"),
        Op::Sec => ("Set carry", "C"),
        Op::Cli => ("Clear interrupt disable", "I"),
        Op::Sei => ("Set interrupt disable", "I"),
        Op::Clv => ("Clear overflow", "V"),
        Op::Lda => ("Load A", "N Z"),
        Op::Sta => ("Store A", "-"),
        Op::Adc => ("A + M + C", "N V Z C"),
        Op::Sbc => ("A - M - (1 - C)", "N V Z C"),
        Op::And => ("A AND M", "N Z"),
        Op::Ora => ("A OR M", "N Z"),
        Op::Eor => ("A XOR M", "N Z"),
        Op::Cmp => ("Compare A with M", "N Z C"),
        Op::Ldx => ("Load X", "N Z"),
        Op::Ldy => ("Load Y", "N Z"),
        Op::Stx => ("Store X", "-"),
        Op::Sty => ("Store Y", "-"),
        Op::Cpx => ("Compare X with M", "N Z C"),
        Op::Cpy => ("Compare Y with M", "N Z C"),
        Op::Inc => ("M + 1", "N Z"),
        Op::Dec => ("M - 1", "N Z"),
        Op::Asl => ("Shift left; bit 7 to C", "N Z C"),
        Op::Lsr => ("Shift right; bit 0 to C", "N Z C"),
        Op::Rol => ("Rotate left through C", "N Z C"),
        Op::Ror => ("Rotate right through C", "N Z C"),
        Op::Bit => ("Test bits: Z = A AND M is zero, N = M bit 7, V = M bit 6", "N V Z"),
    }
}

/// The instruction-set reference, generated into Appendix C of
/// docs/prg/programmers-reference-guide.md (between the GENERATED markers).
pub fn isa_reference() -> String {
    let mut s = String::new();
    let n = OPCODES.iter().flatten().count();
    s.push_str(&format!("The UNC-1 processor has {} operations in {} opcodes. Cycle costs are fixed: a taken branch adds one cycle, and nothing else varies. There is no decimal mode and no page-crossing penalty.\n\n", ALL_OPS.len(), n));
    s.push_str("### Opcode map\n\nRows are the high nibble, columns the low nibble. Blank cells are not instructions this book describes: as a rule, the processor jams on them.\n\n|   |");
    for c in 0..16 {
        s.push_str(&format!(" x{c:X} |"));
    }
    s.push_str("\n|---|");
    s.push_str(&"---|".repeat(16));
    s.push('\n');
    for r in 0..16 {
        s.push_str(&format!("| **{r:X}x** |"));
        for c in 0..16 {
            match OPCODES[r * 16 + c] {
                Some(i) => s.push_str(&format!(" {} {} |", i.op.mnemonic(), short(i.mode))),
                None => s.push_str("  |"),
            }
        }
        s.push('\n');
    }
    s.push_str("\n### Instructions\n\n| Syntax | Opcode | Bytes | Cycles | Operation | Flags |\n|---|---|---|---|---|---|\n");
    for &op in ALL_OPS {
        let (what, flags) = describe(op);
        for (code, info) in OPCODES.iter().enumerate().filter_map(|(c, i)| i.filter(|i| i.op == op).map(|i| (c, i))) {
            let cyc = if op.is_branch() { "2 (+1 if taken)".to_string() } else { info.cycles.to_string() };
            s.push_str(&format!("| `{}` | ${code:02X} | {} | {cyc} | {what} | {flags} |\n", syntax(op, info.mode), info.mode.len()));
        }
    }
    s
}

fn short(m: Mode) -> &'static str {
    match m {
        Mode::Imp | Mode::Rel => "",
        Mode::Acc => "A",
        Mode::Imm => "#",
        Mode::Fp => "fp",
        Mode::Fpx => "fp,X",
        Mode::Fpy => "fp,Y",
        Mode::Abs => "abs",
        Mode::Absx => "abs,X",
        Mode::Absy => "abs,Y",
        Mode::Ind => "(abs)",
        Mode::Indy => "(fp),Y",
    }
}
