//! SNAKE (games/snake.s), played through scripted keys on a booted 16K machine.
//! The game's state lives in the fast page, reserved with `.res` at the top of the
//! source; the tests look it up by name from the assembly's symbols.

use unc_asm::Assembly;
use unc_core::machine::{CYCLES_PER_FRAME, PIXELS_H, PIXELS_W, io};
use unc_core::sound::noise;
use unc_core::{Machine, RamSize};

const SRC: &str = include_str!("../../../games/snake.s");

const FOOD: u8 = 0x71;
const BODY: u8 = 0x64;
const HEAD_UP: u8 = 0x60;
const SPACE: u8 = b' ';

struct Game {
    m: Machine,
    asm: Assembly,
    work: u16,
    main: u16,
    work_start: Option<u64>,
    /// Most cycles one frame's work took, from `work` back to `main`.
    worst: u64,
    /// Cycles from `start` to the main loop: the character set copy and the title.
    boot: u64,
}

impl Game {
    /// Boots like the Machine Room: power on, let the ROM run, load, PC = start.
    fn boot() -> Game {
        let asm = unc_asm::assemble(SRC).unwrap_or_else(|e| panic!("snake.s doesn't assemble: {e:?}"));
        assert!(asm.symbol("start").is_some(), "start label");
        let mut m = unc_asm::boot(RamSize::K16, &asm);
        let work = asm.symbol("work").unwrap();
        let main = asm.symbol("main").unwrap();
        let boot = m.run_until(main, 10 * CYCLES_PER_FRAME as u64);
        assert!(boot.is_ok(), "the processor stopped while booting: {boot:?}");
        let boot = boot.unwrap();
        let mut g = Game { m, asm, work, main, work_start: None, worst: 0, boot };
        g.frames(2);
        g
    }

    /// Boots and presses SPACE on the title screen.
    fn start() -> Game {
        let mut g = Game::boot();
        g.press(b' ');
        assert_eq!(g.get("STATE"), 1, "SPACE starts a game");
        g
    }

    fn sym(&self, name: &str) -> u16 {
        self.asm.symbol(name).unwrap_or_else(|| panic!("no symbol {name}"))
    }
    fn get(&self, name: &str) -> u8 {
        self.m.peek(self.sym(name))
    }
    fn set(&mut self, name: &str, v: u8) {
        let a = self.sym(name);
        self.m.poke(a, v);
    }
    fn head(&self) -> (u8, u8) {
        (self.get("HX"), self.get("HY"))
    }
    fn cell(&self, x: u8, y: u8) -> u8 {
        self.m.peek(0x0400 + y as u16 * 40 + x as u16)
    }
    fn poke_cell(&mut self, x: u8, y: u8, v: u8) {
        self.m.poke(0x0400 + y as u16 * 40 + x as u16, v);
    }
    /// A 24-bit number in the fast page: SCORE and HISCORE.
    fn num(&self, name: &str) -> u32 {
        let a = self.sym(name);
        u32::from_le_bytes([self.m.peek(a), self.m.peek(a + 1), self.m.peek(a + 2), 0])
    }
    fn set_num(&mut self, name: &str, v: u32) {
        let a = self.sym(name);
        for (i, b) in v.to_le_bytes()[..3].iter().enumerate() {
            self.m.poke(a + i as u16, *b);
        }
    }
    /// The timer's divisor, as the program last wrote it.
    fn divisor(&self) -> u16 {
        u16::from_le_bytes([self.m.peek(io::TIMER_LO), self.m.peek(io::TIMER_HI)])
    }
    /// SPEAKER bits 0 and 1: gate and data. `%11` while a note plays.
    fn speaker(&self) -> u8 {
        self.m.peek(io::SPEAKER) & 3
    }
    /// The last frame's audio, as the hosts render it.
    fn audio(&mut self) -> Vec<f32> {
        let mut buf = vec![1.0f32; 882];
        self.m.audio_frame(44_100, &mut buf);
        buf
    }
    /// Frames until the gate closes (at most `max`), with the divisors heard on the way.
    fn play_out(&mut self, max: u32) -> (u32, Vec<u16>) {
        let mut notes = vec![self.divisor()];
        for n in 1..=max {
            self.frame();
            if self.speaker() == 0 {
                return (n, notes);
            }
            if notes.last() != Some(&self.divisor()) {
                notes.push(self.divisor());
            }
        }
        panic!("still sounding after {max} frames: {notes:?}");
    }
    /// The screen as text, reverse video read as plain: the blinking prompts read the
    /// same in either phase.
    fn text(&self) -> String {
        self.m.screen_text_plain()
    }
    /// The custom character set's address (`MYSET`) and one glyph in it.
    fn glyph(&self, c: u8) -> Vec<u8> {
        let at = self.sym("MYSET") + c as u16 * 8;
        (0..8).map(|i| self.m.peek(at + i)).collect()
    }

