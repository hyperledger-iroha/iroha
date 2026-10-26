//! End-to-end stream trials against the simulated camera.
//!
//! One trial plays a stream on a virtual screen at `display_fps`, films it with
//! a virtual phone camera (frame blending and tearing whenever an exposure
//! straddles a frame change, or a rolling shutter that exposes the rows one
//! after another) and runs the frames through a [`ScanSession`] until the
//! payload arrives or the horizon is reached. Operators use it through `iroha offline petal
//! simulate`; the repository's qualification examples use it too.

use std::collections::HashMap;
use std::time::Instant;

use crate::decode::DecodeError;
use crate::image::{Luma, Rgb};
use crate::layout::{CANVAS, SQUARE_TOP};
use crate::render::{RenderOptions, render};
use crate::session::{ScanLimits, ScanSession};
use crate::sim::{CaptureConfig, blend, capture, fit_to_frame, tear};
use crate::stream::StreamEncoder;

/// A named camera model with the pose range it is exercised over.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraModel {
    /// Camera and scene parameters (pose fields are re-drawn per trial).
    pub config: CaptureConfig,
    /// Largest tilt, in degrees, drawn on either axis.
    pub tilt_max_deg: f64,
}

impl CameraModel {
    /// Looks up `modern`, `legacy`, `worst` or `soft` (720p, σ = 1.8 px).
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        let (config, tilt_max_deg) = match name {
            "modern" => (CaptureConfig::modern(), 15.0),
            "legacy" => (CaptureConfig::legacy(), 15.0),
            "worst" => (CaptureConfig::worst(), 22.0),
            "soft" => (
                CaptureConfig {
                    blur_sigma: 1.8,
                    noise: 6.0,
                    fill: 0.8,
                    ..CaptureConfig::modern()
                },
                12.0,
            ),
            _ => return None,
        };
        Some(Self {
            config,
            tilt_max_deg,
        })
    }

    /// The names accepted by [`CameraModel::named`].
    pub const NAMES: [&'static str; 4] = ["modern", "legacy", "soft", "worst"];
}

/// Parameters of one trial.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StreamTrial {
    /// Camera model.
    pub camera: CameraModel,
    /// Frames the screen shows per second.
    pub display_fps: f64,
    /// Frames the camera delivers per second.
    pub camera_fps: f64,
    /// Exposure time of one camera frame in seconds.
    pub exposure_s: f64,
    /// Rolling-shutter readout from the first to the last row, in seconds: row `r` starts its
    /// exposure `r / height` of it after row 0. Zero exposes every row at once, and a frame that
    /// straddles a frame change is blended or torn as a whole.
    pub readout_s: f64,
    /// Give up after this many seconds.
    pub horizon_s: f64,
    /// Seed for pose, phase and noise.
    pub seed: u64,
    /// Width in pixels of the rendered source cards.
    pub render_size: usize,
    /// The receiving phone is held upright: cameras deliver landscape frames, so the code turns
    /// by 90° or 270° (± 15°) in the frame, the card's long side runs along the frame's long
    /// side and a rolling shutter sweeps across the card's width. Otherwise the code takes any
    /// rotation.
    pub upright: bool,
    /// The screen shows the cards with their katakana bands blacked out, so the camera gets
    /// what the square alone carries; the same seed then gives the same poses, phases and noise
    /// as with the bands shown.
    pub hide_bands: bool,
    /// A slow phone's decoder: when set, every push costs what it would cost there, and the
    /// session takes only the newest camera frame whenever it is free (dropping the frames that
    /// arrived while it worked), as `CameraX`'s keep-only-latest analyser does. When unset, every
    /// camera frame is decoded.
    pub decoder: Option<DecoderCost>,
}

/// What the parts of a push cost a phone's decoder, each in milliseconds (see
/// [`StreamTrial::decoder`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DecoderCost {
    /// A capture decoded by a full search.
    pub full_search: f64,
    /// A capture decoded by tracking the previous pose.
    pub tracked: f64,
    /// A try of the turn lanes.
    pub turns: f64,
    /// A try of one band's kana lanes.
    pub band: f64,
    /// A merge of held captures.
    pub merge: f64,
}

