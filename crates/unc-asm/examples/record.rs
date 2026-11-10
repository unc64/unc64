//! Runs a program on a machine with the sound chip in and records what it plays.
//!
//!   cargo run --release -p unc-asm --example record -- SOURCE.s OUT.wav SECONDS [KEYS] [REGS.bin]
//!
//! KEYS are typed one a second from the first second on (`_` is SPACE, `~` RETURN). OUT.wav
//! is mono, 44.1 kHz, 16-bit: the beeper and the chip, as `Machine::audio_frame` gives
//! them. The last screen is printed.
//!
//! REGS.bin, if given, gets the AUNT's registers after every frame, 25 bytes a frame, laid
//! out as a C64's SID has them (three voices of FREQ lo, hi, pulse width lo, hi, control,
//! AD, SR; then the filter and volume), so scripts/sid-style.py can read a tune from here
//! and a tune from there the same way.

use unc_core::{Machine, RamSize};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        return Err("usage: record SOURCE.s OUT.wav SECONDS [KEYS]".into());
    }
    let src = std::fs::read_to_string(&args[1])?;
    let asm = unc_asm::assemble(&src).map_err(|e| format!("{e:?}"))?;
    let mut m = Machine::boot(RamSize::K16);
    m.set_sound(true);
    unc_asm::load(&mut m, &asm);
    let frames = (args[3].parse::<f64>()? * 50.08) as usize;
    let keys: Vec<u8> = args.get(4).map(|k| k.bytes().collect()).unwrap_or_default();
    let mut pcm: Vec<u8> = Vec::new();
    let mut regs: Vec<u8> = Vec::new();
    let (mut owed, mut down) = (0.0f64, None);
    for f in 0..frames {
        if let Some(k) = down.take() {
            m.key_up(k);
        }
        if f % 50 == 49
            && let Some(&k) = keys.get(f / 50)
        {
            let k = match k {
                b'_' => b' ',
                b'~' => 13,
                k => k,
            };
            m.key(k);
            down = Some(k);
        }
        m.run_frame();
        for v in 0..3u16 {
            let at = 0xC0C0 + v * 8;
            let (wave, width) = (m.peek(at + 3), m.peek(at + 2));
            regs.extend_from_slice(&[
                m.peek(at),
                m.peek(at + 1),
                width << 4,
                width >> 4,
                (wave & 15) << 4 | (m.peek(at + 4) & 1),
                m.peek(at + 5),
                m.peek(at + 6),
            ]);
        }
        let (cutoff, filter, volume) = (m.peek(0xC0D8), m.peek(0xC0D9), m.peek(0xC0DA));
        regs.extend_from_slice(&[0, cutoff, (filter & 0xF0) | (filter & 7), (volume & 0x70) | (volume & 15)]);
        owed += Machine::samples_per_frame(44_100);
        let mut out = vec![0.0f32; owed as usize];
        owed -= out.len() as f64;
        m.audio_frame(44_100, &mut out);
        for s in out {
            pcm.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
        }
    }
    let mut wav = Vec::with_capacity(pcm.len() + 44);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + pcm.len() as u32).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    for v in [16u32, 0x0001_0001, 44_100, 88_200, 0x0010_0002] {
        wav.extend_from_slice(&v.to_le_bytes());
    }
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
    wav.extend_from_slice(&pcm);
    std::fs::write(&args[2], wav)?;
    if let Some(path) = args.get(5) {
        std::fs::write(path, &regs)?;
    }
    println!("{}", m.screen_text_plain());
    Ok(())
}
