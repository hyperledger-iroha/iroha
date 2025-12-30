#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

//! Etenraku scheduling derived from “MIDI synth design in Rust”.
//!
//! The original design document focused on modelling the hichiriki, ryūteki, and shō parts of
//! Etenraku using a lightweight Rust soft-synth.  This module adapts that score for the monitor by
//! providing timed events, MIDI export helpers, and small helpers that keep the ASCII prologue in
//! sync with the audio rendition.

use std::{
    cmp::Ordering,
    f32::consts::{LN_2, PI},
};

use eyre::Result;

pub const HYOSHI_BEATS: f32 = 16.0;
pub const OBACHI_OFFSET_BEATS: f32 = HYOSHI_BEATS * 0.5;
#[allow(dead_code)]
pub const PRELUDE_BEATS: f32 = 0.0;

const TOTAL_SECTIONS: usize = 3;
const BEATS_PER_SECTION: f32 = HYOSHI_BEATS;
const TOTAL_BEATS: f32 = BEATS_PER_SECTION * TOTAL_SECTIONS as f32;
const BASE_SECTION_BPM: [f32; TOTAL_SECTIONS] = [52.4, 54.6, 56.2];
const SECTION_WANDER_BPM: [f32; TOTAL_SECTIONS] = [0.04, 0.06, 0.08];
const TEMPO_SUBDIVISIONS_PER_BEAT: usize = 128;
const MIDI_TICKS_PER_BEAT: u16 = 480;
const A4_TUNING_HZ: f32 = 430.0;
const APPROX_SECONDS_PER_BEAT: f32 = 74.0 / TOTAL_BEATS;

const HICHIRIKI_EVENTS: &[(f32, f32, &str)] = &[
    // Section 1 (Jo)
    (0.0, 2.0, "B4"),
    (2.0, 1.0, "A4"),
    (3.0, 1.0, "B4"),
    (4.0, 2.0, "E5"),
    (6.0, 1.0, "E5"),
    (7.0, 1.0, "B4"),
    (8.0, 2.0, "A4"),
    (10.0, 2.0, "E4"),
    (12.0, 2.0, "B4"),
    (14.0, 1.0, "A4"),
    (15.0, 1.0, "B4"),
    // Section 2 (Ha)
    (16.0, 2.0, "E5"),
    (18.0, 1.0, "D5"),
    (19.0, 1.0, "E5"),
    (20.0, 2.0, "B4"),
    (22.0, 1.0, "A4"),
    (23.0, 1.0, "B4"),
    (24.0, 2.0, "A4"),
    (26.0, 2.0, "E4"),
    (28.0, 2.0, "B4"),
    (30.0, 1.0, "A4"),
    (31.0, 1.0, "B4"),
    // Section 3 (Kyū)
    (32.0, 2.0, "A4"),
    (34.0, 1.0, "E5"),
    (35.0, 1.0, "D5"),
    (36.0, 2.0, "B4"),
    (38.0, 1.0, "A4"),
    (39.0, 1.0, "B4"),
    (40.0, 2.0, "A4"),
    (42.0, 2.0, "E4"),
    (44.0, 2.0, "B4"),
    (46.0, 1.0, "A4"),
    (47.0, 1.0, "B4"),
];

const SHO_CHORD_PROGRESSION: &[(f32, f32, &str)] = &[
    // Jo
    (0.0, 16.0, "Kotsu"),
    // Ha
    (16.0, 4.0, "Ichikotsu"),
    (20.0, 12.0, "Kotsu"),
    // Kyū
    (32.0, 4.0, "Kotsu"),
    (36.0, 4.0, "Ichikotsu"),
    (40.0, 8.0, "Kotsu"),
];

const KOTO_ARPEGGIOS: &[(f32, f32, &[&str])] = &[
    (0.5, 6.5, &["A3", "E4", "A4", "C#5"]),
    (8.5, 5.5, &["B3", "F#4", "B4", "D5"]),
    (16.75, 5.5, &["C#4", "G#4", "C#5", "E5"]),
    (24.5, 5.5, &["B3", "F#4", "B4", "D5"]),
    (32.25, 6.0, &["A3", "E4", "A4", "C#5"]),
    (40.0, 6.0, &["E3", "B3", "E4", "A4"]),
    (46.0, 4.0, &["A3", "C#4", "E4", "A4"]),
];

const BIWA_SWELLS: &[(f32, f32, &str, bool)] = &[
    (1.0, 2.5, "E3", true),
    (9.0, 2.0, "F#3", false),
    (17.0, 2.5, "A3", true),
    (25.0, 2.25, "G#3", false),
    (33.0, 2.25, "F#3", false),
    (41.0, 3.0, "E3", true),
    (47.0, 2.5, "A3", true),
];

const TAIKO_ACCENTS: &[(f32, u8)] = &[
    // Jo
    (10.0, 108),
    (12.0, 122),
    // Ha
    (26.0, 108),
    (28.0, 122),
    // Kyū
    (42.0, 110),
    (44.0, 124),
];

