//! The beeper's timer, the speaker, the noise register, and the sound the hosts play.
//! Rev A. The spec is docs/prg/programmers-reference-guide.md, chapter 12.
//!
//! The speaker is wired the way the IBM PC/XT wires it (port 61h): an 8253-style timer
//! channel in square-wave mode, clocked at 1 MHz, feeds the speaker through an AND gate.
//!
//! ```text
//! speaker = DATA AND (GATE ? timer output : 1)
//! ```
//!
//! DATA is SPEAKER bit 1 (`Io::speaker`; a write to BEEPER flips it), GATE is SPEAKER
//! bit 0. With the gate off the timer's output is held high, so DATA alone moves the speaker:
//! bit-banging. Nothing here is stepped cycle by cycle: the timer's output at any cycle, and
//! the time it spends high in any stretch of cycles, are worked out from where its wave began.

use crate::machine::{CYCLES_PER_FRAME, Machine};

/// The timer channel that drives the speaker, as it stands. Copy-cheap, so the audio log can
/// keep a copy at each change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timer {
    /// SPEAKER bit 0. The timer counts while it's set; while it's clear the output is high.
    pub gate: bool,
    /// TIMER_LO as last written.
    pub lo: u8,
    /// TIMER_HI as last written. Writing it hands TIMER_HI:TIMER_LO to the counter.
    pub hi: u8,
    /// The divisor the counter runs with, 1-65536.
    n: u32,
    /// A cycle at which a wave with divisor `n` began: the phase. The output is high for the
    /// first half of each wave, (n + 1) / 2 cycles, and low for the rest, n / 2 cycles.
    start: i64,
    /// A new divisor waiting for the end of the half-wave that was running when it was
    /// written: (the cycle it takes over, the divisor).
    next: Option<(u64, u32)>,
}

impl Default for Timer {
    fn default() -> Self {
        Timer { gate: false, lo: 0, hi: 0, n: 65536, start: 0, next: None }
    }
}

/// Cycles high in one wave of divisor `n`: the first half, rounded up, as on the 8253.
fn high_half(n: u32) -> i64 {
    n.div_ceil(2) as i64
}

impl Timer {
    /// The divisor the counter is running with at cycle `t` (1-65536).
    pub fn divisor(&self, t: u64) -> u32 {
        self.at(t).n
    }

    /// The timer at cycle `t`: a waiting divisor has taken over if its cycle has come.
    fn at(&self, t: u64) -> Timer {
        let mut s = *self;
        if let Some((e, n)) = s.next
            && e <= t
        {
            // The old wave ends a half here. A rising edge starts a whole new wave; a falling
            // edge starts the new wave's low half.
            let rising = (e as i64 - s.start).rem_euclid(s.n as i64) == 0;
            s.start = if rising { e as i64 } else { e as i64 - high_half(n) };
            s.n = n;
            s.next = None;
        }
        s
    }

    /// The timer's output at cycle `t`: SPEAKER bit 5 on read.
    pub fn out(&self, t: u64) -> bool {
        let s = self.at(t);
        !s.gate || (t as i64 - s.start).rem_euclid(s.n as i64) < high_half(s.n)
    }

    /// How many of the cycles `a..b` the output is high.
    pub fn high(&self, a: u64, b: u64) -> u64 {
        if let Some((e, _)) = self.next
            && a < e
            && e < b
        {
            return self.high(a, e) + self.at(e).high(e, b);
        }
        let s = self.at(a);
        if !s.gate {
            return b - a;
        }
        (s.high_since_start(b) - s.high_since_start(a)) as u64
    }

    /// High cycles from `start` to `t`, counted as though the wave had always run.
    fn high_since_start(&self, t: u64) -> i64 {
        let (n, h) = (self.n as i64, high_half(self.n));
        let d = t as i64 - self.start;
        d.div_euclid(n) * h + d.rem_euclid(n).min(h)
    }

    /// SPEAKER bit 0 written at cycle `t`. Turning the gate on starts a new wave at once,
    /// high half first; turning it off holds the output high.
    pub fn set_gate(&mut self, t: u64, on: bool) {
        *self = self.at(t);
        if on == self.gate {
            return;
        }
        if let Some((_, n)) = self.next.take() {
            self.n = n;
        }
        self.gate = on;
        self.start = t as i64;
    }

