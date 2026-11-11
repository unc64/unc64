//! Save states: the whole computer as bytes, and back. Rev A. Host-only: the
//! machine has no instruction that saves or loads one. The note for hosts is
//! docs/hosts.md.
//!
//! The format, all numbers little-endian:
//!
//! | Bytes | Holds |
//! |---|---|
//! | 4 | `UNCS` |
//! | 1 | Format version: `VERSION` |
//! | 1 | RAM: 16 or 64 (K) |
//! | 18 | The processor: A, X, Y, SP, PC, P, the cycle count, its state and the IRQ line |
//! | 66 + keys held | The I/O: video, raster, interrupts, frame, keyboard (KBD, the held keys, the matrix), colours, character set and bitmap pages, TEXTROWS with CASE in its bit 7 (version 5 on), the timer and speaker, the shifter |
//! | 1, or 16 with the board | The video board (version 2 on): 0 if it isn't in; 1 and its 15 bytes of registers (`VideoBoard::save`) if it is |
//! | 122 | The sound chip (version 4 on): 1 and its 121 bytes (`Chip::save`); or one byte, 0, if a host took it out |
//! | 4 | The computer's side of the tape interface: motor, data latch, byte ready, overrun |
//! | 4 | Cycles into the current frame |
//! | 312 | The border on each raster line of the last frame (for the picture) |
//! | 16K or 48K | RAM, from `$0000` to the top of memory |
//!
//! So a UNC 16 with no key down saves to 16,917 bytes, and one with the 64K expansion to
//! 49,685; the video board adds 15 more. (The sprite table and images are in RAM.)
//!
//! **Version 4** is the same without TEXTROWS: it loads with four text rows under the
//! bitmap, as at reset.
//! **Version 3** is that without the sound chip's bytes: it loads with the chip as it
//! is at switch-on, silent.
//! **Version 2** had the first Rev A board: 37 bytes of registers, eight sprites in them.
//! That board is gone, so a version 2 state loads with the board in, its bank, scroll and
//! layer kept and its sprites dropped (the table and the colours as at reset).
//! **Version 1** is the same layout without the video board's byte or registers: it loads
//! as a machine without the board.
//!
//! **Not saved:** the tape deck's own state (the tape in it, the counter, the PLAY and RECORD
//! keys), the
//! breakpoints, the host's `observe` counts, the sound already rendered and what the reserved
//! range answers (`Machine::set_reserved`: story state the host applies). The deck isn't
//! part of the computer, just as a reset leaves it alone: `load_state` keeps whatever tape
//! the host has in the deck now, where it is, and takes only the computer's side of the
//! interface (the motor and the data latch) from the save. A host that wants a tape back
//! re-inserts it. That's safe at any moment: a LOAD that was waiting reads the tape it
//! finds, and stops with BREAK or `?LOAD ERROR` if it isn't the one it was reading; a SAVE
//! that was recording stops with BREAK unless RECORD and PLAY are down on the host's deck.

use crate::board::VideoBoard;
use crate::cpu::{Cpu, CpuState};
use crate::keys::KEY_ROWS;
use crate::machine::{HELD_KEYS, LINES_PER_FRAME, Machine, Observation, RamSize, CYCLES_PER_FRAME};
use crate::shifter::Shifter;
use crate::sound::{AudioLog, Timer};

pub const MAGIC: &[u8; 4] = b"UNCS";
/// Bumped whenever the layout changes. Older versions this build knows the layout of load;
/// anything else is refused, never guessed at.
pub const VERSION: u8 = 5;
/// The oldest version `load_state` reads: version 1, from before the video board.
pub const OLDEST: u8 = 1;

pub(crate) struct Writer(pub Vec<u8>);

impl Writer {
    pub fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    pub fn bool(&mut self, v: bool) {
        self.0.push(v as u8);
    }
    pub fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub fn bytes(&mut self, v: &[u8]) {
        self.0.extend_from_slice(v);
    }
}

