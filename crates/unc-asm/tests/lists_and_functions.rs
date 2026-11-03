//! `.irp`, pasting names, functions in expressions, `&&` and `||`, 64-bit literals,
//! `.align` in reserved space and the precedence warnings (manual chapter 6).

use unc_asm::asm::int_sin;
use unc_asm::assemble;

fn bytes(src: &str) -> Vec<u8> {
    let a = assemble(src).unwrap_or_else(|e| panic!("{e:?}\n{src}"));
    a.segments.iter().flat_map(|s| s.bytes.clone()).collect()
}

fn err(src: &str) -> String {
    let e = assemble(src).expect_err("should fail");
    format!("{}: {}", e[0].line, e[0].message)
}

fn value(e: &str) -> i64 {
    let a = assemble(&format!("V = {e}")).unwrap_or_else(|err| panic!("{e}: {err:?}"));
    // Symbols are addresses (u16); compare the low 16 bits, sign-extended.
    a.symbol("V").unwrap() as i16 as i64
}

#[test]
fn irp_repeats_a_block_once_per_value() {
    assert_eq!(bytes(".irp v, 3, 1, 4\n.byte v\n.endr"), vec![3, 1, 4]);
    assert_eq!(bytes(".irp v\n.byte v\n.endr\n.byte 9"), vec![9], "no values, no copies");
    let src = "B0 = $10\nB1 = $20\n.irp s, B0, B1\n LDA s\n STA s+1\n.endr";
    assert_eq!(bytes(src), bytes(" LDA $10\n STA $11\n LDA $20\n STA $21"));
    assert_eq!(bytes(".irp m, \"HI\", \"YO\"\n.text m\n.endr"), b"HIYO".to_vec(), "strings as values");
    assert_eq!(bytes(".irp v, 1 + 2, 4\n.byte v * 2\n.endr"), vec![6, 8], "an expression value is bracketed");
    assert_eq!(bytes(".irp v, min(1, 2), [3]\n.byte v\n.endr"), vec![1, 3], "commas inside brackets don't split");
    assert_eq!(bytes(".irp r, X, Y\n LDA tbl,r\n.endr\ntbl = $10"), bytes(" LDA $0010,X\n LDA $0010,Y"), "registers as values");
    assert_eq!(bytes(".irp v, 2, 3\n.byte v ; v stays in comments\n.endr"), vec![2, 3]);
    let a = assemble(".irp v, 1, 2\n.byte v ; v\n.endr").unwrap();
    assert!(a.listing.iter().all(|l| !l.text.contains("; 1")), "comments are left alone");
}

#[test]
fn irp_and_rept_nest_both_ways() {
    let src = ".irp base, $10, $40\n.rept 3, k\n.byte base + k\n.endr\n.endr";
    assert_eq!(bytes(src), vec![0x10, 0x11, 0x12, 0x40, 0x41, 0x42]);
    let src = ".rept 2, row\n.irp col, 1, 5\n.byte row * 16 + col\n.endr\n.endr";
    assert_eq!(bytes(src), vec![0x01, 0x05, 0x11, 0x15]);
    let src = ".irp a1, 1, 2\n.irp b1, 10, 20\n.byte a1 + b1\n.endr\n.endr";
    assert_eq!(bytes(src), vec![11, 21, 12, 22]);
}

#[test]
fn irp_errors() {
    assert_eq!(err(".irp X, 1\n.endr"), "1: 'X' is a register: name the counter something else");
    assert_eq!(err(".irp a, 1\n.endr"), "1: 'a' is a register: name the counter something else");
    assert_eq!(err(".irp 1v, 1\n.endr"), "1: bad counter name '1v'");
    assert_eq!(err(".irp v, 1, , 2\n.endr"), "1: an .irp value is empty");
    assert_eq!(err(".irp v, 1\n NOP"), "1: '.irp' without '.endr'");
}

