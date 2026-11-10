use unc_core::cpu::{Bus, CpuState};
use unc_core::isa::{Mode, Op, opcode_for};
use unc_core::machine::{CYCLES_PER_FRAME, CYCLES_PER_LINE, Machine, RamSize, SYSTEM_ROM, Stop, TAPE_BYTE_CYCLES, io};

fn op(o: Op, m: Mode) -> u8 {
    opcode_for(o, m).unwrap()
}

/// A machine with `code` at $0800 and the CPU pointed at it (no ROM boot).
fn with_program(code: &[u8]) -> Machine {
    let mut m = Machine::new(RamSize::K16);
    m.load(0x0800, code);
    m.cpu.pc = 0x0800;
    m
}

#[test]
fn ram_ends_at_16k_on_the_floor_machine() {
    let mut m = Machine::new(RamSize::K16);
    m.poke(0x3FFF, 0x12);
    m.poke(0x4000, 0x34);
    assert_eq!(m.peek(0x3FFF), 0x12);
    assert_eq!(m.peek(0x4000), 0xFF, "no RAM there: open bus reads $FF");
    assert_eq!(m.peek(0xBFFF), 0xFF);
}

#[test]
fn the_64k_machine_has_ram_up_to_bfff() {
    let mut m = Machine::new(RamSize::K64);
    m.poke(0x4000, 0x34);
    m.poke(0xBFFF, 0x56);
    assert_eq!((m.peek(0x4000), m.peek(0xBFFF)), (0x34, 0x56));
}

#[test]
fn rom_is_mapped_at_e000_and_read_only() {
    let mut m = Machine::new(RamSize::K16);
    assert_eq!(m.peek(0xE000), SYSTEM_ROM[0]);
    assert_eq!(m.peek(0xFFFF), SYSTEM_ROM[0x1FFF]);
    let before = m.peek(0xE000);
    m.poke(0xE000, before.wrapping_add(1));
    assert_eq!(m.peek(0xE000), before);
}

#[test]
fn reset_vector_points_into_rom() {
    let m = Machine::new(RamSize::K16);
    assert!(m.cpu.pc >= 0xE000, "reset PC ${:04X}", m.cpu.pc);
}

#[test]
fn border_and_background_keep_four_bits() {
    let mut m = Machine::new(RamSize::K16);
    m.poke(io::BORDER, 0xF2);
    m.poke(io::BACKGROUND, 0x37);
    assert_eq!((m.peek(io::BORDER), m.peek(io::BACKGROUND)), (0x02, 0x07));
}

#[test]
fn keyboard_strobe_clears_on_read_but_not_on_peek() {
    let mut m = Machine::new(RamSize::K16);
    m.key(b'A');
    assert_eq!(m.peek(io::KBD), b'A' | 0x80);
    assert_eq!(m.peek(io::KBD_STROBE), b'A' | 0x80, "peek has no side effects");
    m.mem.read(io::KBD_STROBE);
    assert_eq!(m.peek(io::KBD), b'A');
}

#[test]
fn the_reserved_range_answers() {
    let m = Machine::new(RamSize::K16);
    assert_eq!((m.peek(0xC3F0), m.peek(0xC3F1)), (19, 86));
    assert_eq!(m.peek(0xC3F2), 0);
    assert_eq!(m.peek(0xC070), 0xFF, "unmapped I/O reads $FF");
}

/// A write to the reserved range changes nothing in the machine, and the host hears of it.
#[test]
fn the_host_hears_writes_to_the_reserved_range() {
    let mut m = Machine::new(RamSize::K16);
    assert_eq!(m.observe().reserved_write, None);
    m.mem.write(0xC3FF, 3);
    m.mem.write(0xC3F4, 9);
    assert_eq!(m.peek(0xC3FF), 0, "nothing changed");
    assert_eq!(m.observe().reserved_write, Some((4, 9)), "the last one");
    assert_eq!(m.observe().reserved_write, None, "and once");
    m.poke(0xC3FF, 3);
    assert_eq!(m.observe().reserved_write, None, "the host's own pokes don't count");
}

