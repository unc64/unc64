//! The language features, each from a document and its analysis.

use crate::server::{Doc, Server, Settings, TOKEN_MODIFIERS, TOKEN_TYPES};
use crate::text::{byte_col, lines, utf16_col};
use serde_json::{Value, json};
use unc_asm::analysis::{SourceLine, SymRef, Symbol, SymbolKind};
use unc_asm::lang::{self, Named, Token, TokenKind, tokens};
use unc_core::isa::Op;

type Pos = (usize, u32);

fn range(line: usize, l: &str, start: usize, end: usize) -> Value {
    json!({ "start": { "line": line, "character": utf16_col(l, start) }, "end": { "line": line, "character": utf16_col(l, end) } })
}

fn markdown(s: String) -> Value {
    json!({ "kind": "markdown", "value": s })
}

/// `$0812`, or `$12` for the fast page; `-5` for negatives.
fn hex(v: i64) -> String {
    match v {
        0..=0xFF => format!("${v:02X}"),
        0x100..=0xFFFF => format!("${v:04X}"),
        v if v < 0 => v.to_string(),
        v => format!("${v:X}"),
    }
}

/// The token under the cursor: the one containing it, else the one ending at it.
fn token_at(l: &str, at: usize) -> Option<Token> {
    let toks = tokens(l);
    toks.iter().find(|t| t.start <= at && at < t.end).or_else(|| toks.iter().find(|t| t.end == at && t.kind != TokenKind::Operator)).copied()
}

fn source_line(doc: &Doc, line: usize) -> Option<&SourceLine> {
    doc.analysis.main().lines.get(line)
}

/// The name defined or used under the cursor, as the assembler knows it.
fn symref_at(doc: &Doc, (line, col): Pos) -> Option<&SymRef> {
    let at = byte_col(doc.line(line), col);
    let sl = source_line(doc, line)?;
    sl.defs.iter().chain(&sl.refs).find(|r| r.start <= at && at <= r.end)
}

/// The global label a line's local labels belong to: the last one at or above it.
fn scope_at(doc: &Doc, line: usize) -> String {
    let main = doc.analysis.main();
    main.lines[..(line + 1).min(main.lines.len())]
        .iter()
        .rev()
        .flat_map(|l| l.defs.iter().rev())
        .find(|d| !d.name.contains('@') && doc.analysis.symbol(&d.name).is_some_and(|s| s.kind == SymbolKind::Label))
        .map(|d| d.name.clone())
        .unwrap_or_default()
}

// ---------- completion ----------

/// Where a definition (`BORDER  = $C020`) goes: after the last constant at the top of the
/// program, else after the comments that open it.
fn definition_line(doc: &Doc) -> usize {
    let ls = lines(&doc.text);
    let mut last_const = None;
    let mut first_code = None;
    for (i, l) in ls.iter().enumerate() {
        match tokens(l).first().map(|t| t.kind) {
            Some(TokenKind::Constant) => last_const = Some(i),
            Some(TokenKind::Comment) | None => {}
            Some(_) => {
                first_code = Some(i);
                break;
            }
        }
    }
    match (last_const, first_code) {
        (Some(c), _) => c + 1,
        (None, Some(f)) => {
            // Before the code, after the comment block above it.
            let mut i = 0;
            while i < f && tokens(ls[i]).first().is_some_and(|t| t.kind == TokenKind::Comment) {
                i += 1;
            }
            if i > 0 { (i + 1).min(f) } else { 0 }
        }
        (None, None) => ls.len(),
    }
}

fn define_edit(doc: &Doc, n: &Named) -> Value {
    let at = definition_line(doc);
    json!({ "range": { "start": { "line": at, "character": 0 }, "end": { "line": at, "character": 0 } }, "newText": format!("{}\n", n.definition()) })
}

