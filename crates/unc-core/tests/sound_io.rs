//! Rev A hardware: the beeper's timer and the audio it makes, NOISE, the shifter and
//! the keyboard matrix (manual chapters 8, 11 and 12, Appendix B).

use unc_core::cpu::Bus;
use unc_core::isa::{Mode, Op, opcode_for};
use unc_core::keys::key_position;
use unc_core::machine::{CYCLES_PER_FRAME, HELD_KEYS, Machine, RamSize, io};
use unc_core::shifter::Shifter;
use unc_core::sound::{AUDIO_VOLUME, Timer, noise};

fn op(o: Op, m: Mode) -> u8 {
    opcode_for(o, m).unwrap()
}

fn lda(v: u8) -> [u8; 2] {
    [op(Op::Lda, Mode::Imm), v]
}

fn sta(addr: u16) -> [u8; 3] {
    let [lo, hi] = addr.to_le_bytes();
    [op(Op::Sta, Mode::Abs), lo, hi]
}

/// A machine with `code` at $0800 followed by `JMP *`, the CPU pointed at it (no ROM boot).
fn with_program(code: &[u8]) -> Machine {
    let mut m = Machine::new(RamSize::K16);
    let mut code = code.to_vec();
    let here = 0x0800 + code.len() as u16;
    code.extend([op(Op::Jmp, Mode::Abs), here as u8, (here >> 8) as u8]);
    m.load(0x0800, &code);
    m.cpu.pc = 0x0800;
    m
}

/// Code that sets the timer's divisor and then writes `ctl` to SPEAKER.
fn tone(divisor: u16, ctl: u8) -> Vec<u8> {
    let [lo, hi] = divisor.to_le_bytes();
    [lda(lo).as_slice(), &sta(io::TIMER_LO), &lda(hi), &sta(io::TIMER_HI), &lda(ctl), &sta(io::SPEAKER)].concat()
}

/// Steps until the instruction at `pc` is next, and returns the cycle it starts at.
fn run_to(m: &mut Machine, pc: u16) -> u64 {
    m.run_until(pc, 1_000_000).expect("reaches pc");
    m.cpu.cycles
}

/// Runs `frames` whole frames and returns their audio, `per` samples a frame at 44.1 kHz.
fn audio(m: &mut Machine, frames: usize, per: usize) -> Vec<f32> {
    let mut all = vec![];
    for _ in 0..frames {
        m.run_frame();
        let mut buf = vec![1.0f32; per];
        m.audio_frame(44_100, &mut buf);
        all.extend(buf);
    }
    all
}

/// Times the signal changes sign, ignoring exact zeros.
fn zero_crossings(s: &[f32]) -> usize {
    let signs: Vec<bool> = s.iter().filter(|&&v| v != 0.0).map(|&v| v > 0.0).collect();
    signs.windows(2).filter(|w| w[0] != w[1]).count()
}

// ---------- the timer ----------

#[test]
fn the_timer_makes_a_square_wave_of_1mhz_over_the_divisor_to_the_cycle() {
    let mut m = with_program(&tone(1000, 0b11));
    let on = run_to(&mut m, 0x0800 + 15); // STA SPEAKER starts at this cycle...
    let on = on - 4; // ...minus its own 4 cycles: run_to stops after it
    let t = m.mem.io.timer;
    for k in 0..50u64 {
        let w = on + 1000 * k;
        assert!(t.out(w) && t.out(w + 499), "high for the first 500 cycles of wave {k}");
        assert!(!t.out(w + 500) && !t.out(w + 999), "low for the second 500 of wave {k}");
    }
    assert_eq!(t.high(on, on + 1_000_000), 500_000, "a second: 1,000 waves, half high");
    assert!(m.speaker_out(), "the speaker follows the timer");
    assert_eq!(m.peek(io::SPEAKER) & 0x23, 0x23, "gate, data, and the output (bit 5) high");
}

#[test]
fn an_odd_divisor_is_high_one_cycle_longer_than_low() {
    let mut t = Timer::default();
    t.lo = 0xE9;
    t.hi = 0x03; // 1001
    t.commit(0);
    t.set_gate(100, true);
    assert!(t.out(100 + 500) && !t.out(100 + 501) && !t.out(100 + 1000) && t.out(100 + 1001));
    assert_eq!(t.high(100, 100 + 1001), 501);
    assert_eq!(t.divisor(0), 1001);
}

