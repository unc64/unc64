//! STACK (games/stack.s): a falling-tromino puzzle with bombs, played through scripted keys
//! and, where a test needs a particular board, by poking the state addresses its .res
//! block lays out in the fast page.

use unc_core::machine::{CYCLES_PER_FRAME, io};
use unc_core::{Machine, RamSize, Stop};

const SRC: &str = include_str!("../../../games/stack.s");

// The addresses laid out by the .res blocks in games/stack.s.
const STATE: u16 = 0x00;
const PX: u16 = 0x01;
const PY: u16 = 0x02;
const ROT: u16 = 0x03;
const TYPE: u16 = 0x04;
const NTYPE: u16 = 0x05;
const NROT: u16 = 0x06;
const FALLT: u16 = 0x07;
const LEVEL: u16 = 0x08;
const LVLCNT: u16 = 0x09;
const PCOUNT: u16 = 0x0A;
const GOTMR: u16 = 0x0B;
const SCORE: u16 = 0x0D; // 24 bits, low byte first
const LINES: u16 = 0x10; // as SCORE
const HISC: u16 = 0x13; // as SCORE
const CHARS: u16 = 0x0800;
const BOARD: u16 = 0x0C00;
const VMODE: u16 = 0xC000;

// Key codes (manual chapter 11).
const LEFT: u8 = 28;
const RIGHT: u8 = 29;
const UP: u8 = 30;
const DOWN: u8 = 31;

const TITLE: u8 = 0;
const PLAYING: u8 = 1;
const OVER: u8 = 2;
const I3: u8 = 0;
const L3: u8 = 1;
const BOMB: u8 = 2;
const BLOCK: u8 = 0x79; // the I3 block glyph; any non-zero board byte is a block

const WELLX: usize = 12;
const WELLY: usize = 3;

struct Game {
    m: Machine,
    asm: unc_asm::Assembly,
}

impl Game {
    /// Boots the ROM like the Machine Room does, loads STACK and runs to its title screen.
    fn boot() -> Game {
        let asm = unc_asm::assemble(SRC).unwrap_or_else(|e| panic!("stack.s doesn't assemble: {e:?}"));
        assert!(asm.symbol("start").is_some(), "stack.s has a start");
        let m = unc_asm::boot(RamSize::K16, &asm);
        let mut g = Game { m, asm };
        g.frames(3);
        g
    }

    /// Boots and presses SPACE on the title screen.
    fn playing() -> Game {
        let mut g = Game::boot();
        g.press(b' ');
        assert_eq!(g.peek(STATE), PLAYING);
        g
    }

    fn frames(&mut self, n: u32) {
        for _ in 0..n {
            let stop = self.m.run_to_vblank();
            assert_eq!(stop, Stop::Budget, "the game stopped");
        }
    }

    /// A tap: the key goes down for one frame, then comes up.
    fn press(&mut self, key: u8) {
        self.m.key(key);
        self.frames(1);
        self.m.key_up(key);
    }

    fn peek(&self, a: u16) -> u8 {
        self.m.peek(a)
    }

    fn poke(&mut self, a: u16, v: u8) {
        self.m.poke(a, v);
    }

    fn cell(&self, row: usize, col: usize) -> u8 {
        self.peek(BOARD + (row * 9 + col) as u16)
    }

    fn set_cell(&mut self, row: usize, col: usize, v: u8) {
        self.poke(BOARD + (row * 9 + col) as u16, v);
    }

    fn clear_board(&mut self) {
        for i in 0..162 {
            self.poke(BOARD + i, 0);
        }
    }

    /// Puts the falling piece where a test wants it, and holds off gravity.
    fn place(&mut self, ty: u8, rot: u8, x: u8, y: u8) {
        self.poke(TYPE, ty);
        self.poke(ROT, rot);
        self.poke(PX, x);
        self.poke(PY, y);
        self.poke(FALLT, 250);
    }

    /// A 24-bit number, low byte first, as NUMSTR takes it.
    fn points(&self, at: u16) -> u32 {
        (0..3).map(|i| (self.peek(at + i) as u32) << (8 * i)).sum()
    }

    fn set_points(&mut self, at: u16, v: u32) {
        assert!(v < 1 << 24);
        for i in 0..3 {
            self.poke(at + i, (v >> (8 * i)) as u8);
        }
    }

    fn score(&self) -> u32 {
        self.points(SCORE)
    }

    fn hisc(&self) -> u32 {
        self.points(HISC)
    }

    fn lines(&self) -> u32 {
        self.points(LINES)
    }

    fn screen_at(&self, row: usize, col: usize) -> u8 {
        self.m.screen()[row * 40 + col]
    }

    /// The screen as text with reverse video read as plain (screen_text shows it as '?').
    fn plain_text(&self) -> String {
        self.m.screen_text_plain()
    }

    fn row_text(&self, row: usize) -> String {
        self.m.screen_text().lines().nth(row).unwrap_or("").to_string()
    }

    /// The panel's text on a row: from column 24 to the end of the line.
    fn panel(&self, row: usize) -> String {
        self.row_text(row).chars().skip(24).collect::<String>().trim().to_string()
    }

    fn sym(&self, name: &str) -> u16 {
        self.asm.symbol(name).unwrap_or_else(|| panic!("no symbol {name}"))
    }

