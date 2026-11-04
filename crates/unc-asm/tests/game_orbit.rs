//! ORBIT (games/orbit.s), played through scripted keys on a booted 16K machine.
//! The game's state lives at the addresses documented at the top of the source.

use unc_asm::Assembly;
use unc_core::machine::{CYCLES_PER_FRAME, PIXELS_H, PIXELS_W, io};
use unc_core::{Machine, RamSize, Stop};

const SRC: &str = include_str!("../../../games/orbit.s");

// The object table (fast page): object 0 is the ship, 1-4 bullets, 5-16 rocks.
const OXL: u16 = 0x00;
const OXH: u16 = 0x11;
const OYL: u16 = 0x22;
const OYH: u16 = 0x33;
const OVXL: u16 = 0x44;
const OVXH: u16 = 0x55;
const OVYL: u16 = 0x66;
const OVYH: u16 = 0x77;
const SHIP: u16 = 0;
const BULLETS: std::ops::Range<u16> = 1..5;
const ROCKS: std::ops::Range<u16> = 5..17;
const KIND: u16 = 0x3A40;
// Game variables. The source lays them out with .res; `the_addresses_are_the_sources`
// checks these against its symbol table.
const STATE: u16 = 0x9C;
const TICK: u16 = 0x9F;
const SHIPDIR: u16 = 0xA0;
const THRUST: u16 = 0xA4;
const LIVES: u16 = 0xA7;
const WAVE: u16 = 0xA8;
/// Six digits, 0-9 each, the highest first.
const SCORE: u16 = 0xA9;
const INVUL: u16 = 0xAF;
const SHOTS: u16 = 0xB2;
const OVERRUN: u16 = 0xBD;
const NOCHK: u16 = 0xBE;
/// The sound effect playing, which is also its priority (0: none).
const SNDPRI: u16 = 0x3AA6;

// Keys (manual chapter 11): the cursor keys have codes 28-31.
const LEFT: u8 = 28;
const RIGHT: u8 = 29;
const UP: u8 = 30;
// Frames of burn from one tap of thrust (THRTAP in the source).
const THRTAP: i32 = 12;
// Frames between shots while SPACE is held (COOLT).
const COOLT: usize = 6;

// The sound effects, numbered by priority (FX_* in the source), and the shot's steps.
const FX_SHOT: u8 = 1;
const FX_BIG: u8 = 3;
const FX_WAVE: u8 = 4;
const FX_DIE: u8 = 5;
const SHOT_DIVISORS: [u16; 5] = [400, 550, 750, 1000, 1350];
// The engine's rumble: divisor high bytes $14-$23 (ENGINE and ENGJIT).
const ENGINE_HI: std::ops::RangeInclusive<u8> = 0x14..=0x23;

const TITLE: u8 = 0;
const PLAY: u8 = 1;
const DYING: u8 = 2;
const OVER: u8 = 3;
const BIG: u8 = 2;
const SMALL: u8 = 1;

#[derive(Clone)]
struct Game {
    m: Machine,
    work: u16,
    endwork: u16,
    started: Option<u64>,
    /// Most cycles one frame's work took (set-up frames excluded).
    worst: u64,
    /// Cycles of all timed frames' work, and how many there were: for the mean.
    total: u64,
    timed: u64,
    /// The bitmap at the end of the last set-up frame (title or new game): the scenery.
    scenery: Vec<u8>,
    /// The sound of the last whole frame, as the hosts play it (44.1 kHz).
    audio: Vec<f32>,
}

impl Game {
    /// Boots like the Machine Room: power on, let the ROM run, load, PC = start.
    fn boot() -> Game {
        let asm = assemble();
        assert!(asm.symbol("start").is_some(), "start");
        let mut m = unc_asm::boot(RamSize::K16, &asm);
        let (work, endwork) = (asm.symbol("work").unwrap(), asm.symbol("endwork").unwrap());
        m.set_breakpoint(work, true);
        m.set_breakpoint(endwork, true);
        let mut g =
            Game { m, work, endwork, started: None, worst: 0, total: 0, timed: 0, scenery: Vec::new(), audio: Vec::new() };
        // Building the tables and drawing the title takes a few frames: wait for the loop.
        let loaded = g.m.cpu.cycles;
        while g.peek(TICK) < 2 {
            assert!(g.m.cpu.cycles - loaded < 20 * CYCLES_PER_FRAME as u64, "no title after 20 frames");
            g.frame();
        }
        g
    }

    /// One frame of machine time, timing the game's work with breakpoints: to the next line
    /// 219, where the arena ends and ORBIT's work starts, so a frame's work is all inside one
    /// step.
    fn frame(&mut self) {
        let at = 219 * unc_core::machine::CYCLES_PER_LINE;
        let to = match (at + CYCLES_PER_FRAME - self.m.mem.frame_cycle) % CYCLES_PER_FRAME {
            0 => CYCLES_PER_FRAME,
            n => n,
        };
        let end = self.m.cpu.cycles + to as u64;
        while self.m.cpu.cycles < end {
            match self.m.run_cycles((end - self.m.cpu.cycles) as u32) {
                Stop::Budget => break,
                Stop::Breakpoint(pc) if pc == self.work => self.started = Some(self.m.cpu.cycles),
                Stop::Breakpoint(pc) if pc == self.endwork => {
                    if self.peek(NOCHK) != 0 {
                        self.scenery = self.bitmap();
                    } else if let Some(s) = self.started.take() {
                        let c = self.m.cpu.cycles - s;
                        self.worst = self.worst.max(c);
                        self.total += c;
                        self.timed += 1;
                    }
                }
                s => panic!("the game stopped: {s:?} at ${:04X}", self.m.cpu.pc),
            }
        }
        // The frame's sound, rendered every frame so the hosts' filters run as they do live.
        self.audio.resize(882, 0.0);
        self.m.audio_frame(44_100, &mut self.audio);
    }

