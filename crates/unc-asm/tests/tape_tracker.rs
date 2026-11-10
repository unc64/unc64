//! MUSIC TRACKER (games/sound/tracker.s), the sound chip's demonstration cassette, worked
//! through scripted keys on a booted 16K machine with the chip in. Its state is in the
//! fast page; the tests look it up by name.

use unc_asm::Assembly;
use unc_core::chip;
use unc_core::machine::CYCLES_PER_FRAME;
use unc_core::{Machine, RamSize};

const SRC: &str = include_str!("../../../games/sound/tracker.s");
const AUNT: u16 = 0xC0C0;
const ROWS: u32 = 32;
const SPEED: u32 = 6;

struct Tape {
    m: Machine,
    asm: Assembly,
    work: u16,
    main: u16,
    work_start: Option<u64>,
    /// Most cycles one frame's work took.
    worst: u64,
    /// Everything the machine has played, at 44.1 kHz.
    sound: Vec<f32>,
}

impl Tape {
    fn boot(sound: bool) -> Tape {
        let asm = unc_asm::assemble(SRC).unwrap_or_else(|e| panic!("tracker.s doesn't assemble: {e:?}"));
        let mut m = Machine::boot(RamSize::K16);
        m.set_sound(sound);
        unc_asm::load(&mut m, &asm);
        let (work, main) = (asm.symbol("work").unwrap(), asm.symbol("main").unwrap());
        let mut t = Tape { m, asm, work, main, work_start: None, worst: 0, sound: Vec::new() };
        t.frames(3);
        t
    }

    fn sym(&self, name: &str) -> u16 {
        self.asm.symbol(name).unwrap_or_else(|| panic!("no symbol {name}"))
    }
    fn get(&self, name: &str) -> u8 {
        self.m.peek(self.sym(name))
    }
    fn frame(&mut self) {
        let end = self.m.cpu.cycles + CYCLES_PER_FRAME as u64;
        while self.m.cpu.cycles < end {
            let pc = self.m.cpu.pc;
            if pc == self.work {
                self.work_start = Some(self.m.cpu.cycles);
            } else if pc == self.main
                && let Some(s) = self.work_start.take()
            {
                self.worst = self.worst.max(self.m.cpu.cycles - s);
            }
            assert!(self.m.step() > 0, "the processor stopped: {:?} at ${:04X}", self.m.cpu.state, self.m.cpu.pc);
        }
        let mut out = [0.0f32; 882];
        self.m.audio_frame(44_100, &mut out);
        self.sound.extend_from_slice(&out);
    }
    fn frames(&mut self, n: u32) {
        for _ in 0..n {
            self.frame();
        }
    }
    /// A tap: the key goes down, a frame, the key comes up, a frame.
    fn press(&mut self, k: u8) {
        self.m.key(k);
        self.frame();
        self.m.key_up(k);
        self.frame();
    }
    fn line(&self, row: usize) -> String {
        self.m.screen_text_plain().lines().nth(row).unwrap_or("").to_string()
    }
    /// The cell of `voice` at `row` in the patterns now showing: (note, instrument).
    fn cell(&self, voice: u16, row: u16) -> (u8, u8) {
        let pat = self.m.peek(self.sym("PAT") + voice) as u16;
        let at = self.sym("PATTERNS") + pat * 64 + row * 2;
        (self.m.peek(at), self.m.peek(at + 1))
    }
    fn loud(&self, from: usize) -> f32 {
        let s = &self.sound[from..];
        (s.iter().map(|v| v * v).sum::<f32>() / s.len() as f32).sqrt()
    }
}

#[test]
fn it_needs_the_chip_and_says_so() {
    let t = Tape::boot(false);
    assert!(t.line(11).contains("THE TRACKER NEEDS THE SOUND CHIP."), "{}", t.m.screen_text_plain());
    assert!(t.line(13).contains("FIT THE AUNT"));
}

#[test]
fn the_panel_shows_the_tune_and_its_first_rows() {
    let t = Tape::boot(true);
    let screen = t.m.screen_text_plain();
    assert!(t.line(0).contains("MUSIC TRACKER") && t.line(0).contains("ROB'S YOUR UNCLE"), "{screen}");
    assert!(t.line(1).contains("POS 00") && t.line(1).contains("BPM 125"), "{screen}");
    assert!(t.line(2).contains("STOP") && t.line(2).contains("FILTER IN"), "{screen}");
    assert!(t.line(4).contains("VOICE 1") && t.line(4).contains("VOICE 3"), "{screen}");
    // row 0 is the bright one, in the middle; nothing above it
    assert!(t.line(12).trim().is_empty(), "{screen}");
    // the riff starts alone: the bass's E, instrument 7, and nothing yet above it
    assert!(t.line(13).contains("00") && t.line(13).contains("--- .") && t.line(13).contains("E-1 7"), "{screen}");
    assert!(t.line(14).starts_with(" 01"), "{screen}");
    assert_eq!(t.loud(0), 0.0, "silent until asked");
}

