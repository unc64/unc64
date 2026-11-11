//! The Programmer's Reference Guide is the spec, so it is tested like code:
//! Appendix C must match the opcode table, and every listing in the book must
//! assemble, run, and do what the text around it says.

use unc_core::{Machine, RamSize, Stop};

const MANUAL: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/prg/programmers-reference-guide.md");
const BEGIN: &str = "<!-- BEGIN GENERATED: instruction set -->\n";
const END: &str = "<!-- END GENERATED -->";

fn manual() -> String {
    std::fs::read_to_string(MANUAL).expect("docs/prg/programmers-reference-guide.md")
}

#[test]
fn docs_appendix_c_matches_the_opcode_table() {
    let have = manual();
    let b = have.find(BEGIN).expect("BEGIN marker in Appendix C") + BEGIN.len();
    let e = have.find(END).expect("END marker in Appendix C");
    let want = format!("{}{}{}", &have[..b], unc_asm::doc::isa_reference(), &have[e..]);
    if std::env::var("UNC_UPDATE_DOCS").is_ok() {
        std::fs::write(MANUAL, &want).unwrap();
        return;
    }
    assert!(have == want, "Appendix C is stale: run `UNC_UPDATE_DOCS=1 cargo test -p unc-asm docs`");
}

/// Every ```asm block in the book.
fn listings() -> Vec<String> {
    listings_in(&manual())
}

fn listings_in(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur: Option<String> = None;
    for line in text.lines() {
        match &mut cur {
            None if line.trim() == "```asm" => cur = Some(String::new()),
            Some(src) if line.trim() == "```" => {
                out.push(std::mem::take(src));
                cur = None;
            }
            Some(src) => {
                src.push_str(line);
                src.push('\n');
            }
            None => {}
        }
    }
    out
}

struct Run {
    m: Machine,
    asm: unc_asm::Assembly,
    stop: Stop,
}

/// Whether a listing uses the video board: it's in chapter 15. Those listings run on a
/// machine with the board in.
fn uses_board(src: &str) -> bool {
    let text = manual();
    let from = text.find("\n## 15. ").expect("chapter 15");
    let to = from + 1 + text[from + 1..].find("\n## ").unwrap();
    listings_in(&text[from..to]).iter().any(|l| l == src)
}

/// Boots the machine like the Machine Room does (with the video board if the listing uses
/// it), loads the listing, presses `key` if given, and runs for `frames` frames or until it
/// stops.
fn run(src: &str, key: Option<u8>, frames: u32) -> Run {
    let asm = unc_asm::assemble(src).unwrap_or_else(|e| panic!("listing doesn't assemble: {e:?}\n{src}"));
    assert!(asm.start().is_some(), "listing has a start");
    let mut m = Machine::boot_with_board(RamSize::K16, uses_board(src));
    unc_asm::load(&mut m, &asm);
    if let Some(k) = key {
        m.key(k);
    }
    let mut stop = Stop::Budget;
    for _ in 0..frames {
        stop = m.run_frame();
        if stop != Stop::Budget {
            break;
        }
    }
    assert!(!matches!(stop, Stop::Jammed(_)), "listing jammed: {stop:?}\n{src}");
    Run { m, asm, stop }
}

fn word(m: &Machine, addr: u16) -> u16 {
    m.peek(addr) as u16 | (m.peek(addr + 1) as u16) << 8
}