    fn frames(&mut self, n: usize) {
        for _ in 0..n {
            self.frame();
        }
    }

    /// A tap: the key goes down for one frame (KBD and the matrix both see it), then up.
    fn key(&mut self, k: u8) {
        self.m.key(k);
        self.frame();
        self.m.key_up(k);
    }

    /// A key goes down and stays down (the matrix shows it) until `release`.
    fn hold(&mut self, k: u8) {
        self.m.key(k);
    }

    fn release(&mut self, k: u8) {
        self.m.key_up(k);
    }

    fn dir(&self) -> i32 {
        self.peek(SHIPDIR) as i32
    }

    /// Steps turned since direction `from`, clockwise, 0-31.
    fn turned(&self, from: i32) -> i32 {
        (self.dir() - from).rem_euclid(32)
    }

    fn peek(&self, a: u16) -> u8 {
        self.m.peek(a)
    }

    fn poke(&mut self, a: u16, v: u8) {
        self.m.poke(a, v)
    }

    fn start_game() -> Game {
        let mut g = Game::boot();
        g.key(b' ');
        g.settle();
        assert_eq!(g.peek(STATE), PLAY);
        g
    }

    /// Runs until a frame's work ends inside its frame: past any set-up (new game, title).
    fn settle(&mut self) {
        for _ in 0..10 {
            let t = self.peek(TICK);
            self.frame();
            if self.peek(TICK) == t.wrapping_add(1) && self.peek(NOCHK) == 0 {
                return;
            }
        }
        panic!("the game didn't settle");
    }

    fn pos(&self, o: u16) -> (u8, u8) {
        (self.peek(OXH + o), self.peek(OYH + o))
    }

    fn vel(&self, o: u16) -> (i32, i32) {
        let w = |l: u16, h: u16| (self.peek(l + o) as u16 | (self.peek(h + o) as u16) << 8) as i16 as i32;
        (w(OVXL, OVXH), w(OVYL, OVYH))
    }

    fn place(&mut self, o: u16, x: u8, y: u8, vx: i16, vy: i16) {
        for (a, v) in [(OXL, 0x80), (OXH, x), (OYL, 0x80), (OYH, y)] {
            self.poke(a + o, v);
        }
        for (l, h, v) in [(OVXL, OVXH, vx), (OVYL, OVYH, vy)] {
            self.poke(l + o, v as u8);
            self.poke(h + o, (v >> 8) as u8);
        }
    }

    fn kind(&self, o: u16) -> u8 {
        self.peek(KIND + o)
    }

    fn rocks(&self) -> Vec<u8> {
        ROCKS.map(|o| self.kind(o)).filter(|&k| k != 0).collect()
    }

    fn clear_rocks(&mut self) {
        for o in ROCKS {
            self.poke(KIND + o, 0);
        }
    }

    /// The score, from its six decimal digits.
    fn score(&self) -> u32 {
        (0..6).fold(0, |n, i| n * 10 + self.peek(SCORE + i) as u32)
    }

    /// The timer's divisor as last written, and whether the gate and data are both on
    /// (SPEAKER = %11: the timer drives the speaker).
    fn tone(&self) -> (u16, bool) {
        let div = self.peek(io::TIMER_LO) as u16 | (self.peek(io::TIMER_HI) as u16) << 8;
        (div, self.peek(io::SPEAKER) & 3 == 3)
    }

    /// The speaker is at rest: the gate closed, the data off.
    fn quiet(&self) -> bool {
        self.peek(io::SPEAKER) & 3 == 0
    }

    /// The last frame's sound is exact silence.
    fn silent(&self) -> bool {
        self.audio.iter().all(|&v| v == 0.0)
    }

    /// Runs until no sound effect is playing (the new wave's tune, say).
    fn hush(&mut self) {
        for _ in 0..100 {
            if self.peek(SNDPRI) == 0 {
                self.frame();
                return;
            }
            self.frame();
        }
        panic!("an effect that never ends");
    }

    fn text(&self) -> String {
        self.m.screen_text()
    }

    fn pixels(&self) -> Vec<u8> {
        let mut px = vec![0u8; PIXELS_W * PIXELS_H];
        self.m.render(true, &mut px);
        px
    }

    /// Lit pixels in a box around arena (x, y) (screen x = arena x + 32).
    fn ink_near(&self, x: u8, y: u8, r: i32) -> usize {
        let px = self.pixels();
        let (cx, cy) = (x as i32 + 32, y as i32);
        let mut n = 0;
        for yy in cy - r..=cy + r {
            for xx in cx - r..=cx + r {
                if (0..168).contains(&yy) && (32..288).contains(&xx) {
                    n += px[yy as usize * PIXELS_W + xx as usize] as usize;
                }
            }
        }
        n
    }

    /// Puts rock `o` of `kind` straight above the ship, both held still, and shoots it.
    /// Returns once the rock is hit (its kind changes).
    fn shoot_rock_above(&mut self, o: u16, kind: u8) {
        self.place(o, 64, 50, 0, 0);
        self.poke(KIND + o, kind);
        self.place(SHIP, 64, 84, 0, 0);
        self.poke(SHIPDIR, 24);
        self.key(b' ');
        for _ in 0..20 {
            if self.kind(o) != kind {
                return;
            }
            self.place(SHIP, 64, 84, 0, 0);
            self.place(o, 64, 50, 0, 0);
            self.frame();
        }
        panic!("the shot missed");
    }

    /// Text row `n` of the screen.
    fn row(&self, n: usize) -> String {
        self.text().lines().nth(n).unwrap().to_string()
    }