#[test]
fn space_plays_a_row_every_six_frames_and_is_heard() {
    let mut t = Tape::boot(true);
    t.press(b' ');
    assert_eq!(t.get("PLAYING"), 1);
    assert_eq!(t.get("ROW"), 0, "the first row sounds first");
    let at = t.sound.len();
    t.frames(SPEED * 10);
    assert_eq!(t.get("ROW"), 10);
    assert!(t.line(2).contains("PLAY"));
    assert!(t.loud(at) > 0.01, "{}", t.loud(at));
    // the notes are on the chip: voice 3's FREQ is the bass's E, 41.2 Hz or its octave
    let freq = u16::from_le_bytes([t.m.peek(AUNT + 16), t.m.peek(AUNT + 17)]) as f64 * 1e6 / (1u64 << 24) as f64;
    assert!((freq - 41.2).abs() < 1.0 || (freq - 82.4).abs() < 2.0, "{freq}");
    t.press(b' ');
    assert_eq!(t.get("PLAYING"), 0);
    for v in 0..3u16 {
        assert_eq!(t.m.peek(AUNT + v * 8 + chip::CONTROL as u16) & 1, 0, "voice {v} let go");
    }
    let row = t.get("ROW");
    t.frames(20);
    assert_eq!(t.get("ROW"), row, "stopped");
}

/// The tunes on the tape: name, positions, where it goes round from, frames a row.
const TUNES: [(&str, u32, u8, u32); 4] = [
    ("ROB'S YOUR UNCLE", 24, 2, 6),
    ("SWITCHED ON", 16, 0, 9),
    ("THE LONG WAY ROUND", 16, 0, 8),
    ("THE GLIDE", 20, 0, 6),
];

#[test]
fn every_tune_plays_in_time_and_comes_round() {
    let mut t = Tape::boot(true);
    for (n, (name, positions, again, speed)) in TUNES.into_iter().enumerate() {
        assert_eq!(t.get("SONG") as usize, n);
        assert!(t.line(0).contains(name), "{}", t.line(0));
        assert_eq!((t.get("PLAYING"), t.get("POS"), t.get("ROW")), (0, 0, 0), "{name}: stopped at its top");
        assert_eq!(t.get("SPEED") as u32, speed);
        let at = t.sound.len();
        t.press(13);
        let mut seen = vec![false; positions as usize];
        for _ in 0..(positions * ROWS * speed + speed * 4) {
            t.frame();
            seen[t.get("POS") as usize] = true;
        }
        assert!(seen.iter().all(|&s| s), "{name}: {seen:?}");
        assert_eq!(t.get("POS"), again, "{name}: round again");
        let peak = t.sound[at..].iter().fold(0.0f32, |a, v| a.max(v.abs()));
        assert!((0.2..0.8).contains(&peak), "{name}: {peak}");
        assert!(t.loud(at) > 0.03, "{name}: {}", t.loud(at));
        println!("{name}: peak {peak}, rms {}", t.loud(at));
        t.press(b'T');
        for v in 0..3u16 {
            assert_eq!(t.m.peek(AUNT + v * 8 + chip::CONTROL as u16) & 1, 0, "{name}: T stops it");
        }
    }
    assert_eq!(t.get("SONG"), 0, "and after the last, the first again");
    assert!(t.line(0).contains(TUNES[0].0) && !t.line(0).contains("GLIDE"), "{}", t.line(0));
    assert!(t.worst < CYCLES_PER_FRAME as u64, "a frame's work took {} cycles", t.worst);
    println!("worst frame {} cycles", t.worst);
}

#[test]
fn the_tape_fits_a_unc_16() {
    let asm = unc_asm::assemble(SRC).unwrap();
    let top = asm.segments.iter().map(|s| s.start as u32 + s.bytes.len() as u32).max().unwrap();
    assert!(top <= 0x4000, "{top:#x}");
    assert!(asm.symbol("ORDER").unwrap() < asm.symbol("PATTERNS").unwrap());
}

