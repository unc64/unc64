//! FROSTHALL (games/niece/frosthall.s): the NIECE's raycaster. Walked through scripted keys
//! on a booted 64K machine with the chip; its state is read through the symbol table.

use unc_core::machine::{CYCLES_PER_FRAME, NIECE_PIXEL, PIXELS_W};
use unc_core::niece;
use unc_core::{Machine, RamSize, Stop};

const SRC: &str = include_str!("../../../games/niece/frosthall.s");

const LEFT: u8 = 28;
const RIGHT: u8 = 29;
const UP: u8 = 30;
const DOWN: u8 = 31;

struct Hall {
    m: Machine,
    asm: unc_asm::Assembly,
}

/// What one picture cost: the processor's cycles for all of it, for the walls and for the
/// things, and the commands the NIECE was given.
#[derive(Debug, Clone, Copy)]
struct Picture {
    cycles: u64,
    walls: u64,
    things: u64,
}

impl Hall {
    fn boot() -> Hall {
        let asm = unc_asm::assemble(SRC).unwrap_or_else(|e| panic!("frosthall.s doesn't assemble: {e:?}"));
        let mut m = Machine::boot(RamSize::K64);
        m.set_niece(true);
        unc_asm::load(&mut m, &asm);
        let mut h = Hall { m, asm };
        // start-up: the pictures go onto the sheet
        h.to("main", 40);
        h
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

    /// Runs until the program is at `label`, within `frames` frames. Returns the cycles.
    fn to(&mut self, label: &str, frames: u64) -> u64 {
        let pc = self.sym(label);
        if self.m.cpu.pc == pc {
            self.m.step();
        }
        self.m.run_until(pc, frames * CYCLES_PER_FRAME as u64).unwrap_or_else(|s| panic!("never reached {label}: {s:?}"))
    }

    /// One picture, from `main` to `main`.
    fn picture(&mut self) -> Picture {
        assert_eq!(self.m.cpu.pc, self.sym("main"));
        let t0 = self.m.cpu.cycles;
        self.to("cast", 20);
        let t1 = self.m.cpu.cycles;
        self.to("things", 20);
        let t2 = self.m.cpu.cycles;
        self.to("panelf", 20);
        let t3 = self.m.cpu.cycles;
        self.to("main", 20);
        Picture { cycles: self.m.cpu.cycles - t0, walls: t2 - t1, things: t3 - t2 }
    }

    /// Puts you at (x, y) in cells and 256ths, facing `angle`.
    fn stand(&mut self, x: f32, y: f32, angle: u8) {
        let (x, y) = ((x * 256.0) as u16, (y * 256.0) as u16);
        self.set("PXL", x as u8);
        self.set("PXH", (x >> 8) as u8);
        self.set("PYL", y as u8);
        self.set("PYH", (y >> 8) as u8);
        self.set("ANG", angle);
    }

    fn lost(&self) -> bool {
        self.m.mem.niece.as_ref().unwrap().lost
    }
}

/// The places `what_a_picture_costs` and the timing test stand in: x, y, facing.
const VIEWS: [(f32, f32, u8); 9] =
    [(3.5, 2.5, 0), (3.5, 2.5, 64), (3.5, 2.5, 128), (5.5, 9.5, 0), (15.5, 7.5, 0), (15.5, 7.5, 96), (24.5, 14.5, 200), (8.5, 21.5, 20), (17.5, 13.5, 0)];

impl Hall {
    /// Draws a picture from where you stand and returns it once it's shown: the page's
    /// 160 x 100 colours.
    fn look(&mut self) -> Vec<u8> {
        self.picture();
        self.picture();
        // the second picture's FLIP, and nothing of the third yet
        let shown = self.m.mem.niece.as_ref().unwrap().page;
        for _ in 0..3 * CYCLES_PER_FRAME {
            self.m.step();
            if self.m.mem.niece.as_ref().unwrap().page != shown {
                break;
            }
        }
        let n = self.m.mem.niece.as_ref().unwrap();
        let top = n.page as i32 * 100;
        let page: Vec<u8> = (0..100).flat_map(|y| (0..160).map(move |x| (x, y))).map(|(x, y)| n.pixel(x, top + y)).collect();
        self.to("main", 20);
        page
    }

    fn depth(&self, x: usize) -> u8 {
        self.m.mem.niece.as_ref().unwrap().depth[x]
    }

    /// Holds a key down for `frames` frames.
    fn hold(&mut self, key: u8, frames: u32) {
        self.m.key(key);
        for _ in 0..frames {
            self.m.run_frame();
            assert!(!self.lost(), "a command was lost");
        }
        self.m.key_up(key);
        self.to("main", 20);
    }

