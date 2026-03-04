#![allow(clippy::cast_precision_loss, clippy::suboptimal_flops)]

//! Animated ASCII art for `iroha_monitor`.
//!
//! The art is intentionally hand-crafted to evoke a lively matsuri scene:
//! a torii gate with lanterns, Mt. Fuji in the background, gentle waves, and
//! koi shimmering across the waterline. Rendering is done entirely with text
//! so it works on any terminal, including the `TERM=dumb` that our tests use.
//! The header also sketches the current hyōshi breath contour from the Etenraku
//! transcription so the animation mirrors the music’s phrasing.

use std::convert::TryFrom;

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::etenraku::{self, hyoshi_breath_scalar};

const SKY_FRAMES_NIGHT: [&[&str]; 4] = [
    &[
        "             ☆         ✦              ✧        ☆         ",
        "    ✧                ✶            ☆         ✧            ",
    ],
    &[
        "         ✦        ☆           ✧         ☆               ",
        "    ☆          ✶          ✧         ✦         ☆        ",
    ],
    &[
        "             ✧         ☆              ✦        ✶       ",
        "       ☆         ✦             ✧            ☆          ",
    ],
    &[
        "         ☆        ✶          ✧         ✦        ☆       ",
        "    ✦           ☆           ✧        ✶         ✦        ",
    ],
];

const SKY_FRAMES_DAWN: [&[&str]; 4] = [
    &[
        "             ☀         ✶              ✧        ☀         ",
        "    ✧                ✶            ☀         ✧            ",
    ],
    &[
        "         ✶        ☀           ✧         ☀               ",
        "    ☀          ✶          ✧         ✶         ☀        ",
    ],
    &[
        "             ✧         ☀              ✶        ✧       ",
        "       ☀         ✶             ✧            ☀          ",
    ],
    &[
        "         ☀        ✧          ✶         ✧        ☀       ",
        "    ✶           ☀           ✧        ✶         ✶        ",
    ],
];

const SKY_FRAMES_SAKURA: [&[&str]; 4] = [
    &[
        "             ❀         ✿              ✧        ❀         ",
        "    ✧                ❁            ❀         ✧            ",
    ],
    &[
        "         ✿        ❀           ✧         ❀               ",
        "    ❀          ❁          ✧         ✿         ❀        ",
    ],
    &[
        "             ✧         ❀              ✿        ❁       ",
        "       ❀         ✿             ✧            ❀          ",
    ],
    &[
        "         ❀        ❁          ✧         ✿        ❀       ",
        "    ✿           ❀           ✧        ❁         ✿        ",
    ],
];

/// Torii gate and hanging lanterns (shared across themes).
const TORII_ART: [&str; 6] = [
    "                _____________☆_____________               ",
    "      _________/      祭   |   |   祭      \\_________       ",
    "     |  |    | |           |           | |    |  |      ",
    "     |  |    | |           |           | |    |  |      ",
    "     |  |====|=|===========+===========|=|====|  |      ",
    "       ||    ||             |             ||    ||      ",
];

/// Mountain silhouette with a stylised "富士" banner.
const MOUNTAIN_ART: [&str; 3] = [
    "                   /\\             富士          ",
    "          ________/  \\_______                   ",
    "         /                    \\                 ",
];

/// Koi sprite frames skimming the surface.
const KOI_FRAMES: [&str; 4] = ["<><", "≋><", "<>≋", "<><≋"];

const BEATS_PER_FRAME: f32 = etenraku::HYOSHI_BEATS;
const BREATH_MIN: f32 = 0.5;
const BREATH_MAX: f32 = 1.2;
const OBAICHI_WINDOW: f32 = 0.08;
const WAVE_TABLE_LEN: usize = 16;

#[derive(Clone, Copy)]
enum WaveEnergy {
    Calm,
    Lively,
    Crest,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum AsciiTheme {
    #[default]
    Night,
    Dawn,
    Sakura,
}

#[derive(Debug, Clone, Copy)]
pub struct AsciiConfig {
    pub speed: u16,
    pub theme: AsciiTheme,
}

impl Default for AsciiConfig {
    fn default() -> Self {
        Self {
            speed: 1,
            theme: AsciiTheme::Night,
        }
    }
}

/// Generates animated ASCII art for the monitor header.
#[derive(Debug, Clone)]
pub struct AsciiAnimator {
    wave_offset: usize,
    star_phase: usize,
    koi_phase: usize,
    beat: f32,
    config: AsciiConfig,
}

impl Default for AsciiAnimator {
    fn default() -> Self {
        Self::with_config(AsciiConfig::default())
    }
}

impl AsciiAnimator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_config(config: AsciiConfig) -> Self {
        Self {
            wave_offset: 0,
            star_phase: 0,
            koi_phase: 0,
            beat: 0.0,
            config,
        }
    }

