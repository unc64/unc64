//! Plays a list of register writes on sound chips and writes what they put out.
//!
//!   cargo run --release -p unc-core --example chip_render -- WRITES OUT.raw
//!
//! WRITES is text, a line a write, in order of time: `cycle chip register value` (decimal;
//! register 0-63, the address less $C0C0), and a last line `end cycle`. OUT.raw is 16-bit
//! little-endian samples at 62,500 Hz, one channel a chip. scripts/music.py uses it to
//! record the game's music on two chips.

use std::io::Write;
use unc_core::chip::Chip;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let [_, writes, out] = &args[..] else {
        return Err("usage: chip_render WRITES OUT.raw".into());
    };
    let text = std::fs::read_to_string(writes)?;
    let count = text.lines().filter(|l| !l.starts_with("end")).map(|l| l.split(' ').nth(1).unwrap_or("0").parse::<usize>().unwrap_or(0)).max().unwrap_or(0) + 1;
    let mut chips = vec![Chip::default(); count];
    let mut now = 0u64;
    for line in text.lines() {
        let mut f = line.split(' ');
        let first = f.next().ok_or("empty line")?;
        let cycle: u64 = if first == "end" { f.next().ok_or("end when?")?.parse()? } else { first.parse()? };
        if cycle < now {
            return Err(format!("out of order: {line}").into());
        }
        for c in &mut chips {
            c.clock(cycle - now);
        }
        now = cycle;
        if first == "end" {
            break;
        }
        let mut n = || -> Result<usize, Box<dyn std::error::Error>> { Ok(f.next().ok_or("short line")?.parse()?) };
        let (chip, reg, value) = (n()?, n()?, n()?);
        chips[chip].store(reg as u8, value as u8);
    }
    let len = chips.iter().map(|c| c.samples.len()).min().unwrap_or(0);
    let mut bytes = Vec::with_capacity(len * count * 2);
    for i in 0..len {
        for c in &chips {
            bytes.extend_from_slice(&c.samples[i].to_le_bytes());
        }
    }
    std::fs::File::create(out)?.write_all(&bytes)?;
    Ok(())
}
