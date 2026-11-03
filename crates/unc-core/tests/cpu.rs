use unc_core::cpu::{Bus, Cpu, CpuState, VEC_IRQ, VEC_RESET, flag};
use unc_core::isa::{Mode, OPCODES, Op, opcode_for};

struct Flat(Box<[u8; 0x10000]>);

impl Flat {
    fn new() -> Self {
        Flat(Box::new([0; 0x10000]))
    }
    fn load(&mut self, addr: u16, bytes: &[u8]) {
        for (i, &b) in bytes.iter().enumerate() {
            self.0[addr as usize + i] = b;
        }
    }
    fn word(&mut self, addr: u16, v: u16) {
        self.load(addr, &v.to_le_bytes());
    }
}

impl Bus for Flat {
    fn read(&mut self, addr: u16) -> u8 {
        self.0[addr as usize]
    }
    fn write(&mut self, addr: u16, value: u8) {
        self.0[addr as usize] = value;
    }
}

const ORG: u16 = 0x0200;

fn op(o: Op, m: Mode) -> u8 {
    opcode_for(o, m).unwrap_or_else(|| panic!("{o:?} has no {m:?}"))
}

fn cpu_at(pc: u16) -> Cpu {
    let mut c = Cpu::new();
    c.pc = pc;
    c
}

/// Runs one instruction at ORG after `setup`. Returns the CPU, memory and cycles.
fn run1(code: &[u8], setup: impl FnOnce(&mut Cpu, &mut Flat)) -> (Cpu, Flat, u8) {
    let mut bus = Flat::new();
    bus.load(ORG, code);
    let mut cpu = cpu_at(ORG);
    setup(&mut cpu, &mut bus);
    let c = cpu.step(&mut bus);
    (cpu, bus, c)
}

fn nz(v: u8) -> u8 {
    (if v == 0 { flag::Z } else { 0 }) | (v & 0x80)
}

// ---------- the table ----------

#[test]
fn every_opcode_costs_its_table_cycles_and_advances_pc() {
    for code in 0..=255u8 {
        let Some(info) = OPCODES[code as usize] else { continue };
        if matches!(info.op, Op::Jmp | Op::Jsr | Op::Rts | Op::Rti | Op::Brk | Op::Hlt) || info.op.is_branch() {
            continue;
        }
        let (cpu, _, cycles) = run1(&[code, 0x10, 0x20], |_, _| {});
        assert_eq!(cycles, info.cycles, "{:?} {:?} (${code:02X})", info.op, info.mode);
        assert_eq!(cpu.pc, ORG + info.mode.len() as u16, "{:?} {:?}", info.op, info.mode);
        assert_eq!(cpu.cycles, cycles as u64);
    }
}

#[test]
fn control_flow_opcodes_cost_their_table_cycles() {
    for (o, m) in [(Op::Jmp, Mode::Abs), (Op::Jmp, Mode::Ind), (Op::Jsr, Mode::Abs), (Op::Rts, Mode::Imp), (Op::Rti, Mode::Imp), (Op::Brk, Mode::Imp), (Op::Hlt, Mode::Imp)] {
        let (_, _, cycles) = run1(&[op(o, m), 0x00, 0x30], |c, _| c.sp = 0xF0);
        assert_eq!(cycles, OPCODES[op(o, m) as usize].unwrap().cycles, "{o:?} {m:?}");
    }
}

#[test]
fn cycle_costs_are_between_two_and_six() {
    for info in OPCODES.iter().flatten() {
        assert!((2..=6).contains(&info.cycles), "{:?} {:?} costs {}", info.op, info.mode, info.cycles);
    }
}

#[test]
fn unassigned_opcodes_jam_without_moving_pc() {
    for code in 0..=255u8 {
        if OPCODES[code as usize].is_some() || unc_core::isa::undocumented(code).is_some() {
            continue;
        }
        let (cpu, mut bus, cycles) = run1(&[code], |_, _| {});
        assert_eq!(cpu.state, CpuState::Jammed(code));
        assert_eq!((cpu.pc, cycles), (ORG, 0));
        let mut cpu = cpu;
        assert_eq!(cpu.step(&mut bus), 0, "stays jammed");
    }
}

