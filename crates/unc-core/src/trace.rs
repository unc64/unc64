//! The memory view of the game's HOLD box: what the host watches the
//! processor do to memory, byte by byte, and the questions the view asks of the machine.
//! Host-only, like `Observation`: nothing a program can read, and nothing the machine does
//! depends on it, so determinism is untouched. Off until the host turns it on (`watch`).

use std::sync::OnceLock;

use crate::cpu::CpuState;
use crate::isa::OPCODES;
use crate::machine::{
    io, Machine, BITMAP_STRIDE, PIXELS_H, PIXELS_W, ROM_START, SCREEN, SCREEN_COLS, SYSTEM_ROM,
};

/// Where each byte was last touched: stamped in frames, counted from 1 when the watch began.
#[derive(Clone)]
pub struct Trace {
    /// The frame running now.
    pub frame: u32,
    /// The frame each byte was last written in by the processor (0: not since the watch began).
    pub written: Box<[u32]>,
    /// The frame the processor last ran an instruction from each byte in (its opcode and
    /// its operand bytes).
    pub ran: Box<[u32]>,
    /// Where the instruction that last wrote each byte starts.
    pub writer: Box<[u16]>,
    /// The BASIC line that was running when each byte was last written (`NO_LINE`: none).
    pub line: Box<[u16]>,
    /// The instruction running now: what a write is stamped with.
    pub(crate) pc: u16,
}

impl Trace {
    fn new() -> Self {
        Trace {
            frame: 1,
            written: vec![0; 0x10000].into_boxed_slice(),
            ran: vec![0; 0x10000].into_boxed_slice(),
            writer: vec![0; 0x10000].into_boxed_slice(),
            line: vec![NO_LINE; 0x10000].into_boxed_slice(),
            pc: 0,
        }
    }

    /// How many frames ago a stamp was: 0 this frame, 255 for 255 or more, or never.
    pub fn age(&self, stamp: u32) -> u8 {
        if stamp == 0 { 255 } else { self.frame.wrapping_sub(stamp).min(255) as u8 }
    }
}

/// `Trace::line` for a write made with no BASIC program running.
pub const NO_LINE: u16 = 0xFFFF;

/// What a step to the next BASIC line did (`Machine::step_line`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineStep {
    /// The line it was on.
    pub from: u16,
    /// The line it stopped at the start of; None if BASIC stopped running (the program
    /// ended, an error, STOP) or the budget ran out first.
    pub to: Option<u16>,
    /// The times `from` started again on the way: a line going round its own loop
    /// (`FOR J=1 TO 150:NEXT`) counts once.
    pub again: u32,
    /// The processor's cycles it took.
    pub cycles: u64,
}

/// A line of the BASIC program in memory, as LIST shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BasicLine {
    pub number: u16,
    /// Where the line starts: its length byte.
    pub addr: u16,
    /// Its bytes, from the length byte to the 0 that ends it.
    pub len: u16,
    /// The text after the number, keywords spelled out.
    pub text: String,
}

/// The ROM's keywords, in token order ($80 first): read out of the ROM image's KWTAB, the
/// table LIST spells them from (each word's last letter has bit 7 set; a 0 ends it).
pub fn keywords() -> &'static [String] {
    static WORDS: OnceLock<Vec<String>> = OnceLock::new();
    WORDS.get_or_init(|| {
        let rom: &[u8] = &SYSTEM_ROM[..];
        let Some(at) = rom.windows(5).position(|w| w == b"PRIN\xD4") else { return Vec::new() };
        let mut out = Vec::new();
        let mut word = String::new();
        for &b in &rom[at..] {
            if b == 0 {
                break;
            }
            word.push((b & 0x7F) as char);
            if b & 0x80 != 0 {
                out.push(std::mem::take(&mut word));
            }
        }
        out
    })
}

impl Machine {
    /// Turns the memory view's watch on (every stamp starts afresh) or off.
    pub fn watch(&mut self, on: bool) {
        self.mem.trace = on.then(|| Box::new(Trace::new()));
    }

    /// The watch, if it's on.
    pub fn trace(&self) -> Option<&Trace> {
        self.mem.trace.as_deref()
    }

