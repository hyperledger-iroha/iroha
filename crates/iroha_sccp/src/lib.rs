#![cfg_attr(not(feature = "std"), no_std)]
#![allow(missing_docs)]
#![allow(missing_copy_implementations)]

extern crate alloc;

use alloc::{borrow::ToOwned, string::String, vec::Vec};

use blake2::{
    Blake2bVar,
    digest::{Update, VariableOutput},
};
use sha2::{Digest, Sha256};
use tiny_keccak::Hasher;
#[cfg(feature = "std")]
use {
    fastpq_prover::{
        OperationKind as FastpqOperationKind, Prover as FastpqProver,
        PublicInputs as FastpqPublicInputs, StateTransition as FastpqStateTransition,
        TransitionBatch as FastpqTransitionBatch,
    },
    iroha_crypto::{Algorithm, EcdsaSecp256k1Sha256, KeyPair},
    iroha_data_model::zk::{BackendTag, OpenVerifyEnvelope, StarkFriOpenProofV1},
    norito::to_bytes,
};

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
pub const SCCP_EVM_SECP256K1_PROOF_BACKEND_V1: &str = "evm-secp256k1-keccak-v1";

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
const SCCP_PARLIAMENT_HASH_PREFIX_V1: &[u8] = b"sccp:parliament:v1";
const SCCP_TRANSPARENT_STATEMENT_PREFIX_V1: &[u8] = b"sccp:transparent:statement:v1";
const SCCP_DESTINATION_BINDING_PREFIX_V1: &[u8] = b"sccp:destination:binding:v1";
const SCCP_TRANSPARENT_FASTPQ_DSID_PREFIX_V1: &[u8] = b"sccp:transparent:fastpq:dsid:v1";
const SCCP_TRANSPARENT_OPEN_VERIFY_SCHEMA_HASH_PREFIX_V1: &[u8] =
    b"sccp:transparent:open-verify-schema:v1";
const SCCP_TRANSPARENT_FASTPQ_PARAMETER_SET_V1: &str = "fastpq-lane-balanced";
const SCCP_TRANSPARENT_FASTPQ_STATEMENT_KEY_V1: &[u8] = b"sccp:transparent:v1:statement";
const SCCP_TRANSPARENT_FASTPQ_CONTEXT_KEY_V1: &[u8] = b"sccp:transparent:v1:context";
const SCCP_TRANSPARENT_FASTPQ_PAYLOAD_KEY_V1: &[u8] = b"sccp:transparent:v1:payload";
const SCCP_TRANSPARENT_OPEN_VERIFY_CIRCUIT_ID_V1: &str = "sccp-message-transparent-v1";
const SCCP_EVM_ATTESTATION_DOMAIN_PREFIX_V1: &[u8] = b"iroha:sccp:evm-attestation:v1";
const SCCP_EVM_DESTINATION_BINDING_DOMAIN_PREFIX_V1: &[u8] =
    b"iroha:sccp:evm-destination-binding:v1";