#[test]
fn q_w_and_e_silence_a_voice_and_f_takes_the_filter_out() {
    let mut t = Tape::boot(true);
    t.press(13);
    t.frames(ROWS * SPEED * 4 + 30);
    let wave = |t: &Tape, v: u16| t.m.peek(AUNT + v * 8 + chip::WAVE as u16);
    assert!((0..3).all(|v| wave(&t, v) != 0));
    t.press(b'W');
    t.frames(2);
    assert_eq!(wave(&t, 1), 0);
    assert!(wave(&t, 0) != 0 && wave(&t, 2) != 0);
    assert!(t.line(4).contains("MUTED 2"), "{}", t.line(4));
    t.press(b'W');
    t.frames(2);
    assert_ne!(wave(&t, 1), 0);
    // THE GLIDE's chords go through the filter; F takes it out
    for _ in 0..3 {
        t.press(b'T');
    }
    assert!(t.line(0).contains("THE GLIDE"));
    t.press(13);
    let mut through = 0;
    for _ in 0..SPEED * 16 {
        t.frame();
        through |= t.m.peek(0xC0D9) & 7;
    }
    assert_eq!(through, 0b010, "voice 2, the chord");
    t.press(b'F');
    for _ in 0..SPEED * 16 {
        t.frame();
        assert_eq!(t.m.peek(0xC0D9) & 7, 0);
    }
    assert!(t.line(2).contains("FILTER OUT"));
}

#[test]
fn notes_are_written_from_the_keys_and_heard() {
    let mut t = Tape::boot(true);
    // voice 2, row 3, instrument 4, octave 2
    t.press(29);
    for _ in 0..3 {
        t.press(31);
    }
    t.press(b'4');
    t.press(b',');
    t.press(b',');
    assert_eq!((t.get("CURV"), t.get("ROW"), t.get("INS"), t.get("OCT")), (1, 3, 4, 2));
    let at = t.sound.len();
    t.press(b'G'); // F#
    assert_eq!(t.cell(1, 3), (1 + 2 * 12 + 6, 4));
    assert_eq!(t.get("ROW"), 4, "and on to the next row");
    assert!(t.line(12).contains("F#2 4"), "{}", t.m.screen_text_plain());
    t.frames(10);
    assert!(t.loud(at) > 0.005, "heard as it's written");
    t.frames(60);
    assert_eq!(t.m.peek(AUNT + 8 + chip::CONTROL as u16) & 1, 0, "and let go by itself");
    t.press(b'=');
    assert_eq!(t.cell(1, 4), (0xFF, 0));
    assert!(t.line(12).contains("=== ."), "{}", t.line(12));
    t.press(30);
    t.press(8);
    assert_eq!(t.cell(1, 4), (0, 0));
    // the position keys
    t.press(b']');
    assert_eq!(t.get("POS"), 1);
    t.press(b'[');
    t.press(b'[');
    assert_eq!(t.get("POS"), 0);
    // while it plays, the keys don't write
    t.press(b' ');
    let before = t.cell(0, 20);
    t.press(b'Z');
    assert_eq!(t.cell(0, 20), before);
}

fn freq(t: &Tape, v: u16) -> f64 {
    u16::from_le_bytes([t.m.peek(AUNT + v * 8), t.m.peek(AUNT + v * 8 + 1)]) as f64 * 1e6 / (1u64 << 24) as f64
}

fn gate(t: &Tape, v: u16) -> u8 {
    t.m.peek(AUNT + v * 8 + chip::CONTROL as u16) & 1
}

/// The string: a note of instrument 1 starts the wave table being redrawn, a drawing a frame
/// at first, and the voice's waveform is the table.
#[test]
fn the_string_redraws_the_wave_table_as_its_note_sounds() {
    let mut t = Tape::boot(true);
    let table = |t: &Tape| (0..32u16).map(|i| t.m.peek(0xC0E0 + i)).collect::<Vec<u8>>();
    assert!(table(&t).iter().all(|&b| b == 0), "nothing drawn before a note");
    t.press(13);
    // the tune comes in at position 02, on the string
    while t.get("POS") < 2 {
        t.frame();
    }
    t.frame();
    assert_eq!(t.m.peek(AUNT + chip::WAVE as u16), 0x10, "voice 1 plays the table");
    assert_eq!(t.get("DRAWON"), 1);
    let mut drawings = vec![table(&t)];
    for _ in 0..10 {
        t.frame();
        let now = table(&t);
        assert_ne!(&now, drawings.last().unwrap(), "a new drawing every frame while the note is young");
        drawings.push(now);
    }
    // driven: at the strike most of the table is against its ends
    let ends = |w: &Vec<u8>| w.iter().filter(|&&b| b == 0 || b == 255).count();
    assert!(ends(&drawings[0]) > 20, "{:?}", drawings[0]);
}

/// An instrument that lets go: its note's gate comes off two frames before the next note on
/// its voice, and the bass's does in every bar of the first tune.
#[test]
fn a_note_is_let_go_two_frames_before_the_next() {
    let mut t = Tape::boot(true);
    t.press(13);
    while !(t.get("STEP") == 1 && t.get("ROW") == 1) {
        t.frame();
    }
    // row 1: the bass's E from row 0 is still held, and row 2 has its octave
    let mut seen = Vec::new();
    while t.get("ROW") == 1 {
        seen.push((t.get("TICK"), gate(&t, 2)));
        t.frame();
    }
    assert_eq!(seen, vec![(6, 1), (5, 1), (4, 1), (3, 1), (2, 0), (1, 0)]);
    assert_eq!(gate(&t, 2), 1, "and the next note has its own attack");
}

