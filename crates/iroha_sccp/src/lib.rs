//! SCCP payload, proof, and counterparty submission helpers for Iroha bridge flows.
//!
//! SCCP V1 supports Ethereum, BSC, and TRON as complete bidirectional route
//! families. Reserved domain numbers for Solana and TON are not decodable V1
//! wire/runtime profiles. SCCP will
//! not support Sub&#115;trate/Pol&#107;adot networks for now; treat that as launch
//! scope, not pending compatibility work.
//!
//! The crate targets the Rust standard library unconditionally.

extern crate alloc;

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
mod native_admission;
pub use native_admission::*;

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
use iroha_crypto::{KeyPair, Signature};
use iroha_data_model::{
    block::BlockHeader,
    bridge::{
        BridgeSccpDestinationProofBackendV1, BridgeSccpDestinationProofV1, SccpBn254G1PointV1,
        SccpBn254G2PointV1, SccpDestinationDeploymentV1, SccpGovernedRouteV1,
        SccpGroth16Bn254VerifyingKeyV1,
    },
    consensus::VALIDATOR_SET_HASH_VERSION_V1,
    peer::PeerId,
};
use norito::to_bytes;
use tiny_keccak::Hasher;

/// SCCP protocol domain assigned to SORA networks.
pub const SCCP_DOMAIN_SORA: u32 = 0;
/// SCCP protocol domain assigned to Ethereum networks.
pub const SCCP_DOMAIN_ETH: u32 = 1;
/// SCCP protocol domain assigned to BNB Smart Chain networks.
pub const SCCP_DOMAIN_BSC: u32 = 2;
/// SCCP protocol domain assigned to TRON networks.
pub const SCCP_DOMAIN_TRON: u32 = 5;
/// Public Sora Nexus chain id bound into SORA-origin SCCP finality proofs.
pub const SCCP_NEXUS_FINALITY_CHAIN_ID_V1: &str = "00000000-0000-0000-0000-000000000753";
/// Public TAIRA chain id bound into TAIRA-origin SCCP finality proofs.
pub const SCCP_TAIRA_FINALITY_CHAIN_ID_V1: &str = "809574f5-fee7-5e69-bfcf-52451e42d50f";
/// TAIRA testnet SCCP route id used for the initial XOR bridge to TRON Nile.
pub const SCCP_TAIRA_TRON_XOR_ROUTE_ID_V1: &str = "taira_tron_xor";
/// TAIRA SCCP route id used for the exact XOR bridge to Ethereum.
pub const SCCP_TAIRA_ETH_XOR_ROUTE_ID_V1: &str = "taira_eth_xor";
/// TAIRA testnet SCCP route id used for the XOR bridge to BSC.
pub const SCCP_TAIRA_BSC_XOR_ROUTE_ID_V1: &str = "taira_bsc_xor";
/// TAIRA SCCP asset key for XOR in every exact EVM-family and TRON route.
pub const SCCP_TAIRA_XOR_ASSET_KEY_V1: &str = "xor";
/// Exact Solidity/TVM value-moving route entrypoint for Taira finalization.
pub const SCCP_FINALIZE_FROM_TAIRA_ABI_V1: &str =
    "finalizeFromTaira(bytes,bytes32[6],bytes32,bytes)";
/// Keccak-256 selector for [`SCCP_FINALIZE_FROM_TAIRA_ABI_V1`].
pub const SCCP_FINALIZE_FROM_TAIRA_SELECTOR_V1: [u8; 4] = [0x95, 0xd7, 0x57, 0xc4];
/// Canonical printable-ASCII text used by SORA accounts and route-local identifiers.
pub const SCCP_CODEC_CANONICAL_TEXT: u8 = 1;
/// Raw nonzero 20-byte EVM account address.
pub const SCCP_CODEC_EVM_ADDRESS20: u8 = 2;
/// Raw nonzero TRON account including its mandatory `0x41` network prefix.
pub const SCCP_CODEC_TRON_ADDRESS21: u8 = 5;
/// Maximum byte length of one canonical textual SCCP wire value.
pub const SCCP_MAX_CANONICAL_TEXT_BYTES_V1: usize = 256;

/// Closed list of external protocol domains implemented by SCCP V1.
pub const SCCP_CORE_REMOTE_DOMAINS: [u32; 3] = [SCCP_DOMAIN_ETH, SCCP_DOMAIN_BSC, SCCP_DOMAIN_TRON];

/// Remote SCCP domains in the current supported production launch scope.
pub const SCCP_SUPPORTED_LAUNCH_REMOTE_DOMAINS_V1: [u32; 3] =
    [SCCP_DOMAIN_ETH, SCCP_DOMAIN_BSC, SCCP_DOMAIN_TRON];

/// External protocol domains that can safely originate native SCCP messages in V1.
///
pub const SCCP_NATIVE_INBOUND_REMOTE_DOMAINS_V1: [u32; 3] =
    [SCCP_DOMAIN_ETH, SCCP_DOMAIN_BSC, SCCP_DOMAIN_TRON];

/// External domains with a checked-in value-moving outbound route implementation.
///
pub const SCCP_VALUE_MOVING_OUTBOUND_REMOTE_DOMAINS_V1: [u32; 3] =
    [SCCP_DOMAIN_ETH, SCCP_DOMAIN_BSC, SCCP_DOMAIN_TRON];

/// Domain separator for the single V1 SCCP message-identity construction.
///
/// The preimage binds the exact directed network lane and canonical payload.
/// Governed destination deployment bindings are deliberately excluded so a
/// binding rotation cannot make the same economic message replayable.
pub const SCCP_LANE_MESSAGE_ID_PREFIX_V1: &[u8] = b"sccp:lane-message-id:v1";
/// Iroha consensus transcript version authenticated by SCCP finality proofs.
pub const IROHA_CONSENSUS_PROTO_VERSION_V1: u32 = 1;

const SCCP_HUB_LEAF_PREFIX_V1: &[u8] = b"sccp:hub:leaf:v1";
const SCCP_HUB_NODE_PREFIX_V1: &[u8] = b"sccp:hub:node:v1";
const SCCP_PAYLOAD_HASH_PREFIX_V1: &[u8] = b"sccp:payload:v1";
const SCCP_GROTH16_STATEMENT_PREFIX_V1: &[u8] = b"sccp:groth16-bn254:statement:v1";
const SCCP_GROTH16_PROOF_REQUEST_PREFIX_V1: &[u8] = b"sccp:groth16-bn254:proof-request:v1";
const SCCP_GROTH16_PROOF_RESULT_PREFIX_V1: &[u8] = b"sccp:groth16-bn254:proof-result:v1";
/// Maximum canonical Norito size of a Nexus SCCP proof artifact.
pub const SCCP_NEXUS_MAX_ENCODED_PROOF_BYTES_V1: usize = 16 * 1024 * 1024;
/// Maximum canonical Norito size of a Groth16 request, result, or complete artifact.
///
/// A request may contain one maximum-sized Nexus bundle. The fixed allowance
/// covers the closed 36-word verification key, proof, hashes, and Norito
/// framing without making the admission bound depend on decoded lengths.
pub const SCCP_GROTH16_BN254_MAX_ENCODED_ARTIFACT_BYTES_V1: usize =
    SCCP_NEXUS_MAX_ENCODED_PROOF_BYTES_V1 + 64 * 1024;
/// Maximum padded-base64 size accepted by an HTTP adapter for one Groth16 artifact.
pub const SCCP_GROTH16_BN254_MAX_BASE64_ARTIFACT_BYTES_V1: usize =
    4 * SCCP_GROTH16_BN254_MAX_ENCODED_ARTIFACT_BYTES_V1.div_ceil(3);
/// Maximum canonical JSON size accepted for a Groth16 request, result, or artifact.
pub const SCCP_GROTH16_BN254_MAX_JSON_ARTIFACT_BYTES_V1: usize =
    2 * SCCP_GROTH16_BN254_MAX_ENCODED_ARTIFACT_BYTES_V1 + 256 * 1024;
/// Maximum number of sibling nodes in a Nexus SCCP commitment proof.
pub const SCCP_NEXUS_MAX_MERKLE_PROOF_STEPS_V1: usize = 64;
/// Maximum validator roster accepted in a Nexus SCCP commit QC.
pub const SCCP_NEXUS_MAX_FINALITY_VALIDATORS_V1: usize = 4_096;
const SCCP_NEXUS_MAX_BLOCK_HEADER_BYTES_V1: usize = 256 * 1024;
const SCCP_NEXUS_MAX_CONSENSUS_MODE_TAG_BYTES_V1: usize = 128;
const SCCP_NEXUS_MAX_PUBLIC_KEY_TEXT_BYTES_V1: usize = 256;
const SCCP_NEXUS_MAX_BLS_PROOF_BYTES_V1: usize = 256;
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

