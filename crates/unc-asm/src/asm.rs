//! Two-pass assembler for the UNC 64. The full syntax is chapter 6 of the manual.
//!
//! Lines: `label:` definitions, `NAME = expr` constants, `;` comments, mnemonics in any case.
//! `@name` labels are local to the nearest global label before them.
//! Operands: `#expr`, `expr`, `expr,X`, `expr,Y`, `(expr)`, `(expr),Y`, `A`. A `(` that starts
//! an operand means indirect; anywhere else it must follow a function name.
//! Expressions: `$hex` and `%bin` (up to 64 bits), decimal, `'c'`, labels, `*` (current
//! address), `[ ]` for grouping, the functions `sin(a, n)`, `cos(a, n)`, `sqrt(x)`, `abs(x)`,
//! `min(a, b)`, `max(a, b)` (integer only, the same on every host), and C's operators and
//! precedence: unary `-` `~`; `* / %`; `+ -`; `<< >>`; `< <= > >=`; `== !=`; `&`; `^`; `|`;
//! `&&`; `||`. Comparisons give 1 or 0. Where a value is expected, `<` (low byte) and `>`
//! (high byte) apply to everything after them, so `<T+1` is the low byte of T+1; `*` is the
//! address and `%` starts a binary number. After a value they're operators: less, greater,
//! multiply, remainder. A bitwise operator with a comparison on one side only, or `<`/`>` over
//! a comparison, is a precedence trap: it assembles, with a warning.
//! Strings and character literals take the escapes `\"`, `\'` and `\\`.
//! Directives: `.org`, `.byte`, `.word`, `.text`, `.asciiz`, `.fill count[,value]`,
//! `.align n[,value]` (without a value it reserves the padding unless it's between stored
//! bytes), `.res n` (`.dsb`), `.rept n[,counter]` ... `.endr`, `.irp name, values...` ...
//! `.endr` (`\name` pastes the value into a name), `.include "file"`, `.assert expr[,"message"]`.
//! A plain operand that fits in a byte uses the fast-page mode when the instruction has one;
//! write the address with 3-4 hex digits (`$0012`) to force absolute.

use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use unc_core::isa::{Mode, Op, opcode_for};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsmError {
    pub line: usize,
    pub message: String,
    /// The included file the line is in; `None` for the main source.
    pub file: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub start: u16,
    pub bytes: Vec<u8>,
}

/// Where the bytes of one source line went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineInfo {
    pub line: usize,
    pub addr: u16,
    pub len: u16,
    /// The included file the line is in; `None` for the main source.
    pub file: Option<String>,
}

/// One line of the listing: every source line as assembled, `.rept` copies included.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListLine {
    pub file: Option<String>,
    pub line: usize,
    /// The address, for lines that emit bytes, reserve space or define a label.
    pub addr: Option<u16>,
    pub bytes: Vec<u8>,
    pub text: String,
    /// Something the reader should know, e.g. that a missing fast-page mode became absolute.
    pub note: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Assembly {
    pub segments: Vec<Segment>,
    pub symbols: Vec<(String, u16)>,
    pub lines: Vec<LineInfo>,
    pub listing: Vec<ListLine>,
    /// Lines that assembled but probably don't mean what they say, like `F & $FF == 0`
    /// (which is `F & [$FF == 0]`). Each is also a note in the listing.
    pub warnings: Vec<AsmError>,
}

impl Assembly {
    pub fn symbol(&self, name: &str) -> Option<u16> {
        self.symbols.iter().find(|(n, _)| n == name).map(|&(_, v)| v)
    }

    /// Where to start running: the `start` label if there is one, else the first byte emitted.
    pub fn start(&self) -> Option<u16> {
        self.symbol("start").or_else(|| self.segments.iter().find(|s| !s.bytes.is_empty()).map(|s| s.start))
    }

    /// A flat image of `from..=to`, gaps filled with `fill`. Bytes outside the range are dropped.
    pub fn image(&self, from: u16, to: u16, fill: u8) -> Vec<u8> {
        let mut out = vec![fill; (to - from) as usize + 1];
        for s in &self.segments {
            for (i, &b) in s.bytes.iter().enumerate() {
                let a = s.start as usize + i;
                if a >= from as usize && a <= to as usize {
                    out[a - from as usize] = b;
                }
            }
        }
        out
    }

    /// The symbol table as constant definitions, one per line, in address order.
    /// Local labels appear as `global@local`.
    pub fn symbol_file(&self) -> String {
        self.symbols.iter().map(|(n, v)| format!("{n} = ${v:04X}\n")).collect()
    }

    /// The listing as text: address, bytes, source line number, source text.
    /// Lines from included files show `file:line`. Notes follow the text after `; **`.
    pub fn listing_text(&self) -> String {
        let mut out = String::new();
        for l in &self.listing {
            let loc = match &l.file {
                Some(f) => format!("{f}:{}", l.line),
                None => l.line.to_string(),
            };
            let addr = l.addr.map(|a| format!("{a:04X}")).unwrap_or_else(|| "    ".into());
            let hex = |b: &[u8]| b.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ");
            let first = &l.bytes[..l.bytes.len().min(3)];
            let note = l.note.as_ref().map(|n| format!("  ; ** {n}")).unwrap_or_default();
            out.push_str(format!("{addr}  {:<8}  {loc:>5}  {}{note}", hex(first), l.text).trim_end());
            out.push('\n');
            let mut a = l.addr.unwrap_or(0) as usize + first.len();
            for chunk in l.bytes[first.len()..].chunks(8) {
                out.push_str(&format!("{a:04X}  {}\n", hex(chunk)));
                a += chunk.len();
            }
        }
        out
    }
}

#[derive(Debug, Clone)]
enum Operand {
    None,
    Acc,
    Imm(String),
    Plain(String),
    X(String),
    Y(String),
    Ind(String),
    IndY(String),
}

impl Operand {
    fn expr(&self) -> Option<&str> {
        match self {
            Operand::None | Operand::Acc => None,
            Operand::Imm(e) | Operand::Plain(e) | Operand::X(e) | Operand::Y(e) | Operand::Ind(e) | Operand::IndY(e) => Some(e),
        }
    }
}

#[derive(Debug, Clone)]
enum Body {
    Nothing,
    Const(String, String),
    Directive(String, String),
    Ins(Op, Operand),
}

impl Body {
    /// The expressions on the line, strings left out.
    fn exprs(&self) -> Vec<String> {
        match self {
            Body::Nothing => Vec::new(),
            Body::Const(_, e) => vec![e.clone()],
            Body::Directive(_, args) => split_items(args).into_iter().filter(|it| string_lit(it).is_none()).collect(),
            Body::Ins(_, operand) => operand.expr().map(str::to_string).into_iter().collect(),
        }
    }
}

/// A line of source as read: from the main source, an included file, or a `.rept` copy.
#[derive(Debug, Clone)]
struct Src {
    file: Option<Rc<str>>,
    no: usize,
    text: String,
    depth: u8,
}

impl Src {
    fn error(&self, message: String) -> AsmError {
        AsmError { line: self.no, message, file: self.file.as_deref().map(str::to_string) }
    }
}