    /// The memory view's two ages for `len` bytes from `from`: frames since each was written
    /// and since it was run (0 this frame, 255 for 255 or more, or never, or the watch off).
    pub fn ages(&self, from: u16, len: usize) -> (Vec<u8>, Vec<u8>) {
        let Some(t) = self.trace() else { return (vec![255; len], vec![255; len]) };
        let at = |i: usize| (from as usize + i) & 0xFFFF;
        ((0..len).map(|i| t.age(t.written[at(i)])).collect(), (0..len).map(|i| t.age(t.ran[at(i)])).collect())
    }

    /// Where the instruction that last wrote `addr` starts, if the processor has written it
    /// since the watch began.
    pub fn writer_of(&self, addr: u16) -> Option<u16> {
        let t = self.trace()?;
        (t.written[addr as usize] != 0).then(|| t.writer[addr as usize])
    }

    /// The BASIC line that was running when the processor last wrote `addr`, if one was.
    pub fn writer_line(&self, addr: u16) -> Option<u16> {
        let t = self.trace()?;
        let l = t.line[addr as usize];
        (t.written[addr as usize] != 0 && l != NO_LINE).then_some(l)
    }

    /// Before each instruction: the bytes it's run from, and what its writes are stamped
    /// with. The interrupt's way in is stamped as the instruction it comes before.
    pub(crate) fn trace_step(&mut self) {
        if self.mem.trace.is_none() {
            return;
        }
        let pc = self.cpu.pc;
        let len = OPCODES[self.mem.peek(pc) as usize].map_or(1, |i| i.mode.len());
        let t = self.mem.trace.as_deref_mut().unwrap();
        t.pc = pc;
        for i in 0..len as u16 {
            t.ran[pc.wrapping_add(i) as usize] = t.frame;
        }
    }

    /// The byte the picture's pixel (x, y), 0-319 and 0-199, is drawn from, and which of its
    /// bits (7 the leftmost): the screen's character code in text, the bitmap's byte where
    /// the bitmap shows. The NIECE's page isn't asked (it's the picture's own byte in front
    /// of it). None off the picture.
    pub fn addr_at(&self, x: usize, y: usize) -> Option<(u16, u8)> {
        if x >= PIXELS_W || y >= PIXELS_H {
            return None;
        }
        let vmode = self.mem.io.vmode;
        let split = self.mem.bitmap_lines();
        let bit = 7 - (x & 7) as u8;
        if vmode & io::VMODE_BITMAP != 0 && y < split {
            let base = (self.mem.io.bitmap as u16) << 8;
            return Some((base.wrapping_add((y * BITMAP_STRIDE + x / 8) as u16), bit));
        }
        Some((SCREEN + ((y / 8) * SCREEN_COLS + x / 8) as u16, bit))
    }

    /// STEP in BASIC: runs the program on to the start of the next line with a different
    /// number. BASIC writes the line's number (CURLIN, `$0205-$0206`) as each line starts,
    /// and again each time a line goes round its own loop (`FOR J=1 TO 150:NEXT` writes it
    /// 150 times): those count in `again`, and it runs on. Breakpoints aren't stopped at.
    /// None if no program is running. Stops early, `to` None, if BASIC stops running, the
    /// processor stops, or `budget` cycles run out.
    pub fn step_line(&mut self, budget: u64) -> Option<LineStep> {
        let from = self.basic_line()?;
        let start = self.cpu.cycles;
        let mut again = 0;
        loop {
            if !matches!(self.cpu.state, CpuState::Running) {
                return Some(LineStep { from, to: None, again, cycles: self.cpu.cycles - start });
            }
            let lw = self.mem.obs.line_writes;
            self.step();
            if self.mem.ram[0x0204] == 0 {
                return Some(LineStep { from, to: None, again, cycles: self.cpu.cycles - start });
            }
            if self.mem.obs.line_writes != lw {
                let line = u16::from_le_bytes([self.mem.ram[0x0205], self.mem.ram[0x0206]]);
                if line != from {
                    return Some(LineStep { from, to: Some(line), again, cycles: self.cpu.cycles - start });
                }
                again += 1;
            }
            if self.cpu.cycles - start >= budget {
                return Some(LineStep { from, to: None, again, cycles: self.cpu.cycles - start });
            }
        }
    }

