//! TRON light client (spec `specs/sccp.md` §4.13.3).
//!
//! TRON witnesses sign only their own `BlockHeader.raw_data`; there is no quorum signature. A
//! block is final (*solid*) once enough of the active witnesses have built on it, so every piece
//! of evidence is a self-authenticating header segment.
//!
//! **Stored set:** the active witness set of maintenance period `p` (set id `p`): up to 27
//! witness accounts, each with the address its block signatures recover to (the witness
//! permission key). Period `p` starts at `T_p = origin + p · interval` of the compiled profile;
//! a header belongs to the period of its timestamp. A set is superseded at the start of the next
//! period and is fresh until `ws_bound_ms` after it.
//!
//! **Solidity:** header `x` is solid when at least 19 distinct members of the set of `x`'s period
//! signed headers after `x` in the segment with their stored keys. Headers by other producers or
//! keys are carried but never counted.
//!
//! **Advance:** at most `min(params.max_updates_per_advance, 16)` segments of at most 128
//! parent-linked signed headers. A segment may reach at most one period past the newest set;
//! then it must contain that period's maintenance block (the first header of the period, whose
//! parent is still in the previous period) and the set is learned: the distinct producers
//! (account and recovered signer) of the headers within the first 27 slots after the maintenance
//! block (plus the two skipped slots), each of which must be solid under the previous set. At
//! least 19 producers are required; witnesses that produced nothing there are evicted. Each
//! segment must make a header solid; the newest solid header becomes a checkpoint and the head.
//! A witness's rotated key is learned at the next boundary, where its window header carries it.
//!
//! **Backfill:** at most `min(params.max_backfill_headers, 1200)` unsigned `raw_data` headers
//! ending at a stored checkpoint; the first one becomes a checkpoint.
//!
//! **Proof:** the event block is the first header of a signed segment in which it is solid, or of
//! unsigned `raw_data` headers ending at a stored checkpoint. The transaction is opened by its
//! path in the SHA-256 promote-odd binary Merkle tree under `txTrieRoot` and must be one
//! successful (`contractRet = SUCCESS`) `TriggerSmartContract` with no TRX or token value. Its
//! calldata is a canonical `transferToTaira` call (the normalized event carries the caller and
//! the call; Taira rebuilds the payload) or a void call. TRON logs are not header-committed.
//!
//! **Equivocation:** a record is a signed segment; it asserts its solid headers. Two records
//! conflict when solid headers share a height with different ids, or their heights and times are
//! ordered inconsistently.

use std::collections::{BTreeMap, BTreeSet};

use core::fmt;

use iroha_crypto::EcdsaSecp256k1Sha256;
use iroha_data_model::{
    bridge::SccpNetworkV1,
    sccp::{
        inbound::SccpSourceLocatorV1,
        light_client::{
            SccpLcCheckpointDataV1, SccpLcCheckpointOriginV1, SccpLcCheckpointV1,
            SccpLcConsensusSetV1, SccpLcEquivocationFreezeV1, SccpLcFreezeReasonV1, SccpLcHeadV1,
            SccpLcPointV1, SccpLightClientParamsV1, SccpLightClientV1,
        },
        outbound::SccpVoidKindV1,
    },
};
use sha2::{Digest as _, Sha256};

use super::{
    SccpLcConflictV1, SccpLcError, SccpVerifierWorkV1,
    profile::{
        MAX_SOURCE_FUTURE_MS, TRON_ACTIVE_WITNESSES, TRON_MAX_ANCESTRY_HEADERS,
        TRON_MAX_SEGMENT_HEADERS, TRON_MAX_SEGMENTS_PER_ADVANCE, TRON_SOLID_THRESHOLD,
        TronChainProfileV1,
    },
    proof::{SccpLcSetDataV1, SccpNormalizedEventV1, SccpSourceEmitterV1, SccpVerifiedProofV1},
    state::{
        CheckpointRecorder, SccpLcDeltaV1, SccpLcInitialStateV1, SccpLcPurgeV1, SccpLcStateView,
        SccpLcSupersessionV1, is_set_fresh, state_hash,
    },
};
use crate::v1::{
    constants::MAX_VOID_FROZEN_RANGE_EVM,
    evm_abi::{AbiError, TransferToTairaCallV1, VoidCallV1},
    hashes::keccak256,
    network::tag,
};

const NETWORK: SccpNetworkV1 = SccpNetworkV1::TronMainnet;
const ADDRESS_BYTES: usize = 21;
const ADDRESS_PREFIX: u8 = 0x41;
const MAX_RAW_HEADER_BYTES: usize = 16 * 1024;
const MAX_TRANSACTION_BYTES: usize = 512 * 1024;
const MAX_MERKLE_DEPTH: usize = 64;
const TRIGGER_SMART_CONTRACT: u64 = 31;
const TRIGGER_TYPE_URL: &[u8] = b"type.googleapis.com/protocol.TriggerSmartContract";
const CONTRACT_RESULT_SUCCESS: u64 = 1;
const SECP256K1_ORDER: [u8; 32] = [
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfe,
    0xba, 0xae, 0xdc, 0xe6, 0xaf, 0x48, 0xa0, 0x3b, 0xbf, 0xd2, 0x5e, 0x8c, 0xd0, 0x36, 0x41, 0x41,
];
const SECP256K1_HALF_ORDER: [u8; 32] = [
    0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0x5d, 0x57, 0x6e, 0x73, 0x57, 0xa4, 0x50, 0x1d, 0xdf, 0xe9, 0x2f, 0x46, 0x68, 0x1b, 0x20, 0xa0,
];

// ---------------------------------------------------------------------------------------------
// Frames
// ---------------------------------------------------------------------------------------------

/// One active witness.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_sccp::light_client::tron::TronWitnessV1")]
pub struct TronWitnessV1 {
    /// 21-byte witness account address (`0x41`-prefixed).
    #[norito(with = "crate::json_utils::bytes_hex")]
    pub account_address: Vec<u8>,
    /// 21-byte address the witness's block signatures recover to.
    #[norito(with = "crate::json_utils::bytes_hex")]
    pub signing_address: Vec<u8>,
}

/// Stored active witness set of one maintenance period (`SccpLcConsensusSetV1.set_bytes`); the
/// set id is `period`.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_sccp::light_client::tron::TronWitnessSetV1")]
pub struct TronWitnessSetV1 {
    /// Maintenance period.
    #[norito(with = "crate::json_utils::u64_string")]
    pub period: u64,
    /// Witnesses in ascending account order.
    pub witnesses: Vec<TronWitnessV1>,
}

/// A header's exact `raw_data` protobuf and its witness signature.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_sccp::light_client::tron::TronSignedHeaderV1")]
pub struct TronSignedHeaderV1 {
    /// Exact `BlockHeader.raw` protobuf bytes.
    #[norito(with = "crate::json_utils::bytes_hex")]
    pub raw_data: Vec<u8>,
    /// 65-byte recoverable secp256k1 signature over `sha256(raw_data)`.
    #[norito(with = "crate::json_utils::bytes_hex")]
    pub witness_signature: Vec<u8>,
}

/// Parent-linked signed headers, ascending.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_sccp::light_client::tron::TronSegmentV1")]
pub struct TronSegmentV1 {
    /// Signed headers.
    pub headers: Vec<TronSignedHeaderV1>,
}

/// TRON advance: segments, oldest first.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_sccp::light_client::tron::TronLcAdvanceV1")]
pub struct TronLcAdvanceV1 {
    /// Segments; maintenance boundaries in source order.
    pub segments: Vec<TronSegmentV1>,
}