#[cfg(test)]
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
/// Bounded Merkle path from one SCCP commitment to its block commitment root.
pub struct SccpMerkleProofV1 {
    /// Bottom-up sibling steps.
    pub steps: Vec<SccpMerkleStepV1>,
}

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
/// Consensus phase encoded in a Nexus finality certificate.
pub enum NexusConsensusPhaseV1 {
    /// Prepare/prevote phase.
    Prepare = 1,
    /// Commit/precommit phase.
    Commit = 2,
    /// View-change certificate phase.
    NewView = 3,
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
/// Compact reference to the highest quorum certificate carried by a commit QC.
pub struct NexusQcRefV1 {
    /// Certified block height.
    #[norito(with = "json_utils::u64_string")]
    pub height: u64,
    /// Certified consensus view.
    #[norito(with = "json_utils::u64_string")]
    pub view: u64,
    /// Certified consensus epoch.
    #[norito(with = "json_utils::u64_string")]
    pub epoch: u64,
    /// Hash of the certified block.
    #[norito(with = "json_utils::hex32")]
    pub subject_block_hash: H256,
    /// Phase in which the certificate was formed.
    pub phase: NexusConsensusPhaseV1,
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
/// Nexus commit quorum certificate used by SCCP finality verification.
pub struct NexusCommitQcV1 {
    /// Certificate schema version. SCCP V1 requires `1`.
    pub version: u8,
    /// Consensus phase; production finality requires [`NexusConsensusPhaseV1::Commit`].
    pub phase: NexusConsensusPhaseV1,
    /// Certified block height.
    #[norito(with = "json_utils::u64_string")]
    pub height: u64,
    /// Certified consensus view.
    #[norito(with = "json_utils::u64_string")]
    pub view: u64,
    /// Certified consensus epoch.
    #[norito(with = "json_utils::u64_string")]
    pub epoch: u64,
    /// Canonical Sumeragi consensus-mode tag.
    pub mode_tag: String,
    /// Hash of the certified block header.
    #[norito(with = "json_utils::hex32")]
    pub subject_block_hash: H256,
    /// State root inherited from the certified block's parent.
    #[norito(with = "json_utils::hex32")]
    pub parent_state_root: H256,
    /// State root produced by the certified block.
    #[norito(with = "json_utils::hex32")]
    pub post_state_root: H256,
    /// Canonical chain-order commitment.
    #[norito(with = "json_utils::hex32")]
    pub chain_order_hash: H256,
    /// Rechain sequence authenticated by the certificate.
    #[norito(with = "json_utils::u64_string")]
    pub rechain_seq: u64,
    /// Optional highest-QC reference included in the signed vote transcript.
    pub highest_qc: Option<NexusQcRefV1>,
    /// Canonical hash of the validator roster.
    #[norito(with = "json_utils::hex32")]
    pub validator_set_hash: H256,
    /// Version of the validator-roster hash construction.
    pub validator_set_hash_version: u16,
    /// Validator BLS public keys in canonical roster order.
    pub validator_public_keys: Vec<String>,
    /// Proofs of possession corresponding to [`Self::validator_public_keys`].
    #[norito(with = "json_utils::vec_bytes_hex")]
    pub validator_set_pops: Vec<Vec<u8>>,
    /// Little-endian signer bitmap over the canonical validator roster.
    #[norito(with = "json_utils::bytes_hex")]
    pub signers_bitmap: Vec<u8>,
    /// Aggregate BLS signature over the canonical commit-vote transcript.
    #[norito(with = "json_utils::bytes_hex")]
    pub bls_aggregate_signature: Vec<u8>,
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
/// Self-contained Nexus block-finality proof for an SCCP commitment root.
pub struct NexusBridgeFinalityProofV1 {
    /// Proof schema version. SCCP V1 requires `1`.
    pub version: u8,
    /// Exact chain identifier included in the consensus transcript.
    pub chain_id: String,
    /// Finalized block height.
    #[norito(with = "json_utils::u64_string")]
    pub height: u64,
    /// Hash of [`Self::block_header_bytes`].
    #[norito(with = "json_utils::hex32")]
    pub block_hash: H256,
    /// SCCP commitment root carried by the finalized block header.
    #[norito(with = "json_utils::hex32")]
    pub commitment_root: H256,
    /// Canonical encoded Iroha block header.
    #[norito(with = "json_utils::bytes_hex")]
    pub block_header_bytes: Vec<u8>,
    /// Commit QC that finalizes the block.
    pub commit_qc: NexusCommitQcV1,
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
/// Canonical SORA-origin SCCP message, Merkle inclusion, and Nexus finality bundle.
pub struct NexusSccpMessageProofV1 {
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
    /// Canonical encoded [`NexusBridgeFinalityProofV1`].
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
/// Public statement exposed to a destination-chain SCCP verifier.
pub struct SccpMessageTransparentPublicInputsV1 {
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
    /// SCCP commitment root authenticated by Nexus finality.
    #[norito(with = "json_utils::hex32")]
    pub commitment_root: H256,
    /// Finalized Nexus block height.
    #[norito(with = "json_utils::u64_string")]
    pub finality_height: u64,
    /// Finalized Nexus block hash.
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
#[norito(tag = "family", content = "target", rename_all = "snake_case")]
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
    /// Exact governed route contract that must receive [`Self::calldata`].
    pub target: SccpDestinationCallTargetV1,
    /// Public statement authenticated by the Groth16 proof.
    pub public_inputs: SccpMessageTransparentPublicInputsV1,
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
    pub bundle: NexusSccpMessageProofV1,
}

#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize,
)]
/// Decoded value of one closed SCCP V1 wire codec.
pub enum SccpNormalizedCodecValueV1 {
    /// Printable-ASCII canonical text.
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
/// Normalized transfer payload consumed by proof backends.
pub struct SccpTransferProjectionV1 {
    /// Payload schema version.
    pub version: u8,
    /// Transfer source protocol domain.
    pub source_domain: u32,
    /// Transfer destination protocol domain.
    pub dest_domain: u32,
    /// Sender-chosen replay-separating nonce.
    pub nonce: u64,
    /// Nonzero immutable governed-route revision.
    pub route_revision: u32,
    /// Asset home protocol domain.
    pub asset_home_domain: u32,
    /// Decoded canonical asset identifier.
    pub asset_id: SccpNormalizedCodecValueV1,
    /// Positive amount in the route's smallest unit.
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

impl_str_json_enum!(NexusConsensusPhaseV1, "unsupported Nexus consensus phase", {
    NexusConsensusPhaseV1::Prepare => "Prepare",
    NexusConsensusPhaseV1::Commit => "Commit",
    NexusConsensusPhaseV1::NewView => "NewView",
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
            "CanonicalText" => Ok(Self::CanonicalText {
                value: <String as norito::json::JsonDeserialize>::json_from_value(
                    json_required_field("SccpNormalizedCodecValueV1", payload, "value")?,
                )?,
            }),
            "EvmAddress20" => Ok(Self::EvmAddress20 {
                bytes: json_fixed_hex_field::<20>("SccpNormalizedCodecValueV1", payload, "bytes")?,
            }),
            "TronAddress21" => Ok(Self::TronAddress21 {
                bytes: json_fixed_hex_field::<21>("SccpNormalizedCodecValueV1", payload, "bytes")?,
            }),
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
    /// Structured transparent public inputs.
    pub public_inputs: SccpMessageTransparentPublicInputsV1,
    /// Exact audited verification key pinned by the governed deployment.
    pub verifying_key: SccpGroth16Bn254VerifyingKeyV1,
    /// Solidity-compatible hash of [`Self::verifying_key`].
    #[norito(with = "json_utils::hex32")]
    pub verifier_key_hash: H256,
    /// Canonical encoded SORA message and finality bundle.
    #[norito(with = "json_utils::bytes_hex")]
    pub bundle_bytes: Vec<u8>,
    /// Canonical transparent statement hash.
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
        SCCP_DOMAIN_SORA | SCCP_DOMAIN_ETH | SCCP_DOMAIN_BSC | SCCP_DOMAIN_TRON
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
        SCCP_DOMAIN_ETH | SCCP_DOMAIN_BSC | SCCP_DOMAIN_TRON
    )
}

/// Return whether a remote domain has a checked-in value-moving outbound route in V1.
pub const fn sccp_domain_has_value_moving_outbound_route_v1(domain_id: u32) -> bool {
    matches!(
        domain_id,
        SCCP_DOMAIN_ETH | SCCP_DOMAIN_BSC | SCCP_DOMAIN_TRON
    )
}

/// Return whether `codec_id` is one of the closed SCCP V1 wire codecs.
pub fn is_supported_codec(codec_id: u8) -> bool {
    matches!(
        codec_id,
        SCCP_CODEC_CANONICAL_TEXT | SCCP_CODEC_EVM_ADDRESS20 | SCCP_CODEC_TRON_ADDRESS21
    )
}

/// Return the stable machine-readable name of one SCCP wire codec.
pub fn sccp_codec_key(codec_id: u8) -> Option<&'static str> {
    match codec_id {
        SCCP_CODEC_CANONICAL_TEXT => Some("canonical_text"),
        SCCP_CODEC_EVM_ADDRESS20 => Some("evm_address20"),
        SCCP_CODEC_TRON_ADDRESS21 => Some("tron_address21"),
        _ => None,
    }
}

/// Return a concise description of one SCCP wire codec.
pub fn sccp_codec_description(codec_id: u8) -> Option<&'static str> {
    match codec_id {
        SCCP_CODEC_CANONICAL_TEXT => Some(
            "Non-empty printable ASCII bytes for canonical SORA accounts and route-local names.",
        ),
        SCCP_CODEC_EVM_ADDRESS20 => Some("Raw nonzero 20-byte EVM account addresses."),
        SCCP_CODEC_TRON_ADDRESS21 => {
            Some("Raw nonzero TRON account addresses including the 0x41 prefix.")
        }
        _ => None,
    }
}

/// Return the stable chain-family key for one SCCP protocol domain.
pub fn sccp_chain_key_for_domain(domain: u32) -> Option<&'static str> {
    match domain {
        SCCP_DOMAIN_SORA => Some("sora"),
        SCCP_DOMAIN_ETH => Some("eth"),
        SCCP_DOMAIN_BSC => Some("bsc"),
        SCCP_DOMAIN_TRON => Some("tron"),
        _ => None,
    }
}

/// Return the account-identifier codec required by one external domain.
pub fn sccp_counterparty_account_codec(domain: u32) -> Option<u8> {
    match domain {
        SCCP_DOMAIN_SORA => Some(SCCP_CODEC_CANONICAL_TEXT),
        SCCP_DOMAIN_ETH | SCCP_DOMAIN_BSC => Some(SCCP_CODEC_EVM_ADDRESS20),
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
/// Encode transparent public inputs in their fixed, platform-independent V1 layout.
pub fn canonical_sccp_message_transparent_public_inputs_bytes(
    public_inputs: &SccpMessageTransparentPublicInputsV1,
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

/// Encode an SCCP Merkle path, returning an empty vector when it exceeds V1 bounds.
pub fn canonical_sccp_merkle_proof_bytes(proof: &SccpMerkleProofV1) -> Vec<u8> {
    canonical_sccp_merkle_proof_bytes_checked(proof).unwrap_or_default()
}

/// Encode a bounded SCCP Merkle path in its canonical V1 byte layout.
pub fn canonical_sccp_merkle_proof_bytes_checked(proof: &SccpMerkleProofV1) -> Option<Vec<u8>> {
    if proof.steps.len() > SCCP_NEXUS_MAX_MERKLE_PROOF_STEPS_V1 {
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

/// Encode a Nexus SCCP bundle, returning an empty vector when its lengths overflow V1 bounds.
pub fn canonical_nexus_sccp_message_bundle_bytes(bundle: &NexusSccpMessageProofV1) -> Vec<u8> {
    canonical_nexus_sccp_message_bundle_bytes_checked(bundle).unwrap_or_default()
}

fn canonical_nexus_sccp_message_bundle_bytes_len_checked(
    bundle: &NexusSccpMessageProofV1,
) -> Option<Vec<u8>> {
    let commitment = canonical_commitment_bytes(&bundle.commitment);
    let merkle_proof = canonical_sccp_merkle_proof_bytes_checked(&bundle.merkle_proof)?;
    let payload = canonical_sccp_payload_bytes(&bundle.payload);

    let mut out = Vec::new();
    push_u8(&mut out, bundle.version);
    out.extend_from_slice(&bundle.commitment_root);
    push_vec_checked(&mut out, &commitment)?;
    push_vec_checked(&mut out, &merkle_proof)?;
    push_vec_checked(&mut out, &payload)?;
    push_vec_checked(&mut out, &bundle.finality_proof)?;
    Some(out)
}

/// Validate and encode one canonical, length-bounded Nexus SCCP message bundle.
pub fn canonical_nexus_sccp_message_bundle_bytes_checked(
    bundle: &NexusSccpMessageProofV1,
) -> Option<Vec<u8>> {
    if !verify_message_bundle_structure(bundle) {
        return None;
    }
    canonical_nexus_sccp_message_bundle_bytes_len_checked(bundle)
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

/// Derive the ten BN254 field public signals consumed by SCCP Groth16 verifiers.
///
/// The output order matches `SccpGroth16Bn254MessageVerifier`: message id,
/// payload hash, target-domain word, commitment root, finality-height word,
/// finality block hash, source-domain word, statement hash, and destination
/// binding hash, and immutable route-configuration hash. Each word is
/// `keccak256(abi.encode(keccak256(label), value)) mod Fr` encoded as a
/// big-endian 32-byte BN254 scalar.
pub fn sccp_groth16_bn254_public_signal_words(
    public_inputs: &SccpMessageTransparentPublicInputsV1,
    source_domain: u32,
    statement_hash: H256,
    destination_binding_hash: H256,
    route_configuration_hash: H256,
) -> [H256; 10] {
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

fn sccp_evm_public_input_words(public_inputs: &SccpMessageTransparentPublicInputsV1) -> [H256; 6] {
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
    public_inputs: &SccpMessageTransparentPublicInputsV1,
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
/// The result is the concatenation of 36 ABI words: alpha G1, beta/gamma/delta
/// G2 in contract limb order, then the eleven IC G1 points.
pub fn canonical_sccp_groth16_bn254_verifying_key_bytes_v1(
    verifying_key: &SccpGroth16Bn254VerifyingKeyV1,
) -> Option<Vec<u8>> {
    if !sccp_groth16_bn254_verifying_key_is_well_formed_v1(verifying_key) {
        return None;
    }
    let mut out = Vec::with_capacity(36 * 32);
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
    public_signals: &[H256; 10],
    verifying_key: &SccpGroth16Bn254VerifyingKeyV1,
) -> bool {
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

/// Verify an SCCP Groth16 proof against an exact governed BN254 verification key.
///
/// This performs the same ten signal hashes and four-term pairing equation as
/// `SccpGroth16Bn254MessageVerifier.sol`. The expected key hash must come from
/// typed governed deployment state, never from proof-controlled metadata.
pub fn verify_sccp_groth16_bn254_proof_v1(
    public_inputs: &SccpMessageTransparentPublicInputsV1,
    source_domain: u32,
    statement_hash: H256,
    destination_binding_hash: H256,
    route_configuration_hash: H256,
    proof_bytes: &[u8],
    verifying_key: &SccpGroth16Bn254VerifyingKeyV1,
    expected_verifier_key_hash: H256,
) -> bool {
    if public_inputs.version != 1
        || source_domain != SCCP_DOMAIN_SORA
        || public_inputs.target_domain == source_domain
        || !h256_is_nonzero(&public_inputs.message_id)
        || !h256_is_nonzero(&public_inputs.payload_hash)
        || !h256_is_nonzero(&public_inputs.commitment_root)
        || public_inputs.finality_height == 0
        || !h256_is_nonzero(&public_inputs.finality_block_hash)
        || !h256_is_nonzero(&statement_hash)
        || !h256_is_nonzero(&destination_binding_hash)
        || !h256_is_nonzero(&route_configuration_hash)
        || destination_binding_hash == route_configuration_hash
        || !h256_is_nonzero(&expected_verifier_key_hash)
        || sccp_groth16_bn254_verifying_key_hash_v1(verifying_key)
            != Some(expected_verifier_key_hash)
    {
        return false;
    }
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
    let public_signals = sccp_groth16_bn254_public_signal_words(
        public_inputs,
        source_domain,
        statement_hash,
        destination_binding_hash,
        route_configuration_hash,
    );
    verify_sccp_groth16_bn254_pairing_equation_v1(&proof, &public_signals, verifying_key)
}

fn decode_canonical_sccp_merkle_proof_bytes(proof_bytes: &[u8]) -> Option<SccpMerkleProofV1> {
    let mut cursor = PayloadCursor::new(proof_bytes);
    let step_count = usize::try_from(cursor.take_u32()?).ok()?;
    if step_count > SCCP_NEXUS_MAX_MERKLE_PROOF_STEPS_V1
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

fn decode_canonical_nexus_sccp_message_bundle_summary(
    bundle_bytes: &[u8],
) -> Option<SccpCanonicalMessageBundleSummaryV1> {
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
        || canonical_sccp_payload_bytes(&payload) != payload_bytes
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

    Some(SccpCanonicalMessageBundleSummaryV1 {
        source_network: commitment.context.lane.source,
        target_network: commitment.context.lane.target,
        destination_binding_hash: commitment.context.destination_binding_hash,
        route_configuration_hash: commitment.context.route_configuration_hash,
        canonical_payload_bytes: payload_bytes,
        message_id: commitment.message_id,
        payload_hash: commitment.payload_hash,
        commitment_root,
        finality_proof,
    })
}

fn sccp_proof_request_bundle_bytes_match_public_inputs(
    public_inputs: &SccpMessageTransparentPublicInputsV1,
    bundle_bytes: &[u8],
    source_network: SccpNetworkV1,
    target_network: SccpNetworkV1,
    destination_binding_hash: H256,
    route_configuration_hash: H256,
) -> bool {
    let Some(bundle) = decode_canonical_nexus_sccp_message_bundle_summary(bundle_bytes) else {
        return false;
    };
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
    let Some(finality) = decode_nexus_bridge_finality_proof(&bundle.finality_proof) else {
        return false;
    };
    verify_nexus_bridge_finality_proof_structure(&finality)
        && finality.commitment_root == bundle.commitment_root
        && finality.height == public_inputs.finality_height
        && finality.block_hash == public_inputs.finality_block_hash
}
fn sccp_destination_proof_backend_tag_v1(backend: BridgeSccpDestinationProofBackendV1) -> u8 {
    match backend {
        BridgeSccpDestinationProofBackendV1::EvmGroth16Bn254 => 0,
        BridgeSccpDestinationProofBackendV1::TronGroth16Bn254 => 1,
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
    }
}

#[allow(clippy::too_many_arguments)]
fn sccp_groth16_bn254_statement_hash_v1(
    backend: BridgeSccpDestinationProofBackendV1,
    source_network: SccpNetworkV1,
    target_network: SccpNetworkV1,
    public_inputs: &SccpMessageTransparentPublicInputsV1,
    canonical_payload_bytes: &[u8],
    bundle_bytes: &[u8],
    destination_binding_hash: H256,
    route_configuration_hash: H256,
    verifier_key_hash: H256,
) -> Option<H256> {
    if source_network != SccpNetworkV1::SoraTaira
        || !sccp_destination_proof_backend_supports_network_v1(backend, target_network)
        || target_network.domain_id() != public_inputs.target_domain
        || canonical_payload_bytes.is_empty()
        || bundle_bytes.is_empty()
        || [
            destination_binding_hash,
            route_configuration_hash,
            verifier_key_hash,
        ]
        .iter()
        .any(|value| !h256_is_nonzero(value))
        || destination_binding_hash == route_configuration_hash
        || destination_binding_hash == verifier_key_hash
        || route_configuration_hash == verifier_key_hash
    {
        return None;
    }
    let mut statement =
        Vec::with_capacity(canonical_payload_bytes.len() + bundle_bytes.len() + 512);
    push_u8(&mut statement, 1);
    push_u8(
        &mut statement,
        sccp_destination_proof_backend_tag_v1(backend),
    );
    push_vec_checked(
        &mut statement,
        &canonical_sccp_network_bytes_v1(source_network),
    )?;
    push_vec_checked(
        &mut statement,
        &canonical_sccp_network_bytes_v1(target_network),
    )?;
    statement.extend_from_slice(&destination_binding_hash);
    statement.extend_from_slice(&route_configuration_hash);
    statement.extend_from_slice(&verifier_key_hash);
    statement.extend_from_slice(&canonical_sccp_message_transparent_public_inputs_bytes(
        public_inputs,
    ));
    push_vec_checked(&mut statement, canonical_payload_bytes)?;
    push_vec_checked(&mut statement, bundle_bytes)?;
    Some(prefixed_blake2b(
        SCCP_GROTH16_STATEMENT_PREFIX_V1,
        &statement,
    ))
}

#[allow(clippy::too_many_arguments)]
fn sccp_groth16_bn254_proof_request_hash(
    backend: BridgeSccpDestinationProofBackendV1,
    source_network: SccpNetworkV1,
    target_network: SccpNetworkV1,
    public_inputs_bytes: &[u8],
    canonical_payload_bytes: &[u8],
    bundle_bytes: &[u8],
    statement_hash: H256,
    destination_binding_hash: H256,
    route_configuration_hash: H256,
    verifier_key_hash: H256,
    verifying_key_bytes: &[u8],
    public_signal_words: &[H256; 10],
) -> H256 {
    let mut preimage = Vec::with_capacity(
        public_inputs_bytes.len()
            + canonical_payload_bytes.len()
            + bundle_bytes.len()
            + verifying_key_bytes.len()
            + 512,
    );
    push_u8(&mut preimage, 1);
    push_u8(
        &mut preimage,
        sccp_destination_proof_backend_tag_v1(backend),
    );
    push_vec(
        &mut preimage,
        &canonical_sccp_network_bytes_v1(source_network),
    );
    push_vec(
        &mut preimage,
        &canonical_sccp_network_bytes_v1(target_network),
    );
    push_vec(&mut preimage, public_inputs_bytes);
    push_vec(&mut preimage, canonical_payload_bytes);
    push_vec(&mut preimage, bundle_bytes);
    preimage.extend_from_slice(&statement_hash);
    preimage.extend_from_slice(&destination_binding_hash);
    preimage.extend_from_slice(&route_configuration_hash);
    preimage.extend_from_slice(&verifier_key_hash);
    push_vec(&mut preimage, verifying_key_bytes);
    for word in public_signal_words {
        preimage.extend_from_slice(word);
    }
    prefixed_blake2b(SCCP_GROTH16_PROOF_REQUEST_PREFIX_V1, &preimage)
}

#[allow(clippy::too_many_arguments)]
fn build_sccp_groth16_bn254_proof_request(
    backend: BridgeSccpDestinationProofBackendV1,
    source_network: SccpNetworkV1,
    target_network: SccpNetworkV1,
    public_inputs: &SccpMessageTransparentPublicInputsV1,
    canonical_payload_bytes: &[u8],
    bundle_bytes: &[u8],
    destination_binding_hash: H256,
    route_configuration_hash: H256,
    verifying_key: &SccpGroth16Bn254VerifyingKeyV1,
    expected_verifier_key_hash: H256,
) -> Option<SccpGroth16Bn254ProofRequestV1> {
    if source_network != SccpNetworkV1::SoraTaira
        || !sccp_destination_proof_backend_supports_network_v1(backend, target_network)
        || public_inputs.target_domain != target_network.domain_id()
        || !sccp_groth16_proof_request_public_inputs_are_valid(
            source_network,
            target_network,
            public_inputs,
        )
        || !sccp_proof_request_bundle_bytes_match_public_inputs(
            public_inputs,
            bundle_bytes,
            source_network,
            target_network,
            destination_binding_hash,
            route_configuration_hash,
        )
        || payload_hash(canonical_payload_bytes) != public_inputs.payload_hash
        || !h256_is_nonzero(&expected_verifier_key_hash)
        || sccp_groth16_bn254_verifying_key_hash_v1(verifying_key)
            != Some(expected_verifier_key_hash)
    {
        return None;
    }
    let statement_hash = sccp_groth16_bn254_statement_hash_v1(
        backend,
        source_network,
        target_network,
        public_inputs,
        canonical_payload_bytes,
        bundle_bytes,
        destination_binding_hash,
        route_configuration_hash,
        expected_verifier_key_hash,
    )?;
    let public_inputs_bytes = canonical_sccp_message_transparent_public_inputs_bytes(public_inputs);
    let public_signal_words = sccp_groth16_bn254_public_signal_words(
        public_inputs,
        source_network.domain_id(),
        statement_hash,
        destination_binding_hash,
        route_configuration_hash,
    );
    let verifying_key_bytes = canonical_sccp_groth16_bn254_verifying_key_bytes_v1(verifying_key)?;
    let request_hash = sccp_groth16_bn254_proof_request_hash(
        backend,
        source_network,
        target_network,
        &public_inputs_bytes,
        canonical_payload_bytes,
        bundle_bytes,
        statement_hash,
        destination_binding_hash,
        route_configuration_hash,
        expected_verifier_key_hash,
        &verifying_key_bytes,
        &public_signal_words,
    );
    Some(SccpGroth16Bn254ProofRequestV1 {
        version: 1,
        backend,
        source_network,
        target_network,
        public_inputs: public_inputs.clone(),
        verifying_key: *verifying_key,
        verifier_key_hash: expected_verifier_key_hash,
        bundle_bytes: bundle_bytes.to_vec(),
        statement_hash,
        destination_binding_hash,
        route_configuration_hash,
        request_hash,
    })
}

fn sccp_governed_route_groth16_material_v1(
    governed_route: &SccpGovernedRouteV1,
) -> Option<(SccpGroth16Bn254VerifyingKeyV1, H256)> {
    let (verifying_key, verifier_key_hash) = match governed_route.destination {
        SccpDestinationDeploymentV1::Evm(deployment) => {
            (deployment.verifying_key, deployment.verifier_key_hash)
        }
        SccpDestinationDeploymentV1::Tron(deployment) => {
            (deployment.verifying_key, deployment.verifier_key_hash)
        }
    };
    (sccp_groth16_bn254_verifying_key_hash_v1(&verifying_key) == Some(verifier_key_hash))
        .then_some((verifying_key, verifier_key_hash))
}

/// Return whether a governed route carries a canonical, subgroup-checked
/// Groth16 key whose Solidity hash equals the deployment commitment.
pub fn sccp_governed_route_groth16_key_is_valid_v1(governed_route: &SccpGovernedRouteV1) -> bool {
    sccp_governed_route_groth16_material_v1(governed_route).is_some()
}

fn sccp_governed_groth16_route_matches_bundle_v1(
    bundle: &NexusSccpMessageProofV1,
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
/// function is called.
pub fn build_sccp_groth16_bn254_proof_request_from_governed_route_v1(
    bundle: &NexusSccpMessageProofV1,
    governed_route: &SccpGovernedRouteV1,
) -> Option<SccpGroth16Bn254ProofRequestV1> {
    if !verify_message_bundle_structure(bundle)
        || verified_sccp_message_nexus_finality_proof_for_production(bundle).is_none()
        || !sccp_governed_groth16_route_matches_bundle_v1(bundle, governed_route)
    {
        return None;
    }
    let public_inputs = sccp_message_transparent_public_inputs(bundle)?;
    let canonical_payload_bytes = canonical_sccp_payload_bytes(&bundle.payload);
    let bundle_bytes = canonical_nexus_sccp_message_bundle_bytes_checked(bundle)?;
    let destination_binding_hash = governed_route.destination_binding_hash().ok()?;
    let route_configuration_hash = governed_route.route_configuration_hash().ok()?;
    let (verifying_key, expected_verifier_key_hash) =
        sccp_governed_route_groth16_material_v1(governed_route)?;
    let backend = match governed_route.destination {
        SccpDestinationDeploymentV1::Evm(_) => BridgeSccpDestinationProofBackendV1::EvmGroth16Bn254,
        SccpDestinationDeploymentV1::Tron(_) => {
            BridgeSccpDestinationProofBackendV1::TronGroth16Bn254
        }
    };
    build_sccp_groth16_bn254_proof_request(
        backend,
        governed_route.lane_id.target,
        governed_route.lane_id.source,
        &public_inputs,
        &canonical_payload_bytes,
        &bundle_bytes,
        destination_binding_hash,
        route_configuration_hash,
        &verifying_key,
        expected_verifier_key_hash,
    )
}

/// Return whether a request is exactly the canonical request for a bundle and governed route.
pub fn sccp_groth16_bn254_proof_request_matches_governed_route_v1(
    request: &SccpGroth16Bn254ProofRequestV1,
    bundle: &NexusSccpMessageProofV1,
    governed_route: &SccpGovernedRouteV1,
) -> bool {
    build_sccp_groth16_bn254_proof_request_from_governed_route_v1(bundle, governed_route).as_ref()
        == Some(request)
}

fn sccp_groth16_bn254_proof_request_is_canonical(
    request: &SccpGroth16Bn254ProofRequestV1,
    expected_backend: BridgeSccpDestinationProofBackendV1,
) -> bool {
    if request.version != 1
        || request.backend != expected_backend
        || !sccp_destination_proof_backend_supports_network_v1(
            request.backend,
            request.target_network,
        )
        || !sccp_groth16_proof_request_public_inputs_are_valid(
            request.source_network,
            request.target_network,
            &request.public_inputs,
        )
        || !h256_is_nonzero(&request.statement_hash)
        || !h256_is_nonzero(&request.verifier_key_hash)
        || !h256_is_nonzero(&request.destination_binding_hash)
        || !h256_is_nonzero(&request.route_configuration_hash)
        || request.destination_binding_hash == request.route_configuration_hash
        || request.destination_binding_hash == request.verifier_key_hash
        || request.route_configuration_hash == request.verifier_key_hash
        || sccp_groth16_bn254_verifying_key_hash_v1(&request.verifying_key)
            != Some(request.verifier_key_hash)
    {
        return false;
    }
    let Some(bundle) = decode_canonical_nexus_sccp_message_bundle_summary(&request.bundle_bytes)
    else {
        return false;
    };
    if !sccp_proof_request_bundle_bytes_match_public_inputs(
        &request.public_inputs,
        &request.bundle_bytes,
        request.source_network,
        request.target_network,
        request.destination_binding_hash,
        request.route_configuration_hash,
    ) {
        return false;
    }
    let Some(statement_hash) = sccp_groth16_bn254_statement_hash_v1(
        request.backend,
        request.source_network,
        request.target_network,
        &request.public_inputs,
        &bundle.canonical_payload_bytes,
        &request.bundle_bytes,
        request.destination_binding_hash,
        request.route_configuration_hash,
        request.verifier_key_hash,
    ) else {
        return false;
    };
    if request.statement_hash != statement_hash {
        return false;
    }
    let public_inputs_bytes =
        canonical_sccp_message_transparent_public_inputs_bytes(&request.public_inputs);
    let public_signal_words = sccp_groth16_bn254_public_signal_words(
        &request.public_inputs,
        request.source_network.domain_id(),
        request.statement_hash,
        request.destination_binding_hash,
        request.route_configuration_hash,
    );
    let Some(verifying_key_bytes) =
        canonical_sccp_groth16_bn254_verifying_key_bytes_v1(&request.verifying_key)
    else {
        return false;
    };
    request.request_hash
        == sccp_groth16_bn254_proof_request_hash(
            request.backend,
            request.source_network,
            request.target_network,
            &public_inputs_bytes,
            &bundle.canonical_payload_bytes,
            &request.bundle_bytes,
            request.statement_hash,
            request.destination_binding_hash,
            request.route_configuration_hash,
            request.verifier_key_hash,
            &verifying_key_bytes,
            &public_signal_words,
        )
}

fn sccp_groth16_proof_request_public_inputs_are_valid(
    source_network: SccpNetworkV1,
    target_network: SccpNetworkV1,
    public_inputs: &SccpMessageTransparentPublicInputsV1,
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

fn wrap_sccp_groth16_bn254_proof_result(
    proof_bytes: &[u8],
    request: &SccpGroth16Bn254ProofRequestV1,
    expected_backend: BridgeSccpDestinationProofBackendV1,
) -> Option<SccpGroth16Bn254ProofArtifactV1> {
    if !sccp_groth16_bn254_proof_request_is_canonical(request, expected_backend)
        || !verify_sccp_groth16_bn254_proof_v1(
            &request.public_inputs,
            request.source_network.domain_id(),
            request.statement_hash,
            request.destination_binding_hash,
            request.route_configuration_hash,
            proof_bytes,
            &request.verifying_key,
            request.verifier_key_hash,
        )
    {
        return None;
    }
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

fn sccp_groth16_bn254_proof_request_is_self_canonical(
    request: &SccpGroth16Bn254ProofRequestV1,
) -> bool {
    sccp_groth16_bn254_proof_request_is_canonical(request, request.backend)
}

fn sccp_groth16_bn254_proof_result_is_structurally_valid(
    result: &SccpGroth16Bn254ProofResultV1,
) -> bool {
    let Some(proof) = decode_sccp_evm_groth16_bn254_proof_bytes(&result.proof_bytes) else {
        return false;
    };
    result.version == 1
        && h256_is_nonzero(&result.request_hash)
        && h256_is_nonzero(&result.result_hash)
        && proof.version == 1
        && proof.source_domain == SCCP_DOMAIN_SORA
        && h256_is_nonzero(&proof.message_id)
        && h256_is_nonzero(&proof.commitment_root)
        && result.result_hash
            == sccp_groth16_bn254_proof_result_hash(result.request_hash, &result.proof_bytes)
}

fn sccp_groth16_bn254_proof_artifact_is_self_canonical(
    artifact: &SccpGroth16Bn254ProofArtifactV1,
) -> bool {
    if artifact.version != 1
        || !sccp_groth16_bn254_proof_request_is_self_canonical(&artifact.request)
        || !sccp_groth16_bn254_proof_result_is_structurally_valid(&artifact.result)
        || artifact.result.request_hash != artifact.request.request_hash
    {
        return false;
    }
    let expected = match artifact.request.backend {
        BridgeSccpDestinationProofBackendV1::EvmGroth16Bn254 => {
            wrap_sccp_evm_groth16_bn254_proof_result(
                &artifact.result.proof_bytes,
                &artifact.request,
            )
        }
        BridgeSccpDestinationProofBackendV1::TronGroth16Bn254 => {
            wrap_sccp_tron_groth16_bn254_proof_result(
                &artifact.result.proof_bytes,
                &artifact.request,
            )
        }
    };
    expected.as_ref() == Some(artifact)
}

fn decode_canonical_sccp_groth16_bn254_norito_v1<T>(
    bytes: &[u8],
    validate: fn(&T) -> bool,
) -> Option<T>
where
    T: for<'de> norito::NoritoDeserialize<'de> + norito::NoritoSerialize,
{
    if !preflight_uncompressed_norito_frame(bytes, SCCP_GROTH16_BN254_MAX_ENCODED_ARTIFACT_BYTES_V1)
    {
        return None;
    }
    let decoded = norito::decode_from_bytes::<T>(bytes).ok()?;
    if !validate(&decoded) || to_bytes(&decoded).ok()?.as_slice() != bytes {
        return None;
    }
    Some(decoded)
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

/// Decode a closed bridge destination-proof and require its outer backend to
/// equal the canonical artifact's inner backend and target family.
pub fn decode_bridge_sccp_destination_proof_v1(
    proof: &BridgeSccpDestinationProofV1,
) -> Option<SccpGroth16Bn254ProofArtifactV1> {
    let artifact =
        decode_canonical_sccp_groth16_bn254_proof_artifact_v1(proof.encoded_artifact.as_slice())?;
    (sccp_groth16_artifact_bridge_backend_v1(&artifact) == Some(proof.backend)
        && proof.route_configuration_hash == artifact.request.route_configuration_hash
        && proof.is_well_formed_for(
            artifact.request.destination_binding_hash,
            artifact.result.result_hash,
        ))
    .then_some(artifact)
}

/// Return whether a submitted Groth16 artifact is exactly bound to the
/// canonical request reconstructed from governed historical state.
pub fn sccp_groth16_bn254_proof_artifact_matches_governed_route_v1(
    artifact: &SccpGroth16Bn254ProofArtifactV1,
    bundle: &NexusSccpMessageProofV1,
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
    };
    expected.as_ref() == Some(artifact)
}

/// Verify an artifact against governed historical state and derive the exact
/// destination calldata package accepted by the governed deployment.
fn build_sccp_verified_destination_call_from_groth16_artifact_v1(
    bundle: &NexusSccpMessageProofV1,
    artifact: &SccpGroth16Bn254ProofArtifactV1,
    governed_route: &SccpGovernedRouteV1,
) -> Option<SccpVerifiedDestinationCallV1> {
    if !sccp_groth16_bn254_proof_artifact_matches_governed_route_v1(
        artifact,
        bundle,
        governed_route,
    ) {
        return None;
    }
    let canonical_payload_bytes = canonical_sccp_payload_bytes(&bundle.payload);
    if payload_hash(&canonical_payload_bytes) != artifact.request.public_inputs.payload_hash {
        return None;
    }
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
    };
    let SccpPayloadV1::Transfer(transfer) = &bundle.payload;
    Some(SccpVerifiedDestinationCallV1 {
        version: 1,
        backend: artifact.request.backend,
        counterparty_domain: artifact.request.target_network.domain_id(),
        route_revision: transfer.route_revision,
        destination_binding_hash: artifact.request.destination_binding_hash,
        route_configuration_hash: artifact.request.route_configuration_hash,
        target,
        public_inputs: artifact.request.public_inputs.clone(),
        statement_hash: artifact.request.statement_hash,
        request_hash: artifact.request.request_hash,
        proof_bytes: artifact.result.proof_bytes.clone(),
        canonical_payload_bytes,
        calldata,
        bundle: bundle.clone(),
    })
}

/// Verify one closed bridge SCCP destination proof against the exact bundle
/// and historical governed route, then derive the canonical destination call.
pub fn verify_sccp_destination_proof_v1(
    proof: &BridgeSccpDestinationProofV1,
    bundle: &NexusSccpMessageProofV1,
    governed_route: &SccpGovernedRouteV1,
) -> Option<SccpVerifiedDestinationCallV1> {
    let artifact = decode_bridge_sccp_destination_proof_v1(proof)?;
    let route_backend = match governed_route.destination {
        SccpDestinationDeploymentV1::Evm(_) => BridgeSccpDestinationProofBackendV1::EvmGroth16Bn254,
        SccpDestinationDeploymentV1::Tron(_) => {
            BridgeSccpDestinationProofBackendV1::TronGroth16Bn254
        }
    };
    if proof.backend != route_backend
        || proof.route_configuration_hash != governed_route.route_configuration_hash().ok()?
        || proof.route_configuration_hash != bundle.commitment.context.route_configuration_hash
    {
        return None;
    }
    build_sccp_verified_destination_call_from_groth16_artifact_v1(bundle, &artifact, governed_route)
}

fn sccp_exact_xor_destination_route_id_v1(target_domain: u32) -> Option<&'static [u8]> {
    match target_domain {
        SCCP_DOMAIN_ETH => Some(SCCP_TAIRA_ETH_XOR_ROUTE_ID_V1.as_bytes()),
        SCCP_DOMAIN_BSC => Some(SCCP_TAIRA_BSC_XOR_ROUTE_ID_V1.as_bytes()),
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

/// Derive transparent-proof public inputs from one canonical SORA-origin bundle.
pub fn sccp_message_transparent_public_inputs(
    bundle: &NexusSccpMessageProofV1,
) -> Option<SccpMessageTransparentPublicInputsV1> {
    if !verify_message_bundle_structure(bundle) {
        return None;
    }
    let (finality_height, finality_block_hash) = sccp_message_finality_public_inputs(bundle)?;
    Some(SccpMessageTransparentPublicInputsV1 {
        version: 1,
        message_id: bundle.commitment.message_id,
        payload_hash: bundle.commitment.payload_hash,
        target_domain: bundle.commitment.context.lane.target.domain_id(),
        commitment_root: bundle.commitment_root,
        finality_height,
        finality_block_hash,
    })
}
fn decode_nonzero_fixed<const N: usize>(bytes: &[u8]) -> Option<[u8; N]> {
    let value: [u8; N] = bytes.try_into().ok()?;
    value.iter().any(|byte| *byte != 0).then_some(value)
}

fn canonical_sccp_text(bytes: &[u8]) -> Option<&str> {
    if bytes.is_empty()
        || bytes.len() > SCCP_MAX_CANONICAL_TEXT_BYTES_V1
        || !bytes.iter().all(|byte| matches!(byte, 0x21..=0x7e))
    {
        return None;
    }
    core::str::from_utf8(bytes).ok()
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

fn push_vec(out: &mut Vec<u8>, value: &[u8]) {
    push_u32(
        out,
        u32::try_from(value.len()).expect("SCCP vector length fits into u32"),
    );
    out.extend_from_slice(value);
}

fn push_vec_checked(out: &mut Vec<u8>, value: &[u8]) -> Option<()> {
    push_u32_len_checked(out, value.len())?;
    out.extend_from_slice(value);
    Some(())
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
pub fn canonical_transfer_payload_bytes(payload: &TransferPayloadV1) -> Vec<u8> {
    let mut out = Vec::new();
    push_u8(&mut out, payload.version);
    push_u32(&mut out, payload.source_domain);
    push_u32(&mut out, payload.dest_domain);
    push_u64(&mut out, payload.nonce);
    push_u32(&mut out, payload.route_revision);
    push_u32(&mut out, payload.asset_home_domain);
    push_u8(&mut out, payload.asset_id_codec);
    push_vec(&mut out, &payload.asset_id);
    push_u128(&mut out, payload.amount);
    push_u8(&mut out, payload.sender_codec);
    push_vec(&mut out, &payload.sender);
    push_u8(&mut out, payload.recipient_codec);
    push_vec(&mut out, &payload.recipient);
    push_u8(&mut out, payload.route_id_codec);
    push_vec(&mut out, &payload.route_id);
    out
}

/// Encode any closed SCCP payload with its stable V1 discriminant and canonical body.
pub fn canonical_sccp_payload_bytes(payload: &SccpPayloadV1) -> Vec<u8> {
    let mut out = Vec::new();
    match payload {
        SccpPayloadV1::Transfer(payload) => {
            push_u8(&mut out, SccpPayloadV1::TRANSFER_DISCRIMINANT);
            out.extend_from_slice(&canonical_transfer_payload_bytes(payload));
        }
    }
    out
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
    let payload_hash = payload_hash(&canonical_sccp_payload_bytes(payload));
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
/// otherwise identical payload from aliasing across mainnet, testnet, Nexus,
/// or Taira. The governed destination binding is not part of this identity;
/// replay protection therefore survives destination deployment rotation.
/// The Keccak-256 input is `SCCP_LANE_MESSAGE_ID_PREFIX_V1 || 0x01 ||
/// le_u32(lane_len) || canonical_lane || le_u32(payload_len) ||
/// canonical_payload`.
pub fn sccp_message_id(lane: SccpLaneIdV1, payload: &SccpPayloadV1) -> Option<H256> {
    if !verify_sccp_payload_structure(payload) || !sccp_payload_matches_lane(lane, payload) {
        return None;
    }
    let lane_bytes = canonical_sccp_lane_id_bytes_v1(lane)?;
    let payload_bytes = canonical_sccp_payload_bytes(payload);
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

/// Decode one canonical, size-bounded Nexus bridge-finality proof.
pub fn decode_nexus_bridge_finality_proof(
    proof_bytes: &[u8],
) -> Option<NexusBridgeFinalityProofV1> {
    decode_canonical_nexus_proof_artifact(proof_bytes)
}

fn hash_block_header_for_sccp_finality(header: &BlockHeader) -> H256 {
    let mut out = [0u8; 32];
    out.copy_from_slice(header.hash().as_ref().as_ref());
    out
}

fn nexus_validator_set_hash_from_public_keys(public_keys: &[String]) -> Option<H256> {
    let validator_set = public_keys
        .iter()
        .map(|key| {
            key.parse::<iroha_crypto::PublicKey>()
                .ok()
                .map(PeerId::from)
        })
        .collect::<Option<Vec<_>>>()?;
    let hash = iroha_crypto::HashOf::<Vec<PeerId>>::new(&validator_set);
    let mut out = [0u8; 32];
    out.copy_from_slice(hash.as_ref().as_ref());
    Some(out)
}

/// Decode one canonical, size-bounded Nexus SCCP message bundle.
pub fn decode_nexus_sccp_message_proof(proof_bytes: &[u8]) -> Option<NexusSccpMessageProofV1> {
    decode_canonical_nexus_proof_artifact(proof_bytes)
}

fn decode_canonical_nexus_proof_artifact<T>(proof_bytes: &[u8]) -> Option<T>
where
    T: for<'de> norito::NoritoDeserialize<'de> + norito::NoritoSerialize,
{
    if !preflight_uncompressed_norito_frame(proof_bytes, SCCP_NEXUS_MAX_ENCODED_PROOF_BYTES_V1) {
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

/// Verify the canonical structure and quorum-certificate binding of Nexus finality.
pub fn verify_nexus_bridge_finality_proof_structure(proof: &NexusBridgeFinalityProofV1) -> bool {
    if proof.version != 1
        || !matches!(
            proof.chain_id.as_str(),
            SCCP_NEXUS_FINALITY_CHAIN_ID_V1 | SCCP_TAIRA_FINALITY_CHAIN_ID_V1
        )
        || proof.height == 0
        || proof.block_header_bytes.is_empty()
        || proof.block_header_bytes.len() > SCCP_NEXUS_MAX_BLOCK_HEADER_BYTES_V1
        || !preflight_uncompressed_norito_frame(
            &proof.block_header_bytes,
            SCCP_NEXUS_MAX_BLOCK_HEADER_BYTES_V1,
        )
    {
        return false;
    }
    let Ok(block_header) = norito::decode_from_bytes::<BlockHeader>(&proof.block_header_bytes)
    else {
        return false;
    };
    if to_bytes(&block_header).ok().as_deref() != Some(proof.block_header_bytes.as_slice())
        || block_header.height().get() != proof.height
        || hash_block_header_for_sccp_finality(&block_header) != proof.block_hash
        || block_header.sccp_commitment_root() != Some(proof.commitment_root)
    {
        return false;
    }
    let qc = &proof.commit_qc;
    if qc.version != 1
        || qc.phase != NexusConsensusPhaseV1::Commit
        || qc.height != proof.height
        || qc.subject_block_hash != proof.block_hash
        || qc.mode_tag.is_empty()
        || qc.mode_tag.len() > SCCP_NEXUS_MAX_CONSENSUS_MODE_TAG_BYTES_V1
        || !h256_is_nonzero(&qc.validator_set_hash)
        || qc.validator_set_hash_version != VALIDATOR_SET_HASH_VERSION_V1
        || qc.validator_public_keys.is_empty()
        || qc.validator_public_keys.len() > SCCP_NEXUS_MAX_FINALITY_VALIDATORS_V1
        || qc.validator_set_pops.len() != qc.validator_public_keys.len()
        || qc.bls_aggregate_signature.is_empty()
        || qc.bls_aggregate_signature.len() > SCCP_NEXUS_MAX_BLS_PROOF_BYTES_V1
        || qc.bls_aggregate_signature.iter().all(|byte| *byte == 0)
    {
        return false;
    }

    for (idx, public_key) in qc.validator_public_keys.iter().enumerate() {
        if public_key.is_empty() || public_key.len() > SCCP_NEXUS_MAX_PUBLIC_KEY_TEXT_BYTES_V1 {
            return false;
        }
        if qc.validator_public_keys[..idx]
            .iter()
            .any(|known| known == public_key)
        {
            return false;
        }
    }
    let Some(computed_validator_set_hash) =
        nexus_validator_set_hash_from_public_keys(&qc.validator_public_keys)
    else {
        return false;
    };
    if computed_validator_set_hash != qc.validator_set_hash {
        return false;
    }
    for pop in &qc.validator_set_pops {
        if pop.is_empty() || pop.len() > SCCP_NEXUS_MAX_BLS_PROOF_BYTES_V1 {
            return false;
        }
    }

    let roster_len = qc.validator_public_keys.len();
    if qc.signers_bitmap.len() != roster_len.div_ceil(8) {
        return false;
    }
    signer_indices_from_bitmap(&qc.signers_bitmap, roster_len)
        .is_some_and(|indices| !indices.is_empty())
}

/// Verify a Nexus finality proof's canonical header, validator set, PoPs, and commit signature.
pub fn verify_nexus_bridge_finality_proof_cryptographic(
    proof: &NexusBridgeFinalityProofV1,
) -> bool {
    if !verify_nexus_bridge_finality_proof_structure(proof) {
        return false;
    }
    verify_nexus_commit_qc_bls_aggregate(proof)
}

#[cfg(feature = "bls")]
const fn nexus_min_votes_for_len(len: usize) -> usize {
    if len <= 3 {
        return len;
    }
    len.saturating_mul(2) / 3 + 1
}

#[cfg(feature = "bls")]
fn verify_nexus_commit_qc_bls_aggregate(proof: &NexusBridgeFinalityProofV1) -> bool {
    let qc = &proof.commit_qc;
    let roster_len = qc.validator_public_keys.len();
    let Some(signer_indices) = signer_indices_from_bitmap(&qc.signers_bitmap, roster_len) else {
        return false;
    };
    if signer_indices.len() < nexus_min_votes_for_len(roster_len) {
        return false;
    }

    let public_keys = match qc
        .validator_public_keys
        .iter()
        .map(|key| key.parse::<iroha_crypto::PublicKey>())
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(public_keys) => public_keys,
        Err(_) => return false,
    };
    for (public_key, pop) in public_keys.iter().zip(qc.validator_set_pops.iter()) {
        if !public_key
            .try_algorithm()
            .is_ok_and(|algorithm| algorithm == Algorithm::BlsNormal)
        {
            return false;
        }
        if iroha_crypto::bls_normal_pop_verify(public_key, pop).is_err() {
            return false;
        }
    }

    let message = nexus_commit_vote_preimage(&proof.chain_id, qc);
    let signer_public_keys = signer_indices
        .iter()
        .map(|idx| public_keys.get(*idx))
        .collect::<Option<Vec<_>>>();
    let Some(signer_public_keys) = signer_public_keys else {
        return false;
    };
    let signer_pops = signer_indices
        .iter()
        .map(|idx| qc.validator_set_pops.get(*idx).map(Vec::as_slice))
        .collect::<Option<Vec<_>>>();
    let Some(signer_pops) = signer_pops else {
        return false;
    };
    iroha_crypto::bls_normal_verify_preaggregated_same_message(
        &message,
        &qc.bls_aggregate_signature,
        &signer_public_keys,
        &signer_pops,
    )
    .is_ok()
}

#[cfg(not(feature = "bls"))]
fn verify_nexus_commit_qc_bls_aggregate(_proof: &NexusBridgeFinalityProofV1) -> bool {
    false
}

/// Decode and structurally verify the Nexus finality proof for a SORA-origin message.
///
/// External-origin messages are deliberately rejected: first-release inbound
/// admission accepts only the closed protocol-native proof API.
pub fn verified_sccp_message_nexus_finality_proof(
    bundle: &NexusSccpMessageProofV1,
) -> Option<NexusBridgeFinalityProofV1> {
    if !verify_message_bundle_structure(bundle) {
        return None;
    }
    let finality_proof = decode_nexus_bridge_finality_proof(&bundle.finality_proof)?;
    (verify_nexus_bridge_finality_proof_structure(&finality_proof)
        && finality_proof.commitment_root == bundle.commitment_root)
        .then_some(finality_proof)
}

/// Decode and cryptographically verify Nexus finality for a SORA-origin message.
///
/// Builds without BLS support fail closed.
pub fn verified_sccp_message_nexus_finality_proof_for_production(
    bundle: &NexusSccpMessageProofV1,
) -> Option<NexusBridgeFinalityProofV1> {
    if !verify_message_bundle_structure(bundle) {
        return None;
    }
    let finality_proof = decode_nexus_bridge_finality_proof(&bundle.finality_proof)?;
    (verify_nexus_bridge_finality_proof_cryptographic(&finality_proof)
        && finality_proof.commitment_root == bundle.commitment_root)
        .then_some(finality_proof)
}

fn sccp_message_finality_public_inputs(bundle: &NexusSccpMessageProofV1) -> Option<(u64, H256)> {
    if sccp_message_source_domain(&bundle.payload) != SCCP_DOMAIN_SORA {
        return None;
    }
    let proof = decode_nexus_bridge_finality_proof(&bundle.finality_proof)?;
    let expected_chain_id = match bundle.commitment.context.lane.source {
        SccpNetworkV1::SoraNexus => SCCP_NEXUS_FINALITY_CHAIN_ID_V1,
        SccpNetworkV1::SoraTaira => SCCP_TAIRA_FINALITY_CHAIN_ID_V1,
        _ => return None,
    };
    if !verify_nexus_bridge_finality_proof_structure(&proof)
        || proof.commitment_root != bundle.commitment_root
        || proof.chain_id != expected_chain_id
    {
        return None;
    }
    Some((proof.height, proof.block_hash))
}
/// Build the exact domain-separated commit-vote preimage signed by Nexus validators.
pub fn nexus_commit_vote_preimage(chain_id: &str, certificate: &NexusCommitQcV1) -> Vec<u8> {
    let mut out = Vec::with_capacity(32 + 32 * 4 + 8 * 6 + 3);
    let domain = iroha_consensus_domain(chain_id, "Vote", b"v1", &certificate.mode_tag);
    out.extend_from_slice(&domain);
    out.extend_from_slice(&certificate.subject_block_hash);
    out.extend_from_slice(&certificate.parent_state_root);
    out.extend_from_slice(&certificate.post_state_root);
    out.extend_from_slice(&certificate.height.to_be_bytes());
    out.extend_from_slice(&certificate.view.to_be_bytes());
    out.extend_from_slice(&certificate.epoch.to_be_bytes());
    out.extend_from_slice(&certificate.chain_order_hash);
    out.extend_from_slice(&certificate.rechain_seq.to_be_bytes());
    out.push(certificate.phase as u8);
    match certificate.highest_qc {
        Some(highest_qc) => {
            out.push(1);
            out.extend_from_slice(&highest_qc.height.to_be_bytes());
            out.extend_from_slice(&highest_qc.view.to_be_bytes());
            out.extend_from_slice(&highest_qc.epoch.to_be_bytes());
            out.extend_from_slice(&highest_qc.subject_block_hash);
            out.push(highest_qc.phase as u8);
        }
        None => out.push(0),
    }
    out
}

/// Verify one SORA-origin outbound message bundle.
///
/// External-origin bundles are intentionally outside this API. They must be
/// admitted through `SccpNativeInboundMessageProofV1`, whose closed variant
/// selects and verifies the corresponding protocol-native source proof.
pub fn verify_message_bundle_structure(bundle: &NexusSccpMessageProofV1) -> bool {
    if bundle.version != 1
        || bundle.commitment.version != 1
        || sccp_message_source_domain(&bundle.payload) != SCCP_DOMAIN_SORA
        || bundle.merkle_proof.steps.len() > SCCP_NEXUS_MAX_MERKLE_PROOF_STEPS_V1
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
    sccp_message_finality_public_inputs(bundle).is_some()
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

fn iroha_consensus_domain(
    chain_id: &str,
    message_type_tag: &str,
    extra: &[u8],
    mode_tag: &str,
) -> H256 {
    let mut hasher = Blake2bVar::new(64).expect("fixed hash length");
    hasher.update(b"iroha-sumeragi-consensus/v1");
    hasher.update(chain_id.as_bytes());
    hasher.update(mode_tag.as_bytes());
    hasher.update(&IROHA_CONSENSUS_PROTO_VERSION_V1.to_be_bytes());
    hasher.update(message_type_tag.as_bytes());
    hasher.update(extra);
    let mut digest = [0u8; 64];
    hasher
        .finalize_variable(&mut digest)
        .expect("fixed hash length");
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest[..32]);
    out
}

fn signer_indices_from_bitmap(bitmap: &[u8], roster_len: usize) -> Option<Vec<usize>> {
    if bitmap.len() != roster_len.div_ceil(8) {
        return None;
    }

    let mut indices = Vec::new();
    for (byte_idx, byte) in bitmap.iter().enumerate() {
        if *byte == 0 {
            continue;
        }
        for bit in 0..8 {
            if (byte >> bit) & 1 == 0 {
                continue;
            }
            let idx = byte_idx * 8 + bit;
            if idx >= roster_len {
                return None;
            }
            indices.push(idx);
        }
    }
    Some(indices)
}

#[cfg(all(test, feature = "bls"))]
mod tests {
    use std::{num::NonZeroU64, sync::OnceLock};

    use halo2curves::{
        Coordinates, CurveAffine,
        bn256::{Fq, Fq2, Fr, G1Affine, G2Affine},
        group::{Curve, prime::PrimeCurveAffine},
    };
    use iroha_data_model::{
        account::AccountId,
        bridge::{
            BridgeProofPayload, BridgeSccpDestinationProofBackendV1, BridgeTransparentProof,
            SCCP_V1_TAIRA_TO_TOKEN_MULTIPLIER, SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
            SccpBn254G1PointV1, SccpBn254G2PointV1, SccpDestinationDeploymentV1,
            SccpEvmDestinationDeploymentV1, SccpEvmSourceEmitterV1, SccpGovernedRouteV1,
            SccpGroth16Bn254IcV1, SccpGroth16Bn254VerifyingKeyV1, SccpLaneIdV1, SccpNetworkV1,
            SccpOutboundMessageContextV1, SccpRouteActivationV1, SccpSoraSettlementV1,
            SccpSourceEmitterV1, SccpSourceIdentityV1, SccpTronDestinationDeploymentV1,
            sccp_exact_tron_xor_route_config_hash_v1, sccp_lane_id_hash_v1,
            sccp_v1_taira_xor_asset_definition_id,
        },
        proof::ProofBox,
    };

    use super::*;

    struct OutboundFixture {
        route: SccpGovernedRouteV1,
        bundle: NexusSccpMessageProofV1,
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
            },
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
            route_address: [0x51; 20],
            route_code_hash: [0x61; 32],
            taira_to_token_multiplier: SCCP_V1_TAIRA_TO_TOKEN_MULTIPLIER,
        }
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
            source_identity: SccpSourceIdentityV1 {
                lane: lane_id,
                emitter: SccpSourceEmitterV1::Evm(SccpEvmSourceEmitterV1 {
                    address: deployment.route_address,
                    runtime_code_hash: deployment.route_code_hash,
                    route_config_hash,
                }),
            },
            destination,
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

    fn signed_finality_proof(commitment_root: H256, height: u64) -> Vec<u8> {
        let chain_id = SCCP_TAIRA_FINALITY_CHAIN_ID_V1.to_owned();
        let mut block_header = BlockHeader::new(
            NonZeroU64::new(height).expect("nonzero finality height"),
            None,
            None,
            None,
            0,
            0,
        );
        block_header.set_sccp_commitment_root(Some(commitment_root));
        let block_hash = hash_block_header_for_sccp_finality(&block_header);
        let keypairs = [
            KeyPair::try_from_seed(vec![1; 32], Algorithm::BlsNormal).expect("BLS key 1"),
            KeyPair::try_from_seed(vec![2; 32], Algorithm::BlsNormal).expect("BLS key 2"),
            KeyPair::try_from_seed(vec![3; 32], Algorithm::BlsNormal).expect("BLS key 3"),
        ];
        let validator_public_keys = keypairs
            .iter()
            .map(|keypair| keypair.public_key().to_string())
            .collect::<Vec<_>>();
        let validator_set_hash = nexus_validator_set_hash_from_public_keys(&validator_public_keys)
            .expect("valid BLS validator roster");
        let mut commit_qc = NexusCommitQcV1 {
            version: 1,
            phase: NexusConsensusPhaseV1::Commit,
            height,
            view: 1,
            epoch: 1,
            mode_tag: "normal".to_owned(),
            subject_block_hash: block_hash,
            parent_state_root: [0x12; 32],
            post_state_root: [0x13; 32],
            chain_order_hash: [0x14; 32],
            rechain_seq: 1,
            highest_qc: None,
            validator_set_hash,
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_public_keys,
            validator_set_pops: keypairs
                .iter()
                .map(|keypair| {
                    iroha_crypto::bls_normal_pop_prove(keypair.private_key()).expect("BLS PoP")
                })
                .collect(),
            signers_bitmap: vec![0b0000_0111],
            bls_aggregate_signature: Vec::new(),
        };
        let message = nexus_commit_vote_preimage(&chain_id, &commit_qc);
        let signatures = keypairs
            .iter()
            .map(|keypair| {
                Signature::try_new(keypair.private_key(), &message).expect("BLS commit vote")
            })
            .collect::<Vec<_>>();
        let signature_refs = signatures
            .iter()
            .map(|signature| signature.payload().as_ref())
            .collect::<Vec<_>>();
        commit_qc.bls_aggregate_signature =
            iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
                .expect("aggregate BLS commit votes");
        to_bytes(&NexusBridgeFinalityProofV1 {
            version: 1,
            chain_id,
            height,
            block_hash,
            commitment_root,
            block_header_bytes: to_bytes(&block_header).expect("canonical block header"),
            commit_qc,
        })
        .expect("canonical finality proof")
    }

    fn message_bundle(route: &SccpGovernedRouteV1) -> NexusSccpMessageProofV1 {
        let payload = transfer_payload(route.revision);
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
        let bundle = NexusSccpMessageProofV1 {
            version: 1,
            commitment_root,
            commitment,
            merkle_proof,
            payload,
            finality_proof: signed_finality_proof(commitment_root, 31),
        };
        assert!(verify_message_bundle_structure(&bundle));
        assert!(verified_sccp_message_nexus_finality_proof_for_production(&bundle).is_some());
        bundle
    }

    fn valid_proof(request: &SccpGroth16Bn254ProofRequestV1) -> Vec<u8> {
        let signals = sccp_groth16_bn254_public_signal_words(
            &request.public_inputs,
            request.source_network.domain_id(),
            request.statement_hash,
            request.destination_binding_hash,
            request.route_configuration_hash,
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
            assert!(verify_sccp_groth16_bn254_proof_v1(
                &request.public_inputs,
                request.source_network.domain_id(),
                request.statement_hash,
                request.destination_binding_hash,
                request.route_configuration_hash,
                &proof_bytes,
                &request.verifying_key,
                request.verifier_key_hash,
            ));
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

    fn assert_request_rejected(request: SccpGroth16Bn254ProofRequestV1) {
        assert!(encode_canonical_sccp_groth16_bn254_proof_request_v1(&request).is_none());
        let bytes = to_bytes(&request).expect("encode adversarial request");
        assert!(decode_canonical_sccp_groth16_bn254_proof_request_v1(&bytes).is_none());
    }

    #[test]
    fn exact_outbound_path_roundtrips_and_derives_canonical_calldata() {
        let fixture = fixture();
        let payload_bytes = canonical_sccp_payload_bytes(&fixture.bundle.payload);
        assert_eq!(
            decode_canonical_sccp_payload_bytes(&payload_bytes),
            Some(fixture.bundle.payload.clone())
        );
        let bundle_bytes = canonical_nexus_sccp_message_bundle_bytes_checked(&fixture.bundle)
            .expect("canonical bundle");
        assert_eq!(bundle_bytes, fixture.request.bundle_bytes);
        assert!(decode_canonical_nexus_sccp_message_bundle_summary(&bundle_bytes).is_some());

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
    fn solidity_key_route_and_tenth_signal_vectors_match() {
        let key = verifying_key();
        assert!(sccp_groth16_bn254_verifying_key_is_well_formed_v1(&key));
        assert_eq!(
            canonical_sccp_groth16_bn254_verifying_key_bytes_v1(&key)
                .expect("canonical key")
                .len(),
            36 * 32
        );
        assert_eq!(
            sccp_groth16_bn254_verifying_key_hash_v1(&key),
            Some(hex32(
                "51f287450cb7bcc401e07ffe5d726f13aee45f6cce5cb0c8415794d4ba47c774"
            ))
        );

        let tron_deployment = SccpTronDestinationDeploymentV1 {
            token_address: [0x11; 20],
            token_code_hash: [0x21; 32],
            verifier_address: [0x31; 20],
            verifier_code_hash: [0x41; 32],
            verifying_key: key,
            verifier_key_hash: sccp_groth16_bn254_verifying_key_hash_v1(&key).unwrap(),
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
            hex32("6571ac200c92c7db53afa625984f3cbcc5d2d2490033812b4cbac84f3fa7cfc9")
        );

        let request = &fixture().request;
        let signals = sccp_groth16_bn254_public_signal_words(
            &request.public_inputs,
            request.source_network.domain_id(),
            request.statement_hash,
            request.destination_binding_hash,
            request.route_configuration_hash,
        );
        assert_eq!(signals.len(), 10);
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
    }

    #[test]
    fn every_request_role_and_nested_artifact_commitment_is_fail_closed() {
        let base = &fixture().request;
        macro_rules! reject_mutation {
            ($body:expr) => {{
                let mut candidate = base.clone();
                $body(&mut candidate);
                assert_request_rejected(candidate);
            }};
        }
        reject_mutation!(|candidate: &mut SccpGroth16Bn254ProofRequestV1| candidate.version = 2);
        reject_mutation!(
            |candidate: &mut SccpGroth16Bn254ProofRequestV1| candidate.backend =
                BridgeSccpDestinationProofBackendV1::TronGroth16Bn254
        );
        reject_mutation!(|candidate: &mut SccpGroth16Bn254ProofRequestV1| candidate
            .source_network =
            SccpNetworkV1::SoraNexus);
        reject_mutation!(|candidate: &mut SccpGroth16Bn254ProofRequestV1| candidate
            .target_network =
            SccpNetworkV1::EthereumSepolia);
        reject_mutation!(|candidate: &mut SccpGroth16Bn254ProofRequestV1| candidate
            .public_inputs
            .message_id[0] ^= 1);
        reject_mutation!(|candidate: &mut SccpGroth16Bn254ProofRequestV1| candidate
            .public_inputs
            .payload_hash[0] ^=
            1);
        reject_mutation!(|candidate: &mut SccpGroth16Bn254ProofRequestV1| candidate
            .public_inputs
            .target_domain =
            SCCP_DOMAIN_BSC);
        reject_mutation!(|candidate: &mut SccpGroth16Bn254ProofRequestV1| candidate
            .public_inputs
            .commitment_root[0] ^=
            1);
        reject_mutation!(|candidate: &mut SccpGroth16Bn254ProofRequestV1| candidate
            .public_inputs
            .finality_height +=
            1);
        reject_mutation!(|candidate: &mut SccpGroth16Bn254ProofRequestV1| candidate
            .public_inputs
            .finality_block_hash[0] ^=
            1);
        reject_mutation!(|candidate: &mut SccpGroth16Bn254ProofRequestV1| candidate
            .verifying_key
            .alpha1
            .y = word_u64(3));
        reject_mutation!(|candidate: &mut SccpGroth16Bn254ProofRequestV1| candidate
            .verifier_key_hash[0] ^=
            1);
        reject_mutation!(|candidate: &mut SccpGroth16Bn254ProofRequestV1| candidate
            .bundle_bytes
            .push(0));
        reject_mutation!(|candidate: &mut SccpGroth16Bn254ProofRequestV1| candidate
            .statement_hash[0] ^=
            1);
        reject_mutation!(|candidate: &mut SccpGroth16Bn254ProofRequestV1| candidate
            .destination_binding_hash[0] ^=
            1);
        reject_mutation!(|candidate: &mut SccpGroth16Bn254ProofRequestV1| candidate
            .route_configuration_hash[0] ^=
            1);
        reject_mutation!(|candidate: &mut SccpGroth16Bn254ProofRequestV1| candidate
            .request_hash[0] ^=
            1);

        let mut artifact = fixture().artifact.clone();
        artifact.result.version = 2;
        assert!(
            decode_canonical_sccp_groth16_bn254_proof_artifact_v1(&to_bytes(&artifact).unwrap())
                .is_none()
        );
        let mut artifact = fixture().artifact.clone();
        artifact.result.request_hash[0] ^= 1;
        assert!(
            decode_canonical_sccp_groth16_bn254_proof_artifact_v1(&to_bytes(&artifact).unwrap())
                .is_none()
        );
        let mut artifact = fixture().artifact.clone();
        artifact.result.proof_bytes[0] ^= 1;
        assert!(
            decode_canonical_sccp_groth16_bn254_proof_artifact_v1(&to_bytes(&artifact).unwrap())
                .is_none()
        );
        let mut artifact = fixture().artifact.clone();
        artifact.result.result_hash[0] ^= 1;
        assert!(
            decode_canonical_sccp_groth16_bn254_proof_artifact_v1(&to_bytes(&artifact).unwrap())
                .is_none()
        );
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
            SccpNetworkV1::EthereumSepolia,
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
            proof: ProofBox::new("generic-transparent".to_owned(), vec![1, 2, 3]),
            recursion_depth: None,
        });
        assert!(!matches!(generic, BridgeProofPayload::SccpDestination(_)));
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
        let mut finality = decode_nexus_bridge_finality_proof(&candidate.finality_proof).unwrap();
        finality.height += 1;
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

    #[test]
    fn quorum_formula_matches_consensus_for_every_supported_roster() {
        for roster_len in 1..=SCCP_NEXUS_MAX_FINALITY_VALIDATORS_V1 {
            let expected = roster_len.saturating_mul(2) / 3 + 1;
            assert_eq!(nexus_min_votes_for_len(roster_len), expected);
            assert!(expected <= roster_len);
            if expected > 1 {
                assert!((expected - 1) < nexus_min_votes_for_len(roster_len));
            }
        }
    }
}
