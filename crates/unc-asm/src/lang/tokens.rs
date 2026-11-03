//! A lexer for one line of UNC assembly, for highlighting: every part of the line with what
//! it is. It reads a line the way the assembler does (chapter 6), but never fails: what the
//! assembler would reject comes out as `Invalid` or as the nearest kind.

use crate::asm::{DIRECTIVE_NAMES, FUNCTIONS};
use unc_core::isa::Op;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenKind {
    /// An instruction: `LDA`.
    Mnemonic,
    /// A directive: `.byte`.
    Directive,
    /// A label being defined: `loop` in `loop:`.
    Label,
    /// A local label, defined or used: `@loop`.
    Local,
    /// A constant being defined: `BORDER` in `BORDER = $C020`.
    Constant,
    /// A name used in an expression: a label, a constant, or a `.rept`/`.irp` name.
    Symbol,
    /// `$C020`, `%0101`, `42`, `'A'`.
    Number,
    /// `"HELLO"`, quotes included.
    String,
    /// `; ...` to the end of the line.
    Comment,
    /// `#`, `,`, `=`, `:`, `+`, `<<`, `[`, `(`, `*` (the address) and the rest.
    Operator,
    /// `A` as the accumulator operand, `X` and `Y` as index registers.
    Register,
    /// `sin`, `cos`, `sqrt`, `abs`, `min`, `max` before their `(`.
    Function,
    /// Something the assembler won't take: an unknown mnemonic or directive, a stray character.
    Invalid,
}

impl TokenKind {
    pub const ALL: [TokenKind; 13] = [
        TokenKind::Mnemonic,
        TokenKind::Directive,
        TokenKind::Label,
        TokenKind::Local,
        TokenKind::Constant,
        TokenKind::Symbol,
        TokenKind::Number,
        TokenKind::String,
        TokenKind::Comment,
        TokenKind::Operator,
        TokenKind::Register,
        TokenKind::Function,
        TokenKind::Invalid,
    ];

    /// The kind's name in JSON and in the editors' data.
    pub const fn name(self) -> &'static str {
        match self {
            TokenKind::Mnemonic => "mnemonic",
            TokenKind::Directive => "directive",
            TokenKind::Label => "label",
            TokenKind::Local => "local",
            TokenKind::Constant => "constant",
            TokenKind::Symbol => "symbol",
            TokenKind::Number => "number",
            TokenKind::String => "string",
            TokenKind::Comment => "comment",
            TokenKind::Operator => "operator",
            TokenKind::Register => "register",
            TokenKind::Function => "function",
            TokenKind::Invalid => "invalid",
        }
    }
}

/// One token: its kind and its byte range in the line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub start: usize,
    pub end: usize,
}

impl Token {
    pub fn text<'a>(&self, line: &'a str) -> &'a str {
        &line[self.start..self.end]
    }
}

pub(crate) fn is_name_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_' || c == b'@'
}

pub(crate) fn is_name_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'@'
}

/// A name, allowing `\name` pastes inside it (`ROW\i`).
fn is_name(s: &str) -> bool {
    let b = s.as_bytes();
    !b.is_empty() && is_name_start(b[0]) && b.iter().all(|&c| is_name_char(c) || c == b'\\')
}

pub(crate) fn is_directive(word: &str) -> bool {
    DIRECTIVE_NAMES.iter().any(|d| d.eq_ignore_ascii_case(word))
}

pub(crate) fn is_function(word: &str) -> bool {
    FUNCTIONS.iter().any(|f| f.0.eq_ignore_ascii_case(word))
}

/// Where the comment starts: the first `;` outside a string or character literal.
pub(crate) fn comment_start(line: &str) -> usize {
    let b = line.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b';' => return i,
            b'"' => i = skip_string(b, i),
            b'\'' => i = char_literal(b, i).unwrap_or(i + 1),
            _ => i += 1,
        }
    }
    b.len()
}

/// The end of the string starting at `i` (past its closing quote, or the end of the line).
fn skip_string(b: &[u8], i: usize) -> usize {
    let mut j = i + 1;
    while j < b.len() {
        match b[j] {
            b'\\' => j += 2,
            b'"' => return j + 1,
            _ => j += 1,
        }
    }
    b.len()
}

