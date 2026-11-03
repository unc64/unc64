//! BASIC's LOAD and the jump table's LOAD (manual chapter 14), driven as a person would:
//! a tape in the deck, LOAD typed at READY., PLAY pressed.

use unc_asm::tape::{self, HEADER_LEN, Program};
use unc_core::machine::{TAPE_BYTE_CYCLES, io};
use unc_core::{Machine, RamSize};

const MARK: u16 = 0x3F00;

/// A machine-code program at `load` that stores `tag` at MARK and halts.
fn tagged(name: &str, load: u16, tag: u8) -> Program {
    let src = format!(".org ${load:04X}\n .byte 0, 0\nstart: LDA #{tag}\n STA ${MARK:04X}\n HLT\n");
    tape::program(&unc_asm::assemble(&src).unwrap(), name)
}

fn type_keys(m: &mut Machine, keys: &[u8]) {
    for &k in keys {
        m.key(k);
        for _ in 0..200 {
            m.run_cycles(500);
            if m.peek(io::KBD) & 0x80 == 0 {
                break;
            }
        }
    }
}

fn type_line(m: &mut Machine, s: &str) {
    type_keys(m, s.as_bytes());
    type_keys(m, b"\r");
}

fn rows(m: &Machine) -> Vec<String> {
    m.screen_text().lines().map(str::to_string).filter(|l| !l.is_empty() && l != "_").collect()
}

/// Runs frame by frame until `done`, at most `secs` seconds of machine time.
fn run_until(m: &mut Machine, secs: f64, done: impl Fn(&Machine) -> bool) -> f64 {
    let t0 = m.cpu.cycles;
    while !done(m) {
        m.run_frame();
        let t = (m.cpu.cycles - t0) as f64 / 1e6;
        assert!(t < secs, "not done after {t:.1} s: {:?}", rows(m));
    }
    (m.cpu.cycles - t0) as f64 / 1e6
}

fn halted(m: &Machine) -> bool {
    m.cpu.state == unc_core::CpuState::Halted
}

fn ready(m: &Machine) -> bool {
    rows(m).last().is_some_and(|l| l == "READY.") && m.cpu.state == unc_core::CpuState::Running
}

fn boot_with(tape: Vec<u8>) -> Machine {
    let mut m = Machine::boot(RamSize::K16);
    m.insert_tape(tape);
    m
}

#[test]
fn load_waits_for_play_then_takes_the_first_program_and_starts_it() {
    let one = tagged("ONE", 0x0800, 11);
    let mut m = boot_with([one.encode(), tagged("TWO", 0x0800, 22).encode()].concat());
    type_line(&mut m, "LOAD");
    m.run_frame();
    assert_eq!(rows(&m).last().map(String::as_str), Some("PRESS PLAY ON TAPE"));
    assert!(!m.screen().contains(&0xA0), "no cursor while LOAD waits for PLAY");
    assert_ne!(m.peek(io::TAPE) & io::TAPE_MOTOR, 0, "the motor is on, waiting for PLAY");
    for _ in 0..50 {
        m.run_frame();
    }
    assert_eq!(m.mem.deck.position, 0, "nothing moves until PLAY");
    m.press_play();
    let entry = one.entry;
    let t0 = m.cpu.cycles;
    m.run_until(entry, 5_000_000).expect("the program starts");
    let secs = (m.cpu.cycles - t0) as f64 / 1e6;
    assert!(secs < (HEADER_LEN + 9) as f64 / 500.0, "{secs} s");
    // the start state of chapter 6
    let c = &m.cpu;
    assert_eq!((c.sp, c.p, c.a, c.x, c.y), (0xFF, 0x24, 0, 0, 0));
    assert_eq!(m.peek(io::TAPE) & io::TAPE_MOTOR, 0, "motor off");
    assert_eq!(m.peek(io::TAPE) & io::TAPE_PLAY, io::TAPE_PLAY, "PLAY stays down");
    assert_eq!(m.peek(io::BORDER), 0, "the border as it was");
    let r = rows(&m);
    let n = r.len();
    assert_eq!(r[n - 5..], ["LOAD", "PRESS PLAY ON TAPE", "SEARCHING", "FOUND ONE", "LOADING"]);
    assert!(!m.screen().contains(&0xA0), "the program starts on a screen with no cursor");
    run_until(&mut m, 1.0, halted);
    assert_eq!(m.peek(MARK), 11);
}