#[test]
fn every_listing_in_the_manual_assembles_and_runs() {
    let all = listings();
    assert!(all.len() >= 17, "found only {} listings", all.len());
    let mut checked = 0;
    for src in &all {
        let title = src.lines().next().unwrap_or("");
        let has = |s: &str| title.contains(s);
        if has("Print a message") {
            let r = run(src, None, 5);
            assert_eq!(r.stop, Stop::Halted);
            assert!(r.m.screen_text().contains("HELLO FROM THE UNC 64!"));
        } else if has("Fill the screen with stars") {
            let r = run(src, None, 5);
            assert!(r.m.screen().iter().all(|&c| c == b'*'));
        } else if has("Print every byte value") {
            let r = run(src, None, 50);
            assert_eq!(r.stop, Stop::Halted);
            assert!(r.m.screen().windows(8).any(|w| w == b"FD FE FF"));
        } else if has("RESULT = NUM1 + NUM2") {
            let r = run(src, None, 1);
            assert_eq!(word(&r.m, r.asm.symbol("result").unwrap()), 3345);
        } else if has("multiply") {
            let r = run(src, None, 1);
            assert_eq!(r.m.peek(0x10) as u32 | (r.m.peek(0x12) as u32) << 8, 40000);
        } else if has("Print 142 in decimal") {
            let r = run(src, None, 2);
            assert_eq!(r.stop, Stop::Halted);
            assert!(r.m.screen_text().lines().any(|l| l.contains("142") && !l.contains("0142")));
        } else if has("Show all 256 screen codes") {
            let r = run(src, None, 10);
            assert_eq!(r.stop, Stop::Halted);
            let s = r.m.screen();
            let at = |col: usize, row: usize| s[row * 40 + col];
            for code in 0..=255usize {
                assert_eq!(at(4 + 2 * (code % 16), 3 + code / 16) as usize, code);
            }
            let text = r.m.screen_text();
            assert!(text.starts_with("UNC 64 CHARACTER SET"));
            assert!(text.lines().nth(2).unwrap().ends_with("C D E F"));
            assert!(text.lines().nth(18).unwrap().starts_with("F0"));
        } else if has("Move a block along row 12") {
            let r = run(src, None, 10);
            let row = &r.m.screen()[12 * 40..13 * 40];
            assert_eq!(row.iter().filter(|&&c| c == 0x0F).count(), 1);
        } else if has("Draw an 8 x 8 sprite at any x with the shifter") {
            let r = run(src, None, 3);
            assert_eq!(r.stop, Stop::Halted);
            let mut px = vec![0u8; 320 * 200];
            r.m.render(true, &mut px);
            let ball = [0x3C, 0x7E, 0xFF, 0xFF, 0xFF, 0xFF, 0x7E, 0x3C];
            for x in [3usize, 77, 160, 254, 312] {
                let rows: Vec<u8> = (0..8).map(|y| (0..8).fold(0u8, |b, i| b << 1 | px[(60 + y) * 320 + x + i])).collect();
                assert_eq!(rows, ball, "the ball at x = {x}");
            }
            let lit = px.iter().filter(|&&p| p == 1).count();
            assert_eq!(lit, 5 * 52, "five balls and nothing else");
        } else if has("Read held keys from the matrix") {
            let r = run(src, None, 1);
            let mut m = r.m;
            let corner = |m: &Machine| m.screen()[..3].to_vec();
            m.run_frame();
            assert_eq!(corner(&m), b"   ");
            m.key(b'A');
            m.key(b' ');
            m.run_frame();
            assert_eq!(corner(&m), b"A *", "two keys held at once");
            m.key_up(b'A');
            m.key(b'D');
            m.run_frame();
            assert_eq!(corner(&m), b" D*");
            m.key(b'A');
            m.run_frame();
            assert_eq!(corner(&m), b"AD*", "three");
            for k in *b"AD " {
                m.key_up(k);
            }
            m.run_frame();
            assert_eq!(corner(&m), b"   ", "all let go");
        } else if has("Play a tune on the timer") {
            let r = run(src, None, 1);
            let mut m = r.m;
            let (mut notes, mut playing, mut sounding) = (vec![], false, 0);
            for _ in 0..400 {
                if m.run_frame() == Stop::Halted {
                    break;
                }
                let io = &m.mem.io;
                let on = io.speaker && io.timer.gate;
                if on && !playing {
                    notes.push(io.timer.divisor(io.cycle));
                }
                playing = on;
                let mut s = [0.0f32; 882];
                m.audio_frame(44_100, &mut s);
                if on && s.windows(2).filter(|w| (w[0] > 0.0) != (w[1] > 0.0)).count() > 4 {
                    sounding += 1;
                }
            }
            assert_eq!(m.cpu.state, unc_core::CpuState::Halted);
            let (c, d, e, f, g) = (3822, 3405, 3034, 2863, 2551);
            assert_eq!(notes, [c, d, e, c, c, d, e, c, e, f, g, e, f, g], "Frere Jacques");
            assert!(sounding > 200, "the notes are heard: {sounding} frames");
            assert_eq!(m.peek(0xC031) & 3, 0, "silent at the end");
        } else if has("Change the border colour") {
            let r = run(src, None, 3);
            assert_eq!(r.m.border_lines[100], (100 / 8) as u8 & 0x0F);
        } else if has("A raster split: the background") {
            let r = run(src, None, 3);
            assert_eq!(r.m.line_colours(0)[0], 0, "black at the top");
            assert_eq!(r.m.line_colours(12 * 8 - 1)[0], 0, "down to row 11");
            assert_eq!(r.m.line_colours(12 * 8)[0], 2, "red from row 12, in the same frame");
            assert_eq!(r.m.line_colours(199)[0], 2);
        } else if has("A red band") {
            let r = run(src, None, 3);
            assert_eq!(r.m.border_lines[51 + 11 * 8], 2);
            assert_eq!(r.m.border_lines[40], 0);
            assert_eq!(r.m.border_lines[51 + 14 * 8], 0);
        } else if has("Raster split") {
            let r = run(src, None, 3);
            assert_eq!(r.m.border_lines[150], 6);
            assert_eq!(r.m.border_lines[30], 0);
            assert_eq!(r.m.border_lines[280], 0);
        } else if has("Custom characters") {
            let r = run(src, None, 3);
            assert_eq!(r.stop, Stop::Halted);
            assert_eq!(r.m.peek(0xC022), 0x38);
            assert!(r.m.screen()[12 * 40..13 * 40].iter().all(|&c| c == 0x7F));
            let mut px = vec![0u8; 320 * 200];
            r.m.render(true, &mut px);
            let owl: Vec<u8> = (0..8).map(|y| (0..8).fold(0u8, |b, x| b << 1 | px[(96 + y) * 320 + x])).collect();
            assert_eq!(owl, [0x42, 0x7E, 0xDB, 0xFF, 0x66, 0x7E, 0x24, 0x00]);
            let a = &r.m.mem.ram[0x3800 + 0x41 * 8..][..8];
            assert_eq!(a, &unc_core::charset::CHAR_ROM[0x41 * 8..][..8], "the rest of the set is UNSCII");
        } else if has("A box in 256-glyph mode") {
            let r = run(src, None, 3);
            assert_eq!(r.stop, Stop::Halted);
            assert_eq!((r.m.peek(0xC000), r.m.peek(0xC022)), (0x01, 0x30), "256 glyphs from $3000");
            let rows: Vec<&[u8]> = (10..15).map(|y| &r.m.screen()[y * 40 + 18..][..5]).collect();
            assert_eq!(rows[0], &[0x12, 0x10, 0x18, 0x10, 0x13]);
            assert_eq!(rows[1], &[0x11, b' ', 0x11, b' ', 0x11]);
            assert_eq!(rows[2], &[0x16, 0x10, 0x1A, 0x10, 0x17]);
            assert_eq!(rows[4], &[0x14, 0x10, 0x19, 0x10, 0x15]);
            let rom = &unc_core::charset::CHAR_ROM;
            assert_eq!(&r.m.mem.ram[0x3000..0x3400], &rom[..0x400], "the bottom half is UNSCII");
            let reversed: Vec<u8> = rom[..0x400].iter().map(|b| !b).collect();
            assert_eq!(&r.m.mem.ram[0x3400..0x3800], &reversed[..], "the top half is UNSCII reversed");
            let mut px = vec![0u8; 320 * 200];
            r.m.render(true, &mut px);
            let cell = |cx: usize, cy: usize| -> Vec<u8> {
                (0..8).map(|y| (0..8).fold(0u8, |b, x| b << 1 | px[(cy * 8 + y) * 320 + cx * 8 + x])).collect()
            };
            assert_eq!(cell(20, 12), rom[0x1A * 8..][..8], "the cross, drawn from the RAM set");
            assert_eq!(cell(18, 10), rom[0x12 * 8..][..8], "the top-left corner");
        } else if has("Plot one pixel at (100, 57)") {
            let r = run(src, None, 10);
            assert_eq!(r.stop, Stop::Halted);
            assert_eq!(r.m.peek(0xC000), 0x02, "the bitmap is on, full screen");
            assert_eq!(r.m.peek(0x28F4), 0x08);
            let mut px = vec![0u8; 320 * 200];
            r.m.render(true, &mut px);
            assert_eq!(px.iter().filter(|&&p| p == 1).count(), 1, "one pixel");
            assert_eq!(px[57 * 320 + 100], 1, "at (100, 57)");
            let mut m = unc_asm::boot(RamSize::K16, &r.asm);
            m.poke(0x3F3F, 0xAA);
            m.poke(0x3F40, 0x55);
            m.poke(0x3FFF, 0x55);
            for _ in 0..10 {
                m.run_frame();
            }
            assert_eq!((m.peek(0x3F3F), m.peek(0x3F40), m.peek(0x3FFF)), (0, 0x55, 0x55), "clears 8,000 bytes, no more");
        } else if has("A sine wave across the screen") {
            let r = run(src, None, 2);
            assert_eq!(r.stop, Stop::Halted);
            assert!(r.asm.warnings.is_empty(), "{:?}", r.asm.warnings);
            let s = r.m.screen();
            let wave = r.asm.symbol("wave").unwrap();
            for x in 0..40usize {
                let row = (12 - unc_asm::asm::int_sin(x as i64 * 256 / 40, 12)) as usize;
                assert_eq!(r.m.peek(wave + x as u16) as usize, row, "the table at column {x}");
                assert_eq!(s[row * 40 + x], b'*', "a star at column {x}, row {row}");
            }
            assert_eq!((r.m.peek(wave), r.m.peek(wave + 10), r.m.peek(wave + 30)), (12, 0, 24), "middle, top, bottom");
            assert_eq!(s.iter().filter(|&&c| c == b'*').count(), 40, "one star per column");
        } else if has("Show the score at the top right") {
            let r = run(src, None, 2);
            assert_eq!(r.stop, Stop::Halted);
            let top = r.m.screen_text().lines().next().unwrap().to_string();
            assert!(top.ends_with("REV A      000420"), "{top}");
            assert_eq!(&r.m.mem.ram[0x03D0..0x03D8], b"00000420", "NUMBUF");
        } else if has("Keep score as a row of digits") {
            let asm = unc_asm::assemble(src).unwrap();
            let mut m = unc_asm::boot(RamSize::K16, &asm);
            // Every JSR add, timed: 34 cycles, and 30 more for each carry.
            let add = asm.symbol("add").unwrap();
            let jsr = unc_core::isa::opcode_for(unc_core::isa::Op::Jsr, unc_core::isa::Mode::Abs).unwrap();
            let mut costs = Vec::new();
            while m.cpu.state == unc_core::CpuState::Running {
                let pc = m.cpu.pc;
                if m.peek(pc) == jsr && word(&m, pc + 1) == add {
                    let t = m.cpu.cycles;
                    m.step_over();
                    costs.push(m.cpu.cycles - t);
                } else {
                    m.step();
                }
            }
            assert_eq!(costs.len(), 4 + 143);
            assert!(costs.iter().all(|c| c >= &34 && (c - 34) % 30 == 0), "{costs:?}");
            assert_eq!(costs.iter().min(), Some(&34));
            assert_eq!(costs.iter().max(), Some(&(34 + 3 * 30)), "1999 + 7 carries three times");
            assert_eq!(&m.mem.ram[0x10..0x16], &[0, 0, 2, 0, 0, 1]);
            assert_eq!(&m.screen()[12 * 40 + 17..][..6], b"002001");
            // 999999 + 1 carries through all six digits and rolls over: under 200 cycles.
            m.load(0x10, &[9; 6]);
            let tail = unc_asm::assemble(&format!("ADD = ${add:04X}\n .org $3000\nstart: LDA #1\n LDX #5\n JSR ADD\nback: HLT")).unwrap();
            unc_asm::load(&mut m, &tail);
            let all = m.run_until(tail.symbol("back").unwrap(), 1000).unwrap() - 4;
            assert!(all < 200, "{all}");
            assert_eq!(&m.mem.ram[0x10..0x16], &[0; 6]);
            assert_eq!(&m.screen()[12 * 40 + 17..][..6], b"000000");
        } else if has("top-left corner") {
            let r = run(src, Some(b'Q'), 2);
            assert_eq!(r.m.screen()[0], b'Q');
        } else if has("Echo every key twice") {
            let r = run(src, Some(b'Q'), 2);
            assert!(r.m.screen_text().contains("QQ"));
        } else if has("A data tape") {
            // Off a real tape, with a BASIC program in memory: the numbers arrive, READY.
            // comes back, and the program and its variables are still there.
            let asm = unc_asm::assemble(src).unwrap();
            let mut m = Machine::boot(RamSize::K16);
            let typed = |m: &mut Machine, s: &str| {
                for k in s.bytes().chain([13]) {
                    m.key(k);
                    for _ in 0..400 {
                        m.run_cycles(500);
                        if m.peek(0xC010) & 0x80 == 0 {
                            break;
                        }
                    }
                    m.key_up(k);
                }
            };
            typed(&mut m, "10 FOR I=12288 TO 12297:PRINT PEEK(I);:NEXT");
            typed(&mut m, "N=7");
            m.insert_tape(unc_asm::tape::make(&asm, "DATA"));
            m.press_play();
            typed(&mut m, "LOAD");
            for _ in 0..200 {
                m.run_frame();
            }
            let rows: Vec<String> = m.screen_text().lines().map(str::to_string).filter(|l| !l.is_empty() && l != "_").collect();
            assert_eq!(rows[rows.len() - 2..], ["DATA AT 12288-12297", "READY."], "{rows:?}");
            assert_eq!(&m.mem.ram[0x3000..0x300A], &[3, 1, 4, 1, 5, 9, 2, 6, 5, 3]);
            assert_eq!(m.peek(0x0260 + 2 * 13), 7, "N kept");
            typed(&mut m, "RUN");
            for _ in 0..20 {
                m.run_frame();
            }
            assert!(m.screen_text().contains("3141592653"), "{}", m.screen_text());
        } else if has("Read the tape yourself") {
            let asm = unc_asm::assemble(src).unwrap();
            let tape: Vec<u8> = (0..300u32).map(|i| (i * 37 + 5) as u8).collect();
            let mut m = unc_asm::boot(RamSize::K16, &asm);
            for _ in 0..3 {
                m.run_frame();
            }
            assert_eq!(m.peek(0xC040), 0x04, "no tape, no PLAY: the motor runs and it waits");
            m.insert_tape(tape.clone());
            m.run_frame();
            m.press_play();
            let mut stop = Stop::Budget;
            for _ in 0..30 {
                stop = m.run_frame();
            }
            assert_eq!(stop, Stop::Halted);
            assert_eq!(&m.mem.ram[0x3000..0x3100], &tape[..256]);
            assert_eq!(m.peek(0xC040) & 0x44, 0, "motor off, nothing lost");
        } else if has("Record 256 bytes from $3000") {
            let asm = unc_asm::assemble(src).unwrap();
            let data: Vec<u8> = (0..256u32).map(|i| (i * 53 + 11) as u8).collect();
            let mut m = Machine::boot(RamSize::K16);
            m.load(0x3000, &data);
            m.insert_blank_tape();
            unc_asm::load(&mut m, &asm);
            for _ in 0..3 {
                m.run_frame();
            }
            assert_eq!(m.peek(0xC040), 0x06, "a tape in, the motor on, no RECORD: no room, it waits");
            m.press_record();
            let t0 = m.cpu.cycles;
            let mut stop = Stop::Budget;
            for _ in 0..30 {
                stop = m.run_frame();
                if stop == Stop::Halted {
                    break;
                }
            }
            assert_eq!(stop, Stop::Halted);
            assert!(((m.cpu.cycles - t0) as f64) < 500_000.0, "under half a second");
            assert_eq!(m.peek(0xC040) & 0x04, 0, "motor off");
            assert_eq!(m.eject_tape().unwrap(), data, "all 256, the last one included");
        } else if has("440 Hz") {
            let r = run(src, None, 1);
            let before = (r.m.mem.io.speaker_toggles, r.m.cpu.cycles);
            let mut m = r.m;
            for _ in 0..50 {
                m.run_frame();
            }
            let flips = (m.mem.io.speaker_toggles - before.0) as f64;
            let hz = flips / 2.0 / ((m.cpu.cycles - before.1) as f64 / 1_000_000.0);
            assert!((hz - 439.4).abs() < 0.5, "tone is {hz} Hz");
        } else if has("on voice 1 of the AUNT") {
            // the note sounds for 25 frames, at A, and the gate is off when it halts
            let r = run(src, None, 10);
            let mut m = r.m;
            assert_eq!(u16::from_le_bytes([m.peek(0xC0C0), m.peek(0xC0C1)]), 7382);
            assert_eq!(m.peek(0xC0C4) & 1, 1, "the gate is on");
            let mut out = [0.0f32; 882];
            m.audio_frame(44_100, &mut out);
            let rises = out.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
            assert!((8..=10).contains(&rises), "a frame of A has about 9 waves: {rises}");
            let r = run(src, None, 40);
            assert!(matches!(r.stop, Stop::Halted), "{:?}", r.stop);
            assert_eq!(r.m.peek(0xC0C4) & 1, 0, "and off");
            assert!(r.m.cpu.cycles > 25 * 19_968);
        } else if has("A first sprite: a ship") {
            check_ship(src);
        } else if has("Twelve birds in a row") {
            check_birds(src);
        } else if has("Four colours on the bitmap") {
            check_four_colours(src);
        } else if has("Bounce a ball between two walls") {
            check_bounce(src);
        } else if has("A scrolling message: a pixel a frame") {
            check_scroller(src);
        } else if has("Parallax: a message scrolls") {
            check_parallax(src);
        } else {
            panic!("listing has no check in tests/manual.rs; add one:\n{title}");
        }
        checked += 1;
    }
    assert_eq!(checked, all.len());
}

