//! The NIECE, the prototype graphics chip (manual chapter 17): its registers, its queue and
//! what each command costs, its commands pixel by pixel, the depth line, FLIP and the beam,
//! its arithmetic, and where its page shows in the picture.

use unc_core::machine::{CYCLES_PER_FRAME, CYCLES_PER_LINE, Machine, NIECE_PIXEL, PIXELS_H, PIXELS_W, RamSize, VBLANK_LINE};
use unc_core::niece::{self, cmd, flag, reg};

const N: u16 = niece::BASE;

fn machine() -> Machine {
    let mut m = Machine::new(RamSize::K64);
    m.set_niece(true);
    // halted, so only the clock moves, a cycle at a time (`idle`)
    m.poke(0x0800, 0x02); // HLT
    m.cpu.pc = 0x0800;
    m.step();
    m
}

fn word(m: &mut Machine, r: u16, v: i32) {
    m.poke(N + r, v as u8);
    m.poke(N + r + 1, (v >> 8) as u8);
}

fn dest(m: &mut Machine, x: i32, y: i32, w: i32, h: i32) {
    word(m, reg::NDX, x);
    word(m, reg::NDY, y);
    word(m, reg::NDW, w);
    word(m, reg::NDH, h);
}

fn source(m: &mut Machine, x: i32, y: i32, w: i32, h: i32) {
    m.poke(N + reg::NSX, x as u8);
    word(m, reg::NSY, y);
    m.poke(N + reg::NSW, w as u8);
    m.poke(N + reg::NSH, h as u8);
}

/// Writes a command and runs the machine (a HLT at $0800, so only the clock moves) until the
/// queue is empty. Returns the cycles that took.
fn go(m: &mut Machine, op: u8) -> u64 {
    m.poke(N + reg::NGO, op);
    let t0 = m.cpu.cycles + m.idle_cycles;
    while m.peek(N + reg::NSTATUS) & niece::STATUS_BUSY != 0 {
        idle(m, 1);
    }
    m.cpu.cycles + m.idle_cycles - t0
}

/// The clock moves on by `cycles` with the processor halted.
fn idle(m: &mut Machine, cycles: u32) {
    for _ in 0..cycles {
        m.run_cycles(1);
        // a peek doesn't move the chip: a register write does
        let v = m.peek(N + reg::NCTRL);
        m.poke(N + reg::NCTRL, v);
    }
}

fn px(m: &Machine, x: i32, y: i32) -> u8 {
    m.mem.niece.as_ref().unwrap().pixel(x, y)
}

/// The page that isn't shown: where commands draw.
fn back(m: &Machine) -> i32 {
    (m.mem.niece.as_ref().unwrap().page ^ 1) as i32 * 100
}

#[test]
fn without_the_chip_its_addresses_read_ff() {
    let mut m = Machine::new(RamSize::K64);
    assert!(!m.has_niece());
    for a in N..N + niece::SPAN {
        m.poke(a, 0x12);
        assert_eq!(m.peek(a), 0xFF, "${a:04X}");
    }
}

#[test]
fn the_id_and_the_registers_at_switch_on() {
    let m = machine();
    assert!(m.has_niece());
    assert_eq!(m.peek(N + reg::NIECE), 0x17);
    assert_eq!(m.peek(N + reg::NCTRL), 0);
    assert_eq!(m.peek(N + reg::NSTATUS), 0);
    assert_eq!(m.peek(N + reg::NQUEUE), 0);
    for i in 0..16 {
        assert_eq!(m.peek(N + reg::NINK + i), i as u8, "every ink table starts as itself");
    }
    // the gaps
    for r in [0x1E, 0x1F, 0x3E, 0x46, 0x4E] {
        assert_eq!(m.peek(N + r), 0xFF);
    }
    assert_eq!(m.peek(N + niece::SPAN), 0xFF, "past the chip");
}

