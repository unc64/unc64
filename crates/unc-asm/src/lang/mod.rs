//! The UNC assembly language as data, for the tools outside the machine: the language
//! server, the Vim and VS Code files, the in-game editor and the website's editors. One
//! source of truth, so none of them can drift:
//!
//! - instructions come from the processor's own table (`unc_core::isa`) and the operation
//!   descriptions that also generate Appendix C (`doc::describe`);
//! - directives, functions, the memory map, I/O registers, ROM routines and their costs,
//!   system variables and the assembler's messages are read out of the manual's own tables
//!   when the crate is built (`build.rs`), and checked against the assembler by the tests.
//!
//! `json()` is all of it as JSON; `editors/README.md` documents the schema. `tokens()`
//! classifies one line for highlighting.

mod editors;
mod tokens;

pub use editors::{textmate_grammar, vim_syntax};
pub use tokens::{Token, TokenKind, tokens};
pub(crate) use tokens::comment_start;

use crate::asm::FUNCTIONS;
#[cfg(test)]
use crate::asm::DIRECTIVE_NAMES;
use crate::doc;
use crate::json::{Json, obj};
use std::sync::OnceLock;
use unc_core::isa::{ALL_OPS, Mode, OPCODES, Op};

mod manual {
    include!(concat!(env!("OUT_DIR"), "/manual_tables.rs"));
}

/// The version of the JSON layout. A change that breaks a reader gets a new number.
pub const SCHEMA: u32 = 1;

/// One addressing mode of an instruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeInfo {
    pub mode: Mode,
    /// Short and stable, for JSON: `imp`, `acc`, `imm`, `fp`, `fpx`, `fpy`, `abs`, `absx`,
    /// `absy`, `ind`, `indy`, `rel`.
    pub key: &'static str,
    /// As the manual and the messages say it: `absolute,X`.
    pub name: &'static str,
    /// As Appendix C writes it: `LDA $nnnn,X`.
    pub syntax: String,
    /// An LSP/TextMate-style snippet: `LDA ${1:addr},X`.
    pub snippet: String,
    pub opcode: u8,
    pub bytes: u8,
    /// The fixed cost. For a branch, not taken.
    pub cycles: u8,
    /// A branch's cost when it's taken.
    pub cycles_taken: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instruction {
    pub op: Op,
    pub mnemonic: &'static str,
    pub description: &'static str,
    /// The flags it changes: `N Z`, `-` for none, `all`.
    pub flags: &'static str,
    pub branch: bool,
    pub modes: Vec<ModeInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Directive {
    /// With the dot: `.byte`.
    pub name: String,
    /// Other names for it: `.db`.
    pub aliases: Vec<String>,
    /// `.fill count[, value]`.
    pub syntax: &'static str,
    pub snippet: &'static str,
    /// The manual's example: `.fill 10, $20`.
    pub example: String,
    pub description: String,
    /// `open` for `.rept` and `.irp`, `close` for `.endr`.
    pub block: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub name: &'static str,
    pub arity: usize,
    /// As the manual writes it: `sin(a, n)`.
    pub syntax: String,
    pub snippet: String,
    pub description: String,
}

/// An I/O register (Appendix B). A few cover a range: KEYS is `$C060-$C068`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IoRegister {
    pub name: String,
    pub address: u16,
    pub end: u16,
    pub read: String,
    pub write: String,
    pub chapter: Option<u32>,
}

/// A jump-table entry (Appendix D), with its cost from chapter 13.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RomRoutine {
    pub name: String,
    pub address: u16,
    pub input: String,
    pub output: String,
    pub changes: String,
    /// "about 125, on any row; when it scrolls: about 19,000: nearly a whole frame". Empty if
    /// the manual doesn't give one.
    pub cycles: String,
}

/// A named system variable (Appendix D).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variable {
    pub name: String,
    pub address: u16,
    pub end: u16,
    pub description: String,
}

