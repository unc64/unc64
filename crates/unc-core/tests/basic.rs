//! UNC BASIC Rev A, driven the way a person would: by typing.

use unc_core::machine::{BOOT_FRAMES, Machine, RamSize, io};

const CURX: u16 = 0x0200;
const CURY: u16 = 0x0201;
const RUNFLG: u16 = 0x0204;
const CURLIN: u16 = 0x0205;

fn boot() -> Machine {
    let mut m = Machine::new(RamSize::K16);
    for _ in 0..BOOT_FRAMES {
        m.run_frame();
    }
    m
}

/// Types the keys one at a time, giving the machine time to take each one.
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

/// Types a line and RETURN, then lets it run for `frames` frames.
fn line(m: &mut Machine, text: &str, frames: u32) {
    type_keys(m, text.as_bytes());
    type_keys(m, b"\r");
    for _ in 0..frames {
        m.run_frame();
    }
}

fn screen(m: &Machine) -> String {
    m.screen_text()
}

fn rows(m: &Machine) -> Vec<String> {
    m.screen_text().lines().map(str::to_string).collect()
}

/// The last `n` non-empty rows above the cursor.
fn last_rows(m: &Machine, n: usize) -> Vec<String> {
    let r: Vec<String> = rows(m).into_iter().filter(|l| !l.is_empty() && l != "_").collect();
    r[r.len().saturating_sub(n)..].to_vec()
}

/// Runs a direct-mode line and returns what it printed (the rows between the typed line and READY.).
/// Runs until BASIC is back at READY. with nothing still moving on the screen (a scroll
/// takes most of a frame, and a snapshot halfway through one shows rows twice).
/// Returns the cycles it took, to the nearest 5,000.
fn wait_ready(m: &mut Machine) -> u64 {
    let start = m.cpu.cycles;
    loop {
        let before = m.screen().to_vec();
        m.run_cycles(5_000);
        if m.screen() == &before[..] && last_rows(m, 1) == ["READY."] {
            return m.cpu.cycles - start;
        }
        assert!(m.cpu.cycles - start < 20_000_000, "BASIC never came back to READY.: {:?}", rows(m));
    }
}

fn direct(m: &mut Machine, text: &str) -> Vec<String> {
    line(m, text, 0);
    wait_ready(m);
    let r: Vec<String> = rows(m).into_iter().filter(|l| !l.is_empty() && l != "_").collect();
    // The typed line, unless the command cleared the screen (CLS, GR, TEXT).
    let from = r.iter().rposition(|l| l == text).map_or(0, |at| at + 1);
    let mut out = r[from..].to_vec();
    assert_eq!(out.pop().as_deref(), Some("READY."), "{:?}", r);
    out
}

fn program(m: &mut Machine, lines: &[&str]) {
    for l in lines {
        line(m, l, 0);
    }
}

#[test]
fn boots_to_basic_with_bytes_free_and_help_hint() {
    let m = boot();
    let s = screen(&m);
    assert!(s.contains("UNC BASIC REV A  14335 BYTES FREE"), "{s}");
    assert!(s.contains("TYPE HELP FOR HELP"), "{s}");
    assert_eq!(last_rows(&m, 2), [" TYPE HELP FOR HELP", "READY."]);
    assert!(s.contains("READY.\n_"), "{s}");
    let mut m64 = Machine::new(RamSize::K64);
    for _ in 0..BOOT_FRAMES {
        m64.run_frame();
    }
    assert!(screen(&m64).contains("47103 BYTES FREE"));
}

#[test]
fn print_hello_in_direct_mode() {
    let mut m = boot();
    assert_eq!(direct(&mut m, "PRINT \"HELLO\""), ["HELLO"]);
    assert_eq!(direct(&mut m, "?\"HI\""), ["HI"], "? is short for PRINT");
    assert_eq!(direct(&mut m, "PRINT \"A\";\"B\";3"), ["AB3"]);
    assert_eq!(direct(&mut m, "PRINT 1,2"), ["1         2"], "a comma moves to the next tenth column");
}

#[test]
fn arithmetic_precedence_and_signs() {
    let mut m = boot();
    for (e, want) in [
        ("2+3*4", "14"),
        ("(2+3)*4", "20"),
        ("10-2-3", "5"),
        ("-5+2", "-3"),
        ("7/2", "3"),
        ("-7/2", "-3"),
        ("100*-3", "-300"),
        ("2*3+4*5", "26"),
        ("$10+1", "17"),
        ("$C020", "-16352"),
        ("65535", "-1"),
        ("32767+1", "-32768"),
        ("ABS(-5)", "5"),
        ("1<2", "-1"),
        ("2<1", "0"),
        ("3=3 AND 2<>2", "0"),
        ("3>=3 AND 2<=2", "-1"),
        ("1 OR 2", "3"),
    ] {
        assert_eq!(direct(&mut m, &format!("PRINT {e}")), [want], "{e}");
    }
}

#[test]
fn variables_and_several_statements_per_line() {
    let mut m = boot();
    assert_eq!(direct(&mut m, "A=1:LET B=2:PRINT A+B"), ["3"]);
    assert_eq!(direct(&mut m, "PRINT Z"), ["0"], "variables start at 0");
}

#[test]
fn backspace_edits_the_line_being_typed() {
    let mut m = boot();
    line(&mut m, "PRINR\x08T 7", 3);
    assert_eq!(last_rows(&m, 3), ["PRINT 7", "7", "READY."]);
}