    /// One frame, stepped an instruction at a time to time the game's work: to the next
    /// VBLANK, where FRAME goes up (manual chapter 9), so the work it starts is all inside the
    /// next step.
    fn frame(&mut self) {
        let at = unc_core::machine::VBLANK_LINE * unc_core::machine::CYCLES_PER_LINE;
        let to = match (at + CYCLES_PER_FRAME - self.m.mem.frame_cycle) % CYCLES_PER_FRAME {
            0 => CYCLES_PER_FRAME,
            n => n,
        };
        let end = self.m.cpu.cycles + to as u64;
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
    }
    fn frames(&mut self, n: u32) {
        for _ in 0..n {
            self.frame();
        }
    }
    /// A tap, as the host sends it: the key goes down, one frame, the key comes up.
    fn press(&mut self, k: u8) {
        self.m.key(k);
        self.frame();
        self.m.key_up(k);
    }
    /// Runs until the snake moves (or the game ends). Returns the frames it took.
    fn tick(&mut self) -> u32 {
        let before = self.head();
        for n in 1..=20 {
            self.frame();
            if self.head() != before || self.get("STATE") != 1 {
                return n;
            }
        }
        panic!("the snake didn't move in 20 frames");
    }
    /// Removes every heart from the playfield, so only the ones a test puts down count.
    fn clear_food(&mut self) {
        for y in 2..24 {
            for x in 1..39 {
                if self.cell(x, y) == FOOD {
                    self.poke_cell(x, y, SPACE);
                }
            }
        }
    }
    fn food_count(&self) -> usize {
        self.m.screen()[80..24 * 40].iter().filter(|&&c| c == FOOD).count()
    }
    /// The cell the snake moves into next, going straight on.
    fn ahead(&self) -> (u8, u8) {
        let (x, y) = self.head();
        match self.get("DIR") {
            0 => (x, y - 1),
            1 => (x + 1, y),
            2 => (x, y + 1),
            _ => (x - 1, y),
        }
    }
    fn feed(&mut self) {
        let (x, y) = self.ahead();
        self.poke_cell(x, y, FOOD);
        self.tick();
    }
    fn snake_cells(&self) -> usize {
        self.m.screen().iter().filter(|&&c| (HEAD_UP..=BODY).contains(&c)).count()
    }
}

#[test]
fn title_screen() {
    let g = Game::boot();
    assert_eq!(g.get("STATE"), 0);
    let t = g.text();
    // its own glyphs sit on lowercase's codes: the middle dot is $65 (e), the arrows $76-$79
    assert!(t.contains("SNAKE e PRESS SPACE"), "{t}");
    assert!(t.contains("W A S D OR wvyx TO STEER"), "{t}");
    assert_eq!(g.cell(19, 13), 0x77, "an up arrow");
    assert!(t.contains("HI SCORE 000000"), "{t}");
    // The frame is drawn with UNSCII box glyphs.
    assert_eq!((g.cell(0, 0), g.cell(39, 0), g.cell(0, 24), g.cell(39, 24)), (0x12, 0x13, 0x14, 0x15));
    assert_eq!((g.cell(5, 0), g.cell(0, 5)), (0x10, 0x11));
    // The big title is made of body segments.
    assert_eq!(g.cell(9, 3), BODY);
}

