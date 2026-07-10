//! Native Ethereum light-client, state, storage, and receipt verification for SCCP.
//!
//! The verifier starts at one immutable governed beacon light-client bootstrap,
//! applies a bounded sequence of protocol `LightClientUpdate` objects, and then
//! opens three canonical Ethereum Merkle-Patricia tries under the finalized
//! execution payload: the immutable transfer-route contract account and the
//! successful transaction receipt. No RPC assertion, owner-authorized generic
//! emitter, proxy convention, or domain-only fallback is admitted.

use alloc::{collections::BTreeSet, vec::Vec};
use core::fmt;

use iroha_data_model::bridge::sccp::{
    SccpEvmSourceEmitterV1, SccpNetworkV1, SccpSourceEmitterV1, SccpSourceIdentityV1,
};
use tiny_keccak::{Hasher as _, Keccak};

use super::{
    H256, SccpPayloadV1, canonical_sccp_payload_bytes, decode_canonical_sccp_payload_bytes,
    prefixed_blake2b, sccp_lane_id_hash_v1, sccp_lane_source_event_digest_v1, sccp_message_id,
    sccp_source_identity_hash_v1,
};
use crate::ethereum_native::{
    AuthenticatedExecutionBlock, BeaconBlockHeader, BlsPublicKey, BlsSignature,
    CapellaExecutionPayloadHeader, CurrentSyncCommitteeBranch, DenebExecutionPayloadHeader,
    EthereumFork, EthereumLightClientError, EthereumLightClientState, ExtraData, FinalityBranch,
    ForkActivation, ForkSchedule, LightClientBootstrap, LightClientHeader, LightClientUpdate,
    NextSyncCommitteeBranch, Root, SYNC_COMMITTEE_BITS_BYTES, SYNC_COMMITTEE_SIZE, SyncAggregate,
    SyncCommittee,
};

const ETHEREUM_NATIVE_ANCHOR_PREFIX_V1: &[u8] = b"sccp:ethereum:native-anchor:v1";
const ETHEREUM_SOURCE_EVENT_SIGNATURE_V1: &[u8] =
    b"SccpTransfer(bytes32,bytes32,bytes32,bytes32,bytes32,bytes)";
/// Maximum consecutive native light-client updates admitted by one source proof.
pub const ETHEREUM_NATIVE_MAX_LIGHT_CLIENT_UPDATES: usize = 128;
const MAX_ENCODED_SOURCE_PROOF_BYTES: usize = 16 * 1024 * 1024;
const MAX_ENCODED_SOURCE_PROOF_BYTES_U64: u64 = 16 * 1024 * 1024;
const MAX_JSON_SOURCE_PROOF_BYTES: usize = 40 * 1024 * 1024;
const MAX_MPT_PROOF_NODES: usize = 64;
const MAX_MPT_NODE_BYTES: usize = 1024 * 1024;
const MAX_MPT_PROOF_BYTES: usize = 4 * 1024 * 1024;
const MAX_RECEIPT_LOGS: usize = 1_024;
const MAX_LOG_TOPICS: usize = 4;
const NORITO_COMPRESSION_OFFSET: usize = 4 + 1 + 1 + 16;
const NORITO_LENGTH_OFFSET: usize = NORITO_COMPRESSION_OFFSET + 1;
const EMPTY_CODE_HASH: H256 = [
    0xc5, 0xd2, 0x46, 0x01, 0x86, 0xf7, 0x23, 0x3c, 0x92, 0x7e, 0x7d, 0xb2, 0xdc, 0xc7, 0x03, 0xc0,
    0xe5, 0x00, 0xb6, 0x53, 0xca, 0x82, 0x27, 0x3b, 0x7b, 0xfa, 0xd8, 0x04, 0x5d, 0x85, 0xa4, 0x70,
];
const EMPTY_TRIE_ROOT: H256 = [
    0x56, 0xe8, 0x1f, 0x17, 0x1b, 0xcc, 0x55, 0xa6, 0xff, 0x83, 0x45, 0xe6, 0x92, 0xc0, 0xf8, 0x6e,
    0x5b, 0x48, 0xe0, 0x1b, 0x99, 0x6c, 0xad, 0xc0, 0x01, 0x62, 0x2f, 0xb5, 0xe3, 0x63, 0xb4, 0x21,
];

/// Closed Ethereum fork tag used by the SCCP wire DTOs.
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
#[norito(tag = "fork", content = "detail", rename_all = "snake_case")]
pub enum EthereumNativeForkV1 {
    /// Altair.
    Altair,
    /// Bellatrix.
    Bellatrix,
    /// Capella.
    Capella,
    /// Deneb.
    Deneb,
    /// Electra.
    Electra,
    /// Fulu.
    Fulu,
}

impl From<EthereumNativeForkV1> for EthereumFork {
    fn from(value: EthereumNativeForkV1) -> Self {
        match value {
            EthereumNativeForkV1::Altair => Self::Altair,
            EthereumNativeForkV1::Bellatrix => Self::Bellatrix,
            EthereumNativeForkV1::Capella => Self::Capella,
            EthereumNativeForkV1::Deneb => Self::Deneb,
            EthereumNativeForkV1::Electra => Self::Electra,
            EthereumNativeForkV1::Fulu => Self::Fulu,
        }
    }
}

impl From<EthereumFork> for EthereumNativeForkV1 {
    fn from(value: EthereumFork) -> Self {
        match value {
            EthereumFork::Altair => Self::Altair,
            EthereumFork::Bellatrix => Self::Bellatrix,
            EthereumFork::Capella => Self::Capella,
            EthereumFork::Deneb => Self::Deneb,
            EthereumFork::Electra => Self::Electra,
            EthereumFork::Fulu => Self::Fulu,
        }
    }
}

/// One governed fork activation in an Ethereum light-client schedule.
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
pub struct EthereumNativeForkActivationV1 {
    /// First epoch at which the fork is active.
    #[norito(with = "crate::json_utils::u64_string")]
    pub epoch: u64,
    /// Four-byte consensus fork version.
    #[norito(with = "crate::json_utils::bytes_hex")]
    pub version: Vec<u8>,
}

/// Complete closed Altair-through-Fulu fork schedule.
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
pub struct EthereumNativeForkScheduleV1 {
    /// Network genesis validators root.
    #[norito(with = "crate::json_utils::hex32")]
    pub genesis_validators_root: H256,
    /// Altair activation.
    pub altair: EthereumNativeForkActivationV1,
    /// Bellatrix activation.
    pub bellatrix: EthereumNativeForkActivationV1,
    /// Capella activation.
    pub capella: EthereumNativeForkActivationV1,
    /// Deneb activation.
    pub deneb: EthereumNativeForkActivationV1,
    /// Electra activation.
    pub electra: EthereumNativeForkActivationV1,
    /// Fulu activation.
    pub fulu: EthereumNativeForkActivationV1,
}

/// Wire representation of the official SSZ `BeaconBlockHeader`.
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
pub struct EthereumNativeBeaconHeaderV1 {
    /// Beacon slot.
    #[norito(with = "crate::json_utils::u64_string")]
    pub slot: u64,
    /// Proposer validator index.
    #[norito(with = "crate::json_utils::u64_string")]
    pub proposer_index: u64,
    /// Parent beacon block root.
    #[norito(with = "crate::json_utils::hex32")]
    pub parent_root: H256,
    /// Beacon state root.
    #[norito(with = "crate::json_utils::hex32")]
    pub state_root: H256,
    /// Beacon block body root.
    #[norito(with = "crate::json_utils::hex32")]
    pub body_root: H256,
}

/// Wire representation of the Capella execution payload header.
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
pub struct EthereumNativeCapellaExecutionHeaderV1 {
    /// Parent execution block hash.
    #[norito(with = "crate::json_utils::hex32")]
    pub parent_hash: H256,
    /// Fee recipient.
    #[norito(with = "crate::json_utils::bytes_hex")]
    pub fee_recipient: Vec<u8>,
    /// State trie root.
    #[norito(with = "crate::json_utils::hex32")]
    pub state_root: H256,
    /// Receipts trie root.
    #[norito(with = "crate::json_utils::hex32")]
    pub receipts_root: H256,
    /// 256-byte execution logs bloom.
    #[norito(with = "crate::json_utils::bytes_hex")]
    pub logs_bloom: Vec<u8>,
    /// Previous RANDAO mix.
    #[norito(with = "crate::json_utils::hex32")]
    pub prev_randao: H256,
    /// Execution block number.
    #[norito(with = "crate::json_utils::u64_string")]
    pub block_number: u64,
    /// Gas limit.
    #[norito(with = "crate::json_utils::u64_string")]
    pub gas_limit: u64,
    /// Gas used.
    #[norito(with = "crate::json_utils::u64_string")]
    pub gas_used: u64,
    /// Execution timestamp.
    #[norito(with = "crate::json_utils::u64_string")]
    pub timestamp: u64,
    /// SSZ `ByteList[32]` extra data.
    #[norito(with = "crate::json_utils::bytes_hex")]
    pub extra_data: Vec<u8>,
    /// Little-endian SSZ `uint256` base fee.
    #[norito(with = "crate::json_utils::hex32")]
    pub base_fee_per_gas: H256,
    /// Execution block hash.
    #[norito(with = "crate::json_utils::hex32")]
    pub block_hash: H256,
    /// Transactions list root.
    #[norito(with = "crate::json_utils::hex32")]
    pub transactions_root: H256,
    /// Withdrawals list root.
    #[norito(with = "crate::json_utils::hex32")]
    pub withdrawals_root: H256,
}

/// Wire representation of the Deneb execution payload header used through Fulu.
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
pub struct EthereumNativeDenebExecutionHeaderV1 {
    /// Capella-compatible header fields.
    pub base: EthereumNativeCapellaExecutionHeaderV1,
    /// Blob gas used.
    #[norito(with = "crate::json_utils::u64_string")]
    pub blob_gas_used: u64,
    /// Excess blob gas.
    #[norito(with = "crate::json_utils::u64_string")]
    pub excess_blob_gas: u64,
}

/// Fork-closed execution payload header carried by a light-client header.
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
#[norito(tag = "layout", content = "header", rename_all = "snake_case")]
pub enum EthereumNativeExecutionHeaderV1 {
    /// Capella layout.
    Capella(EthereumNativeCapellaExecutionHeaderV1),
    /// Deneb layout, inherited unchanged by Electra and Fulu.
    Deneb(EthereumNativeDenebExecutionHeaderV1),
}

/// Wire representation of a fork-specific Ethereum `LightClientHeader`.
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
pub struct EthereumNativeLightClientHeaderV1 {
    /// Closed fork layout used by this header.
    pub fork: EthereumNativeForkV1,
    /// Beacon header.
    pub beacon: EthereumNativeBeaconHeaderV1,
    /// Execution header; absent exactly for Altair and Bellatrix.
    pub execution: Option<EthereumNativeExecutionHeaderV1>,
    /// Execution-payload Merkle branch; empty before Capella, four roots after.
    #[norito(with = "crate::json_utils::vec_bytes_hex")]
    pub execution_branch: Vec<Vec<u8>>,
}

/// Wire representation of the official 512-position sync committee.
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
pub struct EthereumNativeSyncCommitteeV1 {
    /// Compressed 48-byte min-pk public keys in positional order.
    #[norito(with = "crate::json_utils::vec_bytes_hex")]
    pub public_keys: Vec<Vec<u8>>,
    /// Compressed aggregate public key committed by beacon state.
    #[norito(with = "crate::json_utils::bytes_hex")]
    pub aggregate_public_key: Vec<u8>,
}

/// Wire representation of a governed `LightClientBootstrap`.
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
pub struct EthereumNativeLightClientBootstrapV1 {
    /// Trusted light-client header.
    pub header: EthereumNativeLightClientHeaderV1,
    /// Current sync committee committed by the header state root.
    pub current_sync_committee: EthereumNativeSyncCommitteeV1,
    /// Fork-shaped current-committee branch.
    #[norito(with = "crate::json_utils::vec_bytes_hex")]
    pub current_sync_committee_branch: Vec<Vec<u8>>,
}

/// Wire representation of one full Ethereum `LightClientUpdate`.
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
pub struct EthereumNativeLightClientUpdateV1 {
    /// Sync-committee-attested header.
    pub attested_header: EthereumNativeLightClientHeaderV1,
    /// Next committee committed by the attested state.
    pub next_sync_committee: EthereumNativeSyncCommitteeV1,
    /// Fork-shaped next-committee branch.
    #[norito(with = "crate::json_utils::vec_bytes_hex")]
    pub next_sync_committee_branch: Vec<Vec<u8>>,
    /// Header committed by the finalized checkpoint.
    pub finalized_header: EthereumNativeLightClientHeaderV1,
    /// Fork-shaped finalized-checkpoint branch.
    #[norito(with = "crate::json_utils::vec_bytes_hex")]
    pub finality_branch: Vec<Vec<u8>>,
    /// Little-endian positional `Bitvector[512]`.
    #[norito(with = "crate::json_utils::bytes_hex")]
    pub sync_committee_bits: Vec<u8>,
    /// Compressed 96-byte aggregate BLS signature.
    #[norito(with = "crate::json_utils::bytes_hex")]
    pub sync_committee_signature: Vec<u8>,
    /// Slot at which the aggregate signature was created.
    #[norito(with = "crate::json_utils::u64_string")]
    pub signature_slot: u64,
}