/// DELETE goes back up a line that wrapped: from the left edge of the second row to the last
/// column of the first, rubbing out as it goes, and what's left is the line that's run.
#[test]
fn delete_backs_up_a_wrapped_line() {
    let mut m = boot();
    // PRINT 1+ ... 40 columns: "PRINT " and 1+1+...: the line wraps onto a second row
    let text = format!("PRINT {}1", "1+".repeat(18));
    assert!(text.len() > 40, "the line wraps");
    type_keys(&mut m, text.as_bytes());
    let row = m.peek(CURY);
    let over = text.len() - 40;
    // back to the left edge of the second row, then three more: up onto the first
    for _ in 0..over + 3 {
        type_keys(&mut m, &[8]);
    }
    assert_eq!(m.peek(CURY), row - 1, "back up on the first row");
    assert_eq!(m.peek(CURX) as usize, 37, "three in from the end of it");
    // what's left, PRINT 1+1+... cut short, still runs: "+" rubbed out with the 1 after it
    let kept = &text[..text.len() - over - 3];
    let kept = kept.trim_end_matches('+');
    type_keys(&mut m, &vec![8u8; text.len() - over - 3 - kept.len()]);
    type_keys(&mut m, b"\r");
    wait_ready(&mut m);
    let ones = kept.matches('1').count();
    assert!(screen(&m).contains(&format!(" {ones}")), "it ran what was left: {:?}", last_rows(&m, 4));
}

#[test]
fn lines_are_stored_sorted_and_list_round_trips() {
    let mut m = boot();
    program(&mut m, &["30 PRINT \"A:B\";X", "10 REM FOR THE RECORD", "20 ?\"HI\"", "25 IF X>1 THEN 10"]);
    assert_eq!(
        direct(&mut m, "LIST"),
        ["10 REM FOR THE RECORD", "20 PRINT\"HI\"", "25 IF X>1 THEN 10", "30 PRINT \"A:B\";X"]
    );
    assert_eq!(direct(&mut m, "LIST 20-25"), ["20 PRINT\"HI\"", "25 IF X>1 THEN 10"]);
    assert_eq!(direct(&mut m, "LIST -10"), ["10 REM FOR THE RECORD"]);
    assert_eq!(direct(&mut m, "LIST 30"), ["30 PRINT \"A:B\";X"]);
}

#[test]
fn retyping_a_line_replaces_it_and_a_bare_number_deletes_it() {
    let mut m = boot();
    program(&mut m, &["10 PRINT 1", "20 PRINT 2", "30 PRINT 3", "20 PRINT 22", "10"]);
    assert_eq!(direct(&mut m, "LIST"), ["20 PRINT 22", "30 PRINT 3"]);
    assert_eq!(direct(&mut m, "RUN"), ["22", "3"]);
}

#[test]
fn new_forgets_the_program() {
    let mut m = boot();
    program(&mut m, &["10 PRINT 1"]);
    line(&mut m, "NEW", 2);
    assert_eq!(direct(&mut m, "LIST"), Vec::<String>::new());
    assert_eq!(m.peek(0x0800), 0, "the end marker is back at $0800");
}

#[test]
fn for_next_with_and_without_step() {
    let mut m = boot();
    program(&mut m, &["10 FOR I=10 TO 1 STEP -3", "20 PRINT I;\" \";", "30 NEXT I", "40 FOR J=0 TO 10 STEP 5:PRINT J;:NEXT", "50 PRINT"]);
    assert_eq!(direct(&mut m, "RUN"), ["10 7 4 1 0510"]);
    assert_eq!(direct(&mut m, "FOR K=1 TO 3:PRINT K;:NEXT:PRINT"), ["123"], "loops work in direct mode too");
}

#[test]
fn nested_loops() {
    let mut m = boot();
    program(&mut m, &["10 FOR I=1 TO 3", "20 FOR J=1 TO I", "30 PRINT \"*\";", "40 NEXT J", "50 PRINT", "60 NEXT I"]);
    assert_eq!(direct(&mut m, "RUN"), ["*", "**", "***"]);
}

#[test]
fn goto_gosub_return_and_end() {
    let mut m = boot();
    program(&mut m, &[
        "10 GOSUB 100",
        "20 GOSUB 200",
        "30 PRINT \"DONE\"",
        "40 END",
        "50 PRINT \"NOT HERE\"",
        "100 PRINT \"ONE\":GOSUB 200:RETURN",
        "200 PRINT \"TWO\":RETURN",
    ]);
    assert_eq!(direct(&mut m, "RUN"), ["ONE", "TWO", "TWO", "DONE"]);
    program(&mut m, &["NEW", "10 GOTO 30", "20 PRINT \"SKIPPED\"", "30 PRINT \"LANDED\""]);
    assert_eq!(direct(&mut m, "RUN"), ["LANDED"]);
}

#[test]
fn if_then_line_number_and_statement() {
    let mut m = boot();
    program(&mut m, &[
        "10 X=5",
        "20 IF X>3 THEN PRINT \"BIG\":PRINT \"REALLY\"",
        "30 IF X<3 THEN PRINT \"SMALL\":PRINT \"SKIPPED TOO\"",
        "40 IF X=5 THEN 60",
        "50 PRINT \"NO\"",
        "60 PRINT \"YES\"",
    ]);
    assert_eq!(direct(&mut m, "RUN"), ["BIG", "REALLY", "YES"]);
}

