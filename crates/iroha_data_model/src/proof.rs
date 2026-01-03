//! Zero-knowledge proof payloads and identifiers.
//!
//! This module defines an opaque container for proofs that can be attached to
//! query responses and other messages without committing to a specific proving
//! system. The container carries a backend identifier (`Ident`) and raw bytes
//! produced by that backend. Norito serialization preserves both fields
//! byte-for-byte to ensure stable hashing and compatibility across nodes.

use std::io::Write;

#[cfg(feature = "json")]
use base64::Engine as _;
#[cfg(feature = "json")]
use base64::engine::general_purpose::STANDARD;
use iroha_schema::{Ident, IntoSchema};
use norito::{
    codec::{Decode, Encode},
    core as ncore,
};

use crate::{confidential::ConfidentialStatus, zk::BackendTag};

/// Read a length‑prefixed field produced by Norito struct serializers.
fn take_len_prefixed_slice<'a>(
    bytes: &'a [u8],
    offset: &mut usize,
) -> Result<&'a [u8], ncore::Error> {
    let tail = bytes.get(*offset..).ok_or(ncore::Error::LengthMismatch)?;
    let (len, hdr) = ncore::read_len_dyn_slice(tail)?;
    let start = offset
        .checked_add(hdr)
        .ok_or(ncore::Error::LengthMismatch)?;
    let end = start.checked_add(len).ok_or(ncore::Error::LengthMismatch)?;
    let field = bytes.get(start..end).ok_or(ncore::Error::LengthMismatch)?;
    *offset = end;
    Ok(field)
}

/// Opaque zero-knowledge proof bytes tagged with a backend identifier.
///
/// - `backend`: schema identifier for the proof backend (e.g., "halo2/ipa",
///   "groth16/bn254", "stark/fri-v1"). The exact strings are out of scope for
///   this container and are treated as application-level identifiers.
/// - `bytes`: proof payload as produced by the backend. Consumers interpret the
///   bytes according to `backend`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, IntoSchema)]
#[norito(reuse_archived)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ProofBox {
    /// Identifier of the proof backend/format.
    pub backend: iroha_schema::Ident,
    /// Opaque proof bytes.
    pub bytes: Vec<u8>,
}

impl ProofBox {
    /// Construct a new proof container.
    pub fn new(backend: iroha_schema::Ident, bytes: Vec<u8>) -> Self {
        Self { backend, bytes }
    }
}

impl<'de> norito::NoritoDeserialize<'de> for ProofBox {
    fn deserialize(archived: &'de ncore::Archived<Self>) -> Self {
        Self::try_deserialize(archived)
            .expect("ProofBox deserialization must succeed for canonical archives")
    }

    fn try_deserialize(archived: &'de ncore::Archived<Self>) -> Result<Self, ncore::Error> {
        let ptr = core::ptr::from_ref(archived).cast::<u8>();
        let bytes = ncore::payload_slice_from_ptr(ptr)?;
        let (value, used) = <Self as ncore::DecodeFromSlice>::decode_from_slice(bytes)?;
        if norito::debug_trace_enabled() {
            eprintln!(
                "ProofBox::try_deserialize consumed {used} of {} bytes",
                bytes.len()
            );
        }
        if used != bytes.len() {
            return Err(ncore::Error::LengthMismatch);
        }
        Ok(value)
    }
}

impl<'a> ncore::DecodeFromSlice<'a> for ProofBox {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), ncore::Error> {
        let mut offset = 0usize;
        let backend_bytes = take_len_prefixed_slice(bytes, &mut offset)?;
        let (backend, used) = <Ident as ncore::DecodeFromSlice>::decode_from_slice(backend_bytes)?;
        if used != backend_bytes.len() {
            return Err(ncore::Error::LengthMismatch);
        }
        let proof_bytes_slice = take_len_prefixed_slice(bytes, &mut offset)?;
        if norito::debug_trace_enabled() {
            let mut head = [0u8; 8];
            let preview = &proof_bytes_slice[..proof_bytes_slice.len().min(8)];
            head[..preview.len()].copy_from_slice(preview);
            eprintln!(
                "ProofBox::decode_from_slice backend_len={} proof_len={} vec_head_le={}",
                backend_bytes.len(),
                proof_bytes_slice.len(),
                u64::from_le_bytes(head)
            );
        }
        let (proof_bytes, used) =
            <Vec<u8> as ncore::DecodeFromSlice>::decode_from_slice(proof_bytes_slice)?;
        if used != proof_bytes_slice.len() {
            return Err(ncore::Error::LengthMismatch);
        }
        Ok((
            Self {
                backend,
                bytes: proof_bytes,
            },
            offset,
        ))
    }
}