#[test]
fn custom_character_set_is_in_ram_and_used() {
    let mut g = Game::boot();
    let myset = g.sym("MYSET");
    assert_eq!(myset & 0xFF, 0, "on a page boundary");
    assert!(myset >= 0x0800 && myset + 0x400 <= 0x4000, "in a UNC 16's RAM: ${myset:04X}");
    assert_eq!(g.m.peek(io::CHARSET), (myset >> 8) as u8, "CHARSET points at the copy at MYSET");
    for c in [b'A', b'0', 0x10u8, 0x74] {
        let rom: Vec<u8> = (0..8).map(|i| g.m.peek(0xD000 + c as u16 * 8 + i)).collect();
        assert_eq!(rom, g.glyph(c), "glyph ${c:02X} is UNSCII's");
    }
    let right = g.glyph(0x61);
    assert_eq!(right, [0x78, 0xF4, 0xFC, 0xFE, 0xFC, 0xF4, 0x78, 0x00]);

    // On screen, the head cell shows the redefined glyph.
    g.press(b' ');
    let (x, y) = g.head();
    assert_eq!(g.cell(x, y), 0x61, "a head facing right");
    let mut px = vec![0u8; PIXELS_W * PIXELS_H];
    g.m.render(true, &mut px);
    let shown: Vec<u8> = (0..8)
        .map(|r| (0..8).fold(0u8, |b, c| b << 1 | px[(y as usize * 8 + r) * PIXELS_W + x as usize * 8 + c]))
        .collect();
    assert_eq!(shown, right);

    // Turning down redraws the head facing down.
    g.press(b'S');
    g.tick();
    let (x, y) = g.head();
    assert_eq!(g.cell(x, y), 0x62);
}

#[test]
fn the_heart_beats() {
    let mut g = Game::boot();
    let rom: Vec<u8> = (0..8).map(|i| g.m.peek(0xD000 + 0x91 * 8 + i)).collect();
    let mut seen_big = false;
    let mut seen_small = false;
    for _ in 0..40 {
        g.frame();
        let h = g.glyph(FOOD);
        seen_big |= h == rom;
        seen_small |= h != rom && h.iter().any(|&b| b != 0);
    }
    assert!(seen_big && seen_small, "the heart glyph alternates");
}

#[test]
fn space_starts_a_game() {
    let mut g = Game::boot();
    g.press(b'W'); // other keys do nothing on the title
    assert_eq!(g.get("STATE"), 0);
    g.press(b' ');
    assert_eq!(g.get("STATE"), 1);
    let t = g.text();
    let header: Vec<&str> = t.lines().next().unwrap().split_whitespace().collect();
    assert_eq!(header, ["SCORE", "000000", "SNAKE", "HI", "000000"], "{t}");
    // Playfield border: rows 1-24.
    assert_eq!((g.cell(0, 1), g.cell(39, 1), g.cell(0, 24), g.cell(39, 24)), (0x12, 0x13, 0x14, 0x15));
    // Three segments heading right from the middle.
    assert_eq!(g.head(), (20, 12));
    assert_eq!((g.cell(18, 12), g.cell(19, 12), g.cell(20, 12)), (BODY, BODY, 0x61));
    assert_eq!((g.get("LEN"), g.get("DIR")), (3, 1));
    // One heart, on a free cell inside the walls, where FX/FY say.
    assert_eq!(g.food_count(), 1);
    let (fx, fy) = (g.get("FX"), g.get("FY"));
    assert!((1..=38).contains(&fx) && (2..=23).contains(&fy));
    assert_eq!(g.cell(fx, fy), FOOD);
}

#[test]
fn food_depends_on_when_space_is_pressed() {
    let place = |wait: u32| {
        let mut g = Game::boot();
        g.frames(wait);
        g.press(b' ');
        (g.get("FX"), g.get("FY"))
    };
    let spots: std::collections::HashSet<_> = (0..4).map(place).collect();
    assert!(spots.len() >= 3, "different start times give different hearts: {spots:?}");
}

#[test]
fn the_snake_advances_one_cell_per_tick() {
    let mut g = Game::start();
    g.clear_food();
    let speed = g.get("SPEED") as u32;
    assert_eq!(speed, 8);
    g.tick(); // line up with the tick
    for i in 0..5 {
        let (x, y) = g.head();
        let n = g.tick();
        assert_eq!(n, speed, "a move every {speed} frames");
        assert_eq!(g.head(), (x + 1, y), "one cell right, move {i}");
        assert_eq!(g.snake_cells(), 3, "length stays 3");
        assert_eq!(g.cell(x, y), BODY);
        assert_eq!(g.cell(x - 2, y), SPACE, "the tail was rubbed out");
    }
}

