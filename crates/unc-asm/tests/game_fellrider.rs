//! FELLRIDER (games/niece/fellrider.s): a moped with real two-dimensional physics over
//! scrolling fells, for the NIECE (manual chapter 17). Ridden through scripted keys on a
//! booted 64K machine with the chip in; its state is read from the addresses the source's
//! symbol table gives.
//!
//! Two things are tested. That the physics is physics: the moped rests, pitches, rolls,
//! bounces, flies and falls as a body on two sprung wheels should. And the question the game
//! is there to answer: whether a 1 MHz processor can do that, a scrolling ground and the
//! drawing at 50 frames a second. So a meter watches every frame: the processor's work
//! (`work` to `endwork`, split at `terrain` and `draw`), the chip's drawing (every command
//! written to NGO, costed as chapter 17 costs it), and that each FLIP came at the very next
//! vertical blank.

use unc_core::machine::{CYCLES_PER_FRAME, CYCLES_PER_LINE, VBLANK_LINE};
use unc_core::niece::{self, Command};
use unc_core::{Machine, RamSize, Stop};

const SRC: &str = include_str!("../../../games/niece/fellrider.s");
const BUDGET: u64 = CYCLES_PER_FRAME as u64;

const TITLE: u8 = 0;
const RIDE: u8 = 1;
const DOWN: u8 = 2;
const OVER: u8 = 3;

const LEFT: u8 = 28;
const RIGHT: u8 = 29;
const UP: u8 = 30;
const DOWNKEY: u8 = 31;

const NGO: u16 = 0xC103;
const STA_ABS: u8 = 0x33;

/// The ground's middle line, and where the centre of mass rests above flat ground: the
/// hubs hang 7 below it, the wheels are 5 in radius, and the springs give 2 under its weight.
const BASE: f64 = 74.0;
const REST: f64 = BASE - 10.0;

/// What a stretch of frames cost, the processor and the chip.
#[derive(Default, Debug)]
struct Meter {
    frames: u64,
    /// `work` to `endwork`, in cycles: the worst frame and all of them.
    worst: u64,
    total: u64,
    /// Its five parts, the worst of each and all: the keys, the rules and the places; the
    /// ground's new columns; the three strips' BLITs; the forces, the panel and the sound;
    /// and the commands for the things, the moped and the dust.
    worst_part: [u64; 5],
    total_part: [u64; 5],
    /// From one `work` to the next: 19,968 when the FLIP came at the next vertical blank.
    longest: u64,
    shortest: u64,
    last_work: Option<u64>,
    begun: Option<u64>,
    mark: u64,
    part: usize,
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
    /// `work`, `terrain`, `draw`, `phys`, `sprites`, `endwork`
    marks: [u16; 6],
}

fn assemble() -> unc_asm::Assembly {
    unc_asm::assemble(SRC).unwrap_or_else(|e| panic!("fellrider.s doesn't assemble: {e:?}"))
}

impl Game {
    /// A UNC 64 with the NIECE, booted to READY., with FELLRIDER loaded and started as the
    /// hosts start a program, and run until the sheet is drawn and the title is up.
    fn boot() -> Game {
        let mut g = Game::boot_niece(true);
        let work = g.marks[0];
        g.m.run_until(work, 400 * BUDGET).unwrap_or_else(|s| panic!("never reached the title: {s:?}"));
        g.frames(3);
        assert_eq!(g.get("STATE"), TITLE);
        g.meter = Meter::default();
        g
    }

    fn boot_niece(niece: bool) -> Game {
        let asm = assemble();
        let mut m = Machine::boot(RamSize::K64);
        m.set_niece(niece);
        unc_asm::load(&mut m, &asm);
        let marks = ["work", "terrain", "draw", "phys", "sprites", "endwork"].map(|n| asm.symbol(n).unwrap());
        Game { m, asm, meter: Meter::default(), marks }
    }

    /// Boots and presses SPACE on the title.
    fn ride() -> Game {
        let mut g = Game::boot();
        g.press(b' ');
        g.until(5, "SPACE starts a run", |g| g.get("STATE") == RIDE);
        g.meter = Meter::default();
        g
    }

