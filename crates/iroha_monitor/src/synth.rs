//! Retro-inspired soft synth for the monitor's Etenraku theme.
//!
//! The implementation follows the design notes from
//! “Designing a Retro Synthesizer for Etenraku (Gagaku Melody)” and maps the
//! score to four voices: shō, hichiriki, ryūteki, and koto. The arrangement
//! keeps the full timing from `etenraku::synth_events` but renders it with
//! retro-synth techniques (additive pads, subtractive reeds, breathy flute,
//! and an FM plucked string).
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use std::f32::consts::{PI, TAU};

use crate::etenraku::{
    self, OrnamentMark, Ornaments, SequenceEvent, SequenceLayer, layer_intonation_cents,
};

const MASTER_GAIN: f32 = 0.22;
const TAIL_SECONDS: f32 = 3.5;
const PAN_SHO: f32 = 0.0;
const PAN_HICHIRIKI: f32 = -0.18;
const PAN_RYUTEKI: f32 = 0.18;
const PAN_KOTO: f32 = 0.1;

fn saturating_samples(value: f32) -> u32 {
    if !value.is_finite() {
        return 0;
    }
    value.clamp(0.0, u32::MAX as f32) as u32
}

/// Streaming synth state used by the CPAL callback.
pub struct SynthState {
    events: Vec<TimedEvent>,
    event_idx: usize,
    channels: usize,
    current_sample: u64,
    total_samples: u64,
    voices: RetroVoices,
    sample_rate: f32,
    timeline: etenraku::ScoreTimeline,
    finished: bool,
}

#[derive(Clone)]
struct TimedEvent {
    sample: u64,
    beat: f32,
    event: SequenceEvent,
}

#[derive(Clone, Copy)]
struct RenderContext {
    section_index: usize,
    hyoshi_phase: f32,
    bar_phase: f32,
    dist_to_obachi: f32,
}

impl RenderContext {
    fn new(_time_seconds: f32, beat: f32) -> Self {
        let hyoshi = (beat / etenraku::HYOSHI_BEATS).floor();
        let section_index = hyoshi.max(0.0) as usize;
        let hyoshi_phase = hyoshi.mul_add(-etenraku::HYOSHI_BEATS, beat);
        let bar_phase = beat.rem_euclid(4.0);
        let obachi = hyoshi.mul_add(etenraku::HYOSHI_BEATS, etenraku::OBACHI_OFFSET_BEATS);
        let dist_to_obachi = beat - obachi;
        Self {
            section_index,
            hyoshi_phase,
            bar_phase,
            dist_to_obachi,
        }
    }

    fn section_index(&self) -> usize {
        self.section_index.min(2)
    }

    fn dist_to_obachi(&self) -> f32 {
        self.dist_to_obachi
    }
}

impl SynthState {
    pub fn render_chunk(&mut self, dest: &mut [f32]) {
        if dest.is_empty() {
            return;
        }

        if self.finished {
            dest.fill(0.0);
            return;
        }

        let channels = self.channels.max(1);
        assert!(
            dest.len().is_multiple_of(channels),
            "dest buffer must be a multiple of the channel count"
        );
        dest.fill(0.0);

        let frames = dest.len() / channels;
        for frame_idx in 0..frames {
            // Dispatch due events.
            while self.event_idx < self.events.len()
                && self.events[self.event_idx].sample <= self.current_sample
            {
                let timed = &self.events[self.event_idx];
                let event = timed.event;
                if event.on {
                    self.voices.note_on(event, timed.beat);
                } else {
                    self.voices.note_off(event, timed.beat);
                }
                self.event_idx += 1;
            }

            let time_seconds = self.current_sample as f32 / self.sample_rate;
            let beat = self.timeline.beat_at(time_seconds);
            let context = RenderContext::new(time_seconds, beat);
            let (mut left, mut right) = self.voices.next_stereo_sample(context);
            left *= MASTER_GAIN;
            right *= MASTER_GAIN;

            let offset = frame_idx * channels;
            match channels {
                1 => dest[offset] = (left + right) * 0.5,
                2 => {
                    dest[offset] = left;
                    dest[offset + 1] = right;
                }
                _ => {
                    dest[offset] = left;
                    dest[offset + 1] = right;
                    let fold = (left + right) * 0.5;
                    for ch in 2..channels {
                        dest[offset + ch] = fold;
                    }
                }
            }

            self.current_sample = self.current_sample.saturating_add(1);
            if self.event_idx >= self.events.len()
                && self.voices.is_idle()
                && self.current_sample >= self.total_samples
            {
                self.finished = true;
                break;
            }
        }
    }
}