    /// Advance the internal animation state.
    pub fn advance(&mut self) {
        let step = usize::from(self.config.speed.max(1));
        self.wave_offset = (self.wave_offset + step) % 16;
        self.star_phase = (self.star_phase + step) % sky_cycle_len();
        self.koi_phase = (self.koi_phase + step) % KOI_FRAMES.len();
        let total_beats = etenraku::total_beats().max(etenraku::HYOSHI_BEATS);
        let mut next = self.beat + BEATS_PER_FRAME;
        if next >= total_beats {
            next -= total_beats;
        }
        self.beat = next;
    }

    /// Render a frame sized for the provided terminal width.
    pub fn frame(&self, width: u16) -> Vec<String> {
        self.frame_with_height(width, None)
    }

    /// Render a frame sized for the provided terminal width and capped to
    /// `max_lines` if supplied.
    pub fn frame_with_height(&self, width: u16, max_lines: Option<usize>) -> Vec<String> {
        let width = usize::from(width.max(1));
        let mut lines = Vec::new();

        for &sky_line in sky_frame(self.config.theme, self.star_phase) {
            lines.push(pad_center(sky_line, width));
        }
        for &gate_line in &TORII_ART {
            lines.push(pad_center(gate_line, width));
        }
        for &mountain_line in &MOUNTAIN_ART {
            lines.push(pad_center(mountain_line, width));
        }

        lines.push(self.horizon_line(width));
        lines.push(self.wave_line(width, 0));
        lines.push(self.wave_line(width, 3));

        if let Some(limit) = max_lines {
            clip_lines(lines, limit)
        } else {
            lines
        }
    }

    fn horizon_line(&self, width: usize) -> String {
        let koi = KOI_FRAMES[self.koi_phase];
        if width == 0 {
            return String::new();
        }
        let koi_start = (width / 4)
            .saturating_add(self.wave_offset % width.max(1))
            .min(width.saturating_sub(1));
        let accents = horizon_accents(self.config.theme);
        let koi_chars: Vec<char> = koi.chars().collect();
        let mut koi_index = 0usize;
        let mut consumed = 0usize;
        let mut column_index = 0usize;
        let mut line = String::with_capacity(width);
        while consumed < width {
            let mut ch = if column_index >= koi_start && koi_index < koi_chars.len() {
                let symbol = koi_chars[koi_index];
                koi_index += 1;
                symbol
            } else if column_index % 7 == (self.wave_offset % 7) {
                accents.0
            } else if column_index % 11 == ((self.wave_offset + 3) % 11) {
                accents.1
            } else {
                accents.2
            };
            let mut ch_width = UnicodeWidthChar::width(ch).unwrap_or(1).max(1);
            if consumed + ch_width > width {
                ch = ' ';
                ch_width = 1;
            }
            line.push(ch);
            consumed += ch_width;
            column_index += 1;
        }
        line
    }

    fn wave_line(&self, width: usize, phase_shift: usize) -> String {
        if width == 0 {
            return String::new();
        }
        let mut line = String::with_capacity(width);
        let offset = (self.wave_offset + phase_shift) % WAVE_TABLE_LEN;
        let mut step = 0usize;
        let total_beats = etenraku::total_beats().max(etenraku::HYOSHI_BEATS);
        let wave_table_len =
            f32::from(u16::try_from(WAVE_TABLE_LEN).expect("wave table length fits in u16"));
        let beat_step = etenraku::HYOSHI_BEATS / wave_table_len;
        let mut consumed = 0usize;
        while consumed < width {
            let step_f32 = step as f32;
            let sample = beat_step
                .mul_add(step_f32, self.beat)
                .rem_euclid(total_beats);
            let energy = wave_energy_for_beat(sample);
            let palette = wave_palette(self.config.theme, energy);
            let mut symbol = palette[(step + offset) % palette.len()];
            let local = sample.rem_euclid(etenraku::HYOSHI_BEATS);
            if is_obachi_marker(local) {
                symbol = '|';
            }
            let sym_width = UnicodeWidthChar::width(symbol).unwrap_or(1).max(1);
            if consumed + sym_width > width {
                line.push(' ');
                consumed += 1;
            } else {
                line.push(symbol);
                consumed += sym_width;
            }
            step += 1;
        }
        line
    }
}

