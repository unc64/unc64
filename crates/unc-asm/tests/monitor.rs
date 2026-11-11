//! The monitor in the 4K ROM (manual chapter 16): MON, the * prompt, bytes shown and stored,
//! instructions listed, and a routine called.

use unc_core::machine::io;
use unc_core::{Machine, RamSize};

fn type_keys(m: &mut Machine, keys: &[u8]) {
    for &k in keys {
        m.key(k);
        for _ in 0..400 {
            m.run_cycles(500);
            if m.peek(io::KBD) & 0x80 == 0 {
                break;
            }
        }
        m.key_up(k);
    }
}

/// Runs until the cursor is back and nothing on the screen is moving.
fn settle(m: &mut Machine) {
    for _ in 0..2000 {
        let before = m.screen().to_vec();
        m.run_cycles(5_000);
        if m.screen() == &before[..] && m.screen().iter().filter(|&&c| c == 0xA0).count() == 1 {
            return;
        }
    }
    panic!("never came back for a key: {:?}", rows(m));
}

fn line(m: &mut Machine, text: &str) {
    type_keys(m, text.as_bytes());
    type_keys(m, b"\r");
    settle(m);
}

fn rows(m: &Machine) -> Vec<String> {
    m.screen_text().lines().map(str::to_string).filter(|l| !l.is_empty() && l != "_").collect()
}

/// What a monitor line printed: the rows after the line as typed, up to the next prompt.
fn said(m: &mut Machine, text: &str) -> Vec<String> {
    line(m, "Q");
    line(m, "CLS");
    line(m, "MON");
    line(m, text);
    let r = rows(m);
    let at = r.iter().position(|l| *l == format!("*{text}")).unwrap_or_else(|| panic!("no *{text}: {r:?}"));
    let mut out = r[at + 1..].to_vec();
    assert_eq!(out.pop().as_deref(), Some("*_"), "the prompt comes back: {r:?}");
    out
}

fn monitor() -> Machine {
    let mut m = Machine::boot(RamSize::K16);
    line(&mut m, "MON");
    assert_eq!(rows(&m).last().unwrap(), "*_", "MON gives the prompt");
    assert_eq!(m.peek(0x0204), 0, "and BASIC isn't running");
    m
}

#[test]
fn bytes_are_shown_stored_and_shown_again() {
    let mut m = monitor();
    m.load(0x1000, &[0xA9, 0x01, 0x8D, 0x20, 0xC0, 0x60, 0x11, 0x22, 0x33, 0x44]);
    assert_eq!(said(&mut m, "1000"), ["1000- A9"]);
    assert_eq!(said(&mut m, "1000.1009"), ["1000- A9 01 8D 20 C0 60 11 22", "1008- 33 44"]);
    assert_eq!(said(&mut m, "1003.100A"), ["1003- 20 C0 60 11 22", "1008- 33 44 00"], "a row ends where its address ends in 7 or F");
    assert_eq!(said(&mut m, "1000 1005"), ["1000- A9", "1005- 60"]);
    // store, and carry on storing
    assert_eq!(said(&mut m, "1010: DE AD"), Vec::<String>::new());
    // with CASE set the keyboard sends small letters, and they're hex and commands too
    m.poke(io::CASE, 1);
    line(&mut m, "1012: be ef");
    line(&mut m, ": 01 02");
    line(&mut m, "1010.1013");
    assert_eq!(rows(&m)[rows(&m).len() - 2], "1010- DE AD BE EF");
    m.poke(io::CASE, 0);
    assert_eq!(m.mem.ram[0x1010..0x1014], [0xDE, 0xAD, 0xBE, 0xEF]);
    assert_eq!(m.mem.ram[0x1014..0x1016], [1, 2], "a colon alone carried on where the last one stopped");
    // an address, then RETURN alone: the rest of its row, and the next row
    line(&mut m, "Q");
    line(&mut m, "CLS");
    line(&mut m, "MON");
    line(&mut m, "1012");
    line(&mut m, "");
    line(&mut m, "");
    assert_eq!(&rows(&m)[2..], ["*1012", "1012- BE", "*", "1013- EF 01 02 00 00", "*", "1018- 00 00 00 00 00 00 00 00", "*_"]);
    // nonsense gets a question mark and the prompt
    assert_eq!(said(&mut m, "1000Z"), ["?"]);
    // Q is BASIC again, with nothing lost
    line(&mut m, "Q");
    assert_eq!(rows(&m).last().unwrap(), "READY.");
    line(&mut m, "PRINT PEEK(4112)");
    assert!(rows(&m).contains(&"222".to_string()));
}

#[test]
fn every_opcode_lists_as_the_assembler_disassembles_it() {
    let mut m = monitor();
    for first in (0..256u16).step_by(16) {
        // sixteen opcodes at $2000, three bytes each, operands that tell their bytes apart
        for i in 0..16u16 {
            let op = (first + i) as u8;
            m.load(0x2000 + i * 3, &[op, 0x80 + i as u8, 0x12 + i as u8]);
        }
        let mut want = vec![];
        let mut at = 0x2000u16;
        while want.len() < 20 {
            let read = |a: u16| m.peek(a);
            let (text, len) = unc_asm::disasm(&read, at);
            let bytes: Vec<String> = (0..3).map(|k| if k < len { format!("{:02X}", m.peek(at + k)) } else { "  ".into() }).collect();
            let text = if text.starts_with(".byte") { "???".to_string() } else { text };
            want.push(format!("{at:04X}  {}  {text}", bytes.join(" ")).trim_end().to_string());
            at += len;
        }
        let got = said(&mut m, "2000L");
        assert_eq!(got, want, "opcodes from ${first:02X}");
    }
    // L alone carries on from where the last list stopped
    m.load(0x3000, &[0x01; 64]);
    line(&mut m, "Q");
    line(&mut m, "CLS");
    line(&mut m, "MON");
    line(&mut m, "3000L");
    line(&mut m, "L");
    let r = rows(&m);
    assert_eq!(r[r.len() - 2], "3027  01        NOP", "forty NOPs in two lists: {r:?}");
}