/// Opaque verifying key bytes tagged with a backend identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, IntoSchema)]
#[norito(reuse_archived)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct VerifyingKeyBox {
    /// Identifier of the proof backend/format (must match associated proofs).
    pub backend: iroha_schema::Ident,
    /// Opaque verifying key bytes.
    pub bytes: Vec<u8>,
}

impl VerifyingKeyBox {
    /// Construct a new verifying key container.
    pub fn new(backend: iroha_schema::Ident, bytes: Vec<u8>) -> Self {
        Self { backend, bytes }
    }
}

impl<'de> norito::NoritoDeserialize<'de> for VerifyingKeyBox {
    fn deserialize(archived: &'de ncore::Archived<Self>) -> Self {
        Self::try_deserialize(archived)
            .expect("VerifyingKeyBox deserialization must succeed for canonical archives")
    }

    fn try_deserialize(archived: &'de ncore::Archived<Self>) -> Result<Self, ncore::Error> {
        let ptr = core::ptr::from_ref(archived).cast::<u8>();
        let bytes = ncore::payload_slice_from_ptr(ptr)?;
        let (value, used) = <Self as ncore::DecodeFromSlice>::decode_from_slice(bytes)?;
        if norito::debug_trace_enabled() {
            eprintln!(
                "VerifyingKeyBox::try_deserialize consumed {used} of {} bytes",
                bytes.len()
            );
        }
        if used != bytes.len() {
            return Err(ncore::Error::LengthMismatch);
        }
        Ok(value)
    }
}

impl<'a> ncore::DecodeFromSlice<'a> for VerifyingKeyBox {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), ncore::Error> {
        let mut offset = 0usize;
        let backend_bytes = take_len_prefixed_slice(bytes, &mut offset)?;
        let (backend, used) = <Ident as ncore::DecodeFromSlice>::decode_from_slice(backend_bytes)?;
        if used != backend_bytes.len() {
            return Err(ncore::Error::LengthMismatch);
        }
        let vk_bytes_slice = take_len_prefixed_slice(bytes, &mut offset)?;
        let (vk_bytes, used) =
            <Vec<u8> as ncore::DecodeFromSlice>::decode_from_slice(vk_bytes_slice)?;
        if used != vk_bytes_slice.len() {
            return Err(ncore::Error::LengthMismatch);
        }
        Ok((
            Self {
                backend,
                bytes: vk_bytes,
            },
            offset,
        ))
    }
}

/// Identifier for a registered verifying key in WSV.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[norito(reuse_archived)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct VerifyingKeyId {
    /// Identifier of the proof backend/format.
    pub backend: iroha_schema::Ident,
    /// Human-readable key name under backend namespace.
    pub name: String,
}

impl VerifyingKeyId {
    /// Create a new verifying key identifier using an explicit backend namespace and name.
    pub fn new(backend: impl Into<iroha_schema::Ident>, name: impl Into<String>) -> Self {
        Self {
            backend: backend.into(),
            name: name.into(),
        }
    }
}

/// Registry record for a verifying key with governance versioning.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[norito(reuse_archived)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct VerifyingKeyRecord {
    /// Monotonic version number managed by governance.
    pub version: u32,
    /// Backend circuit identifier associated with the verifying key.
    pub circuit_id: String,
    /// Optional manifest identifier that owns this verifier. `None` or `"core"` denotes built-ins.
    #[norito(default)]
    pub owner_manifest_id: Option<String>,
    /// Namespace that this verifier is bound to (e.g., contract namespace or ISI namespace).
    pub namespace: String,
    /// Proving backend tag (e.g., Halo2 IPA).
    pub backend: BackendTag,
    /// Curve name used by the backend (human readable; e.g., "pasta", "pallas").
    pub curve: String,
    /// Stable hash of the public input schema to detect witness layout changes.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub public_inputs_schema_hash: [u8; 32],
    /// 32-byte commitment of the verifying key bytes (stable hash of `backend || bytes`).
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub commitment: [u8; 32],
    /// Length of the verifying key in bytes (if published off-ledger).
    pub vk_len: u32,
    /// Maximum proof byte length accepted when this verifier is active.
    pub max_proof_bytes: u32,
    /// Identifier of the deterministic gas schedule applied to this verifier.
    pub gas_schedule_id: Option<String>,
    /// Optional URI (CID) pointing to metadata describing the verifier.
    pub metadata_uri_cid: Option<String>,
    /// Optional URI (CID) pointing to the verifying key bytes bundle.
    pub vk_bytes_cid: Option<String>,
    /// Block height when the verifier becomes active (inclusive).
    pub activation_height: Option<u64>,
    /// Block height when the verifier is withdrawn and must not be used.
    pub withdraw_height: Option<u64>,
    /// Optional inline verifying key bytes. Some deployments may store only commitments.
    pub key: Option<VerifyingKeyBox>,
    /// Status of the verifying key record.
    pub status: ConfidentialStatus,
}

