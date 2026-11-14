//! A development tool: assembles a program, runs it on a booted machine and writes the
//! picture as a PPM, colours as the colour monitor shows them (manual Appendix A).
//!
//! `cargo run --release -p unc-asm --example shot -- SOURCE OUT.ppm [--frames N] [--ram 16|64]
//! [--board] [--niece] [--press FRAME:CODE:FRAMES]... [--every N]`
//!
//! `--press 50:32:5` holds the key with code 32 (space) down from frame 50 for 5 frames.
//! `--every N` also writes OUT-0000.ppm, OUT-000N.ppm ... as it goes.

use unc_core::machine::{FIRST_PICTURE_LINE, NIECE_PIXEL, PIXELS_H, PIXELS_W};
use unc_core::{Machine, RamSize};

const PALETTE: [u32; 16] = [
    0x000000, 0xe8e4da, 0x9a3b2e, 0x5fb3a8, 0x7a3f8a, 0x4f9a4f, 0x2c3d9a, 0xe2c84a, 0xa8632a, 0x5a3e1a, 0xd0705f, 0x3a3a3a, 0x6b6b6b, 0x9fe08a,
    0x6f8fd8, 0xa9a9a9,
];

/// The picture with a border of 16 pixels, as P6.
pub fn ppm(m: &Machine) -> Vec<u8> {
    const B: usize = 16;
    let (w, h) = (PIXELS_W + 2 * B, PIXELS_H + 2 * B);
    let mut out = format!("P6 {w} {h} 255\n").into_bytes();
    let f = m.frame();
    for y in 0..h {
        let line = (y + FIRST_PICTURE_LINE).wrapping_sub(B);
        let border = PALETTE[(m.border_lines[line.min(311)] & 15) as usize];
        for x in 0..w {
            let rgb = if (B..B + PIXELS_W).contains(&x) && (B..B + PIXELS_H).contains(&y) {
                let p = f[(y - B) * PIXELS_W + x - B];
                let c = if p & NIECE_PIXEL != 0 { p & 15 } else { m.line_colours(y - B)[(p & 3) as usize] };
                PALETTE[(c & 15) as usize]
            } else {
                border
            };
            out.extend_from_slice(&rgb.to_be_bytes()[1..]);
        }
    }
    out
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (mut files, mut frames, mut ram, mut board, mut niece, mut every) = (Vec::new(), 100u32, RamSize::K16, false, false, 0u32);
    let mut presses: Vec<(u32, u8, u32)> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let next = args.get(i + 1).cloned().unwrap_or_default();
        match args[i].as_str() {
            "--frames" => { frames = next.parse().expect("--frames N"); i += 1 }
            "--every" => { every = next.parse().expect("--every N"); i += 1 }
            "--ram" => { ram = if next == "64" { RamSize::K64 } else { RamSize::K16 }; i += 1 }
            "--board" => board = true,
            "--niece" => niece = true,
            "--press" => {
                let p: Vec<u32> = next.split(':').map(|v| v.parse().expect("--press FRAME:CODE:FRAMES")).collect();
                presses.push((p[0], p[1] as u8, p[2]));
                i += 1
            }
            s => files.push(s.to_string()),
        }
        i += 1;
    }
    let [src, out] = &files[..] else {
        eprintln!("usage: shot SOURCE OUT.ppm [--frames N] [--ram 16|64] [--board] [--niece] [--press FRAME:CODE:FRAMES]... [--every N]");
        std::process::exit(2);
    };
    let text = std::fs::read_to_string(src).expect("the source");
    let asm = unc_asm::assemble(&text).unwrap_or_else(|e| {
        for e in e {
            eprintln!("{src}:{}: {}", e.line, e.message);
        }
        std::process::exit(1);
    });
    let mut m = Machine::boot_with_board(ram, board);
    m.set_niece(niece);
    unc_asm::load(&mut m, &asm);
    for f in 0..frames {
        for &(at, code, len) in &presses {
            if f == at {
                m.key(code);
            }
            if f == at + len {
                m.key_up(code);
            }
        }
        let stop = m.run_frame();
        if every != 0 && f % every == 0 {
            std::fs::write(out.replace(".ppm", &format!("-{f:04}.ppm")), ppm(&m)).expect("writing");
        }
        if matches!(stop, unc_core::Stop::Jammed(_)) {
            eprintln!("jammed at ${:04X} in frame {f}", m.cpu.pc);
            break;
        }
    }
    std::fs::write(out, ppm(&m)).expect("writing");
}