#[test]
fn a_key_turns_the_snake() {
    let mut g = Game::start();
    g.clear_food();
    let (x, y) = g.head();
    g.press(b'S');
    g.tick();
    assert_eq!(g.head(), (x, y + 1));
    assert_eq!(g.get("DIR"), 2);
    g.press(b'A');
    g.tick();
    assert_eq!(g.head(), (x - 1, y + 1));
    g.press(b'W');
    g.tick();
    assert_eq!(g.head(), (x - 1, y), "into the cell the tail just left");
    assert_eq!(g.get("STATE"), 1);
    g.press(b'S');
    assert_eq!(g.get("QN"), 0, "down would reverse into the neck");
}

#[test]
fn reversing_into_the_neck_is_ignored() {
    let mut g = Game::start();
    g.clear_food();
    let (x, y) = g.head();
    g.press(b'A'); // straight back
    assert_eq!(g.get("QN"), 0);
    g.press(b'D'); // the way it's already going: auto-repeat of a held key
    g.press(b'D');
    assert_eq!(g.get("QN"), 0);
    g.tick();
    assert_eq!(g.head(), (x + 1, y));
    assert_eq!((g.get("DIR"), g.get("STATE")), (1, 1));
}

#[test]
fn two_quick_turns_are_queued() {
    let mut g = Game::start();
    g.clear_food();
    g.tick();
    let (x, y) = g.head();
    g.press(b'W'); // up...
    g.press(b'S'); // ...and back down again: reverses the queued turn, ignored
    g.press(b'A'); // ...then left, all inside one tick
    g.press(b'S'); // a third turn: the queue holds two
    assert_eq!(g.get("QN"), 2);
    g.tick();
    assert_eq!(g.head(), (x, y - 1));
    g.tick();
    assert_eq!(g.head(), (x - 1, y - 1));
    assert_eq!(g.get("QN"), 0);
}

#[test]
fn eating_grows_the_snake_and_scores() {
    let mut g = Game::start();
    g.clear_food();
    g.frames(30); // the start jingle is over
    assert_eq!(g.speaker(), 0);
    g.feed();
    assert_eq!(g.get("LEN"), 4);
    assert_eq!(g.snake_cells(), 4);
    assert_eq!(g.num("SCORE"), 10);
    assert!(g.text().starts_with(" SCORE 000010 "), "{}", g.text());
    assert_eq!(g.food_count(), 1, "a new heart appears");
    assert_eq!((g.speaker(), g.divisor()), (3, g.sym("G5")), "the timer gulps");
    // Growth holds: the next moves don't shrink it back.
    g.tick();
    g.tick();
    assert_eq!(g.snake_cells(), 4);
}

#[test]
fn speed_rises_with_the_score() {
    let mut g = Game::start();
    g.clear_food();
    for _ in 0..4 {
        g.feed();
        g.clear_food();
    }
    assert_eq!(g.num("SCORE"), 40);
    assert_eq!(g.get("SPEED"), 7);
    g.tick();
    assert_eq!(g.tick(), 7, "a move every 7 frames now");
}

#[test]
fn hitting_the_wall_ends_the_game() {
    let mut g = Game::start();
    g.clear_food();
    g.feed(); // 10 points, a new high score
    g.clear_food();
    let mut moves = 0;
    while g.get("STATE") == 1 {
        g.tick();
        moves += 1;
        assert!(moves < 30);
    }
    assert_eq!(g.get("STATE"), 2);
    assert_eq!(g.head(), (39, 12), "died at the right-hand wall");
    assert_eq!(moves, 18);
    let t = g.text();
    assert!(t.contains("GAME OVER"), "{t}");
    assert!(t.contains("SCORE 000010"), "{t}");
    assert!(t.contains("NEW HIGH SCORE"), "{t}");
    assert_eq!(g.num("HISCORE"), 10);
    assert!(t.lines().next().unwrap().ends_with("HI 000010"), "{t}");
    assert_eq!(g.m.peek(io::BORDER), 2, "the border flashes red");
}