#[test]
fn a_divisor_of_0_means_65536() {
    let mut m = with_program(&tone(0, 0b11));
    let on = run_to(&mut m, 0x0800 + 15) - 4;
    let t = m.mem.io.timer;
    assert_eq!(t.divisor(on), 65536);
    assert!(t.out(on + 32767) && !t.out(on + 32768) && !t.out(on + 65535) && t.out(on + 65536));
}

#[test]
fn a_divisor_of_1_holds_the_output_high() {
    let mut t = Timer::default();
    t.lo = 1;
    t.commit(0);
    t.set_gate(0, true);
    assert_eq!(t.high(0, 10_000), 10_000);
}

#[test]
fn with_the_gate_off_the_output_is_high_and_the_divisor_loads_at_once() {
    let mut m = with_program(&tone(1000, 0b10)); // data on, gate off
    let t0 = run_to(&mut m, 0x0800 + 15);
    let t = m.mem.io.timer;
    assert!(!t.gate);
    assert!((t0..t0 + 5000).all(|c| t.out(c)), "held high");
    assert_eq!(t.divisor(t0), 1000, "loaded straight away");
    assert!(m.speaker_out(), "data on, gate off: the speaker is out");
    assert_eq!(m.peek(io::SPEAKER), 0x22);
}

#[test]
fn with_speaker_data_off_the_timer_runs_but_the_speaker_stays_in() {
    let mut m = with_program(&tone(500, 0b01)); // gate on, data off
    run_to(&mut m, 0x0800 + 15);
    for _ in 0..3 {
        m.run_frame();
    }
    let t = m.mem.io.timer;
    let now = m.cpu.cycles;
    assert!((now..now + 1000).any(|c| !t.out(c)), "the timer is running");
    let mut buf = vec![1.0; 882];
    m.audio_frame(44_100, &mut buf);
    assert!(buf.iter().all(|&s| s == 0.0), "and the speaker is silent");
    assert_eq!(m.peek(io::SPEAKER) & 0x03, 0x01);
}

#[test]
fn turning_the_gate_on_starts_a_new_wave_high_half_first() {
    let mut t = Timer::default();
    t.lo = 100;
    t.commit(0);
    t.set_gate(0, true);
    assert!(!t.out(1050), "mid low half");
    t.set_gate(1050, false);
    t.set_gate(1077, true);
    assert!(t.out(1077) && t.out(1126) && !t.out(1127), "restarted at 1077");
    t.set_gate(1080, true);
    assert!(t.out(1126) && !t.out(1127), "setting it again doesn't restart it");
}

#[test]
fn a_new_divisor_takes_over_at_the_end_of_the_half_wave() {
    let mut t = Timer::default();
    t.lo = 100;
    t.commit(0);
    t.set_gate(0, true);
    t.lo = 40;
    t.commit(20); // in the high half (0-49) of the first wave
    assert!(t.out(49) && !t.out(50), "the old high half finishes");
    assert!(!t.out(69) && t.out(70), "then a low half of the new divisor: 20 cycles");
    assert!(t.out(89) && !t.out(90), "and new waves of 40 from there");
    assert_eq!(t.high(0, 70), 50);
    assert_eq!(t.high(70, 70 + 400), 200);
    assert_eq!(t.divisor(49), 100);
    assert_eq!(t.divisor(50), 40);
    t.lo = 10;
    t.commit(95); // in the low half (90-109) of a 40 wave: takes over at the rise
    assert!(!t.out(109) && t.out(110) && t.out(114) && !t.out(115) && t.out(120));
}

#[test]
fn turning_the_gate_off_loads_a_waiting_divisor() {
    let mut t = Timer::default();
    t.lo = 100;
    t.commit(0);
    t.set_gate(0, true);
    t.lo = 30;
    t.commit(10);
    t.set_gate(20, false);
    assert_eq!(t.divisor(20), 30);
    t.set_gate(25, true);
    assert!(t.out(39) && !t.out(40) && t.out(55));
}

