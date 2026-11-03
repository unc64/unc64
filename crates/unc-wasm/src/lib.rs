//! The UNC 64 for the browser: a plain C ABI over one machine instance.
//! Strings cross the boundary through two byte buffers: JS writes source into the input
//! buffer, and reads JSON or text back from the output buffer.

use std::cell::RefCell;
use unc_asm::{assemble, disasm};
use unc_core::cpu::CpuState;
use unc_core::machine::{Machine, PIXELS_H, PIXELS_W, RamSize, Stop};

thread_local! {
    static M: RefCell<Machine> = RefCell::new(Machine::new(RamSize::K16));
    static INPUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static OUTPUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static PIXELS: RefCell<Vec<u8>> = RefCell::new(vec![0; PIXELS_W * PIXELS_H]);
    static AUDIO: RefCell<Vec<f32>> = const { RefCell::new(Vec::new()) };
    /// Whether the next `power_on` puts the video board in (`set_video_board`).
    static BOARD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// The last program `assemble_project_input` assembled, for `make_tape_input`.
    static LAST: RefCell<Option<unc_asm::Assembly>> = const { RefCell::new(None) };
}

fn m<R>(f: impl FnOnce(&mut Machine) -> R) -> R {
    M.with(|m| f(&mut m.borrow_mut()))
}

fn out(s: String) -> u32 {
    let n = s.len() as u32;
    OUTPUT.with(|o| *o.borrow_mut() = s.into_bytes());
    n
}

fn stop_code(s: Stop) -> u32 {
    match s {
        Stop::Budget => 0,
        Stop::Breakpoint(_) => 1,
        Stop::Halted => 2,
        Stop::Jammed(_) => 3,
    }
}

