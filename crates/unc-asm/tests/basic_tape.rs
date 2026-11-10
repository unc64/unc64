//! BASIC programs on tape (manual chapter 14, type 2): SAVE to a blank tape, LOAD on another
//! machine, driven as a person would: lines typed at READY., RECORD and PLAY pressed.

use unc_asm::tape::{self, BASIC_START, Program, TYPE_BASIC, TapeError};
use unc_core::machine::{TAPE_BYTE_CYCLES, io};
use unc_core::{Machine, RamSize};

const VARS: u16 = 0x0260;
const FORSP: u16 = 0x0207;
const GOSP: u16 = 0x0208;

const PROGRAM: &[&str] = &[
    "10 REM COUNT TO FIVE",
    "20 FOR I=1 TO 5:PRINT I;:NEXT",
    "30 PRINT \"DONE\";N",
    "40 GOSUB 100:END",
    "100 PRINT \"SUB\":RETURN",
];

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

fn last(m: &Machine, n: usize) -> Vec<String> {
    let r = rows(m);
    r[r.len().saturating_sub(n)..].to_vec()
}

/// Runs frame by frame until `done`, at most `secs` seconds of machine time. Returns the time.
fn run_until(m: &mut Machine, secs: f64, done: impl Fn(&Machine) -> bool) -> f64 {
    let t0 = m.cpu.cycles;
    while !done(m) {
        m.run_frame();
        let t = (m.cpu.cycles - t0) as f64 / 1e6;
        assert!(t < secs, "not done after {t:.1} s: {:?}", rows(m));
    }
    (m.cpu.cycles - t0) as f64 / 1e6
}

fn ready(m: &Machine) -> bool {
    rows(m).last().is_some_and(|l| l == "READY.") && m.cpu.state == unc_core::CpuState::Running
}

/// Clears the screen and waits for READY., so the next checks see only what follows.
fn fresh_screen(m: &mut Machine) {
    type_line(m, "CLS");
    run_until(m, 1.0, ready);
}

/// The rows after `fresh_screen`'s READY.
fn out(m: &Machine) -> Vec<String> {
    rows(m)[1..].to_vec()
}

fn prgend(m: &Machine) -> u16 {
    u16::from_le_bytes([m.peek(0x0209), m.peek(0x020A)])
}

fn typed(ram: RamSize, lines: &[&str]) -> Machine {
    let mut m = Machine::boot(ram);
    for l in lines {
        type_line(&mut m, l);
    }
    for _ in 0..5 {
        m.run_frame(); // the last line stored (a long one scrolls the screen)
    }
    m
}

fn list(m: &mut Machine) -> Vec<String> {
    fresh_screen(m);
    type_line(m, "LIST");
    run_until(m, 2.0, ready);
    let r = out(m);
    r[1..r.len() - 1].to_vec()
}

/// SAVEs the machine's program onto the tape in the deck (a blank one if none) the way a
/// person does: SAVE typed, RECORD pressed at the prompt. Returns the tape, ejected.
fn save(m: &mut Machine, cmd: &str) -> Vec<u8> {
    if m.mem.deck.tape.is_none() {
        m.insert_blank_tape();
    }
    fresh_screen(m);
    type_line(m, cmd);
    m.run_frame();
    assert_eq!(out(m)[1], "PRESS RECORD AND PLAY ON TAPE");
    assert_eq!(m.peek(io::TAPE) & io::TAPE_MOTOR, 0, "the motor waits for the keys");
    assert!(m.press_record());
    run_until(m, 60.0, ready);
    m.eject_tape().unwrap()
}

fn load(m: &mut Machine, tape: Vec<u8>, cmd: &str) {
    m.insert_tape(tape);
    fresh_screen(m);
    type_line(m, cmd);
    m.run_frame();
    m.press_play();
    run_until(m, 60.0, ready);
}