    /// TIMER_HI written at cycle `t`: TIMER_HI:TIMER_LO becomes the divisor (0 is 65536).
    /// With the gate off it's simply loaded. With the timer running, it takes over at the
    /// end of the current half-wave, so a tune changes note without a click.
    pub fn commit(&mut self, t: u64) {
        *self = self.at(t);
        let n = match u16::from_le_bytes([self.lo, self.hi]) {
            0 => 65536,
            v => v as u32,
        };
        if !self.gate {
            self.n = n;
            self.next = None;
            return;
        }
        let (old, h) = (self.n as i64, high_half(self.n));
        let pos = (t as i64 - self.start).rem_euclid(old);
        let edge = t as i64 + if pos < h { h - pos } else { old - pos };
        self.next = Some((edge as u64, n));
    }
}

/// What NOISE ($C034) reads at cycle `t`: a pseudo-random byte that depends on nothing but
/// the cycle it's read at. Fully deterministic: the same program fed the same keys at the
/// same cycles reads the same bytes. (A splitmix64 step of the cycle count, mixed with a
/// constant that is the same at every power-on.)
pub fn noise(t: u64) -> u8 {
    let mut z = t ^ 0x1983_0C34_5EED_0064;
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (z ^ (z >> 31)) as u8
}

/// How loud the hosts play the speaker: a full square wave swings between +/- half of this.
pub const AUDIO_VOLUME: f32 = 0.25;

/// The speaker's state from one cycle on, until the next mark.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Mark {
    cycle: u64,
    data: bool,
    timer: Timer,
}

impl Mark {
    fn high(&self, a: u64, b: u64) -> u64 {
        if self.data { self.timer.high(a, b) } else { 0 }
    }
}

/// The speaker over emulated time, frame by frame, as `border_lines` records the border: the
/// state at the first cycle of the frame, then a cycle-stamped copy after each change. From
/// the last whole frame, `render` makes the samples the hosts play. Not visible to programs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioLog {
    /// This frame so far.
    marks: Vec<Mark>,
    /// The last whole frame, and the cycle it ended at.
    last: Vec<Mark>,
    last_end: u64,
    /// The output filter, carried from frame to frame, in 1/65536ths of the speaker's travel:
    /// the DC blocker's last input and output, and the low-pass filter's output.
    dc_in: i64,
    dc_out: i64,
    lp: i64,
}

impl Default for AudioLog {
    fn default() -> Self {
        let rest = Mark { cycle: 0, data: false, timer: Timer::default() };
        AudioLog { marks: vec![rest], last: vec![rest], last_end: 0, dc_in: 0, dc_out: 0, lp: 0 }
    }
}

impl AudioLog {
    /// The speaker changed at cycle `t` (DATA, the gate or the divisor).
    pub fn mark(&mut self, t: u64, data: bool, timer: Timer) {
        let m = Mark { cycle: t, data, timer };
        match self.marks.last_mut() {
            Some(last) if last.cycle >= t => *last = Mark { cycle: last.cycle, ..m },
            _ => self.marks.push(m),
        }
    }

    /// The frame ended at cycle `t`, which is where the next one begins.
    pub fn end_frame(&mut self, t: u64) {
        let now = *self.marks.last().expect("a frame always has its first mark");
        std::mem::swap(&mut self.marks, &mut self.last);
        self.last_end = t.max(self.last[0].cycle);
        self.marks.clear();
        self.marks.push(Mark { cycle: self.last_end, ..now });
    }

    /// Cycles `a..b` of the last frame that the speaker spent out, starting the search
    /// for the mark in force at `*j`.
    fn high(&self, j: &mut usize, a: u64, b: u64) -> u64 {
        let marks = &self.last;
        while *j + 1 < marks.len() && marks[*j + 1].cycle <= a {
            *j += 1;
        }
        let (mut sum, mut t, mut k) = (0, a, *j);
        loop {
            let end = marks.get(k + 1).map_or(b, |m| m.cycle.min(b));
            if end > t {
                sum += marks[k].high(t, end);
                t = end;
            }
            if t >= b || k + 1 >= marks.len() {
                return sum;
            }
            k += 1;
        }
    }