/// The end of the character literal at `i`, if there's one: `'A'` or `'\''`.
fn char_literal(b: &[u8], i: usize) -> Option<usize> {
    match b.get(i + 1..)? {
        [b'\\', _, b'\'', ..] => Some(i + 4),
        [c, b'\'', ..] if *c != b'\\' => Some(i + 3),
        // A character outside ASCII: find the closing quote after it.
        [c, ..] if *c >= 0x80 => {
            let mut j = i + 2;
            while j < b.len() && (b[j] & 0xC0) == 0x80 {
                j += 1;
            }
            (b.get(j) == Some(&b'\'')).then_some(j + 1)
        }
        _ => None,
    }
}

/// Classifies every part of one line of source. Byte offsets; whitespace isn't a token.
pub fn tokens(line: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let cstart = comment_start(line);
    let code = &line[..cstart];
    lex_code(code, &mut out);
    if cstart < line.len() {
        out.push(Token { kind: TokenKind::Comment, start: cstart, end: line.trim_end().len().max(cstart + 1) });
    }
    out
}

fn push(out: &mut Vec<Token>, kind: TokenKind, start: usize, end: usize) {
    if end > start {
        out.push(Token { kind, start, end });
    }
}

/// The range of `s[from..to]` with the whitespace at both ends left out.
fn trimmed(s: &str, from: usize, to: usize) -> (usize, usize) {
    let part = &s[from..to];
    if part.trim().is_empty() {
        return (from, from);
    }
    let start = from + (part.len() - part.trim_start().len());
    (start, from + part.trim_end().len())
}

fn name_kind(name: &str, def: TokenKind) -> TokenKind {
    if name.starts_with('@') { TokenKind::Local } else { def }
}

fn lex_code(code: &str, out: &mut Vec<Token>) {
    // `NAME = value`: the assembler looks at the first `=`.
    if let Some(eq) = code.find('=') {
        let (s, e) = trimmed(code, 0, eq);
        if is_name(&code[s..e]) && code.as_bytes().get(eq + 1) != Some(&b'=') {
            push(out, name_kind(&code[s..e], TokenKind::Constant), s, e);
            push(out, TokenKind::Operator, eq, eq + 1);
            lex_expr(code, eq + 1, code.len(), out);
            return;
        }
    }
    let mut rest = 0;
    // `label:` at the start: the assembler looks at the first `:`.
    if let Some(colon) = code.find(':') {
        let (s, e) = trimmed(code, 0, colon);
        if is_name(&code[s..e]) {
            push(out, name_kind(&code[s..e], TokenKind::Label), s, e);
            push(out, TokenKind::Operator, colon, colon + 1);
            rest = colon + 1;
        }
    }
    let (s, end) = trimmed(code, rest, code.len());
    if s == end {
        return;
    }
    let word_end = code[s..end].find(char::is_whitespace).map_or(end, |i| s + i);
    let word = &code[s..word_end];
    if let Some(d) = word.strip_prefix('.') {
        let known = is_directive(d);
        push(out, if known { TokenKind::Directive } else { TokenKind::Invalid }, s, word_end);
        lex_expr(code, word_end, end, out);
    } else if Op::from_mnemonic(word).is_some() {
        push(out, TokenKind::Mnemonic, s, word_end);
        lex_operand(code, word_end, end, out);
    } else if let Some(eq) = code[s..end].find('=').map(|i| s + i)
        && is_name(code[s..eq].trim())
    {
        // `x: FOO = 1`, which the assembler rejects: still show what's what.
        let (ns, ne) = trimmed(code, s, eq);
        push(out, TokenKind::Constant, ns, ne);
        push(out, TokenKind::Operator, eq, eq + 1);
        lex_expr(code, eq + 1, end, out);
    } else {
        push(out, TokenKind::Invalid, s, word_end);
        lex_expr(code, word_end, end, out);
    }
}

