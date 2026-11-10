//! The UNC-1 processor: 8-bit, 6502-flavoured, Rev A.
//!
//! Differences from the 6502 (deliberate): no decimal mode, no page-crossing penalties,
//! JSR pushes the real return address, JMP (ind) has no page-wrap bug, BRK is one byte,
//! HLT stops the processor, unassigned opcodes jam it.

use crate::isa::{Mode, OPCODES, Op};

pub trait Bus {
    fn read(&mut self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, value: u8);
}

pub mod flag {
    pub const C: u8 = 0x01;
    pub const Z: u8 = 0x02;
    pub const I: u8 = 0x04;
    pub const B: u8 = 0x10;
    pub const U: u8 = 0x20;
    pub const V: u8 = 0x40;
    pub const N: u8 = 0x80;
}

pub const VEC_NMI: u16 = 0xFFFA;
pub const VEC_RESET: u16 = 0xFFFC;
pub const VEC_IRQ: u16 = 0xFFFE;
pub const IRQ_CYCLES: u8 = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CpuState {
    Running,
    /// Stopped by HLT. A reset restarts it, and an NMI wakes it.
    Halted,
    /// Stopped by an unassigned opcode. PC points at the offending byte.
    Jammed(u8),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cpu {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub sp: u8,
    pub pc: u16,
    /// Status: N V 1 B - I Z C. B is never stored here, only pushed.
    pub p: u8,
    pub cycles: u64,
    pub state: CpuState,
    /// Level-triggered interrupt request line, driven by the machine's I/O.
    pub irq: bool,
    /// A non-maskable interrupt waiting to be taken: the RESTORE key was pressed (`nmi`).
    pub nmi: bool,
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

impl Cpu {
    pub fn new() -> Self {
        Cpu { a: 0, x: 0, y: 0, sp: 0xFF, pc: 0, p: flag::I | flag::U, cycles: 0, state: CpuState::Running, irq: false, nmi: false }
    }

    pub fn reset(&mut self, bus: &mut impl Bus) {
        *self = Cpu::new();
        self.pc = read16(bus, VEC_RESET);
    }

    pub fn flag(&self, f: u8) -> bool {
        self.p & f != 0
    }

    fn set(&mut self, f: u8, on: bool) {
        if on { self.p |= f } else { self.p &= !f }
    }

    fn set_nz(&mut self, v: u8) {
        self.set(flag::Z, v == 0);
        self.set(flag::N, v & 0x80 != 0);
    }

    fn fetch(&mut self, bus: &mut impl Bus) -> u8 {
        let v = bus.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        v
    }

    fn fetch16(&mut self, bus: &mut impl Bus) -> u16 {
        let lo = self.fetch(bus);
        let hi = self.fetch(bus);
        u16::from_le_bytes([lo, hi])
    }

    fn push(&mut self, bus: &mut impl Bus, v: u8) {
        bus.write(0x0100 | self.sp as u16, v);
        self.sp = self.sp.wrapping_sub(1);
    }

    fn pull(&mut self, bus: &mut impl Bus) -> u8 {
        self.sp = self.sp.wrapping_add(1);
        bus.read(0x0100 | self.sp as u16)
    }

    fn push16(&mut self, bus: &mut impl Bus, v: u16) {
        let [lo, hi] = v.to_le_bytes();
        self.push(bus, hi);
        self.push(bus, lo);
    }

    fn pull16(&mut self, bus: &mut impl Bus) -> u16 {
        let lo = self.pull(bus);
        let hi = self.pull(bus);
        u16::from_le_bytes([lo, hi])
    }

    /// Effective address for a memory mode. Fast-page indexing wraps inside the fast page;
    /// absolute indexing wraps at 64K.
    fn ea(&mut self, bus: &mut impl Bus, mode: Mode) -> u16 {
        match mode {
            Mode::Fp => self.fetch(bus) as u16,
            Mode::Fpx => self.fetch(bus).wrapping_add(self.x) as u16,
            Mode::Fpy => self.fetch(bus).wrapping_add(self.y) as u16,
            Mode::Abs => self.fetch16(bus),
            Mode::Absx => self.fetch16(bus).wrapping_add(self.x as u16),
            Mode::Absy => self.fetch16(bus).wrapping_add(self.y as u16),
            Mode::Indy => {
                let z = self.fetch(bus);
                let lo = bus.read(z as u16);
                let hi = bus.read(z.wrapping_add(1) as u16);
                u16::from_le_bytes([lo, hi]).wrapping_add(self.y as u16)
            }
            Mode::Ind => {
                let a = self.fetch16(bus);
                read16(bus, a)
            }
            Mode::Imp | Mode::Acc | Mode::Imm | Mode::Rel => unreachable!("{mode:?} has no address"),
        }
    }

    fn operand(&mut self, bus: &mut impl Bus, mode: Mode) -> u8 {
        if mode == Mode::Imm {
            self.fetch(bus)
        } else {
            let a = self.ea(bus, mode);
            bus.read(a)
        }
    }

    fn adc(&mut self, m: u8) {
        let sum = self.a as u16 + m as u16 + self.flag(flag::C) as u16;
        let r = sum as u8;
        self.set(flag::C, sum > 0xFF);
        self.set(flag::V, (!(self.a ^ m) & (self.a ^ r) & 0x80) != 0);
        self.a = r;
        self.set_nz(r);
    }

    fn compare(&mut self, reg: u8, m: u8) {
        self.set(flag::C, reg >= m);
        self.set_nz(reg.wrapping_sub(m));
    }

    fn shift(&mut self, op: Op, v: u8) -> u8 {
        let c = self.flag(flag::C) as u8;
        let (r, carry) = match op {
            Op::Asl => (v << 1, v & 0x80 != 0),
            Op::Lsr => (v >> 1, v & 1 != 0),
            Op::Rol => ((v << 1) | c, v & 0x80 != 0),
            Op::Ror => ((v >> 1) | (c << 7), v & 1 != 0),
            Op::Inc => (v.wrapping_add(1), self.flag(flag::C)),
            Op::Dec => (v.wrapping_sub(1), self.flag(flag::C)),
            _ => unreachable!(),
        };
        self.set(flag::C, carry);
        self.set_nz(r);
        r
    }

    fn interrupt(&mut self, bus: &mut impl Bus, vector: u16, brk: bool) {
        self.push16(bus, self.pc);
        let p = if brk { self.p | flag::B } else { self.p & !flag::B };
        self.push(bus, p | flag::U);
        self.set(flag::I, true);
        self.pc = read16(bus, vector);
    }

    /// The NMI line is pulled: edge-triggered, so once for each call. The interrupt is taken
    /// before the next instruction, whatever the I flag says, through the vector at $FFFA.
    /// It wakes a processor stopped by HLT, with the HLT itself as the return address, so an
    /// RTI halts it again. A jammed processor stays jammed: only a reset helps that.
    pub fn nmi(&mut self) {
        match self.state {
            CpuState::Running => self.nmi = true,
            CpuState::Halted => {
                self.pc = self.pc.wrapping_sub(1);
                self.state = CpuState::Running;
                self.nmi = true;
            }
            CpuState::Jammed(_) => {}
        }
    }

    /// Executes one instruction, or takes a pending NMI or IRQ. Returns the cycles spent;
    /// 0 when the CPU is halted or jammed.
    pub fn step(&mut self, bus: &mut impl Bus) -> u8 {
        if self.state != CpuState::Running {
            return 0;
        }
        if self.nmi {
            self.nmi = false;
            self.interrupt(bus, VEC_NMI, false);
            self.cycles += IRQ_CYCLES as u64;
            return IRQ_CYCLES;
        }
        if self.irq && !self.flag(flag::I) {
            self.interrupt(bus, VEC_IRQ, false);
            self.cycles += IRQ_CYCLES as u64;
            return IRQ_CYCLES;
        }
        let opcode = self.fetch(bus);
        let Some(info) = OPCODES[opcode as usize] else {
            if let Some((op, mode, cycles)) = crate::isa::undocumented(opcode) {
                self.undocumented(bus, op, mode);
                self.cycles += cycles as u64;
                return cycles;
            }
            self.pc = self.pc.wrapping_sub(1);
            self.state = CpuState::Jammed(opcode);
            return 0;
        };
        let mut cycles = info.cycles;
        let mode = info.mode;
        match info.op {
            Op::Lda => { self.a = self.operand(bus, mode); self.set_nz(self.a) }
            Op::Ldx => { self.x = self.operand(bus, mode); self.set_nz(self.x) }
            Op::Ldy => { self.y = self.operand(bus, mode); self.set_nz(self.y) }
            Op::Sta => { let a = self.ea(bus, mode); bus.write(a, self.a) }
            Op::Stx => { let a = self.ea(bus, mode); bus.write(a, self.x) }
            Op::Sty => { let a = self.ea(bus, mode); bus.write(a, self.y) }
            Op::Adc => { let m = self.operand(bus, mode); self.adc(m) }
            Op::Sbc => { let m = self.operand(bus, mode); self.adc(!m) }
            Op::And => { self.a &= self.operand(bus, mode); self.set_nz(self.a) }
            Op::Ora => { self.a |= self.operand(bus, mode); self.set_nz(self.a) }
            Op::Eor => { self.a ^= self.operand(bus, mode); self.set_nz(self.a) }
            Op::Cmp => { let m = self.operand(bus, mode); self.compare(self.a, m) }
            Op::Cpx => { let m = self.operand(bus, mode); self.compare(self.x, m) }
            Op::Cpy => { let m = self.operand(bus, mode); self.compare(self.y, m) }
            Op::Bit => {
                let m = self.operand(bus, mode);
                self.set(flag::Z, self.a & m == 0);
                self.set(flag::N, m & 0x80 != 0);
                self.set(flag::V, m & 0x40 != 0);
            }
            Op::Inc | Op::Dec | Op::Asl | Op::Lsr | Op::Rol | Op::Ror => {
                if mode == Mode::Acc {
                    self.a = self.shift(info.op, self.a);
                } else {
                    let a = self.ea(bus, mode);
                    let v = bus.read(a);
                    let r = self.shift(info.op, v);
                    bus.write(a, r);
                }
            }
            Op::Beq | Op::Bne | Op::Bcc | Op::Bcs | Op::Bmi | Op::Bpl | Op::Bvc | Op::Bvs => {
                let off = self.fetch(bus) as i8;
                let take = match info.op {
                    Op::Beq => self.flag(flag::Z),
                    Op::Bne => !self.flag(flag::Z),
                    Op::Bcs => self.flag(flag::C),
                    Op::Bcc => !self.flag(flag::C),
                    Op::Bmi => self.flag(flag::N),
                    Op::Bpl => !self.flag(flag::N),
                    Op::Bvs => self.flag(flag::V),
                    _ => !self.flag(flag::V),
                };
                if take {
                    self.pc = self.pc.wrapping_add(off as i16 as u16);
                    cycles += 1;
                }
            }
            Op::Jmp => self.pc = if mode == Mode::Abs { self.fetch16(bus) } else { self.ea(bus, mode) },
            Op::Jsr => { let t = self.fetch16(bus); self.push16(bus, self.pc); self.pc = t }
            Op::Rts => self.pc = self.pull16(bus),
            Op::Rti => {
                let p = self.pull(bus);
                self.p = (p & !flag::B) | flag::U;
                self.pc = self.pull16(bus);
            }
            Op::Brk => self.interrupt(bus, VEC_IRQ, true),
            Op::Nop => {}
            Op::Hlt => self.state = CpuState::Halted,
            Op::Tax => { self.x = self.a; self.set_nz(self.x) }
            Op::Tay => { self.y = self.a; self.set_nz(self.y) }
            Op::Txa => { self.a = self.x; self.set_nz(self.a) }
            Op::Tya => { self.a = self.y; self.set_nz(self.a) }
            Op::Txs => self.sp = self.x,
            Op::Tsx => { self.x = self.sp; self.set_nz(self.x) }
            Op::Inx => { self.x = self.x.wrapping_add(1); self.set_nz(self.x) }
            Op::Iny => { self.y = self.y.wrapping_add(1); self.set_nz(self.y) }
            Op::Dex => { self.x = self.x.wrapping_sub(1); self.set_nz(self.x) }
            Op::Dey => { self.y = self.y.wrapping_sub(1); self.set_nz(self.y) }
            Op::Pha => self.push(bus, self.a),
            Op::Pla => { self.a = self.pull(bus); self.set_nz(self.a) }
            Op::Php => self.push(bus, self.p | flag::B | flag::U),
            Op::Plp => { let p = self.pull(bus); self.p = (p & !flag::B) | flag::U }
            Op::Clc => self.set(flag::C, false),
            Op::Sec => self.set(flag::C, true),
            Op::Cli => self.set(flag::I, false),
            Op::Sei => self.set(flag::I, true),
            Op::Clv => self.set(flag::V, false),
        }
        self.cycles += cycles as u64;
        cycles
    }
}

impl Cpu {
    /// The instructions the book leaves out (`isa::UNDOCUMENTED`).
    fn undocumented(&mut self, bus: &mut impl Bus, op: crate::isa::Undoc, mode: Mode) {
        use crate::isa::Undoc;
        match op {
            Undoc::Lax => {
                self.a = self.operand(bus, mode);
                self.x = self.a;
                self.set_nz(self.a);
            }
            Undoc::Sax => {
                let a = self.ea(bus, mode);
                bus.write(a, self.a & self.x);
            }
            Undoc::Dcp | Undoc::Isb | Undoc::Slo => {
                let a = self.ea(bus, mode);
                let v = bus.read(a);
                let r = self.shift(
                    match op {
                        Undoc::Dcp => Op::Dec,
                        Undoc::Isb => Op::Inc,
                        _ => Op::Asl,
                    },
                    v,
                );
                bus.write(a, r);
                match op {
                    Undoc::Dcp => self.compare(self.a, r),
                    Undoc::Isb => self.adc(!r),
                    _ => {
                        self.a |= r;
                        self.set_nz(self.a);
                    }
                }
            }
            Undoc::Anc => {
                self.a &= self.operand(bus, mode);
                self.set_nz(self.a);
                self.set(flag::C, self.a & 0x80 != 0);
            }
            Undoc::Txy => {
                self.y = self.x;
                self.set_nz(self.y);
            }
        }
    }
}

pub fn read16(bus: &mut impl Bus, addr: u16) -> u16 {
    let lo = bus.read(addr);
    let hi = bus.read(addr.wrapping_add(1));
    u16::from_le_bytes([lo, hi])
}