/// Build the streaming synth state for the monitor's builtin audio.
pub fn prepare(sample_rate: u32, channels: usize) -> SynthState {
    let (raw_events, timeline) = etenraku::synth_events();
    let sample_rate_f = sample_rate.max(1) as f32;

    let mut events: Vec<TimedEvent> = raw_events
        .into_iter()
        .filter(|event| {
            matches!(
                event.layer,
                SequenceLayer::Sho
                    | SequenceLayer::Hichiriki
                    | SequenceLayer::Ryuteki
                    | SequenceLayer::Koto
                    | SequenceLayer::Biwa
            )
        })
        .map(|event| {
            let mut sample = (event.t * sample_rate_f).round();
            if sample < 0.0 {
                sample = 0.0;
            }
            let sample = u64::try_from(sample as i64).unwrap_or(0);
            let beat = timeline.beat_at(event.t);
            TimedEvent {
                sample,
                beat,
                event,
            }
        })
        .collect();

    events.sort_by(|a, b| {
        a.sample
            .cmp(&b.sample)
            .then_with(|| b.event.on.cmp(&a.event.on))
            .then_with(|| a.event.note.cmp(&b.event.note))
    });

    let last_event_sample = events.iter().map(|ev| ev.sample).max().unwrap_or(0);
    let tail = ((timeline.total_seconds() + TAIL_SECONDS) * sample_rate_f).ceil();
    let tail_samples = u64::try_from(tail as i64).unwrap_or(0);
    let total_samples = last_event_sample.max(tail_samples);

    SynthState {
        events,
        event_idx: 0,
        channels: channels.max(1),
        current_sample: 0,
        total_samples,
        voices: RetroVoices::new(sample_rate_f),
        sample_rate: sample_rate_f,
        timeline,
        finished: false,
    }
}

struct RetroVoices {
    sho: ShoVoice,
    hichiriki: HichirikiVoice,
    ryuteki: RyutekiVoice,
    koto: KotoVoice,
}

impl RetroVoices {
    fn new(sample_rate: f32) -> Self {
        Self {
            sho: ShoVoice::new(sample_rate),
            hichiriki: HichirikiVoice::new(sample_rate),
            ryuteki: RyutekiVoice::new(sample_rate),
            koto: KotoVoice::new(sample_rate),
        }
    }

    fn note_on(&mut self, event: SequenceEvent, beat: f32) {
        match event.layer {
            SequenceLayer::Sho => self.sho.note_on(event.note, event.vel, beat),
            SequenceLayer::Hichiriki => {
                self.hichiriki
                    .note_on(event.note, event.vel, event.ornaments, beat)
            }
            SequenceLayer::Ryuteki => {
                self.ryuteki
                    .note_on(event.note, event.vel, event.ornaments, beat)
            }
            SequenceLayer::Koto | SequenceLayer::Biwa => {
                self.koto.note_on(event.note, event.vel, event.ornaments)
            }
            _ => {}
        }
    }

    fn note_off(&mut self, event: SequenceEvent, _beat: f32) {
        match event.layer {
            SequenceLayer::Sho => self.sho.note_off(event.note),
            SequenceLayer::Hichiriki => self.hichiriki.note_off(),
            SequenceLayer::Ryuteki => self.ryuteki.note_off(),
            SequenceLayer::Koto | SequenceLayer::Biwa => self.koto.note_off(),
            _ => {}
        }
    }

    fn next_stereo_sample(&mut self, ctx: RenderContext) -> (f32, f32) {
        let sho = self.sho.next_sample(ctx);
        let hichiriki = self.hichiriki.next_sample(ctx);
        let ryuteki = self.ryuteki.next_sample(ctx);
        let koto = self.koto.next_sample();

        let (mut left, mut right) = pan_sample(sho, PAN_SHO);
        let (hl, hr) = pan_sample(hichiriki, PAN_HICHIRIKI);
        let (rl, rr) = pan_sample(ryuteki, PAN_RYUTEKI);
        let (kl, kr) = pan_sample(koto, PAN_KOTO);

        left += hl + rl + kl;
        right += hr + rr + kr;
        (left, right)
    }

    fn is_idle(&self) -> bool {
        self.sho.is_idle()
            && self.hichiriki.is_idle()
            && self.ryuteki.is_idle()
            && self.koto.is_idle()
    }
}

struct ShoVoice {
    sample_rate: f32,
    notes: Vec<ShoOsc>,
    noise: Lcg32,
}

struct ShoOsc {
    note: u8,
    oscillator: Oscillator,
    envelope: Envelope,
    base_gain: f32,
    detune_ratio: f32,
    transition_env: f32,
}