#[test]
fn save_then_load_on_a_fresh_machine_gives_the_same_program() {
    let mut a = typed(RamSize::K16, PROGRAM);
    let listing = list(&mut a);
    assert_eq!(listing, PROGRAM);
    let program = tape::resident_basic(&a);
    a.poke(io::BORDER, 6);
    let t = save(&mut a, "SAVE \"COUNTER\"");
    assert_eq!(out(&a), ["SAVE \"COUNTER\"", "PRESS RECORD AND PLAY ON TAPE", "SAVING COUNTER", "READY."]);
    assert_eq!(a.peek(io::BORDER), 6, "the border as it was");
    assert_eq!(a.peek(io::TAPE) & io::TAPE_MOTOR, 0, "motor off");
    // what's on the tape: one type-2 program, exactly the bytes from $0800 to the end marker
    let progs = tape::parse(&t).unwrap();
    assert_eq!(progs.len(), 1);
    let p = &progs[0];
    assert_eq!((p.kind, p.name(), p.load, p.entry), (TYPE_BASIC, "COUNTER".into(), BASIC_START, 0));
    assert_eq!(p.data, program);
    assert_eq!(p.data.len(), (prgend(&a) - 0x0800 + 1) as usize);
    assert_eq!(*p.data.last().unwrap(), 0, "the end marker");
    assert_eq!(t, tape::basic("COUNTER", &program).encode(), "tape::basic makes the same tape");
    assert_eq!(tape::resident_basic(&a), program, "SAVE leaves the program alone");
    // another machine: LOAD, and READY. without running it
    let mut b = Machine::boot(RamSize::K16);
    load(&mut b, t, "LOAD");
    assert_eq!(out(&b), ["LOAD", "PRESS PLAY ON TAPE", "SEARCHING", "FOUND COUNTER", "LOADING", "READY."]);
    assert_eq!(b.peek(io::TAPE) & io::TAPE_MOTOR, 0);
    assert_eq!(prgend(&b), prgend(&a));
    assert_eq!(list(&mut b), PROGRAM);
    fresh_screen(&mut b);
    type_line(&mut b, "N=42");
    type_line(&mut b, "RUN");
    run_until(&mut b, 2.0, ready);
    assert_eq!(last(&b, 3), ["12345DONE0", "SUB", "READY."].map(String::from), "RUN clears N");
}

#[test]
fn loading_a_basic_program_replaces_the_resident_one_and_clears_variables() {
    let t = save(&mut typed(RamSize::K16, &["10 PRINT \"NEW\""]), "SAVE \"NEW\"");
    let mut m = typed(RamSize::K16, &["10 REM OLD", "20 REM LONGER THAN THE NEW ONE", "30 REM AND MORE"]);
    type_line(&mut m, "X=7:Z=-1");
    m.run_frame();
    m.poke(FORSP, 10); // as if a FOR and a GOSUB were open
    m.poke(GOSP, 5);
    load(&mut m, t, "LOAD \"NEW\"");
    assert_eq!(list(&mut m), ["10 PRINT \"NEW\""]);
    assert!((0..52).all(|i| m.peek(VARS + i) == 0), "every variable 0");
    assert_eq!((m.peek(FORSP), m.peek(GOSP)), (0, 0), "the FOR and GOSUB stacks empty");
    type_line(&mut m, "RUN");
    run_until(&mut m, 1.0, ready);
    assert_eq!(last(&m, 2), ["NEW", "READY."]);
    // new lines go in as usual
    type_line(&mut m, "5 PRINT \"FIRST\"");
    assert_eq!(list(&mut m), ["5 PRINT \"FIRST\"", "10 PRINT \"NEW\""]);
}

#[test]
fn an_empty_program_saves_and_loads_as_just_its_end_marker() {
    let mut m = Machine::boot(RamSize::K16);
    let t = save(&mut m, "SAVE");
    assert_eq!(out(&m)[2], "SAVING");
    let p = &tape::parse(&t).unwrap()[0];
    assert_eq!((p.name(), p.data.clone()), (String::new(), vec![0]), "no name, no lines");
    let mut b = typed(RamSize::K16, &["10 REM GONE"]);
    load(&mut b, t, "LOAD");
    assert_eq!(prgend(&b), 0x0800);
    assert!(list(&mut b).is_empty());
}

