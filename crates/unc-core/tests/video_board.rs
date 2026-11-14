//! The video board, Rev A (manual chapter 15): the sprite table, eight a line, 2-bit
//! images, flips, wide and behind, the collision and line registers, four colours and the
//! 4-colour bitmap, fine scrolling, the layer, BOARD, and a machine without the board, which
//! must not change.

use unc_core::board::{BOARD_ID, attr};
use unc_core::cpu::Bus;
use unc_core::isa::{Mode, Op, opcode_for};
use unc_core::machine::{Machine, PIXELS_H, PIXELS_W, RamSize, io};

const W: usize = PIXELS_W;
const TABLE: u16 = 0x3700;
const BANK: u16 = 0x3800;

/// A machine with the board, idling in `JMP $0800` instead of booting, on a blank screen
/// (screen code 0 is an empty cell) and an empty sprite table.
fn board() -> Machine {
    let mut m = Machine::with_board(RamSize::K16, true);
    let jmp = opcode_for(Op::Jmp, Mode::Abs).unwrap();
    m.load(0x0800, &[jmp, 0x00, 0x08]);
    m.cpu.pc = 0x0800;
    m
}

fn picture(m: &Machine) -> Vec<u8> {
    let mut px = vec![0u8; W * PIXELS_H];
    m.render(true, &mut px);
    px
}

fn lit(px: &[u8]) -> usize {
    px.iter().filter(|&&p| p != 0).count()
}

/// Image p from 16 rows of 16 pixel values, 0-3: four pixels a byte, the leftmost in bits 7-6.
fn image(m: &mut Machine, p: u8, rows: &[[u8; 16]; 16]) {
    let at = BANK + p as u16 * 64;
    for (r, row) in rows.iter().enumerate() {
        for b in 0..4 {
            let byte = (0..4).fold(0u8, |acc, i| acc << 2 | row[b * 4 + i] & 3);
            m.poke(at + (r * 4 + b) as u16, byte);
        }
    }
}

fn solid(v: u8) -> [[u8; 16]; 16] {
    [[v; 16]; 16]
}

/// Table entry n: shown at (x, y), 9-bit x, with image `p` and the attribute bits `a`.
fn sprite(m: &mut Machine, n: u16, x: u16, y: u8, p: u8, a: u8) {
    let at = TABLE + n * 4;
    m.load(at, &[y, x as u8, p, a | attr::SHOWN | (x >> 8) as u8 & attr::X8]);
}

/// The pixels of line y from x, w of them.
fn line(px: &[u8], x: usize, y: usize, w: usize) -> Vec<u8> {
    px[y * W + x..][..w].to_vec()
}

/// What the processor reads (with its side effects), not the host's `peek`.
fn cpu_read(m: &mut Machine, addr: u16) -> u8 {
    m.mem.read(addr)
}

const UNUSED: [std::ops::RangeInclusive<u16>; 5] =
    [0xC080..=0xC09F, 0xC0A2..=0xC0A3, 0xC0A7..=0xC0A7, 0xC0AE..=0xC0AF, 0xC0B4..=0xC0BE];

#[test]
fn without_the_board_every_register_reads_ff_and_ink_is_1() {
    for mut m in [Machine::new(RamSize::K16), Machine::new(RamSize::K64), Machine::with_board(RamSize::K16, false)] {
        assert!(!m.has_board());
        for a in 0xC080..=0xC0BF {
            m.poke(a, 0x00);
            m.poke(a, 0x5A);
            assert_eq!(m.peek(a), 0xFF, "${a:04X}");
            assert_eq!(cpu_read(&mut m, a), 0xFF, "${a:04X}");
        }
        m.poke(0x0400, 0x0F);
        assert!(picture(&m).iter().all(|&p| p <= 1), "1 for ink, 0 for paper");
        assert_eq!(picture(&m)[0], 1);
        m.poke(io::BACKGROUND, 6);
        assert_eq!((m.colour_of(0), m.colour_of(1), m.colour_of(3)), (6, 1, 1));
    }
}

#[test]
fn board_reads_15_and_the_registers_start_at_their_reset_values() {
    let m = Machine::with_board(RamSize::K16, true);
    assert!(m.has_board());
    assert_eq!((m.peek(io::BOARD), BOARD_ID), (0x15, 0x15));
    assert_eq!((m.peek(io::SPRTAB), m.peek(io::SPRBANK)), (0x37, 0x38), "the table at $3700, the images at $3800");
    assert_eq!(m.peek(io::LAYERPAGE), 0x3C, "the layer at $3C00");
    assert_eq!((m.peek(io::COLOUR1), m.peek(io::COLOUR2), m.peek(io::COLOUR3)), (11, 15, 1));
    for a in [io::SPRHIT, io::SPRHITBG, io::SPRLINE, io::SCROLLX, io::SCROLLY, io::LAYER, io::LAYERSX, io::LAYERSY, io::COLOURMODE] {
        assert_eq!(m.peek(a), 0, "${a:04X}");
    }
    // the first board's sprite registers are gone: unused, like every other unused address
    for a in UNUSED.into_iter().flatten() {
        assert_eq!(m.peek(a), 0xFF, "${a:04X}");
    }
}