fn json_str(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

#[unsafe(no_mangle)]
pub extern "C" fn input_buffer(len: u32) -> *mut u8 {
    INPUT.with(|i| {
        let mut i = i.borrow_mut();
        i.resize(len as usize, 0);
        i.as_mut_ptr()
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn output_ptr() -> *const u8 {
    OUTPUT.with(|o| o.borrow().as_ptr())
}

/// Power-cycles the machine with 16 or 64 KB of RAM and lets the ROM boot. The video board
/// is in if `set_video_board(1)` was called before.
#[unsafe(no_mangle)]
pub extern "C" fn power_on(ram_kb: u32) {
    let board = BOARD.with(|b| b.get());
    m(|m| *m = Machine::boot_with_board(if ram_kb >= 64 { RamSize::K64 } else { RamSize::K16 }, board));
}

/// Puts the video board in (1) or takes it out (0) at the next `power_on` (manual chapter
/// 15). The machine running now doesn't change.
#[unsafe(no_mangle)]
pub extern "C" fn set_video_board(on: u32) {
    BOARD.with(|b| b.set(on != 0));
}

/// 1 if the machine running now has the video board in.
#[unsafe(no_mangle)]
pub extern "C" fn has_video_board() -> u32 {
    m(|m| m.has_board() as u32)
}

#[unsafe(no_mangle)]
pub extern "C" fn reset() {
    m(|m| m.reset());
}

/// Assembles the input buffer. On success loads the program and sets the start state
/// (`unc_asm::load`: PC = start, SP = $FF, I set, A = X = Y = 0). Writes JSON to the
/// output buffer and returns its length. `.include` reports that includes aren't available.
#[unsafe(no_mangle)]
pub extern "C" fn assemble_input() -> u32 {
    let src = INPUT.with(|i| String::from_utf8_lossy(&i.borrow()).into_owned());
    match assemble(&src) {
        Err(errs) => {
            let list: Vec<String> = errs.iter().map(|e| format!("[{},{}]", e.line, json_str(&e.message))).collect();
            out(format!("{{\"ok\":false,\"errors\":[{}]}}", list.join(",")))
        }
        Ok(a) => {
            let start = m(|m| unc_asm::load(m, &a));
            let lines: Vec<String> = a.lines.iter().map(|l| format!("[{},{},{}]", l.line, l.addr, l.len)).collect();
            let syms: Vec<String> = a.symbols.iter().map(|(n, v)| format!("[{},{v}]", json_str(n))).collect();
            let size: usize = a.segments.iter().map(|s| s.bytes.len()).sum();
            out(format!("{{\"ok\":true,\"start\":{start},\"size\":{size},\"lines\":[{}],\"symbols\":[{}]}}", lines.join(","), syms.join(",")))
        }
    }
}

/// Assembles a project: the input buffer holds its files as `path NUL text NUL path NUL text
/// ...`, the main file first; `.include "path"` reads the others. On success loads the
/// program and sets the start state, as `assemble_input`, and keeps it for `make_tape_input`.
/// Writes JSON to the output buffer and returns its length: `{"ok":false,"errors":[[line,
/// "message","file"|null],...]}` or `{"ok":true,"start","size","segments":[[start,len],...],
/// "lines":[[line,addr,len,"file"|null],...],"symbols":[["name",value],...]}`. A null file is
/// the main one.
#[unsafe(no_mangle)]
pub extern "C" fn assemble_project_input() -> u32 {
    let raw = INPUT.with(|i| String::from_utf8_lossy(&i.borrow()).into_owned());
    let mut parts = raw.split('\0');
    let mut files: Vec<(String, String)> = Vec::new();
    while let (Some(path), Some(text)) = (parts.next(), parts.next()) {
        files.push((path.to_string(), text.to_string()));
    }
    let Some((_, main)) = files.first().cloned() else {
        return out("{\"ok\":false,\"errors\":[[1,\"no files\",null]]}".into());
    };
    let resolve = |name: &str| {
        let name = name.trim_start_matches("./");
        files.iter().find(|(p, _)| p == name).map(|(_, t)| t.clone())
    };
    let file = |f: &Option<String>| f.as_deref().map(json_str).unwrap_or_else(|| "null".into());
    match unc_asm::assemble_with(&main, resolve) {
        Err(errs) => {
            LAST.with(|l| *l.borrow_mut() = None);
            let list: Vec<String> = errs.iter().map(|e| format!("[{},{},{}]", e.line, json_str(&e.message), file(&e.file))).collect();
            out(format!("{{\"ok\":false,\"errors\":[{}]}}", list.join(",")))
        }
        Ok(a) => {
            let start = m(|m| unc_asm::load(m, &a));
            let lines: Vec<String> = a.lines.iter().map(|l| format!("[{},{},{},{}]", l.line, l.addr, l.len, file(&l.file))).collect();
            let syms: Vec<String> = a.symbols.iter().map(|(n, v)| format!("[{},{v}]", json_str(n))).collect();
            let segs: Vec<String> = a.segments.iter().filter(|s| !s.bytes.is_empty()).map(|s| format!("[{},{}]", s.start, s.bytes.len())).collect();
            let size: usize = a.segments.iter().map(|s| s.bytes.len()).sum();
            LAST.with(|l| *l.borrow_mut() = Some(a));
            out(format!(
                "{{\"ok\":true,\"start\":{start},\"size\":{size},\"segments\":[{}],\"lines\":[{}],\"symbols\":[{}]}}",
                segs.join(","),
                lines.join(","),
                syms.join(",")
            ))
        }
    }
}

/// Puts the last program `assemble_project_input` assembled on a tape (manual chapter 14,
/// `unc_asm::tape::make`), named by the text in the input buffer. The tape's bytes go to the
/// output buffer; returns their length (0 if nothing has assembled).
#[unsafe(no_mangle)]
pub extern "C" fn make_tape_input() -> u32 {
    let name = INPUT.with(|i| String::from_utf8_lossy(&i.borrow()).into_owned());
    let tape = LAST.with(|l| l.borrow().as_ref().map(|a| unc_asm::tape::make(a, &name)));
    let Some(tape) = tape else { return 0 };
    let n = tape.len() as u32;
    OUTPUT.with(|o| *o.borrow_mut() = tape);
    n
}

/// Disassembles `count` instructions from `addr` as lines of `addr\tbytes\ttext`.
#[unsafe(no_mangle)]
pub extern "C" fn disassemble(addr: u32, count: u32) -> u32 {
    m(|m| {
        let read = |a: u16| m.peek(a);
        let mut a = addr as u16;
        let mut s = String::new();
        for _ in 0..count {
            let (text, len) = disasm(&read, a);
            let bytes: Vec<String> = (0..len).map(|i| format!("{:02X}", read(a.wrapping_add(i)))).collect();
            s.push_str(&format!("{a}\t{}\t{text}\n", bytes.join(" ")));
            a = a.wrapping_add(len);
        }
        out(s)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn step() -> u32 {
    m(|m| m.step() as u32)
}

#[unsafe(no_mangle)]
pub extern "C" fn step_over() -> u32 {
    m(|m| stop_code(m.step_over()))
}

#[unsafe(no_mangle)]
pub extern "C" fn run_frame() -> u32 {
    m(|m| stop_code(m.run_frame()))
}

/// 0 A, 1 X, 2 Y, 3 SP, 4 P, 5 PC.
#[unsafe(no_mangle)]
pub extern "C" fn reg(which: u32) -> u32 {
    m(|m| {
        let c = &m.cpu;
        match which {
            0 => c.a as u32,
            1 => c.x as u32,
            2 => c.y as u32,
            3 => c.sp as u32,
            4 => c.p as u32,
            5 => c.pc as u32,
            _ => 0,
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn set_pc(pc: u32) {
    m(|m| {
        m.cpu.pc = pc as u16;
        m.cpu.state = CpuState::Running;
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn cycles() -> f64 {
    m(|m| m.cpu.cycles as f64)
}

/// 0 running, 1 halted, 2 jammed.
#[unsafe(no_mangle)]
pub extern "C" fn cpu_state() -> u32 {
    m(|m| match m.cpu.state {
        CpuState::Running => 0,
        CpuState::Halted => 1,
        CpuState::Jammed(_) => 2,
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn jam_opcode() -> u32 {
    m(|m| match m.cpu.state {
        CpuState::Jammed(o) => o as u32,
        _ => 0,
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn peek(addr: u32) -> u32 {
    m(|m| m.peek(addr as u16) as u32)
}

#[unsafe(no_mangle)]
pub extern "C" fn poke(addr: u32, value: u32) {
    m(|m| m.poke(addr as u16, value as u8));
}

/// A key goes down: KBD gets it, and HELD reads it until `key_up`.
#[unsafe(no_mangle)]
pub extern "C" fn key(ascii: u32) {
    m(|m| m.key(ascii as u8));
}

/// A key comes up: HELD forgets it.
#[unsafe(no_mangle)]
pub extern "C" fn key_up(ascii: u32) {
    m(|m| m.key_up(ascii as u8));
}

#[unsafe(no_mangle)]
pub extern "C" fn set_breakpoint(addr: u32, on: u32) {
    m(|m| m.set_breakpoint(addr as u16, on != 0));
}

#[unsafe(no_mangle)]
pub extern "C" fn breakpoint(addr: u32) -> u32 {
    m(|m| m.breakpoint(addr as u16) as u32)
}

#[unsafe(no_mangle)]
pub extern "C" fn clear_breakpoints() {
    m(|m| m.clear_breakpoints());
}

/// The 1000 bytes of screen memory at $0400.
#[unsafe(no_mangle)]
pub extern "C" fn screen_ptr() -> *const u8 {
    m(|m| m.mem.ram[0x0400..].as_ptr())
}

/// The picture as the beam drew it (320 x 200 bytes, manual chapter 9), into the pixel
/// buffer, which it returns. Without the video board 0 is paper and 1 ink; with it, each
/// byte is a colour number 0-3. `line_colours_ptr` says what each shows on each line
/// (`colour_of`, what they show now). `blink_on` is the cursor's blink for the lines the
/// beam draws next.
#[unsafe(no_mangle)]
pub extern "C" fn render(blink_on: u32) -> *const u8 {
    PIXELS.with(|p| {
        let mut p = p.borrow_mut();
        m(|m| {
            m.blink_on = blink_on != 0;
            p[..m.frame().len()].copy_from_slice(m.frame());
        });
        p.as_ptr()
    })
}

/// 800 bytes: for each of the picture's 200 lines, the colours (0-15) of values 0-3 as the
/// beam drew it: BACKGROUND, then white ink without the video board, COLOUR1-3 with it.
#[unsafe(no_mangle)]
pub extern "C" fn line_colours_ptr() -> *const u8 {
    m(|m| m.line_colours_all().as_ptr() as *const u8)
}

/// The colour (0-15, manual Appendix A) of a value in `render`'s picture: 0 is BACKGROUND;
/// without the video board anything else is white ink (1), and with it 1-3 are COLOUR1-3.
#[unsafe(no_mangle)]
pub extern "C" fn colour_of(n: u32) -> u32 {
    m(|m| m.colour_of(n as u8) as u32)
}

/// 312 bytes: the border colour on each raster line of the last frame.
#[unsafe(no_mangle)]
pub extern "C" fn border_lines_ptr() -> *const u8 {
    m(|m| m.border_lines.as_ptr())
}

/// Renders the speaker over the last frame as `count` mono f32 samples at `sample_rate`
/// (`Machine::audio_frame`) and returns them. Call once after each `run_frame`; about
/// `sample_rate * 0.019968` samples keep real time.
#[unsafe(no_mangle)]
pub extern "C" fn audio_frame(sample_rate: u32, count: u32) -> *const f32 {
    AUDIO.with(|a| {
        let mut a = a.borrow_mut();
        a.resize(count as usize, 0.0);
        m(|m| m.audio_frame(sample_rate, &mut a));
        a.as_ptr()
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn raster_line() -> u32 {
    m(|m| m.mem.raster_line())
}

#[unsafe(no_mangle)]
pub extern "C" fn ram_kb() -> u32 {
    m(|m| if m.mem.ram_size == RamSize::K64 { 64 } else { 16 })
}

/// Puts the bytes in the input buffer in the tape deck as a tape (manual chapter 14),
/// rewound, PLAY up. LOAD typed at the machine then waits for `press_play`.
#[unsafe(no_mangle)]
pub extern "C" fn insert_tape_input() {
    let tape = INPUT.with(|i| i.borrow().clone());
    m(|m| m.insert_tape(tape));
}

/// The deck's PLAY key. Returns 1 if it went down (it needs a tape in).
#[unsafe(no_mangle)]
pub extern "C" fn press_play() -> u32 {
    m(|m| m.press_play() as u32)
}

/// The deck's STOP key: PLAY and RECORD come up.
#[unsafe(no_mangle)]
pub extern "C" fn press_stop() {
    m(|m| m.press_stop());
}

/// Winds the tape back to its start; PLAY comes up.
#[unsafe(no_mangle)]
pub extern "C" fn rewind_tape() {
    m(|m| m.rewind_tape());
}

/// Fast load (`Machine::set_tape_turbo`): the next byte arrives as soon as the ROM has taken
/// the last. LOAD runs exactly as usual, only faster. A host setting, not the machine's.
#[unsafe(no_mangle)]
pub extern "C" fn set_tape_turbo(on: u32) {
    m(|m| m.set_tape_turbo(on != 0));
}

/// The deck's counter: bytes past the head since the tape was rewound.
#[unsafe(no_mangle)]
pub extern "C" fn tape_counter() -> u32 {
    m(|m| m.mem.deck.position as u32)
}

/// The tape's length in bytes (0 with no tape in).
#[unsafe(no_mangle)]
pub extern "C" fn tape_length() -> u32 {
    m(|m| m.mem.deck.tape.as_ref().map_or(0, |t| t.len() as u32))
}

/// What TAPE ($C040) reads: bit 0 PLAY down, 1 tape in, 2 motor on (chapter 14).
#[unsafe(no_mangle)]
pub extern "C" fn tape_status() -> u32 {
    m(|m| m.mem.deck.status() as u32)
}

/// Puts a blank tape in the deck: nothing on it, for SAVE to record on. Any tape in comes out.
#[unsafe(no_mangle)]
pub extern "C" fn insert_blank_tape() {
    m(|m| m.insert_blank_tape());
}

/// The deck's RECORD key, which takes PLAY down with it. Returns 1 if they went down (they
/// need a tape in). SAVE then records while the machine runs the motor.
#[unsafe(no_mangle)]
pub extern "C" fn press_record() -> u32 {
    m(|m| m.press_record() as u32)
}

/// 1 while the RECORD key is down (TAPE bit 3 says the same).
#[unsafe(no_mangle)]
pub extern "C" fn tape_record() -> u32 {
    m(|m| m.mem.deck.record as u32)
}

/// The tape's bytes into the output buffer, as a `.tap` image, with whatever was recorded on
/// it; the tape stays in. Returns the length, or -1 with no tape in (a blank tape is 0).
#[unsafe(no_mangle)]
pub extern "C" fn tape_bytes() -> i32 {
    let tape = m(|m| m.mem.deck.tape.clone());
    tape_out(tape)
}

/// STOP/EJECT: PLAY and RECORD up, and the tape out, its bytes into the output buffer as for
/// `tape_bytes`. Returns the length, or -1 if there was no tape.
#[unsafe(no_mangle)]
pub extern "C" fn eject_tape() -> i32 {
    let tape = m(|m| m.eject_tape());
    tape_out(tape)
}

fn tape_out(tape: Option<Vec<u8>>) -> i32 {
    match tape {
        Some(t) => {
            let n = t.len() as i32;
            OUTPUT.with(|o| *o.borrow_mut() = t);
            n
        }
        None => -1,
    }
}

/// A tape holding the machine's BASIC program as a type-2 program (manual chapter 14),
/// named by the input buffer (up to 16 characters, upper case): what SAVE would record,
/// without the deck. The bytes go to the output buffer; returns their length.
#[unsafe(no_mangle)]
pub extern "C" fn basic_tape() -> u32 {
    let name = INPUT.with(|i| String::from_utf8_lossy(&i.borrow()).into_owned());
    let tape = m(|m| unc_asm::tape::basic(&name, &unc_asm::tape::resident_basic(m)).encode());
    let n = tape.len() as u32;
    OUTPUT.with(|o| *o.borrow_mut() = tape);
    n
}

// Editor tools: the assembly language as data, tokens and analysis for the Machine Room's and
// the showcase's editors. All from unc_asm::lang and unc_asm::analysis (editors/README.md has
// the JSON layouts). Tools outside the machine: none of this touches the emulation.

/// Writes the UNC assembly language as JSON to the output buffer (instructions, directives,
/// functions, I/O registers, ROM routines, ...) and returns its length.
#[unsafe(no_mangle)]
pub extern "C" fn language_json() -> u32 {
    out(unc_asm::lang::json_compact())
}

/// Tokens for highlighting each line of the source in the input buffer, as JSON:
/// `[[[start, end, "kind"], ...], ...]`, one array per line, columns in characters.
#[unsafe(no_mangle)]
pub extern "C" fn tokens_input() -> u32 {
    let src = INPUT.with(|i| String::from_utf8_lossy(&i.borrow()).into_owned());
    out(unc_asm::lang::source_tokens_json(&src))
}

/// Analyses the source in the input buffer without loading it: each line's address, bytes and
/// cycles, the .rept blocks' totals, the symbols, and the errors and warnings with columns in
/// characters. Writes JSON to the output buffer and returns its length.
#[unsafe(no_mangle)]
pub extern "C" fn analyze_input() -> u32 {
    let src = INPUT.with(|i| String::from_utf8_lossy(&i.borrow()).into_owned());
    out(unc_asm::analysis_json(&src))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(bytes: &[u8]) {
        let p = input_buffer(bytes.len() as u32);
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len()) };
    }
    fn output(n: u32) -> Vec<u8> {
        OUTPUT.with(|o| o.borrow()[..n as usize].to_vec())
    }

    #[test]
    fn a_project_assembles_across_its_files_and_goes_on_tape() {
        power_on(16);
        put(b"main.s\0        .org $0800\nstart:  LDA #VALUE\n        .include \"defs.s\"\n        HLT\n\0defs.s\0VALUE = 42\n        NOP\n");
        let json = String::from_utf8(output(assemble_project_input())).unwrap();
        assert!(json.starts_with("{\"ok\":true,\"start\":2048"), "{json}");
        assert!(json.contains("\"defs.s\""), "lines name the included file: {json}");
        assert!(json.contains("\"segments\":[[2048,4]]"), "{json}");
        put(b"Two files");
        let tape = output(make_tape_input());
        let progs = unc_asm::tape::parse(&tape).unwrap();
        assert_eq!(progs[0].name(), "TWO FILES");
        assert_eq!((progs[0].load, progs[0].entry, progs[0].data.len(), progs[0].data[1]), (0x0800, 0x0800, 4, 42));
    }

    #[test]
    fn errors_name_their_file() {
        put(b"main.s\0        .include \"bad.s\"\n\0bad.s\0        LDQ #1\n");
        let json = String::from_utf8(output(assemble_project_input())).unwrap();
        assert!(json.contains("\"ok\":false") && json.contains("\"bad.s\""), "{json}");
        put(b"X");
        assert_eq!(make_tape_input(), 0, "nothing assembled, no tape");
    }

    #[test]
    fn the_deck_counts_and_loads_fast() {
        set_video_board(0);
        power_on(16);
        put(b"main.s\0        .org $3000\nstart:  HLT\n");
        assemble_project_input();
        put(b"T");
        let tape = output(make_tape_input());
        power_on(16);
        put(&tape);
        insert_tape_input();
        assert_eq!(tape_length(), tape.len() as u32);
        assert_eq!(tape_status() & 2, 2, "a tape is in");
        set_tape_turbo(1);
        for k in b"LOAD\r" {
            key(*k as u32);
            run_frame();
            key_up(*k as u32);
            run_frame();
        }
        press_play();
        for _ in 0..20 {
            run_frame();
        }
        assert_eq!(tape_counter(), tape.len() as u32, "fast load reads it all within 20 frames");
        assert_eq!(cpu_state(), 1, "and the program ran: HLT");
        eject_tape();
        assert_eq!(tape_length(), 0);
    }
}