// ---------- arithmetic, exhaustive ----------

#[test]
fn adc_matches_reference_for_every_input() {
    for a in 0..=255u16 {
        for m in 0..=255u16 {
            for c in 0..=1u16 {
                let (cpu, _, _) = run1(&[op(Op::Adc, Mode::Imm), m as u8], |cpu, _| {
                    cpu.a = a as u8;
                    cpu.p = flag::U | if c == 1 { flag::C } else { 0 };
                });
                let sum = a + m + c;
                let r = sum as u8;
                let signed = (a as u8 as i8) as i16 + (m as u8 as i8) as i16 + c as i16;
                assert_eq!(cpu.a, r);
                assert_eq!(cpu.flag(flag::C), sum > 255, "C {a} {m} {c}");
                assert_eq!(cpu.flag(flag::V), !(-128..=127).contains(&signed), "V {a} {m} {c}");
                assert_eq!(cpu.p & (flag::N | flag::Z), nz(r));
            }
        }
    }
}

#[test]
fn sbc_matches_reference_for_every_input() {
    for a in 0..=255i16 {
        for m in 0..=255i16 {
            for c in 0..=1i16 {
                let (cpu, _, _) = run1(&[op(Op::Sbc, Mode::Imm), m as u8], |cpu, _| {
                    cpu.a = a as u8;
                    cpu.p = flag::U | if c == 1 { flag::C } else { 0 };
                });
                let diff = a - m - (1 - c);
                let signed = (a as u8 as i8) as i16 - (m as u8 as i8) as i16 - (1 - c);
                assert_eq!(cpu.a, diff as u8);
                assert_eq!(cpu.flag(flag::C), diff >= 0, "C {a} {m} {c}");
                assert_eq!(cpu.flag(flag::V), !(-128..=127).contains(&signed), "V {a} {m} {c}");
                assert_eq!(cpu.p & (flag::N | flag::Z), nz(diff as u8));
            }
        }
    }
}

#[test]
fn compares_match_reference_for_every_input() {
    for (o, which) in [(Op::Cmp, 0), (Op::Cpx, 1), (Op::Cpy, 2)] {
        for r in 0..=255u8 {
            for m in 0..=255u8 {
                let (cpu, _, _) = run1(&[op(o, Mode::Imm), m], |cpu, _| match which {
                    0 => cpu.a = r,
                    1 => cpu.x = r,
                    _ => cpu.y = r,
                });
                assert_eq!(cpu.flag(flag::C), r >= m);
                assert_eq!(cpu.p & (flag::N | flag::Z), nz(r.wrapping_sub(m)));
            }
        }
    }
}

#[test]
fn logic_ops_match_reference_for_every_input() {
    for (o, f) in [(Op::And, (|a, m| a & m) as fn(u8, u8) -> u8), (Op::Ora, |a, m| a | m), (Op::Eor, |a, m| a ^ m)] {
        for a in 0..=255u8 {
            for m in 0..=255u8 {
                let (cpu, _, _) = run1(&[op(o, Mode::Imm), m], |cpu, _| cpu.a = a);
                assert_eq!(cpu.a, f(a, m));
                assert_eq!(cpu.p & (flag::N | flag::Z), nz(f(a, m)));
            }
        }
    }
}

#[test]
fn bit_matches_reference_for_every_input() {
    for a in 0..=255u8 {
        for m in 0..=255u8 {
            let (cpu, _, _) = run1(&[op(Op::Bit, Mode::Fp), 0x80], |cpu, bus| {
                cpu.a = a;
                bus.0[0x80] = m;
            });
            assert_eq!(cpu.flag(flag::Z), a & m == 0);
            assert_eq!(cpu.flag(flag::N), m & 0x80 != 0);
            assert_eq!(cpu.flag(flag::V), m & 0x40 != 0);
            assert_eq!(cpu.a, a, "BIT leaves A alone");
        }
    }
}

