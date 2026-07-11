//! Native TRON `DPoS` header-finality verification for SCCP.
//!
//! TRON witnesses sign only `BlockHeader.raw_data`.  There is no native quorum
//! signature over a target block and there is no witness-set handover seal.
//! This module therefore replays the protocol's scheduled-producer and
//! solid-height state machine from an exact governed checkpoint.  Proofs that
//! reach a maintenance boundary fail closed because block headers do not commit
//! the post-maintenance active-witness roster or witness permission mapping.

use alloc::{collections::BTreeSet, vec::Vec};

use iroha_crypto::EcdsaSecp256k1Sha256;
use iroha_data_model::bridge::sccp::{
    SccpLaneIdV1, SccpNetworkV1, SccpSourceEmitterV1, SccpSourceIdentityV1,
};
use sha2::{Digest, Sha256};

use super::{
    H256, SCCP_CODEC_TRON_ADDRESS21, SccpPayloadV1, canonical_sccp_payload_bytes, keccak256_bytes,
    payload_hash, prefixed_blake2b, read_protobuf_varint_at, sccp_lane_id_hash_v1,
    sccp_lane_source_event_digest_v1, sccp_message_id, sccp_source_identity_hash_v1,
    tron_recoverable_signature_for_recovery, verify_sccp_payload_structure,
};

const TRON_NATIVE_ANCHOR_PREFIX_V1: &[u8] = b"sccp:tron:native-dpos-anchor:v1";
const TRON_TRIGGER_SMART_CONTRACT_TYPE_URL_V1: &[u8] =
    b"type.googleapis.com/protocol.TriggerSmartContract";
const TRON_NATIVE_TRANSFER_CALL_ABI_V1: &[u8] = b"transferToTaira(bytes,uint256)";
const TRON_TAIRA_TO_TOKEN_SCALE_V1: u64 = 1_000_000_000;
const TRON_ADDRESS_BYTES: usize = 21;
const TRON_SIGNATURE_BYTES: usize = 65;
const TRON_BLOCK_INTERVAL_MS: u64 = 3_000;
const TRON_MAINTENANCE_SKIP_SLOTS: u32 = 2;
const TRON_SINGLE_REPEAT: u32 = 1;
const TRON_SOLIDIFIED_THRESHOLD_PERCENT: u8 = 70;
const TRON_ACTIVE_WITNESS_COUNT: usize = 27;
const TRON_MAX_RAW_HEADER_BYTES: usize = 16 * 1024;
const TRON_MAX_TRANSACTION_BYTES: usize = 512 * 1024;
const TRON_MAX_TRANSACTION_SIGNATURES: usize = 32;
const TRON_MAX_TRANSACTION_MERKLE_DEPTH: usize = 64;

/// Maximum post-anchor headers before the selected TRON target.
///
/// V1 requires a governed checkpoint no more than one complete 27-witness
/// scheduling round before the target.
pub const TRON_NATIVE_MAX_TARGET_HEADERS: usize = TRON_ACTIVE_WITNESS_COUNT;
/// Maximum headers after a TRON target before it becomes solid.
///
/// One complete active-witness round is enough to include all 27 producers in
/// a healthy schedule and therefore the required 19 distinct producers for
/// the native 70% solid-height order statistic.
pub const TRON_NATIVE_MAX_FINALITY_SUFFIX_HEADERS: usize = TRON_ACTIVE_WITNESS_COUNT;
/// Maximum headers in one canonical native TRON finality continuation.
pub const TRON_NATIVE_MAX_FINALITY_HEADERS: usize =
    TRON_NATIVE_MAX_TARGET_HEADERS + TRON_NATIVE_MAX_FINALITY_SUFFIX_HEADERS;

fn sha256_bytes(payload: &[u8]) -> H256 {
    Sha256::digest(payload).into()
}

/// Build the exact concrete TRON-to-SORA transfer call admitted by V1.
pub fn canonical_tron_native_transfer_call_data(
    sora_recipient: &[u8],
    taira_amount: u128,
) -> Option<Vec<u8>> {
    if sora_recipient.is_empty() || sora_recipient.len() > 256 || taira_amount == 0 {
        return None;
    }
    let padded_len = sora_recipient.len().checked_add(31)? & !31;
    let selector = keccak256_bytes(TRON_NATIVE_TRANSFER_CALL_ABI_V1);
    let mut out = Vec::with_capacity(4 + 96 + padded_len);
    out.extend_from_slice(&selector[..4]);
    out.extend_from_slice(&[0; 31]);
    out.push(64);
    out.extend_from_slice(&scaled_tron_token_amount_word(taira_amount));
    out.extend_from_slice(&[0; 24]);
    out.extend_from_slice(&u64::try_from(sora_recipient.len()).ok()?.to_be_bytes());
    out.extend_from_slice(sora_recipient);
    out.resize(4 + 96 + padded_len, 0);
    Some(out)
}

fn scaled_tron_token_amount_word(taira_amount: u128) -> [u8; 32] {
    let mut word = [0u8; 32];
    let mut carry = 0u64;
    for (index, byte) in taira_amount.to_be_bytes().iter().copied().enumerate().rev() {
        let product = u64::from(byte) * TRON_TAIRA_TO_TOKEN_SCALE_V1 + carry;
        word[16 + index] = u8::try_from(product & 0xff).expect("masked token amount byte");
        carry = product >> 8;
    }
    for byte in word[..16].iter_mut().rev() {
        *byte = u8::try_from(carry & 0xff).expect("masked token amount carry byte");
        carry >>= 8;
    }
    debug_assert_eq!(carry, 0);
    word
}

/// One active TRON super representative at a governed checkpoint.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
pub struct TronNativeWitnessV1 {
    /// Canonical TRON account address, including the `0x41` network prefix.
    #[norito(with = "crate::json_utils::bytes_hex")]
    pub account_address: Vec<u8>,
    /// Canonical address recovered from the account's active witness permission.
    #[norito(with = "crate::json_utils::bytes_hex")]
    pub signing_address: Vec<u8>,
    /// Latest block number produced by this witness at the checkpoint.
    #[norito(with = "crate::json_utils::u64_string")]
    pub latest_block_number: u64,
}

/// Consensus state required to continue native TRON `DPoS` verification.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
pub struct TronNativeDposAnchorV1 {
    /// Anchor schema version.  The first release accepts exactly `1`.
    pub version: u8,
    /// Exact TRON network profile to which this state belongs.
    pub network: SccpNetworkV1,
    /// Number of the checkpoint block whose post-state is represented.
    #[norito(with = "crate::json_utils::u64_string")]
    pub block_number: u64,
    /// Native TRON block id of the checkpoint block.
    #[norito(with = "crate::json_utils::hex32")]
    pub block_id: H256,
    /// Checkpoint block timestamp in milliseconds.
    #[norito(with = "crate::json_utils::u64_string")]
    pub timestamp_ms: u64,
    /// Network genesis block timestamp used by the `DPoS` absolute-slot rule.
    #[norito(with = "crate::json_utils::u64_string")]
    pub genesis_timestamp_ms: u64,
    /// First scheduled maintenance timestamp after the checkpoint.
    #[norito(with = "crate::json_utils::u64_string")]
    pub next_maintenance_time_ms: u64,
    /// Governed network maintenance interval in milliseconds.
    #[norito(with = "crate::json_utils::u64_string")]
    pub maintenance_interval_ms: u64,
    /// Protocol maintenance slots skipped after a maintenance block.
    pub maintenance_skip_slots: u32,
    /// Protocol repetition count for each scheduled witness.
    pub single_repeat: u32,
    /// Percentage used by TRON's solid-height order statistic.
    pub solidified_threshold_percent: u8,
    /// Whether the active consensus optimization requires 3-second alignment.
    pub require_aligned_timestamps: bool,
    /// Whether the checkpoint itself performed a maintenance transition.
    ///
    /// Java-TRON skips two wall-clock slots after such a block while advancing
    /// the witness schedule by only one position for the first successor.
    pub anchor_is_maintenance: bool,
    /// Solid block number recomputed from `witnesses` at this checkpoint.
    #[norito(with = "crate::json_utils::u64_string")]
    pub solid_block_number: u64,
    /// Active witnesses in the exact protocol scheduling order.
    pub witnesses: Vec<TronNativeWitnessV1>,
}

/// One native TRON header and its producer signature.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
pub struct TronNativeSignedHeaderV1 {
    /// Exact deterministic protobuf serialization of `BlockHeader.raw`.
    #[norito(with = "crate::json_utils::bytes_hex")]
    pub raw_data: Vec<u8>,
    /// Java-TRON recoverable secp256k1 signature (`r || s || recid`).
    #[norito(with = "crate::json_utils::bytes_hex")]
    pub witness_signature: Vec<u8>,
}

/// Native TRON finality proof continued from a governed `DPoS` checkpoint.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
pub struct TronNativeFinalityProofV1 {
    /// Proof schema version.  The first release accepts exactly `1`.
    pub version: u8,
    /// Full governed checkpoint preimage.
    pub anchor: TronNativeDposAnchorV1,
    /// Consecutive native headers beginning immediately after `anchor`.
    pub headers: Vec<TronNativeSignedHeaderV1>,
    /// Zero-based header containing the SCCP transaction.
    pub target_header_index: u16,
}

/// Cheap deterministic reservation for native TRON finality verification.
///
/// The estimate uses proof framing only and performs no protobuf parsing,
/// hashing, or secp256k1 recovery, so Core can reserve consensus work before
/// dispatching cryptographic verification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TronNativeFinalityWorkEstimateV1 {
    /// Number of post-anchor continuation headers.
    pub continuation_headers: u16,
    /// Bytes in all raw headers and their witness signatures.
    pub framed_header_bytes: u32,
    /// Maximum secp256k1 witness-key recoveries, one per continuation header.
    pub secp256k1_recoveries: u16,
}

/// Inclusion proof for one full native TRON transaction protobuf.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
pub struct TronNativeTransactionProofV1 {
    /// Zero-based transaction position in the native block transaction list.
    #[norito(with = "crate::json_utils::u64_string")]
    pub transaction_index: u64,
    /// Total number of transactions committed by the block.
    #[norito(with = "crate::json_utils::u64_string")]
    pub transaction_count: u64,
    /// Exact full serialized `protocol.Transaction`, including `ret` fields.
    #[norito(with = "crate::json_utils::bytes_hex")]
    pub transaction_bytes: Vec<u8>,
    /// Consumed native Merkle siblings, bottom-up; odd final nodes are promoted.
    #[norito(with = "crate::json_utils::vec_bytes_hex")]
    pub merkle_branch: Vec<Vec<u8>>,
}

/// Complete native TRON SCCP source proof.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
pub struct TronNativeSourceProofV1 {
    /// Native `DPoS` continuation that makes the transaction block solid.
    pub finality: TronNativeFinalityProofV1,
    /// Full successful transaction and native Merkle inclusion proof.
    pub transaction: TronNativeTransactionProofV1,
}

/// Authenticated native fields for the solid SCCP transaction block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValidatedTronNativeFinalityV1 {
    /// Domain-separated canonical governed-anchor hash.
    pub anchor_hash: H256,
    /// Target block number.
    pub block_number: u64,
    /// Target native block id.
    pub block_id: H256,
    /// Target transaction Merkle root.
    pub transaction_root: H256,
    /// Optional account-state root advertised by the target header.
    pub account_state_root: Option<H256>,
    /// Account address of the target block producer.
    pub witness_address: [u8; TRON_ADDRESS_BYTES],
    /// Native solid height after applying every supplied header.
    pub resulting_solid_block_number: u64,
}