#[test]
fn biting_yourself_ends_the_game() {
    let mut g = Game::start();
    g.clear_food();
    g.feed();
    g.clear_food();
    g.feed();
    g.clear_food();
    assert_eq!(g.get("LEN"), 5);
    for k in *b"SA" {
        g.press(k);
        g.tick();
        assert_eq!(g.get("STATE"), 1);
    }
    g.press(b'W'); // into the third segment
    g.tick();
    assert_eq!(g.get("STATE"), 2);
}

#[test]
fn space_restarts_after_game_over() {
    let mut g = Game::start();
    g.clear_food();
    g.feed();
    while g.get("STATE") == 1 {
        g.tick();
    }
    g.press(b' '); // too soon: thrown away during the pause
    assert_eq!(g.get("STATE"), 2);
    g.frames(50);
    assert_eq!(g.m.peek(io::BORDER), 0, "border back to black");
    g.press(b'D');
    assert_eq!(g.get("STATE"), 2, "only SPACE restarts");
    g.press(b' ');
    assert_eq!(g.get("STATE"), 1);
    assert_eq!((g.get("LEN"), g.get("SPEED"), g.head()), (3, 8, (20, 12)));
    assert_eq!(g.num("SCORE"), 0);
    let t = g.text();
    assert!(!t.contains("GAME OVER"));
    assert!(t.lines().next().unwrap().ends_with("HI 000010"), "the high score stays: {t}");
    assert_eq!(g.snake_cells(), 3);
}

#[test]
fn the_ring_buffer_wraps() {
    // Circle a 10 x 5 loop at one move a frame for 300 moves: the head index
    // wraps past 255 and the snake must stay exactly three cells long.
    let mut g = Game::start();
    g.clear_food();
    g.set("SPEED", 1);
    g.set("TICK", 1);
    let lap: Vec<u8> = [(b'D', 10), (b'S', 5), (b'A', 10), (b'W', 5)]
        .iter()
        .flat_map(|&(k, n)| std::iter::repeat_n(k, n))
        .collect();
    for i in 0..300 {
        g.press(lap[i % lap.len()]);
        assert_eq!(g.get("STATE"), 1, "died on move {i}");
    }
    assert_eq!(g.snake_cells(), 3);
    assert_eq!(g.get("LEN"), 3);
}

#[test]
fn every_frame_fits_the_budget() {
    let mut g = Game::start();
    for _ in 0..3 {
        g.feed();
    }
    g.press(b'S');
    g.tick();
    while g.get("STATE") == 1 {
        g.tick();
    }
    g.frames(60);
    g.press(b' ');
    g.frames(10);
    eprintln!("worst frame: {} cycles of {}; boot (once): {}", g.worst, CYCLES_PER_FRAME, g.boot);
    assert!(g.boot < 2 * CYCLES_PER_FRAME as u64, "boot took {} cycles", g.boot);
    assert!(g.worst < CYCLES_PER_FRAME as u64, "a frame's work took {} cycles", g.worst);
}

#[test]
fn the_cursor_keys_steer_too() {
    let mut g = Game::start();
    g.clear_food();
    let (x, y) = g.head();
    // down, left, up, right: codes 31, 28, 30, 29
    for (k, dir, at) in [(31, 2, (x, y + 1)), (28, 3, (x - 1, y + 1)), (30, 0, (x - 1, y)), (29, 1, (x, y))] {
        g.press(k);
        g.tick();
        assert_eq!((g.get("DIR"), g.head()), (dir, at), "cursor key {k}");
    }
    g.press(28); // left, straight back into the neck
    assert_eq!(g.get("QN"), 0);
}

#[test]
fn a_held_key_steers_as_soon_as_its_turn_is_allowed() {
    // Heading right: hold LEFT (a reversal, ignored), tap UP. The snake goes up one
    // row and, with LEFT still down, comes back left: a U-turn with no key repeat.
    let mut g = Game::start();
    g.clear_food();
    g.tick();
    let (x, y) = g.head();
    g.m.key(28);
    g.frame();
    assert_eq!(g.get("QN"), 0, "the reversal isn't queued");
    g.press(30); // tap up; HELD falls back to LEFT
    assert_eq!(g.m.peek(io::HELD), 28);
    g.tick();
    assert_eq!(g.head(), (x, y - 1));
    g.tick();
    assert_eq!((g.head(), g.get("DIR")), ((x - 1, y - 1), 3), "LEFT, held, steers");
    g.m.key_up(28);
    g.tick();
    assert_eq!(g.head(), (x - 2, y - 1), "straight on once it's let go");

    // A held letter key works the same way, and a tap still wins over a held key.
    g.m.key(b'S'); // down, held
    g.tick();
    assert_eq!(g.head(), (x - 2, y), "the press turns it down");
    g.press(b'A'); // tap left: queued, so it's used rather than the held S
    g.tick();
    assert_eq!(g.head(), (x - 3, y));
    g.tick();
    assert_eq!(g.head(), (x - 3, y + 1), "then the held S again");
    g.m.key_up(b'S');
}

