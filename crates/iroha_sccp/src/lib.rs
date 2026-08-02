//! SCCP payload, proof, and counterparty submission helpers for Iroha bridge flows.
//!
//! SCCP V1 supports Ethereum, BSC, Solana testnet, and TRON as complete
//! bidirectional route families. Solana uses one exact testnet identity and a
//! governed recursive Agave proof; no domain-wide or mainnet alias is
//! decodable. SCCP will
//! not support Sub&#115;trate/Pol&#107;adot networks for now; treat that as launch
//! scope, not pending compatibility work.
//!
//! The crate targets the Rust standard library unconditionally.
//! BLS verification for Taira and BSC finality is also unconditional so Cargo
//! feature selection cannot change consensus admission results.

extern crate alloc;

mod source_identity;
pub use source_identity::*;
mod ethereum_native;
pub use ethereum_native::*;
mod ethereum_source;
pub use ethereum_source::*;
mod bsc_native;
pub use bsc_native::*;
mod solana_native;
pub use solana_native::*;
mod tron_native;
pub use tron_native::*;
mod native_admission;
pub use native_admission::*;
#[cfg(any(test, feature = "test-fixtures"))]
mod test_fixtures;
#[cfg(any(test, feature = "test-fixtures"))]
pub use test_fixtures::{
    SccpExactOutboundTestFixtureV1, SccpFinalizedBlockTestFixtureV1,
    sccp_exact_evm_governed_route_test_fixture_v1, sccp_exact_outbound_test_fixture_for_nonce_v1,
    sccp_exact_outbound_test_fixture_v1, sccp_finalize_taira_block_test_fixture_v1,
    sccp_sora_outbound_execution_policy_test_fixture_v1,
};

use alloc::{borrow::ToOwned, format, string::String, vec::Vec};

use blake2::{
    Blake2bVar,
    digest::{Update, VariableOutput},
};
#[cfg(test)]
use halo2curves::ff::{Field, PrimeField};
use halo2curves::{
    CurveAffine,
    bn256::{self, Fq, Fq2, Fr, G1Affine, G2Affine},
    group::{Curve, Group, cofactor::CofactorGroup, prime::PrimeCurveAffine},
    pairing::MillerLoopResult,
};
use iroha_crypto::Algorithm;
#[cfg(test)]
use iroha_crypto::KeyPair;
#[cfg(test)]
use iroha_data_model::bridge::{
    SccpGroth16Bn254SemanticCircuitV1, sccp_groth16_bn254_public_signal_schema_hash_v1,
    sccp_solana_native_verifier_config_hash_v1, sccp_sora_taira_chain_id_hash_v1,
};
use iroha_data_model::{
    account::{AccountController, AccountId},
    block::BlockHeader,
    bridge::{
        BRIDGE_FINALITY_PROOF_VERSION_V1, BridgeSccpDestinationProofBackendV1,
        BridgeSccpDestinationProofV1, SCCP_V1_TAIRA_TO_SOLANA_TOKEN_MULTIPLIER, SccpBn254G1PointV1,
        SccpBn254G2PointV1, SccpDestinationDeploymentV1, SccpGovernedRouteV1,
        SccpGroth16Bn254VerifyingKeyV1, SccpOutboundProofPolicyV1, SccpSemanticProofProfileV1,
        SccpSolanaDestinationDeploymentV1, SccpSoraFinalityAnchorV1,
        canonical_sccp_semantic_proof_profile_bytes_v1,
        canonical_sccp_sora_finality_anchor_bytes_v1, sccp_semantic_proof_profile_hash_v1,
        sccp_sora_finality_anchor_hash_v1,
    },
};
use norito::to_bytes;
use sha2::{Digest as _, Sha256};
use tiny_keccak::Hasher;

#[cfg(any(test, feature = "test-fixtures"))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
/// Per-thread work counters for the closed SCCP destination-proof path.
///
/// This instrumentation is compiled only for crate tests or with the existing
/// `test-fixtures` feature, so production verification does not pay for atomic
/// or thread-local accounting.
pub struct SccpDestinationProofWorkCountersV1 {
    /// Canonical outer destination artifacts decoded on this thread.
    pub artifact_framing_decodes: usize,
    /// Canonical embedded Taira message bundles decoded on this thread.
    pub bundle_decodes: usize,
    /// BN254 Groth16 pairing equations evaluated on this thread.
    pub groth16_pairings: usize,
    /// Taira commit-QC BLS aggregates evaluated on this thread.
    pub bls_verifications: usize,
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
/// SCCP protocol domain assigned to Solana networks.
pub const SCCP_DOMAIN_SOLANA: u32 = 3;
/// SCCP protocol domain assigned to TRON networks.
pub const SCCP_DOMAIN_TRON: u32 = 5;
/// Public TAIRA chain id bound into TAIRA-origin SCCP finality proofs.
pub const SCCP_TAIRA_FINALITY_CHAIN_ID_V1: &str = "fc56984b-2be7-431d-840e-21514d1883f0";
/// Canonical I105 chain discriminant required for every SCCP Taira account literal.
pub const SCCP_TAIRA_I105_DISCRIMINANT_V1: u16 = 369;
/// TAIRA SCCP route id used for the initial XOR bridge to TRON Nile.
pub const SCCP_TAIRA_TRON_XOR_ROUTE_ID_V1: &str = "taira_tron_xor";
/// TAIRA SCCP route id used for the exact XOR bridge to Ethereum.
pub const SCCP_TAIRA_ETH_XOR_ROUTE_ID_V1: &str = "taira_eth_xor";
/// TAIRA SCCP route id used for the XOR bridge to BSC.
pub const SCCP_TAIRA_BSC_XOR_ROUTE_ID_V1: &str = "taira_bsc_xor";
/// TAIRA SCCP route id used for the exact XOR bridge to Solana testnet.
pub const SCCP_TAIRA_SOL_XOR_ROUTE_ID_V1: &str = "taira_sol_xor";
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
/// Raw nonzero 32-byte Solana public key.
///
/// The wire form is binary. Base58 is a presentation encoding and is never
/// admitted as a second consensus representation of the same key.
pub const SCCP_CODEC_SOLANA_PUBKEY32: u8 = 6;
/// Maximum byte length of one canonical textual SCCP wire value.
pub const SCCP_MAX_CANONICAL_TEXT_BYTES_V1: usize = 256;
/// Seed prefix of the governed Solana verifier-material PDA.
pub const SCCP_SOLANA_MATERIAL_PDA_SEED_V1: &[u8] = b"sccp-vk-v1";
/// Seed prefix of one message-specific Solana proof PDA.
pub const SCCP_SOLANA_PROOF_PDA_SEED_V1: &[u8] = b"sccp-proof-v1";
/// Exact fixed byte length of canonical SCCP destination public inputs.
pub const SCCP_SOLANA_DESTINATION_PUBLIC_INPUT_BYTES_V1: usize = 141;
/// Maximum canonical payload bytes stored in a destination proof account.
pub const SCCP_SOLANA_DESTINATION_MAX_PAYLOAD_BYTES_V1: usize = 512;
/// Maximum contiguous upload chunk accepted by the verifier program.
pub const SCCP_SOLANA_DESTINATION_MAX_PROOF_CHUNK_BYTES_V1: usize = 512;
/// Exact largest compact proof body: public inputs, statement hash, proof,
/// payload length, and payload.
pub const SCCP_SOLANA_DESTINATION_PROOF_BODY_MAX_BYTES_V1: usize =
    SCCP_SOLANA_DESTINATION_PUBLIC_INPUT_BYTES_V1
        + 32
        + SCCP_SOLANA_AGAVE_GROTH16_PROOF_BYTES_V1
        + 2
        + SCCP_SOLANA_DESTINATION_MAX_PAYLOAD_BYTES_V1;
/// V1 compact native-verifier opcode for consuming a sealed proof account.
pub const SCCP_SOLANA_VERIFY_SEALED_PROOF_OPCODE_V1: u8 = 6;

/// Closed list of external protocol domains implemented by SCCP V1.
pub const SCCP_CORE_REMOTE_DOMAINS: [u32; 4] = [
    SCCP_DOMAIN_ETH,
    SCCP_DOMAIN_BSC,
    SCCP_DOMAIN_SOLANA,
    SCCP_DOMAIN_TRON,
];

/// Remote SCCP domains in the current supported production launch scope.
pub const SCCP_SUPPORTED_LAUNCH_REMOTE_DOMAINS_V1: [u32; 4] = [
    SCCP_DOMAIN_ETH,
    SCCP_DOMAIN_BSC,
    SCCP_DOMAIN_SOLANA,
    SCCP_DOMAIN_TRON,
];

/// Return whether every key in an account controller is executable by the V1
/// EVM/TVM destination contracts.
///
/// Rust supports additional account-key algorithms, but accepting one as a
/// Taira-origin SCCP sender would create an outbound lock that the immutable
/// first-release destination routes cannot parse exactly. V1 therefore admits
/// single-key and canonical multisig controllers composed only from Ed25519 and
/// compressed secp256k1 public keys. This check is an economic admission rule,
/// not a signature-policy shortcut: normal transaction authorization still
/// verifies the complete controller before this predicate is reached.
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
    SCCP_DOMAIN_SOLANA,
    SCCP_DOMAIN_TRON,
];

