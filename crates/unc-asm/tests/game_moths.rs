//! MOTHS (games/niece/moths.s): a fixed shooter for the NIECE, the prototype graphics chip
//! of manual chapter 17. Played through scripted keys on a booted 64K machine with the chip
//! in; its state is read from the addresses the source's symbol table gives.
//!
//! The question the game is there to answer is whether a 1 MHz processor can keep a NIECE
//! busy at 50 frames a second, so the tests measure both halves: the processor's work
//! (`work` to `endwork`) and the chip's drawing (every command written to NGO, costed as
//! chapter 17 costs it), and that every frame's FLIP happens at the very next vertical blank.

use unc_core::machine::{CYCLES_PER_FRAME, CYCLES_PER_LINE, VBLANK_LINE};
use unc_core::niece::{self, Command};
use unc_core::{CpuState, Machine, RamSize, Stop};

const SRC: &str = include_str!("../../../games/niece/moths.s");
const BUDGET: u64 = CYCLES_PER_FRAME as u64;

const TITLE: u8 = 1;
const PLAY: u8 = 2;
const OVER: u8 = 3;

const LEFT: u8 = 28;
const RIGHT: u8 = 29;

const NGO: u16 = 0xC103;
const STA_ABS: u8 = 0x33;

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
    unc_asm::assemble(SRC).unwrap_or_else(|e| panic!("moths.s doesn't assemble: {e:?}"))
}

impl Game {
    /// A UNC 64 with the NIECE, booted to READY., with MOTHS loaded and started as the hosts
    /// start a program, and run until the sheet is drawn and the title is up.
    fn boot() -> Game {
        let mut g = Game::boot_niece(true);
        g.until(200, "the title", |g| g.get("STATE") == TITLE && g.chip().shown());
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
        Game { m, asm, meter: Meter::default(), work, endwork }
    }