#[test]
fn scores_print_six_digits_wide_and_leave_no_cursor() {
    // NUMSTR and PRNUM print through CHROUT, which no longer draws a cursor ($A0)
    // after the last digit, so the game has nothing to rub out.
    let mut g = Game::start();
    assert!(!g.m.screen().contains(&0xA0), "{}", g.m.screen_text());
    g.clear_food();
    for _ in 0..11 {
        g.feed();
        g.clear_food();
    }
    assert_eq!(g.num("SCORE"), 110);
    assert!(!g.m.screen().contains(&0xA0), "{}", g.m.screen_text());
    assert!(g.text().starts_with(" SCORE 000110 "), "{}", g.text());
}

#[test]
fn prompts_blink_in_steady_reverse_video() {
    // VMODE bit 3 holds a reversed space steady, so the prompt flips as a whole,
    // spaces included, and the hardware doesn't blink it on its own.
    let mut g = Game::boot();
    assert_ne!(g.m.peek(io::VMODE) & 8, 0, "STEADY is on");
    assert_eq!(g.m.peek(io::VMODE) & 7, 0, "text mode, 128 glyphs");
    let (gap, letter) = ((23, 17), (24, 17)); // "PRESS SPACE": the space, and the S after it
    let mut phases = std::collections::HashSet::new();
    for _ in 0..40 {
        g.frame();
        let (s, l) = (g.cell(gap.0, gap.1), g.cell(letter.0, letter.1));
        assert!((s, l) == (b' ', b'S') || (s, l) == (0xA0, b'S' | 0x80), "{s:02X} {l:02X}");
        phases.insert(s);
    }
    assert_eq!(phases.len(), 2, "it blinks");
    while g.cell(gap.0, gap.1) != 0xA0 {
        g.frame();
    }
    let (mut on, mut off) = (vec![0u8; PIXELS_W * PIXELS_H], vec![0u8; PIXELS_W * PIXELS_H]);
    g.m.render(true, &mut on);
    g.m.render(false, &mut off);
    assert!(on == off, "the reversed space doesn't blink by itself");
}

#[test]
fn starts_from_the_defined_start_state() {
    // No start-up code clears the fast page: the high score starts at 0 because
    // A is 0 at start, and the title silences the sequencer, whatever was in
    // memory before.
    let asm = unc_asm::assemble(SRC).unwrap();
    let mut m = Machine::boot(RamSize::K16);
    for a in 0..0x30 {
        m.poke(a, 0xA5);
    }
    unc_asm::load(&mut m, &asm);
    m.run_until(asm.symbol("main").unwrap(), 10 * CYCLES_PER_FRAME as u64).unwrap();
    let hi = asm.symbol("HISCORE").unwrap();
    assert_eq!((m.peek(hi), m.peek(hi + 1), m.peek(hi + 2)), (0, 0, 0));
    assert!(m.screen_text_plain().contains("HI SCORE 000000"));
    for _ in 0..60 {
        m.run_frame();
        assert_eq!(m.peek(io::SPEAKER) & 3, 0, "no stray sound from the old SNDT");
    }
}

// ---------- sound: the timer, stepped by a per-frame sequencer ----------

#[test]
fn eating_plays_a_gulp_on_the_timer_then_closes_the_gate() {
    let mut g = Game::start();
    g.clear_food();
    g.frames(30);
    g.feed();
    // The note is written to the timer and the gate opened on the frame of the bite...
    assert_eq!(g.speaker(), 3, "gate and data on");
    assert!(g.audio().iter().any(|&s| s != 0.0), "and it's heard");
    // ...G5 for 2 frames, G6 for 3, then the gate closes: 5 frames in all.
    let (frames, notes) = g.play_out(10);
    assert_eq!(notes, [g.sym("G5"), g.sym("G6")]);
    assert_eq!(frames, 5, "closed within the effect's length");
    assert_eq!(g.get("SNDT"), 0);
}

