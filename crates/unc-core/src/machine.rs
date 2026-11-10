//! The UNC 64 as a whole: memory map, I/O, raster timing, breakpoints. Rev A.
//! The spec for everything here is docs/prg/programmers-reference-guide.md (chapters 8-13, Appendix B).

use crate::board::{self, VideoBoard};
use crate::chip;
use crate::charset::CHAR_ROM;
use crate::cpu::{Bus, Cpu, CpuState};
use crate::keys::KEY_ROWS;
use crate::shifter::Shifter;
use crate::sound::{AudioLog, Timer, noise};

pub const SCREEN: u16 = 0x0400;
pub const SCREEN_COLS: usize = 40;
pub const SCREEN_ROWS: usize = 25;
pub const ROM_START: u16 = 0xE000;
/// The UNSCII character ROM, visible to the processor in the I/O area.
pub const CHAR_ROM_START: u16 = 0xD000;
/// Where the bitmap sits after reset: 8,000 bytes from $2000, RAM even on the UNC 16.
pub const BITMAP_START: u16 = 0x2000;
/// Bytes per bitmap line, and the first bitmap line that shows text in mixed mode at reset:
/// TEXTROWS is 4 then, and the split is 200 - 8 * TEXTROWS.
pub const BITMAP_STRIDE: usize = 40;
pub const MIXED_SPLIT: usize = 168;
/// The text screen in pixels: 40 x 25 cells of 8 x 8.
pub const PIXELS_W: usize = SCREEN_COLS * 8;
pub const PIXELS_H: usize = SCREEN_ROWS * 8;
pub const CYCLES_PER_LINE: u32 = 64;
pub const LINES_PER_FRAME: u32 = 312;
pub const CYCLES_PER_FRAME: u32 = CYCLES_PER_LINE * LINES_PER_FRAME;
/// The raster line the picture's first line is drawn on (chapter 9): lines 51-250.
pub const FIRST_PICTURE_LINE: usize = 51;
/// The line the beam leaves the picture on, into the bottom border: FRAME goes up and VBLANK
/// is raised as it starts (chapter 9).
pub const VBLANK_LINE: u32 = 251;

pub mod io {
    pub const VMODE: u16 = 0xC000;
    pub const RASTER: u16 = 0xC001;
    pub const RASTER_HI: u16 = 0xC002;
    pub const RASTER_CMP: u16 = 0xC003;
    pub const IRQ_ENABLE: u16 = 0xC004;
    pub const IRQ_STATUS: u16 = 0xC005;
    pub const FRAME: u16 = 0xC006;
    pub const KBD: u16 = 0xC010;
    pub const KBD_STROBE: u16 = 0xC011;
    /// Rev A: the most recently pressed key that is still down, or 0.
    pub const HELD: u16 = 0xC012;
    pub const BORDER: u16 = 0xC020;
    pub const BACKGROUND: u16 = 0xC021;
    pub const CHARSET: u16 = 0xC022;
    pub const BITMAP: u16 = 0xC023;
    /// TEXTROWS: in mixed mode, how many text rows show under the bitmap, 0-25. 4 at reset.
    pub const TEXTROWS: u16 = 0xC024;
    /// TEXTROWS at reset: rows 21-24.
    pub const TEXTROWS_AT_RESET: u8 = 4;
    pub const BEEPER: u16 = 0xC030;
    /// The tape deck's status (read) and the motor (write). Chapter 14.
    pub const TAPE: u16 = 0xC040;
    /// Playing: the byte the tape last delivered; reading it takes the byte. Recording: the
    /// next byte to record; writing it gives the deck the byte.
    pub const TAPE_DATA: u16 = 0xC041;
    pub const RESERVED: u16 = 0xC3F0;

    /// TAPE bit 0: the PLAY key is down.
    pub const TAPE_PLAY: u8 = 0x01;
    /// TAPE bit 1: a tape is in the deck.
    pub const TAPE_IN: u8 = 0x02;
    /// TAPE bit 2: the motor is on. Writing TAPE sets it from this bit.
    pub const TAPE_MOTOR: u8 = 0x04;
    /// TAPE bit 3: the RECORD key is down (it goes down with PLAY). Rev A.
    pub const TAPE_RECORD: u8 = 0x08;
    /// TAPE bit 4: recording, and TAPE_DATA is empty: room for the next byte. Rev A.
    pub const TAPE_ROOM: u8 = 0x10;
    /// TAPE bit 6: a byte arrived before the one before it was taken; that one is lost.
    /// Playing only.
    pub const TAPE_OVERRUN: u8 = 0x40;
    /// TAPE bit 7: TAPE_DATA holds a byte not taken yet. Playing only.
    pub const TAPE_READY: u8 = 0x80;

    pub const IRQ_RASTER: u8 = 0x01;
    pub const IRQ_VBLANK: u8 = 0x02;

    /// VMODE bit 0: 256 glyphs from a 2K character set, no hardware reverse video.
    pub const VMODE_256: u8 = 0x01;
    /// VMODE bit 1: the 320 x 200 bitmap, one bit per pixel, at BITMAP * $100.
    pub const VMODE_BITMAP: u8 = 0x02;
    /// VMODE bit 2: with the bitmap on, the last TEXTROWS text rows show below the bitmap
    /// (at reset rows 21-24, below its top 168 lines).
    pub const VMODE_MIXED: u8 = 0x04;
    /// VMODE bit 3: no hardware blink. Code $A0 shows as a steady reversed space.
    pub const VMODE_STEADY: u8 = 0x08;

    /// Key codes for the cursor keys.
    pub const KEY_LEFT: u8 = 28;
    pub const KEY_RIGHT: u8 = 29;
    pub const KEY_UP: u8 = 30;
    pub const KEY_DOWN: u8 = 31;
    /// The INSERT key (shift and DELETE): the screen editor opens a space at the cursor.
    pub const KEY_INSERT: u8 = 20;

    // Rev A: the beeper's timer and the noise register (chapter 12), the shifter
    // (chapter 8) and the keyboard matrix (chapter 11).
    /// SPEAKER: bit 0 the timer's gate, bit 1 speaker data; bit 5 reads the timer's output.
    pub const SPEAKER: u16 = 0xC031;
    /// TIMER_LO and TIMER_HI: the timer's 16-bit divisor. Writing TIMER_HI loads it.
    pub const TIMER_LO: u16 = 0xC032;
    pub const TIMER_HI: u16 = 0xC033;
    /// NOISE: a pseudo-random byte that depends on the cycle it's read at.
    pub const NOISE: u16 = 0xC034;
    pub const SPEAKER_GATE: u8 = 0x01;
    pub const SPEAKER_DATA: u8 = 0x02;
    pub const SPEAKER_OUT: u8 = 0x20;
    /// SHIFT_IN: pushes a byte into the shifter's 16-bit window.
    pub const SHIFT_IN: u16 = 0xC050;
    /// SHIFT_AMOUNT: 0-7, how far right SHIFT_OUT moves the window.
    pub const SHIFT_AMOUNT: u16 = 0xC051;
    /// SHIFT_OUT: the window moved right by SHIFT_AMOUNT, low byte.
    pub const SHIFT_OUT: u16 = 0xC052;
    /// SHIFT_FLIP: pushes a byte with its bits reversed, for drawing a shape mirrored.
    pub const SHIFT_FLIP: u16 = 0xC053;
    /// KEYS to KEYS + 8: the keyboard matrix, nine rows of eight keys, 1 while down.
    pub const KEYS: u16 = 0xC060;

