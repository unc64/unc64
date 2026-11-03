use unc_asm::{assemble, disasm};
use unc_core::isa::{Mode, OPCODES, Op, opcode_for};

fn bytes(src: &str) -> Vec<u8> {
    let a = assemble(src).unwrap_or_else(|e| panic!("{e:?}"));
    a.segments.iter().flat_map(|s| s.bytes.clone()).collect()
}

fn err(src: &str) -> String {
    let e = assemble(src).expect_err("should fail");
    format!("{}: {}", e[0].line, e[0].message)
}

fn op(o: Op, m: Mode) -> u8 {
    opcode_for(o, m).unwrap()
}

#[test]
fn every_addressing_mode_syntax() {
    let cases: &[(&str, Vec<u8>)] = &[
        ("NOP", vec![op(Op::Nop, Mode::Imp)]),
        ("ASL A", vec![op(Op::Asl, Mode::Acc)]),
        ("asl", vec![op(Op::Asl, Mode::Acc)]),
        ("LDA #$12", vec![op(Op::Lda, Mode::Imm), 0x12]),
        ("LDA $12", vec![op(Op::Lda, Mode::Fp), 0x12]),
        ("LDA $12,X", vec![op(Op::Lda, Mode::Fpx), 0x12]),
        ("LDX $12,Y", vec![op(Op::Ldx, Mode::Fpy), 0x12]),
        ("LDA $1234", vec![op(Op::Lda, Mode::Abs), 0x34, 0x12]),
        ("LDA $1234,x", vec![op(Op::Lda, Mode::Absx), 0x34, 0x12]),
        ("LDA $1234,Y", vec![op(Op::Lda, Mode::Absy), 0x34, 0x12]),
        ("LDA $12,Y", vec![op(Op::Lda, Mode::Absy), 0x12, 0x00]),
        ("LDA ($12),Y", vec![op(Op::Lda, Mode::Indy), 0x12]),
        ("JMP ($1234)", vec![op(Op::Jmp, Mode::Ind), 0x34, 0x12]),
        ("JMP $1234", vec![op(Op::Jmp, Mode::Abs), 0x34, 0x12]),
        ("LDA $0012", vec![op(Op::Lda, Mode::Abs), 0x12, 0x00]),
        ("lda #'A'", vec![op(Op::Lda, Mode::Imm), 65]),
        ("LDA #%1010", vec![op(Op::Lda, Mode::Imm), 10]),
        ("LDA #-1", vec![op(Op::Lda, Mode::Imm), 0xFF]),
    ];
    for (src, want) in cases {
        assert_eq!(&bytes(src), want, "{src}");
    }
}

#[test]
fn programs_start_at_0800_by_default_and_org_moves_them() {
    let a = assemble("NOP").unwrap();
    assert_eq!(a.segments[0].start, 0x0800);
    let a = assemble(".org $C000\nNOP\n.org $1000\nNOP NOP".replace("NOP NOP", "HLT").as_str()).unwrap();
    assert_eq!(a.segments.len(), 2);
    assert_eq!((a.segments[0].start, a.segments[1].start), (0xC000, 0x1000));
}

#[test]
fn labels_forward_and_backward() {
    let src = "start: LDX #3\nloop: DEX\n BNE loop\n JMP done\n NOP\ndone: HLT";
    let b = bytes(src);
    assert_eq!(&b[3..5], &[op(Op::Bne, Mode::Rel), (-3i8) as u8]);
    assert_eq!(&b[5..8], &[op(Op::Jmp, Mode::Abs), 0x09, 0x08]);
    let a = assemble(src).unwrap();
    assert_eq!(a.symbol("done"), Some(0x0809));
    assert_eq!(a.start(), Some(0x0800));
}

#[test]
fn forward_references_to_the_fast_page_still_assemble_as_absolute() {
    let b = bytes(" LDA ptr\nptr = $10");
    assert_eq!(b, vec![op(Op::Lda, Mode::Abs), 0x10, 0x00]);
    let b = bytes("ptr = $10\n LDA ptr");
    assert_eq!(b, vec![op(Op::Lda, Mode::Fp), 0x10]);
}