#[test]
fn every_register_reads_back_what_was_written() {
    let mut m = Machine::with_board(RamSize::K16, true);
    for (i, a) in [io::SPRTAB, io::SPRBANK, io::LAYERPAGE].into_iter().enumerate() {
        let v = 0x5A ^ i as u8;
        m.poke(a, v);
        assert_eq!(m.peek(a), v, "${a:04X}");
    }
    for a in [io::SCROLLX, io::SCROLLY, io::LAYERSX, io::LAYERSY] {
        m.poke(a, 0xFD);
        assert_eq!(m.peek(a), 5, "${a:04X}: three bits");
    }
    m.poke(io::LAYER, 0xFF);
    m.poke(io::COLOURMODE, 0xFF);
    assert_eq!((m.peek(io::LAYER), m.peek(io::COLOURMODE)), (1, 1), "one bit each");
    for a in [io::COLOUR1, io::COLOUR2, io::COLOUR3] {
        m.poke(a, 0xF7);
        assert_eq!(m.peek(a), 7, "${a:04X}: a colour, 0-15");
    }
    for a in [io::SPRHIT, io::SPRHITBG, io::SPRLINE] {
        m.poke(a, 0xFF);
        assert_eq!(m.peek(a), 0, "${a:04X} is read-only");
    }
    m.poke(io::BOARD, 0);
    assert_eq!(m.peek(io::BOARD), 0x15);
    for a in UNUSED.into_iter().flatten() {
        m.poke(a, 0);
        assert_eq!(m.peek(a), 0xFF, "${a:04X}");
    }
}

#[test]
fn a_reset_keeps_the_board_and_resets_its_registers() {
    let mut m = Machine::with_board(RamSize::K16, true);
    m.poke(io::SPRTAB, 0x20);
    m.poke(io::COLOUR2, 3);
    m.reset();
    assert!(m.has_board());
    assert_eq!((m.peek(io::SPRTAB), m.peek(io::COLOUR2), m.peek(io::BOARD)), (0x37, 15, 0x15));
}

#[test]
fn with_the_board_the_picture_is_colour_numbers_and_looks_as_before() {
    let mut m = board();
    m.poke(0x0400, 0x0F);
    let px = picture(&m);
    assert_eq!((px[0], px[8]), (3, 0), "text ink is colour 3, paper 0");
    m.poke(io::BACKGROUND, 6);
    assert_eq!((m.colour_of(0), m.colour_of(1), m.colour_of(2), m.colour_of(3)), (6, 11, 15, 1), "ink is white, as before");
    m.poke(io::COLOUR3, 7);
    m.poke(io::COLOUR1, 2);
    assert_eq!((m.colour_of(3), m.colour_of(1)), (7, 2));
    // the 1-bit bitmap's ink is colour 3 too
    m.poke(io::VMODE, io::VMODE_BITMAP);
    m.poke(0x2000, 0x80);
    let px = picture(&m);
    assert_eq!((px[0], px[1], lit(&px)), (3, 0, 1));
}

#[test]
fn a_sprite_is_a_table_entry_and_a_2_bit_image() {
    let mut m = board();
    // image 3: every row 0 1 2 3 0 1 2 3 ..., one further along each row
    let rows: [[u8; 16]; 16] = std::array::from_fn(|r| std::array::from_fn(|c| ((c + r) % 4) as u8));
    image(&mut m, 3, &rows);
    m.load(TABLE + 5 * 4, &[50, 100, 3, 0]); // entry 5, not shown: bit 7 clear
    assert_eq!(lit(&picture(&m)), 0);
    sprite(&mut m, 5, 100, 50, 3, 0);
    let px = picture(&m);
    for (r, row) in rows.iter().enumerate() {
        assert_eq!(line(&px, 100, 50 + r, 16), row.to_vec(), "row {r}: pixel values are colour numbers");
    }
    assert_eq!(lit(&px), 16 * 12, "colour 0 is see-through: three pixels in four show");
    assert_eq!(&m.mem.ram[0x38C0..0x38C4], &[0b00011011; 4], "image 3 at $3800 + 3 * 64");
    // SPRBANK and SPRTAB move the images and the table
    m.poke(io::SPRBANK, 0x30);
    m.load(0x3000 + 3 * 64, &[0b11000000]);
    let px = picture(&m);
    assert_eq!((lit(&px), px[50 * W + 100]), (1, 3));
    m.poke(io::SPRTAB, 0x36);
    assert_eq!(lit(&picture(&m)), 0, "an empty table at $3600");
    // the image's address wraps at 16 bits: image 255 of bank $FC is $FC00 + 255 * 64 = $3BC0
    m.poke(io::SPRTAB, 0x37);
    m.poke(io::SPRBANK, 0xFC);
    m.load(TABLE + 5 * 4 + 2, &[255]);
    m.load(0x3BC0, &[0b01000000]);
    let px = picture(&m);
    assert_eq!((lit(&px), px[50 * W + 100]), (1, 1));
}