#[test]
fn timer_registers_read_back_and_the_low_byte_alone_changes_nothing() {
    let mut m = Machine::new(RamSize::K16);
    m.poke(io::TIMER_LO, 0x34);
    assert_eq!(m.mem.io.timer.divisor(0), 65536, "TIMER_LO waits for TIMER_HI");
    m.poke(io::TIMER_HI, 0x12);
    assert_eq!((m.peek(io::TIMER_LO), m.peek(io::TIMER_HI)), (0x34, 0x12));
    assert_eq!(m.mem.io.timer.divisor(0), 0x1234);
    m.poke(io::SPEAKER, 0xFF);
    assert_eq!(m.peek(io::SPEAKER), 0x23, "bits 0, 1 and 5; the rest read 0");
    assert_eq!(m.peek(io::BEEPER), 0xFF, "BEEPER is write-only");
}

#[test]
fn beeper_flips_speaker_data_and_with_the_gate_off_drives_the_speaker() {
    let mut m = Machine::new(RamSize::K16);
    assert!(!m.speaker_out(), "in at power-on");
    assert_eq!(m.peek(io::SPEAKER), 0x20, "gate off: output high, data off");
    m.poke(io::BEEPER, 0);
    assert!(m.speaker_out());
    assert_eq!(m.peek(io::SPEAKER), 0x22, "the flip is SPEAKER bit 1");
    m.poke(io::BEEPER, 0x55);
    assert!(!m.speaker_out());
    assert_eq!(m.mem.io.speaker_toggles, 2);
    // With the timer playing, a flip turns data off: the tone stops, as on the PC.
    m.poke(io::TIMER_LO, 0xE8);
    m.poke(io::TIMER_HI, 0x03);
    m.poke(io::SPEAKER, 0b11);
    m.poke(io::BEEPER, 0);
    assert_eq!(m.peek(io::SPEAKER) & 0x03, 0x01);
    assert!(!m.speaker_out());
}

#[test]
fn speaker_out_bit_5_can_be_polled_cycle_exactly() {
    // Count the rising edges of a divisor-64 wave seen by a polling loop over 10 frames:
    //   loop: LDA SPEAKER / AND #$20 / CMP $10 / BEQ loop / STA $10 / INC $11 / JMP loop
    let mut code = tone(64, 0b01);
    let lp = 0x0800 + code.len() as u16;
    code.extend([op(Op::Lda, Mode::Abs), 0x31, 0xC0, op(Op::And, Mode::Imm), 0x20]);
    code.extend([op(Op::Cmp, Mode::Fp), 0x10, op(Op::Beq, Mode::Rel), (-9i8) as u8]);
    code.extend([op(Op::Sta, Mode::Fp), 0x10, op(Op::Inc, Mode::Fp), 0x11]);
    code.extend([op(Op::Jmp, Mode::Abs), lp as u8, (lp >> 8) as u8]);
    let mut m = with_program(&code);
    m.run_cycles(64 * 100);
    let changes = m.peek(0x11);
    assert!((195..=201).contains(&changes), "two changes a wave: {changes}");
    let mut a = m.clone();
    m.run_cycles(5000);
    a.run_cycles(5000);
    assert_eq!(m.peek(0x11), a.peek(0x11), "and the same every time");
}

// ---------- the sound the hosts play ----------

#[test]
fn a_silent_machine_renders_exact_silence() {
    let mut m = Machine::boot(RamSize::K16); // past the switch-on beep
    audio(&mut m, 10, 882); // the beep's tail fading out
    let s = audio(&mut m, 20, 882);
    assert!(s.iter().all(|&v| v == 0.0));
}

#[test]
fn at_switch_on_the_rom_beeps_880_hz_for_300_ms_as_ready_appears() {
    let mut m = Machine::new(RamSize::K16);
    m.reset();
    let s = audio(&mut m, 30, 882);
    let on: Vec<usize> = (0..s.len()).filter(|&i| s[i].abs() > AUDIO_VOLUME * 0.3).collect();
    let (a, b) = (on[0], *on.last().unwrap());
    let per_sample = CYCLES_PER_FRAME as f64 / 1e6 / 882.0;
    let secs = (b - a) as f64 * per_sample;
    assert!((secs - 0.3).abs() < 0.025, "beeps for {secs} s");
    let mid = &s[a + 2000..b - 2000]; // clear of the onset and the tail
    let hz = zero_crossings(mid) as f64 / 2.0 / (mid.len() as f64 * per_sample);
    assert!((hz - 880.3).abs() < 6.0, "{hz} Hz");
    // and leaves the speaker as it found it: gate off, data off
    assert_eq!(m.peek(io::SPEAKER) & 0b11, 0);
}

