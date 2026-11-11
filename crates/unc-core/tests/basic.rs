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

// ---------- the screen editor ----------

const LEFT: u8 = io::KEY_LEFT;
const RIGHT: u8 = io::KEY_RIGHT;
const UP: u8 = io::KEY_UP;
const DOWN: u8 = io::KEY_DOWN;
const INSERT: u8 = io::KEY_INSERT;

/// Presses and lets go of each key, as fingers do, and lets the editor finish with the last.
fn press(m: &mut Machine, keys: &[u8]) {
    for &k in keys {
        type_keys(m, &[k]);
        m.key_up(k);
    }
    settle(m);
}

/// Runs until nothing on the screen is moving and the cursor is back.
fn settle(m: &mut Machine) {
    for _ in 0..400 {
        let before = m.screen().to_vec();
        m.run_cycles(5_000);
        if m.screen() == &before[..] && cursors(m).len() == 1 {
            return;
        }
    }
    panic!("the editor never came back for a key: {:?}", rows(m));
}

/// The cursor to the first empty row under everything on the screen: where you'd type next.
fn clear_row(m: &mut Machine) {
    let y = rows(m).iter().rposition(|l| !l.is_empty() && l != "_").map_or(0, |y| y + 1);
    cursor_to(m, 0, y as u8);
}

/// The row of the screen that reads `text`.
fn row_of(m: &Machine, text: &str) -> u8 {
    rows(m).iter().position(|l| l == text).unwrap_or_else(|| panic!("no row {text:?}: {:?}", rows(m))) as u8
}

/// Moves the cursor to a column and row with the cursor keys.
fn cursor_to(m: &mut Machine, x: u8, y: u8) {
    let mut moves = 0;
    let mut step = |m: &mut Machine, k: u8| {
        press(m, &[k]);
        moves += 1;
        assert!(moves < 200, "the cursor won't go to {x},{y}: {:?}", rows(m));
    };
    while m.peek(CURY) > y {
        step(m, UP);
    }
    while m.peek(CURY) < y {
        step(m, DOWN);
    }
    while m.peek(CURX) > x {
        step(m, LEFT);
    }
    while m.peek(CURX) < x {
        step(m, RIGHT);
    }
    assert_eq!((m.peek(CURX), m.peek(CURY)), (x, y));
}

/// RETURN, and time for BASIC to take the line.
fn ret(m: &mut Machine) {
    press(m, b"\r");
}

/// CLS: READY. on row 0, and the cursor's row, 1.
fn cls(m: &mut Machine) -> u8 {
    direct(m, "CLS");
    assert_eq!(rows(m)[0], "READY.");
    m.peek(CURY)
}

/// The C64's defining nicety: cursor up into a LISTed line, type over it,
/// RETURN, and the program has the line as the screen shows it.
#[test]
fn a_listed_line_is_changed_where_it_stands() {
    let mut m = boot();
    program(&mut m, &["10 PRINT \"HELLO\"", "20 GOTO 10"]);
    direct(&mut m, "CLS");
    direct(&mut m, "LIST");
    let y = row_of(&m, "10 PRINT \"HELLO\"");
    cursor_to(&mut m, 10, y);
    press(&mut m, b"JELLY");
    ret(&mut m);
    assert_eq!((m.peek(CURX), m.peek(CURY)), (0, y + 1), "RETURN leaves the cursor on the next row");
    assert_eq!(rows(&m)[y as usize + 1], "_0 GOTO 10", "a stored line says nothing, and the cursor is on the next");
    clear_row(&mut m);
    direct(&mut m, "CLS");
    assert_eq!(direct(&mut m, "LIST"), ["10 PRINT \"JELLY\"", "20 GOTO 10"]);
    // a new number over an old line copies it: the C64's way to duplicate a line
    let y = row_of(&m, "20 GOTO 10");
    cursor_to(&mut m, 0, y);
    press(&mut m, b"3");
    ret(&mut m);
    clear_row(&mut m);
    direct(&mut m, "CLS");
    assert_eq!(direct(&mut m, "LIST"), ["10 PRINT \"JELLY\"", "20 GOTO 10", "30 GOTO 10"]);
}