/// Immutable governed native Ethereum light-client anchor.
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
pub struct EthereumNativeTrustedAnchorV1 {
    /// Anchor schema version; exactly `1` is accepted.
    pub version: u8,
    /// Exact Ethereum network profile.
    pub network: SccpNetworkV1,
    /// Complete governed fork schedule.
    pub fork_schedule: EthereumNativeForkScheduleV1,
    /// Trusted beacon block root selected out of band by governance.
    #[norito(with = "crate::json_utils::hex32")]
    pub trusted_beacon_block_root: H256,
    /// Bootstrap proving the current committee under the trusted block.
    pub bootstrap: EthereumNativeLightClientBootstrapV1,
    /// Expected native state commitment after validating the bootstrap.
    #[norito(with = "crate::json_utils::hex32")]
    pub anchor_state_commitment: H256,
}

/// Canonical Ethereum MPT inclusion nodes ordered from root to leaf.
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
pub struct EthereumNativeMptProofV1 {
    /// Raw canonical RLP nodes. Inline children are embedded in their parent
    /// and must not be repeated as separate proof elements.
    #[norito(with = "crate::json_utils::vec_bytes_hex")]
    pub nodes: Vec<Vec<u8>>,
}

/// Finalized execution fields duplicated in the proof for explicit binding.
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
pub struct EthereumNativeFinalizedExecutionV1 {
    /// Fork of the finalized light-client header.
    pub fork: EthereumNativeForkV1,
    /// Finalized beacon slot.
    #[norito(with = "crate::json_utils::u64_string")]
    pub beacon_slot: u64,
    /// Finalized beacon block root.
    #[norito(with = "crate::json_utils::hex32")]
    pub beacon_block_root: H256,
    /// Execution block number.
    #[norito(with = "crate::json_utils::u64_string")]
    pub block_number: u64,
    /// Execution block hash.
    #[norito(with = "crate::json_utils::hex32")]
    pub block_hash: H256,
    /// Execution state trie root.
    #[norito(with = "crate::json_utils::hex32")]
    pub state_root: H256,
    /// Execution receipts trie root.
    #[norito(with = "crate::json_utils::hex32")]
    pub receipts_root: H256,
}

/// Complete native Ethereum SCCP source proof.
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
pub struct EthereumNativeSourceProofV1 {
    /// Proof schema version; exactly `1` is accepted.
    pub version: u8,
    /// Full typed direct-contract source identity.
    pub source_identity: SccpSourceIdentityV1,
    /// Explicit identity commitment.
    #[norito(with = "crate::json_utils::hex32")]
    pub source_identity_hash: H256,
    /// Explicit exact directed-lane commitment.
    #[norito(with = "crate::json_utils::hex32")]
    pub lane_hash: H256,
    /// Governed trusted-anchor preimage.
    pub trusted_anchor: EthereumNativeTrustedAnchorV1,
    /// Explicit governed anchor commitment.
    #[norito(with = "crate::json_utils::hex32")]
    pub trusted_anchor_hash: H256,
    /// Consecutive full native updates applied from the anchor.
    pub updates: Vec<EthereumNativeLightClientUpdateV1>,
    /// State commitment after the final update (or the anchor if empty).
    #[norito(with = "crate::json_utils::hex32")]
    pub final_state_commitment: H256,
    /// Finalized execution fields authenticated by the final state.
    pub finalized_execution: EthereumNativeFinalizedExecutionV1,
    /// SCCP message identifier committed by the source event.
    #[norito(with = "crate::json_utils::hex32")]
    pub message_id: H256,
    /// SCCP payload hash committed by the source event.
    #[norito(with = "crate::json_utils::hex32")]
    pub payload_hash: H256,
    /// Exact lane-bound SCCP source event digest.
    #[norito(with = "crate::json_utils::hex32")]
    pub source_event_digest: H256,
    /// State-trie proof for `keccak256(source_contract_address)`.
    pub account_proof: EthereumNativeMptProofV1,
    /// Transaction index whose canonical RLP is the receipts-trie key.
    #[norito(with = "crate::json_utils::u64_string")]
    pub transaction_index: u64,
    /// Receipts-trie proof for the successful typed or legacy receipt.
    pub receipt_proof: EthereumNativeMptProofV1,
}

/// MPT opening role used in precise verification errors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EthereumNativeMptRoleV1 {
    /// Execution state account opening.
    Account,
    /// Successful transaction receipt opening.
    Receipt,
}

/// Errors produced by native Ethereum SCCP source verification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EthereumNativeSourceErrorV1 {
    /// A V1 version field was not exactly one.
    UnsupportedVersion(&'static str),
    /// The proof or governed anchor selected a non-Ethereum network.
    UnsupportedNetwork,
    /// The typed source identity was malformed or did not describe a direct EVM contract.
    InvalidSourceIdentity,
    /// The caller's expected typed identity did not match the proof.
    SourceIdentityMismatch,
    /// The canonical source-identity commitment did not match.
    SourceIdentityHashMismatch,
    /// The canonical exact-lane commitment did not match.
    LaneHashMismatch,
    /// The proof's message id or payload hash did not match the caller's statement.
    MessageStatementMismatch,
    /// The explicit source-event digest was not the canonical lane-bound digest.
    SourceEventDigestMismatch,
    /// A fixed-width or fork-shaped wire field was malformed.
    MalformedWire(&'static str),
    /// The proof contained too many light-client updates.
    TooManyLightClientUpdates(usize),
    /// Native Ethereum light-client validation failed.
    LightClient(EthereumLightClientError),
    /// The governed bootstrap's explicit state commitment was wrong.
    AnchorStateCommitmentMismatch,
    /// The governed trusted-anchor commitment was wrong.
    TrustedAnchorHashMismatch,
    /// The final explicit light-client state commitment was wrong.
    FinalStateCommitmentMismatch,
    /// The final header was pre-Capella and did not authenticate execution fields.
    MissingFinalizedExecution,
    /// The explicit finalized execution fields differed from the authenticated header.
    FinalizedExecutionMismatch,
    /// An encoded source proof exceeded the deterministic input bound.
    EncodedProofTooLarge(usize),
    /// Norito decoding failed or admitted a non-canonical encoding.
    InvalidNoritoEncoding,
    /// An MPT root was the zero sentinel.
    EmptyTrieRoot(EthereumNativeMptRoleV1),
    /// An MPT proof exceeded node, per-node, or aggregate byte bounds.
    MptProofBounds(EthereumNativeMptRoleV1),
    /// An MPT proof repeated an explicit node.
    DuplicateMptNode(EthereumNativeMptRoleV1),
    /// An MPT node did not match its authenticated hash or inline reference.
    MptNodeReferenceMismatch(EthereumNativeMptRoleV1),
    /// An MPT node or child reference was not canonical RLP/trie form.
    NonCanonicalMpt(EthereumNativeMptRoleV1),
    /// The authenticated MPT path did not equal the requested key.
    MptKeyMismatch(EthereumNativeMptRoleV1),
    /// Extra explicit nodes remained after a successful inclusion opening.
    UnusedMptNodes(EthereumNativeMptRoleV1),
    /// The canonical state account was malformed.
    MalformedAccount,
    /// The authenticated runtime code hash differed from the governed identity.
    RuntimeCodeHashMismatch,
    /// The receipt envelope or receipt tuple was malformed.
    MalformedReceipt,
    /// The authenticated transaction receipt had failed status.
    FailedReceipt,
    /// The receipt did not contain exactly one expected canonical SCCP source event.
    SourceEventLogMismatch,
}

impl From<EthereumLightClientError> for EthereumNativeSourceErrorV1 {
    fn from(value: EthereumLightClientError) -> Self {
        Self::LightClient(value)
    }
}

impl fmt::Display for EthereumNativeSourceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion(field) => write!(formatter, "unsupported {field} version"),
            Self::UnsupportedNetwork => formatter.write_str("unsupported Ethereum network"),
            Self::InvalidSourceIdentity => {
                formatter.write_str("invalid direct EVM source identity")
            }
            Self::SourceIdentityMismatch => formatter.write_str("source identity mismatch"),
            Self::SourceIdentityHashMismatch => {
                formatter.write_str("source identity hash mismatch")
            }
            Self::LaneHashMismatch => formatter.write_str("exact lane hash mismatch"),
            Self::MessageStatementMismatch => formatter.write_str("message statement mismatch"),
            Self::SourceEventDigestMismatch => formatter.write_str("source event digest mismatch"),
            Self::MalformedWire(field) => {
                write!(formatter, "malformed Ethereum wire field: {field}")
            }
            Self::TooManyLightClientUpdates(count) => {
                write!(formatter, "too many light-client updates: {count}")
            }
            Self::LightClient(error) => write!(formatter, "Ethereum light-client error: {error}"),
            Self::AnchorStateCommitmentMismatch => {
                formatter.write_str("anchor state commitment mismatch")
            }
            Self::TrustedAnchorHashMismatch => formatter.write_str("trusted anchor hash mismatch"),
            Self::FinalStateCommitmentMismatch => {
                formatter.write_str("final state commitment mismatch")
            }
            Self::MissingFinalizedExecution => {
                formatter.write_str("finalized header has no execution proof")
            }
            Self::FinalizedExecutionMismatch => {
                formatter.write_str("finalized execution fields mismatch")
            }
            Self::EncodedProofTooLarge(size) => {
                write!(formatter, "encoded proof is too large: {size} bytes")
            }
            Self::InvalidNoritoEncoding => {
                formatter.write_str("invalid canonical Norito source proof")
            }
            Self::EmptyTrieRoot(role) => write!(formatter, "zero {role:?} trie root"),
            Self::MptProofBounds(role) => write!(formatter, "{role:?} MPT proof exceeds bounds"),
            Self::DuplicateMptNode(role) => {
                write!(formatter, "duplicate explicit {role:?} MPT node")
            }
            Self::MptNodeReferenceMismatch(role) => {
                write!(formatter, "{role:?} MPT node reference mismatch")
            }
            Self::NonCanonicalMpt(role) => write!(formatter, "non-canonical {role:?} MPT"),
            Self::MptKeyMismatch(role) => write!(formatter, "{role:?} MPT key mismatch"),
            Self::UnusedMptNodes(role) => write!(formatter, "unused {role:?} MPT nodes"),
            Self::MalformedAccount => formatter.write_str("malformed canonical Ethereum account"),
            Self::RuntimeCodeHashMismatch => formatter.write_str("runtime code hash mismatch"),
            Self::MalformedReceipt => formatter.write_str("malformed canonical Ethereum receipt"),
            Self::FailedReceipt => formatter.write_str("failed Ethereum receipt"),
            Self::SourceEventLogMismatch => formatter.write_str("SCCP source event log mismatch"),
        }
    }
}

impl std::error::Error for EthereumNativeSourceErrorV1 {}

/// Fully authenticated result of native Ethereum SCCP source verification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValidatedEthereumNativeSourceV1 {
    /// Canonical typed source-identity commitment.
    pub source_identity_hash: H256,
    /// Canonical exact directed-lane commitment.
    pub lane_hash: H256,
    /// Governed trusted-anchor commitment.
    pub trusted_anchor_hash: H256,
    /// Native light-client state commitment after all updates.
    pub final_state_commitment: H256,
    /// Authenticated source message id.
    pub message_id: H256,
    /// Authenticated source payload hash.
    pub payload_hash: H256,
    /// Authenticated canonical lane-bound event digest.
    pub source_event_digest: H256,
    /// Finalized beacon slot.
    pub finalized_beacon_slot: u64,
    /// Finalized beacon block root.
    pub finalized_beacon_block_root: H256,
    /// Finalized execution block number.
    pub execution_block_number: u64,
    /// Finalized execution block hash.
    pub execution_block_hash: H256,
    /// Authenticated execution state root.
    pub execution_state_root: H256,
    /// Authenticated execution receipts root.
    pub execution_receipts_root: H256,
    /// Transaction index opened in the receipts trie.
    pub transaction_index: u64,
}

fn activation_from_wire(
    activation: &EthereumNativeForkActivationV1,
) -> Result<ForkActivation, EthereumNativeSourceErrorV1> {
    let version = <[u8; 4]>::try_from(activation.version.as_slice())
        .map_err(|_| EthereumNativeSourceErrorV1::MalformedWire("fork version"))?;
    Ok(ForkActivation::new(activation.epoch, version))
}

fn schedule_from_wire(
    schedule: &EthereumNativeForkScheduleV1,
) -> Result<ForkSchedule, EthereumNativeSourceErrorV1> {
    ForkSchedule::new(
        schedule.genesis_validators_root,
        [
            activation_from_wire(&schedule.altair)?,
            activation_from_wire(&schedule.bellatrix)?,
            activation_from_wire(&schedule.capella)?,
            activation_from_wire(&schedule.deneb)?,
            activation_from_wire(&schedule.electra)?,
            activation_from_wire(&schedule.fulu)?,
        ],
    )
    .map_err(Into::into)
}

