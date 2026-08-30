use super::{
    CertificateRef, ContextId, HeightContext, Phase, Proposal, ProposalJustification,
    QuorumCertificate, Round, Subject, TimeoutCertificate, TimeoutVote, ValidatorId, Vote,
    reducer::FinalizedHeight,
    refinement::{self, StrictSameRoundTimeoutUpgradeProjection},
};
use std::{collections::BTreeMap, error::Error, fmt};
/// Canonical Sumeragi-v2 safety-WAL file magic.
pub const SAFETY_WAL_FILE_MAGIC: [u8; 8] = *b"SUMV2WAL";
/// Canonical Sumeragi-v2 safety-WAL frame magic.
pub const SAFETY_WAL_FRAME_MAGIC: [u8; 4] = *b"S2FR";
/// Current safety-WAL byte-layout revision.
pub const SAFETY_WAL_FORMAT_VERSION: u16 = 1;
/// Byte width of every safety-WAL checksum and hash-chain link.
pub const SAFETY_WAL_HASH_LEN: usize = 32;
/// Maximum encoded payload accepted in one safety-WAL frame.
pub const SAFETY_WAL_MAX_RECORD_BYTES: usize = 16 * 1024 * 1024;
const SAFETY_WAL_FILE_HEADER_PREFIX_LEN: usize = SAFETY_WAL_FILE_MAGIC.len()
    + 2
    + 2
    + SAFETY_WAL_HASH_LEN
    + SAFETY_WAL_HASH_LEN
    + 8
    + SAFETY_WAL_HASH_LEN;
/// Canonical byte width of a complete safety-WAL file header.
pub const SAFETY_WAL_FILE_HEADER_LEN: usize =
    SAFETY_WAL_FILE_HEADER_PREFIX_LEN + SAFETY_WAL_HASH_LEN;
/// Canonical byte width of a frame header before its payload and checksum.
pub const SAFETY_WAL_FRAME_HEADER_LEN: usize =
    SAFETY_WAL_FRAME_MAGIC.len() + 8 + 4 + SAFETY_WAL_HASH_LEN;