pub(crate) struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8], String> {
        if self.0.len() < n {
            return Err("save state: too short".into());
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Ok(a)
    }
    pub fn u8(&mut self) -> Result<u8, String> {
        Ok(self.bytes(1)?[0])
    }
    pub fn bool(&mut self) -> Result<bool, String> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            v => Err(format!("save state: {v} isn't a flag")),
        }
    }
    pub fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.bytes(2)?.try_into().unwrap()))
    }
    pub fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.bytes(4)?.try_into().unwrap()))
    }
    pub fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.bytes(8)?.try_into().unwrap()))
    }
}

impl Machine {
    /// The whole computer as bytes, in the versioned format above: memory, the processor,
    /// every I/O register, the timer and speaker, the shifter, the keyboard, the frame and
    /// cycle counters, the RAM size. Not the tape deck: see `load_state`. Rev A.
    pub fn save_state(&self) -> Vec<u8> {
        let mut w = Writer(Vec::with_capacity(0xC200));
        w.bytes(MAGIC);
        w.u8(VERSION);
        let ram_end = self.mem.ram_size.end() as usize;
        w.u8(if self.mem.ram_size == RamSize::K64 { 64 } else { 16 });
        let c = &self.cpu;
        for v in [c.a, c.x, c.y, c.sp] {
            w.u8(v);
        }
        w.u16(c.pc);
        w.u8(c.p);
        w.u64(c.cycles);
        let (state, op) = match c.state {
            CpuState::Running => (0, 0),
            CpuState::Halted => (1, 0),
            CpuState::Jammed(op) => (2, op),
        };
        w.u8(state);
        w.u8(op);
        w.bool(c.irq);
        let io = &self.mem.io;
        for v in [io.vmode, io.raster_hi, io.raster_cmp, io.irq_enable, io.irq_status, io.frame, io.kbd] {
            w.u8(v);
        }
        w.u8(io.held.len() as u8);
        w.bytes(&io.held);
        // (CASE rides in bit 7 of TEXTROWS's byte)
        for v in [io.border, io.background, io.charset, io.bitmap, io.text_rows | io.case << 7] {
            w.u8(v);
        }
        w.u64(io.cycle);
        io.timer.save(&mut w);
        w.u16(io.shifter.window);
        w.u8(io.shifter.amount);
        w.bytes(&io.keys);
        w.bool(io.speaker);
        w.u32(io.speaker_toggles);
        match &self.mem.board {
            None => w.u8(0),
            Some(b) => {
                w.u8(1);
                b.save(&mut w);
            }
        }
        match &self.mem.sound {
            None => w.u8(0),
            Some(c) => {
                // as it stands now: run up to this cycle
                let mut c = c.clone();
                c.catch_up(self.cpu.cycles + self.idle_cycles);
                w.u8(1);
                c.chip.save(&mut w);
            }
        }
        let d = &self.mem.deck;
        w.bool(d.motor);
        w.u8(d.data);
        w.bool(d.ready);
        w.bool(d.overrun);
        w.u32(self.mem.frame_cycle);
        w.bytes(&self.border_lines);
        w.bytes(&self.mem.ram[..ram_end]);
        w.0
    }

