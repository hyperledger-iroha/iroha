//! End-to-end stream trials against the simulated camera.
//!
//! One trial plays a stream on a virtual screen at `display_fps`, films it with
//! a virtual phone camera (frame blending and rolling-shutter tearing whenever
//! an exposure straddles a frame change) and runs the frames through a
//! [`ScanSession`] until the payload arrives or
//! the horizon is reached. Operators use it through `iroha offline petal
//! simulate`; the repository's qualification examples use it too.

use std::collections::HashMap;
use std::time::Instant;

use crate::decode::DecodeError;
use crate::image::{Luma, Rgb};
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
    /// Give up after this many seconds.
    pub horizon_s: f64,
    /// Seed for pose, phase and noise.
    pub seed: u64,
    /// Side in pixels of the rendered source frames.
    pub render_size: usize,
}

impl StreamTrial {
    /// A typical phone: 30 fps camera, 1/60 s exposure, 8 fps animation.
    #[must_use]
    pub fn typical(camera: CameraModel, seed: u64) -> Self {
        Self {
            camera,
            display_fps: 8.0,
            camera_fps: 30.0,
            exposure_s: 1.0 / 60.0,
            horizon_s: 90.0,
            seed,
            render_size: 768,
        }
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
    /// Frames in which lane `P`, `K`, `D` decoded.
    pub lanes: [u32; 3],
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
    config.rotation_deg = rng.range(0.0, 360.0);
    config.tilt_x_deg = rng.range(-trial.camera.tilt_max_deg, trial.camera.tilt_max_deg);
    config.tilt_y_deg = rng.range(-trial.camera.tilt_max_deg, trial.camera.tilt_max_deg);
    config.shift = (rng.range(-0.03, 0.03), rng.range(-0.03, 0.03));
    let config = fit_to_frame(&config, 4.0);
    let phase = rng.range(0.0, 1.0 / trial.display_fps);
    let mut sources: HashMap<u16, Rgb> = HashMap::new();
    let mut grab = |frame: u16, cfg: &CaptureConfig| -> Luma {
        let source = sources.entry(frame).or_insert_with(|| {
            render(
                &encoder.cells(frame),
                &RenderOptions {
                    size: trial.render_size,
                    supersample: 2,
                    ..RenderOptions::default()
                },
            )
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
        lanes: [0; 3],
        mean_decode_ms: 0.0,
    };
    let mut index = 0u32;
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
        let image = if first == last {
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
        let started = Instant::now();
        let result = session.push(&image, (t * 1000.0) as u64);
        decode_ms += started.elapsed().as_secs_f64() * 1000.0;
        outcome.frames += 1;
        if !matches!(
            result.error,
            Some(DecodeError::NoFinders | DecodeError::UnsupportedImage)
        ) {
            outcome.located += 1;
        }
        for (lane, letter) in ['P', 'K', 'D'].into_iter().enumerate() {
            if result.lanes.contains(letter) {
                outcome.lanes[lane] += 1;
            }
        }
        if let Some(done) = result.completed {
            outcome.completed_at_s = Some(t);
            outcome.payload_matches = done.payload == payload;
            break;
        }
        index += 1;
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
        assert!(outcome.lanes[2] > 0);
    }

    #[test]
    fn camera_names_resolve() {
        for name in CameraModel::NAMES {
            assert!(CameraModel::named(name).is_some(), "{name}");
        }
        assert!(CameraModel::named("potato").is_none());
    }
}