#[derive(Debug, Clone)]
struct Line {
    src: Src,
    label: Option<String>,
    body: Body,
}

/// Every character of `s` with its byte offset, and whether it's inside a string or a
/// character literal (the quotes count as inside). Backslash escapes the next character.
fn quoted(s: &str) -> Vec<(usize, char, bool)> {
    let mut out = Vec::with_capacity(s.len());
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (i, c) in s.char_indices() {
        let was = quote.is_some();
        match quote {
            None if c == '"' || c == '\'' => quote = Some(c),
            Some(_) if escaped => escaped = false,
            Some(_) if c == '\\' => escaped = true,
            Some(q) if c == q => quote = None,
            _ => {}
        }
        out.push((i, c, was || quote.is_some()));
    }
    out
}

fn strip_comment(s: &str) -> &str {
    match quoted(s).into_iter().find(|&(_, c, q)| c == ';' && !q) {
        Some((i, _, _)) => &s[..i],
        None => s,
    }
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '@'
}

fn is_ident(s: &str) -> bool {
    let mut cs = s.chars();
    matches!(cs.next(), Some(c) if c.is_ascii_alphabetic() || c == '_' || c == '@') && cs.all(is_ident_char)
}

/// `label:` at the start of a line, and the rest.
fn split_label(code: &str) -> (Option<&str>, &str) {
    if let Some(colon) = code.find(':') {
        let name = code[..colon].trim();
        if is_ident(name) {
            return (Some(name), code[colon + 1..].trim());
        }
    }
    (None, code)
}

/// The directive a raw line uses, lower case, without the dot.
fn directive_word(raw: &str) -> Option<String> {
    let code = strip_comment(raw).trim();
    let (_, rest) = split_label(code);
    rest.split_whitespace().next()?.strip_prefix('.').map(str::to_ascii_lowercase)
}

/// Rewrites each `@name` outside strings as `scope@name`.
fn mangle_locals(code: &str, scope: &str) -> String {
    let mut out = String::with_capacity(code.len() + 16);
    let mut prev = ' ';
    for (_, c, q) in quoted(code) {
        if c == '@' && !q && !is_ident_char(prev) {
            out.push_str(scope);
        }
        out.push(c);
        prev = c;
    }
    out
}

/// A `.rept` counter or `.irp` name and what it stands for in one copy of the block.
struct Subst<'a> {
    name: &'a str,
    /// What `\name` pastes into a name: the value exactly as written.
    paste: &'a str,
    /// What the name becomes elsewhere: the value, in `[ ]` unless it's a single name, number
    /// or string, so `v * 2` with `v` = `1 + 2` is 6.
    value: String,
}

impl<'a> Subst<'a> {
    fn new(name: &'a str, value: &'a str) -> Self {
        let single = string_lit(value).is_some() || matches!(tokenize(value).as_deref(), Ok([_]));
        Subst { name, paste: value, value: if single { value.to_string() } else { format!("[{value}]") } }
    }

    /// Replaces the name outside strings and comments: as a whole word with the value, and
    /// after a `\` by pasting it into the word before.
    fn apply(&self, text: &str) -> Result<String, String> {
        let chars = quoted(text);
        let mut out = String::with_capacity(text.len());
        let mut i = 0;
        while i < chars.len() {
            let (at, c, q) = chars[i];
            if !q && c == ';' {
                out.push_str(&text[at..]);
                break;
            }
            if !q && is_ident_char(c) {
                let mut j = i;
                while j < chars.len() && !chars[j].2 && is_ident_char(chars[j].1) {
                    j += 1;
                }
                let word: String = chars[i..j].iter().map(|x| x.1).collect();
                let prev = if i > 0 { chars[i - 1].1 } else { ' ' };
                if word != self.name || "$%.".contains(prev) {
                    out.push_str(&word);
                } else if prev == '\\' {
                    if self.paste.is_empty() || !self.paste.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                        return Err(format!("can't paste '{}' into a name", self.paste));
                    }
                    out.pop();
                    out.push_str(self.paste);
                } else {
                    out.push_str(&self.value);
                }
                i = j;
            } else {
                out.push(c);
                i += 1;
            }
        }
        Ok(out)
    }
}

fn parse_operand(s: &str) -> Operand {
    let s = s.trim();
    let up = s.to_ascii_uppercase();
    let up = up.replace(' ', "");
    if s.is_empty() {
        Operand::None
    } else if up == "A" {
        Operand::Acc
    } else if let Some(rest) = s.strip_prefix('#') {
        Operand::Imm(rest.trim().to_string())
    } else if s.starts_with('(') && up.ends_with("),Y") {
        let close = s.rfind(')').unwrap_or(s.len());
        Operand::IndY(s[1..close].trim().to_string())
    } else if s.starts_with('(') && s.ends_with(')') {
        Operand::Ind(s[1..s.len() - 1].trim().to_string())
    } else if up.ends_with(",X") {
        Operand::X(s[..s.rfind(',').unwrap()].trim().to_string())
    } else if up.ends_with(",Y") {
        Operand::Y(s[..s.rfind(',').unwrap()].trim().to_string())
    } else {
        Operand::Plain(s.to_string())
    }
}

/// Parses a comment-free line: an optional label and what follows it.
fn parse_line(s: &str) -> Result<(Option<String>, Body), String> {
    if let Some(eq) = s.find('=') {
        let name = s[..eq].trim();
        if is_ident(name) {
            return Ok((None, Body::Const(name.to_string(), s[eq + 1..].trim().to_string())));
        }
    }
    let (label, s) = split_label(s);
    let label = label.map(str::to_string);
    if let (Some(l), Some(eq)) = (&label, s.find('='))
        && is_ident(s[..eq].trim())
    {
        let name = s[..eq].trim();
        return Err(format!("a constant can't have a label: put '{name} = ...' on its own line, without '{l}:'"));
    }
    if s.is_empty() {
        return Ok((label, Body::Nothing));
    }
    let (word, rest) = match s.find(char::is_whitespace) {
        Some(i) => (&s[..i], s[i..].trim()),
        None => (s, ""),
    };
    let body = if let Some(d) = word.strip_prefix('.') {
        Body::Directive(d.to_ascii_lowercase(), rest.to_string())
    } else if let Some(op) = Op::from_mnemonic(word) {
        Body::Ins(op, parse_operand(rest))
    } else {
        return Err(format!("unknown instruction '{word}'"));
    };
    Ok((label, body))
}

// ---------- expressions ----------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(i64),
    Ident(String),
    /// `*` where a value is expected: the current address.
    Pc,
    /// A one-character operator. After a value, `<` and `>` compare; before one, they take
    /// the low and high byte.
    Sym(char),
    Shl,
    Shr,
    Le,
    Ge,
    Eq,
    Ne,
    AndAnd,
    OrOr,
}

fn unescape(c: char) -> Result<char, String> {
    match c {
        '"' | '\'' | '\\' => Ok(c),
        _ => Err(format!("unknown escape '\\{c}'")),
    }
}