#[test]
fn registers_read_back_what_was_written() {
    let mut m = machine();
    dest(&mut m, -3, 0x1234, 0xBEEF, 7);
    source(&mut m, 200, 0x1FF, 33, 44);
    assert_eq!(m.peek(N + reg::NDX), 0xFD);
    assert_eq!(m.peek(N + reg::NDX + 1), 0xFF);
    assert_eq!(m.peek(N + reg::NDY + 1), 0x12);
    assert_eq!(m.peek(N + reg::NDW), 0xEF);
    assert_eq!(m.peek(N + reg::NSX), 200);
    assert_eq!(m.peek(N + reg::NSY + 1), 1);
    m.poke(N + reg::NSY + 1, 0xFF);
    assert_eq!(m.peek(N + reg::NSY + 1), 1, "NSY has nine bits");
    m.poke(N + reg::NCOLOUR, 0xF7);
    assert_eq!(m.peek(N + reg::NCOLOUR), 0xF7, "two colours: FILL's second in the high four bits");
    m.poke(N + reg::NKEY, 0x1C);
    assert_eq!(m.peek(N + reg::NKEY), 12);
    m.poke(N + reg::NCTRL, 0xFF);
    assert_eq!(m.peek(N + reg::NCTRL), 1);
}

#[test]
fn vdata_reads_and_writes_the_sheet_and_moves_on() {
    let mut m = machine();
    word(&mut m, reg::NVADDR, 0xFFFE);
    for b in [0x12, 0x34, 0x56] {
        m.poke(N + reg::NVDATA, b);
    }
    assert_eq!(m.peek(N + reg::NVADDR), 0x01, "it wraps at the end of the 64K");
    assert_eq!(m.peek(N + reg::NVADDR + 1), 0x00);
    // the last line of the sheet's last four pixels, then its first two
    assert_eq!([px(&m, 252, 511), px(&m, 253, 511), px(&m, 254, 511), px(&m, 255, 511)], [1, 2, 3, 4]);
    assert_eq!([px(&m, 0, 0), px(&m, 1, 0)], [5, 6]);
    // reading: a program's LDA NVDATA takes the byte and moves on
    word(&mut m, reg::NVADDR, 0xFFFE);
    let prog = [0x2B, 0x06, 0xC1, 0x2B, 0x06, 0xC1, 0x02]; // LDA $C106, LDA $C106, HLT
    m.load(0x0800, &prog);
    m.cpu.state = unc_core::CpuState::Running;
    m.cpu.pc = 0x0800;
    m.step();
    assert_eq!(m.cpu.a, 0x12);
    m.step();
    assert_eq!(m.cpu.a, 0x34);
    assert_eq!(m.peek(N + reg::NVADDR), 0x00);
}

#[test]
fn fill_draws_on_the_page_not_shown_and_costs_a_cycle_for_four_pixels() {
    let mut m = machine();
    m.poke(N + reg::NCOLOUR, 9);
    dest(&mut m, 10, 20, 30, 5);
    // 150 pixels: 4 + 38
    assert_eq!(go(&mut m, cmd::FILL), 42);
    let top = back(&m);
    assert_eq!(top, 100, "page 0 is shown at switch-on, so page 1 is drawn on");
    assert_eq!(px(&m, 10, top + 20), 9);
    assert_eq!(px(&m, 39, top + 24), 9);
    for (x, y) in [(9, 20), (40, 20), (10, 19), (10, 25)] {
        assert_eq!(px(&m, x, top + y), 0, "({x}, {y}) is outside");
    }
    assert_eq!(px(&m, 10, 20), 0, "the page shown is untouched");
}

#[test]
fn a_commands_pixels_appear_when_it_finishes_not_before() {
    let mut m = machine();
    m.poke(N + reg::NCOLOUR, 5);
    dest(&mut m, 0, 0, 160, 100);
    m.poke(N + reg::NGO, cmd::FILL);
    // 16,000 pixels: 4 + 4,000 cycles
    idle(&mut m, 4003);
    assert_eq!(m.peek(N + reg::NSTATUS) & niece::STATUS_BUSY, niece::STATUS_BUSY);
    assert_eq!(px(&m, 0, 100), 0);
    idle(&mut m, 1);
    assert_eq!(m.peek(N + reg::NSTATUS) & niece::STATUS_BUSY, 0);
    assert_eq!(px(&m, 0, 100), 5);
    assert_eq!(px(&m, 159, 199), 5);
}