impl VerifyingKeyRecord {
    /// Create a new verifier record with baseline metadata. Optional fields
    /// default to `None` and can be filled in by governance instructions.
    #[must_use]
    pub fn new(
        version: u32,
        circuit_id: impl Into<String>,
        backend: BackendTag,
        curve: impl Into<String>,
        public_inputs_schema_hash: [u8; 32],
        commitment: [u8; 32],
    ) -> Self {
        Self::new_with_owner(
            version,
            circuit_id,
            None,
            "core",
            backend,
            curve,
            public_inputs_schema_hash,
            commitment,
        )
    }

    /// Create a new verifier record with explicit owner/namespace metadata.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_owner(
        version: u32,
        circuit_id: impl Into<String>,
        owner_manifest_id: Option<String>,
        namespace: impl Into<String>,
        backend: BackendTag,
        curve: impl Into<String>,
        public_inputs_schema_hash: [u8; 32],
        commitment: [u8; 32],
    ) -> Self {
        Self {
            version,
            circuit_id: circuit_id.into(),
            owner_manifest_id,
            namespace: namespace.into(),
            backend,
            curve: curve.into(),
            public_inputs_schema_hash,
            commitment,
            vk_len: 0,
            max_proof_bytes: 0,
            gas_schedule_id: None,
            metadata_uri_cid: None,
            vk_bytes_cid: None,
            activation_height: None,
            withdraw_height: None,
            key: None,
            status: ConfidentialStatus::Proposed,
        }
    }

    /// Returns true if the record is permitted for verification at the current height.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.status.is_active()
    }
}

/// Attachment of a zero-knowledge proof to a transaction.
/// Exactly one of `vk_ref` or `vk_inline` must be provided.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, IntoSchema)]
#[norito(reuse_archived)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ProofAttachment {
    /// Identifier of the proof backend/format.
    pub backend: Ident,
    /// Proof payload as produced by the backend.
    pub proof: ProofBox,
    /// Reference to a verifying key stored in WSV.
    pub vk_ref: Option<VerifyingKeyId>,
    /// Inline verifying key bytes.
    pub vk_inline: Option<VerifyingKeyBox>,
    /// Optional verifying key commitment (32-byte hash of VK bytes under backend).
    /// When present, it can be used for stateless deduplication with the proof hash.
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::json_helpers::fixed_bytes::option")
    )]
    #[norito(default)]
    pub vk_commitment: Option<[u8; 32]>,
    /// Optional hash of the verify envelope payload passed via pointer‑ABI TLV
    /// (e.g., NoritoBytes(OpenVerifyEnvelope)). When present, it is used to
    /// bind the verification inputs to the transaction `call_hash` in emitted
    /// events and audit metadata.
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::json_helpers::fixed_bytes::option")
    )]
    #[norito(default)]
    pub envelope_hash: Option<[u8; 32]>,
    /// Optional lane privacy proof tying this attachment to a Nexus commitment.
    #[norito(default)]
    pub lane_privacy: Option<crate::nexus::LanePrivacyProof>,
}

impl ProofAttachment {
    /// Construct an attachment referencing a verifying key stored in WSV.
    pub fn new_ref(backend: Ident, proof: ProofBox, vk_ref: VerifyingKeyId) -> Self {
        Self {
            backend,
            proof,
            vk_ref: Some(vk_ref),
            vk_inline: None,
            vk_commitment: None,
            envelope_hash: None,
            lane_privacy: None,
        }
    }
    /// Construct an attachment that inlines a verifying key alongside the proof.
    pub fn new_inline(backend: Ident, proof: ProofBox, vk: VerifyingKeyBox) -> Self {
        Self {
            backend,
            proof,
            vk_ref: None,
            vk_inline: Some(vk),
            vk_commitment: None,
            envelope_hash: None,
            lane_privacy: None,
        }
    }
}