// ---- Chapter 15: the video board. These listings run with the board in (`uses_board`). ----

const W: usize = 320;

fn picture(m: &Machine) -> Vec<u8> {
    let mut px = vec![0u8; W * 200];
    m.render(true, &mut px);
    px
}

/// The picture without the video board's layer: the screen and the sprites.
fn picture_without_layer(m: &Machine) -> Vec<u8> {
    let mut c = m.clone();
    c.poke(0xC0AA, 0);
    picture(&c)
}

/// Table entry n's x, all nine bits (bit 8 is bit 0 of byte 3).
fn sprite_x(m: &Machine, n: u16) -> u16 {
    let at = 0x3700 + n * 4;
    m.peek(at + 1) as u16 | (m.peek(at + 3) as u16 & 1) << 8
}

/// Row `r` of the 2-bit image at `at`, as 16 pixel values.
fn image_row(m: &Machine, at: usize, r: usize) -> Vec<u8> {
    (0..16).map(|c| (m.mem.ram[at + r * 4 + c / 4] >> (6 - 2 * (c % 4))) & 3).collect()
}

/// `w` pixels from (x, y).
fn pixels(px: &[u8], x: usize, y: usize, w: usize) -> Vec<u8> {
    px[y * W + x..][..w].to_vec()
}