#[test]
fn clipping_costs_only_the_pixels_on_the_page() {
    let mut m = machine();
    m.poke(N + reg::NCOLOUR, 3);
    dest(&mut m, -10, -10, 20, 20);
    // 10 x 10 on the page: 4 + 25
    assert_eq!(go(&mut m, cmd::FILL), 29);
    assert_eq!(px(&m, 9, 109), 3);
    assert_eq!(px(&m, 10, 109), 0);
    assert_eq!(px(&m, 255, 109), 0, "nothing wrapped round the sheet");
    // wholly off the page: only the 4
    dest(&mut m, 200, 0, 50, 50);
    assert_eq!(go(&mut m, cmd::FILL), 4);
    dest(&mut m, 0, 0, 0, 50);
    assert_eq!(go(&mut m, cmd::FILL), 4);
}

#[test]
fn the_queue_holds_64_runs_in_order_and_says_when_it_lost_one() {
    let mut m = machine();
    dest(&mut m, 0, 0, 160, 100);
    for c in 0..64 {
        m.poke(N + reg::NCOLOUR, c & 15);
        m.poke(N + reg::NGO, cmd::FILL);
    }
    assert_eq!(m.peek(N + reg::NQUEUE), 64);
    assert_eq!(m.peek(N + reg::NSTATUS), niece::STATUS_BUSY | niece::STATUS_FULL);
    m.poke(N + reg::NCOLOUR, 15);
    m.poke(N + reg::NGO, cmd::FILL);
    assert_eq!(m.peek(N + reg::NQUEUE), 64);
    assert_eq!(m.peek(N + reg::NSTATUS) & niece::STATUS_LOST, niece::STATUS_LOST);
    // a program's reads of NSTATUS leave LOST set (a wait loop polls it); a write clears it
    m.load(0x0800, &[0x2B, 0x01, 0xC1, 0x2B, 0x01, 0xC1, 0x33, 0x01, 0xC1, 0x02]); // LDA $C101 twice, STA $C101, HLT
    m.cpu.state = unc_core::CpuState::Running;
    m.cpu.pc = 0x0800;
    m.step();
    m.step();
    assert_eq!(m.cpu.a & niece::STATUS_LOST, niece::STATUS_LOST);
    m.step();
    assert_eq!(m.peek(N + reg::NSTATUS) & niece::STATUS_LOST, 0);
    assert_eq!(m.peek(N + reg::NQUEUE), 64, "and changes nothing else");
    // one command done every 4,004 cycles
    idle(&mut m, 4004);
    assert_eq!(m.peek(N + reg::NQUEUE), 63);
    assert_eq!(px(&m, 0, 100), 0);
    idle(&mut m, 4004);
    assert_eq!(m.peek(N + reg::NQUEUE), 62);
    assert_eq!(px(&m, 0, 100), 1);
    idle(&mut m, 62 * 4004);
    assert_eq!(m.peek(N + reg::NQUEUE), 0);
    assert_eq!(px(&m, 0, 100), 63 & 15, "the lost one never ran");
    // an unknown command isn't one
    m.poke(N + reg::NGO, 0);
    m.poke(N + reg::NGO, 5);
    assert_eq!(m.peek(N + reg::NQUEUE), 0);
}

/// A 4 x 4 picture on the sheet at (160, 0): each pixel's colour is 1 + x + 4 * y... kept to
/// 1-15 so 0 only appears where the test puts it.
fn stamp(m: &mut Machine) {
    for y in 0..4 {
        word(m, reg::NVADDR, y * 128 + 80);
        for x in 0..2 {
            let c = |x: i32| ((x + 4 * y) % 15 + 1) as u8;
            m.poke(N + reg::NVDATA, c(2 * x) << 4 | c(2 * x + 1));
        }
    }
}
fn stamp_at(x: i32, y: i32) -> u8 {
    ((x + 4 * y) % 15 + 1) as u8
}