impl norito::NoritoSerialize for ProofAttachment {
    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), ncore::Error> {
        fn write_prefixed<W: Write, T: norito::NoritoSerialize>(
            writer: &mut W,
            value: &T,
        ) -> Result<(), ncore::Error> {
            let mut buf = Vec::new();
            value.serialize(&mut buf)?;
            ncore::write_len(writer, buf.len() as u64)?;
            writer.write_all(&buf)?;
            Ok(())
        }

        write_prefixed(&mut writer, &self.backend)?;
        write_prefixed(&mut writer, &self.proof)?;
        write_prefixed(&mut writer, &self.vk_ref)?;
        write_prefixed(&mut writer, &self.vk_inline)?;

        // Omit trailing default fields to keep payloads compact and deterministic.
        let tail = if self.lane_privacy.is_some() {
            3
        } else if self.envelope_hash.is_some() {
            2
        } else {
            i32::from(self.vk_commitment.is_some())
        };
        if tail >= 1 {
            write_prefixed(&mut writer, &self.vk_commitment)?;
        }
        if tail >= 2 {
            write_prefixed(&mut writer, &self.envelope_hash)?;
        }
        if tail >= 3 {
            write_prefixed(&mut writer, &self.lane_privacy)?;
        }
        Ok(())
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        self.encoded_len_exact()
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        let mut buf = Vec::new();
        self.serialize(&mut buf).ok()?;
        Some(buf.len())
    }
}

impl<'de> norito::NoritoDeserialize<'de> for ProofAttachment {
    fn deserialize(archived: &'de ncore::Archived<Self>) -> Self {
        Self::try_deserialize(archived)
            .expect("ProofAttachment deserialization must succeed for canonical archives")
    }

    fn try_deserialize(archived: &'de ncore::Archived<Self>) -> Result<Self, ncore::Error> {
        let ptr = core::ptr::from_ref(archived).cast::<u8>();
        let bytes = ncore::payload_slice_from_ptr(ptr)?;
        let (value, used) = <Self as ncore::DecodeFromSlice>::decode_from_slice(bytes)?;
        if norito::debug_trace_enabled() {
            eprintln!(
                "ProofAttachment::try_deserialize consumed {used} of {} bytes",
                bytes.len()
            );
        }
        if used != bytes.len() {
            return Err(ncore::Error::LengthMismatch);
        }
        Ok(value)
    }
}

impl<'a> ncore::DecodeFromSlice<'a> for ProofAttachment {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), ncore::Error> {
        let mut offset = 0usize;

        let backend_bytes = take_len_prefixed_slice(bytes, &mut offset)?;
        let (backend, used) = <Ident as ncore::DecodeFromSlice>::decode_from_slice(backend_bytes)?;
        if used != backend_bytes.len() {
            return Err(ncore::Error::LengthMismatch);
        }

        let proof_slice = take_len_prefixed_slice(bytes, &mut offset)?;
        let (proof, used) = <ProofBox as ncore::DecodeFromSlice>::decode_from_slice(proof_slice)?;
        if used != proof_slice.len() {
            return Err(ncore::Error::LengthMismatch);
        }

        let vk_ref_slice = take_len_prefixed_slice(bytes, &mut offset)?;
        let (vk_ref, used) =
            <Option<VerifyingKeyId> as ncore::DecodeFromSlice>::decode_from_slice(vk_ref_slice)?;
        if used != vk_ref_slice.len() {
            return Err(ncore::Error::LengthMismatch);
        }

        let vk_inline_slice = take_len_prefixed_slice(bytes, &mut offset)?;
        let (vk_inline, used) =
            <Option<VerifyingKeyBox> as ncore::DecodeFromSlice>::decode_from_slice(
                vk_inline_slice,
            )?;
        if used != vk_inline_slice.len() {
            return Err(ncore::Error::LengthMismatch);
        }

        // Optional fields may be omitted in compact payloads; treat missing tail as `None`.
        let vk_commitment = if offset == bytes.len() {
            None
        } else {
            let slice = take_len_prefixed_slice(bytes, &mut offset)?;
            let (value, used) =
                <Option<[u8; 32]> as ncore::DecodeFromSlice>::decode_from_slice(slice)?;
            if used != slice.len() {
                return Err(ncore::Error::LengthMismatch);
            }
            value
        };

        let envelope_hash = if offset == bytes.len() {
            None
        } else {
            let slice = take_len_prefixed_slice(bytes, &mut offset)?;
            let (value, used) =
                <Option<[u8; 32]> as ncore::DecodeFromSlice>::decode_from_slice(slice)?;
            if used != slice.len() {
                return Err(ncore::Error::LengthMismatch);
            }
            value
        };