const SHOKO_STRIKES: &[f32] = &[13.0, 29.0, 45.0];

const TAIKO_NOTE: u8 = 48;
const SHOKO_NOTE: u8 = 81;
const KAKKO_HIGH_NOTE: u8 = 76;
const KAKKO_LOW_NOTE: u8 = 77;

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SequenceLayer {
    Hichiriki,
    Ryuteki,
    Sho,
    Koto,
    Biwa,
    Taiko,
    Shoko,
    Kakko,
}

#[derive(Clone, Copy, Debug)]
pub struct SequenceEvent {
    pub t: f32,
    pub on: bool,
    pub note: u8,
    pub vel: u8,
    pub layer: SequenceLayer,
    pub ornaments: Ornaments,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScoreSection {
    Jo,
    Ha,
    Kyu,
}

impl ScoreSection {
    fn index(self) -> usize {
        match self {
            Self::Jo => 0,
            Self::Ha => 1,
            Self::Kyu => 2,
        }
    }

    fn from_beat(beat: f32) -> Self {
        let section = (beat / BEATS_PER_SECTION)
            .floor()
            .clamp(0.0, (TOTAL_SECTIONS - 1) as f32) as usize;
        match section {
            0 => Self::Jo,
            1 => Self::Ha,
            _ => Self::Kyu,
        }
    }
}

#[derive(Clone, Debug)]
struct NoteSpec {
    start_beats: f32,
    duration_beats: f32,
    note: u8,
    layer: SequenceLayer,
    velocities: (u8, u8),
    ornaments: Ornaments,
}

impl NoteSpec {
    fn end_beats(&self) -> f32 {
        self.start_beats + self.duration_beats
    }

    fn section(&self) -> ScoreSection {
        ScoreSection::from_beat(self.start_beats)
    }
}

#[derive(Clone)]
struct TempoMap {
    step_beats: f32,
    times: Vec<f32>,
}

impl TempoMap {
    fn total_seconds(&self) -> f32 {
        *self.times.last().unwrap_or(&0.0)
    }

    fn seconds_at(&self, beat: f32) -> f32 {
        if self.times.is_empty() {
            return 0.0;
        }
        let beat = beat.clamp(0.0, TOTAL_BEATS);
        let idx = (beat / self.step_beats).floor() as usize;
        let idx = idx.min(self.times.len().saturating_sub(2));
        let base_beat = idx as f32 * self.step_beats;
        let frac = if self.step_beats <= f32::EPSILON {
            0.0
        } else {
            (beat - base_beat) / self.step_beats
        };
        let t0 = self.times[idx];
        let t1 = self.times[idx + 1];
        t0 + (t1 - t0).mul_add(frac, 0.0)
    }

    fn beat_at(&self, seconds: f32) -> f32 {
        if self.times.is_empty() {
            return 0.0;
        }
        let seconds = seconds.clamp(0.0, self.total_seconds());
        let idx = match self
            .times
            .binary_search_by(|probe| probe.partial_cmp(&seconds).unwrap_or(Ordering::Equal))
        {
            Ok(idx) => idx,
            Err(idx) => idx.min(self.times.len().saturating_sub(1)),
        };
        if idx == 0 {
            return 0.0;
        }
        let t0 = self.times[idx - 1];
        let t1 = self.times[idx];
        let frac = if (t1 - t0).abs() <= f32::EPSILON {
            0.0
        } else {
            (seconds - t0) / (t1 - t0)
        };
        let base_beat = (idx - 1) as f32 * self.step_beats;
        frac.mul_add(self.step_beats, base_beat)
    }
}

#[derive(Clone)]
pub struct ScoreTimeline {
    tempo: TempoMap,
    total_beats: f32,
}

impl ScoreTimeline {
    fn new(tempo: TempoMap) -> Self {
        Self {
            tempo,
            total_beats: TOTAL_BEATS,
        }
    }

    pub fn total_seconds(&self) -> f32 {
        self.tempo.total_seconds()
    }

    pub fn seconds_at(&self, beat: f32) -> f32 {
        self.tempo.seconds_at(beat)
    }

    pub fn beat_at(&self, seconds: f32) -> f32 {
        self.tempo.beat_at(seconds)
    }