    // The video board, Rev A (chapter 15): `$C080-$C0BF`. Without the board every one
    // of them reads $FF and ignores writes; with it, the ones not named here do too.
    /// SPRTAB: high byte of the sprite table's page: 32 entries of 4 bytes. $37 at reset.
    pub const SPRTAB: u16 = 0xC0A0;
    /// SPRBANK: high byte of the sprite images' bank. $38 at reset.
    pub const SPRBANK: u16 = 0xC0A1;
    /// SPRHIT: table sprites 0-7 that touched another sprite since the last read. Reading clears it.
    pub const SPRHIT: u16 = 0xC0A4;
    /// SPRHITBG: table sprites 0-7 that touched the picture since the last read. Reading clears it.
    pub const SPRHITBG: u16 = 0xC0A5;
    /// SPRLINE: bit 7 if a line had more than eight sprites since the last read. Reading clears it.
    pub const SPRLINE: u16 = 0xC0A6;
    /// SCROLLX, SCROLLY: the main picture's fine scroll, bits 0-2.
    pub const SCROLLX: u16 = 0xC0A8;
    pub const SCROLLY: u16 = 0xC0A9;
    /// LAYER: bit 0 shows the background layer.
    pub const LAYER: u16 = 0xC0AA;
    /// LAYERPAGE: high byte of the layer's 40 x 25 screen. $3C at reset.
    pub const LAYERPAGE: u16 = 0xC0AB;
    /// LAYERSX, LAYERSY: the layer's fine scroll, bits 0-2.
    pub const LAYERSX: u16 = 0xC0AC;
    pub const LAYERSY: u16 = 0xC0AD;
    /// COLOURMODE: bit 0, the 4-colour bitmap (160 x 200).
    pub const COLOURMODE: u16 = 0xC0B0;
    /// COLOUR1-COLOUR3: the colours (0-15) of colour numbers 1-3. 11, 15, 1 at reset.
    pub const COLOUR1: u16 = 0xC0B1;
    pub const COLOUR2: u16 = 0xC0B2;
    pub const COLOUR3: u16 = 0xC0B3;
    /// The sound chip (manual ch. 12): `$C0C0-$C0FF`. Without it every one of them
    /// reads $FF and ignores writes. Voice 1 is eight registers from AUNT, voice 2 from
    /// AUNT + 8, voice 3 from AUNT + 16 (`chip::FREQ_LO` and the rest are the offsets).
    pub const AUNT: u16 = 0xC0C0;
    pub const CUTOFF: u16 = 0xC0D8;
    pub const FILTER: u16 = 0xC0D9;
    pub const VOLUME: u16 = 0xC0DA;
    pub const VOICE3: u16 = 0xC0DC;
    pub const ENV3: u16 = 0xC0DD;
    /// SOUND: $16 with the chip in.
    pub const SOUND: u16 = 0xC0DF;
    /// WAVETABLE to WAVETABLE + 31: the drawn waveform.
    pub const WAVETABLE: u16 = 0xC0E0;
    /// BOARD: $15 with the board in, $FF without.
    pub const BOARD: u16 = 0xC0BF;
}