/// Authenticated native TRON SCCP transaction fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValidatedTronNativeTransactionV1 {
    /// SHA-256 leaf hash of the complete serialized transaction.
    pub transaction_hash: H256,
    /// Triggering account address, including the `0x41` prefix.
    pub caller_address: [u8; TRON_ADDRESS_BYTES],
    /// Governed source contract address, including the `0x41` prefix.
    pub contract_address: [u8; TRON_ADDRESS_BYTES],
    /// Exact canonical typed-lane hash carried by the source call.
    pub lane_hash: H256,
    /// Exact lane-bound message identifier derived from the transfer call.
    pub message_id: H256,
    /// Hash of the canonical transfer payload derived from the call.
    pub payload_hash: H256,
    /// Exact SCCP source event digest carried by the call data.
    pub source_event_digest: H256,
    /// Governed immutable transfer-route configuration.
    pub route_config_hash: H256,
}

/// How TRON source deployment identity is authenticated in the first release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TronSourceDeploymentAuthenticationV1 {
    /// Runtime code and immutable route configuration are governed identity inputs.
    ///
    /// TRON's header `accountStateRoot` excludes smart-contract bytecode,
    /// storage or permission records. Native headers therefore cannot
    /// independently prove those fields without a full execution light client.
    GovernedIdentity,
}

/// Authenticated result of a complete native TRON SCCP source proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValidatedTronNativeSourceV1 {
    /// Exact governed typed source-identity hash.
    pub source_identity_hash: H256,
    /// Native target finality result.
    pub finality: ValidatedTronNativeFinalityV1,
    /// Native successful transaction result.
    pub transaction: ValidatedTronNativeTransactionV1,
    /// Explicit deployment authentication model for this chain.
    pub deployment_authentication: TronSourceDeploymentAuthenticationV1,
}

/// Fail-closed reason returned by native TRON finality verification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TronNativeFinalityError {
    /// The proof or checkpoint schema version is unsupported.
    UnsupportedVersion,
    /// The checkpoint names a non-TRON network.
    WrongNetwork,
    /// The governed anchor hash does not match the canonical checkpoint.
    AnchorHashMismatch,
    /// Static or stateful checkpoint fields are malformed or inconsistent.
    InvalidAnchor,
    /// The proof contains no headers, too many headers, or a bad target index.
    InvalidProofShape,
    /// A raw header protobuf is noncanonical, incomplete, duplicated, or unknown.
    InvalidHeaderEncoding,
    /// A header does not continue the checkpoint's native parent/number chain.
    HeaderChainMismatch,
    /// A header timestamp violates the native absolute-slot rules.
    InvalidTimestamp,
    /// Verifying the sequence would cross an unauthenticated maintenance update.
    MaintenanceBoundary,
    /// The scheduled witness does not match the header producer.
    WrongScheduledWitness,
    /// The native producer signature is malformed or resolves to the wrong key.
    InvalidWitnessSignature,
    /// The supplied continuation does not make the target block solid.
    TargetNotSolid,
    /// The target became solid before the last supplied continuation header.
    NonMinimalContinuation,
}

/// Fail-closed reason returned by native TRON transaction verification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TronNativeTransactionError {
    /// The proof shape, transaction count, index, or byte bounds are invalid.
    InvalidProofShape,
    /// The protobuf transaction or one of its nested messages is noncanonical.
    InvalidTransactionEncoding,
    /// The authenticated transaction result is absent or not `SUCCESS`.
    TransactionFailed,
    /// The transaction does not call the exact governed source contract.
    WrongContract,
    /// The successful call sender differed from the canonical payload sender.
    WrongCaller,
    /// The `TriggerSmartContract` call does not carry the exact SCCP event payload.
    WrongCallData,
    /// The native transaction Merkle branch does not reconstruct the header root.
    InvalidMerkleProof,
}

/// Fail-closed reason returned by complete native TRON source verification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TronNativeSourceError {
    /// The typed identity is malformed, belongs to another family, or names another lane.
    InvalidSourceIdentity,
    /// The canonical typed identity does not match the governed registry hash.
    SourceIdentityHashMismatch,
    /// Native `DPoS` finality verification failed.
    Finality(TronNativeFinalityError),
    /// Native transaction decoding or inclusion verification failed.
    Transaction(TronNativeTransactionError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ParsedTronRawHeaderV1 {
    number: u64,
    timestamp_ms: u64,
    transaction_root: Option<H256>,
    parent_block_id: H256,
    _witness_id: Option<u64>,
    witness_address: [u8; TRON_ADDRESS_BYTES],
    _header_version: u32,
    account_state_root: Option<H256>,
}

fn tron_network_tag(network: SccpNetworkV1) -> Option<u8> {
    match network {
        SccpNetworkV1::TronMainnet => Some(0),
        SccpNetworkV1::TronNile => Some(1),
        SccpNetworkV1::TronShasta => Some(2),
        _ => None,
    }
}

fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

/// Return canonical bytes for a governed native TRON `DPoS` checkpoint.
pub fn canonical_tron_native_anchor_bytes(anchor: &TronNativeDposAnchorV1) -> Option<Vec<u8>> {
    validate_anchor(anchor)?;
    let mut out =
        Vec::with_capacity(128usize.checked_add(anchor.witnesses.len().checked_mul(50)?)?);
    out.push(anchor.version);
    out.push(tron_network_tag(anchor.network)?);
    push_u64(&mut out, anchor.block_number);
    out.extend_from_slice(&anchor.block_id);
    push_u64(&mut out, anchor.timestamp_ms);
    push_u64(&mut out, anchor.genesis_timestamp_ms);
    push_u64(&mut out, anchor.next_maintenance_time_ms);
    push_u64(&mut out, anchor.maintenance_interval_ms);
    push_u32(&mut out, anchor.maintenance_skip_slots);
    push_u32(&mut out, anchor.single_repeat);
    out.push(anchor.solidified_threshold_percent);
    out.push(u8::from(anchor.require_aligned_timestamps));
    out.push(u8::from(anchor.anchor_is_maintenance));
    push_u64(&mut out, anchor.solid_block_number);
    push_u32(&mut out, u32::try_from(anchor.witnesses.len()).ok()?);
    for witness in &anchor.witnesses {
        out.extend_from_slice(&witness.account_address);
        out.extend_from_slice(&witness.signing_address);
        push_u64(&mut out, witness.latest_block_number);
    }
    Some(out)
}

/// Hash a canonical governed native TRON `DPoS` checkpoint.
pub fn tron_native_anchor_hash(anchor: &TronNativeDposAnchorV1) -> Option<H256> {
    Some(prefixed_blake2b(
        TRON_NATIVE_ANCHOR_PREFIX_V1,
        &canonical_tron_native_anchor_bytes(anchor)?,
    ))
}

fn is_tron_address(address: &[u8]) -> bool {
    address.len() == TRON_ADDRESS_BYTES
        && address.first() == Some(&0x41)
        && address[1..].iter().any(|byte| *byte != 0)
}

fn block_id_number(block_id: &H256) -> u64 {
    u64::from_be_bytes([
        block_id[0],
        block_id[1],
        block_id[2],
        block_id[3],
        block_id[4],
        block_id[5],
        block_id[6],
        block_id[7],
    ])
}

fn solid_height(latest_block_numbers: &[u64]) -> Option<u64> {
    if latest_block_numbers.len() != TRON_ACTIVE_WITNESS_COUNT {
        return None;
    }
    let mut sorted = latest_block_numbers.to_vec();
    sorted.sort_unstable();
    let position = sorted.len().checked_mul(usize::from(
        100u8.checked_sub(TRON_SOLIDIFIED_THRESHOLD_PERCENT)?,
    ))? / 100;
    sorted.get(position).copied()
}

fn validate_anchor(anchor: &TronNativeDposAnchorV1) -> Option<()> {
    if anchor.version != 1
        || tron_network_tag(anchor.network).is_none()
        || anchor.block_number == 0
        || block_id_number(&anchor.block_id) != anchor.block_number
        || anchor.genesis_timestamp_ms == 0
        || anchor.timestamp_ms <= anchor.genesis_timestamp_ms
        || anchor.next_maintenance_time_ms <= anchor.timestamp_ms
        || (anchor.anchor_is_maintenance
            && anchor.next_maintenance_time_ms
                <= anchor.timestamp_ms.saturating_add(
                    TRON_BLOCK_INTERVAL_MS
                        .saturating_mul(u64::from(anchor.maintenance_skip_slots) + 1),
                ))
        || anchor.maintenance_interval_ms == 0
        || !anchor
            .maintenance_interval_ms
            .is_multiple_of(TRON_BLOCK_INTERVAL_MS)
        || anchor.maintenance_skip_slots != TRON_MAINTENANCE_SKIP_SLOTS
        || anchor.single_repeat != TRON_SINGLE_REPEAT
        || anchor.solidified_threshold_percent != TRON_SOLIDIFIED_THRESHOLD_PERCENT
        || anchor.witnesses.len() != TRON_ACTIVE_WITNESS_COUNT
        || (anchor.require_aligned_timestamps
            && !anchor.timestamp_ms.is_multiple_of(TRON_BLOCK_INTERVAL_MS))
    {
        return None;
    }

    let mut accounts = BTreeSet::<[u8; TRON_ADDRESS_BYTES]>::new();
    let mut signers = BTreeSet::<[u8; TRON_ADDRESS_BYTES]>::new();
    let mut latest = Vec::with_capacity(anchor.witnesses.len());
    for witness in &anchor.witnesses {
        if !is_tron_address(&witness.account_address)
            || !is_tron_address(&witness.signing_address)
            || witness.latest_block_number > anchor.block_number
        {
            return None;
        }
        let account: [u8; TRON_ADDRESS_BYTES] =
            witness.account_address.as_slice().try_into().ok()?;
        let signer: [u8; TRON_ADDRESS_BYTES] =
            witness.signing_address.as_slice().try_into().ok()?;
        if !accounts.insert(account) || !signers.insert(signer) {
            return None;
        }
        latest.push(witness.latest_block_number);
    }
    // A distinct witness must not borrow another active witness's account as a
    // signing identity.  An account may, and normally does, sign for itself.
    for witness in &anchor.witnesses {
        let account: [u8; TRON_ADDRESS_BYTES] =
            witness.account_address.as_slice().try_into().ok()?;
        let signer: [u8; TRON_ADDRESS_BYTES] =
            witness.signing_address.as_slice().try_into().ok()?;
        if account != signer && accounts.contains(&signer) {
            return None;
        }
    }
    (solid_height(&latest)? == anchor.solid_block_number
        && anchor.solid_block_number <= anchor.block_number)
        .then_some(())
}

fn read_bytes_field<'a>(bytes: &'a [u8], cursor: &mut usize) -> Option<&'a [u8]> {
    let len = usize::try_from(read_protobuf_varint_at(bytes, cursor)?).ok()?;
    let end = cursor.checked_add(len)?;
    let value = bytes.get(*cursor..end)?;
    *cursor = end;
    Some(value)
}