fn is_obachi_marker(local_beat: f32) -> bool {
    etenraku::HYOSHI_BEATS.mul_add(-0.5, local_beat).abs() <= OBAICHI_WINDOW
}

fn wave_energy_for_beat(beat: f32) -> WaveEnergy {
    let envelope = hyoshi_breath_scalar(beat);
    let normalized = ((envelope - BREATH_MIN) / (BREATH_MAX - BREATH_MIN)).clamp(0.0, 1.0);
    if normalized < 0.45 {
        WaveEnergy::Calm
    } else if normalized < 0.75 {
        WaveEnergy::Lively
    } else {
        WaveEnergy::Crest
    }
}

fn sky_cycle_len() -> usize {
    SKY_FRAMES_NIGHT.len()
}

fn sky_frame(theme: AsciiTheme, phase: usize) -> &'static [&'static str] {
    let frames = match theme {
        AsciiTheme::Night => &SKY_FRAMES_NIGHT,
        AsciiTheme::Dawn => &SKY_FRAMES_DAWN,
        AsciiTheme::Sakura => &SKY_FRAMES_SAKURA,
    };
    frames[phase % frames.len()]
}

fn horizon_accents(theme: AsciiTheme) -> (char, char, char) {
    match theme {
        AsciiTheme::Night => ('˜', 'ˉ', '~'),
        AsciiTheme::Dawn => ('~', 'ˍ', '='),
        AsciiTheme::Sakura => ('ゝ', 'ゞ', '〜'),
    }
}

const NIGHT_WAVES_CALM: [char; WAVE_TABLE_LEN] = [
    '~', 'ˉ', '~', '-', '~', 'ˉ', '-', '~', '~', 'ˉ', '~', '-', '~', 'ˉ', '-', '~',
];
const NIGHT_WAVES_LIVELY: [char; WAVE_TABLE_LEN] = [
    '〜', '、', '∽', '〰', '≈', '~', '﹌', '﹋', '〜', '、', '∽', '〰', '≈', '~', '﹌', '﹋',
];
const NIGHT_WAVES_CREST: [char; WAVE_TABLE_LEN] = [
    '≋', '≈', '^', '≈', '≋', '^', '≈', '≋', '≈', '^', '≈', '≋', '≈', '^', '≈', '≋',
];

const DAWN_WAVES_CALM: [char; WAVE_TABLE_LEN] = [
    '~', '-', '~', '-', '~', '-', '~', '-', '~', '-', '~', '-', '~', '-', '~', '-',
];
const DAWN_WAVES_LIVELY: [char; WAVE_TABLE_LEN] = [
    '=', '~', '-', '~', '=', '~', '-', '~', '=', '~', '-', '~', '=', '~', '-', '~',
];
const DAWN_WAVES_CREST: [char; WAVE_TABLE_LEN] = [
    '≋', '^', '=', '^', '≋', '^', '=', '^', '≋', '^', '=', '^', '≋', '^', '=', '^',
];

const SAKURA_WAVES_CALM: [char; WAVE_TABLE_LEN] = [
    'ゝ', '゠', '〜', 'ゝ', '゠', '〜', 'ゝ', '゠', '〜', 'ゝ', '゠', '〜', 'ゝ', '゠', '〜', 'ゝ',
];
const SAKURA_WAVES_LIVELY: [char; WAVE_TABLE_LEN] = [
    '゠', 'ゝ', 'ゞ', '゠', 'ゝ', 'ゞ', '゠', 'ゝ', 'ゞ', '゠', 'ゝ', 'ゞ', '゠', 'ゝ', 'ゞ', '゠',
];
const SAKURA_WAVES_CREST: [char; WAVE_TABLE_LEN] = [
    'ゞ', '✶', 'ゞ', '✶', 'ゞ', '✶', 'ゞ', '✶', 'ゞ', '✶', 'ゞ', '✶', 'ゞ', '✶', 'ゞ', '✶',
];

