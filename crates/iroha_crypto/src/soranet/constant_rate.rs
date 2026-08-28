//! Authenticated fixed-size cells for strict SoraNet constant-rate transport.
//!
//! QUIC DATAGRAMs are unreliable and may be reordered.  Strict SoraNet does
//! not try to disguise that property as reliable delivery: every scheduled
//! tick, including cover, consumes the post-handshake record sequence.  A
//! missing, duplicated, reordered, or unauthenticated cell therefore makes the
//! next received tick fail closed instead of letting application traffic fall
//! back to an unscheduled stream.

use super::record::{
    DuplexRecordLayer, RECORD_HEADER_LEN, RECORD_TAG_LEN, RecordEndpoint, RecordError, RecordLayer,
    RecordOpener, RecordSealer, RecordStreamContext, RecordStreamKind,
};
use std::collections::{HashMap, VecDeque};
use thiserror::Error;

/// Exact QUIC DATAGRAM payload size advertised by `snnet.constant_rate` v1.
pub const CONSTANT_RATE_CELL_BYTES: usize = 1_024;
/// Bytes used by the outer fixed-cell header.
pub const CONSTANT_RATE_CELL_HEADER_BYTES: usize = 4;
/// Maximum authenticated record carried by one fixed cell.
pub const CONSTANT_RATE_CELL_RECORD_BYTES: usize =
    CONSTANT_RATE_CELL_BYTES - CONSTANT_RATE_CELL_HEADER_BYTES;

const CELL_TYPE_DATA: u8 = 1;
const CELL_CLASS_CONTROL: u8 = 0;
const CELL_CLASS_INTERACTIVE: u8 = 1;
const CELL_CLASS_BULK: u8 = 2;
const MUX_MAGIC: [u8; 4] = *b"SNM1";
const MUX_HEADER_BYTES: usize = 18;
const MUX_ALLOWED_FLAGS: u8 = MuxFlags::OPEN | MuxFlags::FIN | MuxFlags::RESET;
const CONSTANT_RATE_RECORD_CONTEXT_INDEX: u64 = u64::MAX;

/// Maximum logical payload in one authenticated strict constant-rate cell.
pub const CONSTANT_RATE_MAX_PAYLOAD_BYTES: usize =
    CONSTANT_RATE_CELL_RECORD_BYTES - RECORD_HEADER_LEN - RECORD_TAG_LEN - MUX_HEADER_BYTES;

/// Traffic class committed inside every authenticated constant-rate cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum CellClass {
    /// Handshake, liveness, measurement, and other control traffic.
    Control = CELL_CLASS_CONTROL,
    /// Latency-sensitive application traffic.
    Interactive = CELL_CLASS_INTERACTIVE,
    /// Bulk application traffic and idle cover.
    Bulk = CELL_CLASS_BULK,
}

impl CellClass {
    fn from_wire(value: u8) -> Result<Self, ConstantRateError> {
        match value {
            CELL_CLASS_CONTROL => Ok(Self::Control),
            CELL_CLASS_INTERACTIVE => Ok(Self::Interactive),
            CELL_CLASS_BULK => Ok(Self::Bulk),
            _ => Err(ConstantRateError::UnknownClass(value)),
        }
    }

    const fn scheduler_index(self) -> usize {
        match self {
            Self::Control => 0,
            Self::Interactive => 1,
            Self::Bulk => 2,
        }
    }

    const fn scheduler_weight(self) -> i32 {
        match self {
            Self::Control => 8,
            Self::Interactive => 4,
            Self::Bulk => 1,
        }
    }
}

/// Authenticated logical consumer carried by the fixed-rate mux.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MuxChannel {
    /// An authenticated cover tick with no application payload.
    Cover = 0,
    /// General SoraNet application payload.
    Application = 1,
    /// Exit-adapter request or response payload.
    Exit = 2,
    /// Relay measurement payload.
    Measurement = 3,
    /// Sora VPN helper payload.
    Vpn = 4,
}

impl MuxChannel {
    fn from_wire(value: u8) -> Result<Self, ConstantRateError> {
        match value {
            0 => Ok(Self::Cover),
            1 => Ok(Self::Application),
            2 => Ok(Self::Exit),
            3 => Ok(Self::Measurement),
            4 => Ok(Self::Vpn),
            _ => Err(ConstantRateError::UnknownChannel(value)),
        }
    }
}

/// Logical stream transition flags authenticated by the SoraNet record layer.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct MuxFlags(u8);

impl MuxFlags {
    /// This cell starts a new logical lane.
    pub const OPEN: u8 = 1 << 0;
    /// This cell finishes the sending direction of a logical lane.
    pub const FIN: u8 = 1 << 1;
    /// This cell aborts the logical lane. RESET cannot carry payload.
    pub const RESET: u8 = 1 << 2;

    /// Construct a checked set of logical transition flags.
    ///
    /// # Errors
    /// Returns [`ConstantRateError::InvalidFlags`] for any reserved bit.
    pub const fn new(bits: u8) -> Result<Self, ConstantRateError> {
        if bits & !MUX_ALLOWED_FLAGS != 0 {
            return Err(ConstantRateError::InvalidFlags(bits));
        }
        Ok(Self(bits))
    }

