//! The AUNT, the UNC 64's sound chip: three voices, a wave table, one filter. The spec is
//! manual ch. 12.
//!
//! A card, like the video board: registers at `$C0C0-$C0FF`, SOUND (`$C0DF`) reads `$16`
//! when it is in. Mono. Everything is integer arithmetic stepped a cycle at a time, so the
//! same writes at the same cycles make the same samples on every host.
//!
//! `Chip` stands alone: `store` and `peek` take a register number 0-63 (the address less
//! `$C0C0`), `clock` runs it, and `samples` fills with one 16-bit sample every 16 cycles
//! (62,500 a second). The machine's side of it (catching up when a register is touched,
//! mixing it with the beeper) is `Card`, at the end.

use crate::state::{Reader, Writer};

/// SOUND reads this with the chip in.
pub const ID: u8 = 0x16;
/// Cycles to a sample, and so samples a second.
pub const CYCLES_PER_SAMPLE: u8 = 16;
pub const SAMPLE_RATE: u32 = 1_000_000 / CYCLES_PER_SAMPLE as u32;

// A voice's registers, by offset.
pub const FREQ_LO: u8 = 0;
pub const FREQ_HI: u8 = 1;
pub const WIDTH: u8 = 2;
pub const WAVE: u8 = 3;
pub const CONTROL: u8 = 4;
pub const ATTACK_DECAY: u8 = 5;
pub const SUSTAIN_RELEASE: u8 = 6;
pub const TRIM: u8 = 7;
// The chip's own, by register number.
pub const CUTOFF: u8 = 0x18;
pub const FILTER: u8 = 0x19;
pub const VOLUME: u8 = 0x1A;
pub const VOICE3: u8 = 0x1C;
pub const ENV3: u8 = 0x1D;
pub const SOUND: u8 = 0x1F;
pub const WAVETABLE: u8 = 0x20;

pub const WAVE_TRIANGLE: u8 = 0x01;
pub const WAVE_SAW: u8 = 0x02;
pub const WAVE_PULSE: u8 = 0x04;
pub const WAVE_NOISE: u8 = 0x08;
pub const WAVE_TABLE: u8 = 0x10;
pub const WAVE_SHORT: u8 = 0x80;
const WAVE_ANY: u8 = 0x1F;
pub const CONTROL_GATE: u8 = 0x01;
pub const CONTROL_SYNC: u8 = 0x02;
pub const CONTROL_RING: u8 = 0x04;
pub const VOLUME_LOW: u8 = 0x10;
pub const VOLUME_BAND: u8 = 0x20;
pub const VOLUME_HIGH: u8 = 0x40;
pub const VOLUME_3OFF: u8 = 0x80;

/// Cycles to one step of the envelope at each rate, 0-15: a full attack (255 steps) takes
/// 2, 8, 16, 24, 38, 56, 68, 80, 100, 250, 500, 800 ms, 1, 3, 5, 8 s.
pub const RATE_CYCLES: [u16; 16] =
    [9, 32, 63, 95, 149, 220, 267, 313, 392, 977, 1954, 3126, 3907, 11720, 19532, 31251];