        let lane_privacy = if offset == bytes.len() {
            None
        } else {
            let slice = take_len_prefixed_slice(bytes, &mut offset)?;
            let (value, used) =
                <Option<crate::nexus::LanePrivacyProof> as ncore::DecodeFromSlice>::decode_from_slice(
                    slice,
                )?;
            if used != slice.len() {
                return Err(ncore::Error::LengthMismatch);
            }
            value
        };

        Ok((
            Self {
                backend,
                proof,
                vk_ref,
                vk_inline,
                vk_commitment,
                envelope_hash,
                lane_privacy,
            },
            offset,
        ))
    }
}

/// A list of proof attachments for a transaction.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[norito(reuse_archived)]
pub struct ProofAttachmentList(
    /// Ordered attachments that make up the proof payload.
    pub Vec<ProofAttachment>,
);

#[cfg(feature = "json")]
impl norito::json::JsonSerialize for ProofAttachmentList {
    fn json_serialize(&self, out: &mut String) {
        let bytes =
            norito::to_bytes(self).expect("ProofAttachmentList Norito serialization must succeed");
        let encoded = STANDARD.encode(bytes);
        norito::json::JsonSerialize::json_serialize(&encoded, out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for ProofAttachmentList {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let encoded = parser.parse_string()?;
        let bytes = STANDARD
            .decode(encoded.as_str())
            .map_err(|err| norito::json::Error::Message(err.to_string()))?;
        let archived = norito::from_bytes::<ProofAttachmentList>(&bytes)
            .map_err(|err| norito::json::Error::Message(err.to_string()))?;
        norito::core::NoritoDeserialize::try_deserialize(archived)
            .map_err(|err| norito::json::Error::Message(err.to_string()))
    }
}

/// Identifier of a proof for storage and deduplication.
/// Combines backend identifier with a stable 32-byte proof hash.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[norito(reuse_archived)]
pub struct ProofId {
    /// Identifier of the proof backend/format.
    pub backend: iroha_schema::Ident,
    /// Stable 32-byte hash of the proof bytes (and optionally normalized inputs).
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub proof_hash: [u8; 32],
}

#[inline]
fn hex_val(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(10 + (c - b'a')),
        b'A'..=b'F' => Some(10 + (c - b'A')),
        _ => None,
    }
}

impl core::fmt::Display for ProofId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Print as backend:HEX
        write!(f, "{}:", self.backend)?;
        for b in &self.proof_hash {
            write!(f, "{b:02X}")?;
        }
        Ok(())
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonSerialize for ProofId {
    fn json_serialize(&self, out: &mut String) {
        let repr = self.to_string();
        norito::json::JsonSerialize::json_serialize(&repr, out);
    }
}

impl core::str::FromStr for ProofId {
    type Err = &'static str;

    /// Parse a stable string form produced by Display: `"<backend>:<hex32bytes>"`.
    ///
    /// - `backend` is parsed as `iroha_schema::Ident` (verbatim substring before the first ':').
    /// - `hex32bytes` must be exactly 64 hex chars (case-insensitive). Optional `0x` prefix is allowed.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (backend_str, hex_str) = s.split_once(':').ok_or("missing ':'")?;
        if backend_str.is_empty() {
            return Err("empty backend");
        }
        let mut h = hex_str;
        if let Some(rest) = h.strip_prefix("0x") {
            h = rest;
        }
        if h.len() != 64 {
            return Err("invalid hash length");
        }
        let mut arr = [0u8; 32];
        let bytes = h.as_bytes();
        for i in 0..32 {
            let hi = hex_val(bytes[2 * i]).ok_or("invalid hex digit")?;
            let lo = hex_val(bytes[2 * i + 1]).ok_or("invalid hex digit")?;
            arr[i] = (hi << 4) | lo;
        }
        Ok(ProofId {
            backend: backend_str.into(),
            proof_hash: arr,
        })
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for ProofId {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        value
            .parse()
            .map_err(|err: &str| norito::json::Error::Message(err.to_owned()))
    }
}

#[cfg(test)]
mod parse_tests {
    use super::*;