fn instruction_doc(i: &lang::Instruction) -> String {
    let mut s = format!("**{}** {}. Flags: {}\n\n| Mode | Syntax | Opcode | Bytes | Cycles |\n|---|---|---|---|---|\n", i.mnemonic, i.description, i.flags);
    for m in &i.modes {
        let cyc = match m.cycles_taken {
            Some(t) => format!("{} ({t} taken)", m.cycles),
            None => m.cycles.to_string(),
        };
        s.push_str(&format!("| {} | `{}` | ${:02X} | {} | {cyc} |\n", m.name, m.syntax, m.opcode, m.bytes));
    }
    s
}

pub fn completion(doc: &Doc, (line, col): Pos, snippets: bool) -> Value {
    let l = doc.line(line);
    let at = byte_col(l, col);
    if let Some(t) = token_at(l, at)
        && matches!(t.kind, TokenKind::Comment | TokenKind::String)
        && t.start < at
        && (t.kind == TokenKind::Comment || at < t.end)
    {
        return json!([]);
    }
    let b = l.as_bytes();
    let mut ps = at;
    while ps > 0 && (b[ps - 1].is_ascii_alphanumeric() || b"_@.".contains(&b[ps - 1])) {
        ps -= 1;
    }
    let word = &l[ps..at];
    let head = &l[..ps];
    let edit = |text: &str| json!({ "range": range(line, l, ps, at), "newText": text });
    let lang = lang::get();
    let mut items: Vec<Value> = Vec::new();

    // At the start of a statement: after nothing, or after a label.
    let head_code = head.trim();
    let statement = head_code.is_empty() || (head_code.ends_with(':') && tokens(head_code).iter().all(|t| matches!(t.kind, TokenKind::Label | TokenKind::Local | TokenKind::Operator)));
    if statement {
        if head.is_empty() && !word.starts_with('.') {
            // Column 0 is where names are defined: offer the manual's, as constants.
            for n in lang.named().filter(|n| doc.analysis.symbol(n.name()).is_none()) {
                items.push(json!({
                    "label": n.definition().split_whitespace().collect::<Vec<_>>().join(" "),
                    "filterText": n.name(),
                    "kind": 21,
                    "detail": format!("{} {}", n.what(), lang::range_hex(n.address(), n.address())),
                    "documentation": markdown(n.detail().join("\n")),
                    "textEdit": edit(&n.definition()),
                    "sortText": format!("3{}", n.name()),
                }));
            }
        }
        if !word.starts_with('.') {
            for i in &lang.instructions {
                let cycles: Vec<String> = i.modes.iter().map(|m| m.cycles.to_string()).collect();
                items.push(json!({
                    "label": i.mnemonic,
                    "kind": 14,
                    "detail": format!("{} · {} cycles", i.description, cycles.join("/")),
                    "documentation": markdown(instruction_doc(i)),
                    "textEdit": edit(i.mnemonic),
                    "sortText": format!("1{}", i.mnemonic),
                }));
                if snippets && i.modes.len() > 1 {
                    for (k, m) in i.modes.iter().enumerate() {
                        items.push(json!({
                            "label": m.syntax,
                            "filterText": i.mnemonic,
                            "kind": 15,
                            "detail": format!("{} · {} bytes · {} cycles", m.name, m.bytes, m.cycles),
                            "textEdit": edit(&m.snippet),
                            "insertTextFormat": 2,
                            "sortText": format!("2{}{k:02}", i.mnemonic),
                        }));
                    }
                }
            }
        }
        for d in &lang.directives {
            for name in std::iter::once(&d.name).chain(&d.aliases) {
                let (text, format) = if snippets && name == &d.name { (d.snippet.to_string(), 2) } else { (name.clone(), 1) };
                items.push(json!({
                    "label": name,
                    "kind": 14,
                    "detail": d.syntax,
                    "documentation": markdown(format!("`{}`\n\n{}\n\nExample: `{}`", d.syntax, d.description, d.example)),
                    "textEdit": edit(&text),
                    "insertTextFormat": format,
                    "sortText": format!("1{name}"),
                }));
            }
        }
        return json!({ "isIncomplete": false, "items": items });
    }

    // An operand or a directive's values.
    if head_code.to_ascii_lowercase().split_whitespace().any(|w| w == ".include") {
        return json!([]);
    }
    let scope = scope_at(doc, line);
    for s in &doc.analysis.symbols {
        let (label, kind) = match s.kind {
            SymbolKind::Local => match s.name.strip_prefix(&format!("{scope}@")) {
                Some(local) => (format!("@{local}"), 3),
                None => continue,
            },
            SymbolKind::Label => (s.name.clone(), 3),
            SymbolKind::Constant => (s.name.clone(), 21),
        };
        let where_ = match &s.file {
            Some(f) => format!("{f}:{}", s.line),
            None => format!("line {}", s.line),
        };
        let value = s.value.map(hex).unwrap_or_else(|| "?".into());
        items.push(json!({
            "label": label,
            "kind": kind,
            "detail": format!("{} = {value} ({where_})", s.kind.name()),
            "textEdit": edit(&label),
            "sortText": format!("0{label}"),
        }));
    }
    for n in lang.named().filter(|n| doc.analysis.symbol(n.name()).is_none()) {
        items.push(json!({
            "label": n.name(),
            "kind": 21,
            "detail": format!("{} {}: adds {}", n.what(), lang::range_hex(n.address(), n.address()), n.definition().split_whitespace().collect::<Vec<_>>().join(" ")),
            "documentation": markdown(format!("{}\n\nThe UNC has no built-in names: accepting this adds `{}` to the program.", n.detail().join("\n"), n.definition())),
            "textEdit": edit(n.name()),
            "additionalTextEdits": [define_edit(doc, &n)],
            "sortText": format!("1{}", n.name()),
        }));
    }
    for f in &lang.functions {
        let (text, format) = if snippets { (f.snippet.clone(), 2) } else { (f.name.to_string(), 1) };
        items.push(json!({
            "label": f.name,
            "kind": 2,
            "detail": f.syntax,
            "documentation": markdown(f.description.clone()),
            "textEdit": edit(&text),
            "insertTextFormat": format,
            "sortText": format!("2{}", f.name),
        }));
    }
    json!({ "isIncomplete": false, "items": items })
}

