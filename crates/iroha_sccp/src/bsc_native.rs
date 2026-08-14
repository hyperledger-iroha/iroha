//! Protocol-native BNB Smart Chain Parlia verification for SCCP.
//!
//! Parlia blocks have two independent authentication layers.  The proposer
//! signs the chain-id-prefixed execution header with secp256k1, while a native
//! `VoteAttestation` uses BLS12-381 to justify a target and finalize its source.
//! This module replays both layers from a governed, immutable snapshot.  It
//! supports the post-Mendel first-release protocol and refuses unknown header
//! layouts or validator-set changes that were not finalized by the preceding
//! set.
use super::{
    H256, SccpPayloadV1, canonical_sccp_payload_bytes, decode_canonical_sccp_payload_bytes,
    prefixed_blake2b, sccp_lane_id_hash_v1, sccp_message_id, sccp_source_identity_hash_v1,
};
use alloc::{
    collections::{BTreeMap, BTreeSet},
    vec,
    vec::Vec,
};
use iroha_crypto::EcdsaSecp256k1Sha256;
use iroha_crypto::{ethereum_bls_pop_fast_aggregate_verify, ethereum_bls_pop_validate_public_key};
use iroha_data_model::bridge::sccp::{SccpNetworkV1, SccpSourceEmitterV1, SccpSourceIdentityV1};
use tiny_keccak::{Hasher as _, Keccak};
/// BNB Smart Chain mainnet EIP-155 chain identifier.
pub const BSC_NATIVE_MAINNET_CHAIN_ID: u64 = 56;
/// BNB Smart Chain Chapel testnet EIP-155 chain identifier.
pub const BSC_NATIVE_TESTNET_CHAIN_ID: u64 = 97;
/// Mainnet timestamp at which Osaka and Mendel activate together.
pub const BSC_NATIVE_MAINNET_MENDEL_TIME: u64 = 1_777_343_400;
/// Chapel timestamp at which Osaka and Mendel activate together.
pub const BSC_NATIVE_TESTNET_MENDEL_TIME: u64 = 1_774_319_400;
/// Post-Maxwell Parlia epoch length.
pub const BSC_NATIVE_EPOCH_LENGTH: u64 = 1_000;
/// Post-Fermi Parlia block interval in milliseconds.
pub const BSC_NATIVE_BLOCK_INTERVAL_MS: u64 = 450;
/// Maximum native fast-finality ancestor depth after Fermi.
pub const BSC_NATIVE_ATTESTATION_ANCESTOR_DEPTH: usize = 3;
const BSC_NATIVE_ANCHOR_PREFIX_V1: &[u8] = b"sccp:bsc:native-parlia-anchor:v1";
const BSC_NATIVE_EVENT_ABI_V1: &[u8] =
    b"SccpTransfer(bytes32,bytes32,bytes32,bytes32,bytes32,bytes)";
const EXTRA_VANITY_BYTES: usize = 32;
const EXTRA_SEAL_BYTES: usize = 65;
const VALIDATOR_BYTES: usize = 20 + 48;
const EMPTY_UNCLE_HASH: H256 = [
    0x1d, 0xcc, 0x4d, 0xe8, 0xde, 0xc7, 0x5d, 0x7a, 0xab, 0x85, 0xb5, 0x67, 0xb6, 0xcc, 0xd4, 0x1a,
    0xd3, 0x12, 0x45, 0x1b, 0x94, 0x8a, 0x74, 0x13, 0xf0, 0xa1, 0x42, 0xfd, 0x40, 0xd4, 0x93, 0x47,
];
const EMPTY_TRIE_HASH: H256 = [
    0x56, 0xe8, 0x1f, 0x17, 0x1b, 0xcc, 0x55, 0xa6, 0xff, 0x83, 0x45, 0xe6, 0x92, 0xc0, 0xf8, 0x6e,
    0x5b, 0x48, 0xe0, 0x1b, 0x99, 0x6c, 0xad, 0xc0, 0x01, 0x62, 0x2f, 0xb5, 0xe3, 0x63, 0xb4, 0x21,
];
const BLOB_GAS_PER_BLOB: u64 = 1 << 17;
const BLOB_TARGET: u64 = 3;
const BLOB_MAX: u64 = 6;
const BLOB_ELIGIBLE_BLOCK_INTERVAL: u64 = 5;
const MIN_GAS_LIMIT: u64 = 5_000;
const GAS_LIMIT_BOUND_DIVISOR: u64 = 1_024;
const MAX_HEADER_BYTES: usize = 128 * 1_024;
const MAX_EXTRA_BYTES: usize = 64 * 1_024;
const MAX_VALIDATORS: usize = 64;
const MAX_TURN_LENGTH: u8 = 64;
const MAX_ATTESTATION_EXTRA_BYTES: usize = 256;
const MAX_RECEIPT_BYTES: usize = 1_024 * 1_024;
const MAX_RECEIPT_LOGS: usize = 1_024;
const MAX_LOG_TOPICS: usize = 4;
const MAX_MPT_NODES: usize = 128;
const MAX_MPT_NODE_BYTES: usize = 512 * 1_024;
const MAX_MPT_TOTAL_BYTES: usize = 4 * 1_024 * 1_024;
/// Maximum number of post-anchor headers before the selected BSC target.
///
/// V1 requires governance to refresh a Parlia anchor at least once per
/// 1,000-block epoch.  A proof may therefore select any header in the first
/// complete epoch after its anchor, but cannot turn a stale checkpoint into an
/// unbounded signature-replay job.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the protocol-fixed epoch length is 1,000, which fits every supported usize width"
)]
pub const BSC_NATIVE_MAX_TARGET_HEADERS: usize = BSC_NATIVE_EPOCH_LENGTH as usize;
/// Maximum number of headers after a BSC target before it becomes final.
///
/// A vote may refer to any of the three retained ancestor contexts.  The next
/// contiguous vote can then make that target the finalized source, hence the
/// exact worst case is `ancestor_depth + 1` headers.
pub const BSC_NATIVE_MAX_FINALITY_SUFFIX_HEADERS: usize = BSC_NATIVE_ATTESTATION_ANCESTOR_DEPTH + 1;
/// Maximum headers in one canonical native BSC finality continuation.
pub const BSC_NATIVE_MAX_FINALITY_HEADERS: usize =
    BSC_NATIVE_MAX_TARGET_HEADERS + BSC_NATIVE_MAX_FINALITY_SUFFIX_HEADERS;