    /// The visible bitmap, 168 lines of 40 bytes.
    fn bitmap(&self) -> Vec<u8> {
        (0x2000..0x2000 + 168 * 40).map(|a| self.peek(a)).collect()
    }
}

fn assemble() -> Assembly {
    unc_asm::assemble(SRC).unwrap_or_else(|e| panic!("orbit.s doesn't assemble: {e:?}"))
}

/// The code fits below the bitmap; past it, at $2000, only tables, which start copies away
/// before the bitmap is cleared: the gravity table to the hidden lines, the rest (MOVLD to
/// MOVEND) to the text rows under the arena.
#[test]
fn fits_below_the_bitmap() {
    let asm = assemble();
    let gtab = asm.symbol("GTABLD").expect("GTABLD");
    assert_eq!(gtab, 0x2000);
    let (moved, end) = (asm.symbol("MOVLD").expect("MOVLD"), asm.symbol("MOVEND").expect("MOVEND"));
    assert_eq!(moved, gtab + 22 * 22);
    for s in asm.segments.iter().filter(|s| !s.bytes.is_empty()) {
        let end_s = s.start as usize + s.bytes.len();
        if s.start == gtab {
            assert_eq!(end_s, end as usize, "only the tables are loaded past the code");
        } else {
            assert!(s.start >= 0x0800 && end_s <= 0x2000, "segment ${:04X}-${end_s:04X} is outside $0800-$1FFF", s.start);
        }
    }
    assert!(asm.symbol("start").is_some());
}

#[test]
fn the_addresses_are_the_sources() {
    let asm = assemble();
    let names = [
        ("OXL", OXL), ("OXH", OXH), ("OYL", OYL), ("OYH", OYH), ("OVXL", OVXL), ("OVXH", OVXH), ("OVYL", OVYL),
        ("OVYH", OVYH), ("KIND", KIND), ("STATE", STATE), ("TICK", TICK), ("SHIPDIR", SHIPDIR), ("THRUST", THRUST),
        ("LIVES", LIVES), ("WAVE", WAVE), ("SCORE", SCORE), ("INVUL", INVUL), ("SHOTS", SHOTS), ("OVERRUN", OVERRUN),
        ("NOCHK", NOCHK), ("SNDPRI", SNDPRI),
    ];
    for (name, addr) in names {
        assert_eq!(asm.symbol(name), Some(addr), "{name}");
    }
}

#[test]
fn title_screen() {
    let g = Game::boot();
    assert_eq!(g.peek(STATE), TITLE);
    assert_eq!(g.peek(0xC000), 0x06, "bitmap, mixed mode");
    let text = g.text();
    let row23 = text.lines().nth(23).unwrap();
    assert!(row23.contains("ORBIT") && row23.contains("PRESS SPACE"), "{row23}");
    // The planet: a filled disc at the arena's centre (screen 160, 84).
    let px = g.pixels();
    assert_eq!(px[84 * PIXELS_W + 160], 1);
    assert_eq!(px[84 * PIXELS_W + 160 + 14], 0);
    // The logo's top bar runs along line 14; the O's left side at screen x 85.
    let lit = (85..300).filter(|&x| px[14 * PIXELS_W + x] == 1).count();
    assert!(lit > 40, "logo top edge: {lit} pixels");
    assert_eq!(px[27 * PIXELS_W + 85], 1);
    // Demo rocks orbit behind the title.
    assert_eq!(g.rocks(), vec![BIG; 3]);
}

#[test]
fn space_starts_a_game() {
    let g = Game::start_game();
    assert_eq!(g.peek(LIVES), 3);
    assert_eq!(g.peek(WAVE), 1);
    assert_eq!(g.score(), 0);
    assert_eq!(g.kind(SHIP), 1);
    assert_eq!(g.rocks(), vec![BIG; 3]);
    let text = g.text();
    let hud = text.lines().nth(22).unwrap();
    assert!(hud.contains("SCORE 000000") && hud.contains("WAVE 01") && hud.contains("SHIPS 3"), "{hud}");
    let (x, y) = g.pos(SHIP);
    assert!((128..136).contains(&x) && (43..46).contains(&y), "above the planet: ({x}, {y})");
    assert_eq!(g.peek(SHIPDIR), 24, "pointing up");
}

#[test]
fn a_and_d_rotate_the_ship() {
    let mut g = Game::start_game();
    g.key(b'A');
    assert_eq!(g.peek(SHIPDIR), 23);
    g.frames(10);
    g.key(b'D');
    assert_eq!(g.peek(SHIPDIR), 24);
    g.frames(10);
    g.key(b'D');
    assert_eq!(g.peek(SHIPDIR), 25);
    g.frames(10);
    assert_eq!(g.peek(SHIPDIR), 25, "a tap is exactly one step");
    // The drawn ship follows: the nose of direction 25 is up and to the right.
    g.poke(INVUL, 0);
    g.frame();
    let (x, y) = g.pos(SHIP);
    assert!(g.ink_near(x, y, 9) >= 15, "the ship is drawn");
}

#[test]
fn a_press_too_short_for_the_matrix_still_turns() {
    let mut g = Game::start_game();
    // Down and up again between two frames: the matrix never shows it, KBD does.
    g.m.key(b'A');
    g.m.key_up(b'A');
    g.frame();
    assert_eq!(g.peek(SHIPDIR), 23);
    g.frames(10);
    assert_eq!(g.peek(SHIPDIR), 23, "once");
}