// ---------- hover ----------

fn number_value(t: &str) -> Option<i64> {
    let b = t.as_bytes();
    match b.first()? {
        b'$' => u64::from_str_radix(&t[1..], 16).ok().map(|v| v as i64),
        b'%' => u64::from_str_radix(&t[1..], 2).ok().map(|v| v as i64),
        b'\'' => match &b[1..] {
            [b'\\', c, b'\''] => Some(*c as i64),
            _ => t[1..t.len() - 1].chars().next().map(|c| c as i64),
        },
        _ => t.parse().ok(),
    }
}

/// The value in hex, decimal and binary.
fn bases(v: i64) -> String {
    if (0..=0xFFFF).contains(&v) {
        let bin = if v < 0x100 { format!("%{v:08b}") } else { format!("%{v:016b}") };
        format!("`{}` = `{v}` = `{bin}`", hex(v))
    } else {
        format!("`{}` = `{v}`", hex(v))
    }
}

/// What the manual says lives at an address.
fn address_notes(v: i64) -> Vec<String> {
    if !(0..=0xFFFF).contains(&v) {
        return Vec::new();
    }
    let (named, region) = lang::get().at_address(v as u16);
    let mut out: Vec<String> = named.iter().flat_map(|n| n.detail()).collect();
    if let Some(r) = region {
        out.push(format!("Memory map `{}`: {}", lang::range_hex(r.start, r.end), r.description));
    }
    out
}