#[test]
fn every_event_has_its_sound() {
    // SPACE: a rising arpeggio, C5 G5 C6 E6 for 3 frames each, G6 for 9.
    let mut g = Game::boot();
    assert_eq!(g.speaker(), 0);
    g.press(b' ');
    let (frames, notes) = g.play_out(30);
    assert_eq!(notes, ["C5", "G5", "C6", "E6", "G6"].map(|n| g.sym(n)));
    assert_eq!(frames, 21);

    // Every fourth heart the snake speeds up, and the gulp becomes a trill.
    g.clear_food();
    for _ in 0..3 {
        g.feed();
        g.clear_food();
    }
    g.feed();
    assert_eq!(g.get("SPEED"), 7);
    let (frames, notes) = g.play_out(20);
    assert_eq!(notes, ["C7", "G6", "C7", "G6", "C7"].map(|n| g.sym(n)));
    assert_eq!(frames, 12);

    // A new sound cuts off the one playing: one voice.
    g.clear_food();
    g.feed();
    g.frames(2);
    assert_eq!((g.speaker(), g.divisor()), (3, g.sym("G6")), "the gulp's second note");
    g.set("TICK", 1);
    g.poke_cell(g.ahead().0, g.ahead().1, FOOD);
    g.frame(); // bites again while it plays
    assert_eq!((g.speaker(), g.divisor(), g.get("SNDT")), (3, g.sym("G5"), 2), "the gulp starts again");

    // Dying: a slide down, a new (lower) note every frame, then a thud, then silence.
    g.frames(10);
    g.clear_food();
    while g.get("STATE") == 1 {
        g.tick();
    }
    assert_eq!(g.speaker(), 3);
    let (frames, notes) = g.play_out(60);
    assert_eq!(frames, 33 + 10 - 1, "32 frames of slide and 10 of thud");
    assert_eq!(notes.len(), 33);
    assert!(notes[0] < 2000 && notes[31] > 5500 && notes[32] == 9000, "falls from ~670 Hz to 111: {notes:?}");
    let every8: Vec<u16> = notes.iter().step_by(8).copied().collect();
    assert!(every8.windows(2).all(|w| w[1] > w[0]), "lower and lower: {notes:?}");
    assert!(notes.windows(2).any(|w| w[1] < w[0]), "with a wobble: {notes:?}");
    // The rest of the game over is silent.
    for _ in 0..30 {
        g.frame();
        assert_eq!(g.speaker(), 0);
    }
    assert!(g.audio().iter().all(|&s| s == 0.0));
}

#[test]
fn the_game_is_silent_at_rest() {
    let mut g = Game::boot();
    for _ in 0..100 {
        g.frame();
        assert_eq!(g.speaker(), 0, "the title is silent");
    }
    assert!(g.audio().iter().all(|&s| s == 0.0));
    g.press(b' ');
    g.clear_food();
    g.frames(25); // the start jingle plays out
    for _ in 0..60 {
        g.frame(); // the snake moves, turns, but eats nothing
        assert_eq!((g.speaker(), g.get("SNDT")), (0, 0), "silent between sounds");
    }
    g.press(b'S');
    g.frames(30);
    assert_eq!(g.speaker(), 0);
    assert!(g.audio().iter().all(|&s| s == 0.0));
}

// ---------- numbers ----------