/// The filter's first coefficient for each CUTOFF, in 65536ths: 2 sin(pi fc / 62,500) with
/// fc = 30 Hz x 2^(CUTOFF / 32).
#[rustfmt::skip]
pub const CUTOFF_TABLE: [u16; 256] = [
    198, 202, 206, 211, 216, 220, 225, 230, 235, 240, 245, 251, 256, 262, 268, 274,
    280, 286, 292, 298, 305, 311, 318, 325, 332, 340, 347, 355, 362, 370, 379, 387,
    395, 404, 413, 422, 431, 441, 450, 460, 470, 480, 491, 502, 513, 524, 535, 547,
    559, 571, 584, 597, 610, 623, 637, 651, 665, 679, 694, 709, 725, 741, 757, 774,
    791, 808, 826, 844, 862, 881, 900, 920, 940, 961, 982, 1003, 1025, 1048, 1071, 1094,
    1118, 1143, 1168, 1193, 1219, 1246, 1273, 1301, 1330, 1359, 1388, 1419, 1450, 1482, 1514, 1547,
    1581, 1616, 1651, 1687, 1724, 1762, 1801, 1840, 1880, 1921, 1964, 2007, 2050, 2095, 2141, 2188,
    2236, 2285, 2335, 2386, 2438, 2492, 2546, 2602, 2659, 2717, 2777, 2838, 2900, 2963, 3028, 3094,
    3162, 3231, 3302, 3374, 3448, 3524, 3601, 3680, 3760, 3843, 3927, 4013, 4100, 4190, 4282, 4376,
    4471, 4569, 4669, 4772, 4876, 4983, 5092, 5203, 5317, 5433, 5552, 5674, 5798, 5925, 6055, 6187,
    6322, 6461, 6602, 6747, 6894, 7045, 7199, 7357, 7517, 7682, 7850, 8022, 8197, 8376, 8559, 8747,
    8938, 9133, 9333, 9537, 9745, 9958, 10176, 10398, 10625, 10858, 11095, 11337, 11585, 11838, 12096, 12360,
    12630, 12906, 13187, 13475, 13769, 14070, 14376, 14690, 15010, 15337, 15672, 16013, 16362, 16718, 17082, 17454,
    17834, 18222, 18618, 19023, 19437, 19859, 20290, 20731, 21181, 21641, 22110, 22589, 23079, 23579, 24089, 24610,
    25143, 25686, 26241, 26808, 27386, 27976, 28579, 29195, 29823, 30464, 31118, 31786, 32468, 33163, 33873, 34597,
    35336, 36090, 36859, 37643, 38443, 39259, 40091, 40940, 41805, 42687, 43586, 44502, 45436, 46388, 47358, 48345,
];

/// The filter's damping for each resonance, 0-15, in 65536ths: 1.414 (none) down to 0.12.
pub const RESONANCE_TABLE: [u32; 16] =
    [92668, 78616, 66695, 56582, 48002, 40723, 34548, 29309, 24865, 21094, 17896, 15182, 12880, 10927, 9270, 7864];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Attack,
    Decay,
    Release,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Voice {
    freq: u16,
    width: u8,
    wave: u8,
    control: u8,
    attack_decay: u8,
    sustain_release: u8,
    /// Sixteenths taken off the voice's level, 0-15.
    trim: u8,
    /// The 24-bit counter: FREQ is added every cycle.
    acc: u32,
    /// The noise register, 15 bits.
    lfsr: u16,
    env: u8,
    stage: Stage,
    /// Cycles since the envelope's last step, and steps skipped while it falls slowly.
    rate: u16,
    skip: u8,
}

impl Default for Voice {
    fn default() -> Self {
        Voice {
            freq: 0,
            width: 0,
            wave: 0,
            control: 0,
            attack_decay: 0,
            sustain_release: 0,
            trim: 0,
            acc: 0,
            lfsr: 0x7FFF,
            env: 0,
            stage: Stage::Release,
            rate: 0,
            skip: 0,
        }
    }
}

impl Voice {
    /// One cycle of the envelope.
    fn envelope(&mut self) {
        // let go and died away: nothing to count until the gate comes on (which starts the count over)
        if self.stage == Stage::Release && self.env == 0 {
            return;
        }
        let rate = match self.stage {
            Stage::Attack => self.attack_decay >> 4,
            Stage::Decay => self.attack_decay & 15,
            Stage::Release => self.sustain_release & 15,
        };
        self.rate += 1;
        if self.rate < RATE_CYCLES[rate as usize] {
            return;
        }
        self.rate = 0;
        if self.stage == Stage::Attack {
            self.env = self.env.wrapping_add(1); // a gate at full loudness goes round to 0, as the release build does
            if self.env == 255 {
                self.stage = Stage::Decay;
                self.skip = 0;
            }
            return;
        }
        // falling: slower the lower it is, so it dies away as a struck thing does
        let wait = match self.env {
            94.. => 1,
            55.. => 2,
            27.. => 4,
            15.. => 8,
            7.. => 16,
            _ => 30,
        };
        self.skip += 1;
        if self.skip < wait {
            return;
        }
        self.skip = 0;
        let floor = if self.stage == Stage::Decay { (self.sustain_release >> 4) * 17 } else { 0 };
        if self.env > floor {
            self.env -= 1;
        }
    }