/// External domains with a checked-in value-moving outbound route implementation.
///
pub const SCCP_VALUE_MOVING_OUTBOUND_REMOTE_DOMAINS_V1: [u32; 4] = [
    SCCP_DOMAIN_ETH,
    SCCP_DOMAIN_BSC,
    SCCP_DOMAIN_SOLANA,
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
/// Maximum canonical JSON size accepted for a Groth16 request, result, or artifact.
pub const SCCP_GROTH16_BN254_MAX_JSON_ARTIFACT_BYTES_V1: usize =
    2 * SCCP_GROTH16_BN254_MAX_ENCODED_ARTIFACT_BYTES_V1 + 256 * 1024;
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
/// SCCP intentionally reuses the generic bridge proof type so consensus,
/// bridge, Torii, and destination admission cannot drift into different vote
/// transcripts or quorum rules.
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

/// Runtime Solana accounts chosen for one exact destination settlement.
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
pub struct SccpSolanaDestinationRuntimeAccountsV1 {
    /// Transaction fee payer, signer, SPL-token owner, and payload recipient.
    #[norito(with = "json_utils::hex32")]
    pub payer: H256,
    /// Writable SPL token account receiving the governed mint.
    #[norito(with = "json_utils::hex32")]
    pub destination_token_account: H256,
    /// Native-verifier-owned account staging the canonical proof material.
    #[norito(with = "json_utils::hex32")]
    pub proof_account: H256,
    /// Bridge verifier-authority PDA signing the native-verifier CPI.
    #[norito(with = "json_utils::hex32")]
    pub bridge_verifier_authority: H256,
}

/// Exact proof-account header committed by `init-proof` and checked at seal.
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
pub struct SccpSolanaDestinationProofHeaderV1 {
    /// Governed sealed verification-material account.
    #[norito(with = "json_utils::hex32")]
    pub material_account: H256,
    /// Exact compact proof-body byte length.
    pub body_len: u16,
    /// SHA-256 of the exact compact proof body.
    #[norito(with = "json_utils::hex32")]
    pub body_sha256: H256,
    /// Exact SCCP message id.
    #[norito(with = "json_utils::hex32")]
    pub message_id: H256,
    /// Exact canonical SCCP payload hash.
    #[norito(with = "json_utils::hex32")]
    pub payload_hash: H256,
    /// Exact typed Groth16 statement hash.
    #[norito(with = "json_utils::hex32")]
    pub statement_hash: H256,
    /// Exact destination SPL token account.
    #[norito(with = "json_utils::hex32")]
    pub destination_token_account: H256,
    /// Proof-account payer, signer, and SPL-token owner.
    #[norito(with = "json_utils::hex32")]
    pub payer: H256,
    /// Positive nine-decimal SPL base-unit amount.
    #[norito(with = "json_utils::u64_string")]
    pub amount: u64,
}

/// One contiguous proof-account upload chunk.
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
pub struct SccpSolanaDestinationProofChunkV1 {
    /// Contiguous offset in the compact proof body.
    pub offset: u16,
    /// Nonempty bytes; at most 512 bytes.
    #[norito(with = "json_utils::bytes_hex")]
    pub bytes: Vec<u8>,
    /// Exact `[1,4,offset:u16le,len:u16le,chunk]` append wire.
    #[norito(with = "json_utils::bytes_hex")]
    pub instruction_data: Vec<u8>,
}

/// Canonical contents staged and sealed in one Solana proof account.
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
pub struct SccpSolanaDestinationProofAccountV1 {
    /// Proof-account schema version. V1 accepts exactly `1`.
    pub version: u8,
    /// Exact genesis-bound target network.
    pub network: SccpNetworkV1,
    /// Message-specific accounts committed by the sealed proof account.
    pub runtime_accounts: SccpSolanaDestinationRuntimeAccountsV1,
    /// Exact governed route, mint, program, state, verifier, and key material.
    pub deployment: SccpSolanaDestinationDeploymentV1,
    /// Nonzero immutable governed route revision.
    pub route_revision: u32,
    /// Nine-decimal Taira payload amount authenticated by the proof.
    #[norito(with = "json_utils::u128_string")]
    pub payload_amount: u128,
    /// Positive nine-decimal SPL base-unit amount accepted by the verifier.
    #[norito(with = "json_utils::u64_string")]
    pub amount: u64,
    /// Exact governed destination binding.
    #[norito(with = "json_utils::hex32")]
    pub destination_binding_hash: H256,
    /// Exact immutable route-configuration commitment.
    #[norito(with = "json_utils::hex32")]
    pub route_configuration_hash: H256,
    /// Exact audited semantic-proof profile commitment.
    #[norito(with = "json_utils::hex32")]
    pub semantic_proof_profile_hash: H256,
    /// Exact governed Taira-finality-anchor commitment.
    #[norito(with = "json_utils::hex32")]
    pub sora_finality_anchor_hash: H256,
    /// Hash of the typed canonical SCCP proof statement.
    #[norito(with = "json_utils::hex32")]
    pub statement_hash: H256,
    /// Hash of the exact canonical proving request.
    #[norito(with = "json_utils::hex32")]
    pub request_hash: H256,
    /// Public message statement authenticated by the proof.
    pub public_inputs: SccpMessagePublicInputsV1,
    /// Canonical fixed-width BN254 Groth16 proof.
    #[norito(with = "json_utils::bytes_hex")]
    pub proof_bytes: Vec<u8>,
    /// Exact canonical SCCP transfer payload.
    #[norito(with = "json_utils::bytes_hex")]
    pub canonical_payload_bytes: Vec<u8>,
    /// Exact compact on-chain body:
    /// `public_inputs[141] || statement_hash[32] || proof[384] ||
    /// payload_len:u16le || payload`.
    #[norito(with = "json_utils::bytes_hex")]
    pub proof_body: Vec<u8>,
    /// Header committed by the initialization instruction.
    pub header: SccpSolanaDestinationProofHeaderV1,
    /// Exact `[1,3,...]` proof-account initialization wire.
    #[norito(with = "json_utils::bytes_hex")]
    pub init_instruction_data: Vec<u8>,
    /// Contiguous upload plan covering the body exactly once.
    pub chunks: Vec<SccpSolanaDestinationProofChunkV1>,
    /// Exact `[1,5]` seal wire.
    #[norito(with = "json_utils::bytes_hex")]
    pub seal_instruction_data: Vec<u8>,
}

/// Exact seed roles used to derive the sealed verifier-material PDA.
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
pub struct SccpSolanaMaterialPdaSeedsV1 {
    /// Keccak-256 of the canonical governed BN254 verification key.
    #[norito(with = "json_utils::hex32")]
    pub verifier_key_keccak: H256,
    /// SHA-256 of the governed native-verifier configuration.
    #[norito(with = "json_utils::hex32")]
    pub verifier_config_sha256: H256,
}

/// Exact seed roles used to derive one message-specific proof PDA.
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
pub struct SccpSolanaProofPdaSeedsV1 {
    /// Governed sealed verification-material PDA.
    #[norito(with = "json_utils::hex32")]
    pub material_account: H256,
    /// Exact SCCP message id.
    #[norito(with = "json_utils::hex32")]
    pub message_id: H256,
    /// Transaction payer and SPL-token owner.
    #[norito(with = "json_utils::hex32")]
    pub payer: H256,
}

/// Exact seven-account order of compact opcode `6` verification.
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
pub struct SccpSolanaDestinationVerifyAccountsV1 {
    /// Writable transaction payer and signer.
    #[norito(with = "json_utils::hex32")]
    pub payer: H256,
    /// Read-only bridge verifier-authority PDA signer.
    #[norito(with = "json_utils::hex32")]
    pub bridge_verifier_authority: H256,
    /// Read-only governed bridge state.
    #[norito(with = "json_utils::hex32")]
    pub bridge_state: H256,
    /// Read-only governed SPL mint.
    #[norito(with = "json_utils::hex32")]
    pub mint: H256,
    /// Read-only destination SPL token account.
    #[norito(with = "json_utils::hex32")]
    pub destination_token_account: H256,
    /// Read-only sealed verifier-material account.
    #[norito(with = "json_utils::hex32")]
    pub material_account: H256,
    /// Read-only sealed message proof account.
    #[norito(with = "json_utils::hex32")]
    pub proof_account: H256,
}

/// Fully verified, compact Solana settlement call.
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
pub struct SccpVerifiedSolanaDestinationCallV1 {
    /// Call schema version. V1 accepts exactly `1`.
    pub version: u8,
    /// Closed destination backend. V1 accepts only `SolanaGroth16Bn254`.
    pub backend: BridgeSccpDestinationProofBackendV1,
    /// Canonical proof-account value to stage before settlement.
    pub proof_account: SccpSolanaDestinationProofAccountV1,
    /// Host-side derived public signals; the program recomputes these and does
    /// not upload or trust them as proof-account bytes.
    pub public_signals: SccpGroth16Bn254PublicSignalsV1,
    /// Exact governed material-PDA seed roles.
    pub material_pda_seeds: SccpSolanaMaterialPdaSeedsV1,
    /// Exact message-specific proof-PDA seed roles.
    pub proof_pda_seeds: SccpSolanaProofPdaSeedsV1,
    /// Exact seven accounts in compact verification order.
    pub verify_accounts: SccpSolanaDestinationVerifyAccountsV1,
    /// Exact `[1,6,message_id[32],amount:u64le]` verification wire.
    #[norito(with = "json_utils::bytes_hex")]
    pub verify_instruction_data: Vec<u8>,
    /// Original typed message and finality bundle retained for audit.
    pub bundle: TairaSccpMessageProofV1,
}

fn sha256_bytes(payload: &[u8]) -> H256 {
    Sha256::digest(payload).into()
}

fn solana_destination_account_roles_are_valid_v1(
    runtime: SccpSolanaDestinationRuntimeAccountsV1,
    deployment: &SccpSolanaDestinationDeploymentV1,
) -> bool {
    let accounts = [
        runtime.payer,
        runtime.destination_token_account,
        runtime.proof_account,
        runtime.bridge_verifier_authority,
        deployment.token_mint_address,
        deployment.route_program_id,
        deployment.route_program_data_address,
        deployment.route_state_account,
        deployment.native_verifier_program_id,
        deployment.native_verifier_program_data_address,
        deployment.native_verifier_material_account,
    ];
    accounts.iter().all(h256_is_nonzero) && !hash_roles_alias(&accounts)
}

fn sccp_solana_payload_amount_to_spl_base_units_v1(
    payload_amount: u128,
    deployment: &SccpSolanaDestinationDeploymentV1,
) -> Option<u64> {
    if deployment.taira_to_token_multiplier != SCCP_V1_TAIRA_TO_SOLANA_TOKEN_MULTIPLIER {
        return None;
    }
    payload_amount
        .checked_mul(u128::from(deployment.taira_to_token_multiplier))
        .and_then(|amount| u64::try_from(amount).ok())
}

fn solana_destination_proof_body_bytes_v1(
    account: &SccpSolanaDestinationProofAccountV1,
) -> Option<Vec<u8>> {
    let public_inputs = canonical_sccp_message_public_inputs_bytes(&account.public_inputs);
    if public_inputs.len() != SCCP_SOLANA_DESTINATION_PUBLIC_INPUT_BYTES_V1
        || account.proof_bytes.len() != SCCP_SOLANA_AGAVE_GROTH16_PROOF_BYTES_V1
        || account.canonical_payload_bytes.is_empty()
        || account.canonical_payload_bytes.len() > SCCP_SOLANA_DESTINATION_MAX_PAYLOAD_BYTES_V1
    {
        return None;
    }
    let payload_len = u16::try_from(account.canonical_payload_bytes.len()).ok()?;
    let mut body = Vec::with_capacity(
        public_inputs.len()
            + 32
            + account.proof_bytes.len()
            + 2
            + account.canonical_payload_bytes.len(),
    );
    body.extend_from_slice(&public_inputs);
    body.extend_from_slice(&account.statement_hash);
    body.extend_from_slice(&account.proof_bytes);
    body.extend_from_slice(&payload_len.to_le_bytes());
    body.extend_from_slice(&account.canonical_payload_bytes);
    Some(body)
}

/// Validate one sealed Solana destination proof-account value.
#[must_use]
#[expect(
    clippy::too_many_lines,
    reason = "this fail-closed V1 validator keeps the proof-account body, header, chunk, public-input, payload, and deployment bindings in one auditable protocol boundary"
)]
pub fn sccp_solana_destination_proof_account_is_well_formed_v1(
    account: &SccpSolanaDestinationProofAccountV1,
) -> bool {
    if account.version != 1
        || account.network != SccpNetworkV1::SolanaTestnet
        || account.route_revision == 0
        || account.payload_amount == 0
        || account.amount == 0
        || !solana_destination_account_roles_are_valid_v1(
            account.runtime_accounts,
            &account.deployment,
        )
        || sccp_groth16_bn254_verifying_key_hash_v1(account.deployment.verifying_key)
            != Some(account.deployment.verifier_key_hash)
        || account
            .deployment
            .outbound_proof_policy
            .semantic_profile_hash()
            .ok()
            != Some(account.semantic_proof_profile_hash)
        || account
            .deployment
            .outbound_proof_policy
            .sora_finality_anchor_hash()
            .ok()
            != Some(account.sora_finality_anchor_hash)
        || account.public_inputs.version != 1
        || account.public_inputs.target_domain != SCCP_DOMAIN_SOLANA
        || account.proof_bytes.len() != SCCP_SOLANA_AGAVE_GROTH16_PROOF_BYTES_V1
        || account.canonical_payload_bytes.is_empty()
        || account.canonical_payload_bytes.len() > SCCP_SOLANA_DESTINATION_MAX_PAYLOAD_BYTES_V1
        || [
            account.destination_binding_hash,
            account.route_configuration_hash,
            account.semantic_proof_profile_hash,
            account.sora_finality_anchor_hash,
            account.statement_hash,
            account.request_hash,
        ]
        .iter()
        .any(|hash| !h256_is_nonzero(hash))
        || hash_roles_alias(&[
            account.destination_binding_hash,
            account.route_configuration_hash,
            account.semantic_proof_profile_hash,
            account.sora_finality_anchor_hash,
            account.statement_hash,
            account.request_hash,
        ])
    {
        return false;
    }

    if sccp_solana_payload_amount_to_spl_base_units_v1(account.payload_amount, &account.deployment)
        != Some(account.amount)
    {
        return false;
    }

    let Some(payload) = decode_canonical_sccp_payload_bytes(&account.canonical_payload_bytes)
    else {
        return false;
    };
    let SccpPayloadV1::Transfer(transfer) = &payload;
    let payload_amount_matches_transfer = transfer.amount == account.payload_amount;
    if canonical_sccp_payload_bytes(&payload).ok().as_deref()
        != Some(account.canonical_payload_bytes.as_slice())
        || !sccp_payload_matches_exact_xor_destination_route_v1(&payload, SCCP_DOMAIN_SOLANA)
        || !payload_amount_matches_transfer
        || transfer.route_revision != account.route_revision
        || transfer.recipient.as_slice() != account.runtime_accounts.payer
        || payload_hash(&account.canonical_payload_bytes) != account.public_inputs.payload_hash
    {
        return false;
    }

    let Some(expected_body) = solana_destination_proof_body_bytes_v1(account) else {
        return false;
    };
    let body_sha256 = sha256_bytes(&expected_body);
    let expected_header = SccpSolanaDestinationProofHeaderV1 {
        material_account: account.deployment.native_verifier_material_account,
        body_len: u16::try_from(expected_body.len()).expect("bounded compact proof body"),
        body_sha256,
        message_id: account.public_inputs.message_id,
        payload_hash: account.public_inputs.payload_hash,
        statement_hash: account.statement_hash,
        destination_token_account: account.runtime_accounts.destination_token_account,
        payer: account.runtime_accounts.payer,
        amount: account.amount,
    };
    if account.proof_body != expected_body
        || account.header != expected_header
        || account.init_instruction_data
            != encode_sccp_solana_init_proof_instruction_v1(&expected_header)
        || account.chunks != build_sccp_solana_destination_proof_chunks_v1(&expected_body)
        || account.seal_instruction_data != [1, 5]
    {
        return false;
    }

    let proof = decode_sccp_evm_groth16_bn254_proof_bytes(&account.proof_bytes);
    proof.is_some_and(|proof| {
        proof.version == 1
            && proof.message_id == account.public_inputs.message_id
            && proof.source_domain == SCCP_DOMAIN_SORA
            && proof.commitment_root == account.public_inputs.commitment_root
            && encode_sccp_evm_groth16_bn254_proof_bytes(&proof) == account.proof_bytes
    })
}