#[test]
fn poke_the_border_and_peek_it_back() {
    let mut m = boot();
    direct(&mut m, "POKE 49184,2");
    assert_eq!(m.peek(io::BORDER), 2, "the wonder beat: a red border");
    direct(&mut m, "POKE $C021,5");
    assert_eq!(m.peek(io::BACKGROUND), 5);
    assert_eq!(direct(&mut m, "PRINT PEEK(49184)"), ["2"]);
    direct(&mut m, "POKE 1024,1");
    assert_eq!(m.peek(0x0400), 1);
}

/// Every screen cell holding the cursor, `$A0`.
fn cursors(m: &Machine) -> Vec<usize> {
    m.screen().iter().enumerate().filter(|&(_, &c)| c == 0xA0).map(|(i, _)| i).collect()
}

/// The screen offset of CURX/CURY: where the next character goes.
fn cursor_at(m: &Machine) -> usize {
    m.peek(CURY) as usize * 40 + m.peek(CURX) as usize
}

#[test]
fn the_cursor_shows_only_while_basic_waits_for_a_line() {
    let mut m = boot();
    assert_eq!(cursors(&m), [cursor_at(&m)], "READY. waits with one cursor, where the next key goes");
    type_keys(&mut m, b"PRINT");
    assert_eq!(cursors(&m), [cursor_at(&m)], "it follows the typing");
    type_keys(&mut m, b"\r");
    wait_ready(&mut m);
    assert_eq!(cursors(&m), [cursor_at(&m)], "RETURN takes it away; the next READY. brings it back");

    // A running program's screen has no cursor, even one that prints and never ends.
    program(&mut m, &["10 PRINT \"HI\";", "20 GOTO 20"]);
    line(&mut m, "RUN", 5);
    assert_eq!(m.peek(RUNFLG), 1, "running");
    assert!(screen(&m).contains("\nHI"), "{}", screen(&m));
    assert!(cursors(&m).is_empty(), "no cursor while the program runs: {}", screen(&m));
    m.key(3);
    wait_ready(&mut m);
    assert_eq!(cursors(&m), [cursor_at(&m)], "STOP, READY., and the cursor is back");

    // INPUT shows it while it waits, and takes it away on RETURN.
    program(&mut m, &["NEW", "10 INPUT A", "20 GOTO 20"]);
    line(&mut m, "RUN", 3);
    assert_eq!(cursors(&m), [cursor_at(&m)], "INPUT waits with the cursor");
    line(&mut m, "5", 3);
    assert_eq!(m.peek(RUNFLG), 1, "running");
    assert!(cursors(&m).is_empty(), "gone once the program carries on: {}", screen(&m));
}

#[test]
fn the_cursor_puts_back_what_was_under_it() {
    let mut m = boot();
    program(&mut m, &["10 CLS:POKE 1027,88:INPUT A", "20 GOTO 20"]);
    line(&mut m, "RUN", 3);
    assert_eq!(rows(&m)[0], "? _X", "INPUT waits after its prompt, an X poked just past the cursor");
    type_keys(&mut m, b"5");
    assert_eq!(rows(&m)[0], "? 5_", "the cursor sits on the X while it waits");
    type_keys(&mut m, b"\r");
    m.run_frame();
    assert_eq!(rows(&m)[0], "? 5X", "and gives it back on RETURN");
    assert_eq!(m.peek(0x0260), 5, "A = 5");
    // STOP while typing a line throws the line away; the cursor goes with it.
    m.key(3);
    wait_ready(&mut m);
    type_keys(&mut m, b"AB");
    let at = cursor_at(&m);
    type_keys(&mut m, b"\x03");
    assert_eq!(m.screen()[at], b' ', "STOP took the cursor away");
    assert_eq!(cursors(&m), [cursor_at(&m)], "and BASIC waits for a new line, with a new one");
}

#[test]
fn input_reads_a_number_and_asks_again_for_nonsense() {
    let mut m = boot();
    program(&mut m, &["10 INPUT \"HOW MANY\";A", "20 PRINT A*2"]);
    line(&mut m, "RUN", 3);
    assert_eq!(rows(&m).iter().rev().find(|l| !l.is_empty()).unwrap(), "HOW MANY? _", "the prompt waits with the cursor");
    line(&mut m, "BANANA", 3);
    assert!(screen(&m).contains("?REDO"));
    line(&mut m, "-21", 3);
    assert_eq!(last_rows(&m, 2), ["-42", "READY."]);
}

#[test]
fn rnd_stays_in_range_and_is_deterministic() {
    let mut m = boot();
    program(&mut m, &["10 FOR I=1 TO 12:PRINT RND(6);:NEXT:PRINT"]);
    let a = direct(&mut m, "RUN");
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].len(), 12);
    assert!(a[0].chars().all(|c| ('0'..='5').contains(&c)), "{a:?}");
    let mut m2 = boot();
    program(&mut m2, &["10 FOR I=1 TO 12:PRINT RND(6);:NEXT:PRINT"]);
    assert_eq!(direct(&mut m2, "RUN"), a, "same machine, same numbers");
    assert_eq!(direct(&mut m, "PRINT RND(0)"), ["0"]);
}