/// INSERT opens a space at the cursor and DELETE closes one, each moving the rest of the
/// line; the cursor keys wrap from one row's edge to the next row's.
#[test]
fn insert_opens_a_space_and_delete_closes_one() {
    let mut m = boot();
    let y = cls(&mut m);
    let row = |m: &Machine| rows(m)[y as usize].clone();
    press(&mut m, b"10 PRINT 12");
    cursor_to(&mut m, 9, y);
    press(&mut m, &[INSERT, INSERT]);
    assert_eq!(row(&m), "10 PRINT _ 12", "two cells open under the cursor");
    press(&mut m, b"9+");
    assert_eq!(row(&m), "10 PRINT 9+_2");
    cursor_to(&mut m, 5, y);
    press(&mut m, &[8, 8]);
    assert_eq!(row(&m), "10 _NT 9+12", "DELETE takes the two before the cursor and pulls the rest left");
    press(&mut m, &[INSERT, INSERT]);
    press(&mut m, b"PR");
    ret(&mut m);
    clear_row(&mut m);
    assert_eq!(direct(&mut m, "LIST"), ["10 PRINT 9+12"]);
    // left from a row's first column goes to the last of the row above, and right comes back
    cursor_to(&mut m, 0, 5);
    press(&mut m, &[LEFT]);
    assert_eq!((m.peek(CURX), m.peek(CURY)), (39, 4));
    press(&mut m, &[RIGHT]);
    assert_eq!((m.peek(CURX), m.peek(CURY)), (0, 5));
    // and the screen's corners stop them
    cursor_to(&mut m, 0, 0);
    press(&mut m, &[LEFT, UP]);
    assert_eq!((m.peek(CURX), m.peek(CURY)), (0, 0));
    cursor_to(&mut m, 39, 24);
    press(&mut m, &[RIGHT, DOWN]);
    assert_eq!((m.peek(CURX), m.peek(CURY)), (39, 24));
    assert!(!screen(&m).contains("READY.\nREADY."), "and nothing scrolled: {:?}", rows(&m));
}

/// A line that wrapped is one line on any of its rows: RETURN on the second row takes
/// both, DELETE and INSERT move characters across the edge, and RETURN leaves the cursor
/// under the whole line.
#[test]
fn a_wrapped_line_is_one_line_on_any_of_its_rows() {
    let mut m = boot();
    let long = format!("10 PRINT {}1", "1+".repeat(20)); // 50 characters
    program(&mut m, &[&long, "20 END"]);
    direct(&mut m, "CLS");
    direct(&mut m, "LIST");
    let y = row_of(&m, &long[..40]);
    let row = |m: &Machine, n: u8| rows(m)[(y + n) as usize].clone();
    assert_eq!(row(&m, 1), &long[40..]);
    assert_eq!(row(&m, 2), "20 END");
    // on its second row, the last 1 becomes a 5
    cursor_to(&mut m, 9, y + 1);
    press(&mut m, b"5");
    ret(&mut m);
    assert_eq!((m.peek(CURX), m.peek(CURY)), (0, y + 2));
    clear_row(&mut m);
    direct(&mut m, "RUN");
    assert_eq!(last_rows(&m, 2), ["25", "READY."], "twenty 1s and a 5");
    // DELETE at the second row's first column takes the first row's last character, and
    // the second row moves left with it
    direct(&mut m, "CLS");
    direct(&mut m, "LIST");
    assert_eq!(row_of(&m, &long[..40]), y);
    cursor_to(&mut m, 0, y + 1);
    press(&mut m, &[8]);
    assert_eq!((m.peek(CURX), m.peek(CURY)), (39, y));
    cursor_to(&mut m, 0, y + 2);
    assert_eq!(row(&m, 0).len(), 40);
    assert_eq!(row(&m, 1).len(), 9, "the second row moved left by one");
    // INSERT at the line's start pushes one back over the edge
    cursor_to(&mut m, 9, y);
    press(&mut m, &[INSERT]);
    press(&mut m, &[DOWN, DOWN, DOWN, DOWN]);
    assert_eq!(&row(&m, 0)[..10], "10 PRINT  ");
    assert_eq!(row(&m, 1).len(), 10);
    // RETURN on the first row goes past both
    cursor_to(&mut m, 9, y);
    press(&mut m, b"2");
    ret(&mut m);
    assert_eq!((m.peek(CURX), m.peek(CURY)), (0, y + 2));
    assert_eq!(row(&m, 2), "_0 END", "the line below is untouched, the cursor on it");
}

