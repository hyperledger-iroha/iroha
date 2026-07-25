//! Consensus-verifiable Solana Agave source proofs for SCCP.
//!
//! Solana does not expose a compact native finality certificate that another
//! chain can replay independently. The V1 lane therefore admits one governed
//! recursive BN254 Groth16 circuit. Its trust-anchor preimage fixes the exact
//! Solana testnet genesis, an Agave rooted-bank checkpoint, the audited Agave
//! rules and feature set, the recursive circuit and witness generator, and the
//! complete verification key. Governance stores the hash of that preimage;
//! callers cannot select another circuit or key.
//!
//! The circuit statement is deliberately closed. It proves Agave replay from
//! the governed checkpoint to a later rooted bank and proves inclusion and
//! success of exactly one direct route instruction and exactly one matching
//! program-owned burn receipt/event. That instruction is bound to the governed
//! immutable program deployment and to every economic SCCP field. Canonical
//! stake, vote-state, and replay-transcript commitments make the private
//! finality witness independently identifiable without putting its unbounded
//! contents in consensus state.

use alloc::vec::Vec;
use core::fmt;

use iroha_data_model::bridge::{
    SccpGroth16Bn254VerifyingKeyV1, SccpNetworkV1, SccpSourceEmitterV1, SccpSourceIdentityV1,
};

use super::{
    H256, SCCP_CODEC_CANONICAL_TEXT, SCCP_CODEC_SOLANA_PUBKEY32, SCCP_DOMAIN_SOLANA,
    SCCP_DOMAIN_SORA, SCCP_TAIRA_SOL_XOR_ROUTE_ID_V1, SCCP_TAIRA_XOR_ASSET_KEY_V1, SccpPayloadV1,
    canonical_sccp_groth16_bn254_verifying_key_bytes_v1, canonical_sccp_payload_bytes,
    decode_sccp_evm_groth16_bn254_proof_bytes, payload_hash, prefixed_blake2b,
    sccp_groth16_bn254_signal_word, sccp_groth16_bn254_verifying_key_hash_v1, sccp_lane_id_hash_v1,
    sccp_lane_source_event_digest_v1, sccp_message_id, sccp_source_identity_hash_v1,
    verify_sccp_groth16_bn254_pairing_equation_v1, verify_sccp_payload_structure,
};

/// Canonical raw 32-byte genesis hash of Solana testnet
/// (`4uhcVJyU9pJkvQyS88uRDiswHXSCkY3zQawwpjk2NsNY`).
pub const SCCP_SOLANA_TESTNET_GENESIS_HASH_V1: H256 = [
    0x3a, 0x13, 0x2e, 0xce, 0x10, 0x30, 0x5e, 0xc1, 0x83, 0x07, 0x25, 0x50, 0x2f, 0xa2, 0xb7, 0xe7,
    0xeb, 0x81, 0x57, 0xe9, 0x12, 0x3d, 0x4c, 0x1f, 0x65, 0x4a, 0x71, 0x78, 0x71, 0x61, 0xdc, 0x21,
];

/// Exact byte length of a canonical BN254 Groth16 proof envelope.
pub const SCCP_SOLANA_AGAVE_GROTH16_PROOF_BYTES_V1: usize = 12 * 32;
/// Maximum recipient bytes in one proved Solana route instruction.
pub const SCCP_SOLANA_AGAVE_MAX_RECIPIENT_BYTES_V1: usize = 256;

const SOLANA_TESTNET_NETWORK_TAG_V1: u8 = 13;
const SOLANA_AGAVE_ANCHOR_PREFIX_V1: &[u8] = b"sccp:solana:agave-anchor:v1";
const SOLANA_AGAVE_SEMANTIC_PROFILE_PREFIX_V1: &[u8] = b"sccp:solana:agave-semantic-profile:v1";
const SOLANA_AGAVE_SIGNAL_SCHEMA_PREFIX_V1: &[u8] = b"sccp:solana:agave-public-signal-schema:v1";
const SOLANA_AGAVE_TRANSFER_INSTRUCTION_PREFIX_V1: &[u8] =
    b"sccp:solana:agave-transfer-instruction:v1";
const SOLANA_AGAVE_TRANSACTION_SIGNATURE_PREFIX_V1: &[u8] = b"sccp:solana:transaction-signature:v1";

const SIGNAL_ANCHOR_HASH_V1: &[u8] = b"sccp:solana:signal:anchor-hash:v1";
const SIGNAL_LANE_HASH_V1: &[u8] = b"sccp:solana:signal:lane-hash:v1";
const SIGNAL_SOURCE_IDENTITY_HASH_V1: &[u8] = b"sccp:solana:signal:source-identity-hash:v1";
const SIGNAL_ROOTED_SLOT_V1: &[u8] = b"sccp:solana:signal:rooted-slot:v1";
const SIGNAL_ROOTED_BANK_HASH_V1: &[u8] = b"sccp:solana:signal:rooted-bank-hash:v1";
const SIGNAL_MESSAGE_ID_V1: &[u8] = b"sccp:solana:signal:message-id:v1";
const SIGNAL_PAYLOAD_HASH_V1: &[u8] = b"sccp:solana:signal:payload-hash:v1";
const SIGNAL_SOURCE_EVENT_DIGEST_V1: &[u8] = b"sccp:solana:signal:source-event-digest:v1";
const SIGNAL_TRANSFER_INSTRUCTION_HASH_V1: &[u8] =
    b"sccp:solana:signal:transfer-instruction-hash:v1";
const SIGNAL_TRANSACTION_SIGNATURE_HASH_V1: &[u8] =
    b"sccp:solana:signal:transaction-signature-hash:v1";
const SIGNAL_ROUTE_CONFIGURATION_HASH_V1: &[u8] = b"sccp:solana:signal:route-configuration-hash:v1";

const SOLANA_AGAVE_PUBLIC_SIGNAL_LABELS_V1: [&[u8]; 11] = [
    SIGNAL_ANCHOR_HASH_V1,
    SIGNAL_LANE_HASH_V1,
    SIGNAL_SOURCE_IDENTITY_HASH_V1,
    SIGNAL_ROOTED_SLOT_V1,
    SIGNAL_ROOTED_BANK_HASH_V1,
    SIGNAL_MESSAGE_ID_V1,
    SIGNAL_PAYLOAD_HASH_V1,
    SIGNAL_SOURCE_EVENT_DIGEST_V1,
    SIGNAL_TRANSFER_INSTRUCTION_HASH_V1,
    SIGNAL_TRANSACTION_SIGNATURE_HASH_V1,
    SIGNAL_ROUTE_CONFIGURATION_HASH_V1,
];

/// Immutable commitments identifying the audited recursive Agave circuit.
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
#[norito(deny_unknown_fields)]
pub struct SccpSolanaAgaveSemanticProfileV1 {
    /// Profile schema version. V1 accepts exactly `1`.
    pub version: u8,
    /// Commitment to the exact Agave release and consensus implementation.
    #[norito(with = "crate::json_utils::hex32")]
    pub agave_release_commitment: H256,
    /// Commitment to the exact activated Solana feature set.
    #[norito(with = "crate::json_utils::hex32")]
    pub feature_set_commitment: H256,
    /// Commitment to the complete recursive constraint system and proving key.
    #[norito(with = "crate::json_utils::hex32")]
    pub circuit_commitment: H256,
    /// Commitment to the reproducible witness generator and dependencies.
    #[norito(with = "crate::json_utils::hex32")]
    pub witness_generator_commitment: H256,
    /// Commitment to the ordered eleven-signal schema in this module.
    #[norito(with = "crate::json_utils::hex32")]
    pub public_signal_schema_hash: H256,
}

