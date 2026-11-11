//! DEBUG (games/debug.s): a bitmap-mode formation shooter for a 16K UNC 16.
//! The game is played through scripted keys; its state is read from the fast-page
//! addresses documented at the top of the source.

use unc_core::machine::io;
use unc_core::{Machine, RamSize, Stop};

const SRC: &str = include_str!("../../../games/debug.s");
const BUDGET: u64 = 19_968;
const CY: u8 = 156;

struct Game {
    m: Machine,
    asm: unc_asm::Assembly,
    /// Most cycles one pass of the main loop took (fbegin to fdone), by the state it
    /// started and ended in: (1, 1) is a frame of play, (0, 1) starting a game, and so on.
    worst: std::collections::HashMap<(u8, u8), u64>,
    /// Cycles and passes of the main loop that started and ended in play: (1, 1).
    play: (u64, u64),
    begun: Option<(u64, u8)>,
}

impl Game {
    /// Boots the ROM like the Machine Room, loads the game and runs its start-up.
    fn boot() -> Game {
        let asm = unc_asm::assemble(SRC).unwrap_or_else(|e| panic!("debug.s doesn't assemble: {e:?}"));
        assert!(asm.symbol("start").is_some(), "start label");
        let m = unc_asm::boot(RamSize::K16, &asm);
        let mut g = Game { m, asm, worst: Default::default(), play: (0, 0), begun: None };
        g.frames(12);
        g
    }

    /// Boots and presses SPACE on the title screen. Exceptions off unless a test turns them on.
    fn play() -> Game {
        Game::play_after(0)
    }

    /// The same, with SPACE pressed `wait` frames later: a different moment, so a different
    /// seed for RND.
    fn play_after(wait: u32) -> Game {
        let mut g = Game::boot();
        g.frames(wait);
        g.press(b' ');
        g.until(5, "SPACE starts the game", |g| g.get("STATE") == 1);
        g.set("NOBOMB", 1);
        g
    }

    fn sym(&self, name: &str) -> u16 {
        self.asm.symbol(name).unwrap_or_else(|| panic!("no symbol {name}"))
    }
    fn get(&self, name: &str) -> u8 {
        self.m.peek(self.sym(name))
    }
    fn at(&self, name: &str, i: u16) -> u8 {
        self.m.peek(self.sym(name) + i)
    }
    fn set(&mut self, name: &str, v: u8) {
        let a = self.sym(name);
        self.m.poke(a, v);
    }
    fn set_at(&mut self, name: &str, i: u16, v: u8) {
        let a = self.sym(name) + i;
        self.m.poke(a, v);
    }

    /// Runs one frame, timing the game's work between `fbegin` and `fdone`.
    fn frame(&mut self) {
        let (b, d) = (self.sym("fbegin"), self.sym("fdone"));
        self.m.set_breakpoint(b, true);
        self.m.set_breakpoint(d, true);
        let f0 = self.m.mem.io.frame;
        while self.m.mem.io.frame == f0 {
            // to the next VBLANK, where FRAME goes up (manual chapter 9): the work it starts
            // is all inside the next step
            let at = unc_core::machine::VBLANK_LINE * unc_core::machine::CYCLES_PER_LINE;
            let left = match (at + BUDGET as u32 - self.m.mem.frame_cycle) % BUDGET as u32 {
                0 => BUDGET as u32,
                n => n,
            };
            match self.m.run_cycles(left) {
                Stop::Budget => break,
                Stop::Breakpoint(pc) if pc == b => self.begun = Some((self.m.cpu.cycles, self.get("STATE"))),
                Stop::Breakpoint(pc) if pc == d => {
                    if let Some((t, st)) = self.begun.take() {
                        let (key, c) = ((st, self.get("STATE")), self.m.cpu.cycles - t);
                        let w = self.worst.entry(key).or_default();
                        *w = (*w).max(c);
                        if key == (1, 1) {
                            self.play = (self.play.0 + c, self.play.1 + 1);
                        }
                    }
                }
                s => panic!("machine stopped: {s:?} at ${:04X}", self.m.cpu.pc),
            }
        }
        self.m.clear_breakpoints();
    }

    fn worst(&self, from: u8, to: u8) -> u64 {
        self.worst.get(&(from, to)).copied().unwrap_or(0)
    }

    fn frames(&mut self, n: u32) {
        for _ in 0..n {
            self.frame();
        }
    }

    /// A tap: the key goes down (into KBD and HELD) for one frame, then comes up.
    fn press(&mut self, key: u8) {
        self.m.key(key);
        self.frame();
        self.m.key_up(key);
    }

    /// Holds a key down for `frames` frames, then lets it go.
    fn hold(&mut self, key: u8, frames: u32) {
        self.m.key(key);
        self.frames(frames);
        self.m.key_up(key);
    }

    fn until(&mut self, max: u32, what: &str, f: impl Fn(&Game) -> bool) -> u32 {
        for i in 0..max {
            if f(self) {
                return i;
            }
            self.frame();
        }
        panic!("{what}: not within {max} frames");
    }

    fn pixels(&self) -> Vec<u8> {
        let mut px = vec![0u8; 320 * 200];
        self.m.render(true, &mut px);
        px
    }

    fn text_row(&self, row: usize) -> String {
        self.m.screen_text().lines().nth(row).unwrap_or("").to_string()
    }

    fn score(&self) -> u32 {
        (0..6).fold(0, |s, i| s * 10 + self.at("SCORE", i) as u32)
    }

    /// The left x, in 2-px units, of bug (row, col) as drawn.
    fn bug_x(&self, row: u16, col: u8) -> u8 {
        self.at("ROWX", row) - 64 + 8 * col
    }

    /// Lit pixels in the 8 x 8 cell whose top-left pixel is (x, y).
    fn lit(&self, px: &[u8], x: usize, y: usize) -> usize {
        (0..8).map(|dy| (0..8).filter(|dx| px[(y + dy) * 320 + x + dx] == 1).count()).sum()
    }

    /// The 8 x 8 picture at pixel (x, y), one byte per line, bit 7 leftmost.
    fn cell(&self, px: &[u8], x: usize, y: usize, lines: usize) -> Vec<u8> {
        (0..lines).map(|dy| (0..8).fold(0u8, |b, dx| b << 1 | px[(y + dy) * 320 + x + dx])).collect()
    }

    /// Bug sprite `k` (species * 2 + frame), 8 lines, from the sprite table: the
    /// game keeps one copy of each sprite and the shifter moves it into place.
    fn bug(&self, k: u16) -> Vec<u8> {
        (0..8).map(|i| self.at("spr", 8 * k + i)).collect()
    }

    /// Sprite `name` (CANNON, BOMB, PATCH...), its first `lines` lines.
    fn obj(&self, name: &str, lines: u16) -> Vec<u8> {
        (0..lines).map(|i| self.at(&format!("spr_{name}"), i)).collect()
    }