fn instruction_hover(doc: &Doc, line: usize, op: Op) -> String {
    let lang = lang::get();
    let i = lang.instruction(op.mnemonic()).expect("every op is in the language");
    let mut s = String::new();
    match source_line(doc, line).filter(|sl| sl.instruction.is_some_and(|(o, _)| o == op)) {
        Some(sl) => {
            let (_, mode) = sl.instruction.expect("filtered");
            let m = i.modes.iter().find(|m| m.mode == mode).expect("the mode is in the table");
            let cycles = match m.cycles_taken {
                Some(t) => format!("**{} cycles**, {t} if taken", m.cycles),
                None => format!("**{} cycles**", m.cycles),
            };
            s.push_str(&format!("**{}** {}: `{}`\n\n{} · {} bytes · opcode `${:02X}` · flags {}\n\n", i.mnemonic, m.name, m.syntax, cycles, m.bytes, m.opcode, i.flags));
            s.push_str(&format!("{}.", i.description));
            if let Some(a) = sl.addr {
                let bytes: Vec<String> = sl.bytes.iter().map(|b| format!("{b:02X}")).collect();
                s.push_str(&format!(" Here: `${a:04X}: {}`", bytes.join(" ")));
                if sl.copies > 1 {
                    s.push_str(&format!(", assembled {} times: {} cycles in all", sl.copies, sl.total_cycles));
                }
            }
            s.push_str("\n\n---\n\n");
            s.push_str(&instruction_doc(i));
        }
        None => s.push_str(&instruction_doc(i)),
    }
    s
}

fn symbol_hover(doc: &Doc, sym: &Symbol, written: &str) -> String {
    let mut s = match sym.value {
        Some(v) => format!("**{written}** {} = {}", sym.kind.name(), bases(v)),
        None => format!("**{written}** {} (value not worked out: the program has errors)", sym.kind.name()),
    };
    if sym.kind == SymbolKind::Local
        && let Some((global, _)) = sym.name.split_once('@')
    {
        s.push_str(&format!(", under `{}`", if global.is_empty() { "(no label)" } else { global }));
    }
    let def_line = match &sym.file {
        None => doc.line(sym.line - 1).trim().to_string(),
        Some(_) => String::new(),
    };
    let place = match &sym.file {
        Some(f) => format!("{f}, line {}", sym.line),
        None => format!("line {}", sym.line),
    };
    s.push_str(&format!("\n\nDefined at {place}"));
    if !def_line.is_empty() {
        s.push_str(&format!(": `{def_line}`"));
    }
    let mut notes = Vec::new();
    if let Some(n) = lang::get().named_as(&sym.name)
        && sym.value != Some(n.address() as i64)
    {
        notes.push(format!("Note: the manual's {} {} is at `{}`.", n.what(), n.name(), lang::range_hex(n.address(), n.address())));
    }
    if let Some(v) = sym.value
        && sym.kind == SymbolKind::Constant
    {
        notes.extend(address_notes(v).into_iter().filter(|l| !l.starts_with("Memory map") || v >= 0x100));
    }
    if !notes.is_empty() {
        s.push_str("\n\n---\n\n");
        s.push_str(&notes.join("\n\n"));
    }
    s
}

pub fn hover(doc: &Doc, (line, col): Pos) -> Value {
    let l = doc.line(line);
    let at = byte_col(l, col);
    let Some(t) = token_at(l, at) else { return Value::Null };
    let text = t.text(l);
    let lang = lang::get();
    let md = match t.kind {
        TokenKind::Mnemonic => Op::from_mnemonic(text).map(|op| instruction_hover(doc, line, op)),
        TokenKind::Directive => lang.directive(text).map(|d| format!("**{}** `{}`\n\n{}\n\nExample: `{}`", d.name, d.syntax, d.description, d.example)),
        TokenKind::Function => lang.function(text).map(|f| format!("**{}** (function, {} value{})\n\n{}", f.syntax, f.arity, if f.arity == 1 { "" } else { "s" }, f.description)),
        TokenKind::Label | TokenKind::Local | TokenKind::Constant | TokenKind::Symbol => {
            let name = symref_at(doc, (line, col)).map(|r| r.name.clone());
            match name.as_deref().and_then(|n| doc.analysis.symbol(n)) {
                Some(sym) => Some(symbol_hover(doc, sym, text)),
                None => match lang.named_as(text) {
                    Some(n) => Some(format!("{}\n\nNot defined in this program. The UNC has no built-in names: add `{}`.", n.detail().join("\n"), n.definition())),
                    None if name.is_none() => Some(format!("**{text}**: a `.rept` counter or `.irp` name")),
                    None => Some(format!("**{text}**: undefined")),
                },
            }
        }
        TokenKind::Number => number_value(text).map(|v| {
            let immediate = tokens(l).iter().any(|p| p.end <= t.start && p.kind == TokenKind::Operator && p.text(l) == "#");
            let mut s = bases(v);
            if text.starts_with('\'') {
                s = format!("`{text}` = {s}");
            }
            if !immediate {
                let notes = address_notes(v);
                if !notes.is_empty() {
                    s.push_str("\n\n---\n\n");
                    s.push_str(&notes.join("\n\n"));
                }
            }
            s
        }),
        TokenKind::Register => Some(match text.to_ascii_uppercase().as_str() {
            "A" => "**A**, the accumulator: the shifts and rotates work on it".to_string(),
            r => format!("**{r}**, the index register: the address plus {r}"),
        }),
        _ => None,
    };
    match md {
        Some(md) => json!({ "contents": markdown(md), "range": range(line, l, t.start, t.end) }),
        None => Value::Null,
    }
}