    /// Return the exact wire bits.
    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// Whether this transition opens a lane.
    #[must_use]
    pub const fn is_open(self) -> bool {
        self.0 & Self::OPEN != 0
    }

    /// Whether this transition cleanly finishes a lane direction.
    #[must_use]
    pub const fn is_fin(self) -> bool {
        self.0 & Self::FIN != 0
    }

    /// Whether this transition aborts a lane.
    #[must_use]
    pub const fn is_reset(self) -> bool {
        self.0 & Self::RESET != 0
    }
}

/// One authenticated logical transition before record sealing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MuxFrame {
    /// Logical consumer of this payload.
    pub channel: MuxChannel,
    /// Scheduler class authenticated with the payload.
    pub class: CellClass,
    /// Non-zero logical lane identifier. Cover uses the reserved zero lane.
    pub lane_id: u64,
    /// Logical lane transition flags.
    pub flags: MuxFlags,
    /// Bounded application bytes carried by this tick.
    pub payload: Vec<u8>,
}

impl MuxFrame {
    /// Construct a checked application frame.
    ///
    /// # Errors
    /// Rejects the cover channel, lane zero, oversized/empty progress, and
    /// RESET transitions that attempt to carry application bytes.
    pub fn new(
        channel: MuxChannel,
        class: CellClass,
        lane_id: u64,
        flags: MuxFlags,
        payload: Vec<u8>,
    ) -> Result<Self, ConstantRateError> {
        let frame = Self {
            channel,
            class,
            lane_id,
            flags,
            payload,
        };
        frame.validate()?;
        Ok(frame)
    }

    /// Construct the authenticated cover transition used for an idle tick.
    #[must_use]
    pub fn cover() -> Self {
        Self {
            channel: MuxChannel::Cover,
            class: CellClass::Bulk,
            lane_id: 0,
            flags: MuxFlags::default(),
            payload: Vec::new(),
        }
    }

    fn validate(&self) -> Result<(), ConstantRateError> {
        if self.payload.len() > CONSTANT_RATE_MAX_PAYLOAD_BYTES {
            return Err(ConstantRateError::PayloadTooLarge {
                actual: self.payload.len(),
                maximum: CONSTANT_RATE_MAX_PAYLOAD_BYTES,
            });
        }
        if self.channel == MuxChannel::Cover {
            if self.class != CellClass::Bulk
                || self.lane_id != 0
                || self.flags.bits() != 0
                || !self.payload.is_empty()
            {
                return Err(ConstantRateError::InvalidCover);
            }
            return Ok(());
        }
        if self.lane_id == 0 {
            return Err(ConstantRateError::ReservedLane);
        }
        if self.flags.is_reset() && !self.payload.is_empty() {
            return Err(ConstantRateError::ResetWithPayload);
        }
        if self.flags.is_reset() && (self.flags.is_open() || self.flags.is_fin()) {
            return Err(ConstantRateError::ContradictoryFlags(self.flags.bits()));
        }
        if self.payload.is_empty() && self.flags.bits() == 0 {
            return Err(ConstantRateError::EmptyProgress);
        }
        Ok(())
    }

    fn encode(&self) -> Result<Vec<u8>, ConstantRateError> {
        self.validate()?;
        let payload_len =
            u16::try_from(self.payload.len()).map_err(|_| ConstantRateError::PayloadTooLarge {
                actual: self.payload.len(),
                maximum: CONSTANT_RATE_MAX_PAYLOAD_BYTES,
            })?;
        let mut encoded = Vec::with_capacity(MUX_HEADER_BYTES + self.payload.len());
        encoded.extend_from_slice(&MUX_MAGIC);
        encoded.push(self.channel as u8);
        encoded.push(self.class as u8);
        encoded.push(self.flags.bits());
        encoded.push(0);
        encoded.extend_from_slice(&self.lane_id.to_be_bytes());
        encoded.extend_from_slice(&payload_len.to_be_bytes());
        encoded.extend_from_slice(&self.payload);
        Ok(encoded)
    }

    fn decode(encoded: &[u8]) -> Result<Self, ConstantRateError> {
        if encoded.len() < MUX_HEADER_BYTES {
            return Err(ConstantRateError::TruncatedMux {
                actual: encoded.len(),
                minimum: MUX_HEADER_BYTES,
            });
        }
        if encoded[..4] != MUX_MAGIC {
            return Err(ConstantRateError::InvalidMuxMagic);
        }
        if encoded[7] != 0 {
            return Err(ConstantRateError::NonCanonicalMuxReserved);
        }
        let channel = MuxChannel::from_wire(encoded[4])?;
        let class = CellClass::from_wire(encoded[5])?;
        let flags = MuxFlags::new(encoded[6])?;
        let lane_id = u64::from_be_bytes(
            encoded[8..16]
                .try_into()
                .expect("checked constant-rate mux header width"),
        );
        let payload_len = usize::from(u16::from_be_bytes([encoded[16], encoded[17]]));
        let expected = MUX_HEADER_BYTES
            .checked_add(payload_len)
            .ok_or(ConstantRateError::LengthOverflow)?;
        if encoded.len() != expected {
            return Err(ConstantRateError::MuxLengthMismatch {
                declared: payload_len,
                actual: encoded.len().saturating_sub(MUX_HEADER_BYTES),
            });
        }
        Self::new(
            channel,
            class,
            lane_id,
            flags,
            encoded[MUX_HEADER_BYTES..].to_vec(),
        )
    }
}