/// A range of the memory map (Appendix B).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Region {
    pub start: u16,
    pub end: u16,
    pub description: String,
}

/// An assembler message (Appendix E), with an example as the manual writes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub message: String,
    pub meaning: String,
    pub warning: bool,
}

/// Everything at once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lang {
    pub instructions: Vec<Instruction>,
    pub directives: Vec<Directive>,
    pub functions: Vec<Function>,
    pub io: Vec<IoRegister>,
    pub rom: Vec<RomRoutine>,
    pub variables: Vec<Variable>,
    pub memory: Vec<Region>,
    pub messages: Vec<Message>,
}

/// A name the manual gives an address: what a program usually defines as a constant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Named<'a> {
    Io(&'a IoRegister),
    Rom(&'a RomRoutine),
    Variable(&'a Variable),
}

impl Named<'_> {
    pub fn name(&self) -> &str {
        match self {
            Named::Io(r) => &r.name,
            Named::Rom(r) => &r.name,
            Named::Variable(v) => &v.name,
        }
    }

    pub fn address(&self) -> u16 {
        match self {
            Named::Io(r) => r.address,
            Named::Rom(r) => r.address,
            Named::Variable(v) => v.address,
        }
    }

    /// `I/O register`, `ROM routine`, `system variable`.
    pub fn what(&self) -> &'static str {
        match self {
            Named::Io(_) => "I/O register",
            Named::Rom(_) => "ROM routine",
            Named::Variable(_) => "system variable",
        }
    }

    /// The manual's description in a line.
    pub fn summary(&self) -> String {
        match self {
            Named::Io(r) => match (r.read.is_empty(), r.write.is_empty()) {
                (false, false) if r.read == r.write => r.read.clone(),
                (false, false) => format!("read: {}; write: {}", r.read, r.write),
                (false, true) => format!("read: {}", r.read),
                (true, false) => format!("write: {}", r.write),
                (true, true) => String::new(),
            },
            Named::Rom(r) => {
                let mut s = r.output.clone();
                if !r.input.is_empty() {
                    s = format!("{}: {s}", r.input);
                }
                s
            }
            Named::Variable(v) => v.description.clone(),
        }
    }

    /// The manual's description as markdown lines.
    pub fn detail(&self) -> Vec<String> {
        match self {
            Named::Io(r) => {
                let mut l = vec![format!("**{}** `{}`: I/O register (Appendix B{})", r.name, range_hex(r.address, r.end), r.chapter.map(|c| format!(", chapter {c}")).unwrap_or_default())];
                if !r.read.is_empty() {
                    l.push(format!("- read: {}", r.read));
                }
                if !r.write.is_empty() {
                    l.push(format!("- write: {}", r.write));
                }
                l
            }
            Named::Rom(r) => {
                let mut l = vec![format!("**{}** `${:04X}`: ROM routine, `JSR {}` (Appendix D)", r.name, r.address, r.name)];
                for (k, v) in [("in", &r.input), ("out", &r.output), ("changes", &r.changes), ("cycles", &r.cycles)] {
                    if !v.is_empty() {
                        l.push(format!("- {k}: {v}"));
                    }
                }
                l
            }
            Named::Variable(v) => vec![format!("**{}** `{}`: system variable (Appendix D): {}", v.name, range_hex(v.address, v.end), v.description)],
        }
    }

    /// The constant definition a program writes for it: `BORDER  = $C020`.
    pub fn definition(&self) -> String {
        let name = self.name();
        let hex = if self.address() < 0x100 { format!("${:02X}", self.address()) } else { format!("${:04X}", self.address()) };
        format!("{name:<7} = {hex}")
    }
}

/// `$C020`, or `$C060-$C068` for a range.
pub fn range_hex(a: u16, b: u16) -> String {
    let h = |v: u16| if a < 0x100 && b < 0x100 { format!("${v:02X}") } else { format!("${v:04X}") };
    if a == b { h(a) } else { format!("{}-{}", h(a), h(b)) }
}

