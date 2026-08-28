//! SCCP payload, proof, and counterparty submission helpers for Iroha bridge flows.
//!
//! SCCP V1 supports Ethereum mainnet, BSC mainnet, TRON mainnet, and TON mainnet as complete
//! bidirectional route families. No testnet or additional external-network profile is decodable.
//! SCCP will not support Sub&#115;trate/Pol&#107;adot networks for now; treat that as launch scope, not
//! pending compatibility work.
//!
//! The crate targets the Rust standard library unconditionally.
//! BLS verification for Taira and BSC finality is also unconditional so Cargo
//! feature selection cannot change consensus admission results.
extern crate alloc;
mod replay_archive;
pub use replay_archive::*;
mod source_identity;
pub use source_identity::*;
mod ethereum_native;
pub use ethereum_native::*;
mod ethereum_source;
pub use ethereum_source::*;
mod bsc_native;
pub use bsc_native::*;
mod tron_native;
pub use tron_native::*;
mod ton_native;
pub use ton_native::*;
mod native_admission;
pub use native_admission::*;
#[cfg(any(test, feature = "test-fixtures"))]
mod test_fixtures;
use alloc::{borrow::ToOwned, format, string::String, vec::Vec};
use blake2::{
    Blake2bVar,
    digest::{Update, VariableOutput},
};
#[cfg(test)]
use halo2curves::ff::Field;
use halo2curves::ff::PrimeField;
use halo2curves::{
    CurveAffine,
    bls12381::{
        self, Fr as Bls12381Fr, G1Affine as Bls12381G1Affine, G2Affine as Bls12381G2Affine,
    },
    bn256::{self, Fq, Fq2, Fr, G1Affine, G2Affine},
    group::{Curve, Group, GroupEncoding, cofactor::CofactorGroup, prime::PrimeCurveAffine},
    pairing::MillerLoopResult,
};
use iroha_crypto::Algorithm;
#[cfg(test)]
use iroha_crypto::KeyPair;
#[cfg(test)]
use iroha_data_model::bridge::{
    SccpGroth16Bn254SemanticCircuitV1, sccp_groth16_bn254_public_signal_schema_hash_v1,
    sccp_sora_taira_chain_id_hash_v1,
};
use iroha_data_model::{
    NetworkId,
    account::{AccountController, AccountId},
    block::BlockHeader,
    bridge::{
        BRIDGE_FINALITY_PROOF_VERSION_V2, BridgeSccpDestinationProofBackendV1,
        BridgeSccpDestinationProofV1, SCCP_V1_TAIRA_TO_TON_TOKEN_MULTIPLIER, SCCP_V1_TON_MAX_COINS,
        SccpBn254G1PointV1, SccpBn254G2PointV1, SccpDestinationDeploymentV1, SccpGovernedRouteV1,
        SccpGroth16Bls12381VerifyingKeyV1, SccpGroth16Bn254VerifyingKeyV1,
        SccpOutboundProofPolicyV1, SccpSemanticProofProfileV1, SccpSoraFinalityAnchorV1,
        SccpTonAddressV1, SccpTonDestinationDeploymentV1,
        canonical_sccp_groth16_bls12381_verifying_key_bytes_v1 as canonical_structural_sccp_groth16_bls12381_verifying_key_bytes_v1,
        canonical_sccp_semantic_proof_profile_bytes_v1,
        canonical_sccp_sora_finality_anchor_bytes_v1,
        sccp_groth16_bls12381_public_signal_schema_hash_v1, sccp_semantic_proof_profile_hash_v1,
        sccp_sora_finality_anchor_hash_v1,
        sccp_ton_groth16_bls12381_proof_profile_commitment_v1 as structural_sccp_ton_groth16_bls12381_proof_profile_commitment_v1,
    },
};
use norito::to_bytes;
use sha2::{Digest as _, Sha256};
#[cfg(any(test, feature = "test-fixtures"))]
pub use test_fixtures::{
    SccpExactOutboundTestFixtureV1, SccpExactTonOutboundTestFixtureV1,
    SccpFinalizedBlockTestFixtureV1, sccp_exact_evm_governed_route_test_fixture_v1,
    sccp_exact_outbound_test_fixture_for_nonce_v1, sccp_exact_outbound_test_fixture_v1,
    sccp_exact_ton_governed_route_test_fixture_v1, sccp_exact_ton_outbound_test_fixture_v1,
    sccp_finalize_taira_block_test_fixture_v1, sccp_sora_outbound_execution_policy_test_fixture_v1,
};
use tiny_keccak::Hasher;
#[cfg(any(test, feature = "test-fixtures"))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
/// Per-thread work counters for the closed SCCP destination-proof path.
///
/// This instrumentation is compiled only for crate tests or with the existing `test-fixtures`
/// feature, so production verification does not pay for atomic or thread-local accounting.
pub struct SccpDestinationProofWorkCountersV1 {
    /// Canonical outer destination artifacts decoded on this thread.
    pub artifact_framing_decodes: usize,
    /// Canonical embedded Taira message bundles decoded on this thread.
    pub bundle_decodes: usize,
    /// BN254 Groth16 pairing equations evaluated on this thread.
    pub groth16_pairings: usize,
    /// Taira commit-QC BLS aggregates evaluated on this thread.
    pub bls_verifications: usize,
    /// BLS12-381 points decompressed and subgroup-checked on this thread.
    pub bls12381_point_decodes: usize,
}
#[cfg(any(test, feature = "test-fixtures"))]
std::thread_local! {
    static SCCP_DESTINATION_PROOF_WORK_COUNTERS_V1:
        core::cell::Cell<SccpDestinationProofWorkCountersV1> = const {
            core::cell::Cell::new(SccpDestinationProofWorkCountersV1 {
                artifact_framing_decodes: 0,
                bundle_decodes: 0,
                groth16_pairings: 0,
                bls_verifications: 0,
                bls12381_point_decodes: 0,
            })
        };
}
#[cfg(any(test, feature = "test-fixtures"))]
fn update_sccp_destination_proof_work_counters_v1(
    update: impl FnOnce(&mut SccpDestinationProofWorkCountersV1),
) {
    SCCP_DESTINATION_PROOF_WORK_COUNTERS_V1.with(|counter| {
        let mut value = counter.get();
        update(&mut value);
        counter.set(value);
    });
}
#[cfg(any(test, feature = "test-fixtures"))]
fn count_sccp_destination_artifact_decode_v1() {
    update_sccp_destination_proof_work_counters_v1(|value| {
        value.artifact_framing_decodes = value.artifact_framing_decodes.saturating_add(1);
    });
}
#[cfg(not(any(test, feature = "test-fixtures")))]
fn count_sccp_destination_artifact_decode_v1() {}
#[cfg(any(test, feature = "test-fixtures"))]
fn count_sccp_destination_bundle_decode_v1() {
    update_sccp_destination_proof_work_counters_v1(|value| {
        value.bundle_decodes = value.bundle_decodes.saturating_add(1);
    });
}
#[cfg(not(any(test, feature = "test-fixtures")))]
fn count_sccp_destination_bundle_decode_v1() {}
#[cfg(any(test, feature = "test-fixtures"))]
fn count_sccp_destination_groth16_pairing_v1() {
    update_sccp_destination_proof_work_counters_v1(|value| {
        value.groth16_pairings = value.groth16_pairings.saturating_add(1);
    });
}
#[cfg(not(any(test, feature = "test-fixtures")))]
fn count_sccp_destination_groth16_pairing_v1() {}
#[cfg(any(test, feature = "test-fixtures"))]
fn count_sccp_destination_bls_verification_v1() {
    update_sccp_destination_proof_work_counters_v1(|value| {
        value.bls_verifications = value.bls_verifications.saturating_add(1);
    });
}
#[cfg(not(any(test, feature = "test-fixtures")))]
fn count_sccp_destination_bls_verification_v1() {}
#[cfg(any(test, feature = "test-fixtures"))]
fn count_sccp_destination_bls12381_point_decode_v1() {
    update_sccp_destination_proof_work_counters_v1(|value| {
        value.bls12381_point_decodes = value.bls12381_point_decodes.saturating_add(1);
    });
}
#[cfg(not(any(test, feature = "test-fixtures")))]
fn count_sccp_destination_bls12381_point_decode_v1() {}
#[cfg(any(test, feature = "test-fixtures"))]
/// Reset closed destination-proof work counters for the current test thread.
pub fn reset_sccp_destination_proof_work_counters_v1() {
    SCCP_DESTINATION_PROOF_WORK_COUNTERS_V1.with(|counter| {
        counter.set(SccpDestinationProofWorkCountersV1::default());
    });
}
#[cfg(any(test, feature = "test-fixtures"))]
/// Snapshot closed destination-proof work counters for the current test thread.
pub fn sccp_destination_proof_work_counters_v1() -> SccpDestinationProofWorkCountersV1 {
    SCCP_DESTINATION_PROOF_WORK_COUNTERS_V1.with(core::cell::Cell::get)
}
/// SCCP protocol domain assigned to SORA networks.
pub const SCCP_DOMAIN_SORA: u32 = 0;
/// SCCP protocol domain assigned to Ethereum networks.
pub const SCCP_DOMAIN_ETH: u32 = 1;
/// SCCP protocol domain assigned to BNB Smart Chain networks.
pub const SCCP_DOMAIN_BSC: u32 = 2;
/// SCCP protocol domain assigned to TON networks.
pub const SCCP_DOMAIN_TON: u32 = 4;
/// SCCP protocol domain assigned to TRON networks.
pub const SCCP_DOMAIN_TRON: u32 = 5;
/// Public TAIRA chain label retained as SCCP deployment metadata.
pub const SCCP_TAIRA_CHAIN_ID_V1: &str = "fc56984b-2be7-431d-840e-21514d1883f0";
/// Canonical checked TAIRA network identity bound into TAIRA-origin SCCP finality proofs.
pub const SCCP_TAIRA_FINALITY_NETWORK_ID_V1: &str =
    "hash:82531CE8EAE8BFF6BEECA4698BFD13A3BC8BEC5F0EE0D23D428C97FC17AB0F3B#3E94";
/// Return the exact genesis-derived TAIRA network identity governed by SCCP V1.
#[must_use]
pub fn sccp_taira_finality_network_id_v1() -> NetworkId {
    SCCP_TAIRA_FINALITY_NETWORK_ID_V1
        .parse()
        .expect("compiled SCCP Taira network identity must be canonical")
}
/// Canonical I105 chain discriminant required for every SCCP Taira account literal.
pub const SCCP_TAIRA_I105_DISCRIMINANT_V1: u16 = 369;
/// TAIRA SCCP route id used for the XOR bridge to TRON mainnet.
pub const SCCP_TAIRA_TRON_XOR_ROUTE_ID_V1: &str = "taira_tron_xor";
/// TAIRA SCCP route id used for the exact XOR bridge to Ethereum mainnet.
pub const SCCP_TAIRA_ETH_XOR_ROUTE_ID_V1: &str = "taira_eth_xor";
/// TAIRA SCCP route id used for the XOR bridge to BSC mainnet.
pub const SCCP_TAIRA_BSC_XOR_ROUTE_ID_V1: &str = "taira_bsc_xor";
/// TAIRA SCCP route id used for the exact XOR bridge to TON mainnet.
pub const SCCP_TAIRA_TON_XOR_ROUTE_ID_V1: &str = "taira_ton_xor";
/// TAIRA SCCP asset key for XOR in every exact EVM-family and TRON route.
pub const SCCP_TAIRA_XOR_ASSET_KEY_V1: &str = "xor";
/// Exact Solidity/TVM value-moving route entrypoint for Taira finalization.
pub const SCCP_FINALIZE_FROM_TAIRA_ABI_V1: &str =
    "finalizeFromTaira(bytes,bytes32[6],bytes32,bytes)";
/// Keccak-256 selector for [`SCCP_FINALIZE_FROM_TAIRA_ABI_V1`].
pub const SCCP_FINALIZE_FROM_TAIRA_SELECTOR_V1: [u8; 4] = [0x95, 0xd7, 0x57, 0xc4];
/// Printable ASCII or an exact canonical I105 account identifier.
pub const SCCP_CODEC_CANONICAL_TEXT: u8 = 1;
/// Raw nonzero 20-byte EVM account address.
pub const SCCP_CODEC_EVM_ADDRESS20: u8 = 2;
/// Raw nonzero TRON account including its mandatory `0x41` network prefix.
pub const SCCP_CODEC_TRON_ADDRESS21: u8 = 5;
/// Raw TON account: signed big-endian `i32` workchain followed by a nonzero
/// 32-byte account id.
///
/// V1 value-moving routes require workchain `0`; friendly/base64 flags and
/// checksums are presentation-only and are never admitted as alternate wire
/// encodings.
pub const SCCP_CODEC_TON_ACCOUNT36: u8 = 7;
/// Maximum byte length of one canonical textual SCCP wire value.
pub const SCCP_MAX_CANONICAL_TEXT_BYTES_V1: usize = 256;
/// Closed list of external protocol domains implemented by SCCP V1.
pub const SCCP_CORE_REMOTE_DOMAINS: [u32; 4] = [
    SCCP_DOMAIN_ETH,
    SCCP_DOMAIN_BSC,
    SCCP_DOMAIN_TON,
    SCCP_DOMAIN_TRON,
];
/// Remote SCCP domains in the current supported production launch scope.
pub const SCCP_SUPPORTED_LAUNCH_REMOTE_DOMAINS_V1: [u32; 4] = [
    SCCP_DOMAIN_ETH,
    SCCP_DOMAIN_BSC,
    SCCP_DOMAIN_TON,
    SCCP_DOMAIN_TRON,
];
/// Return whether every key in an account controller is executable by the V1
/// EVM/TVM destination contracts.
///
/// Rust supports additional account-key algorithms, but accepting one as a Taira-origin SCCP sender
/// would create an outbound lock that the immutable first-release destination routes cannot parse
/// exactly. V1 therefore admits single-key and canonical multisig controllers composed only from
/// Ed25519 and compressed secp256k1 public keys. This check is an economic admission rule, not a
/// signature-policy shortcut: normal transaction authorization still verifies the complete
/// controller before this predicate is reached.
#[must_use]
pub fn sccp_destination_contract_supports_account_v1(account: &AccountId) -> bool {
    fn supports_key(key: &iroha_crypto::PublicKey) -> bool {
        matches!(
            key.try_algorithm(),
            Ok(Algorithm::Ed25519 | Algorithm::Secp256k1)
        )
    }
    match account.controller() {
        AccountController::Single(key) => supports_key(key),
        AccountController::Multisig(policy) => policy
            .members()
            .iter()
            .all(|member| supports_key(member.public_key())),
    }
}
/// External protocol domains that can safely originate native SCCP messages in V1.
///
pub const SCCP_NATIVE_INBOUND_REMOTE_DOMAINS_V1: [u32; 4] = [
    SCCP_DOMAIN_ETH,
    SCCP_DOMAIN_BSC,
    SCCP_DOMAIN_TON,
    SCCP_DOMAIN_TRON,
];
/// External domains with a checked-in value-moving outbound route implementation.
///
pub const SCCP_VALUE_MOVING_OUTBOUND_REMOTE_DOMAINS_V1: [u32; 4] = [
    SCCP_DOMAIN_ETH,
    SCCP_DOMAIN_BSC,
    SCCP_DOMAIN_TON,
    SCCP_DOMAIN_TRON,
];
/// Domain separator for the single V1 SCCP message-identity construction.
///
/// The preimage binds the exact directed network lane and canonical payload.
/// Governed destination deployment bindings are deliberately excluded so a
/// binding rotation cannot make the same economic message replayable.
pub const SCCP_LANE_MESSAGE_ID_PREFIX_V1: &[u8] = b"sccp:lane-message-id:v1";
const SCCP_HUB_LEAF_PREFIX_V1: &[u8] = b"sccp:hub:leaf:v1";
const SCCP_HUB_NODE_PREFIX_V1: &[u8] = b"sccp:hub:node:v1";
const SCCP_PAYLOAD_HASH_PREFIX_V1: &[u8] = b"sccp:payload:v1";
const SCCP_GROTH16_STATEMENT_PREFIX_V1: &[u8] = b"sccp:groth16-bn254:statement:v1";
const SCCP_GROTH16_PROOF_REQUEST_PREFIX_V1: &[u8] = b"sccp:groth16-bn254:proof-request:v1";
const SCCP_GROTH16_PROOF_RESULT_PREFIX_V1: &[u8] = b"sccp:groth16-bn254:proof-result:v1";
const SCCP_TON_GROTH16_STATEMENT_PREFIX_V1: &[u8] = b"sccp:groth16-bls12381:statement:v1";
const SCCP_TON_GROTH16_PROOF_REQUEST_PREFIX_V1: &[u8] = b"sccp:groth16-bls12381:proof-request:v1";
const SCCP_TON_GROTH16_PROOF_RESULT_PREFIX_V1: &[u8] = b"sccp:groth16-bls12381:proof-result:v1";
const SCCP_TON_GROTH16_SIGNAL_MESSAGE_ID_V1: &[u8] = b"sccp:groth16-bls12381:signal:message-id:v1";
const SCCP_TON_GROTH16_SIGNAL_PAYLOAD_HASH_V1: &[u8] =
    b"sccp:groth16-bls12381:signal:payload-hash:v1";
const SCCP_TON_GROTH16_SIGNAL_TARGET_DOMAIN_V1: &[u8] =
    b"sccp:groth16-bls12381:signal:target-domain:v1";
const SCCP_TON_GROTH16_SIGNAL_COMMITMENT_ROOT_V1: &[u8] =
    b"sccp:groth16-bls12381:signal:commitment-root:v1";
const SCCP_TON_GROTH16_SIGNAL_FINALITY_HEIGHT_V1: &[u8] =
    b"sccp:groth16-bls12381:signal:finality-height:v1";
const SCCP_TON_GROTH16_SIGNAL_FINALITY_BLOCK_HASH_V1: &[u8] =
    b"sccp:groth16-bls12381:signal:finality-block-hash:v1";
const SCCP_TON_GROTH16_SIGNAL_SOURCE_DOMAIN_V1: &[u8] =
    b"sccp:groth16-bls12381:signal:source-domain:v1";
const SCCP_TON_GROTH16_SIGNAL_STATEMENT_HASH_V1: &[u8] =
    b"sccp:groth16-bls12381:signal:statement-hash:v1";
const SCCP_TON_GROTH16_SIGNAL_DESTINATION_BINDING_HASH_V1: &[u8] =
    b"sccp:groth16-bls12381:signal:destination-binding-hash:v1";
const SCCP_TON_GROTH16_SIGNAL_ROUTE_CONFIGURATION_HASH_V1: &[u8] =
    b"sccp:groth16-bls12381:signal:route-config-hash:v1";
const SCCP_TON_GROTH16_SIGNAL_SORA_FINALITY_ANCHOR_HASH_V1: &[u8] =
    b"sccp:groth16-bls12381:signal:sora-finality-anchor-hash:v1";
/// Maximum canonical Norito size of a Taira SCCP proof artifact.
pub const SCCP_TAIRA_MAX_ENCODED_PROOF_BYTES_V1: usize = 16 * 1024 * 1024;
/// Maximum canonical Norito size of a Groth16 request, result, or complete artifact.
///
/// A request may contain one maximum-sized Taira bundle. The fixed allowance
/// covers the closed 38-word verification key, proof, hashes, and Norito
/// framing without making the admission bound depend on decoded lengths.
pub const SCCP_GROTH16_BN254_MAX_ENCODED_ARTIFACT_BYTES_V1: usize =
    SCCP_TAIRA_MAX_ENCODED_PROOF_BYTES_V1 + 64 * 1024;
/// Maximum padded-base64 size accepted by an HTTP adapter for one Groth16 artifact.
pub const SCCP_GROTH16_BN254_MAX_BASE64_ARTIFACT_BYTES_V1: usize =
    4 * SCCP_GROTH16_BN254_MAX_ENCODED_ARTIFACT_BYTES_V1.div_ceil(3);
/// Maximum canonical bytes in one closed destination-proof envelope.
pub const SCCP_DESTINATION_PROOF_MAX_ENCODED_BYTES_V1: usize =
    SCCP_GROTH16_BN254_MAX_ENCODED_ARTIFACT_BYTES_V1 + 64 * 1024;
/// Maximum canonical padded-base64 length of one closed destination-proof envelope.
pub const SCCP_DESTINATION_PROOF_MAX_BASE64_BYTES_V1: usize =
    4 * SCCP_DESTINATION_PROOF_MAX_ENCODED_BYTES_V1.div_ceil(3);
/// Maximum canonical JSON size accepted for a Groth16 request, result, or artifact.
pub const SCCP_GROTH16_BN254_MAX_JSON_ARTIFACT_BYTES_V1: usize =
    2 * SCCP_GROTH16_BN254_MAX_ENCODED_ARTIFACT_BYTES_V1 + 256 * 1024;
/// Exact compressed proof size: one G1, one G2, and one G1 point.
pub const SCCP_TON_GROTH16_BLS12381_PROOF_BYTES_V1: usize = 48 + 96 + 48;
/// Exact compressed verifying-key size for eleven signals and twelve IC points.
pub const SCCP_TON_GROTH16_BLS12381_VERIFYING_KEY_BYTES_V1: usize = 1 + 48 + 3 * 96 + 12 * 48;
/// Largest canonical transfer payload admitted by the TON contract boundary.
pub const SCCP_TON_DESTINATION_MAX_PAYLOAD_BYTES_V1: usize = 374;
const SCCP_TON_CANONICAL_PAYLOAD_HEADER_BYTES_V1: usize = 50;
const SCCP_TON_CANONICAL_PAYLOAD_CHUNK_BYTES_V1: usize = 100;
/// TL-B opcode of `SccpFinalizeFromTaira`.
pub const SCCP_TON_FINALIZE_FROM_TAIRA_OPCODE_V1: u32 = 0x5343_4350;
/// Standard TON Bag-of-Cells magic prefix.
pub const SCCP_TON_BOC_MAGIC_V1: [u8; 4] = [0xb5, 0xee, 0x9c, 0x72];
/// Maximum number of sibling nodes in a Taira SCCP commitment proof.
pub const SCCP_TAIRA_MAX_MERKLE_PROOF_STEPS_V1: usize = 64;
/// Maximum validator roster accepted in a Taira SCCP commit QC.
pub const SCCP_TAIRA_MAX_FINALITY_VALIDATORS_V1: usize = 4_096;
const SCCP_TAIRA_MAX_BLOCK_HEADER_BYTES_V1: usize = 256 * 1024;
const SCCP_TAIRA_MAX_BLS_PROOF_BYTES_V1: usize = 256;
const SCCP_NORITO_COMPRESSION_OFFSET: usize = 4 + 1 + 1 + 16;
const SCCP_NORITO_LENGTH_OFFSET: usize = SCCP_NORITO_COMPRESSION_OFFSET + 1;
/// BSC system-contract address that publishes the active validator set.
pub const SCCP_BSC_VALIDATOR_SET_CONTRACT_ADDRESS: [u8; 20] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x10, 0x00,
];
/// Ethereum mainnet slots per epoch from the consensus mainnet preset.
pub const SCCP_ETH_MAINNET_SLOTS_PER_EPOCH: u64 = 32;
/// Ethereum mainnet epochs per sync committee period from the Altair preset.
pub const SCCP_ETH_MAINNET_EPOCHS_PER_SYNC_COMMITTEE_PERIOD: u64 = 256;
/// Ethereum mainnet slots per sync committee period.
pub const SCCP_ETH_MAINNET_SLOTS_PER_SYNC_COMMITTEE_PERIOD: u64 =
    SCCP_ETH_MAINNET_SLOTS_PER_EPOCH * SCCP_ETH_MAINNET_EPOCHS_PER_SYNC_COMMITTEE_PERIOD;
/// Return the Ethereum mainnet sync committee period for a beacon slot.
///
/// This follows the consensus-spec rule
/// `compute_epoch_at_slot(slot) // EPOCHS_PER_SYNC_COMMITTEE_PERIOD`.
pub const fn sccp_eth_mainnet_sync_committee_period_for_slot(slot: u64) -> u64 {
    slot / SCCP_ETH_MAINNET_SLOTS_PER_SYNC_COMMITTEE_PERIOD
}
/// Byte length of an EVM SCCP source-route contract address.
pub const SCCP_EVM_SOURCE_BRIDGE_EMITTER_ADDRESS_BYTES: usize = 20;
const SCCP_GROTH16_BN254_SIGNAL_MESSAGE_ID_V1: &[u8] = b"sccp:groth16-bn254:signal:message-id:v1";
const SCCP_GROTH16_BN254_SIGNAL_PAYLOAD_HASH_V1: &[u8] =
    b"sccp:groth16-bn254:signal:payload-hash:v1";
const SCCP_GROTH16_BN254_SIGNAL_TARGET_DOMAIN_V1: &[u8] =
    b"sccp:groth16-bn254:signal:target-domain:v1";
const SCCP_GROTH16_BN254_SIGNAL_COMMITMENT_ROOT_V1: &[u8] =
    b"sccp:groth16-bn254:signal:commitment-root:v1";
const SCCP_GROTH16_BN254_SIGNAL_FINALITY_HEIGHT_V1: &[u8] =
    b"sccp:groth16-bn254:signal:finality-height:v1";
const SCCP_GROTH16_BN254_SIGNAL_FINALITY_BLOCK_HASH_V1: &[u8] =
    b"sccp:groth16-bn254:signal:finality-block-hash:v1";
const SCCP_GROTH16_BN254_SIGNAL_SOURCE_DOMAIN_V1: &[u8] =
    b"sccp:groth16-bn254:signal:source-domain:v1";
const SCCP_GROTH16_BN254_SIGNAL_STATEMENT_HASH_V1: &[u8] =
    b"sccp:groth16-bn254:signal:statement-hash:v1";
const SCCP_GROTH16_BN254_SIGNAL_DESTINATION_BINDING_HASH_V1: &[u8] =
    b"sccp:groth16-bn254:signal:destination-binding-hash:v1";
const SCCP_GROTH16_BN254_SIGNAL_ROUTE_CONFIGURATION_HASH_V1: &[u8] =
    b"sccp:groth16-bn254:signal:route-configuration-hash:v1";
const SCCP_GROTH16_BN254_SIGNAL_SORA_FINALITY_ANCHOR_HASH_V1: &[u8] =
    b"sccp:groth16-bn254:signal:sora-finality-anchor-hash:v1";
const SCCP_GROTH16_BN254_SCALAR_FIELD_MODULUS_BE: H256 = [
    0x30, 0x64, 0x4e, 0x72, 0xe1, 0x31, 0xa0, 0x29, 0xb8, 0x50, 0x45, 0xb6, 0x81, 0x81, 0x58, 0x5d,
    0x28, 0x33, 0xe8, 0x48, 0x79, 0xb9, 0x70, 0x91, 0x43, 0xe1, 0xf5, 0x93, 0xf0, 0x00, 0x00, 0x01,
];
const SCCP_GROTH16_BLS12381_SCALAR_FIELD_MODULUS_BE: H256 = [
    0x73, 0xed, 0xa7, 0x53, 0x29, 0x9d, 0x7d, 0x48, 0x33, 0x39, 0xd8, 0x08, 0x09, 0xa1, 0xd8, 0x05,
    0x53, 0xbd, 0xa4, 0x02, 0xff, 0xfe, 0x5b, 0xfe, 0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x01,
];
const SCCP_GROTH16_BLS12381_BASE_FIELD_MODULUS_BE: [u8; 48] = [
    0x1a, 0x01, 0x11, 0xea, 0x39, 0x7f, 0xe6, 0x9a, 0x4b, 0x1b, 0xa7, 0xb6, 0x43, 0x4b, 0xac, 0xd7,
    0x64, 0x77, 0x4b, 0x84, 0xf3, 0x85, 0x12, 0xbf, 0x67, 0x30, 0xd2, 0xa0, 0xf6, 0xb0, 0xf6, 0x24,
    0x1e, 0xab, 0xff, 0xfe, 0xb1, 0x53, 0xff, 0xff, 0xb9, 0xfe, 0xff, 0xff, 0xff, 0xff, 0xaa, 0xab,
];
/// Fixed 256-bit protocol hash or word.
pub type H256 = [u8; 32];
const SECP256K1_SCALAR_ORDER_BE: H256 = [
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfe,
    0xba, 0xae, 0xdc, 0xe6, 0xaf, 0x48, 0xa0, 0x3b, 0xbf, 0xd2, 0x5e, 0x8c, 0xd0, 0x36, 0x41, 0x41,
];
const SECP256K1_SCALAR_HALF_ORDER_BE: H256 = [
    0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0x5d, 0x57, 0x6e, 0x73, 0x57, 0xa4, 0x50, 0x1d, 0xdf, 0xe9, 0x2f, 0x46, 0x68, 0x1b, 0x20, 0xa0,
];
const BN254_BASE_FIELD_MODULUS_BE: H256 = [
    0x30, 0x64, 0x4e, 0x72, 0xe1, 0x31, 0xa0, 0x29, 0xb8, 0x50, 0x45, 0xb6, 0x81, 0x81, 0x58, 0x5d,
    0x97, 0x81, 0x6a, 0x91, 0x68, 0x71, 0xca, 0x8d, 0x3c, 0x20, 0x8c, 0x16, 0xd8, 0x7c, 0xfd, 0x47,
];
fn encode_lower_hex(bytes: &[u8]) -> String {
    const LUT: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(LUT[usize::from(byte >> 4)] as char);
        out.push(LUT[usize::from(byte & 0x0f)] as char);
    }
    out
}
fn encode_0x_lower_hex(bytes: &[u8]) -> String {
    format!("0x{}", encode_lower_hex(bytes))
}
fn decode_ascii_lower_hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}
#[cfg(any(test, feature = "test-fixtures"))]
fn decode_fixed_hex_bytes<const N: usize>(value: &str) -> Option<[u8; N]> {
    let raw = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value)
        .as_bytes();
    if raw.len() != N * 2 {
        return None;
    }
    let mut out = [0u8; N];
    for (idx, chunk) in raw.chunks_exact(2).enumerate() {
        let hi = decode_ascii_lower_hex_nibble(chunk[0])?;
        let lo = decode_ascii_lower_hex_nibble(chunk[1])?;
        out[idx] = (hi << 4) | lo;
    }
    Some(out)
}
fn decode_hex_bytes(value: &str) -> Option<Vec<u8>> {
    let raw = value.strip_prefix("0x")?.as_bytes();
    if !raw.len().is_multiple_of(2) {
        return None;
    }
    let mut out = Vec::with_capacity(raw.len() / 2);
    for chunk in raw.chunks_exact(2) {
        let hi = decode_ascii_lower_hex_nibble(chunk[0])?;
        let lo = decode_ascii_lower_hex_nibble(chunk[1])?;
        out.push((hi << 4) | lo);
    }
    Some(out)
}
fn decode_canonical_0x_lower_hex_fixed<const N: usize>(value: &str) -> Option<[u8; N]> {
    let raw = value.strip_prefix("0x")?.as_bytes();
    if raw.len() != N * 2 {
        return None;
    }
    let mut out = [0u8; N];
    for (index, chunk) in raw.chunks_exact(2).enumerate() {
        let high = decode_ascii_lower_hex_nibble(chunk[0])?;
        let low = decode_ascii_lower_hex_nibble(chunk[1])?;
        out[index] = (high << 4) | low;
    }
    Some(out)
}
mod json_utils {
    use alloc::{
        format,
        string::{String, ToString},
        vec::Vec,
    };
    use norito::json::{self, Error, JsonDeserialize, Parser};
    fn encode_hex(bytes: &[u8]) -> String {
        const LUT: &[u8; 16] = b"0123456789abcdef";
        let mut out = String::with_capacity(2 + bytes.len() * 2);
        out.push_str("0x");
        for byte in bytes {
            out.push(LUT[usize::from(byte >> 4)] as char);
            out.push(LUT[usize::from(byte & 0x0f)] as char);
        }
        out
    }
    fn decode_hex_vec(value: &str) -> Result<Vec<u8>, Error> {
        super::decode_hex_bytes(value).ok_or_else(|| {
            Error::Message("expected canonical lowercase 0x-prefixed hex byte string".into())
        })
    }
    fn decode_hex_fixed<const N: usize>(value: &str) -> Result<[u8; N], Error> {
        super::decode_canonical_0x_lower_hex_fixed::<N>(value).ok_or_else(|| {
            Error::Message(format!(
                "expected canonical lowercase 0x-prefixed {N}-byte hex string"
            ))
        })
    }
    fn unsigned_decimal_string_is_canonical(value: &str) -> bool {
        !value.is_empty()
            && value.as_bytes().iter().all(u8::is_ascii_digit)
            && (value == "0" || !value.starts_with('0'))
    }
    fn parse_canonical_decimal_u64_string(value: &str) -> Result<u64, Error> {
        if !unsigned_decimal_string_is_canonical(value) {
            return Err(Error::Message(
                "expected canonical unsigned u64 decimal string".into(),
            ));
        }
        value
            .parse::<u64>()
            .map_err(|err| Error::Message(format!("failed to parse u64 string: {err}")))
    }
    fn parse_canonical_decimal_u128_string(value: &str) -> Result<u128, Error> {
        if !unsigned_decimal_string_is_canonical(value) {
            return Err(Error::Message(
                "expected canonical unsigned u128 decimal string".into(),
            ));
        }
        value
            .parse::<u128>()
            .map_err(|err| Error::Message(format!("failed to parse u128 string: {err}")))
    }
    fn parse_decimal_u64(parser: &mut Parser<'_>) -> Result<u64, Error> {
        parser.skip_ws();
        if parser.peek() == Some(b'"') {
            return parse_canonical_decimal_u64_string(&parser.parse_string()?);
        }
        parser.parse_u64()
    }
    fn parse_decimal_u128(parser: &mut Parser<'_>) -> Result<u128, Error> {
        parser.skip_ws();
        if parser.peek() == Some(b'"') {
            return parse_canonical_decimal_u128_string(&parser.parse_string()?);
        }
        parser.parse_u64().map(u128::from)
    }
    pub mod hex32 {
        use super::{Error, Parser, decode_hex_fixed, encode_hex, json};
        pub fn serialize(value: &[u8; 32], out: &mut String) {
            json::write_json_string(&encode_hex(value), out);
        }
        pub fn deserialize(parser: &mut Parser<'_>) -> Result<[u8; 32], Error> {
            let value = parser.parse_string()?;
            decode_hex_fixed::<32>(&value)
        }
    }
    pub mod hex20 {
        use super::{Error, Parser, decode_hex_fixed, encode_hex, json};
        pub fn serialize(value: &[u8; 20], out: &mut String) {
            json::write_json_string(&encode_hex(value), out);
        }
        #[expect(
            dead_code,
            reason = "the JSON derive resolves this field hook in generated deserialization code"
        )]
        pub fn deserialize(parser: &mut Parser<'_>) -> Result<[u8; 20], Error> {
            let value = parser.parse_string()?;
            decode_hex_fixed::<20>(&value)
        }
    }
    pub mod bytes_hex {
        use super::{Error, Parser, Vec, decode_hex_vec, encode_hex, json};
        pub fn serialize(value: &[u8], out: &mut String) {
            json::write_json_string(&encode_hex(value), out);
        }
        pub fn deserialize(parser: &mut Parser<'_>) -> Result<Vec<u8>, Error> {
            let value = parser.parse_string()?;
            decode_hex_vec(&value)
        }
    }
    pub mod vec_bytes_hex {
        use super::{
            Error, JsonDeserialize, Parser, String, Vec, decode_hex_vec, encode_hex, json,
        };
        pub fn serialize(value: &[Vec<u8>], out: &mut String) {
            out.push('[');
            for (index, item) in value.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                json::write_json_string(&encode_hex(item), out);
            }
            out.push(']');
        }
        pub fn deserialize(parser: &mut Parser<'_>) -> Result<Vec<Vec<u8>>, Error> {
            let values = <Vec<String> as JsonDeserialize>::json_deserialize(parser)?;
            values
                .into_iter()
                .map(|value| decode_hex_vec(&value))
                .collect()
        }
    }
    pub mod u64_string {
        use super::{Error, Parser, ToString, json, parse_decimal_u64};
        #[expect(
            clippy::trivially_copy_pass_by_ref,
            reason = "norito field serializers receive values by reference"
        )]
        pub fn serialize(value: &u64, out: &mut String) {
            json::write_json_string(&value.to_string(), out);
        }
        pub fn deserialize(parser: &mut Parser<'_>) -> Result<u64, Error> {
            parse_decimal_u64(parser)
        }
    }
    pub mod u128_string {
        use super::{Error, Parser, ToString, json, parse_decimal_u128};
        pub fn serialize(value: &u128, out: &mut String) {
            json::write_json_string(&value.to_string(), out);
        }
        pub fn deserialize(parser: &mut Parser<'_>) -> Result<u128, Error> {
            parse_decimal_u128(parser)
        }
    }
    pub mod canonical_u64_string {
        use super::{Error, Parser, ToString, json, parse_canonical_decimal_u64_string};
        #[expect(
            clippy::trivially_copy_pass_by_ref,
            reason = "norito field serializers receive values by reference"
        )]
        pub fn serialize(value: &u64, out: &mut String) {
            json::write_json_string(&value.to_string(), out);
        }
        pub fn deserialize(parser: &mut Parser<'_>) -> Result<u64, Error> {
            parse_canonical_decimal_u64_string(&parser.parse_string()?)
        }
    }
    pub mod canonical_u128_string {
        use super::{Error, Parser, ToString, json, parse_canonical_decimal_u128_string};
        pub fn serialize(value: &u128, out: &mut String) {
            json::write_json_string(&value.to_string(), out);
        }
        pub fn deserialize(parser: &mut Parser<'_>) -> Result<u128, Error> {
            parse_canonical_decimal_u128_string(&parser.parse_string()?)
        }
    }
}
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
/// Canonical cross-domain asset transfer payload.
pub struct TransferPayloadV1 {
    /// Payload schema version. SCCP V1 requires `1`.
    pub version: u8,
    /// Domain on which the transfer is locked or destroyed.
    pub source_domain: u32,
    /// Domain on which the transfer is released or created.
    pub dest_domain: u32,
    /// Sender-chosen nonce included in the message identity.
    #[norito(with = "json_utils::u64_string")]
    pub nonce: u64,
    /// Nonzero immutable governed-route revision selected by this transfer.
    pub route_revision: u32,
    /// Protocol domain on which the transferred asset is native.
    pub asset_home_domain: u32,
    /// Codec tag describing [`Self::asset_id`].
    pub asset_id_codec: u8,
    /// Canonical identifier of the transferred asset.
    #[norito(with = "json_utils::bytes_hex")]
    pub asset_id: Vec<u8>,
    /// Positive transfer amount expressed in the route's smallest unit.
    #[norito(with = "json_utils::u128_string")]
    pub amount: u128,
    /// Codec tag describing [`Self::sender`].
    pub sender_codec: u8,
    /// Canonical source-chain sender identifier.
    #[norito(with = "json_utils::bytes_hex")]
    pub sender: Vec<u8>,
    /// Codec tag describing [`Self::recipient`].
    pub recipient_codec: u8,
    /// Canonical destination-chain recipient identifier.
    #[norito(with = "json_utils::bytes_hex")]
    pub recipient: Vec<u8>,
    /// Codec tag describing [`Self::route_id`].
    pub route_id_codec: u8,
    /// Canonical route identifier selected by the transfer.
    #[norito(with = "json_utils::bytes_hex")]
    pub route_id: Vec<u8>,
}
#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize,
)]
/// Closed SCCP V1 application-payload union.
pub enum SccpPayloadV1 {
    /// Transfer an asset between domains.
    Transfer(TransferPayloadV1),
}
impl SccpPayloadV1 {
    const TRANSFER_DISCRIMINANT: u8 = 2;
}
/// Failure to encode a value in the canonical SCCP V1 payload layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SccpCanonicalPayloadEncodingErrorV1 {
    /// A variable-length field cannot be represented by the V1 `u32` prefix.
    FieldLengthOverflow {
        /// Stable canonical field name.
        field: &'static str,
        /// Actual field length in bytes.
        actual: usize,
        /// Largest field length representable by the V1 layout.
        maximum: u32,
    },
}
impl core::fmt::Display for SccpCanonicalPayloadEncodingErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::FieldLengthOverflow {
                field,
                actual,
                maximum,
            } => write!(
                formatter,
                "SCCP canonical payload field `{field}` length {actual} exceeds V1 maximum {maximum}"
            ),
        }
    }
}
impl std::error::Error for SccpCanonicalPayloadEncodingErrorV1 {}
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
/// Stable message-kind tag committed by the SCCP hub Merkle tree.
pub enum SccpHubMessageKind {
    /// Asset transfer.
    Transfer,
}
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
/// Merkle-leaf commitment for one outbound SCCP message.
pub struct SccpHubCommitmentV1 {
    /// Commitment schema version. SCCP V1 requires `1`.
    pub version: u8,
    /// Stable application-payload kind.
    pub kind: SccpHubMessageKind,
    /// Exact outbound lane and governed destination deployment binding.
    pub context: SccpOutboundMessageContextV1,
    /// Exact lane-bound message identifier.
    #[norito(with = "json_utils::hex32")]
    pub message_id: H256,
    /// Hash of the canonical application-payload bytes.
    #[norito(with = "json_utils::hex32")]
    pub payload_hash: H256,
}
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
/// One sibling step in an SCCP commitment Merkle proof.
pub struct SccpMerkleStepV1 {
    /// Hash of the sibling node at this level.
    #[norito(with = "json_utils::hex32")]
    pub sibling_hash: H256,
    /// Whether the sibling is concatenated to the left of the running hash.
    pub sibling_is_left: bool,
}
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
/// Bounded Merkle path from one SCCP commitment to its block commitment root.
pub struct SccpMerkleProofV1 {
    /// Bottom-up sibling steps.
    pub steps: Vec<SccpMerkleStepV1>,
}
/// Exact typed Sumeragi-v2 finality proof carried by an SCCP message bundle.
///
/// SCCP intentionally reuses the generic bridge proof type so consensus, bridge, Torii, and
/// destination admission cannot drift into different vote transcripts or quorum rules.
pub type TairaBridgeFinalityProofV1 = iroha_data_model::bridge::BridgeFinalityProof;
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
/// Canonical SORA-origin SCCP message, Merkle inclusion, and Taira finality bundle.
pub struct TairaSccpMessageProofV1 {
    /// Bundle schema version. SCCP V1 requires `1`.
    pub version: u8,
    /// Commitment-tree root authenticated by [`Self::finality_proof`].
    #[norito(with = "json_utils::hex32")]
    pub commitment_root: H256,
    /// Selected message commitment.
    pub commitment: SccpHubCommitmentV1,
    /// Merkle path from [`Self::commitment`] to [`Self::commitment_root`].
    pub merkle_proof: SccpMerkleProofV1,
    /// Canonical payload whose identity and hash are committed by the leaf.
    pub payload: SccpPayloadV1,
    /// Canonical encoded [`TairaBridgeFinalityProofV1`].
    #[norito(with = "json_utils::bytes_hex")]
    pub finality_proof: Vec<u8>,
}
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
/// Public statement exposed to a destination-chain SCCP verifier.
pub struct SccpMessagePublicInputsV1 {
    /// Statement schema version. SCCP V1 requires `1`.
    pub version: u8,
    /// Exact lane-bound SCCP message identifier.
    #[norito(with = "json_utils::hex32")]
    pub message_id: H256,
    /// Hash of the canonical application payload.
    #[norito(with = "json_utils::hex32")]
    pub payload_hash: H256,
    /// Destination SCCP protocol domain.
    pub target_domain: u32,
    /// SCCP commitment root authenticated by Taira finality.
    #[norito(with = "json_utils::hex32")]
    pub commitment_root: H256,
    /// Finalized Taira block height.
    #[norito(with = "json_utils::u64_string")]
    pub finality_height: u64,
    /// Finalized Taira block hash.
    #[norito(with = "json_utils::hex32")]
    pub finality_block_hash: H256,
}
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
#[norito(
    tag = "family",
    content = "target",
    rename_all = "snake_case",
    deny_unknown_fields
)]
/// Exact governed destination contract selected for a verified SCCP call.
pub enum SccpDestinationCallTargetV1 {
    /// EVM route contract on the exact governed EVM network.
    Evm {
        /// Exact destination network.
        network: SccpNetworkV1,
        /// Governed route-contract address.
        #[norito(with = "json_utils::hex20")]
        route_address: [u8; 20],
    },
    /// TRON TVM route contract on the exact governed TRON network.
    Tron {
        /// Exact destination network.
        network: SccpNetworkV1,
        /// Governed route-contract address without the `0x41` network prefix.
        #[norito(with = "json_utils::hex20")]
        route_address: [u8; 20],
    },
    /// TON route contract on an exact zero-state-bound TON network.
    Ton {
        /// Exact destination network.
        network: SccpNetworkV1,
        /// Governed raw basechain route-contract address.
        route_address: SccpTonAddressV1,
    },
}
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
/// State-verified destination call derived from one closed SCCP proof artifact.
pub struct SccpVerifiedDestinationCallV1 {
    /// Call schema version. SCCP V1 requires `1`.
    pub version: u8,
    /// Closed proof backend selected by the governed destination family.
    pub backend: BridgeSccpDestinationProofBackendV1,
    /// Exact external destination domain.
    pub counterparty_domain: u32,
    /// Nonzero historical governed route revision authenticated by the payload.
    pub route_revision: u32,
    /// Exact governed destination binding committed by the message.
    #[norito(with = "json_utils::hex32")]
    pub destination_binding_hash: H256,
    /// Immutable historical governed route configuration committed by the message.
    #[norito(with = "json_utils::hex32")]
    pub route_configuration_hash: H256,
    /// Exact audited semantic circuit profile resolved from governed state.
    pub semantic_proof_profile: SccpSemanticProofProfileV1,
    /// Domain-separated commitment to [`Self::semantic_proof_profile`].
    #[norito(with = "json_utils::hex32")]
    pub semantic_proof_profile_hash: H256,
    /// Exact governed SORA finality checkpoint resolved from state.
    pub sora_finality_anchor: SccpSoraFinalityAnchorV1,
    /// Domain-separated commitment to [`Self::sora_finality_anchor`].
    #[norito(with = "json_utils::hex32")]
    pub sora_finality_anchor_hash: H256,
    /// Exact governed route contract that must receive [`Self::calldata`].
    pub target: SccpDestinationCallTargetV1,
    /// Public statement authenticated by the Groth16 proof.
    pub public_inputs: SccpMessagePublicInputsV1,
    /// Hash of the canonical typed SCCP statement.
    #[norito(with = "json_utils::hex32")]
    pub statement_hash: H256,
    /// Hash of the exact canonical proving request.
    #[norito(with = "json_utils::hex32")]
    pub request_hash: H256,
    /// Canonical fixed-width Groth16 proof envelope.
    #[norito(with = "json_utils::bytes_hex")]
    pub proof_bytes: Vec<u8>,
    /// Exact canonical SCCP transfer payload supplied to the destination route.
    #[norito(with = "json_utils::bytes_hex")]
    pub canonical_payload_bytes: Vec<u8>,
    /// Exact `finalizeFromTaira` calldata derived after all state checks pass.
    #[norito(with = "json_utils::bytes_hex")]
    pub calldata: Vec<u8>,
    /// Original SORA message and finality bundle retained for audit and settlement.
    pub bundle: TairaSccpMessageProofV1,
}
/// Exact eleven BN254 public-signal words consumed by a destination verifier.
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
pub struct SccpGroth16Bn254PublicSignalsV1 {
    /// Domain-separated message-id signal.
    #[norito(with = "json_utils::hex32")]
    pub message_id: H256,
    /// Domain-separated payload-hash signal.
    #[norito(with = "json_utils::hex32")]
    pub payload_hash: H256,
    /// Domain-separated target-domain signal.
    #[norito(with = "json_utils::hex32")]
    pub target_domain: H256,
    /// Domain-separated commitment-root signal.
    #[norito(with = "json_utils::hex32")]
    pub commitment_root: H256,
    /// Domain-separated Taira-finality-height signal.
    #[norito(with = "json_utils::hex32")]
    pub finality_height: H256,
    /// Domain-separated Taira-finality-block-hash signal.
    #[norito(with = "json_utils::hex32")]
    pub finality_block_hash: H256,
    /// Domain-separated source-domain signal.
    #[norito(with = "json_utils::hex32")]
    pub source_domain: H256,
    /// Domain-separated typed-statement-hash signal.
    #[norito(with = "json_utils::hex32")]
    pub statement_hash: H256,
    /// Domain-separated governed-destination-binding signal.
    #[norito(with = "json_utils::hex32")]
    pub destination_binding_hash: H256,
    /// Domain-separated immutable-route-configuration signal.
    #[norito(with = "json_utils::hex32")]
    pub route_configuration_hash: H256,
    /// Domain-separated governed-Taira-finality-anchor signal.
    #[norito(with = "json_utils::hex32")]
    pub sora_finality_anchor_hash: H256,
}
impl SccpGroth16Bn254PublicSignalsV1 {
    /// Return the canonical verifier order.
    #[must_use]
    pub const fn words(self) -> [H256; 11] {
        [
            self.message_id,
            self.payload_hash,
            self.target_domain,
            self.commitment_root,
            self.finality_height,
            self.finality_block_hash,
            self.source_domain,
            self.statement_hash,
            self.destination_binding_hash,
            self.route_configuration_hash,
            self.sora_finality_anchor_hash,
        ]
    }
}
impl From<[H256; 11]> for SccpGroth16Bn254PublicSignalsV1 {
    fn from(words: [H256; 11]) -> Self {
        Self {
            message_id: words[0],
            payload_hash: words[1],
            target_domain: words[2],
            commitment_root: words[3],
            finality_height: words[4],
            finality_block_hash: words[5],
            source_domain: words[6],
            statement_hash: words[7],
            destination_binding_hash: words[8],
            route_configuration_hash: words[9],
            sora_finality_anchor_hash: words[10],
        }
    }
}
fn sha256_bytes(payload: &[u8]) -> H256 {
    Sha256::digest(payload).into()
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum SccpParsedDestinationMaterialV1 {
    Bn254 {
        artifact: SccpGroth16Bn254ProofArtifactV1,
        public_signal_words: [H256; 11],
        groth16_proof: SccpEvmGroth16Bn254ProofV1,
    },
    TonBls12381 {
        artifact: SccpTonGroth16Bls12381ProofArtifactV1,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Curve-specific cryptographic work required after structural parsing.
pub enum SccpDestinationProofCryptoWorkV1 {
    /// One four-term Groth16 pairing equation over BN254.
    Groth16Bn254Pairing,
    /// One four-term Groth16 pairing equation over BLS12-381.
    Groth16Bls12381Pairing,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Curve-neutral result fields exposed by an opaque parsed artifact.
pub struct SccpParsedDestinationProofResultV1 {
    /// Commitment to the exact proof result and request.
    pub result_hash: H256,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Curve-neutral artifact fields needed by Core before route verification.
pub struct SccpParsedDestinationArtifactV1 {
    /// Exact result commitment used by the outer proof anti-alias check.
    pub result: SccpParsedDestinationProofResultV1,
}
#[derive(Clone, Debug, PartialEq, Eq)]
/// One canonically framed destination artifact with its embedded bundle and
/// finality proof decoded exactly once but not yet trusted against governance.
///
/// Fields are intentionally private. Callers may inspect them to resolve the authoritative
/// historical route. Call derivation additionally requires the exact trusted finality value
/// resolved by the caller's local authority boundary.
pub struct SccpParsedDestinationProofV1 {
    artifact: SccpParsedDestinationArtifactV1,
    backend: BridgeSccpDestinationProofBackendV1,
    material: SccpParsedDestinationMaterialV1,
    bundle: TairaSccpMessageProofV1,
    finality: TairaBridgeFinalityProofV1,
    canonical_payload_bytes: Vec<u8>,
}
impl SccpParsedDestinationProofV1 {
    /// Return the curve-neutral artifact commitment fields.
    #[must_use]
    pub const fn artifact(&self) -> &SccpParsedDestinationArtifactV1 {
        &self.artifact
    }
    /// Return the closed destination proof backend selected by the artifact.
    #[must_use]
    pub const fn backend(&self) -> BridgeSccpDestinationProofBackendV1 {
        self.backend
    }
    /// Return the one curve-specific pairing operation needed to verify it.
    #[must_use]
    pub const fn crypto_work(&self) -> SccpDestinationProofCryptoWorkV1 {
        match &self.material {
            SccpParsedDestinationMaterialV1::Bn254 { .. } => {
                SccpDestinationProofCryptoWorkV1::Groth16Bn254Pairing
            }
            SccpParsedDestinationMaterialV1::TonBls12381 { .. } => {
                SccpDestinationProofCryptoWorkV1::Groth16Bls12381Pairing
            }
        }
    }
    /// Return the canonically decoded message bundle.
    #[must_use]
    pub const fn bundle(&self) -> &TairaSccpMessageProofV1 {
        &self.bundle
    }
    /// Return the structurally checked Taira finality proof.
    #[must_use]
    pub const fn finality(&self) -> &TairaBridgeFinalityProofV1 {
        &self.finality
    }

    /// Return whether the opaque parsed artifact carries this exact
    /// state-derived curve-specific request.
    #[must_use]
    pub fn matches_exact_request(&self, expected: &SccpDestinationProofRequestV1) -> bool {
        match (&self.material, expected) {
            (
                SccpParsedDestinationMaterialV1::Bn254 { artifact, .. },
                SccpDestinationProofRequestV1::Groth16Bn254(expected),
            ) => artifact.request == *expected,
            (
                SccpParsedDestinationMaterialV1::TonBls12381 { artifact },
                SccpDestinationProofRequestV1::Groth16Bls12381(expected),
            ) => artifact.request == *expected,
            _ => false,
        }
    }
}
#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize,
)]
/// Decoded value of one closed SCCP V1 wire codec.
pub enum SccpNormalizedCodecValueV1 {
    /// Printable ASCII text or an exact canonical I105 account literal.
    CanonicalText {
        /// Decoded text value.
        value: String,
    },
    /// Raw 20-byte EVM address.
    EvmAddress20 {
        /// Canonical address bytes.
        bytes: [u8; 20],
    },
    /// Raw 21-byte TRON address including the `0x41` prefix.
    TronAddress21 {
        /// Canonical address bytes.
        bytes: [u8; 21],
    },
    /// Canonical raw TON account.
    TonAccount36 {
        /// Signed TON workchain identifier decoded from the big-endian prefix.
        workchain: i32,
        /// Raw nonzero 256-bit account identifier.
        account: [u8; 32],
    },
}
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
/// Normalized transfer payload consumed by proof backends.
pub struct SccpTransferProjectionV1 {
    /// Payload schema version.
    pub version: u8,
    /// Transfer source protocol domain.
    pub source_domain: u32,
    /// Transfer destination protocol domain.
    pub dest_domain: u32,
    /// Sender-chosen replay-separating nonce.
    #[norito(with = "json_utils::canonical_u64_string")]
    pub nonce: u64,
    /// Nonzero immutable governed-route revision.
    pub route_revision: u32,
    /// Asset home protocol domain.
    pub asset_home_domain: u32,
    /// Decoded canonical asset identifier.
    pub asset_id: SccpNormalizedCodecValueV1,
    /// Positive amount in the route's smallest unit.
    #[norito(with = "json_utils::canonical_u128_string")]
    pub amount: u128,
    /// Decoded canonical sender identifier.
    pub sender: SccpNormalizedCodecValueV1,
    /// Decoded canonical recipient identifier.
    pub recipient: SccpNormalizedCodecValueV1,
    /// Decoded canonical route identifier.
    pub route_id: SccpNormalizedCodecValueV1,
}
#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize,
)]
/// Closed normalized payload union consumed by SCCP proof backends.
pub enum SccpPayloadProjectionV1 {
    /// Transfer projection.
    Transfer(SccpTransferProjectionV1),
}
macro_rules! impl_str_json_enum {
    ($ty:ty, $err:literal, { $($variant:path => $label:expr),+ $(,)? }) => {
        impl $ty {
            /// Return the stable wire label for this closed SCCP enum value.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $($variant => $label,)+
                }
            }
        }
        impl core::str::FromStr for $ty {
            type Err = &'static str;
            fn from_str(value: &str) -> Result<Self, Self::Err> {
                match value {
                    $($label => Ok($variant),)+
                    _ => Err($err),
                }
            }
        }
        impl norito::json::FastJsonWrite for $ty {
            fn write_json(&self, out: &mut String) {
                norito::json::write_json_string(self.as_str(), out);
            }
        }
        impl norito::json::JsonDeserialize for $ty {
            fn json_deserialize(
                parser: &mut norito::json::Parser<'_>,
            ) -> Result<Self, norito::json::Error> {
                let value = parser.parse_string()?;
                value.parse().map_err(|_| {
                    norito::json::Error::Message($err.into())
                })
            }
            fn json_from_value(
                value: &norito::json::Value,
            ) -> Result<Self, norito::json::Error> {
                let Some(value) = value.as_str() else {
                    return Err(norito::json::Error::Message(format!(
                        "{err}: expected string",
                        err = $err,
                    )));
                };
                value.parse().map_err(|_| {
                    norito::json::Error::Message($err.into())
                })
            }
        }
    };
}
fn json_external_tagged_variant<'a>(
    type_name: &'static str,
    value: &'a norito::json::Value,
) -> Result<(&'a str, &'a norito::json::Value), norito::json::Error> {
    let Some(object) = value.as_object() else {
        return Err(norito::json::Error::Message(format!(
            "{type_name} must be an externally tagged object"
        )));
    };
    if object.len() != 1 {
        return Err(norito::json::Error::Message(format!(
            "{type_name} must contain exactly one variant key"
        )));
    }
    let (tag, payload) = object.iter().next().expect("object length checked above");
    Ok((tag.as_str(), payload))
}
fn json_required_field<'a>(
    type_name: &'static str,
    value: &'a norito::json::Value,
    field: &'static str,
) -> Result<&'a norito::json::Value, norito::json::Error> {
    let Some(object) = value.as_object() else {
        return Err(norito::json::Error::Message(format!(
            "{type_name} variant payload must be an object"
        )));
    };
    object.get(field).ok_or_else(|| {
        norito::json::Error::Message(format!("missing `{field}` field in {type_name} payload"))
    })
}
fn json_require_exact_fields(
    type_name: &'static str,
    value: &norito::json::Value,
    expected_fields: &[&'static str],
) -> Result<(), norito::json::Error> {
    let Some(object) = value.as_object() else {
        return Err(norito::json::Error::Message(format!(
            "{type_name} variant payload must be an object"
        )));
    };
    for field in object.keys() {
        if !expected_fields.contains(&field.as_str()) {
            return Err(norito::json::Error::Message(format!(
                "unknown field `{field}` in {type_name} payload"
            )));
        }
    }
    for field in expected_fields {
        if !object.contains_key(*field) {
            return Err(norito::json::Error::Message(format!(
                "missing `{field}` field in {type_name} payload"
            )));
        }
    }
    Ok(())
}
fn json_fixed_hex_field<const N: usize>(
    type_name: &'static str,
    value: &norito::json::Value,
    field: &'static str,
) -> Result<[u8; N], norito::json::Error> {
    let field_value = json_required_field(type_name, value, field)?;
    let Some(raw) = field_value.as_str() else {
        return Err(norito::json::Error::Message(format!(
            "`{field}` field in {type_name} payload must be a hex string"
        )));
    };
    decode_canonical_0x_lower_hex_fixed::<N>(raw).ok_or_else(|| {
        norito::json::Error::Message(format!(
            "`{field}` field in {type_name} payload must be a canonical lowercase 0x-prefixed {N}-byte hex string"
        ))
    })
}
fn write_json_key(out: &mut String, key: &str) {
    norito::json::write_json_string(key, out);
    out.push(':');
}
fn write_prefixed_hex_json(out: &mut String, bytes: &[u8]) {
    norito::json::write_json_string(&encode_0x_lower_hex(bytes), out);
}
macro_rules! impl_external_tagged_tuple_json_enum {
    ($ty:ident, $err:literal, { $($variant:ident($payload:ty) => $label:literal),+ $(,)? }) => {
        impl norito::json::FastJsonWrite for $ty {
            fn write_json(&self, out: &mut String) {
                out.push('{');
                match self {
                    $(
                        Self::$variant(payload) => {
                            write_json_key(out, $label);
                            norito::json::JsonSerialize::json_serialize(payload, out);
                        }
                    ),+
                }
                out.push('}');
            }
        }
        impl norito::json::JsonDeserialize for $ty {
            fn json_deserialize(
                parser: &mut norito::json::Parser<'_>,
            ) -> Result<Self, norito::json::Error> {
                let value = <norito::json::Value as norito::json::JsonDeserialize>::json_deserialize(parser)?;
                Self::json_from_value(&value)
            }
            fn json_from_value(
                value: &norito::json::Value,
            ) -> Result<Self, norito::json::Error> {
                let (tag, payload) = json_external_tagged_variant(stringify!($ty), value)?;
                match tag {
                    $(
                        $label => Ok(Self::$variant(<$payload as norito::json::JsonDeserialize>::json_from_value(payload)?)),
                    )+
                    _ => Err(norito::json::Error::Message($err.into())),
                }
            }
        }
    };
}
impl_str_json_enum!(SccpHubMessageKind, "unsupported SCCP hub message kind", {
    SccpHubMessageKind::Transfer => "Transfer",
});
impl_external_tagged_tuple_json_enum!(SccpPayloadV1, "unsupported SCCP payload variant", {
    Transfer(TransferPayloadV1) => "Transfer",
});
impl norito::json::FastJsonWrite for SccpNormalizedCodecValueV1 {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        match self {
            Self::CanonicalText { value } => {
                write_json_key(out, "CanonicalText");
                out.push('{');
                write_json_key(out, "value");
                norito::json::JsonSerialize::json_serialize(value, out);
                out.push('}');
            }
            Self::EvmAddress20 { bytes } => {
                write_json_key(out, "EvmAddress20");
                out.push('{');
                write_json_key(out, "bytes");
                write_prefixed_hex_json(out, bytes);
                out.push('}');
            }
            Self::TronAddress21 { bytes } => {
                write_json_key(out, "TronAddress21");
                out.push('{');
                write_json_key(out, "bytes");
                write_prefixed_hex_json(out, bytes);
                out.push('}');
            }
            Self::TonAccount36 { workchain, account } => {
                write_json_key(out, "TonAccount36");
                out.push('{');
                write_json_key(out, "workchain");
                norito::json::JsonSerialize::json_serialize(workchain, out);
                out.push(',');
                write_json_key(out, "account");
                write_prefixed_hex_json(out, account);
                out.push('}');
            }
        }
        out.push('}');
    }
}
impl norito::json::JsonDeserialize for SccpNormalizedCodecValueV1 {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value =
            <norito::json::Value as norito::json::JsonDeserialize>::json_deserialize(parser)?;
        Self::json_from_value(&value)
    }
    fn json_from_value(value: &norito::json::Value) -> Result<Self, norito::json::Error> {
        let (tag, payload) = json_external_tagged_variant("SccpNormalizedCodecValueV1", value)?;
        match tag {
            "CanonicalText" => {
                json_require_exact_fields(
                    "SccpNormalizedCodecValueV1::CanonicalText",
                    payload,
                    &["value"],
                )?;
                Ok(Self::CanonicalText {
                    value: <String as norito::json::JsonDeserialize>::json_from_value(
                        json_required_field("SccpNormalizedCodecValueV1", payload, "value")?,
                    )?,
                })
            }
            "EvmAddress20" => {
                json_require_exact_fields(
                    "SccpNormalizedCodecValueV1::EvmAddress20",
                    payload,
                    &["bytes"],
                )?;
                Ok(Self::EvmAddress20 {
                    bytes: json_fixed_hex_field::<20>(
                        "SccpNormalizedCodecValueV1",
                        payload,
                        "bytes",
                    )?,
                })
            }
            "TronAddress21" => {
                json_require_exact_fields(
                    "SccpNormalizedCodecValueV1::TronAddress21",
                    payload,
                    &["bytes"],
                )?;
                Ok(Self::TronAddress21 {
                    bytes: json_fixed_hex_field::<21>(
                        "SccpNormalizedCodecValueV1",
                        payload,
                        "bytes",
                    )?,
                })
            }
            "TonAccount36" => {
                json_require_exact_fields(
                    "SccpNormalizedCodecValueV1::TonAccount36",
                    payload,
                    &["workchain", "account"],
                )?;
                let address = SccpTonAddressV1 {
                    workchain: <i32 as norito::json::JsonDeserialize>::json_from_value(
                        json_required_field("SccpNormalizedCodecValueV1", payload, "workchain")?,
                    )?,
                    account: json_fixed_hex_field::<32>(
                        "SccpNormalizedCodecValueV1",
                        payload,
                        "account",
                    )?,
                };
                canonical_sccp_ton_account36_bytes_v1(address).ok_or_else(|| {
                    norito::json::Error::Message(
                        "TON SCCP account must be a nonzero basechain raw address".into(),
                    )
                })?;
                Ok(Self::TonAccount36 {
                    workchain: address.workchain,
                    account: address.account,
                })
            }
            _ => Err(norito::json::Error::Message(
                "unsupported SCCP normalized codec value variant".into(),
            )),
        }
    }
}
impl_external_tagged_tuple_json_enum!(
    SccpPayloadProjectionV1,
    "unsupported SCCP payload projection variant",
    {
        Transfer(SccpTransferProjectionV1) => "Transfer",
    }
);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Decoded fixed-width BN254 Groth16 proof tuple accepted by SCCP contracts.
pub struct SccpEvmGroth16Bn254ProofV1 {
    /// Proof-envelope version. SCCP V1 requires `1`.
    pub version: u8,
    /// Exact message identifier copied into the envelope preflight header.
    pub message_id: H256,
    /// SORA source protocol domain copied into the envelope preflight header.
    pub source_domain: u32,
    /// SORA commitment root copied into the envelope preflight header.
    pub commitment_root: H256,
    /// Groth16 G1 proof point `A` as two BN254 base-field words.
    pub a: [H256; 2],
    /// Groth16 G2 proof point `B` as four BN254 base-field words.
    pub b: [H256; 4],
    /// Groth16 G1 proof point `C` as two BN254 base-field words.
    pub c: [H256; 2],
}
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
/// Immutable input archive handed to an external EVM/TVM Groth16 prover.
pub struct SccpGroth16Bn254ProofRequestV1 {
    /// Request schema version. SCCP V1 requires `1`.
    pub version: u8,
    /// Closed prover/verifier backend selected by governed deployment state.
    pub backend: BridgeSccpDestinationProofBackendV1,
    /// Exact SORA source network profile.
    pub source_network: SccpNetworkV1,
    /// Exact external destination network profile.
    pub target_network: SccpNetworkV1,
    /// Structured base inputs committed by the governed Groth16 statement.
    pub public_inputs: SccpMessagePublicInputsV1,
    /// Exact audited verification key pinned by the governed deployment.
    pub verifying_key: SccpGroth16Bn254VerifyingKeyV1,
    /// Solidity-compatible hash of [`Self::verifying_key`].
    #[norito(with = "json_utils::hex32")]
    pub verifier_key_hash: H256,
    /// Exact governed semantic circuit profile selected by route state.
    pub semantic_proof_profile: SccpSemanticProofProfileV1,
    /// Domain-separated hash of [`Self::semantic_proof_profile`].
    #[norito(with = "json_utils::hex32")]
    pub semantic_proof_profile_hash: H256,
    /// Exact governed Taira finality checkpoint exposed as public signal 10.
    pub sora_finality_anchor: SccpSoraFinalityAnchorV1,
    /// Domain-separated hash of [`Self::sora_finality_anchor`].
    #[norito(with = "json_utils::hex32")]
    pub sora_finality_anchor_hash: H256,
    /// Canonical encoded SORA message and finality bundle.
    #[norito(with = "json_utils::bytes_hex")]
    pub bundle_bytes: Vec<u8>,
    /// Canonical governed statement hash.
    #[norito(with = "json_utils::hex32")]
    pub statement_hash: H256,
    /// Exact typed governed destination deployment binding hash.
    #[norito(with = "json_utils::hex32")]
    pub destination_binding_hash: H256,
    /// Immutable historical governed route configuration recorded for the message.
    #[norito(with = "json_utils::hex32")]
    pub route_configuration_hash: H256,
    /// Hash of the complete canonical prover request.
    #[norito(with = "json_utils::hex32")]
    pub request_hash: H256,
}
/// EVM-specific name for the shared BN254 Groth16 prover request.
pub type SccpEvmGroth16Bn254ProofRequestV1 = SccpGroth16Bn254ProofRequestV1;
/// TRON-specific name for the shared BN254 Groth16 prover request.
pub type SccpTronGroth16Bn254ProofRequestV1 = SccpGroth16Bn254ProofRequestV1;
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
/// Minimal request-bound result returned by an external BN254 Groth16 prover.
pub struct SccpGroth16Bn254ProofResultV1 {
    /// Result schema version. SCCP V1 requires `1`.
    pub version: u8,
    /// Hash of the exact canonical request answered by this result.
    #[norito(with = "json_utils::hex32")]
    pub request_hash: H256,
    /// Canonical fixed-width Groth16 proof envelope.
    #[norito(with = "json_utils::bytes_hex")]
    pub proof_bytes: Vec<u8>,
    /// Hash of `request_hash || proof_bytes` under the backend result domain.
    #[norito(with = "json_utils::hex32")]
    pub result_hash: H256,
}
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
/// Self-contained canonical Groth16 proof artifact.
pub struct SccpGroth16Bn254ProofArtifactV1 {
    /// Artifact schema version. SCCP V1 requires `1`.
    pub version: u8,
    /// Exact state-derived request, including the bundle and governed key.
    pub request: SccpGroth16Bn254ProofRequestV1,
    /// Minimal proof result bound to [`Self::request`].
    pub result: SccpGroth16Bn254ProofResultV1,
}
/// EVM-specific name for the shared canonical Groth16 artifact.
pub type SccpEvmGroth16Bn254ProofArtifactV1 = SccpGroth16Bn254ProofArtifactV1;
/// TRON-specific name for the shared canonical Groth16 artifact.
pub type SccpTronGroth16Bn254ProofArtifactV1 = SccpGroth16Bn254ProofArtifactV1;
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
/// One canonical compressed non-identity BLS12-381 G1 point.
pub struct SccpBls12381G1CompressedV1 {
    /// IETF-compatible 48-byte compressed point encoding.
    #[norito(with = "json_utils::bytes_hex")]
    pub bytes: Vec<u8>,
}
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
/// One canonical compressed non-identity BLS12-381 G2 point.
pub struct SccpBls12381G2CompressedV1 {
    /// IETF-compatible 96-byte compressed point encoding.
    #[norito(with = "json_utils::bytes_hex")]
    pub bytes: Vec<u8>,
}
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
/// Canonical compressed BLS12-381 Groth16 proof tuple.
pub struct SccpGroth16Bls12381ProofV1 {
    /// Proof point A in G1.
    pub a: SccpBls12381G1CompressedV1,
    /// Proof point B in G2.
    pub b: SccpBls12381G2CompressedV1,
    /// Proof point C in G1.
    pub c: SccpBls12381G1CompressedV1,
}
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
/// Exact eleven BLS12-381 scalar-field signals consumed by the TON verifier.
pub struct SccpGroth16Bls12381PublicSignalsV1 {
    /// Domain-separated message-id signal.
    #[norito(with = "json_utils::hex32")]
    pub message_id: H256,
    /// Domain-separated payload-hash signal.
    #[norito(with = "json_utils::hex32")]
    pub payload_hash: H256,
    /// Domain-separated target-domain signal.
    #[norito(with = "json_utils::hex32")]
    pub target_domain: H256,
    /// Domain-separated commitment-root signal.
    #[norito(with = "json_utils::hex32")]
    pub commitment_root: H256,
    /// Domain-separated finality-height signal.
    #[norito(with = "json_utils::hex32")]
    pub finality_height: H256,
    /// Domain-separated finality-block-hash signal.
    #[norito(with = "json_utils::hex32")]
    pub finality_block_hash: H256,
    /// Domain-separated source-domain signal.
    #[norito(with = "json_utils::hex32")]
    pub source_domain: H256,
    /// Domain-separated statement-hash signal.
    #[norito(with = "json_utils::hex32")]
    pub statement_hash: H256,
    /// Domain-separated destination-binding signal.
    #[norito(with = "json_utils::hex32")]
    pub destination_binding_hash: H256,
    /// Domain-separated route-configuration signal.
    #[norito(with = "json_utils::hex32")]
    pub route_configuration_hash: H256,
    /// Domain-separated governed Taira finality-anchor signal.
    #[norito(with = "json_utils::hex32")]
    pub sora_finality_anchor_hash: H256,
}
impl SccpGroth16Bls12381PublicSignalsV1 {
    /// Return the canonical verifier order.
    #[must_use]
    pub const fn words(self) -> [H256; 11] {
        [
            self.message_id,
            self.payload_hash,
            self.target_domain,
            self.commitment_root,
            self.finality_height,
            self.finality_block_hash,
            self.source_domain,
            self.statement_hash,
            self.destination_binding_hash,
            self.route_configuration_hash,
            self.sora_finality_anchor_hash,
        ]
    }
}
impl From<[H256; 11]> for SccpGroth16Bls12381PublicSignalsV1 {
    fn from(words: [H256; 11]) -> Self {
        Self {
            message_id: words[0],
            payload_hash: words[1],
            target_domain: words[2],
            commitment_root: words[3],
            finality_height: words[4],
            finality_block_hash: words[5],
            source_domain: words[6],
            statement_hash: words[7],
            destination_binding_hash: words[8],
            route_configuration_hash: words[9],
            sora_finality_anchor_hash: words[10],
        }
    }
}
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
/// Immutable state-derived input archive for the TON BLS12-381 prover.
pub struct SccpTonGroth16Bls12381ProofRequestV1 {
    /// Request schema version. SCCP V1 requires `1`.
    pub version: u8,
    /// Closed backend. This must be `TonGroth16Bls12381`.
    pub backend: BridgeSccpDestinationProofBackendV1,
    /// Exact SORA Taira source profile.
    pub source_network: SccpNetworkV1,
    /// Exact TON mainnet destination profile.
    pub target_network: SccpNetworkV1,
    /// Structured base message inputs.
    pub public_inputs: SccpMessagePublicInputsV1,
    /// Exact curve-specific signal words.
    pub public_signals: SccpGroth16Bls12381PublicSignalsV1,
    /// Complete subgroup-checked BLS12-381 verification key.
    pub verifying_key: SccpGroth16Bls12381VerifyingKeyV1,
    /// SHA-256 commitment to the canonical verification key.
    #[norito(with = "json_utils::hex32")]
    pub verifier_key_hash: H256,
    /// Exact governed BLS12-381 circuit commitment.
    #[norito(with = "json_utils::hex32")]
    pub verifier_circuit_hash: H256,
    /// Exact proof-format and public-input mapping commitment.
    #[norito(with = "json_utils::hex32")]
    pub proof_profile_commitment: H256,
    /// Exact curve-specific semantic profile.
    pub semantic_proof_profile: SccpSemanticProofProfileV1,
    /// Commitment to the semantic profile.
    #[norito(with = "json_utils::hex32")]
    pub semantic_proof_profile_hash: H256,
    /// Exact governed Taira finality anchor.
    pub sora_finality_anchor: SccpSoraFinalityAnchorV1,
    /// Commitment to the finality anchor.
    #[norito(with = "json_utils::hex32")]
    pub sora_finality_anchor_hash: H256,
    /// Canonical encoded SORA message and finality bundle.
    #[norito(with = "json_utils::bytes_hex")]
    pub bundle_bytes: Vec<u8>,
    /// Canonical governed statement hash.
    #[norito(with = "json_utils::hex32")]
    pub statement_hash: H256,
    /// Exact governed destination deployment binding.
    #[norito(with = "json_utils::hex32")]
    pub destination_binding_hash: H256,
    /// Exact immutable route-configuration commitment.
    #[norito(with = "json_utils::hex32")]
    pub route_configuration_hash: H256,
    /// Hash of the complete canonical request.
    #[norito(with = "json_utils::hex32")]
    pub request_hash: H256,
}
#[derive(Clone, Debug, PartialEq, Eq)]
/// Closed curve-specific SCCP destination proving request.
///
/// Torii returns the concrete inner request encoding selected by the governed
/// route. This Rust enum classifies that closed pair without adding a second
/// wrapper to the wire format.
pub enum SccpDestinationProofRequestV1 {
    /// EVM or TRON request using the BN254 verifier.
    Groth16Bn254(SccpGroth16Bn254ProofRequestV1),
    /// TON request using the BLS12-381 verifier.
    Groth16Bls12381(SccpTonGroth16Bls12381ProofRequestV1),
}
impl SccpDestinationProofRequestV1 {
    /// Return the exact closed backend selected by this request.
    #[must_use]
    pub const fn backend(&self) -> BridgeSccpDestinationProofBackendV1 {
        match self {
            Self::Groth16Bn254(request) => request.backend,
            Self::Groth16Bls12381(request) => request.backend,
        }
    }

    /// Return the exact source network.
    #[must_use]
    pub const fn source_network(&self) -> SccpNetworkV1 {
        match self {
            Self::Groth16Bn254(request) => request.source_network,
            Self::Groth16Bls12381(request) => request.source_network,
        }
    }

    /// Return the exact destination network.
    #[must_use]
    pub const fn target_network(&self) -> SccpNetworkV1 {
        match self {
            Self::Groth16Bn254(request) => request.target_network,
            Self::Groth16Bls12381(request) => request.target_network,
        }
    }

    /// Return the shared message and finality inputs.
    #[must_use]
    pub const fn public_inputs(&self) -> &SccpMessagePublicInputsV1 {
        match self {
            Self::Groth16Bn254(request) => &request.public_inputs,
            Self::Groth16Bls12381(request) => &request.public_inputs,
        }
    }

    /// Return the canonical embedded Taira bundle bytes.
    #[must_use]
    pub fn bundle_bytes(&self) -> &[u8] {
        match self {
            Self::Groth16Bn254(request) => &request.bundle_bytes,
            Self::Groth16Bls12381(request) => &request.bundle_bytes,
        }
    }

    /// Return the governed destination binding.
    #[must_use]
    pub const fn destination_binding_hash(&self) -> H256 {
        match self {
            Self::Groth16Bn254(request) => request.destination_binding_hash,
            Self::Groth16Bls12381(request) => request.destination_binding_hash,
        }
    }

    /// Return the immutable route-configuration commitment.
    #[must_use]
    pub const fn route_configuration_hash(&self) -> H256 {
        match self {
            Self::Groth16Bn254(request) => request.route_configuration_hash,
            Self::Groth16Bls12381(request) => request.route_configuration_hash,
        }
    }
}
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
/// Request-bound result returned by a TON BLS12-381 Groth16 prover.
pub struct SccpTonGroth16Bls12381ProofResultV1 {
    /// Result schema version. SCCP V1 requires `1`.
    pub version: u8,
    /// Hash of the exact request answered by this result.
    #[norito(with = "json_utils::hex32")]
    pub request_hash: H256,
    /// Canonical compressed proof tuple.
    pub proof: SccpGroth16Bls12381ProofV1,
    /// SHA-256 commitment to this exact result.
    #[norito(with = "json_utils::hex32")]
    pub result_hash: H256,
}
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
/// Self-contained TON BLS12-381 destination proof artifact.
pub struct SccpTonGroth16Bls12381ProofArtifactV1 {
    /// Artifact schema version. SCCP V1 requires `1`.
    pub version: u8,
    /// Exact state-derived proving request.
    pub request: SccpTonGroth16Bls12381ProofRequestV1,
    /// Pairing-valid result bound to the request.
    pub result: SccpTonGroth16Bls12381ProofResultV1,
}
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
/// Fully verified TON settlement material and exact internal-message body BOC.
pub struct SccpVerifiedTonDestinationCallV1 {
    /// Call schema version. SCCP V1 requires `1`.
    pub version: u8,
    /// TON BLS12-381 backend tag.
    pub backend: BridgeSccpDestinationProofBackendV1,
    /// Exact governed TON destination profile.
    pub network: SccpNetworkV1,
    /// Exact governed route contract.
    pub route_address: SccpTonAddressV1,
    /// Caller-selected TON query id; replay identity remains the message id.
    pub query_id: u64,
    /// Nonzero governed route revision.
    pub route_revision: u32,
    /// Canonical TON recipient authenticated by the SCCP payload.
    pub recipient: SccpTonAddressV1,
    /// Positive Jetton base-unit amount.
    #[norito(with = "json_utils::canonical_u128_string")]
    pub amount: u128,
    /// Exact destination binding.
    #[norito(with = "json_utils::hex32")]
    pub destination_binding_hash: H256,
    /// Exact route-configuration commitment.
    #[norito(with = "json_utils::hex32")]
    pub route_configuration_hash: H256,
    /// Exact public signals verified by the pairing equation.
    pub public_signals: SccpGroth16Bls12381PublicSignalsV1,
    /// Exact statement hash.
    #[norito(with = "json_utils::hex32")]
    pub statement_hash: H256,
    /// Exact request hash.
    #[norito(with = "json_utils::hex32")]
    pub request_hash: H256,
    /// Canonical compressed proof bytes placed in the TVM body.
    #[norito(with = "json_utils::bytes_hex")]
    pub proof_bytes: Vec<u8>,
    /// Exact canonical SCCP payload bytes.
    #[norito(with = "json_utils::bytes_hex")]
    pub canonical_payload_bytes: Vec<u8>,
    /// Canonical BOC containing one `SccpFinalizeFromTaira` message body.
    #[norito(with = "json_utils::bytes_hex")]
    pub internal_message_body_boc: Vec<u8>,
    /// Original message/finality bundle retained for audit and settlement.
    pub bundle: TairaSccpMessageProofV1,
}
/// Return whether `domain_id` is a recognized SCCP V1 protocol domain.
pub fn is_supported_domain(domain_id: u32) -> bool {
    matches!(
        domain_id,
        SCCP_DOMAIN_SORA | SCCP_DOMAIN_ETH | SCCP_DOMAIN_BSC | SCCP_DOMAIN_TON | SCCP_DOMAIN_TRON
    )
}
/// Return whether a remote SCCP domain is in the current supported production launch scope.
pub fn sccp_domain_in_supported_launch_scope_v1(domain_id: u32) -> bool {
    SCCP_SUPPORTED_LAUNCH_REMOTE_DOMAINS_V1.contains(&domain_id)
}
/// Return whether a remote protocol domain can originate native SCCP messages in V1.
pub const fn sccp_domain_supports_native_inbound_source_v1(domain_id: u32) -> bool {
    matches!(
        domain_id,
        SCCP_DOMAIN_ETH | SCCP_DOMAIN_BSC | SCCP_DOMAIN_TON | SCCP_DOMAIN_TRON
    )
}
/// Return whether a remote domain has a checked-in value-moving outbound route in V1.
pub const fn sccp_domain_has_value_moving_outbound_route_v1(domain_id: u32) -> bool {
    matches!(
        domain_id,
        SCCP_DOMAIN_ETH | SCCP_DOMAIN_BSC | SCCP_DOMAIN_TON | SCCP_DOMAIN_TRON
    )
}
/// Return whether `codec_id` is one of the closed SCCP V1 wire codecs.
pub fn is_supported_codec(codec_id: u8) -> bool {
    matches!(
        codec_id,
        SCCP_CODEC_CANONICAL_TEXT
            | SCCP_CODEC_EVM_ADDRESS20
            | SCCP_CODEC_TRON_ADDRESS21
            | SCCP_CODEC_TON_ACCOUNT36
    )
}
/// Return the stable machine-readable name of one SCCP wire codec.
pub fn sccp_codec_key(codec_id: u8) -> Option<&'static str> {
    match codec_id {
        SCCP_CODEC_CANONICAL_TEXT => Some("canonical_text"),
        SCCP_CODEC_EVM_ADDRESS20 => Some("evm_address20"),
        SCCP_CODEC_TRON_ADDRESS21 => Some("tron_address21"),
        SCCP_CODEC_TON_ACCOUNT36 => Some("ton_account36"),
        _ => None,
    }
}
/// Return a concise description of one SCCP wire codec.
pub fn sccp_codec_description(codec_id: u8) -> Option<&'static str> {
    match codec_id {
        SCCP_CODEC_CANONICAL_TEXT => Some(
            "Non-empty printable ASCII bytes, or an exact canonical I105 literal, for SORA accounts and route-local names.",
        ),
        SCCP_CODEC_EVM_ADDRESS20 => Some("Raw nonzero 20-byte EVM account addresses."),
        SCCP_CODEC_TRON_ADDRESS21 => {
            Some("Raw nonzero TRON account addresses including the 0x41 prefix.")
        }
        SCCP_CODEC_TON_ACCOUNT36 => Some(
            "Raw TON account: signed big-endian i32 workchain followed by a nonzero 32-byte account id; V1 value-moving routes require basechain workchain 0.",
        ),
        _ => None,
    }
}
/// Return the stable chain-family key for one SCCP protocol domain.
pub fn sccp_chain_key_for_domain(domain: u32) -> Option<&'static str> {
    match domain {
        SCCP_DOMAIN_SORA => Some("sora"),
        SCCP_DOMAIN_ETH => Some("eth"),
        SCCP_DOMAIN_BSC => Some("bsc"),
        SCCP_DOMAIN_TON => Some("ton"),
        SCCP_DOMAIN_TRON => Some("tron"),
        _ => None,
    }
}
/// Return the account-identifier codec required by one external domain.
pub fn sccp_counterparty_account_codec(domain: u32) -> Option<u8> {
    match domain {
        SCCP_DOMAIN_SORA => Some(SCCP_CODEC_CANONICAL_TEXT),
        SCCP_DOMAIN_ETH | SCCP_DOMAIN_BSC => Some(SCCP_CODEC_EVM_ADDRESS20),
        SCCP_DOMAIN_TON => Some(SCCP_CODEC_TON_ACCOUNT36),
        SCCP_DOMAIN_TRON => Some(SCCP_CODEC_TRON_ADDRESS21),
        _ => None,
    }
}
/// Return the non-SORA endpoint of a valid SORA/external domain pair.
pub fn sccp_counterparty_domain(primary: u32, secondary: u32) -> Option<u32> {
    if primary != SCCP_DOMAIN_SORA {
        return Some(primary);
    }
    if secondary != SCCP_DOMAIN_SORA {
        return Some(secondary);
    }
    None
}
/// Return the external destination for one SORA-origin outbound message.
///
/// External-origin messages deliberately return `None`; inbound admission uses the closed
/// protocol-native proof API and never constructs an outbound counterparty artifact.
pub fn sccp_counterparty_domain_for_message_payload(payload: &SccpPayloadV1) -> Option<u32> {
    let source_domain = sccp_message_source_domain(payload);
    let target_domain = sccp_message_target_domain(payload);
    (source_domain == SCCP_DOMAIN_SORA
        && target_domain != SCCP_DOMAIN_SORA
        && is_supported_domain(target_domain))
    .then_some(target_domain)
}
/// Return the stable application-payload label for `payload`.
pub fn sccp_message_payload_kind_key(payload: &SccpPayloadV1) -> &'static str {
    match payload {
        SccpPayloadV1::Transfer(_) => "transfer",
    }
}
/// Strictly decode every codec-tagged field into a normalized proof projection.
pub fn sccp_payload_projection(payload: &SccpPayloadV1) -> Option<SccpPayloadProjectionV1> {
    match payload {
        SccpPayloadV1::Transfer(payload) => Some(SccpPayloadProjectionV1::Transfer(
            SccpTransferProjectionV1 {
                version: payload.version,
                source_domain: payload.source_domain,
                dest_domain: payload.dest_domain,
                nonce: payload.nonce,
                route_revision: payload.route_revision,
                asset_home_domain: payload.asset_home_domain,
                asset_id: decode_sccp_normalized_codec_value(
                    payload.asset_id_codec,
                    &payload.asset_id,
                )?,
                amount: payload.amount,
                sender: decode_sccp_normalized_codec_value(payload.sender_codec, &payload.sender)?,
                recipient: decode_sccp_normalized_codec_value(
                    payload.recipient_codec,
                    &payload.recipient,
                )?,
                route_id: decode_sccp_normalized_codec_value(
                    payload.route_id_codec,
                    &payload.route_id,
                )?,
            },
        )),
    }
}
/// Encode the six base public inputs in their fixed, platform-independent V1 layout.
pub fn canonical_sccp_message_public_inputs_bytes(
    public_inputs: &SccpMessagePublicInputsV1,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + 32 + 32 + 4 + 32 + 8 + 32);
    push_u8(&mut out, public_inputs.version);
    out.extend_from_slice(&public_inputs.message_id);
    out.extend_from_slice(&public_inputs.payload_hash);
    push_u32(&mut out, public_inputs.target_domain);
    out.extend_from_slice(&public_inputs.commitment_root);
    push_u64(&mut out, public_inputs.finality_height);
    out.extend_from_slice(&public_inputs.finality_block_hash);
    out
}
/// Encode a bounded SCCP Merkle path in its canonical V1 byte layout.
pub fn canonical_sccp_merkle_proof_bytes_checked(proof: &SccpMerkleProofV1) -> Option<Vec<u8>> {
    if proof.steps.len() > SCCP_TAIRA_MAX_MERKLE_PROOF_STEPS_V1 {
        return None;
    }
    let mut out = Vec::new();
    push_u32_len_checked(&mut out, proof.steps.len())?;
    for step in &proof.steps {
        out.extend_from_slice(&step.sibling_hash);
        push_u8(&mut out, u8::from(step.sibling_is_left));
    }
    Some(out)
}
fn canonical_taira_sccp_message_bundle_bytes_len_checked(
    bundle: &TairaSccpMessageProofV1,
) -> Option<Vec<u8>> {
    let commitment = canonical_commitment_bytes(&bundle.commitment);
    let merkle_proof = canonical_sccp_merkle_proof_bytes_checked(&bundle.merkle_proof)?;
    let payload = canonical_sccp_payload_bytes(&bundle.payload).ok()?;
    let mut out = Vec::new();
    push_u8(&mut out, bundle.version);
    out.extend_from_slice(&bundle.commitment_root);
    push_vec_checked(&mut out, &commitment)?;
    push_vec_checked(&mut out, &merkle_proof)?;
    push_vec_checked(&mut out, &payload)?;
    push_vec_checked(&mut out, &bundle.finality_proof)?;
    Some(out)
}
/// Validate and encode one canonical, length-bounded Taira SCCP message bundle.
pub fn canonical_taira_sccp_message_bundle_bytes_checked(
    bundle: &TairaSccpMessageProofV1,
) -> Option<Vec<u8>> {
    if !verify_message_bundle_structure(bundle) {
        return None;
    }
    canonical_taira_sccp_message_bundle_bytes_len_checked(bundle)
}
fn h256_be_ge(left: &H256, right: &H256) -> bool {
    left.iter()
        .zip(right.iter())
        .find_map(|(left, right)| {
            if left == right {
                None
            } else {
                Some(left > right)
            }
        })
        .unwrap_or(true)
}
fn h256_be_sub_assign(left: &mut H256, right: &H256) {
    let mut borrow = 0u16;
    for idx in (0..32).rev() {
        let minuend = u16::from(left[idx]);
        let subtrahend = u16::from(right[idx]) + borrow;
        if minuend >= subtrahend {
            left[idx] = u8::try_from(minuend - subtrahend).expect("byte difference fits");
            borrow = 0;
        } else {
            left[idx] = u8::try_from((minuend + 256) - subtrahend).expect("byte difference fits");
            borrow = 1;
        }
    }
}
fn h256_mod_bn254_scalar_field(mut value: H256) -> H256 {
    while h256_be_ge(&value, &SCCP_GROTH16_BN254_SCALAR_FIELD_MODULUS_BE) {
        h256_be_sub_assign(&mut value, &SCCP_GROTH16_BN254_SCALAR_FIELD_MODULUS_BE);
    }
    value
}
fn sccp_groth16_bn254_signal_word(label: &[u8], value: H256) -> H256 {
    let label_hash = keccak256_bytes(label);
    let mut payload = Vec::with_capacity(64);
    payload.extend_from_slice(&label_hash);
    payload.extend_from_slice(&value);
    h256_mod_bn254_scalar_field(keccak256_bytes(&payload))
}
/// Derive the eleven BN254 field public signals consumed by SCCP Groth16 verifiers.
///
/// The output order matches `SccpGroth16Bn254MessageVerifier`: message id, payload hash,
/// target-domain word, commitment root, finality-height word, finality block hash, source-domain
/// word, statement hash, and destination binding hash, immutable route-configuration hash, and
/// governed SORA finality-anchor hash. Each word is `keccak256(abi.encode(keccak256(label), value))
/// mod Fr` encoded as a big-endian 32-byte BN254 scalar.
pub fn sccp_groth16_bn254_public_signal_words(
    public_inputs: &SccpMessagePublicInputsV1,
    source_domain: u32,
    statement_hash: H256,
    destination_binding_hash: H256,
    route_configuration_hash: H256,
    sora_finality_anchor_hash: H256,
) -> [H256; 11] {
    let public_input_words = sccp_evm_public_input_words(public_inputs);
    [
        sccp_groth16_bn254_signal_word(
            SCCP_GROTH16_BN254_SIGNAL_MESSAGE_ID_V1,
            public_input_words[0],
        ),
        sccp_groth16_bn254_signal_word(
            SCCP_GROTH16_BN254_SIGNAL_PAYLOAD_HASH_V1,
            public_input_words[1],
        ),
        sccp_groth16_bn254_signal_word(
            SCCP_GROTH16_BN254_SIGNAL_TARGET_DOMAIN_V1,
            public_input_words[2],
        ),
        sccp_groth16_bn254_signal_word(
            SCCP_GROTH16_BN254_SIGNAL_COMMITMENT_ROOT_V1,
            public_input_words[3],
        ),
        sccp_groth16_bn254_signal_word(
            SCCP_GROTH16_BN254_SIGNAL_FINALITY_HEIGHT_V1,
            public_input_words[4],
        ),
        sccp_groth16_bn254_signal_word(
            SCCP_GROTH16_BN254_SIGNAL_FINALITY_BLOCK_HASH_V1,
            public_input_words[5],
        ),
        sccp_groth16_bn254_signal_word(
            SCCP_GROTH16_BN254_SIGNAL_SOURCE_DOMAIN_V1,
            abi_word_u32(source_domain),
        ),
        sccp_groth16_bn254_signal_word(SCCP_GROTH16_BN254_SIGNAL_STATEMENT_HASH_V1, statement_hash),
        sccp_groth16_bn254_signal_word(
            SCCP_GROTH16_BN254_SIGNAL_DESTINATION_BINDING_HASH_V1,
            destination_binding_hash,
        ),
        sccp_groth16_bn254_signal_word(
            SCCP_GROTH16_BN254_SIGNAL_ROUTE_CONFIGURATION_HASH_V1,
            route_configuration_hash,
        ),
        sccp_groth16_bn254_signal_word(
            SCCP_GROTH16_BN254_SIGNAL_SORA_FINALITY_ANCHOR_HASH_V1,
            sora_finality_anchor_hash,
        ),
    ]
}
fn h256_mod_bls12381_scalar_field(mut value: H256) -> H256 {
    while h256_be_ge(&value, &SCCP_GROTH16_BLS12381_SCALAR_FIELD_MODULUS_BE) {
        h256_be_sub_assign(&mut value, &SCCP_GROTH16_BLS12381_SCALAR_FIELD_MODULUS_BE);
    }
    value
}
fn sccp_groth16_bls12381_signal_word(label: &[u8], value: H256) -> H256 {
    let label_hash = sha256_bytes(label);
    let mut payload = Vec::with_capacity(64);
    payload.extend_from_slice(&label_hash);
    payload.extend_from_slice(&value);
    h256_mod_bls12381_scalar_field(sha256_bytes(&payload))
}
/// Derive the exact eleven BLS12-381 scalar-field signals consumed by the TON
/// destination verifier.
///
/// Every word is `SHA-256(SHA-256(label) || value) mod r`, encoded as a
/// big-endian 32-byte scalar. The curve-specific labels and reduction prevent
/// a BN254 request or proof from being reinterpreted as TON material.
#[must_use]
pub fn sccp_groth16_bls12381_public_signal_words_v1(
    public_inputs: &SccpMessagePublicInputsV1,
    source_domain: u32,
    statement_hash: H256,
    destination_binding_hash: H256,
    route_configuration_hash: H256,
    sora_finality_anchor_hash: H256,
) -> [H256; 11] {
    let public_input_words = sccp_evm_public_input_words(public_inputs);
    [
        sccp_groth16_bls12381_signal_word(
            SCCP_TON_GROTH16_SIGNAL_MESSAGE_ID_V1,
            public_input_words[0],
        ),
        sccp_groth16_bls12381_signal_word(
            SCCP_TON_GROTH16_SIGNAL_PAYLOAD_HASH_V1,
            public_input_words[1],
        ),
        sccp_groth16_bls12381_signal_word(
            SCCP_TON_GROTH16_SIGNAL_TARGET_DOMAIN_V1,
            public_input_words[2],
        ),
        sccp_groth16_bls12381_signal_word(
            SCCP_TON_GROTH16_SIGNAL_COMMITMENT_ROOT_V1,
            public_input_words[3],
        ),
        sccp_groth16_bls12381_signal_word(
            SCCP_TON_GROTH16_SIGNAL_FINALITY_HEIGHT_V1,
            public_input_words[4],
        ),
        sccp_groth16_bls12381_signal_word(
            SCCP_TON_GROTH16_SIGNAL_FINALITY_BLOCK_HASH_V1,
            public_input_words[5],
        ),
        sccp_groth16_bls12381_signal_word(
            SCCP_TON_GROTH16_SIGNAL_SOURCE_DOMAIN_V1,
            abi_word_u32(source_domain),
        ),
        sccp_groth16_bls12381_signal_word(
            SCCP_TON_GROTH16_SIGNAL_STATEMENT_HASH_V1,
            statement_hash,
        ),
        sccp_groth16_bls12381_signal_word(
            SCCP_TON_GROTH16_SIGNAL_DESTINATION_BINDING_HASH_V1,
            destination_binding_hash,
        ),
        sccp_groth16_bls12381_signal_word(
            SCCP_TON_GROTH16_SIGNAL_ROUTE_CONFIGURATION_HASH_V1,
            route_configuration_hash,
        ),
        sccp_groth16_bls12381_signal_word(
            SCCP_TON_GROTH16_SIGNAL_SORA_FINALITY_ANCHOR_HASH_V1,
            sora_finality_anchor_hash,
        ),
    ]
}
/// Derive the immutable TON proof-format commitment.
///
/// The commitment binds the exact compressed point sizes, eleven-signal
/// schema, scalar field, signal hash construction, and three-point proof
/// order. Deployments carrying any other value are rejected by the typed TON
/// request builder.
#[must_use]
pub fn sccp_ton_groth16_bls12381_proof_profile_commitment_v1() -> H256 {
    structural_sccp_ton_groth16_bls12381_proof_profile_commitment_v1()
}
fn read_be_u32(bytes: &[u8]) -> Option<u32> {
    (bytes.len() == 4).then(|| {
        let mut raw = [0u8; 4];
        raw.copy_from_slice(bytes);
        u32::from_be_bytes(raw)
    })
}
fn abi_padded_bytes(value: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&abi_word_u64(value.len() as u64));
    out.extend_from_slice(value);
    let padding = (32 - (value.len() % 32)) % 32;
    if padding != 0 {
        out.resize(out.len() + padding, 0);
    }
    out
}
fn sccp_evm_public_input_words(public_inputs: &SccpMessagePublicInputsV1) -> [H256; 6] {
    [
        public_inputs.message_id,
        public_inputs.payload_hash,
        abi_word_u32(public_inputs.target_domain),
        public_inputs.commitment_root,
        abi_word_u64(public_inputs.finality_height),
        public_inputs.finality_block_hash,
    ]
}
fn encode_sccp_finalize_from_taira_calldata_v1(
    proof_bytes: &[u8],
    public_inputs: &SccpMessagePublicInputsV1,
    statement_hash: H256,
    canonical_payload_bytes: &[u8],
) -> Option<Vec<u8>> {
    if proof_bytes.len() != 32 * 12
        || canonical_payload_bytes.is_empty()
        || !h256_is_nonzero(&statement_hash)
    {
        return None;
    }
    let head_len = 9usize.checked_mul(32)?;
    let proof_tail = abi_padded_bytes(proof_bytes);
    let payload_offset = head_len.checked_add(proof_tail.len())?;
    let payload_tail = abi_padded_bytes(canonical_payload_bytes);
    let capacity = 4usize
        .checked_add(head_len)?
        .checked_add(proof_tail.len())?
        .checked_add(payload_tail.len())?;
    let mut out = Vec::with_capacity(capacity);
    out.extend_from_slice(&SCCP_FINALIZE_FROM_TAIRA_SELECTOR_V1);
    out.extend_from_slice(&abi_word_u64(u64::try_from(head_len).ok()?));
    for word in sccp_evm_public_input_words(public_inputs) {
        out.extend_from_slice(&word);
    }
    out.extend_from_slice(&statement_hash);
    out.extend_from_slice(&abi_word_u64(u64::try_from(payload_offset).ok()?));
    out.extend_from_slice(&proof_tail);
    out.extend_from_slice(&payload_tail);
    Some(out)
}
fn abi_read_u32_word(word: &[u8]) -> Option<u32> {
    if word.len() != 32 || word[..28].iter().any(|byte| *byte != 0) {
        return None;
    }
    read_be_u32(&word[28..32])
}
fn abi_read_u8_word(word: &[u8]) -> Option<u8> {
    u8::try_from(abi_read_u32_word(word)?).ok()
}
fn abi_word_is_bn254_base_field_element(word: &H256) -> bool {
    word < &BN254_BASE_FIELD_MODULUS_BE
}
fn read_be_u64_exact(bytes: &[u8]) -> Option<u64> {
    (bytes.len() == 8).then(|| {
        let mut raw = [0u8; 8];
        raw.copy_from_slice(bytes);
        u64::from_be_bytes(raw)
    })
}
fn bn254_fq_from_abi_word(word: &H256) -> Option<Fq> {
    if !abi_word_is_bn254_base_field_element(word) {
        return None;
    }
    Some(Fq::from_raw([
        read_be_u64_exact(&word[24..32])?,
        read_be_u64_exact(&word[16..24])?,
        read_be_u64_exact(&word[8..16])?,
        read_be_u64_exact(&word[0..8])?,
    ]))
}
fn bn254_fr_from_abi_word(word: &H256) -> Option<Fr> {
    if word >= &SCCP_GROTH16_BN254_SCALAR_FIELD_MODULUS_BE {
        return None;
    }
    Some(Fr::from_raw([
        read_be_u64_exact(&word[24..32])?,
        read_be_u64_exact(&word[16..24])?,
        read_be_u64_exact(&word[8..16])?,
        read_be_u64_exact(&word[0..8])?,
    ]))
}
fn bn254_g1_affine(point: &SccpBn254G1PointV1) -> Option<G1Affine> {
    if !h256_is_nonzero(&point.x) && !h256_is_nonzero(&point.y) {
        return None;
    }
    let x = bn254_fq_from_abi_word(&point.x)?;
    let y = bn254_fq_from_abi_word(&point.y)?;
    let affine = Option::<G1Affine>::from(G1Affine::from_xy(x, y))?;
    (!bool::from(affine.is_identity()) && bool::from(affine.to_curve().is_torsion_free()))
        .then_some(affine)
}
fn bn254_g2_affine(point: &SccpBn254G2PointV1) -> Option<G2Affine> {
    if !h256_is_nonzero(&point.x_c0)
        && !h256_is_nonzero(&point.x_c1)
        && !h256_is_nonzero(&point.y_c0)
        && !h256_is_nonzero(&point.y_c1)
    {
        return None;
    }
    let affine = Option::<G2Affine>::from(G2Affine::from_xy(
        Fq2::new(
            bn254_fq_from_abi_word(&point.x_c0)?,
            bn254_fq_from_abi_word(&point.x_c1)?,
        ),
        Fq2::new(
            bn254_fq_from_abi_word(&point.y_c0)?,
            bn254_fq_from_abi_word(&point.y_c1)?,
        ),
    ))?;
    (!bool::from(affine.is_identity()) && bool::from(affine.to_curve().is_torsion_free()))
        .then_some(affine)
}
fn sccp_g1_point_from_words(point: &[H256; 2]) -> SccpBn254G1PointV1 {
    SccpBn254G1PointV1 {
        x: point[0],
        y: point[1],
    }
}
fn sccp_g2_point_from_words(point: &[H256; 4]) -> SccpBn254G2PointV1 {
    SccpBn254G2PointV1 {
        x_c0: point[0],
        x_c1: point[1],
        y_c0: point[2],
        y_c1: point[3],
    }
}
fn abi_g1_point_is_structurally_valid(point: &[H256; 2]) -> bool {
    bn254_g1_affine(&sccp_g1_point_from_words(point)).is_some()
}
fn abi_g2_point_is_structurally_valid(point: &[H256; 4]) -> bool {
    bn254_g2_affine(&sccp_g2_point_from_words(point)).is_some()
}
/// Return whether a closed SCCP Groth16 key contains only canonical subgroup points.
pub fn sccp_groth16_bn254_verifying_key_is_well_formed_v1(
    verifying_key: &SccpGroth16Bn254VerifyingKeyV1,
) -> bool {
    verifying_key.version == 1
        && bn254_g1_affine(&verifying_key.alpha1).is_some()
        && bn254_g2_affine(&verifying_key.beta2).is_some()
        && bn254_g2_affine(&verifying_key.gamma2).is_some()
        && bn254_g2_affine(&verifying_key.delta2).is_some()
        && verifying_key
            .ic
            .points()
            .iter()
            .all(|point| bn254_g1_affine(point).is_some())
}
/// Encode a valid SCCP Groth16 key exactly as Solidity `verifyingKeyHash()` does.
///
/// The result is the concatenation of 38 ABI words: alpha G1, beta/gamma/delta
/// G2 in contract limb order, then the twelve IC G1 points.
pub fn canonical_sccp_groth16_bn254_verifying_key_bytes_v1(
    verifying_key: &SccpGroth16Bn254VerifyingKeyV1,
) -> Option<Vec<u8>> {
    if !sccp_groth16_bn254_verifying_key_is_well_formed_v1(verifying_key) {
        return None;
    }
    let mut out = Vec::with_capacity(38 * 32);
    out.extend_from_slice(&verifying_key.alpha1.x);
    out.extend_from_slice(&verifying_key.alpha1.y);
    for point in [
        verifying_key.beta2,
        verifying_key.gamma2,
        verifying_key.delta2,
    ] {
        out.extend_from_slice(&point.x_c0);
        out.extend_from_slice(&point.x_c1);
        out.extend_from_slice(&point.y_c0);
        out.extend_from_slice(&point.y_c1);
    }
    for point in verifying_key.ic.points() {
        out.extend_from_slice(&point.x);
        out.extend_from_slice(&point.y);
    }
    Some(out)
}
/// Hash a valid SCCP Groth16 key byte-identically to Solidity `verifyingKeyHash()`.
pub fn sccp_groth16_bn254_verifying_key_hash_v1(
    verifying_key: &SccpGroth16Bn254VerifyingKeyV1,
) -> Option<H256> {
    Some(keccak256_bytes(
        &canonical_sccp_groth16_bn254_verifying_key_bytes_v1(verifying_key)?,
    ))
}
fn verify_sccp_groth16_bn254_pairing_equation_v1(
    proof: &SccpEvmGroth16Bn254ProofV1,
    public_signals: &[H256; 11],
    verifying_key: &SccpGroth16Bn254VerifyingKeyV1,
) -> bool {
    count_sccp_destination_groth16_pairing_v1();
    let Some(alpha1) = bn254_g1_affine(&verifying_key.alpha1) else {
        return false;
    };
    let Some(beta2) = bn254_g2_affine(&verifying_key.beta2) else {
        return false;
    };
    let Some(gamma2) = bn254_g2_affine(&verifying_key.gamma2) else {
        return false;
    };
    let Some(delta2) = bn254_g2_affine(&verifying_key.delta2) else {
        return false;
    };
    let Some(proof_a) = bn254_g1_affine(&sccp_g1_point_from_words(&proof.a)) else {
        return false;
    };
    let Some(proof_b) = bn254_g2_affine(&sccp_g2_point_from_words(&proof.b)) else {
        return false;
    };
    let Some(proof_c) = bn254_g1_affine(&sccp_g1_point_from_words(&proof.c)) else {
        return false;
    };
    let ic = verifying_key.ic.points();
    let Some(mut vk_x) = ic
        .first()
        .and_then(bn254_g1_affine)
        .map(|point| point.to_curve())
    else {
        return false;
    };
    for (ic, signal) in ic[1..].iter().zip(public_signals) {
        let Some(ic) = bn254_g1_affine(ic) else {
            return false;
        };
        let Some(signal) = bn254_fr_from_abi_word(signal) else {
            return false;
        };
        vk_x += ic.to_curve() * signal;
    }
    let neg_a = (-proof_a.to_curve()).to_affine();
    let vk_x = vk_x.to_affine();
    let pairing = bn256::multi_miller_loop(&[
        (&neg_a, &proof_b),
        (&alpha1, &beta2),
        (&vk_x, &gamma2),
        (&proof_c, &delta2),
    ])
    .final_exponentiation();
    bool::from(pairing.is_identity())
}
fn bls12381_g1_affine(bytes: &[u8]) -> Option<Bls12381G1Affine> {
    count_sccp_destination_bls12381_point_decode_v1();
    let bytes: [u8; 48] = bytes.try_into().ok()?;
    let encoded = bytes.into();
    let affine = Option::<Bls12381G1Affine>::from(Bls12381G1Affine::from_bytes(&encoded))?;
    (!bool::from(affine.is_identity())
        && bool::from(affine.to_curve().is_torsion_free())
        && affine.to_bytes().as_ref() == bytes.as_slice())
    .then_some(affine)
}
fn bls12381_g2_affine(bytes: &[u8]) -> Option<Bls12381G2Affine> {
    count_sccp_destination_bls12381_point_decode_v1();
    let bytes: [u8; 96] = bytes.try_into().ok()?;
    let encoded = bytes.into();
    let affine = Option::<Bls12381G2Affine>::from(Bls12381G2Affine::from_bytes(&encoded))?;
    (!bool::from(affine.is_identity())
        && bool::from(affine.to_curve().is_torsion_free())
        && affine.to_bytes().as_ref() == bytes.as_slice())
    .then_some(affine)
}
/// Return whether a TON BLS12-381 key has exactly twelve IC points and every
/// compressed point is canonical, non-identity, on-curve, and in the prime
/// subgroup.
#[must_use]
pub fn sccp_groth16_bls12381_verifying_key_is_well_formed_v1(
    key: &SccpGroth16Bls12381VerifyingKeyV1,
) -> bool {
    key.version == 1
        && bls12381_g1_affine(&key.alpha1).is_some()
        && bls12381_g2_affine(&key.beta2).is_some()
        && bls12381_g2_affine(&key.gamma2).is_some()
        && bls12381_g2_affine(&key.delta2).is_some()
        && key
            .ic
            .points()
            .iter()
            .all(|point| bls12381_g1_affine(point).is_some())
}
fn structural_sccp_groth16_bls12381_verifying_key_bytes_v1(
    key: &SccpGroth16Bls12381VerifyingKeyV1,
) -> Option<Vec<u8>> {
    canonical_structural_sccp_groth16_bls12381_verifying_key_bytes_v1(*key).ok()
}
/// Encode the exact compressed TON verification key in contract order.
#[must_use]
pub fn canonical_sccp_groth16_bls12381_verifying_key_bytes_v1(
    key: &SccpGroth16Bls12381VerifyingKeyV1,
) -> Option<Vec<u8>> {
    if !sccp_groth16_bls12381_verifying_key_is_well_formed_v1(key) {
        return None;
    }
    let bytes = structural_sccp_groth16_bls12381_verifying_key_bytes_v1(key)?;
    (bytes.len() == SCCP_TON_GROTH16_BLS12381_VERIFYING_KEY_BYTES_V1).then_some(bytes)
}
/// SHA-256 commitment to a canonical TON BLS12-381 verification key.
#[must_use]
pub fn sccp_groth16_bls12381_verifying_key_hash_v1(
    key: &SccpGroth16Bls12381VerifyingKeyV1,
) -> Option<H256> {
    let key_bytes = canonical_sccp_groth16_bls12381_verifying_key_bytes_v1(key)?;
    Some(sha256_bytes(&key_bytes))
}
fn bls12381_g1_compressed_is_structurally_canonical_v1(bytes: &[u8]) -> bool {
    let Ok(mut x) = <[u8; 48]>::try_from(bytes) else {
        return false;
    };
    if x[0] & 0x80 == 0 || x[0] & 0x40 != 0 {
        return false;
    }
    x[0] &= 0x1f;
    x < SCCP_GROTH16_BLS12381_BASE_FIELD_MODULUS_BE
}
fn bls12381_g2_compressed_is_structurally_canonical_v1(bytes: &[u8]) -> bool {
    let Ok(encoded) = <[u8; 96]>::try_from(bytes) else {
        return false;
    };
    let mut second = [0_u8; 48];
    second.copy_from_slice(&encoded[48..]);
    bls12381_g1_compressed_is_structurally_canonical_v1(&encoded[..48])
        && second < SCCP_GROTH16_BLS12381_BASE_FIELD_MODULUS_BE
}
fn structural_sccp_groth16_bls12381_proof_bytes_v1(
    proof: &SccpGroth16Bls12381ProofV1,
) -> Option<Vec<u8>> {
    if !bls12381_g1_compressed_is_structurally_canonical_v1(&proof.a.bytes)
        || !bls12381_g2_compressed_is_structurally_canonical_v1(&proof.b.bytes)
        || !bls12381_g1_compressed_is_structurally_canonical_v1(&proof.c.bytes)
    {
        return None;
    }
    let mut bytes = Vec::with_capacity(SCCP_TON_GROTH16_BLS12381_PROOF_BYTES_V1);
    bytes.extend_from_slice(&proof.a.bytes);
    bytes.extend_from_slice(&proof.b.bytes);
    bytes.extend_from_slice(&proof.c.bytes);
    (bytes.len() == SCCP_TON_GROTH16_BLS12381_PROOF_BYTES_V1).then_some(bytes)
}
/// Encode a canonical compressed BLS12-381 Groth16 proof as `A || B || C`.
#[must_use]
pub fn canonical_sccp_groth16_bls12381_proof_bytes_v1(
    proof: &SccpGroth16Bls12381ProofV1,
) -> Option<Vec<u8>> {
    bls12381_g1_affine(&proof.a.bytes)?;
    bls12381_g2_affine(&proof.b.bytes)?;
    bls12381_g1_affine(&proof.c.bytes)?;
    structural_sccp_groth16_bls12381_proof_bytes_v1(proof)
}
/// Decode and subgroup-check one exact `A || B || C` compressed proof.
#[must_use]
pub fn decode_sccp_groth16_bls12381_proof_bytes_v1(
    bytes: &[u8],
) -> Option<SccpGroth16Bls12381ProofV1> {
    if bytes.len() != SCCP_TON_GROTH16_BLS12381_PROOF_BYTES_V1 {
        return None;
    }
    let proof = SccpGroth16Bls12381ProofV1 {
        a: SccpBls12381G1CompressedV1 {
            bytes: bytes[..48].to_vec(),
        },
        b: SccpBls12381G2CompressedV1 {
            bytes: bytes[48..144].to_vec(),
        },
        c: SccpBls12381G1CompressedV1 {
            bytes: bytes[144..].to_vec(),
        },
    };
    (canonical_sccp_groth16_bls12381_proof_bytes_v1(&proof)?.as_slice() == bytes).then_some(proof)
}
fn bls12381_fr_from_be_word(word: H256) -> Option<Bls12381Fr> {
    if word >= SCCP_GROTH16_BLS12381_SCALAR_FIELD_MODULUS_BE {
        return None;
    }
    let mut little_endian = word;
    little_endian.reverse();
    Option::<Bls12381Fr>::from(Bls12381Fr::from_repr(little_endian.into()))
}
/// Verify the complete four-term BLS12-381 Groth16 pairing equation for the
/// exact eleven TON public signals.
#[must_use]
pub fn verify_sccp_groth16_bls12381_pairing_v1(
    proof: &SccpGroth16Bls12381ProofV1,
    public_signals: &SccpGroth16Bls12381PublicSignalsV1,
    key: &SccpGroth16Bls12381VerifyingKeyV1,
) -> bool {
    count_sccp_destination_groth16_pairing_v1();
    if !sccp_groth16_bls12381_verifying_key_is_well_formed_v1(key) {
        return false;
    }
    let Some(alpha1) = bls12381_g1_affine(&key.alpha1) else {
        return false;
    };
    let Some(beta2) = bls12381_g2_affine(&key.beta2) else {
        return false;
    };
    let Some(gamma2) = bls12381_g2_affine(&key.gamma2) else {
        return false;
    };
    let Some(delta2) = bls12381_g2_affine(&key.delta2) else {
        return false;
    };
    let Some(proof_a) = bls12381_g1_affine(&proof.a.bytes) else {
        return false;
    };
    let Some(proof_b) = bls12381_g2_affine(&proof.b.bytes) else {
        return false;
    };
    let Some(proof_c) = bls12381_g1_affine(&proof.c.bytes) else {
        return false;
    };
    let ic = key.ic.points();
    let Some(mut vk_x) = bls12381_g1_affine(&ic[0]).map(|point| point.to_curve()) else {
        return false;
    };
    for (ic, signal) in ic[1..].iter().zip(public_signals.words()) {
        let Some(ic) = bls12381_g1_affine(ic) else {
            return false;
        };
        let Some(signal) = bls12381_fr_from_be_word(signal) else {
            return false;
        };
        vk_x += ic.to_curve() * signal;
    }
    let neg_a = (-proof_a.to_curve()).to_affine();
    let vk_x = vk_x.to_affine();
    let pairing = bls12381::multi_miller_loop(&[
        (&neg_a, &proof_b),
        (&alpha1, &beta2),
        (&vk_x, &gamma2),
        (&proof_c, &delta2),
    ])
    .final_exponentiation();
    bool::from(pairing.is_identity())
}
fn abi_word_at(payload: &[u8], index: usize) -> Option<H256> {
    let start = index.checked_mul(32)?;
    let end = start.checked_add(32)?;
    let mut word = [0u8; 32];
    word.copy_from_slice(payload.get(start..end)?);
    Some(word)
}
/// Decode the fixed-width canonical ABI words of one EVM Groth16/bn254 proof.
pub fn decode_sccp_evm_groth16_bn254_proof_bytes(
    payload: &[u8],
) -> Option<SccpEvmGroth16Bn254ProofV1> {
    if payload.len() != 32 * 12 {
        return None;
    }
    let version = abi_read_u8_word(&abi_word_at(payload, 0)?)?;
    let message_id = abi_word_at(payload, 1)?;
    let source_domain = abi_read_u32_word(&abi_word_at(payload, 2)?)?;
    let commitment_root = abi_word_at(payload, 3)?;
    let proof = SccpEvmGroth16Bn254ProofV1 {
        version,
        message_id,
        source_domain,
        commitment_root,
        a: [abi_word_at(payload, 4)?, abi_word_at(payload, 5)?],
        b: [
            abi_word_at(payload, 6)?,
            abi_word_at(payload, 7)?,
            abi_word_at(payload, 8)?,
            abi_word_at(payload, 9)?,
        ],
        c: [abi_word_at(payload, 10)?, abi_word_at(payload, 11)?],
    };
    (abi_g1_point_is_structurally_valid(&proof.a)
        && abi_g2_point_is_structurally_valid(&proof.b)
        && abi_g1_point_is_structurally_valid(&proof.c))
    .then_some(proof)
}
/// Encode one EVM Groth16/bn254 proof as its fixed-width canonical ABI words.
pub fn encode_sccp_evm_groth16_bn254_proof_bytes(proof: &SccpEvmGroth16Bn254ProofV1) -> Vec<u8> {
    let mut out = Vec::with_capacity(32 * 12);
    out.extend_from_slice(&abi_word_u32(u32::from(proof.version)));
    out.extend_from_slice(&proof.message_id);
    out.extend_from_slice(&abi_word_u32(proof.source_domain));
    out.extend_from_slice(&proof.commitment_root);
    for word in proof.a.iter().chain(proof.b.iter()).chain(proof.c.iter()) {
        out.extend_from_slice(word);
    }
    out
}
fn verify_sccp_groth16_bn254_proof_against_validated_request_v1(
    validated: &ValidatedSccpGroth16Bn254ProofRequestV1<'_>,
    proof_bytes: &[u8],
) -> bool {
    let request = validated.request;
    let public_inputs = &request.public_inputs;
    let source_domain = request.source_network.domain_id();
    let Some(proof) = decode_sccp_evm_groth16_bn254_proof_bytes(proof_bytes) else {
        return false;
    };
    if proof.version != 1
        || proof.message_id != public_inputs.message_id
        || proof.source_domain != source_domain
        || proof.commitment_root != public_inputs.commitment_root
        || encode_sccp_evm_groth16_bn254_proof_bytes(&proof) != proof_bytes
    {
        return false;
    }
    verify_sccp_groth16_bn254_pairing_equation_v1(
        &proof,
        &validated.public_signal_words,
        &request.verifying_key,
    )
}
/// Verify an SCCP Groth16 proof against an exact governed BN254 prover request.
///
/// This performs the same eleven signal hashes and four-term pairing equation as
/// `SccpGroth16Bn254MessageVerifier.sol`. The expected key hash must come from the request derived
/// from typed governed deployment state, never from proof-controlled metadata. The semantic-profile
/// hash is not a twelfth signal; it is nevertheless required here so all six governed hash roles
/// receive the same nonzero, pairwise-distinct admission check as the destination verifier.
pub fn verify_sccp_groth16_bn254_proof_v1(
    request: &SccpGroth16Bn254ProofRequestV1,
    proof_bytes: &[u8],
) -> bool {
    let Some(validated) = validate_sccp_groth16_bn254_proof_request_v1(request, request.backend)
    else {
        return false;
    };
    verify_sccp_groth16_bn254_proof_against_validated_request_v1(&validated, proof_bytes)
}
fn decode_canonical_sccp_merkle_proof_bytes(proof_bytes: &[u8]) -> Option<SccpMerkleProofV1> {
    let mut cursor = PayloadCursor::new(proof_bytes);
    let step_count = usize::try_from(cursor.take_u32()?).ok()?;
    if step_count > SCCP_TAIRA_MAX_MERKLE_PROOF_STEPS_V1
        || step_count > proof_bytes.len().saturating_sub(4) / 33
    {
        return None;
    }
    let mut steps = Vec::with_capacity(step_count);
    for _ in 0..step_count {
        let sibling_hash: H256 = cursor.take_exact(32)?.try_into().ok()?;
        let sibling_is_left = match cursor.take_u8()? {
            0 => false,
            1 => true,
            _ => return None,
        };
        steps.push(SccpMerkleStepV1 {
            sibling_hash,
            sibling_is_left,
        });
    }
    cursor.is_finished().then_some(SccpMerkleProofV1 { steps })
}
struct SccpCanonicalMessageBundleSummaryV1 {
    source_network: SccpNetworkV1,
    target_network: SccpNetworkV1,
    destination_binding_hash: H256,
    route_configuration_hash: H256,
    canonical_payload_bytes: Vec<u8>,
    message_id: H256,
    payload_hash: H256,
    commitment_root: H256,
    finality_proof: Vec<u8>,
}
struct SccpDecodedCanonicalMessageBundleV1 {
    bundle: TairaSccpMessageProofV1,
    canonical_payload_bytes: Vec<u8>,
}
#[derive(Clone, Copy)]
struct SccpCanonicalMessageBundleBindingV1<'a> {
    source_network: SccpNetworkV1,
    target_network: SccpNetworkV1,
    destination_binding_hash: H256,
    route_configuration_hash: H256,
    message_id: H256,
    payload_hash: H256,
    commitment_root: H256,
    finality_proof: &'a [u8],
}
impl SccpCanonicalMessageBundleSummaryV1 {
    fn binding(&self) -> SccpCanonicalMessageBundleBindingV1<'_> {
        SccpCanonicalMessageBundleBindingV1 {
            source_network: self.source_network,
            target_network: self.target_network,
            destination_binding_hash: self.destination_binding_hash,
            route_configuration_hash: self.route_configuration_hash,
            message_id: self.message_id,
            payload_hash: self.payload_hash,
            commitment_root: self.commitment_root,
            finality_proof: &self.finality_proof,
        }
    }
}
fn decode_canonical_taira_sccp_message_bundle_summary(
    bundle_bytes: &[u8],
) -> Option<SccpCanonicalMessageBundleSummaryV1> {
    let decoded = decode_canonical_taira_sccp_message_bundle_with_payload_v1(bundle_bytes)?;
    let bundle = decoded.bundle;
    Some(SccpCanonicalMessageBundleSummaryV1 {
        source_network: bundle.commitment.context.lane.source,
        target_network: bundle.commitment.context.lane.target,
        destination_binding_hash: bundle.commitment.context.destination_binding_hash,
        route_configuration_hash: bundle.commitment.context.route_configuration_hash,
        canonical_payload_bytes: decoded.canonical_payload_bytes,
        message_id: bundle.commitment.message_id,
        payload_hash: bundle.commitment.payload_hash,
        commitment_root: bundle.commitment_root,
        finality_proof: bundle.finality_proof,
    })
}
fn decode_canonical_taira_sccp_message_bundle_with_payload_v1(
    bundle_bytes: &[u8],
) -> Option<SccpDecodedCanonicalMessageBundleV1> {
    count_sccp_destination_bundle_decode_v1();
    if bundle_bytes.is_empty() || bundle_bytes.len() > SCCP_TAIRA_MAX_ENCODED_PROOF_BYTES_V1 {
        return None;
    }
    let mut cursor = PayloadCursor::new(bundle_bytes);
    if cursor.take_u8()? != 1 {
        return None;
    }
    let commitment_root: H256 = cursor.take_exact(32)?.try_into().ok()?;
    let commitment_bytes = cursor.take_vec()?;
    let merkle_proof_bytes = cursor.take_vec()?;
    let payload_bytes = cursor.take_vec()?;
    let finality_proof = cursor.take_vec()?;
    if !cursor.is_finished() {
        return None;
    }
    let payload = decode_canonical_sccp_payload_bytes(&payload_bytes)?;
    if !verify_sccp_payload_structure(&payload)
        || canonical_sccp_payload_bytes(&payload).ok()? != payload_bytes
    {
        return None;
    }
    let commitment = decode_canonical_commitment_bytes(&commitment_bytes)?;
    let expected_commitment = hub_commitment_from_sccp_payload(commitment.context, &payload)?;
    if commitment != expected_commitment {
        return None;
    }
    let merkle_proof = decode_canonical_sccp_merkle_proof_bytes(&merkle_proof_bytes)?;
    if merkle_root_from_commitment(&commitment, &merkle_proof) != commitment_root {
        return None;
    }
    Some(SccpDecodedCanonicalMessageBundleV1 {
        bundle: TairaSccpMessageProofV1 {
            version: 1,
            commitment_root,
            commitment,
            merkle_proof,
            payload,
            finality_proof,
        },
        canonical_payload_bytes: payload_bytes,
    })
}
/// Decode the compact canonical message-bundle byte layout embedded in one Groth16 prover request.
///
/// This is distinct from [`decode_taira_sccp_message_proof`], which decodes a top-level
/// Norito-framed Torii artifact. The embedded request layout is length-delimited by the SCCP
/// protocol itself and must never be guessed as a Norito frame.
pub fn decode_canonical_taira_sccp_message_bundle_v1(
    bundle_bytes: &[u8],
) -> Option<TairaSccpMessageProofV1> {
    let decoded = decode_canonical_taira_sccp_message_bundle_with_payload_v1(bundle_bytes)?;
    let finality = decode_taira_bridge_finality_proof(&decoded.bundle.finality_proof)?;
    verify_message_bundle_structure_with_finality(&decoded.bundle, &finality)
        .then_some(decoded.bundle)
}
fn sccp_proof_request_bundle_binding_matches_public_inputs_with_finality(
    public_inputs: &SccpMessagePublicInputsV1,
    bundle: SccpCanonicalMessageBundleBindingV1<'_>,
    source_network: SccpNetworkV1,
    target_network: SccpNetworkV1,
    destination_binding_hash: H256,
    route_configuration_hash: H256,
    finality: &TairaBridgeFinalityProofV1,
) -> bool {
    if bundle.source_network != source_network
        || bundle.target_network != target_network
        || source_network.domain_id() != SCCP_DOMAIN_SORA
        || target_network.domain_id() != public_inputs.target_domain
        || bundle.destination_binding_hash != destination_binding_hash
        || bundle.route_configuration_hash != route_configuration_hash
        || bundle.message_id != public_inputs.message_id
        || bundle.payload_hash != public_inputs.payload_hash
        || bundle.commitment_root != public_inputs.commitment_root
    {
        return false;
    }
    finality.block_header.sccp_commitment_root() == Some(bundle.commitment_root)
        && finality.finality_artifact.height == public_inputs.finality_height
        && hash_block_header_for_sccp_finality(&finality.block_header)
            == public_inputs.finality_block_hash
}
fn sccp_destination_proof_backend_tag_v1(backend: BridgeSccpDestinationProofBackendV1) -> u8 {
    match backend {
        BridgeSccpDestinationProofBackendV1::EvmGroth16Bn254 => 0,
        BridgeSccpDestinationProofBackendV1::TronGroth16Bn254 => 1,
        BridgeSccpDestinationProofBackendV1::TonGroth16Bls12381 => 3,
    }
}
fn sccp_destination_proof_backend_supports_network_v1(
    backend: BridgeSccpDestinationProofBackendV1,
    target_network: SccpNetworkV1,
) -> bool {
    match backend {
        BridgeSccpDestinationProofBackendV1::EvmGroth16Bn254 => matches!(
            target_network,
            SccpNetworkV1::EthereumMainnet | SccpNetworkV1::BscMainnet
        ),
        BridgeSccpDestinationProofBackendV1::TronGroth16Bn254 => {
            target_network == SccpNetworkV1::TronMainnet
        }
        BridgeSccpDestinationProofBackendV1::TonGroth16Bls12381 => {
            target_network == SccpNetworkV1::TonMainnet
        }
    }
}
fn sccp_groth16_bn254_statement_hash_v1(
    request: &SccpGroth16Bn254ProofRequestV1,
    canonical_payload_bytes: &[u8],
) -> Option<H256> {
    if request.source_network != SccpNetworkV1::SoraTaira
        || !sccp_destination_proof_backend_supports_network_v1(
            request.backend,
            request.target_network,
        )
        || request.target_network.domain_id() != request.public_inputs.target_domain
        || canonical_payload_bytes.is_empty()
        || request.bundle_bytes.is_empty()
        || [
            request.destination_binding_hash,
            request.route_configuration_hash,
            request.verifier_key_hash,
            request.semantic_proof_profile_hash,
            request.sora_finality_anchor_hash,
        ]
        .iter()
        .any(|value| !h256_is_nonzero(value))
        || hash_roles_alias(&[
            request.destination_binding_hash,
            request.route_configuration_hash,
            request.verifier_key_hash,
            request.semantic_proof_profile_hash,
            request.sora_finality_anchor_hash,
        ])
    {
        return None;
    }
    let mut statement =
        Vec::with_capacity(canonical_payload_bytes.len() + request.bundle_bytes.len() + 512);
    push_u8(&mut statement, 1);
    push_u8(
        &mut statement,
        sccp_destination_proof_backend_tag_v1(request.backend),
    );
    push_vec_checked(
        &mut statement,
        &canonical_sccp_network_bytes_v1(request.source_network),
    )?;
    push_vec_checked(
        &mut statement,
        &canonical_sccp_network_bytes_v1(request.target_network),
    )?;
    statement.extend_from_slice(&request.destination_binding_hash);
    statement.extend_from_slice(&request.route_configuration_hash);
    statement.extend_from_slice(&request.verifier_key_hash);
    statement.extend_from_slice(&request.semantic_proof_profile_hash);
    statement.extend_from_slice(&request.sora_finality_anchor_hash);
    statement.extend_from_slice(&canonical_sccp_message_public_inputs_bytes(
        &request.public_inputs,
    ));
    push_vec_checked(&mut statement, canonical_payload_bytes)?;
    push_vec_checked(&mut statement, &request.bundle_bytes)?;
    Some(prefixed_blake2b(
        SCCP_GROTH16_STATEMENT_PREFIX_V1,
        &statement,
    ))
}
fn sccp_groth16_bn254_proof_request_hash(
    request: &SccpGroth16Bn254ProofRequestV1,
    canonical_payload_bytes: &[u8],
    public_signal_words: &[H256; 11],
) -> Option<H256> {
    let public_inputs_bytes = canonical_sccp_message_public_inputs_bytes(&request.public_inputs);
    let semantic_proof_profile_bytes =
        canonical_sccp_semantic_proof_profile_bytes_v1(request.semantic_proof_profile).ok()?;
    let sora_finality_anchor_bytes =
        canonical_sccp_sora_finality_anchor_bytes_v1(request.sora_finality_anchor).ok()?;
    let verifying_key_bytes =
        canonical_sccp_groth16_bn254_verifying_key_bytes_v1(&request.verifying_key)?;
    let mut preimage = Vec::with_capacity(
        public_inputs_bytes.len()
            + canonical_payload_bytes.len()
            + request.bundle_bytes.len()
            + semantic_proof_profile_bytes.len()
            + sora_finality_anchor_bytes.len()
            + verifying_key_bytes.len()
            + 512,
    );
    push_u8(&mut preimage, 1);
    push_u8(
        &mut preimage,
        sccp_destination_proof_backend_tag_v1(request.backend),
    );
    push_vec_checked(
        &mut preimage,
        &canonical_sccp_network_bytes_v1(request.source_network),
    )?;
    push_vec_checked(
        &mut preimage,
        &canonical_sccp_network_bytes_v1(request.target_network),
    )?;
    push_vec_checked(&mut preimage, &public_inputs_bytes)?;
    push_vec_checked(&mut preimage, canonical_payload_bytes)?;
    push_vec_checked(&mut preimage, &request.bundle_bytes)?;
    push_vec_checked(&mut preimage, &semantic_proof_profile_bytes)?;
    push_vec_checked(&mut preimage, &sora_finality_anchor_bytes)?;
    preimage.extend_from_slice(&request.statement_hash);
    preimage.extend_from_slice(&request.destination_binding_hash);
    preimage.extend_from_slice(&request.route_configuration_hash);
    preimage.extend_from_slice(&request.verifier_key_hash);
    preimage.extend_from_slice(&request.semantic_proof_profile_hash);
    preimage.extend_from_slice(&request.sora_finality_anchor_hash);
    push_vec_checked(&mut preimage, &verifying_key_bytes)?;
    for word in public_signal_words {
        preimage.extend_from_slice(word);
    }
    Some(prefixed_blake2b(
        SCCP_GROTH16_PROOF_REQUEST_PREFIX_V1,
        &preimage,
    ))
}
struct SccpGroth16Bn254ProofRequestBuildContextV1<'a> {
    backend: BridgeSccpDestinationProofBackendV1,
    source_network: SccpNetworkV1,
    target_network: SccpNetworkV1,
    public_inputs: &'a SccpMessagePublicInputsV1,
    canonical_payload_bytes: &'a [u8],
    bundle_bytes: &'a [u8],
    bundle_binding: SccpCanonicalMessageBundleBindingV1<'a>,
    finality: &'a TairaBridgeFinalityProofV1,
    destination_binding_hash: H256,
    route_configuration_hash: H256,
    verifying_key: &'a SccpGroth16Bn254VerifyingKeyV1,
    expected_verifier_key_hash: H256,
    outbound_proof_policy: SccpOutboundProofPolicyV1,
}
fn sccp_groth16_bn254_build_context_is_valid_v1(
    context: &SccpGroth16Bn254ProofRequestBuildContextV1<'_>,
    semantic_proof_profile_hash: H256,
    sora_finality_anchor_hash: H256,
) -> bool {
    context.source_network == SccpNetworkV1::SoraTaira
        && sccp_destination_proof_backend_supports_network_v1(
            context.backend,
            context.target_network,
        )
        && context.public_inputs.target_domain == context.target_network.domain_id()
        && sccp_groth16_proof_request_public_inputs_are_valid(
            context.source_network,
            context.target_network,
            context.public_inputs,
        )
        && sccp_proof_request_bundle_binding_matches_public_inputs_with_finality(
            context.public_inputs,
            context.bundle_binding,
            context.source_network,
            context.target_network,
            context.destination_binding_hash,
            context.route_configuration_hash,
            context.finality,
        )
        && payload_hash(context.canonical_payload_bytes) == context.public_inputs.payload_hash
        && h256_is_nonzero(&context.expected_verifier_key_hash)
        && sccp_groth16_bn254_verifying_key_hash_v1(context.verifying_key)
            == Some(context.expected_verifier_key_hash)
        && !hash_roles_alias(&[
            context.destination_binding_hash,
            context.route_configuration_hash,
            context.expected_verifier_key_hash,
            semantic_proof_profile_hash,
            sora_finality_anchor_hash,
        ])
}
fn build_sccp_groth16_bn254_proof_request(
    context: &SccpGroth16Bn254ProofRequestBuildContextV1<'_>,
) -> Option<SccpGroth16Bn254ProofRequestV1> {
    context.outbound_proof_policy.validate().ok()?;
    let semantic_proof_profile = context.outbound_proof_policy.semantic_profile;
    let sora_finality_anchor = context.outbound_proof_policy.sora_finality_anchor;
    let semantic_proof_profile_hash =
        sccp_semantic_proof_profile_hash_v1(semantic_proof_profile).ok()?;
    let sora_finality_anchor_hash = sccp_sora_finality_anchor_hash_v1(sora_finality_anchor).ok()?;
    if !sccp_groth16_bn254_build_context_is_valid_v1(
        context,
        semantic_proof_profile_hash,
        sora_finality_anchor_hash,
    ) {
        return None;
    }
    let mut request = SccpGroth16Bn254ProofRequestV1 {
        version: 1,
        backend: context.backend,
        source_network: context.source_network,
        target_network: context.target_network,
        public_inputs: *context.public_inputs,
        verifying_key: *context.verifying_key,
        verifier_key_hash: context.expected_verifier_key_hash,
        semantic_proof_profile,
        semantic_proof_profile_hash,
        sora_finality_anchor,
        sora_finality_anchor_hash,
        bundle_bytes: context.bundle_bytes.to_vec(),
        statement_hash: [0; 32],
        destination_binding_hash: context.destination_binding_hash,
        route_configuration_hash: context.route_configuration_hash,
        request_hash: [0; 32],
    };
    let statement_hash =
        sccp_groth16_bn254_statement_hash_v1(&request, context.canonical_payload_bytes)?;
    if !sccp_groth16_bn254_request_hash_roles_are_distinct_v1(
        statement_hash,
        context.destination_binding_hash,
        context.route_configuration_hash,
        context.expected_verifier_key_hash,
        semantic_proof_profile_hash,
        sora_finality_anchor_hash,
    ) {
        return None;
    }
    request.statement_hash = statement_hash;
    let public_signal_words = sccp_groth16_bn254_public_signal_words(
        context.public_inputs,
        context.source_network.domain_id(),
        statement_hash,
        context.destination_binding_hash,
        context.route_configuration_hash,
        sora_finality_anchor_hash,
    );
    request.request_hash = sccp_groth16_bn254_proof_request_hash(
        &request,
        context.canonical_payload_bytes,
        &public_signal_words,
    )?;
    Some(request)
}
fn prefixed_sha256(prefix: &[u8], payload: &[u8]) -> H256 {
    let mut preimage = Vec::with_capacity(prefix.len() + payload.len());
    preimage.extend_from_slice(prefix);
    preimage.extend_from_slice(payload);
    sha256_bytes(&preimage)
}
fn ton_semantic_circuit_commitment_v1(policy: SccpOutboundProofPolicyV1) -> Option<H256> {
    policy.validate().ok()?;
    let SccpSemanticProofProfileV1::SoraTairaFinalityInclusionGroth16Bls12381(circuit) =
        policy.semantic_profile
    else {
        return None;
    };
    (circuit.public_signal_schema_hash == sccp_groth16_bls12381_public_signal_schema_hash_v1())
        .then_some(circuit.circuit_commitment)
}
fn sccp_ton_deployment_matches_verifying_key_v1(
    deployment: &SccpTonDestinationDeploymentV1,
) -> bool {
    deployment.taira_to_token_multiplier == SCCP_V1_TAIRA_TO_TON_TOKEN_MULTIPLIER
        && deployment.proof_profile_commitment
            == sccp_ton_groth16_bls12381_proof_profile_commitment_v1()
        && ton_semantic_circuit_commitment_v1(deployment.outbound_proof_policy)
            == Some(deployment.verifier_circuit_hash)
        && sccp_groth16_bls12381_verifying_key_hash_v1(&deployment.verifying_key)
            == Some(deployment.verifier_key_hash)
}
fn sccp_governed_ton_route_matches_bundle_v1(
    bundle: &TairaSccpMessageProofV1,
    governed_route: &SccpGovernedRouteV1,
) -> bool {
    let SccpDestinationDeploymentV1::Ton(deployment) = governed_route.destination else {
        return false;
    };
    if governed_route.validate().is_err()
        || governed_route.lane_id.source != SccpNetworkV1::TonMainnet
        || governed_route.lane_id.target != SccpNetworkV1::SoraTaira
        || bundle.commitment.context.lane
            != (SccpLaneIdV1 {
                source: SccpNetworkV1::SoraTaira,
                target: governed_route.lane_id.source,
            })
        || governed_route.destination_binding_hash().ok()
            != Some(bundle.commitment.context.destination_binding_hash)
        || governed_route.route_configuration_hash().ok()
            != Some(bundle.commitment.context.route_configuration_hash)
        || !sccp_ton_deployment_matches_verifying_key_v1(&deployment)
        || !sccp_payload_matches_exact_xor_destination_route_v1(&bundle.payload, SCCP_DOMAIN_TON)
    {
        return false;
    }
    let SccpPayloadV1::Transfer(transfer) = &bundle.payload;
    transfer.route_revision == governed_route.revision
        && transfer.route_id == governed_route.route_id.as_bytes()
        && transfer.asset_id == governed_route.asset_key.as_bytes()
}
fn sccp_ton_groth16_bls12381_statement_hash_v1(
    request: &SccpTonGroth16Bls12381ProofRequestV1,
    canonical_payload_bytes: &[u8],
) -> Option<H256> {
    if request.version != 1
        || request.backend != BridgeSccpDestinationProofBackendV1::TonGroth16Bls12381
        || request.source_network != SccpNetworkV1::SoraTaira
        || request.target_network != SccpNetworkV1::TonMainnet
        || request.public_inputs.target_domain != SCCP_DOMAIN_TON
        || canonical_payload_bytes.is_empty()
        || canonical_payload_bytes.len() > SCCP_TON_DESTINATION_MAX_PAYLOAD_BYTES_V1
        || request.bundle_bytes.is_empty()
    {
        return None;
    }
    let roles = [
        request.destination_binding_hash,
        request.route_configuration_hash,
        request.verifier_circuit_hash,
        request.verifier_key_hash,
        request.proof_profile_commitment,
        request.semantic_proof_profile_hash,
        request.sora_finality_anchor_hash,
    ];
    if roles.iter().any(|role| !h256_is_nonzero(role)) || hash_roles_alias(&roles) {
        return None;
    }
    let profile_bytes =
        canonical_sccp_semantic_proof_profile_bytes_v1(request.semantic_proof_profile).ok()?;
    let anchor_bytes =
        canonical_sccp_sora_finality_anchor_bytes_v1(request.sora_finality_anchor).ok()?;
    let mut preimage = Vec::with_capacity(
        canonical_payload_bytes.len() + request.bundle_bytes.len() + profile_bytes.len() + 768,
    );
    preimage.push(1);
    preimage.push(sccp_destination_proof_backend_tag_v1(request.backend));
    push_vec_checked(
        &mut preimage,
        &canonical_sccp_network_bytes_v1(request.source_network),
    )?;
    push_vec_checked(
        &mut preimage,
        &canonical_sccp_network_bytes_v1(request.target_network),
    )?;
    preimage.extend_from_slice(&request.destination_binding_hash);
    preimage.extend_from_slice(&request.route_configuration_hash);
    preimage.extend_from_slice(&request.verifier_circuit_hash);
    preimage.extend_from_slice(&request.verifier_key_hash);
    preimage.extend_from_slice(&request.proof_profile_commitment);
    preimage.extend_from_slice(&request.semantic_proof_profile_hash);
    preimage.extend_from_slice(&request.sora_finality_anchor_hash);
    push_vec_checked(&mut preimage, &profile_bytes)?;
    push_vec_checked(&mut preimage, &anchor_bytes)?;
    preimage.extend_from_slice(&canonical_sccp_message_public_inputs_bytes(
        &request.public_inputs,
    ));
    push_vec_checked(&mut preimage, canonical_payload_bytes)?;
    push_vec_checked(&mut preimage, &request.bundle_bytes)?;
    Some(prefixed_sha256(
        SCCP_TON_GROTH16_STATEMENT_PREFIX_V1,
        &preimage,
    ))
}
fn sccp_ton_groth16_bls12381_request_hash_v1(
    request: &SccpTonGroth16Bls12381ProofRequestV1,
    canonical_payload_bytes: &[u8],
) -> Option<H256> {
    let key_bytes =
        structural_sccp_groth16_bls12381_verifying_key_bytes_v1(&request.verifying_key)?;
    let profile_bytes =
        canonical_sccp_semantic_proof_profile_bytes_v1(request.semantic_proof_profile).ok()?;
    let anchor_bytes =
        canonical_sccp_sora_finality_anchor_bytes_v1(request.sora_finality_anchor).ok()?;
    let mut preimage = Vec::with_capacity(
        request.bundle_bytes.len()
            + canonical_payload_bytes.len()
            + key_bytes.len()
            + profile_bytes.len()
            + anchor_bytes.len()
            + 1024,
    );
    preimage.push(1);
    preimage.push(sccp_destination_proof_backend_tag_v1(request.backend));
    push_vec_checked(
        &mut preimage,
        &canonical_sccp_network_bytes_v1(request.source_network),
    )?;
    push_vec_checked(
        &mut preimage,
        &canonical_sccp_network_bytes_v1(request.target_network),
    )?;
    preimage.extend_from_slice(&canonical_sccp_message_public_inputs_bytes(
        &request.public_inputs,
    ));
    for signal in request.public_signals.words() {
        preimage.extend_from_slice(&signal);
    }
    push_vec_checked(&mut preimage, &key_bytes)?;
    push_vec_checked(&mut preimage, &profile_bytes)?;
    push_vec_checked(&mut preimage, &anchor_bytes)?;
    push_vec_checked(&mut preimage, canonical_payload_bytes)?;
    push_vec_checked(&mut preimage, &request.bundle_bytes)?;
    preimage.extend_from_slice(&request.statement_hash);
    preimage.extend_from_slice(&request.destination_binding_hash);
    preimage.extend_from_slice(&request.route_configuration_hash);
    preimage.extend_from_slice(&request.verifier_circuit_hash);
    preimage.extend_from_slice(&request.verifier_key_hash);
    preimage.extend_from_slice(&request.proof_profile_commitment);
    preimage.extend_from_slice(&request.semantic_proof_profile_hash);
    preimage.extend_from_slice(&request.sora_finality_anchor_hash);
    Some(prefixed_sha256(
        SCCP_TON_GROTH16_PROOF_REQUEST_PREFIX_V1,
        &preimage,
    ))
}
fn build_sccp_ton_groth16_bls12381_request_from_bound_finality_v1(
    bundle: &TairaSccpMessageProofV1,
    governed_route: &SccpGovernedRouteV1,
    finality: &TairaBridgeFinalityProofV1,
) -> Option<SccpTonGroth16Bls12381ProofRequestV1> {
    if !sccp_governed_ton_route_matches_bundle_v1(bundle, governed_route) {
        return None;
    }
    let SccpDestinationDeploymentV1::Ton(deployment) = governed_route.destination else {
        return None;
    };
    let public_inputs = sccp_message_public_inputs_with_finality(bundle, finality)?;
    let canonical_payload_bytes = canonical_sccp_payload_bytes(&bundle.payload).ok()?;
    if canonical_payload_bytes.len() > SCCP_TON_DESTINATION_MAX_PAYLOAD_BYTES_V1 {
        return None;
    }
    let bundle_bytes = canonical_taira_sccp_message_bundle_bytes_checked(bundle)?;
    let semantic_proof_profile = deployment.outbound_proof_policy.semantic_profile;
    let semantic_proof_profile_hash = deployment
        .outbound_proof_policy
        .semantic_profile_hash()
        .ok()?;
    let sora_finality_anchor = deployment.outbound_proof_policy.sora_finality_anchor;
    let sora_finality_anchor_hash = deployment
        .outbound_proof_policy
        .sora_finality_anchor_hash()
        .ok()?;
    let mut request = SccpTonGroth16Bls12381ProofRequestV1 {
        version: 1,
        backend: BridgeSccpDestinationProofBackendV1::TonGroth16Bls12381,
        source_network: SccpNetworkV1::SoraTaira,
        target_network: governed_route.lane_id.source,
        public_inputs,
        public_signals: SccpGroth16Bls12381PublicSignalsV1::from([[0; 32]; 11]),
        verifying_key: deployment.verifying_key,
        verifier_key_hash: deployment.verifier_key_hash,
        verifier_circuit_hash: deployment.verifier_circuit_hash,
        proof_profile_commitment: deployment.proof_profile_commitment,
        semantic_proof_profile,
        semantic_proof_profile_hash,
        sora_finality_anchor,
        sora_finality_anchor_hash,
        bundle_bytes,
        statement_hash: [0; 32],
        destination_binding_hash: governed_route.destination_binding_hash().ok()?,
        route_configuration_hash: governed_route.route_configuration_hash().ok()?,
        request_hash: [0; 32],
    };
    request.statement_hash =
        sccp_ton_groth16_bls12381_statement_hash_v1(&request, &canonical_payload_bytes)?;
    request.public_signals =
        SccpGroth16Bls12381PublicSignalsV1::from(sccp_groth16_bls12381_public_signal_words_v1(
            &request.public_inputs,
            SCCP_DOMAIN_SORA,
            request.statement_hash,
            request.destination_binding_hash,
            request.route_configuration_hash,
            request.sora_finality_anchor_hash,
        ));
    request.request_hash =
        sccp_ton_groth16_bls12381_request_hash_v1(&request, &canonical_payload_bytes)?;
    Some(request)
}
/// Build a canonical TON BLS12-381 request from an untrusted Taira bundle,
/// governed route, including its exact content-addressed verification key.
///
/// This entry point verifies Taira finality before emitting prover material.
#[must_use]
pub fn build_sccp_ton_groth16_bls12381_proof_request_from_governed_route_v1(
    bundle: &TairaSccpMessageProofV1,
    governed_route: &SccpGovernedRouteV1,
) -> Option<SccpTonGroth16Bls12381ProofRequestV1> {
    let finality =
        verified_sccp_message_taira_finality_proof_cryptographically_self_consistent(bundle)?;
    build_sccp_ton_groth16_bls12381_request_from_bound_finality_v1(
        bundle,
        governed_route,
        &finality,
    )
}
/// Build the same TON request after a trusted caller has authenticated the
/// exact finality artifact carried by `bundle`.
#[must_use]
pub fn build_sccp_ton_groth16_bls12381_proof_request_from_structurally_bound_finality_v1(
    bundle: &TairaSccpMessageProofV1,
    governed_route: &SccpGovernedRouteV1,
    finality: &TairaBridgeFinalityProofV1,
) -> Option<SccpTonGroth16Bls12381ProofRequestV1> {
    if decode_taira_bridge_finality_proof(&bundle.finality_proof).as_ref() != Some(finality) {
        return None;
    }
    build_sccp_ton_groth16_bls12381_request_from_bound_finality_v1(bundle, governed_route, finality)
}
fn validate_sccp_ton_groth16_bls12381_request_with_bundle_structural_v1(
    request: &SccpTonGroth16Bls12381ProofRequestV1,
    decoded: &SccpDecodedCanonicalMessageBundleV1,
    finality: &TairaBridgeFinalityProofV1,
) -> Option<()> {
    if request.version != 1
        || request.backend != BridgeSccpDestinationProofBackendV1::TonGroth16Bls12381
        || request.source_network != SccpNetworkV1::SoraTaira
        || request.target_network != SccpNetworkV1::TonMainnet
        || !sccp_groth16_proof_request_public_inputs_are_valid(
            request.source_network,
            request.target_network,
            &request.public_inputs,
        )
        || request.public_inputs.target_domain != SCCP_DOMAIN_TON
        || request.proof_profile_commitment
            != sccp_ton_groth16_bls12381_proof_profile_commitment_v1()
        || structural_sccp_groth16_bls12381_verifying_key_bytes_v1(&request.verifying_key)
            .map(|bytes| sha256_bytes(&bytes))
            != Some(request.verifier_key_hash)
    {
        return None;
    }
    let policy = SccpOutboundProofPolicyV1 {
        version: 1,
        semantic_profile: request.semantic_proof_profile,
        sora_finality_anchor: request.sora_finality_anchor,
    };
    if policy.validate().is_err()
        || ton_semantic_circuit_commitment_v1(policy) != Some(request.verifier_circuit_hash)
        || policy.semantic_profile_hash().ok() != Some(request.semantic_proof_profile_hash)
        || policy.sora_finality_anchor_hash().ok() != Some(request.sora_finality_anchor_hash)
    {
        return None;
    }
    if !verify_taira_bridge_finality_proof_structure(finality)
        || !sccp_proof_request_bundle_binding_matches_public_inputs_with_finality(
            &request.public_inputs,
            SccpCanonicalMessageBundleBindingV1 {
                source_network: decoded.bundle.commitment.context.lane.source,
                target_network: decoded.bundle.commitment.context.lane.target,
                destination_binding_hash: decoded
                    .bundle
                    .commitment
                    .context
                    .destination_binding_hash,
                route_configuration_hash: decoded
                    .bundle
                    .commitment
                    .context
                    .route_configuration_hash,
                message_id: decoded.bundle.commitment.message_id,
                payload_hash: decoded.bundle.commitment.payload_hash,
                commitment_root: decoded.bundle.commitment_root,
                finality_proof: &decoded.bundle.finality_proof,
            },
            request.source_network,
            request.target_network,
            request.destination_binding_hash,
            request.route_configuration_hash,
            finality,
        )
        || !sccp_payload_matches_exact_xor_destination_route_v1(
            &decoded.bundle.payload,
            SCCP_DOMAIN_TON,
        )
    {
        return None;
    }
    let roles = [
        request.statement_hash,
        request.destination_binding_hash,
        request.route_configuration_hash,
        request.verifier_circuit_hash,
        request.verifier_key_hash,
        request.proof_profile_commitment,
        request.semantic_proof_profile_hash,
        request.sora_finality_anchor_hash,
    ];
    if roles.iter().any(|role| !h256_is_nonzero(role)) || hash_roles_alias(&roles) {
        return None;
    }
    if sccp_ton_groth16_bls12381_statement_hash_v1(request, &decoded.canonical_payload_bytes)
        != Some(request.statement_hash)
    {
        return None;
    }
    let expected_signals = sccp_groth16_bls12381_public_signal_words_v1(
        &request.public_inputs,
        SCCP_DOMAIN_SORA,
        request.statement_hash,
        request.destination_binding_hash,
        request.route_configuration_hash,
        request.sora_finality_anchor_hash,
    );
    if request.public_signals.words() != expected_signals
        || sccp_ton_groth16_bls12381_request_hash_v1(request, &decoded.canonical_payload_bytes)
            != Some(request.request_hash)
    {
        return None;
    }
    Some(())
}
fn validate_sccp_ton_groth16_bls12381_request_with_bundle_v1(
    request: &SccpTonGroth16Bls12381ProofRequestV1,
    decoded: &SccpDecodedCanonicalMessageBundleV1,
    finality: &TairaBridgeFinalityProofV1,
) -> Option<()> {
    validate_sccp_ton_groth16_bls12381_request_with_bundle_structural_v1(
        request, decoded, finality,
    )?;
    (sccp_groth16_bls12381_verifying_key_hash_v1(&request.verifying_key)
        == Some(request.verifier_key_hash))
    .then_some(())
}
fn validate_sccp_ton_groth16_bls12381_request_v1(
    request: &SccpTonGroth16Bls12381ProofRequestV1,
) -> Option<(TairaSccpMessageProofV1, Vec<u8>)> {
    let decoded =
        decode_canonical_taira_sccp_message_bundle_with_payload_v1(&request.bundle_bytes)?;
    let finality = decode_taira_bridge_finality_proof(&decoded.bundle.finality_proof)?;
    validate_sccp_ton_groth16_bls12381_request_with_bundle_v1(request, &decoded, &finality)?;
    Some((decoded.bundle, decoded.canonical_payload_bytes))
}
fn sccp_ton_groth16_bls12381_result_hash_v1(
    request_hash: H256,
    proof: &SccpGroth16Bls12381ProofV1,
) -> Option<H256> {
    let proof_bytes = structural_sccp_groth16_bls12381_proof_bytes_v1(proof)?;
    let mut preimage = Vec::with_capacity(32 + proof_bytes.len());
    preimage.extend_from_slice(&request_hash);
    preimage.extend_from_slice(&proof_bytes);
    Some(prefixed_sha256(
        SCCP_TON_GROTH16_PROOF_RESULT_PREFIX_V1,
        &preimage,
    ))
}
/// Pairing-verify and bind one raw TON proof to an exact canonical request.
#[must_use]
pub fn wrap_sccp_ton_groth16_bls12381_proof_result_v1(
    proof_bytes: &[u8],
    request: &SccpTonGroth16Bls12381ProofRequestV1,
) -> Option<SccpTonGroth16Bls12381ProofArtifactV1> {
    validate_sccp_ton_groth16_bls12381_request_v1(request)?;
    let proof = decode_sccp_groth16_bls12381_proof_bytes_v1(proof_bytes)?;
    if !verify_sccp_groth16_bls12381_pairing_v1(
        &proof,
        &request.public_signals,
        &request.verifying_key,
    ) {
        return None;
    }
    Some(SccpTonGroth16Bls12381ProofArtifactV1 {
        version: 1,
        request: request.clone(),
        result: SccpTonGroth16Bls12381ProofResultV1 {
            version: 1,
            request_hash: request.request_hash,
            result_hash: sccp_ton_groth16_bls12381_result_hash_v1(request.request_hash, &proof)?,
            proof,
        },
    })
}
fn sccp_ton_groth16_bls12381_artifact_result_is_canonical_v1(
    artifact: &SccpTonGroth16Bls12381ProofArtifactV1,
) -> bool {
    artifact.version == 1
        && artifact.result.version == 1
        && artifact.result.request_hash == artifact.request.request_hash
        && sccp_ton_groth16_bls12381_result_hash_v1(
            artifact.result.request_hash,
            &artifact.result.proof,
        ) == Some(artifact.result.result_hash)
}
fn sccp_ton_groth16_bls12381_artifact_matches_decoded_bundle_v1(
    artifact: &SccpTonGroth16Bls12381ProofArtifactV1,
    decoded: &SccpDecodedCanonicalMessageBundleV1,
    finality: &TairaBridgeFinalityProofV1,
) -> bool {
    sccp_ton_groth16_bls12381_artifact_result_is_canonical_v1(artifact)
        && validate_sccp_ton_groth16_bls12381_request_with_bundle_structural_v1(
            &artifact.request,
            decoded,
            finality,
        )
        .is_some()
}
fn sccp_ton_groth16_bls12381_artifact_is_self_canonical_v1(
    artifact: &SccpTonGroth16Bls12381ProofArtifactV1,
) -> bool {
    if !sccp_ton_groth16_bls12381_artifact_result_is_canonical_v1(artifact)
        || artifact.result.version != 1
        || validate_sccp_ton_groth16_bls12381_request_v1(&artifact.request).is_none()
    {
        return false;
    }
    verify_sccp_groth16_bls12381_pairing_v1(
        &artifact.result.proof,
        &artifact.request.public_signals,
        &artifact.request.verifying_key,
    )
}
/// Return whether an artifact is exactly reconstructed from governed TON route
/// history and contains a valid BLS12-381 Groth16 proof.
#[must_use]
pub fn sccp_ton_groth16_bls12381_artifact_matches_governed_route_v1(
    artifact: &SccpTonGroth16Bls12381ProofArtifactV1,
    bundle: &TairaSccpMessageProofV1,
    governed_route: &SccpGovernedRouteV1,
) -> bool {
    let Some(expected_request) =
        build_sccp_ton_groth16_bls12381_proof_request_from_governed_route_v1(
            bundle,
            governed_route,
        )
    else {
        return false;
    };
    artifact.request == expected_request
        && sccp_ton_groth16_bls12381_artifact_is_self_canonical_v1(artifact)
}
/// Encode one self-consistent TON proving request with canonical Norito framing.
#[must_use]
pub fn encode_canonical_sccp_ton_groth16_bls12381_proof_request_v1(
    request: &SccpTonGroth16Bls12381ProofRequestV1,
) -> Option<Vec<u8>> {
    validate_sccp_ton_groth16_bls12381_request_v1(request)?;
    let bytes = to_bytes(request).ok()?;
    (bytes.len() <= SCCP_GROTH16_BN254_MAX_ENCODED_ARTIFACT_BYTES_V1).then_some(bytes)
}
/// Decode exactly one canonical, bounded TON proving request.
#[must_use]
pub fn decode_canonical_sccp_ton_groth16_bls12381_proof_request_v1(
    bytes: &[u8],
) -> Option<SccpTonGroth16Bls12381ProofRequestV1> {
    decode_canonical_sccp_groth16_bn254_norito_v1(
        bytes,
        |request: &SccpTonGroth16Bls12381ProofRequestV1| {
            validate_sccp_ton_groth16_bls12381_request_v1(request).is_some()
        },
    )
}
/// Encode one concrete request using its curve-specific canonical wire type.
#[must_use]
pub fn encode_canonical_sccp_destination_proof_request_v1(
    request: &SccpDestinationProofRequestV1,
) -> Option<Vec<u8>> {
    match request {
        SccpDestinationProofRequestV1::Groth16Bn254(request) => {
            encode_canonical_sccp_groth16_bn254_proof_request_v1(request)
        }
        SccpDestinationProofRequestV1::Groth16Bls12381(request) => {
            encode_canonical_sccp_ton_groth16_bls12381_proof_request_v1(request)
        }
    }
}
/// Decode either member of the closed curve-specific proving-request family.
///
/// The returned enum is local classification only; `bytes` remain the exact
/// concrete request wire type so existing prover implementations do not need
/// to unwrap another protocol envelope.
#[must_use]
pub fn decode_canonical_sccp_destination_proof_request_v1(
    bytes: &[u8],
) -> Option<SccpDestinationProofRequestV1> {
    if let Some(request) = decode_canonical_sccp_groth16_bn254_proof_request_v1(bytes) {
        return Some(SccpDestinationProofRequestV1::Groth16Bn254(request));
    }
    decode_canonical_sccp_ton_groth16_bls12381_proof_request_v1(bytes)
        .map(SccpDestinationProofRequestV1::Groth16Bls12381)
}
/// Encode one pairing-valid TON result with canonical Norito framing.
#[must_use]
pub fn encode_canonical_sccp_ton_groth16_bls12381_proof_result_v1(
    result: &SccpTonGroth16Bls12381ProofResultV1,
) -> Option<Vec<u8>> {
    if result.version != 1
        || result.request_hash == [0; 32]
        || sccp_ton_groth16_bls12381_result_hash_v1(result.request_hash, &result.proof)
            != Some(result.result_hash)
    {
        return None;
    }
    let bytes = to_bytes(result).ok()?;
    (bytes.len() <= SCCP_GROTH16_BN254_MAX_ENCODED_ARTIFACT_BYTES_V1).then_some(bytes)
}
/// Encode one fully self-canonical TON proof artifact.
#[must_use]
pub fn encode_canonical_sccp_ton_groth16_bls12381_proof_artifact_v1(
    artifact: &SccpTonGroth16Bls12381ProofArtifactV1,
) -> Option<Vec<u8>> {
    if !sccp_ton_groth16_bls12381_artifact_is_self_canonical_v1(artifact) {
        return None;
    }
    let bytes = to_bytes(artifact).ok()?;
    (bytes.len() <= SCCP_GROTH16_BN254_MAX_ENCODED_ARTIFACT_BYTES_V1).then_some(bytes)
}
/// Decode one fully self-canonical TON proof artifact.
#[must_use]
pub fn decode_canonical_sccp_ton_groth16_bls12381_proof_artifact_v1(
    bytes: &[u8],
) -> Option<SccpTonGroth16Bls12381ProofArtifactV1> {
    decode_canonical_sccp_groth16_bn254_norito_v1(
        bytes,
        sccp_ton_groth16_bls12381_artifact_is_self_canonical_v1,
    )
}
/// Wrap one canonical TON artifact in the closed bridge-proof container.
#[must_use]
pub fn bridge_sccp_ton_destination_proof_v1(
    artifact: &SccpTonGroth16Bls12381ProofArtifactV1,
) -> Option<BridgeSccpDestinationProofV1> {
    let encoded_artifact = encode_canonical_sccp_ton_groth16_bls12381_proof_artifact_v1(artifact)?;
    let proof = BridgeSccpDestinationProofV1 {
        backend: BridgeSccpDestinationProofBackendV1::TonGroth16Bls12381,
        route_configuration_hash: artifact.request.route_configuration_hash,
        encoded_artifact,
    };
    proof
        .is_well_formed_for(
            artifact.request.destination_binding_hash,
            artifact.result.result_hash,
        )
        .then_some(proof)
}
/// Decode a TON bridge-proof container and require exact agreement between
/// its outer route/backend fields and the canonical inner artifact.
fn decode_bridge_sccp_ton_destination_proof_framing_v1(
    proof: &BridgeSccpDestinationProofV1,
) -> Option<SccpTonGroth16Bls12381ProofArtifactV1> {
    if proof.backend != BridgeSccpDestinationProofBackendV1::TonGroth16Bls12381 {
        return None;
    }
    count_sccp_destination_artifact_decode_v1();
    let artifact = decode_canonical_sccp_groth16_bn254_norito_v1(
        &proof.encoded_artifact,
        sccp_ton_groth16_bls12381_artifact_result_is_canonical_v1,
    )?;
    (proof.route_configuration_hash == artifact.request.route_configuration_hash
        && proof.is_well_formed_for(
            artifact.request.destination_binding_hash,
            artifact.result.result_hash,
        ))
    .then_some(artifact)
}
/// Decode a TON bridge-proof container and require exact agreement between
/// its outer route/backend fields and the canonical inner artifact.
#[must_use]
pub fn decode_bridge_sccp_ton_destination_proof_v1(
    proof: &BridgeSccpDestinationProofV1,
) -> Option<SccpTonGroth16Bls12381ProofArtifactV1> {
    let artifact = decode_bridge_sccp_ton_destination_proof_framing_v1(proof)?;
    sccp_ton_groth16_bls12381_artifact_is_self_canonical_v1(&artifact).then_some(artifact)
}
#[derive(Clone, Debug)]
struct TonBocCellV1 {
    data: Vec<u8>,
    refs: Vec<usize>,
}
fn ton_boc_uint_width_v1(value: usize) -> Option<usize> {
    match value {
        0..=0xff => Some(1),
        0x100..=0xffff => Some(2),
        0x1_0000..=0xff_ffff => Some(3),
        0x100_0000..=0xffff_ffff => Some(4),
        _ => None,
    }
}
fn push_ton_boc_uint_v1(out: &mut Vec<u8>, value: usize, width: usize) -> Option<()> {
    if width == 0 || width > 4 || value >= (1usize << (width * 8)) {
        return None;
    }
    let encoded = u32::try_from(value).ok()?.to_be_bytes();
    out.extend_from_slice(&encoded[4 - width..]);
    Some(())
}
fn encode_ton_boc_cells_v1(cells: &[TonBocCellV1]) -> Option<Vec<u8>> {
    if cells.is_empty() || cells.len() > u16::MAX.into() {
        return None;
    }
    // The header stores `cells.len()`, while references store indices through
    // `cells.len() - 1`; size the field for both rather than only for indices.
    let size_bytes = ton_boc_uint_width_v1(cells.len())?;
    if size_bytes > 4 {
        return None;
    }
    let mut serialized_cells = Vec::new();
    for (cell_index, cell) in cells.iter().enumerate() {
        if cell.data.len() > 127
            || cell.refs.len() > 4
            || cell
                .refs
                .iter()
                .any(|reference| *reference <= cell_index || *reference >= cells.len())
        {
            return None;
        }
        serialized_cells.push(u8::try_from(cell.refs.len()).ok()?);
        serialized_cells.push(u8::try_from(cell.data.len().checked_mul(2)?).ok()?);
        serialized_cells.extend_from_slice(&cell.data);
        for reference in &cell.refs {
            push_ton_boc_uint_v1(&mut serialized_cells, *reference, size_bytes)?;
        }
    }
    let offset_bytes = ton_boc_uint_width_v1(serialized_cells.len())?;
    let mut out = Vec::with_capacity(serialized_cells.len() + 32);
    out.extend_from_slice(&SCCP_TON_BOC_MAGIC_V1);
    // No index, CRC32C, cache bits, or custom flags; low three bits carry
    // the cell-index width.
    out.push(u8::try_from(size_bytes).ok()?);
    out.push(u8::try_from(offset_bytes).ok()?);
    push_ton_boc_uint_v1(&mut out, cells.len(), size_bytes)?;
    push_ton_boc_uint_v1(&mut out, 1, size_bytes)?;
    push_ton_boc_uint_v1(&mut out, 0, size_bytes)?;
    push_ton_boc_uint_v1(&mut out, serialized_cells.len(), offset_bytes)?;
    push_ton_boc_uint_v1(&mut out, 0, size_bytes)?;
    out.extend_from_slice(&serialized_cells);
    Some(out)
}
/// Build the canonical Bag-of-Cells representation of one
/// `SccpFinalizeFromTaira` internal-message body.
///
/// The root stores the opcode, query id, schema version, message id, and
/// statement hash. Its references use the exact contract TL-B topology:
/// four linked public-signal cells, a proof root referencing `A/B/C`, and a
/// payload root referencing the fixed `50/100/100/remainder` segmentation.
#[must_use]
pub fn encode_sccp_ton_finalize_from_taira_body_boc_v1(
    query_id: u64,
    request: &SccpTonGroth16Bls12381ProofRequestV1,
    proof: &SccpGroth16Bls12381ProofV1,
    canonical_payload_bytes: &[u8],
) -> Option<Vec<u8>> {
    validate_sccp_ton_groth16_bls12381_request_v1(request)?;
    if !verify_sccp_groth16_bls12381_pairing_v1(
        proof,
        &request.public_signals,
        &request.verifying_key,
    ) {
        return None;
    }
    encode_sccp_ton_finalize_from_taira_body_boc_after_verification_v1(
        query_id,
        request,
        proof,
        canonical_payload_bytes,
    )
}
fn encode_sccp_ton_finalize_from_taira_body_boc_after_verification_v1(
    query_id: u64,
    request: &SccpTonGroth16Bls12381ProofRequestV1,
    proof: &SccpGroth16Bls12381ProofV1,
    canonical_payload_bytes: &[u8],
) -> Option<Vec<u8>> {
    if canonical_payload_bytes.len() < SCCP_TON_CANONICAL_PAYLOAD_HEADER_BYTES_V1
        || canonical_payload_bytes.len() > SCCP_TON_DESTINATION_MAX_PAYLOAD_BYTES_V1
        || payload_hash(canonical_payload_bytes) != request.public_inputs.payload_hash
    {
        return None;
    }
    let proof_bytes = canonical_sccp_groth16_bls12381_proof_bytes_v1(proof)?;
    let signals = request.public_signals.words();
    let mut root_data = Vec::with_capacity(78);
    root_data.extend_from_slice(&SCCP_TON_FINALIZE_FROM_TAIRA_OPCODE_V1.to_be_bytes());
    root_data.extend_from_slice(&query_id.to_be_bytes());
    root_data.extend_from_slice(&1_u16.to_be_bytes());
    root_data.extend_from_slice(&request.public_inputs.message_id);
    root_data.extend_from_slice(&request.statement_hash);
    let signal_cell = |range: core::ops::Range<usize>, next: Option<usize>| {
        let mut data = Vec::with_capacity(range.len() * 32);
        for signal in &signals[range] {
            data.extend_from_slice(signal);
        }
        TonBocCellV1 {
            data,
            refs: next.into_iter().collect(),
        }
    };
    let remainder = &canonical_payload_bytes[SCCP_TON_CANONICAL_PAYLOAD_HEADER_BYTES_V1..];
    let first_end = remainder
        .len()
        .min(SCCP_TON_CANONICAL_PAYLOAD_CHUNK_BYTES_V1);
    let second_end = remainder
        .len()
        .min(2 * SCCP_TON_CANONICAL_PAYLOAD_CHUNK_BYTES_V1);
    let cells = vec![
        TonBocCellV1 {
            data: root_data,
            refs: vec![1, 5, 9],
        },
        signal_cell(0..3, Some(2)),
        signal_cell(3..6, Some(3)),
        signal_cell(6..9, Some(4)),
        signal_cell(9..11, None),
        TonBocCellV1 {
            data: Vec::new(),
            refs: vec![6, 7, 8],
        },
        TonBocCellV1 {
            data: proof_bytes[..48].to_vec(),
            refs: Vec::new(),
        },
        TonBocCellV1 {
            data: proof_bytes[48..144].to_vec(),
            refs: Vec::new(),
        },
        TonBocCellV1 {
            data: proof_bytes[144..].to_vec(),
            refs: Vec::new(),
        },
        TonBocCellV1 {
            data: Vec::new(),
            refs: vec![10, 11, 12, 13],
        },
        TonBocCellV1 {
            data: canonical_payload_bytes[..SCCP_TON_CANONICAL_PAYLOAD_HEADER_BYTES_V1].to_vec(),
            refs: Vec::new(),
        },
        TonBocCellV1 {
            data: remainder[..first_end].to_vec(),
            refs: Vec::new(),
        },
        TonBocCellV1 {
            data: remainder[first_end..second_end].to_vec(),
            refs: Vec::new(),
        },
        TonBocCellV1 {
            data: remainder[second_end..].to_vec(),
            refs: Vec::new(),
        },
    ];
    encode_ton_boc_cells_v1(&cells)
}
fn sccp_ton_payload_amount_to_jetton_base_units_v1(
    amount: u128,
    deployment: &SccpTonDestinationDeploymentV1,
) -> Option<u128> {
    if deployment.taira_to_token_multiplier != SCCP_V1_TAIRA_TO_TON_TOKEN_MULTIPLIER {
        return None;
    }
    let amount = amount.checked_mul(u128::from(deployment.taira_to_token_multiplier))?;
    // TEP-74 `coins` is VarUInteger 16 and therefore carries at most 15
    // value bytes. The per-call amount must also remain within the exact
    // immutable cap authenticated by route governance.
    (deployment.max_wrapped_supply != 0
        && deployment.max_wrapped_supply <= SCCP_V1_TON_MAX_COINS
        && amount != 0
        && amount <= deployment.max_wrapped_supply)
        .then_some(amount)
}
/// Verify a closed TON destination proof against exact governed history and
/// explicitly trusted Taira finality, then derive the canonical internal-message body BOC.
#[must_use]
pub fn verify_sccp_ton_destination_proof_v1(
    proof: &BridgeSccpDestinationProofV1,
    bundle: &TairaSccpMessageProofV1,
    governed_route: &SccpGovernedRouteV1,
    trusted_finality: &TairaBridgeFinalityProofV1,
    query_id: u64,
) -> Option<SccpVerifiedTonDestinationCallV1> {
    let embedded_finality = decode_taira_bridge_finality_proof(&bundle.finality_proof)?;
    if embedded_finality != *trusted_finality {
        return None;
    }
    let artifact = decode_bridge_sccp_ton_destination_proof_v1(proof)?;
    if !sccp_ton_groth16_bls12381_artifact_matches_governed_route_v1(
        &artifact,
        bundle,
        governed_route,
    ) {
        return None;
    }
    let SccpDestinationDeploymentV1::Ton(deployment) = governed_route.destination else {
        return None;
    };
    let SccpPayloadV1::Transfer(transfer) = &bundle.payload;
    let recipient = decode_sccp_ton_account36_v1(&transfer.recipient)?;
    let amount = sccp_ton_payload_amount_to_jetton_base_units_v1(transfer.amount, &deployment)?;
    let canonical_payload_bytes = canonical_sccp_payload_bytes(&bundle.payload).ok()?;
    let proof_bytes = canonical_sccp_groth16_bls12381_proof_bytes_v1(&artifact.result.proof)?;
    let internal_message_body_boc = encode_sccp_ton_finalize_from_taira_body_boc_v1(
        query_id,
        &artifact.request,
        &artifact.result.proof,
        &canonical_payload_bytes,
    )?;
    if !verify_taira_bridge_finality_proof_cryptographic(trusted_finality) {
        return None;
    }
    Some(SccpVerifiedTonDestinationCallV1 {
        version: 1,
        backend: BridgeSccpDestinationProofBackendV1::TonGroth16Bls12381,
        network: governed_route.lane_id.source,
        route_address: deployment.route_address,
        query_id,
        route_revision: transfer.route_revision,
        recipient,
        amount,
        destination_binding_hash: artifact.request.destination_binding_hash,
        route_configuration_hash: artifact.request.route_configuration_hash,
        public_signals: artifact.request.public_signals,
        statement_hash: artifact.request.statement_hash,
        request_hash: artifact.request.request_hash,
        proof_bytes,
        canonical_payload_bytes,
        internal_message_body_boc,
        bundle: bundle.clone(),
    })
}
fn sccp_governed_route_groth16_material_v1(
    governed_route: &SccpGovernedRouteV1,
) -> Option<(
    SccpGroth16Bn254VerifyingKeyV1,
    H256,
    SccpOutboundProofPolicyV1,
)> {
    let (verifying_key, verifier_key_hash, policy) = match governed_route.destination {
        SccpDestinationDeploymentV1::Evm(deployment) => (
            deployment.verifying_key,
            deployment.verifier_key_hash,
            deployment.outbound_proof_policy,
        ),
        SccpDestinationDeploymentV1::Tron(deployment) => (
            deployment.verifying_key,
            deployment.verifier_key_hash,
            deployment.outbound_proof_policy,
        ),
        // TON is governed by a distinct BLS12-381 proof profile and carries a
        // commitment to its verifier key, not a BN254 key. It must never enter
        // this BN254 material path.
        SccpDestinationDeploymentV1::Ton(_) => return None,
    };
    (sccp_groth16_bn254_verifying_key_hash_v1(&verifying_key) == Some(verifier_key_hash)
        && policy.validate().is_ok())
    .then_some((verifying_key, verifier_key_hash, policy))
}
/// Return whether a governed route carries a canonical, subgroup-checked
/// Groth16 key whose Solidity hash equals the deployment commitment.
pub fn sccp_governed_route_groth16_key_is_valid_v1(governed_route: &SccpGovernedRouteV1) -> bool {
    sccp_governed_route_groth16_material_v1(governed_route).is_some()
}
fn sccp_governed_groth16_route_matches_bundle_v1(
    bundle: &TairaSccpMessageProofV1,
    governed_route: &SccpGovernedRouteV1,
) -> bool {
    let route = governed_route;
    if route.validate().is_err()
        || route.lane_id.target != SccpNetworkV1::SoraTaira
        || bundle.commitment.context.lane
            != (SccpLaneIdV1 {
                source: route.lane_id.target,
                target: route.lane_id.source,
            })
        || route.destination_binding_hash().ok()
            != Some(bundle.commitment.context.destination_binding_hash)
        || route.route_configuration_hash().ok()
            != Some(bundle.commitment.context.route_configuration_hash)
        || !sccp_governed_route_groth16_key_is_valid_v1(route)
        || !sccp_payload_matches_exact_xor_destination_route_v1(
            &bundle.payload,
            route.lane_id.source.domain_id(),
        )
    {
        return false;
    }
    let SccpPayloadV1::Transfer(payload) = &bundle.payload;
    payload.route_revision == route.revision
        && payload.route_id == route.route_id.as_bytes()
        && payload.asset_id == route.asset_key.as_bytes()
}
/// Build a canonical query-free Groth16 request from a bundle and resolved governed route.
///
/// No request field chooses deployment material. Core or Torii must resolve
/// `governed_route` by the bundle's committed destination binding before this
/// function is called. This entry point accepts an untrusted bundle and therefore
/// performs the one required BLS verification before constructing the request.
pub fn build_sccp_groth16_bn254_proof_request_from_governed_route_v1(
    bundle: &TairaSccpMessageProofV1,
    governed_route: &SccpGovernedRouteV1,
) -> Option<SccpGroth16Bn254ProofRequestV1> {
    let finality =
        verified_sccp_message_taira_finality_proof_cryptographically_self_consistent(bundle)?;
    build_sccp_groth16_bn254_proof_request_from_bound_finality_v1(bundle, governed_route, &finality)
}
/// Build a canonical query-free Groth16 request after a trusted caller has already
/// authenticated the bundle's exact finality artifact.
///
/// This is a structural assembly boundary, not a finality trust boundary. The supplied
/// proof must be the exact canonical proof encoded by `bundle`, and the message, Merkle,
/// header, route, and request bindings are still checked. No BLS operation is performed;
/// Core calls this only after binding the proof to its `VerifiedV2FinalityArtifact` marker.
pub fn build_sccp_groth16_bn254_proof_request_from_structurally_bound_finality_v1(
    bundle: &TairaSccpMessageProofV1,
    governed_route: &SccpGovernedRouteV1,
    finality: &TairaBridgeFinalityProofV1,
) -> Option<SccpGroth16Bn254ProofRequestV1> {
    if decode_taira_bridge_finality_proof(&bundle.finality_proof).as_ref() != Some(finality) {
        return None;
    }
    build_sccp_groth16_bn254_proof_request_from_bound_finality_v1(bundle, governed_route, finality)
}
/// Build the exact curve-specific destination proving request after a trusted
/// caller has authenticated the bundle's finality artifact.
///
/// The governed destination variant selects the only admissible curve. No
/// caller-provided backend string participates in dispatch.
#[must_use]
pub fn build_sccp_destination_proof_request_from_structurally_bound_finality_v1(
    bundle: &TairaSccpMessageProofV1,
    governed_route: &SccpGovernedRouteV1,
    finality: &TairaBridgeFinalityProofV1,
) -> Option<SccpDestinationProofRequestV1> {
    match governed_route.destination {
        SccpDestinationDeploymentV1::Ton(_) => {
            build_sccp_ton_groth16_bls12381_proof_request_from_structurally_bound_finality_v1(
                bundle,
                governed_route,
                finality,
            )
            .map(SccpDestinationProofRequestV1::Groth16Bls12381)
        }
        SccpDestinationDeploymentV1::Evm(_) | SccpDestinationDeploymentV1::Tron(_) => {
            build_sccp_groth16_bn254_proof_request_from_structurally_bound_finality_v1(
                bundle,
                governed_route,
                finality,
            )
            .map(SccpDestinationProofRequestV1::Groth16Bn254)
        }
    }
}
fn build_sccp_groth16_bn254_proof_request_from_bound_finality_v1(
    bundle: &TairaSccpMessageProofV1,
    governed_route: &SccpGovernedRouteV1,
    finality: &TairaBridgeFinalityProofV1,
) -> Option<SccpGroth16Bn254ProofRequestV1> {
    if !sccp_governed_groth16_route_matches_bundle_v1(bundle, governed_route) {
        return None;
    }
    let public_inputs = sccp_message_public_inputs_with_finality(bundle, finality)?;
    let canonical_payload_bytes = canonical_sccp_payload_bytes(&bundle.payload).ok()?;
    let bundle_bytes = canonical_taira_sccp_message_bundle_bytes_checked(bundle)?;
    let destination_binding_hash = governed_route.destination_binding_hash().ok()?;
    let route_configuration_hash = governed_route.route_configuration_hash().ok()?;
    let (verifying_key, expected_verifier_key_hash, outbound_proof_policy) =
        sccp_governed_route_groth16_material_v1(governed_route)?;
    let backend = match governed_route.destination {
        SccpDestinationDeploymentV1::Evm(_) => BridgeSccpDestinationProofBackendV1::EvmGroth16Bn254,
        SccpDestinationDeploymentV1::Tron(_) => {
            BridgeSccpDestinationProofBackendV1::TronGroth16Bn254
        }
        SccpDestinationDeploymentV1::Ton(_) => return None,
    };
    build_sccp_groth16_bn254_proof_request(&SccpGroth16Bn254ProofRequestBuildContextV1 {
        backend,
        source_network: governed_route.lane_id.target,
        target_network: governed_route.lane_id.source,
        public_inputs: &public_inputs,
        canonical_payload_bytes: &canonical_payload_bytes,
        bundle_bytes: &bundle_bytes,
        bundle_binding: SccpCanonicalMessageBundleBindingV1 {
            source_network: bundle.commitment.context.lane.source,
            target_network: bundle.commitment.context.lane.target,
            destination_binding_hash: bundle.commitment.context.destination_binding_hash,
            route_configuration_hash: bundle.commitment.context.route_configuration_hash,
            message_id: bundle.commitment.message_id,
            payload_hash: bundle.commitment.payload_hash,
            commitment_root: bundle.commitment_root,
            finality_proof: &bundle.finality_proof,
        },
        finality,
        destination_binding_hash,
        route_configuration_hash,
        verifying_key: &verifying_key,
        expected_verifier_key_hash,
        outbound_proof_policy,
    })
}
/// Return whether a request is exactly the canonical request for a bundle and governed route.
pub fn sccp_groth16_bn254_proof_request_matches_governed_route_v1(
    request: &SccpGroth16Bn254ProofRequestV1,
    bundle: &TairaSccpMessageProofV1,
    governed_route: &SccpGovernedRouteV1,
) -> bool {
    build_sccp_groth16_bn254_proof_request_from_governed_route_v1(bundle, governed_route).as_ref()
        == Some(request)
}
fn sccp_groth16_bn254_proof_request_header_is_canonical_v1(
    request: &SccpGroth16Bn254ProofRequestV1,
    expected_backend: BridgeSccpDestinationProofBackendV1,
) -> bool {
    let outbound_proof_policy = SccpOutboundProofPolicyV1 {
        version: 1,
        semantic_profile: request.semantic_proof_profile,
        sora_finality_anchor: request.sora_finality_anchor,
    };
    request.version == 1
        && request.backend == expected_backend
        && sccp_destination_proof_backend_supports_network_v1(
            request.backend,
            request.target_network,
        )
        && sccp_groth16_proof_request_public_inputs_are_valid(
            request.source_network,
            request.target_network,
            &request.public_inputs,
        )
        && h256_is_nonzero(&request.statement_hash)
        && h256_is_nonzero(&request.verifier_key_hash)
        && h256_is_nonzero(&request.destination_binding_hash)
        && h256_is_nonzero(&request.route_configuration_hash)
        && h256_is_nonzero(&request.semantic_proof_profile_hash)
        && h256_is_nonzero(&request.sora_finality_anchor_hash)
        && outbound_proof_policy.validate().is_ok()
        && sccp_semantic_proof_profile_hash_v1(request.semantic_proof_profile).ok()
            == Some(request.semantic_proof_profile_hash)
        && sccp_sora_finality_anchor_hash_v1(request.sora_finality_anchor).ok()
            == Some(request.sora_finality_anchor_hash)
        && sccp_groth16_bn254_request_hash_roles_are_distinct_v1(
            request.statement_hash,
            request.destination_binding_hash,
            request.route_configuration_hash,
            request.verifier_key_hash,
            request.semantic_proof_profile_hash,
            request.sora_finality_anchor_hash,
        )
        && sccp_groth16_bn254_verifying_key_hash_v1(&request.verifying_key)
            == Some(request.verifier_key_hash)
}
struct ValidatedSccpGroth16Bn254ProofRequestV1<'a> {
    request: &'a SccpGroth16Bn254ProofRequestV1,
    public_signal_words: [H256; 11],
}
fn validate_sccp_groth16_bn254_proof_request_with_bundle_v1<'a>(
    request: &'a SccpGroth16Bn254ProofRequestV1,
    expected_backend: BridgeSccpDestinationProofBackendV1,
    bundle: SccpCanonicalMessageBundleBindingV1<'_>,
    canonical_payload_bytes: &[u8],
    finality: &TairaBridgeFinalityProofV1,
) -> Option<ValidatedSccpGroth16Bn254ProofRequestV1<'a>> {
    if !sccp_groth16_bn254_proof_request_header_is_canonical_v1(request, expected_backend)
        || !sccp_proof_request_bundle_binding_matches_public_inputs_with_finality(
            &request.public_inputs,
            bundle,
            request.source_network,
            request.target_network,
            request.destination_binding_hash,
            request.route_configuration_hash,
            finality,
        )
    {
        return None;
    }
    let statement_hash = sccp_groth16_bn254_statement_hash_v1(request, canonical_payload_bytes)?;
    if request.statement_hash != statement_hash {
        return None;
    }
    let public_signal_words = sccp_groth16_bn254_public_signal_words(
        &request.public_inputs,
        request.source_network.domain_id(),
        request.statement_hash,
        request.destination_binding_hash,
        request.route_configuration_hash,
        request.sora_finality_anchor_hash,
    );
    if sccp_groth16_bn254_proof_request_hash(request, canonical_payload_bytes, &public_signal_words)
        != Some(request.request_hash)
    {
        return None;
    }
    Some(ValidatedSccpGroth16Bn254ProofRequestV1 {
        request,
        public_signal_words,
    })
}
fn validate_sccp_groth16_bn254_proof_request_with_decoder_v1<F>(
    request: &SccpGroth16Bn254ProofRequestV1,
    expected_backend: BridgeSccpDestinationProofBackendV1,
    decode_bundle: F,
) -> Option<ValidatedSccpGroth16Bn254ProofRequestV1<'_>>
where
    F: FnOnce(&[u8]) -> Option<SccpCanonicalMessageBundleSummaryV1>,
{
    let bundle = decode_bundle(&request.bundle_bytes)?;
    let finality = decode_taira_bridge_finality_proof(bundle.binding().finality_proof)?;
    if !verify_taira_bridge_finality_proof_structure(&finality) {
        return None;
    }
    validate_sccp_groth16_bn254_proof_request_with_bundle_v1(
        request,
        expected_backend,
        bundle.binding(),
        &bundle.canonical_payload_bytes,
        &finality,
    )
}
fn validate_sccp_groth16_bn254_proof_request_v1(
    request: &SccpGroth16Bn254ProofRequestV1,
    expected_backend: BridgeSccpDestinationProofBackendV1,
) -> Option<ValidatedSccpGroth16Bn254ProofRequestV1<'_>> {
    validate_sccp_groth16_bn254_proof_request_with_decoder_v1(
        request,
        expected_backend,
        decode_canonical_taira_sccp_message_bundle_summary,
    )
}
fn sccp_groth16_bn254_proof_request_is_canonical(
    request: &SccpGroth16Bn254ProofRequestV1,
    expected_backend: BridgeSccpDestinationProofBackendV1,
) -> bool {
    validate_sccp_groth16_bn254_proof_request_v1(request, expected_backend).is_some()
}
fn sccp_groth16_bn254_request_hash_roles_are_distinct_v1(
    statement_hash: H256,
    destination_binding_hash: H256,
    route_configuration_hash: H256,
    verifier_key_hash: H256,
    semantic_proof_profile_hash: H256,
    sora_finality_anchor_hash: H256,
) -> bool {
    hash_roles_are_distinct([
        statement_hash,
        destination_binding_hash,
        route_configuration_hash,
        verifier_key_hash,
        semantic_proof_profile_hash,
        sora_finality_anchor_hash,
    ])
}
fn sccp_groth16_proof_request_public_inputs_are_valid(
    source_network: SccpNetworkV1,
    target_network: SccpNetworkV1,
    public_inputs: &SccpMessagePublicInputsV1,
) -> bool {
    public_inputs.version == 1
        && source_network == SccpNetworkV1::SoraTaira
        && target_network.is_external()
        && source_network != target_network
        && public_inputs.target_domain == target_network.domain_id()
        && h256_is_nonzero(&public_inputs.message_id)
        && h256_is_nonzero(&public_inputs.payload_hash)
        && h256_is_nonzero(&public_inputs.commitment_root)
        && public_inputs.finality_height != 0
        && h256_is_nonzero(&public_inputs.finality_block_hash)
}
fn sccp_groth16_bn254_proof_result_hash(request_hash: H256, proof_bytes: &[u8]) -> H256 {
    let mut preimage = Vec::with_capacity(32 + proof_bytes.len());
    preimage.extend_from_slice(&request_hash);
    preimage.extend_from_slice(proof_bytes);
    prefixed_blake2b(SCCP_GROTH16_PROOF_RESULT_PREFIX_V1, &preimage)
}
fn bind_sccp_groth16_bn254_proof_result_v1(
    proof_bytes: &[u8],
    validated: &ValidatedSccpGroth16Bn254ProofRequestV1<'_>,
) -> Option<SccpGroth16Bn254ProofArtifactV1> {
    if !verify_sccp_groth16_bn254_proof_against_validated_request_v1(validated, proof_bytes) {
        return None;
    }
    let request = validated.request;
    Some(SccpGroth16Bn254ProofArtifactV1 {
        version: 1,
        request: request.clone(),
        result: SccpGroth16Bn254ProofResultV1 {
            version: 1,
            request_hash: request.request_hash,
            proof_bytes: proof_bytes.to_vec(),
            result_hash: sccp_groth16_bn254_proof_result_hash(request.request_hash, proof_bytes),
        },
    })
}
fn wrap_sccp_groth16_bn254_proof_result(
    proof_bytes: &[u8],
    request: &SccpGroth16Bn254ProofRequestV1,
    expected_backend: BridgeSccpDestinationProofBackendV1,
) -> Option<SccpGroth16Bn254ProofArtifactV1> {
    let validated = validate_sccp_groth16_bn254_proof_request_v1(request, expected_backend)?;
    bind_sccp_groth16_bn254_proof_result_v1(proof_bytes, &validated)
}
/// Validate and bind raw EVM Groth16 proof bytes to their exact proving request.
pub fn wrap_sccp_evm_groth16_bn254_proof_result(
    proof_bytes: &[u8],
    request: &SccpEvmGroth16Bn254ProofRequestV1,
) -> Option<SccpEvmGroth16Bn254ProofArtifactV1> {
    wrap_sccp_groth16_bn254_proof_result(
        proof_bytes,
        request,
        BridgeSccpDestinationProofBackendV1::EvmGroth16Bn254,
    )
}
/// Wrap BSC mainnet Groth16 proof bytes returned by an external browser or app prover.
pub fn wrap_sccp_bsc_mainnet_groth16_bn254_proof_result(
    proof_bytes: &[u8],
    request: &SccpEvmGroth16Bn254ProofRequestV1,
) -> Option<SccpEvmGroth16Bn254ProofArtifactV1> {
    if request.target_network != SccpNetworkV1::BscMainnet
        || request.public_inputs.target_domain != SCCP_DOMAIN_BSC
    {
        return None;
    }
    wrap_sccp_evm_groth16_bn254_proof_result(proof_bytes, request)
}
/// Validate and bind raw TRON Groth16 proof bytes to their exact proving request.
pub fn wrap_sccp_tron_groth16_bn254_proof_result(
    proof_bytes: &[u8],
    request: &SccpTronGroth16Bn254ProofRequestV1,
) -> Option<SccpTronGroth16Bn254ProofArtifactV1> {
    wrap_sccp_groth16_bn254_proof_result(
        proof_bytes,
        request,
        BridgeSccpDestinationProofBackendV1::TronGroth16Bn254,
    )
}
fn sccp_groth16_bn254_proof_request_is_self_canonical(
    request: &SccpGroth16Bn254ProofRequestV1,
) -> bool {
    sccp_groth16_bn254_proof_request_is_canonical(request, request.backend)
}
fn sccp_groth16_bn254_proof_result_is_structurally_valid(
    result: &SccpGroth16Bn254ProofResultV1,
) -> bool {
    decode_sccp_groth16_bn254_proof_result_v1(result).is_some()
}
fn decode_sccp_groth16_bn254_proof_result_v1(
    result: &SccpGroth16Bn254ProofResultV1,
) -> Option<SccpEvmGroth16Bn254ProofV1> {
    let proof = decode_sccp_evm_groth16_bn254_proof_bytes(&result.proof_bytes)?;
    (result.version == 1
        && h256_is_nonzero(&result.request_hash)
        && h256_is_nonzero(&result.result_hash)
        && proof.version == 1
        && proof.source_domain == SCCP_DOMAIN_SORA
        && h256_is_nonzero(&proof.message_id)
        && h256_is_nonzero(&proof.commitment_root)
        && result.result_hash
            == sccp_groth16_bn254_proof_result_hash(result.request_hash, &result.proof_bytes))
    .then_some(proof)
}
fn sccp_groth16_bn254_proof_artifact_is_self_canonical_with_decoder_v1<F>(
    artifact: &SccpGroth16Bn254ProofArtifactV1,
    decode_bundle: F,
) -> bool
where
    F: FnOnce(&[u8]) -> Option<SccpCanonicalMessageBundleSummaryV1>,
{
    if artifact.version != 1 || artifact.result.request_hash != artifact.request.request_hash {
        return false;
    }
    let Some(validated) = validate_sccp_groth16_bn254_proof_request_with_decoder_v1(
        &artifact.request,
        artifact.request.backend,
        decode_bundle,
    ) else {
        return false;
    };
    let expected =
        bind_sccp_groth16_bn254_proof_result_v1(&artifact.result.proof_bytes, &validated);
    expected.as_ref() == Some(artifact)
}
fn sccp_groth16_bn254_proof_artifact_is_self_canonical(
    artifact: &SccpGroth16Bn254ProofArtifactV1,
) -> bool {
    sccp_groth16_bn254_proof_artifact_is_self_canonical_with_decoder_v1(
        artifact,
        decode_canonical_taira_sccp_message_bundle_summary,
    )
}
fn decode_canonical_sccp_groth16_bn254_norito_framing_v1<T>(bytes: &[u8]) -> Option<T>
where
    T: for<'de> norito::NoritoDeserialize<'de> + norito::NoritoSerialize,
{
    if !preflight_uncompressed_norito_frame(bytes, SCCP_GROTH16_BN254_MAX_ENCODED_ARTIFACT_BYTES_V1)
    {
        return None;
    }
    let decoded = norito::decode_from_bytes::<T>(bytes).ok()?;
    if to_bytes(&decoded).ok()?.as_slice() != bytes {
        return None;
    }
    Some(decoded)
}
fn decode_canonical_sccp_groth16_bn254_norito_v1<T>(
    bytes: &[u8],
    validate: fn(&T) -> bool,
) -> Option<T>
where
    T: for<'de> norito::NoritoDeserialize<'de> + norito::NoritoSerialize,
{
    let decoded = decode_canonical_sccp_groth16_bn254_norito_framing_v1(bytes)?;
    validate(&decoded).then_some(decoded)
}
fn decode_canonical_sccp_groth16_bn254_json_v1<T>(json: &str, validate: fn(&T) -> bool) -> Option<T>
where
    T: norito::json::JsonDeserialize + norito::json::JsonSerialize,
{
    if json.is_empty() || json.len() > SCCP_GROTH16_BN254_MAX_JSON_ARTIFACT_BYTES_V1 {
        return None;
    }
    let decoded = norito::json::from_str::<T>(json).ok()?;
    if !validate(&decoded) || norito::json::to_json(&decoded).ok()?.as_str() != json {
        return None;
    }
    Some(decoded)
}
/// Encode one self-consistent Groth16 request with canonical Norito framing.
pub fn encode_canonical_sccp_groth16_bn254_proof_request_v1(
    request: &SccpGroth16Bn254ProofRequestV1,
) -> Option<Vec<u8>> {
    if !sccp_groth16_bn254_proof_request_is_self_canonical(request) {
        return None;
    }
    let bytes = to_bytes(request).ok()?;
    (bytes.len() <= SCCP_GROTH16_BN254_MAX_ENCODED_ARTIFACT_BYTES_V1).then_some(bytes)
}
/// Decode exactly one canonical, size-bounded Groth16 request.
pub fn decode_canonical_sccp_groth16_bn254_proof_request_v1(
    bytes: &[u8],
) -> Option<SccpGroth16Bn254ProofRequestV1> {
    decode_canonical_sccp_groth16_bn254_norito_v1(
        bytes,
        sccp_groth16_bn254_proof_request_is_self_canonical,
    )
}
/// Decode exactly one canonical, size-bounded JSON Groth16 request.
pub fn decode_canonical_sccp_groth16_bn254_proof_request_json_v1(
    json: &str,
) -> Option<SccpGroth16Bn254ProofRequestV1> {
    decode_canonical_sccp_groth16_bn254_json_v1(
        json,
        sccp_groth16_bn254_proof_request_is_self_canonical,
    )
}
/// Encode one structurally valid minimal Groth16 result with canonical framing.
pub fn encode_canonical_sccp_groth16_bn254_proof_result_v1(
    result: &SccpGroth16Bn254ProofResultV1,
) -> Option<Vec<u8>> {
    if !sccp_groth16_bn254_proof_result_is_structurally_valid(result) {
        return None;
    }
    let bytes = to_bytes(result).ok()?;
    (bytes.len() <= SCCP_GROTH16_BN254_MAX_ENCODED_ARTIFACT_BYTES_V1).then_some(bytes)
}
/// Decode exactly one canonical, size-bounded minimal Groth16 result.
pub fn decode_canonical_sccp_groth16_bn254_proof_result_v1(
    bytes: &[u8],
) -> Option<SccpGroth16Bn254ProofResultV1> {
    decode_canonical_sccp_groth16_bn254_norito_v1(
        bytes,
        sccp_groth16_bn254_proof_result_is_structurally_valid,
    )
}
/// Decode exactly one canonical, size-bounded JSON minimal Groth16 result.
pub fn decode_canonical_sccp_groth16_bn254_proof_result_json_v1(
    json: &str,
) -> Option<SccpGroth16Bn254ProofResultV1> {
    decode_canonical_sccp_groth16_bn254_json_v1(
        json,
        sccp_groth16_bn254_proof_result_is_structurally_valid,
    )
}
/// Encode one pairing-verified Groth16 artifact with canonical Norito framing.
pub fn encode_canonical_sccp_groth16_bn254_proof_artifact_v1(
    artifact: &SccpGroth16Bn254ProofArtifactV1,
) -> Option<Vec<u8>> {
    if !sccp_groth16_bn254_proof_artifact_is_self_canonical(artifact) {
        return None;
    }
    let bytes = to_bytes(artifact).ok()?;
    (bytes.len() <= SCCP_GROTH16_BN254_MAX_ENCODED_ARTIFACT_BYTES_V1).then_some(bytes)
}
/// Decode exactly one canonical, bounded, pairing-verified Groth16 artifact.
pub fn decode_canonical_sccp_groth16_bn254_proof_artifact_v1(
    bytes: &[u8],
) -> Option<SccpGroth16Bn254ProofArtifactV1> {
    decode_canonical_sccp_groth16_bn254_norito_v1(
        bytes,
        sccp_groth16_bn254_proof_artifact_is_self_canonical,
    )
}
/// Decode exactly one canonical JSON, bounded, pairing-verified Groth16 artifact.
pub fn decode_canonical_sccp_groth16_bn254_proof_artifact_json_v1(
    json: &str,
) -> Option<SccpGroth16Bn254ProofArtifactV1> {
    decode_canonical_sccp_groth16_bn254_json_v1(
        json,
        sccp_groth16_bn254_proof_artifact_is_self_canonical,
    )
}
fn sccp_groth16_artifact_bridge_backend_v1(
    artifact: &SccpGroth16Bn254ProofArtifactV1,
) -> Option<BridgeSccpDestinationProofBackendV1> {
    sccp_destination_proof_backend_supports_network_v1(
        artifact.request.backend,
        artifact.request.target_network,
    )
    .then_some(artifact.request.backend)
}
/// Wrap one canonical Groth16 artifact in the closed bridge destination-proof container.
pub fn bridge_sccp_destination_proof_v1(
    artifact: &SccpGroth16Bn254ProofArtifactV1,
) -> Option<BridgeSccpDestinationProofV1> {
    Some(BridgeSccpDestinationProofV1 {
        backend: sccp_groth16_artifact_bridge_backend_v1(artifact)?,
        route_configuration_hash: artifact.request.route_configuration_hash,
        encoded_artifact: encode_canonical_sccp_groth16_bn254_proof_artifact_v1(artifact)?,
    })
}
fn decode_bridge_sccp_destination_proof_framing_v1(
    proof: &BridgeSccpDestinationProofV1,
) -> Option<SccpGroth16Bn254ProofArtifactV1> {
    count_sccp_destination_artifact_decode_v1();
    let artifact =
        decode_canonical_sccp_groth16_bn254_norito_framing_v1(proof.encoded_artifact.as_slice())?;
    (sccp_groth16_artifact_bridge_backend_v1(&artifact) == Some(proof.backend)
        && proof.route_configuration_hash == artifact.request.route_configuration_hash
        && proof.is_well_formed_for(
            artifact.request.destination_binding_hash,
            artifact.result.result_hash,
        ))
    .then_some(artifact)
}
/// Decode a closed bridge destination-proof and require its outer backend to
/// equal the canonical artifact's inner backend and target family.
pub fn decode_bridge_sccp_destination_proof_v1(
    proof: &BridgeSccpDestinationProofV1,
) -> Option<SccpGroth16Bn254ProofArtifactV1> {
    let artifact = decode_bridge_sccp_destination_proof_framing_v1(proof)?;
    sccp_groth16_bn254_proof_artifact_is_self_canonical(&artifact).then_some(artifact)
}
/// Return whether a submitted Groth16 artifact is exactly bound to the
/// canonical request reconstructed from governed historical state.
pub fn sccp_groth16_bn254_proof_artifact_matches_governed_route_v1(
    artifact: &SccpGroth16Bn254ProofArtifactV1,
    bundle: &TairaSccpMessageProofV1,
    governed_route: &SccpGovernedRouteV1,
) -> bool {
    let Some(request) =
        build_sccp_groth16_bn254_proof_request_from_governed_route_v1(bundle, governed_route)
    else {
        return false;
    };
    if artifact.version != 1 || artifact.request != request {
        return false;
    }
    let expected = match governed_route.destination {
        SccpDestinationDeploymentV1::Evm(_) => {
            wrap_sccp_evm_groth16_bn254_proof_result(&artifact.result.proof_bytes, &request)
        }
        SccpDestinationDeploymentV1::Tron(_) => {
            wrap_sccp_tron_groth16_bn254_proof_result(&artifact.result.proof_bytes, &request)
        }
        SccpDestinationDeploymentV1::Ton(_) => return false,
    };
    expected.as_ref() == Some(artifact)
}
fn build_sccp_verified_destination_call_v1(
    bundle: &TairaSccpMessageProofV1,
    artifact: &SccpGroth16Bn254ProofArtifactV1,
    governed_route: &SccpGovernedRouteV1,
    canonical_payload_bytes: Vec<u8>,
) -> Option<SccpVerifiedDestinationCallV1> {
    let calldata = encode_sccp_finalize_from_taira_calldata_v1(
        &artifact.result.proof_bytes,
        &artifact.request.public_inputs,
        artifact.request.statement_hash,
        &canonical_payload_bytes,
    )?;
    let target = match governed_route.destination {
        SccpDestinationDeploymentV1::Evm(deployment) => SccpDestinationCallTargetV1::Evm {
            network: governed_route.lane_id.source,
            route_address: deployment.route_address,
        },
        SccpDestinationDeploymentV1::Tron(deployment) => SccpDestinationCallTargetV1::Tron {
            network: governed_route.lane_id.source,
            route_address: deployment.route_address,
        },
        // TON uses a dedicated BLS12-381 artifact and TVM internal-message
        // body. It must not be projected into Solidity ABI calldata.
        SccpDestinationDeploymentV1::Ton(_) => return None,
    };
    let SccpPayloadV1::Transfer(transfer) = &bundle.payload;
    Some(SccpVerifiedDestinationCallV1 {
        version: 1,
        backend: artifact.request.backend,
        counterparty_domain: artifact.request.target_network.domain_id(),
        route_revision: transfer.route_revision,
        destination_binding_hash: artifact.request.destination_binding_hash,
        route_configuration_hash: artifact.request.route_configuration_hash,
        semantic_proof_profile: artifact.request.semantic_proof_profile,
        semantic_proof_profile_hash: artifact.request.semantic_proof_profile_hash,
        sora_finality_anchor: artifact.request.sora_finality_anchor,
        sora_finality_anchor_hash: artifact.request.sora_finality_anchor_hash,
        target,
        public_inputs: artifact.request.public_inputs,
        statement_hash: artifact.request.statement_hash,
        request_hash: artifact.request.request_hash,
        proof_bytes: artifact.result.proof_bytes.clone(),
        canonical_payload_bytes,
        calldata,
        bundle: bundle.clone(),
    })
}
/// Decode one destination artifact, its canonical embedded SCCP bundle, and its Taira finality
/// proof exactly once without evaluating a pairing or BLS aggregate.
///
/// The result is structurally and hash bound, but remains untrusted until it is resolved against
/// historical governed route state and caller-supplied authoritative finality by
/// [`verify_parsed_sccp_destination_proof_v1`].
fn parse_sccp_bn254_destination_proof_v1(
    proof: &BridgeSccpDestinationProofV1,
) -> Option<SccpParsedDestinationProofV1> {
    let artifact = decode_bridge_sccp_destination_proof_framing_v1(proof)?;
    let decoded =
        decode_canonical_taira_sccp_message_bundle_with_payload_v1(&artifact.request.bundle_bytes)?;
    let bundle = decoded.bundle;
    let finality = decode_taira_bridge_finality_proof(&bundle.finality_proof)?;
    if !verify_message_bundle_structure_with_finality(&bundle, &finality) {
        return None;
    }
    let binding = SccpCanonicalMessageBundleBindingV1 {
        source_network: bundle.commitment.context.lane.source,
        target_network: bundle.commitment.context.lane.target,
        destination_binding_hash: bundle.commitment.context.destination_binding_hash,
        route_configuration_hash: bundle.commitment.context.route_configuration_hash,
        message_id: bundle.commitment.message_id,
        payload_hash: bundle.commitment.payload_hash,
        commitment_root: bundle.commitment_root,
        finality_proof: &bundle.finality_proof,
    };
    let validated_request = validate_sccp_groth16_bn254_proof_request_with_bundle_v1(
        &artifact.request,
        proof.backend,
        binding,
        &decoded.canonical_payload_bytes,
        &finality,
    )?;
    if artifact.version != 1
        || artifact.result.request_hash != artifact.request.request_hash
        || proof.route_configuration_hash != artifact.request.route_configuration_hash
    {
        return None;
    }
    let groth16_proof = decode_sccp_groth16_bn254_proof_result_v1(&artifact.result)?;
    if groth16_proof.version != 1
        || groth16_proof.message_id != artifact.request.public_inputs.message_id
        || groth16_proof.source_domain != artifact.request.source_network.domain_id()
        || groth16_proof.commitment_root != artifact.request.public_inputs.commitment_root
        || encode_sccp_evm_groth16_bn254_proof_bytes(&groth16_proof) != artifact.result.proof_bytes
    {
        return None;
    }
    let public_signal_words = validated_request.public_signal_words;
    Some(SccpParsedDestinationProofV1 {
        artifact: SccpParsedDestinationArtifactV1 {
            result: SccpParsedDestinationProofResultV1 {
                result_hash: artifact.result.result_hash,
            },
        },
        backend: proof.backend,
        material: SccpParsedDestinationMaterialV1::Bn254 {
            artifact,
            public_signal_words,
            groth16_proof,
        },
        bundle,
        finality,
        canonical_payload_bytes: decoded.canonical_payload_bytes,
    })
}
fn parse_sccp_ton_destination_proof_v1(
    proof: &BridgeSccpDestinationProofV1,
) -> Option<SccpParsedDestinationProofV1> {
    let artifact = decode_bridge_sccp_ton_destination_proof_framing_v1(proof)?;
    let decoded =
        decode_canonical_taira_sccp_message_bundle_with_payload_v1(&artifact.request.bundle_bytes)?;
    let finality = decode_taira_bridge_finality_proof(&decoded.bundle.finality_proof)?;
    if !sccp_ton_groth16_bls12381_artifact_matches_decoded_bundle_v1(&artifact, &decoded, &finality)
    {
        return None;
    }
    Some(SccpParsedDestinationProofV1 {
        artifact: SccpParsedDestinationArtifactV1 {
            result: SccpParsedDestinationProofResultV1 {
                result_hash: artifact.result.result_hash,
            },
        },
        backend: proof.backend,
        material: SccpParsedDestinationMaterialV1::TonBls12381 { artifact },
        bundle: decoded.bundle,
        finality,
        canonical_payload_bytes: decoded.canonical_payload_bytes,
    })
}
/// Decode one destination artifact, its canonical embedded SCCP bundle, and
/// its structurally checked Taira finality proof exactly once.
///
/// Parsing performs no curve decompression, subgroup check, pairing, or BLS
/// aggregate verification. Use [`SccpParsedDestinationProofV1::crypto_work`]
/// to classify the single pairing operation required by route-bound
/// verification after the caller reserves bounded verifier work.
pub fn parse_sccp_destination_proof_v1(
    proof: &BridgeSccpDestinationProofV1,
) -> Option<SccpParsedDestinationProofV1> {
    match proof.backend {
        BridgeSccpDestinationProofBackendV1::TonGroth16Bls12381 => {
            parse_sccp_ton_destination_proof_v1(proof)
        }
        BridgeSccpDestinationProofBackendV1::EvmGroth16Bn254
        | BridgeSccpDestinationProofBackendV1::TronGroth16Bn254 => {
            parse_sccp_bn254_destination_proof_v1(proof)
        }
    }
}
/// Decode one canonical closed destination-proof envelope and structurally
/// parse its curve-specific artifact.
///
/// This boundary performs no curve decompression, subgroup check, pairing, or
/// BLS aggregate verification. Consensus callers must reserve the work
/// classified by [`SccpParsedDestinationProofV1::crypto_work`] before invoking
/// route-bound verification.
#[must_use]
pub fn decode_and_parse_canonical_sccp_destination_proof_v1(
    bytes: &[u8],
) -> Option<(BridgeSccpDestinationProofV1, SccpParsedDestinationProofV1)> {
    if !preflight_uncompressed_norito_frame(bytes, SCCP_DESTINATION_PROOF_MAX_ENCODED_BYTES_V1) {
        return None;
    }
    let proof: BridgeSccpDestinationProofV1 = norito::decode_from_bytes(bytes).ok()?;
    if proof.encoded_artifact.len() > SCCP_GROTH16_BN254_MAX_ENCODED_ARTIFACT_BYTES_V1
        || to_bytes(&proof).ok()?.as_slice() != bytes
    {
        return None;
    }
    let parsed = parse_sccp_destination_proof_v1(&proof)?;
    Some((proof, parsed))
}
fn build_sccp_groth16_bn254_proof_request_from_parsed_v1(
    parsed: &SccpParsedDestinationProofV1,
    governed_route: &SccpGovernedRouteV1,
) -> Option<SccpGroth16Bn254ProofRequestV1> {
    let SccpParsedDestinationMaterialV1::Bn254 { artifact, .. } = &parsed.material else {
        return None;
    };
    if !sccp_governed_groth16_route_matches_bundle_v1(&parsed.bundle, governed_route) {
        return None;
    }
    let request = &artifact.request;
    let destination_binding_hash = governed_route.destination_binding_hash().ok()?;
    let route_configuration_hash = governed_route.route_configuration_hash().ok()?;
    let (verifying_key, expected_verifier_key_hash, outbound_proof_policy) =
        sccp_governed_route_groth16_material_v1(governed_route)?;
    let backend = match governed_route.destination {
        SccpDestinationDeploymentV1::Evm(_) => BridgeSccpDestinationProofBackendV1::EvmGroth16Bn254,
        SccpDestinationDeploymentV1::Tron(_) => {
            BridgeSccpDestinationProofBackendV1::TronGroth16Bn254
        }
        SccpDestinationDeploymentV1::Ton(_) => return None,
    };
    build_sccp_groth16_bn254_proof_request(&SccpGroth16Bn254ProofRequestBuildContextV1 {
        backend,
        source_network: governed_route.lane_id.target,
        target_network: governed_route.lane_id.source,
        public_inputs: &request.public_inputs,
        canonical_payload_bytes: &parsed.canonical_payload_bytes,
        bundle_bytes: &request.bundle_bytes,
        bundle_binding: SccpCanonicalMessageBundleBindingV1 {
            source_network: parsed.bundle.commitment.context.lane.source,
            target_network: parsed.bundle.commitment.context.lane.target,
            destination_binding_hash: parsed.bundle.commitment.context.destination_binding_hash,
            route_configuration_hash: parsed.bundle.commitment.context.route_configuration_hash,
            message_id: parsed.bundle.commitment.message_id,
            payload_hash: parsed.bundle.commitment.payload_hash,
            commitment_root: parsed.bundle.commitment_root,
            finality_proof: &parsed.bundle.finality_proof,
        },
        finality: &parsed.finality,
        destination_binding_hash,
        route_configuration_hash,
        verifying_key: &verifying_key,
        expected_verifier_key_hash,
        outbound_proof_policy,
    })
}
fn build_sccp_verified_ton_destination_call_from_parsed_v1(
    parsed: &SccpParsedDestinationProofV1,
    artifact: &SccpTonGroth16Bls12381ProofArtifactV1,
    governed_route: &SccpGovernedRouteV1,
) -> Option<SccpVerifiedDestinationCallV1> {
    let expected_request = build_sccp_ton_groth16_bls12381_request_from_bound_finality_v1(
        &parsed.bundle,
        governed_route,
        &parsed.finality,
    )?;
    if artifact.request != expected_request
        || !verify_sccp_groth16_bls12381_pairing_v1(
            &artifact.result.proof,
            &artifact.request.public_signals,
            &artifact.request.verifying_key,
        )
    {
        return None;
    }
    let SccpDestinationDeploymentV1::Ton(deployment) = governed_route.destination else {
        return None;
    };
    let SccpPayloadV1::Transfer(transfer) = &parsed.bundle.payload;
    decode_sccp_ton_account36_v1(&transfer.recipient)?;
    sccp_ton_payload_amount_to_jetton_base_units_v1(transfer.amount, &deployment)?;
    let calldata = encode_sccp_ton_finalize_from_taira_body_boc_after_verification_v1(
        0,
        &artifact.request,
        &artifact.result.proof,
        &parsed.canonical_payload_bytes,
    )?;
    Some(SccpVerifiedDestinationCallV1 {
        version: 1,
        backend: BridgeSccpDestinationProofBackendV1::TonGroth16Bls12381,
        counterparty_domain: SCCP_DOMAIN_TON,
        route_revision: transfer.route_revision,
        destination_binding_hash: artifact.request.destination_binding_hash,
        route_configuration_hash: artifact.request.route_configuration_hash,
        semantic_proof_profile: artifact.request.semantic_proof_profile,
        semantic_proof_profile_hash: artifact.request.semantic_proof_profile_hash,
        sora_finality_anchor: artifact.request.sora_finality_anchor,
        sora_finality_anchor_hash: artifact.request.sora_finality_anchor_hash,
        target: SccpDestinationCallTargetV1::Ton {
            network: governed_route.lane_id.source,
            route_address: deployment.route_address,
        },
        public_inputs: artifact.request.public_inputs,
        statement_hash: artifact.request.statement_hash,
        request_hash: artifact.request.request_hash,
        proof_bytes: canonical_sccp_groth16_bls12381_proof_bytes_v1(&artifact.result.proof)?,
        canonical_payload_bytes: parsed.canonical_payload_bytes.clone(),
        calldata,
        bundle: parsed.bundle.clone(),
    })
}
/// Bind one parsed artifact to the exact historical governed route, evaluate
/// one Groth16 pairing and the embedded Taira finality certificate, and derive
/// the destination call without decoding the submitted request again.
///
/// Call material is never exposed by the parse-only phase. `trusted_finality`
/// must come from an authoritative local state boundary; exact equality with
/// the proof-embedded value is mandatory before any pairing is evaluated.
pub fn verify_parsed_sccp_destination_proof_v1(
    parsed: SccpParsedDestinationProofV1,
    governed_route: &SccpGovernedRouteV1,
    trusted_finality: &TairaBridgeFinalityProofV1,
) -> Option<SccpVerifiedDestinationCallV1> {
    if parsed.finality != *trusted_finality {
        return None;
    }
    let call = match &parsed.material {
        SccpParsedDestinationMaterialV1::Bn254 {
            artifact,
            public_signal_words,
            groth16_proof,
        } => {
            let expected_request =
                build_sccp_groth16_bn254_proof_request_from_parsed_v1(&parsed, governed_route)?;
            if artifact.request != expected_request
                || !verify_sccp_groth16_bn254_pairing_equation_v1(
                    groth16_proof,
                    public_signal_words,
                    &artifact.request.verifying_key,
                )
            {
                return None;
            }
            build_sccp_verified_destination_call_v1(
                &parsed.bundle,
                artifact,
                governed_route,
                parsed.canonical_payload_bytes.clone(),
            )?
        }
        SccpParsedDestinationMaterialV1::TonBls12381 { artifact } => {
            build_sccp_verified_ton_destination_call_from_parsed_v1(
                &parsed,
                artifact,
                governed_route,
            )?
        }
    };
    verify_taira_bridge_finality_proof_cryptographic(trusted_finality).then_some(call)
}
/// Verify one closed bridge SCCP destination proof against the exact bundle,
/// historical governed route, and explicitly trusted Taira finality, then
/// derive the canonical destination call.
pub fn verify_sccp_destination_proof_v1(
    proof: &BridgeSccpDestinationProofV1,
    bundle: &TairaSccpMessageProofV1,
    governed_route: &SccpGovernedRouteV1,
    trusted_finality: &TairaBridgeFinalityProofV1,
) -> Option<SccpVerifiedDestinationCallV1> {
    let parsed = parse_sccp_destination_proof_v1(proof)?;
    if parsed.bundle() != bundle {
        return None;
    }
    verify_parsed_sccp_destination_proof_v1(parsed, governed_route, trusted_finality)
}
fn sccp_exact_xor_destination_route_id_v1(target_domain: u32) -> Option<&'static [u8]> {
    match target_domain {
        SCCP_DOMAIN_ETH => Some(SCCP_TAIRA_ETH_XOR_ROUTE_ID_V1.as_bytes()),
        SCCP_DOMAIN_BSC => Some(SCCP_TAIRA_BSC_XOR_ROUTE_ID_V1.as_bytes()),
        SCCP_DOMAIN_TON => Some(SCCP_TAIRA_TON_XOR_ROUTE_ID_V1.as_bytes()),
        SCCP_DOMAIN_TRON => Some(SCCP_TAIRA_TRON_XOR_ROUTE_ID_V1.as_bytes()),
        _ => None,
    }
}
fn sccp_payload_matches_exact_xor_destination_route_v1(
    payload: &SccpPayloadV1,
    target_domain: u32,
) -> bool {
    let Some(expected_route_id) = sccp_exact_xor_destination_route_id_v1(target_domain) else {
        return false;
    };
    let expected_recipient_codec = match target_domain {
        SCCP_DOMAIN_ETH | SCCP_DOMAIN_BSC => SCCP_CODEC_EVM_ADDRESS20,
        SCCP_DOMAIN_TON => SCCP_CODEC_TON_ACCOUNT36,
        SCCP_DOMAIN_TRON => SCCP_CODEC_TRON_ADDRESS21,
        _ => return false,
    };
    let SccpPayloadV1::Transfer(transfer) = payload;
    verify_sccp_payload_structure(payload)
        && transfer.source_domain == SCCP_DOMAIN_SORA
        && transfer.dest_domain == target_domain
        && transfer.asset_home_domain == SCCP_DOMAIN_SORA
        && transfer.asset_id_codec == SCCP_CODEC_CANONICAL_TEXT
        && transfer.asset_id == SCCP_TAIRA_XOR_ASSET_KEY_V1.as_bytes()
        && transfer.sender_codec == SCCP_CODEC_CANONICAL_TEXT
        && transfer.recipient_codec == expected_recipient_codec
        && transfer.route_id_codec == SCCP_CODEC_CANONICAL_TEXT
        && transfer.route_id == expected_route_id
}
/// Derive the six base public inputs from one canonical SORA-origin bundle.
pub fn sccp_message_public_inputs(
    bundle: &TairaSccpMessageProofV1,
) -> Option<SccpMessagePublicInputsV1> {
    let finality = decode_taira_bridge_finality_proof(&bundle.finality_proof)?;
    sccp_message_public_inputs_with_finality(bundle, &finality)
}
fn sccp_message_public_inputs_with_finality(
    bundle: &TairaSccpMessageProofV1,
    finality: &TairaBridgeFinalityProofV1,
) -> Option<SccpMessagePublicInputsV1> {
    if !verify_message_bundle_structure_with_finality(bundle, finality) {
        return None;
    }
    Some(SccpMessagePublicInputsV1 {
        version: 1,
        message_id: bundle.commitment.message_id,
        payload_hash: bundle.commitment.payload_hash,
        target_domain: bundle.commitment.context.lane.target.domain_id(),
        commitment_root: bundle.commitment_root,
        finality_height: finality.finality_artifact.height,
        finality_block_hash: hash_block_header_for_sccp_finality(&finality.block_header),
    })
}
fn decode_nonzero_fixed<const N: usize>(bytes: &[u8]) -> Option<[u8; N]> {
    let value: [u8; N] = bytes.try_into().ok()?;
    value.iter().any(|byte| *byte != 0).then_some(value)
}
fn canonical_sccp_text(bytes: &[u8]) -> Option<&str> {
    if bytes.is_empty() || bytes.len() > SCCP_MAX_CANONICAL_TEXT_BYTES_V1 {
        return None;
    }
    let value = core::str::from_utf8(bytes).ok()?;
    if bytes.iter().all(|byte| matches!(byte, 0x21..=0x7e)) {
        return Some(value);
    }
    // Canonical universal account identities use I105's complete base-105
    // alphabet, which intentionally includes half-width kana. Accepting
    // arbitrary Unicode here would make route and asset labels ambiguous, so
    // the non-ASCII branch is closed to an exact, checksum-valid I105 literal.
    let discriminant =
        iroha_data_model::account::address::AccountAddress::i105_discriminant(value).ok()?;
    let address = iroha_data_model::account::address::AccountAddress::parse_encoded(
        value,
        Some(discriminant),
    )
    .ok()?;
    (address
        .to_i105_for_discriminant(discriminant)
        .ok()?
        .as_str()
        == value)
        .then_some(value)
}
fn decode_tron_address21(bytes: &[u8]) -> Option<[u8; 21]> {
    let address: [u8; 21] = bytes.try_into().ok()?;
    (address[0] == 0x41 && address[1..].iter().any(|byte| *byte != 0)).then_some(address)
}
/// Encode one first-release TON basechain account in the external raw-address
/// representation used by SCCP payloads.
///
/// The signed workchain prefix is big-endian. This deliberately differs from
/// the little-endian integer encoding used inside registry commitment
/// preimages, so callers cannot accidentally substitute one boundary for the
/// other.
#[must_use]
pub fn canonical_sccp_ton_account36_bytes_v1(address: SccpTonAddressV1) -> Option<[u8; 36]> {
    if !address.is_sccp_basechain_contract() {
        return None;
    }
    let mut bytes = [0_u8; 36];
    bytes[..4].copy_from_slice(&address.workchain.to_be_bytes());
    bytes[4..].copy_from_slice(&address.account);
    Some(bytes)
}
/// Decode one canonical nonzero TON basechain raw account.
#[must_use]
pub fn decode_sccp_ton_account36_v1(bytes: &[u8]) -> Option<SccpTonAddressV1> {
    let bytes: [u8; 36] = bytes.try_into().ok()?;
    let address = SccpTonAddressV1 {
        workchain: i32::from_be_bytes(bytes[..4].try_into().expect("fixed four-byte prefix")),
        account: bytes[4..].try_into().expect("fixed 32-byte account"),
    };
    (canonical_sccp_ton_account36_bytes_v1(address) == Some(bytes)).then_some(address)
}
/// Decode one closed SCCP wire codec into a typed, canonical normalized value.
pub fn decode_sccp_normalized_codec_value(
    codec_id: u8,
    bytes: &[u8],
) -> Option<SccpNormalizedCodecValueV1> {
    match codec_id {
        SCCP_CODEC_CANONICAL_TEXT => {
            let value = canonical_sccp_text(bytes)?;
            Some(SccpNormalizedCodecValueV1::CanonicalText {
                value: value.to_owned(),
            })
        }
        SCCP_CODEC_EVM_ADDRESS20 => Some(SccpNormalizedCodecValueV1::EvmAddress20 {
            bytes: decode_nonzero_fixed(bytes)?,
        }),
        SCCP_CODEC_TRON_ADDRESS21 => Some(SccpNormalizedCodecValueV1::TronAddress21 {
            bytes: decode_tron_address21(bytes)?,
        }),
        SCCP_CODEC_TON_ACCOUNT36 => {
            let address = decode_sccp_ton_account36_v1(bytes)?;
            Some(SccpNormalizedCodecValueV1::TonAccount36 {
                workchain: address.workchain,
                account: address.account,
            })
        }
        _ => None,
    }
}
fn validate_sccp_codec_bytes(codec_id: u8, bytes: &[u8]) -> bool {
    decode_sccp_normalized_codec_value(codec_id, bytes).is_some()
}
fn push_u8(out: &mut Vec<u8>, value: u8) {
    out.push(value);
}
fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn push_u32_len_checked(out: &mut Vec<u8>, len: usize) -> Option<()> {
    let len = u32::try_from(len).ok()?;
    push_u32(out, len);
    Some(())
}
fn push_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn push_u128(out: &mut Vec<u8>, value: u128) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn push_vec_checked(out: &mut Vec<u8>, value: &[u8]) -> Option<()> {
    push_u32_len_checked(out, value.len())?;
    out.extend_from_slice(value);
    Some(())
}
fn sccp_payload_field_len_v1(
    field: &'static str,
    actual: usize,
) -> Result<u32, SccpCanonicalPayloadEncodingErrorV1> {
    u32::try_from(actual).map_err(
        |_| SccpCanonicalPayloadEncodingErrorV1::FieldLengthOverflow {
            field,
            actual,
            maximum: u32::MAX,
        },
    )
}
fn push_sccp_payload_vec_v1(
    out: &mut Vec<u8>,
    field: &'static str,
    value: &[u8],
) -> Result<(), SccpCanonicalPayloadEncodingErrorV1> {
    push_u32(out, sccp_payload_field_len_v1(field, value.len())?);
    out.extend_from_slice(value);
    Ok(())
}
fn protobuf_varint_len(mut value: u64) -> usize {
    let mut len = 1usize;
    while value >= 0x80 {
        len += 1;
        value >>= 7;
    }
    len
}
fn read_protobuf_varint_at(bytes: &[u8], cursor: &mut usize) -> Option<u64> {
    let start = *cursor;
    let mut value = 0u64;
    let mut shift = 0u32;
    for index in 0..10 {
        let byte = *bytes.get(*cursor)?;
        *cursor = (*cursor).checked_add(1)?;
        let chunk = u64::from(byte & 0x7f);
        if index == 9 && chunk > 1 {
            return None;
        }
        value |= chunk.checked_shl(shift)?;
        if byte & 0x80 == 0 {
            let consumed = (*cursor).checked_sub(start)?;
            return (consumed == protobuf_varint_len(value)).then_some(value);
        }
        shift = shift.checked_add(7)?;
    }
    None
}
/// Encode a transfer payload in its canonical length-prefixed V1 layout.
///
/// # Errors
///
/// Returns [`SccpCanonicalPayloadEncodingErrorV1::FieldLengthOverflow`] when
/// any variable-length field cannot be represented by the V1 `u32` prefix.
pub fn canonical_transfer_payload_bytes(
    payload: &TransferPayloadV1,
) -> Result<Vec<u8>, SccpCanonicalPayloadEncodingErrorV1> {
    let mut out = Vec::new();
    push_u8(&mut out, payload.version);
    push_u32(&mut out, payload.source_domain);
    push_u32(&mut out, payload.dest_domain);
    push_u64(&mut out, payload.nonce);
    push_u32(&mut out, payload.route_revision);
    push_u32(&mut out, payload.asset_home_domain);
    push_u8(&mut out, payload.asset_id_codec);
    push_sccp_payload_vec_v1(&mut out, "asset_id", &payload.asset_id)?;
    push_u128(&mut out, payload.amount);
    push_u8(&mut out, payload.sender_codec);
    push_sccp_payload_vec_v1(&mut out, "sender", &payload.sender)?;
    push_u8(&mut out, payload.recipient_codec);
    push_sccp_payload_vec_v1(&mut out, "recipient", &payload.recipient)?;
    push_u8(&mut out, payload.route_id_codec);
    push_sccp_payload_vec_v1(&mut out, "route_id", &payload.route_id)?;
    Ok(out)
}
/// Encode any closed SCCP payload with its stable V1 discriminant and canonical body.
///
/// # Errors
///
/// Returns [`SccpCanonicalPayloadEncodingErrorV1::FieldLengthOverflow`] when
/// any variable-length field cannot be represented by the V1 `u32` prefix.
pub fn canonical_sccp_payload_bytes(
    payload: &SccpPayloadV1,
) -> Result<Vec<u8>, SccpCanonicalPayloadEncodingErrorV1> {
    let mut out = Vec::new();
    match payload {
        SccpPayloadV1::Transfer(payload) => {
            push_u8(&mut out, SccpPayloadV1::TRANSFER_DISCRIMINANT);
            out.extend_from_slice(&canonical_transfer_payload_bytes(payload)?);
        }
    }
    Ok(out)
}
#[cfg(test)]
mod canonical_payload_encoding_tests {
    use super::*;
    fn transfer_fixture() -> TransferPayloadV1 {
        TransferPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_ETH,
            nonce: 7,
            route_revision: 1,
            asset_home_domain: SCCP_DOMAIN_SORA,
            asset_id_codec: SCCP_CODEC_CANONICAL_TEXT,
            asset_id: b"xor".to_vec(),
            amount: 11,
            sender_codec: SCCP_CODEC_CANONICAL_TEXT,
            sender: b"alice@taira".to_vec(),
            recipient_codec: SCCP_CODEC_EVM_ADDRESS20,
            recipient: vec![0x11; 20],
            route_id_codec: SCCP_CODEC_CANONICAL_TEXT,
            route_id: SCCP_TAIRA_ETH_XOR_ROUTE_ID_V1.as_bytes().to_vec(),
        }
    }
    #[test]
    fn canonical_payload_encoders_roundtrip_every_variable_field() {
        let transfer = transfer_fixture();
        let transfer_bytes = canonical_transfer_payload_bytes(&transfer)
            .expect("bounded transfer payload must encode");
        assert_eq!(
            decode_canonical_transfer_payload_bytes(&transfer_bytes),
            Some(transfer.clone())
        );
        let payload = SccpPayloadV1::Transfer(transfer);
        let payload_bytes =
            canonical_sccp_payload_bytes(&payload).expect("bounded SCCP payload must encode");
        assert_eq!(
            decode_canonical_sccp_payload_bytes(&payload_bytes),
            Some(payload)
        );
    }
    #[test]
    fn canonical_payload_field_length_accepts_exact_u32_boundary() {
        let boundary = usize::try_from(u32::MAX).expect("usize represents u32 lengths");
        assert_eq!(
            sccp_payload_field_len_v1("recipient", boundary),
            Ok(u32::MAX)
        );
    }
    #[cfg(target_pointer_width = "64")]
    #[test]
    fn canonical_payload_field_length_overflow_is_typed_and_never_panics() {
        let actual = usize::try_from(u64::from(u32::MAX) + 1)
            .expect("64-bit usize represents one byte beyond the SCCP V1 limit");
        for field in ["asset_id", "sender", "recipient", "route_id"] {
            assert_eq!(
                sccp_payload_field_len_v1(field, actual),
                Err(SccpCanonicalPayloadEncodingErrorV1::FieldLengthOverflow {
                    field,
                    actual,
                    maximum: u32::MAX,
                })
            );
        }
    }
}
/// Decode one complete canonical transfer-payload body without accepting trailing bytes.
pub fn decode_canonical_transfer_payload_bytes(payload_bytes: &[u8]) -> Option<TransferPayloadV1> {
    let mut cursor = PayloadCursor::new(payload_bytes);
    let payload = TransferPayloadV1 {
        version: cursor.take_u8()?,
        source_domain: cursor.take_u32()?,
        dest_domain: cursor.take_u32()?,
        nonce: cursor.take_u64()?,
        route_revision: cursor.take_u32()?,
        asset_home_domain: cursor.take_u32()?,
        asset_id_codec: cursor.take_u8()?,
        asset_id: cursor.take_vec()?,
        amount: cursor.take_u128()?,
        sender_codec: cursor.take_u8()?,
        sender: cursor.take_vec()?,
        recipient_codec: cursor.take_u8()?,
        recipient: cursor.take_vec()?,
        route_id_codec: cursor.take_u8()?,
        route_id: cursor.take_vec()?,
    };
    cursor.is_finished().then_some(payload)
}
struct PayloadCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> PayloadCursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn take_exact(&mut self, len: usize) -> Option<&'a [u8]> {
        let end = self.offset.checked_add(len)?;
        let slice = self.bytes.get(self.offset..end)?;
        self.offset = end;
        Some(slice)
    }
    fn take_u8(&mut self) -> Option<u8> {
        self.take_exact(1).map(|bytes| bytes[0])
    }
    fn take_u32(&mut self) -> Option<u32> {
        let mut out = [0u8; 4];
        out.copy_from_slice(self.take_exact(4)?);
        Some(u32::from_le_bytes(out))
    }
    fn take_u64(&mut self) -> Option<u64> {
        let mut out = [0u8; 8];
        out.copy_from_slice(self.take_exact(8)?);
        Some(u64::from_le_bytes(out))
    }
    fn take_u128(&mut self) -> Option<u128> {
        let mut out = [0u8; 16];
        out.copy_from_slice(self.take_exact(16)?);
        Some(u128::from_le_bytes(out))
    }
    fn take_vec(&mut self) -> Option<Vec<u8>> {
        let len = usize::try_from(self.take_u32()?).ok()?;
        Some(self.take_exact(len)?.to_vec())
    }
    fn is_finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}
/// Decode one complete closed SCCP payload from its canonical V1 representation.
pub fn decode_canonical_sccp_payload_bytes(payload_bytes: &[u8]) -> Option<SccpPayloadV1> {
    let mut cursor = PayloadCursor::new(payload_bytes);
    let discriminant = cursor.take_u8()?;
    let payload = match discriminant {
        SccpPayloadV1::TRANSFER_DISCRIMINANT => {
            let payload = decode_canonical_transfer_payload_bytes(&cursor.bytes[cursor.offset..])?;
            cursor.offset = cursor.bytes.len();
            SccpPayloadV1::Transfer(payload)
        }
        _ => return None,
    };
    cursor.is_finished().then_some(payload)
}
fn h256_is_nonzero(value: &H256) -> bool {
    value.iter().any(|byte| *byte != 0)
}
fn hash_roles_alias(values: &[H256]) -> bool {
    values
        .iter()
        .enumerate()
        .any(|(index, value)| values[index + 1..].contains(value))
}
fn secp256k1_recoverable_signature_s_is_low(signature: &[u8; 65]) -> bool {
    let mut s = [0u8; 32];
    s.copy_from_slice(&signature[32..64]);
    h256_is_nonzero(&s) && s <= SECP256K1_SCALAR_HALF_ORDER_BE
}
fn secp256k1_recoverable_signature_r_is_valid(signature: &[u8; 65]) -> bool {
    let mut r = [0u8; 32];
    r.copy_from_slice(&signature[..32]);
    h256_is_nonzero(&r) && r < SECP256K1_SCALAR_ORDER_BE
}
fn tron_recoverable_signature_is_canonical(signature: &[u8; 65]) -> bool {
    matches!(signature[64], 0..=3)
        && secp256k1_recoverable_signature_r_is_valid(signature)
        && secp256k1_recoverable_signature_s_is_low(signature)
}
fn tron_recoverable_signature_for_recovery(signature: &[u8; 65]) -> Option<[u8; 65]> {
    if !tron_recoverable_signature_is_canonical(signature) {
        return None;
    }
    let mut normalized = *signature;
    normalized[64] = signature[64].checked_add(27)?;
    Some(normalized)
}
/// Validate the canonical structure and domain semantics of an SCCP v1 payload.
pub fn verify_sccp_payload_structure(payload: &SccpPayloadV1) -> bool {
    let target_domain = sccp_message_target_domain(payload);
    if !is_supported_domain(target_domain) {
        return false;
    }
    match payload {
        SccpPayloadV1::Transfer(payload) => {
            let Some(expected_sender_codec) =
                sccp_counterparty_account_codec(payload.source_domain)
            else {
                return false;
            };
            let Some(expected_recipient_codec) =
                sccp_counterparty_account_codec(payload.dest_domain)
            else {
                return false;
            };
            payload.version == 1
                && payload.route_revision != 0
                && is_supported_domain(payload.source_domain)
                && is_supported_domain(payload.asset_home_domain)
                && payload.source_domain != payload.dest_domain
                && validate_sccp_codec_bytes(payload.asset_id_codec, &payload.asset_id)
                && payload.amount != 0
                && payload.sender_codec == expected_sender_codec
                && validate_sccp_codec_bytes(payload.sender_codec, &payload.sender)
                && payload.recipient_codec == expected_recipient_codec
                && validate_sccp_codec_bytes(payload.recipient_codec, &payload.recipient)
                && validate_sccp_codec_bytes(payload.route_id_codec, &payload.route_id)
        }
    }
}
/// Build the exact outbound hub commitment for a governed destination context.
///
/// The constructor is intentionally fallible. Besides validating the exact SORA-to-external lane
/// against the payload domains, it rejects zero values and collisions among the lane, destination
/// binding, route configuration, message, and payload hash roles. This keeps malformed records out
/// of both Merkle trees and the durable replay index.
pub fn hub_commitment_from_sccp_payload(
    context: SccpOutboundMessageContextV1,
    payload: &SccpPayloadV1,
) -> Option<SccpHubCommitmentV1> {
    if !context.is_well_formed()
        || !verify_sccp_payload_structure(payload)
        || !sccp_payload_matches_lane(context.lane, payload)
    {
        return None;
    }
    let lane_hash = sccp_lane_id_hash_v1(context.lane)?;
    let message_id = sccp_message_id(context.lane, payload)?;
    let canonical_payload = canonical_sccp_payload_bytes(payload).ok()?;
    let payload_hash = payload_hash(&canonical_payload);
    if !hash_roles_are_distinct([
        lane_hash,
        context.destination_binding_hash,
        context.route_configuration_hash,
        message_id,
        payload_hash,
    ]) {
        return None;
    }
    Some(SccpHubCommitmentV1 {
        version: 1,
        kind: sccp_message_kind(payload),
        context,
        message_id,
        payload_hash,
    })
}
/// Encode a hub commitment independently of Rust/Norito enum layouts.
///
/// The fixed V1 layout is `version || kind || source_profile || target_profile
/// || destination_binding_hash || route_configuration_hash || message_id ||
/// payload_hash`, where the first four fields are one byte and each hash is 32
/// bytes. The closed profile tags are defined by [`sccp_network_tag_v1`].
pub fn canonical_commitment_bytes(commitment: &SccpHubCommitmentV1) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + 1 + 1 + 1 + 32 * 4);
    push_u8(&mut out, commitment.version);
    push_u8(
        &mut out,
        match commitment.kind {
            SccpHubMessageKind::Transfer => 5,
        },
    );
    push_u8(
        &mut out,
        sccp_network_tag_v1(commitment.context.lane.source),
    );
    push_u8(
        &mut out,
        sccp_network_tag_v1(commitment.context.lane.target),
    );
    out.extend_from_slice(&commitment.context.destination_binding_hash);
    out.extend_from_slice(&commitment.context.route_configuration_hash);
    out.extend_from_slice(&commitment.message_id);
    out.extend_from_slice(&commitment.payload_hash);
    out
}
/// Decode the exact fixed-width V1 hub commitment representation.
pub fn decode_canonical_commitment_bytes(bytes: &[u8]) -> Option<SccpHubCommitmentV1> {
    let mut cursor = PayloadCursor::new(bytes);
    let version = cursor.take_u8()?;
    let kind = match cursor.take_u8()? {
        5 => SccpHubMessageKind::Transfer,
        _ => return None,
    };
    let source = sccp_network_from_tag_v1(cursor.take_u8()?)?;
    let target = sccp_network_from_tag_v1(cursor.take_u8()?)?;
    let destination_binding_hash = cursor.take_exact(32)?.try_into().ok()?;
    let route_configuration_hash = cursor.take_exact(32)?.try_into().ok()?;
    let message_id = cursor.take_exact(32)?.try_into().ok()?;
    let payload_hash = cursor.take_exact(32)?.try_into().ok()?;
    if !cursor.is_finished() {
        return None;
    }
    let commitment = SccpHubCommitmentV1 {
        version,
        kind,
        context: SccpOutboundMessageContextV1 {
            lane: SccpLaneIdV1 { source, target },
            destination_binding_hash,
            route_configuration_hash,
        },
        message_id,
        payload_hash,
    };
    let lane_hash = sccp_lane_id_hash_v1(commitment.context.lane)?;
    (commitment.version == 1
        && commitment.context.is_well_formed()
        && hash_roles_are_distinct([
            lane_hash,
            commitment.context.destination_binding_hash,
            commitment.context.route_configuration_hash,
            commitment.message_id,
            commitment.payload_hash,
        ])
        && canonical_commitment_bytes(&commitment) == bytes)
        .then_some(commitment)
}
/// Derive the single V1 SCCP message identity for either lane direction.
///
/// Exact source and target profiles are part of the preimage, preventing an
/// otherwise identical payload from aliasing across the admitted external
/// mainnet profiles. The governed destination binding is not part of this identity;
/// replay protection therefore survives destination deployment rotation.
/// The Keccak-256 input is `SCCP_LANE_MESSAGE_ID_PREFIX_V1 || 0x01 ||
/// le_u32(lane_len) || canonical_lane || le_u32(payload_len) ||
/// canonical_payload`.
pub fn sccp_message_id(lane: SccpLaneIdV1, payload: &SccpPayloadV1) -> Option<H256> {
    if !verify_sccp_payload_structure(payload) || !sccp_payload_matches_lane(lane, payload) {
        return None;
    }
    let lane_bytes = canonical_sccp_lane_id_bytes_v1(lane)?;
    let payload_bytes = canonical_sccp_payload_bytes(payload).ok()?;
    let mut preimage = Vec::with_capacity(1 + 8 + lane_bytes.len() + payload_bytes.len());
    push_u8(&mut preimage, 1);
    push_vec_checked(&mut preimage, &lane_bytes)?;
    push_vec_checked(&mut preimage, &payload_bytes)?;
    let message_id = prefixed_keccak(SCCP_LANE_MESSAGE_ID_PREFIX_V1, &preimage);
    h256_is_nonzero(&message_id).then_some(message_id)
}
fn sccp_payload_matches_lane(lane: SccpLaneIdV1, payload: &SccpPayloadV1) -> bool {
    lane.is_well_formed()
        && lane.source.domain_id() == sccp_message_source_domain(payload)
        && lane.target.domain_id() == sccp_message_target_domain(payload)
}
fn hash_roles_are_distinct<const N: usize>(roles: [H256; N]) -> bool {
    roles.iter().all(h256_is_nonzero)
        && roles
            .iter()
            .enumerate()
            .all(|(index, role)| roles[index + 1..].iter().all(|other| role != other))
}
/// Return the stable hub-message kind corresponding to a closed SCCP payload variant.
pub fn sccp_message_kind(payload: &SccpPayloadV1) -> SccpHubMessageKind {
    match payload {
        SccpPayloadV1::Transfer(_) => SccpHubMessageKind::Transfer,
    }
}
/// Return the protocol destination domain carried by an SCCP payload.
pub fn sccp_message_target_domain(payload: &SccpPayloadV1) -> u32 {
    match payload {
        SccpPayloadV1::Transfer(payload) => payload.dest_domain,
    }
}
/// Return the protocol source domain carried or implied by an SCCP payload.
pub fn sccp_message_source_domain(payload: &SccpPayloadV1) -> u32 {
    match payload {
        SccpPayloadV1::Transfer(payload) => payload.source_domain,
    }
}
/// Hash canonical SCCP payload bytes under the V1 payload role separator.
pub fn payload_hash(payload: &[u8]) -> H256 {
    prefixed_blake2b(SCCP_PAYLOAD_HASH_PREFIX_V1, payload)
}
/// Hash one canonical hub commitment as an SCCP Merkle leaf.
pub fn commitment_leaf_hash(commitment: &SccpHubCommitmentV1) -> H256 {
    prefixed_blake2b(
        SCCP_HUB_LEAF_PREFIX_V1,
        &canonical_commitment_bytes(commitment),
    )
}
/// Reconstruct an SCCP Merkle root from one commitment and its ordered sibling path.
pub fn merkle_root_from_commitment(
    commitment: &SccpHubCommitmentV1,
    proof: &SccpMerkleProofV1,
) -> H256 {
    let mut current = commitment_leaf_hash(commitment);
    for step in &proof.steps {
        current = if step.sibling_is_left {
            hash_merkle_node(&step.sibling_hash, &current)
        } else {
            hash_merkle_node(&current, &step.sibling_hash)
        };
    }
    current
}
/// Build the deterministic SCCP Merkle root for a non-empty commitment sequence.
pub fn commitment_merkle_root(commitments: &[SccpHubCommitmentV1]) -> Option<H256> {
    let mut level: Vec<H256> = commitments.iter().map(commitment_leaf_hash).collect();
    if level.is_empty() {
        return None;
    }
    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        let mut idx = 0usize;
        while idx < level.len() {
            let left = level[idx];
            if let Some(right) = level.get(idx + 1) {
                next.push(hash_merkle_node(&left, right));
            } else {
                next.push(left);
            }
            idx += 2;
        }
        level = next;
    }
    level.first().copied()
}
/// Build the canonical sibling path for one indexed SCCP commitment.
pub fn commitment_merkle_proof(
    commitments: &[SccpHubCommitmentV1],
    index: usize,
) -> Option<SccpMerkleProofV1> {
    if index >= commitments.len() {
        return None;
    }
    let mut level: Vec<H256> = commitments.iter().map(commitment_leaf_hash).collect();
    let mut current_index = index;
    let mut steps = Vec::new();
    while level.len() > 1 {
        if current_index.is_multiple_of(2) {
            if let Some(sibling_hash) = level.get(current_index + 1) {
                steps.push(SccpMerkleStepV1 {
                    sibling_hash: *sibling_hash,
                    sibling_is_left: false,
                });
            }
        } else if let Some(sibling_hash) = level.get(current_index - 1) {
            steps.push(SccpMerkleStepV1 {
                sibling_hash: *sibling_hash,
                sibling_is_left: true,
            });
        }
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        let mut idx = 0usize;
        while idx < level.len() {
            let left = level[idx];
            if let Some(right) = level.get(idx + 1) {
                next.push(hash_merkle_node(&left, right));
            } else {
                next.push(left);
            }
            idx += 2;
        }
        level = next;
        current_index /= 2;
    }
    Some(SccpMerkleProofV1 { steps })
}
/// Decode one canonical, size-bounded Taira bridge-finality proof.
pub fn decode_taira_bridge_finality_proof(
    proof_bytes: &[u8],
) -> Option<TairaBridgeFinalityProofV1> {
    decode_canonical_taira_proof_artifact(proof_bytes)
}
fn hash_block_header_for_sccp_finality(header: &BlockHeader) -> H256 {
    let mut out = [0u8; 32];
    out.copy_from_slice(header.hash().as_ref().as_ref());
    out
}
/// Decode one canonical, size-bounded Taira SCCP message bundle.
pub fn decode_taira_sccp_message_proof(proof_bytes: &[u8]) -> Option<TairaSccpMessageProofV1> {
    decode_canonical_taira_proof_artifact(proof_bytes)
}
fn decode_canonical_taira_proof_artifact<T>(proof_bytes: &[u8]) -> Option<T>
where
    T: for<'de> norito::NoritoDeserialize<'de> + norito::NoritoSerialize,
{
    if !preflight_uncompressed_norito_frame(proof_bytes, SCCP_TAIRA_MAX_ENCODED_PROOF_BYTES_V1) {
        return None;
    }
    let artifact = norito::decode_from_bytes(proof_bytes).ok()?;
    (to_bytes(&artifact).ok()?.as_slice() == proof_bytes).then_some(artifact)
}
fn preflight_uncompressed_norito_frame(bytes: &[u8], maximum: usize) -> bool {
    if bytes.is_empty()
        || bytes.len() > maximum
        || bytes.len() < norito::core::Header::SIZE
        || bytes.get(..4) != Some(b"NRT0")
        || bytes.get(SCCP_NORITO_COMPRESSION_OFFSET) != Some(&0)
    {
        return false;
    }
    bytes
        .get(SCCP_NORITO_LENGTH_OFFSET..SCCP_NORITO_LENGTH_OFFSET + 8)
        .and_then(|raw| <[u8; 8]>::try_from(raw).ok())
        .map(u64::from_le_bytes)
        .is_some_and(|declared| declared <= maximum as u64)
}
/// Verify the canonical structure and quorum-certificate binding of Taira finality.
pub fn verify_taira_bridge_finality_proof_structure(proof: &TairaBridgeFinalityProofV1) -> bool {
    let artifact = &proof.finality_artifact;
    let roster_len = artifact.height_context.roster.len();
    let Some(commitment_root) = proof.block_header.sccp_commitment_root() else {
        return false;
    };
    let Ok(block_header_bytes) = to_bytes(&proof.block_header) else {
        return false;
    };
    if proof.version != BRIDGE_FINALITY_PROOF_VERSION_V2
        || artifact.height_context.network_id != sccp_taira_finality_network_id_v1()
        || block_header_bytes.len() > SCCP_TAIRA_MAX_BLOCK_HEADER_BYTES_V1
        || !preflight_uncompressed_norito_frame(
            &block_header_bytes,
            SCCP_TAIRA_MAX_BLOCK_HEADER_BYTES_V1,
        )
        || !h256_is_nonzero(&commitment_root)
        || proof.block_header.merkle_root().is_none()
        || proof.block_header.result_merkle_root().is_none()
        || roster_len == 0
        || roster_len > SCCP_TAIRA_MAX_FINALITY_VALIDATORS_V1
        || artifact.validator_set_pops.len() != roster_len
        || artifact
            .validator_set_pops
            .iter()
            .any(|pop| pop.is_empty() || pop.len() > SCCP_TAIRA_MAX_BLS_PROOF_BYTES_V1)
        || artifact.commit_qc.aggregate_signature.is_empty()
        || artifact.commit_qc.aggregate_signature.len() > SCCP_TAIRA_MAX_BLS_PROOF_BYTES_V1
        || artifact
            .commit_qc
            .aggregate_signature
            .iter()
            .all(|byte| *byte == 0)
    {
        return false;
    }
    artifact.validate_for_header(&proof.block_header).is_ok()
}
/// Verify a Taira finality proof's canonical header, validator set, `PoPs`, and commit signature.
pub fn verify_taira_bridge_finality_proof_cryptographic(
    proof: &TairaBridgeFinalityProofV1,
) -> bool {
    if !verify_taira_bridge_finality_proof_structure(proof) {
        return false;
    }
    count_sccp_destination_bls_verification_v1();
    iroha_data_model::bridge::verify_bridge_finality_proof(
        proof,
        &sccp_taira_finality_network_id_v1(),
    )
    .is_ok()
}
/// Decode and structurally verify the Taira finality proof for a SORA-origin message.
///
/// External-origin messages are deliberately rejected: first-release inbound
/// admission accepts only the closed protocol-native proof API.
pub fn verified_sccp_message_taira_finality_proof(
    bundle: &TairaSccpMessageProofV1,
) -> Option<TairaBridgeFinalityProofV1> {
    let finality_proof = decode_taira_bridge_finality_proof(&bundle.finality_proof)?;
    verify_message_bundle_structure_with_finality(bundle, &finality_proof).then_some(finality_proof)
}
/// Decode and cryptographically verify a proof-controlled Taira v2 artifact.
///
/// This establishes internal cryptographic consistency for the complete frozen v2 context, exact
/// equal-vote quorum, `PoPs`, and exact commit-vote transcript. The context and roster are still
/// carried by the proof, so callers MUST NOT treat this function as a trust anchor. Production
/// destination proofs additionally bind an audited semantic circuit to a governed
/// [`SccpSoraFinalityAnchorV1`]. BLS verification is mandatory in every build of this crate.
pub fn verified_sccp_message_taira_finality_proof_cryptographically_self_consistent(
    bundle: &TairaSccpMessageProofV1,
) -> Option<TairaBridgeFinalityProofV1> {
    let finality_proof = decode_taira_bridge_finality_proof(&bundle.finality_proof)?;
    (verify_message_bundle_structure_with_finality(bundle, &finality_proof)
        && verify_taira_bridge_finality_proof_cryptographic(&finality_proof))
    .then_some(finality_proof)
}
fn sccp_message_finality_public_inputs_with_finality(
    bundle: &TairaSccpMessageProofV1,
    proof: &TairaBridgeFinalityProofV1,
) -> Option<(u64, H256)> {
    if sccp_message_source_domain(&bundle.payload) != SCCP_DOMAIN_SORA {
        return None;
    }
    if bundle.commitment.context.lane.source != SccpNetworkV1::SoraTaira {
        return None;
    }
    if !verify_taira_bridge_finality_proof_structure(proof)
        || proof.block_header.sccp_commitment_root() != Some(bundle.commitment_root)
    {
        return None;
    }
    Some((
        proof.finality_artifact.height,
        hash_block_header_for_sccp_finality(&proof.block_header),
    ))
}
/// Verify one SORA-origin outbound message bundle.
///
/// External-origin bundles are intentionally outside this API. They must be
/// admitted through `SccpNativeInboundMessageProofV1`, whose closed variant
/// selects and verifies the corresponding protocol-native source proof.
pub fn verify_message_bundle_structure(bundle: &TairaSccpMessageProofV1) -> bool {
    let Some(finality) = decode_taira_bridge_finality_proof(&bundle.finality_proof) else {
        return false;
    };
    verify_message_bundle_structure_with_finality(bundle, &finality)
}
fn verify_message_bundle_structure_with_finality(
    bundle: &TairaSccpMessageProofV1,
    finality: &TairaBridgeFinalityProofV1,
) -> bool {
    if bundle.version != 1
        || bundle.commitment.version != 1
        || sccp_message_source_domain(&bundle.payload) != SCCP_DOMAIN_SORA
        || bundle.merkle_proof.steps.len() > SCCP_TAIRA_MAX_MERKLE_PROOF_STEPS_V1
    {
        return false;
    }
    let target_domain = sccp_message_target_domain(&bundle.payload);
    let Some(expected_commitment) =
        hub_commitment_from_sccp_payload(bundle.commitment.context, &bundle.payload)
    else {
        return false;
    };
    if !verify_sccp_payload_structure(&bundle.payload)
        || target_domain == SCCP_DOMAIN_SORA
        || bundle.commitment != expected_commitment
        || merkle_root_from_commitment(&bundle.commitment, &bundle.merkle_proof)
            != bundle.commitment_root
    {
        return false;
    }
    sccp_message_finality_public_inputs_with_finality(bundle, finality).is_some()
}
fn prefixed_keccak(prefix: &[u8], payload: &[u8]) -> H256 {
    let mut keccak = tiny_keccak::Keccak::v256();
    keccak.update(prefix);
    keccak.update(payload);
    let mut out = [0u8; 32];
    keccak.finalize(&mut out);
    out
}
fn keccak256_bytes(payload: &[u8]) -> H256 {
    let mut keccak = tiny_keccak::Keccak::v256();
    keccak.update(payload);
    let mut out = [0u8; 32];
    keccak.finalize(&mut out);
    out
}
fn abi_word_u32(value: u32) -> H256 {
    let mut word = [0u8; 32];
    word[28..].copy_from_slice(&value.to_be_bytes());
    word
}
fn abi_word_u64(value: u64) -> H256 {
    let mut word = [0u8; 32];
    word[24..].copy_from_slice(&value.to_be_bytes());
    word
}
fn prefixed_blake2b(prefix: &[u8], payload: &[u8]) -> H256 {
    let mut hasher = Blake2bVar::new(32).expect("fixed hash length");
    hasher.update(prefix);
    hasher.update(payload);
    let mut out = [0u8; 32];
    hasher
        .finalize_variable(&mut out)
        .expect("fixed hash length");
    out
}
fn hash_merkle_node(left: &H256, right: &H256) -> H256 {
    let mut hasher = Blake2bVar::new(32).expect("fixed hash length");
    hasher.update(SCCP_HUB_NODE_PREFIX_V1);
    hasher.update(left);
    hasher.update(right);
    let mut out = [0u8; 32];
    hasher
        .finalize_variable(&mut out)
        .expect("fixed hash length");
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    use halo2curves::{
        Coordinates, CurveAffine,
        bn256::{Fq, Fq2, Fr, G1Affine, G2Affine},
        group::{Curve, prime::PrimeCurveAffine},
    };
    use iroha_data_model::{
        account::{AccountId, MultisigMember, MultisigPolicy},
        bridge::{
            BridgeProofPayload, BridgeSccpDestinationProofBackendV1, BridgeTransparentProof,
            SCCP_V1_TAIRA_TO_TOKEN_MULTIPLIER, SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
            SccpBn254G1PointV1, SccpBn254G2PointV1, SccpDestinationDeploymentV1,
            SccpEvmDestinationDeploymentV1, SccpEvmSourceEmitterV1, SccpGovernedRouteV1,
            SccpGroth16Bls12381IcV1, SccpGroth16Bls12381SemanticCircuitV1, SccpGroth16Bn254IcV1,
            SccpGroth16Bn254VerifyingKeyV1, SccpInboundFinalityCutoffV1, SccpLaneIdV1,
            SccpNetworkV1, SccpOutboundMessageContextV1, SccpRouteActivationV1,
            SccpSoraSettlementV1, SccpSourceEmitterV1, SccpSourceIdentityV1,
            SccpTonSourceEmitterV1, SccpTronDestinationDeploymentV1,
            sccp_exact_tron_xor_route_config_hash_v1, sccp_lane_id_hash_v1,
            sccp_v1_taira_xor_asset_definition_id,
        },
        proof::ProofBox,
    };
    use std::{cell::Cell, sync::OnceLock};
    const TEST_MAX_OUTSTANDING_LIABILITY: u128 = 1_000_000_000_000;
    struct OutboundFixture {
        route: SccpGovernedRouteV1,
        bundle: TairaSccpMessageProofV1,
        request: SccpGroth16Bn254ProofRequestV1,
        artifact: SccpGroth16Bn254ProofArtifactV1,
        bridge_proof: BridgeSccpDestinationProofV1,
    }
    #[test]
    fn taira_finality_network_id_matches_the_governed_genesis_vector() {
        assert_eq!(
            sccp_taira_finality_network_id_v1().to_string(),
            SCCP_TAIRA_FINALITY_NETWORK_ID_V1
        );
    }
    fn word_u64(value: u64) -> H256 {
        let mut word = [0; 32];
        word[24..].copy_from_slice(&value.to_be_bytes());
        word
    }
    fn trusted_finality(bundle: &TairaSccpMessageProofV1) -> TairaBridgeFinalityProofV1 {
        decode_taira_bridge_finality_proof(&bundle.finality_proof)
            .expect("test bundle carries canonical Taira finality")
    }
    fn hex32(value: &str) -> H256 {
        decode_fixed_hex_bytes(value).expect("lowercase 32-byte test vector")
    }
    fn fq_word(value: Fq) -> H256 {
        let repr = value.to_repr();
        let mut word = [0; 32];
        for (output, input) in word.iter_mut().zip(repr.as_ref().iter().rev()) {
            *output = *input;
        }
        word
    }
    fn g1_words(point: G1Affine) -> [H256; 2] {
        let coordinates: Coordinates<G1Affine> =
            Option::from(point.coordinates()).expect("non-infinity G1 point");
        [fq_word(*coordinates.x()), fq_word(*coordinates.y())]
    }
    fn g2_model(point: G2Affine) -> SccpBn254G2PointV1 {
        let coordinates: Coordinates<G2Affine> =
            Option::from(point.coordinates()).expect("non-infinity G2 point");
        SccpBn254G2PointV1 {
            x_c0: fq_word(*coordinates.x().c0()),
            x_c1: fq_word(*coordinates.x().c1()),
            y_c0: fq_word(*coordinates.y().c0()),
            y_c1: fq_word(*coordinates.y().c1()),
        }
    }
    fn g1_model() -> SccpBn254G1PointV1 {
        SccpBn254G1PointV1 {
            x: word_u64(1),
            y: word_u64(2),
        }
    }
    fn g2_model_generator() -> SccpBn254G2PointV1 {
        SccpBn254G2PointV1 {
            x_c0: hex32("1800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed"),
            x_c1: hex32("198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c2"),
            y_c0: hex32("12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daa"),
            y_c1: hex32("090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975b"),
        }
    }
    fn verifying_key() -> SccpGroth16Bn254VerifyingKeyV1 {
        let g1 = g1_model();
        let g2 = g2_model_generator();
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
    fn bls12381_g1_bytes(point: Bls12381G1Affine) -> [u8; 48] {
        let encoded = point.to_bytes();
        let mut bytes = [0; 48];
        bytes.copy_from_slice(encoded.as_ref());
        bytes
    }
    fn bls12381_g2_bytes(point: Bls12381G2Affine) -> [u8; 96] {
        let encoded = point.to_bytes();
        let mut bytes = [0; 96];
        bytes.copy_from_slice(encoded.as_ref());
        bytes
    }
    fn bls12381_fr_be_word(value: Bls12381Fr) -> H256 {
        let encoded = value.to_repr();
        let mut word = [0; 32];
        for (output, input) in word.iter_mut().zip(encoded.as_ref().iter().rev()) {
            *output = *input;
        }
        word
    }
    fn bls12381_verifying_key() -> SccpGroth16Bls12381VerifyingKeyV1 {
        let g1 = bls12381_g1_bytes(Bls12381G1Affine::generator());
        let g2 = bls12381_g2_bytes(Bls12381G2Affine::generator());
        SccpGroth16Bls12381VerifyingKeyV1 {
            version: 1,
            alpha1: g1,
            beta2: g2,
            gamma2: g2,
            delta2: g2,
            ic: SccpGroth16Bls12381IcV1 {
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
    fn valid_bls12381_proof(
        public_signals: &SccpGroth16Bls12381PublicSignalsV1,
    ) -> SccpGroth16Bls12381ProofV1 {
        let mut a_scalar = Bls12381Fr::from(3_u64);
        for signal in public_signals.words() {
            a_scalar += bls12381_fr_from_be_word(signal).expect("canonical BLS12-381 signal");
        }
        SccpGroth16Bls12381ProofV1 {
            a: SccpBls12381G1CompressedV1 {
                bytes: bls12381_g1_bytes((Bls12381G1Affine::generator() * a_scalar).to_affine())
                    .to_vec(),
            },
            b: SccpBls12381G2CompressedV1 {
                bytes: bls12381_g2_bytes(Bls12381G2Affine::generator()).to_vec(),
            },
            c: SccpBls12381G1CompressedV1 {
                bytes: bls12381_g1_bytes(Bls12381G1Affine::generator()).to_vec(),
            },
        }
    }
    fn read_test_ton_boc_uint(bytes: &[u8], cursor: &mut usize, width: usize) -> usize {
        let end = cursor
            .checked_add(width)
            .expect("test BOC cursor fits usize");
        let value = bytes[*cursor..end]
            .iter()
            .fold(0usize, |value, byte| (value << 8) | usize::from(*byte));
        *cursor = end;
        value
    }
    fn decode_test_ton_boc_cells(bytes: &[u8]) -> Vec<TonBocCellV1> {
        assert!(bytes.starts_with(&SCCP_TON_BOC_MAGIC_V1));
        let mut cursor = SCCP_TON_BOC_MAGIC_V1.len();
        let flags = bytes[cursor];
        cursor += 1;
        assert_eq!(flags & 0xf8, 0, "fixture BOC must not use optional framing");
        let size_bytes = usize::from(flags & 0x07);
        let offset_bytes = usize::from(bytes[cursor]);
        cursor += 1;
        let cell_count = read_test_ton_boc_uint(bytes, &mut cursor, size_bytes);
        assert_eq!(read_test_ton_boc_uint(bytes, &mut cursor, size_bytes), 1);
        assert_eq!(read_test_ton_boc_uint(bytes, &mut cursor, size_bytes), 0);
        let total_cell_bytes = read_test_ton_boc_uint(bytes, &mut cursor, offset_bytes);
        assert_eq!(read_test_ton_boc_uint(bytes, &mut cursor, size_bytes), 0);
        let cells_end = cursor
            .checked_add(total_cell_bytes)
            .expect("test BOC length fits usize");
        let mut cells = Vec::with_capacity(cell_count);
        for _ in 0..cell_count {
            let descriptor_one = bytes[cursor];
            let descriptor_two = bytes[cursor + 1];
            cursor += 2;
            assert_eq!(descriptor_one & 0xf8, 0, "only ordinary level-zero cells");
            assert_eq!(
                descriptor_two & 1,
                0,
                "all SCCP body cells are byte-aligned"
            );
            let ref_count = usize::from(descriptor_one & 0x07);
            let data_len = usize::from(descriptor_two / 2);
            let data_end = cursor
                .checked_add(data_len)
                .expect("test BOC data length fits usize");
            let data = bytes[cursor..data_end].to_vec();
            cursor = data_end;
            let refs = (0..ref_count)
                .map(|_| read_test_ton_boc_uint(bytes, &mut cursor, size_bytes))
                .collect();
            cells.push(TonBocCellV1 { data, refs });
        }
        assert_eq!(cursor, cells_end);
        assert_eq!(cursor, bytes.len());
        cells
    }
    fn outbound_proof_policy() -> SccpOutboundProofPolicyV1 {
        SccpOutboundProofPolicyV1 {
            version: 1,
            semantic_profile: SccpSemanticProofProfileV1::SoraTairaFinalityInclusionGroth16Bn254(
                SccpGroth16Bn254SemanticCircuitV1 {
                    version: 1,
                    circuit_commitment: [0x71; 32],
                    witness_generator_commitment: [0x72; 32],
                    public_signal_schema_hash: sccp_groth16_bn254_public_signal_schema_hash_v1(),
                },
            ),
            sora_finality_anchor: SccpSoraFinalityAnchorV1 {
                version: 1,
                source_network: SccpNetworkV1::SoraTaira,
                protocol_version: iroha_data_model::block::consensus_v2::PROTOCOL_VERSION,
                chain_id_hash: sccp_sora_taira_chain_id_hash_v1(),
                checkpoint_height: 5,
                checkpoint_block_hash: [0x73; 32],
                checkpoint_context_id: [0x74; 32],
                checkpoint_finality_artifact_hash: [0x75; 32],
            },
        }
    }
    fn ton_outbound_proof_policy() -> SccpOutboundProofPolicyV1 {
        SccpOutboundProofPolicyV1 {
            version: 1,
            semantic_profile: SccpSemanticProofProfileV1::SoraTairaFinalityInclusionGroth16Bls12381(
                SccpGroth16Bls12381SemanticCircuitV1 {
                    version: 1,
                    circuit_commitment: [0x76; 32],
                    witness_generator_commitment: [0x77; 32],
                    public_signal_schema_hash: sccp_groth16_bls12381_public_signal_schema_hash_v1(),
                },
            ),
            sora_finality_anchor: outbound_proof_policy().sora_finality_anchor,
        }
    }
    #[test]
    fn canonical_text_accepts_exact_i105_and_rejects_unicode_substitutions() {
        let account = AccountId::new(
            KeyPair::try_from_seed(vec![0x91; 32], Algorithm::Ed25519)
                .expect("canonical-text account fixture key")
                .public_key()
                .clone(),
        );
        let canonical = account
            .canonical_i105()
            .expect("canonical-text account fixture has an I105 form");
        assert!(
            !canonical.is_ascii(),
            "fixture must exercise I105's non-ASCII base-105 digits"
        );
        assert_eq!(
            decode_sccp_normalized_codec_value(SCCP_CODEC_CANONICAL_TEXT, canonical.as_bytes()),
            Some(SccpNormalizedCodecValueV1::CanonicalText {
                value: canonical.clone(),
            })
        );
        let test_literal = account
            .to_account_address()
            .and_then(|address| address.to_i105_for_discriminant(0x0171))
            .expect("canonical-text account has a test-discriminant I105 form");
        assert_eq!(
            decode_sccp_normalized_codec_value(SCCP_CODEC_CANONICAL_TEXT, test_literal.as_bytes()),
            Some(SccpNormalizedCodecValueV1::CanonicalText {
                value: test_literal,
            }),
            "canonical SCCP text must preserve the literal's embedded discriminant"
        );
        let mut checksum_substitution = canonical.clone();
        let final_digit = checksum_substitution
            .pop()
            .expect("I105 fixture has checksum digits");
        checksum_substitution.push(if final_digit == '1' { '2' } else { '1' });
        for invalid in [
            checksum_substitution.as_bytes(),
            "ｲ".as_bytes(),
            b"two words".as_slice(),
            b"line\nbreak".as_slice(),
            b"invalid\xffutf8".as_slice(),
        ] {
            assert_eq!(
                decode_sccp_normalized_codec_value(SCCP_CODEC_CANONICAL_TEXT, invalid),
                None
            );
        }
    }
    #[test]
    fn destination_contract_account_policy_is_closed_over_every_multisig_member() {
        let key = |seed: u8, algorithm| {
            KeyPair::try_from_seed(vec![seed; 32], algorithm)
                .expect("destination-controller fixture key")
                .public_key()
                .clone()
        };
        let ed25519 = key(0x31, Algorithm::Ed25519);
        let secp256k1 = key(0x32, Algorithm::Secp256k1);
        let mldsa = key(0x33, Algorithm::MlDsa);
        assert!(sccp_destination_contract_supports_account_v1(
            &AccountId::new(ed25519.clone())
        ));
        assert!(sccp_destination_contract_supports_account_v1(
            &AccountId::new(secp256k1.clone())
        ));
        assert!(!sccp_destination_contract_supports_account_v1(
            &AccountId::new(mldsa.clone())
        ));
        let supported_multisig = MultisigPolicy::new(
            2,
            vec![
                MultisigMember::new(ed25519.clone(), 1).expect("Ed25519 member"),
                MultisigMember::new(secp256k1, 1).expect("secp256k1 member"),
            ],
        )
        .expect("supported mixed-curve policy");
        assert!(sccp_destination_contract_supports_account_v1(
            &AccountId::new_multisig(supported_multisig)
        ));
        let unsupported_multisig = MultisigPolicy::new(
            2,
            vec![
                MultisigMember::new(ed25519, 1).expect("Ed25519 member"),
                MultisigMember::new(mldsa, 1).expect("ML-DSA member"),
            ],
        )
        .expect("valid Rust policy with one unsupported destination member");
        assert!(!sccp_destination_contract_supports_account_v1(
            &AccountId::new_multisig(unsupported_multisig)
        ));
    }
    #[test]
    fn normalized_codec_json_is_closed_and_rejects_ambiguous_objects() {
        let values = [
            SccpNormalizedCodecValueV1::CanonicalText {
                value: "route-v1".into(),
            },
            SccpNormalizedCodecValueV1::EvmAddress20 { bytes: [0x12; 20] },
            SccpNormalizedCodecValueV1::TronAddress21 { bytes: [0x41; 21] },
            SccpNormalizedCodecValueV1::TonAccount36 {
                workchain: 0,
                account: [0x14; 32],
            },
        ];
        for value in values {
            let json = norito::json::to_json(&value).expect("serialize normalized codec value");
            let decoded = norito::json::from_json::<SccpNormalizedCodecValueV1>(&json)
                .expect("deserialize canonical normalized codec value");
            assert_eq!(decoded, value);
        }
        for hostile in [
            r#"{"CanonicalText":{"value":"route-v1","extra":true}}"#,
            r#"{"CanonicalText":{"value":"route-v1","value":"other"}}"#,
            r#"{"CanonicalText":{}}"#,
            r#"{"CanonicalText":{"value":"route-v1"},"EvmAddress20":{"bytes":"0x1212121212121212121212121212121212121212"}}"#,
            r#"{"EvmAddress20":{"bytes":"1212121212121212121212121212121212121212"}}"#,
            r#"{"EvmAddress20":{"bytes":"0x1212121212121212121212121212121212121212","extra":0}}"#,
            r#"{"TronAddress21":{"bytes":"0x414141414141414141414141414141414141414141","unexpected":null}}"#,
            r#"{"TonAccount36":{"workchain":-1,"account":"0x1414141414141414141414141414141414141414141414141414141414141414"}}"#,
            r#"{"TonAccount36":{"workchain":0,"account":"0x0000000000000000000000000000000000000000000000000000000000000000"}}"#,
            r#"{"TonAccount36":{"workchain":0,"account":"0x1414141414141414141414141414141414141414141414141414141414141414","extra":0}}"#,
            r#"{"Unknown":{"value":"route-v1"}}"#,
        ] {
            assert!(
                norito::json::from_json::<SccpNormalizedCodecValueV1>(hostile).is_err(),
                "hostile normalized codec JSON must be rejected: {hostile}"
            );
        }
    }
    #[test]
    fn ton_account36_codec_is_big_endian_basechain_only_and_nonzero() {
        let address = ton_address(0xa5);
        let encoded = canonical_sccp_ton_account36_bytes_v1(address)
            .expect("nonzero basechain TON account encodes");
        assert_eq!(&encoded[..4], &0_i32.to_be_bytes());
        assert_eq!(&encoded[4..], &[0xa5; 32]);
        assert_eq!(decode_sccp_ton_account36_v1(&encoded), Some(address));
        assert_eq!(
            decode_sccp_normalized_codec_value(SCCP_CODEC_TON_ACCOUNT36, &encoded),
            Some(SccpNormalizedCodecValueV1::TonAccount36 {
                workchain: 0,
                account: [0xa5; 32],
            })
        );
        assert!(is_supported_codec(SCCP_CODEC_TON_ACCOUNT36));
        assert_eq!(
            sccp_counterparty_account_codec(SCCP_DOMAIN_TON),
            Some(SCCP_CODEC_TON_ACCOUNT36)
        );
        assert!(SCCP_CORE_REMOTE_DOMAINS.contains(&SCCP_DOMAIN_TON));
        assert!(SCCP_NATIVE_INBOUND_REMOTE_DOMAINS_V1.contains(&SCCP_DOMAIN_TON));
        assert!(SCCP_VALUE_MOVING_OUTBOUND_REMOTE_DOMAINS_V1.contains(&SCCP_DOMAIN_TON));

        let mut hostile = encoded;
        hostile[..4].copy_from_slice(&(-1_i32).to_be_bytes());
        assert!(decode_sccp_ton_account36_v1(&hostile).is_none());
        hostile[..4].copy_from_slice(&1_i32.to_be_bytes());
        assert!(decode_sccp_ton_account36_v1(&hostile).is_none());
        hostile = encoded;
        hostile[4..].fill(0);
        assert!(decode_sccp_ton_account36_v1(&hostile).is_none());
        assert!(decode_sccp_ton_account36_v1(&encoded[..35]).is_none());
        let mut trailing = encoded.to_vec();
        trailing.push(0);
        assert!(decode_sccp_ton_account36_v1(&trailing).is_none());
    }
    #[test]
    fn ton_bls12381_pairing_is_curve_separated_and_canonical() {
        let key = bls12381_verifying_key();
        assert!(sccp_groth16_bls12381_verifying_key_is_well_formed_v1(&key));
        assert_eq!(
            sccp_groth16_bls12381_verifying_key_hash_v1(&key),
            iroha_data_model::bridge::sccp_groth16_bls12381_verifying_key_hash_v1(key).ok()
        );
        let words = core::array::from_fn(|index| {
            bls12381_fr_be_word(Bls12381Fr::from(
                u64::try_from(index + 1).expect("small signal index"),
            ))
        });
        let public_signals = SccpGroth16Bls12381PublicSignalsV1::from(words);
        let proof = valid_bls12381_proof(&public_signals);
        assert!(verify_sccp_groth16_bls12381_pairing_v1(
            &proof,
            &public_signals,
            &key
        ));
        let proof_bytes = canonical_sccp_groth16_bls12381_proof_bytes_v1(&proof)
            .expect("valid proof has canonical bytes");
        assert_eq!(proof_bytes.len(), SCCP_TON_GROTH16_BLS12381_PROOF_BYTES_V1);
        assert_eq!(
            decode_sccp_groth16_bls12381_proof_bytes_v1(&proof_bytes),
            Some(proof.clone())
        );
        assert!(
            decode_sccp_groth16_bls12381_proof_bytes_v1(&proof_bytes[..proof_bytes.len() - 1])
                .is_none()
        );

        let mut changed_signals = public_signals;
        changed_signals.message_id[31] ^= 1;
        assert!(!verify_sccp_groth16_bls12381_pairing_v1(
            &proof,
            &changed_signals,
            &key
        ));
        let mut out_of_field = public_signals;
        out_of_field.message_id = SCCP_GROTH16_BLS12381_SCALAR_FIELD_MODULUS_BE;
        assert!(!verify_sccp_groth16_bls12381_pairing_v1(
            &proof,
            &out_of_field,
            &key
        ));
        let mut identity_proof = proof.clone();
        identity_proof.a.bytes = bls12381_g1_bytes(Bls12381G1Affine::identity()).to_vec();
        assert!(canonical_sccp_groth16_bls12381_proof_bytes_v1(&identity_proof).is_none());
        assert!(!verify_sccp_groth16_bls12381_pairing_v1(
            &identity_proof,
            &public_signals,
            &key
        ));
    }
    #[test]
    fn ton_boc_encoder_is_canonical_and_rejects_invalid_graphs() {
        let one_cell = encode_ton_boc_cells_v1(&[TonBocCellV1 {
            data: vec![0xaa],
            refs: Vec::new(),
        }])
        .expect("one ordinary byte-aligned TON cell encodes");
        assert_eq!(
            one_cell,
            vec![
                0xb5, 0xee, 0x9c, 0x72, 0x01, 0x01, 0x01, 0x01, 0x00, 0x03, 0x00, 0x00, 0x02, 0xaa,
            ]
        );
        assert!(encode_ton_boc_cells_v1(&[]).is_none());
        assert!(
            encode_ton_boc_cells_v1(&[TonBocCellV1 {
                data: vec![0; 128],
                refs: Vec::new(),
            }])
            .is_none()
        );
        assert!(
            encode_ton_boc_cells_v1(&[TonBocCellV1 {
                data: Vec::new(),
                refs: vec![0],
            }])
            .is_none()
        );
    }
    #[test]
    fn transfer_projection_json_preserves_full_integer_ranges_as_canonical_strings() {
        let projection = SccpTransferProjectionV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_TRON,
            nonce: u64::MAX,
            route_revision: u32::MAX,
            asset_home_domain: SCCP_DOMAIN_SORA,
            asset_id: SccpNormalizedCodecValueV1::CanonicalText {
                value: SCCP_TAIRA_XOR_ASSET_KEY_V1.into(),
            },
            amount: u128::MAX,
            sender: SccpNormalizedCodecValueV1::CanonicalText {
                value: "alice".into(),
            },
            recipient: SccpNormalizedCodecValueV1::TronAddress21 { bytes: [0x41; 21] },
            route_id: SccpNormalizedCodecValueV1::CanonicalText {
                value: SCCP_TAIRA_TRON_XOR_ROUTE_ID_V1.into(),
            },
        };
        let json = norito::json::to_json(&projection).expect("serialize transfer projection");
        assert!(json.contains(r#""nonce":"18446744073709551615""#));
        assert!(json.contains(r#""amount":"340282366920938463463374607431768211455""#));
        assert_eq!(
            norito::json::from_json::<SccpTransferProjectionV1>(&json)
                .expect("deserialize full-range transfer projection"),
            projection
        );
        let hostile = [
            json.replace(
                r#""nonce":"18446744073709551615""#,
                r#""nonce":"018446744073709551615""#,
            ),
            json.replace(
                r#""nonce":"18446744073709551615""#,
                r#""nonce":18446744073709551615"#,
            ),
            json.replace(
                r#""amount":"340282366920938463463374607431768211455""#,
                r#""amount":"0340282366920938463463374607431768211455""#,
            ),
            json.replace(
                r#""amount":"340282366920938463463374607431768211455""#,
                r#""amount":1"#,
            ),
        ];
        for value in hostile {
            assert!(
                norito::json::from_json::<SccpTransferProjectionV1>(&value).is_err(),
                "noncanonical projection JSON must be rejected: {value}"
            );
        }
    }
    fn evm_deployment() -> SccpEvmDestinationDeploymentV1 {
        let key = verifying_key();
        SccpEvmDestinationDeploymentV1 {
            token_address: [0x11; 20],
            token_code_hash: [0x21; 32],
            verifier_address: [0x31; 20],
            verifier_code_hash: [0x41; 32],
            verifying_key: key,
            verifier_key_hash: sccp_groth16_bn254_verifying_key_hash_v1(&key)
                .expect("valid repeated-generator key"),
            outbound_proof_policy: outbound_proof_policy(),
            route_address: [0x51; 20],
            route_code_hash: [0x61; 32],
            replay_verifier_address: [0x71; 20],
            replay_verifier_code_hash: [0x72; 32],
            mint_breaker_address: [0x81; 20],
            mint_breaker_code_hash: [0x82; 32],
            taira_to_token_multiplier: SCCP_V1_TAIRA_TO_TOKEN_MULTIPLIER,
            max_wrapped_supply: TEST_MAX_OUTSTANDING_LIABILITY
                * SCCP_V1_TAIRA_TO_TOKEN_MULTIPLIER as u128,
        }
    }
    fn ton_address(byte: u8) -> SccpTonAddressV1 {
        SccpTonAddressV1 {
            workchain: 0,
            account: [byte; 32],
        }
    }
    fn ton_deployment() -> SccpTonDestinationDeploymentV1 {
        let verifying_key = bls12381_verifying_key();
        SccpTonDestinationDeploymentV1 {
            jetton_master_address: ton_address(0x81),
            jetton_master_code_hash: [0x91; 32],
            jetton_master_initial_data_hash: [0x89; 32],
            jetton_wallet_code_hash: [0x92; 32],
            route_address: ton_address(0x82),
            route_code_hash: [0x93; 32],
            route_initial_data_hash: [0x8a; 32],
            embedded_verifier_code_hash: [0x94; 32],
            verifier_circuit_hash: [0x76; 32],
            verifying_key,
            verifier_key_hash: sccp_groth16_bls12381_verifying_key_hash_v1(&verifying_key)
                .expect("valid BLS12-381 key"),
            proof_profile_commitment: sccp_ton_groth16_bls12381_proof_profile_commitment_v1(),
            mint_breaker_guardian_keys: [
                [0xa1; 32], [0xa2; 32], [0xa3; 32], [0xa4; 32], [0xa5; 32],
            ]
            .into(),
            outbound_proof_policy: ton_outbound_proof_policy(),
            taira_to_token_multiplier: SCCP_V1_TAIRA_TO_TON_TOKEN_MULTIPLIER,
            max_wrapped_supply: TEST_MAX_OUTSTANDING_LIABILITY
                * SCCP_V1_TAIRA_TO_TON_TOKEN_MULTIPLIER as u128,
        }
    }
    #[test]
    fn ton_destination_amount_is_bounded_by_coins_domain_and_governed_cap() {
        let deployment = ton_deployment();
        assert_eq!(
            sccp_ton_payload_amount_to_jetton_base_units_v1(
                deployment.max_wrapped_supply,
                &deployment,
            ),
            Some(deployment.max_wrapped_supply)
        );
        assert_eq!(
            sccp_ton_payload_amount_to_jetton_base_units_v1(0, &deployment),
            None
        );
        assert_eq!(
            sccp_ton_payload_amount_to_jetton_base_units_v1(
                deployment.max_wrapped_supply + 1,
                &deployment,
            ),
            None
        );

        let maximum = SccpTonDestinationDeploymentV1 {
            max_wrapped_supply: SCCP_V1_TON_MAX_COINS,
            ..deployment
        };
        assert_eq!(
            sccp_ton_payload_amount_to_jetton_base_units_v1(SCCP_V1_TON_MAX_COINS, &maximum),
            Some(SCCP_V1_TON_MAX_COINS)
        );
        assert_eq!(
            sccp_ton_payload_amount_to_jetton_base_units_v1(
                SCCP_V1_TON_MAX_COINS,
                &SccpTonDestinationDeploymentV1 {
                    max_wrapped_supply: SCCP_V1_TON_MAX_COINS + 1,
                    ..maximum
                },
            ),
            None
        );
    }
    fn governed_route(
        network: SccpNetworkV1,
        revision: u32,
        activation: SccpRouteActivationV1,
    ) -> SccpGovernedRouteV1 {
        let lane_id = SccpLaneIdV1 {
            source: network,
            target: SccpNetworkV1::SoraTaira,
        };
        let route_id = match network.domain_id() {
            SCCP_DOMAIN_ETH => SCCP_TAIRA_ETH_XOR_ROUTE_ID_V1,
            SCCP_DOMAIN_BSC => SCCP_TAIRA_BSC_XOR_ROUTE_ID_V1,
            _ => panic!("EVM fixture requires Ethereum or BSC"),
        };
        let deployment = evm_deployment();
        let destination = SccpDestinationDeploymentV1::Evm(deployment);
        let route_config_hash = destination
            .route_configuration_hash(
                lane_id,
                route_id,
                SCCP_TAIRA_XOR_ASSET_KEY_V1,
                revision,
                SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
            )
            .expect("valid exact EVM route configuration");
        let route = SccpGovernedRouteV1 {
            lane_id,
            route_id: route_id.to_owned(),
            asset_key: SCCP_TAIRA_XOR_ASSET_KEY_V1.to_owned(),
            revision,
            activation,
            inbound_finality_cutoff: None,
            source_identity: SccpSourceIdentityV1 {
                lane: lane_id,
                emitter: SccpSourceEmitterV1::Evm(SccpEvmSourceEmitterV1 {
                    address: deployment.route_address,
                    runtime_code_hash: deployment.route_code_hash,
                    route_config_hash,
                }),
            },
            destination,
            sora_outbound_execution_policy: sccp_sora_outbound_execution_policy_test_fixture_v1(),
            settlement: SccpSoraSettlementV1 {
                asset_definition_id: sccp_v1_taira_xor_asset_definition_id(),
                payload_amount_scale: SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
                max_outstanding_liability: TEST_MAX_OUTSTANDING_LIABILITY,
            },
        };
        route.validate().expect("valid governed EVM fixture route");
        assert_eq!(
            route.route_configuration_hash().expect("route config"),
            route_config_hash
        );
        route
    }
    fn ton_governed_route(revision: u32) -> SccpGovernedRouteV1 {
        let lane_id = SccpLaneIdV1 {
            source: SccpNetworkV1::TonMainnet,
            target: SccpNetworkV1::SoraTaira,
        };
        let deployment = ton_deployment();
        let destination = SccpDestinationDeploymentV1::Ton(deployment);
        let route_config_hash = destination
            .route_configuration_hash(
                lane_id,
                SCCP_TAIRA_TON_XOR_ROUTE_ID_V1,
                SCCP_TAIRA_XOR_ASSET_KEY_V1,
                revision,
                SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
            )
            .expect("valid exact TON route configuration");
        let route = SccpGovernedRouteV1 {
            lane_id,
            route_id: SCCP_TAIRA_TON_XOR_ROUTE_ID_V1.to_owned(),
            asset_key: SCCP_TAIRA_XOR_ASSET_KEY_V1.to_owned(),
            revision,
            activation: SccpRouteActivationV1::Bidirectional,
            inbound_finality_cutoff: None,
            source_identity: SccpSourceIdentityV1 {
                lane: lane_id,
                emitter: SccpSourceEmitterV1::Ton(SccpTonSourceEmitterV1 {
                    address: deployment.route_address,
                    code_hash: deployment.route_code_hash,
                    route_config_hash,
                }),
            },
            destination,
            sora_outbound_execution_policy: sccp_sora_outbound_execution_policy_test_fixture_v1(),
            settlement: SccpSoraSettlementV1 {
                asset_definition_id: sccp_v1_taira_xor_asset_definition_id(),
                payload_amount_scale: SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
                max_outstanding_liability: TEST_MAX_OUTSTANDING_LIABILITY,
            },
        };
        route.validate().expect("valid governed TON fixture route");
        assert_eq!(
            route.route_configuration_hash().expect("route config"),
            route_config_hash
        );
        route
    }
    fn transfer_payload(revision: u32) -> SccpPayloadV1 {
        SccpPayloadV1::Transfer(TransferPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_ETH,
            nonce: 7,
            route_revision: revision,
            asset_home_domain: SCCP_DOMAIN_SORA,
            asset_id_codec: SCCP_CODEC_CANONICAL_TEXT,
            asset_id: SCCP_TAIRA_XOR_ASSET_KEY_V1.as_bytes().to_vec(),
            amount: 123,
            sender_codec: SCCP_CODEC_CANONICAL_TEXT,
            sender: b"alice".to_vec(),
            recipient_codec: SCCP_CODEC_EVM_ADDRESS20,
            recipient: vec![0x91; 20],
            route_id_codec: SCCP_CODEC_CANONICAL_TEXT,
            route_id: SCCP_TAIRA_ETH_XOR_ROUTE_ID_V1.as_bytes().to_vec(),
        })
    }
    fn ton_transfer_payload(revision: u32, recipient: SccpTonAddressV1) -> SccpPayloadV1 {
        SccpPayloadV1::Transfer(TransferPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_TON,
            nonce: 19,
            route_revision: revision,
            asset_home_domain: SCCP_DOMAIN_SORA,
            asset_id_codec: SCCP_CODEC_CANONICAL_TEXT,
            asset_id: SCCP_TAIRA_XOR_ASSET_KEY_V1.as_bytes().to_vec(),
            amount: 3,
            sender_codec: SCCP_CODEC_CANONICAL_TEXT,
            sender: b"alice".to_vec(),
            recipient_codec: SCCP_CODEC_TON_ACCOUNT36,
            recipient: canonical_sccp_ton_account36_bytes_v1(recipient)
                .expect("canonical TON recipient")
                .to_vec(),
            route_id_codec: SCCP_CODEC_CANONICAL_TEXT,
            route_id: SCCP_TAIRA_TON_XOR_ROUTE_ID_V1.as_bytes().to_vec(),
        })
    }
    fn message_bundle(route: &SccpGovernedRouteV1) -> TairaSccpMessageProofV1 {
        message_bundle_with_payload(route, transfer_payload(route.revision))
    }
    fn message_bundle_with_payload(
        route: &SccpGovernedRouteV1,
        payload: SccpPayloadV1,
    ) -> TairaSccpMessageProofV1 {
        let context = SccpOutboundMessageContextV1::new(
            SccpLaneIdV1 {
                source: route.lane_id.target,
                target: route.lane_id.source,
            },
            route
                .destination_binding_hash()
                .expect("destination binding"),
            route.route_configuration_hash().expect("route config"),
        )
        .expect("exact outbound context");
        let commitment =
            hub_commitment_from_sccp_payload(context, &payload).expect("hub commitment");
        let merkle_proof = SccpMerkleProofV1 { steps: Vec::new() };
        let commitment_root = merkle_root_from_commitment(&commitment, &merkle_proof);
        let finality_proof =
            crate::test_fixtures::signed_finality_proof_for_message_test_fixture_v1(
                context,
                &payload,
                commitment_root,
            );
        let bundle = TairaSccpMessageProofV1 {
            version: 1,
            commitment_root,
            commitment,
            merkle_proof,
            payload,
            finality_proof,
        };
        assert!(verify_message_bundle_structure(&bundle));
        assert!(
            verified_sccp_message_taira_finality_proof_cryptographically_self_consistent(&bundle)
                .is_some()
        );
        bundle
    }
    fn valid_proof(request: &SccpGroth16Bn254ProofRequestV1) -> Vec<u8> {
        valid_proof_for_hash_roles(
            request,
            request.statement_hash,
            request.destination_binding_hash,
            request.route_configuration_hash,
            request.sora_finality_anchor_hash,
        )
    }
    fn valid_proof_for_hash_roles(
        request: &SccpGroth16Bn254ProofRequestV1,
        statement_hash: H256,
        destination_binding_hash: H256,
        route_configuration_hash: H256,
        sora_finality_anchor_hash: H256,
    ) -> Vec<u8> {
        let signals = sccp_groth16_bn254_public_signal_words(
            &request.public_inputs,
            request.source_network.domain_id(),
            statement_hash,
            destination_binding_hash,
            route_configuration_hash,
            sora_finality_anchor_hash,
        );
        let mut scalar = Fr::from(3_u64);
        for signal in &signals {
            scalar += bn254_fr_from_abi_word(signal).expect("canonical scalar signal");
        }
        let a = (G1Affine::generator() * scalar).to_affine();
        let proof = SccpEvmGroth16Bn254ProofV1 {
            version: 1,
            message_id: request.public_inputs.message_id,
            source_domain: request.source_network.domain_id(),
            commitment_root: request.public_inputs.commitment_root,
            a: g1_words(a),
            b: [
                request.verifying_key.beta2.x_c0,
                request.verifying_key.beta2.x_c1,
                request.verifying_key.beta2.y_c0,
                request.verifying_key.beta2.y_c1,
            ],
            c: [
                request.verifying_key.alpha1.x,
                request.verifying_key.alpha1.y,
            ],
        };
        encode_sccp_evm_groth16_bn254_proof_bytes(&proof)
    }
    fn fixture() -> &'static OutboundFixture {
        static FIXTURE: OnceLock<OutboundFixture> = OnceLock::new();
        FIXTURE.get_or_init(|| {
            let route = governed_route(
                SccpNetworkV1::EthereumMainnet,
                1,
                SccpRouteActivationV1::Bidirectional,
            );
            let bundle = message_bundle(&route);
            let request =
                build_sccp_groth16_bn254_proof_request_from_governed_route_v1(&bundle, &route)
                    .expect("canonical governed request");
            let proof_bytes = valid_proof(&request);
            assert!(verify_sccp_groth16_bn254_proof_v1(&request, &proof_bytes,));
            let artifact = wrap_sccp_evm_groth16_bn254_proof_result(&proof_bytes, &request)
                .expect("valid Groth16 artifact");
            let bridge_proof =
                bridge_sccp_destination_proof_v1(&artifact).expect("closed bridge proof");
            assert!(
                verify_sccp_destination_proof_v1(
                    &bridge_proof,
                    &bundle,
                    &route,
                    &trusted_finality(&bundle),
                )
                .is_some()
            );
            OutboundFixture {
                route,
                bundle,
                request,
                artifact,
                bridge_proof,
            }
        })
    }
    #[test]
    fn ton_destination_path_roundtrips_bls_artifact_and_derives_boc() {
        let route = ton_governed_route(1);
        let recipient = ton_address(0xa6);
        let bundle =
            message_bundle_with_payload(&route, ton_transfer_payload(route.revision, recipient));
        assert!(
            build_sccp_groth16_bn254_proof_request_from_governed_route_v1(&bundle, &route)
                .is_none(),
            "TON material must never enter the BN254 request path"
        );
        let request =
            build_sccp_ton_groth16_bls12381_proof_request_from_governed_route_v1(&bundle, &route)
                .expect("canonical governed TON request");
        assert_eq!(
            request.backend,
            BridgeSccpDestinationProofBackendV1::TonGroth16Bls12381
        );
        assert!(
            request
                .public_signals
                .words()
                .iter()
                .all(|word| word < &SCCP_GROTH16_BLS12381_SCALAR_FIELD_MODULUS_BE)
        );
        let request_bytes = encode_canonical_sccp_ton_groth16_bls12381_proof_request_v1(&request)
            .expect("canonical TON request bytes");
        assert_eq!(
            decode_canonical_sccp_ton_groth16_bls12381_proof_request_v1(&request_bytes),
            Some(request.clone())
        );

        let raw_proof = valid_bls12381_proof(&request.public_signals);
        let raw_proof_bytes = canonical_sccp_groth16_bls12381_proof_bytes_v1(&raw_proof)
            .expect("valid TON proof bytes");
        let artifact = wrap_sccp_ton_groth16_bls12381_proof_result_v1(&raw_proof_bytes, &request)
            .expect("pairing-valid TON artifact");
        let artifact_bytes =
            encode_canonical_sccp_ton_groth16_bls12381_proof_artifact_v1(&artifact)
                .expect("canonical TON artifact bytes");
        assert_eq!(
            decode_canonical_sccp_ton_groth16_bls12381_proof_artifact_v1(&artifact_bytes),
            Some(artifact.clone())
        );
        let bridge_proof =
            bridge_sccp_ton_destination_proof_v1(&artifact).expect("closed TON bridge proof");
        let finality = trusted_finality(&bundle);
        reset_sccp_destination_proof_work_counters_v1();
        let parsed = parse_sccp_destination_proof_v1(&bridge_proof)
            .expect("generic parser accepts canonical TON artifact");
        assert_eq!(
            parsed.backend(),
            BridgeSccpDestinationProofBackendV1::TonGroth16Bls12381
        );
        assert_eq!(
            parsed.crypto_work(),
            SccpDestinationProofCryptoWorkV1::Groth16Bls12381Pairing
        );
        assert_eq!(
            parsed.artifact().result.result_hash,
            artifact.result.result_hash
        );
        assert_eq!(
            sccp_destination_proof_work_counters_v1(),
            SccpDestinationProofWorkCountersV1 {
                artifact_framing_decodes: 1,
                bundle_decodes: 1,
                groth16_pairings: 0,
                bls_verifications: 0,
                bls12381_point_decodes: 0,
            }
        );
        let generic = verify_parsed_sccp_destination_proof_v1(parsed, &route, &finality)
            .expect("generic route verifier accepts TON with one BLS12-381 pairing");
        assert_eq!(generic.backend, bridge_proof.backend);
        assert!(matches!(
            generic.target,
            SccpDestinationCallTargetV1::Ton {
                network: SccpNetworkV1::TonMainnet,
                route_address,
            } if route_address == ton_address(0x82)
        ));
        assert!(generic.calldata.starts_with(&SCCP_TON_BOC_MAGIC_V1));
        let work = sccp_destination_proof_work_counters_v1();
        assert_eq!(work.artifact_framing_decodes, 1);
        assert_eq!(work.bundle_decodes, 1);
        assert_eq!(work.groth16_pairings, 1);
        assert_eq!(work.bls_verifications, 1);
        assert!(
            work.bls12381_point_decodes > 0,
            "route-bound TON verification must perform metered point validation"
        );
        let call = verify_sccp_ton_destination_proof_v1(
            &bridge_proof,
            &bundle,
            &route,
            &finality,
            0x0102_0304_0506_0708,
        )
        .expect("verified TON internal-message call");
        assert_eq!(call.network, SccpNetworkV1::TonMainnet);
        assert_eq!(call.route_address, ton_address(0x82));
        assert_eq!(call.recipient, recipient);
        assert_eq!(call.amount, 3);
        assert_eq!(call.proof_bytes, raw_proof_bytes);
        assert!(
            call.internal_message_body_boc
                .starts_with(&SCCP_TON_BOC_MAGIC_V1)
        );
        let cells = decode_test_ton_boc_cells(&call.internal_message_body_boc);
        assert_eq!(cells.len(), 14);
        assert_eq!(cells[0].refs, vec![1, 5, 9]);
        assert_eq!(cells[0].data.len(), 78);
        assert_eq!(
            &cells[0].data[..4],
            &SCCP_TON_FINALIZE_FROM_TAIRA_OPCODE_V1.to_be_bytes()
        );
        assert_eq!(&cells[0].data[4..12], &call.query_id.to_be_bytes());
        assert_eq!(&cells[0].data[12..14], &1_u16.to_be_bytes());
        assert_eq!(&cells[0].data[14..46], &request.public_inputs.message_id);
        assert_eq!(&cells[0].data[46..], &request.statement_hash);

        let mut signal_bytes = Vec::with_capacity(11 * 32);
        for word in request.public_signals.words() {
            signal_bytes.extend_from_slice(&word);
        }
        assert_eq!(cells[1].refs, vec![2]);
        assert_eq!(cells[1].data, signal_bytes[..96]);
        assert_eq!(cells[2].refs, vec![3]);
        assert_eq!(cells[2].data, signal_bytes[96..192]);
        assert_eq!(cells[3].refs, vec![4]);
        assert_eq!(cells[3].data, signal_bytes[192..288]);
        assert!(cells[4].refs.is_empty());
        assert_eq!(cells[4].data, signal_bytes[288..]);

        assert_eq!(cells[5].refs, vec![6, 7, 8]);
        assert!(cells[5].data.is_empty());
        assert_eq!(cells[6].data, raw_proof_bytes[..48]);
        assert_eq!(cells[7].data, raw_proof_bytes[48..144]);
        assert_eq!(cells[8].data, raw_proof_bytes[144..]);
        assert!(cells[6..=8].iter().all(|cell| cell.refs.is_empty()));

        let payload_bytes = canonical_sccp_payload_bytes(&bundle.payload)
            .expect("TON fixture carries canonical payload bytes");
        let remainder = &payload_bytes[SCCP_TON_CANONICAL_PAYLOAD_HEADER_BYTES_V1..];
        let first_end = remainder
            .len()
            .min(SCCP_TON_CANONICAL_PAYLOAD_CHUNK_BYTES_V1);
        let second_end = remainder
            .len()
            .min(2 * SCCP_TON_CANONICAL_PAYLOAD_CHUNK_BYTES_V1);
        assert_eq!(cells[9].refs, vec![10, 11, 12, 13]);
        assert!(cells[9].data.is_empty());
        assert_eq!(
            cells[10].data,
            payload_bytes[..SCCP_TON_CANONICAL_PAYLOAD_HEADER_BYTES_V1]
        );
        assert_eq!(cells[11].data, remainder[..first_end]);
        assert_eq!(cells[12].data, remainder[first_end..second_end]);
        assert_eq!(cells[13].data, remainder[second_end..]);
        assert!(cells[10..=13].iter().all(|cell| cell.refs.is_empty()));

        let mut untrusted_finality = finality.clone();
        untrusted_finality.finality_artifact.height += 1;
        reset_sccp_destination_proof_work_counters_v1();
        assert!(
            verify_sccp_ton_destination_proof_v1(
                &bridge_proof,
                &bundle,
                &route,
                &untrusted_finality,
                call.query_id,
            )
            .is_none()
        );
        let mismatch_work = sccp_destination_proof_work_counters_v1();
        assert_eq!(mismatch_work.groth16_pairings, 0);
        assert_eq!(mismatch_work.bls_verifications, 0);

        let mut wrong_outer = bridge_proof.clone();
        wrong_outer.route_configuration_hash[0] ^= 1;
        assert!(
            verify_sccp_ton_destination_proof_v1(
                &wrong_outer,
                &bundle,
                &route,
                &finality,
                call.query_id,
            )
            .is_none()
        );
        let mut wrong_profile = request.clone();
        wrong_profile.proof_profile_commitment[0] ^= 1;
        assert!(
            encode_canonical_sccp_ton_groth16_bls12381_proof_request_v1(&wrong_profile).is_none()
        );
        let mut wrong_key = request;
        wrong_key.verifying_key.alpha1 = bls12381_g1_bytes(Bls12381G1Affine::identity());
        assert!(encode_canonical_sccp_ton_groth16_bls12381_proof_request_v1(&wrong_key).is_none());
    }
    #[test]
    fn ton_parse_defers_curve_validation_until_route_bound_verification() {
        let route = ton_governed_route(1);
        let bundle = message_bundle_with_payload(
            &route,
            ton_transfer_payload(route.revision, ton_address(0xa6)),
        );
        let request =
            build_sccp_ton_groth16_bls12381_proof_request_from_governed_route_v1(&bundle, &route)
                .expect("canonical governed TON request");
        let valid_proof = valid_bls12381_proof(&request.public_signals);
        let valid_proof_bytes = canonical_sccp_groth16_bls12381_proof_bytes_v1(&valid_proof)
            .expect("valid TON proof bytes");
        let mut artifact =
            wrap_sccp_ton_groth16_bls12381_proof_result_v1(&valid_proof_bytes, &request)
                .expect("pairing-valid TON artifact");

        let off_curve = (0_u16..=u16::MAX)
            .find_map(|candidate| {
                let mut encoded = vec![0_u8; 48];
                encoded[0] = 0x80 | u8::try_from(candidate >> 8).ok()?;
                encoded[47] = candidate as u8;
                (bls12381_g1_compressed_is_structurally_canonical_v1(&encoded)
                    && bls12381_g1_affine(&encoded).is_none())
                .then_some(encoded)
            })
            .expect("the compressed field domain contains an off-curve x coordinate");
        artifact.result.proof.a.bytes = off_curve;
        artifact.result.result_hash = sccp_ton_groth16_bls12381_result_hash_v1(
            artifact.result.request_hash,
            &artifact.result.proof,
        )
        .expect("structurally canonical off-curve proof has an exact wire commitment");
        let bridge_proof = BridgeSccpDestinationProofV1 {
            backend: BridgeSccpDestinationProofBackendV1::TonGroth16Bls12381,
            route_configuration_hash: artifact.request.route_configuration_hash,
            encoded_artifact: to_bytes(&artifact).expect("adversarial artifact encodes"),
        };

        reset_sccp_destination_proof_work_counters_v1();
        let parsed = parse_sccp_destination_proof_v1(&bridge_proof)
            .expect("parse accepts fixed-shape proof bytes without curve work");
        let parse_work = sccp_destination_proof_work_counters_v1();
        assert_eq!(parse_work.artifact_framing_decodes, 1);
        assert_eq!(parse_work.bundle_decodes, 1);
        assert_eq!(parse_work.groth16_pairings, 0);
        assert_eq!(parse_work.bls_verifications, 0);
        assert_eq!(
            parse_work.bls12381_point_decodes, 0,
            "attacker-controlled subgroup work must be deferred until after Core reserves quota"
        );

        assert!(
            verify_parsed_sccp_destination_proof_v1(parsed, &route, &trusted_finality(&bundle),)
                .is_none(),
            "route-bound verification must reject the off-curve proof"
        );
        let verification_work = sccp_destination_proof_work_counters_v1();
        assert_eq!(verification_work.groth16_pairings, 1);
        assert!(verification_work.bls12381_point_decodes > 0);
    }
    fn assert_request_rejected(request: &SccpGroth16Bn254ProofRequestV1) {
        assert!(encode_canonical_sccp_groth16_bn254_proof_request_v1(request).is_none());
        let bytes = to_bytes(request).expect("encode adversarial request");
        assert!(decode_canonical_sccp_groth16_bn254_proof_request_v1(&bytes).is_none());
    }
    fn rehash_request_after_proof_policy_mutation(
        request: &mut SccpGroth16Bn254ProofRequestV1,
        canonical_payload_bytes: &[u8],
    ) {
        request.semantic_proof_profile_hash =
            sccp_semantic_proof_profile_hash_v1(request.semantic_proof_profile)
                .expect("individually valid semantic profile");
        request.sora_finality_anchor_hash =
            sccp_sora_finality_anchor_hash_v1(request.sora_finality_anchor)
                .expect("individually valid finality anchor");
        request.statement_hash =
            sccp_groth16_bn254_statement_hash_v1(request, canonical_payload_bytes)
                .expect("policy mutation preserves canonical statement roles");
        let public_signal_words = sccp_groth16_bn254_public_signal_words(
            &request.public_inputs,
            request.source_network.domain_id(),
            request.statement_hash,
            request.destination_binding_hash,
            request.route_configuration_hash,
            request.sora_finality_anchor_hash,
        );
        request.request_hash = sccp_groth16_bn254_proof_request_hash(
            request,
            canonical_payload_bytes,
            &public_signal_words,
        )
        .expect("policy mutation preserves canonical request hashing");
    }
    fn assert_cross_policy_alias_request_rejected(
        mut request: SccpGroth16Bn254ProofRequestV1,
        canonical_payload_bytes: &[u8],
        semantic_role: &str,
        anchor_role: &str,
    ) {
        assert!(
            request.semantic_proof_profile.validate().is_ok(),
            "{semantic_role} alias with {anchor_role} must remain individually valid"
        );
        assert!(
            request.sora_finality_anchor.validate().is_ok(),
            "anchor must remain individually valid for {semantic_role}/{anchor_role}"
        );
        rehash_request_after_proof_policy_mutation(&mut request, canonical_payload_bytes);
        assert!(
            SccpOutboundProofPolicyV1 {
                version: 1,
                semantic_profile: request.semantic_proof_profile,
                sora_finality_anchor: request.sora_finality_anchor,
            }
            .validate()
            .is_err(),
            "full policy accepted {semantic_role}/{anchor_role} alias"
        );
        assert!(
            !sccp_groth16_bn254_proof_request_header_is_canonical_v1(&request, request.backend,),
            "request header accepted {semantic_role}/{anchor_role} alias"
        );
        let request_json = norito::json::to_json(&request).expect("adversarial request JSON");
        assert!(
            decode_canonical_sccp_groth16_bn254_proof_request_json_v1(&request_json).is_none(),
            "JSON decoder accepted {semantic_role}/{anchor_role} alias"
        );
        assert_request_rejected(&request);
    }
    #[test]
    fn exact_outbound_path_roundtrips_and_derives_canonical_calldata() {
        let fixture = fixture();
        let payload_bytes = canonical_sccp_payload_bytes(&fixture.bundle.payload)
            .expect("valid SCCP outbound fixture payload encodes");
        assert_eq!(
            decode_canonical_sccp_payload_bytes(&payload_bytes),
            Some(fixture.bundle.payload.clone())
        );
        let bundle_bytes = canonical_taira_sccp_message_bundle_bytes_checked(&fixture.bundle)
            .expect("canonical bundle");
        assert_eq!(bundle_bytes, fixture.request.bundle_bytes);
        assert!(decode_canonical_taira_sccp_message_bundle_summary(&bundle_bytes).is_some());
        let request_bytes = encode_canonical_sccp_groth16_bn254_proof_request_v1(&fixture.request)
            .expect("canonical request bytes");
        assert_eq!(
            decode_canonical_sccp_groth16_bn254_proof_request_v1(&request_bytes),
            Some(fixture.request.clone())
        );
        let request_json = norito::json::to_json(&fixture.request).expect("request JSON");
        assert_eq!(
            decode_canonical_sccp_groth16_bn254_proof_request_json_v1(&request_json),
            Some(fixture.request.clone())
        );
        let result_bytes =
            encode_canonical_sccp_groth16_bn254_proof_result_v1(&fixture.artifact.result)
                .expect("canonical result bytes");
        assert_eq!(
            decode_canonical_sccp_groth16_bn254_proof_result_v1(&result_bytes),
            Some(fixture.artifact.result.clone())
        );
        let artifact_bytes =
            encode_canonical_sccp_groth16_bn254_proof_artifact_v1(&fixture.artifact)
                .expect("canonical artifact bytes");
        assert_eq!(
            decode_canonical_sccp_groth16_bn254_proof_artifact_v1(&artifact_bytes),
            Some(fixture.artifact.clone())
        );
        let artifact_json = norito::json::to_json(&fixture.artifact).expect("artifact JSON");
        assert_eq!(
            decode_canonical_sccp_groth16_bn254_proof_artifact_json_v1(&artifact_json),
            Some(fixture.artifact.clone())
        );
        let call = verify_sccp_destination_proof_v1(
            &fixture.bridge_proof,
            &fixture.bundle,
            &fixture.route,
            &trusted_finality(&fixture.bundle),
        )
        .expect("verified destination call");
        assert_eq!(call.route_revision, 1);
        assert_eq!(
            call.route_configuration_hash,
            fixture
                .route
                .route_configuration_hash()
                .expect("route config")
        );
        assert_eq!(
            call.destination_binding_hash,
            fixture.request.destination_binding_hash
        );
        assert_eq!(&call.calldata[..4], &SCCP_FINALIZE_FROM_TAIRA_SELECTOR_V1);
        let proof_offset = usize::try_from(abi_read_u32_word(&call.calldata[4..36]).unwrap())
            .expect("proof offset");
        let payload_offset = usize::try_from(abi_read_u32_word(&call.calldata[260..292]).unwrap())
            .expect("payload offset");
        assert_eq!(proof_offset, 9 * 32);
        assert!(payload_offset > proof_offset);
        assert_eq!(
            &call.calldata[4 + proof_offset + 32..4 + proof_offset + 32 + call.proof_bytes.len()],
            call.proof_bytes.as_slice()
        );
        assert_eq!(
            decode_canonical_sccp_payload_bytes(&call.canonical_payload_bytes),
            Some(call.bundle.payload.clone())
        );
    }
    #[test]
    fn artifact_admission_decodes_bundle_once_across_pairing_and_binding() {
        let fixture = fixture();
        let decode_calls = Cell::new(0usize);
        assert!(
            sccp_groth16_bn254_proof_artifact_is_self_canonical_with_decoder_v1(
                &fixture.artifact,
                |bundle_bytes| {
                    decode_calls.set(decode_calls.get() + 1);
                    decode_canonical_taira_sccp_message_bundle_summary(bundle_bytes)
                },
            )
        );
        assert_eq!(decode_calls.get(), 1);
    }
    #[test]
    fn parsed_destination_proof_decodes_once_then_fully_verifies() {
        let fixture = fixture();
        reset_sccp_destination_proof_work_counters_v1();
        let parsed = parse_sccp_destination_proof_v1(&fixture.bridge_proof)
            .expect("canonical destination artifact parses");
        assert_eq!(
            sccp_destination_proof_work_counters_v1(),
            SccpDestinationProofWorkCountersV1 {
                artifact_framing_decodes: 1,
                bundle_decodes: 1,
                groth16_pairings: 0,
                bls_verifications: 0,
                bls12381_point_decodes: 0,
            }
        );
        let verified = verify_parsed_sccp_destination_proof_v1(
            parsed,
            &fixture.route,
            &trusted_finality(&fixture.bundle),
        )
        .expect("parsed artifact binds to governed route");
        assert_eq!(verified.public_inputs, fixture.request.public_inputs);
        assert_eq!(
            verified.public_inputs.finality_height,
            fixture.request.public_inputs.finality_height
        );
        assert_eq!(
            sccp_destination_proof_work_counters_v1(),
            SccpDestinationProofWorkCountersV1 {
                artifact_framing_decodes: 1,
                bundle_decodes: 1,
                groth16_pairings: 1,
                bls_verifications: 1,
                bls12381_point_decodes: 0,
            },
            "route-bound verification must evaluate each proof exactly once"
        );
    }
    #[test]
    fn destination_call_requires_exact_caller_trusted_finality_before_crypto() {
        let fixture = fixture();
        let parsed = parse_sccp_destination_proof_v1(&fixture.bridge_proof)
            .expect("canonical destination artifact parses");
        let mut untrusted_substitute = trusted_finality(&fixture.bundle);
        untrusted_substitute.finality_artifact.height += 1;
        reset_sccp_destination_proof_work_counters_v1();
        assert!(
            verify_parsed_sccp_destination_proof_v1(parsed, &fixture.route, &untrusted_substitute,)
                .is_none(),
            "proof-embedded self-consistency must not substitute for caller authority"
        );
        assert_eq!(
            sccp_destination_proof_work_counters_v1(),
            SccpDestinationProofWorkCountersV1::default(),
            "trusted-finality mismatch must fail before pairing or BLS work"
        );
    }
    #[test]
    fn hostile_governed_binding_fails_before_pairing_or_bls() {
        let fixture = fixture();
        let parsed = parse_sccp_destination_proof_v1(&fixture.bridge_proof)
            .expect("canonical destination artifact parses");
        let hostile_route = governed_route(
            SccpNetworkV1::BscMainnet,
            fixture.route.revision,
            SccpRouteActivationV1::Staged,
        );
        reset_sccp_destination_proof_work_counters_v1();
        assert!(
            verify_parsed_sccp_destination_proof_v1(
                parsed,
                &hostile_route,
                &trusted_finality(&fixture.bundle),
            )
            .is_none(),
            "cross-profile governed route substitution must fail closed"
        );
        assert_eq!(
            sccp_destination_proof_work_counters_v1(),
            SccpDestinationProofWorkCountersV1::default(),
            "state-binding rejection must precede every expensive proof operation"
        );
    }
    #[test]
    fn malformed_destination_framing_and_bundle_fail_before_crypto() {
        let fixture = fixture();
        let mut trailing_artifact = fixture.bridge_proof.clone();
        trailing_artifact.encoded_artifact.push(0);
        reset_sccp_destination_proof_work_counters_v1();
        assert!(parse_sccp_destination_proof_v1(&trailing_artifact).is_none());
        assert_eq!(
            sccp_destination_proof_work_counters_v1(),
            SccpDestinationProofWorkCountersV1 {
                artifact_framing_decodes: 1,
                bundle_decodes: 0,
                groth16_pairings: 0,
                bls_verifications: 0,
                bls12381_point_decodes: 0,
            }
        );
        let mut malformed_bundle_artifact = fixture.artifact.clone();
        malformed_bundle_artifact.request.bundle_bytes.push(0);
        let malformed_bundle = BridgeSccpDestinationProofV1 {
            backend: fixture.bridge_proof.backend,
            route_configuration_hash: fixture.bridge_proof.route_configuration_hash,
            encoded_artifact: to_bytes(&malformed_bundle_artifact)
                .expect("adversarial artifact has canonical outer Norito framing"),
        };
        reset_sccp_destination_proof_work_counters_v1();
        assert!(parse_sccp_destination_proof_v1(&malformed_bundle).is_none());
        assert_eq!(
            sccp_destination_proof_work_counters_v1(),
            SccpDestinationProofWorkCountersV1 {
                artifact_framing_decodes: 1,
                bundle_decodes: 1,
                groth16_pairings: 0,
                bls_verifications: 0,
                bls12381_point_decodes: 0,
            }
        );
    }
    #[test]
    fn validated_request_context_reuses_precomputed_public_signals() {
        let fixture = fixture();
        let decode_calls = Cell::new(0usize);
        let validated = validate_sccp_groth16_bn254_proof_request_with_decoder_v1(
            &fixture.request,
            fixture.request.backend,
            |bundle_bytes| {
                decode_calls.set(decode_calls.get() + 1);
                decode_canonical_taira_sccp_message_bundle_summary(bundle_bytes)
            },
        )
        .expect("canonical request context");
        assert_eq!(decode_calls.get(), 1);
        assert!(
            verify_sccp_groth16_bn254_proof_against_validated_request_v1(
                &validated,
                &fixture.artifact.result.proof_bytes,
            )
        );
        let rebound = bind_sccp_groth16_bn254_proof_result_v1(
            &fixture.artifact.result.proof_bytes,
            &validated,
        )
        .expect("pairing-verified artifact");
        assert_eq!(rebound, fixture.artifact);
        assert_eq!(decode_calls.get(), 1);
    }
    #[test]
    fn request_statement_hash_is_separated_from_every_governed_hash_role() {
        let base = &fixture().request;
        let governed_roles = [
            ("destination binding", base.destination_binding_hash),
            ("route configuration", base.route_configuration_hash),
            ("verifier key", base.verifier_key_hash),
            ("semantic proof profile", base.semantic_proof_profile_hash),
            ("SORA finality anchor", base.sora_finality_anchor_hash),
        ];
        for (role, role_hash) in governed_roles {
            assert_ne!(base.statement_hash, role_hash, "fixture aliases {role}");
            assert!(
                !sccp_groth16_bn254_request_hash_roles_are_distinct_v1(
                    role_hash,
                    base.destination_binding_hash,
                    base.route_configuration_hash,
                    base.verifier_key_hash,
                    base.semantic_proof_profile_hash,
                    base.sora_finality_anchor_hash,
                ),
                "statement alias with {role} was not detected"
            );
            let mut candidate = base.clone();
            candidate.statement_hash = role_hash;
            assert_request_rejected(&candidate);
        }
    }
    #[test]
    fn proof_request_decoders_reject_cross_policy_semantic_anchor_role_aliases() {
        let fixture = fixture();
        let base = &fixture.request;
        let canonical_payload_bytes = canonical_sccp_payload_bytes(&fixture.bundle.payload)
            .expect("canonical fixture payload");
        assert!(sccp_groth16_bn254_proof_request_header_is_canonical_v1(
            base,
            base.backend,
        ));
        let anchor_roles = [
            ("chain id", base.sora_finality_anchor.chain_id_hash),
            (
                "checkpoint block",
                base.sora_finality_anchor.checkpoint_block_hash,
            ),
            (
                "checkpoint context",
                base.sora_finality_anchor.checkpoint_context_id,
            ),
            (
                "finality artifact",
                base.sora_finality_anchor.checkpoint_finality_artifact_hash,
            ),
        ];
        let semantic_roles = ["circuit commitment", "witness generator commitment"];
        for (semantic_role_index, semantic_role) in semantic_roles.into_iter().enumerate() {
            for (anchor_role, anchor_hash) in anchor_roles {
                let mut candidate = base.clone();
                let SccpSemanticProofProfileV1::SoraTairaFinalityInclusionGroth16Bn254(
                    ref mut circuit,
                ) = candidate.semantic_proof_profile
                else {
                    unreachable!("BN254 fixture must retain its BN254 semantic profile")
                };
                match semantic_role_index {
                    0 => circuit.circuit_commitment = anchor_hash,
                    1 => circuit.witness_generator_commitment = anchor_hash,
                    _ => unreachable!("closed semantic role matrix"),
                }
                assert_cross_policy_alias_request_rejected(
                    candidate,
                    &canonical_payload_bytes,
                    semantic_role,
                    anchor_role,
                );
            }
        }
        let semantic_commitments = base.semantic_proof_profile.commitments();
        let semantic_roles = [
            ("circuit commitment", semantic_commitments[0]),
            ("witness generator commitment", semantic_commitments[1]),
            ("public signal schema", semantic_commitments[2]),
        ];
        let anchor_roles = [
            "checkpoint block",
            "checkpoint context",
            "finality artifact",
        ];
        for (anchor_role_index, anchor_role) in anchor_roles.into_iter().enumerate() {
            for (semantic_role, semantic_hash) in semantic_roles {
                let mut candidate = base.clone();
                match anchor_role_index {
                    0 => candidate.sora_finality_anchor.checkpoint_block_hash = semantic_hash,
                    1 => candidate.sora_finality_anchor.checkpoint_context_id = semantic_hash,
                    2 => {
                        candidate
                            .sora_finality_anchor
                            .checkpoint_finality_artifact_hash = semantic_hash;
                    }
                    _ => unreachable!("closed anchor role matrix"),
                }
                assert_cross_policy_alias_request_rejected(
                    candidate,
                    &canonical_payload_bytes,
                    semantic_role,
                    anchor_role,
                );
            }
        }
    }
    #[test]
    fn direct_verifier_rejects_every_hash_role_alias_with_a_matching_proof() {
        let request = &fixture().request;
        let role_names = [
            "statement",
            "destination binding",
            "route configuration",
            "verifier key",
            "semantic proof profile",
            "SORA finality anchor",
        ];
        let base_roles = [
            request.statement_hash,
            request.destination_binding_hash,
            request.route_configuration_hash,
            request.verifier_key_hash,
            request.semantic_proof_profile_hash,
            request.sora_finality_anchor_hash,
        ];
        assert!(hash_roles_are_distinct(base_roles));
        let proof_bytes = valid_proof(request);
        let mut zero_semantic_hash = request.clone();
        zero_semantic_hash.semantic_proof_profile_hash = [0; 32];
        assert!(
            !verify_sccp_groth16_bn254_proof_v1(&zero_semantic_hash, &proof_bytes,),
            "accepted a zero semantic-proof-profile hash"
        );
        for first in 0..base_roles.len() {
            for second in first + 1..base_roles.len() {
                let mut roles = base_roles;
                if second == 3 {
                    // Keep the governed verifier-key hash exact and alias the other role to it.
                    roles[first] = roles[second];
                } else {
                    roles[second] = roles[first];
                }
                assert!(!hash_roles_are_distinct(roles));
                let proof_bytes =
                    valid_proof_for_hash_roles(request, roles[0], roles[1], roles[2], roles[5]);
                let proof = decode_sccp_evm_groth16_bn254_proof_bytes(&proof_bytes)
                    .expect("generated proof is canonical");
                let signals = sccp_groth16_bn254_public_signal_words(
                    &request.public_inputs,
                    request.source_network.domain_id(),
                    roles[0],
                    roles[1],
                    roles[2],
                    roles[5],
                );
                assert!(
                    verify_sccp_groth16_bn254_pairing_equation_v1(
                        &proof,
                        &signals,
                        &request.verifying_key,
                    ),
                    "test proof does not match {} == {}",
                    role_names[first],
                    role_names[second]
                );
                assert!(
                    {
                        let mut aliased = request.clone();
                        aliased.statement_hash = roles[0];
                        aliased.destination_binding_hash = roles[1];
                        aliased.route_configuration_hash = roles[2];
                        aliased.verifier_key_hash = roles[3];
                        aliased.semantic_proof_profile_hash = roles[4];
                        aliased.sora_finality_anchor_hash = roles[5];
                        !verify_sccp_groth16_bn254_proof_v1(&aliased, &proof_bytes)
                    },
                    "accepted {} == {}",
                    role_names[first],
                    role_names[second]
                );
            }
        }
    }
    #[test]
    fn ten_signal_json_and_norito_verifying_keys_are_rejected() {
        #[derive(norito::derive::NoritoSerialize)]
        struct TenSignalIcV1 {
            constant: SccpBn254G1PointV1,
            signal_0: SccpBn254G1PointV1,
            signal_1: SccpBn254G1PointV1,
            signal_2: SccpBn254G1PointV1,
            signal_3: SccpBn254G1PointV1,
            signal_4: SccpBn254G1PointV1,
            signal_5: SccpBn254G1PointV1,
            signal_6: SccpBn254G1PointV1,
            signal_7: SccpBn254G1PointV1,
            signal_8: SccpBn254G1PointV1,
            signal_9: SccpBn254G1PointV1,
        }
        #[derive(norito::derive::NoritoSerialize)]
        struct TenSignalVerifyingKeyV1 {
            version: u8,
            alpha1: SccpBn254G1PointV1,
            beta2: SccpBn254G2PointV1,
            gamma2: SccpBn254G2PointV1,
            delta2: SccpBn254G2PointV1,
            ic: TenSignalIcV1,
        }
        let request = &fixture().request;
        let mut request_json =
            norito::json::to_value(request).expect("serialize canonical proof request");
        let ic = request_json
            .get_mut("verifying_key")
            .and_then(norito::json::Value::as_object_mut)
            .and_then(|key| key.get_mut("ic"))
            .and_then(norito::json::Value::as_object_mut)
            .expect("proof request contains a typed IC object");
        assert!(ic.remove("signal_10").is_some());
        let ten_signal_json =
            norito::json::to_json(&request_json).expect("serialize ten-signal request JSON");
        assert!(
            norito::json::from_json::<SccpGroth16Bn254ProofRequestV1>(&ten_signal_json).is_err()
        );
        assert!(
            decode_canonical_sccp_groth16_bn254_proof_request_json_v1(&ten_signal_json).is_none()
        );
        let key = verifying_key();
        let old_key = TenSignalVerifyingKeyV1 {
            version: key.version,
            alpha1: key.alpha1,
            beta2: key.beta2,
            gamma2: key.gamma2,
            delta2: key.delta2,
            ic: TenSignalIcV1 {
                constant: key.ic.constant,
                signal_0: key.ic.signal_0,
                signal_1: key.ic.signal_1,
                signal_2: key.ic.signal_2,
                signal_3: key.ic.signal_3,
                signal_4: key.ic.signal_4,
                signal_5: key.ic.signal_5,
                signal_6: key.ic.signal_6,
                signal_7: key.ic.signal_7,
                signal_8: key.ic.signal_8,
                signal_9: key.ic.signal_9,
            },
        };
        let old_key_bytes = to_bytes(&old_key).expect("encode old ten-signal Norito key");
        assert!(
            norito::decode_from_bytes::<SccpGroth16Bn254VerifyingKeyV1>(&old_key_bytes).is_err()
        );
        assert_eq!(
            canonical_sccp_groth16_bn254_verifying_key_bytes_v1(&key)
                .expect("canonical eleven-signal key")
                .len(),
            38 * 32
        );
    }
    #[test]
    fn solidity_key_route_and_eleventh_signal_vectors_match() {
        let key = verifying_key();
        assert!(sccp_groth16_bn254_verifying_key_is_well_formed_v1(&key));
        assert_eq!(key.ic.points().len(), 12);
        assert_eq!(
            canonical_sccp_groth16_bn254_verifying_key_bytes_v1(&key)
                .expect("canonical key")
                .len(),
            38 * 32
        );
        assert_eq!(
            sccp_groth16_bn254_verifying_key_hash_v1(&key),
            Some(hex32(
                "6923e63427820ab42cc16c3c2bc0eb4097577919bb3911ea50cbb4f20cebfddb"
            ))
        );
        let tron_deployment = SccpTronDestinationDeploymentV1 {
            token_address: [0x11; 20],
            token_code_hash: [0x21; 32],
            verifier_address: [0x31; 20],
            verifier_code_hash: [0x41; 32],
            verifying_key: key,
            verifier_key_hash: sccp_groth16_bn254_verifying_key_hash_v1(&key).unwrap(),
            outbound_proof_policy: outbound_proof_policy(),
            route_address: [0x51; 20],
            route_code_hash: [0x61; 32],
            replay_verifier_address: [0x71; 20],
            replay_verifier_code_hash: [0x72; 32],
            mint_breaker_address: [0x81; 20],
            mint_breaker_code_hash: [0x82; 32],
            taira_to_token_multiplier: SCCP_V1_TAIRA_TO_TOKEN_MULTIPLIER,
            max_wrapped_supply: TEST_MAX_OUTSTANDING_LIABILITY
                * SCCP_V1_TAIRA_TO_TOKEN_MULTIPLIER as u128,
        };
        let inbound = SccpLaneIdV1 {
            source: SccpNetworkV1::TronMainnet,
            target: SccpNetworkV1::SoraTaira,
        };
        let outbound = SccpLaneIdV1 {
            source: inbound.target,
            target: inbound.source,
        };
        let route_config = sccp_exact_tron_xor_route_config_hash_v1(
            SccpNetworkV1::TronMainnet,
            sccp_lane_id_hash_v1(inbound).unwrap(),
            sccp_lane_id_hash_v1(outbound).unwrap(),
            &tron_deployment,
            7,
        )
        .expect("TRON contract route config");
        assert_eq!(
            route_config,
            hex32("27da5c364f20fdee0bdd8cd84bb01908d88f40d9eb5171f9dec8b330f604258d")
        );
        let request = &fixture().request;
        let signals = sccp_groth16_bn254_public_signal_words(
            &request.public_inputs,
            request.source_network.domain_id(),
            request.statement_hash,
            request.destination_binding_hash,
            request.route_configuration_hash,
            request.sora_finality_anchor_hash,
        );
        assert_eq!(signals.len(), 11);
        assert_eq!(
            signals[9],
            sccp_groth16_bn254_signal_word(
                SCCP_GROTH16_BN254_SIGNAL_ROUTE_CONFIGURATION_HASH_V1,
                request.route_configuration_hash,
            )
        );
        let mut changed = request.route_configuration_hash;
        changed[0] ^= 1;
        assert_ne!(
            signals[9],
            sccp_groth16_bn254_signal_word(
                SCCP_GROTH16_BN254_SIGNAL_ROUTE_CONFIGURATION_HASH_V1,
                changed,
            )
        );
        assert_eq!(
            signals[10],
            sccp_groth16_bn254_signal_word(
                SCCP_GROTH16_BN254_SIGNAL_SORA_FINALITY_ANCHOR_HASH_V1,
                request.sora_finality_anchor_hash,
            )
        );
        let mut changed_anchor = request.sora_finality_anchor_hash;
        changed_anchor[0] ^= 1;
        assert_ne!(
            signals[10],
            sccp_groth16_bn254_signal_word(
                SCCP_GROTH16_BN254_SIGNAL_SORA_FINALITY_ANCHOR_HASH_V1,
                changed_anchor,
            )
        );
    }
    fn assert_request_mutation_rejected(mutate: impl FnOnce(&mut SccpGroth16Bn254ProofRequestV1)) {
        let base = &fixture().request;
        let mut candidate = base.clone();
        mutate(&mut candidate);
        assert_ne!(
            &candidate, base,
            "negative mutation must change the fixture"
        );
        assert_request_rejected(&candidate);
    }
    #[test]
    fn request_network_and_public_input_roles_are_fail_closed() {
        assert_request_mutation_rejected(|candidate| candidate.version = 2);
        assert_request_mutation_rejected(|candidate| {
            candidate.backend = BridgeSccpDestinationProofBackendV1::TronGroth16Bn254;
        });
        assert_request_mutation_rejected(|candidate| {
            candidate.source_network = SccpNetworkV1::BscMainnet;
        });
        assert_request_mutation_rejected(|candidate| {
            candidate.target_network = SccpNetworkV1::BscMainnet;
        });
        assert_request_mutation_rejected(|candidate| candidate.public_inputs.message_id[0] ^= 1);
        assert_request_mutation_rejected(|candidate| candidate.public_inputs.payload_hash[0] ^= 1);
        assert_request_mutation_rejected(|candidate| {
            candidate.public_inputs.target_domain = SCCP_DOMAIN_BSC;
        });
        assert_request_mutation_rejected(|candidate| {
            candidate.public_inputs.commitment_root[0] ^= 1;
        });
        assert_request_mutation_rejected(|candidate| candidate.public_inputs.finality_height += 1);
        assert_request_mutation_rejected(|candidate| {
            candidate.public_inputs.finality_block_hash[0] ^= 1;
        });
    }
    #[test]
    fn request_verifier_and_finality_policy_roles_are_fail_closed() {
        assert_request_mutation_rejected(|candidate| {
            candidate.verifying_key.alpha1.y = word_u64(3)
        });
        assert_request_mutation_rejected(|candidate| candidate.verifier_key_hash[0] ^= 1);
        assert_request_mutation_rejected(|candidate| {
            let SccpSemanticProofProfileV1::SoraTairaFinalityInclusionGroth16Bn254(ref mut circuit) =
                candidate.semantic_proof_profile
            else {
                unreachable!("BN254 fixture must retain its BN254 semantic profile")
            };
            circuit.circuit_commitment[0] ^= 1;
        });
        assert_request_mutation_rejected(|candidate| {
            candidate.semantic_proof_profile_hash[0] ^= 1;
        });
        assert_request_mutation_rejected(|candidate| candidate.sora_finality_anchor.version = 2);
        assert_request_mutation_rejected(|candidate| {
            candidate.sora_finality_anchor.source_network = SccpNetworkV1::EthereumMainnet;
        });
        assert_request_mutation_rejected(|candidate| {
            candidate.sora_finality_anchor.protocol_version = candidate
                .sora_finality_anchor
                .protocol_version
                .saturating_add(1);
        });
        assert_request_mutation_rejected(|candidate| {
            candidate.sora_finality_anchor.chain_id_hash[0] ^= 1;
        });
        assert_request_mutation_rejected(|candidate| {
            candidate.sora_finality_anchor.checkpoint_height += 1;
        });
        assert_request_mutation_rejected(|candidate| {
            candidate.sora_finality_anchor.checkpoint_block_hash[0] ^= 1;
        });
        assert_request_mutation_rejected(|candidate| {
            candidate.sora_finality_anchor.checkpoint_context_id[0] ^= 1;
        });
        assert_request_mutation_rejected(|candidate| {
            candidate
                .sora_finality_anchor
                .checkpoint_finality_artifact_hash[0] ^= 1;
        });
        assert_request_mutation_rejected(|candidate| candidate.sora_finality_anchor_hash[0] ^= 1);
    }
    fn assert_artifact_mutation_rejected(
        mutate: impl FnOnce(&mut SccpGroth16Bn254ProofArtifactV1),
    ) {
        let mut artifact = fixture().artifact.clone();
        mutate(&mut artifact);
        assert!(
            decode_canonical_sccp_groth16_bn254_proof_artifact_v1(&to_bytes(&artifact).unwrap())
                .is_none()
        );
    }
    #[test]
    fn request_nested_and_artifact_commitments_are_fail_closed() {
        assert_request_mutation_rejected(|candidate| candidate.bundle_bytes.push(0));
        assert_request_mutation_rejected(|candidate| candidate.statement_hash[0] ^= 1);
        assert_request_mutation_rejected(|candidate| candidate.destination_binding_hash[0] ^= 1);
        assert_request_mutation_rejected(|candidate| candidate.route_configuration_hash[0] ^= 1);
        assert_request_mutation_rejected(|candidate| candidate.request_hash[0] ^= 1);
        assert_artifact_mutation_rejected(|artifact| artifact.result.version = 2);
        assert_artifact_mutation_rejected(|artifact| artifact.result.request_hash[0] ^= 1);
        assert_artifact_mutation_rejected(|artifact| artifact.result.proof_bytes[0] ^= 1);
        assert_artifact_mutation_rejected(|artifact| artifact.result.result_hash[0] ^= 1);
    }
    #[test]
    fn canonical_decoders_reject_framing_json_and_size_attacks() {
        let artifact = &fixture().artifact;
        let bytes = encode_canonical_sccp_groth16_bn254_proof_artifact_v1(artifact).unwrap();
        for length in [0, 1, norito::core::Header::SIZE - 1, bytes.len() - 1] {
            assert!(
                decode_canonical_sccp_groth16_bn254_proof_artifact_v1(&bytes[..length]).is_none()
            );
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(decode_canonical_sccp_groth16_bn254_proof_artifact_v1(&trailing).is_none());
        for offset in [4, 5] {
            let mut wrong_header = bytes.clone();
            wrong_header[offset] ^= 1;
            assert!(decode_canonical_sccp_groth16_bn254_proof_artifact_v1(&wrong_header).is_none());
        }
        let mut compressed = bytes.clone();
        compressed[SCCP_NORITO_COMPRESSION_OFFSET] = 1;
        assert!(decode_canonical_sccp_groth16_bn254_proof_artifact_v1(&compressed).is_none());
        let mut declared_bomb = bytes.clone();
        declared_bomb[SCCP_NORITO_LENGTH_OFFSET..SCCP_NORITO_LENGTH_OFFSET + 8].copy_from_slice(
            &u64::try_from(SCCP_GROTH16_BN254_MAX_ENCODED_ARTIFACT_BYTES_V1 + 1)
                .unwrap()
                .to_le_bytes(),
        );
        assert!(decode_canonical_sccp_groth16_bn254_proof_artifact_v1(&declared_bomb).is_none());
        assert!(
            decode_canonical_sccp_groth16_bn254_proof_artifact_v1(&vec![
                0;
                SCCP_GROTH16_BN254_MAX_ENCODED_ARTIFACT_BYTES_V1
                    + 1
            ])
            .is_none()
        );
        let json = norito::json::to_json(artifact).unwrap();
        assert!(
            decode_canonical_sccp_groth16_bn254_proof_artifact_json_v1(&format!(" {json}"))
                .is_none()
        );
        let unknown = format!("{},\"unknown\":0}}", &json[..json.len() - 1]);
        assert!(decode_canonical_sccp_groth16_bn254_proof_artifact_json_v1(&unknown).is_none());
        let duplicate = format!("{},\"version\":1}}", &json[..json.len() - 1]);
        assert!(decode_canonical_sccp_groth16_bn254_proof_artifact_json_v1(&duplicate).is_none());
        let json_bomb = " ".repeat(SCCP_GROTH16_BN254_MAX_JSON_ARTIFACT_BYTES_V1 + 1);
        assert!(decode_canonical_sccp_groth16_bn254_proof_artifact_json_v1(&json_bomb).is_none());
        assert_eq!(
            SCCP_GROTH16_BN254_MAX_BASE64_ARTIFACT_BYTES_V1,
            4 * SCCP_GROTH16_BN254_MAX_ENCODED_ARTIFACT_BYTES_V1.div_ceil(3)
        );
    }
    fn non_subgroup_g2() -> SccpBn254G2PointV1 {
        for value in 1..10_000_u64 {
            let x = Fq2::new(Fq::from(value), Fq::from(value + 1));
            let rhs = x.square() * x + G2Affine::b();
            let Some(y) = Option::<Fq2>::from(rhs.sqrt()) else {
                continue;
            };
            let Some(point) = Option::<G2Affine>::from(G2Affine::from_xy(x, y)) else {
                continue;
            };
            if !bool::from(point.to_curve().is_torsion_free()) {
                return g2_model(point);
            }
        }
        panic!("failed to find deterministic non-subgroup G2 point");
    }
    #[test]
    fn curve_and_abi_adversaries_fail_closed() {
        let mut key = verifying_key();
        key.alpha1 = SccpBn254G1PointV1 {
            x: [0; 32],
            y: [0; 32],
        };
        assert!(!sccp_groth16_bn254_verifying_key_is_well_formed_v1(&key));
        let mut key = verifying_key();
        key.alpha1.y = word_u64(3);
        assert!(!sccp_groth16_bn254_verifying_key_is_well_formed_v1(&key));
        let mut key = verifying_key();
        key.alpha1.x = BN254_BASE_FIELD_MODULUS_BE;
        assert!(!sccp_groth16_bn254_verifying_key_is_well_formed_v1(&key));
        let mut key = verifying_key();
        key.beta2 = non_subgroup_g2();
        assert!(!sccp_groth16_bn254_verifying_key_is_well_formed_v1(&key));
        let proof = &fixture().artifact.result.proof_bytes;
        let mut infinity = proof.clone();
        infinity[4 * 32..6 * 32].fill(0);
        assert!(decode_sccp_evm_groth16_bn254_proof_bytes(&infinity).is_none());
        let mut noncanonical = proof.clone();
        noncanonical[4 * 32..5 * 32].copy_from_slice(&BN254_BASE_FIELD_MODULUS_BE);
        assert!(decode_sccp_evm_groth16_bn254_proof_bytes(&noncanonical).is_none());
        let mut swapped_g2 = proof.clone();
        let first = swapped_g2[6 * 32..7 * 32].to_vec();
        let second = swapped_g2[7 * 32..8 * 32].to_vec();
        swapped_g2[6 * 32..7 * 32].copy_from_slice(&second);
        swapped_g2[7 * 32..8 * 32].copy_from_slice(&first);
        assert!(decode_sccp_evm_groth16_bn254_proof_bytes(&swapped_g2).is_none());
        let mut subgroup = proof.clone();
        let point = non_subgroup_g2();
        for (index, word) in [point.x_c0, point.x_c1, point.y_c0, point.y_c1]
            .into_iter()
            .enumerate()
        {
            subgroup[(6 + index) * 32..(7 + index) * 32].copy_from_slice(&word);
        }
        assert!(decode_sccp_evm_groth16_bn254_proof_bytes(&subgroup).is_none());
    }
    #[test]
    fn historical_lifecycle_survives_but_cross_route_and_outer_substitution_fail() {
        let fixture = fixture();
        for activation in [
            SccpRouteActivationV1::InboundOnly,
            SccpRouteActivationV1::Paused,
            SccpRouteActivationV1::Retired,
        ] {
            let mut historical = fixture.route.clone();
            historical.activation = activation;
            historical.inbound_finality_cutoff =
                activation
                    .is_terminal()
                    .then_some(SccpInboundFinalityCutoffV1 {
                        trust_anchor_hash: [0x91; 32],
                        max_anchor_interval_height: 100,
                    });
            assert_eq!(
                build_sccp_groth16_bn254_proof_request_from_governed_route_v1(
                    &fixture.bundle,
                    &historical,
                ),
                Some(fixture.request.clone())
            );
            assert!(
                verify_sccp_destination_proof_v1(
                    &fixture.bridge_proof,
                    &fixture.bundle,
                    &historical,
                    &trusted_finality(&fixture.bundle),
                )
                .is_some()
            );
        }
        let successor = governed_route(
            SccpNetworkV1::EthereumMainnet,
            2,
            SccpRouteActivationV1::Bidirectional,
        );
        assert!(
            build_sccp_groth16_bn254_proof_request_from_governed_route_v1(
                &fixture.bundle,
                &successor,
            )
            .is_none()
        );
        let other_network = governed_route(
            SccpNetworkV1::BscMainnet,
            1,
            SccpRouteActivationV1::Bidirectional,
        );
        assert!(
            verify_sccp_destination_proof_v1(
                &fixture.bridge_proof,
                &fixture.bundle,
                &other_network,
                &trusted_finality(&fixture.bundle),
            )
            .is_none()
        );
        let mut outer = fixture.bridge_proof.clone();
        outer.backend = BridgeSccpDestinationProofBackendV1::TronGroth16Bn254;
        assert!(decode_bridge_sccp_destination_proof_v1(&outer).is_none());
        let mut outer = fixture.bridge_proof.clone();
        outer.route_configuration_hash[0] ^= 1;
        assert!(decode_bridge_sccp_destination_proof_v1(&outer).is_none());
        let mut outer = fixture.bridge_proof.clone();
        outer.encoded_artifact.push(0);
        assert!(decode_bridge_sccp_destination_proof_v1(&outer).is_none());
        let generic = BridgeProofPayload::TransparentZk(BridgeTransparentProof {
            verifier_manifest_hash: [0x91; 32],
            proof: ProofBox::new("generic-transparent".to_owned(), vec![1, 2, 3]),
            recursion_depth: None,
        });
        assert!(!matches!(generic, BridgeProofPayload::SccpDestination(_)));
    }
    #[test]
    fn request_builder_reuses_bound_finality_without_repeating_bls() {
        let fixture = fixture();
        let finality = decode_taira_bridge_finality_proof(&fixture.bundle.finality_proof)
            .expect("canonical fixture finality");
        reset_sccp_destination_proof_work_counters_v1();
        for _ in 0..4 {
            assert_eq!(
                build_sccp_groth16_bn254_proof_request_from_structurally_bound_finality_v1(
                    &fixture.bundle,
                    &fixture.route,
                    &finality,
                ),
                Some(fixture.request.clone())
            );
        }
        let mut substituted = finality.clone();
        substituted.finality_artifact.height =
            substituted.finality_artifact.height.saturating_add(1);
        assert!(
            build_sccp_groth16_bn254_proof_request_from_structurally_bound_finality_v1(
                &fixture.bundle,
                &fixture.route,
                &substituted,
            )
            .is_none()
        );
        assert_eq!(
            sccp_destination_proof_work_counters_v1().bls_verifications,
            0,
            "structural assembly must reuse the caller's verified finality marker"
        );
        assert_eq!(
            build_sccp_groth16_bn254_proof_request_from_governed_route_v1(
                &fixture.bundle,
                &fixture.route,
            ),
            Some(fixture.request.clone())
        );
        assert_eq!(
            sccp_destination_proof_work_counters_v1().bls_verifications,
            1,
            "the untrusted entry point must still perform one BLS verification"
        );
    }
    #[test]
    fn bundle_context_payload_finality_and_revision_mutations_fail() {
        let fixture = fixture();
        let mut candidates = Vec::new();
        let mut candidate = fixture.bundle.clone();
        candidate.version = 2;
        candidates.push(candidate);
        let mut candidate = fixture.bundle.clone();
        candidate.commitment.context.destination_binding_hash[0] ^= 1;
        candidates.push(candidate);
        let mut candidate = fixture.bundle.clone();
        candidate.commitment.context.route_configuration_hash[0] ^= 1;
        candidates.push(candidate);
        let mut candidate = fixture.bundle.clone();
        candidate.commitment.context.lane.target = SccpNetworkV1::BscMainnet;
        candidates.push(candidate);
        let mut candidate = fixture.bundle.clone();
        candidate.commitment.message_id[0] ^= 1;
        candidates.push(candidate);
        let mut candidate = fixture.bundle.clone();
        candidate.commitment.payload_hash[0] ^= 1;
        candidates.push(candidate);
        let mut candidate = fixture.bundle.clone();
        let SccpPayloadV1::Transfer(payload) = &mut candidate.payload;
        payload.route_revision = 2;
        candidates.push(candidate);
        let mut candidate = fixture.bundle.clone();
        let mut finality = decode_taira_bridge_finality_proof(&candidate.finality_proof).unwrap();
        finality.finality_artifact.height += 1;
        candidate.finality_proof = to_bytes(&finality).unwrap();
        candidates.push(candidate);
        let mut candidate = fixture.bundle.clone();
        let mut finality = decode_taira_bridge_finality_proof(&candidate.finality_proof).unwrap();
        finality.finality_artifact.height_context.network_id = NetworkId::from_genesis_hash(
            iroha_crypto::HashOf::<BlockHeader>::from_untyped_unchecked(iroha_crypto::Hash::new(
                b"substituted SCCP bundle network",
            )),
        );
        candidate.finality_proof = to_bytes(&finality).unwrap();
        candidates.push(candidate);
        let mut candidate = fixture.bundle.clone();
        let mut finality = decode_taira_bridge_finality_proof(&candidate.finality_proof).unwrap();
        finality
            .block_header
            .set_sccp_commitment_root(Some([0xa5; 32]));
        candidate.finality_proof = to_bytes(&finality).unwrap();
        candidates.push(candidate);
        for candidate in candidates {
            assert!(
                build_sccp_groth16_bn254_proof_request_from_governed_route_v1(
                    &candidate,
                    &fixture.route,
                )
                .is_none()
            );
        }
        let revision_two = transfer_payload(2);
        assert_ne!(
            sccp_message_id(
                fixture.bundle.commitment.context.lane,
                &fixture.bundle.payload
            ),
            sccp_message_id(fixture.bundle.commitment.context.lane, &revision_two)
        );
    }
    fn exact_v2_finality_fixture() -> TairaBridgeFinalityProofV1 {
        decode_taira_bridge_finality_proof(&fixture().bundle.finality_proof)
            .expect("canonical v2 finality fixture")
    }
    fn assert_finality_structure_rejected(
        proof: &TairaBridgeFinalityProofV1,
        mutate: impl FnOnce(&mut TairaBridgeFinalityProofV1),
    ) {
        let mut attack = proof.clone();
        mutate(&mut attack);
        assert!(!verify_taira_bridge_finality_proof_structure(&attack));
    }
    #[test]
    fn exact_v2_finality_accepts_fixture_and_rejects_context_attacks() {
        use iroha_data_model::block::consensus_v2::{GlobalPhase, PROTOCOL_VERSION};
        let proof = exact_v2_finality_fixture();
        assert!(verify_taira_bridge_finality_proof_structure(&proof));
        assert!(verify_taira_bridge_finality_proof_cryptographic(&proof));
        assert_finality_structure_rejected(&proof, |attack| {
            attack.version = BRIDGE_FINALITY_PROOF_VERSION_V2.saturating_add(1);
        });
        assert_finality_structure_rejected(&proof, |attack| {
            attack.finality_artifact.protocol_version = PROTOCOL_VERSION.saturating_add(1);
            attack.finality_artifact.height_context.protocol_version =
                PROTOCOL_VERSION.saturating_add(1);
            attack.finality_artifact.commit_qc.round.context_id =
                attack.finality_artifact.height_context.id();
            attack.finality_artifact.commit_qc.proposal_round.context_id =
                attack.finality_artifact.height_context.id();
        });
        assert_finality_structure_rejected(&proof, |attack| {
            attack.finality_artifact.height_context.network_id = NetworkId::from_genesis_hash(
                iroha_crypto::HashOf::<BlockHeader>::from_untyped_unchecked(
                    iroha_crypto::Hash::new(b"attacker SCCP finality network"),
                ),
            );
            attack.finality_artifact.commit_qc.round.context_id =
                attack.finality_artifact.height_context.id();
            attack.finality_artifact.commit_qc.proposal_round.context_id =
                attack.finality_artifact.height_context.id();
        });
        assert_finality_structure_rejected(&proof, |attack| {
            attack.finality_artifact.commit_qc.round.context_id.0 =
                iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::new(
                    b"attacker context",
                ));
        });
        assert_finality_structure_rejected(&proof, |attack| {
            attack.finality_artifact.commit_qc.phase = GlobalPhase::Prepare;
        });
        assert_finality_structure_rejected(&proof, |attack| {
            attack.finality_artifact.commit_qc.subject.payload_hash =
                iroha_crypto::Hash::new(b"attacker payload");
        });
    }
    #[test]
    fn exact_v2_finality_enforces_equal_vote_quorum_and_signer_integrity() {
        let proof = exact_v2_finality_fixture();
        let mut attack = proof.clone();
        attack.finality_artifact.commit_qc.signers = vec![1, 2, 3];
        assert!(
            verify_taira_bridge_finality_proof_structure(&attack),
            "any exact three-of-four validator set satisfies the revision-4 equal-vote quorum"
        );
        let mut attack = proof.clone();
        attack.finality_artifact.commit_qc.signers = vec![0, 1];
        assert!(
            !verify_taira_bridge_finality_proof_structure(&attack),
            "two of four validators must fail the revision-4 count quorum regardless of power"
        );
        let mut attack = proof.clone();
        attack.finality_artifact.height_context.quorum.min_signers = attack
            .finality_artifact
            .height_context
            .quorum
            .min_signers
            .saturating_sub(1);
        attack.finality_artifact.commit_qc.round.context_id =
            attack.finality_artifact.height_context.id();
        attack.finality_artifact.commit_qc.proposal_round.context_id =
            attack.finality_artifact.height_context.id();
        assert!(
            !verify_taira_bridge_finality_proof_structure(&attack),
            "a proof-controlled count threshold must not replace the canonical roster quorum"
        );
        let mut attack = proof.clone();
        attack.finality_artifact.height_context.quorum.total_power = attack
            .finality_artifact
            .height_context
            .quorum
            .total_power
            .saturating_add(1);
        attack.finality_artifact.commit_qc.round.context_id =
            attack.finality_artifact.height_context.id();
        attack.finality_artifact.commit_qc.proposal_round.context_id =
            attack.finality_artifact.height_context.id();
        assert!(
            !verify_taira_bridge_finality_proof_structure(&attack),
            "a proof-controlled total power must not replace the exact roster sum"
        );
        for signers in [vec![0, 0, 1], vec![1, 0, 2], vec![0, 1, 4]] {
            let mut attack = proof.clone();
            attack.finality_artifact.commit_qc.signers = signers;
            assert!(!verify_taira_bridge_finality_proof_structure(&attack));
        }
    }
    #[test]
    fn exact_v2_finality_rejects_proof_material_and_crypto_attacks() {
        let proof = exact_v2_finality_fixture();
        assert_finality_structure_rejected(&proof, |attack| {
            attack.finality_artifact.validator_set_pops.pop();
        });
        assert_finality_structure_rejected(&proof, |attack| {
            attack.finality_artifact.validator_set_pops[0].clear();
        });
        assert_finality_structure_rejected(&proof, |attack| {
            attack.finality_artifact.validator_set_pops[0] =
                vec![0x55; SCCP_TAIRA_MAX_BLS_PROOF_BYTES_V1 + 1];
        });
        assert_finality_structure_rejected(&proof, |attack| {
            attack.finality_artifact.commit_qc.aggregate_signature =
                vec![0x55; SCCP_TAIRA_MAX_BLS_PROOF_BYTES_V1 + 1];
        });
        let mut attack = proof.clone();
        attack.finality_artifact.commit_qc.aggregate_signature[0] ^= 1;
        assert!(verify_taira_bridge_finality_proof_structure(&attack));
        assert!(!verify_taira_bridge_finality_proof_cryptographic(&attack));
        let mut attack = proof.clone();
        attack.finality_artifact.validator_set_pops[0][0] ^= 1;
        assert!(verify_taira_bridge_finality_proof_structure(&attack));
        assert!(!verify_taira_bridge_finality_proof_cryptographic(&attack));
        let mut attack = proof.clone();
        assert!(!attack.finality_artifact.commit_qc.signers.contains(&3));
        attack.finality_artifact.validator_set_pops[3][0] ^= 1;
        assert!(verify_taira_bridge_finality_proof_structure(&attack));
        assert!(
            !verify_taira_bridge_finality_proof_cryptographic(&attack),
            "every frozen-roster PoP, including non-signers, must authenticate its validator key"
        );
    }
    #[test]
    fn exact_v2_finality_rejects_missing_or_invalid_header_roots() {
        let proof = exact_v2_finality_fixture();
        for (merkle_root, result_merkle_root, missing) in [
            (None, proof.block_header.result_merkle_root(), "entrypoint"),
            (proof.block_header.merkle_root(), None, "result"),
        ] {
            let mut attack = proof.clone();
            let mut incomplete_header = BlockHeader::new(
                proof.block_header.height(),
                proof.block_header.prev_block_hash(),
                merkle_root,
                result_merkle_root,
                u64::try_from(proof.block_header.creation_time().as_millis())
                    .expect("fixture creation time fits u64"),
                proof.block_header.view_change_index(),
            );
            incomplete_header.set_sccp_commitment_root(proof.block_header.sccp_commitment_root());
            let incomplete_hash = incomplete_header.hash();
            attack.block_header = incomplete_header;
            attack.finality_artifact.block_hash = incomplete_hash;
            attack.finality_artifact.subject.block_hash = incomplete_hash;
            attack.finality_artifact.commit_qc.subject.block_hash = incomplete_hash;
            attack
                .finality_artifact
                .validate_for_header(&incomplete_header)
                .expect("the hostile artifact is otherwise structurally header-consistent");
            assert!(
                !verify_taira_bridge_finality_proof_structure(&attack),
                "SCCP finality must reject a missing {missing} Merkle root"
            );
        }
        let mut attack = proof.clone();
        attack.block_header.set_sccp_commitment_root(None);
        assert!(!verify_taira_bridge_finality_proof_structure(&attack));
        let mut attack = proof;
        attack.block_header.set_sccp_commitment_root(Some([0; 32]));
        assert!(!verify_taira_bridge_finality_proof_structure(&attack));
    }
}