/// The host can make the reserved range answer more (part one, chapter 8): kept by a reset,
/// left out of save states, and the processor can't write it.
#[test]
fn the_host_can_change_what_the_reserved_range_answers() {
    let mut m = with_program(&[op(Op::Lda, Mode::Imm), 9, op(Op::Sta, Mode::Abs), 0xF2, 0xC3, op(Op::Hlt, Mode::Imp)]);
    let saved = m.save_state();
    m.set_reserved(2, 4);
    m.set_reserved(3, 7);
    assert_eq!((0..4).map(|i| m.peek(0xC3F0 + i)).collect::<Vec<_>>(), [19, 86, 4, 7]);
    m.run_cycles(100);
    assert_eq!(m.peek(0xC3F2), 4, "a program's write doesn't change it");
    m.reset();
    assert_eq!(m.peek(0xC3F3), 7, "a reset keeps it");
    m.load_state(&saved).unwrap();
    assert_eq!(m.peek(0xC3F3), 7, "a save state doesn't carry it either way");
    assert_eq!(Machine::new(RamSize::K16).peek(0xC3F2), 0, "a new machine answers 19, 86, 0");
}

#[test]
fn raster_advances_every_64_cycles_and_frames_wrap() {
    let mut m = with_program(&[op(Op::Jmp, Mode::Abs), 0x00, 0x08]);
    m.mem.frame_cycle = 0;
    m.run_cycles(CYCLES_PER_LINE * 10);
    assert_eq!(m.peek(io::RASTER), 10);
    let frame = m.peek(io::FRAME);
    m.run_cycles(CYCLES_PER_LINE * 240);
    assert_eq!(m.peek(io::RASTER), 250);
    assert_eq!(m.peek(io::FRAME), frame, "the beam is still in the picture");
    m.run_cycles(CYCLES_PER_LINE * 10);
    assert_eq!((m.mem.read(io::RASTER), m.peek(io::RASTER_HI)), (4, 1), "line 260");
    assert_eq!(m.peek(io::FRAME), frame.wrapping_add(1), "FRAME went up as the beam left the picture, at line 251");
    m.run_frame();
    assert_eq!(m.peek(io::FRAME), frame.wrapping_add(1), "and not again at the frame's end");
    assert!(m.mem.frame_cycle < 6, "stops at the frame boundary");
}

#[test]
fn raster_compare_and_vblank_raise_interrupts_when_enabled() {
    let mut m = with_program(&[op(Op::Cli, Mode::Imp), op(Op::Jmp, Mode::Abs), 0x01, 0x08]);
    m.load(0x3000, &[op(Op::Hlt, Mode::Imp)]);
    m.mem.rom[0x1FFE] = 0x00;
    m.mem.rom[0x1FFF] = 0x30;
    m.mem.frame_cycle = 0;
    m.poke(io::RASTER_CMP, 100);
    m.run_cycles(CYCLES_PER_LINE * 101);
    assert_eq!(m.cpu.state, CpuState::Running, "not enabled: no interrupt");
    assert_eq!(m.peek(io::IRQ_STATUS) & io::IRQ_RASTER, io::IRQ_RASTER);
    m.poke(io::IRQ_STATUS, 0xFF);
    assert_eq!(m.peek(io::IRQ_STATUS), 0, "write 1 to acknowledge");
    m.poke(io::IRQ_ENABLE, io::IRQ_VBLANK);
    let stop = m.run_cycles(CYCLES_PER_FRAME);
    assert_eq!(stop, Stop::Halted, "vblank IRQ ran the handler at $3000");
}

#[test]
fn border_colour_is_recorded_per_raster_line() {
    let mut code = vec![];
    for c in 1..=4u8 {
        code.extend([op(Op::Lda, Mode::Imm), c, op(Op::Sta, Mode::Abs), 0x20, 0xC0]);
        for _ in 0..30 {
            code.push(op(Op::Nop, Mode::Imp));
        }
    }
    code.push(op(Op::Hlt, Mode::Imp));
    let mut m = with_program(&code);
    m.mem.frame_cycle = 0;
    m.run_frame();
    assert_eq!(m.border_lines[0], 1);
    assert_eq!(m.border_lines[3], 4);
    let changes = m.border_lines[..5].windows(2).filter(|w| w[0] != w[1]).count();
    assert_eq!(changes, 3);
}

#[test]
fn breakpoints_stop_before_the_instruction_and_continue_works() {
    let mut m = with_program(&[op(Op::Inx, Mode::Imp), op(Op::Inx, Mode::Imp), op(Op::Inx, Mode::Imp), op(Op::Hlt, Mode::Imp)]);
    m.set_breakpoint(0x0802, true);
    assert_eq!(m.run_cycles(1000), Stop::Breakpoint(0x0802));
    assert_eq!(m.cpu.x, 2);
    assert_eq!(m.run_cycles(1000), Stop::Halted, "continuing steps past the breakpoint");
    assert_eq!(m.cpu.x, 3);
    m.clear_breakpoints();
    assert!(!m.breakpoint(0x0802));
}