/// The markdown of a manual cell as plain text: no backticks, no emphasis, `\|` as `|`.
pub fn plain(md: &str) -> String {
    let mut out = String::with_capacity(md.len());
    let mut code = false;
    let mut cs = md.chars().peekable();
    while let Some(c) = cs.next() {
        match c {
            '\\' if cs.peek() == Some(&'|') => {}
            '`' => code = !code,
            '*' if !code => {}
            c => out.push(c),
        }
    }
    out.trim().to_string()
}

/// `$C020` or `$C060-$C068` (in backticks or not) as a range.
fn parse_range(cell: &str) -> Option<(u16, u16)> {
    let p = plain(cell);
    let hex = |s: &str| u16::from_str_radix(s.trim().trim_start_matches('$'), 16).ok();
    match p.split_once('-') {
        Some((a, b)) => Some((hex(a)?, hex(b)?)),
        None => hex(&p).map(|a| (a, a)),
    }
}

pub fn mode_key(m: Mode) -> &'static str {
    match m {
        Mode::Imp => "imp",
        Mode::Acc => "acc",
        Mode::Imm => "imm",
        Mode::Fp => "fp",
        Mode::Fpx => "fpx",
        Mode::Fpy => "fpy",
        Mode::Abs => "abs",
        Mode::Absx => "absx",
        Mode::Absy => "absy",
        Mode::Ind => "ind",
        Mode::Indy => "indy",
        Mode::Rel => "rel",
    }
}

fn mode_snippet(op: Op, m: Mode) -> String {
    let n = op.mnemonic();
    match m {
        Mode::Imp => n.to_string(),
        Mode::Acc => format!("{n} A"),
        Mode::Imm => format!("{n} #${{1:value}}"),
        Mode::Fp => format!("{n} ${{1:fp}}"),
        Mode::Fpx => format!("{n} ${{1:fp}},X"),
        Mode::Fpy => format!("{n} ${{1:fp}},Y"),
        Mode::Abs => format!("{n} ${{1:addr}}"),
        Mode::Absx => format!("{n} ${{1:addr}},X"),
        Mode::Absy => format!("{n} ${{1:addr}},Y"),
        Mode::Ind => format!("{n} (${{1:addr}})"),
        Mode::Indy => format!("{n} (${{1:fp}}),Y"),
        Mode::Rel => format!("{n} ${{1:label}}"),
    }
}

fn instructions() -> Vec<Instruction> {
    ALL_OPS
        .iter()
        .map(|&op| {
            let (description, flags) = doc::describe(op);
            let modes = OPCODES
                .iter()
                .enumerate()
                .filter_map(|(code, i)| i.filter(|i| i.op == op).map(|i| (code as u8, i)))
                .map(|(opcode, i)| ModeInfo {
                    mode: i.mode,
                    key: mode_key(i.mode),
                    name: i.mode.name(),
                    syntax: doc::syntax(op, i.mode),
                    snippet: mode_snippet(op, i.mode),
                    opcode,
                    bytes: i.mode.len(),
                    cycles: i.cycles,
                    cycles_taken: op.is_branch().then_some(i.cycles + 1),
                })
                .collect();
            Instruction { op, mnemonic: op.mnemonic(), description, flags, branch: op.is_branch(), modes }
        })
        .collect()
}