    /// SPEAKER's gate (bit 0) and data (bit 1): %11 while a note plays, %00 when silent.
    fn speaker(&self) -> u8 {
        self.peek(io::SPEAKER) & 3
    }

    /// The timer's divisor, as the game last wrote it.
    fn divisor(&self) -> u16 {
        u16::from_le_bytes([self.peek(io::TIMER_LO), self.peek(io::TIMER_HI)])
    }

    /// The notes of a sound effect, from the game's table: (frames, divisor, rough).
    fn effect(&self, name: &str) -> Vec<(u32, u16, bool)> {
        let mut a = self.sym(name) + 1; // after the priority
        let mut notes = vec![];
        while self.peek(a) != 0 {
            let f = self.peek(a);
            notes.push(((f & 0x7F) as u32, u16::from_le_bytes([self.peek(a + 1), self.peek(a + 2)]), f & 0x80 != 0));
            a += 3;
        }
        notes
    }

    /// The effect playing, by its label in the game's table, or None when silent.
    fn sounding(&self) -> Option<String> {
        if self.peek(self.sym("SNDT")) == 0 {
            return None;
        }
        let note = self.sym("sounds") + self.peek(self.sym("SNDI")) as u16 - 3;
        let names = ["s_move", "s_turn", "s_land", "s_line1", "s_line2", "s_line3", "s_bomb", "s_level", "s_over"];
        names.iter().filter(|n| self.sym(n) < note).max_by_key(|n| self.sym(n)).map(|n| n.to_string())
    }

    /// Runs frames and returns their audio, 882 samples a frame, as the hosts play it.
    fn audio(&mut self, frames: u32) -> Vec<f32> {
        let mut all = vec![];
        for _ in 0..frames {
            self.frames(1);
            let mut buf = vec![1.0f32; 882];
            self.m.audio_frame(44_100, &mut buf);
            all.extend(buf);
        }
        all
    }
}

#[test]
fn documented_addresses_match_the_source() {
    let g = Game::boot();
    for (name, addr) in [
        ("STATE", STATE),
        ("PX", PX),
        ("PY", PY),
        ("ROT", ROT),
        ("TYPE", TYPE),
        ("NTYPE", NTYPE),
        ("NROT", NROT),
        ("FALLT", FALLT),
        ("LEVEL", LEVEL),
        ("LVLCNT", LVLCNT),
        ("PCOUNT", PCOUNT),
        ("GOTMR", GOTMR),
        ("SCORE", SCORE),
        ("LINES", LINES),
        ("HISC", HISC),
        ("CHARS", CHARS),
        ("BOARD", BOARD),
    ] {
        assert_eq!(g.asm.symbol(name), Some(addr), "{name}");
    }
    // The character set and the board are reserved below the program, which fits in 16K.
    let start = g.asm.symbol("start").unwrap();
    assert!(start >= BOARD + 162, "the program starts after the board");
    assert!(g.asm.symbol("code_end").unwrap() <= 0x4000);
    // Nothing is loaded into the reserved memory: the program is code and data only.
    assert!(g.asm.segments.iter().all(|s| s.bytes.is_empty() || s.start >= start));
}

#[test]
fn title_screen_then_space_starts_a_game() {
    let mut g = Game::boot();
    assert_eq!(g.peek(STATE), TITLE);
    assert!(g.m.screen_text().contains("PRESS SPACE"));
    assert_eq!(g.peek(0xC022), (CHARS >> 8) as u8, "custom character set switched in");
    assert_ne!(g.peek(VMODE) & 0x08, 0, "a steady $A0: the GAME OVER box doesn't blink");
    assert!(g.row_text(22).contains("HI 000000"), "the high score on the title, six digits");
    // Other keys do nothing on the title.
    g.press(b'A');
    g.press(b'W');
    assert_eq!(g.peek(STATE), TITLE);
    g.press(b' ');
    assert_eq!(g.peek(STATE), PLAYING);
    g.frames(1);
    // The well: walls, floor, the panel.
    for r in 0..18 {
        assert_eq!(g.screen_at(WELLY + r, WELLX - 1), 0x11, "left wall, row {r}");
        assert_eq!(g.screen_at(WELLY + r, WELLX + 9), 0x11, "right wall, row {r}");
    }
    assert_eq!(g.screen_at(WELLY + 18, WELLX - 1), 0x14);
    assert_eq!(g.screen_at(WELLY + 18, WELLX + 9), 0x15);
    assert!(g.row_text(1).contains("S T A C K"));
    assert!(g.row_text(3).contains("SCORE"));
    assert_eq!(g.panel(4), "000000", "score, six digits");
    assert_eq!(g.panel(7), "000", "lines, three");
    assert_eq!(g.panel(10), "01", "level 1, two");
    assert_eq!(g.panel(21), "HI 000000");
    assert!(!g.m.screen_text().contains('_'), "no cursor left behind by the ROM's printing");
    assert_eq!(g.score(), 0);
    assert_eq!(g.peek(LEVEL), 1);
    assert_eq!((g.peek(PX), g.peek(PY)), (3, 0), "pieces enter at the top middle");
}