fn parse_tron_raw_header(raw_data: &[u8]) -> Option<ParsedTronRawHeaderV1> {
    if raw_data.is_empty() || raw_data.len() > TRON_MAX_RAW_HEADER_BYTES {
        return None;
    }
    let mut cursor = 0usize;
    let mut previous_field = 0u32;
    let mut timestamp_ms = None;
    let mut transaction_root = None;
    let mut parent_block_id = None;
    let mut number = None;
    let mut witness_id = None;
    let mut witness_address = None;
    let mut header_version = None;
    let mut account_state_root = None;
    while cursor < raw_data.len() {
        let key = read_protobuf_varint_at(raw_data, &mut cursor)?;
        let field = u32::try_from(key >> 3).ok()?;
        let wire = u8::try_from(key & 7).ok()?;
        if field <= previous_field {
            return None;
        }
        previous_field = field;
        match (field, wire) {
            (1, 0) => timestamp_ms = Some(read_protobuf_varint_at(raw_data, &mut cursor)?),
            (2, 2) => {
                transaction_root = Some(read_bytes_field(raw_data, &mut cursor)?.try_into().ok()?)
            }
            (3, 2) => {
                parent_block_id = Some(read_bytes_field(raw_data, &mut cursor)?.try_into().ok()?)
            }
            (7, 0) => number = Some(read_protobuf_varint_at(raw_data, &mut cursor)?),
            (8, 0) => {
                let value = read_protobuf_varint_at(raw_data, &mut cursor)?;
                if value == 0 || i64::try_from(value).is_err() {
                    return None;
                }
                witness_id = Some(value);
            }
            (9, 2) => {
                witness_address = Some(read_bytes_field(raw_data, &mut cursor)?.try_into().ok()?)
            }
            (10, 0) => {
                let value = u32::try_from(read_protobuf_varint_at(raw_data, &mut cursor)?).ok()?;
                if value == 0 || i32::try_from(value).is_err() {
                    return None;
                }
                header_version = Some(value);
            }
            (11, 2) => {
                account_state_root = Some(read_bytes_field(raw_data, &mut cursor)?.try_into().ok()?)
            }
            _ => return None,
        }
    }
    let parsed = ParsedTronRawHeaderV1 {
        number: number?,
        timestamp_ms: timestamp_ms?,
        transaction_root,
        parent_block_id: parent_block_id?,
        _witness_id: witness_id,
        witness_address: witness_address?,
        _header_version: header_version.unwrap_or(0),
        account_state_root,
    };
    (parsed.number != 0
        && i64::try_from(parsed.number).is_ok()
        && parsed.timestamp_ms != 0
        && i64::try_from(parsed.timestamp_ms).is_ok()
        && parsed.parent_block_id.iter().any(|byte| *byte != 0)
        && is_tron_address(&parsed.witness_address)
        && parsed
            .transaction_root
            .is_none_or(|root| root.iter().any(|byte| *byte != 0))
        && parsed
            .account_state_root
            .is_none_or(|root| root.iter().any(|byte| *byte != 0)))
    .then_some(parsed)
}

fn tron_block_id(number: u64, raw_hash: H256) -> H256 {
    let mut block_id = raw_hash;
    block_id[..8].copy_from_slice(&number.to_be_bytes());
    block_id
}

fn recover_tron_address(raw_hash: H256, signature: &[u8]) -> Option<[u8; TRON_ADDRESS_BYTES]> {
    let signature: [u8; TRON_SIGNATURE_BYTES] = signature.try_into().ok()?;
    let normalized = tron_recoverable_signature_for_recovery(&signature)?;
    let public_key =
        EcdsaSecp256k1Sha256::recover_public_key_from_prehash(&raw_hash, &normalized).ok()?;
    let evm = EcdsaSecp256k1Sha256::evm_address(&public_key);
    let mut address = [0u8; TRON_ADDRESS_BYTES];
    address[0] = 0x41;
    address[1..].copy_from_slice(&evm);
    Some(address)
}

#[derive(Clone, Copy)]
struct TronNativeFinalityTargetV1 {
    header: ParsedTronRawHeaderV1,
    block_id: H256,
    transaction_root: H256,
}

struct TronNativeFinalityReplayV1 {
    expected_parent: H256,
    previous_number: u64,
    previous_timestamp: u64,
    previous_slot: u64,
    latest_block_numbers: Vec<u64>,
    target: Option<TronNativeFinalityTargetV1>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct TronNativeFinalityWorkPerformedV1 {
    continuation_headers: u16,
    secp256k1_recoveries: u16,
}

impl TronNativeFinalityReplayV1 {
    fn from_anchor(anchor: &TronNativeDposAnchorV1) -> Result<Self, TronNativeFinalityError> {
        let previous_slot = anchor
            .timestamp_ms
            .checked_sub(anchor.genesis_timestamp_ms)
            .ok_or(TronNativeFinalityError::InvalidAnchor)?
            / TRON_BLOCK_INTERVAL_MS;
        Ok(Self {
            expected_parent: anchor.block_id,
            previous_number: anchor.block_number,
            previous_timestamp: anchor.timestamp_ms,
            previous_slot,
            latest_block_numbers: anchor
                .witnesses
                .iter()
                .map(|witness| witness.latest_block_number)
                .collect(),
            target: None,
        })
    }
}

fn tron_native_header_schedule(
    anchor: &TronNativeDposAnchorV1,
    header: &ParsedTronRawHeaderV1,
    header_index: usize,
    replay: &TronNativeFinalityReplayV1,
) -> Result<(u64, usize), TronNativeFinalityError> {
    if header.timestamp_ms <= replay.previous_timestamp
        || header.timestamp_ms <= anchor.genesis_timestamp_ms
        || (anchor.require_aligned_timestamps
            && !header.timestamp_ms.is_multiple_of(TRON_BLOCK_INTERVAL_MS))
    {
        return Err(TronNativeFinalityError::InvalidTimestamp);
    }
    if header.timestamp_ms >= anchor.next_maintenance_time_ms {
        return Err(TronNativeFinalityError::MaintenanceBoundary);
    }
    let absolute_slot = header
        .timestamp_ms
        .checked_sub(anchor.genesis_timestamp_ms)
        .ok_or(TronNativeFinalityError::InvalidTimestamp)?
        / TRON_BLOCK_INTERVAL_MS;
    if absolute_slot <= replay.previous_slot {
        return Err(TronNativeFinalityError::InvalidTimestamp);
    }
    let schedule_slot = if header_index == 0 && anchor.anchor_is_maintenance {
        let first_slot_time = replay
            .previous_timestamp
            .checked_sub(replay.previous_timestamp % TRON_BLOCK_INTERVAL_MS)
            .and_then(|aligned| {
                aligned.checked_add(
                    TRON_BLOCK_INTERVAL_MS
                        .checked_mul(u64::from(anchor.maintenance_skip_slots).checked_add(1)?)?,
                )
            })
            .ok_or(TronNativeFinalityError::InvalidTimestamp)?;
        if header.timestamp_ms < first_slot_time {
            return Err(TronNativeFinalityError::InvalidTimestamp);
        }
        let relative_slot = header
            .timestamp_ms
            .checked_sub(first_slot_time)
            .ok_or(TronNativeFinalityError::InvalidTimestamp)?
            / TRON_BLOCK_INTERVAL_MS;
        replay
            .previous_slot
            .checked_add(relative_slot)
            .and_then(|slot| slot.checked_add(1))
            .ok_or(TronNativeFinalityError::InvalidTimestamp)?
    } else {
        absolute_slot
    };
    let single_repeat = usize::try_from(anchor.single_repeat)
        .map_err(|_| TronNativeFinalityError::InvalidAnchor)?;
    let schedule_span = anchor
        .witnesses
        .len()
        .checked_mul(single_repeat)
        .ok_or(TronNativeFinalityError::InvalidAnchor)?;
    let scheduled_position = usize::try_from(
        schedule_slot
            % u64::try_from(schedule_span).map_err(|_| TronNativeFinalityError::InvalidAnchor)?,
    )
    .map_err(|_| TronNativeFinalityError::InvalidTimestamp)?
        / single_repeat;
    Ok((absolute_slot, scheduled_position))
}

fn replay_tron_native_header(
    anchor: &TronNativeDposAnchorV1,
    signed_header: &TronNativeSignedHeaderV1,
    header_index: usize,
    target_index: usize,
    replay: &mut TronNativeFinalityReplayV1,
) -> Result<(), TronNativeFinalityError> {
    let header = parse_tron_raw_header(&signed_header.raw_data)
        .ok_or(TronNativeFinalityError::InvalidHeaderEncoding)?;
    let raw_hash = sha256_bytes(&signed_header.raw_data);
    let block_id = tron_block_id(header.number, raw_hash);
    if header.number
        != replay
            .previous_number
            .checked_add(1)
            .ok_or(TronNativeFinalityError::HeaderChainMismatch)?
        || header.parent_block_id != replay.expected_parent
    {
        return Err(TronNativeFinalityError::HeaderChainMismatch);
    }
    let (absolute_slot, scheduled_position) =
        tron_native_header_schedule(anchor, &header, header_index, replay)?;
    let scheduled = anchor
        .witnesses
        .get(scheduled_position)
        .ok_or(TronNativeFinalityError::InvalidAnchor)?;
    if scheduled.account_address.as_slice() != header.witness_address {
        return Err(TronNativeFinalityError::WrongScheduledWitness);
    }
    let expected_signer: [u8; TRON_ADDRESS_BYTES] = scheduled
        .signing_address
        .as_slice()
        .try_into()
        .map_err(|_| TronNativeFinalityError::InvalidAnchor)?;
    if recover_tron_address(raw_hash, &signed_header.witness_signature) != Some(expected_signer) {
        return Err(TronNativeFinalityError::InvalidWitnessSignature);
    }
    *replay
        .latest_block_numbers
        .get_mut(scheduled_position)
        .ok_or(TronNativeFinalityError::InvalidAnchor)? = header.number;
    if header_index == target_index {
        let transaction_root = header
            .transaction_root
            .ok_or(TronNativeFinalityError::InvalidHeaderEncoding)?;
        replay.target = Some(TronNativeFinalityTargetV1 {
            header,
            block_id,
            transaction_root,
        });
    }
    replay.expected_parent = block_id;
    replay.previous_number = header.number;
    replay.previous_timestamp = header.timestamp_ms;
    replay.previous_slot = absolute_slot;
    Ok(())
}

/// Return a cryptography-free upper bound for one native TRON finality proof.
///
/// The shape policy admits one complete witness round before the target and
/// one complete witness round after it.  This function reads only vector and
/// byte lengths, so callers can reserve work before parsing or recovery.
///
/// # Errors
///
/// Returns [`TronNativeFinalityError::InvalidProofShape`] when the proof falls
/// outside the V1 target, suffix, per-header, or aggregate byte bounds.
pub fn tron_native_finality_work_estimate(
    proof: &TronNativeFinalityProofV1,
) -> Result<TronNativeFinalityWorkEstimateV1, TronNativeFinalityError> {
    let header_count = proof.headers.len();
    let target_index = usize::from(proof.target_header_index);
    if header_count == 0
        || header_count > TRON_NATIVE_MAX_FINALITY_HEADERS
        || target_index >= header_count
    {
        return Err(TronNativeFinalityError::InvalidProofShape);
    }
    let suffix_headers = header_count
        .checked_sub(target_index)
        .and_then(|count| count.checked_sub(1))
        .ok_or(TronNativeFinalityError::InvalidProofShape)?;
    if target_index >= TRON_NATIVE_MAX_TARGET_HEADERS
        || suffix_headers > TRON_NATIVE_MAX_FINALITY_SUFFIX_HEADERS
        || proof.headers.iter().any(|header| {
            header.raw_data.len() > TRON_MAX_RAW_HEADER_BYTES
                || header.witness_signature.len() != TRON_SIGNATURE_BYTES
        })
    {
        return Err(TronNativeFinalityError::InvalidProofShape);
    }
    let framed_header_bytes = proof
        .headers
        .iter()
        .try_fold(0_usize, |total, header| {
            total
                .checked_add(header.raw_data.len())?
                .checked_add(header.witness_signature.len())
        })
        .and_then(|total| u32::try_from(total).ok())
        .ok_or(TronNativeFinalityError::InvalidProofShape)?;
    let continuation_headers =
        u16::try_from(header_count).map_err(|_| TronNativeFinalityError::InvalidProofShape)?;
    Ok(TronNativeFinalityWorkEstimateV1 {
        continuation_headers,
        framed_header_bytes,
        secp256k1_recoveries: continuation_headers,
    })
}

fn verify_tron_native_finality_counted(
    proof: &TronNativeFinalityProofV1,
    expected_network: SccpNetworkV1,
    expected_anchor_hash: H256,
    work: &mut TronNativeFinalityWorkPerformedV1,
) -> Result<ValidatedTronNativeFinalityV1, TronNativeFinalityError> {
    if proof.version != 1 || proof.anchor.version != 1 {
        return Err(TronNativeFinalityError::UnsupportedVersion);
    }
    if proof.anchor.network != expected_network || tron_network_tag(expected_network).is_none() {
        return Err(TronNativeFinalityError::WrongNetwork);
    }
    let _ = tron_native_finality_work_estimate(proof)?;
    validate_anchor(&proof.anchor).ok_or(TronNativeFinalityError::InvalidAnchor)?;
    let anchor_hash =
        tron_native_anchor_hash(&proof.anchor).ok_or(TronNativeFinalityError::InvalidAnchor)?;
    if anchor_hash != expected_anchor_hash {
        return Err(TronNativeFinalityError::AnchorHashMismatch);
    }
    let target_index = usize::from(proof.target_header_index);
    let mut replay = TronNativeFinalityReplayV1::from_anchor(&proof.anchor)?;
    for (index, signed_header) in proof.headers.iter().enumerate() {
        work.continuation_headers = work.continuation_headers.saturating_add(1);
        work.secp256k1_recoveries = work.secp256k1_recoveries.saturating_add(1);
        replay_tron_native_header(
            &proof.anchor,
            signed_header,
            index,
            target_index,
            &mut replay,
        )?;

        if let Some(target) = replay.target {
            let resulting_solid = solid_height(&replay.latest_block_numbers)
                .ok_or(TronNativeFinalityError::InvalidAnchor)?;
            if resulting_solid >= target.header.number {
                if index + 1 != proof.headers.len() {
                    return Err(TronNativeFinalityError::NonMinimalContinuation);
                }
                return Ok(ValidatedTronNativeFinalityV1 {
                    anchor_hash,
                    block_number: target.header.number,
                    block_id: target.block_id,
                    transaction_root: target.transaction_root,
                    account_state_root: target.header.account_state_root,
                    witness_address: target.header.witness_address,
                    resulting_solid_block_number: resulting_solid,
                });
            }
        }
    }
    Err(TronNativeFinalityError::TargetNotSolid)
}

/// Verify a native TRON `DPoS` continuation and return the authenticated target.
///
/// # Errors
///
/// Returns an error if the checkpoint is malformed or mismatched, a header is
/// noncanonical or violates native scheduling, its producer signature is
/// invalid, or the supplied continuation does not solidify the target block.
pub fn verify_tron_native_finality(
    proof: &TronNativeFinalityProofV1,
    expected_network: SccpNetworkV1,
    expected_anchor_hash: H256,
) -> Result<ValidatedTronNativeFinalityV1, TronNativeFinalityError> {
    verify_tron_native_finality_counted(
        proof,
        expected_network,
        expected_anchor_hash,
        &mut TronNativeFinalityWorkPerformedV1::default(),
    )
}

fn transaction_encoding_error() -> TronNativeTransactionError {
    TronNativeTransactionError::InvalidTransactionEncoding
}

fn read_transaction_bytes_field<'a>(
    bytes: &'a [u8],
    cursor: &mut usize,
) -> Result<&'a [u8], TronNativeTransactionError> {
    read_bytes_field(bytes, cursor).ok_or_else(transaction_encoding_error)
}