/// The two-character operators, which only come after a value.
fn two_char(c: char, next: Option<char>) -> Option<Tok> {
    Some(match (c, next?) {
        ('<', '<') => Tok::Shl,
        ('>', '>') => Tok::Shr,
        ('<', '=') => Tok::Le,
        ('>', '=') => Tok::Ge,
        ('=', '=') => Tok::Eq,
        ('!', '=') => Tok::Ne,
        ('&', '&') => Tok::AndAnd,
        ('|', '|') => Tok::OrOr,
        _ => return None,
    })
}

fn tokenize(s: &str) -> Result<Vec<Tok>, String> {
    let cs: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    // True where a value is expected: there `*` is the address and `%` starts a binary number;
    // after a value they're multiply and remainder.
    let mut want_value = true;
    while i < cs.len() {
        let c = cs[i];
        let next = cs.get(i + 1).copied();
        let tok = if c.is_whitespace() {
            i += 1;
            continue;
        } else if c == '$' || (c == '%' && want_value) || c.is_ascii_digit() {
            let (radix, start) = match c {
                '$' => (16, i + 1),
                '%' => (2, i + 1),
                _ => (10, i),
            };
            let mut j = start;
            while j < cs.len() && cs[j].is_digit(radix) {
                j += 1;
            }
            let digits: String = cs[start..j].iter().collect();
            let bad = || format!("bad number '{}'", cs[i..j].iter().collect::<String>());
            // Hex and binary numbers may use all 64 bits; past $7FFF_FFFF_FFFF_FFFF they're negative.
            let v = if radix == 10 {
                i64::from_str_radix(&digits, radix).map_err(|_| bad())?
            } else {
                u64::from_str_radix(&digits, radix).map_err(|_| bad())? as i64
            };
            i = j;
            Tok::Num(v)
        } else if c == '\'' {
            match cs[i + 1..] {
                ['\\', e, '\'', ..] => {
                    i += 4;
                    Tok::Num(unescape(e)? as i64)
                }
                [ch, '\'', ..] if ch != '\\' => {
                    i += 3;
                    Tok::Num(ch as i64)
                }
                _ => return Err("bad character literal".into()),
            }
        } else if c.is_ascii_alphabetic() || c == '_' || c == '@' {
            let mut j = i;
            while j < cs.len() && is_ident_char(cs[j]) {
                j += 1;
            }
            let name = cs[i..j].iter().collect();
            i = j;
            Tok::Ident(name)
        } else if c == '*' && want_value {
            i += 1;
            Tok::Pc
        } else if let Some(t) = (!want_value).then(|| two_char(c, next)).flatten() {
            i += 2;
            t
        } else if "+-*/%&|^~<>[](),".contains(c) {
            i += 1;
            Tok::Sym(c)
        } else {
            return Err(format!("unexpected '{c}' in expression"));
        };
        want_value = !matches!(tok, Tok::Num(_) | Tok::Ident(_) | Tok::Pc | Tok::Sym(']' | ')'));
        out.push(tok);
    }
    Ok(out)
}

struct Eval<'a> {
    toks: Vec<Tok>,
    i: usize,
    syms: &'a HashMap<String, i64>,
    pc: i64,
    /// The first precedence trap seen, for the listing and the warnings.
    warn: Option<String>,
}

type V = Option<i64>;

/// A value, and if it's an unbracketed comparison (or `&&`, `||`, or a bitwise mix of
/// comparisons), the operator that made it: so a 0-or-1, and a precedence trap if a bitwise
/// operator or `<` `>` takes it whole.
type Val = (V, Option<&'static str>);

/// Binary operators from loosest to tightest, as in C.
const LEVELS: &[&[Tok]] = &[
    &[Tok::OrOr],
    &[Tok::AndAnd],
    &[Tok::Sym('|')],
    &[Tok::Sym('^')],
    &[Tok::Sym('&')],
    &[Tok::Eq, Tok::Ne],
    &[Tok::Sym('<'), Tok::Le, Tok::Sym('>'), Tok::Ge],
    &[Tok::Shl, Tok::Shr],
    &[Tok::Sym('+'), Tok::Sym('-')],
    &[Tok::Sym('*'), Tok::Sym('/'), Tok::Sym('%')],
];

fn apply(op: &Tok, a: i64, b: i64) -> Result<i64, String> {
    Ok(match op {
        Tok::OrOr => (a != 0 || b != 0) as i64,
        Tok::AndAnd => (a != 0 && b != 0) as i64,
        Tok::Sym('|') => a | b,
        Tok::Sym('^') => a ^ b,
        Tok::Sym('&') => a & b,
        Tok::Eq => (a == b) as i64,
        Tok::Ne => (a != b) as i64,
        Tok::Sym('<') => (a < b) as i64,
        Tok::Le => (a <= b) as i64,
        Tok::Sym('>') => (a > b) as i64,
        Tok::Ge => (a >= b) as i64,
        Tok::Shl | Tok::Shr if !(0..64).contains(&b) => return Err(format!("bad shift count {b}")),
        Tok::Shl => a.wrapping_shl(b as u32),
        Tok::Shr => a >> b,
        Tok::Sym('+') => a.wrapping_add(b),
        Tok::Sym('-') => a.wrapping_sub(b),
        Tok::Sym('*') => a.wrapping_mul(b),
        Tok::Sym('/' | '%') if b == 0 => return Err("division by zero".into()),
        Tok::Sym('/') => a.wrapping_div(b),
        Tok::Sym('%') => a.wrapping_rem(b),
        _ => unreachable!("not a binary operator"),
    })
}

fn describe(t: &Tok) -> String {
    match t {
        Tok::Num(n) => n.to_string(),
        Tok::Ident(n) => n.clone(),
        Tok::Pc => "*".into(),
        Tok::Sym(c) => c.to_string(),
        Tok::Shl => "<<".into(),
        Tok::Shr => ">>".into(),
        Tok::Le => "<=".into(),
        Tok::Ge => ">=".into(),
        Tok::Eq => "==".into(),
        Tok::Ne => "!=".into(),
        Tok::AndAnd => "&&".into(),
        Tok::OrOr => "||".into(),
    }
}

/// The operator's text, for messages about comparisons and bitwise operators.
fn op_name(t: &Tok) -> &'static str {
    match t {
        Tok::Eq => "==",
        Tok::Ne => "!=",
        Tok::Sym('<') => "<",
        Tok::Le => "<=",
        Tok::Sym('>') => ">",
        Tok::Ge => ">=",
        Tok::AndAnd => "&&",
        Tok::OrOr => "||",
        Tok::Sym('&') => "&",
        Tok::Sym('|') => "|",
        Tok::Sym('^') => "^",
        _ => "",
    }
}

/// The functions an expression can call: name, how many values, and how to write it.
pub(crate) const FUNCTIONS: &[(&str, usize, &str)] = &[
    ("sin", 2, "sin(angle, size)"),
    ("cos", 2, "cos(angle, size)"),
    ("sqrt", 1, "sqrt(x)"),
    ("abs", 1, "abs(x)"),
    ("min", 2, "min(a, b)"),
    ("max", 2, "max(a, b)"),
];

