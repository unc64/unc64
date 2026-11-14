//! The NIECE, the Northern Image Engine, Colour Experimental: a prototype graphics chip, not
//! part of System Rev A. The spec is docs/prg/programmers-reference-guide.md, chapter 17, and
//! Appendix B for the registers (`$C100-$C14F`, `reg` here holds the offsets).
//!
//! It has 64K of memory of its own, the **sheet**: 256 x 512 pixels of 4 bits, a colour
//! (0-15, Appendix A) each. The top of the sheet is two **pages** of 160 x 100, one shown
//! behind the computer's picture and one drawn on; the rest holds whatever the program puts
//! there to draw from. The processor never draws a pixel: it fills in the registers and
//! writes a command to NGO, and the command joins a queue of 64 with the registers as they
//! stood. The chip works through the queue at a fixed rate, four pixels a cycle, so when
//! each command is done is known to the cycle.
//!
//! The chip keeps no clock of its own: `catch_up` brings it to the cycle it's asked about,
//! before every register access and before each line of the picture is drawn.

use std::collections::VecDeque;

use crate::machine::{CYCLES_PER_FRAME, CYCLES_PER_LINE, VBLANK_LINE};
use crate::state::{Reader, Writer};

/// What NIECE (`$C14F`) reads with the chip in.
pub const ID: u8 = 0x17;
/// The registers' first address, and how many addresses the chip answers.
pub const BASE: u16 = 0xC100;
pub const SPAN: u16 = 0x50;
/// The sheet: 256 x 512 pixels, two to a byte, the left one in the high four bits.
pub const SHEET_W: i32 = 256;
pub const SHEET_H: i32 = 512;
pub const ROW_BYTES: usize = 128;
pub const VRAM: usize = 0x10000;
/// A page: 160 x 100. Page 0 is the sheet's lines 0-99, page 1 its lines 100-199.
pub const PAGE_W: i32 = 160;
pub const PAGE_H: i32 = 100;
/// Commands the queue holds, the one being drawn included.
pub const QUEUE: usize = 64;
/// What a command costs: 4 cycles, and a cycle for every 4 pixels (part of 4 counts as 4).
pub const SETUP_CYCLES: u64 = 4;
pub const PIXELS_PER_CYCLE: u64 = 4;

/// The registers, as offsets from `BASE`.
pub mod reg {
    pub const NCTRL: u16 = 0x00;
    pub const NSTATUS: u16 = 0x01;
    pub const NQUEUE: u16 = 0x02;
    pub const NGO: u16 = 0x03;
    pub const NVADDR: u16 = 0x04;
    pub const NVDATA: u16 = 0x06;
    pub const NKEY: u16 = 0x07;
    pub const NDX: u16 = 0x08;
    pub const NDY: u16 = 0x0A;
    pub const NDW: u16 = 0x0C;
    pub const NDH: u16 = 0x0E;
    pub const NSX: u16 = 0x10;
    pub const NSY: u16 = 0x11;
    pub const NSW: u16 = 0x13;
    pub const NSH: u16 = 0x14;
    pub const NCOLOUR: u16 = 0x15;
    pub const NFLAGS: u16 = 0x16;
    pub const NDEPTH: u16 = 0x17;
    pub const NTABLE: u16 = 0x18;
    pub const NINK: u16 = 0x20;
    pub const NMULA: u16 = 0x30;
    pub const NMULB: u16 = 0x32;
    pub const NPROD: u16 = 0x34;
    pub const NDIVN: u16 = 0x38;
    pub const NDIVD: u16 = 0x3C;
    pub const NQUOT: u16 = 0x40;
    pub const NREM: u16 = 0x44;
    pub const NIECE: u16 = 0x4F;
}

/// NCTRL bit 0: the shown page is in the picture, behind everything else.
pub const CTRL_SHOW: u8 = 0x01;
/// NSTATUS: bit 7 while the queue holds anything, bit 6 while it's full, bit 5 once a command
/// was written to a full queue and lost (it stays until NSTATUS is written), bit 0 the page
/// shown.
pub const STATUS_BUSY: u8 = 0x80;
pub const STATUS_FULL: u8 = 0x40;
pub const STATUS_LOST: u8 = 0x20;
pub const STATUS_PAGE: u8 = 0x01;