#[test]
fn step_over_runs_a_whole_subroutine() {
    let mut m = with_program(&[op(Op::Jsr, Mode::Abs), 0x00, 0x30, op(Op::Hlt, Mode::Imp)]);
    m.load(0x3000, &[op(Op::Inx, Mode::Imp), op(Op::Inx, Mode::Imp), op(Op::Rts, Mode::Imp)]);
    m.step_over();
    assert_eq!((m.cpu.pc, m.cpu.x), (0x0803, 2));
    assert_eq!(m.step_over(), Stop::Halted);
}

#[test]
fn jams_are_reported_with_the_opcode() {
    let mut m = with_program(&[0xFF]);
    assert_eq!(m.run_frame(), Stop::Jammed(0xFF));
    assert_eq!(m.cpu.pc, 0x0800);
}

#[test]
fn reset_clears_ram_and_snapshots_replay_identically() {
    let prog = [op(Op::Inc, Mode::Abs), 0x00, 0x30, op(Op::Jmp, Mode::Abs), 0x00, 0x08];
    let mut m = with_program(&prog);
    m.run_cycles(5000);
    let snap = m.clone();
    m.run_cycles(20000);
    let mut replay = snap;
    replay.run_cycles(20000);
    assert_eq!(m.cpu, replay.cpu);
    assert_eq!(m.peek(0x3000), replay.peek(0x3000));
    m.reset();
    assert_eq!(m.peek(0x3000), 0);
}

// ---------- UNSCII and the video chip ----------

use unc_core::charset::CHAR_ROM;
use unc_core::machine::{CHAR_ROM_START, PIXELS_W};

fn glyph_rows(m: &Machine, cell: usize) -> Vec<u8> {
    let mut px = vec![0u8; PIXELS_W * 200];
    m.render(true, &mut px);
    let (cx, cy) = (cell % 40 * 8, cell / 40 * 8);
    (0..8).map(|y| (0..8).fold(0u8, |b, x| b << 1 | px[(cy + y) * PIXELS_W + cx + x])).collect()
}

#[test]
fn the_character_rom_is_readable_at_d000() {
    let m = Machine::new(RamSize::K16);
    for i in 0..1024u16 {
        assert_eq!(m.peek(CHAR_ROM_START + i), CHAR_ROM[i as usize]);
    }
    assert!(CHAR_ROM[0..8].iter().all(|&b| b == 0), "$00 is an empty cell");
    assert!(CHAR_ROM[0x0F * 8..0x10 * 8].iter().all(|&b| b == 0xFF), "$0F is a solid block");
    assert!(CHAR_ROM[0x20 * 8..0x21 * 8].iter().all(|&b| b == 0), "$20 is a space");
    assert!(CHAR_ROM[b'A' as usize * 8..][..8].iter().any(|&b| b != 0), "A has ink");
}

#[test]
fn the_character_rom_ignores_writes() {
    let mut m = Machine::new(RamSize::K16);
    let before = m.peek(0xD208);
    m.poke(0xD208, !before);
    assert_eq!(m.peek(0xD208), before);
}

#[test]
fn quarter_blocks_light_the_quarters_their_bits_name() {
    for q in 0..16usize {
        let g = &CHAR_ROM[q * 8..q * 8 + 8];
        let top = (if q & 1 != 0 { 0xF0 } else { 0 }) | (if q & 2 != 0 { 0x0F } else { 0 });
        let bottom = (if q & 4 != 0 { 0xF0 } else { 0 }) | (if q & 8 != 0 { 0x0F } else { 0 });
        assert!(g[..4].iter().all(|&b| b == top) && g[4..].iter().all(|&b| b == bottom), "quarters ${q:02X}");
    }
}

#[test]
fn charset_resets_to_the_rom_and_render_draws_its_glyphs() {
    let mut m = Machine::new(RamSize::K16);
    assert_eq!(m.peek(io::CHARSET), 0xD0);
    m.poke(0x0400, b'A');
    assert_eq!(glyph_rows(&m, 0), CHAR_ROM[b'A' as usize * 8..][..8]);
}

#[test]
fn codes_80_to_ff_are_reverse_video_and_a0_blinks() {
    let mut m = Machine::new(RamSize::K16);
    m.poke(0x0400, b'A' | 0x80);
    let want: Vec<u8> = CHAR_ROM[b'A' as usize * 8..][..8].iter().map(|b| !b).collect();
    assert_eq!(glyph_rows(&m, 0), want);
    m.poke(0x0401, 0xA0);
    assert_eq!(glyph_rows(&m, 1), vec![0xFF; 8], "cursor on");
    let mut px = vec![0u8; PIXELS_W * 200];
    m.render(false, &mut px);
    assert!((0..8).all(|y| px[y * PIXELS_W + 8..][..8].iter().all(|&p| p == 0)), "cursor off");
    assert!(px[..8].contains(&1), "other reverse cells don't blink");
}