/// sin(2πk/256) for k = 0..=64 (a quarter circle), times 2^62, rounded to the nearest
/// integer. Computed to 200 bits; `sin_table_is_exact` in the tests checks it.
#[rustfmt::skip]
const SINE: [u64; 65] = [
    0x0000000000000000, 0x0192155F7A3667E0, 0x0323ECBE21BB027D, 0x04B54824B3867D73,
    0x0645E9AF0A6D0AF8, 0x07D59395AA5CC38D, 0x0964083747309D11, 0x0AF10A22459FE32A,
    0x0C7C5C1E34D3055B, 0x0E05C1353F27B17E, 0x0F8CFCBD90AF8D58, 0x1111D262B1F67761,
    0x1294062ED59F05A9, 0x14135C9417660143, 0x158F9A75AB1FDCFE, 0x17088530FA459EAF,
    0x187DE2A6AEA962D2, 0x19EF7943A8ED8A2E, 0x1B5D1009E15CC02B, 0x1CC66E9931C45E17,
    0x1E2B5D3806F63B1E, 0x1F8BA4DBF89AB9FB, 0x20E70F3245FFDB2D, 0x223D66A836964508,
    0x238E76735CD190D9, 0x24DA0A99BA25BD51, 0x261FEFF9C2E069C2, 0x275FF45240A17279,
    0x2899E64A123BAC30, 0x29CD9577C7CBD228, 0x2AFAD26919D93F45, 0x2C216EAA3A59BDB7,
    0x2D413CCCFE779921, 0x2E5A106FDFFF2C87, 0x2F6BBE44D55F5DBC, 0x30761C17FF2EDBA4,
    0x317900D62A2E816A, 0x3274449324C7F69F, 0x3367C08FE70E8168, 0x34534F408C4F03BB,
    0x3536CC521D434606, 0x361214B02A03FF37, 0x36E5068A32DC7B22, 0x37AF8158DF2A533F,
    0x387165E3017B61A4, 0x392A96426823E9ED, 0x39DAF5E8798EE5E2, 0x3A8269A29B927359,
    0x3B20D79E651A8C51, 0x3BB6276D998478C2, 0x3C424209ED0DC97F, 0x3CC511D891C223DD,
    0x3D3E82AD8C5BB4BB, 0x3DAE81CED092C67A, 0x3E14FDF72461AE55, 0x3E71E758C9CB118A,
    0x3EC52F9FEEB96056, 0x3F0EC9F4E297526B, 0x3F4EAAFE114A2D43, 0x3F84C8E1C33FA68F,
    0x3FB11B47A24A4B3C, 0x3FD39B5A0310742A, 0x3FEC43C6F2DAFBC7, 0x3FFB10C1099A1976,
    0x4000000000000000,
];

/// round(size * sin(2π * angle / 256)), halves away from zero, in integers only, so every
/// host gives the same answer. Exact (the true value, rounded) for every angle and every
/// |size| up to 2^20; for bigger sizes it can be one off when the value is within
/// size/2^63 of a half.
pub fn int_sin(angle: i64, size: i64) -> i64 {
    let a = angle.rem_euclid(256) as usize;
    let (k, negative) = if a < 128 { (a, false) } else { (a - 128, true) };
    let k = if k > 64 { 128 - k } else { k };
    let r = ((size.unsigned_abs() as u128 * SINE[k] as u128 + (1 << 61)) >> 62) as i128;
    (if negative != (size < 0) { -r } else { r }) as i64
}

fn call(name: &str, args: &[i64]) -> Result<i64, String> {
    Ok(match (name, args) {
        ("sin", &[a, n]) => int_sin(a, n),
        ("cos", &[a, n]) => int_sin(a.wrapping_add(64), n),
        ("sqrt", &[x]) if x < 0 => return Err(format!("sqrt of a negative number ({x})")),
        ("sqrt", &[x]) => (x as u64).isqrt() as i64,
        ("abs", &[x]) => x.wrapping_abs(),
        ("min", &[a, b]) => a.min(b),
        ("max", &[a, b]) => a.max(b),
        _ => unreachable!("the argument count was checked"),
    })
}

impl Eval<'_> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.i)
    }

    fn warn(&mut self, message: String) {
        self.warn.get_or_insert(message);
    }

    /// A whole expression: `<` and `>` take everything after them.
    fn expr(&mut self) -> Result<Val, String> {
        self.binary(0)
    }

    fn binary(&mut self, level: usize) -> Result<Val, String> {
        if level == LEVELS.len() {
            return Ok((self.unary()?, None));
        }
        let (mut v, mut cmp) = self.binary(level + 1)?;
        while let Some(op) = self.peek().filter(|t| LEVELS[level].contains(t)).cloned() {
            self.i += 1;
            let (r, rcmp) = self.binary(level + 1)?;
            v = match (v, r) {
                (Some(a), Some(b)) => Some(apply(&op, a, b)?),
                _ => None,
            };
            cmp = match op {
                Tok::Sym('&' | '|' | '^') => match (cmp, rcmp) {
                    // Two comparisons: a logical AND or OR, as meant.
                    (Some(c), Some(_)) => Some(c),
                    (None, None) => None,
                    (Some(c), None) | (None, Some(c)) => {
                        self.warn(format!("'{c}' binds tighter than '{}': add [ ] to say which you mean", op_name(&op)));
                        None
                    }
                },
                _ => Some(op_name(&op)).filter(|n| !n.is_empty()),
            };
        }
        Ok((v, cmp))
    }

    /// `<expr` (`high` false) or `>expr`: a byte of everything after it.
    fn byte_of(&mut self, high: bool) -> Result<V, String> {
        self.i += 1;
        let (v, cmp) = self.expr()?;
        if cmp.is_some() {
            let (sign, which) = if high { ('>', "high") } else { ('<', "low") };
            self.warn(format!("'{sign}' takes the {which} byte of the whole comparison: add [ ] to say which you mean"));
        }
        Ok(v.map(|v| if high { (v >> 8) & 0xFF } else { v & 0xFF }))
    }

    fn unary(&mut self) -> Result<V, String> {
        match self.peek().cloned() {
            Some(Tok::Sym('<')) => self.byte_of(false),
            Some(Tok::Sym('>')) => self.byte_of(true),
            Some(Tok::Sym('-')) => {
                self.i += 1;
                Ok(self.unary()?.map(|v| v.wrapping_neg()))
            }
            Some(Tok::Sym('~')) => {
                self.i += 1;
                Ok(self.unary()?.map(|v| !v))
            }
            _ => self.atom(),
        }
    }

    /// `name(value, ...)`, with the `(` next.
    fn call(&mut self, name: &str) -> Result<V, String> {
        let lower = name.to_ascii_lowercase();
        let &(name, want, usage) = FUNCTIONS.iter().find(|f| f.0 == lower).ok_or_else(|| format!("unknown function '{name}'"))?;
        self.i += 1;
        let mut args = Vec::new();
        loop {
            args.push(self.expr()?.0);
            match self.peek() {
                Some(Tok::Sym(',')) => self.i += 1,
                Some(Tok::Sym(')')) => break,
                _ => return Err("missing ')'".into()),
            }
        }
        self.i += 1;
        if args.len() != want {
            let s = if want == 1 { "" } else { "s" };
            return Err(format!("{name} takes {want} value{s}: {usage}"));
        }
        match args.into_iter().collect::<Option<Vec<i64>>>() {
            Some(a) => call(name, &a).map(Some),
            None => Ok(None),
        }
    }

    fn atom(&mut self) -> Result<V, String> {
        let t = self.peek().cloned();
        self.i += 1;
        match t {
            Some(Tok::Num(n)) => Ok(Some(n)),
            Some(Tok::Ident(name)) if self.peek() == Some(&Tok::Sym('(')) => self.call(&name),
            Some(Tok::Ident(name)) => Ok(self.syms.get(&name).copied()),
            Some(Tok::Pc) => Ok(Some(self.pc)),
            Some(Tok::Sym('[')) => {
                let (v, _) = self.expr()?;
                if self.peek() != Some(&Tok::Sym(']')) {
                    return Err("missing ']'".into());
                }
                self.i += 1;
                Ok(v)
            }
            Some(Tok::Sym('(')) => Err("round brackets are for indirect addressing and functions: group with [ ]".into()),
            Some(t) => Err(format!("unexpected '{}' in expression", describe(&t))),
            None => Err("missing value".into()),
        }
    }
}