/// The commands NGO takes. Anything else is ignored.
pub mod cmd {
    pub const FILL: u8 = 1;
    pub const BLIT: u8 = 2;
    pub const LINE: u8 = 3;
    pub const FLIP: u8 = 4;
}

/// NFLAGS.
pub mod flag {
    /// Pixels of the colour in NKEY aren't drawn (BLIT).
    pub const KEY: u8 = 0x01;
    /// The source mirrored left to right, and upside down (BLIT).
    pub const FLIPX: u8 = 0x02;
    pub const FLIPY: u8 = 0x04;
    /// A column is drawn only where NDEPTH is less than the depth line's value there.
    pub const ZTEST: u8 = 0x08;
    /// Each column drawn takes NDEPTH as its value in the depth line.
    pub const ZWRITE: u8 = 0x10;
    /// Draw on the sheet itself, anywhere, instead of on the page not shown.
    pub const SHEET: u8 = 0x20;
    /// Bits 6-7: which of the four ink tables the source's colours go through (BLIT).
    pub const TABLE_SHIFT: u8 = 6;
}

/// A command in the queue: the registers as they stood when NGO was written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    pub op: u8,
    pub dx: i16,
    pub dy: i16,
    pub dw: u16,
    pub dh: u16,
    pub sx: u8,
    pub sy: u16,
    pub sw: u8,
    pub sh: u8,
    pub colour: u8,
    pub flags: u8,
    pub depth: u8,
    pub key: u8,
    pub ink: [u8; 16],
    /// The cycle NGO was written at.
    pub at: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Niece {
    pub ctrl: u8,
    /// A command was written to a full queue.
    pub lost: bool,
    /// The page shown, 0 or 1. Commands draw on the other.
    pub page: u8,
    pub vaddr: u16,
    pub key: u8,
    pub dx: i16,
    pub dy: i16,
    pub dw: u16,
    pub dh: u16,
    pub sx: u8,
    pub sy: u16,
    pub sw: u8,
    pub sh: u8,
    pub colour: u8,
    pub flags: u8,
    pub depth_reg: u8,
    pub table: u8,
    pub inks: [[u8; 16]; 4],
    pub mula: i16,
    pub mulb: i16,
    pub divn: u32,
    pub divd: u16,
    pub vram: Box<[u8; VRAM]>,
    /// The depth line: a value for each column, 0-255.
    pub depth: [u8; SHEET_W as usize],
    pub queue: VecDeque<Command>,
    /// The cycle the chip finished its last command at: the next can't start before.
    pub free_at: u64,
}

impl Default for Niece {
    fn default() -> Self {
        let identity: [u8; 16] = std::array::from_fn(|i| i as u8);
        Niece {
            ctrl: 0,
            lost: false,
            page: 0,
            vaddr: 0,
            key: 0,
            dx: 0,
            dy: 0,
            dw: 0,
            dh: 0,
            sx: 0,
            sy: 0,
            sw: 0,
            sh: 0,
            colour: 0,
            flags: 0,
            depth_reg: 0,
            table: 0,
            inks: [identity; 4],
            mula: 0,
            mulb: 0,
            divn: 0,
            divd: 0,
            vram: Box::new([0; VRAM]),
            depth: [0xFF; SHEET_W as usize],
            queue: VecDeque::new(),
            free_at: 0,
        }
    }
}

/// Where a command draws: the sheet's rectangle it may touch.
struct Target {
    top: i32,
    w: i32,
    h: i32,
}

impl Command {
    fn target(&self, shown: u8) -> Target {
        if self.flags & flag::SHEET != 0 {
            Target { top: 0, w: SHEET_W, h: SHEET_H }
        } else {
            Target { top: (shown ^ 1) as i32 * PAGE_H, w: PAGE_W, h: PAGE_H }
        }
    }