/// Per-class queue depths in a strict fixed-rate scheduler.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct QueueDepths {
    /// Queued control frames.
    pub control: usize,
    /// Queued latency-sensitive frames.
    pub interactive: usize,
    /// Queued bulk frames.
    pub bulk: usize,
}

impl QueueDepths {
    /// Total number of queued authenticated frames.
    #[must_use]
    pub const fn total(self) -> usize {
        self.control + self.interactive + self.bulk
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct LaneKey {
    channel: MuxChannel,
    lane_id: u64,
}

/// Directional logical-lane validator for the authenticated DATAGRAM mux.
///
/// A separate instance is used for each sending direction. It prevents data
/// before OPEN, duplicate OPEN, class changes that could reorder one lane in
/// the weighted scheduler, and use after FIN/RESET.
#[derive(Debug)]
pub struct MuxLifecycle {
    lanes: HashMap<LaneKey, CellClass>,
    maximum_lanes: usize,
}

impl MuxLifecycle {
    /// Construct a bounded directional lane registry.
    #[must_use]
    pub fn new(maximum_lanes: usize) -> Self {
        Self {
            lanes: HashMap::new(),
            maximum_lanes,
        }
    }

    /// Validate and commit one mux transition.
    ///
    /// Cover does not consume a lane. OPEN reserves a non-zero lane, FIN and
    /// RESET release it, and all intervening frames must retain the OPEN class.
    ///
    /// # Errors
    /// Returns a canonical-frame or lane-lifecycle failure without mutating
    /// state when the transition is rejected.
    pub fn accept(&mut self, frame: &MuxFrame) -> Result<(), ConstantRateError> {
        frame.validate()?;
        if frame.channel == MuxChannel::Cover {
            return Ok(());
        }

        let key = LaneKey {
            channel: frame.channel,
            lane_id: frame.lane_id,
        };
        if frame.flags.is_open() {
            if self.lanes.contains_key(&key) {
                return Err(ConstantRateError::DuplicateOpen {
                    channel: frame.channel,
                    lane_id: frame.lane_id,
                });
            }
            if self.lanes.len() >= self.maximum_lanes {
                return Err(ConstantRateError::LaneCapacity {
                    maximum: self.maximum_lanes,
                });
            }
            if !frame.flags.is_fin() {
                self.lanes
                    .try_reserve(1)
                    .map_err(|_| ConstantRateError::LaneAllocation)?;
                self.lanes.insert(key, frame.class);
            }
            return Ok(());
        }

        let Some(open_class) = self.lanes.get(&key).copied() else {
            return Err(ConstantRateError::LaneNotOpen {
                channel: frame.channel,
                lane_id: frame.lane_id,
            });
        };
        if frame.class != open_class {
            return Err(ConstantRateError::LaneClassChanged {
                channel: frame.channel,
                lane_id: frame.lane_id,
                opened: open_class,
                actual: frame.class,
            });
        }
        if frame.flags.is_fin() || frame.flags.is_reset() {
            self.lanes.remove(&key);
        }
        Ok(())
    }

    /// Number of currently open lanes in this direction.
    #[must_use]
    pub fn open_lanes(&self) -> usize {
        self.lanes.len()
    }
}

/// Bounded, deterministic scheduler used by strict constant-rate send loops.
///
/// Queues never evict payload. A full queue is a fatal backpressure signal to
/// the owning circuit; silently dropping a frame would violate the logical
/// reliability contract. Calling [`Self::next_frame`] once per negotiated tick
/// emits exactly one authenticated payload frame or canonical cover.
#[derive(Debug)]
pub struct FixedRateScheduler {
    queues: [VecDeque<MuxFrame>; 3],
    queue_capacity_per_class: usize,
    lifecycle: MuxLifecycle,
    deficit: [i32; 3],
}

impl FixedRateScheduler {
    /// Construct a scheduler with explicit first-release resource bounds.
    #[must_use]
    pub fn new(queue_capacity_per_class: usize, maximum_lanes: usize) -> Self {
        Self {
            queues: [VecDeque::new(), VecDeque::new(), VecDeque::new()],
            queue_capacity_per_class,
            lifecycle: MuxLifecycle::new(maximum_lanes),
            deficit: [0; 3],
        }
    }

    /// Enqueue one logical transition for a future fixed-rate tick.
    ///
    /// # Errors
    /// Rejects caller-supplied cover, a full class queue, or an invalid lane
    /// transition. No existing payload is evicted on failure.
    pub fn enqueue(&mut self, frame: MuxFrame) -> Result<(), ConstantRateError> {
        if frame.channel == MuxChannel::Cover {
            return Err(ConstantRateError::CoverEnqueue);
        }
        let index = frame.class.scheduler_index();
        if self.queues[index].len() >= self.queue_capacity_per_class {
            return Err(ConstantRateError::QueueFull {
                class: frame.class,
                maximum: self.queue_capacity_per_class,
            });
        }
        self.queues[index]
            .try_reserve(1)
            .map_err(|_| ConstantRateError::QueueAllocation)?;
        self.lifecycle.accept(&frame)?;
        self.queues[index].push_back(frame);
        Ok(())
    }

    /// Select the next transition using smooth 8:4:1 weighted round-robin.
    ///
    /// Canonical cover is returned only when every payload queue is empty.
    #[must_use]
    pub fn next_frame(&mut self) -> MuxFrame {
        let classes = [CellClass::Control, CellClass::Interactive, CellClass::Bulk];
        let mut active_weight = 0_i32;
        let mut selected = None;
        for class in classes {
            let index = class.scheduler_index();
            if self.queues[index].is_empty() {
                self.deficit[index] = 0;
                continue;
            }
            let weight = class.scheduler_weight();
            active_weight += weight;
            self.deficit[index] += weight;
            if selected.is_none_or(|winner| self.deficit[index] > self.deficit[winner]) {
                selected = Some(index);
            }
        }
        let Some(index) = selected else {
            return MuxFrame::cover();
        };
        self.deficit[index] -= active_weight;
        let frame = self.queues[index]
            .pop_front()
            .expect("selected constant-rate queue is non-empty");
        if self.queues[index].is_empty() {
            self.deficit[index] = 0;
        }
        frame
    }

    /// Snapshot current queue depths.
    #[must_use]
    pub fn queue_depths(&self) -> QueueDepths {
        QueueDepths {
            control: self.queues[CellClass::Control.scheduler_index()].len(),
            interactive: self.queues[CellClass::Interactive.scheduler_index()].len(),
            bulk: self.queues[CellClass::Bulk.scheduler_index()].len(),
        }
    }
}

/// Fragment one logical byte sequence into canonical mux transitions.
///
/// OPEN is attached to the first fragment and FIN to the last. An empty
/// sequence is representable only when at least one lifecycle flag is present.
///
/// # Errors
/// Returns canonical-frame validation failures.
pub fn fragment_payload(
    channel: MuxChannel,
    class: CellClass,
    lane_id: u64,
    payload: &[u8],
    open: bool,
    fin: bool,
) -> Result<Vec<MuxFrame>, ConstantRateError> {
    if payload.is_empty() {
        if !open && !fin {
            return Ok(Vec::new());
        }
        let mut bits = 0;
        if open {
            bits |= MuxFlags::OPEN;
        }
        if fin {
            bits |= MuxFlags::FIN;
        }
        return Ok(vec![MuxFrame::new(
            channel,
            class,
            lane_id,
            MuxFlags::new(bits)?,
            Vec::new(),
        )?]);
    }

    let chunk_count = payload.len().div_ceil(CONSTANT_RATE_MAX_PAYLOAD_BYTES);
    payload
        .chunks(CONSTANT_RATE_MAX_PAYLOAD_BYTES)
        .enumerate()
        .map(|(index, chunk)| {
            let mut bits = 0;
            if open && index == 0 {
                bits |= MuxFlags::OPEN;
            }
            if fin && index + 1 == chunk_count {
                bits |= MuxFlags::FIN;
            }
            MuxFrame::new(
                channel,
                class,
                lane_id,
                MuxFlags::new(bits)?,
                chunk.to_vec(),
            )
        })
        .collect()
}

/// Exact fixed-size cell transmitted as one QUIC DATAGRAM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireCell {
    class: CellClass,
    record: Vec<u8>,
}

impl WireCell {
    fn new(class: CellClass, record: Vec<u8>) -> Result<Self, ConstantRateError> {
        if record.is_empty() || record.len() > CONSTANT_RATE_CELL_RECORD_BYTES {
            return Err(ConstantRateError::RecordLength {
                actual: record.len(),
                maximum: CONSTANT_RATE_CELL_RECORD_BYTES,
            });
        }
        Ok(Self { class, record })
    }

