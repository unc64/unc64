use unc_core::isa::{Mode, Op, opcode_for};
use unc_core::machine::{BOOT_FRAMES, Machine, RamSize, Stop};

fn op(o: Op, m: Mode) -> u8 {
    opcode_for(o, m).unwrap()
}

fn boot(size: RamSize) -> Machine {
    let mut m = Machine::new(size);
    for _ in 0..BOOT_FRAMES {
        m.run_frame();
    }
    m
}

fn type_keys(m: &mut Machine, keys: &[u8]) {
    for &k in keys {
        m.key(k);
        m.run_frame();
        m.run_frame();
    }
}

fn row(m: &Machine, r: usize) -> String {
    m.screen_text().lines().nth(r).unwrap_or("").to_string()
}

#[test]
fn boots_to_the_banner_on_a_16k_machine() {
    let m = boot(RamSize::K16);
    let text = m.screen_text();
    assert!(text.contains("UNC 64 SYSTEM ROM  REV A"), "{text}");
    assert!(text.contains("16K RAM SYSTEM"), "{text}");
    assert!(text.contains("READY."), "{text}");
}

#[test]
fn detects_the_64k_expansion() {
    let m = boot(RamSize::K64);
    assert!(m.screen_text().contains("64K RAM SYSTEM"));
    assert_eq!(m.peek(0x0203), 0xC0, "RAMTOP");
}

#[test]
fn echoes_typed_keys_with_a_cursor() {
    let mut m = boot(RamSize::K16);
    type_keys(&mut m, b"HELLO");
    let r = m.screen_text().lines().position(|l| l.starts_with("HELLO")).expect("HELLO on screen");
    assert_eq!(row(&m, r), "HELLO_");
}

#[test]
fn return_starts_a_new_line_and_backspace_erases() {
    let mut m = boot(RamSize::K16);
    type_keys(&mut m, b"AB\x08C");
    assert!(m.screen_text().contains("\nAC_"), "{}", m.screen_text());
    type_keys(&mut m, b"\r");
    let text = m.screen_text();
    assert!(text.contains("\nAC\n?SYNTAX ERROR\nREADY.\n_"), "RETURN hands the line to BASIC: {text}");
}

#[test]
fn printing_past_the_bottom_scrolls() {
    let mut m = boot(RamSize::K16);
    type_keys(&mut m, b"FOR I=1 TO 30:PRINT I:NEXT\r");
    for _ in 0..30 {
        m.run_frame();
    }
    let text = m.screen_text();
    assert_eq!(text.lines().count(), 25);
    assert_eq!(row(&m, 21), "29");
    assert_eq!(row(&m, 22), "30");
    assert_eq!(row(&m, 23), "READY.");
    assert_eq!(row(&m, 24), "_");
    assert!(!text.contains("UNC 64 SYSTEM ROM"), "the banner scrolled away");
}

#[test]
fn programs_print_through_the_jump_table() {
    let mut m = boot(RamSize::K16);
    let mut code = vec![op(Op::Lda, Mode::Imm), 12, op(Op::Jsr, Mode::Abs), 0x03, 0xFF];
    for &c in b"HI " {
        code.extend([op(Op::Lda, Mode::Imm), c, op(Op::Jsr, Mode::Abs), 0x03, 0xFF]);
    }
    code.extend([op(Op::Lda, Mode::Imm), 0xA7, op(Op::Jsr, Mode::Abs), 0x0C, 0xFF, op(Op::Hlt, Mode::Imp)]);
    m.load(0x0800, &code);
    m.cpu.pc = 0x0800;
    assert_eq!(m.run_cycles(1_000_000), Stop::Halted);
    assert_eq!(row(&m, 0), "HI A7", "printing leaves no cursor behind");
}

#[test]
fn jump_table_entries_are_jmp_instructions() {
    let m = Machine::new(RamSize::K16);
    // CLS, CHROUT, GETKEY, PRSTR, PRHEX, PRDEC, RND, LOAD, NUMSTR, PRNUM, RNDSEED
    for i in 0..11u16 {
        assert_eq!(m.peek(0xFF00 + i * 3), op(Op::Jmp, Mode::Abs));
    }
}

#[test]
fn default_irq_handler_returns_so_interrupts_are_harmless() {
    let mut m = boot(RamSize::K16);
    m.poke(0xC004, 0x02);
    for _ in 0..5 {
        assert_eq!(m.run_frame(), Stop::Budget);
        m.poke(0xC005, 0xFF);
    }
    type_keys(&mut m, b"OK");
    assert!(m.screen_text().contains("OK_"));
}