impl ShoVoice {
    fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            notes: Vec::new(),
            noise: Lcg32::new(0x5A5A_1F1F),
        }
    }

    fn note_on(&mut self, note: u8, velocity: u8, beat: f32) {
        let freq = midi_note_to_freq(note, SequenceLayer::Sho);
        let gain = velocity_to_gain(velocity) * 0.65;
        let section = (beat / etenraku::HYOSHI_BEATS).floor() as usize;
        let attack = match section {
            0 => 0.26,
            1 => 0.21,
            _ => 0.18,
        };
        let release = 0.6;
        let detune_ratio = cents_to_ratio(sho_detune_cents(note));
        if let Some(existing) = self.notes.iter_mut().find(|osc| osc.note == note) {
            existing.base_gain = gain;
            existing
                .envelope
                .reset_with(self.sample_rate, attack, release);
            existing.envelope.trigger();
            existing.oscillator.set_freq(freq * existing.detune_ratio);
            existing.transition_env = 1.0;
            return;
        }

        let mut envelope = Envelope::with_times(self.sample_rate, attack, release);
        envelope.trigger();
        self.notes.push(ShoOsc {
            note,
            oscillator: Oscillator::new(freq * detune_ratio),
            envelope,
            base_gain: gain,
            detune_ratio,
            transition_env: 1.0,
        });
    }

    fn note_off(&mut self, note: u8) {
        if let Some(existing) = self.notes.iter_mut().find(|osc| osc.note == note) {
            existing.envelope.release();
        }
    }

    fn next_sample(&mut self, ctx: RenderContext) -> f32 {
        let section = ctx.section_index();
        let breath_gain = sho_breath_gain(&ctx);
        let shaped_breath = sho_breath_shape(breath_gain);
        let mut acc = 0.0;
        self.notes.retain_mut(|osc| {
            let env = osc.envelope.advance();
            if env <= f32::EPSILON && osc.envelope.is_idle() {
                return false;
            }
            let sample = osc.oscillator.sample(self.sample_rate);
            let mut amp = osc.base_gain * env * shaped_breath;
            let bloom = (breath_gain - 1.0).max(0.0);
            amp *= bloom.mul_add(0.35, 1.0);
            let mut value = sample * amp;
            if osc.transition_env > f32::EPSILON {
                let tic = self.noise.next() * 0.02 * osc.transition_env;
                value += tic;
                osc.transition_env *= 0.92;
            }
            acc += value;
            true
        });
        let section_halo = match section {
            0 => 0.38,
            1 => 0.4,
            _ => 0.42,
        };
        acc * section_halo
    }

    fn is_idle(&self) -> bool {
        self.notes.is_empty()
    }
}

struct HichirikiVoice {
    sample_rate: f32,
    oscillator: Oscillator,
    envelope: Envelope,
    base_gain: f32,
    breath_mix: f32,
    vibrato_phase: f32,
    vibrato_rate: f32,
    vibrato_depth_cents: f32,
    vibrato_delay_samples: u32,
    vibrato_counter: u32,
    glide_peak_cents: f32,
    glide_duration_samples: u32,
    glide_release_samples: u32,
    glide_counter: u32,
    breath_lag_samples: u32,
    breath_env: f32,
    current_note: u8,
    noise: Lcg32,
}