fn beacon_header_from_wire(header: &EthereumNativeBeaconHeaderV1) -> BeaconBlockHeader {
    BeaconBlockHeader {
        slot: header.slot,
        proposer_index: header.proposer_index,
        parent_root: header.parent_root,
        state_root: header.state_root,
        body_root: header.body_root,
    }
}

fn capella_execution_from_wire(
    header: &EthereumNativeCapellaExecutionHeaderV1,
) -> Result<CapellaExecutionPayloadHeader, EthereumNativeSourceErrorV1> {
    let fee_recipient = <[u8; 20]>::try_from(header.fee_recipient.as_slice())
        .map_err(|_| EthereumNativeSourceErrorV1::MalformedWire("fee recipient"))?;
    let logs_bloom = <[u8; 256]>::try_from(header.logs_bloom.as_slice())
        .map_err(|_| EthereumNativeSourceErrorV1::MalformedWire("logs bloom"))?;
    Ok(CapellaExecutionPayloadHeader {
        parent_hash: header.parent_hash,
        fee_recipient,
        state_root: header.state_root,
        receipts_root: header.receipts_root,
        logs_bloom,
        prev_randao: header.prev_randao,
        block_number: header.block_number,
        gas_limit: header.gas_limit,
        gas_used: header.gas_used,
        timestamp: header.timestamp,
        extra_data: ExtraData::new(header.extra_data.clone())?,
        base_fee_per_gas: header.base_fee_per_gas,
        block_hash: header.block_hash,
        transactions_root: header.transactions_root,
        withdrawals_root: header.withdrawals_root,
    })
}

fn deneb_execution_from_wire(
    header: &EthereumNativeDenebExecutionHeaderV1,
) -> Result<DenebExecutionPayloadHeader, EthereumNativeSourceErrorV1> {
    let base = capella_execution_from_wire(&header.base)?;
    Ok(DenebExecutionPayloadHeader {
        parent_hash: base.parent_hash,
        fee_recipient: base.fee_recipient,
        state_root: base.state_root,
        receipts_root: base.receipts_root,
        logs_bloom: base.logs_bloom,
        prev_randao: base.prev_randao,
        block_number: base.block_number,
        gas_limit: base.gas_limit,
        gas_used: base.gas_used,
        timestamp: base.timestamp,
        extra_data: base.extra_data,
        base_fee_per_gas: base.base_fee_per_gas,
        block_hash: base.block_hash,
        transactions_root: base.transactions_root,
        withdrawals_root: base.withdrawals_root,
        blob_gas_used: header.blob_gas_used,
        excess_blob_gas: header.excess_blob_gas,
    })
}

fn fixed_roots<const N: usize>(
    roots: &[Vec<u8>],
    field: &'static str,
) -> Result<[Root; N], EthereumNativeSourceErrorV1> {
    if roots.len() != N {
        return Err(EthereumNativeSourceErrorV1::MalformedWire(field));
    }
    let parsed = roots
        .iter()
        .map(|root| {
            <Root>::try_from(root.as_slice())
                .map_err(|_| EthereumNativeSourceErrorV1::MalformedWire(field))
        })
        .collect::<Result<Vec<_>, _>>()?;
    parsed
        .try_into()
        .map_err(|_| EthereumNativeSourceErrorV1::MalformedWire(field))
}

fn light_client_header_from_wire(
    header: &EthereumNativeLightClientHeaderV1,
) -> Result<LightClientHeader, EthereumNativeSourceErrorV1> {
    let beacon = beacon_header_from_wire(&header.beacon);
    match (header.fork, &header.execution) {
        (EthereumNativeForkV1::Altair, None) if header.execution_branch.is_empty() => {
            Ok(LightClientHeader::Altair { beacon })
        }
        (EthereumNativeForkV1::Bellatrix, None) if header.execution_branch.is_empty() => {
            Ok(LightClientHeader::Bellatrix { beacon })
        }
        (
            EthereumNativeForkV1::Capella,
            Some(EthereumNativeExecutionHeaderV1::Capella(execution)),
        ) => Ok(LightClientHeader::Capella {
            beacon,
            execution: Box::new(capella_execution_from_wire(execution)?),
            execution_branch: fixed_roots::<4>(
                &header.execution_branch,
                "Capella execution branch",
            )?,
        }),
        (EthereumNativeForkV1::Deneb, Some(EthereumNativeExecutionHeaderV1::Deneb(execution))) => {
            Ok(LightClientHeader::Deneb {
                beacon,
                execution: Box::new(deneb_execution_from_wire(execution)?),
                execution_branch: fixed_roots::<4>(
                    &header.execution_branch,
                    "Deneb execution branch",
                )?,
            })
        }
        (
            EthereumNativeForkV1::Electra,
            Some(EthereumNativeExecutionHeaderV1::Deneb(execution)),
        ) => Ok(LightClientHeader::Electra {
            beacon,
            execution: Box::new(deneb_execution_from_wire(execution)?),
            execution_branch: fixed_roots::<4>(
                &header.execution_branch,
                "Electra execution branch",
            )?,
        }),
        (EthereumNativeForkV1::Fulu, Some(EthereumNativeExecutionHeaderV1::Deneb(execution))) => {
            Ok(LightClientHeader::Fulu {
                beacon,
                execution: Box::new(deneb_execution_from_wire(execution)?),
                execution_branch: fixed_roots::<4>(
                    &header.execution_branch,
                    "Fulu execution branch",
                )?,
            })
        }
        _ => Err(EthereumNativeSourceErrorV1::MalformedWire(
            "fork-specific light-client header",
        )),
    }
}

fn sync_committee_from_wire(
    committee: &EthereumNativeSyncCommitteeV1,
) -> Result<SyncCommittee, EthereumNativeSourceErrorV1> {
    if committee.public_keys.len() != SYNC_COMMITTEE_SIZE {
        return Err(EthereumNativeSourceErrorV1::MalformedWire(
            "sync committee length",
        ));
    }
    let public_keys = committee
        .public_keys
        .iter()
        .map(|key| {
            let key = <[u8; 48]>::try_from(key.as_slice()).map_err(|_| {
                EthereumNativeSourceErrorV1::MalformedWire("sync committee public key")
            })?;
            Ok(BlsPublicKey::new(key))
        })
        .collect::<Result<Vec<_>, EthereumNativeSourceErrorV1>>()?;
    let public_keys = <[BlsPublicKey; SYNC_COMMITTEE_SIZE]>::try_from(public_keys)
        .map_err(|_| EthereumNativeSourceErrorV1::MalformedWire("sync committee length"))?;
    let aggregate_public_key = <[u8; 48]>::try_from(committee.aggregate_public_key.as_slice())
        .map_err(|_| {
            EthereumNativeSourceErrorV1::MalformedWire("sync committee aggregate public key")
        })?;
    Ok(SyncCommittee::new(
        public_keys,
        BlsPublicKey::new(aggregate_public_key),
    ))
}

fn current_committee_branch_from_wire(
    fork: EthereumNativeForkV1,
    roots: &[Vec<u8>],
) -> Result<CurrentSyncCommitteeBranch, EthereumNativeSourceErrorV1> {
    Ok(match fork {
        EthereumNativeForkV1::Electra | EthereumNativeForkV1::Fulu => {
            CurrentSyncCommitteeBranch::Electra(fixed_roots::<6>(
                roots,
                "Electra current committee branch",
            )?)
        }
        _ => CurrentSyncCommitteeBranch::PreElectra(fixed_roots::<5>(
            roots,
            "pre-Electra current committee branch",
        )?),
    })
}

fn next_committee_branch_from_wire(
    fork: EthereumNativeForkV1,
    roots: &[Vec<u8>],
) -> Result<NextSyncCommitteeBranch, EthereumNativeSourceErrorV1> {
    Ok(match fork {
        EthereumNativeForkV1::Electra | EthereumNativeForkV1::Fulu => {
            NextSyncCommitteeBranch::Electra(fixed_roots::<6>(
                roots,
                "Electra next committee branch",
            )?)
        }
        _ => NextSyncCommitteeBranch::PreElectra(fixed_roots::<5>(
            roots,
            "pre-Electra next committee branch",
        )?),
    })
}

fn finality_branch_from_wire(
    fork: EthereumNativeForkV1,
    roots: &[Vec<u8>],
) -> Result<FinalityBranch, EthereumNativeSourceErrorV1> {
    Ok(match fork {
        EthereumNativeForkV1::Electra | EthereumNativeForkV1::Fulu => {
            FinalityBranch::Electra(fixed_roots::<7>(roots, "Electra finality branch")?)
        }
        _ => FinalityBranch::PreElectra(fixed_roots::<6>(roots, "pre-Electra finality branch")?),
    })
}

fn bootstrap_from_wire(
    bootstrap: &EthereumNativeLightClientBootstrapV1,
) -> Result<LightClientBootstrap, EthereumNativeSourceErrorV1> {
    Ok(LightClientBootstrap {
        header: light_client_header_from_wire(&bootstrap.header)?,
        current_sync_committee: sync_committee_from_wire(&bootstrap.current_sync_committee)?,
        current_sync_committee_branch: current_committee_branch_from_wire(
            bootstrap.header.fork,
            &bootstrap.current_sync_committee_branch,
        )?,
    })
}

fn update_from_wire(
    update: &EthereumNativeLightClientUpdateV1,
) -> Result<LightClientUpdate, EthereumNativeSourceErrorV1> {
    let sync_committee_bits =
        <[u8; SYNC_COMMITTEE_BITS_BYTES]>::try_from(update.sync_committee_bits.as_slice())
            .map_err(|_| EthereumNativeSourceErrorV1::MalformedWire("sync committee bitvector"))?;
    let sync_committee_signature = <[u8; 96]>::try_from(update.sync_committee_signature.as_slice())
        .map_err(|_| EthereumNativeSourceErrorV1::MalformedWire("sync committee signature"))?;
    Ok(LightClientUpdate {
        attested_header: light_client_header_from_wire(&update.attested_header)?,
        next_sync_committee: sync_committee_from_wire(&update.next_sync_committee)?,
        next_sync_committee_branch: next_committee_branch_from_wire(
            update.attested_header.fork,
            &update.next_sync_committee_branch,
        )?,
        finalized_header: light_client_header_from_wire(&update.finalized_header)?,
        finality_branch: finality_branch_from_wire(
            update.attested_header.fork,
            &update.finality_branch,
        )?,
        sync_aggregate: SyncAggregate::new(
            sync_committee_bits,
            BlsSignature::new(sync_committee_signature),
        ),
        signature_slot: update.signature_slot,
    })
}

fn is_ethereum_network(network: SccpNetworkV1) -> bool {
    matches!(
        network,
        SccpNetworkV1::EthereumMainnet | SccpNetworkV1::EthereumSepolia
    )
}

fn validate_trusted_anchor(
    anchor: &EthereumNativeTrustedAnchorV1,
) -> Result<EthereumLightClientState, EthereumNativeSourceErrorV1> {
    if anchor.version != 1 {
        return Err(EthereumNativeSourceErrorV1::UnsupportedVersion(
            "Ethereum trusted anchor",
        ));
    }
    if !is_ethereum_network(anchor.network) {
        return Err(EthereumNativeSourceErrorV1::UnsupportedNetwork);
    }
    let state = EthereumLightClientState::from_trusted_anchor(
        schedule_from_wire(&anchor.fork_schedule)?,
        anchor.trusted_beacon_block_root,
        bootstrap_from_wire(&anchor.bootstrap)?,
    )?;
    if state.state_commitment() != anchor.anchor_state_commitment {
        return Err(EthereumNativeSourceErrorV1::AnchorStateCommitmentMismatch);
    }
    Ok(state)
}

/// Validate and hash one governed native Ethereum trusted anchor.
///
/// # Errors
///
/// Returns an error when the anchor is malformed, selects a non-Ethereum
/// network, fails native bootstrap validation, or cannot be canonically encoded.
pub fn ethereum_native_trusted_anchor_hash_v1(
    anchor: &EthereumNativeTrustedAnchorV1,
) -> Result<H256, EthereumNativeSourceErrorV1> {
    validate_trusted_anchor(anchor)?;
    let encoded =
        norito::to_bytes(anchor).map_err(|_| EthereumNativeSourceErrorV1::InvalidNoritoEncoding)?;
    Ok(prefixed_blake2b(ETHEREUM_NATIVE_ANCHOR_PREFIX_V1, &encoded))
}

