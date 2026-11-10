//! The host's frame meter (`Machine::frame_work`, the HOLD box's needle): the cycles a frame
//! spent working before its program started waiting for the next one.
use unc_core::{Machine, RamSize};

fn run(src: &str, frames: u32) -> Machine {
    let asm = unc_asm::assemble(src).unwrap_or_else(|e| panic!("{e:?}"));
    let mut m = Machine::boot(RamSize::K16);
    unc_asm::load(&mut m, &asm);
    for _ in 0..frames {
        m.run_frame();
    }
    m
}

#[test]
fn a_game_loop_shows_its_work() {
    // wait for the frame, then burn about 5,000 cycles (a 5-cycle loop, 1,000 times)
    let src = "FRAME = $C006\n.org $0800\nstart: LDA FRAME\nwait: CMP FRAME\n BEQ wait\n LDY #4\nouter: LDX #250\ninner: DEX\n BNE inner\n DEY\n BNE outer\n JMP start\n";
    let m = run(src, 10);
    let w = m.frame_work();
    assert!((4_000..7_000).contains(&w), "about 5,000 cycles of work a frame: {w}");
}

#[test]
fn a_program_that_never_waits_fills_the_frame() {
    let m = run(".org $0800\nstart: INX\n JMP start\n", 5);
    assert_eq!(m.frame_work(), 19_968);
}

#[test]
fn basic_waiting_for_a_key_is_idle() {
    let mut m = Machine::boot(RamSize::K16);
    for _ in 0..5 {
        m.run_frame();
    }
    assert!(m.frame_work() < 2_000, "READY. waits for a key: {}", m.frame_work());
}

fn type_line(m: &mut Machine, s: &str) {
    for &b in s.as_bytes().iter().chain(b"\r") {
        m.key(b);
        for _ in 0..200 {
            m.run_cycles(500);
            if m.peek(unc_core::machine::io::KBD) & 0x80 == 0 {
                break;
            }
        }
    }
}

#[test]
fn a_line_break_stops_on_the_line_every_time_it_runs() {
    let mut m = Machine::boot(RamSize::K16);
    for _ in 0..5 {
        m.run_frame();
    }
    type_line(&mut m, "10 A=A+1");
    type_line(&mut m, "20 GOTO 10");
    m.set_line_break(20, true);
    type_line(&mut m, "RUN");
    let mut hits = 0;
    for _ in 0..200 {
        if let unc_core::Stop::Breakpoint(_) = m.run_frame() {
            assert_eq!(m.basic_line(), Some(20));
            hits += 1;
            if hits == 3 {
                break;
            }
        }
    }
    assert_eq!(hits, 3, "stopped on line 20 three times");
    let a = m.peek(0x0260) as u16 | (m.peek(0x0261) as u16) << 8;
    assert!((2..=4).contains(&a), "line 10 ran once between stops: A={a}");
}
