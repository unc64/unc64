//! The host's view of a running program (`Machine::observe`): I/O register traffic,
//! the reserved range, screen and bitmap writes. Invisible to programs.

use unc_core::isa::{Mode, Op, opcode_for};
use unc_core::machine::{CYCLES_PER_FRAME, Machine, RamSize, io};

/// Machine code, one instruction at a time: (op, mode, operand).
fn code(m: &mut Machine, at: u16, prog: &[(Op, Mode, u16)]) {
    let mut a = at;
    for &(op, mode, v) in prog {
        let o = opcode_for(op, mode).unwrap_or_else(|| panic!("{op:?} {mode:?}"));
        let bytes: Vec<u8> = match mode {
            Mode::Imp | Mode::Acc => vec![o],
            Mode::Abs | Mode::Absx | Mode::Absy | Mode::Ind => vec![o, v as u8, (v >> 8) as u8],
            _ => vec![o, v as u8],
        };
        m.load(a, &bytes);
        a += bytes.len() as u16;
    }
    m.cpu.pc = at;
}

fn halt(m: &mut Machine) {
    for _ in 0..10 {
        if m.run_frame() == unc_core::Stop::Halted {
            return;
        }
    }
    panic!("never halted");
}

fn type_line(m: &mut Machine, s: &str) {
    for k in s.bytes().chain([13]) {
        m.key(k);
        for _ in 0..200 {
            m.run_cycles(500);
            if m.peek(io::KBD) & 0x80 == 0 {
                break;
            }
        }
    }
}

#[test]
fn counts_a_basic_program_reading_the_keyboard_and_drawing() {
    let mut m = Machine::boot(RamSize::K16);
    type_line(&mut m, "10 POKE 1024+RND(40),PEEK(49170)+PEEK(49169):GOTO 10");
    type_line(&mut m, "RUN");
    m.observe();
    for _ in 0..50 {
        m.run_frame();
    }
    let o = m.observe();
    assert_eq!(o.frames, 50);
    assert!(o.read_count(io::HELD) > 100, "HELD: {}", o.read_count(io::HELD));
    assert_eq!(o.read_count(io::HELD), o.read_count(io::KBD_STROBE), "one of each per pass");
    // BASIC itself reads KBD before every statement, looking for STOP.
    assert!(o.read_count(io::KBD) >= o.read_count(io::HELD));
    // (the pass under way when the count is taken may have read and not yet drawn)
    assert!(o.screen_writes + 1 >= o.read_count(io::HELD), "{}", o.screen_writes);
    assert_eq!(o.bitmap_writes, 0, "nothing near the bitmap at $2000");
    assert!(!o.reserved_read);
    assert_eq!(o.write_count(io::BORDER), 0);
    // and the counts start again
    let o = m.observe();
    assert_eq!((o.frames, o.read_count(io::HELD), o.screen_writes), (0, 0, 0));
}

#[test]
fn the_host_peeking_and_poking_counts_for_nothing() {
    let mut m = Machine::boot(RamSize::K16);
    m.observe();
    for a in 0xC000..=0xC0FF {
        m.peek(a);
    }
    m.peek(0xC3F0);
    m.poke(0x0400, 1);
    m.poke(0x2000, 1);
    m.poke(io::BORDER, 2);
    m.load(0x0400, &[1; 1000]);
    let o = m.observe();
    assert_eq!(o.reads.iter().sum::<u32>(), 0);
    assert_eq!(o.writes.iter().sum::<u32>(), 0);
    assert!(!o.reserved_read);
    assert_eq!((o.screen_writes, o.bitmap_writes), (0, 0));
    assert_eq!(m.peek(io::BORDER), 2, "the poke still did what it does");
}