#[test]
fn expressions() {
    let b = bytes("BASE = $1234\n LDA #<BASE\n LDA #>BASE\n LDA #[2+3]*4\n LDA #10/3\n LDA #'A'+1\n.word BASE+1, BASE-$34");
    assert_eq!(b, vec![0x28, 0x34, 0x28, 0x12, 0x28, 20, 0x28, 3, 0x28, 66, 0x35, 0x12, 0x00, 0x12]);
    let b = bytes(".org $2000\nhere: .word *, here");
    assert_eq!(b, vec![0x00, 0x20, 0x00, 0x20]);
}

#[test]
fn data_directives() {
    let b = bytes(".byte 1, $FF, \"AB\", 'C'\n.text \"HI\"\n.asciiz \"OK\"\n.fill 3, $EA\n.fill 2");
    assert_eq!(b, vec![1, 0xFF, b'A', b'B', b'C', b'H', b'I', b'O', b'K', 0, 0xEA, 0xEA, 0xEA, 0, 0]);
}

#[test]
fn comments_and_semicolons_inside_strings() {
    let b = bytes("  LDA #1 ; load one\n.text \"A;B\" ; a semicolon in a string\n; whole line comment");
    assert_eq!(b, vec![0x28, 1, b'A', b';', b'B']);
}

#[test]
fn line_info_maps_addresses_back_to_source() {
    let a = assemble("start:\n  LDA #1\n\n  STA $0400\n").unwrap();
    let l: Vec<(usize, u16, u16)> = a.lines.iter().map(|l| (l.line, l.addr, l.len)).collect();
    assert_eq!(l, vec![(2, 0x0800, 2), (4, 0x0802, 3)]);
}

#[test]
fn helpful_errors() {
    assert_eq!(err("  LDQ #1"), "1: unknown instruction 'LDQ'");
    assert_eq!(err("  STA #1"), "1: STA has no immediate mode");
    assert_eq!(err("  LDA nowhere"), "1: undefined symbol 'nowhere'");
    assert_eq!(err("x: NOP\nx: NOP"), "2: 'x' is defined twice");
    assert_eq!(err("  LDA #256"), "1: #256 doesn't fit in a byte");
    assert_eq!(err("  INC $1234,Y"), "1: INC has no absolute,Y mode");
    assert_eq!(err("  BNE far\n.fill 200\nfar: NOP"), "1: branch out of range (200 bytes)");
    assert_eq!(err("  LDA ($1234),Y"), "1: $1234 is outside the fast page");
    assert_eq!(err(".bogus 1"), "1: unknown directive '.bogus'");
    assert_eq!(err("  LDA #[1+2"), "1: missing ']'");
    assert_eq!(err("  JMP"), "1: JMP needs an operand");
}

#[test]
fn every_opcode_round_trips_through_the_disassembler() {
    for code in 0..=255u8 {
        let Some(info) = OPCODES[code as usize] else {
            let (text, len) = disasm(&|_| code, 0x0800);
            assert_eq!((text, len), (format!(".byte ${code:02X}"), 1));
            continue;
        };
        let mem = [code, 0x12, 0x34];
        let (text, len) = disasm(&|a| mem[(a - 0x0800) as usize], 0x0800);
        assert_eq!(len, info.mode.len() as u16);
        let b = bytes(&format!(".org $0800\n {text}"));
        assert_eq!(b, mem[..len as usize].to_vec(), "{:?} {:?} -> {text}", info.op, info.mode);
    }
}

#[test]
fn the_system_rom_binary_matches_its_source() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let src = std::fs::read_to_string(format!("{root}/rom/system.s")).unwrap();
    let bin = std::fs::read(format!("{root}/rom/system.bin")).unwrap();
    let a = assemble(&src).unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(a.image(0xE000, 0xFFFF, 0xFF), bin, "rom/system.bin is stale: run `cargo run -p unc-asm -- rom/system.s -o rom/system.bin --range E000-FFFF`");
    assert_eq!(&bin[..], &unc_core::machine::SYSTEM_ROM[..], "rebuild unc-core after changing the ROM");
}

#[test]
fn mnemonic_lookup_is_case_insensitive_and_complete() {
    for &o in unc_core::isa::ALL_OPS {
        assert_eq!(Op::from_mnemonic(&o.mnemonic().to_lowercase()), Some(o));
        assert!(OPCODES.iter().flatten().any(|i| i.op == o), "{o:?} has no opcode");
    }
}