#[test]
fn a_piece_falls_one_row_every_45_frames_at_level_1() {
    let mut g = Game::playing();
    assert_eq!(g.peek(PY), 0);
    g.frames(44);
    assert_eq!(g.peek(PY), 0);
    g.frames(1);
    assert_eq!(g.peek(PY), 1);
    g.frames(45);
    assert_eq!(g.peek(PY), 2);
}

#[test]
fn a_and_d_move_and_the_walls_stop_the_piece() {
    let mut g = Game::playing();
    g.place(I3, 0, 3, 5); // flat: cells at columns x..x+2 of row y+1
    g.press(b'D');
    assert_eq!(g.peek(PX), 4);
    for _ in 0..6 {
        g.press(b'D');
    }
    assert_eq!(g.peek(PX), 6, "the right wall stops it");
    for c in 6..9 {
        assert_eq!(g.screen_at(WELLY + 6, WELLX + c), BLOCK, "drawn at column {c}");
    }
    for _ in 0..10 {
        g.press(b'A');
    }
    assert_eq!(g.peek(PX), 0, "the left wall stops it");
    // A block in the way stops it too.
    g.set_cell(6, 3, BLOCK);
    g.press(b'D');
    assert_eq!(g.peek(PX), 0);
}

#[test]
fn w_rotates_and_a_blocked_rotation_is_refused() {
    let mut g = Game::playing();
    g.place(I3, 0, 3, 5);
    g.press(b'W');
    assert_eq!(g.peek(ROT), 1);
    for r in 5..8 {
        assert_eq!(g.screen_at(WELLY + r, WELLX + 4), BLOCK, "upright at row {r}");
    }
    g.press(b'W');
    assert_eq!(g.peek(ROT), 2);
    // Upright against the right wall: lying flat would need column 9.
    g.place(I3, 1, 7, 5);
    g.press(b'W');
    assert_eq!(g.peek(ROT), 1, "refused at the wall");
    // The corner turns clockwise, unless the board is in the way.
    g.place(L3, 0, 3, 5);
    g.press(b'W');
    assert_eq!(g.peek(ROT), 1);
    g.place(L3, 0, 3, 5);
    g.set_cell(5, 4, BLOCK); // where rotation 1 needs a cell
    g.press(b'W');
    assert_eq!(g.peek(ROT), 0, "refused by a block");
}

#[test]
fn the_cursor_keys_do_what_wasd_do() {
    let mut g = Game::playing();
    g.place(I3, 0, 3, 5);
    g.press(RIGHT);
    assert_eq!(g.peek(PX), 4);
    g.press(LEFT);
    g.press(LEFT);
    assert_eq!(g.peek(PX), 2);
    g.press(UP);
    assert_eq!(g.peek(ROT), 1);
    g.press(DOWN);
    assert_eq!(g.peek(PY), 6);
    assert_eq!(g.score(), 1);
}

#[test]
fn a_held_move_waits_then_repeats_on_the_games_own_timing() {
    let mut g = Game::playing();
    g.place(I3, 0, 0, 5);
    g.m.key(b'D');
    let mut xs = vec![];
    for f in 0..16 {
        // The host's own auto-repeat (the showcase pad's: after 260 ms, every 90 ms)
        // arrives as more presses. The game ignores them and keeps its own time.
        if f >= 13 && (f - 13) % 4 == 0 {
            g.m.key(b'D');
        }
        g.frames(1);
        xs.push(g.peek(PX));
    }
    // A move on the press, a pause of 8 frames, then a move every second frame.
    assert_eq!(xs, [1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5]);
    g.frames(10);
    assert_eq!(g.peek(PX), 6, "until the wall");
    g.m.key_up(b'D');
    g.press(b'A');
    g.frames(20);
    assert_eq!(g.peek(PX), 5, "let go, it stops");
}

#[test]
fn a_tap_shorter_than_a_frame_still_counts() {
    let mut g = Game::playing();
    g.place(I3, 0, 3, 5);
    g.m.key(b'A');
    g.m.key_up(b'A'); // HELD never sees it: KBD does
    g.frames(1);
    assert_eq!(g.peek(PX), 2);
    g.frames(20);
    assert_eq!(g.peek(PX), 2);
}

#[test]
fn letting_go_of_one_key_goes_back_to_the_one_still_held() {
    let mut g = Game::playing();
    g.place(I3, 0, 3, 5);
    g.m.key(b'D');
    g.frames(1);
    assert_eq!(g.peek(PX), 4);
    g.press(b'A'); // pressed while D is still down
    assert_eq!(g.peek(PX), 3);
    g.frames(1); // A is up: D is timed afresh, with its delay
    g.frames(7);
    assert_eq!(g.peek(PX), 3);
    g.frames(1);
    assert_eq!(g.peek(PX), 4);
}

#[test]
fn rotate_and_hard_drop_do_not_repeat() {
    let mut g = Game::playing();
    g.place(I3, 0, 3, 5);
    let hold_with_host_repeats = |g: &mut Game, k: u8| {
        g.m.key(k);
        for f in 0..30 {
            if f >= 12 && f % 3 == 0 {
                g.m.key(k);
            }
            g.frames(1);
        }
        g.m.key_up(k);
    };
    hold_with_host_repeats(&mut g, b'W');
    assert_eq!(g.peek(ROT), 1, "one turn");
    hold_with_host_repeats(&mut g, b' ');
    assert_eq!(g.peek(PY), 0, "the next piece waits at the top");
    assert_eq!((0..162).filter(|&i| g.peek(BOARD + i) != 0).count(), 3, "one piece dropped");
}