/// Hash function supplied by the production adapter for WAL framing.
///
/// The production mapping uses BLAKE3. Keeping the function behind this tiny
/// interface lets the pure consensus crate own the exact framing and recovery
/// relation without importing a second cryptographic implementation. Hash
/// collision resistance remains a documented trusted contract.
pub trait WalFileHasher {
    /// Hash canonical bytes into one fixed-width WAL digest.
    fn hash(&self, bytes: &[u8]) -> [u8; SAFETY_WAL_HASH_LEN];
}
impl<F> WalFileHasher for F
where
    F: Fn(&[u8]) -> [u8; SAFETY_WAL_HASH_LEN],
{
    fn hash(&self, bytes: &[u8]) -> [u8; SAFETY_WAL_HASH_LEN] {
        self(bytes)
    }
}
/// Exact height-context and validator-process identity frozen into a WAL header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WalFileIdentity {
    protocol_version: u16,
    network_id: [u8; SAFETY_WAL_HASH_LEN],
    context_id: ContextId,
    height: u64,
    consensus_key_hash: [u8; SAFETY_WAL_HASH_LEN],
}
impl WalFileIdentity {
    /// Construct the exact identity expected by one validator process.
    #[must_use]
    pub const fn new(
        protocol_version: u16,
        network_id: [u8; SAFETY_WAL_HASH_LEN],
        context_id: ContextId,
        height: u64,
        consensus_key_hash: [u8; SAFETY_WAL_HASH_LEN],
    ) -> Self {
        Self {
            protocol_version,
            network_id,
            context_id,
            height,
            consensus_key_hash,
        }
    }
    /// Return the consensus wire-protocol revision.
    #[must_use]
    pub const fn protocol_version(self) -> u16 {
        self.protocol_version
    }
    /// Return the exact genesis-derived network identifier.
    #[must_use]
    pub const fn network_id(self) -> [u8; SAFETY_WAL_HASH_LEN] {
        self.network_id
    }
    /// Return the exact frozen height-context identifier.
    #[must_use]
    pub const fn context_id(self) -> ContextId {
        self.context_id
    }
    /// Return the block height owned by this WAL.
    #[must_use]
    pub const fn height(self) -> u64 {
        self.height
    }
    /// Return the local consensus-public-key digest.
    #[must_use]
    pub const fn consensus_key_hash(self) -> [u8; SAFETY_WAL_HASH_LEN] {
        self.consensus_key_hash
    }
}
/// Header field whose persisted identity differs from the running validator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WalIdentityField {
    /// Consensus wire-protocol revision.
    ProtocolVersion,
    /// Exact genesis-derived network identifier.
    NetworkId,
    /// Frozen height-context identifier.
    ContextId,
    /// Block height owned by the WAL.
    Height,
    /// Local consensus-key digest.
    ConsensusKeyHash,
}
/// Structural failure in a complete WAL header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WalHeaderCorruption {
    /// Fewer bytes than one complete header were present.
    Truncated,
    /// The file magic did not identify the Sumeragi-v2 WAL.
    Magic,
    /// The byte-layout revision is unsupported.
    FormatVersion,
    /// The complete header checksum did not match its canonical prefix.
    Checksum,
}
/// Structural or hash-chain failure in a complete WAL frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WalFrameCorruption {
    /// The frame magic was invalid.
    Magic,
    /// The frame sequence was missing, duplicated, or reordered.
    Sequence,
    /// The declared record length exceeds the protocol safety bound.
    RecordLength,
    /// The frame does not extend the preceding complete frame hash.
    PreviousHash,
    /// The complete frame checksum did not match its canonical bytes.
    Checksum,
}
/// Failure while encoding or recovering the canonical safety-WAL bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WalCodecError {
    /// The file header is missing or corrupt.
    InvalidHeader(WalHeaderCorruption),
    /// The file belongs to another protocol, chain, or consensus key.
    IdentityMismatch(WalIdentityField),
    /// A complete frame is corrupt. Only an incomplete final frame may be ignored.
    CorruptFrame {
        /// Sequence expected at the corrupt frame boundary.
        sequence: u64,
        /// Exact integrity check that failed.
        reason: WalFrameCorruption,
    },
    /// A record exceeds the fixed frame-size safety bound.
    RecordTooLarge {
        /// Supplied payload byte length.
        actual: usize,
        /// Maximum accepted payload byte length.
        maximum: usize,
    },
    /// The next frame sequence cannot be represented.
    SequenceOverflow,
}
impl fmt::Display for WalCodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHeader(reason) => {
                write!(formatter, "invalid safety-WAL header: {reason:?}")
            }
            Self::IdentityMismatch(field) => {
                write!(formatter, "safety-WAL identity mismatch: {field:?}")
            }
            Self::CorruptFrame { sequence, reason } => {
                write!(formatter, "corrupt safety-WAL frame {sequence}: {reason:?}")
            }
            Self::RecordTooLarge { actual, maximum } => write!(
                formatter,
                "safety-WAL record is too large: {actual} bytes (maximum {maximum})"
            ),
            Self::SequenceOverflow => formatter.write_str("safety-WAL frame sequence overflow"),
        }
    }
}
impl Error for WalCodecError {}
/// One verified complete frame recovered from canonical WAL bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecoveredWalRecord {
    sequence: u64,
    payload: Vec<u8>,
    frame_hash: [u8; SAFETY_WAL_HASH_LEN],
}
impl RecoveredWalRecord {
    /// Return the physical frame sequence, starting at zero.
    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }
    /// Return the opaque canonical payload bytes.
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
    /// Return the checksum of the exact complete frame accepted by recovery.
    #[must_use]
    pub const fn frame_hash(&self) -> [u8; SAFETY_WAL_HASH_LEN] {
        self.frame_hash
    }
}
/// Result of validating the complete WAL prefix.
///
/// `valid_prefix_len` is the only permitted truncation point. When
/// `incomplete_tail` is true, bytes after it are an unacknowledged final append
/// and must be removed and synchronized before the file is opened for append.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalFileRecovery {
    records: Vec<RecoveredWalRecord>,
    valid_prefix_len: usize,
    incomplete_tail: bool,
    next_sequence: u64,
    last_frame_hash: [u8; SAFETY_WAL_HASH_LEN],
}
impl WalFileRecovery {
    /// Return all complete hash-chained records in physical order.
    #[must_use]
    pub fn records(&self) -> &[RecoveredWalRecord] {
        &self.records
    }
    /// Return the exact byte boundary following the last complete frame.
    #[must_use]
    pub const fn valid_prefix_len(&self) -> usize {
        self.valid_prefix_len
    }
    /// Report whether an incomplete, unacknowledged final append was ignored.
    #[must_use]
    pub const fn has_incomplete_tail(&self) -> bool {
        self.incomplete_tail
    }
    /// Return the physical sequence required for the next append.
    #[must_use]
    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }
    /// Return the hash-chain link required for the next append.
    #[must_use]
    pub const fn last_frame_hash(&self) -> [u8; SAFETY_WAL_HASH_LEN] {
        self.last_frame_hash
    }
}
/// Canonical bytes and hash of one planned WAL append.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncodedWalFrame {
    sequence: u64,
    bytes: Vec<u8>,
    frame_hash: [u8; SAFETY_WAL_HASH_LEN],
}
impl EncodedWalFrame {
    /// Return the physical frame sequence.
    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }
    /// Return the exact bytes to append with one `write_all` operation.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Return the hash-chain link for the next frame.
    #[must_use]
    pub const fn frame_hash(&self) -> [u8; SAFETY_WAL_HASH_LEN] {
        self.frame_hash
    }
}
/// Encode the canonical, checksummed safety-WAL file header.
#[must_use]
pub fn encode_wal_file_header(
    identity: WalFileIdentity,
    hasher: &impl WalFileHasher,
) -> [u8; SAFETY_WAL_FILE_HEADER_LEN] {
    let mut header = [0_u8; SAFETY_WAL_FILE_HEADER_LEN];
    let mut offset = 0;
    header[offset..offset + SAFETY_WAL_FILE_MAGIC.len()].copy_from_slice(&SAFETY_WAL_FILE_MAGIC);
    offset += SAFETY_WAL_FILE_MAGIC.len();
    header[offset..offset + 2].copy_from_slice(&SAFETY_WAL_FORMAT_VERSION.to_le_bytes());
    offset += 2;
    header[offset..offset + 2].copy_from_slice(&identity.protocol_version.to_le_bytes());
    offset += 2;
    header[offset..offset + SAFETY_WAL_HASH_LEN].copy_from_slice(&identity.network_id);
    offset += SAFETY_WAL_HASH_LEN;
    header[offset..offset + SAFETY_WAL_HASH_LEN].copy_from_slice(identity.context_id.as_bytes());
    offset += SAFETY_WAL_HASH_LEN;
    header[offset..offset + 8].copy_from_slice(&identity.height.to_le_bytes());
    offset += 8;
    header[offset..offset + SAFETY_WAL_HASH_LEN].copy_from_slice(&identity.consensus_key_hash);
    let checksum = hasher.hash(&header[..SAFETY_WAL_FILE_HEADER_PREFIX_LEN]);
    header[SAFETY_WAL_FILE_HEADER_PREFIX_LEN..].copy_from_slice(&checksum);
    header
}
/// Encode one complete canonical safety-WAL frame.
///
/// # Errors
///
/// Returns an error before producing bytes when the payload exceeds the fixed
/// limit or the sequence has no representable successor.
pub fn encode_wal_frame(
    sequence: u64,
    previous_hash: [u8; SAFETY_WAL_HASH_LEN],
    payload: &[u8],
    hasher: &impl WalFileHasher,
) -> Result<EncodedWalFrame, WalCodecError> {
    if payload.len() > SAFETY_WAL_MAX_RECORD_BYTES {
        return Err(WalCodecError::RecordTooLarge {
            actual: payload.len(),
            maximum: SAFETY_WAL_MAX_RECORD_BYTES,
        });
    }
    let payload_len = u32::try_from(payload.len()).map_err(|_| WalCodecError::RecordTooLarge {
        actual: payload.len(),
        maximum: SAFETY_WAL_MAX_RECORD_BYTES,
    })?;
    sequence
        .checked_add(1)
        .ok_or(WalCodecError::SequenceOverflow)?;
    let mut bytes =
        Vec::with_capacity(SAFETY_WAL_FRAME_HEADER_LEN + payload.len() + SAFETY_WAL_HASH_LEN);
    bytes.extend_from_slice(&SAFETY_WAL_FRAME_MAGIC);
    bytes.extend_from_slice(&sequence.to_le_bytes());
    bytes.extend_from_slice(&payload_len.to_le_bytes());
    bytes.extend_from_slice(&previous_hash);
    bytes.extend_from_slice(payload);
    let frame_hash = hasher.hash(&bytes);
    bytes.extend_from_slice(&frame_hash);
    Ok(EncodedWalFrame {
        sequence,
        bytes,
        frame_hash,
    })
}
/// Validate a WAL header and its maximal complete hash-chained frame prefix.
///
/// An incomplete final frame is returned as an unacknowledged tail and never
/// exposed as a recovered record. Every complete-frame integrity failure,
/// including a failure before an incomplete suffix, is returned fail closed.
///
/// # Errors
///
/// Returns an error for a malformed header, identity mismatch, corrupt
/// complete frame, out-of-order sequence, oversized record, or sequence
/// exhaustion.
#[allow(clippy::too_many_lines)]
pub fn recover_wal_file(
    bytes: &[u8],
    expected_identity: WalFileIdentity,
    hasher: &impl WalFileHasher,
) -> Result<WalFileRecovery, WalCodecError> {
    validate_wal_file_header(bytes, expected_identity, hasher)?;
    let mut offset = SAFETY_WAL_FILE_HEADER_LEN;
    let mut expected_sequence = 0_u64;
    let mut previous_hash = [0_u8; SAFETY_WAL_HASH_LEN];
    let mut records = Vec::new();
    let mut incomplete_tail = false;
    while offset < bytes.len() {
        if bytes.len().saturating_sub(offset) < SAFETY_WAL_FRAME_HEADER_LEN {
            incomplete_tail = true;
            break;
        }
        let frame_start = offset;
        if bytes[offset..offset + SAFETY_WAL_FRAME_MAGIC.len()] != SAFETY_WAL_FRAME_MAGIC {
            return Err(WalCodecError::CorruptFrame {
                sequence: expected_sequence,
                reason: WalFrameCorruption::Magic,
            });
        }
        offset += SAFETY_WAL_FRAME_MAGIC.len();
        let sequence = read_wal_u64(&bytes[offset..offset + 8]);
        offset += 8;
        let payload_len =
            usize::try_from(read_wal_u32(&bytes[offset..offset + 4])).unwrap_or(usize::MAX);
        offset += 4;
        let mut encoded_previous = [0_u8; SAFETY_WAL_HASH_LEN];
        encoded_previous.copy_from_slice(&bytes[offset..offset + SAFETY_WAL_HASH_LEN]);
        offset += SAFETY_WAL_HASH_LEN;
        if sequence != expected_sequence {
            return Err(WalCodecError::CorruptFrame {
                sequence: expected_sequence,
                reason: WalFrameCorruption::Sequence,
            });
        }
        if payload_len > SAFETY_WAL_MAX_RECORD_BYTES {
            return Err(WalCodecError::CorruptFrame {
                sequence,
                reason: WalFrameCorruption::RecordLength,
            });
        }
        if encoded_previous != previous_hash {
            return Err(WalCodecError::CorruptFrame {
                sequence,
                reason: WalFrameCorruption::PreviousHash,
            });
        }
        let frame_len = SAFETY_WAL_FRAME_HEADER_LEN
            .checked_add(payload_len)
            .and_then(|length| length.checked_add(SAFETY_WAL_HASH_LEN))
            .ok_or(WalCodecError::CorruptFrame {
                sequence,
                reason: WalFrameCorruption::RecordLength,
            })?;
        if bytes.len().saturating_sub(frame_start) < frame_len {
            incomplete_tail = true;
            offset = frame_start;
            break;
        }
        let payload_end = offset + payload_len;
        let payload = bytes[offset..payload_end].to_vec();
        let mut encoded_hash = [0_u8; SAFETY_WAL_HASH_LEN];
        encoded_hash.copy_from_slice(&bytes[payload_end..payload_end + SAFETY_WAL_HASH_LEN]);
        let calculated_hash = hasher.hash(&bytes[frame_start..payload_end]);
        if encoded_hash != calculated_hash {
            return Err(WalCodecError::CorruptFrame {
                sequence,
                reason: WalFrameCorruption::Checksum,
            });
        }
        if expected_sequence == u64::MAX {
            return Err(WalCodecError::SequenceOverflow);
        }
        if !wal_complete_frame_valid_body!(
            false,
            true,
            expected_sequence,
            u64::MAX,
            sequence,
            payload_len,
            SAFETY_WAL_MAX_RECORD_BYTES,
            encoded_previous,
            previous_hash,
            encoded_hash,
            calculated_hash,
        ) {
            // Field-specific branches above provide stable diagnostics. This
            // final shared production/Verus gate is intentionally redundant:
            // a future parser edit cannot expose a frame while omitting one of
            // the verified acceptance predicates.
            return Err(WalCodecError::CorruptFrame {
                sequence,
                reason: WalFrameCorruption::Checksum,
            });
        }
        records.push(RecoveredWalRecord {
            sequence,
            payload,
            frame_hash: encoded_hash,
        });
        previous_hash = encoded_hash;
        expected_sequence = expected_sequence
            .checked_add(1)
            .ok_or(WalCodecError::SequenceOverflow)?;
        offset = payload_end + SAFETY_WAL_HASH_LEN;
    }
    Ok(WalFileRecovery {
        records,
        valid_prefix_len: offset,
        incomplete_tail,
        next_sequence: expected_sequence,
        last_frame_hash: previous_hash,
    })
}
fn validate_wal_file_header(
    bytes: &[u8],
    expected_identity: WalFileIdentity,
    hasher: &impl WalFileHasher,
) -> Result<(), WalCodecError> {
    if bytes.len() < SAFETY_WAL_FILE_HEADER_LEN {
        return Err(WalCodecError::InvalidHeader(WalHeaderCorruption::Truncated));
    }
    let magic_matches = bytes[..SAFETY_WAL_FILE_MAGIC.len()] == SAFETY_WAL_FILE_MAGIC;
    if !magic_matches {
        return Err(WalCodecError::InvalidHeader(WalHeaderCorruption::Magic));
    }
    let mut offset = SAFETY_WAL_FILE_MAGIC.len();
    let format_matches = read_wal_u16(&bytes[offset..offset + 2]) == SAFETY_WAL_FORMAT_VERSION;
    if !format_matches {
        return Err(WalCodecError::InvalidHeader(
            WalHeaderCorruption::FormatVersion,
        ));
    }
    offset += 2;
    let actual_protocol = read_wal_u16(&bytes[offset..offset + 2]);
    if actual_protocol != expected_identity.protocol_version {
        return Err(WalCodecError::IdentityMismatch(
            WalIdentityField::ProtocolVersion,
        ));
    }
    offset += 2;
    let mut actual_network_id = [0_u8; SAFETY_WAL_HASH_LEN];
    actual_network_id.copy_from_slice(&bytes[offset..offset + SAFETY_WAL_HASH_LEN]);
    if actual_network_id != expected_identity.network_id {
        return Err(WalCodecError::IdentityMismatch(WalIdentityField::NetworkId));
    }
    offset += SAFETY_WAL_HASH_LEN;
    let mut actual_context_id = [0_u8; SAFETY_WAL_HASH_LEN];
    actual_context_id.copy_from_slice(&bytes[offset..offset + SAFETY_WAL_HASH_LEN]);
    let actual_context_id = ContextId::new(actual_context_id);
    if actual_context_id != expected_identity.context_id {
        return Err(WalCodecError::IdentityMismatch(WalIdentityField::ContextId));
    }
    offset += SAFETY_WAL_HASH_LEN;
    let actual_height = read_wal_u64(&bytes[offset..offset + 8]);
    if actual_height != expected_identity.height {
        return Err(WalCodecError::IdentityMismatch(WalIdentityField::Height));
    }
    offset += 8;
    let mut actual_key = [0_u8; SAFETY_WAL_HASH_LEN];
    actual_key.copy_from_slice(&bytes[offset..offset + SAFETY_WAL_HASH_LEN]);
    if actual_key != expected_identity.consensus_key_hash {
        return Err(WalCodecError::IdentityMismatch(
            WalIdentityField::ConsensusKeyHash,
        ));
    }
    let expected = hasher.hash(&bytes[..SAFETY_WAL_FILE_HEADER_PREFIX_LEN]);
    let checksum_matches =
        bytes[SAFETY_WAL_FILE_HEADER_PREFIX_LEN..SAFETY_WAL_FILE_HEADER_LEN] == expected;
    if !checksum_matches {
        return Err(WalCodecError::InvalidHeader(WalHeaderCorruption::Checksum));
    }
    if !wal_header_accepted_body!(
        true,
        magic_matches,
        format_matches,
        actual_protocol,
        expected_identity.protocol_version,
        actual_network_id,
        expected_identity.network_id,
        actual_context_id,
        expected_identity.context_id,
        actual_height,
        expected_identity.height,
        actual_key,
        expected_identity.consensus_key_hash,
        checksum_matches,
    ) {
        return Err(WalCodecError::InvalidHeader(WalHeaderCorruption::Checksum));
    }
    Ok(())
}
const fn read_wal_u16(bytes: &[u8]) -> u16 {
    u16::from_le_bytes([bytes[0], bytes[1]])
}
const fn read_wal_u32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}
const fn read_wal_u64(bytes: &[u8]) -> u64 {
    u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ])
}
/// Ordered durable-write operation supplied by a filesystem adapter.
///
/// Implementations must perform the named operation on the same open append
/// handle. The core lifecycle calls them exactly as `write_all`, `flush`, then
/// `sync_data`, and mints no acknowledgement if any call fails.
pub trait WalAppendIo {
    /// Adapter-specific I/O failure.
    type Error;
    /// Append every byte or return an error after a possible partial write.
    ///
    /// # Errors
    ///
    /// Returns the adapter failure after zero or more bytes may have reached
    /// the append handle.
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), Self::Error>;
    /// Flush userspace buffers for the append handle.
    ///
    /// # Errors
    ///
    /// Returns the adapter failure. The complete frame may already be visible
    /// but is not acknowledged as durable.
    fn flush(&mut self) -> Result<(), Self::Error>;
    /// Synchronize appended file data to the trusted durable boundary.
    ///
    /// # Errors
    ///
    /// Returns the adapter failure without minting a durability receipt.
    fn sync_data(&mut self) -> Result<(), Self::Error>;
}
/// Physical I/O stage at which an append failed closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WalIoStage {
    /// Complete-frame `write_all`.
    Write,
    /// Userspace `flush`.
    Flush,
    /// Durable `sync_data`.
    SyncData,
}
/// Failure of the ordered append lifecycle.
#[derive(Debug)]
pub enum WalAppendError<E> {
    /// Framing failed before any I/O was attempted.
    Codec(WalCodecError),
    /// An I/O stage failed; the writer is poisoned until verified recovery.
    Io {
        /// Exact failed stage.
        stage: WalIoStage,
        /// Adapter-specific source error.
        source: E,
    },
    /// A previous I/O error requires reopen and verified WAL recovery.
    FailedClosed,
}
impl<E: fmt::Display> fmt::Display for WalAppendError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Codec(error) => error.fmt(formatter),
            Self::Io { stage, source } => {
                write!(formatter, "safety-WAL {stage:?} failed: {source}")
            }
            Self::FailedClosed => formatter.write_str(
                "safety-WAL writer is failed closed; reopen and verify recovery before appending",
            ),
        }
    }
}
impl<E: Error + 'static> Error for WalAppendError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Codec(error) => Some(error),
            Self::Io { source, .. } => Some(source),
            Self::FailedClosed => None,
        }
    }
}
/// Receipt minted only after write, flush, and durable synchronization succeed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WalAppendReceipt {
    sequence: u64,
    frame_hash: [u8; SAFETY_WAL_HASH_LEN],
}
impl WalAppendReceipt {
    /// Return the acknowledged physical frame sequence.
    #[must_use]
    pub const fn sequence(self) -> u64 {
        self.sequence
    }
    /// Return the acknowledged frame hash used by its successor.
    #[must_use]
    pub const fn frame_hash(self) -> [u8; SAFETY_WAL_HASH_LEN] {
        self.frame_hash
    }
}
/// Hash-chain state for ordered append acknowledgement.
///
/// Any I/O error poisons the state. Retrying against the same handle could
/// append after an unknown partial tail, so only a complete verified recovery
/// path may mint a replacement state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WalAppendState {
    next_sequence: u64,
    last_frame_hash: [u8; SAFETY_WAL_HASH_LEN],
    failed_closed: bool,
}
impl WalAppendState {
    /// Construct append state from a successfully verified recovery result.
    #[must_use]
    pub const fn from_recovery(recovery: &WalFileRecovery) -> Self {
        Self {
            next_sequence: recovery.next_sequence,
            last_frame_hash: recovery.last_frame_hash,
            failed_closed: false,
        }
    }
    /// Construct append state from a prefix verified by the production streaming reader.
    ///
    /// This crate-private constructor keeps the host recovery path from materializing the entire
    /// WAL merely to recover its final sequence and hash-chain link.
    pub(crate) const fn from_verified_stream_recovery(
        next_sequence: u64,
        last_frame_hash: [u8; SAFETY_WAL_HASH_LEN],
    ) -> Self {
        Self {
            next_sequence,
            last_frame_hash,
            failed_closed: false,
        }
    }
    /// Return the next required physical frame sequence.
    #[must_use]
    pub const fn next_sequence(self) -> u64 {
        self.next_sequence
    }
    /// Report whether an I/O failure requires verified reopen and replay.
    #[must_use]
    pub const fn is_failed_closed(self) -> bool {
        self.failed_closed
    }
    /// Encode, append, flush, and synchronize one frame in the mandatory order.
    ///
    /// State advances and a receipt is returned only after all three stages
    /// succeed. A failed stage preserves the pre-append sequence/hash and
    /// permanently closes this instance.
    ///
    /// # Errors
    ///
    /// Returns a codec error before I/O, the exact failed I/O stage, or
    /// `FailedClosed` after a previous I/O failure.
    pub fn append<H: WalFileHasher, I: WalAppendIo>(
        &mut self,
        payload: &[u8],
        hasher: &H,
        io: &mut I,
    ) -> Result<WalAppendReceipt, WalAppendError<I::Error>> {
        if self.failed_closed {
            return Err(WalAppendError::FailedClosed);
        }
        let frame = encode_wal_frame(self.next_sequence, self.last_frame_hash, payload, hasher)
            .map_err(WalAppendError::Codec)?;
        let next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(WalAppendError::Codec(WalCodecError::SequenceOverflow))?;
        if let Err(source) = io.write_all(frame.bytes()) {
            self.failed_closed = true;
            return Err(WalAppendError::Io {
                stage: WalIoStage::Write,
                source,
            });
        }
        let write_complete = true;
        if let Err(source) = io.flush() {
            self.failed_closed = true;
            return Err(WalAppendError::Io {
                stage: WalIoStage::Flush,
                source,
            });
        }
        let flush_complete = true;
        if let Err(source) = io.sync_data() {
            self.failed_closed = true;
            return Err(WalAppendError::Io {
                stage: WalIoStage::SyncData,
                source,
            });
        }
        let sync_complete = true;
        if !wal_append_acknowledged_body!(write_complete, flush_complete, sync_complete) {
            self.failed_closed = true;
            return Err(WalAppendError::FailedClosed);
        }
        self.next_sequence = next_sequence;
        self.last_frame_hash = frame.frame_hash();
        Ok(WalAppendReceipt {
            sequence: frame.sequence(),
            frame_hash: frame.frame_hash(),
        })
    }
}
/// Typed authorization to retire one closed height's WAL.
///
/// The sole constructor derives from evidence exposed only after the reducer has
/// applied its durable decision and verified the exact block-and-CommitQC Kura
/// receipt. The filesystem adapter must require this token before removing the
/// WAL and must synchronize the containing directory afterward.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WalRetirementAuthorization {
    context_id: ContextId,
    height: u64,
    subject: Subject,
    certificate: CertificateRef,
}
impl WalRetirementAuthorization {
    /// Derive retirement authority from a successfully closed reducer height.
    #[must_use]
    pub fn from_finalized_height(finalized: &FinalizedHeight) -> Self {
        Self {
            context_id: finalized.context().id(),
            height: finalized.context().height(),
            subject: finalized.decision().subject(),
            certificate: finalized.decision().reference(),
        }
    }
    /// Construct a self-consistent retirement token for filesystem adapter tests.
    #[cfg(test)]
    pub(crate) fn from_durable_decision_for_test(
        context_id: ContextId,
        height: u64,
        subject: Subject,
        certificate: CertificateRef,
    ) -> Option<Self> {
        let authorization = Self {
            context_id,
            height,
            subject,
            certificate,
        };
        authorization
            .matches_durable_decision(context_id, height, subject, certificate)
            .then_some(authorization)
    }
    /// Return the frozen height-context identity.
    #[must_use]
    pub const fn context_id(self) -> ContextId {
        self.context_id
    }
    /// Return the closed block height.
    #[must_use]
    pub const fn height(self) -> u64 {
        self.height
    }
    /// Return the exact finalized block subject.
    #[must_use]
    pub const fn subject(self) -> Subject {
        self.subject
    }
    /// Return the exact durable `CommitQC` reference.
    #[must_use]
    pub const fn certificate(self) -> CertificateRef {
        self.certificate
    }
    /// Return whether this durable-finality token owns the exact WAL target.
    ///
    /// The subject and certificate establish that the token denotes a
    /// self-consistent Commit decision. The physical WAL itself is height-local,
    /// so its immutable target is the exact `(context_id, height)` pair.
    #[must_use]
    pub fn authorizes_wal(self, identity: WalFileIdentity) -> bool {
        let internally_valid = self.matches_durable_decision(
            self.context_id,
            self.height,
            self.subject,
            self.certificate,
        );
        wal_retirement_target_matches_body!(
            internally_valid,
            self.context_id,
            self.height,
            identity.context_id,
            identity.height,
        )
    }
    /// Check this token against the exact finalized-height evidence.
    ///
    /// The three durability premises are true by construction of
    /// `FinalizedHeight`: it is available only after application and matching
    /// durable block-and-certificate receipt verification.
    #[must_use]
    pub fn matches_finalized_height(self, finalized: &FinalizedHeight) -> bool {
        let decision = finalized.decision();
        self.matches_durable_decision(
            finalized.context().id(),
            finalized.context().height(),
            decision.subject(),
            decision.reference(),
        )
    }
    /// Check this token against an exact durable decision identity.
    ///
    /// This exposes the same production/Verus predicate for negative adapter
    /// tests without exposing any constructor for retirement authority.
    #[must_use]
    pub fn matches_durable_decision(
        self,
        receipt_context: ContextId,
        receipt_height: u64,
        receipt_subject: Subject,
        receipt_certificate: CertificateRef,
    ) -> bool {
        wal_retirement_authorized_body!(
            true,
            true,
            true,
            self.context_id,
            self.height,
            self.subject,
            self.certificate.context_id(),
            self.certificate.round().height(),
            self.certificate.round().view(),
            self.certificate.proposal_round().height(),
            self.certificate.proposal_round().view(),
            self.certificate.phase(),
            Phase::Commit,
            self.certificate.subject(),
            receipt_context,
            receipt_height,
            receipt_subject,
            receipt_certificate.context_id(),
            receipt_certificate.round().height(),
            receipt_certificate.round().view(),
            receipt_certificate.proposal_round().height(),
            receipt_certificate.proposal_round().view(),
            receipt_certificate.phase(),
            receipt_certificate.subject(),
        )
    }
}
/// Monotonic identifier of a requested append to the safety WAL.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PersistenceId(u64);
impl PersistenceId {
    /// Constructs a persistence identifier.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
    /// Returns the numeric identifier.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}
/// Safety-relevant transition stored in the append-only WAL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WalRecord {
    /// Persist one locally validated proposal before signing it.
    ProposalIntent(Proposal),
    /// Persist a validated Prepare intent before signing the vote.
    PrepareIntent(Vote),
    /// Persist a newly observed highest `PrepareQC` before reporting it.
    ObservePrepare(QuorumCertificate),
    /// Atomically persist the lock and Commit intent before signing Commit.
    LockAndCommit {
        /// `PrepareQC` that establishes the new lock.
        prepare: QuorumCertificate,
        /// Commit vote authorized by that lock.
        vote: Vote,
    },
    /// Persist a timeout intent before signing it.
    TimeoutIntent(TimeoutVote),
    /// Persist a timeout certificate before entering its successor view.
    ///
    /// A carried `PrepareQC` may promote the durable lock. Its exact body can
    /// then be re-proposed unchanged in the successor view; only a new
    /// same-round PrepareQC may authorize a new [`Self::LockAndCommit`].
    InstallTimeout(TimeoutCertificate),
    /// Persist a `CommitQC` decision before applying the block.
    ///
    /// A validated first Decision supersedes any local Prepare lock. Locks
    /// constrain voting, while a quorum Commit certificate is finality
    /// authority; replay accepts only a later semantically identical Decision
    /// and rejects every non-decision transition after finality.
    Decision(QuorumCertificate),
}
impl WalRecord {
    pub(crate) fn context_id(&self) -> ContextId {
        match self {
            Self::ProposalIntent(proposal) => proposal.context_id(),
            Self::PrepareIntent(vote) | Self::LockAndCommit { vote, .. } => vote.context_id(),
            Self::ObservePrepare(certificate) | Self::Decision(certificate) => {
                certificate.reference().context_id()
            }
            Self::TimeoutIntent(vote) => vote.context_id(),
            Self::InstallTimeout(certificate) => certificate.context_id(),
        }
    }
}
/// One complete append-only WAL frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalEntry {
    id: PersistenceId,
    record: WalRecord,
}
impl WalEntry {
    /// Constructs a WAL entry.
    #[must_use]
    pub const fn new(id: PersistenceId, record: WalRecord) -> Self {
        Self { id, record }
    }
    /// Returns the monotonic entry identifier.
    #[must_use]
    pub const fn id(&self) -> PersistenceId {
        self.id
    }
    /// Returns the stored safety transition.
    #[must_use]
    pub const fn record(&self) -> &WalRecord {
        &self.record
    }
}
/// Consensus state reconstructed exclusively from acknowledged WAL entries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DurableState {
    context_id: ContextId,
    height: u64,
    current_view: u64,
    last_id: PersistenceId,
    proposal_intents: BTreeMap<Round, Proposal>,
    prepare_intents: BTreeMap<Round, Vote>,
    commit_intents: BTreeMap<Round, Vote>,
    timeout_intents: BTreeMap<Round, TimeoutVote>,
    highest_prepare: Option<QuorumCertificate>,
    locked: Option<QuorumCertificate>,
    last_timeout: Option<TimeoutCertificate>,
    decision: Option<QuorumCertificate>,
}
impl DurableState {
    /// Creates the initial durable state for a height.
    #[must_use]
    pub fn new(context: &HeightContext) -> Self {
        Self {
            context_id: context.id(),
            height: context.height(),
            current_view: 0,
            last_id: PersistenceId::default(),
            proposal_intents: BTreeMap::new(),
            prepare_intents: BTreeMap::new(),
            commit_intents: BTreeMap::new(),
            timeout_intents: BTreeMap::new(),
            highest_prepare: None,
            locked: None,
            last_timeout: None,
            decision: None,
        }
    }
    /// Replays complete WAL entries in order and returns the reconstructed state.
    ///
    /// # Errors
    ///
    /// Returns an error if any frame is missing, reordered, malformed, or
    /// violates a durable consensus invariant.
    pub fn replay(
        context: &HeightContext,
        local_validator: Option<ValidatorId>,
        entries: impl IntoIterator<Item = WalEntry>,
    ) -> Result<Self, ReplayError> {
        let mut state = Self::new(context);
        for entry in entries {
            state.apply(context, local_validator, &entry)?;
        }
        Ok(state)
    }
    /// Applies one complete WAL frame.
    ///
    /// # Errors
    ///
    /// Returns an error if the frame is out of sequence, targets another
    /// context, or violates a durable vote, lock, timeout, or decision rule.
    #[allow(clippy::too_many_lines)]
    pub fn apply(
        &mut self,
        context: &HeightContext,
        local_validator: Option<ValidatorId>,
        entry: &WalEntry,
    ) -> Result<(), ReplayError> {
        let mut next = self.clone();
        next.apply_in_place(context, local_validator, entry)?;
        *self = next;
        Ok(())
    }
    /// Classify one timeout certificate through the source-shared lock-only
    /// upgrade kernel.
    ///
    /// Exact installed-round equality is derived here once and consumed by
    /// live reducer admission, persistence acknowledgement, and WAL replay.
    pub(crate) fn is_strict_same_round_timeout_upgrade(
        &self,
        certificate: &TimeoutCertificate,
    ) -> bool {
        let selected = certificate.highest_prepare();
        refinement::strict_same_round_timeout_upgrade_is_allowed(
            StrictSameRoundTimeoutUpgradeProjection {
                current_view: self.current_view,
                timeout_view: certificate.round().view(),
                installed_same_round: self
                    .last_timeout
                    .as_ref()
                    .is_some_and(|installed| installed.round() == certificate.round()),
                selected_prepare_present: selected.is_some(),
                selected_prepare_view: selected.map_or(0, |candidate| candidate.round().view()),
                highest_prepare_present: self.highest_prepare.is_some(),
                highest_prepare_view: self
                    .highest_prepare
                    .as_ref()
                    .map_or(0, |highest| highest.round().view()),
                locked_prepare_present: self.locked.is_some(),
                locked_prepare_view: self
                    .locked
                    .as_ref()
                    .map_or(0, |locked| locked.round().view()),
            },
        )
    }
    /// Return whether `certificate` is the exact latest durable timeout
    /// justification for a locally generated proposal in `proposal_view`.
    ///
    /// The source-shared kernel derives exact full-certificate identity from
    /// concrete values. This method intentionally accepts no caller-computed
    /// safety or equality flag.
    pub(crate) fn is_exact_local_proposal_timeout_justification(
        &self,
        proposal_view: u64,
        certificate: &TimeoutCertificate,
    ) -> bool {
        refinement::local_proposal_timeout_justification_is_exact(
            self.context_id,
            self.height,
            self.current_view,
            proposal_view,
            certificate,
            self.last_timeout.as_ref(),
        )
    }
    #[allow(clippy::too_many_lines)]
    fn apply_in_place(
        &mut self,
        context: &HeightContext,
        local_validator: Option<ValidatorId>,
        entry: &WalEntry,
    ) -> Result<(), ReplayError> {
        let expected = self
            .last_id
            .0
            .checked_add(1)
            .ok_or(ReplayError::SequenceOverflow)?;
        if entry.id.0 != expected {
            return Err(ReplayError::NonContiguousSequence {
                expected: PersistenceId::new(expected),
                actual: entry.id,
            });
        }
        if entry.record.context_id() != self.context_id {
            return Err(ReplayError::ContextMismatch);
        }
        if self.decision.is_some() && !matches!(&entry.record, WalRecord::Decision(_)) {
            return Err(ReplayError::RecordAfterDecision);
        }
        match &entry.record {
            WalRecord::ProposalIntent(proposal) => {
                if Some(proposal.proposer()) != local_validator
                    || proposal.proposer() != context.leader(self.current_view)
                    || proposal.context_id() != self.context_id
                    || proposal.round() != Round::new(self.height, self.current_view)
                    || self.timeout_intents.contains_key(&proposal.round())
                {
                    return Err(ReplayError::InvalidProposalIntent);
                }
                let proposal_high = match proposal.justification() {
                    ProposalJustification::ParentCommit(parent)
                        if proposal.round().view() == 0
                            && match (*parent, context.parent_commit()) {
                                (None, None) => true,
                                (Some(carried), Some(frozen)) => {
                                    carried.proposal_round() == carried.round()
                                        && carried.round().height().checked_add(1)
                                            == Some(context.height())
                                        && carried.same_commit_decision(frozen)
                                }
                                (None, Some(_)) | (Some(_), None) => false,
                            } =>
                    {
                        None
                    }
                    ProposalJustification::Timeout(certificate)
                        if proposal.round().view() > 0
                            && certificate.validate(context).is_ok()
                            && self.is_exact_local_proposal_timeout_justification(
                                proposal.round().view(),
                                certificate,
                            )
                            && certificate.round().view().checked_add(1)
                                == Some(proposal.round().view())
                            && certificate.highest_prepare().is_none_or(|highest| {
                                highest.subject() == proposal.manifest().subject()
                            }) =>
                    {
                        certificate.highest_prepare()
                    }
                    _ => return Err(ReplayError::InvalidProposalIntent),
                };
                if let Some(locked) = &self.locked
                    && locked.subject() != proposal.manifest().subject()
                    && proposal_high.is_none_or(|highest| {
                        highest.phase() != Phase::Prepare
                            || highest.subject() != proposal.manifest().subject()
                            || highest.round().view() <= locked.round().view()
                    })
                {
                    return Err(ReplayError::InvalidProposalIntent);
                }
                if let Some(existing) = self.proposal_intents.get(&proposal.round()) {
                    if existing != proposal {
                        return Err(ReplayError::ConflictingProposalIntent(proposal.round()));
                    }
                } else {
                    self.proposal_intents
                        .insert(proposal.round(), proposal.clone());
                }
            }
            WalRecord::PrepareIntent(vote) => {
                Self::validate_local_vote(context, local_validator, *vote, Phase::Prepare)?;
                if vote.round().view() != self.current_view {
                    return Err(ReplayError::InvalidLocalVote);
                }
                if self.timeout_intents.contains_key(&vote.round()) {
                    return Err(ReplayError::ViewClosed(vote.round()));
                }
                insert_unique_vote(&mut self.prepare_intents, *vote)?;
            }
            WalRecord::ObservePrepare(certificate) => {
                validate_qc(context, certificate, Phase::Prepare)?;
                if certificate.round().view() > self.current_view {
                    return Err(ReplayError::InvalidCertificate);
                }
                update_highest(&mut self.highest_prepare, certificate.clone())?;
            }
            WalRecord::LockAndCommit { prepare, vote } => {
                validate_qc(context, prepare, Phase::Prepare)?;
                Self::validate_local_vote(context, local_validator, *vote, Phase::Commit)?;
                if vote.round().view() != self.current_view || vote.proposal_round() != vote.round()
                {
                    return Err(ReplayError::InvalidLocalVote);
                }
                if self.decision.is_some() {
                    return Err(ReplayError::InvalidLocalVote);
                }
                if vote.proposal_round() != prepare.proposal_round()
                    || prepare.proposal_round() != prepare.round()
                    || vote.subject() != prepare.subject()
                {
                    return Err(ReplayError::CommitDoesNotMatchPrepare);
                }
                if self.timeout_intents.contains_key(&vote.round()) {
                    return Err(ReplayError::ViewClosed(vote.round()));
                }
                if let Some(locked) = &self.locked
                    && (prepare.round().view() < locked.round().view()
                        || (prepare.round().view() == locked.round().view()
                            && prepare.subject() != locked.subject()))
                {
                    return Err(ReplayError::LockRegression);
                }
                insert_unique_vote(&mut self.commit_intents, *vote)?;
                update_highest(&mut self.highest_prepare, prepare.clone())?;
                self.locked = Some(prepare.clone());
            }
            WalRecord::TimeoutIntent(vote) => {
                if vote.context_id() != self.context_id
                    || vote.round().height() != self.height
                    || vote.round().view() != self.current_view
                    || Some(vote.signer()) != local_validator
                    || context.validator(&vote.signer()).is_none()
                {
                    return Err(ReplayError::InvalidLocalVote);
                }
                if vote.highest_prepare() != self.highest_prepare.as_ref() {
                    return Err(ReplayError::TimeoutHighQcMismatch);
                }
                if let Some(existing) = self.timeout_intents.get(&vote.round()) {
                    if existing != vote {
                        return Err(ReplayError::ConflictingVoteIntent(vote.round()));
                    }
                } else {
                    self.timeout_intents.insert(vote.round(), vote.clone());
                }
            }
            WalRecord::InstallTimeout(certificate) => {
                certificate
                    .validate(context)
                    .map_err(|_| ReplayError::InvalidCertificate)?;
                let selected = certificate.highest_prepare().cloned();
                // A second valid TC for the round which installed the current
                // view may reveal a PrepareQC omitted by the first quorum.
                // Admit only a strict origin-rank upgrade over every installed
                // Prepare witness. This changes the lock, not the lifecycle
                // view; equal, lower, and unprepared replacements fail closed.
                let strict_same_round_upgrade =
                    self.is_strict_same_round_timeout_upgrade(certificate);
                if certificate.round().view() < self.current_view && !strict_same_round_upgrade {
                    return Err(ReplayError::ViewRegression);
                }
                if let Some(highest) = &selected {
                    match &self.highest_prepare {
                        None => self.highest_prepare = Some(highest.clone()),
                        Some(existing) if highest.round().view() > existing.round().view() => {
                            self.highest_prepare = Some(highest.clone());
                        }
                        Some(existing)
                            if highest.round().view() == existing.round().view()
                                && highest.subject() != existing.subject() =>
                        {
                            return Err(ReplayError::ConflictingHighestPrepare);
                        }
                        Some(_) => {}
                    }
                    match &self.locked {
                        None => self.locked = Some(highest.clone()),
                        Some(locked) if highest.round().view() > locked.round().view() => {
                            self.locked = Some(highest.clone());
                        }
                        Some(locked)
                            if highest.round().view() == locked.round().view()
                                && highest.subject() != locked.subject() =>
                        {
                            return Err(ReplayError::LockRegression);
                        }
                        Some(_) => {}
                    }
                }
                // Installing a TC never lowers or clears a lock. A different
                // subject is safe only when the TC carries a strictly higher
                // PrepareQC; timeout votes transport that full certificate so
                // an omitted local lock becomes known to the next TC quorum.
                if !strict_same_round_upgrade {
                    self.current_view = certificate
                        .round()
                        .view()
                        .checked_add(1)
                        .ok_or(ReplayError::ViewOverflow)?;
                }
                self.last_timeout = Some(certificate.clone());
            }
            WalRecord::Decision(certificate) => {
                validate_qc(context, certificate, Phase::Commit)?;
                if let Some(existing) = &self.decision {
                    if !existing
                        .reference()
                        .same_commit_decision(certificate.reference())
                    {
                        return Err(ReplayError::ConflictingDecision);
                    }
                } else {
                    self.decision = Some(certificate.clone());
                }
            }
        }
        self.last_id = entry.id;
        Ok(())
    }
    fn validate_local_vote(
        context: &HeightContext,
        local_validator: Option<ValidatorId>,
        vote: Vote,
        phase: Phase,
    ) -> Result<(), ReplayError> {
        if vote.context_id() != context.id()
            || vote.round().height() != context.height()
            || vote.proposal_round().height() != context.height()
            || vote.proposal_round() != vote.round()
            || vote.phase() != phase
            || Some(vote.signer()) != local_validator
            || context.validator(&vote.signer()).is_none()
        {
            return Err(ReplayError::InvalidLocalVote);
        }
        Ok(())
    }
    /// Returns the height context identifier.
    #[must_use]
    pub const fn context_id(&self) -> ContextId {
        self.context_id
    }
    /// Returns the height represented by this state.
    #[must_use]
    pub const fn height(&self) -> u64 {
        self.height
    }
    /// Returns the current persisted view.
    #[must_use]
    pub const fn current_view(&self) -> u64 {
        self.current_view
    }
    /// Returns the last applied WAL identifier.
    #[must_use]
    pub const fn last_id(&self) -> PersistenceId {
        self.last_id
    }
    /// Returns the next required WAL identifier.
    ///
    /// # Errors
    ///
    /// Returns an error if the monotonic identifier is exhausted.
    pub fn next_id(&self) -> Result<PersistenceId, ReplayError> {
        self.last_id
            .0
            .checked_add(1)
            .map(PersistenceId::new)
            .ok_or(ReplayError::SequenceOverflow)
    }
    /// Returns the highest durable `PrepareQC`.
    #[must_use]
    pub const fn highest_prepare(&self) -> Option<&QuorumCertificate> {
        self.highest_prepare.as_ref()
    }
    /// Returns the current durable lock.
    #[must_use]
    pub const fn locked(&self) -> Option<&QuorumCertificate> {
        self.locked.as_ref()
    }
    /// Returns the last installed timeout certificate.
    #[must_use]
    pub const fn last_timeout(&self) -> Option<&TimeoutCertificate> {
        self.last_timeout.as_ref()
    }
    /// Returns the durable decision, if any.
    #[must_use]
    pub const fn decision(&self) -> Option<&QuorumCertificate> {
        self.decision.as_ref()
    }
    /// Returns the local Prepare intent for a round.
    #[must_use]
    pub fn prepare_intent(&self, round: Round) -> Option<Vote> {
        self.prepare_intents.get(&round).copied()
    }
    /// Returns the local proposal intent for a round.
    #[must_use]
    pub fn proposal_intent(&self, round: Round) -> Option<&Proposal> {
        self.proposal_intents.get(&round)
    }
    /// Returns the local Commit intent for a round.
    #[must_use]
    pub fn commit_intent(&self, round: Round) -> Option<Vote> {
        self.commit_intents.get(&round).copied()
    }
    pub(crate) fn prepare_intents(&self) -> impl Iterator<Item = Vote> + '_ {
        self.prepare_intents.values().copied()
    }
    pub(crate) fn commit_intents(&self) -> impl Iterator<Item = Vote> + '_ {
        self.commit_intents.values().copied()
    }
    /// Return the sole same-round Commit intent authorized by `locked`.
    ///
    /// A timeout may leave this already-durable intent retransmittable, but it
    /// cannot manufacture a new finality round for the old same-round vote.
    /// Progress without the old quorum requires an unchanged body re-proposal
    /// and a new PrepareQC, which becomes a distinct lock.
    pub(crate) fn commit_intent_for_lock(&self, locked: &QuorumCertificate) -> Option<Vote> {
        let round = locked.round();
        self.commit_intent(round).filter(|vote| {
            vote.phase() == Phase::Commit
                && vote.round() == round
                && vote.proposal_round() == round
                && vote.subject() == locked.subject()
        })
    }
    /// Returns the local timeout intent for a round.
    #[must_use]
    pub fn timeout_intent(&self, round: Round) -> Option<TimeoutVote> {
        self.timeout_intents.get(&round).cloned()
    }
}
fn validate_qc(
    context: &HeightContext,
    certificate: &QuorumCertificate,
    expected_phase: Phase,
) -> Result<(), ReplayError> {
    if certificate.phase() != expected_phase {
        return Err(ReplayError::InvalidCertificate);
    }
    certificate
        .validate(context)
        .map(|_| ())
        .map_err(|_| ReplayError::InvalidCertificate)
}
fn insert_unique_vote(intents: &mut BTreeMap<Round, Vote>, vote: Vote) -> Result<(), ReplayError> {
    if let Some(existing) = intents.get(&vote.round()) {
        if existing != &vote {
            return Err(ReplayError::ConflictingVoteIntent(vote.round()));
        }
    } else {
        intents.insert(vote.round(), vote);
    }
    Ok(())
}
fn update_highest(
    highest: &mut Option<QuorumCertificate>,
    candidate: QuorumCertificate,
) -> Result<(), ReplayError> {
    if let Some(existing) = highest {
        if candidate.round().view() < existing.round().view() {
            return Err(ReplayError::HighestQcRegression);
        }
        if candidate.round().view() == existing.round().view()
            && candidate.subject() != existing.subject()
        {
            return Err(ReplayError::ConflictingHighestPrepare);
        }
        if candidate.round().view() == existing.round().view() {
            return Ok(());
        }
    }
    *highest = Some(candidate);
    Ok(())
}
/// Failure while applying or replaying complete WAL records.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplayError {
    /// WAL identifiers cannot be incremented further.
    SequenceOverflow,
    /// A WAL frame was missing, duplicated, or reordered.
    NonContiguousSequence {
        /// Required next identifier.
        expected: PersistenceId,
        /// Identifier found in the WAL.
        actual: PersistenceId,
    },
    /// A WAL record belongs to another height context.
    ContextMismatch,
    /// A local vote record has the wrong signer, context, height, or phase.
    InvalidLocalVote,
    /// A local proposal has an invalid leader, round, justification, or lock.
    InvalidProposalIntent,
    /// A second local proposal conflicts with an already durable proposal.
    ConflictingProposalIntent(Round),
    /// A certificate failed structural, membership, or quorum validation.
    InvalidCertificate,
    /// A second local intent conflicts with an already durable vote.
    ConflictingVoteIntent(Round),
    /// A Commit intent does not match its authorizing `PrepareQC`.
    CommitDoesNotMatchPrepare,
    /// A replayed lock moves backwards or conflicts at the same view.
    LockRegression,
    /// A highest `PrepareQC` moves backwards.
    HighestQcRegression,
    /// Equal-view highest `PrepareQC`s certify different subjects.
    ConflictingHighestPrepare,
    /// A timeout does not report the highest durable `PrepareQC`.
    TimeoutHighQcMismatch,
    /// A Prepare or Commit intent was appended after the view was durably closed.
    ViewClosed(Round),
    /// A timeout certificate would move the persisted view backwards.
    ViewRegression,
    /// The successor view cannot be represented.
    ViewOverflow,
    /// Two durable `CommitQC`s decide different subjects.
    ConflictingDecision,
    /// A non-decision record appeared after durable finality.
    RecordAfterDecision,
}
impl fmt::Display for ReplayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SequenceOverflow => formatter.write_str("WAL sequence overflow"),
            Self::NonContiguousSequence { expected, actual } => write!(
                formatter,
                "non-contiguous WAL sequence: expected {}, got {}",
                expected.get(),
                actual.get()
            ),
            Self::ContextMismatch => formatter.write_str("WAL context mismatch"),
            Self::InvalidLocalVote => formatter.write_str("invalid local vote in WAL"),
            Self::InvalidProposalIntent => formatter.write_str("invalid local proposal in WAL"),
            Self::ConflictingProposalIntent(round) => write!(
                formatter,
                "conflicting proposal intent at height {}, view {}",
                round.height(),
                round.view()
            ),
            Self::InvalidCertificate => formatter.write_str("invalid certificate in WAL"),
            Self::ConflictingVoteIntent(round) => write!(
                formatter,
                "conflicting vote intent at height {}, view {}",
                round.height(),
                round.view()
            ),
            Self::CommitDoesNotMatchPrepare => {
                formatter.write_str("Commit intent does not match PrepareQC")
            }
            Self::LockRegression => formatter.write_str("durable lock regression"),
            Self::HighestQcRegression => formatter.write_str("highest PrepareQC regression"),
            Self::ConflictingHighestPrepare => {
                formatter.write_str("conflicting highest PrepareQCs")
            }
            Self::TimeoutHighQcMismatch => formatter.write_str("timeout high-QC mismatch"),
            Self::ViewClosed(round) => write!(
                formatter,
                "height {}, view {} is durably closed",
                round.height(),
                round.view()
            ),
            Self::ViewRegression => formatter.write_str("durable view regression"),
            Self::ViewOverflow => formatter.write_str("view overflow"),
            Self::ConflictingDecision => formatter.write_str("conflicting durable decisions"),
            Self::RecordAfterDecision => {
                formatter.write_str("non-decision WAL record after durable finality")
            }
        }
    }
}
impl Error for ReplayError {}
#[cfg(test)]
mod byte_lifecycle_tests {
    use super::super::{
        Digest, NetworkId, OpaqueSignature, SignatureShare, TimeoutSignatureGroup, Validator,
        VotingMode, VotingPower,
    };
    use super::*;
    const IDENTITY: WalFileIdentity =
        WalFileIdentity::new(3, [0x11; 32], ContextId::repeat(0x33), 9, [0x22; 32]);
    fn replay_context() -> HeightContext {
        let roster = (1_u8..=4)
            .map(|byte| Validator::new(ValidatorId::repeat(byte), VotingPower::new(1)))
            .collect();
        HeightContext::new(
            ContextId::repeat(0x50),
            NetworkId::repeat(0x51),
            2,
            Some(CertificateRef::new(
                ContextId::repeat(0x40),
                Round::new(1, 0),
                Phase::Commit,
                Subject::repeat(0x41),
            )),
            7,
            roster,
            VotingMode::Permissioned,
            Digest::repeat(0x52),
            Digest::repeat(0x55),
            Digest::repeat(0x53),
            Digest::repeat(0x54),
        )
        .expect("valid WAL replay context")
    }
    fn replay_shares() -> Vec<SignatureShare> {
        (1_u8..=3)
            .map(|byte| {
                SignatureShare::new(
                    ValidatorId::repeat(byte),
                    OpaqueSignature::new(vec![byte; 8]),
                )
            })
            .collect()
    }
    fn replay_prepare(context: &HeightContext, view: u64, subject: u8) -> QuorumCertificate {
        QuorumCertificate::new(
            CertificateRef::new(
                context.id(),
                Round::new(context.height(), view),
                Phase::Prepare,
                Subject::repeat(subject),
            ),
            replay_shares(),
        )
    }
    fn replay_timeout(
        context: &HeightContext,
        view: u64,
        highest: Option<QuorumCertificate>,
    ) -> TimeoutCertificate {
        TimeoutCertificate::new(
            context.id(),
            Round::new(context.height(), view),
            vec![TimeoutSignatureGroup::new(highest, replay_shares())],
        )
    }
    #[test]
    fn same_round_timeout_replay_accepts_only_a_strict_prepare_origin_upgrade() {
        let context = replay_context();
        let prepare_zero = replay_prepare(&context, 0, 0x60);
        let prepare_one = replay_prepare(&context, 1, 0x61);
        let first = replay_timeout(&context, 0, None);
        let second = replay_timeout(&context, 1, Some(prepare_zero.clone()));
        let upgrade = replay_timeout(&context, 1, Some(prepare_one.clone()));
        let mut durable = DurableState::replay(
            &context,
            None,
            [
                WalEntry::new(PersistenceId::new(1), WalRecord::InstallTimeout(first)),
                WalEntry::new(PersistenceId::new(2), WalRecord::InstallTimeout(second)),
                WalEntry::new(
                    PersistenceId::new(3),
                    WalRecord::InstallTimeout(upgrade.clone()),
                ),
            ],
        )
        .expect("strict same-round Prepare origin upgrade replays");
        assert_eq!(durable.current_view(), 2);
        assert_eq!(durable.highest_prepare(), Some(&prepare_one));
        assert_eq!(durable.locked(), Some(&prepare_one));
        assert_eq!(durable.last_timeout(), Some(&upgrade));
        for rejected in [
            replay_timeout(&context, 1, None),
            replay_timeout(&context, 1, Some(prepare_zero)),
            replay_timeout(&context, 1, Some(prepare_one)),
        ] {
            assert_eq!(
                durable.apply(
                    &context,
                    None,
                    &WalEntry::new(PersistenceId::new(4), WalRecord::InstallTimeout(rejected),),
                ),
                Err(ReplayError::ViewRegression)
            );
            assert_eq!(durable.current_view(), 2, "rejection must be atomic");
            assert_eq!(durable.last_timeout(), Some(&upgrade));
        }
    }
    fn test_hash(bytes: &[u8]) -> [u8; SAFETY_WAL_HASH_LEN] {
        let mut lanes = [
            0xcbf2_9ce4_8422_2325_u64,
            0x9e37_79b9_7f4a_7c15,
            0x6a09_e667_f3bc_c909,
            0xbb67_ae85_84ca_a73b,
        ];
        for (index, byte) in bytes.iter().copied().enumerate() {
            let lane = index % lanes.len();
            lanes[lane] ^= u64::from(byte) | ((index as u64) << 8);
            lanes[lane] = lanes[lane]
                .wrapping_mul(0x0000_0100_0000_01b3)
                .rotate_left(u32::try_from((index % 63) + 1).expect("bounded rotation"));
        }
        for (index, lane) in lanes.iter_mut().enumerate() {
            *lane ^= u64::try_from(bytes.len()).expect("fixture length")
                << u32::try_from(index * 7).expect("bounded shift");
        }
        let mut digest = [0_u8; SAFETY_WAL_HASH_LEN];
        for (index, lane) in lanes.into_iter().enumerate() {
            digest[index * 8..(index + 1) * 8].copy_from_slice(&lane.to_le_bytes());
        }
        digest
    }
    fn header() -> Vec<u8> {
        encode_wal_file_header(IDENTITY, &test_hash).to_vec()
    }
    fn file_with_frames(payloads: &[&[u8]]) -> Vec<u8> {
        let mut bytes = header();
        let mut previous_hash = [0_u8; SAFETY_WAL_HASH_LEN];
        for (index, payload) in payloads.iter().enumerate() {
            let frame = encode_wal_frame(
                u64::try_from(index).expect("fixture sequence"),
                previous_hash,
                payload,
                &test_hash,
            )
            .expect("encode fixture frame");
            previous_hash = frame.frame_hash();
            bytes.extend_from_slice(frame.bytes());
        }
        bytes
    }
    #[test]
    fn canonical_header_and_hash_chain_round_trip() {
        let bytes = file_with_frames(&[b"prepare", b"lock-and-commit", b"decision"]);
        let recovered = recover_wal_file(&bytes, IDENTITY, &test_hash).expect("recover WAL");
        assert_eq!(recovered.valid_prefix_len(), bytes.len());
        assert!(!recovered.has_incomplete_tail());
        assert_eq!(recovered.next_sequence(), 3);
        assert_eq!(
            recovered
                .records()
                .iter()
                .map(|record| (record.sequence(), record.payload()))
                .collect::<Vec<_>>(),
            vec![
                (0, b"prepare".as_slice()),
                (1, b"lock-and-commit".as_slice()),
                (2, b"decision".as_slice()),
            ]
        );
        assert_eq!(
            recovered
                .records()
                .last()
                .expect("three complete fixture frames")
                .frame_hash(),
            recovered.last_frame_hash()
        );
        assert_eq!(&bytes[..SAFETY_WAL_FILE_MAGIC.len()], b"SUMV2WAL");
        assert_eq!(
            &bytes[SAFETY_WAL_FILE_HEADER_LEN
                ..SAFETY_WAL_FILE_HEADER_LEN + SAFETY_WAL_FRAME_MAGIC.len()],
            b"S2FR"
        );
    }
    #[test]
    fn every_incomplete_final_frame_boundary_is_unacknowledged() {
        let header = header();
        let first = encode_wal_frame(0, [0; 32], b"durable", &test_hash).expect("first frame");
        for cut in 0..first.bytes().len() {
            let mut crashed = header.clone();
            crashed.extend_from_slice(&first.bytes()[..cut]);
            let recovered = recover_wal_file(&crashed, IDENTITY, &test_hash)
                .unwrap_or_else(|error| panic!("cut {cut} must recover: {error}"));
            assert!(recovered.records().is_empty(), "cut {cut}");
            assert_eq!(recovered.valid_prefix_len(), header.len(), "cut {cut}");
            assert_eq!(recovered.has_incomplete_tail(), cut != 0, "cut {cut}");
            assert_eq!(recovered.next_sequence(), 0, "cut {cut}");
        }
        let mut complete = header.clone();
        complete.extend_from_slice(first.bytes());
        let recovered = recover_wal_file(&complete, IDENTITY, &test_hash).expect("complete frame");
        assert_eq!(recovered.records().len(), 1);
        assert!(!recovered.has_incomplete_tail());
        let second =
            encode_wal_frame(1, first.frame_hash(), b"next", &test_hash).expect("second frame");
        for cut in 0..second.bytes().len() {
            let mut crashed = complete.clone();
            crashed.extend_from_slice(&second.bytes()[..cut]);
            let recovered = recover_wal_file(&crashed, IDENTITY, &test_hash)
                .unwrap_or_else(|error| panic!("second cut {cut} must recover: {error}"));
            assert_eq!(recovered.records().len(), 1, "cut {cut}");
            assert_eq!(recovered.valid_prefix_len(), complete.len(), "cut {cut}");
            assert_eq!(recovered.has_incomplete_tail(), cut != 0, "cut {cut}");
            assert_eq!(recovered.next_sequence(), 1, "cut {cut}");
            assert_eq!(recovered.last_frame_hash(), first.frame_hash(), "cut {cut}");
        }
    }
    #[test]
    fn complete_corruption_before_an_incomplete_tail_fails_closed() {
        let first_payload = b"durable decision";
        let mut bytes = file_with_frames(&[first_payload]);
        let first_payload_offset = SAFETY_WAL_FILE_HEADER_LEN + SAFETY_WAL_FRAME_HEADER_LEN;
        bytes[first_payload_offset] ^= 0x80;
        bytes.extend_from_slice(b"S2FR\x01\x00");
        assert_eq!(
            recover_wal_file(&bytes, IDENTITY, &test_hash),
            Err(WalCodecError::CorruptFrame {
                sequence: 0,
                reason: WalFrameCorruption::Checksum,
            })
        );
    }
    #[test]
    fn complete_final_frame_corruption_is_not_downgraded_to_a_crash_tail() {
        let mut magic = file_with_frames(&[b"prepare"]);
        magic[SAFETY_WAL_FILE_HEADER_LEN] ^= 0x01;
        assert!(matches!(
            recover_wal_file(&magic, IDENTITY, &test_hash),
            Err(WalCodecError::CorruptFrame {
                sequence: 0,
                reason: WalFrameCorruption::Magic,
            })
        ));
        let mut oversized = file_with_frames(&[b"prepare"]);
        let payload_len_offset =
            SAFETY_WAL_FILE_HEADER_LEN + SAFETY_WAL_FRAME_MAGIC.len() + std::mem::size_of::<u64>();
        let declared =
            u32::try_from(SAFETY_WAL_MAX_RECORD_BYTES + 1).expect("record limit fits u32");
        oversized[payload_len_offset..payload_len_offset + 4]
            .copy_from_slice(&declared.to_le_bytes());
        assert!(matches!(
            recover_wal_file(&oversized, IDENTITY, &test_hash),
            Err(WalCodecError::CorruptFrame {
                sequence: 0,
                reason: WalFrameCorruption::RecordLength,
            })
        ));
        let mut checksum = file_with_frames(&[b"prepare"]);
        *checksum.last_mut().expect("frame checksum byte") ^= 0x01;
        assert!(matches!(
            recover_wal_file(&checksum, IDENTITY, &test_hash),
            Err(WalCodecError::CorruptFrame {
                sequence: 0,
                reason: WalFrameCorruption::Checksum,
            })
        ));
        let first = encode_wal_frame(0, [0; 32], b"prepare", &test_hash).expect("first");
        let second =
            encode_wal_frame(1, first.frame_hash(), b"decision", &test_hash).expect("second");
        let mut chain = header();
        chain.extend_from_slice(first.bytes());
        chain.extend_from_slice(second.bytes());
        let second_start = SAFETY_WAL_FILE_HEADER_LEN + first.bytes().len();
        let previous_hash_offset = second_start + SAFETY_WAL_FRAME_MAGIC.len() + 8 + 4;
        chain[previous_hash_offset] ^= 0x01;
        assert!(matches!(
            recover_wal_file(&chain, IDENTITY, &test_hash),
            Err(WalCodecError::CorruptFrame {
                sequence: 1,
                reason: WalFrameCorruption::PreviousHash,
            })
        ));
        let mut sequence = file_with_frames(&[b"first", b"second"]);
        let first_len = encode_wal_frame(0, [0; 32], b"first", &test_hash)
            .expect("first")
            .bytes()
            .len();
        let second_sequence = SAFETY_WAL_FILE_HEADER_LEN + first_len + SAFETY_WAL_FRAME_MAGIC.len();
        sequence[second_sequence..second_sequence + 8].copy_from_slice(&9_u64.to_le_bytes());
        assert!(matches!(
            recover_wal_file(&sequence, IDENTITY, &test_hash),
            Err(WalCodecError::CorruptFrame {
                sequence: 1,
                reason: WalFrameCorruption::Sequence,
            })
        ));
    }
    #[test]
    fn header_and_consensus_identity_fail_closed() {
        let valid = header();
        for cut in 0..SAFETY_WAL_FILE_HEADER_LEN {
            assert_eq!(
                recover_wal_file(&valid[..cut], IDENTITY, &test_hash),
                Err(WalCodecError::InvalidHeader(WalHeaderCorruption::Truncated)),
                "header cut {cut}"
            );
        }
        let mut magic = valid.clone();
        magic[0] ^= 0x01;
        assert_eq!(
            recover_wal_file(&magic, IDENTITY, &test_hash),
            Err(WalCodecError::InvalidHeader(WalHeaderCorruption::Magic))
        );
        let mut format = valid.clone();
        format[SAFETY_WAL_FILE_MAGIC.len()] ^= 0x01;
        assert_eq!(
            recover_wal_file(&format, IDENTITY, &test_hash),
            Err(WalCodecError::InvalidHeader(
                WalHeaderCorruption::FormatVersion
            ))
        );
        let mut checksum = valid.clone();
        *checksum.last_mut().expect("header checksum byte") ^= 0x01;
        assert_eq!(
            recover_wal_file(&checksum, IDENTITY, &test_hash),
            Err(WalCodecError::InvalidHeader(WalHeaderCorruption::Checksum))
        );
        let different_protocol_version = IDENTITY.protocol_version() ^ 1;
        for (identity, expected) in [
            (
                WalFileIdentity::new(
                    different_protocol_version,
                    IDENTITY.network_id(),
                    IDENTITY.context_id(),
                    IDENTITY.height(),
                    IDENTITY.consensus_key_hash(),
                ),
                WalIdentityField::ProtocolVersion,
            ),
            (
                WalFileIdentity::new(
                    IDENTITY.protocol_version(),
                    [0x44; 32],
                    IDENTITY.context_id(),
                    IDENTITY.height(),
                    IDENTITY.consensus_key_hash(),
                ),
                WalIdentityField::NetworkId,
            ),
            (
                WalFileIdentity::new(
                    IDENTITY.protocol_version(),
                    IDENTITY.network_id(),
                    ContextId::repeat(0x55),
                    IDENTITY.height(),
                    IDENTITY.consensus_key_hash(),
                ),
                WalIdentityField::ContextId,
            ),
            (
                WalFileIdentity::new(
                    IDENTITY.protocol_version(),
                    IDENTITY.network_id(),
                    IDENTITY.context_id(),
                    IDENTITY.height() + 1,
                    IDENTITY.consensus_key_hash(),
                ),
                WalIdentityField::Height,
            ),
            (
                WalFileIdentity::new(
                    IDENTITY.protocol_version(),
                    IDENTITY.network_id(),
                    IDENTITY.context_id(),
                    IDENTITY.height(),
                    [0x55; 32],
                ),
                WalIdentityField::ConsensusKeyHash,
            ),
        ] {
            assert_eq!(
                recover_wal_file(&valid, identity, &test_hash),
                Err(WalCodecError::IdentityMismatch(expected))
            );
        }
    }
    #[test]
    fn retirement_authorization_targets_one_exact_wal_identity() {
        let subject = Subject::repeat(0x66);
        let certificate = CertificateRef::new(
            IDENTITY.context_id(),
            Round::new(IDENTITY.height(), 2),
            Phase::Commit,
            subject,
        );
        let authorization = WalRetirementAuthorization {
            context_id: IDENTITY.context_id(),
            height: IDENTITY.height(),
            subject,
            certificate,
        };
        assert!(authorization.authorizes_wal(IDENTITY));
        assert!(!authorization.authorizes_wal(WalFileIdentity::new(
            IDENTITY.protocol_version(),
            IDENTITY.network_id(),
            ContextId::repeat(0x77),
            IDENTITY.height(),
            IDENTITY.consensus_key_hash(),
        )));
        assert!(!authorization.authorizes_wal(WalFileIdentity::new(
            IDENTITY.protocol_version(),
            IDENTITY.network_id(),
            IDENTITY.context_id(),
            IDENTITY.height() + 1,
            IDENTITY.consensus_key_hash(),
        )));
        let invalid_phase = WalRetirementAuthorization {
            certificate: CertificateRef::new(
                IDENTITY.context_id(),
                Round::new(IDENTITY.height(), 2),
                Phase::Prepare,
                subject,
            ),
            ..authorization
        };
        assert!(!invalid_phase.authorizes_wal(IDENTITY));
    }
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum FakeIoError {
        Failed(WalIoStage),
    }
    impl fmt::Display for FakeIoError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(formatter, "injected {self:?}")
        }
    }
    impl Error for FakeIoError {}
    #[derive(Debug, Default)]
    struct FakeIo {
        bytes: Vec<u8>,
        calls: Vec<WalIoStage>,
        fail_at: Option<WalIoStage>,
    }
    impl WalAppendIo for FakeIo {
        type Error = FakeIoError;
        fn write_all(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
            self.calls.push(WalIoStage::Write);
            if self.fail_at == Some(WalIoStage::Write) {
                self.bytes.extend_from_slice(&bytes[..bytes.len() / 2]);
                return Err(FakeIoError::Failed(WalIoStage::Write));
            }
            self.bytes.extend_from_slice(bytes);
            Ok(())
        }
        fn flush(&mut self) -> Result<(), Self::Error> {
            self.calls.push(WalIoStage::Flush);
            if self.fail_at == Some(WalIoStage::Flush) {
                return Err(FakeIoError::Failed(WalIoStage::Flush));
            }
            Ok(())
        }
        fn sync_data(&mut self) -> Result<(), Self::Error> {
            self.calls.push(WalIoStage::SyncData);
            if self.fail_at == Some(WalIoStage::SyncData) {
                return Err(FakeIoError::Failed(WalIoStage::SyncData));
            }
            Ok(())
        }
    }
    #[test]
    fn append_acknowledgement_requires_write_flush_and_sync_in_order() {
        let initial_bytes = header();
        let recovery = recover_wal_file(&initial_bytes, IDENTITY, &test_hash).expect("empty WAL");
        let mut state = WalAppendState::from_recovery(&recovery);
        let mut io = FakeIo::default();
        let receipt = state
            .append(b"prepare intent", &test_hash, &mut io)
            .expect("durable append");
        assert_eq!(
            io.calls,
            [WalIoStage::Write, WalIoStage::Flush, WalIoStage::SyncData]
        );
        assert_eq!(receipt.sequence(), 0);
        assert_eq!(state.next_sequence(), 1);
        assert_eq!(receipt.frame_hash(), state.last_frame_hash);
        assert!(!state.is_failed_closed());
        let mut persisted = initial_bytes;
        persisted.extend_from_slice(&io.bytes);
        let replayed = recover_wal_file(&persisted, IDENTITY, &test_hash).expect("replay append");
        assert_eq!(replayed.records()[0].payload(), b"prepare intent");
        assert_eq!(replayed.records()[0].frame_hash(), receipt.frame_hash());
        assert_eq!(replayed.last_frame_hash(), receipt.frame_hash());
    }
    #[test]
    fn every_io_failure_preserves_append_state_and_requires_recovery() {
        for failed_stage in [WalIoStage::Write, WalIoStage::Flush, WalIoStage::SyncData] {
            let initial_bytes = header();
            let recovery =
                recover_wal_file(&initial_bytes, IDENTITY, &test_hash).expect("empty WAL");
            let mut state = WalAppendState::from_recovery(&recovery);
            let before = state;
            let mut io = FakeIo {
                fail_at: Some(failed_stage),
                ..FakeIo::default()
            };
            assert!(matches!(
                state.append(b"decision", &test_hash, &mut io),
                Err(WalAppendError::Io { stage, .. }) if stage == failed_stage
            ));
            assert_eq!(state.next_sequence(), before.next_sequence());
            assert_eq!(state.last_frame_hash, before.last_frame_hash);
            assert!(state.is_failed_closed());
            let calls_after_failure = io.calls.len();
            assert!(matches!(
                state.append(b"must not retry", &test_hash, &mut io),
                Err(WalAppendError::FailedClosed)
            ));
            assert_eq!(io.calls.len(), calls_after_failure);
            let mut on_disk = initial_bytes;
            on_disk.extend_from_slice(&io.bytes);
            let reopened = recover_wal_file(&on_disk, IDENTITY, &test_hash)
                .unwrap_or_else(|error| panic!("{failed_stage:?} recovery: {error}"));
            match failed_stage {
                WalIoStage::Write => {
                    assert!(reopened.records().is_empty());
                    assert!(reopened.has_incomplete_tail());
                }
                WalIoStage::Flush | WalIoStage::SyncData => {
                    // A complete but unacknowledged frame is conservatively
                    // replayed after its checksum and chain are verified.
                    assert_eq!(reopened.records().len(), 1);
                    assert!(!reopened.has_incomplete_tail());
                }
            }
            let reopened_state = WalAppendState::from_recovery(&reopened);
            assert!(!reopened_state.is_failed_closed());
            assert_eq!(
                reopened_state.next_sequence(),
                reopened.records().len() as u64
            );
        }
    }
    #[test]
    fn codec_preflight_errors_do_not_write_or_mint_receipts() {
        let recovery = recover_wal_file(&header(), IDENTITY, &test_hash).expect("empty WAL");
        let mut overflow = WalAppendState {
            next_sequence: u64::MAX,
            ..WalAppendState::from_recovery(&recovery)
        };
        let mut io = FakeIo::default();
        assert!(matches!(
            overflow.append(b"overflow", &test_hash, &mut io),
            Err(WalAppendError::Codec(WalCodecError::SequenceOverflow))
        ));
        assert!(io.calls.is_empty());
        assert!(!overflow.is_failed_closed());
        let oversized = vec![0_u8; SAFETY_WAL_MAX_RECORD_BYTES + 1];
        let mut state = WalAppendState::from_recovery(&recovery);
        assert!(matches!(
            state.append(&oversized, &test_hash, &mut io),
            Err(WalAppendError::Codec(WalCodecError::RecordTooLarge { .. }))
        ));
        assert!(io.calls.is_empty());
        assert_eq!(state.next_sequence(), 0);
    }
}