#[test]
fn a_custom_character_set_in_ram_replaces_the_glyphs() {
    let mut m = Machine::new(RamSize::K16);
    let owl = [0x42, 0x7E, 0xDB, 0xFF, 0x66, 0x7E, 0x24, 0x00];
    m.load(0x3800 + b'A' as u16 * 8, &owl);
    m.poke(io::CHARSET, 0x38);
    m.poke(0x0400, b'A');
    m.poke(0x0401, b'A' | 0x80);
    assert_eq!(glyph_rows(&m, 0), owl);
    assert_eq!(glyph_rows(&m, 1), owl.iter().map(|b| !b).collect::<Vec<_>>());
}

#[test]
fn vmode_bit_0_gives_256_separate_glyphs() {
    let mut m = Machine::new(RamSize::K16);
    m.load(0x3000 + 0xC1 * 8, &[1, 2, 3, 4, 5, 6, 7, 8]);
    m.poke(io::CHARSET, 0x30);
    m.poke(io::VMODE, io::VMODE_256);
    m.poke(0x0400, 0xC1);
    m.poke(0x0401, 0xA0);
    m.load(0x3000 + 0xA0 * 8, &[0x81; 8]);
    assert_eq!(glyph_rows(&m, 0), vec![1, 2, 3, 4, 5, 6, 7, 8]);
    let mut px = vec![0u8; PIXELS_W * 200];
    m.render(false, &mut px);
    assert_eq!(px[8], 1, "$A0 is an ordinary glyph in 256-glyph mode: no blinking");
}

// ---------- the bitmap ----------

use unc_core::machine::{BITMAP_START, MIXED_SPLIT};

fn pixels(m: &Machine) -> Vec<u8> {
    let mut px = vec![0u8; PIXELS_W * 200];
    m.render(true, &mut px);
    px
}

#[test]
fn bitmap_registers_reset_to_text_mode_with_the_bitmap_at_2000() {
    let m = Machine::new(RamSize::K16);
    assert_eq!(m.peek(io::BITMAP), 0x20);
    assert_eq!(BITMAP_START, 0x2000);
    assert_eq!(m.peek(io::VMODE) & (io::VMODE_BITMAP | io::VMODE_MIXED), 0, "text mode after reset");
}

#[test]
fn a_pixel_is_one_bit_at_base_plus_y_times_40_plus_x_over_8() {
    let mut m = Machine::new(RamSize::K16);
    m.poke(io::VMODE, io::VMODE_BITMAP);
    for (x, y) in [(0usize, 0usize), (7, 0), (8, 0), (319, 0), (0, 1), (100, 57), (319, 199)] {
        for a in 0x2000..0x2000 + 8000u16 {
            m.poke(a, 0);
        }
        let addr = 0x2000 + (y * 40 + x / 8) as u16;
        m.poke(addr, 0x80 >> (x % 8));
        let px = pixels(&m);
        assert_eq!(px.iter().filter(|&&p| p == 1).count(), 1, "({x},{y})");
        assert_eq!(px[y * PIXELS_W + x], 1, "({x},{y})");
    }
}

#[test]
fn the_bitmap_can_move() {
    let mut m = Machine::new(RamSize::K16);
    m.poke(io::VMODE, io::VMODE_BITMAP);
    m.poke(io::BITMAP, 0x10);
    m.poke(0x1000, 0xFF);
    assert_eq!(&pixels(&m)[..8], &[1; 8]);
}

#[test]
fn mixed_mode_shows_text_rows_21_to_24_below_line_168() {
    let mut m = Machine::new(RamSize::K16);
    for a in 0x2000..0x2000 + 8000u16 {
        m.poke(a, 0xFF);
    }
    m.poke(0x0400 + 21 * 40, b'A');
    m.poke(0x0400 + 5 * 40, b'A');
    m.poke(io::VMODE, io::VMODE_BITMAP | io::VMODE_MIXED);
    let px = pixels(&m);
    assert_eq!(MIXED_SPLIT, 168);
    assert!(px[..168 * PIXELS_W].iter().all(|&p| p == 1), "the bitmap above, the text on row 5 hidden");
    let a: Vec<u8> = (0..8).map(|y| (0..8).fold(0u8, |b, x| b << 1 | px[(168 + y) * PIXELS_W + x])).collect();
    assert_eq!(a, CHAR_ROM[b'A' as usize * 8..][..8], "row 21 is text");
    assert!(px[180 * PIXELS_W + 100..][..8].iter().all(|&p| p == 0), "blank text cells are paper");
    m.poke(io::VMODE, io::VMODE_BITMAP);
    assert!(pixels(&m).iter().all(|&p| p == 1), "without mixed mode the bitmap fills the screen");
}