/// `Ok(None)` means the expression uses a symbol that isn't defined yet.
fn eval(s: &str, syms: &HashMap<String, i64>, pc: i64) -> Result<V, String> {
    let toks = tokenize(s)?;
    if toks.is_empty() {
        return Err("missing value".into());
    }
    let mut e = Eval { toks, i: 0, syms, pc, warn: None };
    let (v, _) = e.expr()?;
    match e.peek() {
        None => Ok(v),
        Some(t @ Tok::Sym(',' | ')')) => Err(format!("unexpected '{}' in expression", describe(t))),
        Some(_) => Err(format!("unexpected text in '{s}'")),
    }
}

/// The precedence trap in an expression, if it has one. Only the shape matters, not the values.
fn lint(s: &str) -> Option<String> {
    let toks = tokenize(s).ok()?;
    let mut e = Eval { toks, i: 0, syms: &HashMap::new(), pc: 0, warn: None };
    let _ = e.expr();
    e.warn
}

/// "'name' is defined twice", with a local label as `'@name' ... (after 'global')`.
fn defined_twice(name: &str) -> String {
    match name.find('@') {
        Some(at) if at > 0 => format!("'{}' is defined twice (after '{}')", &name[at..], &name[..at]),
        _ => format!("'{name}' is defined twice"),
    }
}

fn undefined_in(s: &str, syms: &HashMap<String, i64>) -> String {
    let missing = tokenize(s)
        .ok()
        .and_then(|t| t.into_iter().find_map(|t| match t { Tok::Ident(n) if !syms.contains_key(&n) => Some(n), _ => None }));
    match missing {
        Some(n) => match n.find('@') {
            Some(0) => format!("undefined local label '{n}' (there's no global label before it)"),
            Some(at) => format!("undefined local label '{}' (after '{}')", &n[at..], &n[..at]),
            None => format!("undefined symbol '{n}'"),
        },
        None => format!("undefined symbol '{s}'"),
    }
}

fn need(s: &str, syms: &HashMap<String, i64>, pc: i64) -> Result<i64, String> {
    eval(s, syms, pc)?.ok_or_else(|| undefined_in(s, syms))
}

fn forces_abs(e: &str) -> bool {
    let e = e.trim();
    e.starts_with('$') && e.len() >= 4 && e[1..].chars().all(|c| c.is_ascii_hexdigit())
}

fn choose_mode(op: Op, operand: &Operand, val: V) -> Result<Mode, String> {
    let has = |m| opcode_for(op, m).is_some();
    let small = |e: &str| !forces_abs(e) && matches!(val, Some(v) if (0..=255).contains(&v));
    let name = op.mnemonic();
    let pick = |fp: Mode, abs: Mode, e: &str| {
        if (small(e) || !has(abs)) && has(fp) {
            Ok(fp)
        } else if has(abs) {
            Ok(abs)
        } else {
            Err(format!("{name} has no {} mode", abs.name()))
        }
    };
    match operand {
        Operand::None if has(Mode::Imp) => Ok(Mode::Imp),
        Operand::None if has(Mode::Acc) => Ok(Mode::Acc),
        Operand::None => Err(format!("{name} needs an operand")),
        Operand::Acc if has(Mode::Acc) => Ok(Mode::Acc),
        Operand::Acc => Err(format!("{name} can't operate on A")),
        Operand::Imm(_) if has(Mode::Imm) => Ok(Mode::Imm),
        Operand::Imm(_) => Err(format!("{name} has no immediate mode")),
        Operand::Plain(_) if op.is_branch() => Ok(Mode::Rel),
        Operand::Plain(e) => pick(Mode::Fp, Mode::Abs, e),
        Operand::X(e) => pick(Mode::Fpx, Mode::Absx, e),
        Operand::Y(e) => pick(Mode::Fpy, Mode::Absy, e),
        Operand::Ind(_) if has(Mode::Ind) => Ok(Mode::Ind),
        Operand::Ind(_) => Err(format!("{name} has no indirect mode")),
        Operand::IndY(_) if has(Mode::Indy) => Ok(Mode::Indy),
        Operand::IndY(_) => Err(format!("{name} has no (fast page),Y mode")),
    }
}

/// Why an operand that fits the fast page was assembled as absolute, for the listing.
fn absolute_note(op: Op, mode: Mode, operand: &Operand, v: i64) -> Option<String> {
    let fp = match mode {
        Mode::Abs => Mode::Fp,
        Mode::Absx => Mode::Fpx,
        Mode::Absy => Mode::Fpy,
        _ => return None,
    };
    let e = operand.expr()?;
    if forces_abs(e) || !(0..=255).contains(&v) {
        return None;
    }
    Some(if opcode_for(op, fp).is_none() {
        format!("{} has no {} mode: {}", op.mnemonic(), fp.name(), mode.name())
    } else {
        format!("defined later: {}", mode.name())
    })
}