/// An instrument that slides: THE GLIDE's lead goes from a note to the next without letting
/// go, and gets there over a few frames.
#[test]
fn a_slide_comes_to_its_note_from_the_last_one() {
    let mut t = Tape::boot(true);
    for _ in 0..3 {
        t.press(b'T');
    }
    t.press(13);
    while !(t.get("STEP") == 1 && t.get("ROW") == 6) {
        assert!(t.get("ROW") < 6);
        if t.get("ROW") > 0 {
            assert_eq!(gate(&t, 0), 1, "held from the first row");
        }
        t.frame();
    }
    // row 6: D5 (587 Hz) to F5 (698 Hz)
    let mut path = Vec::new();
    while t.get("ROW") == 6 {
        assert_eq!(gate(&t, 0), 1, "tied: the gate stays on");
        path.push(freq(&t, 0));
        t.frame();
    }
    assert!((path[0] - 587.3).abs() < 8.0, "it starts where it was: {path:?}");
    assert!(path.windows(2).all(|w| w[1] >= w[0]), "{path:?}");
    assert!((path[5] - 698.5).abs() < 5.0, "and is there by the end of the row: {path:?}");
    assert!(path[1] > path[0] + 20.0 && path[1] < 690.0, "by halves: {path:?}");
}

#[test]
fn o_and_p_step_through_all_the_instruments() {
    let mut t = Tape::boot(true);
    let last = t.sym("NINS") as u8;
    assert!(last > 9 && last < 32);
    for _ in 0..9 {
        t.press(b'P');
    }
    assert_eq!(t.get("INS"), 10);
    assert!(t.line(1).contains("INS A"), "{}", t.line(1));
    t.press(b'O');
    assert_eq!(t.get("INS"), 9);
    for _ in 0..40 {
        t.press(b'P');
    }
    assert_eq!(t.get("INS"), last, "and stops at the last");
    for _ in 0..40 {
        t.press(b'O');
    }
    assert_eq!(t.get("INS"), 1);
    // a note written with an instrument past 9 shows its letter
    t.press(b'P');
    for _ in 0..9 {
        t.press(b'P');
    }
    t.press(b'Z');
    assert_eq!(t.cell(0, 0), (1 + 4 * 12, 11));
    assert!(t.line(12).contains("C-4 B"), "{}", t.m.screen_text_plain());
}

/// The order from the keys: K and L change the pattern a voice plays at a position, A adds an
/// empty position (three patterns nothing uses), R takes one out, and the tune still goes
/// round from where it did.
#[test]
fn the_order_is_changed_from_the_keys() {
    let mut t = Tape::boot(true);
    let order = t.sym("ORDER");
    let pos = |t: &Tape, p: u16| [t.m.peek(order + p * 3), t.m.peek(order + p * 3 + 1), t.m.peek(order + p * 3 + 2)];
    let end = |t: &Tape| (0..32u16).find(|&p| t.m.peek(order + p * 3) == 0xFF).unwrap();
    assert_eq!(end(&t), 24);
    assert_eq!(t.m.peek(order + 24 * 3 + 1), 2, "it goes round from position 02");
    let (p0, p1) = (pos(&t, 0), pos(&t, 1));
    // L: voice 1's pattern at position 00, the next one, and K back; K from 0 goes to the last
    t.press(b'L');
    assert_eq!(pos(&t, 0)[0], p0[0] + 1);
    assert_eq!(t.get("PAT"), p0[0] + 1, "and it is the pattern showing");
    t.press(b'K');
    assert_eq!(pos(&t, 0), p0);
    let npats = t.sym("NPATS") as u8;
    assert_eq!(npats, 96);
    // A: a new position after 00, empty; everything else one later, and the tune goes round from 03
    let used: std::collections::BTreeSet<u8> = (0..4u16)
        .flat_map(|tune| (0..96u16).map(move |i| order + tune * 96 + i))
        .map(|a| t.m.peek(a))
        .collect();
    t.press(b'A');
    t.frames(3); // looking through every tune's order for patterns nothing uses takes a frame or two
    assert_eq!((t.get("POS"), end(&t)), (1, 25));
    assert_eq!((pos(&t, 0), pos(&t, 2)), (p0, p1));
    let new = pos(&t, 1);
    assert!(new[0] != new[1] && new[1] != new[2] && new[0] != new[2], "{new:?}");
    for p in new {
        assert!(!used.contains(&p), "pattern {p} of {new:?} was some tune's already; {:?}", pos(&t, 0));
        let at = t.sym("PATTERNS") + p as u16 * 64;
        assert!((0..64u16).all(|i| t.m.peek(at + i) == 0), "pattern {p} is empty");
    }
    assert!(t.line(13).contains("--- .") && !t.line(13).contains("E-1"), "a clean page: {}", t.line(13));
    assert_eq!(t.m.peek(order + 25 * 3 + 1), 3);
    // R: and out again
    t.press(b'R');
    assert_eq!(end(&t), 24);
    assert_eq!((pos(&t, 0), pos(&t, 1)), (p0, p1));
    assert_eq!(t.m.peek(order + 24 * 3 + 1), 2);
    // Y and U: the tempo, down and up, kept by the tune
    t.press(b'Y');
    assert_eq!((t.get("SPEED"), t.m.peek(t.sym("SONGTAB") + 2)), (7, 7));
    assert!(t.line(1).contains("BPM 107"), "{}", t.line(1));
    t.press(b'U');
    t.press(b'U');
    assert_eq!(t.get("SPEED"), 5);
    assert!(t.line(1).contains("BPM 150"), "{}", t.line(1));
    // the slowest there is, and no slower: 18 frames a row
    for _ in 0..20 {
        t.press(b'Y');
    }
    assert_eq!(t.get("SPEED"), 18);
    assert!(t.line(1).contains("BPM  41"), "{}", t.line(1));
    for _ in 0..13 {
        t.press(b'U');
    }
    assert_eq!(t.get("SPEED"), 5);
    // and it still plays, all the way round
    t.press(13);
    t.frames(24 * ROWS * 5 + 20);
    assert_eq!(t.get("POS"), 2);
    // the tempo while it plays: U, and the rows come every four frames from the next one
    t.press(b'U');
    assert_eq!((t.get("PLAYING"), t.get("SPEED")), (1, 4));
    assert!(t.line(1).contains("BPM 187"), "{}", t.line(1));
    t.frames(8);
    let row = t.get("ROW");
    t.frames(4 * 5);
    assert_eq!(t.get("ROW"), row + 5);
    t.press(b'Y');
    t.press(b'Y');
    assert_eq!((t.get("PLAYING"), t.get("SPEED")), (1, 6));
}