/// Frames the hosts run after switching on, so the ROM has booted to BASIC before a program
/// is loaded: about a frame and a half to READY., then the switch-on beep, 15 frames of it.
/// A program loaded during the beep would start with the tone still sounding.
pub const BOOT_FRAMES: u32 = 18;
/// How many keys the machine remembers as held down at once.
pub const HELD_KEYS: usize = 8;
/// What the reserved range `$C3F0-$C3FF` answers on a new machine: 19, 86, then 0s.
pub const RESERVED_AT_START: [u8; 16] = [19, 86, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
/// Cycles between two bytes off the tape while it moves: 12 bytes a frame, about 601 a second.
pub const TAPE_BYTE_CYCLES: u32 = CYCLES_PER_FRAME / 12;

pub static SYSTEM_ROM: &[u8; 0x2000] = include_bytes!("../../../rom/system.bin");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RamSize {
    K16,
    K64,
}

impl RamSize {
    /// First address past installed RAM. The 64K machine shows 48K below the I/O and ROM.
    pub const fn end(self) -> u32 {
        match self {
            RamSize::K16 => 0x4000,
            RamSize::K64 => 0xC000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Io {
    pub vmode: u8,
    /// Bit 8 of the raster line, latched when RASTER is read. RASTER_HI reads this.
    pub raster_hi: u8,
    pub raster_cmp: u8,
    pub irq_enable: u8,
    pub irq_status: u8,
    pub frame: u8,
    pub kbd: u8,
    /// Keys that are down, oldest first. HELD reads the last one.
    pub held: Vec<u8>,
    pub border: u8,
    pub background: u8,
    /// High byte of the character set's address. Resets to $D0, the UNSCII ROM.
    pub charset: u8,
    /// High byte of the bitmap's address. Resets to $20: $2000-$3F3F.
    pub bitmap: u8,
    /// TEXTROWS: the text rows under the bitmap in mixed mode, 0-25. Resets to 4.
    pub text_rows: u8,
    /// Rev A (chapter 12): the cycle count when the instruction running now began.
    /// I/O happens at that cycle: the timer, NOISE and the audio log go by it.
    pub cycle: u64,
    /// The speaker's timer: SPEAKER bit 0 (the gate), TIMER_LO and TIMER_HI.
    pub timer: Timer,
    /// The speaker over the frame, for the hosts' sound (`Machine::audio_frame`).
    pub audio: AudioLog,
    /// The shifter (chapter 8).
    pub shifter: Shifter,
    /// The keyboard matrix (chapter 11): a bit is 1 while its key is down.
    pub keys: [u8; KEY_ROWS],
    /// Speaker data, SPEAKER bit 1. Any write to BEEPER flips it.
    pub speaker: bool,
    pub speaker_toggles: u32,
}

impl Default for Io {
    fn default() -> Self {
        Io {
            vmode: 0,
            raster_hi: 0,
            raster_cmp: 0,
            irq_enable: 0,
            irq_status: 0,
            frame: 0,
            kbd: 0,
            held: Vec::new(),
            border: 0,
            background: 0,
            charset: (CHAR_ROM_START >> 8) as u8,
            bitmap: (BITMAP_START >> 8) as u8,
            text_rows: io::TEXTROWS_AT_RESET,
            cycle: 0,
            timer: Timer::default(),
            audio: AudioLog::default(),
            shifter: Shifter::default(),
            keys: [0; KEY_ROWS],
            speaker: false,
            speaker_toggles: 0,
        }
    }
}

/// The cassette deck (chapter 14). The tape and the keys are the deck's; the motor, the
/// data latch and its two flags are the computer's side of the interface.
///
/// Playing, a byte comes off the tape into the latch every `TAPE_BYTE_CYCLES`. Recording
/// (RECORD down, which takes PLAY down with it), the computer writes a byte into the latch
/// and the deck puts it on the tape at the next byte time, at the head's place: over what
/// was there, or on the end, so a blank tape grows with what's recorded. Rev A.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Deck {
    /// The tape in the deck: the bytes on it, from the start. A blank tape is empty.
    pub tape: Option<Vec<u8>>,
    /// The counter: how many bytes have gone past the head since the tape was rewound.
    pub position: usize,
    /// The PLAY key is down.
    pub play: bool,
    /// The RECORD key is down. It only goes down with PLAY, and comes up with it.
    pub record: bool,
    /// The motor, switched by the computer through TAPE bit 2.
    pub motor: bool,
    /// The latch: the last byte the tape delivered, or (recording) the next one to record.
    pub data: u8,
    /// `data` is in transit: not taken by the computer yet (playing), or not recorded yet
    /// (recording).
    pub ready: bool,
    /// A byte arrived while `ready` was still set, and the one before it was lost.
    pub overrun: bool,
    /// Cycles the tape has moved towards the next byte.
    clock: u32,
    /// Host-only, for scripted runs: the next byte arrives the moment the computer has taken
    /// the last one, instead of every `TAPE_BYTE_CYCLES`. The ROM's LOAD runs exactly as
    /// usual, only as fast as it can read; it can't overrun. Recording, each byte goes on
    /// the tape as soon as it's written, so SAVE runs as fast as it can write. Not saved,
    /// not a machine feature: nothing a program reads changes but the timing.
    pub turbo: bool,
}

impl Deck {
    /// The tape moves (and data flows) only while PLAY is down and the motor is on.
    pub fn moving(&self) -> bool {
        self.play && self.motor && self.tape.is_some()
    }

    /// What TAPE reads.
    pub fn status(&self) -> u8 {
        let bit = |on: bool, b: u8| if on { b } else { 0 };
        let latch = if self.record {
            bit(!self.ready, io::TAPE_ROOM)
        } else {
            bit(self.overrun, io::TAPE_OVERRUN) | bit(self.ready, io::TAPE_READY)
        };
        bit(self.play, io::TAPE_PLAY)
            | bit(self.tape.is_some(), io::TAPE_IN)
            | bit(self.motor, io::TAPE_MOTOR)
            | bit(self.record, io::TAPE_RECORD)
            | latch
    }

    /// RECORD goes down or up. Whatever was in the latch was going the other way: it's dropped.
    fn set_record(&mut self, on: bool) {
        if self.record != on {
            self.ready = false;
            self.overrun = false;
        }
        self.record = on;
    }

    /// The computer writes TAPEDATA: with RECORD down, the next byte to record (one still
    /// waiting is replaced). Without it, nothing happens.
    fn write(&mut self, value: u8) {
        if self.record {
            self.data = value;
            self.ready = true;
        }
    }

    /// The tape moves on by `cycles`: a byte every `TAPE_BYTE_CYCLES`. Past the last byte,
    /// the deck's auto-stop lets the PLAY key up (but not while recording: the tape grows).
    fn advance(&mut self, cycles: u32) {
        if !self.moving() {
            return;
        }
        if self.record {
            return self.advance_recording(cycles);
        }
        if self.turbo {
            if !self.ready {
                self.next_byte();
            }
            return;
        }
        self.clock += cycles;
        while self.clock >= TAPE_BYTE_CYCLES {
            self.clock -= TAPE_BYTE_CYCLES;
            if !self.next_byte() {
                return;
            }
        }
    }

    /// Recording: at each byte time the deck records the byte in the latch, if there is one.
    /// A byte time with the latch empty records nothing (the tape carries bytes, not sound).
    /// Turbo records each byte as soon as it's written.
    fn advance_recording(&mut self, cycles: u32) {
        if self.turbo {
            self.record_byte();
            return;
        }
        self.clock += cycles;
        while self.clock >= TAPE_BYTE_CYCLES {
            self.clock -= TAPE_BYTE_CYCLES;
            self.record_byte();
        }
    }

    /// The byte in the latch goes on the tape at the head, over what was there or on the end.
    fn record_byte(&mut self) {
        if !self.ready {
            return;
        }
        let Some(tape) = self.tape.as_mut() else { return };
        match tape.get_mut(self.position) {
            Some(b) => *b = self.data,
            None => tape.push(self.data),
        }
        self.position += 1;
        self.ready = false;
    }

    /// The next byte reaches the head; past the last one, PLAY comes up. Returns whether the
    /// tape is still playing.
    fn next_byte(&mut self) -> bool {
        let tape = self.tape.as_deref().unwrap_or_default();
        match tape.get(self.position) {
            Some(&b) => {
                self.overrun |= self.ready;
                self.data = b;
                self.ready = true;
                self.position += 1;
                true
            }
            None => {
                self.play = false;
                self.clock = 0;
                false
            }
        }
    }
}

/// What the host has seen the processor do since the last `Machine::observe`. Rev A
/// draft. Host-only bookkeeping: nothing a program can read, and nothing the machine
/// does depends on it, so determinism is untouched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Observation {
    /// Processor reads of each I/O register, `$C000` + index.
    pub reads: [u32; 256],
    /// Processor writes to each I/O register, `$C000` + index.
    pub writes: [u32; 256],
    /// Something read the reserved range, `$C3F0-$C3FF`.
    pub reserved_read: bool,
    /// The last write the processor made to the reserved range: (offset 0-15, value). The
    /// write changes nothing in the machine; the host may make something of it.
    pub reserved_write: Option<(u8, u8)>,
    /// Frames that ended.
    pub frames: u32,
    /// Processor writes to screen memory, `$0400-$07E7`.
    pub screen_writes: u32,
    /// Processor writes to the 8,000 bytes from BITMAP * `$100`, wherever BITMAP points
    /// at the time of the write (and whether or not the bitmap is showing).
    pub bitmap_writes: u32,
    /// Writes to the high byte of the BASIC line number (CURLIN+1, `$0206`): the ROM writes
    /// it each time a program arrives on a line (host-only, for line breakpoints).
    pub line_writes: u32,
}

impl Default for Observation {
    fn default() -> Self {
        Observation { reads: [0; 256], writes: [0; 256], reserved_read: false, reserved_write: None, frames: 0, screen_writes: 0, bitmap_writes: 0, line_writes: 0 }
    }
}

impl Observation {
    /// Reads of the I/O register at `addr` (`$C000-$C0FF`).
    pub fn read_count(&self, addr: u16) -> u32 {
        if addr >> 8 == 0xC0 { self.reads[addr as u8 as usize] } else { 0 }
    }

    /// Writes to the I/O register at `addr` (`$C000-$C0FF`).
    pub fn write_count(&self, addr: u16) -> u32 {
        if addr >> 8 == 0xC0 { self.writes[addr as u8 as usize] } else { 0 }
    }
}

#[derive(Clone)]
pub struct Memory {
    pub ram: Box<[u8; 0x10000]>,
    pub rom: Box<[u8; 0x2000]>,
    pub ram_size: RamSize,
    pub io: Io,
    /// The tape deck plugged into the computer.
    pub deck: Deck,
    /// Cycles elapsed in the current frame.
    pub frame_cycle: u32,
    /// The host's counters (`Machine::observe`). Only the processor's own reads and writes
    /// count: `peek`, `poke` and `load` from the host don't.
    pub obs: Box<Observation>,
    /// What `$C3F0-$C3FF` answers: 19, 86, then 0s, until the host says otherwise
    /// (`Machine::set_reserved`).
    pub reserved: [u8; 16],
    /// The video board (chapter 15), if it's in.
    pub board: Option<VideoBoard>,
    /// The AUNT, the sound chip (manual ch. 12). In every machine from the UNC 16 up;
    /// `Machine::set_sound(false)` takes it out, for a host that wants a machine without.
    pub sound: Option<Box<chip::Card>>,
    /// The host's frame meter (`Machine::frame_work`): where this frame's work ended.
    pub meter: FrameMeter,
    /// The HOLD box's memory view, when the host watches (`Machine::watch`): when each
    /// byte was written and run, and by what. Host-only, like `obs`.
    pub trace: Option<Box<crate::trace::Trace>>,
}

/// How much of a frame a program spent working before it started waiting: the cycle of the
/// first read in a wait loop, a poll of FRAME, RASTER or the keyboard that reads the same
/// register and value as the poll just before it (under 40 cycles before). Host-only, like `Observation`: nothing a
/// program can see, and nothing the machine does depends on it.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameMeter {
    last_addr: u16,
    last_value: u8,
    last_cycle: u32,
    work: Option<u32>,
    /// The last whole frame's work, in cycles (`CYCLES_PER_FRAME` if it never waited).
    pub last: u32,
}

impl Memory {
    pub fn new(ram_size: RamSize) -> Self {
        Memory {
            ram: Box::new([0; 0x10000]),
            rom: Box::new(*SYSTEM_ROM),
            ram_size,
            io: Io::default(),
            deck: Deck::default(),
            frame_cycle: 0,
            obs: Box::default(),
            reserved: RESERVED_AT_START,
            board: None,
            sound: Some(Box::default()),
            meter: FrameMeter::default(),
            trace: None,
        }
    }

    pub fn raster_line(&self) -> u32 {
        self.frame_cycle / CYCLES_PER_LINE
    }

    /// Read without side effects, for debuggers.
    pub fn peek(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0xBFFF if (addr as u32) < self.ram_size.end() => self.ram[addr as usize],
            0x0000..=0xBFFF => 0xFF,
            0xC000..=0xDFFF => self.io_peek(addr),
            _ => self.rom[(addr - ROM_START) as usize],
        }
    }

    fn io_peek(&self, addr: u16) -> u8 {
        let line = self.raster_line();
        match addr {
            io::VMODE => self.io.vmode,
            io::RASTER => line as u8,
            io::RASTER_HI => self.io.raster_hi,
            io::RASTER_CMP => self.io.raster_cmp,
            io::IRQ_ENABLE => self.io.irq_enable,
            io::IRQ_STATUS => self.io.irq_status,
            io::FRAME => self.io.frame,
            io::KBD | io::KBD_STROBE => self.io.kbd,
            io::HELD => self.io.held.last().copied().unwrap_or(0),
            io::BORDER => self.io.border,
            io::BACKGROUND => self.io.background,
            io::CHARSET => self.io.charset,
            io::BITMAP => self.io.bitmap,
            io::TEXTROWS => self.io.text_rows,
            io::TAPE => self.deck.status(),
            io::TAPE_DATA => self.deck.data,
            0xC3F0..=0xC3FF => self.reserved[(addr - io::RESERVED) as usize],
            0xC031..=0xC03F | 0xC050..=0xC06F => self.io_peek_rev_a(addr),
            0xC080..=0xC0BF => self.board.as_ref().map_or(0xFF, |b| b.peek(addr)),
            0xC0C0..=0xC0FF => self.sound.as_ref().map_or(0xFF, |c| c.chip.peek(addr as u8 - 0xC0)),
            0xD000..=0xD3FF => CHAR_ROM[(addr - CHAR_ROM_START) as usize],
            _ => 0xFF,
        }
    }
}

