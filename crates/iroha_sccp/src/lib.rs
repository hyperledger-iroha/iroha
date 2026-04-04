#![cfg_attr(not(feature = "std"), no_std)]
#![allow(missing_docs)]
#![allow(missing_copy_implementations)]

extern crate alloc;

use alloc::{string::String, vec::Vec};

use blake2::{
    Blake2bVar,
    digest::{Update, VariableOutput},
};
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

pub const SCCP_CODEC_TEXT_UTF8: u8 = 1;
pub const SCCP_CODEC_EVM_HEX: u8 = 2;
pub const SCCP_CODEC_SOLANA_BASE58: u8 = 3;
pub const SCCP_CODEC_TON_RAW: u8 = 4;
pub const SCCP_CODEC_TRON_BASE58CHECK: u8 = 5;

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

pub type H256 = [u8; 32];

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
    pub proof_family: String,
    pub message_backend: String,
    pub registry_backend: String,
    pub manifest_seed: String,
    pub finality_model: SccpProofFinalityModelV1,
    pub verifier_target: SccpProofVerifierTargetV1,
    pub public_inputs: SccpMessageTransparentPublicInputsV1,
    #[cfg_attr(feature = "serde", serde(with = "serde_utils::bytes_hex"))]
    pub proof_bytes: Vec<u8>,
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
    pub proof_family: String,
    pub message_backend: String,
    pub registry_backend: String,
    pub counterparty_account_codec: u8,
    pub counterparty_account_codec_key: String,
    pub finality_model: SccpProofFinalityModelV1,
    pub verifier_target: SccpProofVerifierTargetV1,
    pub manifest_seed: String,
    pub required_public_inputs: Vec<String>,
    pub message_payload_kinds: Vec<String>,
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
    )
}

pub fn sccp_codec_key(codec_id: u8) -> Option<&'static str> {
    match codec_id {
        SCCP_CODEC_TEXT_UTF8 => Some("text_utf8"),
        SCCP_CODEC_EVM_HEX => Some("evm_hex"),
        SCCP_CODEC_SOLANA_BASE58 => Some("solana_base58"),
        SCCP_CODEC_TON_RAW => Some("ton_raw"),
        SCCP_CODEC_TRON_BASE58CHECK => Some("tron_base58check"),
        _ => None,
    }
}