    pub fn average_seconds_per_beat(&self) -> f32 {
        if self.total_beats <= f32::EPSILON {
            0.0
        } else {
            self.total_seconds() / self.total_beats
        }
    }
}

fn approx_seconds_to_beats(seconds: f32) -> f32 {
    if seconds <= f32::EPSILON {
        0.0
    } else {
        seconds / APPROX_SECONDS_PER_BEAT
    }
}

fn base_hichiriki_specs() -> Vec<NoteSpec> {
    let mut specs = Vec::with_capacity(HICHIRIKI_EVENTS.len());
    for &(start_beats, duration_beats, name) in HICHIRIKI_EVENTS {
        let start = start_beats.max(0.0);
        let duration = duration_beats.max(0.25);
        let note = note_name_to_midi(name);
        let ornaments = ornaments_for_hichiriki_beats(duration);
        specs.push(NoteSpec {
            start_beats: start,
            duration_beats: duration,
            note,
            layer: SequenceLayer::Hichiriki,
            velocities: (108, 54),
            ornaments,
        });
    }
    enforce_monophonic_line(&mut specs);
    specs
}

fn base_ryuteki_specs() -> Vec<NoteSpec> {
    let mut specs = Vec::new();
    for &(start_beats, duration_beats, name) in HICHIRIKI_EVENTS {
        let start = start_beats.max(0.0);
        let duration = duration_beats.max(0.25);
        let note = note_name_to_midi(name).saturating_add(12);
        let ornaments = ornaments_for_ryuteki_beats(duration);
        specs.push(NoteSpec {
            start_beats: start,
            duration_beats: duration,
            note,
            layer: SequenceLayer::Ryuteki,
            velocities: (102, 50),
            ornaments,
        });
    }
    enforce_monophonic_line(&mut specs);
    specs
}

/// Ensure single-line winds (hichiriki, ryūteki) never emit overlapping notes.
fn enforce_monophonic_line(notes: &mut [NoteSpec]) {
    if notes.is_empty() {
        return;
    }
    let mut last_end = 0.0;
    for note in notes.iter_mut() {
        if note.start_beats < last_end {
            note.start_beats = last_end;
        }
        last_end = note.end_beats();
    }
}

fn base_sho_specs() -> Vec<NoteSpec> {
    let mut specs = Vec::new();
    for &(start_beats, duration_beats, chord) in SHO_CHORD_PROGRESSION {
        let start = start_beats.max(0.0);
        let duration = duration_beats.max(0.25);
        for &name in chord_note_names(chord) {
            let note = note_name_to_midi(name);
            specs.push(NoteSpec {
                start_beats: start,
                duration_beats: duration,
                note,
                layer: SequenceLayer::Sho,
                velocities: (90, 40),
                ornaments: Ornaments::empty(),
            });
        }
    }
    specs
}

fn base_koto_specs() -> Vec<NoteSpec> {
    let mut specs = Vec::new();
    for &(start_beats, sustain_beats, chord) in KOTO_ARPEGGIOS {
        let sustain = sustain_beats.max(1.5);
        for (voice_idx, name) in chord.iter().enumerate() {
            let voice_start = 0.07f32.mul_add(voice_idx as f32, start_beats).max(0.0);
            let duration = 0.08f32.mul_add(-(voice_idx as f32), sustain).max(1.25);
            let velocity = 86u8.saturating_sub((voice_idx as u8) * 6);
            specs.push(NoteSpec {
                start_beats: voice_start,
                duration_beats: duration,
                note: note_name_to_midi(name),
                layer: SequenceLayer::Koto,
                velocities: (velocity, 36),
                ornaments: if voice_idx == 0 {
                    Ornaments::with(&[OrnamentMark::Suriage])
                } else {
                    Ornaments::empty()
                },
            });
        }
    }
    specs
}

fn base_biwa_specs() -> Vec<NoteSpec> {
    let mut specs = Vec::new();
    for &(start_beats, sustain_beats, name, accent) in BIWA_SWELLS {
        let sustain = sustain_beats.max(1.25);
        let (on, off) = if accent { (102, 48) } else { (94, 44) };
        let ornaments = if accent {
            Ornaments::with(&[OrnamentMark::Ate, OrnamentMark::Oshi])
        } else {
            Ornaments::with(&[OrnamentMark::Ate])
        };
        specs.push(NoteSpec {
            start_beats: start_beats.max(0.0),
            duration_beats: sustain,
            note: note_name_to_midi(name),
            layer: SequenceLayer::Biwa,
            velocities: (on, off),
            ornaments,
        });
    }
    specs
}

fn base_taiko_specs() -> Vec<NoteSpec> {
    let mut specs = Vec::with_capacity(TAIKO_ACCENTS.len());
    for &(start_beats, velocity) in TAIKO_ACCENTS {
        let start = start_beats.clamp(0.0, TOTAL_BEATS);
        specs.push(NoteSpec {
            start_beats: start,
            duration_beats: 0.4,
            note: TAIKO_NOTE,
            layer: SequenceLayer::Taiko,
            velocities: (velocity.min(127), 0),
            ornaments: Ornaments::empty(),
        });
    }
    specs
}

fn base_shoko_specs() -> Vec<NoteSpec> {
    let mut specs = Vec::with_capacity(SHOKO_STRIKES.len());
    for &start_beats in SHOKO_STRIKES {
        let start = start_beats.clamp(0.0, TOTAL_BEATS);
        specs.push(NoteSpec {
            start_beats: start,
            duration_beats: 0.25,
            note: SHOKO_NOTE,
            layer: SequenceLayer::Shoko,
            velocities: (96, 0),
            ornaments: Ornaments::empty(),
        });
    }
    specs
}

fn base_kakko_specs() -> Vec<NoteSpec> {
    let mut specs = Vec::new();
    let mut beat = 0.0;
    let mut high = true;
    while beat < TOTAL_BEATS - 0.5 {
        let note = if high {
            KAKKO_HIGH_NOTE
        } else {
            KAKKO_LOW_NOTE
        };
        let velocity = if high { 92 } else { 86 };
        specs.push(NoteSpec {
            start_beats: beat.max(0.0),
            duration_beats: 0.18,
            note,
            layer: SequenceLayer::Kakko,
            velocities: (velocity, 0),
            ornaments: Ornaments::empty(),
        });
        beat += 0.85;
        high = !high;
    }
    specs
}

fn nearest_obachi(beat: f32) -> f32 {
    if TOTAL_BEATS <= f32::EPSILON {
        return 0.0;
    }
    let normalized = (beat - OBACHI_OFFSET_BEATS) / HYOSHI_BEATS;
    let idx = normalized.round();
    let obachi = idx.mul_add(HYOSHI_BEATS, OBACHI_OFFSET_BEATS);
    obachi.clamp(0.0, TOTAL_BEATS)
}

fn apply_obachi_anchor(notes: &mut [NoteSpec], window: f32, offset: f32) {
    if notes.is_empty() {
        return;
    }
    for note in notes.iter_mut() {
        let obachi = nearest_obachi(note.start_beats);
        if (note.start_beats - obachi).abs() <= window {
            note.start_beats = obachi + offset;
        }
    }
}

fn apply_section_entry_alignment(notes: &mut [NoteSpec], landing_offset: f32) {
    for section_idx in 0..TOTAL_SECTIONS {
        let section_start = section_idx as f32 * BEATS_PER_SECTION;
        let section_end = section_start + BEATS_PER_SECTION;
        let target = (section_start + OBACHI_OFFSET_BEATS) + landing_offset;
        if let Some(entry) = notes
            .iter_mut()
            .filter(|note| note.start_beats >= section_start && note.start_beats < section_end)
            .min_by(|a, b| {
                a.start_beats
                    .partial_cmp(&b.start_beats)
                    .unwrap_or(Ordering::Equal)
            })
        {
            entry.start_beats = target.clamp(section_start, section_end);
        }
    }
}

fn apply_hichiriki_phrasing(notes: &mut [NoteSpec]) {
    for note in notes.iter_mut() {
        let section = note.section();
        let section_start = section.index() as f32 * BEATS_PER_SECTION;
        let local = (note.start_beats - section_start).max(0.0);
        let obachi = section_start + OBACHI_OFFSET_BEATS;
        let window = 0.25;
        let shift = match section {
            ScoreSection::Jo => {
                if (note.start_beats - obachi).abs() <= window {
                    0.0
                } else {
                    let ease = (local / 7.0).clamp(0.0, 1.0);
                    let lead = -0.02_f32;
                    lead * (1.0 - ease)
                }
            }
            ScoreSection::Ha => {
                if (note.start_beats - obachi).abs() <= window {
                    0.0
                } else {
                    -0.01
                }
            }
            ScoreSection::Kyu => {
                if (note.start_beats - obachi).abs() <= window {
                    0.0
                } else {
                    0.012
                }
            }
        };
        note.start_beats = (note.start_beats + shift).max(section_start);
    }
}

fn apply_hichiriki_tataku_timing(notes: &mut [NoteSpec]) {
    let target = approx_seconds_to_beats(0.2);
    for note in notes.iter_mut() {
        if note.ornaments.contains(OrnamentMark::Tataku) {
            note.duration_beats = note.duration_beats.min(target * 1.1).max(target * 0.9);
            let (on, off) = note.velocities;
            note.velocities = (on.saturating_sub(8), off);
        }
    }
}

fn apply_sho_member_stagger(notes: &mut [NoteSpec]) {
    if notes.is_empty() {
        return;
    }
    notes.sort_by(|a, b| {
        a.start_beats
            .partial_cmp(&b.start_beats)
            .unwrap_or(Ordering::Equal)
    });
    let mut idx = 0;
    while idx < notes.len() {
        let start = notes[idx].start_beats;
        let duration = notes[idx].duration_beats;
        let mut end_idx = idx + 1;
        while end_idx < notes.len()
            && (notes[end_idx].start_beats - start).abs() <= f32::EPSILON
            && (notes[end_idx].duration_beats - duration).abs() <= f32::EPSILON
        {
            end_idx += 1;
        }
        let section = notes[idx].section();
        let spread = match section {
            ScoreSection::Jo => 0.03,
            ScoreSection::Ha => 0.02,
            ScoreSection::Kyu => 0.01,
        };
        let count = (end_idx - idx).max(1);
        let step = if count <= 1 {
            0.0
        } else {
            spread / (count - 1) as f32
        };
        for (offset_idx, note) in notes[idx..end_idx].iter_mut().enumerate() {
            let offset = step * offset_idx as f32;
            note.start_beats += offset;
        }
        idx = end_idx;
    }
}

fn apply_jitter(
    notes: &mut [NoteSpec],
    section_std: [f32; TOTAL_SECTIONS],
    obachi_window: f32,
    seed: u64,
) {
    if notes.is_empty() {
        return;
    }
    notes.sort_by(|a, b| {
        a.start_beats
            .partial_cmp(&b.start_beats)
            .unwrap_or(Ordering::Equal)
    });
    let mut rng = ScoreRng::new(seed);
    for note in notes.iter_mut() {
        let section = note.section();
        let std = section_std[section.index()];
        if std <= f32::EPSILON {
            continue;
        }
        let obachi = nearest_obachi(note.start_beats);
        if (note.start_beats - obachi).abs() <= obachi_window {
            continue;
        }
        let jitter = rng.next_gaussian() * std;
        note.start_beats = (note.start_beats + jitter).max(0.0);
    }
}

fn build_tempo_map() -> TempoMap {
    let step_beats = 1.0 / TEMPO_SUBDIVISIONS_PER_BEAT as f32;
    let total_steps = (TOTAL_BEATS / step_beats).ceil() as usize;
    let mut times = Vec::with_capacity(total_steps.saturating_add(2));
    times.push(0.0);

    let hyoshi_count = (TOTAL_BEATS / HYOSHI_BEATS).ceil() as usize;
    let mut rng = ScoreRng::new(0x41C6_4E6DA5);
    let mut pre_amp = Vec::with_capacity(hyoshi_count);
    let mut post_amp = Vec::with_capacity(hyoshi_count);
    for idx in 0..hyoshi_count {
        let section = ScoreSection::from_beat(idx as f32 * HYOSHI_BEATS);
        let range = SECTION_WANDER_BPM[section.index()];
        pre_amp.push(range * rng.next_signed());
        post_amp.push(range * rng.next_signed());
    }

    let mut beat = 0.0;
    for _ in 0..total_steps {
        let section = ScoreSection::from_beat(beat);
        let base_bpm = BASE_SECTION_BPM[section.index()].max(1.0);
        let hyoshi_idx = (beat / HYOSHI_BEATS)
            .floor()
            .clamp(0.0, (hyoshi_count - 1) as f32) as usize;
        let hyoshi_base = hyoshi_idx as f32 * HYOSHI_BEATS;
        let obachi = hyoshi_base + OBACHI_OFFSET_BEATS;
        let drift_amp = if beat < obachi {
            pre_amp[hyoshi_idx]
        } else {
            post_amp[hyoshi_idx]
        };
        let phase = if OBACHI_OFFSET_BEATS <= f32::EPSILON {
            0.0
        } else {
            (beat - obachi) / OBACHI_OFFSET_BEATS
        };
        let drift = drift_amp * (phase * PI).sin();
        let bpm = (base_bpm + drift).max(1.0);
        let seconds = step_beats * 60.0 / bpm;
        let last = *times.last().unwrap_or(&0.0);
        times.push(last + seconds);
        beat += step_beats;
    }

    TempoMap { step_beats, times }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Ornaments {
    bits: u8,
}

impl Ornaments {
    pub const fn empty() -> Self {
        Self { bits: 0 }
    }

    pub fn with(marks: &[OrnamentMark]) -> Self {
        let mut bits = 0;
        let mut idx = 0;
        while idx < marks.len() {
            bits |= marks[idx].bit();
            idx += 1;
        }
        Self { bits }
    }

    pub fn contains(self, mark: OrnamentMark) -> bool {
        self.bits & mark.bit() != 0
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub enum OrnamentMark {
    Seme,
    Fukura,
    Tataku,
    Ate,
    Oshi,
    Mawashi,
    Suriage,
    Orite,
}

impl OrnamentMark {
    const fn bit(self) -> u8 {
        match self {
            Self::Seme => 1 << 0,
            Self::Fukura => 1 << 1,
            Self::Tataku => 1 << 2,
            Self::Ate => 1 << 3,
            Self::Oshi => 1 << 4,
            Self::Mawashi => 1 << 5,
            Self::Suriage => 1 << 6,
            Self::Orite => 1 << 7,
        }
    }
}

pub fn hyoshi_breath_scalar(beat: f32) -> f32 {
    if HYOSHI_BEATS <= 0.0 {
        return 1.0;
    }
    let phase = (beat.rem_euclid(HYOSHI_BEATS)) / HYOSHI_BEATS;
    let sin = (PI * phase).sin().abs();
    0.55_f32.mul_add(sin.powi(2), 0.45)
}

pub fn layer_intonation_cents(layer: SequenceLayer, note: u8) -> f32 {
    let base_offset = 1200.0 * (A4_TUNING_HZ / 440.0).ln() / LN_2;
    let scale_adjust = match note % 12 {
        1 | 6 => -8.0, // C#, F#
        2 => -3.0,     // D
        4 | 9 => -3.5, // E, A
        11 => 3.5,     // B
        _ => 0.0,
    };
    match layer {
        SequenceLayer::Hichiriki | SequenceLayer::Ryuteki | SequenceLayer::Sho => {
            base_offset + scale_adjust
        }
        _ => 0.0,
    }
}

pub fn total_beats() -> f32 {
    TOTAL_BEATS
}

#[derive(Clone)]
struct MidiEvent {
    tick: u32,
    data: Vec<u8>,
}

pub fn write_demo_midi_file() -> Result<String> {
    let (events, timeline) = synth_events();
    let ticks_per_beat = MIDI_TICKS_PER_BEAT;
    let seconds_per_beat = timeline.average_seconds_per_beat().max(1.0e-3);
    let tempo_micros = (seconds_per_beat * 1_000_000.0).round() as u32;

    let mut track_events: Vec<MidiEvent> = vec![
        MidiEvent {
            tick: 0,
            data: vec![
                0xFF,
                0x51,
                0x03,
                (tempo_micros >> 16) as u8,
                (tempo_micros >> 8) as u8,
                tempo_micros as u8,
            ],
        },
        MidiEvent {
            tick: 0,
            data: vec![0xC0, 68],
        }, // Hichiriki → oboe
        MidiEvent {
            tick: 0,
            data: vec![0xC1, 73],
        }, // Ryūteki → flute
        MidiEvent {
            tick: 0,
            data: vec![0xC2, 19],
        }, // Shō → organ
        MidiEvent {
            tick: 0,
            data: vec![0xC3, 107],
        }, // Koto → koto
        MidiEvent {
            tick: 0,
            data: vec![0xC4, 106],
        }, // Biwa → shamisen
        MidiEvent {
            tick: 0,
            data: vec![0xB0, 0x07, 100],
        },
        MidiEvent {
            tick: 0,
            data: vec![0xB1, 0x07, 96],
        },
        MidiEvent {
            tick: 0,
            data: vec![0xB2, 0x07, 88],
        },
        MidiEvent {
            tick: 0,
            data: vec![0xB3, 0x07, 92],
        },
        MidiEvent {
            tick: 0,
            data: vec![0xB4, 0x07, 90],
        },
        MidiEvent {
            tick: 0,
            data: vec![0xB9, 0x07, 118],
        },
    ];

    for event in events {
        let channel = midi_channel(event.layer);
        let status = if event.on {
            0x90 | channel
        } else {
            0x80 | channel
        };
        let tick = seconds_to_ticks(event.t, seconds_per_beat, ticks_per_beat);
        track_events.push(MidiEvent {
            tick,
            data: vec![status, event.note, event.vel.min(127)],
        });
    }

    let end_tick = seconds_to_ticks(timeline.total_seconds(), seconds_per_beat, ticks_per_beat);
    track_events.push(MidiEvent {
        tick: end_tick,
        data: vec![0xFF, 0x2F, 0x00],
    });

    track_events.sort_by(|a, b| a.tick.cmp(&b.tick));
    let mut track_bytes = Vec::new();
    let mut last_tick = 0;
    for event in track_events {
        write_var_len(event.tick.saturating_sub(last_tick), &mut track_bytes);
        track_bytes.extend_from_slice(&event.data);
        last_tick = event.tick;
    }

    let mut file = Vec::new();
    file.extend_from_slice(b"MThd");
    file.extend_from_slice(&6u32.to_be_bytes());
    file.extend_from_slice(&0u16.to_be_bytes());
    file.extend_from_slice(&1u16.to_be_bytes());
    file.extend_from_slice(&ticks_per_beat.to_be_bytes());
    file.extend_from_slice(b"MTrk");
    file.extend_from_slice(&(track_bytes.len() as u32).to_be_bytes());
    file.extend_from_slice(&track_bytes);

    let path = std::env::temp_dir().join("iroha_monitor_etenraku.mid");
    std::fs::write(&path, &file)?;
    Ok(path.to_string_lossy().into_owned())
}

pub fn synth_events() -> (Vec<SequenceEvent>, ScoreTimeline) {
    let timeline = ScoreTimeline::new(build_tempo_map());

    let mut hichiriki = base_hichiriki_specs();
    apply_section_entry_alignment(&mut hichiriki, -0.01);
    apply_hichiriki_phrasing(&mut hichiriki);
    apply_hichiriki_tataku_timing(&mut hichiriki);
    apply_obachi_anchor(&mut hichiriki, 0.05, -0.005);
    apply_jitter(&mut hichiriki, [0.015, 0.012, 0.010], 0.02, 0xB137_1F11);

    let mut ryuteki = base_ryuteki_specs();
    apply_obachi_anchor(&mut ryuteki, 0.05, 0.003);
    apply_jitter(&mut ryuteki, [0.012, 0.010, 0.012], 0.02, 0xC001_FEED);

    let mut sho = base_sho_specs();
    apply_sho_member_stagger(&mut sho);
    apply_obachi_anchor(&mut sho, 0.04, 0.0);
    apply_jitter(&mut sho, [0.008, 0.006, 0.004], 0.015, 0xA11D_CAFE);

    let mut koto = base_koto_specs();
    apply_obachi_anchor(&mut koto, 0.08, 0.012);
    apply_jitter(&mut koto, [0.006, 0.005, 0.004], 0.012, 0x5AA5_F011);

    let mut biwa = base_biwa_specs();
    apply_obachi_anchor(&mut biwa, 0.08, -0.006);
    apply_jitter(&mut biwa, [0.006, 0.005, 0.005], 0.012, 0xB1A0_5EED);

    let taiko = base_taiko_specs();
    let shoko = base_shoko_specs();
    let mut kakko = base_kakko_specs();
    apply_jitter(&mut kakko, [0.002, 0.002, 0.001], 0.01, 0xCACC_0D0E);

    let mut note_specs = Vec::new();
    note_specs.extend(hichiriki);
    note_specs.extend(ryuteki);
    note_specs.extend(sho);
    note_specs.extend(koto);
    note_specs.extend(biwa);
    note_specs.extend(taiko);
    note_specs.extend(shoko);
    note_specs.extend(kakko);

    let mut events = Vec::with_capacity(note_specs.len() * 2);
    for note in note_specs {
        let start_seconds = timeline.seconds_at(note.start_beats);
        let end_seconds = timeline
            .seconds_at(note.end_beats())
            .max(start_seconds + 0.01);
        events.push(SequenceEvent {
            t: start_seconds,
            on: true,
            note: note.note,
            vel: note.velocities.0,
            layer: note.layer,
            ornaments: note.ornaments,
        });
        events.push(SequenceEvent {
            t: end_seconds,
            on: false,
            note: note.note,
            vel: note.velocities.1,
            layer: note.layer,
            ornaments: Ornaments::empty(),
        });
    }

    events.sort_by(|a, b| {
        a.t.partial_cmp(&b.t)
            .unwrap_or(Ordering::Equal)
            .then_with(|| b.on.cmp(&a.on))
            .then_with(|| a.note.cmp(&b.note))
    });

    (events, timeline)
}

fn midi_channel(layer: SequenceLayer) -> u8 {
    match layer {
        SequenceLayer::Hichiriki => 0,
        SequenceLayer::Ryuteki => 1,
        SequenceLayer::Sho => 2,
        SequenceLayer::Koto => 3,
        SequenceLayer::Biwa => 4,
        SequenceLayer::Taiko | SequenceLayer::Shoko | SequenceLayer::Kakko => 9,
    }
}

fn ornaments_for_hichiriki_beats(duration_beats: f32) -> Ornaments {
    let long_threshold = approx_seconds_to_beats(6.0);
    let short_threshold = approx_seconds_to_beats(2.0);
    if duration_beats >= long_threshold {
        Ornaments::with(&[OrnamentMark::Fukura, OrnamentMark::Mawashi])
    } else if duration_beats <= short_threshold {
        Ornaments::with(&[OrnamentMark::Tataku])
    } else {
        Ornaments::with(&[OrnamentMark::Fukura])
    }
}

fn ornaments_for_ryuteki_beats(duration_beats: f32) -> Ornaments {
    let long_threshold = approx_seconds_to_beats(6.0);
    let short_threshold = approx_seconds_to_beats(2.0);
    if duration_beats >= long_threshold {
        Ornaments::with(&[OrnamentMark::Mawashi])
    } else if duration_beats <= short_threshold {
        Ornaments::with(&[OrnamentMark::Tataku])
    } else {
        Ornaments::empty()
    }
}

fn chord_note_names(name: &str) -> &'static [&'static str] {
    match name {
        "Kotsu" => &["A4", "B4", "E5", "F#5", "A5", "B5"],
        "Ichikotsu" => &["A4", "B4", "C#5", "D#5", "F#5", "G#5"],
        _ => &[],
    }
}

fn note_name_to_midi(name: &str) -> u8 {
    let (prefix, octave_str) = name.split_at(name.len() - 1);
    let octave: i32 = octave_str.parse().expect("invalid octave");
    let semitone = match prefix {
        "C" => 0,
        "C#" | "Db" => 1,
        "D" => 2,
        "D#" | "Eb" => 3,
        "E" => 4,
        "F" => 5,
        "F#" | "Gb" => 6,
        "G" => 7,
        "G#" | "Ab" => 8,
        "A" => 9,
        "A#" | "Bb" => 10,
        "B" => 11,
        _ => panic!("unsupported note name: {name}"),
    };
    let midi = 12 * (octave + 1) + semitone;
    midi.clamp(0, 127) as u8
}

fn seconds_to_ticks(seconds: f32, seconds_per_beat: f32, ticks_per_beat: u16) -> u32 {
    if seconds_per_beat <= f32::EPSILON {
        return 0;
    }
    let beats = seconds.max(0.0) / seconds_per_beat;
    (beats * f32::from(ticks_per_beat)).round().max(0.0) as u32
}

#[derive(Clone, Debug)]
struct ScoreRng(u64);

impl ScoreRng {
    fn new(seed: u64) -> Self {
        let seed = if seed == 0 { 0x000D_ECAF_CAFE } else { seed };
        Self(seed)
    }

    fn next_u32(&mut self) -> u32 {
        // Simple splitmix64 variant; sufficient for deterministic humanisation.
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 32) as u32
    }

    fn next_f32(&mut self) -> f32 {
        let bits = 0x3F80_0000 | (self.next_u32() >> 9);
        f32::from_bits(bits) - 1.0
    }

    fn next_signed(&mut self) -> f32 {
        self.next_f32().mul_add(2.0, -1.0)
    }

    fn next_gaussian(&mut self) -> f32 {
        let u1 = self.next_f32().abs().max(1.0e-6);
        let u2 = self.next_f32();
        (-2.0 * u1.ln()).sqrt() * (2.0 * PI * u2).cos()
    }
}

fn write_var_len(mut value: u32, out: &mut Vec<u8>) {
    let mut buffer = [0u8; 4];
    let mut index = 3;
    buffer[index] = (value & 0x7F) as u8;
    while {
        value >>= 7;
        value > 0
    } {
        index -= 1;
        buffer[index] = ((value & 0x7F) as u8) | 0x80;
    }
    out.extend_from_slice(&buffer[index..]);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_monophonic(notes: &[NoteSpec], label: &str) {
        let mut last_end = 0.0_f32;
        for note in notes {
            assert!(
                note.start_beats + 1.0e-4 >= last_end,
                "{label} should remain monophonic (start={} < last_end={})",
                note.start_beats,
                last_end
            );
            last_end = note.end_beats();
        }
    }

    #[test]
    fn note_name_conversion() {
        assert_eq!(note_name_to_midi("C4"), 60);
        assert_eq!(note_name_to_midi("A4"), 69);
        assert_eq!(note_name_to_midi("C#5"), 73);
    }

    #[test]
    fn synth_events_cover_duration() {
        let (events, timeline) = synth_events();
        assert!(!events.is_empty());
        let max = events.iter().fold(0.0_f32, |acc, ev| acc.max(ev.t));
        assert!(max <= timeline.total_seconds());
    }

    #[test]
    fn midi_file_is_written() {
        let path = write_demo_midi_file().expect("midi generation");
        assert!(std::path::Path::new(&path).exists());
    }

    #[test]
    fn breath_scalar_stays_in_range() {
        for step in 0..64 {
            let beat = step as f32 * (HYOSHI_BEATS / 64.0);
            let scalar = hyoshi_breath_scalar(beat);
            assert!((0.0..=1.5).contains(&scalar));
        }
    }

    #[test]
    fn tempo_drift_resets_at_obachi() {
        let timeline = ScoreTimeline::new(build_tempo_map());
        let delta = 0.05;
        let hyoshi_count = (TOTAL_BEATS / HYOSHI_BEATS).round() as usize;
        for idx in 0..hyoshi_count {
            let center = (idx as f32).mul_add(HYOSHI_BEATS, OBACHI_OFFSET_BEATS);
            if center <= delta || center + delta >= TOTAL_BEATS {
                continue;
            }
            let before = timeline.seconds_at(center) - timeline.seconds_at(center - delta);
            let after = timeline.seconds_at(center + delta) - timeline.seconds_at(center);
            let diff = (before - after).abs();
            assert!(
                diff <= 0.002,
                "drift should reset near obachi (diff={diff})"
            );
        }
    }

    #[test]
    fn synth_events_cover_extended_layers() {
        let (events, timeline) = synth_events();
        assert!(timeline.total_seconds() > 0.0);

        let mut has_koto = false;
        let mut has_biwa = false;
        let mut has_taiko = false;
        let mut has_shoko = false;
        let mut has_kakko = false;
        let mut last_taiko_time = 0.0_f32;

        for event in &events {
            if !event.on {
                continue;
            }
            match event.layer {
                SequenceLayer::Koto => has_koto = true,
                SequenceLayer::Biwa => has_biwa = true,
                SequenceLayer::Taiko => {
                    has_taiko = true;
                    last_taiko_time = last_taiko_time.max(event.t);
                }
                SequenceLayer::Shoko => has_shoko = true,
                SequenceLayer::Kakko => has_kakko = true,
                _ => {}
            }
        }

        assert!(
            has_koto,
            "koto strums must be present in the rendered sequence"
        );
        assert!(
            has_biwa,
            "biwa phrases must be present in the rendered sequence"
        );
        assert!(
            has_taiko,
            "taiko downbeats must be present in the rendered sequence"
        );
        assert!(
            has_shoko,
            "shoko gongs must be present in the rendered sequence"
        );
        assert!(
            has_kakko,
            "kakko pulses must be present in the rendered sequence"
        );
        assert!(
            last_taiko_time > timeline.total_seconds() - 6.0,
            "final taiko cadence should reach the closing section"
        );

        let kakko_hits = events
            .iter()
            .filter(|event| event.on && matches!(event.layer, SequenceLayer::Kakko))
            .count();
        assert!(
            kakko_hits >= 20,
            "kakko pulse train should provide ongoing subdivision (found {kakko_hits})"
        );
    }

    #[test]
    fn hichiriki_sequence_is_monophonic() {
        let specs = base_hichiriki_specs();
        assert_monophonic(&specs, "hichiriki");
    }

    #[test]
    fn ryuteki_sequence_is_monophonic() {
        let specs = base_ryuteki_specs();
        assert_monophonic(&specs, "ryūteki");
    }
}