    /// Puts a shot just under bug (row, col), where the next frame moves it into the bug.
    fn shoot_at(&mut self, row: u16, col: u8) {
        let x = self.bug_x(row, col) + 1;
        let y = self.at("ROWY", row) + 8;
        self.set_at("SACT", 0, 1);
        self.set_at("SX", 0, x);
        self.set_at("SY", 0, y);
        self.set_at("SOFF", 0, 3);
        self.set_at("SHIT", 0, 1);
    }

    /// Shoots bug (row, col) until it's dead. A shot can miss when its row marches
    /// two steps in one frame; then it flies on (and may hit another bug).
    fn kill(&mut self, row: u16, col: u8) {
        for _ in 0..4 {
            if self.at("ALIVE", row) & 1 << col == 0 {
                return;
            }
            self.shoot_at(row, col);
            self.frame();
            self.until(40, "the shot lands", |g| g.at("SACT", 0) == 0);
        }
        panic!("bug {row},{col} survived");
    }

    /// SPEAKER's gate (bit 0) and data (bit 1).
    fn speaker(&self) -> u8 {
        self.m.peek(io::SPEAKER) & 3
    }

    /// The divisor last written to the timer.
    fn divisor(&self) -> u16 {
        u16::from_le_bytes([self.m.peek(io::TIMER_LO), self.m.peek(io::TIMER_HI)])
    }

    /// A sound effect's priority and its steps, (frames, divisor), read from the program.
    fn effect(&self, name: &str) -> (u8, Vec<(u8, u16)>) {
        let a = self.sym(name);
        let steps = (0..)
            .map(|k| a + 1 + 3 * k)
            .take_while(|&s| self.m.peek(s) != 0)
            .map(|s| (self.m.peek(s), u16::from_le_bytes([self.m.peek(s + 1), self.m.peek(s + 2)])))
            .collect();
        (self.m.peek(a), steps)
    }

    /// One frame of the speaker's sound, as the hosts play it.
    fn audio(&mut self) -> Vec<f32> {
        let mut buf = vec![1.0f32; 882];
        self.m.audio_frame(44_100, &mut buf);
        buf
    }

    /// Frames until the hosts' sound is exactly 0: the speaker at rest, and the output
    /// filter settled.
    fn until_silent(&mut self) -> u32 {
        for n in 0..10 {
            if self.audio().iter().all(|&s| s == 0.0) {
                return n;
            }
            self.frame();
        }
        panic!("not silent within 10 frames");
    }

    /// Lit pixels in a rectangle.
    fn lit_in(px: &[u8], x0: usize, y0: usize, w: usize, h: usize) -> usize {
        (y0..y0 + h).map(|y| (x0..x0 + w).filter(|&x| px[y * 320 + x] == 1).count()).sum()
    }
}


#[test]
fn the_program_fits_below_the_bitmap() {
    // The source's .asserts check it ends below the bitmap and its variables below $D0.
    let asm = unc_asm::assemble(SRC).unwrap();
    let size: usize = asm.segments.iter().map(|s| s.bytes.len()).sum();
    let (fx, spr, end) = (asm.symbol("fxtab").unwrap(), asm.symbol("spr").unwrap(), asm.symbol("progend").unwrap());
    eprintln!(
        "debug.s: {size} bytes, $0800-${:04X}: {} of code and text, {} of sound effects, {} of sprites",
        end - 1,
        fx - 0x0800,
        spr - fx,
        end - spr
    );
    assert_eq!(size, (end - 0x0800) as usize, "one segment: the line tables are reserved, built at start-up");
    assert_eq!(end - spr, 80, "ten sprites, 8 bytes each: no shifted copies");
    for name in ["STATE", "CX", "LIVES", "WAVE", "COUNT", "ALIVE", "FXB", "FY", "SCORE", "ROWX", "ROWY", "LATE", "PROF"] {
        assert!(asm.symbol(name).unwrap() < 0xD0, "{name} is in the fast page, clear of the ROM's $D0-$FE");
    }
}

#[test]
fn title_screen_shows_the_name_and_waits_for_space() {
    let mut g = Game::boot();
    assert_eq!(g.get("STATE"), 0);
    assert_eq!(g.m.peek(0xC000), 0x07, "bitmap + mixed mode, 256 glyphs");
    assert_eq!(g.m.peek(0xC023), 0x20, "bitmap at $2000");
    assert!(g.text_row(23).contains("DEBUG - PRESS SPACE"), "{}", g.text_row(23));
    assert!(g.text_row(24).starts_with("A/D ?? MOVE  SPACE FIRE"), "{}", g.text_row(24));
    assert_eq!((g.m.peek(0x07C4), g.m.peek(0x07C5)), (0x96, 0x98), "arrows for the cursor keys");
    let px = g.pixels();
    // "DEBUG" from the character ROM, four times size, at (80, 8).
    for (i, ch) in "DEBUG".bytes().enumerate() {
        for gy in 0..8 {
            let row = unc_core::charset::CHAR_ROM[ch as usize * 8 + gy];
            for gx in 0..8 {
                let want = row >> (7 - gx) & 1;
                for k in 0..16 {
                    let (x, y) = (80 + 32 * i + 4 * gx + k % 4, 8 + 4 * gy + k / 4);
                    assert_eq!(px[y * 320 + x], want, "'{}' at ({x}, {y})", ch as char);
                }
            }
        }
    }
    // The legend's bugs walk: their legs change every 16 frames.
    let before = g.cell(&px, 96, 64, 8);
    g.frames(16);
    let after = g.cell(&g.pixels(), 96, 64, 8);
    assert_ne!(before, after, "legs move");
    assert!([g.bug(0), g.bug(1)].contains(&after));
    // Only SPACE starts.
    g.press(b'A');
    g.press(b'D');
    g.frames(3);
    assert_eq!(g.get("STATE"), 0);
    g.press(b' ');
    g.until(5, "SPACE starts", |g| g.get("STATE") == 1);
}

#[test]
fn a_new_game_draws_the_formation_cannon_and_hud() {
    let g = Game::play();
    assert_eq!((g.get("LIVES"), g.get("WAVE"), g.get("COUNT")), (3, 1, 40));
    assert_eq!(g.score(), 0);
    assert_eq!(g.text_row(21), "SCORE 000000  HI 000000  LIVES 3 WAVE 01");
    let px = g.pixels();
    for row in 0..5u16 {
        assert_eq!(g.at("ALIVE", row), 0xFF);
        let species = [0, 1, 1, 2, 2][row as usize];
        let frames = [g.bug(species * 2), g.bug(species * 2 + 1)];
        for col in 0..8 {
            let (x, y) = (2 * g.bug_x(row, col) as usize, g.at("ROWY", row) as usize);
            assert!(frames.contains(&g.cell(&px, x, y, 8)), "bug {row},{col} at ({x}, {y})");
        }
    }
    let cx = 2 * g.get("CX") as usize;
    assert_eq!(g.cell(&px, cx, CY as usize, 8), g.obj("CANNON", 8), "the cursor cannon");
    assert_eq!(g.get("LATE"), 0);
}

