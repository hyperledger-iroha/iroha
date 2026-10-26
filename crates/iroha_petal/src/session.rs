//! The receive-side object an app holds while its camera is open.
//!
//! After a frame decodes, the next frames are first read by [`track`]ing the
//! code from its last pose, which skips the finder search; a full [`decode`]
//! runs when tracking fails or the last pose is older than
//! [`TRACK_WINDOW_MS`].
//!
//! A camera sees each shown frame two to four times, and noise, display
//! banding or hand shake can cost every one of those captures a field lane.
//! When a capture reads some lane but not every field lane, the session keeps
//! its [`FieldEvidence`] under the shown frame its decoded lane headers name, for
//! the last [`MERGE_WINDOW`] shown frames. From the second such capture on, it
//! merges them ([`merge_fields`]) and offers a recovered lane only when that
//! lane's own header names the same frame.
//!
//! Every field lane a capture decodes also lets the session read the turn lane
//! on top of it ([`read_turns`]). A turn lane is tried once per shown frame: the
//! session remembers the last [`TURN_MEMORY`] it tried and does not look at those
//! petals again. Reading turns costs a slow phone core several times a frame's
//! decode, so after a try that reads no turn lane (a camera too soft or too
//! shaky for the turns) the session passes over the next captures that would
//! try one, one more after every such try, at most [`TURN_BACKOFF`], until a
//! try reads a lane again.
//!
//! The katakana bands above and below the square are read the same way
//! ([`read_kana`]): each band once per shown frame (the last [`KANA_MEMORY`]
//! tried are remembered), with its own back-off of at most [`KANA_BACKOFF`]
//! captures after tries that read no kana lane (a camera that does not see the
//! bands, or too blurred for their glyphs and polarity).
//!
//! The bands go first. A capture whose bands read a kana lane tries the turn lanes
//! too only when it has time to spare: when the work it has done (finding or
//! following the code, a merge, the bands) and what the session's last turn try
//! took fit together in [`CAPTURE_BUDGET_MS`], a 30 fps camera's frame time
//! (before the first turn try, twice the work done stands in for it). Otherwise the
//! turn lanes of that shown frame wait for its next capture. On a slow phone core a
//! turn try costs five tracked decodes and a band try one, so reading both on one
//! capture overran the time a frame is shown at 12 fps and the session missed the
//! next shown frames; a fast decoder reads both at once, which matters when its
//! camera gives it few frames a second. A band try that reads nothing (bands out of
//! view, too blurred, or a torn capture) leaves the turn lanes to the same capture.
//!
//! The session times its work on the monotonic clock. [`ScanSession::with_work_cost`]
//! puts a model's costs in place of the measured times, as the simulator's model of a
//! slow phone and the tests do.

use std::collections::VecDeque;
use std::time::Instant;

use crate::decode::{
    DecodeError, DecodeOptions, DecodedFrame, FieldEvidence, decode, field_evidence, merge_fields,
    track,
};
use crate::image::Luma;
use crate::kana::{KanaKind, KanaLane, kana_packet, read_kana};
use crate::lanes::Lane;
use crate::layout::{KANA_BANDS, SECTORS};
use crate::stream::{
    AssemblerLimits, Completed, LaneHeader, Progress, StreamAssembler, parse_atom_lane,
};
use crate::turns::{read_turns, turn_packet};

/// How long a decoded pose stays usable for tracking the next frames.
pub const TRACK_WINDOW_MS: u64 = 500;
/// Shown frames whose captures are held for merging.
pub const MERGE_WINDOW: usize = 6;
/// Captures held per shown frame; a further capture replaces the oldest.
pub const MERGE_CAPTURES: usize = 4;
/// Turn lanes remembered as tried (the four of each of the last [`MERGE_WINDOW`] shown frames).
pub const TURN_MEMORY: usize = MERGE_WINDOW * SECTORS;
/// Most captures passed over after tries that read no turn lane.
pub const TURN_BACKOFF: usize = 8;
/// Bands remembered as tried (the two of each of the last [`MERGE_WINDOW`] shown frames).
pub const KANA_MEMORY: usize = MERGE_WINDOW * KANA_BANDS;
/// Most captures passed over after tries that read no kana lane.
pub const KANA_BACKOFF: usize = 8;
/// The time a capture's work may take, turn lanes included, for a capture whose bands read a
/// lane to try the turn lanes too: a 30 fps camera's frame time, so a decoder that keeps to it
/// misses no camera frame.
pub const CAPTURE_BUDGET_MS: f64 = 1000.0 / 30.0;

