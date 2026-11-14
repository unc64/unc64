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
        let frames = g.until(400, "the title", |g| g.get("STATE") == TITLE && g.chip().shown());
        assert!(frames < 150, "setting up the sheet took {frames} frames");
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
                for (i, &(by, bh, col, pat)) in bands.iter().enumerate() {
                    // the last is the hillside, whose pattern goes with the hills'
                    let pat = if i == 6 && (cam >> 1) & 1 != 0 { !pat } else { pat };
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
    g.m.key(SPACE);
    let held = g.until(60, "SPACE starts a game", |g| g.get("STATE") == PLAY);
    g.m.key_up(SPACE);
    eprintln!("unpacking a level and drawing its first 21 columns holds the picture for {held} frames");
    assert!(held <= 12);
    g.frames(2);
    let text = g.text();
    let berries = g.things(E_BERRY).len();
    assert!(text.lines().next().unwrap().starts_with(&format!(" BERRIES 00/{berries}  LIVES 3  TIME 2")), "{text}");
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
    assert!(g.text().starts_with(&format!(" BERRIES 01/{}", berries.len())), "{}", g.text());
    assert!(g.things(E_BERRY).is_empty());
}

#[test]
fn a_lemming_walks_to_and_fro_bites_and_is_flattened_by_a_jump() {
    let mut g = Game::play();
    let i = g.things(E_LEM)[0];
    let y = g.at("EY", i) as i32;
    assert_eq!(y, 64, "on the grass");
    // watch it going left: it turns round at the rock, and never leaves the grass
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
    assert_eq!((g.flags(47, 8), g.flags(48, 8)), (SOLID, 0));
    assert_eq!(lo, 48 * 8, "it turned at the rock");
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
    let berries = g.things(E_BERRY).len();
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
    assert_eq!(g.things(E_BERRY).len(), berries, "the berries are back");
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
    // to the birch wood, which is flat: the camera is past its fourth time round the ring
    let mut jumping = false;
    for _ in 0..2000 {
        if g.px() >= 1210 && g.get("GROUND") == 1 {
            break;
        }
        g.walker(&mut jumping);
        g.frame();
    }
    g.m.key_up(RIGHT);
    g.m.key_up(SPACE);
    g.frames(20);
    g.to_work();
    let far = g.cam();
    assert!(far > 1024 + 100, "{far}");
    // back left: the camera swings round to lead him the other way, two pixels a frame, and
    // then goes with him
    g.m.key(LEFT);
    let mut last = far;
    for f in 0..170 {
        g.frame();
        g.to_work();
        let cam = g.cam();
        assert!((0..=2).contains(&(last - cam)), "frame {f}: the camera went from {last} to {cam}");
        last = cam;
        g.assert_page(&format!("going left, frame {f}"));
    }
    g.m.key_up(LEFT);
    assert!(last < 1024 - 40, "back across the ring's join: {far} to {last}");
    // put down half a level away, the camera comes at six pixels a frame, a new column
    // nearly every frame, and the page is right all the way
    g.set("PXL", (200 & 255) as u8);
    g.set("PXH", (200 >> 8) as u8);
    g.set("PY", 20);
    g.set("CAMSPD", 6);
    let mut most = 0;
    for f in 0..250 {
        g.set("PY", 20);
        g.set("VYL", 0);
        g.set("VYH", 0);
        g.frame();
        g.to_work();
        most = most.max(last - g.cam());
        last = g.cam();
        g.assert_page(&format!("hurrying, frame {f}"));
    }
    assert_eq!(most, 6);
    assert_eq!(last, g.px() - 94, "and stops with him where it likes him");
    assert!((190..=200).contains(&g.px()));
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

// ---------- the music ----------
//
// Nobody here can listen, so these hold the tunes to what can be read off the chip: the form,
// the three voices' jobs, that each sound is made as the MUSIC TRACKER's instrument of the
// same name makes it, and that what comes out of the loudspeaker is there and doesn't clip.

const W_TRI: u8 = 1;
const W_PULSE: u8 = 4;
const W_NOISE: u8 = 8;
const W_DRAWN: u8 = 16;

/// The AUNT's voice `v` (0-2) as its registers stand: FREQ, WAVE, and whether its gate is on.
fn voice(g: &Game, v: u16) -> (u16, u8, bool) {
    let at = 0xC0C0 + v * 8;
    (u16::from_le_bytes([g.m.peek(at), g.m.peek(at + 1)]), g.m.peek(at + 3), g.m.peek(at + 4) & 1 != 0)
}

/// A pattern as the player reads it: (row it starts on, note, rows it has, instrument).
/// Voice 1's instrument is its line's; the others name theirs as they go.
fn pattern(g: &Game, mut at: u16, mut ins: u8) -> Vec<(u32, u8, u8, u8)> {
    let (mut notes, mut row) = (Vec::new(), 0);
    for _ in 0..400 {
        match g.m.peek(at) {
            0xFF => return notes,
            i @ 0x80..=0x9F => {
                ins = i & 0x7F;
                at += 1;
            }
            n => {
                let d = g.m.peek(at + 1);
                assert!(d > 0 && (n <= 95 || n == 0x7E), "note {n} for {d} rows");
                notes.push((row, n, d, ins));
                row += d as u32;
                at += 2;
            }
        }
    }
    panic!("a pattern with no end");
}

struct Line {
    at: u16,
    pats: [u16; 3],
    ins: u8,
    up: u8,
}

/// A tune's order: its lines, and where its end says to go on from (0: nowhere).
fn order(g: &Game, tune: &str) -> (Vec<Line>, u16) {
    let word = |a: u16| u16::from_le_bytes([g.m.peek(a), g.m.peek(a + 1)]);
    let mut at = g.sym(tune) + 2;
    let mut lines = Vec::new();
    while word(at) != 0 {
        lines.push(Line { at, pats: [word(at), word(at + 2), word(at + 4)], ins: g.m.peek(at + 6), up: g.m.peek(at + 7) });
        at += 8;
    }
    (lines, word(at + 2))
}

fn rows(p: &[(u32, u8, u8, u8)]) -> u32 {
    p.iter().map(|n| n.2 as u32).sum()
}

fn ordp(g: &Game) -> u16 {
    u16::from_le_bytes([g.get("ORDP"), g.at("ORDP", 1)])
}

/// An instrument's number, from its name in the source ($80 | number).
fn ins(g: &Game, name: &str) -> u8 {
    g.sym(name) as u8 & 0x7F
}

const REST: u8 = 0;
const OFF: u8 = 0x7E;

#[test]
fn the_one_tune_is_written_as_the_player_reads_it_and_has_no_snare_in_it() {
    let g = Game::boot();
    let hz = |n: u16| (g.at("FREQLO", n - 1) as f64 + 256.0 * g.at("FREQHI", n - 1) as f64) / 16.777;
    assert_eq!(hz(g.sym("A4")).round(), 440.0, "the note table: A is 440 Hz");
    // one tune and three stingers, and nothing else: no tune of a level's own, no snare
    for gone in ["t_day", "t_dusk", "day_loop", "dusk_loop", "T_DAY", "T_DUSK", "SNARE", "CRACK", "TOM", "HAT", "KICK", "BASS"] {
        assert!(g.asm.symbol(gone).is_none(), "{gone} is still in the source");
    }
    let kit: Vec<u8> = ["STRING", "LEAD", "TRIANG", "BLIP", "THUMP", "RIM", "TSS", "OPEN"].iter().map(|n| ins(&g, n)).collect();
    assert_eq!(kit, [1, 2, 3, 4, 5, 6, 7, 8], "eight instruments");
    let string = ins(&g, "STRING");
    for (tune, loops_to, bars) in [("t_title", "title_loop", 24), ("t_clear", "", 2), ("t_life", "", 1), ("t_over", "", 2)] {
        let (lines, end) = order(&g, tune);
        assert_eq!(end, if loops_to.is_empty() { 0 } else { g.sym(loops_to) }, "{tune}: where its end goes");
        assert_eq!(g.m.peek(g.sym(tune)), 7, "{tune}: 107 beats a minute");
        let mut looped = 0;
        for l in &lines {
            let p: Vec<_> = (0..3).map(|v| pattern(&g, l.pats[v], l.ins)).collect();
            // the three voices' patterns are as long as each other, in whole bars of 16 rows
            assert_eq!((rows(&p[0]), rows(&p[0])), (rows(&p[1]), rows(&p[2])), "{tune}, the line at ${:04X}", l.at);
            assert_eq!(rows(&p[0]) % 16, 0);
            if loops_to.is_empty() || l.at >= end {
                looped += rows(&p[0]) / 16;
            }
            // who plays what: the tune on a lead, the percussion on hats and the rim, the
            // bass on the kick and the blip. No row anywhere is a snare
            assert!([ins(&g, "STRING"), ins(&g, "LEAD"), ins(&g, "TRIANG")].contains(&l.ins));
            assert!(p[1].iter().filter(|n| n.1 != REST).all(|n| [ins(&g, "TSS"), ins(&g, "OPEN"), ins(&g, "RIM")].contains(&n.3)), "{tune}: the percussion's kit");
            assert!(p[2].iter().filter(|n| n.1 != REST && n.1 != OFF).all(|n| [ins(&g, "BLIP"), ins(&g, "THUMP")].contains(&n.3)), "{tune}: the bass voice's kit");
        }
        assert_eq!(looped, bars, "{tune}: bars before it goes round, the way in not counted");
        eprintln!("{tune}: {} lines, {looped} bars", lines.len());
        if loops_to.is_empty() {
            continue;
        }
        // the way in is four bars at most, and all three voices play in its first bar
        assert_eq!(lines[1].at, end, "{tune} goes round to just after the way in");
        let way_in: Vec<_> = (0..3).map(|v| pattern(&g, lines[0].pats[v], lines[0].ins)).collect();
        assert!(rows(&way_in[0]) <= 64);
        assert!(way_in.iter().all(|p| p.iter().any(|n| n.0 < 16 && n.1 != REST && n.1 != OFF)), "{tune}: a voice waits");
        // A is on the string; it is said again a pulse an octave up from the same patterns; B
        // is other patterns on a third instrument
        let a = &lines[1];
        assert_eq!(a.ins, string, "{tune}: the tune starts on the string");
        let again = lines[2..].iter().find(|l| l.pats[0] == a.pats[0] && l.ins != a.ins).expect("A on another instrument");
        assert_eq!((again.ins, again.up), (ins(&g, "LEAD"), 12));
        let b = lines[2..].iter().find(|l| l.pats[0] != a.pats[0] && l.pats[0] != lines[2].pats[0] && l.ins != a.ins && l.ins != again.ins).expect("a B section");
        assert_eq!(b.ins, ins(&g, "TRIANG"));
        // the tune is a bar that moves and a bar that waits
        let lead = pattern(&g, a.pats[0], a.ins);
        let bar = |k: u32| lead.iter().filter(|n| n.0 / 16 == k && n.1 != REST && n.1 != OFF).collect::<Vec<_>>();
        for k in [1, 3] {
            assert!(bar(k).iter().any(|n| n.2 >= 8), "{tune}: bar {} of the tune has no long note", k + 1);
        }
        assert!(bar(0).len() >= 4 && bar(0).len() > bar(1).len());
        // the bass: one riff, the same every bar, moved to the bar's chord, a chord a bar. It
        // stops for nothing: where a snare used to be, on the second and fourth beats (rows
        // 4 and 12), it plays through
        let bass = pattern(&g, a.pats[2], 0);
        let riff = |k: u32| bass.iter().filter(|n| n.0 / 16 == k).map(|n| (n.0 % 16, n.3)).collect::<Vec<_>>();
        assert_eq!(riff(0), riff(1), "the bass's rhythm, bar by bar");
        assert_eq!(riff(0), riff(2));
        let (blip, thump) = (ins(&g, "BLIP"), ins(&g, "THUMP"));
        assert_eq!(riff(0), [(0, thump), (3, blip), (4, blip), (6, blip), (8, thump), (10, blip), (12, blip), (14, blip)]);
        assert!(bass.iter().all(|n| n.1 != REST && n.1 != OFF), "the bass rests");
        let roots: std::collections::BTreeSet<u8> = (0..4).map(|k| bass.iter().find(|n| n.0 == k * 16).unwrap().1).collect();
        assert_eq!(roots.len(), 4, "a chord a bar, four bars");
        // the percussion: hats, and the rim on the second and fourth beats
        let perc = pattern(&g, a.pats[1], 0);
        let rim: Vec<u32> = perc.iter().filter(|n| n.3 == ins(&g, "RIM") && n.1 != REST).map(|n| n.0 % 16).collect();
        assert_eq!(rim, [4, 12, 4, 12, 4, 12, 4, 12]);
        assert_eq!(perc.iter().filter(|n| n.0 < 16 && n.1 != REST).count(), 10);
    }
}

/// A frame of what the chip was told, and of the player's own account of it.
#[derive(Clone)]
struct Heard {
    v: [(u16, u8, bool); 3],
    age: [u8; 3],
    ins: [u8; 3],
    note: [u8; 3],
    line: u16,
    table: [u8; 32],
}

fn hear(g: &Game) -> Heard {
    let mut table = [0u8; 32];
    for (i, b) in table.iter_mut().enumerate() {
        *b = g.m.peek(0xC0E0 + i as u16);
    }
    Heard {
        v: [voice(g, 0), voice(g, 1), voice(g, 2)],
        age: [g.get("VAGE"), g.at("VAGE", 1), g.at("VAGE", 2)],
        ins: [g.get("VINS") / 8, g.at("VINS", 1) / 8, g.at("VINS", 2) / 8],
        note: [g.get("VNOTE"), g.at("VNOTE", 1), g.at("VNOTE", 2)],
        line: ordp(g),
        table,
    }
}

#[test]
fn the_tune_starts_on_the_title_and_plays_on_through_the_levels() {
    let mut g = Game::boot_niece(true);
    g.until(400, "the tune", |g| g.get("TUNEON") == 1);
    let (lines, end) = order(&g, "t_title");
    assert_eq!((ordp(&g), g.get("MSPD")), (lines[0].at, 7));
    let mut heard = Vec::new();
    let mut started = None;
    // the title for ten bars; SPACE; level 1, standing at its start, to the tune's second
    // time round and a little more
    for f in 0..(2 + 24 + 6) * 16 * 7u32 {
        if f == 10 * 112 {
            g.m.key(SPACE);
        }
        if f == 10 * 112 + 3 {
            g.m.key_up(SPACE);
        }
        g.frame();
        heard.push(hear(&g));
        if started.is_none() && g.get("STATE") == PLAY {
            started = Some(heard.len());
        }
        assert_eq!(g.get("TUNEON"), 1);
    }
    let started = started.expect("a game");
    assert_eq!((g.get("STATE"), g.get("LEVEL"), g.get("LIVES")), (PLAY, 0, 3));
    // all three voices in the first bar
    for v in 0..3 {
        let on = heard[..112].iter().filter(|h| h.v[v].2).count();
        assert!(on >= 20, "voice {} has its gate on for {on} frames of the first bar", v + 1);
    }
    // the order: every line in turn and round to the first after the way in, and the game
    // starting in the middle of it changed nothing: the tune went on from where it was
    let mut visited: Vec<u16> = Vec::new();
    for h in &heard {
        if visited.last() != Some(&h.line) {
            visited.push(h.line);
        }
    }
    let mut want: Vec<u16> = lines.iter().map(|l| l.at).collect();
    want.push(end);
    want.push(lines[2].at);
    assert_eq!(visited, want, "the order");
    assert_eq!(heard[started].line, heard[started - 30].line, "the same line of the order before and after the level was unpacked");
    assert!(heard[started].line != lines[0].at);
    // any two seconds, the title or the level: all three voices sounding. (While the level
    // is unpacked the voices are let go: that's ten frames.)
    for (i, w) in heard.windows(100).enumerate() {
        for v in 0..3 {
            let on = w.iter().filter(|h| h.v[v].2).count();
            assert!(on >= 10, "frames {i} to {}: voice {} has its gate on for {on} frames in 100", i + 100, v + 1);
        }
        let hits = w.windows(2).filter(|p| p[1].age[1] < p[0].age[1] && p[1].note[1] != 0).count();
        assert!(hits >= 6, "frames from {i}: {hits} hits of percussion in two seconds");
    }
    // voice 1 is the string, then a pulse, then a triangle
    let waves: std::collections::BTreeSet<u8> = heard.iter().filter(|h| h.v[0].2).map(|h| h.v[0].1).collect();
    assert_eq!(waves.into_iter().collect::<Vec<_>>(), [W_TRI, W_PULSE, W_DRAWN]);
    // the bass is under the tune
    let mut bass: Vec<u16> = heard.iter().filter(|h| h.v[2].2 && h.v[2].1 == W_PULSE && h.age[2] > 6).map(|h| h.v[2].0).collect();
    let mut tune: Vec<u16> = heard.iter().filter(|h| h.v[0].2).map(|h| h.v[0].0).collect();
    bass.sort();
    tune.sort();
    assert!(bass[bass.len() * 9 / 10] < tune[tune.len() / 10], "the bass up to {} and the tune down to {}", bass[bass.len() * 9 / 10], tune[tune.len() / 10]);
    // the bass voice's noise is the pick and nothing more: one frame of it in each note, its
    // second, and none anywhere else. (The snare that was in the riff is gone.)
    let notes = (1..heard.len()).filter(|&f| heard[f].note[2] != 0 && heard[f].age[2] == 1 && heard[f - 1].age[2] != 0).count();
    let noise: Vec<usize> = (0..heard.len()).filter(|&f| heard[f].v[2].1 == W_NOISE).collect();
    assert!(notes > 200, "{notes} bass notes");
    assert!(noise.iter().all(|&f| heard[f].age[2] == 2), "noise on the bass voice that isn't a note's second frame");
    assert!((noise.len() as i64 - notes as i64).abs() <= 3, "{} frames of noise for {notes} notes", noise.len());
    eprintln!("the bass voice: {notes} notes and {} frames of noise in {} frames, {:.1}%", noise.len(), heard.len(), noise.len() as f64 * 100.0 / heard.len() as f64);
    // and into level 2: the hut's few notes came between, so there the tune starts afresh
    g.until(100, "he lands", |g| g.get("GROUND") == 1);
    g.to_level_two();
    assert_eq!((g.get("TUNEON"), g.get("TUNEN"), g.get("MSPD")), (1, 0, 7));
    assert!(lines.iter().any(|l| l.at == ordp(&g)));
    assert_eq!(g.get("LATE"), 0);
}

#[test]
fn every_sound_is_made_as_the_trackers_instrument_makes_it() {
    // The tracker's recipes, read off the chip a frame at a time while the tune plays from
    // its top through A on the string and A on the pulse.
    let mut g = Game::boot_niece(true);
    g.until(400, "the tune", |g| g.get("TUNEON") == 1);
    let mut heard = vec![hear(&g)];
    for _ in 0..(2 + 16) * 112 {
        g.frame();
        heard.push(hear(&g));
    }
    // (a note starts on the frame its age goes back to 1)
    let starts = |v: usize, name: &str| -> Vec<usize> {
        let i = ins(&g, name);
        (1..heard.len() - 12).filter(|&f| heard[f].ins[v] == i && heard[f].note[v] != 0 && heard[f].age[v] == 1 && heard[f - 1].age[v] != 0).collect()
    };
    let wave = |v: usize, f: usize, n: usize| -> Vec<u8> { heard[f..f + n].iter().map(|h| h.v[v].1).collect() };
    // a bass note: a frame of noise for the pick, a blip an octave up, and the note
    let blip = starts(2, "BLIP");
    assert!(blip.len() > 80, "{} bass notes", blip.len());
    for &f in &blip {
        assert_eq!(wave(2, f, 4), [W_PULSE, W_NOISE, W_PULSE, W_PULSE], "a bass note's first four frames, at frame {f}");
        let (up, note) = (heard[f + 2].v[2].0 as i32, heard[f + 3].v[2].0 as i32);
        assert!((up - 2 * note).abs() <= 2, "the blip is an octave over the note: {up} and {note}");
    }
    // the kick: a triangle two octaves up, a frame of noise, and a pulse falling onto the note
    let thump = starts(2, "THUMP");
    assert!(thump.len() >= 30);
    for &f in &thump {
        assert_eq!(wave(2, f, 6), [W_TRI, W_NOISE, W_PULSE, W_PULSE, W_PULSE, W_PULSE], "the kick at frame {f}");
        let p: Vec<u16> = heard[f + 2..f + 6].iter().map(|h| h.v[2].0).collect();
        assert!(p[0] > p[1] && p[1] > p[2] && p[2] > p[3], "falling: {p:?}");
    }
    // hats are two frames of noise with the gate on; the rim is a stick, a skin, a rattle
    assert!(starts(1, "TSS").iter().all(|&f| wave(1, f, 2) == [W_NOISE, W_NOISE] && !heard[f + 2].v[1].2));
    let rim = starts(1, "RIM");
    assert!(rim.len() >= 30);
    assert!(rim.iter().all(|&f| wave(1, f, 5) == [W_NOISE, W_TRI, W_TRI, W_NOISE, W_NOISE]));
    // the string: the wave table is drawn again as the note ages, every frame at first.
    // Over a note of 70 frames, 20 different drawings or more, each one of the tape's
    let long = starts(0, "STRING").into_iter().find(|&f| (1..70).all(|k| heard[f + k].age[0] == heard[f].age[0] + k as u8)).expect("a long note on the string");
    let drawings: std::collections::BTreeSet<[u8; 32]> = heard[long..long + 70].iter().map(|h| h.table).collect();
    assert!(drawings.len() >= 20, "{} drawings of the wave table in a note of 70 frames", drawings.len());
    assert_ne!(heard[long].table, heard[long + 1].table);
    assert_ne!(heard[long + 1].table, heard[long + 2].table);
    let waves_at = g.sym("WAVES");
    for d in &drawings {
        assert!((0..32).any(|k| (0..32).all(|i| g.m.peek(waves_at + k * 32 + i) == d[i as usize])));
    }
    assert!(heard[long..long + 70].iter().all(|h| h.v[0].1 == W_DRAWN && h.v[0].2));
    let rails = |t: &[u8; 32]| t.iter().filter(|&&b| b == 0 || b == 255).count();
    assert!(rails(&heard[long].table) >= 24 && rails(&heard[long + 69].table) < rails(&heard[long].table));
    // the pulse lead: every note has an attack of its own, because the one before was let go
    // two frames before it; its vibrato comes late
    let lead = starts(0, "LEAD");
    assert!(lead.len() >= 30, "{} notes on the pulse", lead.len());
    let mut let_go = 0;
    for &f in &lead[1..] {
        if heard[f - 1].ins[0] == ins(&g, "LEAD") {
            assert!(!heard[f - 1].v[0].2 && !heard[f - 2].v[0].2 && heard[f - 3].v[0].2, "the note before frame {f} wasn't let go two frames early");
            let_go += 1;
        }
        assert!(heard[f].v[0].2);
    }
    assert!(let_go >= 25);
    let held = lead.iter().copied().find(|&f| (1..40).all(|k| heard[f + k].age[0] as usize == k + 1)).expect("a held note on the pulse");
    let early: std::collections::BTreeSet<u16> = heard[held..held + 11].iter().map(|h| h.v[0].0).collect();
    let late: std::collections::BTreeSet<u16> = heard[held + 14..held + 38].iter().map(|h| h.v[0].0).collect();
    assert_eq!(early.len(), 1, "no vibrato for a quarter of a second");
    assert!(late.len() >= 4, "and then there is: {late:?}");
}

#[test]
fn the_pulse_width_runs_and_the_bass_goes_through_the_filter() {
    let mut g = Game::boot();
    // the bass voice goes through the low-pass, whose corner never stands still; the hats
    // and the tune go round it
    let (mut corners, mut through) = (std::collections::BTreeSet::new(), std::collections::BTreeSet::new());
    for _ in 0..400 {
        g.frame();
        corners.insert(g.m.peek(0xC0D8));
        through.insert(g.m.peek(0xC0D9) & 7);
    }
    assert!(corners.len() > 60 && *corners.iter().next().unwrap() >= 104 && *corners.iter().last().unwrap() <= 191, "{corners:?}");
    assert!(through.contains(&4) && through.iter().all(|m| m & 3 == 0), "voice 3 and only voice 3: {through:?}");
    assert_eq!(g.m.peek(0xC0DA), 0x1C, "volume 12, low-pass");
    // the pulse lead: the width moves every frame of every note
    let lead = ins(&g, "LEAD");
    g.until(3000, "A on the pulse", |g| g.get("VINS") / 8 == lead && g.get("VNOTE") != 0);
    let mut widths = vec![g.m.peek(0xC0C2)];
    for _ in 0..300 {
        g.frame();
        if g.get("VINS") / 8 == lead {
            widths.push(g.m.peek(0xC0C2));
        }
    }
    let still = widths.windows(2).filter(|w| w[0] == w[1]).count();
    assert!(still * 10 < widths.len(), "the width stood still for {still} frames of {}", widths.len());
}

#[test]
fn the_hut_a_lost_life_and_game_over_each_have_a_few_notes_that_end() {
    let mut g = Game::play();
    g.frames(150);
    // a life: one bar, and then the level's tune from its start
    g.stand(300, GROUND_Y);
    g.m.key(RIGHT);
    g.until(100, "into the pool", |g| g.get("STATE") == DYING);
    g.m.key_up(RIGHT);
    assert_eq!((ordp(&g), g.get("TUNEON"), g.get("TUNEN")), (g.sym("t_life") + 2, 1, g.sym("T_LIFE") as u8));
    let mut notes = std::collections::BTreeSet::new();
    for _ in 0..45 {
        g.frame();
        notes.insert(g.get("VNOTE"));
    }
    assert!(notes.len() >= 3, "{notes:?}");
    g.until(100, "back", |g| g.get("STATE") == PLAY);
    g.frame();
    assert_eq!((ordp(&g), g.get("TUNEON"), g.get("TUNEN")), (g.sym("t_title") + 2, 1, 0), "the tune, from its start");
    // the hut: two bars, and then nothing but the clock's ticks
    let door = (0..256).find(|&c| g.flags(c, 8) == 0x10).expect("a door");
    g.stand(door * 8 - 40, GROUND_Y);
    g.m.key(RIGHT);
    g.until(100, "the hut", |g| g.get("STATE") == CLEAR);
    g.m.key_up(RIGHT);
    assert_eq!((ordp(&g), g.get("TUNEON")), (g.sym("t_clear") + 2, 1));
    let frames = g.until(250, "its end", |g| g.get("TUNEON") == 0);
    assert!((218..=228).contains(&frames), "{frames} frames of it");
    assert!(!voice(&g, 0).2 && !voice(&g, 2).2, "voices 1 and 3 let go");
    g.until(400, "the count", |g| g.get("CT") == 1);
    g.frames(20);
    assert!(g.get("TUNEON") == 0 && g.get("BSND") == 0);
    // game over: two bars, and silence
    let mut g = Game::play();
    g.set("LIVES", 1);
    g.stand(300, GROUND_Y);
    g.hold(RIGHT, 60);
    g.until(200, "game over", |g| g.get("STATE") == OVER);
    assert_eq!((ordp(&g), g.get("TUNEON")), (g.sym("t_over") + 2, 1));
    let frames = g.until(300, "its end", |g| g.get("TUNEON") == 0);
    assert!((218..=228).contains(&frames), "{frames} frames of it");
    g.frames(5);
    assert!((0..3).all(|v| !voice(&g, v).2), "all three let go");
    // and from game over the title starts the tune again
    g.until(600, "the title", |g| g.get("STATE") == TITLE);
    g.frames(3);
    assert_eq!((g.get("TUNEON"), g.get("TUNEN")), (1, 0));
    assert_eq!(g.get("LATE"), 0);
}

#[test]
fn what_comes_out_of_the_loudspeaker_is_there_and_doesnt_clip() {
    // From switching on: every frame's sound, as a host would play it, through the title and
    // into level 1.
    let mut g = Game::boot_niece(true);
    let mut owed = 0.0f64;
    let mut frames: Vec<(Vec<f32>, bool)> = Vec::new();
    for f in 0..1500 {
        if f == 700 {
            g.m.key(SPACE);
        }
        if f == 704 {
            g.m.key_up(SPACE);
        }
        g.frame();
        owed += Machine::samples_per_frame(44_100);
        let mut s = vec![0.0f32; owed as usize];
        owed -= s.len() as f64;
        g.m.audio_frame(44_100, &mut s);
        frames.push((s, g.get("TUNEON") == 1 && (0..3).any(|v| voice(&g, v).2)));
    }
    let rms = |fr: &[(Vec<f32>, bool)]| {
        let (sum, n) = fr.iter().flat_map(|f| f.0.iter()).fold((0.0f64, 0u32), |(s, n), &x| (s + x as f64 * x as f64, n + 1));
        (sum / n as f64).sqrt()
    };
    let start = frames.iter().position(|f| f.1).expect("the title's tune");
    assert!(rms(&frames[..start]) < 1e-4, "silence while the sheet is set up: {}", rms(&frames[..start]));
    let (title, day) = (rms(&frames[start + 10..690]), rms(&frames[750..]));
    let peak = frames.iter().flat_map(|f| f.0.iter()).fold(0.0f32, |m, x| m.max(x.abs()));
    eprintln!("the loudspeaker: the title {title:.4} RMS, level 1 {day:.4}, the highest peak {peak:.3}");
    assert!(title > 0.03 && day > 0.03, "{title} {day}");
    assert!(peak < 0.99, "it clips: {peak}");
    // no second of either is silent, and none is twice as loud as the whole
    for (i, w) in frames[start + 10..].chunks(50).enumerate() {
        if w.len() == 50 && !(680 - start - 10..760 - start - 10).contains(&(i * 50)) {
            let r = rms(w);
            assert!(r > 0.01 && r < 0.5, "second {i}: {r}");
        }
    }
}

#[test]
fn an_effect_borrows_voice_two_and_gives_it_back() {
    let mut g = Game::play();
    g.frames(250);
    let busy = |g: &Game| g.get("JSND") | g.get("BSND") | g.get("HSND") != 0;
    let berry = g.things(E_BERRY)[0];
    let lemming = g.things(E_LEM)[0];
    for (what, wave) in [("a jump", W_PULSE), ("a berry", W_TRI), ("a bite", 2)] {
        match what {
            "a jump" => g.m.key(SPACE),
            "a berry" => {
                g.stand(g.thing_x(berry) - 20, GROUND_Y);
                g.frames(30);
                g.put(g.thing_x(berry) + 1, g.at("EY", berry) as i32 - 4)
            }
            _ => {
                g.stand(g.thing_x(lemming) - 40, GROUND_Y - 30);
                g.frames(30);
                g.put(g.thing_x(lemming), GROUND_Y);
            }
        }
        g.until(4, what, &busy);
        g.m.key_up(SPACE);
        assert_eq!((voice(&g, 1).1, voice(&g, 1).2, g.at("VNOTE", 1)), (wave, true, 0), "{what}: voice 2 is the effect's");
        // while it has the voice the percussion's hits go by unplayed, and the tune and the
        // bass never stop; a quarter of a second after it started the effect is over, and the
        // percussion's next hit sets the voice up afresh
        let (mut gates, mut over, mut back) = ([0; 2], None, None);
        for f in 1..=40 {
            g.frame();
            gates[0] += voice(&g, 0).2 as u32;
            gates[1] += voice(&g, 2).2 as u32;
            if busy(&g) {
                assert_eq!(voice(&g, 1).1, wave, "{what}: something else wrote voice 2's waveform");
            } else if over.is_none() {
                over = Some(f);
            }
            if back.is_none() && !busy(&g) && g.at("VNOTE", 1) != 0 && g.at("VAGE", 1) >= 1 {
                back = Some(f);
                let i = g.at("VINS", 1) as u16;
                assert_eq!((g.m.peek(0xC0CD), g.m.peek(0xC0CE), g.m.peek(0xC0CF)), (g.at("INSTR", i + 1), g.at("INSTR", i + 2), g.at("INSTR", i + 7) >> 4), "{what}: the instrument's envelope and TRIM are back");
            }
        }
        eprintln!("{what}: over after {} frames, and the percussion has voice 2 again after {}", over.unwrap_or(99), back.unwrap_or(99));
        assert!(over.unwrap_or(99) <= 12, "{what}: longer than a quarter of a second");
        assert!(back.unwrap_or(99) <= 30, "{what}: voice 2 still isn't the music's");
        assert!(gates[0] >= 12 && gates[1] >= 8, "{what}: voices 1 and 3 kept playing: {gates:?}");
        // and nothing is stuck: in the next two seconds voice 2 plays hats and toms
        let mut waves = std::collections::BTreeSet::new();
        let mut hits = 0;
        let mut age = 0;
        for _ in 0..100 {
            g.set("INVUL", 0);
            g.put(60, GROUND_Y - 30);
            g.frame();
            waves.insert(voice(&g, 1).1);
            hits += (g.at("VAGE", 1) < age) as u32;
            age = g.at("VAGE", 1);
        }
        assert!(waves.contains(&W_NOISE) && hits >= 8, "{what}: {waves:?}, {hits} hits");
        assert_eq!((g.get("TUNEON"), g.get("STATE")), (1, PLAY));
    }
}

#[test]
fn the_player_is_cheap_enough() {
    // What the sound costs a frame, effects and music: from `noises` to the instruction
    // after its call, over level 1 from start to hut.
    let mut g = Game::play();
    let (from, to) = (g.sym("noises"), g.sym("aftersound"));
    let (mut worst, mut total, mut frames, mut began) = (0u64, 0u64, 0u64, None);
    let mut jumping = false;
    while g.get("STATE") != CLEAR && frames < 4000 {
        g.set("LIVES", 3);
        g.walker(&mut jumping);
        let end = g.m.cpu.cycles + to_vblank(&g.m) as u64;
        while g.m.cpu.cycles < end {
            if g.m.cpu.pc == from {
                began = Some(g.m.cpu.cycles);
            } else if g.m.cpu.pc == to
                && let Some(t) = began.take()
            {
                let c = g.m.cpu.cycles - t;
                worst = worst.max(c);
                total += c;
                frames += 1;
            }
            g.step();
        }
    }
    eprintln!("the sound: {} cycles a frame on average, {worst} at the worst, over {frames} frames", total / frames);
    assert!(total / frames < 1100 && worst < 2700, "{} and {worst}", total / frames);
}

/// The tune for a person to listen to, as a WAV (mono, 16 bits, 44.1 kHz): the way in, a
/// whole time round and six bars of the second. With it, the chip's registers a frame at a time, 25 bytes a frame as crates/unc-asm/examples/record.rs
/// writes them: `CLOUDBERRY_SHOTS=dir cargo test --release -p unc-asm --test game_cloudberry
/// recordings -- --ignored`.
#[test]
#[ignore]
fn recordings() {
    let dir = std::env::var("CLOUDBERRY_SHOTS").unwrap_or_else(|_| "target/cloudberry".into());
    std::fs::create_dir_all(&dir).unwrap();
    fn record(g: &mut Game, frames: u32, path: &str) {
        let (mut pcm, mut regs): (Vec<u8>, Vec<u8>) = (Vec::new(), Vec::new());
        let mut owed = 0.0f64;
        for _ in 0..frames {
            g.frame();
            for v in 0..3u16 {
                let at = 0xC0C0 + v * 8;
                let (wave, width) = (g.m.peek(at + 3), g.m.peek(at + 2));
                regs.extend_from_slice(&[g.m.peek(at), g.m.peek(at + 1), width << 4, width >> 4, (wave & 15) << 4 | (g.m.peek(at + 4) & 1), g.m.peek(at + 5), g.m.peek(at + 6)]);
            }
            let (cutoff, filter, volume) = (g.m.peek(0xC0D8), g.m.peek(0xC0D9), g.m.peek(0xC0DA));
            regs.extend_from_slice(&[0, cutoff, (filter & 0xF0) | (filter & 7), (volume & 0x70) | (volume & 15)]);
            owed += Machine::samples_per_frame(44_100);
            let mut s = vec![0.0f32; owed as usize];
            owed -= s.len() as f64;
            g.m.audio_frame(44_100, &mut s);
            pcm.extend(s.iter().flat_map(|x| ((x.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes()));
        }
        let mut wav = Vec::new();
        wav.extend(b"RIFF");
        wav.extend((36 + pcm.len() as u32).to_le_bytes());
        wav.extend(b"WAVEfmt ");
        for v in [16u32, 0x0001_0001, 44_100, 88_200, 0x0010_0002] {
            wav.extend(v.to_le_bytes());
        }
        wav.extend(b"data");
        wav.extend((pcm.len() as u32).to_le_bytes());
        wav.extend(pcm);
        std::fs::write(path, wav).unwrap();
        std::fs::write(path.replace(".wav", ".regs"), regs).unwrap();
    }
    let mut g = Game::boot_niece(true);
    g.until(400, "the title", |g| g.get("TUNEON") == 1);
    record(&mut g, (2 + 24 + 6) * 16 * 7, &format!("{dir}/tune.wav"));
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
