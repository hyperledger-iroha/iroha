//! Native TON masterchain finality and source-message verification for SCCP.
//!
//! TON does not sign application-specific bridge statements. Validators sign a
//! native `BlockIdExt`, either directly (ordinary catchain finality) or through
//! the Simplex `consensus.dataToSign` transcript. This module therefore starts
//! at an exact governed masterchain checkpoint, verifies the native signature
//! transcript and validator roster, follows authenticated block references,
//! opens the finalized shard descriptor, and finally parses the concrete
//! account transaction and external-out message. No caller-provided event
//! fields are trusted independently of authenticated TON cells.

use super::{
    H256, SccpPayloadV1, canonical_sccp_payload_bytes, payload_hash, prefixed_blake2b,
    sccp_lane_id_hash_v1, sccp_lane_source_event_digest_v1, sccp_message_id,
    sccp_source_identity_hash_v1, verify_sccp_payload_structure,
};
use alloc::{
    collections::{BTreeMap, BTreeSet},
    vec,
    vec::Vec,
};
use core::fmt;
use iroha_data_model::bridge::{
    SCCP_TON_BASECHAIN_WORKCHAIN_V1, SCCP_TON_MAINNET_GLOBAL_ID_V1,
    SCCP_TON_MAINNET_ZERO_STATE_FILE_HASH_V1, SCCP_TON_MAINNET_ZERO_STATE_ROOT_HASH_V1,
    SCCP_TON_MASTERCHAIN_SHARD_V1, SCCP_TON_MASTERCHAIN_WORKCHAIN_V1, SCCP_TON_ZERO_STATE_SEQNO_V1,
    SCCP_V1_TON_STORAGE_VERSION, SccpDestinationDeploymentV1, SccpGovernedRouteV1,
    SccpGroth16Bls12381IcV1, SccpGroth16Bls12381VerifyingKeyV1, SccpLaneIdV1, SccpNetworkV1,
    SccpRouteKeyV1, SccpSourceEmitterV1, SccpSourceIdentityV1, SccpTonAddressV1,
    SccpTonDestinationDeploymentV1, SccpTonMintBreakerGuardianKeysV1,
    canonical_sccp_lane_id_bytes_v1, sccp_groth16_bls12381_verifying_key_hash_v1,
};
use sha2::{Digest as _, Sha256, Sha512};

const TON_NATIVE_ANCHOR_PREFIX_V1: &[u8] = b"sccp:ton:native-masterchain-anchor:v1";
const TON_BOC_MAGIC: [u8; 4] = [0xb5, 0xee, 0x9c, 0x72];
const TON_BLOCK_CONSTRUCTOR: u32 = 0x11ef_55aa;
const TON_BLOCK_INFO_CONSTRUCTOR: u32 = 0x9bc7_a987;
const TON_SHARD_STATE_CONSTRUCTOR: u32 = 0x9023_afe2;
const TON_SPLIT_STATE_CONSTRUCTOR: u32 = 0x5f32_7da5;
const TON_MC_BLOCK_EXTRA_CONSTRUCTOR: u16 = 0xcca5;
const TON_TRANSACTION_CONSTRUCTOR: u8 = 0x7;
const TON_ACCOUNT_BLOCK_CONSTRUCTOR: u8 = 0x5;
const TON_VALIDATOR_CONSTRUCTOR: u8 = 0x53;
const TON_VALIDATOR_ADDR_CONSTRUCTOR: u8 = 0x73;
const TON_VALIDATORS_CONSTRUCTOR: u8 = 0x11;
const TON_VALIDATORS_EXT_CONSTRUCTOR: u8 = 0x12;
const TON_ED25519_PUBKEY_TLB_CONSTRUCTOR: u32 = 0x8e81_278a;
const TON_CATCHAIN_CONFIG_CONSTRUCTOR: u8 = 0xc1;
const TON_CATCHAIN_CONFIG_NEW_CONSTRUCTOR: u8 = 0xc2;
const TON_CONFIG_CURRENT_VALIDATORS: u32 = 34;
const TON_CONFIG_CATCHAIN: u32 = 28;
const TON_PUB_ED25519_TL_CONSTRUCTOR: u32 = 0x4813_b4c6;
const TON_BLOCK_ID_TL_CONSTRUCTOR: u32 = 0xc50b_6e70;
const TON_BLOCK_ID_EXT_TL_CONSTRUCTOR: u32 = 0x6752_eb78;
const TON_CONSENSUS_DATA_TO_SIGN_TL_CONSTRUCTOR: u32 = 0xa8e3_3df8;
const TON_CONSENSUS_CANDIDATE_ID_TL_CONSTRUCTOR: u32 = 0xb691_cd3f;
const TON_CONSENSUS_CANDIDATE_PARENT_TL_CONSTRUCTOR: u32 = 0x1a4b_9af1;
const TON_CONSENSUS_CANDIDATE_WITHOUT_PARENTS_TL_CONSTRUCTOR: u32 = 0x22cb_cca9;
const TON_CONSENSUS_CANDIDATE_ORDINARY_TL_CONSTRUCTOR: u32 = 0xe8f9_bcdc;
const TON_CONSENSUS_CANDIDATE_EMPTY_TL_CONSTRUCTOR: u32 = 0x72b4_d933;
const TON_CONSENSUS_SIMPLEX_FINALIZE_TL_CONSTRUCTOR: u32 = 0x40a7_e105;
const TON_SCCP_EVENT_OP_V1: u32 = 0x5343_4350;
const TON_SCCP_EVENT_VERSION_V1: u16 = 1;
const TON_MAX_CELL_DATA_BYTES: usize = 128;
const TON_MAX_BOC_BYTES: usize = 64 * 1024;
const TON_MAX_BOC_CELLS: usize = 4_096;
const TON_MAX_REFS: usize = 4;
/// Maximum cell depth admitted by TON's reference cell traits.
const TON_MAX_CELL_DEPTH: u16 = 1_024;
const TON_MAX_VALIDATORS: usize = 1_024;
const TON_MAX_SIGNATURES: usize = 1_024;
const TON_MAX_MASTERCHAIN_BLOCKS: usize = 64;
const TON_MAX_TOTAL_VALIDATOR_WEIGHT: u64 = 1_u64 << 61;
const TON_SHARD_ACCOUNT_KEY_BITS: u16 = 256;
const TON_ACCOUNT_TRANSACTION_KEY_BITS: u16 = 64;
const TON_OUT_MESSAGE_KEY_BITS: u16 = 15;
const TON_CONFIG_KEY_BITS: u16 = 32;
const TON_VALIDATOR_SET_KEY_BITS: u16 = 16;
const TON_PAYLOAD_HEADER_BYTES: usize = 50;
const TON_PAYLOAD_MIDDLE_CHUNK_BYTES: usize = 100;
const TON_MAX_CANONICAL_PAYLOAD_BYTES: usize = 374;
const TON_SCCP_PENDING_OPERATION_CAP_V1: u16 = 1_024;

/// Maximum post-anchor masterchain blocks accepted by one TON proof.
pub const TON_NATIVE_MAX_MASTERCHAIN_BLOCKS_V1: usize = TON_MAX_MASTERCHAIN_BLOCKS;
/// Maximum bytes accepted for any individual proof BoC.
pub const TON_NATIVE_MAX_BOC_BYTES_V1: usize = TON_MAX_BOC_BYTES;

/// Native TON extended block identifier.
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
pub struct TonBlockIdExtV1 {
    /// Signed workchain identifier.
    pub workchain: i32,
    /// TON full-shard identifier.
    #[norito(with = "crate::json_utils::u64_string")]
    pub shard: u64,
    /// Block sequence number.
    pub seqno: u32,
    /// TON representation hash of the block root cell.
    #[norito(with = "crate::json_utils::hex32")]
    pub root_hash: H256,
    /// SHA-256 file hash carried by native block references and signatures.
    #[norito(with = "crate::json_utils::hex32")]
    pub file_hash: H256,
}

/// One validator in the exact native order used for TON set hashing.
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
pub struct TonValidatorV1 {
    /// Raw Ed25519 public key from `ValidatorDescr`.
    #[norito(with = "crate::json_utils::hex32")]
    pub public_key: H256,
    /// Positive native validator weight.
    #[norito(with = "crate::json_utils::u64_string")]
    pub weight: u64,
    /// Raw ADNL address committed by the native validator-list hash.
    #[norito(with = "crate::json_utils::hex32")]
    pub adnl_address: H256,
}

/// Exact active masterchain validator subset at a checkpoint.
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
pub struct TonValidatorSetV1 {
    /// Catchain sequence number used to derive this subset.
    pub catchain_seqno: u32,
    /// Native CRC32C `validator_list_hash_short`.
    pub validator_list_hash_short: u32,
    /// Validators in exact native set order.
    pub validators: Vec<TonValidatorV1>,
}

/// Full config-34 roster retained at a governed checkpoint for the next set transition.
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
pub struct TonValidatorConfigV1 {
    /// Inclusive UNIX activation time encoded by config 34.
    pub valid_since: u32,
    /// Exclusive UNIX end time encoded by config 34.
    pub valid_until: u32,
    /// Number of leading validators eligible for the masterchain subset.
    pub main_validator_count: u16,
    /// Config-28 masterchain shuffle flag.
    pub shuffle_masterchain_validators: bool,
    /// Complete config-34 roster in dictionary-index order.
    pub validators: Vec<TonValidatorV1>,
}

/// Exact governed native TON checkpoint.
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
pub struct TonNativeAnchorV1 {
    /// Anchor schema version. V1 accepts exactly `1`.
    pub version: u8,
    /// Exact TON network profile.
    pub network: SccpNetworkV1,
    /// Profile zero-state identity copied into governance state.
    pub zero_state: TonBlockIdExtV1,
    /// Finalized masterchain checkpoint from which proof replay begins.
    pub checkpoint: TonBlockIdExtV1,
    /// Post-state root of the checkpoint.
    #[norito(with = "crate::json_utils::hex32")]
    pub checkpoint_state_root: H256,
    /// Active subset capable of signing the first continuation block.
    pub active_validator_set: TonValidatorSetV1,
    /// Latest authenticated config available for a later catchain transition.
    pub pending_validator_config: Option<TonValidatorConfigV1>,
}

/// One validator signature over a native TON finality transcript.
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
pub struct TonValidatorSignatureV1 {
    /// SHA-256 short id of boxed TL `pub.ed25519`.
    #[norito(with = "crate::json_utils::hex32")]
    pub node_id_short: H256,
    /// Canonical 64-byte Ed25519 signature.
    #[norito(with = "crate::json_utils::bytes_hex")]
    pub signature: Vec<u8>,
}

/// Ordinary catchain signatures over boxed TL `ton.blockId`.
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
pub struct TonOrdinaryBlockSignaturesV1 {
    /// Native catchain sequence number.
    pub catchain_seqno: u32,
    /// Native validator-list hash.
    pub validator_list_hash_short: u32,
    /// Unique native signatures.
    pub signatures: Vec<TonValidatorSignatureV1>,
}

/// Simplex final signatures over the official nested TL transcript.
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
pub struct TonSimplexBlockSignaturesV1 {
    /// Native catchain sequence number.
    pub catchain_seqno: u32,
    /// Native validator-list hash.
    pub validator_list_hash_short: u32,
    /// Simplex session identifier.
    #[norito(with = "crate::json_utils::hex32")]
    pub session_id: H256,
    /// Simplex slot in this session.
    pub slot: u32,
    /// Exact boxed TL `consensus.CandidateHashData` bytes.
    #[norito(with = "crate::json_utils::bytes_hex")]
    pub candidate_data: Vec<u8>,
    /// Unique native final signatures.
    pub signatures: Vec<TonValidatorSignatureV1>,
}

/// Closed native TON block-signature transcript.
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
#[norito(tag = "finality", content = "signatures", rename_all = "snake_case")]
pub enum TonBlockSignaturesV1 {
    /// Ordinary catchain final signatures.
    Ordinary(TonOrdinaryBlockSignaturesV1),
    /// Simplex finalize-vote signatures.
    Simplex(TonSimplexBlockSignaturesV1),
}

/// One authenticated post-anchor masterchain block.
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
pub struct TonMasterchainBlockProofV1 {
    /// Native block identifier signed by validators.
    pub block_id: TonBlockIdExtV1,
    /// Complete or Merkle-pruned BoC rooted at `block_id.root_hash`.
    #[norito(with = "crate::json_utils::bytes_hex")]
    pub block_proof_boc: Vec<u8>,
    /// Native final signatures for this exact `BlockIdExt`.
    pub signatures: TonBlockSignaturesV1,
}

/// Native masterchain continuation from a governed checkpoint.
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
pub struct TonNativeFinalityProofV1 {
    /// Proof schema version. V1 accepts exactly `1`.
    pub version: u8,
    /// Full governed checkpoint preimage.
    pub anchor: TonNativeAnchorV1,
    /// Consecutive masterchain blocks after the checkpoint.
    pub blocks: Vec<TonMasterchainBlockProofV1>,
}

/// Authenticated shard transaction and source-message opening.
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
pub struct TonShardEventProofV1 {
    /// Shard block selected by the finalized masterchain `ShardHashes` tree.
    pub shard_block_id: TonBlockIdExtV1,
    /// Complete or Merkle-pruned shard-block BoC.
    #[norito(with = "crate::json_utils::bytes_hex")]
    pub shard_block_proof_boc: Vec<u8>,
    /// Merkle proof rooted at the selected transaction's pre-state `Account` hash.
    ///
    /// This binds the governed code and route configuration to the code that
    /// executed the event transaction. The shard post-state alone is
    /// insufficient because another transaction can restore governed state.
    #[norito(with = "crate::json_utils::bytes_hex")]
    pub transaction_pre_state_proof_boc: Vec<u8>,
    /// Merkle proof rooted at the shard block's post-state.
    #[norito(with = "crate::json_utils::bytes_hex")]
    pub shard_state_proof_boc: Vec<u8>,
    /// Exact logical time key of the source transaction.
    #[norito(with = "crate::json_utils::u64_string")]
    pub transaction_lt: u64,
    /// Exact 15-bit outbound-message dictionary key.
    pub outbound_message_index: u16,
}

/// Complete typed TON native SCCP source proof.
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
pub struct TonNativeSourceProofV1 {
    /// Source-proof schema version. V1 accepts exactly `1`.
    pub version: u8,
    /// Native masterchain finality proof.
    pub finality: TonNativeFinalityProofV1,
    /// Authenticated shard event opening.
    pub event: TonShardEventProofV1,
}

/// One TON account-state opening selected by a finalized masterchain head.
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
pub struct TonAccountStateOpeningV1 {
    /// Shard block selected from the finalized masterchain `ShardHashes` tree.
    pub shard_block_id: TonBlockIdExtV1,
    /// Canonical complete or Merkle-pruned shard-block BoC.
    #[norito(with = "crate::json_utils::bytes_hex")]
    pub shard_block_proof_boc: Vec<u8>,
    /// Canonical account opening rooted at the shard block's post-state.
    #[norito(with = "crate::json_utils::bytes_hex")]
    pub shard_state_proof_boc: Vec<u8>,
}

/// Canonical dual-account proof for one TON mint-breaker observation.
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
pub struct SccpTonBreakerObservationProofV1 {
    /// Proof schema version. Final V1 accepts exactly `1`.
    pub version: u8,
    /// Exact governed route revision to which the observation applies.
    pub route_key: SccpRouteKeyV1,
    /// One authenticated TON-mainnet masterchain continuation.
    pub finality: TonNativeFinalityProofV1,
    /// Authenticated state of the governed SCCP route account.
    pub route_account: TonAccountStateOpeningV1,
    /// Authenticated state of the governed Jetton-master account.
    pub jetton_master_account: TonAccountStateOpeningV1,
}

/// Authenticated identity of one active TON account at one shard block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TonAccountStateReadbackV1 {
    /// Exact opened account address.
    pub address: SccpTonAddressV1,
    /// Finalized shard block containing this post-state.
    pub shard_block_id: TonBlockIdExtV1,
    /// Masterchain sequence recorded by the authenticated `ShardDescr`.
    pub registered_masterchain_seqno: u32,
    /// Shard post-state root opened by the proof.
    pub shard_state_root_hash: H256,
    /// Representation hash of the complete active `Account` cell.
    pub account_state_hash: H256,
    /// Representation hash of the active account code cell.
    pub code_hash: H256,
    /// Representation hash of the active account data cell.
    pub data_hash: H256,
    /// Last transaction hash recorded by the enclosing `ShardAccount`.
    pub last_transaction_hash: H256,
    /// Last transaction logical time recorded by the enclosing `ShardAccount`.
    pub last_transaction_lt: u64,
    /// Account-storage logical time authenticated inside the active account.
    pub storage_last_transaction_lt: u64,
}

/// Authenticated summary of one exact TON replay forest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TonReplayForestReadbackV1 {
    /// Representation hash of the nonempty-shard dictionary root, when present.
    pub nonempty_shard_roots_hash: Option<H256>,
    /// Number of occupied replay leaves.
    pub leaf_count: u64,
    /// Monotonic forest update sequence; final V1 requires equality with `leaf_count`.
    pub update_sequence: u64,
}

/// Authenticated summary of one optional TON dictionary and its explicit count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TonPendingMapReadbackV1 {
    /// Representation hash of the dictionary root, when nonempty.
    pub dictionary_root_hash: Option<H256>,
    /// Explicit operation count stored beside the dictionary.
    pub count: u16,
}

/// Complete immutable TON bridge configuration decoded from account storage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TonDeploymentReadbackV1 {
    /// Mainnet global id authenticated by the stored configuration.
    pub expected_global_id: i32,
    /// Exact governed route revision.
    pub route_revision: u32,
    /// Fixed Taira-to-TON base-unit multiplier.
    pub taira_to_ton_multiplier: u64,
    /// Positive immutable wrapped-supply cap.
    pub max_wrapped_supply: u128,
    /// Canonical TON-to-SORA lane bytes.
    pub source_lane_bytes: Vec<u8>,
    /// Canonical SORA-to-TON lane bytes.
    pub destination_lane_bytes: Vec<u8>,
    /// Hash of `source_lane_bytes` committed by the route.
    pub source_lane_hash: H256,
    /// Hash of `destination_lane_bytes` committed by the route.
    pub destination_lane_hash: H256,
    /// Exact concrete route-configuration hash.
    pub route_configuration_hash: H256,
    /// Exact destination-deployment binding hash.
    pub destination_binding_hash: H256,
    /// Hash of the governed semantic proof profile.
    pub semantic_proof_profile_hash: H256,
    /// Jetton-master code identity.
    pub jetton_master_code_hash: H256,
    /// Canonically reconstructed initial Jetton-master data identity.
    pub jetton_master_initial_data_hash: H256,
    /// Jetton-wallet code identity.
    pub jetton_wallet_code_hash: H256,
    /// SCCP route code identity.
    pub route_code_hash: H256,
    /// Canonically reconstructed initial SCCP-route data identity.
    pub route_initial_data_hash: H256,
    /// Governed SORA finality-anchor hash.
    pub sora_finality_anchor_hash: H256,
    /// Governed Groth16 circuit commitment.
    pub verifier_circuit_hash: H256,
    /// Hash of the canonical BLS12-381 verifying-key bytes.
    pub verifying_key_hash: H256,
    /// Exact proof-profile commitment consumed by the linked verifier.
    pub proof_profile_commitment: H256,
    /// Exactly five nonzero, strictly increasing breaker guardian keys.
    pub mint_breaker_guardian_keys: SccpTonMintBreakerGuardianKeysV1,
    /// Representation hash of the linked verifier code.
    pub embedded_verifier_code_hash: H256,
    /// Representation hash of the exact typed verifying-key cell.
    pub verifying_key_cell_hash: H256,
    /// Fully decoded typed verifying key.
    pub verifying_key: SccpGroth16Bls12381VerifyingKeyV1,
    /// Representation hash of the immutable master metadata cell.
    pub master_metadata_hash: H256,
    /// Representation hash of the complete shared bridge-configuration cell.
    pub bridge_config_cell_hash: H256,
}

/// Mutable SCCP route state authenticated by a breaker observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TonRouteStorageReadbackV1 {
    /// Stored configuration hash.
    pub route_configuration_hash: H256,
    /// Stored hash of the referenced bridge-configuration cell.
    pub bridge_config_cell_hash: H256,
    /// Inbound-mint replay forest.
    pub inbound_mint_replay: TonReplayForestReadbackV1,
    /// Outbound-burn replay forest.
    pub outbound_burn_replay: TonReplayForestReadbackV1,
    /// Pending route-to-master mints.
    pub pending_mints: TonPendingMapReadbackV1,
    /// Pending wallet-to-route burns.
    pub pending_burns: TonPendingMapReadbackV1,
    /// One-way route breaker flag.
    pub minting_disabled: bool,
}

/// Mutable Jetton-master state authenticated by a breaker observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TonMasterStorageReadbackV1 {
    /// Stored configuration hash.
    pub route_configuration_hash: H256,
    /// Stored hash of the referenced bridge-configuration cell.
    pub bridge_config_cell_hash: H256,
    /// Current Jetton total supply.
    pub total_supply: u128,
    /// Representation hash of the immutable TEP-64 metadata cell.
    pub metadata_hash: H256,
    /// Reciprocal governed SCCP route address.
    pub bridge_address: SccpTonAddressV1,
    /// Master mint replay forest.
    pub mint_replay: TonReplayForestReadbackV1,
    /// Master burn replay forest.
    pub burn_replay: TonReplayForestReadbackV1,
    /// Pending master-to-wallet mints.
    pub pending_mints: TonPendingMapReadbackV1,
    /// One-way master breaker flag.
    pub minting_disabled: bool,
}

/// Fully authenticated, normalized TON breaker observation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedSccpTonBreakerObservationV1 {
    /// Exact governed route key.
    pub route_key: SccpRouteKeyV1,
    /// Finalized TON-mainnet masterchain block.
    pub masterchain_block_id: TonBlockIdExtV1,
    /// Authenticated masterchain generation time in UNIX seconds.
    pub masterchain_gen_utime: u32,
    /// Governed route account identity and state hashes.
    pub route_account: TonAccountStateReadbackV1,
    /// Governed Jetton-master account identity and state hashes.
    pub jetton_master_account: TonAccountStateReadbackV1,
    /// Shared immutable deployment decoded independently from both accounts.
    pub deployment: TonDeploymentReadbackV1,
    /// Mutable SCCP route storage.
    pub route_storage: TonRouteStorageReadbackV1,
    /// Mutable Jetton-master storage.
    pub master_storage: TonMasterStorageReadbackV1,
    /// `route_storage.minting_disabled || master_storage.minting_disabled`.
    pub effective_disabled: bool,
    /// SHA-256 of canonical Norito bytes for the complete proof object.
    pub canonical_proof_sha256: H256,
    /// Byte length of those canonical proof bytes.
    pub canonical_proof_byte_len: u32,
}

/// Cheap deterministic reservation for native TON source verification.
///
/// The estimate reads only typed vector and byte lengths. It performs no BoC
/// parsing, hashing, public-key validation, or Ed25519 verification, so Core
/// can reserve consensus work before dispatching attacker-controlled cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TonNativeFinalityWorkEstimateV1 {
    /// Number of consecutive post-anchor masterchain blocks.
    pub continuation_blocks: u16,
    /// Aggregate bytes in the bounded BoCs covered by this estimate.
    pub framed_boc_bytes: u32,
    /// Exact number of Ed25519 signatures supplied for verification.
    pub ed25519_signature_checks: u32,
    /// Conservative upper bound on Ed25519 validator-key validations.
    pub validator_key_checks_upper_bound: u32,
}

/// Normalized result of complete TON source verification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValidatedTonNativeSourceV1 {
    /// Governed source-identity hash.
    pub source_identity_hash: H256,
    /// Exact-lane hash.
    pub lane_hash: H256,
    /// Governed native anchor hash.
    pub anchor_hash: H256,
    /// Finalized masterchain sequence number.
    pub masterchain_seqno: u32,
    /// Finalized masterchain root hash.
    pub masterchain_block_hash: H256,
    /// Shard sequence number containing the event.
    pub shard_seqno: u32,
    /// Shard block root hash containing the event.
    pub shard_block_hash: H256,
    /// Authenticated transaction representation hash.
    pub transaction_hash: H256,
    /// Authenticated transaction logical time.
    pub transaction_lt: u64,
    /// Authenticated external-out message representation hash.
    pub outbound_message_hash: H256,
    /// Canonical message identifier authenticated by the event body.
    pub message_id: H256,
    /// Canonical payload hash authenticated by the event body.
    pub payload_hash: H256,
    /// Canonical source-event digest authenticated by the event body.
    pub source_event_digest: H256,
}

/// Fail-closed native TON verification error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TonNativeSourceError {
    /// A V1 version field was not exactly one.
    UnsupportedVersion,
    /// The exact network or zero-state profile was wrong.
    WrongNetwork,
    /// Governed source identity was malformed or not TON.
    InvalidSourceIdentity,
    /// Governed source-identity commitment did not match.
    SourceIdentityHashMismatch,
    /// Governed native checkpoint was malformed.
    InvalidAnchor,
    /// Governed native checkpoint commitment did not match.
    AnchorHashMismatch,
    /// Proof framing exceeded a deterministic resource cap.
    ResourceLimit,
    /// A BoC was malformed, noncanonical, unsupported, or not rooted as claimed.
    InvalidBoc,
    /// A masterchain block did not extend the authenticated checkpoint.
    BrokenMasterchainLink,
    /// The active validator roster or its native hash was invalid.
    InvalidValidatorSet,
    /// A key-block validator transition was absent or unauthenticated.
    InvalidValidatorTransition,
    /// Native final signatures were malformed, duplicated, unknown, or below quorum.
    InvalidSignatures,
    /// Simplex candidate data or its official transcript was malformed or selected another block.
    InvalidSimplexTranscript,
    /// The finalized masterchain block did not authenticate the claimed shard block.
    ShardNotFinalized,
    /// Shard block/state/account identity did not match the governed emitter.
    InvalidShardState,
    /// Governed source bridge code or persistent route commitment was not authenticated.
    SourceDeploymentMismatch,
    /// The selected account block or transaction was absent or malformed.
    InvalidTransaction,
    /// Transaction compute/action phases did not complete successfully.
    UnsuccessfulTransaction,
    /// The selected outbound message was absent, bounced, or not emitted by the source bridge.
    InvalidOutboundMessage,
    /// Authenticated SCCP body did not match the exact lane/message/payload statement.
    EventStatementMismatch,
    /// A TON breaker proof or its dual-account framing was malformed.
    InvalidBreakerObservation,
    /// Authenticated route/master storage did not match exact governed deployment state.
    BreakerDeploymentMismatch,
}

impl fmt::Display for TonNativeSourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedVersion => "unsupported native TON proof version",
            Self::WrongNetwork => "native TON network identity mismatch",
            Self::InvalidSourceIdentity => "invalid governed TON source identity",
            Self::SourceIdentityHashMismatch => "TON source identity hash mismatch",
            Self::InvalidAnchor => "invalid governed TON checkpoint",
            Self::AnchorHashMismatch => "TON checkpoint hash mismatch",
            Self::ResourceLimit => "native TON proof exceeds a resource limit",
            Self::InvalidBoc => "invalid or unsupported TON BoC",
            Self::BrokenMasterchainLink => "broken TON masterchain continuation",
            Self::InvalidValidatorSet => "invalid TON validator set",
            Self::InvalidValidatorTransition => "unauthenticated TON validator-set transition",
            Self::InvalidSignatures => "invalid TON finality signatures",
            Self::InvalidSimplexTranscript => "invalid TON Simplex finality transcript",
            Self::ShardNotFinalized => "TON shard block is not finalized by the masterchain",
            Self::InvalidShardState => "invalid TON shard state or source account",
            Self::SourceDeploymentMismatch => "TON source deployment commitment mismatch",
            Self::InvalidTransaction => "invalid TON source transaction",
            Self::UnsuccessfulTransaction => "TON source transaction did not succeed",
            Self::InvalidOutboundMessage => "invalid TON source outbound message",
            Self::EventStatementMismatch => "TON SCCP event statement mismatch",
            Self::InvalidBreakerObservation => "invalid TON breaker observation proof",
            Self::BreakerDeploymentMismatch => {
                "TON breaker observation does not match governed deployment"
            }
        })
    }
}

impl std::error::Error for TonNativeSourceError {}

fn nonzero(hash: &H256) -> bool {
    hash.iter().any(|byte| *byte != 0)
}

fn ton_network_global_id(network: SccpNetworkV1) -> Option<i32> {
    match network {
        SccpNetworkV1::TonMainnet => Some(SCCP_TON_MAINNET_GLOBAL_ID_V1),
        _ => None,
    }
}

fn ton_network_tag(network: SccpNetworkV1) -> Option<u8> {
    match network {
        SccpNetworkV1::TonMainnet => Some(0x44),
        _ => None,
    }
}

fn ton_expected_zero_state(network: SccpNetworkV1) -> Option<TonBlockIdExtV1> {
    let (root_hash, file_hash) = match network {
        SccpNetworkV1::TonMainnet => (
            SCCP_TON_MAINNET_ZERO_STATE_ROOT_HASH_V1,
            SCCP_TON_MAINNET_ZERO_STATE_FILE_HASH_V1,
        ),
        _ => return None,
    };
    Some(TonBlockIdExtV1 {
        workchain: SCCP_TON_MASTERCHAIN_WORKCHAIN_V1,
        shard: SCCP_TON_MASTERCHAIN_SHARD_V1,
        seqno: SCCP_TON_ZERO_STATE_SEQNO_V1,
        root_hash,
        file_hash,
    })
}

fn valid_block_id(block: TonBlockIdExtV1) -> bool {
    block.seqno != 0 && nonzero(&block.root_hash) && nonzero(&block.file_hash)
}