/// Lines `ys` of `after` are lines `ys` of `before` moved `dx` pixels left (columns 8-311).
fn moved_left(before: &[u8], after: &[u8], ys: std::ops::Range<usize>, dx: usize) -> bool {
    ys.into_iter().all(|y| (8..312).all(|x| after[y * W + x] == before[y * W + x + dx]))
}

fn check_ship(src: &str) {
    let r = run(src, None, 3);
    assert!(r.m.has_board());
    let mut m = r.m;
    let ship = r.asm.symbol("ship").unwrap() as usize;
    assert_eq!(&m.mem.ram[0x3800..0x3840], &m.mem.ram[ship..ship + 64], "image 0 at $3800");
    assert_eq!(&m.mem.ram[0x3700..0x3704], &[92, 152, 0, 0x80], "entry 0: y, x, image, shown");
    assert!(m.mem.ram[0x3704..0x3780].iter().all(|&b| b == 0), "the rest of the table is empty");
    let px = picture(&m);
    for row in 0..16 {
        assert_eq!(pixels(&px, 152, 92 + row, 16), image_row(&m, 0x3800, row), "row {row} of the ship");
    }
    let colours: std::collections::BTreeSet<u8> = px[92 * W..108 * W].iter().copied().collect();
    assert_eq!(colours.into_iter().collect::<Vec<_>>(), [0, 1, 2, 3], "exhaust, body and window");
    m.key(29); // right, held
    for _ in 0..10 {
        m.run_frame();
    }
    assert_eq!(sprite_x(&m, 0), 162, "a pixel a frame");
    m.key_up(29);
    m.key(28); // left: to x = 2, then 10 more, to -8
    for _ in 0..170 {
        m.run_frame();
    }
    assert_eq!((sprite_x(&m, 0), m.peek(0x3703)), (504, 0x89), "-8: x = 504; bit 8, flipped and shown");
    let px = picture(&m);
    for row in 0..16 {
        // flipped: screen column c is image column 15 - c, at x = c - 8
        let img = image_row(&m, 0x3800, row);
        let want: Vec<u8> = (8..16).map(|c| img[15 - c]).collect();
        assert_eq!(pixels(&px, 0, 92 + row, 8), want, "the ship's nose, turned round, at the left edge");
    }
    m.key_up(28);
    m.key(30); // up, off the top
    for _ in 0..100 {
        m.run_frame();
    }
    assert_eq!(m.peek(0x3700), 248, "y = -8");
}