#[test]
fn save_on_a_tape_with_play_and_record_down_needs_no_prompt_and_appends() {
    let mut m = typed(RamSize::K16, &["10 PRINT \"ONE\""]);
    m.insert_blank_tape();
    m.press_record();
    type_line(&mut m, "SAVE \"ONE\"");
    run_until(&mut m, 5.0, ready);
    assert!(!m.screen_text().contains("PRESS RECORD"));
    type_line(&mut m, "10 PRINT \"TWO\"");
    type_line(&mut m, "SAVE \"TWO\"");
    run_until(&mut m, 5.0, |m| rows(m).iter().filter(|l| *l == "READY.").count() >= 3);
    let t = m.eject_tape().unwrap();
    let names: Vec<String> = tape::parse(&t).unwrap().iter().map(Program::name).collect();
    assert_eq!(names, ["ONE", "TWO"], "the second after the first: the tape grows");
    // LOAD "TWO" passes ONE by, as for machine code
    let mut b = Machine::boot(RamSize::K16);
    load(&mut b, t, "LOAD \"TWO\"");
    assert_eq!(out(&b)[3..], ["FOUND ONE", "FOUND TWO", "LOADING", "READY."]);
    assert_eq!(list(&mut b), ["10 PRINT \"TWO\""]);
}

#[test]
fn save_in_a_program_carries_on() {
    let mut m = typed(RamSize::K16, &["10 SAVE \"ME\":PRINT \"SAVED\"", "20 PRINT \"ON\""]);
    m.insert_blank_tape();
    m.press_record();
    fresh_screen(&mut m);
    type_line(&mut m, "RUN");
    run_until(&mut m, 5.0, ready);
    assert_eq!(out(&m)[1..], ["SAVING ME", "SAVED", "ON", "READY."]);
    let p = &tape::parse(&m.eject_tape().unwrap()).unwrap()[0];
    assert_eq!(p.data[3], 0xA2, "SAVE's token");
}

#[test]
fn a_tape_with_machine_code_and_basic_loads_either() {
    let code = tape::program(&unc_asm::assemble(".org $3000\nstart: LDA #9\n STA $3F00\n HLT\n").unwrap(), "CODE");
    let basic = tape::basic("PROG", &unc_asm::basic::tokenize("10 PRINT \"HI\"").unwrap());
    let t = [code.encode(), basic.encode()].concat();
    let mut m = Machine::boot(RamSize::K16);
    load(&mut m, t.clone(), "LOAD \"PROG\"");
    assert_eq!(list(&mut m), ["10 PRINT \"HI\""]);
    // and machine code still starts as before
    let mut m = Machine::boot(RamSize::K16);
    m.insert_tape(t);
    type_line(&mut m, "LOAD");
    m.press_play();
    run_until(&mut m, 2.0, |m| m.cpu.state == unc_core::CpuState::Halted);
    assert_eq!(m.peek(0x3F00), 9);
}

#[test]
fn break_stops_save_cleanly() {
    // the STOP key at the prompt
    let mut m = typed(RamSize::K16, PROGRAM);
    m.insert_blank_tape();
    fresh_screen(&mut m);
    type_line(&mut m, "SAVE \"X\"");
    m.run_frame();
    type_keys(&mut m, &[3]);
    run_until(&mut m, 1.0, ready);
    assert_eq!(out(&m)[1..], ["PRESS RECORD AND PLAY ON TAPE", "BREAK", "READY."]);
    assert!(m.mem.deck.tape.as_ref().unwrap().is_empty(), "nothing recorded");
    // the STOP key while it saves: the border and the motor come back, the program stays
    let long: Vec<String> = (1..=60).map(|n| format!("{n} PRINT \"A LINE OF THE PROGRAM, NUMBER\";{n}")).collect();
    let mut m = typed(RamSize::K16, &long.iter().map(String::as_str).collect::<Vec<_>>());
    let program_long = tape::resident_basic(&m);
    m.poke(io::BORDER, 5);
    m.insert_blank_tape();
    m.press_record();
    fresh_screen(&mut m);
    type_line(&mut m, "SAVE \"LONG\"");
    run_until(&mut m, 2.0, |m| m.mem.deck.position > 100);
    type_keys(&mut m, &[3]);
    run_until(&mut m, 1.0, ready);
    assert_eq!(out(&m)[1..], ["SAVING LONG", "BREAK", "READY."]);
    assert_eq!((m.peek(io::BORDER), m.peek(io::TAPE) & io::TAPE_MOTOR), (5, 0));
    assert_eq!(tape::resident_basic(&m), program_long);
    let part = m.eject_tape().unwrap();
    assert!(part.len() < program_long.len(), "only part of it: {} bytes", part.len());
    assert!(tape::parse(&part).is_err(), "a program cut short");
    // STOP on the deck instead
    let mut m = typed(RamSize::K16, &long.iter().map(String::as_str).collect::<Vec<_>>());
    m.insert_blank_tape();
    m.press_record();
    fresh_screen(&mut m);
    type_line(&mut m, "SAVE \"LONG\"");
    run_until(&mut m, 2.0, |m| m.mem.deck.position > 100);
    m.press_stop();
    run_until(&mut m, 1.0, ready);
    assert_eq!(out(&m)[1..], ["SAVING LONG", "BREAK", "READY."]);
    assert_eq!(m.peek(io::TAPE) & (io::TAPE_MOTOR | io::TAPE_RECORD | io::TAPE_PLAY), 0);
}

