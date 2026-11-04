//! The five games under the beam (manual chapter 9): each line of the picture is drawn as the
//! raster finishes it, so a game that changes what's on the screen while the beam is drawing
//! it would show a torn frame. Here every game is played for a while, with keys going down
//! and up, and every frame the beam drew is compared with the picture as it stands when the
//! beam has just finished it (line 251, as FRAME goes up): if the game drew nothing while
//! the beam was drawing, they're the same.
//!
//! Every game but ORBIT waits for FRAME and does its work in the blank: 112 lines, 7,168
//! cycles, before the beam is back in the picture. ORBIT, in mixed mode, starts drawing at
//! line 219, where its arena ends (144 lines), works out the next frame while the beam draws
//! the arena, and redraws a rock at a time, only where the beam won't catch it (`beamok`),
//! twice a frame: before its work, and after it, ahead of the beam or behind it
//! (`rockslate`); so the beam may show the picture as it stood between any two rocks.
//! SNAKE and DEBUG always finish in time. A frame whose drawing runs on into the beam can
//! still show a line half drawn, for that one frame, as on a real machine: STACK when a
//! piece locks and the well is redrawn, AURORA on its heaviest frames (a coarse step in the
//! gorge, everything flying), ORBIT as a game is set up (the screen cleared, the scenery and
//! the HUD drawn: a few frames each time, five for a game over and a new game). The limits
//! below hold those where they are (measured 2026-11-03, ORBIT 2026-11-04), so a change
//! that makes a game tear more fails here. The first frames, the title being drawn, don't
//! count.

use unc_core::machine::{CYCLES_PER_FRAME, CYCLES_PER_LINE, PIXELS_H, PIXELS_W, VBLANK_LINE};
use unc_core::{Machine, RamSize, Stop};

fn game(src: &str, board: bool) -> (Machine, Vec<u16>) {
    let asm = unc_asm::assemble(src).unwrap_or_else(|e| panic!("doesn't assemble: {e:?}"));
    let mut m = Machine::boot_with_board(RamSize::K16, board);
    unc_asm::load(&mut m, &asm);
    // ORBIT redraws its rocks one at a time, twice a frame: before its work, and after it,
    // ahead of the beam or behind it (rocks, rockslate, rock1). Between two of them the
    // picture is whole, and the beam can show any of those pictures: they're watched for
    // at the start of each pass and after each rock (the return from every JSR rock1).
    let mut watch = Vec::new();
    if let Some(r1) = asm.symbol("rock1") {
        let jsr = unc_core::isa::opcode_for(unc_core::Op::Jsr, unc_core::Mode::Abs).unwrap();
        let [lo, hi] = r1.to_le_bytes();
        for seg in &asm.segments {
            for (i, w) in seg.bytes.windows(3).enumerate() {
                if w == [jsr, lo, hi] {
                    watch.push(seg.start + i as u16 + 3);
                }
            }
        }
        assert!(!watch.is_empty(), "no JSR rock1");
        watch.extend([asm.symbol("rocks").unwrap(), asm.symbol("rockslate").unwrap()]);
    }
    for &a in &watch {
        m.set_breakpoint(a, true);
    }
    (m, watch)
}

/// Plays `frames` frames, pressing a key now and then (SPACE first, to start), and returns
/// the frames the beam drew with lines that are neither as the game left them last frame
/// nor as it leaves them this frame (each looked at when the game starts its work, on line
/// `sync`: VBLANK, 251, or ORBIT's 219): a line half drawn, or drawn while it was half
/// changed. A line still showing the last frame's picture, because the game changed it
/// after the beam had passed, is a frame late, not torn; so is one showing the picture as
/// it stood at any of the `watch` addresses (ORBIT's rocks, between two of them).
fn torn_frames(m: &mut Machine, frames: u32, keys: &[u8], sync: u32, watch: &[u16]) -> Vec<(u32, usize)> {
    let mut torn = Vec::new();
    let mut prev = vec![0u8; PIXELS_W * PIXELS_H];
    let mut now = vec![0u8; PIXELS_W * PIXELS_H];
    let mut mids: Vec<Vec<u8>> = Vec::new();
    for f in 0..frames {
        match f % 40 {
            0 => m.key(if f < 40 { b' ' } else { keys[(f / 40) as usize % keys.len()] }),
            12 => {
                for &k in keys.iter().chain([b' '].iter()) {
                    m.key_up(k);
                }
            }
            _ => {}
        }
        // to where the game starts drawing: the beam has drawn the part it draws in
        let at = sync * CYCLES_PER_LINE;
        let to = (at + CYCLES_PER_FRAME - m.mem.frame_cycle) % CYCLES_PER_FRAME;
        let end = m.cpu.cycles + if to == 0 { CYCLES_PER_FRAME } else { to } as u64;
        while m.cpu.cycles < end {
            match m.run_cycles((end - m.cpu.cycles) as u32) {
                Stop::Breakpoint(pc) if watch.contains(&pc) => {
                    let mut mid = vec![0u8; PIXELS_W * PIXELS_H];
                    m.render(m.blink_on, &mut mid);
                    mids.push(mid);
                }
                _ => break,
            }
        }
        m.render(m.blink_on, &mut now);
        let row = |p: &[u8], y: usize| p[y * PIXELS_W..][..PIXELS_W].to_vec();
        let ys: Vec<usize> = (0..PIXELS_H)
            .filter(|&y| {
                let drawn = row(m.frame(), y);
                drawn != row(&now, y) && drawn != row(&prev, y) && !mids.iter().any(|mid| drawn == row(mid, y))
            })
            .collect();
        mids.clear();
        if std::env::var("BEAMDBG").is_ok() && !ys.is_empty() {
            eprintln!("frame {f}: lines {:?}", ys);
        }
        if !ys.is_empty() && f > 0 {
            torn.push((f, ys.len()));
        }
        std::mem::swap(&mut prev, &mut now);
    }
    torn
}

/// Out of 1,500 frames of play, after the first 12, at most `limit` torn.
fn check(name: &str, src: &str, board: bool, keys: &[u8], limit: usize) {
    let (mut m, watch) = game(src, board);
    let sync = if name == "ORBIT" { 219 } else { VBLANK_LINE };
    let torn: Vec<_> = torn_frames(&mut m, 1500, keys, sync, &watch).into_iter().filter(|&(f, _)| f >= 12).collect();
    if std::env::var("BEAMCOUNT").is_ok() {
        eprintln!("COUNT {name} {} {:?}", torn.len(), torn);
    }
    assert!(torn.len() <= limit, "{name}: {} torn frames (at most {limit}): {:?}", torn.len(), &torn[..torn.len().min(40)]);
}

#[test]
fn snake_draws_out_of_the_beams_way() {
    check("SNAKE", include_str!("../../../games/snake.s"), false, &[28, 30, 29, 31], 0);
}

#[test]
fn stack_draws_out_of_the_beams_way() {
    check("STACK", include_str!("../../../games/stack.s"), false, &[28, 29, 30, 31, b' '], 10);
}

#[test]
fn orbit_draws_out_of_the_beams_way() {
    check("ORBIT", include_str!("../../../games/orbit.s"), false, &[28, 29, 30, b' '], 10);
}

#[test]
fn debug_draws_out_of_the_beams_way() {
    check("DEBUG", include_str!("../../../games/debug.s"), false, &[28, 29, b' '], 0);
}

#[test]
fn aurora_draws_out_of_the_beams_way() {
    check("AURORA", include_str!("../../../games/board/aurora.s"), true, &[30, 31, 29, 28, b' '], 20);
}