#[test]
fn low_and_high_byte_take_the_whole_expression() {
    let b = bytes("T = $12FF\n LDA #<T+1\n LDA #>T+1\n LDA #<[T]+1\n LDA #>T-$FF\n.byte <T+1, >T+1, [<T]-1");
    assert_eq!(b, vec![0x28, 0x00, 0x28, 0x13, 0x28, 0x00, 0x28, 0x12, 0x00, 0x13, 0xFE]);
}

#[test]
fn operators_follow_c_precedence() {
    let cases: &[(&str, u8)] = &[
        ("1 + 2 * 3", 7),
        ("[1 + 2] * 3", 9),
        ("1 | 2 << 3", 17),
        ("$F0 & $3C | 1", 0x31),
        ("$FF ^ $0F & $3C", 0xF3),
        ("1 << 2 + 1", 8),
        ("$80 >> 3", 0x10),
        ("17 % 5", 2),
        ("17 %5", 2),
        ("10 % 3 * 2", 2),
        ("%0101", 5),
        ("[%11] + %1", 4),
        ("2 * %11", 6),
        ("~0", 0xFF),
        ("~$0F & $FF", 0xF0),
        ("-1 & 7", 7),
        ("<$1234 | $80", 0xB4),
        ("3 < 4", 1),
        ("3 > 4", 0),
        ("4 <= 4", 1),
        ("5 >= 6", 0),
        ("2 + 2 == 4", 1),
        ("1 != 1", 0),
        ("1 < 2 == 1", 1),
        ("6 & 3 == 3", 0),
        ("1 << 2 < 5", 1),
        ("[<$1234] < $40", 1),
        ("<$1234 < $40", 0),
        ("[>$1234] > $11", 1),
        ("'\\''", b'\''),
        ("'\\\\'", b'\\'),
    ];
    for (e, want) in cases {
        assert_eq!(bytes(&format!(".byte {e}")), vec![*want], "{e}");
    }
    let b = bytes(".org $0810\nhere: .byte * & $FF, * % 7, 2 * * - $1000");
    assert_eq!(b, vec![0x10, (0x0810 % 7) as u8, 0x20]);
    assert_eq!(err(".byte 5 % 0"), "1: division by zero");
    assert_eq!(err(".byte 1 << 70"), "1: bad shift count 70");
}

#[test]
fn a_constant_defined_later_keeps_its_own_address() {
    let a = assemble("start: NOP\nDIST = later - *\n.byte DIST\nlater: NOP").unwrap();
    assert_eq!(a.symbol("DIST"), Some(1));
    assert_eq!(a.segments[0].bytes, vec![op(Op::Nop, Mode::Imp), 1, op(Op::Nop, Mode::Imp)]);
}

#[test]
fn writing_an_address_twice_is_an_error() {
    let e = err(".org $1000\n LDA #1\n NOP\n.org $1002\n HLT");
    assert_eq!(e, "5: address $1002 is written twice (first by line 3)");
    let e = assemble(".org $1000\n.fill 4\n.org $1000\n.fill 2").unwrap_err();
    assert_eq!((e[0].line, e[0].message.as_str()), (4, "address $1000 is written twice (first by line 2)"));
    assert!(assemble(".org $1000\n NOP\n.org $1001\n NOP").is_ok(), "touching is fine");
}

#[test]
fn align_pads_and_res_reserves() {
    let a = assemble(".org $1001\n NOP\n.align 4\nfour: NOP\n.align $100, $EA\npage: HLT").unwrap();
    assert_eq!(a.symbol("four"), Some(0x1004));
    assert_eq!(a.symbol("page"), Some(0x1100));
    assert_eq!(a.segments.len(), 1);
    assert_eq!(&a.segments[0].bytes[1..3], &[0, 0]);
    assert_eq!(a.segments[0].bytes[5], 0xEA);
    let a = assemble(".org $3000\nbuf: .res 16\nbuf2: .dsb 2\nafter: NOP\n.org $3000\n.byte 7").unwrap();
    assert_eq!((a.symbol("buf"), a.symbol("buf2"), a.symbol("after")), (Some(0x3000), Some(0x3010), Some(0x3012)));
    let seg: Vec<(u16, usize)> = a.segments.iter().map(|s| (s.start, s.bytes.len())).collect();
    assert_eq!(seg, vec![(0x3012, 1), (0x3000, 1)], "reserved bytes aren't written, so they don't clash");
    assert_eq!(err(".align 0"), "1: bad align count 0");
    assert_eq!(err(".res -1"), "1: bad reserve count -1");
}