impl SccpSolanaAgaveSemanticProfileV1 {
    /// Return whether every semantic role is exact, nonzero, and separated.
    #[must_use]
    pub fn is_well_formed(self) -> bool {
        self.version == 1
            && self.public_signal_schema_hash == sccp_solana_agave_public_signal_schema_hash_v1()
            && hashes_are_nonzero_and_distinct(&[
                self.agave_release_commitment,
                self.feature_set_commitment,
                self.circuit_commitment,
                self.witness_generator_commitment,
                self.public_signal_schema_hash,
            ])
    }
}

/// Governed Solana testnet checkpoint and recursive-verifier material.
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
#[norito(deny_unknown_fields)]
pub struct SccpSolanaAgaveTrustAnchorV1 {
    /// Anchor schema version. V1 accepts exactly `1`.
    pub version: u8,
    /// Exact source network. V1 accepts only `SolanaTestnet`.
    pub network: SccpNetworkV1,
    /// Raw canonical testnet genesis hash.
    #[norito(with = "crate::json_utils::hex32")]
    pub genesis_hash: H256,
    /// Governed rooted checkpoint slot.
    #[norito(with = "crate::json_utils::u64_string")]
    pub checkpoint_slot: u64,
    /// Governed rooted checkpoint bank hash.
    #[norito(with = "crate::json_utils::hex32")]
    pub checkpoint_bank_hash: H256,
    /// Exact audited recursive-circuit semantics.
    pub semantic_profile: SccpSolanaAgaveSemanticProfileV1,
    /// Complete BN254 key for exactly the eleven public signals.
    pub verifying_key: SccpGroth16Bn254VerifyingKeyV1,
}

/// Public statement proved by the governed recursive Agave circuit.
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
#[norito(deny_unknown_fields)]
pub struct SccpSolanaAgaveTransferStatementV1 {
    /// Statement schema version. V1 accepts exactly `1`.
    pub version: u8,
    /// Rooted Solana slot containing the successful route transaction.
    #[norito(with = "crate::json_utils::u64_string")]
    pub rooted_slot: u64,
    /// Rooted bank hash authenticated by recursive Agave replay.
    #[norito(with = "crate::json_utils::hex32")]
    pub rooted_bank_hash: H256,
    /// Commitment to the canonical stake snapshot consumed by finality replay.
    #[norito(with = "crate::json_utils::hex32")]
    pub finality_stake_snapshot_hash: H256,
    /// Commitment to the canonical vote-account state consumed by replay.
    #[norito(with = "crate::json_utils::hex32")]
    pub finality_vote_state_hash: H256,
    /// Commitment to the complete deterministic Agave replay witness.
    #[norito(with = "crate::json_utils::hex32")]
    pub finality_replay_transcript_hash: H256,
    /// Canonical 64-byte Ed25519 transaction signature.
    #[norito(with = "crate::json_utils::bytes_hex")]
    pub transaction_signature: Vec<u8>,
    /// Zero-based transaction position in the rooted bank's ordered entry stream.
    pub transaction_index: u32,
    /// Zero-based direct top-level route-instruction index.
    pub instruction_index: u16,
    /// Number of matching successful route instructions in the transaction.
    /// V1 requires exactly one.
    pub matching_route_instruction_count: u16,
    /// Number of matching authenticated source events in the transaction.
    /// V1 requires exactly one.
    pub matching_source_event_count: u16,
    /// Signing Solana wallet and token-account owner.
    #[norito(with = "crate::json_utils::hex32")]
    pub sender: H256,
    /// Exact SPL mint burned by the route instruction.
    #[norito(with = "crate::json_utils::hex32")]
    pub mint: H256,
    /// Exact sender-owned SPL token account debited by the instruction.
    #[norito(with = "crate::json_utils::hex32")]
    pub source_token_account: H256,
    /// Program-derived burn receipt account created by the successful instruction.
    #[norito(with = "crate::json_utils::hex32")]
    pub burn_receipt_account: H256,
    /// Hash of the canonical program-owned burn receipt state.
    #[norito(with = "crate::json_utils::hex32")]
    pub burn_receipt_hash: H256,
    /// Positive nine-decimal SPL base-unit amount. It must equal the SCCP
    /// payload's nine-decimal Taira mantissa exactly.
    #[norito(with = "crate::json_utils::u64_string")]
    pub amount: u64,
    /// Exact canonical SORA recipient bytes passed to the route instruction.
    #[norito(with = "crate::json_utils::bytes_hex")]
    pub recipient: Vec<u8>,
    /// Sender-selected nonce passed to the route instruction.
    #[norito(with = "crate::json_utils::u64_string")]
    pub nonce: u64,
    /// Nonzero immutable governed route revision passed to the instruction.
    pub route_revision: u32,
    /// Governed immutable route configuration read by the instruction.
    #[norito(with = "crate::json_utils::hex32")]
    pub route_configuration_hash: H256,
    /// Hash of the exact Solana-testnet-to-Taira lane.
    #[norito(with = "crate::json_utils::hex32")]
    pub lane_hash: H256,
    /// Governed source-program deployment identity hash.
    #[norito(with = "crate::json_utils::hex32")]
    pub source_identity_hash: H256,
    /// Exact lane-bound SCCP message identifier.
    #[norito(with = "crate::json_utils::hex32")]
    pub message_id: H256,
    /// Canonical SCCP payload hash.
    #[norito(with = "crate::json_utils::hex32")]
    pub payload_hash: H256,
    /// Exact lane/message/payload event digest.
    #[norito(with = "crate::json_utils::hex32")]
    pub source_event_digest: H256,
}

/// Complete typed Solana testnet source proof.
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
#[norito(deny_unknown_fields)]
pub struct SccpSolanaAgaveSourceProofV1 {
    /// Proof schema version. V1 accepts exactly `1`.
    pub version: u8,
    /// Full governed anchor preimage.
    pub anchor: SccpSolanaAgaveTrustAnchorV1,
    /// Exact public statement authenticated by the recursive proof.
    pub statement: SccpSolanaAgaveTransferStatementV1,
    /// Canonical twelve-word BN254 Groth16 proof envelope.
    #[norito(with = "crate::json_utils::bytes_hex")]
    pub proof_bytes: Vec<u8>,
}

/// Normalized result of successful Solana native verification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValidatedSccpSolanaAgaveSourceV1 {
    /// Governed source-program deployment identity hash.
    pub source_identity_hash: H256,
    /// Exact lane hash.
    pub lane_hash: H256,
    /// Governed anchor hash.
    pub anchor_hash: H256,
    /// Authenticated event digest.
    pub source_event_digest: H256,
    /// Rooted slot containing the successful route instruction.
    pub rooted_slot: u64,
    /// Authenticated rooted bank hash.
    pub rooted_bank_hash: H256,
    /// Hash of the authenticated Solana transaction signature.
    pub transaction_signature_hash: H256,
}

/// Fail-closed Solana native source-proof error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SolanaNativeSourceErrorV1 {
    /// A V1 wrapper, anchor, profile, or statement version was not `1`.
    UnsupportedVersion,
    /// The proof did not identify the exact canonical Solana testnet genesis.
    InvalidNetwork,
    /// The governed checkpoint or recursive-verifier material was malformed.
    InvalidAnchor,
    /// The source program deployment identity was malformed or mismatched.
    InvalidSourceIdentity,
    /// The SCCP payload or economic instruction statement was inconsistent.
    InvalidStatement,
    /// The fixed-width proof envelope was malformed or noncanonical.
    InvalidProofEncoding,
    /// The governed BN254 pairing equation did not verify.
    InvalidProof,
}

impl fmt::Display for SolanaNativeSourceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion => {
                formatter.write_str("unsupported Solana source proof version")
            }
            Self::InvalidNetwork => formatter.write_str("invalid Solana testnet identity"),
            Self::InvalidAnchor => formatter.write_str("invalid governed Solana Agave anchor"),
            Self::InvalidSourceIdentity => {
                formatter.write_str("invalid governed Solana source deployment identity")
            }
            Self::InvalidStatement => formatter.write_str("invalid Solana SCCP transfer statement"),
            Self::InvalidProofEncoding => {
                formatter.write_str("invalid canonical Solana Groth16 proof encoding")
            }
            Self::InvalidProof => formatter.write_str("Solana recursive Groth16 proof failed"),
        }
    }
}