/// Typing past the right edge, or INSERT on a full row, gives the line another row: the
/// rows below move down out of its way instead of being taken for part of it.
#[test]
fn a_line_that_grows_pushes_the_rows_below_down() {
    let mut m = boot();
    program(&mut m, &["10 REM", "20 END"]);
    direct(&mut m, "CLS");
    direct(&mut m, "LIST");
    let y = row_of(&m, "10 REM");
    let at = y as usize;
    assert_eq!(&rows(&m)[at..at + 4], ["10 REM", "20 END", "READY.", "_"]);
    cursor_to(&mut m, 6, y);
    press(&mut m, "X".repeat(34).as_bytes());
    assert_eq!((m.peek(CURX), m.peek(CURY)), (0, y + 1), "the cursor wrapped onto a row of the line's own");
    assert_eq!(&rows(&m)[at + 1..at + 4], ["_", "20 END", "READY."], "which the rows below made room for");
    press(&mut m, b"YZ");
    ret(&mut m);
    assert_eq!((m.peek(CURX), m.peek(CURY)), (0, y + 2));
    clear_row(&mut m);
    direct(&mut m, "CLS");
    assert_eq!(direct(&mut m, "LIST"), [&format!("10 REM{}", "X".repeat(34))[..], "YZ", "20 END"]);
    // INSERT on a row that is full does the same (a row a program filled: one that was
    // typed or listed has its second row already)
    let y = cls(&mut m) + 1;
    let at = y as usize;
    for x in 0..40 {
        m.poke(0x0400 + y as u16 * 40 + x, b'A');
    }
    press(&mut m, &[DOWN, DOWN]);
    press(&mut m, b"BELOW");
    cursor_to(&mut m, 7, y);
    press(&mut m, &[INSERT]);
    assert_eq!(rows(&m)[at], format!("AAAAAAA_{}", "A".repeat(32)));
    assert_eq!(rows(&m)[at + 1], "A", "a row of its own for it");
    assert_eq!(rows(&m)[at + 2], "BELOW");
    // a line is three rows at most: at the end of the third the cursor stays, and INSERT
    // on a full one does nothing
    let y = cls(&mut m);
    press(&mut m, "40 REM".as_bytes());
    press(&mut m, "B".repeat(114).as_bytes());
    assert_eq!((m.peek(CURX), m.peek(CURY)), (39, y + 2), "120 characters, and the cursor waits on the last");
    press(&mut m, b"C");
    assert_eq!((m.peek(CURX), m.peek(CURY)), (39, y + 2));
    cursor_to(&mut m, 6, y);
    press(&mut m, &[INSERT]);
    m.run_cycles(5_000);
    assert_eq!(rows(&m)[y as usize], format!("40 REM_{}", "B".repeat(33)), "full: INSERT opens nothing");
    ret(&mut m);
    assert_eq!((m.peek(CURX), m.peek(CURY)), (0, y + 3));
    clear_row(&mut m);
    direct(&mut m, "CLS");
    let listed = direct(&mut m, "LIST 40").concat();
    assert_eq!(listed, format!("40 REM{}C", "B".repeat(113)));
}

/// At the bottom of the screen a growing line moves everything up instead.
#[test]
fn a_line_that_grows_on_the_bottom_row_scrolls_the_screen() {
    let mut m = boot();
    for _ in 0..30 {
        press(&mut m, b"\r");
    }
    assert_eq!(m.peek(CURY), 24);
    press(&mut m, format!("PRINT {}5", "1+".repeat(20)).as_bytes());
    assert_eq!((m.peek(CURX), m.peek(CURY)), (7, 24));
    cursor_to(&mut m, 6, 23);
    press(&mut m, b"9\r");
    wait_ready(&mut m);
    assert_eq!(last_rows(&m, 2), ["33", "READY."], "9 + nineteen 1s + 5: {:?}", rows(&m));
}