/// Parent-linked unsigned `raw_data` headers, ascending.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_sccp::light_client::tron::TronRawSegmentV1")]
pub struct TronRawSegmentV1 {
    /// `BlockHeader.raw` protobuf bytes.
    #[norito(with = "crate::json_utils::vec_bytes_hex")]
    pub headers: Vec<Vec<u8>>,
}

/// Inclusion of one full `protocol.Transaction` in the event block.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_sccp::light_client::tron::TronTransactionProofV1")]
pub struct TronTransactionProofV1 {
    /// Position of the transaction in the block.
    pub transaction_index: u32,
    /// Transactions in the block.
    pub transaction_count: u32,
    /// Exact `Transaction` protobuf bytes (raw data, signatures and results).
    #[norito(with = "crate::json_utils::bytes_hex")]
    pub transaction: Vec<u8>,
    /// Merkle siblings, bottom-up; a node without a sibling is promoted.
    #[norito(with = "crate::json_utils::vec_bytes_hex")]
    pub merkle_branch: Vec<Vec<u8>>,
}

/// How the event block of a proof is final.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(tag = "kind", content = "value", rename_all = "snake_case")]
#[norito_schema(name = "iroha_sccp::light_client::tron::TronProofAnchorV1")]
pub enum TronProofAnchorV1 {
    /// A signed segment starting at the event block, in which it is solid.
    Solid(TronSegmentV1),
    /// Unsigned headers from the event block to a stored checkpoint.
    Checkpoint(TronRawSegmentV1),
}

/// TRON inbound or void proof.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_sccp::light_client::tron::TronSourceProofV1")]
pub struct TronSourceProofV1 {
    /// Finality of the event block (the anchor's first header).
    pub anchor: TronProofAnchorV1,
    /// The transaction and its inclusion.
    pub transaction: TronTransactionProofV1,
}

/// One TRON equivocation record: a signed segment asserting its solid headers.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_sccp::light_client::tron::TronLcEvidenceV1")]
pub struct TronLcEvidenceV1 {
    /// Signed segment.
    pub segment: TronSegmentV1,
}

/// TRON weak-subjectivity bootstrap (`SccpLcBootstrapV1.bytes`).
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_sccp::light_client::tron::TronLcBootstrapV1")]
pub struct TronLcBootstrapV1 {
    /// The trusted active set of the checkpoint's period.
    pub set: TronWitnessSetV1,
    /// Trusted solid header (`raw_data`) of that period.
    #[norito(with = "crate::json_utils::bytes_hex")]
    pub checkpoint_header: Vec<u8>,
}

// ---------------------------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------------------------

/// TRON-specific verification failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TronLcError {
    /// A `raw_data` header is not a bounded canonical `BlockHeader.raw` protobuf.
    MalformedHeader,
    /// A witness signature is not a canonical 65-byte recoverable signature.
    MalformedSignature,
    /// A header does not link to its predecessor (parent id, height and strictly later time).
    AncestryBroken {
        /// Index of the unlinked header.
        index: usize,
    },
    /// A header lies after the compiled `supported_until`.
    UnsupportedFork {
        /// Header time (ms).
        time_ms: u64,
    },
    /// A header belongs to a period whose set is not stored and cannot be learned here.
    UnlearnedPeriod {
        /// Period.
        period: u64,
    },
    /// A segment enters a new period without its maintenance block and parent.
    BoundaryNotObserved {
        /// Period.
        period: u64,
    },
    /// A segment ends inside the learning window of a new period.
    IncompleteLearningWindow {
        /// Period.
        period: u64,
    },
    /// One witness produced window headers with two different keys.
    ConflictingWitnessKey,
    /// Fewer than 19 witnesses produced in a new period's learning window.
    TooFewProducers {
        /// Period.
        period: u64,
        /// Distinct producers.
        count: usize,
    },
    /// The header that must be solid (a proof's event block, a window header, or any header of
    /// an advance segment) is not.
    NotSolid {
        /// Header height.
        source_height: u64,
    },
    /// A checkpoint-anchored chain or backfill does not end at the stored checkpoint.
    CheckpointMismatch {
        /// Source height.
        source_height: u64,
    },
    /// A stored consensus set does not decode as a TRON set with its id, or a set is not 19..=27
    /// distinct valid witnesses in account order.
    MalformedSet {
        /// Set id.
        set_id: u64,
    },
    /// The transaction proof is malformed or does not reach the block's `txTrieRoot`.
    TransactionNotIncluded,
    /// The transaction is not a canonical single `TriggerSmartContract` protobuf.
    MalformedTransaction,
    /// The transaction did not succeed, or moved TRX or tokens.
    TransactionFailed,
    /// The calldata is not a canonical `transferToTaira` or void call.
    NotSccpCall(AbiError),
    /// The bootstrap checkpoint is not in its set's period.
    InvalidBootstrap,
    /// A height or time overflows.
    Overflow,
}

impl fmt::Display for TronLcError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedHeader => formatter.write_str("malformed TRON raw header"),
            Self::MalformedSignature => formatter.write_str("malformed witness signature"),
            Self::AncestryBroken { index } => {
                write!(formatter, "header {index} does not link to its predecessor")
            }
            Self::UnsupportedFork { time_ms } => write!(
                formatter,
                "header time {time_ms} ms is after the compiled TRON supported_until"
            ),
            Self::UnlearnedPeriod { period } => {
                write!(formatter, "the witness set of period {period} is not known")
            }
            Self::BoundaryNotObserved { period } => write!(
                formatter,
                "the segment enters period {period} without its maintenance block"
            ),
            Self::IncompleteLearningWindow { period } => write!(
                formatter,
                "the segment ends inside the learning window of period {period}"
            ),
            Self::ConflictingWitnessKey => {
                formatter.write_str("a witness produced window headers with two keys")
            }
            Self::TooFewProducers { period, count } => write!(
                formatter,
                "only {count} witnesses produced in the learning window of period {period}"
            ),
            Self::NotSolid { source_height } => {
                write!(formatter, "header {source_height} is not solid")
            }
            Self::CheckpointMismatch { source_height } => write!(
                formatter,
                "the chain does not end at the stored checkpoint {source_height}"
            ),
            Self::MalformedSet { set_id } => {
                write!(formatter, "witness set {set_id} is malformed")
            }
            Self::TransactionNotIncluded => {
                formatter.write_str("the transaction is not included in the event block")
            }
            Self::MalformedTransaction => formatter.write_str("malformed TRON transaction"),
            Self::TransactionFailed => {
                formatter.write_str("the transaction did not succeed or moved value")
            }
            Self::NotSccpCall(error) => write!(formatter, "not an SCCP call: {error}"),
            Self::InvalidBootstrap => {
                formatter.write_str("the bootstrap checkpoint is not in its set's period")
            }
            Self::Overflow => formatter.write_str("source height or time overflows"),
        }
    }
}

impl std::error::Error for TronLcError {}

impl From<TronLcError> for SccpLcError {
    fn from(value: TronLcError) -> Self {
        Self::Tron(value)
    }
}

// ---------------------------------------------------------------------------------------------
// Protobuf
// ---------------------------------------------------------------------------------------------

/// One protobuf field value.
#[derive(Clone, Copy)]
enum Wire<'a> {
    Varint(u64),
    Bytes(&'a [u8]),
    Fixed,
}