#[test]
fn the_formation_marches_and_steps_down_at_the_edge() {
    let mut g = Game::play();
    let (fx0, fy0) = (g.get("FXB"), g.get("FY"));
    let x0 = g.bug_x(4, 0);
    g.frames(40);
    assert_eq!(g.get("DIR"), 0, "marching right");
    assert!(g.get("FXB") > fx0, "moved right");
    assert!(g.bug_x(4, 0) > x0);
    assert_eq!(g.get("FY"), fy0);
    // Each row redraws on its own turn, bottom row first: never more than a step apart.
    for r in 0..5 {
        assert!(g.get("FXB") - g.at("ROWX", r) <= 1);
    }
    let px = g.pixels();
    let x = 2 * g.bug_x(4, 0) as usize;
    assert!(g.lit(&px, x, g.at("ROWY", 4) as usize) > 0, "the bug is drawn where it now is");
    assert_eq!(Game::lit_in(&px, x - 8, g.at("ROWY", 4) as usize, 8, 8), 0, "and not where it was");
    let n = g.until(400, "step down", |g| g.get("FY") != fy0);
    eprintln!("stepped down after {} frames", n + 40);
    assert_eq!(g.get("FY"), fy0 + 4);
    assert_eq!(g.get("DIR"), 1, "now marching left");
    assert_eq!(g.get("FXB") - 64 + 56, 151, "the right-hand bugs were at the right wall");
    g.frames(8);
    let px = g.pixels();
    for r in 0..5 {
        let y = g.at("ROWY", r);
        assert_eq!(y, fy0 + 4 + 12 * r as u8, "row {r} stepped down");
        let x = 2 * g.bug_x(r, 0) as usize;
        assert_eq!(Game::lit_in(&px, x, y as usize - 4, 128, 4), 0, "row {r}: the lines it left are clear");
    }
    let fx = g.get("FXB");
    g.frames(30);
    assert!(g.get("FXB") < fx, "moving left");
}

#[test]
fn a_and_d_move_the_cannon_and_the_walls_stop_it() {
    let mut g = Game::play();
    assert_eq!(g.get("CX"), 77);
    g.press(b'D');
    g.frames(10);
    assert_eq!(g.get("CX"), 85, "a tap moves one column: 16 px");
    let px = g.pixels();
    assert_eq!(g.cell(&px, 170, CY as usize, 8), g.obj("CANNON", 8));
    assert_eq!(Game::lit_in(&px, 154, CY as usize, 16, 8), 0, "the old cannon is gone");
    g.press(b'A');
    g.frames(10);
    assert_eq!(g.get("CX"), 77);
    g.hold(b'D', 120);
    assert_eq!(g.get("CX"), 151, "the right wall");
    g.hold(b'D', 10);
    assert_eq!(g.get("CX"), 151);
    assert_eq!(g.cell(&g.pixels(), 302, CY as usize, 8), g.obj("CANNON", 8));
    g.hold(b'A', 120);
    assert_eq!(g.get("CX"), 4, "the left wall");
    g.press(b'A');
    g.frames(10);
    assert_eq!(g.get("CX"), 4);
    assert_eq!(g.cell(&g.pixels(), 8, CY as usize, 8), g.obj("CANNON", 8));
}

#[test]
fn held_keys_move_until_released_and_a_tap_moves_one_column() {
    let mut g = Game::play();
    // Held: a step every frame, 2 px, then 4 px from the 16th frame on.
    g.m.key(b'D');
    let mut xs = vec![g.get("CX")];
    for _ in 0..30 {
        g.frame();
        xs.push(g.get("CX"));
    }
    assert!(xs[..16].windows(2).all(|w| w[1] == w[0] + 1), "moves every frame while held: {xs:?}");
    assert!(xs[16..].windows(2).all(|w| w[1] == w[0] + 2), "and speeds up after 16 frames: {xs:?}");
    // Let go: it stops at once.
    g.m.key_up(b'D');
    g.frames(5);
    assert_eq!(g.get("CX"), xs[30], "stops when the key comes up");
    // A tap, however short, moves one column; so do the cursor keys.
    let x = g.get("CX");
    g.press(b'A');
    g.frames(10);
    assert_eq!(g.get("CX"), x - 8, "a tap moves one column");
    g.press(29);
    g.frames(10);
    assert_eq!(g.get("CX"), x, "cursor right");
    g.hold(28, 12);
    assert_eq!(g.get("CX"), x - 12, "cursor left, held");
    g.frames(3);
    assert_eq!(g.get("CX"), x - 12);
    // Fire while moving. HELD shows only SPACE once it goes down on top of D, but the
    // cannon keeps going and fires; when SPACE comes up HELD shows D again.
    g.m.key(b'D');
    g.frames(3);
    let x = g.get("CX");
    g.m.key(b' ');
    g.frame();
    assert_eq!(g.at("SACT", 0), 1, "the shot went");
    assert_eq!(g.get("CX"), x + 1, "and the cannon kept moving");
    g.frames(4);
    assert_eq!(g.get("CX"), x + 5, "while SPACE is held on top");
    g.m.key_up(b' ');
    g.frames(3);
    assert_eq!(g.get("CX"), x + 8, "and after it comes up");
    g.m.key_up(b'D');
    g.frames(3);
    assert_eq!(g.get("CX"), x + 8, "stopped");
    // D let go under a held SPACE: HELD falls back to SPACE with no new press, so the
    // cannon stops instead of sliding on.
    g.m.key(b' ');
    g.frame();
    g.m.key(b'D');
    g.frames(10);
    let x = g.get("CX");
    g.m.key_up(b'D');
    g.frames(3);
    assert_eq!(g.get("CX"), x, "D came up under SPACE: stopped");
    g.m.key_up(b' ');
    // A tap and a fire in quick succession: the tap still moves its column.
    let x = g.get("CX");
    g.until(30, "the volley has landed", |g| g.at("SACT", 0) == 0);
    g.press(b'D');
    g.press(b' ');
    g.frames(8);
    assert_eq!(g.get("CX"), x + 8);
    assert_eq!(g.at("SACT", 0), 1, "and the shot went");
}