/// RETURN takes whatever row the cursor is on. On READY. itself that's READ Y, as it was
/// on the machines this one remembers.
#[test]
fn return_on_any_row_runs_what_it_shows() {
    let mut m = boot();
    direct(&mut m, "CLS");
    direct(&mut m, "PRINT 1+1");
    let y = row_of(&m, "PRINT 1+1");
    cursor_to(&mut m, 8, y);
    press(&mut m, b"8\r");
    assert_eq!(rows(&m)[y as usize + 1], "9", "PRINT 1+8, its answer printed over the old one");
    let y = row_of(&m, "READY.");
    cursor_to(&mut m, 3, y);
    press(&mut m, b"\r");
    assert_eq!(rows(&m)[y as usize + 1], "?OUT OF DATA", "{:?}", rows(&m));
    clear_row(&mut m);
    // reverse video reads as plain: HELP's headings are only words
    direct(&mut m, "HELP");
    let y = row_of(&m, " STOP (CTRL+C)   STOPS A PROGRAM");
    cursor_to(&mut m, 0, y);
    press(&mut m, b"\r");
    assert_eq!(rows(&m)[y as usize + 1], "?SYNTAX ERROR", "{:?}", rows(&m));
}

/// A line typed without moving the cursor is what was typed, whatever a program left on
/// the row: after a game, CLS is CLS. Move the cursor and the line is the screen's.
#[test]
fn a_typed_line_is_what_was_typed_on_a_row_a_program_left_full() {
    let mut m = boot();
    let y = cls(&mut m);
    let fill = |m: &mut Machine, row: u8| {
        for x in 6..40 {
            m.poke(0x0400 + row as u16 * 40 + x, b'#');
        }
    };
    for row in [y, y + 2, y + 4] {
        fill(&mut m, row); // READY. takes the rows between
    }
    press(&mut m, b"A=7");
    assert_eq!(&rows(&m)[y as usize][..10], "A=7_  ####", "typed in front of what's there");
    ret(&mut m);
    assert_eq!(m.peek(0x0260), 7, "A=7 is all BASIC got");
    // DELETE pulls the row left under the cursor, and it's still only what was typed
    press(&mut m, b"A=91");
    press(&mut m, &[8]);
    ret(&mut m);
    assert_eq!(m.peek(0x0260), 9);
    // a cursor key, and RETURN takes the row as it shows
    press(&mut m, b"A=5");
    assert_eq!(m.peek(CURY), y + 4);
    press(&mut m, &[LEFT]);
    ret(&mut m);
    assert_eq!(rows(&m)[y as usize + 5], "?SYNTAX ERROR", "A=5 and a row of #s: {:?}", rows(&m));
    assert_eq!(m.peek(0x0260), 5, "(the 5 went in first)");
}

/// INPUT is the same editor: its line is read from after the prompt, and a number put
/// right with the cursor keys is the number it gets.
#[test]
fn input_reads_its_line_from_after_the_prompt() {
    let mut m = boot();
    program(&mut m, &["10 INPUT \"HOW MANY 2\";A", "20 PRINT A*2"]);
    line(&mut m, "RUN", 3);
    press(&mut m, b"1234");
    press(&mut m, &[LEFT, LEFT, LEFT, 8, RIGHT, INSERT]);
    press(&mut m, b"0\r");
    wait_ready(&mut m);
    assert_eq!(last_rows(&m, 3), ["HOW MANY 2? 2034", "4068", "READY."]);
}

/// A game steered with the cursor keys ends with one held. READY. waits for it to be let
/// go and forgets it, so the cursor is where READY. left it.
#[test]
fn a_cursor_key_held_when_a_program_ends_does_not_move_the_cursor() {
    let mut m = boot();
    program(&mut m, &["10 IF PEEK(49170)<>30 THEN 10"]);
    line(&mut m, "RUN", 3);
    m.key(UP);
    for _ in 0..20 {
        m.run_frame();
    }
    assert_eq!(m.peek(RUNFLG), 0, "the program is over");
    assert!(cursors(&m).is_empty(), "no cursor while the key is down");
    m.key_up(UP);
    wait_ready(&mut m);
    assert_eq!(m.peek(CURX), 0);
    assert_eq!(rows(&m)[m.peek(CURY) as usize - 1], "READY.", "the cursor is under READY.");
    // a letter typed ahead is kept
    line(&mut m, "RUN", 3);
    m.key(UP);
    m.run_frame();
    m.key_up(UP);
    press(&mut m, b"L");
    press(&mut m, b"IST\r");
    wait_ready(&mut m);
    assert_eq!(last_rows(&m, 3), ["LIST", "10 IF PEEK(49170)<>30 THEN 10", "READY."]);
}

