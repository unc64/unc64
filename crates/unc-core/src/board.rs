//! The video board, Rev A: the upgrade card with 32 hardware sprites in a table in
//! memory (eight a line), smooth scrolling, a background layer and four colours. The spec is
//! docs/prg/programmers-reference-guide.md, chapter 15, and Appendix B for the registers
//! (`$C0A0-$C0BF`, in `machine::io`).
//!
//! With the board, the picture is made of colour numbers 0-3: 0 is BACKGROUND, 1-3 are
//! COLOUR1-COLOUR3. The board draws each line of the picture as the beam finishes it, like
//! the rest of the picture (`Machine::frame`), from its registers, its table and memory as
//! they stand then. Its collision and line registers take what each line's sprites touched,
//! as the line is drawn, whether or not a host shows it.

use crate::machine::{PIXELS_H, PIXELS_W, io};
use crate::state::{Reader, Writer};

/// What BOARD (`$C0BF`) reads with the board in: the card's id.
pub const BOARD_ID: u8 = 0x15;
/// Entries in the sprite table, and how many of them the board draws on one line.
pub const SPRITES: usize = 32;
pub const PER_LINE: usize = 8;
/// Bytes per table entry: Y, X bits 0-7, image, attributes.
pub const ENTRY_BYTES: u16 = 4;
/// A sprite is 16 x 16 pixels at 2 bits a pixel: 16 rows of 4 bytes, 64 bytes an image.
pub const SPRITE_SIZE: i32 = 16;
pub const SPRITE_BYTES: u16 = 64;
/// SPRTAB at reset: the table at `$3700`.
pub const SPRTAB_AT_RESET: u8 = 0x37;
/// SPRBANK at reset: the images from `$3800`, inside a UNC 16's 16K.
pub const SPRBANK_AT_RESET: u8 = 0x38;
/// LAYERPAGE at reset: the layer's 1,000 codes at `$3C00`.
pub const LAYERPAGE_AT_RESET: u8 = 0x3C;
/// COLOUR1-COLOUR3 at reset: dark grey, light grey, white. Text ink is colour 3, so a
/// picture looks as it does without the board.
pub const COLOURS_AT_RESET: [u8; 3] = [11, 15, 1];

/// The attribute byte, byte 3 of a table entry. Bits 5 and 6 are spare: kept, ignored.
pub mod attr {
    /// Bit 8 of the sprite's x.
    pub const X8: u8 = 0x01;
    /// 32 x 32: each pixel of the image drawn 2 x 2.
    pub const WIDE: u8 = 0x02;
    /// Behind the picture: the sprite shows only where the picture is colour 0.
    pub const BEHIND: u8 = 0x04;
    /// Mirrored left to right.
    pub const FLIPX: u8 = 0x08;
    /// Upside down.
    pub const FLIPY: u8 = 0x10;
    /// Shown.
    pub const SHOWN: u8 = 0x80;
}

/// SPRLINE bit 7: some line had more than eight sprites.
pub const SPRLINE_OVER: u8 = 0x80;
/// COLOURMODE bit 0: the 4-colour bitmap, 160 x 200.
pub const COLOURMODE_BITMAP4: u8 = 0x01;

/// The board's registers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VideoBoard {
    /// SPRTAB: high byte of the sprite table's address.
    pub table: u8,
    /// SPRBANK: high byte of the image bank. Image p is at SPRBANK * 256 + p * 64.
    pub bank: u8,
    /// SPRHIT: table sprites 0-7 whose ink met another sprite's ink, since the last read.
    pub hit: u8,
    /// SPRHITBG: table sprites 0-7 whose ink met the picture's (the screen's or the layer's),
    /// since the last read.
    pub hit_bg: u8,
    /// SPRLINE: bit 7 if a line had more than eight sprites since the last read.
    pub line: u8,
    /// SCROLLX, SCROLLY: the main picture's fine scroll, 0-7 pixels right and down.
    pub scroll_x: u8,
    pub scroll_y: u8,
    /// LAYER: bit 0 shows the background layer.
    pub layer: u8,
    /// LAYERPAGE: high byte of the layer's 40 x 25 screen.
    pub layer_page: u8,
    /// LAYERSX, LAYERSY: the layer's own fine scroll, 0-7.
    pub layer_sx: u8,
    pub layer_sy: u8,
    /// COLOURMODE: bit 0, the 4-colour bitmap.
    pub colour_mode: u8,
    /// COLOUR1-COLOUR3: the colour (0-15) of colour numbers 1-3.
    pub colours: [u8; 3],
}

