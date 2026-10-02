//! Payload streams: the sender-side [`StreamEncoder`] and the receiver-side
//! [`StreamAssembler`].
//!
//! Every frame carries a handful of fountain atoms, one lane at a time:
//! lane `P` one atom, lane `K` five, and lane `D` one atom — except on every
//! fourth frame (`frame % 4 == 0`), when lane `D` carries the stream *beacon*
//! instead, so a receiver can join at any frame within a fraction of a second.
//! Atom ids run contiguously over the atoms actually sent (see
//! [`first_atom_id`]). Any single readable lane is useful on its own.
//!
//! Lane data layouts (all big-endian):
//!
//! * every lane starts with `tag:u8, frame:u16`; `tag` is the low byte of the
//!   payload CRC-32C.
//! * lanes `P`, `K` and non-beacon `D`: `atoms…` (16 bytes each).
//! * beacon `D`: `version:u8, kind:u8, len:u24, crc:u32`, zero padded.

use std::collections::VecDeque;

use crate::crc::crc32c;
use crate::fountain::{Atom, FountainDecoder, encode_atom, split_payload};
use crate::lanes::{
    ATOM_LEN, D_ATOMS, D_DATA, FrameCells, K_ATOMS, K_DATA, LANE_HEADER_LEN, Lane, P_ATOMS, P_DATA,
    encode_lane,
};

/// Version/profile byte of the beacon: format version 1, layout profile 0.
pub const FORMAT_VERSION: u8 = 0x10;
/// Largest payload a beacon can describe (`u24`).
pub const MAX_PAYLOAD_LEN: usize = (1 << 24) - 1;
/// Default receiver payload limit; override with [`AssemblerLimits`].
pub const DEFAULT_MAX_PAYLOAD_LEN: usize = 65_536;
/// A beacon replaces the lane-`D` atom on frames divisible by this interval.
pub const BEACON_INTERVAL: u16 = 4;

const BEACON_BODY_LEN: usize = 9;

/// Whether `frame` carries the beacon in lane `D`.
#[must_use]
pub const fn is_beacon_frame(frame: u16) -> bool {
    frame.is_multiple_of(BEACON_INTERVAL)
}

/// Fountain atoms carried by `frame`.
#[must_use]
pub const fn atoms_in_frame(frame: u16) -> usize {
    if is_beacon_frame(frame) {
        P_ATOMS + K_ATOMS
    } else {
        P_ATOMS + D_ATOMS + K_ATOMS
    }
}

/// Fountain id of the first atom of `frame`.
///
/// Frame `f` follows `f` earlier frames, `ceil(f / 4)` of which were beacon
/// frames with one atom fewer.
#[must_use]
pub fn first_atom_id(frame: u16) -> u32 {
    let f = u32::from(frame);
    f * (P_ATOMS + D_ATOMS + K_ATOMS) as u32 - f.div_ceil(u32::from(BEACON_INTERVAL))
}

fn lane_first_id(lane: Lane, frame: u16) -> u32 {
    let base = first_atom_id(frame);
    match lane {
        Lane::P => base,
        Lane::D => base + P_ATOMS as u32,
        Lane::K => base + (P_ATOMS + if is_beacon_frame(frame) { 0 } else { D_ATOMS }) as u32,
    }
}

/// Errors raised by the sender.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamError {
    /// The payload is empty.
    EmptyPayload,
    /// The payload exceeds [`MAX_PAYLOAD_LEN`].
    PayloadTooLarge,
}

impl core::fmt::Display for StreamError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::EmptyPayload => f.write_str("petal stream payload is empty"),
            Self::PayloadTooLarge => {
                f.write_str("petal stream payload exceeds the 24-bit length field")
            }
        }
    }
}

impl std::error::Error for StreamError {}

/// Identity of a stream, as carried by every beacon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamMeta {
    /// Application payload kind.
    pub kind: u8,
    /// Payload length in bytes.
    pub len: u32,
    /// CRC-32C of the payload.
    pub crc: u32,
}

impl StreamMeta {
    /// The one-byte stream tag repeated in every lane header.
    #[must_use]
    pub fn tag(&self) -> u8 {
        self.crc as u8
    }

    /// Number of fountain source atoms.
    #[must_use]
    pub fn source_atoms(&self) -> usize {
        (self.len as usize).div_ceil(ATOM_LEN)
    }
}