fn read_transaction_varint(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<u64, TronNativeTransactionError> {
    read_protobuf_varint_at(bytes, cursor).ok_or_else(transaction_encoding_error)
}

fn transaction_field_key(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<(u32, u8), TronNativeTransactionError> {
    let key = read_transaction_varint(bytes, cursor)?;
    let field = u32::try_from(key >> 3).map_err(|_| transaction_encoding_error())?;
    let wire = u8::try_from(key & 7).map_err(|_| transaction_encoding_error())?;
    if field == 0 {
        return Err(transaction_encoding_error());
    }
    Ok((field, wire))
}

fn parse_tron_any(bytes: &[u8]) -> Result<&[u8], TronNativeTransactionError> {
    let mut cursor = 0usize;
    let mut previous = 0u32;
    let mut type_url = None;
    let mut value = None;
    while cursor < bytes.len() {
        let (field, wire) = transaction_field_key(bytes, &mut cursor)?;
        if field <= previous {
            return Err(transaction_encoding_error());
        }
        previous = field;
        match (field, wire) {
            (1, 2) => type_url = Some(read_transaction_bytes_field(bytes, &mut cursor)?),
            (2, 2) => value = Some(read_transaction_bytes_field(bytes, &mut cursor)?),
            _ => return Err(transaction_encoding_error()),
        }
    }
    if type_url != Some(TRON_TRIGGER_SMART_CONTRACT_TYPE_URL_V1) {
        return Err(transaction_encoding_error());
    }
    value.ok_or_else(transaction_encoding_error)
}

fn parse_tron_trigger_sccp_call(
    bytes: &[u8],
    expected_contract: [u8; 20],
    expected_sender: [u8; TRON_ADDRESS_BYTES],
    expected_recipient: &[u8],
    expected_amount: u128,
) -> Result<([u8; TRON_ADDRESS_BYTES], [u8; TRON_ADDRESS_BYTES]), TronNativeTransactionError> {
    let mut cursor = 0usize;
    let mut previous = 0u32;
    let mut owner = None;
    let mut contract = None;
    let mut data = None;
    while cursor < bytes.len() {
        let (field, wire) = transaction_field_key(bytes, &mut cursor)?;
        if field <= previous {
            return Err(transaction_encoding_error());
        }
        previous = field;
        match (field, wire) {
            (1, 2) => {
                owner = Some(
                    read_transaction_bytes_field(bytes, &mut cursor)?
                        .try_into()
                        .map_err(|_| transaction_encoding_error())?,
                );
            }
            (2, 2) => {
                contract = Some(
                    read_transaction_bytes_field(bytes, &mut cursor)?
                        .try_into()
                        .map_err(|_| transaction_encoding_error())?,
                );
            }
            (4, 2) => data = Some(read_transaction_bytes_field(bytes, &mut cursor)?),
            // Proto3 omits zero call/token values.  Non-zero values change the
            // source-call semantics, so every explicit numeric field fails.
            (3 | 5 | 6, 0) => return Err(TronNativeTransactionError::WrongCallData),
            _ => return Err(transaction_encoding_error()),
        }
    }
    let owner: [u8; TRON_ADDRESS_BYTES] = owner.ok_or_else(transaction_encoding_error)?;
    let contract: [u8; TRON_ADDRESS_BYTES] = contract.ok_or_else(transaction_encoding_error)?;
    if !is_tron_address(&owner) || !is_tron_address(&contract) || owner == contract {
        return Err(transaction_encoding_error());
    }
    let mut expected_contract_address = [0u8; TRON_ADDRESS_BYTES];
    expected_contract_address[0] = 0x41;
    expected_contract_address[1..].copy_from_slice(&expected_contract);
    if contract != expected_contract_address {
        return Err(TronNativeTransactionError::WrongContract);
    }
    if owner != expected_sender {
        return Err(TronNativeTransactionError::WrongCaller);
    }
    let expected_data =
        canonical_tron_native_transfer_call_data(expected_recipient, expected_amount)
            .ok_or(TronNativeTransactionError::WrongCallData)?;
    if data != Some(expected_data.as_slice()) {
        return Err(TronNativeTransactionError::WrongCallData);
    }
    Ok((owner, contract))
}

fn parse_tron_contract_sccp_call(
    bytes: &[u8],
    expected_contract: [u8; 20],
    expected_sender: [u8; TRON_ADDRESS_BYTES],
    expected_recipient: &[u8],
    expected_amount: u128,
) -> Result<([u8; TRON_ADDRESS_BYTES], [u8; TRON_ADDRESS_BYTES]), TronNativeTransactionError> {
    let mut cursor = 0usize;
    let mut previous = 0u32;
    let mut contract_type = None;
    let mut parameter = None;
    while cursor < bytes.len() {
        let (field, wire) = transaction_field_key(bytes, &mut cursor)?;
        if field <= previous {
            return Err(transaction_encoding_error());
        }
        previous = field;
        match (field, wire) {
            (1, 0) => contract_type = Some(read_transaction_varint(bytes, &mut cursor)?),
            (2, 2) => parameter = Some(read_transaction_bytes_field(bytes, &mut cursor)?),
            (5, 0) => {
                let id = read_transaction_varint(bytes, &mut cursor)?;
                if id == 0 || i32::try_from(id).is_err() {
                    return Err(transaction_encoding_error());
                }
            }
            _ => return Err(transaction_encoding_error()),
        }
    }
    if contract_type != Some(31) {
        return Err(TronNativeTransactionError::WrongCallData);
    }
    let trigger = parse_tron_any(parameter.ok_or_else(transaction_encoding_error)?)?;
    parse_tron_trigger_sccp_call(
        trigger,
        expected_contract,
        expected_sender,
        expected_recipient,
        expected_amount,
    )
}

fn parse_tron_raw_transaction_sccp_call(
    bytes: &[u8],
    expected_contract: [u8; 20],
    expected_sender: [u8; TRON_ADDRESS_BYTES],
    expected_recipient: &[u8],
    expected_amount: u128,
) -> Result<([u8; TRON_ADDRESS_BYTES], [u8; TRON_ADDRESS_BYTES]), TronNativeTransactionError> {
    let mut cursor = 0usize;
    let mut previous = 0u32;
    let mut ref_block_bytes = None;
    let mut ref_block_num = None;
    let mut ref_block_hash = None;
    let mut expiration = None;
    let mut call = None;
    let mut timestamp = None;
    let mut fee_limit = None;
    while cursor < bytes.len() {
        let (field, wire) = transaction_field_key(bytes, &mut cursor)?;
        if field <= previous {
            return Err(transaction_encoding_error());
        }
        previous = field;
        match (field, wire) {
            (1, 2) => ref_block_bytes = Some(read_transaction_bytes_field(bytes, &mut cursor)?),
            (3, 0) => ref_block_num = Some(read_transaction_varint(bytes, &mut cursor)?),
            (4, 2) => ref_block_hash = Some(read_transaction_bytes_field(bytes, &mut cursor)?),
            (8, 0) => expiration = Some(read_transaction_varint(bytes, &mut cursor)?),
            (11, 2) => {
                call = Some(parse_tron_contract_sccp_call(
                    read_transaction_bytes_field(bytes, &mut cursor)?,
                    expected_contract,
                    expected_sender,
                    expected_recipient,
                    expected_amount,
                )?);
            }
            (14, 0) => timestamp = Some(read_transaction_varint(bytes, &mut cursor)?),
            (18, 0) => fee_limit = Some(read_transaction_varint(bytes, &mut cursor)?),
            _ => return Err(transaction_encoding_error()),
        }
    }
    let ref_block_num = ref_block_num.ok_or_else(transaction_encoding_error)?;
    let expiration = expiration.ok_or_else(transaction_encoding_error)?;
    let timestamp = timestamp.ok_or_else(transaction_encoding_error)?;
    let fee_limit = fee_limit.ok_or_else(transaction_encoding_error)?;
    if ref_block_bytes.is_none_or(|value| value.len() != 2 || value.iter().all(|byte| *byte == 0))
        || ref_block_hash
            .is_none_or(|value| value.len() != 8 || value.iter().all(|byte| *byte == 0))
        || ref_block_num == 0
        || i64::try_from(ref_block_num).is_err()
        || timestamp == 0
        || i64::try_from(timestamp).is_err()
        || expiration <= timestamp
        || i64::try_from(expiration).is_err()
        || fee_limit == 0
        || i64::try_from(fee_limit).is_err()
    {
        return Err(transaction_encoding_error());
    }
    call.ok_or_else(transaction_encoding_error)
}

fn verify_tron_transaction_success(bytes: &[u8]) -> Result<(), TronNativeTransactionError> {
    let mut cursor = 0usize;
    let mut previous = 0u32;
    let mut contract_result = None;
    while cursor < bytes.len() {
        let (field, wire) = transaction_field_key(bytes, &mut cursor)?;
        if field <= previous {
            return Err(transaction_encoding_error());
        }
        previous = field;
        match (field, wire) {
            (1, 0) => {
                let fee = read_transaction_varint(bytes, &mut cursor)?;
                if fee == 0 || i64::try_from(fee).is_err() {
                    return Err(transaction_encoding_error());
                }
            }
            // `SUCESS` is protobuf's zero value and is omitted canonically;
            // any serialized `ret` value is therefore a failure or alias.
            (2, 0) => return Err(TronNativeTransactionError::TransactionFailed),
            (3, 0) => contract_result = Some(read_transaction_varint(bytes, &mut cursor)?),
            _ => return Err(transaction_encoding_error()),
        }
    }
    if contract_result != Some(1) {
        return Err(TronNativeTransactionError::TransactionFailed);
    }
    Ok(())
}

fn parse_full_tron_sccp_transaction(
    bytes: &[u8],
    expected_contract: [u8; 20],
    expected_sender: [u8; TRON_ADDRESS_BYTES],
    expected_recipient: &[u8],
    expected_amount: u128,
) -> Result<([u8; TRON_ADDRESS_BYTES], [u8; TRON_ADDRESS_BYTES]), TronNativeTransactionError> {
    if bytes.is_empty() || bytes.len() > TRON_MAX_TRANSACTION_BYTES {
        return Err(TronNativeTransactionError::InvalidProofShape);
    }
    let mut cursor = 0usize;
    let mut previous = 0u32;
    let mut raw_data = None;
    let mut signature_count = 0usize;
    let mut result_count = 0usize;
    let mut result_success = false;
    while cursor < bytes.len() {
        let (field, wire) = transaction_field_key(bytes, &mut cursor)?;
        if field < previous || (field == previous && !matches!(field, 2 | 5)) {
            return Err(transaction_encoding_error());
        }
        previous = field;
        match (field, wire) {
            (1, 2) if raw_data.is_none() => {
                raw_data = Some(read_transaction_bytes_field(bytes, &mut cursor)?);
            }
            (2, 2) => {
                signature_count = signature_count
                    .checked_add(1)
                    .ok_or(TronNativeTransactionError::InvalidProofShape)?;
                let signature = read_transaction_bytes_field(bytes, &mut cursor)?;
                if signature_count > TRON_MAX_TRANSACTION_SIGNATURES
                    || signature.len() != TRON_SIGNATURE_BYTES
                {
                    return Err(transaction_encoding_error());
                }
            }
            (5, 2) => {
                result_count = result_count
                    .checked_add(1)
                    .ok_or(TronNativeTransactionError::InvalidProofShape)?;
                if result_count != 1 {
                    return Err(transaction_encoding_error());
                }
                verify_tron_transaction_success(read_transaction_bytes_field(bytes, &mut cursor)?)?;
                result_success = true;
            }
            _ => return Err(transaction_encoding_error()),
        }
    }
    if signature_count == 0 || result_count != 1 || !result_success {
        return Err(transaction_encoding_error());
    }
    parse_tron_raw_transaction_sccp_call(
        raw_data.ok_or_else(transaction_encoding_error)?,
        expected_contract,
        expected_sender,
        expected_recipient,
        expected_amount,
    )
}

fn tron_transaction_merkle_node(left: H256, right: H256) -> H256 {
    let mut preimage = [0u8; 64];
    preimage[..32].copy_from_slice(&left);
    preimage[32..].copy_from_slice(&right);
    sha256_bytes(&preimage)
}

fn tron_transaction_merkle_root(
    leaf: H256,
    transaction_index: u64,
    transaction_count: u64,
    branch: &[Vec<u8>],
) -> Option<H256> {
    if transaction_count == 0
        || transaction_index >= transaction_count
        || branch.len() > TRON_MAX_TRANSACTION_MERKLE_DEPTH
        || branch.iter().any(|node| node.len() != 32)
    {
        return None;
    }
    let mut current = leaf;
    let mut index = transaction_index;
    let mut count = transaction_count;
    let mut branch_index = 0usize;
    while count > 1 {
        if index & 1 == 0 {
            if index + 1 < count {
                let sibling: H256 = branch.get(branch_index)?.as_slice().try_into().ok()?;
                branch_index = branch_index.checked_add(1)?;
                current = tron_transaction_merkle_node(current, sibling);
            }
        } else {
            let sibling: H256 = branch.get(branch_index)?.as_slice().try_into().ok()?;
            branch_index = branch_index.checked_add(1)?;
            current = tron_transaction_merkle_node(sibling, current);
        }
        index >>= 1;
        count = count.checked_add(1)?.checked_div(2)?;
    }
    (branch_index == branch.len()).then_some(current)
}

/// Verify a full successful `TriggerSmartContract` transaction and native inclusion.
///
/// # Errors
///
/// Returns an error if the proof shape, transaction protobuf, exact governed
/// call, successful result, or transaction-Merkle inclusion is invalid.
pub fn verify_tron_native_sccp_transaction(
    proof: &TronNativeTransactionProofV1,
    expected_transaction_root: H256,
    expected_contract_address: [u8; 20],
    route_config_hash: H256,
    lane: SccpLaneIdV1,
    payload: &SccpPayloadV1,
) -> Result<ValidatedTronNativeTransactionV1, TronNativeTransactionError> {
    let SccpPayloadV1::Transfer(transfer) = payload;
    let Ok(expected_sender_address): Result<[u8; TRON_ADDRESS_BYTES], _> =
        transfer.sender.as_slice().try_into()
    else {
        return Err(TronNativeTransactionError::InvalidProofShape);
    };
    let canonical_payload = canonical_sccp_payload_bytes(payload)
        .map_err(|_| TronNativeTransactionError::InvalidProofShape)?;
    let expected_lane_hash =
        sccp_lane_id_hash_v1(lane).ok_or(TronNativeTransactionError::InvalidProofShape)?;
    let expected_message_id =
        sccp_message_id(lane, payload).ok_or(TronNativeTransactionError::InvalidProofShape)?;
    let expected_payload_hash = payload_hash(&canonical_payload);
    let source_event_digest =
        sccp_lane_source_event_digest_v1(lane, expected_message_id, expected_payload_hash)
            .ok_or(TronNativeTransactionError::InvalidProofShape)?;
    let hash_roles = [
        expected_lane_hash,
        expected_message_id,
        expected_payload_hash,
        source_event_digest,
        route_config_hash,
    ];
    if expected_transaction_root.iter().all(|byte| *byte == 0)
        || expected_contract_address.iter().all(|byte| *byte == 0)
        || !verify_sccp_payload_structure(payload)
        || !matches!(lane.target, SccpNetworkV1::SoraTaira)
        || !matches!(
            lane.source,
            SccpNetworkV1::TronMainnet | SccpNetworkV1::TronNile | SccpNetworkV1::TronShasta
        )
        || transfer.sender_codec != SCCP_CODEC_TRON_ADDRESS21
        || !is_tron_address(&expected_sender_address)
        || expected_contract_address == expected_sender_address[1..]
        || transfer.recipient.is_empty()
        || transfer.recipient.len() > 256
        || transfer.amount == 0
        || expected_lane_hash.iter().all(|byte| *byte == 0)
        || expected_message_id.iter().all(|byte| *byte == 0)
        || expected_payload_hash.iter().all(|byte| *byte == 0)
        || source_event_digest.iter().all(|byte| *byte == 0)
        || route_config_hash.iter().all(|byte| *byte == 0)
        || hash_roles
            .iter()
            .enumerate()
            .any(|(index, hash)| hash_roles[index + 1..].contains(hash))
        || proof.transaction_count == 0
        || proof.transaction_index >= proof.transaction_count
        || proof.transaction_bytes.is_empty()
        || proof.transaction_bytes.len() > TRON_MAX_TRANSACTION_BYTES
        || proof.merkle_branch.len() > TRON_MAX_TRANSACTION_MERKLE_DEPTH
    {
        return Err(TronNativeTransactionError::InvalidProofShape);
    }
    let (caller_address, contract_address) = parse_full_tron_sccp_transaction(
        &proof.transaction_bytes,
        expected_contract_address,
        expected_sender_address,
        &transfer.recipient,
        transfer.amount,
    )?;
    let transaction_hash = sha256_bytes(&proof.transaction_bytes);
    let reconstructed = tron_transaction_merkle_root(
        transaction_hash,
        proof.transaction_index,
        proof.transaction_count,
        &proof.merkle_branch,
    )
    .ok_or(TronNativeTransactionError::InvalidMerkleProof)?;
    if reconstructed != expected_transaction_root {
        return Err(TronNativeTransactionError::InvalidMerkleProof);
    }
    Ok(ValidatedTronNativeTransactionV1 {
        transaction_hash,
        caller_address,
        contract_address,
        lane_hash: expected_lane_hash,
        message_id: expected_message_id,
        payload_hash: expected_payload_hash,
        source_event_digest,
        route_config_hash,
    })
}

/// Verify native TRON finality and transaction inclusion against one typed lane identity.
///
/// The returned authentication model is deliberately explicit: the native
/// header chain proves scheduled production, solidity, the successful call,
/// and its transaction Merkle inclusion.  The registry governs the exact
/// runtime-code/configuration identity because TRON headers do not commit those
/// contract records.
///
/// # Errors
///
/// Returns an error if the governed source identity or message statement does
/// not match, or if native finality or transaction verification fails.
pub fn verify_tron_native_source(
    proof: &TronNativeSourceProofV1,
    source_identity: &SccpSourceIdentityV1,
    expected_source_identity_hash: H256,
    expected_anchor_hash: H256,
    expected_message_id: H256,
    expected_payload_hash: H256,
    payload: &SccpPayloadV1,
) -> Result<ValidatedTronNativeSourceV1, TronNativeSourceError> {
    if !source_identity.is_well_formed()
        || !matches!(
            source_identity.lane.source,
            SccpNetworkV1::TronMainnet | SccpNetworkV1::TronNile | SccpNetworkV1::TronShasta
        )
    {
        return Err(TronNativeSourceError::InvalidSourceIdentity);
    }
    let SccpSourceEmitterV1::Tron(emitter) = source_identity.emitter else {
        return Err(TronNativeSourceError::InvalidSourceIdentity);
    };
    let SccpPayloadV1::Transfer(transfer) = payload;
    let expected_sender: [u8; TRON_ADDRESS_BYTES] = transfer
        .sender
        .as_slice()
        .try_into()
        .map_err(|_| TronNativeSourceError::InvalidSourceIdentity)?;
    if !verify_sccp_payload_structure(payload)
        || transfer.sender_codec != SCCP_CODEC_TRON_ADDRESS21
        || expected_sender[0] != 0x41
    {
        return Err(TronNativeSourceError::InvalidSourceIdentity);
    }
    let canonical_payload = canonical_sccp_payload_bytes(payload)
        .map_err(|_| TronNativeSourceError::InvalidSourceIdentity)?;
    if payload_hash(&canonical_payload) != expected_payload_hash
        || sccp_message_id(source_identity.lane, payload) != Some(expected_message_id)
    {
        return Err(TronNativeSourceError::InvalidSourceIdentity);
    }
    let source_event_digest = sccp_lane_source_event_digest_v1(
        source_identity.lane,
        expected_message_id,
        expected_payload_hash,
    )
    .ok_or(TronNativeSourceError::InvalidSourceIdentity)?;
    let identity_hash = sccp_source_identity_hash_v1(source_identity)
        .ok_or(TronNativeSourceError::InvalidSourceIdentity)?;
    if identity_hash != expected_source_identity_hash {
        return Err(TronNativeSourceError::SourceIdentityHashMismatch);
    }
    let finality = verify_tron_native_finality(
        &proof.finality,
        source_identity.lane.source,
        expected_anchor_hash,
    )
    .map_err(TronNativeSourceError::Finality)?;
    let lane_hash = sccp_lane_id_hash_v1(source_identity.lane)
        .ok_or(TronNativeSourceError::InvalidSourceIdentity)?;
    let transaction = verify_tron_native_sccp_transaction(
        &proof.transaction,
        finality.transaction_root,
        emitter.address,
        emitter.route_config_hash,
        source_identity.lane,
        payload,
    )
    .map_err(TronNativeSourceError::Transaction)?;
    if transaction.lane_hash != lane_hash
        || transaction.message_id != expected_message_id
        || transaction.payload_hash != expected_payload_hash
        || transaction.source_event_digest != source_event_digest
    {
        return Err(TronNativeSourceError::InvalidSourceIdentity);
    }
    Ok(ValidatedTronNativeSourceV1 {
        source_identity_hash: identity_hash,
        finality,
        transaction,
        deployment_authentication: TronSourceDeploymentAuthenticationV1::GovernedIdentity,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair};

    fn push_varint(out: &mut Vec<u8>, mut value: u64) {
        while value >= 0x80 {
            out.push(u8::try_from(value & 0x7f).expect("varint byte") | 0x80);
            value >>= 7;
        }
        out.push(u8::try_from(value).expect("varint tail"));
    }

    fn push_key(out: &mut Vec<u8>, field: u32, wire: u8) {
        push_varint(out, (u64::from(field) << 3) | u64::from(wire));
    }

    fn push_bytes(out: &mut Vec<u8>, field: u32, value: &[u8]) {
        push_key(out, field, 2);
        push_varint(out, u64::try_from(value.len()).expect("length"));
        out.extend_from_slice(value);
    }

    fn push_int(out: &mut Vec<u8>, field: u32, value: u64) {
        push_key(out, field, 0);
        push_varint(out, value);
    }

    fn signer_address(signer: &KeyPair) -> [u8; TRON_ADDRESS_BYTES] {
        let (Algorithm::Secp256k1, bytes) = signer
            .public_key()
            .try_to_bytes()
            .expect("secp256k1 public key")
        else {
            panic!("wrong key algorithm");
        };
        let public_key = EcdsaSecp256k1Sha256::parse_public_key(bytes).expect("public key");
        let mut address = [0u8; TRON_ADDRESS_BYTES];
        address[0] = 0x41;
        address[1..].copy_from_slice(&EcdsaSecp256k1Sha256::evm_address(&public_key));
        address
    }

    fn sign_header(signer: &KeyPair, raw_data: Vec<u8>) -> TronNativeSignedHeaderV1 {
        let hash = sha256_bytes(&raw_data);
        let secret_key_bytes = signer.private_key().to_bytes().1;
        let secret_key = EcdsaSecp256k1Sha256::parse_private_key(&secret_key_bytes)
            .expect("secp256k1 private key");
        let mut signature = EcdsaSecp256k1Sha256::sign_prehash_recoverable(&hash, &secret_key)
            .expect("recoverable signature");
        signature[64] = signature[64].checked_sub(27).expect("Ethereum recovery id");
        TronNativeSignedHeaderV1 {
            raw_data,
            witness_signature: signature.to_vec(),
        }
    }

    fn raw_header(
        number: u64,
        timestamp_ms: u64,
        parent: H256,
        witness: [u8; TRON_ADDRESS_BYTES],
        transaction_root: Option<H256>,
    ) -> Vec<u8> {
        let mut out = Vec::new();
        push_int(&mut out, 1, timestamp_ms);
        if let Some(root) = transaction_root {
            push_bytes(&mut out, 2, &root);
        }
        push_bytes(&mut out, 3, &parent);
        push_int(&mut out, 7, number);
        push_bytes(&mut out, 9, &witness);
        push_int(&mut out, 10, 31);
        push_bytes(&mut out, 11, &[0xA5; 32]);
        out
    }

    fn transaction_bytes(
        contract_payload: [u8; 20],
        contract_result: u64,
        type_url: &[u8],
    ) -> Vec<u8> {
        let owner = test_sender();
        let mut contract_address = [0u8; TRON_ADDRESS_BYTES];
        contract_address[0] = 0x41;
        contract_address[1..].copy_from_slice(&contract_payload);
        let call_data = canonical_tron_native_transfer_call_data(
            &test_transfer().recipient,
            test_transfer().amount,
        )
        .unwrap();

        let mut trigger = Vec::new();
        push_bytes(&mut trigger, 1, &owner);
        push_bytes(&mut trigger, 2, &contract_address);
        push_bytes(&mut trigger, 4, &call_data);

        let mut any = Vec::new();
        push_bytes(&mut any, 1, type_url);
        push_bytes(&mut any, 2, &trigger);

        let mut contract = Vec::new();
        push_int(&mut contract, 1, 31);
        push_bytes(&mut contract, 2, &any);

        let mut raw = Vec::new();
        push_bytes(&mut raw, 1, &[0x12, 0x34]);
        push_int(&mut raw, 3, 123);
        push_bytes(&mut raw, 4, &[0x44; 8]);
        push_int(&mut raw, 8, 2_000_000);
        push_bytes(&mut raw, 11, &contract);
        push_int(&mut raw, 14, 1_000_000);
        push_int(&mut raw, 18, 100_000_000);

        let mut result = Vec::new();
        push_int(&mut result, 3, contract_result);

        let mut transaction = Vec::new();
        push_bytes(&mut transaction, 1, &raw);
        push_bytes(&mut transaction, 2, &[0x55; TRON_SIGNATURE_BYTES]);
        push_bytes(&mut transaction, 5, &result);
        transaction
    }

    fn test_lane_hash() -> H256 {
        sccp_lane_id_hash_v1(iroha_data_model::bridge::sccp::SccpLaneIdV1 {
            source: SccpNetworkV1::TronMainnet,
            target: SccpNetworkV1::SoraTaira,
        })
        .expect("test TRON lane hash")
    }

    fn test_sender() -> [u8; TRON_ADDRESS_BYTES] {
        let mut sender = [0x22; TRON_ADDRESS_BYTES];
        sender[0] = 0x41;
        sender
    }

    fn test_transfer() -> crate::TransferPayloadV1 {
        crate::TransferPayloadV1 {
            version: 1,
            source_domain: 5,
            dest_domain: 0,
            nonce: 7,
            route_revision: 1,
            asset_home_domain: 0,
            asset_id_codec: super::super::SCCP_CODEC_CANONICAL_TEXT,
            asset_id: b"xor".to_vec(),
            amount: 77,
            sender_codec: SCCP_CODEC_TRON_ADDRESS21,
            sender: test_sender().to_vec(),
            recipient_codec: super::super::SCCP_CODEC_CANONICAL_TEXT,
            recipient: b"alice@taira".to_vec(),
            route_id_codec: super::super::SCCP_CODEC_CANONICAL_TEXT,
            route_id: b"taira_tron_xor".to_vec(),
        }
    }

    #[derive(Clone)]
    struct TestTransactionStatement {
        message_id: H256,
        payload_hash: H256,
        source_event_digest: H256,
        route_config_hash: H256,
    }

    fn test_transaction_statement() -> TestTransactionStatement {
        let payload = SccpPayloadV1::Transfer(test_transfer());
        let lane = iroha_data_model::bridge::sccp::SccpLaneIdV1 {
            source: SccpNetworkV1::TronMainnet,
            target: SccpNetworkV1::SoraTaira,
        };
        let canonical_payload =
            canonical_sccp_payload_bytes(&payload).expect("valid TRON SCCP test payload encodes");
        let message_id = sccp_message_id(lane, &payload).unwrap();
        let payload_hash = payload_hash(&canonical_payload);
        let source_event_digest =
            sccp_lane_source_event_digest_v1(lane, message_id, payload_hash).unwrap();
        TestTransactionStatement {
            message_id,
            payload_hash,
            source_event_digest,
            route_config_hash: [0x45; 32],
        }
    }

    fn verify_test_transaction(
        proof: &TronNativeTransactionProofV1,
        root: H256,
        contract: [u8; 20],
        statement: &TestTransactionStatement,
    ) -> Result<ValidatedTronNativeTransactionV1, TronNativeTransactionError> {
        verify_tron_native_sccp_transaction(
            proof,
            root,
            contract,
            statement.route_config_hash,
            SccpLaneIdV1 {
                source: SccpNetworkV1::TronMainnet,
                target: SccpNetworkV1::SoraTaira,
            },
            &SccpPayloadV1::Transfer(test_transfer()),
        )
    }

    struct Fixture {
        signers: Vec<KeyPair>,
        anchor: TronNativeDposAnchorV1,
    }

    fn fixture() -> Fixture {
        let signers = (0..TRON_ACTIVE_WITNESS_COUNT)
            .map(|_| KeyPair::random_with_algorithm(Algorithm::Secp256k1))
            .collect::<Vec<_>>();
        let witnesses = signers
            .iter()
            .enumerate()
            .map(|(index, signer)| {
                let address = signer_address(signer);
                TronNativeWitnessV1 {
                    account_address: address.to_vec(),
                    signing_address: address.to_vec(),
                    latest_block_number: if index < 9 { 80 } else { 99 },
                }
            })
            .collect::<Vec<_>>();
        Fixture {
            signers,
            anchor: TronNativeDposAnchorV1 {
                version: 1,
                network: SccpNetworkV1::TronMainnet,
                block_number: 100,
                block_id: {
                    let mut id = [0x11; 32];
                    id[..8].copy_from_slice(&100u64.to_be_bytes());
                    id
                },
                timestamp_ms: 3_300_000,
                genesis_timestamp_ms: 300_000,
                next_maintenance_time_ms: 30_000_000,
                maintenance_interval_ms: 21_600_000,
                maintenance_skip_slots: 2,
                single_repeat: 1,
                solidified_threshold_percent: 70,
                require_aligned_timestamps: true,
                anchor_is_maintenance: false,
                solid_block_number: 80,
                witnesses,
            },
        }
    }

    fn proof_with_target_root_and_signers(
        target_transaction_root: H256,
    ) -> (TronNativeFinalityProofV1, H256, Vec<KeyPair>) {
        let fixture = fixture();
        let anchor_hash = tron_native_anchor_hash(&fixture.anchor).expect("anchor hash");
        let mut parent = fixture.anchor.block_id;
        let mut headers = Vec::new();
        // Nineteen distinct scheduled producers raise the order statistic to
        // the target height, matching Java-TRON's native 70% solidification.
        for offset in 1..=19u64 {
            let number = fixture.anchor.block_number + offset;
            let timestamp = fixture.anchor.timestamp_ms + offset * TRON_BLOCK_INTERVAL_MS;
            let slot = timestamp
                .checked_sub(fixture.anchor.genesis_timestamp_ms)
                .expect("fixture header follows genesis")
                / TRON_BLOCK_INTERVAL_MS;
            let scheduled = usize::try_from(slot % TRON_ACTIVE_WITNESS_COUNT as u64).unwrap();
            let tx_root = (offset == 1).then_some(target_transaction_root);
            let raw = raw_header(
                number,
                timestamp,
                parent,
                signer_address(&fixture.signers[scheduled]),
                tx_root,
            );
            let signed = sign_header(&fixture.signers[scheduled], raw);
            let parsed = parse_tron_raw_header(&signed.raw_data).expect("header");
            parent = tron_block_id(parsed.number, sha256_bytes(&signed.raw_data));
            headers.push(signed);
        }
        (
            TronNativeFinalityProofV1 {
                version: 1,
                anchor: fixture.anchor,
                headers,
                target_header_index: 0,
            },
            anchor_hash,
            fixture.signers,
        )
    }

    fn proof_with_target_root(target_transaction_root: H256) -> (TronNativeFinalityProofV1, H256) {
        let (proof, anchor_hash, _) = proof_with_target_root_and_signers(target_transaction_root);
        (proof, anchor_hash)
    }

    fn append_valid_headers(
        proof: &mut TronNativeFinalityProofV1,
        signers: &[KeyPair],
        count: usize,
    ) {
        let last = parse_tron_raw_header(&proof.headers.last().unwrap().raw_data).unwrap();
        let mut parent = tron_block_id(
            last.number,
            sha256_bytes(&proof.headers.last().unwrap().raw_data),
        );
        let mut number = last.number;
        let mut timestamp_ms = last.timestamp_ms;
        for _ in 0..count {
            number += 1;
            timestamp_ms += TRON_BLOCK_INTERVAL_MS;
            let slot = (timestamp_ms - proof.anchor.genesis_timestamp_ms) / TRON_BLOCK_INTERVAL_MS;
            let scheduled = usize::try_from(slot % TRON_ACTIVE_WITNESS_COUNT as u64).unwrap();
            let raw = raw_header(
                number,
                timestamp_ms,
                parent,
                signer_address(&signers[scheduled]),
                None,
            );
            let signed = sign_header(&signers[scheduled], raw);
            parent = tron_block_id(number, sha256_bytes(&signed.raw_data));
            proof.headers.push(signed);
        }
    }

    fn proof_with_distinct_confirmations() -> (TronNativeFinalityProofV1, H256) {
        proof_with_target_root([0xD5; 32])
    }

    #[test]
    fn finality_work_estimate_enforces_witness_round_target_and_suffix_bounds() {
        let fixture = fixture();
        let header = TronNativeSignedHeaderV1 {
            raw_data: vec![0x01, 0x02],
            witness_signature: vec![0x03; TRON_SIGNATURE_BYTES],
        };
        let mut proof = TronNativeFinalityProofV1 {
            version: 1,
            anchor: fixture.anchor,
            headers: vec![header.clone()],
            target_header_index: 0,
        };
        let estimate = tron_native_finality_work_estimate(&proof).unwrap();
        assert_eq!(estimate.continuation_headers, 1);
        assert_eq!(estimate.secp256k1_recoveries, 1);
        assert_eq!(
            estimate.framed_header_bytes,
            u32::try_from(2 + TRON_SIGNATURE_BYTES).unwrap()
        );

        proof.headers = vec![header.clone(); TRON_NATIVE_MAX_FINALITY_HEADERS];
        proof.target_header_index = u16::try_from(TRON_NATIVE_MAX_TARGET_HEADERS - 1).unwrap();
        let boundary = tron_native_finality_work_estimate(&proof).unwrap();
        assert_eq!(
            usize::from(boundary.continuation_headers),
            TRON_NATIVE_MAX_FINALITY_HEADERS
        );

        proof.target_header_index = u16::try_from(TRON_NATIVE_MAX_TARGET_HEADERS).unwrap();
        assert_eq!(
            tron_native_finality_work_estimate(&proof),
            Err(TronNativeFinalityError::InvalidProofShape)
        );

        proof.target_header_index = 0;
        proof.headers = vec![header.clone(); TRON_NATIVE_MAX_FINALITY_SUFFIX_HEADERS + 2];
        assert_eq!(
            tron_native_finality_work_estimate(&proof),
            Err(TronNativeFinalityError::InvalidProofShape)
        );

        proof.headers = vec![header.clone(); TRON_NATIVE_MAX_FINALITY_HEADERS + 1];
        assert_eq!(
            tron_native_finality_work_estimate(&proof),
            Err(TronNativeFinalityError::InvalidProofShape)
        );

        proof.headers = vec![TronNativeSignedHeaderV1 {
            raw_data: vec![0; TRON_MAX_RAW_HEADER_BYTES + 1],
            witness_signature: vec![0; TRON_SIGNATURE_BYTES],
        }];
        assert_eq!(
            tron_native_finality_work_estimate(&proof),
            Err(TronNativeFinalityError::InvalidProofShape)
        );

        proof.headers = vec![TronNativeSignedHeaderV1 {
            raw_data: vec![0x01],
            witness_signature: vec![0; TRON_SIGNATURE_BYTES - 1],
        }];
        assert_eq!(
            tron_native_finality_work_estimate(&proof),
            Err(TronNativeFinalityError::InvalidProofShape)
        );

        proof.headers = vec![header];
        proof.target_header_index = 1;
        assert_eq!(
            tron_native_finality_work_estimate(&proof),
            Err(TronNativeFinalityError::InvalidProofShape)
        );
    }

    #[test]
    fn native_dpos_accepts_target_only_after_nineteen_distinct_producers() {
        let (proof, anchor_hash, signers) = proof_with_target_root_and_signers([0xD5; 32]);
        let validated =
            verify_tron_native_finality(&proof, SccpNetworkV1::TronMainnet, anchor_hash)
                .expect("native finality");
        assert_eq!(validated.block_number, 101);
        assert!(validated.resulting_solid_block_number >= 101);

        let mut exact_work = TronNativeFinalityWorkPerformedV1::default();
        verify_tron_native_finality_counted(
            &proof,
            SccpNetworkV1::TronMainnet,
            anchor_hash,
            &mut exact_work,
        )
        .unwrap();
        assert_eq!(exact_work.continuation_headers, 19);
        assert_eq!(exact_work.secp256k1_recoveries, 19);

        let mut only_eighteen = proof.clone();
        only_eighteen.headers.pop();
        assert_eq!(
            verify_tron_native_finality(&only_eighteen, SccpNetworkV1::TronMainnet, anchor_hash,),
            Err(TronNativeFinalityError::TargetNotSolid)
        );

        let mut one_surplus = proof.clone();
        append_valid_headers(&mut one_surplus, &signers, 1);
        let mut one_surplus_work = TronNativeFinalityWorkPerformedV1::default();
        assert_eq!(
            verify_tron_native_finality_counted(
                &one_surplus,
                SccpNetworkV1::TronMainnet,
                anchor_hash,
                &mut one_surplus_work,
            ),
            Err(TronNativeFinalityError::NonMinimalContinuation)
        );
        assert_eq!(one_surplus_work, exact_work);

        let mut many_surplus = proof.clone();
        append_valid_headers(&mut many_surplus, &signers, 8);
        assert_eq!(many_surplus.headers.len(), TRON_ACTIVE_WITNESS_COUNT);
        let mut many_surplus_work = TronNativeFinalityWorkPerformedV1::default();
        assert_eq!(
            verify_tron_native_finality_counted(
                &many_surplus,
                SccpNetworkV1::TronMainnet,
                anchor_hash,
                &mut many_surplus_work,
            ),
            Err(TronNativeFinalityError::NonMinimalContinuation)
        );
        assert_eq!(
            many_surplus_work, exact_work,
            "no appended witness signature is recovered"
        );

        let mut over_window = proof;
        append_valid_headers(&mut over_window, &signers, 36);
        assert_eq!(
            over_window.headers.len(),
            TRON_NATIVE_MAX_FINALITY_HEADERS + 1
        );
        let mut over_window_work = TronNativeFinalityWorkPerformedV1::default();
        assert_eq!(
            verify_tron_native_finality_counted(
                &over_window,
                SccpNetworkV1::TronMainnet,
                anchor_hash,
                &mut over_window_work,
            ),
            Err(TronNativeFinalityError::InvalidProofShape)
        );
        assert_eq!(
            over_window_work,
            TronNativeFinalityWorkPerformedV1::default(),
            "over-window continuations fail before anchor or signature work"
        );
    }

    #[test]
    fn native_dpos_rejects_chain_schedule_signature_and_maintenance_mutations() {
        let (proof, anchor_hash) = proof_with_distinct_confirmations();
        let reject = |proof: &TronNativeFinalityProofV1, expected| {
            assert_eq!(
                verify_tron_native_finality(proof, SccpNetworkV1::TronMainnet, anchor_hash,),
                Err(expected)
            );
        };

        let mut wrong_parent = proof.clone();
        let parsed = parse_tron_raw_header(&wrong_parent.headers[0].raw_data).unwrap();
        let mut parent = parsed.parent_block_id;
        parent[8] ^= 1;
        let witness: [u8; TRON_ADDRESS_BYTES] = parsed.witness_address.try_into().unwrap();
        wrong_parent.headers[0].raw_data = raw_header(
            parsed.number,
            parsed.timestamp_ms,
            parent,
            witness,
            parsed.transaction_root,
        );
        reject(&wrong_parent, TronNativeFinalityError::HeaderChainMismatch);

        let mut wrong_schedule = proof.clone();
        let parsed = parse_tron_raw_header(&wrong_schedule.headers[0].raw_data).unwrap();
        let mut address_offset = wrong_schedule.headers[0]
            .raw_data
            .windows(TRON_ADDRESS_BYTES)
            .position(|window| window == parsed.witness_address)
            .unwrap();
        address_offset += 1;
        wrong_schedule.headers[0].raw_data[address_offset] ^= 1;
        reject(
            &wrong_schedule,
            TronNativeFinalityError::WrongScheduledWitness,
        );

        let mut wrong_signature = proof.clone();
        wrong_signature.headers[0].witness_signature[0] ^= 1;
        reject(
            &wrong_signature,
            TronNativeFinalityError::InvalidWitnessSignature,
        );

        let mut boundary = proof.clone();
        boundary.anchor.next_maintenance_time_ms =
            boundary.anchor.timestamp_ms + TRON_BLOCK_INTERVAL_MS;
        let boundary_hash = tron_native_anchor_hash(&boundary.anchor).unwrap();
        assert_eq!(
            verify_tron_native_finality(&boundary, SccpNetworkV1::TronMainnet, boundary_hash,),
            Err(TronNativeFinalityError::MaintenanceBoundary)
        );
    }

    #[test]
    fn maintenance_checkpoint_skips_time_slots_but_advances_one_witness() {
        let fixture = fixture();
        let mut anchor = fixture.anchor;
        anchor.anchor_is_maintenance = true;
        let anchor_hash = tron_native_anchor_hash(&anchor).unwrap();
        let timestamp = anchor.timestamp_ms + 3 * TRON_BLOCK_INTERVAL_MS;
        let prior_absolute_slot =
            (anchor.timestamp_ms - anchor.genesis_timestamp_ms) / TRON_BLOCK_INTERVAL_MS;
        let scheduled_index = usize::try_from(
            prior_absolute_slot.checked_add(1).unwrap() % TRON_ACTIVE_WITNESS_COUNT as u64,
        )
        .unwrap();
        let raw = raw_header(
            anchor.block_number + 1,
            timestamp,
            anchor.block_id,
            signer_address(&fixture.signers[scheduled_index]),
            Some([0xD5; 32]),
        );
        let proof = TronNativeFinalityProofV1 {
            version: 1,
            anchor: anchor.clone(),
            headers: vec![sign_header(&fixture.signers[scheduled_index], raw)],
            target_header_index: 0,
        };
        assert_eq!(
            verify_tron_native_finality(&proof, SccpNetworkV1::TronMainnet, anchor_hash,),
            Err(TronNativeFinalityError::TargetNotSolid)
        );

        let absolute_index = usize::try_from(
            (timestamp - anchor.genesis_timestamp_ms) / TRON_BLOCK_INTERVAL_MS
                % TRON_ACTIVE_WITNESS_COUNT as u64,
        )
        .unwrap();
        assert_ne!(absolute_index, scheduled_index);
        let wrong_raw = raw_header(
            anchor.block_number + 1,
            timestamp,
            anchor.block_id,
            signer_address(&fixture.signers[absolute_index]),
            Some([0xD5; 32]),
        );
        let wrong = TronNativeFinalityProofV1 {
            version: 1,
            anchor,
            headers: vec![sign_header(&fixture.signers[absolute_index], wrong_raw)],
            target_header_index: 0,
        };
        assert_eq!(
            verify_tron_native_finality(&wrong, SccpNetworkV1::TronMainnet, anchor_hash,),
            Err(TronNativeFinalityError::WrongScheduledWitness)
        );
    }

    #[test]
    fn raw_header_rejects_unknown_duplicate_reordered_and_overlong_varints() {
        let fixture = fixture();
        let witness: [u8; TRON_ADDRESS_BYTES] = fixture.anchor.witnesses[0]
            .account_address
            .as_slice()
            .try_into()
            .unwrap();
        let canonical = raw_header(
            101,
            3_003_000,
            fixture.anchor.block_id,
            witness,
            Some([7; 32]),
        );
        assert!(parse_tron_raw_header(&canonical).is_some());

        let mut unknown = canonical.clone();
        push_int(&mut unknown, 12, 1);
        assert!(parse_tron_raw_header(&unknown).is_none());

        let mut duplicate = canonical.clone();
        push_int(&mut duplicate, 10, 31);
        assert!(parse_tron_raw_header(&duplicate).is_none());

        let mut reordered = Vec::new();
        push_int(&mut reordered, 7, 101);
        reordered.extend_from_slice(&canonical);
        assert!(parse_tron_raw_header(&reordered).is_none());

        let mut overlong = canonical.clone();
        overlong[0] = 0x88;
        overlong.insert(1, 0x00);
        assert!(parse_tron_raw_header(&overlong).is_none());
    }

    #[test]
    fn transfer_call_scales_taira_units_to_trc20_base_units() {
        let recipient = b"alice@taira";
        let call = canonical_tron_native_transfer_call_data(recipient, 77)
            .expect("canonical scaled transfer call");
        let mut expected_amount_word = [0u8; 32];
        expected_amount_word[16..].copy_from_slice(&(77_u128 * 1_000_000_000).to_be_bytes());
        assert_eq!(&call[36..68], &expected_amount_word);
        assert_ne!(&call[52..68], &77_u128.to_be_bytes());
        assert!(canonical_tron_native_transfer_call_data(recipient, u128::MAX).is_some());
        assert!(canonical_tron_native_transfer_call_data(recipient, 0).is_none());
        assert!(canonical_tron_native_transfer_call_data(&[], 1).is_none());
        assert!(canonical_tron_native_transfer_call_data(&[b'a'; 257], 1).is_none());
    }

    #[test]
    fn native_transaction_authenticates_full_success_result_call_and_single_leaf_root() {
        let contract = [0x33; 20];
        let statement = test_transaction_statement();
        let transaction = transaction_bytes(contract, 1, TRON_TRIGGER_SMART_CONTRACT_TYPE_URL_V1);
        let root = sha256_bytes(&transaction);
        let proof = TronNativeTransactionProofV1 {
            transaction_index: 0,
            transaction_count: 1,
            transaction_bytes: transaction,
            merkle_branch: Vec::new(),
        };
        let validated = verify_test_transaction(&proof, root, contract, &statement)
            .expect("native transaction proof");
        assert_eq!(validated.transaction_hash, root);
        assert_eq!(&validated.contract_address[1..], contract);
        assert_eq!(validated.lane_hash, test_lane_hash());
        assert_eq!(validated.message_id, statement.message_id);
        assert_eq!(validated.payload_hash, statement.payload_hash);
        assert_eq!(validated.source_event_digest, statement.source_event_digest);
    }

    #[test]
    fn complete_native_source_binds_typed_lane_transfer_contract_and_anchor() {
        use iroha_data_model::bridge::sccp::{SccpLaneIdV1, SccpTronSourceEmitterV1};

        let contract = [0x33; 20];
        let statement = test_transaction_statement();
        let payload = SccpPayloadV1::Transfer(test_transfer());
        let transaction_bytes =
            transaction_bytes(contract, 1, TRON_TRIGGER_SMART_CONTRACT_TYPE_URL_V1);
        let transaction_root = sha256_bytes(&transaction_bytes);
        let (finality, anchor_hash) = proof_with_target_root(transaction_root);
        let proof = TronNativeSourceProofV1 {
            finality,
            transaction: TronNativeTransactionProofV1 {
                transaction_index: 0,
                transaction_count: 1,
                transaction_bytes,
                merkle_branch: Vec::new(),
            },
        };
        let identity = SccpSourceIdentityV1 {
            lane: SccpLaneIdV1 {
                source: SccpNetworkV1::TronMainnet,
                target: SccpNetworkV1::SoraTaira,
            },
            emitter: SccpSourceEmitterV1::Tron(SccpTronSourceEmitterV1 {
                address: contract,
                runtime_code_hash: [0x44; 32],
                route_config_hash: statement.route_config_hash,
            }),
        };
        let identity_hash = sccp_source_identity_hash_v1(&identity).unwrap();
        let validated = verify_tron_native_source(
            &proof,
            &identity,
            identity_hash,
            anchor_hash,
            statement.message_id,
            statement.payload_hash,
            &payload,
        )
        .expect("complete native source proof");
        assert_eq!(validated.source_identity_hash, identity_hash);
        assert_eq!(
            validated.deployment_authentication,
            TronSourceDeploymentAuthenticationV1::GovernedIdentity
        );

        assert_eq!(
            verify_tron_native_source(
                &proof,
                &identity,
                [0xAA; 32],
                anchor_hash,
                statement.message_id,
                statement.payload_hash,
                &payload,
            ),
            Err(TronNativeSourceError::SourceIdentityHashMismatch)
        );
        let mut wrong_profile = identity;
        wrong_profile.lane.source = SccpNetworkV1::TronNile;
        let wrong_profile_hash = sccp_source_identity_hash_v1(&wrong_profile).unwrap();
        assert_eq!(
            verify_tron_native_source(
                &proof,
                &wrong_profile,
                wrong_profile_hash,
                anchor_hash,
                statement.message_id,
                statement.payload_hash,
                &payload,
            ),
            Err(TronNativeSourceError::InvalidSourceIdentity)
        );
    }

    #[test]
    fn native_transaction_rejects_failure_old_selector_payload_splice_and_receipt_aliases() {
        let contract = [0x33; 20];
        let statement = test_transaction_statement();
        let proof_for = |bytes: Vec<u8>| TronNativeTransactionProofV1 {
            transaction_index: 0,
            transaction_count: 1,
            merkle_branch: Vec::new(),
            transaction_bytes: bytes,
        };

        let failed = transaction_bytes(contract, 2, TRON_TRIGGER_SMART_CONTRACT_TYPE_URL_V1);
        assert_eq!(
            verify_test_transaction(
                &proof_for(failed.clone()),
                sha256_bytes(&failed),
                contract,
                &statement,
            ),
            Err(TronNativeTransactionError::TransactionFailed)
        );

        let wrong_type = transaction_bytes(contract, 1, b"type.googleapis.com/Receipt");
        assert_eq!(
            verify_test_transaction(
                &proof_for(wrong_type.clone()),
                sha256_bytes(&wrong_type),
                contract,
                &statement,
            ),
            Err(TronNativeTransactionError::InvalidTransactionEncoding)
        );

        let valid = transaction_bytes(contract, 1, TRON_TRIGGER_SMART_CONTRACT_TYPE_URL_V1);
        let mut legacy_unscaled = valid.clone();
        let canonical_call = canonical_tron_native_transfer_call_data(
            &test_transfer().recipient,
            test_transfer().amount,
        )
        .expect("canonical scaled transfer call");
        let call_offset = legacy_unscaled
            .windows(canonical_call.len())
            .position(|window| window == canonical_call)
            .expect("embedded transfer call");
        legacy_unscaled[call_offset + 36..call_offset + 52].fill(0);
        legacy_unscaled[call_offset + 52..call_offset + 68]
            .copy_from_slice(&test_transfer().amount.to_be_bytes());
        assert_eq!(
            verify_test_transaction(
                &proof_for(legacy_unscaled.clone()),
                sha256_bytes(&legacy_unscaled),
                contract,
                &statement,
            ),
            Err(TronNativeTransactionError::WrongCallData)
        );
        assert_eq!(
            verify_test_transaction(
                &proof_for(valid.clone()),
                sha256_bytes(&valid),
                [0x44; 20],
                &statement,
            ),
            Err(TronNativeTransactionError::WrongContract)
        );
        let lane = SccpLaneIdV1 {
            source: SccpNetworkV1::TronMainnet,
            target: SccpNetworkV1::SoraTaira,
        };
        let mut wrong_sender = test_transfer();
        wrong_sender.sender[1] ^= 1;
        assert_eq!(
            verify_tron_native_sccp_transaction(
                &proof_for(valid.clone()),
                sha256_bytes(&valid),
                contract,
                statement.route_config_hash,
                lane,
                &SccpPayloadV1::Transfer(wrong_sender),
            ),
            Err(TronNativeTransactionError::WrongCaller)
        );
        let mut wrong_recipient = test_transfer();
        wrong_recipient.recipient.push(b'x');
        assert_eq!(
            verify_tron_native_sccp_transaction(
                &proof_for(valid.clone()),
                sha256_bytes(&valid),
                contract,
                statement.route_config_hash,
                lane,
                &SccpPayloadV1::Transfer(wrong_recipient),
            ),
            Err(TronNativeTransactionError::WrongCallData)
        );
        let mut wrong_amount = test_transfer();
        wrong_amount.amount += 1;
        assert_eq!(
            verify_tron_native_sccp_transaction(
                &proof_for(valid.clone()),
                sha256_bytes(&valid),
                contract,
                statement.route_config_hash,
                lane,
                &SccpPayloadV1::Transfer(wrong_amount),
            ),
            Err(TronNativeTransactionError::WrongCallData)
        );
        assert_eq!(
            verify_tron_native_sccp_transaction(
                &proof_for(valid.clone()),
                sha256_bytes(&valid),
                contract,
                statement.route_config_hash,
                SccpLaneIdV1 {
                    source: SccpNetworkV1::EthereumMainnet,
                    target: SccpNetworkV1::SoraTaira,
                },
                &SccpPayloadV1::Transfer(test_transfer()),
            ),
            Err(TronNativeTransactionError::InvalidProofShape)
        );

        // A TransactionInfo/log-like payload is not a native transaction leaf
        // and cannot be smuggled in under the block transaction root.
        let receipt_alias = vec![0x0A, 0x20];
        assert_eq!(
            verify_test_transaction(
                &proof_for(receipt_alias.clone()),
                sha256_bytes(&receipt_alias),
                contract,
                &statement,
            ),
            Err(TronNativeTransactionError::InvalidTransactionEncoding)
        );
    }

    #[test]
    fn native_transaction_merkle_proof_promotes_odd_leaf_without_duplication() {
        let contract = [0x33; 20];
        let statement = test_transaction_statement();
        let transaction = transaction_bytes(contract, 1, TRON_TRIGGER_SMART_CONTRACT_TYPE_URL_V1);
        let leaf = sha256_bytes(&transaction);
        let left_pair = tron_transaction_merkle_node([0x10; 32], [0x20; 32]);
        let root = tron_transaction_merkle_node(left_pair, leaf);
        let proof = TronNativeTransactionProofV1 {
            transaction_index: 2,
            transaction_count: 3,
            transaction_bytes: transaction,
            merkle_branch: vec![left_pair.to_vec()],
        };
        assert!(verify_test_transaction(&proof, root, contract, &statement).is_ok());

        let duplicated_odd_root =
            tron_transaction_merkle_node(left_pair, tron_transaction_merkle_node(leaf, leaf));
        assert_eq!(
            verify_test_transaction(&proof, duplicated_odd_root, contract, &statement,),
            Err(TronNativeTransactionError::InvalidMerkleProof)
        );

        let mut extra_branch = proof.clone();
        extra_branch.merkle_branch.push([0x99; 32].to_vec());
        assert_eq!(
            verify_test_transaction(&extra_branch, root, contract, &statement,),
            Err(TronNativeTransactionError::InvalidMerkleProof)
        );
    }

    #[test]
    fn anchor_rejects_duplicate_permissions_wrong_constants_and_hash_replay() {
        let fixture = fixture();
        assert!(canonical_tron_native_anchor_bytes(&fixture.anchor).is_some());
        let hash = tron_native_anchor_hash(&fixture.anchor).unwrap();

        let mut duplicate_signer = fixture.anchor.clone();
        duplicate_signer.witnesses[1].signing_address =
            duplicate_signer.witnesses[0].signing_address.clone();
        assert!(tron_native_anchor_hash(&duplicate_signer).is_none());

        let mut wrong_threshold = fixture.anchor.clone();
        wrong_threshold.solidified_threshold_percent = 67;
        assert!(tron_native_anchor_hash(&wrong_threshold).is_none());

        let mut wrong_skip = fixture.anchor.clone();
        wrong_skip.maintenance_skip_slots = 1;
        assert!(tron_native_anchor_hash(&wrong_skip).is_none());

        let (proof, _) = proof_with_distinct_confirmations();
        assert_eq!(
            verify_tron_native_finality(&proof, SccpNetworkV1::TronNile, hash),
            Err(TronNativeFinalityError::WrongNetwork)
        );
        assert_eq!(
            verify_tron_native_finality(&proof, SccpNetworkV1::TronMainnet, [0xFF; 32],),
            Err(TronNativeFinalityError::AnchorHashMismatch)
        );
    }
}