impl DecoderCost {
    /// A Pixel 6 held to its four Cortex-A55 cores at 1.8 GHz, the Kotlin SDK on ART (measured
    /// 2026-10-26 on simulated captures of 1280 × 720).
    pub const A55: Self = Self {
        full_search: 105.0,
        tracked: 14.5,
        turns: 75.0,
        band: 15.0,
        merge: 10.0,
    };
}

impl StreamTrial {
    /// A typical phone held upright: 30 fps camera, 1/60 s exposure, and the default 12 fps
    /// animation.
    #[must_use]
    pub fn typical(camera: CameraModel, seed: u64) -> Self {
        Self {
            camera,
            display_fps: 12.0,
            camera_fps: 30.0,
            exposure_s: 1.0 / 60.0,
            readout_s: 0.0,
            horizon_s: 90.0,
            seed,
            render_size: 768,
            upright: true,
            hide_bands: false,
            decoder: None,
        }
    }
}

/// Blacks out the rows of a rendered card above and below its square.
fn black_out_bands(card: &mut Rgb) {
    let unit = f64::from(CANVAS) / card.width as f64;
    let top = (f64::from(SQUARE_TOP) / unit).floor() as usize;
    let bottom = ((f64::from(SQUARE_TOP) + f64::from(CANVAS)) / unit).ceil() as usize;
    for y in (0..top).chain(bottom..card.height) {
        card.data[y * card.width * 3..(y + 1) * card.width * 3].fill(0);
    }
}

/// What a trial observed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrialOutcome {
    /// Seconds until the payload was complete, if it was.
    pub completed_at_s: Option<f64>,
    /// Whether the delivered payload equals the one sent.
    pub payload_matches: bool,
    /// Camera frames processed.
    pub frames: u32,
    /// Frames in which a code was located.
    pub located: u32,
    /// Frames in which lane `D` decoded.
    pub lane_d: u32,
    /// Field lanes decoded, summed over the frames (up to four per frame).
    pub field_lanes: u32,
    /// Turn lanes read, summed over the frames (each at most once per shown frame).
    pub turn_lanes: u32,
    /// Kana lanes read, summed over the frames (each band at most once per shown frame).
    pub kana_lanes: u32,
    /// Camera time the trial ran in seconds: until the camera frame after the last one decoded,
    /// or with a slow decoder until its last push ended.
    pub elapsed_s: f64,
    /// Mean decode time per frame in milliseconds.
    pub mean_decode_ms: f64,
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 11) as f64) / (1u64 << 53) as f64
    }

    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next()
    }
}