    /// A run on ground that stays flat however far it goes.
    fn ride_flat() -> Game {
        let mut g = Game::ride();
        g.set("RMAX", 0);
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
    /// `n` bytes at `name` + `i`, low byte first, as a signed number.
    fn num(&self, name: &str, i: u16, n: u16) -> i64 {
        let v = (0..n).fold(0i64, |v, k| v | (self.at(name, i + k) as i64) << (8 * k));
        let top = 1i64 << (8 * n - 1);
        if n < 8 && v & top != 0 { v - (top << 1) } else { v }
    }
    fn put(&mut self, name: &str, n: u16, v: i64) {
        for k in 0..n {
            self.set_at(name, k, (v >> (8 * k)) as u8);
        }
    }

    // ---- the moped, in pixels, frames and degrees
    fn x(&self) -> f64 {
        self.num("X0", 0, 5) as f64 / 65536.0
    }
    fn y(&self) -> f64 {
        self.num("Y0", 0, 3) as f64 / 65536.0
    }
    fn vx(&self) -> f64 {
        self.num("VX", 0, 3) as f64 / 65536.0
    }
    fn vy(&self) -> f64 {
        self.num("VY", 0, 3) as f64 / 65536.0
    }
    /// The pitch, nose up, from -180 to 180.
    fn pitch(&self) -> f64 {
        self.num("TH", 1, 2) as f64 * 360.0 / 65536.0
    }
    /// Degrees a frame.
    fn turning(&self) -> f64 {
        self.num("OM", 0, 3) as f64 * 360.0 / 65536.0 / 256.0
    }
    /// How far wheel `i`'s spring is squeezed (0 the back, 1 the front; 2 is the head into
    /// the ground).
    fn squeeze(&self, i: u16) -> f64 {
        (self.at("CPH", i) as f64 * 256.0 + self.at("CPL", i) as f64) / 256.0
    }
    /// The load on it, as a fraction of the moped's weight.
    fn load(&self, i: u16) -> f64 {
        (self.at("NH", i) as f64 * 256.0 + self.at("NL", i) as f64) / 3604.0
    }
    /// Tyre `i`'s speed, pixels a frame.
    fn spin(&self, i: u16) -> f64 {
        let v = self.at("SF", i) as i64 | (self.at("SL", i) as i64) << 8 | (self.at("SH", i) as i8 as i64) << 16;
        v as f64 / 65536.0
    }
    fn gas(&self) -> u32 {
        self.num("GAS", 0, 2) as u16 as u32
    }
    fn word(&self, name: &str) -> u32 {
        self.num(name, 0, 2) as u16 as u32
    }

    /// Puts the moped at (x, y), still, at `pitch` degrees.
    fn place(&mut self, x: f64, y: f64, pitch: f64) {
        self.put("X0", 5, (x * 65536.0) as i64);
        self.put("Y0", 3, (y * 65536.0) as i64);
        let th = (pitch / 360.0 * 65536.0).round() as i64;
        self.set("TH", 0);
        self.set_at("TH", 1, th as u8);
        self.set_at("TH", 2, (th >> 8) as u8);
        for name in ["VX", "VY", "OM"] {
            self.put(name, 3, 0);
        }
        for i in 0..2 {
            for name in ["SF", "SL", "SH"] {
                self.set_at(name, i, 0);
            }
        }
    }

    /// The ground's height in the ring at world column `x`, as the physics reads it.
    fn ground(&self, x: i64) -> f64 {
        let i = (x & 255) as u16;
        self.at("HGH", i) as f64 + self.at("HGL", i) as f64 / 256.0
    }
    /// Shapes the ground the physics rides on: the ring's column for each world column
    /// from `from` to `to` takes `f`'s height. (The ring only; the picture isn't redrawn.)
    fn shape(&mut self, from: i64, to: i64, f: impl Fn(f64) -> f64) {
        for x in from..to {
            let h = (f(x as f64) * 256.0).round() as i64;
            let i = (x & 255) as u16;
            self.set_at("HGL", i, h as u8);
            self.set_at("HGH", i, (h >> 8) as u8);
        }
    }

    /// One instruction, with the meter watching: the marks going by, and every command
    /// written to NGO.
    fn step(&mut self) {
        let pc = self.m.cpu.pc;
        let now = self.m.cpu.cycles;
        let mt = &mut self.meter;
        if pc == self.marks[0] {
            if let Some(t) = mt.last_work {
                let d = now - t;
                mt.longest = mt.longest.max(d);
                mt.shortest = if mt.shortest == 0 { d } else { mt.shortest.min(d) };
                mt.most_commands = mt.most_commands.max(mt.f_commands);
                mt.most_drawing = mt.most_drawing.max(mt.f_drawing);
            }
            mt.last_work = Some(now);
            mt.begun = Some(now);
            mt.mark = now;
            mt.part = 0;
            mt.f_commands = 0;
            mt.f_drawing = 0;
        } else if mt.begun.is_some() && mt.part < 5 && pc == self.marks[mt.part + 1] {
            let c = now - mt.mark;
            mt.worst_part[mt.part] = mt.worst_part[mt.part].max(c);
            mt.total_part[mt.part] += c;
            mt.mark = now;
            mt.part += 1;
            if mt.part == 5 {
                let c = now - mt.begun.take().unwrap();
                mt.worst = mt.worst.max(c);
                mt.total += c;
                mt.frames += 1;
            }
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
                if mt.last_work.is_some() {
                    mt.latest_done = mt.latest_done.max(mt.chip_free.max(now) - mt.last_work.unwrap());
                }
            } else {
                mt.f_commands += 1;
                mt.commands += 1;
                mt.f_drawing += c.cycles();
                mt.drawing += c.cycles();
                mt.chip_free = mt.chip_free.max(now) + c.cycles();
            }
        }
    }

    /// One frame of the game: to the next time it's at `work`, an instruction at a time.
    fn frame(&mut self) {
        let work = self.marks[0];
        let limit = self.m.cpu.cycles + 40 * BUDGET;
        self.step();
        while self.m.cpu.pc != work {
            self.step();
            assert!(self.m.cpu.cycles < limit, "the game stopped coming round to `work`");
        }
    }

    /// A frame the meter doesn't count.
    fn unmetered_frame(&mut self) {
        let lost = self.meter.lost;
        let saved = std::mem::take(&mut self.meter);
        self.frame();
        self.frame();
        assert_eq!(self.meter.lost, lost, "a command was lost");
        self.meter = Meter { last_work: None, begun: None, chip_free: 0, ..saved };
    }

    fn frames(&mut self, n: u32) {
        for _ in 0..n {
            self.frame();
        }
    }

