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

pub const SCCP_CORE_REMOTE_DOMAINS: [u32; 7] = [
    SCCP_DOMAIN_ETH,
    SCCP_DOMAIN_BSC,
    SCCP_DOMAIN_SOL,
    SCCP_DOMAIN_TON,
    SCCP_DOMAIN_TRON,
    SCCP_DOMAIN_SORA_KUSAMA,
    SCCP_DOMAIN_SORA_POLKADOT,
];

pub const SCCP_MSG_PREFIX_BURN_V1: &[u8] = b"sccp:burn:v1";
pub const SCCP_MSG_PREFIX_TOKEN_ADD_V1: &[u8] = b"sccp:token:add:v1";
pub const SCCP_MSG_PREFIX_TOKEN_PAUSE_V1: &[u8] = b"sccp:token:pause:v1";
pub const SCCP_MSG_PREFIX_TOKEN_RESUME_V1: &[u8] = b"sccp:token:resume:v1";
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
        if raw.len() % 2 != 0 {
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
        use super::{
            Deserialize, Deserializer, Serializer, String, decode_hex_fixed, encode_hex,
        };

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
    )
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
        },
    );
    push_u32(&mut out, commitment.target_domain);
    out.extend_from_slice(&commitment.message_id);
    out.extend_from_slice(&commitment.payload_hash);
    push_option_h256(&mut out, commitment.parliament_certificate_hash.as_ref());
    out
}

pub fn burn_message_id(payload: &BurnPayloadV1) -> H256 {
    prefixed_keccak(SCCP_MSG_PREFIX_BURN_V1, &canonical_burn_payload_bytes(payload))
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
}