    /// The waveform now, 12 bits. `before` is the top bit of the voice before, for ring.
    fn wave(&self, before: bool, table: &[u8; 32]) -> u16 {
        let mut out = 0x0FFF;
        if self.wave & WAVE_TRIANGLE != 0 {
            let fold = (self.acc >> 23 != 0) ^ (self.control & CONTROL_RING != 0 && before);
            let t = (self.acc >> 11) as u16 & 0x0FFF;
            out &= if fold { t ^ 0x0FFF } else { t };
        }
        if self.wave & WAVE_SAW != 0 {
            out &= (self.acc >> 12) as u16;
        }
        if self.wave & WAVE_PULSE != 0 && (self.acc >> 16) as u8 >= self.width {
            out = 0;
        }
        if self.wave & WAVE_NOISE != 0 {
            out &= (self.lfsr >> 3) & 0x0FF0;
        }
        if self.wave & WAVE_TABLE != 0 {
            out &= (table[(self.acc >> 19) as usize & 31] as u16) << 4;
        }
        out
    }
}

/// The chip. `samples` grows by one every 16 cycles clocked; whoever plays it drains it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Chip {
    voices: [Voice; 3],
    cutoff: u8,
    filter: u8,
    volume: u8,
    table: [u8; 32],
    /// The filter's two integrators.
    low: i64,
    band: i64,
    /// Cycles into the sample being made, and each voice's waveform summed over them.
    phase: u8,
    sums: [u32; 3],
    pub samples: Vec<i16>,
}

impl Chip {
    /// What register `reg` (0-63) reads, without running the chip.
    pub fn peek(&self, reg: u8) -> u8 {
        match reg {
            0..=0x17 => {
                let v = &self.voices[(reg >> 3) as usize];
                match reg & 7 {
                    FREQ_LO => v.freq as u8,
                    FREQ_HI => (v.freq >> 8) as u8,
                    WIDTH => v.width,
                    WAVE => v.wave,
                    CONTROL => v.control,
                    ATTACK_DECAY => v.attack_decay,
                    SUSTAIN_RELEASE => v.sustain_release,
                    _ => v.trim,
                }
            }
            CUTOFF => self.cutoff,
            FILTER => self.filter,
            VOLUME => self.volume,
            VOICE3 => (self.voices[2].wave(self.voices[1].acc >> 23 != 0, &self.table) >> 4) as u8,
            ENV3 => self.voices[2].env,
            SOUND => ID,
            0x20..=0x3F => self.table[(reg - WAVETABLE) as usize],
            _ => 0xFF,
        }
    }

    /// A write to register `reg` (0-63).
    pub fn store(&mut self, reg: u8, value: u8) {
        match reg {
            0..=0x17 => {
                let v = &mut self.voices[(reg >> 3) as usize];
                match reg & 7 {
                    FREQ_LO => v.freq = v.freq & 0xFF00 | value as u16,
                    FREQ_HI => v.freq = v.freq & 0x00FF | (value as u16) << 8,
                    WIDTH => v.width = value,
                    WAVE => {
                        // noise switched on, or its length changed: the register starts over,
                        // so a drum is the same drum every time
                        if value & WAVE_NOISE != 0 && (value ^ v.wave) & (WAVE_NOISE | WAVE_SHORT) != 0 {
                            v.lfsr = 0x7FFF;
                        }
                        v.wave = value & (WAVE_ANY | WAVE_SHORT);
                    }
                    CONTROL => {
                        let gate = value & CONTROL_GATE != 0;
                        if gate != (v.control & CONTROL_GATE != 0) {
                            v.stage = if gate { Stage::Attack } else { Stage::Release };
                            v.rate = 0;
                            v.skip = 0;
                        }
                        v.control = value & 7;
                    }
                    ATTACK_DECAY => v.attack_decay = value,
                    SUSTAIN_RELEASE => v.sustain_release = value,
                    _ => v.trim = value & 15,
                }
            }
            CUTOFF => self.cutoff = value,
            FILTER => self.filter = value & 0xF7,
            VOLUME => self.volume = value,
            0x20..=0x3F => self.table[(reg - WAVETABLE) as usize] = value,
            _ => {}
        }
    }

    /// Runs the chip for `cycles` cycles.
    pub fn clock(&mut self, cycles: u64) {
        if self.at_rest() {
            // nothing moves but the sample clock (and the filter, dying away): the same
            // samples as stepping every cycle, without stepping
            let waves: [u32; 3] =
                std::array::from_fn(|i| self.voices[i].wave(self.voices[(i + 2) % 3].acc >> 23 != 0, &self.table) as u32);
            let mut left = cycles;
            while left > 0 {
                let n = left.min((CYCLES_PER_SAMPLE - self.phase) as u64);
                for (sum, w) in self.sums.iter_mut().zip(waves) {
                    *sum += w * n as u32;
                }
                self.phase += n as u8;
                left -= n;
                if self.phase == CYCLES_PER_SAMPLE {
                    self.phase = 0;
                    let s = self.sample();
                    self.samples.push(s);
                }
            }
            return;
        }
        for _ in 0..cycles {
            self.cycle();
        }
    }