#[test]
fn the_audio_has_the_timers_frequency() {
    let mut m = with_program(&tone(2273, 0b11)); // 1,000,000 / 2273 = 439.95 Hz
    m.run_frame();
    m.audio_frame(44_100, &mut [0.0; 882]);
    let s = audio(&mut m, 50, 882);
    let secs = 50.0 * CYCLES_PER_FRAME as f64 / 1e6;
    let hz = zero_crossings(&s) as f64 / 2.0 / secs;
    assert!((hz - 439.95).abs() < 1.5, "{hz} Hz");
    let settled = &s[882 * 10..]; // once the onset's step has died away
    let peak = settled.iter().fold(0.0f32, |a, &v| a.max(v.abs()));
    assert!(peak > AUDIO_VOLUME * 0.4 && peak <= AUDIO_VOLUME * 0.6, "modest: peak {peak}");
    let mean = settled.iter().sum::<f32>() / settled.len() as f32;
    assert!(mean.abs() < 0.01, "centred: mean {mean}");
}

#[test]
fn the_audio_spreads_a_frame_over_any_number_of_samples() {
    for per in [800, 881, 958, 1000] {
        let mut m = with_program(&tone(1000, 0b11)); // 1 kHz
        m.run_frame();
        let s = audio(&mut m, 25, per);
        assert_eq!(s.len(), 25 * per);
        let hz = zero_crossings(&s) as f64 / 2.0 / (25.0 * CYCLES_PER_FRAME as f64 / 1e6);
        assert!((hz - 1000.0).abs() < 3.0, "{per} samples a frame: {hz} Hz");
    }
}

#[test]
fn a_bit_banged_tone_shows_in_the_audio_too() {
    // The chapter 12 "hard way": flip BEEPER every 1,138 cycles, 439.4 Hz.
    //   tone: STA BEEPER / LDX #226 / wait: DEX / BNE wait / JMP tone
    let code = [
        op(Op::Sta, Mode::Abs), 0x30, 0xC0, op(Op::Ldx, Mode::Imm), 226,
        op(Op::Dex, Mode::Imp), op(Op::Bne, Mode::Rel), (-3i8) as u8,
        op(Op::Jmp, Mode::Abs), 0x00, 0x08,
    ];
    let mut m = with_program(&code);
    m.run_frame();
    let s = audio(&mut m, 50, 882);
    let hz = zero_crossings(&s) as f64 / 2.0 / (50.0 * CYCLES_PER_FRAME as f64 / 1e6);
    assert!((hz - 439.4).abs() < 1.5, "{hz} Hz");
}

#[test]
fn a_stopped_tone_and_a_speaker_left_out_both_fade_to_exact_silence() {
    for ctl in [0b00, 0b10] {
        let mut m = with_program(&tone(1500, 0b11));
        m.run_frame();
        audio(&mut m, 5, 882);
        m.poke(io::SPEAKER, ctl);
        let s = audio(&mut m, 30, 882);
        assert!(s[..882].iter().any(|&v| v != 0.0));
        assert!(s[882 * 20..].iter().all(|&v| v == 0.0), "SPEAKER = {ctl}: silent after 20 frames");
        let step = s.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0, f32::max);
        assert!(step <= AUDIO_VOLUME, "no jump bigger than the speaker's own travel");
    }
}

#[test]
fn audio_is_deterministic_and_changes_mid_frame_land_on_their_cycle() {
    let run = |late: bool| {
        let mut m = with_program(&tone(800, 0b11));
        m.run_frame();
        m.run_cycles(if late { 10_000 } else { 2_000 });
        m.poke(io::SPEAKER, 0);
        m.run_frame();
        let mut s = vec![0.0; 882];
        m.audio_frame(44_100, &mut s);
        s
    };
    assert_eq!(run(false), run(false));
    let (early, late) = (run(false), run(true));
    let last_sound = |s: &[f32]| s.iter().rposition(|&v| v.abs() > AUDIO_VOLUME * 0.3).unwrap_or(0);
    assert!(last_sound(&late) > last_sound(&early) + 200, "the tone stops later in the frame");
}