/// The common three-byte header of every lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaneHeader {
    /// Stream tag.
    pub tag: u8,
    /// Frame counter (wraps at 65536).
    pub frame: u16,
}

/// A decoded beacon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Beacon {
    /// Lane header.
    pub header: LaneHeader,
    /// Stream identity.
    pub meta: StreamMeta,
}

/// Atoms read from one lane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtomPacket {
    /// Lane header.
    pub header: LaneHeader,
    /// Fountain id of the first atom; the rest follow consecutively.
    pub first_id: u32,
    /// The atoms.
    pub atoms: Vec<Atom>,
}

/// What lane `D` carried.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DLane {
    /// The stream beacon.
    Beacon(Beacon),
    /// A payload atom.
    Atoms(AtomPacket),
}

fn parse_header(data: &[u8]) -> LaneHeader {
    LaneHeader {
        tag: data[0],
        frame: u16::from_be_bytes([data[1], data[2]]),
    }
}

fn parse_atoms(lane: Lane, data: &[u8], header: LaneHeader, count: usize) -> AtomPacket {
    let atoms = (0..count)
        .map(|i| {
            let start = LANE_HEADER_LEN + i * ATOM_LEN;
            let mut atom = [0u8; ATOM_LEN];
            atom.copy_from_slice(&data[start..start + ATOM_LEN]);
            atom
        })
        .collect();
    AtomPacket {
        header,
        first_id: lane_first_id(lane, header.frame),
        atoms,
    }
}

/// Parses the data bytes of lane `P` or lane `K`.
#[must_use]
pub fn parse_atom_lane(lane: Lane, data: &[u8]) -> Option<AtomPacket> {
    let count = match lane {
        Lane::P => P_ATOMS,
        Lane::K => K_ATOMS,
        Lane::D => return None,
    };
    if data.len() != lane.data_len() {
        return None;
    }
    Some(parse_atoms(lane, data, parse_header(data), count))
}

/// Parses the data bytes of lane `D`.
#[must_use]
pub fn parse_d_lane(data: &[u8]) -> Option<DLane> {
    if data.len() != D_DATA {
        return None;
    }
    let header = parse_header(data);
    if !is_beacon_frame(header.frame) {
        return Some(DLane::Atoms(parse_atoms(Lane::D, data, header, D_ATOMS)));
    }
    let body = &data[LANE_HEADER_LEN..];
    if body[0] != FORMAT_VERSION {
        return None;
    }
    let len = u32::from_be_bytes([0, body[2], body[3], body[4]]);
    if len == 0 {
        return None;
    }
    Some(DLane::Beacon(Beacon {
        header,
        meta: StreamMeta {
            kind: body[1],
            len,
            crc: u32::from_be_bytes([body[5], body[6], body[7], body[8]]),
        },
    }))
}

/// Sender side: turns one payload into an endless sequence of frames.
#[derive(Debug, Clone)]
pub struct StreamEncoder {
    meta: StreamMeta,
    source: Vec<Atom>,
}

impl StreamEncoder {
    /// Prepares `payload` of application kind `kind` for streaming.
    ///
    /// # Errors
    /// Fails for empty or oversized payloads.
    pub fn new(payload: &[u8], kind: u8) -> Result<Self, StreamError> {
        if payload.is_empty() {
            return Err(StreamError::EmptyPayload);
        }
        if payload.len() > MAX_PAYLOAD_LEN {
            return Err(StreamError::PayloadTooLarge);
        }
        Ok(Self {
            meta: StreamMeta {
                kind,
                len: payload.len() as u32,
                crc: crc32c(payload),
            },
            source: split_payload(payload),
        })
    }

    /// Stream identity.
    #[must_use]
    pub fn meta(&self) -> StreamMeta {
        self.meta
    }

    /// Frames needed to send every source atom once (no losses, no repair).
    #[must_use]
    pub fn systematic_frames(&self) -> usize {
        let mut frames = 0usize;
        let mut atoms = 0usize;
        while atoms < self.source.len() {
            atoms += atoms_in_frame(frames as u16);
            frames += 1;
        }
        frames
    }

    fn atoms(&self, first_id: u32, count: usize) -> Vec<u8> {
        (0..count as u32)
            .flat_map(|i| encode_atom(&self.source, self.meta.crc, first_id + i))
            .collect()
    }