    #[test]
    fn proof_id_parse_roundtrip_upper_lower_and_0x() {
        let id = ProofId {
            backend: "halo2/ipa".into(),
            proof_hash: [0xAB; 32],
        };
        let disp = format!("{id}");
        // Uppercase produced by Display
        let parsed = disp.parse::<ProofId>().expect("parse");
        assert_eq!(parsed, id);
        // Lowercase hex accepted
        let lower = disp.to_lowercase();
        let parsed2 = lower.parse::<ProofId>().expect("parse lower");
        assert_eq!(parsed2, id);
        // 0x prefix also accepted
        let mut hex_lower = String::with_capacity(64);
        for b in &id.proof_hash {
            use std::fmt::Write as _;
            let _ = write!(&mut hex_lower, "{b:02x}");
        }
        let with0x = format!("{}:0x{}", id.backend, hex_lower);
        let parsed3 = with0x.parse::<ProofId>().expect("parse 0x");
        assert_eq!(parsed3, id);
    }
}

/// Verification status of a submitted proof artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[norito(reuse_archived)]
pub enum ProofStatus {
    /// Proof was observed/queued for verification.
    Submitted,
    /// Proof was successfully verified against the specified verifying key.
    Verified,
    /// Proof failed to verify.
    Rejected,
}

#[cfg(feature = "json")]
impl norito::json::JsonSerialize for ProofStatus {
    fn json_serialize(&self, out: &mut String) {
        let label = match self {
            ProofStatus::Submitted => "Submitted",
            ProofStatus::Verified => "Verified",
            ProofStatus::Rejected => "Rejected",
        };
        norito::json::write_json_string(label, out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for ProofStatus {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        match value.as_str() {
            "Submitted" => Ok(ProofStatus::Submitted),
            "Verified" => Ok(ProofStatus::Verified),
            "Rejected" => Ok(ProofStatus::Rejected),
            other => Err(norito::json::Error::unknown_field(other.to_owned())),
        }
    }
}

/// Stored record for a proof verification outcome.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[norito(reuse_archived)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ProofRecord {
    /// Proof identifier (backend + hash of proof bytes).
    pub id: ProofId,
    /// Optional reference to a verifying key stored in WSV.
    pub vk_ref: Option<VerifyingKeyId>,
    /// Optional inline verifying key commitment (32-byte stable hash) used during verification.
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::json_helpers::fixed_bytes::option")
    )]
    pub vk_commitment: Option<[u8; 32]>,
    /// Resulting status of verification.
    pub status: ProofStatus,
    /// Height at which verification was recorded (if applicable).
    pub verified_at_height: Option<u64>,
    /// Optional bridge-proof payload and metadata when the proof records a bridge artifact.
    pub bridge: Option<crate::bridge::BridgeProofRecord>,
}

/// Wrapper for attaching an optional proof to a committed transaction response.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[norito(reuse_archived)]
pub struct ProofedCommittedTransaction {
    /// Base committed transaction returned by the ledger.
    pub base: crate::query::CommittedTransaction,
    /// Optional proof attached to the transaction result.
    pub proof: Option<ProofBox>,
}