#[test]
fn bitmap_off_draws_text_exactly_as_before() {
    let mut m = Machine::new(RamSize::K16);
    m.poke(0x0400, b'A');
    let text = pixels(&m);
    for a in 0x2000..0x2000 + 8000u16 {
        m.poke(a, 0x55);
    }
    m.poke(io::VMODE, io::VMODE_MIXED);
    assert_eq!(pixels(&m), text, "mixed alone does nothing without the bitmap");
}

// ---------- Rev A additions: steady cursor, HELD, cursor keys, test helpers ----------

#[test]
fn vmode_bit_3_stops_the_cursor_blinking() {
    let mut m = Machine::new(RamSize::K16);
    m.poke(0x0400, 0xA0);
    m.poke(io::VMODE, io::VMODE_STEADY);
    let mut px = vec![0u8; PIXELS_W * 200];
    m.render(false, &mut px);
    assert!((0..8).all(|y| px[y * PIXELS_W..][..8].iter().all(|&p| p == 1)), "a steady reversed space");
    m.poke(io::VMODE, 0);
    m.render(false, &mut px);
    assert_eq!(px[0], 0, "blinking again");
    assert_eq!(Machine::new(RamSize::K16).peek(io::VMODE) & io::VMODE_STEADY, 0, "resets to blinking");
}

#[test]
fn held_reads_the_latest_key_still_down() {
    let mut m = Machine::new(RamSize::K16);
    assert_eq!(m.peek(io::HELD), 0);
    m.key(io::KEY_LEFT);
    assert_eq!(m.peek(io::HELD), 28);
    m.key(b'A');
    assert_eq!(m.peek(io::HELD), b'A');
    assert_eq!(m.peek(io::KBD), b'A' | 0x80, "KBD works as before");
    m.mem.read(io::KBD_STROBE);
    assert_eq!(m.peek(io::HELD), b'A', "taking the key doesn't let it go");
    m.key_up(b'A');
    assert_eq!(m.peek(io::HELD), 28, "back to the key still down");
    m.key_up(b'Z');
    assert_eq!(m.peek(io::HELD), 28, "letting go of a key that isn't down does nothing");
    m.key_up(io::KEY_LEFT);
    assert_eq!(m.peek(io::HELD), 0);
    m.poke(io::HELD, 5);
    assert_eq!(m.peek(io::HELD), 0, "read-only");
    assert_eq!((io::KEY_LEFT, io::KEY_RIGHT, io::KEY_UP, io::KEY_DOWN), (28, 29, 30, 31));
}

#[test]
fn reading_raster_latches_raster_hi() {
    let mut m = with_program(&[op(Op::Jmp, Mode::Abs), 0x00, 0x08]);
    m.mem.frame_cycle = 255 * CYCLES_PER_LINE + CYCLES_PER_LINE - 1; // the last cycle of line 255
    assert_eq!(m.mem.read(io::RASTER), 255);
    m.mem.frame_cycle += 1; // line 256 begins between the two reads
    assert_eq!(m.mem.read(io::RASTER_HI), 0, "HI belongs to the RASTER read: line 255, not 511 or 0");
    assert_eq!(m.mem.read(io::RASTER), 0);
    assert_eq!(m.mem.read(io::RASTER_HI), 1, "line 256");
    m.mem.frame_cycle = 10 * CYCLES_PER_LINE;
    assert_eq!(m.peek(io::RASTER_HI), 1, "still latched until RASTER is read");
    assert_eq!(m.peek(io::RASTER), 10, "peek has no side effects");
    assert_eq!(m.peek(io::RASTER_HI), 1);
    m.mem.read(io::RASTER);
    assert_eq!(m.peek(io::RASTER_HI), 0);
}

#[test]
fn reading_kbd_strobe_returns_bit_7_then_clears_it() {
    let mut m = Machine::new(RamSize::K16);
    m.key(b'Q');
    assert_eq!(m.mem.read(io::KBD_STROBE), b'Q' | 0x80);
    assert_eq!(m.mem.read(io::KBD_STROBE), b'Q');
}

