//! The receive-side object an app holds while its camera is open.

use crate::decode::{DecodeError, DecodeOptions, DecodedFrame, decode};
use crate::image::Luma;
use crate::stream::{AssemblerLimits, Completed, Progress, StreamAssembler};

/// Limits of a scan session.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScanLimits {
    /// Forget a half-received stream after this long without progress.
    pub idle_timeout_ms: u64,
    /// Forget a stream that has not finished this long after it started.
    pub absolute_timeout_ms: u64,
    /// Assembler memory and size limits.
    pub assembler: AssemblerLimits,
    /// Image decoder options.
    pub decode: DecodeOptions,
}

impl Default for ScanLimits {
    fn default() -> Self {
        Self {
            idle_timeout_ms: 30_000,
            absolute_timeout_ms: 180_000,
            assembler: AssemblerLimits::default(),
            decode: DecodeOptions::default(),
        }
    }
}

/// Counters for diagnostics and UI hints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScanStats {
    /// Camera frames offered.
    pub frames: u32,
    /// Frames in which a code was located, whether or not a lane could be read.
    pub located: u32,
    /// Frames in which at least one lane decoded.
    pub readable: u32,
    /// Lane `P` successes.
    pub lane_p: u32,
    /// Lane `K` successes.
    pub lane_k: u32,
    /// Lane `D` successes.
    pub lane_d: u32,
}

/// The result of offering one camera frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanOutcome {
    /// Why the frame produced nothing, when it did not. `NoOrientation` means a
    /// code was located but no lane could be read (too far, too blurry).
    pub error: Option<DecodeError>,
    /// Lanes that decoded, as letters from `"PKD"`.
    pub lanes: String,
    /// Receive progress after this frame.
    pub progress: Progress,
    /// The finished payload, delivered exactly once.
    pub completed: Option<Completed>,
}

/// Decodes camera frames and reassembles the stream they carry.
pub struct ScanSession {
    limits: ScanLimits,
    assembler: StreamAssembler,
    stats: ScanStats,
    started_ms: Option<u64>,
    progress_ms: u64,
    last_rank: usize,
}

impl ScanSession {
    /// Creates a session.
    #[must_use]
    pub fn new(limits: ScanLimits) -> Self {
        Self {
            assembler: StreamAssembler::new(limits.assembler),
            limits,
            stats: ScanStats::default(),
            started_ms: None,
            progress_ms: 0,
            last_rank: 0,
        }
    }

    /// Diagnostic counters.
    #[must_use]
    pub fn stats(&self) -> ScanStats {
        self.stats
    }

    /// Current progress.
    #[must_use]
    pub fn progress(&self) -> Progress {
        self.assembler.progress()
    }

    /// Drops all partial state.
    pub fn reset(&mut self) {
        self.assembler.reset();
        self.started_ms = None;
        self.last_rank = 0;
    }

    /// Offers one camera luma plane captured at monotonic time `now_ms`.
    pub fn push(&mut self, image: &Luma, now_ms: u64) -> ScanOutcome {
        if let Some(start) = self.started_ms
            && (now_ms.saturating_sub(self.progress_ms) > self.limits.idle_timeout_ms
                || now_ms.saturating_sub(start) > self.limits.absolute_timeout_ms)
        {
            self.reset();
        }
        self.stats.frames += 1;
        let result = decode(image, &self.limits.decode);
        let (error, lanes) = match &result {
            Ok(frame) => (None, self.absorb(frame, now_ms)),
            Err(error) => (Some(*error), String::new()),
        };
        if !matches!(
            error,
            Some(DecodeError::NoFinders | DecodeError::UnsupportedImage)
        ) {
            self.stats.located += 1;
        }
        let progress = self.assembler.progress();
        if progress.rank > self.last_rank || (progress.meta.is_some() && self.started_ms.is_none())
        {
            self.progress_ms = now_ms;
            self.started_ms.get_or_insert(now_ms);
        }
        self.last_rank = progress.rank;
        ScanOutcome {
            error,
            lanes,
            progress,
            completed: self.assembler.take_completed(),
        }
    }