#[test]
fn blit_copies_scales_and_flips() {
    let mut m = machine();
    stamp(&mut m);
    source(&mut m, 160, 0, 4, 4);
    // same size
    dest(&mut m, 5, 5, 4, 4);
    assert_eq!(go(&mut m, cmd::BLIT), 4 + 4);
    for y in 0..4 {
        for x in 0..4 {
            assert_eq!(px(&m, 5 + x, 105 + y), stamp_at(x, y));
        }
    }
    // three times the size: each source pixel 3 x 3
    dest(&mut m, 20, 20, 12, 12);
    assert_eq!(go(&mut m, cmd::BLIT), 4 + 36);
    for y in 0..12 {
        for x in 0..12 {
            assert_eq!(px(&m, 20 + x, 120 + y), stamp_at(x / 3, y / 3), "({x}, {y})");
        }
    }
    // half the size: source pixels 0 and 2 of each row and column
    dest(&mut m, 40, 40, 2, 2);
    go(&mut m, cmd::BLIT);
    assert_eq!([px(&m, 40, 140), px(&m, 41, 140), px(&m, 40, 141), px(&m, 41, 141)], [stamp_at(0, 0), stamp_at(2, 0), stamp_at(0, 2), stamp_at(2, 2)]);
    // flipped both ways
    dest(&mut m, 50, 50, 4, 4);
    m.poke(N + reg::NFLAGS, flag::FLIPX | flag::FLIPY);
    go(&mut m, cmd::BLIT);
    for y in 0..4 {
        for x in 0..4 {
            assert_eq!(px(&m, 50 + x, 150 + y), stamp_at(3 - x, 3 - y));
        }
    }
}

#[test]
fn a_blit_clipped_at_the_edge_still_takes_the_right_source_pixels() {
    let mut m = machine();
    stamp(&mut m);
    source(&mut m, 160, 0, 4, 4);
    // 4 x 4 drawn 400 x 400 with its middle on the page's: a wall close up
    dest(&mut m, -120, -150, 400, 400);
    assert_eq!(go(&mut m, cmd::BLIT), 4 + 4000);
    for (x, y) in [(0, 0), (79, 49), (80, 50), (159, 99)] {
        assert_eq!(px(&m, x, 100 + y), stamp_at((x + 120) / 100, (y + 150) / 100), "({x}, {y})");
    }
}

#[test]
fn the_key_colour_is_see_through_and_the_ink_table_recolours() {
    let mut m = machine();
    stamp(&mut m);
    source(&mut m, 160, 0, 4, 1); // colours 1 2 3 4
    m.poke(N + reg::NCOLOUR, 14);
    dest(&mut m, 0, 0, 160, 100);
    go(&mut m, cmd::FILL);
    m.poke(N + reg::NKEY, 2);
    m.poke(N + reg::NFLAGS, flag::KEY);
    dest(&mut m, 0, 0, 4, 1);
    go(&mut m, cmd::BLIT);
    assert_eq!([px(&m, 0, 100), px(&m, 1, 100), px(&m, 2, 100), px(&m, 3, 100)], [1, 14, 3, 4]);
    // table 2: 1 becomes 0 and 3 becomes 7; the key is tested on the source's colour, before
    m.poke(N + reg::NTABLE, 2);
    m.poke(N + reg::NINK + 1, 0);
    m.poke(N + reg::NINK + 3, 7);
    m.poke(N + reg::NINK + 2, 9);
    m.poke(N + reg::NFLAGS, flag::KEY | 2 << flag::TABLE_SHIFT);
    dest(&mut m, 0, 1, 4, 1);
    go(&mut m, cmd::BLIT);
    assert_eq!([px(&m, 0, 101), px(&m, 1, 101), px(&m, 2, 101), px(&m, 3, 101)], [0, 14, 7, 4]);
    // table 0 is still itself
    m.poke(N + reg::NTABLE, 0);
    assert_eq!(m.peek(N + reg::NINK + 3), 3);
}

