//! AURORA (games/board/aurora.s): the video board's demonstration cassette, a horizontally
//! scrolling shooter. Played through scripted keys on a booted 16K machine with the board;
//! its state is read from the addresses the source's symbol table gives.

use unc_core::machine::{CYCLES_PER_FRAME, PIXELS_H, PIXELS_W, io};
use unc_core::{Machine, RamSize, Stop};

const SRC: &str = include_str!("../../../games/board/aurora.s");
const BUDGET: u64 = CYCLES_PER_FRAME as u64;

const TITLE: u8 = 1;
const PLAY: u8 = 2;
const OVER: u8 = 3;

const LEFT: u8 = 28;
const RIGHT: u8 = 29;
const UP: u8 = 30;
const DOWN: u8 = 31;

const TABLE: u16 = 0x3700;
const SCREEN: u16 = 0x0400;
const LAYER: u16 = 0x3C00;

struct Game {
    m: Machine,
    asm: unc_asm::Assembly,
    /// The most cycles one frame's work took (`work` to `endwork`), set-up frames left out.
    worst: u64,
    /// All timed frames' cycles, and how many: for the mean.
    total: u64,
    timed: u64,
    begun: Option<u64>,
}

/// Cycles from now to the start of the next VBLANK line (a whole frame if it's now).
fn to_vblank(m: &Machine) -> u32 {
    let at = unc_core::machine::VBLANK_LINE * unc_core::machine::CYCLES_PER_LINE;
    match (at + CYCLES_PER_FRAME - m.mem.frame_cycle) % CYCLES_PER_FRAME {
        0 => CYCLES_PER_FRAME,
        n => n,
    }
}

fn assemble() -> unc_asm::Assembly {
    unc_asm::assemble(SRC).unwrap_or_else(|e| panic!("aurora.s doesn't assemble: {e:?}"))
}

impl Game {
    /// A UNC 16 with the video board, booted to READY., with AURORA loaded and started
    /// as the hosts start a program.
    fn boot() -> Game {
        Game::boot_board(true)
    }

    fn boot_board(board: bool) -> Game {
        let asm = assemble();
        let mut m = Machine::boot_with_board(RamSize::K16, board);
        unc_asm::load(&mut m, &asm);
        let mut g = Game {
            m,
            asm,
            worst: 0,
            total: 0,
            timed: 0,
            begun: None,
        };
        g.frames(4);
        g
    }

    /// Boots and presses SPACE on the title.
    fn play() -> Game {
        let mut g = Game::boot();
        g.frames(10);
        g.press(b' ');
        g.until(5, "SPACE starts a game", |g| g.get("STATE") == PLAY);
        g.frames(2);
        g
    }