#[test]
fn with_play_already_down_there_is_no_prompt() {
    let mut m = boot_with(tagged("ONE", 0x0800, 11).encode());
    m.press_play();
    type_line(&mut m, "LOAD");
    run_until(&mut m, 2.0, halted);
    assert!(!m.screen_text().contains("PRESS PLAY"));
    assert_eq!(m.peek(MARK), 11);
}

#[test]
fn load_with_a_name_skips_the_others() {
    let mut m = boot_with([tagged("ONE", 0x0800, 11).encode(), tagged("TWO", 0x3000, 22).encode()].concat());
    m.press_play();
    type_line(&mut m, "LOAD \"TWO\"");
    run_until(&mut m, 2.0, halted);
    assert_eq!(m.peek(MARK), 22);
    let r = rows(&m);
    let n = r.len();
    assert_eq!(r[n - 5..], ["LOAD \"TWO\"", "SEARCHING FOR TWO", "FOUND ONE", "FOUND TWO", "LOADING"]);
    assert_ne!(m.peek(0x0803), 11, "ONE went by without loading (its LDA #11 would be here)");
}

#[test]
fn load_wrong_runs_to_the_end_of_the_tape_and_breaks() {
    let t = [tagged("ONE", 0x0800, 11).encode(), tagged("TWO", 0x0800, 22).encode()].concat();
    let len = t.len();
    let mut m = boot_with(t);
    m.press_play();
    type_line(&mut m, "LOAD \"WRONG\"");
    run_until(&mut m, 2.0, ready);
    let r = rows(&m);
    let n = r.len();
    assert_eq!(r[n - 6..], ["LOAD \"WRONG\"", "SEARCHING FOR WRONG", "FOUND ONE", "FOUND TWO", "BREAK", "READY."]);
    assert_eq!(m.mem.deck.position, len, "at the end of the tape");
    assert_eq!(m.peek(io::TAPE) & (io::TAPE_PLAY | io::TAPE_MOTOR), 0, "the deck stopped itself; the motor is off");
    assert_eq!(m.peek(MARK), 0);
}

#[test]
fn stop_aborts_load_with_break() {
    // while it waits for PLAY
    let mut m = boot_with(tagged("ONE", 0x0800, 11).encode());
    type_line(&mut m, "LOAD");
    m.run_frame();
    type_keys(&mut m, &[3]);
    run_until(&mut m, 1.0, ready);
    assert_eq!(rows(&m)[rows(&m).len() - 2..], ["BREAK", "READY."]);
    assert_eq!(m.peek(io::TAPE) & io::TAPE_MOTOR, 0);
    // while it loads: the border comes back too
    let big = Program { data: vec![0x5A; 3000], ..tagged("BIG", 0x0800, 0) };
    let mut m = boot_with(big.encode());
    m.poke(io::BORDER, 6);
    m.press_play();
    type_line(&mut m, "LOAD");
    run_until(&mut m, 2.0, |m| m.screen_text().contains("LOADING"));
    type_keys(&mut m, &[3]);
    run_until(&mut m, 1.0, ready);
    assert_eq!(rows(&m)[rows(&m).len() - 2..], ["BREAK", "READY."]);
    assert_eq!((m.peek(io::BORDER), m.peek(io::TAPE) & io::TAPE_MOTOR), (6, 0));
    // STOP on the deck instead
    let mut m = boot_with(big.encode());
    m.press_play();
    type_line(&mut m, "LOAD");
    run_until(&mut m, 2.0, |m| m.screen_text().contains("LOADING"));
    m.press_stop();
    run_until(&mut m, 1.0, ready);
    assert_eq!(rows(&m)[rows(&m).len() - 2..], ["BREAK", "READY."]);
}