#[test]
fn rept_repeats_a_block_with_an_optional_counter() {
    assert_eq!(bytes(".rept 3\n NOP\n.endr"), vec![op(Op::Nop, Mode::Imp); 3]);
    assert_eq!(bytes(".rept 4, i\n.byte i * 10\n.endr"), vec![0, 10, 20, 30]);
    assert_eq!(bytes("N = 2\n.rept N, row\n.rept 2, col\n.byte row * 16 + col\n.endr\n.endr"), vec![0x00, 0x01, 0x10, 0x11]);
    assert_eq!(bytes(".rept 0\n NOP\n.endr\n HLT"), vec![op(Op::Hlt, Mode::Imp)]);
    assert_eq!(bytes(".rept 2, i\n.text \"i\"\n.endr"), b"ii".to_vec(), "strings are left alone");
    let a = assemble("tbl:\n.rept 3, k\n.word tbl + k * 2 ; k\n.endr").unwrap();
    assert_eq!(a.segments[0].bytes, vec![0x00, 0x08, 0x02, 0x08, 0x04, 0x08]);
    assert_eq!(a.listing.iter().filter(|l| l.line == 3).count(), 3, "each copy is listed");
    assert_eq!(err(".rept 2\n NOP"), "1: '.rept' without '.endr'");
    assert_eq!(err(" NOP\n.endr"), "2: '.endr' without '.rept' or '.irp'");
    assert_eq!(err(".rept later\n.endr\nlater:"), "1: the .rept count must be known here: undefined symbol 'later'");
    assert_eq!(err(".rept 2, X\n.endr"), "1: 'X' is a register: name the counter something else");
}

#[test]
fn local_labels_belong_to_the_global_label_before_them() {
    let src = "one:  LDX #2\n@loop: DEX\n  BNE @loop\n  JMP @done\n@done: RTS\ntwo:  LDX #3\n@loop: DEX\n  BNE @loop\n  RTS";
    let a = assemble(src).unwrap();
    assert_eq!(a.symbol("one@loop"), Some(0x0802));
    assert_eq!(a.symbol("two@loop"), Some(0x080B));
    assert_eq!(a.symbol("one@done"), Some(0x0808));
    let b = &a.segments[0].bytes;
    assert_eq!(&b[3..5], &[op(Op::Bne, Mode::Rel), (-3i8) as u8]);
    assert_eq!(&b[5..8], &[op(Op::Jmp, Mode::Abs), 0x08, 0x08]);
    assert_eq!(err("one: NOP\n BNE @nowhere"), "2: undefined local label '@nowhere' (after 'one')");
    assert_eq!(err("one:\n@x: NOP\n@x: NOP"), "3: '@x' is defined twice (after 'one')");
    assert_eq!(err(" JMP @x"), "1: undefined local label '@x' (there's no global label before it)");
    assert_eq!(bytes("v: .byte '@', \"@a\""), vec![b'@', b'@', b'a'], "an @ in a string is just a character");
}