    fn sym(&self, name: &str) -> u16 {
        self.asm
            .symbol(name)
            .unwrap_or_else(|| panic!("no symbol {name}"))
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

    /// One frame of machine time, timing the game's work between `work` and `endwork`: to the
    /// next VBLANK, where FRAME goes up (manual chapter 9), so a frame's work, which starts
    /// there, is all inside one step.
    fn frame(&mut self) {
        let (w, e) = (self.sym("work"), self.sym("endwork"));
        self.m.set_breakpoint(w, true);
        self.m.set_breakpoint(e, true);
        let end = self.m.cpu.cycles + to_vblank(&self.m) as u64;
        while self.m.cpu.cycles < end {
            match self.m.run_cycles((end - self.m.cpu.cycles) as u32) {
                Stop::Budget => break,
                Stop::Breakpoint(pc) if pc == w => self.begun = Some(self.m.cpu.cycles),
                Stop::Breakpoint(pc) if pc == e => {
                    if let Some(t) = self.begun.take()
                        && self.get("NOCHK") == 0
                    {
                        let c = self.m.cpu.cycles - t;
                        self.worst = self.worst.max(c);
                        self.total += c;
                        self.timed += 1;
                    }
                }
                s => panic!("the machine stopped: {s:?} at ${:04X}", self.m.cpu.pc),
            }
        }
        self.m.clear_breakpoints();
    }

    fn frames(&mut self, n: u32) {
        for _ in 0..n {
            self.frame();
        }
    }

    /// A tap: down for one frame, then up.
    fn press(&mut self, k: u8) {
        self.m.key(k);
        self.frame();
        self.m.key_up(k);
    }

    fn hold(&mut self, k: u8, frames: u32) {
        self.m.key(k);
        self.frames(frames);
        self.m.key_up(k);
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

    /// The picture as the board draws it: a colour number (0-3) a pixel.
    fn pixels(&self) -> Vec<u8> {
        let mut px = vec![0u8; PIXELS_W * PIXELS_H];
        self.m.render(true, &mut px);
        px
    }

    /// Sprite table entry n: (y, x, image, attributes).
    fn entry(&self, n: u16) -> (u8, u16, u8, u8) {
        let b = |i: u16| self.m.peek(TABLE + n * 4 + i);
        (b(0), b(1) as u16 | ((b(3) as u16 & 1) << 8), b(2), b(3))
    }

    fn shown(&self) -> Vec<u16> {
        (0..32).filter(|&n| self.entry(n).3 & 0x80 != 0).collect()
    }

    fn score(&self) -> u32 {
        (0..6).fold(0, |s, i| s * 10 + self.at("SCORE", i) as u32)
    }

    fn count(&self, name: &str, n: u16) -> usize {
        (0..n).filter(|&i| self.at(name, i) != 0).count()
    }

    fn text(&self) -> String {
        self.m.screen_text()
    }

    fn layer_text(&self) -> String {
        (0..25)
            .map(|r| {
                (0..40)
                    .map(|c| match self.m.peek(LAYER + r * 40 + c) {
                        b @ 0x20..=0x5F => b as char,
                        _ => '?',
                    })
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn screen_row(&self, r: u16) -> Vec<u8> {
        (0..40).map(|c| self.m.peek(SCREEN + r * 40 + c)).collect()
    }

    /// Writes the picture, in the colour monitor's colours, as a PNG.
    fn png(&self, path: &str, scale: usize) {
        const PALETTE: [u32; 16] = [
            0x000000, 0xe8e4da, 0x9a3b2e, 0x5fb3a8, 0x7a3f8a, 0x4f9a4f, 0x2c3d9a, 0xe2c84a,
            0xa8632a, 0x5a3e1a, 0xd0705f, 0x3a3a3a, 0x6b6b6b, 0x9fe08a, 0x6f8fd8, 0xa9a9a9,
        ];
        let px = self.pixels();
        let (w, h) = (PIXELS_W * scale, PIXELS_H * scale);
        let mut raw = Vec::with_capacity((w * 3 + 1) * h);
        for y in 0..h {
            raw.push(0);
            for x in 0..w {
                let c =
                    PALETTE[self.m.colour_of(px[(y / scale) * PIXELS_W + x / scale]) as usize & 15];
                raw.extend_from_slice(&[(c >> 16) as u8, (c >> 8) as u8, c as u8]);
            }
        }
        std::fs::write(path, png::encode(w as u32, h as u32, &raw)).unwrap();
    }
}

/// A PNG without a compressor: stored deflate blocks, enough for test pictures.
mod png {
    fn crc(data: &[u8]) -> u32 {
        let mut c = !0u32;
        for &b in data {
            c ^= b as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    0xEDB8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
        }
        !c
    }
    fn chunk(out: &mut Vec<u8>, kind: &[u8], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc(&body).to_be_bytes());
    }
    pub fn encode(w: u32, h: u32, raw: &[u8]) -> Vec<u8> {
        let mut z = vec![0x78, 0x01];
        for (i, block) in raw.chunks(65535).enumerate() {
            let last = (i + 1) * 65535 >= raw.len();
            z.push(last as u8);
            z.extend_from_slice(&(block.len() as u16).to_le_bytes());
            z.extend_from_slice(&(!(block.len() as u16)).to_le_bytes());
            z.extend_from_slice(block);
        }
        let (mut a, mut b) = (1u32, 0u32);
        for &x in raw {
            a = (a + x as u32) % 65521;
            b = (b + a) % 65521;
        }
        z.extend_from_slice(&((b << 16) | a).to_be_bytes());
        let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut ihdr = w.to_be_bytes().to_vec();
        ihdr.extend_from_slice(&h.to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
        chunk(&mut out, b"IHDR", &ihdr);
        chunk(&mut out, b"IDAT", &z);
        chunk(&mut out, b"IEND", &[]);
        out
    }
}

/// A deterministic dice for the autoplay: xorshift32.
struct Dice(u32);
impl Dice {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
}

// ---------- loading ----------

#[test]
fn it_fits_a_unc_16_and_keeps_clear_of_the_boards_memory() {
    let asm = assemble();
    let (lo, hi) = asm
        .segments
        .iter()
        .filter(|s| !s.bytes.is_empty())
        .fold((u16::MAX, 0u16), |(lo, hi), s| {
            (lo.min(s.start), hi.max(s.start + s.bytes.len() as u16 - 1))
        });
    assert_eq!(lo, 0x0800, "it loads at $0800 like the others");
    let g = Game::boot();
    let dyn_end = g.sym("PROGEND");
    eprintln!(
        "program ${lo:04X}-${hi:04X} ({} bytes on tape), images to ${dyn_end:04X}",
        hi - lo + 1
    );
    assert!(
        dyn_end <= g.sym("RAM") && g.sym("RAMEND") <= TABLE,
        "the variables stop short of the table"
    );
    assert_eq!(g.m.peek(io::SPRTAB), 0x37);
    assert_eq!(
        g.m.peek(io::SPRBANK),
        (g.sym("IMAGES") >> 8) as u8,
        "the images are in the program"
    );
    assert_eq!(g.sym("IMAGES") & 0xFF, 0);
    assert_eq!(
        g.m.peek(io::CHARSET),
        0x38,
        "the character set is where the board's images would be"
    );
    assert_eq!(g.m.peek(io::LAYERPAGE), 0x3C);
}

#[test]
fn it_loads_from_tape_on_a_machine_with_the_board() {
    let asm = assemble();
    let tape = unc_asm::tape::make(&asm, "AURORA");
    let mut m = Machine::boot_with_board(RamSize::K16, true);
    assert!(m.screen_text().contains("VIDEO BOARD REV A"));
    m.insert_tape(tape);
    for &k in b"LOAD \"AURORA\"\r" {
        m.key(k);
        for _ in 0..200 {
            m.run_cycles(500);
            if m.peek(io::KBD) & 0x80 == 0 {
                break;
            }
        }
    }
    m.run_frame();
    m.press_play();
    let start = asm.symbol("start").unwrap();
    m.run_until(start, 40_000_000)
        .unwrap_or_else(|e| panic!("never started: {e:?}\n{}", m.screen_text()));
    for (i, s) in asm.segments.iter().enumerate() {
        assert_eq!(
            &m.mem.ram[s.start as usize..][..s.bytes.len()],
            &s.bytes[..],
            "segment {i}"
        );
    }
    for _ in 0..30 {
        assert!(!matches!(m.run_frame(), Stop::Jammed(_)));
    }
    let title = asm.symbol("STATE").map(|a| m.peek(a));
    assert_eq!(title, Some(TITLE));
}

#[test]
fn without_the_board_it_says_so_and_waits() {
    let mut g = Game::boot_board(false);
    g.frames(50);
    let text = g.text();
    assert!(text.contains("AURORA NEEDS THE VIDEO BOARD."), "{text}");
    assert!(text.contains("SWITCH OFF, FIT THE BOARD,"));
    assert_eq!(
        g.m.cpu.state,
        unc_core::CpuState::Running,
        "waiting, not halted or jammed"
    );
    // a key does nothing
    g.press(b' ');
    g.frames(10);
    assert!(g.text().contains("AURORA NEEDS THE VIDEO BOARD."));
}

// ---------- the title ----------

#[test]
fn the_title_shows_the_name_in_wide_sprites_and_waits_for_space() {
    let mut g = Game::boot();
    g.frames(20);
    assert_eq!(g.get("STATE"), TITLE);
    let layer = g.layer_text();
    assert!(
        layer.contains("A DEMONSTRATION OF THE VIDEO BOARD"),
        "{layer}"
    );
    assert!(layer.contains("HI 000000"));
    assert!(layer.contains("(C) 1984 UNITED NORTHERN COMPUTERS"));
    assert_eq!(g.m.peek(io::LAYER) & 1, 1, "the layer is on");
    // AURORA: six wide sprites, entries 24-29, A U R O R A
    let imgs: Vec<u8> = (24..30).map(|n| g.entry(n).2).collect();
    assert_eq!(imgs[0], imgs[5]);
    assert_eq!(imgs[2], imgs[4]);
    for n in 24..30 {
        assert_eq!(g.entry(n).3 & 0x82, 0x82, "entry {n} wide and shown");
    }
    // they bob
    let y0: Vec<u8> = (24..30).map(|n| g.entry(n).0).collect();
    g.frames(16);
    let y1: Vec<u8> = (24..30).map(|n| g.entry(n).0).collect();
    assert_ne!(y0, y1);
    // PRESS SPACE blinks, in the layer
    let mut seen = (false, false);
    for _ in 0..80 {
        g.frame();
        let on = g.layer_text().contains("PRESS SPACE");
        seen = (seen.0 || on, seen.1 || !on);
    }
    assert_eq!(seen, (true, true));
    // the terrain scrolls under it, the layer doesn't
    let x0 = g.m.peek(io::SCROLLX);
    g.frame();
    assert_ne!(g.m.peek(io::SCROLLX), x0);
    assert_eq!(g.m.peek(io::LAYERSX), 0);
    // other keys don't start it
    g.press(b'X');
    g.frames(3);
    assert_eq!(g.get("STATE"), TITLE);
    g.press(b' ');
    g.until(3, "SPACE starts", |g| g.get("STATE") == PLAY);
}

#[test]
fn the_title_is_silent() {
    let mut g = Game::boot();
    for _ in 0..120 {
        g.frame();
        assert_eq!(g.m.peek(io::SPEAKER) & 3, 0);
    }
}

// ---------- playing ----------

#[test]
fn a_new_game_shows_the_ship_and_a_hud_of_sprites() {
    let mut g = Game::play();
    assert_eq!(g.get("LIVES"), 3);
    assert_eq!(g.score(), 0);
    assert_eq!(g.get("STAGE"), 0);
    g.set("INVUL", 0);
    g.frame();
    let (y, x, img, att) = g.entry(0);
    assert_eq!(att & 0x80, 0x80, "the ship is entry 0");
    assert_eq!(img, g.sym("I_SHIP") as u8);
    assert_eq!((y, x), (g.get("SHY"), 2 * g.get("SHX") as u16));
    // the HUD: entries 24-31 along the top, in the band nothing else enters
    for n in 24..32 {
        let (y, _, img, att) = g.entry(n);
        assert_eq!((y, att & 0x80), (0, 0x80), "HUD entry {n}");
        assert_eq!(img as u16, g.sym("I_DYN") + n - 24);
    }
    // the stage's banner scrolls along row 12 with the terrain
    g.frames(200);
    assert!(
        g.text().lines().nth(12).unwrap().contains("STAGE 1"),
        "{}",
        g.text()
    );
}

#[test]
fn the_hud_draws_the_score_into_its_sprites() {
    let mut g = Game::play();
    g.frames(10);
    let img = |g: &Game, n: u16| -> Vec<u8> {
        let a = g.sym("IMAGES") + (g.sym("I_DYN") + n) * 64;
        (0..64).map(|i| g.m.peek(a + i)).collect()
    };
    let zeros = img(&g, 0);
    assert!(zeros.iter().any(|&b| b != 0), "000000 is drawn");
    assert_eq!(img(&g, 0), img(&g, 1), "00 and 00");
    // 1,230 points: the second sprite shows 12 and the third 30, a frame at a time
    for (i, d) in [0u8, 0, 1, 2, 3, 0].iter().enumerate() {
        g.set_at("SCORE", i as u16, *d);
    }
    g.frames(4);
    assert_eq!(img(&g, 0), zeros);
    assert_ne!(img(&g, 1), zeros);
    assert_ne!(img(&g, 2), zeros);
    assert_ne!(img(&g, 1), img(&g, 2));
    // the HUD sprites don't move while the picture scrolls
    let before: Vec<_> = (24..32).map(|n| g.entry(n)).collect();
    let mut scrolls = std::collections::HashSet::new();
    for _ in 0..9 {
        g.frame();
        scrolls.insert(g.m.peek(io::SCROLLX));
        let now: Vec<_> = (24..32).map(|n| (g.entry(n).0, g.entry(n).1)).collect();
        assert_eq!(now, before.iter().map(|e| (e.0, e.1)).collect::<Vec<_>>());
    }
    assert_eq!(scrolls.len(), 8, "SCROLLX went through all eight");
}

#[test]
fn the_terrain_scrolls_a_pixel_a_frame_and_a_cell_every_eight() {
    let mut g = Game::play();
    let mut fine = vec![];
    for _ in 0..16 {
        let before: Vec<Vec<u8>> = (2..25).map(|r| g.screen_row(r)).collect();
        g.frame();
        let x = g.m.peek(io::SCROLLX);
        assert_eq!(x, g.get("FINE"));
        fine.push(x);
        let after: Vec<Vec<u8>> = (2..25).map(|r| g.screen_row(r)).collect();
        if x == 7 {
            // the coarse step: everything a cell left (column 38 may be mended to meet 39)
            for (b, a) in before.iter().zip(&after) {
                assert_eq!(&a[..37], &b[1..38]);
            }
        } else {
            assert_eq!(before, after, "between coarse steps only SCROLLX moves");
        }
    }
    assert!(
        fine.windows(2).all(|w| w[1] == (w[0] + 7) % 8),
        "SCROLLX counts 7 down to 0: {fine:?}"
    );
    // the layer goes a quarter as fast
    let mut lsx = vec![];
    for _ in 0..64 {
        g.frame();
        lsx.push(g.m.peek(io::LAYERSX));
    }
    let changes = lsx.windows(2).filter(|w| w[0] != w[1]).count();
    assert_eq!(changes, 16, "{lsx:?}");
}

#[test]
fn the_terrain_is_made_of_our_own_characters() {
    let mut g = Game::play();
    g.frames(30);
    let rock = g.sym("G_ROCK") as u8;
    let bottom = g.screen_row(24);
    assert!(
        bottom.iter().all(|&c| c >= rock),
        "the bottom row is ground: {bottom:?}"
    );
    let set = 0x3800;
    let glyph: Vec<u8> = (0..8)
        .map(|i| g.m.peek(set + rock as u16 * 8 + i))
        .collect();
    let rom: Vec<u8> = (0..8)
        .map(|i| g.m.peek(0xD000 + rock as u16 * 8 + i))
        .collect();
    assert_ne!(glyph, rom, "the rock is ours, not UNSCII's");
    assert_eq!(
        g.m.peek(set + b'A' as u16 * 8),
        g.m.peek(0xD000 + b'A' as u16 * 8),
        "letters are UNSCII's"
    );
    // nothing in the HUD's rows
    assert!(
        g.screen_row(0)
            .iter()
            .chain(&g.screen_row(1))
            .all(|&c| c == b' ')
    );
}

#[test]
fn the_ship_flies_with_the_cursor_keys_and_wasd() {
    let mut g = Game::play();
    g.set("SHX", 40);
    g.set("SHY", 100);
    for (k, dx, dy) in [
        (RIGHT, 1, 0),
        (LEFT, -1, 0),
        (UP, 0, -2),
        (DOWN, 0, 2),
        (b'D', 1, 0),
        (b'A', -1, 0),
        (b'W', 0, -2),
        (b'S', 0, 2),
    ] {
        let (x, y) = (g.get("SHX") as i32, g.get("SHY") as i32);
        g.hold(k, 10);
        assert_eq!(
            (g.get("SHX") as i32, g.get("SHY") as i32),
            (x + 10 * dx, y + 10 * dy),
            "key {k}"
        );
    }
    // two at once: diagonally
    g.m.key(RIGHT);
    g.m.key(UP);
    g.frames(5);
    g.m.key_up(RIGHT);
    g.m.key_up(UP);
    assert_eq!((g.get("SHX"), g.get("SHY")), (45, 90));
    // the edges stop it
    g.hold(LEFT, 100);
    assert_eq!(g.get("SHX"), g.sym("SXMIN") as u8);
    g.set("SHY", 30);
    g.hold(UP, 20);
    assert_eq!(g.get("SHY"), 16, "never into the HUD's band");
}

#[test]
fn space_fires_shots_that_are_sprites() {
    let mut g = Game::play();
    g.frames(10);
    assert_eq!(g.count("SOT", 8), 0);
    g.press(b' ');
    assert_eq!(g.count("SOT", 8), 1);
    let x0 = g.at("SOX", 0).max(g.at("SOX", 7));
    g.frame();
    let shot = (0..8).find(|&i| g.at("SOT", i) != 0).unwrap();
    assert!(g.at("SOX", shot) > x0);
    let img = g.sym("I_SHOT") as u8;
    assert!(
        (1..24).any(|n| g.entry(n).2 == img && g.entry(n).3 & 0x80 != 0),
        "a shot in the table"
    );
    // it plays on the timer
    assert_eq!(g.m.peek(io::SPEAKER) & 3, 3);
    // the guns cool between volleys; taps faster than that are lost
    g.frames(50);
    assert_eq!(g.count("SOT", 8), 0, "off the right edge");
    g.press(b' ');
    g.press(b' ');
    assert_eq!(g.count("SOT", 8), 1);
    g.frames(5);
    g.press(b' ');
    assert_eq!(g.count("SOT", 8), 2);
}

#[test]
fn holding_space_charges_the_lance_and_letting_go_fires_it() {
    let mut g = Game::play();
    g.frames(40);
    g.set("INVUL", 0);
    g.m.key(b' ');
    g.frames(21);
    assert_eq!(
        g.get("CHG"),
        20,
        "the press fires a shot, then each frame held charges"
    );
    assert_eq!(g.entry(0).2, g.sym("I_SHIPC") as u8, "the nose sparks");
    assert_eq!(g.m.peek(io::SPEAKER) & 3, 3, "and hums");
    // the HUD's bar: two characters of eighths
    g.frame();
    assert_eq!((g.at("HUDC", 8), g.at("HUDC", 9)), (5, 0));
    g.frames(60);
    assert_eq!(g.get("CHG"), 64, "full");
    assert_eq!((g.at("HUDC", 8), g.at("HUDC", 9)), (8, 8));
    g.m.key_up(b' ');
    g.frame();
    let lance = (0..8).find(|&i| g.at("SOT", i) == 4).expect("a wide LANCE");
    assert_eq!(g.get("CHG"), 0);
    let e = (1..24)
        .find(|&n| g.entry(n).2 == g.sym("I_LANCE") as u8)
        .unwrap();
    assert_eq!(g.entry(e).3 & 0x82, 0x82, "drawn wide");
    let _ = lance;
    // a half charge: the narrow one
    g.frames(40);
    g.m.key(b' ');
    g.frames(30);
    g.m.key_up(b' ');
    g.frame();
    assert!((0..8).any(|i| g.at("SOT", i) == 3));
    // a short hold: nothing more than the shot it started with
    g.frames(40);
    let n = g.count("SOT", 8);
    g.m.key(b' ');
    g.frames(10);
    g.m.key_up(b' ');
    g.frames(2);
    assert_eq!(g.count("SOT", 8), n + 1);
}

/// Puts something in enemy slot k at (x, y), standing still: a GROWLER (3) only tumbles
/// in place, a capsule (7) only sits.
fn park(g: &mut Game, k: u16, kind: u8, x: u8, y: u8) {
    g.set_at("ENT", k, kind);
    g.set_at("ENX", k, x);
    g.set_at("ENY", k, y);
    g.set_at("ENB", k, y);
    g.set_at("ENVX", k, 0);
    g.set_at("ENVY", k, 0);
    g.set_at("ENHP", k, 1);
    g.set_at("ENWS", k, 0);
    g.set_at("ENS", k, 1);
    g.set_at("ENATT", k, 0);
}

#[test]
fn waves_of_enemies_come_in_from_the_right() {
    let mut g = Game::play();
    let n = g.until(300, "a wave", |g| g.count("ENT", 8) > 0);
    eprintln!("the first wave after {n} frames");
    let k = (0..8).find(|&i| g.at("ENT", i) != 0).unwrap();
    assert_eq!(g.at("ENT", k), 1, "WISPS first");
    let x0 = g.at("ENX", k);
    assert!(x0 >= 150);
    g.until(200, "five of them", |g| g.count("ENT", 8) >= 5);
    g.frames(30);
    let k = (0..8).find(|&i| g.at("ENT", i) == 1).unwrap();
    let (x, y) = (g.at("ENX", k), g.at("ENY", k));
    g.frames(10);
    assert!(g.at("ENX", k) < x, "they come left");
    assert_ne!(g.at("ENY", k), y, "on a sine wave");
    assert!(g.shown().len() > 5);
}

#[test]
fn shooting_a_growler_scores_and_blows_it_up() {
    let mut g = Game::play();
    g.frames(5);
    g.set("WAVET", 255);
    g.set("SHY", 100);
    park(&mut g, 0, 3, 60, 96);
    g.set("SHX", 20);
    g.press(b' ');
    g.until(30, "the hit", |g| g.at("ENT", 0) != 3);
    assert_eq!(g.at("ENT", 0), 6, "an explosion");
    assert_eq!(g.score(), 300);
    assert_eq!(g.count("SOT", 8), 0, "the shot is spent");
    g.frames(20);
    assert_eq!(g.at("ENT", 0), 0, "and gone");
}

#[test]
fn a_whole_wave_shot_down_leaves_a_capsule_and_the_capsule_upgrades() {
    let mut g = Game::play();
    g.frames(5);
    g.set("WAVET", 255);
    g.set("SHX", 20);
    g.set("SHY", 100);
    // a wave of two, number 1
    for k in 0..2 {
        park(&mut g, k, 3, 50 + 12 * k as u8, 96);
        g.set_at("ENWS", k, 1);
    }
    g.set_at("WSIZE", 1, 2);
    g.set_at("WKILL", 1, 0);
    g.press(b' ');
    g.frames(10);
    g.press(b' ');
    g.until(40, "a capsule", |g| (0..8).any(|i| g.at("ENT", i) == 7));
    let k = (0..8).find(|&i| g.at("ENT", i) == 7).unwrap();
    assert_eq!(g.at("ENB", k), 0, "FAN first");
    // fly into it
    g.set("INVUL", 0);
    g.set_at("ENVX", k, 0);
    g.set("SHX", g.at("ENX", k));
    g.set("SHY", g.at("ENY", k));
    g.frames(3);
    assert_eq!(g.at("ENT", k), 0, "picked up");
    assert_eq!(g.get("WEAP") & 1, 1);
    g.frames(10);
    g.press(b' ');
    assert_eq!(g.count("SOT", 8), 3, "three at once");
}

#[test]
fn the_firefly_follows_and_fires_with_you() {
    let mut g = Game::play();
    g.frames(5);
    g.set("WAVET", 255);
    g.set("SHX", 60);
    g.set("SHY", 100);
    park(&mut g, 0, 7, 60, 100);
    g.set_at("ENB", 0, 2);
    g.frames(3);
    assert_eq!(g.get("DRON"), 1);
    g.frames(40);
    assert_eq!((g.get("DRX"), g.get("DRY")), (53, 86), "above and behind");
    g.press(b' ');
    assert_eq!(g.count("SOT", 8), 2);
    let img = g.sym("I_FLY0") as u8;
    assert!((1..24).any(|n| g.entry(n).2 == img || g.entry(n).2 == img + 1));
}

#[test]
fn shooting_a_frosteye_leaves_a_pit() {
    let mut g = Game::play();
    g.frames(5);
    g.set("WAVET", 255);
    g.set("INVUL", 255);
    let eye = g.sym("G_EYEF") as u8;
    // a FROSTEYE in place of the ground's top cell in column 30, in the list
    let row = (8..25u16)
        .find(|&r| g.m.peek(SCREEN + r * 40 + 30) != b' ')
        .unwrap();
    g.m.poke(SCREEN + row * 40 + 30, eye);
    for c in 0..30 {
        g.m.poke(SCREEN + row * 40 + c, b' '); // a clear line of fire
    }
    g.set_at("TCOL", 0, 30);
    g.set_at("TROW", 0, row as u8);
    g.set_at("TACT", 0, 1);
    g.set("SHY", (row * 8 - 7) as u8);
    g.set("SHX", 20);
    g.press(b' ');
    let t = g.until(40, "the hit", |g| g.at("TACT", 0) == 0);
    assert!(t > 0);
    assert!(
        g.screen_row(row).contains(&(g.sym("G_PITF") as u8)),
        "a pit where it was"
    );
    assert_eq!(g.score(), 200);
}

#[test]
fn frosteyes_fire_at_the_ship() {
    let mut g = Game::play();
    g.frames(5);
    g.set("WAVET", 255);
    g.set("INVUL", 255);
    g.set_at("TCOL", 1, 30);
    g.set_at("TROW", 1, 18);
    g.set_at("TACT", 1, 1);
    g.until(400, "a bullet", |g| g.count("BUT", 5) > 0);
}

#[test]
fn a_bullet_that_touches_the_ship_blows_it_up() {
    let mut g = Game::play();
    g.frames(5);
    g.set("WAVET", 255);
    g.set("INVUL", 0);
    g.set("SHX", 40);
    g.set("SHY", 100);
    // a bullet sitting on the ship
    g.set_at("BUT", 0, 1);
    g.set_at("BUX", 0, 42);
    g.set_at("BUY", 0, 100);
    g.set_at("BUVX", 0, 0);
    g.set_at("BUVY", 0, 0);
    g.frames(3);
    assert_eq!(g.get("SHIPST"), 1, "SPRHIT bit 0, then the box: blown up");
    assert_eq!(g.get("LIVES"), 2);
    assert_eq!(g.m.peek(io::BORDER), 2, "the border flashes");
    let n = g.until(200, "the next ship", |g| g.get("SHIPST") == 0);
    eprintln!("the next ship after {n} frames");
    assert!(g.get("INVUL") > 0, "blinking and safe");
}

#[test]
fn a_bullet_beside_the_ship_misses() {
    let mut g = Game::play();
    g.frames(5);
    g.set("WAVET", 255);
    g.set("INVUL", 0);
    g.set("SHX", 40);
    g.set("SHY", 100);
    // in the ship's 16 x 16 square, but not on its pixels
    g.set_at("BUT", 0, 1);
    g.set_at("BUX", 0, 43);
    g.set_at("BUY", 0, 105 - 16);
    g.set_at("BUVX", 0, 0);
    g.set_at("BUVY", 0, 0);
    g.frames(3);
    assert_eq!(g.get("SHIPST"), 0);
}

#[test]
fn flying_into_the_ground_blows_the_ship_up() {
    let mut g = Game::play();
    g.frames(5);
    g.set("WAVET", 255);
    g.set("INVUL", 0);
    g.set("SHX", 40);
    g.set("SHY", 150);
    g.m.key(DOWN);
    g.until(40, "a crash", |g| g.get("SHIPST") == 1);
    g.m.key_up(DOWN);
    // where the ground is
    let row = (g.get("SHY") as u16 + 13) / 8;
    eprintln!("crashed at y {} (row {row})", g.get("SHY"));
    assert!(row >= 17);
}

#[test]
fn stars_and_the_aurora_dont_count_as_terrain() {
    let mut g = Game::play();
    g.frames(5);
    g.set("WAVET", 255);
    // on the aurora's rows, under an open sky, for a while
    g.set("SHX", 60);
    g.set("SHY", 30);
    let mut touched = false;
    for _ in 0..200 {
        g.set("INVUL", 0);
        g.frame();
        touched |= g.get("HITBG") & 1 != 0;
        assert_eq!(g.get("SHIPST"), 0);
    }
    assert!(
        touched,
        "SPRHITBG saw the layer's ink, and the ship looked at the screen and flew on"
    );
}

#[test]
fn a_crowded_line_makes_the_table_turn_round() {
    let mut g = Game::play();
    g.frames(5);
    g.set("WAVET", 255);
    g.set("INVUL", 255);
    g.set("SHY", 40);
    // eight GROWLERS and five bullets on one line
    for k in 0..8 {
        park(&mut g, k, 3, 10 + 16 * k as u8, 120);
    }
    for k in 0..5 {
        g.set_at("BUT", k, 1);
        g.set_at("BUX", k, 30 + 20 * k as u8);
        g.set_at("BUY", k, 120);
        g.set_at("BUVX", k, 0);
        g.set_at("BUVY", k, 0);
    }
    for k in 0..8 {
        g.set_at("ENVX", k, 0);
    }
    g.frames(3);
    assert!(g.get("FLICK") > 0, "SPRLINE said so");
    let order = |g: &Game| (1..14).map(|n| g.entry(n).1).collect::<Vec<_>>();
    let a = order(&g);
    g.frame();
    let b = order(&g);
    assert_ne!(a, b, "a different order each frame");
    // every one of them shows in some frame
    let mut seen = std::collections::HashSet::new();
    for _ in 0..14 {
        g.frame();
        for n in 1..9 {
            seen.insert((g.entry(n).1, g.entry(n).2));
        }
    }
    assert_eq!(seen.len(), 13, "everyone gets a turn in the first eight");
}

#[test]
fn the_warden_comes_at_the_end_of_the_stage_and_its_fall_starts_the_next() {
    let mut g = Game::play();
    g.frames(5);
    // skip to the boss record of the first stage's script
    let mut p = g.sym("sc_fells");
    while g.m.peek(p) != 10 {
        p += 4;
    }
    g.set_at("WAVEP", 0, p as u8);
    g.set_at("WAVEP", 1, (p >> 8) as u8);
    g.set("WLEFT", 0);
    g.set("WAVET", 1);
    g.frames(2);
    assert_ne!(g.get("BOSS"), 0);
    let b = g.get("BOSS") as u16 - 1;
    assert_eq!(g.at("ENT", b), 4);
    assert_eq!(g.count("ENT", 8), 3, "with two SHARDS");
    g.frames(120);
    let e = (1..24)
        .find(|&n| g.entry(n).2 == g.sym("I_WARD0") as u8)
        .expect("the WARDEN in the table");
    assert_eq!(g.entry(e).3 & 0x82, 0x82, "wide");
    // down to one hit, then a shot
    g.set_at("ENHP", b, 1);
    g.set("INVUL", 255);
    g.set("SHY", g.at("ENY", b) + 8);
    let c3 = g.m.peek(io::COLOUR3);
    for _ in 0..20 {
        g.set("SHY", g.at("ENY", b).wrapping_add(8));
        g.press(b' ');
        g.frames(5);
        if g.get("BOSS") == 0 {
            break;
        }
    }
    assert_eq!(g.get("BOSS"), 0, "down");
    assert!(g.score() >= 5000);
    g.until(200, "the next stage", |g| g.get("STAGE") == 1);
    assert_ne!(g.m.peek(io::COLOUR3), c3, "a new colour for the terrain");
    assert_eq!(g.get("DIFF"), 1);
}

#[test]
fn losing_the_last_ship_is_game_over_and_space_starts_again() {
    let mut g = Game::play();
    g.frames(5);
    g.set("WAVET", 255);
    g.set("SHX", 20);
    g.set("SHY", 100);
    park(&mut g, 0, 3, 60, 96);
    g.press(b' ');
    g.until(30, "300 points", |g| g.score() == 300);
    g.set("LIVES", 1);
    g.set("INVUL", 0);
    g.set_at("BUT", 0, 1);
    g.set_at("BUX", 0, g.get("SHX") + 2);
    g.set_at("BUY", 0, 100);
    g.set_at("BUVX", 0, 0);
    g.set_at("BUVY", 0, 0);
    g.until(200, "game over", |g| g.get("STATE") == OVER);
    assert!(g.text().contains("GAME OVER"));
    assert_eq!(
        (0..24).filter(|&n| g.entry(n).3 & 0x80 != 0).count(),
        0,
        "only the HUD is left"
    );
    assert_eq!(
        (0..6).map(|i| g.at("HISC", i)).collect::<Vec<_>>(),
        [0, 0, 0, 3, 0, 0],
        "the high score"
    );
    g.press(b' ');
    g.frames(2);
    assert_eq!(g.get("STATE"), OVER, "not straight away");
    g.frames(40);
    g.press(b' ');
    g.until(3, "a new game", |g| g.get("STATE") == PLAY);
    g.frames(2);
    assert_eq!((g.get("LIVES"), g.score()), (3, 0));
    assert!(!g.text().contains("GAME OVER"));
    assert_eq!(g.at("HISC", 3), 3, "kept");
}

#[test]
fn a_new_game_after_game_over_scrolls_again() {
    // Game over stops the terrain; the next game must start it again, and the
    // layer must keep its quarter speed (with SCR stuck where the layer's coarse
    // step falls, the aurora jumped a cell every frame).
    let mut g = Game::play();
    g.frames(5);
    g.set("LIVES", 1);
    g.set("INVUL", 0);
    g.set_at("BUT", 0, 1);
    g.set_at("BUX", 0, g.get("SHX") + 2);
    g.set_at("BUY", 0, g.get("SHY"));
    g.set_at("BUVX", 0, 0);
    g.set_at("BUVY", 0, 0);
    g.until(200, "game over", |g| g.get("STATE") == OVER);
    g.frames(40);
    g.set("SCR", 27); // (SCR + 4) & 31 == 31: the layer's coarse step
    g.press(b' ');
    g.until(3, "a new game", |g| g.get("STATE") == PLAY);
    g.frames(2);
    let mut fine = vec![];
    let mut lsx = vec![];
    let lay0 = g.layer_text();
    let mut lay_steps = 0;
    let mut lay = lay0.clone();
    for _ in 0..32 {
        g.frame();
        fine.push(g.get("FINE"));
        lsx.push(g.m.peek(io::LAYERSX));
        let l = g.layer_text();
        if l != lay {
            lay_steps += 1;
            lay = l;
        }
    }
    assert!(
        fine.windows(2).all(|w| w[0] != w[1]),
        "the terrain moves a pixel every frame: {fine:?}"
    );
    assert!(lsx.windows(2).any(|w| w[0] != w[1]), "the layer moves: {lsx:?}");
    assert!(
        lay_steps <= 2,
        "the layer moves a cell every 32 frames, not every frame ({lay_steps} in 32)"
    );
}

#[test]
fn game_over_goes_back_to_the_title_by_itself() {
    let mut g = Game::play();
    g.frames(5);
    g.set("LIVES", 1);
    g.set("INVUL", 0);
    g.set_at("BUT", 0, 1);
    g.set_at("BUX", 0, g.get("SHX") + 2);
    g.set_at("BUY", 0, g.get("SHY"));
    g.set_at("BUVX", 0, 0);
    g.set_at("BUVY", 0, 0);
    g.until(200, "game over", |g| g.get("STATE") == OVER);
    g.until(1600, "the title", |g| g.get("STATE") == TITLE);
    g.frames(3);
    assert!(
        g.layer_text()
            .contains("A DEMONSTRATION OF THE VIDEO BOARD")
    );
}

// ---------- the frame budget ----------

#[test]
fn every_frame_of_play_fits_in_one_50_hz_frame() {
    // A hard case: the gorge (the deepest roof and ground, so the most rows to move),
    // DIFF 7 (more of everything), every weapon, holding nothing back.
    let mut g = Game::play();
    g.set("WEAP", 3);
    g.set("DRON", 1);
    g.set("STAGE", 7);
    g.set("DIFF", 7);
    let mut d = Dice(12345);
    for i in 0..3000u32 {
        g.set("INVUL", 200);
        g.set("LIVES", 5);
        if i < 2000 {
            g.set("STYLE", 3);
        }
        if i % 6 == 0 {
            g.m.key(b' ');
        } else if i % 6 == 1 {
            g.m.key_up(b' ');
        }
        if i % 25 == 0 {
            for k in [UP, DOWN, LEFT, RIGHT] {
                g.m.key_up(k);
            }
            let k = [UP, DOWN, LEFT, RIGHT][(d.next() % 4) as usize];
            g.m.key(k);
        }
        g.frame();
        assert_eq!(g.get("STATE"), PLAY);
    }
    let mean = g.total / g.timed;
    eprintln!(
        "play: worst frame {} cycles, mean {mean}, of {BUDGET}; MAXC {} MAXF {}",
        g.worst,
        g.get("MAXC"),
        g.get("MAXF")
    );
    assert!(g.worst < BUDGET, "the worst frame took {} cycles", g.worst);
    assert_eq!(g.get("LATE"), 0, "no frame of play ran into the next");
}

#[test]
fn a_long_game_with_random_keys_never_jams() {
    // The machine stops the test if it jams or halts (Game::frame). Random keys, held
    // for random spells, through deaths, game overs, the title and new games.
    let mut g = Game::boot();
    let mut d = Dice(0xA5A5_1984);
    let keys = [
        LEFT, RIGHT, UP, DOWN, b'W', b'A', b'S', b'D', b' ', b' ', b'X', 13,
    ];
    let mut down: Vec<u8> = vec![];
    let mut states = std::collections::HashSet::new();
    let mut games = 0;
    for _ in 0..12_000 {
        if d.next().is_multiple_of(8) {
            let k = keys[(d.next() as usize) % keys.len()];
            if let Some(i) = down.iter().position(|&x| x == k) {
                g.m.key_up(k);
                down.remove(i);
            } else {
                g.m.key(k);
                down.push(k);
            }
        }
        let st = g.get("STATE");
        g.frame();
        if st != PLAY && g.get("STATE") == PLAY {
            games += 1;
        }
        states.insert(g.get("STATE"));
        assert!((1..=3).contains(&g.get("STATE")));
        if g.begun.is_some() {
            continue; // a new screen being set up, over more than a frame
        }
        if g.get("STATE") == PLAY {
            assert!(
                (16..=185).contains(&g.get("SHY")),
                "the ship at y {} shipst {} deadt {} lives {} games {games} st {st}",
                g.get("SHY"),
                g.get("SHIPST"),
                g.get("DEADT"),
                g.get("LIVES")
            );
        }
        // nothing but the HUD above line 16
        for n in 1..24 {
            let (y, _, _, att) = g.entry(n);
            assert!(
                att & 0x80 == 0 || (16..200).contains(&y),
                "entry {n} at y {y}"
            );
        }
    }
    eprintln!(
        "{games} games; worst frame {} cycles, mean {}",
        g.worst,
        g.total / g.timed.max(1)
    );
    assert!(games >= 2, "{games} games");
    assert_eq!(states.len(), 3, "title, play and game over all came round");
    assert!(g.worst < BUDGET);
    assert_eq!(g.get("LATE"), 0);
}

// ---------- the board's colours ----------

#[test]
fn the_aurora_shimmers_in_colour_1() {
    let mut g = Game::play();
    let mut c1 = std::collections::HashSet::new();
    for _ in 0..200 {
        g.frame();
        c1.insert(g.m.peek(io::COLOUR1));
    }
    assert!(c1.len() >= 3, "{c1:?}");
    assert_eq!(g.m.peek(io::COLOUR2), 1, "white");
    // the layer has the aurora's glyphs along the top
    let fringe = g.sym("G_FRINGE") as u8;
    let n = (0..7 * 40)
        .filter(|&i| g.m.peek(LAYER + 80 + i) & !1 == fringe)
        .count();
    assert!(n > 5, "{n} cells of the aurora's edge");
    // on the picture: colour 1 where only the layer is
    let px = g.pixels();
    assert!(px[16 * 320..72 * 320].contains(&1));
}

/// Where a frame of play goes: the cycles of each routine st_play calls, for the worst
/// frames of the hard case. `cargo test -p unc-asm --test game_aurora profile -- --ignored`
#[test]
#[ignore]
fn profile() {
    let mut g = Game::play();
    g.set("WEAP", 3);
    g.set("DRON", 1);
    g.set("STAGE", 7);
    g.set("DIFF", 7);
    let sp = g.sym("st_play");
    let names = [
        "shipcol", "input", "moveship", "fire", "scroll", "shots", "enemies", "bullets", "turrets",
        "waves", "drone", "life", "table", "hud", "colours",
    ];
    let mut worst: Vec<(u64, Vec<u64>)> = vec![];
    let mut d = Dice(12345);
    for i in 0..3000u32 {
        g.set("INVUL", 200);
        g.set("LIVES", 5);
        if i < 2000 {
            g.set("STYLE", 3);
        }
        if i % 6 == 0 {
            g.m.key(b' ');
        } else if i % 6 == 1 {
            g.m.key_up(b' ');
        }
        if i % 25 == 0 {
            for k in [UP, DOWN, LEFT, RIGHT] {
                g.m.key_up(k);
            }
            g.m.key([UP, DOWN, LEFT, RIGHT][(d.next() % 4) as usize]);
        }
        // run to st_play, then from call to call
        let end = g.m.cpu.cycles + to_vblank(&g.m) as u64;
        let mut parts = vec![];
        for k in 0..=15u16 {
            g.m.set_breakpoint(sp + 3 * k, true);
        }
        let ret = g.sym("endwork");
        g.m.set_breakpoint(ret, true);
        let mut last = None;
        while g.m.cpu.cycles < end {
            match g.m.run_cycles((end - g.m.cpu.cycles) as u32) {
                Stop::Budget => break,
                Stop::Breakpoint(pc) => {
                    if let Some(t) = last {
                        parts.push(g.m.cpu.cycles - t);
                    }
                    last = if pc == ret {
                        None
                    } else {
                        Some(g.m.cpu.cycles)
                    };
                }
                s => panic!("{s:?}"),
            }
        }
        g.m.clear_breakpoints();
        let total: u64 = parts.iter().sum();
        worst.push((total, parts));
    }
    worst.sort_by_key(|w| std::cmp::Reverse(w.0));
    for (t, parts) in worst.iter().take(6) {
        let s: Vec<String> = names
            .iter()
            .zip(parts)
            .map(|(n, c)| format!("{n} {c}"))
            .collect();
        eprintln!("{t}: {}", s.join(", "));
    }
}

/// Pictures of the game for a person to look at: `AURORA_SHOTS=dir cargo test -p unc-asm
/// --test game_aurora pictures -- --ignored`.
#[test]
#[ignore]
fn pictures() {
    let dir = std::env::var("AURORA_SHOTS").unwrap_or_else(|_| "target/aurora".into());
    std::fs::create_dir_all(&dir).unwrap();
    let mut g = Game::boot();
    g.frames(60);
    g.png(&format!("{dir}/title.png"), 2);
    g.press(b' ');
    g.frames(100);
    g.png(&format!("{dir}/play1.png"), 2);
    g.m.key(RIGHT);
    g.m.key(b' ');
    g.frames(30);
    g.m.key_up(b' ');
    g.frames(3);
    g.png(&format!("{dir}/play2.png"), 2);
    g.m.key_up(RIGHT);
    for i in 0..600 {
        if i % 10 == 0 {
            g.m.key(b' ');
        }
        if i % 10 == 1 {
            g.m.key_up(b' ');
        }
        g.frame();
        if i % 100 == 99 {
            g.png(&format!("{dir}/play{}.png", 3 + i / 100), 2);
        }
    }
    eprintln!("worst {} mean {}", g.worst, g.total / g.timed.max(1));
}