#[test]
fn a_damaged_tape_gives_load_error() {
    let mut t = tagged("ONE", 0x0800, 11).encode();
    t[HEADER_LEN + 3] ^= 1;
    let mut m = boot_with(t);
    m.press_play();
    type_line(&mut m, "LOAD");
    run_until(&mut m, 2.0, ready);
    assert_eq!(rows(&m)[rows(&m).len() - 2..], ["?LOAD ERROR", "READY."]);
    assert_eq!(m.peek(io::TAPE) & io::TAPE_MOTOR, 0);
    assert_eq!(m.peek(MARK), 0, "never started");
    // and a header from a later format version
    let mut t = tagged("ONE", 0x0800, 11).encode();
    t[4] = 2;
    let mut m = boot_with(t);
    m.press_play();
    type_line(&mut m, "LOAD");
    run_until(&mut m, 2.0, ready);
    assert_eq!(rows(&m)[rows(&m).len() - 2..], ["?LOAD ERROR", "READY."]);
}

#[test]
fn a_program_too_big_for_memory_is_refused() {
    let big = Program { data: vec![0; 0x3801], ..tagged("BIG", 0x0800, 0) };
    let mut m = boot_with(big.encode());
    m.press_play();
    type_line(&mut m, "LOAD");
    run_until(&mut m, 2.0, ready);
    assert_eq!(rows(&m)[rows(&m).len() - 2..], ["?OUT OF MEMORY", "READY."]);
    // exactly up to the top of memory fits
    let fits = Program { data: vec![0; 0x3800], ..tagged("FITS", 0x0800, 0) };
    let mut m = boot_with(fits.encode());
    m.press_play();
    type_line(&mut m, "LOAD");
    m.run_until(0x0802, 50_000_000).expect("starts");
}

#[test]
fn the_border_shows_the_bytes_as_they_load() {
    let data: Vec<u8> = (0..1200u32).map(|i| (i * 5) as u8).collect();
    let p = Program { data, ..tagged("STRIPES", 0x0800, 0) };
    let mut m = boot_with(p.encode());
    m.press_play();
    type_line(&mut m, "LOAD");
    run_until(&mut m, 2.0, |m| m.screen_text().contains("LOADING"));
    m.run_frame();
    m.run_frame();
    let mut colours: Vec<u8> = m.border_lines.to_vec();
    colours.dedup();
    assert!(colours.len() >= 10, "a dozen stripes a frame: {colours:?}");
}

#[test]
fn load_lists_as_load() {
    let mut m = Machine::boot(RamSize::K16);
    type_line(&mut m, "10 LOAD \"SNAKE\"");
    type_line(&mut m, "LIST");
    run_until(&mut m, 1.0, ready);
    assert!(rows(&m).contains(&"10 LOAD \"SNAKE\"".to_string()));
    assert_eq!(m.peek(0x0803), 0xA1, "LOAD's token");
    type_line(&mut m, "HELP");
    run_until(&mut m, 1.0, ready);
    assert!(m.screen_text_plain().contains(" LOAD \"NAME\"     FROM TAPE"));
}

#[test]
fn machine_code_loads_through_the_jump_table() {
    let mut m = boot_with([tagged("ONE", 0x0800, 11).encode(), tagged("TWO", 0x0900, 22).encode()].concat());
    let caller = unc_asm::assemble(".org $3000\nstart: LDX #<name\n LDY #>name\n JSR $FF15\n HLT\nname: .asciiz \"TWO\"\n").unwrap();
    unc_asm::load(&mut m, &caller);
    m.press_play();
    run_until(&mut m, 2.0, halted);
    assert_eq!(m.peek(MARK), 22);
    assert!((0x0900..0x0910).contains(&m.cpu.pc), "halted in TWO, not in the caller: ${:04X}", m.cpu.pc);
}