#[test]
fn error_messages() {
    let mut m = boot();
    assert_eq!(direct(&mut m, "PRINT ("), ["?SYNTAX ERROR"]);
    assert_eq!(direct(&mut m, "HELLO"), ["?SYNTAX ERROR"]);
    assert_eq!(direct(&mut m, "GOTO 99"), ["?UNDEFINED LINE"]);
    assert_eq!(direct(&mut m, "NEXT"), ["?NEXT WITHOUT FOR"]);
    assert_eq!(direct(&mut m, "RETURN"), ["?RETURN WITHOUT GOSUB"]);
    assert_eq!(direct(&mut m, "PRINT 1/0"), ["?DIVISION BY ZERO"]);
    program(&mut m, &["10 PRINT 1", "20 X=1/0"]);
    assert_eq!(direct(&mut m, "RUN"), ["1", "?DIVISION BY ZERO IN 20"]);
    program(&mut m, &["NEW", "10 GOSUB 10"]);
    assert_eq!(direct(&mut m, "RUN"), ["?OUT OF MEMORY IN 10"]);
    assert_eq!(m.peek(RUNFLG), 0);
}

#[test]
fn a_full_memory_says_out_of_memory() {
    let mut m = boot();
    let body = "REM ".to_string() + &"X".repeat(110);
    let mut n = 0;
    loop {
        n += 1;
        line(&mut m, &format!("{n} {body}"), 0);
        if screen(&m).contains("?OUT OF MEMORY") || n > 200 {
            break;
        }
    }
    assert!(screen(&m).contains("?OUT OF MEMORY"), "{}", screen(&m));
    assert!((110..130).contains(&n), "14K holds about 120 of these lines, got {n}");
}

#[test]
fn stop_breaks_a_running_program_and_the_running_flag_follows() {
    let mut m = boot();
    program(&mut m, &["10 X=X+1", "20 GOTO 10"]);
    assert_eq!(m.peek(RUNFLG), 0);
    line(&mut m, "RUN", 3);
    assert_ne!(m.peek(RUNFLG), 0, "running");
    let cur = m.peek(CURLIN) as u16 | (m.peek(CURLIN + 1) as u16) << 8;
    assert!(cur == 10 || cur == 20, "current line {cur}");
    m.key(3);
    for _ in 0..3 {
        m.run_frame();
    }
    let r = last_rows(&m, 2);
    assert!(r[0] == "BREAK IN 10" || r[0] == "BREAK IN 20", "{r:?}");
    assert_eq!(r[1], "READY.");
    assert_eq!(m.peek(RUNFLG), 0, "back in direct mode");
    assert_eq!(direct(&mut m, "PRINT X>10"), ["-1"], "variables survive a break");
}

#[test]
fn sys_calls_machine_code_and_comes_back() {
    use unc_core::isa::{Mode, Op, opcode_for};
    let mut m = boot();
    let op = |o, md| opcode_for(o, md).unwrap();
    m.load(0x3000, &[op(Op::Lda, Mode::Imm), 42, op(Op::Sta, Mode::Abs), 0x00, 0x31, op(Op::Rts, Mode::Imp)]);
    assert_eq!(direct(&mut m, "SYS 12288:PRINT PEEK(12544)"), ["42"]);
}

#[test]
fn help_and_cls() {
    let mut m = boot();
    line(&mut m, "HELP", 0);
    wait_ready(&mut m);
    // One whole screen: the bar on top, the help in rows 0-22, READY. under it, nothing scrolled.
    let plain: Vec<String> = m.screen_text_plain().lines().map(|l| l.replace('?', " ")).collect();
    assert_eq!(plain[0], " UNC BASIC REV A      MORE: MANUAL CH. 2");
    assert!(!m.screen()[..40].contains(&0xA0), "the bar's gaps are $80, not $A0: those blink as the cursor");
    assert!(m.screen()[..40].iter().all(|&c| c >= 0x80), "the bar on top is reverse video, all 40 columns");
    assert_eq!(plain[2], "PROGRAMS");
    assert_eq!(m.screen()[80], b'P' | 0x80, "the headings are reverse video");
    assert_eq!(plain[3], " 10 PRINT \"HI\"   STORES THE LINE");
    assert_eq!(plain[15], "GRAPHICS");
    assert_eq!(plain[22], " RND(N) ABS(N) PEEK(A) POINT(X,Y) GET");
    assert_eq!(plain[23], "READY.");
    assert_eq!(cursors(&m), [24 * 40], "the only blinking cell is the cursor, on the last row");
    for kw in ["RUN", "LIST", "NEW", "LOAD", "SAVE", "PRINT", "INPUT", "LET", "IF", "THEN", "REM", "GOTO", "GOSUB",
        "RETURN", "END", "CLS", "FOR", "TO", "STEP", "NEXT", "POKE", "SYS", "GR", "TEXT", "PLOT", "LINE", "CIRCLE",
        "INK", "RND", "ABS", "PEEK", "POINT", "AND", "OR", "DATA", "READ", "RESTORE", "GET"] {
        assert!(plain.iter().any(|l| l.split(|c: char| !c.is_ascii_alphabetic()).any(|w| w == kw)), "HELP mentions {kw}");
    }
    // From graphics, HELP goes back to the text screen first: it needs all of it.
    gr(&mut m);
    line(&mut m, "HELP", 0);
    wait_ready(&mut m);
    assert_eq!(m.peek(io::VMODE) & VMODE_GFX, 0, "HELP leaves graphics, as TEXT does");
    assert_eq!(m.screen_text_plain().lines().next().map(|l| l.replace('?', " ")).as_deref(), Some(" UNC BASIC REV A      MORE: MANUAL CH. 2"));
    assert_eq!(direct(&mut m, "CLS"), Vec::<String>::new());
    assert_eq!(rows(&m)[0], "READY.");
}