impl std::error::Error for SolanaNativeSourceErrorV1 {}

fn hashes_are_nonzero_and_distinct(hashes: &[H256]) -> bool {
    hashes
        .iter()
        .enumerate()
        .all(|(index, hash)| hash.iter().any(|byte| *byte != 0) && !hashes[..index].contains(hash))
}

fn push_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_len_prefixed(out: &mut Vec<u8>, value: &[u8]) -> Option<()> {
    push_u32(out, u32::try_from(value.len()).ok()?);
    out.extend_from_slice(value);
    Some(())
}

fn abi_word_u64(value: u64) -> H256 {
    let mut word = [0; 32];
    word[24..].copy_from_slice(&value.to_be_bytes());
    word
}

/// Encode the exact ordered public-signal schema.
#[must_use]
pub fn canonical_sccp_solana_agave_public_signal_schema_bytes_v1() -> Vec<u8> {
    let mut out = Vec::new();
    out.push(1);
    out.push(11);
    for label in SOLANA_AGAVE_PUBLIC_SIGNAL_LABELS_V1 {
        push_u16(
            &mut out,
            u16::try_from(label.len()).expect("fixed Solana signal labels fit u16"),
        );
        out.extend_from_slice(label);
    }
    out
}

/// Hash the exact ordered public-signal schema.
#[must_use]
pub fn sccp_solana_agave_public_signal_schema_hash_v1() -> H256 {
    prefixed_blake2b(
        SOLANA_AGAVE_SIGNAL_SCHEMA_PREFIX_V1,
        &canonical_sccp_solana_agave_public_signal_schema_bytes_v1(),
    )
}

/// Encode a valid semantic-profile commitment in its canonical layout.
#[must_use]
pub fn canonical_sccp_solana_agave_semantic_profile_bytes_v1(
    profile: SccpSolanaAgaveSemanticProfileV1,
) -> Option<Vec<u8>> {
    if !profile.is_well_formed() {
        return None;
    }
    let mut out = Vec::with_capacity(1 + 5 * 32);
    out.push(profile.version);
    out.extend_from_slice(&profile.agave_release_commitment);
    out.extend_from_slice(&profile.feature_set_commitment);
    out.extend_from_slice(&profile.circuit_commitment);
    out.extend_from_slice(&profile.witness_generator_commitment);
    out.extend_from_slice(&profile.public_signal_schema_hash);
    Some(out)
}

/// Hash a valid recursive Agave semantic profile.
#[must_use]
pub fn sccp_solana_agave_semantic_profile_hash_v1(
    profile: SccpSolanaAgaveSemanticProfileV1,
) -> Option<H256> {
    Some(prefixed_blake2b(
        SOLANA_AGAVE_SEMANTIC_PROFILE_PREFIX_V1,
        &canonical_sccp_solana_agave_semantic_profile_bytes_v1(profile)?,
    ))
}

/// Encode the complete governed Solana Agave anchor preimage.
#[must_use]
pub fn canonical_sccp_solana_agave_anchor_bytes_v1(
    anchor: &SccpSolanaAgaveTrustAnchorV1,
) -> Option<Vec<u8>> {
    if anchor.version != 1
        || anchor.network != SccpNetworkV1::SolanaTestnet
        || anchor.genesis_hash != SCCP_SOLANA_TESTNET_GENESIS_HASH_V1
        || anchor.checkpoint_slot == 0
        || !anchor.semantic_profile.is_well_formed()
    {
        return None;
    }
    let semantic_profile_hash =
        sccp_solana_agave_semantic_profile_hash_v1(anchor.semantic_profile)?;
    let verifying_key_hash = sccp_groth16_bn254_verifying_key_hash_v1(anchor.verifying_key)?;
    if !hashes_are_nonzero_and_distinct(&[
        anchor.genesis_hash,
        anchor.checkpoint_bank_hash,
        anchor.semantic_profile.agave_release_commitment,
        anchor.semantic_profile.feature_set_commitment,
        anchor.semantic_profile.circuit_commitment,
        anchor.semantic_profile.witness_generator_commitment,
        anchor.semantic_profile.public_signal_schema_hash,
        semantic_profile_hash,
        verifying_key_hash,
    ]) {
        return None;
    }
    let verifying_key_bytes =
        canonical_sccp_groth16_bn254_verifying_key_bytes_v1(anchor.verifying_key)?;
    let profile_bytes =
        canonical_sccp_solana_agave_semantic_profile_bytes_v1(anchor.semantic_profile)?;
    let mut out = Vec::with_capacity(76 + profile_bytes.len() + verifying_key_bytes.len());
    out.push(anchor.version);
    out.push(SOLANA_TESTNET_NETWORK_TAG_V1);
    out.extend_from_slice(&anchor.genesis_hash);
    push_u64(&mut out, anchor.checkpoint_slot);
    out.extend_from_slice(&anchor.checkpoint_bank_hash);
    push_len_prefixed(&mut out, &profile_bytes)?;
    out.extend_from_slice(&verifying_key_hash);
    push_len_prefixed(&mut out, &verifying_key_bytes)?;
    Some(out)
}

/// Hash the complete governed Solana Agave anchor preimage.
#[must_use]
pub fn sccp_solana_agave_anchor_hash_v1(anchor: &SccpSolanaAgaveTrustAnchorV1) -> Option<H256> {
    Some(prefixed_blake2b(
        SOLANA_AGAVE_ANCHOR_PREFIX_V1,
        &canonical_sccp_solana_agave_anchor_bytes_v1(anchor)?,
    ))
}

fn statement_shape_is_valid(statement: &SccpSolanaAgaveTransferStatementV1) -> bool {
    statement.version == 1
        && statement.rooted_slot != 0
        && statement.transaction_signature.len() == 64
        && statement
            .transaction_signature
            .iter()
            .any(|byte| *byte != 0)
        && statement.amount != 0
        && statement.route_revision != 0
        && statement.matching_route_instruction_count == 1
        && statement.matching_source_event_count == 1
        && !statement.recipient.is_empty()
        && statement.recipient.len() <= SCCP_SOLANA_AGAVE_MAX_RECIPIENT_BYTES_V1
        && hashes_are_nonzero_and_distinct(&[
            statement.rooted_bank_hash,
            statement.finality_stake_snapshot_hash,
            statement.finality_vote_state_hash,
            statement.finality_replay_transcript_hash,
            statement.sender,
            statement.mint,
            statement.source_token_account,
            statement.burn_receipt_account,
            statement.burn_receipt_hash,
            statement.route_configuration_hash,
            statement.lane_hash,
            statement.source_identity_hash,
            statement.message_id,
            statement.payload_hash,
            statement.source_event_digest,
        ])
}