    /// The data bytes of every lane of `frame`: `(P, K, D)`.
    #[must_use]
    pub fn lane_data(&self, frame: u16) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let header = [self.meta.tag(), (frame >> 8) as u8, frame as u8];
        let build = |lane: Lane, body: Vec<u8>| {
            let mut data = header.to_vec();
            data.extend_from_slice(&body);
            debug_assert_eq!(data.len(), lane.data_len());
            data
        };
        let p = build(Lane::P, self.atoms(lane_first_id(Lane::P, frame), P_ATOMS));
        let k = build(Lane::K, self.atoms(lane_first_id(Lane::K, frame), K_ATOMS));
        let d = if is_beacon_frame(frame) {
            let mut body = vec![FORMAT_VERSION, self.meta.kind];
            body.extend_from_slice(&self.meta.len.to_be_bytes()[1..]);
            body.extend_from_slice(&self.meta.crc.to_be_bytes());
            body.resize(D_DATA - LANE_HEADER_LEN, 0);
            debug_assert!(body.len() >= BEACON_BODY_LEN);
            build(Lane::D, body)
        } else {
            build(Lane::D, self.atoms(lane_first_id(Lane::D, frame), D_ATOMS))
        };
        debug_assert_eq!((p.len(), k.len()), (P_DATA, K_DATA));
        (p, k, d)
    }

    /// The transmitted codewords of `frame`: `(P, K, D)`.
    #[must_use]
    pub fn words(&self, frame: u16) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let (p, k, d) = self.lane_data(frame);
        (
            encode_lane(Lane::P, &p),
            encode_lane(Lane::K, &k),
            encode_lane(Lane::D, &d),
        )
    }

    /// Every cell of `frame`, ready to render.
    #[must_use]
    pub fn cells(&self, frame: u16) -> FrameCells {
        let (p, k, d) = self.words(frame);
        FrameCells::from_words(&p, &k, &d)
    }
}

/// Receiver limits that bound memory and work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssemblerLimits {
    /// Largest payload the receiver accepts.
    pub max_payload_len: usize,
    /// Atoms buffered while waiting for the first beacon.
    pub max_pending_atoms: usize,
}

impl Default for AssemblerLimits {
    fn default() -> Self {
        Self {
            max_payload_len: DEFAULT_MAX_PAYLOAD_LEN,
            max_pending_atoms: 128,
        }
    }
}

/// A reassembled, CRC-verified payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completed {
    /// Stream identity.
    pub meta: StreamMeta,
    /// The payload bytes.
    pub payload: Vec<u8>,
}

/// Snapshot of receive progress for a UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Progress {
    /// Stream identity once a beacon was accepted.
    pub meta: Option<StreamMeta>,
    /// Source atoms of the active stream.
    pub source_atoms: usize,
    /// Independent atoms collected so far.
    pub rank: usize,
    /// Atoms offered to the decoder (including duplicates).
    pub atoms_received: u32,
    /// Reassembled payloads that failed the CRC check and were discarded
    /// (cumulative over the assembler's lifetime, not cleared by `reset`).
    pub integrity_failures: u32,
    /// Whether the payload is complete and verified.
    pub complete: bool,
}

struct Active {
    meta: StreamMeta,
    decoder: FountainDecoder,
    done: bool,
}

/// Receiver side: collects atoms from any lane of any frame.
pub struct StreamAssembler {
    limits: AssemblerLimits,
    active: Option<Active>,
    pending: VecDeque<(u8, u32, Atom)>,
    conflicting: Option<(StreamMeta, u8)>,
    completed: Option<Completed>,
    atoms_received: u32,
    integrity_failures: u32,
}

impl StreamAssembler {
    /// Creates an assembler.
    #[must_use]
    pub fn new(limits: AssemblerLimits) -> Self {
        Self {
            limits,
            active: None,
            pending: VecDeque::new(),
            conflicting: None,
            completed: None,
            atoms_received: 0,
            integrity_failures: 0,
        }
    }

    /// Forgets the active stream and any completed payload.
    pub fn reset(&mut self) {
        self.active = None;
        self.pending.clear();
        self.conflicting = None;
        self.completed = None;
        self.atoms_received = 0;
    }