fn check_birds(src: &str) {
    let r = run(src, None, 2);
    let mut m = r.m;
    let bird = r.asm.symbol("bird").unwrap() as usize;
    assert_eq!(&m.mem.ram[0x3800..0x3840], &m.mem.ram[bird..bird + 64]);
    m.load(0x0400, &[0x20; 1000]); // the start-up message out of the way: only sprites
    // per bird: frames wholly on the screen, and frames it was drawn in
    let (mut on, mut seen) = ([0u32; 12], [0u32; 12]);
    let mut wrapped = false;
    let eye = image_row(&m, 0x3800, 7);
    for _ in 0..300 {
        m.run_frame();
        assert_eq!(m.peek(0xC0A6), 0x80, "more than eight on a line");
        unc_core::Bus::read(&mut m.mem, 0xC0A6); // as the processor reads it: cleared
        let xs: Vec<u16> = (0..12).map(|n| sprite_x(&m, n)).collect();
        assert!(m.mem.ram[0x3730..0x3780].iter().all(|&b| b == 0), "entries 12-31 stay empty");
        // which bird is which: they're 26 apart, and bird 0 has nobody 26 to its left
        let x0 = *xs.iter().find(|&&x| !xs.contains(&((x + 512 - 26) % 512))).unwrap();
        let px = picture(&m);
        for x in xs {
            let i = ((x + 512 - x0) % 512 / 26) as usize;
            let x = x as usize;
            if x <= 304 {
                on[i] += 1;
                seen[i] += (pixels(&px, x, 107, 16) == eye) as u32;
            }
            wrapped |= x >= 496;
        }
        let eyes = (0..W).filter(|&x| px[107 * W + x] == 3).count();
        assert!(eyes <= 8 * 2, "at most eight birds' eyes on line 107: {eyes}");
    }
    assert!(wrapped, "round through 511");
    // the middle four every frame, the other eight every other frame
    for i in 0..12 {
        assert!(on[i] > 50, "bird {i}: {on:?}");
        if (4..8).contains(&i) {
            assert_eq!(seen[i], on[i], "bird {i} always shows");
        } else {
            assert!(seen[i] * 2 >= on[i] - 1 && seen[i] * 2 <= on[i] + 1, "bird {i} every other frame: {seen:?} of {on:?}");
        }
    }
}