fn varint_len(mut value: u64) -> usize {
    let mut len = 1;
    while value >= 0x80 {
        len += 1;
        value >>= 7;
    }
    len
}

/// Read one minimally encoded varint at `cursor`, advancing it.
fn read_varint(bytes: &[u8], cursor: &mut usize) -> Option<u64> {
    let start = *cursor;
    let mut value = 0_u64;
    for index in 0..10_u32 {
        let byte = *bytes.get(*cursor)?;
        *cursor += 1;
        let chunk = u64::from(byte & 0x7f);
        if index == 9 && chunk > 1 {
            return None;
        }
        value |= chunk << (7 * index);
        if byte & 0x80 == 0 {
            return (*cursor - start == varint_len(value)).then_some(value);
        }
    }
    None
}

/// Decode a protobuf message into `(field, value)` pairs in encoding order.
fn fields(bytes: &[u8]) -> Option<Vec<(u64, Wire<'_>)>> {
    let mut cursor = 0;
    let mut out = Vec::new();
    while cursor < bytes.len() {
        let key = read_varint(bytes, &mut cursor)?;
        let field = key >> 3;
        if field == 0 {
            return None;
        }
        let value = match key & 7 {
            0 => Wire::Varint(read_varint(bytes, &mut cursor)?),
            1 => {
                cursor = cursor.checked_add(8).filter(|end| *end <= bytes.len())?;
                Wire::Fixed
            }
            2 => {
                let len = usize::try_from(read_varint(bytes, &mut cursor)?).ok()?;
                let end = cursor.checked_add(len).filter(|end| *end <= bytes.len())?;
                let value = &bytes[cursor..end];
                cursor = end;
                Wire::Bytes(value)
            }
            5 => {
                cursor = cursor.checked_add(4).filter(|end| *end <= bytes.len())?;
                Wire::Fixed
            }
            _ => return None,
        };
        out.push((field, value));
    }
    Some(out)
}

/// The single occurrence of each non-repeated field; `None` for a duplicate.
fn singular<'a>(fields: &[(u64, Wire<'a>)], repeated: &[u64]) -> Option<BTreeMap<u64, Wire<'a>>> {
    let mut out = BTreeMap::new();
    for (field, value) in fields {
        if repeated.contains(field) {
            continue;
        }
        if out.insert(*field, *value).is_some() {
            return None;
        }
    }
    Some(out)
}

fn varint_of(map: &BTreeMap<u64, Wire<'_>>, field: u64) -> Option<Option<u64>> {
    match map.get(&field) {
        None => Some(None),
        Some(Wire::Varint(value)) => Some(Some(*value)),
        Some(_) => None,
    }
}

fn bytes_of<'a>(map: &BTreeMap<u64, Wire<'a>>, field: u64) -> Option<Option<&'a [u8]>> {
    match map.get(&field) {
        None => Some(None),
        Some(Wire::Bytes(value)) => Some(Some(value)),
        Some(_) => None,
    }
}

// ---------------------------------------------------------------------------------------------
// Headers
// ---------------------------------------------------------------------------------------------

/// Fields of a decoded `BlockHeader.raw`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HeaderV1 {
    id: [u8; 32],
    raw_hash: [u8; 32],
    number: u64,
    time_ms: u64,
    tx_root: [u8; 32],
    parent_id: [u8; 32],
    witness: [u8; ADDRESS_BYTES],
    state_root: Option<[u8; 32]>,
}

impl HeaderV1 {
    const fn point(&self) -> SccpLcPointV1 {
        SccpLcPointV1 {
            source_height: self.number,
            block_hash: self.id,
            source_time_ms: self.time_ms,
        }
    }

    const fn checkpoint(&self) -> SccpLcCheckpointDataV1 {
        SccpLcCheckpointDataV1 {
            source_height: self.number,
            block_hash: self.id,
            state_root: self.state_root,
            receipts_or_tx_root: self.tx_root,
            source_time_ms: self.time_ms,
        }
    }
}

fn is_address(bytes: &[u8]) -> bool {
    bytes.len() == ADDRESS_BYTES && bytes[0] == ADDRESS_PREFIX && bytes[1..] != [0; 20]
}

/// Decode `BlockHeader.raw`: `timestamp` (1), `txTrieRoot` (2), `parentHash` (3), `number` (7),
/// `witness_id` (8), `witness_address` (9), `version` (10) and `accountStateRoot` (11), no
/// other or duplicate field. The block id is `sha256(raw)` with its first eight bytes replaced
/// by the big-endian number.
fn decode_header(raw: &[u8]) -> Result<HeaderV1, TronLcError> {
    let malformed = || TronLcError::MalformedHeader;
    if raw.is_empty() || raw.len() > MAX_RAW_HEADER_BYTES {
        return Err(malformed());
    }
    let map = singular(&fields(raw).ok_or_else(malformed)?, &[]).ok_or_else(malformed)?;
    if map.keys().any(|field| !matches!(field, 1..=3 | 7..=11)) {
        return Err(malformed());
    }
    let fixed32 = |field| -> Result<Option<[u8; 32]>, TronLcError> {
        bytes_of(&map, field)
            .ok_or_else(malformed)?
            .map(|bytes| <[u8; 32]>::try_from(bytes).map_err(|_| malformed()))
            .transpose()
    };
    let number = varint_of(&map, 7)
        .ok_or_else(malformed)?
        .ok_or_else(malformed)?;
    let time_ms = varint_of(&map, 1)
        .ok_or_else(malformed)?
        .ok_or_else(malformed)?;
    let parent_id = fixed32(3)?.ok_or_else(malformed)?;
    let witness = bytes_of(&map, 9)
        .ok_or_else(malformed)?
        .filter(|bytes| is_address(bytes))
        .and_then(|bytes| <[u8; ADDRESS_BYTES]>::try_from(bytes).ok())
        .ok_or_else(malformed)?;
    varint_of(&map, 8).ok_or_else(malformed)?;
    varint_of(&map, 10).ok_or_else(malformed)?;
    if number == 0 || time_ms == 0 || i64::try_from(number).is_err() || parent_id == [0; 32] {
        return Err(malformed());
    }
    let raw_hash: [u8; 32] = Sha256::digest(raw).into();
    let mut id = raw_hash;
    id[..8].copy_from_slice(&number.to_be_bytes());
    Ok(HeaderV1 {
        id,
        raw_hash,
        number,
        time_ms,
        tx_root: fixed32(2)?.unwrap_or([0; 32]),
        parent_id,
        witness,
        state_root: fixed32(11)?,
    })
}

/// Recover the 21-byte signer address of a witness signature (`v ∈ {0, 1, 27, 28}`, low `s`).
fn recover_signer(
    raw_hash: &[u8; 32],
    signature: &[u8],
) -> Result<[u8; ADDRESS_BYTES], TronLcError> {
    let signature = <[u8; 65]>::try_from(signature).map_err(|_| TronLcError::MalformedSignature)?;
    let r = &signature[..32];
    let s = &signature[32..64];
    let recovery = match signature[64] {
        v @ (0 | 1) => v,
        v @ (27 | 28) => v - 27,
        _ => return Err(TronLcError::MalformedSignature),
    };
    if r.iter().all(|byte| *byte == 0)
        || r >= &SECP256K1_ORDER[..]
        || s.iter().all(|byte| *byte == 0)
        || s > &SECP256K1_HALF_ORDER[..]
    {
        return Err(TronLcError::MalformedSignature);
    }
    let mut normalized = signature;
    normalized[64] = recovery + 27;
    let public_key = EcdsaSecp256k1Sha256::recover_public_key_from_prehash(raw_hash, &normalized)
        .map_err(|_| TronLcError::MalformedSignature)?;
    let mut address = [0_u8; ADDRESS_BYTES];
    address[0] = ADDRESS_PREFIX;
    address[1..].copy_from_slice(&EcdsaSecp256k1Sha256::evm_address(&public_key));
    Ok(address)
}