/// Plays `payload` through the simulated screen and camera.
///
/// # Panics
/// Panics when `payload` is empty or larger than the stream limit.
#[must_use]
pub fn run_stream_trial(payload: &[u8], kind: u8, trial: &StreamTrial) -> TrialOutcome {
    let encoder = StreamEncoder::new(payload, kind).expect("payload fits a stream");
    let mut rng = Rng(trial.seed ^ 0x5DEE_CE66_D1CE_4E5B);
    let mut config = trial.camera.config;
    config.seed = trial.seed.wrapping_add(1);
    config.rotation_deg = if trial.upright {
        let quarter = if rng.next() < 0.5 { 90.0 } else { 270.0 };
        quarter + rng.range(-15.0, 15.0)
    } else {
        rng.range(0.0, 360.0)
    };
    config.tilt_x_deg = rng.range(-trial.camera.tilt_max_deg, trial.camera.tilt_max_deg);
    config.tilt_y_deg = rng.range(-trial.camera.tilt_max_deg, trial.camera.tilt_max_deg);
    config.shift = (rng.range(-0.03, 0.03), rng.range(-0.03, 0.03));
    let config = fit_to_frame(&config, 4.0);
    let phase = rng.range(0.0, 1.0 / trial.display_fps);
    let mut sources: HashMap<u16, Rgb> = HashMap::new();
    let mut grab = |frame: u16, cfg: &CaptureConfig| -> Luma {
        let source = sources.entry(frame).or_insert_with(|| {
            let mut card = render(
                &encoder.cells(frame),
                &RenderOptions {
                    size: trial.render_size,
                    supersample: 2,
                    ..RenderOptions::default()
                },
            );
            if trial.hide_bands {
                black_out_bands(&mut card);
            }
            card
        });
        capture(source, cfg)
    };
    let mut session = ScanSession::new(ScanLimits::default());
    let mut decode_ms = 0.0;
    let mut outcome = TrialOutcome {
        completed_at_s: None,
        payload_matches: false,
        frames: 0,
        located: 0,
        lane_d: 0,
        field_lanes: 0,
        turn_lanes: 0,
        kana_lanes: 0,
        elapsed_s: 0.0,
        mean_decode_ms: 0.0,
    };
    let mut index = 0u32;
    // when a slow decoder's last push ends, in seconds
    let mut clock = 0.0f64;
    loop {
        let t = f64::from(index) / trial.camera_fps;
        if t > trial.horizon_s {
            break;
        }
        let slot = |time: f64| -> u16 { ((time + phase) * trial.display_fps).floor() as u16 };
        let (first, last) = (slot(t), slot(t + trial.exposure_s));
        let mut cfg = config;
        cfg.seed = config
            .seed
            .wrapping_mul(1000)
            .wrapping_add(u64::from(index));
        let image = if trial.readout_s > 0.0 {
            // rows before a frame change show the old frame, rows after it the new one and rows
            // whose exposure straddles it a blend of both
            let shots: Vec<Luma> = (first..=slot(t + trial.readout_s + trial.exposure_s))
                .map(|frame| grab(frame, &cfg))
                .collect();
            let (width, height) = (shots[0].width, shots[0].height);
            let mut data = vec![0u8; width * height];
            for (row, out) in data.chunks_exact_mut(width).enumerate() {
                let start = t + trial.readout_s * row as f64 / height as f64;
                let (a, b) = (slot(start), slot(start + trial.exposure_s));
                let line =
                    |frame: u16| &shots[usize::from(frame - first)].data[row * width..][..width];
                if a == b {
                    out.copy_from_slice(line(a));
                } else {
                    let boundary = f64::from(b) / trial.display_fps - phase;
                    let share =
                        ((start + trial.exposure_s - boundary) / trial.exposure_s).clamp(0.0, 1.0);
                    for ((o, &x), &y) in out.iter_mut().zip(line(a)).zip(line(b)) {
                        *o = (f64::from(x) * (1.0 - share) + f64::from(y) * share).round() as u8;
                    }
                }
            }
            Luma {
                width,
                height,
                data,
            }
        } else if first == last {
            grab(first, &cfg)
        } else {
            let boundary = (f64::from(last) / trial.display_fps - phase).max(t);
            let share = ((t + trial.exposure_s - boundary) / trial.exposure_s).clamp(0.0, 1.0);
            let (a, b) = (grab(first, &cfg), grab(last, &cfg));
            if index.is_multiple_of(2) {
                blend(&a, &b, share)
            } else {
                tear(&a, &b, ((1.0 - share) * a.height as f64) as usize)
            }
        };
        let before = session.stats();
        let started = Instant::now();
        let result = session.push(&image, (t * 1000.0) as u64);
        decode_ms += started.elapsed().as_secs_f64() * 1000.0;
        // on a slow decoder the push ends this much later, and the next push takes the newest
        // camera frame by then
        let done_at = trial.decoder.map_or(t, |cost| {
            let after = session.stats();
            let mut ms = if after.tracked > before.tracked {
                cost.tracked
            } else {
                cost.full_search
            };
            if after.turn_tries > before.turn_tries {
                ms += cost.turns;
            }
            ms += cost.band * f64::from(after.kana_tries - before.kana_tries);
            if after.merges > before.merges {
                ms += cost.merge;
            }
            clock = clock.max(t) + ms / 1000.0;
            clock
        });
        outcome.frames += 1;
        if !matches!(
            result.error,
            Some(DecodeError::NoFinders | DecodeError::UnsupportedImage)
        ) {
            outcome.located += 1;
        }
        outcome.lane_d += u32::from(result.lanes.contains('D'));
        outcome.field_lanes += result.lanes.chars().filter(char::is_ascii_digit).count() as u32;
        outcome.turn_lanes += result.turned.len() as u32;
        outcome.kana_lanes += (result.kana.len() / 2) as u32;
        outcome.elapsed_s = if trial.decoder.is_some() {
            done_at
        } else {
            f64::from(index + 1) / trial.camera_fps
        };
        if let Some(done) = result.completed {
            outcome.completed_at_s = Some(done_at);
            outcome.payload_matches = done.payload == payload;
            break;
        }
        index = if trial.decoder.is_some() {
            // the newest camera frame when the push ends, or the next one
            ((clock * trial.camera_fps).floor() as u32).max(index + 1)
        } else {
            index + 1
        };
    }
    outcome.mean_decode_ms = decode_ms / f64::from(outcome.frames.max(1));
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_modern_camera_receives_a_small_payload_quickly() {
        let payload: Vec<u8> = (0..120u16).map(|i| (i * 7) as u8).collect();
        let mut camera = CameraModel::named("modern").unwrap();
        camera.config.width = 480;
        camera.config.height = 360;
        let trial = StreamTrial {
            horizon_s: 20.0,
            render_size: 384,
            ..StreamTrial::typical(camera, 5)
        };
        let outcome = run_stream_trial(&payload, 2, &trial);
        let finished = outcome.completed_at_s.expect("payload arrives");
        assert!(finished < 5.0, "took {finished} s");
        assert!(outcome.payload_matches);
        assert!(outcome.lane_d > 0 && outcome.field_lanes > 0);
    }

    #[test]
    fn a_rolling_shutter_camera_receives_a_small_payload() {
        let payload: Vec<u8> = (0..120u16).map(|i| (i * 7) as u8).collect();
        let mut camera = CameraModel::named("modern").unwrap();
        camera.config.width = 480;
        camera.config.height = 360;
        let trial = StreamTrial {
            display_fps: 12.0,
            exposure_s: 0.008,
            readout_s: 0.03,
            horizon_s: 20.0,
            render_size: 384,
            ..StreamTrial::typical(camera, 6)
        };
        let outcome = run_stream_trial(&payload, 2, &trial);
        assert!(
            outcome.completed_at_s.is_some_and(|t| t < 5.0),
            "{outcome:?}"
        );
        assert!(outcome.payload_matches);
    }

    #[test]
    fn hiding_the_bands_blacks_out_only_the_rows_beyond_the_square() {
        let shown = render(
            &StreamEncoder::new(&[1, 2, 3], 2).unwrap().cells(0),
            &RenderOptions {
                size: 256,
                ..RenderOptions::default()
            },
        );
        let mut hidden = shown.clone();
        black_out_bands(&mut hidden);
        let row = shown.width * 3;
        let rows = |card: &Rgb, from: usize, to: usize| card.data[from * row..to * row].to_vec();
        // the square spans rows 80.5 to 336.5 of the 417-row card
        let (top, bottom) = (80, 337);
        assert!(rows(&shown, 0, top).iter().any(|&v| v > 0));
        assert!(rows(&shown, bottom, shown.height).iter().any(|&v| v > 0));
        assert!(rows(&hidden, 0, top).iter().all(|&v| v == 0));
        assert!(rows(&hidden, bottom, hidden.height).iter().all(|&v| v == 0));
        assert_eq!(rows(&hidden, top, bottom), rows(&shown, top, bottom));
    }

    #[test]
    fn a_camera_reads_no_kana_lanes_from_hidden_bands() {
        let payload: Vec<u8> = (0..600u16).map(|i| (i * 7) as u8).collect();
        let camera = CameraModel::named("modern").unwrap();
        let trial = StreamTrial {
            horizon_s: 20.0,
            ..StreamTrial::typical(camera, 5)
        };
        let shown = run_stream_trial(&payload, 2, &trial);
        let hidden = run_stream_trial(
            &payload,
            2,
            &StreamTrial {
                hide_bands: true,
                ..trial
            },
        );
        assert!(shown.payload_matches && shown.kana_lanes > 0, "{shown:?}");
        assert!(
            hidden.payload_matches && hidden.kana_lanes == 0,
            "{hidden:?}"
        );
        assert!(hidden.field_lanes > 0);
    }

    #[test]
    fn camera_names_resolve() {
        for name in CameraModel::NAMES {
            assert!(CameraModel::named(name).is_some(), "{name}");
        }
        assert!(CameraModel::named("potato").is_none());
    }
}
