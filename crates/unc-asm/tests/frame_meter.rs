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