#[test]
fn a_command_takes_the_registers_as_they_stood() {
    let mut m = machine();
    m.poke(N + reg::NCOLOUR, 4);
    dest(&mut m, 0, 0, 160, 100);
    m.poke(N + reg::NGO, cmd::FILL);
    // changed while the first is still drawing
    m.poke(N + reg::NCOLOUR, 6);
    dest(&mut m, 0, 0, 1, 1);
    go(&mut m, cmd::FILL);
    assert_eq!(px(&m, 0, 100), 6);
    assert_eq!(px(&m, 1, 100), 4);
    assert_eq!(px(&m, 159, 199), 4);
}

#[test]
fn the_depth_line() {
    let mut m = machine();
    // a far wall over columns 0-9 at depth 100, a near one over 10-19 at depth 20
    m.poke(N + reg::NFLAGS, flag::ZWRITE);
    m.poke(N + reg::NCOLOUR, 1);
    m.poke(N + reg::NDEPTH, 100);
    dest(&mut m, 0, 0, 10, 10);
    go(&mut m, cmd::FILL);
    m.poke(N + reg::NDEPTH, 20);
    dest(&mut m, 10, 0, 10, 10);
    go(&mut m, cmd::FILL);
    // a thing at depth 50 across both: it shows in front of the far wall only
    m.poke(N + reg::NFLAGS, flag::ZTEST);
    m.poke(N + reg::NCOLOUR, 2);
    m.poke(N + reg::NDEPTH, 50);
    dest(&mut m, 0, 5, 30, 1);
    go(&mut m, cmd::FILL);
    assert_eq!(px(&m, 9, 105), 2);
    assert_eq!(px(&m, 10, 105), 1, "behind the near wall");
    assert_eq!(px(&m, 20, 105), 2, "columns no wall wrote are 255, as far as can be");
    // the same depth as the wall is not in front of it
    m.poke(N + reg::NDEPTH, 100);
    m.poke(N + reg::NCOLOUR, 3);
    go(&mut m, cmd::FILL);
    assert_eq!(px(&m, 9, 105), 2);
    // tested and written at once: only where it passed
    m.poke(N + reg::NFLAGS, flag::ZTEST | flag::ZWRITE);
    m.poke(N + reg::NDEPTH, 60);
    go(&mut m, cmd::FILL);
    let n = m.mem.niece.as_ref().unwrap();
    assert_eq!((n.depth[9], n.depth[10], n.depth[20]), (60, 20, 60));
}

#[test]
fn line_draws_both_ends_and_costs_its_length() {
    let mut m = machine();
    m.poke(N + reg::NCOLOUR, 7);
    dest(&mut m, 10, 10, 8, 4);
    // 9 pixels: 4 + 3
    assert_eq!(go(&mut m, cmd::LINE), 7);
    let got: Vec<i32> = (10..=18).map(|x| (0..100).find(|&y| px(&m, x, 100 + y) == 7).unwrap()).collect();
    assert_eq!(got, [10, 11, 11, 12, 12, 13, 13, 14, 14]);
    // backwards and steep, off the page at one end
    dest(&mut m, 3, 5, -6, -20);
    assert_eq!(go(&mut m, cmd::LINE), 4 + 6);
    assert_eq!(px(&m, 3, 105), 7);
    assert_eq!(px(&m, 2, 100), 7, "(1.5, 0) rounds to x = 2");
    // a dot
    dest(&mut m, 100, 50, 0, 0);
    assert_eq!(go(&mut m, cmd::LINE), 5);
    assert_eq!(px(&m, 100, 150), 7);
}