    /// No voice is sounding or counting: every gate off, every envelope at 0, every FREQ 0.
    /// A machine that never touches the chip stays here.
    fn at_rest(&self) -> bool {
        self.voices.iter().all(|v| v.freq == 0 && v.env == 0 && v.control & CONTROL_GATE == 0)
    }

    fn cycle(&mut self) {
        // the counters first: a voice that wraps restarts the next one, if that one asks
        let mut wrapped = [false; 3];
        for (i, v) in self.voices.iter_mut().enumerate() {
            let old = v.acc;
            v.acc = (old + v.freq as u32) & 0x00FF_FFFF;
            wrapped[i] = v.acc < old;
            if v.acc & !old & 0x0008_0000 != 0 {
                let bit = (v.lfsr ^ (v.lfsr >> 1)) & 1;
                v.lfsr = (v.lfsr >> 1) | (bit << 14);
                if v.wave & WAVE_SHORT != 0 {
                    v.lfsr = v.lfsr & !0x40 | (bit << 6);
                }
            }
            v.envelope();
        }
        for i in 0..3 {
            let before = (i + 2) % 3;
            if self.voices[i].control & CONTROL_SYNC != 0 && wrapped[before] {
                self.voices[i].acc = 0;
            }
            let top = self.voices[before].acc >> 23 != 0;
            self.sums[i] += self.voices[i].wave(top, &self.table) as u32;
        }
        self.phase += 1;
        if self.phase == CYCLES_PER_SAMPLE {
            self.phase = 0;
            let s = self.sample();
            self.samples.push(s);
        }
    }

    /// One sample from the 16 cycles just summed.
    fn sample(&mut self) -> i16 {
        let (mut direct, mut filtered) = (0i64, 0i64);
        for (i, v) in self.voices.iter().enumerate() {
            let sum = std::mem::take(&mut self.sums[i]);
            if v.wave & WAVE_ANY == 0 || (i == 2 && self.volume & VOLUME_3OFF != 0) {
                continue;
            }
            let x = (sum as i64 / CYCLES_PER_SAMPLE as i64 - 2048) * v.env as i64 * (16 - v.trim as i64) / 16;
            if self.filter & (1 << i) != 0 {
                filtered += x;
            } else {
                direct += x;
            }
        }
        let f = CUTOFF_TABLE[self.cutoff as usize] as i64;
        let q = RESONANCE_TABLE[(self.filter >> 4) as usize] as i64;
        self.low += (f * self.band) >> 16;
        let high = filtered - self.low - ((q * self.band) >> 16);
        self.band += (f * high) >> 16;
        let mut out = direct;
        if self.volume & VOLUME_LOW != 0 {
            out += self.low;
        }
        if self.volume & VOLUME_BAND != 0 {
            out += self.band;
        }
        if self.volume & VOLUME_HIGH != 0 {
            out += high;
        }
        ((out * (self.volume & 15) as i64) >> 10).clamp(-32767, 32767) as i16
    }

    /// Save states: 121 bytes, the registers and everything inside. Not the samples.
    pub(crate) fn save(&self, w: &mut Writer) {
        for v in &self.voices {
            w.u16(v.freq);
            for b in [v.width, v.wave, v.control, v.attack_decay, v.sustain_release, v.trim] {
                w.u8(b);
            }
            w.u32(v.acc);
            w.u16(v.lfsr);
            w.u8(v.env);
            w.u8(v.stage as u8);
            w.u16(v.rate);
            w.u8(v.skip);
        }
        for b in [self.cutoff, self.filter, self.volume] {
            w.u8(b);
        }
        w.bytes(&self.table);
        w.u64(self.low as u64);
        w.u64(self.band as u64);
        w.u8(self.phase);
        for s in self.sums {
            w.u32(s);
        }
    }