impl HichirikiVoice {
    fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            oscillator: Oscillator::new(440.0),
            envelope: Envelope::with_times(sample_rate, 0.02, 0.35),
            base_gain: 0.0,
            breath_mix: 0.1,
            vibrato_phase: 0.0,
            vibrato_rate: 5.1,
            vibrato_depth_cents: 12.0,
            vibrato_delay_samples: 0,
            vibrato_counter: 0,
            glide_peak_cents: 0.0,
            glide_duration_samples: 0,
            glide_release_samples: 0,
            glide_counter: 0,
            breath_lag_samples: 0,
            breath_env: 0.0,
            current_note: 0,
            noise: Lcg32::new(0x1357_2468),
        }
    }

    fn note_on(&mut self, note: u8, velocity: u8, ornaments: Ornaments, beat: f32) {
        self.current_note = note;
        let freq = midi_note_to_freq(note, SequenceLayer::Hichiriki);
        self.oscillator = Oscillator::new(freq);
        let attack = if ornaments.contains(OrnamentMark::Tataku) {
            0.008
        } else {
            0.02
        };
        let release = 0.32;
        self.envelope.reset_with(self.sample_rate, attack, release);
        self.envelope.trigger();

        let mut gain = velocity_to_gain(velocity) * 0.85;
        if ornaments.contains(OrnamentMark::Fukura) {
            gain *= 1.1;
        }
        self.base_gain = gain;

        self.breath_mix = if ornaments.contains(OrnamentMark::Fukura) {
            0.18
        } else {
            0.12
        };

        self.vibrato_rate = if ornaments.contains(OrnamentMark::Mawashi) {
            6.0
        } else {
            5.1
        };
        self.vibrato_depth_cents = if ornaments.contains(OrnamentMark::Mawashi) {
            18.0
        } else {
            12.0
        };
        self.vibrato_phase = 0.0;

        let base_delay: f32 = if ornaments.contains(OrnamentMark::Tataku) {
            0.18
        } else {
            0.62
        };
        let delay_samples = base_delay.mul_add(self.sample_rate, 0.0).round().max(0.0);
        self.vibrato_delay_samples = saturating_samples(delay_samples);
        self.vibrato_counter = 0;

        if ornaments.contains(OrnamentMark::Seme) {
            let peak = self.noise.next().mul_add(0.08, 0.30);
            self.glide_peak_cents = (peak * 100.0).clamp(20.0, 45.0);
            let up_ms = self.noise.next().abs().mul_add(0.04, 0.09);
            let rel_ms = self.noise.next().abs().mul_add(0.03, 0.08);
            self.glide_duration_samples =
                saturating_samples(up_ms.mul_add(self.sample_rate, 0.0).max(0.0));
            self.glide_release_samples =
                saturating_samples(rel_ms.mul_add(self.sample_rate, 0.0).max(0.0));
            self.glide_counter = 0;
        } else if ornaments.contains(OrnamentMark::Mawashi) {
            let sweep = self.noise.next().mul_add(0.06, 0.20);
            self.glide_peak_cents = (sweep * 100.0).clamp(-35.0, 35.0);
            let duration = 0.18_f32.mul_add(self.sample_rate, 0.0).max(0.0);
            let duration_samples = saturating_samples(duration);
            self.glide_duration_samples = duration_samples;
            self.glide_release_samples = duration_samples;
            self.glide_counter = 0;
        } else {
            self.glide_peak_cents = 0.0;
            self.glide_duration_samples = 0;
            self.glide_release_samples = 0;
            self.glide_counter = 0;
        }

        let lag = self.noise.next().abs().mul_add(0.03, 0.04);
        self.breath_lag_samples = saturating_samples(lag.mul_add(self.sample_rate, 0.0).max(0.0));
        self.breath_env = 0.0;

        self.noise
            .reseed((u32::from(note) << 16) ^ u32::from(velocity) ^ 0x9E37_79B9 ^ beat.to_bits());
    }

    fn note_off(&mut self) {
        self.envelope.release();
    }

    fn next_sample(&mut self, ctx: RenderContext) -> f32 {
        let env = self.envelope.advance();
        if env <= f32::EPSILON && self.envelope.is_idle() {
            return 0.0;
        }

        self.vibrato_counter = self.vibrato_counter.saturating_add(1);
        let obachi_lock = ctx.dist_to_obachi().abs() < 0.18;
        let mut cents_mod = 0.0;
        if self.glide_peak_cents.abs() > f32::EPSILON {
            if self.glide_counter <= self.glide_duration_samples {
                let dur = self.glide_duration_samples.max(1);
                let progress = (self.glide_counter as f32 / dur as f32).clamp(0.0, 1.0);
                let shaped = 1.0 - (-6.0 * progress).exp();
                cents_mod += self.glide_peak_cents * shaped;
            } else if self.glide_release_samples > 0
                && self.glide_counter <= self.glide_duration_samples + self.glide_release_samples
            {
                let rel = self.glide_release_samples.max(1);
                let idx = self.glide_counter - self.glide_duration_samples;
                let progress = (idx as f32 / rel as f32).clamp(0.0, 1.0);
                cents_mod += self.glide_peak_cents * (1.0 - progress);
            }
            self.glide_counter = self.glide_counter.saturating_add(1);
        }

        if !obachi_lock
            && self.vibrato_counter >= self.vibrato_delay_samples
            && self.vibrato_depth_cents > 0.0
        {
            cents_mod += (self.vibrato_phase.sin()) * self.vibrato_depth_cents;
            self.vibrato_phase =
                (self.vibrato_phase + TAU * self.vibrato_rate / self.sample_rate).rem_euclid(TAU);
        }

        match self.current_note % 12 {
            4 | 9 => {
                if obachi_lock {
                    cents_mod += 1.5;
                }
            }
            1 | 6 => {
                let phase = ctx.hyoshi_phase;
                if (7.0..=7.4).contains(&phase) {
                    cents_mod -= 2.0;
                } else if (7.7..=8.05).contains(&phase) {
                    cents_mod += 2.0;
                }
            }
            _ => {}
        }

        let ratio = cents_to_ratio(cents_mod);
        let freq = self.oscillator.freq * ratio;
        self.oscillator.set_freq(freq);
        let tone = self.oscillator.sample(self.sample_rate);

        let mut target_noise = self.breath_mix * env.mul_add(0.4, 0.6);
        if self.vibrato_counter < self.breath_lag_samples {
            target_noise *= 0.25;
        }
        self.breath_env = target_noise.mul_add(0.12, self.breath_env * 0.88);
        let breath = self.noise.next() * self.breath_env;

        let sample = tone.mul_add(self.breath_env.mul_add(-0.45, 1.0), breath);
        sample * self.base_gain * env
    }

    fn is_idle(&self) -> bool {
        self.envelope.is_idle()
    }
}