fn push_i32_le(out: &mut Vec<u8>, value: i32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_u16_le(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_u32_le(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_u64_le(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
std::thread_local! {
    static TON_ROSTER_KEY_PARSE_COUNT: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

fn parse_ton_validator_public_key(public_key: &H256) -> Option<()> {
    #[cfg(test)]
    TON_ROSTER_KEY_PARSE_COUNT.with(|count| count.set(count.get().saturating_add(1)));
    iroha_crypto::ed25519_parse_public_key(public_key).ok()?;
    Some(())
}

/// Return the native short node id of one strict Ed25519 validator key.
pub fn ton_validator_node_id_short_v1(public_key: &H256) -> Option<H256> {
    parse_ton_validator_public_key(public_key)?;
    Some(ton_validator_node_id_short_from_validated(public_key))
}

fn ton_validator_node_id_short_from_validated(public_key: &H256) -> H256 {
    let mut boxed = Vec::with_capacity(36);
    push_u32_le(&mut boxed, TON_PUB_ED25519_TL_CONSTRUCTOR);
    boxed.extend_from_slice(public_key);
    Sha256::digest(&boxed).into()
}

/// Reproduce TON's native CRC32C validator-list hash exactly.
pub fn ton_validator_list_hash_short_v1(
    catchain_seqno: u32,
    validators: &[TonValidatorV1],
) -> Option<u32> {
    validate_validator_roster(validators)?;
    ton_validator_list_hash_short_from_validated(catchain_seqno, validators)
}

fn ton_validator_list_hash_short_from_validated(
    catchain_seqno: u32,
    validators: &[TonValidatorV1],
) -> Option<u32> {
    let mut bytes = Vec::with_capacity(12usize.checked_add(validators.len().checked_mul(72)?)?);
    push_i32_le(&mut bytes, -1_877_581_587);
    push_u32_le(&mut bytes, catchain_seqno);
    push_u32_le(&mut bytes, u32::try_from(validators.len()).ok()?);
    for validator in validators {
        bytes.extend_from_slice(&validator.public_key);
        push_u64_le(&mut bytes, validator.weight);
        bytes.extend_from_slice(&validator.adnl_address);
    }
    Some(ton_crc32c(&bytes))
}

fn validate_validator_roster(validators: &[TonValidatorV1]) -> Option<u64> {
    if validators.is_empty() || validators.len() > TON_MAX_VALIDATORS {
        return None;
    }
    let mut keys = BTreeSet::new();
    let mut node_ids = BTreeSet::new();
    let mut adnl = BTreeSet::new();
    let mut total = 0_u64;
    for validator in validators {
        if validator.weight == 0 || !keys.insert(validator.public_key) {
            return None;
        }
        // `validator#53` has no ADNL field and the reference implementation
        // hashes an all-zero address for it. Nonzero ADNL identities, when
        // present, must still be unique.
        if nonzero(&validator.adnl_address) && !adnl.insert(validator.adnl_address) {
            return None;
        }
        parse_ton_validator_public_key(&validator.public_key)?;
        let node_id = ton_validator_node_id_short_from_validated(&validator.public_key);
        if !node_ids.insert(node_id) {
            return None;
        }
        total = total.checked_add(validator.weight)?;
    }
    (total != 0 && total <= TON_MAX_TOTAL_VALIDATOR_WEIGHT).then_some(total)
}

fn validate_active_validator_set(set: &TonValidatorSetV1) -> Option<u64> {
    let total = validate_validator_roster(&set.validators)?;
    (ton_validator_list_hash_short_from_validated(set.catchain_seqno, &set.validators)?
        == set.validator_list_hash_short)
        .then_some(total)
}

fn validate_validator_config(config: &TonValidatorConfigV1) -> Option<()> {
    if config.valid_since >= config.valid_until
        || config.main_validator_count == 0
        || usize::from(config.main_validator_count) > config.validators.len()
    {
        return None;
    }
    validate_validator_roster(&config.validators)?;
    Some(())
}

/// Canonical bytes committed by the governed TON native anchor hash.
pub fn canonical_ton_native_anchor_bytes_v1(anchor: &TonNativeAnchorV1) -> Option<Vec<u8>> {
    validate_ton_native_anchor(anchor)?;
    canonical_ton_native_anchor_bytes_from_validated(anchor)
}

fn canonical_ton_native_anchor_bytes_from_validated(anchor: &TonNativeAnchorV1) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    out.push(anchor.version);
    out.push(ton_network_tag(anchor.network)?);
    push_block_id_canonical(&mut out, anchor.zero_state);
    push_block_id_canonical(&mut out, anchor.checkpoint);
    out.extend_from_slice(&anchor.checkpoint_state_root);
    push_validator_set_canonical_from_validated(&mut out, &anchor.active_validator_set)?;
    match &anchor.pending_validator_config {
        None => out.push(0),
        Some(config) => {
            out.push(1);
            push_validator_config_canonical_from_validated(&mut out, config)?;
        }
    }
    Some(out)
}

fn push_block_id_canonical(out: &mut Vec<u8>, block: TonBlockIdExtV1) {
    push_i32_le(out, block.workchain);
    push_u64_le(out, block.shard);
    push_u32_le(out, block.seqno);
    out.extend_from_slice(&block.root_hash);
    out.extend_from_slice(&block.file_hash);
}

fn push_validator_canonical(out: &mut Vec<u8>, validator: &TonValidatorV1) {
    out.extend_from_slice(&validator.public_key);
    push_u64_le(out, validator.weight);
    out.extend_from_slice(&validator.adnl_address);
}

fn push_validator_set_canonical_from_validated(
    out: &mut Vec<u8>,
    set: &TonValidatorSetV1,
) -> Option<()> {
    push_u32_le(out, set.catchain_seqno);
    push_u32_le(out, set.validator_list_hash_short);
    push_u32_le(out, u32::try_from(set.validators.len()).ok()?);
    for validator in &set.validators {
        push_validator_canonical(out, validator);
    }
    Some(())
}

fn push_validator_config_canonical_from_validated(
    out: &mut Vec<u8>,
    config: &TonValidatorConfigV1,
) -> Option<()> {
    push_u32_le(out, config.valid_since);
    push_u32_le(out, config.valid_until);
    push_u16_le(out, config.main_validator_count);
    out.push(u8::from(config.shuffle_masterchain_validators));
    push_u32_le(out, u32::try_from(config.validators.len()).ok()?);
    for validator in &config.validators {
        push_validator_canonical(out, validator);
    }
    Some(())
}

fn validate_ton_native_anchor(anchor: &TonNativeAnchorV1) -> Option<()> {
    if anchor.version != 1
        || anchor.zero_state != ton_expected_zero_state(anchor.network)?
        || anchor.checkpoint.workchain != SCCP_TON_MASTERCHAIN_WORKCHAIN_V1
        || anchor.checkpoint.shard != SCCP_TON_MASTERCHAIN_SHARD_V1
        || !valid_block_id(anchor.checkpoint)
        || !nonzero(&anchor.checkpoint_state_root)
    {
        return None;
    }
    validate_active_validator_set(&anchor.active_validator_set)?;
    if let Some(config) = &anchor.pending_validator_config {
        validate_validator_config(config)?;
    }
    Some(())
}

/// Hash one valid governed TON native checkpoint.
pub fn ton_native_anchor_hash_v1(anchor: &TonNativeAnchorV1) -> Option<H256> {
    validate_ton_native_anchor(anchor)?;
    ton_native_anchor_hash_from_validated(anchor)
}

fn ton_native_anchor_hash_from_validated(anchor: &TonNativeAnchorV1) -> Option<H256> {
    Some(prefixed_blake2b(
        TON_NATIVE_ANCHOR_PREFIX_V1,
        &canonical_ton_native_anchor_bytes_from_validated(anchor)?,
    ))
}

fn ton_block_id_tl_bytes(block: TonBlockIdExtV1) -> Vec<u8> {
    let mut out = Vec::with_capacity(68);
    push_u32_le(&mut out, TON_BLOCK_ID_TL_CONSTRUCTOR);
    out.extend_from_slice(&block.root_hash);
    out.extend_from_slice(&block.file_hash);
    out
}

/// Serialize one boxed TL `tonNode.blockIdExt` exactly.
pub fn ton_block_id_ext_tl_bytes_v1(block: TonBlockIdExtV1) -> Vec<u8> {
    let mut out = Vec::with_capacity(84);
    push_u32_le(&mut out, TON_BLOCK_ID_EXT_TL_CONSTRUCTOR);
    push_i32_le(&mut out, block.workchain);
    push_u64_le(&mut out, block.shard);
    push_u32_le(&mut out, block.seqno);
    out.extend_from_slice(&block.root_hash);
    out.extend_from_slice(&block.file_hash);
    out
}

fn push_tl_bytes(out: &mut Vec<u8>, bytes: &[u8]) -> Option<()> {
    let len = bytes.len();
    if len < 254 {
        out.push(u8::try_from(len).ok()?);
        out.extend_from_slice(bytes);
        while out.len() % 4 != 0 {
            out.push(0);
        }
        return Some(());
    }
    if len > 0x00ff_ffff {
        return None;
    }
    out.push(254);
    out.push(u8::try_from(len & 0xff).ok()?);
    out.push(u8::try_from((len >> 8) & 0xff).ok()?);
    out.push(u8::try_from((len >> 16) & 0xff).ok()?);
    out.extend_from_slice(bytes);
    while out.len() % 4 != 0 {
        out.push(0);
    }
    Some(())
}

struct TlCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> TlCursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take<const N: usize>(&mut self) -> Option<[u8; N]> {
        let end = self.offset.checked_add(N)?;
        let value = self.bytes.get(self.offset..end)?.try_into().ok()?;
        self.offset = end;
        Some(value)
    }

    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take()?))
    }

    fn i32(&mut self) -> Option<i32> {
        Some(i32::from_le_bytes(self.take()?))
    }

    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take()?))
    }

    fn h256(&mut self) -> Option<H256> {
        self.take()
    }

    fn exhausted(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

fn parse_tl_block_id_ext_bare(cursor: &mut TlCursor<'_>) -> Option<TonBlockIdExtV1> {
    Some(TonBlockIdExtV1 {
        workchain: cursor.i32()?,
        shard: cursor.u64()?,
        seqno: cursor.u32()?,
        root_hash: cursor.h256()?,
        file_hash: cursor.h256()?,
    })
}

fn parse_tl_candidate_id(cursor: &mut TlCursor<'_>) -> Option<()> {
    (cursor.u32()? == TON_CONSENSUS_CANDIDATE_ID_TL_CONSTRUCTOR).then_some(())?;
    (cursor.i32()? >= 0).then_some(())?;
    cursor.h256()?;
    Some(())
}

fn parse_simplex_candidate_data(bytes: &[u8]) -> Option<TonBlockIdExtV1> {
    let mut cursor = TlCursor::new(bytes);
    let constructor = cursor.u32()?;
    // The schema spells this field `block:tonNode.blockIdExt`; lower-case
    // constructor names are bare in TL, so the nested block carries no
    // `tonNode.blockIdExt` constructor id.
    let block = parse_tl_block_id_ext_bare(&mut cursor)?;
    match constructor {
        TON_CONSENSUS_CANDIDATE_ORDINARY_TL_CONSTRUCTOR => {
            cursor.h256()?;
            match cursor.u32()? {
                TON_CONSENSUS_CANDIDATE_PARENT_TL_CONSTRUCTOR => {
                    parse_tl_candidate_id(&mut cursor)?;
                }
                TON_CONSENSUS_CANDIDATE_WITHOUT_PARENTS_TL_CONSTRUCTOR => {}
                _ => return None,
            }
        }
        TON_CONSENSUS_CANDIDATE_EMPTY_TL_CONSTRUCTOR => {
            // `parent:consensus.candidateId` is likewise a bare field.
            (cursor.i32()? >= 0).then_some(())?;
            cursor.h256()?;
        }
        _ => return None,
    }
    cursor.exhausted().then_some(block)
}

fn simplex_finality_transcript(
    block: TonBlockIdExtV1,
    signatures: &TonSimplexBlockSignaturesV1,
) -> Option<Vec<u8>> {
    if signatures.slot > i32::MAX.cast_unsigned()
        || signatures.candidate_data.is_empty()
        || signatures.candidate_data.len() > 4 * 1024
        || parse_simplex_candidate_data(&signatures.candidate_data)? != block
        || !nonzero(&signatures.session_id)
    {
        return None;
    }
    let candidate_hash: H256 = Sha256::digest(&signatures.candidate_data).into();
    let mut candidate_id = Vec::with_capacity(40);
    push_u32_le(&mut candidate_id, TON_CONSENSUS_CANDIDATE_ID_TL_CONSTRUCTOR);
    push_u32_le(&mut candidate_id, signatures.slot);
    candidate_id.extend_from_slice(&candidate_hash);
    let mut finalize_vote = Vec::with_capacity(44);
    push_u32_le(
        &mut finalize_vote,
        TON_CONSENSUS_SIMPLEX_FINALIZE_TL_CONSTRUCTOR,
    );
    finalize_vote.extend_from_slice(&candidate_id);
    let mut transcript = Vec::with_capacity(84);
    push_u32_le(&mut transcript, TON_CONSENSUS_DATA_TO_SIGN_TL_CONSTRUCTOR);
    transcript.extend_from_slice(&signatures.session_id);
    push_tl_bytes(&mut transcript, &finalize_vote)?;
    Some(transcript)
}

fn verify_block_signatures(
    block: TonBlockIdExtV1,
    active: &TonValidatorSetV1,
    signatures: &TonBlockSignaturesV1,
) -> Result<(), TonNativeSourceError> {
    let total_weight =
        validate_active_validator_set(active).ok_or(TonNativeSourceError::InvalidValidatorSet)?;
    let (catchain_seqno, validator_hash, entries, transcript) = match signatures {
        TonBlockSignaturesV1::Ordinary(proof) => (
            proof.catchain_seqno,
            proof.validator_list_hash_short,
            proof.signatures.as_slice(),
            ton_block_id_tl_bytes(block),
        ),
        TonBlockSignaturesV1::Simplex(proof) => (
            proof.catchain_seqno,
            proof.validator_list_hash_short,
            proof.signatures.as_slice(),
            simplex_finality_transcript(block, proof)
                .ok_or(TonNativeSourceError::InvalidSimplexTranscript)?,
        ),
    };
    if catchain_seqno != active.catchain_seqno
        || validator_hash != active.validator_list_hash_short
        || entries.is_empty()
        || entries.len() > TON_MAX_SIGNATURES
    {
        return Err(TonNativeSourceError::InvalidSignatures);
    }
    let by_node = active
        .validators
        .iter()
        .map(|validator| {
            (
                ton_validator_node_id_short_from_validated(&validator.public_key),
                validator,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    let mut signed_weight = 0_u64;
    let mut raw_signatures = Vec::<&[u8]>::with_capacity(entries.len());
    let mut raw_keys = Vec::<&[u8]>::with_capacity(entries.len());
    let mut messages = Vec::<&[u8]>::with_capacity(entries.len());
    for signature in entries {
        if signature.signature.len() != 64 || !seen.insert(signature.node_id_short) {
            return Err(TonNativeSourceError::InvalidSignatures);
        }
        let validator = by_node
            .get(&signature.node_id_short)
            .copied()
            .ok_or(TonNativeSourceError::InvalidSignatures)?;
        signed_weight = signed_weight
            .checked_add(validator.weight)
            .ok_or(TonNativeSourceError::InvalidSignatures)?;
        raw_signatures.push(signature.signature.as_slice());
        raw_keys.push(validator.public_key.as_slice());
        messages.push(transcript.as_slice());
    }
    if u128::from(signed_weight) * 3 <= u128::from(total_weight) * 2 {
        return Err(TonNativeSourceError::InvalidSignatures);
    }
    // The batch verifier's signer-key parsing is charged by
    // `ed25519_signature_checks`; roster parsing above is charged separately
    // by `validator_key_checks_upper_bound`.
    iroha_crypto::ed25519_verify_batch_deterministic(&messages, &raw_signatures, &raw_keys)
        .map_err(|_| TonNativeSourceError::InvalidSignatures)
}

fn ton_crc32c(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff_u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0x82f6_3b78 & mask);
        }
    }
    !crc
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TonBocCell {
    descriptor: u8,
    data_descriptor: u8,
    data: Vec<u8>,
    refs: Vec<usize>,
    exotic: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TonBoc {
    roots: Vec<usize>,
    cells: Vec<TonBocCell>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct TonCellStructuralKey {
    descriptor: u8,
    data_descriptor: u8,
    data: Vec<u8>,
    child_classes: Vec<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TonCellType {
    Ordinary,
    PrunedBranch,
    MerkleProof,
    MerkleUpdate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TonPrunedBranch {
    mask: u8,
    hashes: Vec<H256>,
    depths: Vec<u16>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TonComputedCell {
    mask: u8,
    hashes: [H256; 4],
    depths: [u16; 4],
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TonBitReader<'a> {
    cell: &'a TonBocCell,
    bit_len: usize,
    bit_offset: usize,
    ref_offset: usize,
}

fn ton_read_sized_uint(bytes: &[u8], cursor: &mut usize, size: usize) -> Option<usize> {
    if !(1..=8).contains(&size) {
        return None;
    }
    let end = cursor.checked_add(size)?;
    let mut value = 0_usize;
    for byte in bytes.get(*cursor..end)? {
        value = value.checked_shl(8)?.checked_add(usize::from(*byte))?;
    }
    *cursor = end;
    Some(value)
}

fn ton_cell_serialized_bit_len(data_descriptor: u8, data: &[u8]) -> Option<usize> {
    if data_descriptor & 1 == 0 {
        let byte_len = usize::from(data_descriptor) / 2;
        return (byte_len == data.len()).then_some(byte_len.checked_mul(8)?);
    }
    let full_bytes = usize::from(data_descriptor).checked_add(1)? / 2;
    let floor_bytes = usize::from(data_descriptor) / 2;
    if full_bytes != data.len() || floor_bytes.checked_add(1)? != full_bytes {
        return None;
    }
    let last = *data.last()?;
    if last == 0 {
        return None;
    }
    let tail_bits = 7_usize.checked_sub(usize::try_from(last.trailing_zeros()).ok()?)?;
    floor_bytes.checked_mul(8)?.checked_add(tail_bits)
}

impl<'a> TonBitReader<'a> {
    fn new(cell: &'a TonBocCell) -> Option<Self> {
        Some(Self {
            cell,
            bit_len: ton_cell_serialized_bit_len(cell.data_descriptor, &cell.data)?,
            bit_offset: 0,
            ref_offset: 0,
        })
    }

    fn read_bit(&mut self) -> Option<bool> {
        if self.bit_offset >= self.bit_len {
            return None;
        }
        let byte = *self.cell.data.get(self.bit_offset / 8)?;
        let shift = 7_usize.checked_sub(self.bit_offset % 8)?;
        self.bit_offset = self.bit_offset.checked_add(1)?;
        Some(((byte >> shift) & 1) != 0)
    }

    fn read_u64(&mut self, bits: usize) -> Option<u64> {
        if bits > 64 {
            return None;
        }
        let mut value = 0_u64;
        for _ in 0..bits {
            value = value.checked_shl(1)?;
            if self.read_bit()? {
                value = value.checked_add(1)?;
            }
        }
        Some(value)
    }

    fn read_usize(&mut self, bits: usize) -> Option<usize> {
        usize::try_from(self.read_u64(bits)?).ok()
    }

    fn read_i32(&mut self, bits: usize) -> Option<i32> {
        if bits == 0 || bits > 32 {
            return None;
        }
        let raw = u32::try_from(self.read_u64(bits)?).ok()?;
        if bits == 32 {
            return Some(i32::from_be_bytes(raw.to_be_bytes()));
        }
        let sign = 1_u32 << (bits - 1);
        let extended = if raw & sign == 0 {
            raw
        } else {
            raw | (!0_u32 << bits)
        };
        Some(i32::from_be_bytes(extended.to_be_bytes()))
    }

    fn read_h256(&mut self) -> Option<H256> {
        let mut out = [0_u8; 32];
        for byte in &mut out {
            *byte = u8::try_from(self.read_u64(8)?).ok()?;
        }
        Some(out)
    }

    fn skip_bits(&mut self, bits: usize) -> Option<()> {
        if self.remaining_bits()? < bits {
            return None;
        }
        self.bit_offset = self.bit_offset.checked_add(bits)?;
        Some(())
    }

    fn read_ref(&mut self) -> Option<usize> {
        let index = *self.cell.refs.get(self.ref_offset)?;
        self.ref_offset = self.ref_offset.checked_add(1)?;
        Some(index)
    }

    fn remaining_bits(&self) -> Option<usize> {
        self.bit_len.checked_sub(self.bit_offset)
    }

    fn remaining_refs(&self) -> Option<usize> {
        self.cell.refs.len().checked_sub(self.ref_offset)
    }

    fn exhausted(&self) -> bool {
        self.remaining_bits() == Some(0) && self.remaining_refs() == Some(0)
    }
}

fn ton_cell_type(cell: &TonBocCell) -> Option<TonCellType> {
    if !cell.exotic {
        return Some(TonCellType::Ordinary);
    }
    match *cell.data.first()? {
        1 => Some(TonCellType::PrunedBranch),
        3 => Some(TonCellType::MerkleProof),
        4 => Some(TonCellType::MerkleUpdate),
        _ => None,
    }
}

fn ton_level_mask_value(mask: u8) -> u8 {
    mask & 0x07
}

fn ton_level_mask_level(mask: u8) -> u8 {
    let mask = ton_level_mask_value(mask);
    if mask == 0 {
        0
    } else {
        8 - u8::try_from(mask.leading_zeros()).expect("u8 leading-zero count fits")
    }
}

fn ton_level_mask_hash_index(mask: u8) -> usize {
    usize::try_from(ton_level_mask_value(mask).count_ones()).expect("three-bit popcount fits")
}

fn ton_level_mask_apply(mask: u8, level: u8) -> u8 {
    if level == 0 {
        0
    } else {
        ton_level_mask_value(mask) & ((1_u8 << level) - 1)
    }
}

fn ton_level_mask_is_significant(mask: u8, level: u8) -> bool {
    level == 0 || ((ton_level_mask_value(mask) >> (level - 1)) & 1) != 0
}

fn ton_child_hash_depth(computed: &TonComputedCell, level: u8) -> Option<(H256, u16)> {
    let index = usize::from(level.min(3));
    let depth = *computed.depths.get(index)?;
    (depth <= TON_MAX_CELL_DEPTH).then_some((*computed.hashes.get(index)?, depth))
}

fn ton_parse_pruned_branch(cell: &TonBocCell) -> Option<TonPrunedBranch> {
    if cell.data_descriptor & 1 != 0
        || usize::from(cell.data_descriptor) / 2 != cell.data.len()
        || !cell.refs.is_empty()
        || cell.data.len() < 2
        || cell.data.first().copied()? != 1
    {
        return None;
    }
    if cell.data.len() == 35 {
        let depth = u16::from_be_bytes(cell.data.get(33..35)?.try_into().ok()?);
        if depth > TON_MAX_CELL_DEPTH {
            return None;
        }
        return Some(TonPrunedBranch {
            mask: 1,
            hashes: vec![cell.data.get(1..33)?.try_into().ok()?],
            depths: vec![depth],
        });
    }
    let raw_mask = *cell.data.get(1)?;
    if raw_mask & !0x07 != 0 {
        return None;
    }
    let mask = raw_mask;
    let level = ton_level_mask_level(mask);
    if !(1..=3).contains(&level) {
        return None;
    }
    let count = ton_level_mask_hash_index(mask);
    if cell.data.len() != 2_usize.checked_add(count.checked_mul(34)?)? {
        return None;
    }
    let mut hashes = Vec::with_capacity(count);
    for index in 0..count {
        let start = 2_usize.checked_add(index.checked_mul(32)?)?;
        hashes.push(
            cell.data
                .get(start..start.checked_add(32)?)?
                .try_into()
                .ok()?,
        );
    }
    let depths_start = 2_usize.checked_add(count.checked_mul(32)?)?;
    let mut depths = Vec::with_capacity(count);
    for index in 0..count {
        let start = depths_start.checked_add(index.checked_mul(2)?)?;
        let depth = u16::from_be_bytes(
            cell.data
                .get(start..start.checked_add(2)?)?
                .try_into()
                .ok()?,
        );
        if depth > TON_MAX_CELL_DEPTH {
            return None;
        }
        depths.push(depth);
    }
    Some(TonPrunedBranch {
        mask,
        hashes,
        depths,
    })
}

fn parse_ton_boc(bytes: &[u8]) -> Option<TonBoc> {
    if bytes.len() < 6 || bytes.len() > TON_MAX_BOC_BYTES || bytes.get(..4)? != TON_BOC_MAGIC {
        return None;
    }
    let mut cursor = 4_usize;
    let flags_size = *bytes.get(cursor)?;
    cursor += 1;
    let has_index = flags_size & 0x80 != 0;
    let has_crc32c = flags_size & 0x40 != 0;
    let has_cache_bits = flags_size & 0x20 != 0;
    let flags = (flags_size >> 3) & 0x03;
    let size_bytes = usize::from(flags_size & 0x07);
    let offset_bytes = usize::from(*bytes.get(cursor)?);
    cursor += 1;
    if has_cache_bits
        || flags != 0
        || !(1..=4).contains(&size_bytes)
        || !(1..=8).contains(&offset_bytes)
    {
        return None;
    }
    let cells_count = ton_read_sized_uint(bytes, &mut cursor, size_bytes)?;
    let roots_count = ton_read_sized_uint(bytes, &mut cursor, size_bytes)?;
    let absent_count = ton_read_sized_uint(bytes, &mut cursor, size_bytes)?;
    let total_cells_size = ton_read_sized_uint(bytes, &mut cursor, offset_bytes)?;
    if cells_count == 0 || cells_count > TON_MAX_BOC_CELLS || roots_count != 1 || absent_count != 0
    {
        return None;
    }
    let root = ton_read_sized_uint(bytes, &mut cursor, size_bytes)?;
    if root >= cells_count {
        return None;
    }
    let roots = vec![root];
    let index_offsets = if has_index {
        let mut offsets = Vec::with_capacity(cells_count);
        let mut previous = 0_usize;
        for index in 0..cells_count {
            let offset = ton_read_sized_uint(bytes, &mut cursor, offset_bytes)?;
            if offset < previous || offset > total_cells_size {
                return None;
            }
            if index + 1 == cells_count && offset != total_cells_size {
                return None;
            }
            previous = offset;
            offsets.push(offset);
        }
        Some(offsets)
    } else {
        None
    };
    let cell_data_start = cursor;
    let cell_data_end = cell_data_start.checked_add(total_cells_size)?;
    let expected_end = cell_data_end.checked_add(if has_crc32c { 4 } else { 0 })?;
    if expected_end != bytes.len() {
        return None;
    }
    if has_crc32c {
        let expected = ton_crc32c(bytes.get(..cell_data_end)?).to_le_bytes();
        if bytes.get(cell_data_end..expected_end)? != expected {
            return None;
        }
    }
    let cell_data = bytes.get(cell_data_start..cell_data_end)?;
    let mut cell_cursor = 0_usize;
    let mut cells = Vec::with_capacity(cells_count);
    for cell_index in 0..cells_count {
        let descriptor = *cell_data.get(cell_cursor)?;
        cell_cursor += 1;
        let data_descriptor = *cell_data.get(cell_cursor)?;
        cell_cursor += 1;
        let refs_count = usize::from(descriptor & 0x07);
        let exotic = descriptor & 0x08 != 0;
        let has_hashes = descriptor & 0x10 != 0;
        let data_bytes = usize::from(data_descriptor).checked_add(1)? / 2;
        if refs_count > TON_MAX_REFS || has_hashes || data_bytes > TON_MAX_CELL_DATA_BYTES {
            return None;
        }
        let data_end = cell_cursor.checked_add(data_bytes)?;
        let data = cell_data.get(cell_cursor..data_end)?.to_vec();
        ton_cell_serialized_bit_len(data_descriptor, &data)?;
        cell_cursor = data_end;
        let mut refs = Vec::with_capacity(refs_count);
        for _ in 0..refs_count {
            let reference = ton_read_sized_uint(cell_data, &mut cell_cursor, size_bytes)?;
            if reference >= cells_count || reference <= cell_index {
                return None;
            }
            refs.push(reference);
        }
        if index_offsets
            .as_ref()
            .is_some_and(|offsets| offsets.get(cell_index) != Some(&cell_cursor))
        {
            return None;
        }
        cells.push(TonBocCell {
            descriptor: descriptor & !0x10,
            data_descriptor,
            data,
            refs,
            exotic,
        });
    }
    (cell_cursor == cell_data.len()).then_some(TonBoc { roots, cells })
}

fn ton_minimum_sized_uint_bytes(value: usize) -> usize {
    let significant_bits =
        usize::try_from(usize::BITS - value.leading_zeros()).expect("usize bit width fits usize");
    significant_bits.div_ceil(8).max(1)
}

fn ton_write_sized_uint(out: &mut Vec<u8>, value: usize, size: usize) -> Option<()> {
    if !(1..=8).contains(&size) || ton_minimum_sized_uint_bytes(value) > size {
        return None;
    }
    let encoded = u64::try_from(value).ok()?.to_be_bytes();
    out.extend_from_slice(encoded.get(8_usize.checked_sub(size)?..)?);
    Some(())
}

fn ton_canonical_cell_order(boc: &TonBoc, root: usize) -> Option<Vec<usize>> {
    fn visit(
        boc: &TonBoc,
        index: usize,
        visiting: &mut [bool],
        visited: &mut [bool],
        postorder: &mut Vec<usize>,
    ) -> Option<()> {
        if *visited.get(index)? {
            return Some(());
        }
        if *visiting.get(index)? {
            return None;
        }
        *visiting.get_mut(index)? = true;
        for reference in boc.cells.get(index)?.refs.iter().rev() {
            visit(boc, *reference, visiting, visited, postorder)?;
        }
        *visiting.get_mut(index)? = false;
        *visited.get_mut(index)? = true;
        postorder.push(index);
        Some(())
    }

    let mut visiting = vec![false; boc.cells.len()];
    let mut visited = vec![false; boc.cells.len()];
    let mut postorder = Vec::with_capacity(boc.cells.len());
    visit(boc, root, &mut visiting, &mut visited, &mut postorder)?;
    if visited.iter().any(|seen| !seen) {
        return None;
    }
    postorder.reverse();
    (postorder.first() == Some(&root)).then_some(postorder)
}

fn ton_reject_duplicate_subgraphs(boc: &TonBoc) -> Option<()> {
    let mut class_by_cell = vec![0_usize; boc.cells.len()];
    let mut classes = BTreeMap::<TonCellStructuralKey, usize>::new();
    for index in (0..boc.cells.len()).rev() {
        let cell = boc.cells.get(index)?;
        let key = TonCellStructuralKey {
            descriptor: cell.descriptor,
            data_descriptor: cell.data_descriptor,
            data: cell.data.clone(),
            child_classes: cell
                .refs
                .iter()
                .map(|reference| class_by_cell.get(*reference).copied())
                .collect::<Option<Vec<_>>>()?,
        };
        let class = classes.len().checked_add(1)?;
        if classes.insert(key, class).is_some() {
            // Canonical BoCs share one cell for one structural subtree. Two
            // byte-identical subgraphs would otherwise give the same TON root
            // while changing the observation proof bytes and CAS digest.
            return None;
        }
        *class_by_cell.get_mut(index)? = class;
    }
    Some(())
}

fn encode_canonical_ton_boc(boc: &TonBoc, root: usize) -> Option<Vec<u8>> {
    let order = ton_canonical_cell_order(boc, root)?;
    ton_reject_duplicate_subgraphs(boc)?;
    let mut canonical_index = vec![usize::MAX; boc.cells.len()];
    for (index, old_index) in order.iter().copied().enumerate() {
        *canonical_index.get_mut(old_index)? = index;
    }
    let size_bytes = ton_minimum_sized_uint_bytes(order.len());
    if size_bytes > 4 {
        return None;
    }
    let mut cell_data = Vec::new();
    for old_index in &order {
        let cell = boc.cells.get(*old_index)?;
        if usize::from(cell.descriptor & 0x07) != cell.refs.len()
            || (cell.descriptor & 0x08 != 0) != cell.exotic
            || cell.descriptor & 0x10 != 0
            || cell.data_descriptor & 1 != 0 && cell.data.last() == Some(&0x80)
        {
            return None;
        }
        if ton_cell_type(cell)? == TonCellType::PrunedBranch && cell.data.len() == 35 {
            // The historical implicit-mask form is representation-malleable
            // with the explicit final-V1 pruned-branch encoding.
            return None;
        }
        cell_data.push(cell.descriptor);
        cell_data.push(cell.data_descriptor);
        cell_data.extend_from_slice(&cell.data);
        for reference in &cell.refs {
            let mapped = *canonical_index.get(*reference)?;
            if mapped == usize::MAX || mapped <= *canonical_index.get(*old_index)? {
                return None;
            }
            let encoded = u64::try_from(mapped).ok()?.to_be_bytes();
            cell_data.extend_from_slice(encoded.get(8_usize.checked_sub(size_bytes)?..)?);
        }
    }
    let offset_bytes = ton_minimum_sized_uint_bytes(cell_data.len());
    if offset_bytes > 8 {
        return None;
    }
    let mut out = Vec::with_capacity(
        6_usize
            .checked_add(size_bytes.checked_mul(3)?)?
            .checked_add(offset_bytes)?
            .checked_add(size_bytes)?
            .checked_add(cell_data.len())?,
    );
    out.extend_from_slice(&TON_BOC_MAGIC);
    out.push(u8::try_from(size_bytes).ok()?); // no index, CRC, cache bits, or flags
    out.push(u8::try_from(offset_bytes).ok()?);
    ton_write_sized_uint(&mut out, order.len(), size_bytes)?;
    ton_write_sized_uint(&mut out, 1, size_bytes)?;
    ton_write_sized_uint(&mut out, 0, size_bytes)?;
    ton_write_sized_uint(&mut out, cell_data.len(), offset_bytes)?;
    ton_write_sized_uint(&mut out, 0, size_bytes)?; // canonical root is cell zero
    out.extend_from_slice(&cell_data);
    Some(out)
}

fn parse_canonical_single_root_boc(bytes: &[u8]) -> Option<(TonBoc, Vec<TonComputedCell>, usize)> {
    let boc = parse_ton_boc(bytes)?;
    let root = *boc.roots.first()?;
    if boc.roots.len() != 1 || root != 0 {
        return None;
    }
    match ton_cell_type(boc.cells.get(root)?)? {
        TonCellType::Ordinary => {}
        TonCellType::MerkleProof => {
            let child = *boc.cells.get(root)?.refs.first()?;
            if ton_cell_type(boc.cells.get(child)?)? != TonCellType::Ordinary {
                return None;
            }
        }
        TonCellType::PrunedBranch | TonCellType::MerkleUpdate => return None,
    }
    for (index, cell) in boc.cells.iter().enumerate() {
        if index != root && ton_cell_type(cell)? == TonCellType::MerkleProof {
            // A proof envelope has one root wrapper at most. Native block
            // MerkleUpdate cells remain valid typed block content.
            return None;
        }
    }
    // Validate descriptors, exotic payloads, and the 1,024-cell depth bound
    // iteratively before the canonical-order DFS. This prevents a deeply
    // nested hostile BOC from reaching recursive canonicalization first.
    let computed = ton_boc_cell_hashes(&boc)?;
    if encode_canonical_ton_boc(&boc, root)?.as_slice() != bytes {
        return None;
    }
    Some((boc, computed, root))
}

/// Derive the authenticated root hash only when a proof BoC has the one
/// canonical final-V1 byte representation.
///
/// Canonical proof BoCs are single-root, unindexed, checksum-free, use minimal
/// integer widths and root index zero, contain no unreachable or duplicate
/// structural subgraphs, and admit at most one root Merkle-proof wrapper.
#[must_use]
pub fn ton_canonical_boc_single_root_hash_v1(bytes: &[u8]) -> Option<H256> {
    let (boc, computed, root) = parse_canonical_single_root_boc(bytes)?;
    ton_proven_root_hash(&boc, &computed, root)
}

fn ton_boc_child_for_hash_level(
    cell_type: TonCellType,
    computed: &TonComputedCell,
    level: u8,
) -> Option<(H256, u16)> {
    let child_level = match cell_type {
        TonCellType::MerkleProof | TonCellType::MerkleUpdate => level.checked_add(1)?,
        TonCellType::Ordinary | TonCellType::PrunedBranch => level,
    };
    ton_child_hash_depth(computed, child_level)
}

fn ton_boc_cell_hashes(boc: &TonBoc) -> Option<Vec<TonComputedCell>> {
    let empty = TonComputedCell {
        mask: 0,
        hashes: [[0_u8; 32]; 4],
        depths: [0_u16; 4],
    };
    let mut computed = vec![empty; boc.cells.len()];
    for index in (0..boc.cells.len()).rev() {
        let cell = boc.cells.get(index)?;
        let cell_type = ton_cell_type(cell)?;
        let pruned = match cell_type {
            TonCellType::PrunedBranch => Some(ton_parse_pruned_branch(cell)?),
            _ => None,
        };
        let mask = match cell_type {
            TonCellType::Ordinary => cell.refs.iter().try_fold(0_u8, |mask, reference| {
                Some(mask | computed.get(*reference)?.mask)
            })?,
            TonCellType::PrunedBranch => pruned.as_ref()?.mask,
            TonCellType::MerkleProof => {
                if cell.data_descriptor & 1 != 0 || cell.data.len() != 35 || cell.refs.len() != 1 {
                    return None;
                }
                let reference = *cell.refs.first()?;
                let (child_hash, child_depth) = ton_child_hash_depth(computed.get(reference)?, 0)?;
                if cell.data.get(1..33)? != child_hash
                    || u16::from_be_bytes(cell.data.get(33..35)?.try_into().ok()?) != child_depth
                {
                    return None;
                }
                ton_level_mask_value(computed.get(reference)?.mask >> 1)
            }
            TonCellType::MerkleUpdate => {
                if cell.data_descriptor & 1 != 0 || cell.data.len() != 69 || cell.refs.len() != 2 {
                    return None;
                }
                for (position, hash_offset, depth_offset) in
                    [(0_usize, 1_usize, 65_usize), (1, 33, 67)]
                {
                    let reference = *cell.refs.get(position)?;
                    let (child_hash, child_depth) =
                        ton_child_hash_depth(computed.get(reference)?, 0)?;
                    if cell.data.get(hash_offset..hash_offset + 32)? != child_hash
                        || u16::from_be_bytes(
                            cell.data
                                .get(depth_offset..depth_offset + 2)?
                                .try_into()
                                .ok()?,
                        ) != child_depth
                    {
                        return None;
                    }
                }
                ton_level_mask_value(
                    (computed.get(*cell.refs.first()?)?.mask
                        | computed.get(*cell.refs.get(1)?)?.mask)
                        >> 1,
                )
            }
        };
        if (cell.descriptor >> 5) & 0x07 != mask {
            return None;
        }
        let total_hash_count = ton_level_mask_hash_index(mask).checked_add(1)?;
        let hash_count = if cell_type == TonCellType::PrunedBranch {
            1
        } else {
            total_hash_count
        };
        let hash_offset = total_hash_count.checked_sub(hash_count)?;
        let mut hashes = Vec::<H256>::with_capacity(hash_count);
        let mut depths = Vec::<u16>::with_capacity(hash_count);
        let level = ton_level_mask_level(mask);
        let mut hash_index = 0_usize;
        for level_index in 0..=level {
            if !ton_level_mask_is_significant(mask, level_index) {
                continue;
            }
            if hash_index < hash_offset {
                hash_index += 1;
                continue;
            }
            let current_data: &[u8] = if hash_index == hash_offset {
                if level_index != 0 && cell_type != TonCellType::PrunedBranch {
                    return None;
                }
                &cell.data
            } else {
                hashes.get(hash_index.checked_sub(hash_offset)?.checked_sub(1)?)?
            };
            let mut current_depth = 0_u16;
            for reference in &cell.refs {
                let (_, child_depth) = ton_boc_child_for_hash_level(
                    cell_type,
                    computed.get(*reference)?,
                    level_index,
                )?;
                current_depth = current_depth.max(child_depth);
            }
            if !cell.refs.is_empty() {
                current_depth = current_depth.checked_add(1)?;
            }
            if current_depth > TON_MAX_CELL_DEPTH {
                return None;
            }
            let descriptor = u8::try_from(cell.refs.len()).ok()?
                | if cell_type == TonCellType::Ordinary {
                    0
                } else {
                    0x08
                }
                | ton_level_mask_apply(mask, level_index).checked_shl(5)?;
            let mut repr = Vec::with_capacity(
                2_usize
                    .checked_add(current_data.len())?
                    .checked_add(cell.refs.len().checked_mul(34)?)?,
            );
            repr.push(descriptor);
            repr.push(cell.data_descriptor);
            repr.extend_from_slice(current_data);
            for reference in &cell.refs {
                let (_, child_depth) = ton_boc_child_for_hash_level(
                    cell_type,
                    computed.get(*reference)?,
                    level_index,
                )?;
                repr.extend_from_slice(&child_depth.to_be_bytes());
            }
            for reference in &cell.refs {
                let (child_hash, _) = ton_boc_child_for_hash_level(
                    cell_type,
                    computed.get(*reference)?,
                    level_index,
                )?;
                repr.extend_from_slice(&child_hash);
            }
            hashes.push(Sha256::digest(&repr).into());
            depths.push(current_depth);
            hash_index += 1;
        }
        if hashes.len() != hash_count || depths.len() != hash_count {
            return None;
        }
        let mut resolved_hashes = [[0_u8; 32]; 4];
        let mut resolved_depths = [0_u16; 4];
        for resolved_level in 0_u8..4 {
            let resolved_index =
                ton_level_mask_hash_index(ton_level_mask_apply(mask, resolved_level));
            if let Some(pruned) = &pruned {
                if resolved_index != ton_level_mask_hash_index(mask) {
                    resolved_hashes[usize::from(resolved_level)] =
                        *pruned.hashes.get(resolved_index)?;
                    resolved_depths[usize::from(resolved_level)] =
                        *pruned.depths.get(resolved_index)?;
                } else {
                    resolved_hashes[usize::from(resolved_level)] = *hashes.first()?;
                    resolved_depths[usize::from(resolved_level)] = *depths.first()?;
                }
            } else {
                resolved_hashes[usize::from(resolved_level)] = *hashes.get(resolved_index)?;
                resolved_depths[usize::from(resolved_level)] = *depths.get(resolved_index)?;
            }
        }
        if resolved_depths
            .iter()
            .any(|depth| *depth > TON_MAX_CELL_DEPTH)
        {
            return None;
        }
        computed[index] = TonComputedCell {
            mask,
            hashes: resolved_hashes,
            depths: resolved_depths,
        };
    }
    Some(computed)
}

fn parse_single_root_boc(bytes: &[u8]) -> Option<(TonBoc, Vec<TonComputedCell>, usize)> {
    let boc = parse_ton_boc(bytes)?;
    if boc.roots.len() != 1 {
        return None;
    }
    let root = *boc.roots.first()?;
    let computed = ton_boc_cell_hashes(&boc)?;
    Some((boc, computed, root))
}

fn ton_merkle_opened_index(boc: &TonBoc, mut index: usize) -> Option<usize> {
    let mut remaining = boc.cells.len().checked_add(1)?;
    loop {
        remaining = remaining.checked_sub(1)?;
        match ton_cell_type(boc.cells.get(index)?)? {
            TonCellType::Ordinary | TonCellType::PrunedBranch => return Some(index),
            TonCellType::MerkleProof => {
                index = *boc.cells.get(index)?.refs.first()?;
            }
            TonCellType::MerkleUpdate => return None,
        }
    }
}

fn ton_virtual_root_index(boc: &TonBoc, index: usize) -> Option<usize> {
    let index = ton_merkle_opened_index(boc, index)?;
    (ton_cell_type(boc.cells.get(index)?)? == TonCellType::Ordinary).then_some(index)
}

fn ton_original_tree_hash(computed: &[TonComputedCell], index: usize) -> Option<H256> {
    Some(computed.get(index)?.hashes[0])
}

fn ton_opened_original_tree_hash(
    boc: &TonBoc,
    computed: &[TonComputedCell],
    index: usize,
) -> Option<H256> {
    ton_original_tree_hash(computed, ton_merkle_opened_index(boc, index)?)
}

fn ton_proven_root_hash(boc: &TonBoc, computed: &[TonComputedCell], root: usize) -> Option<H256> {
    match ton_cell_type(boc.cells.get(root)?)? {
        TonCellType::Ordinary => ton_original_tree_hash(computed, root),
        TonCellType::MerkleProof => boc.cells.get(root)?.data.get(1..33)?.try_into().ok(),
        TonCellType::PrunedBranch | TonCellType::MerkleUpdate => None,
    }
}

/// Derive the authenticated hash-zero identity of one bounded single-root BoC.
pub fn ton_boc_single_root_hash_v1(bytes: &[u8]) -> Option<H256> {
    let (boc, computed, root) = parse_single_root_boc(bytes)?;
    ton_proven_root_hash(&boc, &computed, root)
}

// Parse one complete ordinary-cell DAG for strict deployment evidence.
fn parse_complete_ordinary_single_root_boc(
    bytes: &[u8],
) -> Option<(TonBoc, Vec<TonComputedCell>, usize)> {
    let (boc, computed, root) = parse_single_root_boc(bytes)?;
    if boc
        .cells
        .iter()
        .any(|cell| ton_cell_type(cell) != Some(TonCellType::Ordinary))
    {
        return None;
    }
    let mut reachable = vec![false; boc.cells.len()];
    let mut pending = vec![root];
    while let Some(index) = pending.pop() {
        if *reachable.get(index)? {
            continue;
        }
        *reachable.get_mut(index)? = true;
        pending.extend_from_slice(&boc.cells.get(index)?.refs);
    }
    if reachable.iter().any(|seen| !seen) {
        return None;
    }
    Some((boc, computed, root))
}

/// Derive the representation hash of one bounded single-root BOC whose complete
/// cell DAG contains only ordinary cells rather than exotic proof wrappers.
/// Deployment evidence uses this form so every committed cell is present and
/// no unreachable trailing cell can masquerade as part of the artifact.
#[must_use]
pub fn ton_boc_single_ordinary_root_hash_v1(bytes: &[u8]) -> Option<H256> {
    let (_boc, computed, root) = parse_complete_ordinary_single_root_boc(bytes)?;
    ton_original_tree_hash(&computed, root)
}

/// Derive the basechain account id for the canonical SCCP TON `StateInit` made
/// from exact code and data BOCs.
///
/// The constructed root has absent `split_depth` and `special`, present code
/// and data references, and an empty library (`00110` in TL-B field order).
/// Both supplied BOCs must be complete, single-root ordinary-cell DAGs.
#[must_use]
pub fn ton_state_init_address_hash_v1(code_boc: &[u8], data_boc: &[u8]) -> Option<H256> {
    let (_code, code_cells, code_root) = parse_complete_ordinary_single_root_boc(code_boc)?;
    let (_data, data_cells, data_root) = parse_complete_ordinary_single_root_boc(data_boc)?;
    let (code_hash, code_depth) = ton_child_hash_depth(code_cells.get(code_root)?, 0)?;
    let (data_hash, data_depth) = ton_child_hash_depth(data_cells.get(data_root)?, 0)?;
    Some(
        ton_state_init_hash_from_children(
            TonCellHashDepth::new(code_hash, code_depth)?,
            TonCellHashDepth::new(data_hash, data_depth)?,
        )?
        .hash,
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TonCellHashDepth {
    hash: H256,
    depth: u16,
}

impl TonCellHashDepth {
    fn new(hash: H256, depth: u16) -> Option<Self> {
        (nonzero(&hash) && depth <= TON_MAX_CELL_DEPTH).then_some(Self { hash, depth })
    }
}

#[derive(Default)]
struct TonCanonicalCellBits {
    data: Vec<u8>,
    bit_len: usize,
}

impl TonCanonicalCellBits {
    fn push_bit(&mut self, value: bool) -> Option<()> {
        if self.bit_len >= 1_023 {
            return None;
        }
        if self.bit_len % 8 == 0 {
            self.data.push(0);
        }
        if value {
            *self.data.last_mut()? |= 1 << (7 - self.bit_len % 8);
        }
        self.bit_len = self.bit_len.checked_add(1)?;
        Some(())
    }

    fn push_u64(&mut self, value: u64, width: usize) -> Option<()> {
        if width > 64 || width < 64 && value >= (1_u64 << width) {
            return None;
        }
        for shift in (0..width).rev() {
            self.push_bit(value & (1_u64 << shift) != 0)?;
        }
        Some(())
    }

    fn push_bytes(&mut self, value: &[u8]) -> Option<()> {
        for byte in value {
            self.push_u64(u64::from(*byte), 8)?;
        }
        Some(())
    }

    fn push_std_address(&mut self, address: SccpTonAddressV1) -> Option<()> {
        if address.workchain != SCCP_TON_BASECHAIN_WORKCHAIN_V1 || !nonzero(&address.account) {
            return None;
        }
        self.push_bit(true)?;
        self.push_bit(false)?;
        self.push_bit(false)?; // `addr_std$10` without anycast: `100`.
        let workchain = i8::try_from(address.workchain).ok()?;
        self.push_u64(u64::from(workchain.to_be_bytes()[0]), 8)?;
        self.push_bytes(&address.account)
    }

    fn finish(mut self, refs: &[TonCellHashDepth]) -> Option<TonCellHashDepth> {
        if refs.len() > TON_MAX_REFS
            || refs
                .iter()
                .any(|reference| reference.depth > TON_MAX_CELL_DEPTH)
        {
            return None;
        }
        let byte_len = self.bit_len.div_ceil(8);
        let data_descriptor = if self.bit_len % 8 == 0 {
            byte_len.checked_mul(2)?
        } else {
            *self.data.last_mut()? |= 1 << (7 - self.bit_len % 8);
            byte_len.checked_mul(2)?.checked_sub(1)?
        };
        let mut repr = Vec::with_capacity(
            2_usize
                .checked_add(self.data.len())?
                .checked_add(refs.len().checked_mul(34)?)?,
        );
        repr.push(u8::try_from(refs.len()).ok()?);
        repr.push(u8::try_from(data_descriptor).ok()?);
        repr.extend_from_slice(&self.data);
        for reference in refs {
            repr.extend_from_slice(&reference.depth.to_be_bytes());
        }
        for reference in refs {
            repr.extend_from_slice(&reference.hash);
        }
        let depth = if refs.is_empty() {
            0
        } else {
            refs.iter()
                .map(|reference| reference.depth)
                .max()?
                .checked_add(1)?
        };
        TonCellHashDepth::new(Sha256::digest(&repr).into(), depth)
    }
}

fn ton_opened_hash_depth(
    boc: &TonBoc,
    computed: &[TonComputedCell],
    index: usize,
) -> Option<TonCellHashDepth> {
    let index = ton_merkle_opened_index(boc, index)?;
    let (hash, depth) = ton_child_hash_depth(computed.get(index)?, 0)?;
    TonCellHashDepth::new(hash, depth)
}

fn ton_state_init_hash_from_children(
    code: TonCellHashDepth,
    data: TonCellHashDepth,
) -> Option<TonCellHashDepth> {
    let mut bits = TonCanonicalCellBits::default();
    bits.push_bit(false)?; // split_depth absent
    bits.push_bit(false)?; // special absent
    bits.push_bit(true)?; // code reference present
    bits.push_bit(true)?; // data reference present
    bits.push_bit(false)?; // empty library
    bits.finish(&[code, data])
}

fn ton_empty_replay_forest_hash_depth_v1() -> Option<TonCellHashDepth> {
    let mut bits = TonCanonicalCellBits::default();
    bits.push_bit(false)?; // empty nonempty-shard-root dictionary
    bits.push_u64(0, 64)?; // leaf count
    bits.push_u64(0, 64)?; // update sequence
    bits.finish(&[])
}

fn ton_empty_replay_pair_hash_depth_v1() -> Option<TonCellHashDepth> {
    let empty = ton_empty_replay_forest_hash_depth_v1()?;
    TonCanonicalCellBits::default().finish(&[empty, empty])
}

fn ton_empty_route_pending_hash_depth_v1() -> Option<TonCellHashDepth> {
    let mut bits = TonCanonicalCellBits::default();
    bits.push_bit(false)?; // empty mint dictionary
    bits.push_bit(false)?; // empty burn dictionary
    bits.push_u64(0, 16)?; // pending mint count
    bits.push_u64(0, 16)?; // pending burn count
    bits.finish(&[])
}

fn ton_canonical_route_initial_data_hash_depth_v1(
    route_configuration_hash: H256,
    bridge_config: TonCellHashDepth,
) -> Option<TonCellHashDepth> {
    if !nonzero(&route_configuration_hash) || route_configuration_hash == bridge_config.hash {
        return None;
    }
    let replay = ton_empty_replay_pair_hash_depth_v1()?;
    let pending = ton_empty_route_pending_hash_depth_v1()?;
    let mut bits = TonCanonicalCellBits::default();
    bits.push_u64(u64::from(SCCP_V1_TON_STORAGE_VERSION), 8)?;
    bits.push_bytes(&route_configuration_hash)?;
    bits.push_bytes(&bridge_config.hash)?;
    bits.push_bit(false)?; // minting enabled initially
    bits.finish(&[bridge_config, replay, pending])
}

fn ton_canonical_master_initial_data_hash_depth_v1(
    route_configuration_hash: H256,
    bridge_config: TonCellHashDepth,
    master_metadata: TonCellHashDepth,
    route_address: SccpTonAddressV1,
) -> Option<TonCellHashDepth> {
    if !nonzero(&route_configuration_hash)
        || route_configuration_hash == bridge_config.hash
        || master_metadata.hash == bridge_config.hash
    {
        return None;
    }
    let replay = ton_empty_replay_pair_hash_depth_v1()?;
    let mut bits = TonCanonicalCellBits::default();
    bits.push_u64(u64::from(SCCP_V1_TON_STORAGE_VERSION), 8)?;
    bits.push_bytes(&route_configuration_hash)?;
    bits.push_bytes(&bridge_config.hash)?;
    bits.push_u64(0, 4)?; // canonical zero `coins`
    bits.push_std_address(route_address)?;
    bits.push_bit(false)?; // empty pending-mint dictionary
    bits.push_u64(0, 16)?; // pending mint count
    bits.push_bit(false)?; // minting enabled initially
    bits.finish(&[master_metadata, bridge_config, replay])
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TonCanonicalDeploymentBindingsV1 {
    route_initial_data: TonCellHashDepth,
    route_address: SccpTonAddressV1,
    master_initial_data: TonCellHashDepth,
    master_address: SccpTonAddressV1,
}

fn ton_canonical_deployment_bindings_v1(
    route_configuration_hash: H256,
    bridge_config: TonCellHashDepth,
    master_metadata: TonCellHashDepth,
    route_code: TonCellHashDepth,
    master_code: TonCellHashDepth,
) -> Option<TonCanonicalDeploymentBindingsV1> {
    let route_initial_data =
        ton_canonical_route_initial_data_hash_depth_v1(route_configuration_hash, bridge_config)?;
    let route_address = SccpTonAddressV1 {
        workchain: SCCP_TON_BASECHAIN_WORKCHAIN_V1,
        account: ton_state_init_hash_from_children(route_code, route_initial_data)?.hash,
    };
    let master_initial_data = ton_canonical_master_initial_data_hash_depth_v1(
        route_configuration_hash,
        bridge_config,
        master_metadata,
        route_address,
    )?;
    let master_address = SccpTonAddressV1 {
        workchain: SCCP_TON_BASECHAIN_WORKCHAIN_V1,
        account: ton_state_init_hash_from_children(master_code, master_initial_data)?.hash,
    };
    Some(TonCanonicalDeploymentBindingsV1 {
        route_initial_data,
        route_address,
        master_initial_data,
        master_address,
    })
}

fn ton_hashmap_uint_len_bits(max_value: usize) -> usize {
    usize::try_from(usize::BITS - max_value.leading_zeros()).expect("usize width fits")
}

fn ton_key_bit(key: &[u8], bit_len: u16, offset: usize) -> Option<bool> {
    if offset >= usize::from(bit_len) || key.len() != usize::from(bit_len).div_ceil(8) {
        return None;
    }
    let shift = 7_usize.checked_sub(offset % 8)?;
    Some((key[offset / 8] >> shift) & 1 != 0)
}

fn ton_read_hashmap_label(
    reader: &mut TonBitReader<'_>,
    key: &[u8],
    key_bit_len: u16,
    key_offset: usize,
    maximum: usize,
) -> Option<usize> {
    let long_or_same = reader.read_bit()?;
    let length;
    if !long_or_same {
        let mut unary = 0_usize;
        while reader.read_bit()? {
            unary = unary.checked_add(1)?;
            if unary > maximum {
                return None;
            }
        }
        length = unary;
        for index in 0..length {
            if reader.read_bit()? != ton_key_bit(key, key_bit_len, key_offset + index)? {
                return None;
            }
        }
    } else if !reader.read_bit()? {
        length = reader.read_usize(ton_hashmap_uint_len_bits(maximum))?;
        if length > maximum {
            return None;
        }
        for index in 0..length {
            if reader.read_bit()? != ton_key_bit(key, key_bit_len, key_offset + index)? {
                return None;
            }
        }
    } else {
        let value = reader.read_bit()?;
        length = reader.read_usize(ton_hashmap_uint_len_bits(maximum))?;
        if length > maximum {
            return None;
        }
        for index in 0..length {
            if value != ton_key_bit(key, key_bit_len, key_offset + index)? {
                return None;
            }
        }
    }
    Some(length)
}

fn ton_read_hashmap_label_bits(reader: &mut TonBitReader<'_>, maximum: usize) -> Option<Vec<bool>> {
    let long_or_same = reader.read_bit()?;
    let length;
    let mut bits = Vec::new();
    if !long_or_same {
        let mut unary = 0_usize;
        while reader.read_bit()? {
            unary = unary.checked_add(1)?;
            if unary > maximum {
                return None;
            }
        }
        length = unary;
        for _ in 0..length {
            bits.push(reader.read_bit()?);
        }
    } else if !reader.read_bit()? {
        length = reader.read_usize(ton_hashmap_uint_len_bits(maximum))?;
        if length > maximum {
            return None;
        }
        for _ in 0..length {
            bits.push(reader.read_bit()?);
        }
    } else {
        let value = reader.read_bit()?;
        length = reader.read_usize(ton_hashmap_uint_len_bits(maximum))?;
        if length > maximum {
            return None;
        }
        bits.resize(length, value);
    }
    Some(bits)
}

fn ton_hashmap_ref_value(boc: &TonBoc, root: usize, key: &[u8], key_bit_len: u16) -> Option<usize> {
    let mut cell_index = ton_virtual_root_index(boc, root)?;
    let mut key_offset = 0_usize;
    let mut remaining = usize::from(key_bit_len);
    for _ in 0..=boc.cells.len() {
        cell_index = ton_virtual_root_index(boc, cell_index)?;
        let cell = boc.cells.get(cell_index)?;
        (ton_cell_type(cell)? == TonCellType::Ordinary).then_some(())?;
        let mut reader = TonBitReader::new(cell)?;
        let label = ton_read_hashmap_label(&mut reader, key, key_bit_len, key_offset, remaining)?;
        key_offset = key_offset.checked_add(label)?;
        remaining = remaining.checked_sub(label)?;
        if remaining == 0 {
            if reader.remaining_bits()? != 0 || reader.remaining_refs()? != 1 {
                return None;
            }
            return ton_virtual_root_index(boc, reader.read_ref()?);
        }
        if reader.remaining_bits()? != 0 || reader.remaining_refs()? != 2 {
            return None;
        }
        let branch = ton_key_bit(key, key_bit_len, key_offset)?;
        key_offset += 1;
        remaining -= 1;
        let left = reader.read_ref()?;
        let right = reader.read_ref()?;
        cell_index = if branch { right } else { left };
    }
    None
}

fn ton_hashmap_aug_leaf_reader<'a>(
    boc: &'a TonBoc,
    root: usize,
    key: &[u8],
    key_bit_len: u16,
    skip_extra: fn(&mut TonBitReader<'_>) -> Option<()>,
) -> Option<TonBitReader<'a>> {
    let mut cell_index = ton_virtual_root_index(boc, root)?;
    let mut key_offset = 0_usize;
    let mut remaining = usize::from(key_bit_len);
    for _ in 0..=boc.cells.len() {
        cell_index = ton_virtual_root_index(boc, cell_index)?;
        let cell = boc.cells.get(cell_index)?;
        (ton_cell_type(cell)? == TonCellType::Ordinary).then_some(())?;
        let mut reader = TonBitReader::new(cell)?;
        let label = ton_read_hashmap_label(&mut reader, key, key_bit_len, key_offset, remaining)?;
        key_offset += label;
        remaining -= label;
        if remaining == 0 {
            return Some(reader);
        }
        if reader.remaining_refs()? < 2 {
            return None;
        }
        let branch = ton_key_bit(key, key_bit_len, key_offset)?;
        key_offset += 1;
        remaining -= 1;
        let left = reader.read_ref()?;
        let right = reader.read_ref()?;
        skip_extra(&mut reader)?;
        if !reader.exhausted() {
            return None;
        }
        cell_index = if branch { right } else { left };
    }
    None
}

fn ton_skip_var_uint(reader: &mut TonBitReader<'_>, length_bits: usize) -> Option<()> {
    let byte_len = reader.read_usize(length_bits)?;
    reader.skip_bits(byte_len.checked_mul(8)?)
}

fn ton_skip_grams(reader: &mut TonBitReader<'_>) -> Option<()> {
    ton_skip_var_uint(reader, 4)
}

fn ton_skip_currency_collection(reader: &mut TonBitReader<'_>) -> Option<()> {
    ton_skip_grams(reader)?;
    if reader.read_bit()? {
        reader.read_ref()?;
    }
    Some(())
}

fn ton_skip_storage_used(reader: &mut TonBitReader<'_>) -> Option<()> {
    ton_skip_var_uint(reader, 3)?;
    ton_skip_var_uint(reader, 3)
}

fn ton_read_shard_ident(reader: &mut TonBitReader<'_>) -> Option<(i32, u64)> {
    if reader.read_u64(2)? != 0 {
        return None;
    }
    let prefix_bits = reader.read_usize(6)?;
    if prefix_bits > 60 {
        return None;
    }
    let workchain = reader.read_i32(32)?;
    let shard = reader.read_u64(64)?;
    let terminator = shard.trailing_zeros();
    if shard == 0 || terminator != 63_u32.checked_sub(u32::try_from(prefix_bits).ok()?)? {
        return None;
    }
    Some((workchain, shard))
}

fn ton_parse_ext_block_ref(
    boc: &TonBoc,
    cell_index: usize,
    workchain: i32,
    shard: u64,
) -> Option<TonBlockIdExtV1> {
    let index = ton_virtual_root_index(boc, cell_index)?;
    let cell = boc.cells.get(index)?;
    (ton_cell_type(cell)? == TonCellType::Ordinary).then_some(())?;
    let mut reader = TonBitReader::new(cell)?;
    reader.read_u64(64)?; // end_lt
    let seqno = u32::try_from(reader.read_u64(32)?).ok()?;
    let root_hash = reader.read_h256()?;
    let file_hash = reader.read_h256()?;
    reader.exhausted().then_some(TonBlockIdExtV1 {
        workchain,
        shard,
        seqno,
        root_hash,
        file_hash,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TonParsedBlockInfo {
    not_master: bool,
    after_merge: bool,
    before_split: bool,
    key_block: bool,
    seqno: u32,
    workchain: i32,
    shard: u64,
    gen_utime: u32,
    validator_list_hash_short: u32,
    catchain_seqno: u32,
    min_ref_mc_seqno: u32,
    previous: Option<TonBlockIdExtV1>,
    master_ref: Option<TonBlockIdExtV1>,
}

fn ton_parse_block_info(boc: &TonBoc, cell_index: usize) -> Option<TonParsedBlockInfo> {
    let index = ton_virtual_root_index(boc, cell_index)?;
    let cell = boc.cells.get(index)?;
    (ton_cell_type(cell)? == TonCellType::Ordinary).then_some(())?;
    let mut reader = TonBitReader::new(cell)?;
    if u32::try_from(reader.read_u64(32)?).ok()? != TON_BLOCK_INFO_CONSTRUCTOR {
        return None;
    }
    reader.read_u64(32)?; // version
    let not_master = reader.read_bit()?;
    let after_merge = reader.read_bit()?;
    let before_split = reader.read_bit()?;
    reader.read_bit()?; // after_split
    reader.read_bit()?; // want_split
    reader.read_bit()?; // want_merge
    let key_block = reader.read_bit()?;
    let vert_seqno_incr = reader.read_bit()?;
    let flags = u8::try_from(reader.read_u64(8)?).ok()?;
    if flags > 1 || (key_block && not_master) {
        return None;
    }
    let seqno = u32::try_from(reader.read_u64(32)?).ok()?;
    let vert_seqno = u32::try_from(reader.read_u64(32)?).ok()?;
    if vert_seqno < u32::from(vert_seqno_incr) {
        return None;
    }
    let (workchain, shard) = ton_read_shard_ident(&mut reader)?;
    let gen_utime = u32::try_from(reader.read_u64(32)?).ok()?;
    let start_lt = reader.read_u64(64)?;
    let end_lt = reader.read_u64(64)?;
    if seqno == 0 || start_lt >= end_lt {
        return None;
    }
    let validator_list_hash_short = u32::try_from(reader.read_u64(32)?).ok()?;
    let catchain_seqno = u32::try_from(reader.read_u64(32)?).ok()?;
    let min_ref_mc_seqno = u32::try_from(reader.read_u64(32)?).ok()?;
    reader.read_u64(32)?; // prev_key_block_seqno
    if flags & 1 != 0 {
        reader.read_u64(32)?; // global version
        reader.read_u64(64)?; // capabilities
    }
    let master_ref_index = if not_master {
        Some(reader.read_ref()?)
    } else {
        None
    };
    let previous_ref_index = reader.read_ref()?;
    if vert_seqno_incr {
        reader.read_ref()?;
    }
    if !reader.exhausted() {
        return None;
    }
    let previous = if after_merge {
        // A merged shard has two predecessors; SCCP never relies on a single
        // ambiguous predecessor for masterchain replay.
        None
    } else {
        Some(ton_parse_ext_block_ref(
            boc,
            previous_ref_index,
            workchain,
            shard,
        )?)
    };
    let master_ref = match master_ref_index {
        Some(reference) => Some(ton_parse_ext_block_ref(
            boc,
            reference,
            SCCP_TON_MASTERCHAIN_WORKCHAIN_V1,
            SCCP_TON_MASTERCHAIN_SHARD_V1,
        )?),
        None => None,
    };
    Some(TonParsedBlockInfo {
        not_master,
        after_merge,
        before_split,
        key_block,
        seqno,
        workchain,
        shard,
        gen_utime,
        validator_list_hash_short,
        catchain_seqno,
        min_ref_mc_seqno,
        previous,
        master_ref,
    })
}

#[derive(Debug)]
struct TonParsedBlock {
    global_id: i32,
    info: TonParsedBlockInfo,
    old_state_hash: H256,
    new_state_hash: H256,
    extra_index: usize,
}

fn ton_parse_block(
    boc: &TonBoc,
    computed: &[TonComputedCell],
    root: usize,
) -> Option<TonParsedBlock> {
    let index = ton_virtual_root_index(boc, root)?;
    let cell = boc.cells.get(index)?;
    (ton_cell_type(cell)? == TonCellType::Ordinary).then_some(())?;
    let mut reader = TonBitReader::new(cell)?;
    if u32::try_from(reader.read_u64(32)?).ok()? != TON_BLOCK_CONSTRUCTOR {
        return None;
    }
    let global_id = reader.read_i32(32)?;
    let info_index = reader.read_ref()?;
    reader.read_ref()?; // value_flow
    let state_update_index = reader.read_ref()?;
    let extra_index = reader.read_ref()?;
    if !reader.exhausted() {
        return None;
    }
    let state_update = boc.cells.get(state_update_index)?;
    if ton_cell_type(state_update)? != TonCellType::MerkleUpdate
        || state_update.data.len() != 69
        || state_update.refs.len() != 2
        || state_update.data.first().copied()? != 4
    {
        return None;
    }
    // Cell-hash evaluation already checked both embedded hashes/depths against
    // the referenced old/new state cells.
    computed.get(state_update_index)?;
    Some(TonParsedBlock {
        global_id,
        info: ton_parse_block_info(boc, info_index)?,
        old_state_hash: state_update.data.get(1..33)?.try_into().ok()?,
        new_state_hash: state_update.data.get(33..65)?.try_into().ok()?,
        extra_index,
    })
}

#[derive(Clone, Copy, Debug)]
struct TonMasterchainExtra {
    shard_hashes_root: Option<usize>,
    config_dictionary_root: Option<usize>,
}

fn ton_parse_masterchain_extra(
    boc: &TonBoc,
    block_extra_index: usize,
) -> Option<TonMasterchainExtra> {
    let extra_index = ton_virtual_root_index(boc, block_extra_index)?;
    let extra_cell = boc.cells.get(extra_index)?;
    (ton_cell_type(extra_cell)? == TonCellType::Ordinary).then_some(())?;
    let mut extra = TonBitReader::new(extra_cell)?;
    extra.read_ref()?; // in_msg_descr
    extra.read_ref()?; // out_msg_descr
    extra.read_ref()?; // account_blocks
    extra.skip_bits(512)?; // rand_seed, created_by
    if !extra.read_bit()? {
        return None;
    }
    let custom_index = extra.read_ref()?;
    if !extra.exhausted() {
        return None;
    }
    let custom_index = ton_virtual_root_index(boc, custom_index)?;
    let custom_cell = boc.cells.get(custom_index)?;
    (ton_cell_type(custom_cell)? == TonCellType::Ordinary).then_some(())?;
    let mut custom = TonBitReader::new(custom_cell)?;
    if u16::try_from(custom.read_u64(16)?).ok()? != TON_MC_BLOCK_EXTRA_CONSTRUCTOR {
        return None;
    }
    let key_block = custom.read_bit()?;
    let shard_hashes_root = if custom.read_bit()? {
        Some(custom.read_ref()?)
    } else {
        None
    };
    if custom.read_bit()? {
        custom.read_ref()?; // shard-fees HashmapAug root
    }
    // HashmapAugE carries its aggregate even when empty.
    ton_skip_currency_collection(&mut custom)?;
    ton_skip_currency_collection(&mut custom)?;
    custom.read_ref()?; // previous signatures/recover/mint auxiliary cell
    let config_dictionary_root = if key_block {
        custom.read_h256()?; // config contract address
        Some(custom.read_ref()?)
    } else {
        None
    };
    custom.exhausted().then_some(TonMasterchainExtra {
        shard_hashes_root,
        config_dictionary_root,
    })
}

fn ton_read_validator_descr(reader: &mut TonBitReader<'_>) -> Option<TonValidatorV1> {
    let constructor = u8::try_from(reader.read_u64(8)?).ok()?;
    if !matches!(
        constructor,
        TON_VALIDATOR_CONSTRUCTOR | TON_VALIDATOR_ADDR_CONSTRUCTOR
    ) {
        return None;
    }
    if u32::try_from(reader.read_u64(32)?).ok()? != TON_ED25519_PUBKEY_TLB_CONSTRUCTOR {
        return None;
    }
    let public_key = reader.read_h256()?;
    let weight = reader.read_u64(64)?;
    if weight == 0 {
        return None;
    }
    let adnl_address = if constructor == TON_VALIDATOR_ADDR_CONSTRUCTOR {
        reader.read_h256()?
    } else {
        [0_u8; 32]
    };
    Some(TonValidatorV1 {
        public_key,
        weight,
        adnl_address,
    })
}

fn bits_to_u16(bits: &[bool]) -> Option<u16> {
    if bits.len() > 16 {
        return None;
    }
    let mut value = 0_u16;
    for bit in bits {
        value = value.checked_shl(1)?;
        if *bit {
            value = value.checked_add(1)?;
        }
    }
    Some(value)
}

fn ton_collect_validator_edge(
    boc: &TonBoc,
    reader: &mut TonBitReader<'_>,
    remaining: usize,
    prefix: &mut Vec<bool>,
    output: &mut Vec<(u16, TonValidatorV1)>,
    budget: &mut usize,
) -> Option<()> {
    if *budget == 0 || output.len() >= TON_MAX_VALIDATORS {
        return None;
    }
    *budget -= 1;
    let label = ton_read_hashmap_label_bits(reader, remaining)?;
    let label_len = label.len();
    prefix.extend(label);
    let remaining = remaining.checked_sub(label_len)?;
    if remaining == 0 {
        let key = bits_to_u16(prefix)?;
        let validator = ton_read_validator_descr(reader)?;
        if !reader.exhausted() {
            return None;
        }
        output.push((key, validator));
        prefix.truncate(prefix.len().checked_sub(label_len)?);
        return Some(());
    }
    if reader.remaining_bits()? != 0 || reader.remaining_refs()? != 2 {
        return None;
    }
    let left = reader.read_ref()?;
    let right = reader.read_ref()?;
    for (bit, child) in [(false, left), (true, right)] {
        let child = ton_virtual_root_index(boc, child)?;
        let cell = boc.cells.get(child)?;
        (ton_cell_type(cell)? == TonCellType::Ordinary).then_some(())?;
        let mut child_reader = TonBitReader::new(cell)?;
        prefix.push(bit);
        ton_collect_validator_edge(
            boc,
            &mut child_reader,
            remaining.checked_sub(1)?,
            prefix,
            output,
            budget,
        )?;
        prefix.pop();
    }
    prefix.truncate(prefix.len().checked_sub(label_len)?);
    Some(())
}

fn ton_parse_validator_config(boc: &TonBoc, cell_index: usize) -> Option<TonValidatorConfigV1> {
    let cell_index = ton_virtual_root_index(boc, cell_index)?;
    let cell = boc.cells.get(cell_index)?;
    (ton_cell_type(cell)? == TonCellType::Ordinary).then_some(())?;
    let mut reader = TonBitReader::new(cell)?;
    let constructor = u8::try_from(reader.read_u64(8)?).ok()?;
    if !matches!(
        constructor,
        TON_VALIDATORS_CONSTRUCTOR | TON_VALIDATORS_EXT_CONSTRUCTOR
    ) {
        return None;
    }
    let valid_since = u32::try_from(reader.read_u64(32)?).ok()?;
    let valid_until = u32::try_from(reader.read_u64(32)?).ok()?;
    let total = u16::try_from(reader.read_u64(16)?).ok()?;
    let main_validator_count = u16::try_from(reader.read_u64(16)?).ok()?;
    if valid_since >= valid_until
        || total == 0
        || usize::from(total) > TON_MAX_VALIDATORS
        || main_validator_count == 0
        || main_validator_count > total
    {
        return None;
    }
    let declared_total_weight = if constructor == TON_VALIDATORS_EXT_CONSTRUCTOR {
        Some(reader.read_u64(64)?)
    } else {
        None
    };
    let mut validators = Vec::with_capacity(usize::from(total));
    let mut prefix = Vec::with_capacity(16);
    let mut budget = boc.cells.len().checked_add(1)?;
    if constructor == TON_VALIDATORS_EXT_CONSTRUCTOR {
        if !reader.read_bit()? || reader.remaining_bits()? != 0 || reader.remaining_refs()? != 1 {
            return None;
        }
        let root = ton_virtual_root_index(boc, reader.read_ref()?)?;
        let root_cell = boc.cells.get(root)?;
        let mut root_reader = TonBitReader::new(root_cell)?;
        ton_collect_validator_edge(
            boc,
            &mut root_reader,
            usize::from(TON_VALIDATOR_SET_KEY_BITS),
            &mut prefix,
            &mut validators,
            &mut budget,
        )?;
    } else {
        ton_collect_validator_edge(
            boc,
            &mut reader,
            usize::from(TON_VALIDATOR_SET_KEY_BITS),
            &mut prefix,
            &mut validators,
            &mut budget,
        )?;
    }
    validators.sort_by_key(|(key, _)| *key);
    if validators.len() != usize::from(total)
        || validators
            .iter()
            .enumerate()
            .any(|(index, (key, _))| usize::from(*key) != index)
    {
        return None;
    }
    let validators = validators
        .into_iter()
        .map(|(_, validator)| validator)
        .collect::<Vec<_>>();
    let total_weight = validate_validator_roster(&validators)?;
    if declared_total_weight.is_some_and(|declared| declared != total_weight) {
        return None;
    }
    Some(TonValidatorConfigV1 {
        valid_since,
        valid_until,
        main_validator_count,
        shuffle_masterchain_validators: false,
        validators,
    })
}

fn ton_parse_catchain_shuffle(boc: &TonBoc, cell_index: usize) -> Option<bool> {
    let cell_index = ton_virtual_root_index(boc, cell_index)?;
    let cell = boc.cells.get(cell_index)?;
    (ton_cell_type(cell)? == TonCellType::Ordinary).then_some(())?;
    let mut reader = TonBitReader::new(cell)?;
    let constructor = u8::try_from(reader.read_u64(8)?).ok()?;
    let shuffle = match constructor {
        TON_CATCHAIN_CONFIG_CONSTRUCTOR => false,
        TON_CATCHAIN_CONFIG_NEW_CONSTRUCTOR => {
            if reader.read_u64(7)? != 0 {
                return None;
            }
            reader.read_bit()?
        }
        _ => return None,
    };
    for _ in 0..4 {
        if reader.read_u64(32)? == 0 {
            return None;
        }
    }
    reader.exhausted().then_some(shuffle)
}

fn ton_config_from_dictionary(boc: &TonBoc, root: usize) -> Option<TonValidatorConfigV1> {
    let validators_cell = ton_hashmap_ref_value(
        boc,
        root,
        &TON_CONFIG_CURRENT_VALIDATORS.to_be_bytes(),
        TON_CONFIG_KEY_BITS,
    )?;
    let catchain_cell = ton_hashmap_ref_value(
        boc,
        root,
        &TON_CONFIG_CATCHAIN.to_be_bytes(),
        TON_CONFIG_KEY_BITS,
    )?;
    let mut config = ton_parse_validator_config(boc, validators_cell)?;
    config.shuffle_masterchain_validators = ton_parse_catchain_shuffle(boc, catchain_cell)?;
    Some(config)
}

struct TonValidatorPrng {
    seed: H256,
    shard: u64,
    workchain: i32,
    catchain_seqno: u32,
    block: [u8; 64],
    position: usize,
}

impl TonValidatorPrng {
    fn masterchain(catchain_seqno: u32) -> Self {
        Self {
            seed: [0_u8; 32],
            shard: SCCP_TON_MASTERCHAIN_SHARD_V1,
            workchain: SCCP_TON_MASTERCHAIN_WORKCHAIN_V1,
            catchain_seqno,
            block: [0_u8; 64],
            position: 8,
        }
    }

    fn increment_seed(&mut self) {
        for byte in self.seed.iter_mut().rev() {
            let (next, carry) = byte.overflowing_add(1);
            *byte = next;
            if !carry {
                break;
            }
        }
    }

    fn next_u64(&mut self) -> u64 {
        if self.position >= 8 {
            let mut input = [0_u8; 48];
            input[..32].copy_from_slice(&self.seed);
            input[32..40].copy_from_slice(&self.shard.to_be_bytes());
            input[40..44].copy_from_slice(&self.workchain.to_be_bytes());
            input[44..48].copy_from_slice(&self.catchain_seqno.to_be_bytes());
            self.block.copy_from_slice(&Sha512::digest(input));
            self.increment_seed();
            self.position = 0;
        }
        let start = self.position * 8;
        self.position += 1;
        u64::from_be_bytes(
            self.block[start..start + 8]
                .try_into()
                .expect("fixed SHA-512 chunk"),
        )
    }

    fn next_ranged(&mut self, range: u64) -> u64 {
        u64::try_from((u128::from(range) * u128::from(self.next_u64())) >> 64)
            .expect("high half of a u64 product fits in u64")
    }
}

fn ton_select_masterchain_validator_set(
    config: &TonValidatorConfigV1,
    catchain_seqno: u32,
) -> Option<TonValidatorSetV1> {
    validate_validator_config(config)?;
    let count = usize::from(config.main_validator_count).min(config.validators.len());
    let validators = if config.shuffle_masterchain_validators {
        let mut indices = vec![0_usize; count];
        let mut prng = TonValidatorPrng::masterchain(catchain_seqno);
        for index in 0..count {
            let selected =
                usize::try_from(prng.next_ranged(u64::try_from(index + 1).ok()?)).ok()?;
            indices[index] = indices[selected];
            indices[selected] = index;
        }
        indices
            .into_iter()
            .map(|index| config.validators.get(index).cloned())
            .collect::<Option<Vec<_>>>()?
    } else {
        config.validators.get(..count)?.to_vec()
    };
    let validator_list_hash_short =
        ton_validator_list_hash_short_from_validated(catchain_seqno, &validators)?;
    Some(TonValidatorSetV1 {
        catchain_seqno,
        validator_list_hash_short,
        validators,
    })
}

fn ton_finality_signature_shape(
    signatures: &TonBlockSignaturesV1,
) -> Result<&[TonValidatorSignatureV1], TonNativeSourceError> {
    let entries = match signatures {
        TonBlockSignaturesV1::Ordinary(proof) => proof.signatures.as_slice(),
        TonBlockSignaturesV1::Simplex(proof) => {
            if proof.slot > u32::MAX >> 1
                || proof.candidate_data.is_empty()
                || proof.candidate_data.len() > 4 * 1024
            {
                return Err(TonNativeSourceError::InvalidSimplexTranscript);
            }
            proof.signatures.as_slice()
        }
    };
    if entries.is_empty() || entries.len() > TON_MAX_SIGNATURES {
        return Err(TonNativeSourceError::ResourceLimit);
    }
    if entries.iter().any(|entry| entry.signature.len() != 64) {
        return Err(TonNativeSourceError::InvalidSignatures);
    }
    Ok(entries)
}

/// Return a cryptography-free work bound for a native TON finality proof.
///
/// # Errors
///
/// Returns a fail-closed [`TonNativeSourceError`] when a block, BoC,
/// signature, candidate transcript, or governed roster exceeds its V1 bound.
pub fn ton_native_finality_work_estimate(
    proof: &TonNativeFinalityProofV1,
) -> Result<TonNativeFinalityWorkEstimateV1, TonNativeSourceError> {
    let block_count = proof.blocks.len();
    if block_count == 0 || block_count > TON_MAX_MASTERCHAIN_BLOCKS {
        return Err(TonNativeSourceError::ResourceLimit);
    }
    let active_validators = proof.anchor.active_validator_set.validators.len();
    if active_validators == 0 || active_validators > TON_MAX_VALIDATORS {
        return Err(TonNativeSourceError::InvalidValidatorSet);
    }
    let pending_validators = match &proof.anchor.pending_validator_config {
        Some(config) => {
            let count = config.validators.len();
            if count == 0
                || count > TON_MAX_VALIDATORS
                || config.main_validator_count == 0
                || usize::from(config.main_validator_count) > count
            {
                return Err(TonNativeSourceError::InvalidValidatorTransition);
            }
            count
        }
        None => 0,
    };
    let mut boc_bytes = 0_usize;
    let mut signature_checks = 0_usize;
    for block in &proof.blocks {
        if block.block_proof_boc.is_empty() || block.block_proof_boc.len() > TON_MAX_BOC_BYTES {
            return Err(TonNativeSourceError::ResourceLimit);
        }
        boc_bytes = boc_bytes
            .checked_add(block.block_proof_boc.len())
            .ok_or(TonNativeSourceError::ResourceLimit)?;
        signature_checks = signature_checks
            .checked_add(ton_finality_signature_shape(&block.signatures)?.len())
            .ok_or(TonNativeSourceError::ResourceLimit)?;
    }
    let continuation_blocks =
        u16::try_from(block_count).map_err(|_| TonNativeSourceError::ResourceLimit)?;
    let framed_boc_bytes =
        u32::try_from(boc_bytes).map_err(|_| TonNativeSourceError::ResourceLimit)?;
    let ed25519_signature_checks =
        u32::try_from(signature_checks).map_err(|_| TonNativeSourceError::ResourceLimit)?;
    // Anchor validation checks its active and pending sets once. Each
    // continuation can additionally validate one current subset, one pending
    // transition roster, and one newly authenticated key-block roster.
    let validator_key_checks_upper_bound = block_count
        .checked_mul(3)
        .and_then(|count| count.checked_mul(TON_MAX_VALIDATORS))
        .and_then(|count| count.checked_add(active_validators))
        .and_then(|count| count.checked_add(pending_validators))
        .and_then(|count| u32::try_from(count).ok())
        .ok_or(TonNativeSourceError::ResourceLimit)?;
    Ok(TonNativeFinalityWorkEstimateV1 {
        continuation_blocks,
        framed_boc_bytes,
        ed25519_signature_checks,
        validator_key_checks_upper_bound,
    })
}

/// Return a cryptography-free work bound for a complete native TON source proof.
///
/// This extends [`ton_native_finality_work_estimate`] with the shard-block and
/// post-state BoCs opened after masterchain finality.
///
/// # Errors
///
/// Returns a fail-closed [`TonNativeSourceError`] when any count or byte bound
/// needed before native parsing or cryptography is exceeded.
pub fn ton_native_source_work_estimate(
    proof: &TonNativeSourceProofV1,
) -> Result<TonNativeFinalityWorkEstimateV1, TonNativeSourceError> {
    let mut estimate = ton_native_finality_work_estimate(&proof.finality)?;
    for boc in [
        proof.event.shard_block_proof_boc.as_slice(),
        proof.event.transaction_pre_state_proof_boc.as_slice(),
        proof.event.shard_state_proof_boc.as_slice(),
    ] {
        if boc.is_empty() || boc.len() > TON_MAX_BOC_BYTES {
            return Err(TonNativeSourceError::ResourceLimit);
        }
        estimate.framed_boc_bytes = estimate
            .framed_boc_bytes
            .checked_add(u32::try_from(boc.len()).map_err(|_| TonNativeSourceError::ResourceLimit)?)
            .ok_or(TonNativeSourceError::ResourceLimit)?;
    }
    Ok(estimate)
}

/// Return the cheap deterministic native-work reservation for one breaker proof.
///
/// This function examines only typed byte/vector lengths. It performs no BoC
/// parsing, hashing, key parsing, or signature verification, so consensus can
/// charge the complete attacker-controlled envelope before native dispatch.
///
/// # Errors
///
/// Returns [`TonNativeSourceError::ResourceLimit`] when any bounded BoC or
/// aggregate counter exceeds its final-V1 limit.
pub fn ton_breaker_observation_work_estimate(
    proof: &SccpTonBreakerObservationProofV1,
) -> Result<TonNativeFinalityWorkEstimateV1, TonNativeSourceError> {
    let mut estimate = ton_native_finality_work_estimate(&proof.finality)?;
    for boc in [
        proof.route_account.shard_block_proof_boc.as_slice(),
        proof.route_account.shard_state_proof_boc.as_slice(),
        proof.jetton_master_account.shard_block_proof_boc.as_slice(),
        proof.jetton_master_account.shard_state_proof_boc.as_slice(),
    ] {
        if boc.is_empty() || boc.len() > TON_MAX_BOC_BYTES {
            return Err(TonNativeSourceError::ResourceLimit);
        }
        estimate.framed_boc_bytes = estimate
            .framed_boc_bytes
            .checked_add(u32::try_from(boc.len()).map_err(|_| TonNativeSourceError::ResourceLimit)?)
            .ok_or(TonNativeSourceError::ResourceLimit)?;
    }
    Ok(estimate)
}

#[derive(Debug)]
struct VerifiedMasterchain {
    block_id: TonBlockIdExtV1,
    gen_utime: u32,
    boc: TonBoc,
    extra: TonMasterchainExtra,
    chain_ids: Vec<TonBlockIdExtV1>,
}

fn verify_masterchain_finality(
    proof: &TonNativeFinalityProofV1,
    expected_network: SccpNetworkV1,
    expected_anchor_hash: H256,
) -> Result<VerifiedMasterchain, TonNativeSourceError> {
    if proof.version != 1 || proof.anchor.version != 1 {
        return Err(TonNativeSourceError::UnsupportedVersion);
    }
    let _ = ton_native_finality_work_estimate(proof)?;
    if proof.anchor.network != expected_network {
        return Err(TonNativeSourceError::WrongNetwork);
    }
    validate_ton_native_anchor(&proof.anchor).ok_or(TonNativeSourceError::InvalidAnchor)?;
    if ton_native_anchor_hash_from_validated(&proof.anchor) != Some(expected_anchor_hash) {
        return Err(TonNativeSourceError::AnchorHashMismatch);
    }
    if proof.blocks.is_empty() || proof.blocks.len() > TON_MAX_MASTERCHAIN_BLOCKS {
        return Err(TonNativeSourceError::ResourceLimit);
    }
    let global_id =
        ton_network_global_id(expected_network).ok_or(TonNativeSourceError::WrongNetwork)?;
    let mut previous = proof.anchor.checkpoint;
    let mut state_root = proof.anchor.checkpoint_state_root;
    let mut active = proof.anchor.active_validator_set.clone();
    let mut pending = proof.anchor.pending_validator_config.clone();
    let mut last = None;
    let mut chain_ids = vec![proof.anchor.checkpoint];
    for block_proof in &proof.blocks {
        if block_proof.block_proof_boc.is_empty()
            || block_proof.block_proof_boc.len() > TON_MAX_BOC_BYTES
            || block_proof.block_id.workchain != SCCP_TON_MASTERCHAIN_WORKCHAIN_V1
            || block_proof.block_id.shard != SCCP_TON_MASTERCHAIN_SHARD_V1
            || block_proof.block_id.seqno
                != previous
                    .seqno
                    .checked_add(1)
                    .ok_or(TonNativeSourceError::BrokenMasterchainLink)?
        {
            return Err(TonNativeSourceError::ResourceLimit);
        }
        let (boc, computed, root) = parse_single_root_boc(&block_proof.block_proof_boc)
            .ok_or(TonNativeSourceError::InvalidBoc)?;
        if ton_proven_root_hash(&boc, &computed, root) != Some(block_proof.block_id.root_hash) {
            return Err(TonNativeSourceError::InvalidBoc);
        }
        let parsed =
            ton_parse_block(&boc, &computed, root).ok_or(TonNativeSourceError::InvalidBoc)?;
        if parsed.global_id != global_id
            || parsed.info.not_master
            || parsed.info.after_merge
            || parsed.info.workchain != block_proof.block_id.workchain
            || parsed.info.shard != block_proof.block_id.shard
            || parsed.info.seqno != block_proof.block_id.seqno
            || parsed.info.previous != Some(previous)
            || parsed.old_state_hash != state_root
        {
            return Err(TonNativeSourceError::BrokenMasterchainLink);
        }
        if active.catchain_seqno != parsed.info.catchain_seqno
            || active.validator_list_hash_short != parsed.info.validator_list_hash_short
        {
            if active.catchain_seqno.checked_add(1) != Some(parsed.info.catchain_seqno) {
                // Without exact monotonicity an attacker could search arbitrary
                // shuffle seeds for a config-34 subset favorable to its stake.
                return Err(TonNativeSourceError::InvalidValidatorTransition);
            }
            let config = pending
                .as_ref()
                .ok_or(TonNativeSourceError::InvalidValidatorTransition)?;
            if parsed.info.gen_utime < config.valid_since
                || parsed.info.gen_utime >= config.valid_until
            {
                return Err(TonNativeSourceError::InvalidValidatorTransition);
            }
            active = ton_select_masterchain_validator_set(config, parsed.info.catchain_seqno)
                .ok_or(TonNativeSourceError::InvalidValidatorTransition)?;
            if active.validator_list_hash_short != parsed.info.validator_list_hash_short {
                return Err(TonNativeSourceError::InvalidValidatorTransition);
            }
        }
        verify_block_signatures(block_proof.block_id, &active, &block_proof.signatures)?;
        let extra = ton_parse_masterchain_extra(&boc, parsed.extra_index)
            .ok_or(TonNativeSourceError::InvalidBoc)?;
        if parsed.info.key_block {
            let dictionary = extra
                .config_dictionary_root
                .ok_or(TonNativeSourceError::InvalidValidatorTransition)?;
            pending = Some(
                ton_config_from_dictionary(&boc, dictionary)
                    .ok_or(TonNativeSourceError::InvalidValidatorTransition)?,
            );
        } else if extra.config_dictionary_root.is_some() {
            return Err(TonNativeSourceError::InvalidBoc);
        }
        previous = block_proof.block_id;
        state_root = parsed.new_state_hash;
        chain_ids.push(previous);
        last = Some(VerifiedMasterchain {
            block_id: previous,
            gen_utime: parsed.info.gen_utime,
            boc,
            extra,
            chain_ids: chain_ids.clone(),
        });
    }
    last.ok_or(TonNativeSourceError::ResourceLimit)
}

fn ton_shard_child(shard: u64, right: bool) -> Option<u64> {
    let terminator = shard & shard.wrapping_neg();
    if terminator <= 1 {
        return None;
    }
    let delta = terminator >> 1;
    Some(if right {
        shard.checked_add(delta)?
    } else {
        shard.checked_sub(delta)?
    })
}

fn ton_parse_shard_descriptor(
    reader: &mut TonBitReader<'_>,
    workchain: i32,
    shard: u64,
) -> Option<(TonBlockIdExtV1, u32)> {
    let constructor = u8::try_from(reader.read_u64(4)?).ok()?;
    if !matches!(constructor, 0x0b | 0x0a) {
        return None;
    }
    let seqno = u32::try_from(reader.read_u64(32)?).ok()?;
    let registered_masterchain_seqno = u32::try_from(reader.read_u64(32)?).ok()?;
    let start_lt = reader.read_u64(64)?;
    let end_lt = reader.read_u64(64)?;
    let root_hash = reader.read_h256()?;
    let file_hash = reader.read_h256()?;
    if seqno == 0 || start_lt >= end_lt || !nonzero(&root_hash) || !nonzero(&file_hash) {
        return None;
    }
    Some((
        TonBlockIdExtV1 {
            workchain,
            shard,
            seqno,
            root_hash,
            file_hash,
        },
        registered_masterchain_seqno,
    ))
}

fn ton_select_shard_descriptor(
    boc: &TonBoc,
    shard_hashes_root: usize,
    address: SccpTonAddressV1,
) -> Option<(TonBlockIdExtV1, u32)> {
    let bin_tree =
        ton_hashmap_ref_value(boc, shard_hashes_root, &address.workchain.to_be_bytes(), 32)?;
    let mut cell_index = bin_tree;
    let mut shard = SCCP_TON_MASTERCHAIN_SHARD_V1;
    let mut depth = 0_usize;
    for _ in 0..=60 {
        cell_index = ton_virtual_root_index(boc, cell_index)?;
        let cell = boc.cells.get(cell_index)?;
        (ton_cell_type(cell)? == TonCellType::Ordinary).then_some(())?;
        let mut reader = TonBitReader::new(cell)?;
        if !reader.read_bit()? {
            return ton_parse_shard_descriptor(&mut reader, address.workchain, shard);
        }
        if reader.remaining_bits()? != 0 || reader.remaining_refs()? != 2 {
            return None;
        }
        let left = reader.read_ref()?;
        let right = reader.read_ref()?;
        let go_right = ton_key_bit(&address.account, 256, depth)?;
        shard = ton_shard_child(shard, go_right)?;
        cell_index = if go_right { right } else { left };
        depth += 1;
    }
    None
}

fn ton_parse_block_extra_account_blocks(boc: &TonBoc, extra_index: usize) -> Option<usize> {
    let index = ton_virtual_root_index(boc, extra_index)?;
    let cell = boc.cells.get(index)?;
    (ton_cell_type(cell)? == TonCellType::Ordinary).then_some(())?;
    let mut reader = TonBitReader::new(cell)?;
    reader.read_ref()?; // in_msg_descr
    reader.read_ref()?; // out_msg_descr
    let account_blocks = reader.read_ref()?;
    reader.skip_bits(512)?;
    if reader.read_bit()? {
        reader.read_ref()?;
    }
    reader.exhausted().then_some(account_blocks)
}

fn ton_hashmap_aug_e_root(
    boc: &TonBoc,
    wrapper_index: usize,
    skip_extra: fn(&mut TonBitReader<'_>) -> Option<()>,
) -> Option<usize> {
    let index = ton_virtual_root_index(boc, wrapper_index)?;
    let cell = boc.cells.get(index)?;
    (ton_cell_type(cell)? == TonCellType::Ordinary).then_some(())?;
    let mut reader = TonBitReader::new(cell)?;
    if !reader.read_bit()? {
        skip_extra(&mut reader)?;
        return reader.exhausted().then_some(usize::MAX);
    }
    let root = reader.read_ref()?;
    skip_extra(&mut reader)?;
    reader.exhausted().then_some(root)
}

fn ton_skip_depth_balance(reader: &mut TonBitReader<'_>) -> Option<()> {
    let depth = reader.read_usize(5)?;
    if depth > 30 {
        return None;
    }
    ton_skip_currency_collection(reader)
}

fn ton_hashmap_aug_transaction_ref<'a>(
    boc: &'a TonBoc,
    mut reader: TonBitReader<'a>,
    transaction_lt: u64,
) -> Option<usize> {
    let key = transaction_lt.to_be_bytes();
    let mut key_offset = 0_usize;
    let mut remaining = usize::from(TON_ACCOUNT_TRANSACTION_KEY_BITS);
    for _ in 0..=boc.cells.len() {
        let label = ton_read_hashmap_label(
            &mut reader,
            &key,
            TON_ACCOUNT_TRANSACTION_KEY_BITS,
            key_offset,
            remaining,
        )?;
        key_offset += label;
        remaining -= label;
        if remaining == 0 {
            ton_skip_currency_collection(&mut reader)?;
            let value = ton_virtual_root_index(boc, reader.read_ref()?)?;
            return reader.exhausted().then_some(value);
        }
        if reader.remaining_refs()? < 2 {
            return None;
        }
        let go_right = ton_key_bit(&key, TON_ACCOUNT_TRANSACTION_KEY_BITS, key_offset)?;
        key_offset += 1;
        remaining -= 1;
        let left = reader.read_ref()?;
        let right = reader.read_ref()?;
        let child = ton_virtual_root_index(boc, if go_right { right } else { left })?;
        let child_cell = boc.cells.get(child)?;
        (ton_cell_type(child_cell)? == TonCellType::Ordinary).then_some(())?;
        reader = TonBitReader::new(child_cell)?;
    }
    None
}

fn ton_skip_hashmap_aug_root_node(
    reader: &mut TonBitReader<'_>,
    key_bits: usize,
    skip_extra: fn(&mut TonBitReader<'_>) -> Option<()>,
) -> Option<()> {
    let label = ton_read_hashmap_label_bits(reader, key_bits)?;
    let remaining = key_bits.checked_sub(label.len())?;
    if remaining == 0 {
        skip_extra(reader)?;
        reader.read_ref()?; // leaf value ^Transaction
    } else {
        reader.read_ref()?;
        reader.read_ref()?;
        skip_extra(reader)?;
    }
    Some(())
}

fn ton_transaction_from_account_blocks(
    boc: &TonBoc,
    account_blocks_wrapper: usize,
    account: H256,
    transaction_lt: u64,
) -> Option<usize> {
    let root = ton_hashmap_aug_e_root(boc, account_blocks_wrapper, ton_skip_currency_collection)?;
    if root == usize::MAX {
        return None;
    }
    let mut leaf = ton_hashmap_aug_leaf_reader(
        boc,
        root,
        &account,
        TON_SHARD_ACCOUNT_KEY_BITS,
        ton_skip_currency_collection,
    )?;
    ton_skip_currency_collection(&mut leaf)?; // augmentation before AccountBlock
    if u8::try_from(leaf.read_u64(4)?).ok()? != TON_ACCOUNT_BLOCK_CONSTRUCTOR
        || leaf.read_h256()? != account
    {
        return None;
    }
    let transaction_dictionary = leaf.clone();
    ton_skip_hashmap_aug_root_node(
        &mut leaf,
        usize::from(TON_ACCOUNT_TRANSACTION_KEY_BITS),
        ton_skip_currency_collection,
    )?;
    leaf.read_ref()?; // AccountBlock account-state hash update
    if !leaf.exhausted() {
        return None;
    }
    ton_hashmap_aug_transaction_ref(boc, transaction_dictionary, transaction_lt)
}

#[derive(Clone, Copy, Debug)]
struct TonParsedTransaction {
    hash: H256,
    logical_time: u64,
    previous_logical_time: u64,
    old_account_hash: H256,
    new_account_hash: H256,
    out_message_count: u16,
    auxiliary_index: usize,
    description_index: usize,
}

fn ton_parse_transaction(
    boc: &TonBoc,
    computed: &[TonComputedCell],
    transaction_index: usize,
    expected_account: H256,
    expected_lt: u64,
) -> Option<TonParsedTransaction> {
    let index = ton_virtual_root_index(boc, transaction_index)?;
    let cell = boc.cells.get(index)?;
    (ton_cell_type(cell)? == TonCellType::Ordinary).then_some(())?;
    let mut reader = TonBitReader::new(cell)?;
    if u8::try_from(reader.read_u64(4)?).ok()? != TON_TRANSACTION_CONSTRUCTOR
        || reader.read_h256()? != expected_account
    {
        return None;
    }
    let logical_time = reader.read_u64(64)?;
    if logical_time != expected_lt || logical_time == 0 {
        return None;
    }
    reader.read_h256()?; // previous transaction hash
    let previous_logical_time = reader.read_u64(64)?;
    if previous_logical_time >= logical_time {
        return None;
    }
    reader.read_u64(32)?; // now
    let out_message_count = u16::try_from(reader.read_u64(15)?).ok()?;
    if out_message_count == 0 || out_message_count > 512 {
        return None;
    }
    let original_status = u8::try_from(reader.read_u64(2)?).ok()?;
    let end_status = u8::try_from(reader.read_u64(2)?).ok()?;
    if original_status != 2 || end_status != 2 {
        return None;
    }
    let auxiliary_index = reader.read_ref()?;
    ton_skip_currency_collection(&mut reader)?;
    let state_update = reader.read_ref()?;
    let description_index = reader.read_ref()?;
    if !reader.exhausted() {
        return None;
    }
    let state_update = ton_virtual_root_index(boc, state_update)?;
    let update_cell = boc.cells.get(state_update)?;
    let mut update_reader = TonBitReader::new(update_cell)?;
    if update_reader.read_u64(8)? != 0x72 {
        return None;
    }
    let old_account_hash = update_reader.read_h256()?;
    let new_account_hash = update_reader.read_h256()?;
    if !update_reader.exhausted() || !nonzero(&old_account_hash) || !nonzero(&new_account_hash) {
        return None;
    }
    Some(TonParsedTransaction {
        hash: ton_original_tree_hash(computed, index)?,
        logical_time,
        previous_logical_time,
        old_account_hash,
        new_account_hash,
        out_message_count,
        auxiliary_index,
        description_index,
    })
}

fn ton_skip_account_status_change(reader: &mut TonBitReader<'_>) -> Option<()> {
    if reader.read_bit()? {
        reader.read_bit()?;
    }
    Some(())
}

fn ton_skip_storage_phase(reader: &mut TonBitReader<'_>) -> Option<()> {
    ton_skip_grams(reader)?;
    if reader.read_bit()? {
        ton_skip_grams(reader)?;
    }
    ton_skip_account_status_change(reader)
}

fn ton_skip_credit_phase(reader: &mut TonBitReader<'_>) -> Option<()> {
    if reader.read_bit()? {
        ton_skip_grams(reader)?;
    }
    ton_skip_currency_collection(reader)
}

fn ton_parse_vm_compute_phase(boc: &TonBoc, reader: &mut TonBitReader<'_>) -> Option<bool> {
    if !reader.read_bit()? {
        // Every compute-skipped reason is a failed source event.
        if reader.read_u64(2)? == 3 && reader.read_bit()? {
            return None;
        }
        return Some(false);
    }
    let success = reader.read_bit()?;
    reader.read_bit()?; // msg_state_used
    reader.read_bit()?; // account_activated
    ton_skip_grams(reader)?;
    let details = reader.read_ref()?;
    let details = ton_virtual_root_index(boc, details)?;
    let cell = boc.cells.get(details)?;
    let mut details = TonBitReader::new(cell)?;
    ton_skip_var_uint(&mut details, 3)?;
    ton_skip_var_uint(&mut details, 3)?;
    if details.read_bit()? {
        ton_skip_var_uint(&mut details, 2)?;
    }
    details.read_i32(8)?; // mode
    let exit_code = details.read_i32(32)?;
    if details.read_bit()? {
        details.read_i32(32)?;
    }
    details.read_u64(32)?;
    details.read_h256()?;
    details.read_h256()?;
    if !details.exhausted() {
        return None;
    }
    Some(success && matches!(exit_code, 0 | 1))
}

fn ton_parse_action_phase(
    boc: &TonBoc,
    action_index: usize,
    expected_messages: u16,
) -> Option<bool> {
    let index = ton_virtual_root_index(boc, action_index)?;
    let cell = boc.cells.get(index)?;
    let mut reader = TonBitReader::new(cell)?;
    let success = reader.read_bit()?;
    let valid = reader.read_bit()?;
    let no_funds = reader.read_bit()?;
    ton_skip_account_status_change(&mut reader)?;
    if reader.read_bit()? {
        ton_skip_grams(&mut reader)?;
    }
    if reader.read_bit()? {
        ton_skip_grams(&mut reader)?;
    }
    let result_code = reader.read_i32(32)?;
    if reader.read_bit()? {
        reader.read_i32(32)?;
    }
    let total_actions = u16::try_from(reader.read_u64(16)?).ok()?;
    let special_actions = u16::try_from(reader.read_u64(16)?).ok()?;
    let skipped_actions = u16::try_from(reader.read_u64(16)?).ok()?;
    let messages_created = u16::try_from(reader.read_u64(16)?).ok()?;
    reader.read_h256()?;
    ton_skip_storage_used(&mut reader)?;
    if !reader.exhausted()
        || total_actions < messages_created
        || special_actions > total_actions
        || messages_created != expected_messages
    {
        return None;
    }
    Some(success && valid && !no_funds && result_code == 0 && skipped_actions == 0)
}

fn ton_skip_bounce_phase(reader: &mut TonBitReader<'_>) -> Option<()> {
    if reader.read_bit()? {
        ton_skip_storage_used(reader)?;
        ton_skip_grams(reader)?;
        ton_skip_grams(reader)?;
    } else if reader.read_bit()? {
        ton_skip_storage_used(reader)?;
        ton_skip_grams(reader)?;
    }
    Some(())
}

fn ton_transaction_succeeded(boc: &TonBoc, transaction: TonParsedTransaction) -> Option<bool> {
    let index = ton_virtual_root_index(boc, transaction.description_index)?;
    let cell = boc.cells.get(index)?;
    let mut reader = TonBitReader::new(cell)?;
    if reader.read_u64(4)? != 0 {
        return Some(false);
    }
    reader.read_bit()?; // credit_first
    if reader.read_bit()? {
        ton_skip_storage_phase(&mut reader)?;
    }
    if reader.read_bit()? {
        ton_skip_credit_phase(&mut reader)?;
    }
    let compute_success = ton_parse_vm_compute_phase(boc, &mut reader)?;
    let action_index = if reader.read_bit()? {
        Some(reader.read_ref()?)
    } else {
        None
    };
    let aborted = reader.read_bit()?;
    if reader.read_bit()? {
        ton_skip_bounce_phase(&mut reader)?;
    }
    let destroyed = reader.read_bit()?;
    if !reader.exhausted() {
        return None;
    }
    let action_success = action_index
        .and_then(|action| ton_parse_action_phase(boc, action, transaction.out_message_count))
        .unwrap_or(false);
    Some(compute_success && action_success && !aborted && !destroyed)
}

fn ton_transaction_out_message(
    boc: &TonBoc,
    transaction: TonParsedTransaction,
    message_index: u16,
) -> Option<usize> {
    if message_index >= transaction.out_message_count || message_index >= (1 << 15) {
        return None;
    }
    let auxiliary = ton_virtual_root_index(boc, transaction.auxiliary_index)?;
    let cell = boc.cells.get(auxiliary)?;
    let mut reader = TonBitReader::new(cell)?;
    if reader.read_bit()? {
        reader.read_ref()?;
    }
    if !reader.read_bit()? {
        return None;
    }
    let root = reader.read_ref()?;
    if !reader.exhausted() {
        return None;
    }
    let key = message_index.checked_shl(1)?.to_be_bytes();
    ton_hashmap_ref_value(boc, root, &key, TON_OUT_MESSAGE_KEY_BITS)
}

fn ton_read_internal_address(reader: &mut TonBitReader<'_>) -> Option<SccpTonAddressV1> {
    if !reader.read_bit()? {
        return None;
    }
    let variable = reader.read_bit()?;
    if reader.read_bit()? {
        let depth = reader.read_usize(5)?;
        if depth == 0 || depth > 30 {
            return None;
        }
        reader.skip_bits(depth)?;
    }
    let (workchain, account) = if variable {
        let length = reader.read_usize(9)?;
        if length != 256 {
            return None;
        }
        (reader.read_i32(32)?, reader.read_h256()?)
    } else {
        (reader.read_i32(8)?, reader.read_h256()?)
    };
    Some(SccpTonAddressV1 { workchain, account })
}

fn ton_read_external_none(reader: &mut TonBitReader<'_>) -> Option<()> {
    (reader.read_u64(2)? == 0).then_some(())
}

fn ton_read_exact_payload_cells(
    boc: &TonBoc,
    first: usize,
    expected_len: usize,
) -> Option<Vec<u8>> {
    if !(TON_PAYLOAD_HEADER_BYTES..=TON_MAX_CANONICAL_PAYLOAD_BYTES).contains(&expected_len) {
        return None;
    }
    let mut remaining = expected_len.checked_sub(TON_PAYLOAD_HEADER_BYTES)?;
    let second_len = remaining.min(TON_PAYLOAD_MIDDLE_CHUNK_BYTES);
    remaining = remaining.checked_sub(second_len)?;
    let third_len = remaining.min(TON_PAYLOAD_MIDDLE_CHUNK_BYTES);
    let fourth_len = remaining.checked_sub(third_len)?;
    let first = ton_virtual_root_index(boc, first)?;
    let first_cell = boc.cells.get(first)?;
    if ton_cell_type(first_cell)? != TonCellType::Ordinary
        || first_cell.data_descriptor & 1 != 0
        || first_cell.data.len() != TON_PAYLOAD_HEADER_BYTES
        || first_cell.refs.len() != 1
    {
        return None;
    }
    let second = ton_virtual_root_index(boc, *first_cell.refs.first()?)?;
    let second_cell = boc.cells.get(second)?;
    if ton_cell_type(second_cell)? != TonCellType::Ordinary
        || second_cell.data_descriptor & 1 != 0
        || second_cell.refs.len() != 1
        || second_cell.data.len() != second_len
    {
        return None;
    }
    let third = ton_virtual_root_index(boc, *second_cell.refs.first()?)?;
    let third_cell = boc.cells.get(third)?;
    if ton_cell_type(third_cell)? != TonCellType::Ordinary
        || third_cell.data_descriptor & 1 != 0
        || third_cell.refs.len() != 1
        || third_cell.data.len() != third_len
    {
        return None;
    }
    let fourth = ton_virtual_root_index(boc, *third_cell.refs.first()?)?;
    let fourth_cell = boc.cells.get(fourth)?;
    if ton_cell_type(fourth_cell)? != TonCellType::Ordinary
        || fourth_cell.data_descriptor & 1 != 0
        || !fourth_cell.refs.is_empty()
        || fourth_cell.data.len() != fourth_len
    {
        return None;
    }
    let mut payload = Vec::with_capacity(
        first_cell
            .data
            .len()
            .checked_add(second_cell.data.len())?
            .checked_add(third_cell.data.len())?
            .checked_add(fourth_cell.data.len())?,
    );
    payload.extend_from_slice(&first_cell.data);
    payload.extend_from_slice(&second_cell.data);
    payload.extend_from_slice(&third_cell.data);
    payload.extend_from_slice(&fourth_cell.data);
    Some(payload)
}

fn ton_parse_sccp_event_body(
    boc: &TonBoc,
    mut body: TonBitReader<'_>,
    expected_lane_hash: H256,
    expected_message_id: H256,
    expected_payload_hash: H256,
    expected_event_digest: H256,
    expected_payload: &[u8],
) -> Option<()> {
    if u32::try_from(body.read_u64(32)?).ok()? != TON_SCCP_EVENT_OP_V1
        || u16::try_from(body.read_u64(16)?).ok()? != TON_SCCP_EVENT_VERSION_V1
        || body.read_h256()? != expected_lane_hash
        || body.read_h256()? != expected_message_id
        || body.read_h256()? != expected_payload_hash
        || body.remaining_bits()? != 0
        || body.remaining_refs()? != 1
    {
        return None;
    }
    let tail = ton_virtual_root_index(boc, body.read_ref()?)?;
    let tail_cell = boc.cells.get(tail)?;
    if ton_cell_type(tail_cell)? != TonCellType::Ordinary {
        return None;
    }
    let mut tail = TonBitReader::new(tail_cell)?;
    if tail.read_h256()? != expected_event_digest
        || tail.remaining_bits()? != 0
        || tail.remaining_refs()? != 1
    {
        return None;
    }
    let payload = ton_read_exact_payload_cells(boc, tail.read_ref()?, expected_payload.len())?;
    (payload == expected_payload).then_some(())
}

fn ton_parse_external_event_message(
    boc: &TonBoc,
    computed: &[TonComputedCell],
    message_index: usize,
    emitter: SccpTonAddressV1,
    lane_hash: H256,
    message_id: H256,
    payload_hash: H256,
    event_digest: H256,
    payload: &[u8],
) -> Option<H256> {
    let index = ton_virtual_root_index(boc, message_index)?;
    let cell = boc.cells.get(index)?;
    (ton_cell_type(cell)? == TonCellType::Ordinary).then_some(())?;
    let mut reader = TonBitReader::new(cell)?;
    if reader.read_u64(2)? != 3 || ton_read_internal_address(&mut reader)? != emitter {
        return None;
    }
    ton_read_external_none(&mut reader)?;
    reader.read_u64(64)?; // created_lt
    reader.read_u64(32)?; // created_at
    if reader.read_bit()? {
        // Source-event logs never carry StateInit.
        return None;
    }
    let body_by_ref = reader.read_bit()?;
    if body_by_ref {
        let body_index = reader.read_ref()?;
        if !reader.exhausted() {
            return None;
        }
        let body_index = ton_virtual_root_index(boc, body_index)?;
        let body_cell = boc.cells.get(body_index)?;
        ton_parse_sccp_event_body(
            boc,
            TonBitReader::new(body_cell)?,
            lane_hash,
            message_id,
            payload_hash,
            event_digest,
            payload,
        )?;
    } else {
        ton_parse_sccp_event_body(
            boc,
            reader,
            lane_hash,
            message_id,
            payload_hash,
            event_digest,
            payload,
        )?;
    }
    ton_original_tree_hash(computed, index)
}

fn ton_parse_shard_state_accounts(
    boc: &TonBoc,
    state_root: usize,
    expected_global_id: i32,
    expected_block: TonBlockIdExtV1,
) -> Option<usize> {
    let index = ton_virtual_root_index(boc, state_root)?;
    let cell = boc.cells.get(index)?;
    (ton_cell_type(cell)? == TonCellType::Ordinary).then_some(())?;
    let mut reader = TonBitReader::new(cell)?;
    let constructor = u32::try_from(reader.read_u64(32)?).ok()?;
    if constructor == TON_SPLIT_STATE_CONSTRUCTOR || constructor != TON_SHARD_STATE_CONSTRUCTOR {
        return None;
    }
    if reader.read_i32(32)? != expected_global_id {
        return None;
    }
    let (workchain, shard) = ton_read_shard_ident(&mut reader)?;
    if workchain != expected_block.workchain || shard != expected_block.shard {
        return None;
    }
    if u32::try_from(reader.read_u64(32)?).ok()? != expected_block.seqno {
        return None;
    }
    reader.read_u64(32)?; // vertical seqno
    reader.read_u64(32)?; // generation time
    reader.read_u64(64)?; // generation lt
    reader.read_u64(32)?; // minimum referenced masterchain seqno
    reader.read_ref()?; // outbound queue
    reader.read_bit()?; // before_split
    let accounts = reader.read_ref()?;
    reader.read_ref()?; // balances/libraries/master-ref auxiliary
    if reader.read_bit()? {
        reader.read_ref()?; // masterchain-only custom extra
    }
    reader.exhausted().then_some(accounts)
}

fn ton_skip_storage_extra_info(reader: &mut TonBitReader<'_>) -> Option<()> {
    match reader.read_u64(3)? {
        0 => Some(()),
        1 => {
            reader.read_h256()?;
            Some(())
        }
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TonLastTransactionLtRequirement {
    /// AccountStorage records the previous transaction's end LT, while the
    /// current Transaction records that transaction's start LT in
    /// `prev_trans_lt`. The next transaction may begin at the same LT as the
    /// stored end, but never before it.
    BetweenPreviousAndCurrent {
        previous_start_lt: u64,
        current_start_lt: u64,
    },
    /// AccountStorage records an end LT strictly after the corresponding
    /// ShardAccount/Transaction start LT.
    After(u64),
}

impl TonLastTransactionLtRequirement {
    fn accepts(self, actual: u64) -> bool {
        match self {
            Self::BetweenPreviousAndCurrent {
                previous_start_lt,
                current_start_lt,
            } => {
                (previous_start_lt == 0 && actual == 0 || previous_start_lt < actual)
                    && actual <= current_start_lt
            }
            Self::After(minimum) => actual > minimum,
        }
    }
}

fn ton_read_canonical_var_uint(
    reader: &mut TonBitReader<'_>,
    length_bits: usize,
    maximum_bytes: usize,
) -> Option<u128> {
    let byte_len = reader.read_usize(length_bits)?;
    if byte_len >= maximum_bytes || byte_len > 16 {
        return None;
    }
    if byte_len == 0 {
        return Some(0);
    }
    let first = u8::try_from(reader.read_u64(8)?).ok()?;
    if first == 0 {
        return None;
    }
    let mut value = u128::from(first);
    for _ in 1..byte_len {
        value = value
            .checked_shl(8)?
            .checked_add(u128::from(u8::try_from(reader.read_u64(8)?).ok()?))?;
    }
    Some(value)
}

fn ton_read_canonical_coins(reader: &mut TonBitReader<'_>) -> Option<u128> {
    ton_read_canonical_var_uint(reader, 4, 16)
}

fn ton_skip_canonical_storage_used(reader: &mut TonBitReader<'_>) -> Option<()> {
    ton_read_canonical_var_uint(reader, 3, 7)?;
    ton_read_canonical_var_uint(reader, 3, 7)?;
    Some(())
}

fn ton_skip_canonical_currency_collection(reader: &mut TonBitReader<'_>) -> Option<()> {
    ton_read_canonical_coins(reader)?;
    if reader.read_bit()? {
        // Extra-currency balances do not affect SCCP configuration, but their
        // dictionary root remains authenticated by the enclosing account hash.
        reader.read_ref()?;
    }
    Some(())
}

fn ton_read_canonical_std_address(reader: &mut TonBitReader<'_>) -> Option<SccpTonAddressV1> {
    if !reader.read_bit()? || reader.read_bit()? || reader.read_bit()? {
        // Exactly `addr_std$10`, with no anycast prefix.
        return None;
    }
    let address = SccpTonAddressV1 {
        workchain: reader.read_i32(8)?,
        account: reader.read_h256()?,
    };
    (address.workchain == SCCP_TON_BASECHAIN_WORKCHAIN_V1 && nonzero(&address.account))
        .then_some(address)
}

fn ton_complete_ordinary_cell_bytes(boc: &TonBoc, index: usize) -> Option<Vec<u8>> {
    let index = ton_virtual_root_index(boc, index)?;
    let cell = boc.cells.get(index)?;
    if ton_cell_type(cell)? != TonCellType::Ordinary
        || cell.data_descriptor & 1 != 0
        || !cell.refs.is_empty()
    {
        return None;
    }
    Some(cell.data.clone())
}

fn ton_opaque_ref_hash(boc: &TonBoc, computed: &[TonComputedCell], index: usize) -> Option<H256> {
    ton_opened_original_tree_hash(boc, computed, index).filter(nonzero)
}

fn ton_optional_dictionary_root_hash(
    boc: &TonBoc,
    computed: &[TonComputedCell],
    reader: &mut TonBitReader<'_>,
) -> Option<Option<H256>> {
    if !reader.read_bit()? {
        return Some(None);
    }
    Some(Some(ton_opaque_ref_hash(
        boc,
        computed,
        reader.read_ref()?,
    )?))
}

fn ton_parse_replay_forest_readback(
    boc: &TonBoc,
    computed: &[TonComputedCell],
    index: usize,
) -> Option<TonReplayForestReadbackV1> {
    let index = ton_virtual_root_index(boc, index)?;
    let cell = boc.cells.get(index)?;
    (ton_cell_type(cell)? == TonCellType::Ordinary).then_some(())?;
    let mut reader = TonBitReader::new(cell)?;
    let nonempty_shard_roots_hash = ton_optional_dictionary_root_hash(boc, computed, &mut reader)?;
    let leaf_count = reader.read_u64(64)?;
    let update_sequence = reader.read_u64(64)?;
    if !reader.exhausted()
        || update_sequence != leaf_count
        || (leaf_count == 0) != nonempty_shard_roots_hash.is_none()
    {
        return None;
    }
    Some(TonReplayForestReadbackV1 {
        nonempty_shard_roots_hash,
        leaf_count,
        update_sequence,
    })
}

fn ton_hashmap_ref_entries(
    boc: &TonBoc,
    root: usize,
    key_bits: usize,
) -> Option<BTreeMap<u16, usize>> {
    fn visit(
        boc: &TonBoc,
        index: usize,
        remaining: usize,
        prefix: u16,
        out: &mut BTreeMap<u16, usize>,
        budget: &mut usize,
    ) -> Option<()> {
        *budget = budget.checked_sub(1)?;
        let index = ton_virtual_root_index(boc, index)?;
        let cell = boc.cells.get(index)?;
        (ton_cell_type(cell)? == TonCellType::Ordinary).then_some(())?;
        let mut reader = TonBitReader::new(cell)?;
        let label = ton_read_hashmap_label_bits(&mut reader, remaining)?;
        let mut key = prefix;
        for bit in &label {
            key = key.checked_shl(1)? | u16::from(*bit);
        }
        let remaining = remaining.checked_sub(label.len())?;
        if remaining == 0 {
            if reader.remaining_bits()? != 0 || reader.remaining_refs()? != 1 {
                return None;
            }
            let value = ton_virtual_root_index(boc, reader.read_ref()?)?;
            return out.insert(key, value).is_none().then_some(());
        }
        if reader.remaining_bits()? != 0 || reader.remaining_refs()? != 2 {
            return None;
        }
        let left = reader.read_ref()?;
        let right = reader.read_ref()?;
        visit(boc, left, remaining - 1, key.checked_shl(1)?, out, budget)?;
        visit(
            boc,
            right,
            remaining - 1,
            key.checked_shl(1)?.checked_add(1)?,
            out,
            budget,
        )
    }

    if key_bits == 0 || key_bits > 16 {
        return None;
    }
    let mut out = BTreeMap::new();
    let mut budget = boc.cells.len().checked_add(1)?;
    visit(boc, root, key_bits, 0, &mut out, &mut budget)?;
    Some(out)
}

fn ton_exact_point<const N: usize>(boc: &TonBoc, index: usize) -> Option<[u8; N]> {
    ton_complete_ordinary_cell_bytes(boc, index)?
        .as_slice()
        .try_into()
        .ok()
}

fn ton_parse_verifying_key(
    boc: &TonBoc,
    computed: &[TonComputedCell],
    index: usize,
) -> Option<(SccpGroth16Bls12381VerifyingKeyV1, H256)> {
    let verifying_key_cell_hash = ton_opaque_ref_hash(boc, computed, index)?;
    let index = ton_virtual_root_index(boc, index)?;
    let cell = boc.cells.get(index)?;
    (ton_cell_type(cell)? == TonCellType::Ordinary).then_some(())?;
    let mut reader = TonBitReader::new(cell)?;
    let version = u8::try_from(reader.read_u64(8)?).ok()?;
    let signal_count = u8::try_from(reader.read_u64(8)?).ok()?;
    let alpha = reader.read_ref()?;
    let beta = reader.read_ref()?;
    let gamma = reader.read_ref()?;
    let tail = reader.read_ref()?;
    if version != 1 || signal_count != 11 || !reader.exhausted() {
        return None;
    }
    let tail = ton_virtual_root_index(boc, tail)?;
    let tail_cell = boc.cells.get(tail)?;
    (ton_cell_type(tail_cell)? == TonCellType::Ordinary).then_some(())?;
    let mut tail_reader = TonBitReader::new(tail_cell)?;
    let delta = tail_reader.read_ref()?;
    if !tail_reader.read_bit()? {
        return None;
    }
    let ic_root = tail_reader.read_ref()?;
    if !tail_reader.exhausted() {
        return None;
    }
    let entries = ton_hashmap_ref_entries(boc, ic_root, 8)?;
    if entries.len() != 12 || entries.keys().copied().ne(0_u16..12) {
        return None;
    }
    let mut ic = [[0_u8; 48]; 12];
    for (slot, point) in ic.iter_mut().enumerate() {
        *point = ton_exact_point::<48>(boc, *entries.get(&u16::try_from(slot).ok()?)?)?;
    }
    let key = SccpGroth16Bls12381VerifyingKeyV1 {
        version,
        alpha1: ton_exact_point::<48>(boc, alpha)?,
        beta2: ton_exact_point::<96>(boc, beta)?,
        gamma2: ton_exact_point::<96>(boc, gamma)?,
        delta2: ton_exact_point::<96>(boc, delta)?,
        ic: SccpGroth16Bls12381IcV1 {
            constant: ic[0],
            signal_0: ic[1],
            signal_1: ic[2],
            signal_2: ic[3],
            signal_3: ic[4],
            signal_4: ic[5],
            signal_5: ic[6],
            signal_6: ic[7],
            signal_7: ic[8],
            signal_8: ic[9],
            signal_9: ic[10],
            signal_10: ic[11],
        },
    };
    key.validate_structure().ok()?;
    if !crate::sccp_groth16_bls12381_verifying_key_is_well_formed_v1(&key) {
        return None;
    }
    Some((key, verifying_key_cell_hash))
}

fn ton_parse_mint_breaker_guardians(
    boc: &TonBoc,
    index: usize,
) -> Option<SccpTonMintBreakerGuardianKeysV1> {
    let index = ton_virtual_root_index(boc, index)?;
    let cell = boc.cells.get(index)?;
    (ton_cell_type(cell)? == TonCellType::Ordinary).then_some(())?;
    let mut reader = TonBitReader::new(cell)?;
    let guardian_0 = reader.read_h256()?;
    let guardian_1 = reader.read_h256()?;
    let guardian_2 = reader.read_h256()?;
    let tail = reader.read_ref()?;
    if !reader.exhausted() {
        return None;
    }
    let tail = ton_virtual_root_index(boc, tail)?;
    let tail_cell = boc.cells.get(tail)?;
    (ton_cell_type(tail_cell)? == TonCellType::Ordinary).then_some(())?;
    let mut tail = TonBitReader::new(tail_cell)?;
    let guardian_3 = tail.read_h256()?;
    let guardian_4 = tail.read_h256()?;
    if !tail.exhausted() {
        return None;
    }
    let keys = [guardian_0, guardian_1, guardian_2, guardian_3, guardian_4];
    if keys.iter().any(|key| !nonzero(key)) || keys.windows(2).any(|pair| pair[0] >= pair[1]) {
        return None;
    }
    Some(keys.into())
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TonParsedDeploymentReadbackV1 {
    readback: TonDeploymentReadbackV1,
    bridge_config: TonCellHashDepth,
    master_metadata: TonCellHashDepth,
}

fn ton_parse_deployment_readback(
    boc: &TonBoc,
    computed: &[TonComputedCell],
    index: usize,
) -> Option<TonParsedDeploymentReadbackV1> {
    let bridge_config = ton_opened_hash_depth(boc, computed, index)?;
    let bridge_config_cell_hash = bridge_config.hash;
    let index = ton_virtual_root_index(boc, index)?;
    let cell = boc.cells.get(index)?;
    (ton_cell_type(cell)? == TonCellType::Ordinary).then_some(())?;
    let mut reader = TonBitReader::new(cell)?;
    let expected_global_id = reader.read_i32(32)?;
    let route_revision = u32::try_from(reader.read_u64(32)?).ok()?;
    let taira_to_ton_multiplier = reader.read_u64(64)?;
    let max_wrapped_supply = ton_read_canonical_coins(&mut reader)?;
    let source_lane = reader.read_ref()?;
    let destination_lane = reader.read_ref()?;
    let route_hashes = reader.read_ref()?;
    let finality_hashes = reader.read_ref()?;
    if !reader.exhausted() {
        return None;
    }

    let source_lane_bytes = ton_complete_ordinary_cell_bytes(boc, source_lane)?;
    let destination_lane_bytes = ton_complete_ordinary_cell_bytes(boc, destination_lane)?;

    let route_hashes = ton_virtual_root_index(boc, route_hashes)?;
    let route_hashes_cell = boc.cells.get(route_hashes)?;
    (ton_cell_type(route_hashes_cell)? == TonCellType::Ordinary).then_some(())?;
    let mut route_hashes_reader = TonBitReader::new(route_hashes_cell)?;
    let source_lane_hash = route_hashes_reader.read_h256()?;
    let destination_lane_hash = route_hashes_reader.read_h256()?;
    let route_configuration_hash = route_hashes_reader.read_h256()?;
    let route_hashes_tail = route_hashes_reader.read_ref()?;
    if !route_hashes_reader.exhausted() {
        return None;
    }
    let route_hashes_tail = ton_virtual_root_index(boc, route_hashes_tail)?;
    let route_hashes_tail_cell = boc.cells.get(route_hashes_tail)?;
    (ton_cell_type(route_hashes_tail_cell)? == TonCellType::Ordinary).then_some(())?;
    let mut route_hashes_tail_reader = TonBitReader::new(route_hashes_tail_cell)?;
    let destination_binding_hash = route_hashes_tail_reader.read_h256()?;
    let semantic_proof_profile_hash = route_hashes_tail_reader.read_h256()?;
    let deployment_codes = route_hashes_tail_reader.read_ref()?;
    if !route_hashes_tail_reader.exhausted() {
        return None;
    }
    let deployment_codes = ton_virtual_root_index(boc, deployment_codes)?;
    let deployment_codes_cell = boc.cells.get(deployment_codes)?;
    (ton_cell_type(deployment_codes_cell)? == TonCellType::Ordinary).then_some(())?;
    let mut deployment_codes_reader = TonBitReader::new(deployment_codes_cell)?;
    let jetton_master_code_hash = deployment_codes_reader.read_h256()?;
    let jetton_wallet_code_hash = deployment_codes_reader.read_h256()?;
    let route_code_hash = deployment_codes_reader.read_h256()?;
    if !deployment_codes_reader.exhausted() {
        return None;
    }

    let finality_hashes = ton_virtual_root_index(boc, finality_hashes)?;
    let finality_hashes_cell = boc.cells.get(finality_hashes)?;
    (ton_cell_type(finality_hashes_cell)? == TonCellType::Ordinary).then_some(())?;
    let mut finality_hashes_reader = TonBitReader::new(finality_hashes_cell)?;
    let sora_finality_anchor_hash = finality_hashes_reader.read_h256()?;
    let verifier_circuit_hash = finality_hashes_reader.read_h256()?;
    let verifying_key_hash = finality_hashes_reader.read_h256()?;
    let finality_tail = finality_hashes_reader.read_ref()?;
    if !finality_hashes_reader.exhausted() {
        return None;
    }
    let finality_tail = ton_virtual_root_index(boc, finality_tail)?;
    let finality_tail_cell = boc.cells.get(finality_tail)?;
    (ton_cell_type(finality_tail_cell)? == TonCellType::Ordinary).then_some(())?;
    let mut finality_tail_reader = TonBitReader::new(finality_tail_cell)?;
    let proof_profile_commitment = finality_tail_reader.read_h256()?;
    let guardians = finality_tail_reader.read_ref()?;
    let embedded_verifier_code_hash = finality_tail_reader.read_h256()?;
    let declared_verifying_key_cell_hash = finality_tail_reader.read_h256()?;
    let verifying_key_cell = finality_tail_reader.read_ref()?;
    let master_metadata = finality_tail_reader.read_ref()?;
    if !finality_tail_reader.exhausted() {
        return None;
    }
    let mint_breaker_guardian_keys = ton_parse_mint_breaker_guardians(boc, guardians)?;
    let (verifying_key, verifying_key_cell_hash) =
        ton_parse_verifying_key(boc, computed, verifying_key_cell)?;
    if declared_verifying_key_cell_hash != verifying_key_cell_hash
        || sccp_groth16_bls12381_verifying_key_hash_v1(verifying_key).ok()? != verifying_key_hash
    {
        return None;
    }
    let master_metadata = ton_opened_hash_depth(boc, computed, master_metadata)?;
    let master_metadata_hash = master_metadata.hash;
    Some(TonParsedDeploymentReadbackV1 {
        readback: TonDeploymentReadbackV1 {
            expected_global_id,
            route_revision,
            taira_to_ton_multiplier,
            max_wrapped_supply,
            source_lane_bytes,
            destination_lane_bytes,
            source_lane_hash,
            destination_lane_hash,
            route_configuration_hash,
            destination_binding_hash,
            semantic_proof_profile_hash,
            jetton_master_code_hash,
            jetton_master_initial_data_hash: [0; 32],
            jetton_wallet_code_hash,
            route_code_hash,
            route_initial_data_hash: [0; 32],
            sora_finality_anchor_hash,
            verifier_circuit_hash,
            verifying_key_hash,
            proof_profile_commitment,
            mint_breaker_guardian_keys,
            embedded_verifier_code_hash,
            verifying_key_cell_hash,
            verifying_key,
            master_metadata_hash,
            bridge_config_cell_hash,
        },
        bridge_config,
        master_metadata,
    })
}

fn ton_parse_route_storage_readback(
    boc: &TonBoc,
    computed: &[TonComputedCell],
    data_index: usize,
) -> Option<(TonParsedDeploymentReadbackV1, TonRouteStorageReadbackV1)> {
    let data_index = ton_virtual_root_index(boc, data_index)?;
    let cell = boc.cells.get(data_index)?;
    (ton_cell_type(cell)? == TonCellType::Ordinary).then_some(())?;
    let mut reader = TonBitReader::new(cell)?;
    if u8::try_from(reader.read_u64(8)?).ok()? != SCCP_V1_TON_STORAGE_VERSION {
        return None;
    }
    let route_configuration_hash = reader.read_h256()?;
    let bridge_config_cell_hash = reader.read_h256()?;
    let config = reader.read_ref()?;
    let replay = reader.read_ref()?;
    let pending = reader.read_ref()?;
    let minting_disabled = reader.read_bit()?;
    if !reader.exhausted() || ton_opaque_ref_hash(boc, computed, config)? != bridge_config_cell_hash
    {
        return None;
    }
    let deployment = ton_parse_deployment_readback(boc, computed, config)?;

    let replay = ton_virtual_root_index(boc, replay)?;
    let replay_cell = boc.cells.get(replay)?;
    (ton_cell_type(replay_cell)? == TonCellType::Ordinary).then_some(())?;
    let mut replay_reader = TonBitReader::new(replay_cell)?;
    let inbound_mint_replay =
        ton_parse_replay_forest_readback(boc, computed, replay_reader.read_ref()?)?;
    let outbound_burn_replay =
        ton_parse_replay_forest_readback(boc, computed, replay_reader.read_ref()?)?;
    if !replay_reader.exhausted() {
        return None;
    }

    let pending = ton_virtual_root_index(boc, pending)?;
    let pending_cell = boc.cells.get(pending)?;
    (ton_cell_type(pending_cell)? == TonCellType::Ordinary).then_some(())?;
    let mut pending_reader = TonBitReader::new(pending_cell)?;
    let pending_mint_root = ton_optional_dictionary_root_hash(boc, computed, &mut pending_reader)?;
    let pending_burn_root = ton_optional_dictionary_root_hash(boc, computed, &mut pending_reader)?;
    let pending_mint_count = u16::try_from(pending_reader.read_u64(16)?).ok()?;
    let pending_burn_count = u16::try_from(pending_reader.read_u64(16)?).ok()?;
    if !pending_reader.exhausted()
        || pending_mint_count > TON_SCCP_PENDING_OPERATION_CAP_V1
        || pending_burn_count > TON_SCCP_PENDING_OPERATION_CAP_V1
        || (pending_mint_count == 0) != pending_mint_root.is_none()
        || (pending_burn_count == 0) != pending_burn_root.is_none()
    {
        return None;
    }
    Some((
        deployment,
        TonRouteStorageReadbackV1 {
            route_configuration_hash,
            bridge_config_cell_hash,
            inbound_mint_replay,
            outbound_burn_replay,
            pending_mints: TonPendingMapReadbackV1 {
                dictionary_root_hash: pending_mint_root,
                count: pending_mint_count,
            },
            pending_burns: TonPendingMapReadbackV1 {
                dictionary_root_hash: pending_burn_root,
                count: pending_burn_count,
            },
            minting_disabled,
        },
    ))
}

fn ton_parse_master_storage_readback(
    boc: &TonBoc,
    computed: &[TonComputedCell],
    data_index: usize,
) -> Option<(
    TonParsedDeploymentReadbackV1,
    TonMasterStorageReadbackV1,
    TonCellHashDepth,
)> {
    let data_index = ton_virtual_root_index(boc, data_index)?;
    let cell = boc.cells.get(data_index)?;
    (ton_cell_type(cell)? == TonCellType::Ordinary).then_some(())?;
    let mut reader = TonBitReader::new(cell)?;
    if u8::try_from(reader.read_u64(8)?).ok()? != SCCP_V1_TON_STORAGE_VERSION {
        return None;
    }
    let route_configuration_hash = reader.read_h256()?;
    let bridge_config_cell_hash = reader.read_h256()?;
    let total_supply = ton_read_canonical_coins(&mut reader)?;
    let metadata = reader.read_ref()?;
    let config = reader.read_ref()?;
    let bridge_address = ton_read_canonical_std_address(&mut reader)?;
    let replay = reader.read_ref()?;
    let pending_mint_root = ton_optional_dictionary_root_hash(boc, computed, &mut reader)?;
    let pending_mint_count = u16::try_from(reader.read_u64(16)?).ok()?;
    let minting_disabled = reader.read_bit()?;
    if !reader.exhausted()
        || pending_mint_count > TON_SCCP_PENDING_OPERATION_CAP_V1
        || (pending_mint_count == 0) != pending_mint_root.is_none()
        || ton_opaque_ref_hash(boc, computed, config)? != bridge_config_cell_hash
    {
        return None;
    }
    let metadata = ton_opened_hash_depth(boc, computed, metadata)?;
    let metadata_hash = metadata.hash;
    let deployment = ton_parse_deployment_readback(boc, computed, config)?;
    let replay = ton_virtual_root_index(boc, replay)?;
    let replay_cell = boc.cells.get(replay)?;
    (ton_cell_type(replay_cell)? == TonCellType::Ordinary).then_some(())?;
    let mut replay_reader = TonBitReader::new(replay_cell)?;
    let mint_replay = ton_parse_replay_forest_readback(boc, computed, replay_reader.read_ref()?)?;
    let burn_replay = ton_parse_replay_forest_readback(boc, computed, replay_reader.read_ref()?)?;
    if !replay_reader.exhausted() {
        return None;
    }
    Some((
        deployment,
        TonMasterStorageReadbackV1 {
            route_configuration_hash,
            bridge_config_cell_hash,
            total_supply,
            metadata_hash,
            bridge_address,
            mint_replay,
            burn_replay,
            pending_mints: TonPendingMapReadbackV1 {
                dictionary_root_hash: pending_mint_root,
                count: pending_mint_count,
            },
            minting_disabled,
        },
        metadata,
    ))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TonOpenedAccountState {
    readback: TonAccountStateReadbackV1,
    code: TonCellHashDepth,
    data_index: usize,
}

fn ton_parse_active_account_state(
    boc: &TonBoc,
    computed: &[TonComputedCell],
    account_index: usize,
    expected_address: SccpTonAddressV1,
    shard_block_id: TonBlockIdExtV1,
    registered_masterchain_seqno: u32,
    shard_state_root_hash: H256,
    last_transaction_hash: H256,
    last_transaction_lt: u64,
) -> Option<TonOpenedAccountState> {
    let account_state_hash = ton_opaque_ref_hash(boc, computed, account_index)?;
    let index = ton_virtual_root_index(boc, account_index)?;
    let cell = boc.cells.get(index)?;
    (ton_cell_type(cell)? == TonCellType::Ordinary).then_some(())?;
    let mut reader = TonBitReader::new(cell)?;
    if !reader.read_bit()? || ton_read_canonical_std_address(&mut reader)? != expected_address {
        return None;
    }
    ton_skip_canonical_storage_used(&mut reader)?;
    ton_skip_storage_extra_info(&mut reader)?;
    reader.read_u64(32)?; // last_paid
    if reader.read_bit()? {
        ton_read_canonical_coins(&mut reader)?;
    }
    let storage_last_transaction_lt = reader.read_u64(64)?;
    let logical_time_is_valid = if last_transaction_lt == 0 {
        storage_last_transaction_lt == 0
    } else {
        storage_last_transaction_lt > last_transaction_lt
    };
    if !logical_time_is_valid {
        return None;
    }
    ton_skip_canonical_currency_collection(&mut reader)?;
    if !reader.read_bit()? || reader.read_bit()? || reader.read_bit()? {
        // Exactly active, with no split depth or tick/tock specialization.
        return None;
    }
    let code = if reader.read_bit()? {
        reader.read_ref()?
    } else {
        return None;
    };
    let data = if reader.read_bit()? {
        reader.read_ref()?
    } else {
        return None;
    };
    if reader.read_bit()? || !reader.exhausted() {
        // Canonical SCCP deployments carry no mutable library dictionary.
        return None;
    }
    let code = ton_opened_hash_depth(boc, computed, code)?;
    let code_hash = code.hash;
    let data_hash = ton_opaque_ref_hash(boc, computed, data)?;
    let data_index = ton_virtual_root_index(boc, data)?;
    Some(TonOpenedAccountState {
        readback: TonAccountStateReadbackV1 {
            address: expected_address,
            shard_block_id,
            registered_masterchain_seqno,
            shard_state_root_hash,
            account_state_hash,
            code_hash,
            data_hash,
            last_transaction_hash,
            last_transaction_lt,
            storage_last_transaction_lt,
        },
        code,
        data_index,
    })
}

fn ton_verify_breaker_account_opening(
    opening: &TonAccountStateOpeningV1,
    masterchain: &VerifiedMasterchain,
    expected_address: SccpTonAddressV1,
) -> Result<(TonBoc, Vec<TonComputedCell>, TonOpenedAccountState), TonNativeSourceError> {
    let shard_hashes = masterchain
        .extra
        .shard_hashes_root
        .ok_or(TonNativeSourceError::ShardNotFinalized)?;
    let (finalized_shard, registered_masterchain_seqno) =
        ton_select_shard_descriptor(&masterchain.boc, shard_hashes, expected_address)
            .ok_or(TonNativeSourceError::ShardNotFinalized)?;
    if finalized_shard != opening.shard_block_id
        || registered_masterchain_seqno > masterchain.block_id.seqno
    {
        return Err(TonNativeSourceError::ShardNotFinalized);
    }
    let (shard_boc, shard_computed, shard_root) =
        parse_canonical_single_root_boc(&opening.shard_block_proof_boc)
            .ok_or(TonNativeSourceError::InvalidBoc)?;
    if ton_proven_root_hash(&shard_boc, &shard_computed, shard_root)
        != Some(opening.shard_block_id.root_hash)
    {
        return Err(TonNativeSourceError::InvalidBoc);
    }
    let shard_block = ton_parse_block(&shard_boc, &shard_computed, shard_root)
        .ok_or(TonNativeSourceError::InvalidBoc)?;
    if shard_block.global_id != SCCP_TON_MAINNET_GLOBAL_ID_V1
        || !shard_block.info.not_master
        || shard_block.info.workchain != opening.shard_block_id.workchain
        || shard_block.info.shard != opening.shard_block_id.shard
        || shard_block.info.seqno != opening.shard_block_id.seqno
        || shard_block.info.before_split
        || shard_block.info.min_ref_mc_seqno > masterchain.block_id.seqno
        || !shard_block
            .info
            .master_ref
            .is_some_and(|reference| masterchain.chain_ids.contains(&reference))
    {
        return Err(TonNativeSourceError::ShardNotFinalized);
    }
    let (state_boc, state_computed, state_root) =
        parse_canonical_single_root_boc(&opening.shard_state_proof_boc)
            .ok_or(TonNativeSourceError::InvalidShardState)?;
    if ton_proven_root_hash(&state_boc, &state_computed, state_root)
        != Some(shard_block.new_state_hash)
    {
        return Err(TonNativeSourceError::InvalidShardState);
    }
    let state = ton_virtual_root_index(&state_boc, state_root)
        .ok_or(TonNativeSourceError::InvalidShardState)?;
    let accounts = ton_parse_shard_state_accounts(
        &state_boc,
        state,
        SCCP_TON_MAINNET_GLOBAL_ID_V1,
        opening.shard_block_id,
    )
    .ok_or(TonNativeSourceError::InvalidShardState)?;
    let accounts_root = ton_hashmap_aug_e_root(&state_boc, accounts, ton_skip_depth_balance)
        .ok_or(TonNativeSourceError::InvalidShardState)?;
    if accounts_root == usize::MAX {
        return Err(TonNativeSourceError::InvalidShardState);
    }
    let mut account = ton_hashmap_aug_leaf_reader(
        &state_boc,
        accounts_root,
        &expected_address.account,
        TON_SHARD_ACCOUNT_KEY_BITS,
        ton_skip_depth_balance,
    )
    .ok_or(TonNativeSourceError::InvalidShardState)?;
    ton_skip_depth_balance(&mut account).ok_or(TonNativeSourceError::InvalidShardState)?;
    let account_ref = account
        .read_ref()
        .ok_or(TonNativeSourceError::InvalidShardState)?;
    let last_transaction_hash = account
        .read_h256()
        .ok_or(TonNativeSourceError::InvalidShardState)?;
    let last_transaction_lt = account
        .read_u64(64)
        .ok_or(TonNativeSourceError::InvalidShardState)?;
    if !account.exhausted() {
        return Err(TonNativeSourceError::InvalidShardState);
    }
    let opened = ton_parse_active_account_state(
        &state_boc,
        &state_computed,
        account_ref,
        expected_address,
        opening.shard_block_id,
        registered_masterchain_seqno,
        shard_block.new_state_hash,
        last_transaction_hash,
        last_transaction_lt,
    )
    .ok_or(TonNativeSourceError::InvalidShardState)?;
    Ok((state_boc, state_computed, opened))
}

fn ton_deployment_readback_matches_governance(
    readback: &TonDeploymentReadbackV1,
    route: &SccpGovernedRouteV1,
    deployment: &SccpTonDestinationDeploymentV1,
) -> bool {
    let destination_lane = SccpLaneIdV1 {
        source: SccpNetworkV1::SoraTaira,
        target: SccpNetworkV1::TonMainnet,
    };
    let expected_source_lane_bytes = canonical_sccp_lane_id_bytes_v1(route.lane_id);
    let expected_destination_lane_bytes = canonical_sccp_lane_id_bytes_v1(destination_lane);
    let expected_source_lane_hash = sccp_lane_id_hash_v1(route.lane_id);
    let expected_destination_lane_hash = sccp_lane_id_hash_v1(destination_lane);
    let expected_route_configuration_hash = route.route_configuration_hash().ok();
    let expected_destination_binding_hash = route.destination_binding_hash().ok();
    let expected_semantic_profile_hash = deployment
        .outbound_proof_policy
        .semantic_profile_hash()
        .ok();
    let expected_finality_anchor_hash = deployment
        .outbound_proof_policy
        .sora_finality_anchor_hash()
        .ok();
    readback.expected_global_id == SCCP_TON_MAINNET_GLOBAL_ID_V1
        && readback.route_revision == route.revision
        && readback.taira_to_ton_multiplier == deployment.taira_to_token_multiplier
        && readback.max_wrapped_supply == deployment.max_wrapped_supply
        && Some(readback.source_lane_bytes.as_slice()) == expected_source_lane_bytes.as_deref()
        && Some(readback.destination_lane_bytes.as_slice())
            == expected_destination_lane_bytes.as_deref()
        && Some(readback.source_lane_hash) == expected_source_lane_hash
        && Some(readback.destination_lane_hash) == expected_destination_lane_hash
        && Some(readback.route_configuration_hash) == expected_route_configuration_hash
        && Some(readback.destination_binding_hash) == expected_destination_binding_hash
        && Some(readback.semantic_proof_profile_hash) == expected_semantic_profile_hash
        && readback.jetton_master_code_hash == deployment.jetton_master_code_hash
        && readback.jetton_master_initial_data_hash == deployment.jetton_master_initial_data_hash
        && readback.jetton_wallet_code_hash == deployment.jetton_wallet_code_hash
        && readback.route_code_hash == deployment.route_code_hash
        && readback.route_initial_data_hash == deployment.route_initial_data_hash
        && Some(readback.sora_finality_anchor_hash) == expected_finality_anchor_hash
        && readback.verifier_circuit_hash == deployment.verifier_circuit_hash
        && readback.verifying_key_hash == deployment.verifier_key_hash
        && readback.proof_profile_commitment == deployment.proof_profile_commitment
        && readback.mint_breaker_guardian_keys == deployment.mint_breaker_guardian_keys
        && readback.embedded_verifier_code_hash == deployment.embedded_verifier_code_hash
        && readback.verifying_key == deployment.verifying_key
}

fn ton_block_signatures_are_canonically_ordered(signatures: &TonBlockSignaturesV1) -> bool {
    let entries = match signatures {
        TonBlockSignaturesV1::Ordinary(proof) => proof.signatures.as_slice(),
        TonBlockSignaturesV1::Simplex(proof) => proof.signatures.as_slice(),
    };
    entries
        .windows(2)
        .all(|pair| pair[0].node_id_short < pair[1].node_id_short)
}

/// Verify one canonical dual-account TON mint-breaker observation.
///
/// Both account openings are selected from the same finalized TON-mainnet
/// masterchain block. The verifier then consumes the complete route/master
/// storage layouts and requires byte-for-byte agreement with the governed
/// destination deployment, including lanes, hash roles, verifier material,
/// guardian order, cap, reciprocal address, replay state, pending state, and
/// both irreversible breaker flags.
///
/// # Errors
///
/// Returns a fail-closed [`TonNativeSourceError`] for a noncanonical proof,
/// invalid finality, mismatched route revision, malformed typed storage, or any
/// authenticated field that differs from exact governance state.
pub fn verify_sccp_ton_breaker_observation_v1(
    proof: &SccpTonBreakerObservationProofV1,
    governed_route: &SccpGovernedRouteV1,
    expected_anchor_hash: H256,
) -> Result<VerifiedSccpTonBreakerObservationV1, TonNativeSourceError> {
    if proof.version != 1 || proof.finality.version != 1 {
        return Err(TonNativeSourceError::UnsupportedVersion);
    }
    let _ = ton_breaker_observation_work_estimate(proof)?;
    if !nonzero(&expected_anchor_hash)
        || governed_route.validate().is_err()
        || proof.route_key != governed_route.key()
        || governed_route.lane_id.source != SccpNetworkV1::TonMainnet
        || governed_route.lane_id.target != SccpNetworkV1::SoraTaira
        || proof.finality.anchor.network != SccpNetworkV1::TonMainnet
    {
        return Err(TonNativeSourceError::InvalidBreakerObservation);
    }
    let SccpDestinationDeploymentV1::Ton(deployment) = &governed_route.destination else {
        return Err(TonNativeSourceError::BreakerDeploymentMismatch);
    };
    for block in &proof.finality.blocks {
        if !ton_block_signatures_are_canonically_ordered(&block.signatures) {
            return Err(TonNativeSourceError::InvalidBreakerObservation);
        }
        parse_canonical_single_root_boc(&block.block_proof_boc)
            .ok_or(TonNativeSourceError::InvalidBoc)?;
    }
    let masterchain = verify_masterchain_finality(
        &proof.finality,
        SccpNetworkV1::TonMainnet,
        expected_anchor_hash,
    )?;
    let (route_boc, route_computed, route_account) = ton_verify_breaker_account_opening(
        &proof.route_account,
        &masterchain,
        deployment.route_address,
    )?;
    let (master_boc, master_computed, jetton_master_account) = ton_verify_breaker_account_opening(
        &proof.jetton_master_account,
        &masterchain,
        deployment.jetton_master_address,
    )?;
    if route_account.readback.code_hash != deployment.route_code_hash
        || jetton_master_account.readback.code_hash != deployment.jetton_master_code_hash
    {
        return Err(TonNativeSourceError::BreakerDeploymentMismatch);
    }
    let (mut route_deployment, route_storage) =
        ton_parse_route_storage_readback(&route_boc, &route_computed, route_account.data_index)
            .ok_or(TonNativeSourceError::BreakerDeploymentMismatch)?;
    let (master_deployment, master_storage, master_storage_metadata) =
        ton_parse_master_storage_readback(
            &master_boc,
            &master_computed,
            jetton_master_account.data_index,
        )
        .ok_or(TonNativeSourceError::BreakerDeploymentMismatch)?;
    if route_deployment != master_deployment
        || route_deployment.bridge_config != master_deployment.bridge_config
        || route_deployment.master_metadata != master_deployment.master_metadata
        || route_deployment.master_metadata != master_storage_metadata
    {
        return Err(TonNativeSourceError::BreakerDeploymentMismatch);
    }
    let bindings = ton_canonical_deployment_bindings_v1(
        route_deployment.readback.route_configuration_hash,
        route_deployment.bridge_config,
        route_deployment.master_metadata,
        route_account.code,
        jetton_master_account.code,
    )
    .ok_or(TonNativeSourceError::BreakerDeploymentMismatch)?;
    route_deployment.readback.route_initial_data_hash = bindings.route_initial_data.hash;
    route_deployment.readback.jetton_master_initial_data_hash = bindings.master_initial_data.hash;
    let route_deployment = route_deployment.readback;
    if bindings.route_address != deployment.route_address
        || bindings.master_address != deployment.jetton_master_address
        || !ton_deployment_readback_matches_governance(
            &route_deployment,
            governed_route,
            deployment,
        )
        || route_storage.route_configuration_hash != route_deployment.route_configuration_hash
        || master_storage.route_configuration_hash != route_deployment.route_configuration_hash
        || route_storage.bridge_config_cell_hash != route_deployment.bridge_config_cell_hash
        || master_storage.bridge_config_cell_hash != route_deployment.bridge_config_cell_hash
        || master_storage.metadata_hash != route_deployment.master_metadata_hash
        || master_storage.bridge_address != deployment.route_address
        || master_storage.total_supply > route_deployment.max_wrapped_supply
    {
        return Err(TonNativeSourceError::BreakerDeploymentMismatch);
    }
    let canonical_proof = norito::encode_canonical(proof)
        .map_err(|_| TonNativeSourceError::InvalidBreakerObservation)?;
    let canonical_proof_byte_len =
        u32::try_from(canonical_proof.len()).map_err(|_| TonNativeSourceError::ResourceLimit)?;
    let effective_disabled = route_storage.minting_disabled || master_storage.minting_disabled;
    Ok(VerifiedSccpTonBreakerObservationV1 {
        route_key: proof.route_key.clone(),
        masterchain_block_id: masterchain.block_id,
        masterchain_gen_utime: masterchain.gen_utime,
        route_account: route_account.readback,
        jetton_master_account: jetton_master_account.readback,
        deployment: route_deployment,
        route_storage,
        master_storage,
        effective_disabled,
        canonical_proof_sha256: Sha256::digest(&canonical_proof).into(),
        canonical_proof_byte_len,
    })
}

fn ton_parse_account_deployment(
    boc: &TonBoc,
    computed: &[TonComputedCell],
    account_index: usize,
    expected_address: SccpTonAddressV1,
    expected_code_hash: H256,
    expected_route_config_hash: H256,
    last_transaction_lt_requirement: TonLastTransactionLtRequirement,
) -> Option<()> {
    let index = ton_virtual_root_index(boc, account_index)?;
    let cell = boc.cells.get(index)?;
    (ton_cell_type(cell)? == TonCellType::Ordinary).then_some(())?;
    let mut reader = TonBitReader::new(cell)?;
    if !reader.read_bit()? || ton_read_internal_address(&mut reader)? != expected_address {
        return None;
    }
    ton_skip_storage_used(&mut reader)?;
    ton_skip_storage_extra_info(&mut reader)?;
    reader.read_u64(32)?; // last_paid
    if reader.read_bit()? {
        ton_skip_grams(&mut reader)?;
    }
    let storage_last_transaction_lt = reader.read_u64(64)?;
    if !last_transaction_lt_requirement.accepts(storage_last_transaction_lt) {
        return None;
    }
    ton_skip_currency_collection(&mut reader)?;
    if !reader.read_bit()? {
        // `account_active$1` is the only state that can authenticate code.
        return None;
    }
    if reader.read_bit()? {
        reader.read_u64(5)?; // fixed-prefix length
    }
    if reader.read_bit()? {
        reader.read_u64(2)?; // tick/tock flags
    }
    let code = if reader.read_bit()? {
        reader.read_ref()?
    } else {
        return None;
    };
    let data = if reader.read_bit()? {
        reader.read_ref()?
    } else {
        return None;
    };
    if reader.read_bit()? {
        reader.read_ref()?;
    }
    if !reader.exhausted()
        || ton_opened_original_tree_hash(boc, computed, code)? != expected_code_hash
    {
        return None;
    }
    let data = ton_virtual_root_index(boc, data)?;
    let data_cell = boc.cells.get(data)?;
    if ton_cell_type(data_cell)? != TonCellType::Ordinary {
        return None;
    }
    let mut data_reader = TonBitReader::new(data_cell)?;
    if u8::try_from(data_reader.read_u64(8)?).ok()? != SCCP_V1_TON_STORAGE_VERSION
        || data_reader.read_h256()? != expected_route_config_hash
    {
        return None;
    }
    // The reviewed source contract fixes these as the first fields of its
    // typed persistent state. Remaining fields are authenticated by the same
    // data-cell hash but are deliberately not interpreted by SCCP admission.
    Some(())
}

fn ton_verify_transaction_pre_state(
    proof_boc: &[u8],
    expected_account_hash: H256,
    emitter: SccpTonAddressV1,
    code_hash: H256,
    route_config_hash: H256,
    previous_transaction_lt: u64,
    transaction_lt: u64,
) -> Result<(), TonNativeSourceError> {
    let (boc, computed, root) =
        parse_single_root_boc(proof_boc).ok_or(TonNativeSourceError::SourceDeploymentMismatch)?;
    if ton_proven_root_hash(&boc, &computed, root) != Some(expected_account_hash) {
        return Err(TonNativeSourceError::SourceDeploymentMismatch);
    }
    ton_parse_account_deployment(
        &boc,
        &computed,
        root,
        emitter,
        code_hash,
        route_config_hash,
        TonLastTransactionLtRequirement::BetweenPreviousAndCurrent {
            previous_start_lt: previous_transaction_lt,
            current_start_lt: transaction_lt,
        },
    )
    .ok_or(TonNativeSourceError::SourceDeploymentMismatch)
}

fn ton_verify_source_account_state(
    proof_boc: &[u8],
    expected_state_root: H256,
    expected_global_id: i32,
    shard_block: TonBlockIdExtV1,
    emitter: SccpTonAddressV1,
    code_hash: H256,
    route_config_hash: H256,
    transaction_lt: u64,
    transaction_hash: H256,
    transaction_new_account_hash: H256,
) -> Result<(), TonNativeSourceError> {
    let (boc, computed, root) =
        parse_single_root_boc(proof_boc).ok_or(TonNativeSourceError::InvalidShardState)?;
    if ton_proven_root_hash(&boc, &computed, root) != Some(expected_state_root) {
        return Err(TonNativeSourceError::InvalidShardState);
    }
    let state =
        ton_virtual_root_index(&boc, root).ok_or(TonNativeSourceError::InvalidShardState)?;
    let accounts = ton_parse_shard_state_accounts(&boc, state, expected_global_id, shard_block)
        .ok_or(TonNativeSourceError::InvalidShardState)?;
    let accounts_root = ton_hashmap_aug_e_root(&boc, accounts, ton_skip_depth_balance)
        .ok_or(TonNativeSourceError::InvalidShardState)?;
    if accounts_root == usize::MAX {
        return Err(TonNativeSourceError::InvalidShardState);
    }
    let mut account = ton_hashmap_aug_leaf_reader(
        &boc,
        accounts_root,
        &emitter.account,
        TON_SHARD_ACCOUNT_KEY_BITS,
        ton_skip_depth_balance,
    )
    .ok_or(TonNativeSourceError::InvalidShardState)?;
    ton_skip_depth_balance(&mut account).ok_or(TonNativeSourceError::InvalidShardState)?;
    let account_ref = account
        .read_ref()
        .ok_or(TonNativeSourceError::InvalidShardState)?;
    let last_transaction_hash = account
        .read_h256()
        .ok_or(TonNativeSourceError::InvalidShardState)?;
    let last_transaction_lt = account
        .read_u64(64)
        .ok_or(TonNativeSourceError::InvalidShardState)?;
    if !account.exhausted()
        || last_transaction_lt < transaction_lt
        || (last_transaction_lt == transaction_lt && last_transaction_hash != transaction_hash)
    {
        return Err(TonNativeSourceError::InvalidShardState);
    }
    if last_transaction_lt == transaction_lt
        && ton_opened_original_tree_hash(&boc, &computed, account_ref)
            != Some(transaction_new_account_hash)
    {
        return Err(TonNativeSourceError::InvalidShardState);
    }
    ton_parse_account_deployment(
        &boc,
        &computed,
        account_ref,
        emitter,
        code_hash,
        route_config_hash,
        TonLastTransactionLtRequirement::After(last_transaction_lt),
    )
    .ok_or(TonNativeSourceError::SourceDeploymentMismatch)
}

/// Verify a complete native TON source proof against exact governed material.
///
/// # Errors
///
/// Returns a fail-closed [`TonNativeSourceError`] for malformed native cells,
/// invalid finality, unauthenticated transitions, failed transactions, or an
/// event body that differs from the supplied canonical SCCP payload.
#[allow(clippy::too_many_arguments)]
pub fn verify_ton_native_source(
    proof: &TonNativeSourceProofV1,
    governed_source_identity: &SccpSourceIdentityV1,
    expected_source_identity_hash: H256,
    expected_anchor_hash: H256,
    expected_message_id: H256,
    expected_payload_hash: H256,
    payload: &SccpPayloadV1,
) -> Result<ValidatedTonNativeSourceV1, TonNativeSourceError> {
    if proof.version != 1 || proof.finality.version != 1 {
        return Err(TonNativeSourceError::UnsupportedVersion);
    }
    let _ = ton_native_source_work_estimate(proof)?;
    if !governed_source_identity.is_well_formed()
        || governed_source_identity.lane.source != proof.finality.anchor.network
        || governed_source_identity.lane.target != SccpNetworkV1::SoraTaira
    {
        return Err(TonNativeSourceError::InvalidSourceIdentity);
    }
    let SccpSourceEmitterV1::Ton(emitter) = governed_source_identity.emitter else {
        return Err(TonNativeSourceError::InvalidSourceIdentity);
    };
    if emitter.address.workchain != SCCP_TON_BASECHAIN_WORKCHAIN_V1 {
        return Err(TonNativeSourceError::InvalidSourceIdentity);
    }
    let source_identity_hash = sccp_source_identity_hash_v1(governed_source_identity)
        .ok_or(TonNativeSourceError::InvalidSourceIdentity)?;
    if source_identity_hash != expected_source_identity_hash {
        return Err(TonNativeSourceError::SourceIdentityHashMismatch);
    }
    if !verify_sccp_payload_structure(payload) {
        return Err(TonNativeSourceError::EventStatementMismatch);
    }
    let canonical_payload = canonical_sccp_payload_bytes(payload)
        .map_err(|_| TonNativeSourceError::EventStatementMismatch)?;
    let lane_hash = sccp_lane_id_hash_v1(governed_source_identity.lane)
        .ok_or(TonNativeSourceError::InvalidSourceIdentity)?;
    let message_id = sccp_message_id(governed_source_identity.lane, payload)
        .ok_or(TonNativeSourceError::EventStatementMismatch)?;
    let canonical_payload_hash = payload_hash(&canonical_payload);
    let source_event_digest = sccp_lane_source_event_digest_v1(
        governed_source_identity.lane,
        message_id,
        canonical_payload_hash,
    )
    .ok_or(TonNativeSourceError::EventStatementMismatch)?;
    if message_id != expected_message_id || canonical_payload_hash != expected_payload_hash {
        return Err(TonNativeSourceError::EventStatementMismatch);
    }
    if proof.event.transaction_lt == 0
        || proof.event.outbound_message_index >= (1 << 15)
        || proof.event.shard_block_proof_boc.is_empty()
        || proof.event.shard_block_proof_boc.len() > TON_MAX_BOC_BYTES
        || proof.event.transaction_pre_state_proof_boc.is_empty()
        || proof.event.transaction_pre_state_proof_boc.len() > TON_MAX_BOC_BYTES
        || proof.event.shard_state_proof_boc.is_empty()
        || proof.event.shard_state_proof_boc.len() > TON_MAX_BOC_BYTES
    {
        return Err(TonNativeSourceError::ResourceLimit);
    }
    let masterchain = verify_masterchain_finality(
        &proof.finality,
        governed_source_identity.lane.source,
        expected_anchor_hash,
    )?;
    let shard_hashes = masterchain
        .extra
        .shard_hashes_root
        .ok_or(TonNativeSourceError::ShardNotFinalized)?;
    let (finalized_shard, registered_masterchain_seqno) =
        ton_select_shard_descriptor(&masterchain.boc, shard_hashes, emitter.address)
            .ok_or(TonNativeSourceError::ShardNotFinalized)?;
    if finalized_shard != proof.event.shard_block_id
        || registered_masterchain_seqno > masterchain.block_id.seqno
    {
        return Err(TonNativeSourceError::ShardNotFinalized);
    }
    let (shard_boc, shard_computed, shard_root) =
        parse_single_root_boc(&proof.event.shard_block_proof_boc)
            .ok_or(TonNativeSourceError::InvalidBoc)?;
    if ton_proven_root_hash(&shard_boc, &shard_computed, shard_root)
        != Some(proof.event.shard_block_id.root_hash)
    {
        return Err(TonNativeSourceError::InvalidBoc);
    }
    let shard_block = ton_parse_block(&shard_boc, &shard_computed, shard_root)
        .ok_or(TonNativeSourceError::InvalidBoc)?;
    let global_id = ton_network_global_id(governed_source_identity.lane.source)
        .ok_or(TonNativeSourceError::WrongNetwork)?;
    if shard_block.global_id != global_id
        || !shard_block.info.not_master
        || shard_block.info.workchain != proof.event.shard_block_id.workchain
        || shard_block.info.shard != proof.event.shard_block_id.shard
        || shard_block.info.seqno != proof.event.shard_block_id.seqno
        || shard_block.info.before_split
        || shard_block.info.min_ref_mc_seqno > masterchain.block_id.seqno
        || !shard_block
            .info
            .master_ref
            .is_some_and(|reference| masterchain.chain_ids.contains(&reference))
    {
        return Err(TonNativeSourceError::ShardNotFinalized);
    }
    let account_blocks = ton_parse_block_extra_account_blocks(&shard_boc, shard_block.extra_index)
        .ok_or(TonNativeSourceError::InvalidTransaction)?;
    let transaction_index = ton_transaction_from_account_blocks(
        &shard_boc,
        account_blocks,
        emitter.address.account,
        proof.event.transaction_lt,
    )
    .ok_or(TonNativeSourceError::InvalidTransaction)?;
    let transaction = ton_parse_transaction(
        &shard_boc,
        &shard_computed,
        transaction_index,
        emitter.address.account,
        proof.event.transaction_lt,
    )
    .ok_or(TonNativeSourceError::InvalidTransaction)?;
    ton_verify_transaction_pre_state(
        &proof.event.transaction_pre_state_proof_boc,
        transaction.old_account_hash,
        emitter.address,
        emitter.code_hash,
        emitter.route_config_hash,
        transaction.previous_logical_time,
        transaction.logical_time,
    )?;
    if !ton_transaction_succeeded(&shard_boc, transaction)
        .ok_or(TonNativeSourceError::InvalidTransaction)?
    {
        return Err(TonNativeSourceError::UnsuccessfulTransaction);
    }
    let outbound =
        ton_transaction_out_message(&shard_boc, transaction, proof.event.outbound_message_index)
            .ok_or(TonNativeSourceError::InvalidOutboundMessage)?;
    let outbound_message_hash = ton_parse_external_event_message(
        &shard_boc,
        &shard_computed,
        outbound,
        emitter.address,
        lane_hash,
        message_id,
        canonical_payload_hash,
        source_event_digest,
        &canonical_payload,
    )
    .ok_or(TonNativeSourceError::EventStatementMismatch)?;
    ton_verify_source_account_state(
        &proof.event.shard_state_proof_boc,
        shard_block.new_state_hash,
        global_id,
        proof.event.shard_block_id,
        emitter.address,
        emitter.code_hash,
        emitter.route_config_hash,
        transaction.logical_time,
        transaction.hash,
        transaction.new_account_hash,
    )?;
    Ok(ValidatedTonNativeSourceV1 {
        source_identity_hash,
        lane_hash,
        anchor_hash: expected_anchor_hash,
        masterchain_seqno: masterchain.block_id.seqno,
        masterchain_block_hash: masterchain.block_id.root_hash,
        shard_seqno: proof.event.shard_block_id.seqno,
        shard_block_hash: proof.event.shard_block_id.root_hash,
        transaction_hash: transaction.hash,
        transaction_lt: transaction.logical_time,
        outbound_message_hash,
        message_id,
        payload_hash: canonical_payload_hash,
        source_event_digest,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair, Signature};

    fn hex32(value: &str) -> H256 {
        assert_eq!(value.len(), 64);
        let mut out = [0_u8; 32];
        for (index, byte) in out.iter_mut().enumerate() {
            let offset = index * 2;
            *byte = u8::from_str_radix(&value[offset..offset + 2], 16).expect("valid fixture hex");
        }
        out
    }

    fn fixture_block() -> TonBlockIdExtV1 {
        TonBlockIdExtV1 {
            workchain: SCCP_TON_MASTERCHAIN_WORKCHAIN_V1,
            shard: SCCP_TON_MASTERCHAIN_SHARD_V1,
            seqno: 42,
            root_hash: [0x11; 32],
            file_hash: [0x22; 32],
        }
    }

    fn fixture_validator(seed: u8, weight: u64) -> (KeyPair, TonValidatorV1) {
        let pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("fixture Ed25519 key");
        let (algorithm, raw) = pair
            .public_key()
            .try_to_bytes()
            .expect("fixture public key bytes");
        assert_eq!(algorithm, Algorithm::Ed25519);
        let public_key = raw.try_into().expect("Ed25519 public keys are 32 bytes");
        (
            pair,
            TonValidatorV1 {
                public_key,
                weight,
                adnl_address: [seed; 32],
            },
        )
    }

    fn signed_entry(
        pair: &KeyPair,
        validator: TonValidatorV1,
        transcript: &[u8],
    ) -> TonValidatorSignatureV1 {
        TonValidatorSignatureV1 {
            node_id_short: ton_validator_node_id_short_v1(&validator.public_key)
                .expect("fixture node id"),
            signature: Signature::try_new(pair.private_key(), transcript)
                .expect("fixture signature")
                .payload()
                .to_vec(),
        }
    }

    fn ordinary_cell(data: Vec<u8>, refs: Vec<usize>) -> TonBocCell {
        TonBocCell {
            descriptor: u8::try_from(refs.len()).expect("fixture ref count"),
            data_descriptor: u8::try_from(data.len() * 2).expect("fixture cell byte count"),
            data,
            refs,
            exotic: false,
        }
    }

    fn pruned_branch_cell(mask: u8, hashes: &[H256], depths: &[u16]) -> TonBocCell {
        let count = ton_level_mask_hash_index(mask);
        assert_eq!(hashes.len(), count);
        assert_eq!(depths.len(), count);
        let mut data = vec![1, mask];
        for hash in hashes {
            data.extend_from_slice(hash);
        }
        for depth in depths {
            data.extend_from_slice(&depth.to_be_bytes());
        }
        TonBocCell {
            descriptor: 0x08 | (mask << 5),
            data_descriptor: u8::try_from(data.len() * 2).expect("fixture cell byte count"),
            data,
            refs: Vec::new(),
            exotic: true,
        }
    }

    fn merkle_proof_cell(reference: usize, child_mask: u8, hash: H256, depth: u16) -> TonBocCell {
        let mut data = vec![3];
        data.extend_from_slice(&hash);
        data.extend_from_slice(&depth.to_be_bytes());
        TonBocCell {
            descriptor: 0x09 | (ton_level_mask_value(child_mask >> 1) << 5),
            data_descriptor: u8::try_from(data.len() * 2).expect("fixture cell byte count"),
            data,
            refs: vec![reference],
            exotic: true,
        }
    }

    fn reset_roster_key_parse_count() {
        TON_ROSTER_KEY_PARSE_COUNT.with(|count| count.set(0));
    }

    fn roster_key_parse_count() -> usize {
        TON_ROSTER_KEY_PARSE_COUNT.with(core::cell::Cell::get)
    }

    #[derive(Default)]
    struct TestBits(Vec<bool>);

    impl TestBits {
        fn bit(&mut self, value: bool) {
            self.0.push(value);
        }

        fn uint(&mut self, value: u64, width: usize) {
            for shift in (0..width).rev() {
                self.bit(value & (1_u64 << shift) != 0);
            }
        }

        fn bytes(&mut self, value: &[u8]) {
            for byte in value {
                self.uint(u64::from(*byte), 8);
            }
        }

        fn cell(self, refs: Vec<usize>) -> TonBocCell {
            let bit_len = self.0.len();
            let byte_len = bit_len.div_ceil(8);
            let mut data = vec![0_u8; byte_len];
            for (index, bit) in self.0.into_iter().enumerate() {
                if bit {
                    data[index / 8] |= 1 << (7 - index % 8);
                }
            }
            let data_descriptor = if bit_len % 8 == 0 {
                byte_len * 2
            } else {
                data[bit_len / 8] |= 1 << (7 - bit_len % 8);
                byte_len * 2 - 1
            };
            TonBocCell {
                descriptor: u8::try_from(refs.len()).expect("fixture ref count"),
                data_descriptor: u8::try_from(data_descriptor).expect("fixture data descriptor"),
                data,
                refs,
                exotic: false,
            }
        }
    }

    fn account_deployment_boc(
        address: SccpTonAddressV1,
        last_transaction_lt: u64,
        code: u8,
        route_config_hash: H256,
    ) -> TonBoc {
        assert_eq!(address.workchain, SCCP_TON_BASECHAIN_WORKCHAIN_V1);
        let mut account = TestBits::default();
        account.bit(true); // account$1
        account.bit(true); // addr_std$10
        account.bit(false);
        account.bit(false); // no anycast
        account.uint(0, 8); // basechain workchain
        account.bytes(&address.account);
        account.uint(0, 3); // zero used cells
        account.uint(0, 3); // zero used bits
        account.uint(0, 3); // storage_extra_none$000
        account.uint(0, 32); // last_paid
        account.bit(false); // no due payment
        account.uint(last_transaction_lt, 64);
        account.uint(0, 4); // zero grams
        account.bit(false); // no extra currencies
        account.bit(true); // account_active$1
        account.bit(false); // no fixed prefix
        account.bit(false); // no tick/tock flags
        account.bit(true); // code reference
        account.bit(true); // data reference
        account.bit(false); // no library

        let mut data = vec![SCCP_V1_TON_STORAGE_VERSION];
        data.extend_from_slice(&route_config_hash);
        TonBoc {
            roots: vec![0],
            cells: vec![
                account.cell(vec![1, 2]),
                ordinary_cell(vec![code], Vec::new()),
                ordinary_cell(data, Vec::new()),
            ],
        }
    }

    fn serialize_test_boc(boc: &TonBoc) -> Vec<u8> {
        assert_eq!(boc.roots, [0]);
        assert!(boc.cells.len() < 256);
        let total_cells_size = boc
            .cells
            .iter()
            .map(|cell| 2 + cell.data.len() + cell.refs.len())
            .sum::<usize>();
        assert!(total_cells_size < usize::from(u16::MAX));
        let offset_bytes = if total_cells_size < 256 { 1 } else { 2 };
        let mut out = TON_BOC_MAGIC.to_vec();
        out.extend_from_slice(&[
            1,
            offset_bytes,
            u8::try_from(boc.cells.len()).expect("fixture cell count"),
            1,
            0,
        ]);
        if offset_bytes == 1 {
            out.push(u8::try_from(total_cells_size).expect("fixture serialized size"));
        } else {
            out.extend_from_slice(
                &u16::try_from(total_cells_size)
                    .expect("fixture serialized size")
                    .to_be_bytes(),
            );
        }
        out.push(0);
        for cell in &boc.cells {
            out.push(cell.descriptor);
            out.push(cell.data_descriptor);
            out.extend_from_slice(&cell.data);
            out.extend(
                cell.refs
                    .iter()
                    .map(|reference| u8::try_from(*reference).expect("fixture reference")),
            );
        }
        out
    }

    fn payload_boc(payload: &[u8], lengths: [usize; 4]) -> TonBoc {
        assert_eq!(lengths.iter().sum::<usize>(), payload.len());
        let mut offset = 0_usize;
        let mut cells = Vec::with_capacity(4);
        for (index, length) in lengths.into_iter().enumerate() {
            let end = offset + length;
            let refs = if index == 3 {
                Vec::new()
            } else {
                vec![index + 1]
            };
            cells.push(ordinary_cell(payload[offset..end].to_vec(), refs));
            offset = end;
        }
        TonBoc {
            roots: vec![0],
            cells,
        }
    }

    fn simplex_candidate_without_parents(block: TonBlockIdExtV1) -> Vec<u8> {
        let mut candidate = Vec::new();
        push_u32_le(
            &mut candidate,
            TON_CONSENSUS_CANDIDATE_ORDINARY_TL_CONSTRUCTOR,
        );
        push_i32_le(&mut candidate, block.workchain);
        push_u64_le(&mut candidate, block.shard);
        push_u32_le(&mut candidate, block.seqno);
        candidate.extend_from_slice(&block.root_hash);
        candidate.extend_from_slice(&block.file_hash);
        candidate.extend_from_slice(&[0x33; 32]);
        push_u32_le(
            &mut candidate,
            TON_CONSENSUS_CANDIDATE_WITHOUT_PARENTS_TL_CONSTRUCTOR,
        );
        candidate
    }

    fn masterchain_continuation_fixture(
        previous: TonBlockIdExtV1,
        active: &TonValidatorSetV1,
    ) -> (TonBlockIdExtV1, Vec<u8>, H256, H256) {
        let old_state = ordinary_cell(vec![0xa1], Vec::new());
        let new_state = ordinary_cell(vec![0xa2], Vec::new());
        let state_cells = TonBoc {
            roots: vec![0],
            cells: vec![old_state.clone(), new_state.clone()],
        };
        let state_hashes = ton_boc_cell_hashes(&state_cells).expect("state hashes");
        let old_state_hash = state_hashes[0].hashes[0];
        let new_state_hash = state_hashes[1].hashes[0];

        let mut root = TestBits::default();
        root.uint(u64::from(TON_BLOCK_CONSTRUCTOR), 32);
        root.uint(
            u64::from(u32::from_be_bytes(
                SCCP_TON_MAINNET_GLOBAL_ID_V1.to_be_bytes(),
            )),
            32,
        );

        let mut info = TestBits::default();
        info.uint(u64::from(TON_BLOCK_INFO_CONSTRUCTOR), 32);
        info.uint(0, 32); // version
        for _ in 0..8 {
            info.bit(false);
        }
        info.uint(0, 8); // flags
        info.uint(u64::from(previous.seqno + 1), 32);
        info.uint(0, 32); // vertical seqno
        info.uint(0, 2); // ShardIdent constructor
        info.uint(0, 6); // masterchain prefix length
        info.uint(u64::from(u32::MAX), 32);
        info.uint(SCCP_TON_MASTERCHAIN_SHARD_V1, 64);
        info.uint(1, 32); // generation time
        info.uint(1, 64); // start logical time
        info.uint(2, 64); // end logical time
        info.uint(u64::from(active.validator_list_hash_short), 32);
        info.uint(u64::from(active.catchain_seqno), 32);
        info.uint(0, 32); // minimum referenced masterchain seqno
        info.uint(0, 32); // previous key-block seqno

        let mut previous_ref = TestBits::default();
        previous_ref.uint(1, 64); // end logical time
        previous_ref.uint(u64::from(previous.seqno), 32);
        previous_ref.bytes(&previous.root_hash);
        previous_ref.bytes(&previous.file_hash);

        let mut update_data = vec![4];
        update_data.extend_from_slice(&old_state_hash);
        update_data.extend_from_slice(&new_state_hash);
        update_data.extend_from_slice(&state_hashes[0].depths[0].to_be_bytes());
        update_data.extend_from_slice(&state_hashes[1].depths[0].to_be_bytes());
        let state_update = TonBocCell {
            descriptor: 0x0a,
            data_descriptor: u8::try_from(update_data.len() * 2)
                .expect("fixture update descriptor"),
            data: update_data,
            refs: vec![4, 5],
            exotic: true,
        };

        let mut extra = TestBits::default();
        extra.bytes(&[0; 64]); // random seed and creator
        extra.bit(true); // custom masterchain extra is present
        let mut custom = TestBits::default();
        custom.uint(u64::from(TON_MC_BLOCK_EXTRA_CONSTRUCTOR), 16);
        custom.bit(false); // not a key block
        custom.bit(false); // no ShardHashes dictionary needed for this finality-only fixture
        custom.bit(false); // no shard-fees dictionary
        for _ in 0..2 {
            custom.uint(0, 4); // zero grams
            custom.bit(false); // no extra currencies
        }

        let boc = TonBoc {
            roots: vec![0],
            cells: vec![
                root.cell(vec![1, 2, 3, 6]),
                info.cell(vec![7]),
                ordinary_cell(Vec::new(), Vec::new()),
                state_update,
                old_state,
                new_state,
                extra.cell(vec![8, 9, 10, 11]),
                previous_ref.cell(Vec::new()),
                ordinary_cell(Vec::new(), Vec::new()),
                ordinary_cell(Vec::new(), Vec::new()),
                ordinary_cell(Vec::new(), Vec::new()),
                custom.cell(vec![12]),
                ordinary_cell(Vec::new(), Vec::new()),
            ],
        };
        let bytes = serialize_test_boc(&boc);
        let block_id = TonBlockIdExtV1 {
            workchain: SCCP_TON_MASTERCHAIN_WORKCHAIN_V1,
            shard: SCCP_TON_MASTERCHAIN_SHARD_V1,
            seqno: previous.seqno + 1,
            root_hash: ton_boc_single_root_hash_v1(&bytes).expect("fixture block root"),
            file_hash: Sha256::digest(&bytes).into(),
        };
        (block_id, bytes, old_state_hash, new_state_hash)
    }

    fn work_estimate_fixture() -> TonNativeSourceProofV1 {
        let (_, validator) = fixture_validator(1, 1);
        let validators = vec![validator];
        let validator_list_hash_short =
            ton_validator_list_hash_short_v1(7, &validators).expect("fixture set hash");
        let signature = TonValidatorSignatureV1 {
            node_id_short: ton_validator_node_id_short_v1(&validator.public_key)
                .expect("fixture node id"),
            signature: vec![0; 64],
        };
        let signed = |count| {
            TonBlockSignaturesV1::Ordinary(TonOrdinaryBlockSignaturesV1 {
                catchain_seqno: 7,
                validator_list_hash_short,
                signatures: vec![signature.clone(); count],
            })
        };
        let checkpoint = TonBlockIdExtV1 {
            seqno: 1,
            ..fixture_block()
        };
        TonNativeSourceProofV1 {
            version: 1,
            finality: TonNativeFinalityProofV1 {
                version: 1,
                anchor: TonNativeAnchorV1 {
                    version: 1,
                    network: SccpNetworkV1::TonMainnet,
                    zero_state: ton_expected_zero_state(SccpNetworkV1::TonMainnet)
                        .expect("mainnet profile"),
                    checkpoint,
                    checkpoint_state_root: [0x55; 32],
                    active_validator_set: TonValidatorSetV1 {
                        catchain_seqno: 7,
                        validator_list_hash_short,
                        validators,
                    },
                    pending_validator_config: None,
                },
                blocks: vec![
                    TonMasterchainBlockProofV1 {
                        block_id: TonBlockIdExtV1 {
                            seqno: 2,
                            ..checkpoint
                        },
                        block_proof_boc: vec![1; 3],
                        signatures: signed(1),
                    },
                    TonMasterchainBlockProofV1 {
                        block_id: TonBlockIdExtV1 {
                            seqno: 3,
                            ..checkpoint
                        },
                        block_proof_boc: vec![2; 5],
                        signatures: signed(2),
                    },
                ],
            },
            event: TonShardEventProofV1 {
                shard_block_id: TonBlockIdExtV1 {
                    workchain: SCCP_TON_BASECHAIN_WORKCHAIN_V1,
                    seqno: 9,
                    ..fixture_block()
                },
                shard_block_proof_boc: vec![3; 7],
                transaction_pre_state_proof_boc: vec![5; 13],
                shard_state_proof_boc: vec![4; 11],
                transaction_lt: 1,
                outbound_message_index: 0,
            },
        }
    }

    fn breaker_work_estimate_fixture() -> SccpTonBreakerObservationProofV1 {
        let source = work_estimate_fixture();
        let shard_block_id = source.event.shard_block_id;
        SccpTonBreakerObservationProofV1 {
            version: 1,
            route_key: SccpRouteKeyV1 {
                lane_id: SccpLaneIdV1 {
                    source: SccpNetworkV1::TonMainnet,
                    target: SccpNetworkV1::SoraTaira,
                },
                route_id: "taira_ton_xor".to_owned(),
                asset_key: "xor".to_owned(),
                revision: 1,
            },
            finality: source.finality,
            route_account: TonAccountStateOpeningV1 {
                shard_block_id,
                shard_block_proof_boc: vec![0x11; 3],
                shard_state_proof_boc: vec![0x12; 5],
            },
            jetton_master_account: TonAccountStateOpeningV1 {
                shard_block_id,
                shard_block_proof_boc: vec![0x21; 7],
                shard_state_proof_boc: vec![0x22; 11],
            },
        }
    }

    #[test]
    fn validator_node_id_and_roster_hash_match_native_vectors() {
        let keys = [
            hex32("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"),
            hex32("3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c"),
            hex32("fc51cd8e6218a1a38da47ed00230f0580816ed13ba3303ac5deb911548908025"),
        ];
        assert_eq!(
            ton_validator_node_id_short_v1(&keys[0]),
            Some(hex32(
                "1ebe11eac72c9c99edca05d0fe3bbf1bdbfd5225d20862df516e14dece65d11e"
            ))
        );
        let validators = keys
            .into_iter()
            .zip([1_u64, 2, 3])
            .zip([1_u8, 2, 3])
            .map(|((public_key, weight), adnl)| TonValidatorV1 {
                public_key,
                weight,
                adnl_address: [adnl; 32],
            })
            .collect::<Vec<_>>();
        assert_eq!(
            ton_validator_list_hash_short_v1(17, &validators),
            Some(0x9a58_6c28)
        );

        let mut duplicate = validators.clone();
        duplicate[2].public_key = duplicate[0].public_key;
        assert_eq!(ton_validator_list_hash_short_v1(17, &duplicate), None);
        duplicate = validators.clone();
        duplicate[2].adnl_address = duplicate[0].adnl_address;
        assert_eq!(ton_validator_list_hash_short_v1(17, &duplicate), None);

        let invalid_key = [0xff; 32];
        assert_eq!(ton_validator_node_id_short_v1(&invalid_key), None);
        duplicate = validators;
        duplicate[2].public_key = invalid_key;
        assert_eq!(ton_validator_list_hash_short_v1(17, &duplicate), None);
    }

    #[test]
    fn governed_rosters_are_parsed_once_per_metered_validation_pass() {
        let mut anchor = work_estimate_fixture().finality.anchor;
        let (_, pending_validator) = fixture_validator(2, 2);
        anchor.pending_validator_config = Some(TonValidatorConfigV1 {
            valid_since: 1,
            valid_until: u32::MAX,
            main_validator_count: 1,
            shuffle_masterchain_validators: false,
            validators: vec![pending_validator],
        });

        reset_roster_key_parse_count();
        assert!(canonical_ton_native_anchor_bytes_v1(&anchor).is_some());
        assert_eq!(roster_key_parse_count(), 2);

        reset_roster_key_parse_count();
        assert_eq!(validate_ton_native_anchor(&anchor), Some(()));
        assert_eq!(roster_key_parse_count(), 2);
        assert!(ton_native_anchor_hash_from_validated(&anchor).is_some());
        assert_eq!(
            roster_key_parse_count(),
            2,
            "hashing validated anchor material must not reparse validator keys"
        );

        reset_roster_key_parse_count();
        assert!(
            ton_select_masterchain_validator_set(
                anchor
                    .pending_validator_config
                    .as_ref()
                    .expect("pending fixture"),
                8,
            )
            .is_some()
        );
        assert_eq!(roster_key_parse_count(), 1);

        anchor.active_validator_set.validators[0].public_key = [0xff; 32];
        assert_eq!(canonical_ton_native_anchor_bytes_v1(&anchor), None);
        assert_eq!(ton_native_anchor_hash_v1(&anchor), None);
    }

    #[test]
    fn source_work_estimate_counts_all_bocs_signatures_and_key_bounds() {
        let proof = work_estimate_fixture();
        assert_eq!(
            ton_native_finality_work_estimate(&proof.finality),
            Ok(TonNativeFinalityWorkEstimateV1 {
                continuation_blocks: 2,
                framed_boc_bytes: 8,
                ed25519_signature_checks: 3,
                validator_key_checks_upper_bound: 6_145,
            })
        );
        assert_eq!(
            ton_native_source_work_estimate(&proof),
            Ok(TonNativeFinalityWorkEstimateV1 {
                continuation_blocks: 2,
                framed_boc_bytes: 39,
                ed25519_signature_checks: 3,
                validator_key_checks_upper_bound: 6_145,
            })
        );
    }

    #[test]
    fn source_proof_binary_and_json_roundtrip_include_transaction_pre_state() {
        let proof = work_estimate_fixture();
        let encoded = norito::to_bytes(&proof).expect("TON source proof encodes");
        assert_eq!(
            norito::decode_from_bytes::<TonNativeSourceProofV1>(&encoded)
                .expect("TON source proof decodes"),
            proof
        );
        let json = norito::json::to_json(&proof).expect("TON source proof JSON encodes");
        assert!(json.contains("transaction_pre_state_proof_boc"));
        assert_eq!(
            norito::json::from_json::<TonNativeSourceProofV1>(&json)
                .expect("TON source proof JSON decodes"),
            proof
        );
    }

    #[test]
    fn transaction_pre_state_proof_binds_governed_execution_code_and_config() {
        let emitter = SccpTonAddressV1 {
            workchain: SCCP_TON_BASECHAIN_WORKCHAIN_V1,
            account: [0x91; 32],
        };
        let governed_route_config = [0xa1; 32];
        // Transaction.prev_trans_lt is the previous transaction's start LT;
        // AccountStorage.last_trans_lt is its later end LT.
        let governed = account_deployment_boc(emitter, 42, 0x11, governed_route_config);
        let governed_computed = ton_boc_cell_hashes(&governed).expect("governed account hashes");
        let governed_account_hash = governed_computed[0].hashes[3];
        let governed_code_hash = governed_computed[1].hashes[3];
        let governed_boc = serialize_test_boc(&governed);
        assert_eq!(
            ton_verify_transaction_pre_state(
                &governed_boc,
                governed_account_hash,
                emitter,
                governed_code_hash,
                governed_route_config,
                41,
                43,
            ),
            Ok(())
        );
        assert_eq!(
            ton_verify_transaction_pre_state(
                &governed_boc,
                governed_account_hash,
                emitter,
                governed_code_hash,
                governed_route_config,
                41,
                42,
            ),
            Ok(()),
            "the next transaction may start at the previous transaction end LT"
        );

        // Content opening follows bounded proof wrappers while the governed
        // code identity remains the terminal cell's TON hash zero.
        let mut opened_account = governed.cells[0].clone();
        opened_account.refs = vec![1, 3];
        let code_proof = merkle_proof_cell(
            2,
            governed_computed[1].mask,
            governed_computed[1].hashes[0],
            governed_computed[1].depths[0],
        );
        let opened = TonBoc {
            roots: vec![0],
            cells: vec![
                opened_account,
                code_proof,
                governed.cells[1].clone(),
                governed.cells[2].clone(),
            ],
        };
        let opened_hashes = ton_boc_cell_hashes(&opened).expect("opened account proof hashes");
        let mut nested_account = opened.cells[0].clone();
        nested_account.refs = vec![2, 4];
        let mut nested_code_proof = opened.cells[1].clone();
        nested_code_proof.refs = vec![3];
        let nested = TonBoc {
            roots: vec![0],
            cells: vec![
                merkle_proof_cell(
                    1,
                    opened_hashes[0].mask,
                    opened_hashes[0].hashes[0],
                    opened_hashes[0].depths[0],
                ),
                nested_account,
                nested_code_proof,
                opened.cells[2].clone(),
                opened.cells[3].clone(),
            ],
        };
        assert_eq!(
            ton_verify_transaction_pre_state(
                &serialize_test_boc(&nested),
                opened_hashes[0].hashes[0],
                emitter,
                governed_code_hash,
                governed_route_config,
                41,
                43,
            ),
            Ok(())
        );

        let mut pruned_account = governed.cells[0].clone();
        pruned_account.descriptor |= 1 << 5;
        let pruned_code = TonBoc {
            roots: vec![0],
            cells: vec![
                pruned_account,
                pruned_branch_cell(
                    1,
                    &[governed_computed[1].hashes[0]],
                    &[governed_computed[1].depths[0]],
                ),
                governed.cells[2].clone(),
            ],
        };
        let pruned_hashes = ton_boc_cell_hashes(&pruned_code).expect("pruned code proof hashes");
        assert_eq!(pruned_hashes[0].hashes[0], governed_account_hash);
        assert_eq!(
            ton_verify_transaction_pre_state(
                &serialize_test_boc(&pruned_code),
                governed_account_hash,
                emitter,
                governed_code_hash,
                governed_route_config,
                41,
                43,
            ),
            Ok(())
        );

        // A transaction executed by different code cannot pass merely because
        // a later transaction restores the governed post-state.
        let malicious = account_deployment_boc(emitter, 42, 0x22, [0xb2; 32]);
        let malicious_hash =
            ton_boc_cell_hashes(&malicious).expect("malicious account hashes")[0].hashes[3];
        let malicious_boc = serialize_test_boc(&malicious);
        assert_eq!(
            ton_verify_transaction_pre_state(
                &malicious_boc,
                malicious_hash,
                emitter,
                governed_code_hash,
                governed_route_config,
                41,
                43,
            ),
            Err(TonNativeSourceError::SourceDeploymentMismatch)
        );
        assert_eq!(
            ton_verify_transaction_pre_state(
                &governed_boc,
                malicious_hash,
                emitter,
                governed_code_hash,
                governed_route_config,
                41,
                43,
            ),
            Err(TonNativeSourceError::SourceDeploymentMismatch)
        );
        assert_eq!(
            ton_verify_transaction_pre_state(
                &governed_boc,
                governed_account_hash,
                emitter,
                governed_code_hash,
                governed_route_config,
                42,
                43,
            ),
            Err(TonNativeSourceError::SourceDeploymentMismatch)
        );
        assert_eq!(
            ton_verify_transaction_pre_state(
                &governed_boc,
                governed_account_hash,
                emitter,
                governed_code_hash,
                governed_route_config,
                41,
                41,
            ),
            Err(TonNativeSourceError::SourceDeploymentMismatch)
        );

        // A preloaded active account can legitimately emit its first
        // transaction with both prior start/end clocks still at zero.
        let never_transacted = account_deployment_boc(emitter, 0, 0x11, governed_route_config);
        let never_transacted_computed =
            ton_boc_cell_hashes(&never_transacted).expect("preloaded account hashes");
        let never_transacted_hash = never_transacted_computed[0].hashes[3];
        let never_transacted_code_hash = never_transacted_computed[1].hashes[3];
        assert_eq!(
            ton_verify_transaction_pre_state(
                &serialize_test_boc(&never_transacted),
                never_transacted_hash,
                emitter,
                never_transacted_code_hash,
                governed_route_config,
                0,
                1,
            ),
            Ok(())
        );
    }

    #[test]
    fn source_work_estimate_rejects_expensive_shapes_before_crypto() {
        let mut proof = work_estimate_fixture();
        proof.finality.blocks.clear();
        assert_eq!(
            ton_native_source_work_estimate(&proof),
            Err(TonNativeSourceError::ResourceLimit)
        );

        proof = work_estimate_fixture();
        proof.finality.blocks[0].block_proof_boc = vec![0; TON_MAX_BOC_BYTES + 1];
        assert_eq!(
            ton_native_source_work_estimate(&proof),
            Err(TonNativeSourceError::ResourceLimit)
        );

        proof = work_estimate_fixture();
        let TonBlockSignaturesV1::Ordinary(signatures) = &mut proof.finality.blocks[0].signatures
        else {
            unreachable!("fixture uses ordinary signatures")
        };
        signatures.signatures[0].signature.pop();
        assert_eq!(
            ton_native_source_work_estimate(&proof),
            Err(TonNativeSourceError::InvalidSignatures)
        );

        proof = work_estimate_fixture();
        proof.event.transaction_pre_state_proof_boc = vec![0; TON_MAX_BOC_BYTES + 1];
        assert_eq!(
            ton_native_source_work_estimate(&proof),
            Err(TonNativeSourceError::ResourceLimit)
        );

        proof = work_estimate_fixture();
        proof.event.shard_state_proof_boc = vec![0; TON_MAX_BOC_BYTES + 1];
        assert_eq!(
            ton_native_source_work_estimate(&proof),
            Err(TonNativeSourceError::ResourceLimit)
        );
    }

    #[test]
    fn breaker_work_estimate_charges_both_account_openings_before_parsing() {
        let mut proof = breaker_work_estimate_fixture();
        let base = ton_native_finality_work_estimate(&proof.finality).expect("fixture finality");
        let estimate =
            ton_breaker_observation_work_estimate(&proof).expect("bounded breaker proof");
        assert_eq!(
            estimate.ed25519_signature_checks,
            base.ed25519_signature_checks
        );
        assert_eq!(
            estimate.validator_key_checks_upper_bound,
            base.validator_key_checks_upper_bound
        );
        assert_eq!(
            estimate.framed_boc_bytes,
            base.framed_boc_bytes + 3 + 5 + 7 + 11
        );

        proof.route_account.shard_state_proof_boc = vec![0; TON_MAX_BOC_BYTES + 1];
        assert_eq!(
            ton_breaker_observation_work_estimate(&proof),
            Err(TonNativeSourceError::ResourceLimit)
        );
    }

    #[test]
    fn breaker_proof_binary_and_json_roundtrip_preserve_both_openings() {
        let proof = breaker_work_estimate_fixture();
        let encoded = norito::encode_canonical(&proof).expect("TON breaker proof encodes");
        assert_eq!(
            norito::decode_canonical::<SccpTonBreakerObservationProofV1>(&encoded)
                .expect("TON breaker proof decodes"),
            proof
        );
        let json = norito::json::to_json(&proof).expect("TON breaker proof JSON encodes");
        assert!(json.contains("route_account"));
        assert!(json.contains("jetton_master_account"));
        assert_eq!(
            norito::json::from_json::<SccpTonBreakerObservationProofV1>(&json)
                .expect("TON breaker proof JSON decodes"),
            proof
        );
    }

    #[test]
    fn breaker_account_readback_preserves_authenticated_shard_registration() {
        let address = SccpTonAddressV1 {
            workchain: SCCP_TON_BASECHAIN_WORKCHAIN_V1,
            account: [0x31; 32],
        };
        let boc = account_deployment_boc(address, 43, 0x51, [0x61; 32]);
        let computed = ton_boc_cell_hashes(&boc).expect("account proof hashes");
        let shard_block_id = TonBlockIdExtV1 {
            workchain: SCCP_TON_BASECHAIN_WORKCHAIN_V1,
            shard: SCCP_TON_MASTERCHAIN_SHARD_V1,
            seqno: 17,
            root_hash: [0x71; 32],
            file_hash: [0x72; 32],
        };
        let opened = ton_parse_active_account_state(
            &boc,
            &computed,
            0,
            address,
            shard_block_id,
            16,
            [0x73; 32],
            [0x74; 32],
            42,
        )
        .expect("active account readback");
        assert_eq!(opened.readback.address, address);
        assert_eq!(opened.readback.shard_block_id, shard_block_id);
        assert_eq!(opened.readback.registered_masterchain_seqno, 16);
        assert_eq!(opened.readback.storage_last_transaction_lt, 43);
        assert_eq!(opened.readback.account_state_hash, computed[0].hashes[0]);
        assert_eq!(opened.readback.code_hash, computed[1].hashes[0]);
        assert_eq!(opened.readback.data_hash, computed[2].hashes[0]);
    }

    #[test]
    fn ordinary_signatures_require_unique_strictly_more_than_two_thirds_weight() {
        let block = fixture_block();
        let transcript = ton_block_id_tl_bytes(block);
        let fixtures = (1_u8..=3)
            .map(|seed| fixture_validator(seed, 1))
            .collect::<Vec<_>>();
        let validators = fixtures
            .iter()
            .map(|(_, validator)| *validator)
            .collect::<Vec<_>>();
        let hash = ton_validator_list_hash_short_v1(9, &validators).expect("fixture set hash");
        let active = TonValidatorSetV1 {
            catchain_seqno: 9,
            validator_list_hash_short: hash,
            validators,
        };
        let entries = fixtures
            .iter()
            .map(|(pair, validator)| signed_entry(pair, *validator, &transcript))
            .collect::<Vec<_>>();
        let proof = |signatures| {
            TonBlockSignaturesV1::Ordinary(TonOrdinaryBlockSignaturesV1 {
                catchain_seqno: 9,
                validator_list_hash_short: hash,
                signatures,
            })
        };

        assert_eq!(
            verify_block_signatures(block, &active, &proof(entries[..2].to_vec())),
            Err(TonNativeSourceError::InvalidSignatures)
        );
        reset_roster_key_parse_count();
        assert_eq!(
            verify_block_signatures(block, &active, &proof(entries.clone())),
            Ok(())
        );
        assert_eq!(
            roster_key_parse_count(),
            active.validators.len(),
            "signature verification must charge one roster-key pass separately from signer checks"
        );
        assert_eq!(
            verify_block_signatures(
                block,
                &active,
                &proof(vec![
                    entries[0].clone(),
                    entries[0].clone(),
                    entries[2].clone()
                ]),
            ),
            Err(TonNativeSourceError::InvalidSignatures)
        );
        let (unknown_pair, unknown_validator) = fixture_validator(9, 1);
        let unknown = signed_entry(&unknown_pair, unknown_validator, &transcript);
        assert_eq!(
            verify_block_signatures(
                block,
                &active,
                &proof(vec![entries[0].clone(), entries[1].clone(), unknown]),
            ),
            Err(TonNativeSourceError::InvalidSignatures)
        );
        let mut corrupted = entries;
        corrupted[2].signature[0] ^= 0x80;
        assert_eq!(
            verify_block_signatures(block, &active, &proof(corrupted)),
            Err(TonNativeSourceError::InvalidSignatures)
        );
    }

    #[test]
    fn native_masterchain_continuation_accepts_ordinary_and_simplex_finality() {
        let (pair, validator) = fixture_validator(1, 1);
        let validators = vec![validator];
        let active = TonValidatorSetV1 {
            catchain_seqno: 9,
            validator_list_hash_short: ton_validator_list_hash_short_v1(9, &validators)
                .expect("fixture set hash"),
            validators,
        };
        let checkpoint = TonBlockIdExtV1 {
            workchain: SCCP_TON_MASTERCHAIN_WORKCHAIN_V1,
            shard: SCCP_TON_MASTERCHAIN_SHARD_V1,
            seqno: 41,
            root_hash: [0x11; 32],
            file_hash: [0x22; 32],
        };
        let (block, block_proof_boc, checkpoint_state_root, expected_new_state) =
            masterchain_continuation_fixture(checkpoint, &active);
        let anchor = TonNativeAnchorV1 {
            version: 1,
            network: SccpNetworkV1::TonMainnet,
            zero_state: ton_expected_zero_state(SccpNetworkV1::TonMainnet)
                .expect("mainnet zero state"),
            checkpoint,
            checkpoint_state_root,
            active_validator_set: active.clone(),
            pending_validator_config: None,
        };
        let anchor_hash = ton_native_anchor_hash_v1(&anchor).expect("valid anchor hash");
        let ordinary_transcript = ton_block_id_tl_bytes(block);
        let ordinary = TonBlockSignaturesV1::Ordinary(TonOrdinaryBlockSignaturesV1 {
            catchain_seqno: active.catchain_seqno,
            validator_list_hash_short: active.validator_list_hash_short,
            signatures: vec![signed_entry(&pair, validator, &ordinary_transcript)],
        });
        let proof_with = |signatures| TonNativeFinalityProofV1 {
            version: 1,
            anchor: anchor.clone(),
            blocks: vec![TonMasterchainBlockProofV1 {
                block_id: block,
                block_proof_boc: block_proof_boc.clone(),
                signatures,
            }],
        };

        reset_roster_key_parse_count();
        let ordinary_verified = verify_masterchain_finality(
            &proof_with(ordinary),
            SccpNetworkV1::TonMainnet,
            anchor_hash,
        )
        .expect("ordinary finality");
        assert_eq!(ordinary_verified.block_id, block);
        assert_eq!(ordinary_verified.gen_utime, 1);
        assert_eq!(
            ton_proven_root_hash(
                &ordinary_verified.boc,
                &ton_boc_cell_hashes(&ordinary_verified.boc).expect("verified block hashes"),
                ordinary_verified.boc.roots[0],
            ),
            Some(block.root_hash)
        );
        assert_eq!(roster_key_parse_count(), 2);

        let candidate_data = simplex_candidate_without_parents(block);
        let unsigned_simplex = TonSimplexBlockSignaturesV1 {
            catchain_seqno: active.catchain_seqno,
            validator_list_hash_short: active.validator_list_hash_short,
            session_id: [0x44; 32],
            slot: 7,
            candidate_data,
            signatures: Vec::new(),
        };
        let simplex_transcript =
            simplex_finality_transcript(block, &unsigned_simplex).expect("Simplex transcript");
        let simplex = TonBlockSignaturesV1::Simplex(TonSimplexBlockSignaturesV1 {
            signatures: vec![signed_entry(&pair, validator, &simplex_transcript)],
            ..unsigned_simplex
        });
        let simplex_verified = verify_masterchain_finality(
            &proof_with(simplex.clone()),
            SccpNetworkV1::TonMainnet,
            anchor_hash,
        )
        .expect("Simplex finality");
        assert_eq!(simplex_verified.block_id, block);

        let TonBlockSignaturesV1::Simplex(mut wrong_session) = simplex else {
            unreachable!("fixture uses Simplex signatures")
        };
        wrong_session.session_id[0] ^= 1;
        assert!(matches!(
            verify_masterchain_finality(
                &proof_with(TonBlockSignaturesV1::Simplex(wrong_session)),
                SccpNetworkV1::TonMainnet,
                anchor_hash,
            ),
            Err(TonNativeSourceError::InvalidSignatures)
        ));

        assert_ne!(checkpoint_state_root, expected_new_state);
    }

    #[test]
    fn simplex_transcript_is_exact_and_slot_is_a_nonnegative_tl_int() {
        let block = fixture_block();
        let candidate_data = simplex_candidate_without_parents(block);
        assert_eq!(parse_simplex_candidate_data(&candidate_data), Some(block));
        let mut signatures = TonSimplexBlockSignaturesV1 {
            catchain_seqno: 1,
            validator_list_hash_short: 2,
            session_id: [0x44; 32],
            slot: u32::MAX >> 1,
            candidate_data: candidate_data.clone(),
            signatures: Vec::new(),
        };
        let transcript = simplex_finality_transcript(block, &signatures).expect("valid transcript");
        assert_eq!(transcript.len(), 84);
        assert_eq!(
            &transcript[..4],
            &TON_CONSENSUS_DATA_TO_SIGN_TL_CONSTRUCTOR.to_le_bytes()
        );
        assert_eq!(&transcript[4..36], &signatures.session_id);
        assert_eq!(transcript[36], 44);
        assert_eq!(
            &transcript[37..41],
            &TON_CONSENSUS_SIMPLEX_FINALIZE_TL_CONSTRUCTOR.to_le_bytes()
        );
        assert_eq!(
            &transcript[41..45],
            &TON_CONSENSUS_CANDIDATE_ID_TL_CONSTRUCTOR.to_le_bytes()
        );
        assert_eq!(&transcript[45..49], &i32::MAX.to_le_bytes());
        assert_eq!(&transcript[81..], &[0, 0, 0]);

        signatures.slot = (u32::MAX >> 1) + 1;
        assert_eq!(simplex_finality_transcript(block, &signatures), None);
        signatures.slot = 1;
        signatures.candidate_data.push(0);
        assert_eq!(simplex_finality_transcript(block, &signatures), None);

        let mut boxed_nested = candidate_data;
        boxed_nested.splice(
            4..4,
            TON_BLOCK_ID_EXT_TL_CONSTRUCTOR.to_le_bytes().into_iter(),
        );
        assert_eq!(parse_simplex_candidate_data(&boxed_nested), None);
    }

    #[test]
    fn boc_parser_matches_empty_cell_hash_and_crc_vectors() {
        const EMPTY: &[u8] = &[
            0xb5, 0xee, 0x9c, 0x72, 0x01, 0x01, 0x01, 0x01, 0x00, 0x02, 0x00, 0x00, 0x00,
        ];
        const EMPTY_WITH_CRC: &[u8] = &[
            0xb5, 0xee, 0x9c, 0x72, 0x41, 0x01, 0x01, 0x01, 0x00, 0x02, 0x00, 0x00, 0x00, 0x4c,
            0xac, 0xb9, 0xcd,
        ];
        let expected = hex32("96a296d224f285c67bee93c30f8a309157f0daa35dc5b87e410b78630a09cfc7");
        assert_eq!(ton_boc_single_root_hash_v1(EMPTY), Some(expected));
        assert_eq!(ton_boc_single_ordinary_root_hash_v1(EMPTY), Some(expected));
        let mut proof_data = vec![3];
        proof_data.extend_from_slice(&expected);
        proof_data.extend_from_slice(&0_u16.to_be_bytes());
        let merkle_proof = serialize_test_boc(&TonBoc {
            roots: vec![0],
            cells: vec![
                TonBocCell {
                    descriptor: 0x09,
                    data_descriptor: 70,
                    data: proof_data,
                    refs: vec![1],
                    exotic: true,
                },
                ordinary_cell(Vec::new(), Vec::new()),
            ],
        });
        assert_eq!(ton_boc_single_root_hash_v1(&merkle_proof), Some(expected));
        assert_eq!(ton_boc_single_ordinary_root_hash_v1(&merkle_proof), None);
        let unused_cell = serialize_test_boc(&TonBoc {
            roots: vec![0],
            cells: vec![
                ordinary_cell(Vec::new(), Vec::new()),
                ordinary_cell(vec![0x42], Vec::new()),
            ],
        });
        assert_eq!(ton_boc_single_root_hash_v1(&unused_cell), Some(expected));
        assert_eq!(ton_boc_single_ordinary_root_hash_v1(&unused_cell), None);
        let code_boc = serialize_test_boc(&TonBoc {
            roots: vec![0],
            cells: vec![ordinary_cell(vec![0x11], Vec::new())],
        });
        let data_boc = serialize_test_boc(&TonBoc {
            roots: vec![0],
            cells: vec![ordinary_cell(vec![0x22], Vec::new())],
        });
        let mut state_init_bits = TestBits::default();
        state_init_bits.bit(false); // split_depth absent
        state_init_bits.bit(false); // special absent
        state_init_bits.bit(true); // code reference present
        state_init_bits.bit(true); // data reference present
        state_init_bits.bit(false); // empty library
        let state_init_boc = serialize_test_boc(&TonBoc {
            roots: vec![0],
            cells: vec![
                state_init_bits.cell(vec![1, 2]),
                ordinary_cell(vec![0x11], Vec::new()),
                ordinary_cell(vec![0x22], Vec::new()),
            ],
        });
        let state_init_hash =
            ton_state_init_address_hash_v1(&code_boc, &data_boc).expect("canonical StateInit hash");
        assert_eq!(
            ton_boc_single_ordinary_root_hash_v1(&state_init_boc),
            Some(state_init_hash)
        );
        assert_ne!(
            ton_state_init_address_hash_v1(&data_boc, &code_boc),
            Some(state_init_hash)
        );
        assert_eq!(ton_boc_single_root_hash_v1(EMPTY_WITH_CRC), Some(expected));
        let mut corrupted = EMPTY_WITH_CRC.to_vec();
        *corrupted.last_mut().expect("fixture crc") ^= 1;
        assert_eq!(ton_boc_single_root_hash_v1(&corrupted), None);
        let mut trailing = EMPTY.to_vec();
        trailing.push(0);
        assert_eq!(ton_boc_single_root_hash_v1(&trailing), None);
    }

    #[test]
    fn canonical_initial_storage_hashes_match_exact_tolk_layout() {
        fn hash_depth(boc: &TonBoc, index: usize) -> TonCellHashDepth {
            let computed = ton_boc_cell_hashes(boc).expect("valid fixture cell graph");
            ton_opened_hash_depth(boc, &computed, index).expect("fixture hash and depth")
        }

        fn empty_forest_bits() -> TestBits {
            let mut bits = TestBits::default();
            bits.bit(false);
            bits.uint(0, 64);
            bits.uint(0, 64);
            bits
        }

        let route_configuration_hash = [0x31; 32];
        let config_cell = ordinary_cell(vec![0xa1], Vec::new());
        let metadata_cell = ordinary_cell(vec![0xa2], Vec::new());
        let route_code_cell = ordinary_cell(vec![0xa3], Vec::new());
        let master_code_cell = ordinary_cell(vec![0xa4], Vec::new());
        let bridge_config = hash_depth(
            &TonBoc {
                roots: vec![0],
                cells: vec![config_cell.clone()],
            },
            0,
        );
        let master_metadata = hash_depth(
            &TonBoc {
                roots: vec![0],
                cells: vec![metadata_cell.clone()],
            },
            0,
        );
        let route_code = hash_depth(
            &TonBoc {
                roots: vec![0],
                cells: vec![route_code_cell.clone()],
            },
            0,
        );
        let master_code = hash_depth(
            &TonBoc {
                roots: vec![0],
                cells: vec![master_code_cell.clone()],
            },
            0,
        );

        let mut route_bits = TestBits::default();
        route_bits.uint(u64::from(SCCP_V1_TON_STORAGE_VERSION), 8);
        route_bits.bytes(&route_configuration_hash);
        route_bits.bytes(&bridge_config.hash);
        route_bits.bit(false);
        let mut pending_bits = TestBits::default();
        pending_bits.bit(false);
        pending_bits.bit(false);
        pending_bits.uint(0, 16);
        pending_bits.uint(0, 16);
        let route_data_boc = TonBoc {
            roots: vec![0],
            cells: vec![
                route_bits.cell(vec![1, 2, 3]),
                config_cell.clone(),
                TestBits::default().cell(vec![4, 5]),
                pending_bits.cell(Vec::new()),
                empty_forest_bits().cell(Vec::new()),
                empty_forest_bits().cell(Vec::new()),
            ],
        };
        let route_initial_data = hash_depth(&route_data_boc, 0);
        assert_eq!(
            ton_canonical_route_initial_data_hash_depth_v1(route_configuration_hash, bridge_config,),
            Some(route_initial_data)
        );

        let route_address = SccpTonAddressV1 {
            workchain: SCCP_TON_BASECHAIN_WORKCHAIN_V1,
            account: ton_state_init_hash_from_children(route_code, route_initial_data)
                .expect("route StateInit")
                .hash,
        };
        let mut master_bits = TestBits::default();
        master_bits.uint(u64::from(SCCP_V1_TON_STORAGE_VERSION), 8);
        master_bits.bytes(&route_configuration_hash);
        master_bits.bytes(&bridge_config.hash);
        master_bits.uint(0, 4);
        master_bits.bit(true);
        master_bits.bit(false);
        master_bits.bit(false);
        master_bits.uint(
            u64::from(
                i8::try_from(route_address.workchain)
                    .expect("basechain workchain")
                    .to_be_bytes()[0],
            ),
            8,
        );
        master_bits.bytes(&route_address.account);
        master_bits.bit(false);
        master_bits.uint(0, 16);
        master_bits.bit(false);
        let master_data_boc = TonBoc {
            roots: vec![0],
            cells: vec![
                master_bits.cell(vec![1, 2, 3]),
                metadata_cell.clone(),
                config_cell.clone(),
                TestBits::default().cell(vec![4, 5]),
                empty_forest_bits().cell(Vec::new()),
                empty_forest_bits().cell(Vec::new()),
            ],
        };
        let master_initial_data = hash_depth(&master_data_boc, 0);
        assert_eq!(
            ton_canonical_master_initial_data_hash_depth_v1(
                route_configuration_hash,
                bridge_config,
                master_metadata,
                route_address,
            ),
            Some(master_initial_data)
        );

        let bindings = ton_canonical_deployment_bindings_v1(
            route_configuration_hash,
            bridge_config,
            master_metadata,
            route_code,
            master_code,
        )
        .expect("canonical route/master bindings");
        assert_eq!(bindings.route_initial_data, route_initial_data);
        assert_eq!(bindings.route_address, route_address);
        assert_eq!(bindings.master_initial_data, master_initial_data);
        assert_eq!(
            bindings.route_address.account,
            ton_state_init_address_hash_v1(
                &serialize_test_boc(&TonBoc {
                    roots: vec![0],
                    cells: vec![route_code_cell],
                }),
                &serialize_test_boc(&route_data_boc),
            )
            .expect("route StateInit BOC parity")
        );
        assert_eq!(
            bindings.master_address.account,
            ton_state_init_address_hash_v1(
                &serialize_test_boc(&TonBoc {
                    roots: vec![0],
                    cells: vec![master_code_cell],
                }),
                &serialize_test_boc(&master_data_boc),
            )
            .expect("master StateInit BOC parity")
        );

        let changed_route_code = TonCellHashDepth {
            hash: [0xb1; 32],
            ..route_code
        };
        let changed = ton_canonical_deployment_bindings_v1(
            route_configuration_hash,
            bridge_config,
            master_metadata,
            changed_route_code,
            master_code,
        )
        .expect("changed route code remains structurally valid");
        assert_eq!(changed.route_initial_data, bindings.route_initial_data);
        assert_ne!(changed.route_address, bindings.route_address);
        assert_ne!(changed.master_initial_data, bindings.master_initial_data);
        assert_ne!(changed.master_address, bindings.master_address);

        let changed_metadata = TonCellHashDepth {
            hash: [0xb2; 32],
            ..master_metadata
        };
        let changed = ton_canonical_deployment_bindings_v1(
            route_configuration_hash,
            bridge_config,
            changed_metadata,
            route_code,
            master_code,
        )
        .expect("changed metadata remains structurally valid");
        assert_eq!(changed.route_address, bindings.route_address);
        assert_ne!(changed.master_initial_data, bindings.master_initial_data);
        assert_ne!(changed.master_address, bindings.master_address);

        let changed_master_code = TonCellHashDepth {
            hash: [0xb3; 32],
            ..master_code
        };
        let changed = ton_canonical_deployment_bindings_v1(
            route_configuration_hash,
            bridge_config,
            master_metadata,
            route_code,
            changed_master_code,
        )
        .expect("changed master code remains structurally valid");
        assert_eq!(changed.route_address, bindings.route_address);
        assert_eq!(changed.master_initial_data, bindings.master_initial_data);
        assert_ne!(changed.master_address, bindings.master_address);
    }

    #[test]
    fn breaker_boc_gate_accepts_only_one_canonical_representation() {
        const EMPTY: &[u8] = &[
            0xb5, 0xee, 0x9c, 0x72, 0x01, 0x01, 0x01, 0x01, 0x00, 0x02, 0x00, 0x00, 0x00,
        ];
        const EMPTY_WITH_CRC: &[u8] = &[
            0xb5, 0xee, 0x9c, 0x72, 0x41, 0x01, 0x01, 0x01, 0x00, 0x02, 0x00, 0x00, 0x00, 0x4c,
            0xac, 0xb9, 0xcd,
        ];
        const NONMINIMAL_SIZE_WIDTH: &[u8] = &[
            0xb5, 0xee, 0x9c, 0x72, 0x02, 0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x02, 0x00,
            0x00, 0x00, 0x00,
        ];
        const NONMINIMAL_OFFSET_WIDTH: &[u8] = &[
            0xb5, 0xee, 0x9c, 0x72, 0x01, 0x02, 0x01, 0x01, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00,
            0x00,
        ];
        let expected = hex32("96a296d224f285c67bee93c30f8a309157f0daa35dc5b87e410b78630a09cfc7");
        assert_eq!(ton_canonical_boc_single_root_hash_v1(EMPTY), Some(expected));
        assert!(parse_ton_boc(EMPTY_WITH_CRC).is_some());
        assert!(parse_ton_boc(NONMINIMAL_SIZE_WIDTH).is_some());
        assert!(parse_ton_boc(NONMINIMAL_OFFSET_WIDTH).is_some());
        assert_eq!(ton_canonical_boc_single_root_hash_v1(EMPTY_WITH_CRC), None);
        assert_eq!(
            ton_canonical_boc_single_root_hash_v1(NONMINIMAL_SIZE_WIDTH),
            None
        );
        assert_eq!(
            ton_canonical_boc_single_root_hash_v1(NONMINIMAL_OFFSET_WIDTH),
            None
        );

        let mut unreachable = serialize_test_boc(&TonBoc {
            roots: vec![0],
            cells: vec![
                ordinary_cell(Vec::new(), Vec::new()),
                ordinary_cell(vec![0x42], Vec::new()),
            ],
        });
        assert!(parse_ton_boc(&unreachable).is_some());
        assert_eq!(ton_canonical_boc_single_root_hash_v1(&unreachable), None);
        // Selecting the second cell as root leaves the first one unreachable as
        // well as violating canonical root index zero.
        unreachable[10] = 1;
        assert!(parse_ton_boc(&unreachable).is_some());
        assert_eq!(ton_canonical_boc_single_root_hash_v1(&unreachable), None);
    }

    #[test]
    fn breaker_boc_gate_rejects_duplicate_and_alternate_dags() {
        let duplicate = serialize_test_boc(&TonBoc {
            roots: vec![0],
            cells: vec![
                ordinary_cell(vec![0x10], vec![1, 2]),
                ordinary_cell(vec![0x20], Vec::new()),
                ordinary_cell(vec![0x20], Vec::new()),
            ],
        });
        assert!(parse_ton_boc(&duplicate).is_some());
        assert_eq!(ton_canonical_boc_single_root_hash_v1(&duplicate), None);

        let alternate = serialize_test_boc(&TonBoc {
            roots: vec![0],
            cells: vec![
                // Logical reference order is A then B, but the cell table puts
                // B before A. Both parents share the same canonical child.
                ordinary_cell(vec![0x10], vec![2, 1]),
                ordinary_cell(vec![0xb0], vec![3]),
                ordinary_cell(vec![0xa0], vec![3]),
                ordinary_cell(vec![0xc0], Vec::new()),
            ],
        });
        assert!(parse_ton_boc(&alternate).is_some());
        assert_eq!(ton_canonical_boc_single_root_hash_v1(&alternate), None);

        let canonical_graph = TonBoc {
            roots: vec![0],
            cells: vec![
                ordinary_cell(vec![0x10], vec![1, 2]),
                ordinary_cell(vec![0xa0], vec![3]),
                ordinary_cell(vec![0xb0], vec![3]),
                ordinary_cell(vec![0xc0], Vec::new()),
            ],
        };
        let canonical = serialize_test_boc(&canonical_graph);
        assert_eq!(
            encode_canonical_ton_boc(&canonical_graph, 0),
            Some(canonical.clone())
        );
        assert!(ton_canonical_boc_single_root_hash_v1(&canonical).is_some());
    }

    #[test]
    fn breaker_boc_gate_enforces_raw_level_masks_tuple_counts_and_max_depth() {
        let sparse = TonBoc {
            roots: vec![0],
            cells: vec![pruned_branch_cell(0x02, &[[0x41; 32]], &[7])],
        };
        assert!(
            ton_boc_cell_hashes(&sparse).is_some(),
            "one set level carries exactly one stored hash/depth tuple"
        );

        let extra_tuple = TonBoc {
            roots: vec![0],
            cells: vec![pruned_branch_cell(0x02, &[[0x41; 32], [0x42; 32]], &[7, 8])],
        };
        assert!(ton_boc_cell_hashes(&extra_tuple).is_none());

        let mut high_bit_alias = pruned_branch_cell(0x01, &[[0x51; 32]], &[9]);
        high_bit_alias.data[1] |= 0x08;
        let high_bit_alias = TonBoc {
            roots: vec![0],
            cells: vec![high_bit_alias],
        };
        assert!(ton_boc_cell_hashes(&high_bit_alias).is_none());

        let excessive_pruned_depth = TonBoc {
            roots: vec![0],
            cells: vec![pruned_branch_cell(
                0x01,
                &[[0x61; 32]],
                &[TON_MAX_CELL_DEPTH + 1],
            )],
        };
        assert!(ton_boc_cell_hashes(&excessive_pruned_depth).is_none());

        let cells = (0..=usize::from(TON_MAX_CELL_DEPTH))
            .map(|index| {
                ordinary_cell(
                    Vec::new(),
                    (index < usize::from(TON_MAX_CELL_DEPTH))
                        .then_some(index + 1)
                        .into_iter()
                        .collect(),
                )
            })
            .collect();
        let boundary = TonBoc {
            roots: vec![0],
            cells,
        };
        assert!(ton_boc_cell_hashes(&boundary).is_some());

        let excessive = TonBoc {
            roots: vec![0],
            cells: (0..=usize::from(TON_MAX_CELL_DEPTH) + 1)
                .map(|index| {
                    ordinary_cell(
                        Vec::new(),
                        (index <= usize::from(TON_MAX_CELL_DEPTH))
                            .then_some(index + 1)
                            .into_iter()
                            .collect(),
                    )
                })
                .collect(),
        };
        assert!(ton_boc_cell_hashes(&excessive).is_none());
    }

    #[test]
    fn breaker_boc_gate_rejects_tail_alias_legacy_pruning_and_nested_wrappers() {
        let zero_bit_alias = [
            0xb5, 0xee, 0x9c, 0x72, 0x01, 0x01, 0x01, 0x01, 0x00, 0x03, 0x00, 0x00, 0x01, 0x80,
        ];
        assert!(parse_ton_boc(&zero_bit_alias).is_some());
        assert_eq!(ton_canonical_boc_single_root_hash_v1(&zero_bit_alias), None);

        let byte_aligned_tail_alias = serialize_test_boc(&TonBoc {
            roots: vec![0],
            cells: vec![TonBocCell {
                descriptor: 0,
                data_descriptor: 3,
                data: vec![0x42, 0x80],
                refs: Vec::new(),
                exotic: false,
            }],
        });
        assert!(parse_ton_boc(&byte_aligned_tail_alias).is_some());
        assert_eq!(
            ton_canonical_boc_single_root_hash_v1(&byte_aligned_tail_alias),
            None
        );

        let mut root = ordinary_cell(vec![0x41], vec![1]);
        root.descriptor |= 1 << 5;
        let mut legacy_data = vec![1];
        legacy_data.extend_from_slice(&[0x51; 32]);
        legacy_data.extend_from_slice(&1_u16.to_be_bytes());
        let legacy = serialize_test_boc(&TonBoc {
            roots: vec![0],
            cells: vec![
                root,
                TonBocCell {
                    descriptor: 0x08 | (1 << 5),
                    data_descriptor: 70,
                    data: legacy_data,
                    refs: Vec::new(),
                    exotic: true,
                },
            ],
        });
        assert!(parse_single_root_boc(&legacy).is_some());
        assert_eq!(ton_canonical_boc_single_root_hash_v1(&legacy), None);

        let leaf = TonBoc {
            roots: vec![0],
            cells: vec![ordinary_cell(vec![0x42], Vec::new())],
        };
        let leaf_hashes = ton_boc_cell_hashes(&leaf).expect("leaf hashes");
        let inner = TonBoc {
            roots: vec![0],
            cells: vec![
                merkle_proof_cell(
                    1,
                    leaf_hashes[0].mask,
                    leaf_hashes[0].hashes[0],
                    leaf_hashes[0].depths[0],
                ),
                ordinary_cell(vec![0x42], Vec::new()),
            ],
        };
        let inner_hashes = ton_boc_cell_hashes(&inner).expect("inner hashes");
        let nested = serialize_test_boc(&TonBoc {
            roots: vec![0],
            cells: vec![
                merkle_proof_cell(
                    1,
                    inner_hashes[0].mask,
                    inner_hashes[0].hashes[0],
                    inner_hashes[0].depths[0],
                ),
                merkle_proof_cell(
                    2,
                    leaf_hashes[0].mask,
                    leaf_hashes[0].hashes[0],
                    leaf_hashes[0].depths[0],
                ),
                ordinary_cell(vec![0x42], Vec::new()),
            ],
        });
        assert!(parse_single_root_boc(&nested).is_some());
        assert_eq!(ton_canonical_boc_single_root_hash_v1(&nested), None);
    }

    #[test]
    fn breaker_storage_amounts_are_minimal_and_below_two_to_the_120() {
        let mut maximum = TestBits::default();
        maximum.uint(15, 4);
        maximum.bytes(&[0xff; 15]);
        let maximum = maximum.cell(Vec::new());
        let mut reader = TonBitReader::new(&maximum).expect("maximum coins reader");
        assert_eq!(
            ton_read_canonical_coins(&mut reader),
            Some((1_u128 << 120) - 1)
        );
        assert!(reader.exhausted());

        let mut zero = TestBits::default();
        zero.uint(0, 4);
        let zero = zero.cell(Vec::new());
        let mut reader = TonBitReader::new(&zero).expect("zero coins reader");
        assert_eq!(ton_read_canonical_coins(&mut reader), Some(0));
        assert!(reader.exhausted());

        let mut nonminimal = TestBits::default();
        nonminimal.uint(2, 4);
        nonminimal.bytes(&[0, 1]);
        let nonminimal = nonminimal.cell(Vec::new());
        let mut reader = TonBitReader::new(&nonminimal).expect("nonminimal coins reader");
        assert_eq!(ton_read_canonical_coins(&mut reader), None);
    }

    #[test]
    fn breaker_replay_forest_readback_binds_root_count_and_sequence() {
        let replay = |root_present: bool, leaf_count: u64, update_sequence: u64, trailing: bool| {
            let mut bits = TestBits::default();
            bits.bit(root_present);
            bits.uint(leaf_count, 64);
            bits.uint(update_sequence, 64);
            if trailing {
                bits.bit(false);
            }
            TonBoc {
                roots: vec![0],
                cells: if root_present {
                    vec![bits.cell(vec![1]), ordinary_cell(vec![0x55], Vec::new())]
                } else {
                    vec![bits.cell(Vec::new())]
                },
            }
        };

        let empty = replay(false, 0, 0, false);
        let empty_hashes = ton_boc_cell_hashes(&empty).expect("empty replay hashes");
        assert_eq!(
            ton_parse_replay_forest_readback(&empty, &empty_hashes, 0),
            Some(TonReplayForestReadbackV1 {
                nonempty_shard_roots_hash: None,
                leaf_count: 0,
                update_sequence: 0,
            })
        );

        let occupied = replay(true, 1, 1, false);
        let occupied_hashes = ton_boc_cell_hashes(&occupied).expect("occupied replay hashes");
        let occupied_readback = ton_parse_replay_forest_readback(&occupied, &occupied_hashes, 0)
            .expect("occupied replay readback");
        assert_eq!(occupied_readback.leaf_count, 1);
        assert_eq!(occupied_readback.update_sequence, 1);
        assert_eq!(
            occupied_readback.nonempty_shard_roots_hash,
            Some(occupied_hashes[1].hashes[0])
        );

        for invalid in [
            replay(false, 1, 1, false),
            replay(true, 0, 0, false),
            replay(true, 2, 1, false),
            replay(true, 1, 1, true),
        ] {
            let hashes = ton_boc_cell_hashes(&invalid).expect("invalid replay hashes");
            assert_eq!(ton_parse_replay_forest_readback(&invalid, &hashes, 0), None);
        }
    }

    #[test]
    fn breaker_guardians_are_exactly_five_nonzero_sorted_keys() {
        let guardians = |keys: [H256; 5], trailing: bool| {
            let mut head = TestBits::default();
            for key in &keys[..3] {
                head.bytes(key);
            }
            let mut tail = TestBits::default();
            for key in &keys[3..] {
                tail.bytes(key);
            }
            if trailing {
                tail.bit(false);
            }
            TonBoc {
                roots: vec![0],
                cells: vec![head.cell(vec![1]), tail.cell(Vec::new())],
            }
        };
        let canonical = [[1; 32], [2; 32], [3; 32], [4; 32], [5; 32]];
        let canonical_boc = guardians(canonical, false);
        assert_eq!(
            ton_parse_mint_breaker_guardians(&canonical_boc, 0),
            Some(canonical.into())
        );

        let mut zero = canonical;
        zero[0] = [0; 32];
        assert_eq!(
            ton_parse_mint_breaker_guardians(&guardians(zero, false), 0),
            None
        );
        let mut reordered = canonical;
        reordered.swap(2, 3);
        assert_eq!(
            ton_parse_mint_breaker_guardians(&guardians(reordered, false), 0),
            None
        );
        assert_eq!(
            ton_parse_mint_breaker_guardians(&guardians(canonical, true), 0),
            None
        );
    }

    #[test]
    fn breaker_signature_envelope_requires_strict_node_id_order() {
        let signature = |id| TonValidatorSignatureV1 {
            node_id_short: [id; 32],
            signature: vec![id; 64],
        };
        let ordinary = |signatures| {
            TonBlockSignaturesV1::Ordinary(TonOrdinaryBlockSignaturesV1 {
                catchain_seqno: 1,
                validator_list_hash_short: 1,
                signatures,
            })
        };
        assert!(ton_block_signatures_are_canonically_ordered(&ordinary(
            vec![signature(1), signature(2), signature(3),]
        )));
        assert!(!ton_block_signatures_are_canonically_ordered(&ordinary(
            vec![signature(1), signature(1),]
        )));
        assert!(!ton_block_signatures_are_canonically_ordered(&ordinary(
            vec![signature(2), signature(1),]
        )));

        let simplex = TonBlockSignaturesV1::Simplex(TonSimplexBlockSignaturesV1 {
            catchain_seqno: 1,
            validator_list_hash_short: 1,
            session_id: [0x44; 32],
            slot: 1,
            candidate_data: vec![1],
            signatures: vec![signature(1), signature(2)],
        });
        assert!(ton_block_signatures_are_canonically_ordered(&simplex));
    }

    #[test]
    fn boc_hash_zero_preserves_original_tree_identity_across_pruned_levels() {
        let complete = TonBoc {
            roots: vec![0],
            cells: vec![
                ordinary_cell(vec![0x41], vec![1]),
                ordinary_cell(vec![0x42], Vec::new()),
            ],
        };
        let complete_hashes = ton_boc_cell_hashes(&complete).expect("complete tree hashes");
        let expected_root = complete_hashes[0].hashes[0];
        let child_hash = complete_hashes[1].hashes[0];
        let child_depth = complete_hashes[1].depths[0];

        for mask in [1_u8, 3, 7] {
            let count = usize::from(ton_level_mask_level(mask));
            let mut root = ordinary_cell(vec![0x41], vec![1]);
            root.descriptor |= mask << 5;
            let stored_hashes = [child_hash; 3];
            let stored_depths = [child_depth; 3];
            let pruned = TonBoc {
                roots: vec![0],
                cells: vec![
                    root,
                    pruned_branch_cell(mask, &stored_hashes[..count], &stored_depths[..count]),
                ],
            };
            let computed = ton_boc_cell_hashes(&pruned).expect("pruned proof hashes");
            assert_eq!(computed[0].hashes[0], expected_root);
            assert_ne!(
                computed[0].hashes[3], expected_root,
                "higher virtual hashes must not replace TON hash-zero identity"
            );
            assert_eq!(
                ton_boc_single_root_hash_v1(&serialize_test_boc(&pruned)),
                Some(expected_root)
            );
        }
    }

    #[test]
    fn virtual_root_resolution_unwraps_nested_merkle_proofs() {
        let leaf = TonBoc {
            roots: vec![0],
            cells: vec![ordinary_cell(vec![0x42], Vec::new())],
        };
        let leaf_hashes = ton_boc_cell_hashes(&leaf).expect("leaf hashes");
        let inner = TonBoc {
            roots: vec![0],
            cells: vec![
                merkle_proof_cell(
                    1,
                    leaf_hashes[0].mask,
                    leaf_hashes[0].hashes[0],
                    leaf_hashes[0].depths[0],
                ),
                ordinary_cell(vec![0x42], Vec::new()),
            ],
        };
        let inner_hashes = ton_boc_cell_hashes(&inner).expect("inner proof hashes");
        let nested = TonBoc {
            roots: vec![0],
            cells: vec![
                merkle_proof_cell(
                    1,
                    inner_hashes[0].mask,
                    inner_hashes[0].hashes[0],
                    inner_hashes[0].depths[0],
                ),
                merkle_proof_cell(
                    2,
                    leaf_hashes[0].mask,
                    leaf_hashes[0].hashes[0],
                    leaf_hashes[0].depths[0],
                ),
                ordinary_cell(vec![0x42], Vec::new()),
            ],
        };
        assert!(ton_boc_cell_hashes(&nested).is_some());
        assert_eq!(ton_virtual_root_index(&nested, 0), Some(2));
        assert_eq!(
            ton_boc_single_root_hash_v1(&serialize_test_boc(&nested)),
            Some(inner_hashes[0].hashes[0])
        );

        let terminal = pruned_branch_cell(7, &[[0x71; 32], [0x72; 32], [0x73; 32]], &[5, 6, 7]);
        let terminal_boc = TonBoc {
            roots: vec![0],
            cells: vec![terminal.clone()],
        };
        let terminal_hashes = ton_boc_cell_hashes(&terminal_boc).expect("terminal proof hashes");
        let inner_cell = merkle_proof_cell(
            1,
            terminal_hashes[0].mask,
            terminal_hashes[0].hashes[0],
            terminal_hashes[0].depths[0],
        );
        let inner_boc = TonBoc {
            roots: vec![0],
            cells: vec![inner_cell.clone(), terminal.clone()],
        };
        let inner_hashes = ton_boc_cell_hashes(&inner_boc).expect("level-three proof hashes");
        let middle_cell = merkle_proof_cell(
            1,
            inner_hashes[0].mask,
            inner_hashes[0].hashes[0],
            inner_hashes[0].depths[0],
        );
        let mut shifted_inner = inner_cell.clone();
        shifted_inner.refs = vec![2];
        let middle_boc = TonBoc {
            roots: vec![0],
            cells: vec![middle_cell.clone(), shifted_inner.clone(), terminal.clone()],
        };
        let middle_hashes = ton_boc_cell_hashes(&middle_boc).expect("level-two proof hashes");
        let outer_cell = merkle_proof_cell(
            1,
            middle_hashes[0].mask,
            middle_hashes[0].hashes[0],
            middle_hashes[0].depths[0],
        );
        let mut shifted_middle = middle_cell;
        shifted_middle.refs = vec![2];
        shifted_inner.refs = vec![3];
        let nested_pruned = TonBoc {
            roots: vec![0],
            cells: vec![outer_cell, shifted_middle, shifted_inner, terminal],
        };
        let nested_hashes =
            ton_boc_cell_hashes(&nested_pruned).expect("nested pruned proof hashes");
        assert_eq!(ton_merkle_opened_index(&nested_pruned, 0), Some(3));
        assert_eq!(ton_virtual_root_index(&nested_pruned, 0), None);
        assert_eq!(
            ton_opened_original_tree_hash(&nested_pruned, &nested_hashes, 0),
            Some([0x71; 32])
        );
    }

    #[test]
    fn transaction_parser_opens_a_merkle_wrapped_hash_update() {
        let account = [0x31; 32];
        let old_account_hash = [0x41; 32];
        let new_account_hash = [0x42; 32];
        let mut transaction = TestBits::default();
        transaction.uint(u64::from(TON_TRANSACTION_CONSTRUCTOR), 4);
        transaction.bytes(&account);
        transaction.uint(10, 64);
        transaction.bytes(&[0x51; 32]);
        transaction.uint(9, 64);
        transaction.uint(1, 32);
        transaction.uint(1, 15);
        transaction.uint(2, 2);
        transaction.uint(2, 2);
        transaction.uint(0, 4); // zero grams
        transaction.bit(false); // no extra currencies

        let mut hash_update_data = vec![0x72];
        hash_update_data.extend_from_slice(&old_account_hash);
        hash_update_data.extend_from_slice(&new_account_hash);
        let hash_update = TonBoc {
            roots: vec![0],
            cells: vec![ordinary_cell(hash_update_data.clone(), Vec::new())],
        };
        let update_hashes = ton_boc_cell_hashes(&hash_update).expect("HashUpdate hashes");
        let mut description = ordinary_cell(Vec::new(), vec![5]);
        description.descriptor |= 7 << 5;
        let mut transaction = transaction.cell(vec![1, 2, 4]);
        transaction.descriptor |= 7 << 5;
        let boc = TonBoc {
            roots: vec![0],
            cells: vec![
                transaction,
                ordinary_cell(Vec::new(), Vec::new()),
                merkle_proof_cell(
                    3,
                    update_hashes[0].mask,
                    update_hashes[0].hashes[0],
                    update_hashes[0].depths[0],
                ),
                ordinary_cell(hash_update_data, Vec::new()),
                description,
                pruned_branch_cell(7, &[[0x81; 32], [0x82; 32], [0x83; 32]], &[1, 2, 3]),
            ],
        };
        let computed = ton_boc_cell_hashes(&boc).expect("transaction proof hashes");
        let parsed = ton_parse_transaction(&boc, &computed, 0, account, 10)
            .expect("wrapped HashUpdate must parse");
        assert_eq!(parsed.old_account_hash, old_account_hash);
        assert_eq!(parsed.new_account_hash, new_account_hash);
        assert_eq!(parsed.hash, computed[0].hashes[0]);
        assert_ne!(parsed.hash, computed[0].hashes[3]);
    }

    #[test]
    fn boc_parser_rejects_noncanonical_intermediate_index_offsets() {
        // Root cell (three serialized bytes) references one empty child (two
        // bytes), so the only canonical cumulative index is [3, 5].
        let indexed = [
            0xb5, 0xee, 0x9c, 0x72, 0x81, 0x01, 0x02, 0x01, 0x00, 0x05, 0x00, 0x03, 0x05, 0x01,
            0x00, 0x01, 0x00, 0x00,
        ];
        assert!(parse_ton_boc(&indexed).is_some());
        let mut malformed = indexed;
        malformed[11] = 2;
        assert_eq!(parse_ton_boc(&malformed), None);
    }

    #[test]
    fn boc_parser_enforces_byte_and_cell_caps_before_allocation() {
        assert_eq!(TON_NATIVE_MAX_BOC_BYTES_V1, 64 * 1024);
        assert_eq!(TON_MAX_BOC_CELLS, 4_096);
        let mut oversized = vec![0_u8; TON_MAX_BOC_BYTES + 1];
        oversized[..4].copy_from_slice(&TON_BOC_MAGIC);
        assert_eq!(parse_ton_boc(&oversized), None);

        // size_bytes=2, offset_bytes=1, cells_count=4097. The parser must
        // reject the declared count before attempting to allocate cell data.
        let excessive_cells = [
            0xb5, 0xee, 0x9c, 0x72, 0x02, 0x01, 0x10, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00,
            0x00,
        ];
        assert_eq!(parse_ton_boc(&excessive_cells), None);
    }

    #[test]
    fn boc_parser_rejects_maximum_multi_root_framing_at_the_header() {
        let count = u16::try_from(TON_MAX_BOC_CELLS).expect("TON cell cap fits u16");
        let total_cells_size = count.checked_mul(2).expect("empty cells fit u16");
        let mut multi_root = TON_BOC_MAGIC.to_vec();
        multi_root.extend_from_slice(&[2, 2]);
        multi_root.extend_from_slice(&count.to_be_bytes());
        multi_root.extend_from_slice(&count.to_be_bytes());
        multi_root.extend_from_slice(&0_u16.to_be_bytes());
        multi_root.extend_from_slice(&total_cells_size.to_be_bytes());
        for root in 0..count {
            multi_root.extend_from_slice(&root.to_be_bytes());
        }
        for _ in 0..count {
            multi_root.extend_from_slice(&[0, 0]);
        }
        assert_eq!(multi_root.len(), 16_398);
        assert_eq!(parse_ton_boc(&multi_root), None);
    }

    #[test]
    fn payload_snake_enforces_fixed_chunks_at_both_size_boundaries() {
        for (length, chunks) in [
            (TON_PAYLOAD_HEADER_BYTES, [50, 0, 0, 0]),
            (208, [50, 100, 58, 0]),
            (250, [50, 100, 100, 0]),
            (TON_MAX_CANONICAL_PAYLOAD_BYTES, [50, 100, 100, 124]),
        ] {
            let payload = (0..length)
                .map(|index| u8::try_from(index % 251).expect("fixture byte"))
                .collect::<Vec<_>>();
            let boc = payload_boc(&payload, chunks);
            assert_eq!(
                ton_read_exact_payload_cells(&boc, 0, payload.len()),
                Some(payload)
            );
        }
    }

    #[test]
    fn payload_snake_rejects_alternate_segmentation_and_trailing_refs() {
        let payload = (0..208)
            .map(|index| u8::try_from(index % 251).expect("fixture byte"))
            .collect::<Vec<_>>();
        let alternate = payload_boc(&payload, [50, 99, 59, 0]);
        assert_eq!(
            ton_read_exact_payload_cells(&alternate, 0, payload.len()),
            None
        );
        let mut trailing_ref = payload_boc(&payload, [50, 100, 58, 0]);
        trailing_ref.cells[3].refs.push(3);
        trailing_ref.cells[3].descriptor = 1;
        assert_eq!(
            ton_read_exact_payload_cells(&trailing_ref, 0, payload.len()),
            None
        );
        let too_large = vec![0_u8; TON_MAX_CANONICAL_PAYLOAD_BYTES + 1];
        let boc = payload_boc(&too_large, [50, 100, 100, 125]);
        assert_eq!(ton_read_exact_payload_cells(&boc, 0, too_large.len()), None);
    }
}