#[test]
fn a_held_soft_drop_falls_every_second_frame_and_stops_at_the_next_piece() {
    let mut g = Game::playing();
    g.place(I3, 0, 0, 10); // flat on row 11, six rows above the floor
    g.m.key(b'S');
    g.frames(1);
    assert_eq!(g.peek(PY), 11, "a row on the press");
    g.frames(2);
    assert_eq!(g.peek(PY), 12, "and every second frame after");
    g.frames(8);
    assert_eq!(g.peek(PY), 16);
    assert_eq!(g.score(), 6, "a point a row");
    g.frames(2); // on the floor: the next one locks it
    assert_eq!(g.peek(PY), 0, "a new piece");
    assert!((0..3).all(|c| g.cell(17, c) != 0));
    g.frames(30);
    assert_eq!(g.peek(PY), 0, "S is still down, but the new piece needs a new press");
    g.m.key_up(b'S');
    g.frames(1);
    g.press(b'S');
    assert_eq!(g.peek(PY), 1);
}

#[test]
fn the_score_is_24_bits_and_prints_padded_to_six_digits() {
    let mut g = Game::playing();
    g.place(I3, 0, 3, 5);
    for (before, shown) in [(419, "000420"), (65_535, "065536"), (999_998, "999999")] {
        g.set_points(SCORE, before);
        g.press(b'S'); // a point
        assert_eq!(g.score(), before + 1);
        g.frames(1);
        assert_eq!(g.panel(4), shown, "{before} + 1");
    }
    g.set_points(SCORE, 1_234_004);
    g.press(b'S');
    g.frames(1);
    assert_eq!(g.panel(4), "1234005", "a seventh digit is printed, never cut");
    // A high score lower only in the low byte is beaten.
    g.set_points(HISC, 1_234_020);
    g.poke(NTYPE, L3);
    force_game_over(&mut g); // 22 points for the drop
    assert_eq!(g.peek(STATE), OVER);
    assert_eq!(g.score(), 1_234_027);
    assert_eq!(g.hisc(), 1_234_027);
    assert_eq!(g.panel(4), "1234027");
    assert_eq!(g.panel(21), "HI 1234027");
}

#[test]
fn space_hard_drops_locks_and_deals_the_next_piece() {
    let mut g = Game::playing();
    g.poke(NTYPE, L3);
    g.poke(NROT, 2);
    g.place(I3, 1, 3, 0); // upright in column 4, rows 0-2
    g.press(b' ');
    for r in 15..18 {
        assert_ne!(g.cell(r, 4), 0, "locked at row {r}");
    }
    assert_eq!(g.cell(14, 4), 0);
    assert_eq!(g.score(), 2 * 15, "two points a row dropped");
    assert_eq!((g.peek(TYPE), g.peek(ROT)), (L3, 2), "the previewed piece comes next");
    assert_eq!((g.peek(PX), g.peek(PY)), (3, 0));
    assert_eq!(g.peek(STATE), PLAYING);
    // The locked blocks are on screen, in the I3 glyph.
    g.frames(1);
    assert_eq!(g.screen_at(WELLY + 17, WELLX + 4), BLOCK);
}

#[test]
fn s_soft_drops_one_row_and_locks_on_the_floor() {
    let mut g = Game::playing();
    g.place(I3, 0, 0, 15); // flat on row 16
    g.press(b'S');
    assert_eq!(g.peek(PY), 16);
    assert_eq!(g.score(), 1);
    g.press(b'S'); // already on the floor: it locks
    assert_eq!(g.peek(PY), 0, "a new piece");
    assert!((0..3).all(|c| g.cell(17, c) != 0));
}

#[test]
fn a_full_row_clears_scores_and_the_rows_above_drop() {
    let mut g = Game::playing();
    for c in [0, 1, 2, 6, 7, 8] {
        g.set_cell(17, c, BLOCK);
    }
    g.set_cell(16, 0, BLOCK);
    g.place(I3, 0, 3, 0); // flat, fills columns 3-5 of row 17
    g.press(b' ');
    assert_eq!(g.lines(), 1);
    assert_eq!(g.score(), 2 * 16 + 10);
    assert_ne!(g.cell(17, 0), 0, "the block above came down");
    assert!((1..9).all(|c| g.cell(17, c) == 0));
    assert!((0..9).all(|c| g.cell(16, c) == 0));
    g.frames(1);
    assert_eq!(g.panel(7), "001", "lines on the panel");
    assert_eq!(g.panel(4), "000042", "score on the panel");
}

#[test]
fn three_rows_at_once_score_more_and_ten_lines_raise_the_level() {
    let mut g = Game::playing();
    g.poke(LVLCNT, 2);
    for r in 15..18 {
        for c in 0..9 {
            if c != 4 {
                g.set_cell(r, c, BLOCK);
            }
        }
    }
    g.set_cell(14, 8, BLOCK);
    g.place(I3, 1, 3, 0);
    g.press(b' ');
    assert_eq!(g.lines(), 3);
    assert_eq!(g.score(), 2 * 15 + 60);
    assert_ne!(g.cell(17, 8), 0, "the lone block fell three rows");
    assert_eq!(g.peek(LEVEL), 2);
    assert_eq!(g.peek(LVLCNT), 9);
    assert_eq!(g.peek(FALLT), 38, "level 2 falls faster");
    g.frames(1);
    assert_eq!(g.panel(10), "02");
}