    /// Return the authenticated scheduler class.
    #[must_use]
    pub const fn class(&self) -> CellClass {
        self.class
    }

    /// Serialize into the exact v1 DATAGRAM size with canonical zero padding.
    #[must_use]
    pub fn encode(&self) -> [u8; CONSTANT_RATE_CELL_BYTES] {
        let mut encoded = [0_u8; CONSTANT_RATE_CELL_BYTES];
        encoded[0] = CELL_TYPE_DATA;
        encoded[1] = self.class as u8;
        encoded[2..4].copy_from_slice(&(self.record.len() as u16).to_be_bytes());
        encoded[4..4 + self.record.len()].copy_from_slice(&self.record);
        encoded
    }

    /// Parse one canonical fixed-size cell.
    ///
    /// # Errors
    /// Rejects alternate sizes/types/classes, empty or oversized records, and
    /// any non-zero padding byte.
    pub fn decode(encoded: &[u8]) -> Result<Self, ConstantRateError> {
        if encoded.len() != CONSTANT_RATE_CELL_BYTES {
            return Err(ConstantRateError::CellLength {
                actual: encoded.len(),
                expected: CONSTANT_RATE_CELL_BYTES,
            });
        }
        if encoded[0] != CELL_TYPE_DATA {
            return Err(ConstantRateError::UnknownCellType(encoded[0]));
        }
        let class = CellClass::from_wire(encoded[1])?;
        let record_len = usize::from(u16::from_be_bytes([encoded[2], encoded[3]]));
        if record_len == 0 || record_len > CONSTANT_RATE_CELL_RECORD_BYTES {
            return Err(ConstantRateError::RecordLength {
                actual: record_len,
                maximum: CONSTANT_RATE_CELL_RECORD_BYTES,
            });
        }
        let end = CONSTANT_RATE_CELL_HEADER_BYTES + record_len;
        if encoded[end..].iter().any(|byte| *byte != 0) {
            return Err(ConstantRateError::NonCanonicalCellPadding);
        }
        Self::new(class, encoded[4..end].to_vec())
    }
}

/// Sending half of the strict authenticated cell codec.
pub struct ConstantRateSealer {
    records: RecordSealer,
}

impl ConstantRateSealer {
    /// Seal one application or cover transition into an exact-size wire cell.
    ///
    /// # Errors
    /// Returns mux, record, or fixed-cell validation failures.
    pub fn seal(&mut self, frame: &MuxFrame) -> Result<WireCell, ConstantRateError> {
        let plaintext = frame.encode()?;
        let record = self.records.seal(&plaintext)?;
        WireCell::new(frame.class, record)
    }
}

/// Receiving half of the strict authenticated cell codec.
pub struct ConstantRateOpener {
    records: RecordOpener,
}

impl ConstantRateOpener {
    /// Authenticate and decode one exact-size wire cell.
    ///
    /// Record sequencing makes every cell, including cover, part of one
    /// continuity contract. Loss, duplication, or reordering therefore fails
    /// with [`RecordError::SequenceMismatch`].
    ///
    /// # Errors
    /// Returns fixed-cell, record-authentication, continuity, or mux failures.
    pub fn open(&mut self, encoded: &[u8]) -> Result<MuxFrame, ConstantRateError> {
        let cell = WireCell::decode(encoded)?;
        let plaintext = self.records.open(&cell.record)?;
        let frame = MuxFrame::decode(&plaintext)?;
        if frame.class != cell.class {
            return Err(ConstantRateError::ClassMismatch {
                outer: cell.class,
                authenticated: frame.class,
            });
        }
        Ok(frame)
    }
}

/// Derive the one reserved record context used by strict constant-rate cells.
///
/// The context index is outside QUIC's stream-index space, so it cannot collide
/// with the ordinary stream record contexts used during handshake migration.
///
/// # Errors
/// Forwards record-layer key derivation and context-uniqueness failures.
pub fn codec(
    records: &RecordLayer,
) -> Result<(ConstantRateSealer, ConstantRateOpener), ConstantRateError> {
    let DuplexRecordLayer { sealer, opener } = records.stream(RecordStreamContext::new(
        RecordEndpoint::Client,
        RecordStreamKind::Bidirectional,
        CONSTANT_RATE_RECORD_CONTEXT_INDEX,
    ))?;
    Ok((
        ConstantRateSealer { records: sealer },
        ConstantRateOpener { records: opener },
    ))
}

/// Strict constant-rate framing failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ConstantRateError {
    /// Fixed cell did not have the negotiated size.
    #[error("constant-rate cell has {actual} bytes; expected {expected}")]
    CellLength {
        /// Received byte length.
        actual: usize,
        /// Exact negotiated byte length.
        expected: usize,
    },
    /// Fixed cell used an undefined type.
    #[error("constant-rate cell uses unknown type {0:#04x}")]
    UnknownCellType(u8),
    /// Fixed cell or authenticated mux used an undefined scheduler class.
    #[error("constant-rate cell uses unknown class {0:#04x}")]
    UnknownClass(u8),
    /// Fixed cell carried an invalid authenticated record length.
    #[error("constant-rate record has {actual} bytes; maximum is {maximum}")]
    RecordLength {
        /// Received or declared record length.
        actual: usize,
        /// Maximum record length that fits one cell.
        maximum: usize,
    },
    /// Fixed cell padding was not canonical zero fill.
    #[error("constant-rate cell has non-zero canonical padding")]
    NonCanonicalCellPadding,
    /// Authenticated mux header was truncated.
    #[error("constant-rate mux has {actual} bytes; minimum is {minimum}")]
    TruncatedMux {
        /// Received mux length.
        actual: usize,
        /// Minimum complete mux-header length.
        minimum: usize,
    },
    /// Authenticated mux magic/version was invalid.
    #[error("constant-rate mux magic or version is invalid")]
    InvalidMuxMagic,
    /// Authenticated mux used a reserved channel identifier.
    #[error("constant-rate mux uses unknown channel {0:#04x}")]
    UnknownChannel(u8),
    /// Authenticated mux used reserved flag bits.
    #[error("constant-rate mux uses invalid flags {0:#04x}")]
    InvalidFlags(u8),
    /// Authenticated mux reserved byte was not canonical zero.
    #[error("constant-rate mux reserved byte is non-zero")]
    NonCanonicalMuxReserved,
    /// Authenticated mux length arithmetic overflowed.
    #[error("constant-rate mux length overflow")]
    LengthOverflow,
    /// Authenticated mux payload length did not match the record.
    #[error("constant-rate mux declared {declared} payload bytes but carried {actual}")]
    MuxLengthMismatch {
        /// Payload length committed by the mux header.
        declared: usize,
        /// Payload bytes actually present.
        actual: usize,
    },
    /// Authenticated mux payload exceeded one cell.
    #[error("constant-rate payload has {actual} bytes; maximum is {maximum}")]
    PayloadTooLarge {
        /// Supplied payload length.
        actual: usize,
        /// Maximum payload length in one authenticated cell.
        maximum: usize,
    },
    /// The zero lane is reserved for cover.
    #[error("constant-rate logical lane zero is reserved for cover")]
    ReservedLane,
    /// Cover did not use its one canonical representation.
    #[error("constant-rate cover transition is not canonical")]
    InvalidCover,
    /// RESET attempted to carry payload bytes.
    #[error("constant-rate RESET transition must not carry payload")]
    ResetWithPayload,
    /// Mutually exclusive lifecycle transitions were combined.
    #[error("constant-rate mux uses contradictory lifecycle flags {0:#04x}")]
    ContradictoryFlags(u8),
    /// A non-cover cell made no payload or lifecycle progress.
    #[error("constant-rate application transition is empty")]
    EmptyProgress,
    /// A caller attempted to enqueue cover instead of letting the scheduler generate it.
    #[error("constant-rate cover cells may only be generated by the fixed-rate scheduler")]
    CoverEnqueue,
    /// A bounded scheduler queue cannot accept another frame.
    #[error("constant-rate {class:?} queue reached its {maximum}-frame capacity")]
    QueueFull {
        /// Scheduler class whose queue is full.
        class: CellClass,
        /// Configured per-class frame capacity.
        maximum: usize,
    },
    /// A scheduler queue could not reserve memory.
    #[error("constant-rate scheduler queue allocation failed")]
    QueueAllocation,
    /// A logical lane was opened more than once in one direction.
    #[error("constant-rate {channel:?} lane {lane_id} received duplicate OPEN")]
    DuplicateOpen {
        /// Logical consumer owning the lane.
        channel: MuxChannel,
        /// Duplicate non-zero lane identifier.
        lane_id: u64,
    },
    /// A logical transition referred to a lane that is not open.
    #[error("constant-rate {channel:?} lane {lane_id} is not open")]
    LaneNotOpen {
        /// Logical consumer named by the transition.
        channel: MuxChannel,
        /// Non-zero lane identifier that has no open state.
        lane_id: u64,
    },
    /// A lane changed scheduler class after OPEN, which could reorder its bytes.
    #[error("constant-rate {channel:?} lane {lane_id} changed class from {opened:?} to {actual:?}")]
    LaneClassChanged {
        /// Logical consumer owning the lane.
        channel: MuxChannel,
        /// Non-zero lane identifier.
        lane_id: u64,
        /// Scheduler class authenticated by OPEN.
        opened: CellClass,
        /// Scheduler class supplied by the rejected transition.
        actual: CellClass,
    },
    /// Too many logical lanes are open in one direction.
    #[error("constant-rate logical lane capacity {maximum} is exhausted")]
    LaneCapacity {
        /// Configured maximum number of simultaneous lanes.
        maximum: usize,
    },
    /// The lane registry could not reserve memory.
    #[error("constant-rate logical lane registry allocation failed")]
    LaneAllocation,
    /// Outer scheduling class disagreed with the authenticated mux class.
    #[error(
        "constant-rate outer class {outer:?} does not match authenticated class {authenticated:?}"
    )]
    ClassMismatch {
        /// Unauthenticated routing class in the fixed outer header.
        outer: CellClass,
        /// Authenticated routing class in the opened mux frame.
        authenticated: CellClass,
    },
    /// Post-handshake authentication or continuity failed.
    #[error(transparent)]
    Record(#[from] RecordError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SessionKey;

    fn codecs() -> (
        ConstantRateSealer,
        ConstantRateOpener,
        ConstantRateSealer,
        ConstantRateOpener,
    ) {
        let key = (0_u8..32).collect::<Vec<_>>();
        let client = RecordLayer::new(SessionKey::new(key.clone()), RecordEndpoint::Client)
            .expect("client records");
        let relay =
            RecordLayer::new(SessionKey::new(key), RecordEndpoint::Relay).expect("relay records");
        let (client_sealer, client_opener) = codec(&client).expect("client codec");
        let (relay_sealer, relay_opener) = codec(&relay).expect("relay codec");
        (client_sealer, client_opener, relay_sealer, relay_opener)
    }

    #[test]
    fn authenticated_payload_and_cover_round_trip_both_directions() {
        let (mut client_sealer, mut client_opener, mut relay_sealer, mut relay_opener) = codecs();
        let open = MuxFrame::new(
            MuxChannel::Vpn,
            CellClass::Interactive,
            7,
            MuxFlags::new(MuxFlags::OPEN).expect("flags"),
            b"packet".to_vec(),
        )
        .expect("frame");
        let encoded = client_sealer.seal(&open).expect("seal").encode();
        assert_eq!(encoded.len(), CONSTANT_RATE_CELL_BYTES);
        assert_eq!(relay_opener.open(&encoded).expect("open"), open);

        let cover = MuxFrame::cover();
        let encoded = relay_sealer.seal(&cover).expect("seal cover").encode();
        assert_eq!(client_opener.open(&encoded).expect("open cover"), cover);
    }

    #[test]
    fn loss_duplication_and_reordering_fail_closed() {
        let (mut client_sealer, _, _, mut relay_opener) = codecs();
        let first = client_sealer
            .seal(&MuxFrame::cover())
            .expect("first")
            .encode();
        let second = client_sealer
            .seal(&MuxFrame::cover())
            .expect("second")
            .encode();
        let third = client_sealer
            .seal(&MuxFrame::cover())
            .expect("third")
            .encode();

        relay_opener.open(&first).expect("first opens");
        let error = relay_opener
            .open(&third)
            .expect_err("skipping a tick must fail continuity");
        assert!(matches!(
            error,
            ConstantRateError::Record(RecordError::SequenceMismatch {
                expected: 1,
                actual: 2
            })
        ));
        relay_opener
            .open(&second)
            .expect("expected tick remains valid");
        let error = relay_opener
            .open(&second)
            .expect_err("duplicate tick must fail continuity");
        assert!(matches!(
            error,
            ConstantRateError::Record(RecordError::SequenceMismatch {
                expected: 2,
                actual: 1
            })
        ));
    }

    #[test]
    fn tampering_and_noncanonical_padding_fail_closed() {
        let (mut client_sealer, _, _, mut relay_opener) = codecs();
        let mut encoded = client_sealer
            .seal(&MuxFrame::cover())
            .expect("seal")
            .encode();
        encoded[CONSTANT_RATE_CELL_HEADER_BYTES + RECORD_HEADER_LEN] ^= 1;
        assert!(matches!(
            relay_opener.open(&encoded),
            Err(ConstantRateError::Record(RecordError::Authentication))
        ));

        let (mut client_sealer, _, _, mut relay_opener) = codecs();
        let mut encoded = client_sealer
            .seal(&MuxFrame::cover())
            .expect("seal")
            .encode();
        encoded[CONSTANT_RATE_CELL_BYTES - 1] = 1;
        assert_eq!(
            relay_opener.open(&encoded),
            Err(ConstantRateError::NonCanonicalCellPadding)
        );
    }

    #[test]
    fn mux_bounds_and_reserved_shapes_are_rejected() {
        assert!(matches!(
            MuxFrame::new(
                MuxChannel::Application,
                CellClass::Bulk,
                0,
                MuxFlags::new(MuxFlags::OPEN).expect("flags"),
                vec![1],
            ),
            Err(ConstantRateError::ReservedLane)
        ));
        assert!(matches!(
            MuxFrame::new(
                MuxChannel::Vpn,
                CellClass::Control,
                1,
                MuxFlags::new(MuxFlags::RESET).expect("flags"),
                vec![1],
            ),
            Err(ConstantRateError::ResetWithPayload)
        ));
        assert!(matches!(
            MuxFrame::new(
                MuxChannel::Measurement,
                CellClass::Control,
                1,
                MuxFlags::default(),
                Vec::new(),
            ),
            Err(ConstantRateError::EmptyProgress)
        ));
        assert!(matches!(
            MuxFrame::new(
                MuxChannel::Application,
                CellClass::Bulk,
                1,
                MuxFlags::default(),
                vec![0; CONSTANT_RATE_MAX_PAYLOAD_BYTES + 1],
            ),
            Err(ConstantRateError::PayloadTooLarge { .. })
        ));
        assert!(matches!(
            MuxFrame::new(
                MuxChannel::Exit,
                CellClass::Bulk,
                1,
                MuxFlags::new(MuxFlags::OPEN | MuxFlags::RESET).expect("flags"),
                Vec::new(),
            ),
            Err(ConstantRateError::ContradictoryFlags(_))
        ));
    }

    fn frame(
        channel: MuxChannel,
        class: CellClass,
        lane_id: u64,
        flags: u8,
        payload: &[u8],
    ) -> MuxFrame {
        MuxFrame::new(
            channel,
            class,
            lane_id,
            MuxFlags::new(flags).expect("flags"),
            payload.to_vec(),
        )
        .expect("frame")
    }

    #[test]
    fn lane_lifecycle_is_directional_bounded_and_fail_closed() {
        let mut lanes = MuxLifecycle::new(1);
        let open = frame(
            MuxChannel::Vpn,
            CellClass::Interactive,
            7,
            MuxFlags::OPEN,
            b"a",
        );
        lanes.accept(&open).expect("open lane");
        assert_eq!(lanes.open_lanes(), 1);
        assert!(matches!(
            lanes.accept(&open),
            Err(ConstantRateError::DuplicateOpen { lane_id: 7, .. })
        ));
        assert!(matches!(
            lanes.accept(&frame(
                MuxChannel::Exit,
                CellClass::Bulk,
                8,
                MuxFlags::OPEN,
                b"b",
            )),
            Err(ConstantRateError::LaneCapacity { maximum: 1 })
        ));
        assert!(matches!(
            lanes.accept(&frame(MuxChannel::Vpn, CellClass::Control, 7, 0, b"b",)),
            Err(ConstantRateError::LaneClassChanged { lane_id: 7, .. })
        ));
        lanes
            .accept(&frame(
                MuxChannel::Vpn,
                CellClass::Interactive,
                7,
                MuxFlags::FIN,
                b"z",
            ))
            .expect("finish lane");
        assert_eq!(lanes.open_lanes(), 0);
        assert!(matches!(
            lanes.accept(&frame(
                MuxChannel::Vpn,
                CellClass::Interactive,
                7,
                0,
                b"late",
            )),
            Err(ConstantRateError::LaneNotOpen { lane_id: 7, .. })
        ));
    }

    #[test]
    fn strict_scheduler_preserves_payload_and_never_evicts_on_overflow() {
        let mut scheduler = FixedRateScheduler::new(2, 3);
        scheduler
            .enqueue(frame(
                MuxChannel::Measurement,
                CellClass::Control,
                1,
                MuxFlags::OPEN,
                b"one",
            ))
            .expect("first");
        scheduler
            .enqueue(frame(
                MuxChannel::Measurement,
                CellClass::Control,
                1,
                0,
                b"two",
            ))
            .expect("second");
        let overflow = scheduler.enqueue(frame(
            MuxChannel::Measurement,
            CellClass::Control,
            1,
            0,
            b"three",
        ));
        assert!(matches!(
            overflow,
            Err(ConstantRateError::QueueFull {
                class: CellClass::Control,
                maximum: 2
            })
        ));
        assert_eq!(scheduler.next_frame().payload, b"one");
        assert_eq!(scheduler.next_frame().payload, b"two");
        assert_eq!(scheduler.next_frame(), MuxFrame::cover());
    }

    #[test]
    fn strict_scheduler_honors_weighted_classes() {
        let mut scheduler = FixedRateScheduler::new(16, 3);
        for (channel, class, lane) in [
            (MuxChannel::Measurement, CellClass::Control, 1),
            (MuxChannel::Vpn, CellClass::Interactive, 2),
            (MuxChannel::Exit, CellClass::Bulk, 3),
        ] {
            for index in 0..13_u8 {
                scheduler
                    .enqueue(frame(
                        channel,
                        class,
                        lane,
                        if index == 0 { MuxFlags::OPEN } else { 0 },
                        &[index],
                    ))
                    .expect("enqueue class backlog");
            }
        }
        let mut counts = [0_usize; 3];
        for _ in 0..13 {
            let frame = scheduler.next_frame();
            counts[frame.class.scheduler_index()] += 1;
        }
        assert_eq!(counts, [8, 4, 1]);
    }

    #[test]
    fn fragmentation_marks_only_boundary_transitions() {
        let payload = vec![0x5a; CONSTANT_RATE_MAX_PAYLOAD_BYTES * 2 + 1];
        let frames = fragment_payload(
            MuxChannel::Application,
            CellClass::Bulk,
            9,
            &payload,
            true,
            true,
        )
        .expect("fragment");
        assert_eq!(frames.len(), 3);
        assert!(frames[0].flags.is_open());
        assert!(!frames[0].flags.is_fin());
        assert_eq!(frames[1].flags.bits(), 0);
        assert!(!frames[2].flags.is_open());
        assert!(frames[2].flags.is_fin());
        assert_eq!(
            frames
                .iter()
                .flat_map(|frame| frame.payload.iter().copied())
                .collect::<Vec<_>>(),
            payload
        );
    }

    #[test]
    fn every_payload_channel_and_cover_share_fixed_cells_across_hops() {
        let first_session_key = (0_u8..32).collect::<Vec<_>>();
        let second_session_key = (32_u8..64).collect::<Vec<_>>();
        let first_client = RecordLayer::new(
            SessionKey::new(first_session_key.clone()),
            RecordEndpoint::Client,
        )
        .expect("first client records");
        let first_relay =
            RecordLayer::new(SessionKey::new(first_session_key), RecordEndpoint::Relay)
                .expect("first relay records");
        let second_client = RecordLayer::new(
            SessionKey::new(second_session_key.clone()),
            RecordEndpoint::Client,
        )
        .expect("second client records");
        let second_relay =
            RecordLayer::new(SessionKey::new(second_session_key), RecordEndpoint::Relay)
                .expect("second relay records");
        let (mut first_sealer, _) = codec(&first_client).expect("first sender");
        let (_, mut first_opener) = codec(&first_relay).expect("first receiver");
        let (mut second_sealer, _) = codec(&second_client).expect("second sender");
        let (_, mut second_opener) = codec(&second_relay).expect("second receiver");
        let frames = [
            frame(
                MuxChannel::Application,
                CellClass::Bulk,
                1,
                MuxFlags::OPEN | MuxFlags::FIN,
                b"application",
            ),
            frame(
                MuxChannel::Exit,
                CellClass::Bulk,
                2,
                MuxFlags::OPEN | MuxFlags::FIN,
                b"exit",
            ),
            frame(
                MuxChannel::Measurement,
                CellClass::Control,
                3,
                MuxFlags::OPEN | MuxFlags::FIN,
                b"measurement",
            ),
            frame(
                MuxChannel::Vpn,
                CellClass::Interactive,
                4,
                MuxFlags::OPEN | MuxFlags::FIN,
                b"vpn",
            ),
            MuxFrame::cover(),
        ];
        for expected in frames {
            let first_wire = first_sealer
                .seal(&expected)
                .expect("seal first hop")
                .encode();
            assert_eq!(first_wire.len(), CONSTANT_RATE_CELL_BYTES);
            let forwarded = first_opener.open(&first_wire).expect("open first hop");
            let second_wire = second_sealer
                .seal(&forwarded)
                .expect("seal second hop")
                .encode();
            assert_eq!(second_wire.len(), CONSTANT_RATE_CELL_BYTES);
            assert_eq!(
                second_opener.open(&second_wire).expect("open second hop"),
                expected
            );
        }
    }
}
