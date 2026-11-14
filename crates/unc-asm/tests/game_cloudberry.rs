//! CLOUDBERRY (games/niece/cloudberry.s): a platformer that scrolls, for the NIECE, the
//! prototype graphics chip of manual chapter 17. Played through scripted keys on a booted 64K
//! machine with the chip in; its state is read from the addresses the source's symbol table
//! gives.
//!
//! The game is there to answer two questions. Is the scroll right: does the page show the
//! level a pixel further on for every pixel the camera moves, across the join of the ring it's
//! kept in on the sheet and across any number of new columns? `the_page` works the whole
//! picture out again from the map, the tiles and the backdrops and compares every pixel. And
//! does a 1 MHz processor hold it at 50 frames a second? The meter measures both halves of a
//! frame, the processor's work (`work` to `endwork`) and the chip's drawing (every command
//! written to NGO, costed as chapter 17 costs it), and that every FLIP came at the very next
//! vertical blank.

use unc_core::machine::{CYCLES_PER_FRAME, CYCLES_PER_LINE, VBLANK_LINE};
use unc_core::niece::{self, Command};
use unc_core::{CpuState, Machine, RamSize, Stop};

const SRC: &str = include_str!("../../../games/niece/cloudberry.s");
const BUDGET: u64 = CYCLES_PER_FRAME as u64;

const TITLE: u8 = 1;
const PLAY: u8 = 2;
const DYING: u8 = 3;
const CLEAR: u8 = 4;
const OVER: u8 = 5;

const LEFT: u8 = 28;
const RIGHT: u8 = 29;
const UP: u8 = 30;
const SPACE: u8 = b' ';

const NGO: u16 = 0xC103;
const STA_ABS: u8 = 0x33;

const KEYC: u8 = 4;
const TOPY: i32 = 4;
const SOLID: u8 = 0x80;
const ONEWAY: u8 = 0x40;
const E_BERRY: u8 = 1;
const E_LEM: u8 = 2;
const E_MIDGE: u8 = 3;
const E_FLAT: u8 = 4;
const F_KEY: u8 = 0x01;
const F_FLIPX: u8 = 0x02;
const F_SHEET: u8 = 0x20;
const F_WHITE: u8 = 0x40;

/// What a stretch of frames cost, the processor and the chip.
#[derive(Default, Debug)]
struct Meter {
    frames: u64,
    /// `work` to `endwork`, in cycles: the worst frame and all of them.
    worst: u64,
    total: u64,
    /// From one `work` to the next: 19,968 when the FLIP came at the next vertical blank.
    longest: u64,
    shortest: u64,
    last_work: Option<u64>,
    begun: Option<u64>,
    /// Commands written to NGO in a frame, the FLIP not counted: the most, and all.
    most_commands: u64,
    commands: u64,
    /// The chip's drawing in a frame, in its cycles (4 + pixels / 4 a command).
    most_drawing: u64,
    drawing: u64,
    /// The latest the chip finished a frame's drawing, in cycles after `work`.
    latest_done: u64,
    /// The deepest the queue got, the command just written included.
    deepest: usize,
    /// Commands written to a full queue.
    lost: u64,
    // this frame so far
    f_commands: u64,
    f_drawing: u64,
    chip_free: u64,
}

struct Game {
    m: Machine,
    asm: unc_asm::Assembly,
    meter: Meter,
    work: u16,
    endwork: u16,
    /// The commands written since the last `work`, the FLIP too.
    written: Vec<Command>,
    /// And the ones of the frame before that: a whole frame's.
    frame_commands: Vec<Command>,
}

/// Cycles from now to the start of the next VBLANK line (a whole frame if it's now).
fn to_vblank(m: &Machine) -> u32 {
    let at = VBLANK_LINE * CYCLES_PER_LINE;
    match (at + CYCLES_PER_FRAME - m.mem.frame_cycle) % CYCLES_PER_FRAME {
        0 => CYCLES_PER_FRAME,
        n => n,
    }
}

fn assemble() -> unc_asm::Assembly {
    unc_asm::assemble(SRC).unwrap_or_else(|e| panic!("cloudberry.s doesn't assemble: {e:?}"))
}

impl Game {
    /// A UNC 64 with the NIECE, booted to READY., with CLOUDBERRY loaded and started as the
    /// hosts start a program, and run until the sheet is drawn and the title is up.
    fn boot() -> Game {
        let mut g = Game::boot_niece(true);
        g.until(400, "the title", |g| g.get("STATE") == TITLE && g.chip().shown());
        g.frames(3);
        g.meter = Meter::default();
        g
    }

    fn boot_niece(niece: bool) -> Game {
        let asm = assemble();
        let mut m = Machine::boot(RamSize::K64);
        m.set_niece(niece);
        unc_asm::load(&mut m, &asm);
        let (work, endwork) = (asm.symbol("work").unwrap(), asm.symbol("endwork").unwrap());
        Game { m, asm, meter: Meter::default(), work, endwork, written: Vec::new(), frame_commands: Vec::new() }
    }

    /// Boots and presses SPACE on the title: level 1, standing at its start, its name gone
    /// from the screen.
    fn play() -> Game {
        let mut g = Game::boot();
        g.press(SPACE);
        g.until(60, "SPACE starts a game", |g| g.get("STATE") == PLAY);
        g.until(100, "he lands", |g| g.get("GROUND") == 1);
        g.set("BANT", 1);
        g.frames(2);
        g.to_work();
        g.meter = Meter::default();
        g
    }