/// Each directive's general form and snippet; the descriptions come from the manual.
/// `lang_directives_match_the_assembler` checks this covers exactly `DIRECTIVE_NAMES`.
const DIRECTIVE_FORMS: &[(&str, &str, &str)] = &[
    ("org", ".org address", ".org ${1:\\$0800}"),
    ("byte", ".byte value, ...", ".byte ${1:value}"),
    ("text", ".text \"string\"", ".text \"${1:TEXT}\""),
    ("asciiz", ".asciiz \"string\"", ".asciiz \"${1:TEXT}\""),
    ("word", ".word value, ...", ".word ${1:value}"),
    ("fill", ".fill count[, value]", ".fill ${1:count}, ${2:value}"),
    ("align", ".align n[, value]", ".align ${1:256}"),
    ("res", ".res count", ".res ${1:count}"),
    ("rept", ".rept count[, counter] ... .endr", ".rept ${1:count}, ${2:i}\n\t$0\n.endr"),
    ("irp", ".irp name, value, ... ... .endr", ".irp ${1:name}, ${2:values}\n\t$0\n.endr"),
    ("endr", ".endr", ".endr"),
    ("include", ".include \"file\"", ".include \"${1:file.s}\""),
    ("assert", ".assert expr[, \"message\"]", ".assert ${1:expr}, \"${2:message}\""),
];

fn directives() -> Vec<Directive> {
    let mut out = Vec::new();
    for row in manual::DIRECTIVES {
        let (cell, desc) = (row[0], row[1]);
        // The directives the row is about: `.rept 8` ... `.endr`.
        let names: Vec<String> = cell
            .split('`')
            .skip(1)
            .step_by(2)
            .filter_map(|code| code.split_whitespace().next())
            .filter(|w| w.starts_with('.'))
            .map(str::to_ascii_lowercase)
            .collect();
        let Some(name) = names.first() else { continue };
        // "`.db` is the same."
        let aliases = desc
            .split('`')
            .collect::<Vec<_>>()
            .windows(2)
            .filter(|w| w[1].starts_with(" is the same"))
            .filter_map(|w| w[0].rsplit('`').next())
            .filter(|a| a.starts_with('.'))
            .map(str::to_string)
            .collect();
        let example = plain(cell);
        let form = DIRECTIVE_FORMS.iter().find(|f| format!(".{}", f.0) == *name);
        let block = match name.as_str() {
            ".rept" | ".irp" => Some("open"),
            _ => None,
        };
        out.push(Directive {
            name: name.clone(),
            aliases,
            syntax: form.map_or("", |f| f.1),
            snippet: form.map_or("", |f| f.2),
            example,
            description: plain(desc),
            block,
        });
        // `.endr` has no row of its own: it closes the blocks of the rows that name it.
        if names.iter().any(|n| n == ".endr") && !out.iter().any(|d| d.name == ".endr") {
            let form = DIRECTIVE_FORMS.iter().find(|f| f.0 == "endr").expect("endr form");
            out.push(Directive {
                name: ".endr".into(),
                aliases: Vec::new(),
                syntax: form.1,
                snippet: form.2,
                example: ".endr".into(),
                description: "End a .rept or .irp block.".into(),
                block: Some("close"),
            });
        }
    }
    // `.endr` after the blocks it closes, in the manual's order otherwise.
    if let Some(i) = out.iter().position(|d| d.name == ".endr") {
        let endr = out.remove(i);
        let last_block = out.iter().rposition(|d| d.block == Some("open")).map_or(out.len(), |p| p + 1);
        out.insert(last_block, endr);
    }
    out
}

fn functions() -> Vec<Function> {
    let mut out = Vec::new();
    for row in manual::FUNCTIONS {
        let (cell, desc) = (row[0], row[1]);
        let forms: Vec<&str> = cell.split('`').skip(1).step_by(2).collect();
        for form in &forms {
            let Some((name, args)) = form.split_once('(') else { continue };
            let Some(&(fname, arity, _)) = FUNCTIONS.iter().find(|f| f.0 == name) else { continue };
            let params: Vec<String> = args.trim_end_matches(')').split(',').map(|a| a.trim().to_string()).collect();
            let snippet = format!("{fname}({})", params.iter().enumerate().map(|(i, p)| format!("${{{}:{p}}}", i + 1)).collect::<Vec<_>>().join(", "));
            let description = if forms.len() > 1 { format!("{}: {}", plain(cell), plain(desc)) } else { plain(desc) };
            out.push(Function { name: fname, arity, syntax: form.to_string(), snippet, description });
        }
    }
    out
}