#[test]
fn a_backslash_pastes_the_name_into_a_label() {
    let a = assemble(".rept 3, i\nROW\\i: .byte i\n.endr\n.word ROW2").unwrap();
    assert_eq!((a.symbol("ROW0"), a.symbol("ROW1"), a.symbol("ROW2")), (Some(0x0800), Some(0x0801), Some(0x0802)));
    assert_eq!(a.segments[0].bytes, vec![0, 1, 2, 0x02, 0x08]);
    let a = assemble(".irp s, BUG, ANT\nSPR_\\s = * \n.byte 1\n.endr\n.word SPR_ANT").unwrap();
    assert_eq!(a.symbol("SPR_ANT"), Some(0x0801));
    let a = assemble(".rept 2, i\n.rept 2, j\nT\\i\\j: .byte i * 2 + j\n.endr\n.endr").unwrap();
    assert_eq!((a.symbol("T00"), a.symbol("T11")), (Some(0x0800), Some(0x0803)));
    assert_eq!(bytes(".rept 2, i\n.text \"\\\\i\"\n.endr"), b"\\i\\i".to_vec(), "not in strings");
    assert_eq!(err(" LDA ROW\\i"), "1: '\\i' pastes a .rept or .irp name, but there's none called 'i' here");
    assert_eq!(err(".irp v, 1+2\nX\\v: NOP\n.endr"), "1: can't paste '1+2' into a name");
}

#[test]
fn sin_and_cos_use_256ths_of_a_circle() {
    assert_eq!(int_sin(64, 127), 127);
    assert_eq!(int_sin(192, 127), -127);
    assert_eq!(int_sin(32, 127), 90); // 127 * 0.7071 = 89.80
    assert_eq!(int_sin(32, 100), 71);
    assert_eq!(int_sin(1, 1000), 25); // 24.54
    for n in [0, 1, 127, -50, 1i64 << 40] {
        assert_eq!((int_sin(0, n), int_sin(128, n), int_sin(256, n)), (0, 0, 0));
    }
    for a in -300..300 {
        for n in [1, 7, 127, 1000, -64] {
            let s = int_sin(a, n);
            assert_eq!(int_sin(-a, n), -s, "odd: sin({a}, {n})");
            assert_eq!(int_sin(128 - a, n), s, "sin(128 - a) = sin(a): {a}");
            assert_eq!(int_sin(a + 256, n), s, "wraps: {a}");
            assert_eq!(int_sin(a, -n), -s, "symmetric rounding: {a}");
        }
    }
    let b = bytes(".byte sin(64, 127), sin(0, 9), cos(0, 100), cos(128, 100), sin(-64, 127), cos(64, 5)");
    assert_eq!(b, vec![127, 0, 100, (-100i8) as u8, (-127i8) as u8, 0]);
}

/// The quarter-wave table, read back through `int_sin(k, 2^62)`, must agree with the
/// identities sin² + cos² = 1 and sin 2x = 2 sin x cos x to within its rounding.
#[test]
fn sin_table_is_exact() {
    let t = |k: i64| int_sin(k, 1 << 62) as i128;
    let one: i128 = 1 << 62;
    assert_eq!((t(0), t(64)), (0, one));
    for k in 0..=64 {
        let err = t(k) * t(k) + t(64 - k) * t(64 - k) - one * one;
        assert!(err.abs() <= 3 * one / 2, "sin² + cos² at {k}: {err}");
    }
    for k in 0..=32 {
        let err = t(2 * k) - 2 * t(k) * t(64 - k) / one;
        assert!(err.abs() <= 3, "sin 2x at {k}: {err}");
    }
    for k in 0..64 {
        assert!(t(k) < t(k + 1), "rising at {k}");
    }
    // Against the host's f64 wherever f64 can't be in doubt.
    for n in (0..=4096).chain([10_000, 32_767, 65_535, 65_536]) {
        for a in 0..256 {
            let x = n as f64 * (std::f64::consts::TAU * a as f64 / 256.0).sin();
            if (x.abs().fract() - 0.5).abs() > 1e-6 {
                assert_eq!(int_sin(a, n), x.round() as i64, "sin({a}, {n}) vs {x}");
            }
        }
    }
}