#[test]
fn the_sheet_flag_draws_anywhere_on_the_sheet() {
    let mut m = machine();
    m.poke(N + reg::NCOLOUR, 11);
    m.poke(N + reg::NFLAGS, flag::SHEET);
    dest(&mut m, 250, 500, 20, 20);
    // 6 x 12 on the sheet
    assert_eq!(go(&mut m, cmd::FILL), 4 + 18);
    assert_eq!(px(&m, 255, 511), 11);
    assert_eq!(px(&m, 249, 500), 0);
    assert_eq!(px(&m, 0, 0), 0);
}

#[test]
fn flip_waits_for_the_beam_to_leave_the_picture() {
    let mut m = machine();
    idle(&mut m, 1);
    let vblank = (VBLANK_LINE * CYCLES_PER_LINE) as u64;
    m.poke(N + reg::NGO, cmd::FLIP);
    m.poke(N + reg::NCOLOUR, 8);
    dest(&mut m, 0, 0, 1, 1);
    m.poke(N + reg::NGO, cmd::FILL);
    let to_go = vblank - m.mem.frame_cycle as u64;
    idle(&mut m, to_go as u32 - 1);
    assert_eq!(m.peek(N + reg::NSTATUS), niece::STATUS_BUSY, "page 0 still shown, and the fill waits behind the flip");
    assert_eq!(m.peek(N + reg::NQUEUE), 2);
    idle(&mut m, 1);
    assert_eq!(m.mem.frame_cycle as u64, vblank);
    assert_eq!(m.peek(N + reg::NSTATUS), niece::STATUS_BUSY | niece::STATUS_PAGE);
    idle(&mut m, 5);
    assert_eq!(m.peek(N + reg::NSTATUS), niece::STATUS_PAGE);
    assert_eq!(px(&m, 0, 0), 8, "the fill drew on page 0, which isn't shown any more");
    // a flip written exactly as the beam leaves flips at once
    idle(&mut m, CYCLES_PER_FRAME - 5);
    m.poke(N + reg::NGO, cmd::FLIP);
    m.poke(N + reg::NCTRL, 0);
    assert_eq!(m.peek(N + reg::NSTATUS), 0, "flipped back, in no time");
}

#[test]
fn the_page_shows_behind_the_picture_two_by_two() {
    let mut m = machine();
    m.poke(N + reg::NFLAGS, flag::SHEET);
    m.poke(N + reg::NCOLOUR, 6);
    dest(&mut m, 0, 0, 160, 100);
    go(&mut m, cmd::FILL);
    m.poke(N + reg::NCOLOUR, 10);
    dest(&mut m, 159, 99, 1, 1);
    go(&mut m, cmd::FILL);
    let mut out = vec![0u8; PIXELS_W * PIXELS_H];
    m.render(true, &mut out);
    assert!(out.iter().all(|&p| p < 4), "not shown until NCTRL says so");
    m.poke(N + reg::NCTRL, 1);
    // a character on the screen: its ink stays in front
    m.poke(0x0400, b'A');
    m.render(true, &mut out);
    assert_eq!(out[100 * PIXELS_W + 100], NIECE_PIXEL | 6);
    for (x, y) in [(318, 198), (319, 198), (318, 199), (319, 199)] {
        assert_eq!(out[y * PIXELS_W + x], NIECE_PIXEL | 10);
    }
    assert_eq!(out[199 * PIXELS_W + 317], NIECE_PIXEL | 6);
    let cell: Vec<u8> = (0..8).flat_map(|y| out[y * PIXELS_W..y * PIXELS_W + 8].to_vec()).collect();
    assert!(cell.contains(&1) && cell.contains(&(NIECE_PIXEL | 6)), "A's ink over the page");
}