#[test]
fn a_bomb_clears_its_3x3_and_the_blocks_above_settle() {
    let mut g = Game::playing();
    for r in 15..18 {
        for c in 0..8 {
            g.set_cell(r, c, BLOCK); // column 8 open: no full rows
        }
    }
    g.set_cell(10, 3, BLOCK); // floating above the blast's left column
    g.set_cell(9, 5, BLOCK);
    g.set_cell(8, 5, BLOCK);
    g.place(BOMB, 0, 3, 0); // the bomb is the cell at column x+1
    let toggles = g.m.mem.io.speaker_toggles;
    g.press(b' ');
    // It landed on row 14 (on top of row 15) at column 4: rows 13-15, columns 3-5 go.
    assert_eq!(g.score(), 2 * 14 + 5 * 3, "two points a row, five a block");
    assert_eq!(g.cell(10, 3), 0);
    assert_ne!(g.cell(13, 3), 0, "fell into the hole");
    assert_ne!(g.cell(12, 5), 0);
    assert_ne!(g.cell(11, 5), 0);
    assert_eq!(g.cell(9, 5), 0);
    assert_eq!(g.cell(8, 5), 0);
    assert_eq!(g.cell(14, 4), 0, "the bomb itself leaves nothing");
    for c in 3..6 {
        assert_eq!(g.cell(15, c), 0, "blasted at column {c}");
        assert_ne!(g.cell(16, c), 0, "below the blast is untouched");
    }
    for c in [0, 1, 2, 6, 7] {
        assert_ne!(g.cell(15, c), 0, "outside the blast is untouched");
    }
    assert_eq!(g.sounding().as_deref(), Some("s_bomb"), "the bomb's roar, over the landing");
    let mut divisors = vec![];
    for i in 0..10 {
        g.frames(1);
        if i == 0 {
            assert_eq!(g.peek(0xC020), 2, "the border flashes");
        }
        assert_eq!(g.speaker(), 0b11, "the timer plays it");
        divisors.push(g.divisor());
    }
    divisors.sort();
    divisors.dedup();
    assert!(divisors.len() >= 8, "rough: a new divisor from NOISE every frame {divisors:?}");
    assert_eq!(g.m.mem.io.speaker_toggles, toggles, "and nobody flips the speaker by hand");
    g.frames(1);
    assert_eq!(g.peek(0xC020), 0, "the border flashed for 10 frames");
}

#[test]
fn a_bomb_at_the_wall_and_on_the_floor_clips_its_blast() {
    let mut g = Game::playing();
    for c in 0..2 {
        for r in 16..18 {
            g.set_cell(r, c, BLOCK);
        }
    }
    g.place(BOMB, 0, 0xFF, 0); // bomb in column 0, falls to row 15
    g.press(b' ');
    // Rows 14-16 of columns 0-1: row 16 blasted, row 17 stays.
    assert_eq!(g.score(), 2 * 15 + 5 * 2);
    assert!((0..2).all(|c| g.cell(17, c) != 0 && g.cell(16, c) == 0));

    let mut g = Game::playing();
    g.place(BOMB, 0, 3, 0);
    g.set_cell(17, 3, BLOCK);
    g.set_cell(16, 3, BLOCK);
    g.set_cell(17, 5, BLOCK);
    g.press(b' '); // column 4 is empty: it reaches the floor, row 17
    assert_eq!(g.cell(16, 3), 0);
    assert_eq!(g.cell(17, 3), 0);
    assert_eq!(g.cell(17, 5), 0);
    assert_eq!(g.score(), 2 * 17 + 5 * 3);
}

#[test]
fn every_eighth_piece_is_a_bomb_and_the_preview_says_so() {
    let mut g = Game::playing();
    for n in 1..=24u32 {
        let ty = g.peek(TYPE);
        if n % 8 == 0 {
            assert_eq!(ty, BOMB, "piece {n}");
        } else {
            assert_ne!(ty, BOMB, "piece {n}");
        }
        if n % 8 == 7 {
            assert_eq!(g.peek(NTYPE), BOMB, "preview before piece {}", n + 1);
            g.frames(1);
            assert_eq!(g.screen_at(14, 26), 0x7B, "the bomb glyph in the NEXT box");
            assert!(g.row_text(19).contains("BOMB IN 1"));
        }
        g.clear_board();
        g.press(b' ');
    }
}

#[test]
fn pieces_are_random_but_both_kinds_turn_up() {
    let mut g = Game::playing();
    let mut seen = [0u32; 3];
    for _ in 0..40 {
        seen[g.peek(TYPE) as usize] += 1;
        g.clear_board();
        g.press(b' ');
    }
    assert!(seen[0] >= 8 && seen[1] >= 8, "I3/L3/bomb counts {seen:?}");
    assert_eq!(seen[2], 5);
}