/// Decode a size-bounded, canonical Norito native Ethereum source proof.
///
/// # Errors
///
/// Returns an error for an oversized, malformed, non-canonical, or immediately
/// out-of-bounds proof encoding.
pub fn decode_ethereum_native_source_proof_v1(
    bytes: &[u8],
) -> Result<EthereumNativeSourceProofV1, EthereumNativeSourceErrorV1> {
    if bytes.len() > MAX_ENCODED_SOURCE_PROOF_BYTES {
        return Err(EthereumNativeSourceErrorV1::EncodedProofTooLarge(
            bytes.len(),
        ));
    }
    // Reject compressed envelopes before Norito can allocate their declared
    // uncompressed payload. V1 source proofs have one canonical uncompressed
    // representation, so accepting compression would add both an allocation
    // bomb surface and a second wire alias for the same proof.
    if bytes.len() < norito::core::Header::SIZE
        || bytes.get(..4) != Some(b"NRT0")
        || bytes.get(NORITO_COMPRESSION_OFFSET) != Some(&0)
    {
        return Err(EthereumNativeSourceErrorV1::InvalidNoritoEncoding);
    }
    let declared_length = bytes
        .get(NORITO_LENGTH_OFFSET..NORITO_LENGTH_OFFSET + 8)
        .and_then(|raw| <[u8; 8]>::try_from(raw).ok())
        .map(u64::from_le_bytes)
        .ok_or(EthereumNativeSourceErrorV1::InvalidNoritoEncoding)?;
    if declared_length > MAX_ENCODED_SOURCE_PROOF_BYTES_U64 {
        return Err(EthereumNativeSourceErrorV1::EncodedProofTooLarge(
            usize::try_from(declared_length).unwrap_or(usize::MAX),
        ));
    }
    let proof: EthereumNativeSourceProofV1 = norito::decode_from_bytes(bytes)
        .map_err(|_| EthereumNativeSourceErrorV1::InvalidNoritoEncoding)?;
    if norito::to_bytes(&proof).map_err(|_| EthereumNativeSourceErrorV1::InvalidNoritoEncoding)?
        != bytes
    {
        return Err(EthereumNativeSourceErrorV1::InvalidNoritoEncoding);
    }
    validate_decoded_source_proof_bounds(&proof)?;
    Ok(proof)
}

/// Decode a size-bounded JSON native Ethereum source proof.
///
/// # Errors
///
/// Returns an error for an oversized or malformed JSON document, or when the
/// decoded proof exceeds update or MPT bounds.
pub fn decode_ethereum_native_source_proof_json_v1(
    json: &str,
) -> Result<EthereumNativeSourceProofV1, EthereumNativeSourceErrorV1> {
    if json.len() > MAX_JSON_SOURCE_PROOF_BYTES {
        return Err(EthereumNativeSourceErrorV1::EncodedProofTooLarge(
            json.len(),
        ));
    }
    let proof: EthereumNativeSourceProofV1 = norito::json::from_json(json)
        .map_err(|_| EthereumNativeSourceErrorV1::InvalidNoritoEncoding)?;
    validate_decoded_source_proof_bounds(&proof)?;
    Ok(proof)
}

fn validate_decoded_source_proof_bounds(
    proof: &EthereumNativeSourceProofV1,
) -> Result<(), EthereumNativeSourceErrorV1> {
    if proof.updates.len() > ETHEREUM_NATIVE_MAX_LIGHT_CLIENT_UPDATES {
        return Err(EthereumNativeSourceErrorV1::TooManyLightClientUpdates(
            proof.updates.len(),
        ));
    }
    validate_mpt_proof_bounds(&proof.account_proof, EthereumNativeMptRoleV1::Account)?;
    validate_mpt_proof_bounds(&proof.receipt_proof, EthereumNativeMptRoleV1::Receipt)?;
    Ok(())
}

#[derive(Clone, Copy)]
enum RlpItem<'a> {
    Bytes { payload: &'a [u8] },
    List { payload: &'a [u8], raw: &'a [u8] },
}

fn read_big_endian_len(bytes: &[u8]) -> Option<usize> {
    if bytes.is_empty() || bytes[0] == 0 || bytes.len() > core::mem::size_of::<usize>() {
        return None;
    }
    bytes.iter().try_fold(0usize, |value, byte| {
        value.checked_mul(256)?.checked_add(usize::from(*byte))
    })
}

fn parse_rlp_item_at<'a>(bytes: &'a [u8], cursor: &mut usize) -> Option<RlpItem<'a>> {
    let start = *cursor;
    let first = *bytes.get(start)?;
    match first {
        0x00..=0x7f => {
            *cursor = start.checked_add(1)?;
            Some(RlpItem::Bytes {
                payload: bytes.get(start..*cursor)?,
            })
        }
        0x80..=0xb7 => {
            let len = usize::from(first - 0x80);
            let payload_start = start.checked_add(1)?;
            let end = payload_start.checked_add(len)?;
            let payload = bytes.get(payload_start..end)?;
            if len == 1 && payload[0] < 0x80 {
                return None;
            }
            *cursor = end;
            Some(RlpItem::Bytes { payload })
        }
        0xb8..=0xbf => {
            let len_len = usize::from(first - 0xb7);
            let length_start = start.checked_add(1)?;
            let length_end = length_start.checked_add(len_len)?;
            let len = read_big_endian_len(bytes.get(length_start..length_end)?)?;
            if len < 56 {
                return None;
            }
            let end = length_end.checked_add(len)?;
            let payload = bytes.get(length_end..end)?;
            *cursor = end;
            Some(RlpItem::Bytes { payload })
        }
        0xc0..=0xf7 => {
            let len = usize::from(first - 0xc0);
            let payload_start = start.checked_add(1)?;
            let end = payload_start.checked_add(len)?;
            let payload = bytes.get(payload_start..end)?;
            *cursor = end;
            Some(RlpItem::List {
                payload,
                raw: bytes.get(start..end)?,
            })
        }
        0xf8..=0xff => {
            let len_len = usize::from(first - 0xf7);
            let length_start = start.checked_add(1)?;
            let length_end = length_start.checked_add(len_len)?;
            let len = read_big_endian_len(bytes.get(length_start..length_end)?)?;
            if len < 56 {
                return None;
            }
            let end = length_end.checked_add(len)?;
            let payload = bytes.get(length_end..end)?;
            *cursor = end;
            Some(RlpItem::List {
                payload,
                raw: bytes.get(start..end)?,
            })
        }
    }
}

fn parse_single_rlp(bytes: &[u8]) -> Option<RlpItem<'_>> {
    let mut cursor = 0usize;
    let item = parse_rlp_item_at(bytes, &mut cursor)?;
    (cursor == bytes.len()).then_some(item)
}

fn parse_rlp_list(bytes: &[u8], max_items: usize) -> Option<Vec<RlpItem<'_>>> {
    let RlpItem::List { payload, .. } = parse_single_rlp(bytes)? else {
        return None;
    };
    parse_rlp_list_payload(payload, max_items)
}

fn parse_rlp_list_payload(payload: &[u8], max_items: usize) -> Option<Vec<RlpItem<'_>>> {
    let mut cursor = 0usize;
    let mut items = Vec::new();
    while cursor < payload.len() {
        if items.len() == max_items {
            return None;
        }
        items.push(parse_rlp_item_at(payload, &mut cursor)?);
    }
    (cursor == payload.len()).then_some(items)
}

fn rlp_bytes(item: RlpItem<'_>) -> Option<&[u8]> {
    match item {
        RlpItem::Bytes { payload, .. } => Some(payload),
        RlpItem::List { .. } => None,
    }
}

fn keccak256(bytes: &[u8]) -> H256 {
    let mut hasher = Keccak::v256();
    hasher.update(bytes);
    let mut output = [0u8; 32];
    hasher.finalize(&mut output);
    output
}

fn key_nibbles(bytes: &[u8]) -> Vec<u8> {
    let mut nibbles = Vec::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        nibbles.push(byte >> 4);
        nibbles.push(byte & 0x0f);
    }
    nibbles
}

fn decode_compact_path(bytes: &[u8]) -> Option<(bool, Vec<u8>)> {
    if bytes.is_empty() {
        return None;
    }
    let nibbles = key_nibbles(bytes);
    let flag = *nibbles.first()?;
    if flag > 3 {
        return None;
    }
    let is_leaf = flag & 2 != 0;
    let odd = flag & 1 != 0;
    if odd {
        Some((is_leaf, nibbles.get(1..)?.to_vec()))
    } else {
        if nibbles.get(1) != Some(&0) {
            return None;
        }
        Some((is_leaf, nibbles.get(2..)?.to_vec()))
    }
}

#[derive(Clone)]
enum MptNodeReference {
    Hash(H256),
    Inline(Vec<u8>),
}

fn child_reference(item: RlpItem<'_>) -> Result<Option<MptNodeReference>, ()> {
    match item {
        RlpItem::Bytes { payload: [] } => Ok(None),
        RlpItem::Bytes { payload, .. } => {
            let hash = H256::try_from(payload).map_err(|_| ())?;
            if hash.iter().all(|byte| *byte == 0) {
                return Err(());
            }
            Ok(Some(MptNodeReference::Hash(hash)))
        }
        RlpItem::List { raw, .. } if raw.len() < 32 => {
            Ok(Some(MptNodeReference::Inline(raw.to_vec())))
        }
        RlpItem::List { .. } => Err(()),
    }
}

fn validate_mpt_proof_bounds(
    proof: &EthereumNativeMptProofV1,
    role: EthereumNativeMptRoleV1,
) -> Result<(), EthereumNativeSourceErrorV1> {
    if proof.nodes.is_empty() || proof.nodes.len() > MAX_MPT_PROOF_NODES {
        return Err(EthereumNativeSourceErrorV1::MptProofBounds(role));
    }
    let mut total = 0usize;
    let mut seen = BTreeSet::new();
    for node in &proof.nodes {
        if node.is_empty() || node.len() > MAX_MPT_NODE_BYTES {
            return Err(EthereumNativeSourceErrorV1::MptProofBounds(role));
        }
        total = total
            .checked_add(node.len())
            .ok_or(EthereumNativeSourceErrorV1::MptProofBounds(role))?;
        if total > MAX_MPT_PROOF_BYTES {
            return Err(EthereumNativeSourceErrorV1::MptProofBounds(role));
        }
        if !seen.insert(node.as_slice()) {
            return Err(EthereumNativeSourceErrorV1::DuplicateMptNode(role));
        }
    }
    Ok(())
}

fn verify_mpt_inclusion(
    root: H256,
    key: &[u8],
    proof: &EthereumNativeMptProofV1,
    role: EthereumNativeMptRoleV1,
) -> Result<Vec<u8>, EthereumNativeSourceErrorV1> {
    if root.iter().all(|byte| *byte == 0) || root == EMPTY_TRIE_ROOT {
        return Err(EthereumNativeSourceErrorV1::EmptyTrieRoot(role));
    }
    validate_mpt_proof_bounds(proof, role)?;
    let path = key_nibbles(key);
    let mut path_cursor = 0usize;
    let mut proof_cursor = 0usize;
    let mut expected = MptNodeReference::Hash(root);
    let mut first_node = true;
    let mut previous_was_extension = false;

    loop {
        let raw = match expected {
            MptNodeReference::Hash(expected_hash) => {
                let raw = proof
                    .nodes
                    .get(proof_cursor)
                    .ok_or(EthereumNativeSourceErrorV1::MptNodeReferenceMismatch(role))?;
                proof_cursor = proof_cursor
                    .checked_add(1)
                    .ok_or(EthereumNativeSourceErrorV1::MptProofBounds(role))?;
                if keccak256(raw) != expected_hash || (!first_node && raw.len() < 32) {
                    return Err(EthereumNativeSourceErrorV1::MptNodeReferenceMismatch(role));
                }
                raw.clone()
            }
            MptNodeReference::Inline(raw) => raw,
        };
        first_node = false;
        let items =
            parse_rlp_list(&raw, 17).ok_or(EthereumNativeSourceErrorV1::NonCanonicalMpt(role))?;
        match items.len() {
            17 => {
                previous_was_extension = false;
                let mut child_count = 0usize;
                for item in &items[..16] {
                    if child_reference(*item)
                        .map_err(|()| EthereumNativeSourceErrorV1::NonCanonicalMpt(role))?
                        .is_some()
                    {
                        child_count += 1;
                    }
                }
                let value = rlp_bytes(items[16])
                    .ok_or(EthereumNativeSourceErrorV1::NonCanonicalMpt(role))?;
                if (value.is_empty() && child_count < 2) || (!value.is_empty() && child_count == 0)
                {
                    return Err(EthereumNativeSourceErrorV1::NonCanonicalMpt(role));
                }
                if path_cursor == path.len() {
                    if value.is_empty() {
                        return Err(EthereumNativeSourceErrorV1::MptKeyMismatch(role));
                    }
                    if proof_cursor != proof.nodes.len() {
                        return Err(EthereumNativeSourceErrorV1::UnusedMptNodes(role));
                    }
                    return Ok(value.to_vec());
                }
                let nibble = usize::from(path[path_cursor]);
                path_cursor += 1;
                expected = child_reference(items[nibble])
                    .map_err(|()| EthereumNativeSourceErrorV1::NonCanonicalMpt(role))?
                    .ok_or(EthereumNativeSourceErrorV1::MptKeyMismatch(role))?;
            }
            2 => {
                let compact = rlp_bytes(items[0])
                    .ok_or(EthereumNativeSourceErrorV1::NonCanonicalMpt(role))?;
                let (is_leaf, partial_path) = decode_compact_path(compact)
                    .ok_or(EthereumNativeSourceErrorV1::NonCanonicalMpt(role))?;
                if (!is_leaf && partial_path.is_empty()) || (!is_leaf && previous_was_extension) {
                    return Err(EthereumNativeSourceErrorV1::NonCanonicalMpt(role));
                }
                let remaining = path
                    .get(path_cursor..)
                    .ok_or(EthereumNativeSourceErrorV1::MptKeyMismatch(role))?;
                if !remaining.starts_with(&partial_path) {
                    return Err(EthereumNativeSourceErrorV1::MptKeyMismatch(role));
                }
                path_cursor = path_cursor
                    .checked_add(partial_path.len())
                    .ok_or(EthereumNativeSourceErrorV1::MptProofBounds(role))?;
                if is_leaf {
                    if path_cursor != path.len() {
                        return Err(EthereumNativeSourceErrorV1::MptKeyMismatch(role));
                    }
                    let value = rlp_bytes(items[1])
                        .filter(|value| !value.is_empty())
                        .ok_or(EthereumNativeSourceErrorV1::NonCanonicalMpt(role))?;
                    if proof_cursor != proof.nodes.len() {
                        return Err(EthereumNativeSourceErrorV1::UnusedMptNodes(role));
                    }
                    return Ok(value.to_vec());
                }
                expected = child_reference(items[1])
                    .map_err(|()| EthereumNativeSourceErrorV1::NonCanonicalMpt(role))?
                    .ok_or(EthereumNativeSourceErrorV1::NonCanonicalMpt(role))?;
                previous_was_extension = true;
            }
            _ => return Err(EthereumNativeSourceErrorV1::NonCanonicalMpt(role)),
        }
    }
}