    /// Boots and presses SPACE on the title.
    fn play() -> Game {
        let mut g = Game::boot();
        g.press(b' ');
        g.until(5, "SPACE starts a game", |g| g.get("STATE") == PLAY);
        g.frames(2);
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
        panic!("{what}: not within {max} frames\n{}", self.text());
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

    /// Keeps the ranks full and the player unhurt, so a frame is as full as frames get.
    fn refill(&mut self) {
        for i in 0..32 {
            if self.at("MST", i) == 0 {
                self.set_at("MST", i, 1);
            }
        }
        self.set("LEFT", 32);
        self.set("INVUL", 0);
        self.set("PDEAD", 0);
        self.set("LIVES", 3);
    }

    /// A speck of dust on the lantern-carrier's head.
    fn dust_him(&mut self) {
        self.set("INVUL", 0);
        let (x, y) = (self.get("PX"), self.sym("PTOP") as u8 + 4);
        self.set_at("DUX", 0, x);
        self.set_at("DUY", 0, y);
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
    assert!(text.contains("MOTHS NEEDS THE NIECE."), "{text}");
    assert!(text.contains("SWITCH OFF, FIT IT, TRY AGAIN."));
    assert_eq!(g.m.cpu.state, CpuState::Running, "waiting, not halted or jammed");
    g.m.key(b' ');
    for _ in 0..10 {
        g.m.run_frame();
    }
    assert!(g.text().contains("MOTHS NEEDS THE NIECE."));
    assert_eq!(g.m.cpu.state, CpuState::Running);
}

// ---------- the sheet ----------

#[test]
fn the_sheet_holds_the_pictures_and_three_backgrounds_that_join_up() {
    let g = Game::boot();
    let n = g.chip();
    // the small moth, where the table says: its first row is 88 4444 4444 88
    let (x, key) = (g.sym("X_MSA") as i32, g.sym("KEYC") as u8);
    let row: Vec<u8> = (0..12).map(|i| n.pixel(x + i, 0)).collect();
    assert_eq!(row, [8, 8, key, key, key, key, key, key, key, key, 8, 8]);
    // the sky: black at the top, stars in it, and not a pixel of the pages' area touched
    let sky = g.sym("SKYY") as i32;
    assert!((0..256).all(|x| n.pixel(x, sky + 71) != 0), "the sunset along the bottom");
    let stars = (0..256).flat_map(|x| (0..12).map(move |y| (x, y))).filter(|&(x, y)| n.pixel(x, sky + y) != 0).count();
    assert!(stars > 5, "{stars} stars in the black");
    // the trees: black along the bottom all the way across, see-through along the top
    let trees = g.sym("TREEY") as i32;
    assert!((0..256).all(|x| n.pixel(x, trees + 39) == 0));
    assert!((0..256).filter(|&x| n.pixel(x, trees + 2) == key).count() > 200);
    assert!((0..256).any(|x| n.pixel(x, trees + 8) == 0), "a treetop");
    // the ground: see-through above the grass, grass, then the path
    let ground = g.sym("GRNDY") as i32;
    assert!((0..256).any(|x| n.pixel(x, ground) == key));
    assert!((0..256).filter(|&x| n.pixel(x, ground + 10) == 5).count() > 150, "grass");
    assert!((0..256).filter(|&x| n.pixel(x, ground + 24) == 9).count() > 150, "the path");
    // the ink tables: 1 all white, 2 and 3 other moths from the same picture
    assert_eq!(n.inks[1], [1; 16]);
    assert_ne!(n.inks[2][8], 8);
    assert_ne!(n.inks[3][8], n.inks[2][8]);
    assert_eq!(n.key, key);
}

// ---------- the title ----------

#[test]
fn the_title_draws_its_name_and_waits_for_space() {
    let mut g = Game::boot();
    g.frames(20);
    assert_eq!(g.get("STATE"), TITLE);
    let text = g.text();
    assert!(text.contains("SCORE 000000"), "{text}");
    assert!(text.contains("HI 000000"));
    assert!(text.contains("A DEMONSTRATION OF THE NIECE"));
    // PRESS SPACE blinks
    let mut seen = (false, false);
    for _ in 0..80 {
        g.frame();
        let on = g.text().contains("PRESS SPACE");
        seen = (seen.0 || on, seen.1 || !on);
    }
    assert_eq!(seen, (true, true));
    // the page is shown, and it's a picture: the letters' yellow, the grass, the sky's blue
    assert!(g.chip().shown());
    let px = g.page();
    for c in [7u8, 5, 6, 0] {
        assert!(px.iter().filter(|&&p| p == c).count() > 100, "colour {c}");
    }
    // the background moves by itself, the far things less than the near
    let a = g.page();
    g.frames(25);
    assert_ne!(a, g.page());
    // other keys don't start it
    g.press(b'X');
    g.frames(3);
    assert_eq!(g.get("STATE"), TITLE);
    g.press(b' ');
    g.until(3, "SPACE starts", |g| g.get("STATE") == PLAY);
    g.frames(2);
    assert!(!g.text().contains("PRESS SPACE"));
    assert!(!g.text().contains("DEMONSTRATION"));
    g.report("the title");
    g.assert_fifty();
}

// ---------- playing ----------

#[test]
fn a_new_game_has_three_lanterns_and_thirty_two_moths() {
    let mut g = Game::play();
    assert_eq!(g.get("LIVES"), 3);
    assert_eq!(g.score(), 0);
    assert_eq!(g.get("LEFT"), 32);
    assert_eq!(g.count("MST", 32), 32);
    assert!(g.text().contains("WAVE  1"), "{}", g.text());
    assert!(g.text().contains("LANTERNS 3"));
    // nothing swoops while the banner is up
    g.frames(50);
    assert_eq!(g.count("DMI", 6), 0);
    g.until(100, "the banner goes", |g| g.get("PHASE") == 1);
    assert!(!g.text().contains("WAVE"));
    // 32 moths, the lantern's light, its carrier and three backgrounds: 37 commands
    g.meter = Meter::default();
    g.frames(3);
    assert!(g.meter.most_commands >= 37, "{} commands", g.meter.most_commands);
}

#[test]
fn he_walks_with_the_cursor_keys_and_a_and_d_and_stops_at_the_edges() {
    let mut g = Game::play();
    assert_eq!(g.get("PX"), 80);
    for (k, dx) in [(RIGHT, 2i32), (LEFT, -2), (b'D', 2), (b'A', -2)] {
        let x = g.get("PX") as i32;
        g.hold(k, 10);
        assert_eq!(g.get("PX") as i32, x + 10 * dx, "key {k}");
        // he faces the way he walks: FLIPX when it's left
        assert_eq!(g.get("PFACE") != 0, dx < 0);
    }
    g.hold(LEFT, 60);
    assert_eq!(g.get("PX"), g.sym("PXMIN") as u8);
    g.hold(RIGHT, 100);
    assert_eq!(g.get("PX"), g.sym("PXMAX") as u8);
    // walking and firing at once: the matrix
    g.set("PX", 80);
    g.m.key(LEFT);
    g.m.key(b' ');
    g.frames(12);
    g.m.key_up(LEFT);
    g.m.key_up(b' ');
    assert_eq!(g.get("PX"), 56);
    assert!(g.count("SHY", 4) >= 1);
}

#[test]
fn the_background_moves_as_he_walks_the_near_more_than_the_far() {
    // What the three BLITs of the background took from: NSX of each, read off the queue's
    // commands as they are written.
    fn sources(g: &mut Game) -> Vec<u8> {
        let mut v = vec![];
        let end = g.m.cpu.cycles + to_vblank(&g.m) as u64;
        while g.m.cpu.cycles < end {
            let pc = g.m.cpu.pc;
            let go = g.m.peek(pc) == STA_ABS && g.m.peek(pc + 1) == NGO as u8 && g.m.peek(pc + 2) == 0xC1;
            if go && g.m.cpu.a == niece::cmd::BLIT && g.chip().dw == 160 {
                v.push(g.chip().sx);
            }
            g.step();
        }
        v
    }
    let mut g = Game::play();
    g.set("PX", 40);
    g.frames(2);
    let a = sources(&mut g);
    assert_eq!(a.len(), 3, "sky, trees, ground");
    g.hold(RIGHT, 32);
    let b = sources(&mut g);
    let moved: Vec<u8> = a.iter().zip(&b).map(|(a, b)| b.wrapping_sub(*a)).collect();
    assert_eq!(moved, [8, 16, 32], "64 pixels' walk: an eighth, a quarter, a half");
}

#[test]
fn a_spark_turns_a_moth_in_the_ranks_to_dust_and_scores() {
    let mut g = Game::play();
    g.set("PHASE", 1);
    g.set("LAUNCH", 255);
    g.press(b' ');
    assert_eq!(g.count("SHY", 4), 1);
    let y0 = (0..4).map(|i| g.at("SHY", i)).max().unwrap();
    g.frame();
    assert!((0..4).map(|i| g.at("SHY", i)).max().unwrap() < y0, "it goes up");
    g.until(40, "a hit", |g| g.get("LEFT") == 31);
    assert_eq!(g.count("MST", 32), 31);
    assert_eq!(g.count("SHY", 4), 0, "the spark is spent");
    // the bottom rank is worth 10, and the moth is a puff: white first, then dust
    assert_eq!(g.score(), 10);
    assert!(g.text().contains("SCORE 000010"));
    assert_eq!(g.count("PFA", 8), 1);
    g.frames(20);
    assert_eq!(g.count("PFA", 8), 0);
    // a spark that meets nothing goes off the top
    g.set_at("SHX", 0, 3);
    g.set_at("SHY", 0, 70);
    g.frames(30);
    assert_eq!(g.count("SHY", 4), 0);
    assert_eq!(g.get("LEFT"), 31);
}

#[test]
fn a_hit_moth_is_drawn_white_through_ink_table_1() {
    let mut g = Game::play();
    g.set("LAUNCH", 255);
    let white = |g: &Game| g.page()[..60 * 160].iter().filter(|&&p| p == 1).count();
    g.frames(3);
    let before = white(&g);
    g.press(b' ');
    g.until(40, "a hit", |g| g.get("LEFT") == 31);
    g.frames(2);
    assert!(white(&g) > before + 20, "{} white pixels, {before} before", white(&g));
}

#[test]
fn holding_space_keeps_sparks_coming_four_at_most() {
    let mut g = Game::play();
    g.set("PX", 150); // under nothing: the ranks start at the left
    g.set("FX", 2);
    g.set("FDIR", 0);
    g.m.key(b' ');
    let mut most = 0;
    for _ in 0..60 {
        g.frame();
        most = most.max(g.count("SHY", 4));
    }
    g.m.key_up(b' ');
    assert!((2..=4).contains(&most), "{most}");
}

#[test]
fn a_swooping_moth_grows_as_it_comes_down_and_shrinks_as_it_climbs() {
    let mut g = Game::play();
    g.until(120, "the banner goes", |g| g.get("PHASE") == 1);
    g.set("PX", 80);
    g.set("LAUNCH", 1);
    g.frame();
    g.until(3, "a swoop", |g| g.count("DMI", 6) == 1);
    let k = (0..6).find(|&i| g.at("DMI", i) != 0).unwrap();
    let moth = g.at("DMI", k) as u16 - 1;
    assert_eq!(g.at("MST", moth), 2);
    g.set("LAUNCH", 255);
    let mut seen: Vec<(u8, u8)> = vec![];
    for _ in 0..200 {
        g.set("INVUL", 50);
        g.frame();
        if g.at("DMI", k) == 0 {
            break;
        }
        seen.push((g.at("DCY", k), g.at("DCW", k)));
    }
    assert_eq!(g.at("MST", moth), 1, "back in the ranks");
    let lowest = seen.iter().map(|s| s.0).max().unwrap();
    let widest = seen.iter().map(|s| s.1).max().unwrap();
    assert!(lowest >= g.sym("DEPTH") as u8 - 2, "down to {lowest}");
    assert!(widest >= 34, "nearly three times the size at the bottom: {widest}");
    assert!(seen[0].1 <= 14 && seen.last().unwrap().1 <= 14, "small at both ends: {seen:?}");
    // the width is the height's, from the table
    for &(y, w) in &seen {
        assert_eq!(w, g.at("SZW", y as u16), "at line {y}");
    }
    assert!(seen.windows(2).any(|w| w[1].1 > w[0].1) && seen.windows(2).any(|w| w[1].1 < w[0].1));
}

#[test]
fn a_swooping_moth_can_be_shot_for_a_hundred_and_its_box_grows_with_it() {
    let mut g = Game::play();
    g.until(120, "the banner goes", |g| g.get("PHASE") == 1);
    g.set("PX", 80);
    g.set("LAUNCH", 1);
    g.frame();
    g.until(3, "a swoop", |g| g.count("DMI", 6) == 1);
    g.set("LAUNCH", 255);
    let k = (0..6).find(|&i| g.at("DMI", i) != 0).unwrap();
    g.until(100, "it's big", |g| g.at("DCW", k) >= 30);
    // a spark 12 pixels to one side of its middle: a miss on a moth in the ranks, a hit now
    let (x, y) = (g.at("DCXL", k), g.at("DCY", k));
    g.set("INVUL", 50);
    g.set_at("SHX", 0, x + 12);
    g.set_at("SHY", 0, y + 4);
    g.frame();
    assert_eq!(g.at("DMI", k), 0, "hit");
    assert_eq!(g.get("LEFT"), 31);
    assert_eq!(g.score(), 100);
}

#[test]
fn a_moth_that_reaches_him_costs_a_lantern() {
    let mut g = Game::play();
    g.until(120, "the banner goes", |g| g.get("PHASE") == 1);
    g.set("PX", 80);
    g.set("LAUNCH", 1);
    g.frame();
    g.until(3, "a swoop", |g| g.count("DMI", 6) == 1);
    g.set("LAUNCH", 255);
    // he stands still, and it was aimed at him
    g.until(150, "a hit", |g| g.get("PDEAD") != 0);
    assert_eq!(g.get("LIVES"), 2);
    assert!(g.text().contains("LANTERNS 2"));
    let n = g.until(200, "the next lantern", |g| g.get("PDEAD") == 0);
    assert!(n > 30);
    assert!(g.get("INVUL") > 0, "blinking and safe");
}

#[test]
fn the_emperor_comes_when_the_ranks_are_empty_and_its_fall_starts_the_next_wave() {
    let mut g = Game::play();
    g.until(120, "the banner goes", |g| g.get("PHASE") == 1);
    for i in 0..32 {
        g.set_at("MST", i, 0);
    }
    g.set("LEFT", 0);
    g.frames(2);
    assert_eq!((g.get("PHASE"), g.get("BST")), (2, 1));
    assert!(g.text().contains("THE EMPEROR"));
    assert_eq!(g.count("EST", 8), 8, "with its ring");
    // it comes nearer and goes away: its width goes from under 48 to over 80 and back
    let mut widths = vec![];
    for _ in 0..450 {
        g.set("INVUL", 50);
        g.set("BDROP", 200);
        g.frame();
        widths.push(g.get("BW"));
    }
    let (lo, hi) = (*widths.iter().min().unwrap(), *widths.iter().max().unwrap());
    assert!(lo <= 44 && hi >= 84, "{lo} to {hi}");
    // a spark in its middle: white for a few frames, and one less to go
    let hp = g.get("BHP");
    let (x, y) = (g.get("BX"), g.get("BY"));
    g.set_at("SHX", 0, x);
    g.set_at("SHY", 0, y);
    for i in 0..8 {
        g.set_at("EST", i, 0); // its ring out of the way
    }
    g.frame();
    assert_eq!(g.get("BHP"), hp - 1);
    assert!(g.get("BFL") > 0);
    assert_eq!(g.score(), 10);
    // the last one
    g.set("BHP", 1);
    g.frames(6);
    let (x, y) = (g.get("BX"), g.get("BY"));
    g.set_at("SHX", 0, x);
    g.set_at("SHY", 0, y);
    g.frame();
    assert_eq!(g.get("BST"), 2, "falling to dust");
    g.until(200, "the next wave", |g| g.get("LEVEL") == 2);
    assert_eq!(g.score(), 1020);
    assert_eq!(g.get("LEFT"), 32);
    assert!(g.text().contains("WAVE  2"), "{}", g.text());
}

#[test]
fn the_last_lantern_is_game_over_and_space_starts_again() {
    let mut g = Game::play();
    g.set("LAUNCH", 255);
    g.press(b' ');
    g.until(40, "10 points", |g| g.score() == 10);
    g.set("LIVES", 1);
    g.dust_him();
    g.until(5, "dust on him", |g| g.get("PDEAD") != 0);
    g.until(200, "game over", |g| g.get("STATE") == OVER);
    assert!(g.text().contains("GAME OVER"));
    assert!(g.text().contains("LANTERNS 0"));
    assert!(g.text().contains("HI 000010"), "the best so far:\n{}", g.text());
    // not straight away: he may still have had his thumb on SPACE
    g.press(b' ');
    g.frames(2);
    assert_eq!(g.get("STATE"), OVER);
    g.frames(60);
    g.press(b' ');
    g.until(3, "a new game", |g| g.get("STATE") == PLAY);
    g.frames(2);
    assert_eq!((g.get("LIVES"), g.score(), g.get("LEFT"), g.get("LEVEL")), (3, 0, 32, 1));
    assert!(!g.text().contains("GAME OVER"));
    assert!(g.text().contains("HI 000010"), "kept");
    g.report("a game, game over and another");
    g.assert_fifty();
}

#[test]
fn game_over_goes_back_to_the_title_by_itself() {
    let mut g = Game::play();
    g.set("LIVES", 1);
    g.dust_him();
    g.until(200, "game over", |g| g.get("STATE") == OVER);
    g.until(600, "the title", |g| g.get("STATE") == TITLE);
    g.frames(40);
    assert!(g.text().contains("A DEMONSTRATION OF THE NIECE"));
    assert!(!g.text().contains("GAME OVER"));
    g.assert_fifty();
}

// ---------- 1 MHz ----------

#[test]
fn the_fullest_swarm_runs_at_fifty_frames_a_second() {
    // As full as it gets: wave 8's numbers (six swooping at once, the fastest), the ranks
    // filled up again as fast as they're shot, four sparks in the air, eight puffs, dust.
    let mut g = Game::play();
    g.set("LEVEL", 8);
    g.set("MAXD", 6);
    g.set("LINT", 6);
    g.set("LAUNCH", 1);
    g.set("DUSTP", 60);
    g.until(120, "the banner goes", |g| g.get("PHASE") == 1);
    g.meter = Meter::default();
    let mut d = Dice(1985);
    let (mut most_things, mut most_divers) = (0, 0);
    g.m.key(b' ');
    for i in 0..1500u32 {
        g.refill();
        if i % 20 == 0 {
            g.m.key_up(LEFT);
            g.m.key_up(RIGHT);
            match d.next() % 3 {
                0 => g.m.key(LEFT),
                1 => g.m.key(RIGHT),
                _ => {}
            }
        }
        g.frame();
        assert_eq!(g.get("STATE"), PLAY);
        let divers = g.count("DMI", 6);
        // moths in the ranks or swooping, puffs, sparks, dust, the lantern-carrier and his light
        let things = 32 + g.count("PFA", 8) + g.count("SHY", 4) + g.count("DUY", 6) + 2;
        most_things = most_things.max(things);
        most_divers = most_divers.max(divers);
    }
    g.report("the fullest swarm");
    eprintln!("the fullest swarm: {most_things} things on the screen at once, {most_divers} of them swooping");
    assert_eq!(most_divers, 6);
    assert!(most_things >= 45, "{most_things} things");
    assert!(g.score() > 1000, "and it was a game: {} points", g.score());
    g.assert_fifty();
}

#[test]
fn the_emperor_runs_at_fifty_frames_a_second() {
    let mut g = Game::play();
    g.until(120, "the banner goes", |g| g.get("PHASE") == 1);
    for i in 0..32 {
        g.set_at("MST", i, 0);
    }
    g.set("LEFT", 0);
    g.set("BDINT", 8);
    g.frames(2);
    g.meter = Meter::default();
    let mut d = Dice(17);
    g.m.key(b' ');
    for i in 0..900u32 {
        g.set("INVUL", 0);
        g.set("PDEAD", 0);
        g.set("LIVES", 3);
        g.set("BHP", 40);
        if i % 20 == 0 {
            g.m.key_up(LEFT);
            g.m.key_up(RIGHT);
            match d.next() % 3 {
                0 => g.m.key(LEFT),
                1 => g.m.key(RIGHT),
                _ => {}
            }
        }
        if i % 50 == 0 {
            for e in 0..8 {
                g.set_at("EST", e, 1); // the ring whole again
            }
        }
        g.frame();
        assert_eq!((g.get("STATE"), g.get("BST")), (PLAY, 1));
    }
    g.report("the EMPEROR");
    g.assert_fifty();
}

#[test]
fn a_long_game_with_random_keys_loses_no_command_and_no_frame() {
    // Random keys, held for random spells, through lost lanterns, game overs, the title and
    // new games. Every instruction goes through the meter, which sees every write to NGO.
    let mut g = Game::boot();
    let mut d = Dice(0xA5A5_1985);
    let keys = [LEFT, RIGHT, b'A', b'D', b' ', b' ', b' ', b'X', 13];
    let mut down: Vec<u8> = vec![];
    let mut states = std::collections::HashSet::new();
    let (mut games, mut best, mut waves) = (0, 0, 0);
    for _ in 0..9000 {
        if d.next() % 8 == 0 {
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
        best = best.max(g.score());
        waves = waves.max(g.get("LEVEL"));
        assert!((1..=3).contains(&g.get("STATE")));
        let px = g.get("PX");
        if g.get("STATE") == PLAY {
            assert!((g.sym("PXMIN") as u8..=g.sym("PXMAX") as u8).contains(&px), "he's at {px}");
        }
        assert!(g.get("LEFT") <= 32);
    }
    g.report("a long game");
    eprintln!("a long game: {games} games, the best {best} points, wave {waves}");
    assert!(games >= 2, "{games} games");
    assert_eq!(states.len(), 3, "title, play and game over all came round");
    assert!(best >= 100, "somebody shot something: {best}");
    g.assert_fifty();
}

/// Pictures of the game for a person to look at, as PPMs in the colour monitor's colours:
/// `MOTHS_SHOTS=dir cargo test --release -p unc-asm --test game_moths pictures -- --ignored`.
#[test]
#[ignore]
fn pictures() {
    use unc_core::machine::{NIECE_PIXEL, PIXELS_H, PIXELS_W};
    const PALETTE: [u32; 16] = [
        0x000000, 0xe8e4da, 0x9a3b2e, 0x5fb3a8, 0x7a3f8a, 0x4f9a4f, 0x2c3d9a, 0xe2c84a, 0xa8632a, 0x5a3e1a, 0xd0705f, 0x3a3a3a, 0x6b6b6b,
        0x9fe08a, 0x6f8fd8, 0xa9a9a9,
    ];
    let dir = std::env::var("MOTHS_SHOTS").unwrap_or_else(|_| "target/moths".into());
    std::fs::create_dir_all(&dir).unwrap();
    let shot = |g: &Game, name: &str| {
        let mut out = format!("P6 {PIXELS_W} {PIXELS_H} 255\n").into_bytes();
        let f = g.m.frame();
        for y in 0..PIXELS_H {
            for x in 0..PIXELS_W {
                let p = f[y * PIXELS_W + x];
                let c = if p & NIECE_PIXEL != 0 { p & 15 } else { g.m.line_colours(y)[(p & 3) as usize] };
                out.extend_from_slice(&PALETTE[(c & 15) as usize].to_be_bytes()[1..]);
            }
        }
        std::fs::write(format!("{dir}/{name}.ppm"), out).unwrap();
    };
    let mut g = Game::boot();
    g.frames(70);
    shot(&g, "1-title");
    g.press(b' ');
    g.frames(40);
    shot(&g, "2-wave");
    // a busy wave: six swooping, sparks going up
    g.set("MAXD", 6);
    g.set("LINT", 8);
    g.set("LAUNCH", 1);
    g.set("DUSTP", 40);
    g.frames(70);
    g.m.key(b' ');
    for i in 0..5 {
        for f in 0..37 {
            g.set("INVUL", 0);
            g.set("PDEAD", 0);
            if f == 10 {
                g.m.key(if i % 2 == 0 { RIGHT } else { LEFT });
            }
            g.frame();
        }
        g.m.key_up(LEFT);
        g.m.key_up(RIGHT);
        shot(&g, &format!("3-swarm{i}"));
    }
    // the EMPEROR, far and near
    for i in 0..32 {
        g.set_at("MST", i, 0);
    }
    for i in 0..6 {
        g.set_at("DMI", i, 0);
    }
    g.set("LEFT", 0);
    for i in 0..8 {
        for _ in 0..45 {
            g.set("INVUL", 0);
            g.set("PDEAD", 0);
            g.set("BHP", 30);
            g.frame();
        }
        shot(&g, &format!("4-emperor{i}"));
    }
    g.set("BHP", 1);
    g.until(200, "its fall", |g| g.get("BST") == 2);
    g.frames(30);
    shot(&g, "5-fall");
    g.m.key_up(b' ');
    g.set("LIVES", 1);
    g.dust_him();
    g.until(300, "game over", |g| g.get("STATE") == OVER);
    g.frames(30);
    shot(&g, "6-over");
}

/// Where a frame of the fullest swarm goes, routine by routine, in cycles a frame:
/// `cargo test --release -p unc-asm --test game_moths profile -- --ignored --nocapture`.
#[test]
#[ignore]
fn profile() {
    let mut g = Game::play();
    g.set("LEVEL", 8);
    g.set("MAXD", 6);
    g.set("LINT", 6);
    g.set("LAUNCH", 1);
    g.set("DUSTP", 60);
    g.until(120, "the banner goes", |g| g.get("PHASE") == 1);
    let names = [
        "main", "work", "backdrop", "input", "shots", "swarm", "boss", "dust", "phase", "dform", "dboss", "dpuffs", "dplayer", "dshots",
        "ddust", "ddivers", "hud",
    ];
    let at: Vec<u16> = names.iter().map(|n| g.sym(n)).collect();
    let mut spent = vec![0u64; names.len()];
    let mut now = 0;
    g.m.key(b' ');
    let frames = 600;
    for _ in 0..frames {
        g.refill();
        let end = g.m.cpu.cycles + to_vblank(&g.m) as u64;
        while g.m.cpu.cycles < end {
            if let Some(i) = at.iter().position(|&a| a == g.m.cpu.pc) {
                now = i;
            }
            let t = g.m.cpu.cycles;
            g.step();
            spent[now] += g.m.cpu.cycles - t;
        }
    }
    for (n, c) in names.iter().zip(&spent) {
        eprintln!("{n:>9} {:>6}", c / frames);
    }
}