/// Under GR the cursor stays on the four rows you can see.
#[test]
fn under_gr_the_cursor_stays_on_the_text_rows() {
    let mut m = boot();
    direct(&mut m, "GR");
    for _ in 0..6 {
        press(&mut m, &[UP, LEFT]);
    }
    assert_eq!((m.peek(CURX), m.peek(CURY)), (0, 21));
    direct(&mut m, "TEXT");
}

// ---------- small letters ----------

/// The keyboard sends capitals until CASE ($C013) says otherwise. With it set, small letters
/// arrive as typed, and BASIC takes them for capitals everywhere but inside quotes and after
/// REM: keywords, variables and hex digits are the same either way.
#[test]
fn basic_takes_small_letters_for_capitals_outside_quotes() {
    let mut m = boot();
    // as switched on: whatever the host sends, a letter is a capital
    type_keys(&mut m, b"print \"hello\"\r");
    wait_ready(&mut m);
    assert_eq!(last_rows(&m, 3), ["PRINT \"HELLO\"", "HELLO", "READY."]);
    assert_eq!(m.peek(io::CASE), 0);
    // a deliberate POKE, and letters arrive as typed
    direct(&mut m, "POKE 49171,1");
    assert_eq!(m.peek(io::CASE), 1);
    type_keys(&mut m, b"print \"Hello, World\"\r");
    wait_ready(&mut m);
    assert_eq!(last_rows(&m, 3), ["print \"Hello, World\"", "Hello, World", "READY."], "the line stays as typed; what runs is PRINT");
    direct(&mut m, "new");
    for l in ["10 rem Mixed Case stays", "20 a=$c020:poke a,2:b=Peek(a)", "30 Print \"b is\";b:if b=2 Then print \"ok\""] {
        line(&mut m, l, 2);
    }
    assert_eq!(direct(&mut m, "list"), ["10 REM Mixed Case stays", "20 A=$C020:POKE A,2:B=PEEK(A)", "30 PRINT \"b is\";B:IF B=2 THEN PRINT \"ok\""]);
    assert_eq!(direct(&mut m, "run"), ["b is2", "ok"]);
    assert_eq!(m.peek(0x0260 + 2), 2, "b is B");
    // STOP and RESTORE put the keyboard back to capitals
    stop_restore(&mut m);
    assert_eq!(m.peek(io::CASE), 0);
    type_keys(&mut m, b"print 1\r");
    wait_ready(&mut m);
    assert_eq!(last_rows(&m, 3), ["PRINT 1", "1", "READY."]);
}

// ---------- STOP and RESTORE ----------

/// STOP held, and RESTORE pressed: the NMI.
fn stop_restore(m: &mut Machine) {
    m.key_matrix(3, true);
    m.restore();
    for _ in 0..3 {
        m.run_frame();
    }
    m.key_matrix(3, false);
    settle(m);
}

