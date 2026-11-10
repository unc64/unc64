//! The sound chip (manual ch. 12): the chip alone, then in the machine.

use unc_core::chip::*;
use unc_core::{Machine, RamSize};

const V1: u8 = 0;
const V2: u8 = 8;
const V3: u8 = 16;

/// A chip with one voice sounding: a waveform, a pitch, held at full level.
fn held(voice: u8, wave: u8, freq: u16) -> Chip {
    let mut c = Chip::default();
    c.store(voice + FREQ_LO, freq as u8);
    c.store(voice + FREQ_HI, (freq >> 8) as u8);
    c.store(voice + WIDTH, 0x80);
    c.store(voice + WAVE, wave);
    c.store(voice + ATTACK_DECAY, 0x00);
    c.store(voice + SUSTAIN_RELEASE, 0xF0);
    c.store(VOLUME, 15);
    c.store(voice + CONTROL, CONTROL_GATE);
    c
}

fn run(c: &mut Chip, seconds: f64) -> Vec<i16> {
    c.samples.clear();
    c.clock((seconds * 1e6) as u64);
    std::mem::take(&mut c.samples)
}

/// Upward crossings of zero: the pitch, for one second of samples.
fn rises(s: &[i16]) -> usize {
    s.windows(2).filter(|w| w[0] < 0 && w[1] >= 0).count()
}

fn rms(s: &[i16]) -> f64 {
    (s.iter().map(|&v| (v as f64).powi(2)).sum::<f64>() / s.len() as f64).sqrt()
}

#[test]
fn silent_at_reset_and_a_sample_every_16_cycles() {
    let mut c = Chip::default();
    c.clock(1_000_000);
    assert_eq!(c.samples.len(), 62_500);
    assert!(c.samples.iter().all(|&s| s == 0));
    assert_eq!(c.peek(SOUND), ID);
    assert_eq!(c.peek(0x1B), 0xFF, "reserved");
}

#[test]
fn registers_read_as_written() {
    let mut c = Chip::default();
    for (reg, v) in [(V2 + FREQ_LO, 0x34), (V2 + FREQ_HI, 0x12), (V2 + WIDTH, 0x40), (V2 + ATTACK_DECAY, 0x9A),
        (V2 + SUSTAIN_RELEASE, 0xBC), (V2 + TRIM, 0x0C), (CUTOFF, 0x77), (VOLUME, 0x9F), (WAVETABLE + 5, 0xEE)]
    {
        c.store(reg, v);
        assert_eq!(c.peek(reg), v, "register {reg:#x}");
    }
    c.store(FILTER, 0xFF);
    assert_eq!(c.peek(FILTER), 0xF7);
    c.store(V1 + WAVE, 0xFF);
    assert_eq!(c.peek(V1 + WAVE), 0x9F);
    c.store(SOUND, 0);
    assert_eq!(c.peek(SOUND), ID);
}

#[test]
fn pitch_is_freq_times_a_million_over_two_to_the_24() {
    // A, 440 Hz: 7382
    for wave in [WAVE_TRIANGLE, WAVE_SAW, WAVE_PULSE, WAVE_TABLE] {
        let mut c = held(V1, wave, 7382);
        for i in 0..32 {
            c.store(WAVETABLE + i, if i < 16 { 0 } else { 255 });
        }
        run(&mut c, 0.1);
        let n = rises(&run(&mut c, 1.0));
        assert!((439..=441).contains(&n), "wave {wave:#x}: {n} Hz");
    }
}

#[test]
fn a_voice_at_full_level_peaks_near_a_quarter_of_full_scale() {
    let mut c = held(V1, WAVE_SAW, 7382);
    run(&mut c, 0.1);
    let s = run(&mut c, 0.1);
    let peak = s.iter().map(|v| v.unsigned_abs()).max().unwrap();
    assert!((7000..=7700).contains(&peak), "{peak}");
    // TRIM takes sixteenths off the voice
    c.store(V1 + TRIM, 8);
    let s = run(&mut c, 0.1);
    let half = s.iter().map(|v| v.unsigned_abs()).max().unwrap();
    assert!((3500..=3850).contains(&half), "{half}");
    c.store(V1 + TRIM, 0);
    // and VOLUME scales it
    c.store(VOLUME, 5);
    let s = run(&mut c, 0.1);
    let quiet = s.iter().map(|v| v.unsigned_abs()).max().unwrap();
    assert!((2300..=2600).contains(&quiet), "{quiet}");
}