struct RyutekiVoice {
    sample_rate: f32,
    oscillator: Oscillator,
    envelope: Envelope,
    base_gain: f32,
    noise_env: f32,
    noise_decay: f32,
    noise_level: f32,
    vibrato_phase: f32,
    vibrato_rate: f32,
    vibrato_depth_cents: f32,
    vibrato_delay_samples: u32,
    vibrato_counter: u32,
    lip_phase: f32,
    lip_rate: f32,
    lip_depth_cents: f32,
    current_note: u8,
    noise: Lcg32,
}

impl RyutekiVoice {
    fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            oscillator: Oscillator::new(440.0),
            envelope: Envelope::with_times(sample_rate, 0.03, 0.26),
            base_gain: 0.0,
            noise_env: 0.0,
            noise_decay: 0.0,
            noise_level: 0.22,
            vibrato_phase: 0.0,
            vibrato_rate: 5.8,
            vibrato_depth_cents: 9.0,
            vibrato_delay_samples: 0,
            vibrato_counter: 0,
            lip_phase: 0.0,
            lip_rate: 0.3,
            lip_depth_cents: 4.0,
            current_note: 0,
            noise: Lcg32::new(0x2468_ACED),
        }
    }

    fn note_on(&mut self, note: u8, velocity: u8, ornaments: Ornaments, beat: f32) {
        self.current_note = note;
        let freq = midi_note_to_freq(note, SequenceLayer::Ryuteki);
        self.oscillator = Oscillator::new(freq);
        self.envelope.reset_with(self.sample_rate, 0.03, 0.22);
        self.envelope.trigger();

        self.base_gain = velocity_to_gain(velocity) * 0.7;
        self.noise_env = 1.0;
        self.noise_decay = exp_decay(self.sample_rate, 0.06);
        self.noise_level = if ornaments.contains(OrnamentMark::Fukura) {
            0.3
        } else {
            0.22
        };
        self.vibrato_rate = if ornaments.contains(OrnamentMark::Mawashi) {
            6.2
        } else {
            5.8
        };
        self.vibrato_depth_cents = if ornaments.contains(OrnamentMark::Mawashi) {
            14.0
        } else {
            9.0
        };
        self.vibrato_phase = 0.0;
        let hyoshi_phase = beat.rem_euclid(etenraku::HYOSHI_BEATS);
        let delay: f32 = if hyoshi_phase < 4.0 { 0.55 } else { 0.28 };
        let delay_samples = delay.mul_add(self.sample_rate, 0.0).round().max(0.0);
        self.vibrato_delay_samples = saturating_samples(delay_samples);
        self.vibrato_counter = 0;
        self.lip_rate = self.noise.next().abs().mul_add(0.20, 0.25);
        self.lip_depth_cents = self.noise.next().abs().mul_add(3.0, 3.0);
        self.lip_phase = 0.0;
        self.noise.reseed((u32::from(note) << 8) ^ 0xA5A5_1122);
    }

    fn note_off(&mut self) {
        self.envelope.release();
    }

    fn next_sample(&mut self, ctx: RenderContext) -> f32 {
        let env = self.envelope.advance();
        if env <= f32::EPSILON && self.envelope.is_idle() {
            return 0.0;
        }

        self.noise_env *= self.noise_decay;
        let noise_env = self.noise_env.clamp(0.0, 1.0);

        self.vibrato_counter = self.vibrato_counter.saturating_add(1);
        let obachi_lock = ctx.dist_to_obachi().abs() < 0.18;
        let mut cents_mod = 0.0;
        self.lip_phase = (self.lip_phase + TAU * self.lip_rate / self.sample_rate).rem_euclid(TAU);
        if !obachi_lock {
            cents_mod += self.lip_phase.sin() * self.lip_depth_cents;
        }

        if !obachi_lock
            && self.vibrato_counter >= self.vibrato_delay_samples
            && self.vibrato_depth_cents > 0.0
        {
            cents_mod += (self.vibrato_phase.sin()) * self.vibrato_depth_cents;
            self.vibrato_phase =
                (self.vibrato_phase + TAU * self.vibrato_rate / self.sample_rate).rem_euclid(TAU);
        }

        // Hyōshi-based intonation shading near obachi landing.
        let phase = ctx.hyoshi_phase;
        if (7.2..=8.0).contains(&phase) {
            cents_mod -= 2.0;
        } else if phase > 8.0 && phase < 9.0 {
            let blend = ((phase - 8.0) / 1.0).clamp(0.0, 1.0);
            cents_mod -= 2.0 * (1.0 - blend);
        }

        if matches!(self.current_note % 12, 1 | 6) && (7.0..=7.4).contains(&phase) {
            cents_mod -= 1.5;
        }

        let ratio = cents_to_ratio(cents_mod);
        let freq = self.oscillator.freq * ratio;
        self.oscillator.set_freq(freq);
        let tone = self.oscillator.sample(self.sample_rate);

        let breath_mix = noise_env.mul_add(0.6, 0.4);
        let env_mix = env.mul_add(0.5, 0.5);
        let breath = self.noise.next() * self.noise_level * breath_mix * env_mix;
        let sample = tone.mul_add(self.noise_level.mul_add(-0.4, 1.0), breath);

        sample * self.base_gain * env
    }

    fn is_idle(&self) -> bool {
        self.envelope.is_idle()
    }
}