fn io_registers() -> Vec<IoRegister> {
    manual::IO_REGISTERS
        .iter()
        .filter(|r| !r[1].is_empty())
        .filter_map(|r| {
            let (address, end) = parse_range(r[0])?;
            Some(IoRegister { name: plain(r[1]), address, end, read: plain(r[2]), write: plain(r[3]), chapter: r[4].parse().ok() })
        })
        .collect()
}

fn rom_routines() -> Vec<RomRoutine> {
    manual::JUMP_TABLE
        .iter()
        .filter_map(|r| {
            let (address, _) = parse_range(r[0])?;
            let name = plain(r[1]);
            let cycles: Vec<String> = manual::ROM_COSTS
                .iter()
                .filter_map(|c| {
                    let who = plain(c[0]);
                    if who == name {
                        Some(plain(c[1]))
                    } else {
                        who.strip_prefix(&format!("{name}, ")).map(|when| format!("{when}: {}", plain(c[1])))
                    }
                })
                .collect();
            Some(RomRoutine { name, address, input: plain(r[2]), output: plain(r[3]), changes: plain(r[4]), cycles: cycles.join("; ") })
        })
        .collect()
}

fn variables() -> Vec<Variable> {
    manual::SYSTEM_VARIABLES
        .iter()
        .filter(|r| !r[1].is_empty())
        .filter_map(|r| {
            let (address, end) = parse_range(r[0])?;
            Some(Variable { name: plain(r[1]), address, end, description: plain(r[2]) })
        })
        .collect()
}

fn memory() -> Vec<Region> {
    manual::MEMORY_MAP
        .iter()
        .filter_map(|r| {
            let (start, end) = parse_range(r[0])?;
            Some(Region { start, end, description: plain(r[2]) })
        })
        .collect()
}

fn messages() -> Vec<Message> {
    let rows = |t: &[&[&str]], warning: bool| -> Vec<Message> { t.iter().map(|r| Message { message: plain(r[0]), meaning: plain(r[1]), warning }).collect() };
    let mut m = rows(manual::ERRORS, false);
    m.extend(rows(manual::WARNINGS, true));
    m
}

/// The language, built once.
pub fn get() -> &'static Lang {
    static LANG: OnceLock<Lang> = OnceLock::new();
    LANG.get_or_init(|| Lang {
        instructions: instructions(),
        directives: directives(),
        functions: functions(),
        io: io_registers(),
        rom: rom_routines(),
        variables: variables(),
        memory: memory(),
        messages: messages(),
    })
}

impl Lang {
    pub fn instruction(&self, mnemonic: &str) -> Option<&Instruction> {
        self.instructions.iter().find(|i| i.mnemonic.eq_ignore_ascii_case(mnemonic))
    }

    /// A directive by its name or an alias, with or without the dot, in any case.
    pub fn directive(&self, name: &str) -> Option<&Directive> {
        let n = format!(".{}", name.trim_start_matches('.')).to_ascii_lowercase();
        self.directives.iter().find(|d| d.name == n || d.aliases.contains(&n))
    }

    pub fn function(&self, name: &str) -> Option<&Function> {
        self.functions.iter().find(|f| f.name.eq_ignore_ascii_case(name))
    }