#[test]
fn x_320_to_511_is_off_the_left_and_y_200_to_255_off_the_top() {
    let mut m = board();
    image(&mut m, 0, &solid(2));
    let cols = |px: &[u8], y: usize| (0..W).filter(|&x| px[y * W + x] != 0).collect::<Vec<_>>();
    let rows = |px: &[u8], x: usize| (0..PIXELS_H).filter(|&y| px[y * W + x] != 0).collect::<Vec<_>>();
    sprite(&mut m, 0, 510, 20, 0, 0); // x = -2: attribute bit 0 is bit 8
    assert_eq!(cols(&picture(&m), 20), (0..14).collect::<Vec<_>>());
    sprite(&mut m, 0, 320, 20, 0, 0);
    assert_eq!(lit(&picture(&m)), 0, "-192: gone");
    sprite(&mut m, 0, 319, 20, 0, 0);
    assert_eq!(cols(&picture(&m), 20), [319]);
    sprite(&mut m, 0, 0x100, 20, 0, 0);
    assert_eq!(cols(&picture(&m), 20), (256..272).collect::<Vec<_>>());
    sprite(&mut m, 0, 40, 250, 0, 0); // y = -6
    assert_eq!(rows(&picture(&m), 40), (0..10).collect::<Vec<_>>());
    sprite(&mut m, 0, 40, 200, 0, 0);
    assert_eq!(lit(&picture(&m)), 0, "-56: gone");
    sprite(&mut m, 0, 40, 199, 0, 0);
    assert_eq!(rows(&picture(&m), 40), [199]);
}

#[test]
fn flips_and_wide() {
    let mut m = board();
    let mut rows = [[0u8; 16]; 16];
    rows[0][0] = 1; // top left
    rows[0][1] = 2;
    rows[15][15] = 3; // bottom right
    image(&mut m, 1, &rows);
    let at = |px: &[u8], x: usize, y: usize| px[(10 + y) * W + 10 + x];
    for (a, pixels) in [
        (0, [(0, 0, 1), (1, 0, 2), (15, 15, 3)]),
        (attr::FLIPX, [(15, 0, 1), (14, 0, 2), (0, 15, 3)]),
        (attr::FLIPY, [(0, 15, 1), (1, 15, 2), (15, 0, 3)]),
        (attr::FLIPX | attr::FLIPY, [(15, 15, 1), (14, 15, 2), (0, 0, 3)]),
    ] {
        sprite(&mut m, 0, 10, 10, 1, a);
        let px = picture(&m);
        for (x, y, v) in pixels {
            assert_eq!(at(&px, x, y), v, "attr {a:02X}: ({x}, {y})");
        }
        assert_eq!(lit(&px), 3, "attr {a:02X}");
    }
    sprite(&mut m, 0, 10, 10, 1, attr::WIDE);
    let px = picture(&m);
    assert_eq!(lit(&px), 12, "each pixel 2 x 2");
    for (x, y, v) in [(0, 0, 1), (1, 1, 1), (2, 0, 2), (3, 1, 2), (30, 30, 3), (31, 31, 3)] {
        assert_eq!(at(&px, x, y), v, "wide ({x}, {y})");
    }
    sprite(&mut m, 0, 10, 10, 1, attr::WIDE | attr::FLIPX);
    let px = picture(&m);
    assert_eq!((at(&px, 31, 0), at(&px, 28, 1), at(&px, 0, 31)), (1, 2, 3), "wide and flipped");
    // bits 5 and 6 are spare: kept in memory, ignored
    sprite(&mut m, 0, 10, 10, 1, 0x60);
    assert_eq!(lit(&picture(&m)), 3);
}

#[test]
fn lower_entries_draw_on_top_and_behind_shows_only_on_colour_0() {
    let mut m = board();
    image(&mut m, 0, &solid(1));
    image(&mut m, 1, &solid(2));
    sprite(&mut m, 0, 0, 0, 0, 0);
    sprite(&mut m, 1, 8, 0, 1, 0);
    let px = picture(&m);
    assert_eq!((px[0], px[10], px[20]), (1, 1, 2), "entry 0 over entry 1");
    // behind: the screen's ink wins, colour 0 lets the sprite show
    m.poke(0x0400, 0x0F); // a solid cell at (0-7, 0-7)
    sprite(&mut m, 0, 0, 0, 0, attr::BEHIND);
    let px = picture(&m);
    assert_eq!((px[0], px[7 * W + 7], px[8], px[8 * W]), (3, 3, 1, 1), "behind the ink, in front of paper");
    // a sprite behind still covers the ones under it on colour 0, and lets them show on ink
    sprite(&mut m, 1, 0, 0, 1, 0);
    let px = picture(&m);
    assert_eq!((px[0], px[8]), (2, 1), "on ink entry 1, in front, shows; on paper entry 0 is on top");
    // the layer is part of the picture too
    m.poke(0x0400, 0);
    m.poke(0x3C00, 0x0F);
    m.poke(io::LAYER, 1);
    sprite(&mut m, 1, 100, 100, 1, 0);
    image(&mut m, 0, &solid(3));
    let px = picture(&m);
    assert_eq!((px[0], px[8]), (1, 3), "behind the layer's ink (colour 1), in front of its paper");
}