#[test]
fn the_score_passes_9999_and_65535() {
    let mut g = Game::start();
    g.clear_food();
    g.set_num("SCORE", 9_990);
    g.feed();
    assert_eq!(g.num("SCORE"), 10_000);
    assert!(g.text().starts_with(" SCORE 010000 "), "{}", g.text());
    // The score is 24 bits: a snake that has stopped growing can keep eating.
    g.clear_food();
    g.set_num("SCORE", 65_530);
    g.feed();
    assert_eq!(g.num("SCORE"), 65_540);
    assert!(g.text().starts_with(" SCORE 065540 "), "{}", g.text());
    g.clear_food();
    while g.get("STATE") == 1 {
        g.tick();
    }
    let t = g.text();
    assert!(t.contains("SCORE 065540") && t.contains("NEW HIGH SCORE"), "{t}");
    assert!(t.lines().next().unwrap().ends_with("HI 065540"), "{t}");
    assert_eq!(g.num("HISCORE"), 65_540);
    // A new game prints 000000 over it.
    g.frames(50);
    g.press(b' ');
    assert!(g.text().starts_with(" SCORE 000000 "), "{}", g.text());
    assert!(g.text().lines().next().unwrap().ends_with("HI 065540"));
    // Only a higher score replaces the high score: compared on all three bytes.
    g.clear_food();
    g.set_num("SCORE", 0x01_0000 - 10); // more than 65540 in the low two bytes
    g.feed();
    g.clear_food();
    while g.get("STATE") == 1 {
        g.tick();
    }
    assert_eq!(g.num("HISCORE"), 65_540, "65530 isn't a high score");
    assert!(!g.text().contains("NEW HIGH"));
}

#[test]
fn the_seed_comes_from_noise_through_rndseed() {
    // At the moment SPACE starts a game, two NOISE reads go to RNDSEED as X and Y.
    let mut g = Game::boot();
    g.m.key(b' ');
    let newgame = g.sym("newgame");
    g.m.run_until(newgame, 2 * CYCLES_PER_FRAME as u64).unwrap();
    let c0 = g.m.cpu.cycles;
    g.m.step(); // LDX NOISE
    let c1 = g.m.cpu.cycles;
    g.m.step(); // LDY NOISE
    let (x, y) = (g.m.cpu.x, g.m.cpu.y);
    assert_eq!((x, y), (noise(c0), noise(c1)), "read from NOISE, on these cycles");
    g.m.run_until(newgame + 9, 1000).unwrap(); // past JSR RNDSEED
    assert_eq!((g.m.peek(0x020B), g.m.peek(0x020C)), (x, y), "SEED is X and Y");

    // So the heart depends on the moment, and not at all on SEED before it.
    let first_heart = |wait: u32, seed: u8| {
        let mut g = Game::boot();
        g.m.poke(0x020B, seed);
        g.m.poke(0x020C, seed);
        g.frames(wait);
        g.press(b' ');
        (g.get("FX"), g.get("FY"))
    };
    assert_eq!(first_heart(5, 0x11), first_heart(5, 0x77), "the old SEED doesn't count");
    assert_ne!(first_heart(5, 0x11), first_heart(6, 0x11), "the moment does");
}

// ---------- the keyboard matrix ----------

#[test]
fn two_held_keys_both_steer() {
    // Heading right, hold UP and LEFT together. HELD shows only LEFT, the last one
    // down, which is a reversal; the matrix shows UP too. So the snake turns up,
    // then left (a U-turn without a tap), and while both stay down it climbs a
    // staircase: up, left, up, left.
    let mut g = Game::start();
    g.clear_food();
    g.tick();
    let (x, y) = g.head();
    g.m.key(30);
    g.m.key(28);
    assert_eq!(g.m.peek(io::HELD), 28, "HELD sees one key");
    let mut path = vec![];
    for _ in 0..4 {
        g.tick();
        path.push((g.head(), g.get("DIR")));
    }
    assert_eq!(path, [((x, y - 1), 0), ((x - 1, y - 1), 3), ((x - 1, y - 2), 0), ((x - 2, y - 2), 3)]);
    g.m.key_up(30);
    g.m.key_up(28);
    g.tick();
    g.tick();
    assert_eq!((g.head(), g.get("DIR")), ((x - 4, y - 2), 3), "keys up: straight on");

    // The letter keys too. Heading left, hold S and D: D is a reversal until S has
    // turned the snake down, then it's a turn: down, right, down, right.
    let (x, y) = g.head();
    g.m.key(b'S');
    g.m.key(b'D');
    let mut path = vec![];
    for _ in 0..4 {
        g.tick();
        path.push(g.head());
    }
    assert_eq!(path, [(x, y + 1), (x + 1, y + 1), (x + 1, y + 2), (x + 2, y + 2)]);
    g.m.key_up(b'S');
    g.m.key_up(b'D');
    assert_eq!(g.get("STATE"), 1);
}
