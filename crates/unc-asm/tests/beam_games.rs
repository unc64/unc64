//! The five games under the beam (manual chapter 9): each line of the picture is drawn as the
//! raster finishes it, so a game that changes what's on the screen while the beam is drawing
//! it would show a torn frame. Here every game is played for a while, with keys going down
//! and up, and every frame the beam drew is compared with the picture as it stands when the
//! beam has just finished it (line 251, as FRAME goes up): if the game drew nothing while
//! the beam was drawing, they're the same.
//!
//! Every game but ORBIT waits for FRAME and does its work in the blank: 112 lines, 7,168
//! cycles, before the beam is back in the picture. ORBIT, in mixed mode, starts at line
//! 219, where its arena ends (144 lines), and redraws a rock only where the beam won't catch
//! it (`beamok`). SNAKE and DEBUG always finish in time. A frame whose drawing runs on into
//! the beam can still show a line half drawn, for that one frame, as on a real machine:
//! STACK when a piece locks and the well is redrawn, AURORA on its heaviest frames (a coarse
//! step in the gorge, everything flying), ORBIT when its ship, drawn as lines, is near the
//! top of the arena as the beam comes round. The limits below hold those where they are
//! (measured 2026-11-03), so a change that makes a game tear more fails here. The first
//! frames, the title being drawn, don't count.

use unc_core::machine::{CYCLES_PER_FRAME, CYCLES_PER_LINE, PIXELS_H, PIXELS_W, VBLANK_LINE};
use unc_core::{Machine, RamSize};

fn game(src: &str, board: bool) -> Machine {
    let asm = unc_asm::assemble(src).unwrap_or_else(|e| panic!("doesn't assemble: {e:?}"));
    let mut m = Machine::boot_with_board(RamSize::K16, board);
    unc_asm::load(&mut m, &asm);
    m
}

/// Plays `frames` frames, pressing a key now and then (SPACE first, to start), and returns
/// the frames the beam drew with lines that are neither as the game left them last frame
/// nor as it leaves them this frame (each looked at when the game starts its work, on line
/// `sync`: VBLANK, 251, or ORBIT's 219): a line half drawn, or drawn while it was half
/// changed. A line still showing the last frame's picture, because the game changed it
/// after the beam had passed, is a frame late, not torn.
fn torn_frames(m: &mut Machine, frames: u32, keys: &[u8], sync: u32) -> Vec<(u32, usize)> {
    let mut torn = Vec::new();
    let mut prev = vec![0u8; PIXELS_W * PIXELS_H];
    let mut now = vec![0u8; PIXELS_W * PIXELS_H];
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
        m.run_cycles(if to == 0 { CYCLES_PER_FRAME } else { to });
        m.render(m.blink_on, &mut now);
        let row = |p: &[u8], y: usize| p[y * PIXELS_W..][..PIXELS_W].to_vec();
        let ys: Vec<usize> = (0..PIXELS_H)
            .filter(|&y| {
                let drawn = row(m.frame(), y);
                drawn != row(&now, y) && drawn != row(&prev, y)
            })
            .collect();
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
    let mut m = game(src, board);
    let sync = if name == "ORBIT" { 219 } else { VBLANK_LINE };
    let torn: Vec<_> = torn_frames(&mut m, 1500, keys, sync).into_iter().filter(|&(f, _)| f >= 12).collect();
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
    check("ORBIT", include_str!("../../../games/orbit.s"), false, &[28, 29, 30, b' '], 90);
}

#[test]
fn debug_draws_out_of_the_beams_way() {
    check("DEBUG", include_str!("../../../games/debug.s"), false, &[28, 29, b' '], 0);
}

#[test]
fn aurora_draws_out_of_the_beams_way() {
    check("AURORA", include_str!("../../../games/board/aurora.s"), true, &[30, 31, 29, 28, b' '], 20);
}