#[test]
fn eight_a_line_in_table_order_and_sprline() {
    let mut m = board();
    image(&mut m, 0, &solid(2));
    for n in 0..10u16 {
        sprite(&mut m, n, n * 20, 50, 0, 0);
    }
    let px = picture(&m);
    for n in 0..10usize {
        assert_eq!(px[55 * W + n * 20], if n < 8 { 2 } else { 0 }, "entry {n}");
    }
    m.run_frame();
    assert_eq!(m.peek(io::SPRLINE), 0x80);
    assert_eq!(cpu_read(&mut m, io::SPRLINE), 0x80);
    assert_eq!(cpu_read(&mut m, io::SPRLINE), 0, "reading clears it");
    // entries 8 and 9 ten lines lower: on the lines they share with 0-7 they lose; below, they show
    sprite(&mut m, 8, 160, 60, 0, 0);
    sprite(&mut m, 9, 180, 60, 0, 0);
    let px = picture(&m);
    assert_eq!((px[62 * W + 160], px[70 * W + 160], px[70 * W + 180]), (0, 2, 2));
    // the first eight shown on the line, wherever they are across it: sprites parked off the
    // left edge still take their places
    for n in 0..8u16 {
        sprite(&mut m, n, 496, 50 + n as u8, 0, 0); // x = -16, lines 50+n to 65+n
    }
    sprite(&mut m, 8, 100, 60, 0, 0);
    sprite(&mut m, 9, 120, 60, 0, 0);
    let px = picture(&m);
    assert_eq!((px[60 * W + 100], px[60 * W + 120]), (0, 0), "line 60: eight parked sprites");
    assert_eq!((px[66 * W + 100], px[66 * W + 120]), (2, 0), "line 66: seven, so entry 8 is drawn and 9 isn't");
    assert_eq!((px[73 * W + 100], px[73 * W + 120]), (2, 2), "line 73: just the two");
    // a hidden entry takes no place; eight on a line exactly is fine
    for n in 0..10u16 {
        sprite(&mut m, n, n * 20, 50, 0, 0);
    }
    m.load(TABLE + 3, &[0]);
    m.load(TABLE + 4 * 9 + 3, &[0]);
    let px = picture(&m);
    assert_eq!((px[55 * W], px[55 * W + 160]), (0, 2), "entry 8 is now the eighth");
    m.run_frame();
    assert_eq!(m.peek(io::SPRLINE), 0);
}

#[test]
fn sprhit_latches_sprites_0_to_7_on_another_sprites_ink() {
    let mut m = board();
    let mut ring = solid(1);
    for row in ring.iter_mut().take(15).skip(1) {
        row[1..15].fill(0);
    }
    image(&mut m, 0, &ring); // a hollow square
    image(&mut m, 1, &solid(2));
    let mut dot = [[0u8; 16]; 16];
    dot[8][8] = 3;
    image(&mut m, 2, &dot);
    sprite(&mut m, 0, 100, 100, 0, 0);
    sprite(&mut m, 1, 100, 100, 2, 0);
    m.run_frame();
    assert_eq!(m.peek(io::SPRHIT), 0, "the dot sits in the ring's see-through middle");
    sprite(&mut m, 3, 110, 100, 1, 0);
    m.run_frame();
    assert_eq!(m.peek(io::SPRHIT), 0b1001);
    assert_eq!(m.peek(io::SPRHIT), 0b1001, "the host's peek doesn't clear it");
    sprite(&mut m, 3, 200, 100, 1, 0);
    m.run_frame();
    assert_eq!(cpu_read(&mut m, io::SPRHIT), 0b1001, "kept until it's read");
    assert_eq!(cpu_read(&mut m, io::SPRHIT), 0);
    // entry 12 on the ring: the ring's bit is set; 12 has no bit
    sprite(&mut m, 12, 92, 100, 1, 0);
    m.run_frame();
    assert_eq!(cpu_read(&mut m, io::SPRHIT), 0b0001);
    // behind or not, ink is ink
    sprite(&mut m, 12, 92, 100, 1, attr::BEHIND);
    m.run_frame();
    assert_eq!(cpu_read(&mut m, io::SPRHIT), 0b0001);
}