/// Every game in games/ loads from a tape, with the timing the manual quotes.
#[test]
fn every_game_loads_from_tape_and_runs() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../games");
    let mut names: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().is_some_and(|e| e == "s")).collect();
    names.sort();
    let mut loaded = 0;
    for path in names {
        let src = std::fs::read_to_string(&path).unwrap();
        let Ok(asm) = unc_asm::assemble(&src) else { continue }; // a game still being written
        let name = path.file_stem().unwrap().to_str().unwrap().to_uppercase();
        let t = tape::make(&asm, &name);
        let mut m = boot_with(t.clone());
        type_line(&mut m, "LOAD");
        m.run_frame();
        m.press_play();
        let start = asm.start().unwrap();
        let t0 = m.cpu.cycles;
        m.run_until(start, 30_000_000).unwrap_or_else(|e| panic!("{name}: {e:?} {:?}", rows(&m)));
        let secs = (m.cpu.cycles - t0) as f64 / 1e6;
        let ideal = t.len() as f64 * TAPE_BYTE_CYCLES as f64 / 1e6;
        println!("{name}: {} bytes on tape, loaded in {secs:.2} s ({ideal:.2} s of tape)", t.len());
        assert!(secs < ideal + 0.5, "{name}: {secs} s");
        assert!(rows(&m).contains(&format!("FOUND {name}")));
        for (i, s) in asm.segments.iter().enumerate() {
            assert_eq!(&m.mem.ram[s.start as usize..][..s.bytes.len()], &s.bytes[..], "{name} segment {i}");
        }
        for _ in 0..100 {
            assert!(!matches!(m.run_frame(), unc_core::Stop::Jammed(_)), "{name} jammed");
        }
        loaded += 1;
    }
    assert!(loaded >= 3, "only {loaded} games");
}

/// SNAKE from a tape, start to title, the way the player does it.
#[test]
fn snake_from_tape_shows_its_title() {
    let src = include_str!("../../../games/snake.s");
    let asm = unc_asm::assemble(src).unwrap();
    let mut m = boot_with(tape::make(&asm, "SNAKE"));
    type_line(&mut m, "LOAD \"SNAKE\"");
    run_until(&mut m, 1.0, |m| m.screen_text().contains("PRESS PLAY ON TAPE"));
    m.press_play();
    let secs = run_until(&mut m, 10.0, |m| m.screen_text().contains("TO STEER"));
    assert!((2.0..4.0).contains(&secs), "SNAKE took {secs:.2} s");
    assert_eq!(m.peek(io::TAPE) & io::TAPE_MOTOR, 0);
}

/// A routine at `load` that prints `msg` and goes back to BASIC through READY ($FF21).
fn returning(name: &str, load: u16, msg: &str) -> Program {
    let src = format!(".org ${load:04X}\nstart: LDX #<m\n LDY #>m\n JSR $FF09\n JMP $FF21\nm: .asciiz \"{msg}\"\n");
    tape::program(&unc_asm::assemble(&src).unwrap(), name)
}

fn prgend(m: &Machine) -> u16 {
    u16::from_le_bytes([m.peek(0x0209), m.peek(0x020A)])
}

/// The BASIC program's bytes, $0800 to its end marker.
fn basic(m: &Machine) -> Vec<u8> {
    (0x0800..=prgend(m)).map(|a| m.peek(a)).collect()
}

#[test]
fn a_program_loaded_clear_of_basic_leaves_it_and_ready_goes_back_to_it() {
    let mut m = boot_with(returning("DATA", 0x3000, "LOADED").encode());
    type_line(&mut m, "10 PRINT \"MINE\";X");
    type_line(&mut m, "X=7");
    m.run_frame();
    let before = basic(&m);
    m.press_play();
    type_line(&mut m, "LOAD");
    run_until(&mut m, 2.0, ready);
    let r = rows(&m);
    assert_eq!(r[r.len() - 4..], ["FOUND DATA", "LOADING", "LOADED", "READY."]);
    assert_eq!(basic(&m), before, "the BASIC program stays");
    type_line(&mut m, "RUN");
    run_until(&mut m, 1.0, ready);
    assert!(rows(&m).contains(&"MINE0".to_string()), "RUN clears X, as ever: {:?}", rows(&m));
    // LOAD kept the variables too
    let mut m = boot_with(returning("DATA", 0x3000, "LOADED").encode());
    type_line(&mut m, "10 REM");
    type_line(&mut m, "X=7");
    m.press_play();
    type_line(&mut m, "LOAD");
    run_until(&mut m, 2.0, ready);
    type_line(&mut m, "PRINT X");
    run_until(&mut m, 1.0, ready);
    assert_eq!(rows(&m)[rows(&m).len() - 2], "7");
}