#[test]
fn space_fires_a_shot_that_travels_up() {
    let mut g = Game::play();
    g.hold(b'A', 90); // clear of the formation
    g.frames(6);
    assert_eq!(g.get("CX"), 4);
    g.press(b' ');
    assert_eq!(g.at("SACT", 0), 1);
    assert_eq!(g.at("SY", 0), CY - 4 - 6, "fired from the barrel and moved");
    assert_eq!((g.get("SNDPRI"), g.speaker()), (2, 0b11), "a zap on the timer");
    let mut last = g.at("SY", 0);
    for _ in 0..5 {
        g.frame();
        assert_eq!(g.at("SY", 0), last - 6);
        last -= 6;
    }
    let px = g.pixels();
    assert_eq!(g.cell(&px, 8, last as usize, 4), vec![0x18; 4], "a 2-px shot over the barrel");
    assert_eq!(Game::lit_in(&px, 8, last as usize + 4, 8, 20), 0, "with no trail");
    g.until(30, "the shot leaves the top", |g| g.at("SACT", 0) == 0);
    g.frame();
    assert_eq!(Game::lit_in(&g.pixels(), 8, 0, 8, 150), 0, "nothing left behind");
    // One volley at a time: a second SPACE mid-flight waits for the first shot.
    g.press(b' ');
    g.frames(10);
    g.press(b' ');
    g.frame();
    let y = g.at("SY", 0);
    assert!(y < 100, "still the first shot ({y})");
}

#[test]
fn a_space_pressed_just_before_the_shot_lands_fires_when_it_does() {
    let mut g = Game::play();
    g.hold(b'A', 90);
    g.press(b' ');
    g.until(30, "the shot nears the top", |g| g.at("SY", 0) < 30);
    g.press(b' ');
    g.until(10, "the buffered shot", |g| g.at("SACT", 0) == 1 && g.at("SY", 0) > 100);
}

#[test]
fn a_shot_that_hits_a_bug_removes_it_and_scores() {
    let mut g = Game::play();
    // Line the cannon up under column 2 of the bottom row, allowing for the march.
    let x = g.bug_x(4, 2) + 2;
    g.set("CX", x);
    g.press(b' ');
    g.until(30, "a hit", |g| g.get("COUNT") == 39);
    assert_eq!(g.at("ALIVE", 4), !0x04, "column 2 of the bottom row");
    assert_eq!(g.at("SACT", 0), 0, "the shot is used up");
    assert_eq!(g.score(), 10);
    assert!(g.text_row(21).starts_with("SCORE 000010"));
    let px = g.pixels();
    let y = g.at("ROWY", 4) as usize;
    assert_eq!(g.lit(&px, 2 * g.bug_x(4, 2) as usize, y), 0, "the bug is gone");
    assert!(g.lit(&px, 2 * g.bug_x(4, 1) as usize, y) > 0, "its neighbours aren't");
    assert!(g.lit(&px, 2 * g.bug_x(4, 3) as usize, y) > 0);
    assert!(g.lit(&px, 2 * g.bug_x(3, 2) as usize, g.at("ROWY", 3) as usize) > 0, "nor the bug above");
    // Rows are worth 10, 20 and 30.
    g.shoot_at(2, 5);
    g.frames(2);
    assert_eq!(g.score(), 30);
    g.shoot_at(0, 5);
    g.frames(2);
    assert_eq!(g.score(), 60);
    assert_eq!(g.get("COUNT"), 37);
}

#[test]
fn bugs_drop_exceptions_from_the_bottom_of_a_column() {
    let mut g = Game::play();
    g.hold(b'A', 60); // out from under them
    g.set("NOBOMB", 0);
    g.set("BTIM", 1);
    g.frame();
    let i = (0..3).find(|&i| g.at("BACT", i) == 1).expect("an exception");
    let (bx, by) = (g.at("BX", i), g.at("BY", i));
    let col = (0..8).find(|&c| g.bug_x(4, c) == bx).expect("dropped from a bug's x");
    assert_eq!(by, g.at("ROWY", 4) + 8, "from under the bottom row (column {col})");
    g.frames(3);
    assert_eq!(g.at("BY", i), by + 6, "falling 2 px a frame");
    let px = g.pixels();
    assert_eq!(g.cell(&px, 2 * bx as usize, by as usize + 6, 6), g.obj("BOMB", 6), "an '!'");
    // Later waves drop them faster: three in the air at once, at most.
    g.set("WAVE", 9);
    let n = g.until(300, "three at once", |g| (0..3).all(|i| g.at("BACT", i) == 1));
    eprintln!("three exceptions in the air after {n} more frames");
}

#[test]
fn an_exception_hitting_the_cannon_costs_a_life() {
    let mut g = Game::play();
    let cx = g.get("CX");
    g.set_at("BACT", 0, 1);
    g.set_at("BX", 0, cx);
    g.set_at("BY", 0, CY - 14);
    g.until(8, "the hit", |g| g.get("STATE") == 2);
    assert_eq!(g.get("LIVES"), 2);
    assert_eq!(g.at("BACT", 0), 0);
    assert!(g.text_row(23).contains("UNCAUGHT EXCEPTION!"));
    assert!(g.text_row(21).contains("LIVES 2"));
    g.frame();
    assert_eq!(g.m.border_lines[100], 2, "the border flashes red");
    let fx = g.get("FXB");
    g.frames(40);
    assert_eq!(g.get("FXB"), fx, "everything waits while you recover");
    g.until(15, "back in play", |g| g.get("STATE") == 1);
    assert_eq!(g.cell(&g.pixels(), 2 * cx as usize, CY as usize, 8), g.obj("CANNON", 8));
    // A miss to either side is a miss.
    g.set_at("BACT", 0, 1);
    g.set_at("BX", 0, cx + 3);
    g.set_at("BY", 0, CY - 14);
    g.frames(20);
    assert_eq!((g.get("STATE"), g.get("LIVES")), (1, 2));
}

#[test]
fn losing_the_last_life_is_a_fatal_exception() {
    let mut g = Game::play();
    g.set("LIVES", 1);
    let cx = g.get("CX");
    g.set_at("BACT", 1, 1);
    g.set_at("BX", 1, cx);
    g.set_at("BY", 1, CY - 10);
    g.until(8, "game over", |g| g.get("STATE") == 3);
    g.frames(4); // drawing the message takes about 3 frames
    assert_eq!(g.get("REASON"), 2);
    assert!(g.text_row(23).starts_with("FATAL EXCEPTION"));
    let px = g.pixels();
    let f = unc_core::charset::CHAR_ROM[b'F' as usize * 8];
    let want = (0..16).fold(0u16, |w, i| w << 1 | (f as u16 >> (7 - i / 2) & 1));
    let got = (0..16).fold(0u16, |w, x| w << 1 | px[70 * 320 + 40 + x] as u16);
    assert_eq!(got, want, "a double-size 'F' at (40, 70)");
}

