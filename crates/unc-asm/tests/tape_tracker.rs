//! AUNT TRACKER (games/sound/tracker.s), the sound chip's demonstration cassette, worked
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
    assert!(t.line(0).contains("AUNT TRACKER") && t.line(0).contains("ROB'S YOUR UNCLE"), "{screen}");
    assert!(t.line(1).contains("POS 00") && t.line(1).contains("SPD 6"), "{screen}");
    assert!(t.line(2).contains("STOP") && t.line(2).contains("FILTER IN"), "{screen}");
    assert!(t.line(4).contains("VOICE 1") && t.line(4).contains("VOICE 3"), "{screen}");
    // row 0 is the bright one, in the middle; nothing above it
    assert!(t.line(12).trim().is_empty(), "{screen}");
    assert!(t.line(13).contains("00") && t.line(13).contains("E-5 7") && t.line(13).contains("A-3 9"), "{screen}");
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
    // the notes are on the chip: voice 1's FREQ is E5, 659.3 Hz
    let freq = u16::from_le_bytes([t.m.peek(AUNT), t.m.peek(AUNT + 1)]) as f64 * 1e6 / (1u64 << 24) as f64;
    assert!((freq - 659.3).abs() < 12.0, "{freq}");
    t.press(b' ');
    assert_eq!(t.get("PLAYING"), 0);
    for v in 0..3u16 {
        assert_eq!(t.m.peek(AUNT + v * 8 + chip::CONTROL as u16) & 1, 0, "voice {v} let go");
    }
    let row = t.get("ROW");
    t.frames(20);
    assert_eq!(t.get("ROW"), row, "stopped");
}

#[test]
fn the_whole_tune_plays_in_time_and_comes_round() {
    let mut t = Tape::boot(true);
    t.press(13);
    let mut seen = vec![false; 20];
    // 20 positions, then on from position 4
    for _ in 0..(20 * ROWS * SPEED + 40) {
        t.frame();
        seen[t.get("POS") as usize] = true;
    }
    assert!(seen.iter().all(|&s| s), "{seen:?}");
    assert_eq!(t.get("POS"), 4, "round again from the first A");
    assert!(t.worst < CYCLES_PER_FRAME as u64, "a frame's work took {} cycles", t.worst);
    let peak = t.sound.iter().fold(0.0f32, |a, v| a.max(v.abs()));
    assert!((0.2..0.8).contains(&peak), "{peak}");
    println!("worst frame {} cycles, peak {peak}", t.worst);
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
    // the bass goes through the filter; F takes it out
    let mut through = 0;
    for _ in 0..SPEED * 16 {
        t.frame();
        through |= t.m.peek(0xC0D9) & 7;
    }
    assert_eq!(through, 0b100, "voice 3, whenever it's the bass and not a drum");
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