#[test]
fn break_while_a_basic_program_loads_leaves_an_empty_program_that_works() {
    let long: Vec<String> = (1..=60).map(|n| format!("{n} PRINT {n}")).collect();
    let t = tape::basic("LONG", &unc_asm::basic::tokenize(&long.join("\n")).unwrap()).encode();
    let mut m = typed(RamSize::K16, &["10 REM OLD"]);
    m.insert_tape(t);
    m.press_play();
    type_line(&mut m, "LOAD");
    run_until(&mut m, 2.0, |m| m.screen_text().contains("LOADING"));
    m.run_frame();
    type_keys(&mut m, &[3]);
    run_until(&mut m, 1.0, ready);
    assert_eq!(last(&m, 2), ["BREAK", "READY."]);
    assert_eq!((prgend(&m), m.peek(0x0800)), (0x0800, 0), "no program: just the end marker");
    type_line(&mut m, "10 PRINT 7");
    assert_eq!(list(&mut m), ["10 PRINT 7"]);
}

#[test]
fn a_type_2_tape_that_isnt_a_program_gives_load_error() {
    let mut m = Machine::boot(RamSize::K16);
    let good = unc_asm::basic::tokenize("10 PRINT 1\n20 PRINT 2").unwrap();
    let mut no_end = good.clone();
    no_end.pop();
    let mut short_line = good.clone();
    short_line[0] = 2;
    let mut extra = good.clone();
    extra.push(0);
    // Found after loading: the resident program is gone, as NEW leaves it. Found in the
    // header: refused before it touches the program.
    let cases = [
        ("no end marker", no_end, 0x0800, false),
        ("a line too short", short_line, 0x0800, false),
        ("bytes after the end", extra, 0x0800, false),
        ("not at $0800", good.clone(), 0x0900, true),
        ("empty", vec![], 0x0800, true),
    ];
    for (why, data, load, kept) in cases {
        let p = Program { load, ..tape::basic("BAD", &data) };
        assert!(matches!(tape::parse(&p.encode()), Err(TapeError::Basic { .. })), "{why}");
        type_line(&mut m, "10 REM KEEP");
        load_tape(&mut m, p.encode());
        assert_eq!(last(&m, 2), ["?LOAD ERROR", "READY."], "{why}");
        if kept {
            assert_eq!(list(&mut m), ["10 REM KEEP"], "{why}");
        } else {
            assert_eq!((prgend(&m), m.peek(0x0800)), (0x0800, 0), "{why}: an empty program");
        }
    }
    // a type the ROM doesn't know
    let p = Program { kind: 3, ..tape::basic("BAD", &good) };
    load_tape(&mut m, p.encode());
    assert_eq!(last(&m, 2), ["?LOAD ERROR", "READY."]);
}

fn load_tape(m: &mut Machine, t: Vec<u8>) {
    m.insert_tape(t);
    m.press_play();
    type_line(m, "LOAD");
    run_until(m, 5.0, ready);
}