/// An instruction's operand: `A`, `#expr`, `expr`, `expr,X`, `(expr)`, `(expr),Y`.
fn lex_operand(code: &str, from: usize, to: usize, out: &mut Vec<Token>) {
    let (s, e) = trimmed(code, from, to);
    if s == e {
        return;
    }
    let op = &code[s..e];
    if op.eq_ignore_ascii_case("a") {
        push(out, TokenKind::Register, s, e);
        return;
    }
    let first = out.len();
    lex_expr(code, s, e, out);
    // `,X` or `,Y` at the end: the index register.
    let squeezed: String = op.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_ascii_uppercase();
    if (squeezed.ends_with(",X") || squeezed.ends_with(",Y"))
        && let Some(last) = out[first..].last_mut()
        && last.kind == TokenKind::Symbol
        && last.end - last.start == 1
    {
        last.kind = TokenKind::Register;
    }
}

/// The two-character operators, which only come after a value.
fn two_char(a: u8, b: Option<u8>) -> bool {
    matches!((a, b), (b'<', Some(b'<')) | (b'>', Some(b'>')) | (b'<', Some(b'=')) | (b'>', Some(b'=')) | (b'=', Some(b'=')) | (b'!', Some(b'=')) | (b'&', Some(b'&')) | (b'|', Some(b'|')))
}