#[test]
fn catching_a_patch_gives_a_double_shot_for_a_while() {
    let mut g = Game::play();
    g.hold(b'A', 90);
    g.frames(6);
    let cx = g.get("CX");
    g.set("PACT", 1);
    g.set("PX", cx + 2);
    g.set("PY", 120);
    g.frames(2);
    assert_eq!(g.cell(&g.pixels(), 2 * cx as usize + 4, 122, 6), g.obj("PATCH", 6), "a '+'");
    g.until(40, "the catch", |g| g.get("DBL") > 0);
    assert_eq!(g.get("PACT"), 0);
    assert!(g.text_row(23).contains("PATCHED: DOUBLE SHOT"));
    g.press(b' ');
    assert_eq!((g.at("SACT", 0), g.at("SACT", 1)), (1, 1), "two shots");
    g.frame();
    let px = g.pixels();
    let y = g.at("SY", 0) as usize;
    assert_eq!(g.cell(&px, 2 * cx as usize, y, 4), vec![0xC3; 4], "one at each edge of the cannon");
    g.until(30, "both land", |g| g.at("SACT", 0) == 0 && g.at("SACT", 1) == 0);
    // It wears off: 200 ticks of 2 frames.
    g.set("DBL", 2);
    g.frames(4);
    assert_eq!(g.get("DBL"), 0);
    g.press(b' ');
    assert_eq!((g.at("SACT", 0), g.at("SACT", 1)), (1, 0), "single shots again");
    // A patch that falls past the cannon is lost.
    g.set("PACT", 1);
    g.set("PX", cx + 20);
    g.set("PY", 140);
    g.frames(25);
    assert_eq!((g.get("PACT"), g.get("DBL")), (0, 0));
}

#[test]
fn shot_bugs_sometimes_drop_a_patch() {
    // About 1 kill in 6 drops one, but the game's seed comes from the moment SPACE went
    // down, and about 1 seed in 900 gives no patch in a whole wave: try a few moments.
    for wait in 0..3 {
        let mut g = Game::play_after(wait);
        for k in 0..40u8 {
            let (row, col) = (4 - k as u16 / 8, k % 8);
            g.shoot_at(row, col);
            g.frame();
            assert_eq!(g.at("ALIVE", row) & 1 << col, 0, "bug {row},{col}");
            if g.get("PACT") == 1 {
                assert_eq!(g.get("PX"), g.bug_x(row, col), "the patch falls from the bug");
                assert_eq!(g.get("PY"), g.at("ROWY", row) + 9, "and has fallen 1 px");
                eprintln!("patch after {} kills", k + 1);
                return;
            }
        }
    }
    panic!("no patch in 40 kills, three games running");
}

#[test]
fn clearing_the_formation_starts_a_lower_faster_wave() {
    let mut g = Game::play();
    let fy = g.get("FY");
    let rate = g.get("RATE");
    for k in 0..40u8 {
        let (row, col) = (4 - k as u16 / 8, k % 8);
        g.kill(row, col);
    }
    g.frame();
    assert_eq!(g.score(), 160 + 320 + 240);
    assert_eq!(g.get("STATE"), 4);
    assert!(g.text_row(23).contains("ALL BUGS FIXED"));
    assert_eq!(Game::lit_in(&g.pixels(), 0, 0, 320, 150), 0, "every bug was rubbed out cleanly");
    g.until(70, "the next wave", |g| g.get("STATE") == 1);
    eprintln!("the frame that draws a new wave: {} cycles", g.worst(4, 1));
    assert!(g.worst(4, 1) < BUDGET);
    assert_eq!((g.get("WAVE"), g.get("COUNT")), (2, 40));
    assert!(g.text_row(21).ends_with("WAVE 02"));
    assert_eq!(g.get("FY"), fy + 6, "lower");
    g.frame();
    assert!(g.get("RATE") > rate, "faster");
    let px = g.pixels();
    let y = g.at("ROWY", 0) as usize;
    assert!(g.lit(&px, 2 * g.bug_x(0, 0) as usize, y) > 0, "a full formation again");
    assert_eq!(g.get("LATE"), 0);
}

#[test]
fn a_bug_reaching_the_cannon_row_crashes_the_program() {
    let mut g = Game::play();
    g.set("FY", 100); // the bottom row's feet at line 155: one line short
    g.frames(10);
    assert_eq!(g.get("STATE"), 1);
    assert_eq!(g.at("ROWY", 4), 148);
    g.set("FY", 104);
    g.until(10, "the crash", |g| g.get("STATE") == 3);
    g.frames(4);
    assert_eq!(g.get("REASON"), 1);
    assert!(g.text_row(23).starts_with("PROGRAM CRASHED"));
    let px = g.pixels();
    assert!(Game::lit_in(&px, 40, 70, 240, 16) > 300, "PROGRAM CRASHED, double size");
}

#[test]
fn space_restarts_after_game_over_and_keeps_the_high_score() {
    let mut g = Game::play();
    g.shoot_at(0, 3);
    g.frames(2);
    assert_eq!(g.score(), 30);
    g.set("FY", 104);
    g.until(10, "the crash", |g| g.get("STATE") == 3);
    assert!(g.text_row(21).contains("HI 000030"));
    g.press(b' ');
    g.frames(5);
    assert_eq!(g.get("STATE"), 3, "a held fire key doesn't restart at once");
    g.frames(50);
    g.press(b' ');
    g.until(5, "a new game", |g| g.get("STATE") == 1);
    assert_eq!((g.get("LIVES"), g.get("WAVE"), g.get("COUNT"), g.get("FY")), (3, 1, 40, 16));
    assert_eq!(g.score(), 0);
    assert_eq!(g.text_row(21), "SCORE 000000  HI 000030  LIVES 3 WAVE 01");
    assert_eq!(Game::lit_in(&g.pixels(), 0, 90, 320, 60), 0, "the game-over text is gone");
}

#[test]
fn every_frame_of_play_fits_in_one_50_hz_frame() {
    // Normal play: exceptions on, moving and firing.
    let mut g = Game::play();
    g.set("NOBOMB", 0);
    for i in 0..400 {
        match i % 40 {
            0 => g.m.key(b'D'),
            8 => g.m.key_up(b'D'),
            20 => g.m.key(b'A'),
            28 => g.m.key_up(b'A'),
            n if n % 7 == 3 => g.m.key(b' '),
            n if n % 7 == 4 => g.m.key_up(b' '),
            _ => {}
        }
        g.frame();
        if g.get("STATE") == 3 {
            break;
        }
    }
    let normal = g.worst(1, 1);
    // The worst case: wave 9 (two full-width row redraws every frame), sparse bugs,
    // a double shot, three exceptions and a patch in the air.
    let mut g = Game::play();
    g.set("WAVE", 9);
    for r in 0..5 {
        g.set_at("ALIVE", r, 0x81);
    }
    g.set("COUNT", 10);
    g.set("NOBOMB", 0);
    g.set("LIVES", 9);
    g.set("DBL", 250);
    g.m.key(b' '); // held: it keeps firing
    for i in 0..300 {
        if g.get("PACT") == 0 {
            g.set("PACT", 1);
            g.set("PX", 20 + i as u8 % 100);
            g.set("PY", 60);
        }
        if i % 50 == 49 {
            // An exception about to land on the cannon: the hit frame on top of the rest.
            let cx = g.get("CX");
            g.set_at("BACT", 2, 1);
            g.set_at("BX", 2, cx);
            g.set_at("BY", 2, CY - 6);
        }
        g.frame();
        if g.get("STATE") != 1 {
            g.frames(60);
        }
    }
    let (stress, hit) = (g.worst(1, 1), g.worst(1, 2));
    eprintln!("worst play frame: {normal} cycles normal, {stress} stressed, {hit} with a hit; of {BUDGET}");
    assert!(hit > 0, "the stress run included a hit");
    assert!(normal < BUDGET && stress < BUDGET && hit < BUDGET);
    assert_eq!(g.get("LATE"), 0, "no dropped frames");
    // The screen changes are the exception: clearing the bitmap and drawing big text
    // take a few frames while the picture stands still. Measured here.
    g.set("LIVES", 1);
    let cx = g.get("CX");
    g.set_at("BACT", 0, 1);
    g.set_at("BX", 0, cx);
    g.set_at("BY", 0, CY - 4);
    g.until(10, "game over", |g| g.get("STATE") == 3);
    g.frames(60);
    g.press(b' ');
    g.until(5, "a new game", |g| g.get("STATE") == 1);
    let (over, new) = (g.worst(1, 3), g.worst(3, 1));
    eprintln!("game over screen: {over} cycles; new game: {new} cycles");
    assert!(over < 4 * BUDGET && new < 3 * BUDGET);
}