/// Fills rows 0-1 in the middle, so the next piece can't enter, and drops the piece aside.
fn force_game_over(g: &mut Game) {
    block_the_entrance(g);
    g.press(b' ');
}

fn block_the_entrance(g: &mut Game) {
    g.place(I3, 1, 6, 4); // upright in column 7
    for r in 0..2 {
        for c in 3..6 {
            g.set_cell(r, c, BLOCK);
        }
    }
}

#[test]
fn the_game_ends_when_a_new_piece_cannot_enter() {
    let mut g = Game::playing();
    g.poke(NTYPE, L3);
    force_game_over(&mut g);
    assert_eq!(g.peek(STATE), OVER);
    assert!(g.plain_text().contains("GAME OVER"));
    // The message is in reverse video over the well.
    assert_eq!(g.screen_at(WELLY + 7, WELLX), b'G' | 0x80);
    // Nothing moves any more.
    let before = g.m.screen().to_vec();
    g.press(b'A');
    g.frames(60);
    assert_eq!(g.m.screen(), &before[..]);
    assert_eq!(g.peek(STATE), OVER);
}

#[test]
fn space_restarts_after_game_over_but_not_straight_away() {
    let mut g = Game::playing();
    g.place(I3, 1, 3, 0);
    g.press(b' '); // 30 points
    force_game_over(&mut g);
    assert_eq!(g.peek(STATE), OVER);
    assert_eq!(g.hisc(), g.score(), "the high score is kept");
    let hi = g.score();
    assert!(hi > 0);
    // An auto-repeated SPACE from the hard drop must not restart at once.
    g.press(b' ');
    g.press(b' ');
    assert_eq!(g.peek(STATE), OVER);
    g.frames(50);
    assert_eq!(g.peek(GOTMR), 0);
    g.press(b' ');
    assert_eq!(g.peek(STATE), PLAYING);
    assert_eq!(g.score(), 0);
    assert_eq!(g.lines(), 0);
    assert_eq!(g.peek(LEVEL), 1);
    assert!((0..162).all(|i| g.peek(BOARD + i) == 0), "the board is empty");
    assert_eq!(g.hisc(), hi);
    g.frames(1);
    assert_eq!(g.panel(21), format!("HI {hi:06}"));
    assert!(!g.plain_text().contains("GAME OVER"));
}

#[test]
fn start_up_takes_about_a_frame_and_a_bit() {
    let asm = unc_asm::assemble(SRC).unwrap();
    let mut m = unc_asm::boot(RamSize::K16, &asm);
    let spent = m.run_until(asm.symbol("mainlp").unwrap(), 5 * CYCLES_PER_FRAME as u64);
    assert!(spent.is_ok(), "never reached the main loop: {spent:?}");
    let spent = spent.unwrap();
    println!("start-up (character set copy and title): {spent} cycles");
    assert!(spent < 2 * 19_968);
}

#[test]
fn worst_case_frame_fits_in_the_budget() {
    let mut g = Game::boot();
    let go = g.asm.symbol("frame_go").unwrap();
    let end = g.asm.symbol("frame_end").unwrap();
    g.m.set_breakpoint(go, true);
    g.m.set_breakpoint(end, true);
    let mut worst = (0u64, String::new());
    let mut t0 = 0u64;
    let mut run = |g: &mut Game, frames: u32, what: &str| {
        let mut here = 0;
        for _ in 0..frames {
            loop {
                match g.m.run_to_vblank() {
                    Stop::Budget => break,
                    Stop::Breakpoint(pc) if pc == go => t0 = g.m.cpu.cycles,
                    Stop::Breakpoint(pc) if pc == end => {
                        let spent = g.m.cpu.cycles - t0;
                        here = here.max(spent);
                        if spent > worst.0 {
                            worst = (spent, what.to_string());
                        }
                    }
                    s => panic!("stopped: {s:?}"),
                }
            }
        }
        println!("{what:>28}: {here:>6} cycles");
    };
    run(&mut g, 5, "title");
    g.m.key(b' ');
    run(&mut g, 1, "new game");
    g.m.key_up(b' ');
    run(&mut g, 30, "falling");
    // A deep triple clear: column 4 open from the top, rows 15-17 one short, a full stack above.
    for r in 3..18 {
        for c in 0..9 {
            if c != 4 {
                g.set_cell(r, c, BLOCK);
            }
        }
    }
    for r in 3..15 {
        g.set_cell(r, 8, 0);
    }
    g.place(I3, 1, 3, 0);
    g.m.key(b' ');
    run(&mut g, 1, "hard drop, triple clear");
    g.m.key_up(b' ');
    assert_eq!(g.lines(), 3, "the triple clear happened");
    run(&mut g, 3, "after the clear");
    // A bomb from the top into a deep, full-but-one stack, so the settle refills rows.
    g.clear_board();
    for r in 2..18 {
        for c in 0..9 {
            if c != 4 {
                g.set_cell(r, c, BLOCK);
            }
        }
    }
    g.place(BOMB, 0, 3, 0);
    g.m.key(b' ');
    run(&mut g, 1, "hard drop, bomb");
    g.m.key_up(b' ');
    assert_eq!(g.peek(0x0C00 + 17 * 9 + 4), 0, "the bomb went off at the bottom");
    run(&mut g, 3, "after the bomb");
    // Moves and rotations on a tall board.
    // A bomb tucked under an overhang whose settle completes three rows (contrived: the
    // worst a bomb can do), locked by S.
    g.clear_board();
    for r in 3..18 {
        for c in 0..9 {
            if !(r < 15 && c == 8) && !(r == 16 && c == 4) {
                g.set_cell(r, c, BLOCK);
            }
        }
    }
    g.place(BOMB, 0, 3, 16);
    g.m.key(b'S');
    run(&mut g, 1, "bomb settle, triple clear");
    g.m.key_up(b'S');
    assert_eq!(g.lines(), 6, "the bomb's settle cleared three rows");
    run(&mut g, 2, "after the bomb settle");
    for k in *b"ADWSWD" {
        g.m.key(k);
        run(&mut g, 1, "moves");
        g.m.key_up(k);
    }
    // Held keys: an auto-shift into the wall, then a held soft drop.
    g.m.key(RIGHT);
    run(&mut g, 20, "held move");
    g.m.key_up(RIGHT);
    g.m.key(DOWN);
    run(&mut g, 20, "held soft drop");
    g.m.key_up(DOWN);
    block_the_entrance(&mut g);
    g.m.key(b' ');
    run(&mut g, 60, "game over");
    g.m.key_up(b' ');
    assert_eq!(g.peek(STATE), OVER);
    g.m.key(b' ');
    run(&mut g, 2, "restart");
    assert_eq!(g.peek(STATE), PLAYING);
    println!("worst frame: {} cycles ({})", worst.0, worst.1);
    assert!(worst.0 > 0);
    assert!(worst.0 < 19_968, "worst frame {} cycles ({})", worst.0, worst.1);
}

