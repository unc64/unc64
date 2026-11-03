//! `unc-asm SOURCE [-o OUT] [--range E000-FFFF] [--fill FF] [--sym FILE] [--lst FILE]
//! [--tape OUT.tap [--name NAME]]`
//! Assembles SOURCE into a flat binary covering the range (default: everything emitted).
//! `--tape` writes a tape image instead (manual chapter 14), named NAME or after SOURCE.
//! `unc-asm --basic LISTING.bas --tape OUT.tap [--name NAME]` puts a BASIC listing on tape
//! as a BASIC program (type 2): the listing is typed into a booted machine, which
//! tokenizes it as at the keyboard (`unc_asm::basic`).
//! `.include "name"` reads `name` relative to SOURCE's folder. `--sym` writes the symbol
//! table as `NAME = $XXXX` lines; `--lst` writes a listing of address, bytes and source.
//! Warnings (precedence traps) go to stderr as `file:line: warning: message`.
//!
//! For editor tools: `unc-asm --lang-json` prints the language data (`unc_asm::lang::json`,
//! schema in editors/README.md), `--vim-syntax` the Vim syntax file and `--textmate` the
//! VS Code grammar, both made from that data, and `--analysis-json SOURCE` what the assembler
//! makes of each line (`unc_asm::analysis`): addresses, bytes, cycles, symbols, errors.

use std::path::Path;
use std::{env, fs, process::ExitCode};

const USAGE: &str = "usage: unc-asm SOURCE [-o OUT] [--range E000-FFFF] [--fill FF] [--sym FILE] [--lst FILE] [--tape OUT.tap [--name NAME]]\n       unc-asm --basic LISTING.bas --tape OUT.tap [--name NAME]\n       unc-asm --lang-json | --vim-syntax | --textmate | --analysis-json SOURCE";

fn hex(s: &str) -> Option<u16> {
    u16::from_str_radix(s.trim_start_matches('$'), 16).ok()
}

fn write(path: &str, data: &[u8]) -> bool {
    match fs::write(path, data) {
        Ok(()) => true,
        Err(e) => {
            eprintln!("{path}: {e}");
            false
        }
    }
}

fn print(s: String) -> ExitCode {
    use std::io::Write;
    match std::io::stdout().write_all(s.as_bytes()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}

fn stem(src: &str) -> String {
    Path::new(src).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

/// `--basic`: the listing in `text` on a tape as a BASIC program.
fn basic_tape(src: &str, text: &str, tape: Option<String>, name: Option<String>) -> ExitCode {
    let Some(path) = tape else {
        eprintln!("{src}: --basic makes a tape: give --tape OUT.tap");
        return ExitCode::FAILURE;
    };
    let program = match unc_asm::basic::tokenize(text) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{src}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let p = unc_asm::tape::basic(&name.unwrap_or_else(|| stem(src)), &program);
    if !write(&path, &p.encode()) {
        return ExitCode::FAILURE;
    }
    let fits16 = if program.len() <= 0x3800 { "" } else { " (too big for a UNC 16)" };
    println!("{src}: {} BASIC, {} bytes{fits16} -> {path}", p.name(), program.len());
    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--lang-json") => return print(unc_asm::lang::json()),
        Some("--vim-syntax") => return print(unc_asm::lang::vim_syntax()),
        Some("--textmate") => return print(unc_asm::lang::textmate_grammar()),
        Some("--analysis-json") => {
            let Some(src) = args.get(1) else {
                eprintln!("{USAGE}");
                return ExitCode::FAILURE;
            };
            let text = match fs::read_to_string(src) {
                Ok(t) => t,
                Err(e) => { eprintln!("{src}: {e}"); return ExitCode::FAILURE }
            };
            let dir = Path::new(src).parent().map(Path::to_path_buf).unwrap_or_default();
            return print(unc_asm::analysis::analysis_json_with(&text, |name| fs::read_to_string(dir.join(name)).ok()) + "\n");
        }
        _ => {}
    }
    let mut src = None;
    let mut out = None;
    let mut sym = None;
    let mut lst = None;
    let mut tape = None;
    let mut name = None;
    let mut range = None;
    let mut fill = 0xFFu8;
    let mut basic = false;
    let mut i = 0;
    while i < args.len() {
        let next = args.get(i + 1).cloned();
        match args[i].as_str() {
            "-o" => { out = next; i += 1 }
            "--sym" => { sym = next; i += 1 }
            "--lst" => { lst = next; i += 1 }
            "--tape" => { tape = next; i += 1 }
            "--name" => { name = next; i += 1 }
            "--basic" => basic = true,
            "--range" => {
                range = next.as_deref().and_then(|r| r.split_once('-')).and_then(|(a, b)| Some((hex(a)?, hex(b)?)));
                i += 1
            }
            "--fill" => { fill = next.as_deref().and_then(hex).unwrap_or(0xFF) as u8; i += 1 }
            s => src = Some(s.to_string()),
        }
        i += 1;
    }
    let Some(src) = src.filter(|_| out.is_some() || sym.is_some() || lst.is_some() || tape.is_some()) else {
        eprintln!("{USAGE}");
        return ExitCode::FAILURE;
    };
    let text = match fs::read_to_string(&src) {
        Ok(t) => t,
        Err(e) => { eprintln!("{src}: {e}"); return ExitCode::FAILURE }
    };
    if basic {
        return basic_tape(&src, &text, tape, name);
    }
    let dir = Path::new(&src).parent().map(Path::to_path_buf).unwrap_or_default();
    let a = match unc_asm::assemble_with(&text, |name| fs::read_to_string(dir.join(name)).ok()) {
        Ok(a) => a,
        Err(errs) => {
            for e in errs {
                eprintln!("{}:{}: {}", e.file.as_deref().unwrap_or(&src), e.line, e.message);
            }
            return ExitCode::FAILURE;
        }
    };
    for w in &a.warnings {
        eprintln!("{}:{}: warning: {}", w.file.as_deref().unwrap_or(&src), w.line, w.message);
    }
    if let Some(out) = out {
        let (from, to) = range.unwrap_or_else(|| {
            let lo = a.segments.iter().map(|s| s.start).min().unwrap_or(0);
            let hi = a.segments.iter().map(|s| s.start as usize + s.bytes.len() - 1).max().unwrap_or(0) as u16;
            (lo, hi)
        });
        let img = a.image(from, to, fill);
        if !write(&out, &img) {
            return ExitCode::FAILURE;
        }
        println!("{src}: {} bytes ${from:04X}-${to:04X} -> {out}", img.len());
    }
    if let Some(path) = tape {
        let name = name.unwrap_or_else(|| stem(&src));
        let p = unc_asm::tape::program(&a, &name);
        if !write(&path, &p.encode()) {
            return ExitCode::FAILURE;
        }
        let end = p.load as usize + p.data.len();
        println!("{src}: {} ${:04X}-${:04X}, entry ${:04X} -> {path}", p.name(), p.load, end.max(1) - 1, p.entry);
    }
    if let Some(path) = sym
        && !write(&path, a.symbol_file().as_bytes())
    {
        return ExitCode::FAILURE;
    }
    if let Some(path) = lst
        && !write(&path, a.listing_text().as_bytes())
    {
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