#[test]
fn the_profiler_shows_cpu_time_in_the_border() {
    let mut g = Game::play();
    g.press(b'P');
    g.frames(2);
    assert_eq!(g.get("PROF"), 1);
    // The work starts as FRAME goes up, at VBLANK (line 251, manual chapter 9); that line
    // ends before the first STA BORDER, so the band starts on the next: the frame's work in
    // green, then the sound's step in yellow, under a line or two of it, round the bottom
    // of the frame and on from the top. Read from line 251.
    let bands = |g: &Game| {
        let v = unc_core::machine::VBLANK_LINE as usize;
        let b: Vec<u8> = g.m.border_lines[v..].iter().chain(&g.m.border_lines[..v]).copied().collect();
        let green = 1 + b[1..].iter().take_while(|&&c| c == 5).count();
        let yellow = b[green..].iter().take_while(|&&c| c == 7).count();
        assert!(green > 5 && green < 300, "a green band down to line {green}");
        assert!(b[green + yellow..].iter().all(|&c| c == 0), "then black");
        yellow
    };
    bands(&g);
    // The sequencer costs 20-90 cycles, a raster line is 64: some frames show a line of it.
    let lines: Vec<usize> = (0..200).map(|_| {
        g.frame();
        bands(&g)
    }).collect();
    let shown = lines.iter().filter(|&&n| n > 0).count();
    eprintln!("the sound's yellow line showed in {shown} of 200 frames");
    assert!(lines.iter().all(|&n| n <= 2) && shown > 0);
    g.press(b'P');
    g.frames(2);
    assert!(g.m.border_lines.iter().all(|&c| c == 0));
}

// ---------- the shifter ----------

impl Game {
    /// FNV-1a over the shown bitmap, lines 0-167.
    fn bitmap_hash(&self) -> u64 {
        (0x2000..0x2000 + 168 * 40u16).fold(0xcbf2_9ce4_8422_2325u64, |h, a| (h ^ self.m.peek(a) as u64).wrapping_mul(0x100_0000_01b3))
    }
}

/// A scripted game: the same keys on the same frames, with RND's seed (SEED, $020B) fixed
/// once it starts. Returns the game, and the bitmap's hash after every frame.
fn scripted_game() -> (Game, Vec<u64>) {
    let mut g = Game::play();
    g.m.poke(0x020B, 0x34);
    g.m.poke(0x020C, 0x12);
    let mut hs = vec![g.bitmap_hash()];
    for i in 0..260u32 {
        match i {
            0 => g.m.key(b'D'),
            19 => g.m.key_up(b'D'),
            20 => g.m.key(b' '),
            22 => g.m.key_up(b' '),
            30 => g.set("DBL", 250),
            40 => g.m.key(b'A'),
            45 => g.m.key(b' '),
            50 => g.m.key_up(b' '),
            61 => g.m.key_up(b'A'),
            70 => {
                for (k, x) in [(0u16, 40u8), (1, 61), (2, 82)] {
                    g.set_at("BACT", k, 1);
                    g.set_at("BX", k, x);
                    g.set_at("BY", k, 90 + 3 * k as u8);
                }
                g.set("PACT", 1);
                g.set("PX", 103);
                g.set("PY", 100);
            }
            80 => g.set("NOBOMB", 0),
            100 | 130 | 160 => {
                let row = (i / 30 - 3) as u16 + 2;
                g.shoot_at(row, 0);
            }
            110 => g.m.key(b' '),
            150 => g.m.key_up(b' '),
            170 => g.m.key(29),
            200 => g.m.key_up(29),
            _ => {}
        }
        g.frame();
        if g.get("STATE") == 2 {
            g.set("LIVES", 9);
        }
        hs.push(g.bitmap_hash());
    }
    (g, hs)
}

#[test]
fn sprites_drawn_through_the_shifter_look_exactly_as_the_pre_shifted_ones_did() {
    // The scripted game moves the formation through all four of its 2-px phases, the
    // cannon left and right, single and double shots, three exceptions and a patch, kills
    // three bugs and takes a hit (the cannon blinks). The hash of the bitmap after every
    // one of its 261 frames was recorded from the version with pre-shifted sprite tables
    // (commit deece71); drawn through the shifter, every frame is the same, byte for byte.
    let (g, hs) = scripted_game();
    assert_eq!((g.get("COUNT"), g.get("STATE"), g.score()), (34, 1, 100), "the game went as scripted");
    assert_eq!(hs[0], 0x29f5_7343_75d8_652d, "the first frame");
    assert_eq!(hs[hs.len() - 1], 0xdaf2_d589_5fe3_a7ee, "the last frame");
    let all = hs.iter().fold(0u64, |a, &h| a.rotate_left(5) ^ h);
    assert_eq!(all, 0xb60f_a6a9_efc7_7dc3, "every frame in between");
    let (sum, n) = g.play;
    eprintln!("scripted game: {n} frames of play, {} cycles a frame on average, {} at worst", sum / n, g.worst(1, 1));
}