#[test]
fn sprhitbg_latches_sprite_ink_on_the_screen_or_the_layer() {
    let mut m = board();
    let mut half = solid(0);
    for row in half.iter_mut() {
        row[8..].fill(2); // ink only on the right half
    }
    image(&mut m, 0, &half);
    sprite(&mut m, 6, 160, 96, 0, 0);
    m.run_frame();
    assert_eq!(m.peek(io::SPRHITBG), 0, "blank screen");
    m.poke(0x0400 + 12 * 40 + 20, 0x0F); // under the sprite's see-through half
    m.run_frame();
    assert_eq!(m.peek(io::SPRHITBG), 0, "colour 0 of a sprite touches nothing");
    m.poke(0x0400 + 12 * 40 + 21, 0x08); // a quarter block under its ink
    m.run_frame();
    assert_eq!(cpu_read(&mut m, io::SPRHITBG), 1 << 6);
    assert_eq!(cpu_read(&mut m, io::SPRHITBG), 0, "reading clears it");
    m.poke(0x0400 + 12 * 40 + 21, 0x20);
    m.poke(0x3C00 + 12 * 40 + 21, 0x0F);
    m.run_frame();
    assert_eq!(m.peek(io::SPRHITBG), 0, "layer off");
    m.poke(io::LAYER, 1);
    m.run_frame();
    assert_eq!(cpu_read(&mut m, io::SPRHITBG), 1 << 6, "the layer's ink");
    // entries past 7 have no bit
    m.load(TABLE + 6 * 4, &[0, 0, 0, 0]);
    sprite(&mut m, 20, 160, 96, 0, 0);
    m.run_frame();
    assert_eq!(m.peek(io::SPRHITBG), 0);
}

#[test]
fn scrollx_and_scrolly_move_the_main_picture_with_colour_0_behind() {
    let mut m = board();
    m.poke(0x0400, 0x0F);
    m.poke(0x0400 + 24 * 40 + 39, 0x0F);
    m.poke(io::SCROLLX, 3);
    m.poke(io::SCROLLY, 5);
    let px = picture(&m);
    let ink = |x: usize, y: usize| px[y * W + x];
    assert_eq!((ink(2, 5), ink(3, 4), ink(3, 5), ink(10, 12), ink(11, 13)), (0, 0, 3, 3, 0));
    assert_eq!(lit(&px), 64 + 5 * 3, "the bottom-right cell is cut to 5 x 3");
    // the sprites don't move with it
    image(&mut m, 0, &solid(1));
    sprite(&mut m, 0, 100, 100, 0, 0);
    assert_eq!(picture(&m)[100 * W + 100], 1);
}

#[test]
fn the_layer_is_colour_1_where_the_main_picture_is_0() {
    let mut m = board();
    for cell in 0..1000u16 {
        m.poke(0x3C00 + cell, 0x20);
    }
    m.poke(0x3C00 + 41, b'A');
    assert_eq!(lit(&picture(&m)), 0, "LAYER is off");
    m.poke(io::LAYER, 1);
    let a: Vec<u8> = (0..8).map(|y| m.peek(0xD000 + 0x41 * 8 + y)).collect();
    let cell = |px: &[u8], dx: usize, dy: usize| -> Vec<u8> {
        (0..8).map(|y| (0..8).fold(0u8, |b, x| b << 1 | (px[(8 + y + dy) * W + 8 + x + dx] != 0) as u8)).collect()
    };
    let px = picture(&m);
    assert_eq!(cell(&px, 0, 0), a, "the layer's A");
    assert!(px.iter().all(|&p| p <= 1), "in colour 1");
    m.poke(io::LAYERSX, 2);
    m.poke(io::LAYERSY, 1);
    let px = picture(&m);
    assert_eq!(cell(&px, 2, 1), a, "its own scroll");
    m.poke(io::SCROLLX, 7);
    assert_eq!(picture(&m), px, "SCROLLX doesn't move it");
    // the screen in front: its ink (3) wins
    m.poke(io::SCROLLX, 0);
    m.poke(0x0400 + 41, 0xA0); // a reversed space: all ink
    let px = picture(&m);
    assert!((0..8).all(|y| (8..16).all(|x| px[(8 + y) * W + x] == 3)));
}

#[test]
fn the_4_colour_bitmap_is_160_by_200_at_2_bits_a_pixel() {
    let mut m = board();
    m.poke(io::VMODE, io::VMODE_BITMAP);
    m.poke(0x2000, 0b00_01_10_11); // line 0: pixels 0-3
    m.poke(0x2000 + 199 * 40 + 39, 0b11_00_00_10); // line 199: pixels 156 and 159
    let px = picture(&m);
    assert_eq!(line(&px, 0, 0, 8), [0, 0, 0, 3, 3, 0, 3, 3], "1 bit a pixel until COLOURMODE");
    m.poke(io::COLOURMODE, 1);
    let px = picture(&m);
    assert_eq!(line(&px, 0, 0, 8), [0, 0, 1, 1, 2, 2, 3, 3], "each pixel two wide");
    assert_eq!(line(&px, 312, 199, 8), [3, 3, 0, 0, 0, 0, 2, 2]);
    assert_eq!(lit(&px), 6 + 4);
    // mixed: text rows 21-24 below, ink 3
    m.poke(io::VMODE, io::VMODE_BITMAP | io::VMODE_MIXED);
    m.poke(0x0400 + 21 * 40, 0x0F);
    let px = picture(&m);
    assert_eq!((px[0], px[2], px[168 * W], px[199 * W + 312]), (0, 1, 3, 0));
    // without the bitmap, COLOURMODE changes nothing
    m.poke(io::VMODE, 0);
    assert_eq!(picture(&m)[168 * W], 3);
    // and without the board there's no COLOURMODE
    let mut plain = Machine::new(RamSize::K16);
    plain.poke(io::VMODE, io::VMODE_BITMAP);
    plain.poke(0xC0B0, 1);
    plain.poke(0x2000, 0b00_01_10_11);
    let mut px = vec![0u8; W * PIXELS_H];
    plain.render(true, &mut px);
    assert_eq!(line(&px, 0, 0, 8), [0, 0, 0, 1, 1, 0, 1, 1]);
}

