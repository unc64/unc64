//! What the editor tools need to know about a program, from the real assembler: for each
//! source line its address, bytes, instruction and fixed cycle cost (Appendix C), the symbols
//! it defines and uses with their columns, the `.rept`/`.irp` blocks with their totals, and
//! every error and warning with the columns it's about.
//!
//! It works on a program with errors too: then the addresses and symbols are as far as the
//! assembler got, so an editor can keep showing them while a line is being typed.

use crate::asm::{AsmError, Assembly, assemble_core};
use crate::json::{Json, char_col, obj};
use crate::lang::{TokenKind, comment_start, tokens};
use std::cell::RefCell;
use std::collections::HashMap;
use unc_core::isa::{Mode, OPCODES, Op, opcode_for};

/// An error or warning, with the byte columns of the part of the line it's about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// The included file the line is in; `None` for the main source.
    pub file: Option<String>,
    pub line: usize,
    pub start: usize,
    pub end: usize,
    pub message: String,
    pub warning: bool,
}

/// A name on a line, with its byte columns. `name` is as the assembler knows it: a local
/// label `@loop` under `clear` is `clear@loop`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymRef {
    pub name: String,
    pub start: usize,
    pub end: usize,
}

/// One line of a source file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceLine {
    /// From 1.
    pub line: usize,
    /// Where its first copy went, if it stores bytes, reserves space or defines a label.
    pub addr: Option<u16>,
    /// The first copy's bytes.
    pub bytes: Vec<u8>,
    pub instruction: Option<(Op, Mode)>,
    /// The instruction's fixed cost; for a branch, not taken (taken adds one).
    pub cycles: Option<u8>,
    /// How many times the line was assembled: more than 1 inside `.rept` and `.irp`.
    pub copies: u32,
    /// `cycles` over every copy.
    pub total_cycles: u64,
    pub defs: Vec<SymRef>,
    pub refs: Vec<SymRef>,
}

impl SourceLine {
    pub fn is_branch(&self) -> bool {
        self.instruction.is_some_and(|(op, _)| op.is_branch())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File {
    /// `None` for the main source, else the name as `.include` gives it.
    pub name: Option<String>,
    pub lines: Vec<SourceLine>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    Label,
    Local,
    Constant,
}

impl SymbolKind {
    pub fn name(self) -> &'static str {
        match self {
            SymbolKind::Label => "label",
            SymbolKind::Local => "local",
            SymbolKind::Constant => "constant",
        }
    }
}

/// A symbol and where it's defined.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub name: String,
    /// `None` if the assembler didn't get as far as working it out.
    pub value: Option<i64>,
    pub kind: SymbolKind,
    pub file: Option<String>,
    pub line: usize,
    pub start: usize,
    pub end: usize,
}

/// A `.rept` or `.irp` block, from its first line to its `.endr`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub file: Option<String>,
    /// `rept` or `irp`.
    pub kind: &'static str,
    /// The line of the `.rept` or `.irp`.
    pub start: usize,
    /// The line of its `.endr`; `None` if it has none.
    pub end: Option<usize>,
    /// How many times the assembler reached the block (more than once inside another block).
    pub entered: u32,
    /// The copies of the lines in between, over every time it was reached.
    pub copies: u32,
    /// The cycles of every instruction in every copy, branches not taken.
    pub cycles: u64,
    /// Whether there's a branch in it, which costs one more when it's taken.
    pub branches: bool,
}

impl Block {
    /// The cycles of going through the block once, all its copies: `cycles / entered`.
    pub fn cycles_per_entry(&self) -> u64 {
        self.cycles / self.entered.max(1) as u64
    }