#[test]
fn shifts_and_rotates_match_reference_on_a_and_memory() {
    type F = fn(u8, bool) -> (u8, bool);
    let cases: [(Op, F); 4] = [
        (Op::Asl, |v, _| (v << 1, v & 0x80 != 0)),
        (Op::Lsr, |v, _| (v >> 1, v & 1 != 0)),
        (Op::Rol, |v, c| ((v << 1) | c as u8, v & 0x80 != 0)),
        (Op::Ror, |v, c| ((v >> 1) | ((c as u8) << 7), v & 1 != 0)),
    ];
    for (o, f) in cases {
        for v in 0..=255u8 {
            for c in [false, true] {
                let (want, wc) = f(v, c);
                let carry = if c { flag::C } else { 0 };
                let (cpu, _, _) = run1(&[op(o, Mode::Acc)], |cpu, _| { cpu.a = v; cpu.p = flag::U | carry });
                assert_eq!((cpu.a, cpu.flag(flag::C)), (want, wc), "{o:?} A {v} {c}");
                assert_eq!(cpu.p & (flag::N | flag::Z), nz(want));
                let (_, bus, _) = run1(&[op(o, Mode::Abs), 0x00, 0x30], |cpu, bus| { bus.0[0x3000] = v; cpu.p = flag::U | carry });
                assert_eq!(bus.0[0x3000], want, "{o:?} mem {v} {c}");
            }
        }
    }
}

#[test]
fn inc_and_dec_wrap_and_leave_carry_alone() {
    for v in 0..=255u8 {
        for c in [0, flag::C] {
            let (cpu, bus, _) = run1(&[op(Op::Inc, Mode::Fp), 0x40], |cpu, bus| { bus.0[0x40] = v; cpu.p = flag::U | c });
            assert_eq!(bus.0[0x40], v.wrapping_add(1));
            assert_eq!(cpu.p & flag::C, c);
            assert_eq!(cpu.p & (flag::N | flag::Z), nz(v.wrapping_add(1)));
            let (cpu, bus, _) = run1(&[op(Op::Dec, Mode::Fp), 0x40], |cpu, bus| { bus.0[0x40] = v; cpu.p = flag::U | c });
            assert_eq!(bus.0[0x40], v.wrapping_sub(1));
            assert_eq!(cpu.p & flag::C, c);
        }
    }
}

#[test]
fn register_increments_and_transfers_set_n_and_z() {
    for v in 0..=255u8 {
        let check = |o: Op, set: fn(&mut Cpu, u8), get: fn(&Cpu) -> u8, want: u8| {
            let (cpu, _, _) = run1(&[op(o, Mode::Imp)], |cpu, _| set(cpu, v));
            assert_eq!(get(&cpu), want, "{o:?} {v}");
            assert_eq!(cpu.p & (flag::N | flag::Z), nz(want), "{o:?} flags {v}");
        };
        check(Op::Inx, |c, v| c.x = v, |c| c.x, v.wrapping_add(1));
        check(Op::Iny, |c, v| c.y = v, |c| c.y, v.wrapping_add(1));
        check(Op::Dex, |c, v| c.x = v, |c| c.x, v.wrapping_sub(1));
        check(Op::Dey, |c, v| c.y = v, |c| c.y, v.wrapping_sub(1));
        check(Op::Tax, |c, v| c.a = v, |c| c.x, v);
        check(Op::Tay, |c, v| c.a = v, |c| c.y, v);
        check(Op::Txa, |c, v| c.x = v, |c| c.a, v);
        check(Op::Tya, |c, v| c.y = v, |c| c.a, v);
        check(Op::Tsx, |c, v| c.sp = v, |c| c.x, v);
    }
    let (cpu, _, _) = run1(&[op(Op::Txs, Mode::Imp)], |cpu, _| { cpu.x = 0; cpu.p = flag::U });
    assert_eq!(cpu.sp, 0);
    assert!(!cpu.flag(flag::Z), "TXS sets no flags");
}

#[test]
fn loads_set_n_and_z_for_every_value() {
    for v in 0..=255u8 {
        for o in [Op::Lda, Op::Ldx, Op::Ldy] {
            let (cpu, _, _) = run1(&[op(o, Mode::Imm), v], |_, _| {});
            let got = match o { Op::Lda => cpu.a, Op::Ldx => cpu.x, _ => cpu.y };
            assert_eq!(got, v);
            assert_eq!(cpu.p & (flag::N | flag::Z), nz(v));
        }
    }
}