/// A part of a capture's work, as the session times it to tell whether a capture has time to
/// spare for the turn lanes (see the module docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Work {
    /// Finding the code by a full search and taking in the lanes read.
    Search,
    /// Following the code from the previous capture's pose and taking in the lanes read.
    Track,
    /// Merging the held captures of a shown frame.
    Merge,
    /// Reading the kana lanes of this many bands.
    Bands(u32),
    /// Reading the turn lanes.
    Turns,
}

/// Limits of a scan session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// Lane `D` successes.
    pub lane_d: u32,
    /// Field lane successes (up to four per frame).
    pub field_lanes: u32,
    /// Frames read by tracking the previous pose instead of a full search.
    pub tracked: u32,
    /// Frames read with one corner finder hidden and inferred.
    pub inferred: u32,
    /// Merges of two or more captures of one shown frame.
    pub merges: u32,
    /// Field lanes recovered by merging captures.
    pub merged_lanes: u32,
    /// Turn lanes tried (each at most once per shown frame).
    pub turn_tries: u32,
    /// Turn lanes read.
    pub turn_lanes: u32,
    /// Bands tried (each at most once per shown frame).
    pub kana_tries: u32,
    /// Kana lanes read (up to four per shown frame).
    pub kana_lanes: u32,
    /// Captures whose bands read a lane that also tried the turn lanes, having time to spare.
    pub spare_turns: u32,
}

/// The result of offering one camera frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanOutcome {
    /// Why the frame produced nothing, when it did not. `NoOrientation` means a
    /// code was located but no lane could be read (too far, too blurry).
    pub error: Option<DecodeError>,
    /// Lanes that decoded in this capture alone, as letters: `D` and the sector
    /// digits `0`–`3`.
    pub lanes: String,
    /// Field lanes recovered by merging this capture with earlier captures of its
    /// shown frame, as sector digits.
    pub merged: String,
    /// Turn lanes read from this capture, as the sector digits of the field lanes
    /// they ride on.
    pub turned: String,
    /// Kana lanes read from this capture, as their names (`P0`, `K0`, `P1`, `K1`).
    pub kana: String,
    /// Receive progress after this frame.
    pub progress: Progress,
    /// The finished payload, delivered exactly once.
    pub completed: Option<Completed>,
}

/// The captures of one shown frame held for merging.
struct Held {
    shown: LaneHeader,
    evidence: VecDeque<FieldEvidence>,
    done: [bool; SECTORS],
}

/// The header every lane starts with.
fn lane_header(data: &[u8]) -> Option<LaneHeader> {
    match *data {
        [tag, high, low, ..] => Some(LaneHeader {
            tag,
            frame: u16::from_be_bytes([high, low]),
        }),
        _ => None,
    }
}

/// The shown frame every decoded lane header of `frame` names; `None` when no lane
/// decoded or the lanes of a torn capture name different frames.
fn shown_frame(frame: &DecodedFrame) -> Option<LaneHeader> {
    let mut shown = None;
    for lane in Lane::ALL {
        let Some(result) = frame.lane(lane) else {
            continue;
        };
        let header = lane_header(&result.data)?;
        if shown.is_some_and(|s| s != header) {
            return None;
        }
        shown = Some(header);
    }
    shown
}

/// Decodes camera frames and reassembles the stream they carry.
pub struct ScanSession {
    limits: ScanLimits,
    assembler: StreamAssembler,
    stats: ScanStats,
    started_ms: Option<u64>,
    progress_ms: u64,
    last_rank: usize,
    last_pose: Option<(DecodedFrame, u64)>,
    held: VecDeque<Held>,
    tried: VecDeque<(LaneHeader, u8)>,
    turn_misses: usize,
    turn_wait: usize,
    kana_tried: VecDeque<(LaneHeader, u8)>,
    kana_misses: usize,
    kana_wait: usize,
    /// What a part of a capture cost, given the milliseconds it took.
    work_cost: Box<dyn Fn(Work, f64) -> f64 + Send>,
    /// What the last turn try cost, in milliseconds.
    turn_cost_ms: Option<f64>,
}

impl ScanSession {
    /// Creates a session that times its work on the monotonic clock.
    #[must_use]
    pub fn new(limits: ScanLimits) -> Self {
        Self::with_work_cost(limits, |_, measured_ms| measured_ms)
    }

    /// Creates a session whose work costs what `work_cost` says, given the part of a capture and
    /// the milliseconds it took: a model of a decoder's speed in place of the measured time.
    #[must_use]
    pub fn with_work_cost(
        limits: ScanLimits,
        work_cost: impl Fn(Work, f64) -> f64 + Send + 'static,
    ) -> Self {
        Self {
            assembler: StreamAssembler::new(limits.assembler),
            limits,
            stats: ScanStats::default(),
            started_ms: None,
            progress_ms: 0,
            last_rank: 0,
            last_pose: None,
            held: VecDeque::new(),
            tried: VecDeque::new(),
            turn_misses: 0,
            turn_wait: 0,
            kana_tried: VecDeque::new(),
            kana_misses: 0,
            kana_wait: 0,
            work_cost: Box::new(work_cost),
            turn_cost_ms: None,
        }
    }