#[test]
fn a_thousand_empty_loops_take_under_a_second() {
    let mut m = boot();
    type_keys(&mut m, b"FOR I=1 TO 1000:NEXT\r");
    let secs = wait_ready(&mut m) as f64 / 1e6;
    println!("FOR I=1 TO 1000:NEXT took {secs:.2} s");
    assert!(secs < 1.0, "{secs} s");
}

// ---------- the manual's BASIC listings, exactly as printed ----------

#[test]
fn manual_chapter_1_hello_loop_and_stop() {
    let mut m = boot();
    assert_eq!(direct(&mut m, "PRINT \"HELLO\""), ["HELLO"]);
    assert_eq!(direct(&mut m, "PRINT 6*7"), ["42"]);
    program(&mut m, &["10 PRINT \"HELLO\"", "20 GOTO 10"]);
    assert_eq!(direct(&mut m, "LIST"), ["10 PRINT \"HELLO\"", "20 GOTO 10"]);
    line(&mut m, "RUN", 40);
    assert!(rows(&m).iter().filter(|r| r.as_str() == "HELLO").count() >= 20, "the screen fills with HELLOs");
    m.key(3);
    wait_ready(&mut m);
    assert!(last_rows(&m, 2)[0].starts_with("BREAK IN "), "{:?}", last_rows(&m, 2));
}

#[test]
fn manual_chapter_1_counting_and_squares() {
    let mut m = boot();
    program(&mut m, &["NEW", "10 FOR I=1 TO 10", "20 PRINT I", "30 NEXT I"]);
    let want: Vec<String> = (1..=10).map(|i| i.to_string()).collect();
    assert_eq!(direct(&mut m, "RUN"), want);
    program(&mut m, &["20 PRINT I, I*I"]);
    let out = direct(&mut m, "RUN");
    assert_eq!(out[2], "3         9");
    assert_eq!(out[9], "10        100");
}

#[test]
fn manual_chapter_1_the_border_program() {
    let mut m = boot();
    program(&mut m, &["NEW", "10 FOR C=0 TO 15", "20 POKE 49184,C", "30 NEXT C", "40 GOTO 10"]);
    line(&mut m, "RUN", 2);
    let seen: std::collections::BTreeSet<u8> = m.border_lines.iter().copied().collect();
    assert!(seen.len() >= 8, "the border cycles through the colours: {seen:?}");
}

#[test]
fn manual_chapter_2_sys_example() {
    let mut m = boot();
    program(&mut m, &[
        "10 POKE 12288,40:POKE 12289,2",
        "20 POKE 12290,51:POKE 12291,32:POKE 12292,192",
        "30 POKE 12293,3",
        "40 SYS 12288",
    ]);
    assert_eq!(direct(&mut m, "RUN"), Vec::<String>::new());
    assert_eq!(m.peek(io::BORDER), 2);
}

#[test]
fn manual_chapter_2_speed_figures() {
    let mut m = boot();
    let time = |m: &mut Machine, text: &str| {
        type_keys(m, text.as_bytes());
        type_keys(m, b"\r");
        wait_ready(m) as f64 / 1e6
    };
    let empty = time(&mut m, "FOR I=1 TO 1000:NEXT");
    program(&mut m, &["10 FOR I=1 TO 100", "20 POKE 1024+X,32:X=X+1:POKE 1024+X,81", "30 NEXT I"]);
    let work = time(&mut m, "RUN") / 100.0;
    println!("1000 empty passes: {empty:.2} s; a line of three statements: {:.2} ms", work * 1000.0);
    assert!((0.3..0.7).contains(&empty), "half a second for 1,000 empty passes: {empty}");
    assert!((0.004..0.009).contains(&work), "a working line takes about 6 ms: {work}");
}

// ---------- graphics ----------

const VMODE_GFX: u8 = io::VMODE_BITMAP | io::VMODE_MIXED;

fn pixel(m: &Machine, x: i32, y: i32) -> bool {
    let base = (m.peek(io::BITMAP) as u16) << 8;
    m.peek(base + (y * 40 + x / 8) as u16) & (0x80 >> (x % 8)) != 0
}

fn lit(m: &Machine) -> Vec<(i32, i32)> {
    (0..200).flat_map(|y| (0..320).map(move |x| (x, y))).filter(|&(x, y)| pixel(m, x, y)).collect()
}

fn gr(m: &mut Machine) {
    direct(m, "GR");
}

#[test]
fn gr_and_text_switch_the_video_mode() {
    let mut m = boot();
    m.load(0x2000, &[0x5A; 8000]);
    gr(&mut m);
    assert_eq!(m.peek(io::VMODE) & VMODE_GFX, VMODE_GFX, "bitmap and mixed mode on");
    assert_eq!(m.peek(io::BITMAP), 0x20, "the bitmap is $2000-$3F3F on a UNC 16");
    assert!(lit(&m).is_empty(), "GR clears the bitmap");
    assert_eq!(rows(&m)[21], "READY.", "the cursor starts on row 21, the first visible text row");
    assert_eq!(cursors(&m), [22 * 40], "one cursor, under READY.: GR leaves none on row 0 or 21");
    assert_eq!(direct(&mut m, "PRINT 1+1"), ["2"], "BASIC works in the text rows");
    assert_eq!(direct(&mut m, "TEXT"), Vec::<String>::new());
    assert_eq!(m.peek(io::VMODE) & VMODE_GFX, 0);
    assert_eq!(rows(&m)[0], "READY.", "TEXT clears the screen");
    let mut m64 = Machine::new(RamSize::K64);
    for _ in 0..BOOT_FRAMES {
        m64.run_frame();
    }
    direct(&mut m64, "GR");
    assert_eq!(m64.peek(io::BITMAP), 0xA0, "on a UNC 64 the bitmap is $A000-$BF3F");
}