/// One Parlia validator and its fast-finality vote key.
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
pub struct BscNativeValidatorV1 {
    /// Canonical 20-byte execution-layer consensus address.
    #[norito(with = "crate::json_utils::bytes_hex")]
    pub consensus_address: Vec<u8>,
    /// Canonical compressed 48-byte BLS min-pk public key.
    #[norito(with = "crate::json_utils::bytes_hex")]
    pub vote_public_key: Vec<u8>,
}
/// One retained Parlia recent-proposer record.
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
pub struct BscNativeRecentProposerV1 {
    /// Block number at which the proposer signed.
    #[norito(with = "crate::json_utils::u64_string")]
    pub block_number: u64,
    /// Canonical 20-byte proposer address.
    #[norito(with = "crate::json_utils::bytes_hex")]
    pub consensus_address: Vec<u8>,
}
/// Native Parlia source/target justification state.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
pub struct BscNativeJustificationV1 {
    /// Finalized source block number.
    #[norito(with = "crate::json_utils::u64_string")]
    pub source_number: u64,
    /// Finalized source block hash.
    #[norito(with = "crate::json_utils::hex32")]
    pub source_hash: H256,
    /// Highest justified target block number.
    #[norito(with = "crate::json_utils::u64_string")]
    pub target_number: u64,
    /// Highest justified target block hash.
    #[norito(with = "crate::json_utils::hex32")]
    pub target_hash: H256,
}
/// Validator context used to verify votes for one recent target block.
///
/// Parlia verifies an attestation using the snapshot at `target_number - 1`.
/// The anchor retains the three contexts that a first post-anchor header may
/// legitimately reference under the post-Fermi ancestor-depth rule.
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
pub struct BscNativeVoteContextV1 {
    /// Target block number.
    #[norito(with = "crate::json_utils::u64_string")]
    pub target_number: u64,
    /// Target block hash.
    #[norito(with = "crate::json_utils::hex32")]
    pub target_hash: H256,
    /// Sorted validator roster active at the target's parent.
    pub validators: Vec<BscNativeValidatorV1>,
}
/// Epoch roster advertised by an epoch header but not yet activated.
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
pub struct BscNativePendingEpochV1 {
    /// Epoch checkpoint block number.
    #[norito(with = "crate::json_utils::u64_string")]
    pub checkpoint_number: u64,
    /// Authenticated epoch checkpoint block hash.
    #[norito(with = "crate::json_utils::hex32")]
    pub checkpoint_hash: H256,
    /// Sorted next validator roster carried by the checkpoint header.
    pub validators: Vec<BscNativeValidatorV1>,
    /// Next turn length carried by the checkpoint header.
    pub turn_length: u8,
}
/// Governed immutable Parlia checkpoint from which native replay begins.
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
pub struct BscNativeParliaAnchorV1 {
    /// Anchor schema version; the first release accepts exactly `1`.
    pub version: u8,
    /// Exact BSC network profile.
    pub network: SccpNetworkV1,
    /// Canonical full RLP of the checkpoint execution header.
    #[norito(with = "crate::json_utils::bytes_hex")]
    pub header_rlp: Vec<u8>,
    /// Active epoch length, fixed to the post-Maxwell protocol value.
    #[norito(with = "crate::json_utils::u64_string")]
    pub epoch_length: u64,
    /// Active block interval, fixed to the post-Fermi protocol value.
    #[norito(with = "crate::json_utils::u64_string")]
    pub block_interval_ms: u64,
    /// Active consecutive-block turn length.
    pub turn_length: u8,
    /// Active validators sorted by consensus address.
    pub validators: Vec<BscNativeValidatorV1>,
    /// Epoch checkpoint from which the active roster was installed.
    #[norito(with = "crate::json_utils::u64_string")]
    pub active_validator_checkpoint_number: u64,
    /// Canonical hash of the active roster's epoch checkpoint.
    #[norito(with = "crate::json_utils::hex32")]
    pub active_validator_checkpoint_hash: H256,
    /// Exact recent-proposer state after the checkpoint.
    pub recents: Vec<BscNativeRecentProposerV1>,
    /// Current source-finalized/target-justified state.
    pub justification: BscNativeJustificationV1,
    /// Recent target vote contexts, newest first.
    pub recent_vote_contexts: Vec<BscNativeVoteContextV1>,
    /// Unactivated epoch roster when the checkpoint lies inside a handover delay.
    pub pending_epoch: Option<BscNativePendingEpochV1>,
}
/// Consecutive native headers proving Parlia finality from one governed anchor.
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
pub struct BscNativeFinalityProofV1 {
    /// Proof schema version; the first release accepts exactly `1`.
    pub version: u8,
    /// Full governed anchor preimage.
    pub anchor: BscNativeParliaAnchorV1,
    /// Canonical full RLP headers beginning immediately after the anchor.
    #[norito(with = "crate::json_utils::vec_bytes_hex")]
    pub headers_rlp: Vec<Vec<u8>>,
    /// Zero-based proof header containing the SCCP receipt.
    pub target_header_index: u16,
}
/// Cheap deterministic reservation for native BSC finality verification.
///
/// The estimate is derived from proof framing only and performs no signature
/// recovery, public-key validation, or aggregate verification.  BLS values are
/// conservative upper bounds because deciding whether a canonical header
/// carries an attestation requires parsing its RLP.  Core can reserve this work
/// before dispatching any cryptographic verification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BscNativeFinalityWorkEstimateV1 {
    /// Number of post-anchor continuation headers.
    pub continuation_headers: u16,
    /// Bytes in the anchor header and all continuation headers.
    pub framed_header_bytes: u32,
    /// Maximum secp256k1 recoveries: one anchor seal plus every continuation.
    pub secp256k1_recoveries: u16,
    /// Maximum BLS aggregate checks, one optional attestation per continuation.
    pub bls_aggregate_checks_upper_bound: u16,
    /// Maximum aggregate public-key contributions at the 64-validator cap.
    pub bls_signer_contributions_upper_bound: u32,
}
/// Merkle-Patricia inclusion proof for one successful execution receipt.
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
pub struct BscNativeReceiptProofV1 {
    /// Zero-based receipt/transaction index in the block trie.
    #[norito(with = "crate::json_utils::u64_string")]
    pub transaction_index: u64,
    /// Exact legacy or EIP-2718 typed receipt bytes stored in the trie.
    #[norito(with = "crate::json_utils::bytes_hex")]
    pub receipt_bytes: Vec<u8>,
    /// Ordered MPT nodes from the receipts root to the receipt leaf.
    #[norito(with = "crate::json_utils::vec_bytes_hex")]
    pub proof_nodes: Vec<Vec<u8>>,
}
/// Account proof for the immutable concrete SCCP transfer-route contract.
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
pub struct BscNativeEmitterStateProofV1 {
    /// Ordered state-trie nodes proving the emitter account.
    #[norito(with = "crate::json_utils::vec_bytes_hex")]
    pub account_proof_nodes: Vec<Vec<u8>>,
}
/// Complete protocol-native BSC SCCP source proof.
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
pub struct BscNativeSourceProofV1 {
    /// Native Parlia finality continuation.
    pub finality: BscNativeFinalityProofV1,
    /// Successful receipt and receipts-trie inclusion proof.
    pub receipt: BscNativeReceiptProofV1,
    /// Finalized emitter account/runtime-code proof.
    pub emitter_state: BscNativeEmitterStateProofV1,
}
/// Authenticated finalized BSC execution fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValidatedBscNativeFinalityV1 {
    /// Governed anchor hash.
    pub anchor_hash: H256,
    /// Finalized target block number.
    pub block_number: u64,
    /// Finalized target block hash.
    pub block_hash: H256,
    /// Target state root.
    pub state_root: H256,
    /// Target receipts root.
    pub receipts_root: H256,
    /// Highest finalized block after replay.
    pub resulting_finalized_number: u64,
    /// Highest finalized block hash after replay.
    pub resulting_finalized_hash: H256,
}
/// Authenticated successful SCCP receipt fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValidatedBscNativeReceiptV1 {
    /// Receipt index authenticated by the MPT proof.
    pub transaction_index: u64,
    /// Governed emitter address found in the unique matching log.
    pub emitter: [u8; 20],
    /// Exact typed lane hash carried by the event.
    pub lane_hash: H256,
    /// Exact lane-bound message identifier carried by the event.
    pub message_id: H256,
    /// Hash of the exact canonical payload carried in event data.
    pub payload_hash: H256,
    /// Exact SCCP event digest carried by the event.
    pub source_event_digest: H256,
    /// Immutable concrete route configuration carried by the event.
    pub route_config_hash: H256,
}
/// Exact governed values that one native BSC receipt must authenticate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BscNativeReceiptExpectationV1<'a> {
    /// Finalized receipts root under which inclusion must be proven.
    pub receipts_root: H256,
    /// Immutable concrete transfer-route emitter address.
    pub emitter: [u8; 20],
    /// Exact typed lane hash carried by the source event.
    pub lane_hash: H256,
    /// Exact lane-bound message identifier carried by the source event.
    pub message_id: H256,
    /// Exact SCCP source-event digest carried by the source event.
    pub source_event_digest: H256,
    /// Hash of the exact canonical payload carried by the source event.
    pub payload_hash: H256,
    /// Immutable concrete route-configuration hash carried by the source event.
    pub route_config_hash: H256,
    /// Exact canonical SCCP payload bytes carried by the source event.
    pub canonical_payload: &'a [u8],
}
/// Authenticated direct emitter deployment fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValidatedBscNativeEmitterStateV1 {
    /// Account storage root opened from the finalized state root.
    pub storage_root: H256,
    /// Runtime code hash opened from the finalized account.
    pub runtime_code_hash: H256,
}
/// Authenticated result of a complete native BSC SCCP proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValidatedBscNativeSourceV1 {
    /// Exact governed typed source-identity hash.
    pub source_identity_hash: H256,
    /// Exact typed lane hash.
    pub lane_hash: H256,
    /// Native finality result.
    pub finality: ValidatedBscNativeFinalityV1,
    /// Successful receipt result.
    pub receipt: ValidatedBscNativeReceiptV1,
    /// Finalized emitter account result.
    pub emitter_state: ValidatedBscNativeEmitterStateV1,
}
/// Fail-closed reason returned by the native Parlia verifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BscNativeFinalityError {
    /// Proof or anchor schema version is unsupported.
    UnsupportedVersion,
    /// Network is not one of the two closed BSC profiles.
    WrongNetwork,
    /// The supplied governed anchor hash does not match its validated preimage.
    AnchorHashMismatch,
    /// Anchor serialization failed after semantic validation.
    AnchorEncoding,
    /// The governed anchor state is malformed or internally inconsistent.
    InvalidAnchor,
    /// Proof resource bounds were exceeded.
    ResourceLimit,
    /// Header RLP was non-canonical or had the wrong current-fork field layout.
    InvalidHeaderRlp,
    /// A header fell outside the closed post-Mendel fork window.
    UnsupportedFork,
    /// Header parent hash or number was not contiguous.
    NonContiguousHeader,
    /// Header timestamp or millisecond encoding violated Parlia rules.
    InvalidTimestamp,
    /// Header execution/gas/blob fields violated the current fork rules.
    InvalidExecutionFields,
    /// Parlia extra-data was malformed.
    InvalidExtraData,
    /// Proposer seal was malformed, non-canonical, or recovered the wrong address.
    InvalidProposerSeal,
    /// Proposer was not authorized by the active snapshot.
    UnauthorizedProposer,
    /// Proposer violated the recent-signing rule.
    RecentlySigned,
    /// Difficulty did not identify the snapshot's in-turn status.
    WrongDifficulty,
    /// Vote attestation RLP or source/target fields were invalid.
    InvalidAttestation,
    /// Vote bitmap was non-canonical or below the native quorum.
    InvalidVoteAddressSet,
    /// A vote public key or aggregate signature was invalid.
    InvalidBlsSignature,
    /// Epoch list was malformed, duplicated, unsorted, or conflicted with state.
    InvalidEpochRoster,
    /// A pending validator-set change was not finalized by the old set before use.
    UnauthenticatedEpochTransition,
    /// Target header index was absent or the target did not become finalized.
    TargetNotFinalized,
    /// The target became final before the last supplied continuation header.
    NonMinimalContinuation,
}
/// Fail-closed reason returned by receipt inclusion verification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BscNativeReceiptError {
    /// Receipt or proof resource bounds were exceeded.
    ResourceLimit,
    /// Merkle-Patricia inclusion failed.
    InvalidMptProof,
    /// Receipt type or RLP was unsupported/non-canonical.
    InvalidReceipt,
    /// Receipt status was not successful.
    FailedReceipt,
    /// Logs were malformed or did not contain one exact lane-bound SCCP event.
    InvalidSourceEvent,
}
/// Fail-closed reason returned by finalized emitter-state verification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BscNativeEmitterStateError {
    /// Account proof resource bounds were exceeded.
    ResourceLimit,
    /// Emitter account proof was absent, malformed, or failed inclusion.
    InvalidAccountProof,
    /// Finalized account code hash differed from governed identity.
    RuntimeCodeHashMismatch,
}
/// Fail-closed reason returned by complete native BSC source verification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BscNativeSourceError {
    /// Typed source identity was malformed or belonged to another chain family.
    InvalidSourceIdentity,
    /// Caller-supplied identity hash did not match the exact typed identity.
    SourceIdentityHashMismatch,
    /// Native Parlia finality verification failed.
    Finality(BscNativeFinalityError),
    /// Receipt inclusion/event verification failed.
    Receipt(BscNativeReceiptError),
    /// Finalized emitter state verification failed.
    EmitterState(BscNativeEmitterStateError),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct NetworkParameters {
    chain_id: u64,
    mendel_time: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ParsedHeader<'a> {
    fields: [RlpItem<'a>; 21],
    parent_hash: H256,
    coinbase: [u8; 20],
    state_root: H256,
    receipts_root: H256,
    difficulty: u64,
    number: u64,
    gas_limit: u64,
    gas_used: u64,
    time: u64,
    timestamp_ms: u64,
    extra: &'a [u8],
    blob_gas_used: u64,
    excess_blob_gas: u64,
    block_hash: H256,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct ParsedExtra {
    seal: [u8; 65],
    attestation: Option<VoteAttestation>,
    epoch: Option<PendingEpoch>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct VoteData {
    source_number: u64,
    source_hash: H256,
    target_number: u64,
    target_hash: H256,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct VoteAttestation {
    address_set: u64,
    aggregate_signature: [u8; 96],
    data: VoteData,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Validator {
    address: [u8; 20],
    vote_key: [u8; 48],
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct PendingEpoch {
    checkpoint_number: u64,
    checkpoint_hash: H256,
    validators: Vec<Validator>,
    turn_length: u8,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct VoteContext {
    target_number: u64,
    target_hash: H256,
    validators: Vec<Validator>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct ParliaState {
    number: u64,
    hash: H256,
    header: HeaderState,
    turn_length: u8,
    validators: Vec<Validator>,
    active_validator_checkpoint: CanonicalBlock,
    recents: BTreeMap<u64, [u8; 20]>,
    justification: VoteData,
    vote_contexts: Vec<VoteContext>,
    pending_epoch: Option<PendingEpoch>,
}
struct AnchorRosterState {
    validators: Vec<Validator>,
    active_validator_checkpoint: CanonicalBlock,
    current_epoch_checkpoint: u64,
    recents: BTreeMap<u64, [u8; 20]>,
}
struct AnchorVoteState {
    justification: VoteData,
    vote_contexts: Vec<VoteContext>,
    pending_epoch: Option<PendingEpoch>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct BscNativeFinalityWorkPerformedV1 {
    continuation_headers: u16,
    secp256k1_recoveries: u16,
    bls_aggregate_checks: u16,
    bls_signer_contributions: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HeaderState {
    number: u64,
    gas_limit: u64,
    gas_used: u64,
    time: u64,
    timestamp_ms: u64,
    blob_gas_used: u64,
    excess_blob_gas: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CanonicalBlock {
    number: u64,
    hash: H256,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RlpItem<'a> {
    raw: &'a [u8],
    payload: &'a [u8],
    is_list: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum MptReference {
    Hash(H256),
    Inline(Vec<u8>),
}
fn network_parameters(network: SccpNetworkV1) -> Option<NetworkParameters> {
    match network {
        SccpNetworkV1::BscMainnet => Some(NetworkParameters {
            chain_id: BSC_NATIVE_MAINNET_CHAIN_ID,
            mendel_time: BSC_NATIVE_MAINNET_MENDEL_TIME,
        }),
        SccpNetworkV1::BscTestnet => Some(NetworkParameters {
            chain_id: BSC_NATIVE_TESTNET_CHAIN_ID,
            mendel_time: BSC_NATIVE_TESTNET_MENDEL_TIME,
        }),
        _ => None,
    }
}
fn keccak256(bytes: &[u8]) -> H256 {
    let mut hash = [0_u8; 32];
    let mut hasher = Keccak::v256();
    hasher.update(bytes);
    hasher.finalize(&mut hash);
    hash
}
fn nonzero(bytes: &[u8]) -> bool {
    bytes.iter().any(|byte| *byte != 0)
}
fn read_be_usize(bytes: &[u8]) -> Option<usize> {
    if bytes.is_empty() || bytes[0] == 0 || bytes.len() > core::mem::size_of::<usize>() {
        return None;
    }
    bytes.iter().try_fold(0_usize, |value, byte| {
        value.checked_mul(256)?.checked_add(usize::from(*byte))
    })
}
fn parse_rlp_item(bytes: &[u8], offset: usize) -> Option<(RlpItem<'_>, usize)> {
    let first = *bytes.get(offset)?;
    let (payload_start, payload_len, is_list) = match first {
        0x00..=0x7f => (offset, 1_usize, false),
        0x80..=0xb7 => {
            let len = usize::from(first - 0x80);
            let start = offset.checked_add(1)?;
            let end = start.checked_add(len)?;
            let payload = bytes.get(start..end)?;
            if len == 1 && payload[0] < 0x80 {
                return None;
            }
            (start, len, false)
        }
        0xb8..=0xbf => {
            let len_of_len = usize::from(first - 0xb7);
            let len_start = offset.checked_add(1)?;
            let len_end = len_start.checked_add(len_of_len)?;
            let len = read_be_usize(bytes.get(len_start..len_end)?)?;
            if len < 56 {
                return None;
            }
            (len_end, len, false)
        }
        0xc0..=0xf7 => (offset.checked_add(1)?, usize::from(first - 0xc0), true),
        0xf8..=0xff => {
            let len_of_len = usize::from(first - 0xf7);
            let len_start = offset.checked_add(1)?;
            let len_end = len_start.checked_add(len_of_len)?;
            let len = read_be_usize(bytes.get(len_start..len_end)?)?;
            if len < 56 {
                return None;
            }
            (len_end, len, true)
        }
    };
    let payload_end = payload_start.checked_add(payload_len)?;
    let raw = bytes.get(offset..payload_end)?;
    let payload = bytes.get(payload_start..payload_end)?;
    Some((
        RlpItem {
            raw,
            payload,
            is_list,
        },
        payload_end,
    ))
}
fn parse_rlp_single(bytes: &[u8]) -> Option<RlpItem<'_>> {
    let (item, end) = parse_rlp_item(bytes, 0)?;
    (end == bytes.len()).then_some(item)
}
fn parse_rlp_list(bytes: &[u8]) -> Option<Vec<RlpItem<'_>>> {
    let outer = parse_rlp_single(bytes)?;
    if !outer.is_list {
        return None;
    }
    parse_rlp_list_payload(outer.payload)
}
fn parse_rlp_list_payload(payload: &[u8]) -> Option<Vec<RlpItem<'_>>> {
    let mut items = Vec::new();
    let mut cursor = 0_usize;
    while cursor < payload.len() {
        let (item, next) = parse_rlp_item(payload, cursor)?;
        if next <= cursor {
            return None;
        }
        items.push(item);
        cursor = next;
    }
    (cursor == payload.len()).then_some(items)
}
fn rlp_bytes(item: RlpItem<'_>) -> Option<&[u8]> {
    (!item.is_list).then_some(item.payload)
}
fn parse_rlp_u64(item: RlpItem<'_>) -> Option<u64> {
    let bytes = rlp_bytes(item)?;
    if bytes.len() > 8 || bytes.first() == Some(&0) {
        return None;
    }
    bytes.iter().try_fold(0_u64, |value, byte| {
        value.checked_mul(256)?.checked_add(u64::from(*byte))
    })
}
fn parse_fixed<const N: usize>(item: RlpItem<'_>) -> Option<[u8; N]> {
    rlp_bytes(item)?.try_into().ok()
}
fn rlp_length_prefix(length: usize, short: u8, long: u8) -> Option<Vec<u8>> {
    if length <= 55 {
        return Some(vec![short.checked_add(u8::try_from(length).ok()?)?]);
    }
    let encoded = length.to_be_bytes();
    let first = encoded.iter().position(|byte| *byte != 0)?;
    let significant = &encoded[first..];
    let mut out = Vec::with_capacity(1 + significant.len());
    out.push(long.checked_add(u8::try_from(significant.len()).ok()?)?);
    out.extend_from_slice(significant);
    Some(out)
}
fn rlp_encode_bytes(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.len() == 1 && bytes[0] < 0x80 {
        return Some(bytes.to_vec());
    }
    let mut out = rlp_length_prefix(bytes.len(), 0x80, 0xb7)?;
    out.extend_from_slice(bytes);
    Some(out)
}
fn rlp_encode_u64(value: u64) -> Option<Vec<u8>> {
    if value == 0 {
        return Some(vec![0x80]);
    }
    let bytes = value.to_be_bytes();
    let first = bytes.iter().position(|byte| *byte != 0)?;
    rlp_encode_bytes(&bytes[first..])
}
fn rlp_encode_list_raw(fields: &[&[u8]]) -> Option<Vec<u8>> {
    let payload_len = fields
        .iter()
        .try_fold(0_usize, |total, field| total.checked_add(field.len()))?;
    let mut out = rlp_length_prefix(payload_len, 0xc0, 0xf7)?;
    for field in fields {
        out.extend_from_slice(field);
    }
    Some(out)
}
fn parse_header(raw: &[u8]) -> Result<ParsedHeader<'_>, BscNativeFinalityError> {
    if raw.is_empty() || raw.len() > MAX_HEADER_BYTES {
        return Err(BscNativeFinalityError::ResourceLimit);
    }
    let fields = parse_rlp_list(raw).ok_or(BscNativeFinalityError::InvalidHeaderRlp)?;
    let fields: [RlpItem<'_>; 21] = fields
        .try_into()
        .map_err(|_| BscNativeFinalityError::InvalidHeaderRlp)?;
    let parent_hash = parse_fixed(fields[0]).ok_or(BscNativeFinalityError::InvalidHeaderRlp)?;
    let uncle_hash = parse_fixed(fields[1]).ok_or(BscNativeFinalityError::InvalidHeaderRlp)?;
    let coinbase = parse_fixed(fields[2]).ok_or(BscNativeFinalityError::InvalidHeaderRlp)?;
    let state_root = parse_fixed(fields[3]).ok_or(BscNativeFinalityError::InvalidHeaderRlp)?;
    let transactions_root: H256 =
        parse_fixed(fields[4]).ok_or(BscNativeFinalityError::InvalidHeaderRlp)?;
    let receipts_root = parse_fixed(fields[5]).ok_or(BscNativeFinalityError::InvalidHeaderRlp)?;
    if rlp_bytes(fields[6]).is_none_or(|bloom| bloom.len() != 256) {
        return Err(BscNativeFinalityError::InvalidHeaderRlp);
    }
    let difficulty = parse_rlp_u64(fields[7]).ok_or(BscNativeFinalityError::InvalidHeaderRlp)?;
    let number = parse_rlp_u64(fields[8]).ok_or(BscNativeFinalityError::InvalidHeaderRlp)?;
    let gas_limit = parse_rlp_u64(fields[9]).ok_or(BscNativeFinalityError::InvalidHeaderRlp)?;
    let gas_used = parse_rlp_u64(fields[10]).ok_or(BscNativeFinalityError::InvalidHeaderRlp)?;
    let time = parse_rlp_u64(fields[11]).ok_or(BscNativeFinalityError::InvalidHeaderRlp)?;
    let extra = rlp_bytes(fields[12]).ok_or(BscNativeFinalityError::InvalidHeaderRlp)?;
    if extra.len() > MAX_EXTRA_BYTES {
        return Err(BscNativeFinalityError::ResourceLimit);
    }
    let mix_digest: H256 =
        parse_fixed(fields[13]).ok_or(BscNativeFinalityError::InvalidHeaderRlp)?;
    let nonce: [u8; 8] = parse_fixed(fields[14]).ok_or(BscNativeFinalityError::InvalidHeaderRlp)?;
    let base_fee = parse_rlp_u64(fields[15]).ok_or(BscNativeFinalityError::InvalidHeaderRlp)?;
    let withdrawals_root: H256 =
        parse_fixed(fields[16]).ok_or(BscNativeFinalityError::InvalidHeaderRlp)?;
    let blob_gas_used =
        parse_rlp_u64(fields[17]).ok_or(BscNativeFinalityError::InvalidHeaderRlp)?;
    let excess_blob_gas =
        parse_rlp_u64(fields[18]).ok_or(BscNativeFinalityError::InvalidHeaderRlp)?;
    let parent_beacon_root: H256 =
        parse_fixed(fields[19]).ok_or(BscNativeFinalityError::InvalidHeaderRlp)?;
    let _requests_hash: H256 =
        parse_fixed(fields[20]).ok_or(BscNativeFinalityError::InvalidHeaderRlp)?;
    if uncle_hash != EMPTY_UNCLE_HASH
        || !nonzero(&parent_hash)
        || !nonzero(&coinbase)
        || !nonzero(&state_root)
        || !nonzero(&transactions_root)
        || !nonzero(&receipts_root)
        || nonce != [0; 8]
        || base_fee != 0
        || withdrawals_root != EMPTY_TRIE_HASH
        || parent_beacon_root != [0; 32]
        || mix_digest[..24] != [0; 24]
    {
        return Err(BscNativeFinalityError::InvalidExecutionFields);
    }
    let milliseconds = u64::from_be_bytes(
        mix_digest[24..]
            .try_into()
            .map_err(|_| BscNativeFinalityError::InvalidTimestamp)?,
    );
    if milliseconds >= 1_000 {
        return Err(BscNativeFinalityError::InvalidTimestamp);
    }
    let timestamp_ms = time
        .checked_mul(1_000)
        .and_then(|value| value.checked_add(milliseconds))
        .ok_or(BscNativeFinalityError::InvalidTimestamp)?;
    Ok(ParsedHeader {
        fields,
        parent_hash,
        coinbase,
        state_root,
        receipts_root,
        difficulty,
        number,
        gas_limit,
        gas_used,
        time,
        timestamp_ms,
        extra,
        blob_gas_used,
        excess_blob_gas,
        block_hash: keccak256(raw),
    })
}
fn verify_fork_window(
    header: &ParsedHeader<'_>,
    params: NetworkParameters,
) -> Result<(), BscNativeFinalityError> {
    if header.time < params.mendel_time {
        return Err(BscNativeFinalityError::UnsupportedFork);
    }
    Ok(())
}
fn verify_execution_fields(
    parent: HeaderState,
    header: &ParsedHeader<'_>,
) -> Result<(), BscNativeFinalityError> {
    if header.gas_limit > i64::MAX as u64
        || header.gas_used > header.gas_limit
        || header.gas_limit < MIN_GAS_LIMIT
    {
        return Err(BscNativeFinalityError::InvalidExecutionFields);
    }
    let gas_delta = parent.gas_limit.abs_diff(header.gas_limit);
    let gas_bound = parent.gas_limit / GAS_LIMIT_BOUND_DIVISOR;
    if gas_bound == 0 || gas_delta >= gas_bound {
        return Err(BscNativeFinalityError::InvalidExecutionFields);
    }
    let max_blob_gas = BLOB_MAX
        .checked_mul(BLOB_GAS_PER_BLOB)
        .ok_or(BscNativeFinalityError::InvalidExecutionFields)?;
    if header.blob_gas_used > max_blob_gas
        || !header.blob_gas_used.is_multiple_of(BLOB_GAS_PER_BLOB)
        || (!header.number.is_multiple_of(BLOB_ELIGIBLE_BLOCK_INTERVAL)
            && header.blob_gas_used != 0)
    {
        return Err(BscNativeFinalityError::InvalidExecutionFields);
    }
    let expected_excess = if parent.number.is_multiple_of(BLOB_ELIGIBLE_BLOCK_INTERVAL) {
        let target = BLOB_TARGET
            .checked_mul(BLOB_GAS_PER_BLOB)
            .ok_or(BscNativeFinalityError::InvalidExecutionFields)?;
        parent
            .excess_blob_gas
            .checked_add(parent.blob_gas_used)
            .ok_or(BscNativeFinalityError::InvalidExecutionFields)?
            .saturating_sub(target)
    } else {
        parent.excess_blob_gas
    };
    if header.excess_blob_gas != expected_excess {
        return Err(BscNativeFinalityError::InvalidExecutionFields);
    }
    Ok(())
}
fn validators_from_wire(
    validators: &[BscNativeValidatorV1],
) -> Result<Vec<Validator>, BscNativeFinalityError> {
    if validators.is_empty() || validators.len() > MAX_VALIDATORS {
        return Err(BscNativeFinalityError::InvalidEpochRoster);
    }
    let mut out = Vec::with_capacity(validators.len());
    let mut prior_address = None;
    let mut vote_keys = BTreeSet::new();
    for validator in validators {
        let address: [u8; 20] = validator
            .consensus_address
            .as_slice()
            .try_into()
            .map_err(|_| BscNativeFinalityError::InvalidEpochRoster)?;
        let vote_key: [u8; 48] = validator
            .vote_public_key
            .as_slice()
            .try_into()
            .map_err(|_| BscNativeFinalityError::InvalidEpochRoster)?;
        if !nonzero(&address)
            || prior_address.is_some_and(|prior| prior >= address)
            || !vote_keys.insert(vote_key)
        {
            return Err(BscNativeFinalityError::InvalidEpochRoster);
        }
        ethereum_bls_pop_validate_public_key(&vote_key)
            .map_err(|_| BscNativeFinalityError::InvalidBlsSignature)?;
        prior_address = Some(address);
        out.push(Validator { address, vote_key });
    }
    Ok(out)
}
fn validators_from_extra(bytes: &[u8]) -> Result<Vec<Validator>, BscNativeFinalityError> {
    if bytes.is_empty() || !bytes.len().is_multiple_of(VALIDATOR_BYTES) {
        return Err(BscNativeFinalityError::InvalidEpochRoster);
    }
    let count = bytes.len() / VALIDATOR_BYTES;
    if count == 0 || count > MAX_VALIDATORS {
        return Err(BscNativeFinalityError::InvalidEpochRoster);
    }
    let wire = bytes
        .chunks_exact(VALIDATOR_BYTES)
        .map(|entry| BscNativeValidatorV1 {
            consensus_address: entry[..20].to_vec(),
            vote_public_key: entry[20..].to_vec(),
        })
        .collect::<Vec<_>>();
    validators_from_wire(&wire)
}
fn parse_vote_data(item: RlpItem<'_>) -> Option<VoteData> {
    if !item.is_list {
        return None;
    }
    let fields = parse_rlp_list_payload(item.payload)?;
    if fields.len() != 4 {
        return None;
    }
    let data = VoteData {
        source_number: parse_rlp_u64(fields[0])?,
        source_hash: parse_fixed(fields[1])?,
        target_number: parse_rlp_u64(fields[2])?,
        target_hash: parse_fixed(fields[3])?,
    };
    (data.source_number < data.target_number
        && nonzero(&data.source_hash)
        && nonzero(&data.target_hash))
    .then_some(data)
}
fn parse_attestation(bytes: &[u8]) -> Result<VoteAttestation, BscNativeFinalityError> {
    let fields = parse_rlp_list(bytes).ok_or(BscNativeFinalityError::InvalidAttestation)?;
    if fields.len() != 4 {
        return Err(BscNativeFinalityError::InvalidAttestation);
    }
    let address_set = parse_rlp_u64(fields[0]).ok_or(BscNativeFinalityError::InvalidAttestation)?;
    let aggregate_signature: [u8; 96] =
        parse_fixed(fields[1]).ok_or(BscNativeFinalityError::InvalidAttestation)?;
    let data = parse_vote_data(fields[2]).ok_or(BscNativeFinalityError::InvalidAttestation)?;
    let extra = rlp_bytes(fields[3]).ok_or(BscNativeFinalityError::InvalidAttestation)?;
    if address_set == 0
        || !nonzero(&aggregate_signature)
        || extra.len() > MAX_ATTESTATION_EXTRA_BYTES
    {
        return Err(BscNativeFinalityError::InvalidAttestation);
    }
    Ok(VoteAttestation {
        address_set,
        aggregate_signature,
        data,
    })
}
fn parse_extra(header: &ParsedHeader<'_>) -> Result<ParsedExtra, BscNativeFinalityError> {
    let minimum = EXTRA_VANITY_BYTES
        .checked_add(EXTRA_SEAL_BYTES)
        .ok_or(BscNativeFinalityError::InvalidExtraData)?;
    if header.extra.len() < minimum {
        return Err(BscNativeFinalityError::InvalidExtraData);
    }
    let seal_start = header
        .extra
        .len()
        .checked_sub(EXTRA_SEAL_BYTES)
        .ok_or(BscNativeFinalityError::InvalidExtraData)?;
    let seal: [u8; 65] = header.extra[seal_start..]
        .try_into()
        .map_err(|_| BscNativeFinalityError::InvalidExtraData)?;
    let middle = &header.extra[EXTRA_VANITY_BYTES..seal_start];
    let (epoch, attestation_bytes) = if header.number.is_multiple_of(BSC_NATIVE_EPOCH_LENGTH) {
        let count = usize::from(
            *middle
                .first()
                .ok_or(BscNativeFinalityError::InvalidEpochRoster)?,
        );
        if count == 0 || count > MAX_VALIDATORS {
            return Err(BscNativeFinalityError::InvalidEpochRoster);
        }
        let roster_len = count
            .checked_mul(VALIDATOR_BYTES)
            .ok_or(BscNativeFinalityError::InvalidEpochRoster)?;
        let roster_start = 1_usize;
        let roster_end = roster_start
            .checked_add(roster_len)
            .ok_or(BscNativeFinalityError::InvalidEpochRoster)?;
        let turn_position = roster_end;
        let turn_length = *middle
            .get(turn_position)
            .ok_or(BscNativeFinalityError::InvalidEpochRoster)?;
        if turn_length == 0 || turn_length > MAX_TURN_LENGTH {
            return Err(BscNativeFinalityError::InvalidEpochRoster);
        }
        let validators = validators_from_extra(
            middle
                .get(roster_start..roster_end)
                .ok_or(BscNativeFinalityError::InvalidEpochRoster)?,
        )?;
        let attestation_start = turn_position
            .checked_add(1)
            .ok_or(BscNativeFinalityError::InvalidExtraData)?;
        (
            Some(PendingEpoch {
                checkpoint_number: header.number,
                checkpoint_hash: header.block_hash,
                validators,
                turn_length,
            }),
            middle
                .get(attestation_start..)
                .ok_or(BscNativeFinalityError::InvalidExtraData)?,
        )
    } else {
        (None, middle)
    };
    let attestation = if attestation_bytes.is_empty() {
        None
    } else {
        Some(parse_attestation(attestation_bytes)?)
    };
    Ok(ParsedExtra {
        seal,
        attestation,
        epoch,
    })
}
fn proposer_seal_hash(
    header: &ParsedHeader<'_>,
    chain_id: u64,
) -> Result<H256, BscNativeFinalityError> {
    let chain_id = rlp_encode_u64(chain_id).ok_or(BscNativeFinalityError::InvalidHeaderRlp)?;
    let extra_without_seal = header
        .extra
        .get(..header.extra.len().saturating_sub(EXTRA_SEAL_BYTES))
        .ok_or(BscNativeFinalityError::InvalidExtraData)?;
    let encoded_extra =
        rlp_encode_bytes(extra_without_seal).ok_or(BscNativeFinalityError::InvalidHeaderRlp)?;
    let mut fields = Vec::with_capacity(22);
    fields.push(chain_id.as_slice());
    for (index, field) in header.fields.iter().enumerate() {
        if index == 12 {
            fields.push(encoded_extra.as_slice());
        } else {
            fields.push(field.raw);
        }
    }
    let encoded = rlp_encode_list_raw(&fields).ok_or(BscNativeFinalityError::InvalidHeaderRlp)?;
    Ok(keccak256(&encoded))
}
fn recover_proposer(
    header: &ParsedHeader<'_>,
    seal: &[u8; 65],
    chain_id: u64,
) -> Result<[u8; 20], BscNativeFinalityError> {
    if !matches!(seal[64], 0 | 1) {
        return Err(BscNativeFinalityError::InvalidProposerSeal);
    }
    let mut normalized = *seal;
    normalized[64] = normalized[64]
        .checked_add(27)
        .ok_or(BscNativeFinalityError::InvalidProposerSeal)?;
    let digest = proposer_seal_hash(header, chain_id)?;
    let public_key = EcdsaSecp256k1Sha256::recover_public_key_from_prehash(&digest, &normalized)
        .map_err(|_| BscNativeFinalityError::InvalidProposerSeal)?;
    Ok(EcdsaSecp256k1Sha256::evm_address(&public_key))
}
fn vote_data_hash(data: VoteData) -> Result<H256, BscNativeFinalityError> {
    let source_number =
        rlp_encode_u64(data.source_number).ok_or(BscNativeFinalityError::InvalidAttestation)?;
    let source_hash =
        rlp_encode_bytes(&data.source_hash).ok_or(BscNativeFinalityError::InvalidAttestation)?;
    let target_number =
        rlp_encode_u64(data.target_number).ok_or(BscNativeFinalityError::InvalidAttestation)?;
    let target_hash =
        rlp_encode_bytes(&data.target_hash).ok_or(BscNativeFinalityError::InvalidAttestation)?;
    let encoded =
        rlp_encode_list_raw(&[&source_number, &source_hash, &target_number, &target_hash])
            .ok_or(BscNativeFinalityError::InvalidAttestation)?;
    Ok(keccak256(&encoded))
}
fn vote_context_from_wire(
    context: &BscNativeVoteContextV1,
) -> Result<VoteContext, BscNativeFinalityError> {
    if !nonzero(&context.target_hash) {
        return Err(BscNativeFinalityError::InvalidAnchor);
    }
    Ok(VoteContext {
        target_number: context.target_number,
        target_hash: context.target_hash,
        validators: validators_from_wire(&context.validators)?,
    })
}
fn pending_epoch_from_wire(
    pending: &BscNativePendingEpochV1,
) -> Result<PendingEpoch, BscNativeFinalityError> {
    if pending.checkpoint_number == 0
        || !pending
            .checkpoint_number
            .is_multiple_of(BSC_NATIVE_EPOCH_LENGTH)
        || !nonzero(&pending.checkpoint_hash)
        || pending.turn_length == 0
        || pending.turn_length > MAX_TURN_LENGTH
    {
        return Err(BscNativeFinalityError::InvalidAnchor);
    }
    Ok(PendingEpoch {
        checkpoint_number: pending.checkpoint_number,
        checkpoint_hash: pending.checkpoint_hash,
        validators: validators_from_wire(&pending.validators)?,
        turn_length: pending.turn_length,
    })
}
fn miner_history_check_len(validator_count: usize, turn_length: u8) -> Option<u64> {
    let majority = u64::try_from(validator_count)
        .ok()?
        .checked_div(2)?
        .checked_add(1)?;
    majority.checked_mul(u64::from(turn_length))?.checked_sub(1)
}
fn parse_and_validate_anchor_header(
    anchor: &BscNativeParliaAnchorV1,
    params: NetworkParameters,
) -> Result<(ParsedHeader<'_>, ParsedExtra), BscNativeFinalityError> {
    if anchor.epoch_length != BSC_NATIVE_EPOCH_LENGTH
        || anchor.block_interval_ms != BSC_NATIVE_BLOCK_INTERVAL_MS
        || anchor.turn_length == 0
        || anchor.turn_length > MAX_TURN_LENGTH
        || anchor.recent_vote_contexts.is_empty()
        || anchor.recent_vote_contexts.len() > BSC_NATIVE_ATTESTATION_ANCESTOR_DEPTH
    {
        return Err(BscNativeFinalityError::InvalidAnchor);
    }
    let header = parse_header(&anchor.header_rlp)?;
    verify_fork_window(&header, params)?;
    let parsed_extra = parse_extra(&header)?;
    let recovered = recover_proposer(&header, &parsed_extra.seal, params.chain_id)?;
    if recovered != header.coinbase {
        return Err(BscNativeFinalityError::InvalidAnchor);
    }
    Ok((header, parsed_extra))
}
fn validate_anchor_roster(
    anchor: &BscNativeParliaAnchorV1,
    header: &ParsedHeader<'_>,
) -> Result<AnchorRosterState, BscNativeFinalityError> {
    let validators = validators_from_wire(&anchor.validators)?;
    let active_validator_checkpoint = CanonicalBlock {
        number: anchor.active_validator_checkpoint_number,
        hash: anchor.active_validator_checkpoint_hash,
    };
    let current_epoch_checkpoint = header.number - header.number % BSC_NATIVE_EPOCH_LENGTH;
    if active_validator_checkpoint.number == 0
        || !active_validator_checkpoint
            .number
            .is_multiple_of(BSC_NATIVE_EPOCH_LENGTH)
        || active_validator_checkpoint.number > header.number
        || active_validator_checkpoint
            .number
            .saturating_add(BSC_NATIVE_EPOCH_LENGTH)
            < current_epoch_checkpoint
        || !nonzero(&active_validator_checkpoint.hash)
        || validators
            .binary_search_by_key(&header.coinbase, |validator| validator.address)
            .is_err()
    {
        return Err(BscNativeFinalityError::InvalidAnchor);
    }
    let history_len = miner_history_check_len(validators.len(), anchor.turn_length)
        .ok_or(BscNativeFinalityError::InvalidAnchor)?;
    if anchor.recents.len() > usize::try_from(history_len.saturating_add(1)).unwrap_or(usize::MAX) {
        return Err(BscNativeFinalityError::InvalidAnchor);
    }
    let left_bound = header.number.saturating_sub(history_len);
    let mut recents = BTreeMap::new();
    let mut counts = BTreeMap::<[u8; 20], u8>::new();
    for recent in &anchor.recents {
        let address: [u8; 20] = recent
            .consensus_address
            .as_slice()
            .try_into()
            .map_err(|_| BscNativeFinalityError::InvalidAnchor)?;
        if recent.block_number <= left_bound
            || recent.block_number > header.number
            || validators
                .binary_search_by_key(&address, |validator| validator.address)
                .is_err()
            || recents.insert(recent.block_number, address).is_some()
        {
            return Err(BscNativeFinalityError::InvalidAnchor);
        }
        let count = counts.entry(address).or_default();
        *count = count
            .checked_add(1)
            .ok_or(BscNativeFinalityError::InvalidAnchor)?;
        if *count > anchor.turn_length {
            return Err(BscNativeFinalityError::InvalidAnchor);
        }
    }
    Ok(AnchorRosterState {
        validators,
        active_validator_checkpoint,
        current_epoch_checkpoint,
        recents,
    })
}
fn validate_anchor_votes(
    anchor: &BscNativeParliaAnchorV1,
    header: &ParsedHeader<'_>,
    parsed_extra: &ParsedExtra,
    roster: &AnchorRosterState,
) -> Result<AnchorVoteState, BscNativeFinalityError> {
    let justification = VoteData {
        source_number: anchor.justification.source_number,
        source_hash: anchor.justification.source_hash,
        target_number: anchor.justification.target_number,
        target_hash: anchor.justification.target_hash,
    };
    let source_not_before_target = justification.source_number >= justification.target_number;
    let target_beyond_anchor = justification.target_number > header.number;
    if source_not_before_target
        || target_beyond_anchor
        || !nonzero(&justification.source_hash)
        || !nonzero(&justification.target_hash)
        || (roster.active_validator_checkpoint.number == justification.source_number
            && roster.active_validator_checkpoint.hash != justification.source_hash)
        || (roster.active_validator_checkpoint.number == justification.target_number
            && roster.active_validator_checkpoint.hash != justification.target_hash)
    {
        return Err(BscNativeFinalityError::InvalidAnchor);
    }
    let mut vote_contexts = Vec::with_capacity(anchor.recent_vote_contexts.len());
    let mut prior_number = None;
    for wire in &anchor.recent_vote_contexts {
        let context = vote_context_from_wire(wire)?;
        if context.target_number > header.number
            || prior_number.is_some_and(|prior| prior <= context.target_number)
        {
            return Err(BscNativeFinalityError::InvalidAnchor);
        }
        prior_number = Some(context.target_number);
        vote_contexts.push(context);
    }
    if vote_contexts[0].target_number != header.number
        || vote_contexts[0].target_hash != header.block_hash
    {
        return Err(BscNativeFinalityError::InvalidAnchor);
    }
    let pending_epoch = anchor
        .pending_epoch
        .as_ref()
        .map(pending_epoch_from_wire)
        .transpose()?;
    if header.number.is_multiple_of(BSC_NATIVE_EPOCH_LENGTH)
        && pending_epoch.as_ref() != parsed_extra.epoch.as_ref()
    {
        return Err(BscNativeFinalityError::InvalidAnchor);
    }
    if pending_epoch.as_ref().is_some_and(|pending| {
        pending.checkpoint_number > header.number
            || header.number.saturating_sub(pending.checkpoint_number) >= BSC_NATIVE_EPOCH_LENGTH
            || pending.checkpoint_number != roster.current_epoch_checkpoint
            || roster.active_validator_checkpoint.number == pending.checkpoint_number
    }) || (pending_epoch.is_none()
        && roster.active_validator_checkpoint.number != roster.current_epoch_checkpoint)
    {
        return Err(BscNativeFinalityError::InvalidAnchor);
    }
    Ok(AnchorVoteState {
        justification,
        vote_contexts,
        pending_epoch,
    })
}
fn anchor_state(
    anchor: &BscNativeParliaAnchorV1,
) -> Result<(ParliaState, H256, NetworkParameters), BscNativeFinalityError> {
    if anchor.version != 1 {
        return Err(BscNativeFinalityError::UnsupportedVersion);
    }
    let params = network_parameters(anchor.network).ok_or(BscNativeFinalityError::WrongNetwork)?;
    let (header, parsed_extra) = parse_and_validate_anchor_header(anchor, params)?;
    let roster = validate_anchor_roster(anchor, &header)?;
    let votes = validate_anchor_votes(anchor, &header, &parsed_extra, &roster)?;
    let anchor_bytes =
        norito::to_bytes(anchor).map_err(|_| BscNativeFinalityError::AnchorEncoding)?;
    let anchor_hash = prefixed_blake2b(BSC_NATIVE_ANCHOR_PREFIX_V1, &anchor_bytes);
    Ok((
        ParliaState {
            number: header.number,
            hash: header.block_hash,
            header: HeaderState {
                number: header.number,
                gas_limit: header.gas_limit,
                gas_used: header.gas_used,
                time: header.time,
                timestamp_ms: header.timestamp_ms,
                blob_gas_used: header.blob_gas_used,
                excess_blob_gas: header.excess_blob_gas,
            },
            turn_length: anchor.turn_length,
            validators: roster.validators,
            active_validator_checkpoint: roster.active_validator_checkpoint,
            recents: roster.recents,
            justification: votes.justification,
            vote_contexts: votes.vote_contexts,
            pending_epoch: votes.pending_epoch,
        },
        anchor_hash,
        params,
    ))
}
/// Return the canonical execution block number of a valid governed Parlia anchor.
///
/// # Errors
///
/// Returns a finality error when the anchor header is malformed or non-canonical.
pub fn bsc_native_anchor_block_number(
    anchor: &BscNativeParliaAnchorV1,
) -> Result<u64, BscNativeFinalityError> {
    let header = parse_header(&anchor.header_rlp)?;
    Ok(header.number)
}
// Seed table and generator used by Go 1's `math/rand.NewSource`. Parlia's
// out-of-turn delay is consensus-visible and therefore cannot be replaced by
// Rust's RNG or by a statistically equivalent shuffle.
const GO_RNG_COOKED_BYTES: &[u8; 4_856] = include_bytes!("assets/go_rng_cooked_i64le_v1.bin");
const fn decode_go_rng_cooked(bytes: &[u8; 4_856]) -> [i64; 607] {
    let mut values = [0_i64; 607];
    let mut index = 0;
    while index < 607 {
        let offset = index * 8;
        values[index] = i64::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]);
        index += 1;
    }
    values
}
const GO_RNG_COOKED: [i64; 607] = decode_go_rng_cooked(GO_RNG_COOKED_BYTES);
struct GoMathRand {
    tap: usize,
    feed: usize,
    values: [i64; 607],
}
impl GoMathRand {
    fn new(seed: u64) -> Result<Self, BscNativeFinalityError> {
        const INT32_MAX: u64 = (1_u64 << 31) - 1;
        let mut normalized =
            i64::try_from(seed % INT32_MAX).map_err(|_| BscNativeFinalityError::ResourceLimit)?;
        if normalized == 0 {
            normalized = 89_482_311;
        }
        let mut x = i32::try_from(normalized).map_err(|_| BscNativeFinalityError::ResourceLimit)?;
        let mut values = [0_i64; 607];
        for index in -20_i32..607_i32 {
            x = go_seed_rand(x)?;
            if index >= 0 {
                let mut value = i64::from(x) << 40;
                x = go_seed_rand(x)?;
                value ^= i64::from(x) << 20;
                x = go_seed_rand(x)?;
                value ^= i64::from(x);
                let slot =
                    usize::try_from(index).map_err(|_| BscNativeFinalityError::ResourceLimit)?;
                values[slot] = value ^ GO_RNG_COOKED[slot];
            }
        }
        Ok(Self {
            tap: 0,
            feed: 607 - 273,
            values,
        })
    }
    fn int63(&mut self) -> u64 {
        self.tap = if self.tap == 0 { 606 } else { self.tap - 1 };
        self.feed = if self.feed == 0 { 606 } else { self.feed - 1 };
        let value = self.values[self.feed].wrapping_add(self.values[self.tap]);
        self.values[self.feed] = value;
        u64::from_ne_bytes(value.to_ne_bytes()) & ((1_u64 << 63) - 1)
    }
    fn uint32(&mut self) -> Result<u32, BscNativeFinalityError> {
        u32::try_from(self.int63() >> 31).map_err(|_| BscNativeFinalityError::ResourceLimit)
    }
    fn int31n(&mut self, n: u32) -> Result<u32, BscNativeFinalityError> {
        if n == 0 {
            return Err(BscNativeFinalityError::InvalidAnchor);
        }
        let mut value = self.uint32()?;
        let mut product = u64::from(value) * u64::from(n);
        let [low_0, low_1, low_2, low_3, _, _, _, _] = product.to_le_bytes();
        let mut low = u32::from_le_bytes([low_0, low_1, low_2, low_3]);
        if low < n {
            let threshold = n.wrapping_neg() % n;
            while low < threshold {
                value = self.uint32()?;
                product = u64::from(value) * u64::from(n);
                let [low_0, low_1, low_2, low_3, _, _, _, _] = product.to_le_bytes();
                low = u32::from_le_bytes([low_0, low_1, low_2, low_3]);
            }
        }
        u32::try_from(product >> 32).map_err(|_| BscNativeFinalityError::ResourceLimit)
    }
}
fn go_seed_rand(value: i32) -> Result<i32, BscNativeFinalityError> {
    const A: i64 = 48_271;
    const Q: i32 = 44_488;
    const R: i64 = 3_399;
    const INT32_MAX: i64 = (1_i64 << 31) - 1;
    let high = value / Q;
    let low = value % Q;
    let mut next = A * i64::from(low) - R * i64::from(high);
    if next < 0 {
        next += INT32_MAX;
    }
    i32::try_from(next).map_err(|_| BscNativeFinalityError::ResourceLimit)
}
fn go_math_rand_shuffle(seed: u64, values: &mut [u64]) -> Result<(), BscNativeFinalityError> {
    let mut rng = GoMathRand::new(seed)?;
    for index in (1..values.len()).rev() {
        let n = u32::try_from(index + 1).map_err(|_| BscNativeFinalityError::ResourceLimit)?;
        let swap_with =
            usize::try_from(rng.int31n(n)?).map_err(|_| BscNativeFinalityError::ResourceLimit)?;
        values.swap(index, swap_with);
    }
    Ok(())
}
fn recent_counts(state: &ParliaState) -> Result<BTreeMap<[u8; 20], u8>, BscNativeFinalityError> {
    let history_len = miner_history_check_len(state.validators.len(), state.turn_length)
        .ok_or(BscNativeFinalityError::InvalidAnchor)?;
    let left_bound = state.number.saturating_sub(history_len);
    let mut counts = BTreeMap::new();
    for (number, address) in &state.recents {
        if *number <= left_bound {
            continue;
        }
        let count = counts.entry(*address).or_insert(0_u8);
        *count = count.saturating_add(1);
    }
    Ok(counts)
}
fn signed_recently(counts: &BTreeMap<[u8; 20], u8>, address: [u8; 20], turn_length: u8) -> bool {
    counts
        .get(&address)
        .is_some_and(|count| *count >= turn_length)
}
fn in_turn_validator(state: &ParliaState) -> Option<[u8; 20]> {
    let validator_count = u64::try_from(state.validators.len()).ok()?;
    let offset = state
        .number
        .checked_add(1)?
        .checked_div(u64::from(state.turn_length))?
        % validator_count;
    state
        .validators
        .get(usize::try_from(offset).ok()?)
        .map(|validator| validator.address)
}
fn parlia_backoff_ms(
    state: &ParliaState,
    header_number: u64,
    proposer: [u8; 20],
) -> Result<u64, BscNativeFinalityError> {
    let in_turn = in_turn_validator(state).ok_or(BscNativeFinalityError::InvalidAnchor)?;
    if proposer == in_turn {
        return Ok(0);
    }
    let counts = recent_counts(state)?;
    if signed_recently(&counts, proposer, state.turn_length) {
        return Err(BscNativeFinalityError::RecentlySigned);
    }
    let in_turn_recent = signed_recently(&counts, in_turn, state.turn_length);
    let mut eligible = Vec::with_capacity(state.validators.len());
    for validator in &state.validators {
        if validator.address == in_turn
            || signed_recently(&counts, validator.address, state.turn_length)
        {
            continue;
        }
        eligible.push(validator.address);
    }
    let proposer_index = eligible
        .iter()
        .position(|address| *address == proposer)
        .ok_or(BscNativeFinalityError::RecentlySigned)?;
    let seed = header_number
        .checked_div(u64::from(state.turn_length))
        .ok_or(BscNativeFinalityError::InvalidTimestamp)?;
    let mut steps = (0..u64::try_from(eligible.len())
        .map_err(|_| BscNativeFinalityError::InvalidTimestamp)?)
        .collect::<Vec<_>>();
    go_math_rand_shuffle(seed, &mut steps)?;
    let step = *steps
        .get(proposer_index)
        .ok_or(BscNativeFinalityError::InvalidTimestamp)?;
    if in_turn_recent {
        if step == 0 {
            Ok(0)
        } else {
            2_000_u64
                .checked_add(
                    step.checked_sub(1)
                        .and_then(|value| value.checked_mul(1_000))
                        .ok_or(BscNativeFinalityError::InvalidTimestamp)?,
                )
                .ok_or(BscNativeFinalityError::InvalidTimestamp)
        }
    } else {
        2_000_u64
            .checked_add(
                step.checked_mul(1_000)
                    .ok_or(BscNativeFinalityError::InvalidTimestamp)?,
            )
            .ok_or(BscNativeFinalityError::InvalidTimestamp)
    }
}
fn find_canonical_block(blocks: &[CanonicalBlock], number: u64, hash: H256) -> bool {
    blocks
        .iter()
        .any(|block| block.number == number && block.hash == hash)
}
fn verify_attestation(
    state: &ParliaState,
    attestation: &VoteAttestation,
    canonical_blocks: &[CanonicalBlock],
) -> Result<(), BscNativeFinalityError> {
    if attestation.data.source_number != state.justification.target_number
        || attestation.data.source_hash != state.justification.target_hash
    {
        return Err(BscNativeFinalityError::InvalidAttestation);
    }
    let parent_number = state.number;
    if attestation.data.target_number > parent_number
        || parent_number.saturating_sub(attestation.data.target_number)
            >= u64::try_from(BSC_NATIVE_ATTESTATION_ANCESTOR_DEPTH)
                .map_err(|_| BscNativeFinalityError::InvalidAttestation)?
        || !find_canonical_block(
            canonical_blocks,
            attestation.data.target_number,
            attestation.data.target_hash,
        )
    {
        return Err(BscNativeFinalityError::InvalidAttestation);
    }
    let context = state
        .vote_contexts
        .iter()
        .find(|context| {
            context.target_number == attestation.data.target_number
                && context.target_hash == attestation.data.target_hash
        })
        .ok_or(BscNativeFinalityError::InvalidAttestation)?;
    let validator_count = context.validators.len();
    if validator_count < 64 && (attestation.address_set >> validator_count) != 0 {
        return Err(BscNativeFinalityError::InvalidVoteAddressSet);
    }
    let quorum = validator_count
        .checked_mul(2)
        .and_then(|value| value.checked_add(2))
        .and_then(|value| value.checked_div(3))
        .ok_or(BscNativeFinalityError::InvalidVoteAddressSet)?;
    let mut public_keys = Vec::with_capacity(validator_count);
    for (index, validator) in context.validators.iter().enumerate() {
        if (attestation.address_set & (1_u64 << index)) != 0 {
            public_keys.push(validator.vote_key);
        }
    }
    if public_keys.len() < quorum {
        return Err(BscNativeFinalityError::InvalidVoteAddressSet);
    }
    let message = vote_data_hash(attestation.data)?;
    ethereum_bls_pop_fast_aggregate_verify(
        &public_keys,
        &message,
        &attestation.aggregate_signature,
    )
    .map_err(|_| BscNativeFinalityError::InvalidBlsSignature)?;
    Ok(())
}
fn update_justification(state: &mut ParliaState, data: VoteData) {
    if data.source_number.checked_add(1) == Some(data.target_number) {
        state.justification = data;
    } else {
        state.justification.target_number = data.target_number;
        state.justification.target_hash = data.target_hash;
    }
}
fn checkpoint_is_finalized(
    state: &ParliaState,
    pending: &PendingEpoch,
    canonical_blocks: &[CanonicalBlock],
) -> bool {
    state.justification.source_number >= pending.checkpoint_number
        && (state.justification.source_number != pending.checkpoint_number
            || state.justification.source_hash == pending.checkpoint_hash)
        && (pending.checkpoint_number <= canonical_blocks[0].number
            || find_canonical_block(
                canonical_blocks,
                pending.checkpoint_number,
                pending.checkpoint_hash,
            ))
}
fn validate_header_candidate(
    state: &ParliaState,
    header: &ParsedHeader<'_>,
    params: NetworkParameters,
) -> Result<(ParsedExtra, [u8; 20]), BscNativeFinalityError> {
    if header.number
        != state
            .number
            .checked_add(1)
            .ok_or(BscNativeFinalityError::NonContiguousHeader)?
        || header.parent_hash != state.hash
    {
        return Err(BscNativeFinalityError::NonContiguousHeader);
    }
    verify_fork_window(header, params)?;
    verify_execution_fields(state.header, header)?;
    let extra = parse_extra(header)?;
    let proposer = recover_proposer(header, &extra.seal, params.chain_id)?;
    if proposer != header.coinbase {
        return Err(BscNativeFinalityError::InvalidProposerSeal);
    }
    if state
        .validators
        .binary_search_by_key(&proposer, |validator| validator.address)
        .is_err()
    {
        return Err(BscNativeFinalityError::UnauthorizedProposer);
    }
    let counts = recent_counts(state)?;
    if signed_recently(&counts, proposer, state.turn_length) {
        return Err(BscNativeFinalityError::RecentlySigned);
    }
    let in_turn = in_turn_validator(state).ok_or(BscNativeFinalityError::InvalidAnchor)?;
    let expected_difficulty = if proposer == in_turn { 2 } else { 1 };
    if header.difficulty != expected_difficulty {
        return Err(BscNativeFinalityError::WrongDifficulty);
    }
    let backoff = parlia_backoff_ms(state, header.number, proposer)?;
    let minimum_timestamp = state
        .header
        .timestamp_ms
        .checked_add(BSC_NATIVE_BLOCK_INTERVAL_MS)
        .and_then(|value| value.checked_add(backoff))
        .ok_or(BscNativeFinalityError::InvalidTimestamp)?;
    if header.timestamp_ms < minimum_timestamp || header.time < state.header.time {
        return Err(BscNativeFinalityError::InvalidTimestamp);
    }
    Ok((extra, proposer))
}
fn apply_header(
    state: &mut ParliaState,
    header: &ParsedHeader<'_>,
    params: NetworkParameters,
    canonical_blocks: &mut Vec<CanonicalBlock>,
) -> Result<Option<u32>, BscNativeFinalityError> {
    let (extra, proposer) = validate_header_candidate(state, header, params)?;
    let bls_signer_contributions = if let Some(attestation) = &extra.attestation {
        verify_attestation(state, attestation, canonical_blocks)?;
        Some(attestation.address_set.count_ones())
    } else {
        None
    };
    if header.number.is_multiple_of(BSC_NATIVE_EPOCH_LENGTH) {
        if state.pending_epoch.is_some() {
            return Err(BscNativeFinalityError::InvalidEpochRoster);
        }
        state.pending_epoch.clone_from(&extra.epoch);
    } else if extra.epoch.is_some() {
        return Err(BscNativeFinalityError::InvalidEpochRoster);
    }
    let old_history = miner_history_check_len(state.validators.len(), state.turn_length)
        .ok_or(BscNativeFinalityError::InvalidAnchor)?;
    if let Some(expired) = header.number.checked_sub(old_history.saturating_add(1)) {
        state.recents.remove(&expired);
    }
    state.recents.insert(header.number, proposer);
    if let Some(attestation) = extra.attestation {
        update_justification(state, attestation.data);
    }
    state
        .recents
        .retain(|number, _| *number > state.justification.source_number);
    let context = VoteContext {
        target_number: header.number,
        target_hash: header.block_hash,
        validators: state.validators.clone(),
    };
    state.vote_contexts.insert(0, context);
    state
        .vote_contexts
        .truncate(BSC_NATIVE_ATTESTATION_ANCESTOR_DEPTH);
    canonical_blocks.push(CanonicalBlock {
        number: header.number,
        hash: header.block_hash,
    });
    if header.number % BSC_NATIVE_EPOCH_LENGTH == old_history {
        if let Some(pending) = state.pending_epoch.take() {
            let expected_checkpoint = header
                .number
                .checked_sub(old_history)
                .ok_or(BscNativeFinalityError::UnauthenticatedEpochTransition)?;
            if pending.checkpoint_number != expected_checkpoint
                || !checkpoint_is_finalized(state, &pending, canonical_blocks)
            {
                return Err(BscNativeFinalityError::UnauthenticatedEpochTransition);
            }
            state.active_validator_checkpoint = CanonicalBlock {
                number: pending.checkpoint_number,
                hash: pending.checkpoint_hash,
            };
            state.validators = pending.validators;
            state.turn_length = pending.turn_length;
            state.recents.clear();
        } else {
            let current_epoch = header.number - header.number % BSC_NATIVE_EPOCH_LENGTH;
            if state.active_validator_checkpoint.number != current_epoch {
                return Err(BscNativeFinalityError::UnauthenticatedEpochTransition);
            }
        }
    }
    state.number = header.number;
    state.hash = header.block_hash;
    state.header = HeaderState {
        number: header.number,
        gas_limit: header.gas_limit,
        gas_used: header.gas_used,
        time: header.time,
        timestamp_ms: header.timestamp_ms,
        blob_gas_used: header.blob_gas_used,
        excess_blob_gas: header.excess_blob_gas,
    };
    Ok(bls_signer_contributions)
}
/// Hash a semantically valid governed native Parlia anchor.
///
/// # Errors
///
/// Returns [`BscNativeFinalityError`] when the anchor is malformed, violates
/// the configured Parlia rules, or exceeds a verifier resource bound.
pub fn bsc_native_anchor_hash(
    anchor: &BscNativeParliaAnchorV1,
) -> Result<H256, BscNativeFinalityError> {
    let (_, hash, _) = anchor_state(anchor)?;
    Ok(hash)
}
/// Return a cryptography-free upper bound for one native BSC finality proof.
///
/// The shape policy admits at most one full post-anchor epoch before the
/// target and at most the four-header native fast-finality suffix after it.
/// This function reads only vector lengths and byte lengths, making it suitable
/// for consensus quota reservation before header parsing or signature work.
///
/// # Errors
///
/// Returns [`BscNativeFinalityError::ResourceLimit`] when a count or byte bound
/// is exceeded, or [`BscNativeFinalityError::TargetNotFinalized`] when the
/// target index is outside the supplied continuation.
pub fn bsc_native_finality_work_estimate(
    proof: &BscNativeFinalityProofV1,
) -> Result<BscNativeFinalityWorkEstimateV1, BscNativeFinalityError> {
    let header_count = proof.headers_rlp.len();
    if header_count == 0 || header_count > BSC_NATIVE_MAX_FINALITY_HEADERS {
        return Err(BscNativeFinalityError::ResourceLimit);
    }
    let target_index = usize::from(proof.target_header_index);
    if target_index >= header_count {
        return Err(BscNativeFinalityError::TargetNotFinalized);
    }
    let suffix_headers = header_count
        .checked_sub(target_index)
        .and_then(|count| count.checked_sub(1))
        .ok_or(BscNativeFinalityError::ResourceLimit)?;
    if target_index >= BSC_NATIVE_MAX_TARGET_HEADERS
        || suffix_headers > BSC_NATIVE_MAX_FINALITY_SUFFIX_HEADERS
        || proof.anchor.header_rlp.len() > MAX_HEADER_BYTES
        || proof
            .headers_rlp
            .iter()
            .any(|header| header.len() > MAX_HEADER_BYTES)
    {
        return Err(BscNativeFinalityError::ResourceLimit);
    }
    let framed_header_bytes = proof
        .headers_rlp
        .iter()
        .try_fold(proof.anchor.header_rlp.len(), |total, header| {
            total.checked_add(header.len())
        })
        .and_then(|total| u32::try_from(total).ok())
        .ok_or(BscNativeFinalityError::ResourceLimit)?;
    let continuation_headers =
        u16::try_from(header_count).map_err(|_| BscNativeFinalityError::ResourceLimit)?;
    let secp256k1_recoveries = continuation_headers
        .checked_add(1)
        .ok_or(BscNativeFinalityError::ResourceLimit)?;
    let bls_signer_contributions_upper_bound = u32::from(continuation_headers)
        .checked_mul(
            u32::try_from(MAX_VALIDATORS).map_err(|_| BscNativeFinalityError::ResourceLimit)?,
        )
        .ok_or(BscNativeFinalityError::ResourceLimit)?;
    Ok(BscNativeFinalityWorkEstimateV1 {
        continuation_headers,
        framed_header_bytes,
        secp256k1_recoveries,
        bls_aggregate_checks_upper_bound: continuation_headers,
        bls_signer_contributions_upper_bound,
    })
}
fn bsc_target_is_finalized(
    state: &ParliaState,
    target_number: u64,
    canonical_blocks: &[CanonicalBlock],
) -> bool {
    state.justification.source_number >= target_number
        && find_canonical_block(
            canonical_blocks,
            state.justification.source_number,
            state.justification.source_hash,
        )
}
fn verify_bsc_native_finality_counted(
    proof: &BscNativeFinalityProofV1,
    expected_network: SccpNetworkV1,
    expected_anchor_hash: H256,
    work: &mut BscNativeFinalityWorkPerformedV1,
) -> Result<ValidatedBscNativeFinalityV1, BscNativeFinalityError> {
    if proof.version != 1 || proof.anchor.version != 1 {
        return Err(BscNativeFinalityError::UnsupportedVersion);
    }
    if proof.anchor.network != expected_network {
        return Err(BscNativeFinalityError::WrongNetwork);
    }
    let _ = bsc_native_finality_work_estimate(proof)?;
    let target_index = usize::from(proof.target_header_index);
    work.secp256k1_recoveries = work.secp256k1_recoveries.saturating_add(1);
    let (mut state, anchor_hash, params) = anchor_state(&proof.anchor)?;
    if anchor_hash != expected_anchor_hash {
        return Err(BscNativeFinalityError::AnchorHashMismatch);
    }
    let mut canonical_blocks = vec![CanonicalBlock {
        number: state.number,
        hash: state.hash,
    }];
    for context in state.vote_contexts.iter().skip(1) {
        canonical_blocks.push(CanonicalBlock {
            number: context.target_number,
            hash: context.target_hash,
        });
    }
    let mut target = None;
    for (index, raw) in proof.headers_rlp.iter().enumerate() {
        let header = parse_header(raw)?;
        work.continuation_headers = work.continuation_headers.saturating_add(1);
        work.secp256k1_recoveries = work.secp256k1_recoveries.saturating_add(1);
        if index == target_index {
            target = Some((
                header.number,
                header.block_hash,
                header.state_root,
                header.receipts_root,
            ));
        }
        let bls_signer_contributions =
            apply_header(&mut state, &header, params, &mut canonical_blocks)?;
        if let Some(contributions) = bls_signer_contributions {
            work.bls_aggregate_checks = work.bls_aggregate_checks.saturating_add(1);
            work.bls_signer_contributions =
                work.bls_signer_contributions.saturating_add(contributions);
        }
        if let Some((block_number, block_hash, state_root, receipts_root)) = target
            && bsc_target_is_finalized(&state, block_number, &canonical_blocks)
        {
            if index + 1 != proof.headers_rlp.len() {
                return Err(BscNativeFinalityError::NonMinimalContinuation);
            }
            return Ok(ValidatedBscNativeFinalityV1 {
                anchor_hash,
                block_number,
                block_hash,
                state_root,
                receipts_root,
                resulting_finalized_number: state.justification.source_number,
                resulting_finalized_hash: state.justification.source_hash,
            });
        }
    }
    Err(BscNativeFinalityError::TargetNotFinalized)
}
/// Verify native Parlia finality for one proof target.
///
/// # Errors
///
/// Returns [`BscNativeFinalityError`] when the proof is malformed, exceeds a
/// resource bound, is not anchored to the expected network and checkpoint, or
/// does not prove that the selected header was finalized by Parlia.
pub fn verify_bsc_native_finality(
    proof: &BscNativeFinalityProofV1,
    expected_network: SccpNetworkV1,
    expected_anchor_hash: H256,
) -> Result<ValidatedBscNativeFinalityV1, BscNativeFinalityError> {
    verify_bsc_native_finality_counted(
        proof,
        expected_network,
        expected_anchor_hash,
        &mut BscNativeFinalityWorkPerformedV1::default(),
    )
}
fn mpt_proof_is_bounded(nodes: &[Vec<u8>]) -> bool {
    !nodes.is_empty()
        && nodes.len() <= MAX_MPT_NODES
        && nodes
            .iter()
            .all(|node| !node.is_empty() && node.len() <= MAX_MPT_NODE_BYTES)
        && nodes
            .iter()
            .try_fold(0_usize, |total, node| total.checked_add(node.len()))
            .is_some_and(|total| total <= MAX_MPT_TOTAL_BYTES)
}
fn bytes_to_nibbles(bytes: &[u8]) -> Vec<u8> {
    let mut nibbles = Vec::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        nibbles.push(byte >> 4);
        nibbles.push(byte & 0x0f);
    }
    nibbles
}
fn decode_compact_path(bytes: &[u8]) -> Option<(bool, Vec<u8>)> {
    let nibbles = bytes_to_nibbles(bytes);
    let flag = *nibbles.first()?;
    if flag > 3 {
        return None;
    }
    let is_leaf = flag & 2 != 0;
    let odd = flag & 1 != 0;
    let path = if odd {
        nibbles.get(1..)?.to_vec()
    } else {
        if nibbles.get(1) != Some(&0) {
            return None;
        }
        nibbles.get(2..)?.to_vec()
    };
    if !is_leaf && path.is_empty() {
        return None;
    }
    Some((is_leaf, path))
}
fn mpt_child_reference(item: RlpItem<'_>) -> Option<MptReference> {
    if item.is_list {
        if item.raw.is_empty() || item.raw.len() >= 32 {
            return None;
        }
        return Some(MptReference::Inline(item.raw.to_vec()));
    }
    let hash: H256 = item.payload.try_into().ok()?;
    nonzero(&hash).then_some(MptReference::Hash(hash))
}
fn verify_mpt_inclusion(root: H256, key: &[u8], nodes: &[Vec<u8>]) -> Option<Vec<u8>> {
    if !nonzero(&root) || !mpt_proof_is_bounded(nodes) {
        return None;
    }
    let key = bytes_to_nibbles(key);
    let mut key_cursor = 0_usize;
    let mut node_cursor = 0_usize;
    let mut expected = MptReference::Hash(root);
    let mut inline = None::<Vec<u8>>;
    let mut steps = 0_usize;
    loop {
        steps = steps.checked_add(1)?;
        if steps > nodes.len().checked_add(MAX_MPT_NODES)? {
            return None;
        }
        let node = if let Some(inline) = inline.take() {
            inline
        } else {
            let node = nodes.get(node_cursor)?.clone();
            node_cursor = node_cursor.checked_add(1)?;
            node
        };
        match &expected {
            MptReference::Hash(hash) if keccak256(&node) != *hash => return None,
            MptReference::Inline(raw) if node != *raw => return None,
            _ => {}
        }
        let fields = parse_rlp_list(&node)?;
        expected = match fields.len() {
            17 => {
                if key_cursor == key.len() {
                    let value = rlp_bytes(fields[16])?;
                    return (!value.is_empty() && node_cursor == nodes.len())
                        .then(|| value.to_vec());
                }
                let nibble = usize::from(*key.get(key_cursor)?);
                key_cursor = key_cursor.checked_add(1)?;
                mpt_child_reference(fields[nibble])?
            }
            2 => {
                let compact = rlp_bytes(fields[0])?;
                let (leaf, path) = decode_compact_path(compact)?;
                let remaining = key.get(key_cursor..)?;
                if !remaining.starts_with(&path) {
                    return None;
                }
                key_cursor = key_cursor.checked_add(path.len())?;
                if leaf {
                    if key_cursor != key.len() {
                        return None;
                    }
                    let value = rlp_bytes(fields[1])?;
                    return (!value.is_empty() && node_cursor == nodes.len())
                        .then(|| value.to_vec());
                }
                mpt_child_reference(fields[1])?
            }
            _ => return None,
        };
        if let MptReference::Inline(raw) = &expected {
            inline = Some(raw.clone());
        }
    }
}
fn rlp_integer_is_canonical(item: RlpItem<'_>, max_bytes: usize) -> bool {
    rlp_bytes(item).is_some_and(|bytes| {
        bytes.len() <= max_bytes && bytes.first().is_none_or(|first| *first != 0)
    })
}
fn receipt_payload(receipt: &[u8]) -> Option<&[u8]> {
    let first = *receipt.first()?;
    if (1..=4).contains(&first) {
        return receipt.get(1..);
    }
    (first >= 0xc0).then_some(receipt)
}
fn verify_receipt_event(
    receipt: &[u8],
    expected: &BscNativeReceiptExpectationV1<'_>,
) -> Result<(), BscNativeReceiptError> {
    if receipt.is_empty() || receipt.len() > MAX_RECEIPT_BYTES {
        return Err(BscNativeReceiptError::ResourceLimit);
    }
    let payload = receipt_payload(receipt).ok_or(BscNativeReceiptError::InvalidReceipt)?;
    let fields = parse_rlp_list(payload).ok_or(BscNativeReceiptError::InvalidReceipt)?;
    if fields.len() != 4
        || !rlp_integer_is_canonical(fields[0], 1)
        || !rlp_integer_is_canonical(fields[1], 8)
        || rlp_bytes(fields[2]).is_none_or(|bloom| bloom.len() != 256)
        || !fields[3].is_list
    {
        return Err(BscNativeReceiptError::InvalidReceipt);
    }
    if parse_rlp_u64(fields[0]) != Some(1) {
        return Err(BscNativeReceiptError::FailedReceipt);
    }
    let logs =
        parse_rlp_list_payload(fields[3].payload).ok_or(BscNativeReceiptError::InvalidReceipt)?;
    if logs.len() > MAX_RECEIPT_LOGS {
        return Err(BscNativeReceiptError::ResourceLimit);
    }
    let event_topic = keccak256(BSC_NATIVE_EVENT_ABI_V1);
    let mut matched = false;
    for log in logs {
        if !log.is_list {
            return Err(BscNativeReceiptError::InvalidReceipt);
        }
        let fields =
            parse_rlp_list_payload(log.payload).ok_or(BscNativeReceiptError::InvalidReceipt)?;
        if fields.len() != 3 {
            return Err(BscNativeReceiptError::InvalidReceipt);
        }
        let address: [u8; 20] = rlp_bytes(fields[0])
            .and_then(|address| address.try_into().ok())
            .ok_or(BscNativeReceiptError::InvalidReceipt)?;
        if !fields[1].is_list {
            return Err(BscNativeReceiptError::InvalidReceipt);
        }
        let topics = parse_rlp_list_payload(fields[1].payload)
            .ok_or(BscNativeReceiptError::InvalidReceipt)?;
        if topics.len() > MAX_LOG_TOPICS {
            return Err(BscNativeReceiptError::InvalidReceipt);
        }
        let mut parsed_topics = Vec::with_capacity(topics.len());
        for topic in topics {
            let topic: H256 = rlp_bytes(topic)
                .and_then(|topic| topic.try_into().ok())
                .ok_or(BscNativeReceiptError::InvalidReceipt)?;
            parsed_topics.push(topic);
        }
        let data = rlp_bytes(fields[2]).ok_or(BscNativeReceiptError::InvalidReceipt)?;
        if address == expected.emitter && parsed_topics.first() == Some(&event_topic) {
            if parsed_topics.as_slice()
                != [
                    event_topic,
                    expected.lane_hash,
                    expected.message_id,
                    expected.source_event_digest,
                ]
                || !canonical_transfer_event_data_matches(
                    data,
                    expected.payload_hash,
                    expected.route_config_hash,
                    expected.canonical_payload,
                )
            {
                return Err(BscNativeReceiptError::InvalidSourceEvent);
            }
            if matched {
                return Err(BscNativeReceiptError::InvalidSourceEvent);
            }
            matched = true;
        }
    }
    if !matched {
        return Err(BscNativeReceiptError::InvalidSourceEvent);
    }
    Ok(())
}
fn canonical_transfer_event_data_matches(
    data: &[u8],
    payload_hash: H256,
    route_config_hash: H256,
    canonical_payload: &[u8],
) -> bool {
    if data.len() < 128
        || data.get(..32) != Some(payload_hash.as_slice())
        || data.get(32..64) != Some(route_config_hash.as_slice())
    {
        return false;
    }
    let Some(offset) = data.get(64..96) else {
        return false;
    };
    if offset[..31].iter().any(|byte| *byte != 0) || offset[31] != 96 {
        return false;
    }
    let Some(length) = data.get(96..128) else {
        return false;
    };
    if length[..24].iter().any(|byte| *byte != 0) {
        return false;
    }
    let mut raw_len = [0u8; 8];
    raw_len.copy_from_slice(&length[24..]);
    let Ok(payload_len) = usize::try_from(u64::from_be_bytes(raw_len)) else {
        return false;
    };
    if payload_len != canonical_payload.len() {
        return false;
    }
    let Some(padded_len) = payload_len.checked_add(31).map(|len| len & !31) else {
        return false;
    };
    let Some(expected_len) = 128usize.checked_add(padded_len) else {
        return false;
    };
    data.len() == expected_len
        && data.get(128..128 + payload_len) == Some(canonical_payload)
        && data[128 + payload_len..].iter().all(|byte| *byte == 0)
}
/// Verify one successful, exactly lane-bound SCCP receipt under a finalized receipts root.
///
/// # Errors
///
/// Returns [`BscNativeReceiptError`] when the receipt or Merkle-Patricia proof
/// is malformed, exceeds a resource bound, is not included under the supplied
/// root, or lacks exactly one expected SCCP source event.
pub fn verify_bsc_native_receipt(
    proof: &BscNativeReceiptProofV1,
    expected: &BscNativeReceiptExpectationV1<'_>,
) -> Result<ValidatedBscNativeReceiptV1, BscNativeReceiptError> {
    if proof.receipt_bytes.is_empty()
        || proof.receipt_bytes.len() > MAX_RECEIPT_BYTES
        || !mpt_proof_is_bounded(&proof.proof_nodes)
    {
        return Err(BscNativeReceiptError::ResourceLimit);
    }
    let key =
        rlp_encode_u64(proof.transaction_index).ok_or(BscNativeReceiptError::InvalidMptProof)?;
    let value = verify_mpt_inclusion(expected.receipts_root, &key, &proof.proof_nodes)
        .ok_or(BscNativeReceiptError::InvalidMptProof)?;
    if value != proof.receipt_bytes {
        return Err(BscNativeReceiptError::InvalidMptProof);
    }
    verify_receipt_event(&proof.receipt_bytes, expected)?;
    Ok(ValidatedBscNativeReceiptV1 {
        transaction_index: proof.transaction_index,
        emitter: expected.emitter,
        lane_hash: expected.lane_hash,
        message_id: expected.message_id,
        payload_hash: expected.payload_hash,
        source_event_digest: expected.source_event_digest,
        route_config_hash: expected.route_config_hash,
    })
}
fn parse_account(value: &[u8]) -> Option<(H256, H256)> {
    let fields = parse_rlp_list(value)?;
    if fields.len() != 4
        || !rlp_integer_is_canonical(fields[0], 32)
        || !rlp_integer_is_canonical(fields[1], 32)
    {
        return None;
    }
    let storage_root: H256 = parse_fixed(fields[2])?;
    let code_hash: H256 = parse_fixed(fields[3])?;
    (nonzero(&storage_root) && nonzero(&code_hash)).then_some((storage_root, code_hash))
}
/// Verify the immutable concrete emitter account and runtime code hash.
///
/// # Errors
///
/// Returns [`BscNativeEmitterStateError`] when the Merkle-Patricia proof is
/// malformed or unbounded, the account is absent, or its runtime code hash
/// does not match the expected source identity.
pub fn verify_bsc_native_emitter_state(
    proof: &BscNativeEmitterStateProofV1,
    state_root: H256,
    emitter: [u8; 20],
    expected_runtime_code_hash: H256,
) -> Result<ValidatedBscNativeEmitterStateV1, BscNativeEmitterStateError> {
    if !mpt_proof_is_bounded(&proof.account_proof_nodes) {
        return Err(BscNativeEmitterStateError::ResourceLimit);
    }
    let account_key = keccak256(&emitter);
    let account_value = verify_mpt_inclusion(state_root, &account_key, &proof.account_proof_nodes)
        .ok_or(BscNativeEmitterStateError::InvalidAccountProof)?;
    let (storage_root, runtime_code_hash) =
        parse_account(&account_value).ok_or(BscNativeEmitterStateError::InvalidAccountProof)?;
    if runtime_code_hash != expected_runtime_code_hash {
        return Err(BscNativeEmitterStateError::RuntimeCodeHashMismatch);
    }
    Ok(ValidatedBscNativeEmitterStateV1 {
        storage_root,
        runtime_code_hash,
    })
}
/// Verify native BSC finality, receipt inclusion, typed lane binding, and emitter state.
///
/// # Errors
///
/// Returns [`BscNativeSourceError`] when the typed source identity is invalid
/// or when any finality, receipt-inclusion, lane-binding, or emitter-state
/// verification step fails.
pub fn verify_bsc_native_source(
    proof: &BscNativeSourceProofV1,
    source_identity: &SccpSourceIdentityV1,
    expected_source_identity_hash: H256,
    expected_anchor_hash: H256,
    expected_message_id: H256,
    expected_payload_hash: H256,
    canonical_payload: &[u8],
) -> Result<ValidatedBscNativeSourceV1, BscNativeSourceError> {
    if !source_identity.is_well_formed()
        || !matches!(
            source_identity.lane.source,
            SccpNetworkV1::BscMainnet | SccpNetworkV1::BscTestnet
        )
        || !nonzero(&expected_message_id)
        || !nonzero(&expected_payload_hash)
        || canonical_payload.is_empty()
        || super::payload_hash(canonical_payload) != expected_payload_hash
    {
        return Err(BscNativeSourceError::InvalidSourceIdentity);
    }
    let SccpSourceEmitterV1::Evm(emitter) = source_identity.emitter else {
        return Err(BscNativeSourceError::InvalidSourceIdentity);
    };
    let decoded_payload = decode_canonical_sccp_payload_bytes(canonical_payload)
        .ok_or(BscNativeSourceError::InvalidSourceIdentity)?;
    if !matches!(decoded_payload, SccpPayloadV1::Transfer(_))
        || canonical_sccp_payload_bytes(&decoded_payload)
            .ok()
            .as_deref()
            != Some(canonical_payload)
        || sccp_message_id(source_identity.lane, &decoded_payload) != Some(expected_message_id)
    {
        return Err(BscNativeSourceError::InvalidSourceIdentity);
    }
    let source_identity_hash = sccp_source_identity_hash_v1(source_identity)
        .ok_or(BscNativeSourceError::InvalidSourceIdentity)?;
    if source_identity_hash != expected_source_identity_hash {
        return Err(BscNativeSourceError::SourceIdentityHashMismatch);
    }
    let lane_hash = sccp_lane_id_hash_v1(source_identity.lane)
        .ok_or(BscNativeSourceError::InvalidSourceIdentity)?;
    let source_event_digest = super::sccp_lane_source_event_digest_v1(
        source_identity.lane,
        expected_message_id,
        expected_payload_hash,
    )
    .ok_or(BscNativeSourceError::InvalidSourceIdentity)?;
    let finality = verify_bsc_native_finality(
        &proof.finality,
        source_identity.lane.source,
        expected_anchor_hash,
    )
    .map_err(BscNativeSourceError::Finality)?;
    let receipt_expectation = BscNativeReceiptExpectationV1 {
        receipts_root: finality.receipts_root,
        emitter: emitter.address,
        lane_hash,
        message_id: expected_message_id,
        source_event_digest,
        payload_hash: expected_payload_hash,
        route_config_hash: emitter.route_config_hash,
        canonical_payload,
    };
    let receipt = verify_bsc_native_receipt(&proof.receipt, &receipt_expectation)
        .map_err(BscNativeSourceError::Receipt)?;
    let emitter_state = verify_bsc_native_emitter_state(
        &proof.emitter_state,
        finality.state_root,
        emitter.address,
        emitter.runtime_code_hash,
    )
    .map_err(BscNativeSourceError::EmitterState)?;
    Ok(ValidatedBscNativeSourceV1 {
        source_identity_hash,
        lane_hash,
        finality,
        receipt,
        emitter_state,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    const BLS_GENERATOR: [u8; 48] = [
        0x97, 0xf1, 0xd3, 0xa7, 0x31, 0x97, 0xd7, 0x94, 0x26, 0x95, 0x63, 0x8c, 0x4f, 0xa9, 0xac,
        0x0f, 0xc3, 0x68, 0x8c, 0x4f, 0x97, 0x74, 0xb9, 0x05, 0xa1, 0x4e, 0x3a, 0x3f, 0x17, 0x1b,
        0xac, 0x58, 0x6c, 0x55, 0xe8, 0x3f, 0xf9, 0x7a, 0x1a, 0xef, 0xfb, 0x3a, 0xf0, 0x0a, 0xdb,
        0x22, 0xc6, 0xbb,
    ];
    fn bytes(value: &[u8]) -> Vec<u8> {
        rlp_encode_bytes(value).expect("test RLP bytes")
    }
    fn uint(value: u64) -> Vec<u8> {
        rlp_encode_u64(value).expect("test RLP integer")
    }
    fn list(fields: &[Vec<u8>]) -> Vec<u8> {
        let refs = fields.iter().map(Vec::as_slice).collect::<Vec<_>>();
        rlp_encode_list_raw(&refs).expect("test RLP list")
    }
    fn signer_address() -> [u8; 20] {
        let signing_key =
            EcdsaSecp256k1Sha256::parse_private_key(&[7_u8; 32]).expect("test signing key");
        EcdsaSecp256k1Sha256::evm_address(&signing_key.public_key())
    }
    struct HeaderEncodingInput<'a> {
        parent_hash: H256,
        number: u64,
        timestamp_ms: u64,
        difficulty: u64,
        coinbase: [u8; 20],
        middle_extra: &'a [u8],
    }
    #[derive(Clone, Copy)]
    struct HeaderEncodingOptions {
        seal: [u8; 65],
        gas_limit: u64,
        gas_used: u64,
        blob_gas_used: u64,
        excess_blob_gas: u64,
    }
    impl HeaderEncodingOptions {
        fn with_seal(seal: [u8; 65]) -> Self {
            Self {
                seal,
                gas_limit: 100_000_000,
                gas_used: 21_000,
                blob_gas_used: 0,
                excess_blob_gas: 0,
            }
        }
    }
    fn encode_header(input: &HeaderEncodingInput<'_>, options: HeaderEncodingOptions) -> Vec<u8> {
        let mut extra = vec![0_u8; EXTRA_VANITY_BYTES];
        extra.extend_from_slice(input.middle_extra);
        extra.extend_from_slice(&options.seal);
        let time = input.timestamp_ms / 1_000;
        let mut mix = [0_u8; 32];
        mix[24..].copy_from_slice(&(input.timestamp_ms % 1_000).to_be_bytes());
        list(&[
            bytes(&input.parent_hash),
            bytes(&EMPTY_UNCLE_HASH),
            bytes(&input.coinbase),
            bytes(&[0x22; 32]),
            bytes(&[0x33; 32]),
            bytes(&[0x44; 32]),
            bytes(&[0_u8; 256]),
            uint(input.difficulty),
            uint(input.number),
            uint(options.gas_limit),
            uint(options.gas_used),
            uint(time),
            bytes(&extra),
            bytes(&mix),
            bytes(&[0_u8; 8]),
            uint(0),
            bytes(&EMPTY_TRIE_HASH),
            uint(options.blob_gas_used),
            uint(options.excess_blob_gas),
            bytes(&[0_u8; 32]),
            bytes(&[0x55; 32]),
        ])
    }
    fn signed_header(
        parent_hash: H256,
        number: u64,
        timestamp_ms: u64,
        difficulty: u64,
        middle_extra: &[u8],
    ) -> Vec<u8> {
        let coinbase = signer_address();
        let input = HeaderEncodingInput {
            parent_hash,
            number,
            timestamp_ms,
            difficulty,
            coinbase,
            middle_extra,
        };
        let unsigned = encode_header(&input, HeaderEncodingOptions::with_seal([0; 65]));
        let parsed = parse_header(&unsigned).expect("unsigned test header parses");
        let digest =
            proposer_seal_hash(&parsed, BSC_NATIVE_MAINNET_CHAIN_ID).expect("test seal hash");
        let signing_key =
            EcdsaSecp256k1Sha256::parse_private_key(&[7_u8; 32]).expect("test signing key");
        let mut seal = EcdsaSecp256k1Sha256::sign_prehash_recoverable(&digest, &signing_key)
            .expect("test proposer signature");
        seal[64] -= 27;
        encode_header(&input, HeaderEncodingOptions::with_seal(seal))
    }
    fn validator(address: [u8; 20]) -> BscNativeValidatorV1 {
        BscNativeValidatorV1 {
            consensus_address: address.to_vec(),
            vote_public_key: BLS_GENERATOR.to_vec(),
        }
    }
    fn anchor() -> BscNativeParliaAnchorV1 {
        let number = 1_001;
        let header_rlp = signed_header(
            [0x11; 32],
            number,
            BSC_NATIVE_MAINNET_MENDEL_TIME * 1_000,
            2,
            &[],
        );
        let header_hash = keccak256(&header_rlp);
        let validator = validator(signer_address());
        BscNativeParliaAnchorV1 {
            version: 1,
            network: SccpNetworkV1::BscMainnet,
            header_rlp,
            epoch_length: BSC_NATIVE_EPOCH_LENGTH,
            block_interval_ms: BSC_NATIVE_BLOCK_INTERVAL_MS,
            turn_length: 1,
            validators: vec![validator.clone()],
            active_validator_checkpoint_number: 1_000,
            active_validator_checkpoint_hash: [0xaa; 32],
            recents: vec![],
            justification: BscNativeJustificationV1 {
                source_number: 999,
                source_hash: [0x99; 32],
                target_number: 1_000,
                target_hash: [0xaa; 32],
            },
            recent_vote_contexts: vec![BscNativeVoteContextV1 {
                target_number: number,
                target_hash: header_hash,
                validators: vec![validator],
            }],
            pending_epoch: None,
        }
    }
    fn compact_leaf_path(key: &[u8]) -> Vec<u8> {
        let nibbles = bytes_to_nibbles(key);
        assert_eq!(nibbles.len() % 2, 0);
        let mut out = Vec::with_capacity(1 + key.len());
        out.push(0x20);
        for pair in nibbles.chunks_exact(2) {
            out.push((pair[0] << 4) | pair[1]);
        }
        out
    }
    fn singleton_mpt(key: &[u8], value: &[u8]) -> (H256, Vec<Vec<u8>>) {
        let leaf = list(&[bytes(&compact_leaf_path(key)), bytes(value)]);
        (keccak256(&leaf), vec![leaf])
    }
    fn test_payload() -> &'static [u8] {
        b"\x02\x01bsc-canonical-transfer"
    }
    fn test_message_id() -> H256 {
        [0x66; 32]
    }
    fn test_route_config_hash() -> H256 {
        [0x67; 32]
    }
    fn source_event_data() -> Vec<u8> {
        let payload = test_payload();
        let padded_len = payload.len().checked_add(31).unwrap() & !31;
        let mut out = Vec::with_capacity(128 + padded_len);
        out.extend_from_slice(&crate::payload_hash(payload));
        out.extend_from_slice(&test_route_config_hash());
        out.extend_from_slice(&[0; 31]);
        out.push(96);
        let mut length = [0u8; 32];
        length[24..].copy_from_slice(&u64::try_from(payload.len()).unwrap().to_be_bytes());
        out.extend_from_slice(&length);
        out.extend_from_slice(payload);
        out.resize(128 + padded_len, 0);
        out
    }
    fn source_receipt(
        emitter: [u8; 20],
        lane_hash: H256,
        digest: H256,
        status: u64,
        duplicate: bool,
    ) -> Vec<u8> {
        let event_topic = keccak256(BSC_NATIVE_EVENT_ABI_V1);
        let topics = list(&[
            bytes(&event_topic),
            bytes(&lane_hash),
            bytes(&test_message_id()),
            bytes(&digest),
        ]);
        let log = list(&[bytes(&emitter), topics, bytes(&source_event_data())]);
        let logs = if duplicate {
            list(&[log.clone(), log])
        } else {
            list(&[log])
        };
        list(&[uint(status), uint(21_000), bytes(&[0_u8; 256]), logs])
    }
    fn verify_test_bsc_receipt(
        proof: &BscNativeReceiptProofV1,
        root: H256,
        emitter: [u8; 20],
        lane_hash: H256,
        digest: H256,
    ) -> Result<ValidatedBscNativeReceiptV1, BscNativeReceiptError> {
        let expected = BscNativeReceiptExpectationV1 {
            receipts_root: root,
            emitter,
            lane_hash,
            message_id: test_message_id(),
            source_event_digest: digest,
            payload_hash: crate::payload_hash(test_payload()),
            route_config_hash: test_route_config_hash(),
            canonical_payload: test_payload(),
        };
        verify_bsc_native_receipt(proof, &expected)
    }
    fn hex_bytes(hex: &str) -> Vec<u8> {
        assert_eq!(hex.len() % 2, 0);
        hex.as_bytes()
            .chunks_exact(2)
            .map(|pair| {
                let digit = |byte: u8| match byte {
                    b'0'..=b'9' => byte - b'0',
                    b'a'..=b'f' => byte - b'a' + 10,
                    _ => panic!("test vector is lowercase hexadecimal"),
                };
                (digit(pair[0]) << 4) | digit(pair[1])
            })
            .collect()
    }
    fn attestation_bytes(data: VoteData, signature_hex: &str) -> Vec<u8> {
        let signature = hex_bytes(signature_hex);
        assert_eq!(signature.len(), 96);
        let vote_data = list(&[
            uint(data.source_number),
            bytes(&data.source_hash),
            uint(data.target_number),
            bytes(&data.target_hash),
        ]);
        list(&[uint(1), bytes(&signature), vote_data, bytes(&[])])
    }
    #[test]
    fn fork_schedule_and_chain_ids_match_pinned_bsc_config() {
        let mainnet = network_parameters(SccpNetworkV1::BscMainnet).unwrap();
        assert_eq!(mainnet.chain_id, 56);
        assert_eq!(mainnet.mendel_time, 1_777_343_400);
        let testnet = network_parameters(SccpNetworkV1::BscTestnet).unwrap();
        assert_eq!(testnet.chain_id, 97);
        assert_eq!(testnet.mendel_time, 1_774_319_400);
        let post_mendel_future =
            signed_header([0x11; 32], 1_001, 1_900_000_000_u64 * 1_000, 2, &[]);
        let post_mendel_future = parse_header(&post_mendel_future).unwrap();
        assert_eq!(verify_fork_window(&post_mendel_future, testnet), Ok(()));
        assert!(network_parameters(SccpNetworkV1::EthereumMainnet).is_none());
    }
    #[test]
    fn canonical_rlp_rejects_short_aliases_long_aliases_and_trailing_bytes() {
        assert!(parse_rlp_single(&[0x81, 0x01]).is_none());
        assert!(parse_rlp_single(&[0xb8, 0x01, 0x80]).is_none());
        assert!(parse_rlp_single(&[0xf8, 0x01, 0xc0]).is_none());
        assert!(parse_rlp_single(&[0x80, 0x80]).is_none());
        let zero = parse_rlp_single(&[0x00]).unwrap();
        assert_eq!(parse_rlp_u64(zero), None);
        assert_eq!(parse_rlp_u64(parse_rlp_single(&[0x80]).unwrap()), Some(0));
    }
    #[test]
    fn current_header_requires_exactly_twenty_one_fields() {
        let raw = signed_header(
            [0x11; 32],
            1_001,
            BSC_NATIVE_MAINNET_MENDEL_TIME * 1_000,
            2,
            &[],
        );
        assert!(parse_header(&raw).is_ok());
        let fields = parse_rlp_list(&raw).unwrap();
        let short = rlp_encode_list_raw(
            &fields[..20]
                .iter()
                .map(|field| field.raw)
                .collect::<Vec<_>>(),
        )
        .unwrap();
        assert_eq!(
            parse_header(&short),
            Err(BscNativeFinalityError::InvalidHeaderRlp)
        );
        let mut long_fields = fields.iter().map(|field| field.raw).collect::<Vec<_>>();
        let extra = uint(1);
        long_fields.push(&extra);
        let long = rlp_encode_list_raw(&long_fields).unwrap();
        assert_eq!(
            parse_header(&long),
            Err(BscNativeFinalityError::InvalidHeaderRlp)
        );
    }
    #[test]
    fn proposer_seal_uses_chain_id_full_extra_and_post_prague_fields() {
        let raw = signed_header(
            [0x11; 32],
            1_001,
            BSC_NATIVE_MAINNET_MENDEL_TIME * 1_000,
            2,
            &[],
        );
        let header = parse_header(&raw).unwrap();
        let extra = parse_extra(&header).unwrap();
        assert_eq!(
            recover_proposer(&header, &extra.seal, 56).unwrap(),
            signer_address()
        );
        assert_ne!(
            recover_proposer(&header, &extra.seal, 97).ok(),
            Some(signer_address())
        );
        let mut fields = header
            .fields
            .iter()
            .map(|field| field.raw)
            .collect::<Vec<_>>();
        let changed_requests = bytes(&[0x56; 32]);
        fields[20] = &changed_requests;
        let changed = rlp_encode_list_raw(&fields).unwrap();
        let changed = parse_header(&changed).unwrap();
        assert_ne!(
            recover_proposer(&changed, &extra.seal, 56).ok(),
            Some(signer_address())
        );
    }
    #[test]
    fn proposer_recovery_rejects_bad_recovery_id_and_high_s() {
        let raw = signed_header(
            [0x11; 32],
            1_001,
            BSC_NATIVE_MAINNET_MENDEL_TIME * 1_000,
            2,
            &[],
        );
        let header = parse_header(&raw).unwrap();
        let mut seal = parse_extra(&header).unwrap().seal;
        seal[64] = 2;
        assert_eq!(
            recover_proposer(&header, &seal, 56),
            Err(BscNativeFinalityError::InvalidProposerSeal)
        );
        seal = parse_extra(&header).unwrap().seal;
        seal[32..64].fill(0xff);
        assert_eq!(
            recover_proposer(&header, &seal, 56),
            Err(BscNativeFinalityError::InvalidProposerSeal)
        );
    }
    #[test]
    fn extra_data_rejects_non_epoch_rosters_and_bad_epoch_lists() {
        let mut roster = vec![1_u8];
        roster.extend_from_slice(&signer_address());
        roster.extend_from_slice(&BLS_GENERATOR);
        roster.push(1);
        let epoch = signed_header(
            [0x11; 32],
            2_000,
            BSC_NATIVE_MAINNET_MENDEL_TIME * 1_000,
            2,
            &roster,
        );
        assert!(
            parse_extra(&parse_header(&epoch).unwrap())
                .unwrap()
                .epoch
                .is_some()
        );
        let mut zero_count = roster.clone();
        zero_count[0] = 0;
        let zero_count = signed_header(
            [0x11; 32],
            2_000,
            BSC_NATIVE_MAINNET_MENDEL_TIME * 1_000,
            2,
            &zero_count,
        );
        assert_eq!(
            parse_extra(&parse_header(&zero_count).unwrap()),
            Err(BscNativeFinalityError::InvalidEpochRoster)
        );
        let non_epoch = signed_header(
            [0x11; 32],
            2_001,
            BSC_NATIVE_MAINNET_MENDEL_TIME * 1_000,
            2,
            &roster,
        );
        assert_eq!(
            parse_extra(&parse_header(&non_epoch).unwrap()),
            Err(BscNativeFinalityError::InvalidAttestation)
        );
    }
    #[test]
    fn validator_roster_rejects_order_duplicates_and_invalid_vote_keys() {
        let mut lower = signer_address();
        lower[0] = 1;
        let mut higher = lower;
        higher[19] = higher[19].saturating_add(1);
        let valid = vec![validator(lower), validator(higher)];
        assert_eq!(
            validators_from_wire(&valid),
            Err(BscNativeFinalityError::InvalidEpochRoster),
            "duplicate vote keys are rejected even across distinct validators"
        );
        let unsorted = vec![validator(higher), validator(lower)];
        assert_eq!(
            validators_from_wire(&unsorted),
            Err(BscNativeFinalityError::InvalidEpochRoster)
        );
        let invalid_key = vec![BscNativeValidatorV1 {
            consensus_address: lower.to_vec(),
            vote_public_key: vec![0xff; 48],
        }];
        assert_eq!(
            validators_from_wire(&invalid_key),
            Err(BscNativeFinalityError::InvalidBlsSignature)
        );
    }
    #[test]
    fn go_math_rand_port_matches_go_1_regression_vectors() {
        let mut rng = GoMathRand::new(0).expect("bounded Go RNG fixture");
        assert_eq!(rng.int63(), 8_717_895_732_742_165_505);
        assert_eq!(rng.int63(), 2_259_404_117_704_393_152);
        assert_eq!(rng.int63(), 6_050_128_673_802_995_827);
        let mut values = (0..8).collect::<Vec<_>>();
        go_math_rand_shuffle(0, &mut values).expect("bounded fixture shuffle");
        assert_eq!(values, vec![5, 2, 4, 6, 0, 3, 1, 7]);
    }
    #[test]
    fn governed_anchor_hash_binds_network_snapshot_and_header() {
        let anchor = anchor();
        let hash = bsc_native_anchor_hash(&anchor).unwrap();
        assert!(nonzero(&hash));
        let mut replay = anchor.clone();
        replay.network = SccpNetworkV1::BscTestnet;
        assert!(bsc_native_anchor_hash(&replay).is_err());
        let mut turn = anchor.clone();
        turn.turn_length = 2;
        let changed = bsc_native_anchor_hash(&turn).unwrap();
        assert_ne!(changed, hash);
    }
    #[test]
    fn governed_anchor_has_stable_norito_and_json_roundtrips() {
        let anchor = anchor();
        let encoded = norito::to_bytes(&anchor).unwrap();
        let decoded: BscNativeParliaAnchorV1 = norito::decode_from_bytes(&encoded).unwrap();
        assert_eq!(decoded, anchor);
        let json = norito::json::to_json(&anchor).unwrap();
        let decoded_json: BscNativeParliaAnchorV1 = norito::json::from_json(&json).unwrap();
        assert_eq!(decoded_json, anchor);
    }
    #[test]
    fn anchor_rejects_wrong_context_recents_and_fork_window() {
        let mut wrong_context = anchor();
        wrong_context.recent_vote_contexts[0].target_hash[0] ^= 1;
        assert_eq!(
            bsc_native_anchor_hash(&wrong_context),
            Err(BscNativeFinalityError::InvalidAnchor)
        );
        let mut stale_recent = anchor();
        stale_recent.recents.push(BscNativeRecentProposerV1 {
            block_number: 1_000,
            consensus_address: signer_address().to_vec(),
        });
        assert_eq!(
            bsc_native_anchor_hash(&stale_recent),
            Err(BscNativeFinalityError::InvalidAnchor)
        );
        let mut pre_fork = anchor();
        pre_fork.header_rlp = signed_header(
            [0x11; 32],
            1_001,
            (BSC_NATIVE_MAINNET_MENDEL_TIME - 1) * 1_000,
            2,
            &[],
        );
        assert_eq!(
            bsc_native_anchor_hash(&pre_fork),
            Err(BscNativeFinalityError::UnsupportedFork)
        );
    }
    #[test]
    fn header_replay_rejects_parent_number_time_difficulty_and_recent_proposer() {
        let anchor = anchor();
        let (state, _, params) = anchor_state(&anchor).unwrap();
        let next_time = state.header.timestamp_ms + BSC_NATIVE_BLOCK_INTERVAL_MS;
        let valid_raw = signed_header(state.hash, state.number + 1, next_time, 2, &[]);
        let valid = parse_header(&valid_raw).unwrap();
        let mut valid_state = state.clone();
        let mut canonical = vec![CanonicalBlock {
            number: state.number,
            hash: state.hash,
        }];
        let _ = apply_header(&mut valid_state, &valid, params, &mut canonical).unwrap();
        let wrong_parent_raw = signed_header([0xee; 32], state.number + 1, next_time, 2, &[]);
        let wrong_parent = parse_header(&wrong_parent_raw).unwrap();
        assert_eq!(
            apply_header(
                &mut state.clone(),
                &wrong_parent,
                params,
                &mut canonical.clone(),
            ),
            Err(BscNativeFinalityError::NonContiguousHeader)
        );
        let skipped_raw = signed_header(state.hash, state.number + 2, next_time, 2, &[]);
        let skipped = parse_header(&skipped_raw).unwrap();
        assert_eq!(
            apply_header(&mut state.clone(), &skipped, params, &mut canonical.clone(),),
            Err(BscNativeFinalityError::NonContiguousHeader)
        );
        let early_raw = signed_header(state.hash, state.number + 1, next_time - 1, 2, &[]);
        let early = parse_header(&early_raw).unwrap();
        assert_eq!(
            apply_header(&mut state.clone(), &early, params, &mut canonical.clone(),),
            Err(BscNativeFinalityError::InvalidTimestamp)
        );
        let wrong_difficulty_raw = signed_header(state.hash, state.number + 1, next_time, 1, &[]);
        let wrong_difficulty = parse_header(&wrong_difficulty_raw).unwrap();
        assert_eq!(
            apply_header(
                &mut state.clone(),
                &wrong_difficulty,
                params,
                &mut canonical.clone(),
            ),
            Err(BscNativeFinalityError::WrongDifficulty)
        );
        let mut recent_state = state;
        let proposer = signer_address();
        let other = if proposer == [0xff; 20] {
            [0x01; 20]
        } else {
            [0xff; 20]
        };
        let mut validators = vec![
            Validator {
                address: proposer,
                vote_key: [1; 48],
            },
            Validator {
                address: other,
                vote_key: [2; 48],
            },
        ];
        validators.sort_unstable_by_key(|validator| validator.address);
        recent_state.validators = validators;
        recent_state.recents.insert(recent_state.number, proposer);
        assert_eq!(
            apply_header(&mut recent_state, &valid, params, &mut canonical.clone(),),
            Err(BscNativeFinalityError::RecentlySigned)
        );
    }
    #[test]
    fn receipt_mpt_authenticates_unique_lane_bound_success_event() {
        let emitter = [0x61; 20];
        let lane_hash = [0x62; 32];
        let digest = [0x63; 32];
        let receipt = source_receipt(emitter, lane_hash, digest, 1, false);
        let key = uint(7);
        let (root, proof_nodes) = singleton_mpt(&key, &receipt);
        let proof = BscNativeReceiptProofV1 {
            transaction_index: 7,
            receipt_bytes: receipt,
            proof_nodes,
        };
        let validated = verify_test_bsc_receipt(&proof, root, emitter, lane_hash, digest).unwrap();
        assert_eq!(validated.transaction_index, 7);
        assert_eq!(validated.lane_hash, lane_hash);
        assert_eq!(
            verify_test_bsc_receipt(&proof, root, emitter, [0x64; 32], digest),
            Err(BscNativeReceiptError::InvalidSourceEvent)
        );
        assert_eq!(
            verify_test_bsc_receipt(&proof, root, [0x65; 20], lane_hash, digest),
            Err(BscNativeReceiptError::InvalidSourceEvent)
        );
    }
    #[test]
    fn receipt_rejects_failed_duplicate_unsupported_type_and_wrong_mpt_key() {
        let emitter = [0x71; 20];
        let lane_hash = [0x72; 32];
        let digest = [0x73; 32];
        for (receipt, expected) in [
            (
                source_receipt(emitter, lane_hash, digest, 0, false),
                BscNativeReceiptError::FailedReceipt,
            ),
            (
                source_receipt(emitter, lane_hash, digest, 1, true),
                BscNativeReceiptError::InvalidSourceEvent,
            ),
        ] {
            let key = uint(1);
            let (root, nodes) = singleton_mpt(&key, &receipt);
            let proof = BscNativeReceiptProofV1 {
                transaction_index: 1,
                receipt_bytes: receipt,
                proof_nodes: nodes,
            };
            assert_eq!(
                verify_test_bsc_receipt(&proof, root, emitter, lane_hash, digest),
                Err(expected)
            );
        }
        let legacy = source_receipt(emitter, lane_hash, digest, 1, false);
        let mut typed = vec![0x05];
        typed.extend_from_slice(&legacy);
        let key = uint(1);
        let (root, nodes) = singleton_mpt(&key, &typed);
        let mut proof = BscNativeReceiptProofV1 {
            transaction_index: 1,
            receipt_bytes: typed,
            proof_nodes: nodes,
        };
        assert_eq!(
            verify_test_bsc_receipt(&proof, root, emitter, lane_hash, digest),
            Err(BscNativeReceiptError::InvalidReceipt)
        );
        proof.transaction_index = 2;
        assert_eq!(
            verify_test_bsc_receipt(&proof, root, emitter, lane_hash, digest),
            Err(BscNativeReceiptError::InvalidMptProof)
        );
    }
    #[test]
    fn mpt_rejects_trailing_nodes_bad_hash_and_noncanonical_compact_path() {
        let key = [0x12, 0x34];
        let value = [0x99];
        let (root, nodes) = singleton_mpt(&key, &value);
        assert_eq!(
            verify_mpt_inclusion(root, &key, &nodes),
            Some(value.to_vec())
        );
        let mut trailing = nodes.clone();
        trailing.push(nodes[0].clone());
        assert_eq!(verify_mpt_inclusion(root, &key, &trailing), None);
        let mut wrong_hash = root;
        wrong_hash[0] ^= 1;
        assert_eq!(verify_mpt_inclusion(wrong_hash, &key, &nodes), None);
        assert_eq!(decode_compact_path(&[0x01]), None);
        assert_eq!(decode_compact_path(&[0x00]), None);
    }
    #[test]
    fn finalized_emitter_state_binds_exact_runtime_code() {
        let emitter = [0x81; 20];
        let code_hash = [0x83; 32];
        let storage_root = [0x82; 32];
        let account = list(&[uint(1), uint(0), bytes(&storage_root), bytes(&code_hash)]);
        let account_key = keccak256(&emitter);
        let (state_root, account_nodes) = singleton_mpt(&account_key, &account);
        let proof = BscNativeEmitterStateProofV1 {
            account_proof_nodes: account_nodes,
        };
        let validated =
            verify_bsc_native_emitter_state(&proof, state_root, emitter, code_hash).unwrap();
        assert_eq!(validated.storage_root, storage_root);
        assert_eq!(
            verify_bsc_native_emitter_state(&proof, state_root, emitter, [0x84; 32]),
            Err(BscNativeEmitterStateError::RuntimeCodeHashMismatch)
        );
    }
    #[test]
    fn finality_work_estimate_enforces_epoch_target_and_native_suffix_bounds() {
        let anchor = anchor();
        let anchor_header_bytes = anchor.header_rlp.len();
        let mut proof = BscNativeFinalityProofV1 {
            version: 1,
            anchor,
            headers_rlp: vec![vec![0xc0]],
            target_header_index: 0,
        };
        let estimate = bsc_native_finality_work_estimate(&proof).unwrap();
        assert_eq!(estimate.continuation_headers, 1);
        assert_eq!(estimate.secp256k1_recoveries, 2);
        assert_eq!(estimate.bls_aggregate_checks_upper_bound, 1);
        assert_eq!(
            estimate.bls_signer_contributions_upper_bound,
            u32::try_from(MAX_VALIDATORS).unwrap()
        );
        assert_eq!(
            estimate.framed_header_bytes,
            u32::try_from(anchor_header_bytes + 1).unwrap()
        );
        proof.headers_rlp = vec![vec![0xc0]; BSC_NATIVE_MAX_FINALITY_HEADERS];
        proof.target_header_index = u16::try_from(BSC_NATIVE_MAX_TARGET_HEADERS - 1).unwrap();
        let boundary = bsc_native_finality_work_estimate(&proof).unwrap();
        assert_eq!(
            usize::from(boundary.continuation_headers),
            BSC_NATIVE_MAX_FINALITY_HEADERS
        );
        proof.target_header_index = u16::try_from(BSC_NATIVE_MAX_TARGET_HEADERS).unwrap();
        assert_eq!(
            bsc_native_finality_work_estimate(&proof),
            Err(BscNativeFinalityError::ResourceLimit)
        );
        proof.target_header_index = 0;
        proof.headers_rlp = vec![vec![0xc0]; BSC_NATIVE_MAX_FINALITY_SUFFIX_HEADERS + 2];
        assert_eq!(
            bsc_native_finality_work_estimate(&proof),
            Err(BscNativeFinalityError::ResourceLimit)
        );
        proof.headers_rlp = vec![vec![0xc0]; BSC_NATIVE_MAX_FINALITY_HEADERS + 1];
        assert_eq!(
            bsc_native_finality_work_estimate(&proof),
            Err(BscNativeFinalityError::ResourceLimit)
        );
        proof.headers_rlp = vec![vec![0; MAX_HEADER_BYTES + 1]];
        assert_eq!(
            bsc_native_finality_work_estimate(&proof),
            Err(BscNativeFinalityError::ResourceLimit)
        );
        proof.headers_rlp = vec![vec![0xc0]];
        proof.target_header_index = 1;
        assert_eq!(
            bsc_native_finality_work_estimate(&proof),
            Err(BscNativeFinalityError::TargetNotFinalized)
        );
    }
    #[test]
    fn vote_attestation_rejects_source_target_confusion_bitmap_replay_and_quorum() {
        let anchor = anchor();
        let (state, _, _) = anchor_state(&anchor).unwrap();
        let canonical = vec![CanonicalBlock {
            number: state.number,
            hash: state.hash,
        }];
        let base = VoteAttestation {
            address_set: 1,
            aggregate_signature: [0x11; 96],
            data: VoteData {
                source_number: state.justification.target_number,
                source_hash: state.justification.target_hash,
                target_number: state.number,
                target_hash: state.hash,
            },
        };
        assert_eq!(
            verify_attestation(&state, &base, &canonical),
            Err(BscNativeFinalityError::InvalidBlsSignature)
        );
        let mut swapped = base.clone();
        swapped.data.source_hash = swapped.data.target_hash;
        assert_eq!(
            verify_attestation(&state, &swapped, &canonical),
            Err(BscNativeFinalityError::InvalidAttestation)
        );
        let mut outside = base.clone();
        outside.address_set = 2;
        assert_eq!(
            verify_attestation(&state, &outside, &canonical),
            Err(BscNativeFinalityError::InvalidVoteAddressSet)
        );
        let mut replay = base;
        replay.data.target_hash[0] ^= 1;
        assert_eq!(
            verify_attestation(&state, &replay, &canonical),
            Err(BscNativeFinalityError::InvalidAttestation)
        );
    }
    const NATIVE_VOTE_SIG1: &str = "994e94da4ef7fb2675cda81271ee1093332aef607aec286404f4b32573a06cee80b60f4570b349669f15c157529a32f009c796d6c0c9ccb9c57c55f5afb8bbb783ae70216c810fbbc6eac13771264e67d551fd566d25fa9bf270b724b17f292a";
    const NATIVE_VOTE_SIG2: &str = "9512d82b2348a3d5b73676bd9bd9be4340ce321eab94b1cc5f7b244a0e5971277a8577a0ba61eb44af2db1f87fdff99b0bef9ddc34ae1cd6fb548c5385a606a2faa37031ef0f5e2eafae439e8ca6305e018c3acb9985aa2af82c9fc857208e0f";
    const NATIVE_VOTE_SIG3: &str = "a9e29885e6ed4dc61a09c5fe8dedb2393ff3b5a333dfaf2a27eeb8fb8f7a4f1d107d420aa9ad5771b93881f1bc6a768a19d3650c9f433c8e3ae7ff4cfe2f4fcf466433e55e0e1e305dddbc1753abd1f047dcd2a563abbc0c0d56903537dceb68";
    struct NativeVoteChainFixture {
        anchor_header_hash: H256,
        base_time: u64,
        anchor_hash: H256,
        header1_hash: H256,
        finalized: BscNativeFinalityProofV1,
    }
    fn signed_attested_header(
        parent_hash: H256,
        number: u64,
        timestamp_ms: u64,
        data: VoteData,
        signature_hex: &str,
    ) -> Vec<u8> {
        signed_header(
            parent_hash,
            number,
            timestamp_ms,
            2,
            &attestation_bytes(data, signature_hex),
        )
    }
    fn native_vote_chain_fixture() -> NativeVoteChainFixture {
        let anchor = anchor();
        let anchor_header_hash = keccak256(&anchor.header_rlp);
        let expected_anchor_header_hash: H256 =
            hex_bytes("1e98d0dd459b37bb01a2ddad156c8544591aff4d66d8b08c5b344bbbb0fe4e4c")
                .try_into()
                .unwrap();
        assert_eq!(anchor_header_hash, expected_anchor_header_hash);
        let base_time = BSC_NATIVE_MAINNET_MENDEL_TIME * 1_000;
        let header1 = signed_attested_header(
            anchor_header_hash,
            1_002,
            base_time + 450,
            VoteData {
                source_number: 1_000,
                source_hash: [0xaa; 32],
                target_number: 1_001,
                target_hash: anchor_header_hash,
            },
            NATIVE_VOTE_SIG1,
        );
        let header1_hash = keccak256(&header1);
        let expected_header1_hash: H256 =
            hex_bytes("055b1bbcc420336b4947315db2c8aa7de5624666511d697ad764a385d981aa46")
                .try_into()
                .unwrap();
        assert_eq!(header1_hash, expected_header1_hash);
        let header2 = signed_attested_header(
            header1_hash,
            1_003,
            base_time + 900,
            VoteData {
                source_number: 1_001,
                source_hash: anchor_header_hash,
                target_number: 1_002,
                target_hash: header1_hash,
            },
            NATIVE_VOTE_SIG2,
        );
        let header2_hash = keccak256(&header2);
        let expected_header2_hash: H256 =
            hex_bytes("302f28694c79f4ee6427df99caf2dfa0cb28d3867378f7915c26268ba5e0386d")
                .try_into()
                .unwrap();
        assert_eq!(header2_hash, expected_header2_hash);
        let header3 = signed_attested_header(
            header2_hash,
            1_004,
            base_time + 1_350,
            VoteData {
                source_number: 1_002,
                source_hash: header1_hash,
                target_number: 1_003,
                target_hash: header2_hash,
            },
            NATIVE_VOTE_SIG3,
        );
        let anchor_hash = bsc_native_anchor_hash(&anchor).unwrap();
        NativeVoteChainFixture {
            anchor_header_hash,
            base_time,
            anchor_hash,
            header1_hash,
            finalized: BscNativeFinalityProofV1 {
                version: 1,
                anchor,
                headers_rlp: vec![header1, header2, header3],
                target_header_index: 0,
            },
        }
    }
    fn finality_work(
        proof: &BscNativeFinalityProofV1,
        anchor_hash: H256,
    ) -> BscNativeFinalityWorkPerformedV1 {
        let mut work = BscNativeFinalityWorkPerformedV1::default();
        verify_bsc_native_finality_counted(
            proof,
            SccpNetworkV1::BscMainnet,
            anchor_hash,
            &mut work,
        )
        .unwrap();
        work
    }
    fn append_valid_headers(proof: &mut BscNativeFinalityProofV1, count: u64) {
        let last = parse_header(proof.headers_rlp.last().unwrap()).unwrap();
        let mut parent = last.block_hash;
        let mut number = last.number;
        let mut timestamp_ms = last.timestamp_ms;
        for _ in 0..count {
            number += 1;
            timestamp_ms += BSC_NATIVE_BLOCK_INTERVAL_MS;
            let header = signed_header(parent, number, timestamp_ms, 2, &[]);
            parent = keccak256(&header);
            proof.headers_rlp.push(header);
        }
    }
    fn assert_all_headers_valid(proof: &BscNativeFinalityProofV1) {
        let (mut state, _, params) = anchor_state(&proof.anchor).unwrap();
        let mut canonical_blocks = vec![CanonicalBlock {
            number: state.number,
            hash: state.hash,
        }];
        for context in state.vote_contexts.iter().skip(1) {
            canonical_blocks.push(CanonicalBlock {
                number: context.target_number,
                hash: context.target_hash,
            });
        }
        for raw in &proof.headers_rlp {
            let header = parse_header(raw).unwrap();
            let _ = apply_header(&mut state, &header, params, &mut canonical_blocks).unwrap();
        }
    }
    #[test]
    fn native_vote_chain_finalizes_source_not_merely_justified_target() {
        let fixture = native_vote_chain_fixture();
        let only_justified = BscNativeFinalityProofV1 {
            version: 1,
            anchor: fixture.finalized.anchor.clone(),
            headers_rlp: fixture.finalized.headers_rlp[..2].to_vec(),
            target_header_index: 0,
        };
        assert_eq!(
            verify_bsc_native_finality(
                &only_justified,
                SccpNetworkV1::BscMainnet,
                fixture.anchor_hash,
            ),
            Err(BscNativeFinalityError::TargetNotFinalized),
            "a justified target is not final until it becomes a later vote's source"
        );
        let result = verify_bsc_native_finality(
            &fixture.finalized,
            SccpNetworkV1::BscMainnet,
            fixture.anchor_hash,
        )
        .unwrap();
        assert_eq!(result.block_number, 1_002);
        assert_eq!(result.block_hash, fixture.header1_hash);
        assert_eq!(result.resulting_finalized_number, 1_002);
        let exact_work = finality_work(&fixture.finalized, fixture.anchor_hash);
        assert_eq!(exact_work.continuation_headers, 3);
        assert_eq!(exact_work.secp256k1_recoveries, 4);
        assert_eq!(exact_work.bls_aggregate_checks, 3);
        assert_eq!(exact_work.bls_signer_contributions, 3);
    }
    #[test]
    fn native_vote_chain_rejects_surplus_continuations() {
        let fixture = native_vote_chain_fixture();
        let exact_work = finality_work(&fixture.finalized, fixture.anchor_hash);
        let mut one_surplus = fixture.finalized.clone();
        append_valid_headers(&mut one_surplus, 1);
        assert_all_headers_valid(&one_surplus);
        let mut one_surplus_work = BscNativeFinalityWorkPerformedV1::default();
        assert_eq!(
            verify_bsc_native_finality_counted(
                &one_surplus,
                SccpNetworkV1::BscMainnet,
                fixture.anchor_hash,
                &mut one_surplus_work,
            ),
            Err(BscNativeFinalityError::NonMinimalContinuation)
        );
        assert_eq!(one_surplus_work, exact_work);
        let mut many_surplus = fixture.finalized;
        append_valid_headers(&mut many_surplus, 32);
        assert_all_headers_valid(&many_surplus);
        let mut many_surplus_work = BscNativeFinalityWorkPerformedV1::default();
        assert_eq!(
            verify_bsc_native_finality_counted(
                &many_surplus,
                SccpNetworkV1::BscMainnet,
                fixture.anchor_hash,
                &mut many_surplus_work,
            ),
            Err(BscNativeFinalityError::ResourceLimit)
        );
        assert_eq!(
            many_surplus_work,
            BscNativeFinalityWorkPerformedV1::default(),
            "over-window continuations fail before anchor or signature work"
        );
    }
    #[test]
    fn native_vote_chain_rejects_bad_attestation_signature() {
        let fixture = native_vote_chain_fixture();
        let mut bad_signature = fixture.finalized;
        let parsed = parse_header(&bad_signature.headers_rlp[0]).unwrap();
        let mut attestation = parse_extra(&parsed).unwrap().attestation.unwrap();
        attestation.aggregate_signature[0] ^= 1;
        let bad_middle = list(&[
            uint(attestation.address_set),
            bytes(&attestation.aggregate_signature),
            list(&[
                uint(attestation.data.source_number),
                bytes(&attestation.data.source_hash),
                uint(attestation.data.target_number),
                bytes(&attestation.data.target_hash),
            ]),
            bytes(&[]),
        ]);
        bad_signature.headers_rlp[0] = signed_header(
            fixture.anchor_header_hash,
            1_002,
            fixture.base_time + 450,
            2,
            &bad_middle,
        );
        assert_eq!(
            verify_bsc_native_finality(
                &bad_signature,
                SccpNetworkV1::BscMainnet,
                fixture.anchor_hash,
            ),
            Err(BscNativeFinalityError::InvalidBlsSignature)
        );
    }
    #[test]
    fn finality_proof_rejects_empty_bounds_index_and_anchor_replay() {
        let anchor = anchor();
        let anchor_hash = bsc_native_anchor_hash(&anchor).unwrap();
        let mut proof = BscNativeFinalityProofV1 {
            version: 1,
            anchor,
            headers_rlp: vec![],
            target_header_index: 0,
        };
        assert_eq!(
            verify_bsc_native_finality(&proof, SccpNetworkV1::BscMainnet, anchor_hash),
            Err(BscNativeFinalityError::ResourceLimit)
        );
        proof.headers_rlp = vec![vec![0xc0]];
        proof.target_header_index = 1;
        assert_eq!(
            verify_bsc_native_finality(&proof, SccpNetworkV1::BscMainnet, anchor_hash),
            Err(BscNativeFinalityError::TargetNotFinalized)
        );
        proof.target_header_index = 0;
        assert_eq!(
            verify_bsc_native_finality(&proof, SccpNetworkV1::BscMainnet, [0x77; 32]),
            Err(BscNativeFinalityError::AnchorHashMismatch)
        );
        assert_eq!(
            verify_bsc_native_finality(&proof, SccpNetworkV1::BscTestnet, anchor_hash),
            Err(BscNativeFinalityError::WrongNetwork)
        );
    }
}