#[test]
fn screen_text_plain_reads_reverse_video_as_letters() {
    let mut m = Machine::new(RamSize::K16);
    m.load(0x0400, &[0x20; 40]);
    m.load(0x0400, &[0x20, 0xC7, 0xC1, 0xCD, 0xC5, 0xA0, 0x8F, b'!']);
    assert_eq!(m.screen_text().lines().next(), Some(" ????_?!"));
    assert_eq!(m.screen_text_plain().lines().next(), Some(" GAME ?!"));
}

#[test]
fn run_until_stops_at_an_address_or_says_why_not() {
    let nop = op(Op::Nop, Mode::Imp);
    let mut m = with_program(&[nop, nop, op(Op::Hlt, Mode::Imp)]);
    assert_eq!(m.run_until(0x0800, 100), Ok(0), "already there");
    assert_eq!(m.run_until(0x0802, 100), Ok(4));
    assert_eq!(m.run_until(0x0900, 100), Err(Stop::Halted));
    let mut m = with_program(&[op(Op::Jmp, Mode::Abs), 0x00, 0x08]);
    assert_eq!(m.run_until(0x0900, 30), Err(Stop::Budget));
}

#[test]
fn boot_runs_the_rom_to_basic() {
    let m = Machine::boot(RamSize::K16);
    assert!(m.screen_text().contains("READY."));
}

#[test]
fn in_256_glyph_mode_the_rom_charset_has_no_upper_half() {
    let mut m = Machine::new(RamSize::K16);
    assert!((0xD400..0xD800).all(|a| m.peek(a) == 0xFF), "$D400-$D7FF reads $FF");
    m.poke(io::VMODE, io::VMODE_256);
    m.poke(0x0400, 0xC1);
    assert_eq!(glyph_rows(&m, 0), vec![0xFF; 8], "glyph $C1 from the ROM is a solid block");
}

// ---------- the tape deck (chapter 14) ----------

/// A machine idling in a loop at $0800, with a tape of `len` bytes (0, 1, 2, ...) in the deck.
fn with_tape(len: usize) -> (Machine, Vec<u8>) {
    let mut m = with_program(&[op(Op::Jmp, Mode::Abs), 0x00, 0x08]);
    let tape: Vec<u8> = (0..len).map(|i| (i * 7 + 3) as u8).collect();
    m.insert_tape(tape.clone());
    (m, tape)
}

fn motor(m: &mut Machine, on: bool) {
    m.poke(io::TAPE, if on { io::TAPE_MOTOR } else { 0 });
}

#[test]
fn the_tape_register_shows_the_deck() {
    let mut m = with_program(&[]);
    assert_eq!(m.peek(io::TAPE), 0, "no tape, nothing down");
    assert!(!m.press_play(), "PLAY doesn't go down without a tape");
    m.insert_tape(vec![1, 2, 3]);
    assert_eq!(m.peek(io::TAPE), io::TAPE_IN);
    assert!(m.press_play());
    assert_eq!(m.peek(io::TAPE), io::TAPE_IN | io::TAPE_PLAY);
    motor(&mut m, true);
    assert_eq!(m.peek(io::TAPE), io::TAPE_IN | io::TAPE_PLAY | io::TAPE_MOTOR);
    m.press_stop();
    assert_eq!(m.peek(io::TAPE), io::TAPE_IN | io::TAPE_MOTOR, "STOP lets PLAY up; the motor is the computer's");
}

#[test]
fn the_tape_delivers_twelve_bytes_a_frame() {
    let (mut m, _) = with_tape(2000);
    m.press_play();
    motor(&mut m, true);
    m.run_cycles(CYCLES_PER_FRAME * 50);
    assert_eq!(m.mem.deck.position, 600, "50 frames, one second: 600 bytes");
    assert_eq!(TAPE_BYTE_CYCLES * 12, CYCLES_PER_FRAME);
}

#[test]
fn nothing_flows_without_play_or_without_the_motor() {
    let (mut m, _) = with_tape(100);
    motor(&mut m, true);
    m.run_cycles(CYCLES_PER_FRAME * 5);
    assert_eq!(m.mem.deck.position, 0, "motor on, PLAY up");
    assert_eq!(m.peek(io::TAPE) & io::TAPE_READY, 0);
    motor(&mut m, false);
    m.press_play();
    m.run_cycles(CYCLES_PER_FRAME * 5);
    assert_eq!(m.mem.deck.position, 0, "PLAY down, motor off");
    motor(&mut m, true);
    m.run_cycles(TAPE_BYTE_CYCLES * 3);
    assert_eq!(m.mem.deck.position, 3, "both: it moves");
    motor(&mut m, false);
    m.run_cycles(CYCLES_PER_FRAME * 5);
    assert_eq!(m.mem.deck.position, 3, "the motor stops it where it is");
}