impl Default for VideoBoard {
    fn default() -> Self {
        VideoBoard {
            table: SPRTAB_AT_RESET,
            bank: SPRBANK_AT_RESET,
            hit: 0,
            hit_bg: 0,
            line: 0,
            scroll_x: 0,
            scroll_y: 0,
            layer: 0,
            layer_page: LAYERPAGE_AT_RESET,
            layer_sx: 0,
            layer_sy: 0,
            colour_mode: 0,
            colours: COLOURS_AT_RESET,
        }
    }
}

/// One entry of the sprite table, as the board reads it from memory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sprite {
    pub y: u8,
    pub x: u8,
    pub image: u8,
    pub attr: u8,
}

impl Sprite {
    pub fn shown(&self) -> bool {
        self.attr & attr::SHOWN != 0
    }

    /// The pixel size of the image: 1, or 2 for a wide sprite.
    pub fn scale(&self) -> i32 {
        if self.attr & attr::WIDE != 0 { 2 } else { 1 }
    }

    /// x on the picture: 0-319 on it, 320-511 are -192 to -1, off to the left.
    pub fn left(&self) -> i32 {
        let x = self.x as i32 | ((self.attr & attr::X8) as i32) << 8;
        if x >= PIXELS_W as i32 { x - 512 } else { x }
    }

    /// The top line: 0-199 on the picture, 200-255 are -56 to -1, above it.
    pub fn top(&self) -> i32 {
        let y = self.y as i32;
        if y >= PIXELS_H as i32 { y - 256 } else { y }
    }

    /// Whether its rows cover picture line `y`.
    pub fn covers(&self, y: i32) -> bool {
        let top = self.top();
        y >= top && y < top + SPRITE_SIZE * self.scale()
    }
}

/// What drawing the sprites found: the collisions of table sprites 0-7, and whether a line
/// had more than eight sprites.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpriteReport {
    pub hit: u8,
    pub hit_bg: u8,
    pub over: bool,
}

impl VideoBoard {
    /// What the processor reads at `addr` (`$C080-$C0BF`), without side effects.
    pub fn peek(&self, addr: u16) -> u8 {
        match addr {
            io::SPRTAB => self.table,
            io::SPRBANK => self.bank,
            io::SPRHIT => self.hit,
            io::SPRHITBG => self.hit_bg,
            io::SPRLINE => self.line,
            io::SCROLLX => self.scroll_x,
            io::SCROLLY => self.scroll_y,
            io::LAYER => self.layer,
            io::LAYERPAGE => self.layer_page,
            io::LAYERSX => self.layer_sx,
            io::LAYERSY => self.layer_sy,
            io::COLOURMODE => self.colour_mode,
            io::COLOUR1..=io::COLOUR3 => self.colours[(addr - io::COLOUR1) as usize],
            io::BOARD => BOARD_ID,
            _ => 0xFF,
        }
    }

    /// The side effect of the processor reading `addr`: the collision and line registers clear.
    pub fn take(&mut self, addr: u16) {
        match addr {
            io::SPRHIT => self.hit = 0,
            io::SPRHITBG => self.hit_bg = 0,
            io::SPRLINE => self.line = 0,
            _ => {}
        }
    }