#[test]
fn cls_with_gr_on_clears_the_picture_too() {
    let mut m = boot();
    gr(&mut m);
    direct(&mut m, "CIRCLE 160,80,60");
    assert!(!lit(&m).is_empty(), "the circle is drawn");
    // On one line, as on the front page's demo, and on its own line.
    direct(&mut m, "CLS");
    assert!(lit(&m).is_empty(), "CLS clears the bitmap while GR is on");
    assert_eq!(m.peek(io::VMODE) & VMODE_GFX, VMODE_GFX, "and leaves GR on");
    assert_eq!(rows(&m)[21], "READY.", "the cursor goes back to row 21, as after GR");
    assert_eq!(cursors(&m), [22 * 40], "one cursor, under READY., waiting for a line");
    assert_eq!(direct(&mut m, "PRINT 1+1"), ["2"], "and typing works");
    assert_eq!(direct(&mut m, "CIRCLE 160,80,6:PRINT POINT(166,80)"), ["1"], "and so does drawing");
    direct(&mut m, "INK 0:CIRCLE 160,80,60:INK 1:GR:CIRCLE 160,80,6:CLS");
    assert!(lit(&m).is_empty(), "GR:CIRCLE ...:CLS leaves nothing behind");
    assert_eq!(direct(&mut m, "INK 2:CLS:PLOT 5,5:PLOT 5,5:PRINT POINT(5,5)"), ["0"], "CLS keeps the INK (2 flips twice)");
    assert_eq!(direct(&mut m, "TEXT"), Vec::<String>::new(), "TEXT gets out of graphics");
    assert_eq!(m.peek(io::VMODE) & VMODE_GFX, 0);
    assert_eq!(rows(&m)[0], "READY.");
    assert_eq!(direct(&mut m, "CLS"), Vec::<String>::new());
    assert_eq!(rows(&m)[0], "READY.", "and CLS in text mode is the plain CLS");
    // A bitmap moved somewhere else by POKE isn't GR's: CLS leaves it alone.
    let mut m = boot();
    m.load(0x1000, &[0x5A; 64]);
    direct(&mut m, &format!("POKE {},16:POKE {},6", io::BITMAP, io::VMODE));
    direct(&mut m, "CLS");
    assert_eq!(m.peek(0x1000), 0x5A, "a bitmap BASIC didn't place is not cleared");
}

#[test]
fn plot_sets_exactly_one_bit() {
    let mut m = boot();
    gr(&mut m);
    direct(&mut m, "PLOT 0,0:PLOT 319,199:PLOT 100,57");
    assert_eq!(lit(&m), [(0, 0), (100, 57), (319, 199)]);
    assert_eq!(m.peek(0x2000), 0x80, "bit 7 is the leftmost pixel");
    assert_eq!(m.peek(0x2000 + 57 * 40 + 12), 0x08, "x = 100 is byte 12, bit 3");
    assert_eq!(m.peek(0x2000 + 199 * 40 + 39), 0x01);
}

#[test]
fn line_draws_from_end_to_end() {
    let mut m = boot();
    gr(&mut m);
    direct(&mut m, "LINE 10,20,50,20");
    assert_eq!(lit(&m), (10..=50).map(|x| (x, 20)).collect::<Vec<_>>(), "horizontal, both ends included");
    gr(&mut m);
    direct(&mut m, "LINE 0,0,99,99");
    assert_eq!(lit(&m), (0..100).map(|i| (i, i)).collect::<Vec<_>>(), "a 45-degree diagonal");
    gr(&mut m);
    direct(&mut m, "LINE 30,150,20,110");
    let p = lit(&m);
    assert_eq!(p.len(), 41, "a steep line has one pixel per row");
    assert!(p.contains(&(30, 150)) && p.contains(&(20, 110)), "both ends, drawn backwards");
    gr(&mut m);
    direct(&mut m, "LINE 5,5,5,5");
    assert_eq!(lit(&m), [(5, 5)]);
}

#[test]
fn circle_is_symmetric_with_its_four_extremes() {
    let mut m = boot();
    gr(&mut m);
    direct(&mut m, "CIRCLE 160,100,50");
    let p = lit(&m);
    for e in [(210, 100), (110, 100), (160, 150), (160, 50)] {
        assert!(p.contains(&e), "{e:?}");
    }
    assert!(!p.contains(&(160, 100)), "not filled");
    for &(x, y) in &p {
        assert!(pixel(&m, 320 - x, y) && pixel(&m, x, 200 - y) && pixel(&m, 160 + (y - 100), 100 + (x - 160)), "({x},{y})");
        let d2 = (x - 160).pow(2) + (y - 100).pow(2);
        assert!((49 * 49..=51 * 51).contains(&d2), "({x},{y}) is on the circle");
    }
    gr(&mut m);
    direct(&mut m, "CIRCLE 10,10,0");
    assert_eq!(lit(&m), [(10, 10)], "radius 0 is a dot");
}

