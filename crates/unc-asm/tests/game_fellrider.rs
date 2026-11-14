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