pub fn sccp_codec_description(codec_id: u8) -> Option<&'static str> {
    match codec_id {
        SCCP_CODEC_TEXT_UTF8 => Some("Logical UTF-8 identifiers for SORA and route-local names."),
        SCCP_CODEC_EVM_HEX => Some("0x-prefixed 20-byte EVM account addresses."),
        SCCP_CODEC_SOLANA_BASE58 => Some("Base58 Solana public keys."),
        SCCP_CODEC_TON_RAW => Some("TON raw addresses in workchain:account_hex form."),
        SCCP_CODEC_TRON_BASE58CHECK => Some("Tron base58check account addresses."),
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

pub fn sccp_message_payload_kind_keys_v1() -> Vec<String> {
    vec![
        "asset_register".to_owned(),
        "route_activate".to_owned(),
        "transfer".to_owned(),
    ]
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
        proof_family: SCCP_STARK_FRI_PROOF_FAMILY_V1.to_owned(),
        message_backend: sccp_message_backend_for_domain(domain)?,
        registry_backend: sccp_registry_backend_for_domain(domain)?,
        counterparty_account_codec,
        counterparty_account_codec_key: counterparty_account_codec_key.to_owned(),
        finality_model: sccp_proof_finality_model_for_domain(domain)?,
        verifier_target: sccp_proof_verifier_target_for_domain(domain)?,
        manifest_seed: sccp_manifest_seed_for_domain(domain)?,
        required_public_inputs: sccp_required_public_inputs_v1(),
        message_payload_kinds: sccp_message_payload_kind_keys_v1(),
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

pub fn sccp_message_transparent_placeholder_proof_bytes(
    bundle: &NexusSccpMessageProofV1,
    manifest: &SccpProofManifestV1,
) -> Option<Vec<u8>> {
    let public_inputs = sccp_message_transparent_public_inputs(bundle)?;
    let mut statement = Vec::new();
    push_u8(&mut statement, 1);
    push_u32(&mut statement, manifest.local_domain);
    push_u32(&mut statement, manifest.counterparty_domain);
    push_u8(
        &mut statement,
        sccp_proof_finality_model_code(manifest.finality_model),
    );
    push_u8(
        &mut statement,
        sccp_proof_verifier_target_code(manifest.verifier_target),
    );
    push_vec(&mut statement, manifest.proof_family.as_bytes());
    push_vec(&mut statement, manifest.message_backend.as_bytes());
    push_vec(&mut statement, manifest.registry_backend.as_bytes());
    push_vec(&mut statement, manifest.manifest_seed.as_bytes());
    statement.extend_from_slice(&canonical_sccp_message_transparent_public_inputs_bytes(
        &public_inputs,
    ));
    statement.extend_from_slice(&payload_hash(&canonical_sccp_payload_bytes(
        &bundle.payload,
    )));
    Some(
        prefixed_blake2b(b"sccp:transparent:placeholder:v1", &statement)
            .as_slice()
            .to_vec(),
    )
}

pub fn build_nexus_sccp_message_transparent_proof(
    bundle: &NexusSccpMessageProofV1,
) -> Option<NexusSccpMessageTransparentProofV1> {
    let counterparty_domain = sccp_counterparty_domain_for_message_payload(&bundle.payload)?;
    let manifest = sccp_proof_manifest_for_domain(counterparty_domain)?;
    let public_inputs = sccp_message_transparent_public_inputs(bundle)?;
    let proof_bytes = sccp_message_transparent_placeholder_proof_bytes(bundle, &manifest)?;
    Some(NexusSccpMessageTransparentProofV1 {
        version: 1,
        local_domain: manifest.local_domain,
        counterparty_domain,
        proof_family: manifest.proof_family,
        message_backend: manifest.message_backend,
        registry_backend: manifest.registry_backend,
        manifest_seed: manifest.manifest_seed,
        finality_model: manifest.finality_model,
        verifier_target: manifest.verifier_target,
        public_inputs,
        proof_bytes,
        bundle: bundle.clone(),
    })
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
    if proof.message_backend != manifest.message_backend
        || proof.registry_backend != manifest.registry_backend
        || proof.manifest_seed != manifest.manifest_seed
        || proof.finality_model != manifest.finality_model
        || proof.verifier_target != manifest.verifier_target
        || sccp_counterparty_domain_for_message_payload(&proof.bundle.payload)
            != Some(proof.counterparty_domain)
    {
        return false;
    }
    sccp_message_transparent_public_inputs(&proof.bundle)
        .is_some_and(|expected| expected == proof.public_inputs)
}

fn is_ascii_hex_digit(byte: u8) -> bool {
    byte.is_ascii_hexdigit()
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

fn validate_utf8_codec(bytes: &[u8]) -> bool {
    !bytes.is_empty() && core::str::from_utf8(bytes).is_ok()
}

fn validate_evm_hex_codec(bytes: &[u8]) -> bool {
    bytes.len() == 42
        && bytes.first() == Some(&b'0')
        && matches!(bytes.get(1), Some(b'x' | b'X'))
        && bytes[2..].iter().copied().all(is_ascii_hex_digit)
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
    !workchain.is_empty()
        && workchain.parse::<i32>().is_ok()
        && account.len() == 64
        && account.as_bytes().iter().copied().all(is_ascii_hex_digit)
}

fn validate_tron_base58_codec(bytes: &[u8]) -> bool {
    bytes.len() == 34
        && bytes.first() == Some(&b'T')
        && bytes.iter().copied().all(is_ascii_base58_digit)
}

fn validate_sccp_codec_bytes(codec_id: u8, bytes: &[u8]) -> bool {
    match codec_id {
        SCCP_CODEC_TEXT_UTF8 => validate_utf8_codec(bytes),
        SCCP_CODEC_EVM_HEX => validate_evm_hex_codec(bytes),
        SCCP_CODEC_SOLANA_BASE58 => validate_base58_codec(bytes, 32, 44),
        SCCP_CODEC_TON_RAW => validate_ton_raw_codec(bytes),
        SCCP_CODEC_TRON_BASE58CHECK => validate_tron_base58_codec(bytes),
        _ => false,
    }
}

fn push_u8(out: &mut Vec<u8>, value: u8) {
    out.push(value);
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
    push_u32(out, value.len() as u32);
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
    fn message_transparent_proof_builder_roundtrip_verifies() {
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

        let proof =
            build_nexus_sccp_message_transparent_proof(&bundle).expect("build transparent proof");
        assert!(verify_nexus_sccp_message_transparent_proof_structure(
            &proof
        ));
        assert_eq!(proof.counterparty_domain, SCCP_DOMAIN_TON);
        assert_eq!(proof.message_backend, "sccp/stark-fri-v1/ton");
        assert_eq!(proof.registry_backend, "bridge/sccp/stark-fri-v1/ton");
        assert_eq!(proof.public_inputs.message_id, bundle.commitment.message_id);
        assert_eq!(proof.public_inputs.commitment_root, bundle.commitment_root);
        assert!(!proof.proof_bytes.is_empty());

        let encoded = to_bytes(&proof).expect("encode transparent proof");
        let decoded = decode_nexus_sccp_message_transparent_proof(&encoded)
            .expect("decode transparent proof");
        assert_eq!(decoded, proof);
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

        let recovered =
            recover_nexus_sccp_message_transparent_proof("sccp/stark-fri-v1/eth", &legacy_bytes)
                .expect("recover legacy bundle as transparent proof");
        assert!(verify_nexus_sccp_message_transparent_proof_structure(
            &recovered
        ));
        assert_eq!(recovered.counterparty_domain, SCCP_DOMAIN_ETH);
        assert_eq!(recovered.bundle, bundle);
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
    fn commitment_merkle_helpers_support_multi_message_blocks() {
        let payloads = vec![
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
    fn codec_validation_accepts_chain_specific_v1_formats() {
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

        let ton_transfer = SccpPayloadV1::Transfer(TransferPayloadV1 {
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
        assert!(verify_sccp_payload_structure(&ton_transfer));

        let tron_transfer = SccpPayloadV1::Transfer(TransferPayloadV1 {
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
        assert!(verify_sccp_payload_structure(&tron_transfer));
    }

    #[test]
    fn codec_validation_rejects_malformed_chain_specific_v1_formats() {
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

        let bad_ton = SccpPayloadV1::Transfer(TransferPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_TON,
            nonce: 2,
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
        assert!(!verify_sccp_payload_structure(&bad_ton));

        let bad_tron = SccpPayloadV1::Transfer(TransferPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_SORA,
            dest_domain: SCCP_DOMAIN_TRON,
            nonce: 3,
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
        assert!(!verify_sccp_payload_structure(&bad_tron));
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
        assert_eq!(
            manifest.finality_model,
            SccpProofFinalityModelV1::TonMasterchain
        );
        assert_eq!(
            manifest.verifier_target,
            SccpProofVerifierTargetV1::TonContract
        );
    }
}