    /// The part of the destination rectangle on the target: x0, x1, y0, y1 (the ends not
    /// included), in the target's own coordinates.
    fn clip(&self, t: &Target) -> (i32, i32, i32, i32) {
        let (x, y) = (self.dx as i32, self.dy as i32);
        let x0 = x.max(0);
        let x1 = (x + self.dw as i32).min(t.w);
        let y0 = y.max(0);
        let y1 = (y + self.dh as i32).min(t.h);
        (x0, x1.max(x0), y0, y1.max(y0))
    }

    /// LINE's two ends: NDW and NDH are how far it goes, as signed numbers.
    fn line(&self) -> (i32, i32, i32) {
        let (ex, ey) = (self.dw as i16 as i32, self.dh as i16 as i32);
        (ex, ey, ex.abs().max(ey.abs()))
    }

    /// Cycles the command takes once the chip starts on it (FLIP: none, it waits instead).
    /// Both pages are the same size, so this doesn't depend on which one is shown.
    pub fn cycles(&self) -> u64 {
        let pixels = match self.op {
            cmd::FILL | cmd::BLIT => {
                let (x0, x1, y0, y1) = self.clip(&self.target(0));
                (x1 - x0) as u64 * (y1 - y0) as u64
            }
            cmd::LINE => self.line().2 as u64 + 1,
            _ => return 0,
        };
        SETUP_CYCLES + pixels.div_ceil(PIXELS_PER_CYCLE)
    }
}

impl Niece {
    /// The colour of the sheet's pixel (x, y); both wrap round the sheet.
    pub fn pixel(&self, x: i32, y: i32) -> u8 {
        let (x, y) = (x & (SHEET_W - 1), y & (SHEET_H - 1));
        let b = self.vram[y as usize * ROW_BYTES + x as usize / 2];
        if x & 1 == 0 { b >> 4 } else { b & 15 }
    }

    fn plot(&mut self, x: i32, y: i32, c: u8) {
        let b = &mut self.vram[y as usize * ROW_BYTES + x as usize / 2];
        *b = if x & 1 == 0 { (*b & 0x0F) | c << 4 } else { (*b & 0xF0) | (c & 15) };
    }

    /// Line y (0-99) of the page shown, a colour (0-15) for each of its 160 pixels.
    pub fn shown_line(&self, y: usize, out: &mut [u8; PAGE_W as usize]) {
        let row = &self.vram[(self.page as usize * PAGE_H as usize + y) * ROW_BYTES..][..PAGE_W as usize / 2];
        for (i, &b) in row.iter().enumerate() {
            out[i * 2] = b >> 4;
            out[i * 2 + 1] = b & 15;
        }
    }

    pub fn shown(&self) -> bool {
        self.ctrl & CTRL_SHOW != 0
    }

    pub fn status(&self) -> u8 {
        let bit = |on: bool, b: u8| if on { b } else { 0 };
        bit(!self.queue.is_empty(), STATUS_BUSY) | bit(self.queue.len() >= QUEUE, STATUS_FULL) | bit(self.lost, STATUS_LOST) | self.page
    }

    fn product(&self) -> u32 {
        (self.mula as i32 * self.mulb as i32) as u32
    }

    /// NQUOT and NREM. Dividing by 0 gives every bit set in both.
    fn quotient(&self) -> (u32, u16) {
        match self.divd {
            0 => (u32::MAX, u16::MAX),
            d => (self.divn / d as u32, (self.divn % d as u32) as u16),
        }
    }