#[test]
fn observe_counts_the_board_registers_like_any_other() {
    for on in [false, true] {
        let mut m = Machine::with_board(RamSize::K16, on);
        let sta = opcode_for(Op::Sta, Mode::Abs).unwrap();
        let lda = opcode_for(Op::Lda, Mode::Abs).unwrap();
        let hlt = opcode_for(Op::Hlt, Mode::Imp).unwrap();
        m.load(0x0800, &[sta, 0xA0, 0xC0, sta, 0xB2, 0xC0, lda, 0xA4, 0xC0, lda, 0xBF, 0xC0, lda, 0xBF, 0xC0, hlt]);
        m.cpu.pc = 0x0800;
        m.observe();
        m.run_frame();
        let o = m.observe();
        assert_eq!((o.write_count(io::SPRTAB), o.write_count(io::COLOUR2)), (1, 1), "board {on}");
        assert_eq!((o.read_count(io::SPRHIT), o.read_count(io::BOARD)), (1, 2), "board {on}");
        assert_eq!(m.cpu.a, if on { 0x15 } else { 0xFF });
    }
}

#[test]
fn the_banner_says_the_board_is_in() {
    for ram in [RamSize::K16, RamSize::K64] {
        let plain = Machine::boot(ram).screen_text();
        assert!(!plain.contains("VIDEO BOARD"), "{plain}");
        let m = Machine::boot_with_board(ram, true);
        let text = m.screen_text();
        let lines: Vec<&str> = text.lines().collect();
        let k = if ram == RamSize::K64 { "64K" } else { "16K" };
        assert_eq!(lines[2], format!(" {k} RAM SYSTEM"), "{text}");
        assert_eq!(lines[3], " VIDEO BOARD REV A", "{text}");
        let mut without: Vec<&str> = plain.lines().collect();
        without.insert(3, " VIDEO BOARD REV A");
        assert_eq!(lines[..8], without[..8]);
    }
}

#[test]
fn save_states_keep_the_board_and_its_registers() {
    let mut m = board();
    image(&mut m, 0, &solid(2));
    sprite(&mut m, 0, 300, 10, 0, 0);
    sprite(&mut m, 1, 305, 12, 0, attr::FLIPX);
    m.poke(io::SCROLLX, 4);
    m.poke(io::LAYER, 1);
    m.poke(io::COLOUR1, 9);
    m.poke(io::COLOURMODE, 1);
    m.run_frame();
    assert_eq!(m.peek(io::SPRHIT), 3);
    let saved = m.save_state();
    assert_eq!(saved[4], 6, "version 6");
    let mut other = Machine::boot(RamSize::K64);
    other.load_state(&saved).unwrap();
    assert!(other.has_board());
    assert_eq!(other.save_state(), saved);
    for a in 0xC080..=0xC0BF {
        assert_eq!(other.peek(a), m.peek(a), "${a:04X}");
    }
    assert_eq!(picture(&other), picture(&m));
    other.load_state(&Machine::boot(RamSize::K16).save_state()).unwrap();
    assert!(!other.has_board());
    assert_eq!(saved.len(), Machine::new(RamSize::K16).save_state().len() + 15, "15 bytes of registers");
}