/// A header with the address its signature recovers to.
#[derive(Clone, Copy, Debug)]
struct SignedV1 {
    header: HeaderV1,
    signer: [u8; ADDRESS_BYTES],
}

// ---------------------------------------------------------------------------------------------
// Sets
// ---------------------------------------------------------------------------------------------

/// Check a witness set: 19..=27 distinct valid witnesses in ascending account order.
fn check_set(set: &TronWitnessSetV1) -> Result<(), TronLcError> {
    let malformed = TronLcError::MalformedSet { set_id: set.period };
    let count = set.witnesses.len();
    if !(TRON_SOLID_THRESHOLD..=TRON_ACTIVE_WITNESSES).contains(&count) {
        return Err(malformed);
    }
    for pair in set.witnesses.windows(2) {
        if pair[0].account_address >= pair[1].account_address {
            return Err(malformed);
        }
    }
    if set.witnesses.iter().any(|witness| {
        !is_address(&witness.account_address) || !is_address(&witness.signing_address)
    }) {
        return Err(malformed);
    }
    Ok(())
}

fn decode_set(set: &SccpLcConsensusSetV1) -> Result<TronWitnessSetV1, SccpLcError> {
    let malformed = TronLcError::MalformedSet { set_id: set.set_id };
    let decoded = SccpLcSetDataV1::from_frame(&set.set_bytes).map_err(|_| malformed)?;
    let SccpLcSetDataV1::Tron(data) = decoded else {
        return Err(malformed.into());
    };
    if data.period != set.set_id {
        return Err(malformed.into());
    }
    check_set(&data)?;
    Ok(data)
}

/// Witness keys of a set: account → signer.
fn keys(set: &TronWitnessSetV1) -> BTreeMap<&[u8], &[u8]> {
    set.witnesses
        .iter()
        .map(|witness| {
            (
                witness.account_address.as_slice(),
                witness.signing_address.as_slice(),
            )
        })
        .collect()
}

/// Whether at least 19 distinct members of `set` signed headers after `index` with their keys.
fn is_solid(set: &TronWitnessSetV1, chain: &[SignedV1], index: usize) -> bool {
    let keys = keys(set);
    let builders: BTreeSet<&[u8]> = chain[index + 1..]
        .iter()
        .filter(|signed| {
            keys.get(signed.header.witness.as_slice())
                .is_some_and(|key| *key == signed.signer.as_slice())
        })
        .map(|signed| signed.header.witness.as_slice())
        .collect();
    builders.len() >= TRON_SOLID_THRESHOLD
}

/// Stored sets plus the sets an advance learns.
struct Sets<'a, V: SccpLcStateView + ?Sized> {
    view: &'a V,
    learned: BTreeMap<u64, SccpLcConsensusSetV1>,
    superseded: BTreeMap<u64, u64>,
}

impl<'a, V: SccpLcStateView + ?Sized> Sets<'a, V> {
    const fn new(view: &'a V) -> Self {
        Self {
            view,
            learned: BTreeMap::new(),
            superseded: BTreeMap::new(),
        }
    }

    fn get(&self, period: u64) -> Option<SccpLcConsensusSetV1> {
        self.learned
            .get(&period)
            .cloned()
            .or_else(|| self.view.consensus_set(NETWORK, period))
    }
}

struct Ctx<'a> {
    profile: &'a TronChainProfileV1,
    params: &'a SccpLightClientParamsV1,
    now: u64,
    /// Newest set (period).
    newest: u64,
}

impl Ctx<'_> {
    fn check_time(&self, header: &HeaderV1) -> Result<(), SccpLcError> {
        if header.time_ms > self.profile.supported_until_ms {
            return Err(TronLcError::UnsupportedFork {
                time_ms: header.time_ms,
            }
            .into());
        }
        if header.time_ms > self.now.saturating_add(MAX_SOURCE_FUTURE_MS) {
            return Err(SccpLcError::SourceTimeInFuture {
                source_ms: header.time_ms,
                taira_now_ms: self.now,
            });
        }
        Ok(())
    }

    /// Decode and link raw headers (1..=`max`).
    fn linked(
        &self,
        raws: &[&[u8]],
        max: usize,
        kind: &'static str,
    ) -> Result<Vec<HeaderV1>, SccpLcError> {
        let count = raws.len();
        if count == 0 {
            return Err(SccpLcError::TooFewItems {
                kind,
                count,
                min: 1,
            });
        }
        if count > max {
            return Err(SccpLcError::TooManyItems { kind, count, max });
        }
        let headers = raws
            .iter()
            .map(|raw| decode_header(raw))
            .collect::<Result<Vec<_>, _>>()?;
        for (index, pair) in headers.windows(2).enumerate() {
            if pair[1].parent_id != pair[0].id
                || pair[0].number.checked_add(1) != Some(pair[1].number)
                || pair[1].time_ms <= pair[0].time_ms
            {
                return Err(TronLcError::AncestryBroken { index: index + 1 }.into());
            }
        }
        for header in &headers {
            self.check_time(header)?;
        }
        Ok(headers)
    }

    /// Decode, link and recover a signed segment.
    fn signed(
        &self,
        segment: &TronSegmentV1,
        kind: &'static str,
    ) -> Result<Vec<SignedV1>, SccpLcError> {
        let max = count_bound(self.params.max_segment_headers, TRON_MAX_SEGMENT_HEADERS);
        let raws: Vec<&[u8]> = segment
            .headers
            .iter()
            .map(|header| header.raw_data.as_slice())
            .collect();
        let headers = self.linked(&raws, max, kind)?;
        headers
            .into_iter()
            .zip(&segment.headers)
            .map(|(header, signed)| {
                Ok(SignedV1 {
                    signer: recover_signer(&header.raw_hash, &signed.witness_signature)?,
                    header,
                })
            })
            .collect()
    }

    fn period(&self, header: &HeaderV1) -> u64 {
        self.profile.period_at(header.time_ms)
    }

    /// The fresh stored (or learned) set of `period`.
    fn fresh_set<V: SccpLcStateView + ?Sized>(
        &self,
        sets: &Sets<'_, V>,
        period: u64,
    ) -> Result<TronWitnessSetV1, SccpLcError> {
        let record = sets
            .get(period)
            .ok_or(TronLcError::UnlearnedPeriod { period })?;
        let expiry = self
            .profile
            .period_end_ms(period)
            .ok_or(TronLcError::Overflow)?;
        if !is_set_fresh(Some(expiry), self.params.ws_bound_ms, self.now) {
            return Err(SccpLcError::StaleSigningSet {
                set_id: period,
                stale_from_ms: expiry.saturating_add(self.params.ws_bound_ms),
            });
        }
        decode_set(&record)
    }

    /// The newest header of `chain` that is solid under the fresh set of its period.
    fn newest_solid<V: SccpLcStateView + ?Sized>(
        &self,
        sets: &Sets<'_, V>,
        chain: &[SignedV1],
    ) -> Result<Option<usize>, SccpLcError> {
        for index in (0..chain.len()).rev() {
            let period = self.period(&chain[index].header);
            if period > self.newest {
                continue;
            }
            if is_solid(&self.fresh_set(sets, period)?, chain, index) {
                return Ok(Some(index));
            }
        }
        Ok(None)
    }
}