fn check_bounce(src: &str) {
    let r = run(src, None, 2);
    let mut m = r.m;
    let (mut lo, mut hi, mut turns, mut dir) = (255u8, 0u8, 0, 0i32);
    let mut last = m.peek(0x3701);
    for _ in 0..800 {
        m.run_frame();
        let x = m.peek(0x3701);
        let d = x as i32 - last as i32;
        assert_eq!(d.abs(), 1, "a pixel a frame");
        if d != dir {
            turns += 1;
            dir = d;
        }
        (lo, hi, last) = (lo.min(x), hi.max(x), x);
    }
    // the walls are columns 5 and 30, pixels 40-47 and 240-247: the ball touches the right
    // wall at x = 225 (its right edge on 240) and the left one at 47
    assert_eq!((lo, hi), (47, 225));
    assert!(turns >= 5, "{turns}");
    assert_eq!(m.screen()[12 * 40 + 5], 0x0F);
}

fn check_four_colours(src: &str) {
    let r = run(src, None, 20);
    assert_eq!(r.stop, Stop::Halted);
    let m = r.m;
    assert_eq!((m.peek(0xC000), m.peek(0xC0B0)), (2, 1), "the bitmap, in four colours");
    let px = picture(&m);
    for y in [0, 99, 199] {
        for (x, v) in [(0, 0), (79, 0), (80, 1), (159, 1), (160, 2), (239, 2), (240, 3), (319, 3)] {
            assert_eq!(px[y * W + x], v, "({x}, {y})");
        }
    }
    assert_eq!([0, 1, 2, 3].map(|v| m.colour_of(v)), [6, 2, 7, 1], "blue, red, yellow, white");
    assert_eq!((m.peek(0xC023), m.peek(0x1000 + 7999)), (0x10, 0xFF), "the bitmap at $1000");
    assert!(m.mem.ram[0x3700..0x3780].iter().all(|&b| b == 0), "and the sprite table untouched");
}