    /// Every name the manual gives an address: I/O registers, ROM routines, system variables.
    pub fn named(&self) -> impl Iterator<Item = Named<'_>> {
        self.io.iter().map(Named::Io).chain(self.rom.iter().map(Named::Rom)).chain(self.variables.iter().map(Named::Variable))
    }

    /// The manual's name for `name`, exactly as written (names are case-sensitive).
    pub fn named_as(&self, name: &str) -> Option<Named<'_>> {
        self.named().find(|n| n.name() == name)
    }

    /// What the manual says is at an address: the I/O register, ROM routine or system
    /// variable there (a range counts), most specific first, then the memory map's region.
    pub fn at_address(&self, addr: u16) -> (Vec<Named<'_>>, Option<&Region>) {
        let named = self
            .named()
            .filter(|n| match n {
                Named::Io(r) => (r.address..=r.end).contains(&addr),
                Named::Rom(r) => r.address == addr,
                Named::Variable(v) => (v.address..=v.end).contains(&addr),
            })
            .collect();
        (named, self.memory.iter().find(|r| (r.start..=r.end).contains(&addr)))
    }

    /// The manual's explanation of an assembler message (Appendix E), matched by its shape:
    /// the parts in quotes and the numbers can differ.
    pub fn explain(&self, message: &str) -> Option<&Message> {
        // An assertion's own message is whatever the program says.
        if message.starts_with("assertion failed") {
            return self.messages.iter().find(|m| m.message.starts_with("assertion failed"));
        }
        let shape = |s: &str| -> Vec<String> {
            let mut s = s.to_string();
            let mut modes: Vec<&str> = [Mode::Indy, Mode::Fpx, Mode::Fpy, Mode::Absx, Mode::Absy, Mode::Fp, Mode::Abs, Mode::Imm, Mode::Ind, Mode::Acc, Mode::Imp, Mode::Rel].iter().map(|m| m.name()).collect();
            modes.sort_by_key(|m| std::cmp::Reverse(m.len()));
            for m in modes {
                s = s.replace(&format!(" {m} mode"), " MODE mode");
            }
            let mut words = Vec::new();
            for (i, part) in s.split('\'').enumerate() {
                if i % 2 == 1 {
                    words.push("'".to_string());
                    continue;
                }
                words.extend(
                    part.split(|c: char| !c.is_ascii_alphabetic() && c != '#' && c != '.')
                        .map(|w| w.trim_matches('.'))
                        .filter(|w| !w.is_empty() && !w.chars().all(|c| c.is_ascii_uppercase() || c == '#') || *w == "#" || *w == "MODE")
                        .map(|w| if w.starts_with('#') { "#".to_string() } else { w.to_string() }),
                );
            }
            words
        };
        let want = shape(message);
        self.messages.iter().find(|m| !want.is_empty() && shape(&m.message) == want)
    }
}

fn hex(v: u16) -> String {
    format!("${v:04X}")
}

/// The whole language as JSON (layout in `editors/README.md`), indented for reading.
pub fn json() -> String {
    json_value().pretty()
}

/// `json()` without the indentation, for the game and the browser.
pub fn json_compact() -> String {
    json_value().compact()
}