#[test]
fn go_calls_a_routine_and_its_rts_comes_back() {
    let mut m = monitor();
    // LDA #2, STA $C020, RTS
    line(&mut m, "3000: 28 02 33 20 C0 03");
    assert_eq!(said(&mut m, "3000L")[..3], ["3000  28 02     LDA #$02", "3002  33 20 C0  STA $C020", "3005  03        RTS"]);
    assert_eq!(said(&mut m, "3000G"), Vec::<String>::new());
    assert_eq!(m.peek(io::BORDER), 2, "the border is red");
    // G alone goes to the address last given
    m.poke(io::BORDER, 0);
    line(&mut m, "3000");
    line(&mut m, "G");
    assert_eq!(m.peek(io::BORDER), 2);
}

/// A BRK is as it was: the ROM's handler returns, and a program that takes the interrupt
/// vector gets its BRKs itself (chapter 10). The monitor is where MON takes you, nowhere else.
#[test]
fn a_brk_does_not_open_the_monitor() {
    let mut m = Machine::boot(RamSize::K16);
    m.load(0x3000, &[0x00, 0x03]); // BRK, RTS
    line(&mut m, "SYS 12288");
    assert_eq!(rows(&m).last().unwrap(), "READY.", "{:?}", rows(&m));
}

#[test]
fn stop_ends_a_long_list_and_mon_is_a_keyword() {
    let mut m = monitor();
    type_keys(&mut m, b"0.FFFF\r");
    for _ in 0..30 {
        m.run_frame();
    }
    assert_ne!(m.screen().iter().filter(|&&c| c == 0xA0).count(), 1, "still showing");
    type_keys(&mut m, &[3]);
    settle(&mut m);
    assert_eq!(rows(&m).last().unwrap(), "*_");
    line(&mut m, "Q");
    // MON is token $A7: in a program, after its line number
    line(&mut m, "10 mon");
    assert_eq!(m.mem.ram[0x0800..0x0805], [5, 10, 0, 0xA7, 0], "typed small, stored as its token");
    line(&mut m, "LIST");
    assert!(rows(&m).contains(&"10 MON".to_string()));
    line(&mut m, "RUN");
    assert_eq!(rows(&m).last().unwrap(), "*_");
}

/// Chapter 16's examples, as the machine prints them.
#[test]
fn manual_chapter_16_examples() {
    let mut m = Machine::boot(RamSize::K16);
    line(&mut m, "10 PRINT \"HI\"");
    line(&mut m, "20 GOTO 10");
    line(&mut m, "MON");
    let dump = said(&mut m, "0800.0817");
    assert_eq!(dump, ["0800- 0A 0A 00 80 20 22 48 49", "0808- 22 00 08 14 00 84 20 31", "0810- 30 00 00 00 00 00 00 00"]);
    assert_eq!(said(&mut m, "FF00"), ["FF00- 06"], "the jump table starts with a JMP");
    line(&mut m, "3000: 28 02 33 20 C0 03");
    assert_eq!(said(&mut m, "3000.3005"), ["3000- 28 02 33 20 C0 03"]);
    line(&mut m, "3000: 28 02");
    line(&mut m, ": 33 20 C0");
    line(&mut m, ": 03");
    assert_eq!(said(&mut m, "3000L")[..4], ["3000  28 02     LDA #$02", "3002  33 20 C0  STA $C020", "3005  03        RTS", "3006  00        BRK"]);
    assert_eq!(said(&mut m, "3000G"), Vec::<String>::new());
    assert_eq!(m.peek(io::BORDER), 2);
    line(&mut m, "C020: 5");
    assert_eq!(m.peek(io::BORDER), 5, "a store to I/O is a POKE");
    // a first program: stars for ever, until STOP and RESTORE; then another character
    line(&mut m, "3000: 28 2A 05 03 FF 06 00 30");
    assert_eq!(said(&mut m, "3000L")[..3], ["3000  28 2A     LDA #$2A", "3002  05 03 FF  JSR $FF03", "3005  06 00 30  JMP $3000"]);
    for (poke, ch) in [("3001: 2A", "*"), ("3001: 55", "U")] {
        line(&mut m, poke);
        type_keys(&mut m, b"3000G\r");
        for _ in 0..100 {
            m.run_frame();
        }
        assert!(m.screen_text().lines().filter(|l| *l == ch.repeat(40)).count() >= 20, "{ch} fills the screen: {}", m.screen_text());
        m.key(3);
        m.restore();
        settle(&mut m);
        m.key_up(3);
        assert_eq!(rows(&m).first().unwrap(), "READY.", "STOP and RESTORE: BASIC again");
        line(&mut m, "MON");
    }
    // from machine language: JMP $FF24
    line(&mut m, "Q");
    m.load(0x3100, &[0x06, 0x24, 0xFF]);
    line(&mut m, "SYS 12544");
    assert_eq!(rows(&m).last().unwrap(), "*_");
}