struct KotoVoice {
    sample_rate: f32,
    active: bool,
    carrier_phase: f32,
    mod_phase: f32,
    carrier_freq: f32,
    mod_freq: f32,
    amplitude: f32,
    amp_decay: f32,
    mod_env: f32,
    mod_decay: f32,
    mod_index: f32,
    tail_threshold: f32,
    noise_burst: f32,
    noise: Lcg32,
}

impl KotoVoice {
    fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            active: false,
            carrier_phase: 0.0,
            mod_phase: 0.0,
            carrier_freq: 440.0,
            mod_freq: 880.0,
            amplitude: 0.0,
            amp_decay: exp_decay(sample_rate, 1.4),
            mod_env: 0.0,
            mod_decay: exp_decay(sample_rate, 0.4),
            mod_index: 5.0,
            tail_threshold: 1.0e-4,
            noise_burst: 0.0,
            noise: Lcg32::new(0xBEEF_CAFE),
        }
    }

    fn note_on(&mut self, note: u8, velocity: u8, ornaments: Ornaments) {
        self.carrier_freq = midi_note_to_freq(note, SequenceLayer::Koto);
        self.mod_freq = self.carrier_freq * 2.0;
        self.carrier_phase = 0.0;
        self.mod_phase = 0.0;
        self.amplitude = velocity_to_gain(velocity) * 0.9;
        self.amp_decay = exp_decay(self.sample_rate, 0.5);
        self.mod_env = 1.0;
        self.mod_decay = exp_decay(self.sample_rate, 0.5);
        let base_index = if ornaments.contains(OrnamentMark::Tataku) {
            6.0
        } else {
            5.0
        };
        let brightness = self.noise.next().mul_add(0.12, 1.0);
        self.mod_index = base_index * brightness;
        self.tail_threshold = 5.0e-3;
        self.noise_burst = 1.0;
        self.active = true;
        self.noise
            .reseed((u32::from(note) << 4) ^ u32::from(velocity) ^ 0xCAFE_BABE);
    }

    fn note_off(&mut self) {
        self.amplitude *= 0.35;
        self.mod_env *= 0.35;
    }

    fn next_sample(&mut self) -> f32 {
        if !self.active {
            return 0.0;
        }

        let mod_signal =
            if self.mod_phase < PI { 1.0 } else { -1.0 } * self.mod_index * self.mod_env;
        let base = self.carrier_phase + mod_signal;
        let primary = if base.rem_euclid(TAU) < PI { 1.0 } else { -1.0 };
        let duplex = if (base * 1.98).rem_euclid(TAU) < PI {
            0.12
        } else {
            -0.12
        };

        let noise_jitter = self.noise.next();
        let freq_step = self
            .carrier_freq
            .mul_add(TAU / self.sample_rate, self.carrier_phase);
        self.carrier_phase = wrap_phase(noise_jitter.mul_add(0.0003, freq_step));
        self.mod_phase = wrap_phase(
            self.mod_freq
                .mul_add(TAU / self.sample_rate, self.mod_phase),
        );

        self.amplitude *= self.amp_decay;
        self.mod_env *= self.mod_decay;
        if self.amplitude < self.tail_threshold {
            self.active = false;
            self.noise_burst = 0.0;
            return 0.0;
        }
        let mut sample = (primary + duplex) * self.amplitude;
        if self.noise_burst > 1.0e-3 {
            sample += self.noise.next() * 0.045 * self.noise_burst;
            self.noise_burst *= 0.86;
        }
        sample
    }

    fn is_idle(&self) -> bool {
        !self.active
    }
}