/// The instruments' screen: the wave and the envelope drawn, each part of the instrument on a
/// row (up and down choose, left and right change, hex is typed over), its table, and SPACE.
#[test]
fn an_instrument_is_changed_on_its_screen() {
    let mut t = Tape::boot(true);
    let instr = t.sym("INSTR");
    // a screen cell: row, column
    let cell = |t: &Tape, row: u16, col: u16| t.m.peek(0x0400 + row * 40 + col);
    t.press(b'I');
    assert_eq!(t.get("MODE"), 1);
    t.frames(4); // the screen's texts take a frame or two to print
    t.worst = 0;
    let screen = t.m.screen_text_plain();
    assert!(t.line(0).contains("INSTRUMENT 1"), "{screen}");
    assert!(t.line(1).contains("ITS WAVE") && t.line(6).contains("ITS LEVEL"), "{screen}");
    // the string's parts, down the right: the drawn table, no attack, held at full, TRIM 4, it draws
    for (row, text) in [(2, "WAV 10"), (3, "A    0"), (5, "S    F"), (6, "R    2"), (7, "WID 80"), (12, "TRM  4"), (13, "FLG  8")] {
        assert!(t.line(row).contains(text), "row {row}: {text}\n{screen}");
    }
    assert!(t.line(13).contains("00  00 00") && t.line(14).contains("02  FF 00"), "its table: the note, and round\n{screen}");
    // its envelope: straight up, held at the top for eight columns, and down
    assert_eq!(cell(&t, 7, 1), 0x0F, "the top row's first column is full: no attack");
    assert_eq!(cell(&t, 7, 9), 0x0F, "held");
    assert_eq!(cell(&t, 7, 20), b' ', "and let go");
    // P to the pulse lead (4): its wave is drawn half up, half down, as wide as its WIDTH
    for _ in 0..3 {
        t.press(b'P');
    }
    assert!(t.line(0).contains("INSTRUMENT 4"));
    assert!(t.line(2).contains("WAV 04") && t.line(7).contains("WID 80"), "{}", t.m.screen_text_plain());
    assert_eq!((cell(&t, 2, 1), cell(&t, 2, 16), cell(&t, 2, 17), cell(&t, 5, 17)), (0x0F, 0x0F, b' ', 0x60));
    // down to WID, and left twice: the pulse narrows by four each time, and so does the picture
    for _ in 0..5 {
        t.press(31);
    }
    assert_eq!(t.get("ECUR"), 5);
    for _ in 0..8 {
        t.press(28);
    }
    assert_eq!(t.m.peek(instr + 4 * 8 + 3), 0x60);
    t.frames(2);
    assert_eq!((cell(&t, 2, 12), cell(&t, 2, 13)), (0x0F, b' '));
    // up to the attack, right: slower; a digit typed is the digit
    for _ in 0..4 {
        t.press(30);
    }
    assert_eq!((t.get("ECUR"), t.m.peek(instr + 4 * 8 + 1)), (1, 0x38));
    t.press(29);
    assert_eq!(t.m.peek(instr + 4 * 8 + 1), 0x48);
    assert!(t.line(3).contains("A    4"), "{}", t.line(3));
    t.press(b'9');
    assert_eq!(t.m.peek(instr + 4 * 8 + 1), 0x98);
    assert_ne!(cell(&t, 7, 1), 0x0F, "the envelope's first column is no longer at the top");
    // the waveform goes round with left and right
    t.press(30);
    t.press(29);
    assert_eq!(t.m.peek(instr + 4 * 8), 0x08);
    t.press(28);
    assert_eq!(t.m.peek(instr + 4 * 8), 0x04);
    // down into its table, right to the pitch, and twelve semitones typed over it
    for _ in 0..12 {
        t.press(31);
    }
    assert_eq!(t.get("ECUR"), 12);
    t.press(29);
    let table = t.sym("TABLES") + t.m.peek(instr + 4 * 8 + 6) as u16;
    t.press(b'0');
    t.press(b'C');
    assert_eq!(t.m.peek(table + 1), 0x0C);
    assert_eq!((cell(&t, 13, 11), cell(&t, 13, 22), cell(&t, 13, 23)), (0x0F, 0x0F, b' '), "and a bar twelve long");
    // SPACE: heard, an octave up now, and let go by itself
    let at = t.sound.len();
    t.press(b' ');
    assert_eq!(gate(&t, 0), 1);
    assert!((freq(&t, 0) - 523.3).abs() < 6.0, "C-4 and twelve semitones: {}", freq(&t, 0));
    t.frames(40);
    assert!(t.loud(at) > 0.005);
    assert_eq!(gate(&t, 0), 0);
    // the string's picture is the table as the chip has it: it moves while a note of it sounds
    for _ in 0..3 {
        t.press(b'O');
    }
    t.press(b' ');
    t.frames(2);
    let then: Vec<u8> = (1..33).map(|c| cell(&t, 3, c)).collect();
    t.frames(30);
    let now: Vec<u8> = (1..33).map(|c| cell(&t, 3, c)).collect();
    assert_ne!(then, now);
    // all of that, a note sounding or not, inside a frame each time
    assert!(t.worst < CYCLES_PER_FRAME as u64, "a frame's work on the instruments' screen took {} cycles", t.worst);
    println!("worst frame on the instruments' screen {} cycles", t.worst);
    // I: back, and the tune's screen is as it was
    t.press(b'I');
    assert_eq!(t.get("MODE"), 0);
    t.frames(4);
    assert!(t.line(0).contains("ROB'S YOUR UNCLE") && t.line(4).contains("VOICE 1"), "{}", t.m.screen_text_plain());
    // a table that ends by sending you to its own end is silence, not a hang
    t.m.poke(table, 0xFF);
    t.m.poke(table + 1, t.m.peek(instr + 4 * 8 + 6));
    t.press(b'4');
    t.press(b'Z');
    t.frames(10);
    assert_eq!(t.get("PLAYING"), 0);
}