// ---------- sound: the beeper's timer, one effect at a time, stepped once a frame ----------

/// Plays out an effect that has just started: each note's divisor with the gate open for
/// exactly its frames, then the gate closed and data off.
fn plays_out(g: &mut Game, name: &str) {
    assert_eq!(g.sounding().as_deref(), Some(name));
    let notes = g.effect(name);
    assert!(!notes.is_empty());
    for (i, &(frames, divisor, rough)) in notes.iter().enumerate() {
        for f in 0..frames {
            let want = if divisor == 0 { 0b01 } else { 0b11 };
            assert_eq!(g.speaker(), want, "{name} note {i}, frame {f}: gate open, data on unless a rest");
            if !rough && f == 0 {
                assert_eq!(g.divisor(), divisor, "{name} note {i}");
            }
            g.frames(1);
        }
    }
    assert_eq!(g.speaker(), 0, "{name} over: the gate closed, the speaker in");
    assert_eq!(g.sounding(), None);
}

#[test]
fn a_move_ticks_a_turn_chirps_and_a_landing_thuds_then_the_gate_closes() {
    let mut g = Game::playing();
    g.place(I3, 0, 3, 5);
    assert_eq!(g.speaker(), 0);
    g.press(b'D');
    assert_eq!(g.effect("s_move").len(), 1, "a tick is one short note");
    plays_out(&mut g, "s_move");
    g.press(b'W');
    plays_out(&mut g, "s_turn");
    g.press(b' ');
    plays_out(&mut g, "s_land");
    // The tick is heard: the audio the hosts play isn't silent while it sounds.
    g.press(LEFT);
    let mut buf = vec![0.0f32; 882];
    g.m.audio_frame(44_100, &mut buf);
    assert!(buf.iter().any(|&v| v != 0.0), "a tick you can hear");
    // A move into a wall is refused, and silent.
    g.frames(5);
    g.place(I3, 0, 6, 5);
    g.press(b'D');
    assert_eq!(g.peek(PX), 6);
    assert_eq!(g.sounding(), None, "pushing at the wall makes no sound");
}

#[test]
fn it_is_silent_at_rest() {
    let mut g = Game::boot();
    let title = g.audio(50);
    assert!(title.iter().all(|&v| v == 0.0), "the title is silent");
    assert_eq!(g.speaker(), 0, "gate closed");
    g.press(b' ');
    let falling = g.audio(100);
    assert!(falling.iter().all(|&v| v == 0.0), "a piece falling by itself is silent");
    assert_eq!(g.peek(io::SPEAKER) & io::SPEAKER_GATE, 0);
    // Game over: the tune plays out and stops by itself.
    g.poke(NTYPE, L3);
    force_game_over(&mut g);
    assert_eq!(g.sounding().as_deref(), Some("s_over"));
    let len: u32 = g.effect("s_over").iter().map(|n| n.0).sum();
    g.frames(len);
    assert_eq!(g.speaker(), 0);
    assert!(g.audio(20).iter().all(|&v| v == 0.0), "silent again after the tune");
}