pub(crate) fn json_value() -> Json {
    let l = get();
    let instructions: Vec<Json> = l
        .instructions
        .iter()
        .map(|i| {
            let modes: Vec<Json> = i
                .modes
                .iter()
                .map(|m| {
                    obj! {
                        "mode" => m.key, "name" => m.name, "syntax" => m.syntax.as_str(), "snippet" => m.snippet.as_str(),
                        "opcode" => m.opcode, "bytes" => m.bytes, "cycles" => m.cycles, "cycles_taken" => m.cycles_taken,
                    }
                })
                .collect();
            obj! { "mnemonic" => i.mnemonic, "description" => i.description, "flags" => i.flags, "branch" => i.branch, "modes" => Json::Arr(modes) }
        })
        .collect();
    let directives: Vec<Json> = l
        .directives
        .iter()
        .map(|d| {
            obj! {
                "name" => d.name.as_str(), "aliases" => d.aliases.clone(), "syntax" => d.syntax, "snippet" => d.snippet,
                "example" => d.example.as_str(), "description" => d.description.as_str(), "block" => d.block,
            }
        })
        .collect();
    let functions: Vec<Json> = l
        .functions
        .iter()
        .map(|f| obj! { "name" => f.name, "arity" => f.arity, "syntax" => f.syntax.as_str(), "snippet" => f.snippet.as_str(), "description" => f.description.as_str() })
        .collect();
    let io: Vec<Json> = l
        .io
        .iter()
        .map(|r| {
            obj! {
                "name" => r.name.as_str(), "address" => r.address, "end" => r.end, "hex" => range_hex(r.address, r.end),
                "read" => r.read.as_str(), "write" => r.write.as_str(), "chapter" => r.chapter,
            }
        })
        .collect();
    let rom: Vec<Json> = l
        .rom
        .iter()
        .map(|r| {
            obj! {
                "name" => r.name.as_str(), "address" => r.address, "hex" => hex(r.address), "in" => r.input.as_str(),
                "out" => r.output.as_str(), "changes" => r.changes.as_str(), "cycles" => r.cycles.as_str(),
            }
        })
        .collect();
    let variables: Vec<Json> = l
        .variables
        .iter()
        .map(|v| obj! { "name" => v.name.as_str(), "address" => v.address, "end" => v.end, "hex" => range_hex(v.address, v.end), "description" => v.description.as_str() })
        .collect();
    let memory: Vec<Json> = l.memory.iter().map(|r| obj! { "start" => r.start, "end" => r.end, "hex" => format!("${:04X}-${:04X}", r.start, r.end), "description" => r.description.as_str() }).collect();
    let messages: Vec<Json> = l
        .messages
        .iter()
        .map(|m| obj! { "message" => m.message.as_str(), "meaning" => m.meaning.as_str(), "severity" => if m.warning { "warning" } else { "error" } })
        .collect();
    let kinds: Vec<&str> = TokenKind::ALL.iter().map(|k| k.name()).collect();
    obj! {
        "schema" => SCHEMA,
        "language" => "UNC assembly",
        "cpu" => "UNC-1",
        "syntax" => obj! {
            "comment" => ";",
            "label_suffix" => ":",
            "local_prefix" => "@",
            "constant" => "NAME = value",
            "name" => "[A-Za-z_][A-Za-z0-9_]*",
            "names_case_sensitive" => true,
            "case_insensitive" => vec!["mnemonics", "directives", "functions", "registers"],
            "hex_prefix" => "$",
            "binary_prefix" => "%",
            "char_quote" => "'",
            "string_quote" => "\"",
            "escapes" => vec!["\\\"", "\\'", "\\\\"],
            "current_address" => "*",
            "group" => vec!["[", "]"],
            "paste" => "\\",
            "registers" => vec!["A", "X", "Y"],
            "operators" => vec!["||", "&&", "|", "^", "&", "==", "!=", "<", "<=", ">", ">=", "<<", ">>", "+", "-", "*", "/", "%"],
            "unary" => vec!["-", "~", "<", ">"],
            "extensions" => vec![".s", ".uncs", ".unc.s"],
        },
        "token_kinds" => kinds,
        "instructions" => Json::Arr(instructions),
        "directives" => Json::Arr(directives),
        "functions" => Json::Arr(functions),
        "io" => Json::Arr(io),
        "rom" => Json::Arr(rom),
        "variables" => Json::Arr(variables),
        "memory" => Json::Arr(memory),
        "messages" => Json::Arr(messages),
    }
}

/// One line's tokens as JSON: `[[start, end, "kind"], ...]`, columns in characters.
pub fn tokens_json(line: &str) -> String {
    tokens_value(line).compact()
}

/// Every line's tokens as JSON, one array per line (lines split at `\n`, a `\r` before it
/// dropped): `[[[start, end, "kind"], ...], ...]`, columns in characters.
pub fn source_tokens_json(src: &str) -> String {
    Json::Arr(src.split('\n').map(|l| tokens_value(l.strip_suffix('\r').unwrap_or(l))).collect()).compact()
}