fn split_items(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0;
    for (_, c, q) in quoted(s) {
        match c {
            '(' | '[' if !q => depth += 1,
            ')' | ']' if !q => depth -= 1,
            _ => {}
        }
        if c == ',' && !q && depth <= 0 {
            out.push(cur.trim().to_string());
            cur.clear();
        } else {
            cur.push(c);
        }
    }
    if !cur.trim().is_empty() || !out.is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

/// The bytes of a `"string"`, escapes applied. `None` if `s` isn't one quoted string.
fn string_lit(s: &str) -> Option<Result<Vec<u8>, String>> {
    let body = s.strip_prefix('"')?;
    let mut out = Vec::new();
    let mut cs = body.char_indices();
    while let Some((i, c)) = cs.next() {
        match c {
            '"' => return (i + 1 == body.len()).then_some(Ok(out)),
            '\\' => match cs.next() {
                Some((_, e)) => match unescape(e) {
                    Ok(e) => out.push(e as u8),
                    Err(m) => return Some(Err(m)),
                },
                None => return None,
            },
            c => {
                let mut buf = [0; 4];
                out.extend(c.encode_utf8(&mut buf).bytes());
            }
        }
    }
    None
}

/// Every directive the assembler knows, aliases included, lower case and without the dot.
/// `lang` checks it against the manual's table, and a test checks the assembler takes each.
pub(crate) const DIRECTIVE_NAMES: &[&str] = &[
    "org", "byte", "db", "text", "ascii", "asciiz", "word", "dw", "fill", "align", "res", "dsb", "rept", "irp", "endr", "include", "assert",
];

/// What a directive does to the address.
enum DirEffect {
    /// Emits this many bytes.
    Size(i64),
    /// Moves the address.
    Org(i64),
    /// Skips this many bytes without writing them.
    Reserve(i64),
}

fn count(args: &str, syms: &HashMap<String, i64>, pc: i64, what: &str) -> Result<i64, String> {
    let n = need(args, syms, pc)?;
    if !(0..=65536).contains(&n) {
        return Err(format!("bad {what} count {n}"));
    }
    Ok(n)
}

fn directive(name: &str, args: &str, syms: &HashMap<String, i64>, pc: i64, out: Option<&mut Vec<u8>>) -> Result<DirEffect, String> {
    let items = split_items(args);
    let emit = out.is_some();
    let first = items.first().map(String::as_str).unwrap_or("");
    let mut bytes = Vec::new();
    match name {
        "org" => return Ok(DirEffect::Org(need(args, syms, pc)?)),
        "res" | "dsb" => return Ok(DirEffect::Reserve(count(args, syms, pc, "reserve")?)),
        // Checked in pass 2, when every symbol is known.
        "assert" if emit => {
            if need(first, syms, pc)? == 0 {
                return Err(match items.get(1).and_then(|m| string_lit(m)) {
                    Some(m) => String::from_utf8_lossy(&m?).into_owned(),
                    None => format!("assertion failed: {first}"),
                });
            }
        }
        "assert" => {}
        "byte" | "db" | "text" | "ascii" | "asciiz" => {
            for it in &items {
                if let Some(text) = string_lit(it) {
                    bytes.extend(text?);
                } else if emit {
                    let v = need(it, syms, pc)?;
                    if !(-128..=255).contains(&v) {
                        return Err(format!("{v} doesn't fit in a byte"));
                    }
                    bytes.push(v as u8);
                } else {
                    bytes.push(0);
                }
            }
            if name == "asciiz" {
                bytes.push(0);
            }
        }
        "word" | "dw" => {
            for it in &items {
                let v = if emit { need(it, syms, pc)? } else { 0 };
                if !(-32768..=65535).contains(&v) {
                    return Err(format!("{v} doesn't fit in a word"));
                }
                bytes.extend((v as u16).to_le_bytes());
            }
        }
        "fill" | "align" => {
            let n = count(first, syms, pc, name)?;
            let len = if name == "fill" {
                n
            } else if n == 0 {
                return Err("bad align count 0".into());
            } else {
                (n - pc.rem_euclid(n)) % n
            };
            let value = match (emit, items.get(1)) {
                (true, Some(v)) => need(v, syms, pc)? as u8,
                _ => 0,
            };
            bytes.resize(len as usize, value);
        }
        _ => return Err(format!("unknown directive '.{name}'")),
    }
    let n = bytes.len() as i64;
    if let Some(o) = out {
        o.extend(bytes);
    }
    Ok(DirEffect::Size(n))
}

fn encode(op: Op, mode: Mode, operand: &Operand, syms: &HashMap<String, i64>, pc: i64) -> Result<(Vec<u8>, Option<String>), String> {
    let code = opcode_for(op, mode).expect("mode was chosen from the table");
    let mut out = vec![code];
    let Some(e) = operand.expr() else { return Ok((out, None)) };
    let v = need(e, syms, pc)?;
    match mode {
        Mode::Imm => {
            if !(-128..=255).contains(&v) {
                return Err(format!("#{v} doesn't fit in a byte"));
            }
            out.push(v as u8);
        }
        Mode::Fp | Mode::Fpx | Mode::Fpy | Mode::Indy => {
            if !(0..=255).contains(&v) {
                return Err(format!("${v:X} is outside the fast page"));
            }
            out.push(v as u8);
        }
        Mode::Rel => {
            let off = v - (pc + 2);
            if !(-128..=127).contains(&off) {
                return Err(format!("branch out of range ({off} bytes)"));
            }
            out.push(off as i8 as u8);
        }
        _ => {
            if !(0..=0xFFFF).contains(&v) {
                return Err(format!("${v:X} is outside memory"));
            }
            out.extend((v as u16).to_le_bytes());
        }
    }
    Ok((out, absolute_note(op, mode, operand, v)))
}

pub const DEFAULT_ORIGIN: u16 = 0x0800;
/// How deep `.include`s may nest.
pub const MAX_INCLUDE_DEPTH: u8 = 16;
/// A program may expand (`.include`, `.rept`) to at most this many lines.
pub const MAX_LINES: usize = 1_000_000;

/// Assembles a program that has no `.include`s.
pub fn assemble(src: &str) -> Result<Assembly, Vec<AsmError>> {
    assemble_inner(src, None)
}

/// Assembles a program, reading each `.include "name"` through `resolve`, which returns the
/// file's text or `None` if it can't be read.
pub fn assemble_with(src: &str, resolve: impl Fn(&str) -> Option<String>) -> Result<Assembly, Vec<AsmError>> {
    assemble_inner(src, Some(&resolve))
}

fn lines_of(text: &str, file: Option<Rc<str>>, depth: u8) -> impl Iterator<Item = Src> + '_ {
    text.lines().enumerate().map(move |(i, t)| Src { file: file.clone(), no: i + 1, text: t.to_string(), depth })
}

/// Pops a `.rept` or `.irp` body off the queue, up to its `.endr`. `None` if there's no `.endr`.
fn take_block(queue: &mut VecDeque<Src>) -> Option<Vec<Src>> {
    let mut depth = 1;
    let mut body = Vec::new();
    while let Some(s) = queue.pop_front() {
        match directive_word(&s.text).as_deref() {
            Some("rept" | "irp") => depth += 1,
            Some("endr") => {
                depth -= 1;
                if depth == 0 {
                    return Some(body);
                }
            }
            _ => {}
        }
        body.push(s);
    }
    None
}

fn prepend(queue: &mut VecDeque<Src>, lines: Vec<Src>) {
    for l in lines.into_iter().rev() {
        queue.push_front(l);
    }
}

/// How a line moves through memory, to decide where an `.align` stores its padding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Span {
    /// Doesn't move the address.
    Still,
    /// Stores bytes.
    Stores,
    /// Moves without storing: `.org` and `.res`.
    Gap,
    /// An `.align` without a value, which pads a gap or stores zeros between stored bytes.
    Align,
}

type Resolver<'a> = Option<&'a dyn Fn(&str) -> Option<String>>;