#[test]
fn version_1_and_2_states_still_load() {
    let now = Machine::boot(RamSize::K16).save_state();
    // the board's byte comes after the I/O, at 90 with no key held, then the sound chip's
    // byte and its 121
    assert_eq!((now[4], now[90], now[91]), (6, 0, 1));
    // version 5: no NIECE's byte (after the sound chip's 122); it loads without one
    let mut v5 = now.clone();
    assert_eq!(v5.remove(91 + 122), 0);
    v5[4] = 5;
    let mut m = Machine::boot_with_board(RamSize::K64, true);
    m.set_niece(true);
    m.load_state(&v5).unwrap();
    assert!(!m.has_niece(), "version 5: no NIECE");
    assert_eq!(m.save_state(), now);
    // version 4: no TEXTROWS (byte 36, after the bitmap's page); it loads with four rows
    let mut v4 = v5.clone();
    assert_eq!(v4.remove(36), 4);
    v4[4] = 4;
    let mut m = Machine::boot_with_board(RamSize::K64, true);
    m.load_state(&v4).unwrap();
    assert_eq!(m.save_state(), now);
    // version 3: no sound chip either
    let mut v3 = v4.clone();
    v3.drain(90..90 + 122);
    v3[4] = 3;
    let mut v1 = v3.clone();
    v1.remove(89);
    v1[4] = 1;
    assert_eq!(v1.len(), 16_793, "version 1's size");
    let mut m = Machine::boot_with_board(RamSize::K64, true);
    m.load_state(&v1).unwrap();
    assert!(!m.has_board(), "version 1: no board");
    assert_eq!(m.save_state(), now);
    m.load_state(&v3).unwrap();
    assert_eq!(m.save_state(), now);

    // version 2: the first board, eight sprites in 37 bytes of registers
    let mut old = [0u8; 37];
    old[..8].copy_from_slice(&[10; 8]); // SPRX
    old[25] = 0xFF; // SPREN
    old[26] = 0x30; // SPRBANK
    old[29] = 3; // SPRHIT
    old[31..37].copy_from_slice(&[2, 3, 1, 0x34, 4, 5]); // scroll, layer, its page and scroll
    let mut v2 = v3.clone();
    v2[4] = 2;
    v2[89] = 1;
    v2.splice(90..90, old);
    m.load_state(&v2).unwrap();
    assert!(m.has_board());
    let regs = [io::SPRTAB, io::SPRBANK, io::SPRHIT, io::SCROLLX, io::SCROLLY, io::LAYER, io::LAYERPAGE, io::LAYERSX, io::LAYERSY, io::COLOUR2];
    assert_eq!(regs.map(|a| m.peek(a)), [0x37, 0x30, 0, 2, 3, 1, 0x34, 4, 5, 15], "the sprites are dropped, the rest kept");
}

fn type_line(m: &mut Machine, s: &str) {
    assert!(s.len() <= 80, "{s}");
    for k in s.bytes().chain([13]) {
        m.key(k);
        for _ in 0..400 {
            m.run_cycles(500);
            if m.peek(io::KBD) & 0x80 == 0 {
                break;
            }
        }
        m.key_up(k);
    }
}

/// The flock the game's autoplay types: eight birds sharing one 2-bit image, POKEd from
/// BASIC. A V in colour 2 (yellow) with a colour 3 (white) eye.
const BIRDS: [&str; 7] = [
    "10 FOR I=14080 TO 14399:POKE I,0:NEXT:B=128:C=2",
    "20 FOR R=0 TO 7:A=14336+R*4:POKE A+R/4,B:POKE A+3-R/4,C",
    "30 B=B/4:C=C*4:IF B=0 THEN B=128:C=2",
    "40 NEXT:POKE 14366,192:POKE 49330,7",
    "50 FOR N=0 TO 7:A=14080+N*4:POKE A,150-N*12:POKE A+1,20+N*32",
    "60 POKE A+2,0:POKE A+3,128:NEXT",
    "RUN",
];

#[test]
fn eight_birds_from_basic() {
    let mut m = Machine::boot_with_board(RamSize::K16, true);
    for l in BIRDS {
        type_line(&mut m, l);
    }
    for _ in 0..200 {
        m.run_frame();
    }
    assert!(m.screen_text().trim_end().ends_with("READY.\n_"), "{}", m.screen_text());
    assert_eq!(m.colour_of(2), 7, "yellow birds");
    assert_eq!(m.colour_of(3), 1, "white eyes");
    m.load(0x0400, &[0x20; 1000]); // the screen out of the way
    let px = picture(&m);
    for n in 0..8usize {
        let (x, y) = (20 + n * 32, 150 - n * 12);
        for r in 0..8 {
            let mut want = [0u8; 16];
            want[r] = 2;
            want[15 - r] = 2;
            if r == 7 {
                want[8] = 3; // the eye
            }
            assert_eq!(line(&px, x, y + r, 16), want, "bird {n}, row {r}");
        }
        assert!(line(&px, x, y + 8, 16).iter().all(|&p| p == 0));
    }
    assert_eq!(lit(&px), 8 * 16);
    assert_eq!(m.peek(io::SPRLINE), 0);
}

// ---------- the beam (manual chapter 9): each line drawn as the raster finishes it ----------

/// Runs to the start of raster line `line` of the next frame.
fn to_line(m: &mut Machine, line: u32) {
    use unc_core::machine::CYCLES_PER_LINE;
    m.run_frame();
    m.run_cycles(line * CYCLES_PER_LINE);
}

#[test]
fn a_raster_split_changes_the_colours_from_the_line_it_happens_on() {
    let mut m = board();
    m.poke(io::BACKGROUND, 6);
    m.poke(io::COLOUR1 + 2, 1); // COLOUR3, the screen's ink
    m.run_frame();
    // half way down the picture (raster line 151 is picture line 100): new colours
    to_line(&mut m, 151);
    m.poke(io::BACKGROUND, 2);
    m.poke(io::COLOUR1 + 2, 7);
    m.run_frame();
    assert_eq!(m.line_colours(99), [6, 11, 15, 1], "above the split: the old background and ink");
    assert_eq!(m.line_colours(100), [2, 11, 15, 7], "from the split down: the new ones");
    assert_eq!(m.line_colours(199), [2, 11, 15, 7]);
    // the host's live view knows only now
    assert_eq!(m.colour_of(0), 2);
}

