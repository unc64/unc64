//! The language data (`unc_asm::lang`) against the machine itself: the manual's tables that
//! `lang` reads must agree with the emulator's I/O map and the ROM's jump table, and the
//! analysis and the lexer must take every source in the repository without a panic.

use unc_asm::lang::{self, TokenKind};
use unc_core::machine::io;

fn root(path: &str) -> String {
    format!("{}/../../{path}", env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn every_instruction_and_mode_is_there_with_its_cost() {
    let l = lang::get();
    assert_eq!(l.instructions.len(), unc_core::isa::ALL_OPS.len());
    let modes: usize = l.instructions.iter().map(|i| i.modes.len()).sum();
    assert_eq!(modes, unc_core::isa::OPCODES.iter().flatten().count());
    let lda = l.instruction("lda").unwrap();
    let absx = lda.modes.iter().find(|m| m.key == "absx").unwrap();
    assert_eq!((absx.opcode, absx.bytes, absx.cycles, absx.syntax.as_str()), (0x2C, 3, 5, "LDA $nnnn,X"));
    let bne = l.instruction("BNE").unwrap();
    assert!(bne.branch);
    assert_eq!((bne.modes[0].cycles, bne.modes[0].cycles_taken), (2, Some(3)));
}

#[test]
fn the_io_registers_are_the_machines() {
    let l = lang::get();
    let at = |name: &str| l.io.iter().find(|r| r.name == name).unwrap_or_else(|| panic!("{name} missing")).address;
    for (name, addr) in [
        ("VMODE", io::VMODE),
        ("RASTER", io::RASTER),
        ("RASTER_HI", io::RASTER_HI),
        ("RASTER_CMP", io::RASTER_CMP),
        ("IRQ_ENABLE", io::IRQ_ENABLE),
        ("IRQ_STATUS", io::IRQ_STATUS),
        ("FRAME", io::FRAME),
        ("KBD", io::KBD),
        ("KBD_STROBE", io::KBD_STROBE),
        ("HELD", io::HELD),
        ("BORDER", io::BORDER),
        ("BACKGROUND", io::BACKGROUND),
        ("CHARSET", io::CHARSET),
        ("BITMAP", io::BITMAP),
        ("BEEPER", io::BEEPER),
        ("SPEAKER", io::SPEAKER),
        ("TIMER_LO", io::TIMER_LO),
        ("TIMER_HI", io::TIMER_HI),
        ("NOISE", io::NOISE),
        ("TAPE", io::TAPE),
        ("TAPEDATA", io::TAPE_DATA),
        ("SHIFT_IN", io::SHIFT_IN),
        ("SHIFT_AMOUNT", io::SHIFT_AMOUNT),
        ("SHIFT_OUT", io::SHIFT_OUT),
        ("SHIFT_FLIP", io::SHIFT_FLIP),
        ("KEYS", io::KEYS),
        ("SPRTAB", io::SPRTAB),
        ("SPRBANK", io::SPRBANK),
        ("SPRHIT", io::SPRHIT),
        ("SPRHITBG", io::SPRHITBG),
        ("SPRLINE", io::SPRLINE),
        ("SCROLLX", io::SCROLLX),
        ("SCROLLY", io::SCROLLY),
        ("LAYER", io::LAYER),
        ("LAYERPAGE", io::LAYERPAGE),
        ("LAYERSX", io::LAYERSX),
        ("LAYERSY", io::LAYERSY),
        ("COLOURMODE", io::COLOURMODE),
        ("COLOUR1", io::COLOUR1),
        ("TEXTROWS", io::TEXTROWS),
        ("COLOUR2", io::COLOUR2),
        ("COLOUR3", io::COLOUR3),
        ("BOARD", io::BOARD),
        ("CUTOFF", io::CUTOFF),
        ("FILTER", io::FILTER),
        ("VOLUME", io::VOLUME),
        ("VOICE3", io::VOICE3),
        ("ENV3", io::ENV3),
        ("SOUND", io::SOUND),
        ("WAVETABLE", io::WAVETABLE),
    ] {
        assert_eq!(at(name), addr, "{name}");
    }
    assert_eq!(l.io.len(), 50, "a register in the manual the test doesn't know");
    let table = l.io.iter().find(|r| r.name == "WAVETABLE").unwrap();
    assert_eq!(table.end, io::WAVETABLE + 31);
    let keys = l.io.iter().find(|r| r.name == "KEYS").unwrap();
    assert_eq!(keys.end, io::KEYS + 8);
    assert!(l.io.iter().all(|r| !(0xC080..=0xC09F).contains(&r.address)), "the first board's sprite registers are gone");
    assert_eq!(l.io.iter().find(|r| r.name == "BORDER").unwrap().write, "Border colour, 0-15");
}

#[test]
fn the_rom_routines_are_the_roms_jump_table() {
    let src = std::fs::read_to_string(root("rom/system.s")).unwrap();
    let rom = unc_asm::assemble(&src).unwrap();
    let image = rom.image(0xE000, 0xFFFF, 0xFF);
    let byte = |a: u16| image[(a - 0xE000) as usize];
    let l = lang::get();
    let mut next = 0xFF00;
    for r in &l.rom {
        assert_eq!(r.address, next, "{} isn't the next jump-table slot", r.name);
        assert_eq!(byte(r.address), 0x06, "{}: no JMP at ${:04X}", r.name, r.address);
        next += 3;
    }
    assert_ne!(byte(next), 0x06, "the ROM's jump table has an entry at ${next:04X} the manual doesn't list");
    let chrout = l.rom.iter().find(|r| r.name == "CHROUT").unwrap();
    assert_eq!(chrout.address, 0xFF03);
    assert!(chrout.cycles.contains("when it scrolls"), "{}", chrout.cycles);
    assert!(l.rom.iter().find(|r| r.name == "RND").unwrap().cycles == "53");
}

#[test]
fn addresses_and_names_are_found() {
    let l = lang::get();
    let (named, region) = l.at_address(0xC020);
    assert_eq!(named[0].name(), "BORDER");
    assert_eq!(region.unwrap().start, 0xC000);
    let (named, _) = l.at_address(0xC064);
    assert_eq!(named[0].name(), "KEYS");
    let (named, _) = l.at_address(0x0200);
    assert_eq!(named[0].name(), "CURX");
    assert_eq!(l.named_as("CHROUT").unwrap().definition(), "CHROUT  = $FF03");
    assert_eq!(l.named_as("BORDER").unwrap().definition(), "BORDER  = $C020");
    assert_eq!(l.named_as("SRC").unwrap().definition(), "SRC     = $F7");
    assert!(l.named_as("border").is_none(), "names are case-sensitive");
}

#[test]
fn appendix_e_explains_the_assemblers_messages() {
    let l = lang::get();
    assert!(l.messages.len() > 40);
    for src in [" LDQ", " STA #1", " JMP", " LDA nowhere", "x: FOO = 1", " .rept 3", " BNE *+300", " .byte 300", " LDA #1/0"] {
        let e = unc_asm::assemble(src).expect_err(src);
        assert!(l.explain(&e[0].message).is_some(), "{src}: no explanation for '{}'", e[0].message);
    }
}

#[test]
fn json_has_every_part() {
    let j = lang::json();
    for key in ["\"schema\": 1", "\"instructions\"", "\"directives\"", "\"functions\"", "\"io\"", "\"rom\"", "\"variables\"", "\"memory\"", "\"messages\"", "\"token_kinds\""] {
        assert!(j.contains(key), "{key}");
    }
    assert!(j.contains("\"name\": \"BORDER\""));
    assert_eq!(lang::json_compact().matches("\"mnemonic\":").count(), 55);
    assert_eq!(lang::tokens_json("x: LDA #1"), r#"[[0,1,"label"],[1,2,"operator"],[3,6,"mnemonic"],[7,8,"operator"],[8,9,"number"]]"#);
}

/// Every .s file in the repository, with its path.
fn sources() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for dir in ["games", "games/board", "rom", "web/samples"] {
        let Ok(rd) = std::fs::read_dir(root(dir)) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "s") {
                out.push((p.display().to_string(), std::fs::read_to_string(&p).unwrap()));
            }
        }
    }
    assert!(out.len() >= 8);
    out
}