fn check_scroller(src: &str) {
    let r = run(src, None, 2);
    let mut m = r.m;
    let mut scroll = vec![];
    for _ in 0..120 {
        let before = picture(&m);
        m.run_frame();
        let after = picture(&m);
        assert!(moved_left(&before, &after, 96..104, 1), "a pixel a frame, through every coarse step");
        assert!(after[..96 * W].iter().chain(&after[104 * W..]).all(|&p| p == 0), "only row 12");
        scroll.push(m.peek(0xC0A8));
    }
    assert!(scroll.windows(2).all(|w| w[1] == (w[0] + 7) % 8), "SCROLLX counts 7 down to 0: {scroll:?}");
    // once the row is full: 40 letters of the message, round and round
    let msg = "SMOOTH AS SILK, A PIXEL A FRAME ... ".repeat(3);
    for i in 0..400 {
        m.run_frame();
        if i >= 300 {
            let text = m.screen_text();
            let row = text.lines().nth(12).unwrap();
            assert!(row.len() > 30 && msg.contains(row), "{row:?}");
        }
    }
}

fn check_parallax(src: &str) {
    let asm = unc_asm::assemble(src).unwrap();
    let mut m = Machine::boot_with_board(RamSize::K16, true);
    m.poke(0x3FE8, 0xAA);
    unc_asm::load(&mut m, &asm);
    m.run_frame();
    m.run_frame();
    assert_eq!(m.peek(0xC0AA), 1, "the layer on");
    assert!(m.mem.ram[0x3C00..0x3FE8].iter().all(|&c| c == b'.'), "a dot in every cell");
    assert_eq!(m.peek(0x3FE8), 0xAA, "and not a byte past the layer");
    let mut frames = vec![];
    for _ in 0..64 {
        frames.push((picture(&m), picture_without_layer(&m)));
        m.run_frame();
    }
    for t in 0..63 {
        assert!(moved_left(&frames[t].1, &frames[t + 1].1, 96..104, 1), "the message: a pixel a frame");
    }
    for t in 0..60 {
        // row 5 has nothing on the screen: only the dots
        assert!(moved_left(&frames[t].0, &frames[t + 4].0, 40..48, 1), "the dots: a pixel in four frames");
        assert!(frames[t].0[40 * W..48 * W].contains(&1), "dots on row 5");
    }
    let dots = |px: &[u8]| px[40 * W..48 * W].to_vec();
    assert!((0..60).any(|t| dots(&frames[t].0) == dots(&frames[t + 1].0)), "not every frame");
}