// ---------- addressing ----------

#[test]
fn lda_reads_from_every_addressing_mode() {
    let cases: &[(Mode, &[u8], u16)] = &[
        (Mode::Imm, &[0x42], 0),
        (Mode::Fp, &[0x80], 0x0080),
        (Mode::Fpx, &[0x80], 0x0085),
        (Mode::Abs, &[0x34, 0x12], 0x1234),
        (Mode::Absx, &[0x34, 0x12], 0x1239),
        (Mode::Absy, &[0x34, 0x12], 0x1237),
        (Mode::Indy, &[0x70], 0x3003),
    ];
    for &(mode, operand, addr) in cases {
        let mut code = vec![op(Op::Lda, mode)];
        code.extend_from_slice(operand);
        let (cpu, _, _) = run1(&code, |cpu, bus| {
            cpu.x = 5;
            cpu.y = 3;
            bus.word(0x70, 0x3000);
            if addr != 0 {
                bus.0[addr as usize] = 0x42;
            }
        });
        assert_eq!(cpu.a, 0x42, "{mode:?}");
    }
}

#[test]
fn stores_write_through_every_addressing_mode() {
    let cases: &[(Op, Mode, &[u8], u16)] = &[
        (Op::Sta, Mode::Fp, &[0x80], 0x0080),
        (Op::Sta, Mode::Fpx, &[0x80], 0x0085),
        (Op::Sta, Mode::Abs, &[0x34, 0x12], 0x1234),
        (Op::Sta, Mode::Absx, &[0x34, 0x12], 0x1239),
        (Op::Sta, Mode::Absy, &[0x34, 0x12], 0x1237),
        (Op::Sta, Mode::Indy, &[0x70], 0x3003),
        (Op::Stx, Mode::Fp, &[0x80], 0x0080),
        (Op::Stx, Mode::Fpy, &[0x80], 0x0083),
        (Op::Stx, Mode::Abs, &[0x34, 0x12], 0x1234),
        (Op::Sty, Mode::Fp, &[0x80], 0x0080),
        (Op::Sty, Mode::Fpx, &[0x80], 0x0085),
        (Op::Sty, Mode::Abs, &[0x34, 0x12], 0x1234),
    ];
    for &(o, mode, operand, addr) in cases {
        let mut code = vec![op(o, mode)];
        code.extend_from_slice(operand);
        let (_, bus, _) = run1(&code, |cpu, bus| {
            (cpu.a, cpu.x, cpu.y) = (0x42, 5, 3);
            match o {
                Op::Stx => cpu.x = 0x42,
                Op::Sty => cpu.y = 0x42,
                _ => {}
            }
            bus.word(0x70, 0x3000);
        });
        assert_eq!(bus.0[addr as usize], 0x42, "{o:?} {mode:?}");
    }
}

#[test]
fn fast_page_indexing_wraps_inside_the_fast_page() {
    let (cpu, _, _) = run1(&[op(Op::Lda, Mode::Fpx), 0xF0], |cpu, bus| { cpu.x = 0x20; bus.0[0x0010] = 7; bus.0[0x0110] = 9 });
    assert_eq!(cpu.a, 7);
    let (cpu, _, _) = run1(&[op(Op::Ldx, Mode::Fpy), 0xFF], |cpu, bus| { cpu.y = 2; bus.0[0x0001] = 7 });
    assert_eq!(cpu.x, 7);
}

#[test]
fn indirect_pointer_at_ff_wraps_to_00_and_index_carries_into_high_byte() {
    let (cpu, _, _) = run1(&[op(Op::Lda, Mode::Indy), 0xFF], |cpu, bus| {
        bus.0[0xFF] = 0xF0;
        bus.0[0x00] = 0x30;
        cpu.y = 0x20;
        bus.0[0x3110] = 0x5A;
    });
    assert_eq!(cpu.a, 0x5A);
}