#[test]
fn ink_0_erases_and_ink_2_flips() {
    let mut m = boot();
    gr(&mut m);
    direct(&mut m, "LINE 0,0,100,0:INK 0:PLOT 50,0");
    assert!(!pixel(&m, 50, 0) && pixel(&m, 49, 0));
    gr(&mut m);
    direct(&mut m, "INK 2:PLOT 7,7");
    assert!(pixel(&m, 7, 7));
    direct(&mut m, "PLOT 7,7");
    assert!(!pixel(&m, 7, 7), "drawn twice in INK 2: gone");
    direct(&mut m, "CIRCLE 100,100,30:LINE 0,199,319,0:CIRCLE 50,50,1");
    assert!(!lit(&m).is_empty());
    direct(&mut m, "CIRCLE 100,100,30:LINE 0,199,319,0:CIRCLE 50,50,1");
    assert!(lit(&m).is_empty(), "every shape drawn twice in INK 2 vanishes: each pixel is drawn once");
    assert_eq!(direct(&mut m, "GR:PLOT 1,1:PRINT POINT(1,1)"), ["1"], "GR sets INK back to 1");
}

#[test]
fn point_reads_a_pixel() {
    let mut m = boot();
    gr(&mut m);
    assert_eq!(direct(&mut m, "PRINT POINT(5,5)"), ["0"]);
    direct(&mut m, "PLOT 5,5");
    assert_eq!(direct(&mut m, "?POINT(5,5);POINT(6,5);POINT(-1,0)"), ["100"]);
    assert_eq!(direct(&mut m, "?POINT(320,0);POINT(0,200)"), ["00"], "off the screen is 0");
    assert_eq!(direct(&mut m, "IF POINT(5,5) THEN PRINT \"HIT\""), ["HIT"]);
}

#[test]
fn drawing_off_the_screen_is_clipped_silently() {
    let mut m = boot();
    gr(&mut m);
    m.load(0x3F40, &[0xA5; 0xC0]);
    let below = (0..0x2000u16).filter(|a| !(0xD0..0x0800).contains(a)).map(|a| m.peek(a)).collect::<Vec<_>>();
    assert_eq!(direct(&mut m, "LINE -50,-50,400,300:CIRCLE 0,0,100"), Vec::<String>::new());
    assert_eq!(direct(&mut m, "PLOT 1000,1000:PLOT -1,5"), Vec::<String>::new());
    let p = lit(&m);
    assert!(p.contains(&(100, 0)) && p.contains(&(0, 100)), "the visible quarter of the circle is drawn");
    assert!(p.iter().any(|&(x, y)| y == 100 && (140..147).contains(&x)), "and the visible part of the line");
    assert!(m.mem.ram[0x3F40..0x4000].iter().all(|&b| b == 0xA5), "nothing past the bitmap");
    let after = (0..0x2000u16).filter(|a| !(0xD0..0x0800).contains(a)).map(|a| m.peek(a)).collect::<Vec<_>>();
    assert_eq!(below, after, "nothing below it either");
}

#[test]
fn the_bitmap_takes_the_top_8k_from_basic() {
    let mut m = boot();
    let body = "REM ".to_string() + &"X".repeat(110);
    for n in 1..=60 {
        line(&mut m, &format!("{n} {body}"), 0);
    }
    assert!(m.peek(0x020A) >= 0x20, "the program reaches past $2000");
    assert_eq!(direct(&mut m, "GR"), ["?OUT OF MEMORY"]);
    assert_eq!(m.peek(io::VMODE) & io::VMODE_BITMAP, 0, "and the bitmap stays off");
    line(&mut m, "NEW", 2);
    gr(&mut m);
    let mut n = 0;
    while !screen(&m).contains("?OUT OF MEMORY") && n < 100 {
        n += 1;
        line(&mut m, &format!("{n} {body}"), 0);
    }
    assert!((50..56).contains(&n), "about 6K of program fits below the bitmap: {n} lines");
    assert!(m.peek(0x020A) < 0x20, "the program stopped short of $2000");
    line(&mut m, "TEXT", 2);
    line(&mut m, &format!("{n} {body}"), 0);
    assert!(!screen(&m).contains("?OUT OF MEMORY"), "back in TEXT the memory is BASIC's again");
}

#[test]
fn mixed_mode_shows_the_text_rows_under_the_drawing() {
    let mut m = boot();
    gr(&mut m);
    direct(&mut m, "LINE 0,0,319,167:PRINT \"HI\"");
    let mut px = vec![0u8; 320 * 200];
    m.render(true, &mut px);
    assert_eq!(px[0], 1, "line 0 is the bitmap");
    assert_eq!(px[167 * 320 + 319], 1);
    assert!(rows(&m)[21..].iter().any(|r| r == "HI"), "{:?}", &rows(&m)[21..]);
    assert!(px[168 * 320..].contains(&1), "the text rows below show text");
}