    pub fn store(&mut self, addr: u16, value: u8) {
        match addr {
            io::SPRTAB => self.table = value,
            io::SPRBANK => self.bank = value,
            io::SCROLLX => self.scroll_x = value & 7,
            io::SCROLLY => self.scroll_y = value & 7,
            io::LAYER => self.layer = value & 1,
            io::LAYERPAGE => self.layer_page = value,
            io::LAYERSX => self.layer_sx = value & 7,
            io::LAYERSY => self.layer_sy = value & 7,
            io::COLOURMODE => self.colour_mode = value & COLOURMODE_BITMAP4,
            io::COLOUR1..=io::COLOUR3 => self.colours[(addr - io::COLOUR1) as usize] = value & 0x0F,
            // SPRHIT, SPRHITBG, SPRLINE and BOARD are read-only; the gaps ignore writes.
            _ => {}
        }
    }

    /// The colour (0-15) of colour number `n`, 1-3.
    pub fn colour(&self, n: u8) -> u8 {
        self.colours[((n & 3) as usize).max(1) - 1]
    }

    /// Entry n of the sprite table, at SPRTAB * 256 + n * 4. With SPRTAB 0 there is no
    /// table: the board shows no sprites, and every entry reads as not shown.
    pub fn sprite(&self, read: &impl Fn(u16) -> u8, n: usize) -> Sprite {
        let at = ((self.table as u16) << 8).wrapping_add(n as u16 * ENTRY_BYTES);
        let b = |i: u16| read(at.wrapping_add(i));
        Sprite { y: b(0), x: b(1), image: b(2), attr: if self.table == 0 { 0 } else { b(3) } }
    }

    /// Where image p starts: SPRBANK * 256 + p * 64.
    pub fn image(&self, p: u8) -> u16 {
        ((self.bank as u16) << 8).wrapping_add(p as u16 * SPRITE_BYTES)
    }

    /// Draws the sprites over `px`, a 320 x 200 picture of colour numbers, if `draw`, and
    /// reports what they touched: every line as `sprite_line` draws it, from the table as it
    /// stands now.
    pub fn sprites(&self, read: &impl Fn(u16) -> u8, px: &mut [u8], draw: bool) -> SpriteReport {
        let shown: Vec<(usize, Sprite)> =
            (0..SPRITES).map(|n| (n, self.sprite(read, n))).filter(|(_, s)| s.shown()).collect();
        let mut report = SpriteReport::default();
        if shown.is_empty() {
            return report;
        }
        for y in 0..PIXELS_H {
            let row: &mut [u8; PIXELS_W] = (&mut px[y * PIXELS_W..][..PIXELS_W]).try_into().unwrap();
            let r = self.sprite_line(read, &shown, y as i32, row, draw);
            report.hit |= r.hit;
            report.hit_bg |= r.hit_bg;
            report.over |= r.over;
        }
        report
    }