#[test]
fn holding_a_turn_key_turns_smoothly_then_stops() {
    let mut g = Game::start_game();
    g.hold(b'D');
    g.frames(5);
    assert_eq!(g.turned(24), 1, "the first frame turns a step, then it waits: a quick tap is one step");
    // Held for 30 frames: a step on the first, then from the 8th a step every other frame.
    // The host's autorepeat presses on top (KBD) change nothing.
    for i in 5..30 {
        if i >= 25 && i % 2 == 1 {
            g.m.key(b'D');
        }
        g.frame();
    }
    assert_eq!(g.turned(24), 12, "held for 30 frames");
    // It stops as soon as the key comes up.
    g.release(b'D');
    let held = g.dir();
    g.frame();
    assert_eq!(g.dir(), held, "no step once the key is up");
    g.frames(30);
    assert_eq!(g.dir(), held);
    // And the other way.
    g.hold(b'A');
    g.frames(30);
    assert_eq!(g.turned(held), 32 - 12);
    g.release(b'A');
}

#[test]
fn the_cursor_keys_steer_too() {
    let mut g = Game::start_game();
    g.key(LEFT);
    assert_eq!(g.peek(SHIPDIR), 23);
    g.frames(10);
    g.key(RIGHT);
    g.frames(10);
    g.key(RIGHT);
    assert_eq!(g.peek(SHIPDIR), 25);
    g.hold(RIGHT);
    g.frames(30);
    assert_eq!(g.turned(25), 12, "held, it turns like D");
    g.release(RIGHT);
    // Up is thrust.
    g.clear_rocks();
    g.place(SHIP, 0, 84, 0, 0);
    g.poke(SHIPDIR, 24);
    g.frame();
    g.hold(UP);
    g.frames(20);
    g.release(UP);
    assert_eq!(g.vel(SHIP).1, -20 * 11);
}

#[test]
fn holding_thrust_burns_until_the_key_comes_up() {
    let mut g = Game::start_game();
    g.clear_rocks();
    // Far out on the x axis, pointing up: nothing pulls along y there, so vy is all thrust.
    g.place(SHIP, 0, 84, 0, 0);
    g.hold(b'W');
    g.frames(20);
    // The flame flickers (two frames on, two off) for as long as the key is held, well past
    // a tap's burst.
    let flame = assemble().symbol("FLAME").unwrap();
    let mut shown = 0;
    for _ in 0..20 {
        g.frame();
        shown += (g.peek(flame) != 0) as usize;
    }
    assert_eq!(shown, 10, "the flame shows in half the frames of a long burn");
    assert_eq!(g.vel(SHIP).1, -40 * 11, "40 frames held, 40 frames of thrust");
    g.release(b'W');
    g.frames(10);
    assert_eq!(g.vel(SHIP).1, -40 * 11, "and none once the key is up");
    assert_eq!(g.peek(THRUST), 0);
}

#[test]
fn space_over_a_held_turn_fires_and_keeps_turning() {
    let mut g = Game::start_game();
    g.clear_rocks();
    g.hold(b'D');
    g.frames(12);
    let before = g.dir();
    // SPACE goes down over D: the matrix shows both.
    g.hold(b' ');
    g.frame();
    assert_eq!(g.peek(SHOTS), 1, "it fires");
    g.frames(7);
    assert!(g.turned(before) >= 3, "and the turn carries on: {} steps", g.turned(before));
    // SPACE comes up, D stays down.
    g.release(b' ');
    let mid = g.dir();
    g.frames(8);
    assert!(g.turned(mid) >= 3, "still turning");
    g.release(b'D');
    g.frame();
    let end = g.dir();
    g.frames(20);
    assert_eq!(g.dir(), end);
}

#[test]
fn a_steering_key_let_go_over_a_held_space_stops() {
    let mut g = Game::start_game();
    g.clear_rocks();
    g.hold(b' ');
    g.frame();
    // D pressed over SPACE, held, let go while SPACE stays down. The turn must stop.
    g.hold(b'D');
    g.frames(12);
    assert!(g.turned(24) >= 3);
    g.release(b'D');
    g.frame();
    let end = g.dir();
    g.frames(20);
    assert_eq!(g.dir(), end, "no phantom turning under SPACE");
    // The same for thrust: a tap is its burst, and no more.
    g.key(b'W');
    assert!(g.peek(THRUST) > 0);
    let mut burned = 1;
    while g.peek(THRUST) > 0 {
        g.frame();
        burned += 1;
    }
    assert_eq!(burned, THRTAP, "a tap's burst");
    g.frames(20);
    assert_eq!(g.peek(THRUST), 0);
    g.release(b' ');
}

#[test]
fn w_thrust_changes_velocity() {
    let g = Game::start_game();
    let mut idle = g.clone();
    let mut burn = g;
    burn.key(b'W');
    idle.frame();
    burn.frames(5);
    idle.frames(5);
    let (bvx, bvy) = burn.vel(SHIP);
    let (ivx, ivy) = idle.vel(SHIP);
    // Pointing up: 11/256 px/frame^2 upwards for each of the 6 frames.
    assert_eq!(bvx, ivx);
    assert_eq!(ivy - bvy, 6 * 11, "thrust pushed the ship up");
    assert!(burn.peek(THRUST) > 0, "still burning: a tap is a burst");
}

#[test]
fn gravity_pulls_the_ship_toward_the_planet() {
    let mut g = Game::start_game();
    // The ship starts above the planet in a circular orbit: moving right, falling down.
    let (vx0, vy0) = g.vel(SHIP);
    assert!(vx0 > 200, "vx {vx0}");
    g.frames(10);
    let (_, vy) = g.vel(SHIP);
    assert!(vy >= vy0 + 40, "gravity turned the ship's path downward: vy {vy0} -> {vy}");
    // At rest, left of the planet, it falls right, towards it.
    g.place(SHIP, 60, 84, 0, 0);
    g.frames(20);
    let (vx, vy) = g.vel(SHIP);
    assert!(vx > 0 && vy == 0, "pulled toward the planet: ({vx}, {vy})");
    assert!(g.pos(SHIP).0 > 60);
}