/// SAVE and LOAD: the tracker records itself and what's in it as a program on the tape, and
/// the ROM's LOAD brings it back: the same tracker, with the note that was written.
#[test]
fn the_tracker_saves_itself_to_tape_and_loads_back() {
    let mut t = Tape::boot(true);
    // a note on the first row, voice 1: G-4 on the string, where there was none
    t.press(b'B');
    assert_eq!(t.cell(0, 0), (1 + 4 * 12 + 7, 1));
    t.press(30);
    // and an instrument changed
    let instr = t.sym("INSTR");
    t.m.poke(instr + 9 * 8 + 7, 0x50);
    t.m.insert_blank_tape();
    t.m.set_tape_turbo(true);
    t.press(b'/');
    assert!(t.line(2).contains("NAME, THEN RETURN:"), "{}", t.m.screen_text_plain());
    for k in *b"MINE" {
        t.press(k);
    }
    assert!(t.line(2).contains("RETURN: MINE"), "{}", t.line(2));
    t.press(13);
    assert!(t.line(2).contains("PRESS RECORD AND PLAY ON TAPE"), "{}", t.m.screen_text_plain());
    assert!(t.m.press_record());
    let motor = |t: &Tape| t.m.peek(0xC040) & 4 != 0;
    t.frames(2);
    assert!(motor(&t), "recording");
    for _ in 0..3000 {
        t.frame();
        if !motor(&t) {
            break;
        }
    }
    assert!(!motor(&t), "the motor stops when it's all on the tape");
    t.frames(6);
    assert!(t.line(4).contains("VOICE 1") && !t.line(2).contains("RECORD"), "back at the tune's screen\n{}", t.m.screen_text_plain());
    assert!(t.line(0).contains("MINE") && !t.line(0).contains("UNCLE"), "the name typed is the tune's now: {}", t.line(0));
    let tape = t.m.eject_tape().expect("a tape in the deck");
    let programs = unc_asm::tape::parse(&tape).expect("a good tape");
    assert_eq!(programs.len(), 1);
    let p = &programs[0];
    assert_eq!((p.kind, p.name().as_str(), p.load, p.entry, p.data.len()), (1, "MINE", 0x0800, 0x0800, 0x3800));

    // another machine, the tape in, PLAY down, and the ROM's LOAD from the tracker's own key
    let mut u = Tape::boot(true);
    assert_eq!(u.cell(0, 0), (0, 0));
    u.m.insert_tape(tape);
    u.m.set_tape_turbo(true);
    u.m.press_play();
    u.press(0x5C);
    u.press(13); // no name: the first program
    u.frames(3);
    assert!(!u.line(4).contains("VOICE 1"), "the ROM has the screen\n{}", u.m.screen_text_plain());
    for _ in 0..3000 {
        u.frame();
        if u.line(0).contains("MUSIC TRACKER") && u.line(4).contains("VOICE 1") {
            break;
        }
    }
    u.frames(4);
    assert!(u.line(0).contains("MUSIC TRACKER") && u.line(0).contains("MINE"), "the tune has the name it was saved under\n{}", u.m.screen_text_plain());
    assert_eq!(u.cell(0, 0), (1 + 4 * 12 + 7, 1), "the note that was written");
    u.press(b'T');
    assert!(u.line(0).contains("SWITCHED ON"), "and the others keep theirs: {}", u.line(0));
    u.press(b'T');
    u.press(b'T');
    u.press(b'T');
    assert_eq!(u.m.peek(instr + 9 * 8 + 7), 0x50, "and the instrument");
    u.press(13);
    u.frames(30);
    assert_eq!(u.get("PLAYING"), 1);
}