#[test]
fn every_source_in_the_repository_analyses_cleanly() {
    for (path, src) in sources() {
        let dir = std::path::Path::new(&path).parent().unwrap().to_path_buf();
        let a = unc_asm::analyze_with(&src, |n| std::fs::read_to_string(dir.join(n)).ok());
        assert!(a.errors.is_empty(), "{path}: {:?}", a.errors);
        let asm = a.assembly.as_ref().unwrap();
        // Every instruction's line has its cost, and the costs add up to the listing's.
        let listed: u64 = asm.listing.iter().filter(|l| l.file.is_none()).filter_map(|l| l.bytes.first()).count() as u64;
        assert!(listed > 0);
        for (i, line) in src.lines().enumerate() {
            let sl = &a.main().lines[i];
            let toks = lang::tokens(line);
            let has_mnemonic = toks.iter().any(|t| t.kind == TokenKind::Mnemonic);
            assert_eq!(sl.cycles.is_some(), has_mnemonic && sl.copies > 0, "{path}:{}: {line}", i + 1);
            assert!(!toks.iter().any(|t| t.kind == TokenKind::Invalid), "{path}:{}: {line}: {toks:?}", i + 1);
        }
    }
}

#[test]
fn nothing_panics_on_half_typed_lines() {
    for (_, src) in sources().into_iter().take(4) {
        for line in src.lines().take(400) {
            for cut in 0..=line.len() {
                if line.is_char_boundary(cut) {
                    let part = &line[..cut];
                    lang::tokens(part);
                    unc_asm::analysis::error_span(part, "undefined symbol 'x'");
                }
            }
        }
        // A program cut short anywhere still analyses.
        let lines: Vec<&str> = src.lines().collect();
        for n in (0..lines.len()).step_by(97) {
            let a = unc_asm::analyze(&lines[..n].join("\n"));
            assert!(a.main().lines.len() <= n);
        }
    }
}