fn count_bound(param: u32, hard: usize) -> usize {
    usize::try_from(param).unwrap_or(usize::MAX).min(hard)
}

fn set_record(set: TronWitnessSetV1, valid_from: u64) -> Result<SccpLcConsensusSetV1, SccpLcError> {
    Ok(SccpLcConsensusSetV1 {
        set_id: set.period,
        valid_from_source_height: valid_from,
        superseded_at_source_ms: None,
        set_bytes: SccpLcSetDataV1::Tron(set).to_frame()?,
    })
}

/// Learn the set of period `ctx.newest + 1` from `chain`, which must contain its maintenance
/// block and complete learning window.
fn learn_next<V: SccpLcStateView + ?Sized>(
    ctx: &Ctx<'_>,
    sets: &Sets<'_, V>,
    chain: &[SignedV1],
) -> Result<(TronWitnessSetV1, u64, u64), SccpLcError> {
    let period = ctx.newest.checked_add(1).ok_or(TronLcError::Overflow)?;
    let boundary = chain
        .iter()
        .position(|signed| ctx.period(&signed.header) == period)
        .filter(|index| *index > 0 && ctx.period(&chain[index - 1].header) == ctx.newest)
        .ok_or(TronLcError::BoundaryNotObserved { period })?;
    let maintenance = chain[boundary].header;
    let window_end = maintenance
        .time_ms
        .checked_add(ctx.profile.learning_window_ms())
        .ok_or(TronLcError::Overflow)?;
    if chain[chain.len() - 1].header.time_ms <= window_end {
        return Err(TronLcError::IncompleteLearningWindow { period }.into());
    }
    let previous = ctx.fresh_set(sets, ctx.newest)?;
    let mut producers: BTreeMap<[u8; ADDRESS_BYTES], [u8; ADDRESS_BYTES]> = BTreeMap::new();
    for (index, signed) in chain.iter().enumerate().skip(boundary + 1) {
        if signed.header.time_ms > window_end {
            break;
        }
        if !is_solid(&previous, chain, index) {
            return Err(TronLcError::NotSolid {
                source_height: signed.header.number,
            }
            .into());
        }
        if let Some(existing) = producers.insert(signed.header.witness, signed.signer)
            && existing != signed.signer
        {
            return Err(TronLcError::ConflictingWitnessKey.into());
        }
    }
    if producers.len() < TRON_SOLID_THRESHOLD {
        return Err(TronLcError::TooFewProducers {
            period,
            count: producers.len(),
        }
        .into());
    }
    let set = TronWitnessSetV1 {
        period,
        witnesses: producers
            .into_iter()
            .map(|(account, signer)| TronWitnessV1 {
                account_address: account.to_vec(),
                signing_address: signer.to_vec(),
            })
            .collect(),
    };
    let superseded_at = ctx
        .profile
        .period_start_ms(period)
        .ok_or(TronLcError::Overflow)?;
    Ok((set, maintenance.number, superseded_at))
}

// ---------------------------------------------------------------------------------------------
// Bootstrap
// ---------------------------------------------------------------------------------------------

/// Verify a TRON bootstrap (§4.13.2, §4.14.3 `InitializeLightClient`).
///
/// The set is well formed, the checkpoint lies in the set's period within the fork bound and not
/// in the future, and the set is fresh at `taira_now_ms`.
pub(super) fn verify_bootstrap(
    profile: &TronChainProfileV1,
    params: &SccpLightClientParamsV1,
    bootstrap: &TronLcBootstrapV1,
    taira_now_ms: u64,
) -> Result<SccpLcInitialStateV1, SccpLcError> {
    check_set(&bootstrap.set)?;
    let period = bootstrap.set.period;
    let ctx = Ctx {
        profile,
        params,
        now: taira_now_ms,
        newest: period,
    };
    let checkpoint = decode_header(&bootstrap.checkpoint_header)?;
    ctx.check_time(&checkpoint)?;
    if ctx.period(&checkpoint) != period {
        return Err(TronLcError::InvalidBootstrap.into());
    }
    let expiry = profile.period_end_ms(period).ok_or(TronLcError::Overflow)?;
    if !is_set_fresh(Some(expiry), params.ws_bound_ms, taira_now_ms) {
        return Err(SccpLcError::StaleSigningSet {
            set_id: period,
            stale_from_ms: expiry.saturating_add(params.ws_bound_ms),
        });
    }
    let head = SccpLcHeadV1 {
        latest_set_id: period,
        latest_finalized: checkpoint.point(),
        last_progress_taira_ms: taira_now_ms,
    };
    Ok(SccpLcInitialStateV1 {
        light_client: SccpLightClientV1 {
            params: *params,
            head,
            frozen: None,
            state_hash: state_hash(params, &head, None),
        },
        purge: SccpLcPurgeV1::DiscardUnvetted,
        superseded_sets: Vec::new(),
        sets: vec![set_record(bootstrap.set.clone(), checkpoint.number)?],
        checkpoints: vec![SccpLcCheckpointV1 {
            data: checkpoint.checkpoint(),
            recorded_at_taira_ms: taira_now_ms,
            origin: SccpLcCheckpointOriginV1::Parliament,
        }],
    })
}

// ---------------------------------------------------------------------------------------------
// Advance
// ---------------------------------------------------------------------------------------------

/// Verify a TRON advance and return what to write.
pub(super) fn apply_advance<V: SccpLcStateView + ?Sized>(
    profile: &TronChainProfileV1,
    view: &V,
    light_client: &SccpLightClientV1,
    advance: &TronLcAdvanceV1,
    taira_now_ms: u64,
) -> Result<SccpLcDeltaV1, SccpLcError> {
    let params = &light_client.params;
    let count = advance.segments.len();
    if count == 0 {
        return Err(SccpLcError::TooFewItems {
            kind: "advance segments",
            count,
            min: 1,
        });
    }
    let max = count_bound(
        params.max_updates_per_advance,
        TRON_MAX_SEGMENTS_PER_ADVANCE,
    );
    if count > max {
        return Err(SccpLcError::TooManyItems {
            kind: "advance segments",
            count,
            max,
        });
    }
    let mut ctx = Ctx {
        profile,
        params,
        now: taira_now_ms,
        newest: light_client.head.latest_set_id,
    };
    let mut sets = Sets::new(view);
    let mut recorder = CheckpointRecorder::new(
        view,
        NETWORK,
        SccpLcCheckpointOriginV1::Advance,
        taira_now_ms,
    );
    let mut latest_finalized = light_client.head.latest_finalized;
    for segment in &advance.segments {
        let chain = ctx.signed(segment, "segment headers")?;
        let highest = chain
            .iter()
            .map(|signed| ctx.period(&signed.header))
            .max()
            .unwrap_or_default();
        if highest > ctx.newest.saturating_add(1) {
            return Err(TronLcError::UnlearnedPeriod {
                period: ctx.newest.saturating_add(1),
            }
            .into());
        }
        if highest == ctx.newest.saturating_add(1) {
            let (set, valid_from, superseded_at) = learn_next(&ctx, &sets, &chain)?;
            let record = set_record(set, valid_from)?;
            if let Some(existing) = view.consensus_set(NETWORK, record.set_id)
                && existing.set_bytes != record.set_bytes
            {
                return Err(SccpLcError::ConflictsWithStoredData(
                    SccpLcConflictV1::ConsensusSet {
                        set_id: record.set_id,
                    },
                ));
            }
            if sets.get(record.set_id).is_none() {
                if let Some(pending) = sets.learned.get_mut(&ctx.newest) {
                    pending.superseded_at_source_ms = Some(superseded_at);
                } else if view
                    .consensus_set(NETWORK, ctx.newest)
                    .is_some_and(|stored| stored.superseded_at_source_ms.is_none())
                {
                    sets.superseded.insert(ctx.newest, superseded_at);
                }
                sets.learned.insert(record.set_id, record);
            }
            ctx.newest += 1;
        }
        let solid = ctx
            .newest_solid(&sets, &chain)?
            .ok_or_else(|| TronLcError::NotSolid {
                source_height: chain[0].header.number,
            })?;
        let header = chain[solid].header;
        recorder.record(header.checkpoint())?;
        if header.number > latest_finalized.source_height {
            latest_finalized = header.point();
        }
    }
    let moved = ctx.newest != light_client.head.latest_set_id
        || latest_finalized != light_client.head.latest_finalized;
    Ok(SccpLcDeltaV1 {
        new_sets: sets.learned.into_values().collect(),
        superseded_sets: sets
            .superseded
            .into_iter()
            .map(|(set_id, superseded_at_source_ms)| SccpLcSupersessionV1 {
                set_id,
                superseded_at_source_ms,
            })
            .collect(),
        checkpoints: recorder.into_vec(),
        head: moved.then_some(SccpLcHeadV1 {
            latest_set_id: ctx.newest,
            latest_finalized,
            last_progress_taira_ms: taira_now_ms,
        }),
    })
}