    /// Current progress.
    #[must_use]
    pub fn progress(&self) -> Progress {
        let (meta, source_atoms, rank, complete) =
            self.active.as_ref().map_or((None, 0, 0, false), |a| {
                (
                    Some(a.meta),
                    a.decoder.source_atoms(),
                    a.decoder.rank(),
                    a.done,
                )
            });
        Progress {
            meta,
            source_atoms,
            rank,
            atoms_received: self.atoms_received,
            integrity_failures: self.integrity_failures,
            complete,
        }
    }

    /// Takes the completed payload, if any.
    pub fn take_completed(&mut self) -> Option<Completed> {
        self.completed.take()
    }

    fn start(&mut self, meta: StreamMeta) {
        self.active = Some(Active {
            meta,
            decoder: FountainDecoder::new(meta.source_atoms()),
            done: false,
        });
        self.conflicting = None;
        self.completed = None;
        self.atoms_received = 0;
        let tag = meta.tag();
        let pending = std::mem::take(&mut self.pending);
        for (pending_tag, id, atom) in pending {
            if pending_tag == tag {
                self.add_atom(id, atom);
            }
        }
    }

    /// Offers a beacon read from lane `D`.
    pub fn push_beacon(&mut self, beacon: &Beacon) {
        let meta = beacon.meta;
        if meta.len == 0 || meta.len as usize > self.limits.max_payload_len {
            return;
        }
        match &self.active {
            None => self.start(meta),
            Some(active) if active.meta == meta => self.conflicting = None,
            Some(_) => {
                // A different stream: switch only after two consecutive sightings.
                let seen = match self.conflicting {
                    Some((candidate, n)) if candidate == meta => n + 1,
                    _ => 1,
                };
                if seen >= 2 {
                    self.start(meta);
                } else {
                    self.conflicting = Some((meta, seen));
                }
            }
        }
    }

    /// Offers atoms read from a lane.
    pub fn push_atoms(&mut self, packet: &AtomPacket) {
        for (index, atom) in packet.atoms.iter().enumerate() {
            let id = packet.first_id + index as u32;
            match &self.active {
                Some(active) if active.meta.tag() == packet.header.tag => self.add_atom(id, *atom),
                Some(_) => {}
                None => {
                    if self.limits.max_pending_atoms == 0 {
                        continue;
                    }
                    if self.pending.len() >= self.limits.max_pending_atoms {
                        self.pending.pop_front();
                    }
                    self.pending.push_back((packet.header.tag, id, *atom));
                }
            }
        }
    }

    /// Offers whatever lane `D` carried.
    pub fn push_d_lane(&mut self, lane: &DLane) {
        match lane {
            DLane::Beacon(beacon) => self.push_beacon(beacon),
            DLane::Atoms(packet) => self.push_atoms(packet),
        }
    }