#[test]
fn space_fires_a_bullet_that_moves_and_bends() {
    let mut g = Game::start_game();
    g.clear_rocks();
    // Hold the ship still, left of the planet, pointing up.
    g.place(SHIP, 64, 84, 0, 0);
    g.key(b' ');
    assert_eq!(g.peek(SHOTS), 1);
    let b = BULLETS.clone().find(|&o| g.kind(o) != 0).expect("a bullet");
    let (x0, y0) = g.pos(b);
    g.frames(3);
    let (xd, yd) = g.pos(b);
    g.frame();
    let (x1, y1) = g.pos(b);
    assert!(y1 + 8 <= y0, "the bullet flies up: {y0} -> {y1}");
    assert!(x1 >= x0);
    let (vx, _) = g.vel(b);
    assert!(vx > 0, "gravity bends it toward the planet (vx {vx})");
    // The picture is a frame behind the game: it's drawn where it was a frame ago.
    assert!(g.ink_near(xd, yd, 2) >= 3, "and it's drawn");
    // It runs out of range.
    g.frames(60);
    assert_eq!(g.kind(b), 0);
}

#[test]
fn shooting_a_big_rock_splits_it_and_a_small_one_dies() {
    let mut g = Game::start_game();
    g.clear_rocks();
    g.place(SHIP, 64, 84, 0, 0);
    g.frames(2);
    g.place(ROCKS.start, 64, 50, 0, 0);
    g.poke(KIND + ROCKS.start, BIG);
    g.place(SHIP, 64, 84, 0, 0);
    g.key(b' ');
    let mut n = 0;
    while g.rocks().len() < 2 && n < 20 {
        g.place(SHIP, 64, 84, 0, 0);
        g.frame();
        n += 1;
    }
    assert_eq!(g.rocks(), vec![SMALL; 2], "split into two small rocks");
    assert_eq!(g.score(), 20);
    assert!(g.text().lines().nth(22).unwrap().contains("SCORE 000020"));
    // The halves fly apart, across the bullet's path.
    let small: Vec<u16> = ROCKS.filter(|&o| g.kind(o) == SMALL).collect();
    let (va, vb) = (g.vel(small[0]), g.vel(small[1]));
    assert!(va.0 * vb.0 < 0, "kicked apart: {va:?} {vb:?}");
    // Now one small rock, straight above.
    g.poke(KIND + small[1], 0);
    g.place(small[0], 64, 50, 0, 0);
    g.frames(8);
    g.place(small[0], 64, 50, 0, 0);
    g.place(SHIP, 64, 84, 0, 0);
    g.key(b' ');
    let mut n = 0;
    while !g.rocks().is_empty() && n < 20 {
        g.place(SHIP, 64, 84, 0, 0);
        g.frame();
        n += 1;
    }
    assert!(g.rocks().is_empty(), "the small rock is gone");
    assert_eq!(g.score(), 70);
    assert!(g.text().lines().nth(22).unwrap().contains("SCORE 000070"));
}

#[test]
fn a_rock_costs_a_life_then_the_ship_respawns() {
    let mut g = Game::start_game();
    g.poke(INVUL, 0);
    g.place(ROCKS.start, 128, 44, 0, 0);
    g.frame();
    assert_eq!(g.peek(STATE), DYING);
    assert_eq!(g.peek(LIVES), 2);
    assert!(g.text().lines().nth(22).unwrap().contains("SHIPS 2"));
    g.frames(70);
    assert_eq!(g.peek(STATE), PLAY);
    assert_eq!(g.kind(SHIP), 1);
    assert!(g.peek(INVUL) > 0, "safe for a while after respawning");
}

#[test]
fn while_safe_rocks_pass_through() {
    let mut g = Game::start_game();
    assert!(g.peek(INVUL) > 0);
    g.place(ROCKS.start, 128, 44, 0, 0);
    g.frame();
    assert_eq!(g.peek(STATE), PLAY);
}

#[test]
fn the_planet_costs_a_life() {
    let mut g = Game::start_game();
    g.place(SHIP, 128, 74, 0, 0);
    g.frame();
    assert_eq!(g.peek(STATE), DYING);
    assert_eq!(g.peek(LIVES), 2);
}

#[test]
fn game_over_and_restart() {
    let mut g = Game::start_game();
    g.poke(LIVES, 1);
    g.place(SHIP, 128, 74, 0, 0);
    g.frames(70);
    assert_eq!(g.peek(STATE), OVER);
    assert!(g.text().lines().nth(23).unwrap().contains("GAME OVER"));
    // A held fire key mustn't restart it straight away.
    g.key(b' ');
    assert_eq!(g.peek(STATE), OVER);
    g.frames(60);
    g.key(b' ');
    g.settle();
    assert_eq!(g.peek(STATE), PLAY);
    assert_eq!(g.peek(LIVES), 3);
    assert_eq!(g.score(), 0);
    assert!(!g.text().contains("GAME OVER"));
}

#[test]
fn clearing_a_wave_brings_a_bigger_one() {
    let mut g = Game::start_game();
    g.clear_rocks();
    g.frames(60);
    assert_eq!(g.peek(WAVE), 2);
    assert_eq!(g.rocks(), vec![BIG; 4]);
    assert!(g.text().lines().nth(22).unwrap().contains("WAVE 02"));
}