#[test]
fn sqrt_abs_min_max() {
    for (x, r) in [(0, 0), (1, 1), (3, 1), (4, 2), (15, 3), (16, 4), (65535, 255), (65536, 256), (i64::MAX, 3_037_000_499i64)] {
        assert_eq!(assemble(&format!(".assert sqrt({x}) == {r}")).map(|_| ()), Ok(()), "sqrt({x})");
    }
    assert_eq!(bytes(".byte abs(-5), abs(5), min(3, -2), max(3, -2), MAX(1, 2), Sqrt(81)"), vec![5, 5, 0xFE, 3, 2, 9]);
    assert_eq!(bytes(".byte min(later, 9) + 1\nlater = 4"), vec![5], "a value defined later");
    assert_eq!(value("max(min(10, 20), sqrt(400) - 5) * 2"), 30);
}

#[test]
fn a_label_called_sin_still_works() {
    let src = "sin = $20\nmin: NOP\n LDA sin\n JMP min\n LDA #sin(64, 10)\n JMP (sin)\n LDA (sin),Y\n LDA min(sin, 5),X";
    assert_eq!(bytes(src), bytes("min: NOP\n LDA $20\n JMP $0800\n LDA #10\n JMP ($0020)\n LDA ($20),Y\n LDA 5,X"));
    assert_eq!(bytes(" JMP (max(1, $1234))\n LDA (min($30, $40)),Y"), bytes(" JMP ($1234)\n LDA ($30),Y"));
}

#[test]
fn function_errors() {
    assert_eq!(err(".byte foo(1)"), "1: unknown function 'foo'");
    assert_eq!(err(".byte sin(1)"), "1: sin takes 2 values: sin(angle, size)");
    assert_eq!(err(".byte sqrt(1, 2)"), "1: sqrt takes 1 value: sqrt(x)");
    assert_eq!(err(".byte sqrt(-4)"), "1: sqrt of a negative number (-4)");
    assert_eq!(err(".byte min(1, 2"), "1: missing ')'");
    assert_eq!(err(" LDA #(1 + 2) * 2"), "1: round brackets are for indirect addressing and functions: group with [ ]");
    assert_eq!(err(" LDA (1 + 2) * 2"), "1: round brackets are for indirect addressing and functions: group with [ ]");
    assert_eq!(err(" LDA tbl,Z\ntbl = 1"), "1: unexpected ',' in expression");
}

#[test]
fn logical_and_or() {
    let cases: &[(&str, u8)] = &[
        ("1 && 2", 1),
        ("1 && 0", 0),
        ("0 || 0", 0),
        ("0 || 7", 1),
        ("1 || 0 && 0", 1),
        ("3 == 3 && 4 > 5", 0),
        ("3 == 3 || 4 > 5", 1),
        ("3 == 3 & 4 > 5", 0),
        ("3 == 3 | 4 > 5", 1),
        ("2 & 1 && 1", 0),
    ];
    for (e, want) in cases {
        assert_eq!(bytes(&format!(".byte {e}")), vec![*want], "{e}");
    }
}

#[test]
fn hex_and_binary_numbers_take_64_bits() {
    assert_eq!(bytes("SPR = $81423C5A3C993C42\n.rept 8, i\n.byte <[SPR >> [56 - 8 * i]]\n.endr"), vec![0x81, 0x42, 0x3C, 0x5A, 0x3C, 0x99, 0x3C, 0x42]);
    assert_eq!(bytes(".byte $FFFFFFFFFFFFFFFF == -1, $7FFFFFFFFFFFFFFF > 0, $8000000000000000 < 0"), vec![1, 1, 1]);
    assert_eq!(bytes(&format!(".byte %{} == -1", "1".repeat(64))), vec![1]);
    assert_eq!(err(".byte $10000000000000000"), "1: bad number '$10000000000000000'");
    assert_eq!(err(".byte 9223372036854775808"), "1: bad number '9223372036854775808'");
}