    /// What the processor reads at register `r` (an offset from `BASE`), without side effects.
    pub fn peek(&self, r: u16) -> u8 {
        let word = |v: u16, at: u16| v.to_le_bytes()[(r - at) as usize];
        let long = |v: u32, at: u16| v.to_le_bytes()[(r - at) as usize];
        match r {
            reg::NCTRL => self.ctrl,
            reg::NSTATUS => self.status(),
            reg::NQUEUE => self.queue.len() as u8,
            0x04..=0x05 => word(self.vaddr, reg::NVADDR),
            reg::NVDATA => self.vram[self.vaddr as usize],
            reg::NKEY => self.key,
            0x08..=0x09 => word(self.dx as u16, reg::NDX),
            0x0A..=0x0B => word(self.dy as u16, reg::NDY),
            0x0C..=0x0D => word(self.dw, reg::NDW),
            0x0E..=0x0F => word(self.dh, reg::NDH),
            reg::NSX => self.sx,
            0x11..=0x12 => word(self.sy, reg::NSY),
            reg::NSW => self.sw,
            reg::NSH => self.sh,
            reg::NCOLOUR => self.colour,
            reg::NFLAGS => self.flags,
            reg::NDEPTH => self.depth_reg,
            reg::NTABLE => self.table,
            0x20..=0x2F => self.inks[self.table as usize][(r - reg::NINK) as usize],
            0x30..=0x31 => word(self.mula as u16, reg::NMULA),
            0x32..=0x33 => word(self.mulb as u16, reg::NMULB),
            0x34..=0x37 => long(self.product(), reg::NPROD),
            0x38..=0x3B => long(self.divn, reg::NDIVN),
            0x3C..=0x3D => word(self.divd, reg::NDIVD),
            0x40..=0x43 => long(self.quotient().0, reg::NQUOT),
            0x44..=0x45 => word(self.quotient().1, reg::NREM),
            reg::NIECE => ID,
            _ => 0xFF,
        }
    }

    /// The side effect of the processor reading register `r`: NVDATA moves NVADDR on.
    pub fn take(&mut self, r: u16) {
        if r == reg::NVDATA {
            self.vaddr = self.vaddr.wrapping_add(1);
        }
    }

    /// The processor writes register `r` at cycle `now`. The chip is caught up first
    /// (`catch_up`), so NGO sees the queue as it is at that cycle.
    pub fn store(&mut self, r: u16, value: u8, now: u64) {
        fn set(v: &mut u16, hi: bool, b: u8) {
            *v = if hi { (*v & 0x00FF) | (b as u16) << 8 } else { (*v & 0xFF00) | b as u16 };
        }
        fn set_i(v: &mut i16, hi: bool, b: u8) {
            let mut u = *v as u16;
            set(&mut u, hi, b);
            *v = u as i16;
        }
        let hi = r & 1 != 0;
        match r {
            reg::NCTRL => self.ctrl = value & CTRL_SHOW,
            // any write forgets a lost command; a wait loop's reads don't
            reg::NSTATUS => self.lost = false,
            reg::NGO => self.go(value, now),
            0x04..=0x05 => set(&mut self.vaddr, hi, value),
            reg::NVDATA => {
                self.vram[self.vaddr as usize] = value;
                self.vaddr = self.vaddr.wrapping_add(1);
            }
            reg::NKEY => self.key = value & 15,
            0x08..=0x09 => set_i(&mut self.dx, hi, value),
            0x0A..=0x0B => set_i(&mut self.dy, hi, value),
            0x0C..=0x0D => set(&mut self.dw, hi, value),
            0x0E..=0x0F => set(&mut self.dh, hi, value),
            reg::NSX => self.sx = value,
            0x11 => set(&mut self.sy, false, value),
            0x12 => set(&mut self.sy, true, value & 1),
            reg::NSW => self.sw = value,
            reg::NSH => self.sh = value,
            reg::NCOLOUR => self.colour = value & 15,
            reg::NFLAGS => self.flags = value,
            reg::NDEPTH => self.depth_reg = value,
            reg::NTABLE => self.table = value & 3,
            0x20..=0x2F => self.inks[self.table as usize][(r - reg::NINK) as usize] = value & 15,
            0x30..=0x31 => set_i(&mut self.mula, hi, value),
            0x32..=0x33 => set_i(&mut self.mulb, hi, value),
            0x38..=0x3B => {
                let mut b = self.divn.to_le_bytes();
                b[(r - reg::NDIVN) as usize] = value;
                self.divn = u32::from_le_bytes(b);
            }
            0x3C..=0x3D => set(&mut self.divd, hi, value),
            _ => {}
        }
    }