#[test]
fn xor_drawing_leaves_no_trails() {
    let mut g = Game::start_game();
    let scenery = g.scenery.clone();
    // Play: turn, thrust and shoot for a while.
    for i in 0..300 {
        match i % 23 {
            0 | 7 => g.key(b' '),
            3 => g.key(b'D'),
            11 => g.key(b'W'),
            17 => g.key(b'A'),
            _ => g.frame(),
        }
    }
    assert_ne!(g.bitmap(), scenery);
    // Everything goes away; the redraw must leave the scenery exactly as it was.
    for o in 0..17 {
        g.poke(KIND + o, 0);
    }
    g.frames(8);
    let now = g.bitmap();
    let diff: Vec<(usize, usize, u8)> =
        (0..now.len()).filter(|&i| now[i] != scenery[i]).map(|i| (i % 40 * 8, i / 40, now[i] ^ scenery[i])).collect();
    assert!(diff.is_empty(), "{} bytes differ (x, line, bits): {:?}", diff.len(), &diff[..diff.len().min(12)]);
}

#[test]
fn every_frame_fits_in_a_50hz_frame() {
    let mut g = Game::start_game();
    // The heaviest load the game can make: every rock slot full and moving, with the
    // ship turning, burning and shooting.
    for (i, o) in ROCKS.enumerate() {
        let i = i as i16;
        g.place(o, (20 * i) as u8, (14 * i) as u8 % 168, 120 - 20 * i, 60 - 10 * i);
        g.poke(KIND + o, if i % 2 == 0 { BIG } else { SMALL });
    }
    for i in 0..2000 {
        match (i / 500, i % 4) {
            // Taps.
            (0, 0) => g.key(b' '),
            (0, 1) => g.key(b'D'),
            (0, 2) => g.key(b'W'),
            // A held turn, then held thrust, with SPACE tapped over them.
            (1, 0) => {
                g.hold(b'D');
                g.key(b' ');
            }
            (2, 0) => {
                g.release(b'D');
                g.hold(b'W');
                g.key(b' ');
            }
            // Turn, thrust and fire all held at once: the rumble, the shots and every
            // bullet in the air.
            (3, 0) if i == 1500 => {
                g.hold(b'D');
                g.hold(b' ');
                g.frame();
            }
            _ => g.frame(),
        }
        if g.peek(STATE) != PLAY {
            g.poke(STATE, PLAY);
            g.poke(KIND + SHIP, 1);
        }
        g.poke(INVUL, 50);
    }
    assert_eq!(g.peek(OVERRUN), 0, "a frame's work ran into the next frame");
    assert!(g.worst < CYCLES_PER_FRAME as u64, "worst frame {} cycles", g.worst);
    println!("worst frame: {} cycles of {}, mean {}", g.worst, CYCLES_PER_FRAME, g.total / g.timed);
}




// ---------- sound ----------

#[test]
fn a_shot_plays_on_the_timer_then_the_gate_closes() {
    let mut g = Game::start_game();
    g.hush(); // wave 1's tune
    assert!(g.quiet());
    g.clear_rocks();
    g.place(SHIP, 64, 84, 0, 0);
    g.key(b' ');
    assert_eq!(g.peek(SHOTS), 1);
    assert_eq!(g.peek(SNDPRI), FX_SHOT);
    // The sequencer runs at the start of each frame's work, so the shot plays from the
    // next frame: a step a frame, on the timer, with the gate open.
    for div in SHOT_DIVISORS {
        g.frame();
        assert_eq!(g.tone(), (div, true));
        assert!(g.m.mem.io.timer.gate);
        assert!(!g.silent(), "it can be heard");
    }
    g.frame();
    assert!(g.quiet(), "over after its five frames: the gate is closed");
    assert!(!g.m.mem.io.timer.gate);
    assert_eq!(g.peek(SNDPRI), 0);
    g.frames(25);
    assert!(g.silent(), "and the speaker has come to rest");
}

#[test]
fn silent_at_rest_on_the_title_and_in_play() {
    let mut g = Game::boot();
    // The demo rocks orbit, and some fall into the planet: all without a sound.
    for _ in 0..150 {
        g.frame();
        assert!(g.quiet() && g.silent(), "the title is silent");
    }
    g.key(b' ');
    g.settle();
    assert_eq!(g.peek(SNDPRI), FX_WAVE, "a tune for wave 1");
    g.hush();
    g.frames(25);
    for _ in 0..100 {
        g.poke(INVUL, 50);
        g.frame();
        assert!(g.quiet() && g.silent(), "no thrust, no shots: silence");
    }
    assert_eq!(g.peek(WAVE), 1, "and no new wave's tune in the meantime");
}

#[test]
fn the_engine_rumbles_while_thrusting_and_gives_way_to_a_shot() {
    let mut g = Game::start_game();
    g.hush();
    g.clear_rocks();
    g.place(SHIP, 0, 84, 0, 0);
    g.hold(b'W');
    g.frame();
    let rumble = |g: &Game| {
        let (div, on) = g.tone();
        on && ENGINE_HI.contains(&((div >> 8) as u8))
    };
    let mut divisors = std::collections::HashSet::new();
    for _ in 0..20 {
        g.frame();
        assert!(rumble(&g), "a low rumble: {:?}", g.tone());
        divisors.insert(g.tone().0);
    }
    assert!(divisors.len() > 10, "NOISE jitters the divisor: only {} different", divisors.len());
    // A shot takes the one voice; the rumble comes back after it.
    g.key(b' ');
    for div in SHOT_DIVISORS {
        g.frame();
        assert_eq!(g.tone(), (div, true));
    }
    g.frame();
    assert!(rumble(&g), "rumbling again: {:?}", g.tone());
    // Let go: the last frame's burn still sounds as the next frame starts, then silence.
    g.release(b'W');
    g.frames(2);
    assert!(g.quiet());
}