/// STOP and RESTORE get BASIC back from a program STOP can't reach: machine code with
/// interrupts off, going round for ever. The screen is cleared, the machine's registers
/// are as at switch-on, and the program and its variables are still there.
#[test]
fn stop_and_restore_come_back_from_anything() {
    let mut m = boot();
    // SEI, JMP * at $3000; a border, a hooked interrupt vector, and the speaker's gate on
    program(&mut m, &[
        "10 A=42:POKE 49184,5:POKE 49185,6:POKE 788,0:POKE 789,48:POKE 49201,3",
        "20 POKE 12288,33:POKE 12289,6:POKE 12290,1:POKE 12291,48:SYS 12288",
    ]);
    line(&mut m, "RUN", 5);
    assert_eq!(m.cpu.pc, 0x3001, "going round at $3001");
    m.key(3);
    for _ in 0..5 {
        m.run_frame();
    }
    m.key_up(3);
    assert_eq!(m.cpu.pc, 0x3001, "STOP alone is only a key, and nobody is reading the keys");
    stop_restore(&mut m);
    assert_eq!(rows(&m)[0], "READY.", "a clear screen and READY.: {:?}", rows(&m));
    assert_eq!((m.peek(CURX), m.peek(CURY)), (0, 1));
    assert_eq!(m.peek(RUNFLG), 0);
    assert_eq!((m.peek(io::BORDER), m.peek(io::BACKGROUND), m.peek(io::SPEAKER) & 3), (0, 0, 0));
    assert_ne!(m.peek(0x0315), 0x30, "the interrupt vector is the ROM's again");
    assert_eq!(direct(&mut m, "PRINT A"), ["42"], "the variables are as they were");
    assert!(direct(&mut m, "LIST").concat().ends_with("12291,48:SYS 12288"), "and so is the program");
    // from BASIC's graphics too: back to text
    direct(&mut m, "GR");
    program(&mut m, &["NEW", "10 GOTO 10"]);
    line(&mut m, "RUN", 3);
    stop_restore(&mut m);
    assert_eq!(m.peek(io::VMODE), 0, "the text screen");
    assert_eq!(rows(&m)[0], "READY.");
    assert_eq!(direct(&mut m, "LIST"), ["10 GOTO 10"]);
}

/// The uncle's three lines (the game's chapter 9c, ch09c_restore.gd): a JMP to itself at
/// $1000, called with SYS. STOP can't reach it. The switch gets BASIC back without them;
/// STOP and RESTORE get it back with them.
#[test]
fn his_three_lines_lock_up_and_only_restore_keeps_them() {
    const LINES: [&str; 3] = ["10 POKE 4096,6:POKE 4097,0:POKE 4098,16", "20 PRINT \"BACK IN A MOMENT.\"", "30 SYS 4096"];
    assert!(LINES.iter().all(|l| l.len() < 40), "each fits a row of the screen");
    let mut m = boot();
    program(&mut m, &LINES);
    line(&mut m, "RUN", 5);
    assert_eq!(last_rows(&m, 1), ["BACK IN A MOMENT."]);
    assert!(cursors(&m).is_empty(), "no cursor");
    let reads = |m: &mut Machine| {
        m.observe();
        for _ in 0..100 {
            m.run_frame();
        }
        let o = m.observe();
        o.read_count(io::KBD) + o.read_count(io::KBD_STROBE)
    };
    m.key(3);
    assert_eq!(reads(&mut m), 0, "two seconds, and nothing reads the keys: STOP is never seen");
    m.key_up(3);
    assert!((0x1000..0x1003).contains(&m.cpu.pc) && m.peek(RUNFLG) != 0, "still going round");
    // the switch: BASIC is back, and the three lines aren't
    let mut off_and_on = boot();
    assert_eq!(direct(&mut off_and_on, "LIST"), Vec::<String>::new());
    // STOP and RESTORE: BASIC is back, and so are they
    stop_restore(&mut m);
    assert_eq!(direct(&mut m, "LIST"), LINES);
}

/// RESTORE without STOP does nothing: the handler returns and the program carries on.
#[test]
fn restore_alone_does_nothing() {
    let mut m = boot();
    program(&mut m, &["10 A=A+1:GOTO 10"]);
    line(&mut m, "RUN", 3);
    let a = m.peek(0x0260);
    m.restore();
    for _ in 0..3 {
        m.run_frame();
    }
    assert_ne!(m.peek(RUNFLG), 0, "still running");
    assert_ne!(m.peek(0x0260), a, "and counting");
    // at READY. too: nothing moves
    type_keys(&mut m, &[3]);
    m.key_up(3);
    wait_ready(&mut m);
    let before = m.screen().to_vec();
    m.restore();
    m.run_frame();
    settle(&mut m);
    assert_eq!(m.screen(), &before[..]);
}