    /// What `work` cost, given the time since `started`.
    fn cost(&self, work: Work, started: Instant) -> f64 {
        (self.work_cost)(work, started.elapsed().as_secs_f64() * 1000.0)
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
        self.last_pose = None;
        self.held.clear();
        self.forget_turns();
    }

    /// Forgets the turn lanes and bands tried and any back-off.
    fn forget_turns(&mut self) {
        self.tried.clear();
        self.turn_misses = 0;
        self.turn_wait = 0;
        self.kana_tried.clear();
        self.kana_misses = 0;
        self.kana_wait = 0;
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
        let started = Instant::now();
        let tracked = self
            .last_pose
            .as_ref()
            .filter(|(_, at)| now_ms.saturating_sub(*at) <= TRACK_WINDOW_MS)
            .and_then(|(previous, _)| track(image, previous, &self.limits.decode));
        let followed = tracked.is_some();
        if followed {
            self.stats.tracked += 1;
        }
        let result = tracked
            .ok_or(())
            .or_else(|()| decode(image, &self.limits.decode));
        let (error, lanes, merged, turned, kana) = match &result {
            Ok(frame) => {
                if frame.inferred_corner.is_some() {
                    self.stats.inferred += 1;
                }
                self.last_pose = Some((frame.clone(), now_ms));
                let lanes = self.absorb(frame, now_ms);
                let mut spent =
                    self.cost(if followed { Work::Track } else { Work::Search }, started);
                let (merges, started) = (self.stats.merges, Instant::now());
                let merged = self.merge(image, frame);
                if self.stats.merges > merges {
                    spent += self.cost(Work::Merge, started);
                }
                let (tries, started) = (self.stats.kana_tries, Instant::now());
                let kana = self.kana(image, frame);
                let bands = self.stats.kana_tries - tries;
                if bands > 0 {
                    spent += self.cost(Work::Bands(bands), started);
                }
                // a capture whose bands read a lane tries the turn lanes only with time to spare
                // (see the module docs)
                let turned = if kana.is_empty() {
                    self.turn(image, frame)
                } else if spent + self.turn_cost_ms.unwrap_or(2.0 * spent) <= CAPTURE_BUDGET_MS {
                    let tries = self.stats.turn_tries;
                    let turned = self.turn(image, frame);
                    if self.stats.turn_tries > tries {
                        self.stats.spare_turns += 1;
                    }
                    turned
                } else {
                    String::new()
                };
                (None, lanes, merged, turned, kana)
            }
            Err(error) => (
                Some(*error),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
            ),
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
            merged,
            turned,
            kana,
            progress,
            completed: self.assembler.take_completed(),
        }
    }

    fn absorb(&mut self, frame: &DecodedFrame, _now_ms: u64) -> String {
        let lanes = frame.letters();
        self.stats.lane_d += u32::from(frame.d.is_some());
        self.stats.field_lanes += frame.fields.iter().filter(|f| f.is_some()).count() as u32;
        if !lanes.is_empty() {
            self.stats.readable += 1;
        }
        let previous_meta = self.assembler.progress().meta;
        frame.feed(&mut self.assembler);
        if previous_meta.is_some() && self.assembler.progress().meta != previous_meta {
            self.held.clear();
            self.forget_turns();
        }
        lanes
    }

    /// Tries the turn lanes on top of the field lanes this capture decoded, except those already
    /// tried for their shown frame, and offers the atoms of those it reads; after tries that read
    /// nothing it passes over the next captures that would try one (see the module docs).
    fn turn(&mut self, image: &Luma, frame: &DecodedFrame) -> String {
        let mut skip = [true; SECTORS];
        let mut headers = [None; SECTORS];
        for (sector, field) in frame.fields.iter().enumerate() {
            let Some(header) = field.as_ref().and_then(|f| lane_header(&f.data)) else {
                continue;
            };
            headers[sector] = Some(header);
            skip[sector] = self.tried.contains(&(header, sector as u8));
        }
        if skip.iter().all(|s| *s) {
            return String::new();
        }
        if self.turn_wait > 0 {
            self.turn_wait -= 1;
            return String::new();
        }
        for (sector, header) in headers.iter().enumerate() {
            if let (false, Some(header)) = (skip[sector], header) {
                if self.tried.len() == TURN_MEMORY {
                    self.tried.pop_front();
                }
                self.tried.push_back((*header, sector as u8));
                self.stats.turn_tries += 1;
            }
        }
        let started = Instant::now();
        let results = read_turns(image, frame, skip);
        self.turn_cost_ms = Some(self.cost(Work::Turns, started));
        let mut turned = String::new();
        for (sector, result) in results.into_iter().enumerate() {
            let (Some(result), Some(header)) = (result, headers[sector]) else {
                continue;
            };
            let Some(packet) = turn_packet(header, sector as u8, &result.data) else {
                continue;
            };
            self.assembler.push_atoms(&packet);
            self.stats.turn_lanes += 1;
            turned.push(Lane::Field(sector as u8).letter());
        }
        if turned.is_empty() {
            self.turn_misses = (self.turn_misses + 1).min(TURN_BACKOFF);
            self.turn_wait = self.turn_misses;
        } else {
            self.turn_misses = 0;
        }
        turned
    }