#[test]
fn a_breaking_rock_beats_a_shot_and_the_ship_beats_everything() {
    let mut g = Game::start_game();
    g.hush();
    g.clear_rocks();
    g.place(SHIP, 64, 84, 0, 0);
    g.frames(2);
    g.shoot_rock_above(ROCKS.start, BIG);
    assert_eq!(g.peek(SNDPRI), FX_BIG, "the rock breaks with a crunch");
    // Fire again while the crunch plays: the shot goes, the crunch keeps the voice.
    let shots = g.peek(SHOTS);
    g.place(SHIP, 64, 84, 0, 0);
    g.key(b' ');
    assert_eq!(g.peek(SHOTS), shots + 1);
    assert_eq!(g.peek(SNDPRI), FX_BIG);
    g.frame();
    let (div, on) = g.tone();
    assert!(on && div >= 0x0600, "still the crunch (noise from $0600 down), not the shot: {div}");
    // The ship blowing up beats everything, for longer than any other effect.
    g.poke(INVUL, 0);
    g.place(SHIP, 128, 74, 0, 0);
    g.frame();
    assert_eq!(g.peek(STATE), DYING);
    assert_eq!(g.peek(SNDPRI), FX_DIE);
    g.frames(35);
    assert_eq!(g.peek(SNDPRI), FX_DIE);
    assert!(g.tone().1);
    g.frames(10);
    assert!(g.quiet(), "and then silence");
}

// ---------- the keyboard matrix ----------

#[test]
fn turn_thrust_and_fire_all_at_once_with_either_keys() {
    let mut g = Game::start_game();
    g.clear_rocks();
    g.place(SHIP, 0, 84, 0, 0);
    let play = |g: &Game, keys: [u8; 3]| {
        let mut g = g.clone();
        for k in keys {
            g.hold(k);
        }
        g.frames(20);
        for k in keys {
            g.release(k);
        }
        g
    };
    let wasd = play(&g, *b"AW ");
    // Turned: a step on the first frame, then from the 8th every other frame: 7 steps left.
    assert_eq!(wasd.dir(), 24 - 7);
    // Fired whenever the gun was ready: frames 1, 7, 13 and 19.
    assert_eq!(wasd.peek(SHOTS) as usize, 20usize.div_ceil(COOLT));
    assert_eq!(BULLETS.filter(|&o| wasd.kind(o) != 0).count(), 4);
    // Thrust all the while, turning from up towards the left.
    let (vx, vy) = wasd.vel(SHIP);
    assert!(vy < -150 && vx < -20, "pushed up and left: ({vx}, {vy})");
    // The cursor keys, or a mix, do exactly the same.
    let state = |g: &Game| (g.dir(), g.vel(SHIP), g.pos(SHIP), g.peek(SHOTS));
    assert_eq!(state(&play(&g, [LEFT, UP, b' '])), state(&wasd));
    assert_eq!(state(&play(&g, [b'A', UP, b' '])), state(&wasd));
    assert_eq!(state(&play(&g, [LEFT, b'W', b' '])), state(&wasd));
    // Right and thrust, the other way round.
    let right = play(&g, [RIGHT, b'W', b' ']);
    assert_eq!(right.dir(), 24 + 7);
    assert!(right.vel(SHIP).0 > 20);
    // Both ways at once cancel out.
    let both = play(&g, *b"AD ");
    assert_eq!(both.dir(), 24);
    assert_eq!(both.peek(SHOTS), wasd.peek(SHOTS));
}

// ---------- numbers ----------

#[test]
fn the_score_is_a_row_of_digits_that_carries_and_rolls_over() {
    let mut g = Game::start_game();
    g.clear_rocks();
    g.place(SHIP, 64, 84, 0, 0);
    g.frames(2);
    // 999990, on the board and on the screen.
    for (i, d) in [9, 9, 9, 9, 9, 0].into_iter().enumerate() {
        g.poke(SCORE + i as u16, d);
        g.poke(0x0400 + 22 * 40 + 7 + i as u16, b'0' + d);
    }
    // A big rock: 20 points carry through every digit and roll over.
    g.shoot_rock_above(ROCKS.start, BIG);
    assert_eq!(g.score(), 10);
    assert!(g.row(22).contains("SCORE 000010"), "{}", g.row(22));
    // A small one: 50 more.
    for o in ROCKS {
        g.poke(KIND + o, 0);
    }
    g.frames(8);
    g.shoot_rock_above(ROCKS.start, SMALL);
    assert_eq!(g.score(), 60);
    assert!(g.row(22).contains("SCORE 000060"), "{}", g.row(22));
}

#[test]
fn wave_and_ships_are_printed_at_fixed_widths() {
    let mut g = Game::start_game();
    assert!(g.row(22).contains("WAVE 01") && g.row(22).ends_with("SHIPS 3"), "{}", g.row(22));
    g.poke(WAVE, 9);
    g.clear_rocks();
    g.frames(60);
    assert_eq!(g.peek(WAVE), 10);
    assert!(g.row(22).contains("WAVE 10"), "{}", g.row(22));
    g.poke(INVUL, 0);
    g.place(SHIP, 128, 74, 0, 0);
    g.frame();
    assert!(g.row(22).ends_with("SHIPS 2"), "{}", g.row(22));
    // Printing leaves no cursor: the rest of the row is as the HUD drew it.
    assert_eq!(g.row(22), " SCORE 000000     WAVE 10      SHIPS 2");
}

// ---------- the random seed ----------

#[test]
fn the_moment_space_is_pressed_seeds_the_rocks() {
    let layout = |wait: usize| {
        let mut g = Game::boot();
        g.frames(wait);
        g.key(b' ');
        g.settle();
        let rocks: Vec<_> = ROCKS.filter(|&o| g.kind(o) != 0).map(|o| (g.pos(o), g.vel(o))).collect();
        assert_eq!(rocks.len(), 3);
        rocks
    };
    // The title doesn't step RND from frame to frame: only NOISE, read as the game starts,
    // tells one frame's start from the next.
    let layouts: Vec<_> = (0..6).map(layout).collect();
    for i in 0..layouts.len() {
        for j in i + 1..layouts.len() {
            assert_ne!(layouts[i], layouts[j], "SPACE after {i} and {j} frames gave the same rocks");
        }
    }
    assert_eq!(layout(3), layouts[3], "the same moment, the same game: a replay is exact");
}