/// The NMI wakes a processor stopped by HLT. RESTORE alone goes back to the HLT, so it
/// stays stopped; with STOP it's BASIC again. A jammed processor needs the reset switch.
#[test]
fn restore_wakes_a_halted_processor_but_not_a_jammed_one() {
    let mut m = boot();
    program(&mut m, &["10 A=7:POKE 12288,2:SYS 12288"]);
    line(&mut m, "RUN", 5);
    assert_eq!(m.cpu.state, unc_core::CpuState::Halted);
    m.restore();
    for _ in 0..2 {
        m.run_frame();
    }
    assert_eq!(m.cpu.state, unc_core::CpuState::Halted, "RESTORE alone: the HLT again");
    assert_eq!(m.cpu.pc, 0x3001);
    stop_restore(&mut m);
    assert_eq!(m.cpu.state, unc_core::CpuState::Running);
    assert_eq!(rows(&m)[0], "READY.");
    assert_eq!(direct(&mut m, "PRINT A"), ["7"]);
    // $FF is no instruction
    program(&mut m, &["10 POKE 12288,255:SYS 12288"]);
    line(&mut m, "RUN", 5);
    assert!(matches!(m.cpu.state, unc_core::CpuState::Jammed(0xFF)));
    stop_restore_jammed(&mut m);
    assert!(matches!(m.cpu.state, unc_core::CpuState::Jammed(0xFF)), "only a reset");
}

fn stop_restore_jammed(m: &mut Machine) {
    m.key_matrix(3, true);
    m.restore();
    for _ in 0..3 {
        m.run_frame();
    }
    m.key_matrix(3, false);
}