#[test]
fn the_shifter_draws_a_sprite_at_every_pixel_phase() {
    // The patch at each of the 2-px positions in a byte, then a shot bar at every
    // pixel: the bytes in the bitmap are the sprite's row shifted right, across two bytes.
    let mut g = Game::play();
    g.hold(b'A', 90);
    for px in 40..44u8 {
        g.set("PACT", 1);
        g.set("PX", px);
        g.set("PY", 50 + 10 * (px - 40));
        g.frame();
        let y = g.get("PY") as u16;
        for (r, &row) in g.obj("PATCH", 6).iter().enumerate() {
            let a = 0x2000 + (y + r as u16) * 40 + px as u16 / 4;
            let w = (row as u16) << 8 >> (2 * (px & 3));
            assert_eq!((g.m.peek(a), g.m.peek(a + 1)), ((w >> 8) as u8, w as u8), "patch at x {}, row {r}", 2 * px);
        }
        g.set("PACT", 0);
        g.frames(2);
    }
    let cx = g.get("CX");
    for (k, (off, pixel)) in [(0u8, 0u16), (3, 3), (6, 6)].into_iter().enumerate() {
        g.set_at("SACT", 0, 1);
        g.set_at("SX", 0, cx);
        g.set_at("SY", 0, 120 - 10 * k as u8);
        g.set_at("SOFF", 0, off);
        g.frame();
        let x = 2 * cx as u16 + pixel;
        let y = g.at("SY", 0) as u16;
        let w = 0xC000u16 >> (x & 7);
        let a = 0x2000 + y * 40 + x / 8;
        assert_eq!((g.m.peek(a), g.m.peek(a + 1)), ((w >> 8) as u8, w as u8), "the bar at x {x}");
        g.set_at("SACT", 0, 0);
        g.frames(2);
    }
}

// ---------- sound ----------

#[test]
fn the_title_screen_is_silent() {
    let mut g = Game::boot();
    for _ in 0..100 {
        g.frame();
        assert!(g.audio().iter().all(|&s| s == 0.0), "not a sound");
    }
    assert_eq!(g.speaker(), 0, "the gate closed, the speaker data off");
    assert_eq!(g.get("SNDPRI"), 0);
    assert_eq!(g.m.mem.io.speaker_toggles, 0, "and no clicks by hand");
}

#[test]
fn a_shot_plays_on_the_timer_and_the_gate_closes_after_it() {
    let mut g = Game::play();
    g.hold(b'A', 60);
    g.until(60, "quiet", |g| g.get("SNDPRI") == 0);
    assert_eq!(g.speaker(), 0, "quiet means the gate is closed");
    g.set("BEAT", 1); // the march's next heartbeat note is three cycles away
    let (pri, steps) = g.effect("shot");
    assert_eq!(pri, 2);
    let len: u32 = steps.iter().map(|s| s.0 as u32).sum();
    assert!((4..=10).contains(&len), "a short zap: {len} frames");
    g.press(b' ');
    assert_eq!(g.at("SACT", 0), 1);
    // The first step is on the timer the frame the shot goes; the sequencer moves it on
    // once a frame, through every step, then closes the gate.
    for (k, &(frames, div)) in steps.iter().enumerate() {
        for _ in 0..frames {
            assert_eq!((g.divisor(), g.speaker()), (div, 0b11), "step {k}: the timer plays it");
            g.frame();
            assert!(g.audio().iter().any(|&s| s != 0.0), "step {k} is heard");
        }
    }
    assert_eq!(g.speaker(), 0, "after {len} frames the gate closes");
    assert_eq!((g.get("SNDT"), g.get("SNDPRI")), (0, 0));
    // The hosts' filter lets the speaker settle, then it's silent.
    let n = g.until_silent();
    assert_eq!(g.get("SNDPRI"), 0, "no other sound in the meantime ({n} frames)");
}

#[test]
fn sounds_take_turns_by_priority() {
    let mut g = Game::play();
    g.hold(b'A', 60);
    g.until(60, "quiet", |g| g.get("SNDPRI") == 0);
    g.set("BEAT", 1);
    g.press(b' ');
    assert_eq!(g.get("SNDPRI"), 2, "a shot");
    // A squash cuts it off. Its first step is noise: a new random divisor each frame.
    g.shoot_at(4, 3);
    g.frame();
    assert_eq!(g.get("COUNT"), 39);
    assert_eq!(g.get("SNDPRI"), 3, "a squash beats a shot");
    assert_ne!(g.get("SNDN"), 0, "a crunch of noise");
    let d = g.divisor();
    g.frame();
    assert_ne!(g.divisor(), d, "a new pitch every frame");
    // A shot doesn't cut off a squash.
    g.press(b' ');
    assert_eq!(g.at("SACT", 0), 1, "the shot goes");
    assert_eq!(g.get("SNDPRI"), 3, "but the squash plays on");
    // The rows sound different: the top row, worth 30, blips highest.
    let blip = |g: &Game, r: u16| g.effect(&format!("sq{r}")).1[1].1;
    assert!((0..4).all(|r| blip(&g, r) < blip(&g, r + 1)), "higher rows, higher blips");
    // An exception hitting you beats them all...
    let cx = g.get("CX");
    g.set_at("BACT", 0, 1);
    g.set_at("BX", 0, cx);
    g.set_at("BY", 0, CY - 8);
    g.until(8, "the hit", |g| g.get("STATE") == 2);
    assert_eq!(g.get("SNDPRI"), 4, "the hit");
    // ...and the march's heartbeat doesn't cut into it: the march waits anyway.
    let n = g.until(60, "the hit's sound ends", |g| g.get("SNDPRI") != 4);
    eprintln!("the hit sounds for {n} frames");
    assert_eq!(g.speaker(), 0, "then the gate closes");
}

#[test]
fn the_march_beats_four_falling_notes_faster_as_the_swarm_thins() {
    let notes = |g: &mut Game, frames: u32| {
        let mut starts = vec![];
        for f in 0..frames {
            g.frame();
            // A note started this frame: the heartbeat's priority, all 3 of its frames left.
            if g.get("SNDPRI") == 1 && g.get("SNDT") == 3 {
                starts.push((f, g.divisor()));
            }
        }
        starts
    };
    let mut g = Game::play();
    let full = notes(&mut g, 300);
    // Four notes, falling, round and round.
    let order: Vec<u16> = (0..4).map(|k| g.effect(&format!("beat{k}")).1[0].1).collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]), "falling: the divisors grow {order:?}");
    let i = order.iter().position(|&d| d == full[0].1).unwrap();
    for (k, &(_, d)) in full.iter().enumerate() {
        assert_eq!(d, order[(i + k) % 4], "note {k}");
    }
    // Thin the swarm to two bugs.
    for r in 0..4 {
        g.set_at("ALIVE", r, 0);
    }
    g.set_at("ALIVE", 4, 0x18);
    g.set("COUNT", 2);
    let thin = notes(&mut g, 100);
    let gap = |s: &[(u32, u16)]| (s[s.len() - 1].0 - s[0].0) as f64 / (s.len() - 1) as f64;
    let (slow, fast) = (gap(&full), gap(&thin));
    eprintln!("a heartbeat note every {slow:.1} frames for 40 bugs, every {fast:.1} for 2");
    assert!(slow > 20.0 && fast < slow / 4.0);
}