struct Oscillator {
    phase: f32,
    freq: f32,
}

impl Oscillator {
    fn new(freq: f32) -> Self {
        Self {
            phase: 0.0,
            freq: freq.max(1.0),
        }
    }

    fn set_freq(&mut self, freq: f32) {
        self.freq = freq.max(1.0);
    }

    fn sample(&mut self, sample_rate: f32) -> f32 {
        self.phase = wrap_phase(self.phase + TAU * self.freq / sample_rate);
        if self.phase < PI { 1.0 } else { -1.0 }
    }
}

struct Envelope {
    level: f32,
    attack_step: f32,
    release_step: f32,
    state: EnvelopeState,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EnvelopeState {
    Idle,
    Attack,
    Sustain,
    Release,
}

impl Envelope {
    fn with_times(sample_rate: f32, attack_seconds: f32, release_seconds: f32) -> Self {
        let mut env = Self {
            level: 0.0,
            attack_step: step_for(sample_rate, attack_seconds),
            release_step: step_for(sample_rate, release_seconds),
            state: EnvelopeState::Idle,
        };
        if env.attack_step >= 1.0 {
            env.attack_step = 1.0;
        }
        env
    }

    fn reset_with(&mut self, sample_rate: f32, attack_seconds: f32, release_seconds: f32) {
        self.level = 0.0;
        self.attack_step = step_for(sample_rate, attack_seconds).min(1.0);
        self.release_step = step_for(sample_rate, release_seconds).min(1.0);
        self.state = EnvelopeState::Idle;
    }

    fn trigger(&mut self) {
        self.state = EnvelopeState::Attack;
    }

    fn release(&mut self) {
        if self.state != EnvelopeState::Idle {
            self.state = EnvelopeState::Release;
        }
    }

    fn advance(&mut self) -> f32 {
        match self.state {
            EnvelopeState::Idle => {
                self.level = 0.0;
            }
            EnvelopeState::Attack => {
                self.level = (self.level + self.attack_step).min(1.0);
                if self.level >= 0.999 {
                    self.level = 1.0;
                    self.state = EnvelopeState::Sustain;
                }
            }
            EnvelopeState::Sustain => {}
            EnvelopeState::Release => {
                self.level = (self.level - self.release_step).max(0.0);
                if self.level <= 1.0e-4 {
                    self.level = 0.0;
                    self.state = EnvelopeState::Idle;
                }
            }
        }
        self.level
    }

    fn is_idle(&self) -> bool {
        matches!(self.state, EnvelopeState::Idle)
    }
}

#[derive(Clone)]
struct Lcg32(u32);

impl Lcg32 {
    fn new(seed: u32) -> Self {
        let seed = if seed == 0 { 1 } else { seed };
        Self(seed)
    }

    fn reseed(&mut self, seed: u32) {
        self.0 = if seed == 0 { 1 } else { seed };
    }

    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let bits = (self.0 >> 9) | 0x3F80_0000;
        f32::from_bits(bits) - 1.5
    }
}

fn midi_note_to_freq(note: u8, layer: SequenceLayer) -> f32 {
    let base = 440.0 * ((f32::from(note) - 69.0) / 12.0).exp2();
    let cents = layer_intonation_cents(layer, note);
    base * cents_to_ratio(cents)
}

fn cents_to_ratio(cents: f32) -> f32 {
    (cents / 1200.0).exp2()
}

fn velocity_to_gain(vel: u8) -> f32 {
    let norm = f32::from(vel).clamp(0.0, 127.0) / 127.0;
    norm.powf(1.35)
}

fn pan_sample(sample: f32, pan: f32) -> (f32, f32) {
    let pan = pan.clamp(-1.0, 1.0);
    let left = sample * (0.5 * (1.0 - pan));
    let right = sample * (0.5 * (1.0 + pan));
    (left, right)
}

fn step_for(sample_rate: f32, seconds: f32) -> f32 {
    if seconds <= 0.0 {
        1.0
    } else {
        1.0 / (sample_rate * seconds)
    }
}