/// Encode the exact bounded V1 bytes sealed into a Solana proof account.
///
/// The body is exactly `public_inputs[141] || statement_hash[32] ||
/// groth16_proof[384] || payload_len:u16le || canonical_payload`. Account,
/// deployment, and payer roles are bound by the initialization header, PDA
/// seeds, and instruction accounts rather than duplicated in the body.
#[must_use]
pub fn canonical_sccp_solana_destination_proof_account_bytes_v1(
    account: &SccpSolanaDestinationProofAccountV1,
) -> Option<Vec<u8>> {
    if !sccp_solana_destination_proof_account_is_well_formed_v1(account) {
        return None;
    }
    Some(account.proof_body.clone())
}

/// Hash the exact compact proof body with Solana's native SHA-256 primitive.
#[must_use]
pub fn sccp_solana_destination_proof_account_hash_v1(
    account: &SccpSolanaDestinationProofAccountV1,
) -> Option<H256> {
    Some(sha256_bytes(
        &canonical_sccp_solana_destination_proof_account_bytes_v1(account)?,
    ))
}

/// Encode exact `[1,3,...]` proof-account initialization bytes.
#[must_use]
pub fn encode_sccp_solana_init_proof_instruction_v1(
    header: &SccpSolanaDestinationProofHeaderV1,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(172);
    out.extend_from_slice(&[1, 3]);
    out.extend_from_slice(&header.body_len.to_le_bytes());
    out.extend_from_slice(&header.body_sha256);
    out.extend_from_slice(&header.message_id);
    out.extend_from_slice(&header.payload_hash);
    out.extend_from_slice(&header.statement_hash);
    out.extend_from_slice(&header.destination_token_account);
    out.extend_from_slice(&header.amount.to_le_bytes());
    out
}

/// Build exact contiguous `[1,4,offset,len,chunk]` proof upload wires.
#[must_use]
pub fn build_sccp_solana_destination_proof_chunks_v1(
    body: &[u8],
) -> Vec<SccpSolanaDestinationProofChunkV1> {
    let mut chunks = Vec::new();
    let mut offset = 0usize;
    while offset < body.len() {
        let end = core::cmp::min(
            offset + SCCP_SOLANA_DESTINATION_MAX_PROOF_CHUNK_BYTES_V1,
            body.len(),
        );
        let bytes = body[offset..end].to_vec();
        let offset_u16 = u16::try_from(offset).expect("bounded compact proof offset");
        let len_u16 = u16::try_from(bytes.len()).expect("bounded proof chunk");
        let mut instruction_data = Vec::with_capacity(6 + bytes.len());
        instruction_data.extend_from_slice(&[1, 4]);
        instruction_data.extend_from_slice(&offset_u16.to_le_bytes());
        instruction_data.extend_from_slice(&len_u16.to_le_bytes());
        instruction_data.extend_from_slice(&bytes);
        chunks.push(SccpSolanaDestinationProofChunkV1 {
            offset: offset_u16,
            bytes,
            instruction_data,
        });
        offset = end;
    }
    chunks
}

/// Encode exact compact `[1,6,message_id,amount:u64le]` verification bytes.
#[must_use]
pub fn encode_sccp_solana_verify_sealed_proof_instruction_v1(
    message_id: H256,
    amount: u64,
) -> Option<Vec<u8>> {
    if !h256_is_nonzero(&message_id) || amount == 0 {
        return None;
    }
    let mut out = Vec::with_capacity(42);
    out.extend_from_slice(&[1, SCCP_SOLANA_VERIFY_SEALED_PROOF_OPCODE_V1]);
    out.extend_from_slice(&message_id);
    out.extend_from_slice(&amount.to_le_bytes());
    Some(out)
}