#[test]
fn a_program_that_lands_on_basic_replaces_it() {
    // At $0800, on top of the program: LOAD forgets it, as NEW does.
    let mut m = boot_with(returning("GAME", 0x0800, "LOADED").encode());
    type_line(&mut m, "10 REM GONE");
    m.press_play();
    type_line(&mut m, "LOAD");
    run_until(&mut m, 2.0, ready);
    assert_eq!(prgend(&m), 0x0800, "no BASIC program");
    // Just past the end marker: kept. On the end marker itself: replaced.
    for (offset, kept) in [(1u16, true), (0, false)] {
        let mut m = Machine::boot(RamSize::K16);
        type_line(&mut m, "10 REM HERE");
        m.run_frame(); // let the line editor store it
        let at = prgend(&m) + offset;
        m.insert_tape(returning("NEAR", at, "LOADED").encode());
        m.press_play();
        type_line(&mut m, "LOAD");
        run_until(&mut m, 2.0, ready);
        assert_eq!(prgend(&m) != 0x0800, kept, "loaded at ${at:04X}");
    }
    // Wholly below $0800 (on the screen): kept.
    let mut m = Machine::boot(RamSize::K16);
    type_line(&mut m, "10 REM HERE");
    let p = Program { load: 0x07F0, entry: 0x07F0, data: vec![0x06, 0x21, 0xFF], ..returning("LOW", 0x07F0, "") };
    m.insert_tape(p.encode());
    m.press_play();
    type_line(&mut m, "LOAD");
    run_until(&mut m, 2.0, ready);
    assert_ne!(prgend(&m), 0x0800);
}

#[test]
fn ready_through_the_jump_table_resets_the_stack_and_turns_interrupts_on() {
    let mut m = Machine::boot(RamSize::K16);
    type_line(&mut m, "10 REM KEEP");
    let p = unc_asm::assemble(".org $3000\nstart: PHA\n PHA\n PHA\n SEI\n JMP $FF21\n").unwrap();
    unc_asm::load(&mut m, &p);
    run_until(&mut m, 1.0, ready);
    assert!(!m.cpu.flag(unc_core::cpu::flag::I), "interrupts on, as BASIC runs");
    assert!(m.cpu.sp >= 0xF0, "a fresh stack: SP ${:02X}", m.cpu.sp);
    type_line(&mut m, "LIST");
    run_until(&mut m, 1.0, ready);
    assert!(rows(&m).contains(&"10 REM KEEP".to_string()));
}

#[test]
fn turbo_tape_loads_the_same_through_the_rom_only_faster() {
    let tape: Vec<u8> = [tagged("ONE", 0x2000, 0x11), tagged("TWO", 0x2400, 0x22)].iter().flat_map(Program::encode).collect();
    let mut slow = boot_with(tape.clone());
    let mut fast = boot_with(tape);
    fast.set_tape_turbo(true);
    let mut secs = [0.0; 2];
    for (i, m) in [&mut slow, &mut fast].into_iter().enumerate() {
        type_line(m, "LOAD \"TWO\"");
        m.press_play();
        secs[i] = run_until(m, 30.0, halted);
        assert_eq!(m.peek(MARK), 0x22, "the named program loaded and ran");
        assert_eq!(m.peek(io::TAPE) & io::TAPE_OVERRUN, 0, "nothing was lost");
    }
    assert_eq!(slow.peek(0x2000), fast.peek(0x2000), "the same memory");
    assert!(secs[1] * 5.0 < secs[0], "turbo took {:.2} s, the deck {:.2} s", secs[1], secs[0]);
}