#[test]
fn reading_takes_each_byte_in_order_and_the_end_lets_play_up() {
    let (mut m, tape) = with_tape(40);
    m.press_play();
    motor(&mut m, true);
    let mut got = Vec::new();
    while got.len() < tape.len() {
        m.run_cycles(200);
        let status = m.mem.read(io::TAPE);
        if status & io::TAPE_READY != 0 {
            assert_eq!(status & io::TAPE_OVERRUN, 0);
            got.push(m.mem.read(io::TAPE_DATA));
            assert_eq!(m.peek(io::TAPE) & io::TAPE_READY, 0, "reading takes the byte");
        }
    }
    assert_eq!(got, tape);
    assert_ne!(m.peek(io::TAPE) & io::TAPE_PLAY, 0, "still playing just after the last byte");
    m.run_cycles(TAPE_BYTE_CYCLES + 10);
    assert_eq!(m.peek(io::TAPE) & io::TAPE_PLAY, 0, "the auto-stop lets PLAY up at the end");
    assert_eq!(m.mem.deck.position, 40);
    m.press_play();
    m.run_cycles(TAPE_BYTE_CYCLES + 10);
    assert_eq!(m.peek(io::TAPE) & io::TAPE_PLAY, 0, "PLAY at the end of the tape comes straight back up");
    m.rewind_tape();
    m.press_play();
    m.run_cycles(TAPE_BYTE_CYCLES + 10);
    assert_eq!((m.mem.deck.position, m.mem.read(io::TAPE_DATA)), (1, tape[0]), "rewound");
}

#[test]
fn bytes_not_taken_in_time_are_lost() {
    let (mut m, tape) = with_tape(10);
    m.press_play();
    motor(&mut m, true);
    m.run_cycles(TAPE_BYTE_CYCLES * 3 + 10);
    assert_eq!(m.peek(io::TAPE) & (io::TAPE_READY | io::TAPE_OVERRUN), io::TAPE_READY | io::TAPE_OVERRUN);
    assert_eq!(m.mem.read(io::TAPE_DATA), tape[2], "the latest byte; the two before it are gone");
    assert_eq!(m.peek(io::TAPE) & (io::TAPE_READY | io::TAPE_OVERRUN), 0, "reading clears both flags");
    m.run_cycles(TAPE_BYTE_CYCLES);
    assert_eq!(m.mem.read(io::TAPE_DATA), tape[3]);
}

#[test]
fn eject_takes_the_tape_out_and_reset_leaves_it_in() {
    let (mut m, tape) = with_tape(100);
    m.press_play();
    motor(&mut m, true);
    m.run_cycles(TAPE_BYTE_CYCLES * 5 + 10);
    m.reset();
    assert_eq!(m.mem.deck.position, 5, "the counter survives a reset");
    assert_eq!(m.peek(io::TAPE), io::TAPE_IN | io::TAPE_PLAY, "the motor stops; the tape and PLAY stay");
    assert_eq!(m.eject_tape(), Some(tape));
    assert_eq!(m.peek(io::TAPE), 0, "no tape, PLAY up");
    motor(&mut m, true);
    m.run_cycles(CYCLES_PER_FRAME);
    assert_eq!(m.peek(io::TAPE) & io::TAPE_READY, 0, "nothing flows");
    assert_eq!(m.eject_tape(), None);
}

// ---------- recording (chapter 14, Rev A) ----------

#[test]
fn record_goes_down_with_play_and_shows_room_for_a_byte() {
    let mut m = with_program(&[op(Op::Jmp, Mode::Abs), 0x00, 0x08]);
    assert!(!m.press_record(), "RECORD doesn't go down without a tape");
    assert_eq!(m.peek(io::TAPE), 0);
    m.insert_blank_tape();
    assert_eq!(m.peek(io::TAPE), io::TAPE_IN, "a blank tape is in");
    assert!(m.press_record());
    assert_eq!(m.peek(io::TAPE), io::TAPE_IN | io::TAPE_PLAY | io::TAPE_RECORD | io::TAPE_ROOM);
    m.poke(io::TAPE_DATA, 0x55);
    assert_eq!(m.peek(io::TAPE) & io::TAPE_ROOM, 0, "the byte waits for the tape");
    m.press_stop();
    assert_eq!(m.peek(io::TAPE), io::TAPE_IN, "STOP lets both up; the waiting byte is dropped");
    assert_eq!(m.mem.deck.tape.as_deref(), Some(&[][..]), "nothing recorded");
    m.press_play();
    m.poke(io::TAPE_DATA, 0x66);
    assert_eq!(m.peek(io::TAPE), io::TAPE_IN | io::TAPE_PLAY, "writing TAPEDATA without RECORD does nothing");
}