/// Expands `.include` and `.rept` at `s`, putting the lines they make at the front of the queue.
fn expand(kind: &str, args: &str, s: &Src, queue: &mut VecDeque<Src>, resolve: Resolver, syms: &HashMap<String, i64>, pc: i64) -> Result<(), String> {
    match kind {
        "include" => {
            let name = match string_lit(args.trim()) {
                Some(Ok(b)) => String::from_utf8_lossy(&b).into_owned(),
                _ => return Err("'.include' needs a file name in quotes".into()),
            };
            let Some(resolve) = resolve else { return Err("includes aren't available here".into()) };
            if s.depth >= MAX_INCLUDE_DEPTH {
                return Err(format!("includes nest too deep at '{name}' (does it include itself?)"));
            }
            let text = resolve(&name).ok_or_else(|| format!("can't read '{name}'"))?;
            let file: Rc<str> = Rc::from(name.as_str());
            prepend(queue, lines_of(&text, Some(file), s.depth + 1).collect());
            Ok(())
        }
        "rept" | "irp" => {
            let items = split_items(args);
            let body = take_block(queue).ok_or_else(|| format!("'.{kind}' without '.endr'"))?;
            let first = items.first().map(String::as_str).unwrap_or("");
            // The values of the name, one per copy; `None` for a `.rept` without a counter.
            let (name, values): (Option<&str>, Vec<String>) = if kind == "rept" {
                let n = match eval(first, syms, pc)? {
                    Some(n) if (0..=65536).contains(&n) => n,
                    Some(n) => return Err(format!("bad rept count {n}")),
                    None => return Err(format!("the .rept count must be known here: {}", undefined_in(first, syms))),
                };
                (items.get(1).map(|v| block_name(v)).transpose()?, (0..n).map(|k| k.to_string()).collect())
            } else {
                if items.iter().skip(1).any(String::is_empty) {
                    return Err("an .irp value is empty".into());
                }
                (Some(block_name(first)?), items[1..].to_vec())
            };
            if values.len() * body.len() > MAX_LINES {
                return Err(format!("the program expands to more than {MAX_LINES} lines"));
            }
            let mut copies = Vec::new();
            for v in &values {
                let sub = name.map(|n| Subst::new(n, v));
                for b in &body {
                    let text = match &sub {
                        Some(sub) => sub.apply(&b.text)?,
                        None => b.text.clone(),
                    };
                    copies.push(Src { text, ..b.clone() });
                }
            }
            prepend(queue, copies);
            Ok(())
        }
        _ => Err("'.endr' without '.rept' or '.irp'".into()),
    }
}

/// A `.rept` counter or `.irp` name.
fn block_name(v: &str) -> Result<&str, String> {
    if ["a", "x", "y"].iter().any(|r| v.eq_ignore_ascii_case(r)) {
        Err(format!("'{v}' is a register: name the counter something else"))
    } else if is_ident(v) && !v.contains('@') {
        Ok(v)
    } else {
        Err(format!("bad counter name '{v}'"))
    }
}

/// A `\name` left over outside strings and comments, when no `.rept` or `.irp` named it.
fn stray_paste(code: &str) -> Option<String> {
    let cs = quoted(code);
    let at = cs.iter().position(|&(_, c, q)| c == '\\' && !q)?;
    let word: String = cs[at + 1..].iter().map(|x| x.1).take_while(|&c| is_ident_char(c)).collect();
    Some(format!("'\\{word}' pastes a .rept or .irp name, but there's none called '{word}' here"))
}

fn assemble_inner(src: &str, resolve: Resolver) -> Result<Assembly, Vec<AsmError>> {
    let core = assemble_core(src, resolve);
    if core.errors.is_empty() { Ok(core.asm) } else { Err(core.errors) }
}

/// Everything one run of the assembler found out, for `assemble` and for the editor tools
/// (`analysis`), which want the listing and the symbols even when there are errors.
pub(crate) struct Core {
    /// The listing, segments, warnings and symbols. With errors, as far as it got.
    pub asm: Assembly,
    pub errors: Vec<AsmError>,
    /// When pass 1 failed, the errors pass 2 found too (`assemble` leaves them out: some
    /// only follow from the lines pass 1 couldn't read).
    pub later: Vec<AsmError>,
    /// Beside `asm.listing`: the instruction each listed line is, with the mode pass 1 chose
    /// (even when pass 2 couldn't encode it).
    pub ops: Vec<Option<(Op, Mode)>>,
    /// Every symbol's value, before it's cut to 16 bits.
    pub values: HashMap<String, i64>,
}