#[test]
fn the_beam_shows_each_line_as_the_page_stood_then() {
    let mut m = machine();
    m.poke(N + reg::NCTRL, 1);
    idle(&mut m, 1);
    // run to the top of a frame, then fill the shown page half-way down the picture
    let rest = CYCLES_PER_FRAME - m.mem.frame_cycle;
    idle(&mut m, rest);
    idle(&mut m, (51 + 100) * CYCLES_PER_LINE);
    m.poke(N + reg::NFLAGS, flag::SHEET);
    m.poke(N + reg::NCOLOUR, 13);
    dest(&mut m, 0, 0, 160, 100);
    m.poke(N + reg::NGO, cmd::FILL);
    let rest = CYCLES_PER_FRAME - m.mem.frame_cycle;
    idle(&mut m, rest);
    let f = m.frame();
    assert_eq!(f[0], NIECE_PIXEL, "the top was drawn before the fill: colour 0");
    assert_eq!(f[199 * PIXELS_W], NIECE_PIXEL | 13);
    // the fill took 4,004 cycles from the start of line 151: it finished 62 and a half lines
    // on, so the 62 lines before that were drawn without it
    assert_eq!(f[(100 + 61) * PIXELS_W], NIECE_PIXEL);
    assert_eq!(f[(100 + 62) * PIXELS_W], NIECE_PIXEL | 13);
}

#[test]
fn arithmetic() {
    let mut m = machine();
    let mul = |m: &mut Machine, a: i32, b: i32| {
        word(m, reg::NMULA, a);
        word(m, reg::NMULB, b);
        i32::from_le_bytes([0, 1, 2, 3].map(|i| m.peek(N + reg::NPROD + i)))
    };
    assert_eq!(mul(&mut m, 300, 200), 60_000);
    assert_eq!(mul(&mut m, -300, 200), -60_000);
    assert_eq!(mul(&mut m, -32768, -32768), 1 << 30);
    assert_eq!(mul(&mut m, 32767, -1), -32767);
    assert_eq!(m.peek(N + reg::NMULA), 0xFF, "as written");
    let div = |m: &mut Machine, n: u32, d: u32| {
        for (i, b) in n.to_le_bytes().into_iter().enumerate() {
            m.poke(N + reg::NDIVN + i as u16, b);
        }
        word(m, reg::NDIVD, d as i32);
        let q = u32::from_le_bytes([0, 1, 2, 3].map(|i| m.peek(N + reg::NQUOT + i)));
        (q, m.peek(N + reg::NREM) as u32 | (m.peek(N + reg::NREM + 1) as u32) << 8)
    };
    assert_eq!(div(&mut m, 65535, 7), (9362, 1));
    assert_eq!(div(&mut m, 0x1234_5678, 0xFFFF), (0x1234, 0x68AC));
    assert_eq!(div(&mut m, 0xFFFF_FFFF, 1), (0xFFFF_FFFF, 0));
    assert_eq!(div(&mut m, 5, 0), (0xFFFF_FFFF, 0xFFFF));
    assert_eq!(m.peek(N + reg::NDIVN), 5);
}

#[test]
fn a_reset_empties_the_chip_and_a_save_state_keeps_it() {
    let mut m = machine();
    // a program that goes round and round, so the processor's clock is the machine's
    m.load(0x0800, &[0x06, 0x00, 0x08]); // JMP $0800
    m.cpu.state = unc_core::CpuState::Running;
    m.cpu.pc = 0x0800;
    m.run_cycles(100);
    stamp(&mut m);
    source(&mut m, 160, 0, 4, 4);
    dest(&mut m, 0, 0, 160, 100);
    m.poke(N + reg::NCTRL, 1);
    m.poke(N + reg::NGO, cmd::BLIT);
    m.poke(N + reg::NGO, cmd::FLIP);
    m.poke(N + reg::NGO, cmd::BLIT);
    m.run_cycles(1000);
    let saved = m.save_state();
    let mut other = Machine::new(RamSize::K16);
    other.load_state(&saved).unwrap();
    assert!(other.has_niece());
    assert_eq!(other.peek(N + reg::NQUEUE), 3);
    for _ in 0..3 {
        m.run_frame();
        other.run_frame();
        assert_eq!(m.frame(), other.frame());
    }
    assert_eq!(m.peek(N + reg::NQUEUE), 0);
    assert_eq!(m.save_state(), other.save_state());
    // a state from a machine without it takes it out
    let plain = Machine::new(RamSize::K16).save_state();
    other.load_state(&plain).unwrap();
    assert!(!other.has_niece());
    m.reset();
    assert!(m.has_niece(), "a reset keeps the chip in");
    assert_eq!(m.peek(N + reg::NCTRL), 0);
    assert_eq!(px(&m, 160, 0), 0);
}

