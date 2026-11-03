//! The UNC 64 instruction set, Rev A.
//!
//! This table is the single source of truth for opcodes, addressing modes and cycle costs.
//! Appendix C of the Programmer's Reference Guide and the test suite are generated from it.
//!
//! Layout: opcodes $00-$27 are the fixed control, branch, transfer and flag instructions.
//! From $28 up, each operation owns a row of eight opcodes and the low three bits pick the
//! addressing mode (the slot): 0 `#imm`, 1 `fp`, 2 `fp,X`, 3 `abs`, 4 `abs,X`, 5 `abs,Y`,
//! 6 `(fp),Y`, 7 `A` for shifts or `fp,Y` for LDX/STX. $D0-$FF are not in the book.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mode {
    Imp,
    Acc,
    Imm,
    Fp,
    Fpx,
    Fpy,
    Abs,
    Absx,
    Absy,
    Ind,
    Indy,
    Rel,
}

#[allow(clippy::len_without_is_empty)]
impl Mode {
    pub const fn operand_bytes(self) -> u8 {
        match self {
            Mode::Imp | Mode::Acc => 0,
            Mode::Imm | Mode::Fp | Mode::Fpx | Mode::Fpy | Mode::Indy | Mode::Rel => 1,
            Mode::Abs | Mode::Absx | Mode::Absy | Mode::Ind => 2,
        }
    }

    pub const fn len(self) -> u8 {
        1 + self.operand_bytes()
    }

    pub const fn name(self) -> &'static str {
        match self {
            Mode::Imp => "implied",
            Mode::Acc => "accumulator",
            Mode::Imm => "immediate",
            Mode::Fp => "fast page",
            Mode::Fpx => "fast page,X",
            Mode::Fpy => "fast page,Y",
            Mode::Abs => "absolute",
            Mode::Absx => "absolute,X",
            Mode::Absy => "absolute,Y",
            Mode::Ind => "indirect",
            Mode::Indy => "(fast page),Y",
            Mode::Rel => "relative",
        }
    }
}

macro_rules! ops {
    ($($v:ident $m:literal),* $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum Op { $($v),* }
        pub const ALL_OPS: &[Op] = &[$(Op::$v),*];
        impl Op {
            pub const fn mnemonic(self) -> &'static str {
                match self { $(Op::$v => $m),* }
            }
        }
    };
}

ops! {
    Brk "BRK", Nop "NOP", Hlt "HLT", Rts "RTS", Rti "RTI", Jsr "JSR", Jmp "JMP",
    Beq "BEQ", Bne "BNE", Bcc "BCC", Bcs "BCS", Bmi "BMI", Bpl "BPL", Bvc "BVC", Bvs "BVS",
    Tax "TAX", Tay "TAY", Txa "TXA", Tya "TYA", Txs "TXS", Tsx "TSX", Inx "INX", Iny "INY",
    Dex "DEX", Dey "DEY", Pha "PHA", Pla "PLA", Php "PHP", Plp "PLP", Clc "CLC", Sec "SEC",
    Cli "CLI", Sei "SEI", Clv "CLV",
    Lda "LDA", Sta "STA", Adc "ADC", Sbc "SBC", And "AND", Ora "ORA", Eor "EOR", Cmp "CMP",
    Ldx "LDX", Ldy "LDY", Stx "STX", Sty "STY", Cpx "CPX", Cpy "CPY",
    Inc "INC", Dec "DEC", Asl "ASL", Lsr "LSR", Rol "ROL", Ror "ROR", Bit "BIT",
}

impl Op {
    pub const fn is_branch(self) -> bool {
        matches!(
            self,
            Op::Beq | Op::Bne | Op::Bcc | Op::Bcs | Op::Bmi | Op::Bpl | Op::Bvc | Op::Bvs
        )
    }

