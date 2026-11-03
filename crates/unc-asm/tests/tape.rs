//! The tape format (manual chapter 14): what `tape::make` writes, `tape::parse` reads back.

use unc_asm::tape::{self, BASIC_START, HEADER_LEN, MAGIC, Program, TYPE_BASIC, TYPE_MACHINE_CODE, TapeError, VERSION};

#[test]
fn a_basic_program_round_trips_as_type_2() {
    // 10 PRINT 1, then the end marker: length 7, line 10, PRINT's token, " 1", 0; 0.
    let program = [7, 10, 0, 0x80, b' ', b'1', 0, 0];
    let t = tape::basic("hello", &program).encode();
    assert_eq!(t.len(), HEADER_LEN + program.len() + 1);
    assert_eq!(t[5], TYPE_BASIC);
    assert_eq!(&t[22..28], &[0x00, 0x08, 8, 0, 0, 0], "load $0800, length 8, entry 0");
    let p = &tape::parse(&t).unwrap()[0];
    assert_eq!((p.kind, p.name(), p.load, p.entry), (TYPE_BASIC, "HELLO".to_string(), BASIC_START, 0));
    assert_eq!(p.data, program);
    assert_eq!(tape::check_basic(&program), Ok(vec![10]));
    assert_eq!(tape::check_basic(&[0]), Ok(vec![]), "no lines: just the end marker");
    // a machine-code program and a BASIC one, on one tape
    let a = unc_asm::assemble(SRC).unwrap();
    let both = [tape::make(&a, "CODE"), t].concat();
    let kinds: Vec<u8> = tape::parse(&both).unwrap().iter().map(|p| p.kind).collect();
    assert_eq!(kinds, [TYPE_MACHINE_CODE, TYPE_BASIC]);
    // what a machine holds is what goes on the tape
    let mut m = unc_core::Machine::boot(unc_core::RamSize::K16);
    m.load(BASIC_START, &program);
    m.load(tape::PRGEND, &[0x07, 0x08]);
    assert_eq!(tape::resident_basic(&m), program);
    m.load(tape::PRGEND, &[0x05, 0x08]);
    assert_eq!(tape::resident_basic(&m), [0], "PRGEND not on an end marker: no program");
}

const SRC: &str = "
        .org $0900
data:   .byte 1, 2, 3
start:  LDA #2
        STA $C020
        HLT
        .org $0A00
        .byte $FF
";

#[test]
fn a_tape_is_versioned_from_byte_0_and_round_trips() {
    let a = unc_asm::assemble(SRC).unwrap();
    let t = tape::make(&a, "Hello, world 1984 tape");
    assert_eq!(&t[..4], MAGIC);
    assert_eq!(t[4], VERSION);
    assert_eq!(t[5], TYPE_MACHINE_CODE);
    assert_eq!(&t[6..22], b"HELLO, WORLD 198", "upper case, 16 characters");
    assert_eq!(t.len(), HEADER_LEN + 0x101 + 1, "$0900-$0A00 and a checksum");
    let progs = tape::parse(&t).unwrap();
    assert_eq!(progs.len(), 1);
    let p = &progs[0];
    assert_eq!((p.load, p.entry, p.data.len()), (0x0900, 0x0903, 0x101));
    assert_eq!(p.name(), "HELLO, WORLD 198");
    assert_eq!(&p.data[..3], &[1, 2, 3]);
    assert_eq!(p.data[0x50], 0, "gaps are 0, as fresh memory is");
    assert_eq!(p.data[0x100], 0xFF);
    assert_eq!(p.encode(), t);
}

#[test]
fn names_are_padded_and_only_keep_what_the_unc_can_type() {
    assert_eq!(&tape::tape_name("snake"), b"SNAKE           ");
    assert_eq!(&tape::tape_name("a{b}~c"), b"ABC             ");
}

#[test]
fn programs_cat_together_into_one_tape() {
    let one = Program { kind: 1, name: tape::tape_name("ONE"), load: 0x0800, entry: 0x0800, data: vec![9; 10] };
    let two = Program { kind: 1, name: tape::tape_name("TWO"), load: 0x3000, entry: 0x3005, data: vec![] };
    let t = [one.encode(), two.encode()].concat();
    assert_eq!(tape::parse(&t).unwrap(), vec![one, two]);
}

#[test]
fn damage_is_found() {
    let a = unc_asm::assemble(SRC).unwrap();
    let t = tape::make(&a, "X");
    let mut bad = t.clone();
    bad[HEADER_LEN + 1] ^= 0x10;
    assert!(matches!(tape::parse(&bad), Err(TapeError::Checksum { offset: 0, .. })));
    assert_eq!(tape::parse(&t[..t.len() - 1]), Err(TapeError::Truncated { offset: 0 }));
    let mut v2 = t.clone();
    v2[4] = 2;
    assert_eq!(tape::parse(&v2), Err(TapeError::Version { offset: 0, version: 2 }));
    assert_eq!(tape::parse(b"HELLO"), Err(TapeError::NoMagic { offset: 0 }));
    assert_eq!(tape::checksum(&t[4..]), 0, "the checksum makes the XOR of a whole program 0");
}

#[test]
fn the_command_line_writes_a_tape() {
    let dir = std::env::temp_dir().join(format!("unc-asm-tape-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("demo.s");
    std::fs::write(&src, SRC).unwrap();
    let out = dir.join("demo.tap");
    let run = std::process::Command::new(env!("CARGO_BIN_EXE_unc-asm"))
        .arg(&src)
        .arg("--tape")
        .arg(&out)
        .status()
        .unwrap();
    assert!(run.success());
    let t = std::fs::read(&out).unwrap();
    assert_eq!(tape::parse(&t).unwrap()[0].name(), "DEMO", "named after the file");
    let named = dir.join("named.tap");
    let run = std::process::Command::new(env!("CARGO_BIN_EXE_unc-asm"))
        .args([src.as_os_str(), "--tape".as_ref(), named.as_os_str(), "--name".as_ref(), "My Game".as_ref()])
        .status()
        .unwrap();
    assert!(run.success());
    assert_eq!(tape::parse(&std::fs::read(&named).unwrap()).unwrap()[0].name(), "MY GAME");
    std::fs::remove_dir_all(&dir).ok();
}