    /// The BASIC program in memory from `$0800`, a line at a time, as LIST would show it:
    /// each line's length byte, number, then its bytes, a token spelled as its keyword.
    /// Stops at the end marker, at a line that can't be one, or at the end of RAM.
    pub fn basic_lines(&self) -> Vec<BasicLine> {
        let words = keywords();
        let end = (self.mem.ram_size.end() as usize).min(ROM_START as usize);
        let ram = &self.mem.ram;
        let mut out = Vec::new();
        let mut a = 0x0800usize;
        while a + 3 < end && out.len() < 4000 {
            let len = ram[a] as usize;
            if len < 4 || a + len > end {
                break;
            }
            let number = u16::from_le_bytes([ram[a + 1], ram[a + 2]]);
            let mut text = String::new();
            for &b in &ram[a + 3..a + len] {
                if b == 0 {
                    break;
                }
                if b >= 0x80 {
                    text.push_str(words.get((b - 0x80) as usize).map_or("?", |w| w.as_str()));
                } else {
                    text.push(b as char);
                }
            }
            out.push(BasicLine { number, addr: a as u16, len: len as u16, text });
            a += len;
        }
        out
    }
}

/// The last frames, the whole machine at the start of each (`save_state`): the HOLD box's
/// 10 seconds. `record` before each frame runs. "Back" counts moments: 0 is now (the machine
/// itself), 1 the moment the frame before the last one ended, and so on to `len()`.
///
/// A whole state is kept once every `KEY_EVERY` frames (and whenever its size changes); the
/// frames between keep only the bytes that changed from the one before (`Moment::runs`), so
/// 10 seconds of a UNC 64 with the NIECE are about 1.5 MB, not 58. A moment is put back
/// together from the key before it; the newest is kept whole (`last`).
pub struct History {
    moments: std::collections::VecDeque<Moment>,
    /// For each moment, the bytes of RAM different from the moment before it: what the frame
    /// between them changed (0 for the first).
    changed: std::collections::VecDeque<u32>,
    /// The newest moment, whole: what the next one is compared with.
    last: Vec<u8>,
    /// Moments since the newest key.
    since_key: usize,
    cap: usize,
}

/// One moment: a whole save state (a key), or what changed since the moment before it.
struct Moment {
    key: Option<Vec<u8>>,
    /// (offset, the bytes there now), in order.
    runs: Vec<(u32, Vec<u8>)>,
}

/// A whole state every second.
const KEY_EVERY: usize = 50;
/// Two changed bytes closer than this go in one run.
const RUN_GAP: usize = 8;

impl History {
    /// Keeps the last `cap` frames: 500 is 10 seconds.
    pub fn new(cap: usize) -> Self {
        History {
            moments: std::collections::VecDeque::with_capacity(cap + 1),
            changed: std::collections::VecDeque::with_capacity(cap + 1),
            last: Vec::new(),
            since_key: 0,
            cap: cap.max(1),
        }
    }

    /// The machine as the next frame starts: keep it, and forget the oldest past `cap`.
    pub fn record(&mut self, m: &Machine) {
        let state = m.save_state();
        let fresh = self.moments.is_empty();
        let n = if fresh { 0 } else { differ(state_ram(&self.last), state_ram(&state)) };
        let moment = if fresh || state.len() != self.last.len() || self.since_key + 1 >= KEY_EVERY {
            self.since_key = 0;
            Moment { key: Some(state.clone()), runs: Vec::new() }
        } else {
            self.since_key += 1;
            Moment { key: None, runs: runs(&self.last, &state) }
        };
        self.moments.push_back(moment);
        self.changed.push_back(n);
        self.last = state;
        if self.moments.len() > self.cap {
            // the oldest goes; the one after it becomes a key, if it isn't one
            let mut whole = self.moments.pop_front().and_then(|m| m.key).unwrap_or_default();
            self.changed.pop_front();
            if let Some(next) = self.moments.front_mut()
                && next.key.is_none()
            {
                apply(&mut whole, &next.runs);
                next.key = Some(whole);
                next.runs = Vec::new();
            }
        }
    }