#[test]
fn programs_move_between_a_unc_16_and_a_unc_64() {
    let t = save(&mut typed(RamSize::K64, PROGRAM), "SAVE \"P\"");
    let mut small = Machine::boot(RamSize::K16);
    load(&mut small, t, "LOAD");
    assert_eq!(list(&mut small), PROGRAM);
    let t = save(&mut small, "SAVE \"P\"");
    let mut big = Machine::boot(RamSize::K64);
    load(&mut big, t, "LOAD");
    assert_eq!(list(&mut big), PROGRAM);
    // one too big for a UNC 16: 20K of lines
    let lines: Vec<String> = (1..=200).map(|n| format!("{n} REM {}", "X".repeat(96))).collect();
    let program = unc_asm::basic::tokenize(&lines.join("\n")).unwrap();
    assert!(program.len() > 0x3800);
    let t = tape::basic("HUGE", &program).encode();
    let mut big = Machine::boot(RamSize::K64);
    big.set_tape_turbo(true);
    load(&mut big, t.clone(), "LOAD");
    assert_eq!(tape::resident_basic(&big), program, "fits on a UNC 64");
    let mut small = typed(RamSize::K16, &["10 REM MINE"]);
    small.set_tape_turbo(true);
    load(&mut small, t, "LOAD");
    assert_eq!(last(&small, 2), ["?OUT OF MEMORY", "READY."]);
    assert_eq!(list(&mut small), ["10 REM MINE"], "refused before it touched the program");
}

#[test]
fn a_program_that_wont_fit_beside_the_bitmap_is_refused() {
    // GR keeps BASIC below the bitmap at $2000 on a UNC 16
    let lines: Vec<String> = (1..=70).map(|n| format!("{n} REM {}", "X".repeat(100))).collect();
    let program = unc_asm::basic::tokenize(&lines.join("\n")).unwrap();
    assert!((0x1800..0x3800).contains(&program.len()));
    let mut m = Machine::boot(RamSize::K16);
    m.set_tape_turbo(true);
    type_line(&mut m, "GR");
    for _ in 0..10 {
        m.run_frame(); // GR clears 8,000 bytes before it reads another key
    }
    run_until(&mut m, 1.0, |m| m.screen_text().contains("READY."));
    m.insert_tape(tape::basic("BIG", &program).encode());
    m.press_play();
    type_line(&mut m, "LOAD");
    run_until(&mut m, 5.0, |m| m.screen_text().contains("?OUT OF MEMORY"));
    type_line(&mut m, "TEXT");
    load_tape(&mut m, tape::basic("BIG", &program).encode());
    assert_eq!(tape::resident_basic(&m), program, "with the text screen it fits");
}

#[test]
fn turbo_saves_the_same_tape_only_faster() {
    let mut slow = typed(RamSize::K16, PROGRAM);
    let mut fast = typed(RamSize::K16, PROGRAM);
    fast.set_tape_turbo(true);
    let mut secs = [0.0; 2];
    let mut tapes = vec![];
    for (i, m) in [&mut slow, &mut fast].into_iter().enumerate() {
        m.insert_blank_tape();
        m.press_record();
        fresh_screen(m);
        type_line(m, "SAVE \"T\"");
        secs[i] = run_until(m, 10.0, ready);
        tapes.push(m.eject_tape().unwrap());
    }
    assert_eq!(tapes[0], tapes[1]);
    let ideal = tapes[0].len() as f64 * TAPE_BYTE_CYCLES as f64 / 1e6;
    assert!(secs[0] > ideal && secs[0] < ideal + 0.2, "a byte every 1,664 cycles: {:.3} s for {:.3} s of tape", secs[0], ideal);
    assert!(secs[1] * 3.0 < secs[0], "turbo took {:.3} s, the deck {:.3} s", secs[1], secs[0]);
    // a LOAD in turbo too
    let mut m = Machine::boot(RamSize::K16);
    m.set_tape_turbo(true);
    load(&mut m, tapes.remove(0), "LOAD");
    assert_eq!(list(&mut m), PROGRAM);
}

#[test]
fn save_lists_as_save_and_help_knows_it() {
    let mut m = typed(RamSize::K16, &["10 SAVE \"SNAKE\""]);
    assert_eq!(list(&mut m), ["10 SAVE \"SNAKE\""]);
    assert_eq!(m.peek(0x0803), 0xA2, "SAVE's token");
    type_line(&mut m, "HELP");
    run_until(&mut m, 1.0, ready);
    let help = m.screen_text_plain();
    assert!(help.contains(" LOAD \"NAME\"     FROM TAPE") && help.contains(" SAVE \"NAME\"     ONTO TAPE"), "{help}");
    type_line(&mut m, "SAVE X");
    run_until(&mut m, 1.0, ready);
    assert_eq!(last(&m, 2), ["?SYNTAX ERROR", "READY."]);
}