/// Assembles the whole program. When pass 1 finds errors, those are the errors, but pass 2
/// still runs over the lines that could be read, so the listing has every address it can.
pub(crate) fn assemble_core(src: &str, resolve: Resolver) -> Core {
    let mut errors = Vec::new();

    // Pass 1: expand includes and repeats, find the addresses of labels and the sizes of instructions.
    let mut syms: HashMap<String, i64> = HashMap::new();
    let define = |syms: &mut HashMap<String, i64>, name: &str, v: i64, src: &Src, errors: &mut Vec<AsmError>| {
        if name.eq_ignore_ascii_case("a") {
            errors.push(src.error(format!("'{name}' can't be a name: as an operand, A means the accumulator")));
        } else if syms.insert(name.to_string(), v).is_some() {
            errors.push(src.error(defined_twice(name)));
        }
    };
    let mut pending: Vec<(Src, String, String, i64)> = Vec::new();
    let mut lines: Vec<Line> = Vec::new();
    let mut modes: Vec<Option<Mode>> = Vec::new();
    let mut spans: Vec<Span> = Vec::new();
    let mut queue: VecDeque<Src> = lines_of(src, None, 0).collect();
    let mut scope = String::new();
    let mut pc: i64 = DEFAULT_ORIGIN as i64;
    let mut read = 0;
    while let Some(s) = queue.pop_front() {
        read += 1;
        if read > MAX_LINES {
            errors.push(s.error(format!("the program expands to more than {MAX_LINES} lines")));
            break;
        }
        let code = strip_comment(&s.text).trim();
        if let Some(message) = stray_paste(code) {
            errors.push(s.error(message));
            continue;
        }
        if let (Some(l), _) = split_label(code)
            && !l.starts_with('@')
        {
            scope = l.to_string();
        }
        let code = mangle_locals(code, &scope);
        let (label, body) = match parse_line(&code) {
            Ok(x) => x,
            Err(message) => {
                errors.push(s.error(message));
                continue;
            }
        };
        if let Some(lbl) = &label {
            define(&mut syms, lbl, pc, &s, &mut errors);
        }
        let mut mode = None;
        let mut span = Span::Still;
        let mut body = body;
        let result: Result<(), String> = match &body {
            Body::Nothing => Ok(()),
            Body::Directive(d, args) if matches!(d.as_str(), "include" | "rept" | "irp" | "endr") => {
                let r = expand(d, args, &s, &mut queue, resolve, &syms, pc);
                body = Body::Nothing;
                r
            }
            Body::Const(name, e) => match eval(e, &syms, pc) {
                Ok(Some(v)) => {
                    define(&mut syms, name, v, &s, &mut errors);
                    Ok(())
                }
                Ok(None) => {
                    pending.push((s.clone(), name.clone(), e.clone(), pc));
                    Ok(())
                }
                Err(m) => Err(m),
            },
            Body::Directive(d, args) => match directive(d, args, &syms, pc, None) {
                Ok(DirEffect::Org(v)) => {
                    span = Span::Gap;
                    pc = v;
                    Ok(())
                }
                Ok(DirEffect::Reserve(n)) => {
                    span = Span::Gap;
                    pc += n;
                    Ok(())
                }
                Ok(DirEffect::Size(n)) => {
                    if n > 0 {
                        span = if d == "align" && split_items(args).len() < 2 { Span::Align } else { Span::Stores };
                    }
                    pc += n;
                    Ok(())
                }
                Err(m) => Err(m),
            },
            Body::Ins(op, operand) => {
                let val = operand.expr().and_then(|e| eval(e, &syms, pc).ok().flatten());
                choose_mode(*op, operand, val).map(|m| {
                    span = Span::Stores;
                    mode = Some(m);
                    pc += m.len() as i64;
                })
            }
        };
        if let Err(message) = result {
            errors.push(s.error(message));
        }
        if !(0..=0x10000).contains(&pc) {
            errors.push(s.error(format!("address ${pc:X} is outside memory")));
            pc = pc.clamp(0, 0x10000);
        }
        lines.push(Line { src: s, label, body });
        modes.push(mode);
        spans.push(span);
    }
    // Constants that used a symbol defined further down, each with its own `*`.
    loop {
        let before = pending.len();
        pending.retain(|(s, name, e, pc)| match eval(e, &syms, *pc) {
            Ok(Some(v)) => {
                define(&mut syms, name, v, s, &mut errors);
                false
            }
            _ => true,
        });
        if pending.len() == before {
            break;
        }
    }
    for (s, _, e, pc) in &pending {
        let message = match eval(e, &syms, *pc) {
            Err(m) => m,
            _ => undefined_in(e, &syms),
        };
        errors.push(s.error(message));
    }
    // With errors already, pass 2's own would mostly repeat them: keep only these.
    let pass1_failed = !errors.is_empty();
    if pass1_failed {
        errors.sort_by_key(|e| (e.file.is_some(), e.line));
    }

    // Whether bytes are stored right after each line, before any gap: `.align` only stores
    // its padding between stored bytes, and reserves it next to a gap.
    let mut stored_after = vec![false; lines.len()];
    let mut stores = false;
    for (i, span) in spans.iter().enumerate().rev() {
        stored_after[i] = stores;
        match span {
            Span::Stores => stores = true,
            Span::Gap => stores = false,
            Span::Still | Span::Align => {}
        }
    }

    // Pass 2: emit.
    let mut asm = Assembly::default();
    let mut ops = Vec::with_capacity(lines.len());
    let mut later = Vec::new();
    let mut seg = Segment { start: DEFAULT_ORIGIN, bytes: Vec::new() };
    let mut pc: i64 = DEFAULT_ORIGIN as i64;
    // Which line wrote each address (index + 1), to catch two sections writing the same byte.
    let mut writer: Vec<u32> = vec![0; 0x10000];
    for (idx, l) in lines.iter().enumerate() {
        let s = &l.src;
        let file = s.file.as_deref().map(str::to_string);
        let mut moved_to = None;
        let mut reserved = false;
        let mut note = None;
        let result: Result<Vec<u8>, String> = match &l.body {
            Body::Nothing | Body::Const(..) => Ok(Vec::new()),
            Body::Directive(d, args) => {
                let mut out = Vec::new();
                match directive(d, args, &syms, pc, Some(&mut out)) {
                    Ok(DirEffect::Org(v)) => {
                        moved_to = Some(v);
                        Ok(out)
                    }
                    Ok(DirEffect::Reserve(n)) => {
                        moved_to = Some(pc + n);
                        reserved = true;
                        Ok(out)
                    }
                    Ok(DirEffect::Size(n)) if spans[idx] == Span::Align && (seg.bytes.is_empty() || !stored_after[idx]) => {
                        moved_to = Some(pc + n);
                        reserved = true;
                        Ok(Vec::new())
                    }
                    Ok(DirEffect::Size(_)) => Ok(out),
                    Err(m) => Err(m),
                }
            }
            Body::Ins(op, operand) => match modes[idx] {
                Some(mode) => encode(*op, mode, operand, &syms, pc).map(|(b, n)| {
                    note = n;
                    b
                }),
                // Only after pass 1 failed: the line's error is already reported.
                None => Ok(Vec::new()),
            },
        };
        let errors = if pass1_failed { &mut later } else { &mut errors };
        let warning = l.body.exprs().iter().find_map(|e| lint(e));
        let bytes = match result {
            Ok(b) => b,
            Err(message) => {
                // A failed `.assert` is often the trap itself: say so.
                errors.push(s.error(match &warning {
                    Some(w) => format!("{message} (warning: {w})"),
                    None => message,
                }));
                Vec::new()
            }
        };
        if let Some(w) = warning {
            asm.warnings.push(s.error(w.clone()));
            note = Some(match note {
                Some(n) => format!("{n}; {w}"),
                None => w,
            });
        }
        let shows_addr = !bytes.is_empty() || l.label.is_some() || reserved;
        ops.push(match (&l.body, modes[idx]) {
            (Body::Ins(op, _), Some(mode)) => Some((*op, mode)),
            _ => None,
        });
        asm.listing.push(ListLine {
            file: file.clone(),
            line: s.no,
            addr: shows_addr.then_some(pc as u16),
            bytes: bytes.clone(),
            text: s.text.trim_end().to_string(),
            note,
        });
        if !bytes.is_empty() {
            let mut clash = None;
            for k in 0..bytes.len() {
                let a = (pc as usize + k) & 0xFFFF;
                if writer[a] != 0 && clash.is_none() {
                    clash = Some((a, writer[a] as usize - 1));
                }
                writer[a] = idx as u32 + 1;
            }
            if let Some((a, first)) = clash {
                let f = &lines[first].src;
                let by = match &f.file {
                    Some(name) => format!("{name}:{}", f.no),
                    None => format!("line {}", f.no),
                };
                errors.push(s.error(format!("address ${a:04X} is written twice (first by {by})")));
            }
            asm.lines.push(LineInfo { line: s.no, addr: pc as u16, len: bytes.len() as u16, file });
            pc += bytes.len() as i64;
            seg.bytes.extend(bytes);
        }
        if let Some(v) = moved_to {
            if !seg.bytes.is_empty() {
                asm.segments.push(std::mem::replace(&mut seg, Segment { start: 0, bytes: Vec::new() }));
            }
            seg.start = v as u16;
            pc = v;
        }
    }
    if !seg.bytes.is_empty() {
        asm.segments.push(seg);
    }
    let mut symbols: Vec<(String, u16)> = syms.iter().map(|(n, &v)| (n.clone(), v as u16)).collect();
    symbols.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
    asm.symbols = symbols;
    Core { asm, errors, later, ops, values: syms }
}