fn raw_slices(segment: &TronRawSegmentV1) -> Vec<&[u8]> {
    segment.headers.iter().map(Vec::as_slice).collect()
}

/// Check that `headers` end at the stored checkpoint of their last height.
fn ends_at_checkpoint<V: SccpLcStateView + ?Sized>(
    view: &V,
    headers: &[HeaderV1],
) -> Result<(), SccpLcError> {
    let last = headers[headers.len() - 1];
    let stored = view
        .checkpoint(NETWORK, last.number)
        .ok_or(SccpLcError::UnknownCheckpoint {
            source_height: last.number,
        })?;
    if stored.data.block_hash == last.id {
        Ok(())
    } else {
        Err(TronLcError::CheckpointMismatch {
            source_height: last.number,
        }
        .into())
    }
}

/// Verify a `Backfill` segment and return the checkpoint of its first header.
pub(super) fn apply_backfill<V: SccpLcStateView + ?Sized>(
    profile: &TronChainProfileV1,
    view: &V,
    light_client: &SccpLightClientV1,
    segment: &TronRawSegmentV1,
    taira_now_ms: u64,
) -> Result<SccpLcDeltaV1, SccpLcError> {
    let count = segment.headers.len();
    if count < 2 {
        return Err(SccpLcError::TooFewItems {
            kind: "backfill headers",
            count,
            min: 2,
        });
    }
    let params = &light_client.params;
    let ctx = Ctx {
        profile,
        params,
        now: taira_now_ms,
        newest: light_client.head.latest_set_id,
    };
    let max = count_bound(params.max_backfill_headers, TRON_MAX_ANCESTRY_HEADERS);
    let headers = ctx.linked(&raw_slices(segment), max, "backfill headers")?;
    ends_at_checkpoint(view, &headers)?;
    let mut recorder = CheckpointRecorder::new(
        view,
        NETWORK,
        SccpLcCheckpointOriginV1::Backfill,
        taira_now_ms,
    );
    recorder.record(headers[0].checkpoint())?;
    Ok(SccpLcDeltaV1 {
        checkpoints: recorder.into_vec(),
        ..SccpLcDeltaV1::default()
    })
}

// ---------------------------------------------------------------------------------------------
// Transactions
// ---------------------------------------------------------------------------------------------

fn merkle_node(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(left);
    hasher.update(right);
    hasher.finalize().into()
}

/// The root of the promote-odd SHA-256 tree reached from `leaf` at `index` of `count`.
fn merkle_root(leaf: [u8; 32], index: u32, count: u32, branch: &[Vec<u8>]) -> Option<[u8; 32]> {
    if count == 0 || index >= count || branch.len() > MAX_MERKLE_DEPTH {
        return None;
    }
    let mut siblings = branch.iter();
    let mut current = leaf;
    let (mut index, mut count) = (index, count);
    while count > 1 {
        if index & 1 == 1 {
            let sibling = <[u8; 32]>::try_from(siblings.next()?.as_slice()).ok()?;
            current = merkle_node(&sibling, &current);
        } else if index + 1 < count {
            let sibling = <[u8; 32]>::try_from(siblings.next()?.as_slice()).ok()?;
            current = merkle_node(&current, &sibling);
        }
        index >>= 1;
        count = count.div_ceil(2);
    }
    siblings.next().is_none().then_some(current)
}

/// A successful direct `TriggerSmartContract` call.
struct TriggerCallV1 {
    owner: [u8; ADDRESS_BYTES],
    contract: [u8; ADDRESS_BYTES],
    data: Vec<u8>,
}