#[test]
fn absolute_indexing_wraps_at_64k() {
    let (cpu, _, _) = run1(&[op(Op::Lda, Mode::Absx), 0xFF, 0xFF], |cpu, bus| { cpu.x = 2; bus.0[0x0001] = 0x33 });
    assert_eq!(cpu.a, 0x33);
}

#[test]
fn jmp_indirect_has_no_page_wrap_bug() {
    let (cpu, _, _) = run1(&[op(Op::Jmp, Mode::Ind), 0xFF, 0x30], |_, bus| {
        bus.0[0x30FF] = 0x34;
        bus.0[0x3100] = 0x12;
        bus.0[0x3000] = 0x99;
    });
    assert_eq!(cpu.pc, 0x1234);
}

#[test]
fn read_modify_write_works_in_every_mode() {
    for mode in [Mode::Fp, Mode::Fpx, Mode::Abs] {
        let operand: &[u8] = if mode == Mode::Abs { &[0x00, 0x30] } else { &[0x40] };
        let addr = match mode { Mode::Fp => 0x40, Mode::Fpx => 0x42, _ => 0x3000 };
        let mut code = vec![op(Op::Inc, mode)];
        code.extend_from_slice(operand);
        let (_, bus, _) = run1(&code, |cpu, bus| { cpu.x = 2; bus.0[addr] = 9 });
        assert_eq!(bus.0[addr], 10, "{mode:?}");
    }
}

// ---------- branches ----------

#[test]
fn every_branch_costs_one_more_when_taken() {
    let cases = [
        (Op::Beq, flag::Z, true), (Op::Bne, flag::Z, false), (Op::Bcs, flag::C, true), (Op::Bcc, flag::C, false),
        (Op::Bmi, flag::N, true), (Op::Bpl, flag::N, false), (Op::Bvs, flag::V, true), (Op::Bvc, flag::V, false),
    ];
    for (o, f, when_set) in cases {
        for set in [false, true] {
            let (cpu, _, cycles) = run1(&[op(o, Mode::Rel), 0x10], |cpu, _| cpu.p = flag::U | if set { f } else { 0 });
            let taken = set == when_set;
            assert_eq!(cycles, if taken { 3 } else { 2 }, "{o:?} set={set}");
            assert_eq!(cpu.pc, if taken { ORG + 2 + 0x10 } else { ORG + 2 });
        }
    }
}

#[test]
fn branches_go_backwards_and_wrap_around_memory() {
    let (cpu, _, _) = run1(&[op(Op::Bne, Mode::Rel), 0xFC], |cpu, _| cpu.p = flag::U);
    assert_eq!(cpu.pc, ORG + 2 - 4);
    let mut bus = Flat::new();
    bus.load(0xFFF0, &[op(Op::Bne, Mode::Rel), 0x20]);
    let mut cpu = cpu_at(0xFFF0);
    cpu.p = flag::U;
    cpu.step(&mut bus);
    assert_eq!(cpu.pc, 0x0012);
}

// ---------- stack, subroutines, interrupts ----------

#[test]
fn push_and_pull_round_trip_and_wrap_the_stack() {
    let mut bus = Flat::new();
    bus.load(ORG, &[op(Op::Pha, Mode::Imp), op(Op::Lda, Mode::Imm), 0, op(Op::Pla, Mode::Imp)]);
    let mut cpu = cpu_at(ORG);
    cpu.a = 0x99;
    cpu.sp = 0x00;
    cpu.step(&mut bus);
    assert_eq!((bus.0[0x0100], cpu.sp), (0x99, 0xFF), "push at $0100 wraps SP to $FF");
    cpu.step(&mut bus);
    cpu.step(&mut bus);
    assert_eq!((cpu.a, cpu.sp), (0x99, 0x00));
    assert!(cpu.flag(flag::N));
}

#[test]
fn php_pushes_b_and_u_and_plp_ignores_b() {
    let (cpu, bus, _) = run1(&[op(Op::Php, Mode::Imp)], |cpu, _| cpu.p = flag::U | flag::C);
    assert_eq!(bus.0[0x01FF], flag::C | flag::B | flag::U);
    assert_eq!(cpu.sp, 0xFE);
    let (cpu, _, _) = run1(&[op(Op::Plp, Mode::Imp)], |cpu, bus| { cpu.sp = 0xFE; bus.0[0x01FF] = 0xFF });
    assert_eq!(cpu.p, !flag::B);
}