/// Encode the exact economic Solana route-instruction statement.
#[must_use]
pub fn canonical_sccp_solana_agave_transfer_statement_bytes_v1(
    statement: &SccpSolanaAgaveTransferStatementV1,
) -> Option<Vec<u8>> {
    if !statement_shape_is_valid(statement) {
        return None;
    }
    let mut out = Vec::new();
    out.push(statement.version);
    push_u64(&mut out, statement.rooted_slot);
    out.extend_from_slice(&statement.rooted_bank_hash);
    out.extend_from_slice(&statement.finality_stake_snapshot_hash);
    out.extend_from_slice(&statement.finality_vote_state_hash);
    out.extend_from_slice(&statement.finality_replay_transcript_hash);
    out.extend_from_slice(&statement.transaction_signature);
    push_u32(&mut out, statement.transaction_index);
    push_u16(&mut out, statement.instruction_index);
    push_u16(&mut out, statement.matching_route_instruction_count);
    push_u16(&mut out, statement.matching_source_event_count);
    out.extend_from_slice(&statement.sender);
    out.extend_from_slice(&statement.mint);
    out.extend_from_slice(&statement.source_token_account);
    out.extend_from_slice(&statement.burn_receipt_account);
    out.extend_from_slice(&statement.burn_receipt_hash);
    push_u64(&mut out, statement.amount);
    push_len_prefixed(&mut out, &statement.recipient)?;
    push_u64(&mut out, statement.nonce);
    push_u32(&mut out, statement.route_revision);
    out.extend_from_slice(&statement.route_configuration_hash);
    out.extend_from_slice(&statement.lane_hash);
    out.extend_from_slice(&statement.source_identity_hash);
    out.extend_from_slice(&statement.message_id);
    out.extend_from_slice(&statement.payload_hash);
    out.extend_from_slice(&statement.source_event_digest);
    Some(out)
}

/// Hash the exact economic Solana route-instruction statement.
#[must_use]
pub fn sccp_solana_agave_transfer_statement_hash_v1(
    statement: &SccpSolanaAgaveTransferStatementV1,
) -> Option<H256> {
    Some(prefixed_blake2b(
        SOLANA_AGAVE_TRANSFER_INSTRUCTION_PREFIX_V1,
        &canonical_sccp_solana_agave_transfer_statement_bytes_v1(statement)?,
    ))
}

fn transaction_signature_hash(signature: &[u8]) -> Option<H256> {
    (signature.len() == 64 && signature.iter().any(|byte| *byte != 0))
        .then(|| prefixed_blake2b(SOLANA_AGAVE_TRANSACTION_SIGNATURE_PREFIX_V1, signature))
}

/// Derive the exact eleven BN254 public signals for a Solana source proof.
#[must_use]
pub fn sccp_solana_agave_public_signal_words_v1(
    anchor_hash: H256,
    statement: &SccpSolanaAgaveTransferStatementV1,
) -> Option<[H256; 11]> {
    if !statement_shape_is_valid(statement) || anchor_hash.iter().all(|byte| *byte == 0) {
        return None;
    }
    let transfer_hash = sccp_solana_agave_transfer_statement_hash_v1(statement)?;
    let signature_hash = transaction_signature_hash(&statement.transaction_signature)?;
    Some([
        sccp_groth16_bn254_signal_word(SIGNAL_ANCHOR_HASH_V1, anchor_hash),
        sccp_groth16_bn254_signal_word(SIGNAL_LANE_HASH_V1, statement.lane_hash),
        sccp_groth16_bn254_signal_word(
            SIGNAL_SOURCE_IDENTITY_HASH_V1,
            statement.source_identity_hash,
        ),
        sccp_groth16_bn254_signal_word(SIGNAL_ROOTED_SLOT_V1, abi_word_u64(statement.rooted_slot)),
        sccp_groth16_bn254_signal_word(SIGNAL_ROOTED_BANK_HASH_V1, statement.rooted_bank_hash),
        sccp_groth16_bn254_signal_word(SIGNAL_MESSAGE_ID_V1, statement.message_id),
        sccp_groth16_bn254_signal_word(SIGNAL_PAYLOAD_HASH_V1, statement.payload_hash),
        sccp_groth16_bn254_signal_word(
            SIGNAL_SOURCE_EVENT_DIGEST_V1,
            statement.source_event_digest,
        ),
        sccp_groth16_bn254_signal_word(SIGNAL_TRANSFER_INSTRUCTION_HASH_V1, transfer_hash),
        sccp_groth16_bn254_signal_word(SIGNAL_TRANSACTION_SIGNATURE_HASH_V1, signature_hash),
        sccp_groth16_bn254_signal_word(
            SIGNAL_ROUTE_CONFIGURATION_HASH_V1,
            statement.route_configuration_hash,
        ),
    ])
}

/// Verify one governed recursive Solana testnet source proof.
///
/// The governed anchor hash authenticates the exact testnet genesis,
/// checkpoint, Agave semantics, circuit, witness generator, and verification
/// key. The pairing equation then authenticates the rooted bank and the full
/// successful transfer instruction statement.
///
/// # Errors
///
/// Returns a fail-closed error for every malformed, mismatched, oversized,
/// noncanonical, backend-substituted, or cryptographically invalid input.
#[expect(
    clippy::too_many_lines,
    reason = "the single ordered Agave proof validator preserves stable first-error precedence across anchor, statement, and pairing checks"
)]
pub fn verify_sccp_solana_agave_source_v1(
    proof: &SccpSolanaAgaveSourceProofV1,
    source_identity: &SccpSourceIdentityV1,
    expected_source_identity_hash: H256,
    expected_anchor_hash: H256,
    expected_message_id: H256,
    expected_payload_hash: H256,
    payload: &SccpPayloadV1,
) -> Result<ValidatedSccpSolanaAgaveSourceV1, SolanaNativeSourceErrorV1> {
    if proof.version != 1
        || proof.anchor.version != 1
        || proof.anchor.semantic_profile.version != 1
        || proof.statement.version != 1
    {
        return Err(SolanaNativeSourceErrorV1::UnsupportedVersion);
    }
    if proof.anchor.network != SccpNetworkV1::SolanaTestnet
        || proof.anchor.genesis_hash != SCCP_SOLANA_TESTNET_GENESIS_HASH_V1
    {
        return Err(SolanaNativeSourceErrorV1::InvalidNetwork);
    }
    let anchor_hash = sccp_solana_agave_anchor_hash_v1(&proof.anchor)
        .ok_or(SolanaNativeSourceErrorV1::InvalidAnchor)?;
    if anchor_hash != expected_anchor_hash
        || proof.statement.rooted_slot <= proof.anchor.checkpoint_slot
        || proof.statement.rooted_bank_hash == proof.anchor.checkpoint_bank_hash
    {
        return Err(SolanaNativeSourceErrorV1::InvalidAnchor);
    }
    if !source_identity.is_well_formed()
        || source_identity.lane.source != SccpNetworkV1::SolanaTestnet
        || source_identity.lane.target != SccpNetworkV1::SoraTaira
    {
        return Err(SolanaNativeSourceErrorV1::InvalidSourceIdentity);
    }
    let SccpSourceEmitterV1::Solana(emitter) = source_identity.emitter else {
        return Err(SolanaNativeSourceErrorV1::InvalidSourceIdentity);
    };
    let identity_hash = sccp_source_identity_hash_v1(source_identity)
        .ok_or(SolanaNativeSourceErrorV1::InvalidSourceIdentity)?;
    let lane_hash = sccp_lane_id_hash_v1(source_identity.lane)
        .ok_or(SolanaNativeSourceErrorV1::InvalidSourceIdentity)?;
    if identity_hash != expected_source_identity_hash
        || proof.statement.source_identity_hash != identity_hash
        || proof.statement.lane_hash != lane_hash
        || proof.statement.route_configuration_hash != emitter.route_config_hash
    {
        return Err(SolanaNativeSourceErrorV1::InvalidSourceIdentity);
    }

    let SccpPayloadV1::Transfer(transfer) = payload;
    let canonical_payload = canonical_sccp_payload_bytes(payload)
        .map_err(|_| SolanaNativeSourceErrorV1::InvalidStatement)?;
    let source_event_digest = sccp_lane_source_event_digest_v1(
        source_identity.lane,
        expected_message_id,
        expected_payload_hash,
    )
    .ok_or(SolanaNativeSourceErrorV1::InvalidStatement)?;
    if !verify_sccp_payload_structure(payload)
        || transfer.source_domain != SCCP_DOMAIN_SOLANA
        || transfer.dest_domain != SCCP_DOMAIN_SORA
        || transfer.asset_home_domain != SCCP_DOMAIN_SORA
        || transfer.asset_id_codec != SCCP_CODEC_CANONICAL_TEXT
        || transfer.asset_id.as_slice() != SCCP_TAIRA_XOR_ASSET_KEY_V1.as_bytes()
        || transfer.sender_codec != SCCP_CODEC_SOLANA_PUBKEY32
        || transfer.sender.as_slice() != proof.statement.sender
        || transfer.recipient_codec != SCCP_CODEC_CANONICAL_TEXT
        || transfer.recipient != proof.statement.recipient
        || transfer.route_id_codec != SCCP_CODEC_CANONICAL_TEXT
        || transfer.route_id.as_slice() != SCCP_TAIRA_SOL_XOR_ROUTE_ID_V1.as_bytes()
        || transfer.amount != u128::from(proof.statement.amount)
        || transfer.nonce != proof.statement.nonce
        || transfer.route_revision != proof.statement.route_revision
        || payload_hash(&canonical_payload) != expected_payload_hash
        || sccp_message_id(source_identity.lane, payload) != Some(expected_message_id)
        || proof.statement.message_id != expected_message_id
        || proof.statement.payload_hash != expected_payload_hash
        || proof.statement.source_event_digest != source_event_digest
        || !statement_shape_is_valid(&proof.statement)
    {
        return Err(SolanaNativeSourceErrorV1::InvalidStatement);
    }

    if proof.proof_bytes.len() != SCCP_SOLANA_AGAVE_GROTH16_PROOF_BYTES_V1 {
        return Err(SolanaNativeSourceErrorV1::InvalidProofEncoding);
    }
    let decoded = decode_sccp_evm_groth16_bn254_proof_bytes(&proof.proof_bytes)
        .ok_or(SolanaNativeSourceErrorV1::InvalidProofEncoding)?;
    if decoded.version != 1
        || decoded.message_id != expected_message_id
        || decoded.source_domain != SCCP_DOMAIN_SOLANA
        || decoded.commitment_root != proof.statement.rooted_bank_hash
        || super::encode_sccp_evm_groth16_bn254_proof_bytes(&decoded) != proof.proof_bytes
    {
        return Err(SolanaNativeSourceErrorV1::InvalidProofEncoding);
    }
    let public_signals = sccp_solana_agave_public_signal_words_v1(anchor_hash, &proof.statement)
        .ok_or(SolanaNativeSourceErrorV1::InvalidStatement)?;
    if !verify_sccp_groth16_bn254_pairing_equation_v1(
        &decoded,
        &public_signals,
        &proof.anchor.verifying_key,
    ) {
        return Err(SolanaNativeSourceErrorV1::InvalidProof);
    }

    Ok(ValidatedSccpSolanaAgaveSourceV1 {
        source_identity_hash: identity_hash,
        lane_hash,
        anchor_hash,
        source_event_digest,
        rooted_slot: proof.statement.rooted_slot,
        rooted_bank_hash: proof.statement.rooted_bank_hash,
        transaction_signature_hash: transaction_signature_hash(
            &proof.statement.transaction_signature,
        )
        .expect("validated fixed nonzero Solana transaction signature"),
    })
}