#[test]
fn the_short_registers_set_both_bytes_at_once() {
    let mut m = machine();
    dest(&mut m, 0x1234, 0x1234, 0x1234, 0x1234);
    m.poke(N + reg::NDXS, 0xFE);
    m.poke(N + reg::NDYS, 0x7F);
    m.poke(N + reg::NDWS, 200);
    m.poke(N + reg::NDHS, 0);
    let word = |m: &Machine, r: u16| m.peek(N + r) as u16 | (m.peek(N + r + 1) as u16) << 8;
    assert_eq!(word(&m, reg::NDX), 0xFFFE, "signed: -2");
    assert_eq!(word(&m, reg::NDY), 0x007F);
    assert_eq!(word(&m, reg::NDW), 200, "a size isn't signed");
    assert_eq!(word(&m, reg::NDH), 0);
    assert_eq!((m.peek(N + reg::NDXS), m.peek(N + reg::NDWS)), (0xFE, 200), "they read the low bytes");
    // a box hanging off the top left corner, in four stores
    m.poke(N + reg::NDXS, (-3i8) as u8);
    m.poke(N + reg::NDYS, (-3i8) as u8);
    m.poke(N + reg::NDWS, 5);
    m.poke(N + reg::NDHS, 5);
    m.poke(N + reg::NCOLOUR, 9);
    assert_eq!(go(&mut m, cmd::FILL), 4 + 1);
    assert_eq!((px(&m, 1, 101), px(&m, 2, 101), px(&m, 1, 102)), (9, 0, 0));
}

#[test]
fn a_patterned_fill_mixes_two_colours() {
    let mut m = machine();
    // a chequerboard of blue and light blue: 0101 on even lines, 1010 on odd
    m.poke(N + reg::NCOLOUR, 14 << 4 | 6);
    m.poke(N + reg::NPATTERN, 0b0101_1010);
    assert_eq!(m.peek(N + reg::NPATTERN), 0b0101_1010);
    dest(&mut m, 0, 0, 160, 100);
    assert_eq!(go(&mut m, cmd::FILL), 4004, "it costs what a plain fill does");
    for (x, y) in [(0, 0), (1, 0), (2, 0), (3, 0), (0, 1), (1, 1), (158, 98), (159, 99)] {
        assert_eq!(px(&m, x, 100 + y), if (x + y) % 2 == 1 { 14 } else { 6 }, "({x}, {y})");
    }
    // the pattern is laid from the page's corner, not the rectangle's: a second fill joins up
    m.poke(N + reg::NCOLOUR, 7 << 4 | 2);
    dest(&mut m, 3, 1, 5, 3);
    go(&mut m, cmd::FILL);
    for (x, y) in [(3, 1), (4, 1), (7, 3)] {
        assert_eq!(px(&m, x, 100 + y), if (x + y) % 2 == 1 { 7 } else { 2 }, "({x}, {y})");
    }
    // one pixel in four, on even lines only
    m.poke(N + reg::NPATTERN, 0b1000_0000);
    dest(&mut m, 0, 0, 8, 2);
    go(&mut m, cmd::FILL);
    let row = |m: &Machine, y: i32| (0..8).map(|x| px(m, x, 100 + y)).collect::<Vec<u8>>();
    assert_eq!(row(&m, 0), [7, 2, 2, 2, 7, 2, 2, 2]);
    assert_eq!(row(&m, 1), [2; 8]);
    // LINE draws in the first colour, and with no pattern so does FILL
    m.poke(N + reg::NPATTERN, 0);
    dest(&mut m, 0, 50, 3, 0);
    go(&mut m, cmd::LINE);
    go(&mut m, cmd::FILL);
    assert_eq!(row(&m, 50)[..4], [2, 2, 2, 2]);
}