#[test]
fn recording_puts_a_byte_on_the_tape_every_1664_cycles() {
    let mut m = with_program(&[op(Op::Jmp, Mode::Abs), 0x00, 0x08]);
    m.insert_blank_tape();
    m.press_record();
    m.poke(io::TAPE_DATA, 0xA1);
    m.run_cycles(CYCLES_PER_FRAME);
    assert_eq!(m.mem.deck.position, 0, "nothing moves until the motor runs");
    motor(&mut m, true);
    m.run_cycles(TAPE_BYTE_CYCLES - 20);
    assert_eq!(m.peek(io::TAPE) & io::TAPE_ROOM, 0, "not yet");
    m.run_cycles(30);
    assert_ne!(m.peek(io::TAPE) & io::TAPE_ROOM, 0, "recorded: room for the next");
    assert_eq!(m.mem.deck.tape.as_deref(), Some(&[0xA1][..]));
    assert_eq!(m.peek(io::TAPE) & (io::TAPE_READY | io::TAPE_OVERRUN), 0, "nothing comes off the tape while recording");
    m.run_cycles(TAPE_BYTE_CYCLES * 3);
    assert_eq!(m.mem.deck.position, 1, "byte times with nothing to record leave nothing");
    for b in [1u8, 2, 3] {
        m.poke(io::TAPE_DATA, b);
        m.run_cycles(TAPE_BYTE_CYCLES);
    }
    assert_eq!(m.mem.deck.tape.as_deref(), Some(&[0xA1, 1, 2, 3][..]));
    assert_eq!(m.peek(io::TAPE) & io::TAPE_PLAY, io::TAPE_PLAY, "no auto-stop at the end: the tape grows");
    assert_eq!(m.eject_tape(), Some(vec![0xA1, 1, 2, 3]), "the host gets the recording back");
}

#[test]
fn recording_goes_over_what_was_there_and_turbo_records_at_once() {
    let mut m = with_program(&[op(Op::Jmp, Mode::Abs), 0x00, 0x08]);
    m.insert_tape(vec![9; 6]);
    m.press_play();
    motor(&mut m, true);
    m.run_cycles(TAPE_BYTE_CYCLES * 2 + 10);
    assert_eq!(m.mem.deck.position, 2);
    m.press_record(); // at the counter's place
    m.set_tape_turbo(true);
    m.poke(io::TAPE_DATA, 7);
    m.step();
    assert_eq!(m.mem.deck.position, 3, "turbo: recorded as soon as written");
    for b in [6u8, 5, 4, 3, 2] {
        assert_ne!(m.peek(io::TAPE) & io::TAPE_ROOM, 0);
        m.poke(io::TAPE_DATA, b);
        m.step();
    }
    assert_eq!(m.mem.deck.tape.as_deref(), Some(&[9, 9, 7, 6, 5, 4, 3, 2][..]), "over the old bytes, then on the end");
}

#[test]
fn the_clock_keeps_running_while_the_processor_is_halted() {
    // HLT at once: the run stops there, at the cycles it took
    let mut m = with_program(&[op(Op::Hlt, Mode::Imp)]);
    assert_eq!(m.run_frame(), Stop::Halted);
    let at_halt = m.cpu.cycles;
    let frame = m.peek(io::FRAME);
    // the next frames: nothing runs, but the raster and FRAME go on
    for _ in 0..3 {
        assert_eq!(m.run_frame(), Stop::Halted);
    }
    assert_eq!(m.cpu.cycles, at_halt, "the processor's own count waits: the cycles to the HLT stay countable");
    assert!(m.idle_cycles > 2 * CYCLES_PER_FRAME as u64, "the clock ran on");
    assert_eq!(m.peek(io::FRAME), frame.wrapping_add(3));
    assert_eq!(m.cpu.state, CpuState::Halted);
    // a jammed processor too
    let mut j = with_program(&[0xFF]);
    assert!(matches!(j.run_frame(), Stop::Jammed(0xFF)));
    let f = j.peek(io::FRAME);
    j.run_cycles(CYCLES_PER_LINE * 10);
    j.run_frame();
    assert_eq!(j.peek(io::FRAME), f.wrapping_add(1));
}