    /// NGO: the command joins the queue with the registers as they stand. A full queue loses it.
    fn go(&mut self, op: u8, now: u64) {
        if !(cmd::FILL..=cmd::FLIP).contains(&op) {
            return;
        }
        if self.queue.len() >= QUEUE {
            self.lost = true;
            return;
        }
        self.queue.push_back(Command {
            op,
            dx: self.dx,
            dy: self.dy,
            dw: self.dw,
            dh: self.dh,
            sx: self.sx,
            sy: self.sy,
            sw: self.sw,
            sh: self.sh,
            colour: self.colour,
            flags: self.flags,
            depth: self.depth_reg,
            key: self.key,
            ink: self.inks[(self.flags >> flag::TABLE_SHIFT) as usize],
            at: now,
        });
    }

    /// Brings the chip to cycle `t`: every command that's finished by then has been drawn.
    /// `now` and `frame_cycle` are the machine's clock and its place in the frame (they say
    /// when the beam leaves the picture, which is what FLIP waits for). A command starts when
    /// it's written or when the one before it finishes, whichever is later, and its pixels
    /// are all there at the cycle it finishes: not before.
    pub fn catch_up(&mut self, t: u64, now: u64, frame_cycle: u32) {
        while let Some(head) = self.queue.front() {
            let start = head.at.max(self.free_at);
            let finish = if head.op == cmd::FLIP {
                // the cycle the beam next leaves the picture, at or after `start`
                let frame = CYCLES_PER_FRAME as i64;
                let pos = (frame_cycle as i64 - (now as i64 - start as i64)).rem_euclid(frame);
                start + ((VBLANK_LINE * CYCLES_PER_LINE) as i64 - pos).rem_euclid(frame) as u64
            } else {
                start + head.cycles()
            };
            if finish > t {
                break;
            }
            let c = self.queue.pop_front().unwrap();
            self.free_at = finish;
            self.draw(&c);
        }
    }

    /// Whether column x passes the command's depth test; if it does and the command writes
    /// depth, the column takes the command's.
    fn column(&mut self, c: &Command, x: i32) -> bool {
        let d = &mut self.depth[x as usize];
        if c.flags & flag::ZTEST != 0 && c.depth >= *d {
            return false;
        }
        if c.flags & flag::ZWRITE != 0 {
            *d = c.depth;
        }
        true
    }

    fn draw(&mut self, c: &Command) {
        let t = c.target(self.page);
        match c.op {
            cmd::FLIP => self.page ^= 1,
            cmd::FILL => {
                let (x0, x1, y0, y1) = c.clip(&t);
                if y1 == y0 {
                    return;
                }
                for x in x0..x1 {
                    if self.column(c, x) {
                        for y in y0..y1 {
                            self.plot(x, t.top + y, c.colour);
                        }
                    }
                }
            }
            cmd::BLIT => {
                let (x0, x1, y0, y1) = c.clip(&t);
                if y1 == y0 || c.sw == 0 || c.sh == 0 {
                    return;
                }
                let (dx, dy) = (c.dx as i64, c.dy as i64);
                let (dw, dh, sw, sh) = (c.dw as i64, c.dh as i64, c.sw as i64, c.sh as i64);
                let keyed = c.flags & flag::KEY != 0;
                // the source row for each line of the destination
                let rows: Vec<i32> = (y0..y1)
                    .map(|y| {
                        let v = (y as i64 - dy) * sh / dh;
                        c.sy as i32 + (if c.flags & flag::FLIPY != 0 { sh - 1 - v } else { v }) as i32
                    })
                    .collect();
                for x in x0..x1 {
                    if !self.column(c, x) {
                        continue;
                    }
                    let u = (x as i64 - dx) * sw / dw;
                    let u = c.sx as i32 + (if c.flags & flag::FLIPX != 0 { sw - 1 - u } else { u }) as i32;
                    for (y, &v) in (y0..y1).zip(&rows) {
                        let p = self.pixel(u, v);
                        if !keyed || p != c.key {
                            self.plot(x, t.top + y, c.ink[p as usize]);
                        }
                    }
                }
            }
            cmd::LINE => {
                let (ex, ey, n) = c.line();
                // a pixel for each step along the longer side, at the nearest place on the other
                let at = |k: i32, e: i32| if n == 0 { 0 } else { (2 * k as i64 * e as i64 + n as i64).div_euclid(2 * n as i64) as i32 };
                for k in 0..=n {
                    let (x, y) = (c.dx as i32 + at(k, ex), c.dy as i32 + at(k, ey));
                    if (0..t.w).contains(&x) && (0..t.h).contains(&y) && self.column(c, x) {
                        self.plot(x, t.top + y, c.colour);
                    }
                }
            }
            _ => {}
        }
    }