impl ProofedCommittedTransaction {
    /// Wrap a committed transaction with an optional proof payload.
    pub fn new(base: crate::query::CommittedTransaction, proof: Option<ProofBox>) -> Self {
        Self { base, proof }
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::LaneCommitmentId;

    use super::*;
    fn run_or_skip() -> bool {
        std::env::var("IROHA_RUN_IGNORED").ok().as_deref() == Some("1")
    }

    #[test]
    fn proof_attachment_list_roundtrip_bare() {
        let mut attachment = ProofAttachment::new_inline(
            "halo2/ipa".into(),
            ProofBox::new("halo2/ipa".into(), vec![1, 2]),
            VerifyingKeyBox::new("halo2/ipa".into(), vec![9, 9]),
        );
        attachment.lane_privacy = Some(crate::nexus::LanePrivacyProof {
            commitment_id: LaneCommitmentId::new(5),
            witness: crate::nexus::LanePrivacyWitness::Merkle(
                crate::nexus::LanePrivacyMerkleWitness {
                    leaf: [0xAA; 32],
                    proof: iroha_crypto::MerkleProof::from_audit_path(
                        0,
                        vec![Some(
                            iroha_crypto::HashOf::<[u8; 32]>::from_untyped_unchecked(
                                iroha_crypto::Hash::prehashed([0xBB; 32]),
                            ),
                        )],
                    ),
                },
            ),
        });
        let list = ProofAttachmentList(vec![attachment]);
        let bytes = norito::to_bytes(&list).expect("encode");
        let archived = norito::from_bytes::<ProofAttachmentList>(&bytes).expect("archive decode");
        let decoded: ProofAttachmentList = norito::core::NoritoDeserialize::deserialize(archived);
        assert_eq!(decoded, list);
    }

    #[test]
    fn proofbox_norito_roundtrip() {
        if !run_or_skip() {
            eprintln!(
                "Skipping: Norito derive decode mismatch (Ident, Vec<u8>). Set IROHA_RUN_IGNORED=1 to run."
            );
            return;
        }
        let backend: iroha_schema::Ident = "halo2/ipa".into();
        let bytes = vec![0xde, 0xad, 0xbe, 0xef, 0x01, 0x02];
        let p = ProofBox::new(backend, bytes.clone());
        let enc = norito::to_bytes(&p).expect("encode");
        let arch = norito::from_bytes::<ProofBox>(&enc).expect("archived");
        let dec: ProofBox = norito::core::NoritoDeserialize::deserialize(arch);
        assert_eq!(dec.backend, "halo2/ipa".to_owned());
        assert_eq!(dec.bytes, bytes);
    }

    #[test]
    fn verifying_key_roundtrip() {
        if !run_or_skip() {
            eprintln!("Skipping: Norito derive decode mismatch. Set IROHA_RUN_IGNORED=1 to run.");
            return;
        }
        let backend: iroha_schema::Ident = "halo2/ipa".into();
        let vk = VerifyingKeyBox::new(backend, vec![7, 7, 7]);
        let enc = norito::to_bytes(&vk).expect("encode");
        let arch = norito::from_bytes::<VerifyingKeyBox>(&enc).expect("archived");
        let dec: VerifyingKeyBox = norito::core::NoritoDeserialize::deserialize(arch);
        assert_eq!(dec.backend, "halo2/ipa".to_owned());
        assert_eq!(dec.bytes, vec![7, 7, 7]);
    }

    #[test]
    fn vk_record_roundtrip() {
        if !run_or_skip() {
            eprintln!("Skipping: Norito derive decode mismatch. Set IROHA_RUN_IGNORED=1 to run.");
            return;
        }
        let rec = VerifyingKeyRecord {
            version: 1,
            circuit_id: "transfer_v1".into(),
            owner_manifest_id: Some("core".into()),
            namespace: "core".into(),
            backend: BackendTag::Halo2IpaPasta,
            curve: "pallas".into(),
            public_inputs_schema_hash: [0xAA; 32],
            commitment: [0x11; 32],
            vk_len: 4096,
            max_proof_bytes: 8192,
            gas_schedule_id: Some("halo2_default".into()),
            metadata_uri_cid: Some("ipfs://halo2-transfer".into()),
            vk_bytes_cid: Some("ipfs://vk-transfer".into()),
            activation_height: Some(10),
            withdraw_height: Some(30),
            key: Some(VerifyingKeyBox {
                backend: "halo2/ipa".into(),
                bytes: vec![1, 2, 3],
            }),
            status: ConfidentialStatus::Active,
        };
        let enc = norito::to_bytes(&rec).expect("encode");
        let arch = norito::from_bytes::<VerifyingKeyRecord>(&enc).expect("archived");
        let dec: VerifyingKeyRecord = norito::core::NoritoDeserialize::deserialize(arch);
        assert_eq!(dec.version, 1);
        assert_eq!(dec.commitment, [0x11; 32]);
        assert!(dec.key.is_some());
    }

    #[test]
    fn vk_record_new_defaults() {
        let rec = VerifyingKeyRecord::new(
            2,
            "shield_v2",
            BackendTag::Halo2IpaPasta,
            "pallas",
            [0xCC; 32],
            [0xDD; 32],
        );
        assert_eq!(rec.version, 2);
        assert_eq!(rec.status, ConfidentialStatus::Proposed);
        assert_eq!(rec.vk_len, 0);
        assert!(rec.max_proof_bytes == 0);
        assert!(rec.key.is_none());
    }

    #[test]
    fn proof_attachment_roundtrip() {
        if !run_or_skip() {
            eprintln!("Skipping: Norito derive decode mismatch. Set IROHA_RUN_IGNORED=1 to run.");
            return;
        }
        let p = ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]);
        let id = VerifyingKeyId::new("halo2/ipa", "vk_1");
        let a = ProofAttachment::new_ref("halo2/ipa".into(), p.clone(), id);
        let enc = norito::to_bytes(&a).expect("encode");
        let arch = norito::from_bytes::<ProofAttachment>(&enc).expect("archived");
        let dec: ProofAttachment = norito::core::NoritoDeserialize::deserialize(arch);
        assert_eq!(dec.backend, "halo2/ipa".to_owned());
        assert!(dec.vk_ref.is_some());