    /// How much each frame changed, oldest first: for each moment `len()` back to 1, the
    /// bytes of RAM the frame that ended then left different.
    pub fn activity(&self) -> Vec<u32> {
        self.changed.iter().copied().collect()
    }

    /// One byte through the 10 seconds, oldest first: its value at each moment `len()` back
    /// to 1 (0 where the RAM then didn't reach it). Read along the changes, not rebuilt.
    pub fn values(&self, addr: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.moments.len());
        let mut at: Option<usize> = None; // the byte's offset in the states since the last key
        let mut value = 0u8;
        for m in &self.moments {
            if let Some(key) = &m.key {
                let ram = state_ram(key);
                at = (addr < ram.len()).then(|| key.len() - ram.len() + addr);
                value = at.map_or(0, |o| key[o]);
            } else if let Some(o) = at {
                for (start, bytes) in &m.runs {
                    let start = *start as usize;
                    if (start..start + bytes.len()).contains(&o) {
                        value = bytes[o - start];
                    }
                }
            }
            out.push(value);
        }
        out
    }

    /// How far back it goes, in frames.
    pub fn len(&self) -> usize {
        self.moments.len()
    }

    /// About how many bytes it keeps: the keys, the changes, and the newest moment whole.
    pub fn kept_bytes(&self) -> usize {
        let moments: usize = self.moments.iter().map(|m| m.key.as_ref().map_or(0, Vec::len) + m.runs.iter().map(|r| r.1.len() + 32).sum::<usize>()).sum();
        moments + self.last.len()
    }

    pub fn is_empty(&self) -> bool {
        self.moments.is_empty()
    }

    pub fn clear(&mut self) {
        self.moments.clear();
        self.changed.clear();
        self.last.clear();
        self.since_key = 0;
    }

    /// The whole machine `back` frames ago (1 to `len()`), as `save_state` gave it.
    pub fn moment(&self, back: usize) -> Option<Vec<u8>> {
        if !(1..=self.len()).contains(&back) {
            return None;
        }
        if back == 1 {
            return Some(self.last.clone());
        }
        Some(self.whole(self.len() - back))
    }

    /// The RAM `back` frames ago, from `$0000` to the end of the RAM fitted then.
    pub fn ram(&self, back: usize) -> Option<Vec<u8>> {
        self.moment(back).map(|s| state_ram(&s).to_vec())
    }

    /// Moment `i` (0 the oldest), put back together from the key at or before it.
    fn whole(&self, i: usize) -> Vec<u8> {
        let k = (0..=i).rev().find(|&j| self.moments[j].key.is_some()).unwrap_or(0);
        let mut s = self.moments[k].key.clone().unwrap_or_default();
        for j in k + 1..=i {
            apply(&mut s, &self.moments[j].runs);
        }
        s
    }

    /// The frame that ended `back` frames ago, run again on `viewer` from the state it started
    /// in: its picture (`frame`) and, with `viewer.watch(true)` first, what it wrote and ran.
    /// The tape deck isn't in a save state, so a frame that was reading a tape replays
    /// without it. False if it's further back than the history goes.
    pub fn replay(&self, back: usize, viewer: &mut Machine) -> bool {
        let Some(state) = self.moment(back + 1) else { return false };
        if viewer.load_state(&state).is_err() {
            return false;
        }
        if let Some(t) = viewer.mem.trace.as_deref_mut() {
            t.frame = t.frame.wrapping_add(300); // everything from before is old news
        }
        viewer.run_frame();
        true
    }

    /// ▶ FROM HERE: `m` goes back to the moment `back` frames ago, and what came after it is
    /// forgotten. False (and nothing changed) if it's further back than the history goes.
    pub fn rewind(&mut self, back: usize, m: &mut Machine) -> bool {
        let Some(state) = self.moment(back) else { return false };
        if m.load_state(&state).is_err() {
            return false;
        }
        let keep = self.len() - back;
        self.moments.truncate(keep);
        self.changed.truncate(keep);
        if keep == 0 {
            self.clear();
        } else {
            self.last = self.whole(keep - 1);
            self.since_key = self.moments.iter().rev().take_while(|m| m.key.is_none()).count();
        }
        if let Some(t) = m.mem.trace.as_deref_mut() {
            t.written.fill(0);
            t.ran.fill(0);
        }
        true
    }
}