impl Bus for Memory {
    fn read(&mut self, addr: u16) -> u8 {
        // One test on the hot path: everything else is RAM and ROM, which nothing watches.
        if addr & 0xF000 != 0xC000 {
            return self.peek(addr);
        }
        // the sound chip's voice 3 is read as it stands at this cycle
        if addr & 0xFFC0 == 0xC0C0
            && let Some(c) = &mut self.sound
        {
            c.catch_up(self.io.cycle);
        }
        let v = self.peek(addr);
        self.io_read(addr);
        v
    }

    fn write(&mut self, addr: u16, value: u8) {
        if addr < 0xC000 {
            let obs = &mut self.obs;
            obs.screen_writes = obs.screen_writes.wrapping_add((addr.wrapping_sub(SCREEN) < 1000) as u32);
            obs.bitmap_writes = obs.bitmap_writes.wrapping_add((addr.wrapping_sub((self.io.bitmap as u16) << 8) < 8000) as u32);
            obs.line_writes = obs.line_writes.wrapping_add((addr == 0x0206) as u32);
            if self.trace.is_some() {
                let ram = &self.ram;
                let line = if ram[0x0204] != 0 { u16::from_le_bytes([ram[0x0205], ram[0x0206]]) } else { crate::trace::NO_LINE };
                let t = self.trace.as_deref_mut().unwrap();
                t.written[addr as usize] = t.frame;
                t.writer[addr as usize] = t.pc;
                t.line[addr as usize] = line;
            }
        } else if addr >> 8 == 0xC0 {
            let w = &mut self.obs.writes[addr as u8 as usize];
            *w = w.wrapping_add(1);
        } else if addr & 0xFFF0 == io::RESERVED {
            self.obs.reserved_write = Some(((addr & 15) as u8, value));
        }
        self.store(addr, value);
    }
}

impl Memory {
    /// Cycles since the beam last left the picture (VBLANK, chapter 9), where a frame's work
    /// starts: the frame meter's clock.
    fn since_vblank(&self) -> u32 {
        (self.frame_cycle + CYCLES_PER_FRAME - VBLANK_LINE * CYCLES_PER_LINE) % CYCLES_PER_FRAME
    }

    /// The side effects of the processor reading I/O, and the host's counts of it.
    fn io_read(&mut self, addr: u16) {
        if matches!(addr, io::FRAME | io::RASTER | io::RASTER_HI | io::KBD | io::KBD_STROBE) {
            let v = self.peek(addr);
            let t = self.since_vblank();
            let m = &mut self.meter;
            // a wait loop polls within a few cycles of itself; a game's next frame reads the
            // same register thousands of cycles later, after its work
            if m.work.is_none() && m.last_addr == addr && m.last_value == v
                && t.wrapping_sub(m.last_cycle) < 40
            {
                m.work = Some(m.last_cycle);
            }
            m.last_addr = addr;
            m.last_value = v;
            m.last_cycle = t;
        }
        match addr {
            0xC000..=0xC0FF => {
                let r = &mut self.obs.reads[addr as u8 as usize];
                *r = r.wrapping_add(1);
            }
            0xC3F0..=0xC3FF => self.obs.reserved_read = true,
            _ => {}
        }
        match addr {
            io::KBD_STROBE => self.io.kbd &= 0x7F,
            io::TAPE_DATA if !self.deck.record => {
                self.deck.ready = false;
                self.deck.overrun = false;
            }
            // Reading the low byte latches the high bit, so LO then HI is one line number.
            io::RASTER => self.io.raster_hi = (self.raster_line() >> 8) as u8,
            io::SPRHIT | io::SPRHITBG | io::SPRLINE => {
                if let Some(b) = &mut self.board {
                    b.take(addr);
                }
            }
            _ => {}
        }
    }

    /// A write with all its effects on the machine, uncounted: the processor's writes come
    /// through `Bus::write`, which counts them first; the host's `poke` and `load` come here.
    pub fn store(&mut self, addr: u16, value: u8) {
        match addr {
            0x0000..=0xBFFF if (addr as u32) < self.ram_size.end() => self.ram[addr as usize] = value,
            0xC000..=0xDFFF => match addr {
                io::VMODE => self.io.vmode = value,
                io::RASTER_CMP => self.io.raster_cmp = value,
                io::IRQ_ENABLE => self.io.irq_enable = value & 0x03,
                io::IRQ_STATUS => self.io.irq_status &= !value,
                io::KBD_STROBE => self.io.kbd &= 0x7F,
                io::BORDER => self.io.border = value & 0x0F,
                io::BACKGROUND => self.io.background = value & 0x0F,
                io::CHARSET => self.io.charset = value,
                io::BITMAP => self.io.bitmap = value,
                io::TEXTROWS => self.io.text_rows = value.min(SCREEN_ROWS as u8),
                io::BEEPER => {
                    self.io.speaker = !self.io.speaker;
                    self.io.speaker_toggles = self.io.speaker_toggles.wrapping_add(1);
                }
                io::TAPE => self.deck.motor = value & io::TAPE_MOTOR != 0,
                io::TAPE_DATA => self.deck.write(value),
                0xC080..=0xC0BF => {
                    if let Some(b) = &mut self.board {
                        b.store(addr, value);
                    }
                }
                0xC0C0..=0xC0FF => {
                    if let Some(c) = &mut self.sound {
                        c.catch_up(self.io.cycle);
                        c.chip.store(addr as u8 - 0xC0, value);
                    }
                }
                _ => {}
            },
            _ => {}
        }
        if let 0xC030..=0xC03F | 0xC050..=0xC06F = addr {
            self.io_write_rev_a(addr, value);
        }
    }
}