/// Decode a full `protocol.Transaction`: exactly one `raw_data`, any signatures, exactly one
/// result with `contractRet = SUCCESS` and no failed `ret`; the raw data carries exactly one
/// `TriggerSmartContract` contract with no TRX or token value. Every field that cannot change
/// the executed call (memo, permission id, fee limit, `TAPoS`, expiration, timestamp, other
/// result fields) is accepted.
fn decode_trigger_call(transaction: &[u8]) -> Result<TriggerCallV1, TronLcError> {
    let malformed = || TronLcError::MalformedTransaction;
    let top = fields(transaction).ok_or_else(malformed)?;
    if top.iter().any(|(field, _)| !matches!(field, 1 | 2 | 5)) {
        return Err(malformed());
    }
    let map = singular(&top, &[2, 5]).ok_or_else(malformed)?;
    let raw = bytes_of(&map, 1)
        .ok_or_else(malformed)?
        .ok_or_else(malformed)?;
    let results: Vec<&[u8]> = top
        .iter()
        .filter(|(field, _)| *field == 5)
        .map(|(_, value)| match value {
            Wire::Bytes(bytes) => Ok(*bytes),
            _ => Err(malformed()),
        })
        .collect::<Result<_, _>>()?;
    let [result] = results.as_slice() else {
        return Err(malformed());
    };
    let result =
        singular(&fields(result).ok_or_else(malformed)?, &[26, 28]).ok_or_else(malformed)?;
    if varint_of(&result, 2).ok_or_else(malformed)?.unwrap_or(0) != 0
        || varint_of(&result, 3).ok_or_else(malformed)? != Some(CONTRACT_RESULT_SUCCESS)
    {
        return Err(TronLcError::TransactionFailed);
    }
    let raw_fields = fields(raw).ok_or_else(malformed)?;
    if raw_fields
        .iter()
        .any(|(field, _)| !matches!(field, 1 | 3 | 4 | 8 | 9 | 10 | 11 | 12 | 14 | 18))
    {
        return Err(malformed());
    }
    let contracts: Vec<&[u8]> = raw_fields
        .iter()
        .filter(|(field, _)| *field == 11)
        .map(|(_, value)| match value {
            Wire::Bytes(bytes) => Ok(*bytes),
            _ => Err(malformed()),
        })
        .collect::<Result<_, _>>()?;
    singular(&raw_fields, &[9, 11]).ok_or_else(malformed)?;
    let [contract] = contracts.as_slice() else {
        return Err(malformed());
    };
    let contract = fields(contract).ok_or_else(malformed)?;
    if contract.iter().any(|(field, _)| !matches!(field, 1..=5)) {
        return Err(malformed());
    }
    let contract = singular(&contract, &[]).ok_or_else(malformed)?;
    if varint_of(&contract, 1).ok_or_else(malformed)? != Some(TRIGGER_SMART_CONTRACT) {
        return Err(malformed());
    }
    let any = bytes_of(&contract, 2)
        .ok_or_else(malformed)?
        .ok_or_else(malformed)?;
    let any = singular(&fields(any).ok_or_else(malformed)?, &[]).ok_or_else(malformed)?;
    if any.keys().any(|field| !matches!(field, 1 | 2))
        || bytes_of(&any, 1).ok_or_else(malformed)? != Some(TRIGGER_TYPE_URL)
    {
        return Err(malformed());
    }
    let call = bytes_of(&any, 2)
        .ok_or_else(malformed)?
        .ok_or_else(malformed)?;
    let call = singular(&fields(call).ok_or_else(malformed)?, &[]).ok_or_else(malformed)?;
    if call.keys().any(|field| !matches!(field, 1..=6)) {
        return Err(malformed());
    }
    let address = |field| -> Result<[u8; ADDRESS_BYTES], TronLcError> {
        bytes_of(&call, field)
            .ok_or_else(malformed)?
            .filter(|bytes| is_address(bytes))
            .and_then(|bytes| <[u8; ADDRESS_BYTES]>::try_from(bytes).ok())
            .ok_or_else(malformed)
    };
    if varint_of(&call, 3).ok_or_else(malformed)?.unwrap_or(0) != 0
        || varint_of(&call, 5).ok_or_else(malformed)?.unwrap_or(0) != 0
    {
        return Err(TronLcError::TransactionFailed);
    }
    varint_of(&call, 6).ok_or_else(malformed)?;
    Ok(TriggerCallV1 {
        owner: address(1)?,
        contract: address(2)?,
        data: bytes_of(&call, 4)
            .ok_or_else(malformed)?
            .unwrap_or_default()
            .to_vec(),
    })
}

/// The normalized event of a successful SCCP call in the event block.
fn select_event(
    header: &HeaderV1,
    proof: &TronTransactionProofV1,
) -> Result<SccpNormalizedEventV1, TronLcError> {
    if proof.transaction.is_empty() || proof.transaction.len() > MAX_TRANSACTION_BYTES {
        return Err(TronLcError::TransactionNotIncluded);
    }
    let leaf: [u8; 32] = Sha256::digest(&proof.transaction).into();
    if merkle_root(
        leaf,
        proof.transaction_index,
        proof.transaction_count,
        &proof.merkle_branch,
    ) != Some(header.tx_root)
    {
        return Err(TronLcError::TransactionNotIncluded);
    }
    let call = decode_trigger_call(&proof.transaction)?;
    let emitter = SccpSourceEmitterV1::Tron(call.contract);
    let locator = SccpSourceLocatorV1 {
        source_height: header.number,
        block_hash: header.id,
        index_in_block: proof.transaction_index,
    };
    if let Ok(transfer) = TransferToTairaCallV1::decode(&call.data) {
        let mut caller = [0_u8; 20];
        caller.copy_from_slice(&call.owner[1..]);
        return Ok(SccpNormalizedEventV1::TransferCall {
            emitter,
            caller,
            call: transfer,
            locator,
        });
    }
    let void = VoidCallV1::decode(&call.data).map_err(TronLcError::NotSccpCall)?;
    let (first_nonce, count) = void.range();
    if count == 0 || count > MAX_VOID_FROZEN_RANGE_EVM {
        return Err(TronLcError::NotSccpCall(AbiError::BadLength));
    }
    Ok(SccpNormalizedEventV1::Void {
        emitter,
        kind: match void {
            VoidCallV1::Expired { .. } => SccpVoidKindV1::Expired,
            VoidCallV1::Frozen { .. } => SccpVoidKindV1::Frozen,
        },
        first_nonce,
        count,
        message_id_or_zero: [0; 32],
        locator,
    })
}

// ---------------------------------------------------------------------------------------------
// Proof
// ---------------------------------------------------------------------------------------------

/// Verify a TRON inbound or void proof.
pub(super) fn verify_proof<V: SccpLcStateView + ?Sized>(
    profile: &TronChainProfileV1,
    view: &V,
    light_client: &SccpLightClientV1,
    proof: &TronSourceProofV1,
    taira_now_ms: u64,
) -> Result<SccpVerifiedProofV1, SccpLcError> {
    let params = &light_client.params;
    let ctx = Ctx {
        profile,
        params,
        now: taira_now_ms,
        newest: light_client.head.latest_set_id,
    };
    let sets = Sets::new(view);
    let event_block = match &proof.anchor {
        TronProofAnchorV1::Solid(segment) => {
            let chain = ctx.signed(segment, "proof headers")?;
            let event = chain[0].header;
            let set = ctx.fresh_set(&sets, ctx.period(&event))?;
            if !is_solid(&set, &chain, 0) {
                return Err(TronLcError::NotSolid {
                    source_height: event.number,
                }
                .into());
            }
            event
        }
        TronProofAnchorV1::Checkpoint(segment) => {
            let max = count_bound(params.max_ancestry_headers, TRON_MAX_ANCESTRY_HEADERS);
            let headers = ctx.linked(&raw_slices(segment), max, "ancestry headers")?;
            ends_at_checkpoint(view, &headers)?;
            headers[0]
        }
    };
    let mut recorder =
        CheckpointRecorder::new(view, NETWORK, SccpLcCheckpointOriginV1::Proof, taira_now_ms);
    recorder.record(event_block.checkpoint())?;
    let event = select_event(&event_block, &proof.transaction)?;
    Ok(SccpVerifiedProofV1 {
        event,
        checkpoints: recorder.into_vec(),
    })
}

// ---------------------------------------------------------------------------------------------
// Equivocation
// ---------------------------------------------------------------------------------------------

/// The solid headers a record asserts: `(height, id, time)`.
fn solid_claims<V: SccpLcStateView + ?Sized>(
    ctx: &Ctx<'_>,
    view: &V,
    evidence: &TronLcEvidenceV1,
) -> Result<Vec<(u64, [u8; 32], u64)>, SccpLcError> {
    let sets = Sets::new(view);
    let chain = ctx.signed(&evidence.segment, "evidence headers")?;
    let newest = ctx
        .newest_solid(&sets, &chain)?
        .ok_or_else(|| TronLcError::NotSolid {
            source_height: chain[0].header.number,
        })?;
    // Every ancestor of a solid header in the segment is canonical as well.
    Ok(chain[..=newest]
        .iter()
        .map(|signed| {
            (
                signed.header.number,
                signed.header.id,
                signed.header.time_ms,
            )
        })
        .collect())
}