#[test]
fn the_envelope_rises_holds_and_falls() {
    let mut c = Chip::default();
    c.store(V3 + ATTACK_DECAY, 0x00);
    c.store(V3 + SUSTAIN_RELEASE, 0xA0);
    c.store(V3 + CONTROL, CONTROL_GATE);
    c.clock(254 * 9);
    assert!(c.peek(ENV3) >= 250 && c.peek(ENV3) < 255, "{}", c.peek(ENV3));
    c.clock(9);
    assert_eq!(c.peek(ENV3), 255, "a full attack at rate 0: 2.3 ms");
    c.clock(100_000);
    assert_eq!(c.peek(ENV3), 0xA * 17, "decayed to the sustain level and held");
    c.clock(1_000_000);
    assert_eq!(c.peek(ENV3), 0xA * 17);
    c.store(V3 + CONTROL, 0);
    c.clock(1_000);
    let falling = c.peek(ENV3);
    assert!(falling < 0xA * 17 && falling > 0);
    c.clock(100_000);
    assert_eq!(c.peek(ENV3), 0);
    // a slow attack: rate 9 takes a quarter of a second
    c.store(V3 + ATTACK_DECAY, 0x90);
    c.store(V3 + CONTROL, CONTROL_GATE);
    c.clock(125_000);
    assert!((125..=130).contains(&c.peek(ENV3)), "{}", c.peek(ENV3));
}

#[test]
fn the_fall_slows_near_the_bottom() {
    let mut c = Chip::default();
    c.store(V3 + ATTACK_DECAY, 0x00);
    c.store(V3 + SUSTAIN_RELEASE, 0xF8);
    c.store(V3 + CONTROL, CONTROL_GATE);
    c.clock(10_000);
    c.store(V3 + CONTROL, 0);
    let time_to = |c: &mut Chip, level: u8| {
        let mut t = 0u64;
        while c.peek(ENV3) > level {
            c.clock(100);
            t += 100;
        }
        t
    };
    let first = time_to(&mut c, 128);
    let second = time_to(&mut c, 1);
    assert!(second > first * 4, "{first} then {second}");
}

#[test]
fn pulse_width_sets_the_high_part() {
    let mut c = held(V1, WAVE_PULSE, 7382);
    c.store(V1 + WIDTH, 0x40);
    run(&mut c, 0.1);
    let s = run(&mut c, 1.0);
    let high = s.iter().filter(|&&v| v > 0).count() as f64 / s.len() as f64;
    assert!((0.24..=0.26).contains(&high), "{high}");
}

#[test]
fn noise_is_the_same_every_time_and_short_noise_repeats() {
    let hit = |wave: u8| {
        let mut c = held(V1, wave, 0x8000);
        run(&mut c, 0.05)
    };
    assert_eq!(hit(WAVE_NOISE), hit(WAVE_NOISE));
    assert_ne!(hit(WAVE_NOISE), hit(WAVE_NOISE | WAVE_SHORT));
    // FREQ $8000: the register steps every 32 cycles, two samples; its 127 steps come
    // round every 254 samples
    let mut c = held(V1, WAVE_NOISE | WAVE_SHORT, 0x8000);
    run(&mut c, 0.1);
    let s = run(&mut c, 0.1);
    assert_eq!(s[..254], s[254..508]);
    assert!(rms(&s) > 1000.0);
    let long = hit(WAVE_NOISE);
    assert_ne!(long[2000..2254], long[2254..2508]);
}

#[test]
fn the_filter_takes_the_top_off_and_stays_put_when_it_rings() {
    let through = |cutoff: u8, mode: u8, res: u8| {
        let mut c = held(V1, WAVE_SAW, 7382);
        c.store(CUTOFF, cutoff);
        c.store(FILTER, 0x01 | res << 4);
        c.store(VOLUME, 15 | mode);
        run(&mut c, 0.2);
        rms(&run(&mut c, 0.5))
    };
    let open = {
        let mut c = held(V1, WAVE_SAW, 7382);
        run(&mut c, 0.2);
        rms(&run(&mut c, 0.5))
    };
    // corner at 30 Hz x 2^(96/32) = 240 Hz, under the note: most of it goes
    assert!(through(96, VOLUME_LOW, 0) < open * 0.4);
    // corner near the top: nearly all of it passes
    assert!(through(250, VOLUME_LOW, 0) > open * 0.9);
    // the high-pass is the other half
    assert!(through(250, VOLUME_HIGH, 0) < open * 0.45);
    assert!(through(40, VOLUME_HIGH, 0) > open * 0.9);
    // sent to the filter with no output chosen: silence
    assert_eq!(through(128, 0, 0), 0.0);
    // every corner at full resonance: loud, never runaway
    for cutoff in (0..=255).step_by(5) {
        let mut c = held(V1, WAVE_SAW | WAVE_PULSE, 1000 + cutoff as u16 * 200);
        c.store(CUTOFF, cutoff);
        c.store(FILTER, 0xF1);
        c.store(VOLUME, 1 | VOLUME_LOW | VOLUME_BAND);
        let s = run(&mut c, 0.5);
        assert!(s.iter().all(|v| v.unsigned_abs() < 30_000), "cutoff {cutoff}");
    }
}