    pub fn from_mnemonic(s: &str) -> Option<Op> {
        ALL_OPS.iter().copied().find(|op| op.mnemonic().eq_ignore_ascii_case(s))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpInfo {
    pub op: Op,
    pub mode: Mode,
    /// Fixed cost in cycles. Taken branches add one; nothing else varies.
    pub cycles: u8,
}

#[derive(Clone, Copy)]
enum Kind {
    Read,
    Rmw,
}

const FIXED: [(u8, Op, Mode, u8); 35] = [
    (0x00, Op::Brk, Mode::Imp, 6),
    (0x01, Op::Nop, Mode::Imp, 2),
    (0x02, Op::Hlt, Mode::Imp, 2),
    (0x03, Op::Rts, Mode::Imp, 6),
    (0x04, Op::Rti, Mode::Imp, 6),
    (0x05, Op::Jsr, Mode::Abs, 6),
    (0x06, Op::Jmp, Mode::Abs, 3),
    (0x07, Op::Jmp, Mode::Ind, 5),
    (0x08, Op::Beq, Mode::Rel, 2),
    (0x09, Op::Bne, Mode::Rel, 2),
    (0x0A, Op::Bcc, Mode::Rel, 2),
    (0x0B, Op::Bcs, Mode::Rel, 2),
    (0x0C, Op::Bmi, Mode::Rel, 2),
    (0x0D, Op::Bpl, Mode::Rel, 2),
    (0x0E, Op::Bvc, Mode::Rel, 2),
    (0x0F, Op::Bvs, Mode::Rel, 2),
    (0x10, Op::Tax, Mode::Imp, 2),
    (0x11, Op::Tay, Mode::Imp, 2),
    (0x12, Op::Txa, Mode::Imp, 2),
    (0x13, Op::Tya, Mode::Imp, 2),
    (0x14, Op::Txs, Mode::Imp, 2),
    (0x15, Op::Tsx, Mode::Imp, 2),
    (0x16, Op::Inx, Mode::Imp, 2),
    (0x17, Op::Iny, Mode::Imp, 2),
    (0x18, Op::Dex, Mode::Imp, 2),
    (0x19, Op::Dey, Mode::Imp, 2),
    (0x1A, Op::Pha, Mode::Imp, 3),
    (0x1B, Op::Pla, Mode::Imp, 4),
    (0x1C, Op::Php, Mode::Imp, 3),
    (0x1D, Op::Plp, Mode::Imp, 4),
    (0x1E, Op::Clc, Mode::Imp, 2),
    (0x1F, Op::Sec, Mode::Imp, 2),
    (0x20, Op::Cli, Mode::Imp, 2),
    (0x21, Op::Sei, Mode::Imp, 2),
    (0x22, Op::Clv, Mode::Imp, 2),
];

/// (row base, op, bitmask of populated slots, slot-7 mode, cost kind)
const ROWS: [(u8, Op, u8, Mode, Kind); 21] = [
    (0x28, Op::Lda, 0b0111_1111, Mode::Imp, Kind::Read),
    (0x30, Op::Sta, 0b0111_1110, Mode::Imp, Kind::Read),
    (0x38, Op::Adc, 0b0111_1111, Mode::Imp, Kind::Read),
    (0x40, Op::Sbc, 0b0111_1111, Mode::Imp, Kind::Read),
    (0x48, Op::And, 0b0111_1111, Mode::Imp, Kind::Read),
    (0x50, Op::Ora, 0b0111_1111, Mode::Imp, Kind::Read),
    (0x58, Op::Eor, 0b0111_1111, Mode::Imp, Kind::Read),
    (0x60, Op::Cmp, 0b0111_1111, Mode::Imp, Kind::Read),
    (0x68, Op::Ldx, 0b1010_1011, Mode::Fpy, Kind::Read),
    (0x70, Op::Ldy, 0b0001_1111, Mode::Imp, Kind::Read),
    (0x78, Op::Stx, 0b1000_1010, Mode::Fpy, Kind::Read),
    (0x80, Op::Sty, 0b0000_1110, Mode::Imp, Kind::Read),
    (0x88, Op::Cpx, 0b0000_1011, Mode::Imp, Kind::Read),
    (0x90, Op::Cpy, 0b0000_1011, Mode::Imp, Kind::Read),
    (0x98, Op::Inc, 0b0000_1110, Mode::Imp, Kind::Rmw),
    (0xA0, Op::Dec, 0b0000_1110, Mode::Imp, Kind::Rmw),
    (0xA8, Op::Asl, 0b1000_1110, Mode::Acc, Kind::Rmw),
    (0xB0, Op::Lsr, 0b1000_1110, Mode::Acc, Kind::Rmw),
    (0xB8, Op::Rol, 0b1000_1110, Mode::Acc, Kind::Rmw),
    (0xC0, Op::Ror, 0b1000_1110, Mode::Acc, Kind::Rmw),
    (0xC8, Op::Bit, 0b0000_1010, Mode::Imp, Kind::Read),
];

const fn slot_mode(slot: u8, slot7: Mode) -> Mode {
    match slot {
        0 => Mode::Imm,
        1 => Mode::Fp,
        2 => Mode::Fpx,
        3 => Mode::Abs,
        4 => Mode::Absx,
        5 => Mode::Absy,
        6 => Mode::Indy,
        _ => slot7,
    }
}

const fn read_cycles(mode: Mode) -> u8 {
    match mode {
        Mode::Imm | Mode::Acc => 2,
        Mode::Fp => 3,
        Mode::Fpx | Mode::Fpy | Mode::Abs => 4,
        Mode::Absx | Mode::Absy => 5,
        Mode::Indy => 6,
        _ => 2,
    }
}

const fn build() -> [Option<OpInfo>; 256] {
    let mut t: [Option<OpInfo>; 256] = [None; 256];
    let mut i = 0;
    while i < FIXED.len() {
        let (code, op, mode, cycles) = FIXED[i];
        t[code as usize] = Some(OpInfo { op, mode, cycles });
        i += 1;
    }
    let mut r = 0;
    while r < ROWS.len() {
        let (base, op, mask, slot7, kind) = ROWS[r];
        let mut slot = 0;
        while slot < 8 {
            if mask & (1 << slot) != 0 {
                let mode = slot_mode(slot, slot7);
                let base_cycles = read_cycles(mode);
                let cycles = match (kind, mode) {
                    (Kind::Rmw, Mode::Acc) => 2,
                    (Kind::Rmw, _) => base_cycles + 2,
                    (Kind::Read, _) => base_cycles,
                };
                t[(base + slot) as usize] = Some(OpInfo { op, mode, cycles });
            }
            slot += 1;
        }
        r += 1;
    }
    t
}

/// Decoding table: `OPCODES[byte]` is `None` for opcodes the book doesn't describe. The CPU
/// jams on them, except the few in `UNDOCUMENTED`.
pub const OPCODES: [Option<OpInfo>; 256] = build();

/// Encoding lookup: the opcode byte for an operation in an addressing mode.
pub fn opcode_for(op: Op, mode: Mode) -> Option<u8> {
    (0..=255u8).find(|&b| matches!(OPCODES[b as usize], Some(i) if i.op == op && i.mode == mode))
}

pub fn modes_of(op: Op) -> Vec<Mode> {
    OPCODES.iter().flatten().filter(|i| i.op == op).map(|i| i.mode).collect()
}

/// The instructions the book leaves out. They really work, as the 6502's
/// "illegal" opcodes did: each is two documented operations in one, cheaper than the pair.
/// Nothing in the manual, the assembler, the language data or the editors mentions them; a
/// program uses them as bytes (`.byte $D0`, then the operand). The CPU looks here only when
/// `OPCODES` has nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Undoc {
    /// A and X both loaded from memory (LDA then TAX). N Z.
    Lax,
    /// A AND X stored. No flags.
    Sax,
    /// Memory decremented, then compared with A (DEC then CMP). N Z C.
    Dcp,
    /// Memory incremented, then subtracted from A with borrow (INC then SBC). N V Z C.
    Isb,
    /// Memory shifted left, then ORed into A (ASL then ORA). N Z C.
    Slo,
    /// AND immediate, with bit 7 of the result into C as well as N. N Z C.
    Anc,
    /// X to Y. N Z.
    Txy,
}

/// The undocumented opcodes, with their modes and cycles: `(opcode, op, mode, cycles)`.
pub const UNDOCUMENTED: [(u8, Undoc, Mode, u8); 8] = [
    (0xD0, Undoc::Lax, Mode::Abs, 4),
    (0xD1, Undoc::Lax, Mode::Fp, 3),
    (0xD2, Undoc::Sax, Mode::Fp, 3),
    (0xD3, Undoc::Dcp, Mode::Fp, 5),
    (0xD4, Undoc::Isb, Mode::Fp, 5),
    (0xD5, Undoc::Slo, Mode::Fp, 5),
    (0xD6, Undoc::Anc, Mode::Imm, 2),
    (0xD7, Undoc::Txy, Mode::Imp, 2),
];

/// The undocumented instruction at `opcode`, if there is one.
pub fn undocumented(opcode: u8) -> Option<(Undoc, Mode, u8)> {
    UNDOCUMENTED.iter().find(|u| u.0 == opcode).map(|&(_, op, mode, c)| (op, mode, c))
}