    fn chip(&self) -> &niece::Niece {
        self.m.mem.niece.as_ref().expect("the NIECE")
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

    /// One instruction, with the meter watching: `work` and `endwork` going by, and every
    /// command written to NGO.
    fn step(&mut self) {
        let pc = self.m.cpu.pc;
        let now = self.m.cpu.cycles;
        let mt = &mut self.meter;
        if pc == self.work {
            if let Some(t) = mt.last_work {
                let d = now - t;
                mt.longest = mt.longest.max(d);
                mt.shortest = if mt.shortest == 0 { d } else { mt.shortest.min(d) };
                mt.most_commands = mt.most_commands.max(mt.f_commands);
                mt.most_drawing = mt.most_drawing.max(mt.f_drawing);
            }
            mt.last_work = Some(now);
            mt.begun = Some(now);
            mt.f_commands = 0;
            mt.f_drawing = 0;
        } else if pc == self.endwork
            && let Some(t) = mt.begun.take()
        {
            let c = now - t;
            mt.worst = mt.worst.max(c);
            mt.total += c;
            mt.frames += 1;
        }
        let go = self.m.peek(pc) == STA_ABS && self.m.peek(pc + 1) == NGO as u8 && self.m.peek(pc + 2) == (NGO >> 8) as u8;
        let command = go.then(|| {
            let n = self.m.mem.niece.as_ref().unwrap();
            Command {
                op: self.m.cpu.a,
                dx: n.dx,
                dy: n.dy,
                dw: n.dw,
                dh: n.dh,
                sx: n.sx,
                sy: n.sy,
                sw: n.sw,
                sh: n.sh,
                colour: n.colour,
                pattern: n.pattern,
                flags: n.flags,
                depth: n.depth_reg,
                key: n.key,
                ink: [0; 16],
                at: now,
            }
        });
        if self.m.step() == 0 {
            panic!("the machine stopped: {:?} at ${:04X}", self.m.cpu.state, self.m.cpu.pc);
        }
        if let Some(c) = command {
            let n = self.m.mem.niece.as_ref().unwrap();
            let mt = &mut self.meter;
            if n.lost {
                mt.lost += 1;
            }
            // the store caught the chip up to this instruction, so this is the queue as the
            // chip has it, with the new command in
            mt.deepest = mt.deepest.max(n.queue.len());
            if c.op == niece::cmd::FLIP {
                if let Some(t) = mt.begun {
                    mt.latest_done = mt.latest_done.max(mt.chip_free.max(now) - t);
                }
            } else {
                mt.f_commands += 1;
                mt.commands += 1;
                mt.f_drawing += c.cycles();
                mt.drawing += c.cycles();
                mt.chip_free = mt.chip_free.max(now) + c.cycles();
            }
            self.written.push(c);
        }
        if self.m.cpu.pc == self.work {
            self.frame_commands = std::mem::take(&mut self.written);
        }
    }

    /// One frame of machine time: to the next VBLANK, where FRAME goes up and a FLIP happens
    /// (manual chapters 9 and 17), an instruction at a time.
    fn frame(&mut self) {
        let end = self.m.cpu.cycles + to_vblank(&self.m) as u64;
        while self.m.cpu.cycles < end {
            self.step();
        }
    }

    fn frames(&mut self, n: u32) {
        for _ in 0..n {
            self.frame();
        }
    }

    /// On to the start of the next frame's work: the last frame's picture is on the screen,
    /// and CAM, TICK and the rest are still what it was drawn from.
    fn to_work(&mut self) {
        if self.m.cpu.pc == self.work {
            self.step();
        }
        for _ in 0..40 * BUDGET {
            if self.m.cpu.pc == self.work {
                return;
            }
            self.step();
        }
        panic!("never came back to work\n{}", self.text());
    }

    /// A tap: down for two frames, then up.
    fn press(&mut self, k: u8) {
        self.m.key(k);
        self.frames(2);
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
        panic!("{what}: not within {max} frames\n{}", self.text());
    }

    fn text(&self) -> String {
        self.m.screen_text()
    }

    fn word(&self, lo: &str, hi: &str) -> i32 {
        self.get(lo) as i32 | (self.get(hi) as i32) << 8
    }
    /// His left edge in the level, and his top line.
    fn px(&self) -> i32 {
        self.word("PXL", "PXH")
    }
    fn py(&self) -> i32 {
        self.get("PY") as i32
    }
    fn cam(&self) -> i32 {
        self.word("CAML", "CAMH")
    }
    /// His speed across and down, in 256ths of a pixel a frame.
    fn vx(&self) -> i32 {
        self.word("VXL", "VXH") as i16 as i32
    }
    fn vy(&self) -> i32 {
        self.word("VYL", "VYH") as i16 as i32
    }
    fn score(&self) -> u32 {
        (0..5).fold(0, |s, i| s * 10 + self.at("SCORE", i) as u32)
    }
    fn berries(&self) -> u32 {
        self.get("BERRIES") as u32 * 10 + self.at("BERRIES", 1) as u32
    }
    fn time(&self) -> u32 {
        (0..3).fold(0, |s, i| s * 10 + self.at("TIMEV", i) as u32)
    }

    /// Puts him at (x, y) in the level, standing still, and the camera with him.
    fn stand(&mut self, x: i32, y: i32) {
        self.set("PXL", x as u8);
        self.set("PXH", (x >> 8) as u8);
        self.set("PY", y as u8);
        for n in ["PXF", "PYF", "VXL", "VXH", "VYL", "VYH"] {
            self.set(n, 0);
        }
        self.set("CAMSPD", 6);
        self.until(400, "the camera comes", |g| g.get("CAMSPD") == 2);
    }

    /// Puts him at (x, y) this instant, falling if there's nothing under him.
    fn put(&mut self, x: i32, y: i32) {
        self.set("PXL", x as u8);
        self.set("PXH", (x >> 8) as u8);
        self.set("PY", y as u8);
        for n in ["PXF", "PYF", "VXL", "VXH", "VYL", "VYH"] {
            self.set(n, 0);
        }
    }

    /// From anywhere in level 1 to the start of level 2, by way of the hut.
    fn to_level_two(&mut self) {
        let door = (0..256).find(|&c| self.flags(c, 8) == 0x10).expect("a door");
        self.stand(door * 8 - 40, 60);
        self.m.key(RIGHT);
        self.until(100, "the hut", |g| g.get("STATE") == CLEAR);
        self.m.key_up(RIGHT);
        self.until(400, "the count", |g| g.get("CT") == 1);
        self.frames(2);
        self.press(SPACE);
        self.until(60, "level 2", |g| g.get("STATE") == PLAY && g.get("LEVEL") == 1);
        self.until(100, "he lands", |g| g.get("GROUND") == 1);
        self.set("BANT", 1);
        self.frames(2);
        self.to_work();
        self.meter = Meter::default();
    }

    /// The tile at (column, row) of the level, and what it does.
    fn tile(&self, col: i32, row: i32) -> u8 {
        if !(0..12).contains(&row) {
            return 0;
        }
        self.m.peek(self.sym("MAP") + (row * 256 + (col & 255)) as u16)
    }
    fn flags(&self, col: i32, row: i32) -> u8 {
        self.at("tflag", self.tile(col, row) as u16)
    }

    /// The things of one kind: their numbers in the list.
    fn things(&self, kind: u8) -> Vec<u16> {
        (0..128).filter(|&i| self.at("ETYPE", i) == kind).collect()
    }
    fn thing_x(&self, i: u16) -> i32 {
        self.at("EXL", i) as i32 | (self.at("EXH", i) as i32) << 8
    }
    fn no_things(&mut self) {
        for i in 0..128 {
            self.set_at("ETYPE", i, 0);
        }
    }

    /// The page the NIECE shows, a colour (0-15) a pixel, 160 x 100.
    fn page(&self) -> Vec<u8> {
        let mut px = Vec::with_capacity(16000);
        let mut line = [0u8; 160];
        for y in 0..100 {
            self.chip().shown_line(y, &mut line);
            px.extend_from_slice(&line);
        }
        px
    }

    /// The page as it ought to be, worked out again from nothing: the black strip, the sky's
    /// bands, the clouds, the fells and the hills from their strips on the sheet, each at its
    /// own fraction of the camera, and in front of them the level, from the map and the
    /// tiles. No things and no picker: where those are is the caller's business. `None` for
    /// a pixel means the caller shouldn't ask (there are none yet).
    fn the_page(&self) -> Vec<u8> {
        let n = self.chip();
        let (cam, tick) = (self.cam(), self.word("TICK", "TICKH"));
        let (felly, cloudy, tiley, neary) = (self.sym("FELLY") as i32, self.sym("CLOUDY") as i32, self.sym("TILEY") as i32, self.sym("NEARY") as i32);
        // the clouds and fells, and the hills, go through the ink tables their NFLAGS name
        let (fell_ink, hill_ink) = (n.inks[(self.get("SKYF") >> 6) as usize], n.inks[(self.get("HILLF") >> 6) as usize]);
        let bands: Vec<(i32, i32, u8, u8)> =
            (0..7).map(|b| (self.at("BANDS", b * 4) as i32, self.at("BANDS", b * 4 + 1) as i32, self.at("BANDS", b * 4 + 2), self.at("BANDS", b * 4 + 3))).collect();
        let map: Vec<u8> = (0..12 * 256).map(|i| self.at("MAP", i)).collect();
        let mut page = vec![0u8; 16000];
        for y in 0..100i32 {
            for x in 0..160i32 {
                let mut c = 0;
                // the sky
                for &(by, bh, col, pat) in &bands {
                    if (by..by + bh).contains(&y) {
                        let bit = pat >> (7 - (x & 3) - 4 * (y & 1)) & 1;
                        c = if bit != 0 { col >> 4 } else { col & 15 };
                    }
                }
                if (6..26).contains(&y) {
                    let p = n.pixel((cam >> 3) + (tick >> 4) + x, cloudy + y - 6);
                    if p != KEYC {
                        c = fell_ink[p as usize];
                    }
                }
                if (28..60).contains(&y) {
                    let p = n.pixel((cam >> 2) + x, felly + y - 28);
                    if p != KEYC {
                        c = fell_ink[p as usize];
                    }
                }
                if (36..84).contains(&y) {
                    let p = n.pixel((cam >> 1) + x, neary + y - 36);
                    if p != KEYC {
                        c = hill_ink[p as usize];
                    }
                }
                if y >= TOPY {
                    let (wx, wy) = (cam + x, y - TOPY);
                    let t = map[((wy >> 3) * 256 + (wx >> 3)) as usize] as i32;
                    let p = n.pixel((t & 31) * 8 + (wx & 7), tiley + (t >> 5) * 8 + (wy & 7));
                    if p != KEYC {
                        c = p;
                    }
                }
                page[(y * 160 + x) as usize] = c;
            }
        }
        page
    }

    /// Asserts the page shown is `the_page`, but for the picker's own rectangle.
    fn assert_page(&self, what: &str) {
        let (shown, want) = (self.page(), self.the_page());
        let (psx, py) = (self.px() - self.cam(), self.py() + TOPY);
        for y in 0..100 {
            for x in 0..160 {
                if (psx - 4..psx + 10).contains(&x) && (py - 4..py + 12).contains(&y) {
                    continue;
                }
                let i = (y * 160 + x) as usize;
                assert_eq!(shown[i], want[i], "{what}: the page at ({x}, {y}) with the camera at {}", self.cam());
            }
        }
    }

    /// The BLIT that drew the picker in the last whole frame: its source is one of his six
    /// pictures.
    fn picker(&self) -> Option<&Command> {
        let spry = self.sym("SPRY");
        self.frame_commands.iter().find(|c| c.op == niece::cmd::BLIT && c.sy == spry && c.sx < 72 && c.sw == 12 && c.sh == 14 && c.flags & F_SHEET == 0)
    }

    /// The four BLITs of the backdrop in the last whole frame: the clouds, the fells, the
    /// hills and the level.
    fn backdrop(&self) -> Vec<&Command> {
        self.frame_commands.iter().filter(|c| c.op == niece::cmd::BLIT && c.dw == 160 && c.flags & F_SHEET == 0).collect()
    }

    fn report(&self, what: &str) {
        let m = &self.meter;
        let n = m.frames.max(1);
        eprintln!(
            "{what}: {} frames; work: worst {} cycles, mean {} of {BUDGET}; commands a frame: most {}, mean {}; \
             drawing a frame: most {} cycles ({} pixels), mean {} cycles, done at worst {} cycles after work began; \
             queue: deepest {} of 64; frame to frame: {} to {} cycles; lost {}",
            m.frames,
            m.worst,
            m.total / n,
            m.most_commands,
            m.commands / n,
            m.most_drawing,
            m.most_drawing * 4,
            m.drawing / n,
            m.latest_done,
            m.deepest,
            m.shortest,
            m.longest,
            m.lost
        );
    }

    /// The claim: every frame's work fits a frame, the chip's drawing too, every FLIP came at
    /// the next vertical blank (so one `work` is exactly a frame after the last, give or take
    /// the wait loop's seven cycles and an instruction), and no command was lost.
    fn assert_fifty(&self) {
        let m = &self.meter;
        assert!(m.frames > 0);
        assert!(m.worst < BUDGET, "the worst frame's work took {} cycles", m.worst);
        assert!(m.latest_done < BUDGET, "the chip finished {} cycles after work began", m.latest_done);
        assert!(
            m.longest < BUDGET + 16 && m.shortest > BUDGET - 16,
            "a frame took from {} to {} cycles: not one FLIP every vertical blank",
            m.shortest,
            m.longest
        );
        assert_eq!(self.get("LATE"), 0, "the game's own count of late frames");
        assert_eq!(m.lost, 0, "commands lost");
        assert!(m.deepest < niece::QUEUE, "the queue filled");
    }

    /// The keys a careful walker would hold this frame, going right: RIGHT always, and SPACE
    /// from the moment the ground ahead stops (a step down he walks off) or a wall stands in
    /// the way until he's down again. It reads the map as the game does.
    fn walker(&mut self, jumping: &mut bool) {
        self.m.key(RIGHT);
        let (px, py) = (self.px(), self.py());
        let grounded = self.get("GROUND") == 1;
        if grounded && *jumping {
            // let go, so the next press is a new one
            self.m.key_up(SPACE);
            *jumping = false;
            return;
        }
        if !grounded {
            return;
        }
        let ahead = (px + 8) >> 3;
        let feet = (py + 12) >> 3;
        // nothing ahead to stand on, here or a step or two down
        let gap = (feet..feet + 3).all(|r| self.flags(ahead, r) & (SOLID | ONEWAY) == 0);
        let wall = (py >> 3..=(py + 11) >> 3).any(|r| self.flags((px + 9) >> 3, r) & SOLID != 0);
        if gap || wall {
            self.m.key(SPACE);
            *jumping = true;
        }
    }

    /// Walks the level to the hut with `walker`, lives topped up so a lemming can't end it.
    /// Returns the frames it took.
    fn walk_to_the_hut(&mut self, max: u32) -> u32 {
        let mut jumping = false;
        for f in 0..max {
            if self.get("STATE") == CLEAR {
                self.m.key_up(RIGHT);
                self.m.key_up(SPACE);
                return f;
            }
            self.set("LIVES", 3);
            self.walker(&mut jumping);
            self.frame();
        }
        panic!("he never reached the hut: at {} of the level, y {}, state {}\n{}", self.px(), self.py(), self.get("STATE"), self.text());
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

/// The picture as the colour monitor shows it, as a PPM.
fn shot(g: &Game, path: &str) {
    use unc_core::machine::{NIECE_PIXEL, PIXELS_H, PIXELS_W};
    const PALETTE: [u32; 16] = [
        0x000000, 0xe8e4da, 0x9a3b2e, 0x5fb3a8, 0x7a3f8a, 0x4f9a4f, 0x2c3d9a, 0xe2c84a, 0xa8632a, 0x5a3e1a, 0xd0705f, 0x3a3a3a, 0x6b6b6b,
        0x9fe08a, 0x6f8fd8, 0xa9a9a9,
    ];
    let mut out = format!("P6 {PIXELS_W} {PIXELS_H} 255\n").into_bytes();
    let f = g.m.frame();
    for y in 0..PIXELS_H {
        for x in 0..PIXELS_W {
            let p = f[y * PIXELS_W + x];
            let c = if p & NIECE_PIXEL != 0 { p & 15 } else { g.m.line_colours(y)[(p & 3) as usize] };
            out.extend_from_slice(&PALETTE[(c & 15) as usize].to_be_bytes()[1..]);
        }
    }
    std::fs::write(path, out).unwrap();
}

/// The whole sheet, 256 x 512, as a PPM.
fn sheet(g: &Game, path: &str) {
    const PALETTE: [u32; 16] = [
        0x000000, 0xe8e4da, 0x9a3b2e, 0x5fb3a8, 0x7a3f8a, 0x4f9a4f, 0x2c3d9a, 0xe2c84a, 0xa8632a, 0x5a3e1a, 0xd0705f, 0x3a3a3a, 0x6b6b6b,
        0x9fe08a, 0x6f8fd8, 0xa9a9a9,
    ];
    let mut out = b"P6 256 512 255\n".to_vec();
    for y in 0..512 {
        for x in 0..256 {
            out.extend_from_slice(&PALETTE[g.chip().pixel(x, y) as usize].to_be_bytes()[1..]);
        }
    }
    std::fs::write(path, out).unwrap();
}

// ---------- loading ----------

#[test]
fn it_assembles_and_says_which_machine_it_wants() {
    let asm = assemble();
    let (lo, hi) = asm
        .segments
        .iter()
        .filter(|s| !s.bytes.is_empty())
        .fold((u16::MAX, 0u16), |(lo, hi), s| (lo.min(s.start), hi.max(s.start + s.bytes.len() as u16 - 1)));
    eprintln!("program ${lo:04X}-${hi:04X}, {} bytes", hi - lo + 1);
    assert_eq!(lo, 0x0800, "it loads at $0800 like the others");
    assert!(asm.symbol("RAMEND").unwrap() < 0xC000);
    // the build script reads the first three lines to pick the machine
    let head: Vec<&str> = SRC.lines().take(3).collect();
    assert!(head.iter().all(|l| l.starts_with(';')), "{head:?}");
    assert!(head.join("\n").contains("64K"), "{head:?}");
    assert!(!SRC.contains(".include"), "one file");
}

#[test]
fn without_the_niece_it_says_so_and_waits() {
    let mut g = Game::boot_niece(false);
    for _ in 0..50 {
        assert!(!matches!(g.m.run_frame(), Stop::Jammed(_)));
    }
    let text = g.text();
    assert!(text.contains("CLOUDBERRY NEEDS THE NIECE."), "{text}");
    assert!(text.contains("SWITCH OFF, FIT IT, TRY AGAIN."));
    assert_eq!(g.m.cpu.state, CpuState::Running, "waiting, not halted or jammed");
    g.m.key(SPACE);
    for _ in 0..10 {
        g.m.run_frame();
    }
    assert!(g.text().contains("CLOUDBERRY NEEDS THE NIECE."));
    assert_eq!(g.m.cpu.state, CpuState::Running);
}

// ---------- the sheet ----------

#[test]
fn the_sheet_holds_the_tiles_and_three_backdrops_that_join_up() {
    let g = Game::boot();
    let n = g.chip();
    let (tiley, felly, cloudy, neary, ringy) = (g.sym("TILEY") as i32, g.sym("FELLY") as i32, g.sym("CLOUDY") as i32, g.sym("NEARY") as i32, g.sym("RINGY") as i32);
    // tile 0 is nothing; tile 1's top line is grass, as the source draws it
    for y in 0..8 {
        for x in 0..8 {
            assert_eq!(n.pixel(x, tiley + y), KEYC);
        }
    }
    let grass: Vec<u8> = (0..8).map(|x| n.pixel(8 + x, tiley)).collect();
    assert_eq!(grass, [13, 5, 13, 13, 5, 13, 5, 13], "D5DD5D5D");
    // the fells: every column as high as the higher of its two ridges, nothing above
    let high = |top: i32, h: i32, x: i32| (0..h).filter(|&y| n.pixel(x, top + y) != KEYC).count() as i32;
    for x in 0..256 {
        let want = g.at("fellh", x as u16).max(g.at("fell2h", x as u16)) as i32;
        assert_eq!(high(felly, 32, x), want, "the fells' column {x}");
        assert_ne!(n.pixel(x, felly + 31), KEYC);
        // the hills: at least their height (a tree stands higher), solid from the bottom
        let h = g.at("nearh", x as u16) as i32;
        assert!(high(neary, 48, x) >= h, "the hills' column {x}");
        assert!((48 - h..48).all(|y| n.pixel(x, neary + y) != KEYC));
    }
    // each joins up with itself: the last column is next to the first
    for name in ["fellh", "fell2h", "nearh"] {
        let (a, b) = (g.at(name, 255) as i32, g.at(name, 0) as i32);
        assert!((a - b).abs() <= 2, "{name}: {a} then {b}");
    }
    // no tree or cloud is cut off at the strip's right edge
    assert!((0..48).all(|y| n.pixel(255, neary + y) == n.pixel(254, neary + y) || y > 16), "a tree at the edge");
    assert!((0..20).all(|y| n.pixel(255, cloudy + y) == KEYC && n.pixel(0, cloudy + y) == KEYC));
    assert!((0..256).any(|x| n.pixel(x, cloudy + 10) == 1), "clouds");
    // the ring holds the level's first 21 columns, a tile each
    for col in 0..21 {
        for row in 0..12 {
            let t = g.tile(col, row) as i32;
            for (x, y) in [(0, 0), (3, 5), (7, 7)] {
                assert_eq!(n.pixel(col * 8 + x, ringy + row * 8 + y), n.pixel((t & 31) * 8 + x, tiley + (t >> 5) * 8 + y), "column {col}, row {row}");
            }
        }
    }
}

#[test]
fn a_level_is_walled_at_both_ends_and_its_things_are_in_order() {
    let mut g = Game::play();
    for level in 0..2 {
        if level == 1 {
            g.to_level_two();
        }
        assert_eq!(g.get("LEVEL"), level);
        for row in 0..12 {
            assert_eq!(g.flags(0, row), SOLID);
            assert_eq!(g.flags(255, row), SOLID);
        }
        let n = (0..128).filter(|&i| g.at("ETYPE", i) != 0).count();
        assert!((30..120).contains(&n), "{n} things");
        // in order of the 256 pixels they start in, and EFIRST says where each begins
        let mut last = 0;
        for i in 0..n as u16 {
            let chunk = g.at("EXH", i);
            assert!(chunk >= last);
            assert!(g.at("EFIRST", chunk as u16) as u16 <= i && i < g.at("EFIRST", chunk as u16 + 1) as u16);
            last = chunk;
        }
        assert_eq!(g.at("EFIRST", 8) as usize, n);
        let berries = g.things(E_BERRY).len() as u32;
        assert_eq!(g.get("NBERRY") as u32 * 10 + g.at("NBERRY", 1) as u32, berries);
        eprintln!("level {}: {berries} berries, {} lemmings, {} clouds of midges", level + 1, g.things(E_LEM).len(), g.things(E_MIDGE).len());
        assert!(berries >= 30 && g.things(E_LEM).len() >= 4 && g.things(E_MIDGE).len() >= 2);
        // a hut, with a door, near the far end
        let door = (0..256).find(|&c| g.flags(c, 8) == 0x10).expect("a door");
        assert!(door > 230);
    }
    // the level is 256 columns: 2,048 pixels, 12.8 pages
    assert_eq!(g.sym("CAMMAX"), 2048 - 160);
}

// ---------- the title ----------

#[test]
fn the_title_wanders_along_the_level_and_space_starts_a_game() {
    let mut g = Game::boot();
    let text = g.text();
    assert!(text.contains("C L O U D B E R R Y"), "{text}");
    assert!(text.contains("PRESS SPACE"));
    g.to_work();
    let c0 = g.cam();
    for i in 1..=300 {
        g.frame();
        g.to_work();
        assert_eq!(g.cam(), c0 + i, "the camera goes a pixel a frame");
    }
    g.report("the title");
    g.assert_fifty();
    // every BLIT of the level takes from the camera's low byte: past 256 it has gone round
    assert!(g.cam() > 256);
    assert_eq!(g.backdrop()[3].sx, g.cam() as u8);
    g.press(SPACE);
    g.until(60, "SPACE starts a game", |g| g.get("STATE") == PLAY);
    g.frames(2);
    let text = g.text();
    assert!(text.lines().next().unwrap().starts_with(" BERRIES 00/38  LIVES 3  TIME 2"), "{text}");
    assert!(text.contains("1: THE BOG AND THE BIRCHES"));
    assert!(!text.contains("PRESS SPACE"));
    assert_eq!((g.get("LIVES"), g.score(), g.cam()), (3, 0, 0));
    // the level's name goes after a couple of seconds
    g.frames(130);
    assert!(!g.text().contains("THE BOG"));
    assert_eq!(g.get("LATE"), 0);
}

// ---------- running and jumping ----------

/// Where he stands on level 1's first stretch of ground: its top is row 9.
const GROUND_Y: i32 = 9 * 8 - 12;

#[test]
fn he_runs_a_pixel_a_frame_and_slows_to_a_stop() {
    let mut g = Game::play();
    assert_eq!((g.py(), g.get("GROUND")), (GROUND_Y, 1), "he starts on the grass");
    g.m.key(RIGHT);
    g.frames(4);
    assert!(g.vx() > 0 && g.vx() < 256, "he gathers speed: {}", g.vx());
    g.frames(20);
    assert_eq!(g.vx(), 256, "and tops out at a pixel a frame exactly");
    let x0 = g.px();
    for i in 1..=30 {
        g.frame();
        assert_eq!(g.px(), x0 + i);
    }
    assert_eq!(g.picker().unwrap().flags & F_FLIPX, 0);
    g.m.key_up(RIGHT);
    g.frames(12);
    assert_eq!(g.vx(), 0);
    let x1 = g.px();
    assert!(x1 > x0 + 30 && x1 < x0 + 40, "he slid {} pixels to a stop", x1 - x0 - 30);
    g.frames(10);
    assert_eq!(g.px(), x1);
    // and the other way, with A and D as well as the cursor keys
    g.hold(b'A', 30);
    assert!(g.px() < x1 - 20);
    assert_eq!(g.get("PFACE"), F_FLIPX);
    assert_eq!(g.picker().unwrap().flags & F_FLIPX, F_FLIPX, "drawn mirrored");
    g.hold(b'D', 10);
    assert_eq!(g.get("PFACE"), 0);
    // his pictures change as he runs: more than one of them in a second of it
    g.m.key(RIGHT);
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..50 {
        g.frame();
        seen.insert(g.picker().unwrap().sx);
    }
    assert!(seen.len() >= 3, "{seen:?}");
}

/// Presses jump for `frames` frames and returns how high his feet got, in pixels.
fn jump(g: &mut Game, key: u8, frames: u32) -> i32 {
    let y0 = g.py();
    let mut top = y0;
    g.m.key(key);
    for f in 0..80 {
        if f == frames {
            g.m.key_up(key);
        }
        g.frame();
        top = top.min(g.py());
        if f > frames && g.get("GROUND") == 1 {
            break;
        }
    }
    assert_eq!((g.py(), g.get("GROUND")), (y0, 1), "down again where he was");
    y0 - top
}

#[test]
fn a_tap_is_a_hop_and_a_held_key_is_a_leap() {
    let mut g = Game::play();
    let hop = jump(&mut g, SPACE, 2);
    let leap = jump(&mut g, SPACE, 40);
    let middling = jump(&mut g, UP, 8);
    eprintln!("a tap {hop} pixels, eight frames {middling}, held {leap}");
    assert!((6..=12).contains(&hop), "{hop}");
    assert!((24..=28).contains(&leap), "{leap}: three tiles and a little");
    assert!(hop < middling && middling < leap);
    assert_eq!(jump(&mut g, b'W', 40), leap, "W jumps too");
    // holding the key down doesn't jump again on landing
    g.m.key(SPACE);
    g.frames(120);
    g.m.key_up(SPACE);
    assert_eq!((g.py(), g.get("GROUND")), (GROUND_Y, 1));
}

#[test]
fn a_press_just_before_landing_and_one_just_after_the_edge_both_count() {
    let mut g = Game::play();
    // the first pool is columns 42-44: the grass stops at x = 336
    g.stand(318, GROUND_Y);
    g.m.key(RIGHT);
    g.until(60, "he runs off the edge", |g| g.get("GROUND") == 0);
    assert!(g.px() >= 336);
    g.frames(3);
    assert!(g.vy() > 0, "falling");
    g.m.key(SPACE);
    g.frame();
    assert!(g.vy() < -500, "coyote time: {}", g.vy());
    g.until(80, "he lands on the far side", |g| g.get("GROUND") == 1);
    assert!(g.px() > 360 && g.get("LIVES") == 3);
    g.m.key_up(SPACE);
    g.m.key_up(RIGHT);
    g.frames(20);
    // but not ten frames after it
    g.stand(318, GROUND_Y);
    g.m.key(RIGHT);
    g.until(60, "he runs off the edge", |g| g.get("GROUND") == 0);
    g.frames(10);
    g.m.key(SPACE);
    g.frame();
    assert!(g.vy() > 0, "too late");
    g.m.key_up(SPACE);
    g.m.key_up(RIGHT);
    g.until(200, "he comes back", |g| g.get("STATE") == PLAY && g.get("GROUND") == 1);
    // a press in the air, four frames before he lands, jumps as he lands
    g.frames(10);
    let _ = jump(&mut g, SPACE, 40);
    g.m.key(SPACE);
    g.frames(25);
    g.m.key_up(SPACE);
    g.until(60, "nearly down", |g| g.vy() > 0 && g.py() >= GROUND_Y - 6);
    assert_eq!(g.get("GROUND"), 0);
    g.m.key(SPACE);
    g.until(20, "he lands and jumps", |g| g.vy() < -500);
    g.m.key_up(SPACE);
}

#[test]
fn he_stands_on_the_ground_and_on_a_duckboard_he_came_up_through() {
    let mut g = Game::play();
    // level 1's first duckboard: columns 22-26 of row 6
    let (col, row) = (24, 6);
    assert_eq!(g.flags(col, row), ONEWAY);
    assert_eq!(g.flags(col, 9), SOLID);
    g.stand(col * 8, GROUND_Y);
    g.m.key(SPACE);
    g.until(40, "up through it", |g| g.py() + 12 < row * 8);
    g.until(40, "and down onto it", |g| g.get("GROUND") == 1);
    g.m.key_up(SPACE);
    assert_eq!(g.py(), row * 8 - 12, "standing on the duckboard");
    g.frames(30);
    assert_eq!((g.py(), g.get("GROUND")), (row * 8 - 12, 1));
    // jumping from it works like anywhere
    assert!(jump(&mut g, SPACE, 40) >= 24);
    // off its end, down to the grass
    g.m.key(RIGHT);
    g.until(80, "off the end", |g| g.py() == GROUND_Y && g.get("GROUND") == 1);
    g.m.key_up(RIGHT);
    assert!(g.px() >= 27 * 8);
}

#[test]
fn the_cliff_and_the_rocks_stop_him_and_his_head_stops_at_a_rock() {
    let mut g = Game::play();
    g.hold(LEFT, 60);
    assert_eq!((g.px(), g.vx()), (8, 0), "the cliff at the level's left end");
    // the outcrop in the third piece: a step two tiles high at column 72
    assert_eq!((g.flags(72, 7), g.flags(72, 8), g.flags(71, 8)), (SOLID, SOLID, 0));
    g.stand(72 * 8 - 30, GROUND_Y);
    g.hold(RIGHT, 60);
    assert_eq!((g.px(), g.py()), (72 * 8 - 6, GROUND_Y), "his right edge against it");
    g.frames(10);
    assert_eq!((g.px(), g.vx()), (72 * 8 - 6, 0));
    // a jump takes him up onto it, pushing against it all the way
    g.m.key(RIGHT);
    g.m.key(SPACE);
    g.until(60, "onto the step", |g| g.get("GROUND") == 1 && g.py() == 7 * 8 - 12);
    g.m.key_up(SPACE);
    g.m.key_up(RIGHT);
    assert!(g.px() > 72 * 8 - 6);
    // under the outcrop's overhang... there isn't one in level 1, so make a roof: rock over
    // his head, two tiles up, and a jump stops at it
    g.stand(40, GROUND_Y);
    let map = g.sym("MAP");
    for c in 4..7 {
        g.m.poke(map + 5 * 256 + c, 4);
    }
    g.m.key(SPACE);
    let mut top = g.py();
    for _ in 0..30 {
        g.frame();
        top = top.min(g.py());
    }
    g.m.key_up(SPACE);
    assert_eq!(top, 6 * 8, "his head against the rock's underside");
}

// ---------- berries and creatures ----------

#[test]
fn a_berry_picked_is_fifty_points() {
    let mut g = Game::play();
    let berries = g.things(E_BERRY);
    let i = berries[0];
    for &other in &berries[1..] {
        g.set_at("ETYPE", other, 0);
    }
    let (x, y) = (g.thing_x(i), g.at("EY", i) as i32);
    g.stand(x - 40, GROUND_Y);
    g.frames(5);
    assert_eq!((g.berries(), g.score()), (0, 0), "not from 40 pixels away");
    assert_eq!(g.at("ETYPE", i), E_BERRY);
    // it bobs: it isn't drawn on the same line all the time
    let lines: std::collections::BTreeSet<i16> = (0..40)
        .map(|_| {
            g.frame();
            g.frame_commands.iter().find(|c| c.sw == 8 && c.dw == 8 && c.flags & F_SHEET == 0).expect("the berry's BLIT").dy
        })
        .collect();
    assert!(lines.len() >= 3, "{lines:?}");
    g.put(x + 1, y - 4);
    g.frames(2);
    assert_eq!(g.at("ETYPE", i), 0, "picked");
    assert_eq!((g.berries(), g.score()), (1, 50));
    assert!(g.text().starts_with(" BERRIES 01/38"), "{}", g.text());
    assert!(g.things(E_BERRY).is_empty());
}

#[test]
fn a_lemming_walks_to_and_fro_bites_and_is_flattened_by_a_jump() {
    let mut g = Game::play();
    let i = g.things(E_LEM)[0];
    let y = g.at("EY", i) as i32;
    assert_eq!(y, 64, "on the grass");
    // watch it going left: it turns round at the pool's edge and never leaves the grass
    g.set_at("ES", i, F_FLIPX);
    g.stand(g.thing_x(i) - 20, GROUND_Y - 30);
    g.set("INVUL", 0);
    let (mut lo, mut hi) = (i32::MAX, 0);
    for _ in 0..400 {
        g.put(g.cam() + 60, GROUND_Y - 30);
        g.frame();
        let x = g.thing_x(i);
        (lo, hi) = (lo.min(x), hi.max(x));
        assert!(g.flags(x >> 3, 9) != 0 && g.flags((x + 11) >> 3, 9) != 0, "a lemming over the edge at {x}");
    }
    assert_eq!(lo, 45 * 8, "it turned at the pool's edge");
    assert!(hi - lo > 60, "it walked from {lo} to {hi}");
    // walking into it: a life, a knock back, and a while he can't be hurt and flashes white
    let x = g.thing_x(i);
    g.put(x, GROUND_Y);
    g.frames(2);
    assert_eq!(g.get("LIVES"), 2);
    assert!(g.get("INVUL") > 90 && g.vy() < 0 && g.vx() != 0);
    let mut white = 0;
    for _ in 0..60 {
        g.frame();
        white += (g.picker().unwrap().flags & F_WHITE != 0) as u32;
        assert_eq!(g.get("LIVES"), 2, "not twice");
    }
    assert!((20..=40).contains(&white), "{white} frames in 60 drawn through the white ink table");
    assert_eq!(g.chip().inks[1], [1; 16]);
    g.frames(60);
    assert_eq!(g.get("INVUL"), 0);
    g.put(g.thing_x(i) - 50, GROUND_Y);
    g.frames(2);
    assert_eq!(g.picker().unwrap().flags & F_WHITE, 0);
    // coming down on it: flat, a hundred points and a bounce
    let score = g.score();
    g.put(g.thing_x(i) + 3, y - 22);
    g.until(30, "he lands on it", |g| g.at("ETYPE", i) == E_FLAT);
    assert_eq!(g.score(), score + 100);
    assert!(g.vy() < 0, "a bounce");
    assert_eq!(g.get("LIVES"), 2);
    g.until(30, "it's gone", |g| g.at("ETYPE", i) == 0);
}

#[test]
fn midges_hang_in_the_air_and_cost_a_life_from_any_side() {
    let mut g = Game::play();
    let i = g.things(E_MIDGE)[0];
    let (x, y) = (g.thing_x(i), g.at("EY", i) as i32);
    g.stand(x - 40, GROUND_Y);
    // they sway: their BLIT moves both ways
    let (mut xs, mut ys) = (std::collections::BTreeSet::new(), std::collections::BTreeSet::new());
    for _ in 0..120 {
        g.frame();
        let c = g.frame_commands.iter().find(|c| c.sw == 12 && c.sh == 12 && c.flags & F_SHEET == 0).expect("the midges' BLIT");
        xs.insert(c.dx);
        ys.insert(c.dy);
    }
    assert!(xs.len() >= 5 && ys.len() >= 5, "{xs:?} {ys:?}");
    assert_eq!((g.thing_x(i), g.at("EY", i) as i32), (x, y), "but they stay where they are");
    assert_eq!(g.get("LIVES"), 3);
    // falling into them from above is no better than walking into them
    g.put(x + 3, y - 24);
    g.until(40, "into the midges", |g| g.get("LIVES") == 2);
    assert_eq!(g.at("ETYPE", i), E_MIDGE);
}

// ---------- losing and winning ----------

#[test]
fn the_bog_costs_a_life_and_he_comes_back_where_he_last_stood() {
    let mut g = Game::play();
    g.stand(300, GROUND_Y);
    g.m.key(RIGHT);
    g.until(100, "into the pool", |g| g.get("STATE") == DYING);
    g.m.key_up(RIGHT);
    assert_eq!(g.get("LIVES"), 2);
    g.until(100, "back", |g| g.get("STATE") == PLAY);
    g.until(100, "on the grass", |g| g.get("GROUND") == 1);
    assert!((320..=330).contains(&g.px()), "at the edge he ran off: {}", g.px());
    assert_eq!(g.py(), GROUND_Y);
    assert!(g.get("INVUL") > 0);
    assert!(g.text().contains("LIVES 2"));
    assert_eq!(g.get("LATE"), 0);
}

#[test]
fn the_last_life_is_game_over_and_space_starts_again() {
    let mut g = Game::play();
    g.set("LIVES", 1);
    g.stand(300, GROUND_Y);
    g.hold(RIGHT, 60);
    g.until(200, "game over", |g| g.get("STATE") == OVER);
    let text = g.text();
    assert!(text.contains("GAME OVER") && text.contains("SPACE FOR ANOTHER WALK"), "{text}");
    assert!(text.contains("LIVES 0"));
    g.frames(30);
    assert_eq!(g.get("STATE"), OVER);
    g.press(SPACE);
    g.until(60, "another game", |g| g.get("STATE") == PLAY);
    g.frames(3);
    assert_eq!((g.get("LIVES"), g.score(), g.berries(), g.get("LEVEL")), (3, 0, 0, 0));
    assert_eq!(g.things(E_BERRY).len(), 38, "the berries are back");
    assert!(!g.text().contains("GAME OVER"));
    // a lemming's bite on the last life knocks him off the page
    g.set("LIVES", 1);
    let i = g.things(E_LEM)[0];
    g.stand(g.thing_x(i) - 30, GROUND_Y - 30);
    g.put(g.thing_x(i), GROUND_Y);
    g.until(10, "bitten", |g| g.get("STATE") == DYING);
    g.until(200, "game over", |g| g.get("STATE") == OVER);
    // and left alone, game over goes back to the title
    g.until(600, "the title", |g| g.get("STATE") == TITLE);
    assert!(g.text().contains("C L O U D B E R R Y"));
    assert!(!g.text().contains("BERRIES"));
    assert_eq!(g.get("LATE"), 0);
}

#[test]
fn the_hut_ends_the_level_and_the_second_hut_ends_the_game() {
    let mut g = Game::play();
    // the hut's door: the tiles with GOAL
    let door = (0..256).find(|&c| g.flags(c, 8) == 0x10).expect("a door");
    g.stand(door * 8 - 40, GROUND_Y);
    assert_eq!(g.get("STATE"), PLAY);
    g.m.key(RIGHT);
    g.until(100, "the hut", |g| g.get("STATE") == CLEAR);
    g.m.key_up(RIGHT);
    let (score, time) = (g.score(), g.time());
    assert!(time > 200);
    assert!(g.text().contains("THE HUT, AND A FULL PAIL"), "{}", g.text());
    // the time left is counted into the score, ten points a second
    g.until(300, "the count", |g| g.time() == 0);
    assert_eq!(g.score(), score + time * 10);
    g.frames(3);
    assert!(g.text().contains("PRESS SPACE"));
    g.press(SPACE);
    g.until(60, "level 2", |g| g.get("STATE") == PLAY);
    g.frames(3);
    assert_eq!(g.get("LEVEL"), 1);
    assert!(g.text().contains("2: OVER THE HIGH STONES"), "{}", g.text());
    assert_eq!(g.score(), score + time * 10, "the score goes with him");
    assert_eq!((g.cam(), g.berries(), g.time()), (0, 0, 250));
    // its sky is the evening's, and its fells go through ink table 2
    assert_eq!(g.get("SKYF"), 0x81);
    assert_ne!(g.chip().inks[2], g.chip().inks[0]);
    g.until(100, "he lands", |g| g.get("GROUND") == 1);
    let door = (0..256).find(|&c| g.flags(c, 8) == 0x10).expect("a door");
    g.stand(door * 8 - 40, GROUND_Y);
    g.hold(RIGHT, 60);
    assert_eq!(g.get("STATE"), CLEAR);
    assert!(g.text().contains("HOME. JAM FOR THE WINTER"), "{}", g.text());
    g.until(300, "the count", |g| g.time() == 0);
    g.frames(3);
    g.press(SPACE);
    g.until(60, "the title", |g| g.get("STATE") == TITLE);
    assert_eq!(g.get("LEVEL"), 0);
    assert_eq!(g.get("LATE"), 0);
}

// ---------- the scroll ----------

#[test]
fn the_page_is_the_level_a_pixel_on_for_every_pixel_of_the_camera() {
    // Walking right through four pieces of level 1, jumping what's in the way, with the
    // things taken out so that the page is the backdrops, the level and him. Every frame's
    // page is compared, all of it but his own rectangle, with the picture worked out again
    // from the map and the sheet for where the camera says it is.
    let mut g = Game::play();
    g.no_things();
    g.frames(2);
    g.to_work();
    g.assert_page("standing at the start");
    let (mut jumping, mut last, mut ones) = (false, g.cam(), 0);
    let mut columns = std::collections::BTreeSet::new();
    for f in 0..1100 {
        g.walker(&mut jumping);
        g.frame();
        g.to_work();
        let cam = g.cam();
        assert!((0..=2).contains(&(cam - last)), "frame {f}: the camera went from {last} to {cam}");
        ones += (cam - last == 1) as u32;
        columns.insert(cam >> 3);
        last = cam;
        g.assert_page(&format!("frame {f}"));
        // the level's BLIT is one command, from the camera's x modulo 256
        let b = g.backdrop();
        assert_eq!(b.len(), 4);
        assert_eq!((b[3].sx, b[3].sy, b[3].sw, b[3].sh, b[3].dx, b[3].dy), (cam as u8, g.sym("RINGY"), 160, 96, 0, 4));
        assert_eq!(b[3].flags, F_KEY);
        assert_eq!(g.get("STATE"), PLAY);
    }
    eprintln!("the camera got to {last}: {} new columns, and moved a pixel in {ones} of 1100 frames", columns.len());
    assert!(last > 900, "three and a half times round the ring: {last}");
    assert!(columns.len() > 110);
    assert!(ones > 950, "{ones}");
    assert_eq!(g.get("LATE"), 0);
}

#[test]
fn the_ring_is_right_going_left_and_when_the_camera_hurries() {
    let mut g = Game::play();
    g.no_things();
    let mut jumping = false;
    for _ in 0..520 {
        g.walker(&mut jumping);
        g.frame();
    }
    g.m.key_up(RIGHT);
    g.m.key_up(SPACE);
    g.until(100, "down", |g| g.get("GROUND") == 1);
    g.to_work();
    let far = g.cam();
    assert!(far > 256);
    // back left: the camera swings round to lead him the other way, two pixels a frame, and
    // then goes with him
    g.m.key(LEFT);
    let mut last = far;
    for f in 0..150 {
        g.frame();
        g.to_work();
        let cam = g.cam();
        assert!((0..=2).contains(&(last - cam)), "frame {f}: the camera went from {last} to {cam}");
        last = cam;
        g.assert_page(&format!("going left, frame {f}"));
    }
    g.m.key_up(LEFT);
    assert!(last < far - 150 && last < 256 && far > 256, "back across the ring's join: {far} to {last}");
    // put down half a level away, the camera comes at six pixels a frame, a new column
    // nearly every frame, and the page is right all the way
    g.set("PXL", (1200 & 255) as u8);
    g.set("PXH", (1200 >> 8) as u8);
    g.set("PY", 20);
    g.set("CAMSPD", 6);
    let mut most = 0;
    for f in 0..250 {
        g.set("PY", 20);
        g.set("VYL", 0);
        g.set("VYH", 0);
        g.frame();
        g.to_work();
        most = most.max(g.cam() - last);
        last = g.cam();
        g.assert_page(&format!("hurrying, frame {f}"));
    }
    assert_eq!(most, 6);
    assert_eq!(last, 1200 - 60, "and stops with him where it likes him");
    assert_eq!(g.get("CAMSPD"), 2);
    assert_eq!(g.get("LATE"), 0);
    assert!(!g.chip().lost);
}

#[test]
fn the_backdrops_go_by_at_an_eighth_a_quarter_and_a_half_of_the_level() {
    let mut g = Game::play();
    g.no_things();
    let mut jumping = false;
    let (neary, felly, cloudy) = (g.sym("NEARY"), g.sym("FELLY"), g.sym("CLOUDY"));
    let mut first: Option<(i32, [i32; 4])> = None;
    let mut span = [0i32; 4];
    for _ in 0..600 {
        g.walker(&mut jumping);
        g.frame();
        g.to_work();
        let (cam, tick) = (g.cam(), g.word("TICK", "TICKH"));
        let b = g.backdrop();
        assert_eq!([b[0].sy, b[1].sy, b[2].sy], [cloudy, felly, neary]);
        assert_eq!(b[0].sx, ((cam >> 3) + (tick >> 4)) as u8, "the clouds: an eighth, and their own drift");
        assert_eq!(b[1].sx, (cam >> 2) as u8, "the fells: a quarter");
        assert_eq!(b[2].sx, (cam >> 1) as u8, "the hills: a half");
        assert_eq!(b[3].sx, cam as u8, "the level");
        assert!(b.iter().all(|c| c.flags & F_KEY != 0 && c.key == KEYC && c.sw == 160));
        let now = [cam >> 3, cam >> 2, cam >> 1, cam];
        match first {
            None => first = Some((cam, now)),
            Some((_, was)) => span = [now[0] - was[0], now[1] - was[1], now[2] - was[2], now[3] - was[3]],
        }
    }
    eprintln!("the level moved {} pixels, the hills {}, the fells {}, the clouds {} and their drift", span[3], span[2], span[1], span[0]);
    assert!(span[3] > 400);
    assert!((span[2] * 2 - span[3]).abs() <= 2 && (span[1] * 4 - span[3]).abs() <= 4 && (span[0] * 8 - span[3]).abs() <= 8);
}

#[test]
fn the_camera_leads_him_and_never_jitters() {
    let mut g = Game::play();
    g.no_things();
    // running right on the flat: once the camera has caught up he's at column 60 of the page,
    // a hundred pixels of level ahead of him, and stays there to the pixel
    g.m.key(RIGHT);
    g.frames(120);
    g.to_work();
    let mut last = g.cam();
    for _ in 0..100 {
        g.frame();
        g.to_work();
        assert_eq!(g.px() - g.cam(), 60);
        assert_eq!(g.cam() - last, 1, "a pixel a frame, every frame");
        assert_eq!(g.picker().unwrap().dx, 60 - 3);
        last = g.cam();
    }
    g.m.key_up(RIGHT);
    g.frames(30);
    // standing still, nothing moves
    g.to_work();
    let cam = g.cam();
    g.frames(30);
    g.to_work();
    assert_eq!(g.cam(), cam);
    // turning round: the camera swings to put the room on his other side, smoothly: never
    // back the way it came, never more than two pixels in a frame
    g.m.key(LEFT);
    let mut last = cam;
    for _ in 0..80 {
        g.frame();
        g.to_work();
        let d = last - g.cam();
        assert!((0..=2).contains(&d), "{d}");
        last = g.cam();
    }
    assert_eq!(g.px() - g.cam(), 94, "and now he's right of the middle");
    g.m.key_up(LEFT);
    // at the level's ends the camera stops and he walks on
    g.hold(LEFT, 400);
    g.to_work();
    assert_eq!((g.cam(), g.px()), (0, 8));
}

#[test]
fn he_stretches_as_he_leaves_the_ground_and_squashes_as_he_lands() {
    let mut g = Game::play();
    g.frames(10);
    let c = g.picker().unwrap();
    assert_eq!((c.dw, c.dh, c.sw, c.sh), (12, 14, 12, 14), "as he is");
    let feet = c.dy + c.dh as i16;
    g.m.key(SPACE);
    g.frames(3);
    let c = g.picker().unwrap();
    assert_eq!((c.dw, c.dh, c.sw, c.sh), (10, 16, 12, 14), "taller and thinner: the same picture, squeezed");
    g.frames(30);
    let c = g.picker().unwrap();
    assert_eq!((c.dw, c.dh), (12, 14));
    g.m.key_up(SPACE);
    g.until(40, "he lands", |g| g.get("GROUND") == 1);
    g.frame();
    let c = g.picker().unwrap();
    assert_eq!((c.dw, c.dh), (14, 12), "shorter and wider");
    assert_eq!(c.dy + c.dh as i16, feet, "with his feet where they were");
    g.frames(10);
    assert_eq!(g.picker().unwrap().dw, 12);
}

// ---------- fifty frames a second ----------

#[test]
fn the_whole_of_level_one_runs_at_fifty_frames_a_second() {
    let mut g = Game::play();
    let frames = g.walk_to_the_hut(4000);
    g.frames(200);
    g.report("level 1, start to hut");
    eprintln!("level 1: {frames} frames to the hut, {} berries on the way, {} points", g.berries(), g.score());
    assert!(g.berries() >= 10);
    g.assert_fifty();
}

#[test]
fn the_whole_of_level_two_runs_at_fifty_frames_a_second() {
    let mut g = Game::play();
    g.to_level_two();
    let frames = g.walk_to_the_hut(4000);
    g.frames(200);
    g.report("level 2, start to hut");
    eprintln!("level 2: {frames} frames to the hut, {} berries on the way, {} points", g.berries(), g.score());
    g.assert_fifty();
}

#[test]
fn the_busiest_page_of_each_level_runs_at_fifty_frames_a_second() {
    // Where the most things are on the page at once: stand there and jump about for ten
    // seconds, unhurt, with the things all still there.
    let mut g = Game::play();
    let all: Vec<(u16, i32)> = (0..128).filter(|&i| g.at("ETYPE", i) != 0).map(|i| (i, g.thing_x(i))).collect();
    let on_page = |cam: i32| all.iter().filter(|&&(_, x)| x >= cam - 8 && x < cam + 160).count();
    let cam = (0..1888).max_by_key(|&c| on_page(c)).unwrap();
    eprintln!("the busiest page: the camera at {cam}, {} things on it", on_page(cam));
    assert!(on_page(cam) >= 8);
    let kinds: Vec<u8> = (0..128).map(|i| g.at("ETYPE", i)).collect();
    g.stand(cam + 60, 20);
    g.frames(5);
    g.to_work();
    g.meter = Meter::default();
    let mut d = Dice(1985);
    for f in 0..500u32 {
        for (i, &k) in kinds.iter().enumerate() {
            g.set_at("ETYPE", i as u16, k);
        }
        g.set("LIVES", 3);
        if g.get("STATE") != PLAY {
            g.set("STATE", PLAY);
            g.set("HITS", 1);
        }
        if g.py() > 90 || (g.px() - cam - 60).abs() > 60 {
            g.set("PXL", ((cam + 60) & 255) as u8);
            g.set("PXH", ((cam + 60) >> 8) as u8);
            g.set("PY", 20);
        }
        if f % 16 == 0 {
            for k in [LEFT, RIGHT, SPACE] {
                g.m.key_up(k);
            }
            match d.next() % 4 {
                0 => g.m.key(LEFT),
                1 => g.m.key(RIGHT),
                _ => {}
            }
            if d.next() % 2 == 0 {
                g.m.key(SPACE);
            }
        }
        g.frame();
    }
    g.report("the busiest page");
    g.assert_fifty();
}

#[test]
fn a_long_game_with_random_keys_loses_no_command_and_no_frame() {
    // Random keys, held for random spells, through lost lives, game overs, the title and new
    // games. A level's unpacking holds the picture for a few frames and the game starts
    // counting again after it, so LATE is the frames that were late while it was playing.
    let mut g = Game::boot();
    let mut d = Dice(64);
    let (mut games, mut overs, mut furthest) = (0, 0, 0);
    let mut was = TITLE;
    for f in 0..15000u32 {
        if f % 12 == 0 {
            for k in [LEFT, RIGHT, SPACE] {
                g.m.key_up(k);
            }
            match d.next() % 8 {
                0 => g.m.key(LEFT),
                1 => {}
                _ => g.m.key(RIGHT),
            }
            if d.next() % 3 == 0 {
                g.m.key(SPACE);
            }
        }
        g.frame();
        let s = g.get("STATE");
        games += (s == PLAY && was != PLAY && was != DYING) as u32;
        overs += (s == OVER && was != OVER) as u32;
        was = s;
        furthest = furthest.max(g.px());
        assert!(!g.chip().lost, "frame {f}: a command was lost");
        assert_eq!(g.meter.lost, 0);
        assert_eq!(g.get("LATE"), 0, "frame {f}: a late frame, in state {s}");
        assert_eq!(g.m.cpu.state, CpuState::Running);
    }
    eprintln!("a long game: {games} games, {overs} game overs, as far as {furthest} of 2048");
    assert!(games >= 3 && overs >= 2);
}

/// Pictures of the game for a person to look at, as PPMs:
/// `CLOUDBERRY_SHOTS=dir cargo test --release -p unc-asm --test game_cloudberry pictures -- --ignored`.
#[test]
#[ignore]
fn pictures() {
    let dir = std::env::var("CLOUDBERRY_SHOTS").unwrap_or_else(|_| "target/cloudberry".into());
    std::fs::create_dir_all(&dir).unwrap();
    let mut g = Game::boot();
    sheet(&g, &format!("{dir}/0-sheet.ppm"));
    g.frames(120);
    shot(&g, &format!("{dir}/1-title.ppm"));
    g.press(SPACE);
    g.until(60, "play", |g| g.get("STATE") == PLAY);
    g.frames(60);
    shot(&g, &format!("{dir}/2-start.ppm"));
    let mut jumping = false;
    for f in 0..2600 {
        if g.get("STATE") == CLEAR {
            break;
        }
        g.set("LIVES", 3);
        g.walker(&mut jumping);
        g.frame();
        if f % 100 == 50 {
            shot(&g, &format!("{dir}/3-walk{:02}.ppm", f / 100));
        }
    }
    g.frames(30);
    shot(&g, &format!("{dir}/4-hut.ppm"));
    g.until(400, "the count", |g| g.get("CT") == 1);
    g.frames(2);
    g.press(SPACE);
    g.until(60, "level 2", |g| g.get("STATE") == PLAY && g.get("LEVEL") == 1);
    let mut last = 0;
    for f in 0..3000 {
        if g.get("STATE") == CLEAR {
            break;
        }
        g.set("LIVES", 3);
        g.walker(&mut jumping);
        g.frame();
        if f % 100 == 50 {
            shot(&g, &format!("{dir}/5-walk{:02}.ppm", f / 100));
            eprintln!("frame {f}: at {}, {}", g.px(), g.py());
            if g.px() == last {
                break;
            }
            last = g.px();
        }
    }
    g.frames(30);
    shot(&g, &format!("{dir}/6-hut.ppm"));
}

/// Where a frame goes, routine by routine, in cycles a frame, over level 1 start to hut:
/// `cargo test --release -p unc-asm --test game_cloudberry profile -- --ignored --nocapture`.
#[test]
#[ignore]
fn profile() {
    let mut g = Game::play();
    let names = ["main", "work", "sky", "input", "player", "camera", "scroll", "backdrop", "ents", "pdraw", "clock", "boxes", "hud", "noises"];
    let at: Vec<u16> = names.iter().map(|n| g.sym(n)).collect();
    let mut spent = vec![0u64; names.len()];
    let mut most = vec![0u64; names.len()];
    let mut now = 0;
    let mut jumping = false;
    let mut frames = 0;
    while g.get("STATE") != CLEAR && frames < 4000 {
        g.set("LIVES", 3);
        g.walker(&mut jumping);
        let end = g.m.cpu.cycles + to_vblank(&g.m) as u64;
        let mut this = vec![0u64; names.len()];
        while g.m.cpu.cycles < end {
            if let Some(i) = at.iter().position(|&a| a == g.m.cpu.pc) {
                now = i;
            }
            let t = g.m.cpu.cycles;
            g.step();
            this[now] += g.m.cpu.cycles - t;
        }
        for i in 0..names.len() {
            spent[i] += this[i];
            most[i] = most[i].max(this[i]);
        }
        frames += 1;
    }
    for ((n, c), m) in names.iter().zip(&spent).zip(&most) {
        eprintln!("{n:>9} mean {:>6} most {:>6}", c / frames, m);
    }
}