/// A tune made new: 0 empties it (asking first) into one position of three patterns nothing
/// uses, named TUNE #n; - hands out another unused pattern; ; names the tune.
#[test]
fn a_tune_is_emptied_named_and_given_new_patterns() {
    let mut t = Tape::boot(true);
    let (order, patterns) = (t.sym("ORDER"), t.sym("PATTERNS"));
    // every pattern some tune uses, as the tape comes
    let used = |t: &Tape| {
        let mut u = std::collections::BTreeSet::new();
        for tune in 0..4u16 {
            let at = order + tune * 96;
            let mut i = 0;
            while t.m.peek(at + i) != 0xFF {
                u.insert(t.m.peek(at + i));
                i += 1;
            }
        }
        u
    };
    let before = used(&t);
    assert_eq!(before.len(), 85);
    // 0, and anything but Y: nothing happens
    t.press(b'0');
    assert!(t.line(2).contains("EMPTY THIS TUNE? Y DOES IT"), "{}", t.m.screen_text_plain());
    t.press(b'N');
    t.frames(4);
    assert_eq!(used(&t), before);
    assert!(t.line(0).contains("ROB'S YOUR UNCLE"));
    // 0 and Y
    t.press(b'0');
    t.press(b'Y');
    t.frames(4);
    assert!(t.line(0).contains("TUNE #1") && !t.line(0).contains("UNCLE"), "{}", t.line(0));
    let fresh = [t.m.peek(order), t.m.peek(order + 1), t.m.peek(order + 2)];
    assert_eq!((t.m.peek(order + 3), t.m.peek(order + 4)), (0xFF, 0), "one position, and round to it");
    assert!(fresh[0] != fresh[1] && fresh[1] != fresh[2] && fresh[0] != fresh[2]);
    for p in fresh {
        assert!((0..64u16).all(|i| t.m.peek(patterns + p as u16 * 64 + i) == 0), "pattern {p} is empty");
    }
    // none of them is one another tune plays
    let others: std::collections::BTreeSet<u8> = {
        let mut u = used(&t);
        for p in fresh {
            u.remove(&p);
        }
        u
    };
    assert!(fresh.iter().all(|p| !others.contains(p)));
    assert_eq!((t.get("POS"), t.get("ROW"), t.get("SPEED")), (0, 0, 6));
    assert!(t.line(13).contains("--- ."), "{}", t.m.screen_text_plain());
    // the other tunes are as they were
    t.press(b'T');
    assert!(t.line(0).contains("SWITCHED ON"));
    for _ in 0..3 {
        t.press(b'T');
    }
    // a note, a second position (empty too), and another new pattern there for voice 1
    t.press(b'Z');
    t.press(b'A');
    t.frames(3);
    assert_eq!(t.get("POS"), 1);
    let second = [t.m.peek(order + 3), t.m.peek(order + 4), t.m.peek(order + 5)];
    assert!(second.iter().all(|p| !fresh.contains(p) && !others.contains(p)), "{second:?}");
    assert_eq!(t.cell(0, 0), (0, 0), "an empty page");
    t.press(b'-');
    t.frames(3);
    let new = t.m.peek(order + 3);
    assert!(!fresh.contains(&new) && !others.contains(&new) && !second.contains(&new), "{new}");
    assert_eq!((t.m.peek(order + 4), t.m.peek(order + 5)), (second[1], second[2]), "the other voices keep theirs");
    // ; names it
    t.press(b';');
    for k in *b"MY OWN" {
        t.press(k);
    }
    t.press(13);
    t.frames(4);
    assert!(t.line(0).contains("MUSIC TRACKER") && t.line(0).contains("MY OWN") && !t.line(0).contains("TUNE #"), "{}", t.line(0));
    // and it plays: two positions, round
    t.press(13);
    t.frames(2 * ROWS * 6 + 12);
    assert_eq!((t.get("PLAYING"), t.get("POS")), (1, 0));
}