/// Verify two conflicting TRON records and return the freeze reason.
pub(super) fn verify_equivocation<V: SccpLcStateView + ?Sized>(
    profile: &TronChainProfileV1,
    view: &V,
    light_client: &SccpLightClientV1,
    first: (&TronLcEvidenceV1, &[u8]),
    second: (&TronLcEvidenceV1, &[u8]),
    taira_now_ms: u64,
) -> Result<SccpLcFreezeReasonV1, SccpLcError> {
    if first.1 == second.1 {
        return Err(SccpLcError::EvidenceNotConflicting);
    }
    let ctx = Ctx {
        profile,
        params: &light_client.params,
        now: taira_now_ms,
        newest: light_client.head.latest_set_id,
    };
    let a = solid_claims(&ctx, view, first.0)?;
    let b = solid_claims(&ctx, view, second.0)?;
    let conflict = a.iter().any(|(x_height, x_id, x_time)| {
        b.iter().any(|(y_height, y_id, y_time)| {
            if x_height == y_height {
                x_id != y_id
            } else {
                (x_height < y_height) != (x_time < y_time)
            }
        })
    });
    if !conflict {
        return Err(SccpLcError::EvidenceNotConflicting);
    }
    let mut hashes = [keccak256(&[first.1]), keccak256(&[second.1])];
    hashes.sort_unstable();
    Ok(SccpLcFreezeReasonV1::Equivocation(
        SccpLcEquivocationFreezeV1 {
            evidence_hash: keccak256(&[
                b"SCCP/LC/EVIDENCE/V1",
                &[tag(NETWORK)],
                &hashes[0],
                &hashes[1],
            ]),
        },
    ))
}

// ---------------------------------------------------------------------------------------------
// Freshness and work
// ---------------------------------------------------------------------------------------------

/// Taira time from which the newest set is stale: `ws_bound_ms` after its period ends.
pub(super) fn weak_subjectivity_deadline_ms(
    profile: &TronChainProfileV1,
    light_client: &SccpLightClientV1,
) -> u64 {
    profile
        .period_end_ms(light_client.head.latest_set_id)
        .unwrap_or(u64::MAX)
        .saturating_add(light_client.params.ws_bound_ms)
}

/// Whether the newest set is beyond the weak-subjectivity bound.
pub(super) fn is_aged(
    profile: &TronChainProfileV1,
    light_client: &SccpLightClientV1,
    taira_now_ms: u64,
) -> bool {
    weak_subjectivity_deadline_ms(profile, light_client) <= taira_now_ms
}

/// Supersession of an aged light client's newest set, recorded when a fresh bootstrap
/// re-initializes it without a purge: the set's period ended.
pub(super) fn aged_supersessions<V: SccpLcStateView + ?Sized>(
    profile: &TronChainProfileV1,
    view: &V,
    light_client: &SccpLightClientV1,
) -> Result<Vec<SccpLcSupersessionV1>, SccpLcError> {
    let period = light_client.head.latest_set_id;
    if !view
        .consensus_set(NETWORK, period)
        .is_some_and(|set| set.is_current())
    {
        return Ok(Vec::new());
    }
    Ok(vec![SccpLcSupersessionV1 {
        set_id: period,
        superseded_at_source_ms: profile.period_end_ms(period).ok_or(TronLcError::Overflow)?,
    }])
}

fn saturating_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn bytes_len(lengths: impl Iterator<Item = usize>) -> u64 {
    lengths
        .map(|len| u64::try_from(len).unwrap_or(u64::MAX))
        .fold(0, u64::saturating_add)
}

fn segment_signed_work(segment: &TronSegmentV1) -> SccpVerifierWorkV1 {
    SccpVerifierWorkV1 {
        native_headers: saturating_u32(segment.headers.len()),
        native_header_bytes: bytes_len(segment.headers.iter().map(|header| header.raw_data.len())),
        secp256k1_recoveries: saturating_u32(segment.headers.len()),
        ..SccpVerifierWorkV1::default()
    }
}

fn raw_work(segment: &TronRawSegmentV1) -> SccpVerifierWorkV1 {
    SccpVerifierWorkV1 {
        native_headers: saturating_u32(segment.headers.len()),
        native_header_bytes: bytes_len(segment.headers.iter().map(Vec::len)),
        ..SccpVerifierWorkV1::default()
    }
}

/// Work of an advance: every header hashed and recovered.
pub(super) fn advance_work(advance: &TronLcAdvanceV1) -> SccpVerifierWorkV1 {
    advance
        .segments
        .iter()
        .map(segment_signed_work)
        .fold(SccpVerifierWorkV1::default(), |total, work| {
            total.checked_add(&work).unwrap_or(total)
        })
}

/// Work of a backfill: its headers.
pub(super) fn segment_work(segment: &TronRawSegmentV1) -> SccpVerifierWorkV1 {
    raw_work(segment)
}

/// Work of a proof: one proof and its headers.
pub(super) fn proof_work(proof: &TronSourceProofV1) -> SccpVerifierWorkV1 {
    let headers = match &proof.anchor {
        TronProofAnchorV1::Solid(segment) => segment_signed_work(segment),
        TronProofAnchorV1::Checkpoint(segment) => raw_work(segment),
    };
    SccpVerifierWorkV1 {
        proofs: 1,
        ..headers
    }
}

/// Work of one evidence record: its signed segment.
pub(super) fn evidence_work(evidence: &TronLcEvidenceV1) -> SccpVerifierWorkV1 {
    segment_signed_work(&evidence.segment)
}

/// Height, id, parent id, witness and time (ms) of a `raw_data` header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TronHeaderSummaryV1 {
    /// Height.
    pub number: u64,
    /// Block id.
    pub id: [u8; 32],
    /// Parent block id.
    pub parent_id: [u8; 32],
    /// Producing witness account.
    pub witness: [u8; 21],
    /// Time (ms).
    pub time_ms: u64,
    /// `txTrieRoot`.
    pub tx_root: [u8; 32],
}

/// Summarize a `raw_data` header (builders use it to check endpoints).
///
/// # Errors
///
/// Returns [`TronLcError::MalformedHeader`].
pub fn header_summary(raw: &[u8]) -> Result<TronHeaderSummaryV1, TronLcError> {
    let header = decode_header(raw)?;
    Ok(TronHeaderSummaryV1 {
        number: header.number,
        id: header.id,
        parent_id: header.parent_id,
        witness: header.witness,
        time_ms: header.time_ms,
        tx_root: header.tx_root,
    })
}

/// The 21-byte address a header's witness signature recovers to.
///
/// # Errors
///
/// Returns [`TronLcError::MalformedHeader`] or [`TronLcError::MalformedSignature`].
pub fn header_signer(raw: &[u8], signature: &[u8]) -> Result<[u8; 21], TronLcError> {
    recover_signer(&decode_header(raw)?.raw_hash, signature)
}

/// The promote-odd SHA-256 Merkle root of `leaves` and the branch of `index` (builders).
#[must_use]
pub fn merkle_root_and_branch(
    leaves: &[[u8; 32]],
    index: usize,
) -> Option<([u8; 32], Vec<Vec<u8>>)> {
    if leaves.is_empty() || index >= leaves.len() {
        return None;
    }
    let mut level = leaves.to_vec();
    let mut position = index;
    let mut branch = Vec::new();
    while level.len() > 1 {
        let sibling = position ^ 1;
        if sibling < level.len() {
            branch.push(level[sibling].to_vec());
        }
        level = level
            .chunks(2)
            .map(|pair| match pair {
                [left, right] => merkle_node(left, right),
                _ => pair[0],
            })
            .collect();
        position >>= 1;
    }
    Some((level[0], branch))
}

#[cfg(test)]
mod tests;