    /// Tries the bands not yet tried for this capture's shown frame and offers the atoms of the
    /// kana lanes they read; after tries that read nothing it passes over the next captures that
    /// would try one (see the module docs).
    fn kana(&mut self, image: &Luma, frame: &DecodedFrame) -> String {
        let Some(header) = shown_frame(frame) else {
            return String::new();
        };
        let skip: [bool; KANA_BANDS] =
            core::array::from_fn(|band| self.kana_tried.contains(&(header, band as u8)));
        if skip.iter().all(|s| *s) {
            return String::new();
        }
        if self.kana_wait > 0 {
            self.kana_wait -= 1;
            return String::new();
        }
        for (band, &skipped) in skip.iter().enumerate() {
            if !skipped {
                if self.kana_tried.len() == KANA_MEMORY {
                    self.kana_tried.pop_front();
                }
                self.kana_tried.push_back((header, band as u8));
                self.stats.kana_tries += 1;
            }
        }
        let mut read = String::new();
        for (band, results) in read_kana(image, frame, header, skip)
            .into_iter()
            .enumerate()
        {
            for (kind, result) in [KanaKind::Polarity, KanaKind::Glyph]
                .into_iter()
                .zip(results)
            {
                let Some(result) = result else {
                    continue;
                };
                let lane = KanaLane {
                    band: band as u8,
                    kind,
                };
                let Some(packet) = kana_packet(result.header, lane, &result.result.data) else {
                    continue;
                };
                self.assembler.push_atoms(&packet);
                self.stats.kana_lanes += 1;
                read.push_str(&lane.name());
            }
        }
        if read.is_empty() {
            self.kana_misses = (self.kana_misses + 1).min(KANA_BACKOFF);
            self.kana_wait = self.kana_misses;
        } else {
            self.kana_misses = 0;
        }
        read
    }