// ---------- inlay hints ----------

fn cycles_label(c: u8, branch: bool) -> String {
    if branch { format!("{c}/{}c", c + 1) } else { format!("{c}c") }
}

/// Each instruction's cost at the end of its line (`4c`; a branch `2/3c`, not taken/taken;
/// `×8` when a `.rept` assembles it 8 times), each `.rept`/`.irp` block's total on its first
/// line, and with `cumulative_cycles`, the cycles since the last label: `4c  Σ23`.
pub fn inlay_hints(doc: &Doc, r: &Value, settings: &Settings) -> Value {
    let from = r["start"]["line"].as_u64().unwrap_or(0) as usize;
    let to = r["end"]["line"].as_u64().unwrap_or(u64::MAX) as usize;
    let a = &doc.analysis;
    let mut out = Vec::new();
    let mut run = 0u64;
    let mut run_branch = false;
    for (i, sl) in a.main().lines.iter().enumerate() {
        let labelled = sl.defs.iter().any(|d| a.symbol(&d.name).is_some_and(|s| s.kind != SymbolKind::Constant));
        if labelled {
            run = 0;
            run_branch = false;
        }
        let mut parts: Vec<String> = Vec::new();
        let mut tip: Vec<String> = Vec::new();
        if let (Some(c), Some((op, mode))) = (sl.cycles, sl.instruction) {
            run += sl.total_cycles;
            run_branch |= sl.is_branch();
            let mut p = cycles_label(c, sl.is_branch());
            tip.push(match sl.is_branch() {
                true => format!("{} {}: {c} cycles, {} if the branch is taken", op.mnemonic(), mode.name(), c + 1),
                false => format!("{} {}: {c} cycles", op.mnemonic(), mode.name()),
            });
            if sl.copies > 1 {
                p.push_str(&format!(" ×{}", sl.copies));
                tip.push(format!("assembled {} times: {} cycles", sl.copies, sl.total_cycles));
            }
            if settings.cumulative_cycles {
                p.push_str(&format!("  Σ{run}{}", if run_branch { "+" } else { "" }));
                tip.push(format!("{run} cycles since the last label{}", if run_branch { ", branches not taken" } else { "" }));
            }
            parts.push(p);
        }
        for b in a.blocks.iter().filter(|b| b.file.is_none() && b.start == i + 1) {
            let per_entry = b.cycles_per_entry();
            let n = b.copies_per_entry();
            if per_entry == 0 {
                continue;
            }
            let mut p = if n > 0 && per_entry % n as u64 == 0 { format!("{n} × {} = {per_entry} cycles", per_entry / n as u64) } else { format!("{n} copies = {per_entry} cycles") };
            if b.branches {
                p.push_str(" + taken branches");
            }
            if b.entered > 1 {
                p.push_str(&format!(", ×{} = {}", b.entered, b.cycles));
            }
            tip.push(format!(".{} block, lines {}-{}: {per_entry} cycles straight through", b.kind, b.start, b.end.map_or("?".to_string(), |e| e.to_string())));
            parts.push(p);
        }
        if parts.is_empty() || i < from || i > to {
            continue;
        }
        let l = doc.line(i);
        out.push(json!({
            "position": { "line": i, "character": utf16_col(l, l.trim_end().len()) },
            "label": parts.join("  "),
            "paddingLeft": true,
            "tooltip": tip.join("\n"),
        }));
    }
    Value::Array(out)
}