#[test]
fn every_event_has_its_sound_and_game_over_ends_in_silence() {
    let mut g = Game::play();
    let pri = |g: &Game, name: &str| g.effect(name).0;
    // Catch a patch.
    g.hold(b'A', 90);
    let cx = g.get("CX");
    g.set("PACT", 1);
    g.set("PX", cx);
    g.set("PY", 140);
    g.until(20, "the catch", |g| g.get("DBL") > 0);
    assert_eq!(g.get("SNDPRI"), pri(&g, "patch"));
    assert_eq!(g.divisor(), g.effect("patch").1[0].1, "the patch's chord");
    // Clear the wave.
    g.set("COUNT", 1);
    g.shoot_at(4, 4);
    g.until(3, "the wave is clear", |g| g.get("STATE") == 4);
    assert_eq!(g.divisor(), g.effect("clear").1[0].1, "the fanfare");
    g.until(70, "the next wave", |g| g.get("STATE") == 1);
    // Out of lives.
    g.set("LIVES", 1);
    let cx = g.get("CX");
    g.set_at("BACT", 0, 1);
    g.set_at("BX", 0, cx);
    g.set_at("BY", 0, CY - 8);
    g.until(8, "game over", |g| g.get("STATE") == 3);
    assert_eq!(g.get("SNDPRI"), pri(&g, "over"), "game over beats the hit");
    let (_, steps) = g.effect("over");
    let len: u32 = steps.iter().map(|s| s.0 as u32).sum();
    let n = g.until(len + 5, "the tune ends", |g| g.get("SNDPRI") == 0);
    eprintln!("game over: {len} frames of tune, ended {n} frames after the game did");
    assert_eq!(g.speaker(), 0, "the gate closed");
    g.until_silent();
}

// ---------- input, numbers, randomness ----------

#[test]
fn holding_left_and_fire_together_moves_and_fires() {
    for left in [b'A', io::KEY_LEFT] {
        let mut g = Game::play();
        let x0 = g.get("CX");
        // Both go down on the same frame: KBD and HELD show only SPACE, the one pressed
        // last. The matrix shows both.
        g.m.key(left);
        g.m.key(b' ');
        let mut fired = vec![];
        for f in 0..60u32 {
            g.frame();
            let x = g.get("CX");
            if f < 15 {
                assert_eq!(x, x0 - 1 - f as u8, "key {left}: moving every frame while firing");
            }
            if g.at("SACT", 0) == 1 && g.at("SY", 0) == CY - 4 - 6 {
                assert_eq!(g.at("SX", 0), x, "fired from where the cannon is now");
                fired.push(f);
            }
        }
        assert_eq!(g.get("CX"), 4, "it reached the left wall");
        assert!(fired.len() >= 2 && fired[0] == 0, "and kept firing: {fired:?}");
        g.m.key_up(b' ');
        g.m.key_up(left);
        let n = fired.len();
        g.frames(40);
        assert!(g.at("SACT", 0) == 0, "letting go stops it: {n} shots");
    }
}

#[test]
fn the_score_is_kept_in_digits_and_numstr_pads_the_lives_and_wave() {
    let mut g = Game::play();
    assert_eq!(g.text_row(21), "SCORE 000000  HI 000000  LIVES 3 WAVE 01");
    // 9,990 and 10 more: three carries. Only the digits that change are written, so
    // the score poked straight into the digits doesn't show until they do.
    for (i, d) in [0u8, 0, 9, 9, 9, 0].into_iter().enumerate() {
        g.set_at("SCORE", i as u16, d);
    }
    g.shoot_at(4, 3);
    g.frames(2);
    assert_eq!(g.score(), 10_000);
    assert!(g.text_row(21).starts_with("SCORE 010000  HI 000000"), "{}", g.text_row(21));
    // The wave, two digits wide with a zero in front, from NUMSTR and PRNUM.
    g.set("WAVE", 9);
    g.set("COUNT", 1);
    g.shoot_at(4, 4);
    g.until(3, "wave clear", |g| g.get("STATE") == 4);
    assert!(g.text_row(21).ends_with("LIVES 3 WAVE 10"), "{}", g.text_row(21));
    assert_eq!(g.m.mem.ram[0x03D0..0x03D8], *b"00000010", "NUMSTR's digits in NUMBUF");
    g.until(70, "the next wave", |g| g.get("STATE") == 1);
    // A hit: LIVES 2, one digit.
    let cx = g.get("CX");
    g.set_at("BACT", 0, 1);
    g.set_at("BX", 0, cx);
    g.set_at("BY", 0, CY - 8);
    g.until(8, "the hit", |g| g.get("STATE") == 2);
    assert_eq!(g.text_row(21), "SCORE 010010  HI 000000  LIVES 2 WAVE 10");
    assert!(g.text_row(22).trim().is_empty(), "nothing spilled onto the next row");
    // Game over: the high score's digits.
    g.until(60, "back in play", |g| g.get("STATE") == 1);
    g.set("LIVES", 1);
    g.set_at("BACT", 0, 1);
    g.set_at("BX", 0, cx);
    g.set_at("BY", 0, CY - 8);
    g.until(8, "game over", |g| g.get("STATE") == 3);
    assert_eq!(g.text_row(21), "SCORE 010010  HI 010010  LIVES 0 WAVE 10");
    // A wave past 99 still fits its two places.
    g.frames(60);
    g.press(b' ');
    g.until(5, "a new game", |g| g.get("STATE") == 1);
    assert_eq!(g.text_row(21), "SCORE 000000  HI 010010  LIVES 3 WAVE 01");
    g.set("WAVE", 99);
    g.set("COUNT", 1);
    g.shoot_at(4, 2);
    g.until(3, "wave clear", |g| g.get("STATE") == 4);
    assert!(g.text_row(21).ends_with("WAVE 99"), "{}", g.text_row(21));
    assert!(g.text_row(22).trim().is_empty());
}

#[test]
fn the_moment_space_goes_down_seeds_the_game() {
    // Where the first exceptions fall from: RND picks the column and the gaps.
    let game = |wait: u32| {
        let mut g = Game::play_after(wait);
        let seed = u16::from_le_bytes([g.m.peek(0x020B), g.m.peek(0x020C)]);
        g.set("NOBOMB", 0);
        g.set("LIVES", 99);
        let mut drops = vec![];
        let mut was = [0u8; 3];
        for _ in 0..600 {
            g.frame();
            for i in 0..3u16 {
                let now = g.at("BACT", i);
                if now == 1 && was[i as usize] == 0 {
                    drops.push(g.at("BX", i));
                }
                was[i as usize] = now;
            }
            if drops.len() >= 8 {
                break;
            }
        }
        assert!(drops.len() >= 8, "eight exceptions");
        (seed, drops)
    };
    let games: Vec<_> = (0..4).map(game).collect();
    for (w, (seed, drops)) in games.iter().enumerate() {
        eprintln!("SPACE after {w} frames: seed ${seed:04X}, exceptions from x {drops:?}");
    }
    let seeds: std::collections::HashSet<_> = games.iter().map(|g| g.0).collect();
    let patterns: std::collections::HashSet<_> = games.iter().map(|g| g.1.clone()).collect();
    assert_eq!(seeds.len(), 4, "four moments, four seeds");
    assert_eq!(patterns.len(), 4, "and four different games");
    assert_eq!(game(2), games[2], "the same moment replays the same game");
}