/// What changed from `a` to `b` (the same length): runs of bytes, close changes together.
fn runs(a: &[u8], b: &[u8]) -> Vec<(u32, Vec<u8>)> {
    let mut out = Vec::new();
    let mut i = 0;
    let n = a.len().min(b.len());
    while i < n {
        if i + 64 <= n && a[i..i + 64] == b[i..i + 64] {
            i += 64; // most of a state doesn't change: skip it a block at a time
            continue;
        }
        if a[i] == b[i] {
            i += 1;
            continue;
        }
        let start = i;
        let mut end = i + 1; // past the last changed byte
        let mut j = i + 1;
        while j < n && j < end + RUN_GAP {
            if a[j] != b[j] {
                end = j + 1;
            }
            j += 1;
        }
        out.push((start as u32, b[start..end].to_vec()));
        i = end;
    }
    out
}

/// The runs written over `s`.
fn apply(s: &mut [u8], runs: &[(u32, Vec<u8>)]) {
    for (start, bytes) in runs {
        let start = *start as usize;
        s[start..start + bytes.len()].copy_from_slice(bytes);
    }
}

/// The bytes that differ between two equal-length runs of RAM (the shorter's length counts).
pub fn differ(a: &[u8], b: &[u8]) -> u32 {
    let count = |x: &[u8], y: &[u8]| x.iter().zip(y).filter(|(p, q)| p != q).count() as u32;
    a.chunks(64).zip(b.chunks(64)).filter(|(x, y)| x != y).map(|(x, y)| count(x, y)).sum()
}

/// A save state's RAM: its tail, from `$0000` to the end of the RAM it was taken with.
pub fn state_ram(state: &[u8]) -> &[u8] {
    let end = if state.get(5) == Some(&64) { 0xC000 } else { 0x4000 };
    &state[state.len().saturating_sub(end)..]
}

#[cfg(test)]
mod history_tests {
    use super::*;
    use crate::RamSize;

    /// Every moment comes back exactly as it was: across keys, as the oldest go, after a
    /// rewind, and through a change of size (the 64K going in). And it's small.
    #[test]
    fn every_moment_comes_back_whole() {
        let mut m = Machine::new(RamSize::K16);
        let mut h = History::new(120);
        let mut states: Vec<Vec<u8>> = Vec::new();
        let mut seed = 12345u32;
        for f in 0..400 {
            if f == 200 {
                m = Machine::new(RamSize::K64); // a different size: a key of its own
                m.set_niece(true);
            }
            for _ in 0..(f % 7) * 9 {
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                let top = if f >= 200 { 0xC000 } else { 0x4000 };
                m.poke(((seed >> 8) % top) as u16, (seed >> 24) as u8);
            }
            h.record(&m);
            states.push(m.save_state());
            m.run_frame();
            if f == 300 {
                let then = states[states.len() - 30].clone();
                assert!(h.rewind(30, &mut m));
                states.truncate(states.len() - 30); // that moment is now, and after it is forgotten
                assert_eq!(state_ram(&m.save_state()), state_ram(&then), "rewound to the moment");
            }
        }
        assert_eq!(h.len(), 120);
        for back in 1..=h.len() {
            assert_eq!(h.moment(back).unwrap(), states[states.len() - back], "{back} frames back");
        }
        let addr = 0x1234;
        let vals = h.values(addr);
        for (i, v) in vals.iter().enumerate() {
            let s = &states[states.len() - h.len() + i];
            assert_eq!(*v, state_ram(s)[addr], "the byte at moment {i}");
        }
        let kept = h.kept_bytes();
        let whole: usize = states[states.len() - h.len()..].iter().map(|s| s.len()).sum();
        assert!(kept * 10 < whole, "kept {kept} bytes for {whole} of states");
    }
}