#[test]
fn counts_machine_code_register_traffic_exactly() {
    let mut m = Machine::boot(RamSize::K16);
    use Mode::*;
    use Op::*;
    code(&mut m, 0x3000, &[
        (Lda, Abs, 0xC006),  // FRAME
        (Lda, Abs, 0xC006),
        (Sta, Abs, 0xC020),  // BORDER
        (Ldx, Imm, 3),
        (Lda, Absx, 0xC060), // KEYS+3
        (Inc, Abs, 0xC020),  // a read and a write
        (Lda, Abs, 0xC3F1),  // the reserved range
        (Lda, Abs, 0xC400),  // outside $C000-$C0FF: not counted
        (Sta, Abs, 0x0400),  // the screen, first cell
        (Sta, Abs, 0x07E7),  // and last
        (Sta, Abs, 0x07E8),  // not the screen
        (Sta, Abs, 0x2000),  // the bitmap at reset: $2000-$3F3F
        (Sta, Abs, 0x3F3F),
        (Sta, Abs, 0x3F40),  // one past
        (Lda, Imm, 0x30),
        (Sta, Abs, 0xC023),  // BITMAP = $30: $3000-$4F3F
        (Sta, Abs, 0x2000),  // no longer the bitmap
        (Sta, Abs, 0x3000),
        (Hlt, Imp, 0),
    ]);
    m.observe();
    halt(&mut m);
    let o = m.observe();
    assert_eq!(o.read_count(io::FRAME), 2);
    assert_eq!(o.read_count(io::KEYS + 3), 1);
    assert_eq!(o.read_count(io::BORDER), 1);
    assert_eq!(o.write_count(io::BORDER), 2);
    assert_eq!(o.write_count(io::BITMAP), 1);
    assert_eq!(o.reads.iter().sum::<u32>(), 4);
    assert_eq!(o.writes.iter().sum::<u32>(), 3);
    assert!(o.reserved_read);
    assert_eq!(o.screen_writes, 2);
    assert_eq!(o.bitmap_writes, 3);
}

#[test]
fn a_basic_peek_of_the_reserved_range_is_seen() {
    let mut m = Machine::boot(RamSize::K16);
    type_line(&mut m, "PRINT PEEK(50160),PEEK(50161)");
    for _ in 0..5 {
        m.run_frame();
    }
    assert!(m.screen_text().contains("19        86"), "{}", m.screen_text());
    assert!(m.observe().reserved_read);
    assert!(!m.observe().reserved_read, "cleared by the last observe");
}

#[test]
fn frames_count_whole_frames_and_survive_a_reset() {
    let mut m = Machine::boot(RamSize::K16);
    m.observe();
    m.run_cycles(CYCLES_PER_FRAME * 3 + 100);
    m.reset();
    m.run_frame();
    assert_eq!(m.observe().frames, 4);
}

#[test]
fn observing_changes_nothing_the_machine_does() {
    let prog = "10 POKE 1024+RND(1000),PEEK(49168):POKE 49184,PEEK(50160):GOTO 10";
    let mut a = Machine::boot(RamSize::K16);
    let mut b = Machine::boot(RamSize::K16);
    for m in [&mut a, &mut b] {
        type_line(m, prog);
        type_line(m, "RUN");
    }
    for i in 0..200 {
        a.run_frame();
        b.run_frame();
        a.observe();
        if i % 7 == 0 {
            b.key(b'A' + (i % 26) as u8);
            a.key(b'A' + (i % 26) as u8);
        }
    }
    assert_eq!(a.save_state(), b.save_state());
}

/// Not a test: how fast the machine runs a busy BASIC program, to keep `observe`'s
/// bookkeeping honest. `cargo test --release -p unc-core --test observe -- --ignored --nocapture`
#[test]
#[ignore]
fn bench_busy_machine() {
    let mut best = f64::MAX;
    for _ in 0..5 {
        let mut m = Machine::boot(RamSize::K16);
        type_line(&mut m, "10 POKE 1024+RND(1000),PEEK(49168)+42:POKE 49184,PEEK(49158):GOTO 10");
        type_line(&mut m, "RUN");
        let t = std::time::Instant::now();
        for _ in 0..5000 {
            m.run_frame();
        }
        best = best.min(t.elapsed().as_secs_f64());
    }
    println!("5000 frames: {:.3} s, {:.1} MHz", best, 5000.0 * 19968.0 / best / 1e6);
}