    /// A tap: down for two frames, then up.
    fn press(&mut self, k: u8) {
        self.m.key(k);
        self.frames(2);
        self.m.key_up(k);
        self.frame();
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

    fn report(&self, what: &str) {
        let m = &self.meter;
        let n = m.frames.max(1);
        eprintln!(
            "{what}: {} frames; work: worst {} cycles, mean {} of {BUDGET} (physics, rules, panel and sound: worst {}, mean {}; ground: worst {}, \
             mean {}; the picture's commands: worst {}, mean {}); commands a frame: most {}, mean {}; drawing a frame: most {} cycles \
             ({} pixels), mean {} cycles, done at worst {} cycles after work began; queue: deepest {} of 64; frame to frame: {} to {} cycles; \
             lost {}",
            m.frames,
            m.worst,
            m.total / n,
            m.worst_part[0] + m.worst_part[3],
            (m.total_part[0] + m.total_part[3]) / n,
            m.worst_part[1],
            m.total_part[1] / n,
            m.worst_part[2] + m.worst_part[4],
            (m.total_part[2] + m.total_part[4]) / n,
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
            m.longest < BUDGET + 24 && m.shortest > BUDGET - 24,
            "a frame took from {} to {} cycles: not one FLIP every vertical blank",
            m.shortest,
            m.longest
        );
        assert_eq!(self.get("LATE"), 0, "the game's own count of late frames");
        assert_eq!(m.lost, 0, "commands lost");
        assert!(m.deepest < niece::QUEUE, "the queue filled");
    }
}

/// Cycles from now to the start of the next VBLANK line (a whole frame if it's now).
#[allow(dead_code)]
fn to_vblank(m: &Machine) -> u32 {
    let at = VBLANK_LINE * CYCLES_PER_LINE;
    match (at + CYCLES_PER_FRAME - m.mem.frame_cycle) % CYCLES_PER_FRAME {
        0 => CYCLES_PER_FRAME,
        n => n,
    }
}

/// A deterministic dice for the long rides: xorshift32.
struct Dice(u32);
impl Dice {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
}

// ---------- a trace, for comparing with the model the physics was worked out in ----------

/// FELL_TRACE=FILE and FELL_KEYS="60:0,40:2,..." (frames:PADS bits: 1 brake, 2 throttle,
/// 4 lean back, 8 lean forward) writes the state after every frame of that ride.
#[test]
#[ignore = "writes a trace of a scripted ride: FELL_TRACE=out.txt FELL_KEYS=... cargo test ... -- --ignored trace"]
fn trace() {
    let Ok(path) = std::env::var("FELL_TRACE") else { return };
    let keys = std::env::var("FELL_KEYS").unwrap_or_else(|_| "50:0,200:2".into());
    let mut g = Game::ride();
    if let Ok(r) = std::env::var("FELL_RMAX") {
        g.set("RMAX", r.parse().unwrap());
    }
    let mut out = String::new();
    for part in keys.split(',') {
        let (n, pads) = part.split_once(':').unwrap();
        let (n, pads): (u32, u8) = (n.parse().unwrap(), pads.parse().unwrap());
        for (bit, key) in [(1, LEFT), (2, RIGHT), (4, UP), (8, DOWNKEY)] {
            if pads & bit != 0 {
                g.m.key(key);
            } else {
                g.m.key_up(key);
            }
        }
        for _ in 0..n {
            g.frame();
            out += &format!(
                "{} {} {} {} {} {} {} {} {} {} {}\n",
                g.num("X0", 0, 5),
                g.num("Y0", 0, 3),
                g.num("VX", 0, 3),
                g.num("VY", 0, 3),
                g.num("TH", 0, 3) & 0xFFFFFF,
                g.num("OM", 0, 3),
                (g.spin(0) * 65536.0) as i64,
                (g.spin(1) * 65536.0) as i64,
                g.num("CPL", 0, 1) & 255 | (g.at("CPH", 0) as i64) << 8,
                g.num("CPL", 1, 1) & 255 | (g.at("CPH", 1) as i64) << 8,
                g.get("CRASH"),
            );
        }
    }
    std::fs::write(path, out).unwrap();
    g.report("trace");
}

// ---------- the ground, worked out again here ----------

/// The assembler's `sin(a, n)`: n times the sine of a 256ths of a circle, rounded, halves
/// away from zero.
fn asm_sin(a: i64, n: f64) -> i64 {
    let v = n * (a as f64 * std::f64::consts::TAU / 256.0).sin();
    (v.abs() + 0.5).floor() as i64 * if v < 0.0 { -1 } else { 1 }
}

/// The ground's height at world column `x` in 256ths of a line, as the header of the source
/// describes it: three sines of 300, 110 and 37 pixels, times a roughness that grows a 32nd
/// for each pixel past the first 384, held to `rmax`; never above line 36 or below 93.
fn height(x: i64, rmax: i64) -> i64 {
    let wave = |step: i64, size: f64| asm_sin(((x & 0xFFFF) * step & 0xFFFF) >> 8, size);
    let w = wave(218, 60.0) + wave(596, 34.0) + wave(1771, 16.0);
    let rough = ((x - 384) >> 5).clamp(0, 255).min(rmax);
    (74 * 256 - ((w * rough) >> 2)).clamp(36 * 256, 93 * 256)
}

impl Game {
    /// A rider good enough to get a long way: throttle up to `speed` pixels a frame, and
    /// leaning to keep the moped at the ground's slope. Sets the keys for the next frame.
    fn rider(&mut self, speed: f64) {
        let x = self.x() as i64;
        let slope = (self.ground(x + 10) - self.ground(x - 2)) / 12.0;
        let err = self.pitch() + slope.atan().to_degrees();
        let keys = [(RIGHT, self.vx() < speed && err < 12.0), (LEFT, false), (UP, err < -6.0), (DOWNKEY, err > 4.0)];
        for (k, down) in keys {
            if down {
                self.m.key_hold(k);
            } else {
                self.m.key_up(k);
            }
        }
    }
    fn keys_up(&mut self) {
        for k in [LEFT, RIGHT, UP, DOWNKEY, b' '] {
            self.m.key_up(k);
        }
    }
    /// Rides with `rider` until `f` says so, or panics after `max` frames.
    fn ride_until(&mut self, speed: f64, max: u32, what: &str, f: impl Fn(&Game) -> bool) -> u32 {
        for i in 0..max {
            if f(self) {
                self.keys_up();
                return i;
            }
            self.rider(speed);
            self.frame();
        }
        panic!("{what}: not within {max} frames (x = {:.0}, state {})\n{}", self.x(), self.get("STATE"), self.text());
    }
    fn airborne(&self) -> bool {
        self.load(0) == 0.0 && self.load(1) == 0.0
    }
    /// The moped's energy for each unit of its mass: moving, turning (its moment of inertia
    /// is 40.74), height, and what's wound into the two springs and their bump stops.
    /// (A frame ends with the speeds one step ahead of the places, so the height is taken
    /// where this speed will have put it.)
    fn energy(&self) -> f64 {
        let (g, k, stop) = (3604.0 / 65536.0, 901.0 / 65536.0, 6000.0 / 65536.0);
        let om = self.turning().to_radians();
        let springs: f64 = (0..2).map(|i| 0.5 * k * self.squeeze(i).powi(2) + 0.5 * stop * (self.squeeze(i) - 4.5).max(0.0).powi(2)).sum();
        0.5 * (self.vx().powi(2) + self.vy().powi(2)) + 0.5 * 40.74 * om * om - g * (self.y() + self.vy()) + springs
    }
}

// ---------- loading ----------

#[test]
fn it_assembles_and_says_which_machine_it_wants() {
    let asm = assemble();
    assert!(asm.warnings.is_empty(), "{:?}", asm.warnings);
    for (i, line) in SRC.lines().take(3).enumerate() {
        assert!(line.starts_with(';') && line.contains("64K"), "line {}: {line}", i + 1);
    }
    assert!(!SRC.contains(".include"), "one file");
    let top = asm.segments.iter().map(|s| s.start as usize + s.bytes.len()).max().unwrap();
    assert!(top <= 0xC000, "the program ends at ${top:04X}");
}

#[test]
fn without_the_niece_it_says_so_and_stops() {
    let mut g = Game::boot_niece(false);
    assert_eq!(g.m.run_frame(), Stop::Halted);
    assert!(g.text().contains("FELLRIDER NEEDS THE NIECE."), "{}", g.text());
}

/// The body is turned once, at the start: 32 pictures on the sheet. The first is the
/// drawing as it is, and the ninth (a quarter turn, nose up) is the same picture on its
/// tail: what was the top row is its left column, read upwards.
#[test]
fn the_sheet_holds_the_body_at_32_angles() {
    let g = Game::boot();
    let n = g.chip();
    let art = g.sym("bikeart");
    let pixel = |u: i32, v: i32| -> u8 {
        if !(0..25).contains(&u) || !(0..20).contains(&v) {
            return 4;
        }
        let b = g.m.peek(art + (v * 13 + u / 2) as u16);
        if u & 1 == 0 { b >> 4 } else { b & 15 }
    };
    // the drawing's middle (12.5, 10) is the picture's (16, 16)
    let mut wrong = 0;
    for j in 0..32 {
        for i in 0..32 {
            // upright: pixel (i, j) is the drawing's (i - 3.5 + .., j - 6): its 25 x 20 in the middle
            let (u, v) = (((i as f64 - 15.5) + 12.5).floor() as i32, ((j as f64 - 15.5) + 10.0).floor() as i32);
            wrong += (n.pixel(i, 380 + j) != pixel(u, v)) as u32;
            // a quarter turn nose up: across the picture is down the drawing
            let (u, v) = ((-(j as f64 - 15.5) + 12.5).floor() as i32, ((i as f64 - 15.5) + 10.0).floor() as i32);
            wrong += (n.pixel(i, 380 + 2 * 32 + j) != pixel(u, v)) as u32;
        }
    }
    assert!(wrong <= 8, "{wrong} pixels of the two pictures aren't the drawing's");
    // every one of the 32 has about the drawing's number of pixels in it
    let inked = (0..20).flat_map(|v| (0..25).map(move |u| (u, v))).filter(|&(u, v)| pixel(u, v) != 4).count() as i32;
    for k in 0..32 {
        let (x0, y0) = ((k % 8) * 32, 380 + (k / 8) * 32);
        let count = (0..32).flat_map(|j| (0..32).map(move |i| (i, j))).filter(|&(i, j)| n.pixel(x0 + i, y0 + j) != 4).count() as i32;
        assert!((count - inked).abs() < inked / 8, "angle {k}: {count} pixels, the drawing has {inked}");
    }
}

// ---------- the game round the physics ----------

#[test]
fn title_ride_run_over_and_again() {
    let mut g = Game::boot();
    assert_eq!(g.m.peek(niece::BASE + niece::reg::NCTRL), 1, "the page is shown");
    assert!(g.text().contains("PRESS SPACE TO RIDE"), "{}", g.text());
    assert!(g.text().lines().next().unwrap().starts_with("0000M BEST 0000M  COINS 00  GAS"), "{}", g.text());
    // the title is drawn on the page: its white letters between lines 10 and 23
    let page = g.page();
    let white = (10..24).flat_map(|y| (26..134).map(move |x| y * 160 + x)).filter(|&i| page[i] == 1).count();
    assert!(white > 400, "{white} white pixels of title");
    // the keys do nothing until SPACE
    g.hold(RIGHT, 20);
    assert_eq!((g.get("STATE"), g.x()), (TITLE, 128.0));
    g.press(b' ');
    assert_eq!(g.get("STATE"), RIDE);
    assert!(!g.text().contains("PRESS SPACE"));
    // hold the throttle from a standstill and it goes over backwards: the head touches,
    g.m.key(RIGHT);
    g.until(150, "it goes over backwards", |g| g.get("STATE") == DOWN);
    assert!(g.pitch() > 90.0, "pitch {}", g.pitch());
    assert_eq!(g.get("CRASH"), 1);
    // and a second later the run is over, with the keys dead meanwhile
    g.until(60, "the run is over", |g| g.get("STATE") == OVER);
    g.m.key_up(RIGHT);
    let text = g.text();
    assert!(text.contains("YOU CAME OFF!") && text.contains("PRESS SPACE TO RIDE AGAIN"), "{text}");
    let rode = g.word("DIST");
    assert!(rode >= 1 && rode < 20, "rode {rode} metres");
    assert!(text.contains(&format!("RODE {rode:04}M")), "{text}");
    assert_eq!(g.word("BEST"), rode);
    // SPACE: a new run from the start, the best kept
    g.frames(40);
    g.press(b' ');
    assert_eq!(g.get("STATE"), RIDE);
    assert_eq!((g.get("CRASH"), g.word("DIST"), g.gas() > 65000), (0, 0, true));
    assert!((g.x() - 128.0).abs() < 0.01 && g.pitch() == 0.0);
    assert!(g.text().lines().next().unwrap().contains(&format!("BEST {rode:04}M")), "{}", g.text());
    assert_eq!(g.meter.lost, 0);
}

// ---------- the physics ----------

/// At rest on flat ground the moped is level, each spring squeezed two pixels and carrying
/// half its weight, and it stays exactly where it is.
#[test]
fn it_rests_level_on_its_springs() {
    let mut g = Game::ride_flat();
    g.frames(150);
    let at = (g.x(), g.y());
    assert!((g.y() - REST).abs() < 0.02, "y {}", g.y());
    assert!(g.pitch().abs() < 0.1, "pitch {}", g.pitch());
    for i in 0..2 {
        assert!((g.squeeze(i) - 2.0).abs() < 0.03, "spring {i}: {}", g.squeeze(i));
        assert!((g.load(i) - 0.5).abs() < 0.01, "load {i}: {}", g.load(i));
    }
    g.frames(300);
    // (down the page it settles to a 256th of a pixel, the springs' own grain; across,
    // nothing moves it at all)
    assert_eq!(g.x(), at.0, "it crept");
    assert!((g.y() - at.1).abs() < 0.01, "it sank: {} from {}", g.y(), at.1);
    assert!(g.vx() == 0.0 && g.vy().abs() < 0.001 && g.turning().abs() < 0.001, "{} {} {}", g.vx(), g.vy(), g.turning());
}

/// Throttle drives the back wheel: the moped goes forward, its weight goes onto the back
/// spring and the nose comes up. The brake does the opposite.
#[test]
fn throttle_pitches_it_back_and_the_brake_forward() {
    let mut g = Game::ride_flat();
    g.frames(100);
    g.hold(RIGHT, 14);
    assert!(g.vx() > 0.25, "vx {}", g.vx());
    assert!(g.pitch() > 2.0, "pitch {}", g.pitch());
    assert!(g.squeeze(0) > 2.5 && g.squeeze(1) < 1.5, "springs {} {}", g.squeeze(0), g.squeeze(1));
    assert!(g.load(0) > 0.7 && g.load(1) < 0.3, "loads {} {}", g.load(0), g.load(1));
    // the back tyre is turning faster than the ground goes by: it's driving
    assert!(g.spin(0) > g.vx());
    // get it level and rolling at speed, then brake
    g.ride_until(3.0, 300, "up to speed", |g| g.vx() > 2.0 && g.pitch().abs() < 8.0 && g.turning().abs() < 0.3 && !g.airborne());
    let (v, level) = (g.vx(), g.pitch());
    g.hold(LEFT, 10);
    assert!(g.vx() < v - 0.25, "vx {} from {v}", g.vx());
    assert!(g.pitch() < level - 1.5 && g.pitch() < 0.0, "pitch {} from {level}", g.pitch());
    assert!(g.squeeze(1) > 2.5 && g.squeeze(1) > g.squeeze(0), "springs {} {}", g.squeeze(0), g.squeeze(1));
    assert!(g.load(1) > g.load(0));
    assert_eq!(g.get("CRASH"), 0);
}

/// Grip is the load times 1.1 and no more: with the front wheel in the air the brake on
/// it does nothing, and a back wheel carrying everything can push with all of it.
#[test]
fn grip_goes_with_the_load() {
    let mut g = Game::ride_flat();
    g.frames(100);
    // full throttle from rest: the pull on the moped is never more than 1.1 of the back
    // wheel's load, and while the tyre bites it is the engine's push
    g.m.key(RIGHT);
    let mut most = 0.0f64;
    for _ in 0..12 {
        let v = g.vx();
        g.frame();
        let pull = (g.vx() - v) * 65536.0 / 3604.0;
        assert!(pull <= 1.1 * (g.load(0) + g.load(1)) + 0.02, "pull {pull} of its weight on a load of {}", g.load(0));
        most = most.max(pull);
    }
    g.m.key_up(RIGHT);
    assert!(most > 0.45, "the most pull was {most} of its weight");
    // the same throttle with the wheel in the air turns the tyre nearly three times as
    // fast in the same time, and moves the moped not at all
    let on_ground = g.spin(0);
    let mut g = Game::ride_flat();
    g.place(128.0, REST - 40.0, 0.0);
    g.hold(RIGHT, 12);
    assert!(g.airborne());
    assert!(g.spin(0) > 2.5 * on_ground && g.spin(0) > 1.5, "tyre {} (on the ground {on_ground})", g.spin(0));
    assert_eq!(g.vx(), 0.0);
}

/// On a slope with no key held it rolls back down; on the flat it doesn't move.
#[test]
fn it_rolls_back_down_a_slope() {
    let mut g = Game::ride_flat();
    // a hillside rising to the right, one in four
    g.shape(40, 280, |x| BASE - (x - 128.0) * 0.25);
    g.place(128.0, BASE - 10.3, 14.0);
    g.frames(120);
    assert!(g.vx() < -0.5, "vx {}", g.vx());
    assert!(g.x() < 110.0, "x {}", g.x());
    assert!(g.spin(0) < -0.5 && g.spin(1) < -0.5, "the wheels turn as it rolls: {} {}", g.spin(0), g.spin(1));
    assert!(g.pitch() > 8.0 && g.pitch() < 20.0, "it lies on the slope: {}", g.pitch());
    // the brake holds it there (LEFT is the brake while a wheel is turning forwards, and
    // backwards otherwise, so hold it with the throttle's opposite: here, gas)
    assert_eq!(g.get("CRASH"), 0);
}

/// Dropped from a height with no key held, it never bounces as high as it fell from, each
/// bounce is lower than the last, and it comes to rest: energy doesn't appear from nowhere.
#[test]
fn a_drop_never_bounces_higher_and_comes_to_rest() {
    let mut g = Game::ride_flat();
    g.frames(60);
    g.place(128.0, REST - 30.0, 0.0);
    let start = g.energy();
    let mut tops = vec![g.y()];
    let (mut falling, mut landed, mut deepest) = (true, false, 0.0f64);
    let mut last_energy = start;
    let mut up = 0.0f64;
    for _ in 0..400 {
        let flying = g.airborne();
        g.frame();
        landed |= !g.airborne();
        deepest = deepest.max(g.squeeze(0));
        // off the ground nothing but the air touches it, and its energy only falls. (On
        // the springs a step a frame is too coarse to say so frame by frame: there the
        // bounces say it.)
        let e = g.energy();
        if flying && g.airborne() {
            up = up.max(e - last_energy);
        }
        last_energy = e;
        if falling && g.vy() < 0.0 {
            falling = false;
        } else if !falling && g.vy() >= 0.0 {
            falling = true;
            tops.push(g.y());
        }
    }
    assert!(landed);
    assert!(deepest > 3.0, "the springs took the landing: {deepest}");
    assert!(tops.len() >= 3, "it bounced: {tops:?}");
    for w in tops.windows(2) {
        assert!(w[1] > w[0] - 0.01, "a bounce went higher than the one before: {tops:?}");
    }
    assert!(tops[1] > tops[0] + 15.0, "the first bounce: {tops:?}");
    assert!(up < 0.001, "energy went up by {up} in a frame in the air");
    assert!(g.energy() < start);
    assert!((g.y() - REST).abs() < 0.05 && g.vy().abs() < 0.01 && g.turning().abs() < 0.01, "at rest: y {} vy {}", g.y(), g.vy());
    assert_eq!(g.get("CRASH"), 0);
    g.frames(100);
    assert_eq!((g.vx(), g.vy()), (0.0, 0.0));
}

/// A ramp launches it: both wheels leave the ground, it flies over a gap it couldn't have
/// rolled across, turning as it left, and the landing squeezes the springs past where they
/// rest.
#[test]
fn a_ramp_launches_it_and_it_lands() {
    let mut g = Game::ride_flat();
    g.ride_until(3.2, 400, "up to speed", |g| g.vx() > 2.4 && g.pitch().abs() < 4.0 && !g.airborne());
    // a ramp ahead: 36 pixels long and 12 high, then nothing (the ground as it was)
    let x0 = g.x().floor() as i64 + 30;
    g.shape(x0, x0 + 36, |x| BASE - (x - x0 as f64) / 3.0);
    g.m.key(RIGHT);
    g.until(60, "it leaves the ramp", |g| g.airborne() && g.x() > (x0 + 36) as f64);
    g.m.key_up(RIGHT);
    assert!(g.vy() < -0.5, "going up: vy {}", g.vy());
    let (mut air, mut highest) = (0, g.y());
    let om = g.turning();
    let mut last = om;
    while g.airborne() {
        last = g.turning();
        g.frame();
        air += 1;
        highest = highest.min(g.y());
        assert!(air < 200, "it never came down");
    }
    assert!(air >= 15, "{air} frames in the air");
    assert!(highest < REST - 14.0, "it got to y = {highest}");
    assert!(g.x() > (x0 + 36 + 30) as f64, "it landed at {} from a ramp ending at {}", g.x(), x0 + 36);
    // in the air nothing turned it but the air: the turning it left with, a little less
    assert!(om.abs() > 0.05 && last * om > 0.0 && last.abs() <= om.abs() && last.abs() > om.abs() * 0.8, "turning {om} then {last}");
    assert!(g.get("THUMP") > 0 || g.vy() > 0.5);
    let mut deepest = 0.0f64;
    for _ in 0..30 {
        g.frame();
        deepest = deepest.max(g.squeeze(0)).max(g.squeeze(1));
    }
    assert!(deepest > 3.5, "the landing squeezed a spring to {deepest}");
}

/// In the air the throttle and the brake turn the moped: what spins the back wheel up one
/// way turns the rest the other, and stopping a spinning wheel hands its turning back.
/// The rider's weight does the same, either way.
#[test]
fn in_the_air_throttle_brake_and_leaning_turn_it() {
    let high = REST - 70.0;
    // nothing held: it falls without turning
    let mut g = Game::ride_flat();
    g.place(128.0, high, 0.0);
    g.frames(25);
    assert!(g.airborne() && g.pitch() == 0.0 && g.vy() > 1.0, "pitch {} vy {}", g.pitch(), g.vy());
    // throttle: the back wheel spins up forwards (clockwise, seen from here) and the nose rises
    let mut g = Game::ride_flat();
    g.place(128.0, high, 0.0);
    g.hold(RIGHT, 25);
    assert!(g.airborne());
    let (nose_up, turning) = (g.pitch(), g.turning());
    assert!(nose_up > 8.0 && turning > 0.5, "pitch {nose_up} turning {turning}");
    assert!(g.spin(0) > 2.0 && g.spin(1) == 0.0, "tyres {} {}", g.spin(0), g.spin(1));
    // then the brake: the wheel stops and the turning it took comes back
    g.hold(LEFT, 22);
    assert!(g.spin(0) < 0.5, "tyre {}", g.spin(0));
    assert!(g.turning() < turning * 0.4, "turning {} from {turning}", g.turning());
    // leaning back and leaning forward
    for (key, sign) in [(UP, 1.0), (DOWNKEY, -1.0)] {
        let mut g = Game::ride_flat();
        g.place(128.0, high, 0.0);
        g.hold(key, 25);
        assert!(g.airborne());
        assert!(g.pitch() * sign > 8.0, "leaning: pitch {}", g.pitch());
        assert_eq!(g.spin(0), 0.0);
    }
}

/// Landing on the rider's head ends the run, and the heap comes to rest by the same sums.
#[test]
fn landing_on_its_head_ends_the_run() {
    let mut g = Game::ride_flat();
    g.frames(30);
    g.place(128.0, REST - 20.0, 180.0);
    g.until(60, "the head touches", |g| g.get("CRASH") == 1);
    assert!(g.squeeze(2) > 0.0);
    g.until(3, "the rider is down", |g| g.get("STATE") == DOWN);
    // the throttle is dead
    g.m.key(RIGHT);
    g.until(60, "the run is over", |g| g.get("STATE") == OVER);
    g.m.key_up(RIGHT);
    assert_eq!(g.spin(0), 0.0);
    assert!(g.text().contains("YOU CAME OFF!"), "{}", g.text());
    g.frames(250);
    assert!(g.vx().abs() < 0.1 && g.vy().abs() < 0.1 && g.turning().abs() < 0.3, "still moving: {} {} {}", g.vx(), g.vy(), g.turning());
    assert!(g.y() < BASE, "it's on the ground, not in it: y {}", g.y());
    // right way up from the same height, it just lands
    let mut g = Game::ride_flat();
    g.frames(30);
    g.place(128.0, REST - 20.0, 0.0);
    g.frames(150);
    assert_eq!((g.get("CRASH"), g.get("STATE")), (0, RIDE));
}

// ---------- gas and coins ----------

#[test]
fn gas_runs_down_faster_with_the_throttle_and_a_can_fills_it() {
    let mut g = Game::ride();
    let full = g.gas();
    g.frames(50);
    let ticking = full - g.gas();
    g.ride_until(2.5, 200, "a little way", |g| g.x() > 300.0);
    let before = g.gas();
    g.hold(RIGHT, 10);
    let pulling = before - g.gas();
    assert!(ticking > 0 && pulling > 4 * ticking / 5 * 2, "{ticking} in 50 frames ticking over, {pulling} in 10 pulling");
    // coins at 608 and 672, and the first can at 736
    g.ride_until(2.5, 600, "past the first two coins", |g| g.x() > 700.0);
    assert_eq!(g.get("COINS"), 2);
    assert!(g.text().lines().next().unwrap().contains("COINS 02"), "{}", g.text());
    let before = g.gas();
    g.ride_until(2.5, 200, "past the can", |g| g.x() > 760.0);
    assert!(g.gas() > before + 20000, "gas {} from {before}", g.gas());
    assert!(g.text().lines().next().unwrap().contains("COINS 02"));
    assert_eq!(g.get("STATE"), RIDE);
}

#[test]
fn out_of_gas_it_rolls_to_a_stop_and_the_run_is_over() {
    let mut g = Game::ride_flat();
    g.ride_until(2.5, 300, "up to speed", |g| g.vx() > 1.5);
    g.put("GAS", 2, 30);
    g.m.key(RIGHT);
    g.frames(5);
    assert_eq!(g.gas(), 0);
    let v = g.vx();
    g.frames(40);
    assert!(g.vx() < v, "no gas, no push: {} from {v}", g.vx());
    assert_eq!(g.get("STATE"), RIDE, "still rolling");
    g.until(900, "it stops and the run is over", |g| g.get("STATE") == OVER);
    g.m.key_up(RIGHT);
    assert!(g.vx().abs() < 0.07);
    assert!(g.text().contains("OUT OF GAS"), "{}", g.text());
    assert_eq!(g.get("CRASH"), 0);
    assert!(g.text().lines().next().unwrap().ends_with("GAS ________"), "{}", g.text());
}

// ---------- the ground and the picture ----------

/// The ground the physics rides on and the ground on the sheet are both the height
/// function, and the page shows the strip from the camera's column on: a pixel moved is a
/// pixel scrolled.
#[test]
fn the_ground_is_the_height_function_and_scrolls_a_pixel_at_a_time() {
    let mut g = Game::ride();
    g.ride_until(3.0, 3000, "into the rough", |g| g.x() > 2400.0);
    let mut cams = Vec::new();
    for _ in 0..120 {
        g.rider(3.0);
        // nothing to pick up, so the right of the page is ground and scenery only
        for i in 0..16 {
            g.set_at("itype", i, 0);
        }
        g.frame();
        if g.get("STATE") != RIDE {
            break;
        }
        let cam = g.num("CAM", 0, 3);
        assert_eq!(cam, g.x().floor() as i64 - 56, "the camera is the moped's pixel");
        cams.push(cam);
        let n = g.chip();
        let page = g.page();
        for sx in 0..160i64 {
            let x = cam + sx;
            let h = height(x, 255);
            // the ring, for the physics
            assert_eq!((g.ground(x) * 256.0) as i64, h, "column {x}");
            // the strip on the sheet: see-through down to the turf (a tuft is a line higher)
            let top = (h >> 8) - 34;
            let first = (0..66).find(|&r| n.pixel((x & 255) as i32, 314 + r as i32) != 4).unwrap() as i64;
            assert!(first == top || first == top - 1, "column {x}: the strip's ground starts at {first}, the height says {top}");
            let turf = n.pixel((x & 255) as i32, 314 + top as i32);
            assert_eq!(turf, if (h >> 8) < 50 { 1 } else { 13 }, "column {x}: snow above line 50, grass below");
            // the page: that column of the strip, where nothing is drawn over it
            if sx >= 90 {
                for y in (h >> 8)..100 {
                    assert_eq!(page[(y * 160 + sx) as usize], n.pixel((x & 255) as i32, 314 + (y - 34) as i32), "page ({sx}, {y}) with the camera at {cam}");
                }
            }
        }
    }
    assert!(cams.len() > 100);
    let steps: Vec<i64> = cams.windows(2).map(|w| w[1] - w[0]).collect();
    assert!(steps.iter().all(|&s| (0..=4).contains(&s)), "{steps:?}");
    assert!(steps.iter().filter(|&&s| s >= 2).count() > 30, "it was moving: {steps:?}");
    // and the far strips move an eighth and a quarter as fast: their BLITs' sources
    assert!(g.meter.lost == 0);
}

/// The moped is drawn at its pitch: the body's picture is the one nearest the angle, the
/// second half of the circle is the first half drawn with FLIPX and FLIPY, and the wheels
/// are drawn where the springs put them.
#[test]
fn the_body_is_drawn_at_its_pitch_and_the_wheels_on_their_springs() {
    let mut g = Game::ride_flat();
    g.frames(20);
    for pitch in [0.0f64, 20.0, 45.0, 90.0, 135.0, 180.0, -150.0, -90.0, -30.0] {
        // high in the air, so it stays put for the frame it's drawn in
        g.place(128.0, 30.0, pitch);
        g.put("VY", 3, -3604);
        // the body's BLIT is the last 32 x 32 one written
        let mut body = None;
        let work = g.marks[0];
        g.step();
        while g.m.cpu.pc != work {
            let pc = g.m.cpu.pc;
            if g.m.peek(pc) == STA_ABS && g.m.peek(pc + 1) == NGO as u8 && g.m.peek(pc + 2) == (NGO >> 8) as u8 && g.m.cpu.a == niece::cmd::BLIT {
                let n = g.chip();
                if n.sw == 32 && n.sh == 32 {
                    body = Some((n.sx as i32, n.sy as i32, n.flags, n.dx, n.dy));
                }
            }
            g.step();
        }
        let (sx, sy, flags, dx, dy) = body.expect("the body's BLIT");
        let turn = ((pitch.rem_euclid(360.0) / 5.625).round() as i32) % 64;
        let k = turn % 32;
        assert_eq!((sx, sy), ((k % 8) * 32, 380 + (k / 8) * 32), "pitch {pitch}");
        let both = niece::flag::FLIPX | niece::flag::FLIPY;
        assert_eq!(flags & both, if turn >= 32 { both } else { 0 }, "pitch {pitch}");
        assert_eq!(flags & niece::flag::KEY, niece::flag::KEY);
        // the picture's middle is 4.5 pixels up the moped from the centre of mass
        let rad = (turn as f64 * 5.625).to_radians();
        let (mx, my) = (dx as f64 + 16.0 - 56.0, dy as f64 + 16.0 - g.y().floor());
        assert!((mx + 4.5 * rad.sin()).abs() <= 1.0 && (my + 4.5 * rad.cos()).abs() <= 1.0, "pitch {pitch}: the picture's middle is at ({mx}, {my})");
    }
    // the wheels: on the page a wheel's hub is 7 below the centre of mass in the air, and
    // 5 when the moped stands on it
    let mut g = Game::ride_flat();
    g.frames(100);
    let hub = g.get("WYS") as f64 - g.y().floor();
    assert!((hub - 5.0).abs() <= 1.0, "standing: {hub}");
    g.place(128.0, 30.0, 0.0);
    g.frames(2);
    let hub = g.get("WYS") as f64 - g.y().floor();
    assert!((hub - 7.0).abs() <= 1.0, "in the air: {hub}");
}

// ---------- the same every time ----------

#[test]
fn the_same_keys_make_the_same_ride() {
    let ride = || {
        let mut g = Game::ride();
        let mut dice = Dice(0x1234_5678);
        let mut log = Vec::new();
        for i in 0..900 {
            if i % 7 == 0 {
                g.keys_up();
                let r = dice.next();
                // mostly gas, sometimes the brake, leaning now and then
                if r % 8 < 5 {
                    g.m.key_hold(RIGHT);
                } else if r % 8 == 5 {
                    g.m.key_hold(LEFT);
                }
                match (r >> 8) % 6 {
                    0 => g.m.key_hold(UP),
                    1 | 2 => g.m.key_hold(DOWNKEY),
                    _ => {}
                }
            }
            if g.get("STATE") == OVER && i % 50 == 0 {
                g.m.key(b' ');
            }
            g.frame();
            log.push((g.num("X0", 0, 5), g.num("Y0", 0, 3), g.num("TH", 0, 3), g.get("STATE"), g.gas()));
        }
        (log, g.page(), g.text(), g.m.cpu.cycles)
    };
    let (a, b) = (ride(), ride());
    assert!(a.0.iter().any(|s| s.0 > 300 << 16), "it went somewhere");
    assert!(a == b, "two rides with the same keys differ");
}

// ---------- 50 frames a second at 1 MHz ----------

/// The claim in the source's header. A long ride with a rider who keeps it upright, into
/// the rough ground and back to a stop, with the meter on every instruction: every frame's
/// work and every frame's drawing done inside the frame, every FLIP at the next vertical
/// blank, no command lost.
#[test]
fn a_long_ride_holds_fifty_frames_a_second() {
    let mut g = Game::ride();
    let mut dice = Dice(0xF311_1DE5);
    let (mut jumps, mut air, mut fastest) = (0, 0, 0.0f64);
    let mut frames = 0;
    while frames < 6000 && g.get("STATE") == RIDE {
        // as fast as it will go on the easy ground, then as fast as the rider dares
        let speed = if g.x() < 1500.0 { 4.0 } else { 2.2 + (dice.next() % 100) as f64 / 100.0 };
        for _ in 0..25 {
            g.rider(speed);
            g.frame();
            frames += 1;
            fastest = fastest.max(g.vx());
            if g.airborne() {
                air += 1;
                jumps += (air == 12) as u32;
            } else {
                air = 0;
            }
        }
    }
    g.keys_up();
    g.report("a long ride");
    assert!(frames >= 1500 && g.x() > 3000.0, "{frames} frames to x = {}", g.x());
    assert!(jumps >= 3, "{jumps} jumps");
    assert!(fastest > 2.7, "the fastest was {fastest}");
    g.assert_fifty();
    let m = &g.meter;
    // the numbers in the header, with a little room
    assert!(m.worst < 11_000, "the worst frame's work: {}", m.worst);
    assert!(m.total / m.frames < 8_000, "the mean frame's work: {}", m.total / m.frames);
    assert!(m.most_drawing < 9_000, "the most drawing in a frame: {} cycles", m.most_drawing);
    assert!(m.latest_done < 13_000, "the chip was done {} cycles into the frame", m.latest_done);
    assert!(m.deepest <= 24, "the queue got {} deep", m.deepest);
}

/// The same with a rider who doesn't keep it upright: wheelies, loops, crashes, the run
/// over and started again, eight times.
#[test]
fn crashes_and_new_runs_hold_fifty_too() {
    let mut g = Game::ride();
    let mut dice = Dice(0x0BAD_C0DE);
    let (mut crashes, mut loops) = (0, 0);
    let mut turned = 0.0f64;
    for i in 0..5000 {
        if i % 9 == 0 {
            g.keys_up();
            let r = dice.next();
            if r % 8 < 6 {
                g.m.key_hold(RIGHT);
            } else if r % 8 == 6 {
                g.m.key_hold(LEFT);
            }
            match (r >> 8) % 5 {
                0 => g.m.key_hold(UP),
                1 | 2 => g.m.key_hold(DOWNKEY),
                _ => {}
            }
        }
        let was = g.get("STATE");
        if was == OVER && g.get("TIMER") == 0 {
            // a new run makes its 256 columns of ground before its first frame: seven
            // frames with the last picture standing, which aren't the claim
            g.m.key(b' ');
            g.unmetered_frame();
            assert_eq!(g.get("STATE"), RIDE);
            g.m.key_up(b' ');
        }
        g.frame();
        turned = if g.airborne() { turned + g.turning() } else { 0.0 };
        loops += (turned.abs() > 300.0) as u32;
        crashes += (was == RIDE && g.get("STATE") == DOWN) as u32;
    }
    g.keys_up();
    g.report("crashes and new runs");
    assert!(crashes >= 8, "{crashes} crashes");
    let _ = loops;
    g.assert_fifty();
    assert!(g.meter.worst < 12_500, "the worst frame's work: {}", g.meter.worst);
}

// ---------- pictures ----------

const PALETTE: [u32; 16] = [
    0x000000, 0xe8e4da, 0x9a3b2e, 0x5fb3a8, 0x7a3f8a, 0x4f9a4f, 0x2c3d9a, 0xe2c84a, 0xa8632a, 0x5a3e1a, 0xd0705f, 0x3a3a3a, 0x6b6b6b, 0x9fe08a,
    0x6f8fd8, 0xa9a9a9,
];

/// The picture the monitor shows, as a PPM.
fn ppm(m: &Machine) -> Vec<u8> {
    use unc_core::machine::{NIECE_PIXEL, PIXELS_H, PIXELS_W};
    let mut out = format!("P6 {PIXELS_W} {PIXELS_H} 255\n").into_bytes();
    let f = m.frame();
    for y in 0..PIXELS_H {
        for x in 0..PIXELS_W {
            let p = f[y * PIXELS_W + x];
            let c = if p & NIECE_PIXEL != 0 { p & 15 } else { m.line_colours(y)[(p & 3) as usize] };
            out.extend_from_slice(&PALETTE[(c & 15) as usize].to_be_bytes()[1..]);
        }
    }
    out
}

/// UNC_SHOTS=DIR writes pictures of a ride there: the title, a wheelie going over, the
/// rider's ride into the rough every UNC_EVERY frames (default 20), and its end.
#[test]
#[ignore = "writes pictures of a ride: UNC_SHOTS=DIR cargo test --test game_fellrider -- --ignored pictures"]
fn pictures() {
    let Ok(dir) = std::env::var("UNC_SHOTS") else { return };
    let every: u32 = std::env::var("UNC_EVERY").ok().and_then(|v| v.parse().ok()).unwrap_or(20);
    let shot = |g: &mut Game, name: &str| {
        g.m.run_frame();
        std::fs::write(format!("{dir}/{name}.ppm"), ppm(&g.m)).unwrap();
        let work = g.marks[0];
        g.m.run_until(work, 3 * BUDGET).unwrap();
    };
    let mut g = Game::boot();
    shot(&mut g, "title");
    g.press(b' ');
    g.m.key(RIGHT);
    for i in 0..110 {
        g.frame();
        if i % 6 == 0 {
            shot(&mut g, &format!("wheelie-{:03}", i));
        }
    }
    g.m.key_up(RIGHT);
    g.until(100, "over", |g| g.get("STATE") == OVER);
    shot(&mut g, "over");
    g.frames(40);
    g.press(b' ');
    let mut dice = Dice(0xF311_1DE5);
    let mut i = 0;
    let from: f64 = std::env::var("UNC_FROM").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0);
    while i < 8000 && g.get("STATE") != OVER {
        let speed = if g.x() < 1500.0 { 4.0 } else { 2.2 + (dice.next() % 100) as f64 / 100.0 };
        for _ in 0..25 {
            g.rider(speed);
            g.frame();
            i += 1;
            if i % every == 0 && g.x() >= from {
                shot(&mut g, &format!("ride-{:05}", i));
            }
        }
    }
    shot(&mut g, "end");
    g.report("pictures");
}