    fn x(&self) -> f32 {
        (self.get("PXH") as f32 * 256.0 + self.get("PXL") as f32) / 256.0
    }
    fn y(&self) -> f32 {
        (self.get("PYH") as f32 * 256.0 + self.get("PYL") as f32) / 256.0
    }
}

#[test]
fn it_starts_in_the_hall_with_the_panel_under_it() {
    let mut h = Hall::boot();
    assert_eq!(h.m.peek(niece::BASE + niece::reg::NCTRL), 1, "the page is shown");
    let text = h.m.screen_text();
    assert!(text.contains("FROSTHALL"), "{text}");
    assert!(text.lines().take(21).all(|l| l.is_empty()), "the hall's rows are spaces: the page shows through");
    let page = h.look();
    // the panel is black, and the picture above it isn't
    assert!(page[84 * 160..].iter().all(|&c| c == 0));
    assert!(page[..84 * 160].iter().filter(|&&c| c != 0).count() > 84 * 160 / 2);
    // in the picture the monitor shows, the page is behind the text
    h.m.run_frame();
    let f = h.m.frame();
    assert!(f[..168 * PIXELS_W].iter().all(|&p| p & NIECE_PIXEL != 0), "all page above the panel");
    assert!(f[168 * PIXELS_W..].iter().any(|&p| p == 1), "the panel's text is ink");
    // a second of pictures and the count is in the panel
    for _ in 0..60 {
        h.m.run_frame();
    }
    let line = h.m.screen_text().lines().nth(22).unwrap().to_string();
    let n: u32 = line.split_whitespace().rev().nth(1).unwrap().parse().unwrap_or_else(|_| panic!("{line}"));
    assert!((14..=30).contains(&n), "{line}");
}

#[test]
fn without_the_niece_it_says_so() {
    let asm = unc_asm::assemble(SRC).unwrap();
    let mut m = unc_asm::boot(RamSize::K64, &asm);
    assert_eq!(m.run_frame(), Stop::Halted);
    assert!(m.screen_text().contains("FROSTHALL NEEDS THE NIECE."));
}

#[test]
fn a_flat_wall_straight_ahead_is_flat_and_as_tall_as_its_distance_says() {
    let mut h = Hall::boot();
    for mode in 1..=3 {
        h.set("MODE", mode);
        // the start room's north wall is the line y = 1: a cell and a half away
        h.stand(3.5, 2.5, 192);
        let page = h.look();
        // 121 / 1.5 = 80 tall, so the depth line holds 255 - 80 in every column
        for x in 0..160 {
            assert!((174..=176).contains(&h.depth(x)), "mode {mode}, column {x}: {}", h.depth(x));
        }
        // its top at line 2 and its bottom at 81: ceiling (brown) above, floor (grey) below
        let at = |x: usize, y: usize| page[y * 160 + x];
        for x in [0, 37, 80, 159] {
            assert_eq!((at(x, 0), at(x, 1)), (9, 9), "ceiling at column {x}");
            assert_eq!((at(x, 82), at(x, 83)), (12, 12), "floor at column {x}");
        }
        // nearer: half a cell away it's 242 tall, far taller than the page
        h.stand(3.5, 1.5, 192);
        h.look();
        assert!((12..=14).contains(&h.depth(80)), "{}", h.depth(80));
        // and timber: the picture's first column down the page's first, a pixel in ~4 lines
        // (the wall runs from x = 3 - 0.33 to 3 + 0.33 of the map at this distance)
    }
}

#[test]
fn every_other_column_filled_in_looks_like_a_ray_for_each() {
    let mut h = Hall::boot();
    for (x, y, a) in VIEWS {
        h.stand(x, y, a);
        h.set("MODE", 1);
        let every = h.look();
        h.set("MODE", 2);
        let half = h.look();
        let differ = every[..84 * 160].iter().zip(&half[..84 * 160]).filter(|(a, b)| a != b).count();
        assert!(differ * 100 < 84 * 160 * 4, "({x}, {y}) facing {a}: {differ} pixels differ");
    }
}

/// Which thing stands in the cell (x, y).
fn thing_at(h: &Hall, x: u8, y: u8) -> u16 {
    let (tx, ty) = (h.sym("thingx"), h.sym("thingy"));
    (0..h.sym("THINGS")).find(|&i| h.m.peek(tx + i) == x && h.m.peek(ty + i) == y).expect("a thing there")
}

#[test]
fn a_thing_is_drawn_at_its_size_and_hidden_where_a_wall_is_nearer() {
    let mut h = Hall::boot();
    let barrel = thing_at(&h, 5, 4);
    let (tx, ty) = (h.sym("thingx") + barrel, h.sym("thingy") + barrel);
    // straight at it from two cells: 60 pixels of barrel picture on the page's middle
    h.stand(3.5, 4.5, 0);
    let with = h.look();
    h.m.poke(tx, 30);
    h.m.poke(ty, 22);
    let without = h.look();
    let changed: Vec<usize> = (0..84 * 160).filter(|&i| with[i] != without[i]).collect();
    assert!(changed.len() > 500, "{} pixels of barrel", changed.len());
    let (x0, x1) = (changed.iter().map(|i| i % 160).min().unwrap(), changed.iter().map(|i| i % 160).max().unwrap());
    assert!(x0 >= 80 - 30 && x1 < 80 + 30 && x1 - x0 > 25, "columns {x0} to {x1}");
    assert!(changed.iter().all(|i| i / 160 >= 42 - 5), "the barrel is in the bottom half of its picture");
    // from the room below, the wall between hides all of it: the same picture with it or without
    h.stand(8.5, 8.5, 166);
    let without = h.look();
    h.m.poke(tx, 5);
    h.m.poke(ty, 4);
    let with = h.look();
    // (where it would be: the middle of the page. Further off, two far things of the same
    // size overlap, and which is drawn first depends on the list)
    let middle = |p: &[u8]| -> Vec<u8> { (0..84).flat_map(|y| p[y * 160 + 50..y * 160 + 110].to_vec()).collect() };
    assert!(middle(&with) == middle(&without), "the barrel shows through the wall");
    // through the doorway beside that wall, part of it shows: some columns, not all it would have
    h.stand(5.4, 8.5, 192);
    let with = h.look();
    h.m.poke(tx, 30);
    let without = h.look();
    assert!(with != without, "the barrel shows through the doorway");
}

/// Turning on the spot, a thing stays where the hall says it is: its middle on the page is
/// 80 + 121 x (how far to the right) / (how far ahead), the same sum the walls' rays make.
#[test]
fn a_thing_stays_put_in_the_hall_as_you_turn() {
    let mut h = Hall::boot();
    let barrel = thing_at(&h, 5, 4);
    let (tx, ty) = (h.sym("thingx") + barrel, h.sym("thingy") + barrel);
    for (px, py) in [(3.5f64, 4.5f64), (3.2, 3.3), (7.6, 2.4)] {
        for turn in -18i32..=18 {
            // facing the barrel, then up to 25 degrees either side
            let (dx, dy) = (5.5 - px, 4.5 - py);
            let facing = (dy.atan2(dx) * 128.0 / std::f64::consts::PI).round() as i32 + turn;
            let a = (facing.rem_euclid(256)) as u8;
            let rad = a as f64 * std::f64::consts::PI / 128.0;
            let (ahead, right) = (dx * rad.cos() + dy * rad.sin(), dy * rad.cos() - dx * rad.sin());
            let want = 80.0 + 121.25 * right / ahead;
            h.stand(px as f32, py as f32, a);
            h.m.poke(tx, 5);
            h.m.poke(ty, 4);
            let with = h.look();
            h.m.poke(tx, 30);
            h.m.poke(ty, 22);
            let without = h.look();
            let cols: Vec<usize> = (0..84 * 160).filter(|&i| with[i] != without[i]).map(|i| i % 160).collect();
            if !(20.0..140.0).contains(&want) {
                continue; // partly off the page: its middle can't be read off what shows
            }
            assert!(cols.len() > 100, "from ({px}, {py}) facing {a}: the barrel isn't drawn");
            let (x0, x1) = (*cols.iter().min().unwrap() as f64, *cols.iter().max().unwrap() as f64);
            let middle = (x0 + x1 + 1.0) / 2.0;
            assert!((middle - want).abs() < 3.0, "from ({px}, {py}) facing {a}: its middle is at {middle}, the hall says {want:.1}");
        }
    }
}

#[test]
fn walking_goes_by_the_clock_and_stops_at_walls() {
    let mut far = Vec::new();
    for mode in 1..=3 {
        let mut h = Hall::boot();
        h.set("MODE", mode);
        h.stand(3.5, 2.5, 0);
        h.hold(UP, 50);
        // 50 frames at 14/256 of a cell: 2.7 cells, however many pictures that was
        assert!((h.x() - 6.2).abs() < 0.35, "mode {mode}: x = {}", h.x());
        assert!((h.y() - 2.5).abs() < 0.01);
        far.push(h.x());
        h.hold(UP, 200);
        assert!((10.5..=10.76).contains(&h.x()), "stopped a quarter of a cell from the wall: {}", h.x());
        h.hold(DOWN, 30);
        assert!(h.x() < 9.6);
        h.hold(RIGHT, 16);
        assert!((28..=40).contains(&h.get("ANG")), "{}", h.get("ANG"));
        h.hold(LEFT, 32);
        assert!((208..=236).contains(&h.get("ANG")), "{}", h.get("ANG"));
        h.hold(b'D', 16);
        h.hold(b'W', 10);
        assert!(h.x() > 9.0);
        // a thing's cell is in the way too: the barrel at (5, 4)
        h.stand(3.5, 4.5, 0);
        h.hold(UP, 100);
        assert!((4.5..=4.76).contains(&h.x()), "{}", h.x());
    }
    assert!(far.iter().all(|x| (x - far[0]).abs() < 0.3), "{far:?}");
}

#[test]
fn the_keys_1_2_3_choose_the_columns() {
    let mut h = Hall::boot();
    assert_eq!(h.get("MODE"), 2, "it starts with a ray for every other column");
    h.hold(b'1', 5);
    assert_eq!(h.get("MODE"), 1);
    h.hold(b'3', 5);
    assert_eq!(h.get("MODE"), 3);
    let page = h.look();
    // columns four wide
    for x in (0..160).step_by(4) {
        for y in 0..84 {
            let c = page[y * 160 + x];
            assert!((1..4).all(|k| page[y * 160 + x + k] == c), "({x}, {y})");
        }
    }
    h.hold(b'2', 5);
    assert_eq!(h.get("MODE"), 2);
}

/// The claim in the source's header and in chapter 17: at 1 MHz, with a ray for every
/// other column, the hall is drawn 15 to 25 times a second, and no command is ever lost.
#[test]
fn pictures_a_second_at_1_mhz() {
    let mut h = Hall::boot();
    for (mode, least, mean_least) in [(1u8, 11.0, 13.0), (2, 15.0, 19.0), (3, 20.0, 24.0)] {
        h.set("MODE", mode);
        let mut total = 0.0;
        for (x, y, a) in VIEWS {
            h.stand(x, y, a);
            h.picture();
            let p = h.picture();
            let rate = 1_000_000.0 / p.cycles as f64;
            assert!(rate >= least, "mode {mode} at ({x}, {y}) facing {a}: {rate:.1} a second");
            total += rate;
        }
        let mean = total / VIEWS.len() as f64;
        assert!(mean >= mean_least, "mode {mode}: {mean:.1} a second on average");
    }
    // a turn on the spot and a walk, with the queue watched all the way
    let mut deepest = 0;
    h.set("MODE", 2);
    h.stand(15.5, 7.5, 0);
    for key in [RIGHT, UP, LEFT, UP, RIGHT, DOWN] {
        h.m.key(key);
        for _ in 0..60 {
            h.m.run_frame();
            deepest = deepest.max(h.m.mem.niece.as_ref().unwrap().queue.len());
        }
        h.m.key_up(key);
    }
    assert!(!h.lost(), "a command was lost");
    assert!(deepest < 48, "the queue got {deepest} deep");
}

#[test]
#[ignore = "prints what pictures cost around the hall: cargo test --test game_frosthall -- --ignored --nocapture"]
fn what_a_picture_costs() {
    let mut h = Hall::boot();
    // UNC_SHOTS=DIR also writes each view's page there, as mode 1 and mode 2 draw it
    if let Ok(dir) = std::env::var("UNC_SHOTS") {
        const PALETTE: [u32; 16] = [0x000000, 0xe8e4da, 0x9a3b2e, 0x5fb3a8, 0x7a3f8a, 0x4f9a4f, 0x2c3d9a, 0xe2c84a, 0xa8632a, 0x5a3e1a, 0xd0705f, 0x3a3a3a, 0x6b6b6b, 0x9fe08a, 0x6f8fd8, 0xa9a9a9];
        for (i, (x, y, a)) in VIEWS.into_iter().enumerate() {
            for mode in 1..=2 {
                h.set("MODE", mode);
                h.stand(x, y, a);
                let page = h.look();
                let mut out = b"P6 160 100 255\n".to_vec();
                page.iter().for_each(|&c| out.extend_from_slice(&PALETTE[c as usize].to_be_bytes()[1..]));
                std::fs::write(format!("{dir}/view{i}-mode{mode}.ppm"), out).unwrap();
            }
        }
    }
    for mode in 1..=3 {
    h.set("MODE", mode);
    println!("mode {mode}");
    for (x, y, a) in VIEWS {
        h.stand(x, y, a);
        h.picture();
        let p = h.picture();
        println!("({x}, {y}) facing {a}: {} cycles = {:.1} a second; walls {} ({} a column), things {}", p.cycles, 1_000_000.0 / p.cycles as f64, p.walls, p.walls / 80, p.things);
    }
    }
    assert!(!h.lost());
}