#[test]
fn graphics_speed_figures() {
    let mut m = boot();
    let mut time = |text: &str, n: f64| {
        type_keys(&mut m, text.as_bytes());
        type_keys(&mut m, b"\r");
        wait_ready(&mut m) as f64 / 1e6 / n
    };
    let gr_t = time("GR", 1.0);
    let empty = time("FOR I=1 TO 20:NEXT", 20.0);
    let long = time("FOR I=1 TO 20:LINE 0,0,319,199:NEXT", 20.0) - empty;
    let short = time("FOR I=1 TO 20:LINE 0,0,50,0:NEXT", 20.0) - empty;
    let circ = time("FOR I=1 TO 20:CIRCLE 160,100,90:NEXT", 20.0) - empty;
    let small = time("FOR I=1 TO 20:CIRCLE 160,100,10:NEXT", 20.0) - empty;
    let plot = time("FOR I=1 TO 200:PLOT I,I:NEXT", 200.0) - empty;
    println!(
        "GR {:.0} ms; LINE 0,0,319,199 {:.0} ms; LINE 51 long {:.1} ms; CIRCLE r=90 {:.0} ms; r=10 {:.1} ms; PLOT {:.1} ms",
        gr_t * 1e3, long * 1e3, short * 1e3, circ * 1e3, small * 1e3, plot * 1e3
    );
    assert!(long < 0.15 && circ < 0.15, "the biggest line and circle take a few frames: {long} {circ}");
    assert!(short < 0.03 && small < 0.03, "everyday sizes are instant: {short} {small}");
}

#[test]
fn manual_chapter_2_sun_over_a_house() {
    let mut m = boot();
    program(&mut m, &[
        "10 GR",
        "20 CIRCLE 250,40,22",
        "30 LINE 250,10,250,2:LINE 250,70,250,78",
        "40 LINE 220,40,212,40:LINE 280,40,288,40",
        "50 LINE 229,19,223,13:LINE 271,19,277,13",
        "60 LINE 229,61,223,67:LINE 271,61,277,67",
        "70 LINE 60,100,160,100:LINE 160,100,160,160",
        "80 LINE 50,100,110,55:LINE 110,55,170,100",
        "90 CIRCLE 110,80,6",
        "100 LINE 95,160,95,125:LINE 95,125,115,125:LINE 115,125,115,160",
        "110 LINE 125,110,150,110:LINE 150,110,150,135",
        "120 LINE 150,135,125,135:LINE 125,135,125,110",
        "130 LINE 60,100,60,160:LINE 0,160,319,160",
    ]);
    assert_eq!(direct(&mut m, "RUN"), Vec::<String>::new());
    for p in [(272, 40), (250, 2), (110, 55), (60, 130), (0, 160), (319, 160), (104, 80)] {
        assert!(pixel(&m, p.0, p.1), "{p:?}");
    }
}


#[test]
fn read_takes_data_in_order_across_lines() {
    let mut m = boot();
    program(&mut m, &[
        "10 DATA 3, -7, $FF",
        "20 FOR I=1 TO 2:READ A,B:PRINT A;\" \";B:NEXT I",
        "30 PRINT \"END\"",
        "40 DATA 10+5,0",
        "50 X=1:DATA 9",
    ]);
    assert_eq!(direct(&mut m, "RUN"), ["3 -7", "255 15", "END"], "items are expressions; DATA lines run as nothing");
    // the rest of the DATA, then past the end
    assert_eq!(direct(&mut m, "READ A,B:PRINT A;B"), ["09"], "READ goes on where the program left off");
    assert_eq!(direct(&mut m, "READ A"), ["?OUT OF DATA"]);
}

#[test]
fn restore_and_run_start_the_data_again() {
    let mut m = boot();
    program(&mut m, &["10 READ A:PRINT A", "20 RESTORE:READ B:PRINT B", "30 DATA 42,43"]);
    assert_eq!(direct(&mut m, "RUN"), ["42", "42"]);
    assert_eq!(direct(&mut m, "RUN"), ["42", "42"], "RUN restores");
    assert_eq!(direct(&mut m, "READ C:PRINT C"), ["43"]);
    assert_eq!(direct(&mut m, "RESTORE:READ C:PRINT C"), ["42"]);
    // LIST spells the new keywords out
    let listed = direct(&mut m, "LIST 20-30");
    assert_eq!(listed, ["20 RESTORE:READ B:PRINT B", "30 DATA 42,43"]);
}

#[test]
fn data_errors_point_at_the_read_line() {
    let mut m = boot();
    program(&mut m, &["10 READ A", "20 DATA 1 2"]);
    assert_eq!(direct(&mut m, "RUN"), ["?SYNTAX ERROR IN 10"]);
    let mut m = boot();
    program(&mut m, &["10 READ A", "20 PRINT A"]);
    assert_eq!(direct(&mut m, "RUN"), ["?OUT OF DATA IN 10"]);
}

#[test]
fn get_takes_a_key_without_waiting() {
    let mut m = boot();
    assert_eq!(direct(&mut m, "PRINT GET"), ["0"], "no key waiting: 0, at once");
    // a program that polls: it runs on while no key is down, and sees the key when one is
    program(&mut m, &["10 K=GET:N=N+1:IF K=0 THEN 10", "20 PRINT K;N>1"]);
    line(&mut m, "RUN", 5);
    m.key(b'Q');
    wait_ready(&mut m);
    let out = last_rows(&m, 2);
    assert_eq!(out, ["81-1", "READY."], "Q is 81, and the loop went round more than once (true is -1)");
}

/// The manual's DATA listing (chapter 2, Statements): eight bytes of a shape, read and poked.
#[test]
fn the_manuals_data_listing_pokes_its_table() {
    let mut m = boot();
    program(&mut m, &["10 FOR I=0 TO 7:READ B:POKE 12288+I,B:NEXT I", "20 DATA 24,60,126,255,24,24,24,24"]);
    assert!(direct(&mut m, "RUN").is_empty());
    let got: Vec<u8> = (0..8).map(|i| m.peek(12288 + i)).collect();
    assert_eq!(got, [24, 60, 126, 255, 24, 24, 24, 24]);
}