    /// The copies made each time it's reached.
    pub fn copies_per_entry(&self) -> u32 {
        self.copies / self.entered.max(1)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Analysis {
    pub errors: Vec<Diagnostic>,
    pub warnings: Vec<Diagnostic>,
    /// The main source first, then each included file in the order it was first read.
    pub files: Vec<File>,
    pub blocks: Vec<Block>,
    pub symbols: Vec<Symbol>,
    /// The assembled program, when there were no errors.
    pub assembly: Option<Assembly>,
    /// `symbols` by name.
    index: HashMap<String, usize>,
}

/// Analyses a program that has no `.include`s.
pub fn analyze(src: &str) -> Analysis {
    analyze_inner(src, None)
}

/// Analyses a program, reading each `.include` through `resolve` (as `assemble_with`).
pub fn analyze_with(src: &str, resolve: impl Fn(&str) -> Option<String>) -> Analysis {
    analyze_inner(src, Some(&resolve))
}

type Resolve<'a> = Option<&'a dyn Fn(&str) -> Option<String>>;

fn analyze_inner(src: &str, resolve: Resolve) -> Analysis {
    let read: RefCell<Vec<(String, String)>> = RefCell::new(Vec::new());
    let recording = |name: &str| {
        let text = resolve?(name)?;
        let mut r = read.borrow_mut();
        if !r.iter().any(|(n, _)| n == name) {
            r.push((name.to_string(), text.clone()));
        }
        Some(text)
    };
    let core = assemble_core(src, resolve.map(|_| &recording as &dyn Fn(&str) -> Option<String>));
    let mut texts: Vec<(Option<String>, String)> = vec![(None, src.to_string())];
    texts.extend(read.into_inner().into_iter().map(|(n, t)| (Some(n), t)));

    // What the assembler made of each source line, over its copies.
    let mut made: HashMap<(Option<&str>, usize), Vec<usize>> = HashMap::new();
    for (i, l) in core.asm.listing.iter().enumerate() {
        made.entry((l.file.as_deref(), l.line)).or_default().push(i);
    }
    let cost = |i: usize| -> Option<u8> { core.ops[i].and_then(|(op, mode)| opcode_for(op, mode)).and_then(|c| OPCODES[c as usize]).map(|info| info.cycles) };

    let mut a = Analysis::default();
    for (name, text) in &texts {
        let mut lines = Vec::new();
        let mut scope = String::new();
        // The `.rept` counters and `.irp` names in force, innermost last.
        let mut counters: Vec<Vec<String>> = Vec::new();
        let mut open: Vec<(usize, &'static str)> = Vec::new();
        for (i, text) in text.lines().enumerate() {
            let no = i + 1;
            let mut sl = SourceLine { line: no, ..Default::default() };
            if let Some(idx) = made.get(&(name.as_deref(), no)) {
                let first = &core.asm.listing[idx[0]];
                sl.addr = first.addr;
                sl.bytes = first.bytes.clone();
                sl.instruction = core.ops[idx[0]];
                sl.cycles = cost(idx[0]);
                sl.copies = idx.len() as u32;
                sl.total_cycles = idx.iter().filter_map(|&i| cost(i)).map(u64::from).sum();
            }
            let toks = tokens(text);
            let directive = toks.iter().find(|t| t.kind == TokenKind::Directive).map(|t| t.text(text).to_ascii_lowercase());
            let in_counters = |w: &str| counters.iter().flatten().any(|c| c == w);
            let mut block_names = Vec::new();
            let mut after_comma = false;
            let mut seen_symbol = false;
            for t in &toks {
                let w = t.text(text);
                let sym = |name: String| SymRef { name, start: t.start, end: t.end };
                match t.kind {
                    _ if w.contains('\\') => {}
                    TokenKind::Label => {
                        scope = w.to_string();
                        sl.defs.push(sym(w.to_string()));
                    }
                    TokenKind::Constant => sl.defs.push(sym(w.to_string())),
                    TokenKind::Local => {
                        let full = format!("{scope}{w}");
                        // A local label being defined is followed by its `:`.
                        let defined = toks.iter().any(|n| n.start >= t.end && n.kind == TokenKind::Operator && n.text(text) == ":" && text[t.end..n.start].trim().is_empty())
                            && toks.first() == Some(t);
                        if defined { sl.defs.push(sym(full)) } else { sl.refs.push(sym(full)) }
                    }
                    TokenKind::Symbol => {
                        // `.rept n, i` names its counter after the first comma; `.irp s, ...` first.
                        let names_block = match directive.as_deref() {
                            Some(".rept") => after_comma && block_names.is_empty(),
                            Some(".irp") => !seen_symbol,
                            _ => false,
                        };
                        seen_symbol = true;
                        if names_block {
                            block_names.push(w.to_string());
                        } else if !in_counters(w) {
                            sl.refs.push(sym(w.to_string()));
                        }
                    }
                    TokenKind::Operator if w == "," => after_comma = true,
                    _ => {}
                }
            }
            match directive.as_deref() {
                Some(d @ (".rept" | ".irp")) => {
                    counters.push(block_names);
                    open.push((no, if d == ".rept" { "rept" } else { "irp" }));
                }
                Some(".endr") => {
                    counters.pop();
                    if let Some((start, kind)) = open.pop() {
                        a.blocks.push(block(&core, &made, name, kind, start, Some(no), cost));
                    }
                }
                _ => {}
            }
            lines.push(sl);
        }
        for (start, kind) in open {
            a.blocks.push(block(&core, &made, name, kind, start, None, cost));
        }
        a.files.push(File { name: name.clone(), lines });
    }
    a.blocks.sort_by_key(|b| (b.file.is_some(), b.start));

    // Symbols, where they're defined, and their values.
    for f in &a.files {
        for l in &f.lines {
            for d in &l.defs {
                let kind = if d.name.contains('@') {
                    SymbolKind::Local
                } else if l.defs.len() == 1 && is_constant_line(&texts, &f.name, l.line) {
                    SymbolKind::Constant
                } else {
                    SymbolKind::Label
                };
                a.symbols.push(Symbol { name: d.name.clone(), value: core.values.get(&d.name).copied(), kind, file: f.name.clone(), line: l.line, start: d.start, end: d.end });
            }
        }
    }

    // The first definition of a name is the one the assembler kept.
    for (i, s) in a.symbols.iter().enumerate() {
        a.index.entry(s.name.clone()).or_insert(i);
    }

    let text_of = |file: &Option<String>, line: usize| -> &str { texts.iter().find(|(n, _)| n == file).and_then(|(_, t)| t.lines().nth(line.wrapping_sub(1))).unwrap_or("") };
    let diag = |e: &AsmError, warning: bool| {
        let (start, end) = error_span(text_of(&e.file, e.line), &e.message);
        Diagnostic { file: e.file.clone(), line: e.line, start, end, message: e.message.clone(), warning }
    };
    a.errors = core.errors.iter().map(|e| diag(e, false)).collect();
    // After lines that couldn't be read, pass 2's errors too, except where they only follow
    // from those lines: a name they'd have defined, or a line already in error.
    let defined: Vec<&str> = a.symbols.iter().map(|s| s.name.as_str()).collect();
    let follows = |m: &str| quoted_parts(m).first().is_some_and(|q| defined.iter().any(|d| d == q || d.ends_with(q) && q.starts_with('@')));
    for e in &core.later {
        let cascade = (e.message.starts_with("undefined") && follows(&e.message)) || a.errors.iter().any(|x| x.file == e.file && x.line == e.line);
        if !cascade {
            a.errors.push(diag(e, false));
        }
    }
    a.errors.sort_by_key(|e| (e.file.is_some(), e.file.clone(), e.line));
    a.warnings = core.asm.warnings.iter().map(|e| diag(e, true)).collect();
    if core.errors.is_empty() {
        a.assembly = Some(core.asm);
    }
    a
}

fn is_constant_line(texts: &[(Option<String>, String)], file: &Option<String>, line: usize) -> bool {
    let text = texts.iter().find(|(n, _)| n == file).and_then(|(_, t)| t.lines().nth(line - 1)).unwrap_or("");
    tokens(text).first().is_some_and(|t| t.kind == TokenKind::Constant)
}

fn block(
    core: &crate::asm::Core,
    made: &HashMap<(Option<&str>, usize), Vec<usize>>,
    file: &Option<String>,
    kind: &'static str,
    start: usize,
    end: Option<usize>,
    cost: impl Fn(usize) -> Option<u8>,
) -> Block {
    let f = file.as_deref();
    let entered = made.get(&(f, start)).map_or(0, |v| v.len() as u32);
    let last = end.unwrap_or(usize::MAX);
    let mut cycles = 0;
    let mut branches = false;
    let mut copies = None;
    for (i, l) in core.asm.listing.iter().enumerate() {
        if l.file.as_deref() == f && l.line > start && l.line < last {
            if let Some(c) = cost(i) {
                cycles += c as u64;
            }
            branches |= core.ops[i].is_some_and(|(op, _)| op.is_branch());
        }
    }
    // Each copy lists the block's first line that the assembler could read.
    for no in start + 1..last.min(start + 10_000) {
        if let Some(v) = made.get(&(f, no)) {
            copies = Some(v.len() as u32);
            break;
        }
    }
    Block { file: file.clone(), kind, start, end, entered, copies: copies.unwrap_or(0), cycles, branches }
}

/// The parts of a message in single quotes: `'x'`, `'.rept'`. An apostrophe inside a word
/// (`can't`) isn't a quote.
fn quoted_parts(message: &str) -> Vec<&str> {
    let b = message.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\'' && (i == 0 || !b[i - 1].is_ascii_alphanumeric()) {
            let mut j = i + 1;
            while j < b.len() && !(b[j] == b'\'' && (j + 1 == b.len() || !b[j + 1].is_ascii_alphanumeric())) {
                j += 1;
            }
            if j < b.len() && j > i + 1 {
                out.push(&message[i + 1..j]);
            }
            i = j + 1;
        } else {
            i += 1;
        }
    }
    out
}

/// The byte columns of the part of `line` that an assembler message is about: the name or
/// text it quotes, else the operand, else the whole statement (comments left out).
pub fn error_span(line: &str, message: &str) -> (usize, usize) {
    let code = &line[..comment_start(line)];
    let cs = code.len() - code.trim_start().len();
    let ce = code.trim_end().len();
    if cs >= ce {
        return (0, line.trim_end().len());
    }
    let toks: Vec<_> = tokens(line).into_iter().filter(|t| t.kind != TokenKind::Comment).collect();
    for q in quoted_parts(message) {
        let q = q.trim();
        if let Some(t) = toks.iter().find(|t| t.text(line).eq_ignore_ascii_case(q)) {
            return (t.start, t.end);
        }
        if !q.is_empty()
            && let Some(i) = code.to_ascii_lowercase().find(&q.to_ascii_lowercase())
        {
            return (i, i + q.len());
        }
    }
    let word = toks.iter().position(|t| matches!(t.kind, TokenKind::Mnemonic | TokenKind::Directive | TokenKind::Invalid));
    if message.starts_with("address $") {
        return (cs, ce);
    }
    if let Some(w) = word {
        if message.ends_with("needs an operand") {
            return (toks[w].start, toks[w].end);
        }
        if let (Some(first), Some(last)) = (toks.get(w + 1), toks.last())
            && last.end > first.start
        {
            return (first.start, last.end);
        }
    } else if let Some(eq) = toks.iter().position(|t| t.kind == TokenKind::Operator && t.text(line) == "=")
        && let (Some(first), Some(last)) = (toks.get(eq + 1), toks.last())
    {
        return (first.start, last.end);
    }
    (cs, ce)
}

impl Analysis {
    /// The main source.
    pub fn main(&self) -> &File {
        &self.files[0]
    }

    pub fn file(&self, name: Option<&str>) -> Option<&File> {
        self.files.iter().find(|f| f.name.as_deref() == name)
    }

    /// The symbol defined as `name` (as the assembler knows it).
    pub fn symbol(&self, name: &str) -> Option<&Symbol> {
        self.index.get(name).map(|&i| &self.symbols[i])
    }

    /// The analysis as JSON, for the in-game editor and the website: columns in characters.
    /// The layout is in `editors/README.md`.
    pub fn json(&self) -> String {
        self.json_value().compact()
    }

    pub(crate) fn json_value(&self) -> Json {
        let diags = |v: &[Diagnostic]| -> Json {
            Json::Arr(
                v.iter()
                    .map(|d| {
                        let meaning = crate::lang::get().explain(&d.message).map(|m| m.meaning.clone());
                        obj! { "file" => d.file.clone(), "line" => d.line, "start" => d.start, "end" => d.end, "message" => d.message.as_str(), "meaning" => meaning }
                    })
                    .collect(),
            )
        };
        let lines: Vec<Json> = self
            .main()
            .lines
            .iter()
            .filter(|l| l.addr.is_some() || l.copies > 0 && l.instruction.is_some())
            .map(|l| {
                let (mnemonic, mode) = match l.instruction {
                    Some((op, m)) => (Some(op.mnemonic()), Some(crate::lang::mode_key(m))),
                    None => (None, None),
                };
                obj! {
                    "line" => l.line, "addr" => l.addr, "bytes" => l.bytes.clone(), "mnemonic" => mnemonic, "mode" => mode,
                    "cycles" => l.cycles, "cycles_taken" => l.is_branch().then(|| l.cycles.map(|c| c + 1)).flatten(),
                    "copies" => l.copies, "total" => l.total_cycles,
                }
            })
            .collect();
        let blocks: Vec<Json> = self
            .blocks
            .iter()
            .filter(|b| b.file.is_none())
            .map(|b| {
                obj! {
                    "kind" => b.kind, "start" => b.start, "end" => b.end, "entered" => b.entered, "copies" => b.copies,
                    "cycles" => b.cycles, "cycles_per_entry" => b.cycles_per_entry(), "branches" => b.branches,
                }
            })
            .collect();
        let symbols: Vec<Json> = self
            .symbols
            .iter()
            .map(|s| obj! { "name" => s.name.as_str(), "value" => s.value, "kind" => s.kind.name(), "file" => s.file.clone(), "line" => s.line, "start" => s.start, "end" => s.end })
            .collect();
        let (start, size) = match &self.assembly {
            Some(a) => (a.start(), Some(a.segments.iter().map(|s| s.bytes.len()).sum::<usize>())),
            None => (None, None),
        };
        obj! {
            "ok" => self.errors.is_empty(),
            "start" => start,
            "size" => size,
            "errors" => diags(&self.errors),
            "warnings" => diags(&self.warnings),
            "lines" => Json::Arr(lines),
            "blocks" => Json::Arr(blocks),
            "symbols" => Json::Arr(symbols),
        }
    }
}

/// Byte columns to character columns, for JSON readers that index strings by character.
pub(crate) fn to_char_cols(a: &mut Analysis, src: &str, include: impl Fn(&str) -> Option<String>) {
    let texts: Vec<(Option<String>, String)> = std::iter::once((None, src.to_string())).chain(a.files.iter().skip(1).filter_map(|f| Some((f.name.clone(), include(f.name.as_deref()?)?)))).collect();
    let line_of = |file: &Option<String>, line: usize| -> String { texts.iter().find(|(n, _)| n == file).and_then(|(_, t)| t.lines().nth(line.wrapping_sub(1))).unwrap_or("").to_string() };
    for d in a.errors.iter_mut().chain(a.warnings.iter_mut()) {
        let l = line_of(&d.file, d.line);
        (d.start, d.end) = (char_col(&l, d.start), char_col(&l, d.end));
    }
    for s in &mut a.symbols {
        let l = line_of(&s.file, s.line);
        (s.start, s.end) = (char_col(&l, s.start), char_col(&l, s.end));
    }
}

/// `analyze(src)` as JSON, columns in characters: what the Godot and WebAssembly builds give.
pub fn analysis_json(src: &str) -> String {
    let mut a = analyze(src);
    to_char_cols(&mut a, src, |_| None);
    a.json()
}

/// `analyze_with(src, resolve)` as JSON, columns in characters.
pub fn analysis_json_with(src: &str, resolve: impl Fn(&str) -> Option<String>) -> String {
    let mut a = analyze_with(src, &resolve);
    to_char_cols(&mut a, src, &resolve);
    a.json()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_point_at_what_the_message_is_about() {
        fn span<'a>(line: &'a str, msg: &str) -> &'a str {
            let (s, e) = error_span(line, msg);
            &line[s..e]
        }
        assert_eq!(span("        LDQ #1   ; x", "unknown instruction 'LDQ'"), "LDQ");
        assert_eq!(span("        LDA nowhere+1", "undefined symbol 'nowhere'"), "nowhere");
        assert_eq!(span("        STA #1  ; no", "STA has no immediate mode"), "#1");
        assert_eq!(span("        JMP", "JMP needs an operand"), "JMP");
        assert_eq!(span("        .assert MYSET & $FF == 0", "'==' binds tighter than '&': add [ ] to say which you mean"), "==");
        assert_eq!(span("x: FOO = 1", "a constant can't have a label: put 'FOO = ...' on its own line, without 'x:'"), "x:");
        assert_eq!(span("        BNE @far", "undefined local label '@far' (after 'main')"), "@far");
        assert_eq!(span("        .rept 3", "'.rept' without '.endr'"), ".rept");
        assert_eq!(span("        .text \"A\\nB\"", "unknown escape '\\n'"), "\\n");
        assert_eq!(span("loop:   BNE far", "branch out of range (200 bytes)"), "far");
        assert_eq!(span("        NOP", "address $0900 is written twice (first by line 7)"), "NOP");
        assert_eq!(span("X = 1 / 0", "division by zero"), "1 / 0");
    }