    fn absorb(&mut self, frame: &DecodedFrame, _now_ms: u64) -> String {
        let mut lanes = String::new();
        for (letter, present, counter) in [
            ('P', frame.p.is_some(), &mut self.stats.lane_p),
            ('K', frame.k.is_some(), &mut self.stats.lane_k),
            ('D', frame.d.is_some(), &mut self.stats.lane_d),
        ] {
            if present {
                lanes.push(letter);
                *counter += 1;
            }
        }
        if !lanes.is_empty() {
            self.stats.readable += 1;
        }
        frame.feed(&mut self.assembler);
        lanes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{RenderOptions, render};
    use crate::sim::{CaptureConfig, capture, fit_to_frame};
    use crate::stream::StreamEncoder;

    fn payload(len: usize) -> Vec<u8> {
        let mut rng = crate::prng::Xorshift32::new(3);
        (0..len).map(|_| rng.next_byte()).collect()
    }

    #[test]
    fn a_session_receives_a_payload_from_simulated_captures() {
        let data = payload(500);
        let encoder = StreamEncoder::new(&data, 2).unwrap();
        let config = fit_to_frame(
            &CaptureConfig {
                width: 640,
                height: 480,
                rotation_deg: 20.0,
                ..CaptureConfig::modern()
            },
            4.0,
        );
        let mut session = ScanSession::new(ScanLimits::default());
        let mut done = None;
        for frame in 0..40u16 {
            let source = render(
                &encoder.cells(frame),
                &RenderOptions {
                    size: 512,
                    supersample: 2,
                    ..RenderOptions::default()
                },
            );
            let outcome = session.push(&capture(&source, &config), u64::from(frame) * 125);
            if outcome.completed.is_some() {
                done = outcome.completed;
                break;
            }
        }
        let done = done.expect("completed");
        assert_eq!(done.payload, data);
        assert_eq!(done.meta.kind, 2);
        assert!(session.stats().readable > 0 && session.stats().lane_d > 0);
    }

    #[test]
    fn idle_sessions_forget_partial_streams() {
        let encoder = StreamEncoder::new(&payload(4000), 1).unwrap();
        let config = fit_to_frame(
            &CaptureConfig {
                width: 640,
                height: 480,
                ..CaptureConfig::modern()
            },
            4.0,
        );
        let limits = ScanLimits {
            idle_timeout_ms: 1_000,
            ..ScanLimits::default()
        };
        let mut session = ScanSession::new(limits);
        let render_frame = |frame: u16| {
            let source = render(
                &encoder.cells(frame),
                &RenderOptions {
                    size: 512,
                    supersample: 2,
                    ..RenderOptions::default()
                },
            );
            capture(&source, &config)
        };
        session.push(&render_frame(0), 0);
        assert!(session.progress().rank > 0);
        // a frame much later with nothing readable resets the session first
        let blank = Luma::new(640, 480);
        let outcome = session.push(&blank, 60_000);
        assert_eq!(outcome.error, Some(DecodeError::NoFinders));
        assert_eq!(outcome.progress.rank, 0);
    }

    #[test]
    fn located_counts_codes_that_were_seen_but_could_not_be_read() {
        let encoder = StreamEncoder::new(&payload(100), 1).unwrap();
        let mut frame = render(
            &encoder.cells(1),
            &RenderOptions {
                size: 512,
                supersample: 2,
                ..RenderOptions::default()
            },
        );
        // keep only the four blossoms: finders are located, no lane can be read
        let scale = 512.0 / 1024.0;
        for y in 0..512usize {
            for x in 0..512usize {
                let near_finder = crate::layout::FINDER_CENTERS.iter().any(|&(fx, fy)| {
                    ((x as f64 - f64::from(fx) * scale).powi(2)
                        + (y as f64 - f64::from(fy) * scale).powi(2))
                    .sqrt()
                        < 34.0
                });
                if !near_finder {
                    frame.data[(y * 512 + x) * 3..(y * 512 + x) * 3 + 3].fill(0);
                }
            }
        }
        let mut session = ScanSession::new(ScanLimits::default());
        let outcome = session.push(&frame.to_luma(), 0);
        assert_eq!(outcome.error, Some(DecodeError::NoOrientation));
        assert_eq!(session.stats().located, 1);
        assert_eq!(session.stats().readable, 0);
        // a frame with no code at all is not "located"
        session.push(&Luma::new(320, 240), 100);
        assert_eq!(session.stats().located, 1);
        assert_eq!(session.stats().frames, 2);
    }

    #[test]
    fn unreadable_frames_do_not_disturb_progress() {
        let mut session = ScanSession::new(ScanLimits::default());
        let outcome = session.push(&Luma::new(320, 240), 5);
        assert!(outcome.completed.is_none() && outcome.lanes.is_empty());
        assert_eq!(session.stats().frames, 1);
        assert_eq!(session.stats().located, 0);
    }
}
