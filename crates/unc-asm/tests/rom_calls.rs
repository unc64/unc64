//! The jump-table routines added after PRHEX: PRDEC ($FF0F), RND ($FF12), NUMSTR ($FF18),
//! PRNUM ($FF1B) and RNDSEED ($FF1E), called from machine language the way chapter 13
//! documents; and the rule that printing draws no cursor.

use unc_core::RamSize;
use unc_core::machine::CYCLES_PER_FRAME;

const SEED: u16 = 0x020B;
const NUMBUF: u16 = 0x03D0;
const CURX: u16 = 0x0200;
const CURY: u16 = 0x0201;
const PTR: u16 = 0x00FB;

fn run(src: &str) -> (unc_core::Machine, unc_asm::Assembly) {
    let asm = unc_asm::assemble(src).unwrap_or_else(|e| panic!("{e:?}"));
    let mut m = unc_asm::boot(RamSize::K16, &asm);
    let done = asm.symbol("done").expect("a done label");
    m.run_until(done, 20 * CYCLES_PER_FRAME as u64).expect("reaches done");
    (m, asm)
}

#[test]
fn prdec_prints_x_and_y_as_an_unsigned_decimal() {
    let (m, _) = run("PRDEC = $FF0F\nCHROUT = $FF03\nCLS = $FF00
start:  JSR CLS
        LDX #0
next:   STX $10
        LDA lo,X
        PHA
        LDA hi,X
        TAY
        PLA
        TAX
        JSR PRDEC
        STY $11         ; Y comes back unchanged
        LDA #13
        JSR CHROUT
        LDX $10
        INX
        CPX #6
        BNE next
done:   HLT
lo:     .byte <0, <7, <142, <10000, <40960, <65535
hi:     .byte >0, >7, >142, >10000, >40960, >65535");
    let text = m.screen_text();
    let lines: Vec<&str> = text.lines().take(6).collect();
    assert_eq!(lines, ["0", "7", "142", "10000", "40960", "65535"]);
    assert_eq!(m.peek(0x11), 0xFF, "Y kept the last high byte");
}

#[test]
fn rnd_steps_basics_generator_and_returns_a_byte() {
    let (m, _) = run("RND = $FF12
start:  LDA #$34
        STA $020B
        LDA #$12
        STA $020C
        LDX #7
        LDY #9
        JSR RND
        STA $10
        STX $11
        STY $12
        LDX #0
fill:   JSR RND
        STA $1000,X
        INX
        BNE fill
done:   HLT");
    let first = m.peek(0x10);
    assert_eq!((m.peek(0x11), m.peek(0x12)), (7, 9), "X and Y are kept");
    // One step of the 16-bit xorshift (7, 9, 8) from $1234, as the ROM computes it.
    let (lo, hi) = (0x34u8, 0x12u8);
    let a = (hi & 1) << 7 | lo >> 1;
    let hi1 = a ^ hi;
    let a2 = (lo & 1) << 7 | hi1 >> 1;
    let lo1 = a2 ^ lo;
    let hi2 = lo1 ^ hi1;
    assert_eq!(first, hi2, "A is the new high byte of SEED");
    let bytes: Vec<u8> = (0..256).map(|i| m.peek(0x1000 + i)).collect();
    let distinct = bytes.iter().collect::<std::collections::HashSet<_>>().len();
    assert!(distinct > 120, "only {distinct} different values in 256");
    assert_eq!(m.peek(SEED + 1), bytes[255], "SEED is left advanced");
}

/// Cycles for one call, including the JSR, as chapter 13's cost table gives them.
fn cost(call: &str) -> u64 {
    let asm = unc_asm::assemble(&format!("start: {call}\nback: HLT")).unwrap();
    let mut m = unc_asm::boot(RamSize::K16, &asm);
    m.run_until(asm.symbol("back").unwrap(), CYCLES_PER_FRAME as u64).unwrap()
}

#[test]
fn what_prdec_and_rnd_cost() {
    let rnd = cost("JSR $FF12");
    let big = cost("LDX #$FF\n LDY #$FF\n JSR $FF0F") - 4;
    let small = cost("LDX #7\n LDY #0\n JSR $FF0F") - 4;
    println!("RND {rnd}, PRDEC 65535 {big}, PRDEC 7 {small}");
    assert!((40..=60).contains(&rnd), "RND takes {rnd}");
    assert!((1500..=2000).contains(&big), "PRDEC 65535 takes {big}");
    assert!((250..=450).contains(&small), "PRDEC 7 takes {small}");
}

#[test]
fn rnd_and_basic_share_the_seed() {
    let src = "RND = $FF12\nstart: JSR RND\n JSR RND\ndone: HLT";
    let (a, _) = run(src);
    let (b, _) = run(src);
    assert_eq!(a.peek(SEED), b.peek(SEED), "the same boot gives the same numbers");
    let booted = unc_core::Machine::boot(RamSize::K16);
    assert_ne!((a.peek(SEED), a.peek(SEED + 1)), (booted.peek(SEED), booted.peek(SEED + 1)), "RND moved BASIC's SEED");
}


/// Loads X, Y and A with a 24-bit number, low byte in X, as NUMSTR takes it.
fn xya(n: u32) -> String {
    format!("LDX #{}\n LDY #{}\n LDA #{}\n", n & 0xFF, n >> 8 & 0xFF, n >> 16)
}

#[test]
fn numstr_makes_eight_digits_with_leading_zeros_and_prints_nothing() {
    for n in [0u32, 1, 9, 10, 420, 65_535, 65_536, 999_999, 10_000_000, 16_777_215] {
        let (m, _) = run(&format!("NUMSTR = $FF18\nstart: {}\n JSR NUMSTR\n STA $10\ndone: HLT", xya(n)));
        let digits: Vec<u8> = (0..8).map(|i| m.peek(NUMBUF + i)).collect();
        let want = format!("{n:08}");
        assert_eq!(String::from_utf8(digits).unwrap(), want, "{n}");
        let first = want.find(|c| c != '0').unwrap_or(7).min(7) as u8;
        assert_eq!(m.peek(0x10), first, "A = the first digit that isn't a leading zero, for {n}");
        assert_eq!(m.screen(), unc_asm::boot(RamSize::K16, &unc_asm::assemble("HLT").unwrap()).screen(), "NUMSTR doesn't print");
    }
}

#[test]
fn prnum_prints_to_a_width_and_never_cuts_a_number_short() {
    let cases: [(u32, u8, &str); 14] = [
        (420, 6, "000420"), // the usual pattern
        (420, 0, "420"),
        (420, 1, "420"), // more digits than the width: all of them
        (420, 3, "420"),
        (420, 8, "00000420"),
        (0, 0, "0"),
        (0, 1, "0"),
        (0, 6, "000000"),
        (0, 8, "00000000"),
        (7, 1, "7"),
        (123_456, 3, "123456"),
        (16_777_215, 0, "16777215"),
        (16_777_215, 6, "16777215"),
        (5, 200, "00000005"), // over 8 counts as 8
    ];
    let mut src = String::from("NUMSTR = $FF18\nPRNUM = $FF1B\nCHROUT = $FF03\nCLS = $FF00\nstart: JSR CLS\n");
    for (n, w, _) in cases {
        src += &format!("{} JSR NUMSTR\n LDA #{w}\n JSR PRNUM\n LDA #13\n JSR CHROUT\n", xya(n));
    }
    // PRNUM keeps Y, and the digits are still there to print again.
    src += "LDY #$5A\n LDA #0\n JSR PRNUM\n STY $10\ndone: HLT";
    let (m, _) = run(&src);
    let text = m.screen_text();
    let lines: Vec<&str> = text.lines().collect();
    for (i, (n, w, want)) in cases.iter().enumerate() {
        assert_eq!(lines[i], *want, "{n} at width {w}");
    }
    assert_eq!(lines[cases.len()], "5", "the last NUMSTR, printed again");
    assert_eq!(m.peek(0x10), 0x5A, "Y is kept");
    assert!(!m.screen().contains(&0xA0), "no cursor: {text}");
}

#[test]
fn rndseed_sets_the_seed_and_never_leaves_it_zero() {
    let (m, _) = run("RNDSEED = $FF1E\nRND = $FF12
start:  LDX #$34
        LDY #$12
        JSR RNDSEED
        STX $10
        STY $11
        LDA $020B
        STA $12
        LDA $020C
        STA $13
        JSR RND
        STA $14
        LDX #0
        LDY #0
        JSR RNDSEED
done:   HLT");
    assert_eq!((m.peek(0x10), m.peek(0x11)), (0x34, 0x12), "X and Y are kept");
    assert_eq!((m.peek(0x12), m.peek(0x13)), (0x34, 0x12), "SEED = X/Y");
    let (lo, hi) = (0x34u8, 0x12u8);
    let a = (hi & 1) << 7 | lo >> 1;
    let hi1 = a ^ hi;
    let lo1 = ((lo & 1) << 7 | hi1 >> 1) ^ lo;
    assert_eq!(m.peek(0x14), lo1 ^ hi1, "RND goes on from the new seed");
    assert_eq!((m.peek(SEED), m.peek(SEED + 1)), (0xE1, 0xAC), "0 becomes the start-up seed, $ACE1");
}

#[test]
fn printing_draws_no_cursor_and_rubs_nothing_out() {
    let (m, _) = run("CLS = $FF00\nCHROUT = $FF03\nPRSTR = $FF09\nPRHEX = $FF0C\nPRDEC = $FF0F
NUMSTR = $FF18\nPRNUM = $FF1B
start:  JSR CLS
        LDA #'X'        ; something under where the cursor used to be drawn
        STA $0400
        LDA #13         ; a new line leaves it
        JSR CHROUT
        LDX #<msg
        LDY #>msg
        JSR PRSTR
        LDA #$A7
        JSR PRHEX
        LDX #<1234
        LDY #>1234
        JSR PRDEC
        LDX #<420
        LDY #>420
        LDA #0
        JSR NUMSTR
        LDA #6
        JSR PRNUM
        LDA #8          ; back one column, over the last 0: it stays
        JSR CHROUT
done:   HLT
msg:    .asciiz \"HI \"");
    let text = m.screen_text();
    assert_eq!(&text.lines().take(2).collect::<Vec<_>>(), &["X", "HI A71234000420"], "{text}");
    assert!(!m.screen().contains(&0xA0), "no cursor: {text}");
    let (x, y) = (m.peek(CURX), m.peek(CURY));
    assert_eq!((x, y), (14, 1));
    let row = m.peek(PTR) as u16 | (m.peek(PTR + 1) as u16) << 8;
    assert_eq!(row, 0x0400 + 40, "PTR still points at the cursor's row, for programs that use it");
}

#[test]
fn getkey_waits_without_a_cursor() {
    let asm = unc_asm::assemble("CLS = $FF00\nGETKEY = $FF06\nstart: JSR CLS\n JSR GETKEY\n STA $10\ndone: HLT").unwrap();
    let mut m = unc_asm::boot(RamSize::K16, &asm);
    for _ in 0..5 {
        m.run_frame();
    }
    assert!(!m.screen().contains(&0xA0), "GETKEY draws no cursor");
    m.key(b'Q');
    m.run_until(asm.symbol("done").unwrap(), CYCLES_PER_FRAME as u64).unwrap();
    assert_eq!(m.peek(0x10), b'Q');
    assert!(!m.screen().contains(&0xA0));
}

#[test]
fn what_numstr_prnum_and_rndseed_cost() {
    let numstr = |n: u32| cost(&format!("{}JSR $FF18", xya(n))) - 6;
    let (zero, small, six, big, worst) = (numstr(0), numstr(420), numstr(123_456), numstr(16_777_215), numstr(9_999_999));
    println!("NUMSTR 0 {zero}, 420 {small}, 123456 {six}, 16777215 {big}, 9999999 {worst}");
    assert!((450..=600).contains(&zero), "NUMSTR 0 takes {zero}");
    assert!((600..=800).contains(&small), "NUMSTR 420 takes {small}");
    assert!((900..=1100).contains(&six), "NUMSTR 123456 takes {six}");
    assert!((2300..=2700).contains(&worst), "NUMSTR 9999999 takes {worst}");
    // 9,999,999 has the most digits to count of any 24-bit number: it is the worst case.
    for n in [999_999, 8_999_999, 15_999_999, 16_777_215, 65_535, 99_999] {
        assert!(numstr(n) <= worst, "{n}");
    }
    let print = |n: u32, w: u8| cost(&format!("{}JSR $FF18\n LDA #{w}\n JSR $FF1B", xya(n))) - numstr(n) - 8;
    let (p6, p0) = (print(420, 6), print(420, 0));
    println!("PRNUM 420 width 6 {p6}, width 0 {p0}");
    assert!((700..=950).contains(&p6), "PRNUM six digits takes {p6}");
    assert!((450..=650).contains(&p0), "PRNUM three digits takes {p0}");
    let seed = cost("LDX #1\n LDY #2\n JSR $FF1E") - 4;
    println!("RNDSEED {seed}");
    assert!((25..=50).contains(&seed), "RNDSEED takes {seed}");
}

/// A program loaded at `READY.` (the Machine Room, the showcase) starts without the line
/// editor's cursor on screen: the loader puts back what was under it.
#[test]
fn loading_a_program_takes_the_editors_cursor_away() {
    let mut m = unc_core::Machine::boot(unc_core::RamSize::K16);
    let cell = 0x0400 + m.peek(0x0201) as u16 * 40 + m.peek(0x0200) as u16;
    assert_eq!(m.peek(cell), 0xA0, "BASIC waits with its cursor showing");
    let asm = unc_asm::assemble(".org $3000\nstart: HLT").unwrap();
    unc_asm::load(&mut m, &asm);
    assert_eq!(m.peek(cell), b' ', "the cursor is gone, the space under it back");
    assert!(!m.screen().contains(&0xA0));
}