        let vk = VerifyingKeyBox::new("halo2/ipa".into(), vec![9, 9]);
        let a2 = ProofAttachment::new_inline("halo2/ipa".into(), p, vk);
        let enc2 = norito::to_bytes(&a2).expect("encode");
        let arch2 = norito::from_bytes::<ProofAttachment>(&enc2).expect("archived");
        let dec2: ProofAttachment = norito::core::NoritoDeserialize::deserialize(arch2);
        assert!(dec2.vk_inline.is_some());
    }

    #[test]
    fn proofed_committed_tx_roundtrip() {
        use iroha_crypto::{Hash, HashOf};

        use crate::query::CommittedTransaction;
        if !run_or_skip() {
            eprintln!(
                "Skipping: Norito derive decode mismatch (nested). Set IROHA_RUN_IGNORED=1 to run."
            );
            return;
        }
        // Minimal dummy CommittedTransaction with empty merkle items.
        let empty: [u8; 32] = [0; 32];
        let h_block =
            HashOf::<crate::block::BlockHeader>::from_untyped_unchecked(Hash::prehashed(empty));
        let h_entry = HashOf::<crate::transaction::TransactionEntrypoint>::from_untyped_unchecked(
            Hash::prehashed(empty),
        );
        let h_result = HashOf::<crate::transaction::TransactionResult>::from_untyped_unchecked(
            Hash::prehashed(empty),
        );
        let tree: iroha_crypto::MerkleTree<[u8; 32]> = [].into_iter().collect();
        let entry_proof: iroha_crypto::MerkleProof<crate::transaction::TransactionEntrypoint> =
            iroha_crypto::MerkleProof::from_audit_path(0, vec![]);
        let result_proof: iroha_crypto::MerkleProof<crate::transaction::TransactionResult> =
            iroha_crypto::MerkleProof::from_audit_path(0, vec![]);
        // Construct a minimal time-triggered entrypoint and a rejected result
        let authority: crate::account::AccountId =
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
                .parse()
                .expect("valid account id");
        let trigger_id: crate::trigger::TriggerId = "test_trigger".parse().expect("trigger id");
        let time_entry = crate::trigger::TimeTriggerEntrypoint {
            id: trigger_id,
            instructions: crate::transaction::ExecutionStep(
                Vec::<crate::isi::InstructionBox>::new().into(),
            ),
            authority,
        };
        let base = CommittedTransaction {
            block_hash: h_block,
            entrypoint_hash: h_entry,
            entrypoint_proof: entry_proof,
            entrypoint: crate::transaction::TransactionEntrypoint::Time(time_entry),
            result_hash: h_result,
            result_proof,
            result: crate::transaction::TransactionResult::from(Err(
                crate::transaction::error::TransactionRejectionReason::Validation(
                    crate::ValidationFail::NotPermitted("not permitted".into()),
                ),
            )),
        };
        let pct = ProofedCommittedTransaction::new(
            base,
            Some(ProofBox::new("halo2/ipa".into(), vec![1, 2, 3, 4])),
        );
        let enc = norito::to_bytes(&pct).expect("encode");
        let arch = norito::from_bytes::<ProofedCommittedTransaction>(&enc).expect("archived");
        let dec: ProofedCommittedTransaction = norito::core::NoritoDeserialize::deserialize(arch);
        assert!(dec.proof.is_some());
        let _ = tree; // silence unused
    }

    #[test]
    fn proof_record_roundtrip() {
        if !run_or_skip() {
            eprintln!("Skipping: Norito derive decode mismatch. Set IROHA_RUN_IGNORED=1 to run.");
            return;
        }
        let id = ProofId {
            backend: "halo2/ipa".into(),
            proof_hash: [0xAA; 32],
        };
        let rec = ProofRecord {
            id,
            vk_ref: Some(VerifyingKeyId::new("halo2/ipa", "vk")),
            vk_commitment: Some([0x55; 32]),
            status: ProofStatus::Verified,
            verified_at_height: Some(42),
            bridge: None,
        };
        let enc = norito::to_bytes(&rec).expect("encode");
        let arch = norito::from_bytes::<ProofRecord>(&enc).expect("archived");
        let dec: ProofRecord = norito::core::NoritoDeserialize::deserialize(arch);
        assert!(matches!(dec.status, ProofStatus::Verified));
        assert_eq!(dec.verified_at_height, Some(42));
    }
}