#[test]
fn a_sprite_moved_after_the_beam_passed_it_shows_twice() {
    let mut m = board();
    image(&mut m, 0, &solid(2));
    sprite(&mut m, 0, 100, 10, 0, 0);
    m.run_frame();
    // the beam has drawn picture lines 0-39 (raster 51-90): move the sprite below them
    to_line(&mut m, 91);
    sprite(&mut m, 0, 100, 120, 0, 0);
    m.run_frame();
    let f = m.frame().to_vec();
    assert_eq!(line(&f, 100, 10, 16), vec![2; 16], "drawn at y 10 before it moved");
    assert_eq!(line(&f, 100, 120, 16), vec![2; 16], "and at y 120 after: one entry, used twice");
    assert_eq!(line(&picture(&m), 100, 10, 16), vec![0; 16], "the live picture has it only where it is now");
}

#[test]
fn text_changed_mid_frame_tears_where_the_beam_was() {
    let mut m = board();
    for c in 0..1000u16 {
        m.poke(0x0400 + c, 0x80); // reverse blank: a solid cell of ink
    }
    m.run_frame();
    to_line(&mut m, 151); // the beam at picture line 100: rows 0-12 drawn
    for c in 0..1000u16 {
        m.poke(0x0400 + c, 0x20);
    }
    m.run_frame();
    let f = m.frame().to_vec();
    assert_eq!(lit(&line(&f, 0, 0, W)), W, "the top of the frame: the old, solid screen");
    assert_eq!(lit(&line(&f, 0, 99, W)), W);
    assert_eq!(lit(&line(&f, 0, 100, W)), 0, "from the beam down: the new, blank one");
    // a frame later, all of it is the new screen
    m.run_frame();
    assert_eq!(lit(m.frame()), 0);
}

#[test]
fn collisions_latch_as_each_line_is_drawn() {
    let mut m = board();
    image(&mut m, 0, &solid(1));
    sprite(&mut m, 0, 50, 20, 0, 0);
    sprite(&mut m, 1, 58, 20, 0, 0);
    to_line(&mut m, 51 + 25); // lines 0-24 drawn: they overlap from line 20
    assert_eq!(m.peek(io::SPRHIT) & 3, 3, "already set, before the frame ends");
}

/// SPRTAB 0 is the sprites' off switch: there is no table, so nothing shows and nothing
/// hits, whatever is in the fast page. Any other page and they're back.
#[test]
fn sprtab_0_switches_the_sprites_off() {
    let mut m = board();
    image(&mut m, 1, &solid(2));
    sprite(&mut m, 0, 100, 50, 1, 0);
    sprite(&mut m, 1, 108, 50, 1, 0);
    assert_eq!(lit(&picture(&m)), 16 * 24);
    m.poke(io::SPRTAB, 0);
    // a fast page that would be a table of shown sprites, if it were read as one
    for n in 0..32u16 {
        m.load(n * 4, &[50, 100, 1, attr::SHOWN]);
    }
    assert_eq!(lit(&picture(&m)), 0, "no sprites");
    m.run_frame();
    m.run_frame();
    assert_eq!((cpu_read(&mut m, io::SPRHIT), cpu_read(&mut m, io::SPRLINE)), (0, 0), "and nothing they could touch");
    m.poke(io::SPRTAB, 0x37);
    assert_eq!(lit(&picture(&m)), 16 * 24, "the table at $3700 again");
}

/// STOP and RESTORE on a machine with the board: the sprites off, the layer, the scroll and
/// the colour mode off, text white, so READY. can be read whatever the program had set.
#[test]
fn stop_and_restore_clear_the_boards_picture() {
    let mut m = Machine::boot_with_board(RamSize::K16, true);
    image(&mut m, 1, &solid(2));
    sprite(&mut m, 0, 100, 50, 1, 0);
    for (a, v) in [(io::LAYER, 1), (io::SCROLLX, 5), (io::SCROLLY, 3), (io::COLOURMODE, 1), (io::COLOUR3, 0), (io::BORDER, 4)] {
        m.poke(a, v);
    }
    m.key_matrix(3, true);
    m.restore();
    for _ in 0..3 {
        m.run_frame();
    }
    m.key_matrix(3, false);
    for a in [io::SPRTAB, io::LAYER, io::SCROLLX, io::SCROLLY, io::COLOURMODE, io::BORDER] {
        assert_eq!(m.peek(a), 0, "${a:04X}");
    }
    assert_eq!(m.peek(io::COLOUR3), 1, "text is white again");
    assert!(m.screen_text().starts_with("READY."), "{}", m.screen_text());
    let px = picture(&m);
    assert!(px.iter().all(|&p| p == 0 || p == 3), "only READY. and the cursor, in the text's colour");
    assert_eq!(m.peek(TABLE + 3) & attr::SHOWN, attr::SHOWN, "the table in memory isn't touched");
}