    /// Holds this capture's field evidence under its shown frame and, from the second
    /// capture on, merges the held captures of that frame.
    fn merge(&mut self, image: &Luma, frame: &DecodedFrame) -> String {
        let Some(shown) = shown_frame(frame) else {
            return String::new();
        };
        let index = if let Some(index) = self.held.iter().position(|h| h.shown == shown) {
            index
        } else {
            if self.held.len() == MERGE_WINDOW {
                self.held.pop_front();
            }
            self.held.push_back(Held {
                shown,
                evidence: VecDeque::new(),
                done: [false; SECTORS],
            });
            self.held.len() - 1
        };
        let held = &mut self.held[index];
        for (done, field) in held.done.iter_mut().zip(&frame.fields) {
            *done |= field.is_some();
        }
        if held.done.iter().all(|d| *d) {
            held.evidence.clear();
            return String::new();
        }
        let Some(evidence) = field_evidence(image, frame, &self.limits.decode) else {
            return String::new();
        };
        if held.evidence.len() == MERGE_CAPTURES {
            held.evidence.pop_front();
        }
        held.evidence.push_back(evidence);
        if held.evidence.len() < 2 {
            return String::new();
        }
        self.stats.merges += 1;
        let results = merge_fields(held.evidence.make_contiguous(), held.done);
        let mut merged = String::new();
        for (lane, result) in Lane::FIELDS.into_iter().zip(results) {
            let Lane::Field(sector) = lane else {
                continue;
            };
            let Some(packet) = result.and_then(|r| parse_atom_lane(lane, &r.data)) else {
                continue;
            };
            // a merge offers only atoms of the shown frame it was made for
            if packet.header != shown {
                continue;
            }
            held.done[usize::from(sector)] = true;
            self.assembler.push_atoms(&packet);
            self.stats.merged_lanes += 1;
            merged.push(lane.letter());
        }
        if held.done.iter().all(|d| *d) {
            held.evidence.clear();
        }
        merged
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::LaneResult;
    use crate::image::Rgb;
    use crate::lanes::FrameCells;
    use crate::render::{RenderOptions, render};
    use crate::sim::{CaptureConfig, capture, fit_to_frame};
    use crate::stream::StreamEncoder;

    fn payload(len: usize) -> Vec<u8> {
        let mut rng = crate::prng::Xorshift32::new(3);
        (0..len).map(|_| rng.next_byte()).collect()
    }

    fn source(cells: &FrameCells) -> Rgb {
        render(
            cells,
            &RenderOptions {
                size: 512,
                supersample: 2,
                ..RenderOptions::default()
            },
        )
    }

    fn camera(rotation_deg: f64) -> CaptureConfig {
        fit_to_frame(
            &CaptureConfig {
                width: 640,
                height: 480,
                rotation_deg,
                ..CaptureConfig::modern()
            },
            4.0,
        )
    }

    /// A phone held upright: landscape frames with the card turned a quarter.
    fn upright() -> CaptureConfig {
        fit_to_frame(
            &CaptureConfig {
                rotation_deg: 92.0,
                ..CaptureConfig::modern()
            },
            4.0,
        )
    }

    /// Card `frame` of `encoder` with its bands, 768 wide.
    fn card(encoder: &StreamEncoder, frame: u16) -> Rgb {
        render(
            &encoder.cells(frame),
            &RenderOptions {
                size: 768,
                supersample: 2,
                ..RenderOptions::default()
            },
        )
    }

    /// A session whose work costs what `costs` says, however long it took.
    fn costed(costs: fn(Work) -> f64) -> ScanSession {
        ScanSession::with_work_cost(ScanLimits::default(), move |work, _| costs(work))
    }

    #[test]
    fn an_upright_camera_reads_both_bands_once_per_shown_frame() {
        let encoder = StreamEncoder::new(&payload(3000), 2).unwrap();
        // a fast decoder: every capture has time to spare for the turn lanes too
        let mut session = costed(|_| 0.0);
        let first = session.push(&capture(&card(&encoder, 1), &upright()), 0);
        assert_eq!(
            (first.kana.as_str(), first.turned.as_str()),
            ("P0K0P1K1", "0123")
        );
        // a second capture of the same shown frame tries no band and no turn lane again
        let again = session.push(&capture(&card(&encoder, 1), &upright()), 33);
        assert_eq!((again.kana.as_str(), again.turned.as_str()), ("", ""));
        let next = session.push(&capture(&card(&encoder, 2), &upright()), 83);
        assert_eq!(
            (next.kana.as_str(), next.turned.as_str()),
            ("P0K0P1K1", "0123")
        );
        let stats = session.stats();
        assert_eq!((stats.kana_tries, stats.kana_lanes), (4, 8));
        assert_eq!((stats.turn_lanes, stats.spare_turns), (8, 2));
    }

    #[test]
    fn a_slow_decoder_leaves_the_turn_lanes_to_the_next_capture() {
        let encoder = StreamEncoder::new(&payload(3000), 2).unwrap();
        // a slow phone core: the bands take the first capture's spare time
        let mut session = costed(|work| crate::qualify::DecoderCost::A55.of(work));
        let first = session.push(&capture(&card(&encoder, 1), &upright()), 0);
        assert_eq!(
            (first.kana.as_str(), first.turned.as_str()),
            ("P0K0P1K1", "")
        );
        let again = session.push(&capture(&card(&encoder, 1), &upright()), 33);
        assert_eq!((again.kana.as_str(), again.turned.as_str()), ("", "0123"));
        let next = session.push(&capture(&card(&encoder, 2), &upright()), 83);
        assert_eq!((next.kana.as_str(), next.turned.as_str()), ("P0K0P1K1", ""));
        assert_eq!(session.stats().spare_turns, 0);
    }

    #[test]
    fn the_last_turn_try_tells_whether_a_capture_has_time_to_spare() {
        let encoder = StreamEncoder::new(&payload(3000), 2).unwrap();
        let mut session = costed(|work| match work {
            Work::Search | Work::Track => 4.0,
            Work::Bands(bands) => 3.0 * f64::from(bands),
            Work::Merge => 0.0,
            Work::Turns => 30.0,
        });
        // 10 ms of work, and twice that for the turn lanes before any try: they fit
        let first = session.push(&capture(&card(&encoder, 1), &upright()), 0);
        assert_eq!(
            (first.kana.as_str(), first.turned.as_str()),
            ("P0K0P1K1", "0123")
        );
        // the try took 30 ms, which with the next capture's 10 ms does not fit
        let next = session.push(&capture(&card(&encoder, 2), &upright()), 83);
        assert_eq!((next.kana.as_str(), next.turned.as_str()), ("P0K0P1K1", ""));
        let again = session.push(&capture(&card(&encoder, 2), &upright()), 116);
        assert_eq!((again.kana.as_str(), again.turned.as_str()), ("", "0123"));
        assert_eq!(session.stats().spare_turns, 1);
    }

    #[test]
    fn a_steady_camera_is_tracked_after_the_first_frame() {
        let data = payload(3000);
        let encoder = StreamEncoder::new(&data, 2).unwrap();
        let config = camera(8.0);
        let mut session = ScanSession::new(ScanLimits::default());
        for frame in 0..6u16 {
            let outcome = session.push(
                &capture(&source(&encoder.cells(frame)), &config),
                u64::from(frame) * 100,
            );
            assert!(outcome.error.is_none(), "frame {frame}");
        }
        let stats = session.stats();
        assert_eq!(stats.readable, 6);
        assert_eq!(
            stats.tracked, 5,
            "every frame after the first follows the pose"
        );
        // a pause longer than the tracking window forces a full search again
        session.push(
            &capture(&source(&encoder.cells(6)), &config),
            5 * 100 + TRACK_WINDOW_MS + 1,
        );
        assert_eq!(session.stats().tracked, 5);
        assert_eq!(session.stats().readable, 7);
    }

    #[test]
    fn a_session_receives_a_payload_from_simulated_captures() {
        let data = payload(5000);
        let encoder = StreamEncoder::new(&data, 2).unwrap();
        let config = camera(20.0);
        let mut session = ScanSession::new(ScanLimits::default());
        let mut done = None;
        for frame in 0..40u16 {
            let outcome = session.push(
                &capture(&source(&encoder.cells(frame)), &config),
                u64::from(frame) * 100,
            );
            if outcome.completed.is_some() {
                done = outcome.completed;
                break;
            }
        }
        let done = done.expect("completed");
        assert_eq!(done.payload, data);
        assert_eq!(done.meta.kind, 2);
        let stats = session.stats();
        assert!(stats.readable > 0 && stats.lane_d > 0 && stats.field_lanes > 0);
        assert!(stats.turn_lanes > 0, "{stats:?}");
        // 5,000 bytes need 313 atoms: the field lanes and lane D alone carry them in 15 or 16
        // frames, the turn lanes on top in 9 or 10
        assert!(stats.frames <= 11, "{stats:?}");
    }

    #[test]
    fn a_turn_lane_is_read_once_per_shown_frame() {
        let encoder = StreamEncoder::new(&payload(3000), 2).unwrap();
        let config = camera(12.0);
        let mut session = ScanSession::new(ScanLimits::default());
        let shown = source(&encoder.cells(5));
        let first = session.push(&capture(&shown, &CaptureConfig { seed: 1, ..config }), 0);
        assert_eq!(first.lanes, "D0123");
        assert_eq!(first.turned, "0123");
        let again = session.push(&capture(&shown, &CaptureConfig { seed: 2, ..config }), 33);
        assert_eq!((again.lanes.as_str(), again.turned.as_str()), ("D0123", ""));
        assert_eq!(session.stats().turn_lanes, 4);
        // the next shown frame's turn lanes are new
        let next = session.push(
            &capture(
                &source(&encoder.cells(6)),
                &CaptureConfig { seed: 3, ..config },
            ),
            83,
        );
        assert_eq!(next.turned, "0123");
        assert_eq!(
            (session.stats().turn_tries, session.stats().turn_lanes),
            (8, 8)
        );
        session.reset();
        assert!(session.tried.is_empty());
    }

    #[test]
    fn tries_that_read_nothing_back_off() {
        let encoder = StreamEncoder::new(&payload(3000), 2).unwrap();
        // unturned petals: every field lane reads, no turn lane ever does
        let unturned = |frame: u16| {
            let mut cells = encoder.cells(frame);
            cells.turns = [0; crate::layout::FIELD_CELLS];
            source(&cells)
        };
        let config = camera(12.0);
        let mut session = ScanSession::new(ScanLimits::default());
        let mut tried_at = Vec::new();
        for frame in 1..=30u16 {
            let before = session.stats().turn_tries;
            let image = capture(
                &unturned(frame),
                &CaptureConfig {
                    seed: u64::from(frame),
                    ..config
                },
            );
            let outcome = session.push(&image, u64::from(frame) * 83);
            assert!(outcome.lanes.ends_with("0123"), "frame {frame}");
            assert_eq!(outcome.turned, "");
            if session.stats().turn_tries > before {
                tried_at.push(frame);
            }
        }
        // after the n-th try that reads nothing, the next min(n, 8) captures that would try are
        // passed over
        assert_eq!(tried_at, [1, 3, 6, 10, 15, 21, 28]);
        // a try that reads a lane ends the back-off
        session.turn_wait = 0;
        let image = capture(
            &source(&encoder.cells(31)),
            &CaptureConfig { seed: 31, ..config },
        );
        assert_eq!(session.push(&image, 31 * 83).turned, "0123");
        assert_eq!(session.turn_misses, 0);
        let image = capture(
            &source(&encoder.cells(32)),
            &CaptureConfig { seed: 32, ..config },
        );
        assert_eq!(session.push(&image, 32 * 83).turned, "0123");
    }

    #[test]
    fn idle_sessions_forget_partial_streams() {
        let encoder = StreamEncoder::new(&payload(12_000), 1).unwrap();
        let limits = ScanLimits {
            idle_timeout_ms: 1_000,
            ..ScanLimits::default()
        };
        let mut session = ScanSession::new(limits);
        session.push(&capture(&source(&encoder.cells(0)), &camera(0.0)), 0);
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
        let mut frame = crate::render::render_square(
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

    /// The `one-pass` stream of the golden captures.
    fn one_pass() -> StreamEncoder {
        let mut rng = crate::prng::Xorshift32::new(2);
        let data: Vec<u8> = (0..3000).map(|_| rng.next_byte()).collect();
        StreamEncoder::new(&data, 2).unwrap()
    }

    #[test]
    fn merging_recovers_field_lanes_that_no_single_capture_reads() {
        let encoder = one_pass();
        let shown = source(&encoder.cells(5));
        // a soft 480p camera in very dim light: every capture loses its field lanes to sensor
        // noise, which differs from capture to capture
        let config = fit_to_frame(
            &CaptureConfig {
                width: 640,
                height: 480,
                fill: 0.85,
                rotation_deg: 31.0,
                tilt_x_deg: -7.0,
                blur_sigma: 1.4,
                noise: 50.0,
                ..CaptureConfig::modern()
            },
            4.0,
        );
        let options = DecodeOptions::default();
        let mut session = ScanSession::new(ScanLimits::default());
        let mut merged = String::new();
        let mut singles = 0;
        for (i, seed) in (1201..=1204).enumerate() {
            let image = capture(&shown, &CaptureConfig { seed, ..config });
            let single = decode(&image, &options).expect("lane D reads");
            singles += single.fields.iter().filter(|f| f.is_some()).count();
            merged.push_str(&session.push(&image, 33 * i as u64).merged);
        }
        assert!(
            singles <= 1,
            "the captures read {singles} field lanes alone"
        );
        assert!(merged.len() >= 3, "merging recovered {merged:?}");
        let stats = session.stats();
        assert_eq!(stats.merged_lanes as usize, merged.len());
        assert!(stats.merges >= 1);
        // the merged atoms wait for the beacon, whose lane D the same camera reads
        assert_eq!(session.progress().atoms_received, 0);
        let beacon = capture(
            &source(&encoder.cells(4)),
            &CaptureConfig {
                seed: 1205,
                ..config
            },
        );
        session.push(&beacon, 200);
        assert_eq!(session.progress().meta, Some(encoder.meta()));
        assert!(session.progress().atoms_received as usize >= 5 * merged.len());
    }

    /// A clean capture of frame `petals` whose decoded lanes are replaced by lane `D` of
    /// frame `rings`, as if the rings showed another frame than the petal field.
    fn rings_of_another_frame(
        encoder: &StreamEncoder,
        petals: u16,
        rings: u16,
        seed: u64,
    ) -> (Luma, DecodedFrame) {
        let config = fit_to_frame(
            &CaptureConfig {
                width: 640,
                height: 480,
                rotation_deg: 25.0,
                seed,
                ..CaptureConfig::modern()
            },
            4.0,
        );
        let image = capture(&source(&encoder.cells(petals)), &config);
        let mut frame = decode(&image, &DecodeOptions::default()).expect("decodes");
        assert!(frame.fields.iter().all(Option::is_some));
        frame.fields = Default::default();
        frame.d = Some(LaneResult {
            data: encoder.lane_data(rings).d,
            corrected: 0,
            erasures: 0,
        });
        (image, frame)
    }

    #[test]
    fn a_merge_never_offers_atoms_of_another_shown_frame() {
        let encoder = one_pass();
        let mut session = ScanSession::new(ScanLimits::default());
        for seed in 1..=3 {
            let (image, frame) = rings_of_another_frame(&encoder, 11, 10, seed);
            assert_eq!(session.merge(&image, &frame), "", "capture {seed}");
        }
        let stats = session.stats();
        assert_eq!((stats.merges, stats.merged_lanes), (2, 0));
        assert_eq!(session.progress().atoms_received, 0);
        // what was refused: the merged field lanes name frame 11, not frame 10
        let held = &mut session.held[0];
        assert_eq!(held.shown.frame, 10);
        let merged = merge_fields(held.evidence.make_contiguous(), [false; SECTORS]);
        let truth = encoder.lane_data(11);
        for (result, expected) in merged.iter().zip(&truth.fields) {
            assert_eq!(result.as_ref().map(|l| &l.data), Some(expected));
        }
    }

    #[test]
    fn torn_captures_whose_lanes_name_different_frames_are_not_held() {
        let encoder = one_pass();
        let mut session = ScanSession::new(ScanLimits::default());
        let (image, mut frame) = rings_of_another_frame(&encoder, 11, 10, 1);
        frame.fields[2] = Some(LaneResult {
            data: encoder.lane_data(11).fields[2].clone(),
            corrected: 0,
            erasures: 0,
        });
        assert_eq!(session.merge(&image, &frame), "");
        assert!(session.held.is_empty());
    }

    #[test]
    fn the_session_holds_at_most_four_captures_of_six_shown_frames() {
        let encoder = one_pass();
        let mut session = ScanSession::new(ScanLimits::default());
        // refused merges never finish a shown frame, so its captures stay held
        for seed in 1..=MERGE_CAPTURES as u64 + 2 {
            let (image, frame) = rings_of_another_frame(&encoder, 11, 10, seed);
            session.merge(&image, &frame);
        }
        assert_eq!(session.held.len(), 1);
        assert_eq!(session.held[0].evidence.len(), MERGE_CAPTURES);
        assert_eq!(session.stats().merges, MERGE_CAPTURES as u32 + 1);
        let (image, _) = rings_of_another_frame(&encoder, 11, 10, 1);
        for rings in 12..12 + MERGE_WINDOW as u16 {
            let (_, frame) = rings_of_another_frame(&encoder, 11, rings, 1);
            session.merge(&image, &frame);
        }
        assert_eq!(session.held.len(), MERGE_WINDOW);
        assert!(session.held.iter().all(|h| h.shown.frame >= 12));
        session.reset();
        assert!(session.held.is_empty());
    }

    #[test]
    fn switching_streams_with_the_same_short_tag_discards_old_merge_state() {
        let encoder = |last| {
            let mut payload = vec![0u8; 100];
            payload[3] = last;
            StreamEncoder::new(&payload, 2).unwrap()
        };
        let old = encoder(49);
        let new = encoder(128);
        assert_eq!((old.meta().tag(), new.meta().tag()), (61, 61));
        assert_ne!(old.meta(), new.meta());
        let old_image = source(&old.cells(0)).to_luma();
        let new_image = source(&new.cells(0)).to_luma();
        let old_full = decode(&old_image, &DecodeOptions::default()).unwrap();
        let mut new_partial = decode(&new_image, &DecodeOptions::default()).unwrap();
        new_partial.fields = Default::default();
        for was_complete in [false, true] {
            let mut session = ScanSession::new(ScanLimits::default());
            let mut previous = old_full.clone();
            if !was_complete {
                previous.fields = Default::default();
            }
            session.absorb(&previous, 0);
            session.merge(&old_image, &previous);
            assert!(!session.held.is_empty());
            session.absorb(&new_partial, 33);
            session.absorb(&new_partial, 66);
            assert_eq!(session.progress().meta, Some(new.meta()));
            assert!(session.held.is_empty());
            assert_eq!(session.merge(&new_image, &new_partial), "");
            assert_eq!(session.held[0].evidence.len(), 1);
            assert!(session.held[0].done.iter().all(|d| !d));
            assert_eq!(session.merge(&new_image, &new_partial), "0123");
        }
    }

    #[test]
    fn the_first_beacon_preserves_evidence_received_before_it() {
        let encoder = one_pass();
        let image = source(&encoder.cells(1)).to_luma();
        let mut partial = decode(&image, &DecodeOptions::default()).unwrap();
        partial.fields = Default::default();
        let mut session = ScanSession::new(ScanLimits::default());
        session.absorb(&partial, 0);
        session.merge(&image, &partial);
        assert!(session.progress().meta.is_none());
        assert_eq!(session.held[0].evidence.len(), 1);
        let beacon_image = source(&encoder.cells(0)).to_luma();
        let mut beacon = decode(&beacon_image, &DecodeOptions::default()).unwrap();
        beacon.fields = Default::default();
        session.absorb(&beacon, 33);
        assert_eq!(session.progress().meta, Some(encoder.meta()));
        assert_eq!(session.held[0].evidence.len(), 1);
        assert_eq!(session.merge(&image, &partial), "0123");
    }
}