fn canonical_uint_bytes(item: RlpItem<'_>, max_bytes: usize) -> Option<&[u8]> {
    let bytes = rlp_bytes(item)?;
    if bytes.len() > max_bytes || bytes.first() == Some(&0) {
        return None;
    }
    Some(bytes)
}

fn account_storage_root(
    value: &[u8],
    expected_code_hash: H256,
) -> Result<H256, EthereumNativeSourceErrorV1> {
    let fields = parse_rlp_list(value, 4).ok_or(EthereumNativeSourceErrorV1::MalformedAccount)?;
    if fields.len() != 4
        || canonical_uint_bytes(fields[0], 8).is_none()
        || canonical_uint_bytes(fields[1], 32).is_none()
    {
        return Err(EthereumNativeSourceErrorV1::MalformedAccount);
    }
    let storage_root =
        H256::try_from(rlp_bytes(fields[2]).ok_or(EthereumNativeSourceErrorV1::MalformedAccount)?)
            .map_err(|_| EthereumNativeSourceErrorV1::MalformedAccount)?;
    let code_hash =
        H256::try_from(rlp_bytes(fields[3]).ok_or(EthereumNativeSourceErrorV1::MalformedAccount)?)
            .map_err(|_| EthereumNativeSourceErrorV1::MalformedAccount)?;
    if storage_root.iter().all(|byte| *byte == 0) || storage_root == EMPTY_TRIE_ROOT {
        return Err(EthereumNativeSourceErrorV1::MalformedAccount);
    }
    if code_hash != expected_code_hash {
        return Err(EthereumNativeSourceErrorV1::RuntimeCodeHashMismatch);
    }
    if code_hash == EMPTY_CODE_HASH {
        return Err(EthereumNativeSourceErrorV1::RuntimeCodeHashMismatch);
    }
    Ok(storage_root)
}

fn rlp_encode_u64(value: u64) -> Vec<u8> {
    if value == 0 {
        return vec![0x80];
    }
    let bytes = value.to_be_bytes();
    let first = usize::from(u8::try_from(value.leading_zeros() / 8).unwrap_or(8));
    let value = &bytes[first..];
    if value.len() == 1 && value[0] < 0x80 {
        return value.to_vec();
    }
    let mut encoded = Vec::with_capacity(1 + value.len());
    let prefix = match value.len() {
        1 => 0x81,
        2 => 0x82,
        3 => 0x83,
        4 => 0x84,
        5 => 0x85,
        6 => 0x86,
        7 => 0x87,
        8 => 0x88,
        _ => return vec![0x80],
    };
    encoded.push(prefix);
    encoded.extend_from_slice(value);
    encoded
}