#[test]
fn jsr_pushes_the_real_return_address_and_rts_returns_to_it() {
    let mut bus = Flat::new();
    bus.load(ORG, &[op(Op::Jsr, Mode::Abs), 0x00, 0x30, op(Op::Hlt, Mode::Imp)]);
    bus.load(0x3000, &[op(Op::Rts, Mode::Imp)]);
    let mut cpu = cpu_at(ORG);
    cpu.step(&mut bus);
    assert_eq!(cpu.pc, 0x3000);
    assert_eq!((bus.0[0x01FF], bus.0[0x01FE]), (0x02, 0x03), "return address $0203 pushed high byte first");
    cpu.step(&mut bus);
    assert_eq!((cpu.pc, cpu.sp), (ORG + 3, 0xFF));
}

#[test]
fn brk_pushes_pc_and_status_with_b_and_rti_restores_both() {
    let mut bus = Flat::new();
    bus.word(VEC_IRQ, 0x4000);
    bus.load(ORG, &[op(Op::Brk, Mode::Imp)]);
    bus.load(0x4000, &[op(Op::Rti, Mode::Imp)]);
    let mut cpu = cpu_at(ORG);
    cpu.p = flag::U | flag::C;
    cpu.step(&mut bus);
    assert_eq!(cpu.pc, 0x4000);
    assert!(cpu.flag(flag::I));
    assert_eq!(bus.0[0x01FD], flag::C | flag::B | flag::U);
    assert_eq!(u16::from_le_bytes([bus.0[0x01FE], bus.0[0x01FF]]), ORG + 1);
    cpu.step(&mut bus);
    assert_eq!(cpu.pc, ORG + 1);
    assert_eq!(cpu.p, flag::U | flag::C, "RTI restores status, B never stored");
}

#[test]
fn irq_is_taken_only_when_interrupts_are_enabled() {
    let mut bus = Flat::new();
    bus.word(VEC_IRQ, 0x4000);
    bus.load(ORG, &[op(Op::Nop, Mode::Imp), op(Op::Nop, Mode::Imp)]);
    let mut cpu = cpu_at(ORG);
    cpu.irq = true;
    cpu.p = flag::U | flag::I;
    assert_eq!(cpu.step(&mut bus), 2, "masked: NOP runs");
    assert_eq!(cpu.pc, ORG + 1);
    cpu.p = flag::U;
    assert_eq!(cpu.step(&mut bus), 6);
    assert_eq!(cpu.pc, 0x4000);
    assert!(cpu.flag(flag::I));
    assert_eq!(bus.0[0x01FD] & flag::B, 0, "hardware IRQ pushes B clear");
    assert_eq!(u16::from_le_bytes([bus.0[0x01FE], bus.0[0x01FF]]), ORG + 1);
}

#[test]
fn hlt_stops_the_cpu_until_reset() {
    let mut bus = Flat::new();
    bus.word(VEC_RESET, 0x0300);
    bus.load(ORG, &[op(Op::Hlt, Mode::Imp)]);
    let mut cpu = cpu_at(ORG);
    assert_eq!(cpu.step(&mut bus), 2);
    assert_eq!(cpu.state, CpuState::Halted);
    assert_eq!(cpu.step(&mut bus), 0);
    cpu.reset(&mut bus);
    assert_eq!((cpu.pc, cpu.sp, cpu.state), (0x0300, 0xFF, CpuState::Running));
    assert!(cpu.flag(flag::I));
}

#[test]
fn flag_instructions_set_and_clear() {
    for (o, f, on) in [(Op::Sec, flag::C, true), (Op::Clc, flag::C, false), (Op::Sei, flag::I, true), (Op::Cli, flag::I, false), (Op::Clv, flag::V, false)] {
        let (cpu, _, _) = run1(&[op(o, Mode::Imp)], |cpu, _| cpu.p = flag::U | if on { 0 } else { f });
        assert_eq!(cpu.flag(f), on, "{o:?}");
    }
}