/// The Rev A registers: the beeper's timer and NOISE, the shifter, the keyboard matrix.
impl Memory {
    fn io_peek_rev_a(&self, addr: u16) -> u8 {
        let io = &self.io;
        match addr {
            io::SPEAKER => {
                let bit = |on: bool, b: u8| if on { b } else { 0 };
                bit(io.timer.gate, io::SPEAKER_GATE)
                    | bit(io.speaker, io::SPEAKER_DATA)
                    | bit(io.timer.out(io.cycle), io::SPEAKER_OUT)
            }
            io::TIMER_LO => io.timer.lo,
            io::TIMER_HI => io.timer.hi,
            io::NOISE => noise(io.cycle),
            io::SHIFT_AMOUNT => io.shifter.amount,
            io::SHIFT_OUT => io.shifter.out(),
            a if (io::KEYS..io::KEYS + KEY_ROWS as u16).contains(&a) => io.keys[(a - io::KEYS) as usize],
            _ => 0xFF,
        }
    }

    /// Runs after the main write: BEEPER has already flipped `speaker`.
    fn io_write_rev_a(&mut self, addr: u16, value: u8) {
        let io = &mut self.io;
        let t = io.cycle;
        match addr {
            io::BEEPER => {}
            io::SPEAKER => {
                io.timer.set_gate(t, value & io::SPEAKER_GATE != 0);
                io.speaker = value & io::SPEAKER_DATA != 0;
            }
            io::TIMER_LO => {
                io.timer.lo = value;
                return;
            }
            io::TIMER_HI => {
                io.timer.hi = value;
                io.timer.commit(t);
            }
            io::SHIFT_IN => io.shifter.push(value),
            io::SHIFT_AMOUNT => io.shifter.amount = value & 7,
            io::SHIFT_FLIP => io.shifter.push_flipped(value),
            _ => {}
        }
        if let io::BEEPER | io::SPEAKER | io::TIMER_HI = addr {
            io.audio.mark(t, io.speaker, io.timer);
        }
    }
}

/// The sprites of one line of the picture, as `compose_line` draws them: which shown entries
/// of the table it found when the line began, and what they touched so far.
pub(crate) struct SpriteLine {
    shown: Vec<(usize, board::Sprite)>,
    pub report: board::SpriteReport,
}

impl SpriteLine {
    pub(crate) fn new(mem: &Memory) -> Self {
        let shown = match &mem.board {
            Some(b) => (0..board::SPRITES).map(|n| (n, b.sprite(&|a| mem.peek(a), n))).filter(|(_, s)| s.shown()).collect(),
            None => Vec::new(),
        };
        SpriteLine { shown, report: board::SpriteReport::default() }
    }
}

/// The video chip: one line of the picture at a time (chapter 9).
impl Memory {
    /// What values 0-3 of the picture show now: BACKGROUND, and white ink without the video
    /// board, COLOUR1-COLOUR3 with it.
    pub(crate) fn colours_now(&self) -> [u8; 4] {
        match &self.board {
            None => [self.io.background, 1, 1, 1],
            Some(b) => [self.io.background, b.colour(1), b.colour(2), b.colour(3)],
        }
    }

    /// How many lines of the picture the bitmap takes, from the top, when it's on: all 200,
    /// or in mixed mode all but the last TEXTROWS text rows (168 at reset).
    pub fn bitmap_lines(&self) -> usize {
        if self.io.vmode & io::VMODE_MIXED != 0 { PIXELS_H - 8 * self.io.text_rows as usize } else { PIXELS_H }
    }

    /// Line y (0-199) of the picture, from the registers and memory as they stand now: the
    /// text or the bitmap, with the video board its fine scroll, the layer behind it and the
    /// sprites over it. The sprites' table is the one `sprites` read; what they touch goes in
    /// its report.
    pub(crate) fn compose_line(&self, y: usize, blink_on: bool, row: &mut [u8; PIXELS_W], sprites: &mut SpriteLine) {
        let board = self.board.as_ref();
        // the screen's ink: 1 without the board, colour number 3 with it
        let ink = if board.is_some() { 3 } else { 1 };
        let (sx, sy) = board.map_or((0, 0), |b| (b.scroll_x as usize, b.scroll_y as usize));
        row.fill(0);
        if y >= sy {
            let src = y - sy;
            let vmode = self.io.vmode;
            let split = self.bitmap_lines();
            let mut line = [0u8; PIXELS_W];
            if vmode & io::VMODE_BITMAP != 0 && src < split {
                let four = board.is_some_and(|b| b.colour_mode & board::COLOURMODE_BITMAP4 != 0);
                let base = (self.io.bitmap as u16) << 8;
                for col in 0..BITMAP_STRIDE {
                    let bits = self.peek(base.wrapping_add((src * BITMAP_STRIDE + col) as u16));
                    for (x, px) in line[col * 8..col * 8 + 8].iter_mut().enumerate() {
                        *px = if four { (bits >> (6 - x / 2 * 2)) & 3 } else { ((bits >> (7 - x)) & 1) * ink };
                    }
                }
            } else {
                self.text_line(SCREEN, src, blink_on, ink, &mut line);
            }
            row[sx..].copy_from_slice(&line[..PIXELS_W - sx]);
        }
        let Some(b) = board else { return };
        if b.layer & 1 != 0 {
            let (lx, ly) = (b.layer_sx as usize, b.layer_sy as usize);
            if y >= ly {
                let mut layer = [0u8; PIXELS_W];
                self.text_line((b.layer_page as u16) << 8, y - ly, blink_on, 1, &mut layer);
                for (px, &l) in row[lx..].iter_mut().zip(&layer[..PIXELS_W - lx]) {
                    if *px == 0 {
                        *px = l;
                    }
                }
            }
        }
        if !sprites.shown.is_empty() {
            let r = b.sprite_line(&|a| self.peek(a), &sprites.shown, y as i32, row, true);
            sprites.report.hit |= r.hit;
            sprites.report.hit_bg |= r.hit_bg;
            sprites.report.over |= r.over;
        }
    }