#[test]
fn samples_per_frame_is_the_frames_length_in_samples() {
    assert!((Machine::samples_per_frame(44_100) - 880.6).abs() < 0.1);
    assert!((Machine::samples_per_frame(48_000) - 958.5).abs() < 0.1);
}

// ---------- NOISE ----------

#[test]
fn noise_depends_on_the_cycle_and_nothing_else() {
    let mut m = Machine::new(RamSize::K16);
    assert_eq!(m.peek(io::NOISE), m.peek(io::NOISE), "peek has no side effect");
    assert_eq!(m.mem.read(io::NOISE), noise(0));
    m.poke(io::NOISE, 7);
    assert_eq!(m.peek(io::NOISE), noise(0), "writes are ignored");
    let distinct: std::collections::HashSet<u8> = (0..4096u64).map(noise).collect();
    assert!(distinct.len() > 250, "spread over the bytes: {}", distinct.len());
    let ones: u32 = (0..8192u64).map(|c| noise(c).count_ones()).sum();
    assert!((ones as f64 / 8192.0 - 4.0).abs() < 0.1, "about half the bits set");
    let neighbours = (0..1000u64).filter(|&c| noise(c) == noise(c + 1)).count();
    assert!(neighbours < 15, "consecutive cycles differ: {neighbours} repeats");
}

#[test]
fn noise_read_by_a_program_is_deterministic_and_timing_dependent() {
    // Read NOISE 64 times into $3000-$303F:
    //   LDX #0 / [NOP] / loop: LDA NOISE / STA $3000,X / INX / CPX #64 / BNE loop
    let reads = |extra_nop: bool| {
        let mut code = vec![op(Op::Ldx, Mode::Imm), 0];
        if extra_nop {
            code.push(op(Op::Nop, Mode::Imp));
        }
        code.extend([op(Op::Lda, Mode::Abs), 0x34, 0xC0, op(Op::Sta, Mode::Absx), 0x00, 0x30]);
        code.extend([op(Op::Inx, Mode::Imp), op(Op::Cpx, Mode::Imm), 64, op(Op::Bne, Mode::Rel), (-11i8) as u8]);
        let mut m = with_program(&code);
        m.run_cycles(5000);
        m.mem.ram[0x3000..0x3040].to_vec()
    };
    assert_eq!(reads(false), reads(false), "same program, same bytes");
    assert_ne!(reads(false), reads(true), "two cycles later, different bytes");
    let mut sorted = reads(false);
    sorted.dedup();
    assert!(sorted.len() > 50, "not a constant");
}

// ---------- the shifter ----------

#[test]
fn shift_out_is_a_sprite_byte_moved_right_by_every_amount() {
    let mut m = Machine::new(RamSize::K16);
    for amount in 0..8u8 {
        m.poke(io::SHIFT_AMOUNT, amount);
        assert_eq!(m.peek(io::SHIFT_AMOUNT), amount);
        for s in 0..=255u8 {
            m.poke(io::SHIFT_IN, s);
            assert_eq!(m.peek(io::SHIFT_OUT), s >> amount, "the first byte: S >> {amount}");
            m.poke(io::SHIFT_IN, 0);
            let spill = ((s as u16) << (8 - amount)) as u8;
            assert_eq!(m.peek(io::SHIFT_OUT), spill, "the next byte: what spilled out");
        }
    }
}

#[test]
fn the_shifter_window_joins_two_bytes_and_flip_mirrors() {
    let mut sh = Shifter::default();
    for (a, b) in [(0xFFu8, 0x00u8), (0x81, 0x7E), (0x12, 0x34), (0x00, 0xFF)] {
        sh.push(a);
        sh.push(b);
        for k in 0..8 {
            sh.amount = k;
            assert_eq!(sh.out(), ((((a as u16) << 8) | b as u16) >> k) as u8);
        }
    }
    let mut m = Machine::new(RamSize::K16);
    m.poke(io::SHIFT_AMOUNT, 0);
    m.poke(io::SHIFT_FLIP, 0b1100_0001);
    assert_eq!(m.peek(io::SHIFT_OUT), 0b1000_0011);
    m.poke(io::SHIFT_AMOUNT, 0xFB);
    assert_eq!(m.peek(io::SHIFT_AMOUNT), 3, "only bits 0-2 count");
    assert_eq!((m.peek(io::SHIFT_IN), m.peek(io::SHIFT_FLIP)), (0xFF, 0xFF), "write-only");
    m.poke(io::SHIFT_OUT, 0);
    assert_eq!(m.peek(io::SHIFT_OUT), 0b1000_0011 >> 3, "read-only");
}