fn exp_decay(sample_rate: f32, seconds: f32) -> f32 {
    if seconds <= 0.0 {
        0.0
    } else {
        (-1.0 / (sample_rate * seconds)).exp()
    }
}

fn wrap_phase(phase: f32) -> f32 {
    let wrapped = phase % TAU;
    if wrapped < 0.0 {
        wrapped + TAU
    } else {
        wrapped
    }
}

fn sho_detune_cents(note: u8) -> f32 {
    match note {
        69 => 1.2,  // A4
        71 => -0.9, // B4
        73 => 0.9,  // C#5 / Db5
        78 => -0.8, // F#5
        79 => 0.7,  // G5
        81 => -1.4, // A5
        83 => 0.6,  // B5
        85 => -0.6, // C#6
        86 => 0.4,  // D6
        88 => -0.2, // E6
        90 => 0.3,  // F#6 / G6 depending on aitake
        _ => 0.0,
    }
}

fn sho_breath_gain(ctx: &RenderContext) -> f32 {
    let section_scale = match ctx.section_index() {
        0 => 1.0,
        1 => 1.035,
        _ => 1.065,
    };
    let beat = ctx.bar_phase;
    let base = if beat < 1.0 {
        0.08f32.mul_add(beat, 0.92)
    } else if beat < 3.0 {
        let progress = (beat - 1.0) / 2.0;
        progress.mul_add(0.07, 0.95)
    } else {
        let tail = (beat - 3.0).clamp(0.0, 1.0);
        tail.mul_add(-0.14, 1.02)
    };
    (base * section_scale).clamp(0.6, 1.5)
}

fn sho_breath_shape(raw: f32) -> f32 {
    let clamped = raw.clamp(0.6, 1.4);
    let t = ((clamped - 0.6) / 0.8).clamp(0.0, 1.0);
    let cosine = 0.5f32.mul_add(-(PI * t).cos(), 0.5);
    let exp = 1.0 - (-3.0 * t).exp();
    exp.mul_add(0.15, cosine.mul_add(0.3, clamped * 0.55))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_context(sample_idx: usize, sample_rate: f32) -> RenderContext {
        let time = sample_idx as f32 / sample_rate;
        // Approximate beat progression around 54 BPM for testing.
        let beat = time * (54.0 / 60.0);
        RenderContext::new(time, beat)
    }

    #[test]
    fn sho_voice_attack_and_release() {
        let mut sho = ShoVoice::new(48_000.0);
        sho.note_on(64, 96, 0.0);
        let mut max: f32 = 0.0;
        for idx in 0..10_000 {
            let ctx = sample_context(idx, 48_000.0);
            max = max.max(sho.next_sample(ctx).abs());
        }
        assert!(max > 0.001, "sho should emit audio after attack");
        sho.note_off(64);
        for idx in 0..40_000 {
            let ctx = sample_context(10_000 + idx, 48_000.0);
            sho.next_sample(ctx);
        }
        assert!(sho.is_idle(), "sho should release to silence");
    }

    #[test]
    fn hichiriki_noise_blend() {
        let mut voice = HichirikiVoice::new(48_000.0);
        voice.note_on(69, 112, Ornaments::default(), 0.0);
        let mut sum = 0.0;
        for idx in 0..2_000 {
            let ctx = sample_context(idx, 48_000.0);
            sum += voice.next_sample(ctx).abs();
        }
        assert!(sum > 0.01, "hichiriki should generate energy");
        voice.note_off();
        for idx in 0..20_000 {
            let ctx = sample_context(2_000 + idx, 48_000.0);
            voice.next_sample(ctx);
        }
        assert!(voice.is_idle(), "hichiriki envelope should finish");
    }

    #[test]
    fn koto_decay_stops_voice() {
        let mut voice = KotoVoice::new(48_000.0);
        voice.note_on(76, 110, Ornaments::default());
        let mut max: f32 = 0.0;
        for _ in 0..10_000 {
            max = max.max(voice.next_sample().abs());
        }
        assert!(max > 0.001, "koto should emit at start");
        for _ in 0..120_000 {
            voice.next_sample();
        }
        assert!(voice.is_idle(), "koto should decay to silence");
    }

    #[test]
    fn sho_detune_table_balances() {
        // Sum detune offsets for a full aitake chord and ensure the drift is near zero.
        let chord = [69, 81, 83, 85, 86, 88];
        let sum: f32 = chord.iter().map(|&n| sho_detune_cents(n)).sum();
        assert!(sum.abs() < 0.5, "detune offsets should balance, got {sum}");
    }
}