/// The NMI goes through NMIVEC ($0316), so a program can have RESTORE for itself; it takes
/// six cycles to get there and doesn't ask the I flag.
#[test]
fn a_program_can_hook_restore() {
    let mut m = boot();
    // $3000: SEI, JMP *. $3010: INC $C020, RTI (opcodes from Appendix C)
    let inc_abs = unc_core::isa::opcode_for(unc_core::Op::Inc, unc_core::Mode::Abs).unwrap();
    for (i, b) in [0x21u8, 0x06, 0x01, 0x30].into_iter().enumerate() {
        m.poke(0x3000 + i as u16, b);
    }
    for (i, b) in [inc_abs, 0x20, 0xC0, 0x04].into_iter().enumerate() {
        m.poke(0x3010 + i as u16, b);
    }
    m.poke(0x0316, 0x10);
    m.poke(0x0317, 0x30);
    line(&mut m, "SYS 12288", 2);
    assert_eq!(m.cpu.pc, 0x3001);
    assert_eq!(m.peek(io::BORDER), 0);
    let sp = m.cpu.sp;
    m.restore();
    let t = m.cpu.cycles;
    m.run_cycles(1);
    assert_eq!(m.cpu.cycles - t, 6, "six cycles in, as an IRQ takes");
    assert_eq!(m.cpu.sp, sp.wrapping_sub(3), "PC and the flags are on the stack");
    m.run_frame();
    assert_eq!(m.peek(io::BORDER), 1, "the program's own handler ran, once, with I set");
    assert_eq!((m.cpu.pc, m.cpu.sp), (0x3001, sp), "and RTI went back");
    // STOP and RESTORE is the program's too, while it holds the vector
    m.key_matrix(3, true);
    m.restore();
    m.run_frame();
    m.key_matrix(3, false);
    assert_eq!((m.peek(io::BORDER), m.cpu.pc), (2, 0x3001));
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
    assert_eq!(m.peek(0x0260), 5, "A = 5: the line is what was typed, not the X after it");
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
    // seven or eight in a frame, by where in the loop the frame begins
    assert!(seen.len() >= 7, "the border cycles through the colours: {seen:?}");
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

/// The picture as the beam draws it now.
fn picture(m: &Machine) -> Vec<u8> {
    let mut px = vec![0u8; 320 * 200];
    m.render(true, &mut px);
    px
}

/// GR takes the number of text rows under the picture: four if it isn't given, none for
/// the whole screen, one for a status line. TEXTROWS ($C024) is where it goes.
#[test]
fn gr_takes_the_number_of_text_rows() {
    let mut m = boot();
    assert_eq!(m.peek(io::TEXTROWS), 4, "four at reset");
    // the whole screen: the bottom line of the bitmap shows, and no text at all
    program(&mut m, &["10 GR 0:LINE 0,199,319,199:LINE 0,0,319,0:PRINT \"UNSEEN\"", "20 GOTO 20"]);
    line(&mut m, "RUN", 30);
    assert_eq!(m.peek(io::TEXTROWS), 0);
    let px = picture(&m);
    assert!(px[199 * 320..].iter().all(|&p| p == 1), "line 199 is the bitmap's");
    assert!(px[..320].iter().all(|&p| p == 1));
    assert_eq!(px[320..199 * 320].iter().filter(|&&p| p != 0).count(), 0, "and nothing else: the text is under it");
    assert!(screen(&m).contains("UNSEEN"), "(it's in screen memory all the same)");
    // STOP: READY. needs somewhere to be seen, so the four rows are back
    type_keys(&mut m, &[3]);
    m.key_up(3);
    wait_ready(&mut m);
    assert_eq!(m.peek(io::TEXTROWS), 4, "READY. brings GR's four rows back");
    assert_eq!(m.peek(io::VMODE) & VMODE_GFX, VMODE_GFX, "still graphics");
    assert!(m.peek(CURY) >= 21, "and the cursor is on one of them");
    assert!(picture(&m)[168 * 320..].contains(&1), "READY. shows");
    // one row: a status line under 192 lines of picture
    direct(&mut m, "TEXT");
    program(&mut m, &["10 GR 1:LINE 0,191,319,191:PRINT \"SCORE 100\";", "20 GOTO 20"]);
    line(&mut m, "RUN", 30);
    assert_eq!(m.peek(io::TEXTROWS), 1);
    assert_eq!(rows(&m)[24], "SCORE 100", "PRINT starts on the row that shows");
    let px = picture(&m);
    assert!(px[191 * 320..192 * 320].iter().all(|&p| p == 1), "line 191 is still picture");
    let status = &px[192 * 320..];
    assert!(status.contains(&1) && status.contains(&0) && status[9 * 8..320].iter().step_by(8).all(|&p| p == 0), "then eight lines of text");
    type_keys(&mut m, &[3]);
    m.key_up(3);
    wait_ready(&mut m);
    assert_eq!(m.peek(io::TEXTROWS), 4, "and four again at READY.");
    // any expression, and CLS keeps the count
    direct(&mut m, "TEXT");
    program(&mut m, &["10 N=3:GR N*2:CLS:PRINT \"TOP\"", "20 GOTO 20"]);
    line(&mut m, "RUN", 20);
    assert_eq!(m.peek(io::TEXTROWS), 6);
    assert_eq!(rows(&m)[19], "TOP", "six rows: the first is row 19");
    type_keys(&mut m, &[3]);
    m.key_up(3);
    wait_ready(&mut m);
    assert_eq!(m.peek(io::TEXTROWS), 6, "two rows or more stay as they are");
    // GR alone is four, as it always was
    assert_eq!(direct(&mut m, "GR"), Vec::<String>::new());
    assert_eq!((m.peek(io::TEXTROWS), m.peek(CURY)), (4, 22));
    direct(&mut m, "TEXT");
}

/// Chapter 2's listing: nine rings over a status line.
#[test]
fn manual_chapter_2_nine_rings_over_a_status_line() {
    let mut m = boot();
    program(&mut m, &["10 GR 1", "20 FOR R=10 TO 90 STEP 10:CIRCLE 160,96,R:NEXT", "30 PRINT \"NINE RINGS. STOP TO END.\";", "40 GOTO 40"]);
    line(&mut m, "RUN", 60);
    assert_eq!(rows(&m)[24], "NINE RINGS. STOP TO END.");
    assert!(pixel(&m, 160, 6) && pixel(&m, 160, 186) && pixel(&m, 250, 96), "the outer ring, top, bottom and right");
    let px = picture(&m);
    assert!(px[186 * 320 + 160] == 1 && px[192 * 320..].contains(&1), "all of it shows, above the line of text");
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