    /// Renders the last frame as `out.len()` samples spread evenly over its cycles.
    /// Each sample is the speaker's average position over its share of the frame (a box
    /// filter, which keeps a square wave's harmonics from folding back as harsh aliases),
    /// then a gentle low-pass (about 10 kHz) and a DC blocker (about 10 Hz), so a resting
    /// speaker, in or out, fades to exactly 0.0: no clicks, no hum. Integer arithmetic
    /// throughout, so it renders the same on every host.
    pub fn render(&mut self, sample_rate: u32, out: &mut [f32]) {
        let n = out.len() as u64;
        let sr = sample_rate.max(1) as i64;
        let pole = 65536 * sr / (sr + 63); // 2 pi x 10 Hz = 63 rad/s
        let alpha = 65536 * 62_832 / (sr + 62_832); // 2 pi x 10 kHz
        let start = self.last[0].cycle;
        let len = self.last_end - start;
        let mut j = 0;
        for (i, o) in out.iter_mut().enumerate() {
            let a = start + len * i as u64 / n;
            let b = start + len * (i as u64 + 1) / n;
            let x = if b > a { (self.high(&mut j, a, b) * 65536 / (b - a)) as i64 } else { self.dc_in };
            let y = x - self.dc_in + self.dc_out * pole / 65536;
            self.dc_in = x;
            self.dc_out = y;
            let d = y - self.lp;
            self.lp += (d.abs() * alpha + 65535) / 65536 * d.signum(); // rounded up: reaches y exactly
            *o = self.lp as f32 * (AUDIO_VOLUME / 65536.0);
        }
    }
}

impl Machine {
    /// Renders the speaker over the last whole frame into `out`, one sample per element, at
    /// `sample_rate` samples a second (it sets the smoothing filters). Call it once after
    /// each `run_frame`. For real-time playback, `out` holds about `samples_per_frame`
    /// samples (880.6 at 44.1 kHz); the frame is spread over however many there are, so a
    /// host can render one or two more or fewer to keep its buffer level.
    /// Rev A. Hosts only: nothing a program can see changes.
    pub fn audio_frame(&mut self, sample_rate: u32, out: &mut [f32]) {
        self.mem.io.audio.render(sample_rate, out);
    }

    /// Samples in one frame of emulated time at `sample_rate`: 19,968 cycles at 1 MHz.
    pub fn samples_per_frame(sample_rate: u32) -> f64 {
        sample_rate as f64 * CYCLES_PER_FRAME as f64 / 1_000_000.0
    }

    /// The speaker's position now: out (true) or in.
    pub fn speaker_out(&self) -> bool {
        let io = &self.mem.io;
        io.speaker && io.timer.out(io.cycle)
    }
}

/// Save states (`crate::state`): the timer's whole state, private fields included.
impl Timer {
    pub(crate) fn save(&self, w: &mut crate::state::Writer) {
        w.bool(self.gate);
        w.u8(self.lo);
        w.u8(self.hi);
        w.u32(self.n);
        w.u64(self.start as u64);
        w.bool(self.next.is_some());
        let (e, n) = self.next.unwrap_or((0, 0));
        w.u64(e);
        w.u32(n);
    }

    pub(crate) fn restore(r: &mut crate::state::Reader) -> Result<Timer, String> {
        let (gate, lo, hi, n, start) = (r.bool()?, r.u8()?, r.u8()?, r.u32()?, r.u64()? as i64);
        let (has_next, e, nn) = (r.bool()?, r.u64()?, r.u32()?);
        let ok = |d: u32| (1..=65536).contains(&d);
        if !ok(n) || (has_next && !ok(nn)) {
            return Err("save state: bad timer divisor".into());
        }
        Ok(Timer { gate, lo, hi, n, start, next: has_next.then_some((e, nn)) })
    }
}

impl AudioLog {
    /// A log that starts at cycle `t` with the speaker as it stands: what a loaded save
    /// state plays from. The frame before it is silence.
    pub(crate) fn resume(t: u64, data: bool, timer: Timer) -> AudioLog {
        let m = Mark { cycle: t, data, timer };
        AudioLog { marks: vec![m], last: vec![Mark { data: false, ..m }], last_end: t, dc_in: 0, dc_out: 0, lp: 0 }
    }
}