#[test]
fn includes_come_through_the_resolver() {
    let files = |name: &str| match name {
        "consts.s" => Some("BORDER = $C020\n.include \"more.s\"".to_string()),
        "more.s" => Some("COLOUR = 2".to_string()),
        "bad.s" => Some(" NOP\n LDQ #1".to_string()),
        "self.s" => Some(".include \"self.s\"".to_string()),
        _ => None,
    };
    let a = unc_asm::assemble_with(".include \"consts.s\"\n LDA #COLOUR\n STA BORDER", files).unwrap();
    assert_eq!(a.segments[0].bytes, vec![0x28, 2, op(Op::Sta, Mode::Abs), 0x20, 0xC0]);
    assert_eq!(a.lines[0].line, 2);
    assert_eq!(a.lines[0].file, None);
    let e = unc_asm::assemble_with(" NOP\n.include \"bad.s\"", files).unwrap_err();
    assert_eq!((e[0].file.as_deref(), e[0].line, e[0].message.as_str()), (Some("bad.s"), 2, "unknown instruction 'LDQ'"));
    let e = unc_asm::assemble_with(".include \"nope.s\"", files).unwrap_err();
    assert_eq!(e[0].message, "can't read 'nope.s'");
    let e = unc_asm::assemble_with(".include \"self.s\"", files).unwrap_err();
    assert!(e[0].message.starts_with("includes nest too deep at 'self.s'"), "{}", e[0].message);
    assert_eq!(err(".include \"consts.s\""), "1: includes aren't available here");
    assert_eq!(err(".include consts.s"), "1: '.include' needs a file name in quotes");
}

#[test]
fn assert_fails_assembly_when_its_expression_is_zero() {
    assert!(assemble("start: NOP\nend:\n.assert * < $1A00\n.assert end - start == 1").is_ok());
    let src = "start: .fill $1300\nend:\n.assert end <= $1A00, \"code runs into the tables at $1A00\"";
    assert_eq!(err(src), "3: code runs into the tables at $1A00");
    assert_eq!(err(".assert later - 2\nlater = 2"), "1: assertion failed: later - 2");
}

#[test]
fn a_label_on_a_constant_line_is_explained() {
    assert_eq!(err("x: FOO = 1"), "1: a constant can't have a label: put 'FOO = ...' on its own line, without 'x:'");
}

#[test]
fn a_register_name_is_not_a_name() {
    assert_eq!(err("A = 5"), "1: 'A' can't be a name: as an operand, A means the accumulator");
    assert_eq!(err("a: NOP"), "1: 'a' can't be a name: as an operand, A means the accumulator");
}

#[test]
fn string_escapes() {
    let b = bytes(".text \"SAY \\\"HI\\\"\"\n.byte \"A\\\\B\", 1\n.asciiz \"\\\"\"");
    assert_eq!(b, b"SAY \"HI\"A\\B\x01\"\x00".to_vec());
    let b = bytes(".text \"A;B\\\";C\" ; comment");
    assert_eq!(b, b"A;B\";C".to_vec());
    assert_eq!(err(".text \"\\n\""), "1: unknown escape '\\n'");
}

#[test]
fn listing_and_symbol_file() {
    let a = assemble("PTR = $10\nstart: LDA PTR,Y\n LDX PTR,Y\n LDA later\nlater = $20\n.text \"HELLO\"").unwrap();
    let list = a.listing_text();
    let lines: Vec<&str> = list.lines().collect();
    assert!(lines[1].starts_with("0800  "), "{list}");
    assert!(lines[1].ends_with("start: LDA PTR,Y  ; ** LDA has no fast page,Y mode: absolute,Y"), "{list}");
    assert!(lines[2].starts_with("0803  ") && !lines[2].contains("**"), "LDX has fp,Y: {list}");
    assert!(lines[3].ends_with("; ** defined later: absolute"), "{list}");
    assert!(list.contains("HELLO") && list.contains("080B  4C 4F"), "{list}");
    assert_eq!(a.symbol_file().lines().find(|l| l.starts_with("start")), Some("start = $0800"));
}

#[test]
fn load_sets_the_documented_start_state() {
    let a = assemble("start: HLT").unwrap();
    let mut m = unc_core::Machine::boot(unc_core::RamSize::K16);
    m.cpu.a = 9;
    m.cpu.sp = 0x40;
    m.cpu.p = 0xFF;
    assert_eq!(unc_asm::load(&mut m, &a), 0x0800);
    let c = &m.cpu;
    assert_eq!((c.pc, c.sp, c.p, c.a, c.x, c.y), (0x0800, 0xFF, 0x24, 0, 0, 0));
    let m = unc_asm::boot(unc_core::RamSize::K64, &assemble(".org $3000\n NOP").unwrap());
    assert_eq!((m.cpu.pc, m.peek(0x3000)), (0x3000, op(Op::Nop, Mode::Imp)));
    assert_eq!(m.peek(0x0203), 0xC0, "the ROM has booted");
}