#[cfg(test)]
mod tests {
    use halo2curves::{
        Coordinates, CurveAffine,
        bn256::{Fq, Fr, G1Affine},
        ff::PrimeField,
        group::Curve,
    };
    use iroha_data_model::bridge::{
        BridgeNativeProofBackendV1,
        SCCP_SOLANA_TESTNET_GENESIS_HASH_V1 as DATA_MODEL_SOLANA_TESTNET_GENESIS_HASH_V1,
        SccpBn254G1PointV1, SccpBn254G2PointV1, SccpGroth16Bn254IcV1, SccpLaneIdV1,
        SccpNativeTrustAnchorV1, SccpSolanaSourceEmitterV1,
    };

    use super::*;
    use crate::{
        SccpEvmGroth16Bn254ProofV1, TransferPayloadV1, encode_sccp_evm_groth16_bn254_proof_bytes,
    };

    #[derive(Clone)]
    struct Fixture {
        proof: SccpSolanaAgaveSourceProofV1,
        source_identity: SccpSourceIdentityV1,
        source_identity_hash: H256,
        anchor_hash: H256,
        message_id: H256,
        payload_hash: H256,
        payload: SccpPayloadV1,
    }

    fn word_u64(value: u64) -> H256 {
        let mut word = [0; 32];
        word[24..].copy_from_slice(&value.to_be_bytes());
        word
    }

    fn hex32(value: &str) -> H256 {
        crate::decode_fixed_hex_bytes(value).expect("lowercase 32-byte test vector")
    }

    fn fq_word(value: Fq) -> H256 {
        let repr = value.to_repr();
        let mut word = [0; 32];
        for (output, input) in word.iter_mut().zip(repr.as_ref().iter().rev()) {
            *output = *input;
        }
        word
    }

    fn g1_model(point: G1Affine) -> SccpBn254G1PointV1 {
        let coordinates: Coordinates<G1Affine> =
            Option::from(point.coordinates()).expect("non-infinity G1 point");
        SccpBn254G1PointV1 {
            x: fq_word(*coordinates.x()),
            y: fq_word(*coordinates.y()),
        }
    }

    fn g2_generator_model() -> SccpBn254G2PointV1 {
        SccpBn254G2PointV1 {
            x_c0: hex32("1800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed"),
            x_c1: hex32("198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c2"),
            y_c0: hex32("12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daa"),
            y_c1: hex32("090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975b"),
        }
    }

    fn verifying_key() -> SccpGroth16Bn254VerifyingKeyV1 {
        let g1 = SccpBn254G1PointV1 {
            x: word_u64(1),
            y: word_u64(2),
        };
        let g2 = g2_generator_model();
        SccpGroth16Bn254VerifyingKeyV1 {
            version: 1,
            alpha1: g1,
            beta2: g2,
            gamma2: g2,
            delta2: g2,
            ic: SccpGroth16Bn254IcV1 {
                constant: g1,
                signal_0: g1,
                signal_1: g1,
                signal_2: g1,
                signal_3: g1,
                signal_4: g1,
                signal_5: g1,
                signal_6: g1,
                signal_7: g1,
                signal_8: g1,
                signal_9: g1,
                signal_10: g1,
            },
        }
    }

    fn semantic_profile() -> SccpSolanaAgaveSemanticProfileV1 {
        SccpSolanaAgaveSemanticProfileV1 {
            version: 1,
            agave_release_commitment: [0xa1; 32],
            feature_set_commitment: [0xa2; 32],
            circuit_commitment: [0xa3; 32],
            witness_generator_commitment: [0xa4; 32],
            public_signal_schema_hash: sccp_solana_agave_public_signal_schema_hash_v1(),
        }
    }

    fn source_identity() -> SccpSourceIdentityV1 {
        SccpSourceIdentityV1 {
            lane: SccpLaneIdV1 {
                source: SccpNetworkV1::SolanaTestnet,
                target: SccpNetworkV1::SoraTaira,
            },
            emitter: SccpSourceEmitterV1::Solana(SccpSolanaSourceEmitterV1 {
                program_id: [0x11; 32],
                program_data_address: [0x12; 32],
                program_data_slot: 5,
                state_account: [0x13; 32],
                program_code_hash: [0x14; 32],
                route_config_hash: [0x15; 32],
            }),
        }
    }

