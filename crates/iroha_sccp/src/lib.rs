//! SCCP payload, proof, and counterparty submission helpers for Iroha bridge flows.
//!
//! The crate targets the Rust standard library. The historical `std` feature is
//! kept as a compatibility alias for workspace consumers that still enable it.
#![allow(missing_docs)]
#![allow(missing_copy_implementations)]

extern crate alloc;

use alloc::{borrow::ToOwned, format, string::String, vec::Vec};

use blake2::{
    Blake2bVar,
    digest::{Update, VariableOutput},
};
use fastpq_prover::{
    OperationKind as FastpqOperationKind, Prover as FastpqProver,
    PublicInputs as FastpqPublicInputs, StateTransition as FastpqStateTransition,
    TransitionBatch as FastpqTransitionBatch,
};
use iroha_crypto::{Algorithm, EcdsaSecp256k1Sha256, KeyPair};
use iroha_data_model::zk::{BackendTag, OpenVerifyEnvelope, StarkFriOpenProofV1};
use norito::to_bytes;
use sha2::{Digest, Sha256};
use tiny_keccak::Hasher;

pub const SCCP_DOMAIN_SORA: u32 = 0;
pub const SCCP_DOMAIN_ETH: u32 = 1;
pub const SCCP_DOMAIN_BSC: u32 = 2;
pub const SCCP_DOMAIN_SOL: u32 = 3;
pub const SCCP_DOMAIN_TON: u32 = 4;
pub const SCCP_DOMAIN_TRON: u32 = 5;
pub const SCCP_DOMAIN_SORA_KUSAMA: u32 = 6;
pub const SCCP_DOMAIN_SORA_POLKADOT: u32 = 7;
pub const SCCP_DOMAIN_SORA2: u32 = 8;
pub const SCCP_STARK_FRI_PROOF_FAMILY_V1: &str = "stark-fri-v1";
/// Production EVM-family destination verifier backend for SCCP recursive proofs.
pub const SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1: &str = "evm-groth16-bn254-v1";
/// Production-planned TRON TVM destination verifier backend for SCCP recursive proofs.
pub const SCCP_TRON_GROTH16_BN254_PROOF_BACKEND_V1: &str = "tron-groth16-bn254-v1";
/// Reference-only EVM attestation backend. This is never a production SCCP verifier.
pub const SCCP_EVM_SECP256K1_PROOF_BACKEND_V1: &str = "evm-secp256k1-keccak-v1";
/// Typed bridge-proof backend for SCCP burn bundles submitted to Iroha.
pub const SCCP_BURN_BRIDGE_PROOF_BACKEND_V1: &str = "sccp/burn-bundle-v1";
/// Manifest seed reserved for SCCP burn bundle bridge proofs.
pub const SCCP_BURN_BRIDGE_PROOF_MANIFEST_SEED_V1: &str = "iroha:sccp:bridge-proof:burn:v1";

pub const SCCP_CODEC_TEXT_UTF8: u8 = 1;
pub const SCCP_CODEC_EVM_HEX: u8 = 2;
pub const SCCP_CODEC_SOLANA_BASE58: u8 = 3;
pub const SCCP_CODEC_TON_RAW: u8 = 4;
pub const SCCP_CODEC_TRON_BASE58CHECK: u8 = 5;
pub const SCCP_CODEC_SORA_ASSET_ID: u8 = 6;

pub const SCCP_RUNTIME_PROOF_FAMILY_V1: &str = "runtime-scale-v1";
pub const SCCP_RUNTIME_VERIFIER_BACKEND_V1: &str = "sora-nexus-runtime-v1";

pub const SCCP_CORE_REMOTE_DOMAINS: [u32; 8] = [
    SCCP_DOMAIN_ETH,
    SCCP_DOMAIN_BSC,
    SCCP_DOMAIN_SOL,
    SCCP_DOMAIN_TON,
    SCCP_DOMAIN_TRON,
    SCCP_DOMAIN_SORA_KUSAMA,
    SCCP_DOMAIN_SORA_POLKADOT,
    SCCP_DOMAIN_SORA2,
];

pub const SCCP_MSG_PREFIX_BURN_V1: &[u8] = b"sccp:burn:v1";
pub const SCCP_MSG_PREFIX_TOKEN_ADD_V1: &[u8] = b"sccp:token:add:v1";
pub const SCCP_MSG_PREFIX_TOKEN_PAUSE_V1: &[u8] = b"sccp:token:pause:v1";
pub const SCCP_MSG_PREFIX_TOKEN_RESUME_V1: &[u8] = b"sccp:token:resume:v1";
pub const SCCP_MSG_PREFIX_ASSET_REGISTER_V1: &[u8] = b"sccp:asset:register:v1";
pub const SCCP_MSG_PREFIX_ROUTE_ACTIVATE_V1: &[u8] = b"sccp:route:activate:v1";
pub const SCCP_MSG_PREFIX_TRANSFER_V1: &[u8] = b"sccp:transfer:v1";
pub const IROHA_CONSENSUS_PROTO_VERSION_V1: u32 = 1;

const SCCP_HUB_LEAF_PREFIX_V1: &[u8] = b"sccp:hub:leaf:v1";
const SCCP_HUB_NODE_PREFIX_V1: &[u8] = b"sccp:hub:node:v1";
const SCCP_PAYLOAD_HASH_PREFIX_V1: &[u8] = b"sccp:payload:v1";
const SCCP_SOURCE_EVENT_DIGEST_PREFIX_V1: &[u8] = b"sccp:source:event:v1";
const SCCP_SOURCE_EVENT_LEAF_PREFIX_V1: &[u8] = b"sccp:source:event-leaf:v1";
const SCCP_SOURCE_HEADER_PREFIX_V1: &[u8] = b"sccp:source:header:v1";
const SCCP_SOURCE_NODE_PREFIX_V1: &[u8] = b"sccp:source:node:v1";
const SCCP_SOLANA_MESSAGE_PROOF_PREFIX_V1: &[u8] = b"sccp:solana:message-proof:v1";
const SCCP_TRANSPARENT_STATEMENT_PREFIX_V1: &[u8] = b"sccp:transparent:statement:v1";
const SCCP_DESTINATION_BINDING_PREFIX_V1: &[u8] = b"sccp:destination:binding:v1";
const SCCP_TRANSPARENT_FASTPQ_DSID_PREFIX_V1: &[u8] = b"sccp:transparent:fastpq:dsid:v1";
const SCCP_TRANSPARENT_OPEN_VERIFY_SCHEMA_HASH_PREFIX_V1: &[u8] =
    b"sccp:transparent:open-verify-schema:v1";
const SCCP_TRANSPARENT_FASTPQ_PARAMETER_SET_V1: &str = "fastpq-lane-balanced";
const SCCP_TON_BOC_MAGIC: [u8; 4] = [0xB5, 0xEE, 0x9C, 0x72];
const SCCP_TON_SUBMIT_OP_V1: u32 = 0x5343_4350;
const SCCP_TON_MESSAGE_SCHEMA_VERSION_V1: u16 = 1;
const SCCP_TON_MAX_CELL_DATA_BYTES: usize = 127;
const SCCP_TON_MAX_REFS: usize = 4;
const SCCP_TRANSPARENT_FASTPQ_STATEMENT_KEY_V1: &[u8] = b"sccp:transparent:v1:statement";
const SCCP_TRANSPARENT_FASTPQ_CONTEXT_KEY_V1: &[u8] = b"sccp:transparent:v1:context";
const SCCP_TRANSPARENT_FASTPQ_PAYLOAD_KEY_V1: &[u8] = b"sccp:transparent:v1:payload";
const SCCP_TRANSPARENT_OPEN_VERIFY_CIRCUIT_ID_V1: &str = "sccp-message-transparent-v1";
const SCCP_SOURCE_ADAPTER_FASTPQ_DSID_PREFIX_V1: &[u8] = b"sccp:source-adapter:fastpq:dsid:v1";
const SCCP_SOURCE_ADAPTER_FASTPQ_PARAMETER_SET_V1: &str = "fastpq-lane-balanced";
const SCCP_SOURCE_ADAPTER_FASTPQ_STATEMENT_KEY_V1: &[u8] = b"sccp:source-adapter:v1:statement";
const SCCP_SOURCE_ADAPTER_FASTPQ_ADAPTER_KEY_V1: &[u8] = b"sccp:source-adapter:v1:adapter";
const SCCP_SOURCE_ADAPTER_FASTPQ_CONTEXT_KEY_V1: &[u8] = b"sccp:source-adapter:v1:context";
const SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID_V1: &str = "sccp-source-adapter-v1";
const SCCP_EVM_ATTESTATION_DOMAIN_PREFIX_V1: &[u8] = b"iroha:sccp:evm-attestation:v1";
const SCCP_EVM_DESTINATION_BINDING_DOMAIN_PREFIX_V1: &[u8] =
    b"iroha:sccp:evm-destination-binding:v1";

pub type H256 = [u8; 32];

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

#[cfg(feature = "serde")]
mod serde_utils {
    use alloc::{
        borrow::ToOwned,
        format,
        string::{String, ToString},
        vec::Vec,
    };

    use serde::{
        Deserialize, Deserializer, Serializer,
        de::{self, Visitor},
        ser::SerializeSeq,
    };

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

    fn strip_hex_prefix(value: &str) -> &str {
        value
            .strip_prefix("0x")
            .or_else(|| value.strip_prefix("0X"))
            .unwrap_or(value)
    }

    fn decode_nibble(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            b'A'..=b'F' => Some(byte - b'A' + 10),
            _ => None,
        }
    }

    fn decode_hex_vec(value: &str) -> Result<Vec<u8>, String> {
        let raw = strip_hex_prefix(value).as_bytes();
        if !raw.len().is_multiple_of(2) {
            return Err("hex value must have an even number of digits".to_owned());
        }

        let mut out = Vec::with_capacity(raw.len() / 2);
        let mut idx = 0usize;
        while idx < raw.len() {
            let hi = decode_nibble(raw[idx])
                .ok_or_else(|| format!("invalid hex digit at position {idx}"))?;
            let lo = decode_nibble(raw[idx + 1])
                .ok_or_else(|| format!("invalid hex digit at position {}", idx + 1))?;
            out.push((hi << 4) | lo);
            idx += 2;
        }
        Ok(out)
    }

    fn decode_hex_fixed<const N: usize>(value: &str) -> Result<[u8; N], String> {
        let bytes = decode_hex_vec(value)?;
        if bytes.len() != N {
            return Err(format!("expected {N} bytes, got {}", bytes.len()));
        }
        let mut out = [0u8; N];
        out.copy_from_slice(&bytes);
        Ok(out)
    }

    struct DecimalStringVisitor<T> {
        label: &'static str,
        marker: core::marker::PhantomData<T>,
    }

    impl<T> DecimalStringVisitor<T> {
        const fn new(label: &'static str) -> Self {
            Self {
                label,
                marker: core::marker::PhantomData,
            }
        }
    }

    impl Visitor<'_> for DecimalStringVisitor<u64> {
        type Value = u64;

        fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            write!(
                formatter,
                "{} encoded as a decimal string or integer",
                self.label
            )
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(value)
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            u64::try_from(value)
                .map_err(|_| E::custom(format!("{} must not be negative", self.label)))
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            value.parse::<u64>().map_err(|err| {
                E::custom(format!(
                    "failed to parse {} decimal string: {err}",
                    self.label
                ))
            })
        }

        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            self.visit_str(&value)
        }
    }

    impl Visitor<'_> for DecimalStringVisitor<u128> {
        type Value = u128;

        fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            write!(
                formatter,
                "{} encoded as a decimal string or integer",
                self.label
            )
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(u128::from(value))
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            u128::try_from(value)
                .map_err(|_| E::custom(format!("{} must not be negative", self.label)))
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            value.parse::<u128>().map_err(|err| {
                E::custom(format!(
                    "failed to parse {} decimal string: {err}",
                    self.label
                ))
            })
        }

        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            self.visit_str(&value)
        }
    }

    pub mod hex32 {
        use super::{Deserialize, Deserializer, Serializer, String, decode_hex_fixed, encode_hex};

        pub fn serialize<S>(value: &[u8; 32], serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_str(&encode_hex(value))
        }

        pub fn deserialize<'de, D>(deserializer: D) -> Result<[u8; 32], D::Error>
        where
            D: Deserializer<'de>,
        {
            let value = String::deserialize(deserializer)?;
            decode_hex_fixed::<32>(&value).map_err(serde::de::Error::custom)
        }
    }

    pub mod bytes_hex {
        use super::{
            Deserialize, Deserializer, Serializer, String, Vec, decode_hex_vec, encode_hex,
        };

        pub fn serialize<S>(value: &[u8], serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_str(&encode_hex(value))
        }

        pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
        where
            D: Deserializer<'de>,
        {
            let value = String::deserialize(deserializer)?;
            decode_hex_vec(&value).map_err(serde::de::Error::custom)
        }
    }

    pub mod vec_bytes_hex {
        use super::{
            Deserialize, Deserializer, SerializeSeq, Serializer, String, Vec, decode_hex_vec,
            encode_hex,
        };

        pub fn serialize<S>(value: &[Vec<u8>], serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let mut seq = serializer.serialize_seq(Some(value.len()))?;
            for item in value {
                seq.serialize_element(&encode_hex(item))?;
            }
            seq.end()
        }

        pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<Vec<u8>>, D::Error>
        where
            D: Deserializer<'de>,
        {
            let values = Vec::<String>::deserialize(deserializer)?;
            values
                .into_iter()
                .map(|value| decode_hex_vec(&value).map_err(serde::de::Error::custom))
                .collect()
        }
    }

    pub mod u64_string {
        use super::{DecimalStringVisitor, Deserializer, Serializer, ToString};

        #[allow(clippy::trivially_copy_pass_by_ref)]
        pub fn serialize<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_str(&value.to_string())
        }

        pub fn deserialize<'de, D>(deserializer: D) -> Result<u64, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer.deserialize_any(DecimalStringVisitor::<u64>::new("u64"))
        }
    }

    pub mod u128_string {
        use super::{DecimalStringVisitor, Deserializer, Serializer, ToString};

        pub fn serialize<S>(value: &u128, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_str(&value.to_string())
        }

        pub fn deserialize<'de, D>(deserializer: D) -> Result<u128, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer.deserialize_any(DecimalStringVisitor::<u128>::new("u128"))
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct BurnPayloadV1 {
    pub version: u8,
    pub source_domain: u32,
    pub dest_domain: u32,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::u64_string"))]
    pub nonce: u64,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub sora_asset_id: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::u128_string"))]
    pub amount: u128,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub recipient: H256,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct TokenAddPayloadV1 {
    pub version: u8,
    pub target_domain: u32,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::u64_string"))]
    pub nonce: u64,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub sora_asset_id: H256,
    pub decimals: u8,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub name: [u8; 32],
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub symbol: [u8; 32],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct TokenControlPayloadV1 {
    pub version: u8,
    pub target_domain: u32,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::u64_string"))]
    pub nonce: u64,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub sora_asset_id: H256,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct AssetRegisterPayloadV1 {
    pub version: u8,
    pub target_domain: u32,
    pub home_domain: u32,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::u64_string"))]
    pub nonce: u64,
    pub asset_id_codec: u8,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub asset_id: Vec<u8>,
    pub decimals: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct RouteActivatePayloadV1 {
    pub version: u8,
    pub source_domain: u32,
    pub target_domain: u32,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::u64_string"))]
    pub nonce: u64,
    pub asset_id_codec: u8,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub asset_id: Vec<u8>,
    pub route_id_codec: u8,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub route_id: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct TransferPayloadV1 {
    pub version: u8,
    pub source_domain: u32,
    pub dest_domain: u32,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::u64_string"))]
    pub nonce: u64,
    pub asset_home_domain: u32,
    pub asset_id_codec: u8,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub asset_id: Vec<u8>,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::u128_string"))]
    pub amount: u128,
    pub sender_codec: u8,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub sender: Vec<u8>,
    pub recipient_codec: u8,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub recipient: Vec<u8>,
    pub route_id_codec: u8,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub route_id: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub enum SccpPayloadV1 {
    AssetRegister(AssetRegisterPayloadV1),
    RouteActivate(RouteActivatePayloadV1),
    Transfer(TransferPayloadV1),
    TokenAdd(TokenAddPayloadV1),
    TokenPause(TokenControlPayloadV1),
    TokenResume(TokenControlPayloadV1),
}

impl SccpPayloadV1 {
    const ASSET_REGISTER_DISCRIMINANT: u8 = 0;
    const ROUTE_ACTIVATE_DISCRIMINANT: u8 = 1;
    const TRANSFER_DISCRIMINANT: u8 = 2;
    const TOKEN_ADD_DISCRIMINANT: u8 = 3;
    const TOKEN_PAUSE_DISCRIMINANT: u8 = 4;
    const TOKEN_RESUME_DISCRIMINANT: u8 = 5;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub enum SccpHubMessageKind {
    Burn,
    TokenAdd,
    TokenPause,
    TokenResume,
    AssetRegister,
    RouteActivate,
    Transfer,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct SccpHubCommitmentV1 {
    pub version: u8,
    pub kind: SccpHubMessageKind,
    pub target_domain: u32,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub message_id: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub payload_hash: H256,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct SccpMerkleStepV1 {
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub sibling_hash: H256,
    pub sibling_is_left: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct SccpMerkleProofV1 {
    pub steps: Vec<SccpMerkleStepV1>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub enum NexusConsensusPhaseV1 {
    Prepare = 1,
    Commit = 2,
    NewView = 3,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct NexusCommitQcV1 {
    pub version: u8,
    pub phase: NexusConsensusPhaseV1,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::u64_string"))]
    pub height: u64,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::u64_string"))]
    pub view: u64,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::u64_string"))]
    pub epoch: u64,
    pub mode_tag: String,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub subject_block_hash: H256,
    pub validator_set_hash_version: u16,
    pub validator_public_keys: Vec<String>,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::vec_bytes_hex"))]
    pub validator_set_pops: Vec<Vec<u8>>,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub signers_bitmap: Vec<u8>,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub bls_aggregate_signature: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct NexusBridgeFinalityProofV1 {
    pub version: u8,
    pub chain_id: String,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::u64_string"))]
    pub height: u64,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub block_hash: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub commitment_root: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub block_header_bytes: Vec<u8>,
    pub commit_qc: NexusCommitQcV1,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct SccpSourceChainProofEnvelopeV1 {
    pub version: u8,
    pub source_domain: u32,
    pub target_domain: u32,
    pub source_chain: String,
    pub source_proof_plan: SccpSourceProofPlanV1,
    pub finality_model: SccpProofFinalityModelV1,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub message_id: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub payload_hash: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub source_event_digest: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub commitment_root: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::u64_string"))]
    pub finality_height: u64,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub finality_block_hash: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub finalized_header_hash: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub receipt_or_message_root: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub consensus_proof: Vec<u8>,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub message_inclusion_proof: Vec<u8>,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::vec_bytes_hex"))]
    pub inclusion_branch: Vec<Vec<u8>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct SccpSourceAdapterVerificationProofV1 {
    pub version: u8,
    pub proof_family: String,
    pub circuit_id: String,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub proof_bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct SccpSourceVerifierEvidenceV1 {
    pub version: u8,
    pub source_domain: u32,
    pub source_chain: String,
    pub source_proof_plan: SccpSourceProofPlanV1,
    pub finality_model: SccpProofFinalityModelV1,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub adapter_proof_hash: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub adapter_transcript_hash: H256,
    pub adapter_circuit_id: String,
    pub source_trust_anchor_id: String,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub source_trust_anchor_hash: H256,
    pub consensus_verifier_id: String,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub consensus_verifier_hash: H256,
    pub message_inclusion_verifier_id: String,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub message_inclusion_verifier_hash: H256,
    pub finality_policy_id: String,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub finality_policy_hash: H256,
}

#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::JsonSerialize, norito::derive::JsonDeserialize,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct SccpSourceVerifierMaterialV1 {
    pub version: u8,
    pub source_domain: u32,
    pub source_chain: String,
    pub source_proof_plan: SccpSourceProofPlanV1,
    pub finality_model: SccpProofFinalityModelV1,
    pub adapter_circuit_id: String,
    pub source_trust_anchor_id: String,
    pub source_trust_anchor_hash: H256,
    pub consensus_verifier_id: String,
    pub consensus_verifier_hash: H256,
    pub message_inclusion_verifier_id: String,
    pub message_inclusion_verifier_hash: H256,
    pub finality_policy_id: String,
    pub finality_policy_hash: H256,
    pub placeholder_material: bool,
}

impl Default for SccpSourceVerifierMaterialV1 {
    fn default() -> Self {
        Self {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            source_chain: "sora".to_owned(),
            source_proof_plan: SccpSourceProofPlanV1::Unknown,
            finality_model: SccpProofFinalityModelV1::EthereumBeaconExecution,
            adapter_circuit_id: String::new(),
            source_trust_anchor_id: String::new(),
            source_trust_anchor_hash: [0u8; 32],
            consensus_verifier_id: String::new(),
            consensus_verifier_hash: [0u8; 32],
            message_inclusion_verifier_id: String::new(),
            message_inclusion_verifier_hash: [0u8; 32],
            finality_policy_id: String::new(),
            finality_policy_hash: [0u8; 32],
            placeholder_material: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct SccpSourceConsensusProofV1 {
    pub version: u8,
    pub source_domain: u32,
    pub source_chain: String,
    pub source_proof_plan: SccpSourceProofPlanV1,
    pub finality_model: SccpProofFinalityModelV1,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::u64_string"))]
    pub finality_height: u64,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub finality_block_hash: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub receipt_or_message_root: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub finalized_header_hash: H256,
    pub adapter_proof: SccpSourceAdapterProofV1,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub adapter_transcript_hash: H256,
    pub verifier_evidence: SccpSourceVerifierEvidenceV1,
    pub adapter_verification_proof: SccpSourceAdapterVerificationProofV1,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct SccpSourceMessageInclusionProofV1 {
    pub version: u8,
    pub source_domain: u32,
    pub target_domain: u32,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub message_id: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub payload_hash: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub source_event_digest: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub source_event_leaf_hash: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub receipt_or_message_root: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::u64_string"))]
    pub leaf_index: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct SccpEvmBeaconSourceProofV1 {
    pub version: u8,
    pub source_domain: u32,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::u64_string"))]
    pub beacon_slot: u64,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::u64_string"))]
    pub execution_block_number: u64,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub execution_block_hash: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub execution_receipts_root: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub beacon_finalized_root: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub sync_committee_root: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub receipt_trie_proof_hash: H256,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct SccpBscValidatorSetSourceProofV1 {
    pub version: u8,
    pub source_domain: u32,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::u64_string"))]
    pub validator_epoch: u64,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::u64_string"))]
    pub block_number: u64,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub block_hash: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub receipts_root: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub validator_set_hash: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub commit_seal_hash: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub receipt_trie_proof_hash: H256,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct SccpSolanaFinalizedSourceProofV1 {
    pub version: u8,
    pub source_domain: u32,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::u64_string"))]
    pub finalized_slot: u64,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub blockhash: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub bank_hash: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub transaction_status_root: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub message_proof_hash: H256,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct SccpTonMasterchainSourceProofV1 {
    pub version: u8,
    pub source_domain: u32,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::u64_string"))]
    pub masterchain_seqno: u64,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub masterchain_block_hash: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub shard_block_hash: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub shard_state_root: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub transaction_root: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub shard_proof_hash: H256,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct SccpTronDposSourceProofV1 {
    pub version: u8,
    pub source_domain: u32,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::u64_string"))]
    pub solid_block_number: u64,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub block_hash: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub witness_schedule_hash: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub receipt_root: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub transaction_root: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub receipt_proof_hash: H256,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct SccpSubstrateGrandpaSourceProofV1 {
    pub version: u8,
    pub source_domain: u32,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::u64_string"))]
    pub finalized_block_number: u64,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::u64_string"))]
    pub grandpa_set_id: u64,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub block_hash: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub authority_set_hash: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub events_root: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub storage_proof_hash: H256,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub enum SccpSourceAdapterProofV1 {
    EthereumBeaconReceipt(SccpEvmBeaconSourceProofV1),
    BscValidatorSetReceipt(SccpBscValidatorSetSourceProofV1),
    SolanaFinalizedTransaction(SccpSolanaFinalizedSourceProofV1),
    TonMasterchainShard(SccpTonMasterchainSourceProofV1),
    TronDposReceipt(SccpTronDposSourceProofV1),
    SubstrateGrandpaEvent(SccpSubstrateGrandpaSourceProofV1),
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct NexusSccpBurnProofV1 {
    pub version: u8,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub commitment_root: H256,
    pub commitment: SccpHubCommitmentV1,
    pub merkle_proof: SccpMerkleProofV1,
    pub payload: BurnPayloadV1,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub finality_proof: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct NexusSccpMessageProofV1 {
    pub version: u8,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub commitment_root: H256,
    pub commitment: SccpHubCommitmentV1,
    pub merkle_proof: SccpMerkleProofV1,
    pub payload: SccpPayloadV1,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub finality_proof: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SccpRuntimeProofKindV1 {
    Burn,
    TokenAdd,
    TokenPause,
    TokenResume,
    AssetRegister,
    RouteActivate,
    Transfer,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SccpRuntimeHubCommitmentV1 {
    pub version: u8,
    pub kind: SccpRuntimeProofKindV1,
    pub target_domain: u32,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub message_id: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub payload_hash: H256,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SccpRuntimeMerkleStepV1 {
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub sibling_hash: H256,
    pub sibling_is_left: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SccpRuntimeMerkleProofV1 {
    pub steps: Vec<SccpRuntimeMerkleStepV1>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SccpRuntimePayloadV1 {
    AssetRegister(AssetRegisterPayloadV1),
    RouteActivate(RouteActivatePayloadV1),
    Transfer(TransferPayloadV1),
    TokenAdd(TokenAddPayloadV1),
    TokenPause(TokenControlPayloadV1),
    TokenResume(TokenControlPayloadV1),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SccpRuntimeFinalityProofV1 {
    pub version: u8,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::u64_string"))]
    pub epoch: u64,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::u64_string"))]
    pub height: u64,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub block_hash: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub commitment_root: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub validator_set_hash: H256,
    pub signature_count: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SccpRuntimeProofEnvelopeV1 {
    pub version: u8,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub commitment_root: H256,
    pub commitment: SccpRuntimeHubCommitmentV1,
    pub merkle_proof: SccpRuntimeMerkleProofV1,
    pub payload: SccpRuntimePayloadV1,
    pub finality_proof: SccpRuntimeFinalityProofV1,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct SccpMessageTransparentPublicInputsV1 {
    pub version: u8,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub message_id: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub payload_hash: H256,
    pub target_domain: u32,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub commitment_root: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::u64_string"))]
    pub finality_height: u64,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub finality_block_hash: H256,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct NexusSccpMessageTransparentProofV1 {
    pub version: u8,
    pub local_domain: u32,
    pub counterparty_domain: u32,
    pub security_model: SccpProofSecurityModelV1,
    pub anchor_governance: SccpAnchorGovernanceV1,
    pub destination_binding: SccpDestinationBindingV1,
    pub proof_family: String,
    pub verifier_backend: SccpVerifierBackendV1,
    pub message_backend: String,
    pub registry_backend: String,
    pub manifest_seed: String,
    pub finality_model: SccpProofFinalityModelV1,
    pub verifier_target: SccpProofVerifierTargetV1,
    pub public_inputs: SccpMessageTransparentPublicInputsV1,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub proof_bytes: Vec<u8>,
    pub submission_package: SccpCounterpartySubmissionPackageV1,
    pub bundle: NexusSccpMessageProofV1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub enum SccpTransparentChainFamilyV1 {
    Evm,
    Solana,
    Ton,
    Tron,
    Substrate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct SccpMessageTransparentInnerProofV1 {
    pub version: u8,
    pub chain_family: SccpTransparentChainFamilyV1,
    pub chain: String,
    pub local_domain: u32,
    pub counterparty_domain: u32,
    pub security_model: SccpProofSecurityModelV1,
    pub anchor_governance: SccpAnchorGovernanceV1,
    pub destination_binding: SccpDestinationBindingV1,
    pub counterparty_account_codec: u8,
    pub counterparty_account_codec_key: String,
    pub proof_family: String,
    pub verifier_backend: SccpVerifierBackendV1,
    pub message_backend: String,
    pub registry_backend: String,
    pub manifest_seed: String,
    pub finality_model: SccpProofFinalityModelV1,
    pub verifier_target: SccpProofVerifierTargetV1,
    pub public_inputs: SccpMessageTransparentPublicInputsV1,
    pub payload_kind: String,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub payload_hash: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub statement_hash: H256,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SccpOpenVerifyEnvelopeSummaryV1 {
    pub version: u16,
    pub backend: String,
    pub circuit_id: String,
    pub vk_hash: H256,
    pub public_inputs_schema_hash: H256,
    pub public_inputs_schema_len_bytes: u32,
    pub public_input_column_count: u32,
    pub public_input_word_count: u32,
    pub open_proof_len_bytes: u32,
    pub backend_proof_len_bytes: u32,
    pub aux_len_bytes: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub enum SccpNormalizedCodecValueV1 {
    TextUtf8 { value: String },
    EvmHex { bytes: [u8; 20] },
    SolanaBase58 { bytes: [u8; 32] },
    TonRaw { workchain: i32, account: [u8; 32] },
    TronBase58Check { payload: [u8; 21] },
    SoraAssetId { bytes: H256 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct SccpAssetRegisterProjectionV1 {
    pub version: u8,
    pub target_domain: u32,
    pub home_domain: u32,
    pub nonce: u64,
    pub asset_id: SccpNormalizedCodecValueV1,
    pub decimals: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct SccpRouteActivateProjectionV1 {
    pub version: u8,
    pub source_domain: u32,
    pub target_domain: u32,
    pub nonce: u64,
    pub asset_id: SccpNormalizedCodecValueV1,
    pub route_id: SccpNormalizedCodecValueV1,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct SccpTransferProjectionV1 {
    pub version: u8,
    pub source_domain: u32,
    pub dest_domain: u32,
    pub nonce: u64,
    pub asset_home_domain: u32,
    pub asset_id: SccpNormalizedCodecValueV1,
    pub amount: u128,
    pub sender: SccpNormalizedCodecValueV1,
    pub recipient: SccpNormalizedCodecValueV1,
    pub route_id: SccpNormalizedCodecValueV1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct SccpTokenAddProjectionV1 {
    pub version: u8,
    pub target_domain: u32,
    pub nonce: u64,
    pub sora_asset_id: H256,
    pub decimals: u8,
    pub name: [u8; 32],
    pub symbol: [u8; 32],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct SccpTokenControlProjectionV1 {
    pub version: u8,
    pub target_domain: u32,
    pub nonce: u64,
    pub sora_asset_id: H256,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub enum SccpPayloadProjectionV1 {
    AssetRegister(SccpAssetRegisterProjectionV1),
    RouteActivate(SccpRouteActivateProjectionV1),
    Transfer(SccpTransferProjectionV1),
    TokenAdd(SccpTokenAddProjectionV1),
    TokenPause(SccpTokenControlProjectionV1),
    TokenResume(SccpTokenControlProjectionV1),
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct SccpSubmissionArgumentV1 {
    pub key: String,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct SccpCounterpartySubmissionTemplateV1 {
    pub version: u8,
    pub encoding: String,
    pub submission_kind: String,
    pub verifier_entrypoint: String,
    pub required_arguments: Vec<SccpSubmissionArgumentV1>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct SccpCounterpartyProofJobV1 {
    pub version: u8,
    pub chain_family: SccpTransparentChainFamilyV1,
    pub chain: String,
    pub local_domain: u32,
    pub counterparty_domain: u32,
    pub security_model: SccpProofSecurityModelV1,
    pub anchor_governance: SccpAnchorGovernanceV1,
    pub destination_binding: SccpDestinationBindingV1,
    pub proof_family: String,
    pub verifier_backend: SccpVerifierBackendV1,
    pub message_backend: String,
    pub registry_backend: String,
    pub manifest_seed: String,
    pub finality_model: SccpProofFinalityModelV1,
    pub verifier_target: SccpProofVerifierTargetV1,
    pub public_inputs: SccpMessageTransparentPublicInputsV1,
    pub payload_kind: String,
    pub payload_projection: SccpPayloadProjectionV1,
    pub submission_template: SccpCounterpartySubmissionTemplateV1,
    pub submission_package: SccpCounterpartySubmissionPackageV1,
    pub bundle: NexusSccpMessageProofV1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub enum SccpProofFinalityModelV1 {
    EthereumBeaconExecution,
    BscValidatorSet,
    SolanaFinalizedSlot,
    TonMasterchain,
    TronDpos,
    SubstrateGrandpa,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub enum SccpProofVerifierTargetV1 {
    EvmContract,
    SolanaProgram,
    TonContract,
    TronContract,
    SubstrateRuntime,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub enum SccpProofSecurityModelV1 {
    RecursiveZk,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub enum SccpAnchorGovernanceV1 {
    CryptographicProof,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub enum SccpLaunchModeV1 {
    #[default]
    AllLanesAtOnce,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub enum SccpProofSubmitterPolicyV1 {
    #[default]
    Permissionless,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub enum SccpRouteActivationPolicyV1 {
    #[default]
    GovernanceAllowlist,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub enum SccpSourceProofPlanV1 {
    #[default]
    Unknown,
    EthereumBeaconReceiptProof,
    BscValidatorSetReceiptProof,
    SolanaFinalizedTransactionProof,
    TonMasterchainShardProof,
    TronDposReceiptProof,
    SubstrateGrandpaEventProof,
}

macro_rules! impl_str_json_enum {
    ($ty:ty, $err:literal, { $($variant:path => $label:expr),+ $(,)? }) => {
        impl $ty {
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
                    norito::json::Error::Message(format!("{err}: `{value}`", err = $err))
                })
            }
        }
    };
}

impl_str_json_enum!(SccpLaunchModeV1, "unsupported SCCP launch mode", {
    SccpLaunchModeV1::AllLanesAtOnce => "AllLanesAtOnce",
});

impl_str_json_enum!(
    SccpProofSubmitterPolicyV1,
    "unsupported SCCP proof submitter policy",
    {
        SccpProofSubmitterPolicyV1::Permissionless => "Permissionless",
    }
);

impl_str_json_enum!(
    SccpRouteActivationPolicyV1,
    "unsupported SCCP route activation policy",
    {
        SccpRouteActivationPolicyV1::GovernanceAllowlist => "GovernanceAllowlist",
    }
);

impl_str_json_enum!(SccpSourceProofPlanV1, "unsupported SCCP source proof plan", {
    SccpSourceProofPlanV1::Unknown => "Unknown",
    SccpSourceProofPlanV1::EthereumBeaconReceiptProof => "EthereumBeaconReceiptProof",
    SccpSourceProofPlanV1::BscValidatorSetReceiptProof => "BscValidatorSetReceiptProof",
    SccpSourceProofPlanV1::SolanaFinalizedTransactionProof => "SolanaFinalizedTransactionProof",
    SccpSourceProofPlanV1::TonMasterchainShardProof => "TonMasterchainShardProof",
    SccpSourceProofPlanV1::TronDposReceiptProof => "TronDposReceiptProof",
    SccpSourceProofPlanV1::SubstrateGrandpaEventProof => "SubstrateGrandpaEventProof",
});

impl_str_json_enum!(
    SccpProofFinalityModelV1,
    "unsupported SCCP proof finality model",
    {
        SccpProofFinalityModelV1::EthereumBeaconExecution => "EthereumBeaconExecution",
        SccpProofFinalityModelV1::BscValidatorSet => "BscValidatorSet",
        SccpProofFinalityModelV1::SolanaFinalizedSlot => "SolanaFinalizedSlot",
        SccpProofFinalityModelV1::TonMasterchain => "TonMasterchain",
        SccpProofFinalityModelV1::TronDpos => "TronDpos",
        SccpProofFinalityModelV1::SubstrateGrandpa => "SubstrateGrandpa",
    }
);

#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::JsonSerialize, norito::derive::JsonDeserialize,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct SccpProductionPolicyV1 {
    pub version: u8,
    pub launch_mode: SccpLaunchModeV1,
    pub proof_submitter_policy: SccpProofSubmitterPolicyV1,
    pub route_activation_policy: SccpRouteActivationPolicyV1,
    pub per_message_human_approval_required: bool,
}

impl Default for SccpProductionPolicyV1 {
    fn default() -> Self {
        Self {
            version: 1,
            launch_mode: SccpLaunchModeV1::AllLanesAtOnce,
            proof_submitter_policy: SccpProofSubmitterPolicyV1::Permissionless,
            route_activation_policy: SccpRouteActivationPolicyV1::GovernanceAllowlist,
            per_message_human_approval_required: false,
        }
    }
}

#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::JsonSerialize, norito::derive::JsonDeserialize,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct SccpSourceAdapterEngineReadinessV1 {
    pub version: u8,
    pub domain: u32,
    pub chain: String,
    pub source_proof_plan: SccpSourceProofPlanV1,
    pub finality_model: SccpProofFinalityModelV1,
    pub adapter_proof_family: String,
    pub adapter_circuit_id: String,
    #[cfg_attr(feature = "serde", serde(default))]
    #[norito(default)]
    pub source_verifier_material: SccpSourceVerifierMaterialV1,
    pub adapter_statement_binding_ready: bool,
    pub adapter_open_verify_ready: bool,
    pub finality_model_ready: bool,
    pub external_consensus_verifier_ready: bool,
    pub external_message_inclusion_verifier_ready: bool,
    pub source_trust_anchor_ready: bool,
    pub production_ready: bool,
    #[cfg_attr(feature = "serde", serde(default))]
    #[norito(default)]
    pub blockers: Vec<String>,
}

impl Default for SccpSourceAdapterEngineReadinessV1 {
    fn default() -> Self {
        Self {
            version: 1,
            domain: SCCP_DOMAIN_SORA,
            chain: "sora".to_owned(),
            source_proof_plan: SccpSourceProofPlanV1::Unknown,
            finality_model: SccpProofFinalityModelV1::EthereumBeaconExecution,
            adapter_proof_family: String::new(),
            adapter_circuit_id: String::new(),
            source_verifier_material: SccpSourceVerifierMaterialV1::default(),
            adapter_statement_binding_ready: false,
            adapter_open_verify_ready: false,
            finality_model_ready: false,
            external_consensus_verifier_ready: false,
            external_message_inclusion_verifier_ready: false,
            source_trust_anchor_ready: false,
            production_ready: false,
            blockers: Vec::new(),
        }
    }
}

#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::JsonSerialize, norito::derive::JsonDeserialize,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct SccpLaneProductionReadinessV1 {
    pub version: u8,
    pub domain: u32,
    pub chain: String,
    pub source_proof_plan: SccpSourceProofPlanV1,
    pub destination_verifier_plan: SccpDestinationVerifierPlanV1,
    pub verifier_backend: SccpVerifierBackendV1,
    pub source_adapter_engine: SccpSourceAdapterEngineReadinessV1,
    pub source_adapter_ready: bool,
    pub immutable_verifier_ready: bool,
    pub anchors_ready: bool,
    pub routes_allowlisted: bool,
    pub permissionless_submission: bool,
    pub production_ready: bool,
    #[cfg_attr(feature = "serde", serde(default))]
    #[norito(default)]
    pub blockers: Vec<String>,
}

impl Default for SccpLaneProductionReadinessV1 {
    fn default() -> Self {
        Self {
            version: 1,
            domain: SCCP_DOMAIN_SORA,
            chain: "sora".to_owned(),
            source_proof_plan: SccpSourceProofPlanV1::Unknown,
            destination_verifier_plan: SccpDestinationVerifierPlanV1::Unknown,
            verifier_backend: SccpVerifierBackendV1 {
                version: 1,
                family: SccpVerifierBackendFamilyV1::Unknown,
                key: String::new(),
            },
            source_adapter_engine: SccpSourceAdapterEngineReadinessV1::default(),
            source_adapter_ready: false,
            immutable_verifier_ready: false,
            anchors_ready: false,
            routes_allowlisted: false,
            permissionless_submission: true,
            production_ready: false,
            blockers: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub enum SccpDestinationVerifierPlanV1 {
    #[default]
    Unknown,
    EvmGroth16Bn254Adapter,
    SolanaProgramNativeRecursive,
    TonContractNativeRecursive,
    TronContractNativeRecursive,
    SubstrateRuntimeNativeRecursive,
}

impl SccpDestinationVerifierPlanV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "Unknown",
            Self::EvmGroth16Bn254Adapter => "EvmGroth16Bn254Adapter",
            Self::SolanaProgramNativeRecursive => "SolanaProgramNativeRecursive",
            Self::TonContractNativeRecursive => "TonContractNativeRecursive",
            Self::TronContractNativeRecursive => "TronContractNativeRecursive",
            Self::SubstrateRuntimeNativeRecursive => "SubstrateRuntimeNativeRecursive",
        }
    }
}

impl core::str::FromStr for SccpDestinationVerifierPlanV1 {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "Unknown" => Ok(Self::Unknown),
            "EvmGroth16Bn254Adapter" => Ok(Self::EvmGroth16Bn254Adapter),
            "SolanaProgramNativeRecursive" => Ok(Self::SolanaProgramNativeRecursive),
            "TonContractNativeRecursive" => Ok(Self::TonContractNativeRecursive),
            "TronContractNativeRecursive" => Ok(Self::TronContractNativeRecursive),
            "SubstrateRuntimeNativeRecursive" => Ok(Self::SubstrateRuntimeNativeRecursive),
            _ => Err("unsupported SCCP destination verifier plan"),
        }
    }
}

impl norito::json::FastJsonWrite for SccpDestinationVerifierPlanV1 {
    fn write_json(&self, out: &mut String) {
        norito::json::write_json_string(self.as_str(), out);
    }
}

impl norito::json::JsonDeserialize for SccpDestinationVerifierPlanV1 {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        value.parse().map_err(|_| {
            norito::json::Error::Message(format!(
                "unsupported SCCP destination verifier plan `{value}`"
            ))
        })
    }
}

#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::JsonSerialize, norito::derive::JsonDeserialize,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct SccpDestinationRolloutV1 {
    pub version: u8,
    pub domain: u32,
    pub chain: String,
    pub verifier_plan: SccpDestinationVerifierPlanV1,
    pub immutable_verifier_ready: bool,
    pub anchors_ready: bool,
    #[cfg_attr(feature = "serde", serde(default))]
    #[norito(default)]
    pub verifier_identity: Option<String>,
    #[cfg_attr(feature = "serde", serde(default))]
    #[norito(default)]
    pub verifier_code_hash: Option<String>,
    #[cfg_attr(feature = "serde", serde(default))]
    #[norito(default)]
    pub anchor_id: Option<String>,
    #[cfg_attr(feature = "serde", serde(default))]
    #[norito(default)]
    pub blockers: Vec<String>,
}

impl Default for SccpDestinationRolloutV1 {
    fn default() -> Self {
        Self {
            version: 1,
            domain: SCCP_DOMAIN_SORA,
            chain: "sora".to_owned(),
            verifier_plan: SccpDestinationVerifierPlanV1::Unknown,
            immutable_verifier_ready: false,
            anchors_ready: false,
            verifier_identity: None,
            verifier_code_hash: None,
            anchor_id: None,
            blockers: Vec::new(),
        }
    }
}

#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::JsonSerialize, norito::derive::JsonDeserialize,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct SccpDestinationBindingV1 {
    pub version: u8,
    pub key: String,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub binding_hash: H256,
}

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
#[norito(tag = "family", content = "detail", rename_all = "snake_case")]
pub enum SccpVerifierBackendFamilyV1 {
    EvmSecp256k1Keccak,
    SolanaProgram,
    TonContract,
    TronStarkFri,
    SubstrateRuntime,
    EvmGroth16Bn254,
    TronGroth16Bn254,
    Unknown,
}

#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::JsonSerialize, norito::derive::JsonDeserialize,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct SccpVerifierBackendV1 {
    pub version: u8,
    pub family: SccpVerifierBackendFamilyV1,
    pub key: String,
}

#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::JsonSerialize, norito::derive::JsonDeserialize,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct SccpSubmissionArgumentValueV1 {
    pub key: String,
    pub encoding: String,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub bytes: Vec<u8>,
}

#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::JsonSerialize, norito::derive::JsonDeserialize,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct SccpEvmWordPublicInputsV1 {
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub message_id: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub payload_hash: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub target_domain_word: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub commitment_root: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub finality_height_word: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub finality_block_hash: H256,
}

#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::JsonSerialize, norito::derive::JsonDeserialize,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct SccpEvmAttestationSignatureV1 {
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub signer_address: Vec<u8>,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub signature_bytes: Vec<u8>,
}

#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::JsonSerialize, norito::derive::JsonDeserialize,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct SccpEvmAttestationEnvelopeV1 {
    pub version: u8,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub message_id: H256,
    pub source_domain: u32,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub commitment_root: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub native_proof_hash: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub destination_binding_hash: H256,
    pub signatures: Vec<SccpEvmAttestationSignatureV1>,
}

#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::JsonSerialize, norito::derive::JsonDeserialize,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct SccpEvmContractSubmissionPayloadV1 {
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub proof_bytes: Vec<u8>,
    pub public_inputs: SccpEvmWordPublicInputsV1,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub public_inputs_hash: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub statement_hash: H256,
    pub destination_binding: SccpDestinationBindingV1,
    pub attestation: SccpEvmAttestationEnvelopeV1,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SccpEvmGroth16Bn254ProofV1 {
    pub version: u8,
    pub message_id: H256,
    pub source_domain: u32,
    pub commitment_root: H256,
    pub a: [H256; 2],
    pub b: [H256; 4],
    pub c: [H256; 2],
}

#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::JsonSerialize, norito::derive::JsonDeserialize,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct SccpEvmGroth16ContractSubmissionPayloadV1 {
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub proof_bytes: Vec<u8>,
    pub public_inputs: SccpEvmWordPublicInputsV1,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub statement_hash: H256,
    pub destination_binding: SccpDestinationBindingV1,
}

#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::JsonSerialize, norito::derive::JsonDeserialize,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct SccpTronContractSubmissionPayloadV1 {
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub proof_bytes: Vec<u8>,
    pub public_inputs: SccpEvmWordPublicInputsV1,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub statement_hash: H256,
    pub destination_binding: SccpDestinationBindingV1,
}

#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::JsonSerialize, norito::derive::JsonDeserialize,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
#[allow(clippy::struct_field_names)]
pub struct SccpSolanaProgramSubmissionPayloadV1 {
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub proof_bytes: Vec<u8>,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub public_inputs_bytes: Vec<u8>,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub bundle_bytes: Vec<u8>,
}

#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::JsonSerialize, norito::derive::JsonDeserialize,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
#[allow(clippy::struct_field_names)]
pub struct SccpTonInternalMessageSubmissionPayloadV1 {
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub message_body_boc: Vec<u8>,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::u64_string"))]
    pub query_id: u64,
    pub destination_binding: SccpDestinationBindingV1,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub destination_binding_hash: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub proof_bytes: Vec<u8>,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub public_inputs_bytes: Vec<u8>,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub bundle_bytes: Vec<u8>,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub statement_hash: H256,
}

#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::JsonSerialize, norito::derive::JsonDeserialize,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
#[allow(clippy::struct_field_names)]
pub struct SccpSubstrateRuntimeSubmissionPayloadV1 {
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub proof_bytes: Vec<u8>,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub public_inputs_bytes: Vec<u8>,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub bundle_bytes: Vec<u8>,
}

#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::JsonSerialize, norito::derive::JsonDeserialize,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
#[norito(tag = "platform", content = "payload", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
pub enum SccpPlatformSubmissionPayloadV1 {
    EvmContractCall(SccpEvmContractSubmissionPayloadV1),
    EvmGroth16ContractCall(SccpEvmGroth16ContractSubmissionPayloadV1),
    SolanaProgramInstruction(SccpSolanaProgramSubmissionPayloadV1),
    TonInternalMessage(SccpTonInternalMessageSubmissionPayloadV1),
    TronContractCall(SccpTronContractSubmissionPayloadV1),
    SubstrateRuntimeCall(SccpSubstrateRuntimeSubmissionPayloadV1),
}

#[derive(
    Clone, Debug, PartialEq, Eq, norito::derive::JsonSerialize, norito::derive::JsonDeserialize,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct SccpCounterpartySubmissionPackageV1 {
    pub version: u8,
    pub proof_family: String,
    pub verifier_backend: SccpVerifierBackendV1,
    pub envelope_encoding: String,
    pub submission_kind: String,
    pub verifier_entrypoint: String,
    pub platform_payload: SccpPlatformSubmissionPayloadV1,
    pub arguments: Vec<SccpSubmissionArgumentValueV1>,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub envelope_bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)]
pub struct SccpProofManifestV1 {
    pub version: u8,
    pub local_domain: u32,
    pub local_chain: String,
    pub counterparty_domain: u32,
    pub chain: String,
    pub security_model: SccpProofSecurityModelV1,
    pub anchor_governance: SccpAnchorGovernanceV1,
    pub destination_binding: SccpDestinationBindingV1,
    pub proof_family: String,
    pub verifier_backend: SccpVerifierBackendV1,
    pub message_backend: String,
    pub registry_backend: String,
    pub counterparty_account_codec: u8,
    pub counterparty_account_codec_key: String,
    pub finality_model: SccpProofFinalityModelV1,
    pub verifier_target: SccpProofVerifierTargetV1,
    pub manifest_seed: String,
    pub required_public_inputs: Vec<String>,
    pub message_payload_kinds: Vec<String>,
    #[cfg_attr(feature = "serde", serde(default))]
    #[norito(default)]
    pub destination_rollout: SccpDestinationRolloutV1,
    pub production_ready: bool,
    #[cfg_attr(feature = "serde", serde(default))]
    #[norito(default)]
    pub disabled_reason: Option<String>,
    pub submission_template: SccpCounterpartySubmissionTemplateV1,
}

pub fn is_supported_domain(domain_id: u32) -> bool {
    matches!(
        domain_id,
        SCCP_DOMAIN_SORA
            | SCCP_DOMAIN_ETH
            | SCCP_DOMAIN_BSC
            | SCCP_DOMAIN_SOL
            | SCCP_DOMAIN_TON
            | SCCP_DOMAIN_TRON
            | SCCP_DOMAIN_SORA_KUSAMA
            | SCCP_DOMAIN_SORA_POLKADOT
            | SCCP_DOMAIN_SORA2
    )
}

pub fn is_supported_codec(codec_id: u8) -> bool {
    matches!(
        codec_id,
        SCCP_CODEC_TEXT_UTF8
            | SCCP_CODEC_EVM_HEX
            | SCCP_CODEC_SOLANA_BASE58
            | SCCP_CODEC_TON_RAW
            | SCCP_CODEC_TRON_BASE58CHECK
            | SCCP_CODEC_SORA_ASSET_ID
    )
}

pub fn sccp_codec_key(codec_id: u8) -> Option<&'static str> {
    match codec_id {
        SCCP_CODEC_TEXT_UTF8 => Some("text_utf8"),
        SCCP_CODEC_EVM_HEX => Some("evm_hex"),
        SCCP_CODEC_SOLANA_BASE58 => Some("solana_base58"),
        SCCP_CODEC_TON_RAW => Some("ton_raw"),
        SCCP_CODEC_TRON_BASE58CHECK => Some("tron_base58check"),
        SCCP_CODEC_SORA_ASSET_ID => Some("sora_asset_id"),
        _ => None,
    }
}

pub fn sccp_codec_description(codec_id: u8) -> Option<&'static str> {
    match codec_id {
        SCCP_CODEC_TEXT_UTF8 => Some("Logical UTF-8 identifiers for SORA and route-local names."),
        SCCP_CODEC_EVM_HEX => Some("0x-prefixed canonical EIP-55 EVM account addresses."),
        SCCP_CODEC_SOLANA_BASE58 => Some("Base58 Solana public keys."),
        SCCP_CODEC_TON_RAW => Some("Canonical TON raw addresses in workchain:account_hex form."),
        SCCP_CODEC_TRON_BASE58CHECK => Some("Tron base58check account addresses."),
        SCCP_CODEC_SORA_ASSET_ID => Some("Raw 32-byte SORA asset identifiers."),
        _ => None,
    }
}

pub fn sccp_chain_key_for_domain(domain: u32) -> Option<&'static str> {
    match domain {
        SCCP_DOMAIN_SORA => Some("sora"),
        SCCP_DOMAIN_ETH => Some("eth"),
        SCCP_DOMAIN_BSC => Some("bsc"),
        SCCP_DOMAIN_SOL => Some("sol"),
        SCCP_DOMAIN_TON => Some("ton"),
        SCCP_DOMAIN_TRON => Some("tron"),
        SCCP_DOMAIN_SORA_KUSAMA => Some("sora-kusama"),
        SCCP_DOMAIN_SORA_POLKADOT => Some("sora-polkadot"),
        SCCP_DOMAIN_SORA2 => Some("sora2"),
        _ => None,
    }
}

pub fn sccp_counterparty_account_codec(domain: u32) -> Option<u8> {
    match domain {
        SCCP_DOMAIN_SORA
        | SCCP_DOMAIN_SORA_KUSAMA
        | SCCP_DOMAIN_SORA_POLKADOT
        | SCCP_DOMAIN_SORA2 => Some(SCCP_CODEC_TEXT_UTF8),
        SCCP_DOMAIN_ETH | SCCP_DOMAIN_BSC => Some(SCCP_CODEC_EVM_HEX),
        SCCP_DOMAIN_SOL => Some(SCCP_CODEC_SOLANA_BASE58),
        SCCP_DOMAIN_TON => Some(SCCP_CODEC_TON_RAW),
        SCCP_DOMAIN_TRON => Some(SCCP_CODEC_TRON_BASE58CHECK),
        _ => None,
    }
}

pub fn sccp_counterparty_domain(primary: u32, secondary: u32) -> Option<u32> {
    if primary != SCCP_DOMAIN_SORA {
        return Some(primary);
    }
    if secondary != SCCP_DOMAIN_SORA {
        return Some(secondary);
    }
    None
}

pub fn sccp_counterparty_domain_for_message_payload(payload: &SccpPayloadV1) -> Option<u32> {
    match payload {
        SccpPayloadV1::AssetRegister(payload) => {
            sccp_counterparty_domain(payload.target_domain, payload.home_domain)
        }
        SccpPayloadV1::RouteActivate(payload) => {
            sccp_counterparty_domain(payload.target_domain, payload.source_domain)
        }
        SccpPayloadV1::Transfer(payload) => {
            sccp_counterparty_domain(payload.dest_domain, payload.source_domain)
        }
        SccpPayloadV1::TokenAdd(payload) => Some(payload.target_domain),
        SccpPayloadV1::TokenPause(payload) | SccpPayloadV1::TokenResume(payload) => {
            Some(payload.target_domain)
        }
    }
}

pub fn sccp_counterparty_domain_from_backend(backend: &str) -> Option<u32> {
    SCCP_CORE_REMOTE_DOMAINS.into_iter().find(|domain| {
        let Some(manifest) = sccp_proof_manifest_for_domain(*domain) else {
            return false;
        };
        backend == manifest.message_backend || backend == manifest.registry_backend
    })
}

pub fn sccp_proof_finality_model_for_domain(domain: u32) -> Option<SccpProofFinalityModelV1> {
    match domain {
        SCCP_DOMAIN_ETH => Some(SccpProofFinalityModelV1::EthereumBeaconExecution),
        SCCP_DOMAIN_BSC => Some(SccpProofFinalityModelV1::BscValidatorSet),
        SCCP_DOMAIN_SOL => Some(SccpProofFinalityModelV1::SolanaFinalizedSlot),
        SCCP_DOMAIN_TON => Some(SccpProofFinalityModelV1::TonMasterchain),
        SCCP_DOMAIN_TRON => Some(SccpProofFinalityModelV1::TronDpos),
        SCCP_DOMAIN_SORA_KUSAMA | SCCP_DOMAIN_SORA_POLKADOT | SCCP_DOMAIN_SORA2 => {
            Some(SccpProofFinalityModelV1::SubstrateGrandpa)
        }
        _ => None,
    }
}

fn sccp_proof_verifier_target_for_domain(domain: u32) -> Option<SccpProofVerifierTargetV1> {
    match domain {
        SCCP_DOMAIN_ETH | SCCP_DOMAIN_BSC => Some(SccpProofVerifierTargetV1::EvmContract),
        SCCP_DOMAIN_SOL => Some(SccpProofVerifierTargetV1::SolanaProgram),
        SCCP_DOMAIN_TON => Some(SccpProofVerifierTargetV1::TonContract),
        SCCP_DOMAIN_TRON => Some(SccpProofVerifierTargetV1::TronContract),
        SCCP_DOMAIN_SORA_KUSAMA | SCCP_DOMAIN_SORA_POLKADOT | SCCP_DOMAIN_SORA2 => {
            Some(SccpProofVerifierTargetV1::SubstrateRuntime)
        }
        _ => None,
    }
}

fn sccp_verifier_backend_family_for_domain(domain: u32) -> Option<SccpVerifierBackendFamilyV1> {
    match domain {
        SCCP_DOMAIN_ETH | SCCP_DOMAIN_BSC => Some(SccpVerifierBackendFamilyV1::EvmGroth16Bn254),
        SCCP_DOMAIN_SOL => Some(SccpVerifierBackendFamilyV1::SolanaProgram),
        SCCP_DOMAIN_TON => Some(SccpVerifierBackendFamilyV1::TonContract),
        SCCP_DOMAIN_TRON => Some(SccpVerifierBackendFamilyV1::TronGroth16Bn254),
        SCCP_DOMAIN_SORA_KUSAMA | SCCP_DOMAIN_SORA_POLKADOT | SCCP_DOMAIN_SORA2 => {
            Some(SccpVerifierBackendFamilyV1::SubstrateRuntime)
        }
        _ => None,
    }
}

fn sccp_verifier_backend_key_for_domain(domain: u32) -> Option<&'static str> {
    match domain {
        SCCP_DOMAIN_ETH | SCCP_DOMAIN_BSC => Some(SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1),
        SCCP_DOMAIN_SOL => Some("solana-program-v1"),
        SCCP_DOMAIN_TON => Some("ton-contract-v1"),
        SCCP_DOMAIN_TRON => Some(SCCP_TRON_GROTH16_BN254_PROOF_BACKEND_V1),
        SCCP_DOMAIN_SORA_KUSAMA | SCCP_DOMAIN_SORA_POLKADOT | SCCP_DOMAIN_SORA2 => {
            Some("substrate-runtime-v1")
        }
        _ => None,
    }
}

pub fn sccp_verifier_backend_for_domain(domain: u32) -> Option<SccpVerifierBackendV1> {
    Some(SccpVerifierBackendV1 {
        version: 1,
        family: sccp_verifier_backend_family_for_domain(domain)?,
        key: sccp_verifier_backend_key_for_domain(domain)?.to_owned(),
    })
}

pub fn sccp_message_backend_for_domain(domain: u32) -> Option<String> {
    let chain = sccp_chain_key_for_domain(domain)?;
    Some(format!("sccp/{SCCP_STARK_FRI_PROOF_FAMILY_V1}/{chain}"))
}

pub fn sccp_registry_backend_for_domain(domain: u32) -> Option<String> {
    let chain = sccp_chain_key_for_domain(domain)?;
    Some(format!(
        "bridge/sccp/{SCCP_STARK_FRI_PROOF_FAMILY_V1}/{chain}"
    ))
}

pub fn sccp_manifest_seed_for_domain(domain: u32) -> Option<String> {
    let chain = sccp_chain_key_for_domain(domain)?;
    Some(format!(
        "iroha:sccp:bridge-proof:message:stark-fri:v1:{chain}"
    ))
}

/// Compute the bridge proof manifest hash used by SCCP proof submissions.
pub fn sccp_bridge_manifest_hash_for_seed(seed: &str) -> H256 {
    <[u8; 32]>::from(iroha_crypto::Hash::new(seed.as_bytes()))
}

/// Return the reserved manifest hash for SCCP burn bundle bridge proofs.
pub fn sccp_burn_bridge_manifest_hash_v1() -> H256 {
    sccp_bridge_manifest_hash_for_seed(SCCP_BURN_BRIDGE_PROOF_MANIFEST_SEED_V1)
}

/// Return all manifest hashes reserved for typed SCCP bridge proof submissions.
pub fn sccp_reserved_bridge_manifest_hashes_v1() -> Vec<H256> {
    let mut hashes = vec![sccp_burn_bridge_manifest_hash_v1()];
    hashes.extend(
        sccp_proof_manifests_v1()
            .into_iter()
            .map(|manifest| sccp_bridge_manifest_hash_for_seed(&manifest.manifest_seed)),
    );
    hashes
}

pub fn sccp_required_public_inputs_v1() -> Vec<String> {
    vec![
        "message_id".to_owned(),
        "payload_hash".to_owned(),
        "target_domain".to_owned(),
        "commitment_root".to_owned(),
        "finality_height".to_owned(),
        "finality_block_hash".to_owned(),
    ]
}

pub fn sccp_proof_security_model_v1() -> SccpProofSecurityModelV1 {
    SccpProofSecurityModelV1::RecursiveZk
}

pub fn sccp_anchor_governance_v1() -> SccpAnchorGovernanceV1 {
    SccpAnchorGovernanceV1::CryptographicProof
}

fn sccp_destination_verifier_plan_for_domain(domain: u32) -> Option<SccpDestinationVerifierPlanV1> {
    match domain {
        SCCP_DOMAIN_ETH | SCCP_DOMAIN_BSC => {
            Some(SccpDestinationVerifierPlanV1::EvmGroth16Bn254Adapter)
        }
        SCCP_DOMAIN_SOL => Some(SccpDestinationVerifierPlanV1::SolanaProgramNativeRecursive),
        SCCP_DOMAIN_TON => Some(SccpDestinationVerifierPlanV1::TonContractNativeRecursive),
        SCCP_DOMAIN_TRON => Some(SccpDestinationVerifierPlanV1::TronContractNativeRecursive),
        SCCP_DOMAIN_SORA_KUSAMA | SCCP_DOMAIN_SORA_POLKADOT | SCCP_DOMAIN_SORA2 => {
            Some(SccpDestinationVerifierPlanV1::SubstrateRuntimeNativeRecursive)
        }
        _ => None,
    }
}

fn sccp_destination_rollout_blockers_for_domain(domain: u32) -> Option<Vec<String>> {
    let blockers = match domain {
        SCCP_DOMAIN_ETH | SCCP_DOMAIN_BSC => vec![
            "immutable EVM verifier contract is not deployed for this SCCP lane".to_owned(),
            "cryptographic trust anchor is not active for this SCCP lane".to_owned(),
            "Groth16/bn254 adapter proof submission is not wired into the SCCP relayer path"
                .to_owned(),
        ],
        SCCP_DOMAIN_SOL => vec![
            "immutable Solana verifier program is not deployed for this SCCP lane".to_owned(),
            "cryptographic trust anchor is not active for this SCCP lane".to_owned(),
            "native recursive verifier program submission is not wired into the SCCP relayer path"
                .to_owned(),
        ],
        SCCP_DOMAIN_TON => vec![
            "immutable TON verifier contract is not deployed for this SCCP lane".to_owned(),
            "cryptographic trust anchor is not active for this SCCP lane".to_owned(),
            "native recursive verifier contract submission is not wired into the SCCP relayer path"
                .to_owned(),
        ],
        SCCP_DOMAIN_TRON => vec![
            "immutable TRON verifier contract is not deployed for this SCCP lane".to_owned(),
            "cryptographic trust anchor is not active for this SCCP lane".to_owned(),
            "native recursive verifier contract submission is not wired into the SCCP relayer path"
                .to_owned(),
        ],
        SCCP_DOMAIN_SORA_KUSAMA | SCCP_DOMAIN_SORA_POLKADOT | SCCP_DOMAIN_SORA2 => vec![
            "immutable Substrate runtime verifier call is not deployed for this SCCP lane"
                .to_owned(),
            "cryptographic trust anchor is not active for this SCCP lane".to_owned(),
            "native recursive runtime-call submission is not wired into the SCCP relayer path"
                .to_owned(),
        ],
        _ => return None,
    };
    Some(blockers)
}

pub fn sccp_destination_rollout_for_domain(domain: u32) -> Option<SccpDestinationRolloutV1> {
    Some(SccpDestinationRolloutV1 {
        version: 1,
        domain,
        chain: sccp_chain_key_for_domain(domain)?.to_owned(),
        verifier_plan: sccp_destination_verifier_plan_for_domain(domain)?,
        immutable_verifier_ready: false,
        anchors_ready: false,
        verifier_identity: None,
        verifier_code_hash: None,
        anchor_id: None,
        blockers: sccp_destination_rollout_blockers_for_domain(domain)?,
    })
}

fn hex32_string_is_nonzero(value: &str) -> bool {
    let raw = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value)
        .as_bytes();
    if raw.len() != 64 {
        return false;
    }

    let mut has_nonzero = false;
    for byte in raw {
        let Some(value) = (match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            b'A'..=b'F' => Some(byte - b'A' + 10),
            _ => None,
        }) else {
            return false;
        };
        has_nonzero |= value != 0;
    }
    has_nonzero
}

fn non_empty_metadata(value: Option<&str>) -> bool {
    value.is_some_and(|value| !value.trim().is_empty())
}

/// Return whether destination verifier deployment material is sufficient for production.
pub fn sccp_destination_rollout_is_production_ready(
    domain: u32,
    rollout: &SccpDestinationRolloutV1,
) -> bool {
    let Some(expected_plan) = sccp_destination_verifier_plan_for_domain(domain) else {
        return false;
    };
    let Some(expected_chain) = sccp_chain_key_for_domain(domain) else {
        return false;
    };

    rollout.version == 1
        && rollout.domain == domain
        && rollout.chain == expected_chain
        && rollout.verifier_plan == expected_plan
        && rollout.immutable_verifier_ready
        && rollout.anchors_ready
        && rollout.blockers.is_empty()
        && non_empty_metadata(rollout.verifier_identity.as_deref())
        && rollout
            .verifier_code_hash
            .as_deref()
            .is_some_and(hex32_string_is_nonzero)
        && non_empty_metadata(rollout.anchor_id.as_deref())
}

pub fn sccp_production_policy_v1() -> SccpProductionPolicyV1 {
    SccpProductionPolicyV1::default()
}

pub fn sccp_source_proof_plan_for_domain(domain: u32) -> Option<SccpSourceProofPlanV1> {
    match domain {
        SCCP_DOMAIN_ETH => Some(SccpSourceProofPlanV1::EthereumBeaconReceiptProof),
        SCCP_DOMAIN_BSC => Some(SccpSourceProofPlanV1::BscValidatorSetReceiptProof),
        SCCP_DOMAIN_SOL => Some(SccpSourceProofPlanV1::SolanaFinalizedTransactionProof),
        SCCP_DOMAIN_TON => Some(SccpSourceProofPlanV1::TonMasterchainShardProof),
        SCCP_DOMAIN_TRON => Some(SccpSourceProofPlanV1::TronDposReceiptProof),
        SCCP_DOMAIN_SORA_KUSAMA | SCCP_DOMAIN_SORA_POLKADOT | SCCP_DOMAIN_SORA2 => {
            Some(SccpSourceProofPlanV1::SubstrateGrandpaEventProof)
        }
        _ => None,
    }
}

fn sccp_source_proof_blocker_for_domain(domain: u32) -> Option<&'static str> {
    match domain {
        SCCP_DOMAIN_ETH => Some(
            "Ethereum beacon finality plus execution receipt/log inclusion verifier is not wired into the SCCP inbound path",
        ),
        SCCP_DOMAIN_BSC => Some(
            "BSC validator-set finality plus receipt/log inclusion verifier is not wired into the SCCP inbound path",
        ),
        SCCP_DOMAIN_SOL => Some(
            "Solana finalized transaction/message verifier is not wired into the SCCP inbound path",
        ),
        SCCP_DOMAIN_TON => {
            Some("TON masterchain/shard proof verifier is not wired into the SCCP inbound path")
        }
        SCCP_DOMAIN_TRON => {
            Some("TRON solid-block receipt/log verifier is not wired into the SCCP inbound path")
        }
        SCCP_DOMAIN_SORA_KUSAMA | SCCP_DOMAIN_SORA_POLKADOT | SCCP_DOMAIN_SORA2 => Some(
            "Substrate GRANDPA event/storage proof verifier is not wired into the SCCP inbound path",
        ),
        _ => None,
    }
}

fn sccp_source_consensus_engine_blocker_for_domain(domain: u32) -> Option<&'static str> {
    match domain {
        SCCP_DOMAIN_ETH => Some(
            "Ethereum beacon sync-committee/header verifier is not deployed for SCCP source proofs",
        ),
        SCCP_DOMAIN_BSC => {
            Some("BSC validator-set/header-seal verifier is not deployed for SCCP source proofs")
        }
        SCCP_DOMAIN_SOL => {
            Some("Solana finalized-slot/status verifier is not deployed for SCCP source proofs")
        }
        SCCP_DOMAIN_TON => {
            Some("TON masterchain/shard verifier is not deployed for SCCP source proofs")
        }
        SCCP_DOMAIN_TRON => {
            Some("TRON DPoS solid-block verifier is not deployed for SCCP source proofs")
        }
        SCCP_DOMAIN_SORA_KUSAMA | SCCP_DOMAIN_SORA_POLKADOT | SCCP_DOMAIN_SORA2 => {
            Some("Substrate GRANDPA finality verifier is not deployed for SCCP source proofs")
        }
        _ => None,
    }
}

fn sccp_source_inclusion_engine_blocker_for_domain(domain: u32) -> Option<&'static str> {
    match domain {
        SCCP_DOMAIN_ETH | SCCP_DOMAIN_BSC => Some(
            "EVM receipt trie and SCCP log inclusion verifier is not deployed for SCCP source proofs",
        ),
        SCCP_DOMAIN_SOL => Some(
            "Solana transaction status/message inclusion verifier is not deployed for SCCP source proofs",
        ),
        SCCP_DOMAIN_TON => Some(
            "TON shard transaction/message inclusion verifier is not deployed for SCCP source proofs",
        ),
        SCCP_DOMAIN_TRON => {
            Some("TRON receipt/log inclusion verifier is not deployed for SCCP source proofs")
        }
        SCCP_DOMAIN_SORA_KUSAMA | SCCP_DOMAIN_SORA_POLKADOT | SCCP_DOMAIN_SORA2 => Some(
            "Substrate event/storage inclusion verifier is not deployed for SCCP source proofs",
        ),
        _ => None,
    }
}

fn sccp_source_trust_anchor_blocker_for_domain(domain: u32) -> Option<&'static str> {
    match domain {
        SCCP_DOMAIN_ETH => Some(
            "Ethereum finalized checkpoint and sync-committee trust anchor is not active for SCCP source proofs",
        ),
        SCCP_DOMAIN_BSC => {
            Some("BSC validator-set trust anchor is not active for SCCP source proofs")
        }
        SCCP_DOMAIN_SOL => {
            Some("Solana root/epoch trust anchor is not active for SCCP source proofs")
        }
        SCCP_DOMAIN_TON => {
            Some("TON masterchain trust anchor is not active for SCCP source proofs")
        }
        SCCP_DOMAIN_TRON => {
            Some("TRON witness-schedule trust anchor is not active for SCCP source proofs")
        }
        SCCP_DOMAIN_SORA_KUSAMA | SCCP_DOMAIN_SORA_POLKADOT | SCCP_DOMAIN_SORA2 => Some(
            "Substrate GRANDPA authority-set trust anchor is not active for SCCP source proofs",
        ),
        _ => None,
    }
}

pub fn sccp_source_adapter_engine_readiness_for_domain(
    domain: u32,
) -> Option<SccpSourceAdapterEngineReadinessV1> {
    let chain = sccp_chain_key_for_domain(domain)?.to_owned();
    let source_proof_plan = sccp_source_proof_plan_for_domain(domain)?;
    let finality_model = sccp_proof_finality_model_for_domain(domain)?;
    let source_verifier_material = sccp_source_verifier_material_for_domain(domain)?;
    let source_verifier_material_ready =
        sccp_source_verifier_material_is_production_ready(&source_verifier_material);
    let adapter_statement_binding_ready = true;
    let adapter_open_verify_ready = true;
    let finality_model_ready = true;
    let external_consensus_verifier_ready = source_verifier_material_ready;
    let external_message_inclusion_verifier_ready = source_verifier_material_ready;
    let source_trust_anchor_ready = source_verifier_material_ready;
    let production_ready = adapter_statement_binding_ready
        && adapter_open_verify_ready
        && finality_model_ready
        && external_consensus_verifier_ready
        && external_message_inclusion_verifier_ready
        && source_trust_anchor_ready;
    let mut blockers = Vec::new();
    if !external_consensus_verifier_ready {
        blockers.push(sccp_source_consensus_engine_blocker_for_domain(domain)?.to_owned());
    }
    if !external_message_inclusion_verifier_ready {
        blockers.push(sccp_source_inclusion_engine_blocker_for_domain(domain)?.to_owned());
    }
    if !source_trust_anchor_ready {
        blockers.push(sccp_source_trust_anchor_blocker_for_domain(domain)?.to_owned());
    }

    Some(SccpSourceAdapterEngineReadinessV1 {
        version: 1,
        domain,
        chain,
        source_proof_plan,
        finality_model,
        adapter_proof_family: SCCP_STARK_FRI_PROOF_FAMILY_V1.to_owned(),
        adapter_circuit_id: SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID_V1.to_owned(),
        source_verifier_material,
        adapter_statement_binding_ready,
        adapter_open_verify_ready,
        finality_model_ready,
        external_consensus_verifier_ready,
        external_message_inclusion_verifier_ready,
        source_trust_anchor_ready,
        production_ready,
        blockers,
    })
}

/// Return whether the source-chain consensus/inclusion adapter is production-ready.
pub fn sccp_source_adapter_ready_for_domain(domain: u32) -> bool {
    domain != SCCP_DOMAIN_SORA
        && sccp_source_adapter_engine_readiness_for_domain(domain)
            .is_some_and(|readiness| readiness.production_ready)
}

pub fn sccp_lane_production_readiness_for_domain(
    domain: u32,
) -> Option<SccpLaneProductionReadinessV1> {
    let chain = sccp_chain_key_for_domain(domain)?;
    let rollout = sccp_destination_rollout_for_domain(domain)?;
    let verifier_backend = sccp_verifier_backend_for_domain(domain)?;
    let source_adapter_engine = sccp_source_adapter_engine_readiness_for_domain(domain)?;
    // TODO: replace these hard-coded blockers with the on-chain SCCP lane
    // registry once deployment governance commits verifier identities, anchors,
    // and route allowlists for every advertised counterparty.
    let source_adapter_ready = source_adapter_engine.production_ready;
    let destination_rollout_ready = sccp_destination_rollout_is_production_ready(domain, &rollout);
    let routes_allowlisted = false;
    let permissionless_submission = matches!(
        sccp_production_policy_v1().proof_submitter_policy,
        SccpProofSubmitterPolicyV1::Permissionless
    );
    let production_ready = source_adapter_ready
        && destination_rollout_ready
        && routes_allowlisted
        && permissionless_submission;
    let mut blockers = Vec::new();
    if !source_adapter_ready {
        blockers.push(sccp_source_proof_blocker_for_domain(domain)?.to_owned());
        blockers.extend(source_adapter_engine.blockers.clone());
    }
    if !routes_allowlisted {
        blockers.push("production route allowlist is not anchored for this SCCP lane".to_owned());
    }
    if !destination_rollout_ready {
        blockers.push(
            "destination verifier rollout material is not production-ready for this SCCP lane"
                .to_owned(),
        );
    }
    blockers.extend(rollout.blockers.clone());
    blockers.push(
        "all-lanes-at-once launch policy blocks activation until every advertised SCCP counterparty is proof-complete".to_owned(),
    );

    Some(SccpLaneProductionReadinessV1 {
        version: 1,
        domain,
        chain: chain.to_owned(),
        source_proof_plan: sccp_source_proof_plan_for_domain(domain)?,
        destination_verifier_plan: rollout.verifier_plan,
        verifier_backend,
        source_adapter_engine,
        source_adapter_ready,
        immutable_verifier_ready: rollout.immutable_verifier_ready,
        anchors_ready: rollout.anchors_ready,
        routes_allowlisted,
        permissionless_submission,
        production_ready,
        blockers,
    })
}

pub fn sccp_all_lanes_launch_ready_v1() -> bool {
    SCCP_CORE_REMOTE_DOMAINS.into_iter().all(|domain| {
        sccp_lane_production_readiness_for_domain(domain)
            .is_some_and(|readiness| readiness.production_ready)
    })
}

fn sccp_lane_disabled_reason_for_plan(plan: SccpDestinationVerifierPlanV1) -> &'static str {
    match plan {
        SccpDestinationVerifierPlanV1::EvmGroth16Bn254Adapter => {
            "disabled until the immutable EVM Groth16/bn254 SCCP verifier and cryptographic trust anchors are live for this lane"
        }
        SccpDestinationVerifierPlanV1::SolanaProgramNativeRecursive => {
            "disabled until the immutable Solana recursive SCCP verifier and cryptographic trust anchors are live for this lane"
        }
        SccpDestinationVerifierPlanV1::TonContractNativeRecursive => {
            "disabled until the immutable TON recursive SCCP verifier and cryptographic trust anchors are live for this lane"
        }
        SccpDestinationVerifierPlanV1::TronContractNativeRecursive => {
            "disabled until the immutable TRON recursive SCCP verifier and cryptographic trust anchors are live for this lane"
        }
        SccpDestinationVerifierPlanV1::SubstrateRuntimeNativeRecursive => {
            "disabled until the immutable Substrate runtime SCCP verifier and cryptographic trust anchors are live for this lane"
        }
        SccpDestinationVerifierPlanV1::Unknown => SCCP_PRODUCTION_DISABLED_REASON_V1,
    }
}

pub fn sccp_destination_binding_key_for_domain(domain: u32) -> Option<String> {
    let chain = sccp_chain_key_for_domain(domain)?;
    let verifier_backend = sccp_verifier_backend_for_domain(domain)?;
    let verifier_target = sccp_proof_verifier_target_for_domain(domain)?;
    Some(format!(
        "sccp:{}:{}:{}:{}:{}",
        SCCP_DOMAIN_SORA,
        domain,
        chain,
        verifier_backend.key,
        sccp_proof_verifier_target_code(verifier_target)
    ))
}

fn canonical_sccp_destination_binding_bytes(domain: u32) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let verifier_backend = sccp_verifier_backend_for_domain(domain)?;
    let verifier_target = sccp_proof_verifier_target_for_domain(domain)?;
    let manifest_seed = sccp_manifest_seed_for_domain(domain)?;
    push_u8(&mut out, 1);
    push_u32(&mut out, SCCP_DOMAIN_SORA);
    push_u32(&mut out, domain);
    push_u8(
        &mut out,
        sccp_proof_security_model_code(sccp_proof_security_model_v1()),
    );
    push_u8(
        &mut out,
        sccp_anchor_governance_code(sccp_anchor_governance_v1()),
    );
    push_u8(&mut out, sccp_proof_verifier_target_code(verifier_target));
    push_u8(
        &mut out,
        sccp_verifier_backend_family_code(verifier_backend.family),
    );
    push_vec(
        &mut out,
        sccp_destination_binding_key_for_domain(domain)?.as_bytes(),
    );
    push_vec(&mut out, manifest_seed.as_bytes());
    push_vec(&mut out, SCCP_STARK_FRI_PROOF_FAMILY_V1.as_bytes());
    push_vec(&mut out, verifier_backend.key.as_bytes());
    Some(out)
}

pub fn sccp_destination_binding_for_domain(domain: u32) -> Option<SccpDestinationBindingV1> {
    Some(SccpDestinationBindingV1 {
        version: 1,
        key: sccp_destination_binding_key_for_domain(domain)?,
        binding_hash: prefixed_blake2b(
            SCCP_DESTINATION_BINDING_PREFIX_V1,
            &canonical_sccp_destination_binding_bytes(domain)?,
        ),
    })
}

pub const SCCP_PRODUCTION_DISABLED_REASON_V1: &str = "disabled until immutable destination verifiers validate recursive SCCP proofs under cryptographic trust anchors";

pub fn sccp_lane_production_ready_for_domain(domain: u32) -> bool {
    sccp_all_lanes_launch_ready_v1()
        && sccp_lane_production_readiness_for_domain(domain)
            .is_some_and(|readiness| readiness.production_ready)
}

pub fn sccp_lane_disabled_reason_for_domain(domain: u32) -> Option<&'static str> {
    sccp_destination_rollout_for_domain(domain)
        .map(|rollout| sccp_lane_disabled_reason_for_plan(rollout.verifier_plan))
        .filter(|_| !sccp_lane_production_ready_for_domain(domain))
}

pub fn sccp_manifest_is_production_ready(manifest: &SccpProofManifestV1) -> bool {
    manifest.production_ready
}

pub fn sccp_manifest_allows_transparent_proofs(
    manifest: &SccpProofManifestV1,
    allow_unready: bool,
) -> bool {
    sccp_manifest_is_production_ready(manifest) || allow_unready
}

pub fn sccp_message_payload_kind_keys_v1() -> Vec<String> {
    vec![
        "asset_register".to_owned(),
        "route_activate".to_owned(),
        "transfer".to_owned(),
        "token_add".to_owned(),
        "token_pause".to_owned(),
        "token_resume".to_owned(),
    ]
}

fn sccp_submission_arguments(keys: &[(&str, &str)]) -> Vec<SccpSubmissionArgumentV1> {
    keys.iter()
        .map(|(key, description)| SccpSubmissionArgumentV1 {
            key: (*key).to_owned(),
            description: (*description).to_owned(),
        })
        .collect()
}

#[allow(clippy::too_many_lines)]
pub fn sccp_submission_template_for_domain(
    domain: u32,
) -> Option<SccpCounterpartySubmissionTemplateV1> {
    match domain {
        SCCP_DOMAIN_ETH | SCCP_DOMAIN_BSC => Some(SccpCounterpartySubmissionTemplateV1 {
            version: 1,
            encoding: "abi_tuple_v1".to_owned(),
            submission_kind: "contract_call".to_owned(),
            verifier_entrypoint:
                "submitSccpMessageProof(bytes proof_bytes, bytes32[6] public_inputs, bytes32 statement_hash)"
                    .to_owned(),
            required_arguments: sccp_submission_arguments(&[
                (
                    "proof_bytes",
                    "EVM verifier proof bytes: Groth16/bn254 ABI tuple for production lanes or reference attestation envelope for reference-only tests.",
                ),
                (
                    "public_inputs",
                    "Fixed-width ABI words for the SCCP public inputs in manifest order.",
                ),
                (
                    "statement_hash",
                    "Canonical SCCP statement hash exposed as a bytes32 verifier input.",
                ),
            ]),
        }),
        SCCP_DOMAIN_SOL => Some(SccpCounterpartySubmissionTemplateV1 {
            version: 1,
            encoding: "borsh_instruction_v1".to_owned(),
            submission_kind: "program_instruction".to_owned(),
            verifier_entrypoint: "submit_sccp_message_proof".to_owned(),
            required_arguments: sccp_submission_arguments(&[
                (
                    "proof_bytes",
                    "Transparent SCCP proof bytes serialized into the instruction data.",
                ),
                (
                    "public_inputs",
                    "Borsh-encoded SCCP public inputs in manifest order.",
                ),
                (
                    "bundle_bytes",
                    "Borsh-encoded Nexus SCCP message bundle for the verifier program.",
                ),
            ]),
        }),
        SCCP_DOMAIN_TON => Some(SccpCounterpartySubmissionTemplateV1 {
            version: 1,
            encoding: "ton_message_body_boc_v1".to_owned(),
            submission_kind: "internal_message".to_owned(),
            verifier_entrypoint: "op::submit_sccp_message_proof".to_owned(),
            required_arguments: sccp_submission_arguments(&[
                (
                    "message_body_boc",
                    "TON Bag-of-Cells internal message body containing the SCCP submission root cell.",
                ),
            ]),
        }),
        SCCP_DOMAIN_TRON => Some(SccpCounterpartySubmissionTemplateV1 {
            version: 1,
            encoding: "tron_abi_tuple_v1".to_owned(),
            submission_kind: "contract_call".to_owned(),
            verifier_entrypoint:
                "submitSccpMessageProof(bytes proof_bytes, bytes32[6] public_inputs, bytes32 statement_hash)"
                    .to_owned(),
            required_arguments: sccp_submission_arguments(&[
                (
                    "proof_bytes",
                    "TRON TVM verifier proof bytes: Groth16/bn254 ABI tuple for production lanes.",
                ),
                (
                    "public_inputs",
                    "Fixed-width TVM ABI words for the SCCP public inputs in manifest order.",
                ),
                (
                    "statement_hash",
                    "Canonical SCCP statement hash exposed as a bytes32 verifier input.",
                ),
            ]),
        }),
        SCCP_DOMAIN_SORA_KUSAMA | SCCP_DOMAIN_SORA_POLKADOT | SCCP_DOMAIN_SORA2 => {
            Some(SccpCounterpartySubmissionTemplateV1 {
                version: 1,
                encoding: "scale_call_v1".to_owned(),
                submission_kind: "runtime_call".to_owned(),
                verifier_entrypoint: "SccpBridge.submit_message_proof".to_owned(),
                required_arguments: sccp_submission_arguments(&[
                    (
                        "proof_bytes",
                        "Transparent SCCP proof bytes emitted by the prover backend.",
                    ),
                    (
                        "public_inputs",
                        "SCALE-encoded SCCP public inputs in manifest order.",
                    ),
                    (
                        "bundle_bytes",
                        "SCALE-encoded Nexus SCCP message bundle for the runtime verifier.",
                    ),
                ]),
            })
        }
        _ => None,
    }
}

pub fn sccp_proof_manifest_for_domain(domain: u32) -> Option<SccpProofManifestV1> {
    let chain = sccp_chain_key_for_domain(domain)?;
    let counterparty_account_codec = sccp_counterparty_account_codec(domain)?;
    let counterparty_account_codec_key = sccp_codec_key(counterparty_account_codec)?;
    Some(SccpProofManifestV1 {
        version: 1,
        local_domain: SCCP_DOMAIN_SORA,
        local_chain: "sora".to_owned(),
        counterparty_domain: domain,
        chain: chain.to_owned(),
        security_model: sccp_proof_security_model_v1(),
        anchor_governance: sccp_anchor_governance_v1(),
        destination_binding: sccp_destination_binding_for_domain(domain)?,
        proof_family: SCCP_STARK_FRI_PROOF_FAMILY_V1.to_owned(),
        verifier_backend: sccp_verifier_backend_for_domain(domain)?,
        message_backend: sccp_message_backend_for_domain(domain)?,
        registry_backend: sccp_registry_backend_for_domain(domain)?,
        counterparty_account_codec,
        counterparty_account_codec_key: counterparty_account_codec_key.to_owned(),
        finality_model: sccp_proof_finality_model_for_domain(domain)?,
        verifier_target: sccp_proof_verifier_target_for_domain(domain)?,
        manifest_seed: sccp_manifest_seed_for_domain(domain)?,
        required_public_inputs: sccp_required_public_inputs_v1(),
        message_payload_kinds: sccp_message_payload_kind_keys_v1(),
        destination_rollout: sccp_destination_rollout_for_domain(domain)?,
        production_ready: sccp_lane_production_ready_for_domain(domain),
        disabled_reason: sccp_lane_disabled_reason_for_domain(domain).map(str::to_owned),
        submission_template: sccp_submission_template_for_domain(domain)?,
    })
}

pub fn sccp_proof_manifests_v1() -> Vec<SccpProofManifestV1> {
    SCCP_CORE_REMOTE_DOMAINS
        .iter()
        .copied()
        .filter_map(sccp_proof_manifest_for_domain)
        .collect()
}

fn sccp_proof_finality_model_code(model: SccpProofFinalityModelV1) -> u8 {
    match model {
        SccpProofFinalityModelV1::EthereumBeaconExecution => 1,
        SccpProofFinalityModelV1::BscValidatorSet => 2,
        SccpProofFinalityModelV1::SolanaFinalizedSlot => 3,
        SccpProofFinalityModelV1::TonMasterchain => 4,
        SccpProofFinalityModelV1::TronDpos => 5,
        SccpProofFinalityModelV1::SubstrateGrandpa => 6,
    }
}

fn sccp_source_proof_plan_code(plan: SccpSourceProofPlanV1) -> u8 {
    match plan {
        SccpSourceProofPlanV1::Unknown => 0,
        SccpSourceProofPlanV1::EthereumBeaconReceiptProof => 1,
        SccpSourceProofPlanV1::BscValidatorSetReceiptProof => 2,
        SccpSourceProofPlanV1::SolanaFinalizedTransactionProof => 3,
        SccpSourceProofPlanV1::TonMasterchainShardProof => 4,
        SccpSourceProofPlanV1::TronDposReceiptProof => 5,
        SccpSourceProofPlanV1::SubstrateGrandpaEventProof => 6,
    }
}

fn sccp_source_adapter_proof_code(proof: &SccpSourceAdapterProofV1) -> u8 {
    match proof {
        SccpSourceAdapterProofV1::EthereumBeaconReceipt(_) => 1,
        SccpSourceAdapterProofV1::BscValidatorSetReceipt(_) => 2,
        SccpSourceAdapterProofV1::SolanaFinalizedTransaction(_) => 3,
        SccpSourceAdapterProofV1::TonMasterchainShard(_) => 4,
        SccpSourceAdapterProofV1::TronDposReceipt(_) => 5,
        SccpSourceAdapterProofV1::SubstrateGrandpaEvent(_) => 6,
    }
}

fn sccp_proof_verifier_target_code(target: SccpProofVerifierTargetV1) -> u8 {
    match target {
        SccpProofVerifierTargetV1::EvmContract => 1,
        SccpProofVerifierTargetV1::SolanaProgram => 2,
        SccpProofVerifierTargetV1::TonContract => 3,
        SccpProofVerifierTargetV1::TronContract => 4,
        SccpProofVerifierTargetV1::SubstrateRuntime => 5,
    }
}

fn sccp_proof_security_model_code(model: SccpProofSecurityModelV1) -> u8 {
    match model {
        SccpProofSecurityModelV1::RecursiveZk => 1,
    }
}

fn sccp_anchor_governance_code(governance: SccpAnchorGovernanceV1) -> u8 {
    match governance {
        SccpAnchorGovernanceV1::CryptographicProof => 1,
    }
}

fn sccp_verifier_backend_family_code(family: SccpVerifierBackendFamilyV1) -> u8 {
    match family {
        SccpVerifierBackendFamilyV1::EvmSecp256k1Keccak => 1,
        SccpVerifierBackendFamilyV1::SolanaProgram => 2,
        SccpVerifierBackendFamilyV1::TonContract => 3,
        SccpVerifierBackendFamilyV1::TronStarkFri => 4,
        SccpVerifierBackendFamilyV1::SubstrateRuntime => 5,
        SccpVerifierBackendFamilyV1::EvmGroth16Bn254 => 6,
        SccpVerifierBackendFamilyV1::TronGroth16Bn254 => 7,
        SccpVerifierBackendFamilyV1::Unknown => 0,
    }
}

fn sccp_transparent_chain_family_for_domain(domain: u32) -> Option<SccpTransparentChainFamilyV1> {
    match domain {
        SCCP_DOMAIN_ETH | SCCP_DOMAIN_BSC => Some(SccpTransparentChainFamilyV1::Evm),
        SCCP_DOMAIN_SOL => Some(SccpTransparentChainFamilyV1::Solana),
        SCCP_DOMAIN_TON => Some(SccpTransparentChainFamilyV1::Ton),
        SCCP_DOMAIN_TRON => Some(SccpTransparentChainFamilyV1::Tron),
        SCCP_DOMAIN_SORA_KUSAMA | SCCP_DOMAIN_SORA_POLKADOT | SCCP_DOMAIN_SORA2 => {
            Some(SccpTransparentChainFamilyV1::Substrate)
        }
        _ => None,
    }
}

fn sccp_transparent_chain_family_code(family: SccpTransparentChainFamilyV1) -> u8 {
    match family {
        SccpTransparentChainFamilyV1::Evm => 1,
        SccpTransparentChainFamilyV1::Solana => 2,
        SccpTransparentChainFamilyV1::Ton => 3,
        SccpTransparentChainFamilyV1::Tron => 4,
        SccpTransparentChainFamilyV1::Substrate => 5,
    }
}

pub fn sccp_message_payload_kind_key(payload: &SccpPayloadV1) -> &'static str {
    match payload {
        SccpPayloadV1::AssetRegister(_) => "asset_register",
        SccpPayloadV1::RouteActivate(_) => "route_activate",
        SccpPayloadV1::Transfer(_) => "transfer",
        SccpPayloadV1::TokenAdd(_) => "token_add",
        SccpPayloadV1::TokenPause(_) => "token_pause",
        SccpPayloadV1::TokenResume(_) => "token_resume",
    }
}

pub fn sccp_payload_projection(payload: &SccpPayloadV1) -> Option<SccpPayloadProjectionV1> {
    match payload {
        SccpPayloadV1::AssetRegister(payload) => Some(SccpPayloadProjectionV1::AssetRegister(
            SccpAssetRegisterProjectionV1 {
                version: payload.version,
                target_domain: payload.target_domain,
                home_domain: payload.home_domain,
                nonce: payload.nonce,
                asset_id: decode_sccp_normalized_codec_value(
                    payload.asset_id_codec,
                    &payload.asset_id,
                )?,
                decimals: payload.decimals,
            },
        )),
        SccpPayloadV1::RouteActivate(payload) => Some(SccpPayloadProjectionV1::RouteActivate(
            SccpRouteActivateProjectionV1 {
                version: payload.version,
                source_domain: payload.source_domain,
                target_domain: payload.target_domain,
                nonce: payload.nonce,
                asset_id: decode_sccp_normalized_codec_value(
                    payload.asset_id_codec,
                    &payload.asset_id,
                )?,
                route_id: decode_sccp_normalized_codec_value(
                    payload.route_id_codec,
                    &payload.route_id,
                )?,
            },
        )),
        SccpPayloadV1::Transfer(payload) => Some(SccpPayloadProjectionV1::Transfer(
            SccpTransferProjectionV1 {
                version: payload.version,
                source_domain: payload.source_domain,
                dest_domain: payload.dest_domain,
                nonce: payload.nonce,
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
        SccpPayloadV1::TokenAdd(payload) => Some(SccpPayloadProjectionV1::TokenAdd(
            SccpTokenAddProjectionV1 {
                version: payload.version,
                target_domain: payload.target_domain,
                nonce: payload.nonce,
                sora_asset_id: payload.sora_asset_id,
                decimals: payload.decimals,
                name: payload.name,
                symbol: payload.symbol,
            },
        )),
        SccpPayloadV1::TokenPause(payload) => Some(SccpPayloadProjectionV1::TokenPause(
            SccpTokenControlProjectionV1 {
                version: payload.version,
                target_domain: payload.target_domain,
                nonce: payload.nonce,
                sora_asset_id: payload.sora_asset_id,
            },
        )),
        SccpPayloadV1::TokenResume(payload) => Some(SccpPayloadProjectionV1::TokenResume(
            SccpTokenControlProjectionV1 {
                version: payload.version,
                target_domain: payload.target_domain,
                nonce: payload.nonce,
                sora_asset_id: payload.sora_asset_id,
            },
        )),
    }
}

pub fn build_sccp_counterparty_proof_job_from_bundle_with_signer(
    bundle: &NexusSccpMessageProofV1,
    signer: &KeyPair,
) -> Option<SccpCounterpartyProofJobV1> {
    build_sccp_counterparty_proof_job_from_bundle_internal(bundle, None, Some(signer), false)
}

pub fn build_sccp_counterparty_proof_job_from_bundle_with_signer_allow_unready(
    bundle: &NexusSccpMessageProofV1,
    signer: &KeyPair,
    allow_unready: bool,
) -> Option<SccpCounterpartyProofJobV1> {
    build_sccp_counterparty_proof_job_from_bundle_internal(
        bundle,
        None,
        Some(signer),
        allow_unready,
    )
}

pub fn build_sccp_counterparty_proof_job_from_bundle_with_destination_binding_and_signer(
    bundle: &NexusSccpMessageProofV1,
    destination_binding: &SccpDestinationBindingV1,
    signer: &KeyPair,
) -> Option<SccpCounterpartyProofJobV1> {
    build_sccp_counterparty_proof_job_from_bundle_internal(
        bundle,
        Some(destination_binding),
        Some(signer),
        false,
    )
}

pub fn build_sccp_counterparty_proof_job_from_bundle_with_destination_binding_and_signer_allow_unready(
    bundle: &NexusSccpMessageProofV1,
    destination_binding: &SccpDestinationBindingV1,
    signer: &KeyPair,
    allow_unready: bool,
) -> Option<SccpCounterpartyProofJobV1> {
    build_sccp_counterparty_proof_job_from_bundle_internal(
        bundle,
        Some(destination_binding),
        Some(signer),
        allow_unready,
    )
}

pub fn build_sccp_counterparty_proof_job_from_bundle(
    bundle: &NexusSccpMessageProofV1,
) -> Option<SccpCounterpartyProofJobV1> {
    build_sccp_counterparty_proof_job_from_bundle_internal(bundle, None, None, false)
}

pub fn build_sccp_counterparty_proof_job_from_bundle_allow_unready(
    bundle: &NexusSccpMessageProofV1,
    allow_unready: bool,
) -> Option<SccpCounterpartyProofJobV1> {
    build_sccp_counterparty_proof_job_from_bundle_internal(bundle, None, None, allow_unready)
}

pub fn build_sccp_counterparty_proof_job_from_bundle_with_evm_groth16_proof_and_destination_binding(
    bundle: &NexusSccpMessageProofV1,
    groth16_proof_bytes: &[u8],
    destination_binding: &SccpDestinationBindingV1,
) -> Option<SccpCounterpartyProofJobV1> {
    build_sccp_counterparty_proof_job_from_bundle_with_evm_groth16_proof_and_destination_binding_allow_unready(
        bundle,
        groth16_proof_bytes,
        destination_binding,
        false,
    )
}

pub fn build_sccp_counterparty_proof_job_from_bundle_with_evm_groth16_proof_and_destination_binding_allow_unready(
    bundle: &NexusSccpMessageProofV1,
    groth16_proof_bytes: &[u8],
    destination_binding: &SccpDestinationBindingV1,
    allow_unready: bool,
) -> Option<SccpCounterpartyProofJobV1> {
    let counterparty_domain = sccp_counterparty_domain_for_message_payload(&bundle.payload)?;
    let manifest = sccp_proof_manifest_for_domain(counterparty_domain)?;
    if manifest.verifier_backend.family != SccpVerifierBackendFamilyV1::EvmGroth16Bn254
        || manifest.verifier_backend.key != SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1
    {
        return None;
    }
    build_sccp_counterparty_proof_job_from_bundle_with_proof_bytes_internal(
        bundle,
        &manifest,
        counterparty_domain,
        groth16_proof_bytes,
        Some(destination_binding),
        None,
        allow_unready,
    )
}

pub fn build_sccp_counterparty_proof_job_from_bundle_with_tron_groth16_proof(
    bundle: &NexusSccpMessageProofV1,
    groth16_proof_bytes: &[u8],
) -> Option<SccpCounterpartyProofJobV1> {
    build_sccp_counterparty_proof_job_from_bundle_with_tron_groth16_proof_allow_unready(
        bundle,
        groth16_proof_bytes,
        false,
    )
}

pub fn build_sccp_counterparty_proof_job_from_bundle_with_tron_groth16_proof_allow_unready(
    bundle: &NexusSccpMessageProofV1,
    groth16_proof_bytes: &[u8],
    allow_unready: bool,
) -> Option<SccpCounterpartyProofJobV1> {
    build_sccp_counterparty_proof_job_from_bundle_with_tron_groth16_proof_and_destination_binding_allow_unready(
        bundle,
        groth16_proof_bytes,
        &sccp_proof_manifest_for_domain(SCCP_DOMAIN_TRON)?.destination_binding,
        allow_unready,
    )
}

pub fn build_sccp_counterparty_proof_job_from_bundle_with_tron_groth16_proof_and_destination_binding(
    bundle: &NexusSccpMessageProofV1,
    groth16_proof_bytes: &[u8],
    destination_binding: &SccpDestinationBindingV1,
) -> Option<SccpCounterpartyProofJobV1> {
    build_sccp_counterparty_proof_job_from_bundle_with_tron_groth16_proof_and_destination_binding_allow_unready(
        bundle,
        groth16_proof_bytes,
        destination_binding,
        false,
    )
}

pub fn build_sccp_counterparty_proof_job_from_bundle_with_tron_groth16_proof_and_destination_binding_allow_unready(
    bundle: &NexusSccpMessageProofV1,
    groth16_proof_bytes: &[u8],
    destination_binding: &SccpDestinationBindingV1,
    allow_unready: bool,
) -> Option<SccpCounterpartyProofJobV1> {
    let counterparty_domain = sccp_counterparty_domain_for_message_payload(&bundle.payload)?;
    if counterparty_domain != SCCP_DOMAIN_TRON {
        return None;
    }
    let manifest = sccp_proof_manifest_for_domain(counterparty_domain)?;
    if manifest.verifier_backend.family != SccpVerifierBackendFamilyV1::TronGroth16Bn254
        || manifest.verifier_backend.key != SCCP_TRON_GROTH16_BN254_PROOF_BACKEND_V1
    {
        return None;
    }
    build_sccp_counterparty_proof_job_from_bundle_with_proof_bytes_internal(
        bundle,
        &manifest,
        counterparty_domain,
        groth16_proof_bytes,
        Some(destination_binding),
        None,
        allow_unready,
    )
}

fn build_sccp_counterparty_proof_job_from_bundle_internal(
    bundle: &NexusSccpMessageProofV1,
    platform_destination_binding: Option<&SccpDestinationBindingV1>,
    signer: Option<&KeyPair>,
    allow_unready: bool,
) -> Option<SccpCounterpartyProofJobV1> {
    if !verify_message_bundle_structure(bundle) {
        return None;
    }
    let counterparty_domain = sccp_counterparty_domain_for_message_payload(&bundle.payload)?;
    let manifest = sccp_proof_manifest_for_domain(counterparty_domain)?;
    let proof_bytes = build_sccp_message_transparent_fastpq_proof_bytes(bundle, &manifest)?;
    build_sccp_counterparty_proof_job_from_bundle_with_proof_bytes_internal(
        bundle,
        &manifest,
        counterparty_domain,
        &proof_bytes,
        platform_destination_binding,
        signer,
        allow_unready,
    )
}

fn build_sccp_counterparty_proof_job_from_bundle_with_proof_bytes_internal(
    bundle: &NexusSccpMessageProofV1,
    manifest: &SccpProofManifestV1,
    counterparty_domain: u32,
    proof_bytes: &[u8],
    platform_destination_binding: Option<&SccpDestinationBindingV1>,
    signer: Option<&KeyPair>,
    allow_unready: bool,
) -> Option<SccpCounterpartyProofJobV1> {
    if !verify_message_bundle_structure(bundle) {
        return None;
    }
    if !sccp_manifest_allows_transparent_proofs(manifest, allow_unready) {
        return None;
    }
    let chain_family = sccp_transparent_chain_family_for_domain(counterparty_domain)?;
    let chain = sccp_chain_key_for_domain(counterparty_domain)?;
    let public_inputs = sccp_message_transparent_public_inputs(bundle)?;
    let payload_projection = sccp_payload_projection(&bundle.payload)?;
    let submission_package = build_sccp_counterparty_submission_package_internal(
        bundle,
        manifest,
        proof_bytes,
        platform_destination_binding,
        signer,
        allow_unready,
        None,
    )?;

    Some(SccpCounterpartyProofJobV1 {
        version: 1,
        chain_family,
        chain: chain.to_owned(),
        local_domain: manifest.local_domain,
        counterparty_domain,
        security_model: manifest.security_model,
        anchor_governance: manifest.anchor_governance,
        destination_binding: manifest.destination_binding.clone(),
        proof_family: manifest.proof_family.clone(),
        verifier_backend: manifest.verifier_backend.clone(),
        message_backend: manifest.message_backend.clone(),
        registry_backend: manifest.registry_backend.clone(),
        manifest_seed: manifest.manifest_seed.clone(),
        finality_model: manifest.finality_model,
        verifier_target: manifest.verifier_target,
        public_inputs,
        payload_kind: sccp_message_payload_kind_key(&bundle.payload).to_owned(),
        payload_projection,
        submission_template: manifest.submission_template.clone(),
        submission_package,
        bundle: bundle.clone(),
    })
}

pub fn build_sccp_counterparty_proof_job_from_artifact(
    artifact: &NexusSccpMessageTransparentProofV1,
) -> Option<SccpCounterpartyProofJobV1> {
    build_sccp_counterparty_proof_job_from_artifact_allow_unready(artifact, false)
}

pub fn build_sccp_counterparty_proof_job_from_artifact_allow_unready(
    artifact: &NexusSccpMessageTransparentProofV1,
    allow_unready: bool,
) -> Option<SccpCounterpartyProofJobV1> {
    if !verify_nexus_sccp_message_transparent_proof_structure_allow_unready(artifact, allow_unready)
    {
        return None;
    }
    let chain_family = sccp_transparent_chain_family_for_domain(artifact.counterparty_domain)?;
    let chain = sccp_chain_key_for_domain(artifact.counterparty_domain)?;
    let payload_projection = sccp_payload_projection(&artifact.bundle.payload)?;
    let manifest = sccp_proof_manifest_for_domain(artifact.counterparty_domain)?;
    Some(SccpCounterpartyProofJobV1 {
        version: 1,
        chain_family,
        chain: chain.to_owned(),
        local_domain: artifact.local_domain,
        counterparty_domain: artifact.counterparty_domain,
        security_model: artifact.security_model,
        anchor_governance: artifact.anchor_governance,
        destination_binding: artifact.destination_binding.clone(),
        proof_family: artifact.proof_family.clone(),
        verifier_backend: artifact.verifier_backend.clone(),
        message_backend: artifact.message_backend.clone(),
        registry_backend: artifact.registry_backend.clone(),
        manifest_seed: artifact.manifest_seed.clone(),
        finality_model: artifact.finality_model,
        verifier_target: artifact.verifier_target,
        public_inputs: artifact.public_inputs.clone(),
        payload_kind: sccp_message_payload_kind_key(&artifact.bundle.payload).to_owned(),
        payload_projection,
        submission_template: manifest.submission_template,
        submission_package: artifact.submission_package.clone(),
        bundle: artifact.bundle.clone(),
    })
}

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

pub fn canonical_sccp_merkle_proof_bytes(proof: &SccpMerkleProofV1) -> Vec<u8> {
    let mut out = Vec::new();
    push_u32(
        &mut out,
        u32::try_from(proof.steps.len()).expect("merkle proof step count fits into u32"),
    );
    for step in &proof.steps {
        out.extend_from_slice(&step.sibling_hash);
        push_u8(&mut out, u8::from(step.sibling_is_left));
    }
    out
}

pub fn canonical_nexus_sccp_message_bundle_bytes(bundle: &NexusSccpMessageProofV1) -> Vec<u8> {
    let commitment = canonical_commitment_bytes(&bundle.commitment);
    let merkle_proof = canonical_sccp_merkle_proof_bytes(&bundle.merkle_proof);
    let payload = canonical_sccp_payload_bytes(&bundle.payload);

    let mut out = Vec::new();
    push_u8(&mut out, bundle.version);
    out.extend_from_slice(&bundle.commitment_root);
    push_vec(&mut out, &commitment);
    push_vec(&mut out, &merkle_proof);
    push_vec(&mut out, &payload);
    push_vec(&mut out, &bundle.finality_proof);
    out
}

pub fn canonical_sccp_source_chain_proof_envelope_bytes(
    proof: &SccpSourceChainProofEnvelopeV1,
) -> Vec<u8> {
    let mut out = Vec::new();
    push_u8(&mut out, proof.version);
    push_u32(&mut out, proof.source_domain);
    push_u32(&mut out, proof.target_domain);
    push_vec(&mut out, proof.source_chain.as_bytes());
    push_u8(
        &mut out,
        sccp_source_proof_plan_code(proof.source_proof_plan),
    );
    push_u8(
        &mut out,
        sccp_proof_finality_model_code(proof.finality_model),
    );
    out.extend_from_slice(&proof.message_id);
    out.extend_from_slice(&proof.payload_hash);
    out.extend_from_slice(&proof.source_event_digest);
    out.extend_from_slice(&proof.commitment_root);
    push_u64(&mut out, proof.finality_height);
    out.extend_from_slice(&proof.finality_block_hash);
    out.extend_from_slice(&proof.finalized_header_hash);
    out.extend_from_slice(&proof.receipt_or_message_root);
    push_vec(&mut out, &proof.consensus_proof);
    push_vec(&mut out, &proof.message_inclusion_proof);
    push_u32(
        &mut out,
        u32::try_from(proof.inclusion_branch.len())
            .expect("SCCP source proof inclusion branch length fits into u32"),
    );
    for step in &proof.inclusion_branch {
        push_vec(&mut out, step);
    }
    out
}

pub fn canonical_sccp_source_consensus_proof_bytes(proof: &SccpSourceConsensusProofV1) -> Vec<u8> {
    let mut out = Vec::new();
    push_u8(&mut out, proof.version);
    push_u32(&mut out, proof.source_domain);
    push_vec(&mut out, proof.source_chain.as_bytes());
    push_u8(
        &mut out,
        sccp_source_proof_plan_code(proof.source_proof_plan),
    );
    push_u8(
        &mut out,
        sccp_proof_finality_model_code(proof.finality_model),
    );
    push_u64(&mut out, proof.finality_height);
    out.extend_from_slice(&proof.finality_block_hash);
    out.extend_from_slice(&proof.receipt_or_message_root);
    out.extend_from_slice(&proof.finalized_header_hash);
    push_vec(
        &mut out,
        &canonical_sccp_source_adapter_proof_bytes(&proof.adapter_proof),
    );
    out.extend_from_slice(&proof.adapter_transcript_hash);
    push_vec(
        &mut out,
        &canonical_sccp_source_verifier_evidence_bytes(&proof.verifier_evidence),
    );
    push_vec(
        &mut out,
        &canonical_sccp_source_adapter_verification_proof_bytes(&proof.adapter_verification_proof),
    );
    out
}

pub fn canonical_sccp_source_verifier_evidence_bytes(
    evidence: &SccpSourceVerifierEvidenceV1,
) -> Vec<u8> {
    let mut out = Vec::new();
    push_u8(&mut out, evidence.version);
    push_u32(&mut out, evidence.source_domain);
    push_vec(&mut out, evidence.source_chain.as_bytes());
    push_u8(
        &mut out,
        sccp_source_proof_plan_code(evidence.source_proof_plan),
    );
    push_u8(
        &mut out,
        sccp_proof_finality_model_code(evidence.finality_model),
    );
    out.extend_from_slice(&evidence.adapter_proof_hash);
    out.extend_from_slice(&evidence.adapter_transcript_hash);
    push_vec(&mut out, evidence.adapter_circuit_id.as_bytes());
    push_vec(&mut out, evidence.source_trust_anchor_id.as_bytes());
    out.extend_from_slice(&evidence.source_trust_anchor_hash);
    push_vec(&mut out, evidence.consensus_verifier_id.as_bytes());
    out.extend_from_slice(&evidence.consensus_verifier_hash);
    push_vec(&mut out, evidence.message_inclusion_verifier_id.as_bytes());
    out.extend_from_slice(&evidence.message_inclusion_verifier_hash);
    push_vec(&mut out, evidence.finality_policy_id.as_bytes());
    out.extend_from_slice(&evidence.finality_policy_hash);
    out
}

pub fn sccp_source_verifier_evidence_hash(evidence: &SccpSourceVerifierEvidenceV1) -> H256 {
    prefixed_blake2b(
        b"sccp:source-verifier-evidence:v1",
        &canonical_sccp_source_verifier_evidence_bytes(evidence),
    )
}

pub fn canonical_sccp_source_verifier_material_bytes(
    material: &SccpSourceVerifierMaterialV1,
) -> Vec<u8> {
    let mut out = Vec::new();
    push_u8(&mut out, material.version);
    push_u32(&mut out, material.source_domain);
    push_vec(&mut out, material.source_chain.as_bytes());
    push_u8(
        &mut out,
        sccp_source_proof_plan_code(material.source_proof_plan),
    );
    push_u8(
        &mut out,
        sccp_proof_finality_model_code(material.finality_model),
    );
    push_vec(&mut out, material.adapter_circuit_id.as_bytes());
    push_vec(&mut out, material.source_trust_anchor_id.as_bytes());
    out.extend_from_slice(&material.source_trust_anchor_hash);
    push_vec(&mut out, material.consensus_verifier_id.as_bytes());
    out.extend_from_slice(&material.consensus_verifier_hash);
    push_vec(&mut out, material.message_inclusion_verifier_id.as_bytes());
    out.extend_from_slice(&material.message_inclusion_verifier_hash);
    push_vec(&mut out, material.finality_policy_id.as_bytes());
    out.extend_from_slice(&material.finality_policy_hash);
    push_u8(&mut out, u8::from(material.placeholder_material));
    out
}

pub fn sccp_source_verifier_material_hash(material: &SccpSourceVerifierMaterialV1) -> H256 {
    prefixed_blake2b(
        b"sccp:source-verifier-material-record:v1",
        &canonical_sccp_source_verifier_material_bytes(material),
    )
}

pub fn canonical_sccp_source_adapter_verification_proof_bytes(
    proof: &SccpSourceAdapterVerificationProofV1,
) -> Vec<u8> {
    let mut out = Vec::new();
    push_u8(&mut out, proof.version);
    push_vec(&mut out, proof.proof_family.as_bytes());
    push_vec(&mut out, proof.circuit_id.as_bytes());
    push_vec(&mut out, &proof.proof_bytes);
    out
}

pub fn canonical_sccp_source_adapter_proof_bytes(proof: &SccpSourceAdapterProofV1) -> Vec<u8> {
    let mut out = Vec::new();
    push_u8(&mut out, sccp_source_adapter_proof_code(proof));
    match proof {
        SccpSourceAdapterProofV1::EthereumBeaconReceipt(proof) => {
            push_u8(&mut out, proof.version);
            push_u32(&mut out, proof.source_domain);
            push_u64(&mut out, proof.beacon_slot);
            push_u64(&mut out, proof.execution_block_number);
            out.extend_from_slice(&proof.execution_block_hash);
            out.extend_from_slice(&proof.execution_receipts_root);
            out.extend_from_slice(&proof.beacon_finalized_root);
            out.extend_from_slice(&proof.sync_committee_root);
            out.extend_from_slice(&proof.receipt_trie_proof_hash);
        }
        SccpSourceAdapterProofV1::BscValidatorSetReceipt(proof) => {
            push_u8(&mut out, proof.version);
            push_u32(&mut out, proof.source_domain);
            push_u64(&mut out, proof.validator_epoch);
            push_u64(&mut out, proof.block_number);
            out.extend_from_slice(&proof.block_hash);
            out.extend_from_slice(&proof.receipts_root);
            out.extend_from_slice(&proof.validator_set_hash);
            out.extend_from_slice(&proof.commit_seal_hash);
            out.extend_from_slice(&proof.receipt_trie_proof_hash);
        }
        SccpSourceAdapterProofV1::SolanaFinalizedTransaction(proof) => {
            push_u8(&mut out, proof.version);
            push_u32(&mut out, proof.source_domain);
            push_u64(&mut out, proof.finalized_slot);
            out.extend_from_slice(&proof.blockhash);
            out.extend_from_slice(&proof.bank_hash);
            out.extend_from_slice(&proof.transaction_status_root);
            out.extend_from_slice(&proof.message_proof_hash);
        }
        SccpSourceAdapterProofV1::TonMasterchainShard(proof) => {
            push_u8(&mut out, proof.version);
            push_u32(&mut out, proof.source_domain);
            push_u64(&mut out, proof.masterchain_seqno);
            out.extend_from_slice(&proof.masterchain_block_hash);
            out.extend_from_slice(&proof.shard_block_hash);
            out.extend_from_slice(&proof.shard_state_root);
            out.extend_from_slice(&proof.transaction_root);
            out.extend_from_slice(&proof.shard_proof_hash);
        }
        SccpSourceAdapterProofV1::TronDposReceipt(proof) => {
            push_u8(&mut out, proof.version);
            push_u32(&mut out, proof.source_domain);
            push_u64(&mut out, proof.solid_block_number);
            out.extend_from_slice(&proof.block_hash);
            out.extend_from_slice(&proof.witness_schedule_hash);
            out.extend_from_slice(&proof.receipt_root);
            out.extend_from_slice(&proof.transaction_root);
            out.extend_from_slice(&proof.receipt_proof_hash);
        }
        SccpSourceAdapterProofV1::SubstrateGrandpaEvent(proof) => {
            push_u8(&mut out, proof.version);
            push_u32(&mut out, proof.source_domain);
            push_u64(&mut out, proof.finalized_block_number);
            push_u64(&mut out, proof.grandpa_set_id);
            out.extend_from_slice(&proof.block_hash);
            out.extend_from_slice(&proof.authority_set_hash);
            out.extend_from_slice(&proof.events_root);
            out.extend_from_slice(&proof.storage_proof_hash);
        }
    }
    out
}

pub fn sccp_source_adapter_proof_hash(proof: &SccpSourceAdapterProofV1) -> H256 {
    prefixed_blake2b(
        b"sccp:source-adapter-proof:v1",
        &canonical_sccp_source_adapter_proof_bytes(proof),
    )
}

pub fn sccp_source_adapter_transcript_hash(
    source_domain: u32,
    target_domain: u32,
    source_proof_plan: SccpSourceProofPlanV1,
    finality_model: SccpProofFinalityModelV1,
    finality_height: u64,
    finality_block_hash: H256,
    receipt_or_message_root: H256,
    source_event_digest: H256,
    adapter_proof: &SccpSourceAdapterProofV1,
) -> H256 {
    let mut out = Vec::new();
    push_u32(&mut out, source_domain);
    push_u32(&mut out, target_domain);
    push_u8(&mut out, sccp_source_proof_plan_code(source_proof_plan));
    push_u8(&mut out, sccp_proof_finality_model_code(finality_model));
    push_u64(&mut out, finality_height);
    out.extend_from_slice(&finality_block_hash);
    out.extend_from_slice(&receipt_or_message_root);
    out.extend_from_slice(&source_event_digest);
    push_vec(
        &mut out,
        &canonical_sccp_source_adapter_proof_bytes(adapter_proof),
    );
    prefixed_blake2b(b"sccp:source-adapter-transcript:v1", &out)
}

fn sccp_source_verifier_material_id(
    source_domain: u32,
    component: &str,
    suffix: &str,
) -> Option<String> {
    let source_chain = sccp_chain_key_for_domain(source_domain)?;
    Some(format!("sccp:{source_chain}:{component}:{suffix}:v1"))
}

fn sccp_source_verifier_component_hash(
    source_domain: u32,
    source_proof_plan: SccpSourceProofPlanV1,
    finality_model: SccpProofFinalityModelV1,
    component_id: &str,
) -> H256 {
    // TODO: Replace these deterministic material placeholders with on-chain
    // source light-client anchors and immutable verifier code hashes when the
    // external-chain verifier engines are wired into SCCP production admission.
    let mut out = Vec::new();
    push_u8(&mut out, 1);
    push_u32(&mut out, source_domain);
    push_vec(
        &mut out,
        sccp_chain_key_for_domain(source_domain)
            .unwrap_or_default()
            .as_bytes(),
    );
    push_u8(&mut out, sccp_source_proof_plan_code(source_proof_plan));
    push_u8(&mut out, sccp_proof_finality_model_code(finality_model));
    push_vec(
        &mut out,
        SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID_V1.as_bytes(),
    );
    push_vec(&mut out, component_id.as_bytes());
    prefixed_blake2b(b"sccp:source-verifier-material:v1", &out)
}

pub fn sccp_source_verifier_material_for_domain(
    source_domain: u32,
) -> Option<SccpSourceVerifierMaterialV1> {
    let source_chain = sccp_chain_key_for_domain(source_domain)?;
    let source_proof_plan = sccp_source_proof_plan_for_domain(source_domain)?;
    let finality_model = sccp_proof_finality_model_for_domain(source_domain)?;
    let source_trust_anchor_id =
        sccp_source_verifier_material_id(source_domain, "source-trust-anchor", "active")?;
    let consensus_verifier_id = sccp_source_verifier_material_id(
        source_domain,
        "consensus-verifier",
        source_proof_plan.as_str(),
    )?;
    let message_inclusion_verifier_id = sccp_source_verifier_material_id(
        source_domain,
        "message-inclusion-verifier",
        source_proof_plan.as_str(),
    )?;
    let finality_policy_id = sccp_source_verifier_material_id(
        source_domain,
        "finality-policy",
        finality_model.as_str(),
    )?;

    Some(SccpSourceVerifierMaterialV1 {
        version: 1,
        source_domain,
        source_chain: source_chain.to_owned(),
        source_proof_plan,
        finality_model,
        adapter_circuit_id: SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID_V1.to_owned(),
        source_trust_anchor_hash: sccp_source_verifier_component_hash(
            source_domain,
            source_proof_plan,
            finality_model,
            &source_trust_anchor_id,
        ),
        source_trust_anchor_id,
        consensus_verifier_hash: sccp_source_verifier_component_hash(
            source_domain,
            source_proof_plan,
            finality_model,
            &consensus_verifier_id,
        ),
        consensus_verifier_id,
        message_inclusion_verifier_hash: sccp_source_verifier_component_hash(
            source_domain,
            source_proof_plan,
            finality_model,
            &message_inclusion_verifier_id,
        ),
        message_inclusion_verifier_id,
        finality_policy_hash: sccp_source_verifier_component_hash(
            source_domain,
            source_proof_plan,
            finality_model,
            &finality_policy_id,
        ),
        finality_policy_id,
        placeholder_material: true,
    })
}

pub fn sccp_source_verifier_material_uses_builtin_placeholder_components(
    material: &SccpSourceVerifierMaterialV1,
) -> bool {
    let Some(placeholder) = sccp_source_verifier_material_for_domain(material.source_domain) else {
        return false;
    };
    material.source_trust_anchor_id == placeholder.source_trust_anchor_id
        || material.source_trust_anchor_hash == placeholder.source_trust_anchor_hash
        || material.consensus_verifier_id == placeholder.consensus_verifier_id
        || material.consensus_verifier_hash == placeholder.consensus_verifier_hash
        || material.message_inclusion_verifier_id == placeholder.message_inclusion_verifier_id
        || material.message_inclusion_verifier_hash == placeholder.message_inclusion_verifier_hash
        || material.finality_policy_id == placeholder.finality_policy_id
        || material.finality_policy_hash == placeholder.finality_policy_hash
}

pub fn sccp_source_verifier_material_is_production_ready(
    material: &SccpSourceVerifierMaterialV1,
) -> bool {
    material.version == 1
        // TODO: Replace this fail-closed SOL guard when the mainnet Solana
        // recursive verifier, finality policy, and source trust anchor material
        // are represented in `SccpSourceVerifierMaterialV1`.
        && material.source_domain != SCCP_DOMAIN_SOL
        && !material.placeholder_material
        && !sccp_source_verifier_material_uses_builtin_placeholder_components(material)
        && sccp_chain_key_for_domain(material.source_domain)
            .is_some_and(|chain| material.source_chain == chain)
        && sccp_source_proof_plan_for_domain(material.source_domain)
            == Some(material.source_proof_plan)
        && sccp_proof_finality_model_for_domain(material.source_domain)
            == Some(material.finality_model)
        && material.adapter_circuit_id == SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID_V1
        && !material.source_trust_anchor_id.is_empty()
        && !material.consensus_verifier_id.is_empty()
        && !material.message_inclusion_verifier_id.is_empty()
        && !material.finality_policy_id.is_empty()
        && h256_is_nonzero(&material.source_trust_anchor_hash)
        && h256_is_nonzero(&material.consensus_verifier_hash)
        && h256_is_nonzero(&material.message_inclusion_verifier_hash)
        && h256_is_nonzero(&material.finality_policy_hash)
        && h256_is_nonzero(&sccp_source_verifier_material_hash(material))
}

fn build_sccp_source_verifier_evidence_from_material(
    proof: &SccpSourceChainProofEnvelopeV1,
    adapter_proof: &SccpSourceAdapterProofV1,
    adapter_transcript_hash: H256,
    material: &SccpSourceVerifierMaterialV1,
) -> Option<SccpSourceVerifierEvidenceV1> {
    let source_chain = sccp_chain_key_for_domain(proof.source_domain)?;
    if proof.source_chain != source_chain
        || material.version != 1
        || material.source_domain != proof.source_domain
        || material.source_chain != proof.source_chain
        || material.source_proof_plan != proof.source_proof_plan
        || material.finality_model != proof.finality_model
        || material.adapter_circuit_id != SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID_V1
        || sccp_source_proof_plan_for_domain(proof.source_domain) != Some(proof.source_proof_plan)
        || sccp_proof_finality_model_for_domain(proof.source_domain) != Some(proof.finality_model)
        || !h256_is_nonzero(&adapter_transcript_hash)
    {
        return None;
    }

    Some(SccpSourceVerifierEvidenceV1 {
        version: 1,
        source_domain: proof.source_domain,
        source_chain: source_chain.to_owned(),
        source_proof_plan: proof.source_proof_plan,
        finality_model: proof.finality_model,
        adapter_proof_hash: sccp_source_adapter_proof_hash(adapter_proof),
        adapter_transcript_hash,
        adapter_circuit_id: material.adapter_circuit_id.clone(),
        source_trust_anchor_id: material.source_trust_anchor_id.clone(),
        source_trust_anchor_hash: material.source_trust_anchor_hash,
        consensus_verifier_id: material.consensus_verifier_id.clone(),
        consensus_verifier_hash: material.consensus_verifier_hash,
        message_inclusion_verifier_id: material.message_inclusion_verifier_id.clone(),
        message_inclusion_verifier_hash: material.message_inclusion_verifier_hash,
        finality_policy_id: material.finality_policy_id.clone(),
        finality_policy_hash: material.finality_policy_hash,
    })
}

pub fn build_sccp_source_verifier_evidence(
    proof: &SccpSourceChainProofEnvelopeV1,
    adapter_proof: &SccpSourceAdapterProofV1,
    adapter_transcript_hash: H256,
) -> Option<SccpSourceVerifierEvidenceV1> {
    let material = sccp_source_verifier_material_for_domain(proof.source_domain)?;
    build_sccp_source_verifier_evidence_from_material(
        proof,
        adapter_proof,
        adapter_transcript_hash,
        &material,
    )
}

pub fn build_sccp_source_verifier_evidence_with_material(
    proof: &SccpSourceChainProofEnvelopeV1,
    adapter_proof: &SccpSourceAdapterProofV1,
    adapter_transcript_hash: H256,
    material: &SccpSourceVerifierMaterialV1,
) -> Option<SccpSourceVerifierEvidenceV1> {
    build_sccp_source_verifier_evidence_from_material(
        proof,
        adapter_proof,
        adapter_transcript_hash,
        material,
    )
}

fn verify_sccp_source_verifier_evidence_with_material(
    proof: &SccpSourceChainProofEnvelopeV1,
    adapter_proof: &SccpSourceAdapterProofV1,
    adapter_transcript_hash: H256,
    evidence: &SccpSourceVerifierEvidenceV1,
    material: &SccpSourceVerifierMaterialV1,
) -> bool {
    let Some(expected) = build_sccp_source_verifier_evidence_with_material(
        proof,
        adapter_proof,
        adapter_transcript_hash,
        material,
    ) else {
        return false;
    };
    evidence == &expected
        && evidence.version == 1
        && !evidence.source_chain.is_empty()
        && !evidence.adapter_circuit_id.is_empty()
        && !evidence.source_trust_anchor_id.is_empty()
        && !evidence.consensus_verifier_id.is_empty()
        && !evidence.message_inclusion_verifier_id.is_empty()
        && !evidence.finality_policy_id.is_empty()
        && h256_is_nonzero(&evidence.adapter_proof_hash)
        && h256_is_nonzero(&evidence.adapter_transcript_hash)
        && h256_is_nonzero(&evidence.source_trust_anchor_hash)
        && h256_is_nonzero(&evidence.consensus_verifier_hash)
        && h256_is_nonzero(&evidence.message_inclusion_verifier_hash)
        && h256_is_nonzero(&evidence.finality_policy_hash)
        && h256_is_nonzero(&sccp_source_verifier_evidence_hash(evidence))
}

pub fn canonical_sccp_source_adapter_verification_statement_bytes(
    proof: &SccpSourceChainProofEnvelopeV1,
    adapter_proof: &SccpSourceAdapterProofV1,
    adapter_transcript_hash: H256,
    verifier_evidence_hash: H256,
) -> Vec<u8> {
    let adapter_proof_bytes = canonical_sccp_source_adapter_proof_bytes(adapter_proof);
    let mut out = Vec::new();
    push_u8(&mut out, 1);
    push_u32(&mut out, proof.source_domain);
    push_u32(&mut out, proof.target_domain);
    push_vec(&mut out, proof.source_chain.as_bytes());
    push_u8(
        &mut out,
        sccp_source_proof_plan_code(proof.source_proof_plan),
    );
    push_u8(
        &mut out,
        sccp_proof_finality_model_code(proof.finality_model),
    );
    out.extend_from_slice(&proof.message_id);
    out.extend_from_slice(&proof.payload_hash);
    out.extend_from_slice(&proof.source_event_digest);
    out.extend_from_slice(&proof.commitment_root);
    push_u64(&mut out, proof.finality_height);
    out.extend_from_slice(&proof.finality_block_hash);
    out.extend_from_slice(&proof.finalized_header_hash);
    out.extend_from_slice(&proof.receipt_or_message_root);
    out.extend_from_slice(&adapter_transcript_hash);
    out.extend_from_slice(&verifier_evidence_hash);
    out.extend_from_slice(&sccp_source_adapter_proof_hash(adapter_proof));
    push_vec(&mut out, &adapter_proof_bytes);
    out
}

fn canonical_sccp_source_adapter_verification_context_bytes(
    proof: &SccpSourceChainProofEnvelopeV1,
    adapter_proof: &SccpSourceAdapterProofV1,
    adapter_transcript_hash: H256,
    verifier_evidence_hash: H256,
) -> Vec<u8> {
    let statement = canonical_sccp_source_adapter_verification_statement_bytes(
        proof,
        adapter_proof,
        adapter_transcript_hash,
        verifier_evidence_hash,
    );
    let mut out = Vec::new();
    push_u8(&mut out, 1);
    push_vec(
        &mut out,
        SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID_V1.as_bytes(),
    );
    push_vec(
        &mut out,
        sccp_chain_key_for_domain(proof.source_domain)
            .unwrap_or_default()
            .as_bytes(),
    );
    push_vec(
        &mut out,
        SCCP_SOURCE_ADAPTER_FASTPQ_PARAMETER_SET_V1.as_bytes(),
    );
    out.extend_from_slice(&prefixed_blake2b(
        b"sccp:source-adapter:statement:v1",
        &statement,
    ));
    out.extend_from_slice(&adapter_transcript_hash);
    out.extend_from_slice(&verifier_evidence_hash);
    out
}

fn sccp_source_adapter_fastpq_public_inputs(
    proof: &SccpSourceChainProofEnvelopeV1,
    adapter_transcript_hash: H256,
) -> FastpqPublicInputs {
    let mut dsid = [0u8; 16];
    let dsid_hash = prefixed_blake2b(
        SCCP_SOURCE_ADAPTER_FASTPQ_DSID_PREFIX_V1,
        &adapter_transcript_hash,
    );
    dsid.copy_from_slice(&dsid_hash[..16]);
    FastpqPublicInputs {
        dsid,
        slot: proof.finality_height,
        old_root: proof.source_event_digest,
        new_root: proof.receipt_or_message_root,
        perm_root: proof.finality_block_hash,
        tx_set_hash: adapter_transcript_hash,
    }
}

fn sccp_source_adapter_public_input_columns(
    proof: &SccpSourceChainProofEnvelopeV1,
    adapter_transcript_hash: H256,
    verifier_evidence_hash: H256,
) -> Vec<Vec<[u8; 32]>> {
    let mut source_domain = [0u8; 32];
    source_domain[..4].copy_from_slice(&proof.source_domain.to_le_bytes());
    let mut target_domain = [0u8; 32];
    target_domain[..4].copy_from_slice(&proof.target_domain.to_le_bytes());
    let mut finality_height = [0u8; 32];
    finality_height[..8].copy_from_slice(&proof.finality_height.to_le_bytes());
    vec![
        vec![source_domain],
        vec![target_domain],
        vec![proof.message_id],
        vec![proof.payload_hash],
        vec![proof.source_event_digest],
        vec![finality_height],
        vec![proof.finality_block_hash],
        vec![proof.receipt_or_message_root],
        vec![adapter_transcript_hash],
        vec![verifier_evidence_hash],
    ]
}

fn sccp_source_adapter_open_verify_schema_descriptor(
    proof: &SccpSourceChainProofEnvelopeV1,
) -> Vec<u8> {
    let mut descriptor = Vec::new();
    push_u8(&mut descriptor, 1);
    push_vec(
        &mut descriptor,
        SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID_V1.as_bytes(),
    );
    push_vec(
        &mut descriptor,
        SCCP_SOURCE_ADAPTER_FASTPQ_PARAMETER_SET_V1.as_bytes(),
    );
    push_vec(&mut descriptor, proof.source_chain.as_bytes());
    push_u32(&mut descriptor, proof.source_domain);
    push_u32(&mut descriptor, proof.target_domain);
    push_u8(
        &mut descriptor,
        sccp_source_proof_plan_code(proof.source_proof_plan),
    );
    push_u8(
        &mut descriptor,
        sccp_proof_finality_model_code(proof.finality_model),
    );
    for required_input in [
        "source_domain",
        "target_domain",
        "message_id",
        "payload_hash",
        "source_event_digest",
        "finality_height",
        "finality_block_hash",
        "receipt_or_message_root",
        "adapter_transcript_hash",
        "source_verifier_evidence_hash",
    ] {
        push_vec(&mut descriptor, required_input.as_bytes());
    }
    descriptor
}

fn canonical_sccp_source_adapter_fastpq_verifier_bytes(
    proof: &SccpSourceChainProofEnvelopeV1,
) -> Option<Vec<u8>> {
    let params = FastpqProver::canonical_parameter_sets()
        .iter()
        .find(|params| params.name == SCCP_SOURCE_ADAPTER_FASTPQ_PARAMETER_SET_V1)?;
    let mut verifier = Vec::new();
    push_u8(&mut verifier, 1);
    push_vec(
        &mut verifier,
        SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID_V1.as_bytes(),
    );
    push_vec(&mut verifier, proof.source_chain.as_bytes());
    push_u32(&mut verifier, proof.source_domain);
    push_u32(&mut verifier, proof.target_domain);
    push_u8(
        &mut verifier,
        sccp_source_proof_plan_code(proof.source_proof_plan),
    );
    push_u8(
        &mut verifier,
        sccp_proof_finality_model_code(proof.finality_model),
    );
    push_vec(&mut verifier, params.name.as_bytes());
    push_u32(&mut verifier, params.target_security_bits);
    push_u32(&mut verifier, params.grinding_bits);
    push_u32(&mut verifier, params.trace_log_size);
    push_u64(&mut verifier, params.trace_root);
    push_u32(&mut verifier, params.lde_log_size);
    push_u64(&mut verifier, params.lde_root);
    push_u32(&mut verifier, params.permutation_size);
    match params.lookup_log_size {
        Some(lookup_log_size) => {
            push_u8(&mut verifier, 1);
            push_u32(&mut verifier, lookup_log_size);
        }
        None => push_u8(&mut verifier, 0),
    }
    push_u64(&mut verifier, params.omega_coset);
    push_vec(&mut verifier, params.field.name.as_bytes());
    push_vec(&mut verifier, params.field.modulus_decimal.as_bytes());
    push_u32(&mut verifier, params.field.extension_degree);
    push_vec(&mut verifier, params.hash.trace_commitment.as_bytes());
    push_vec(&mut verifier, params.hash.transcript.as_bytes());
    push_u32(&mut verifier, params.fri.arity);
    push_u32(&mut verifier, params.fri.blowup_factor);
    push_u32(&mut verifier, params.fri.max_reductions);
    push_u32(&mut verifier, params.fri.queries);
    Some(verifier)
}

fn sccp_source_adapter_fastpq_verifier_commitment(
    proof: &SccpSourceChainProofEnvelopeV1,
) -> Option<H256> {
    let verifier = canonical_sccp_source_adapter_fastpq_verifier_bytes(proof)?;
    let mut hasher = Sha256::new();
    Digest::update(
        &mut hasher,
        SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID_V1.as_bytes(),
    );
    Digest::update(&mut hasher, &verifier);
    Some(hasher.finalize().into())
}

fn build_sccp_source_adapter_fastpq_batch(
    proof: &SccpSourceChainProofEnvelopeV1,
    adapter_proof: &SccpSourceAdapterProofV1,
    adapter_transcript_hash: H256,
    verifier_evidence_hash: H256,
) -> Option<FastpqTransitionBatch> {
    let statement = canonical_sccp_source_adapter_verification_statement_bytes(
        proof,
        adapter_proof,
        adapter_transcript_hash,
        verifier_evidence_hash,
    );
    let adapter_bytes = canonical_sccp_source_adapter_proof_bytes(adapter_proof);
    let context = canonical_sccp_source_adapter_verification_context_bytes(
        proof,
        adapter_proof,
        adapter_transcript_hash,
        verifier_evidence_hash,
    );
    let mut batch = FastpqTransitionBatch::new(
        SCCP_SOURCE_ADAPTER_FASTPQ_PARAMETER_SET_V1,
        sccp_source_adapter_fastpq_public_inputs(proof, adapter_transcript_hash),
    );
    batch.push(FastpqStateTransition::new(
        SCCP_SOURCE_ADAPTER_FASTPQ_STATEMENT_KEY_V1.to_vec(),
        Vec::new(),
        statement,
        FastpqOperationKind::MetaSet,
    ));
    batch.push(FastpqStateTransition::new(
        SCCP_SOURCE_ADAPTER_FASTPQ_ADAPTER_KEY_V1.to_vec(),
        Vec::new(),
        adapter_bytes,
        FastpqOperationKind::MetaSet,
    ));
    batch.push(FastpqStateTransition::new(
        SCCP_SOURCE_ADAPTER_FASTPQ_CONTEXT_KEY_V1.to_vec(),
        Vec::new(),
        context,
        FastpqOperationKind::MetaSet,
    ));
    batch.sort();
    Some(batch)
}

fn build_sccp_source_adapter_fastpq_raw_proof_bytes(
    batch: &FastpqTransitionBatch,
) -> Option<Vec<u8>> {
    let proof = FastpqProver::canonical(SCCP_SOURCE_ADAPTER_FASTPQ_PARAMETER_SET_V1)
        .ok()?
        .prove(batch)
        .ok()?;
    to_bytes(&proof).ok()
}

pub fn build_sccp_source_adapter_verification_proof(
    proof: &SccpSourceChainProofEnvelopeV1,
    adapter_proof: &SccpSourceAdapterProofV1,
    adapter_transcript_hash: H256,
) -> Option<SccpSourceAdapterVerificationProofV1> {
    let material = sccp_source_verifier_material_for_domain(proof.source_domain)?;
    build_sccp_source_adapter_verification_proof_with_material(
        proof,
        adapter_proof,
        adapter_transcript_hash,
        &material,
    )
}

pub fn build_sccp_source_adapter_verification_proof_with_material(
    proof: &SccpSourceChainProofEnvelopeV1,
    adapter_proof: &SccpSourceAdapterProofV1,
    adapter_transcript_hash: H256,
    material: &SccpSourceVerifierMaterialV1,
) -> Option<SccpSourceAdapterVerificationProofV1> {
    let verifier_evidence = build_sccp_source_verifier_evidence_with_material(
        proof,
        adapter_proof,
        adapter_transcript_hash,
        material,
    )?;
    let verifier_evidence_hash = sccp_source_verifier_evidence_hash(&verifier_evidence);
    let batch = build_sccp_source_adapter_fastpq_batch(
        proof,
        adapter_proof,
        adapter_transcript_hash,
        verifier_evidence_hash,
    )?;
    let raw_proof_bytes = build_sccp_source_adapter_fastpq_raw_proof_bytes(&batch)?;
    let open = StarkFriOpenProofV1 {
        version: 1,
        public_inputs: sccp_source_adapter_public_input_columns(
            proof,
            adapter_transcript_hash,
            verifier_evidence_hash,
        ),
        envelope_bytes: raw_proof_bytes,
    };
    let env = OpenVerifyEnvelope {
        backend: BackendTag::Stark,
        circuit_id: SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID_V1.to_owned(),
        vk_hash: sccp_source_adapter_fastpq_verifier_commitment(proof)?,
        public_inputs: sccp_source_adapter_open_verify_schema_descriptor(proof),
        proof_bytes: to_bytes(&open).ok()?,
        aux: Vec::new(),
    };
    Some(SccpSourceAdapterVerificationProofV1 {
        version: 1,
        proof_family: SCCP_STARK_FRI_PROOF_FAMILY_V1.to_owned(),
        circuit_id: SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID_V1.to_owned(),
        proof_bytes: to_bytes(&env).ok()?,
    })
}

pub fn canonical_sccp_source_message_inclusion_proof_bytes(
    proof: &SccpSourceMessageInclusionProofV1,
) -> Vec<u8> {
    let mut out = Vec::new();
    push_u8(&mut out, proof.version);
    push_u32(&mut out, proof.source_domain);
    push_u32(&mut out, proof.target_domain);
    out.extend_from_slice(&proof.message_id);
    out.extend_from_slice(&proof.payload_hash);
    out.extend_from_slice(&proof.source_event_digest);
    out.extend_from_slice(&proof.source_event_leaf_hash);
    out.extend_from_slice(&proof.receipt_or_message_root);
    push_u64(&mut out, proof.leaf_index);
    out
}

pub fn sccp_source_chain_proof_envelope_hash(proof: &SccpSourceChainProofEnvelopeV1) -> H256 {
    prefixed_blake2b(
        b"sccp:source-proof-envelope:v1",
        &canonical_sccp_source_chain_proof_envelope_bytes(proof),
    )
}

fn keccak256_bytes(payload: &[u8]) -> H256 {
    let mut keccak = tiny_keccak::Keccak::v256();
    keccak.update(payload);
    let mut out = [0u8; 32];
    keccak.finalize(&mut out);
    out
}

fn abi_word_u64(value: u64) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[24..].copy_from_slice(&value.to_be_bytes());
    out
}

fn abi_word_u32(value: u32) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[28..].copy_from_slice(&value.to_be_bytes());
    out
}

fn abi_word_bytes20(value: &[u8; 20]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[12..].copy_from_slice(value);
    out
}

fn read_be_u32(bytes: &[u8]) -> Option<u32> {
    (bytes.len() == 4).then(|| {
        let mut raw = [0u8; 4];
        raw.copy_from_slice(bytes);
        u32::from_be_bytes(raw)
    })
}

fn read_be_usize(bytes: &[u8]) -> Option<usize> {
    (bytes.len() == 8)
        .then(|| {
            let mut raw = [0u8; 8];
            raw.copy_from_slice(bytes);
            u64::from_be_bytes(raw)
        })?
        .try_into()
        .ok()
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

fn encode_abi_call(signature: &str, args: &[SccpSubmissionArgumentValueV1]) -> Option<Vec<u8>> {
    let selector_hash = keccak256_bytes(signature.as_bytes());
    let head_words = args.iter().try_fold(0usize, |acc, arg| {
        let words = match arg.encoding.as_str() {
            "raw_bytes" | "abi_bytes32" => 1usize,
            "abi_bytes32x6" => 6usize,
            _ => return None,
        };
        acc.checked_add(words)
    })?;
    let head_len = head_words.checked_mul(32)?;
    let mut tail = Vec::new();
    let mut out = Vec::with_capacity(4 + head_len);
    out.extend_from_slice(&selector_hash[..4]);
    for arg in args {
        match arg.encoding.as_str() {
            "raw_bytes" => {
                let offset = head_len.checked_add(tail.len())?;
                out.extend_from_slice(&abi_word_u64(offset as u64));
                tail.extend_from_slice(&abi_padded_bytes(&arg.bytes));
            }
            "abi_bytes32" => {
                if arg.bytes.len() != 32 {
                    return None;
                }
                out.extend_from_slice(&arg.bytes);
            }
            "abi_bytes32x6" => {
                if arg.bytes.len() != 32 * 6 {
                    return None;
                }
                out.extend_from_slice(&arg.bytes);
            }
            _ => return None,
        }
    }
    out.extend_from_slice(&tail);
    Some(out)
}

fn push_scale_compact(out: &mut Vec<u8>, value: u32) {
    if value < (1 << 6) {
        out.push(u8::try_from(value).expect("compact value < 2^6 fits into u8") << 2);
    } else if value < (1 << 14) {
        let encoded =
            (u16::try_from(value).expect("compact value < 2^14 fits into u16") << 2) | 0b01;
        out.extend_from_slice(&encoded.to_le_bytes());
    } else if value < (1 << 30) {
        let encoded = (value << 2) | 0b10;
        out.extend_from_slice(&encoded.to_le_bytes());
    } else {
        let bytes = value.to_le_bytes();
        out.push(
            ((u8::try_from(bytes.len()).expect("u32::to_le_bytes length fits into u8") - 4) << 2)
                | 0b11,
        );
        out.extend_from_slice(&bytes);
    }
}

fn push_scale_vec(out: &mut Vec<u8>, value: &[u8]) {
    push_scale_compact(
        out,
        u32::try_from(value.len()).expect("SCCP SCALE vector length fits into u32"),
    );
    out.extend_from_slice(value);
}

fn sccp_submission_envelope_encoding(template: &SccpCounterpartySubmissionTemplateV1) -> String {
    template.encoding.clone()
}

fn encode_sccp_submission_envelope(
    template: &SccpCounterpartySubmissionTemplateV1,
    args: &[SccpSubmissionArgumentValueV1],
) -> Vec<u8> {
    match template.encoding.as_str() {
        "abi_tuple_v1" | "tron_abi_tuple_v1" => {
            encode_abi_call(&template.verifier_entrypoint, args).unwrap_or_default()
        }
        "borsh_instruction_v1" => {
            let arg_bytes = args.iter().map(|arg| arg.bytes.clone()).collect::<Vec<_>>();
            let mut out = Vec::new();
            push_vec(&mut out, template.verifier_entrypoint.as_bytes());
            for value in &arg_bytes {
                push_vec(&mut out, value);
            }
            out
        }
        "ton_message_body_boc_v1" => args
            .first()
            .filter(|arg| args.len() == 1 && arg.key == "message_body_boc")
            .map(|arg| arg.bytes.clone())
            .unwrap_or_default(),
        "scale_call_v1" => {
            let arg_bytes = args.iter().map(|arg| arg.bytes.clone()).collect::<Vec<_>>();
            let mut out = Vec::new();
            push_scale_vec(&mut out, template.verifier_entrypoint.as_bytes());
            for value in &arg_bytes {
                push_scale_vec(&mut out, value);
            }
            out
        }
        _ => {
            let arg_bytes = args.iter().map(|arg| arg.bytes.clone()).collect::<Vec<_>>();
            let mut out = Vec::new();
            push_vec(&mut out, template.verifier_entrypoint.as_bytes());
            for value in &arg_bytes {
                push_vec(&mut out, value);
            }
            out
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TonCellV1 {
    data: Vec<u8>,
    refs: Vec<usize>,
}

fn ton_min_size_bytes(value: usize) -> Option<usize> {
    let value = u128::try_from(value).ok()?;
    (1..=7).find(|size| value <= ((1u128 << (*size * 8)) - 1))
}

fn ton_push_sized_uint(out: &mut Vec<u8>, value: usize, size: usize) -> Option<()> {
    if !(1..=7).contains(&size) {
        return None;
    }
    let value = u64::try_from(value).ok()?;
    let bytes = value.to_be_bytes();
    out.extend_from_slice(&bytes[bytes.len().checked_sub(size)?..]);
    Some(())
}

fn ton_cell_serialized_len(cell: &TonCellV1, size_bytes: usize) -> Option<usize> {
    if cell.data.len() > SCCP_TON_MAX_CELL_DATA_BYTES || cell.refs.len() > SCCP_TON_MAX_REFS {
        return None;
    }
    2usize
        .checked_add(cell.data.len())?
        .checked_add(cell.refs.len().checked_mul(size_bytes)?)
}

fn ton_serialize_cells(cells: &[TonCellV1], size_bytes: usize) -> Option<Vec<u8>> {
    let total_len = cells.iter().try_fold(0usize, |sum, cell| {
        sum.checked_add(ton_cell_serialized_len(cell, size_bytes)?)
    })?;
    let mut out = Vec::with_capacity(total_len);
    for cell in cells {
        if cell.data.len() > SCCP_TON_MAX_CELL_DATA_BYTES || cell.refs.len() > SCCP_TON_MAX_REFS {
            return None;
        }
        out.push(u8::try_from(cell.refs.len()).ok()?);
        out.push(u8::try_from(cell.data.len().checked_mul(2)?).ok()?);
        out.extend_from_slice(&cell.data);
        for ref_index in &cell.refs {
            if *ref_index >= cells.len() {
                return None;
            }
            ton_push_sized_uint(&mut out, *ref_index, size_bytes)?;
        }
    }
    Some(out)
}

fn encode_ton_boc_single_root(cells: &[TonCellV1], root_index: usize) -> Option<Vec<u8>> {
    if cells.is_empty() || root_index >= cells.len() {
        return None;
    }
    let size_bytes = ton_min_size_bytes(cells.len().saturating_sub(1).max(cells.len()))?;
    let cells_bytes = ton_serialize_cells(cells, size_bytes)?;
    let offset_bytes = ton_min_size_bytes(cells_bytes.len())?;
    let mut out = Vec::with_capacity(
        SCCP_TON_BOC_MAGIC
            .len()
            .checked_add(2)?
            .checked_add(size_bytes.checked_mul(4)?)?
            .checked_add(offset_bytes)?
            .checked_add(cells_bytes.len())?,
    );
    out.extend_from_slice(&SCCP_TON_BOC_MAGIC);
    out.push(u8::try_from(size_bytes).ok()?);
    out.push(u8::try_from(offset_bytes).ok()?);
    ton_push_sized_uint(&mut out, cells.len(), size_bytes)?;
    ton_push_sized_uint(&mut out, 1, size_bytes)?;
    ton_push_sized_uint(&mut out, 0, size_bytes)?;
    ton_push_sized_uint(&mut out, cells_bytes.len(), offset_bytes)?;
    ton_push_sized_uint(&mut out, root_index, size_bytes)?;
    out.extend_from_slice(&cells_bytes);
    Some(out)
}

fn ton_push_snake_cells(cells: &mut Vec<TonCellV1>, bytes: &[u8]) -> Option<usize> {
    let start = cells.len();
    if bytes.is_empty() {
        cells.push(TonCellV1 {
            data: Vec::new(),
            refs: Vec::new(),
        });
        return Some(start);
    }
    let chunk_count = bytes.len().div_ceil(SCCP_TON_MAX_CELL_DATA_BYTES);
    for index in 0..chunk_count {
        let chunk_start = index.checked_mul(SCCP_TON_MAX_CELL_DATA_BYTES)?;
        let chunk_end = chunk_start
            .checked_add(SCCP_TON_MAX_CELL_DATA_BYTES)?
            .min(bytes.len());
        let refs = if index + 1 == chunk_count {
            Vec::new()
        } else {
            vec![start.checked_add(index)?.checked_add(1)?]
        };
        cells.push(TonCellV1 {
            data: bytes[chunk_start..chunk_end].to_vec(),
            refs,
        });
    }
    Some(start)
}

pub fn sccp_ton_submission_query_id(public_inputs: &SccpMessageTransparentPublicInputsV1) -> u64 {
    let mut raw = [0u8; 8];
    raw.copy_from_slice(&public_inputs.message_id[..8]);
    u64::from_be_bytes(raw)
}

fn canonical_sccp_ton_submission_metadata_bytes(
    manifest: &SccpProofManifestV1,
    destination_binding: &SccpDestinationBindingV1,
    public_inputs: &SccpMessageTransparentPublicInputsV1,
    statement_hash: H256,
) -> Vec<u8> {
    let mut out = Vec::new();
    push_u8(&mut out, 1);
    push_u32(&mut out, manifest.local_domain);
    push_u32(&mut out, manifest.counterparty_domain);
    push_u8(
        &mut out,
        sccp_proof_security_model_code(manifest.security_model),
    );
    push_u8(
        &mut out,
        sccp_anchor_governance_code(manifest.anchor_governance),
    );
    push_u8(
        &mut out,
        sccp_proof_verifier_target_code(manifest.verifier_target),
    );
    push_u8(
        &mut out,
        sccp_verifier_backend_family_code(manifest.verifier_backend.family),
    );
    push_vec(&mut out, manifest.proof_family.as_bytes());
    push_vec(&mut out, manifest.verifier_backend.key.as_bytes());
    push_vec(&mut out, manifest.message_backend.as_bytes());
    push_vec(&mut out, manifest.registry_backend.as_bytes());
    push_vec(&mut out, manifest.manifest_seed.as_bytes());
    push_vec(&mut out, destination_binding.key.as_bytes());
    out.extend_from_slice(&destination_binding.binding_hash);
    out.extend_from_slice(&statement_hash);
    out.extend_from_slice(&canonical_sccp_message_transparent_public_inputs_bytes(
        public_inputs,
    ));
    out
}

pub fn build_sccp_ton_message_body_boc(
    manifest: &SccpProofManifestV1,
    destination_binding: &SccpDestinationBindingV1,
    public_inputs: &SccpMessageTransparentPublicInputsV1,
    proof_bytes: &[u8],
    bundle_bytes: &[u8],
    statement_hash: H256,
) -> Option<Vec<u8>> {
    if manifest.verifier_target != SccpProofVerifierTargetV1::TonContract
        || !sccp_destination_binding_metadata_is_valid(destination_binding)
    {
        return None;
    }
    let public_inputs_bytes = canonical_sccp_message_transparent_public_inputs_bytes(public_inputs);
    let metadata = canonical_sccp_ton_submission_metadata_bytes(
        manifest,
        destination_binding,
        public_inputs,
        statement_hash,
    );
    let query_id = sccp_ton_submission_query_id(public_inputs);
    let mut root_data = Vec::with_capacity(4 + 8 + 2 + 32 + 32);
    root_data.extend_from_slice(&SCCP_TON_SUBMIT_OP_V1.to_be_bytes());
    root_data.extend_from_slice(&query_id.to_be_bytes());
    root_data.extend_from_slice(&SCCP_TON_MESSAGE_SCHEMA_VERSION_V1.to_be_bytes());
    root_data.extend_from_slice(&statement_hash);
    root_data.extend_from_slice(&destination_binding.binding_hash);

    let mut cells = vec![TonCellV1 {
        data: root_data,
        refs: Vec::new(),
    }];
    let public_inputs_root = ton_push_snake_cells(&mut cells, &public_inputs_bytes)?;
    let proof_root = ton_push_snake_cells(&mut cells, proof_bytes)?;
    let bundle_root = ton_push_snake_cells(&mut cells, bundle_bytes)?;
    let metadata_root = ton_push_snake_cells(&mut cells, &metadata)?;
    cells[0].refs = vec![public_inputs_root, proof_root, bundle_root, metadata_root];
    encode_ton_boc_single_root(&cells, 0)
}

pub fn build_sccp_ton_internal_message_submission_payload(
    manifest: &SccpProofManifestV1,
    proof_bytes: &[u8],
    public_inputs: &SccpMessageTransparentPublicInputsV1,
    bundle: &NexusSccpMessageProofV1,
    statement_hash: H256,
    destination_binding: &SccpDestinationBindingV1,
) -> Option<SccpTonInternalMessageSubmissionPayloadV1> {
    let public_inputs_bytes = canonical_sccp_message_transparent_public_inputs_bytes(public_inputs);
    let bundle_bytes = canonical_nexus_sccp_message_bundle_bytes(bundle);
    let message_body_boc = build_sccp_ton_message_body_boc(
        manifest,
        destination_binding,
        public_inputs,
        proof_bytes,
        &bundle_bytes,
        statement_hash,
    )?;
    Some(SccpTonInternalMessageSubmissionPayloadV1 {
        message_body_boc,
        query_id: sccp_ton_submission_query_id(public_inputs),
        destination_binding: destination_binding.clone(),
        destination_binding_hash: destination_binding.binding_hash,
        proof_bytes: proof_bytes.to_vec(),
        public_inputs_bytes,
        bundle_bytes,
        statement_hash,
    })
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

fn sccp_evm_public_input_word_struct(
    public_inputs: &SccpMessageTransparentPublicInputsV1,
) -> SccpEvmWordPublicInputsV1 {
    let words = sccp_evm_public_input_words(public_inputs);
    SccpEvmWordPublicInputsV1 {
        message_id: words[0],
        payload_hash: words[1],
        target_domain_word: words[2],
        commitment_root: words[3],
        finality_height_word: words[4],
        finality_block_hash: words[5],
    }
}

fn flatten_h256_words(words: &[H256]) -> Vec<u8> {
    let mut out = Vec::with_capacity(words.len() * 32);
    for word in words {
        out.extend_from_slice(word);
    }
    out
}

fn sccp_evm_public_input_words_bytes(
    public_inputs: &SccpMessageTransparentPublicInputsV1,
) -> Vec<u8> {
    flatten_h256_words(&sccp_evm_public_input_words(public_inputs))
}

fn encode_sccp_evm_attestation_envelope(
    envelope: &SccpEvmAttestationEnvelopeV1,
) -> Option<Vec<u8>> {
    let signatures = envelope
        .signatures
        .iter()
        .try_fold(Vec::new(), |mut out, signature| {
            if signature.signature_bytes.len() != 65 {
                return None;
            }
            out.extend_from_slice(&signature.signature_bytes);
            Some(out)
        })?;
    let head_len = 32 * 7;
    let mut out = Vec::with_capacity(head_len + 32 + signatures.len());
    out.extend_from_slice(&abi_word_u32(u32::from(envelope.version)));
    out.extend_from_slice(&envelope.message_id);
    out.extend_from_slice(&abi_word_u32(envelope.source_domain));
    out.extend_from_slice(&envelope.commitment_root);
    out.extend_from_slice(&envelope.native_proof_hash);
    out.extend_from_slice(&envelope.destination_binding_hash);
    out.extend_from_slice(&abi_word_u64(head_len as u64));
    out.extend_from_slice(&abi_padded_bytes(&signatures));
    Some(out)
}

fn decode_sccp_evm_attestation_envelope(payload: &[u8]) -> Option<SccpEvmAttestationEnvelopeV1> {
    if payload.len() < 32 * 7 {
        return None;
    }
    let version = read_be_u32(&payload[28..32])?;
    let mut message_id = [0u8; 32];
    message_id.copy_from_slice(&payload[32..64]);
    let source_domain = read_be_u32(&payload[92..96])?;
    let mut commitment_root = [0u8; 32];
    commitment_root.copy_from_slice(&payload[96..128]);
    let mut native_proof_hash = [0u8; 32];
    native_proof_hash.copy_from_slice(&payload[128..160]);
    let mut destination_binding_hash = [0u8; 32];
    destination_binding_hash.copy_from_slice(&payload[160..192]);
    let offset = read_be_usize(&payload[216..224])?;
    if offset != 32 * 7 || payload.len() < offset + 32 {
        return None;
    }
    let signatures_len = read_be_usize(&payload[offset + 24..offset + 32])?;
    let signatures_start = offset + 32;
    let signatures_end = signatures_start.checked_add(signatures_len)?;
    let padded_len = signatures_len
        .checked_add(31)?
        .checked_div(32)?
        .checked_mul(32)?;
    let padded_end = signatures_start.checked_add(padded_len)?;
    if padded_end != payload.len()
        || signatures_end > padded_end
        || signatures_len % 65 != 0
        || payload[signatures_end..padded_end]
            .iter()
            .any(|byte| *byte != 0)
    {
        return None;
    }
    let signatures = payload[signatures_start..signatures_end]
        .chunks_exact(65)
        .map(|chunk| SccpEvmAttestationSignatureV1 {
            signer_address: Vec::new(),
            signature_bytes: chunk.to_vec(),
        })
        .collect::<Vec<_>>();
    Some(SccpEvmAttestationEnvelopeV1 {
        version: u8::try_from(version).ok()?,
        message_id,
        source_domain,
        commitment_root,
        native_proof_hash,
        destination_binding_hash,
        signatures,
    })
}

fn sccp_evm_public_inputs_hash(public_inputs: &SccpMessageTransparentPublicInputsV1) -> H256 {
    keccak256_bytes(&sccp_evm_public_input_words_bytes(public_inputs))
}

fn sccp_evm_attestation_domain_separator() -> H256 {
    keccak256_bytes(SCCP_EVM_ATTESTATION_DOMAIN_PREFIX_V1)
}

fn sccp_evm_native_proof_hash(native_proof_bytes: &[u8]) -> H256 {
    keccak256_bytes(native_proof_bytes)
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

fn abi_g1_point_is_structurally_valid(point: &[H256; 2]) -> bool {
    point.iter().all(abi_word_is_bn254_base_field_element) && point.iter().any(h256_is_nonzero)
}

fn abi_g2_point_is_structurally_valid(point: &[H256; 4]) -> bool {
    point.iter().all(abi_word_is_bn254_base_field_element) && point.iter().any(h256_is_nonzero)
}

fn abi_word_at(payload: &[u8], index: usize) -> Option<H256> {
    let start = index.checked_mul(32)?;
    let end = start.checked_add(32)?;
    let mut word = [0u8; 32];
    word.copy_from_slice(payload.get(start..end)?);
    Some(word)
}

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

fn verify_sccp_evm_groth16_bn254_proof_binding(
    manifest: &SccpProofManifestV1,
    public_inputs: &SccpMessageTransparentPublicInputsV1,
    proof_bytes: &[u8],
) -> bool {
    verify_sccp_groth16_bn254_proof_binding(
        manifest,
        public_inputs,
        proof_bytes,
        SccpVerifierBackendFamilyV1::EvmGroth16Bn254,
        SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1,
    )
}

fn verify_sccp_tron_groth16_bn254_proof_binding(
    manifest: &SccpProofManifestV1,
    public_inputs: &SccpMessageTransparentPublicInputsV1,
    proof_bytes: &[u8],
) -> bool {
    verify_sccp_groth16_bn254_proof_binding(
        manifest,
        public_inputs,
        proof_bytes,
        SccpVerifierBackendFamilyV1::TronGroth16Bn254,
        SCCP_TRON_GROTH16_BN254_PROOF_BACKEND_V1,
    )
}

fn verify_sccp_groth16_bn254_proof_binding(
    manifest: &SccpProofManifestV1,
    public_inputs: &SccpMessageTransparentPublicInputsV1,
    proof_bytes: &[u8],
    expected_family: SccpVerifierBackendFamilyV1,
    expected_key: &str,
) -> bool {
    if manifest.verifier_backend.family != expected_family
        || manifest.verifier_backend.key != expected_key
    {
        return false;
    }
    let Some(proof) = decode_sccp_evm_groth16_bn254_proof_bytes(proof_bytes) else {
        return false;
    };
    proof.version == 1
        && proof.message_id == public_inputs.message_id
        && proof.source_domain == manifest.local_domain
        && proof.commitment_root == public_inputs.commitment_root
        && encode_sccp_evm_groth16_bn254_proof_bytes(&proof) == proof_bytes
}

fn sccp_evm_destination_binding_hash(
    network_id: H256,
    source_domain: u32,
    target_domain: u32,
    verifier_backend_hash: H256,
    proof_family_hash: H256,
    verifier_address: [u8; 20],
    bridge_address: [u8; 20],
) -> H256 {
    let mut payload = Vec::with_capacity(32 * 8);
    payload.extend_from_slice(&keccak256_bytes(
        SCCP_EVM_DESTINATION_BINDING_DOMAIN_PREFIX_V1,
    ));
    payload.extend_from_slice(&verifier_backend_hash);
    payload.extend_from_slice(&proof_family_hash);
    payload.extend_from_slice(&network_id);
    payload.extend_from_slice(&abi_word_u32(source_domain));
    payload.extend_from_slice(&abi_word_u32(target_domain));
    payload.extend_from_slice(&abi_word_bytes20(&verifier_address));
    payload.extend_from_slice(&abi_word_bytes20(&bridge_address));
    keccak256_bytes(&payload)
}

pub fn build_sccp_evm_destination_binding(
    manifest: &SccpProofManifestV1,
    network_id: H256,
    verifier_address: [u8; 20],
    bridge_address: [u8; 20],
) -> SccpDestinationBindingV1 {
    let verifier_backend_hash = keccak256_bytes(manifest.verifier_backend.key.as_bytes());
    let proof_family_hash = keccak256_bytes(manifest.proof_family.as_bytes());
    SccpDestinationBindingV1 {
        version: 1,
        key: format!(
            "evm:{}:{}:{}:0x{}:0x{}",
            manifest.local_domain,
            manifest.counterparty_domain,
            encode_lower_hex(&network_id),
            encode_lower_hex(&verifier_address),
            encode_lower_hex(&bridge_address)
        ),
        binding_hash: sccp_evm_destination_binding_hash(
            network_id,
            manifest.local_domain,
            manifest.counterparty_domain,
            verifier_backend_hash,
            proof_family_hash,
            verifier_address,
            bridge_address,
        ),
    }
}

fn sccp_evm_attestation_digest(
    envelope: &SccpEvmAttestationEnvelopeV1,
    public_inputs_hash: H256,
    statement_hash: H256,
) -> H256 {
    let mut payload = Vec::with_capacity(32 * 7);
    payload.extend_from_slice(&sccp_evm_attestation_domain_separator());
    payload.extend_from_slice(&envelope.message_id);
    payload.extend_from_slice(&abi_word_u32(envelope.source_domain));
    payload.extend_from_slice(&envelope.commitment_root);
    payload.extend_from_slice(&public_inputs_hash);
    payload.extend_from_slice(&statement_hash);
    payload.extend_from_slice(&envelope.native_proof_hash);
    payload.extend_from_slice(&envelope.destination_binding_hash);
    keccak256_bytes(&payload)
}

fn sccp_evm_signer_public_key_bytes(signer: &KeyPair) -> Option<Vec<u8>> {
    (signer.algorithm() == Algorithm::Secp256k1).then(|| signer.public_key().to_bytes().1.to_vec())
}

fn sccp_evm_signer_address(signer: &KeyPair) -> Option<[u8; 20]> {
    let public_key =
        EcdsaSecp256k1Sha256::parse_public_key(&sccp_evm_signer_public_key_bytes(signer)?).ok()?;
    Some(EcdsaSecp256k1Sha256::evm_address(&public_key))
}

fn sccp_evm_sign_digest(signer: &KeyPair, digest: &H256) -> Option<[u8; 65]> {
    if signer.algorithm() != Algorithm::Secp256k1 {
        return None;
    }
    let secret_key_bytes = signer.private_key().to_bytes().1;
    let secret_key = EcdsaSecp256k1Sha256::parse_private_key(&secret_key_bytes).ok()?;
    EcdsaSecp256k1Sha256::sign_prehash_recoverable(digest, &secret_key).ok()
}

fn build_sccp_evm_contract_submission_payload(
    manifest: &SccpProofManifestV1,
    native_proof_bytes: &[u8],
    public_inputs: &SccpMessageTransparentPublicInputsV1,
    statement_hash: H256,
    destination_binding: &SccpDestinationBindingV1,
    signer: &KeyPair,
) -> Option<SccpEvmContractSubmissionPayloadV1> {
    // TODO: add a separate Groth16/bn254 destination proof package once the
    // immutable production verifier contract and verifying key are generated.
    if manifest.verifier_backend.family != SccpVerifierBackendFamilyV1::EvmSecp256k1Keccak
        || manifest.verifier_backend.key != SCCP_EVM_SECP256K1_PROOF_BACKEND_V1
    {
        return None;
    }
    let public_inputs_hash = sccp_evm_public_inputs_hash(public_inputs);
    let mut attestation = SccpEvmAttestationEnvelopeV1 {
        version: 1,
        message_id: public_inputs.message_id,
        source_domain: manifest.local_domain,
        commitment_root: public_inputs.commitment_root,
        native_proof_hash: sccp_evm_native_proof_hash(native_proof_bytes),
        destination_binding_hash: destination_binding.binding_hash,
        signatures: Vec::new(),
    };
    let digest = sccp_evm_attestation_digest(&attestation, public_inputs_hash, statement_hash);
    let signature_bytes = sccp_evm_sign_digest(signer, &digest)?;
    let signer_address = sccp_evm_signer_address(signer)?;
    attestation.signatures.push(SccpEvmAttestationSignatureV1 {
        signer_address: signer_address.to_vec(),
        signature_bytes: signature_bytes.to_vec(),
    });
    let proof_bytes = encode_sccp_evm_attestation_envelope(&attestation)?;
    Some(SccpEvmContractSubmissionPayloadV1 {
        proof_bytes,
        public_inputs: sccp_evm_public_input_word_struct(public_inputs),
        public_inputs_hash,
        statement_hash,
        destination_binding: destination_binding.clone(),
        attestation,
    })
}

fn build_sccp_evm_groth16_contract_submission_payload(
    manifest: &SccpProofManifestV1,
    groth16_proof_bytes: &[u8],
    public_inputs: &SccpMessageTransparentPublicInputsV1,
    statement_hash: H256,
    destination_binding: &SccpDestinationBindingV1,
) -> Option<SccpEvmGroth16ContractSubmissionPayloadV1> {
    if manifest.verifier_backend.family != SccpVerifierBackendFamilyV1::EvmGroth16Bn254
        || manifest.verifier_backend.key != SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1
        || !verify_sccp_evm_groth16_bn254_proof_binding(
            manifest,
            public_inputs,
            groth16_proof_bytes,
        )
    {
        return None;
    }

    Some(SccpEvmGroth16ContractSubmissionPayloadV1 {
        proof_bytes: groth16_proof_bytes.to_vec(),
        public_inputs: sccp_evm_public_input_word_struct(public_inputs),
        statement_hash,
        destination_binding: destination_binding.clone(),
    })
}

fn build_sccp_tron_groth16_contract_submission_payload(
    manifest: &SccpProofManifestV1,
    groth16_proof_bytes: &[u8],
    public_inputs: &SccpMessageTransparentPublicInputsV1,
    statement_hash: H256,
    destination_binding: &SccpDestinationBindingV1,
) -> Option<SccpTronContractSubmissionPayloadV1> {
    if !verify_sccp_tron_groth16_bn254_proof_binding(manifest, public_inputs, groth16_proof_bytes) {
        return None;
    }

    Some(SccpTronContractSubmissionPayloadV1 {
        proof_bytes: groth16_proof_bytes.to_vec(),
        public_inputs: sccp_evm_public_input_word_struct(public_inputs),
        statement_hash,
        destination_binding: destination_binding.clone(),
    })
}

fn build_sccp_platform_submission_payload(
    manifest: &SccpProofManifestV1,
    native_proof_bytes: &[u8],
    public_inputs: &SccpMessageTransparentPublicInputsV1,
    bundle: &NexusSccpMessageProofV1,
    destination_binding: Option<&SccpDestinationBindingV1>,
    signer: Option<&KeyPair>,
    source_material: Option<&SccpSourceVerifierMaterialV1>,
) -> Option<SccpPlatformSubmissionPayloadV1> {
    if destination_binding.is_some()
        && !matches!(
            manifest.verifier_target,
            SccpProofVerifierTargetV1::EvmContract
                | SccpProofVerifierTargetV1::TonContract
                | SccpProofVerifierTargetV1::TronContract
        )
    {
        return None;
    }
    if signer.is_some()
        && !matches!(
            manifest.verifier_target,
            SccpProofVerifierTargetV1::EvmContract
        )
    {
        return None;
    }
    let canonical_public_inputs =
        canonical_sccp_message_transparent_public_inputs_bytes(public_inputs);
    let canonical_bundle = canonical_nexus_sccp_message_bundle_bytes(bundle);
    let inner =
        build_sccp_message_transparent_inner_proof_internal(bundle, manifest, source_material)?;
    Some(match manifest.verifier_target {
        SccpProofVerifierTargetV1::EvmContract
            if manifest.verifier_backend.family == SccpVerifierBackendFamilyV1::EvmGroth16Bn254 =>
        {
            if signer.is_some() {
                return None;
            }
            SccpPlatformSubmissionPayloadV1::EvmGroth16ContractCall(
                build_sccp_evm_groth16_contract_submission_payload(
                    manifest,
                    native_proof_bytes,
                    public_inputs,
                    inner.statement_hash,
                    destination_binding?,
                )?,
            )
        }
        SccpProofVerifierTargetV1::EvmContract => SccpPlatformSubmissionPayloadV1::EvmContractCall(
            build_sccp_evm_contract_submission_payload(
                manifest,
                native_proof_bytes,
                public_inputs,
                inner.statement_hash,
                destination_binding?,
                signer?,
            )?,
        ),
        SccpProofVerifierTargetV1::SolanaProgram => {
            SccpPlatformSubmissionPayloadV1::SolanaProgramInstruction(
                SccpSolanaProgramSubmissionPayloadV1 {
                    proof_bytes: native_proof_bytes.to_vec(),
                    public_inputs_bytes: canonical_public_inputs,
                    bundle_bytes: canonical_bundle,
                },
            )
        }
        SccpProofVerifierTargetV1::TonContract => {
            SccpPlatformSubmissionPayloadV1::TonInternalMessage(
                build_sccp_ton_internal_message_submission_payload(
                    manifest,
                    native_proof_bytes,
                    public_inputs,
                    bundle,
                    inner.statement_hash,
                    destination_binding.unwrap_or(&manifest.destination_binding),
                )?,
            )
        }
        SccpProofVerifierTargetV1::TronContract => {
            SccpPlatformSubmissionPayloadV1::TronContractCall(
                build_sccp_tron_groth16_contract_submission_payload(
                    manifest,
                    native_proof_bytes,
                    public_inputs,
                    inner.statement_hash,
                    destination_binding.unwrap_or(&manifest.destination_binding),
                )?,
            )
        }
        SccpProofVerifierTargetV1::SubstrateRuntime => {
            SccpPlatformSubmissionPayloadV1::SubstrateRuntimeCall(
                SccpSubstrateRuntimeSubmissionPayloadV1 {
                    proof_bytes: native_proof_bytes.to_vec(),
                    public_inputs_bytes: canonical_public_inputs,
                    bundle_bytes: canonical_bundle,
                },
            )
        }
    })
}

fn sccp_submission_argument_values(
    template: &SccpCounterpartySubmissionTemplateV1,
    platform_payload: &SccpPlatformSubmissionPayloadV1,
) -> Option<Vec<SccpSubmissionArgumentValueV1>> {
    template
        .required_arguments
        .iter()
        .map(|argument| {
            let (encoding, bytes) = match (platform_payload, argument.key.as_str()) {
                (SccpPlatformSubmissionPayloadV1::EvmContractCall(payload), "proof_bytes") => {
                    ("raw_bytes".to_owned(), payload.proof_bytes.clone())
                }
                (
                    SccpPlatformSubmissionPayloadV1::EvmGroth16ContractCall(payload),
                    "proof_bytes",
                ) => ("raw_bytes".to_owned(), payload.proof_bytes.clone()),
                (SccpPlatformSubmissionPayloadV1::EvmContractCall(payload), "public_inputs") => (
                    "abi_bytes32x6".to_owned(),
                    flatten_h256_words(&[
                        payload.public_inputs.message_id,
                        payload.public_inputs.payload_hash,
                        payload.public_inputs.target_domain_word,
                        payload.public_inputs.commitment_root,
                        payload.public_inputs.finality_height_word,
                        payload.public_inputs.finality_block_hash,
                    ]),
                ),
                (
                    SccpPlatformSubmissionPayloadV1::EvmGroth16ContractCall(payload),
                    "public_inputs",
                ) => (
                    "abi_bytes32x6".to_owned(),
                    flatten_h256_words(&[
                        payload.public_inputs.message_id,
                        payload.public_inputs.payload_hash,
                        payload.public_inputs.target_domain_word,
                        payload.public_inputs.commitment_root,
                        payload.public_inputs.finality_height_word,
                        payload.public_inputs.finality_block_hash,
                    ]),
                ),
                (SccpPlatformSubmissionPayloadV1::EvmContractCall(payload), "statement_hash") => {
                    ("abi_bytes32".to_owned(), payload.statement_hash.to_vec())
                }
                (
                    SccpPlatformSubmissionPayloadV1::EvmGroth16ContractCall(payload),
                    "statement_hash",
                ) => ("abi_bytes32".to_owned(), payload.statement_hash.to_vec()),
                (
                    SccpPlatformSubmissionPayloadV1::SolanaProgramInstruction(payload),
                    "proof_bytes",
                ) => ("raw_bytes".to_owned(), payload.proof_bytes.clone()),
                (
                    SccpPlatformSubmissionPayloadV1::SolanaProgramInstruction(payload),
                    "public_inputs",
                ) => ("raw_bytes".to_owned(), payload.public_inputs_bytes.clone()),
                (
                    SccpPlatformSubmissionPayloadV1::SolanaProgramInstruction(payload),
                    "bundle_bytes",
                ) => ("raw_bytes".to_owned(), payload.bundle_bytes.clone()),
                (
                    SccpPlatformSubmissionPayloadV1::TonInternalMessage(payload),
                    "message_body_boc",
                ) => ("ton_boc".to_owned(), payload.message_body_boc.clone()),
                (SccpPlatformSubmissionPayloadV1::TronContractCall(payload), "proof_bytes") => {
                    ("raw_bytes".to_owned(), payload.proof_bytes.clone())
                }
                (SccpPlatformSubmissionPayloadV1::TronContractCall(payload), "public_inputs") => (
                    "abi_bytes32x6".to_owned(),
                    flatten_h256_words(&[
                        payload.public_inputs.message_id,
                        payload.public_inputs.payload_hash,
                        payload.public_inputs.target_domain_word,
                        payload.public_inputs.commitment_root,
                        payload.public_inputs.finality_height_word,
                        payload.public_inputs.finality_block_hash,
                    ]),
                ),
                (SccpPlatformSubmissionPayloadV1::TronContractCall(payload), "statement_hash") => {
                    ("abi_bytes32".to_owned(), payload.statement_hash.to_vec())
                }
                (SccpPlatformSubmissionPayloadV1::SubstrateRuntimeCall(payload), "proof_bytes") => {
                    ("raw_bytes".to_owned(), payload.proof_bytes.clone())
                }
                (
                    SccpPlatformSubmissionPayloadV1::SubstrateRuntimeCall(payload),
                    "public_inputs",
                ) => ("raw_bytes".to_owned(), payload.public_inputs_bytes.clone()),
                (
                    SccpPlatformSubmissionPayloadV1::SubstrateRuntimeCall(payload),
                    "bundle_bytes",
                ) => ("raw_bytes".to_owned(), payload.bundle_bytes.clone()),
                _ => return None,
            };
            Some(SccpSubmissionArgumentValueV1 {
                key: argument.key.clone(),
                encoding,
                bytes,
            })
        })
        .collect::<Option<Vec<_>>>()
}

pub fn build_sccp_counterparty_submission_package_with_signer(
    bundle: &NexusSccpMessageProofV1,
    manifest: &SccpProofManifestV1,
    proof_bytes: &[u8],
    signer: &KeyPair,
) -> Option<SccpCounterpartySubmissionPackageV1> {
    build_sccp_counterparty_submission_package_internal(
        bundle,
        manifest,
        proof_bytes,
        None,
        Some(signer),
        false,
        None,
    )
}

pub fn build_sccp_counterparty_submission_package_with_signer_allow_unready(
    bundle: &NexusSccpMessageProofV1,
    manifest: &SccpProofManifestV1,
    proof_bytes: &[u8],
    signer: &KeyPair,
    allow_unready: bool,
) -> Option<SccpCounterpartySubmissionPackageV1> {
    build_sccp_counterparty_submission_package_internal(
        bundle,
        manifest,
        proof_bytes,
        None,
        Some(signer),
        allow_unready,
        None,
    )
}

pub fn build_sccp_counterparty_submission_package_with_destination_binding_and_signer(
    bundle: &NexusSccpMessageProofV1,
    manifest: &SccpProofManifestV1,
    proof_bytes: &[u8],
    destination_binding: &SccpDestinationBindingV1,
    signer: &KeyPair,
) -> Option<SccpCounterpartySubmissionPackageV1> {
    build_sccp_counterparty_submission_package_internal(
        bundle,
        manifest,
        proof_bytes,
        Some(destination_binding),
        Some(signer),
        false,
        None,
    )
}

pub fn build_sccp_counterparty_submission_package_with_destination_binding(
    bundle: &NexusSccpMessageProofV1,
    manifest: &SccpProofManifestV1,
    proof_bytes: &[u8],
    destination_binding: &SccpDestinationBindingV1,
) -> Option<SccpCounterpartySubmissionPackageV1> {
    build_sccp_counterparty_submission_package_internal(
        bundle,
        manifest,
        proof_bytes,
        Some(destination_binding),
        None,
        false,
        None,
    )
}

pub fn build_sccp_counterparty_submission_package_with_destination_binding_allow_unready(
    bundle: &NexusSccpMessageProofV1,
    manifest: &SccpProofManifestV1,
    proof_bytes: &[u8],
    destination_binding: &SccpDestinationBindingV1,
    allow_unready: bool,
) -> Option<SccpCounterpartySubmissionPackageV1> {
    build_sccp_counterparty_submission_package_internal(
        bundle,
        manifest,
        proof_bytes,
        Some(destination_binding),
        None,
        allow_unready,
        None,
    )
}

pub fn build_sccp_counterparty_submission_package_with_destination_binding_and_signer_allow_unready(
    bundle: &NexusSccpMessageProofV1,
    manifest: &SccpProofManifestV1,
    proof_bytes: &[u8],
    destination_binding: &SccpDestinationBindingV1,
    signer: &KeyPair,
    allow_unready: bool,
) -> Option<SccpCounterpartySubmissionPackageV1> {
    build_sccp_counterparty_submission_package_internal(
        bundle,
        manifest,
        proof_bytes,
        Some(destination_binding),
        Some(signer),
        allow_unready,
        None,
    )
}

pub fn build_sccp_counterparty_submission_package(
    bundle: &NexusSccpMessageProofV1,
    manifest: &SccpProofManifestV1,
    proof_bytes: &[u8],
) -> Option<SccpCounterpartySubmissionPackageV1> {
    build_sccp_counterparty_submission_package_internal(
        bundle,
        manifest,
        proof_bytes,
        None,
        None,
        false,
        None,
    )
}

pub fn build_sccp_counterparty_submission_package_allow_unready(
    bundle: &NexusSccpMessageProofV1,
    manifest: &SccpProofManifestV1,
    proof_bytes: &[u8],
    allow_unready: bool,
) -> Option<SccpCounterpartySubmissionPackageV1> {
    build_sccp_counterparty_submission_package_internal(
        bundle,
        manifest,
        proof_bytes,
        None,
        None,
        allow_unready,
        None,
    )
}

pub fn build_sccp_counterparty_submission_package_with_source_verifier_material(
    bundle: &NexusSccpMessageProofV1,
    manifest: &SccpProofManifestV1,
    proof_bytes: &[u8],
    material: &SccpSourceVerifierMaterialV1,
) -> Option<SccpCounterpartySubmissionPackageV1> {
    build_sccp_counterparty_submission_package_internal(
        bundle,
        manifest,
        proof_bytes,
        None,
        None,
        false,
        Some(material),
    )
}

fn build_sccp_counterparty_submission_package_internal(
    bundle: &NexusSccpMessageProofV1,
    manifest: &SccpProofManifestV1,
    proof_bytes: &[u8],
    destination_binding: Option<&SccpDestinationBindingV1>,
    signer: Option<&KeyPair>,
    allow_unready: bool,
    source_material: Option<&SccpSourceVerifierMaterialV1>,
) -> Option<SccpCounterpartySubmissionPackageV1> {
    if !sccp_manifest_allows_transparent_proofs(manifest, allow_unready) {
        return None;
    }
    let public_inputs = sccp_message_transparent_public_inputs_internal(bundle, source_material)?;
    let platform_payload = build_sccp_platform_submission_payload(
        manifest,
        proof_bytes,
        &public_inputs,
        bundle,
        destination_binding,
        signer,
        source_material,
    )?;
    let arguments =
        sccp_submission_argument_values(&manifest.submission_template, &platform_payload)?;
    Some(SccpCounterpartySubmissionPackageV1 {
        version: 1,
        proof_family: manifest.proof_family.clone(),
        verifier_backend: manifest.verifier_backend.clone(),
        envelope_encoding: sccp_submission_envelope_encoding(&manifest.submission_template),
        submission_kind: manifest.submission_template.submission_kind.clone(),
        verifier_entrypoint: manifest.submission_template.verifier_entrypoint.clone(),
        platform_payload,
        envelope_bytes: encode_sccp_submission_envelope(&manifest.submission_template, &arguments),
        arguments,
    })
}

fn sccp_message_transparent_public_inputs_internal(
    bundle: &NexusSccpMessageProofV1,
    source_material: Option<&SccpSourceVerifierMaterialV1>,
) -> Option<SccpMessageTransparentPublicInputsV1> {
    if !verify_message_bundle_structure_internal(bundle, source_material) {
        return None;
    }
    let source_domain = sccp_message_source_domain(&bundle.payload);
    let target_domain = sccp_message_target_domain(&bundle.payload);
    let (finality_height, finality_block_hash) = sccp_message_finality_public_inputs_internal(
        bundle,
        source_domain,
        target_domain,
        source_material,
    )?;
    Some(SccpMessageTransparentPublicInputsV1 {
        version: 1,
        message_id: bundle.commitment.message_id,
        payload_hash: bundle.commitment.payload_hash,
        target_domain: bundle.commitment.target_domain,
        commitment_root: bundle.commitment_root,
        finality_height,
        finality_block_hash,
    })
}

pub fn sccp_message_transparent_public_inputs(
    bundle: &NexusSccpMessageProofV1,
) -> Option<SccpMessageTransparentPublicInputsV1> {
    sccp_message_transparent_public_inputs_internal(bundle, None)
}

pub fn sccp_message_transparent_public_inputs_with_source_verifier_material(
    bundle: &NexusSccpMessageProofV1,
    material: &SccpSourceVerifierMaterialV1,
) -> Option<SccpMessageTransparentPublicInputsV1> {
    sccp_message_transparent_public_inputs_internal(bundle, Some(material))
}

fn canonical_sccp_message_transparent_statement_bytes(
    bundle: &NexusSccpMessageProofV1,
    manifest: &SccpProofManifestV1,
    public_inputs: &SccpMessageTransparentPublicInputsV1,
) -> Option<Vec<u8>> {
    let chain_family = sccp_transparent_chain_family_for_domain(manifest.counterparty_domain)?;
    let chain = sccp_chain_key_for_domain(manifest.counterparty_domain)?;
    let payload_kind = sccp_message_payload_kind_key(&bundle.payload);
    let payload_hash = payload_hash(&canonical_sccp_payload_bytes(&bundle.payload));
    let mut statement = Vec::new();
    push_u8(&mut statement, 1);
    push_u8(
        &mut statement,
        sccp_transparent_chain_family_code(chain_family),
    );
    push_u32(&mut statement, manifest.local_domain);
    push_u32(&mut statement, manifest.counterparty_domain);
    push_u8(
        &mut statement,
        sccp_proof_security_model_code(manifest.security_model),
    );
    push_u8(
        &mut statement,
        sccp_anchor_governance_code(manifest.anchor_governance),
    );
    push_u8(&mut statement, manifest.counterparty_account_codec);
    push_u8(
        &mut statement,
        sccp_proof_finality_model_code(manifest.finality_model),
    );
    push_u8(
        &mut statement,
        sccp_proof_verifier_target_code(manifest.verifier_target),
    );
    push_u8(
        &mut statement,
        sccp_verifier_backend_family_code(manifest.verifier_backend.family),
    );
    push_vec(&mut statement, chain.as_bytes());
    push_vec(&mut statement, manifest.proof_family.as_bytes());
    push_vec(&mut statement, manifest.verifier_backend.key.as_bytes());
    push_vec(&mut statement, manifest.message_backend.as_bytes());
    push_vec(&mut statement, manifest.registry_backend.as_bytes());
    push_vec(&mut statement, manifest.manifest_seed.as_bytes());
    statement.extend_from_slice(&manifest.destination_binding.binding_hash);
    push_vec(
        &mut statement,
        manifest.counterparty_account_codec_key.as_bytes(),
    );
    push_vec(&mut statement, payload_kind.as_bytes());
    statement.extend_from_slice(&canonical_sccp_message_transparent_public_inputs_bytes(
        public_inputs,
    ));
    statement.extend_from_slice(&payload_hash);
    Some(statement)
}

fn build_sccp_message_transparent_inner_proof_internal(
    bundle: &NexusSccpMessageProofV1,
    manifest: &SccpProofManifestV1,
    source_material: Option<&SccpSourceVerifierMaterialV1>,
) -> Option<SccpMessageTransparentInnerProofV1> {
    let public_inputs = sccp_message_transparent_public_inputs_internal(bundle, source_material)?;
    let chain_family = sccp_transparent_chain_family_for_domain(manifest.counterparty_domain)?;
    let chain = sccp_chain_key_for_domain(manifest.counterparty_domain)?;
    let payload_hash = payload_hash(&canonical_sccp_payload_bytes(&bundle.payload));
    let payload_kind = sccp_message_payload_kind_key(&bundle.payload);
    let statement =
        canonical_sccp_message_transparent_statement_bytes(bundle, manifest, &public_inputs)?;
    let statement_hash = prefixed_blake2b(SCCP_TRANSPARENT_STATEMENT_PREFIX_V1, &statement);
    Some(SccpMessageTransparentInnerProofV1 {
        version: 1,
        chain_family,
        chain: chain.to_owned(),
        local_domain: manifest.local_domain,
        counterparty_domain: manifest.counterparty_domain,
        security_model: manifest.security_model,
        anchor_governance: manifest.anchor_governance,
        destination_binding: manifest.destination_binding.clone(),
        counterparty_account_codec: manifest.counterparty_account_codec,
        counterparty_account_codec_key: manifest.counterparty_account_codec_key.clone(),
        proof_family: manifest.proof_family.clone(),
        verifier_backend: manifest.verifier_backend.clone(),
        message_backend: manifest.message_backend.clone(),
        registry_backend: manifest.registry_backend.clone(),
        manifest_seed: manifest.manifest_seed.clone(),
        finality_model: manifest.finality_model,
        verifier_target: manifest.verifier_target,
        public_inputs,
        payload_kind: payload_kind.to_owned(),
        payload_hash,
        statement_hash,
    })
}

pub fn build_sccp_message_transparent_inner_proof(
    bundle: &NexusSccpMessageProofV1,
    manifest: &SccpProofManifestV1,
) -> Option<SccpMessageTransparentInnerProofV1> {
    build_sccp_message_transparent_inner_proof_internal(bundle, manifest, None)
}

pub fn build_sccp_message_transparent_inner_proof_with_source_verifier_material(
    bundle: &NexusSccpMessageProofV1,
    manifest: &SccpProofManifestV1,
    material: &SccpSourceVerifierMaterialV1,
) -> Option<SccpMessageTransparentInnerProofV1> {
    build_sccp_message_transparent_inner_proof_internal(bundle, manifest, Some(material))
}

pub fn build_sccp_message_transparent_inner_proof_from_bundle(
    bundle: &NexusSccpMessageProofV1,
) -> Option<SccpMessageTransparentInnerProofV1> {
    let counterparty_domain = sccp_counterparty_domain_for_message_payload(&bundle.payload)?;
    let manifest = sccp_proof_manifest_for_domain(counterparty_domain)?;
    build_sccp_message_transparent_inner_proof(bundle, &manifest)
}

pub fn build_sccp_message_transparent_inner_proof_from_artifact(
    artifact: &NexusSccpMessageTransparentProofV1,
) -> Option<SccpMessageTransparentInnerProofV1> {
    if sccp_counterparty_domain_for_message_payload(&artifact.bundle.payload)
        != Some(artifact.counterparty_domain)
    {
        return None;
    }
    build_sccp_message_transparent_inner_proof_from_bundle(&artifact.bundle)
}

fn sccp_message_transparent_fastpq_public_inputs(
    inner: &SccpMessageTransparentInnerProofV1,
) -> FastpqPublicInputs {
    let mut dsid = [0u8; 16];
    let dsid_hash = prefixed_blake2b(
        SCCP_TRANSPARENT_FASTPQ_DSID_PREFIX_V1,
        &inner.statement_hash,
    );
    dsid.copy_from_slice(&dsid_hash[..16]);
    FastpqPublicInputs {
        dsid,
        slot: inner.public_inputs.finality_height,
        old_root: inner.public_inputs.payload_hash,
        new_root: inner.public_inputs.commitment_root,
        perm_root: inner.public_inputs.finality_block_hash,
        tx_set_hash: inner.statement_hash,
    }
}

fn sccp_message_transparent_open_verify_schema_descriptor(
    manifest: &SccpProofManifestV1,
) -> Vec<u8> {
    let mut descriptor = Vec::new();
    push_u8(&mut descriptor, 1);
    push_vec(
        &mut descriptor,
        SCCP_TRANSPARENT_OPEN_VERIFY_CIRCUIT_ID_V1.as_bytes(),
    );
    push_vec(&mut descriptor, manifest.manifest_seed.as_bytes());
    push_vec(&mut descriptor, manifest.message_backend.as_bytes());
    push_vec(&mut descriptor, manifest.registry_backend.as_bytes());
    push_u32(&mut descriptor, manifest.local_domain);
    push_u32(&mut descriptor, manifest.counterparty_domain);
    push_u8(
        &mut descriptor,
        sccp_proof_security_model_code(manifest.security_model),
    );
    push_u8(
        &mut descriptor,
        sccp_anchor_governance_code(manifest.anchor_governance),
    );
    push_u8(
        &mut descriptor,
        sccp_proof_verifier_target_code(manifest.verifier_target),
    );
    push_u8(
        &mut descriptor,
        sccp_proof_finality_model_code(manifest.finality_model),
    );
    push_vec(&mut descriptor, manifest.proof_family.as_bytes());
    push_vec(&mut descriptor, manifest.verifier_backend.key.as_bytes());
    push_vec(
        &mut descriptor,
        manifest.destination_binding.binding_hash.as_slice(),
    );
    for required_input in &manifest.required_public_inputs {
        push_vec(&mut descriptor, required_input.as_bytes());
    }
    descriptor
}

fn canonical_sccp_message_transparent_fastpq_verifier_bytes(
    manifest: &SccpProofManifestV1,
) -> Option<Vec<u8>> {
    let params = FastpqProver::canonical_parameter_sets()
        .iter()
        .find(|params| params.name == SCCP_TRANSPARENT_FASTPQ_PARAMETER_SET_V1)?;
    let mut verifier = Vec::new();
    push_u8(&mut verifier, 1);
    push_vec(&mut verifier, manifest.message_backend.as_bytes());
    push_vec(
        &mut verifier,
        SCCP_TRANSPARENT_OPEN_VERIFY_CIRCUIT_ID_V1.as_bytes(),
    );
    push_vec(&mut verifier, params.name.as_bytes());
    push_u32(&mut verifier, params.target_security_bits);
    push_u32(&mut verifier, params.grinding_bits);
    push_u32(&mut verifier, params.trace_log_size);
    push_u64(&mut verifier, params.trace_root);
    push_u32(&mut verifier, params.lde_log_size);
    push_u64(&mut verifier, params.lde_root);
    push_u32(&mut verifier, params.permutation_size);
    match params.lookup_log_size {
        Some(lookup_log_size) => {
            push_u8(&mut verifier, 1);
            push_u32(&mut verifier, lookup_log_size);
        }
        None => push_u8(&mut verifier, 0),
    }
    push_u64(&mut verifier, params.omega_coset);
    push_vec(&mut verifier, params.field.name.as_bytes());
    push_vec(&mut verifier, params.field.modulus_decimal.as_bytes());
    push_u32(&mut verifier, params.field.extension_degree);
    push_vec(&mut verifier, params.hash.trace_commitment.as_bytes());
    push_vec(&mut verifier, params.hash.transcript.as_bytes());
    push_u32(&mut verifier, params.fri.arity);
    push_u32(&mut verifier, params.fri.blowup_factor);
    push_u32(&mut verifier, params.fri.max_reductions);
    push_u32(&mut verifier, params.fri.queries);
    Some(verifier)
}

fn sccp_message_transparent_fastpq_verifier_commitment(
    manifest: &SccpProofManifestV1,
) -> Option<H256> {
    let verifier = canonical_sccp_message_transparent_fastpq_verifier_bytes(manifest)?;
    let mut hasher = Sha256::new();
    Digest::update(&mut hasher, manifest.message_backend.as_bytes());
    Digest::update(&mut hasher, &verifier);
    Some(hasher.finalize().into())
}

fn sccp_message_transparent_public_input_columns(
    public_inputs: &SccpMessageTransparentPublicInputsV1,
) -> Vec<Vec<[u8; 32]>> {
    let mut target_domain = [0u8; 32];
    target_domain[..4].copy_from_slice(&public_inputs.target_domain.to_le_bytes());
    let mut finality_height = [0u8; 32];
    finality_height[..8].copy_from_slice(&public_inputs.finality_height.to_le_bytes());
    vec![
        vec![public_inputs.message_id],
        vec![public_inputs.payload_hash],
        vec![target_domain],
        vec![public_inputs.commitment_root],
        vec![finality_height],
        vec![public_inputs.finality_block_hash],
    ]
}

fn sccp_open_verify_backend_key(backend: BackendTag) -> &'static str {
    match backend {
        BackendTag::Halo2IpaPasta => "halo2-ipa-pasta",
        BackendTag::Halo2Bn254 => "halo2-bn254",
        BackendTag::Groth16 => "groth16",
        BackendTag::Stark => "stark",
        BackendTag::Unsupported => "unsupported",
    }
}

fn saturating_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn decode_sccp_stark_open_verify_envelope(
    proof_bytes: &[u8],
) -> Option<(OpenVerifyEnvelope, StarkFriOpenProofV1)> {
    let env: OpenVerifyEnvelope = norito::decode_from_bytes(proof_bytes).ok()?;
    if env.backend != BackendTag::Stark {
        return None;
    }
    let open: StarkFriOpenProofV1 = norito::decode_from_bytes(&env.proof_bytes).ok()?;
    if open.version != 1 {
        return None;
    }
    Some((env, open))
}

fn decode_sccp_stark_open_verify_proof(
    proof_bytes: &[u8],
) -> Option<(
    OpenVerifyEnvelope,
    StarkFriOpenProofV1,
    fastpq_prover::Proof,
)> {
    let (env, open) = decode_sccp_stark_open_verify_envelope(proof_bytes)?;
    let proof: fastpq_prover::Proof = norito::decode_from_bytes(&open.envelope_bytes).ok()?;
    Some((env, open, proof))
}

fn decode_sccp_message_transparent_open_verify_envelope(
    proof_bytes: &[u8],
) -> Option<(OpenVerifyEnvelope, StarkFriOpenProofV1)> {
    decode_sccp_stark_open_verify_envelope(proof_bytes)
}

fn decode_sccp_message_transparent_open_verify_proof(
    proof_bytes: &[u8],
) -> Option<(
    OpenVerifyEnvelope,
    StarkFriOpenProofV1,
    fastpq_prover::Proof,
)> {
    decode_sccp_stark_open_verify_proof(proof_bytes)
}

pub fn summarize_sccp_message_transparent_open_verify_proof(
    proof_bytes: &[u8],
) -> Option<SccpOpenVerifyEnvelopeSummaryV1> {
    let (env, open) = decode_sccp_message_transparent_open_verify_envelope(proof_bytes)?;
    let public_input_word_count = open.public_inputs.iter().map(Vec::len).sum::<usize>();
    Some(SccpOpenVerifyEnvelopeSummaryV1 {
        version: 1,
        backend: sccp_open_verify_backend_key(env.backend).to_owned(),
        circuit_id: env.circuit_id,
        vk_hash: env.vk_hash,
        public_inputs_schema_hash: prefixed_blake2b(
            SCCP_TRANSPARENT_OPEN_VERIFY_SCHEMA_HASH_PREFIX_V1,
            &env.public_inputs,
        ),
        public_inputs_schema_len_bytes: saturating_u32(env.public_inputs.len()),
        public_input_column_count: saturating_u32(open.public_inputs.len()),
        public_input_word_count: saturating_u32(public_input_word_count),
        open_proof_len_bytes: saturating_u32(env.proof_bytes.len()),
        backend_proof_len_bytes: saturating_u32(open.envelope_bytes.len()),
        aux_len_bytes: saturating_u32(env.aux.len()),
    })
}

pub fn summarize_sccp_message_transparent_open_verify_proof_from_artifact(
    artifact: &NexusSccpMessageTransparentProofV1,
) -> Option<SccpOpenVerifyEnvelopeSummaryV1> {
    summarize_sccp_message_transparent_open_verify_proof(&artifact.proof_bytes)
}

pub fn build_sccp_message_transparent_open_verify_summary_from_bundle(
    bundle: &NexusSccpMessageProofV1,
) -> Option<SccpOpenVerifyEnvelopeSummaryV1> {
    if !verify_message_bundle_structure(bundle) {
        return None;
    }
    let counterparty_domain = sccp_counterparty_domain_for_message_payload(&bundle.payload)?;
    let manifest = sccp_proof_manifest_for_domain(counterparty_domain)?;
    let proof_bytes = build_sccp_message_transparent_fastpq_proof_bytes(bundle, &manifest)?;
    summarize_sccp_message_transparent_open_verify_proof(&proof_bytes)
}

fn build_sccp_message_transparent_fastpq_batch_internal(
    bundle: &NexusSccpMessageProofV1,
    manifest: &SccpProofManifestV1,
    source_material: Option<&SccpSourceVerifierMaterialV1>,
) -> Option<FastpqTransitionBatch> {
    let inner =
        build_sccp_message_transparent_inner_proof_internal(bundle, manifest, source_material)?;
    let statement =
        canonical_sccp_message_transparent_statement_bytes(bundle, manifest, &inner.public_inputs)?;
    let context = to_bytes(&inner).ok()?;
    let payload = canonical_sccp_payload_bytes(&bundle.payload);

    let mut batch = FastpqTransitionBatch::new(
        SCCP_TRANSPARENT_FASTPQ_PARAMETER_SET_V1,
        sccp_message_transparent_fastpq_public_inputs(&inner),
    );
    batch.push(FastpqStateTransition::new(
        SCCP_TRANSPARENT_FASTPQ_STATEMENT_KEY_V1.to_vec(),
        Vec::new(),
        statement,
        FastpqOperationKind::MetaSet,
    ));
    batch.push(FastpqStateTransition::new(
        SCCP_TRANSPARENT_FASTPQ_CONTEXT_KEY_V1.to_vec(),
        Vec::new(),
        context,
        FastpqOperationKind::MetaSet,
    ));
    batch.push(FastpqStateTransition::new(
        SCCP_TRANSPARENT_FASTPQ_PAYLOAD_KEY_V1.to_vec(),
        Vec::new(),
        payload,
        FastpqOperationKind::MetaSet,
    ));
    batch.sort();
    Some(batch)
}

#[cfg(test)]
fn build_sccp_message_transparent_fastpq_batch(
    bundle: &NexusSccpMessageProofV1,
    manifest: &SccpProofManifestV1,
) -> Option<FastpqTransitionBatch> {
    build_sccp_message_transparent_fastpq_batch_internal(bundle, manifest, None)
}

fn build_sccp_message_transparent_fastpq_raw_proof_bytes(
    batch: &FastpqTransitionBatch,
) -> Option<Vec<u8>> {
    let proof = FastpqProver::canonical(SCCP_TRANSPARENT_FASTPQ_PARAMETER_SET_V1)
        .ok()?
        .prove(batch)
        .ok()?;
    to_bytes(&proof).ok()
}

fn build_sccp_message_transparent_fastpq_proof_bytes_internal(
    bundle: &NexusSccpMessageProofV1,
    manifest: &SccpProofManifestV1,
    source_material: Option<&SccpSourceVerifierMaterialV1>,
) -> Option<Vec<u8>> {
    let batch =
        build_sccp_message_transparent_fastpq_batch_internal(bundle, manifest, source_material)?;
    let raw_proof_bytes = build_sccp_message_transparent_fastpq_raw_proof_bytes(&batch)?;
    let public_inputs = sccp_message_transparent_public_inputs_internal(bundle, source_material)?;
    let open = StarkFriOpenProofV1 {
        version: 1,
        public_inputs: sccp_message_transparent_public_input_columns(&public_inputs),
        envelope_bytes: raw_proof_bytes,
    };
    let env = OpenVerifyEnvelope {
        backend: BackendTag::Stark,
        circuit_id: SCCP_TRANSPARENT_OPEN_VERIFY_CIRCUIT_ID_V1.to_owned(),
        vk_hash: sccp_message_transparent_fastpq_verifier_commitment(manifest)?,
        public_inputs: sccp_message_transparent_open_verify_schema_descriptor(manifest),
        proof_bytes: to_bytes(&open).ok()?,
        aux: Vec::new(),
    };
    to_bytes(&env).ok()
}

fn build_sccp_message_transparent_fastpq_proof_bytes(
    bundle: &NexusSccpMessageProofV1,
    manifest: &SccpProofManifestV1,
) -> Option<Vec<u8>> {
    build_sccp_message_transparent_fastpq_proof_bytes_internal(bundle, manifest, None)
}

pub fn build_nexus_sccp_message_transparent_proof_with_signer(
    bundle: &NexusSccpMessageProofV1,
    signer: &KeyPair,
) -> Option<NexusSccpMessageTransparentProofV1> {
    build_nexus_sccp_message_transparent_proof_internal(bundle, None, Some(signer), false, None)
}

pub fn build_nexus_sccp_message_transparent_proof_with_signer_allow_unready(
    bundle: &NexusSccpMessageProofV1,
    signer: &KeyPair,
    allow_unready: bool,
) -> Option<NexusSccpMessageTransparentProofV1> {
    build_nexus_sccp_message_transparent_proof_internal(
        bundle,
        None,
        Some(signer),
        allow_unready,
        None,
    )
}

pub fn build_nexus_sccp_message_transparent_proof_with_destination_binding_and_signer(
    bundle: &NexusSccpMessageProofV1,
    destination_binding: &SccpDestinationBindingV1,
    signer: &KeyPair,
) -> Option<NexusSccpMessageTransparentProofV1> {
    build_nexus_sccp_message_transparent_proof_internal(
        bundle,
        Some(destination_binding),
        Some(signer),
        false,
        None,
    )
}

pub fn build_nexus_sccp_message_transparent_proof_with_destination_binding_and_signer_allow_unready(
    bundle: &NexusSccpMessageProofV1,
    destination_binding: &SccpDestinationBindingV1,
    signer: &KeyPair,
    allow_unready: bool,
) -> Option<NexusSccpMessageTransparentProofV1> {
    build_nexus_sccp_message_transparent_proof_internal(
        bundle,
        Some(destination_binding),
        Some(signer),
        allow_unready,
        None,
    )
}

pub fn build_nexus_sccp_message_transparent_proof(
    bundle: &NexusSccpMessageProofV1,
) -> Option<NexusSccpMessageTransparentProofV1> {
    build_nexus_sccp_message_transparent_proof_internal(bundle, None, None, false, None)
}

pub fn build_nexus_sccp_message_transparent_proof_allow_unready(
    bundle: &NexusSccpMessageProofV1,
    allow_unready: bool,
) -> Option<NexusSccpMessageTransparentProofV1> {
    build_nexus_sccp_message_transparent_proof_internal(bundle, None, None, allow_unready, None)
}

pub fn build_nexus_sccp_message_transparent_proof_with_source_verifier_material(
    bundle: &NexusSccpMessageProofV1,
    material: &SccpSourceVerifierMaterialV1,
) -> Option<NexusSccpMessageTransparentProofV1> {
    build_nexus_sccp_message_transparent_proof_internal(bundle, None, None, false, Some(material))
}

pub fn build_nexus_sccp_message_transparent_proof_with_source_verifier_material_allow_unready(
    bundle: &NexusSccpMessageProofV1,
    material: &SccpSourceVerifierMaterialV1,
    allow_unready: bool,
) -> Option<NexusSccpMessageTransparentProofV1> {
    build_nexus_sccp_message_transparent_proof_internal(
        bundle,
        None,
        None,
        allow_unready,
        Some(material),
    )
}

pub fn build_nexus_sccp_message_transparent_proof_with_evm_groth16_proof_and_destination_binding(
    bundle: &NexusSccpMessageProofV1,
    groth16_proof_bytes: &[u8],
    destination_binding: &SccpDestinationBindingV1,
) -> Option<NexusSccpMessageTransparentProofV1> {
    build_nexus_sccp_message_transparent_proof_with_evm_groth16_proof_and_destination_binding_allow_unready(
        bundle,
        groth16_proof_bytes,
        destination_binding,
        false,
    )
}

pub fn build_nexus_sccp_message_transparent_proof_with_evm_groth16_proof_and_destination_binding_allow_unready(
    bundle: &NexusSccpMessageProofV1,
    groth16_proof_bytes: &[u8],
    destination_binding: &SccpDestinationBindingV1,
    allow_unready: bool,
) -> Option<NexusSccpMessageTransparentProofV1> {
    let counterparty_domain = sccp_counterparty_domain_for_message_payload(&bundle.payload)?;
    let manifest = sccp_proof_manifest_for_domain(counterparty_domain)?;
    if manifest.verifier_backend.family != SccpVerifierBackendFamilyV1::EvmGroth16Bn254
        || manifest.verifier_backend.key != SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1
        || !sccp_manifest_allows_transparent_proofs(&manifest, allow_unready)
    {
        return None;
    }
    let public_inputs = sccp_message_transparent_public_inputs(bundle)?;
    let submission_package = build_sccp_counterparty_submission_package_internal(
        bundle,
        &manifest,
        groth16_proof_bytes,
        Some(destination_binding),
        None,
        allow_unready,
        None,
    )?;
    Some(NexusSccpMessageTransparentProofV1 {
        version: 1,
        local_domain: manifest.local_domain,
        counterparty_domain,
        security_model: manifest.security_model,
        anchor_governance: manifest.anchor_governance,
        destination_binding: manifest.destination_binding.clone(),
        proof_family: manifest.proof_family,
        verifier_backend: manifest.verifier_backend,
        message_backend: manifest.message_backend,
        registry_backend: manifest.registry_backend,
        manifest_seed: manifest.manifest_seed,
        finality_model: manifest.finality_model,
        verifier_target: manifest.verifier_target,
        public_inputs,
        proof_bytes: groth16_proof_bytes.to_vec(),
        submission_package,
        bundle: bundle.clone(),
    })
}

pub fn build_nexus_sccp_message_transparent_proof_with_tron_groth16_proof(
    bundle: &NexusSccpMessageProofV1,
    groth16_proof_bytes: &[u8],
) -> Option<NexusSccpMessageTransparentProofV1> {
    build_nexus_sccp_message_transparent_proof_with_tron_groth16_proof_allow_unready(
        bundle,
        groth16_proof_bytes,
        false,
    )
}

pub fn build_nexus_sccp_message_transparent_proof_with_tron_groth16_proof_allow_unready(
    bundle: &NexusSccpMessageProofV1,
    groth16_proof_bytes: &[u8],
    allow_unready: bool,
) -> Option<NexusSccpMessageTransparentProofV1> {
    build_nexus_sccp_message_transparent_proof_with_tron_groth16_proof_and_destination_binding_allow_unready(
        bundle,
        groth16_proof_bytes,
        &sccp_proof_manifest_for_domain(SCCP_DOMAIN_TRON)?.destination_binding,
        allow_unready,
    )
}

pub fn build_nexus_sccp_message_transparent_proof_with_tron_groth16_proof_and_destination_binding(
    bundle: &NexusSccpMessageProofV1,
    groth16_proof_bytes: &[u8],
    destination_binding: &SccpDestinationBindingV1,
) -> Option<NexusSccpMessageTransparentProofV1> {
    build_nexus_sccp_message_transparent_proof_with_tron_groth16_proof_and_destination_binding_allow_unready(
        bundle,
        groth16_proof_bytes,
        destination_binding,
        false,
    )
}

pub fn build_nexus_sccp_message_transparent_proof_with_tron_groth16_proof_and_destination_binding_allow_unready(
    bundle: &NexusSccpMessageProofV1,
    groth16_proof_bytes: &[u8],
    destination_binding: &SccpDestinationBindingV1,
    allow_unready: bool,
) -> Option<NexusSccpMessageTransparentProofV1> {
    let counterparty_domain = sccp_counterparty_domain_for_message_payload(&bundle.payload)?;
    if counterparty_domain != SCCP_DOMAIN_TRON {
        return None;
    }
    let manifest = sccp_proof_manifest_for_domain(counterparty_domain)?;
    if manifest.verifier_backend.family != SccpVerifierBackendFamilyV1::TronGroth16Bn254
        || manifest.verifier_backend.key != SCCP_TRON_GROTH16_BN254_PROOF_BACKEND_V1
        || !sccp_manifest_allows_transparent_proofs(&manifest, allow_unready)
    {
        return None;
    }
    let public_inputs = sccp_message_transparent_public_inputs(bundle)?;
    let submission_package = build_sccp_counterparty_submission_package_internal(
        bundle,
        &manifest,
        groth16_proof_bytes,
        Some(destination_binding),
        None,
        allow_unready,
        None,
    )?;
    Some(NexusSccpMessageTransparentProofV1 {
        version: 1,
        local_domain: manifest.local_domain,
        counterparty_domain,
        security_model: manifest.security_model,
        anchor_governance: manifest.anchor_governance,
        destination_binding: manifest.destination_binding.clone(),
        proof_family: manifest.proof_family,
        verifier_backend: manifest.verifier_backend,
        message_backend: manifest.message_backend,
        registry_backend: manifest.registry_backend,
        manifest_seed: manifest.manifest_seed,
        finality_model: manifest.finality_model,
        verifier_target: manifest.verifier_target,
        public_inputs,
        proof_bytes: groth16_proof_bytes.to_vec(),
        submission_package,
        bundle: bundle.clone(),
    })
}

fn build_nexus_sccp_message_transparent_proof_internal(
    bundle: &NexusSccpMessageProofV1,
    platform_destination_binding: Option<&SccpDestinationBindingV1>,
    signer: Option<&KeyPair>,
    allow_unready: bool,
    source_material: Option<&SccpSourceVerifierMaterialV1>,
) -> Option<NexusSccpMessageTransparentProofV1> {
    let counterparty_domain = sccp_counterparty_domain_for_message_payload(&bundle.payload)?;
    let manifest = sccp_proof_manifest_for_domain(counterparty_domain)?;
    if !sccp_manifest_allows_transparent_proofs(&manifest, allow_unready) {
        return None;
    }
    let public_inputs = sccp_message_transparent_public_inputs_internal(bundle, source_material)?;
    let proof_bytes = build_sccp_message_transparent_fastpq_proof_bytes_internal(
        bundle,
        &manifest,
        source_material,
    )?;
    let submission_package = build_sccp_counterparty_submission_package_internal(
        bundle,
        &manifest,
        &proof_bytes,
        platform_destination_binding,
        signer,
        allow_unready,
        source_material,
    )?;
    Some(NexusSccpMessageTransparentProofV1 {
        version: 1,
        local_domain: manifest.local_domain,
        counterparty_domain,
        security_model: manifest.security_model,
        anchor_governance: manifest.anchor_governance,
        destination_binding: manifest.destination_binding.clone(),
        proof_family: manifest.proof_family,
        verifier_backend: manifest.verifier_backend,
        message_backend: manifest.message_backend,
        registry_backend: manifest.registry_backend,
        manifest_seed: manifest.manifest_seed,
        finality_model: manifest.finality_model,
        verifier_target: manifest.verifier_target,
        public_inputs,
        proof_bytes,
        submission_package,
        bundle: bundle.clone(),
    })
}

fn verify_sccp_message_transparent_inner_proof_bytes_internal(
    proof_bytes: &[u8],
    bundle: &NexusSccpMessageProofV1,
    manifest: &SccpProofManifestV1,
    public_inputs: &SccpMessageTransparentPublicInputsV1,
    source_material: Option<&SccpSourceVerifierMaterialV1>,
) -> bool {
    let Some(expected) =
        build_sccp_message_transparent_inner_proof_internal(bundle, manifest, source_material)
    else {
        return false;
    };

    if &expected.public_inputs != public_inputs {
        return false;
    }

    let Some(batch) =
        build_sccp_message_transparent_fastpq_batch_internal(bundle, manifest, source_material)
    else {
        return false;
    };
    let Some((env, open, proof)) = decode_sccp_message_transparent_open_verify_proof(proof_bytes)
    else {
        return false;
    };
    if env.circuit_id != SCCP_TRANSPARENT_OPEN_VERIFY_CIRCUIT_ID_V1
        || env.vk_hash
            != sccp_message_transparent_fastpq_verifier_commitment(manifest).unwrap_or([0u8; 32])
        || env.public_inputs != sccp_message_transparent_open_verify_schema_descriptor(manifest)
        || !env.aux.is_empty()
        || open.public_inputs != sccp_message_transparent_public_input_columns(public_inputs)
    {
        return false;
    }
    fastpq_prover::verify(&batch, &proof).is_ok()
}

#[cfg(test)]
fn verify_sccp_message_transparent_inner_proof_bytes(
    proof_bytes: &[u8],
    bundle: &NexusSccpMessageProofV1,
    manifest: &SccpProofManifestV1,
    public_inputs: &SccpMessageTransparentPublicInputsV1,
) -> bool {
    verify_sccp_message_transparent_inner_proof_bytes_internal(
        proof_bytes,
        bundle,
        manifest,
        public_inputs,
        None,
    )
}

fn verify_sccp_evm_attestation_signatures(
    signatures: &[SccpEvmAttestationSignatureV1],
    digest: &H256,
) -> bool {
    if signatures.is_empty() {
        return false;
    }
    let mut seen = Vec::<Vec<u8>>::new();
    for signature in signatures {
        if signature.signer_address.len() != 20 || signature.signature_bytes.len() != 65 {
            return false;
        }
        let mut compact = [0u8; 65];
        compact.copy_from_slice(&signature.signature_bytes);
        let Ok(public_key) =
            EcdsaSecp256k1Sha256::recover_public_key_from_prehash(digest, &compact)
        else {
            return false;
        };
        let address = EcdsaSecp256k1Sha256::evm_address(&public_key);
        if signature.signer_address.as_slice() != address {
            return false;
        }
        if seen.iter().any(|known| known == &signature.signer_address) {
            return false;
        }
        seen.push(signature.signer_address.clone());
    }
    true
}

fn sccp_destination_binding_metadata_is_valid(binding: &SccpDestinationBindingV1) -> bool {
    binding.version == 1 && !binding.key.trim().is_empty() && h256_is_nonzero(&binding.binding_hash)
}

fn verify_sccp_evm_submission_package_internal(
    manifest: &SccpProofManifestV1,
    proof: &NexusSccpMessageTransparentProofV1,
    source_material: Option<&SccpSourceVerifierMaterialV1>,
) -> bool {
    if proof.submission_package.version != 1
        || proof.submission_package.proof_family != manifest.proof_family
        || proof.submission_package.verifier_backend != manifest.verifier_backend
        || proof.submission_package.envelope_encoding
            != sccp_submission_envelope_encoding(&manifest.submission_template)
        || proof.submission_package.submission_kind != manifest.submission_template.submission_kind
        || proof.submission_package.verifier_entrypoint
            != manifest.submission_template.verifier_entrypoint
    {
        return false;
    }
    let Some(inner) = build_sccp_message_transparent_inner_proof_internal(
        &proof.bundle,
        manifest,
        source_material,
    ) else {
        return false;
    };

    match &proof.submission_package.platform_payload {
        SccpPlatformSubmissionPayloadV1::EvmContractCall(payload) => {
            if manifest.verifier_backend.family != SccpVerifierBackendFamilyV1::EvmSecp256k1Keccak
                || manifest.verifier_backend.key != SCCP_EVM_SECP256K1_PROOF_BACKEND_V1
            {
                return false;
            }
            let expected_public_inputs = sccp_evm_public_input_word_struct(&proof.public_inputs);
            if payload.public_inputs != expected_public_inputs
                || payload.public_inputs_hash != sccp_evm_public_inputs_hash(&proof.public_inputs)
                || payload.statement_hash != inner.statement_hash
                || !sccp_destination_binding_metadata_is_valid(&payload.destination_binding)
            {
                return false;
            }
            let expected_native_proof_hash = sccp_evm_native_proof_hash(&proof.proof_bytes);
            if payload.attestation.version != 1
                || payload.attestation.message_id != proof.public_inputs.message_id
                || payload.attestation.source_domain != manifest.local_domain
                || payload.attestation.commitment_root != proof.public_inputs.commitment_root
                || payload.attestation.native_proof_hash != expected_native_proof_hash
                || payload.attestation.destination_binding_hash
                    != payload.destination_binding.binding_hash
            {
                return false;
            }
            let Some(decoded_envelope) = decode_sccp_evm_attestation_envelope(&payload.proof_bytes)
            else {
                return false;
            };
            if decoded_envelope.version != payload.attestation.version
                || decoded_envelope.message_id != payload.attestation.message_id
                || decoded_envelope.source_domain != payload.attestation.source_domain
                || decoded_envelope.commitment_root != payload.attestation.commitment_root
                || decoded_envelope.native_proof_hash != payload.attestation.native_proof_hash
                || decoded_envelope.destination_binding_hash
                    != payload.attestation.destination_binding_hash
                || decoded_envelope.signatures.len() != payload.attestation.signatures.len()
            {
                return false;
            }
            if decoded_envelope
                .signatures
                .iter()
                .zip(payload.attestation.signatures.iter())
                .any(|(decoded, typed)| decoded.signature_bytes != typed.signature_bytes)
            {
                return false;
            }
            let digest = sccp_evm_attestation_digest(
                &payload.attestation,
                payload.public_inputs_hash,
                payload.statement_hash,
            );
            if !verify_sccp_evm_attestation_signatures(&payload.attestation.signatures, &digest) {
                return false;
            }
            let Some(expected_proof_bytes) =
                encode_sccp_evm_attestation_envelope(&payload.attestation)
            else {
                return false;
            };
            if expected_proof_bytes != payload.proof_bytes {
                return false;
            }
        }
        SccpPlatformSubmissionPayloadV1::EvmGroth16ContractCall(payload) => {
            if manifest.verifier_backend.family != SccpVerifierBackendFamilyV1::EvmGroth16Bn254
                || manifest.verifier_backend.key != SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1
            {
                return false;
            }
            let expected_public_inputs = sccp_evm_public_input_word_struct(&proof.public_inputs);
            if payload.public_inputs != expected_public_inputs
                || payload.statement_hash != inner.statement_hash
                || !sccp_destination_binding_metadata_is_valid(&payload.destination_binding)
                || payload.proof_bytes != proof.proof_bytes
                || !verify_sccp_evm_groth16_bn254_proof_binding(
                    manifest,
                    &proof.public_inputs,
                    &payload.proof_bytes,
                )
            {
                return false;
            }
        }
        _ => return false,
    }

    let Some(arguments) = sccp_submission_argument_values(
        &manifest.submission_template,
        &proof.submission_package.platform_payload,
    ) else {
        return false;
    };
    arguments == proof.submission_package.arguments
        && encode_sccp_submission_envelope(&manifest.submission_template, &arguments)
            == proof.submission_package.envelope_bytes
}

fn verify_sccp_tron_submission_package_internal(
    manifest: &SccpProofManifestV1,
    proof: &NexusSccpMessageTransparentProofV1,
    source_material: Option<&SccpSourceVerifierMaterialV1>,
) -> bool {
    if proof.submission_package.version != 1
        || proof.submission_package.proof_family != manifest.proof_family
        || proof.submission_package.verifier_backend != manifest.verifier_backend
        || proof.submission_package.envelope_encoding
            != sccp_submission_envelope_encoding(&manifest.submission_template)
        || proof.submission_package.submission_kind != manifest.submission_template.submission_kind
        || proof.submission_package.verifier_entrypoint
            != manifest.submission_template.verifier_entrypoint
        || manifest.verifier_backend.family != SccpVerifierBackendFamilyV1::TronGroth16Bn254
        || manifest.verifier_backend.key != SCCP_TRON_GROTH16_BN254_PROOF_BACKEND_V1
    {
        return false;
    }
    let Some(inner) = build_sccp_message_transparent_inner_proof_internal(
        &proof.bundle,
        manifest,
        source_material,
    ) else {
        return false;
    };

    let SccpPlatformSubmissionPayloadV1::TronContractCall(payload) =
        &proof.submission_package.platform_payload
    else {
        return false;
    };
    let expected_public_inputs = sccp_evm_public_input_word_struct(&proof.public_inputs);
    if payload.public_inputs != expected_public_inputs
        || payload.statement_hash != inner.statement_hash
        || !sccp_destination_binding_metadata_is_valid(&payload.destination_binding)
        || payload.proof_bytes != proof.proof_bytes
        || !verify_sccp_tron_groth16_bn254_proof_binding(
            manifest,
            &proof.public_inputs,
            &payload.proof_bytes,
        )
    {
        return false;
    }

    let Some(arguments) = sccp_submission_argument_values(
        &manifest.submission_template,
        &proof.submission_package.platform_payload,
    ) else {
        return false;
    };
    arguments == proof.submission_package.arguments
        && encode_sccp_submission_envelope(&manifest.submission_template, &arguments)
            == proof.submission_package.envelope_bytes
}

#[cfg(test)]
fn verify_sccp_evm_submission_package(
    manifest: &SccpProofManifestV1,
    proof: &NexusSccpMessageTransparentProofV1,
) -> bool {
    verify_sccp_evm_submission_package_internal(manifest, proof, None)
}

pub fn verify_nexus_sccp_message_transparent_proof_structure(
    proof: &NexusSccpMessageTransparentProofV1,
) -> bool {
    verify_nexus_sccp_message_transparent_proof_structure_internal(proof, false, None)
}

pub fn verify_nexus_sccp_message_transparent_proof_structure_allow_unready(
    proof: &NexusSccpMessageTransparentProofV1,
    allow_unready: bool,
) -> bool {
    verify_nexus_sccp_message_transparent_proof_structure_internal(proof, allow_unready, None)
}

pub fn verify_nexus_sccp_message_transparent_proof_structure_with_source_verifier_material(
    proof: &NexusSccpMessageTransparentProofV1,
    material: &SccpSourceVerifierMaterialV1,
) -> bool {
    verify_nexus_sccp_message_transparent_proof_structure_internal(proof, false, Some(material))
}

fn verify_nexus_sccp_message_transparent_proof_structure_internal(
    proof: &NexusSccpMessageTransparentProofV1,
    allow_unready: bool,
    source_material: Option<&SccpSourceVerifierMaterialV1>,
) -> bool {
    if proof.version != 1
        || proof.local_domain != SCCP_DOMAIN_SORA
        || proof.proof_family != SCCP_STARK_FRI_PROOF_FAMILY_V1
        || proof.proof_bytes.is_empty()
        || !verify_message_bundle_structure_internal(&proof.bundle, source_material)
    {
        return false;
    }
    let Some(manifest) = sccp_proof_manifest_for_domain(proof.counterparty_domain) else {
        return false;
    };
    let source_domain = sccp_message_source_domain(&proof.bundle.payload);
    if !allow_unready && source_domain != SCCP_DOMAIN_SORA {
        let source_proof_is_production_ready = if let Some(material) = source_material {
            verified_sccp_message_source_chain_proof_envelope_for_production_with_material(
                &proof.bundle,
                material,
            )
            .is_some()
        } else {
            verified_sccp_message_source_chain_proof_envelope_for_production(&proof.bundle)
                .is_some()
        };
        if !source_proof_is_production_ready {
            return false;
        }
    }
    if source_material.is_some() && source_domain == SCCP_DOMAIN_SORA {
        return false;
    }
    let allow_manifest_unready = allow_unready;
    if !sccp_manifest_allows_transparent_proofs(&manifest, allow_manifest_unready)
        || proof.security_model != manifest.security_model
        || proof.anchor_governance != manifest.anchor_governance
        || proof.destination_binding != manifest.destination_binding
        || proof.message_backend != manifest.message_backend
        || proof.registry_backend != manifest.registry_backend
        || proof.manifest_seed != manifest.manifest_seed
        || proof.verifier_backend != manifest.verifier_backend
        || proof.finality_model != manifest.finality_model
        || proof.verifier_target != manifest.verifier_target
        || sccp_counterparty_domain_for_message_payload(&proof.bundle.payload)
            != Some(proof.counterparty_domain)
    {
        return false;
    }
    sccp_message_transparent_public_inputs_internal(&proof.bundle, source_material).is_some_and(
        |expected| {
            let proof_body_is_valid = if manifest.verifier_target
                == SccpProofVerifierTargetV1::EvmContract
                && manifest.verifier_backend.family == SccpVerifierBackendFamilyV1::EvmGroth16Bn254
            {
                verify_sccp_evm_groth16_bn254_proof_binding(
                    &manifest,
                    &proof.public_inputs,
                    &proof.proof_bytes,
                )
            } else if manifest.verifier_target == SccpProofVerifierTargetV1::TronContract
                && manifest.verifier_backend.family == SccpVerifierBackendFamilyV1::TronGroth16Bn254
            {
                verify_sccp_tron_groth16_bn254_proof_binding(
                    &manifest,
                    &proof.public_inputs,
                    &proof.proof_bytes,
                )
            } else {
                verify_sccp_message_transparent_inner_proof_bytes_internal(
                    &proof.proof_bytes,
                    &proof.bundle,
                    &manifest,
                    &proof.public_inputs,
                    source_material,
                )
            };

            expected == proof.public_inputs
                && match manifest.verifier_target {
                    SccpProofVerifierTargetV1::EvmContract => {
                        verify_sccp_evm_submission_package_internal(
                            &manifest,
                            proof,
                            source_material,
                        )
                    }
                    SccpProofVerifierTargetV1::TronContract => {
                        verify_sccp_tron_submission_package_internal(
                            &manifest,
                            proof,
                            source_material,
                        )
                    }
                    _ => build_sccp_counterparty_submission_package(
                        &proof.bundle,
                        &manifest,
                        &proof.proof_bytes,
                    )
                    .or_else(|| {
                        allow_unready.then(|| {
                            build_sccp_counterparty_submission_package_allow_unready(
                                &proof.bundle,
                                &manifest,
                                &proof.proof_bytes,
                                true,
                            )
                        })?
                    })
                    .or_else(|| {
                        source_material.and_then(|material| {
                        build_sccp_counterparty_submission_package_with_source_verifier_material(
                            &proof.bundle,
                            &manifest,
                            &proof.proof_bytes,
                            material,
                        )
                    })
                    })
                    .is_some_and(|expected_submission_package| {
                        expected_submission_package == proof.submission_package
                    }),
                }
                && proof_body_is_valid
        },
    )
}

fn decode_ascii_hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn is_ascii_base58_digit(byte: u8) -> bool {
    matches!(
        byte,
        b'1'..=b'9'
            | b'A'..=b'H'
            | b'J'..=b'N'
            | b'P'..=b'Z'
            | b'a'..=b'k'
            | b'm'..=b'z'
    )
}

fn validate_evm_hex_codec(bytes: &[u8]) -> bool {
    if bytes.len() != 42 || bytes[..2] != *b"0x" {
        return false;
    }

    let payload = &bytes[2..];
    for chunk in payload.chunks_exact(2) {
        if decode_ascii_hex_nibble(chunk[0]).is_none()
            || decode_ascii_hex_nibble(chunk[1]).is_none()
        {
            return false;
        }
    }

    let lowercase_payload = payload
        .iter()
        .map(u8::to_ascii_lowercase)
        .collect::<Vec<_>>();
    let checksum = keccak256_bytes(&lowercase_payload);

    for (idx, byte) in payload.iter().copied().enumerate() {
        if byte.is_ascii_digit() {
            continue;
        }

        let checksum_nibble = if idx % 2 == 0 {
            checksum[idx / 2] >> 4
        } else {
            checksum[idx / 2] & 0x0f
        };
        let should_be_uppercase = checksum_nibble >= 8;

        if should_be_uppercase {
            if !byte.is_ascii_uppercase() {
                return false;
            }
        } else if !byte.is_ascii_lowercase() {
            return false;
        }
    }

    true
}

fn validate_base58_codec(bytes: &[u8], min_len: usize, max_len: usize) -> bool {
    !bytes.is_empty()
        && bytes.len() >= min_len
        && bytes.len() <= max_len
        && bytes.iter().copied().all(is_ascii_base58_digit)
}

fn validate_ton_raw_codec(bytes: &[u8]) -> bool {
    let Ok(value) = core::str::from_utf8(bytes) else {
        return false;
    };
    let Some((workchain, account)) = value.split_once(':') else {
        return false;
    };
    validate_canonical_i32_decimal(workchain)
        && account.len() == 64
        && account
            .as_bytes()
            .iter()
            .copied()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn validate_canonical_i32_decimal(value: &str) -> bool {
    if value.is_empty() || value.starts_with('+') {
        return false;
    }

    let digits = if let Some(digits) = value.strip_prefix('-') {
        if digits.is_empty() || digits == "0" {
            return false;
        }
        digits
    } else {
        value
    };

    digits
        .as_bytes()
        .iter()
        .copied()
        .all(|byte| byte.is_ascii_digit())
        && (digits.len() == 1 || digits.as_bytes()[0] != b'0')
        && value.parse::<i32>().is_ok()
}

fn validate_tron_base58_codec(bytes: &[u8]) -> bool {
    bytes.len() == 34
        && bytes.first() == Some(&b'T')
        && bytes.iter().copied().all(is_ascii_base58_digit)
}

fn decode_evm_hex_address(bytes: &[u8]) -> Option<[u8; 20]> {
    if !validate_evm_hex_codec(bytes) {
        return None;
    }
    let mut out = [0u8; 20];
    for (idx, chunk) in bytes[2..].chunks_exact(2).enumerate() {
        let hi = decode_ascii_hex_nibble(chunk[0])?;
        let lo = decode_ascii_hex_nibble(chunk[1])?;
        out[idx] = (hi << 4) | lo;
    }
    Some(out)
}

fn decode_sol_base58_address(bytes: &[u8]) -> Option<[u8; 32]> {
    if !validate_base58_codec(bytes, 32, 44) {
        return None;
    }
    let value = core::str::from_utf8(bytes).ok()?;
    let decoded = bs58::decode(value).into_vec().ok()?;
    let mut out = [0u8; 32];
    if decoded.len() != out.len() {
        return None;
    }
    out.copy_from_slice(&decoded);
    Some(out)
}

fn decode_ton_raw_address(bytes: &[u8]) -> Option<(i32, [u8; 32])> {
    if !validate_ton_raw_codec(bytes) {
        return None;
    }
    let value = core::str::from_utf8(bytes).ok()?;
    let (workchain, account_hex) = value.split_once(':')?;
    let mut account = [0u8; 32];
    for (idx, chunk) in account_hex.as_bytes().chunks_exact(2).enumerate() {
        let hi = decode_ascii_hex_nibble(chunk[0])?;
        let lo = decode_ascii_hex_nibble(chunk[1])?;
        account[idx] = (hi << 4) | lo;
    }
    Some((workchain.parse::<i32>().ok()?, account))
}

fn decode_tron_base58check_address(bytes: &[u8]) -> Option<[u8; 21]> {
    if !validate_tron_base58_codec(bytes) {
        return None;
    }
    let value = core::str::from_utf8(bytes).ok()?;
    let decoded = bs58::decode(value).into_vec().ok()?;
    if decoded.len() != 25 {
        return None;
    }
    let (payload, checksum) = decoded.split_at(21);
    if payload.first().copied() != Some(0x41) {
        return None;
    }
    let hash1 = Sha256::digest(payload);
    let hash2 = Sha256::digest(hash1);
    if checksum != &hash2[..4] {
        return None;
    }
    let mut out = [0u8; 21];
    out.copy_from_slice(payload);
    Some(out)
}

fn decode_sora_asset_id(bytes: &[u8]) -> Option<H256> {
    let mut out = [0u8; 32];
    (bytes.len() == out.len()).then(|| {
        out.copy_from_slice(bytes);
        out
    })
}

pub fn decode_sccp_normalized_codec_value(
    codec_id: u8,
    bytes: &[u8],
) -> Option<SccpNormalizedCodecValueV1> {
    match codec_id {
        SCCP_CODEC_TEXT_UTF8 => {
            let value = core::str::from_utf8(bytes).ok()?;
            (!value.is_empty()).then(|| SccpNormalizedCodecValueV1::TextUtf8 {
                value: value.to_owned(),
            })
        }
        SCCP_CODEC_EVM_HEX => Some(SccpNormalizedCodecValueV1::EvmHex {
            bytes: decode_evm_hex_address(bytes)?,
        }),
        SCCP_CODEC_SOLANA_BASE58 => Some(SccpNormalizedCodecValueV1::SolanaBase58 {
            bytes: decode_sol_base58_address(bytes)?,
        }),
        SCCP_CODEC_TON_RAW => {
            let (workchain, account) = decode_ton_raw_address(bytes)?;
            Some(SccpNormalizedCodecValueV1::TonRaw { workchain, account })
        }
        SCCP_CODEC_TRON_BASE58CHECK => Some(SccpNormalizedCodecValueV1::TronBase58Check {
            payload: decode_tron_base58check_address(bytes)?,
        }),
        SCCP_CODEC_SORA_ASSET_ID => Some(SccpNormalizedCodecValueV1::SoraAssetId {
            bytes: decode_sora_asset_id(bytes)?,
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

fn push_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
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

pub fn canonical_burn_payload_bytes(payload: &BurnPayloadV1) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + 4 + 4 + 8 + 32 + 16 + 32);
    push_u8(&mut out, payload.version);
    push_u32(&mut out, payload.source_domain);
    push_u32(&mut out, payload.dest_domain);
    push_u64(&mut out, payload.nonce);
    out.extend_from_slice(&payload.sora_asset_id);
    push_u128(&mut out, payload.amount);
    out.extend_from_slice(&payload.recipient);
    out
}

pub fn canonical_token_add_payload_bytes(payload: &TokenAddPayloadV1) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + 4 + 8 + 32 + 1 + 32 + 32);
    push_u8(&mut out, payload.version);
    push_u32(&mut out, payload.target_domain);
    push_u64(&mut out, payload.nonce);
    out.extend_from_slice(&payload.sora_asset_id);
    push_u8(&mut out, payload.decimals);
    out.extend_from_slice(&payload.name);
    out.extend_from_slice(&payload.symbol);
    out
}

pub fn canonical_token_control_payload_bytes(payload: &TokenControlPayloadV1) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + 4 + 8 + 32);
    push_u8(&mut out, payload.version);
    push_u32(&mut out, payload.target_domain);
    push_u64(&mut out, payload.nonce);
    out.extend_from_slice(&payload.sora_asset_id);
    out
}

pub fn canonical_asset_register_payload_bytes(payload: &AssetRegisterPayloadV1) -> Vec<u8> {
    let mut out = Vec::new();
    push_u8(&mut out, payload.version);
    push_u32(&mut out, payload.target_domain);
    push_u32(&mut out, payload.home_domain);
    push_u64(&mut out, payload.nonce);
    push_u8(&mut out, payload.asset_id_codec);
    push_vec(&mut out, &payload.asset_id);
    push_u8(&mut out, payload.decimals);
    out
}

pub fn canonical_route_activate_payload_bytes(payload: &RouteActivatePayloadV1) -> Vec<u8> {
    let mut out = Vec::new();
    push_u8(&mut out, payload.version);
    push_u32(&mut out, payload.source_domain);
    push_u32(&mut out, payload.target_domain);
    push_u64(&mut out, payload.nonce);
    push_u8(&mut out, payload.asset_id_codec);
    push_vec(&mut out, &payload.asset_id);
    push_u8(&mut out, payload.route_id_codec);
    push_vec(&mut out, &payload.route_id);
    out
}

pub fn canonical_transfer_payload_bytes(payload: &TransferPayloadV1) -> Vec<u8> {
    let mut out = Vec::new();
    push_u8(&mut out, payload.version);
    push_u32(&mut out, payload.source_domain);
    push_u32(&mut out, payload.dest_domain);
    push_u64(&mut out, payload.nonce);
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

pub fn canonical_sccp_payload_bytes(payload: &SccpPayloadV1) -> Vec<u8> {
    let mut out = Vec::new();
    match payload {
        SccpPayloadV1::AssetRegister(payload) => {
            push_u8(&mut out, SccpPayloadV1::ASSET_REGISTER_DISCRIMINANT);
            out.extend_from_slice(&canonical_asset_register_payload_bytes(payload));
        }
        SccpPayloadV1::RouteActivate(payload) => {
            push_u8(&mut out, SccpPayloadV1::ROUTE_ACTIVATE_DISCRIMINANT);
            out.extend_from_slice(&canonical_route_activate_payload_bytes(payload));
        }
        SccpPayloadV1::Transfer(payload) => {
            push_u8(&mut out, SccpPayloadV1::TRANSFER_DISCRIMINANT);
            out.extend_from_slice(&canonical_transfer_payload_bytes(payload));
        }
        SccpPayloadV1::TokenAdd(payload) => {
            push_u8(&mut out, SccpPayloadV1::TOKEN_ADD_DISCRIMINANT);
            out.extend_from_slice(&canonical_token_add_payload_bytes(payload));
        }
        SccpPayloadV1::TokenPause(payload) => {
            push_u8(&mut out, SccpPayloadV1::TOKEN_PAUSE_DISCRIMINANT);
            out.extend_from_slice(&canonical_token_control_payload_bytes(payload));
        }
        SccpPayloadV1::TokenResume(payload) => {
            push_u8(&mut out, SccpPayloadV1::TOKEN_RESUME_DISCRIMINANT);
            out.extend_from_slice(&canonical_token_control_payload_bytes(payload));
        }
    }
    out
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

pub fn decode_canonical_sccp_payload_bytes(payload_bytes: &[u8]) -> Option<SccpPayloadV1> {
    let mut cursor = PayloadCursor::new(payload_bytes);
    let discriminant = cursor.take_u8()?;
    let payload = match discriminant {
        SccpPayloadV1::ASSET_REGISTER_DISCRIMINANT => {
            SccpPayloadV1::AssetRegister(AssetRegisterPayloadV1 {
                version: cursor.take_u8()?,
                target_domain: cursor.take_u32()?,
                home_domain: cursor.take_u32()?,
                nonce: cursor.take_u64()?,
                asset_id_codec: cursor.take_u8()?,
                asset_id: cursor.take_vec()?,
                decimals: cursor.take_u8()?,
            })
        }
        SccpPayloadV1::ROUTE_ACTIVATE_DISCRIMINANT => {
            SccpPayloadV1::RouteActivate(RouteActivatePayloadV1 {
                version: cursor.take_u8()?,
                source_domain: cursor.take_u32()?,
                target_domain: cursor.take_u32()?,
                nonce: cursor.take_u64()?,
                asset_id_codec: cursor.take_u8()?,
                asset_id: cursor.take_vec()?,
                route_id_codec: cursor.take_u8()?,
                route_id: cursor.take_vec()?,
            })
        }
        SccpPayloadV1::TRANSFER_DISCRIMINANT => SccpPayloadV1::Transfer(TransferPayloadV1 {
            version: cursor.take_u8()?,
            source_domain: cursor.take_u32()?,
            dest_domain: cursor.take_u32()?,
            nonce: cursor.take_u64()?,
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
        }),
        SccpPayloadV1::TOKEN_ADD_DISCRIMINANT => SccpPayloadV1::TokenAdd(TokenAddPayloadV1 {
            version: cursor.take_u8()?,
            target_domain: cursor.take_u32()?,
            nonce: cursor.take_u64()?,
            sora_asset_id: {
                let mut out = [0u8; 32];
                out.copy_from_slice(cursor.take_exact(32)?);
                out
            },
            decimals: cursor.take_u8()?,
            name: {
                let mut out = [0u8; 32];
                out.copy_from_slice(cursor.take_exact(32)?);
                out
            },
            symbol: {
                let mut out = [0u8; 32];
                out.copy_from_slice(cursor.take_exact(32)?);
                out
            },
        }),
        SccpPayloadV1::TOKEN_PAUSE_DISCRIMINANT => {
            SccpPayloadV1::TokenPause(TokenControlPayloadV1 {
                version: cursor.take_u8()?,
                target_domain: cursor.take_u32()?,
                nonce: cursor.take_u64()?,
                sora_asset_id: {
                    let mut out = [0u8; 32];
                    out.copy_from_slice(cursor.take_exact(32)?);
                    out
                },
            })
        }
        SccpPayloadV1::TOKEN_RESUME_DISCRIMINANT => {
            SccpPayloadV1::TokenResume(TokenControlPayloadV1 {
                version: cursor.take_u8()?,
                target_domain: cursor.take_u32()?,
                nonce: cursor.take_u64()?,
                sora_asset_id: {
                    let mut out = [0u8; 32];
                    out.copy_from_slice(cursor.take_exact(32)?);
                    out
                },
            })
        }
        _ => return None,
    };
    cursor.is_finished().then_some(payload)
}

fn h256_is_nonzero(value: &H256) -> bool {
    value.iter().any(|byte| *byte != 0)
}

fn fixed_ascii_field_is_non_empty(value: &[u8; 32]) -> bool {
    value
        .iter()
        .position(|byte| *byte == 0)
        .map_or(value.as_slice(), |end| &value[..end])
        .iter()
        .any(|byte| *byte != 0)
}

pub fn verify_sccp_payload_structure(payload: &SccpPayloadV1) -> bool {
    let target_domain = sccp_message_target_domain(payload);
    if !is_supported_domain(target_domain) {
        return false;
    }

    match payload {
        SccpPayloadV1::AssetRegister(payload) => {
            payload.version == 1
                && is_supported_domain(payload.home_domain)
                && validate_sccp_codec_bytes(payload.asset_id_codec, &payload.asset_id)
        }
        SccpPayloadV1::RouteActivate(payload) => {
            payload.version == 1
                && is_supported_domain(payload.source_domain)
                && payload.source_domain != payload.target_domain
                && validate_sccp_codec_bytes(payload.asset_id_codec, &payload.asset_id)
                && validate_sccp_codec_bytes(payload.route_id_codec, &payload.route_id)
        }
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
        SccpPayloadV1::TokenAdd(payload) => {
            payload.version == 1
                && is_supported_domain(payload.target_domain)
                && h256_is_nonzero(&payload.sora_asset_id)
                && fixed_ascii_field_is_non_empty(&payload.name)
                && fixed_ascii_field_is_non_empty(&payload.symbol)
        }
        SccpPayloadV1::TokenPause(payload) | SccpPayloadV1::TokenResume(payload) => {
            payload.version == 1
                && is_supported_domain(payload.target_domain)
                && h256_is_nonzero(&payload.sora_asset_id)
        }
    }
}

pub fn hub_commitment_from_sccp_payload(payload: &SccpPayloadV1) -> SccpHubCommitmentV1 {
    SccpHubCommitmentV1 {
        version: 1,
        kind: sccp_message_kind(payload),
        target_domain: sccp_message_target_domain(payload),
        message_id: sccp_message_id(payload),
        payload_hash: payload_hash(&canonical_sccp_payload_bytes(payload)),
    }
}

pub fn canonical_commitment_bytes(commitment: &SccpHubCommitmentV1) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + 1 + 4 + 32 + 32);
    push_u8(&mut out, commitment.version);
    push_u8(
        &mut out,
        match commitment.kind {
            SccpHubMessageKind::Burn => 0,
            SccpHubMessageKind::TokenAdd => 1,
            SccpHubMessageKind::TokenPause => 2,
            SccpHubMessageKind::TokenResume => 3,
            SccpHubMessageKind::AssetRegister => 4,
            SccpHubMessageKind::RouteActivate => 5,
            SccpHubMessageKind::Transfer => 6,
        },
    );
    push_u32(&mut out, commitment.target_domain);
    out.extend_from_slice(&commitment.message_id);
    out.extend_from_slice(&commitment.payload_hash);
    out
}

pub fn burn_message_id(payload: &BurnPayloadV1) -> H256 {
    prefixed_keccak(
        SCCP_MSG_PREFIX_BURN_V1,
        &canonical_burn_payload_bytes(payload),
    )
}

pub fn token_add_message_id(payload: &TokenAddPayloadV1) -> H256 {
    prefixed_keccak(
        SCCP_MSG_PREFIX_TOKEN_ADD_V1,
        &canonical_token_add_payload_bytes(payload),
    )
}

pub fn token_pause_message_id(payload: &TokenControlPayloadV1) -> H256 {
    prefixed_keccak(
        SCCP_MSG_PREFIX_TOKEN_PAUSE_V1,
        &canonical_token_control_payload_bytes(payload),
    )
}

pub fn token_resume_message_id(payload: &TokenControlPayloadV1) -> H256 {
    prefixed_keccak(
        SCCP_MSG_PREFIX_TOKEN_RESUME_V1,
        &canonical_token_control_payload_bytes(payload),
    )
}

pub fn asset_register_message_id(payload: &AssetRegisterPayloadV1) -> H256 {
    prefixed_keccak(
        SCCP_MSG_PREFIX_ASSET_REGISTER_V1,
        &canonical_asset_register_payload_bytes(payload),
    )
}

pub fn route_activate_message_id(payload: &RouteActivatePayloadV1) -> H256 {
    prefixed_keccak(
        SCCP_MSG_PREFIX_ROUTE_ACTIVATE_V1,
        &canonical_route_activate_payload_bytes(payload),
    )
}

pub fn transfer_message_id(payload: &TransferPayloadV1) -> H256 {
    prefixed_keccak(
        SCCP_MSG_PREFIX_TRANSFER_V1,
        &canonical_transfer_payload_bytes(payload),
    )
}

pub fn sccp_message_id(payload: &SccpPayloadV1) -> H256 {
    match payload {
        SccpPayloadV1::AssetRegister(payload) => asset_register_message_id(payload),
        SccpPayloadV1::RouteActivate(payload) => route_activate_message_id(payload),
        SccpPayloadV1::Transfer(payload) => transfer_message_id(payload),
        SccpPayloadV1::TokenAdd(payload) => token_add_message_id(payload),
        SccpPayloadV1::TokenPause(payload) => token_pause_message_id(payload),
        SccpPayloadV1::TokenResume(payload) => token_resume_message_id(payload),
    }
}

pub fn sccp_message_kind(payload: &SccpPayloadV1) -> SccpHubMessageKind {
    match payload {
        SccpPayloadV1::AssetRegister(_) => SccpHubMessageKind::AssetRegister,
        SccpPayloadV1::RouteActivate(_) => SccpHubMessageKind::RouteActivate,
        SccpPayloadV1::Transfer(_) => SccpHubMessageKind::Transfer,
        SccpPayloadV1::TokenAdd(_) => SccpHubMessageKind::TokenAdd,
        SccpPayloadV1::TokenPause(_) => SccpHubMessageKind::TokenPause,
        SccpPayloadV1::TokenResume(_) => SccpHubMessageKind::TokenResume,
    }
}

pub fn sccp_message_target_domain(payload: &SccpPayloadV1) -> u32 {
    match payload {
        SccpPayloadV1::AssetRegister(payload) => payload.target_domain,
        SccpPayloadV1::RouteActivate(payload) => payload.target_domain,
        SccpPayloadV1::Transfer(payload) => payload.dest_domain,
        SccpPayloadV1::TokenAdd(payload) => payload.target_domain,
        SccpPayloadV1::TokenPause(payload) | SccpPayloadV1::TokenResume(payload) => {
            payload.target_domain
        }
    }
}

pub fn sccp_message_source_domain(payload: &SccpPayloadV1) -> u32 {
    match payload {
        SccpPayloadV1::AssetRegister(payload) => payload.home_domain,
        SccpPayloadV1::RouteActivate(payload) => payload.source_domain,
        SccpPayloadV1::Transfer(payload) => payload.source_domain,
        SccpPayloadV1::TokenAdd(_)
        | SccpPayloadV1::TokenPause(_)
        | SccpPayloadV1::TokenResume(_) => SCCP_DOMAIN_SORA,
    }
}

pub fn payload_hash(payload: &[u8]) -> H256 {
    prefixed_blake2b(SCCP_PAYLOAD_HASH_PREFIX_V1, payload)
}

pub fn sccp_source_event_digest(
    source_domain: u32,
    target_domain: u32,
    message_id: H256,
    payload_hash: H256,
) -> H256 {
    let mut out = Vec::with_capacity(1 + 4 + 4 + 32 + 32);
    push_u8(&mut out, 1);
    push_u32(&mut out, source_domain);
    push_u32(&mut out, target_domain);
    out.extend_from_slice(&message_id);
    out.extend_from_slice(&payload_hash);
    prefixed_blake2b(SCCP_SOURCE_EVENT_DIGEST_PREFIX_V1, &out)
}

pub fn sccp_source_event_leaf_hash(source_event_digest: H256) -> H256 {
    prefixed_blake2b(SCCP_SOURCE_EVENT_LEAF_PREFIX_V1, &source_event_digest)
}

pub fn canonical_sccp_solana_message_proof_bytes(
    source_event_digest: H256,
    receipt_or_message_root: H256,
    inclusion_branch: &[Vec<u8>],
) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    push_u8(&mut out, 1);
    out.extend_from_slice(&source_event_digest);
    out.extend_from_slice(&receipt_or_message_root);
    push_u32(
        &mut out,
        u32::try_from(inclusion_branch.len()).expect("SCCP Solana branch length fits into u32"),
    );
    for sibling in inclusion_branch {
        let sibling: H256 = sibling.as_slice().try_into().ok()?;
        out.extend_from_slice(&sibling);
    }
    Some(out)
}

pub fn sccp_solana_message_proof_hash(
    source_event_digest: H256,
    receipt_or_message_root: H256,
    inclusion_branch: &[Vec<u8>],
) -> Option<H256> {
    Some(prefixed_blake2b(
        SCCP_SOLANA_MESSAGE_PROOF_PREFIX_V1,
        &canonical_sccp_solana_message_proof_bytes(
            source_event_digest,
            receipt_or_message_root,
            inclusion_branch,
        )?,
    ))
}

pub fn sccp_source_finalized_header_hash(
    source_domain: u32,
    finality_model: SccpProofFinalityModelV1,
    finality_height: u64,
    finality_block_hash: H256,
    receipt_or_message_root: H256,
) -> H256 {
    let mut out = Vec::with_capacity(1 + 4 + 1 + 8 + 32 + 32);
    push_u8(&mut out, 1);
    push_u32(&mut out, source_domain);
    push_u8(&mut out, sccp_proof_finality_model_code(finality_model));
    push_u64(&mut out, finality_height);
    out.extend_from_slice(&finality_block_hash);
    out.extend_from_slice(&receipt_or_message_root);
    prefixed_blake2b(SCCP_SOURCE_HEADER_PREFIX_V1, &out)
}

pub fn commitment_leaf_hash(commitment: &SccpHubCommitmentV1) -> H256 {
    prefixed_blake2b(
        SCCP_HUB_LEAF_PREFIX_V1,
        &canonical_commitment_bytes(commitment),
    )
}

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

pub fn sccp_source_message_root_from_branch(
    leaf_hash: H256,
    leaf_index: u64,
    inclusion_branch: &[Vec<u8>],
) -> Option<H256> {
    let mut current = leaf_hash;
    let mut index = leaf_index;
    for sibling in inclusion_branch {
        let sibling: H256 = sibling.as_slice().try_into().ok()?;
        current = if index & 1 == 0 {
            hash_source_merkle_node(&current, &sibling)
        } else {
            hash_source_merkle_node(&sibling, &current)
        };
        index >>= 1;
    }
    Some(current)
}

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

fn runtime_kind_from_hub_kind(kind: SccpHubMessageKind) -> SccpRuntimeProofKindV1 {
    match kind {
        SccpHubMessageKind::Burn => SccpRuntimeProofKindV1::Burn,
        SccpHubMessageKind::TokenAdd => SccpRuntimeProofKindV1::TokenAdd,
        SccpHubMessageKind::TokenPause => SccpRuntimeProofKindV1::TokenPause,
        SccpHubMessageKind::TokenResume => SccpRuntimeProofKindV1::TokenResume,
        SccpHubMessageKind::AssetRegister => SccpRuntimeProofKindV1::AssetRegister,
        SccpHubMessageKind::RouteActivate => SccpRuntimeProofKindV1::RouteActivate,
        SccpHubMessageKind::Transfer => SccpRuntimeProofKindV1::Transfer,
    }
}

fn runtime_kind_code(kind: SccpRuntimeProofKindV1) -> u8 {
    match kind {
        SccpRuntimeProofKindV1::Burn => 0,
        SccpRuntimeProofKindV1::TokenAdd => 1,
        SccpRuntimeProofKindV1::TokenPause => 2,
        SccpRuntimeProofKindV1::TokenResume => 3,
        SccpRuntimeProofKindV1::AssetRegister => 4,
        SccpRuntimeProofKindV1::RouteActivate => 5,
        SccpRuntimeProofKindV1::Transfer => 6,
    }
}

fn runtime_commitment_from_hub(commitment: &SccpHubCommitmentV1) -> SccpRuntimeHubCommitmentV1 {
    SccpRuntimeHubCommitmentV1 {
        version: commitment.version,
        kind: runtime_kind_from_hub_kind(commitment.kind),
        target_domain: commitment.target_domain,
        message_id: commitment.message_id,
        payload_hash: commitment.payload_hash,
    }
}

fn runtime_merkle_proof_from_hub(proof: &SccpMerkleProofV1) -> SccpRuntimeMerkleProofV1 {
    SccpRuntimeMerkleProofV1 {
        steps: proof
            .steps
            .iter()
            .map(|step| SccpRuntimeMerkleStepV1 {
                sibling_hash: step.sibling_hash,
                sibling_is_left: step.sibling_is_left,
            })
            .collect(),
    }
}

fn runtime_payload_from_sccp_payload(payload: &SccpPayloadV1) -> SccpRuntimePayloadV1 {
    match payload {
        SccpPayloadV1::AssetRegister(payload) => {
            SccpRuntimePayloadV1::AssetRegister(payload.clone())
        }
        SccpPayloadV1::RouteActivate(payload) => {
            SccpRuntimePayloadV1::RouteActivate(payload.clone())
        }
        SccpPayloadV1::Transfer(payload) => SccpRuntimePayloadV1::Transfer(payload.clone()),
        SccpPayloadV1::TokenAdd(payload) => SccpRuntimePayloadV1::TokenAdd(*payload),
        SccpPayloadV1::TokenPause(payload) => SccpRuntimePayloadV1::TokenPause(*payload),
        SccpPayloadV1::TokenResume(payload) => SccpRuntimePayloadV1::TokenResume(*payload),
    }
}

pub fn sccp_runtime_validator_set_anchor_hash(qc: &NexusCommitQcV1) -> H256 {
    let mut out = Vec::new();
    push_u16(&mut out, qc.validator_set_hash_version);
    push_scale_vec(&mut out, qc.mode_tag.as_bytes());
    push_scale_compact(
        &mut out,
        u32::try_from(qc.validator_public_keys.len()).expect("validator set length fits u32"),
    );
    for public_key in &qc.validator_public_keys {
        push_scale_vec(&mut out, public_key.as_bytes());
    }
    push_scale_compact(
        &mut out,
        u32::try_from(qc.validator_set_pops.len()).expect("validator POP length fits u32"),
    );
    for pop in &qc.validator_set_pops {
        push_scale_vec(&mut out, pop);
    }
    prefixed_blake2b(b"sccp:nexus:validator-set-anchor:v1", &out)
}

fn runtime_finality_from_nexus_finality(
    finality: &NexusBridgeFinalityProofV1,
) -> Option<SccpRuntimeFinalityProofV1> {
    let signer_count = signer_indices_from_bitmap(
        &finality.commit_qc.signers_bitmap,
        finality.commit_qc.validator_public_keys.len(),
    )?
    .len();
    Some(SccpRuntimeFinalityProofV1 {
        version: 1,
        epoch: finality.commit_qc.epoch,
        height: finality.height,
        block_hash: finality.block_hash,
        commitment_root: finality.commitment_root,
        validator_set_hash: sccp_runtime_validator_set_anchor_hash(&finality.commit_qc),
        signature_count: u16::try_from(signer_count).ok()?,
    })
}

fn runtime_finality_from_source_chain_proof(
    proof: &SccpSourceChainProofEnvelopeV1,
) -> SccpRuntimeFinalityProofV1 {
    SccpRuntimeFinalityProofV1 {
        version: 1,
        epoch: 0,
        height: proof.finality_height,
        block_hash: proof.finality_block_hash,
        commitment_root: proof.commitment_root,
        validator_set_hash: sccp_source_chain_proof_envelope_hash(proof),
        signature_count: 0,
    }
}

pub fn sccp_runtime_envelope_from_message_bundle(
    bundle: &NexusSccpMessageProofV1,
) -> Option<SccpRuntimeProofEnvelopeV1> {
    if !verify_message_bundle_structure(bundle) {
        return None;
    }
    let source_domain = sccp_message_source_domain(&bundle.payload);
    let target_domain = sccp_message_target_domain(&bundle.payload);
    let finality_proof = if source_domain == SCCP_DOMAIN_SORA {
        let finality = decode_nexus_bridge_finality_proof(&bundle.finality_proof)?;
        runtime_finality_from_nexus_finality(&finality)?
    } else {
        let source_proof = decode_sccp_source_chain_proof_envelope(&bundle.finality_proof)?;
        if !verify_sccp_source_chain_proof_binding(
            &source_proof,
            bundle,
            source_domain,
            target_domain,
        ) {
            return None;
        }
        runtime_finality_from_source_chain_proof(&source_proof)
    };
    Some(SccpRuntimeProofEnvelopeV1 {
        version: 1,
        commitment_root: bundle.commitment_root,
        commitment: runtime_commitment_from_hub(&bundle.commitment),
        merkle_proof: runtime_merkle_proof_from_hub(&bundle.merkle_proof),
        payload: runtime_payload_from_sccp_payload(&bundle.payload),
        finality_proof,
    })
}

pub fn sccp_runtime_envelope_bytes_from_message_bundle(
    bundle: &NexusSccpMessageProofV1,
) -> Option<Vec<u8>> {
    Some(encode_sccp_runtime_proof_envelope(
        &sccp_runtime_envelope_from_message_bundle(bundle)?,
    ))
}

pub fn encode_sccp_runtime_proof_envelope(envelope: &SccpRuntimeProofEnvelopeV1) -> Vec<u8> {
    let mut out = Vec::new();
    push_u8(&mut out, envelope.version);
    out.extend_from_slice(&envelope.commitment_root);
    push_runtime_commitment(&mut out, &envelope.commitment);
    push_runtime_merkle_proof(&mut out, &envelope.merkle_proof);
    push_runtime_payload(&mut out, &envelope.payload);
    push_runtime_finality(&mut out, &envelope.finality_proof);
    out
}

fn push_runtime_commitment(out: &mut Vec<u8>, commitment: &SccpRuntimeHubCommitmentV1) {
    push_u8(out, commitment.version);
    push_u8(out, runtime_kind_code(commitment.kind));
    push_u32(out, commitment.target_domain);
    out.extend_from_slice(&commitment.message_id);
    out.extend_from_slice(&commitment.payload_hash);
}

fn push_runtime_merkle_proof(out: &mut Vec<u8>, proof: &SccpRuntimeMerkleProofV1) {
    push_scale_compact(
        out,
        u32::try_from(proof.steps.len()).expect("merkle proof length fits u32"),
    );
    for step in &proof.steps {
        out.extend_from_slice(&step.sibling_hash);
        push_u8(out, u8::from(step.sibling_is_left));
    }
}

fn push_runtime_encoded_payload(out: &mut Vec<u8>, codec: u8, bytes: &[u8]) {
    push_u8(out, codec);
    push_scale_vec(out, bytes);
}

fn push_runtime_payload(out: &mut Vec<u8>, payload: &SccpRuntimePayloadV1) {
    match payload {
        SccpRuntimePayloadV1::AssetRegister(payload) => {
            push_u8(out, 0);
            push_u32(out, payload.target_domain);
            push_u32(out, payload.home_domain);
            push_u64(out, payload.nonce);
            push_runtime_encoded_payload(out, payload.asset_id_codec, &payload.asset_id);
            push_u8(out, payload.decimals);
        }
        SccpRuntimePayloadV1::RouteActivate(payload) => {
            push_u8(out, 1);
            push_u32(out, payload.source_domain);
            push_u32(out, payload.target_domain);
            push_u64(out, payload.nonce);
            push_runtime_encoded_payload(out, payload.asset_id_codec, &payload.asset_id);
            push_runtime_encoded_payload(out, payload.route_id_codec, &payload.route_id);
        }
        SccpRuntimePayloadV1::Transfer(payload) => {
            push_u8(out, 2);
            push_u32(out, payload.source_domain);
            push_u32(out, payload.dest_domain);
            push_u64(out, payload.nonce);
            push_u32(out, payload.asset_home_domain);
            push_runtime_encoded_payload(out, payload.asset_id_codec, &payload.asset_id);
            push_u128(out, payload.amount);
            push_runtime_encoded_payload(out, payload.sender_codec, &payload.sender);
            push_runtime_encoded_payload(out, payload.recipient_codec, &payload.recipient);
            push_runtime_encoded_payload(out, payload.route_id_codec, &payload.route_id);
        }
        SccpRuntimePayloadV1::TokenAdd(payload) => {
            push_u8(out, 3);
            push_u32(out, payload.target_domain);
            push_u64(out, payload.nonce);
            out.extend_from_slice(&payload.sora_asset_id);
            push_u8(out, payload.decimals);
            out.extend_from_slice(&payload.name);
            out.extend_from_slice(&payload.symbol);
        }
        SccpRuntimePayloadV1::TokenPause(payload) => {
            push_u8(out, 4);
            push_u32(out, payload.target_domain);
            push_u64(out, payload.nonce);
            out.extend_from_slice(&payload.sora_asset_id);
        }
        SccpRuntimePayloadV1::TokenResume(payload) => {
            push_u8(out, 5);
            push_u32(out, payload.target_domain);
            push_u64(out, payload.nonce);
            out.extend_from_slice(&payload.sora_asset_id);
        }
    }
}

fn push_runtime_finality(out: &mut Vec<u8>, finality: &SccpRuntimeFinalityProofV1) {
    push_u8(out, finality.version);
    push_u64(out, finality.epoch);
    push_u64(out, finality.height);
    out.extend_from_slice(&finality.block_hash);
    out.extend_from_slice(&finality.commitment_root);
    out.extend_from_slice(&finality.validator_set_hash);
    push_u16(out, finality.signature_count);
}

pub fn decode_nexus_bridge_finality_proof(
    proof_bytes: &[u8],
) -> Option<NexusBridgeFinalityProofV1> {
    norito::decode_from_bytes(proof_bytes).ok()
}

pub fn decode_sccp_source_chain_proof_envelope(
    proof_bytes: &[u8],
) -> Option<SccpSourceChainProofEnvelopeV1> {
    norito::decode_from_bytes(proof_bytes).ok()
}

pub fn decode_sccp_source_consensus_proof(
    proof_bytes: &[u8],
) -> Option<SccpSourceConsensusProofV1> {
    norito::decode_from_bytes(proof_bytes).ok()
}

pub fn decode_sccp_source_message_inclusion_proof(
    proof_bytes: &[u8],
) -> Option<SccpSourceMessageInclusionProofV1> {
    norito::decode_from_bytes(proof_bytes).ok()
}

pub fn decode_nexus_sccp_burn_proof(proof_bytes: &[u8]) -> Option<NexusSccpBurnProofV1> {
    norito::decode_from_bytes(proof_bytes).ok()
}

pub fn decode_nexus_sccp_message_proof(proof_bytes: &[u8]) -> Option<NexusSccpMessageProofV1> {
    norito::decode_from_bytes(proof_bytes).ok()
}

pub fn decode_nexus_sccp_message_transparent_proof(
    proof_bytes: &[u8],
) -> Option<NexusSccpMessageTransparentProofV1> {
    norito::decode_from_bytes(proof_bytes).ok()
}

pub fn recover_nexus_sccp_message_transparent_proof(
    backend: &str,
    proof_bytes: &[u8],
) -> Option<NexusSccpMessageTransparentProofV1> {
    recover_nexus_sccp_message_transparent_proof_allow_unready(backend, proof_bytes, false)
}

pub fn recover_nexus_sccp_message_transparent_proof_allow_unready(
    backend: &str,
    proof_bytes: &[u8],
    allow_unready: bool,
) -> Option<NexusSccpMessageTransparentProofV1> {
    let proof = decode_nexus_sccp_message_transparent_proof(proof_bytes)?;
    (verify_nexus_sccp_message_transparent_proof_structure_allow_unready(&proof, allow_unready)
        && proof.message_backend == backend)
        .then_some(proof)
}

pub fn verify_nexus_bridge_finality_proof_structure(proof: &NexusBridgeFinalityProofV1) -> bool {
    if proof.version != 1
        || proof.chain_id.is_empty()
        || proof.height == 0
        || proof.block_header_bytes.is_empty()
    {
        return false;
    }
    let qc = &proof.commit_qc;
    if qc.version != 1
        || qc.phase != NexusConsensusPhaseV1::Commit
        || qc.height != proof.height
        || qc.subject_block_hash != proof.block_hash
        || qc.mode_tag.is_empty()
        || qc.validator_set_hash_version != 1
        || qc.validator_public_keys.is_empty()
        || qc.validator_set_pops.len() != qc.validator_public_keys.len()
        || qc.bls_aggregate_signature.is_empty()
    {
        return false;
    }

    for (idx, public_key) in qc.validator_public_keys.iter().enumerate() {
        if public_key.is_empty() {
            return false;
        }
        if qc.validator_public_keys[..idx]
            .iter()
            .any(|known| known == public_key)
        {
            return false;
        }
    }
    for pop in &qc.validator_set_pops {
        if pop.is_empty() {
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

fn verify_sccp_source_chain_proof_material_with_material(
    proof: &SccpSourceChainProofEnvelopeV1,
    material: &SccpSourceVerifierMaterialV1,
) -> bool {
    let Some(consensus_proof) = decode_sccp_source_consensus_proof(&proof.consensus_proof) else {
        return false;
    };
    let Some(inclusion_proof) =
        decode_sccp_source_message_inclusion_proof(&proof.message_inclusion_proof)
    else {
        return false;
    };

    let expected_header_hash = sccp_source_finalized_header_hash(
        proof.source_domain,
        proof.finality_model,
        proof.finality_height,
        proof.finality_block_hash,
        proof.receipt_or_message_root,
    );
    let expected_adapter_transcript_hash = sccp_source_adapter_transcript_hash(
        proof.source_domain,
        proof.target_domain,
        proof.source_proof_plan,
        proof.finality_model,
        proof.finality_height,
        proof.finality_block_hash,
        proof.receipt_or_message_root,
        proof.source_event_digest,
        &consensus_proof.adapter_proof,
    );
    if consensus_proof.version != 1
        || consensus_proof.source_domain != proof.source_domain
        || consensus_proof.source_chain != proof.source_chain
        || consensus_proof.source_proof_plan != proof.source_proof_plan
        || consensus_proof.finality_model != proof.finality_model
        || consensus_proof.finality_height != proof.finality_height
        || consensus_proof.finality_block_hash != proof.finality_block_hash
        || consensus_proof.receipt_or_message_root != proof.receipt_or_message_root
        || consensus_proof.finalized_header_hash != proof.finalized_header_hash
        || proof.finalized_header_hash != expected_header_hash
        || consensus_proof.adapter_transcript_hash != expected_adapter_transcript_hash
        || !h256_is_nonzero(&consensus_proof.adapter_transcript_hash)
        || !verify_sccp_source_adapter_proof_binding(&consensus_proof.adapter_proof, proof)
        || !verify_sccp_source_verifier_evidence_with_material(
            proof,
            &consensus_proof.adapter_proof,
            expected_adapter_transcript_hash,
            &consensus_proof.verifier_evidence,
            material,
        )
        || !verify_sccp_source_adapter_verification_proof(
            proof,
            &consensus_proof.adapter_proof,
            expected_adapter_transcript_hash,
            &consensus_proof.verifier_evidence,
            &consensus_proof.adapter_verification_proof,
        )
    {
        return false;
    }

    let expected_leaf_hash = sccp_source_event_leaf_hash(proof.source_event_digest);
    let Some(reconstructed_root) = sccp_source_message_root_from_branch(
        inclusion_proof.source_event_leaf_hash,
        inclusion_proof.leaf_index,
        &proof.inclusion_branch,
    ) else {
        return false;
    };
    inclusion_proof.version == 1
        && inclusion_proof.source_domain == proof.source_domain
        && inclusion_proof.target_domain == proof.target_domain
        && inclusion_proof.message_id == proof.message_id
        && inclusion_proof.payload_hash == proof.payload_hash
        && inclusion_proof.source_event_digest == proof.source_event_digest
        && inclusion_proof.source_event_leaf_hash == expected_leaf_hash
        && inclusion_proof.receipt_or_message_root == proof.receipt_or_message_root
        && reconstructed_root == proof.receipt_or_message_root
}

fn verify_sccp_source_chain_proof_material(proof: &SccpSourceChainProofEnvelopeV1) -> bool {
    let Some(material) = sccp_source_verifier_material_for_domain(proof.source_domain) else {
        return false;
    };
    verify_sccp_source_chain_proof_material_with_material(proof, &material)
}

fn verify_sccp_source_adapter_verification_proof(
    proof: &SccpSourceChainProofEnvelopeV1,
    adapter_proof: &SccpSourceAdapterProofV1,
    adapter_transcript_hash: H256,
    verifier_evidence: &SccpSourceVerifierEvidenceV1,
    adapter_verification_proof: &SccpSourceAdapterVerificationProofV1,
) -> bool {
    if adapter_verification_proof.version != 1
        || adapter_verification_proof.proof_family != SCCP_STARK_FRI_PROOF_FAMILY_V1
        || adapter_verification_proof.circuit_id != SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID_V1
        || adapter_verification_proof.proof_bytes.is_empty()
    {
        return false;
    }
    let verifier_evidence_hash = sccp_source_verifier_evidence_hash(verifier_evidence);
    let Some(batch) = build_sccp_source_adapter_fastpq_batch(
        proof,
        adapter_proof,
        adapter_transcript_hash,
        verifier_evidence_hash,
    ) else {
        return false;
    };
    let Some((env, open, raw_proof)) =
        decode_sccp_stark_open_verify_proof(&adapter_verification_proof.proof_bytes)
    else {
        return false;
    };
    if env.circuit_id != SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID_V1
        || env.vk_hash != sccp_source_adapter_fastpq_verifier_commitment(proof).unwrap_or([0u8; 32])
        || env.public_inputs != sccp_source_adapter_open_verify_schema_descriptor(proof)
        || !env.aux.is_empty()
        || open.public_inputs
            != sccp_source_adapter_public_input_columns(
                proof,
                adapter_transcript_hash,
                verifier_evidence_hash,
            )
    {
        return false;
    }
    fastpq_prover::verify(&batch, &raw_proof).is_ok()
}

fn verify_sccp_source_adapter_proof_binding(
    adapter: &SccpSourceAdapterProofV1,
    proof: &SccpSourceChainProofEnvelopeV1,
) -> bool {
    match adapter {
        SccpSourceAdapterProofV1::EthereumBeaconReceipt(adapter) => {
            proof.source_domain == SCCP_DOMAIN_ETH
                && proof.source_proof_plan == SccpSourceProofPlanV1::EthereumBeaconReceiptProof
                && proof.finality_model == SccpProofFinalityModelV1::EthereumBeaconExecution
                && adapter.version == 1
                && adapter.source_domain == proof.source_domain
                && adapter.beacon_slot != 0
                && adapter.execution_block_number == proof.finality_height
                && adapter.execution_block_hash == proof.finality_block_hash
                && adapter.execution_receipts_root == proof.receipt_or_message_root
                && h256_is_nonzero(&adapter.beacon_finalized_root)
                && h256_is_nonzero(&adapter.sync_committee_root)
                && h256_is_nonzero(&adapter.receipt_trie_proof_hash)
        }
        SccpSourceAdapterProofV1::BscValidatorSetReceipt(adapter) => {
            proof.source_domain == SCCP_DOMAIN_BSC
                && proof.source_proof_plan == SccpSourceProofPlanV1::BscValidatorSetReceiptProof
                && proof.finality_model == SccpProofFinalityModelV1::BscValidatorSet
                && adapter.version == 1
                && adapter.source_domain == proof.source_domain
                && adapter.validator_epoch != 0
                && adapter.block_number == proof.finality_height
                && adapter.block_hash == proof.finality_block_hash
                && adapter.receipts_root == proof.receipt_or_message_root
                && h256_is_nonzero(&adapter.validator_set_hash)
                && h256_is_nonzero(&adapter.commit_seal_hash)
                && h256_is_nonzero(&adapter.receipt_trie_proof_hash)
        }
        SccpSourceAdapterProofV1::SolanaFinalizedTransaction(adapter) => {
            let Some(expected_message_proof_hash) = sccp_solana_message_proof_hash(
                proof.source_event_digest,
                proof.receipt_or_message_root,
                &proof.inclusion_branch,
            ) else {
                return false;
            };
            proof.source_domain == SCCP_DOMAIN_SOL
                && proof.source_proof_plan == SccpSourceProofPlanV1::SolanaFinalizedTransactionProof
                && proof.finality_model == SccpProofFinalityModelV1::SolanaFinalizedSlot
                && adapter.version == 1
                && adapter.source_domain == proof.source_domain
                && adapter.finalized_slot == proof.finality_height
                && adapter.blockhash == proof.finality_block_hash
                && adapter.transaction_status_root == proof.receipt_or_message_root
                && h256_is_nonzero(&adapter.bank_hash)
                && adapter.message_proof_hash == expected_message_proof_hash
        }
        SccpSourceAdapterProofV1::TonMasterchainShard(adapter) => {
            proof.source_domain == SCCP_DOMAIN_TON
                && proof.source_proof_plan == SccpSourceProofPlanV1::TonMasterchainShardProof
                && proof.finality_model == SccpProofFinalityModelV1::TonMasterchain
                && adapter.version == 1
                && adapter.source_domain == proof.source_domain
                && adapter.masterchain_seqno == proof.finality_height
                && adapter.masterchain_block_hash == proof.finality_block_hash
                && adapter.transaction_root == proof.receipt_or_message_root
                && h256_is_nonzero(&adapter.shard_block_hash)
                && h256_is_nonzero(&adapter.shard_state_root)
                && h256_is_nonzero(&adapter.shard_proof_hash)
        }
        SccpSourceAdapterProofV1::TronDposReceipt(adapter) => {
            proof.source_domain == SCCP_DOMAIN_TRON
                && proof.source_proof_plan == SccpSourceProofPlanV1::TronDposReceiptProof
                && proof.finality_model == SccpProofFinalityModelV1::TronDpos
                && adapter.version == 1
                && adapter.source_domain == proof.source_domain
                && adapter.solid_block_number == proof.finality_height
                && adapter.block_hash == proof.finality_block_hash
                && adapter.receipt_root == proof.receipt_or_message_root
                && h256_is_nonzero(&adapter.witness_schedule_hash)
                && h256_is_nonzero(&adapter.transaction_root)
                && h256_is_nonzero(&adapter.receipt_proof_hash)
        }
        SccpSourceAdapterProofV1::SubstrateGrandpaEvent(adapter) => {
            matches!(
                proof.source_domain,
                SCCP_DOMAIN_SORA_KUSAMA | SCCP_DOMAIN_SORA_POLKADOT | SCCP_DOMAIN_SORA2
            ) && proof.source_proof_plan == SccpSourceProofPlanV1::SubstrateGrandpaEventProof
                && proof.finality_model == SccpProofFinalityModelV1::SubstrateGrandpa
                && adapter.version == 1
                && adapter.source_domain == proof.source_domain
                && adapter.finalized_block_number == proof.finality_height
                && adapter.grandpa_set_id != 0
                && adapter.block_hash == proof.finality_block_hash
                && adapter.events_root == proof.receipt_or_message_root
                && h256_is_nonzero(&adapter.authority_set_hash)
                && h256_is_nonzero(&adapter.storage_proof_hash)
        }
    }
}

fn verify_sccp_source_chain_proof_envelope_shape(proof: &SccpSourceChainProofEnvelopeV1) -> bool {
    if proof.version != 1
        || proof.source_domain == SCCP_DOMAIN_SORA
        || !is_supported_domain(proof.source_domain)
        || !is_supported_domain(proof.target_domain)
        || proof.source_domain == proof.target_domain
        || proof.source_chain != sccp_chain_key_for_domain(proof.source_domain).unwrap_or_default()
        || sccp_source_proof_plan_for_domain(proof.source_domain) != Some(proof.source_proof_plan)
        || sccp_proof_finality_model_for_domain(proof.source_domain) != Some(proof.finality_model)
        || proof.finality_height == 0
        || !h256_is_nonzero(&proof.message_id)
        || !h256_is_nonzero(&proof.payload_hash)
        || !h256_is_nonzero(&proof.source_event_digest)
        || !h256_is_nonzero(&proof.commitment_root)
        || !h256_is_nonzero(&proof.finality_block_hash)
        || !h256_is_nonzero(&proof.finalized_header_hash)
        || !h256_is_nonzero(&proof.receipt_or_message_root)
        || proof.consensus_proof.is_empty()
        || proof.message_inclusion_proof.is_empty()
        || proof.inclusion_branch.is_empty()
        || proof.inclusion_branch.iter().any(Vec::is_empty)
    {
        return false;
    }

    proof.source_event_digest
        == sccp_source_event_digest(
            proof.source_domain,
            proof.target_domain,
            proof.message_id,
            proof.payload_hash,
        )
}

pub fn verify_sccp_source_chain_proof_envelope_structure(
    proof: &SccpSourceChainProofEnvelopeV1,
) -> bool {
    verify_sccp_source_chain_proof_envelope_shape(proof)
        && verify_sccp_source_chain_proof_material(proof)
}

pub fn verify_sccp_source_chain_proof_envelope_structure_with_material(
    proof: &SccpSourceChainProofEnvelopeV1,
    material: &SccpSourceVerifierMaterialV1,
) -> bool {
    verify_sccp_source_chain_proof_envelope_shape(proof)
        && verify_sccp_source_chain_proof_material_with_material(proof, material)
}

/// Verify a source-chain proof envelope using the production source-adapter gate.
pub fn verify_sccp_source_chain_proof_envelope_production(
    proof: &SccpSourceChainProofEnvelopeV1,
) -> bool {
    verify_sccp_source_chain_proof_envelope_structure(proof)
        && sccp_source_adapter_ready_for_domain(proof.source_domain)
}

pub fn verify_sccp_source_chain_proof_envelope_production_with_material(
    proof: &SccpSourceChainProofEnvelopeV1,
    material: &SccpSourceVerifierMaterialV1,
) -> bool {
    verify_sccp_source_chain_proof_envelope_structure_with_material(proof, material)
        && sccp_source_verifier_material_is_production_ready(material)
}

fn verify_sccp_source_chain_proof_binding(
    proof: &SccpSourceChainProofEnvelopeV1,
    bundle: &NexusSccpMessageProofV1,
    source_domain: u32,
    target_domain: u32,
) -> bool {
    verify_sccp_source_chain_proof_envelope_structure(proof)
        && proof.source_domain == source_domain
        && proof.target_domain == target_domain
        && proof.message_id == bundle.commitment.message_id
        && proof.payload_hash == bundle.commitment.payload_hash
        && proof.commitment_root == bundle.commitment_root
}

fn verify_sccp_source_chain_proof_binding_with_material(
    proof: &SccpSourceChainProofEnvelopeV1,
    bundle: &NexusSccpMessageProofV1,
    source_domain: u32,
    target_domain: u32,
    material: &SccpSourceVerifierMaterialV1,
) -> bool {
    verify_sccp_source_chain_proof_envelope_structure_with_material(proof, material)
        && proof.source_domain == source_domain
        && proof.target_domain == target_domain
        && proof.message_id == bundle.commitment.message_id
        && proof.payload_hash == bundle.commitment.payload_hash
        && proof.commitment_root == bundle.commitment_root
}

fn verify_sccp_source_chain_proof_binding_for_production(
    proof: &SccpSourceChainProofEnvelopeV1,
    bundle: &NexusSccpMessageProofV1,
    source_domain: u32,
    target_domain: u32,
) -> bool {
    verify_sccp_source_chain_proof_envelope_production(proof)
        && proof.source_domain == source_domain
        && proof.target_domain == target_domain
        && proof.message_id == bundle.commitment.message_id
        && proof.payload_hash == bundle.commitment.payload_hash
        && proof.commitment_root == bundle.commitment_root
}

fn verify_sccp_source_chain_proof_binding_for_production_with_material(
    proof: &SccpSourceChainProofEnvelopeV1,
    bundle: &NexusSccpMessageProofV1,
    source_domain: u32,
    target_domain: u32,
    material: &SccpSourceVerifierMaterialV1,
) -> bool {
    verify_sccp_source_chain_proof_envelope_production_with_material(proof, material)
        && proof.source_domain == source_domain
        && proof.target_domain == target_domain
        && proof.message_id == bundle.commitment.message_id
        && proof.payload_hash == bundle.commitment.payload_hash
        && proof.commitment_root == bundle.commitment_root
}

/// Decode and verify the Nexus finality proof for a SORA-origin SCCP message bundle.
pub fn verified_sccp_message_nexus_finality_proof(
    bundle: &NexusSccpMessageProofV1,
) -> Option<NexusBridgeFinalityProofV1> {
    if !verify_message_bundle_structure(bundle) {
        return None;
    }
    let source_domain = sccp_message_source_domain(&bundle.payload);
    if source_domain != SCCP_DOMAIN_SORA {
        return None;
    }
    let finality_proof = decode_nexus_bridge_finality_proof(&bundle.finality_proof)?;
    (verify_nexus_bridge_finality_proof_structure(&finality_proof)
        && finality_proof.commitment_root == bundle.commitment_root)
        .then_some(finality_proof)
}

/// Decode and verify the source-chain proof envelope for a non-SORA SCCP message bundle.
pub fn verified_sccp_message_source_chain_proof_envelope(
    bundle: &NexusSccpMessageProofV1,
) -> Option<SccpSourceChainProofEnvelopeV1> {
    if !verify_message_bundle_structure(bundle) {
        return None;
    }
    let source_domain = sccp_message_source_domain(&bundle.payload);
    if source_domain == SCCP_DOMAIN_SORA {
        return None;
    }
    let target_domain = sccp_message_target_domain(&bundle.payload);
    let source_proof = decode_sccp_source_chain_proof_envelope(&bundle.finality_proof)?;
    verify_sccp_source_chain_proof_binding(&source_proof, bundle, source_domain, target_domain)
        .then_some(source_proof)
}

/// Decode and production-verify the source-chain proof envelope for a non-SORA SCCP message bundle.
pub fn verified_sccp_message_source_chain_proof_envelope_for_production(
    bundle: &NexusSccpMessageProofV1,
) -> Option<SccpSourceChainProofEnvelopeV1> {
    if !verify_message_bundle_structure(bundle) {
        return None;
    }
    let source_domain = sccp_message_source_domain(&bundle.payload);
    if source_domain == SCCP_DOMAIN_SORA {
        return None;
    }
    let target_domain = sccp_message_target_domain(&bundle.payload);
    let source_proof = decode_sccp_source_chain_proof_envelope(&bundle.finality_proof)?;
    verify_sccp_source_chain_proof_binding_for_production(
        &source_proof,
        bundle,
        source_domain,
        target_domain,
    )
    .then_some(source_proof)
}

pub fn verified_sccp_message_source_chain_proof_envelope_for_production_with_material(
    bundle: &NexusSccpMessageProofV1,
    material: &SccpSourceVerifierMaterialV1,
) -> Option<SccpSourceChainProofEnvelopeV1> {
    if !verify_message_bundle_structure_with_source_verifier_material(bundle, material) {
        return None;
    }
    let source_domain = sccp_message_source_domain(&bundle.payload);
    if source_domain == SCCP_DOMAIN_SORA {
        return None;
    }
    let target_domain = sccp_message_target_domain(&bundle.payload);
    let source_proof = decode_sccp_source_chain_proof_envelope(&bundle.finality_proof)?;
    verify_sccp_source_chain_proof_binding_for_production_with_material(
        &source_proof,
        bundle,
        source_domain,
        target_domain,
        material,
    )
    .then_some(source_proof)
}

fn sccp_message_finality_public_inputs_internal(
    bundle: &NexusSccpMessageProofV1,
    source_domain: u32,
    target_domain: u32,
    source_material: Option<&SccpSourceVerifierMaterialV1>,
) -> Option<(u64, H256)> {
    if source_domain == SCCP_DOMAIN_SORA {
        let finality_proof = decode_nexus_bridge_finality_proof(&bundle.finality_proof)?;
        (verify_nexus_bridge_finality_proof_structure(&finality_proof)
            && finality_proof.commitment_root == bundle.commitment_root)
            .then_some((finality_proof.height, finality_proof.block_hash))
    } else {
        let source_proof = decode_sccp_source_chain_proof_envelope(&bundle.finality_proof)?;
        let source_proof_is_bound = if let Some(material) = source_material {
            verify_sccp_source_chain_proof_binding_with_material(
                &source_proof,
                bundle,
                source_domain,
                target_domain,
                material,
            )
        } else {
            verify_sccp_source_chain_proof_binding(
                &source_proof,
                bundle,
                source_domain,
                target_domain,
            )
        };
        source_proof_is_bound.then_some((
            source_proof.finality_height,
            source_proof.finality_block_hash,
        ))
    }
}

pub fn nexus_commit_vote_preimage(chain_id: &str, certificate: &NexusCommitQcV1) -> Vec<u8> {
    let mut out = Vec::with_capacity(32 + 32 + 8 * 3 + 1);
    let domain = iroha_consensus_domain(chain_id, "Vote", b"v1", &certificate.mode_tag);
    out.extend_from_slice(&domain);
    out.extend_from_slice(&certificate.subject_block_hash);
    out.extend_from_slice(&certificate.height.to_be_bytes());
    out.extend_from_slice(&certificate.view.to_be_bytes());
    out.extend_from_slice(&certificate.epoch.to_be_bytes());
    out.push(certificate.phase as u8);
    out
}

pub fn verify_burn_bundle_structure(bundle: &NexusSccpBurnProofV1) -> bool {
    if bundle.version != 1 {
        return false;
    }
    if !verify_burn_payload_structure(&bundle.payload) {
        return false;
    }
    let Some(finality_proof) = decode_nexus_bridge_finality_proof(&bundle.finality_proof) else {
        return false;
    };
    if !verify_nexus_bridge_finality_proof_structure(&finality_proof)
        || finality_proof.commitment_root != bundle.commitment_root
    {
        return false;
    }
    if bundle.commitment.version != 1
        || bundle.commitment.kind != SccpHubMessageKind::Burn
        || bundle.commitment.target_domain != bundle.payload.dest_domain
        || bundle.commitment.message_id != burn_message_id(&bundle.payload)
        || bundle.commitment.payload_hash
            != payload_hash(&canonical_burn_payload_bytes(&bundle.payload))
    {
        return false;
    }

    merkle_root_from_commitment(&bundle.commitment, &bundle.merkle_proof) == bundle.commitment_root
}

pub fn verify_burn_payload_structure(payload: &BurnPayloadV1) -> bool {
    payload.version == 1
        && is_supported_domain(payload.source_domain)
        && is_supported_domain(payload.dest_domain)
        && payload.dest_domain != payload.source_domain
        && !(payload.dest_domain == SCCP_DOMAIN_SORA && payload.source_domain == SCCP_DOMAIN_SORA)
        && h256_is_nonzero(&payload.sora_asset_id)
        && payload.amount != 0
        && h256_is_nonzero(&payload.recipient)
}

fn verify_message_bundle_structure_internal(
    bundle: &NexusSccpMessageProofV1,
    source_material: Option<&SccpSourceVerifierMaterialV1>,
) -> bool {
    if bundle.version != 1 || bundle.commitment.version != 1 {
        return false;
    }

    let source_domain = sccp_message_source_domain(&bundle.payload);
    let target_domain = sccp_message_target_domain(&bundle.payload);
    let payload_bytes = canonical_sccp_payload_bytes(&bundle.payload);
    if !verify_sccp_payload_structure(&bundle.payload)
        || bundle.commitment.kind != sccp_message_kind(&bundle.payload)
        || bundle.commitment.target_domain != target_domain
        || bundle.commitment.message_id != sccp_message_id(&bundle.payload)
        || bundle.commitment.payload_hash != payload_hash(&payload_bytes)
    {
        return false;
    }

    sccp_message_finality_public_inputs_internal(
        bundle,
        source_domain,
        target_domain,
        source_material,
    )
    .is_some()
        && merkle_root_from_commitment(&bundle.commitment, &bundle.merkle_proof)
            == bundle.commitment_root
}

pub fn verify_message_bundle_structure(bundle: &NexusSccpMessageProofV1) -> bool {
    verify_message_bundle_structure_internal(bundle, None)
}

pub fn verify_message_bundle_structure_with_source_verifier_material(
    bundle: &NexusSccpMessageProofV1,
    material: &SccpSourceVerifierMaterialV1,
) -> bool {
    verify_message_bundle_structure_internal(bundle, Some(material))
}

fn prefixed_keccak(prefix: &[u8], payload: &[u8]) -> H256 {
    let mut keccak = tiny_keccak::Keccak::v256();
    keccak.update(prefix);
    keccak.update(payload);
    let mut out = [0u8; 32];
    keccak.finalize(&mut out);
    out
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

fn hash_source_merkle_node(left: &H256, right: &H256) -> H256 {
    let mut hasher = Blake2bVar::new(32).expect("fixed hash length");
    hasher.update(SCCP_SOURCE_NODE_PREFIX_V1);
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
    let mut hasher = Blake2bVar::new(32).expect("fixed hash length");
    hasher.update(b"iroha-sumeragi-consensus/v1");
    hasher.update(chain_id.as_bytes());
    hasher.update(mode_tag.as_bytes());
    hasher.update(&IROHA_CONSENSUS_PROTO_VERSION_V1.to_be_bytes());
    hasher.update(message_type_tag.as_bytes());
    hasher.update(extra);
    let mut out = [0u8; 32];
    hasher
        .finalize_variable(&mut out)
        .expect("fixed hash length");
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

#[cfg(test)]
mod tests {
    use super::*;
    use norito::to_bytes;

    fn sample_secp256k1_signer() -> iroha_crypto::KeyPair {
        iroha_crypto::KeyPair::from_seed(
            b"iroha:sccp:test:evm-attestor".to_vec(),
            iroha_crypto::Algorithm::Secp256k1,
        )
    }

    fn sample_ready_source_verifier_material(source_domain: u32) -> SccpSourceVerifierMaterialV1 {
        let mut material =
            sccp_source_verifier_material_for_domain(source_domain).expect("source material");
        let chain = sccp_chain_key_for_domain(source_domain).expect("source chain");
        material.placeholder_material = false;
        material.source_trust_anchor_id = format!("sccp:{chain}:source-trust-anchor:mainnet:v1");
        material.source_trust_anchor_hash = prefixed_blake2b(
            b"sccp:test:ready-source-material:trust-anchor",
            &source_domain.to_le_bytes(),
        );
        material.consensus_verifier_id = format!("sccp:{chain}:consensus-verifier:mainnet:v1");
        material.consensus_verifier_hash = prefixed_blake2b(
            b"sccp:test:ready-source-material:consensus-verifier",
            &source_domain.to_le_bytes(),
        );
        material.message_inclusion_verifier_id =
            format!("sccp:{chain}:message-inclusion-verifier:mainnet:v1");
        material.message_inclusion_verifier_hash = prefixed_blake2b(
            b"sccp:test:ready-source-material:message-inclusion",
            &source_domain.to_le_bytes(),
        );
        material.finality_policy_id = format!("sccp:{chain}:finality-policy:mainnet:v1");
        material.finality_policy_hash = prefixed_blake2b(
            b"sccp:test:ready-source-material:finality-policy",
            &source_domain.to_le_bytes(),
        );
        material
    }

    fn sample_reference_evm_attestation_manifest() -> SccpProofManifestV1 {
        let mut manifest = sccp_proof_manifest_for_domain(SCCP_DOMAIN_ETH).expect("eth manifest");
        manifest.verifier_backend = SccpVerifierBackendV1 {
            version: 1,
            family: SccpVerifierBackendFamilyV1::EvmSecp256k1Keccak,
            key: SCCP_EVM_SECP256K1_PROOF_BACKEND_V1.to_owned(),
        };
        manifest.production_ready = true;
        manifest.disabled_reason = None;
        manifest
    }

    fn test_h256_from_hex(value: &str) -> H256 {
        let raw = value
            .strip_prefix("0x")
            .or_else(|| value.strip_prefix("0X"))
            .unwrap_or(value)
            .as_bytes();
        assert_eq!(raw.len(), 64);
        let mut out = [0u8; 32];
        for (idx, chunk) in raw.chunks_exact(2).enumerate() {
            let hi = decode_ascii_hex_nibble(chunk[0]).expect("valid high nibble");
            let lo = decode_ascii_hex_nibble(chunk[1]).expect("valid low nibble");
            out[idx] = (hi << 4) | lo;
        }
        out
    }

    fn sample_evm_groth16_proof(
        public_inputs: &SccpMessageTransparentPublicInputsV1,
        source_domain: u32,
    ) -> SccpEvmGroth16Bn254ProofV1 {
        SccpEvmGroth16Bn254ProofV1 {
            version: 1,
            message_id: public_inputs.message_id,
            source_domain,
            commitment_root: public_inputs.commitment_root,
            a: [abi_word_u64(1), abi_word_u64(2)],
            b: [
                test_h256_from_hex(
                    "1800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed",
                ),
                test_h256_from_hex(
                    "198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c2",
                ),
                test_h256_from_hex(
                    "12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daa",
                ),
                test_h256_from_hex(
                    "090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975b",
                ),
            ],
            c: [abi_word_u64(1), abi_word_u64(2)],
        }
    }

    fn sample_evm_groth16_proof_bytes(
        public_inputs: &SccpMessageTransparentPublicInputsV1,
        source_domain: u32,
    ) -> Vec<u8> {
        encode_sccp_evm_groth16_bn254_proof_bytes(&sample_evm_groth16_proof(
            public_inputs,
            source_domain,
        ))
    }

    fn sample_finality_proof(commitment_root: H256) -> Vec<u8> {
        to_bytes(&NexusBridgeFinalityProofV1 {
            version: 1,
            chain_id: "00000000-0000-0000-0000-000000000753".to_owned(),
            height: 7,
            block_hash: [7u8; 32],
            commitment_root,
            block_header_bytes: vec![0x42; 4],
            commit_qc: NexusCommitQcV1 {
                version: 1,
                phase: NexusConsensusPhaseV1::Commit,
                height: 7,
                view: 0,
                epoch: 0,
                mode_tag: "iroha2-consensus::permissioned-sumeragi@v1".to_owned(),
                subject_block_hash: [7u8; 32],
                validator_set_hash_version: 1,
                validator_public_keys: vec![
                    "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2"
                        .to_owned(),
                ],
                validator_set_pops: vec![vec![1u8; 48]],
                signers_bitmap: vec![0b0000_0001],
                bls_aggregate_signature: vec![2u8; 96],
            },
        })
        .expect("encode finality proof")
    }

    fn sample_source_chain_proof_envelope(
        payload: &SccpPayloadV1,
        commitment: &SccpHubCommitmentV1,
        commitment_root: H256,
    ) -> SccpSourceChainProofEnvelopeV1 {
        sample_source_chain_proof_envelope_with_material(payload, commitment, commitment_root, None)
    }

    fn sample_source_chain_proof_envelope_with_material(
        payload: &SccpPayloadV1,
        commitment: &SccpHubCommitmentV1,
        commitment_root: H256,
        source_material: Option<&SccpSourceVerifierMaterialV1>,
    ) -> SccpSourceChainProofEnvelopeV1 {
        let source_domain = sccp_message_source_domain(payload);
        let target_domain = sccp_message_target_domain(payload);
        assert_ne!(
            source_domain, SCCP_DOMAIN_SORA,
            "SORA-origin messages use the Nexus bridge finality proof format"
        );
        let source_chain = sccp_chain_key_for_domain(source_domain)
            .expect("source domain chain key")
            .to_owned();
        let source_proof_plan =
            sccp_source_proof_plan_for_domain(source_domain).expect("source proof plan");
        let finality_model =
            sccp_proof_finality_model_for_domain(source_domain).expect("source finality model");
        let message_id = commitment.message_id;
        let payload_hash = commitment.payload_hash;
        let source_event_digest =
            sccp_source_event_digest(source_domain, target_domain, message_id, payload_hash);
        let source_event_leaf_hash = sccp_source_event_leaf_hash(source_event_digest);
        let inclusion_branch = vec![vec![0xC3; 32]];
        let receipt_or_message_root =
            sccp_source_message_root_from_branch(source_event_leaf_hash, 0, &inclusion_branch)
                .expect("source root");
        let solana_message_proof_hash = sccp_solana_message_proof_hash(
            source_event_digest,
            receipt_or_message_root,
            &inclusion_branch,
        )
        .expect("Solana source message proof hash");
        let finality_height = 10_000 + u64::from(source_domain);
        let finality_block_hash = [0x71; 32];
        let finalized_header_hash = sccp_source_finalized_header_hash(
            source_domain,
            finality_model,
            finality_height,
            finality_block_hash,
            receipt_or_message_root,
        );
        let adapter_proof = match source_domain {
            SCCP_DOMAIN_ETH => {
                SccpSourceAdapterProofV1::EthereumBeaconReceipt(SccpEvmBeaconSourceProofV1 {
                    version: 1,
                    source_domain,
                    beacon_slot: finality_height.saturating_mul(32),
                    execution_block_number: finality_height,
                    execution_block_hash: finality_block_hash,
                    execution_receipts_root: receipt_or_message_root,
                    beacon_finalized_root: [0xA1; 32],
                    sync_committee_root: [0xA2; 32],
                    receipt_trie_proof_hash: [0xA3; 32],
                })
            }
            SCCP_DOMAIN_BSC => {
                SccpSourceAdapterProofV1::BscValidatorSetReceipt(SccpBscValidatorSetSourceProofV1 {
                    version: 1,
                    source_domain,
                    validator_epoch: 17,
                    block_number: finality_height,
                    block_hash: finality_block_hash,
                    receipts_root: receipt_or_message_root,
                    validator_set_hash: [0xB1; 32],
                    commit_seal_hash: [0xB2; 32],
                    receipt_trie_proof_hash: [0xB3; 32],
                })
            }
            SCCP_DOMAIN_SOL => SccpSourceAdapterProofV1::SolanaFinalizedTransaction(
                SccpSolanaFinalizedSourceProofV1 {
                    version: 1,
                    source_domain,
                    finalized_slot: finality_height,
                    blockhash: finality_block_hash,
                    bank_hash: [0xC1; 32],
                    transaction_status_root: receipt_or_message_root,
                    message_proof_hash: solana_message_proof_hash,
                },
            ),
            SCCP_DOMAIN_TON => {
                SccpSourceAdapterProofV1::TonMasterchainShard(SccpTonMasterchainSourceProofV1 {
                    version: 1,
                    source_domain,
                    masterchain_seqno: finality_height,
                    masterchain_block_hash: finality_block_hash,
                    shard_block_hash: [0xD1; 32],
                    shard_state_root: [0xD2; 32],
                    transaction_root: receipt_or_message_root,
                    shard_proof_hash: [0xD3; 32],
                })
            }
            SCCP_DOMAIN_TRON => {
                SccpSourceAdapterProofV1::TronDposReceipt(SccpTronDposSourceProofV1 {
                    version: 1,
                    source_domain,
                    solid_block_number: finality_height,
                    block_hash: finality_block_hash,
                    witness_schedule_hash: [0xE1; 32],
                    receipt_root: receipt_or_message_root,
                    transaction_root: [0xE2; 32],
                    receipt_proof_hash: [0xE3; 32],
                })
            }
            SCCP_DOMAIN_SORA_KUSAMA | SCCP_DOMAIN_SORA_POLKADOT | SCCP_DOMAIN_SORA2 => {
                SccpSourceAdapterProofV1::SubstrateGrandpaEvent(SccpSubstrateGrandpaSourceProofV1 {
                    version: 1,
                    source_domain,
                    finalized_block_number: finality_height,
                    grandpa_set_id: 42,
                    block_hash: finality_block_hash,
                    authority_set_hash: [0xF1; 32],
                    events_root: receipt_or_message_root,
                    storage_proof_hash: [0xF2; 32],
                })
            }
            _ => panic!("unsupported sample SCCP source domain {source_domain}"),
        };
        let adapter_transcript_hash = sccp_source_adapter_transcript_hash(
            source_domain,
            target_domain,
            source_proof_plan,
            finality_model,
            finality_height,
            finality_block_hash,
            receipt_or_message_root,
            source_event_digest,
            &adapter_proof,
        );
        let mut envelope = SccpSourceChainProofEnvelopeV1 {
            version: 1,
            source_domain,
            target_domain,
            source_chain: source_chain.clone(),
            source_proof_plan,
            finality_model,
            message_id,
            payload_hash,
            source_event_digest,
            commitment_root,
            finality_height,
            finality_block_hash,
            finalized_header_hash,
            receipt_or_message_root,
            consensus_proof: Vec::new(),
            message_inclusion_proof: Vec::new(),
            inclusion_branch,
        };
        let adapter_verification_proof = if let Some(material) = source_material {
            build_sccp_source_adapter_verification_proof_with_material(
                &envelope,
                &adapter_proof,
                adapter_transcript_hash,
                material,
            )
        } else {
            build_sccp_source_adapter_verification_proof(
                &envelope,
                &adapter_proof,
                adapter_transcript_hash,
            )
        }
        .expect("build source adapter verification proof");
        let verifier_evidence = if let Some(material) = source_material {
            build_sccp_source_verifier_evidence_with_material(
                &envelope,
                &adapter_proof,
                adapter_transcript_hash,
                material,
            )
        } else {
            build_sccp_source_verifier_evidence(&envelope, &adapter_proof, adapter_transcript_hash)
        }
        .expect("build source verifier evidence");
        let consensus_proof = to_bytes(&SccpSourceConsensusProofV1 {
            version: 1,
            source_domain,
            source_chain: source_chain.clone(),
            source_proof_plan,
            finality_model,
            finality_height,
            finality_block_hash,
            receipt_or_message_root,
            finalized_header_hash,
            adapter_proof,
            adapter_transcript_hash,
            verifier_evidence,
            adapter_verification_proof,
        })
        .expect("encode source consensus proof");
        let message_inclusion_proof = to_bytes(&SccpSourceMessageInclusionProofV1 {
            version: 1,
            source_domain,
            target_domain,
            message_id,
            payload_hash,
            source_event_digest,
            source_event_leaf_hash,
            receipt_or_message_root,
            leaf_index: 0,
        })
        .expect("encode source message inclusion proof");
        envelope.consensus_proof = consensus_proof;
        envelope.message_inclusion_proof = message_inclusion_proof;
        envelope
    }

    fn sample_source_chain_proof(
        payload: &SccpPayloadV1,
        commitment: &SccpHubCommitmentV1,
        commitment_root: H256,
    ) -> Vec<u8> {
        to_bytes(&sample_source_chain_proof_envelope(
            payload,
            commitment,
            commitment_root,
        ))
        .expect("encode source chain proof envelope")
    }

    fn sample_source_chain_proof_with_material(
        payload: &SccpPayloadV1,
        commitment: &SccpHubCommitmentV1,
        commitment_root: H256,
        source_material: &SccpSourceVerifierMaterialV1,
    ) -> Vec<u8> {
        to_bytes(&sample_source_chain_proof_envelope_with_material(
            payload,
            commitment,
            commitment_root,
            Some(source_material),
        ))
        .expect("encode source chain proof envelope")
    }

    fn sample_message_bundle(payload: SccpPayloadV1) -> NexusSccpMessageProofV1 {
        sample_message_bundle_with_source_material(payload, None)
    }

    fn sample_message_bundle_with_source_material(
        payload: SccpPayloadV1,
        source_material: Option<&SccpSourceVerifierMaterialV1>,
    ) -> NexusSccpMessageProofV1 {
        let commitment = SccpHubCommitmentV1 {
            version: 1,
            kind: sccp_message_kind(&payload),
            target_domain: sccp_message_target_domain(&payload),
            message_id: sccp_message_id(&payload),
            payload_hash: payload_hash(&canonical_sccp_payload_bytes(&payload)),
        };
        let commitment_root = commitment_leaf_hash(&commitment);
        let source_domain = sccp_message_source_domain(&payload);
        let target_domain = sccp_message_target_domain(&payload);
        let finality_proof = if source_domain == SCCP_DOMAIN_SORA
            || !is_supported_domain(source_domain)
            || !is_supported_domain(target_domain)
            || sccp_source_proof_plan_for_domain(source_domain).is_none()
        {
            sample_finality_proof(commitment_root)
        } else if let Some(material) = source_material {
            sample_source_chain_proof_with_material(
                &payload,
                &commitment,
                commitment_root,
                material,
            )
        } else {
            sample_source_chain_proof(&payload, &commitment, commitment_root)
        };
        NexusSccpMessageProofV1 {
            version: 1,
            commitment_root,
            commitment,
            merkle_proof: SccpMerkleProofV1 { steps: Vec::new() },
            payload,
            finality_proof,
        }
    }

    fn assert_rejects_message_payload(payload: SccpPayloadV1) {
        assert!(!verify_sccp_payload_structure(&payload), "{payload:?}");
        assert!(!verify_message_bundle_structure(&sample_message_bundle(
            payload
        )));
    }

    fn sample_burn_bundle(nonce: u64) -> NexusSccpBurnProofV1 {
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce,
            sora_asset_id: [0x11; 32],
            amount: 42,
            recipient: [0x22; 32],
        };
        let commitment = SccpHubCommitmentV1 {
            version: 1,
            kind: SccpHubMessageKind::Burn,
            target_domain: payload.dest_domain,
            message_id: burn_message_id(&payload),
            payload_hash: payload_hash(&canonical_burn_payload_bytes(&payload)),
        };
        let commitment_root = commitment_leaf_hash(&commitment);
        NexusSccpBurnProofV1 {
            version: 1,
            commitment_root,
            commitment,
            merkle_proof: SccpMerkleProofV1 { steps: Vec::new() },
            payload,
            finality_proof: sample_finality_proof(commitment_root),
        }
    }

    fn sample_account_bytes(domain: u32) -> Vec<u8> {
        match domain {
            SCCP_DOMAIN_SORA
            | SCCP_DOMAIN_SORA_KUSAMA
            | SCCP_DOMAIN_SORA_POLKADOT
            | SCCP_DOMAIN_SORA2 => format!(
                "account@{}",
                sccp_chain_key_for_domain(domain).expect("domain chain key")
            )
            .into_bytes(),
            SCCP_DOMAIN_ETH | SCCP_DOMAIN_BSC => {
                b"0x1111111111111111111111111111111111111111".to_vec()
            }
            SCCP_DOMAIN_SOL => b"11111111111111111111111111111111".to_vec(),
            SCCP_DOMAIN_TON => {
                b"0:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_vec()
            }
            SCCP_DOMAIN_TRON => b"TJRabPrwbZy45sbavfcjinPJC18kjpRTv8".to_vec(),
            _ => panic!("unsupported sample SCCP domain {domain}"),
        }
    }

    fn sample_transfer_bundle(
        source_domain: u32,
        dest_domain: u32,
        nonce: u64,
    ) -> NexusSccpMessageProofV1 {
        sample_transfer_bundle_with_source_material(source_domain, dest_domain, nonce, None)
    }

    fn sample_transfer_bundle_with_source_material(
        source_domain: u32,
        dest_domain: u32,
        nonce: u64,
        source_material: Option<&SccpSourceVerifierMaterialV1>,
    ) -> NexusSccpMessageProofV1 {
        assert_ne!(source_domain, dest_domain);
        let source_chain = sccp_chain_key_for_domain(source_domain).expect("source chain key");
        let dest_chain = sccp_chain_key_for_domain(dest_domain).expect("destination chain key");
        sample_message_bundle_with_source_material(
            SccpPayloadV1::Transfer(TransferPayloadV1 {
                version: 1,
                source_domain,
                dest_domain,
                nonce,
                asset_home_domain: if source_domain == SCCP_DOMAIN_SORA {
                    SCCP_DOMAIN_SORA
                } else {
                    source_domain
                },
                asset_id_codec: SCCP_CODEC_TEXT_UTF8,
                asset_id: format!("{source_chain}#{dest_chain}#asset").into_bytes(),
                amount: 100 + u128::from(nonce),
                sender_codec: sccp_counterparty_account_codec(source_domain)
                    .expect("source account codec"),
                sender: sample_account_bytes(source_domain),
                recipient_codec: sccp_counterparty_account_codec(dest_domain)
                    .expect("destination account codec"),
                recipient: sample_account_bytes(dest_domain),
                route_id_codec: SCCP_CODEC_TEXT_UTF8,
                route_id: format!("{source_chain}:{dest_chain}:asset").into_bytes(),
            }),
            source_material,
        )
    }

    fn sample_tron_transfer_bundle(nonce: u64) -> NexusSccpMessageProofV1 {
        sample_message_bundle(SccpPayloadV1::Transfer(TransferPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_TRON,
            nonce,
            asset_home_domain: SCCP_DOMAIN_SORA,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#universal".to_vec(),
            amount: 100 + u128::from(nonce),
            sender_codec: SCCP_CODEC_TEXT_UTF8,
            sender: b"sora:bridge".to_vec(),
            recipient_codec: SCCP_CODEC_TRON_BASE58CHECK,
            recipient: b"TJRabPrwbZy45sbavfcjinPJC18kjpRTv8".to_vec(),
            route_id_codec: SCCP_CODEC_TEXT_UTF8,
            route_id: b"nexus:tron:xor".to_vec(),
        }))
    }

    fn sample_evm_transfer_bundle(nonce: u64) -> NexusSccpMessageProofV1 {
        sample_message_bundle(SccpPayloadV1::Transfer(TransferPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_ETH,
            nonce,
            asset_home_domain: SCCP_DOMAIN_SORA,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#universal".to_vec(),
            amount: 100 + u128::from(nonce),
            sender_codec: SCCP_CODEC_TEXT_UTF8,
            sender: b"sora:bridge".to_vec(),
            recipient_codec: SCCP_CODEC_EVM_HEX,
            recipient: b"0x1111111111111111111111111111111111111111".to_vec(),
            route_id_codec: SCCP_CODEC_TEXT_UTF8,
            route_id: b"nexus:eth:xor".to_vec(),
        }))
    }

    fn sample_valid_evm_submission_proof(
        nonce: u64,
    ) -> (SccpProofManifestV1, NexusSccpMessageTransparentProofV1) {
        let bundle = sample_evm_transfer_bundle(nonce);
        let manifest = sample_reference_evm_attestation_manifest();
        let public_inputs =
            sccp_message_transparent_public_inputs(&bundle).expect("message public inputs");
        let inner =
            build_sccp_message_transparent_inner_proof(&bundle, &manifest).expect("inner proof");
        let native_proof_bytes = vec![0xAA, 0xBB, 0xCC, 0xDD];
        let destination_binding =
            build_sccp_evm_destination_binding(&manifest, [0x11; 32], [0x33; 20], [0x22; 20]);
        let payload = build_sccp_evm_contract_submission_payload(
            &manifest,
            &native_proof_bytes,
            &public_inputs,
            inner.statement_hash,
            &destination_binding,
            &sample_secp256k1_signer(),
        )
        .expect("evm payload");
        let platform_payload = SccpPlatformSubmissionPayloadV1::EvmContractCall(payload);
        let arguments =
            sccp_submission_argument_values(&manifest.submission_template, &platform_payload)
                .expect("argument values");
        let submission_package = SccpCounterpartySubmissionPackageV1 {
            version: 1,
            proof_family: manifest.proof_family.clone(),
            verifier_backend: manifest.verifier_backend.clone(),
            envelope_encoding: sccp_submission_envelope_encoding(&manifest.submission_template),
            submission_kind: manifest.submission_template.submission_kind.clone(),
            verifier_entrypoint: manifest.submission_template.verifier_entrypoint.clone(),
            platform_payload,
            arguments: arguments.clone(),
            envelope_bytes: encode_sccp_submission_envelope(
                &manifest.submission_template,
                &arguments,
            ),
        };
        let proof = NexusSccpMessageTransparentProofV1 {
            version: 1,
            local_domain: manifest.local_domain,
            counterparty_domain: manifest.counterparty_domain,
            security_model: manifest.security_model,
            anchor_governance: manifest.anchor_governance,
            destination_binding: manifest.destination_binding.clone(),
            proof_family: manifest.proof_family.clone(),
            verifier_backend: manifest.verifier_backend.clone(),
            message_backend: manifest.message_backend.clone(),
            registry_backend: manifest.registry_backend.clone(),
            manifest_seed: manifest.manifest_seed.clone(),
            finality_model: manifest.finality_model,
            verifier_target: manifest.verifier_target,
            public_inputs,
            proof_bytes: native_proof_bytes,
            submission_package,
            bundle,
        };
        assert!(verify_sccp_evm_submission_package(&manifest, &proof));
        (manifest, proof)
    }

    fn evm_payload_from_proof(
        proof: &NexusSccpMessageTransparentProofV1,
    ) -> SccpEvmContractSubmissionPayloadV1 {
        let SccpPlatformSubmissionPayloadV1::EvmContractCall(payload) =
            &proof.submission_package.platform_payload
        else {
            panic!("sample proof must contain an EVM submission payload");
        };
        payload.clone()
    }

    fn replace_evm_payload(
        proof: &mut NexusSccpMessageTransparentProofV1,
        manifest: &SccpProofManifestV1,
        payload: SccpEvmContractSubmissionPayloadV1,
    ) {
        let platform_payload = SccpPlatformSubmissionPayloadV1::EvmContractCall(payload);
        let arguments =
            sccp_submission_argument_values(&manifest.submission_template, &platform_payload)
                .expect("argument values");
        proof.submission_package.platform_payload = platform_payload;
        proof.submission_package.arguments = arguments.clone();
        proof.submission_package.envelope_bytes =
            encode_sccp_submission_envelope(&manifest.submission_template, &arguments);
    }

    fn source_chain_proof_from_bundle(
        bundle: &NexusSccpMessageProofV1,
    ) -> SccpSourceChainProofEnvelopeV1 {
        decode_sccp_source_chain_proof_envelope(&bundle.finality_proof)
            .expect("decode source chain proof envelope")
    }

    fn encode_source_chain_proof(proof: &SccpSourceChainProofEnvelopeV1) -> Vec<u8> {
        to_bytes(proof).expect("encode source chain proof envelope")
    }

    fn source_consensus_proof_from_envelope(
        proof: &SccpSourceChainProofEnvelopeV1,
    ) -> SccpSourceConsensusProofV1 {
        decode_sccp_source_consensus_proof(&proof.consensus_proof)
            .expect("decode source consensus proof")
    }

    fn source_inclusion_proof_from_envelope(
        proof: &SccpSourceChainProofEnvelopeV1,
    ) -> SccpSourceMessageInclusionProofV1 {
        decode_sccp_source_message_inclusion_proof(&proof.message_inclusion_proof)
            .expect("decode source inclusion proof")
    }

    fn replace_source_consensus_proof(
        proof: &mut SccpSourceChainProofEnvelopeV1,
        consensus: &SccpSourceConsensusProofV1,
    ) {
        proof.consensus_proof = to_bytes(consensus).expect("encode source consensus proof");
    }

    fn replace_source_inclusion_proof(
        proof: &mut SccpSourceChainProofEnvelopeV1,
        inclusion: &SccpSourceMessageInclusionProofV1,
    ) {
        proof.message_inclusion_proof = to_bytes(inclusion).expect("encode source inclusion proof");
    }

    fn replace_source_adapter_proof(
        proof: &mut SccpSourceChainProofEnvelopeV1,
        adapter_proof: SccpSourceAdapterProofV1,
    ) {
        let mut consensus = source_consensus_proof_from_envelope(proof);
        consensus.adapter_proof = adapter_proof;
        replace_source_consensus_proof(proof, &consensus);
    }

    fn mutate_source_adapter_verification_proof<F>(
        proof: &mut SccpSourceChainProofEnvelopeV1,
        mutate: F,
    ) where
        F: FnOnce(&mut SccpSourceAdapterVerificationProofV1),
    {
        let mut consensus = source_consensus_proof_from_envelope(proof);
        mutate(&mut consensus.adapter_verification_proof);
        replace_source_consensus_proof(proof, &consensus);
    }

    fn mutate_source_verifier_evidence<F>(proof: &mut SccpSourceChainProofEnvelopeV1, mutate: F)
    where
        F: FnOnce(&mut SccpSourceVerifierEvidenceV1),
    {
        let mut consensus = source_consensus_proof_from_envelope(proof);
        mutate(&mut consensus.verifier_evidence);
        replace_source_consensus_proof(proof, &consensus);
    }

    fn mutate_adapter_source_domain(adapter: &mut SccpSourceAdapterProofV1, source_domain: u32) {
        match adapter {
            SccpSourceAdapterProofV1::EthereumBeaconReceipt(proof) => {
                proof.source_domain = source_domain;
            }
            SccpSourceAdapterProofV1::BscValidatorSetReceipt(proof) => {
                proof.source_domain = source_domain;
            }
            SccpSourceAdapterProofV1::SolanaFinalizedTransaction(proof) => {
                proof.source_domain = source_domain;
            }
            SccpSourceAdapterProofV1::TonMasterchainShard(proof) => {
                proof.source_domain = source_domain;
            }
            SccpSourceAdapterProofV1::TronDposReceipt(proof) => {
                proof.source_domain = source_domain;
            }
            SccpSourceAdapterProofV1::SubstrateGrandpaEvent(proof) => {
                proof.source_domain = source_domain;
            }
        }
    }

    fn zero_adapter_witness(adapter: &mut SccpSourceAdapterProofV1) {
        match adapter {
            SccpSourceAdapterProofV1::EthereumBeaconReceipt(proof) => {
                proof.receipt_trie_proof_hash = [0; 32];
            }
            SccpSourceAdapterProofV1::BscValidatorSetReceipt(proof) => {
                proof.commit_seal_hash = [0; 32];
            }
            SccpSourceAdapterProofV1::SolanaFinalizedTransaction(proof) => {
                proof.message_proof_hash = [0; 32];
            }
            SccpSourceAdapterProofV1::TonMasterchainShard(proof) => {
                proof.shard_proof_hash = [0; 32];
            }
            SccpSourceAdapterProofV1::TronDposReceipt(proof) => {
                proof.receipt_proof_hash = [0; 32];
            }
            SccpSourceAdapterProofV1::SubstrateGrandpaEvent(proof) => {
                proof.storage_proof_hash = [0; 32];
            }
        }
    }

    fn shift_adapter_witness(adapter: &mut SccpSourceAdapterProofV1) {
        match adapter {
            SccpSourceAdapterProofV1::EthereumBeaconReceipt(proof) => {
                proof.sync_committee_root[0] ^= 0x01;
            }
            SccpSourceAdapterProofV1::BscValidatorSetReceipt(proof) => {
                proof.validator_set_hash[0] ^= 0x01;
            }
            SccpSourceAdapterProofV1::SolanaFinalizedTransaction(proof) => {
                proof.bank_hash[0] ^= 0x01;
            }
            SccpSourceAdapterProofV1::TonMasterchainShard(proof) => {
                proof.shard_state_root[0] ^= 0x01;
            }
            SccpSourceAdapterProofV1::TronDposReceipt(proof) => {
                proof.witness_schedule_hash[0] ^= 0x01;
            }
            SccpSourceAdapterProofV1::SubstrateGrandpaEvent(proof) => {
                proof.authority_set_hash[0] ^= 0x01;
            }
        }
    }

    fn shift_adapter_root(adapter: &mut SccpSourceAdapterProofV1) {
        match adapter {
            SccpSourceAdapterProofV1::EthereumBeaconReceipt(proof) => {
                proof.execution_receipts_root[0] ^= 0x01;
            }
            SccpSourceAdapterProofV1::BscValidatorSetReceipt(proof) => {
                proof.receipts_root[0] ^= 0x01;
            }
            SccpSourceAdapterProofV1::SolanaFinalizedTransaction(proof) => {
                proof.transaction_status_root[0] ^= 0x01;
            }
            SccpSourceAdapterProofV1::TonMasterchainShard(proof) => {
                proof.transaction_root[0] ^= 0x01;
            }
            SccpSourceAdapterProofV1::TronDposReceipt(proof) => {
                proof.receipt_root[0] ^= 0x01;
            }
            SccpSourceAdapterProofV1::SubstrateGrandpaEvent(proof) => {
                proof.events_root[0] ^= 0x01;
            }
        }
    }

    fn wrong_adapter_variant_for(
        proof: &SccpSourceChainProofEnvelopeV1,
    ) -> SccpSourceAdapterProofV1 {
        if proof.source_domain == SCCP_DOMAIN_ETH {
            SccpSourceAdapterProofV1::SolanaFinalizedTransaction(SccpSolanaFinalizedSourceProofV1 {
                version: 1,
                source_domain: SCCP_DOMAIN_SOL,
                finalized_slot: proof.finality_height,
                blockhash: proof.finality_block_hash,
                bank_hash: [0xC1; 32],
                transaction_status_root: proof.receipt_or_message_root,
                message_proof_hash: [0xC2; 32],
            })
        } else {
            SccpSourceAdapterProofV1::EthereumBeaconReceipt(SccpEvmBeaconSourceProofV1 {
                version: 1,
                source_domain: SCCP_DOMAIN_ETH,
                beacon_slot: proof.finality_height.saturating_mul(32),
                execution_block_number: proof.finality_height,
                execution_block_hash: proof.finality_block_hash,
                execution_receipts_root: proof.receipt_or_message_root,
                beacon_finalized_root: [0xA1; 32],
                sync_committee_root: [0xA2; 32],
                receipt_trie_proof_hash: [0xA3; 32],
            })
        }
    }

    fn refresh_source_event_digest(proof: &mut SccpSourceChainProofEnvelopeV1) {
        proof.source_event_digest = sccp_source_event_digest(
            proof.source_domain,
            proof.target_domain,
            proof.message_id,
            proof.payload_hash,
        );
    }

    fn assert_source_bundle_rejects_proof_mutation<F>(valid: &NexusSccpMessageProofV1, mutate: F)
    where
        F: FnOnce(&mut SccpSourceChainProofEnvelopeV1),
    {
        let mut proof = source_chain_proof_from_bundle(valid);
        mutate(&mut proof);
        let mut bundle = valid.clone();
        bundle.finality_proof = encode_source_chain_proof(&proof);
        assert!(!verify_message_bundle_structure(&bundle), "{proof:?}");
    }

    fn assert_source_envelope_rejects_structure_mutation<F>(
        valid: &SccpSourceChainProofEnvelopeV1,
        mutate: F,
    ) where
        F: FnOnce(&mut SccpSourceChainProofEnvelopeV1),
    {
        let mut proof = valid.clone();
        mutate(&mut proof);
        assert!(
            !verify_sccp_source_chain_proof_envelope_structure(&proof),
            "{proof:?}"
        );
    }

    #[test]
    fn burn_bundle_roundtrip_structure_verifies() {
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 7,
            sora_asset_id: [1u8; 32],
            amount: 42,
            recipient: [2u8; 32],
        };
        let commitment = SccpHubCommitmentV1 {
            version: 1,
            kind: SccpHubMessageKind::Burn,
            target_domain: SCCP_DOMAIN_SORA,
            message_id: burn_message_id(&payload),
            payload_hash: payload_hash(&canonical_burn_payload_bytes(&payload)),
        };
        let commitment_root = commitment_leaf_hash(&commitment);
        let bundle = NexusSccpBurnProofV1 {
            version: 1,
            commitment_root,
            commitment,
            merkle_proof: SccpMerkleProofV1 { steps: Vec::new() },
            payload,
            finality_proof: sample_finality_proof(commitment_root),
        };
        assert!(verify_burn_bundle_structure(&bundle));
    }

    #[test]
    fn runtime_envelope_from_message_bundle_exports_scale_inputs_for_pallet() {
        let payload = SccpPayloadV1::Transfer(TransferPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SORA2,
            nonce: 9,
            asset_home_domain: SCCP_DOMAIN_SORA,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#universal".to_vec(),
            amount: 10,
            sender_codec: SCCP_CODEC_EVM_HEX,
            sender: b"0x1111111111111111111111111111111111111111".to_vec(),
            recipient_codec: SCCP_CODEC_TEXT_UTF8,
            recipient: b"bob".to_vec(),
            route_id_codec: SCCP_CODEC_TEXT_UTF8,
            route_id: b"nexus:eth:xor".to_vec(),
        });
        let bundle = sample_message_bundle(payload);
        let envelope =
            sccp_runtime_envelope_from_message_bundle(&bundle).expect("runtime envelope");
        assert_eq!(envelope.version, 1);
        assert_eq!(envelope.commitment.kind, SccpRuntimeProofKindV1::Transfer);
        assert_eq!(envelope.commitment.message_id, bundle.commitment.message_id);
        let finality = source_chain_proof_from_bundle(&bundle);
        assert_eq!(envelope.finality_proof.epoch, 0);
        assert_eq!(envelope.finality_proof.height, finality.finality_height);
        assert_eq!(
            envelope.finality_proof.block_hash,
            finality.finality_block_hash
        );
        assert_eq!(envelope.finality_proof.signature_count, 0);
        assert_eq!(
            envelope.finality_proof.validator_set_hash,
            sccp_source_chain_proof_envelope_hash(&finality)
        );
        assert_eq!(
            sccp_runtime_envelope_bytes_from_message_bundle(&bundle),
            Some(encode_sccp_runtime_proof_envelope(&envelope))
        );
    }

    #[test]
    fn runtime_envelope_from_token_control_message_exports_scale_inputs_for_pallet() {
        let payload = SccpPayloadV1::TokenPause(TokenControlPayloadV1 {
            version: 1,
            target_domain: SCCP_DOMAIN_SORA2,
            nonce: 10,
            sora_asset_id: [0x42; 32],
        });
        let bundle = sample_message_bundle(payload);
        let envelope =
            sccp_runtime_envelope_from_message_bundle(&bundle).expect("runtime envelope");
        assert_eq!(envelope.commitment.kind, SccpRuntimeProofKindV1::TokenPause);
        assert_eq!(
            sccp_runtime_envelope_bytes_from_message_bundle(&bundle),
            Some(encode_sccp_runtime_proof_envelope(&envelope))
        );
        let encoded = encode_sccp_runtime_proof_envelope(&envelope);
        let expected_len =
            1 + 32 + (1 + 1 + 4 + 32 + 32) + (1 + 1 + 4 + 8 + 32) + (1 + 8 + 8 + 32 + 32 + 32 + 2);
        assert_eq!(encoded.len(), expected_len);
    }

    #[test]
    fn runtime_envelope_rejects_corrupted_message_bundles() {
        let valid = sample_tron_transfer_bundle(90);
        assert!(sccp_runtime_envelope_from_message_bundle(&valid).is_some());

        let mut bundle = valid.clone();
        bundle.commitment.message_id[0] ^= 0x01;
        assert!(sccp_runtime_envelope_from_message_bundle(&bundle).is_none());
        assert!(sccp_runtime_envelope_bytes_from_message_bundle(&bundle).is_none());

        let mut bundle = valid.clone();
        bundle.finality_proof.truncate(4);
        assert!(sccp_runtime_envelope_from_message_bundle(&bundle).is_none());
        assert!(sccp_runtime_envelope_bytes_from_message_bundle(&bundle).is_none());

        let mut bundle = valid;
        let SccpPayloadV1::Transfer(payload) = &mut bundle.payload else {
            panic!("sample_tron_transfer_bundle must produce a transfer");
        };
        payload.route_id.push(b'!');
        assert!(sccp_runtime_envelope_from_message_bundle(&bundle).is_none());
        assert!(sccp_runtime_envelope_bytes_from_message_bundle(&bundle).is_none());
    }

    #[test]
    fn token_control_payloads_are_ordinary_sccp_messages() {
        let add = SccpPayloadV1::TokenAdd(TokenAddPayloadV1 {
            version: 1,
            target_domain: SCCP_DOMAIN_SORA,
            nonce: 1,
            sora_asset_id: [0x11; 32],
            decimals: 18,
            name: [0x22; 32],
            symbol: [0x33; 32],
        });
        let pause = SccpPayloadV1::TokenPause(TokenControlPayloadV1 {
            version: 1,
            target_domain: SCCP_DOMAIN_ETH,
            nonce: 2,
            sora_asset_id: [0x44; 32],
        });
        let resume = SccpPayloadV1::TokenResume(TokenControlPayloadV1 {
            version: 1,
            target_domain: SCCP_DOMAIN_BSC,
            nonce: 3,
            sora_asset_id: [0x55; 32],
        });

        for (payload, discriminant) in [
            (add, SccpPayloadV1::TOKEN_ADD_DISCRIMINANT),
            (pause, SccpPayloadV1::TOKEN_PAUSE_DISCRIMINANT),
            (resume, SccpPayloadV1::TOKEN_RESUME_DISCRIMINANT),
        ] {
            assert!(verify_sccp_payload_structure(&payload));
            let encoded = canonical_sccp_payload_bytes(&payload);
            assert_eq!(encoded.first(), Some(&discriminant));
            let decoded =
                decode_canonical_sccp_payload_bytes(&encoded).expect("decode token control");
            assert_eq!(decoded, payload);
            let bundle = sample_message_bundle(payload);
            assert!(verify_message_bundle_structure(&bundle));
        }
    }

    #[test]
    fn token_payload_structure_rejects_empty_required_fields() {
        let add = TokenAddPayloadV1 {
            version: 1,
            target_domain: SCCP_DOMAIN_SORA,
            nonce: 1,
            sora_asset_id: [0x11; 32],
            decimals: 18,
            name: [b'N'; 32],
            symbol: [b'X'; 32],
        };
        assert!(verify_sccp_payload_structure(&SccpPayloadV1::TokenAdd(add)));

        let mut payload = add;
        payload.target_domain = 0xFFFF_FFFE;
        assert!(!verify_sccp_payload_structure(&SccpPayloadV1::TokenAdd(
            payload
        )));

        let mut payload = add;
        payload.sora_asset_id = [0; 32];
        assert!(!verify_sccp_payload_structure(&SccpPayloadV1::TokenAdd(
            payload
        )));

        let mut payload = add;
        payload.name = [0; 32];
        assert!(!verify_sccp_payload_structure(&SccpPayloadV1::TokenAdd(
            payload
        )));

        let mut payload = add;
        payload.symbol = [0; 32];
        assert!(!verify_sccp_payload_structure(&SccpPayloadV1::TokenAdd(
            payload
        )));

        let control = TokenControlPayloadV1 {
            version: 1,
            target_domain: SCCP_DOMAIN_ETH,
            nonce: 2,
            sora_asset_id: [0x22; 32],
        };
        assert!(verify_sccp_payload_structure(&SccpPayloadV1::TokenPause(
            control
        )));

        let mut payload = control;
        payload.target_domain = 0xFFFF_FFFE;
        assert!(!verify_sccp_payload_structure(&SccpPayloadV1::TokenPause(
            payload
        )));

        let mut payload = control;
        payload.sora_asset_id = [0; 32];
        assert!(!verify_sccp_payload_structure(&SccpPayloadV1::TokenResume(
            payload
        )));
    }

    #[test]
    fn burn_bundle_rejects_mismatched_finality_root() {
        let mut bundle = sample_burn_bundle(7);
        bundle.finality_proof = sample_finality_proof([0xabu8; 32]);
        assert!(!verify_burn_bundle_structure(&bundle));
    }

    #[test]
    fn burn_bundle_rejects_commitment_message_id_tampering() {
        let mut bundle = sample_burn_bundle(8);
        bundle.commitment.message_id[0] ^= 0x01;

        assert!(!verify_burn_bundle_structure(&bundle));
    }

    #[test]
    fn burn_bundle_rejects_commitment_payload_hash_tampering() {
        let mut bundle = sample_burn_bundle(9);
        bundle.commitment.payload_hash[0] ^= 0x01;

        assert!(!verify_burn_bundle_structure(&bundle));
    }

    #[test]
    fn burn_bundle_rejects_merkle_path_tampering() {
        let mut bundle = sample_burn_bundle(10);
        bundle.merkle_proof.steps.push(SccpMerkleStepV1 {
            sibling_hash: [0x44; 32],
            sibling_is_left: true,
        });

        assert!(!verify_burn_bundle_structure(&bundle));
    }

    #[test]
    fn burn_payload_structure_rejects_bad_domain_and_version_edges() {
        let valid = sample_burn_bundle(11).payload;
        assert!(verify_burn_payload_structure(&valid));

        let mut payload = valid;
        payload.version = 2;
        assert!(!verify_burn_payload_structure(&payload));

        let mut payload = valid;
        payload.source_domain = 0xFFFF_FFFE;
        assert!(!verify_burn_payload_structure(&payload));

        let mut payload = valid;
        payload.dest_domain = payload.source_domain;
        assert!(!verify_burn_payload_structure(&payload));

        let mut payload = valid;
        payload.source_domain = SCCP_DOMAIN_SORA;
        payload.dest_domain = SCCP_DOMAIN_SORA;
        assert!(!verify_burn_payload_structure(&payload));
    }

    #[test]
    fn burn_payload_structure_rejects_zero_value_fields() {
        let valid = sample_burn_bundle(12).payload;

        let mut payload = valid;
        payload.sora_asset_id = [0; 32];
        assert!(!verify_burn_payload_structure(&payload));

        let mut payload = valid;
        payload.amount = 0;
        assert!(!verify_burn_payload_structure(&payload));

        let mut payload = valid;
        payload.recipient = [0; 32];
        assert!(!verify_burn_payload_structure(&payload));
    }

    #[test]
    fn message_bundle_roundtrip_structure_verifies() {
        let payload = SccpPayloadV1::Transfer(TransferPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_ETH,
            nonce: 11,
            asset_home_domain: SCCP_DOMAIN_SORA,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#universal".to_vec(),
            amount: 55,
            sender_codec: SCCP_CODEC_TEXT_UTF8,
            sender: b"sora:bridge".to_vec(),
            recipient_codec: SCCP_CODEC_EVM_HEX,
            recipient: b"0x1111111111111111111111111111111111111111".to_vec(),
            route_id_codec: SCCP_CODEC_TEXT_UTF8,
            route_id: b"nexus:eth:xor".to_vec(),
        });
        let bundle = sample_message_bundle(payload);
        assert!(verify_message_bundle_structure(&bundle));
    }

    #[test]
    fn non_sora_source_message_bundles_require_typed_source_chain_envelopes() {
        for (idx, source_domain) in SCCP_CORE_REMOTE_DOMAINS.into_iter().enumerate() {
            let bundle = sample_transfer_bundle(
                source_domain,
                SCCP_DOMAIN_SORA,
                300 + u64::try_from(idx).expect("index fits u64"),
            );
            assert!(verify_message_bundle_structure(&bundle), "{source_domain}");
            assert!(
                decode_nexus_bridge_finality_proof(&bundle.finality_proof).is_none(),
                "{source_domain}"
            );
            let proof = source_chain_proof_from_bundle(&bundle);
            assert!(verify_sccp_source_chain_proof_envelope_structure(&proof));
            assert_eq!(proof.version, 1);
            assert_eq!(proof.source_domain, source_domain);
            assert_eq!(proof.target_domain, SCCP_DOMAIN_SORA);
            assert_eq!(
                proof.source_chain,
                sccp_chain_key_for_domain(source_domain).unwrap()
            );
            assert_eq!(
                proof.source_proof_plan,
                sccp_source_proof_plan_for_domain(source_domain).unwrap()
            );
            assert_eq!(
                proof.finality_model,
                sccp_proof_finality_model_for_domain(source_domain).unwrap()
            );
            assert_eq!(proof.message_id, bundle.commitment.message_id);
            assert_eq!(proof.payload_hash, bundle.commitment.payload_hash);
            assert_eq!(proof.commitment_root, bundle.commitment_root);

            let public_inputs =
                sccp_message_transparent_public_inputs(&bundle).expect("public inputs");
            assert_eq!(public_inputs.finality_height, proof.finality_height);
            assert_eq!(public_inputs.finality_block_hash, proof.finality_block_hash);
        }
    }

    #[test]
    fn message_bundle_structure_rejects_wrong_finality_format_for_source_direction() {
        let mut inbound = sample_transfer_bundle(SCCP_DOMAIN_ETH, SCCP_DOMAIN_SORA, 320);
        inbound.finality_proof = sample_finality_proof(inbound.commitment_root);
        assert!(!verify_message_bundle_structure(&inbound));
        assert!(sccp_message_transparent_public_inputs(&inbound).is_none());

        let mut outbound = sample_evm_transfer_bundle(321);
        let external = sample_transfer_bundle(SCCP_DOMAIN_ETH, SCCP_DOMAIN_SORA, 322);
        outbound.finality_proof = external.finality_proof;
        assert!(!verify_message_bundle_structure(&outbound));
        assert!(sccp_message_transparent_public_inputs(&outbound).is_none());
    }

    #[test]
    fn verified_message_finality_helpers_are_direction_sensitive() {
        let outbound = sample_evm_transfer_bundle(323);
        let outbound_finality = verified_sccp_message_nexus_finality_proof(&outbound)
            .expect("SORA-origin bundle should expose Nexus finality");
        assert_eq!(outbound_finality.commitment_root, outbound.commitment_root);
        assert!(verified_sccp_message_source_chain_proof_envelope(&outbound).is_none());

        let inbound = sample_transfer_bundle(SCCP_DOMAIN_ETH, SCCP_DOMAIN_SORA, 324);
        let inbound_proof = verified_sccp_message_source_chain_proof_envelope(&inbound)
            .expect("non-SORA-origin bundle should expose source-chain envelope");
        assert_eq!(inbound_proof.source_domain, SCCP_DOMAIN_ETH);
        assert_eq!(inbound_proof.target_domain, SCCP_DOMAIN_SORA);
        assert_eq!(inbound_proof.message_id, inbound.commitment.message_id);
        assert_eq!(inbound_proof.payload_hash, inbound.commitment.payload_hash);
        assert!(verified_sccp_message_nexus_finality_proof(&inbound).is_none());

        let mut replay = inbound;
        replay.finality_proof = outbound.finality_proof;
        assert!(verified_sccp_message_source_chain_proof_envelope(&replay).is_none());
        assert!(verified_sccp_message_nexus_finality_proof(&replay).is_none());
    }

    #[test]
    fn source_chain_production_verifier_rejects_structural_envelopes_until_adapter_ready() {
        assert!(!sccp_source_adapter_ready_for_domain(SCCP_DOMAIN_SORA));

        for (idx, source_domain) in SCCP_CORE_REMOTE_DOMAINS.into_iter().enumerate() {
            let bundle = sample_transfer_bundle(
                source_domain,
                SCCP_DOMAIN_SORA,
                325 + u64::try_from(idx).expect("index fits u64"),
            );
            let proof = verified_sccp_message_source_chain_proof_envelope(&bundle)
                .expect("structural source-chain envelope should bind to bundle");
            assert!(verify_sccp_source_chain_proof_envelope_structure(&proof));
            assert!(!sccp_source_adapter_ready_for_domain(source_domain));
            assert!(!verify_sccp_source_chain_proof_envelope_production(&proof));
            assert!(
                verified_sccp_message_source_chain_proof_envelope_for_production(&bundle).is_none()
            );
        }
    }

    #[test]
    fn source_chain_production_verifier_accepts_explicit_ready_material_and_rejects_replays() {
        let mut material =
            sccp_source_verifier_material_for_domain(SCCP_DOMAIN_ETH).expect("eth material");
        assert!(material.placeholder_material);
        material.placeholder_material = false;
        assert!(
            sccp_source_verifier_material_uses_builtin_placeholder_components(&material),
            "flipping the placeholder flag must not promote built-in placeholder hashes"
        );
        assert!(
            !sccp_source_verifier_material_is_production_ready(&material),
            "built-in placeholder verifier material must stay fail-closed"
        );

        let material = sample_ready_source_verifier_material(SCCP_DOMAIN_ETH);
        assert!(sccp_source_verifier_material_is_production_ready(&material));
        let bundle = sample_transfer_bundle_with_source_material(
            SCCP_DOMAIN_ETH,
            SCCP_DOMAIN_SORA,
            701,
            Some(&material),
        );
        let proof = verified_sccp_message_source_chain_proof_envelope_for_production_with_material(
            &bundle, &material,
        )
        .expect("source proof verifies with explicit production material");

        assert!(verify_sccp_source_chain_proof_envelope_structure_with_material(&proof, &material));
        assert!(
            verify_sccp_source_chain_proof_envelope_production_with_material(&proof, &material)
        );
        assert!(
            verified_sccp_message_source_chain_proof_envelope_for_production_with_material(
                &bundle, &material,
            )
            .is_some()
        );
        assert!(
            verified_sccp_message_source_chain_proof_envelope_for_production(&bundle).is_none(),
            "default production path must stay closed while the built-in catalog is placeholder-only"
        );

        let mut placeholder = material.clone();
        placeholder.placeholder_material = true;
        assert!(
            !verify_sccp_source_chain_proof_envelope_production_with_material(&proof, &placeholder,)
        );

        let wrong_domain =
            sccp_source_verifier_material_for_domain(SCCP_DOMAIN_BSC).expect("bsc material");
        assert!(
            !verify_sccp_source_chain_proof_envelope_structure_with_material(&proof, &wrong_domain)
        );

        let mut trust_anchor_replay = material.clone();
        trust_anchor_replay.source_trust_anchor_hash[0] ^= 0x01;
        assert!(
            !verify_sccp_source_chain_proof_envelope_structure_with_material(
                &proof,
                &trust_anchor_replay,
            )
        );

        let mut verifier_replay = material;
        verifier_replay.consensus_verifier_id.push_str(":replay");
        assert!(
            !verify_sccp_source_chain_proof_envelope_structure_with_material(
                &proof,
                &verifier_replay,
            )
        );
    }

    #[test]
    fn solana_source_adapter_binds_message_proof_hash_to_inclusion_witness() {
        let bundle = sample_transfer_bundle(SCCP_DOMAIN_SOL, SCCP_DOMAIN_SORA, 702);
        let valid = source_chain_proof_from_bundle(&bundle);
        let consensus = source_consensus_proof_from_envelope(&valid);
        let SccpSourceAdapterProofV1::SolanaFinalizedTransaction(adapter) =
            &consensus.adapter_proof
        else {
            panic!("expected Solana adapter proof");
        };
        let expected = sccp_solana_message_proof_hash(
            valid.source_event_digest,
            valid.receipt_or_message_root,
            &valid.inclusion_branch,
        )
        .expect("Solana message proof hash");
        assert_eq!(adapter.message_proof_hash, expected);
        assert!(verify_sccp_source_adapter_proof_binding(
            &consensus.adapter_proof,
            &valid,
        ));

        let mut wrong_hash = consensus.adapter_proof.clone();
        let SccpSourceAdapterProofV1::SolanaFinalizedTransaction(adapter) = &mut wrong_hash else {
            panic!("expected Solana adapter proof");
        };
        adapter.message_proof_hash[0] ^= 0x01;
        assert!(!verify_sccp_source_adapter_proof_binding(
            &wrong_hash,
            &valid,
        ));

        let mut wrong_branch = valid.clone();
        wrong_branch.inclusion_branch[0][0] ^= 0x01;
        assert!(!verify_sccp_source_adapter_proof_binding(
            &consensus.adapter_proof,
            &wrong_branch,
        ));
        assert!(
            sccp_solana_message_proof_hash(
                valid.source_event_digest,
                valid.receipt_or_message_root,
                &[vec![0xAA; 31]],
            )
            .is_none()
        );
    }

    #[test]
    fn source_chain_proof_binding_rejects_cross_lane_replays() {
        let valid = sample_transfer_bundle(SCCP_DOMAIN_ETH, SCCP_DOMAIN_SORA, 330);
        assert!(verify_message_bundle_structure(&valid));

        assert_source_bundle_rejects_proof_mutation(&valid, |proof| {
            proof.source_domain = SCCP_DOMAIN_BSC;
            proof.source_chain = sccp_chain_key_for_domain(SCCP_DOMAIN_BSC)
                .unwrap()
                .to_owned();
            proof.source_proof_plan = sccp_source_proof_plan_for_domain(SCCP_DOMAIN_BSC).unwrap();
            proof.finality_model = sccp_proof_finality_model_for_domain(SCCP_DOMAIN_BSC).unwrap();
            refresh_source_event_digest(proof);
        });
        assert_source_bundle_rejects_proof_mutation(&valid, |proof| {
            proof.target_domain = SCCP_DOMAIN_TRON;
            refresh_source_event_digest(proof);
        });
        assert_source_bundle_rejects_proof_mutation(&valid, |proof| {
            proof.message_id[0] ^= 0x01;
            refresh_source_event_digest(proof);
        });
        assert_source_bundle_rejects_proof_mutation(&valid, |proof| {
            proof.payload_hash[0] ^= 0x01;
            refresh_source_event_digest(proof);
        });
        assert_source_bundle_rejects_proof_mutation(&valid, |proof| {
            proof.commitment_root[0] ^= 0x01;
        });
    }

    #[test]
    fn source_chain_proof_envelope_structure_rejects_adversarial_fields() {
        let valid_bundle = sample_transfer_bundle(SCCP_DOMAIN_ETH, SCCP_DOMAIN_SORA, 340);
        let valid = source_chain_proof_from_bundle(&valid_bundle);
        assert!(verify_sccp_source_chain_proof_envelope_structure(&valid));

        assert_source_envelope_rejects_structure_mutation(&valid, |proof| proof.version = 2);
        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            proof.source_domain = SCCP_DOMAIN_SORA;
            proof.source_chain = "sora".to_owned();
            proof.source_proof_plan = SccpSourceProofPlanV1::Unknown;
            refresh_source_event_digest(proof);
        });
        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            proof.source_domain = 0xFFFF_FFFE;
            refresh_source_event_digest(proof);
        });
        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            proof.target_domain = 0xFFFF_FFFE;
            refresh_source_event_digest(proof);
        });
        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            proof.target_domain = proof.source_domain;
            refresh_source_event_digest(proof);
        });
        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            proof.source_chain = "bsc".to_owned();
        });
        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            proof.source_proof_plan = SccpSourceProofPlanV1::BscValidatorSetReceiptProof;
        });
        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            proof.finality_model = SccpProofFinalityModelV1::BscValidatorSet;
        });
        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            proof.source_event_digest[0] ^= 0x01;
        });
        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            proof.finality_height = 0;
        });
        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            proof.message_id = [0; 32];
            refresh_source_event_digest(proof);
        });
        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            proof.payload_hash = [0; 32];
            refresh_source_event_digest(proof);
        });
        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            proof.commitment_root = [0; 32];
        });
        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            proof.finality_block_hash = [0; 32];
        });
        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            proof.finalized_header_hash = [0; 32];
        });
        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            proof.receipt_or_message_root = [0; 32];
        });
        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            proof.consensus_proof.clear();
        });
        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            proof.message_inclusion_proof.clear();
        });
        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            proof.inclusion_branch.clear();
        });
        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            proof.inclusion_branch[0].clear();
        });
    }

    #[test]
    fn source_chain_proof_material_rejects_tampered_consensus_and_inclusion_blobs() {
        let valid_bundle = sample_transfer_bundle(SCCP_DOMAIN_ETH, SCCP_DOMAIN_SORA, 341);
        let valid = source_chain_proof_from_bundle(&valid_bundle);
        let consensus = source_consensus_proof_from_envelope(&valid);
        let inclusion = source_inclusion_proof_from_envelope(&valid);
        assert_eq!(consensus.source_domain, valid.source_domain);
        assert_eq!(consensus.source_chain, valid.source_chain);
        assert_eq!(consensus.source_proof_plan, valid.source_proof_plan);
        assert_eq!(consensus.finality_model, valid.finality_model);
        assert_eq!(consensus.finalized_header_hash, valid.finalized_header_hash);
        assert_eq!(inclusion.source_event_digest, valid.source_event_digest);
        assert_eq!(
            inclusion.source_event_leaf_hash,
            sccp_source_event_leaf_hash(valid.source_event_digest)
        );
        assert_eq!(
            sccp_source_message_root_from_branch(
                inclusion.source_event_leaf_hash,
                inclusion.leaf_index,
                &valid.inclusion_branch
            ),
            Some(valid.receipt_or_message_root)
        );

        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            proof.consensus_proof = vec![0xAA, 0xBB, 0xCC];
        });
        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            proof.message_inclusion_proof = vec![0xDD, 0xEE];
        });
        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            proof.inclusion_branch[0].push(0x00);
        });
        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            proof.inclusion_branch[0][0] ^= 0x01;
        });

        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            let mut consensus = source_consensus_proof_from_envelope(proof);
            consensus.version = 2;
            replace_source_consensus_proof(proof, &consensus);
        });
        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            let mut consensus = source_consensus_proof_from_envelope(proof);
            consensus.source_domain = SCCP_DOMAIN_BSC;
            replace_source_consensus_proof(proof, &consensus);
        });
        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            let mut consensus = source_consensus_proof_from_envelope(proof);
            consensus.source_chain = "bsc".to_owned();
            replace_source_consensus_proof(proof, &consensus);
        });
        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            let mut consensus = source_consensus_proof_from_envelope(proof);
            consensus.source_proof_plan = SccpSourceProofPlanV1::BscValidatorSetReceiptProof;
            replace_source_consensus_proof(proof, &consensus);
        });
        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            let mut consensus = source_consensus_proof_from_envelope(proof);
            consensus.finality_model = SccpProofFinalityModelV1::BscValidatorSet;
            replace_source_consensus_proof(proof, &consensus);
        });
        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            let mut consensus = source_consensus_proof_from_envelope(proof);
            consensus.finality_height += 1;
            replace_source_consensus_proof(proof, &consensus);
        });
        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            let mut consensus = source_consensus_proof_from_envelope(proof);
            consensus.finality_block_hash[0] ^= 0x01;
            replace_source_consensus_proof(proof, &consensus);
        });
        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            let mut consensus = source_consensus_proof_from_envelope(proof);
            consensus.receipt_or_message_root[0] ^= 0x01;
            replace_source_consensus_proof(proof, &consensus);
        });
        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            let mut consensus = source_consensus_proof_from_envelope(proof);
            consensus.finalized_header_hash[0] ^= 0x01;
            replace_source_consensus_proof(proof, &consensus);
        });

        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            let mut inclusion = source_inclusion_proof_from_envelope(proof);
            inclusion.version = 2;
            replace_source_inclusion_proof(proof, &inclusion);
        });
        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            let mut inclusion = source_inclusion_proof_from_envelope(proof);
            inclusion.source_domain = SCCP_DOMAIN_BSC;
            replace_source_inclusion_proof(proof, &inclusion);
        });
        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            let mut inclusion = source_inclusion_proof_from_envelope(proof);
            inclusion.target_domain = SCCP_DOMAIN_TRON;
            replace_source_inclusion_proof(proof, &inclusion);
        });
        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            let mut inclusion = source_inclusion_proof_from_envelope(proof);
            inclusion.message_id[0] ^= 0x01;
            replace_source_inclusion_proof(proof, &inclusion);
        });
        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            let mut inclusion = source_inclusion_proof_from_envelope(proof);
            inclusion.payload_hash[0] ^= 0x01;
            replace_source_inclusion_proof(proof, &inclusion);
        });
        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            let mut inclusion = source_inclusion_proof_from_envelope(proof);
            inclusion.source_event_digest[0] ^= 0x01;
            replace_source_inclusion_proof(proof, &inclusion);
        });
        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            let mut inclusion = source_inclusion_proof_from_envelope(proof);
            inclusion.source_event_leaf_hash[0] ^= 0x01;
            replace_source_inclusion_proof(proof, &inclusion);
        });
        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            let mut inclusion = source_inclusion_proof_from_envelope(proof);
            inclusion.receipt_or_message_root[0] ^= 0x01;
            replace_source_inclusion_proof(proof, &inclusion);
        });
        assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
            let mut inclusion = source_inclusion_proof_from_envelope(proof);
            inclusion.leaf_index = 1;
            replace_source_inclusion_proof(proof, &inclusion);
        });
    }

    #[test]
    fn source_chain_proof_material_requires_plan_specific_adapter_proofs() {
        for (idx, source_domain) in SCCP_CORE_REMOTE_DOMAINS.into_iter().enumerate() {
            let bundle = sample_transfer_bundle(
                source_domain,
                SCCP_DOMAIN_SORA,
                360 + u64::try_from(idx).expect("index fits u64"),
            );
            let valid = source_chain_proof_from_bundle(&bundle);
            let consensus = source_consensus_proof_from_envelope(&valid);
            assert!(verify_sccp_source_chain_proof_envelope_structure(&valid));
            assert!(h256_is_nonzero(&sccp_source_adapter_proof_hash(
                &consensus.adapter_proof
            )));
            assert_eq!(
                consensus.adapter_transcript_hash,
                sccp_source_adapter_transcript_hash(
                    valid.source_domain,
                    valid.target_domain,
                    valid.source_proof_plan,
                    valid.finality_model,
                    valid.finality_height,
                    valid.finality_block_hash,
                    valid.receipt_or_message_root,
                    valid.source_event_digest,
                    &consensus.adapter_proof,
                )
            );
            let expected_verifier_evidence = build_sccp_source_verifier_evidence(
                &valid,
                &consensus.adapter_proof,
                consensus.adapter_transcript_hash,
            )
            .expect("source verifier evidence");
            assert_eq!(consensus.verifier_evidence, expected_verifier_evidence);
            let verifier_evidence_hash =
                sccp_source_verifier_evidence_hash(&consensus.verifier_evidence);
            assert!(h256_is_nonzero(&verifier_evidence_hash));
            assert_eq!(
                consensus.adapter_verification_proof.proof_family,
                SCCP_STARK_FRI_PROOF_FAMILY_V1
            );
            assert_eq!(
                consensus.adapter_verification_proof.circuit_id,
                SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID_V1
            );
            assert!(!consensus.adapter_verification_proof.proof_bytes.is_empty());
            let (env, open, raw_proof) = decode_sccp_stark_open_verify_proof(
                &consensus.adapter_verification_proof.proof_bytes,
            )
            .expect("decode adapter OpenVerify proof");
            assert_eq!(
                env.circuit_id,
                SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID_V1
            );
            assert_eq!(
                env.vk_hash,
                sccp_source_adapter_fastpq_verifier_commitment(&valid)
                    .expect("adapter verifier commitment")
            );
            assert_eq!(
                open.public_inputs,
                sccp_source_adapter_public_input_columns(
                    &valid,
                    consensus.adapter_transcript_hash,
                    verifier_evidence_hash,
                )
            );
            assert_eq!(
                raw_proof.public_io.tx_set_hash,
                consensus.adapter_transcript_hash
            );
            let replay_domain = SCCP_CORE_REMOTE_DOMAINS
                .into_iter()
                .find(|domain| *domain != source_domain)
                .expect("alternate source domain");
            let replay_bundle = sample_transfer_bundle(
                replay_domain,
                SCCP_DOMAIN_SORA,
                10_000 + u64::try_from(idx).expect("index fits u64"),
            );
            let replay_envelope = source_chain_proof_from_bundle(&replay_bundle);
            let replay_consensus = source_consensus_proof_from_envelope(&replay_envelope);

            assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
                replace_source_adapter_proof(proof, wrong_adapter_variant_for(proof));
            });
            assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
                let mut consensus = source_consensus_proof_from_envelope(proof);
                consensus.verifier_evidence = replay_consensus.verifier_evidence.clone();
                replace_source_consensus_proof(proof, &consensus);
            });
            assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
                let mut consensus = source_consensus_proof_from_envelope(proof);
                consensus.adapter_transcript_hash[0] ^= 0x01;
                replace_source_consensus_proof(proof, &consensus);
            });
            assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
                mutate_source_verifier_evidence(proof, |evidence| {
                    evidence.version = 2;
                });
            });
            assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
                mutate_source_verifier_evidence(proof, |evidence| {
                    evidence.source_domain = SCCP_DOMAIN_SORA;
                });
            });
            assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
                mutate_source_verifier_evidence(proof, |evidence| {
                    evidence.source_chain.push_str("-replay");
                });
            });
            assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
                mutate_source_verifier_evidence(proof, |evidence| {
                    evidence.adapter_proof_hash[0] ^= 0x01;
                });
            });
            assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
                mutate_source_verifier_evidence(proof, |evidence| {
                    evidence.adapter_transcript_hash[0] ^= 0x01;
                });
            });
            assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
                mutate_source_verifier_evidence(proof, |evidence| {
                    evidence.adapter_circuit_id.push_str("-wrong");
                });
            });
            assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
                mutate_source_verifier_evidence(proof, |evidence| {
                    evidence.source_trust_anchor_id.clear();
                });
            });
            assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
                mutate_source_verifier_evidence(proof, |evidence| {
                    evidence.source_trust_anchor_hash = [0u8; 32];
                });
            });
            assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
                mutate_source_verifier_evidence(proof, |evidence| {
                    evidence.consensus_verifier_id.push_str("-replay");
                });
            });
            assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
                mutate_source_verifier_evidence(proof, |evidence| {
                    evidence.consensus_verifier_hash[31] ^= 0x01;
                });
            });
            assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
                mutate_source_verifier_evidence(proof, |evidence| {
                    evidence.message_inclusion_verifier_id.push_str("-replay");
                });
            });
            assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
                mutate_source_verifier_evidence(proof, |evidence| {
                    evidence.message_inclusion_verifier_hash = [0u8; 32];
                });
            });
            assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
                mutate_source_verifier_evidence(proof, |evidence| {
                    evidence.finality_policy_id.push_str("-wrong");
                });
            });
            assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
                mutate_source_verifier_evidence(proof, |evidence| {
                    evidence.finality_policy_hash[0] ^= 0x80;
                });
            });
            assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
                let mut consensus = source_consensus_proof_from_envelope(proof);
                mutate_adapter_source_domain(&mut consensus.adapter_proof, SCCP_DOMAIN_SORA);
                replace_source_consensus_proof(proof, &consensus);
            });
            assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
                let mut consensus = source_consensus_proof_from_envelope(proof);
                shift_adapter_witness(&mut consensus.adapter_proof);
                replace_source_consensus_proof(proof, &consensus);
            });
            assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
                let mut consensus = source_consensus_proof_from_envelope(proof);
                shift_adapter_root(&mut consensus.adapter_proof);
                replace_source_consensus_proof(proof, &consensus);
            });
            assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
                let mut consensus = source_consensus_proof_from_envelope(proof);
                zero_adapter_witness(&mut consensus.adapter_proof);
                replace_source_consensus_proof(proof, &consensus);
            });
            assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
                mutate_source_adapter_verification_proof(proof, |adapter_proof| {
                    adapter_proof.version = 2;
                });
            });
            assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
                mutate_source_adapter_verification_proof(proof, |adapter_proof| {
                    adapter_proof.proof_family = "legacy-stark".to_owned();
                });
            });
            assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
                mutate_source_adapter_verification_proof(proof, |adapter_proof| {
                    adapter_proof.circuit_id = "wrong-source-adapter-circuit".to_owned();
                });
            });
            assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
                mutate_source_adapter_verification_proof(proof, |adapter_proof| {
                    adapter_proof.proof_bytes.clear();
                });
            });
            assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
                mutate_source_adapter_verification_proof(proof, |adapter_proof| {
                    let (mut env, _, _) =
                        decode_sccp_stark_open_verify_proof(&adapter_proof.proof_bytes)
                            .expect("decode adapter OpenVerify proof");
                    env.circuit_id.push_str("-replay");
                    adapter_proof.proof_bytes =
                        to_bytes(&env).expect("encode tampered OpenVerify envelope");
                });
            });
            assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
                mutate_source_adapter_verification_proof(proof, |adapter_proof| {
                    let (mut env, mut open, _) =
                        decode_sccp_stark_open_verify_proof(&adapter_proof.proof_bytes)
                            .expect("decode adapter OpenVerify proof");
                    open.public_inputs[0][0][0] ^= 0x01;
                    env.proof_bytes = to_bytes(&open).expect("encode tampered Stark open proof");
                    adapter_proof.proof_bytes =
                        to_bytes(&env).expect("encode tampered OpenVerify envelope");
                });
            });
            assert_source_envelope_rejects_structure_mutation(&valid, |proof| {
                mutate_source_adapter_verification_proof(proof, |adapter_proof| {
                    let (mut env, mut open, mut raw_proof) =
                        decode_sccp_stark_open_verify_proof(&adapter_proof.proof_bytes)
                            .expect("decode adapter OpenVerify proof");
                    raw_proof.public_io.tx_set_hash[0] ^= 0x01;
                    open.envelope_bytes =
                        to_bytes(&raw_proof).expect("encode tampered FastPQ proof");
                    env.proof_bytes = to_bytes(&open).expect("encode tampered Stark open proof");
                    adapter_proof.proof_bytes =
                        to_bytes(&env).expect("encode tampered OpenVerify envelope");
                });
            });
        }
    }

    #[test]
    fn message_bundle_structure_rejects_commitment_metadata_replays() {
        let valid = sample_evm_transfer_bundle(65);
        assert!(verify_message_bundle_structure(&valid));

        let mut bundle = valid.clone();
        bundle.version = 2;
        assert!(!verify_message_bundle_structure(&bundle));

        let mut bundle = valid.clone();
        bundle.commitment.version = 2;
        assert!(!verify_message_bundle_structure(&bundle));

        let mut bundle = valid.clone();
        bundle.commitment.kind = SccpHubMessageKind::Burn;
        assert!(!verify_message_bundle_structure(&bundle));

        let mut bundle = valid.clone();
        bundle.commitment.target_domain = SCCP_DOMAIN_SOL;
        assert!(!verify_message_bundle_structure(&bundle));

        let mut bundle = valid;
        bundle.commitment_root[0] ^= 0x01;
        assert!(!verify_message_bundle_structure(&bundle));
    }

    #[test]
    fn message_transparent_proof_builder_refuses_disabled_counterparty_lanes() {
        let bundle = sample_message_bundle(SccpPayloadV1::Transfer(TransferPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_TON,
            nonce: 18,
            asset_home_domain: SCCP_DOMAIN_SORA,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#universal".to_vec(),
            amount: 91,
            sender_codec: SCCP_CODEC_TEXT_UTF8,
            sender: b"sora:bridge".to_vec(),
            recipient_codec: SCCP_CODEC_TON_RAW,
            recipient: b"0:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                .to_vec(),
            route_id_codec: SCCP_CODEC_TEXT_UTF8,
            route_id: b"nexus:ton:xor".to_vec(),
        }));

        assert!(build_nexus_sccp_message_transparent_proof(&bundle).is_none());
        assert!(build_sccp_counterparty_proof_job_from_bundle(&bundle).is_none());
    }

    #[test]
    fn message_transparent_proof_builder_with_signer_refuses_disabled_evm_lane() {
        let bundle = sample_message_bundle(SccpPayloadV1::Transfer(TransferPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_ETH,
            nonce: 27,
            asset_home_domain: SCCP_DOMAIN_SORA,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#universal".to_vec(),
            amount: 5,
            sender_codec: SCCP_CODEC_TEXT_UTF8,
            sender: b"sora:bridge".to_vec(),
            recipient_codec: SCCP_CODEC_EVM_HEX,
            recipient: b"0x2222222222222222222222222222222222222222".to_vec(),
            route_id_codec: SCCP_CODEC_TEXT_UTF8,
            route_id: b"nexus:eth:xor".to_vec(),
        }));
        let signer = sample_secp256k1_signer();
        assert!(build_nexus_sccp_message_transparent_proof_with_signer(&bundle, &signer).is_none());
    }

    #[test]
    fn transparent_fastpq_proof_bytes_use_open_verify_envelope_binding() {
        let bundle = sample_message_bundle(SccpPayloadV1::Transfer(TransferPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_TRON,
            nonce: 28,
            asset_home_domain: SCCP_DOMAIN_SORA,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#universal".to_vec(),
            amount: 123,
            sender_codec: SCCP_CODEC_TEXT_UTF8,
            sender: b"sora:bridge".to_vec(),
            recipient_codec: SCCP_CODEC_TRON_BASE58CHECK,
            recipient: b"TJRabPrwbZy45sbavfcjinPJC18kjpRTv8".to_vec(),
            route_id_codec: SCCP_CODEC_TEXT_UTF8,
            route_id: b"nexus:tron:xor".to_vec(),
        }));
        let manifest = sccp_proof_manifest_for_domain(SCCP_DOMAIN_TRON).expect("tron manifest");
        let public_inputs =
            sccp_message_transparent_public_inputs(&bundle).expect("message public inputs");
        let proof_bytes =
            build_sccp_message_transparent_fastpq_proof_bytes(&bundle, &manifest).expect("proof");
        let (env, open, _) = decode_sccp_message_transparent_open_verify_proof(&proof_bytes)
            .expect("open verify envelope");

        assert_eq!(env.backend, iroha_data_model::zk::BackendTag::Stark);
        assert_eq!(env.circuit_id, SCCP_TRANSPARENT_OPEN_VERIFY_CIRCUIT_ID_V1);
        assert_eq!(
            env.vk_hash,
            sccp_message_transparent_fastpq_verifier_commitment(&manifest)
                .expect("verifier commitment")
        );
        assert_eq!(
            env.public_inputs,
            sccp_message_transparent_open_verify_schema_descriptor(&manifest)
        );
        assert!(env.aux.is_empty());
        assert_eq!(
            open.public_inputs,
            sccp_message_transparent_public_input_columns(&public_inputs)
        );
        assert!(verify_sccp_message_transparent_inner_proof_bytes(
            &proof_bytes,
            &bundle,
            &manifest,
            &public_inputs,
        ));
    }

    #[test]
    fn transparent_fastpq_verifier_rejects_tampered_open_verify_metadata() {
        let bundle = sample_message_bundle(SccpPayloadV1::Transfer(TransferPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_TRON,
            nonce: 29,
            asset_home_domain: SCCP_DOMAIN_SORA,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#universal".to_vec(),
            amount: 321,
            sender_codec: SCCP_CODEC_TEXT_UTF8,
            sender: b"sora:bridge".to_vec(),
            recipient_codec: SCCP_CODEC_TRON_BASE58CHECK,
            recipient: b"TJRabPrwbZy45sbavfcjinPJC18kjpRTv8".to_vec(),
            route_id_codec: SCCP_CODEC_TEXT_UTF8,
            route_id: b"nexus:tron:xor".to_vec(),
        }));
        let manifest = sccp_proof_manifest_for_domain(SCCP_DOMAIN_TRON).expect("tron manifest");
        let public_inputs =
            sccp_message_transparent_public_inputs(&bundle).expect("message public inputs");
        let proof_bytes =
            build_sccp_message_transparent_fastpq_proof_bytes(&bundle, &manifest).expect("proof");
        let mut env: iroha_data_model::zk::OpenVerifyEnvelope =
            norito::decode_from_bytes(&proof_bytes).expect("decode envelope");
        env.vk_hash[0] ^= 0xFF;
        let tampered = to_bytes(&env).expect("encode tampered envelope");

        assert!(!verify_sccp_message_transparent_inner_proof_bytes(
            &tampered,
            &bundle,
            &manifest,
            &public_inputs,
        ));
    }

    #[test]
    fn transparent_fastpq_verifier_rejects_wrong_open_verify_backend_tag() {
        let bundle = sample_tron_transfer_bundle(38);
        let manifest = sccp_proof_manifest_for_domain(SCCP_DOMAIN_TRON).expect("tron manifest");
        let public_inputs =
            sccp_message_transparent_public_inputs(&bundle).expect("message public inputs");
        let proof_bytes =
            build_sccp_message_transparent_fastpq_proof_bytes(&bundle, &manifest).expect("proof");
        let mut env: OpenVerifyEnvelope =
            norito::decode_from_bytes(&proof_bytes).expect("decode envelope");
        env.backend = BackendTag::Groth16;
        let tampered = to_bytes(&env).expect("encode tampered envelope");

        assert!(!verify_sccp_message_transparent_inner_proof_bytes(
            &tampered,
            &bundle,
            &manifest,
            &public_inputs,
        ));
    }

    #[test]
    fn transparent_fastpq_verifier_rejects_wrong_circuit_id() {
        let bundle = sample_tron_transfer_bundle(39);
        let manifest = sccp_proof_manifest_for_domain(SCCP_DOMAIN_TRON).expect("tron manifest");
        let public_inputs =
            sccp_message_transparent_public_inputs(&bundle).expect("message public inputs");
        let proof_bytes =
            build_sccp_message_transparent_fastpq_proof_bytes(&bundle, &manifest).expect("proof");
        let mut env: OpenVerifyEnvelope =
            norito::decode_from_bytes(&proof_bytes).expect("decode envelope");
        env.circuit_id.push_str("-fork");
        let tampered = to_bytes(&env).expect("encode tampered envelope");

        assert!(!verify_sccp_message_transparent_inner_proof_bytes(
            &tampered,
            &bundle,
            &manifest,
            &public_inputs,
        ));
    }

    #[test]
    fn transparent_fastpq_verifier_rejects_schema_descriptor_tampering() {
        let bundle = sample_tron_transfer_bundle(40);
        let manifest = sccp_proof_manifest_for_domain(SCCP_DOMAIN_TRON).expect("tron manifest");
        let public_inputs =
            sccp_message_transparent_public_inputs(&bundle).expect("message public inputs");
        let proof_bytes =
            build_sccp_message_transparent_fastpq_proof_bytes(&bundle, &manifest).expect("proof");
        let mut env: OpenVerifyEnvelope =
            norito::decode_from_bytes(&proof_bytes).expect("decode envelope");
        env.public_inputs.push(0x99);
        let tampered = to_bytes(&env).expect("encode tampered envelope");

        assert!(!verify_sccp_message_transparent_inner_proof_bytes(
            &tampered,
            &bundle,
            &manifest,
            &public_inputs,
        ));
    }

    #[test]
    fn transparent_fastpq_verifier_rejects_malformed_open_proof_payload() {
        let bundle = sample_tron_transfer_bundle(41);
        let manifest = sccp_proof_manifest_for_domain(SCCP_DOMAIN_TRON).expect("tron manifest");
        let public_inputs =
            sccp_message_transparent_public_inputs(&bundle).expect("message public inputs");
        let proof_bytes =
            build_sccp_message_transparent_fastpq_proof_bytes(&bundle, &manifest).expect("proof");
        let mut env: OpenVerifyEnvelope =
            norito::decode_from_bytes(&proof_bytes).expect("decode envelope");
        env.proof_bytes = vec![0x01, 0x02, 0x03];
        let tampered = to_bytes(&env).expect("encode tampered envelope");

        assert!(!verify_sccp_message_transparent_inner_proof_bytes(
            &tampered,
            &bundle,
            &manifest,
            &public_inputs,
        ));
    }

    #[test]
    fn transparent_fastpq_verifier_rejects_non_v1_open_proof() {
        let bundle = sample_tron_transfer_bundle(42);
        let manifest = sccp_proof_manifest_for_domain(SCCP_DOMAIN_TRON).expect("tron manifest");
        let public_inputs =
            sccp_message_transparent_public_inputs(&bundle).expect("message public inputs");
        let proof_bytes =
            build_sccp_message_transparent_fastpq_proof_bytes(&bundle, &manifest).expect("proof");
        let mut env: OpenVerifyEnvelope =
            norito::decode_from_bytes(&proof_bytes).expect("decode envelope");
        let mut open: StarkFriOpenProofV1 =
            norito::decode_from_bytes(&env.proof_bytes).expect("decode open proof");
        open.version = 2;
        env.proof_bytes = to_bytes(&open).expect("encode tampered open proof");
        let tampered = to_bytes(&env).expect("encode tampered envelope");

        assert!(!verify_sccp_message_transparent_inner_proof_bytes(
            &tampered,
            &bundle,
            &manifest,
            &public_inputs,
        ));
    }

    #[test]
    fn transparent_fastpq_verifier_rejects_auxiliary_envelope_data() {
        let bundle = sample_tron_transfer_bundle(31);
        let manifest = sccp_proof_manifest_for_domain(SCCP_DOMAIN_TRON).expect("tron manifest");
        let public_inputs =
            sccp_message_transparent_public_inputs(&bundle).expect("message public inputs");
        let proof_bytes =
            build_sccp_message_transparent_fastpq_proof_bytes(&bundle, &manifest).expect("proof");
        let mut env: OpenVerifyEnvelope =
            norito::decode_from_bytes(&proof_bytes).expect("decode envelope");
        env.aux.push(0xAA);
        let tampered = to_bytes(&env).expect("encode tampered envelope");

        assert!(!verify_sccp_message_transparent_inner_proof_bytes(
            &tampered,
            &bundle,
            &manifest,
            &public_inputs,
        ));
    }

    #[test]
    fn transparent_fastpq_verifier_rejects_tampered_public_input_columns() {
        let bundle = sample_tron_transfer_bundle(32);
        let manifest = sccp_proof_manifest_for_domain(SCCP_DOMAIN_TRON).expect("tron manifest");
        let public_inputs =
            sccp_message_transparent_public_inputs(&bundle).expect("message public inputs");
        let proof_bytes =
            build_sccp_message_transparent_fastpq_proof_bytes(&bundle, &manifest).expect("proof");
        let mut env: OpenVerifyEnvelope =
            norito::decode_from_bytes(&proof_bytes).expect("decode envelope");
        let mut open: StarkFriOpenProofV1 =
            norito::decode_from_bytes(&env.proof_bytes).expect("decode open proof");
        open.public_inputs[0][0][0] ^= 0x01;
        env.proof_bytes = to_bytes(&open).expect("encode tampered open proof");
        let tampered = to_bytes(&env).expect("encode tampered envelope");

        assert!(!verify_sccp_message_transparent_inner_proof_bytes(
            &tampered,
            &bundle,
            &manifest,
            &public_inputs,
        ));
    }

    #[test]
    fn transparent_fastpq_verifier_rejects_tampered_backend_proof_bytes() {
        let bundle = sample_tron_transfer_bundle(33);
        let manifest = sccp_proof_manifest_for_domain(SCCP_DOMAIN_TRON).expect("tron manifest");
        let public_inputs =
            sccp_message_transparent_public_inputs(&bundle).expect("message public inputs");
        let proof_bytes =
            build_sccp_message_transparent_fastpq_proof_bytes(&bundle, &manifest).expect("proof");
        let mut env: OpenVerifyEnvelope =
            norito::decode_from_bytes(&proof_bytes).expect("decode envelope");
        let mut open: StarkFriOpenProofV1 =
            norito::decode_from_bytes(&env.proof_bytes).expect("decode open proof");
        let byte = open
            .envelope_bytes
            .last_mut()
            .expect("backend proof bytes must be present");
        *byte ^= 0x01;
        env.proof_bytes = to_bytes(&open).expect("encode tampered open proof");
        let tampered = to_bytes(&env).expect("encode tampered envelope");

        assert!(!verify_sccp_message_transparent_inner_proof_bytes(
            &tampered,
            &bundle,
            &manifest,
            &public_inputs,
        ));
    }

    #[test]
    fn transparent_fastpq_verifier_rejects_cross_bundle_replay() {
        let source_bundle = sample_tron_transfer_bundle(34);
        let replay_target_bundle = sample_tron_transfer_bundle(35);
        let manifest = sccp_proof_manifest_for_domain(SCCP_DOMAIN_TRON).expect("tron manifest");
        let replay_target_public_inputs =
            sccp_message_transparent_public_inputs(&replay_target_bundle)
                .expect("message public inputs");
        let proof_bytes =
            build_sccp_message_transparent_fastpq_proof_bytes(&source_bundle, &manifest)
                .expect("proof");

        assert!(!verify_sccp_message_transparent_inner_proof_bytes(
            &proof_bytes,
            &replay_target_bundle,
            &manifest,
            &replay_target_public_inputs,
        ));
    }

    #[test]
    fn transparent_fastpq_verifier_rejects_legacy_raw_proof_payload() {
        let bundle = sample_message_bundle(SccpPayloadV1::Transfer(TransferPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_SOL,
            nonce: 30,
            asset_home_domain: SCCP_DOMAIN_SORA,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#universal".to_vec(),
            amount: 11,
            sender_codec: SCCP_CODEC_TEXT_UTF8,
            sender: b"sora:bridge".to_vec(),
            recipient_codec: SCCP_CODEC_SOLANA_BASE58,
            recipient: b"11111111111111111111111111111111".to_vec(),
            route_id_codec: SCCP_CODEC_TEXT_UTF8,
            route_id: b"nexus:sol:xor".to_vec(),
        }));
        let manifest = sccp_proof_manifest_for_domain(SCCP_DOMAIN_SOL).expect("sol manifest");
        let public_inputs =
            sccp_message_transparent_public_inputs(&bundle).expect("message public inputs");
        let batch = build_sccp_message_transparent_fastpq_batch(&bundle, &manifest).expect("batch");
        let raw_proof_bytes =
            build_sccp_message_transparent_fastpq_raw_proof_bytes(&batch).expect("raw proof");

        assert!(!verify_sccp_message_transparent_inner_proof_bytes(
            &raw_proof_bytes,
            &bundle,
            &manifest,
            &public_inputs,
        ));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn transparent_fastpq_open_verify_summary_reports_binding_metadata() {
        let bundle = sample_message_bundle(SccpPayloadV1::Transfer(TransferPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_TON,
            nonce: 26,
            asset_home_domain: SCCP_DOMAIN_SORA,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#open-verify-summary".to_vec(),
            amount: 123,
            sender_codec: SCCP_CODEC_TEXT_UTF8,
            sender: b"nexus:summary".to_vec(),
            recipient_codec: SCCP_CODEC_TON_RAW,
            recipient: b"0:abcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd"
                .to_vec(),
            route_id_codec: SCCP_CODEC_TEXT_UTF8,
            route_id: b"nexus:ton:summary".to_vec(),
        }));
        let manifest = sccp_proof_manifest_for_domain(SCCP_DOMAIN_TON).expect("ton manifest");
        let public_inputs =
            sccp_message_transparent_public_inputs(&bundle).expect("message public inputs");
        let proof_bytes =
            build_sccp_message_transparent_fastpq_proof_bytes(&bundle, &manifest).expect("proof");
        let summary = summarize_sccp_message_transparent_open_verify_proof(&proof_bytes)
            .expect("proof summary");

        assert_eq!(summary.version, 1);
        assert_eq!(summary.backend, "stark");
        assert_eq!(
            summary.circuit_id,
            SCCP_TRANSPARENT_OPEN_VERIFY_CIRCUIT_ID_V1
        );
        assert_eq!(
            summary.vk_hash,
            sccp_message_transparent_fastpq_verifier_commitment(&manifest)
                .expect("verifier commitment")
        );
        assert_eq!(
            summary.public_inputs_schema_hash,
            prefixed_blake2b(
                SCCP_TRANSPARENT_OPEN_VERIFY_SCHEMA_HASH_PREFIX_V1,
                &sccp_message_transparent_open_verify_schema_descriptor(&manifest),
            )
        );
        assert_eq!(
            summary.public_inputs_schema_len_bytes as usize,
            sccp_message_transparent_open_verify_schema_descriptor(&manifest).len()
        );
        assert_eq!(summary.public_input_column_count, 6);
        assert_eq!(summary.public_input_word_count, 6);
        assert_eq!(
            summary.backend_proof_len_bytes as usize,
            build_sccp_message_transparent_fastpq_raw_proof_bytes(
                &build_sccp_message_transparent_fastpq_batch(&bundle, &manifest)
                    .expect("fastpq batch"),
            )
            .expect("raw proof")
            .len()
        );
        assert_eq!(summary.aux_len_bytes, 0);
        let inner =
            build_sccp_message_transparent_inner_proof(&bundle, &manifest).expect("inner proof");
        let ton_payload = build_sccp_ton_internal_message_submission_payload(
            &manifest,
            &proof_bytes,
            &public_inputs,
            &bundle,
            inner.statement_hash,
            &manifest.destination_binding,
        )
        .expect("ton payload");
        assert_eq!(
            summarize_sccp_message_transparent_open_verify_proof_from_artifact(
                &NexusSccpMessageTransparentProofV1 {
                    version: 1,
                    local_domain: manifest.local_domain,
                    counterparty_domain: manifest.counterparty_domain,
                    security_model: manifest.security_model,
                    anchor_governance: manifest.anchor_governance,
                    destination_binding: manifest.destination_binding.clone(),
                    proof_family: manifest.proof_family.clone(),
                    verifier_backend: manifest.verifier_backend.clone(),
                    message_backend: manifest.message_backend.clone(),
                    registry_backend: manifest.registry_backend.clone(),
                    manifest_seed: manifest.manifest_seed.clone(),
                    finality_model: manifest.finality_model,
                    verifier_target: manifest.verifier_target,
                    public_inputs: public_inputs.clone(),
                    proof_bytes: proof_bytes.clone(),
                    submission_package: SccpCounterpartySubmissionPackageV1 {
                        version: 1,
                        proof_family: manifest.proof_family.clone(),
                        verifier_backend: manifest.verifier_backend.clone(),
                        envelope_encoding: "ton_message_body_boc_v1".to_owned(),
                        submission_kind: manifest.submission_template.submission_kind.clone(),
                        verifier_entrypoint: manifest
                            .submission_template
                            .verifier_entrypoint
                            .clone(),
                        platform_payload: SccpPlatformSubmissionPayloadV1::TonInternalMessage(
                            ton_payload,
                        ),
                        arguments: Vec::new(),
                        envelope_bytes: vec![0xCC],
                    },
                    bundle: bundle.clone(),
                }
            ),
            Some(summary.clone())
        );
        assert_eq!(
            build_sccp_message_transparent_open_verify_summary_from_bundle(&bundle),
            Some(summary)
        );
    }

    #[test]
    fn transparent_fastpq_open_verify_summary_allows_metadata_only_envelopes() {
        let open = StarkFriOpenProofV1 {
            version: 1,
            public_inputs: vec![vec![[0x55; 32]]],
            envelope_bytes: vec![0xAA, 0xBB, 0xCC],
        };
        let open_proof_bytes = norito::to_bytes(&open).expect("encode open proof");
        let proof_bytes = norito::to_bytes(&OpenVerifyEnvelope {
            backend: BackendTag::Stark,
            circuit_id: SCCP_TRANSPARENT_OPEN_VERIFY_CIRCUIT_ID_V1.to_owned(),
            vk_hash: [0x66; 32],
            public_inputs: vec![0x77, 0x88, 0x99],
            proof_bytes: open_proof_bytes.clone(),
            aux: vec![0xDE, 0xAD],
        })
        .expect("encode envelope");

        let summary = summarize_sccp_message_transparent_open_verify_proof(&proof_bytes)
            .expect("proof summary");

        assert_eq!(summary.backend, "stark");
        assert_eq!(
            summary.circuit_id,
            SCCP_TRANSPARENT_OPEN_VERIFY_CIRCUIT_ID_V1
        );
        assert_eq!(summary.vk_hash, [0x66; 32]);
        assert_eq!(
            summary.public_inputs_schema_hash,
            prefixed_blake2b(
                SCCP_TRANSPARENT_OPEN_VERIFY_SCHEMA_HASH_PREFIX_V1,
                &[0x77, 0x88, 0x99]
            )
        );
        assert_eq!(summary.public_inputs_schema_len_bytes, 3);
        assert_eq!(summary.public_input_column_count, 1);
        assert_eq!(summary.public_input_word_count, 1);
        assert_eq!(
            summary.open_proof_len_bytes as usize,
            open_proof_bytes.len()
        );
        assert_eq!(summary.backend_proof_len_bytes, 3);
        assert_eq!(summary.aux_len_bytes, 2);
    }

    #[test]
    fn transparent_fastpq_open_verify_summary_rejects_bad_envelope_shapes() {
        let open = StarkFriOpenProofV1 {
            version: 1,
            public_inputs: vec![vec![[0x55; 32]]],
            envelope_bytes: vec![0xAA, 0xBB, 0xCC],
        };
        let mut env = OpenVerifyEnvelope {
            backend: BackendTag::Stark,
            circuit_id: SCCP_TRANSPARENT_OPEN_VERIFY_CIRCUIT_ID_V1.to_owned(),
            vk_hash: [0x66; 32],
            public_inputs: vec![0x77, 0x88, 0x99],
            proof_bytes: norito::to_bytes(&open).expect("encode open proof"),
            aux: Vec::new(),
        };

        let mut wrong_backend = env.clone();
        wrong_backend.backend = BackendTag::Groth16;
        assert!(
            summarize_sccp_message_transparent_open_verify_proof(
                &norito::to_bytes(&wrong_backend).expect("encode wrong backend envelope"),
            )
            .is_none()
        );

        let mut malformed_open = env.clone();
        malformed_open.proof_bytes = vec![0x01, 0x02];
        assert!(
            summarize_sccp_message_transparent_open_verify_proof(
                &norito::to_bytes(&malformed_open).expect("encode malformed open envelope"),
            )
            .is_none()
        );

        let mut non_v1_open = open;
        non_v1_open.version = 2;
        env.proof_bytes = norito::to_bytes(&non_v1_open).expect("encode non-v1 open proof");
        assert!(
            summarize_sccp_message_transparent_open_verify_proof(
                &norito::to_bytes(&env).expect("encode non-v1 open envelope"),
            )
            .is_none()
        );
    }

    #[test]
    fn proof_manifests_mark_current_counterparty_lanes_non_production() {
        for domain in SCCP_CORE_REMOTE_DOMAINS {
            let manifest = sccp_proof_manifest_for_domain(domain).expect("manifest");
            assert!(!manifest.production_ready);
            assert_eq!(
                manifest.disabled_reason.as_deref(),
                sccp_lane_disabled_reason_for_domain(domain)
            );
            assert_eq!(manifest.destination_rollout.domain, domain);
            assert_eq!(
                manifest.destination_rollout.chain,
                sccp_chain_key_for_domain(domain).expect("chain key")
            );
            assert!(!manifest.destination_rollout.immutable_verifier_ready);
            assert!(!manifest.destination_rollout.anchors_ready);
            assert!(!manifest.destination_rollout.blockers.is_empty());
            assert!(!sccp_destination_rollout_is_production_ready(
                domain,
                &manifest.destination_rollout
            ));
        }
    }

    #[test]
    fn production_policy_requires_permissionless_proofs_allowlisted_routes_and_all_lanes() {
        let policy = sccp_production_policy_v1();
        assert_eq!(policy.launch_mode, SccpLaunchModeV1::AllLanesAtOnce);
        assert_eq!(
            policy.proof_submitter_policy,
            SccpProofSubmitterPolicyV1::Permissionless
        );
        assert_eq!(
            policy.route_activation_policy,
            SccpRouteActivationPolicyV1::GovernanceAllowlist
        );
        assert!(!policy.per_message_human_approval_required);
        assert!(!sccp_all_lanes_launch_ready_v1());
    }

    #[test]
    fn production_readiness_lists_source_destination_and_route_blockers() {
        let readiness =
            sccp_lane_production_readiness_for_domain(SCCP_DOMAIN_ETH).expect("eth readiness");

        assert_eq!(
            readiness.source_proof_plan,
            SccpSourceProofPlanV1::EthereumBeaconReceiptProof
        );
        assert_eq!(
            readiness.destination_verifier_plan,
            SccpDestinationVerifierPlanV1::EvmGroth16Bn254Adapter
        );
        assert_eq!(
            readiness.verifier_backend.key,
            SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1
        );
        assert!(readiness.permissionless_submission);
        assert!(!readiness.source_adapter_ready);
        assert!(
            readiness
                .source_adapter_engine
                .adapter_statement_binding_ready
        );
        assert!(
            readiness
                .source_adapter_engine
                .source_verifier_material
                .placeholder_material
        );
        assert!(!sccp_source_verifier_material_is_production_ready(
            &readiness.source_adapter_engine.source_verifier_material
        ));
        assert!(readiness.source_adapter_engine.adapter_open_verify_ready);
        assert!(readiness.source_adapter_engine.finality_model_ready);
        assert!(
            !readiness
                .source_adapter_engine
                .external_consensus_verifier_ready
        );
        assert!(
            !readiness
                .source_adapter_engine
                .external_message_inclusion_verifier_ready
        );
        assert!(!readiness.source_adapter_engine.source_trust_anchor_ready);
        assert!(!readiness.source_adapter_engine.production_ready);
        assert!(!readiness.immutable_verifier_ready);
        assert!(!readiness.anchors_ready);
        assert!(!readiness.routes_allowlisted);
        assert!(!readiness.production_ready);
        assert!(
            readiness
                .blockers
                .iter()
                .any(|blocker| blocker.contains("destination verifier rollout material"))
        );
        assert!(
            readiness
                .blockers
                .iter()
                .any(|blocker| blocker.contains("Ethereum beacon finality"))
        );
        assert!(
            readiness
                .blockers
                .iter()
                .any(|blocker| blocker.contains("all-lanes-at-once"))
        );
    }

    #[test]
    fn source_adapter_engine_readiness_requires_real_consensus_inclusion_and_anchors() {
        for domain in SCCP_CORE_REMOTE_DOMAINS {
            let readiness = sccp_source_adapter_engine_readiness_for_domain(domain)
                .expect("source adapter engine readiness");
            assert_eq!(readiness.domain, domain);
            assert_eq!(
                readiness.chain,
                sccp_chain_key_for_domain(domain).expect("chain key")
            );
            assert_eq!(
                readiness.source_proof_plan,
                sccp_source_proof_plan_for_domain(domain).expect("source proof plan")
            );
            assert_eq!(
                readiness.finality_model,
                sccp_proof_finality_model_for_domain(domain).expect("finality model")
            );
            assert_eq!(
                readiness.adapter_proof_family,
                SCCP_STARK_FRI_PROOF_FAMILY_V1
            );
            assert_eq!(
                readiness.adapter_circuit_id,
                SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID_V1
            );
            assert_eq!(readiness.source_verifier_material.source_domain, domain);
            assert_eq!(
                readiness.source_verifier_material.source_chain,
                readiness.chain
            );
            assert_eq!(
                readiness.source_verifier_material.source_proof_plan,
                readiness.source_proof_plan
            );
            assert_eq!(
                readiness.source_verifier_material.finality_model,
                readiness.finality_model
            );
            assert!(readiness.source_verifier_material.placeholder_material);
            assert!(h256_is_nonzero(
                &readiness.source_verifier_material.source_trust_anchor_hash
            ));
            assert!(h256_is_nonzero(
                &readiness.source_verifier_material.consensus_verifier_hash
            ));
            assert!(h256_is_nonzero(
                &readiness
                    .source_verifier_material
                    .message_inclusion_verifier_hash
            ));
            assert!(h256_is_nonzero(
                &readiness.source_verifier_material.finality_policy_hash
            ));
            assert!(h256_is_nonzero(&sccp_source_verifier_material_hash(
                &readiness.source_verifier_material
            )));
            assert!(!sccp_source_verifier_material_is_production_ready(
                &readiness.source_verifier_material
            ));
            assert!(readiness.adapter_statement_binding_ready);
            assert!(readiness.adapter_open_verify_ready);
            assert!(readiness.finality_model_ready);
            assert!(!readiness.external_consensus_verifier_ready);
            assert!(!readiness.external_message_inclusion_verifier_ready);
            assert!(!readiness.source_trust_anchor_ready);
            assert!(!readiness.production_ready);
            assert!(!readiness.blockers.is_empty());
            assert!(
                readiness
                    .blockers
                    .iter()
                    .any(|blocker| blocker.contains("verifier is not deployed"))
            );
            assert!(
                readiness
                    .blockers
                    .iter()
                    .any(|blocker| blocker.contains("trust anchor is not active"))
            );
        }
        assert!(sccp_source_adapter_engine_readiness_for_domain(SCCP_DOMAIN_SORA).is_none());
        assert!(!sccp_source_adapter_ready_for_domain(SCCP_DOMAIN_SORA));
    }

    #[test]
    fn destination_rollout_production_gate_rejects_incomplete_or_replayed_material() {
        let mut rollout =
            sccp_destination_rollout_for_domain(SCCP_DOMAIN_ETH).expect("eth rollout");
        assert!(!sccp_destination_rollout_is_production_ready(
            SCCP_DOMAIN_ETH,
            &rollout
        ));

        rollout.immutable_verifier_ready = true;
        rollout.anchors_ready = true;
        rollout.verifier_identity = Some("0x1111111111111111111111111111111111111111".to_owned());
        rollout.verifier_code_hash = Some(format!("0x{}", "22".repeat(32)));
        rollout.anchor_id = Some("eth-beacon-finality-root-mainnet".to_owned());
        rollout.blockers.clear();
        assert!(sccp_destination_rollout_is_production_ready(
            SCCP_DOMAIN_ETH,
            &rollout
        ));

        let mut wrong_domain = rollout.clone();
        wrong_domain.domain = SCCP_DOMAIN_BSC;
        assert!(!sccp_destination_rollout_is_production_ready(
            SCCP_DOMAIN_ETH,
            &wrong_domain
        ));

        let mut wrong_chain = rollout.clone();
        wrong_chain.chain = "bsc".to_owned();
        assert!(!sccp_destination_rollout_is_production_ready(
            SCCP_DOMAIN_ETH,
            &wrong_chain
        ));

        let mut wrong_plan = rollout.clone();
        wrong_plan.verifier_plan = SccpDestinationVerifierPlanV1::SolanaProgramNativeRecursive;
        assert!(!sccp_destination_rollout_is_production_ready(
            SCCP_DOMAIN_ETH,
            &wrong_plan
        ));

        let mut missing_verifier = rollout.clone();
        missing_verifier.verifier_identity = Some(" ".to_owned());
        assert!(!sccp_destination_rollout_is_production_ready(
            SCCP_DOMAIN_ETH,
            &missing_verifier
        ));

        let mut bad_code_hash = rollout.clone();
        bad_code_hash.verifier_code_hash = Some("0x12".to_owned());
        assert!(!sccp_destination_rollout_is_production_ready(
            SCCP_DOMAIN_ETH,
            &bad_code_hash
        ));

        let mut non_hex_code_hash = rollout.clone();
        non_hex_code_hash.verifier_code_hash = Some(format!("0x{}zz", "33".repeat(31)));
        assert!(!sccp_destination_rollout_is_production_ready(
            SCCP_DOMAIN_ETH,
            &non_hex_code_hash
        ));

        let mut zero_code_hash = rollout.clone();
        zero_code_hash.verifier_code_hash = Some(format!("0x{}", "00".repeat(32)));
        assert!(!sccp_destination_rollout_is_production_ready(
            SCCP_DOMAIN_ETH,
            &zero_code_hash
        ));

        let mut missing_anchor = rollout.clone();
        missing_anchor.anchor_id = None;
        assert!(!sccp_destination_rollout_is_production_ready(
            SCCP_DOMAIN_ETH,
            &missing_anchor
        ));

        let mut blocked = rollout;
        blocked
            .blockers
            .push("operator has not cut over".to_owned());
        assert!(!sccp_destination_rollout_is_production_ready(
            SCCP_DOMAIN_ETH,
            &blocked
        ));
    }

    #[test]
    fn source_verifier_material_production_gate_rejects_placeholder_and_mutations() {
        let mut material =
            sccp_source_verifier_material_for_domain(SCCP_DOMAIN_ETH).expect("eth material");
        assert!(material.placeholder_material);
        assert!(!sccp_source_verifier_material_is_production_ready(
            &material
        ));

        material.placeholder_material = false;
        assert!(sccp_source_verifier_material_uses_builtin_placeholder_components(&material));
        assert!(!sccp_source_verifier_material_is_production_ready(
            &material
        ));

        let material = sample_ready_source_verifier_material(SCCP_DOMAIN_ETH);
        assert!(!sccp_source_verifier_material_uses_builtin_placeholder_components(&material));
        assert!(sccp_source_verifier_material_is_production_ready(&material));

        let mut wrong_domain = material.clone();
        wrong_domain.source_domain = SCCP_DOMAIN_BSC;
        assert!(!sccp_source_verifier_material_is_production_ready(
            &wrong_domain
        ));

        let mut wrong_chain = material.clone();
        wrong_chain.source_chain.push_str("-replay");
        assert!(!sccp_source_verifier_material_is_production_ready(
            &wrong_chain
        ));

        let mut wrong_plan = material.clone();
        wrong_plan.source_proof_plan = SccpSourceProofPlanV1::BscValidatorSetReceiptProof;
        assert!(!sccp_source_verifier_material_is_production_ready(
            &wrong_plan
        ));

        let mut wrong_finality = material.clone();
        wrong_finality.finality_model = SccpProofFinalityModelV1::BscValidatorSet;
        assert!(!sccp_source_verifier_material_is_production_ready(
            &wrong_finality
        ));

        let mut wrong_circuit = material.clone();
        wrong_circuit.adapter_circuit_id.push_str("-old");
        assert!(!sccp_source_verifier_material_is_production_ready(
            &wrong_circuit
        ));

        let mut missing_anchor_id = material.clone();
        missing_anchor_id.source_trust_anchor_id.clear();
        assert!(!sccp_source_verifier_material_is_production_ready(
            &missing_anchor_id
        ));

        let mut zero_anchor = material.clone();
        zero_anchor.source_trust_anchor_hash = [0u8; 32];
        assert!(!sccp_source_verifier_material_is_production_ready(
            &zero_anchor
        ));

        let mut missing_consensus_id = material.clone();
        missing_consensus_id.consensus_verifier_id.clear();
        assert!(!sccp_source_verifier_material_is_production_ready(
            &missing_consensus_id
        ));

        let mut zero_consensus = material.clone();
        zero_consensus.consensus_verifier_hash = [0u8; 32];
        assert!(!sccp_source_verifier_material_is_production_ready(
            &zero_consensus
        ));

        let mut missing_inclusion_id = material.clone();
        missing_inclusion_id.message_inclusion_verifier_id.clear();
        assert!(!sccp_source_verifier_material_is_production_ready(
            &missing_inclusion_id
        ));

        let mut zero_inclusion = material.clone();
        zero_inclusion.message_inclusion_verifier_hash = [0u8; 32];
        assert!(!sccp_source_verifier_material_is_production_ready(
            &zero_inclusion
        ));

        let mut missing_policy_id = material.clone();
        missing_policy_id.finality_policy_id.clear();
        assert!(!sccp_source_verifier_material_is_production_ready(
            &missing_policy_id
        ));

        let mut zero_policy = material;
        zero_policy.finality_policy_hash = [0u8; 32];
        assert!(!sccp_source_verifier_material_is_production_ready(
            &zero_policy
        ));
    }

    #[test]
    fn submission_package_builder_refuses_disabled_lane_templates() {
        let bundle = sample_message_bundle(SccpPayloadV1::Transfer(TransferPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_TRON,
            nonce: 41,
            asset_home_domain: SCCP_DOMAIN_SORA,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#universal".to_vec(),
            amount: 13,
            sender_codec: SCCP_CODEC_TEXT_UTF8,
            sender: b"sora:bridge".to_vec(),
            recipient_codec: SCCP_CODEC_TRON_BASE58CHECK,
            recipient: b"TJRabPrwbZy45sbavfcjinPJC18kjpRTv8".to_vec(),
            route_id_codec: SCCP_CODEC_TEXT_UTF8,
            route_id: b"nexus:tron:xor".to_vec(),
        }));
        let manifest = sccp_proof_manifest_for_domain(SCCP_DOMAIN_TRON).expect("tron manifest");
        assert!(build_sccp_counterparty_submission_package(&bundle, &manifest, &[0xAA]).is_none());
    }

    #[test]
    fn configured_source_material_does_not_bypass_disabled_manifest_gate() {
        let manifest = sccp_proof_manifest_for_domain(SCCP_DOMAIN_SOL).expect("sol manifest");
        let material = sample_ready_source_verifier_material(SCCP_DOMAIN_SOL);
        let bundle = sample_transfer_bundle_with_source_material(
            SCCP_DOMAIN_SOL,
            SCCP_DOMAIN_SORA,
            702,
            Some(&material),
        );
        assert!(
            !sccp_source_verifier_material_is_production_ready(&material),
            "SOL source material must stay fail-closed until the real mainnet verifier is wired"
        );
        assert!(!manifest.production_ready);
        assert!(
            build_nexus_sccp_message_transparent_proof_with_source_verifier_material_allow_unready(
                &bundle, &material, true,
            )
            .is_some(),
            "diagnostic builders may still render explicit-material fixtures when allow_unready is set"
        );

        assert!(
            build_sccp_counterparty_submission_package_with_source_verifier_material(
                &bundle,
                &manifest,
                &[0xAA],
                &material,
            )
            .is_none()
        );
        assert!(
            build_nexus_sccp_message_transparent_proof_with_source_verifier_material(
                &bundle, &material,
            )
            .is_none()
        );
    }

    #[test]
    fn verify_transparent_proof_structure_rejects_disabled_lane_even_if_shape_matches_manifest() {
        let bundle = sample_message_bundle(SccpPayloadV1::Transfer(TransferPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_TON,
            nonce: 42,
            asset_home_domain: SCCP_DOMAIN_SORA,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#universal".to_vec(),
            amount: 21,
            sender_codec: SCCP_CODEC_TEXT_UTF8,
            sender: b"sora:bridge".to_vec(),
            recipient_codec: SCCP_CODEC_TON_RAW,
            recipient: b"0:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                .to_vec(),
            route_id_codec: SCCP_CODEC_TEXT_UTF8,
            route_id: b"nexus:ton:xor".to_vec(),
        }));
        let manifest = sccp_proof_manifest_for_domain(SCCP_DOMAIN_TON).expect("ton manifest");
        let public_inputs =
            sccp_message_transparent_public_inputs(&bundle).expect("message public inputs");
        let proof = NexusSccpMessageTransparentProofV1 {
            version: 1,
            local_domain: manifest.local_domain,
            counterparty_domain: manifest.counterparty_domain,
            security_model: manifest.security_model,
            anchor_governance: manifest.anchor_governance,
            destination_binding: manifest.destination_binding.clone(),
            proof_family: manifest.proof_family.clone(),
            verifier_backend: manifest.verifier_backend.clone(),
            message_backend: manifest.message_backend.clone(),
            registry_backend: manifest.registry_backend.clone(),
            manifest_seed: manifest.manifest_seed.clone(),
            finality_model: manifest.finality_model,
            verifier_target: manifest.verifier_target,
            public_inputs,
            proof_bytes: vec![0xAA, 0xBB],
            submission_package: SccpCounterpartySubmissionPackageV1 {
                version: 1,
                proof_family: manifest.proof_family.clone(),
                verifier_backend: manifest.verifier_backend.clone(),
                envelope_encoding: "ton_message_body_boc_v1".to_owned(),
                submission_kind: manifest.submission_template.submission_kind.clone(),
                verifier_entrypoint: manifest.submission_template.verifier_entrypoint.clone(),
                platform_payload: SccpPlatformSubmissionPayloadV1::TonInternalMessage(
                    SccpTonInternalMessageSubmissionPayloadV1 {
                        message_body_boc: vec![0xEE],
                        query_id: 0,
                        destination_binding: manifest.destination_binding.clone(),
                        destination_binding_hash: manifest.destination_binding.binding_hash,
                        proof_bytes: vec![0xAA, 0xBB],
                        public_inputs_bytes: vec![0xCC],
                        bundle_bytes: vec![0xDD],
                        statement_hash: [0u8; 32],
                    },
                ),
                arguments: Vec::new(),
                envelope_bytes: vec![0xEE],
            },
            bundle,
        };

        assert!(!verify_nexus_sccp_message_transparent_proof_structure(
            &proof
        ));
    }

    #[test]
    fn transparent_message_proof_recovery_rejects_raw_bundle_bytes() {
        let bundle = sample_message_bundle(SccpPayloadV1::Transfer(TransferPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 19,
            asset_home_domain: SCCP_DOMAIN_ETH,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#eth".to_vec(),
            amount: 7,
            sender_codec: SCCP_CODEC_EVM_HEX,
            sender: b"0x9999999999999999999999999999999999999999".to_vec(),
            recipient_codec: SCCP_CODEC_TEXT_UTF8,
            recipient: b"alice@universal".to_vec(),
            route_id_codec: SCCP_CODEC_TEXT_UTF8,
            route_id: b"eth:sora:xor".to_vec(),
        }));
        let legacy_bytes = to_bytes(&bundle).expect("encode bundle");

        assert!(
            recover_nexus_sccp_message_transparent_proof("sccp/stark-fri-v1/eth", &legacy_bytes)
                .is_none()
        );
    }

    #[test]
    fn transparent_message_proof_recovery_rejects_raw_open_verify_envelope_bytes() {
        let bundle = sample_tron_transfer_bundle(36);
        let manifest = sccp_proof_manifest_for_domain(SCCP_DOMAIN_TRON).expect("tron manifest");
        let proof_bytes =
            build_sccp_message_transparent_fastpq_proof_bytes(&bundle, &manifest).expect("proof");

        assert!(
            recover_nexus_sccp_message_transparent_proof("sccp/stark-fri-v1/tron", &proof_bytes)
                .is_none()
        );
        assert!(decode_nexus_sccp_message_transparent_proof(&proof_bytes).is_none());
    }

    #[test]
    fn transparent_message_proof_recovery_rejects_truncated_typed_artifact_bytes() {
        let bundle = sample_tron_transfer_bundle(37);
        let manifest = sccp_proof_manifest_for_domain(SCCP_DOMAIN_TRON).expect("tron manifest");
        let public_inputs =
            sccp_message_transparent_public_inputs(&bundle).expect("message public inputs");
        let proof_cell = vec![0xAA, 0xBB];
        let proof_bytes = to_bytes(&NexusSccpMessageTransparentProofV1 {
            version: 1,
            local_domain: manifest.local_domain,
            counterparty_domain: manifest.counterparty_domain,
            security_model: manifest.security_model,
            anchor_governance: manifest.anchor_governance,
            destination_binding: manifest.destination_binding.clone(),
            proof_family: manifest.proof_family.clone(),
            verifier_backend: manifest.verifier_backend.clone(),
            message_backend: manifest.message_backend.clone(),
            registry_backend: manifest.registry_backend.clone(),
            manifest_seed: manifest.manifest_seed.clone(),
            finality_model: manifest.finality_model,
            verifier_target: manifest.verifier_target,
            public_inputs: public_inputs.clone(),
            proof_bytes: proof_cell.clone(),
            submission_package: SccpCounterpartySubmissionPackageV1 {
                version: 1,
                proof_family: manifest.proof_family.clone(),
                verifier_backend: manifest.verifier_backend.clone(),
                envelope_encoding: "tron_tvm_abi_v1".to_owned(),
                submission_kind: "contract_call".to_owned(),
                verifier_entrypoint: "verify_sccp_message".to_owned(),
                platform_payload: SccpPlatformSubmissionPayloadV1::TronContractCall(
                    SccpTronContractSubmissionPayloadV1 {
                        proof_bytes: proof_cell,
                        public_inputs: sccp_evm_public_input_word_struct(&public_inputs),
                        statement_hash: [0u8; 32],
                        destination_binding: manifest.destination_binding.clone(),
                    },
                ),
                arguments: Vec::new(),
                envelope_bytes: vec![0xCC],
            },
            bundle,
        })
        .expect("encode typed artifact");
        let truncated = &proof_bytes[..proof_bytes.len().saturating_sub(3)];

        assert!(
            recover_nexus_sccp_message_transparent_proof("sccp/stark-fri-v1/tron", truncated)
                .is_none()
        );
        assert!(decode_nexus_sccp_message_transparent_proof(truncated).is_none());
    }

    #[test]
    fn canonical_message_payload_roundtrips() {
        let payload = SccpPayloadV1::RouteActivate(RouteActivatePayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            target_domain: SCCP_DOMAIN_TON,
            nonce: 9,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#universal".to_vec(),
            route_id_codec: SCCP_CODEC_TEXT_UTF8,
            route_id: b"nexus:ton:xor".to_vec(),
        });
        let encoded = canonical_sccp_payload_bytes(&payload);
        let decoded = decode_canonical_sccp_payload_bytes(&encoded).expect("decode payload");
        assert_eq!(decoded, payload);
        assert!(verify_sccp_payload_structure(&decoded));
    }

    #[test]
    fn canonical_message_payload_decoder_rejects_truncated_trailing_and_unknown_bytes() {
        let payload = SccpPayloadV1::RouteActivate(RouteActivatePayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            target_domain: SCCP_DOMAIN_TRON,
            nonce: 10,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#universal".to_vec(),
            route_id_codec: SCCP_CODEC_TEXT_UTF8,
            route_id: b"nexus:tron:xor".to_vec(),
        });
        let encoded = canonical_sccp_payload_bytes(&payload);

        assert!(decode_canonical_sccp_payload_bytes(&[]).is_none());
        assert!(decode_canonical_sccp_payload_bytes(&[0xFF]).is_none());
        assert!(decode_canonical_sccp_payload_bytes(&encoded[..encoded.len() - 1]).is_none());

        let mut trailing = encoded;
        trailing.push(0x00);
        assert!(decode_canonical_sccp_payload_bytes(&trailing).is_none());
    }

    #[test]
    fn domain_codec_and_manifest_helpers_reject_unknown_values() {
        const UNKNOWN_DOMAIN: u32 = 0xFFFF_FFFE;
        const UNKNOWN_CODEC: u8 = 0xFE;

        assert!(!is_supported_domain(UNKNOWN_DOMAIN));
        assert_eq!(sccp_chain_key_for_domain(UNKNOWN_DOMAIN), None);
        assert_eq!(sccp_counterparty_account_codec(UNKNOWN_DOMAIN), None);
        assert_eq!(sccp_verifier_backend_for_domain(UNKNOWN_DOMAIN), None);
        assert_eq!(sccp_message_backend_for_domain(UNKNOWN_DOMAIN), None);
        assert_eq!(sccp_registry_backend_for_domain(UNKNOWN_DOMAIN), None);
        assert_eq!(sccp_manifest_seed_for_domain(UNKNOWN_DOMAIN), None);
        assert_eq!(sccp_destination_rollout_for_domain(UNKNOWN_DOMAIN), None);
        assert_eq!(sccp_destination_binding_for_domain(UNKNOWN_DOMAIN), None);
        assert_eq!(sccp_submission_template_for_domain(UNKNOWN_DOMAIN), None);
        assert_eq!(sccp_proof_manifest_for_domain(UNKNOWN_DOMAIN), None);

        assert!(!is_supported_codec(UNKNOWN_CODEC));
        assert_eq!(sccp_codec_key(UNKNOWN_CODEC), None);
        assert_eq!(sccp_codec_description(UNKNOWN_CODEC), None);
        assert_eq!(
            decode_sccp_normalized_codec_value(UNKNOWN_CODEC, b"value"),
            None
        );
        assert_eq!(
            decode_sccp_normalized_codec_value(SCCP_CODEC_TEXT_UTF8, b""),
            None
        );
    }

    #[test]
    fn message_payload_structure_rejects_adversarial_asset_route_and_transfer_edges() {
        const UNKNOWN_DOMAIN: u32 = 0xFFFF_FFFE;

        let asset = AssetRegisterPayloadV1 {
            version: 1,
            target_domain: SCCP_DOMAIN_ETH,
            home_domain: SCCP_DOMAIN_SORA,
            nonce: 1,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#universal".to_vec(),
            decimals: 18,
        };
        assert!(verify_sccp_payload_structure(
            &SccpPayloadV1::AssetRegister(asset.clone())
        ));

        let mut payload = asset.clone();
        payload.version = 2;
        assert_rejects_message_payload(SccpPayloadV1::AssetRegister(payload));

        let mut payload = asset.clone();
        payload.home_domain = UNKNOWN_DOMAIN;
        assert_rejects_message_payload(SccpPayloadV1::AssetRegister(payload));

        let mut payload = asset.clone();
        payload.asset_id.clear();
        assert_rejects_message_payload(SccpPayloadV1::AssetRegister(payload));

        let route = RouteActivatePayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            target_domain: SCCP_DOMAIN_ETH,
            nonce: 2,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#universal".to_vec(),
            route_id_codec: SCCP_CODEC_TEXT_UTF8,
            route_id: b"nexus:eth:xor".to_vec(),
        };
        assert!(verify_sccp_payload_structure(
            &SccpPayloadV1::RouteActivate(route.clone())
        ));

        let mut payload = route.clone();
        payload.source_domain = payload.target_domain;
        assert_rejects_message_payload(SccpPayloadV1::RouteActivate(payload));

        let mut payload = route.clone();
        payload.source_domain = UNKNOWN_DOMAIN;
        assert_rejects_message_payload(SccpPayloadV1::RouteActivate(payload));

        let mut payload = route.clone();
        payload.route_id.clear();
        assert_rejects_message_payload(SccpPayloadV1::RouteActivate(payload));

        let mut payload = route.clone();
        payload.route_id_codec = SCCP_CODEC_EVM_HEX;
        assert_rejects_message_payload(SccpPayloadV1::RouteActivate(payload));

        let transfer = TransferPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_ETH,
            nonce: 3,
            asset_home_domain: SCCP_DOMAIN_SORA,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#universal".to_vec(),
            amount: 77,
            sender_codec: SCCP_CODEC_TEXT_UTF8,
            sender: b"sora:bridge".to_vec(),
            recipient_codec: SCCP_CODEC_EVM_HEX,
            recipient: b"0x1111111111111111111111111111111111111111".to_vec(),
            route_id_codec: SCCP_CODEC_TEXT_UTF8,
            route_id: b"nexus:eth:xor".to_vec(),
        };
        assert!(verify_sccp_payload_structure(&SccpPayloadV1::Transfer(
            transfer.clone()
        )));

        let mut payload = transfer.clone();
        payload.dest_domain = payload.source_domain;
        assert_rejects_message_payload(SccpPayloadV1::Transfer(payload));

        let mut payload = transfer.clone();
        payload.asset_home_domain = UNKNOWN_DOMAIN;
        assert_rejects_message_payload(SccpPayloadV1::Transfer(payload));

        let mut payload = transfer.clone();
        payload.asset_id.clear();
        assert_rejects_message_payload(SccpPayloadV1::Transfer(payload));

        let mut payload = transfer.clone();
        payload.amount = 0;
        assert_rejects_message_payload(SccpPayloadV1::Transfer(payload));

        let mut payload = transfer;
        payload.route_id.clear();
        assert_rejects_message_payload(SccpPayloadV1::Transfer(payload));
    }

    #[test]
    fn counterparty_domain_helpers_prefer_the_remote_side() {
        let asset_register = SccpPayloadV1::AssetRegister(AssetRegisterPayloadV1 {
            version: 1,
            target_domain: SCCP_DOMAIN_ETH,
            home_domain: SCCP_DOMAIN_SORA,
            nonce: 1,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#universal".to_vec(),
            decimals: 18,
        });
        let route_activate = SccpPayloadV1::RouteActivate(RouteActivatePayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_TRON,
            target_domain: SCCP_DOMAIN_SORA,
            nonce: 2,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#universal".to_vec(),
            route_id_codec: SCCP_CODEC_TEXT_UTF8,
            route_id: b"tron:sora:xor".to_vec(),
        });
        let transfer = SccpPayloadV1::Transfer(TransferPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SOL,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 3,
            asset_home_domain: SCCP_DOMAIN_SORA,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#universal".to_vec(),
            amount: 5,
            sender_codec: SCCP_CODEC_SOLANA_BASE58,
            sender: b"11111111111111111111111111111111".to_vec(),
            recipient_codec: SCCP_CODEC_TEXT_UTF8,
            recipient: b"alice@sora".to_vec(),
            route_id_codec: SCCP_CODEC_TEXT_UTF8,
            route_id: b"sol:sora:xor".to_vec(),
        });

        assert_eq!(
            sccp_counterparty_domain_for_message_payload(&asset_register),
            Some(SCCP_DOMAIN_ETH)
        );
        assert_eq!(
            sccp_counterparty_domain_for_message_payload(&route_activate),
            Some(SCCP_DOMAIN_TRON)
        );
        assert_eq!(
            sccp_counterparty_domain_for_message_payload(&transfer),
            Some(SCCP_DOMAIN_SOL)
        );
        assert_eq!(
            sccp_counterparty_domain(SCCP_DOMAIN_SORA, SCCP_DOMAIN_SORA),
            None
        );
    }

    #[test]
    fn payload_projection_covers_asset_register_and_route_activate() {
        let asset_register = SccpPayloadV1::AssetRegister(AssetRegisterPayloadV1 {
            version: 1,
            target_domain: SCCP_DOMAIN_ETH,
            home_domain: SCCP_DOMAIN_SORA,
            nonce: 11,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#universal".to_vec(),
            decimals: 18,
        });
        let route_activate = SccpPayloadV1::RouteActivate(RouteActivatePayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            target_domain: SCCP_DOMAIN_TON,
            nonce: 12,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#universal".to_vec(),
            route_id_codec: SCCP_CODEC_TEXT_UTF8,
            route_id: b"nexus:ton:xor".to_vec(),
        });

        assert_eq!(
            sccp_message_payload_kind_key(&asset_register),
            "asset_register"
        );
        match sccp_payload_projection(&asset_register).expect("asset projection") {
            SccpPayloadProjectionV1::AssetRegister(projection) => {
                assert_eq!(projection.target_domain, SCCP_DOMAIN_ETH);
                assert_eq!(projection.home_domain, SCCP_DOMAIN_SORA);
                assert_eq!(projection.decimals, 18);
                assert_eq!(
                    projection.asset_id,
                    SccpNormalizedCodecValueV1::TextUtf8 {
                        value: "xor#universal".to_owned()
                    }
                );
            }
            other => panic!("unexpected asset projection: {other:?}"),
        }

        assert_eq!(
            sccp_message_payload_kind_key(&route_activate),
            "route_activate"
        );
        match sccp_payload_projection(&route_activate).expect("route projection") {
            SccpPayloadProjectionV1::RouteActivate(projection) => {
                assert_eq!(projection.source_domain, SCCP_DOMAIN_SORA);
                assert_eq!(projection.target_domain, SCCP_DOMAIN_TON);
                assert_eq!(
                    projection.route_id,
                    SccpNormalizedCodecValueV1::TextUtf8 {
                        value: "nexus:ton:xor".to_owned()
                    }
                );
            }
            other => panic!("unexpected route projection: {other:?}"),
        }
    }

    #[test]
    fn payload_projection_rejects_malformed_codec_values_without_normalizing() {
        let asset_register = SccpPayloadV1::AssetRegister(AssetRegisterPayloadV1 {
            version: 1,
            target_domain: SCCP_DOMAIN_ETH,
            home_domain: SCCP_DOMAIN_SORA,
            nonce: 31,
            asset_id_codec: SCCP_CODEC_EVM_HEX,
            asset_id: b"0xfeedface".to_vec(),
            decimals: 18,
        });
        assert!(!verify_sccp_payload_structure(&asset_register));
        assert_eq!(sccp_payload_projection(&asset_register), None);

        let route_activate = SccpPayloadV1::RouteActivate(RouteActivatePayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            target_domain: SCCP_DOMAIN_TON,
            nonce: 32,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#universal".to_vec(),
            route_id_codec: SCCP_CODEC_TON_RAW,
            route_id: b"+0:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                .to_vec(),
        });
        assert!(!verify_sccp_payload_structure(&route_activate));
        assert_eq!(sccp_payload_projection(&route_activate), None);

        let transfer = SccpPayloadV1::Transfer(TransferPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SOL,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 33,
            asset_home_domain: SCCP_DOMAIN_SORA,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#universal".to_vec(),
            amount: 11,
            sender_codec: SCCP_CODEC_SOLANA_BASE58,
            sender: b"not a solana public key".to_vec(),
            recipient_codec: SCCP_CODEC_TEXT_UTF8,
            recipient: b"alice@sora".to_vec(),
            route_id_codec: SCCP_CODEC_TEXT_UTF8,
            route_id: b"sol:sora:xor".to_vec(),
        });
        assert!(!verify_sccp_payload_structure(&transfer));
        assert_eq!(sccp_payload_projection(&transfer), None);
    }

    #[test]
    fn canonical_payload_decoder_rejects_unknown_truncated_and_trailing_bytes() {
        let payload = SccpPayloadV1::Transfer(TransferPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_ETH,
            nonce: 13,
            asset_home_domain: SCCP_DOMAIN_SORA,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#universal".to_vec(),
            amount: 7,
            sender_codec: SCCP_CODEC_TEXT_UTF8,
            sender: b"sora:bridge".to_vec(),
            recipient_codec: SCCP_CODEC_EVM_HEX,
            recipient: b"0x1111111111111111111111111111111111111111".to_vec(),
            route_id_codec: SCCP_CODEC_TEXT_UTF8,
            route_id: b"nexus:eth:xor".to_vec(),
        });
        let encoded = canonical_sccp_payload_bytes(&payload);
        let mut truncated = encoded.clone();
        truncated.pop();
        let mut with_trailing = encoded.clone();
        with_trailing.push(0);

        assert_eq!(decode_canonical_sccp_payload_bytes(&[]), None);
        assert_eq!(decode_canonical_sccp_payload_bytes(&[0xFE]), None);
        assert_eq!(decode_canonical_sccp_payload_bytes(&truncated), None);
        assert_eq!(decode_canonical_sccp_payload_bytes(&with_trailing), None);
        assert_eq!(decode_canonical_sccp_payload_bytes(&encoded), Some(payload));
    }

    #[test]
    fn commitment_merkle_helpers_support_multi_message_blocks() {
        let payloads = [
            SccpPayloadV1::AssetRegister(AssetRegisterPayloadV1 {
                version: 1,
                target_domain: SCCP_DOMAIN_ETH,
                home_domain: SCCP_DOMAIN_SORA,
                nonce: 1,
                asset_id_codec: SCCP_CODEC_TEXT_UTF8,
                asset_id: b"xor#universal".to_vec(),
                decimals: 18,
            }),
            SccpPayloadV1::RouteActivate(RouteActivatePayloadV1 {
                version: 1,
                source_domain: SCCP_DOMAIN_SORA,
                target_domain: SCCP_DOMAIN_ETH,
                nonce: 2,
                asset_id_codec: SCCP_CODEC_TEXT_UTF8,
                asset_id: b"xor#universal".to_vec(),
                route_id_codec: SCCP_CODEC_TEXT_UTF8,
                route_id: b"nexus:eth:xor".to_vec(),
            }),
            SccpPayloadV1::Transfer(TransferPayloadV1 {
                version: 1,
                source_domain: SCCP_DOMAIN_SORA,
                dest_domain: SCCP_DOMAIN_ETH,
                nonce: 3,
                asset_home_domain: SCCP_DOMAIN_SORA,
                asset_id_codec: SCCP_CODEC_TEXT_UTF8,
                asset_id: b"xor#universal".to_vec(),
                amount: 77,
                sender_codec: SCCP_CODEC_TEXT_UTF8,
                sender: b"sora:bridge".to_vec(),
                recipient_codec: SCCP_CODEC_EVM_HEX,
                recipient: b"0x2222222222222222222222222222222222222222".to_vec(),
                route_id_codec: SCCP_CODEC_TEXT_UTF8,
                route_id: b"nexus:eth:xor".to_vec(),
            }),
        ];
        let commitments: Vec<_> = payloads
            .iter()
            .map(hub_commitment_from_sccp_payload)
            .collect();
        let root = commitment_merkle_root(&commitments).expect("non-empty root");
        let proof = commitment_merkle_proof(&commitments, 1).expect("proof for middle message");

        assert_eq!(merkle_root_from_commitment(&commitments[1], &proof), root);
    }

    #[test]
    fn merkle_helpers_cover_empty_singleton_and_odd_last_leaf() {
        let payloads = [
            SccpPayloadV1::AssetRegister(AssetRegisterPayloadV1 {
                version: 1,
                target_domain: SCCP_DOMAIN_ETH,
                home_domain: SCCP_DOMAIN_SORA,
                nonce: 21,
                asset_id_codec: SCCP_CODEC_TEXT_UTF8,
                asset_id: b"xor#universal".to_vec(),
                decimals: 18,
            }),
            SccpPayloadV1::RouteActivate(RouteActivatePayloadV1 {
                version: 1,
                source_domain: SCCP_DOMAIN_SORA,
                target_domain: SCCP_DOMAIN_ETH,
                nonce: 22,
                asset_id_codec: SCCP_CODEC_TEXT_UTF8,
                asset_id: b"xor#universal".to_vec(),
                route_id_codec: SCCP_CODEC_TEXT_UTF8,
                route_id: b"nexus:eth:xor".to_vec(),
            }),
            SccpPayloadV1::Transfer(TransferPayloadV1 {
                version: 1,
                source_domain: SCCP_DOMAIN_SORA,
                dest_domain: SCCP_DOMAIN_ETH,
                nonce: 23,
                asset_home_domain: SCCP_DOMAIN_SORA,
                asset_id_codec: SCCP_CODEC_TEXT_UTF8,
                asset_id: b"xor#universal".to_vec(),
                amount: 77,
                sender_codec: SCCP_CODEC_TEXT_UTF8,
                sender: b"sora:bridge".to_vec(),
                recipient_codec: SCCP_CODEC_EVM_HEX,
                recipient: b"0x2222222222222222222222222222222222222222".to_vec(),
                route_id_codec: SCCP_CODEC_TEXT_UTF8,
                route_id: b"nexus:eth:xor".to_vec(),
            }),
        ];
        let commitments: Vec<_> = payloads
            .iter()
            .map(hub_commitment_from_sccp_payload)
            .collect();

        assert_eq!(commitment_merkle_root(&[]), None);
        assert_eq!(
            commitment_merkle_proof(&commitments, commitments.len()),
            None
        );

        let singleton = &commitments[..1];
        let singleton_root = commitment_merkle_root(singleton).expect("singleton root");
        let singleton_proof = commitment_merkle_proof(singleton, 0).expect("singleton proof");
        assert!(singleton_proof.steps.is_empty());
        assert_eq!(
            merkle_root_from_commitment(&singleton[0], &singleton_proof),
            singleton_root
        );

        let root = commitment_merkle_root(&commitments).expect("odd root");
        let last_leaf_proof = commitment_merkle_proof(&commitments, 2).expect("last leaf proof");
        assert_eq!(last_leaf_proof.steps.len(), 1);
        assert_eq!(
            merkle_root_from_commitment(&commitments[2], &last_leaf_proof),
            root
        );
    }

    #[test]
    fn message_bundle_structure_rejects_commitment_kind_tampering() {
        let mut bundle = sample_message_bundle(SccpPayloadV1::Transfer(TransferPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_ETH,
            nonce: 31,
            asset_home_domain: SCCP_DOMAIN_SORA,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#universal".to_vec(),
            amount: 55,
            sender_codec: SCCP_CODEC_TEXT_UTF8,
            sender: b"sora:bridge".to_vec(),
            recipient_codec: SCCP_CODEC_EVM_HEX,
            recipient: b"0x1111111111111111111111111111111111111111".to_vec(),
            route_id_codec: SCCP_CODEC_TEXT_UTF8,
            route_id: b"nexus:eth:xor".to_vec(),
        }));

        bundle.commitment.kind = SccpHubMessageKind::RouteActivate;
        assert!(!verify_message_bundle_structure(&bundle));
    }

    #[test]
    fn message_bundle_structure_rejects_version_tampering() {
        let mut bundle = sample_tron_transfer_bundle(84);
        bundle.version = 2;
        assert!(!verify_message_bundle_structure(&bundle));

        let mut bundle = sample_tron_transfer_bundle(85);
        bundle.commitment.version = 2;
        assert!(!verify_message_bundle_structure(&bundle));
    }

    #[test]
    fn message_bundle_structure_rejects_target_domain_tampering() {
        let mut bundle = sample_tron_transfer_bundle(86);
        bundle.commitment.target_domain = SCCP_DOMAIN_TON;

        assert!(!verify_message_bundle_structure(&bundle));
    }

    #[test]
    fn message_bundle_structure_rejects_message_id_tampering() {
        let mut bundle = sample_tron_transfer_bundle(87);
        bundle.commitment.message_id[0] ^= 0x01;

        assert!(!verify_message_bundle_structure(&bundle));
    }

    #[test]
    fn message_bundle_structure_rejects_merkle_path_tampering() {
        let mut bundle = sample_tron_transfer_bundle(88);
        bundle.merkle_proof.steps.push(SccpMerkleStepV1 {
            sibling_hash: [0x55; 32],
            sibling_is_left: false,
        });

        assert!(!verify_message_bundle_structure(&bundle));
    }

    #[test]
    fn message_bundle_structure_rejects_mismatched_finality_root() {
        let mut bundle = sample_tron_transfer_bundle(80);
        bundle.finality_proof = sample_finality_proof([0x42; 32]);

        assert!(!verify_message_bundle_structure(&bundle));
    }

    #[test]
    fn message_bundle_structure_rejects_payload_hash_tampering() {
        let mut bundle = sample_tron_transfer_bundle(81);
        bundle.commitment.payload_hash[0] ^= 0x01;

        assert!(!verify_message_bundle_structure(&bundle));
    }

    #[test]
    fn message_bundle_structure_rejects_payload_substitution_after_commitment() {
        let mut bundle = sample_tron_transfer_bundle(82);
        let SccpPayloadV1::Transfer(payload) = &mut bundle.payload else {
            panic!("sample_tron_transfer_bundle must produce a transfer");
        };
        payload.amount += 1;

        assert!(!verify_message_bundle_structure(&bundle));
    }

    #[test]
    fn message_bundle_structure_rejects_truncated_finality_bytes() {
        let mut bundle = sample_tron_transfer_bundle(83);
        bundle.finality_proof.truncate(3);

        assert!(!verify_message_bundle_structure(&bundle));
    }

    #[test]
    fn finality_proof_structure_rejects_bad_bitmap_and_empty_pop() {
        let commitment_root = [0x88; 32];
        let mut proof = decode_nexus_bridge_finality_proof(&sample_finality_proof(commitment_root))
            .expect("decode proof");

        proof.commit_qc.signers_bitmap = vec![0b0000_0000];
        assert!(!verify_nexus_bridge_finality_proof_structure(&proof));

        proof.commit_qc.signers_bitmap = vec![0b0000_0010];
        assert!(!verify_nexus_bridge_finality_proof_structure(&proof));

        proof.commit_qc.signers_bitmap = vec![0b0000_0001];
        proof.commit_qc.validator_set_pops[0].clear();
        assert!(!verify_nexus_bridge_finality_proof_structure(&proof));
    }

    #[test]
    fn finality_proof_structure_rejects_padding_signer_bits_for_multi_byte_rosters() {
        let commitment_root = [0x8B; 32];
        let mut valid = decode_nexus_bridge_finality_proof(&sample_finality_proof(commitment_root))
            .expect("decode proof");
        valid.commit_qc.validator_public_keys =
            (0..9).map(|idx| format!("validator-{idx}")).collect();
        valid.commit_qc.validator_set_pops = (0..9)
            .map(|idx| vec![u8::try_from(idx + 1).expect("index fits u8")])
            .collect();
        valid.commit_qc.signers_bitmap = vec![0b0000_0001, 0b0000_0001];
        assert!(verify_nexus_bridge_finality_proof_structure(&valid));

        let mut padded_bit = valid.clone();
        padded_bit.commit_qc.signers_bitmap = vec![0b0000_0000, 0b0000_0010];
        assert!(!verify_nexus_bridge_finality_proof_structure(&padded_bit));

        let mut truncated_bitmap = valid;
        truncated_bitmap.commit_qc.signers_bitmap = vec![0b0000_0001];
        assert!(!verify_nexus_bridge_finality_proof_structure(
            &truncated_bitmap
        ));
    }

    #[test]
    fn finality_proof_structure_rejects_duplicate_validator_keys() {
        let commitment_root = [0x8C; 32];
        let mut proof = decode_nexus_bridge_finality_proof(&sample_finality_proof(commitment_root))
            .expect("decode proof");
        proof.commit_qc.validator_public_keys =
            vec!["validator-1".to_owned(), "validator-1".to_owned()];
        proof.commit_qc.validator_set_pops = vec![vec![0xAA], vec![0xBB]];
        proof.commit_qc.signers_bitmap = vec![0b0000_0011];

        assert!(!verify_nexus_bridge_finality_proof_structure(&proof));
    }

    #[test]
    fn finality_proof_structure_rejects_version_key_and_bitmap_edges() {
        let commitment_root = [0x8A; 32];
        let valid = decode_nexus_bridge_finality_proof(&sample_finality_proof(commitment_root))
            .expect("decode proof");

        let mut proof = valid.clone();
        proof.commit_qc.version = 2;
        assert!(!verify_nexus_bridge_finality_proof_structure(&proof));

        let mut proof = valid.clone();
        proof.commit_qc.validator_set_hash_version = 2;
        assert!(!verify_nexus_bridge_finality_proof_structure(&proof));

        let mut proof = valid.clone();
        proof.commit_qc.validator_public_keys[0].clear();
        assert!(!verify_nexus_bridge_finality_proof_structure(&proof));

        let mut proof = valid.clone();
        proof.commit_qc.signers_bitmap = vec![0b0000_0001, 0b0000_0000];
        assert!(!verify_nexus_bridge_finality_proof_structure(&proof));

        let mut proof = valid;
        proof.commit_qc.signers_bitmap.clear();
        assert!(!verify_nexus_bridge_finality_proof_structure(&proof));
    }

    #[test]
    fn finality_proof_structure_rejects_header_and_qc_field_tampering() {
        let commitment_root = [0x89; 32];
        let valid = decode_nexus_bridge_finality_proof(&sample_finality_proof(commitment_root))
            .expect("decode proof");

        let mut proof = valid.clone();
        proof.height = 0;
        assert!(!verify_nexus_bridge_finality_proof_structure(&proof));

        let mut proof = valid.clone();
        proof.version = 2;
        assert!(!verify_nexus_bridge_finality_proof_structure(&proof));

        let mut proof = valid.clone();
        proof.chain_id.clear();
        assert!(!verify_nexus_bridge_finality_proof_structure(&proof));

        let mut proof = valid.clone();
        proof.block_header_bytes.clear();
        assert!(!verify_nexus_bridge_finality_proof_structure(&proof));

        let mut proof = valid.clone();
        proof.block_hash[0] ^= 0x01;
        assert!(!verify_nexus_bridge_finality_proof_structure(&proof));

        let mut proof = valid.clone();
        proof.commit_qc.phase = NexusConsensusPhaseV1::Prepare;
        assert!(!verify_nexus_bridge_finality_proof_structure(&proof));

        let mut proof = valid.clone();
        proof.commit_qc.height += 1;
        assert!(!verify_nexus_bridge_finality_proof_structure(&proof));

        let mut proof = valid.clone();
        proof.commit_qc.subject_block_hash[0] ^= 0x01;
        assert!(!verify_nexus_bridge_finality_proof_structure(&proof));

        let mut proof = valid.clone();
        proof.commit_qc.mode_tag.clear();
        assert!(!verify_nexus_bridge_finality_proof_structure(&proof));

        let mut proof = valid.clone();
        proof.commit_qc.validator_public_keys.clear();
        proof.commit_qc.validator_set_pops.clear();
        proof.commit_qc.signers_bitmap.clear();
        assert!(!verify_nexus_bridge_finality_proof_structure(&proof));

        let mut proof = valid.clone();
        proof.commit_qc.validator_set_pops.pop();
        assert!(!verify_nexus_bridge_finality_proof_structure(&proof));

        let mut proof = valid;
        proof.commit_qc.bls_aggregate_signature.clear();
        assert!(!verify_nexus_bridge_finality_proof_structure(&proof));
    }

    #[test]
    #[allow(clippy::similar_names, clippy::too_many_lines)]
    fn codec_validation_accepts_chain_specific_v1_formats() {
        assert!(validate_evm_hex_codec(
            b"0x52908400098527886E0F7030069857D2E4169EE7"
        ));
        assert!(validate_evm_hex_codec(
            b"0xde709f2102306220921060314715629080e2fb77"
        ));

        let evm_transfer = SccpPayloadV1::Transfer(TransferPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_ETH,
            nonce: 1,
            asset_home_domain: SCCP_DOMAIN_SORA,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#universal".to_vec(),
            amount: 10,
            sender_codec: SCCP_CODEC_TEXT_UTF8,
            sender: b"bridge@sora".to_vec(),
            recipient_codec: SCCP_CODEC_EVM_HEX,
            recipient: b"0x3333333333333333333333333333333333333333".to_vec(),
            route_id_codec: SCCP_CODEC_TEXT_UTF8,
            route_id: b"nexus:eth:xor".to_vec(),
        });
        assert!(verify_sccp_payload_structure(&evm_transfer));

        let solana_transfer = SccpPayloadV1::Transfer(TransferPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_SOL,
            nonce: 2,
            asset_home_domain: SCCP_DOMAIN_SORA,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#universal".to_vec(),
            amount: 10,
            sender_codec: SCCP_CODEC_TEXT_UTF8,
            sender: b"bridge@sora".to_vec(),
            recipient_codec: SCCP_CODEC_SOLANA_BASE58,
            recipient: b"11111111111111111111111111111111".to_vec(),
            route_id_codec: SCCP_CODEC_TEXT_UTF8,
            route_id: b"nexus:sol:xor".to_vec(),
        });
        assert!(verify_sccp_payload_structure(&solana_transfer));

        let ton_recipient_payload = SccpPayloadV1::Transfer(TransferPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_TON,
            nonce: 3,
            asset_home_domain: SCCP_DOMAIN_SORA,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#universal".to_vec(),
            amount: 10,
            sender_codec: SCCP_CODEC_TEXT_UTF8,
            sender: b"bridge@sora".to_vec(),
            recipient_codec: SCCP_CODEC_TON_RAW,
            recipient: b"0:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                .to_vec(),
            route_id_codec: SCCP_CODEC_TEXT_UTF8,
            route_id: b"nexus:ton:xor".to_vec(),
        });
        assert!(verify_sccp_payload_structure(&ton_recipient_payload));

        let tron_recipient_payload = SccpPayloadV1::Transfer(TransferPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_TRON,
            nonce: 4,
            asset_home_domain: SCCP_DOMAIN_SORA,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#universal".to_vec(),
            amount: 10,
            sender_codec: SCCP_CODEC_TEXT_UTF8,
            sender: b"bridge@sora".to_vec(),
            recipient_codec: SCCP_CODEC_TRON_BASE58CHECK,
            recipient: b"T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb".to_vec(),
            route_id_codec: SCCP_CODEC_TEXT_UTF8,
            route_id: b"nexus:tron:xor".to_vec(),
        });
        assert!(verify_sccp_payload_structure(&tron_recipient_payload));

        for (domain, codec, sender) in [
            (
                SCCP_DOMAIN_ETH,
                SCCP_CODEC_EVM_HEX,
                b"0x3333333333333333333333333333333333333333".as_slice(),
            ),
            (
                SCCP_DOMAIN_BSC,
                SCCP_CODEC_EVM_HEX,
                b"0x3333333333333333333333333333333333333333".as_slice(),
            ),
            (
                SCCP_DOMAIN_SOL,
                SCCP_CODEC_SOLANA_BASE58,
                b"11111111111111111111111111111111".as_slice(),
            ),
            (
                SCCP_DOMAIN_TON,
                SCCP_CODEC_TON_RAW,
                b"0:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".as_slice(),
            ),
            (
                SCCP_DOMAIN_TRON,
                SCCP_CODEC_TRON_BASE58CHECK,
                b"T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb".as_slice(),
            ),
        ] {
            let inbound_transfer = SccpPayloadV1::Transfer(TransferPayloadV1 {
                version: 1,
                source_domain: domain,
                dest_domain: SCCP_DOMAIN_SORA,
                nonce: u64::from(domain),
                asset_home_domain: domain,
                asset_id_codec: SCCP_CODEC_TEXT_UTF8,
                asset_id: b"xor#remote".to_vec(),
                amount: 10,
                sender_codec: codec,
                sender: sender.to_vec(),
                recipient_codec: SCCP_CODEC_TEXT_UTF8,
                recipient: b"alice@sora".to_vec(),
                route_id_codec: SCCP_CODEC_TEXT_UTF8,
                route_id: b"remote:sora:xor".to_vec(),
            });
            assert!(verify_sccp_payload_structure(&inbound_transfer));
        }
    }

    #[test]
    #[allow(clippy::similar_names, clippy::too_many_lines)]
    fn codec_validation_rejects_malformed_chain_specific_v1_formats() {
        assert!(!validate_evm_hex_codec(
            b"0x52908400098527886e0f7030069857d2e4169ee7"
        ));
        assert!(!validate_evm_hex_codec(
            b"0X52908400098527886E0F7030069857D2E4169EE7"
        ));

        let bad_evm = SccpPayloadV1::Transfer(TransferPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_ETH,
            nonce: 1,
            asset_home_domain: SCCP_DOMAIN_SORA,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#universal".to_vec(),
            amount: 10,
            sender_codec: SCCP_CODEC_TEXT_UTF8,
            sender: b"bridge@sora".to_vec(),
            recipient_codec: SCCP_CODEC_EVM_HEX,
            recipient: b"0xfeedface".to_vec(),
            route_id_codec: SCCP_CODEC_TEXT_UTF8,
            route_id: b"nexus:eth:xor".to_vec(),
        });
        assert!(!verify_sccp_payload_structure(&bad_evm));

        let non_canonical_evm = SccpPayloadV1::Transfer(TransferPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_ETH,
            nonce: 2,
            asset_home_domain: SCCP_DOMAIN_SORA,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#universal".to_vec(),
            amount: 10,
            sender_codec: SCCP_CODEC_TEXT_UTF8,
            sender: b"bridge@sora".to_vec(),
            recipient_codec: SCCP_CODEC_EVM_HEX,
            recipient: b"0x52908400098527886e0f7030069857d2e4169ee7".to_vec(),
            route_id_codec: SCCP_CODEC_TEXT_UTF8,
            route_id: b"nexus:eth:xor".to_vec(),
        });
        assert!(!verify_sccp_payload_structure(&non_canonical_evm));

        let invalid_ton_recipient_payload = SccpPayloadV1::Transfer(TransferPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_TON,
            nonce: 3,
            asset_home_domain: SCCP_DOMAIN_SORA,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#universal".to_vec(),
            amount: 10,
            sender_codec: SCCP_CODEC_TEXT_UTF8,
            sender: b"bridge@sora".to_vec(),
            recipient_codec: SCCP_CODEC_TON_RAW,
            recipient: b"EQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAM9c".to_vec(),
            route_id_codec: SCCP_CODEC_TEXT_UTF8,
            route_id: b"nexus:ton:xor".to_vec(),
        });
        assert!(!verify_sccp_payload_structure(
            &invalid_ton_recipient_payload
        ));

        for non_canonical_ton in [
            b"+0:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".as_slice(),
            b"00:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".as_slice(),
            b"0:0123456789ABCDEF0123456789abcdef0123456789abcdef0123456789abcdef".as_slice(),
        ] {
            assert!(!validate_ton_raw_codec(non_canonical_ton));
        }

        let invalid_tron_recipient_payload = SccpPayloadV1::Transfer(TransferPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_TRON,
            nonce: 4,
            asset_home_domain: SCCP_DOMAIN_SORA,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#universal".to_vec(),
            amount: 10,
            sender_codec: SCCP_CODEC_TEXT_UTF8,
            sender: b"bridge@sora".to_vec(),
            recipient_codec: SCCP_CODEC_TRON_BASE58CHECK,
            recipient: b"0x3333333333333333333333333333333333333333".to_vec(),
            route_id_codec: SCCP_CODEC_TEXT_UTF8,
            route_id: b"nexus:tron:xor".to_vec(),
        });
        assert!(!verify_sccp_payload_structure(
            &invalid_tron_recipient_payload
        ));

        let wrong_sender_codec = SccpPayloadV1::Transfer(TransferPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 5,
            asset_home_domain: SCCP_DOMAIN_ETH,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#eth".to_vec(),
            amount: 10,
            sender_codec: SCCP_CODEC_TEXT_UTF8,
            sender: b"0x3333333333333333333333333333333333333333".to_vec(),
            recipient_codec: SCCP_CODEC_TEXT_UTF8,
            recipient: b"alice@sora".to_vec(),
            route_id_codec: SCCP_CODEC_TEXT_UTF8,
            route_id: b"eth:sora:xor".to_vec(),
        });
        assert!(!verify_sccp_payload_structure(&wrong_sender_codec));

        let wrong_recipient_codec = SccpPayloadV1::Transfer(TransferPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_ETH,
            nonce: 6,
            asset_home_domain: SCCP_DOMAIN_SORA,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#universal".to_vec(),
            amount: 10,
            sender_codec: SCCP_CODEC_TEXT_UTF8,
            sender: b"bridge@sora".to_vec(),
            recipient_codec: SCCP_CODEC_TEXT_UTF8,
            recipient: b"0x3333333333333333333333333333333333333333".to_vec(),
            route_id_codec: SCCP_CODEC_TEXT_UTF8,
            route_id: b"sora:eth:xor".to_vec(),
        });
        assert!(!verify_sccp_payload_structure(&wrong_recipient_codec));

        assert!(!verify_message_bundle_structure(&sample_message_bundle(
            wrong_recipient_codec
        )));
    }

    #[test]
    #[allow(clippy::similar_names)]
    fn normalized_codec_value_decodes_chain_specific_addresses() {
        assert_eq!(
            decode_sccp_normalized_codec_value(
                SCCP_CODEC_EVM_HEX,
                b"0x3333333333333333333333333333333333333333",
            ),
            Some(SccpNormalizedCodecValueV1::EvmHex { bytes: [0x33; 20] })
        );

        assert_eq!(
            decode_sccp_normalized_codec_value(
                SCCP_CODEC_SOLANA_BASE58,
                b"11111111111111111111111111111111",
            ),
            Some(SccpNormalizedCodecValueV1::SolanaBase58 { bytes: [0u8; 32] })
        );

        let decoded_ton_recipient = decode_sccp_normalized_codec_value(
            SCCP_CODEC_TON_RAW,
            b"0:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        )
        .expect("decode ton");
        match decoded_ton_recipient {
            SccpNormalizedCodecValueV1::TonRaw { workchain, account } => {
                assert_eq!(workchain, 0);
                assert_eq!(
                    account,
                    [
                        0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67,
                        0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef,
                        0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef
                    ]
                );
            }
            other => panic!("unexpected ton codec decode: {other:?}"),
        }

        let decoded_tron_recipient = decode_sccp_normalized_codec_value(
            SCCP_CODEC_TRON_BASE58CHECK,
            b"T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb",
        )
        .expect("decode tron");
        match decoded_tron_recipient {
            SccpNormalizedCodecValueV1::TronBase58Check { payload } => {
                assert_eq!(payload[0], 0x41);
                assert_eq!(&payload[1..], &[0u8; 20]);
            }
            other => panic!("unexpected tron codec decode: {other:?}"),
        }

        assert_eq!(
            decode_sccp_normalized_codec_value(SCCP_CODEC_SORA_ASSET_ID, &[0x77; 32]),
            Some(SccpNormalizedCodecValueV1::SoraAssetId { bytes: [0x77; 32] })
        );
        assert_eq!(
            decode_sccp_normalized_codec_value(SCCP_CODEC_SORA_ASSET_ID, &[0x77; 31]),
            None
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn counterparty_proof_job_builder_refuses_disabled_lane_but_projection_still_decodes() {
        let bundle = sample_message_bundle(SccpPayloadV1::Transfer(TransferPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_TON,
            nonce: 18,
            asset_home_domain: SCCP_DOMAIN_SORA,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#universal".to_vec(),
            amount: 91,
            sender_codec: SCCP_CODEC_TEXT_UTF8,
            sender: b"sora:bridge".to_vec(),
            recipient_codec: SCCP_CODEC_TON_RAW,
            recipient: b"0:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                .to_vec(),
            route_id_codec: SCCP_CODEC_TEXT_UTF8,
            route_id: b"nexus:ton:xor".to_vec(),
        }));
        assert!(build_sccp_counterparty_proof_job_from_bundle(&bundle).is_none());

        match sccp_payload_projection(&bundle.payload).expect("payload projection") {
            SccpPayloadProjectionV1::Transfer(transfer) => {
                assert_eq!(transfer.amount, 91);
                assert_eq!(
                    transfer.asset_id,
                    SccpNormalizedCodecValueV1::TextUtf8 {
                        value: "xor#universal".to_owned()
                    }
                );
                assert_eq!(
                    transfer.sender,
                    SccpNormalizedCodecValueV1::TextUtf8 {
                        value: "sora:bridge".to_owned()
                    }
                );
                match transfer.recipient {
                    SccpNormalizedCodecValueV1::TonRaw { workchain, account } => {
                        assert_eq!(workchain, 0);
                        assert_eq!(
                            account,
                            [
                                0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45,
                                0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab,
                                0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef
                            ]
                        );
                    }
                    other => panic!("unexpected normalized recipient: {other:?}"),
                }
                assert_eq!(
                    transfer.route_id,
                    SccpNormalizedCodecValueV1::TextUtf8 {
                        value: "nexus:ton:xor".to_owned()
                    }
                );
            }
            other => panic!("unexpected job payload projection: {other:?}"),
        }
    }

    #[test]
    fn destination_rollout_json_roundtrip_preserves_verifier_plan() {
        let rollout =
            sccp_destination_rollout_for_domain(SCCP_DOMAIN_TON).expect("ton destination rollout");
        let json_value = norito::json::to_value(&rollout).expect("rollout json value");
        let json = norito::json::to_string(&json_value).expect("rollout json");
        let decoded: SccpDestinationRolloutV1 =
            norito::json::from_str(&json).expect("decoded rollout");

        assert_eq!(
            decoded.verifier_plan,
            SccpDestinationVerifierPlanV1::TonContractNativeRecursive
        );
        assert_eq!(decoded.blockers, rollout.blockers);
        assert_eq!(decoded, rollout);
    }

    #[test]
    fn destination_verifier_plan_json_roundtrip_uses_stable_string_label() {
        let plan = SccpDestinationVerifierPlanV1::TonContractNativeRecursive;
        let json = norito::json::to_json(&plan).expect("plan json");
        let decoded: SccpDestinationVerifierPlanV1 =
            norito::json::from_str(&json).expect("decoded plan");

        assert_eq!(json, "\"TonContractNativeRecursive\"");
        assert_eq!(decoded, plan);
    }

    #[test]
    fn proof_manifest_helpers_cover_all_core_remote_domains() {
        let manifests = sccp_proof_manifests_v1();
        assert_eq!(manifests.len(), SCCP_CORE_REMOTE_DOMAINS.len());
        assert_eq!(
            manifests
                .iter()
                .map(|manifest| manifest.counterparty_domain)
                .collect::<Vec<_>>(),
            SCCP_CORE_REMOTE_DOMAINS.to_vec()
        );

        let eth = manifests
            .iter()
            .find(|manifest| manifest.counterparty_domain == SCCP_DOMAIN_ETH)
            .expect("eth manifest");
        assert_eq!(eth.message_backend, "sccp/stark-fri-v1/eth");
        assert_eq!(eth.registry_backend, "bridge/sccp/stark-fri-v1/eth");
        assert_eq!(
            eth.verifier_backend.key.as_str(),
            SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1
        );
        assert_eq!(
            eth.verifier_backend.family,
            SccpVerifierBackendFamilyV1::EvmGroth16Bn254
        );
        assert_eq!(
            eth.finality_model,
            SccpProofFinalityModelV1::EthereumBeaconExecution
        );
        assert_eq!(eth.verifier_target, SccpProofVerifierTargetV1::EvmContract);
        assert_eq!(
            eth.required_public_inputs,
            vec![
                "message_id",
                "payload_hash",
                "target_domain",
                "commitment_root",
                "finality_height",
                "finality_block_hash",
            ]
        );
        assert_eq!(eth.submission_template.encoding, "abi_tuple_v1");
        assert_eq!(eth.submission_template.submission_kind, "contract_call");
        assert_eq!(
            eth.submission_template.verifier_entrypoint,
            "submitSccpMessageProof(bytes proof_bytes, bytes32[6] public_inputs, bytes32 statement_hash)"
        );
        assert_eq!(
            eth.submission_template
                .required_arguments
                .iter()
                .map(|argument| argument.key.as_str())
                .collect::<Vec<_>>(),
            vec!["proof_bytes", "public_inputs", "statement_hash"]
        );
        assert!(!eth.production_ready);
        assert_eq!(
            eth.disabled_reason.as_deref(),
            sccp_lane_disabled_reason_for_domain(SCCP_DOMAIN_ETH)
        );
        assert_eq!(
            eth.destination_rollout.verifier_plan,
            SccpDestinationVerifierPlanV1::EvmGroth16Bn254Adapter
        );

        let tron = manifests
            .iter()
            .find(|manifest| manifest.counterparty_domain == SCCP_DOMAIN_TRON)
            .expect("tron manifest");
        assert_eq!(
            tron.verifier_backend.key.as_str(),
            SCCP_TRON_GROTH16_BN254_PROOF_BACKEND_V1
        );
        assert_eq!(
            tron.verifier_backend.family,
            SccpVerifierBackendFamilyV1::TronGroth16Bn254
        );
        assert_eq!(
            tron.verifier_target,
            SccpProofVerifierTargetV1::TronContract
        );
        assert_eq!(tron.submission_template.encoding, "tron_abi_tuple_v1");
        assert_eq!(
            tron.submission_template
                .required_arguments
                .iter()
                .map(|argument| argument.key.as_str())
                .collect::<Vec<_>>(),
            vec!["proof_bytes", "public_inputs", "statement_hash"]
        );
        assert!(!tron.production_ready);
    }

    #[test]
    fn evm_submission_template_matches_in_repo_wrapper_contract_source() {
        const EVM_BRIDGE_SOL: &str =
            include_str!("../../../contracts/evm/sccp/SccpMessageBridge.sol");
        const EVM_VERIFIER_SOL: &str =
            include_str!("../../../contracts/evm/sccp/ISccpMessageVerifier.sol");

        let manifest = sample_reference_evm_attestation_manifest();

        assert_eq!(
            manifest.submission_template.verifier_entrypoint,
            "submitSccpMessageProof(bytes proof_bytes, bytes32[6] public_inputs, bytes32 statement_hash)"
        );
        assert_eq!(
            manifest
                .submission_template
                .required_arguments
                .iter()
                .map(|argument| argument.key.as_str())
                .collect::<Vec<_>>(),
            vec!["proof_bytes", "public_inputs", "statement_hash"]
        );
        assert!(EVM_BRIDGE_SOL.contains("contract SccpMessageBridge {"));
        assert!(EVM_BRIDGE_SOL.contains("function submitSccpMessageProof("));
        assert!(EVM_VERIFIER_SOL.contains("interface ISccpMessageVerifier"));
        assert!(EVM_VERIFIER_SOL.contains("function verifySccpMessageProof("));
    }

    #[test]
    fn evm_destination_binding_uses_lowercase_hex_key_and_expected_hash() {
        let manifest = sample_reference_evm_attestation_manifest();
        let network_id = core::array::from_fn(|idx| u8::try_from(idx).expect("index fits in u8"));
        let verifier_address = core::array::from_fn(|idx| {
            let idx = u8::try_from(idx).expect("index fits in u8");
            0x80u8.saturating_add(idx)
        });
        let bridge_address = core::array::from_fn(|idx| {
            let idx = u8::try_from(idx).expect("index fits in u8");
            0xA0u8.saturating_add(idx)
        });

        let binding = build_sccp_evm_destination_binding(
            &manifest,
            network_id,
            verifier_address,
            bridge_address,
        );
        let verifier_backend_hash = keccak256_bytes(manifest.verifier_backend.key.as_bytes());
        let proof_family_hash = keccak256_bytes(manifest.proof_family.as_bytes());

        assert_eq!(
            binding.key,
            format!(
                "evm:{}:{}:{}:0x{}:0x{}",
                manifest.local_domain,
                manifest.counterparty_domain,
                encode_lower_hex(&network_id),
                encode_lower_hex(&verifier_address),
                encode_lower_hex(&bridge_address)
            )
        );
        assert_eq!(
            binding.binding_hash,
            sccp_evm_destination_binding_hash(
                network_id,
                manifest.local_domain,
                manifest.counterparty_domain,
                verifier_backend_hash,
                proof_family_hash,
                verifier_address,
                bridge_address
            )
        );
    }

    #[test]
    fn evm_destination_binding_rejects_cross_lane_and_contract_replay_by_hash() {
        let eth = sccp_proof_manifest_for_domain(SCCP_DOMAIN_ETH).expect("eth manifest");
        let bsc = sccp_proof_manifest_for_domain(SCCP_DOMAIN_BSC).expect("bsc manifest");
        let network_id = [0x11; 32];
        let verifier_address = [0x22; 20];
        let bridge_address = [0x33; 20];
        let eth_binding =
            build_sccp_evm_destination_binding(&eth, network_id, verifier_address, bridge_address);

        let bsc_binding =
            build_sccp_evm_destination_binding(&bsc, network_id, verifier_address, bridge_address);
        assert_ne!(eth_binding.key, bsc_binding.key);
        assert_ne!(eth_binding.binding_hash, bsc_binding.binding_hash);

        let changed_verifier =
            build_sccp_evm_destination_binding(&eth, network_id, [0x44; 20], bridge_address);
        assert_ne!(eth_binding.binding_hash, changed_verifier.binding_hash);

        let changed_bridge =
            build_sccp_evm_destination_binding(&eth, network_id, verifier_address, [0x55; 20]);
        assert_ne!(eth_binding.binding_hash, changed_bridge.binding_hash);

        let mut forked_manifest = eth.clone();
        forked_manifest.proof_family.push_str("-fork");
        let forked_binding = build_sccp_evm_destination_binding(
            &forked_manifest,
            network_id,
            verifier_address,
            bridge_address,
        );
        assert_ne!(eth_binding.binding_hash, forked_binding.binding_hash);

        let mut backend_fork = eth;
        backend_fork.verifier_backend.key.push_str("-fork");
        let backend_binding = build_sccp_evm_destination_binding(
            &backend_fork,
            network_id,
            verifier_address,
            bridge_address,
        );
        assert_ne!(eth_binding.binding_hash, backend_binding.binding_hash);
    }

    #[test]
    fn evm_submission_package_builder_accepts_explicit_deployment_binding() {
        let bundle = sample_evm_transfer_bundle(58);
        let manifest = sample_reference_evm_attestation_manifest();
        let signer = sample_secp256k1_signer();
        let native_proof_bytes = vec![0xAA, 0xBB, 0xCC];
        let deployment_binding =
            build_sccp_evm_destination_binding(&manifest, [0x11; 32], [0x33; 20], [0x22; 20]);
        let submission_package =
            build_sccp_counterparty_submission_package_with_destination_binding_and_signer(
                &bundle,
                &manifest,
                &native_proof_bytes,
                &deployment_binding,
                &signer,
            )
            .expect("explicit EVM deployment binding builds a submission package");
        let public_inputs =
            sccp_message_transparent_public_inputs(&bundle).expect("message public inputs");
        let proof = NexusSccpMessageTransparentProofV1 {
            version: 1,
            local_domain: manifest.local_domain,
            counterparty_domain: manifest.counterparty_domain,
            security_model: manifest.security_model,
            anchor_governance: manifest.anchor_governance,
            destination_binding: manifest.destination_binding.clone(),
            proof_family: manifest.proof_family.clone(),
            verifier_backend: manifest.verifier_backend.clone(),
            message_backend: manifest.message_backend.clone(),
            registry_backend: manifest.registry_backend.clone(),
            manifest_seed: manifest.manifest_seed.clone(),
            finality_model: manifest.finality_model,
            verifier_target: manifest.verifier_target,
            public_inputs,
            proof_bytes: native_proof_bytes,
            submission_package,
            bundle,
        };

        let payload = evm_payload_from_proof(&proof);
        assert_eq!(payload.destination_binding, deployment_binding);
        assert_eq!(proof.destination_binding, manifest.destination_binding);
        assert!(verify_sccp_evm_submission_package(&manifest, &proof));
    }

    #[test]
    fn production_evm_manifest_does_not_build_reference_attestation_package() {
        let bundle = sample_evm_transfer_bundle(580);
        let mut manifest = sccp_proof_manifest_for_domain(SCCP_DOMAIN_ETH).expect("eth manifest");
        manifest.production_ready = true;
        manifest.disabled_reason = None;
        assert_eq!(
            manifest.verifier_backend.family,
            SccpVerifierBackendFamilyV1::EvmGroth16Bn254
        );
        let signer = sample_secp256k1_signer();
        let deployment_binding =
            build_sccp_evm_destination_binding(&manifest, [0x11; 32], [0x33; 20], [0x22; 20]);

        assert!(
            build_sccp_counterparty_submission_package_with_destination_binding_and_signer(
                &bundle,
                &manifest,
                &[0xAA, 0xBB],
                &deployment_binding,
                &signer,
            )
            .is_none()
        );
    }

    #[test]
    fn production_evm_manifest_builds_signer_free_groth16_submission_package() {
        let bundle = sample_evm_transfer_bundle(581);
        let manifest = sccp_proof_manifest_for_domain(SCCP_DOMAIN_ETH).expect("eth manifest");
        let public_inputs =
            sccp_message_transparent_public_inputs(&bundle).expect("message public inputs");
        let groth16_proof_bytes =
            sample_evm_groth16_proof_bytes(&public_inputs, manifest.local_domain);
        let deployment_binding =
            build_sccp_evm_destination_binding(&manifest, [0x11; 32], [0x33; 20], [0x22; 20]);

        let package =
            build_sccp_counterparty_submission_package_with_destination_binding_allow_unready(
                &bundle,
                &manifest,
                &groth16_proof_bytes,
                &deployment_binding,
                true,
            )
            .expect("groth16 package");

        let SccpPlatformSubmissionPayloadV1::EvmGroth16ContractCall(payload) =
            &package.platform_payload
        else {
            panic!("expected signer-free groth16 EVM payload");
        };
        assert_eq!(payload.proof_bytes, groth16_proof_bytes);
        assert_eq!(
            payload.public_inputs,
            sccp_evm_public_input_word_struct(&public_inputs)
        );
        assert_eq!(payload.destination_binding, deployment_binding);
        assert_eq!(
            package
                .arguments
                .iter()
                .map(|argument| argument.key.as_str())
                .collect::<Vec<_>>(),
            vec!["proof_bytes", "public_inputs", "statement_hash"]
        );
        assert_eq!(package.arguments[0].bytes, payload.proof_bytes);
        assert_eq!(package.arguments[1].encoding, "abi_bytes32x6");
        assert_eq!(package.arguments[2].bytes, payload.statement_hash);

        let proof =
            build_nexus_sccp_message_transparent_proof_with_evm_groth16_proof_and_destination_binding_allow_unready(
                &bundle,
                &groth16_proof_bytes,
                &deployment_binding,
                true,
            )
            .expect("groth16 transparent proof");
        assert_eq!(proof.proof_bytes, groth16_proof_bytes);
        assert_eq!(proof.submission_package, package);
        assert!(verify_nexus_sccp_message_transparent_proof_structure_allow_unready(&proof, true));
    }

    #[test]
    fn production_evm_manifest_builds_signer_free_groth16_proof_job() {
        let bundle = sample_evm_transfer_bundle(583);
        let manifest = sccp_proof_manifest_for_domain(SCCP_DOMAIN_ETH).expect("eth manifest");
        let public_inputs =
            sccp_message_transparent_public_inputs(&bundle).expect("message public inputs");
        let groth16_proof_bytes =
            sample_evm_groth16_proof_bytes(&public_inputs, manifest.local_domain);
        let deployment_binding =
            build_sccp_evm_destination_binding(&manifest, [0x21; 32], [0x43; 20], [0x65; 20]);

        assert!(
            build_sccp_counterparty_proof_job_from_bundle_allow_unready(&bundle, true).is_none(),
            "generic job builder must not package native FastPQ bytes as an EVM Groth16 proof"
        );

        let job =
            build_sccp_counterparty_proof_job_from_bundle_with_evm_groth16_proof_and_destination_binding_allow_unready(
                &bundle,
                &groth16_proof_bytes,
                &deployment_binding,
                true,
            )
            .expect("groth16 proof job");
        assert_eq!(job.counterparty_domain, SCCP_DOMAIN_ETH);
        assert_eq!(
            job.verifier_backend.family,
            SccpVerifierBackendFamilyV1::EvmGroth16Bn254
        );
        assert_eq!(job.public_inputs, public_inputs);
        assert_eq!(job.submission_template, manifest.submission_template);

        let SccpPlatformSubmissionPayloadV1::EvmGroth16ContractCall(payload) =
            &job.submission_package.platform_payload
        else {
            panic!("expected signer-free groth16 payload");
        };
        assert_eq!(payload.proof_bytes, groth16_proof_bytes);
        assert_eq!(payload.destination_binding, deployment_binding);
        assert_eq!(
            job.submission_package
                .arguments
                .iter()
                .map(|argument| argument.key.as_str())
                .collect::<Vec<_>>(),
            vec!["proof_bytes", "public_inputs", "statement_hash"]
        );

        let artifact =
            build_nexus_sccp_message_transparent_proof_with_evm_groth16_proof_and_destination_binding_allow_unready(
                &bundle,
                &groth16_proof_bytes,
                &deployment_binding,
                true,
            )
            .expect("groth16 transparent proof");
        assert!(
            build_sccp_counterparty_proof_job_from_artifact(&artifact).is_none(),
            "strict artifact-to-job conversion must keep disabled production lanes closed"
        );
        let job_from_artifact =
            build_sccp_counterparty_proof_job_from_artifact_allow_unready(&artifact, true)
                .expect("allow-unready artifact-to-job conversion");
        assert_eq!(job_from_artifact, job);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn production_evm_groth16_submission_rejects_adversarial_abi_edges() {
        let bundle = sample_evm_transfer_bundle(582);
        let manifest = sccp_proof_manifest_for_domain(SCCP_DOMAIN_ETH).expect("eth manifest");
        let reference_manifest = sample_reference_evm_attestation_manifest();
        let public_inputs =
            sccp_message_transparent_public_inputs(&bundle).expect("message public inputs");
        let deployment_binding =
            build_sccp_evm_destination_binding(&manifest, [0x11; 32], [0x33; 20], [0x22; 20]);
        let signer = sample_secp256k1_signer();
        let valid_proof = sample_evm_groth16_proof(&public_inputs, manifest.local_domain);
        let valid_bytes = encode_sccp_evm_groth16_bn254_proof_bytes(&valid_proof);

        assert!(
            build_sccp_counterparty_proof_job_from_bundle_with_evm_groth16_proof_and_destination_binding(
                &bundle,
                &valid_bytes,
                &deployment_binding,
            )
            .is_none(),
            "strict Groth16 job builder must keep disabled production lanes closed"
        );

        assert!(
            build_sccp_counterparty_submission_package_with_destination_binding_and_signer_allow_unready(
                &bundle,
                &manifest,
                &valid_bytes,
                &deployment_binding,
                &signer,
                true,
            )
            .is_none(),
            "production Groth16 package must not accept a signer path"
        );

        assert!(
            build_sccp_counterparty_submission_package_with_destination_binding_allow_unready(
                &bundle,
                &reference_manifest,
                &valid_bytes,
                &deployment_binding,
                true,
            )
            .is_none(),
            "reference secp256k1 manifest must not accept Groth16 proof bytes"
        );

        let mut wrong_version = valid_proof.clone();
        wrong_version.version = 2;
        assert!(
            build_sccp_counterparty_submission_package_with_destination_binding_allow_unready(
                &bundle,
                &manifest,
                &encode_sccp_evm_groth16_bn254_proof_bytes(&wrong_version),
                &deployment_binding,
                true,
            )
            .is_none()
        );

        let mut wrong_message = valid_proof.clone();
        wrong_message.message_id[0] ^= 0x01;
        assert!(
            build_sccp_counterparty_submission_package_with_destination_binding_allow_unready(
                &bundle,
                &manifest,
                &encode_sccp_evm_groth16_bn254_proof_bytes(&wrong_message),
                &deployment_binding,
                true,
            )
            .is_none()
        );
        assert!(
            build_sccp_counterparty_proof_job_from_bundle_with_evm_groth16_proof_and_destination_binding_allow_unready(
                &bundle,
                &encode_sccp_evm_groth16_bn254_proof_bytes(&wrong_message),
                &deployment_binding,
                true,
            )
            .is_none()
        );

        let mut wrong_source = valid_proof.clone();
        wrong_source.source_domain = SCCP_DOMAIN_BSC;
        assert!(
            build_sccp_counterparty_submission_package_with_destination_binding_allow_unready(
                &bundle,
                &manifest,
                &encode_sccp_evm_groth16_bn254_proof_bytes(&wrong_source),
                &deployment_binding,
                true,
            )
            .is_none()
        );

        let mut wrong_root = valid_proof.clone();
        wrong_root.commitment_root[0] ^= 0x01;
        assert!(
            build_sccp_counterparty_submission_package_with_destination_binding_allow_unready(
                &bundle,
                &manifest,
                &encode_sccp_evm_groth16_bn254_proof_bytes(&wrong_root),
                &deployment_binding,
                true,
            )
            .is_none()
        );

        let mut zero_g1 = valid_proof.clone();
        zero_g1.a = [[0u8; 32]; 2];
        assert!(
            decode_sccp_evm_groth16_bn254_proof_bytes(&encode_sccp_evm_groth16_bn254_proof_bytes(
                &zero_g1
            ))
            .is_none()
        );

        let mut zero_g2 = valid_proof.clone();
        zero_g2.b = [[0u8; 32]; 4];
        assert!(
            decode_sccp_evm_groth16_bn254_proof_bytes(&encode_sccp_evm_groth16_bn254_proof_bytes(
                &zero_g2
            ))
            .is_none()
        );

        let mut out_of_field = valid_proof.clone();
        out_of_field.c[0] = BN254_BASE_FIELD_MODULUS_BE;
        assert!(
            decode_sccp_evm_groth16_bn254_proof_bytes(&encode_sccp_evm_groth16_bn254_proof_bytes(
                &out_of_field
            ))
            .is_none()
        );

        let mut malformed = valid_bytes.clone();
        malformed.pop();
        assert!(
            build_sccp_counterparty_submission_package_with_destination_binding_allow_unready(
                &bundle,
                &manifest,
                &malformed,
                &deployment_binding,
                true,
            )
            .is_none()
        );

        let mut overflow_source_domain = valid_bytes;
        overflow_source_domain[64] = 0x01;
        assert!(decode_sccp_evm_groth16_bn254_proof_bytes(&overflow_source_domain).is_none());

        let mut binding_version_replay =
            build_nexus_sccp_message_transparent_proof_with_evm_groth16_proof_and_destination_binding_allow_unready(
                &bundle,
                &encode_sccp_evm_groth16_bn254_proof_bytes(&valid_proof),
                &deployment_binding,
                true,
            )
            .expect("valid groth16 proof artifact");
        match &mut binding_version_replay.submission_package.platform_payload {
            SccpPlatformSubmissionPayloadV1::EvmGroth16ContractCall(payload) => {
                payload.destination_binding.version = 2;
            }
            _ => panic!("expected groth16 payload"),
        }
        assert!(
            !verify_nexus_sccp_message_transparent_proof_structure_allow_unready(
                &binding_version_replay,
                true
            )
        );

        let mut zero_binding_hash = binding_version_replay;
        match &mut zero_binding_hash.submission_package.platform_payload {
            SccpPlatformSubmissionPayloadV1::EvmGroth16ContractCall(payload) => {
                payload.destination_binding.version = 1;
                payload.destination_binding.binding_hash = [0u8; 32];
            }
            _ => panic!("expected groth16 payload"),
        }
        assert!(
            !verify_nexus_sccp_message_transparent_proof_structure_allow_unready(
                &zero_binding_hash,
                true
            )
        );
    }

    #[test]
    fn production_tron_manifest_builds_signer_free_groth16_submission_package() {
        let bundle = sample_tron_transfer_bundle(584);
        let manifest = sccp_proof_manifest_for_domain(SCCP_DOMAIN_TRON).expect("tron manifest");
        let public_inputs =
            sccp_message_transparent_public_inputs(&bundle).expect("message public inputs");
        let groth16_proof_bytes =
            sample_evm_groth16_proof_bytes(&public_inputs, manifest.local_domain);

        assert_eq!(
            manifest.verifier_backend.family,
            SccpVerifierBackendFamilyV1::TronGroth16Bn254
        );
        assert_eq!(
            manifest.verifier_backend.key,
            SCCP_TRON_GROTH16_BN254_PROOF_BACKEND_V1
        );
        assert!(
            build_sccp_counterparty_proof_job_from_bundle_allow_unready(&bundle, true).is_none(),
            "generic job builder must not package native FastPQ bytes as a TRON Groth16 proof"
        );

        let package =
            build_sccp_counterparty_submission_package_with_destination_binding_allow_unready(
                &bundle,
                &manifest,
                &groth16_proof_bytes,
                &manifest.destination_binding,
                true,
            )
            .expect("tron groth16 package");

        let SccpPlatformSubmissionPayloadV1::TronContractCall(payload) = &package.platform_payload
        else {
            panic!("expected TRON contract call payload");
        };
        assert_eq!(payload.proof_bytes, groth16_proof_bytes);
        assert_eq!(
            payload.public_inputs,
            sccp_evm_public_input_word_struct(&public_inputs)
        );
        assert_eq!(payload.destination_binding, manifest.destination_binding);
        assert_eq!(
            package
                .arguments
                .iter()
                .map(|argument| argument.key.as_str())
                .collect::<Vec<_>>(),
            vec!["proof_bytes", "public_inputs", "statement_hash"]
        );
        assert_eq!(package.arguments[0].bytes, payload.proof_bytes);
        assert_eq!(package.arguments[1].encoding, "abi_bytes32x6");
        assert_eq!(package.arguments[2].bytes, payload.statement_hash);

        let proof =
            build_nexus_sccp_message_transparent_proof_with_tron_groth16_proof_allow_unready(
                &bundle,
                &groth16_proof_bytes,
                true,
            )
            .expect("tron groth16 transparent proof");
        assert_eq!(proof.proof_bytes, groth16_proof_bytes);
        assert_eq!(proof.submission_package, package);
        assert!(verify_nexus_sccp_message_transparent_proof_structure_allow_unready(&proof, true));
        assert!(
            build_sccp_counterparty_proof_job_from_artifact(&proof).is_none(),
            "strict artifact-to-job conversion must keep disabled TRON lanes closed"
        );

        let job =
            build_sccp_counterparty_proof_job_from_bundle_with_tron_groth16_proof_allow_unready(
                &bundle,
                &groth16_proof_bytes,
                true,
            )
            .expect("tron groth16 proof job");
        assert_eq!(job.counterparty_domain, SCCP_DOMAIN_TRON);
        assert_eq!(
            job.verifier_backend.family,
            SccpVerifierBackendFamilyV1::TronGroth16Bn254
        );
        assert_eq!(job.submission_package, proof.submission_package);
        assert_eq!(
            build_sccp_counterparty_proof_job_from_artifact_allow_unready(&proof, true)
                .expect("allow-unready artifact-to-job conversion"),
            job
        );
    }

    #[test]
    fn production_tron_groth16_submission_rejects_adversarial_abi_edges() {
        let bundle = sample_tron_transfer_bundle(585);
        let manifest = sccp_proof_manifest_for_domain(SCCP_DOMAIN_TRON).expect("tron manifest");
        let public_inputs =
            sccp_message_transparent_public_inputs(&bundle).expect("message public inputs");
        let signer = sample_secp256k1_signer();
        let valid_proof = sample_evm_groth16_proof(&public_inputs, manifest.local_domain);
        let valid_bytes = encode_sccp_evm_groth16_bn254_proof_bytes(&valid_proof);

        assert!(
            build_sccp_counterparty_proof_job_from_bundle_with_tron_groth16_proof(
                &bundle,
                &valid_bytes,
            )
            .is_none(),
            "strict TRON Groth16 job builder must keep disabled production lanes closed"
        );
        assert!(
            build_sccp_counterparty_submission_package_with_destination_binding_and_signer_allow_unready(
                &bundle,
                &manifest,
                &valid_bytes,
                &manifest.destination_binding,
                &signer,
                true,
            )
            .is_none(),
            "TRON Groth16 packages must not accept a signer path"
        );
        assert!(
            build_sccp_counterparty_proof_job_from_bundle_with_evm_groth16_proof_and_destination_binding_allow_unready(
                &bundle,
                &valid_bytes,
                &manifest.destination_binding,
                true,
            )
            .is_none(),
            "EVM Groth16 builder must not package TRON lanes"
        );

        let mut wrong_message = valid_proof.clone();
        wrong_message.message_id[0] ^= 0x01;
        assert!(
            build_sccp_counterparty_submission_package_with_destination_binding_allow_unready(
                &bundle,
                &manifest,
                &encode_sccp_evm_groth16_bn254_proof_bytes(&wrong_message),
                &manifest.destination_binding,
                true,
            )
            .is_none()
        );

        let mut wrong_source = valid_proof.clone();
        wrong_source.source_domain = SCCP_DOMAIN_TRON;
        assert!(
            build_nexus_sccp_message_transparent_proof_with_tron_groth16_proof_allow_unready(
                &bundle,
                &encode_sccp_evm_groth16_bn254_proof_bytes(&wrong_source),
                true,
            )
            .is_none()
        );

        let mut wrong_root = valid_proof.clone();
        wrong_root.commitment_root[0] ^= 0x01;
        assert!(
            build_sccp_counterparty_proof_job_from_bundle_with_tron_groth16_proof_allow_unready(
                &bundle,
                &encode_sccp_evm_groth16_bn254_proof_bytes(&wrong_root),
                true,
            )
            .is_none()
        );

        let mut zero_g1 = valid_proof.clone();
        zero_g1.a = [[0u8; 32]; 2];
        assert!(
            decode_sccp_evm_groth16_bn254_proof_bytes(&encode_sccp_evm_groth16_bn254_proof_bytes(
                &zero_g1
            ))
            .is_none()
        );

        let mut malformed = valid_bytes;
        malformed.pop();
        assert!(
            build_sccp_counterparty_submission_package_with_destination_binding_allow_unready(
                &bundle,
                &manifest,
                &malformed,
                &manifest.destination_binding,
                true,
            )
            .is_none()
        );
    }

    #[test]
    fn evm_submission_package_builder_requires_explicit_deployment_binding_once_enabled() {
        let bundle = sample_evm_transfer_bundle(59);
        let manifest = sample_reference_evm_attestation_manifest();
        let signer = sample_secp256k1_signer();

        assert!(
            build_sccp_counterparty_submission_package_with_signer(
                &bundle,
                &manifest,
                &[0xAA, 0xBB],
                &signer,
            )
            .is_none()
        );
    }

    #[test]
    fn evm_submission_package_builder_rejects_non_secp256k1_attestor() {
        let bundle = sample_evm_transfer_bundle(63);
        let manifest = sample_reference_evm_attestation_manifest();
        let deployment_binding =
            build_sccp_evm_destination_binding(&manifest, [0x11; 32], [0x33; 20], [0x22; 20]);
        let ed25519_signer = iroha_crypto::KeyPair::from_seed(
            b"iroha:sccp:test:wrong-attestor".to_vec(),
            iroha_crypto::Algorithm::Ed25519,
        );

        assert!(
            build_sccp_counterparty_submission_package_with_destination_binding_and_signer(
                &bundle,
                &manifest,
                &[0xAA, 0xBB],
                &deployment_binding,
                &ed25519_signer,
            )
            .is_none()
        );
    }

    #[test]
    fn submission_package_builder_rejects_explicit_evm_binding_on_non_evm_manifest() {
        let bundle = sample_tron_transfer_bundle(60);
        let mut manifest = sccp_proof_manifest_for_domain(SCCP_DOMAIN_TRON).expect("tron manifest");
        manifest.production_ready = true;
        manifest.disabled_reason = None;
        let signer = sample_secp256k1_signer();
        let deployment_binding =
            build_sccp_evm_destination_binding(&manifest, [0x11; 32], [0x33; 20], [0x22; 20]);

        assert!(
            build_sccp_counterparty_submission_package_with_destination_binding_and_signer(
                &bundle,
                &manifest,
                &[0xAA, 0xBB],
                &deployment_binding,
                &signer,
            )
            .is_none()
        );
    }

    #[test]
    fn ton_submission_package_builder_emits_boc_message_body_and_binding() {
        let bundle = sample_transfer_bundle(SCCP_DOMAIN_SORA, SCCP_DOMAIN_TON, 61);
        let manifest = sccp_proof_manifest_for_domain(SCCP_DOMAIN_TON).expect("ton manifest");
        let proof_bytes = (0u8..=150).collect::<Vec<_>>();

        let package = build_sccp_counterparty_submission_package_allow_unready(
            &bundle,
            &manifest,
            &proof_bytes,
            true,
        )
        .expect("ton package");

        assert_eq!(
            manifest.submission_template.encoding,
            "ton_message_body_boc_v1"
        );
        assert_eq!(package.envelope_encoding, "ton_message_body_boc_v1");
        assert_eq!(package.arguments.len(), 1);
        assert_eq!(package.arguments[0].key, "message_body_boc");
        assert_eq!(package.arguments[0].encoding, "ton_boc");
        let SccpPlatformSubmissionPayloadV1::TonInternalMessage(payload) =
            &package.platform_payload
        else {
            panic!("expected TON internal message payload");
        };
        assert!(payload.message_body_boc.starts_with(&SCCP_TON_BOC_MAGIC));
        assert_eq!(payload.message_body_boc, package.envelope_bytes);
        assert_eq!(payload.message_body_boc, package.arguments[0].bytes);
        assert_eq!(payload.destination_binding, manifest.destination_binding);
        assert_eq!(
            payload.destination_binding_hash,
            manifest.destination_binding.binding_hash
        );
        assert_eq!(payload.proof_bytes, proof_bytes);
        assert_eq!(
            payload.public_inputs_bytes,
            canonical_sccp_message_transparent_public_inputs_bytes(
                &sccp_message_transparent_public_inputs(&bundle).expect("public inputs"),
            )
        );
        assert_eq!(
            payload.bundle_bytes,
            canonical_nexus_sccp_message_bundle_bytes(&bundle)
        );
        assert!(
            payload
                .message_body_boc
                .windows(32)
                .any(|window| window == payload.statement_hash.as_slice())
        );
        assert!(
            payload
                .message_body_boc
                .windows(32)
                .any(|window| window == payload.destination_binding_hash.as_slice())
        );
    }

    #[test]
    fn evm_submission_package_verifier_rejects_destination_binding_mismatch() {
        let bundle = sample_message_bundle(SccpPayloadV1::Transfer(TransferPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_ETH,
            nonce: 52,
            asset_home_domain: SCCP_DOMAIN_SORA,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#universal".to_vec(),
            amount: 123,
            sender_codec: SCCP_CODEC_TEXT_UTF8,
            sender: b"sora:bridge".to_vec(),
            recipient_codec: SCCP_CODEC_EVM_HEX,
            recipient: b"0x1111111111111111111111111111111111111111".to_vec(),
            route_id_codec: SCCP_CODEC_TEXT_UTF8,
            route_id: b"nexus:eth:xor".to_vec(),
        }));
        let manifest = sample_reference_evm_attestation_manifest();
        let public_inputs =
            sccp_message_transparent_public_inputs(&bundle).expect("message public inputs");
        let inner =
            build_sccp_message_transparent_inner_proof(&bundle, &manifest).expect("inner proof");
        let signer = sample_secp256k1_signer();
        let native_proof_bytes = vec![0xAA, 0xBB, 0xCC];
        let network_id = [0x11; 32];
        let verifier_address = [0x33; 20];
        let bridge_address = [0x22; 20];
        let destination_binding = build_sccp_evm_destination_binding(
            &manifest,
            network_id,
            verifier_address,
            bridge_address,
        );
        let payload = build_sccp_evm_contract_submission_payload(
            &manifest,
            &native_proof_bytes,
            &public_inputs,
            inner.statement_hash,
            &destination_binding,
            &signer,
        )
        .expect("evm payload");
        let mut mismatched_binding = payload.destination_binding.clone();
        mismatched_binding.binding_hash[0] ^= 0xFF;
        let platform_payload =
            SccpPlatformSubmissionPayloadV1::EvmContractCall(SccpEvmContractSubmissionPayloadV1 {
                destination_binding: mismatched_binding,
                ..payload.clone()
            });
        let arguments =
            sccp_submission_argument_values(&manifest.submission_template, &platform_payload)
                .expect("argument values");
        let proof = NexusSccpMessageTransparentProofV1 {
            version: 1,
            local_domain: manifest.local_domain,
            counterparty_domain: manifest.counterparty_domain,
            security_model: manifest.security_model,
            anchor_governance: manifest.anchor_governance,
            destination_binding: manifest.destination_binding.clone(),
            proof_family: manifest.proof_family.clone(),
            verifier_backend: manifest.verifier_backend.clone(),
            message_backend: manifest.message_backend.clone(),
            registry_backend: manifest.registry_backend.clone(),
            manifest_seed: manifest.manifest_seed.clone(),
            finality_model: manifest.finality_model,
            verifier_target: manifest.verifier_target,
            public_inputs,
            proof_bytes: payload.proof_bytes.clone(),
            submission_package: SccpCounterpartySubmissionPackageV1 {
                version: 1,
                proof_family: manifest.proof_family.clone(),
                verifier_backend: manifest.verifier_backend.clone(),
                envelope_encoding: sccp_submission_envelope_encoding(&manifest.submission_template),
                submission_kind: manifest.submission_template.submission_kind.clone(),
                verifier_entrypoint: manifest.submission_template.verifier_entrypoint.clone(),
                platform_payload,
                arguments: arguments.clone(),
                envelope_bytes: encode_sccp_submission_envelope(
                    &manifest.submission_template,
                    &arguments,
                ),
            },
            bundle,
        };

        assert!(!verify_sccp_evm_submission_package(&manifest, &proof));
    }

    #[test]
    fn evm_attestation_envelope_decoder_rejects_malformed_offsets_and_lengths() {
        let (_manifest, proof) = sample_valid_evm_submission_proof(53);
        let payload = evm_payload_from_proof(&proof);
        assert!(decode_sccp_evm_attestation_envelope(&payload.proof_bytes).is_some());

        let mut truncated = payload.proof_bytes.clone();
        truncated.truncate(32 * 7 - 1);
        assert!(decode_sccp_evm_attestation_envelope(&truncated).is_none());

        let mut bad_offset = payload.proof_bytes.clone();
        bad_offset[32 * 7 - 1] = 0xE1;
        assert!(decode_sccp_evm_attestation_envelope(&bad_offset).is_none());

        let mut bad_signature_len = payload.proof_bytes;
        bad_signature_len[32 * 7 + 31] = 64;
        assert!(decode_sccp_evm_attestation_envelope(&bad_signature_len).is_none());
    }

    #[test]
    fn evm_attestation_envelope_decoder_rejects_noncanonical_padding_and_tail() {
        let (_manifest, proof) = sample_valid_evm_submission_proof(68);
        let payload = evm_payload_from_proof(&proof);
        assert!(decode_sccp_evm_attestation_envelope(&payload.proof_bytes).is_some());

        let mut non_zero_padding = payload.proof_bytes.clone();
        let padding_byte = non_zero_padding
            .last_mut()
            .expect("sample signature envelope should include ABI padding");
        *padding_byte = 0x01;
        assert!(decode_sccp_evm_attestation_envelope(&non_zero_padding).is_none());

        let mut trailing_zero_word = payload.proof_bytes;
        trailing_zero_word.extend_from_slice(&[0u8; 32]);
        assert!(decode_sccp_evm_attestation_envelope(&trailing_zero_word).is_none());
    }

    #[test]
    fn evm_attestation_envelope_codec_rejects_version_overread_and_bad_signature_shapes() {
        let (_manifest, proof) = sample_valid_evm_submission_proof(62);
        let payload = evm_payload_from_proof(&proof);

        let mut oversized_version = payload.proof_bytes.clone();
        oversized_version[30] = 1;
        assert!(decode_sccp_evm_attestation_envelope(&oversized_version).is_none());

        let mut declared_overread = payload.proof_bytes.clone();
        declared_overread[32 * 7 + 31] = 130;
        assert!(decode_sccp_evm_attestation_envelope(&declared_overread).is_none());

        let mut bad_signature = payload.attestation.clone();
        bad_signature.signatures[0].signature_bytes.pop();
        assert!(encode_sccp_evm_attestation_envelope(&bad_signature).is_none());
    }

    #[test]
    fn evm_submission_package_verifier_rejects_attestation_signature_replay_edges() {
        let (manifest, proof) = sample_valid_evm_submission_proof(54);
        let valid_payload = evm_payload_from_proof(&proof);

        let mut empty_signatures = proof.clone();
        let mut payload = valid_payload.clone();
        payload.attestation.signatures.clear();
        payload.proof_bytes =
            encode_sccp_evm_attestation_envelope(&payload.attestation).expect("encode envelope");
        replace_evm_payload(&mut empty_signatures, &manifest, payload);
        assert!(!verify_sccp_evm_submission_package(
            &manifest,
            &empty_signatures
        ));

        let mut duplicate_signature = proof.clone();
        let mut payload = valid_payload.clone();
        payload
            .attestation
            .signatures
            .push(payload.attestation.signatures[0].clone());
        payload.proof_bytes =
            encode_sccp_evm_attestation_envelope(&payload.attestation).expect("encode envelope");
        replace_evm_payload(&mut duplicate_signature, &manifest, payload);
        assert!(!verify_sccp_evm_submission_package(
            &manifest,
            &duplicate_signature
        ));

        let mut signer_address_mismatch = proof;
        let mut payload = valid_payload;
        payload.attestation.signatures[0].signer_address[0] ^= 0x01;
        payload.proof_bytes =
            encode_sccp_evm_attestation_envelope(&payload.attestation).expect("encode envelope");
        replace_evm_payload(&mut signer_address_mismatch, &manifest, payload);
        assert!(!verify_sccp_evm_submission_package(
            &manifest,
            &signer_address_mismatch
        ));
    }

    #[test]
    fn evm_submission_package_verifier_rejects_digest_and_envelope_replay_edges() {
        let (manifest, proof) = sample_valid_evm_submission_proof(55);
        let valid_payload = evm_payload_from_proof(&proof);

        let mut native_proof_replay = proof.clone();
        native_proof_replay.proof_bytes.push(0xEE);
        assert!(!verify_sccp_evm_submission_package(
            &manifest,
            &native_proof_replay
        ));

        let mut message_id_replay = proof.clone();
        let mut payload = valid_payload.clone();
        payload.attestation.message_id[0] ^= 0x01;
        payload.proof_bytes =
            encode_sccp_evm_attestation_envelope(&payload.attestation).expect("encode envelope");
        replace_evm_payload(&mut message_id_replay, &manifest, payload);
        assert!(!verify_sccp_evm_submission_package(
            &manifest,
            &message_id_replay
        ));

        let mut public_input_hash_replay = proof.clone();
        let mut payload = valid_payload.clone();
        payload.public_inputs_hash[0] ^= 0x01;
        replace_evm_payload(&mut public_input_hash_replay, &manifest, payload);
        assert!(!verify_sccp_evm_submission_package(
            &manifest,
            &public_input_hash_replay
        ));

        let mut noncanonical_envelope = proof;
        let mut payload = valid_payload;
        payload.proof_bytes.push(0x00);
        replace_evm_payload(&mut noncanonical_envelope, &manifest, payload);
        assert!(!verify_sccp_evm_submission_package(
            &manifest,
            &noncanonical_envelope
        ));
    }

    #[test]
    fn evm_submission_package_verifier_rejects_public_input_and_binding_replays() {
        let (manifest, proof) = sample_valid_evm_submission_proof(64);
        let valid_payload = evm_payload_from_proof(&proof);

        let mut public_input_words_replay = proof.clone();
        let mut payload = valid_payload.clone();
        payload.public_inputs.target_domain_word[31] ^= 0x01;
        replace_evm_payload(&mut public_input_words_replay, &manifest, payload);
        assert!(!verify_sccp_evm_submission_package(
            &manifest,
            &public_input_words_replay
        ));

        let mut destination_hash_replay = proof;
        let mut payload = valid_payload;
        payload.attestation.destination_binding_hash[0] ^= 0x01;
        payload.proof_bytes =
            encode_sccp_evm_attestation_envelope(&payload.attestation).expect("encode envelope");
        replace_evm_payload(&mut destination_hash_replay, &manifest, payload);
        assert!(!verify_sccp_evm_submission_package(
            &manifest,
            &destination_hash_replay
        ));
    }

    #[test]
    fn evm_submission_package_verifier_rejects_decoded_envelope_only_replays() {
        let (manifest, proof) = sample_valid_evm_submission_proof(66);
        let valid_payload = evm_payload_from_proof(&proof);

        let mut source_domain_replay = proof.clone();
        let mut payload = valid_payload;
        let mut envelope = payload.attestation.clone();
        envelope.source_domain = SCCP_DOMAIN_BSC;
        payload.proof_bytes =
            encode_sccp_evm_attestation_envelope(&envelope).expect("encode envelope");
        replace_evm_payload(&mut source_domain_replay, &manifest, payload);
        assert!(!verify_sccp_evm_submission_package(
            &manifest,
            &source_domain_replay
        ));

        let mut native_proof_hash_replay = proof;
        let mut payload = evm_payload_from_proof(&native_proof_hash_replay);
        let mut envelope = payload.attestation.clone();
        envelope.native_proof_hash[0] ^= 0x01;
        payload.proof_bytes =
            encode_sccp_evm_attestation_envelope(&envelope).expect("encode envelope");
        replace_evm_payload(&mut native_proof_hash_replay, &manifest, payload);
        assert!(!verify_sccp_evm_submission_package(
            &manifest,
            &native_proof_hash_replay
        ));
    }

    #[test]
    fn evm_submission_package_verifier_rejects_top_level_and_bundle_replays() {
        let (manifest, proof) = sample_valid_evm_submission_proof(67);

        let mut public_input_replay = proof.clone();
        public_input_replay.public_inputs.message_id[0] ^= 0x01;
        assert!(!verify_sccp_evm_submission_package(
            &manifest,
            &public_input_replay
        ));

        let mut bundle_payload_replay = proof.clone();
        match &mut bundle_payload_replay.bundle.payload {
            SccpPayloadV1::Transfer(payload) => payload.amount += 1,
            _ => panic!("sample proof must use transfer payload"),
        }
        assert!(!verify_sccp_evm_submission_package(
            &manifest,
            &bundle_payload_replay
        ));

        let mut finality_replay = proof;
        finality_replay.bundle.finality_proof = sample_finality_proof([0xCC; 32]);
        assert!(!verify_sccp_evm_submission_package(
            &manifest,
            &finality_replay
        ));
    }

    #[test]
    fn evm_submission_package_verifier_rejects_platform_and_envelope_substitutions() {
        let (manifest, proof) = sample_valid_evm_submission_proof(61);
        let valid_payload = evm_payload_from_proof(&proof);

        let mut platform_swap = proof.clone();
        platform_swap.submission_package.platform_payload =
            SccpPlatformSubmissionPayloadV1::TronContractCall(
                SccpTronContractSubmissionPayloadV1 {
                    proof_bytes: valid_payload.proof_bytes.clone(),
                    public_inputs: valid_payload.public_inputs.clone(),
                    statement_hash: valid_payload.statement_hash,
                    destination_binding: manifest.destination_binding.clone(),
                },
            );
        assert!(!verify_sccp_evm_submission_package(
            &manifest,
            &platform_swap
        ));

        let mut decoded_envelope_divergence = proof.clone();
        let mut payload = valid_payload.clone();
        let mut decoded_only = payload.attestation.clone();
        decoded_only.signatures.clear();
        payload.proof_bytes =
            encode_sccp_evm_attestation_envelope(&decoded_only).expect("encode envelope");
        replace_evm_payload(&mut decoded_envelope_divergence, &manifest, payload);
        assert!(!verify_sccp_evm_submission_package(
            &manifest,
            &decoded_envelope_divergence
        ));

        let mut empty_binding_key = proof;
        let mut payload = valid_payload;
        payload.destination_binding.key.clear();
        replace_evm_payload(&mut empty_binding_key, &manifest, payload);
        assert!(!verify_sccp_evm_submission_package(
            &manifest,
            &empty_binding_key
        ));
    }

    #[test]
    fn evm_submission_package_verifier_rejects_metadata_tampering() {
        let (manifest, proof) = sample_valid_evm_submission_proof(56);

        let mut version = proof.clone();
        version.submission_package.version = 2;
        assert!(!verify_sccp_evm_submission_package(&manifest, &version));

        let mut proof_family = proof.clone();
        proof_family
            .submission_package
            .proof_family
            .push_str("-fork");
        assert!(!verify_sccp_evm_submission_package(
            &manifest,
            &proof_family
        ));

        let mut verifier_backend = proof.clone();
        verifier_backend.submission_package.verifier_backend.key = "evm/fork".to_owned();
        assert!(!verify_sccp_evm_submission_package(
            &manifest,
            &verifier_backend
        ));

        let mut envelope_encoding = proof.clone();
        envelope_encoding.submission_package.envelope_encoding = "abi_tuple_v2".to_owned();
        assert!(!verify_sccp_evm_submission_package(
            &manifest,
            &envelope_encoding
        ));

        let mut submission_kind = proof.clone();
        submission_kind.submission_package.submission_kind = "delegate_call".to_owned();
        assert!(!verify_sccp_evm_submission_package(
            &manifest,
            &submission_kind
        ));

        let mut verifier_entrypoint = proof;
        verifier_entrypoint
            .submission_package
            .verifier_entrypoint
            .push_str("Fork");
        assert!(!verify_sccp_evm_submission_package(
            &manifest,
            &verifier_entrypoint
        ));
    }

    #[test]
    fn evm_submission_package_verifier_rejects_argument_and_envelope_tampering() {
        let (manifest, proof) = sample_valid_evm_submission_proof(57);

        let mut mutated_argument_bytes = proof.clone();
        mutated_argument_bytes.submission_package.arguments[0].bytes[0] ^= 0x01;
        assert!(!verify_sccp_evm_submission_package(
            &manifest,
            &mutated_argument_bytes
        ));

        let mut missing_argument = proof.clone();
        missing_argument.submission_package.arguments.pop();
        missing_argument.submission_package.envelope_bytes = encode_sccp_submission_envelope(
            &manifest.submission_template,
            &missing_argument.submission_package.arguments,
        );
        assert!(!verify_sccp_evm_submission_package(
            &manifest,
            &missing_argument
        ));

        let mut duplicate_argument = proof.clone();
        duplicate_argument
            .submission_package
            .arguments
            .push(duplicate_argument.submission_package.arguments[0].clone());
        duplicate_argument.submission_package.envelope_bytes = encode_sccp_submission_envelope(
            &manifest.submission_template,
            &duplicate_argument.submission_package.arguments,
        );
        assert!(!verify_sccp_evm_submission_package(
            &manifest,
            &duplicate_argument
        ));

        let mut envelope_bytes = proof;
        envelope_bytes.submission_package.envelope_bytes[0] ^= 0x01;
        assert!(!verify_sccp_evm_submission_package(
            &manifest,
            &envelope_bytes
        ));
    }

    #[test]
    fn evm_submission_package_builder_refuses_disabled_lane() {
        let bundle = sample_message_bundle(SccpPayloadV1::Transfer(TransferPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_ETH,
            nonce: 27,
            asset_home_domain: SCCP_DOMAIN_SORA,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#universal".to_vec(),
            amount: 123,
            sender_codec: SCCP_CODEC_TEXT_UTF8,
            sender: b"sora:bridge".to_vec(),
            recipient_codec: SCCP_CODEC_EVM_HEX,
            recipient: b"0x1111111111111111111111111111111111111111".to_vec(),
            route_id_codec: SCCP_CODEC_TEXT_UTF8,
            route_id: b"nexus:eth:xor".to_vec(),
        }));

        let signer = sample_secp256k1_signer();
        let manifest = sccp_proof_manifest_for_domain(SCCP_DOMAIN_ETH).expect("eth manifest");
        assert!(
            build_sccp_counterparty_submission_package_with_signer(
                &bundle,
                &manifest,
                &[0xAA, 0xBB],
                &signer,
            )
            .is_none()
        );
    }

    #[test]
    fn evm_manifest_keeps_reference_wrapper_shape_but_marks_lane_disabled() {
        let manifest = sccp_proof_manifest_for_domain(SCCP_DOMAIN_ETH).expect("eth manifest");
        assert_eq!(manifest.submission_template.encoding, "abi_tuple_v1");
        assert_eq!(
            manifest.submission_template.submission_kind,
            "contract_call"
        );
        assert!(!manifest.production_ready);
        assert_eq!(
            manifest.disabled_reason.as_deref(),
            sccp_lane_disabled_reason_for_domain(SCCP_DOMAIN_ETH)
        );
        assert_eq!(
            manifest.destination_rollout.verifier_plan,
            SccpDestinationVerifierPlanV1::EvmGroth16Bn254Adapter
        );
    }

    #[test]
    fn tron_submission_package_builder_refuses_disabled_lane() {
        let bundle = sample_message_bundle(SccpPayloadV1::Transfer(TransferPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_TRON,
            nonce: 28,
            asset_home_domain: SCCP_DOMAIN_SORA,
            asset_id_codec: SCCP_CODEC_TEXT_UTF8,
            asset_id: b"xor#universal".to_vec(),
            amount: 123,
            sender_codec: SCCP_CODEC_TEXT_UTF8,
            sender: b"sora:bridge".to_vec(),
            recipient_codec: SCCP_CODEC_TRON_BASE58CHECK,
            recipient: b"TJRabPrwbZy45sbavfcjinPJC18kjpRTv8".to_vec(),
            route_id_codec: SCCP_CODEC_TEXT_UTF8,
            route_id: b"nexus:tron:xor".to_vec(),
        }));

        let manifest = sccp_proof_manifest_for_domain(SCCP_DOMAIN_TRON).expect("tron manifest");
        assert!(build_sccp_counterparty_submission_package(&bundle, &manifest, &[0xAA]).is_none());
    }

    #[test]
    fn bsc_shares_the_same_evm_wrapper_submission_contract_as_eth() {
        let eth = sccp_proof_manifest_for_domain(SCCP_DOMAIN_ETH).expect("eth manifest");
        let bsc = sccp_proof_manifest_for_domain(SCCP_DOMAIN_BSC).expect("bsc manifest");

        assert_eq!(
            eth.submission_template.verifier_entrypoint,
            bsc.submission_template.verifier_entrypoint
        );
        assert_eq!(
            eth.submission_template.encoding,
            bsc.submission_template.encoding
        );
        assert_eq!(
            eth.submission_template.submission_kind,
            bsc.submission_template.submission_kind
        );
        assert_eq!(
            eth.submission_template.required_arguments,
            bsc.submission_template.required_arguments
        );
        assert_eq!(eth.verifier_backend.key, bsc.verifier_backend.key);
    }

    #[test]
    fn proof_manifest_for_ton_uses_ton_codec_and_seed() {
        let manifest = sccp_proof_manifest_for_domain(SCCP_DOMAIN_TON).expect("ton manifest");
        assert_eq!(manifest.chain, "ton");
        assert_eq!(manifest.counterparty_account_codec, SCCP_CODEC_TON_RAW);
        assert_eq!(manifest.counterparty_account_codec_key, "ton_raw");
        assert_eq!(
            manifest.manifest_seed,
            "iroha:sccp:bridge-proof:message:stark-fri:v1:ton"
        );
        assert_eq!(manifest.verifier_backend.key.as_str(), "ton-contract-v1");
        assert_eq!(
            manifest.finality_model,
            SccpProofFinalityModelV1::TonMasterchain
        );
        assert_eq!(
            manifest.verifier_target,
            SccpProofVerifierTargetV1::TonContract
        );
        assert_eq!(
            manifest.submission_template.encoding,
            "ton_message_body_boc_v1"
        );
        assert_eq!(
            manifest.submission_template.verifier_entrypoint,
            "op::submit_sccp_message_proof"
        );
    }
}