#[test]
fn execution_is_deterministic() {
    let prog = [
        op(Op::Ldx, Mode::Imm), 0, op(Op::Txa, Mode::Imp), op(Op::Adc, Mode::Fpx), 0x10, op(Op::Sta, Mode::Absx), 0x00, 0x30,
        op(Op::Inx, Mode::Imp), op(Op::Bne, Mode::Rel), 0xF7, op(Op::Hlt, Mode::Imp),
    ];
    let run = || {
        let mut bus = Flat::new();
        bus.load(ORG, &prog);
        let mut cpu = cpu_at(ORG);
        while cpu.step(&mut bus) > 0 {}
        (cpu, bus.0[0x3000..0x3100].to_vec())
    };
    let (a, ma) = run();
    let (b, mb) = run();
    assert_eq!(a, b);
    assert_eq!(ma, mb);
    assert_eq!(a.state, CpuState::Halted);
}

// ---------- the instructions the book leaves out ($D0-$D7) ----------

#[test]
fn eight_undocumented_opcodes_work_and_the_rest_still_jam() {
    // LAX abs, LAX fp: A and X both, flags from the value
    let (c, _, cy) = run1(&[0xD0, 0x00, 0x30], |_, b| b.0[0x3000] = 0x80);
    assert_eq!((c.a, c.x, c.p & (flag::N | flag::Z), cy, c.pc), (0x80, 0x80, flag::N, 4, ORG + 3));
    let (c, _, cy) = run1(&[0xD1, 0x10], |_, b| b.0[0x10] = 0);
    assert_eq!((c.a, c.x, c.p & flag::Z, cy), (0, 0, flag::Z, 3));
    // SAX fp: A AND X stored, no flags
    let (c, b, cy) = run1(&[0xD2, 0x20], |c, _| { c.a = 0xF0; c.x = 0x3C; c.p = 0x24; });
    assert_eq!((b.0[0x20], c.p, cy), (0x30, 0x24, 3));
    // DCP fp: DEC then CMP
    let (c, b, cy) = run1(&[0xD3, 0x20], |c, b| { c.a = 5; b.0[0x20] = 6; });
    assert_eq!((b.0[0x20], c.p & (flag::Z | flag::C), cy), (5, flag::Z | flag::C, 5), "6-1 = 5 = A");
    // ISB fp: INC then SBC
    let (c, b, cy) = run1(&[0xD4, 0x20], |c, b| { c.a = 10; c.p |= flag::C; b.0[0x20] = 2; });
    assert_eq!((b.0[0x20], c.a, cy), (3, 7, 5));
    // SLO fp: ASL then ORA; C from the shift
    let (c, b, cy) = run1(&[0xD5, 0x20], |c, b| { c.a = 0x01; b.0[0x20] = 0x81; });
    assert_eq!((b.0[0x20], c.a, c.p & flag::C, cy), (0x02, 0x03, flag::C, 5));
    // ANC #: AND, and bit 7 into C
    let (c, _, cy) = run1(&[0xD6, 0xC0], |c, _| c.a = 0x81);
    assert_eq!((c.a, c.p & (flag::C | flag::N), cy), (0x80, flag::C | flag::N, 2));
    // TXY
    let (c, _, cy) = run1(&[0xD7], |c, _| c.x = 0x42);
    assert_eq!((c.y, cy), (0x42, 2));
    // each is cheaper than the documented pair it does the work of
    let cyc = |o, m| OPCODES[op(o, m) as usize].unwrap().cycles;
    assert!(4 < cyc(Op::Lda, Mode::Abs) + cyc(Op::Tax, Mode::Imp));
    assert!(5 < cyc(Op::Dec, Mode::Fp) + cyc(Op::Cmp, Mode::Fp));
    assert!(2 < cyc(Op::Txa, Mode::Imp) + cyc(Op::Tay, Mode::Imp));
    // past them, the processor jams as ever
    for code in 0xD8..=0xFFu8 {
        let (c, _, _) = run1(&[code], |_, _| {});
        assert_eq!(c.state, CpuState::Jammed(code));
    }
}