#[test]
fn align_reserves_unless_it_is_between_stored_bytes() {
    // After .res and before nothing: a gap, like snake's character set.
    let a = assemble(".org $3000\n NOP\nbuf: .res 3\n.align 256\nset: .res $400").unwrap();
    assert_eq!(a.symbol("set"), Some(0x3100));
    assert_eq!(a.segments.iter().map(|s| (s.start, s.bytes.len())).collect::<Vec<_>>(), vec![(0x3000, 1)]);
    // After .res, before code: the code starts its own segment on the boundary.
    let a = assemble(".org $3000\n.res 3\n.align 16\ntbl: .byte 1").unwrap();
    assert_eq!(a.segments.iter().map(|s| (s.start, s.bytes.len())).collect::<Vec<_>>(), vec![(0x3010, 1)]);
    // After code, before .res or the end: nothing stored.
    let a = assemble(".org $3000\n NOP\n.align 16\nbuf: .res 16\n.align 256").unwrap();
    assert_eq!((a.symbol("buf"), a.segments[0].bytes.len()), (Some(0x3010), 1));
    // Straight after .org: the program starts on the boundary.
    let a = assemble(".org $1001\n.align 4\nstart: NOP").unwrap();
    assert_eq!((a.segments[0].start, a.start()), (0x1004, Some(0x1004)));
    // Between code: zeros, as before; labels and constants between don't matter.
    let a = assemble(".org $1001\n NOP\nX1 = 1\n.align 4\nfour: NOP").unwrap();
    let nop = bytes(" NOP")[0];
    assert_eq!((a.segments.len(), a.segments[0].start), (1, 0x1001));
    assert_eq!(a.segments[0].bytes, vec![nop, 0, 0, nop]);
    // A fill value always stores.
    let a = assemble(".org $1000\n.res 1\n.align 4, $EA\n NOP").unwrap();
    assert_eq!(a.segments[0].start, 0x1001);
    assert_eq!(&a.segments[0].bytes[..3], &[0xEA; 3]);
    // Two in a row follow the same rule.
    let a = assemble(".org $1000\n.res 1\n.align 2\n.align 8\nx: NOP").unwrap();
    assert_eq!((a.segments[0].start, a.symbol("x")), (0x1008, Some(0x1008)));
    let a = assemble(".org $1000\n NOP\n.align 2\n.align 8\n NOP").unwrap();
    assert_eq!(a.segments[0].bytes.len(), 9);
    // The listing shows the address of reserved padding.
    let a = assemble(".org $3000\n.res 3\n.align 16").unwrap();
    assert_eq!(a.listing[2].addr, Some(0x3003));
}

#[test]
fn precedence_traps_warn() {
    let a = assemble("F = $3000\n.byte F & $FF == 0\n.byte <F == 0\n.byte F == 0 | 1").unwrap();
    let w: Vec<(usize, &str)> = a.warnings.iter().map(|w| (w.line, w.message.as_str())).collect();
    assert_eq!(
        w,
        vec![
            (2, "'==' binds tighter than '&': add [ ] to say which you mean"),
            (3, "'<' takes the low byte of the whole comparison: add [ ] to say which you mean"),
            (4, "'==' binds tighter than '|': add [ ] to say which you mean"),
        ]
    );
    assert!(a.listing_text().contains("; ** '==' binds tighter than '&'"), "{}", a.listing_text());
    assert_eq!(a.segments[0].bytes, vec![0, 0, 1], "they still assemble as written");
    let quiet = "F = $3000\n.byte [F & $FF] == 0, [<F] == 0, F == 1 & F == 2, F == 1 | F != 2 & F > 3\n.assert F > 0 && F < $4000\n LDA #<F\n LDA #>F + 1";
    assert_eq!(assemble(quiet).unwrap().warnings, vec![], "brackets, logic and plain bytes are fine");
    let a = assemble("ok = 1\n.assert ok & 1 == 1, \"never\"").unwrap();
    assert_eq!(a.warnings.len(), 1, "asserts are checked too");
    assert_eq!(
        err("MYSET = $3100\n.assert MYSET & $FF == 0, \"on a page\""),
        "2: on a page (warning: '==' binds tighter than '&': add [ ] to say which you mean)",
        "a failed assert says why it might have failed"
    );
    let a = assemble(" LDA #>$1234 >= 1\nC = 1 < 2 ^ 3").unwrap();
    assert_eq!(a.warnings.iter().map(|w| w.line).collect::<Vec<_>>(), vec![1, 2]);
}