fn validate_receipt_event(
    receipt: &[u8],
    expected_emitter: [u8; 20],
    expected_lane_hash: H256,
    expected_message_id: H256,
    expected_event_digest: H256,
    expected_payload_hash: H256,
    expected_route_config_hash: H256,
    expected_payload: &[u8],
) -> Result<(), EthereumNativeSourceErrorV1> {
    let payload = match receipt.first().copied() {
        Some(0x01..=0x04) => receipt
            .get(1..)
            .ok_or(EthereumNativeSourceErrorV1::MalformedReceipt)?,
        Some(0xc0..=0xff) => receipt,
        _ => return Err(EthereumNativeSourceErrorV1::MalformedReceipt),
    };
    let fields = parse_rlp_list(payload, 4).ok_or(EthereumNativeSourceErrorV1::MalformedReceipt)?;
    if fields.len() != 4 {
        return Err(EthereumNativeSourceErrorV1::MalformedReceipt);
    }
    let status = rlp_bytes(fields[0]).ok_or(EthereumNativeSourceErrorV1::MalformedReceipt)?;
    if status.is_empty() {
        return Err(EthereumNativeSourceErrorV1::FailedReceipt);
    }
    if status != [1] {
        return Err(EthereumNativeSourceErrorV1::MalformedReceipt);
    }
    if canonical_uint_bytes(fields[1], 8).is_none_or(|bytes| bytes.is_empty()) {
        return Err(EthereumNativeSourceErrorV1::MalformedReceipt);
    }
    if rlp_bytes(fields[2]).is_none_or(|bloom| bloom.len() != 256) {
        return Err(EthereumNativeSourceErrorV1::MalformedReceipt);
    }
    let RlpItem::List {
        payload: logs_payload,
        ..
    } = fields[3]
    else {
        return Err(EthereumNativeSourceErrorV1::MalformedReceipt);
    };
    let logs = parse_rlp_list_payload(logs_payload, MAX_RECEIPT_LOGS)
        .ok_or(EthereumNativeSourceErrorV1::MalformedReceipt)?;
    let event_signature = keccak256(ETHEREUM_SOURCE_EVENT_SIGNATURE_V1);
    let mut matches = 0usize;
    for log in logs {
        let RlpItem::List { payload, .. } = log else {
            return Err(EthereumNativeSourceErrorV1::MalformedReceipt);
        };
        let log_fields = parse_rlp_list_payload(payload, 3)
            .ok_or(EthereumNativeSourceErrorV1::MalformedReceipt)?;
        if log_fields.len() != 3 {
            return Err(EthereumNativeSourceErrorV1::MalformedReceipt);
        }
        let address = rlp_bytes(log_fields[0])
            .and_then(|bytes| <[u8; 20]>::try_from(bytes).ok())
            .ok_or(EthereumNativeSourceErrorV1::MalformedReceipt)?;
        let RlpItem::List {
            payload: topics_payload,
            ..
        } = log_fields[1]
        else {
            return Err(EthereumNativeSourceErrorV1::MalformedReceipt);
        };
        let topics = parse_rlp_list_payload(topics_payload, MAX_LOG_TOPICS)
            .ok_or(EthereumNativeSourceErrorV1::MalformedReceipt)?;
        let topic_bytes = topics
            .iter()
            .map(|topic| {
                rlp_bytes(*topic)
                    .filter(|bytes| bytes.len() == 32)
                    .ok_or(EthereumNativeSourceErrorV1::MalformedReceipt)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let data = rlp_bytes(log_fields[2]).ok_or(EthereumNativeSourceErrorV1::MalformedReceipt)?;
        if address == expected_emitter
            && topic_bytes
                .first()
                .is_some_and(|topic| *topic == event_signature)
        {
            if topic_bytes.len() != 4
                || topic_bytes[1] != expected_lane_hash
                || topic_bytes[2] != expected_message_id
                || topic_bytes[3] != expected_event_digest
                || !canonical_transfer_event_data_matches(
                    data,
                    expected_payload_hash,
                    expected_route_config_hash,
                    expected_payload,
                )
            {
                return Err(EthereumNativeSourceErrorV1::SourceEventLogMismatch);
            }
            matches = matches.saturating_add(1);
        }
    }
    if matches != 1 {
        return Err(EthereumNativeSourceErrorV1::SourceEventLogMismatch);
    }
    Ok(())
}

fn canonical_transfer_event_data_matches(
    data: &[u8],
    expected_payload_hash: H256,
    expected_route_config_hash: H256,
    expected_payload: &[u8],
) -> bool {
    if data.len() < 128
        || data.get(..32) != Some(expected_payload_hash.as_slice())
        || data.get(32..64) != Some(expected_route_config_hash.as_slice())
    {
        return false;
    }
    let Some(offset_word) = data.get(64..96) else {
        return false;
    };
    if offset_word[..31].iter().any(|byte| *byte != 0) || offset_word[31] != 96 {
        return false;
    }
    let Some(length_word) = data.get(96..128) else {
        return false;
    };
    if length_word[..24].iter().any(|byte| *byte != 0) {
        return false;
    }
    let mut raw_len = [0u8; 8];
    raw_len.copy_from_slice(&length_word[24..]);
    let Ok(payload_len) = usize::try_from(u64::from_be_bytes(raw_len)) else {
        return false;
    };
    if payload_len != expected_payload.len() {
        return false;
    }
    let Some(padded_len) = payload_len.checked_add(31).map(|len| len & !31) else {
        return false;
    };
    let Some(expected_len) = 128usize.checked_add(padded_len) else {
        return false;
    };
    data.len() == expected_len
        && data.get(128..128 + payload_len) == Some(expected_payload)
        && data[128 + payload_len..].iter().all(|byte| *byte == 0)
}

fn finalized_execution_matches(
    explicit: &EthereumNativeFinalizedExecutionV1,
    header: &LightClientHeader,
    execution: AuthenticatedExecutionBlock,
) -> bool {
    explicit.fork == EthereumNativeForkV1::from(execution.fork)
        && explicit.beacon_slot == header.beacon().slot
        && explicit.beacon_block_root == header.beacon().hash_tree_root()
        && explicit.block_number == execution.block_number
        && explicit.block_hash == execution.block_hash
        && explicit.state_root == execution.state_root
        && explicit.receipts_root == execution.receipts_root
        && explicit.block_number != 0
        && explicit.block_hash.iter().any(|byte| *byte != 0)
        && explicit.state_root.iter().any(|byte| *byte != 0)
        && explicit.state_root != EMPTY_TRIE_ROOT
        && explicit.receipts_root.iter().any(|byte| *byte != 0)
        && explicit.receipts_root != EMPTY_TRIE_ROOT
}

/// Verify a complete native Ethereum SCCP source proof.
///
/// The caller supplies the exact expected identity and its canonical hash, the
/// governed trusted-anchor hash, and the message statement. The function never
/// falls back to a numeric domain or an address-only identity.
///
/// # Errors
///
/// Returns a role-specific error when any identity, light-client, MPT, account,
/// receipt, or exact transfer-event binding fails validation.
pub fn verify_ethereum_native_source_proof_v1(
    expected_source_identity: &SccpSourceIdentityV1,
    expected_source_identity_hash: H256,
    expected_trusted_anchor_hash: H256,
    expected_message_id: H256,
    expected_payload_hash: H256,
    expected_payload: &[u8],
    proof: &EthereumNativeSourceProofV1,
) -> Result<ValidatedEthereumNativeSourceV1, EthereumNativeSourceErrorV1> {
    if proof.version != 1 {
        return Err(EthereumNativeSourceErrorV1::UnsupportedVersion(
            "Ethereum source proof",
        ));
    }
    if &proof.source_identity != expected_source_identity {
        return Err(EthereumNativeSourceErrorV1::SourceIdentityMismatch);
    }
    if !proof.source_identity.is_well_formed()
        || !is_ethereum_network(proof.source_identity.lane.source)
        || !matches!(
            proof.source_identity.lane.target,
            SccpNetworkV1::SoraNexus | SccpNetworkV1::SoraTaira
        )
    {
        return Err(EthereumNativeSourceErrorV1::InvalidSourceIdentity);
    }
    let SccpSourceEmitterV1::Evm(SccpEvmSourceEmitterV1 {
        address,
        runtime_code_hash,
        route_config_hash,
    }) = proof.source_identity.emitter
    else {
        return Err(EthereumNativeSourceErrorV1::InvalidSourceIdentity);
    };
    if runtime_code_hash == EMPTY_CODE_HASH {
        return Err(EthereumNativeSourceErrorV1::InvalidSourceIdentity);
    }

    let identity_hash = sccp_source_identity_hash_v1(&proof.source_identity)
        .ok_or(EthereumNativeSourceErrorV1::InvalidSourceIdentity)?;
    if expected_source_identity_hash.iter().all(|byte| *byte == 0)
        || identity_hash != expected_source_identity_hash
        || proof.source_identity_hash != identity_hash
    {
        return Err(EthereumNativeSourceErrorV1::SourceIdentityHashMismatch);
    }
    let lane_hash = sccp_lane_id_hash_v1(proof.source_identity.lane)
        .ok_or(EthereumNativeSourceErrorV1::InvalidSourceIdentity)?;
    if proof.lane_hash != lane_hash {
        return Err(EthereumNativeSourceErrorV1::LaneHashMismatch);
    }
    if proof.message_id != expected_message_id
        || proof.payload_hash != expected_payload_hash
        || expected_message_id.iter().all(|byte| *byte == 0)
        || expected_payload_hash.iter().all(|byte| *byte == 0)
        || expected_payload.is_empty()
        || super::payload_hash(expected_payload) != expected_payload_hash
    {
        return Err(EthereumNativeSourceErrorV1::MessageStatementMismatch);
    }
    let decoded_payload = decode_canonical_sccp_payload_bytes(expected_payload)
        .ok_or(EthereumNativeSourceErrorV1::MessageStatementMismatch)?;
    if !matches!(decoded_payload, SccpPayloadV1::Transfer(_))
        || canonical_sccp_payload_bytes(&decoded_payload) != expected_payload
        || sccp_message_id(proof.source_identity.lane, &decoded_payload)
            != Some(expected_message_id)
    {
        return Err(EthereumNativeSourceErrorV1::MessageStatementMismatch);
    }
    let source_event_digest = sccp_lane_source_event_digest_v1(
        proof.source_identity.lane,
        expected_message_id,
        expected_payload_hash,
    )
    .ok_or(EthereumNativeSourceErrorV1::SourceEventDigestMismatch)?;
    if proof.source_event_digest != source_event_digest {
        return Err(EthereumNativeSourceErrorV1::SourceEventDigestMismatch);
    }

    if proof.trusted_anchor.network != proof.source_identity.lane.source {
        return Err(EthereumNativeSourceErrorV1::UnsupportedNetwork);
    }
    if proof.updates.len() > ETHEREUM_NATIVE_MAX_LIGHT_CLIENT_UPDATES {
        return Err(EthereumNativeSourceErrorV1::TooManyLightClientUpdates(
            proof.updates.len(),
        ));
    }
    let mut state = validate_trusted_anchor(&proof.trusted_anchor)?;
    let encoded_anchor = norito::to_bytes(&proof.trusted_anchor)
        .map_err(|_| EthereumNativeSourceErrorV1::InvalidNoritoEncoding)?;
    let trusted_anchor_hash = prefixed_blake2b(ETHEREUM_NATIVE_ANCHOR_PREFIX_V1, &encoded_anchor);
    if expected_trusted_anchor_hash.iter().all(|byte| *byte == 0)
        || proof.trusted_anchor_hash != trusted_anchor_hash
        || expected_trusted_anchor_hash != trusted_anchor_hash
    {
        return Err(EthereumNativeSourceErrorV1::TrustedAnchorHashMismatch);
    }
    for update in &proof.updates {
        state = state.validate_and_apply(update_from_wire(update)?)?;
    }
    let final_state_commitment = state.state_commitment();
    if proof.final_state_commitment != final_state_commitment {
        return Err(EthereumNativeSourceErrorV1::FinalStateCommitmentMismatch);
    }
    let finalized_header = state.finalized_header();
    let execution = finalized_header
        .authenticated_execution_block()
        .ok_or(EthereumNativeSourceErrorV1::MissingFinalizedExecution)?;
    if !finalized_execution_matches(&proof.finalized_execution, finalized_header, execution) {
        return Err(EthereumNativeSourceErrorV1::FinalizedExecutionMismatch);
    }

    let account_key = keccak256(&address);
    let account_value = verify_mpt_inclusion(
        execution.state_root,
        &account_key,
        &proof.account_proof,
        EthereumNativeMptRoleV1::Account,
    )?;
    let _storage_root = account_storage_root(&account_value, runtime_code_hash)?;
    let receipt_key = rlp_encode_u64(proof.transaction_index);
    let receipt = verify_mpt_inclusion(
        execution.receipts_root,
        &receipt_key,
        &proof.receipt_proof,
        EthereumNativeMptRoleV1::Receipt,
    )?;
    validate_receipt_event(
        &receipt,
        address,
        lane_hash,
        expected_message_id,
        source_event_digest,
        expected_payload_hash,
        route_config_hash,
        expected_payload,
    )?;

    Ok(ValidatedEthereumNativeSourceV1 {
        source_identity_hash: identity_hash,
        lane_hash,
        trusted_anchor_hash,
        final_state_commitment,
        message_id: expected_message_id,
        payload_hash: expected_payload_hash,
        source_event_digest,
        finalized_beacon_slot: finalized_header.beacon().slot,
        finalized_beacon_block_root: finalized_header.beacon().hash_tree_root(),
        execution_block_number: execution.block_number,
        execution_block_hash: execution.block_hash,
        execution_state_root: execution.state_root,
        execution_receipts_root: execution.receipts_root,
        transaction_index: proof.transaction_index,
    })
}

/// Build a complete positive fixture for one caller-supplied SCCP statement.
#[cfg(any(test, feature = "test-fixtures"))]
pub(crate) fn ethereum_native_positive_test_fixture_for_statement(
    message_id: H256,
    canonical_payload: &[u8],
) -> (
    SccpSourceIdentityV1,
    H256,
    H256,
    H256,
    H256,
    EthereumNativeSourceProofV1,
) {
    let (identity, identity_hash, anchor_hash, proof) =
        test_fixtures::source_fixture_for_statement(message_id, canonical_payload);
    (
        identity,
        identity_hash,
        anchor_hash,
        proof.message_id,
        proof.payload_hash,
        proof,
    )
}

#[cfg(any(test, feature = "test-fixtures"))]
mod test_fixtures {
    use std::collections::BTreeMap;

    use sha2::{Digest as _, Sha256};

    use super::*;
    use crate::{SccpLaneIdV1, sccp_source_identity_hash_v1};

    const GENERATOR_PUBLIC_KEY: [u8; 48] = [
        0x97, 0xf1, 0xd3, 0xa7, 0x31, 0x97, 0xd7, 0x94, 0x26, 0x95, 0x63, 0x8c, 0x4f, 0xa9, 0xac,
        0x0f, 0xc3, 0x68, 0x8c, 0x4f, 0x97, 0x74, 0xb9, 0x05, 0xa1, 0x4e, 0x3a, 0x3f, 0x17, 0x1b,
        0xac, 0x58, 0x6c, 0x55, 0xe8, 0x3f, 0xf9, 0x7a, 0x1a, 0xef, 0xfb, 0x3a, 0xf0, 0x0a, 0xdb,
        0x22, 0xc6, 0xbb,
    ];

    fn hash_nodes(left: &H256, right: &H256) -> H256 {
        let mut hasher = Sha256::new();
        hasher.update(left);
        hasher.update(right);
        hasher.finalize().into()
    }

    fn merkle_root_from_branch(mut leaf: H256, gindex: u64, branch: &[H256]) -> H256 {
        for (height, sibling) in branch.iter().enumerate() {
            leaf = if (gindex >> height) & 1 == 0 {
                hash_nodes(&leaf, sibling)
            } else {
                hash_nodes(sibling, &leaf)
            };
        }
        leaf
    }

    fn sparse_node(gindex: u64, max_depth: usize, explicit: &BTreeMap<u64, H256>) -> H256 {
        if let Some(value) = explicit.get(&gindex) {
            return *value;
        }
        let depth = (u64::BITS - 1 - gindex.leading_zeros()) as usize;
        if depth == max_depth {
            return [0; 32];
        }
        hash_nodes(
            &sparse_node(gindex * 2, max_depth, explicit),
            &sparse_node(gindex * 2 + 1, max_depth, explicit),
        )
    }

    fn sparse_branch(target: u64, max_depth: usize, explicit: &BTreeMap<u64, H256>) -> Vec<H256> {
        let depth = (u64::BITS - 1 - target.leading_zeros()) as usize;
        let mut branch = Vec::with_capacity(depth);
        let mut node = target;
        for _ in 0..depth {
            branch.push(sparse_node(node ^ 1, max_depth, explicit));
            node >>= 1;
        }
        branch
    }

    fn encode_length(len: usize, short: u8, long: u8) -> Vec<u8> {
        if len < 56 {
            return vec![short + u8::try_from(len).unwrap()];
        }
        let raw = len.to_be_bytes();
        let first = raw.iter().position(|byte| *byte != 0).unwrap();
        let len_bytes = &raw[first..];
        let mut out = vec![long + u8::try_from(len_bytes.len()).unwrap()];
        out.extend_from_slice(len_bytes);
        out
    }

    pub(super) fn encode_bytes(bytes: &[u8]) -> Vec<u8> {
        if bytes.len() == 1 && bytes[0] < 0x80 {
            return bytes.to_vec();
        }
        let mut out = encode_length(bytes.len(), 0x80, 0xb7);
        out.extend_from_slice(bytes);
        out
    }

    pub(super) fn encode_list(fields: &[Vec<u8>]) -> Vec<u8> {
        let len = fields.iter().map(Vec::len).sum();
        let mut out = encode_length(len, 0xc0, 0xf7);
        for field in fields {
            out.extend_from_slice(field);
        }
        out
    }

    pub(super) fn single_leaf_proof(key: &[u8], value: &[u8]) -> (H256, EthereumNativeMptProofV1) {
        let mut compact_path = Vec::with_capacity(1 + key.len());
        compact_path.push(0x20);
        compact_path.extend_from_slice(key);
        let node = encode_list(&[encode_bytes(&compact_path), encode_bytes(value)]);
        (
            keccak256(&node),
            EthereumNativeMptProofV1 { nodes: vec![node] },
        )
    }

    fn activation(epoch: u64, tag: u8) -> EthereumNativeForkActivationV1 {
        EthereumNativeForkActivationV1 {
            epoch,
            version: vec![tag, 0, 0, 0],
        }
    }

    fn schedule_wire() -> EthereumNativeForkScheduleV1 {
        EthereumNativeForkScheduleV1 {
            genesis_validators_root: [0xa5; 32],
            altair: activation(0, 1),
            bellatrix: activation(0, 2),
            capella: activation(0, 3),
            deneb: activation(u64::MAX, 4),
            electra: activation(u64::MAX, 5),
            fulu: activation(u64::MAX, 6),
        }
    }

    fn committee_wire() -> EthereumNativeSyncCommitteeV1 {
        EthereumNativeSyncCommitteeV1 {
            public_keys: vec![GENERATOR_PUBLIC_KEY.to_vec(); SYNC_COMMITTEE_SIZE],
            aggregate_public_key: GENERATOR_PUBLIC_KEY.to_vec(),
        }
    }

    pub(super) fn canonical_event_data(
        payload_hash: H256,
        route_config_hash: H256,
        payload: &[u8],
    ) -> Vec<u8> {
        let padded_len = payload.len().checked_add(31).unwrap() & !31;
        let mut data = Vec::with_capacity(128 + padded_len);
        data.extend_from_slice(&payload_hash);
        data.extend_from_slice(&route_config_hash);
        data.extend_from_slice(&[0; 31]);
        data.push(96);
        let mut len_word = [0u8; 32];
        len_word[24..].copy_from_slice(&u64::try_from(payload.len()).unwrap().to_be_bytes());
        data.extend_from_slice(&len_word);
        data.extend_from_slice(payload);
        data.resize(128 + padded_len, 0);
        data
    }

    pub(super) fn event_receipt(
        emitter: [u8; 20],
        lane_hash: H256,
        message_id: H256,
        digest: H256,
        payload_hash: H256,
        route_config_hash: H256,
        payload: &[u8],
        status: bool,
        duplicate: bool,
    ) -> Vec<u8> {
        receipt_with_topics(
            emitter,
            &[
                keccak256(ETHEREUM_SOURCE_EVENT_SIGNATURE_V1),
                lane_hash,
                message_id,
                digest,
            ],
            status,
            &canonical_event_data(payload_hash, route_config_hash, payload),
            duplicate,
        )
    }

    pub(super) fn receipt_with_topics(
        emitter: [u8; 20],
        topics: &[H256],
        status: bool,
        data: &[u8],
        duplicate: bool,
    ) -> Vec<u8> {
        let log = encode_list(&[
            encode_bytes(&emitter),
            encode_list(
                &topics
                    .iter()
                    .map(|topic| encode_bytes(topic))
                    .collect::<Vec<_>>(),
            ),
            encode_bytes(data),
        ]);
        let logs = if duplicate {
            encode_list(&[log.clone(), log])
        } else {
            encode_list(&[log])
        };
        let payload = encode_list(&[
            if status {
                encode_bytes(&[1])
            } else {
                encode_bytes(&[])
            },
            encode_bytes(&[0x52, 0x08]),
            encode_bytes(&[0u8; 256]),
            logs,
        ]);
        let mut typed = vec![2];
        typed.extend_from_slice(&payload);
        typed
    }

    pub(super) fn source_fixture_for_statement(
        message_id: H256,
        canonical_payload: &[u8],
    ) -> (
        SccpSourceIdentityV1,
        H256,
        H256,
        EthereumNativeSourceProofV1,
    ) {
        let payload_hash = super::super::payload_hash(canonical_payload);
        let identity = SccpSourceIdentityV1 {
            lane: SccpLaneIdV1 {
                source: SccpNetworkV1::EthereumMainnet,
                target: SccpNetworkV1::SoraTaira,
            },
            emitter: SccpSourceEmitterV1::Evm(SccpEvmSourceEmitterV1 {
                address: [0x11; 20],
                runtime_code_hash: [0x22; 32],
                route_config_hash: [0x33; 32],
            }),
        };
        let identity_hash = sccp_source_identity_hash_v1(&identity).unwrap();
        let lane_hash = sccp_lane_id_hash_v1(identity.lane).unwrap();
        let digest =
            sccp_lane_source_event_digest_v1(identity.lane, message_id, payload_hash).unwrap();

        let storage_root = [0x44; 32];
        let account_value = encode_list(&[
            encode_bytes(&[]),
            encode_bytes(&[]),
            encode_bytes(&storage_root),
            encode_bytes(&[0x22; 32]),
        ]);
        let account_key = keccak256(&[0x11; 20]);
        let (state_root, account_proof) = single_leaf_proof(&account_key, &account_value);
        let receipt = event_receipt(
            [0x11; 20],
            lane_hash,
            message_id,
            digest,
            payload_hash,
            [0x33; 32],
            canonical_payload,
            true,
            false,
        );
        let receipt_key = rlp_encode_u64(0);
        let (receipts_root, receipt_proof) = single_leaf_proof(&receipt_key, &receipt);

        let execution_wire = EthereumNativeCapellaExecutionHeaderV1 {
            parent_hash: [1; 32],
            fee_recipient: vec![2; 20],
            state_root,
            receipts_root,
            logs_bloom: vec![0; 256],
            prev_randao: [3; 32],
            block_number: 17_000_000,
            gas_limit: 30_000_000,
            gas_used: 21_000,
            timestamp: 1_700_000_000,
            extra_data: vec![4, 5],
            base_fee_per_gas: [6; 32],
            block_hash: [7; 32],
            transactions_root: [8; 32],
            withdrawals_root: [9; 32],
        };
        let execution = capella_execution_from_wire(&execution_wire).unwrap();
        let execution_branch = [[0x71; 32], [0x72; 32], [0x73; 32], [0x74; 32]];
        let body_root = merkle_root_from_branch(execution.hash_tree_root(), 25, &execution_branch);

        let committee_wire = committee_wire();
        let committee = sync_committee_from_wire(&committee_wire).unwrap();
        let mut explicit = BTreeMap::new();
        explicit.insert(54, committee.hash_tree_root());
        let beacon_state_root = sparse_node(1, 5, &explicit);
        let current_branch = sparse_branch(54, 5, &explicit);
        let header_wire = EthereumNativeLightClientHeaderV1 {
            fork: EthereumNativeForkV1::Capella,
            beacon: EthereumNativeBeaconHeaderV1 {
                slot: 1,
                proposer_index: 2,
                parent_root: [0x81; 32],
                state_root: beacon_state_root,
                body_root,
            },
            execution: Some(EthereumNativeExecutionHeaderV1::Capella(execution_wire)),
            execution_branch: execution_branch.iter().map(|root| root.to_vec()).collect(),
        };
        let bootstrap_wire = EthereumNativeLightClientBootstrapV1 {
            header: header_wire.clone(),
            current_sync_committee: committee_wire,
            current_sync_committee_branch: current_branch
                .iter()
                .map(|root| root.to_vec())
                .collect(),
        };
        let native_header = light_client_header_from_wire(&header_wire).unwrap();
        let native_state = EthereumLightClientState::from_trusted_anchor(
            schedule_from_wire(&schedule_wire()).unwrap(),
            native_header.beacon().hash_tree_root(),
            bootstrap_from_wire(&bootstrap_wire).unwrap(),
        )
        .unwrap();
        let anchor = EthereumNativeTrustedAnchorV1 {
            version: 1,
            network: SccpNetworkV1::EthereumMainnet,
            fork_schedule: schedule_wire(),
            trusted_beacon_block_root: native_header.beacon().hash_tree_root(),
            bootstrap: bootstrap_wire,
            anchor_state_commitment: native_state.state_commitment(),
        };
        let anchor_hash = ethereum_native_trusted_anchor_hash_v1(&anchor).unwrap();
        let proof = EthereumNativeSourceProofV1 {
            version: 1,
            source_identity: identity,
            source_identity_hash: identity_hash,
            lane_hash,
            trusted_anchor: anchor,
            trusted_anchor_hash: anchor_hash,
            updates: Vec::new(),
            final_state_commitment: native_state.state_commitment(),
            finalized_execution: EthereumNativeFinalizedExecutionV1 {
                fork: EthereumNativeForkV1::Capella,
                beacon_slot: 1,
                beacon_block_root: native_header.beacon().hash_tree_root(),
                block_number: 17_000_000,
                block_hash: [7; 32],
                state_root,
                receipts_root,
            },
            message_id,
            payload_hash,
            source_event_digest: digest,
            account_proof,
            transaction_index: 0,
            receipt_proof,
        };
        (identity, identity_hash, anchor_hash, proof)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use super::test_fixtures::*;
    use super::*;

    fn test_payload() -> &'static [u8] {
        static PAYLOAD: OnceLock<Vec<u8>> = OnceLock::new();
        PAYLOAD
            .get_or_init(|| {
                canonical_sccp_payload_bytes(&SccpPayloadV1::Transfer(crate::TransferPayloadV1 {
                    version: 1,
                    source_domain: crate::SCCP_DOMAIN_ETH,
                    dest_domain: crate::SCCP_DOMAIN_SORA,
                    nonce: 1,
                    route_revision: 1,
                    asset_home_domain: crate::SCCP_DOMAIN_ETH,
                    asset_id_codec: crate::SCCP_CODEC_CANONICAL_TEXT,
                    asset_id: b"xor".to_vec(),
                    amount: 1,
                    sender_codec: crate::SCCP_CODEC_EVM_ADDRESS20,
                    sender: vec![0x11; 20],
                    recipient_codec: crate::SCCP_CODEC_CANONICAL_TEXT,
                    recipient: b"alice@taira".to_vec(),
                    route_id_codec: crate::SCCP_CODEC_CANONICAL_TEXT,
                    route_id: b"ethereum_taira_xor".to_vec(),
                }))
            })
            .as_slice()
    }

    fn test_message_id() -> H256 {
        let payload = decode_canonical_sccp_payload_bytes(test_payload()).unwrap();
        sccp_message_id(
            iroha_data_model::bridge::sccp::SccpLaneIdV1 {
                source: SccpNetworkV1::EthereumMainnet,
                target: SccpNetworkV1::SoraTaira,
            },
            &payload,
        )
        .unwrap()
    }

    #[test]
    fn independently_constructed_single_leaf_mpt_vector_is_canonical() {
        let key = [0x12, 0x34];
        let value = [0xab, 0xcd, 0xef];
        let (root, proof) = single_leaf_proof(&key, &value);
        assert_eq!(
            root,
            [
                0x34, 0x4c, 0xda, 0xee, 0x19, 0x09, 0x3a, 0x13, 0x81, 0x10, 0xb4, 0x1c, 0x8c, 0xd7,
                0x16, 0xeb, 0x26, 0x57, 0x1b, 0xda, 0x75, 0xb7, 0x0b, 0x5c, 0xb7, 0x55, 0x8b, 0x3f,
                0xde, 0x33, 0xa1, 0xf6,
            ]
        );
        assert_eq!(
            verify_mpt_inclusion(root, &key, &proof, EthereumNativeMptRoleV1::Account),
            Ok(value.to_vec())
        );
    }

    #[test]
    fn native_source_proof_roundtrips_and_authenticates_state_and_receipt_tries() {
        let (identity, identity_hash, anchor_hash, _, _, proof) =
            ethereum_native_positive_test_fixture_for_statement(test_message_id(), test_payload());
        let bytes = norito::to_bytes(&proof).unwrap();
        let decoded = decode_ethereum_native_source_proof_v1(&bytes).unwrap();
        assert_eq!(decoded, proof);
        let json = norito::json::to_json(&proof).unwrap();
        let json_decoded = decode_ethereum_native_source_proof_json_v1(&json).unwrap();
        assert_eq!(json_decoded, proof);
        let validated = verify_ethereum_native_source_proof_v1(
            &identity,
            identity_hash,
            anchor_hash,
            proof.message_id,
            proof.payload_hash,
            test_payload(),
            &proof,
        )
        .unwrap();
        assert_eq!(validated.execution_block_hash, [7; 32]);
        assert_eq!(validated.execution_block_number, 17_000_000);
        assert_eq!(validated.transaction_index, 0);
    }

    #[test]
    fn identity_lane_anchor_statement_and_final_state_are_role_bound() {
        let (identity, identity_hash, anchor_hash, proof) =
            source_fixture_for_statement(test_message_id(), test_payload());
        let verify = |proof: &EthereumNativeSourceProofV1| {
            verify_ethereum_native_source_proof_v1(
                &identity,
                identity_hash,
                anchor_hash,
                test_message_id(),
                crate::payload_hash(test_payload()),
                test_payload(),
                proof,
            )
        };

        let mut mutated = proof.clone();
        mutated.source_identity_hash[0] ^= 1;
        assert_eq!(
            verify(&mutated),
            Err(EthereumNativeSourceErrorV1::SourceIdentityHashMismatch)
        );
        let mut mutated = proof.clone();
        mutated.lane_hash[0] ^= 1;
        assert_eq!(
            verify(&mutated),
            Err(EthereumNativeSourceErrorV1::LaneHashMismatch)
        );
        let mut mutated = proof.clone();
        mutated.trusted_anchor_hash[0] ^= 1;
        assert_eq!(
            verify(&mutated),
            Err(EthereumNativeSourceErrorV1::TrustedAnchorHashMismatch)
        );
        let mut mutated = proof.clone();
        mutated.source_event_digest[0] ^= 1;
        assert_eq!(
            verify(&mutated),
            Err(EthereumNativeSourceErrorV1::SourceEventDigestMismatch)
        );
        let mut mutated = proof.clone();
        mutated.final_state_commitment[0] ^= 1;
        assert_eq!(
            verify(&mutated),
            Err(EthereumNativeSourceErrorV1::FinalStateCommitmentMismatch)
        );
        let mut mutated = proof;
        mutated.finalized_execution.receipts_root[0] ^= 1;
        assert_eq!(
            verify(&mutated),
            Err(EthereumNativeSourceErrorV1::FinalizedExecutionMismatch)
        );
    }

    #[test]
    fn pre_capella_finalized_anchor_is_rejected_without_execution_alias() {
        let (identity, identity_hash, _, mut proof) =
            source_fixture_for_statement(test_message_id(), test_payload());
        proof.trusted_anchor.fork_schedule.capella.epoch = u64::MAX;
        let header = &mut proof.trusted_anchor.bootstrap.header;
        header.fork = EthereumNativeForkV1::Bellatrix;
        header.execution = None;
        header.execution_branch.clear();
        let native_header = light_client_header_from_wire(header).unwrap();
        proof.trusted_anchor.trusted_beacon_block_root = native_header.beacon().hash_tree_root();
        let state = EthereumLightClientState::from_trusted_anchor(
            schedule_from_wire(&proof.trusted_anchor.fork_schedule).unwrap(),
            proof.trusted_anchor.trusted_beacon_block_root,
            bootstrap_from_wire(&proof.trusted_anchor.bootstrap).unwrap(),
        )
        .unwrap();
        proof.trusted_anchor.anchor_state_commitment = state.state_commitment();
        proof.final_state_commitment = state.state_commitment();
        proof.trusted_anchor_hash =
            ethereum_native_trusted_anchor_hash_v1(&proof.trusted_anchor).unwrap();

        assert_eq!(
            verify_ethereum_native_source_proof_v1(
                &identity,
                identity_hash,
                proof.trusted_anchor_hash,
                proof.message_id,
                proof.payload_hash,
                test_payload(),
                &proof,
            ),
            Err(EthereumNativeSourceErrorV1::MissingFinalizedExecution)
        );
    }

    #[test]
    fn mpt_rejects_wrong_key_trailing_duplicate_noncanonical_and_role_replay() {
        let key = [0x12, 0x34];
        let (root, proof) = single_leaf_proof(&key, &[0xab]);
        assert_eq!(
            verify_mpt_inclusion(
                root,
                &[0x12, 0x35],
                &proof,
                EthereumNativeMptRoleV1::Account
            ),
            Err(EthereumNativeSourceErrorV1::MptKeyMismatch(
                EthereumNativeMptRoleV1::Account
            ))
        );
        let mut trailing = proof.clone();
        trailing.nodes.push(vec![0xc0]);
        assert_eq!(
            verify_mpt_inclusion(root, &key, &trailing, EthereumNativeMptRoleV1::Account),
            Err(EthereumNativeSourceErrorV1::UnusedMptNodes(
                EthereumNativeMptRoleV1::Account
            ))
        );
        let mut duplicate = proof.clone();
        duplicate.nodes.push(duplicate.nodes[0].clone());
        assert_eq!(
            verify_mpt_inclusion(root, &key, &duplicate, EthereumNativeMptRoleV1::Account),
            Err(EthereumNativeSourceErrorV1::DuplicateMptNode(
                EthereumNativeMptRoleV1::Account
            ))
        );
        let noncanonical = EthereumNativeMptProofV1 {
            nodes: vec![vec![0xc4, 0x81, 0x20, 0x81, 0x01]],
        };
        assert_eq!(
            verify_mpt_inclusion(
                keccak256(&noncanonical.nodes[0]),
                &[],
                &noncanonical,
                EthereumNativeMptRoleV1::Receipt,
            ),
            Err(EthereumNativeSourceErrorV1::NonCanonicalMpt(
                EthereumNativeMptRoleV1::Receipt
            ))
        );
        let bad_compact = EthereumNativeMptProofV1 {
            nodes: vec![encode_list(&[encode_bytes(&[0x21]), encode_bytes(&[1])])],
        };
        assert_eq!(
            verify_mpt_inclusion(
                keccak256(&bad_compact.nodes[0]),
                &[],
                &bad_compact,
                EthereumNativeMptRoleV1::Receipt,
            ),
            Err(EthereumNativeSourceErrorV1::NonCanonicalMpt(
                EthereumNativeMptRoleV1::Receipt
            ))
        );

        // A canonical short child is embedded as raw RLP. Repeating it as an
        // explicit proof element is an unused-node alias; replacing the inline
        // reference by its hash is a non-canonical hash alias.
        let inline_leaf = encode_list(&[encode_bytes(&[0x32]), encode_bytes(&[1])]);
        assert!(inline_leaf.len() < 32);
        let inline_extension = encode_list(&[encode_bytes(&[0x11]), inline_leaf.clone()]);
        let inline_root = keccak256(&inline_extension);
        let inline_proof = EthereumNativeMptProofV1 {
            nodes: vec![inline_extension.clone()],
        };
        assert_eq!(
            verify_mpt_inclusion(
                inline_root,
                &[0x12],
                &inline_proof,
                EthereumNativeMptRoleV1::Account,
            ),
            Ok(vec![1])
        );
        let inline_repeated = EthereumNativeMptProofV1 {
            nodes: vec![inline_extension, inline_leaf.clone()],
        };
        assert_eq!(
            verify_mpt_inclusion(
                inline_root,
                &[0x12],
                &inline_repeated,
                EthereumNativeMptRoleV1::Account,
            ),
            Err(EthereumNativeSourceErrorV1::UnusedMptNodes(
                EthereumNativeMptRoleV1::Account
            ))
        );
        let hashed_extension = encode_list(&[
            encode_bytes(&[0x11]),
            encode_bytes(&keccak256(&inline_leaf)),
        ]);
        let hashed_alias = EthereumNativeMptProofV1 {
            nodes: vec![hashed_extension.clone(), inline_leaf],
        };
        assert_eq!(
            verify_mpt_inclusion(
                keccak256(&hashed_extension),
                &[0x12],
                &hashed_alias,
                EthereumNativeMptRoleV1::Account,
            ),
            Err(EthereumNativeSourceErrorV1::MptNodeReferenceMismatch(
                EthereumNativeMptRoleV1::Account
            ))
        );

        let (identity, identity_hash, anchor_hash, mut source) =
            source_fixture_for_statement(test_message_id(), test_payload());
        core::mem::swap(&mut source.account_proof, &mut source.receipt_proof);
        assert!(matches!(
            verify_ethereum_native_source_proof_v1(
                &identity,
                identity_hash,
                anchor_hash,
                source.message_id,
                source.payload_hash,
                test_payload(),
                &source,
            ),
            Err(EthereumNativeSourceErrorV1::MptNodeReferenceMismatch(
                EthereumNativeMptRoleV1::Account
            ))
        ));
    }

    #[test]
    fn account_opening_rejects_code_and_scalar_aliases() {
        let storage_root = [0x41; 32];
        let account = encode_list(&[
            encode_bytes(&[]),
            encode_bytes(&[]),
            encode_bytes(&storage_root),
            encode_bytes(&[0x22; 32]),
        ]);
        assert_eq!(account_storage_root(&account, [0x22; 32]), Ok(storage_root));
        assert_eq!(
            account_storage_root(&account, [0x23; 32]),
            Err(EthereumNativeSourceErrorV1::RuntimeCodeHashMismatch)
        );
        let leading_zero_nonce = encode_list(&[
            encode_bytes(&[0]),
            encode_bytes(&[]),
            encode_bytes(&storage_root),
            encode_bytes(&[0x22; 32]),
        ]);
        assert_eq!(
            account_storage_root(&leading_zero_nonce, [0x22; 32]),
            Err(EthereumNativeSourceErrorV1::MalformedAccount)
        );
    }

    #[test]
    fn receipt_requires_one_exact_transfer_event_and_canonical_abi_data() {
        let emitter = [0x11; 20];
        let lane = [0x22; 32];
        let message_id = [0x33; 32];
        let digest = [0x44; 32];
        let config_hash = [0x55; 32];
        let payload = test_payload();
        let payload_hash = crate::payload_hash(payload);
        assert_eq!(
            validate_receipt_event(
                &event_receipt(
                    emitter,
                    lane,
                    message_id,
                    digest,
                    payload_hash,
                    config_hash,
                    payload,
                    true,
                    false,
                ),
                emitter,
                lane,
                message_id,
                digest,
                payload_hash,
                config_hash,
                payload,
            ),
            Ok(())
        );
        assert_eq!(
            validate_receipt_event(
                &event_receipt(
                    emitter,
                    lane,
                    message_id,
                    digest,
                    payload_hash,
                    config_hash,
                    payload,
                    false,
                    false,
                ),
                emitter,
                lane,
                message_id,
                digest,
                payload_hash,
                config_hash,
                payload,
            ),
            Err(EthereumNativeSourceErrorV1::FailedReceipt)
        );
        let valid_data = canonical_event_data(payload_hash, config_hash, payload);
        let signature = keccak256(ETHEREUM_SOURCE_EVENT_SIGNATURE_V1);
        for receipt in [
            event_receipt(
                [0x12; 20],
                lane,
                message_id,
                digest,
                payload_hash,
                config_hash,
                payload,
                true,
                false,
            ),
            event_receipt(
                emitter,
                [0x23; 32],
                message_id,
                digest,
                payload_hash,
                config_hash,
                payload,
                true,
                false,
            ),
            event_receipt(
                emitter,
                lane,
                [0x34; 32],
                digest,
                payload_hash,
                config_hash,
                payload,
                true,
                false,
            ),
            event_receipt(
                emitter,
                lane,
                message_id,
                [0x45; 32],
                payload_hash,
                config_hash,
                payload,
                true,
                false,
            ),
            receipt_with_topics(
                emitter,
                &[[0x99; 32], lane, message_id, digest],
                true,
                &valid_data,
                false,
            ),
            receipt_with_topics(
                emitter,
                &[signature, lane, message_id],
                true,
                &valid_data,
                false,
            ),
            receipt_with_topics(
                emitter,
                &[signature, lane, message_id, digest, [0x77; 32]],
                true,
                &valid_data,
                false,
            ),
        ] {
            assert_eq!(
                validate_receipt_event(
                    &receipt,
                    emitter,
                    lane,
                    message_id,
                    digest,
                    payload_hash,
                    config_hash,
                    payload,
                ),
                Err(EthereumNativeSourceErrorV1::SourceEventLogMismatch)
            );
        }
        assert_eq!(
            validate_receipt_event(
                &event_receipt(
                    emitter,
                    lane,
                    message_id,
                    digest,
                    payload_hash,
                    config_hash,
                    payload,
                    true,
                    true,
                ),
                emitter,
                lane,
                message_id,
                digest,
                payload_hash,
                config_hash,
                payload,
            ),
            Err(EthereumNativeSourceErrorV1::MalformedReceipt)
        );
        for mutation in [64usize, 95, 96, valid_data.len() - 1] {
            let mut malformed_data = valid_data.clone();
            malformed_data[mutation] ^= 1;
            let receipt = receipt_with_topics(
                emitter,
                &[signature, lane, message_id, digest],
                true,
                &malformed_data,
                false,
            );
            assert_eq!(
                validate_receipt_event(
                    &receipt,
                    emitter,
                    lane,
                    message_id,
                    digest,
                    payload_hash,
                    config_hash,
                    payload,
                ),
                Err(EthereumNativeSourceErrorV1::SourceEventLogMismatch)
            );
        }
        let mut trailing_data = valid_data.clone();
        trailing_data.extend_from_slice(&[0; 32]);
        let trailing_receipt = receipt_with_topics(
            emitter,
            &[signature, lane, message_id, digest],
            true,
            &trailing_data,
            false,
        );
        assert_eq!(
            validate_receipt_event(
                &trailing_receipt,
                emitter,
                lane,
                message_id,
                digest,
                payload_hash,
                config_hash,
                payload,
            ),
            Err(EthereumNativeSourceErrorV1::SourceEventLogMismatch)
        );
        let old_event = receipt_with_topics(
            emitter,
            &[keccak256(b"SccpSourceEvent(bytes32,bytes32)"), lane, digest],
            true,
            &[],
            false,
        );
        assert_eq!(
            validate_receipt_event(
                &old_event,
                emitter,
                lane,
                message_id,
                digest,
                payload_hash,
                config_hash,
                payload,
            ),
            Err(EthereumNativeSourceErrorV1::SourceEventLogMismatch)
        );
        let mut unknown_type = event_receipt(
            emitter,
            lane,
            message_id,
            digest,
            payload_hash,
            config_hash,
            payload,
            true,
            false,
        );
        unknown_type[0] = 5;
        assert_eq!(
            validate_receipt_event(
                &unknown_type,
                emitter,
                lane,
                message_id,
                digest,
                payload_hash,
                config_hash,
                payload,
            ),
            Err(EthereumNativeSourceErrorV1::MalformedReceipt)
        );
    }

    #[test]
    fn proof_bounds_reject_empty_oversized_and_excess_update_material() {
        assert_eq!(
            validate_mpt_proof_bounds(
                &EthereumNativeMptProofV1 { nodes: Vec::new() },
                EthereumNativeMptRoleV1::Account,
            ),
            Err(EthereumNativeSourceErrorV1::MptProofBounds(
                EthereumNativeMptRoleV1::Account
            ))
        );
        assert_eq!(
            validate_mpt_proof_bounds(
                &EthereumNativeMptProofV1 {
                    nodes: vec![vec![0; MAX_MPT_NODE_BYTES + 1]],
                },
                EthereumNativeMptRoleV1::Receipt,
            ),
            Err(EthereumNativeSourceErrorV1::MptProofBounds(
                EthereumNativeMptRoleV1::Receipt
            ))
        );
        let (_, small_proof) = single_leaf_proof(&[1], &[2]);
        assert_eq!(
            verify_mpt_inclusion(
                [0; 32],
                &[1],
                &small_proof,
                EthereumNativeMptRoleV1::Account,
            ),
            Err(EthereumNativeSourceErrorV1::EmptyTrieRoot(
                EthereumNativeMptRoleV1::Account
            ))
        );
        assert_eq!(
            decode_ethereum_native_source_proof_v1(&vec![0; MAX_ENCODED_SOURCE_PROOF_BYTES + 1]),
            Err(EthereumNativeSourceErrorV1::EncodedProofTooLarge(
                MAX_ENCODED_SOURCE_PROOF_BYTES + 1
            ))
        );

        let (_, _, _, proof) = source_fixture_for_statement(test_message_id(), test_payload());
        let mut compressed_alias = norito::to_bytes(&proof).unwrap();
        compressed_alias[NORITO_COMPRESSION_OFFSET] = 1;
        assert_eq!(
            decode_ethereum_native_source_proof_v1(&compressed_alias),
            Err(EthereumNativeSourceErrorV1::InvalidNoritoEncoding)
        );
        let mut declared_bomb = norito::to_bytes(&proof).unwrap();
        declared_bomb[NORITO_LENGTH_OFFSET..NORITO_LENGTH_OFFSET + 8]
            .copy_from_slice(&(MAX_ENCODED_SOURCE_PROOF_BYTES_U64 + 1).to_le_bytes());
        assert_eq!(
            decode_ethereum_native_source_proof_v1(&declared_bomb),
            Err(EthereumNativeSourceErrorV1::EncodedProofTooLarge(
                MAX_ENCODED_SOURCE_PROOF_BYTES + 1
            ))
        );

        let mut too_many_updates = proof;
        let dummy = EthereumNativeLightClientUpdateV1 {
            attested_header: too_many_updates.trusted_anchor.bootstrap.header.clone(),
            next_sync_committee: too_many_updates
                .trusted_anchor
                .bootstrap
                .current_sync_committee
                .clone(),
            next_sync_committee_branch: Vec::new(),
            finalized_header: too_many_updates.trusted_anchor.bootstrap.header.clone(),
            finality_branch: Vec::new(),
            sync_committee_bits: vec![0; SYNC_COMMITTEE_BITS_BYTES],
            sync_committee_signature: vec![0; 96],
            signature_slot: 0,
        };
        too_many_updates.updates = vec![dummy; ETHEREUM_NATIVE_MAX_LIGHT_CLIENT_UPDATES + 1];
        assert_eq!(
            validate_decoded_source_proof_bounds(&too_many_updates),
            Err(EthereumNativeSourceErrorV1::TooManyLightClientUpdates(
                ETHEREUM_NATIVE_MAX_LIGHT_CLIENT_UPDATES + 1
            ))
        );
    }
}
