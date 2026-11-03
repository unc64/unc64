//! The shift register for bitmap graphics, like the dedicated shifter on 1970s arcade
//! boards. Rev A. The spec is docs/prg/programmers-reference-guide.md, chapter 8.
//!
//! A 16-bit window of pixels: each byte pushed in becomes the low (right-hand) byte, and
//! the byte before it moves up. SHIFT_OUT reads eight pixels of the window, starting
//! SHIFT_AMOUNT pixels left of the newest byte: a sprite's byte, moved right by x AND 7,
//! for a bitmap where bit 7 is the leftmost pixel.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Shifter {
    /// The last two bytes pushed: the newest in bits 0-7.
    pub window: u16,
    /// SHIFT_AMOUNT, 0-7.
    pub amount: u8,
}

impl Shifter {
    /// SHIFT_IN: pushes a byte into the window.
    pub fn push(&mut self, byte: u8) {
        self.window = self.window << 8 | byte as u16;
    }

    /// SHIFT_FLIP: pushes a byte with its bits in reverse order, so a shape is drawn mirrored
    /// (push a wide shape's bytes from right to left).
    pub fn push_flipped(&mut self, byte: u8) {
        self.push(byte.reverse_bits());
    }

    /// SHIFT_OUT: the window moved right by `amount`, low byte.
    pub fn out(&self) -> u8 {
        (self.window >> self.amount) as u8
    }
}