/// An expression, or a directive's comma-separated expressions and strings. Where a value is
/// expected, `*` is the address and `%` starts a binary number; after one they're operators.
fn lex_expr(code: &str, from: usize, to: usize, out: &mut Vec<Token>) {
    let b = &code.as_bytes()[..to];
    let mut i = from;
    let mut want_value = true;
    while i < b.len() {
        let c = b[i];
        let start = i;
        let kind;
        let mut pc = false;
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        } else if c == b'"' {
            i = skip_string(b, i);
            kind = TokenKind::String;
        } else if c == b'\'' {
            match char_literal(b, i) {
                Some(e) => {
                    i = e;
                    kind = TokenKind::Number;
                }
                None => {
                    i += 1;
                    kind = TokenKind::Invalid;
                }
            }
        } else if c == b'$' || (c == b'%' && want_value) || c.is_ascii_digit() {
            let (radix, mut j) = match c {
                b'$' => (16, i + 1),
                b'%' => (2, i + 1),
                _ => (10, i),
            };
            while j < b.len() && (b[j] as char).is_digit(radix) {
                j += 1;
            }
            // `1x` is a bad number, not a number and a name.
            while j < b.len() && is_name_char(b[j]) {
                j += 1;
            }
            i = j;
            kind = TokenKind::Number;
        } else if is_name_start(c) || (c == b'\\' && b.get(i + 1).is_some_and(|&n| is_name_start(n))) {
            let mut j = i + 1;
            while j < b.len() && (is_name_char(b[j]) || (b[j] == b'\\' && b.get(j + 1).is_some_and(|&n| is_name_start(n)))) {
                j += 1;
            }
            let word = &code[i..j];
            let mut k = j;
            while k < b.len() && b[k].is_ascii_whitespace() {
                k += 1;
            }
            kind = if b.get(k) == Some(&b'(') && !word.starts_with('@') {
                if is_function(word) { TokenKind::Function } else { TokenKind::Invalid }
            } else if word.starts_with('@') {
                TokenKind::Local
            } else {
                TokenKind::Symbol
            };
            i = j;
        } else if c == b'*' && want_value {
            i += 1;
            pc = true;
            kind = TokenKind::Operator;
        } else if !want_value && two_char(c, b.get(i + 1).copied()) {
            i += 2;
            kind = TokenKind::Operator;
        } else if b"+-*/%&|^~<>[](),#=:\\!".contains(&c) {
            i += 1;
            kind = TokenKind::Operator;
        } else {
            // One whole character, even outside ASCII.
            i += code[i..].chars().next().map_or(1, char::len_utf8);
            kind = TokenKind::Invalid;
        }
        push(out, kind, start, i);
        want_value = match kind {
            TokenKind::Number | TokenKind::Symbol | TokenKind::Local | TokenKind::String => false,
            TokenKind::Operator => !(pc || matches!(b[start], b']' | b')')),
            _ => true,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(line: &str) -> Vec<(&str, &'static str)> {
        tokens(line).iter().map(|t| (t.text(line), t.kind.name())).collect()
    }

    #[test]
    fn a_full_line() {
        assert_eq!(
            kinds("loop:  LDA #$41        ; a comment"),
            vec![("loop", "label"), (":", "operator"), ("LDA", "mnemonic"), ("#", "operator"), ("$41", "number"), ("; a comment", "comment")]
        );
    }

    #[test]
    fn constants_locals_registers_functions() {
        assert_eq!(kinds("BORDER  = $C020"), vec![("BORDER", "constant"), ("=", "operator"), ("$C020", "number")]);
        assert_eq!(kinds("@col:   LDY wave,X"), vec![("@col", "local"), (":", "operator"), ("LDY", "mnemonic"), ("wave", "symbol"), (",", "operator"), ("X", "register")]);
        assert_eq!(kinds(" asl a"), vec![("asl", "mnemonic"), ("a", "register")]);
        assert_eq!(kinds(" STA (PTR),y"), vec![("STA", "mnemonic"), ("(", "operator"), ("PTR", "symbol"), (")", "operator"), (",", "operator"), ("y", "register")]);
        assert_eq!(
            kinds(" .byte 12 - SIN(col * 256 / 40, 12)"),
            vec![
                (".byte", "directive"),
                ("12", "number"),
                ("-", "operator"),
                ("SIN", "function"),
                ("(", "operator"),
                ("col", "symbol"),
                ("*", "operator"),
                ("256", "number"),
                ("/", "operator"),
                ("40", "number"),
                (",", "operator"),
                ("12", "number"),
                (")", "operator")
            ]
        );
        assert_eq!(kinds(" BNE @loop"), vec![("BNE", "mnemonic"), ("@loop", "local")]);
    }

    #[test]
    fn star_percent_and_two_char_operators_depend_on_place() {
        assert_eq!(kinds(" .word * - 2, 2 * *"), vec![(".word", "directive"), ("*", "operator"), ("-", "operator"), ("2", "number"), (",", "operator"), ("2", "number"), ("*", "operator"), ("*", "operator")]);
        assert_eq!(kinds("X1 = %0101 % 3"), vec![("X1", "constant"), ("=", "operator"), ("%0101", "number"), ("%", "operator"), ("3", "number")]);
        assert_eq!(kinds(" .assert [<B] == 0 && 1"), vec![(".assert", "directive"), ("[", "operator"), ("<", "operator"), ("B", "symbol"), ("]", "operator"), ("==", "operator"), ("0", "number"), ("&&", "operator"), ("1", "number")]);
    }

    #[test]
    fn strings_chars_and_semicolons_inside_them() {
        assert_eq!(kinds(" .text \"A;B\\\"\" ; c"), vec![(".text", "directive"), ("\"A;B\\\"\"", "string"), ("; c", "comment")]);
        assert_eq!(kinds(" LDA #';'"), vec![("LDA", "mnemonic"), ("#", "operator"), ("';'", "number")]);
        assert_eq!(kinds(" LDA #'\\''"), vec![("LDA", "mnemonic"), ("#", "operator"), ("'\\''", "number")]);
    }

    #[test]
    fn pasted_names_and_mistakes() {
        assert_eq!(kinds("ROW\\i:  .res 40"), vec![("ROW\\i", "label"), (":", "operator"), (".res", "directive"), ("40", "number")]);
        assert_eq!(kinds(" LDQ #1"), vec![("LDQ", "invalid"), ("#", "operator"), ("1", "number")]);
        assert_eq!(kinds(" .foo"), vec![(".foo", "invalid")]);
        assert_eq!(kinds(" .BYTE foo(1)"), vec![(".BYTE", "directive"), ("foo", "invalid"), ("(", "operator"), ("1", "number"), (")", "operator")]);
        assert!(kinds("").is_empty());
        assert!(kinds("   \t ").is_empty());
        assert_eq!(kinds("x:   "), vec![("x", "label"), (":", "operator")]);
        assert_eq!(kinds("  =  "), vec![("=", "invalid")]);
        assert_eq!(kinds("; only"), vec![("; only", "comment")]);
    }
}