    /// Puts the computer back as `save_state` saw it. On an error (not a save state, a
    /// version this build doesn't know, damaged) the machine is left exactly as it was.
    ///
    /// The tape deck stays as the host has it now: the tape in it, its counter and the
    /// PLAY key don't change. Only the computer's side of the tape interface (the motor
    /// and the data latch) comes from the save. The host's `observe` counts start again
    /// from 0, and the speaker's sound starts afresh from this moment.
    pub fn load_state(&mut self, bytes: &[u8]) -> Result<(), String> {
        let mut r = Reader(bytes);
        if r.bytes(4).ok() != Some(&MAGIC[..]) {
            return Err("not a UNC 64 save state".into());
        }
        let version = r.u8()?;
        if !(OLDEST..=VERSION).contains(&version) {
            return Err(format!("save state version {version}: this machine reads versions {OLDEST} to {VERSION}"));
        }
        let ram_size = match r.u8()? {
            16 => RamSize::K16,
            64 => RamSize::K64,
            k => return Err(format!("save state: {k}K of RAM isn't a UNC 64")),
        };
        let (a, x, y, sp) = (r.u8()?, r.u8()?, r.u8()?, r.u8()?);
        let (pc, p, cycles) = (r.u16()?, r.u8()?, r.u64()?);
        let state = match (r.u8()?, r.u8()?) {
            (0, _) => CpuState::Running,
            (1, _) => CpuState::Halted,
            (2, op) => CpuState::Jammed(op),
            (s, _) => return Err(format!("save state: processor state {s}")),
        };
        let irq = r.bool()?;
        let cpu = Cpu { a, x, y, sp, pc, p, cycles, state, irq, nmi: false };

        let mut io = crate::machine::Io::default();
        let b = r.bytes(7)?;
        (io.vmode, io.raster_hi, io.raster_cmp, io.irq_enable, io.irq_status, io.frame, io.kbd) =
            (b[0], b[1], b[2], b[3], b[4], b[5], b[6]);
        let held = r.u8()? as usize;
        if held > HELD_KEYS {
            return Err("save state: too many keys held".into());
        }
        io.held = r.bytes(held)?.to_vec();
        let b = r.bytes(4)?;
        (io.border, io.background, io.charset, io.bitmap) = (b[0], b[1], b[2], b[3]);
        let rows = if version >= 5 { r.u8()? } else { crate::machine::io::TEXTROWS_AT_RESET };
        io.text_rows = (rows & 0x7F).min(25);
        io.case = rows >> 7;
        io.cycle = r.u64()?;
        io.timer = Timer::restore(&mut r)?;
        io.shifter = Shifter { window: r.u16()?, amount: r.u8()? & 7 };
        io.keys = r.bytes(KEY_ROWS)?.try_into().unwrap();
        io.speaker = r.bool()?;
        io.speaker_toggles = r.u32()?;
        let board = match version {
            1 => None,
            _ => match r.u8()? {
                0 => None,
                1 if version == 2 => Some(VideoBoard::restore_v2(&mut r)?),
                1 => Some(VideoBoard::restore(&mut r)?),
                v => return Err(format!("save state: video board {v}")),
            },
        };
        let sound = match version {
            // from before the chip: the machine has one now, as at switch-on
            1..=3 => Some(crate::chip::Chip::default()),
            _ => match r.u8()? {
                0 => None,
                1 => Some(crate::chip::Chip::restore(&mut r)?),
                v => return Err(format!("save state: sound chip {v}")),
            },
        };
        let (motor, data, ready, overrun) = (r.bool()?, r.u8()?, r.bool()?, r.bool()?);
        let frame_cycle = r.u32()?;
        if frame_cycle >= CYCLES_PER_FRAME {
            return Err("save state: bad frame position".into());
        }
        let border: [u8; LINES_PER_FRAME as usize] = r.bytes(LINES_PER_FRAME as usize)?.try_into().unwrap();
        let ram_end = ram_size.end() as usize;
        let ram = r.bytes(ram_end)?;
        if !r.0.is_empty() {
            return Err("save state: unexpected bytes at the end".into());
        }

        // Everything read and checked: now change the machine.
        io.audio = AudioLog::resume(cycles - frame_cycle as u64, io.speaker, io.timer);
        let m = &mut self.mem;
        m.ram_size = ram_size;
        m.ram.fill(0);
        m.ram[..ram_end].copy_from_slice(ram);
        m.io = io;
        m.board = board;
        m.sound = sound.map(|mut c| {
            if version < 4 {
                // as if it had been in, at rest, since switch-on: its sample clock where it would be
                c.clock(cycles % crate::chip::CYCLES_PER_SAMPLE as u64);
                c.samples.clear();
            }
            Box::new(crate::chip::Card::at(c, cycles))
        });
        m.deck.motor = motor;
        m.deck.data = data;
        m.deck.ready = ready;
        m.deck.overrun = overrun;
        m.frame_cycle = frame_cycle;
        *m.obs = Observation::default();
        self.cpu = cpu;
        self.border_lines = border;
        self.idle_cycles = 0; // the saved clock is the processor's: the sound resumes from it
        self.redraw();
        Ok(())
    }
}
