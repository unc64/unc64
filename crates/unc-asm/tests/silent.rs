//! The instructions the book leaves out ($D0-$D7, `unc_core::isa::UNDOCUMENTED`) are to be
//! found, not taught: the manual, the assembler, the language data and the editor files
//! say nothing about them, and the disassembler shows them as bytes.

use unc_core::isa::{OPCODES, UNDOCUMENTED};

const NAMES: [&str; 7] = ["LAX", "SAX", "DCP", "ISB", "SLO", "ANC", "TXY"];

fn read(path: &str) -> String {
    std::fs::read_to_string(format!("{}/../../{path}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

#[test]
fn the_manual_and_the_tools_are_silent_about_the_hidden_instructions() {
    let manual = read("docs/prg/programmers-reference-guide.md");
    let lang = unc_asm::lang::json_compact();
    let vim = unc_asm::lang::vim_syntax();
    for n in NAMES {
        let word = |t: &str| t.split(|c: char| !c.is_ascii_alphanumeric()).any(|w| w == n);
        assert!(!word(&manual), "the manual names {n}");
        assert!(!word(&lang), "the language data names {n}");
        assert!(!word(&vim), "the Vim syntax names {n}");
        assert!(unc_asm::assemble(&format!("        .org $0800\n        {n} $10\n")).is_err(), "the assembler knows {n}");
    }
    for &(code, ..) in &UNDOCUMENTED {
        assert!(OPCODES[code as usize].is_none(), "${code:02X} is in the documented table");
        assert!(!manual.contains(&format!("| ${code:02X} |")), "Appendix C lists ${code:02X}");
    }
}