pub type H256 = [u8; 32];

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

    pub mod option_hex32 {
        use super::{Deserialize, Deserializer, Serializer, String, decode_hex_fixed, encode_hex};

        #[allow(clippy::ref_option)]
        pub fn serialize<S>(value: &Option<[u8; 32]>, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            match value {
                Some(bytes) => serializer.serialize_some(&encode_hex(bytes)),
                None => serializer.serialize_none(),
            }
        }

        pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<[u8; 32]>, D::Error>
        where
            D: Deserializer<'de>,
        {
            let value = Option::<String>::deserialize(deserializer)?;
            value
                .map(|text| decode_hex_fixed::<32>(&text).map_err(serde::de::Error::custom))
                .transpose()
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
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
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
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
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
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
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
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
pub enum GovernancePayloadV1 {
    Add(TokenAddPayloadV1),
    Pause(TokenControlPayloadV1),
    Resume(TokenControlPayloadV1),
}

impl GovernancePayloadV1 {
    const ADD_DISCRIMINANT: u8 = 0;
    const PAUSE_DISCRIMINANT: u8 = 1;
    const RESUME_DISCRIMINANT: u8 = 2;
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
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
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
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
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
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
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
pub enum SccpPayloadV1 {
    AssetRegister(AssetRegisterPayloadV1),
    RouteActivate(RouteActivatePayloadV1),
    Transfer(TransferPayloadV1),
}

impl SccpPayloadV1 {
    const ASSET_REGISTER_DISCRIMINANT: u8 = 0;
    const ROUTE_ACTIVATE_DISCRIMINANT: u8 = 1;
    const TRANSFER_DISCRIMINANT: u8 = 2;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
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
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
pub struct SccpHubCommitmentV1 {
    pub version: u8,
    pub kind: SccpHubMessageKind,
    pub target_domain: u32,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub message_id: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub payload_hash: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::option_hex32"))]
    pub parliament_certificate_hash: Option<H256>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
pub struct SccpMerkleStepV1 {
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub sibling_hash: H256,
    pub sibling_is_left: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
pub struct SccpMerkleProofV1 {
    pub steps: Vec<SccpMerkleStepV1>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
pub enum NexusConsensusPhaseV1 {
    Prepare = 1,
    Commit = 2,
    NewView = 3,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
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
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
pub enum NexusParliamentSignatureSchemeV1 {
    SimpleThreshold,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
pub struct NexusParliamentSignatureV1 {
    pub signer: String,
    pub public_key: String,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub signature: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
pub struct NexusParliamentRosterMemberV1 {
    pub signer: String,
    pub public_keys: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
pub struct NexusParliamentCertificateV1 {
    pub version: u8,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub preimage_hash: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::u64_string"))]
    pub enactment_window_start: u64,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::u64_string"))]
    pub enactment_window_end: u64,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub payload_bytes: Vec<u8>,
    pub signature_scheme: NexusParliamentSignatureSchemeV1,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::u64_string"))]
    pub roster_epoch: u64,
    pub roster_members: Vec<NexusParliamentRosterMemberV1>,
    pub required_signatures: u16,
    pub signatures: Vec<NexusParliamentSignatureV1>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
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
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
pub struct NexusSccpGovernanceProofV1 {
    pub version: u8,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub commitment_root: H256,
    pub commitment: SccpHubCommitmentV1,
    pub merkle_proof: SccpMerkleProofV1,
    pub payload: GovernancePayloadV1,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub parliament_certificate: Vec<u8>,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub finality_proof: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
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
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::option_hex32"))]
    pub parliament_certificate_hash: Option<H256>,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SccpRuntimeParliamentCertificateV1 {
    pub version: u8,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub preimage_hash: H256,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::u64_string"))]
    pub enactment_window_start: u64,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::u64_string"))]
    pub enactment_window_end: u64,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::u64_string"))]
    pub roster_epoch: u64,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub roster_hash: H256,
    pub required_signatures: u16,
    pub signature_count: u16,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub certificate_hash: H256,
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
    pub parliament_certificate: Option<SccpRuntimeParliamentCertificateV1>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
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
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
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
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
pub enum SccpTransparentChainFamilyV1 {
    Evm,
    Solana,
    Ton,
    Tron,
    Substrate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
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
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
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
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
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
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
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
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
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

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
pub enum SccpPayloadProjectionV1 {
    AssetRegister(SccpAssetRegisterProjectionV1),
    RouteActivate(SccpRouteActivateProjectionV1),
    Transfer(SccpTransferProjectionV1),
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
pub struct SccpSubmissionArgumentV1 {
    pub key: String,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
pub struct SccpCounterpartySubmissionTemplateV1 {
    pub version: u8,
    pub encoding: String,
    pub submission_kind: String,
    pub verifier_entrypoint: String,
    pub required_arguments: Vec<SccpSubmissionArgumentV1>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
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
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
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
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
pub enum SccpProofVerifierTargetV1 {
    EvmContract,
    SolanaProgram,
    TonContract,
    TronContract,
    SubstrateRuntime,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
pub enum SccpProofSecurityModelV1 {
    RecursiveZk,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
pub enum SccpAnchorGovernanceV1 {
    SoraParliament,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
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

#[cfg(feature = "std")]
impl norito::json::FastJsonWrite for SccpDestinationVerifierPlanV1 {
    fn write_json(&self, out: &mut String) {
        norito::json::write_json_string(self.as_str(), out);
    }
}

#[cfg(feature = "std")]
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

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::JsonSerialize, norito::derive::JsonDeserialize)
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
pub struct SccpDestinationRolloutV1 {
    pub version: u8,
    pub verifier_plan: SccpDestinationVerifierPlanV1,
    pub immutable_verifier_ready: bool,
    pub anchors_ready: bool,
    #[cfg_attr(feature = "serde", serde(default))]
    #[cfg_attr(feature = "std", norito(default))]
    pub verifier_identity: Option<String>,
    #[cfg_attr(feature = "serde", serde(default))]
    #[cfg_attr(feature = "std", norito(default))]
    pub verifier_code_hash: Option<String>,
    #[cfg_attr(feature = "serde", serde(default))]
    #[cfg_attr(feature = "std", norito(default))]
    pub anchor_id: Option<String>,
    #[cfg_attr(feature = "serde", serde(default))]
    #[cfg_attr(feature = "std", norito(default))]
    pub blockers: Vec<String>,
}

impl Default for SccpDestinationRolloutV1 {
    fn default() -> Self {
        Self {
            version: 1,
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

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::JsonSerialize, norito::derive::JsonDeserialize)
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
pub struct SccpDestinationBindingV1 {
    pub version: u8,
    pub key: String,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub binding_hash: H256,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::JsonSerialize, norito::derive::JsonDeserialize)
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
#[norito(tag = "family", content = "detail", rename_all = "snake_case")]
pub enum SccpVerifierBackendFamilyV1 {
    EvmSecp256k1Keccak,
    SolanaProgram,
    TonContract,
    TronStarkFri,
    SubstrateRuntime,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::JsonSerialize, norito::derive::JsonDeserialize)
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
pub struct SccpVerifierBackendV1 {
    pub version: u8,
    pub family: SccpVerifierBackendFamilyV1,
    pub key: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::JsonSerialize, norito::derive::JsonDeserialize)
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
pub struct SccpSubmissionArgumentValueV1 {
    pub key: String,
    pub encoding: String,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::JsonSerialize, norito::derive::JsonDeserialize)
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
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

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::JsonSerialize, norito::derive::JsonDeserialize)
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
pub struct SccpEvmAttestationSignatureV1 {
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub signer_address: Vec<u8>,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub signature_bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::JsonSerialize, norito::derive::JsonDeserialize)
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
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

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::JsonSerialize, norito::derive::JsonDeserialize)
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
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
#[cfg_attr(
    feature = "std",
    derive(norito::derive::JsonSerialize, norito::derive::JsonDeserialize)
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
pub struct SccpTronContractSubmissionPayloadV1 {
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub proof_bytes: Vec<u8>,
    pub public_inputs: SccpEvmWordPublicInputsV1,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::hex32"))]
    pub statement_hash: H256,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::JsonSerialize, norito::derive::JsonDeserialize)
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
#[allow(clippy::struct_field_names)]
pub struct SccpSolanaProgramSubmissionPayloadV1 {
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub proof_bytes: Vec<u8>,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub public_inputs_bytes: Vec<u8>,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub bundle_bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::JsonSerialize, norito::derive::JsonDeserialize)
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
#[allow(clippy::struct_field_names)]
pub struct SccpTonInternalMessageSubmissionPayloadV1 {
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub proof_cell: Vec<u8>,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub public_inputs_cell: Vec<u8>,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub bundle_cell: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::JsonSerialize, norito::derive::JsonDeserialize)
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
#[allow(clippy::struct_field_names)]
pub struct SccpSubstrateRuntimeSubmissionPayloadV1 {
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub proof_bytes: Vec<u8>,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub public_inputs_bytes: Vec<u8>,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub bundle_bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::JsonSerialize, norito::derive::JsonDeserialize)
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
#[norito(tag = "platform", content = "payload", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
pub enum SccpPlatformSubmissionPayloadV1 {
    EvmContractCall(SccpEvmContractSubmissionPayloadV1),
    SolanaProgramInstruction(SccpSolanaProgramSubmissionPayloadV1),
    TonInternalMessage(SccpTonInternalMessageSubmissionPayloadV1),
    TronContractCall(SccpTronContractSubmissionPayloadV1),
    SubstrateRuntimeCall(SccpSubstrateRuntimeSubmissionPayloadV1),
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::JsonSerialize, norito::derive::JsonDeserialize)
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
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
#[cfg_attr(
    feature = "std",
    derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize)
)]
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
    #[cfg_attr(feature = "std", norito(default))]
    pub destination_rollout: SccpDestinationRolloutV1,
    pub production_ready: bool,
    #[cfg_attr(feature = "serde", serde(default))]
    #[cfg_attr(feature = "std", norito(default))]
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

fn sccp_proof_finality_model_for_domain(domain: u32) -> Option<SccpProofFinalityModelV1> {
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
        SCCP_DOMAIN_ETH | SCCP_DOMAIN_BSC => Some(SccpVerifierBackendFamilyV1::EvmSecp256k1Keccak),
        SCCP_DOMAIN_SOL => Some(SccpVerifierBackendFamilyV1::SolanaProgram),
        SCCP_DOMAIN_TON => Some(SccpVerifierBackendFamilyV1::TonContract),
        SCCP_DOMAIN_TRON => Some(SccpVerifierBackendFamilyV1::TronStarkFri),
        SCCP_DOMAIN_SORA_KUSAMA | SCCP_DOMAIN_SORA_POLKADOT | SCCP_DOMAIN_SORA2 => {
            Some(SccpVerifierBackendFamilyV1::SubstrateRuntime)
        }
        _ => None,
    }
}

fn sccp_verifier_backend_key_for_domain(domain: u32) -> Option<&'static str> {
    match domain {
        SCCP_DOMAIN_ETH | SCCP_DOMAIN_BSC => Some(SCCP_EVM_SECP256K1_PROOF_BACKEND_V1),
        SCCP_DOMAIN_SOL => Some("solana-program-v1"),
        SCCP_DOMAIN_TON => Some("ton-contract-v1"),
        SCCP_DOMAIN_TRON => Some("tron-stark-fri-v1"),
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
    SccpAnchorGovernanceV1::SoraParliament
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
            "Sora Parliament anchor set is not approved for this SCCP lane".to_owned(),
            "Groth16/bn254 adapter proof submission is not wired into the SCCP relayer path"
                .to_owned(),
        ],
        SCCP_DOMAIN_SOL => vec![
            "immutable Solana verifier program is not deployed for this SCCP lane".to_owned(),
            "Sora Parliament anchor set is not approved for this SCCP lane".to_owned(),
            "native recursive verifier program submission is not wired into the SCCP relayer path"
                .to_owned(),
        ],
        SCCP_DOMAIN_TON => vec![
            "immutable TON verifier contract is not deployed for this SCCP lane".to_owned(),
            "Sora Parliament anchor set is not approved for this SCCP lane".to_owned(),
            "native recursive verifier contract submission is not wired into the SCCP relayer path"
                .to_owned(),
        ],
        SCCP_DOMAIN_TRON => vec![
            "immutable TRON verifier contract is not deployed for this SCCP lane".to_owned(),
            "Sora Parliament anchor set is not approved for this SCCP lane".to_owned(),
            "native recursive verifier contract submission is not wired into the SCCP relayer path"
                .to_owned(),
        ],
        SCCP_DOMAIN_SORA_KUSAMA | SCCP_DOMAIN_SORA_POLKADOT | SCCP_DOMAIN_SORA2 => vec![
            "immutable Substrate runtime verifier call is not deployed for this SCCP lane"
                .to_owned(),
            "Sora Parliament anchor set is not approved for this SCCP lane".to_owned(),
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
        verifier_plan: sccp_destination_verifier_plan_for_domain(domain)?,
        immutable_verifier_ready: false,
        anchors_ready: false,
        verifier_identity: None,
        verifier_code_hash: None,
        anchor_id: None,
        blockers: sccp_destination_rollout_blockers_for_domain(domain)?,
    })
}

fn sccp_lane_disabled_reason_for_plan(plan: SccpDestinationVerifierPlanV1) -> &'static str {
    match plan {
        SccpDestinationVerifierPlanV1::EvmGroth16Bn254Adapter => {
            "disabled until the immutable EVM Groth16/bn254 SCCP verifier and Sora Parliament anchors are live for this lane"
        }
        SccpDestinationVerifierPlanV1::SolanaProgramNativeRecursive => {
            "disabled until the immutable Solana recursive SCCP verifier and Sora Parliament anchors are live for this lane"
        }
        SccpDestinationVerifierPlanV1::TonContractNativeRecursive => {
            "disabled until the immutable TON recursive SCCP verifier and Sora Parliament anchors are live for this lane"
        }
        SccpDestinationVerifierPlanV1::TronContractNativeRecursive => {
            "disabled until the immutable TRON recursive SCCP verifier and Sora Parliament anchors are live for this lane"
        }
        SccpDestinationVerifierPlanV1::SubstrateRuntimeNativeRecursive => {
            "disabled until the immutable Substrate runtime SCCP verifier and Sora Parliament anchors are live for this lane"
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

pub const SCCP_PRODUCTION_DISABLED_REASON_V1: &str = "disabled until immutable destination verifiers validate recursive SCCP proofs under Sora Parliament-governed trust anchors";

pub fn sccp_lane_production_ready_for_domain(domain: u32) -> bool {
    sccp_destination_rollout_for_domain(domain).is_some_and(|rollout| {
        rollout.immutable_verifier_ready
            && rollout.anchors_ready
            && rollout.verifier_identity.is_some()
            && rollout.verifier_code_hash.is_some()
            && rollout.anchor_id.is_some()
    })
}

pub fn sccp_lane_disabled_reason_for_domain(domain: u32) -> Option<&'static str> {
    sccp_destination_rollout_for_domain(domain)
        .map(|rollout| sccp_lane_disabled_reason_for_plan(rollout.verifier_plan))
        .filter(|_| !sccp_lane_production_ready_for_domain(domain))
}

pub fn sccp_manifest_is_production_ready(manifest: &SccpProofManifestV1) -> bool {
    manifest.production_ready
}

pub fn sccp_message_payload_kind_keys_v1() -> Vec<String> {
    vec![
        "asset_register".to_owned(),
        "route_activate".to_owned(),
        "transfer".to_owned(),
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
                    "EVM attestation envelope bytes authorizing the native SCCP proof hash.",
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
            encoding: "ton_cell_v1".to_owned(),
            submission_kind: "internal_message".to_owned(),
            verifier_entrypoint: "op::submit_sccp_message_proof".to_owned(),
            required_arguments: sccp_submission_arguments(&[
                (
                    "proof_cell",
                    "Transparent SCCP proof cell emitted by the TON prover backend.",
                ),
                (
                    "public_inputs_cell",
                    "Cell-encoded SCCP public inputs in manifest order.",
                ),
                (
                    "bundle_cell",
                    "Cell-encoded Nexus SCCP message bundle for the TON bridge contract.",
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
                    "Transparent SCCP proof bytes emitted by the prover backend.",
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
        SccpAnchorGovernanceV1::SoraParliament => 1,
    }
}

fn sccp_verifier_backend_family_code(family: SccpVerifierBackendFamilyV1) -> u8 {
    match family {
        SccpVerifierBackendFamilyV1::EvmSecp256k1Keccak => 1,
        SccpVerifierBackendFamilyV1::SolanaProgram => 2,
        SccpVerifierBackendFamilyV1::TonContract => 3,
        SccpVerifierBackendFamilyV1::TronStarkFri => 4,
        SccpVerifierBackendFamilyV1::SubstrateRuntime => 5,
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
    }
}

#[cfg(feature = "std")]
pub fn build_sccp_counterparty_proof_job_from_bundle_with_signer(
    bundle: &NexusSccpMessageProofV1,
    signer: &KeyPair,
) -> Option<SccpCounterpartyProofJobV1> {
    build_sccp_counterparty_proof_job_from_bundle_internal(bundle, Some(signer))
}

#[cfg(feature = "std")]
pub fn build_sccp_counterparty_proof_job_from_bundle(
    bundle: &NexusSccpMessageProofV1,
) -> Option<SccpCounterpartyProofJobV1> {
    build_sccp_counterparty_proof_job_from_bundle_internal(bundle, None)
}

#[cfg(feature = "std")]
fn build_sccp_counterparty_proof_job_from_bundle_internal(
    bundle: &NexusSccpMessageProofV1,
    signer: Option<&KeyPair>,
) -> Option<SccpCounterpartyProofJobV1> {
    if !verify_message_bundle_structure(bundle) {
        return None;
    }
    let counterparty_domain = sccp_counterparty_domain_for_message_payload(&bundle.payload)?;
    let manifest = sccp_proof_manifest_for_domain(counterparty_domain)?;
    if !sccp_manifest_is_production_ready(&manifest) {
        return None;
    }
    let chain_family = sccp_transparent_chain_family_for_domain(counterparty_domain)?;
    let chain = sccp_chain_key_for_domain(counterparty_domain)?;
    let public_inputs = sccp_message_transparent_public_inputs(bundle)?;
    let payload_projection = sccp_payload_projection(&bundle.payload)?;
    let proof_bytes = build_sccp_message_transparent_fastpq_proof_bytes(bundle, &manifest)?;
    let submission_package = build_sccp_counterparty_submission_package_internal(
        bundle,
        &manifest,
        &proof_bytes,
        signer,
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
        proof_family: manifest.proof_family,
        verifier_backend: manifest.verifier_backend,
        message_backend: manifest.message_backend,
        registry_backend: manifest.registry_backend,
        manifest_seed: manifest.manifest_seed,
        finality_model: manifest.finality_model,
        verifier_target: manifest.verifier_target,
        public_inputs,
        payload_kind: sccp_message_payload_kind_key(&bundle.payload).to_owned(),
        payload_projection,
        submission_template: manifest.submission_template,
        submission_package,
        bundle: bundle.clone(),
    })
}

#[cfg(not(feature = "std"))]
pub fn build_sccp_counterparty_proof_job_from_bundle(
    bundle: &NexusSccpMessageProofV1,
) -> Option<SccpCounterpartyProofJobV1> {
    build_sccp_counterparty_proof_job_from_bundle_internal(bundle)
}

#[cfg(not(feature = "std"))]
fn build_sccp_counterparty_proof_job_from_bundle_internal(
    bundle: &NexusSccpMessageProofV1,
) -> Option<SccpCounterpartyProofJobV1> {
    if !verify_message_bundle_structure(bundle) {
        return None;
    }
    let counterparty_domain = sccp_counterparty_domain_for_message_payload(&bundle.payload)?;
    let manifest = sccp_proof_manifest_for_domain(counterparty_domain)?;
    if !sccp_manifest_is_production_ready(&manifest) {
        return None;
    }
    let chain_family = sccp_transparent_chain_family_for_domain(counterparty_domain)?;
    let chain = sccp_chain_key_for_domain(counterparty_domain)?;
    let public_inputs = sccp_message_transparent_public_inputs(bundle)?;
    let payload_projection = sccp_payload_projection(&bundle.payload)?;
    let proof_bytes = build_sccp_message_transparent_fastpq_proof_bytes(bundle, &manifest)?;
    let submission_package =
        build_sccp_counterparty_submission_package(bundle, &manifest, &proof_bytes)?;

    Some(SccpCounterpartyProofJobV1 {
        version: 1,
        chain_family,
        chain: chain.to_owned(),
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
        payload_kind: sccp_message_payload_kind_key(&bundle.payload).to_owned(),
        payload_projection,
        submission_template: manifest.submission_template,
        submission_package,
        bundle: bundle.clone(),
    })
}

pub fn build_sccp_counterparty_proof_job_from_artifact(
    artifact: &NexusSccpMessageTransparentProofV1,
) -> Option<SccpCounterpartyProofJobV1> {
    verify_nexus_sccp_message_transparent_proof_structure(artifact).then(|| {
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
    })?
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
    match template.encoding.as_str() {
        "ton_cell_v1" => "ton_message_body_v1".to_owned(),
        _ => template.encoding.clone(),
    }
}

fn encode_sccp_submission_envelope(
    template: &SccpCounterpartySubmissionTemplateV1,
    args: &[SccpSubmissionArgumentValueV1],
) -> Vec<u8> {
    match template.encoding.as_str() {
        "abi_tuple_v1" | "tron_abi_tuple_v1" => {
            encode_abi_call(&template.verifier_entrypoint, args).unwrap_or_default()
        }
        "borsh_instruction_v1" | "ton_cell_v1" => {
            let arg_bytes = args.iter().map(|arg| arg.bytes.clone()).collect::<Vec<_>>();
            let mut out = Vec::new();
            push_vec(&mut out, template.verifier_entrypoint.as_bytes());
            for value in &arg_bytes {
                push_vec(&mut out, value);
            }
            out
        }
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
    if signatures_end > payload.len() || signatures_len % 65 != 0 {
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

#[cfg(feature = "std")]
fn sccp_evm_signer_public_key_bytes(signer: &KeyPair) -> Option<Vec<u8>> {
    (signer.algorithm() == Algorithm::Secp256k1).then(|| signer.public_key().to_bytes().1.to_vec())
}

#[cfg(feature = "std")]
fn sccp_evm_signer_address(signer: &KeyPair) -> Option<[u8; 20]> {
    let public_key =
        EcdsaSecp256k1Sha256::parse_public_key(&sccp_evm_signer_public_key_bytes(signer)?).ok()?;
    Some(EcdsaSecp256k1Sha256::evm_address(&public_key))
}

#[cfg(feature = "std")]
fn sccp_evm_sign_digest(signer: &KeyPair, digest: &H256) -> Option<[u8; 65]> {
    if signer.algorithm() != Algorithm::Secp256k1 {
        return None;
    }
    let secret_key_bytes = signer.private_key().to_bytes().1;
    let secret_key = EcdsaSecp256k1Sha256::parse_private_key(&secret_key_bytes).ok()?;
    EcdsaSecp256k1Sha256::sign_prehash_recoverable(digest, &secret_key).ok()
}

#[cfg(feature = "std")]
fn build_sccp_evm_contract_submission_payload(
    manifest: &SccpProofManifestV1,
    native_proof_bytes: &[u8],
    public_inputs: &SccpMessageTransparentPublicInputsV1,
    statement_hash: H256,
    destination_binding: &SccpDestinationBindingV1,
    signer: &KeyPair,
) -> Option<SccpEvmContractSubmissionPayloadV1> {
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

fn build_sccp_platform_submission_payload(
    manifest: &SccpProofManifestV1,
    native_proof_bytes: &[u8],
    public_inputs: &SccpMessageTransparentPublicInputsV1,
    bundle: &NexusSccpMessageProofV1,
    destination_binding: Option<&SccpDestinationBindingV1>,
    #[cfg(feature = "std")] signer: Option<&KeyPair>,
) -> Option<SccpPlatformSubmissionPayloadV1> {
    let canonical_public_inputs =
        canonical_sccp_message_transparent_public_inputs_bytes(public_inputs);
    let canonical_bundle = canonical_nexus_sccp_message_bundle_bytes(bundle);
    let inner = build_sccp_message_transparent_inner_proof(bundle, manifest)?;
    Some(match manifest.verifier_target {
        #[cfg(feature = "std")]
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
        #[cfg(not(feature = "std"))]
        SccpProofVerifierTargetV1::EvmContract => return None,
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
                SccpTonInternalMessageSubmissionPayloadV1 {
                    proof_cell: native_proof_bytes.to_vec(),
                    public_inputs_cell: canonical_public_inputs,
                    bundle_cell: canonical_bundle,
                },
            )
        }
        SccpProofVerifierTargetV1::TronContract => {
            SccpPlatformSubmissionPayloadV1::TronContractCall(SccpTronContractSubmissionPayloadV1 {
                proof_bytes: native_proof_bytes.to_vec(),
                public_inputs: sccp_evm_public_input_word_struct(public_inputs),
                statement_hash: inner.statement_hash,
            })
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
                (SccpPlatformSubmissionPayloadV1::EvmContractCall(payload), "statement_hash") => {
                    ("abi_bytes32".to_owned(), payload.statement_hash.to_vec())
                }
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
                (SccpPlatformSubmissionPayloadV1::TonInternalMessage(payload), "proof_cell") => {
                    ("raw_bytes".to_owned(), payload.proof_cell.clone())
                }
                (
                    SccpPlatformSubmissionPayloadV1::TonInternalMessage(payload),
                    "public_inputs_cell",
                ) => ("raw_bytes".to_owned(), payload.public_inputs_cell.clone()),
                (SccpPlatformSubmissionPayloadV1::TonInternalMessage(payload), "bundle_cell") => {
                    ("raw_bytes".to_owned(), payload.bundle_cell.clone())
                }
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

#[cfg(feature = "std")]
pub fn build_sccp_counterparty_submission_package_with_signer(
    bundle: &NexusSccpMessageProofV1,
    manifest: &SccpProofManifestV1,
    proof_bytes: &[u8],
    signer: &KeyPair,
) -> Option<SccpCounterpartySubmissionPackageV1> {
    build_sccp_counterparty_submission_package_internal(bundle, manifest, proof_bytes, Some(signer))
}

#[cfg(feature = "std")]
pub fn build_sccp_counterparty_submission_package_with_destination_binding_and_signer(
    bundle: &NexusSccpMessageProofV1,
    manifest: &SccpProofManifestV1,
    proof_bytes: &[u8],
    destination_binding: &SccpDestinationBindingV1,
    signer: &KeyPair,
) -> Option<SccpCounterpartySubmissionPackageV1> {
    if !sccp_manifest_is_production_ready(manifest) {
        return None;
    }
    let public_inputs = sccp_message_transparent_public_inputs(bundle)?;
    let platform_payload = build_sccp_platform_submission_payload(
        manifest,
        proof_bytes,
        &public_inputs,
        bundle,
        Some(destination_binding),
        Some(signer),
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

#[cfg(feature = "std")]
pub fn build_sccp_counterparty_submission_package(
    bundle: &NexusSccpMessageProofV1,
    manifest: &SccpProofManifestV1,
    proof_bytes: &[u8],
) -> Option<SccpCounterpartySubmissionPackageV1> {
    build_sccp_counterparty_submission_package_internal(bundle, manifest, proof_bytes, None)
}

#[cfg(feature = "std")]
fn build_sccp_counterparty_submission_package_internal(
    bundle: &NexusSccpMessageProofV1,
    manifest: &SccpProofManifestV1,
    proof_bytes: &[u8],
    signer: Option<&KeyPair>,
) -> Option<SccpCounterpartySubmissionPackageV1> {
    if !sccp_manifest_is_production_ready(manifest) {
        return None;
    }
    let public_inputs = sccp_message_transparent_public_inputs(bundle)?;
    let platform_payload = build_sccp_platform_submission_payload(
        manifest,
        proof_bytes,
        &public_inputs,
        bundle,
        None,
        signer,
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

#[cfg(not(feature = "std"))]
pub fn build_sccp_counterparty_submission_package(
    bundle: &NexusSccpMessageProofV1,
    manifest: &SccpProofManifestV1,
    proof_bytes: &[u8],
) -> Option<SccpCounterpartySubmissionPackageV1> {
    build_sccp_counterparty_submission_package_internal(bundle, manifest, proof_bytes)
}

#[cfg(not(feature = "std"))]
fn build_sccp_counterparty_submission_package_internal(
    bundle: &NexusSccpMessageProofV1,
    manifest: &SccpProofManifestV1,
    proof_bytes: &[u8],
) -> Option<SccpCounterpartySubmissionPackageV1> {
    if !sccp_manifest_is_production_ready(manifest) {
        return None;
    }
    let public_inputs = sccp_message_transparent_public_inputs(bundle)?;
    let platform_payload = build_sccp_platform_submission_payload(
        manifest,
        proof_bytes,
        &public_inputs,
        bundle,
        None,
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

pub fn sccp_message_transparent_public_inputs(
    bundle: &NexusSccpMessageProofV1,
) -> Option<SccpMessageTransparentPublicInputsV1> {
    if !verify_message_bundle_structure(bundle) {
        return None;
    }
    let finality = decode_nexus_bridge_finality_proof(&bundle.finality_proof)?;
    Some(SccpMessageTransparentPublicInputsV1 {
        version: 1,
        message_id: bundle.commitment.message_id,
        payload_hash: bundle.commitment.payload_hash,
        target_domain: bundle.commitment.target_domain,
        commitment_root: bundle.commitment_root,
        finality_height: finality.height,
        finality_block_hash: finality.block_hash,
    })
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

pub fn build_sccp_message_transparent_inner_proof(
    bundle: &NexusSccpMessageProofV1,
    manifest: &SccpProofManifestV1,
) -> Option<SccpMessageTransparentInnerProofV1> {
    let public_inputs = sccp_message_transparent_public_inputs(bundle)?;
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

#[cfg(feature = "std")]
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

#[cfg(feature = "std")]
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

#[cfg(feature = "std")]
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

#[cfg(feature = "std")]
fn sccp_message_transparent_fastpq_verifier_commitment(
    manifest: &SccpProofManifestV1,
) -> Option<H256> {
    let verifier = canonical_sccp_message_transparent_fastpq_verifier_bytes(manifest)?;
    let mut hasher = Sha256::new();
    Digest::update(&mut hasher, manifest.message_backend.as_bytes());
    Digest::update(&mut hasher, &verifier);
    Some(hasher.finalize().into())
}

#[cfg(feature = "std")]
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

#[cfg(feature = "std")]
fn sccp_open_verify_backend_key(backend: BackendTag) -> &'static str {
    match backend {
        BackendTag::Halo2IpaPasta => "halo2-ipa-pasta",
        BackendTag::Halo2Bn254 => "halo2-bn254",
        BackendTag::Groth16 => "groth16",
        BackendTag::Stark => "stark",
        BackendTag::Unsupported => "unsupported",
    }
}

#[cfg(feature = "std")]
fn saturating_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

#[cfg(feature = "std")]
fn decode_sccp_message_transparent_open_verify_envelope(
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

#[cfg(feature = "std")]
fn decode_sccp_message_transparent_open_verify_proof(
    proof_bytes: &[u8],
) -> Option<(
    OpenVerifyEnvelope,
    StarkFriOpenProofV1,
    fastpq_prover::Proof,
)> {
    let (env, open) = decode_sccp_message_transparent_open_verify_envelope(proof_bytes)?;
    let proof: fastpq_prover::Proof = norito::decode_from_bytes(&open.envelope_bytes).ok()?;
    Some((env, open, proof))
}

#[cfg(feature = "std")]
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

#[cfg(feature = "std")]
pub fn summarize_sccp_message_transparent_open_verify_proof_from_artifact(
    artifact: &NexusSccpMessageTransparentProofV1,
) -> Option<SccpOpenVerifyEnvelopeSummaryV1> {
    summarize_sccp_message_transparent_open_verify_proof(&artifact.proof_bytes)
}

#[cfg(feature = "std")]
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

#[cfg(feature = "std")]
fn build_sccp_message_transparent_fastpq_batch(
    bundle: &NexusSccpMessageProofV1,
    manifest: &SccpProofManifestV1,
) -> Option<FastpqTransitionBatch> {
    let inner = build_sccp_message_transparent_inner_proof(bundle, manifest)?;
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

#[cfg(feature = "std")]
fn build_sccp_message_transparent_fastpq_raw_proof_bytes(
    batch: &FastpqTransitionBatch,
) -> Option<Vec<u8>> {
    let proof = FastpqProver::canonical(SCCP_TRANSPARENT_FASTPQ_PARAMETER_SET_V1)
        .ok()?
        .prove(batch)
        .ok()?;
    to_bytes(&proof).ok()
}

#[cfg(feature = "std")]
fn build_sccp_message_transparent_fastpq_proof_bytes(
    bundle: &NexusSccpMessageProofV1,
    manifest: &SccpProofManifestV1,
) -> Option<Vec<u8>> {
    let batch = build_sccp_message_transparent_fastpq_batch(bundle, manifest)?;
    let raw_proof_bytes = build_sccp_message_transparent_fastpq_raw_proof_bytes(&batch)?;
    let public_inputs = sccp_message_transparent_public_inputs(bundle)?;
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

#[cfg(feature = "std")]
pub fn build_nexus_sccp_message_transparent_proof_with_signer(
    bundle: &NexusSccpMessageProofV1,
    signer: &KeyPair,
) -> Option<NexusSccpMessageTransparentProofV1> {
    build_nexus_sccp_message_transparent_proof_internal(bundle, Some(signer))
}

pub fn build_nexus_sccp_message_transparent_proof(
    bundle: &NexusSccpMessageProofV1,
) -> Option<NexusSccpMessageTransparentProofV1> {
    #[cfg(not(feature = "std"))]
    {
        let _ = bundle;
        return None;
    }

    #[cfg(feature = "std")]
    {
        return build_nexus_sccp_message_transparent_proof_internal(bundle, None);
    }

    #[allow(unreachable_code)]
    None
}

#[cfg(feature = "std")]
fn build_nexus_sccp_message_transparent_proof_internal(
    bundle: &NexusSccpMessageProofV1,
    signer: Option<&KeyPair>,
) -> Option<NexusSccpMessageTransparentProofV1> {
    let counterparty_domain = sccp_counterparty_domain_for_message_payload(&bundle.payload)?;
    let manifest = sccp_proof_manifest_for_domain(counterparty_domain)?;
    if !sccp_manifest_is_production_ready(&manifest) {
        return None;
    }
    let public_inputs = sccp_message_transparent_public_inputs(bundle)?;
    let proof_bytes = build_sccp_message_transparent_fastpq_proof_bytes(bundle, &manifest)?;
    let submission_package = build_sccp_counterparty_submission_package_internal(
        bundle,
        &manifest,
        &proof_bytes,
        signer,
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

fn verify_sccp_message_transparent_inner_proof_bytes(
    proof_bytes: &[u8],
    bundle: &NexusSccpMessageProofV1,
    manifest: &SccpProofManifestV1,
    public_inputs: &SccpMessageTransparentPublicInputsV1,
) -> bool {
    let Some(expected) = build_sccp_message_transparent_inner_proof(bundle, manifest) else {
        return false;
    };

    if &expected.public_inputs != public_inputs {
        return false;
    }

    #[cfg(feature = "std")]
    {
        let Some(batch) = build_sccp_message_transparent_fastpq_batch(bundle, manifest) else {
            return false;
        };
        let Some((env, open, proof)) =
            decode_sccp_message_transparent_open_verify_proof(proof_bytes)
        else {
            return false;
        };
        if env.circuit_id != SCCP_TRANSPARENT_OPEN_VERIFY_CIRCUIT_ID_V1
            || env.vk_hash
                != sccp_message_transparent_fastpq_verifier_commitment(manifest)
                    .unwrap_or([0u8; 32])
            || env.public_inputs != sccp_message_transparent_open_verify_schema_descriptor(manifest)
            || !env.aux.is_empty()
            || open.public_inputs != sccp_message_transparent_public_input_columns(public_inputs)
        {
            return false;
        }
        fastpq_prover::verify(&batch, &proof).is_ok()
    }

    #[cfg(not(feature = "std"))]
    {
        let _ = proof_bytes;
        let _ = expected;
        false
    }
}

#[cfg(feature = "std")]
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

#[cfg(feature = "std")]
fn verify_sccp_evm_submission_package(
    manifest: &SccpProofManifestV1,
    proof: &NexusSccpMessageTransparentProofV1,
) -> bool {
    let SccpPlatformSubmissionPayloadV1::EvmContractCall(payload) =
        &proof.submission_package.platform_payload
    else {
        return false;
    };
    let Some(inner) = build_sccp_message_transparent_inner_proof(&proof.bundle, manifest) else {
        return false;
    };
    let expected_public_inputs = sccp_evm_public_input_word_struct(&proof.public_inputs);
    if payload.public_inputs != expected_public_inputs
        || payload.public_inputs_hash != sccp_evm_public_inputs_hash(&proof.public_inputs)
        || payload.statement_hash != inner.statement_hash
        || payload.destination_binding.key.is_empty()
    {
        return false;
    }
    let expected_native_proof_hash = sccp_evm_native_proof_hash(&proof.proof_bytes);
    if payload.attestation.version != 1
        || payload.attestation.message_id != proof.public_inputs.message_id
        || payload.attestation.source_domain != manifest.local_domain
        || payload.attestation.commitment_root != proof.public_inputs.commitment_root
        || payload.attestation.native_proof_hash != expected_native_proof_hash
        || payload.attestation.destination_binding_hash != payload.destination_binding.binding_hash
    {
        return false;
    }
    let Some(decoded_envelope) = decode_sccp_evm_attestation_envelope(&payload.proof_bytes) else {
        return false;
    };
    if decoded_envelope.version != payload.attestation.version
        || decoded_envelope.message_id != payload.attestation.message_id
        || decoded_envelope.source_domain != payload.attestation.source_domain
        || decoded_envelope.commitment_root != payload.attestation.commitment_root
        || decoded_envelope.native_proof_hash != payload.attestation.native_proof_hash
        || decoded_envelope.destination_binding_hash != payload.attestation.destination_binding_hash
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
    let Some(expected_proof_bytes) = encode_sccp_evm_attestation_envelope(&payload.attestation)
    else {
        return false;
    };
    if expected_proof_bytes != payload.proof_bytes {
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

#[cfg(not(feature = "std"))]
fn verify_sccp_evm_submission_package(
    _manifest: &SccpProofManifestV1,
    _proof: &NexusSccpMessageTransparentProofV1,
) -> bool {
    false
}

pub fn verify_nexus_sccp_message_transparent_proof_structure(
    proof: &NexusSccpMessageTransparentProofV1,
) -> bool {
    if proof.version != 1
        || proof.local_domain != SCCP_DOMAIN_SORA
        || proof.proof_family != SCCP_STARK_FRI_PROOF_FAMILY_V1
        || proof.proof_bytes.is_empty()
        || !verify_message_bundle_structure(&proof.bundle)
    {
        return false;
    }
    let Some(manifest) = sccp_proof_manifest_for_domain(proof.counterparty_domain) else {
        return false;
    };
    if !sccp_manifest_is_production_ready(&manifest)
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
    sccp_message_transparent_public_inputs(&proof.bundle).is_some_and(|expected| {
        expected == proof.public_inputs
            && match manifest.verifier_target {
                SccpProofVerifierTargetV1::EvmContract => {
                    verify_sccp_evm_submission_package(&manifest, proof)
                }
                _ => build_sccp_counterparty_submission_package(
                    &proof.bundle,
                    &manifest,
                    &proof.proof_bytes,
                )
                .is_some_and(|expected_submission_package| {
                    expected_submission_package == proof.submission_package
                }),
            }
            && verify_sccp_message_transparent_inner_proof_bytes(
                &proof.proof_bytes,
                &proof.bundle,
                &manifest,
                &proof.public_inputs,
            )
    })
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

fn push_option_h256(out: &mut Vec<u8>, value: Option<&H256>) {
    match value {
        Some(value) => {
            push_u8(out, 1);
            out.extend_from_slice(value);
        }
        None => push_u8(out, 0),
    }
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

pub fn canonical_governance_payload_bytes(payload: &GovernancePayloadV1) -> Vec<u8> {
    let mut out = Vec::new();
    match payload {
        GovernancePayloadV1::Add(payload) => {
            push_u8(&mut out, GovernancePayloadV1::ADD_DISCRIMINANT);
            out.extend_from_slice(&canonical_token_add_payload_bytes(payload));
        }
        GovernancePayloadV1::Pause(payload) => {
            push_u8(&mut out, GovernancePayloadV1::PAUSE_DISCRIMINANT);
            out.extend_from_slice(&canonical_token_control_payload_bytes(payload));
        }
        GovernancePayloadV1::Resume(payload) => {
            push_u8(&mut out, GovernancePayloadV1::RESUME_DISCRIMINANT);
            out.extend_from_slice(&canonical_token_control_payload_bytes(payload));
        }
    }
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
        _ => return None,
    };
    cursor.is_finished().then_some(payload)
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
            payload.version == 1
                && is_supported_domain(payload.source_domain)
                && is_supported_domain(payload.asset_home_domain)
                && payload.source_domain != payload.dest_domain
                && validate_sccp_codec_bytes(payload.asset_id_codec, &payload.asset_id)
                && payload.amount != 0
                && validate_sccp_codec_bytes(payload.sender_codec, &payload.sender)
                && validate_sccp_codec_bytes(payload.recipient_codec, &payload.recipient)
                && validate_sccp_codec_bytes(payload.route_id_codec, &payload.route_id)
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
        parliament_certificate_hash: None,
    }
}

pub fn canonical_commitment_bytes(commitment: &SccpHubCommitmentV1) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + 1 + 4 + 32 + 32 + 1 + 32);
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
    push_option_h256(&mut out, commitment.parliament_certificate_hash.as_ref());
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

pub fn governance_message_id(payload: &GovernancePayloadV1) -> H256 {
    match payload {
        GovernancePayloadV1::Add(payload) => token_add_message_id(payload),
        GovernancePayloadV1::Pause(payload) => token_pause_message_id(payload),
        GovernancePayloadV1::Resume(payload) => token_resume_message_id(payload),
    }
}

pub fn governance_target_domain(payload: &GovernancePayloadV1) -> u32 {
    match payload {
        GovernancePayloadV1::Add(payload) => payload.target_domain,
        GovernancePayloadV1::Pause(payload) | GovernancePayloadV1::Resume(payload) => {
            payload.target_domain
        }
    }
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
    }
}

pub fn sccp_message_kind(payload: &SccpPayloadV1) -> SccpHubMessageKind {
    match payload {
        SccpPayloadV1::AssetRegister(_) => SccpHubMessageKind::AssetRegister,
        SccpPayloadV1::RouteActivate(_) => SccpHubMessageKind::RouteActivate,
        SccpPayloadV1::Transfer(_) => SccpHubMessageKind::Transfer,
    }
}

pub fn sccp_message_target_domain(payload: &SccpPayloadV1) -> u32 {
    match payload {
        SccpPayloadV1::AssetRegister(payload) => payload.target_domain,
        SccpPayloadV1::RouteActivate(payload) => payload.target_domain,
        SccpPayloadV1::Transfer(payload) => payload.dest_domain,
    }
}

pub fn payload_hash(payload: &[u8]) -> H256 {
    prefixed_blake2b(SCCP_PAYLOAD_HASH_PREFIX_V1, payload)
}

pub fn parliament_certificate_hash(certificate: &[u8]) -> H256 {
    prefixed_blake2b(SCCP_PARLIAMENT_HASH_PREFIX_V1, certificate)
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
        parliament_certificate_hash: commitment.parliament_certificate_hash,
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
    }
}

fn runtime_payload_from_governance_payload(payload: &GovernancePayloadV1) -> SccpRuntimePayloadV1 {
    match payload {
        GovernancePayloadV1::Add(payload) => SccpRuntimePayloadV1::TokenAdd(*payload),
        GovernancePayloadV1::Pause(payload) => SccpRuntimePayloadV1::TokenPause(*payload),
        GovernancePayloadV1::Resume(payload) => SccpRuntimePayloadV1::TokenResume(*payload),
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

pub fn sccp_runtime_parliament_roster_anchor_hash(
    certificate: &NexusParliamentCertificateV1,
) -> H256 {
    let mut out = Vec::new();
    push_u64(&mut out, certificate.roster_epoch);
    push_scale_compact(
        &mut out,
        u32::try_from(certificate.roster_members.len()).expect("roster length fits u32"),
    );
    for member in &certificate.roster_members {
        push_scale_vec(&mut out, member.signer.as_bytes());
        push_scale_compact(
            &mut out,
            u32::try_from(member.public_keys.len()).expect("member key length fits u32"),
        );
        for public_key in &member.public_keys {
            push_scale_vec(&mut out, public_key.as_bytes());
        }
    }
    prefixed_blake2b(b"sccp:nexus:parliament-roster-anchor:v1", &out)
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

fn runtime_certificate_from_nexus_certificate(
    certificate: &NexusParliamentCertificateV1,
    encoded_certificate: &[u8],
) -> Option<SccpRuntimeParliamentCertificateV1> {
    Some(SccpRuntimeParliamentCertificateV1 {
        version: 1,
        preimage_hash: certificate.preimage_hash,
        enactment_window_start: certificate.enactment_window_start,
        enactment_window_end: certificate.enactment_window_end,
        roster_epoch: certificate.roster_epoch,
        roster_hash: sccp_runtime_parliament_roster_anchor_hash(certificate),
        required_signatures: certificate.required_signatures,
        signature_count: u16::try_from(certificate.signatures.len()).ok()?,
        certificate_hash: parliament_certificate_hash(encoded_certificate),
    })
}

pub fn sccp_runtime_envelope_from_message_bundle(
    bundle: &NexusSccpMessageProofV1,
) -> Option<SccpRuntimeProofEnvelopeV1> {
    if !verify_message_bundle_structure(bundle) {
        return None;
    }
    let finality = decode_nexus_bridge_finality_proof(&bundle.finality_proof)?;
    Some(SccpRuntimeProofEnvelopeV1 {
        version: 1,
        commitment_root: bundle.commitment_root,
        commitment: runtime_commitment_from_hub(&bundle.commitment),
        merkle_proof: runtime_merkle_proof_from_hub(&bundle.merkle_proof),
        payload: runtime_payload_from_sccp_payload(&bundle.payload),
        finality_proof: runtime_finality_from_nexus_finality(&finality)?,
        parliament_certificate: None,
    })
}

pub fn sccp_runtime_envelope_from_governance_bundle(
    bundle: &NexusSccpGovernanceProofV1,
) -> Option<SccpRuntimeProofEnvelopeV1> {
    if !verify_governance_bundle_structure(bundle) {
        return None;
    }
    let finality = decode_nexus_bridge_finality_proof(&bundle.finality_proof)?;
    let certificate = decode_nexus_parliament_certificate(&bundle.parliament_certificate)?;
    Some(SccpRuntimeProofEnvelopeV1 {
        version: 1,
        commitment_root: bundle.commitment_root,
        commitment: runtime_commitment_from_hub(&bundle.commitment),
        merkle_proof: runtime_merkle_proof_from_hub(&bundle.merkle_proof),
        payload: runtime_payload_from_governance_payload(&bundle.payload),
        finality_proof: runtime_finality_from_nexus_finality(&finality)?,
        parliament_certificate: Some(runtime_certificate_from_nexus_certificate(
            &certificate,
            &bundle.parliament_certificate,
        )?),
    })
}

pub fn sccp_runtime_envelope_bytes_from_message_bundle(
    bundle: &NexusSccpMessageProofV1,
) -> Option<Vec<u8>> {
    Some(encode_sccp_runtime_proof_envelope(
        &sccp_runtime_envelope_from_message_bundle(bundle)?,
    ))
}

pub fn sccp_runtime_envelope_bytes_from_governance_bundle(
    bundle: &NexusSccpGovernanceProofV1,
) -> Option<Vec<u8>> {
    Some(encode_sccp_runtime_proof_envelope(
        &sccp_runtime_envelope_from_governance_bundle(bundle)?,
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
    match &envelope.parliament_certificate {
        Some(certificate) => {
            push_u8(&mut out, 1);
            push_runtime_parliament_certificate(&mut out, certificate);
        }
        None => push_u8(&mut out, 0),
    }
    out
}

fn push_runtime_commitment(out: &mut Vec<u8>, commitment: &SccpRuntimeHubCommitmentV1) {
    push_u8(out, commitment.version);
    push_u8(out, runtime_kind_code(commitment.kind));
    push_u32(out, commitment.target_domain);
    out.extend_from_slice(&commitment.message_id);
    out.extend_from_slice(&commitment.payload_hash);
    match commitment.parliament_certificate_hash {
        Some(hash) => {
            push_u8(out, 1);
            out.extend_from_slice(&hash);
        }
        None => push_u8(out, 0),
    }
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

fn push_runtime_parliament_certificate(
    out: &mut Vec<u8>,
    certificate: &SccpRuntimeParliamentCertificateV1,
) {
    push_u8(out, certificate.version);
    out.extend_from_slice(&certificate.preimage_hash);
    push_u64(out, certificate.enactment_window_start);
    push_u64(out, certificate.enactment_window_end);
    push_u64(out, certificate.roster_epoch);
    out.extend_from_slice(&certificate.roster_hash);
    push_u16(out, certificate.required_signatures);
    push_u16(out, certificate.signature_count);
    out.extend_from_slice(&certificate.certificate_hash);
}

#[cfg(feature = "std")]
pub fn decode_nexus_bridge_finality_proof(
    proof_bytes: &[u8],
) -> Option<NexusBridgeFinalityProofV1> {
    norito::decode_from_bytes(proof_bytes).ok()
}

#[cfg(not(feature = "std"))]
pub fn decode_nexus_bridge_finality_proof(
    proof_bytes: &[u8],
) -> Option<NexusBridgeFinalityProofV1> {
    let _ = proof_bytes;
    None
}

#[cfg(feature = "std")]
pub fn decode_nexus_parliament_certificate(
    certificate_bytes: &[u8],
) -> Option<NexusParliamentCertificateV1> {
    norito::decode_from_bytes(certificate_bytes).ok()
}

#[cfg(not(feature = "std"))]
pub fn decode_nexus_parliament_certificate(
    certificate_bytes: &[u8],
) -> Option<NexusParliamentCertificateV1> {
    let _ = certificate_bytes;
    None
}

#[cfg(feature = "std")]
pub fn decode_nexus_sccp_burn_proof(proof_bytes: &[u8]) -> Option<NexusSccpBurnProofV1> {
    norito::decode_from_bytes(proof_bytes).ok()
}

#[cfg(not(feature = "std"))]
pub fn decode_nexus_sccp_burn_proof(proof_bytes: &[u8]) -> Option<NexusSccpBurnProofV1> {
    let _ = proof_bytes;
    None
}

#[cfg(feature = "std")]
pub fn decode_nexus_sccp_governance_proof(
    proof_bytes: &[u8],
) -> Option<NexusSccpGovernanceProofV1> {
    norito::decode_from_bytes(proof_bytes).ok()
}

#[cfg(not(feature = "std"))]
pub fn decode_nexus_sccp_governance_proof(
    proof_bytes: &[u8],
) -> Option<NexusSccpGovernanceProofV1> {
    let _ = proof_bytes;
    None
}

#[cfg(feature = "std")]
pub fn decode_nexus_sccp_message_proof(proof_bytes: &[u8]) -> Option<NexusSccpMessageProofV1> {
    norito::decode_from_bytes(proof_bytes).ok()
}

#[cfg(not(feature = "std"))]
pub fn decode_nexus_sccp_message_proof(proof_bytes: &[u8]) -> Option<NexusSccpMessageProofV1> {
    let _ = proof_bytes;
    None
}

#[cfg(feature = "std")]
pub fn decode_nexus_sccp_message_transparent_proof(
    proof_bytes: &[u8],
) -> Option<NexusSccpMessageTransparentProofV1> {
    norito::decode_from_bytes(proof_bytes).ok()
}

#[cfg(not(feature = "std"))]
pub fn decode_nexus_sccp_message_transparent_proof(
    proof_bytes: &[u8],
) -> Option<NexusSccpMessageTransparentProofV1> {
    let _ = proof_bytes;
    None
}

pub fn recover_nexus_sccp_message_transparent_proof(
    backend: &str,
    proof_bytes: &[u8],
) -> Option<NexusSccpMessageTransparentProofV1> {
    if let Some(proof) = decode_nexus_sccp_message_transparent_proof(proof_bytes) {
        return (verify_nexus_sccp_message_transparent_proof_structure(&proof)
            && proof.message_backend == backend)
            .then_some(proof);
    }

    let counterparty_domain = sccp_counterparty_domain_from_backend(backend)?;
    let bundle = decode_nexus_sccp_message_proof(proof_bytes)?;
    let proof = build_nexus_sccp_message_transparent_proof(&bundle)?;
    (proof.counterparty_domain == counterparty_domain && proof.message_backend == backend)
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

    for public_key in &qc.validator_public_keys {
        if public_key.is_empty() {
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
    signer_indices_from_bitmap(&qc.signers_bitmap, roster_len).is_some()
}

pub fn verify_nexus_parliament_certificate_structure(
    certificate: &NexusParliamentCertificateV1,
    governance_payload_encoded: &[u8],
    proof_height: u64,
) -> bool {
    if certificate.version != 1
        || certificate.payload_bytes.is_empty()
        || certificate.signatures.is_empty()
        || certificate.roster_members.is_empty()
        || certificate.required_signatures == 0
        || usize::from(certificate.required_signatures) > certificate.roster_members.len()
        || certificate.enactment_window_start > certificate.enactment_window_end
        || proof_height < certificate.enactment_window_start
        || proof_height > certificate.enactment_window_end
        || certificate.preimage_hash != payload_hash(governance_payload_encoded)
    {
        return false;
    }

    let mut seen_roster_members = Vec::with_capacity(certificate.roster_members.len());
    for roster_member in &certificate.roster_members {
        if roster_member.signer.is_empty() || roster_member.public_keys.is_empty() {
            return false;
        }
        if seen_roster_members.contains(&roster_member.signer.as_str()) {
            return false;
        }
        seen_roster_members.push(roster_member.signer.as_str());

        let mut seen_public_keys = Vec::with_capacity(roster_member.public_keys.len());
        for public_key in &roster_member.public_keys {
            if public_key.is_empty() || seen_public_keys.contains(&public_key.as_str()) {
                return false;
            }
            seen_public_keys.push(public_key.as_str());
        }
    }

    let mut seen_signers = Vec::with_capacity(certificate.signatures.len());
    for signature in &certificate.signatures {
        if signature.signer.is_empty()
            || signature.public_key.is_empty()
            || signature.signature.is_empty()
            || seen_signers.contains(&signature.signer.as_str())
        {
            return false;
        }
        seen_signers.push(signature.signer.as_str());

        let Some(roster_member) = certificate
            .roster_members
            .iter()
            .find(|member| member.signer == signature.signer)
        else {
            return false;
        };
        if !roster_member
            .public_keys
            .iter()
            .any(|public_key| public_key == &signature.public_key)
        {
            return false;
        }
    }

    match certificate.signature_scheme {
        NexusParliamentSignatureSchemeV1::SimpleThreshold => {
            certificate.signatures.len() >= usize::from(certificate.required_signatures)
        }
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
    if bundle.payload.version != 1 || !is_supported_domain(bundle.payload.source_domain) {
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
    if !is_supported_domain(bundle.payload.dest_domain)
        || bundle.payload.dest_domain == bundle.payload.source_domain
        || bundle.payload.dest_domain == 0 && bundle.payload.source_domain == 0
    {
        return false;
    }
    if bundle.commitment.version != 1
        || bundle.commitment.kind != SccpHubMessageKind::Burn
        || bundle.commitment.target_domain != bundle.payload.dest_domain
        || bundle.commitment.message_id != burn_message_id(&bundle.payload)
        || bundle.commitment.payload_hash
            != payload_hash(&canonical_burn_payload_bytes(&bundle.payload))
        || bundle.commitment.parliament_certificate_hash.is_some()
    {
        return false;
    }
    merkle_root_from_commitment(&bundle.commitment, &bundle.merkle_proof) == bundle.commitment_root
}

pub fn verify_governance_bundle_structure(bundle: &NexusSccpGovernanceProofV1) -> bool {
    if bundle.version != 1 || bundle.commitment.version != 1 {
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
    let Some(certificate) = decode_nexus_parliament_certificate(&bundle.parliament_certificate)
    else {
        return false;
    };
    if !verify_nexus_parliament_certificate_structure(
        &certificate,
        &canonical_governance_payload_bytes(&bundle.payload),
        finality_proof.height,
    ) {
        return false;
    }

    let expected_kind = match bundle.payload {
        GovernancePayloadV1::Add(_) => SccpHubMessageKind::TokenAdd,
        GovernancePayloadV1::Pause(_) => SccpHubMessageKind::TokenPause,
        GovernancePayloadV1::Resume(_) => SccpHubMessageKind::TokenResume,
    };
    let target_domain = governance_target_domain(&bundle.payload);
    if !is_supported_domain(target_domain)
        || bundle.commitment.kind != expected_kind
        || bundle.commitment.target_domain != target_domain
        || bundle.commitment.message_id != governance_message_id(&bundle.payload)
        || bundle.commitment.payload_hash
            != payload_hash(&canonical_governance_payload_bytes(&bundle.payload))
        || bundle.commitment.parliament_certificate_hash
            != Some(parliament_certificate_hash(&bundle.parliament_certificate))
    {
        return false;
    }

    merkle_root_from_commitment(&bundle.commitment, &bundle.merkle_proof) == bundle.commitment_root
}

pub fn verify_message_bundle_structure(bundle: &NexusSccpMessageProofV1) -> bool {
    if bundle.version != 1 || bundle.commitment.version != 1 {
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

    let target_domain = sccp_message_target_domain(&bundle.payload);
    let payload_bytes = canonical_sccp_payload_bytes(&bundle.payload);
    if !verify_sccp_payload_structure(&bundle.payload)
        || bundle.commitment.kind != sccp_message_kind(&bundle.payload)
        || bundle.commitment.target_domain != target_domain
        || bundle.commitment.message_id != sccp_message_id(&bundle.payload)
        || bundle.commitment.payload_hash != payload_hash(&payload_bytes)
        || bundle.commitment.parliament_certificate_hash.is_some()
    {
        return false;
    }

    merkle_root_from_commitment(&bundle.commitment, &bundle.merkle_proof) == bundle.commitment_root
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

fn iroha_consensus_domain(
    chain_id: &str,
    message_type_tag: &str,
    extra: &[u8],
    mode_tag: &str,
) -> H256 {
    let mut hasher = Blake2bVar::new(32).expect("fixed hash length");
    hasher.update(b"iroha2-consensus/v2");
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

    fn sample_parliament_certificate(payload: &GovernancePayloadV1) -> Vec<u8> {
        to_bytes(&NexusParliamentCertificateV1 {
            version: 1,
            preimage_hash: payload_hash(&canonical_governance_payload_bytes(payload)),
            enactment_window_start: 1,
            enactment_window_end: 10,
            payload_bytes: vec![9u8; 16],
            signature_scheme: NexusParliamentSignatureSchemeV1::SimpleThreshold,
            roster_epoch: 0,
            roster_members: vec![NexusParliamentRosterMemberV1 {
                signer:
                    "i105:01:ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2"
                        .to_owned(),
                public_keys: vec![
                    "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2"
                        .to_owned(),
                ],
            }],
            required_signatures: 1,
            signatures: vec![NexusParliamentSignatureV1 {
                signer:
                    "i105:01:ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2"
                        .to_owned(),
                public_key:
                    "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2"
                        .to_owned(),
                signature: vec![3u8; 64],
            }],
        })
        .expect("encode parliament certificate")
    }

    fn sample_message_bundle(payload: SccpPayloadV1) -> NexusSccpMessageProofV1 {
        let commitment = SccpHubCommitmentV1 {
            version: 1,
            kind: sccp_message_kind(&payload),
            target_domain: sccp_message_target_domain(&payload),
            message_id: sccp_message_id(&payload),
            payload_hash: payload_hash(&canonical_sccp_payload_bytes(&payload)),
            parliament_certificate_hash: None,
        };
        let commitment_root = commitment_leaf_hash(&commitment);
        NexusSccpMessageProofV1 {
            version: 1,
            commitment_root,
            commitment,
            merkle_proof: SccpMerkleProofV1 { steps: Vec::new() },
            payload,
            finality_proof: sample_finality_proof(commitment_root),
        }
    }

    fn sample_governance_bundle(payload: GovernancePayloadV1) -> NexusSccpGovernanceProofV1 {
        let parliament_certificate = sample_parliament_certificate(&payload);
        let commitment = SccpHubCommitmentV1 {
            version: 1,
            kind: match &payload {
                GovernancePayloadV1::Add(_) => SccpHubMessageKind::TokenAdd,
                GovernancePayloadV1::Pause(_) => SccpHubMessageKind::TokenPause,
                GovernancePayloadV1::Resume(_) => SccpHubMessageKind::TokenResume,
            },
            target_domain: governance_target_domain(&payload),
            message_id: governance_message_id(&payload),
            payload_hash: payload_hash(&canonical_governance_payload_bytes(&payload)),
            parliament_certificate_hash: Some(parliament_certificate_hash(&parliament_certificate)),
        };
        let commitment_root = commitment_leaf_hash(&commitment);
        NexusSccpGovernanceProofV1 {
            version: 1,
            commitment_root,
            commitment,
            merkle_proof: SccpMerkleProofV1 { steps: Vec::new() },
            payload,
            parliament_certificate,
            finality_proof: sample_finality_proof(commitment_root),
        }
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
            parliament_certificate_hash: None,
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
    fn governance_bundle_rejects_wrong_certificate_hash() {
        let payload = GovernancePayloadV1::Pause(TokenControlPayloadV1 {
            version: 1,
            target_domain: SCCP_DOMAIN_SORA,
            nonce: 3,
            sora_asset_id: [7u8; 32],
        });
        let commitment = SccpHubCommitmentV1 {
            version: 1,
            kind: SccpHubMessageKind::TokenPause,
            target_domain: SCCP_DOMAIN_SORA,
            message_id: governance_message_id(&payload),
            payload_hash: payload_hash(&canonical_governance_payload_bytes(&payload)),
            parliament_certificate_hash: Some([9u8; 32]),
        };
        let commitment_root = commitment_leaf_hash(&commitment);
        let parliament_certificate = sample_parliament_certificate(&payload);
        let bundle = NexusSccpGovernanceProofV1 {
            version: 1,
            commitment_root,
            commitment,
            merkle_proof: SccpMerkleProofV1 { steps: Vec::new() },
            payload,
            parliament_certificate,
            finality_proof: sample_finality_proof(commitment_root),
        };
        assert!(!verify_governance_bundle_structure(&bundle));
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
            sender_codec: SCCP_CODEC_TEXT_UTF8,
            sender: b"alice".to_vec(),
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
        assert!(envelope.parliament_certificate.is_none());
        let finality =
            decode_nexus_bridge_finality_proof(&bundle.finality_proof).expect("decode finality");
        assert_eq!(envelope.finality_proof.epoch, finality.commit_qc.epoch);
        assert_eq!(envelope.finality_proof.signature_count, 1);
        assert_eq!(
            envelope.finality_proof.validator_set_hash,
            sccp_runtime_validator_set_anchor_hash(&finality.commit_qc)
        );
        assert_eq!(
            sccp_runtime_envelope_bytes_from_message_bundle(&bundle),
            Some(encode_sccp_runtime_proof_envelope(&envelope))
        );
    }

    #[test]
    fn runtime_envelope_from_governance_bundle_exports_parliament_anchor_fields() {
        let payload = GovernancePayloadV1::Pause(TokenControlPayloadV1 {
            version: 1,
            target_domain: SCCP_DOMAIN_SORA2,
            nonce: 10,
            sora_asset_id: [0x42; 32],
        });
        let bundle = sample_governance_bundle(payload);
        let envelope =
            sccp_runtime_envelope_from_governance_bundle(&bundle).expect("runtime envelope");
        assert_eq!(envelope.commitment.kind, SccpRuntimeProofKindV1::TokenPause);
        assert_eq!(
            envelope.commitment.parliament_certificate_hash,
            Some(parliament_certificate_hash(&bundle.parliament_certificate))
        );
        let certificate = decode_nexus_parliament_certificate(&bundle.parliament_certificate)
            .expect("decode parliament certificate");
        let runtime_certificate = envelope
            .parliament_certificate
            .expect("runtime parliament certificate");
        assert_eq!(runtime_certificate.preimage_hash, certificate.preimage_hash);
        assert_eq!(
            runtime_certificate.roster_hash,
            sccp_runtime_parliament_roster_anchor_hash(&certificate)
        );
        assert_eq!(runtime_certificate.signature_count, 1);
        assert_eq!(
            sccp_runtime_envelope_bytes_from_governance_bundle(&bundle),
            Some(encode_sccp_runtime_proof_envelope(&envelope))
        );
        let encoded = encode_sccp_runtime_proof_envelope(&envelope);
        let expected_len = 1
            + 32
            + (1 + 1 + 4 + 32 + 32 + 1 + 32)
            + 1
            + (1 + 4 + 8 + 32)
            + (1 + 8 + 8 + 32 + 32 + 32 + 2)
            + 1
            + (1 + 32 + 8 + 8 + 8 + 32 + 2 + 2 + 32);
        assert_eq!(encoded.len(), expected_len);
    }

    #[test]
    fn governance_payload_canonical_encoding_preserves_discriminants() {
        let add = GovernancePayloadV1::Add(TokenAddPayloadV1 {
            version: 1,
            target_domain: SCCP_DOMAIN_SORA,
            nonce: 1,
            sora_asset_id: [0x11; 32],
            decimals: 18,
            name: [0x22; 32],
            symbol: [0x33; 32],
        });
        let pause = GovernancePayloadV1::Pause(TokenControlPayloadV1 {
            version: 1,
            target_domain: SCCP_DOMAIN_ETH,
            nonce: 2,
            sora_asset_id: [0x44; 32],
        });
        let resume = GovernancePayloadV1::Resume(TokenControlPayloadV1 {
            version: 1,
            target_domain: SCCP_DOMAIN_BSC,
            nonce: 3,
            sora_asset_id: [0x55; 32],
        });

        for (payload, discriminant) in [
            (add, GovernancePayloadV1::ADD_DISCRIMINANT),
            (pause, GovernancePayloadV1::PAUSE_DISCRIMINANT),
            (resume, GovernancePayloadV1::RESUME_DISCRIMINANT),
        ] {
            let encoded = canonical_governance_payload_bytes(&payload);
            assert_eq!(encoded.first(), Some(&discriminant));
        }
    }

    #[test]
    fn burn_bundle_rejects_mismatched_finality_root() {
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
            parliament_certificate_hash: None,
        };
        let commitment_root = commitment_leaf_hash(&commitment);
        let bundle = NexusSccpBurnProofV1 {
            version: 1,
            commitment_root,
            commitment,
            merkle_proof: SccpMerkleProofV1 { steps: Vec::new() },
            payload,
            finality_proof: sample_finality_proof([0xabu8; 32]),
        };
        assert!(!verify_burn_bundle_structure(&bundle));
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
                        envelope_encoding: "ton_message_body_v1".to_owned(),
                        submission_kind: manifest.submission_template.submission_kind.clone(),
                        verifier_entrypoint: manifest
                            .submission_template
                            .verifier_entrypoint
                            .clone(),
                        platform_payload: SccpPlatformSubmissionPayloadV1::TonInternalMessage(
                            SccpTonInternalMessageSubmissionPayloadV1 {
                                proof_cell: proof_bytes.clone(),
                                public_inputs_cell:
                                    canonical_sccp_message_transparent_public_inputs_bytes(
                                        &public_inputs,
                                    ),
                                bundle_cell: canonical_nexus_sccp_message_bundle_bytes(&bundle),
                            },
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
    fn proof_manifests_mark_current_counterparty_lanes_non_production() {
        for domain in SCCP_CORE_REMOTE_DOMAINS {
            let manifest = sccp_proof_manifest_for_domain(domain).expect("manifest");
            assert!(!manifest.production_ready);
            assert_eq!(
                manifest.disabled_reason.as_deref(),
                sccp_lane_disabled_reason_for_domain(domain)
            );
            assert!(!manifest.destination_rollout.immutable_verifier_ready);
            assert!(!manifest.destination_rollout.anchors_ready);
            assert!(!manifest.destination_rollout.blockers.is_empty());
        }
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
                proof_family: manifest.proof_family,
                verifier_backend: manifest.verifier_backend,
                envelope_encoding: "ton_message_body_v1".to_owned(),
                submission_kind: manifest.submission_template.submission_kind.clone(),
                verifier_entrypoint: manifest.submission_template.verifier_entrypoint.clone(),
                platform_payload: SccpPlatformSubmissionPayloadV1::TonInternalMessage(
                    SccpTonInternalMessageSubmissionPayloadV1 {
                        proof_cell: vec![0xAA, 0xBB],
                        public_inputs_cell: vec![0xCC],
                        bundle_cell: vec![0xDD],
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
    fn transparent_message_proof_recovery_accepts_legacy_bundle_bytes() {
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
    fn message_bundle_structure_rejects_commitment_kind_and_parliament_hash_tampering() {
        let bundle = sample_message_bundle(SccpPayloadV1::Transfer(TransferPayloadV1 {
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

        let mut wrong_kind = bundle.clone();
        wrong_kind.commitment.kind = SccpHubMessageKind::RouteActivate;
        assert!(!verify_message_bundle_structure(&wrong_kind));

        let mut unexpected_parliament_hash = bundle;
        unexpected_parliament_hash
            .commitment
            .parliament_certificate_hash = Some([0x42; 32]);
        assert!(!verify_message_bundle_structure(
            &unexpected_parliament_hash
        ));
    }

    #[test]
    fn finality_proof_structure_rejects_bad_bitmap_and_empty_pop() {
        let commitment_root = [0x88; 32];
        let mut proof = decode_nexus_bridge_finality_proof(&sample_finality_proof(commitment_root))
            .expect("decode proof");

        proof.commit_qc.signers_bitmap = vec![0b0000_0010];
        assert!(!verify_nexus_bridge_finality_proof_structure(&proof));

        proof.commit_qc.signers_bitmap = vec![0b0000_0001];
        proof.commit_qc.validator_set_pops[0].clear();
        assert!(!verify_nexus_bridge_finality_proof_structure(&proof));
    }

    #[test]
    fn parliament_certificate_structure_rejects_duplicate_roster_and_signatures() {
        let payload = GovernancePayloadV1::Pause(TokenControlPayloadV1 {
            version: 1,
            target_domain: SCCP_DOMAIN_SORA,
            nonce: 32,
            sora_asset_id: [0x44; 32],
        });
        let encoded_payload = canonical_governance_payload_bytes(&payload);
        let certificate =
            decode_nexus_parliament_certificate(&sample_parliament_certificate(&payload))
                .expect("decode parliament certificate");

        let mut duplicate_roster = certificate.clone();
        duplicate_roster
            .roster_members
            .push(duplicate_roster.roster_members[0].clone());
        assert!(!verify_nexus_parliament_certificate_structure(
            &duplicate_roster,
            &encoded_payload,
            7
        ));

        let mut duplicate_signature = certificate;
        duplicate_signature
            .signatures
            .push(duplicate_signature.signatures[0].clone());
        assert!(!verify_nexus_parliament_certificate_structure(
            &duplicate_signature,
            &encoded_payload,
            7
        ));
    }

    #[test]
    #[allow(clippy::similar_names)]
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
    }

    #[test]
    #[allow(clippy::similar_names)]
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
            SCCP_EVM_SECP256K1_PROOF_BACKEND_V1
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
    }

    #[test]
    fn evm_submission_template_matches_in_repo_wrapper_contract_source() {
        const EVM_BRIDGE_SOL: &str =
            include_str!("../../../contracts/evm/sccp/SccpMessageBridge.sol");
        const EVM_VERIFIER_SOL: &str =
            include_str!("../../../contracts/evm/sccp/ISccpMessageVerifier.sol");

        let manifest = sccp_proof_manifest_for_domain(SCCP_DOMAIN_ETH).expect("eth manifest");

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
        let manifest = sccp_proof_manifest_for_domain(SCCP_DOMAIN_ETH).expect("eth manifest");
        let network_id = core::array::from_fn(|idx| idx as u8);
        let verifier_address = core::array::from_fn(|idx| 0x80u8.saturating_add(idx as u8));
        let bridge_address = core::array::from_fn(|idx| 0xA0u8.saturating_add(idx as u8));

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
        let manifest = sccp_proof_manifest_for_domain(SCCP_DOMAIN_ETH).expect("eth manifest");
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
            destination_binding: destination_binding.clone(),
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
        assert_eq!(manifest.submission_template.encoding, "ton_cell_v1");
        assert_eq!(
            manifest.submission_template.verifier_entrypoint,
            "op::submit_sccp_message_proof"
        );
    }
}