    fn add_atom(&mut self, id: u32, atom: Atom) {
        let Some(active) = &mut self.active else {
            return;
        };
        if active.done {
            return;
        }
        self.atoms_received = self.atoms_received.saturating_add(1);
        active.decoder.add_encoded(active.meta.crc, id, atom);
        if !active.decoder.is_complete() {
            return;
        }
        let Some(source) = active.decoder.solve() else {
            return;
        };
        let mut payload: Vec<u8> = source.iter().flatten().copied().collect();
        payload.truncate(active.meta.len as usize);
        if crc32c(&payload) == active.meta.crc {
            active.done = true;
            self.completed = Some(Completed {
                meta: active.meta,
                payload,
            });
        } else {
            // Corrupt atoms slipped through: start the elimination over.
            self.integrity_failures = self.integrity_failures.saturating_add(1);
            active.decoder = FountainDecoder::new(active.meta.source_atoms());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lanes::decode_lane;
    use crate::prng::Xorshift32;

    fn payload(len: usize, seed: u32) -> Vec<u8> {
        let mut rng = Xorshift32::new(seed);
        (0..len).map(|_| rng.next_byte()).collect()
    }

    fn feed_frame(
        assembler: &mut StreamAssembler,
        encoder: &StreamEncoder,
        frame: u16,
        lanes: &[Lane],
    ) {
        let (p, k, d) = encoder.words(frame);
        for lane in lanes {
            let word = match lane {
                Lane::P => &p,
                Lane::K => &k,
                Lane::D => &d,
            };
            let data = decode_lane(*lane, word, &[]).expect("clean lane");
            match lane {
                Lane::D => assembler.push_d_lane(&parse_d_lane(&data).expect("d lane")),
                _ => assembler.push_atoms(&parse_atom_lane(*lane, &data).expect("atoms")),
            }
        }
    }

    #[test]
    fn atom_ids_are_contiguous_across_frames() {
        let mut expected = 0u32;
        for frame in 0..=70u16 {
            assert_eq!(first_atom_id(frame), expected, "frame {frame}");
            expected += atoms_in_frame(frame) as u32;
        }
        assert_eq!(atoms_in_frame(0), 6);
        assert_eq!(atoms_in_frame(1), 7);
        assert_eq!(lane_first_id(Lane::K, 4), first_atom_id(4) + 1);
        assert_eq!(lane_first_id(Lane::K, 5), first_atom_id(5) + 2);
        // the frame counter wraps on a beacon frame, so ids repeat cleanly
        assert!(is_beacon_frame(0) && 65_536u32.is_multiple_of(u32::from(BEACON_INTERVAL)));
    }

    #[test]
    fn clean_stream_completes_after_one_systematic_pass() {
        let data = payload(1_000, 21);
        let encoder = StreamEncoder::new(&data, 2).unwrap();
        let mut assembler = StreamAssembler::new(AssemblerLimits::default());
        for frame in 0..encoder.systematic_frames() as u16 {
            feed_frame(
                &mut assembler,
                &encoder,
                frame,
                &[Lane::D, Lane::P, Lane::K],
            );
        }
        let done = assembler.take_completed().expect("complete");
        assert_eq!(done.payload, data);
        assert_eq!(done.meta.kind, 2);
        assert!(assembler.progress().complete);
    }

    #[test]
    fn any_single_lane_is_enough_given_a_beacon() {
        let data = payload(400, 22);
        let encoder = StreamEncoder::new(&data, 1).unwrap();
        for lane in [Lane::P, Lane::K, Lane::D] {
            let mut assembler = StreamAssembler::new(AssemblerLimits::default());
            feed_frame(&mut assembler, &encoder, 0, &[Lane::D]);
            for frame in 0..400u16 {
                feed_frame(&mut assembler, &encoder, frame, &[lane]);
                if assembler.progress().complete {
                    break;
                }
            }
            assert_eq!(
                assembler
                    .take_completed()
                    .unwrap_or_else(|| panic!("{lane:?} alone"))
                    .payload,
                data
            );
        }
    }

    #[test]
    fn atoms_seen_before_the_first_beacon_are_not_lost() {
        let data = payload(300, 23);
        let encoder = StreamEncoder::new(&data, 1).unwrap();
        let mut assembler = StreamAssembler::new(AssemblerLimits::default());
        // frames 1..: no beacon among them until frame 4
        for frame in 1..4u16 {
            feed_frame(
                &mut assembler,
                &encoder,
                frame,
                &[Lane::P, Lane::K, Lane::D],
            );
        }
        assert!(assembler.progress().meta.is_none());
        for frame in 4..encoder.systematic_frames() as u16 + 4 {
            feed_frame(
                &mut assembler,
                &encoder,
                frame,
                &[Lane::P, Lane::K, Lane::D],
            );
        }
        assert_eq!(assembler.take_completed().expect("complete").payload, data);
    }

    #[test]
    fn joining_mid_stream_and_losing_frames_still_completes() {
        let data = payload(2_500, 24);
        let encoder = StreamEncoder::new(&data, 3).unwrap();
        let mut assembler = StreamAssembler::new(AssemblerLimits::default());
        let mut rng = Xorshift32::new(77);
        let mut frame = 15u16; // join late
        let mut shown = 0;
        while !assembler.progress().complete {
            if rng.next_u32() % 10 < 6 {
                // 60 % of the frames are readable
                feed_frame(
                    &mut assembler,
                    &encoder,
                    frame,
                    &[Lane::D, Lane::P, Lane::K],
                );
            }
            frame = frame.wrapping_add(1);
            shown += 1;
            assert!(shown < 600, "stream failed to complete");
        }
        assert_eq!(assembler.take_completed().expect("complete").payload, data);
    }

    #[test]
    fn a_different_stream_replaces_the_active_one_after_two_beacons() {
        let first = StreamEncoder::new(&payload(100, 1), 1).unwrap();
        let second = StreamEncoder::new(&payload(100, 2), 1).unwrap();
        let mut assembler = StreamAssembler::new(AssemblerLimits::default());
        feed_frame(&mut assembler, &first, 0, &[Lane::D]);
        assert_eq!(assembler.progress().meta, Some(first.meta()));
        feed_frame(&mut assembler, &second, 0, &[Lane::D]);
        assert_eq!(assembler.progress().meta, Some(first.meta()));
        feed_frame(&mut assembler, &second, 4, &[Lane::D]);
        assert_eq!(assembler.progress().meta, Some(second.meta()));
    }

    #[test]
    fn a_zero_pending_limit_buffers_nothing() {
        let data = payload(300, 41);
        let encoder = StreamEncoder::new(&data, 1).unwrap();
        let mut assembler = StreamAssembler::new(AssemblerLimits {
            max_pending_atoms: 0,
            ..AssemblerLimits::default()
        });
        // atoms before any beacon are dropped, not buffered
        for frame in 1..4u16 {
            feed_frame(
                &mut assembler,
                &encoder,
                frame,
                &[Lane::P, Lane::K, Lane::D],
            );
        }
        feed_frame(&mut assembler, &encoder, 4, &[Lane::D]);
        assert_eq!(assembler.progress().rank, 0);
        assert_eq!(assembler.progress().atoms_received, 0);
    }

    #[test]
    fn oversized_beacons_are_ignored() {
        let encoder = StreamEncoder::new(&payload(4_000, 5), 1).unwrap();
        let mut assembler = StreamAssembler::new(AssemblerLimits {
            max_payload_len: 1_000,
            ..AssemblerLimits::default()
        });
        feed_frame(&mut assembler, &encoder, 0, &[Lane::D]);
        assert!(assembler.progress().meta.is_none());
    }

    #[test]
    fn corrupt_atoms_are_caught_by_the_payload_crc() {
        let data = payload(200, 25);
        let encoder = StreamEncoder::new(&data, 1).unwrap();
        let mut assembler = StreamAssembler::new(AssemblerLimits::default());
        feed_frame(&mut assembler, &encoder, 0, &[Lane::D, Lane::K]);
        // atom 0 arrives with a valid header but a wrong body
        assembler.push_atoms(&AtomPacket {
            header: LaneHeader {
                tag: encoder.meta().tag(),
                frame: 0,
            },
            first_id: 0,
            atoms: vec![[0xEE; ATOM_LEN]],
        });
        for frame in 1..encoder.systematic_frames() as u16 {
            feed_frame(
                &mut assembler,
                &encoder,
                frame,
                &[Lane::P, Lane::K, Lane::D],
            );
        }
        assert!(assembler.take_completed().is_none());
        assert_eq!(assembler.progress().integrity_failures, 1);
        // clean repair frames after the reset recover the payload
        for frame in 100..600u16 {
            feed_frame(
                &mut assembler,
                &encoder,
                frame,
                &[Lane::P, Lane::K, Lane::D],
            );
            if assembler.progress().complete {
                break;
            }
        }
        assert_eq!(assembler.take_completed().expect("recovered").payload, data);
    }

    #[test]
    fn encoder_rejects_empty_and_oversized_payloads() {
        assert_eq!(
            StreamEncoder::new(&[], 0).unwrap_err(),
            StreamError::EmptyPayload
        );
        assert_eq!(
            StreamEncoder::new(&vec![0u8; MAX_PAYLOAD_LEN + 1], 0).unwrap_err(),
            StreamError::PayloadTooLarge
        );
    }

    #[test]
    fn beacon_roundtrip_through_lane_d() {
        let encoder = StreamEncoder::new(&payload(77, 9), 3).unwrap();
        let (_, _, d) = encoder.lane_data(512);
        let DLane::Beacon(beacon) = parse_d_lane(&d).unwrap() else {
            panic!("beacon expected")
        };
        assert_eq!(beacon.meta, encoder.meta());
        assert_eq!(beacon.header.frame, 512);
        assert!(parse_d_lane(&d[..11]).is_none());
        let mut bad = d.clone();
        bad[3] = 0x20;
        assert!(parse_d_lane(&bad).is_none());
        // non-beacon frames carry an atom instead
        let (_, _, d) = encoder.lane_data(513);
        assert!(matches!(parse_d_lane(&d), Some(DLane::Atoms(_))));
    }
}