/// unc64.com/llms-full.txt is the manual for AI agents, as the PDF prints it (the workshop
/// notes stripped), with the cassette format after it. It mustn't fall behind the book.
#[test]
fn llms_full_txt_is_the_current_manual() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let read = |p: &str| std::fs::read_to_string(format!("{root}/{p}")).unwrap_or_else(|_| panic!("{p}"));
    // the same rules as scripts/build-llms.py
    let mut out = Vec::new();
    let mut skip = false;
    let src = manual();
    for line in src.split('\n') {
        if line.starts_with("> **Workshop note.**") {
            skip = true;
            continue;
        }
        if skip {
            if line.starts_with('>') {
                continue;
            }
            skip = false;
            if line.is_empty() {
                continue; // and the blank line after it
            }
        }
        if line.starts_with("<!-- BEGIN GENERATED") || line.starts_with("<!-- END GENERATED") {
            continue;
        }
        out.push(line);
    }
    let mut book = out.join("\n");
    while book.contains("\n---\n\n---\n") {
        book = book.replace("\n---\n\n---\n", "\n---\n");
    }
    let want = format!(
        "{}{}\n\n---\n\n{}\n",
        read("scripts/llms/full-header.md"),
        book.trim_end_matches('\n'),
        read("docs/cassettes.md").trim_end_matches('\n')
    );
    let have = read("llms-full.txt");
    assert!(!have.contains("> **Workshop note.**"), "no workshop notes in the published copy");
    assert!(have == want, "llms-full.txt is stale: run `python3 scripts/build-llms.py`");
}