// ---------- the shifter ----------

#[test]
fn bullets_are_2x2_dots_at_any_x() {
    let mut g = Game::start_game();
    g.clear_rocks();
    g.poke(KIND + SHIP, 0); // no ship: only the bullet over the scenery
    g.frames(2);
    let scenery = g.scenery.clone();
    assert_eq!(g.bitmap(), scenery);
    let b = BULLETS.start;
    // Every x AND 7, and the right edge, where the dot spills over to the left edge.
    for x in (40u8..48).chain([255]) {
        g.place(b, x, 30, 0, 0);
        g.poke(KIND + b, 40);
        g.frame();
        let (dx, dy) = (g.peek(KIND + 34 + b) as usize, g.peek(KIND + 51 + b) as usize); // DXP, DYP
        let now = g.bitmap();
        let mut lit: Vec<(usize, usize)> = (0..now.len() * 8)
            .filter(|&p| (now[p / 8] ^ scenery[p / 8]) & (0x80 >> (p % 8)) != 0)
            .map(|p| ((p / 8) % 40 * 8 + p % 8, p / 320))
            .collect();
        lit.sort();
        let mut want: Vec<(usize, usize)> =
            [(0, 0), (1, 0), (0, 1), (1, 1)].iter().map(|&(ox, oy)| ((dx + ox) % 256 + 32, dy + oy)).collect();
        want.sort();
        assert_eq!(lit, want, "the dot at x {dx}");
        g.poke(KIND + b, 0);
        g.frame();
        assert_eq!(g.bitmap(), scenery, "and rubbed out");
    }
}

// ---------- the generated tables ----------

/// R * sin(i / 32 of a turn), as the old source worked it out without sin(): Bhaskara's
/// approximation, rounded half away from zero.
fn bhaskara(i: i64, r: i64) -> i64 {
    let (k, j) = (i / 16, i % 16);
    let p = j * (16 - j);
    (1 - 2 * (k & 1)) * ((8 * r * p + 320 - p) / (2 * (320 - p)))
}

/// R * sin(i / 32 of a turn), rounded half away from zero.
fn sine(i: i64, r: i64) -> i64 {
    (r as f64 * (i as f64 * std::f64::consts::TAU / 32.0).sin()).round() as i64
}

#[test]
fn the_generated_tables_match_the_old_ones() {
    let asm = assemble();
    let g = Game::boot();
    let at = |name: &str, i: usize| g.peek(asm.symbol(name).unwrap_or_else(|| panic!("{name}")) + i as u16);
    // Thrust, bullet speed and the spawn ring: the true sine now, which the old
    // approximation matched, or missed by one.
    for i in 0..40 {
        let thr = at("THR", i) as i8 as i64;
        let bv = (at("BVL", i) as u16 | (at("BVH", i) as u16) << 8) as i16 as i64;
        let ringx = at("RINGX", i) as i8 as i64;
        let ii = i as i64;
        assert_eq!((thr, bv, ringx), (sine(ii, 11), sine(ii, 640), sine(ii, 64)), "direction {i}");
        assert_eq!(thr, bhaskara(ii, 11));
        assert!((bv - bhaskara(ii, 640)).abs() <= 1 && (ringx - bhaskara(ii, 64)).abs() <= 1);
        if i < 32 {
            let ringy = at("RINGY", i) as i8 as i64;
            assert_eq!(ringy, sine(ii, 61));
            assert_eq!(ringy, bhaskara(ii, 61));
        }
    }
    // The ship's vertices, as the old SHIPB and its quarter turns had them.
    const SHIPB: [[i8; 16]; 5] = [
        [8, 7, 7, 6, 5, 4, 3, 1, 0, 1, 3, 4, 5, 6, 7, 7],
        [-5, -5, -6, -6, -6, -6, -5, -5, 4, 3, 2, 1, -1, -2, -3, -4],
        [-2, -2, -2, -2, -2, -1, -1, 0, 0, 0, -1, -1, -2, -2, -2, -2],
        [-5, -4, -3, -2, -1, 1, 2, 3, -4, -5, -5, -6, -6, -6, -6, -5],
        [-7, -6, -6, -5, -5, -4, -2, -1, 0, -1, -2, -4, -5, -5, -6, -6],
    ];
    for (v, base) in SHIPB.iter().enumerate() {
        for d in 0..32 {
            let (mut x, mut y) = (base[d % 8], base[8 + d % 8]);
            for _ in 0..d / 8 {
                (x, y) = (-y, x);
            }
            let got = (at(&format!("SVX{v}"), d) as i8, at(&format!("SVY{v}"), d) as i8);
            assert_eq!(got, (x, y), "vertex {v}, direction {d}");
        }
    }
    // Gravity: the inverse square, 8000 / r^2 * (4a + 1.5) / r with r at least 9.
    for a in 0..22 {
        for b in 0..22 {
            let (u, w) = (4.0 * a as f64 + 1.5, 4.0 * b as f64 + 1.5);
            let r = u.hypot(w).max(9.0);
            let want = (8000.0 / (r * r) * u / r).round() as u8;
            assert_eq!(at("GTAB", 22 * a + b), want, "GTAB[{a}][{b}]");
        }
    }
    // Spot checks against the old pasted rows.
    assert_eq!((0..8).map(|b| at("GTAB", b)).collect::<Vec<_>>(), [16, 16, 13, 5, 2, 1, 1, 0]);
    assert_eq!((0..8).map(|b| at("GTAB", 22 * 2 + b)).collect::<Vec<_>>(), [85, 57, 31, 17, 10, 6, 4, 3]);
}