#[test]
fn line_clears_chime_bigger_for_more_rows_and_a_new_level_takes_over() {
    // One row, two rows, three rows: each its own chime, longer for more.
    let mut lens = vec![];
    for rows in 1..=3usize {
        let mut g = Game::playing();
        for r in 18 - rows..18 {
            for c in 0..9 {
                if c != 4 {
                    g.set_cell(r, c, BLOCK);
                }
            }
        }
        g.place(I3, 1, 3, 0); // upright: it lands in column 4, rows 15-17, and fills the gaps
        g.press(b' ');
        assert_eq!(g.lines(), rows as u32, "{rows} rows");
        let name = format!("s_line{rows}");
        lens.push(g.effect(&name).iter().map(|n| n.0).sum::<u32>());
        plays_out(&mut g, &name);
    }
    assert!(lens[0] < lens[1] && lens[1] < lens[2], "bigger for more rows: {lens:?}");
    // The tenth line: the level-up fanfare plays instead of the chime.
    let mut g = Game::playing();
    g.poke(LVLCNT, 1);
    for c in [0, 1, 2, 6, 7, 8] {
        g.set_cell(17, c, BLOCK);
    }
    g.place(I3, 0, 3, 0);
    g.press(b' ');
    assert_eq!(g.peek(LEVEL), 2);
    plays_out(&mut g, "s_level");
}

#[test]
fn a_tick_never_cuts_a_bigger_sound_short_but_replaces_a_tick() {
    let mut g = Game::playing();
    for c in [0, 1, 2, 6, 7, 8] {
        g.set_cell(17, c, BLOCK);
    }
    g.place(I3, 0, 3, 0);
    g.press(b' ');
    assert_eq!(g.sounding().as_deref(), Some("s_line1"));
    g.press(b'A');
    assert_eq!(g.sounding().as_deref(), Some("s_line1"), "the move's tick waits its turn");
    let len: u32 = g.effect("s_line1").iter().map(|n| n.0).sum();
    g.frames(len);
    assert_eq!(g.sounding(), None);
    g.place(I3, 0, 3, 5);
    g.press(b'D');
    g.press(b'W'); // a turn the very next frame: one voice, the newest tick wins
    assert_eq!(g.sounding().as_deref(), Some("s_turn"));
    // A new game silences the game-over tune.
    g.poke(NTYPE, L3);
    force_game_over(&mut g);
    assert_eq!(g.sounding().as_deref(), Some("s_over"));
    g.frames(50);
    g.press(b' ');
    assert_eq!(g.peek(STATE), PLAYING);
    assert_eq!((g.sounding(), g.speaker()), (None, 0), "hushed for the new game");
}

// ---------- the dice: RND seeded from NOISE when SPACE starts a game ----------

/// The first `n` pieces of a game started by SPACE after `wait` frames on the title.
fn pieces_after(wait: u32, n: usize) -> Vec<(u8, u8)> {
    let mut g = Game::boot();
    g.frames(wait);
    g.press(b' ');
    let mut seq = vec![];
    for _ in 0..n {
        seq.push((g.peek(NTYPE), g.peek(NROT)));
        g.clear_board();
        g.press(b' ');
    }
    seq
}

#[test]
fn the_moment_space_is_pressed_picks_the_pieces() {
    let a = pieces_after(10, 20);
    assert_eq!(a, pieces_after(10, 20), "the same moment, the same game: replays work");
    let others: Vec<_> = (11..16).map(|w| pieces_after(w, 20)).collect();
    for (i, o) in others.iter().enumerate() {
        assert_ne!(&a, o, "SPACE {} frames later deals another game", i + 1);
    }
    assert_ne!(others[0], others[1]);
    // The seed comes from NOISE, not from what RND held at start-up.
    let mut g = Game::boot();
    let seed = |g: &Game| u16::from_le_bytes([g.peek(0x020B), g.peek(0x020C)]);
    let before = seed(&g);
    g.press(b' ');
    assert_ne!(seed(&g), before);
}

// ---------- input: the keyboard matrix ----------

#[test]
fn a_tap_moves_exactly_one_column_and_a_hold_slides() {
    let mut g = Game::playing();
    g.place(I3, 0, 0, 5);
    g.press(b'D');
    g.frames(30);
    assert_eq!(g.peek(PX), 1, "a tap: one column, however long after");
    g.press(RIGHT);
    g.frames(30);
    assert_eq!(g.peek(PX), 2, "the arrow too");
    g.m.key(b'A');
    g.frames(14);
    g.m.key_up(b'A');
    g.frames(10);
    assert_eq!(g.peek(PX), 0, "held: it slides to the wall");
    g.place(I3, 0, 0, 5);
    g.m.key(RIGHT);
    g.frames(14); // the press, the 8-frame wait, then every second frame: 4 columns
    g.m.key_up(RIGHT);
    g.frames(10);
    assert_eq!(g.peek(PX), 4, "held 14 frames: four columns");
}

#[test]
fn keys_work_together_slide_while_dropping_and_turn_while_sliding() {
    let mut g = Game::playing();
    g.place(I3, 1, 0, 2); // upright in column 1, rows 2-4
    g.m.key(b'D');
    g.m.key(DOWN); // WASD and the arrows mixed, both held
    g.frames(12);
    assert_eq!(g.peek(PX), 3, "slid: the press, then frames 8 and 10");
    assert_eq!(g.peek(PY), 2 + 6, "and dropped: the press, then every second frame");
    g.press(b'W'); // a turn, with both still down
    assert_eq!(g.peek(ROT), 2);
    g.m.key_up(b'D');
    g.m.key_up(DOWN);
    let (x, y) = (g.peek(PX), g.peek(PY));
    g.frames(10);
    assert_eq!((g.peek(PX), g.peek(PY)), (x, y), "let go, it stays");
}