    fn payload(sender: H256) -> SccpPayloadV1 {
        SccpPayloadV1::Transfer(TransferPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SOLANA,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 9,
            route_revision: 2,
            asset_home_domain: SCCP_DOMAIN_SORA,
            asset_id_codec: SCCP_CODEC_CANONICAL_TEXT,
            asset_id: SCCP_TAIRA_XOR_ASSET_KEY_V1.as_bytes().to_vec(),
            amount: 123,
            sender_codec: SCCP_CODEC_SOLANA_PUBKEY32,
            sender: sender.to_vec(),
            recipient_codec: SCCP_CODEC_CANONICAL_TEXT,
            recipient: b"alice".to_vec(),
            route_id_codec: SCCP_CODEC_CANONICAL_TEXT,
            route_id: SCCP_TAIRA_SOL_XOR_ROUTE_ID_V1.as_bytes().to_vec(),
        })
    }

    fn valid_proof_bytes(
        verifying_key: &SccpGroth16Bn254VerifyingKeyV1,
        anchor_hash: H256,
        statement: &SccpSolanaAgaveTransferStatementV1,
    ) -> Vec<u8> {
        let signals = sccp_solana_agave_public_signal_words_v1(anchor_hash, statement)
            .expect("valid Solana public signals");
        let mut scalar = Fr::from(3_u64);
        for signal in &signals {
            scalar += crate::bn254_fr_from_abi_word(signal).expect("canonical scalar signal");
        }
        let a = g1_model((G1Affine::generator() * scalar).to_affine());
        encode_sccp_evm_groth16_bn254_proof_bytes(&SccpEvmGroth16Bn254ProofV1 {
            version: 1,
            message_id: statement.message_id,
            source_domain: SCCP_DOMAIN_SOLANA,
            commitment_root: statement.rooted_bank_hash,
            a: [a.x, a.y],
            b: [
                verifying_key.beta2.x_c0,
                verifying_key.beta2.x_c1,
                verifying_key.beta2.y_c0,
                verifying_key.beta2.y_c1,
            ],
            c: [verifying_key.alpha1.x, verifying_key.alpha1.y],
        })
    }

    fn fixture() -> Fixture {
        let source_identity = source_identity();
        let source_identity_hash =
            sccp_source_identity_hash_v1(&source_identity).expect("valid source identity");
        let lane_hash = sccp_lane_id_hash_v1(source_identity.lane).expect("valid lane");
        let payload = payload([0x24; 32]);
        let canonical_payload = canonical_sccp_payload_bytes(&payload).expect("valid payload");
        let payload_hash = payload_hash(&canonical_payload);
        let message_id = sccp_message_id(source_identity.lane, &payload).expect("message id");
        let source_event_digest =
            sccp_lane_source_event_digest_v1(source_identity.lane, message_id, payload_hash)
                .expect("event digest");
        let anchor = SccpSolanaAgaveTrustAnchorV1 {
            version: 1,
            network: SccpNetworkV1::SolanaTestnet,
            genesis_hash: SCCP_SOLANA_TESTNET_GENESIS_HASH_V1,
            checkpoint_slot: 10,
            checkpoint_bank_hash: [0xb1; 32],
            semantic_profile: semantic_profile(),
            verifying_key: verifying_key(),
        };
        let anchor_hash = sccp_solana_agave_anchor_hash_v1(&anchor).expect("valid anchor");
        let statement = SccpSolanaAgaveTransferStatementV1 {
            version: 1,
            rooted_slot: 11,
            rooted_bank_hash: [0x21; 32],
            finality_stake_snapshot_hash: [0x31; 32],
            finality_vote_state_hash: [0x32; 32],
            finality_replay_transcript_hash: [0x33; 32],
            transaction_signature: vec![0x22; 64],
            transaction_index: 3,
            instruction_index: 1,
            matching_route_instruction_count: 1,
            matching_source_event_count: 1,
            sender: [0x24; 32],
            mint: [0x25; 32],
            source_token_account: [0x26; 32],
            burn_receipt_account: [0x27; 32],
            burn_receipt_hash: [0x28; 32],
            amount: 123,
            recipient: b"alice".to_vec(),
            nonce: 9,
            route_revision: 2,
            route_configuration_hash: [0x15; 32],
            lane_hash,
            source_identity_hash,
            message_id,
            payload_hash,
            source_event_digest,
        };
        let proof_bytes = valid_proof_bytes(&anchor.verifying_key, anchor_hash, &statement);
        Fixture {
            proof: SccpSolanaAgaveSourceProofV1 {
                version: 1,
                anchor,
                statement,
                proof_bytes,
            },
            source_identity,
            source_identity_hash,
            anchor_hash,
            message_id,
            payload_hash,
            payload,
        }
    }

    fn fixture_with_nonce(nonce: u64) -> Fixture {
        let mut fixture = fixture();
        let SccpPayloadV1::Transfer(transfer) = &mut fixture.payload;
        transfer.nonce = nonce;
        let canonical_payload =
            canonical_sccp_payload_bytes(&fixture.payload).expect("valid payload");
        fixture.payload_hash = payload_hash(&canonical_payload);
        fixture.message_id =
            sccp_message_id(fixture.source_identity.lane, &fixture.payload).expect("message id");
        let source_event_digest = sccp_lane_source_event_digest_v1(
            fixture.source_identity.lane,
            fixture.message_id,
            fixture.payload_hash,
        )
        .expect("event digest");
        fixture.proof.statement.nonce = nonce;
        fixture.proof.statement.message_id = fixture.message_id;
        fixture.proof.statement.payload_hash = fixture.payload_hash;
        fixture.proof.statement.source_event_digest = source_event_digest;
        fixture.proof.proof_bytes = valid_proof_bytes(
            &fixture.proof.anchor.verifying_key,
            fixture.anchor_hash,
            &fixture.proof.statement,
        );
        fixture
    }

    fn verify(fixture: &Fixture, proof: &SccpSolanaAgaveSourceProofV1) -> bool {
        verify_sccp_solana_agave_source_v1(
            proof,
            &fixture.source_identity,
            fixture.source_identity_hash,
            fixture.anchor_hash,
            fixture.message_id,
            fixture.payload_hash,
            &fixture.payload,
        )
        .is_ok()
    }

    #[test]
    fn exact_source_proof_verifies_and_roundtrips() {
        let fixture = fixture();
        let validated = verify_sccp_solana_agave_source_v1(
            &fixture.proof,
            &fixture.source_identity,
            fixture.source_identity_hash,
            fixture.anchor_hash,
            fixture.message_id,
            fixture.payload_hash,
            &fixture.payload,
        )
        .expect("valid recursive source proof");
        assert_eq!(validated.rooted_slot, fixture.proof.statement.rooted_slot);
        assert_eq!(
            validated.rooted_bank_hash,
            fixture.proof.statement.rooted_bank_hash
        );
        assert_eq!(validated.anchor_hash, fixture.anchor_hash);

        let norito = norito::to_bytes(&fixture.proof).expect("encode source proof");
        let decoded: SccpSolanaAgaveSourceProofV1 =
            norito::decode_from_bytes(&norito).expect("decode source proof");
        assert_eq!(decoded, fixture.proof);
        assert_eq!(norito::to_bytes(&decoded).expect("re-encode"), norito);

        let json = norito::json::to_json(&fixture.proof).expect("encode source proof JSON");
        let decoded_json = norito::json::from_str::<SccpSolanaAgaveSourceProofV1>(&json)
            .expect("decode source proof JSON");
        assert_eq!(decoded_json, fixture.proof);
        assert_eq!(
            norito::json::to_json(&decoded_json).expect("re-encode JSON"),
            json
        );
    }

    #[test]
    fn maximum_u64_nonce_is_valid_and_proof_bound() {
        let fixture = fixture_with_nonce(u64::MAX);
        assert!(verify(&fixture, &fixture.proof));
        assert_eq!(fixture.proof.statement.nonce, u64::MAX);

        let mut changed = fixture.proof.clone();
        changed.statement.nonce = u64::MAX - 1;
        assert!(!verify(&fixture, &changed));
    }

    #[test]
    fn canonical_anchor_binds_exact_testnet_and_every_governed_role() {
        let fixture = fixture();
        assert_eq!(
            SCCP_SOLANA_TESTNET_GENESIS_HASH_V1,
            DATA_MODEL_SOLANA_TESTNET_GENESIS_HASH_V1
        );
        let bytes = canonical_sccp_solana_agave_anchor_bytes_v1(&fixture.proof.anchor)
            .expect("canonical anchor");
        assert_eq!(bytes[0], 1);
        assert_eq!(bytes[1], SOLANA_TESTNET_NETWORK_TAG_V1);
        assert_eq!(&bytes[2..34], &SCCP_SOLANA_TESTNET_GENESIS_HASH_V1);

        let mut changed = fixture.proof.anchor;
        changed.checkpoint_slot += 1;
        assert_ne!(
            sccp_solana_agave_anchor_hash_v1(&changed),
            Some(fixture.anchor_hash)
        );
        changed = fixture.proof.anchor;
        changed.checkpoint_bank_hash[0] ^= 1;
        assert_ne!(
            sccp_solana_agave_anchor_hash_v1(&changed),
            Some(fixture.anchor_hash)
        );
        changed = fixture.proof.anchor;
        changed.semantic_profile.agave_release_commitment[0] ^= 1;
        assert_ne!(
            sccp_solana_agave_anchor_hash_v1(&changed),
            Some(fixture.anchor_hash)
        );
        changed = fixture.proof.anchor;
        changed.semantic_profile.feature_set_commitment[0] ^= 1;
        assert_ne!(
            sccp_solana_agave_anchor_hash_v1(&changed),
            Some(fixture.anchor_hash)
        );
        changed = fixture.proof.anchor;
        changed.semantic_profile.circuit_commitment[0] ^= 1;
        assert_ne!(
            sccp_solana_agave_anchor_hash_v1(&changed),
            Some(fixture.anchor_hash)
        );
        changed = fixture.proof.anchor;
        changed.semantic_profile.witness_generator_commitment[0] ^= 1;
        assert_ne!(
            sccp_solana_agave_anchor_hash_v1(&changed),
            Some(fixture.anchor_hash)
        );

        for invalid in [
            {
                let mut value = fixture.proof.anchor;
                value.version = 2;
                value
            },
            {
                let mut value = fixture.proof.anchor;
                value.network = SccpNetworkV1::EthereumSepolia;
                value
            },
            {
                let mut value = fixture.proof.anchor;
                value.genesis_hash[0] ^= 1;
                value
            },
            {
                let mut value = fixture.proof.anchor;
                value.checkpoint_slot = 0;
                value
            },
            {
                let mut value = fixture.proof.anchor;
                value.checkpoint_bank_hash = [0; 32];
                value
            },
            {
                let mut value = fixture.proof.anchor;
                value.semantic_profile.public_signal_schema_hash = [0x55; 32];
                value
            },
            {
                let mut value = fixture.proof.anchor;
                value.semantic_profile.feature_set_commitment =
                    value.semantic_profile.agave_release_commitment;
                value
            },
            {
                let mut value = fixture.proof.anchor;
                value.verifying_key.version = 2;
                value
            },
        ] {
            assert!(canonical_sccp_solana_agave_anchor_bytes_v1(&invalid).is_none());
            assert!(sccp_solana_agave_anchor_hash_v1(&invalid).is_none());
        }
    }

    #[test]
    fn every_public_and_economic_statement_role_is_proof_bound() {
        let fixture = fixture();
        let base = &fixture.proof;
        let mut mutations: Vec<(&str, SccpSolanaAgaveSourceProofV1)> = Vec::new();
        macro_rules! mutate {
            ($name:literal, $body:expr) => {{
                let mut proof = base.clone();
                $body(&mut proof);
                mutations.push(($name, proof));
            }};
        }
        mutate!("rooted slot", |proof: &mut SccpSolanaAgaveSourceProofV1| {
            proof.statement.rooted_slot += 1;
        });
        mutate!("rooted bank", |proof: &mut SccpSolanaAgaveSourceProofV1| {
            proof.statement.rooted_bank_hash[0] ^= 1;
        });
        mutate!(
            "stake snapshot",
            |proof: &mut SccpSolanaAgaveSourceProofV1| {
                proof.statement.finality_stake_snapshot_hash[0] ^= 1;
            }
        );
        mutate!("vote state", |proof: &mut SccpSolanaAgaveSourceProofV1| {
            proof.statement.finality_vote_state_hash[0] ^= 1;
        });
        mutate!(
            "replay transcript",
            |proof: &mut SccpSolanaAgaveSourceProofV1| {
                proof.statement.finality_replay_transcript_hash[0] ^= 1;
            }
        );
        mutate!(
            "transaction signature",
            |proof: &mut SccpSolanaAgaveSourceProofV1| {
                proof.statement.transaction_signature[0] ^= 1;
            }
        );
        mutate!(
            "transaction index",
            |proof: &mut SccpSolanaAgaveSourceProofV1| {
                proof.statement.transaction_index += 1;
            }
        );
        mutate!(
            "instruction index",
            |proof: &mut SccpSolanaAgaveSourceProofV1| {
                proof.statement.instruction_index += 1;
            }
        );
        mutate!("sender", |proof: &mut SccpSolanaAgaveSourceProofV1| {
            proof.statement.sender[0] ^= 1;
        });
        mutate!("mint", |proof: &mut SccpSolanaAgaveSourceProofV1| {
            proof.statement.mint[0] ^= 1;
        });
        mutate!(
            "token account",
            |proof: &mut SccpSolanaAgaveSourceProofV1| {
                proof.statement.source_token_account[0] ^= 1;
            }
        );
        mutate!(
            "burn receipt account",
            |proof: &mut SccpSolanaAgaveSourceProofV1| {
                proof.statement.burn_receipt_account[0] ^= 1;
            }
        );
        mutate!(
            "burn receipt hash",
            |proof: &mut SccpSolanaAgaveSourceProofV1| {
                proof.statement.burn_receipt_hash[0] ^= 1;
            }
        );
        mutate!("amount", |proof: &mut SccpSolanaAgaveSourceProofV1| {
            proof.statement.amount += 1;
        });
        mutate!("recipient", |proof: &mut SccpSolanaAgaveSourceProofV1| {
            proof.statement.recipient[0] ^= 1;
        });
        mutate!("nonce", |proof: &mut SccpSolanaAgaveSourceProofV1| {
            proof.statement.nonce += 1;
        });
        mutate!(
            "route revision",
            |proof: &mut SccpSolanaAgaveSourceProofV1| {
                proof.statement.route_revision += 1;
            }
        );
        mutate!(
            "route configuration",
            |proof: &mut SccpSolanaAgaveSourceProofV1| {
                proof.statement.route_configuration_hash[0] ^= 1;
            }
        );
        mutate!("lane", |proof: &mut SccpSolanaAgaveSourceProofV1| {
            proof.statement.lane_hash[0] ^= 1;
        });
        mutate!(
            "source identity",
            |proof: &mut SccpSolanaAgaveSourceProofV1| {
                proof.statement.source_identity_hash[0] ^= 1;
            }
        );
        mutate!("message id", |proof: &mut SccpSolanaAgaveSourceProofV1| {
            proof.statement.message_id[0] ^= 1;
        });
        mutate!(
            "payload hash",
            |proof: &mut SccpSolanaAgaveSourceProofV1| {
                proof.statement.payload_hash[0] ^= 1;
            }
        );
        mutate!(
            "event digest",
            |proof: &mut SccpSolanaAgaveSourceProofV1| {
                proof.statement.source_event_digest[0] ^= 1;
            }
        );

        for (name, proof) in mutations {
            assert!(!verify(&fixture, &proof), "accepted mutated {name}");
        }
    }

    #[test]
    fn ambiguous_or_oversized_instruction_statements_fail_closed() {
        let fixture = fixture();
        for count in [0, 2, u16::MAX] {
            let mut proof = fixture.proof.clone();
            proof.statement.matching_route_instruction_count = count;
            assert!(!verify(&fixture, &proof));
            let mut proof = fixture.proof.clone();
            proof.statement.matching_source_event_count = count;
            assert!(!verify(&fixture, &proof));
        }
        for signature_length in [0, 63, 65, 1024] {
            let mut proof = fixture.proof.clone();
            proof.statement.transaction_signature = vec![0x44; signature_length];
            assert!(!verify(&fixture, &proof));
            assert!(
                canonical_sccp_solana_agave_transfer_statement_bytes_v1(&proof.statement).is_none()
            );
        }
        let mut proof = fixture.proof.clone();
        proof.statement.recipient = vec![0x41; SCCP_SOLANA_AGAVE_MAX_RECIPIENT_BYTES_V1 + 1];
        assert!(!verify(&fixture, &proof));
        assert!(
            canonical_sccp_solana_agave_transfer_statement_bytes_v1(&proof.statement).is_none()
        );
    }

    #[test]
    fn proof_framing_and_curve_mutations_fail_closed() {
        let fixture = fixture();
        for proof_bytes in [
            Vec::new(),
            vec![0; SCCP_SOLANA_AGAVE_GROTH16_PROOF_BYTES_V1],
            fixture.proof.proof_bytes[..SCCP_SOLANA_AGAVE_GROTH16_PROOF_BYTES_V1 - 1].to_vec(),
            {
                let mut value = fixture.proof.proof_bytes.clone();
                value.push(0);
                value
            },
            {
                let mut value = fixture.proof.proof_bytes.clone();
                value[31] = 2;
                value
            },
            {
                let mut value = fixture.proof.proof_bytes.clone();
                value[32] ^= 1;
                value
            },
            {
                let mut value = fixture.proof.proof_bytes.clone();
                value[95] = 4;
                value
            },
            {
                let mut value = fixture.proof.proof_bytes.clone();
                value[96] ^= 1;
                value
            },
            {
                let mut value = fixture.proof.proof_bytes.clone();
                value[128..160].fill(0);
                value
            },
        ] {
            let mut proof = fixture.proof.clone();
            proof.proof_bytes = proof_bytes;
            assert!(!verify(&fixture, &proof));
        }
    }

    #[test]
    fn governed_anchor_key_and_source_deployment_substitution_fail_closed() {
        let fixture = fixture();

        let mut changed_anchor = fixture.proof.clone();
        changed_anchor.anchor.checkpoint_slot += 1;
        let changed_anchor_hash =
            sccp_solana_agave_anchor_hash_v1(&changed_anchor.anchor).expect("valid changed anchor");
        assert!(
            verify_sccp_solana_agave_source_v1(
                &changed_anchor,
                &fixture.source_identity,
                fixture.source_identity_hash,
                changed_anchor_hash,
                fixture.message_id,
                fixture.payload_hash,
                &fixture.payload,
            )
            .is_err(),
            "an old proof must not authenticate a different governed checkpoint"
        );

        let mut changed_key = fixture.proof.clone();
        changed_key.anchor.verifying_key.alpha1 =
            g1_model((G1Affine::generator() * Fr::from(2)).to_affine());
        let changed_key_anchor_hash = sccp_solana_agave_anchor_hash_v1(&changed_key.anchor)
            .expect("valid changed key anchor");
        assert!(
            verify_sccp_solana_agave_source_v1(
                &changed_key,
                &fixture.source_identity,
                fixture.source_identity_hash,
                changed_key_anchor_hash,
                fixture.message_id,
                fixture.payload_hash,
                &fixture.payload,
            )
            .is_err(),
            "an old proof must not authenticate under a substituted governed key"
        );

        let mut changed_identity = fixture.source_identity;
        let SccpSourceEmitterV1::Solana(ref mut emitter) = changed_identity.emitter else {
            unreachable!("fixture uses Solana emitter")
        };
        emitter.program_id[0] ^= 1;
        assert!(
            verify_sccp_solana_agave_source_v1(
                &fixture.proof,
                &changed_identity,
                sccp_source_identity_hash_v1(&changed_identity).expect("changed identity hash"),
                fixture.anchor_hash,
                fixture.message_id,
                fixture.payload_hash,
                &fixture.payload,
            )
            .is_err()
        );
    }

    #[test]
    fn native_admission_roundtrips_and_rejects_backend_substitution() {
        let fixture = fixture();
        let trust_anchor = SccpNativeTrustAnchorV1 {
            backend: BridgeNativeProofBackendV1::SolanaAgave,
            anchor_hash: fixture.anchor_hash,
            checkpoint_height: fixture.proof.anchor.checkpoint_slot,
        };
        let inbound = crate::SccpNativeInboundMessageProofV1 {
            version: 1,
            payload: fixture.payload.clone(),
            source: crate::SccpNativeSourceProofEnvelopeV1 {
                version: 1,
                lane: fixture.source_identity.lane,
                source_identity_hash: fixture.source_identity_hash,
                trust_anchor,
                message_id: fixture.message_id,
                payload_hash: fixture.payload_hash,
                source_event_digest: fixture.proof.statement.source_event_digest,
                source_finality: crate::SccpNativeFinalityPointV1 {
                    height: fixture.proof.statement.rooted_slot,
                    block_hash: fixture.proof.statement.rooted_bank_hash,
                },
                proof: crate::SccpNativeSourceProofV1::SolanaAgave(Box::new(fixture.proof.clone())),
            },
        };
        let validated = crate::verify_sccp_native_inbound_message_proof_v1(
            &inbound,
            &fixture.source_identity,
            trust_anchor,
        )
        .expect("complete Solana native admission");
        assert_eq!(
            validated.anchor_interval_height,
            fixture.proof.statement.rooted_slot
        );
        assert_eq!(
            validated.source_finality.height,
            fixture.proof.statement.rooted_slot
        );

        let bytes = crate::encode_sccp_native_inbound_message_proof_v1(&inbound)
            .expect("canonical native proof");
        assert_eq!(
            crate::decode_sccp_native_inbound_message_proof_v1(&bytes),
            Ok(inbound.clone())
        );
        let json = norito::json::to_json(&inbound).expect("native proof JSON");
        assert_eq!(
            crate::decode_sccp_native_inbound_message_proof_json_v1(&json),
            Ok(inbound.clone())
        );

        let route_configuration_hash = [0xd1; 32];
        let bridge = crate::bridge_native_protocol_proof_v1(&inbound, route_configuration_hash)
            .expect("closed bridge proof");
        assert_eq!(bridge.backend, BridgeNativeProofBackendV1::SolanaAgave);
        assert_eq!(
            crate::decode_bridge_native_protocol_proof_v1(&bridge),
            Ok(inbound.clone())
        );
        let mut substituted = bridge.clone();
        substituted.backend = BridgeNativeProofBackendV1::EthereumBeacon;
        assert_eq!(
            crate::decode_bridge_native_protocol_proof_v1(&substituted),
            Err(crate::SccpNativeAdmissionErrorV1::BackendMismatch)
        );

        let mut substituted_anchor = inbound.clone();
        substituted_anchor.source.trust_anchor.backend = BridgeNativeProofBackendV1::TronDpos;
        assert!(crate::encode_sccp_native_inbound_message_proof_v1(&substituted_anchor).is_err());
        let mut mismatched_finality = inbound.clone();
        mismatched_finality.source.source_finality.height += 1;
        assert!(crate::encode_sccp_native_inbound_message_proof_v1(&mismatched_finality).is_err());
        let mut trailing = bytes;
        trailing.push(0);
        assert_eq!(
            crate::decode_sccp_native_inbound_message_proof_v1(&trailing),
            Err(crate::SccpNativeAdmissionErrorV1::InvalidNoritoEncoding)
        );
    }
}