// ---------- the keyboard matrix ----------

/// Every key that is in the matrix.
fn matrix_keys() -> Vec<u8> {
    (0x20..=0x5F).chain([28, 29, 30, 31, 13, 8, 20, 3]).collect()
}

fn rows(m: &Machine) -> Vec<u8> {
    (0..9).map(|r| m.peek(io::KEYS + r)).collect()
}

#[test]
fn each_key_lights_one_bit_of_the_matrix_while_it_is_down() {
    let mut m = Machine::new(RamSize::K16);
    for code in matrix_keys() {
        m.key(code);
        let r = rows(&m);
        assert_eq!(r.iter().map(|b| b.count_ones()).sum::<u32>(), 1, "one bit for {code}");
        let (row, bit) = key_position(code).unwrap();
        assert_eq!(r[row], bit);
        if code >= 0x20 {
            assert_eq!((row, bit), (((code - 0x20) >> 3) as usize, 1 << (code & 7)));
        }
        m.key_up(code);
        assert_eq!(rows(&m), vec![0; 9], "released: {code}");
    }
    let row8 = |c: u8| key_position(c).unwrap();
    assert_eq!([row8(28), row8(29), row8(30), row8(31)], [(8, 1), (8, 2), (8, 4), (8, 8)]);
    assert_eq!([row8(13), row8(8), row8(20), row8(3)], [(8, 0x10), (8, 0x20), (8, 0x40), (8, 0x80)]);
    for code in [0u8, 1, 9, 27, 0x60, 0x7F] {
        m.key(code);
        assert_eq!(rows(&m), vec![0; 9], "{code} is not in the matrix");
        m.key_up(code);
    }
}

#[test]
fn many_keys_can_be_held_at_once_and_held_still_works() {
    let mut m = Machine::new(RamSize::K16);
    m.key(b'W');
    m.key(b'A');
    m.key(b' ');
    m.key(io::KEY_UP);
    assert_eq!(m.peek(io::KEYS + 6) & 0x80, 0x80, "W");
    assert_eq!(m.peek(io::KEYS + 4) & 0x02, 0x02, "A");
    assert_eq!(m.peek(io::KEYS), 0x01, "SPACE");
    assert_eq!(m.peek(io::KEYS + 8), 0x04, "UP");
    assert_eq!(m.peek(io::HELD), io::KEY_UP, "HELD: the latest");
    m.key_up(b'A');
    assert_eq!(m.peek(io::KEYS + 4), 0, "A released");
    assert_eq!(m.peek(io::KEYS + 6), 0x80, "W still down");
    m.key(b'A');
    m.key(b'A'); // a host's auto-repeat
    m.key_up(b'A');
    assert_eq!(m.peek(io::KEYS + 4), 0, "one release lets a repeated key go");
    let lots = b"QWERTYUIOP";
    assert!(lots.len() > HELD_KEYS);
    for &k in lots {
        m.key(k);
    }
    for &k in lots {
        let (row, bit) = key_position(k).unwrap();
        assert_eq!(m.peek(io::KEYS + row as u16) & bit, bit, "{} still down", k as char);
    }
    m.poke(io::KEYS, 0xFF);
    assert_eq!(m.peek(io::KEYS), 0x01, "read-only");
    m.mem.read(io::KBD_STROBE);
    assert_eq!(m.peek(io::KEYS), 0x01, "taking the key doesn't let it go");
}

#[test]
fn key_matrix_alone_leaves_kbd_and_held_alone() {
    let mut m = Machine::new(RamSize::K16);
    m.key_matrix(b'D', true);
    assert_eq!(m.peek(io::KEYS + 4), 0x10);
    assert_eq!((m.peek(io::KBD), m.peek(io::HELD)), (0, 0));
    m.key(b'D');
    m.key_up(b'D');
    assert_eq!(m.peek(io::KEYS + 4), 0);
    m.key(b'D');
    m.reset();
    assert_eq!(rows(&m), vec![0; 9], "a reset clears the matrix");
}