    pub(crate) fn restore(r: &mut Reader) -> Result<Chip, String> {
        let mut c = Chip::default();
        for v in &mut c.voices {
            v.freq = r.u16()?;
            let b = r.bytes(6)?;
            (v.width, v.wave, v.control, v.attack_decay, v.sustain_release, v.trim) =
                (b[0], b[1] & (WAVE_ANY | WAVE_SHORT), b[2] & 7, b[3], b[4], b[5] & 15);
            v.acc = r.u32()? & 0x00FF_FFFF;
            v.lfsr = r.u16()? & 0x7FFF;
            v.env = r.u8()?;
            v.stage = match r.u8()? {
                0 => Stage::Attack,
                1 => Stage::Decay,
                2 => Stage::Release,
                s => return Err(format!("save state: sound chip envelope {s}")),
            };
            v.rate = r.u16()?;
            v.skip = r.u8()?;
        }
        let b = r.bytes(3)?;
        (c.cutoff, c.filter, c.volume) = (b[0], b[1] & 0xF7, b[2]);
        c.table = r.bytes(32)?.try_into().unwrap();
        c.low = r.u64()? as i64;
        c.band = r.u64()? as i64;
        c.phase = r.u8()? % CYCLES_PER_SAMPLE;
        for s in &mut c.sums {
            *s = r.u32()? & 0xFFFF;
        }
        Ok(c)
    }
}

/// The chip in the machine: run up to the cycle of each register access and to the end of
/// each frame, its samples kept a frame at a time for the hosts, as the beeper's are.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Card {
    pub chip: Chip,
    /// The cycle the chip has been run to.
    at: u64,
    /// The last whole frame's samples, and the smoothing carried from frame to frame.
    last: Vec<i16>,
    lp: i32,
    tail: i32,
}

impl Card {
    /// A card whose chip (new, or from a save) starts running at cycle `at`.
    pub fn at(chip: Chip, at: u64) -> Card {
        Card { chip, at, ..Card::default() }
    }

    /// Runs the chip up to cycle `t`.
    pub fn catch_up(&mut self, t: u64) {
        if t > self.at {
            self.chip.clock(t - self.at);
            self.at = t;
        }
    }

    /// The frame ended at cycle `t`.
    pub fn end_frame(&mut self, t: u64) {
        self.catch_up(t);
        std::mem::swap(&mut self.last, &mut self.chip.samples);
        self.chip.samples.clear();
    }

    /// Adds the last frame's samples to `out`, spread evenly over it: a low-pass at about
    /// 11 kHz, then a straight line between neighbours. Integers until the last step.
    pub fn mix(&mut self, out: &mut [f32]) {
        let (m, n) = (self.last.len() as u64, out.len() as u64);
        if m == 0 || n == 0 {
            return;
        }
        let mut smooth = Vec::with_capacity(m as usize + 1);
        smooth.push(self.tail);
        for &s in &self.last {
            self.lp += (s as i32 - self.lp) * 43_850 / 65_536;
            smooth.push(self.lp);
        }
        self.tail = self.lp;
        for (i, o) in out.iter_mut().enumerate() {
            let p = (i as u64 + 1) * m * 65_536 / n;
            let (k, frac) = ((p >> 16) as usize, (p & 0xFFFF) as i64);
            let (a, b) = (smooth[k] as i64, smooth[(k + 1).min(m as usize)] as i64);
            *o += (a + (((b - a) * frac) >> 16)) as f32 / 32_768.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// At rest the chip skips ahead; that must leave it exactly where stepping would.
    #[test]
    fn resting_is_the_same_as_stepping() {
        let setup = |c: &mut Chip| {
            c.store(WAVE, WAVE_SAW);
            c.store(8 + WAVE, WAVE_PULSE);
            c.store(8 + WIDTH, 0x40);
            c.store(SUSTAIN_RELEASE, 0xF0);
            c.store(VOLUME, 15 | VOLUME_LOW);
            c.store(FILTER, 0x21);
            c.store(CUTOFF, 120);
        };
        let (mut skipped, mut stepped) = (Chip::default(), Chip::default());
        setup(&mut skipped);
        setup(&mut stepped);
        for cycles in [1u64, 7, 16, 1000, 12_345, 3] {
            assert!(skipped.at_rest());
            skipped.clock(cycles);
            for _ in 0..cycles {
                stepped.cycle();
            }
            // then a note, and both run on the same
            for c in [&mut skipped, &mut stepped] {
                c.store(FREQ_HI, 0x1C);
                c.store(CONTROL, CONTROL_GATE);
                c.clock(5_000);
                c.store(CONTROL, 0);
                c.store(FREQ_HI, 0);
                c.clock(40_000);
            }
            assert_eq!(skipped, stepped, "after {cycles} cycles at rest");
        }
        assert!(skipped.samples.iter().any(|&s| s != 0));
    }
}