    /// The sprites on picture line `y`, over `row` (the line's colour numbers), if `draw`, and
    /// what they touched. `shown` is the table's shown entries, in table order. The first
    /// eight whose rows cover the line are drawn, the lowest index on top; a sprite's pixel 0
    /// is see-through, and a sprite behind shows only where the picture is 0. A sprite off the
    /// picture's left or right edge still takes its place on the line.
    pub fn sprite_line(&self, read: &impl Fn(u16) -> u8, shown: &[(usize, Sprite)], y: i32, row: &mut [u8; PIXELS_W], draw: bool) -> SpriteReport {
        let mut report = SpriteReport::default();
        let mut line: Vec<(usize, Sprite)> = Vec::with_capacity(PER_LINE + 1);
        for &(n, s) in shown {
            if s.covers(y) {
                if line.len() == PER_LINE {
                    report.over = true;
                    break;
                }
                line.push((n, s));
            }
        }
        if line.is_empty() {
            return report;
        }
        let mut colour = [0u8; PIXELS_W];
        let mut ink = [0u32; PIXELS_W];
        // the lowest priority first, so the lowest index ends up on top
        for &(n, s) in line.iter().rev() {
            let scale = s.scale();
            let mut r = (y - s.top()) / scale;
            if s.attr & attr::FLIPY != 0 {
                r = SPRITE_SIZE - 1 - r;
            }
            let at = self.image(s.image).wrapping_add(r as u16 * 4);
            let bits = (0..4).fold(0u32, |b, k| b << 8 | read(at.wrapping_add(k)) as u32);
            let behind = s.attr & attr::BEHIND != 0;
            for c in 0..SPRITE_SIZE {
                let p = if s.attr & attr::FLIPX != 0 { SPRITE_SIZE - 1 - c } else { c };
                let v = (bits >> (30 - 2 * p)) as u8 & 3;
                if v == 0 {
                    continue;
                }
                for dx in 0..scale {
                    let x = s.left() + c * scale + dx;
                    if !(0..PIXELS_W as i32).contains(&x) {
                        continue;
                    }
                    let x = x as usize;
                    ink[x] |= 1 << n;
                    if !behind || row[x] == 0 {
                        colour[x] = v;
                    }
                }
            }
        }
        for x in 0..PIXELS_W {
            let first8 = ink[x] as u8;
            if ink[x].count_ones() > 1 {
                report.hit |= first8;
            }
            if row[x] != 0 {
                report.hit_bg |= first8;
            }
            if draw && colour[x] != 0 {
                row[x] = colour[x];
            }
        }
        report
    }

    pub(crate) fn save(&self, w: &mut Writer) {
        for v in [self.table, self.bank, self.hit, self.hit_bg, self.line, self.scroll_x, self.scroll_y] {
            w.u8(v);
        }
        for v in [self.layer, self.layer_page, self.layer_sx, self.layer_sy, self.colour_mode] {
            w.u8(v);
        }
        w.bytes(&self.colours);
    }

    /// The board as version 3 of the save state has it: 15 bytes (`save`).
    pub(crate) fn restore(r: &mut Reader) -> Result<Self, String> {
        let b = r.bytes(15)?;
        Ok(VideoBoard {
            table: b[0],
            bank: b[1],
            hit: b[2],
            hit_bg: b[3],
            line: b[4] & SPRLINE_OVER,
            scroll_x: b[5] & 7,
            scroll_y: b[6] & 7,
            layer: b[7] & 1,
            layer_page: b[8],
            layer_sx: b[9] & 7,
            layer_sy: b[10] & 7,
            colour_mode: b[11] & COLOURMODE_BITMAP4,
            colours: [b[12] & 15, b[13] & 15, b[14] & 15],
        })
    }

    /// The board as version 2 had it: 37 bytes, eight sprites in registers. That layout is
    /// gone, so its sprites (and their collisions) are dropped. The bank, the scroll and the
    /// layer carry over; the table and the colours start as at reset.
    pub(crate) fn restore_v2(r: &mut Reader) -> Result<Self, String> {
        // x[8], xhi, y[8], ptr[8], enable, bank, wide, behind, hit, hit_bg, then the rest
        let b = r.bytes(37)?;
        let rest = &b[31..];
        Ok(VideoBoard {
            bank: b[26],
            scroll_x: rest[0] & 7,
            scroll_y: rest[1] & 7,
            layer: rest[2] & 1,
            layer_page: rest[3],
            layer_sx: rest[4] & 7,
            layer_sy: rest[5] & 7,
            ..VideoBoard::default()
        })
    }
}

/// Moves a 320 x 200 picture right by `dx` and down by `dy` pixels, in place: the pixels
/// moved off the edge are cut, and the strip uncovered at the left and top is colour 0.
pub fn scroll(px: &mut [u8], dx: usize, dy: usize) {
    if dx == 0 && dy == 0 {
        return;
    }
    for y in (0..PIXELS_H).rev() {
        let row = y * PIXELS_W;
        if y < dy {
            px[row..row + PIXELS_W].fill(0);
            continue;
        }
        let from = (y - dy) * PIXELS_W;
        px.copy_within(from..from + PIXELS_W - dx, row + dx);
        px[row..row + dx].fill(0);
    }
}