    #[test]
    fn lines_cycles_symbols_and_blocks() {
        let src = "SCREEN = $0400\nstart:  LDX #39\n@col:   STA SCREEN,X\n        DEX\n        BPL @col\n        .rept 4, i\n        NOP\n        STA SCREEN+i\n        .endr\n        HLT\n";
        let a = analyze(src);
        assert!(a.errors.is_empty(), "{:?}", a.errors);
        let l = &a.main().lines;
        assert_eq!((l[1].addr, l[1].cycles, l[1].copies), (Some(0x0800), Some(2), 1));
        assert_eq!(l[2].defs, vec![SymRef { name: "start@col".into(), start: 0, end: 4 }]);
        assert_eq!(l[2].refs, vec![SymRef { name: "SCREEN".into(), start: 12, end: 18 }]);
        assert_eq!(l[2].cycles, Some(5));
        assert_eq!(l[4].refs[0].name, "start@col");
        assert!(l[4].is_branch());
        // `i` is the counter, not a symbol.
        assert_eq!(l[7].refs.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), vec!["SCREEN"]);
        assert_eq!((l[6].copies, l[6].total_cycles), (4, 8));
        assert_eq!(a.blocks.len(), 1);
        let b = &a.blocks[0];
        assert_eq!((b.start, b.end, b.entered, b.copies, b.cycles), (6, Some(9), 1, 4, 4 * (2 + 4)));
        assert_eq!(a.symbol("SCREEN").unwrap().kind, SymbolKind::Constant);
        assert_eq!(a.symbol("start@col").unwrap().value, Some(0x0802));
    }

    #[test]
    fn a_program_with_errors_still_has_addresses() {
        let a = analyze("start: LDA #1\n LDQ\n STA $C020\n JMP nowhere\n");
        // The line that can't be read, and the undefined name pass 2 finds.
        assert_eq!(a.errors.len(), 2, "{:?}", a.errors);
        assert_eq!(a.errors[1].message, "undefined symbol 'nowhere'");
        assert!(a.assembly.is_none());
        let l = &a.main().lines;
        assert_eq!((l[0].addr, l[2].addr, l[2].cycles), (Some(0x0800), Some(0x0802), Some(4)));
        assert_eq!(a.symbol("start").unwrap().value, Some(0x0800));
        assert_eq!((a.errors[0].line, a.errors[0].start, a.errors[0].end), (2, 1, 4));
        // Not a name a broken line would have defined: `here` would have been, `loop` not.
        let b = analyze("start: LDA #1\nloop LDA #2\n JMP loop\nhere: LDQ\n JMP here\n");
        let m: Vec<&str> = b.errors.iter().map(|e| e.message.as_str()).collect();
        assert_eq!(m, vec!["unknown instruction 'loop'", "undefined symbol 'loop'", "unknown instruction 'LDQ'"]);
        // An instruction pass 2 can't encode still has its cost.
        let c = analyze(" LDQ\n STA BORDER\n");
        assert_eq!(c.main().lines[1].cycles, Some(4));
    }
}