pub(crate) fn tokens_value(line: &str) -> Json {
    Json::Arr(
        tokens(line)
            .iter()
            .map(|t| Json::Arr(vec![crate::json::char_col(line, t.start).into(), crate::json::char_col(line, t.end).into(), t.kind.name().into()]))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lang_directives_match_the_assembler() {
        let l = get();
        let mut ours: Vec<String> = l.directives.iter().flat_map(|d| std::iter::once(d.name.clone()).chain(d.aliases.clone())).map(|n| n[1..].to_string()).collect();
        let mut theirs: Vec<String> = DIRECTIVE_NAMES.iter().map(|s| s.to_string()).collect();
        ours.sort();
        theirs.sort();
        assert_eq!(ours, theirs, "the manual's directive table and the assembler disagree");
        assert_eq!(DIRECTIVE_FORMS.len(), l.directives.len(), "a directive without a form, or a form without a directive");
        for d in &l.directives {
            assert!(!d.syntax.is_empty() && !d.description.is_empty(), "{}", d.name);
        }
        // The assembler takes each one, and knows no others.
        for name in DIRECTIVE_NAMES {
            let src = format!(" .{name} 1\n");
            if let Err(e) = crate::assemble(&src) {
                assert!(!e[0].message.starts_with("unknown directive"), ".{name}: {:?}", e);
            }
        }
        for name in ["dc", "ds", "macro", "endm", "if", "equ", "set", "segment", "incbin", "dword"] {
            let e = crate::assemble(&format!(" .{name} 1")).expect_err(name);
            assert!(e[0].message.starts_with("unknown directive"), ".{name}");
        }
        assert_eq!(l.directive("DB").unwrap().name, ".byte");
        assert_eq!(l.directive(".endr").unwrap().block, Some("close"));
    }

    #[test]
    fn lang_functions_match_the_assembler() {
        let l = get();
        let ours: Vec<&str> = l.functions.iter().map(|f| f.name).collect();
        let theirs: Vec<&str> = FUNCTIONS.iter().map(|f| f.0).collect();
        assert_eq!(ours, theirs);
        assert_eq!(l.function("SIN").unwrap().snippet, "sin(${1:a}, ${2:n})");
        assert_eq!(l.function("max").unwrap().arity, 2);
    }

    #[test]
    fn plain_text_from_markdown() {
        assert_eq!(plain("**Fast page.** `$D0-$F6` UNC BASIC"), "Fast page. $D0-$F6 UNC BASIC");
        assert_eq!(plain("`a \\| b` and `*`"), "a | b and *");
    }

    #[test]
    fn explain_matches_messages_by_shape() {
        let l = get();
        assert_eq!(l.explain("unknown instruction 'FOO'").unwrap().message, "unknown instruction 'LDQ'");
        assert_eq!(l.explain("'@x' is defined twice (after 'main')").unwrap().message, "'@x' is defined twice (after 'loop')");
        assert_eq!(l.explain("branch out of range (-140 bytes)").unwrap().message, "branch out of range (200 bytes)");
        assert_eq!(l.explain("LDX has no indirect mode").unwrap().message, "STA has no immediate mode");
        assert_eq!(l.explain("LDA has no absolute,X mode").unwrap().message, "STA has no immediate mode");
        assert_eq!(l.explain("#-200 doesn't fit in a byte").unwrap().message, "#256 doesn't fit in a byte");
        assert_eq!(l.explain("-200 doesn't fit in a byte").unwrap().message, "300 doesn't fit in a byte");
        assert_eq!(l.explain("assertion failed: x > 3").unwrap().message, "assertion failed: expr");
        assert_eq!(l.explain("the screen is full"), None);
        assert!(l.explain("'==' binds tighter than '|': add [ ] to say which you mean").unwrap().warning);
    }
}