/// ?: help, every key on a screen of its own, from either screen, and any key goes back to it.
#[test]
fn the_keys_have_a_screen_of_their_own() {
    let mut t = Tape::boot(true);
    assert!(t.line(24).contains("SPACE PLAY T TUNE I INS Z-M PIANO ?HELP"), "{}", t.m.screen_text_plain());
    assert!(t.line(23).trim().is_empty(), "one row, clear of the meters above it");
    t.press(b'?');
    t.frames(6);
    assert_eq!(t.get("MODE"), 2);
    let screen = t.m.screen_text_plain();
    // laid out as the ROM's HELP is: a bar on top, headings, a key and what it does at column 17
    assert!(t.line(0).contains("TRACKER") && t.line(0).contains("BACK"), "{screen}");
    assert_eq!(t.m.peek(0x0400), 0x80, "the bar is reversed, from its first cell");
    assert_eq!(t.m.peek(0x0400 + 40), b'P' | 0x80, "and so are the headings");
    for (row, text) in [(1, "PLAYING"), (2, " RETURN          FROM THE TOP"), (3, " SPACE           PLAY, STOP"), (7, "WRITING, WHEN IT'S STOPPED"),
                        (9, " Z X C V B N M   BOTTOM ROW: A PIANO"), (10, "  S D   G H J    ITS BLACK KEYS"), (11, " , .             OCTAVE DOWN, UP"),
                        (14, "THE TUNE"), (15, " K L  -          ITS PATTERN HERE, NEW"), (18, " 0  ;            EMPTY IT, NAME IT"),
                        (19, " /  \\            SAVE, LOAD ON TAPE"), (20, "INSTRUMENTS: I, AND I BACK"),
                        (22, " LEFT RIGHT 0-F  CHANGE IT"), (24, " FLG 1 FILTER 2 LETGO 4 SLIDE 8 DRAWS")] {
        // (a reversed blank reads as ? in the plain text of the screen)
        assert!(t.line(row).replace('?', " ").starts_with(text), "row {row}: {text}\n{screen}");
    }
    // any key: back, and the key does nothing else (Z would have written a note)
    t.press(b'Z');
    t.frames(6);
    assert_eq!(t.get("MODE"), 0);
    assert!(t.line(0).contains("ROB'S YOUR UNCLE") && t.line(4).contains("VOICE 1"), "{}", t.m.screen_text_plain());
    assert_eq!(t.cell(0, 0), (0, 0));
    // from the instruments' screen, and back to it with the cursor where it was
    t.press(b'I');
    t.frames(4);
    t.press(31);
    t.press(31);
    assert!(t.line(24).contains("SPACE HEAR  O P NEXT  I BACK  ? HELP") && !t.m.screen_text_plain().contains("EMPTY TUNE"), "{}", t.m.screen_text_plain());
    t.press(b'?');
    t.frames(6);
    assert_eq!(t.get("MODE"), 2);
    t.press(b' ');
    t.frames(6);
    assert_eq!((t.get("MODE"), t.get("ECUR")), (1, 2));
    assert!(t.line(0).contains("INSTRUMENT 1") && t.line(2).contains("WAV 10"), "{}", t.m.screen_text_plain());
    t.press(b'I');
    t.frames(4);
    // while a tune plays: it goes on playing behind the keys
    t.press(13);
    t.frames(20);
    t.press(b'?');
    t.frames(30);
    assert_eq!((t.get("MODE"), t.get("PLAYING")), (2, 1));
    let row = t.get("ROW");
    t.frames(30);
    assert_ne!(t.get("ROW"), row);
    t.press(b'X');
    t.frames(8);
    assert_eq!((t.get("MODE"), t.get("PLAYING")), (0, 1));
    assert!(t.line(2).contains("PLAY") && t.line(4).contains("VOICE 1"), "{}", t.m.screen_text_plain());
}
