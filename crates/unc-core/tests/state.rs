//! Save states (`Machine::save_state` / `load_state`): the whole computer, and back,
//! so exactly that the future after a load is the future the save would have had.

use unc_core::machine::{Machine, RamSize, io};
use unc_core::state::{MAGIC, VERSION};

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

/// A program that keeps most of the machine busy: RND, the screen, the timer and speaker,
/// the shifter, the bitmap page, the keyboard, FOR and GOSUB stacks.
fn busy(ram: RamSize) -> Machine {
    let mut m = Machine::boot(ram);
    for l in [
        "10 POKE 49202,RND(256):POKE 49203,1+RND(8):POKE 49201,3",
        "20 FOR I=1 TO 5:GOSUB 100:NEXT",
        "30 POKE 49232,RND(256):POKE 49233,I:POKE 1024+RND(1000),PEEK(49234)",
        "40 POKE 49184,PEEK(49170)+PEEK(49158):GOTO 10",
        "100 POKE 1984+RND(40),PEEK(49204):RETURN",
    ] {
        type_line(&mut m, l);
    }
    type_line(&mut m, "RUN");
    for _ in 0..7 {
        m.run_frame();
    }
    m.run_cycles(1234); // somewhere in the middle of a frame
    m.key(b'Q'); // a key down at the moment of saving
    m
}

/// Everything a program could see, and the picture: the state itself, the screen and the
/// border, frame by frame.
fn future(m: &mut Machine, frames: u32) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    for i in 0..frames {
        if i == 10 {
            m.key_up(b'Q');
        }
        m.run_frame();
        out.push([m.screen(), &m.border_lines[..]].concat());
    }
    out.push(m.save_state());
    out
}

#[test]
fn a_loaded_state_has_the_same_future_byte_for_byte() {
    for ram in [RamSize::K16, RamSize::K64] {
        let mut m = busy(ram);
        let saved = m.save_state();
        let straight = future(&mut m, 120);

        // into a machine that was doing something else entirely
        let mut other = Machine::boot(RamSize::K16);
        type_line(&mut other, "10 GOTO 10");
        other.load_state(&saved).unwrap();
        assert_eq!(other.save_state(), saved, "{ram:?}: load then save gives the same bytes");
        assert_eq!(future(&mut other, 120), straight, "{ram:?}");

        // and back into the machine that saved it, later on
        m.load_state(&saved).unwrap();
        assert_eq!(future(&mut m, 120), straight, "{ram:?}: loaded into itself");
    }
}

#[test]
fn the_ram_size_comes_back_with_the_state() {
    let big = Machine::boot(RamSize::K64);
    let mut m = Machine::boot(RamSize::K16);
    m.load_state(&big.save_state()).unwrap();
    assert_eq!(m.mem.ram_size, RamSize::K64);
    assert_eq!(m.peek(0x0203), 0xC0, "RAMTOP");
    m.poke(0x8000, 42);
    assert_eq!(m.peek(0x8000), 42, "the expansion is there");
    // and going down to 16K, the expansion's memory goes with it
    m.load_state(&Machine::boot(RamSize::K16).save_state()).unwrap();
    assert_eq!(m.peek(0x8000), 0xFF);
}

#[test]
fn a_basic_program_survives_a_quit() {
    let mut m = Machine::boot(RamSize::K16);
    type_line(&mut m, "10 PRINT \"STILL HERE\";A");
    type_line(&mut m, "A=7");
    for _ in 0..3 {
        m.run_frame();
    }
    let saved = m.save_state();
    drop(m);
    let mut m = Machine::boot(RamSize::K16);
    m.load_state(&saved).unwrap();
    type_line(&mut m, "GOTO 10");
    for _ in 0..5 {
        m.run_frame();
    }
    assert!(m.screen_text().contains("STILL HERE7"), "{}", m.screen_text());
}

#[test]
fn the_tape_stays_as_the_host_has_it() {
    let mut m = Machine::boot(RamSize::K16);
    m.insert_tape(b"UNCT not really a program".to_vec());
    m.press_play();
    let saved = m.save_state();
    assert!(!saved.windows(4).skip(4).any(|w| w == b"UNCT"), "the tape isn't in the save");

    let mut empty = Machine::boot(RamSize::K16);
    empty.load_state(&saved).unwrap();
    assert!(empty.mem.deck.tape.is_none(), "no tape: the save doesn't bring one");
    assert_eq!(empty.peek(io::TAPE) & (io::TAPE_IN | io::TAPE_PLAY), 0);

    let mut other = Machine::boot(RamSize::K16);
    other.insert_tape(vec![1, 2, 3]);
    other.load_state(&saved).unwrap();
    assert_eq!(other.mem.deck.tape.as_deref(), Some(&[1u8, 2, 3][..]), "the host's tape stays in");
}

#[test]
fn a_save_from_halfway_through_a_load_is_safe_with_any_tape() {
    // LOAD is waiting for bytes, motor on, when the state is saved.
    let mut m = Machine::boot(RamSize::K16);
    type_line(&mut m, "LOAD");
    for _ in 0..3 {
        m.run_frame();
    }
    assert_ne!(m.peek(io::TAPE) & io::TAPE_MOTOR, 0);
    let saved = m.save_state();
    let mut n = Machine::boot(RamSize::K16);
    n.load_state(&saved).unwrap();
    assert_ne!(n.peek(io::TAPE) & io::TAPE_MOTOR, 0, "the motor is the computer's: saved");
    n.insert_tape(b"rubbish".to_vec());
    n.press_play();
    for _ in 0..50 {
        n.run_frame();
    }
    let t = n.screen_text();
    assert!(t.trim_end().ends_with("BREAK\nREADY.\n_"), "{t}");
}

#[test]
fn bad_states_are_refused_and_leave_the_machine_alone() {
    let good = Machine::boot(RamSize::K16).save_state();
    assert_eq!(&good[..4], MAGIC);
    assert_eq!(good[4], VERSION);
    let mut m = Machine::boot(RamSize::K16);
    type_line(&mut m, "10 REM MINE");
    let before = m.save_state();
    let mut old = good.clone();
    old[4] = 0;
    let mut newer = good.clone();
    newer[4] = VERSION + 1;
    let mut bad_ram = good.clone();
    bad_ram[5] = 32;
    let mut longer = good.clone();
    longer.push(0);
    for (what, bytes) in [
        ("empty", &[][..]),
        ("not a state", &b"PK\x03\x04 a zip file"[..]),
        ("old version", &old[..]),
        ("newer version", &newer[..]),
        ("32K", &bad_ram[..]),
        ("cut short", &good[..good.len() - 1]),
        ("too long", &longer[..]),
    ] {
        let e = m.load_state(bytes).expect_err(what);
        assert!(!e.is_empty());
        assert_eq!(m.save_state(), before, "{what}: the machine is as it was");
    }
    assert!(m.load_state(&newer).unwrap_err().contains("version"));
}