fn wave_palette(theme: AsciiTheme, energy: WaveEnergy) -> &'static [char; WAVE_TABLE_LEN] {
    match (theme, energy) {
        (AsciiTheme::Night, WaveEnergy::Calm) => &NIGHT_WAVES_CALM,
        (AsciiTheme::Night, WaveEnergy::Lively) => &NIGHT_WAVES_LIVELY,
        (AsciiTheme::Night, WaveEnergy::Crest) => &NIGHT_WAVES_CREST,
        (AsciiTheme::Dawn, WaveEnergy::Calm) => &DAWN_WAVES_CALM,
        (AsciiTheme::Dawn, WaveEnergy::Lively) => &DAWN_WAVES_LIVELY,
        (AsciiTheme::Dawn, WaveEnergy::Crest) => &DAWN_WAVES_CREST,
        (AsciiTheme::Sakura, WaveEnergy::Calm) => &SAKURA_WAVES_CALM,
        (AsciiTheme::Sakura, WaveEnergy::Lively) => &SAKURA_WAVES_LIVELY,
        (AsciiTheme::Sakura, WaveEnergy::Crest) => &SAKURA_WAVES_CREST,
    }
}

fn pad_center(src: &str, width: usize) -> String {
    let len = display_width(src);
    if len >= width {
        return truncate_to_width(src, width);
    }
    let padding = width - len;
    let left = padding / 2;
    let right = padding - left;
    let mut buf = String::with_capacity(width + src.len());
    buf.extend(std::iter::repeat_n(' ', left));
    buf.push_str(src);
    buf.extend(std::iter::repeat_n(' ', right));
    buf
}

fn truncate_to_width(src: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let mut buf = String::new();
    let mut consumed = 0;
    for ch in src.chars() {
        let w = UnicodeWidthChar::width(ch).unwrap_or(0);
        if consumed + w > width {
            break;
        }
        buf.push(ch);
        consumed += w;
    }
    if consumed < width {
        buf.extend(std::iter::repeat_n(' ', width - consumed));
    }
    buf
}

fn display_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

fn clip_lines(mut lines: Vec<String>, limit: usize) -> Vec<String> {
    if limit == 0 {
        return Vec::new();
    }
    if lines.len() <= limit {
        return lines;
    }
    let start = lines.len() - limit;
    lines.drain(0..start);
    lines
}

pub fn center_line(text: &str, width: usize) -> String {
    pad_center(text, width)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_has_consistent_width() {
        let mut anim = AsciiAnimator::new();
        for width in [48u16, 64, 72] {
            let frame = anim.frame(width);
            assert!(frame.len() >= 11, "expected at least 11 lines");
            for line in frame {
                assert_eq!(display_width(&line), width as usize);
            }
            anim.advance();
        }
    }

    #[test]
    fn animation_cycles_wave_pattern() {
        let mut anim = AsciiAnimator::new();
        let first = anim.horizon_line(80);
        anim.advance();
        let second = anim.horizon_line(80);
        assert_ne!(first, second, "wave line should animate across frames");
    }

    #[test]
    fn custom_speed_advances_multiple_steps() {
        let mut anim = AsciiAnimator::with_config(AsciiConfig {
            speed: 3,
            theme: AsciiTheme::Dawn,
        });
        let initial = anim.wave_offset;
        anim.advance();
        assert_eq!((initial + 3) % 16, anim.wave_offset);
    }

    #[test]
    fn sakura_theme_uses_custom_accent() {
        let anim = AsciiAnimator::with_config(AsciiConfig {
            speed: 1,
            theme: AsciiTheme::Sakura,
        });
        let line = anim.horizon_line(32);
        assert!(line.contains('ゝ'));
    }

    #[test]
    fn frame_respects_height_limit() {
        let anim = AsciiAnimator::new();
        let frame_full = anim.frame_with_height(60, None);
        let capped = anim.frame_with_height(60, Some(5));
        assert!(frame_full.len() > capped.len());
        assert_eq!(capped.len(), 5);
    }

    #[test]
    fn frame_respects_small_width() {
        let anim = AsciiAnimator::new();
        let frame = anim.frame(24);
        for line in frame {
            assert!(display_width(&line) <= 24);
        }
    }

    #[test]
    fn wave_line_reflects_rhythm_energy() {
        let mut anim = AsciiAnimator::new();
        anim.beat = etenraku::PRELUDE_BEATS + etenraku::HYOSHI_BEATS * 0.5;
        let calm = anim.wave_line(64, 0);

        anim.beat = (etenraku::total_beats() - 16.0).max(etenraku::PRELUDE_BEATS);
        let crest = anim.wave_line(64, 0);
        assert_ne!(calm, crest);
        let energy_score = |line: &str| -> i32 {
            line.chars()
                .map(|ch| match ch {
                    '≋' => 3,
                    '^' => 2,
                    '~' | '∿' => 1,
                    _ => 0,
                })
                .sum()
        };
        let calm_energy = energy_score(&calm);
        let crest_energy = energy_score(&crest);
        assert!(crest_energy > calm_energy);
    }
}