/// SAVE took ROM space, not addresses: the jump table, the vectors and the start-up screen
/// are as they were.
#[test]
fn the_jump_table_vectors_and_banner_are_unchanged() {
    let rom = unc_asm::assemble(include_str!("../../../rom/system.s")).unwrap();
    let m = Machine::boot(RamSize::K16);
    let routines = ["CLS", "CHROUT", "GETKEY", "PRSTR", "PRHEX", "PRDEC", "RNDSTEP", "JLOAD", "NUMSTR", "PRNUM", "RNDSEED", "WARM"];
    for (i, name) in routines.iter().enumerate() {
        let at = 0xFF00 + 3 * i as u16;
        let target = u16::from_le_bytes([m.peek(at + 1), m.peek(at + 2)]);
        assert_eq!((m.peek(at), target), (0x06, rom.symbol(name).unwrap()), "${at:04X}: JMP {name}");
    }
    assert!((0xFF24..0xFFE0).all(|a| m.peek(a) == 0xFF), "$FF24 to the RESET button's check at $FFE0: still free for the jump table");
    let word = |a: u16| u16::from_le_bytes([m.peek(a), m.peek(a + 1)]);
    assert_eq!([word(0xFFFC), word(0xFFFE)], [rom.symbol("POWER").unwrap(), rom.symbol("IRQ").unwrap()]);
    assert_eq!(
        rows(&m),
        ["    UNC 64 SYSTEM ROM  REV A", " 16K RAM SYSTEM", " UNC BASIC REV A  14335 BYTES FREE", " TYPE HELP FOR HELP", "READY."]
    );
}

#[test]
fn a_listing_tokenizes_on_the_host_as_the_machine_does() {
    let host = unc_asm::basic::tokenize(&PROGRAM.join("\n")).unwrap();
    assert_eq!(host, tape::resident_basic(&typed(RamSize::K16, PROGRAM)));
    // lower case, blank lines, replacing and deleting lines, as at the keyboard
    let p = unc_asm::basic::tokenize("20 print \"b\"\n\n10 PRINT \"A\"\n20 PRINT \"C\"\n30 REM GONE\n30\n").unwrap();
    assert_eq!(p, unc_asm::basic::tokenize("10 PRINT \"A\"\n20 PRINT \"C\"").unwrap());
    assert!(unc_asm::basic::tokenize("PRINT 1").unwrap_err().contains("line number"));
    assert!(unc_asm::basic::tokenize("10 PRINT \"{}\"").unwrap_err().contains("the UNC can type"));
    assert!(unc_asm::basic::tokenize(&format!("10 REM {}", "X".repeat(120))).unwrap_err().contains("120"));
    let huge: Vec<String> = (1..=500).map(|n| format!("{n} REM {}", "X".repeat(100))).collect();
    assert!(unc_asm::basic::tokenize(&huge.join("\n")).unwrap_err().contains("doesn't fit"));
}

#[test]
fn the_command_line_makes_a_basic_tape() {
    let dir = std::env::temp_dir().join(format!("unc-asm-basic-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("counter.bas");
    std::fs::write(&src, PROGRAM.join("\n") + "\n").unwrap();
    let out = dir.join("counter.tap");
    let run = std::process::Command::new(env!("CARGO_BIN_EXE_unc-asm"))
        .arg("--basic")
        .arg(&src)
        .arg("--tape")
        .arg(&out)
        .status()
        .unwrap();
    assert!(run.success());
    let t = std::fs::read(&out).unwrap();
    let p = &tape::parse(&t).unwrap()[0];
    assert_eq!((p.kind, p.name()), (TYPE_BASIC, "COUNTER".to_string()));
    let mut m = Machine::boot(RamSize::K16);
    load(&mut m, t, "LOAD");
    assert_eq!(list(&mut m), PROGRAM);
    let bad = dir.join("bad.bas");
    std::fs::write(&bad, "PRINT 1\n").unwrap();
    let run = std::process::Command::new(env!("CARGO_BIN_EXE_unc-asm"))
        .args([bad.as_os_str(), "--tape".as_ref(), dir.join("bad.tap").as_os_str()])
        .arg("--basic")
        .output()
        .unwrap();
    assert!(!run.status.success());
    std::fs::remove_dir_all(&dir).ok();
}