#[test]
fn ring_and_sync_change_the_voice_after() {
    let plain = {
        let mut c = held(V2, WAVE_TRIANGLE, 7382);
        c.store(V1 + FREQ_HI, 0x31);
        run(&mut c, 0.2)
    };
    for bit in [CONTROL_RING, CONTROL_SYNC] {
        let mut c = held(V2, WAVE_TRIANGLE, 7382);
        c.store(V1 + FREQ_HI, 0x31);
        c.store(V2 + CONTROL, CONTROL_GATE | bit);
        assert_ne!(run(&mut c, 0.2), plain, "bit {bit:#x}");
    }
}

#[test]
fn voice_3_can_be_read_and_silenced() {
    let mut c = held(V3, WAVE_SAW, 0x0100);
    c.store(VOLUME, 15 | VOLUME_3OFF);
    let s = run(&mut c, 0.1);
    assert!(s.iter().all(|&v| v == 0));
    // the sawtooth climbs: FREQ $0100 adds 1 to the top 8 bits every 256 cycles
    let a = c.peek(VOICE3);
    c.clock(256 * 10);
    assert_eq!(c.peek(VOICE3), a.wrapping_add(10));
}

// ---------------------------------------------------------------- in the machine

const BASE: u16 = 0xC0C0;

#[test]
fn every_machine_has_it_and_a_host_can_take_it_out() {
    let mut m = Machine::new(RamSize::K16);
    assert!(m.has_sound(), "from the UNC 16 up");
    assert_eq!(m.peek(0xC0DF), ID);
    m.poke(BASE + CUTOFF as u16, 0x55);
    assert_eq!(m.peek(BASE + CUTOFF as u16), 0x55);
    m.reset();
    assert!(m.has_sound());
    assert_eq!(m.peek(BASE + CUTOFF as u16), 0, "a reset clears it");
    m.set_sound(false);
    assert!(!m.has_sound());
    for a in BASE..=0xC0FF {
        m.poke(a, 0x55);
        assert_eq!(m.peek(a), 0xFF, "{a:#x}");
    }
    m.reset();
    assert!(!m.has_sound(), "and a reset leaves it out");
    m.set_sound(true);
    assert_eq!(m.peek(0xC0DF), ID);
}

fn note_on(m: &mut Machine) {
    for (reg, v) in [(FREQ_LO, 0xD6), (FREQ_HI, 0x1C), (WAVE, WAVE_SAW), (ATTACK_DECAY, 0x09), (SUSTAIN_RELEASE, 0xA8),
        (VOLUME, 15), (CONTROL, CONTROL_GATE)]
    {
        m.poke(BASE + reg as u16, v);
    }
}

#[test]
fn the_machine_plays_it() {
    let mut m = Machine::boot(RamSize::K16);
    let mut out = vec![0.0f32; 882];
    for _ in 0..20 {
        m.run_frame();
        m.audio_frame(44_100, &mut out);
    }
    assert!(out.iter().all(|v| v.abs() < 1e-4), "quiet before the note");
    note_on(&mut m);
    let mut all = Vec::new();
    for _ in 0..25 {
        m.run_frame();
        m.audio_frame(44_100, &mut out);
        all.extend_from_slice(&out);
    }
    let peak = all.iter().fold(0.0f32, |a, v| a.max(v.abs()));
    assert!((0.1..0.3).contains(&peak), "{peak}");
    // half a second of A: about 220 rises
    let n = all.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
    assert!((215..=225).contains(&n), "{n}");
    // no click where one frame's samples meet the next
    let step = all.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0f32, f32::max);
    assert!(step < peak * 1.5, "{step}");
}

#[test]
fn a_saved_state_carries_the_chip() {
    let mut m = Machine::boot(RamSize::K16);
    let with = m.save_state().len();
    m.set_sound(false);
    assert_eq!(m.save_state().len(), with - 121, "121 bytes of chip");
    m.set_sound(true);
    note_on(&mut m);
    for _ in 0..3 {
        m.run_frame();
    }
    let saved = m.save_state();
    assert_eq!(saved.len(), with);
    let mut other = Machine::new(RamSize::K16);
    other.load_state(&saved).unwrap();
    assert!(other.has_sound());
    let (mut a, mut b) = (vec![0.0f32; 882], vec![0.0f32; 882]);
    for frame in 0..4 {
        m.run_frame();
        other.run_frame();
        m.audio_frame(44_100, &mut a);
        other.audio_frame(44_100, &mut b);
        // the first frame after a load starts its smoothing from rest
        if frame > 0 {
            assert_eq!(a, b, "frame {frame}");
        }
    }
    assert_eq!(m.save_state(), other.save_state());
    // a version 3 state is from before the chip: it loads with one, silent
    let mut plain = Machine::boot(RamSize::K16);
    plain.set_sound(false);
    let mut old = plain.save_state();
    old[4] = 3;
    let at = old.len() - (4 + 4 + 312 + 0x4000) - 1;
    assert_eq!(old[at], 0);
    old.remove(at);
    assert_eq!(old[36], 4, "TEXTROWS, after the bitmap's page: version 5's");
    old.remove(36);
    other.load_state(&old).unwrap();
    assert!(other.has_sound());
    assert_eq!(other.peek(BASE + FREQ_HI as u16), 0);
}