    /// Line y (0-199) of a 40 x 25 text screen of codes from `screen`, ink drawn as `ink`:
    /// the main screen, or the video board's layer, by the same glyph rules.
    fn text_line(&self, screen: u16, y: usize, blink_on: bool, ink: u8, out: &mut [u8; PIXELS_W]) {
        let wide = self.io.vmode & io::VMODE_256 != 0;
        let blinks = self.io.vmode & io::VMODE_STEADY == 0;
        let base = (self.io.charset as u16) << 8;
        let (cy, gy) = (y / 8, (y % 8) as u16);
        for cx in 0..SCREEN_COLS {
            let code = self.peek(screen.wrapping_add((cy * SCREEN_COLS + cx) as u16));
            let (glyph, reverse) = if wide { (code, false) } else { (code & 0x7F, code & 0x80 != 0) };
            let hidden = !wide && blinks && code == 0xA0 && !blink_on;
            let mut bits = self.peek(base.wrapping_add(glyph as u16 * 8 + gy));
            if reverse {
                bits = !bits;
            }
            if hidden {
                bits = 0;
            }
            for (x, px) in out[cx * 8..cx * 8 + 8].iter_mut().enumerate() {
                *px = ((bits >> (7 - x)) & 1) * ink;
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    /// The cycle budget ran out.
    Budget,
    /// PC reached a breakpoint (the instruction there has not run yet).
    Breakpoint(u16),
    Halted,
    Jammed(u8),
}

#[derive(Clone)]
pub struct Machine {
    pub cpu: Cpu,
    pub mem: Memory,
    breakpoints: Box<[bool; 0x10000]>,
    /// BASIC lines to stop on (host-only, like the breakpoints): the moment a running program
    /// arrives on one, before any of the line has run.
    line_breaks: Vec<u16>,
    /// Border colour as it stood on each raster line during the last frame.
    pub border_lines: [u8; LINES_PER_FRAME as usize],
    /// The picture as the beam drew it, and the colours of each of its lines (`frame`).
    picture: Box<[u8; PIXELS_W * PIXELS_H]>,
    picture_colours: Box<[[u8; 4]; PIXELS_H]>,
    /// The host's cursor blink: whether the cursor cell ($A0) is lit on the lines the beam
    /// draws next. The host sets it, from its own clock, as a monitor's blink would be.
    pub blink_on: bool,
    /// Cycles the machine has run with the processor stopped (halted or jammed): the clock
    /// the raster, the timer and the sound go by is `cpu.cycles` plus these.
    pub idle_cycles: u64,
}

impl Machine {
    /// A UNC 16 (`RamSize::K16`) or one with the 64K expansion, without the video board.
    pub fn new(ram_size: RamSize) -> Self {
        Machine::with_board(ram_size, false)
    }

    /// A machine with the video board in (`board`) or not. Chapter 15.
    pub fn with_board(ram_size: RamSize, board: bool) -> Self {
        let mut mem = Memory::new(ram_size);
        mem.board = board.then(VideoBoard::default);
        let mut m = Machine {
            cpu: Cpu::new(),
            mem,
            breakpoints: Box::new([false; 0x10000]),
            line_breaks: Vec::new(),
            border_lines: [0; LINES_PER_FRAME as usize],
            picture: Box::new([0; PIXELS_W * PIXELS_H]),
            picture_colours: Box::new([[0, 1, 1, 1]; PIXELS_H]),
            blink_on: true,
            idle_cycles: 0,
        };
        m.reset();
        m
    }

    /// Cold reset: RAM cleared, I/O cleared, CPU through the reset vector. The tape deck
    /// isn't part of the computer: the tape stays in, at its place, with PLAY as it was. The
    /// motor stops and the data latch empties, as their I/O registers clear.
    pub fn reset(&mut self) {
        let size = self.mem.ram_size;
        let rom = self.mem.rom.clone();
        let deck = std::mem::take(&mut self.mem.deck);
        let obs = std::mem::take(&mut self.mem.obs);
        let reserved = self.mem.reserved;
        let board = self.mem.board.is_some();
        let sound = self.mem.sound.is_some();
        self.mem = Memory::new(size);
        self.mem.board = board.then(VideoBoard::default);
        // the clock starts again at 0 with the processor's
        self.mem.sound = sound.then(Box::default);
        self.mem.rom = rom;
        self.mem.obs = obs;
        self.mem.reserved = reserved;
        self.mem.deck = Deck { motor: false, ready: false, overrun: false, data: 0, ..deck };
        self.border_lines = [0; LINES_PER_FRAME as usize];
        self.cpu.reset(&mut self.mem);
    }

    /// The RESET button: the processor and the I/O start over as at switch-on, but memory
    /// keeps what it holds. The ROM sees it has been running (WARMMK) and goes back to
    /// `READY.` with the program, as STOP and RESTORE does. It works on anything, a jammed
    /// processor too.
    pub fn reset_button(&mut self) {
        let ram = self.mem.ram.clone();
        self.reset();
        self.mem.ram = ram;
        self.cpu.reset(&mut self.mem);
    }

    pub fn peek(&self, addr: u16) -> u8 {
        self.mem.peek(addr)
    }

    /// Writes from the host: the same effect as the processor's write (an I/O register
    /// does what it does), but `observe` doesn't count it.
    pub fn poke(&mut self, addr: u16, value: u8) {
        self.mem.store(addr, value);
    }

    pub fn load(&mut self, addr: u16, bytes: &[u8]) {
        for (i, &b) in bytes.iter().enumerate() {
            self.mem.store(addr.wrapping_add(i as u16), b);
        }
    }

    /// Host-only: what the reserved byte `$C3F0 + offset` (offset 0-15) answers from now on.
    /// The machine has no way to write there; the story sets it (part one, chapter 8), as it
    /// puts a tape in the deck. A reset keeps it, `Machine::new` starts again at 19, 86 and
    /// 0s, and save states leave it out (the host applies it again after a load, as it
    /// re-inserts a tape). Rev A.
    pub fn set_reserved(&mut self, offset: u8, value: u8) {
        self.mem.reserved[(offset & 15) as usize] = value;
    }

    /// What the processor has done since the last call, for the host's checks (the story's
    /// "did a program read the keyboard and draw?"): reads and writes of each I/O register
    /// in `$C000-$C0FF`, whether anything read `$C3F0-$C3FF`, frames elapsed, and writes to
    /// screen memory and to the bitmap. The counts start again from 0. Rev A.
    ///
    /// Host-only: a program can't see the counts and nothing the machine does depends on
    /// them. The host's own `peek`, `poke` and `load` aren't counted, so a debugger or a
    /// check that reads the machine doesn't disturb what it measures. A reset doesn't
    /// clear them; a cold start (`Machine::new`) begins at 0.
    pub fn observe(&mut self) -> Observation {
        std::mem::take(&mut *self.mem.obs)
    }

    /// Switches on a machine and runs `BOOT_FRAMES` frames, as the hosts do: the ROM has
    /// booted to BASIC and is waiting for a line.
    pub fn boot(ram_size: RamSize) -> Self {
        Machine::boot_with_board(ram_size, false)
    }

    /// As `boot`, with the video board in or not.
    pub fn boot_with_board(ram_size: RamSize, board: bool) -> Self {
        let mut m = Machine::with_board(ram_size, board);
        for _ in 0..BOOT_FRAMES {
            m.run_frame();
        }
        m
    }

    /// Takes the sound chip out, or puts it back (manual ch. 12). Every machine has
    /// it from the start; a chip put back is silent, every register 0. The machine carries
    /// on: no reset.
    pub fn set_sound(&mut self, on: bool) {
        if on != self.mem.sound.is_some() {
            let now = self.cpu.cycles + self.idle_cycles;
            self.mem.sound = on.then(|| Box::new(chip::Card::at(chip::Chip::default(), now)));
        }
    }

    /// Whether the sound chip is in. SOUND (`$C0DF`) reads $16 if it is.
    pub fn has_sound(&self) -> bool {
        self.mem.sound.is_some()
    }

    /// Whether the video board is in (chapter 15). BOARD (`$C0BF`) reads $15 if it is.
    pub fn has_board(&self) -> bool {
        self.mem.board.is_some()
    }

    /// A key goes down: its code goes into KBD with bit 7 set, and HELD reads it until
    /// `key_up` (or until another key goes down).
    pub fn key(&mut self, code: u8) {
        self.key_hold(code);
        self.mem.io.kbd = (code & 0x7F) | 0x80;
    }

    /// A key goes down in the matrix and HELD, but not into KBD: what a key held on a
    /// keyboard does at once, while its press waits for KBD to be free (a program that only
    /// reads HELD, as a game's steering does, never frees it).
    pub fn key_hold(&mut self, code: u8) {
        let code = code & 0x7F;
        self.key_matrix(code, true);
        let held = &mut self.mem.io.held;
        held.retain(|&k| k != code);
        if held.len() == HELD_KEYS {
            held.remove(0);
        }
        held.push(code);
    }

    /// The RESTORE key goes down. It isn't in KBD or the matrix: it pulls the processor's
    /// NMI line (manual chapter 10), once for each press. With STOP held, the ROM's handler
    /// goes back to BASIC's `READY.`; alone, it returns at once.
    pub fn restore(&mut self) {
        self.cpu.nmi();
    }

    /// A key comes up. HELD falls back to the most recent key still down, or 0.
    pub fn key_up(&mut self, code: u8) {
        self.key_matrix(code, false);
        self.mem.io.held.retain(|&k| k != code & 0x7F);
    }

    /// Puts a tape in the deck, rewound to its start, with PLAY and RECORD up. A tape
    /// already in the deck comes out first. `bytes` is the whole tape: one or more programs
    /// in the tape format of chapter 14, one after another.
    pub fn insert_tape(&mut self, bytes: impl Into<Vec<u8>>) {
        let d = &mut self.mem.deck;
        d.tape = Some(bytes.into());
        d.position = 0;
        d.play = false;
        d.set_record(false);
        d.clock = 0;
    }

    /// Puts a blank tape in the deck: nothing on it yet, ready to record on (chapter 14).
    pub fn insert_blank_tape(&mut self) {
        self.insert_tape(Vec::new());
    }

    /// STOP/EJECT: PLAY and RECORD come up and the tape comes out. Returns it, if there was
    /// one: with whatever was recorded on it.
    pub fn eject_tape(&mut self) -> Option<Vec<u8>> {
        let d = &mut self.mem.deck;
        d.play = false;
        d.set_record(false);
        d.position = 0;
        d.clock = 0;
        d.tape.take()
    }

    /// Presses PLAY. It only goes down with a tape in the deck; returns whether it's down.
    /// The tape moves while PLAY is down and the computer runs the motor.
    pub fn press_play(&mut self) -> bool {
        let d = &mut self.mem.deck;
        d.play = d.tape.is_some();
        d.play
    }

    /// Presses RECORD, which takes PLAY down with it, as on a real deck. Both only go down
    /// with a tape in; returns whether they're down. While the computer runs the motor, the
    /// deck records what the computer writes to TAPEDATA (chapter 14). Rev A.
    pub fn press_record(&mut self) -> bool {
        let d = &mut self.mem.deck;
        let on = d.tape.is_some();
        d.play = on;
        d.set_record(on);
        on
    }

    /// Turbo tape, for scripted runs (`Deck::turbo`): LOAD as fast as the ROM can read, and
    /// SAVE as fast as it can write.
    pub fn set_tape_turbo(&mut self, on: bool) {
        self.mem.deck.turbo = on;
    }

    /// Presses STOP: PLAY and RECORD come up and the tape stops where it is.
    pub fn press_stop(&mut self) {
        let d = &mut self.mem.deck;
        d.play = false;
        d.set_record(false);
    }

    /// Winds the tape back to its start (the counter reads 0). PLAY and RECORD come up first.
    pub fn rewind_tape(&mut self) {
        let d = &mut self.mem.deck;
        d.play = false;
        d.set_record(false);
        d.position = 0;
        d.clock = 0;
    }

    pub fn set_breakpoint(&mut self, addr: u16, on: bool) {
        self.breakpoints[addr as usize] = on;
    }

    pub fn breakpoint(&self, addr: u16) -> bool {
        self.breakpoints[addr as usize]
    }

    pub fn clear_breakpoints(&mut self) {
        self.breakpoints.fill(false);
        self.line_breaks.clear();
    }

    /// Stop when a running BASIC program arrives on line `line` (or don't, with `on` false).
    pub fn set_line_break(&mut self, line: u16, on: bool) {
        self.line_breaks.retain(|&l| l != line);
        if on {
            self.line_breaks.push(line);
        }
    }

    /// The BASIC line being run (CURLIN), if a program is running.
    pub fn basic_line(&self) -> Option<u16> {
        (self.mem.ram[0x0204] != 0).then(|| u16::from_le_bytes([self.mem.ram[0x0205], self.mem.ram[0x0206]]))
    }

    /// Host-only: the cycles the last whole frame spent working before its program started
    /// waiting (for FRAME, the raster or a key), or the whole 19,968 if it never waited.
    pub fn frame_work(&self) -> u32 {
        self.mem.meter.last
    }

    /// Runs one instruction and advances the clock. Returns cycles spent (0 if stopped).
    pub fn step(&mut self) -> u8 {
        self.trace_step();
        let c = self.cpu.step(&mut self.mem);
        self.tick(c as u32);
        c
    }

    fn tick(&mut self, cycles: u32) {
        if cycles == 0 {
            return;
        }
        let now = self.cpu.cycles + self.idle_cycles;
        self.mem.io.cycle = now;
        let before = self.mem.raster_line();
        self.mem.frame_cycle += cycles;
        let wrapped = self.mem.frame_cycle >= CYCLES_PER_FRAME;
        if wrapped {
            self.mem.frame_cycle -= CYCLES_PER_FRAME;
            if let Some(t) = self.mem.trace.as_deref_mut() {
                t.frame = t.frame.wrapping_add(1);
            }
        }
        let line = self.mem.raster_line();
        // the beam finishes every line it passed: before the frame ends, its last lines
        if line != before {
            let end = if wrapped { LINES_PER_FRAME } else { line };
            for l in before..end {
                self.beam(l);
            }
            if wrapped {
                for l in 0..line {
                    self.beam(l);
                }
            }
            // the beam leaving the picture: FRAME goes up and VBLANK is raised (chapter 9)
            // (the line before it finished: the beam is on VBLANK_LINE now)
            let left = |from: u32, to: u32| from < VBLANK_LINE && VBLANK_LINE - 1 < to;
            if if wrapped { left(before, LINES_PER_FRAME) || left(0, line) } else { left(before, line) } {
                self.mem.io.frame = self.mem.io.frame.wrapping_add(1);
                self.mem.io.irq_status |= io::IRQ_VBLANK;
                // the frame meter's frame starts here too: a frame's work starts at VBLANK
                let m = &mut self.mem.meter;
                m.last = m.work.take().unwrap_or(CYCLES_PER_FRAME);
                m.last_addr = 0;
            }
        }
        if wrapped {
            self.mem.obs.frames = self.mem.obs.frames.wrapping_add(1);
            let end = now.saturating_sub(self.mem.frame_cycle as u64);
            self.mem.io.audio.end_frame(end);
            if let Some(c) = &mut self.mem.sound {
                c.end_frame(end);
            }
        }
        if line != before && line < 256 && line as u8 == self.mem.io.raster_cmp {
            self.mem.io.irq_status |= io::IRQ_RASTER;
        }
        self.border_lines[line as usize] = self.mem.io.border;
        self.mem.deck.advance(cycles);
        self.cpu.irq = self.mem.io.irq_status & self.mem.io.irq_enable != 0;
    }

    /// The clock running with the processor stopped: `cycles` pass, a raster line at a time.
    /// The processor's own count (`cpu.cycles`) waits with it; `idle_cycles` keeps the rest.
    fn idle(&mut self, cycles: u32) {
        let mut left = cycles;
        while left > 0 {
            let n = left.min(CYCLES_PER_LINE);
            self.idle_cycles += n as u64;
            self.tick(n);
            left -= n;
        }
    }

    fn stopped(&self) -> Option<Stop> {
        match self.cpu.state {
            CpuState::Running => None,
            CpuState::Halted => Some(Stop::Halted),
            CpuState::Jammed(op) => Some(Stop::Jammed(op)),
        }
    }

    /// Runs for at least `budget` cycles, stopping early at a breakpoint, HLT or a jam.
    /// A breakpoint on the very first instruction is ignored, so "continue" works.
    ///
    /// A processor that is already halted or jammed runs nothing, but the machine around it
    /// does: the clock, the raster, the frame counter and the border go on for the whole
    /// budget, as on a real machine (manual chapter 4). One that halts during the run stops
    /// the run there, so the cycles to the HLT can be counted; the next run carries the
    /// clock on.
    pub fn run_cycles(&mut self, budget: u32) -> Stop {
        if let Some(s) = self.stopped() {
            self.idle(budget);
            return s;
        }
        let target = self.cpu.cycles + budget as u64;
        let mut first = true;
        while self.cpu.cycles < target {
            if let Some(s) = self.stopped() {
                return s;
            }
            if !first && self.breakpoints[self.cpu.pc as usize] {
                return Stop::Breakpoint(self.cpu.pc);
            }
            first = false;
            let lw = self.mem.obs.line_writes;
            self.step();
            if !self.line_breaks.is_empty() && self.mem.obs.line_writes != lw {
                let line = u16::from_le_bytes([self.mem.ram[0x0205], self.mem.ram[0x0206]]);
                if self.line_breaks.contains(&line) {
                    return Stop::Breakpoint(self.cpu.pc);
                }
            }
        }
        self.stopped().unwrap_or(Stop::Budget)
    }

    /// Runs to the end of the current frame (one 50 Hz tick).
    pub fn run_frame(&mut self) -> Stop {
        self.run_cycles(CYCLES_PER_FRAME - self.mem.frame_cycle)
    }

    /// Runs to the start of the next VBLANK line (a whole frame if it's there now), where
    /// FRAME goes up: for tests and tools that step a program paced by FRAME one frame's
    /// work at a time, since that work starts there (chapter 9).
    pub fn run_to_vblank(&mut self) -> Stop {
        let at = VBLANK_LINE * CYCLES_PER_LINE;
        match (at + CYCLES_PER_FRAME - self.mem.frame_cycle) % CYCLES_PER_FRAME {
            0 => self.run_cycles(CYCLES_PER_FRAME),
            n => self.run_cycles(n),
        }
    }

    /// Runs until PC is `pc` (at once if it already is), ignoring breakpoints. Returns the
    /// cycles it took, or why it didn't get there: `Stop::Budget` after `max_cycles`, or the
    /// processor halted or jammed.
    pub fn run_until(&mut self, pc: u16, max_cycles: u64) -> Result<u64, Stop> {
        let t0 = self.cpu.cycles;
        while self.cpu.pc != pc {
            if let Some(s) = self.stopped() {
                return Err(s);
            }
            if self.cpu.cycles - t0 >= max_cycles {
                return Err(Stop::Budget);
            }
            self.step();
        }
        Ok(self.cpu.cycles - t0)
    }

    /// Steps one instruction, but runs a JSR's whole subroutine as one step.
    pub fn step_over(&mut self) -> Stop {
        let pc = self.cpu.pc;
        if self.peek(pc) != 0x05 {
            self.step();
            return self.stopped().unwrap_or(Stop::Budget);
        }
        let ret = pc.wrapping_add(3);
        let limit = self.cpu.cycles + 50 * CYCLES_PER_FRAME as u64;
        self.step();
        while self.cpu.pc != ret && self.cpu.cycles < limit {
            if let Some(s) = self.stopped() {
                return s;
            }
            if self.breakpoints[self.cpu.pc as usize] {
                return Stop::Breakpoint(self.cpu.pc);
            }
            self.step();
        }
        self.stopped().unwrap_or(Stop::Budget)
    }

    pub fn screen(&self) -> &[u8] {
        &self.mem.ram[SCREEN as usize..SCREEN as usize + SCREEN_COLS * SCREEN_ROWS]
    }

    /// Draws the picture from the machine as it stands now, the way the video chip draws each
    /// line: `PIXELS_W` x `PIXELS_H` bytes, one a pixel. Without the video board, 1 for ink and
    /// 0 for paper. Glyphs come from the character set at CHARSET * $100. In the default mode,
    /// codes $80-$FF are glyphs $00-$7F in reverse video, and $A0 (the cursor) shows as paper
    /// while `blink_on` is false, unless VMODE bit 3 (steady) is set. With VMODE bit 0 set, all
    /// 256 codes are separate glyphs from a 2K set.
    ///
    /// With VMODE bit 1 set, the bitmap at BITMAP * $100 replaces the text: line y, pixel x
    /// is bit 7 - (x AND 7) of the byte at base + y * 40 + x / 8. With bit 2 (mixed) also set,
    /// the last TEXTROWS text rows still show under it: at reset rows 21-24, lines 168-199.
    ///
    /// With the video board (chapter 15), every pixel is a colour number 0-3 (`colour_of`
    /// says which colour each is): paper 0, the screen's ink 3, the layer's ink 1, a sprite's
    /// pixels what its image says, and COLOURMODE bit 0 turns the bitmap into 160 x 200 at
    /// 2 bits a pixel. The picture moves right by SCROLLX and down by SCROLLY, the layer
    /// shows where it's 0, with its own scroll, and the sprites draw over it, eight a line.
    ///
    /// This is every line drawn from the same moment: for tests and tools. What the monitor
    /// shows is `frame`, where each line was drawn as the beam finished it.
    pub fn render(&self, blink_on: bool, out: &mut [u8]) {
        assert!(out.len() >= PIXELS_W * PIXELS_H);
        let mut sprites = SpriteLine::new(&self.mem);
        for y in 0..PIXELS_H {
            let row: &mut [u8; PIXELS_W] = (&mut out[y * PIXELS_W..][..PIXELS_W]).try_into().unwrap();
            self.mem.compose_line(y, blink_on, row, &mut sprites);
        }
    }

    /// The picture as the beam drew it (chapter 9): line y as the raster finished line 51 + y,
    /// from the registers and memory as they stood then. After `run_frame`, a whole frame;
    /// stopped part way, the lines above the beam are this frame's and the ones below the
    /// last frame's. Values as `render`'s; `line_colours` says what they show on each line.
    pub fn frame(&self) -> &[u8] {
        &self.picture[..]
    }

    /// The colours (0-15) of values 0-3 on line y of `frame`, as they were when the beam drew
    /// it: BACKGROUND, then without the video board white ink, with it COLOUR1-COLOUR3.
    pub fn line_colours(&self, y: usize) -> [u8; 4] {
        self.picture_colours[y]
    }

    /// All 200 lines' colours, four bytes a line, for hosts that take them in one piece.
    pub fn line_colours_all(&self) -> &[[u8; 4]] {
        &self.picture_colours[..]
    }

    /// The colour (0-15, Appendix A) a host shows for a pixel of `render`'s picture: 0 is
    /// BACKGROUND; without the video board anything else is ink, white (1), and with it, 1-3
    /// are COLOUR1-COLOUR3. As things stand now; `line_colours` has them as the beam saw them.
    pub fn colour_of(&self, n: u8) -> u8 {
        self.mem.colours_now()[(n & 3) as usize]
    }

    /// The beam finishing raster line `line`: if it's one of the picture's lines (51-250),
    /// that line is drawn into `frame`, as things stand now, and the video board's SPRHIT,
    /// SPRHITBG and SPRLINE take what its sprites touched.
    fn beam(&mut self, line: u32) {
        let Some(y) = (line as usize).checked_sub(FIRST_PICTURE_LINE) else { return };
        if y >= PIXELS_H {
            return;
        }
        let mut sprites = SpriteLine::new(&self.mem);
        let mut row = [0u8; PIXELS_W];
        self.mem.compose_line(y, self.blink_on, &mut row, &mut sprites);
        self.picture[y * PIXELS_W..][..PIXELS_W].copy_from_slice(&row);
        self.picture_colours[y] = self.mem.colours_now();
        let r = sprites.report;
        if let Some(b) = &mut self.mem.board {
            b.hit |= r.hit;
            b.hit_bg |= r.hit_bg;
            if r.over {
                b.line |= board::SPRLINE_OVER;
            }
        }
    }

    /// Draws every line of `frame` from the machine as it stands: after a save state is
    /// loaded, so the monitor isn't blank until the beam has been round.
    pub fn redraw(&mut self) {
        let mut px = vec![0u8; PIXELS_W * PIXELS_H];
        self.render(self.blink_on, &mut px);
        self.picture.copy_from_slice(&px);
        let c = self.mem.colours_now();
        self.picture_colours.fill(c);
    }

    /// The screen as text, one line per row, for tests and tools. Codes $20-$5F are their
    /// characters, the cursor ($A0) is `_`, and everything else is `?`.
    pub fn screen_text(&self) -> String {
        self.screen_text_with(|c| match c {
            0x20..=0x5F => c as char,
            0xA0 => '_',
            _ => '?',
        })
    }

    /// As `screen_text`, but reverse video reads as the plain character: $A0-$DF are
    /// $20-$5F, so "GAME OVER" drawn in reverse reads as "GAME OVER". Graphics are `?`.
    pub fn screen_text_plain(&self) -> String {
        self.screen_text_with(|c| match c & 0x7F {
            p @ 0x20..=0x5F => p as char,
            _ => '?',
        })
    }

    fn screen_text_with(&self, show: impl Fn(u8) -> char) -> String {
        self.screen()
            .chunks(SCREEN_COLS)
            .map(|row| {
                row.iter()
                    .map(|&c| show(c))
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