    /// `idle` is the part of the machine's clock a save state doesn't keep (`Machine::idle_cycles`):
    /// the cycles in the queue are saved without it, as the processor's own count is.
    pub(crate) fn save(&self, w: &mut Writer, idle: u64) {
        for v in [self.ctrl, self.lost as u8, self.page, self.key, self.sx, self.sw, self.sh, self.colour, self.flags, self.depth_reg, self.table] {
            w.u8(v);
        }
        for v in [self.vaddr, self.dx as u16, self.dy as u16, self.dw, self.dh, self.sy, self.mula as u16, self.mulb as u16, self.divd] {
            w.u16(v);
        }
        w.u32(self.divn);
        for t in &self.inks {
            w.bytes(t);
        }
        w.bytes(&self.depth);
        w.u64(self.free_at.saturating_sub(idle));
        w.u8(self.queue.len() as u8);
        for c in &self.queue {
            for v in [c.op, c.sx, c.sw, c.sh, c.colour, c.flags, c.depth, c.key] {
                w.u8(v);
            }
            for v in [c.dx as u16, c.dy as u16, c.dw, c.dh, c.sy] {
                w.u16(v);
            }
            w.bytes(&c.ink);
            w.u64(c.at.saturating_sub(idle));
        }
        w.bytes(&self.vram[..]);
    }

    pub(crate) fn restore(r: &mut Reader) -> Result<Self, String> {
        let mut n = Niece::default();
        let b = r.bytes(11)?;
        n.ctrl = b[0] & CTRL_SHOW;
        n.lost = b[1] & 1 != 0;
        n.page = b[2] & 1;
        n.key = b[3] & 15;
        (n.sx, n.sw, n.sh, n.colour, n.flags, n.depth_reg, n.table) = (b[4], b[5], b[6], b[7] & 15, b[8], b[9], b[10] & 3);
        n.vaddr = r.u16()?;
        n.dx = r.u16()? as i16;
        n.dy = r.u16()? as i16;
        n.dw = r.u16()?;
        n.dh = r.u16()?;
        n.sy = r.u16()? & 0x1FF;
        n.mula = r.u16()? as i16;
        n.mulb = r.u16()? as i16;
        n.divd = r.u16()?;
        n.divn = r.u32()?;
        for t in &mut n.inks {
            t.copy_from_slice(r.bytes(16)?);
            t.iter_mut().for_each(|c| *c &= 15);
        }
        n.depth.copy_from_slice(r.bytes(SHEET_W as usize)?);
        n.free_at = r.u64()?;
        let queued = r.u8()? as usize;
        if queued > QUEUE {
            return Err("save state: the NIECE's queue is too long".into());
        }
        for _ in 0..queued {
            let b = r.bytes(8)?;
            if !(cmd::FILL..=cmd::FLIP).contains(&b[0]) {
                return Err(format!("save state: NIECE command {}", b[0]));
            }
            let (dx, dy, dw, dh, sy) = (r.u16()? as i16, r.u16()? as i16, r.u16()?, r.u16()?, r.u16()? & 0x1FF);
            let mut ink: [u8; 16] = r.bytes(16)?.try_into().unwrap();
            ink.iter_mut().for_each(|c| *c &= 15);
            n.queue.push_back(Command { op: b[0], sx: b[1], sw: b[2], sh: b[3], colour: b[4] & 15, flags: b[5], depth: b[6], key: b[7] & 15, dx, dy, dw, dh, sy, ink, at: r.u64()? });
        }
        n.vram.copy_from_slice(r.bytes(VRAM)?);
        Ok(n)
    }
}