/// Return whether a serialized Solana call is internally canonical.
#[must_use]
pub fn sccp_verified_solana_destination_call_is_self_canonical_v1(
    call: &SccpVerifiedSolanaDestinationCallV1,
) -> bool {
    if call.version != 1 || call.backend != BridgeSccpDestinationProofBackendV1::SolanaGroth16Bn254
    {
        return false;
    }
    let account = &call.proof_account;
    let expected_signals = sccp_groth16_bn254_public_signal_words(
        &account.public_inputs,
        SCCP_DOMAIN_SORA,
        account.statement_hash,
        account.destination_binding_hash,
        account.route_configuration_hash,
        account.sora_finality_anchor_hash,
    );
    let expected_verify_accounts = SccpSolanaDestinationVerifyAccountsV1 {
        payer: account.runtime_accounts.payer,
        bridge_verifier_authority: account.runtime_accounts.bridge_verifier_authority,
        bridge_state: account.deployment.route_state_account,
        mint: account.deployment.token_mint_address,
        destination_token_account: account.runtime_accounts.destination_token_account,
        material_account: account.deployment.native_verifier_material_account,
        proof_account: account.runtime_accounts.proof_account,
    };
    let Some(canonical_payload) =
        decode_canonical_sccp_payload_bytes(&account.canonical_payload_bytes)
    else {
        return false;
    };
    sccp_solana_destination_proof_account_is_well_formed_v1(account)
        && call.public_signals.words() == expected_signals
        && call.material_pda_seeds
            == (SccpSolanaMaterialPdaSeedsV1 {
                verifier_key_keccak: account.deployment.verifier_key_hash,
                verifier_config_sha256: account.deployment.native_verifier_config_hash,
            })
        && call.proof_pda_seeds
            == (SccpSolanaProofPdaSeedsV1 {
                material_account: account.deployment.native_verifier_material_account,
                message_id: account.public_inputs.message_id,
                payer: account.runtime_accounts.payer,
            })
        && call.verify_accounts == expected_verify_accounts
        && call.verify_instruction_data
            == encode_sccp_solana_verify_sealed_proof_instruction_v1(
                account.public_inputs.message_id,
                account.amount,
            )
            .unwrap_or_default()
        && call.bundle.payload == canonical_payload
        && call.bundle.commitment.message_id == account.public_inputs.message_id
        && call.bundle.commitment.payload_hash == account.public_inputs.payload_hash
        && call.bundle.commitment_root == account.public_inputs.commitment_root
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// One canonically framed destination artifact with its embedded bundle and
/// finality proof decoded exactly once but not yet trusted against governance.
///
/// Fields are intentionally private. Callers may inspect them to resolve the
/// authoritative historical route, but only
/// [`verify_parsed_sccp_destination_proof_v1`] can create the opaque verified
/// context used to bypass repeated cryptographic verification.
pub struct SccpParsedDestinationProofV1 {
    artifact: SccpGroth16Bn254ProofArtifactV1,
    bundle: TairaSccpMessageProofV1,
    finality: TairaBridgeFinalityProofV1,
    canonical_payload_bytes: Vec<u8>,
    public_signal_words: [H256; 11],
    groth16_proof: SccpEvmGroth16Bn254ProofV1,
}

impl SccpParsedDestinationProofV1 {
    /// Return the canonically decoded Groth16 artifact.
    #[must_use]
    pub const fn artifact(&self) -> &SccpGroth16Bn254ProofArtifactV1 {
        &self.artifact
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
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Opaque route-bound destination verification result.
///
/// Construction proves that the embedded finality proof has passed its one
/// Groth16 pairing and that the finality projection is structurally canonical.
/// Core must bind that projection to its trusted local block and QC, then
/// perform the single authoritative BLS aggregate verification there.
pub struct SccpVerifiedDestinationContextV1 {
    call: SccpVerifiedDestinationCallV1,
    finality: TairaBridgeFinalityProofV1,
}

impl SccpVerifiedDestinationContextV1 {
    /// Return the exact governed destination call.
    #[must_use]
    pub const fn call(&self) -> &SccpVerifiedDestinationCallV1 {
        &self.call
    }

    /// Return the structurally checked finality projection awaiting Core's
    /// authoritative local-QC BLS verification.
    #[must_use]
    pub const fn finality(&self) -> &TairaBridgeFinalityProofV1 {
        &self.finality
    }

    /// Consume the context and return its destination call.
    #[must_use]
    pub fn into_call(self) -> SccpVerifiedDestinationCallV1 {
        self.call
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Opaque Solana route-bound destination verification result.
///
/// Its proof-account call has passed the exact governed request comparison and
/// one BN254 pairing. Core must still bind the retained finality projection to
/// its trusted local block and QC before consuming the call.
pub struct SccpVerifiedSolanaDestinationContextV1 {
    call: SccpVerifiedSolanaDestinationCallV1,
    finality: TairaBridgeFinalityProofV1,
}

impl SccpVerifiedSolanaDestinationContextV1 {
    /// Return the exact compact Solana destination call.
    #[must_use]
    pub const fn call(&self) -> &SccpVerifiedSolanaDestinationCallV1 {
        &self.call
    }

    /// Return the structurally checked Taira finality projection.
    #[must_use]
    pub const fn finality(&self) -> &TairaBridgeFinalityProofV1 {
        &self.finality
    }

    /// Consume the context and return its compact destination call.
    #[must_use]
    pub fn into_call(self) -> SccpVerifiedSolanaDestinationCallV1 {
        self.call
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
    /// Raw 32-byte Solana public key.
    SolanaPubkey32 {
        /// Canonical public-key bytes.
        bytes: [u8; 32],
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
            Self::SolanaPubkey32 { bytes } => {
                write_json_key(out, "SolanaPubkey32");
                out.push('{');
                write_json_key(out, "bytes");
                write_prefixed_hex_json(out, bytes);
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
            "SolanaPubkey32" => {
                json_require_exact_fields(
                    "SccpNormalizedCodecValueV1::SolanaPubkey32",
                    payload,
                    &["bytes"],
                )?;
                Ok(Self::SolanaPubkey32 {
                    bytes: json_fixed_hex_field::<32>(
                        "SccpNormalizedCodecValueV1",
                        payload,
                        "bytes",
                    )?,
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

/// Return whether `domain_id` is a recognized SCCP V1 protocol domain.
pub fn is_supported_domain(domain_id: u32) -> bool {
    matches!(
        domain_id,
        SCCP_DOMAIN_SORA
            | SCCP_DOMAIN_ETH
            | SCCP_DOMAIN_BSC
            | SCCP_DOMAIN_SOLANA
            | SCCP_DOMAIN_TRON
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
        SCCP_DOMAIN_ETH | SCCP_DOMAIN_BSC | SCCP_DOMAIN_SOLANA | SCCP_DOMAIN_TRON
    )
}

/// Return whether a remote domain has a checked-in value-moving outbound route in V1.
pub const fn sccp_domain_has_value_moving_outbound_route_v1(domain_id: u32) -> bool {
    matches!(
        domain_id,
        SCCP_DOMAIN_ETH | SCCP_DOMAIN_BSC | SCCP_DOMAIN_SOLANA | SCCP_DOMAIN_TRON
    )
}

/// Return whether `codec_id` is one of the closed SCCP V1 wire codecs.
pub fn is_supported_codec(codec_id: u8) -> bool {
    matches!(
        codec_id,
        SCCP_CODEC_CANONICAL_TEXT
            | SCCP_CODEC_EVM_ADDRESS20
            | SCCP_CODEC_TRON_ADDRESS21
            | SCCP_CODEC_SOLANA_PUBKEY32
    )
}

/// Return the stable machine-readable name of one SCCP wire codec.
pub fn sccp_codec_key(codec_id: u8) -> Option<&'static str> {
    match codec_id {
        SCCP_CODEC_CANONICAL_TEXT => Some("canonical_text"),
        SCCP_CODEC_EVM_ADDRESS20 => Some("evm_address20"),
        SCCP_CODEC_TRON_ADDRESS21 => Some("tron_address21"),
        SCCP_CODEC_SOLANA_PUBKEY32 => Some("solana_pubkey32"),
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
        SCCP_CODEC_SOLANA_PUBKEY32 => Some("Raw nonzero 32-byte Solana public keys."),
        _ => None,
    }
}

/// Return the stable chain-family key for one SCCP protocol domain.
pub fn sccp_chain_key_for_domain(domain: u32) -> Option<&'static str> {
    match domain {
        SCCP_DOMAIN_SORA => Some("sora"),
        SCCP_DOMAIN_ETH => Some("eth"),
        SCCP_DOMAIN_BSC => Some("bsc"),
        SCCP_DOMAIN_SOLANA => Some("solana"),
        SCCP_DOMAIN_TRON => Some("tron"),
        _ => None,
    }
}

/// Return the account-identifier codec required by one external domain.
pub fn sccp_counterparty_account_codec(domain: u32) -> Option<u8> {
    match domain {
        SCCP_DOMAIN_SORA => Some(SCCP_CODEC_CANONICAL_TEXT),
        SCCP_DOMAIN_ETH | SCCP_DOMAIN_BSC => Some(SCCP_CODEC_EVM_ADDRESS20),
        SCCP_DOMAIN_SOLANA => Some(SCCP_CODEC_SOLANA_PUBKEY32),
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
/// External-origin messages deliberately return `None`; inbound admission uses
/// the closed protocol-native proof API and never constructs an outbound
/// counterparty artifact.
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
/// The output order matches `SccpGroth16Bn254MessageVerifier`: message id,
/// payload hash, target-domain word, commitment root, finality-height word,
/// finality block hash, source-domain word, statement hash, and destination
/// binding hash, immutable route-configuration hash, and governed SORA
/// finality-anchor hash. Each word is
/// `keccak256(abi.encode(keccak256(label), value)) mod Fr` encoded as a
/// big-endian 32-byte BN254 scalar.
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
    verifying_key: SccpGroth16Bn254VerifyingKeyV1,
) -> Option<Vec<u8>> {
    if !sccp_groth16_bn254_verifying_key_is_well_formed_v1(&verifying_key) {
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
    verifying_key: SccpGroth16Bn254VerifyingKeyV1,
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
/// `SccpGroth16Bn254MessageVerifier.sol`. The expected key hash must come from
/// the request derived from typed governed deployment state, never from
/// proof-controlled metadata. The semantic-profile hash is not a twelfth
/// signal; it is nevertheless required here so all six governed hash roles
/// receive the same nonzero, pairwise-distinct admission check as the
/// destination verifier.
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

/// Decode the compact canonical message-bundle byte layout embedded in one
/// Groth16 prover request.
///
/// This is distinct from [`decode_taira_sccp_message_proof`], which decodes a
/// top-level Norito-framed Torii artifact. The embedded request layout is
/// length-delimited by the SCCP protocol itself and must never be guessed as a
/// Norito frame.
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
        BridgeSccpDestinationProofBackendV1::SolanaGroth16Bn254 => 2,
    }
}

fn sccp_destination_proof_backend_supports_network_v1(
    backend: BridgeSccpDestinationProofBackendV1,
    target_network: SccpNetworkV1,
) -> bool {
    match backend {
        BridgeSccpDestinationProofBackendV1::EvmGroth16Bn254 => matches!(
            target_network,
            SccpNetworkV1::EthereumMainnet
                | SccpNetworkV1::EthereumSepolia
                | SccpNetworkV1::BscMainnet
                | SccpNetworkV1::BscTestnet
        ),
        BridgeSccpDestinationProofBackendV1::TronGroth16Bn254 => matches!(
            target_network,
            SccpNetworkV1::TronMainnet | SccpNetworkV1::TronNile | SccpNetworkV1::TronShasta
        ),
        BridgeSccpDestinationProofBackendV1::SolanaGroth16Bn254 => {
            target_network == SccpNetworkV1::SolanaTestnet
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
        canonical_sccp_groth16_bn254_verifying_key_bytes_v1(request.verifying_key)?;
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
        && sccp_groth16_bn254_verifying_key_hash_v1(*context.verifying_key)
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
        SccpDestinationDeploymentV1::Solana(deployment) => (
            deployment.verifying_key,
            deployment.verifier_key_hash,
            deployment.outbound_proof_policy,
        ),
    };
    (sccp_groth16_bn254_verifying_key_hash_v1(verifying_key) == Some(verifier_key_hash)
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
        SccpDestinationDeploymentV1::Solana(_) => {
            BridgeSccpDestinationProofBackendV1::SolanaGroth16Bn254
        }
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
        && sccp_groth16_bn254_verifying_key_hash_v1(request.verifying_key)
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
    if !matches!(
        request.target_network,
        SccpNetworkV1::BscMainnet | SccpNetworkV1::BscTestnet
    ) || request.public_inputs.target_domain != SCCP_DOMAIN_BSC
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

/// Validate and bind raw Solana-program Groth16 proof bytes to their exact
/// state-derived proving request.
pub fn wrap_sccp_solana_groth16_bn254_proof_result(
    proof_bytes: &[u8],
    request: &SccpGroth16Bn254ProofRequestV1,
) -> Option<SccpGroth16Bn254ProofArtifactV1> {
    wrap_sccp_groth16_bn254_proof_result(
        proof_bytes,
        request,
        BridgeSccpDestinationProofBackendV1::SolanaGroth16Bn254,
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
        SccpDestinationDeploymentV1::Solana(_) => {
            wrap_sccp_solana_groth16_bn254_proof_result(&artifact.result.proof_bytes, &request)
        }
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
        // Solana uses its dedicated proof-account API because payer and the
        // destination SPL token account are transaction-specific. Never erase
        // those bindings to fit the EVM/TVM calldata DTO; callers use
        // `verify_sccp_solana_destination_proof_v1` instead.
        SccpDestinationDeploymentV1::Solana(_) => return None,
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

#[expect(
    clippy::large_types_passed_by_value,
    reason = "the canonical eleven-word signal array is owned by the parsed proof and moved directly into the returned V1 call without cloning"
)]
fn build_sccp_verified_solana_destination_call_v1(
    bundle: &TairaSccpMessageProofV1,
    artifact: &SccpGroth16Bn254ProofArtifactV1,
    governed_route: &SccpGovernedRouteV1,
    runtime_accounts: SccpSolanaDestinationRuntimeAccountsV1,
    canonical_payload_bytes: Vec<u8>,
    public_signal_words: [H256; 11],
) -> Option<SccpVerifiedSolanaDestinationCallV1> {
    let SccpDestinationDeploymentV1::Solana(deployment) = governed_route.destination else {
        return None;
    };
    let SccpPayloadV1::Transfer(transfer) = &bundle.payload;
    if governed_route.lane_id.source != SccpNetworkV1::SolanaTestnet
        || governed_route.lane_id.target != SccpNetworkV1::SoraTaira
        || artifact.request.backend != BridgeSccpDestinationProofBackendV1::SolanaGroth16Bn254
        || artifact.request.target_network != SccpNetworkV1::SolanaTestnet
        || transfer.recipient_codec != SCCP_CODEC_SOLANA_PUBKEY32
        || transfer.recipient.as_slice() != runtime_accounts.payer
        || artifact.request.bundle_bytes
            != canonical_taira_sccp_message_bundle_bytes_checked(bundle)?
    {
        return None;
    }
    let amount = sccp_solana_payload_amount_to_spl_base_units_v1(transfer.amount, &deployment)?;
    let mut proof_account = SccpSolanaDestinationProofAccountV1 {
        version: 1,
        network: SccpNetworkV1::SolanaTestnet,
        runtime_accounts,
        deployment,
        route_revision: transfer.route_revision,
        payload_amount: transfer.amount,
        amount,
        destination_binding_hash: artifact.request.destination_binding_hash,
        route_configuration_hash: artifact.request.route_configuration_hash,
        semantic_proof_profile_hash: artifact.request.semantic_proof_profile_hash,
        sora_finality_anchor_hash: artifact.request.sora_finality_anchor_hash,
        statement_hash: artifact.request.statement_hash,
        request_hash: artifact.request.request_hash,
        public_inputs: artifact.request.public_inputs,
        proof_bytes: artifact.result.proof_bytes.clone(),
        canonical_payload_bytes,
        proof_body: Vec::new(),
        header: SccpSolanaDestinationProofHeaderV1 {
            material_account: deployment.native_verifier_material_account,
            body_len: 0,
            body_sha256: [0; 32],
            message_id: artifact.request.public_inputs.message_id,
            payload_hash: artifact.request.public_inputs.payload_hash,
            statement_hash: artifact.request.statement_hash,
            destination_token_account: runtime_accounts.destination_token_account,
            payer: runtime_accounts.payer,
            amount,
        },
        init_instruction_data: Vec::new(),
        chunks: Vec::new(),
        seal_instruction_data: vec![1, 5],
    };
    proof_account.proof_body = solana_destination_proof_body_bytes_v1(&proof_account)?;
    proof_account.header.body_len = u16::try_from(proof_account.proof_body.len()).ok()?;
    proof_account.header.body_sha256 = sha256_bytes(&proof_account.proof_body);
    proof_account.init_instruction_data =
        encode_sccp_solana_init_proof_instruction_v1(&proof_account.header);
    proof_account.chunks = build_sccp_solana_destination_proof_chunks_v1(&proof_account.proof_body);
    let verify_accounts = SccpSolanaDestinationVerifyAccountsV1 {
        payer: runtime_accounts.payer,
        bridge_verifier_authority: runtime_accounts.bridge_verifier_authority,
        bridge_state: deployment.route_state_account,
        mint: deployment.token_mint_address,
        destination_token_account: runtime_accounts.destination_token_account,
        material_account: deployment.native_verifier_material_account,
        proof_account: runtime_accounts.proof_account,
    };
    let call = SccpVerifiedSolanaDestinationCallV1 {
        version: 1,
        backend: BridgeSccpDestinationProofBackendV1::SolanaGroth16Bn254,
        proof_account,
        public_signals: public_signal_words.into(),
        material_pda_seeds: SccpSolanaMaterialPdaSeedsV1 {
            verifier_key_keccak: deployment.verifier_key_hash,
            verifier_config_sha256: deployment.native_verifier_config_hash,
        },
        proof_pda_seeds: SccpSolanaProofPdaSeedsV1 {
            material_account: deployment.native_verifier_material_account,
            message_id: artifact.request.public_inputs.message_id,
            payer: runtime_accounts.payer,
        },
        verify_accounts,
        verify_instruction_data: encode_sccp_solana_verify_sealed_proof_instruction_v1(
            artifact.request.public_inputs.message_id,
            amount,
        )?,
        bundle: bundle.clone(),
    };
    sccp_verified_solana_destination_call_is_self_canonical_v1(&call).then_some(call)
}

/// Return whether a compact Solana call still matches exact governed history.
///
/// This is the verification boundary for a call deserialized independently of
/// the opaque parsed-proof context. It rechecks route material, canonical
/// bytes, Taira finality, and the BN254 pairing; callers must not trust a
/// mutable DTO merely because it once came from the builder.
#[must_use]
pub fn sccp_verified_solana_destination_call_matches_governed_route_v1(
    call: &SccpVerifiedSolanaDestinationCallV1,
    governed_route: &SccpGovernedRouteV1,
) -> bool {
    let SccpDestinationDeploymentV1::Solana(deployment) = governed_route.destination else {
        return false;
    };
    let Some(finality) = decode_taira_bridge_finality_proof(&call.bundle.finality_proof) else {
        return false;
    };
    let Some(expected_request) =
        build_sccp_groth16_bn254_proof_request_from_governed_route_v1(&call.bundle, governed_route)
    else {
        return false;
    };
    if !sccp_verified_solana_destination_call_is_self_canonical_v1(call)
        || governed_route.validate().is_err()
        || governed_route.lane_id.source != SccpNetworkV1::SolanaTestnet
        || governed_route.lane_id.target != SccpNetworkV1::SoraTaira
        || call.proof_account.deployment != deployment
        || call.proof_account.route_revision != governed_route.revision
        || governed_route.destination_binding_hash().ok()
            != Some(call.proof_account.destination_binding_hash)
        || governed_route.route_configuration_hash().ok()
            != Some(call.proof_account.route_configuration_hash)
        || call.proof_account.statement_hash != expected_request.statement_hash
        || call.proof_account.request_hash != expected_request.request_hash
        || call.proof_account.public_inputs != expected_request.public_inputs
        || call.proof_account.destination_binding_hash != expected_request.destination_binding_hash
        || call.proof_account.route_configuration_hash != expected_request.route_configuration_hash
        || call.proof_account.semantic_proof_profile_hash
            != expected_request.semantic_proof_profile_hash
        || call.proof_account.sora_finality_anchor_hash
            != expected_request.sora_finality_anchor_hash
        || call.proof_account.deployment.verifying_key != expected_request.verifying_key
        || !sccp_governed_groth16_route_matches_bundle_v1(&call.bundle, governed_route)
        || !verify_taira_bridge_finality_proof_cryptographic(&finality)
    {
        return false;
    }
    let Some(proof) = decode_sccp_evm_groth16_bn254_proof_bytes(&call.proof_account.proof_bytes)
    else {
        return false;
    };
    verify_sccp_groth16_bn254_pairing_equation_v1(
        &proof,
        &call.public_signals.words(),
        &deployment.verifying_key,
    )
}

/// Decode one destination artifact, its canonical embedded SCCP bundle, and
/// its Taira finality proof exactly once without evaluating a pairing or BLS
/// aggregate.
///
/// The result is structurally and hash bound, but remains untrusted until it is
/// resolved against historical governed route state by
/// [`verify_parsed_sccp_destination_proof_v1`].
pub fn parse_sccp_destination_proof_v1(
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
        artifact,
        bundle,
        finality,
        canonical_payload_bytes: decoded.canonical_payload_bytes,
        public_signal_words,
        groth16_proof,
    })
}

fn build_sccp_groth16_bn254_proof_request_from_parsed_v1(
    parsed: &SccpParsedDestinationProofV1,
    governed_route: &SccpGovernedRouteV1,
) -> Option<SccpGroth16Bn254ProofRequestV1> {
    if !sccp_governed_groth16_route_matches_bundle_v1(&parsed.bundle, governed_route) {
        return None;
    }
    let request = &parsed.artifact.request;
    let destination_binding_hash = governed_route.destination_binding_hash().ok()?;
    let route_configuration_hash = governed_route.route_configuration_hash().ok()?;
    let (verifying_key, expected_verifier_key_hash, outbound_proof_policy) =
        sccp_governed_route_groth16_material_v1(governed_route)?;
    let backend = match governed_route.destination {
        SccpDestinationDeploymentV1::Evm(_) => BridgeSccpDestinationProofBackendV1::EvmGroth16Bn254,
        SccpDestinationDeploymentV1::Tron(_) => {
            BridgeSccpDestinationProofBackendV1::TronGroth16Bn254
        }
        SccpDestinationDeploymentV1::Solana(_) => {
            BridgeSccpDestinationProofBackendV1::SolanaGroth16Bn254
        }
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

/// Bind one parsed artifact to the exact historical governed route, evaluate
/// one Groth16 pairing, and derive the destination call without decoding or
/// cryptographically re-verifying the submitted request. The opaque result
/// deliberately defers BLS verification to Core's trusted local-QC check.
pub fn verify_parsed_sccp_destination_proof_v1(
    parsed: SccpParsedDestinationProofV1,
    governed_route: &SccpGovernedRouteV1,
) -> Option<SccpVerifiedDestinationContextV1> {
    let expected_request =
        build_sccp_groth16_bn254_proof_request_from_parsed_v1(&parsed, governed_route)?;
    if parsed.artifact.request != expected_request
        || !verify_sccp_groth16_bn254_pairing_equation_v1(
            &parsed.groth16_proof,
            &parsed.public_signal_words,
            &parsed.artifact.request.verifying_key,
        )
    {
        return None;
    }
    let call = build_sccp_verified_destination_call_v1(
        &parsed.bundle,
        &parsed.artifact,
        governed_route,
        parsed.canonical_payload_bytes,
    )?;
    Some(SccpVerifiedDestinationContextV1 {
        call,
        finality: parsed.finality,
    })
}

/// Bind one parsed artifact to exact Solana route history and derive a compact
/// proof-account settlement call.
///
/// Payer, destination SPL token account, proof account, and route PDAs are
/// explicit inputs because they are transaction-specific. The resulting
/// sealed value hashes them together with the governed program, state, mint,
/// verifier material, message, payload, public inputs, and proof.
pub fn verify_parsed_sccp_solana_destination_proof_v1(
    parsed: SccpParsedDestinationProofV1,
    governed_route: &SccpGovernedRouteV1,
    runtime_accounts: SccpSolanaDestinationRuntimeAccountsV1,
) -> Option<SccpVerifiedSolanaDestinationContextV1> {
    let SccpDestinationDeploymentV1::Solana(_) = governed_route.destination else {
        return None;
    };
    let expected_request =
        build_sccp_groth16_bn254_proof_request_from_parsed_v1(&parsed, governed_route)?;
    if parsed.artifact.request != expected_request
        || parsed.artifact.request.backend
            != BridgeSccpDestinationProofBackendV1::SolanaGroth16Bn254
        || !verify_sccp_groth16_bn254_pairing_equation_v1(
            &parsed.groth16_proof,
            &parsed.public_signal_words,
            &parsed.artifact.request.verifying_key,
        )
    {
        return None;
    }
    let call = build_sccp_verified_solana_destination_call_v1(
        &parsed.bundle,
        &parsed.artifact,
        governed_route,
        runtime_accounts,
        parsed.canonical_payload_bytes,
        parsed.public_signal_words,
    )?;
    Some(SccpVerifiedSolanaDestinationContextV1 {
        call,
        finality: parsed.finality,
    })
}

/// Verify one closed bridge SCCP destination proof against the exact bundle
/// and historical governed route, then derive the canonical destination call.
pub fn verify_sccp_destination_proof_v1(
    proof: &BridgeSccpDestinationProofV1,
    bundle: &TairaSccpMessageProofV1,
    governed_route: &SccpGovernedRouteV1,
) -> Option<SccpVerifiedDestinationCallV1> {
    let parsed = parse_sccp_destination_proof_v1(proof)?;
    if parsed.bundle() != bundle {
        return None;
    }
    let verified = verify_parsed_sccp_destination_proof_v1(parsed, governed_route)?;
    if !verify_taira_bridge_finality_proof_cryptographic(verified.finality()) {
        return None;
    }
    Some(verified.into_call())
}

/// Verify one Solana destination proof and derive its compact proof-account
/// transaction material.
///
/// This is the complete query-free entrypoint for callers that do not already
/// hold an opaque parsed context. It performs canonical framing, exact
/// governed-route reconstruction, one BN254 pairing, and Taira BLS finality
/// verification before returning a call.
pub fn verify_sccp_solana_destination_proof_v1(
    proof: &BridgeSccpDestinationProofV1,
    bundle: &TairaSccpMessageProofV1,
    governed_route: &SccpGovernedRouteV1,
    runtime_accounts: SccpSolanaDestinationRuntimeAccountsV1,
) -> Option<SccpVerifiedSolanaDestinationCallV1> {
    if proof.backend != BridgeSccpDestinationProofBackendV1::SolanaGroth16Bn254 {
        return None;
    }
    let parsed = parse_sccp_destination_proof_v1(proof)?;
    if parsed.bundle() != bundle {
        return None;
    }
    let verified =
        verify_parsed_sccp_solana_destination_proof_v1(parsed, governed_route, runtime_accounts)?;
    if !verify_taira_bridge_finality_proof_cryptographic(verified.finality()) {
        return None;
    }
    Some(verified.into_call())
}

fn sccp_exact_xor_destination_route_id_v1(target_domain: u32) -> Option<&'static [u8]> {
    match target_domain {
        SCCP_DOMAIN_ETH => Some(SCCP_TAIRA_ETH_XOR_ROUTE_ID_V1.as_bytes()),
        SCCP_DOMAIN_BSC => Some(SCCP_TAIRA_BSC_XOR_ROUTE_ID_V1.as_bytes()),
        SCCP_DOMAIN_SOLANA => Some(SCCP_TAIRA_SOL_XOR_ROUTE_ID_V1.as_bytes()),
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
        SCCP_DOMAIN_SOLANA => SCCP_CODEC_SOLANA_PUBKEY32,
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
        SCCP_CODEC_SOLANA_PUBKEY32 => Some(SccpNormalizedCodecValueV1::SolanaPubkey32 {
            bytes: decode_nonzero_fixed(bytes)?,
        }),
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
/// The constructor is intentionally fallible. Besides validating the exact
/// SORA-to-external lane against the payload domains, it rejects zero values
/// and collisions among the lane, destination binding, route configuration,
/// message, and payload hash roles. This keeps malformed records out of both
/// Merkle trees and the durable replay index.
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
/// otherwise identical payload from aliasing across external mainnet and
/// testnet profiles. The governed destination binding is not part of this identity;
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
    if proof.version != BRIDGE_FINALITY_PROOF_VERSION_V1
        || artifact.height_context.chain_id.as_str() != SCCP_TAIRA_FINALITY_CHAIN_ID_V1
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
        &SCCP_TAIRA_FINALITY_CHAIN_ID_V1.into(),
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
/// This establishes internal cryptographic consistency for the complete frozen
/// v2 context, exact equal-vote quorum, `PoPs`, and exact commit-vote
/// transcript. The context and roster are still carried by the proof, so
/// callers MUST NOT treat this function as a trust anchor. Production
/// destination proofs additionally bind an audited semantic circuit to a
/// governed [`SccpSoraFinalityAnchorV1`]. BLS verification is mandatory in
/// every build of this crate.
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
    use std::{cell::Cell, sync::OnceLock};

    use halo2curves::{
        Coordinates, CurveAffine,
        bn256::{Fq, Fq2, Fr, G1Affine, G2Affine},
        group::{Curve, prime::PrimeCurveAffine},
    };
    use iroha_data_model::{
        account::{AccountId, MultisigMember, MultisigPolicy},
        bridge::{
            BridgeProofPayload, BridgeSccpDestinationProofBackendV1, BridgeTransparentProof,
            SCCP_V1_TAIRA_TO_SOLANA_TOKEN_MULTIPLIER, SCCP_V1_TAIRA_TO_TOKEN_MULTIPLIER,
            SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE, SccpBn254G1PointV1, SccpBn254G2PointV1,
            SccpDestinationDeploymentV1, SccpEvmDestinationDeploymentV1, SccpEvmSourceEmitterV1,
            SccpGovernedRouteV1, SccpGroth16Bn254IcV1, SccpGroth16Bn254VerifyingKeyV1,
            SccpInboundFinalityCutoffV1, SccpLaneIdV1, SccpNetworkV1, SccpOutboundMessageContextV1,
            SccpRouteActivationV1, SccpSolanaSourceEmitterV1, SccpSoraSettlementV1,
            SccpSourceEmitterV1, SccpSourceIdentityV1, SccpTronDestinationDeploymentV1,
            sccp_exact_tron_xor_route_config_hash_v1, sccp_lane_id_hash_v1,
            sccp_v1_taira_xor_asset_definition_id,
        },
        proof::ProofBox,
    };

    use super::*;

    struct OutboundFixture {
        route: SccpGovernedRouteV1,
        bundle: TairaSccpMessageProofV1,
        request: SccpGroth16Bn254ProofRequestV1,
        artifact: SccpGroth16Bn254ProofArtifactV1,
        bridge_proof: BridgeSccpDestinationProofV1,
    }

    fn word_u64(value: u64) -> H256 {
        let mut word = [0; 32];
        word[24..].copy_from_slice(&value.to_be_bytes());
        word
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
            SccpNormalizedCodecValueV1::SolanaPubkey32 { bytes: [0x13; 32] },
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
            r#"{"Unknown":{"value":"route-v1"}}"#,
        ] {
            assert!(
                norito::json::from_json::<SccpNormalizedCodecValueV1>(hostile).is_err(),
                "hostile normalized codec JSON must be rejected: {hostile}"
            );
        }
    }

    #[test]
    fn transfer_projection_json_preserves_full_integer_ranges_as_canonical_strings() {
        let projection = SccpTransferProjectionV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_SOLANA,
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
            recipient: SccpNormalizedCodecValueV1::SolanaPubkey32 { bytes: [0x13; 32] },
            route_id: SccpNormalizedCodecValueV1::CanonicalText {
                value: SCCP_TAIRA_SOL_XOR_ROUTE_ID_V1.into(),
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
            verifier_key_hash: sccp_groth16_bn254_verifying_key_hash_v1(key)
                .expect("valid repeated-generator key"),
            outbound_proof_policy: outbound_proof_policy(),
            route_address: [0x51; 20],
            route_code_hash: [0x61; 32],
            taira_to_token_multiplier: SCCP_V1_TAIRA_TO_TOKEN_MULTIPLIER,
        }
    }

    fn solana_deployment(revision: u32) -> SccpSolanaDestinationDeploymentV1 {
        let key = verifying_key();
        let mut deployment = SccpSolanaDestinationDeploymentV1 {
            token_mint_address: [0x11; 32],
            route_program_id: [0x12; 32],
            route_program_data_address: [0x13; 32],
            route_program_data_slot: 14,
            route_state_account: [0x15; 32],
            route_program_code_hash: [0x16; 32],
            native_verifier_program_id: [0x17; 32],
            native_verifier_program_data_address: [0x18; 32],
            native_verifier_program_data_slot: 25,
            native_verifier_material_account: [0x1a; 32],
            native_verifier_program_code_hash: [0x1b; 32],
            native_verifier_config_hash: [0x1c; 32],
            verifying_key: key,
            verifier_key_hash: sccp_groth16_bn254_verifying_key_hash_v1(key)
                .expect("valid repeated-generator key"),
            outbound_proof_policy: outbound_proof_policy(),
            taira_to_token_multiplier: SCCP_V1_TAIRA_TO_SOLANA_TOKEN_MULTIPLIER,
        };
        deployment.native_verifier_config_hash = sccp_solana_native_verifier_config_hash_v1(
            SccpLaneIdV1 {
                source: SccpNetworkV1::SolanaTestnet,
                target: SccpNetworkV1::SoraTaira,
            },
            SCCP_TAIRA_SOL_XOR_ROUTE_ID_V1,
            SCCP_TAIRA_XOR_ASSET_KEY_V1,
            revision,
            [0x31; 32],
            &deployment,
        )
        .expect("exact Solana fixture native-verifier config");
        deployment
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
        let custody = KeyPair::try_from_seed(vec![0x71; 32], Algorithm::Ed25519)
            .expect("custody key")
            .public_key()
            .clone();
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
                custody_account_id: AccountId::new(custody),
                payload_amount_scale: SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
            },
        };
        route.validate().expect("valid governed EVM fixture route");
        assert_eq!(
            route.route_configuration_hash().expect("route config"),
            route_config_hash
        );
        route
    }

    fn solana_governed_route(revision: u32) -> SccpGovernedRouteV1 {
        let lane_id = SccpLaneIdV1 {
            source: SccpNetworkV1::SolanaTestnet,
            target: SccpNetworkV1::SoraTaira,
        };
        let deployment = solana_deployment(revision);
        let destination = SccpDestinationDeploymentV1::Solana(deployment);
        let route_config_hash = destination
            .route_configuration_hash(
                lane_id,
                SCCP_TAIRA_SOL_XOR_ROUTE_ID_V1,
                SCCP_TAIRA_XOR_ASSET_KEY_V1,
                revision,
                SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
            )
            .expect("valid exact Solana route configuration");
        let custody = KeyPair::try_from_seed(vec![0x81; 32], Algorithm::Ed25519)
            .expect("Solana fixture custody key")
            .public_key()
            .clone();
        let route = SccpGovernedRouteV1 {
            lane_id,
            route_id: SCCP_TAIRA_SOL_XOR_ROUTE_ID_V1.to_owned(),
            asset_key: SCCP_TAIRA_XOR_ASSET_KEY_V1.to_owned(),
            revision,
            activation: SccpRouteActivationV1::Bidirectional,
            inbound_finality_cutoff: None,
            source_identity: SccpSourceIdentityV1 {
                lane: lane_id,
                emitter: SccpSourceEmitterV1::Solana(SccpSolanaSourceEmitterV1 {
                    program_id: [0x31; 32],
                    program_data_address: [0x32; 32],
                    program_data_slot: 31,
                    state_account: [0x33; 32],
                    program_code_hash: [0x34; 32],
                    route_config_hash,
                }),
            },
            destination,
            sora_outbound_execution_policy: sccp_sora_outbound_execution_policy_test_fixture_v1(),
            settlement: SccpSoraSettlementV1 {
                asset_definition_id: sccp_v1_taira_xor_asset_definition_id(),
                custody_account_id: AccountId::new(custody),
                payload_amount_scale: SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
            },
        };
        route
            .validate()
            .expect("valid governed Solana fixture route");
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

    fn solana_transfer_payload(revision: u32, payer: H256) -> SccpPayloadV1 {
        SccpPayloadV1::Transfer(TransferPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_SOLANA,
            nonce: 17,
            route_revision: revision,
            asset_home_domain: SCCP_DOMAIN_SORA,
            asset_id_codec: SCCP_CODEC_CANONICAL_TEXT,
            asset_id: SCCP_TAIRA_XOR_ASSET_KEY_V1.as_bytes().to_vec(),
            amount: 3,
            sender_codec: SCCP_CODEC_CANONICAL_TEXT,
            sender: b"alice".to_vec(),
            recipient_codec: SCCP_CODEC_SOLANA_PUBKEY32,
            recipient: payer.to_vec(),
            route_id_codec: SCCP_CODEC_CANONICAL_TEXT,
            route_id: SCCP_TAIRA_SOL_XOR_ROUTE_ID_V1.as_bytes().to_vec(),
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
            assert!(verify_sccp_destination_proof_v1(&bridge_proof, &bundle, &route).is_some());
            OutboundFixture {
                route,
                bundle,
                request,
                artifact,
                bridge_proof,
            }
        })
    }

    fn solana_runtime_accounts() -> SccpSolanaDestinationRuntimeAccountsV1 {
        SccpSolanaDestinationRuntimeAccountsV1 {
            payer: [0xa1; 32],
            destination_token_account: [0xa2; 32],
            proof_account: [0xa3; 32],
            bridge_verifier_authority: [0xa4; 32],
        }
    }

    fn solana_fixture() -> &'static OutboundFixture {
        static FIXTURE: OnceLock<OutboundFixture> = OnceLock::new();
        FIXTURE.get_or_init(|| {
            let route = solana_governed_route(1);
            let runtime = solana_runtime_accounts();
            let bundle = message_bundle_with_payload(
                &route,
                solana_transfer_payload(route.revision, runtime.payer),
            );
            let request =
                build_sccp_groth16_bn254_proof_request_from_governed_route_v1(&bundle, &route)
                    .expect("canonical governed Solana request");
            assert_eq!(
                request.backend,
                BridgeSccpDestinationProofBackendV1::SolanaGroth16Bn254
            );
            let proof_bytes = valid_proof(&request);
            assert!(verify_sccp_groth16_bn254_proof_v1(&request, &proof_bytes));
            let artifact = wrap_sccp_solana_groth16_bn254_proof_result(&proof_bytes, &request)
                .expect("valid Solana Groth16 artifact");
            let bridge_proof =
                bridge_sccp_destination_proof_v1(&artifact).expect("closed Solana bridge proof");
            let call =
                verify_sccp_solana_destination_proof_v1(&bridge_proof, &bundle, &route, runtime)
                    .expect("verified Solana proof-account call");
            assert!(sccp_verified_solana_destination_call_matches_governed_route_v1(&call, &route));
            OutboundFixture {
                route,
                bundle,
                request,
                artifact,
                bridge_proof,
            }
        })
    }

    fn verified_solana_call() -> SccpVerifiedSolanaDestinationCallV1 {
        let fixture = solana_fixture();
        verify_sccp_solana_destination_proof_v1(
            &fixture.bridge_proof,
            &fixture.bundle,
            &fixture.route,
            solana_runtime_accounts(),
        )
        .expect("verified Solana proof-account call")
    }

    fn assert_exact_compact_solana_proof_body_wire(account: &SccpSolanaDestinationProofAccountV1) {
        let public_inputs = canonical_sccp_message_public_inputs_bytes(&account.public_inputs);
        assert_eq!(
            public_inputs.len(),
            SCCP_SOLANA_DESTINATION_PUBLIC_INPUT_BYTES_V1
        );

        let statement_offset = SCCP_SOLANA_DESTINATION_PUBLIC_INPUT_BYTES_V1;
        let proof_offset = statement_offset + 32;
        let payload_len_offset = proof_offset + SCCP_SOLANA_AGAVE_GROTH16_PROOF_BYTES_V1;
        let payload_offset = payload_len_offset + 2;
        assert_eq!(&account.proof_body[..statement_offset], public_inputs);
        assert_eq!(
            &account.proof_body[statement_offset..proof_offset],
            &account.statement_hash
        );
        assert_eq!(
            &account.proof_body[proof_offset..payload_len_offset],
            account.proof_bytes
        );
        assert_eq!(
            u16::from_le_bytes(
                account.proof_body[payload_len_offset..payload_offset]
                    .try_into()
                    .expect("two-byte payload length"),
            ) as usize,
            account.canonical_payload_bytes.len()
        );
        assert_eq!(
            &account.proof_body[payload_offset..],
            account.canonical_payload_bytes
        );
        assert_eq!(
            account.proof_body.len(),
            SCCP_SOLANA_DESTINATION_PUBLIC_INPUT_BYTES_V1
                + 32
                + SCCP_SOLANA_AGAVE_GROTH16_PROOF_BYTES_V1
                + 2
                + account.canonical_payload_bytes.len()
        );
        assert!(account.proof_body.len() <= SCCP_SOLANA_DESTINATION_PROOF_BODY_MAX_BYTES_V1);
        assert_eq!(SCCP_SOLANA_DESTINATION_PROOF_BODY_MAX_BYTES_V1, 1_071);
        assert_eq!(
            account.header.body_sha256,
            sha256_bytes(&account.proof_body)
        );
        assert_eq!(
            usize::from(account.header.body_len),
            account.proof_body.len()
        );
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
    fn solana_destination_uses_exact_compact_proof_account_wire() {
        let call = verified_solana_call();
        let account = &call.proof_account;
        assert_eq!(account.payload_amount, 3);
        assert_eq!(account.amount, 3);
        assert_eq!(account.header.amount, 3);
        assert_exact_compact_solana_proof_body_wire(account);

        let mut expected_init = vec![1, 3];
        expected_init.extend_from_slice(&account.header.body_len.to_le_bytes());
        expected_init.extend_from_slice(&account.header.body_sha256);
        expected_init.extend_from_slice(&account.header.message_id);
        expected_init.extend_from_slice(&account.header.payload_hash);
        expected_init.extend_from_slice(&account.header.statement_hash);
        expected_init.extend_from_slice(&account.header.destination_token_account);
        expected_init.extend_from_slice(&account.header.amount.to_le_bytes());
        assert_eq!(account.init_instruction_data, expected_init);
        assert_eq!(account.init_instruction_data.len(), 172);

        let mut reconstructed = Vec::new();
        let mut next_offset = 0usize;
        for chunk in &account.chunks {
            assert_eq!(usize::from(chunk.offset), next_offset);
            assert!(!chunk.bytes.is_empty());
            assert!(chunk.bytes.len() <= SCCP_SOLANA_DESTINATION_MAX_PROOF_CHUNK_BYTES_V1);
            let mut expected = vec![1, 4];
            expected.extend_from_slice(&chunk.offset.to_le_bytes());
            expected.extend_from_slice(
                &u16::try_from(chunk.bytes.len())
                    .expect("bounded chunk")
                    .to_le_bytes(),
            );
            expected.extend_from_slice(&chunk.bytes);
            assert_eq!(chunk.instruction_data, expected);
            reconstructed.extend_from_slice(&chunk.bytes);
            next_offset += chunk.bytes.len();
        }
        assert_eq!(reconstructed, account.proof_body);
        assert_eq!(account.seal_instruction_data, [1, 5]);

        let mut expected_verify = vec![1, 6];
        expected_verify.extend_from_slice(&account.public_inputs.message_id);
        expected_verify.extend_from_slice(&account.amount.to_le_bytes());
        assert_eq!(call.verify_instruction_data, expected_verify);
        assert_eq!(call.verify_instruction_data.len(), 42);
        assert!(sccp_verified_solana_destination_call_is_self_canonical_v1(
            &call
        ));
        assert!(
            sccp_verified_solana_destination_call_matches_governed_route_v1(
                &call,
                &solana_fixture().route
            )
        );

        let norito = to_bytes(&call).expect("encode compact Solana call");
        let decoded: SccpVerifiedSolanaDestinationCallV1 =
            norito::decode_from_bytes(&norito).expect("decode compact Solana call");
        assert_eq!(decoded, call);
        assert_eq!(to_bytes(&decoded).expect("re-encode compact call"), norito);
        let json = norito::json::to_json(&call).expect("encode compact call JSON");
        let decoded_json = norito::json::from_str::<SccpVerifiedSolanaDestinationCallV1>(&json)
            .expect("decode compact call JSON");
        assert_eq!(decoded_json, call);
    }

    #[test]
    fn solana_destination_amount_conversion_is_exact_and_overflow_safe() {
        let deployment = match solana_fixture().route.destination {
            SccpDestinationDeploymentV1::Solana(deployment) => deployment,
            _ => unreachable!("Solana fixture must use the Solana deployment"),
        };
        assert_eq!(
            sccp_solana_payload_amount_to_spl_base_units_v1(3, &deployment),
            Some(3)
        );
        assert_eq!(
            sccp_solana_payload_amount_to_spl_base_units_v1(u128::from(u64::MAX) + 1, &deployment,),
            None
        );

        let mut wrong_multiplier = deployment;
        wrong_multiplier.taira_to_token_multiplier = SCCP_V1_TAIRA_TO_TOKEN_MULTIPLIER;
        assert_eq!(
            sccp_solana_payload_amount_to_spl_base_units_v1(3, &wrong_multiplier),
            None
        );

        let mut mismatched_amount = verified_solana_call();
        mismatched_amount.proof_account.amount = 4;
        mismatched_amount.proof_account.header.amount = 4;
        assert!(!sccp_verified_solana_destination_call_is_self_canonical_v1(
            &mismatched_amount
        ));
    }

    #[test]
    fn solana_destination_call_substitutions_fail_closed() {
        let fixture = solana_fixture();
        let call = verified_solana_call();

        let mut changed = call.clone();
        changed.backend = BridgeSccpDestinationProofBackendV1::EvmGroth16Bn254;
        assert!(!sccp_verified_solana_destination_call_is_self_canonical_v1(
            &changed
        ));

        let mut changed = call.clone();
        changed.proof_account.runtime_accounts.payer[0] ^= 1;
        assert!(!sccp_verified_solana_destination_call_is_self_canonical_v1(
            &changed
        ));

        let mut changed = call.clone();
        changed.proof_account.proof_body[141] ^= 1;
        assert!(!sccp_verified_solana_destination_call_is_self_canonical_v1(
            &changed
        ));

        let mut changed = call.clone();
        changed.proof_account.chunks[0].bytes[0] ^= 1;
        assert!(!sccp_verified_solana_destination_call_is_self_canonical_v1(
            &changed
        ));

        let mut changed = call.clone();
        changed.verify_instruction_data[2] ^= 1;
        assert!(!sccp_verified_solana_destination_call_is_self_canonical_v1(
            &changed
        ));

        let mut changed = call.clone();
        changed.proof_account.request_hash[0] ^= 1;
        assert!(sccp_verified_solana_destination_call_is_self_canonical_v1(
            &changed
        ));
        assert!(
            !sccp_verified_solana_destination_call_matches_governed_route_v1(
                &changed,
                &fixture.route,
            ),
            "ungoverned request-hash substitutions must fail historical binding"
        );

        let mut hostile_outer = fixture.bridge_proof.clone();
        hostile_outer.backend = BridgeSccpDestinationProofBackendV1::EvmGroth16Bn254;
        assert!(
            verify_sccp_solana_destination_proof_v1(
                &hostile_outer,
                &fixture.bundle,
                &fixture.route,
                solana_runtime_accounts(),
            )
            .is_none()
        );

        let mut hostile_runtime = solana_runtime_accounts();
        hostile_runtime.payer[0] ^= 1;
        assert!(
            verify_sccp_solana_destination_proof_v1(
                &fixture.bridge_proof,
                &fixture.bundle,
                &fixture.route,
                hostile_runtime,
            )
            .is_none()
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
    fn owned_destination_context_decodes_once_and_pairs_once() {
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
            }
        );

        let verified = verify_parsed_sccp_destination_proof_v1(parsed, &fixture.route)
            .expect("parsed artifact binds to governed route");
        assert_eq!(verified.call().public_inputs, fixture.request.public_inputs);
        assert_eq!(
            verified.finality().finality_artifact.height,
            fixture.request.public_inputs.finality_height
        );
        assert_eq!(
            sccp_destination_proof_work_counters_v1(),
            SccpDestinationProofWorkCountersV1 {
                artifact_framing_decodes: 1,
                bundle_decodes: 1,
                groth16_pairings: 1,
                bls_verifications: 0,
            },
            "Core-bound verification must defer the one authoritative BLS check to local state"
        );
    }

    #[test]
    fn hostile_governed_binding_fails_before_pairing_or_bls() {
        let fixture = fixture();
        let parsed = parse_sccp_destination_proof_v1(&fixture.bridge_proof)
            .expect("canonical destination artifact parses");
        let hostile_route = governed_route(
            SccpNetworkV1::EthereumSepolia,
            fixture.route.revision,
            SccpRouteActivationV1::Staged,
        );
        reset_sccp_destination_proof_work_counters_v1();

        assert!(
            verify_parsed_sccp_destination_proof_v1(parsed, &hostile_route).is_none(),
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
                ) = candidate.semantic_proof_profile;
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
            canonical_sccp_groth16_bn254_verifying_key_bytes_v1(key)
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
            canonical_sccp_groth16_bn254_verifying_key_bytes_v1(key)
                .expect("canonical key")
                .len(),
            38 * 32
        );
        assert_eq!(
            sccp_groth16_bn254_verifying_key_hash_v1(key),
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
            verifier_key_hash: sccp_groth16_bn254_verifying_key_hash_v1(key).unwrap(),
            outbound_proof_policy: outbound_proof_policy(),
            route_address: [0x51; 20],
            route_code_hash: [0x61; 32],
            taira_to_token_multiplier: SCCP_V1_TAIRA_TO_TOKEN_MULTIPLIER,
        };
        let inbound = SccpLaneIdV1 {
            source: SccpNetworkV1::TronNile,
            target: SccpNetworkV1::SoraTaira,
        };
        let outbound = SccpLaneIdV1 {
            source: inbound.target,
            target: inbound.source,
        };
        let route_config = sccp_exact_tron_xor_route_config_hash_v1(
            SccpNetworkV1::TronNile,
            sccp_lane_id_hash_v1(inbound).unwrap(),
            sccp_lane_id_hash_v1(outbound).unwrap(),
            &tron_deployment,
            7,
        )
        .expect("TRON contract route config");
        assert_eq!(
            route_config,
            hex32("d6e06a169ace343b7cd3a3bcd0b1188f7b98ff3abe7def64ca230333babc39c9")
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
            candidate.source_network = SccpNetworkV1::EthereumSepolia;
        });
        assert_request_mutation_rejected(|candidate| {
            candidate.target_network = SccpNetworkV1::EthereumSepolia;
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
                candidate.semantic_proof_profile;
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
        candidate.commitment.context.lane.target = SccpNetworkV1::EthereumSepolia;
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
        finality.finality_artifact.height_context.chain_id =
            "00000000-0000-0000-0000-000000000753".into();
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
            attack.version = BRIDGE_FINALITY_PROOF_VERSION_V1.saturating_add(1);
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
            attack.finality_artifact.height_context.chain_id = "attacker-chain".into();
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
    fn exact_v2_finality_rejects_quorum_and_signer_attacks() {
        let proof = exact_v2_finality_fixture();

        let mut attack = proof.clone();
        attack.finality_artifact.commit_qc.signers = vec![1, 2, 3];
        assert!(
            !verify_taira_bridge_finality_proof_structure(&attack),
            "three of four signers with only 60/100 power must fail the power quorum"
        );

        let mut attack = proof.clone();
        attack.finality_artifact.commit_qc.signers = vec![0, 1];
        assert!(
            !verify_taira_bridge_finality_proof_structure(&attack),
            "70/100 power with only two of four signers must fail the count quorum"
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
