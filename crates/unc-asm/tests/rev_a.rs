//! System Rev A, pinned (REVISIONS.md): a program that runs on a Rev A machine runs the same,
//! cycle for cycle, on every later one. These are the tables that promise rests on: the
//! instruction set and its cycles (the documented ones and the eight the book leaves out),
//! the timing of a frame, the ROM's jump table, UNC BASIC's tokens, the tape format and the
//! AUNT's tables. Each
//! is compared with `rev_a.txt`, written when Rev A was frozen. If this test fails, the change
//! is a Rev B change: put it back, or make it a later revision that only adds.

use unc_core::isa::{OPCODES, UNDOCUMENTED};
use unc_core::machine::{self, CYCLES_PER_FRAME, CYCLES_PER_LINE, LINES_PER_FRAME, Machine, RamSize, VBLANK_LINE};

fn rev_a() -> String {
    let mut out = String::new();
    out.push_str("# The instruction set: opcode, operation, mode, cycles\n");
    for (code, info) in OPCODES.iter().enumerate() {
        if let Some(i) = info {
            out.push_str(&format!("${code:02X} {} {:?} {}\n", i.op.mnemonic(), i.mode, i.cycles));
        }
    }
    for (code, op, mode, cycles) in UNDOCUMENTED {
        out.push_str(&format!("${code:02X} {op:?} {mode:?} {cycles} (undocumented)\n"));
    }
    out.push_str("# Timing\n");
    out.push_str(&format!(
        "{CYCLES_PER_LINE} cycles a line, {LINES_PER_FRAME} lines, {CYCLES_PER_FRAME} cycles a frame, picture from line {}, VBLANK at line {VBLANK_LINE}\n",
        machine::FIRST_PICTURE_LINE
    ));
    out.push_str("# The ROM's jump table: each entry a JMP\n");
    let m = Machine::new(RamSize::K16);
    let jmp = unc_core::isa::opcode_for(unc_core::isa::Op::Jmp, unc_core::isa::Mode::Abs).unwrap();
    for a in (0xFF00u16..=0xFF21).step_by(3) {
        out.push_str(&format!("${a:04X} {}\n", if m.peek(a) == jmp { "JMP" } else { "?" }));
    }
    out.push_str("# UNC BASIC's keywords, in token order from $80\n");
    let rom = unc_asm::assemble(include_str!("../../../rom/system.s")).expect("the ROM assembles");
    let mut at = rom.symbol("KWTAB").expect("KWTAB");
    let (mut token, mut word) = (0x80u16, String::new());
    loop {
        let b = m.peek(at);
        if b == 0 {
            break;
        }
        word.push((b & 0x7F) as char);
        if b & 0x80 != 0 {
            out.push_str(&format!("${token:02X} {word}\n"));
            token += 1;
            word.clear();
        }
        at += 1;
    }
    out.push_str("# The tape format\n");
    out.push_str(&format!("{} version {}\n", String::from_utf8_lossy(unc_asm::tape::MAGIC), unc_asm::tape::VERSION));
    // the AUNT is Rev A too: its id, its sample clock and the tables its sound is made of
    use unc_core::chip;
    out.push_str("# The AUNT: id, cycles a sample, the envelope's cycles a step, the filter's corners and resonances\n");
    out.push_str(&format!("${:02X} at ${:04X}, a sample every {} cycles\n", chip::ID, machine::io::SOUND, chip::CYCLES_PER_SAMPLE));
    let row = |v: &[u32]| v.iter().map(u32::to_string).collect::<Vec<_>>().join(" ");
    out.push_str(&format!("{}\n", row(&chip::RATE_CYCLES.map(u32::from))));
    for line in chip::CUTOFF_TABLE.chunks(16) {
        out.push_str(&format!("{}\n", row(&line.iter().map(|&c| c as u32).collect::<Vec<_>>())));
    }
    out.push_str(&format!("{}\n", row(&chip::RESONANCE_TABLE)));
    out
}

#[test]
fn rev_a_is_as_it_was_frozen() {
    let path = format!("{}/tests/rev_a.txt", env!("CARGO_MANIFEST_DIR"));
    let now = rev_a();
    if std::env::var("UNC_FREEZE_REV_A").is_ok() {
        std::fs::write(&path, &now).unwrap();
        return;
    }
    let frozen = std::fs::read_to_string(&path).expect("tests/rev_a.txt");
    if frozen != now {
        let diff: Vec<String> = frozen
            .lines()
            .zip(now.lines())
            .filter(|(a, b)| a != b)
            .take(5)
            .map(|(a, b)| format!("  was: {a}\n  now: {b}"))
            .collect();
        panic!("this is a Rev B change: System Rev A is frozen (REVISIONS.md)\n{}", diff.join("\n"));
    }
}