// ---------- navigation ----------

fn location(uri: &str, line: usize, l: &str, start: usize, end: usize) -> Value {
    json!({ "uri": uri, "range": range(line, l, start, end) })
}

pub fn definition(server: &Server, doc: &Doc, pos: Pos) -> Value {
    let l = doc.line(pos.0);
    // `.include "file.s"`: the file.
    if let Some(t) = token_at(l, byte_col(l, pos.1))
        && t.kind == TokenKind::String
        && tokens(l).iter().any(|d| d.kind == TokenKind::Directive && d.text(l).eq_ignore_ascii_case(".include"))
    {
        let name = t.text(l).trim_matches('"');
        return json!({ "uri": doc.file_uri(Some(name)), "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 0 } } });
    }
    let Some(r) = symref_at(doc, pos) else { return Value::Null };
    let Some(s) = doc.analysis.symbol(&r.name) else { return Value::Null };
    let text = server.file_text(doc, s.file.as_deref()).unwrap_or_default();
    let line = text.lines().nth(s.line - 1).unwrap_or("");
    location(&doc.file_uri(s.file.as_deref()), s.line - 1, line, s.start, s.end)
}

/// Every place a symbol is defined or used, in the document and the files it includes:
/// (file, line from 0, start, end).
fn occurrences(doc: &Doc, name: &str, with_defs: bool) -> Vec<(Option<String>, usize, usize, usize)> {
    let mut out = Vec::new();
    for f in &doc.analysis.files {
        for sl in &f.lines {
            let defs = sl.defs.iter().filter(|_| with_defs);
            for r in defs.chain(&sl.refs).filter(|r| r.name == name) {
                out.push((f.name.clone(), sl.line - 1, r.start, r.end));
            }
        }
    }
    out
}

pub fn references(server: &Server, doc: &Doc, pos: Pos, with_decl: bool) -> Value {
    let Some(r) = symref_at(doc, pos) else { return json!([]) };
    let mut texts: Vec<(Option<String>, Vec<String>)> = Vec::new();
    let mut out = Vec::new();
    for (file, line, s, e) in occurrences(doc, &r.name, with_decl) {
        if !texts.iter().any(|(f, _)| *f == file) {
            let t = server.file_text(doc, file.as_deref()).unwrap_or_default();
            texts.push((file.clone(), t.lines().map(str::to_string).collect()));
        }
        let lines = &texts.iter().find(|(f, _)| *f == file).expect("read").1;
        out.push(location(&doc.file_uri(file.as_deref()), line, lines.get(line).map_or("", String::as_str), s, e));
    }
    Value::Array(out)
}

pub fn document_symbols(doc: &Doc) -> Value {
    let a = &doc.analysis;
    let n_lines = lines(&doc.text).len().max(1);
    let main: Vec<&Symbol> = a.symbols.iter().filter(|s| s.file.is_none()).collect();
    let globals: Vec<usize> = main.iter().filter(|s| s.kind == SymbolKind::Label).map(|s| s.line).collect();
    let item = |s: &Symbol, end_line: usize| -> Value {
        let l = doc.line(s.line - 1);
        let kind = match s.kind {
            SymbolKind::Label => 12,
            SymbolKind::Local => 6,
            SymbolKind::Constant => 14,
        };
        let shown = match s.kind {
            SymbolKind::Local => s.name.split_once('@').map_or(s.name.clone(), |(_, l)| format!("@{l}")),
            _ => s.name.clone(),
        };
        let end_text = doc.line(end_line - 1);
        json!({
            "name": shown,
            "kind": kind,
            "detail": s.value.map(hex).unwrap_or_default(),
            "range": { "start": { "line": s.line - 1, "character": 0 }, "end": { "line": end_line - 1, "character": utf16_col(end_text, end_text.len()) } },
            "selectionRange": range(s.line - 1, l, s.start, s.end),
            "children": [],
        })
    };
    let mut top: Vec<Value> = Vec::new();
    for s in &main {
        match s.kind {
            SymbolKind::Label => {
                let end = globals.iter().find(|&&g| g > s.line).map_or(n_lines, |g| g - 1).max(s.line);
                top.push(item(s, end));
            }
            _ => {
                let parent = globals.iter().rev().find(|&&g| g <= s.line).and_then(|g| top.iter_mut().rev().find(|v| v["range"]["start"]["line"] == *g - 1 && v["kind"] == 12));
                match parent {
                    Some(p) => p["children"].as_array_mut().expect("children").push(item(s, s.line)),
                    None => top.push(item(s, s.line)),
                }
            }
        }
    }
    Value::Array(top)
}

pub fn prepare_rename(doc: &Doc, pos: Pos) -> Value {
    let Some(r) = symref_at(doc, pos) else { return Value::Null };
    if doc.analysis.symbol(&r.name).is_none() {
        return Value::Null;
    }
    let l = doc.line(pos.0);
    json!({ "range": range(pos.0, l, r.start, r.end), "placeholder": &l[r.start..r.end] })
}

fn valid_name(s: &str) -> bool {
    let b = s.as_bytes();
    !b.is_empty() && (b[0].is_ascii_alphabetic() || b[0] == b'_') && b.iter().all(|c| c.is_ascii_alphanumeric() || *c == b'_')
}

pub fn rename(server: &Server, doc: &Doc, pos: Pos, new_name: &str) -> Result<Value, String> {
    let r = symref_at(doc, pos).ok_or("no label or constant here")?;
    let sym = doc.analysis.symbol(&r.name).ok_or("only names this program defines can be renamed")?;
    let local = sym.kind == SymbolKind::Local;
    let bare = new_name.trim().trim_start_matches('@');
    if !valid_name(bare) || bare.eq_ignore_ascii_case("a") {
        return Err(format!("'{new_name}' can't be a name"));
    }
    let (scope, _) = sym.name.split_once('@').unwrap_or(("", ""));
    let new_full = if local { format!("{scope}@{bare}") } else { bare.to_string() };
    if new_full != sym.name && doc.analysis.symbol(&new_full).is_some() {
        return Err(format!("'{new_full}' is already defined"));
    }
    // Every occurrence, and for a global label, the `label@local` names written with it.
    let mut edits: Vec<(Option<String>, usize, usize, usize, String)> = Vec::new();
    for (file, line, s, e) in occurrences(doc, &sym.name, true) {
        let text = if local { format!("@{bare}") } else { bare.to_string() };
        edits.push((file, line, s, e, text));
    }
    if !local {
        let prefix = format!("{}@", sym.name);
        let locals: Vec<String> = doc.analysis.symbols.iter().filter(|s| s.name.starts_with(&prefix)).map(|s| s.name.clone()).collect();
        for name in locals {
            for (file, line, s, e) in occurrences(doc, &name, false) {
                edits.push((file, line, s, e, String::new()));
            }
        }
    }
    let mut changes = serde_json::Map::new();
    for (file, line, s, e, text) in edits {
        let src = server.file_text(doc, file.as_deref()).unwrap_or_default();
        let l = src.lines().nth(line).unwrap_or("");
        let written = &l[s..e];
        let (s, e, text) = match written.find('@') {
            // `clear@loop` written out: rename its part.
            Some(at) if at > 0 && local => (s + at, e, text),
            Some(at) if at > 0 && text.is_empty() => (s, s + at, bare.to_string()),
            Some(_) if text.is_empty() => continue,
            _ => (s, e, text),
        };
        let uri = doc.file_uri(file.as_deref());
        changes.entry(uri).or_insert_with(|| json!([])).as_array_mut().expect("array").push(json!({ "range": range(line, l, s, e), "newText": text }));
    }
    Ok(json!({ "changes": changes }))
}

// ---------- semantic tokens, code actions, folding ----------

fn token_type(t: &str) -> u32 {
    TOKEN_TYPES.iter().position(|x| *x == t).expect("in the legend") as u32
}

fn modifiers(ms: &[&str]) -> u32 {
    ms.iter().map(|m| 1 << TOKEN_MODIFIERS.iter().position(|x| x == m).expect("in the legend")).sum()
}

pub fn semantic_tokens(doc: &Doc) -> Value {
    let a = &doc.analysis;
    let mut data: Vec<u32> = Vec::new();
    let (mut prev_line, mut prev_col) = (0u32, 0u32);
    for (i, l) in lines(&doc.text).iter().enumerate() {
        let sl = a.main().lines.get(i);
        for t in tokens(l) {
            let resolved = || sl.and_then(|sl| sl.defs.iter().chain(&sl.refs).find(|r| r.start == t.start)).and_then(|r| a.symbol(&r.name));
            let (ty, mods): (&str, &[&str]) = match t.kind {
                TokenKind::Mnemonic => ("keyword", &[]),
                TokenKind::Directive => ("macro", &[]),
                TokenKind::Label => ("label", &["declaration"]),
                TokenKind::Local => ("label", if sl.is_some_and(|sl| sl.defs.iter().any(|d| d.start == t.start)) { &["declaration"] } else { &[] }),
                TokenKind::Constant => ("variable", &["declaration", "readonly"]),
                TokenKind::Symbol => match resolved().map(|s| s.kind) {
                    Some(SymbolKind::Constant) => ("variable", &["readonly"]),
                    Some(_) => ("label", &[]),
                    None => ("variable", &[]),
                },
                TokenKind::Number => ("number", &[]),
                TokenKind::String => ("string", &[]),
                TokenKind::Comment => ("comment", &[]),
                TokenKind::Operator => ("operator", &[]),
                TokenKind::Register => ("variable", &["defaultLibrary"]),
                TokenKind::Function => ("function", &["defaultLibrary"]),
                TokenKind::Invalid => continue,
            };
            let (line, col) = (i as u32, utf16_col(l, t.start));
            let len = utf16_col(l, t.end) - col;
            let dl = line - prev_line;
            let dc = if dl == 0 { col - prev_col } else { col };
            data.extend([dl, dc, len, token_type(ty), modifiers(mods)]);
            (prev_line, prev_col) = (line, col);
        }
    }
    json!({ "data": data })
}

/// Quick fixes: define an I/O register, ROM routine or system variable the program uses but
/// hasn't defined (`undefined symbol 'BORDER'` → `BORDER  = $C020`).
pub fn code_actions(doc: &Doc, r: &Value) -> Value {
    let from = r["start"]["line"].as_u64().unwrap_or(0) as usize;
    let to = r["end"]["line"].as_u64().unwrap_or(u64::MAX) as usize;
    let lang = lang::get();
    let mut out = Vec::new();
    let mut seen = Vec::new();
    for e in doc.analysis.errors.iter().filter(|e| e.file.is_none() && (from..=to).contains(&(e.line - 1))) {
        let Some(name) = e.message.strip_prefix("undefined symbol '").and_then(|m| m.strip_suffix('\'')) else { continue };
        let Some(n) = lang.named_as(name) else { continue };
        if seen.contains(&name) {
            continue;
        }
        seen.push(name);
        let l = doc.line(e.line - 1);
        out.push(json!({
            "title": format!("Define {} ({} {})", n.definition().split_whitespace().collect::<Vec<_>>().join(" "), n.what(), lang::range_hex(n.address(), n.address())),
            "kind": "quickfix",
            "isPreferred": true,
            "diagnostics": [{ "range": range(e.line - 1, l, e.start, e.end), "severity": 1, "source": "unc-asm", "message": e.message }],
            "edit": { "changes": { doc.uri.clone(): [define_edit(doc, &n)] } },
        }));
    }
    Value::Array(out)
}

/// `.rept`/`.irp` blocks fold.
pub fn folding_ranges(doc: &Doc) -> Value {
    Value::Array(doc.analysis.blocks.iter().filter(|b| b.file.is_none()).filter_map(|b| Some(json!({ "startLine": b.start - 1, "endLine": b.end? - 1 }))).collect())
}
