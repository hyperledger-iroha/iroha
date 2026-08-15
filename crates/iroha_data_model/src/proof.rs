//! Zero-knowledge proof payloads and identifiers.
//!
//! This module defines an opaque container for proofs that can be attached to query responses and
//! other messages without committing to a specific proving system. The container carries a backend
//! identifier (`Ident`) and raw bytes produced by that backend. Norito serialization preserves both
//! fields byte-for-byte to ensure stable hashing and compatibility across nodes.
use crate::{confidential::ConfidentialStatus, zk::BackendTag};
#[cfg(feature = "json")]
use base64::Engine as _;
#[cfg(feature = "json")]
use base64::engine::general_purpose::STANDARD;
use iroha_schema::{Ident, IntoSchema};
use norito::{
    codec::{Decode, Encode},
    core as ncore,
};
use std::io::Write;
const MAX_BACKEND_FIELD_BYTES: usize = 4 * 1024;
const MAX_REF_FIELD_BYTES: usize = 16 * 1024;
/// Maximum canonical encoded size of a [`ProofBox`] nested in a proof attachment.
pub const PROOF_BOX_MAX_ENCODED_BYTES_V1: usize = 64 * 1024 * 1024;
const MAX_LEN_PREFIXED_FIELD_BYTES: usize = PROOF_BOX_MAX_ENCODED_BYTES_V1;
/// Maximum opaque payload bytes accepted in a first-release [`VerifyingKeyBox`].
pub const VERIFYING_KEY_BOX_MAX_PAYLOAD_BYTES_V1: usize = 8 * 1024 * 1024;
// A `Vec<u8>` value carries an advertised sequence length inside the enclosing
// struct-field frame. Leave bounded room for either fixed or compact Norito
// length headers while still rejecting attacker-sized fields before decoding.
const VERIFYING_KEY_BOX_MAX_FIELD_BYTES_V1: usize = VERIFYING_KEY_BOX_MAX_PAYLOAD_BYTES_V1 + 16;
/// Maximum byte length for portable verifier-key registry id fields.
pub const VERIFYING_KEY_ID_MAX_FIELD_BYTES: usize = 256;
/// Read a length‑prefixed field produced by Norito struct serializers.
fn take_len_prefixed_slice<'a>(
    bytes: &'a [u8],
    offset: &mut usize,
    max_len: usize,
) -> Result<&'a [u8], ncore::Error> {
    let tail = bytes.get(*offset..).ok_or(ncore::Error::LengthMismatch)?;
    let (len, hdr) = ncore::read_len_dyn_slice(tail)?;
    if len > max_len {
        return Err(ncore::Error::LengthMismatch);
    }
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
///   "groth16/bn254", "stark/fri"). The exact strings are out of scope for
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
fn proof_box_canonical_encoded_len_for_lengths_v1(
    backend_len: usize,
    proof_len: usize,
) -> Option<usize> {
    // `Ident` is a string (compact byte-length plus UTF-8 bytes), and every
    // struct member is itself compact-length framed. `Vec<u8>` retains the
    // fixed-width V1 sequence count inside its member frame. Derive every
    // prefix from Norito's canonical primitives so boundary transitions at
    // 2^7, 2^14, ... cannot drift from the serializer.
    let backend_value_len = ncore::varint_len_prefix_len(backend_len).checked_add(backend_len)?;
    if backend_value_len > MAX_BACKEND_FIELD_BYTES {
        return None;
    }
    let backend_field_len =
        ncore::varint_len_prefix_len(backend_value_len).checked_add(backend_value_len)?;
    let proof_value_len = ncore::seq_len_prefix_len(proof_len).checked_add(proof_len)?;
    let proof_field_len =
        ncore::varint_len_prefix_len(proof_value_len).checked_add(proof_value_len)?;
    backend_field_len.checked_add(proof_field_len)
}
/// Return the largest proof payload that keeps the complete canonical nested [`ProofBox`] payload
/// within [`PROOF_BOX_MAX_ENCODED_BYTES_V1`] for the supplied UTF-8 backend id.
///
/// `None` means the backend and mandatory canonical framing alone exceed the
/// closed first-release limit.
#[must_use]
pub fn proof_box_max_proof_bytes_v1(backend: &str) -> Option<usize> {
    if proof_box_canonical_encoded_len_for_lengths_v1(backend.len(), 0)?
        > PROOF_BOX_MAX_ENCODED_BYTES_V1
    {
        return None;
    }
    // Prefix widths make the exact size monotone but piecewise-linear. A
    // bounded binary search avoids duplicating those transition points and
    // never allocates proof storage.
    let mut lower = 0usize;
    let mut upper = PROOF_BOX_MAX_ENCODED_BYTES_V1;
    while lower < upper {
        let distance = upper - lower;
        let candidate = lower + distance / 2 + distance % 2;
        if proof_box_canonical_encoded_len_for_lengths_v1(backend.len(), candidate)
            .is_some_and(|length| length <= PROOF_BOX_MAX_ENCODED_BYTES_V1)
        {
            lower = candidate;
        } else {
            upper = candidate - 1;
        }
    }
    Some(lower)
}
impl ProofBox {
    /// Construct a new proof container.
    pub fn new(backend: iroha_schema::Ident, bytes: Vec<u8>) -> Self {
        Self { backend, bytes }
    }
    /// Return the exact canonical nested payload length of this proof box.
    #[must_use]
    pub fn canonical_encoded_len_v1(&self) -> Option<usize> {
        proof_box_canonical_encoded_len_for_lengths_v1(
            self.backend.as_str().len(),
            self.bytes.len(),
        )
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
        let backend_bytes = take_len_prefixed_slice(bytes, &mut offset, MAX_BACKEND_FIELD_BYTES)?;
        let (backend, used) = <Ident as ncore::DecodeFromSlice>::decode_from_slice(backend_bytes)?;
        if used != backend_bytes.len() {
            return Err(ncore::Error::LengthMismatch);
        }
        let proof_bytes_slice =
            take_len_prefixed_slice(bytes, &mut offset, MAX_LEN_PREFIXED_FIELD_BYTES)?;
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
        let backend_bytes = take_len_prefixed_slice(bytes, &mut offset, MAX_BACKEND_FIELD_BYTES)?;
        let (backend, used) = <Ident as ncore::DecodeFromSlice>::decode_from_slice(backend_bytes)?;
        if used != backend_bytes.len() {
            return Err(ncore::Error::LengthMismatch);
        }
        let vk_bytes_slice =
            take_len_prefixed_slice(bytes, &mut offset, VERIFYING_KEY_BOX_MAX_FIELD_BYTES_V1)?;
        let (declared_vk_len, _) = ncore::inspect_seq_len_slice(vk_bytes_slice)?;
        if declared_vk_len > VERIFYING_KEY_BOX_MAX_PAYLOAD_BYTES_V1 {
            return Err(ncore::Error::LengthMismatch);
        }
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
    /// Returns true when both id components use bounded portable registry syntax.
    #[must_use]
    pub fn is_portable_registry_id(&self) -> bool {
        verifying_key_id_field_is_portable(self.backend.as_str())
            && verifying_key_id_field_is_portable(&self.name)
    }
}
/// Returns true when a verifier-key registry id component is bounded and portable.
#[must_use]
pub fn verifying_key_id_field_is_portable(field: &str) -> bool {
    !field.is_empty()
        && field.len() <= VERIFYING_KEY_ID_MAX_FIELD_BYTES
        && crate::zk::open_verify_circuit_id_is_portable(field)
}
impl<'a> ncore::DecodeFromSlice<'a> for VerifyingKeyId {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), ncore::Error> {
        let mut offset = 0usize;
        let backend_bytes = take_len_prefixed_slice(bytes, &mut offset, MAX_BACKEND_FIELD_BYTES)?;
        let (backend, used) = <Ident as ncore::DecodeFromSlice>::decode_from_slice(backend_bytes)?;
        if used != backend_bytes.len() {
            return Err(ncore::Error::LengthMismatch);
        }
        let name_bytes = take_len_prefixed_slice(bytes, &mut offset, MAX_REF_FIELD_BYTES)?;
        let (name, used) = <String as ncore::DecodeFromSlice>::decode_from_slice(name_bytes)?;
        if used != name_bytes.len() {
            return Err(ncore::Error::LengthMismatch);
        }
        if offset != bytes.len() {
            return Err(ncore::Error::LengthMismatch);
        }
        Ok((Self { backend, name }, offset))
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
    #[cfg_attr(
        feature = "json",
        norito(
            with = "crate::json_helpers::fixed_bytes",
            bounded_with = "crate::json_helpers::fixed_bytes::serialize_bounded"
        )
    )]
    pub public_inputs_schema_hash: [u8; 32],
    /// 32-byte domain-separated commitment of the verifying key bytes and backend.
    #[cfg_attr(
        feature = "json",
        norito(
            with = "crate::json_helpers::fixed_bytes",
            bounded_with = "crate::json_helpers::fixed_bytes::serialize_bounded"
        )
    )]
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
    /// Optional stored verifying key bytes. Some deployments may store only commitments.
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
    /// Returns true if the record is permitted for verification at `height`.
    #[must_use]
    pub fn is_active_at(&self, height: u64) -> bool {
        self.is_active()
            && self
                .activation_height
                .is_none_or(|activation| height >= activation)
            && self
                .withdraw_height
                .is_none_or(|withdraw| height < withdraw)
    }
}
/// Attachment of a zero-knowledge proof to a transaction.
///
/// Proof attachments carry only a registry reference to the verifying key.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, IntoSchema)]
#[norito(reuse_archived)]
#[cfg_attr(feature = "json", derive(crate::DeriveJsonSerialize))]
pub struct ProofAttachment {
    /// Identifier of the proof backend/format.
    pub backend: Ident,
    /// Proof payload as produced by the backend.
    pub proof: ProofBox,
    /// Reference to a verifying key stored in WSV.
    pub vk_ref: VerifyingKeyId,
    /// Optional verifying key commitment (32-byte hash of VK bytes under backend).
    /// When present, it can be used for stateless deduplication with the proof hash.
    #[cfg_attr(
        feature = "json",
        norito(
            with = "crate::json_helpers::fixed_bytes::option",
            bounded_with = "crate::json_helpers::fixed_bytes::option::serialize_bounded"
        )
    )]
    #[cfg_attr(feature = "json", norito(skip_serializing_if = "Option::is_none"))]
    #[norito(default)]
    pub vk_commitment: Option<[u8; 32]>,
    /// Optional hash of the verify envelope payload passed via pointer‑ABI TLV (e.g.,
    /// NoritoBytes(OpenVerifyEnvelope)). When present, it is used to bind the verification inputs
    /// to the transaction `call_hash` in emitted events and audit metadata.
    #[cfg_attr(
        feature = "json",
        norito(
            with = "crate::json_helpers::fixed_bytes::option",
            bounded_with = "crate::json_helpers::fixed_bytes::option::serialize_bounded"
        )
    )]
    #[cfg_attr(feature = "json", norito(skip_serializing_if = "Option::is_none"))]
    #[norito(default)]
    pub envelope_hash: Option<[u8; 32]>,
    /// Optional lane privacy proof tying this attachment to a Nexus commitment.
    #[cfg_attr(feature = "json", norito(skip_serializing_if = "Option::is_none"))]
    #[norito(default)]
    pub lane_privacy: Option<crate::nexus::LanePrivacyProof>,
}
impl ProofAttachment {
    /// Construct an attachment referencing a verifying key stored in WSV.
    pub fn new_ref(backend: Ident, proof: ProofBox, vk_ref: VerifyingKeyId) -> Self {
        Self {
            backend,
            proof,
            vk_ref,
            vk_commitment: None,
            envelope_hash: None,
            lane_privacy: None,
        }
    }
    fn backend_consistency_error(&self) -> Option<&'static str> {
        if self.proof.backend != self.backend {
            Some("proof.backend")
        } else if self.vk_ref.backend != self.backend {
            Some("vk_ref.backend")
        } else {
            None
        }
    }
    /// Return the first structural field error for this attachment, if any.
    ///
    /// This predicate is intentionally pure and layout-neutral so Norito decoding, JSON decoding,
    /// transaction admission, and SDK callers can enforce the same canonical attachment shape
    /// without changing the wire format.
    #[must_use]
    pub fn structural_error(&self) -> Option<(&'static str, &'static str)> {
        self.backend_consistency_error().map_or_else(
            || self.field_content_error(),
            |field| Some((field, "must match attachment backend")),
        )
    }
    fn field_content_error(&self) -> Option<(&'static str, &'static str)> {
        if self.backend.as_str().trim().is_empty() {
            Some(("backend", "must be non-empty"))
        } else if self.proof.backend.as_str().trim().is_empty() {
            Some(("proof.backend", "must be non-empty"))
        } else if self.vk_ref.backend.as_str().trim().is_empty() {
            Some(("vk_ref.backend", "must be non-empty"))
        } else if self.vk_ref.name.trim().is_empty() {
            Some(("vk_ref.name", "must be non-empty"))
        } else if !self.vk_ref.is_portable_registry_id() {
            Some(("vk_ref", "must use portable registry syntax"))
        } else if self.proof.bytes.is_empty() {
            Some(("proof.bytes", "must be non-empty"))
        } else if self
            .proof
            .canonical_encoded_len_v1()
            .is_none_or(|length| length > PROOF_BOX_MAX_ENCODED_BYTES_V1)
        {
            Some(("proof", "canonical encoding exceeds the 64 MiB limit"))
        } else if self
            .vk_commitment
            .is_some_and(|commitment| commitment.iter().all(|byte| *byte == 0))
        {
            Some(("vk_commitment", "must be non-zero"))
        } else if self
            .envelope_hash
            .is_some_and(|hash| hash.iter().all(|byte| *byte == 0))
        {
            Some(("envelope_hash", "must be non-zero"))
        } else if self.envelope_hash.is_some_and(|hash| {
            let expected: [u8; 32] = iroha_crypto::Hash::new(&self.proof.bytes).into();
            hash != expected
        }) {
            Some(("envelope_hash", "must match proof bytes"))
        } else if self
            .lane_privacy
            .as_ref()
            .is_some_and(|proof| proof.validate_structure_v1().is_err())
        {
            Some((
                "lane_privacy",
                "must be a complete canonical bounded Merkle witness",
            ))
        } else {
            None
        }
    }
}
#[cfg(feature = "json")]
const PROOF_ATTACHMENT_JSON_HASH_LITERAL_BYTES_V1: usize = 74;
#[cfg(feature = "json")]
fn proof_attachment_json_value_invalid(
    field: &'static str,
    message: &'static str,
) -> norito::json::Error {
    norito::json::Error::InvalidField {
        field: field.into(),
        message: message.into(),
    }
}
#[cfg(feature = "json")]
fn proof_attachment_json_value_object<'a>(
    value: &'a norito::json::Value,
    field: &'static str,
) -> Result<&'a norito::json::Map, norito::json::Error> {
    value
        .as_object()
        .ok_or_else(|| proof_attachment_json_value_invalid(field, "expected object"))
}
#[cfg(feature = "json")]
fn proof_attachment_json_value_exact_fields(
    object: &norito::json::Map,
    allowed: &[&str],
    field: &'static str,
) -> Result<(), norito::json::Error> {
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        // Do not copy an attacker-controlled key into the error. The strict
        // streaming pass provides detailed diagnostics after this allocation
        // preflight for values whose shape is safe to serialize.
        return Err(proof_attachment_json_value_invalid(
            field,
            "contains an unknown first-release field",
        ));
    }
    Ok(())
}
#[cfg(feature = "json")]
fn proof_attachment_json_value_required<'a>(
    object: &'a norito::json::Map,
    field: &'static str,
) -> Result<&'a norito::json::Value, norito::json::Error> {
    object
        .get(field)
        .ok_or_else(|| norito::json::Error::missing_field(field))
}
#[cfg(feature = "json")]
fn proof_attachment_json_value_string<'a>(
    value: &'a norito::json::Value,
    field: &'static str,
    maximum: usize,
) -> Result<&'a str, norito::json::Error> {
    let value = value
        .as_str()
        .ok_or_else(|| proof_attachment_json_value_invalid(field, "expected string"))?;
    if value.len() > maximum {
        return Err(proof_attachment_json_value_invalid(
            field,
            "string exceeds its first-release byte limit",
        ));
    }
    Ok(value)
}
#[cfg(feature = "json")]
fn proof_attachment_json_value_u64(
    value: &norito::json::Value,
    field: &'static str,
    maximum: u64,
) -> Result<u64, norito::json::Error> {
    let value = value
        .as_u64()
        .filter(|value| *value <= maximum)
        .ok_or_else(|| {
            proof_attachment_json_value_invalid(field, "expected bounded unsigned integer")
        })?;
    Ok(value)
}
#[cfg(feature = "json")]
fn proof_attachment_json_value_byte_array(
    value: &norito::json::Value,
    field: &'static str,
    exact_length: Option<usize>,
    maximum_length: usize,
) -> Result<(), norito::json::Error> {
    let values = value
        .as_array()
        .ok_or_else(|| proof_attachment_json_value_invalid(field, "expected byte array"))?;
    if values.len() > maximum_length || exact_length.is_some_and(|length| values.len() != length) {
        return Err(proof_attachment_json_value_invalid(
            field,
            "byte array has a non-canonical length",
        ));
    }
    if values.iter().any(|value| {
        value
            .as_u64()
            .is_none_or(|value| value > u64::from(u8::MAX))
    }) {
        return Err(proof_attachment_json_value_invalid(
            field,
            "byte array contains a value outside u8",
        ));
    }
    Ok(())
}
#[cfg(feature = "json")]
fn proof_attachment_json_value_preflight_lane(
    value: &norito::json::Value,
) -> Result<(), norito::json::Error> {
    let lane = proof_attachment_json_value_object(value, "lane_privacy")?;
    proof_attachment_json_value_exact_fields(lane, &["commitment_id", "witness"], "lane_privacy")?;
    let commitment_id = proof_attachment_json_value_required(lane, "commitment_id")?
        .as_array()
        .ok_or_else(|| {
            proof_attachment_json_value_invalid(
                "lane_privacy.commitment_id",
                "expected one-element tuple",
            )
        })?;
    if commitment_id.len() != 1 {
        return Err(proof_attachment_json_value_invalid(
            "lane_privacy.commitment_id",
            "expected one-element tuple",
        ));
    }
    proof_attachment_json_value_u64(
        &commitment_id[0],
        "lane_privacy.commitment_id",
        u64::from(u16::MAX),
    )?;
    let witness = proof_attachment_json_value_object(
        proof_attachment_json_value_required(lane, "witness")?,
        "lane_privacy.witness",
    )?;
    proof_attachment_json_value_exact_fields(
        witness,
        &["kind", "payload"],
        "lane_privacy.witness",
    )?;
    proof_attachment_json_value_string(
        proof_attachment_json_value_required(witness, "kind")?,
        "lane_privacy.witness.kind",
        16,
    )?;
    let payload = proof_attachment_json_value_object(
        proof_attachment_json_value_required(witness, "payload")?,
        "lane_privacy.witness.payload",
    )?;
    proof_attachment_json_value_exact_fields(
        payload,
        &["leaf", "proof"],
        "lane_privacy.witness.payload",
    )?;
    proof_attachment_json_value_byte_array(
        proof_attachment_json_value_required(payload, "leaf")?,
        "lane_privacy.witness.payload.leaf",
        Some(32),
        32,
    )?;
    let proof = proof_attachment_json_value_object(
        proof_attachment_json_value_required(payload, "proof")?,
        "lane_privacy.witness.payload.proof",
    )?;
    proof_attachment_json_value_exact_fields(
        proof,
        &["leaf_index", "audit_path"],
        "lane_privacy.witness.payload.proof",
    )?;
    proof_attachment_json_value_u64(
        proof_attachment_json_value_required(proof, "leaf_index")?,
        "lane_privacy.witness.payload.proof.leaf_index",
        u64::from(u32::MAX),
    )?;
    let audit_path = proof_attachment_json_value_required(proof, "audit_path")?
        .as_array()
        .ok_or_else(|| {
            proof_attachment_json_value_invalid(
                "lane_privacy.witness.payload.proof.audit_path",
                "expected array",
            )
        })?;
    if audit_path.is_empty() || audit_path.len() > crate::nexus::LANE_PRIVACY_MAX_MERKLE_DEPTH_V1 {
        return Err(proof_attachment_json_value_invalid(
            "lane_privacy.witness.payload.proof.audit_path",
            "Merkle path has a non-canonical depth",
        ));
    }
    for sibling in audit_path {
        proof_attachment_json_value_string(
            sibling,
            "lane_privacy.witness.payload.proof.audit_path",
            PROOF_ATTACHMENT_JSON_HASH_LITERAL_BYTES_V1,
        )?;
    }
    Ok(())
}
#[cfg(feature = "json")]
fn proof_attachment_json_value_preflight<const MAX_PROOF_BYTES: usize>(
    value: &norito::json::Value,
) -> Result<(), norito::json::Error> {
    let attachment = proof_attachment_json_value_object(value, "ProofAttachment")?;
    proof_attachment_json_value_exact_fields(
        attachment,
        &[
            "backend",
            "proof",
            "vk_ref",
            "vk_commitment",
            "envelope_hash",
            "lane_privacy",
        ],
        "ProofAttachment",
    )?;
    proof_attachment_json_value_string(
        proof_attachment_json_value_required(attachment, "backend")?,
        "backend",
        VERIFYING_KEY_ID_MAX_FIELD_BYTES,
    )?;
    let proof = proof_attachment_json_value_object(
        proof_attachment_json_value_required(attachment, "proof")?,
        "proof",
    )?;
    proof_attachment_json_value_exact_fields(proof, &["backend", "bytes"], "proof")?;
    let proof_backend = proof_attachment_json_value_string(
        proof_attachment_json_value_required(proof, "backend")?,
        "proof.backend",
        VERIFYING_KEY_ID_MAX_FIELD_BYTES,
    )?;
    let maximum_proof_bytes = proof_box_max_proof_bytes_v1(proof_backend)
        .ok_or_else(|| {
            proof_attachment_json_value_invalid(
                "proof.backend",
                "backend framing exceeds the ProofBox limit",
            )
        })?
        .min(MAX_PROOF_BYTES);
    proof_attachment_json_value_byte_array(
        proof_attachment_json_value_required(proof, "bytes")?,
        "proof.bytes",
        None,
        maximum_proof_bytes,
    )?;
    let vk_ref = proof_attachment_json_value_object(
        proof_attachment_json_value_required(attachment, "vk_ref")?,
        "vk_ref",
    )?;
    proof_attachment_json_value_exact_fields(vk_ref, &["backend", "name"], "vk_ref")?;
    for field in ["backend", "name"] {
        proof_attachment_json_value_string(
            proof_attachment_json_value_required(vk_ref, field)?,
            if field == "backend" {
                "vk_ref.backend"
            } else {
                "vk_ref.name"
            },
            VERIFYING_KEY_ID_MAX_FIELD_BYTES,
        )?;
    }
    for field in ["vk_commitment", "envelope_hash"] {
        if let Some(value) = attachment.get(field) {
            proof_attachment_json_value_byte_array(value, field, Some(32), 32)?;
        }
    }
    if let Some(lane_privacy) = attachment.get("lane_privacy") {
        proof_attachment_json_value_preflight_lane(lane_privacy)?;
    }
    Ok(())
}
#[cfg(feature = "json")]
fn proof_attachment_json_mark_field(
    seen: &mut u8,
    field_bit: u8,
    field: &str,
) -> Result<(), norito::json::Error> {
    if *seen & field_bit != 0 {
        return Err(norito::json::Error::duplicate_field(field));
    }
    *seen |= field_bit;
    Ok(())
}
#[cfg(feature = "json")]
fn proof_attachment_json_unknown_field(field: &str, parent: &str) -> norito::json::Error {
    let qualified = if parent.is_empty() {
        field.to_owned()
    } else {
        format!("{parent}.{field}")
    };
    if matches!(
        field,
        "vk_inline" | "vkInline" | "verifyingKeyInline" | "verifying_key_inline"
    ) {
        norito::json::Error::InvalidField {
            field: qualified,
            message: "retired inline verifying-key field is not supported; use vk_ref".into(),
        }
    } else {
        norito::json::Error::InvalidField {
            field: qualified,
            message: "unknown fields are not part of the first-release schema".into(),
        }
    }
}
/// A string whose exact decoded UTF-8 length is bounded before an owned allocation is created.
#[cfg(feature = "json")]
struct ProofAttachmentJsonBoundedStringV1<const MAX: usize>(String);
#[cfg(feature = "json")]
impl<const MAX: usize> norito::json::JsonDeserialize for ProofAttachmentJsonBoundedStringV1<MAX> {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let mut probe = *parser;
        probe.skip_string_bounded(MAX)?;
        let value = String::json_deserialize(parser)?;
        if value.len() > MAX {
            return Err(norito::json::Error::Message(format!(
                "proof attachment string exceeds the {MAX}-byte decoded limit"
            )));
        }
        Ok(Self(value))
    }
}
#[cfg(feature = "json")]
struct ProofAttachmentJsonBytes32V1([u8; 32]);
#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for ProofAttachmentJsonBytes32V1 {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let mut sequence = norito::json::SeqVisitor::new(parser)?;
        let mut bytes = [0_u8; 32];
        for (index, byte) in bytes.iter_mut().enumerate() {
            let raw = sequence.next_element::<u64>()?.ok_or_else(|| {
                norito::json::Error::Message(format!("expected 32 bytes, got {index}"))
            })?;
            *byte = u8::try_from(raw).map_err(|_| norito::json::Error::InvalidField {
                field: "byte array".into(),
                message: format!("byte at index {index} is not a valid u8"),
            })?;
        }
        if !sequence.is_finished() {
            return Err(norito::json::Error::Message(
                "expected exactly 32 bytes".into(),
            ));
        }
        sequence.finish()?;
        Ok(Self(bytes))
    }
}
/// Streaming byte-array decoder used for proof payloads. The length check is
/// performed before reserving or pushing the next byte, so an over-limit
/// element can never grow the output allocation.
#[cfg(feature = "json")]
struct ProofAttachmentJsonBoundedBytesVisitorV1 {
    maximum: usize,
}
#[cfg(feature = "json")]
impl ProofAttachmentJsonBoundedBytesVisitorV1 {
    fn expected_array() -> norito::json::Error {
        norito::json::Error::InvalidField {
            field: "proof.bytes".into(),
            message: "expected a JSON byte array".into(),
        }
    }
}
#[cfg(feature = "json")]
impl<'a> norito::json::Visitor<'a> for ProofAttachmentJsonBoundedBytesVisitorV1 {
    type Value = Vec<u8>;
    fn visit_null(self) -> Result<Self::Value, norito::json::Error> {
        Err(Self::expected_array())
    }
    fn visit_bool(self, _value: bool) -> Result<Self::Value, norito::json::Error> {
        Err(Self::expected_array())
    }
    fn visit_i64(self, _value: i64) -> Result<Self::Value, norito::json::Error> {
        Err(Self::expected_array())
    }
    fn visit_u64(self, _value: u64) -> Result<Self::Value, norito::json::Error> {
        Err(Self::expected_array())
    }
    fn visit_f64(self, _value: f64) -> Result<Self::Value, norito::json::Error> {
        Err(Self::expected_array())
    }
    fn visit_string(self, _value: String) -> Result<Self::Value, norito::json::Error> {
        Err(Self::expected_array())
    }
    fn visit_map(
        self,
        _visitor: norito::json::MapVisitor<'a, '_>,
    ) -> Result<Self::Value, norito::json::Error> {
        Err(Self::expected_array())
    }
    fn visit_seq(
        self,
        mut sequence: norito::json::SeqVisitor<'a, '_>,
    ) -> Result<Self::Value, norito::json::Error> {
        let mut bytes = Vec::new();
        while !sequence.is_finished() {
            if bytes.len() == self.maximum {
                return Err(norito::json::Error::Message(format!(
                    "proof bytes exceed the {}-byte streaming limit",
                    self.maximum
                )));
            }
            if bytes.len() == bytes.capacity() {
                let remaining = self.maximum - bytes.len();
                let additional = bytes.capacity().max(4 * 1024).min(remaining);
                bytes.try_reserve_exact(additional).map_err(|_| {
                    norito::json::Error::Message(
                        "unable to reserve bounded proof byte storage".into(),
                    )
                })?;
            }
            let index = bytes.len();
            let raw = sequence
                .next_element::<u64>()?
                .ok_or_else(|| norito::json::Error::Message("expected proof byte".into()))?;
            let byte = u8::try_from(raw).map_err(|_| norito::json::Error::InvalidField {
                field: "proof.bytes".into(),
                message: format!("byte at index {index} is not a valid u8"),
            })?;
            bytes.push(byte);
        }
        sequence.finish()?;
        Ok(bytes)
    }
}
#[cfg(feature = "json")]
struct ProofAttachmentJsonProofBoxV1<const MAX: usize> {
    backend: String,
    bytes: Vec<u8>,
}
#[cfg(feature = "json")]
fn proof_attachment_json_probe_proof_backend(
    parser: &norito::json::Parser<'_>,
) -> Result<String, norito::json::Error> {
    let mut probe = *parser;
    let mut object = norito::json::MapVisitor::new(&mut probe)?;
    let mut backend = None;
    while let Some(field) = object.next_key()? {
        if field.as_str() == "backend" {
            if backend.is_some() {
                return Err(norito::json::Error::duplicate_field("backend"));
            }
            backend = Some(
                object
                    .parse_value::<ProofAttachmentJsonBoundedStringV1<
                        VERIFYING_KEY_ID_MAX_FIELD_BYTES,
                    >>()?
                    .0,
            );
        } else {
            // This first pass discovers the backend without materializing the
            // potentially enormous numeric proof array. The strict pass below
            // still rejects every unknown or retired member.
            object.skip_value()?;
        }
    }
    object.finish()?;
    backend.ok_or_else(|| norito::json::Error::missing_field("proof.backend"))
}
#[cfg(feature = "json")]
impl<const MAX: usize> norito::json::JsonDeserialize for ProofAttachmentJsonProofBoxV1<MAX> {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        const BACKEND: u8 = 1 << 0;
        const BYTES: u8 = 1 << 1;
        let probed_backend = proof_attachment_json_probe_proof_backend(parser)?;
        let maximum_proof_bytes = proof_box_max_proof_bytes_v1(&probed_backend)
            .ok_or_else(|| norito::json::Error::InvalidField {
                field: "proof.backend".into(),
                message: "backend and canonical framing exceed the 64 MiB ProofBox limit".into(),
            })?
            .min(MAX);
        let mut object = norito::json::MapVisitor::new(parser)?;
        let mut seen = 0_u8;
        let mut backend = None;
        let mut bytes = None;
        while let Some(field) = object.next_key()? {
            match field.as_str() {
                "backend" => {
                    proof_attachment_json_mark_field(&mut seen, BACKEND, "backend")?;
                    backend =
                        Some(
                            object
                                .parse_value::<ProofAttachmentJsonBoundedStringV1<
                                    VERIFYING_KEY_ID_MAX_FIELD_BYTES,
                                >>()?
                                .0,
                        );
                }
                "bytes" => {
                    proof_attachment_json_mark_field(&mut seen, BYTES, "bytes")?;
                    object.parser().skip_ws();
                    if object.parser().peek() != Some(b'[') {
                        return Err(ProofAttachmentJsonBoundedBytesVisitorV1::expected_array());
                    }
                    bytes = Some(object.parse_value_with(
                        ProofAttachmentJsonBoundedBytesVisitorV1 {
                            maximum: maximum_proof_bytes,
                        },
                    )?);
                }
                field => return Err(proof_attachment_json_unknown_field(field, "proof")),
            }
        }
        object.finish()?;
        let backend = backend.ok_or_else(|| norito::json::Error::missing_field("proof.backend"))?;
        let bytes = bytes.ok_or_else(|| norito::json::Error::missing_field("proof.bytes"))?;
        if backend != probed_backend {
            return Err(norito::json::Error::InvalidField {
                field: "proof.backend".into(),
                message: "backend changed between bounded parser passes".into(),
            });
        }
        Ok(Self { backend, bytes })
    }
}
#[cfg(feature = "json")]
struct ProofAttachmentJsonVerifyingKeyRefV1 {
    backend: String,
    name: String,
}
#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for ProofAttachmentJsonVerifyingKeyRefV1 {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        const BACKEND: u8 = 1 << 0;
        const NAME: u8 = 1 << 1;
        let mut object = norito::json::MapVisitor::new(parser)?;
        let mut seen = 0_u8;
        let mut backend = None;
        let mut name = None;
        while let Some(field) = object.next_key()? {
            match field.as_str() {
                "backend" => {
                    proof_attachment_json_mark_field(&mut seen, BACKEND, "backend")?;
                    backend =
                        Some(
                            object
                                .parse_value::<ProofAttachmentJsonBoundedStringV1<
                                    VERIFYING_KEY_ID_MAX_FIELD_BYTES,
                                >>()?
                                .0,
                        );
                }
                "name" => {
                    proof_attachment_json_mark_field(&mut seen, NAME, "name")?;
                    name =
                        Some(
                            object
                                .parse_value::<ProofAttachmentJsonBoundedStringV1<
                                    VERIFYING_KEY_ID_MAX_FIELD_BYTES,
                                >>()?
                                .0,
                        );
                }
                field => return Err(proof_attachment_json_unknown_field(field, "vk_ref")),
            }
        }
        object.finish()?;
        Ok(Self {
            backend: backend.ok_or_else(|| norito::json::Error::missing_field("vk_ref.backend"))?,
            name: name.ok_or_else(|| norito::json::Error::missing_field("vk_ref.name"))?,
        })
    }
}
#[cfg(feature = "json")]
struct ProofAttachmentJsonLaneCommitmentIdV1(u16);
#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for ProofAttachmentJsonLaneCommitmentIdV1 {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let mut sequence = norito::json::SeqVisitor::new(parser)?;
        let commitment_id =
            sequence
                .next_element::<u16>()?
                .ok_or_else(|| norito::json::Error::InvalidField {
                    field: "lane_privacy.commitment_id".into(),
                    message: "expected one-element lane commitment tuple".into(),
                })?;
        if !sequence.is_finished() {
            return Err(norito::json::Error::InvalidField {
                field: "lane_privacy.commitment_id".into(),
                message: "expected one-element lane commitment tuple".into(),
            });
        }
        sequence.finish()?;
        Ok(Self(commitment_id))
    }
}
#[cfg(feature = "json")]
struct ProofAttachmentJsonMerkleSiblingV1([u8; 32]);
#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for ProofAttachmentJsonMerkleSiblingV1 {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        parser.skip_ws();
        if parser.peek() != Some(b'"') {
            return Err(norito::json::Error::InvalidField {
                field: "lane_privacy.witness.payload.proof.audit_path".into(),
                message: "expected a canonical hash literal for every Merkle sibling".into(),
            });
        }
        // `hash:` + 64 hex digits + `#` + four checksum digits.
        let literal = ProofAttachmentJsonBoundedStringV1::<
            PROOF_ATTACHMENT_JSON_HASH_LITERAL_BYTES_V1,
        >::json_deserialize(parser)?
        .0;
        let body = norito::literal::parse("hash", &literal).map_err(|error| {
            norito::json::Error::InvalidField {
                field: "lane_privacy.witness.payload.proof.audit_path".into(),
                message: error.to_string(),
            }
        })?;
        if body.bytes().any(|byte| byte.is_ascii_lowercase()) {
            return Err(norito::json::Error::InvalidField {
                field: "lane_privacy.witness.payload.proof.audit_path".into(),
                message: "canonical hash literals must use uppercase hex digits".into(),
            });
        }
        let hash = body.parse::<iroha_crypto::Hash>().map_err(|error| {
            norito::json::Error::InvalidField {
                field: "lane_privacy.witness.payload.proof.audit_path".into(),
                message: error.to_string(),
            }
        })?;
        let bytes: [u8; 32] = *hash.as_ref();
        if bytes[31] & 1 == 0 {
            return Err(norito::json::Error::InvalidField {
                field: "lane_privacy.witness.payload.proof.audit_path".into(),
                message: "Merkle sibling is not canonically pre-hashed".into(),
            });
        }
        Ok(Self(bytes))
    }
}
#[cfg(feature = "json")]
struct ProofAttachmentJsonAuditPathV1<const MAX: usize>(Vec<[u8; 32]>);
#[cfg(feature = "json")]
impl<const MAX: usize> norito::json::JsonDeserialize for ProofAttachmentJsonAuditPathV1<MAX> {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let mut sequence = norito::json::SeqVisitor::new(parser)?;
        let mut path = Vec::with_capacity(MAX.min(32));
        while !sequence.is_finished() {
            if path.len() == MAX {
                return Err(norito::json::Error::InvalidField {
                    field: "lane_privacy.witness.payload.proof.audit_path".into(),
                    message: format!("Merkle path exceeds the {MAX}-sibling limit"),
                });
            }
            let sibling = sequence
                .next_element::<ProofAttachmentJsonMerkleSiblingV1>()?
                .ok_or_else(|| norito::json::Error::Message("expected Merkle sibling".into()))?;
            path.push(sibling.0);
        }
        sequence.finish()?;
        if path.is_empty() {
            return Err(norito::json::Error::InvalidField {
                field: "lane_privacy.witness.payload.proof.audit_path".into(),
                message: "Merkle path must not be empty".into(),
            });
        }
        Ok(Self(path))
    }
}
#[cfg(feature = "json")]
struct ProofAttachmentJsonLaneMerkleProofV1 {
    leaf_index: u32,
    audit_path: ProofAttachmentJsonAuditPathV1<{ crate::nexus::LANE_PRIVACY_MAX_MERKLE_DEPTH_V1 }>,
}
#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for ProofAttachmentJsonLaneMerkleProofV1 {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        const LEAF_INDEX: u8 = 1 << 0;
        const AUDIT_PATH: u8 = 1 << 1;
        let mut object = norito::json::MapVisitor::new(parser)?;
        let mut seen = 0_u8;
        let mut leaf_index = None;
        let mut audit_path = None;
        while let Some(field) = object.next_key()? {
            match field.as_str() {
                "leaf_index" => {
                    proof_attachment_json_mark_field(&mut seen, LEAF_INDEX, "leaf_index")?;
                    leaf_index = Some(object.parse_value::<u32>()?);
                }
                "audit_path" => {
                    proof_attachment_json_mark_field(&mut seen, AUDIT_PATH, "audit_path")?;
                    audit_path = Some(object.parse_value::<ProofAttachmentJsonAuditPathV1<
                        { crate::nexus::LANE_PRIVACY_MAX_MERKLE_DEPTH_V1 },
                    >>()?);
                }
                field => {
                    return Err(proof_attachment_json_unknown_field(
                        field,
                        "lane_privacy.witness.payload.proof",
                    ));
                }
            }
        }
        object.finish()?;
        Ok(Self {
            leaf_index: leaf_index.ok_or_else(|| {
                norito::json::Error::missing_field("lane_privacy.witness.payload.proof.leaf_index")
            })?,
            audit_path: audit_path.ok_or_else(|| {
                norito::json::Error::missing_field("lane_privacy.witness.payload.proof.audit_path")
            })?,
        })
    }
}
#[cfg(feature = "json")]
struct ProofAttachmentJsonLaneMerklePayloadV1 {
    leaf: [u8; 32],
    proof: ProofAttachmentJsonLaneMerkleProofV1,
}
#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for ProofAttachmentJsonLaneMerklePayloadV1 {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        const LEAF: u8 = 1 << 0;
        const PROOF: u8 = 1 << 1;
        let mut object = norito::json::MapVisitor::new(parser)?;
        let mut seen = 0_u8;
        let mut leaf = None;
        let mut proof = None;
        while let Some(field) = object.next_key()? {
            match field.as_str() {
                "leaf" => {
                    proof_attachment_json_mark_field(&mut seen, LEAF, "leaf")?;
                    leaf = Some(object.parse_value::<ProofAttachmentJsonBytes32V1>()?.0);
                }
                "proof" => {
                    proof_attachment_json_mark_field(&mut seen, PROOF, "proof")?;
                    proof = Some(object.parse_value::<ProofAttachmentJsonLaneMerkleProofV1>()?);
                }
                field => {
                    return Err(proof_attachment_json_unknown_field(
                        field,
                        "lane_privacy.witness.payload",
                    ));
                }
            }
        }
        object.finish()?;
        Ok(Self {
            leaf: leaf.ok_or_else(|| {
                norito::json::Error::missing_field("lane_privacy.witness.payload.leaf")
            })?,
            proof: proof.ok_or_else(|| {
                norito::json::Error::missing_field("lane_privacy.witness.payload.proof")
            })?,
        })
    }
}
#[cfg(feature = "json")]
struct ProofAttachmentJsonLaneWitnessV1 {
    kind: String,
    payload: ProofAttachmentJsonLaneMerklePayloadV1,
}
#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for ProofAttachmentJsonLaneWitnessV1 {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        const KIND: u8 = 1 << 0;
        const PAYLOAD: u8 = 1 << 1;
        let mut object = norito::json::MapVisitor::new(parser)?;
        let mut seen = 0_u8;
        let mut kind = None;
        let mut payload = None;
        while let Some(field) = object.next_key()? {
            match field.as_str() {
                "kind" => {
                    proof_attachment_json_mark_field(&mut seen, KIND, "kind")?;
                    kind = Some(
                        object
                            .parse_value::<ProofAttachmentJsonBoundedStringV1<16>>()?
                            .0,
                    );
                }
                "payload" => {
                    proof_attachment_json_mark_field(&mut seen, PAYLOAD, "payload")?;
                    payload = Some(object.parse_value::<ProofAttachmentJsonLaneMerklePayloadV1>()?);
                }
                field => {
                    return Err(proof_attachment_json_unknown_field(
                        field,
                        "lane_privacy.witness",
                    ));
                }
            }
        }
        object.finish()?;
        Ok(Self {
            kind: kind
                .ok_or_else(|| norito::json::Error::missing_field("lane_privacy.witness.kind"))?,
            payload: payload.ok_or_else(|| {
                norito::json::Error::missing_field("lane_privacy.witness.payload")
            })?,
        })
    }
}
#[cfg(feature = "json")]
struct ProofAttachmentJsonLanePrivacyV1 {
    commitment_id: u16,
    witness: ProofAttachmentJsonLaneWitnessV1,
}
#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for ProofAttachmentJsonLanePrivacyV1 {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        const COMMITMENT_ID: u8 = 1 << 0;
        const WITNESS: u8 = 1 << 1;
        let mut object = norito::json::MapVisitor::new(parser)?;
        let mut seen = 0_u8;
        let mut commitment_id = None;
        let mut witness = None;
        while let Some(field) = object.next_key()? {
            match field.as_str() {
                "commitment_id" => {
                    proof_attachment_json_mark_field(&mut seen, COMMITMENT_ID, "commitment_id")?;
                    commitment_id = Some(
                        object
                            .parse_value::<ProofAttachmentJsonLaneCommitmentIdV1>()?
                            .0,
                    );
                }
                "witness" => {
                    proof_attachment_json_mark_field(&mut seen, WITNESS, "witness")?;
                    witness = Some(object.parse_value::<ProofAttachmentJsonLaneWitnessV1>()?);
                }
                field => {
                    return Err(proof_attachment_json_unknown_field(field, "lane_privacy"));
                }
            }
        }
        object.finish()?;
        Ok(Self {
            commitment_id: commitment_id
                .ok_or_else(|| norito::json::Error::missing_field("lane_privacy.commitment_id"))?,
            witness: witness
                .ok_or_else(|| norito::json::Error::missing_field("lane_privacy.witness"))?,
        })
    }
}
#[cfg(feature = "json")]
impl ProofAttachmentJsonLanePrivacyV1 {
    fn into_lane_privacy_proof(
        self,
    ) -> Result<crate::nexus::LanePrivacyProof, norito::json::Error> {
        if self.witness.kind != "merkle" {
            return Err(norito::json::Error::InvalidField {
                field: "lane_privacy.witness.kind".into(),
                message: "only the canonical merkle witness is supported".into(),
            });
        }
        let ProofAttachmentJsonLaneMerklePayloadV1 { leaf, proof } = self.witness.payload;
        let ProofAttachmentJsonLaneMerkleProofV1 {
            leaf_index,
            audit_path,
        } = proof;
        crate::nexus::LanePrivacyProof::merkle_from_raw_path(
            iroha_crypto::LaneCommitmentId::new(self.commitment_id),
            leaf,
            leaf_index,
            audit_path.0.into_iter().map(Some).collect(),
        )
        .map_err(|error| norito::json::Error::InvalidField {
            field: "lane_privacy".into(),
            message: error.to_string(),
        })
    }
}
#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for ProofAttachment {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        const BACKEND: u8 = 1 << 0;
        const PROOF: u8 = 1 << 1;
        const VK_REF: u8 = 1 << 2;
        const VK_COMMITMENT: u8 = 1 << 3;
        const ENVELOPE_HASH: u8 = 1 << 4;
        const LANE_PRIVACY: u8 = 1 << 5;
        let mut object = norito::json::MapVisitor::new(parser)?;
        let mut seen = 0_u8;
        let mut backend = None;
        let mut proof = None;
        let mut vk_ref = None;
        let mut vk_commitment = None;
        let mut envelope_hash = None;
        let mut lane_privacy = None;
        while let Some(field) = object.next_key()? {
            match field.as_str() {
                "backend" => {
                    proof_attachment_json_mark_field(&mut seen, BACKEND, "backend")?;
                    backend =
                        Some(
                            object
                                .parse_value::<ProofAttachmentJsonBoundedStringV1<
                                    VERIFYING_KEY_ID_MAX_FIELD_BYTES,
                                >>()?
                                .0,
                        );
                }
                "proof" => {
                    proof_attachment_json_mark_field(&mut seen, PROOF, "proof")?;
                    proof = Some(object.parse_value::<ProofAttachmentJsonProofBoxV1<
                        PROOF_BOX_MAX_ENCODED_BYTES_V1,
                    >>()?);
                }
                "vk_ref" => {
                    proof_attachment_json_mark_field(&mut seen, VK_REF, "vk_ref")?;
                    vk_ref = Some(object.parse_value::<ProofAttachmentJsonVerifyingKeyRefV1>()?);
                }
                "vk_commitment" => {
                    proof_attachment_json_mark_field(&mut seen, VK_COMMITMENT, "vk_commitment")?;
                    // Optional means absent-or-present; an explicit null is not
                    // a canonical first-release spelling for a present field.
                    vk_commitment = Some(object.parse_value::<ProofAttachmentJsonBytes32V1>()?.0);
                }
                "envelope_hash" => {
                    proof_attachment_json_mark_field(&mut seen, ENVELOPE_HASH, "envelope_hash")?;
                    envelope_hash = Some(object.parse_value::<ProofAttachmentJsonBytes32V1>()?.0);
                }
                "lane_privacy" => {
                    proof_attachment_json_mark_field(&mut seen, LANE_PRIVACY, "lane_privacy")?;
                    lane_privacy = Some(object.parse_value::<ProofAttachmentJsonLanePrivacyV1>()?);
                }
                field => return Err(proof_attachment_json_unknown_field(field, "")),
            }
        }
        object.finish()?;
        let backend = backend.ok_or_else(|| norito::json::Error::missing_field("backend"))?;
        let proof = proof.ok_or_else(|| norito::json::Error::missing_field("proof"))?;
        let vk_ref = vk_ref.ok_or_else(|| norito::json::Error::missing_field("vk_ref"))?;
        let attachment = Self {
            backend,
            proof: ProofBox::new(proof.backend, proof.bytes),
            vk_ref: VerifyingKeyId::new(vk_ref.backend, vk_ref.name),
            vk_commitment,
            envelope_hash,
            lane_privacy: lane_privacy
                .map(ProofAttachmentJsonLanePrivacyV1::into_lane_privacy_proof)
                .transpose()?,
        };
        if let Some((field, message)) = attachment.structural_error() {
            return Err(norito::json::Error::InvalidField {
                field: field.into(),
                message: message.into(),
            });
        }
        Ok(attachment)
    }
    fn json_from_value(value: &norito::json::Value) -> Result<Self, norito::json::Error> {
        // A Value supplied by an API caller already owns its storage. Walk it
        // by reference first so hostile oversized fields cannot trigger the
        // additional canonical JSON allocation used to re-enter the one true
        // streaming decoder below.
        proof_attachment_json_value_preflight::<PROOF_BOX_MAX_ENCODED_BYTES_V1>(value)?;
        let canonical_json = norito::json::to_json(value)?;
        norito::json::from_str(&canonical_json)
    }
}
impl norito::NoritoSerialize for ProofAttachment {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), ncore::Error> {
        fn write_prefixed<W: Write, T: norito::NoritoSerialize>(
            writer: &mut W,
            value: &T,
            scratch: &mut ncore::DeriveSmallBuf,
        ) -> Result<(), ncore::Error> {
            ncore::write_len_prefixed_exact(writer, value, scratch)
        }
        let mut scratch = ncore::DeriveSmallBuf::new();
        write_prefixed(writer, &self.backend, &mut scratch)?;
        write_prefixed(writer, &self.proof, &mut scratch)?;
        write_prefixed(writer, &self.vk_ref, &mut scratch)?;
        // Omit trailing default fields to keep payloads compact and deterministic.
        let tail = if self.lane_privacy.is_some() {
            3
        } else if self.envelope_hash.is_some() {
            2
        } else {
            i32::from(self.vk_commitment.is_some())
        };
        if tail >= 1 {
            write_prefixed(writer, &self.vk_commitment, &mut scratch)?;
        }
        if tail >= 2 {
            write_prefixed(writer, &self.envelope_hash, &mut scratch)?;
        }
        if tail >= 3 {
            write_prefixed(writer, &self.lane_privacy, &mut scratch)?;
        }
        Ok(())
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        self.encoded_len_exact()
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        fn add_field<T: norito::NoritoSerialize>(total: &mut usize, value: &T) -> Option<()> {
            let field_len = value.encoded_len_exact()?;
            *total = total
                .checked_add(ncore::len_prefix_len(field_len))?
                .checked_add(field_len)?;
            Some(())
        }
        let mut total = 0_usize;
        add_field(&mut total, &self.backend)?;
        add_field(&mut total, &self.proof)?;
        add_field(&mut total, &self.vk_ref)?;
        let tail = if self.lane_privacy.is_some() {
            3
        } else if self.envelope_hash.is_some() {
            2
        } else {
            usize::from(self.vk_commitment.is_some())
        };
        if tail >= 1 {
            add_field(&mut total, &self.vk_commitment)?;
        }
        if tail >= 2 {
            add_field(&mut total, &self.envelope_hash)?;
        }
        if tail >= 3 {
            add_field(&mut total, &self.lane_privacy)?;
        }
        Some(total)
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
        let backend_bytes = take_len_prefixed_slice(bytes, &mut offset, MAX_BACKEND_FIELD_BYTES)?;
        let (backend, used) = <Ident as ncore::DecodeFromSlice>::decode_from_slice(backend_bytes)?;
        if used != backend_bytes.len() {
            return Err(ncore::Error::LengthMismatch);
        }
        let proof_slice =
            take_len_prefixed_slice(bytes, &mut offset, MAX_LEN_PREFIXED_FIELD_BYTES)?;
        let (proof, used) = <ProofBox as ncore::DecodeFromSlice>::decode_from_slice(proof_slice)?;
        if used != proof_slice.len() {
            return Err(ncore::Error::LengthMismatch);
        }
        let vk_ref_slice = take_len_prefixed_slice(bytes, &mut offset, MAX_REF_FIELD_BYTES)?;
        let (vk_ref, used) =
            <VerifyingKeyId as ncore::DecodeFromSlice>::decode_from_slice(vk_ref_slice)?;
        if used != vk_ref_slice.len() {
            return Err(ncore::Error::LengthMismatch);
        }
        // Optional fields may be omitted in compact payloads; treat missing tail as `None`.
        let mut present_tail_fields = 0_usize;
        let vk_commitment = if offset == bytes.len() {
            None
        } else {
            present_tail_fields = 1;
            let slice = take_len_prefixed_slice(bytes, &mut offset, MAX_REF_FIELD_BYTES)?;
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
            present_tail_fields = 2;
            let slice = take_len_prefixed_slice(bytes, &mut offset, MAX_REF_FIELD_BYTES)?;
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
            present_tail_fields = 3;
            let slice = take_len_prefixed_slice(bytes, &mut offset, MAX_LEN_PREFIXED_FIELD_BYTES)?;
            let (value, used) =
                <Option<crate::nexus::LanePrivacyProof> as ncore::DecodeFromSlice>::decode_from_slice(
                    slice,
                )?;
            if used != slice.len() {
                return Err(ncore::Error::LengthMismatch);
            }
            value
        };
        if offset != bytes.len() {
            return Err(ncore::Error::LengthMismatch);
        }
        let canonical_tail_fields = if lane_privacy.is_some() {
            3
        } else if envelope_hash.is_some() {
            2
        } else {
            usize::from(vk_commitment.is_some())
        };
        if present_tail_fields != canonical_tail_fields {
            return Err(ncore::Error::Message(
                "non-canonical redundant ProofAttachment optional tail".into(),
            ));
        }
        let attachment = Self {
            backend,
            proof,
            vk_ref,
            vk_commitment,
            envelope_hash,
            lane_privacy,
        };
        if let Some((field, message)) = attachment.structural_error() {
            return Err(ncore::Error::Message(format!("{field} {message}")));
        }
        Ok((attachment, offset))
    }
}
/// Maximum complete canonical Norito frame for a first-release proof attachment list.
///
/// This intrinsic 8 MiB binary ceiling leaves room beneath Taira's governed 10 MiB
/// signed-transaction wire ceiling. It is not a claim that a maximal frame fits Torii's 8 MiB JSON
/// proof body: base64 and JSON quotes expand the transport, whose exact largest decoded binary
/// string at that body limit is 6,291,453 bytes.
pub const PROOF_ATTACHMENT_LIST_MAX_CANONICAL_FRAME_BYTES_V1: usize = 8 * 1024 * 1024;
/// Maximum attachments carried by one first-release proof attachment list.
///
/// This matches the governed `zk.halo2.verifier_max_batch` default.
pub const PROOF_ATTACHMENT_LIST_MAX_ATTACHMENTS_V1: usize = 16;
#[cfg(test)]
std::thread_local! {
    static PROOF_ATTACHMENT_LIST_AUTHORITATIVE_LENGTH_PASSES: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
}
/// Failure to construct a bounded first-release proof attachment list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ProofAttachmentListError {
    /// First-release transactions must carry at least one attachment when the
    /// optional attachment-list field is present.
    #[error("proof attachment list must not be empty")]
    Empty,
    /// The verifier batch boundary would be exceeded.
    #[error("proof attachment count {actual} exceeds the first-release maximum of {maximum}")]
    TooMany {
        /// Supplied attachment count.
        actual: usize,
        /// First-release maximum.
        maximum: usize,
    },
    /// Canonical length arithmetic or the authoritative counting
    /// serialization pass could not produce a frame length.
    #[error("proof attachment list canonical frame could not be encoded")]
    CanonicalEncodingFailed,
    /// The complete canonical frame would exceed the intrinsic V1 ceiling.
    #[error(
        "proof attachment list canonical frame is {actual} bytes, exceeding the {maximum}-byte first-release maximum"
    )]
    CanonicalFrameTooLarge {
        /// Complete canonical frame size.
        actual: usize,
        /// First-release maximum.
        maximum: usize,
    },
}
/// A list of proof attachments for a transaction.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, IntoSchema)]
#[norito(reuse_archived)]
pub struct ProofAttachmentList(
    /// Ordered attachments that make up the proof payload.
    Vec<ProofAttachment>,
);
impl ProofAttachmentList {
    fn canonical_frame_len_from_payload_len(payload_len: usize) -> Option<usize> {
        let alignment = ncore::archived_payload_align::<Self>();
        let remainder = ncore::Header::SIZE % alignment;
        let padding = if remainder == 0 {
            0
        } else {
            alignment - remainder
        };
        ncore::Header::SIZE
            .checked_add(padding)?
            .checked_add(payload_len)
    }
    fn canonical_frame_len_v1(&self) -> Result<usize, ProofAttachmentListError> {
        let _canonical_flags = ncore::DecodeFlagsGuard::enter(ncore::default_encode_flags());
        // `ProofAttachment::serialize` stages nested fields in temporary
        // vectors. Use its allocation-free exact-length arithmetic only as a
        // fail-fast rejection gate so a caller-provided 64 MiB proof cannot
        // make the authoritative pass allocate far beyond this list's 8 MiB
        // ceiling. A value at or below the ceiling is never admitted from the
        // hint: the real counting serializer below remains authoritative.
        let hinted_payload_len = norito::NoritoSerialize::encoded_len_exact(self)
            .ok_or(ProofAttachmentListError::CanonicalEncodingFailed)?;
        let hinted_frame_len = Self::canonical_frame_len_from_payload_len(hinted_payload_len)
            .ok_or(ProofAttachmentListError::CanonicalEncodingFailed)?;
        if hinted_frame_len > PROOF_ATTACHMENT_LIST_MAX_CANONICAL_FRAME_BYTES_V1 {
            return Ok(hinted_frame_len);
        }
        #[cfg(test)]
        PROOF_ATTACHMENT_LIST_AUTHORITATIVE_LENGTH_PASSES.with(|passes| {
            passes.set(passes.get().saturating_add(1));
        });
        ncore::encoded_frame_len(self)
            .map_err(|_| ProofAttachmentListError::CanonicalEncodingFailed)
    }
    #[cfg(test)]
    fn reset_authoritative_length_passes_for_current_test_thread() {
        PROOF_ATTACHMENT_LIST_AUTHORITATIVE_LENGTH_PASSES.with(|passes| passes.set(0));
    }
    #[cfg(test)]
    fn authoritative_length_passes_for_current_test_thread() -> usize {
        PROOF_ATTACHMENT_LIST_AUTHORITATIVE_LENGTH_PASSES.with(std::cell::Cell::get)
    }
    /// Borrow the ordered attachments.
    #[must_use]
    pub fn as_slice(&self) -> &[ProofAttachment] {
        &self.0
    }
    /// Return the attachment count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }
    /// Return the allocated attachment capacity.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }
    /// Return whether this list is empty.
    ///
    /// Valid constructed values always return `false`; the method is provided
    /// for ordinary collection-style inspection.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    /// Consume the wrapper and return its ordered attachments.
    #[must_use]
    pub fn into_vec(self) -> Vec<ProofAttachment> {
        self.0
    }
    /// Append one attachment while preserving the first-release count and canonical-frame bounds.
    ///
    /// The list is left unchanged when the appended value would violate an invariant.
    ///
    /// # Errors
    ///
    /// Returns [`ProofAttachmentListError`] when the attachment count or the
    /// canonical encoded frame would exceed its first-release bound.
    pub fn try_push(
        &mut self,
        attachment: ProofAttachment,
    ) -> Result<(), ProofAttachmentListError> {
        if self.0.len() >= PROOF_ATTACHMENT_LIST_MAX_ATTACHMENTS_V1 {
            return Err(ProofAttachmentListError::TooMany {
                actual: self.0.len().saturating_add(1),
                maximum: PROOF_ATTACHMENT_LIST_MAX_ATTACHMENTS_V1,
            });
        }
        self.0.push(attachment);
        let validation = match self.canonical_frame_len_v1() {
            Ok(canonical_frame_len)
                if canonical_frame_len <= PROOF_ATTACHMENT_LIST_MAX_CANONICAL_FRAME_BYTES_V1 =>
            {
                Ok(())
            }
            Ok(canonical_frame_len) => Err(ProofAttachmentListError::CanonicalFrameTooLarge {
                actual: canonical_frame_len,
                maximum: PROOF_ATTACHMENT_LIST_MAX_CANONICAL_FRAME_BYTES_V1,
            }),
            Err(error) => Err(error),
        };
        if validation.is_err() {
            let _ = self.0.pop();
        }
        validation
    }
}
impl TryFrom<Vec<ProofAttachment>> for ProofAttachmentList {
    type Error = ProofAttachmentListError;
    fn try_from(attachments: Vec<ProofAttachment>) -> Result<Self, Self::Error> {
        if attachments.is_empty() {
            return Err(ProofAttachmentListError::Empty);
        }
        if attachments.len() > PROOF_ATTACHMENT_LIST_MAX_ATTACHMENTS_V1 {
            return Err(ProofAttachmentListError::TooMany {
                actual: attachments.len(),
                maximum: PROOF_ATTACHMENT_LIST_MAX_ATTACHMENTS_V1,
            });
        }
        let list = Self(attachments);
        let canonical_frame_len = list.canonical_frame_len_v1()?;
        if canonical_frame_len > PROOF_ATTACHMENT_LIST_MAX_CANONICAL_FRAME_BYTES_V1 {
            return Err(ProofAttachmentListError::CanonicalFrameTooLarge {
                actual: canonical_frame_len,
                maximum: PROOF_ATTACHMENT_LIST_MAX_CANONICAL_FRAME_BYTES_V1,
            });
        }
        Ok(list)
    }
}
impl norito::NoritoSerialize for ProofAttachmentList {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), ncore::Error> {
        let field_len = norito::NoritoSerialize::encoded_len_exact(&self.0)
            .ok_or(ncore::Error::LengthMismatch)?;
        ncore::write_len(
            writer,
            u64::try_from(field_len).map_err(|_| ncore::Error::LengthMismatch)?,
        )?;
        norito::NoritoSerialize::serialize(&self.0, writer)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        self.encoded_len_exact()
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        let field_len = norito::NoritoSerialize::encoded_len_exact(&self.0)?;
        ncore::len_prefix_len(field_len).checked_add(field_len)
    }
}
impl<'de> norito::NoritoDeserialize<'de> for ProofAttachmentList {
    fn deserialize(archived: &'de ncore::Archived<Self>) -> Self {
        Self::try_deserialize(archived)
            .expect("ProofAttachmentList deserialization requires a canonical bounded archive")
    }
    fn try_deserialize(archived: &'de ncore::Archived<Self>) -> Result<Self, ncore::Error> {
        let ptr = core::ptr::from_ref(archived).cast::<u8>();
        let bytes = ncore::payload_slice_from_ptr(ptr)?;
        let (list, used) = <Self as ncore::DecodeFromSlice>::decode_from_slice(bytes)?;
        if used != bytes.len() {
            return Err(ncore::Error::LengthMismatch);
        }
        Ok(list)
    }
}
impl<'a> ncore::DecodeFromSlice<'a> for ProofAttachmentList {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), ncore::Error> {
        let canonical_frame_len = Self::canonical_frame_len_from_payload_len(bytes.len())
            .ok_or(ncore::Error::LengthMismatch)?;
        if canonical_frame_len > PROOF_ATTACHMENT_LIST_MAX_CANONICAL_FRAME_BYTES_V1 {
            return Err(ncore::Error::Message(
                "ProofAttachmentList canonical frame exceeds the first-release byte limit".into(),
            ));
        }
        let mut offset = 0_usize;
        let field = take_len_prefixed_slice(
            bytes,
            &mut offset,
            PROOF_ATTACHMENT_LIST_MAX_CANONICAL_FRAME_BYTES_V1,
        )?;
        if offset != bytes.len() {
            return Err(ncore::Error::LengthMismatch);
        }
        // Inspect the fixed V1 sequence count before Vec's planner can reserve
        // storage or inspect attacker-controlled element spans.
        let (attachments, _) = ncore::inspect_seq_len_slice(field)?;
        if attachments == 0 {
            return Err(ncore::Error::Message(
                "ProofAttachmentList must not be empty".into(),
            ));
        }
        if attachments > PROOF_ATTACHMENT_LIST_MAX_ATTACHMENTS_V1 {
            return Err(ncore::Error::Message(format!(
                "ProofAttachmentList attachment count {attachments} exceeds the first-release maximum of {PROOF_ATTACHMENT_LIST_MAX_ATTACHMENTS_V1}"
            )));
        }
        let (attachments, used) =
            <Vec<ProofAttachment> as ncore::DecodeFromSlice>::decode_from_slice(field)?;
        if used != field.len() {
            return Err(ncore::Error::LengthMismatch);
        }
        let list = Self::try_from(attachments)
            .map_err(|error| ncore::Error::Message(error.to_string()))?;
        Ok((list, offset))
    }
}
#[cfg(feature = "json")]
fn proof_attachment_list_base64_encoded_len(decoded_len: usize) -> Option<usize> {
    decoded_len
        .checked_add(2)
        .and_then(|length| length.checked_div(3))
        .and_then(|length| length.checked_mul(4))
}
#[cfg(feature = "json")]
fn proof_attachment_list_base64_sextet(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}
#[cfg(feature = "json")]
fn proof_attachment_list_json_error(message: &'static str) -> norito::json::Error {
    norito::json::Error::InvalidField {
        field: "ProofAttachmentList".into(),
        message: message.into(),
    }
}
#[cfg(feature = "json")]
fn proof_attachment_list_base64_decoded_len(
    encoded: &str,
    maximum_decoded_bytes: usize,
) -> Result<usize, norito::json::Error> {
    let maximum_encoded_bytes = proof_attachment_list_base64_encoded_len(maximum_decoded_bytes)
        .ok_or_else(|| proof_attachment_list_json_error("base64 length arithmetic overflow"))?;
    if encoded.is_empty()
        || encoded.len() > maximum_encoded_bytes
        || !encoded.len().is_multiple_of(4)
    {
        return Err(proof_attachment_list_json_error(
            "base64 token has a non-canonical length",
        ));
    }
    let padding = match encoded.as_bytes() {
        [.., b'=', b'='] => 2,
        [.., b'='] => 1,
        _ => 0,
    };
    let payload_len = encoded.len() - padding;
    let bytes = encoded.as_bytes();
    if bytes[..payload_len]
        .iter()
        .any(|byte| proof_attachment_list_base64_sextet(*byte).is_none())
        || bytes[payload_len..].iter().any(|byte| *byte != b'=')
    {
        return Err(proof_attachment_list_json_error(
            "expected canonical padded standard base64",
        ));
    }
    let tail_is_canonical = match padding {
        0 => true,
        1 => {
            payload_len % 4 == 3
                && proof_attachment_list_base64_sextet(bytes[payload_len - 1])
                    .is_some_and(|sextet| sextet.is_multiple_of(4))
        }
        2 => {
            payload_len % 4 == 2
                && proof_attachment_list_base64_sextet(bytes[payload_len - 1])
                    .is_some_and(|sextet| sextet.is_multiple_of(16))
        }
        _ => false,
    };
    if !tail_is_canonical {
        return Err(proof_attachment_list_json_error(
            "base64 token has non-canonical tail bits",
        ));
    }
    let decoded_len = encoded
        .len()
        .checked_div(4)
        .and_then(|length| length.checked_mul(3))
        .and_then(|length| length.checked_sub(padding))
        .ok_or_else(|| proof_attachment_list_json_error("invalid base64 decoded length"))?;
    if decoded_len > maximum_decoded_bytes {
        return Err(proof_attachment_list_json_error(
            "decoded frame exceeds the first-release byte limit",
        ));
    }
    Ok(decoded_len)
}
#[cfg(feature = "json")]
fn proof_attachment_list_borrowed_base64_token<'a>(
    parser: &mut norito::json::Parser<'a>,
    maximum_decoded_bytes: usize,
) -> Result<(&'a str, usize), norito::json::Error> {
    let maximum_encoded_bytes = proof_attachment_list_base64_encoded_len(maximum_decoded_bytes)
        .ok_or_else(|| proof_attachment_list_json_error("base64 length arithmetic overflow"))?;
    parser.skip_ws();
    let start = parser.position();
    let decoded_token_len = parser.skip_string_bounded(maximum_encoded_bytes)?;
    let end = parser.position();
    let encoded = parser
        .input()
        .get(start.saturating_add(1)..end.saturating_sub(1))
        .ok_or_else(|| proof_attachment_list_json_error("invalid JSON string bounds"))?;
    if encoded.len() != decoded_token_len || encoded.as_bytes().contains(&b'\\') {
        return Err(proof_attachment_list_json_error(
            "base64 token must use its unescaped canonical spelling",
        ));
    }
    let decoded_len = proof_attachment_list_base64_decoded_len(encoded, maximum_decoded_bytes)?;
    Ok((encoded, decoded_len))
}
#[cfg(feature = "json")]
fn proof_attachment_list_validate_limits(
    canonical_frame_bytes: usize,
    attachments: usize,
    maximum_frame_bytes: usize,
    maximum_attachments: usize,
) -> Result<(), norito::json::Error> {
    if canonical_frame_bytes > maximum_frame_bytes {
        return Err(proof_attachment_list_json_error(
            "canonical frame exceeds the first-release byte limit",
        ));
    }
    if attachments == 0 {
        return Err(proof_attachment_list_json_error(
            "proof attachment list must not be empty",
        ));
    }
    if attachments > maximum_attachments {
        return Err(proof_attachment_list_json_error(
            "attachment count exceeds the first-release limit",
        ));
    }
    Ok(())
}
#[cfg(feature = "json")]
fn proof_attachment_list_frame_attachment_count(
    canonical_frame: &[u8],
) -> Result<usize, norito::json::Error> {
    let header = ncore::Header::read(std::io::Cursor::new(canonical_frame)).map_err(|_| {
        proof_attachment_list_json_error("base64 payload is not a complete Norito frame")
    })?;
    if header.compression != ncore::Compression::None {
        return Err(proof_attachment_list_json_error(
            "canonical proof attachment lists must be uncompressed",
        ));
    }
    let payload_len = usize::try_from(header.length).map_err(|_| {
        proof_attachment_list_json_error("Norito payload length exceeds this platform")
    })?;
    let payload_start = canonical_frame
        .len()
        .checked_sub(payload_len)
        .filter(|start| *start >= ncore::Header::SIZE)
        .ok_or_else(|| proof_attachment_list_json_error("truncated Norito frame payload"))?;
    let payload = &canonical_frame[payload_start..];
    // ProofAttachmentList is a one-field tuple struct. Canonical V1 uses one
    // compact field length followed by the Vec's fixed-width sequence count
    // and elements. This mirrors the bounded custom Norito decoder.
    let _canonical_flags = ncore::DecodeFlagsGuard::enter(ncore::default_encode_flags());
    let (field_len, field_header_len) = ncore::read_len_dyn_slice(payload).map_err(|_| {
        proof_attachment_list_json_error("malformed proof attachment list field framing")
    })?;
    let field_end = field_header_len
        .checked_add(field_len)
        .filter(|end| *end == payload.len())
        .ok_or_else(|| {
            proof_attachment_list_json_error("non-canonical proof attachment list field length")
        })?;
    let field = payload
        .get(field_header_len..field_end)
        .ok_or_else(|| proof_attachment_list_json_error("truncated attachment sequence"))?;
    let (attachments, _) = ncore::inspect_seq_len_slice(field).map_err(|_| {
        proof_attachment_list_json_error("malformed proof attachment sequence length")
    })?;
    Ok(attachments)
}
#[cfg(feature = "json")]
impl norito::json::JsonSerialize for ProofAttachmentList {
    fn json_serialize(&self, out: &mut String) {
        norito::json::write_canonical_base64_json(self, out);
    }
    fn json_serialize_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        norito::json::write_canonical_base64_json_to(self, out)
    }
}
#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for ProofAttachmentList {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let (encoded, decoded_len) = proof_attachment_list_borrowed_base64_token(
            parser,
            PROOF_ATTACHMENT_LIST_MAX_CANONICAL_FRAME_BYTES_V1,
        )?;
        let bytes = STANDARD
            .decode(encoded)
            .map_err(|err| norito::json::Error::Message(err.to_string()))?;
        if bytes.len() != decoded_len {
            return Err(proof_attachment_list_json_error(
                "base64 decoder length disagrees with canonical preflight",
            ));
        }
        let attachments = proof_attachment_list_frame_attachment_count(&bytes)?;
        proof_attachment_list_validate_limits(
            bytes.len(),
            attachments,
            PROOF_ATTACHMENT_LIST_MAX_CANONICAL_FRAME_BYTES_V1,
            PROOF_ATTACHMENT_LIST_MAX_ATTACHMENTS_V1,
        )?;
        let list = norito::decode_canonical::<ProofAttachmentList>(&bytes)
            .map_err(|err| norito::json::Error::Message(err.to_string()))?;
        proof_attachment_list_validate_limits(
            bytes.len(),
            list.len(),
            PROOF_ATTACHMENT_LIST_MAX_CANONICAL_FRAME_BYTES_V1,
            PROOF_ATTACHMENT_LIST_MAX_ATTACHMENTS_V1,
        )?;
        Ok(list)
    }
    fn json_from_value(value: &norito::json::Value) -> Result<Self, norito::json::Error> {
        let encoded = value
            .as_str()
            .ok_or_else(|| proof_attachment_list_json_error("expected canonical base64 string"))?;
        proof_attachment_list_base64_decoded_len(
            encoded,
            PROOF_ATTACHMENT_LIST_MAX_CANONICAL_FRAME_BYTES_V1,
        )?;
        let canonical_json = norito::json::to_json(value)?;
        norito::json::from_str(&canonical_json)
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
    #[cfg_attr(
        feature = "json",
        norito(
            with = "crate::json_helpers::fixed_bytes",
            bounded_with = "crate::json_helpers::fixed_bytes::serialize_bounded"
        )
    )]
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
    fn json_serialize_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        fn write_escaped_fragment(
            value: &str,
            out: &mut dyn norito::json::JsonWriteSink,
        ) -> Result<(), norito::json::BoundedJsonError> {
            const HEX: &[u8; 16] = b"0123456789abcdef";
            for ch in value.chars() {
                match ch {
                    '"' => out.push_str("\\\"")?,
                    '\\' => out.push_str("\\\\")?,
                    '\n' => out.push_str("\\n")?,
                    '\r' => out.push_str("\\r")?,
                    '\t' => out.push_str("\\t")?,
                    '\u{08}' => out.push_str("\\b")?,
                    '\u{0C}' => out.push_str("\\f")?,
                    control if (control as u32) < 0x20 => {
                        let byte = control as u8;
                        out.push_str("\\u00")?;
                        out.push(char::from(HEX[usize::from(byte >> 4)]))?;
                        out.push(char::from(HEX[usize::from(byte & 0x0f)]))?;
                    }
                    ordinary => out.push(ordinary)?,
                }
            }
            Ok(())
        }
        const UPPER_HEX: &[u8; 16] = b"0123456789ABCDEF";
        out.push('"')?;
        write_escaped_fragment(self.backend.as_str(), out)?;
        out.push(':')?;
        for byte in self.proof_hash {
            out.push(char::from(UPPER_HEX[usize::from(byte >> 4)]))?;
            out.push(char::from(UPPER_HEX[usize::from(byte & 0x0f)]))?;
        }
        out.push('"')
    }
}
impl core::str::FromStr for ProofId {
    type Err = &'static str;
    /// Parse a stable string form produced by Display: `"<backend>:<hex32bytes>"`.
    ///
    /// - `backend` is parsed as `iroha_schema::Ident` (verbatim substring before the last ':').
    /// - `hex32bytes` must be exactly 64 hex chars (case-insensitive). Optional `0x` prefix is allowed.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (backend_str, hex_str) = s.rsplit_once(':').ok_or("missing ':'")?;
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
    #[test]
    fn proof_id_parse_roundtrips_backend_labels_with_colons() {
        let id = ProofId {
            backend: "halo2/ipa:colon-profile".into(),
            proof_hash: [0xCD; 32],
        };
        let parsed = id.to_string().parse::<ProofId>().expect("parse");
        assert_eq!(parsed, id);
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
    fn json_serialize_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        let label = match self {
            ProofStatus::Submitted => "Submitted",
            ProofStatus::Verified => "Verified",
            ProofStatus::Rejected => "Rejected",
        };
        norito::json::write_json_string_to(label, out)
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
    /// Optional verifying key commitment (32-byte stable hash) used during verification.
    #[cfg_attr(
        feature = "json",
        norito(
            with = "crate::json_helpers::fixed_bytes::option",
            bounded_with = "crate::json_helpers::fixed_bytes::option::serialize_bounded"
        )
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
    use super::*;
    use iroha_crypto::{Hash, HashOf, LaneCommitmentId, MerkleProof};
    fn write_test_field<T: norito::NoritoSerialize>(encoded: &mut Vec<u8>, value: &T) {
        let mut field = Vec::new();
        ncore::serialize_to_buffer(value, &mut field).expect("serialize test field");
        ncore::write_len_header_to_vec(encoded, field.len() as u64);
        encoded.extend_from_slice(&field);
    }
    fn proof_bytes_hash(bytes: &[u8]) -> [u8; 32] {
        iroha_crypto::Hash::new(bytes).into()
    }
    #[cfg(feature = "json")]
    fn hash_json(hash: &[u8; 32]) -> String {
        let body = hash
            .iter()
            .map(std::string::ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        format!("[{body}]")
    }
    fn lane_privacy_with_path(
        leaf_index: u32,
        audit_path: Vec<Option<HashOf<[u8; 32]>>>,
    ) -> crate::nexus::LanePrivacyProof {
        crate::nexus::LanePrivacyProof {
            commitment_id: LaneCommitmentId::new(5),
            witness: crate::nexus::LanePrivacyWitness::Merkle(
                crate::nexus::LanePrivacyMerkleWitness {
                    leaf: [0xAA; 32],
                    proof: MerkleProof::from_audit_path(leaf_index, audit_path),
                },
            ),
        }
    }
    fn canonical_lane_sibling(seed: u8) -> HashOf<[u8; 32]> {
        HashOf::from_untyped_unchecked(Hash::prehashed([seed; Hash::LENGTH]))
    }
    fn bounded_attachment_list(attachments: Vec<ProofAttachment>) -> ProofAttachmentList {
        ProofAttachmentList::try_from(attachments).expect("valid bounded attachment-list fixture")
    }
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
    #[norito(reuse_archived, decode_from_slice)]
    struct ReferenceProofAttachmentList(Vec<ProofAttachment>);
    #[test]
    fn proof_attachment_list_roundtrip_bare() {
        let mut attachment = ProofAttachment::new_ref(
            "halo2/ipa".into(),
            ProofBox::new("halo2/ipa".into(), vec![1, 2]),
            VerifyingKeyId::new("halo2/ipa", "vk_1"),
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
        let list = bounded_attachment_list(vec![attachment]);
        let bytes = norito::encode_canonical(&list).expect("encode canonical lane attachment list");
        assert_eq!(
            ncore::encoded_frame_len(&list).expect("count canonical lane attachment list"),
            bytes.len(),
            "valid all-Some lane witnesses must expose exact canonical frame sizing"
        );
        let decoded = norito::decode_canonical::<ProofAttachmentList>(&bytes)
            .expect("decode canonical lane attachment list");
        assert_eq!(decoded, list);
    }
    #[test]
    fn proof_attachment_list_custom_wire_matches_independent_derived_tuple_codec() {
        let first = ProofAttachment::new_ref(
            "halo2/ipa".into(),
            ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]),
            VerifyingKeyId::new("halo2/ipa", "vk_1"),
        );
        let second = ProofAttachment::new_ref(
            "stark/fri".into(),
            ProofBox::new("stark/fri".into(), vec![4, 5, 6, 7]),
            VerifyingKeyId::new("stark/fri", "vk_2"),
        );
        let list = bounded_attachment_list(vec![first.clone(), second.clone()]);
        let reference = ReferenceProofAttachmentList(vec![first, second]);
        let _canonical_flags = ncore::DecodeFlagsGuard::enter(ncore::default_encode_flags());
        let mut custom_bare = Vec::new();
        ncore::serialize_to_buffer(&list, &mut custom_bare)
            .expect("serialize custom bounded list payload");
        let mut reference_bare = Vec::new();
        ncore::serialize_to_buffer(&reference, &mut reference_bare)
            .expect("serialize independently derived tuple payload");
        assert_eq!(
            custom_bare, reference_bare,
            "custom first-release codec must preserve the original one-field tuple wire"
        );
        let (decoded_reference, reference_used) =
            <ReferenceProofAttachmentList as ncore::DecodeFromSlice>::decode_from_slice(
                &custom_bare,
            )
            .expect("derived reference decoder accepts custom payload");
        assert_eq!(reference_used, custom_bare.len());
        assert_eq!(decoded_reference, reference);
        let (decoded_custom, custom_used) =
            <ProofAttachmentList as ncore::DecodeFromSlice>::decode_from_slice(&reference_bare)
                .expect("custom bounded decoder accepts derived reference payload");
        assert_eq!(custom_used, reference_bare.len());
        assert_eq!(decoded_custom, list);
    }
    #[test]
    fn proof_attachment_list_constructor_enforces_first_release_cardinality() {
        let attachment = ProofAttachment::new_ref(
            "halo2/ipa".into(),
            ProofBox::new("halo2/ipa".into(), vec![1]),
            VerifyingKeyId::new("halo2/ipa", "vk_1"),
        );
        assert!(matches!(
            ProofAttachmentList::try_from(Vec::new()),
            Err(ProofAttachmentListError::Empty)
        ));
        let maximum = ProofAttachmentList::try_from(vec![
            attachment.clone();
            PROOF_ATTACHMENT_LIST_MAX_ATTACHMENTS_V1
        ])
        .expect("exact verifier batch boundary must construct");
        assert_eq!(maximum.len(), PROOF_ATTACHMENT_LIST_MAX_ATTACHMENTS_V1);
        assert!(!maximum.is_empty());
        assert_eq!(maximum.as_slice().len(), maximum.len());
        let frame = norito::encode_canonical(&maximum).expect("encode maximum-count list");
        let decoded = norito::decode_canonical::<ProofAttachmentList>(&frame)
            .expect("maximum-count list must round-trip canonically");
        assert_eq!(decoded, maximum);
        assert!(matches!(
            ProofAttachmentList::try_from(vec![
                attachment;
                PROOF_ATTACHMENT_LIST_MAX_ATTACHMENTS_V1 + 1
            ]),
            Err(ProofAttachmentListError::TooMany {
                actual,
                maximum: PROOF_ATTACHMENT_LIST_MAX_ATTACHMENTS_V1,
            }) if actual == PROOF_ATTACHMENT_LIST_MAX_ATTACHMENTS_V1 + 1
        ));
    }
    #[test]
    fn proof_attachment_list_try_push_preserves_order_and_rolls_back_cardinality_failure() {
        let attachment = |byte| {
            ProofAttachment::new_ref(
                "halo2/ipa".into(),
                ProofBox::new("halo2/ipa".into(), vec![byte]),
                VerifyingKeyId::new("halo2/ipa", "vk_1"),
            )
        };
        let first = attachment(1);
        let second = attachment(2);
        let mut list = bounded_attachment_list(vec![first.clone()]);
        list.try_push(second.clone())
            .expect("second attachment remains within all list limits");
        assert_eq!(list.as_slice(), [first, second]);
        let mut maximum = bounded_attachment_list(vec![
            attachment(3);
            PROOF_ATTACHMENT_LIST_MAX_ATTACHMENTS_V1
        ]);
        let before = norito::encode_canonical(&maximum).expect("encode pre-failure list");
        assert!(matches!(
            maximum.try_push(attachment(4)),
            Err(ProofAttachmentListError::TooMany {
                actual,
                maximum: PROOF_ATTACHMENT_LIST_MAX_ATTACHMENTS_V1,
            }) if actual == PROOF_ATTACHMENT_LIST_MAX_ATTACHMENTS_V1 + 1
        ));
        assert_eq!(maximum.len(), PROOF_ATTACHMENT_LIST_MAX_ATTACHMENTS_V1);
        assert_eq!(
            norito::encode_canonical(&maximum).expect("encode rolled-back list"),
            before,
            "failed cardinality append must leave the list byte-for-byte unchanged"
        );
    }
    #[test]
    fn proof_attachment_list_exact_frame_boundary_and_try_push_rollback() {
        let attachment = |proof_bytes| {
            ProofAttachment::new_ref(
                "halo2/ipa".into(),
                ProofBox::new("halo2/ipa".into(), vec![0_u8; proof_bytes]),
                VerifyingKeyId::new("halo2/ipa", "vk_1"),
            )
        };
        let mut low = 1_usize;
        let mut high = PROOF_ATTACHMENT_LIST_MAX_CANONICAL_FRAME_BYTES_V1;
        while low < high {
            let midpoint = low + (high - low).div_ceil(2);
            if ProofAttachmentList::try_from(vec![attachment(midpoint)]).is_ok() {
                low = midpoint;
            } else {
                high = midpoint - 1;
            }
        }
        let mut list = ProofAttachmentList::try_from(vec![attachment(low)])
            .expect("binary search returns the largest fitting one-attachment list");
        let before = norito::encode_canonical(&list).expect("encode exact-cap list");
        assert_eq!(
            before.len(),
            PROOF_ATTACHMENT_LIST_MAX_CANONICAL_FRAME_BYTES_V1,
            "largest accepted proof must fill the complete canonical frame ceiling exactly"
        );
        assert_eq!(
            ncore::encoded_frame_len(&list).expect("authoritative counting serialization"),
            before.len()
        );
        let hinted_payload = norito::NoritoSerialize::encoded_len_exact(&list)
            .expect("bounded list exposes an exact serialization hint");
        assert_eq!(
            ProofAttachmentList::canonical_frame_len_from_payload_len(hinted_payload)
                .expect("hinted frame length arithmetic"),
            before.len(),
            "the optimization hint must agree with authoritative emitted bytes"
        );
        assert_eq!(
            norito::decode_canonical::<ProofAttachmentList>(&before)
                .expect("exact-cap canonical frame must decode"),
            list
        );
        #[cfg(feature = "json")]
        {
            let json = norito::json::to_json(&list).expect("encode exact-cap list JSON");
            assert_eq!(
                norito::json::from_str::<ProofAttachmentList>(&json)
                    .expect("exact-cap canonical base64 list must decode"),
                list
            );
        }
        let next_error = ProofAttachmentList::try_from(vec![attachment(low + 1)])
            .expect_err("the next proof byte must cross the frame ceiling");
        assert!(matches!(
            next_error,
            ProofAttachmentListError::CanonicalFrameTooLarge {
                actual,
                maximum: PROOF_ATTACHMENT_LIST_MAX_CANONICAL_FRAME_BYTES_V1,
            } if actual == PROOF_ATTACHMENT_LIST_MAX_CANONICAL_FRAME_BYTES_V1 + 1
        ));
        ProofAttachmentList::reset_authoritative_length_passes_for_current_test_thread();
        let error = list
            .try_push(attachment(1))
            .expect_err("one more attachment must cross the canonical frame ceiling");
        assert!(matches!(
            error,
            ProofAttachmentListError::CanonicalFrameTooLarge {
                actual,
                maximum: PROOF_ATTACHMENT_LIST_MAX_CANONICAL_FRAME_BYTES_V1,
            } if actual > PROOF_ATTACHMENT_LIST_MAX_CANONICAL_FRAME_BYTES_V1
        ));
        assert_eq!(
            ProofAttachmentList::authoritative_length_passes_for_current_test_thread(),
            0,
            "a length hint above the ceiling must reject before the allocating serializer"
        );
        assert_eq!(list.len(), 1);
        assert_eq!(
            norito::encode_canonical(&list).expect("encode rolled-back list"),
            before,
            "failed byte-limit append must leave the list byte-for-byte unchanged"
        );
    }
    #[test]
    fn proof_attachment_list_constructor_rejects_frame_above_byte_cap() {
        let attachment = ProofAttachment::new_ref(
            "halo2/ipa".into(),
            ProofBox::new(
                "halo2/ipa".into(),
                vec![0_u8; PROOF_ATTACHMENT_LIST_MAX_CANONICAL_FRAME_BYTES_V1],
            ),
            VerifyingKeyId::new("halo2/ipa", "vk_1"),
        );
        let error = ProofAttachmentList::try_from(vec![attachment])
            .expect_err("payload alone at the frame ceiling leaves no framing headroom");
        assert!(matches!(
            error,
            ProofAttachmentListError::CanonicalFrameTooLarge {
                actual,
                maximum: PROOF_ATTACHMENT_LIST_MAX_CANONICAL_FRAME_BYTES_V1,
            } if actual > PROOF_ATTACHMENT_LIST_MAX_CANONICAL_FRAME_BYTES_V1
        ));
    }
    #[test]
    fn proof_attachment_list_gross_oversize_rejects_before_authoritative_serialization() {
        let mut attachment = ProofAttachment::new_ref(
            "halo2/ipa".into(),
            ProofBox::new(
                "halo2/ipa".into(),
                vec![0_u8; PROOF_ATTACHMENT_LIST_MAX_CANONICAL_FRAME_BYTES_V1],
            ),
            VerifyingKeyId::new("halo2/ipa", "vk_1"),
        );
        attachment.lane_privacy = Some(lane_privacy_with_path(
            1,
            vec![Some(canonical_lane_sibling(0xBB))],
        ));
        assert!(attachment.structural_error().is_none());
        ProofAttachmentList::reset_authoritative_length_passes_for_current_test_thread();
        assert!(matches!(
            ProofAttachmentList::try_from(vec![attachment]),
            Err(ProofAttachmentListError::CanonicalFrameTooLarge {
                actual,
                maximum: PROOF_ATTACHMENT_LIST_MAX_CANONICAL_FRAME_BYTES_V1,
            }) if actual > PROOF_ATTACHMENT_LIST_MAX_CANONICAL_FRAME_BYTES_V1
        ));
        assert_eq!(
            ProofAttachmentList::authoritative_length_passes_for_current_test_thread(),
            0,
            "an oversized but otherwise valid lane proof must fail without staging serializer buffers"
        );
    }
    #[test]
    fn proof_attachment_list_decode_rejects_forged_count_before_vec_decode() {
        let attachment = ProofAttachment::new_ref(
            "halo2/ipa".into(),
            ProofBox::new("halo2/ipa".into(), vec![1]),
            VerifyingKeyId::new("halo2/ipa", "vk_1"),
        );
        let list = bounded_attachment_list(vec![attachment]);
        let mut bare = Vec::new();
        ncore::serialize_to_buffer(&list, &mut bare).expect("serialize list payload");
        let (_, field_header_len) = ncore::read_len_dyn_slice(&bare).expect("list field header");
        for forged_count in [0, PROOF_ATTACHMENT_LIST_MAX_ATTACHMENTS_V1 + 1, usize::MAX] {
            let mut forged = bare.clone();
            let count = u64::try_from(forged_count).unwrap_or(u64::MAX);
            forged[field_header_len..field_header_len + 8].copy_from_slice(&count.to_le_bytes());
            let error = <ProofAttachmentList as ncore::DecodeFromSlice>::decode_from_slice(&forged)
                .expect_err("forged outer sequence count must reject before element decoding");
            let message = error.to_string();
            assert!(
                message.contains("must not be empty")
                    || message.contains("attachment count")
                    || message.contains("sequence length"),
                "unexpected forged-count error: {error}"
            );
        }
    }
    #[test]
    fn proof_attachment_list_decode_rejects_frame_cap_plus_one_before_field_parsing() {
        let alignment = ncore::archived_payload_align::<ProofAttachmentList>();
        let remainder = ncore::Header::SIZE % alignment;
        let padding = if remainder == 0 {
            0
        } else {
            alignment - remainder
        };
        let maximum_payload = PROOF_ATTACHMENT_LIST_MAX_CANONICAL_FRAME_BYTES_V1
            .checked_sub(ncore::Header::SIZE + padding)
            .expect("frame ceiling exceeds fixed framing");
        let oversized_payload = vec![0_u8; maximum_payload + 1];
        let error =
            <ProofAttachmentList as ncore::DecodeFromSlice>::decode_from_slice(&oversized_payload)
                .expect_err(
                    "standalone canonical frame cap plus one must fail before field parsing",
                );
        assert!(
            error.to_string().contains("canonical frame exceeds"),
            "unexpected cap+1 payload rejection: {error}"
        );
    }
    #[test]
    fn proofbox_norito_roundtrip() {
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
        let backend: iroha_schema::Ident = "halo2/ipa".into();
        let vk = VerifyingKeyBox::new(backend, vec![7, 7, 7]);
        let enc = norito::to_bytes(&vk).expect("encode");
        let arch = norito::from_bytes::<VerifyingKeyBox>(&enc).expect("archived");
        let dec: VerifyingKeyBox = norito::core::NoritoDeserialize::deserialize(arch);
        assert_eq!(dec.backend, "halo2/ipa".to_owned());
        assert_eq!(dec.bytes, vec![7, 7, 7]);
    }
    #[test]
    fn verifying_key_id_decode_from_slice_roundtrip() {
        let id = VerifyingKeyId::new("halo2/ipa", "vk_transfer");
        let encoded = id.encode();
        let (decoded, used) =
            <VerifyingKeyId as ncore::DecodeFromSlice>::decode_from_slice(&encoded)
                .expect("decode verifying key id from exact slice");
        assert_eq!(used, encoded.len());
        assert_eq!(decoded, id);
    }
    #[test]
    fn verifying_key_id_portable_registry_id_predicate_is_fail_closed() {
        for (backend, name) in [
            ("halo2/ipa", "vk_transfer"),
            ("halo2/ipa", "halo2/ipa::transfer_v1"),
            ("stark/fri/sha256-goldilocks", "zk_ace.v1"),
            ("stark/fri/sha256_goldilocks.v1", "zk-ace-pq-v0"),
        ] {
            let id = VerifyingKeyId::new(backend, name);
            assert!(
                id.is_portable_registry_id(),
                "portable verifier-key id `{backend}` / `{name}` must be accepted"
            );
        }
        for (label, backend, name) in [
            ("blank-backend", " ", "vk_transfer"),
            ("blank-name", "halo2/ipa", " "),
            ("uppercase-backend", "Halo2/ipa", "vk_transfer"),
            ("uppercase-name", "halo2/ipa", "VkTransfer"),
            ("control-backend", "halo2/ipa\nforged", "vk_transfer"),
            ("control-name", "halo2/ipa", "vk\nforged"),
            ("zero-width-backend", "halo2/ipa\u{200B}", "vk_transfer"),
            ("zero-width-name", "halo2/ipa", "vk\u{200B}transfer"),
            ("path-traversal-backend", "halo2/ipa/../vk", "vk_transfer"),
            ("path-traversal-name", "halo2/ipa", "vk/../transfer"),
            ("dot-segment-backend", "halo2/ipa/./vk", "vk_transfer"),
            ("dot-segment-name", "halo2/ipa", "vk/./transfer"),
            ("hidden-backend", "halo2/.ipa", "vk_transfer"),
            ("hidden-name", "halo2/ipa", ".vk_transfer"),
            ("slash-colon-backend", "halo2/ipa/:vk", "vk_transfer"),
            ("colon-slash-name", "halo2/ipa", "vk:/transfer"),
            ("backslash-backend", "halo2\\ipa", "vk_transfer"),
            ("backslash-name", "halo2/ipa", "vk\\transfer"),
            ("leading-delimiter-name", "halo2/ipa", "-vk_transfer"),
            ("trailing-delimiter-name", "halo2/ipa", "vk_transfer_"),
        ] {
            let id = VerifyingKeyId::new(backend, name);
            assert!(
                !id.is_portable_registry_id(),
                "case {label} must reject verifier-key id `{backend}` / `{name}`"
            );
        }
        let oversized = "a".repeat(VERIFYING_KEY_ID_MAX_FIELD_BYTES + 1);
        assert!(!VerifyingKeyId::new("halo2/ipa", oversized.as_str()).is_portable_registry_id());
        assert!(!VerifyingKeyId::new(oversized.as_str(), "vk_transfer").is_portable_registry_id());
    }
    #[test]
    fn vk_record_roundtrip() {
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
    fn verifying_key_record_active_at_respects_height_window() {
        let mut rec = VerifyingKeyRecord::new(
            1,
            "halo2/ipa:height-window",
            BackendTag::Halo2IpaPasta,
            "pasta",
            [0xAA; 32],
            [0xBB; 32],
        );
        rec.status = ConfidentialStatus::Active;
        assert!(rec.is_active_at(1));
        rec.activation_height = Some(2);
        assert!(!rec.is_active_at(1));
        assert!(rec.is_active_at(2));
        rec.withdraw_height = Some(4);
        assert!(rec.is_active_at(3));
        assert!(!rec.is_active_at(4));
        rec.status = ConfidentialStatus::Proposed;
        assert!(!rec.is_active_at(3));
    }
    #[test]
    fn proof_attachment_roundtrip() {
        let p = ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]);
        let id = VerifyingKeyId::new("halo2/ipa", "vk_1");
        let a = ProofAttachment::new_ref("halo2/ipa".into(), p.clone(), id);
        let enc = norito::to_bytes(&a).expect("encode");
        let arch = norito::from_bytes::<ProofAttachment>(&enc).expect("archived");
        let dec: ProofAttachment = norito::core::NoritoDeserialize::deserialize(arch);
        assert_eq!(dec.backend, "halo2/ipa".to_owned());
        assert_eq!(dec.vk_ref.name.as_str(), "vk_1");
    }
    #[test]
    fn proof_attachment_decode_accepts_matching_envelope_hash() {
        let proof = ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]);
        let mut attachment = ProofAttachment::new_ref(
            "halo2/ipa".into(),
            proof.clone(),
            VerifyingKeyId::new("halo2/ipa", "vk_1"),
        );
        attachment.envelope_hash = Some(proof_bytes_hash(&proof.bytes));
        let encoded = norito::to_bytes(&attachment).expect("encode attachment");
        let decoded = norito::decode_from_bytes::<ProofAttachment>(&encoded)
            .expect("matching envelope hash must decode");
        assert_eq!(decoded.envelope_hash, attachment.envelope_hash);
    }
    #[test]
    fn proof_attachment_decode_rejects_missing_vk_ref_field() {
        let backend: iroha_schema::Ident = "halo2/ipa".into();
        let proof = ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]);
        let mut encoded = Vec::new();
        write_test_field(&mut encoded, &backend);
        write_test_field(&mut encoded, &proof);
        let result = <ProofAttachment as ncore::DecodeFromSlice>::decode_from_slice(&encoded);
        assert!(matches!(result, Err(ncore::Error::LengthMismatch)));
    }
    #[test]
    fn proof_attachment_decode_rejects_legacy_optional_vk_ref_slot() {
        let backend: iroha_schema::Ident = "halo2/ipa".into();
        let proof = ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]);
        let legacy_vk_ref: Option<VerifyingKeyId> = None;
        let legacy_vk_inline = Some(VerifyingKeyBox::new("halo2/ipa".into(), vec![4, 5, 6]));
        let mut encoded = Vec::new();
        write_test_field(&mut encoded, &backend);
        write_test_field(&mut encoded, &proof);
        write_test_field(&mut encoded, &legacy_vk_ref);
        write_test_field(&mut encoded, &legacy_vk_inline);
        let result = <ProofAttachment as ncore::DecodeFromSlice>::decode_from_slice(&encoded);
        assert!(
            result.is_err(),
            "legacy optional vk_ref/vk_inline payload must not decode as registry-only attachment"
        );
    }
    #[test]
    fn proof_attachment_decode_rejects_legacy_some_vk_ref_inline_slots() {
        let backend: iroha_schema::Ident = "halo2/ipa".into();
        let proof = ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]);
        let legacy_vk_ref = Some(VerifyingKeyId::new("halo2/ipa", "legacy_vk"));
        let legacy_vk_inline = Some(VerifyingKeyBox::new("halo2/ipa".into(), vec![4, 5, 6]));
        let mut encoded = Vec::new();
        write_test_field(&mut encoded, &backend);
        write_test_field(&mut encoded, &proof);
        write_test_field(&mut encoded, &legacy_vk_ref);
        write_test_field(&mut encoded, &legacy_vk_inline);
        let result = <ProofAttachment as ncore::DecodeFromSlice>::decode_from_slice(&encoded);
        assert!(
            result.is_err(),
            "legacy Some(vk_ref)/Some(vk_inline) payload must not decode as registry-only attachment"
        );
    }
    #[test]
    fn proof_attachment_decode_rejects_inline_vk_tail_after_vk_ref() {
        let backend: iroha_schema::Ident = "halo2/ipa".into();
        let proof = ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]);
        let vk_ref = VerifyingKeyId::new("halo2/ipa", "vk_1");
        let legacy_vk_inline = Some(VerifyingKeyBox::new("halo2/ipa".into(), vec![4, 5, 6]));
        let mut encoded = Vec::new();
        write_test_field(&mut encoded, &backend);
        write_test_field(&mut encoded, &proof);
        write_test_field(&mut encoded, &vk_ref);
        write_test_field(&mut encoded, &legacy_vk_inline);
        let result = <ProofAttachment as ncore::DecodeFromSlice>::decode_from_slice(&encoded);
        assert!(
            result.is_err(),
            "inline verifying-key tail must not decode as optional vk_commitment"
        );
    }
    #[test]
    fn proof_attachment_decode_rejects_extra_tail_after_allowed_fields() {
        let backend: iroha_schema::Ident = "halo2/ipa".into();
        let proof = ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]);
        let vk_ref = VerifyingKeyId::new("halo2/ipa", "vk_1");
        let vk_commitment = Some([0x11; 32]);
        let envelope_hash = Some([0x22; 32]);
        let lane_privacy: Option<crate::nexus::LanePrivacyProof> = None;
        let extra = Some([0x33; 32]);
        let mut encoded = Vec::new();
        write_test_field(&mut encoded, &backend);
        write_test_field(&mut encoded, &proof);
        write_test_field(&mut encoded, &vk_ref);
        write_test_field(&mut encoded, &vk_commitment);
        write_test_field(&mut encoded, &envelope_hash);
        write_test_field(&mut encoded, &lane_privacy);
        write_test_field(&mut encoded, &extra);
        let result = <ProofAttachment as ncore::DecodeFromSlice>::decode_from_slice(&encoded);
        assert!(
            matches!(result, Err(ncore::Error::LengthMismatch)),
            "extra tail field after lane_privacy must be rejected, got {result:?}"
        );
    }
    #[test]
    fn proof_attachment_decode_rejects_redundant_none_tail_fields() {
        let backend: iroha_schema::Ident = "halo2/ipa".into();
        let proof = ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]);
        let vk_ref = VerifyingKeyId::new("halo2/ipa", "vk_1");
        let required_prefix = || {
            let mut encoded = Vec::new();
            write_test_field(&mut encoded, &backend);
            write_test_field(&mut encoded, &proof);
            write_test_field(&mut encoded, &vk_ref);
            encoded
        };
        let absent_hash: Option<[u8; 32]> = None;
        let present_vk_commitment = Some([0x11; 32]);
        let present_envelope_hash = Some(proof_bytes_hash(&proof.bytes));
        let absent_lane: Option<crate::nexus::LanePrivacyProof> = None;
        let mut malformed = Vec::new();
        let mut trailing_vk_none = required_prefix();
        write_test_field(&mut trailing_vk_none, &absent_hash);
        malformed.push(trailing_vk_none);
        let mut trailing_envelope_none = required_prefix();
        write_test_field(&mut trailing_envelope_none, &present_vk_commitment);
        write_test_field(&mut trailing_envelope_none, &absent_hash);
        malformed.push(trailing_envelope_none);
        let mut trailing_lane_none = required_prefix();
        write_test_field(&mut trailing_lane_none, &absent_hash);
        write_test_field(&mut trailing_lane_none, &present_envelope_hash);
        write_test_field(&mut trailing_lane_none, &absent_lane);
        malformed.push(trailing_lane_none);
        let mut three_redundant_nones = required_prefix();
        write_test_field(&mut three_redundant_nones, &absent_hash);
        write_test_field(&mut three_redundant_nones, &absent_hash);
        write_test_field(&mut three_redundant_nones, &absent_lane);
        malformed.push(three_redundant_nones);
        for encoded in malformed {
            let error = <ProofAttachment as ncore::DecodeFromSlice>::decode_from_slice(&encoded)
                .expect_err("redundant trailing None fields must not have a second wire spelling");
            assert!(
                matches!(&error, ncore::Error::LengthMismatch)
                    || error.to_string().contains("non-canonical redundant"),
                "unexpected error: {error}"
            );
        }
    }
    #[test]
    fn proof_attachment_decode_accepts_none_placeholders_before_later_some_fields() {
        let proof = ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]);
        let mut envelope_only = ProofAttachment::new_ref(
            "halo2/ipa".into(),
            proof.clone(),
            VerifyingKeyId::new("halo2/ipa", "vk_1"),
        );
        envelope_only.envelope_hash = Some(proof_bytes_hash(&proof.bytes));
        let mut lane_only = ProofAttachment::new_ref(
            "halo2/ipa".into(),
            proof,
            VerifyingKeyId::new("halo2/ipa", "vk_1"),
        );
        lane_only.lane_privacy = Some(lane_privacy_with_path(
            0,
            vec![Some(canonical_lane_sibling(0x23))],
        ));
        for attachment in [envelope_only, lane_only] {
            let encoded = norito::to_bytes(&attachment).expect("encode canonical sparse tail");
            let decoded = norito::decode_from_bytes::<ProofAttachment>(&encoded)
                .expect("None placeholders before a later Some field must decode");
            assert_eq!(decoded, attachment);
        }
    }
    #[test]
    fn proof_attachment_decode_rejects_malformed_lane_privacy_paths() {
        let sibling = canonical_lane_sibling(0x22);
        let malformed = [
            lane_privacy_with_path(0, Vec::new()),
            lane_privacy_with_path(0, vec![None]),
            lane_privacy_with_path(2, vec![Some(sibling)]),
            lane_privacy_with_path(
                0,
                vec![Some(sibling); crate::nexus::LANE_PRIVACY_MAX_MERKLE_DEPTH_V1 + 1],
            ),
        ];
        for lane_privacy in malformed {
            let mut attachment = ProofAttachment::new_ref(
                "halo2/ipa".into(),
                ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]),
                VerifyingKeyId::new("halo2/ipa", "vk_1"),
            );
            attachment.lane_privacy = Some(lane_privacy);
            let encoded = norito::to_bytes(&attachment).expect("encode malformed lane witness");
            let error = norito::decode_from_bytes::<ProofAttachment>(&encoded)
                .expect_err("malformed lane witness must not decode inside an attachment");
            assert!(error.to_string().contains("lane_privacy"));
        }
    }
    #[test]
    fn proof_attachment_decode_rejects_blank_verifying_key_name() {
        let attachment = ProofAttachment::new_ref(
            "halo2/ipa".into(),
            ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]),
            VerifyingKeyId::new("halo2/ipa", "   "),
        );
        let encoded = norito::to_bytes(&attachment).expect("encode blank vk name attachment");
        let err = norito::decode_from_bytes::<ProofAttachment>(&encoded)
            .expect_err("blank verifying key names must not decode");
        assert!(err.to_string().contains("vk_ref.name"));
    }
    #[test]
    fn proof_attachment_decode_rejects_blank_backend_fields() {
        let cases = [
            (
                ProofAttachment::new_ref(
                    "   ".into(),
                    ProofBox::new("   ".into(), vec![1, 2, 3]),
                    VerifyingKeyId::new("   ", "vk_1"),
                ),
                "backend",
            ),
            (
                ProofAttachment::new_ref(
                    "halo2/ipa".into(),
                    ProofBox::new("   ".into(), vec![1, 2, 3]),
                    VerifyingKeyId::new("halo2/ipa", "vk_1"),
                ),
                "proof.backend",
            ),
            (
                ProofAttachment::new_ref(
                    "halo2/ipa".into(),
                    ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]),
                    VerifyingKeyId::new("   ", "vk_1"),
                ),
                "vk_ref.backend",
            ),
        ];
        for (attachment, expected_field) in cases {
            let encoded = norito::to_bytes(&attachment).expect("encode blank backend attachment");
            let err = norito::decode_from_bytes::<ProofAttachment>(&encoded)
                .expect_err("blank backend fields must not decode");
            assert!(
                err.to_string().contains(expected_field),
                "expected error to mention {expected_field}, got {err}"
            );
        }
    }
    #[test]
    fn proof_attachment_decode_rejects_nonportable_refs_empty_proofs_and_zero_hashes() {
        let mut zero_vk_commitment = ProofAttachment::new_ref(
            "halo2/ipa".into(),
            ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]),
            VerifyingKeyId::new("halo2/ipa", "vk_1"),
        );
        zero_vk_commitment.vk_commitment = Some([0u8; 32]);
        let mut zero_envelope_hash = ProofAttachment::new_ref(
            "halo2/ipa".into(),
            ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]),
            VerifyingKeyId::new("halo2/ipa", "vk_1"),
        );
        zero_envelope_hash.envelope_hash = Some([0u8; 32]);
        let mut forged_envelope_hash = ProofAttachment::new_ref(
            "halo2/ipa".into(),
            ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]),
            VerifyingKeyId::new("halo2/ipa", "vk_1"),
        );
        let mut forged_hash = proof_bytes_hash(&forged_envelope_hash.proof.bytes);
        forged_hash[0] ^= 0x80;
        forged_envelope_hash.envelope_hash = Some(forged_hash);
        let cases = [
            (
                ProofAttachment::new_ref(
                    "Halo2/ipa".into(),
                    ProofBox::new("Halo2/ipa".into(), vec![1, 2, 3]),
                    VerifyingKeyId::new("Halo2/ipa", "vk_1"),
                ),
                "vk_ref",
            ),
            (
                ProofAttachment::new_ref(
                    "halo2/ipa".into(),
                    ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]),
                    VerifyingKeyId::new("halo2/ipa", "Vk_1"),
                ),
                "vk_ref",
            ),
            (
                ProofAttachment::new_ref(
                    "halo2/ipa".into(),
                    ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]),
                    VerifyingKeyId::new("halo2/ipa", "vk_1\u{200B}"),
                ),
                "vk_ref",
            ),
            (
                ProofAttachment::new_ref(
                    "halo2/ipa".into(),
                    ProofBox::new("halo2/ipa".into(), Vec::new()),
                    VerifyingKeyId::new("halo2/ipa", "vk_1"),
                ),
                "proof.bytes",
            ),
            (zero_vk_commitment, "vk_commitment"),
            (zero_envelope_hash, "envelope_hash"),
            (forged_envelope_hash, "envelope_hash"),
        ];
        for (attachment, expected_field) in cases {
            let encoded = norito::to_bytes(&attachment).expect("encode malformed attachment");
            let err = norito::decode_from_bytes::<ProofAttachment>(&encoded)
                .expect_err("malformed proof attachment must not decode");
            assert!(
                err.to_string().contains(expected_field),
                "expected error to mention {expected_field}, got {err}"
            );
        }
    }
    #[test]
    fn proof_box_canonical_size_limit_accounts_for_backend_and_framing() {
        let backend = "halo2/ipa::transfer_v1";
        let proof = ProofBox::new(backend.into(), vec![1, 2, 3, 4, 5]);
        let canonical_payload =
            ncore::encoded_payload_len(&proof).expect("canonical nested ProofBox payload");
        assert_eq!(proof.canonical_encoded_len_v1(), Some(canonical_payload));
        assert!(
            norito::encode_canonical(&proof)
                .expect("standalone canonical ProofBox frame")
                .len()
                > canonical_payload,
            "a standalone frame adds a header and alignment; the attachment cap covers the complete nested ProofBox payload"
        );
        assert_eq!(
            proof_box_max_proof_bytes_v1(backend),
            Some(PROOF_BOX_MAX_ENCODED_BYTES_V1 - 36)
        );
        let maximum = proof_box_max_proof_bytes_v1(backend).expect("bounded backend");
        let mut maximum_sized_proof = Vec::with_capacity(maximum + 1);
        maximum_sized_proof.resize(maximum, 0xA5);
        let mut attachment = ProofAttachment::new_ref(
            backend.into(),
            ProofBox::new(backend.into(), maximum_sized_proof),
            VerifyingKeyId::new(backend, "transfer_v1"),
        );
        assert_eq!(
            attachment.proof.canonical_encoded_len_v1(),
            Some(PROOF_BOX_MAX_ENCODED_BYTES_V1)
        );
        assert_eq!(
            ncore::encoded_payload_len(&attachment.proof)
                .expect("count maximum nested ProofBox payload"),
            PROOF_BOX_MAX_ENCODED_BYTES_V1
        );
        assert_eq!(attachment.structural_error(), None);
        attachment.proof.bytes.push(0x5A);
        assert_eq!(
            ncore::encoded_payload_len(&attachment.proof)
                .expect("count oversized nested ProofBox payload"),
            PROOF_BOX_MAX_ENCODED_BYTES_V1 + 1
        );
        assert_eq!(
            attachment.structural_error(),
            Some(("proof", "canonical encoding exceeds the 64 MiB limit"))
        );
    }
    #[test]
    fn proof_box_size_accounting_matches_norito_at_compact_prefix_transitions() {
        let _canonical_flags = ncore::DecodeFlagsGuard::enter(ncore::default_encode_flags());
        for backend_len in [1, 126, 127, 128, 255, 256, 4_094] {
            let backend = "a".repeat(backend_len);
            for proof_len in [0, 1, 118, 119, 120, 121, 16_374, 16_375, 16_376, 16_377] {
                let proof = ProofBox::new(backend.clone().into(), vec![0xC3; proof_len]);
                assert_eq!(
                    proof.canonical_encoded_len_v1(),
                    Some(
                        ncore::encoded_payload_len(&proof)
                            .expect("count canonical nested ProofBox payload")
                    ),
                    "backend length {backend_len}, proof length {proof_len}"
                );
            }
        }
        for backend_len in [1, 127, 128, 256, 4_094] {
            let backend = "b".repeat(backend_len);
            let maximum = proof_box_max_proof_bytes_v1(&backend).expect("bounded backend");
            assert_eq!(
                proof_box_canonical_encoded_len_for_lengths_v1(backend_len, maximum),
                Some(PROOF_BOX_MAX_ENCODED_BYTES_V1)
            );
            assert!(
                proof_box_canonical_encoded_len_for_lengths_v1(backend_len, maximum + 1)
                    .is_some_and(|length| length > PROOF_BOX_MAX_ENCODED_BYTES_V1)
            );
        }
        let largest_backend = "c".repeat(4_094);
        let proof = ProofBox::new(largest_backend.clone().into(), vec![0x5A]);
        assert!(proof.canonical_encoded_len_v1().is_some());
        assert!(proof_box_max_proof_bytes_v1(&largest_backend).is_some());
        let encoded = norito::to_bytes(&proof).expect("encode maximum backend field");
        let decoded = norito::decode_from_bytes::<ProofBox>(&encoded)
            .expect("maximum backend field must round-trip");
        assert_eq!(decoded, proof);
        for backend_len in [4_095, 4_096] {
            let backend = "d".repeat(backend_len);
            let proof = ProofBox::new(backend.clone().into(), vec![0x5A]);
            assert_eq!(proof.canonical_encoded_len_v1(), None);
            assert_eq!(proof_box_max_proof_bytes_v1(&backend), None);
            let encoded = norito::to_bytes(&proof).expect("encoding remains infallible");
            assert!(
                norito::decode_from_bytes::<ProofBox>(&encoded).is_err(),
                "raw backend length {backend_len} exceeds the canonical field boundary"
            );
        }
    }
    #[test]
    fn proof_attachment_decode_rejects_backend_mismatches() {
        for (proof_backend, vk_backend, expected_field) in [
            ("stark/fri", "halo2/ipa", "proof.backend"),
            ("halo2/ipa", "stark/fri", "vk_ref.backend"),
        ] {
            let attachment = ProofAttachment::new_ref(
                "halo2/ipa".into(),
                ProofBox::new(proof_backend.into(), vec![1, 2, 3]),
                VerifyingKeyId::new(vk_backend, "vk_1"),
            );
            let encoded = norito::to_bytes(&attachment).expect("encode mismatched attachment");
            let result = norito::decode_from_bytes::<ProofAttachment>(&encoded)
                .expect_err("backend-inconsistent attachment must not decode");
            assert!(
                result.to_string().contains(expected_field),
                "unexpected error: {result}"
            );
        }
    }
    #[cfg(feature = "json")]
    #[test]
    fn proof_attachment_list_json_rejects_backend_mismatch_inside_wire_payload() {
        use base64::Engine as _;
        let list = bounded_attachment_list(vec![ProofAttachment::new_ref(
            "halo2/ipa".into(),
            ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]),
            VerifyingKeyId::new("stark/fri", "vk_1"),
        )]);
        let encoded = norito::to_bytes(&list).expect("encode mismatched attachment list");
        let json = format!("\"{}\"", STANDARD.encode(encoded));
        let err = norito::json::from_str::<ProofAttachmentList>(&json)
            .expect_err("base64 Norito list with backend mismatch must be rejected");
        assert!(err.to_string().contains("vk_ref.backend"));
    }
    #[cfg(feature = "json")]
    #[test]
    fn proof_attachment_list_json_rejects_single_attachment_wire_payload() {
        use base64::Engine as _;
        let attachment = ProofAttachment::new_ref(
            "halo2/ipa".into(),
            ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]),
            VerifyingKeyId::new("halo2/ipa", "vk_1"),
        );
        let encoded = norito::to_bytes(&attachment).expect("encode single attachment");
        let json = format!("\"{}\"", STANDARD.encode(encoded));
        norito::json::from_str::<ProofAttachmentList>(&json)
            .expect_err("single ProofAttachment wire payload must not decode as a list");
    }
    #[cfg(feature = "json")]
    #[test]
    fn proof_attachment_list_json_is_canonical_and_ambient_independent() {
        use base64::Engine as _;
        let list = bounded_attachment_list(vec![ProofAttachment::new_ref(
            "halo2/ipa".into(),
            ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]),
            VerifyingKeyId::new("halo2/ipa", "vk_1"),
        )]);
        let canonical_json =
            norito::json::to_json(&list).expect("encode canonical proof-attachment list JSON");
        let canonical_frame =
            norito::encode_canonical(&list).expect("encode canonical proof-attachment list frame");
        assert_eq!(
            canonical_json,
            format!("\"{}\"", STANDARD.encode(canonical_frame)),
            "streamed base64 must preserve the legacy JSON bytes"
        );
        assert_eq!(
            norito::json::to_json_bounded(&list, canonical_json.len())
                .expect("serialize attachment list at its exact JSON limit"),
            canonical_json
        );
        assert_eq!(
            norito::json::to_json_bounded(&list, canonical_json.len() - 1),
            Err(norito::json::BoundedJsonError::BodyTooLarge)
        );
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        {
            let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            assert_eq!(
                norito::json::to_json(&list)
                    .expect("encode list JSON under alternate ambient layout"),
                canonical_json
            );
        }
        let alternate_frame = {
            let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            norito::to_bytes(&list).expect("encode alternate-layout proof-attachment list")
        };
        let alternate_json = format!("\"{}\"", STANDARD.encode(alternate_frame));
        norito::json::from_str::<ProofAttachmentList>(&alternate_json)
            .expect_err("alternate-layout proof-attachment list JSON must be rejected");
        let value = norito::json::parse_value(&canonical_json)
            .expect("parse canonical list as a borrowed generic value");
        let frame = STANDARD
            .decode(value.as_str().expect("list JSON must be a base64 string"))
            .expect("decode canonical list frame for count preflight test");
        assert_eq!(
            proof_attachment_list_frame_attachment_count(&frame)
                .expect("inspect canonical list count without decoding elements"),
            1
        );
        let from_value =
            <ProofAttachmentList as norito::json::JsonDeserialize>::json_from_value(&value)
                .expect("canonical list Value must pass bounded preflight");
        assert_eq!(from_value, list);
    }
    #[cfg(feature = "json")]
    #[test]
    fn proof_manual_json_writers_preserve_bytes_and_closed_limits() {
        fn assert_bounded<T: norito::json::JsonSerialize>(value: &T) {
            let expected = norito::json::to_json(value).expect("serialize ordinary JSON");
            assert_eq!(
                norito::json::to_json_bounded(value, expected.len())
                    .expect("serialize at exact JSON limit"),
                expected
            );
            assert_eq!(
                norito::json::to_json_bounded(value, expected.len() - 1),
                Err(norito::json::BoundedJsonError::BodyTooLarge)
            );
        }
        let mut attachment = ProofAttachment::new_ref(
            "halo2/ipa".into(),
            ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]),
            VerifyingKeyId::new("halo2/ipa", "vk_1"),
        );
        attachment.vk_commitment = Some([0xBC; 32]);
        assert_bounded(&attachment);
        let id = ProofId {
            backend: "halo2/ipa:profile".into(),
            proof_hash: [0xAB; 32],
        };
        assert_bounded(&id);
        let record = ProofRecord {
            id,
            vk_ref: None,
            vk_commitment: Some([0xCD; 32]),
            status: ProofStatus::Verified,
            verified_at_height: Some(7),
            bridge: None,
        };
        assert_bounded(&record);
        assert_bounded(&crate::query::QueryResponse::Singular(
            crate::query::SingularQueryOutputBox::ProofRecord(record),
        ));
        for status in [
            ProofStatus::Submitted,
            ProofStatus::Verified,
            ProofStatus::Rejected,
        ] {
            assert_bounded(&status);
        }
    }
    #[cfg(feature = "json")]
    #[test]
    fn proof_attachment_list_json_limit_helpers_use_closed_boundaries() {
        use base64::Engine as _;
        proof_attachment_list_validate_limits(8, 2, 8, 2)
            .expect("frame and count exactly at their limits must pass");
        assert!(proof_attachment_list_validate_limits(9, 2, 8, 2).is_err());
        assert!(proof_attachment_list_validate_limits(8, 0, 8, 2).is_err());
        assert!(proof_attachment_list_validate_limits(8, 3, 8, 2).is_err());
        let at_limit = STANDARD.encode([0_u8; 6]);
        assert_eq!(
            proof_attachment_list_base64_decoded_len(&at_limit, 6)
                .expect("decoded bytes exactly at the test limit"),
            6
        );
        let over_limit = STANDARD.encode([0_u8; 7]);
        proof_attachment_list_base64_decoded_len(&over_limit, 6)
            .expect_err("encoded token above the decoded-byte limit must reject");
        let json = format!("\"{at_limit}\"");
        let mut parser = norito::json::Parser::new(&json);
        let (borrowed, decoded_len) = proof_attachment_list_borrowed_base64_token(&mut parser, 6)
            .expect("canonical token exactly at the bounded parser limit");
        assert_eq!(borrowed, at_limit);
        assert_eq!(decoded_len, 6);
        assert_eq!(parser.position(), json.len());
        let full_limit_plus_one = PROOF_ATTACHMENT_LIST_MAX_CANONICAL_FRAME_BYTES_V1 + 1;
        let encoded_len = proof_attachment_list_base64_encoded_len(full_limit_plus_one)
            .expect("full-size base64 arithmetic");
        let full_over_limit = "A".repeat(encoded_len);
        assert_eq!(
            proof_attachment_list_base64_decoded_len(
                &full_over_limit,
                PROOF_ATTACHMENT_LIST_MAX_CANONICAL_FRAME_BYTES_V1,
            )
            .expect_err("canonical full-size token decoding to cap+1 must fail")
            .to_string(),
            proof_attachment_list_json_error("decoded frame exceeds the first-release byte limit")
                .to_string()
        );
        let full_over_limit_json = format!("\"{full_over_limit}\"");
        norito::json::from_str::<ProofAttachmentList>(&full_over_limit_json)
            .expect_err("full-size cap+1 JSON token must fail before base64 allocation");
    }
    #[cfg(feature = "json")]
    #[test]
    fn proof_attachment_list_json_rejects_noncanonical_base64_before_decode() {
        for encoded in [
            "",      // no complete Norito frame
            "AQ",    // missing required padding
            "AQ=",   // impossible encoded length
            "A===",  // excess padding
            "A=AA",  // interior padding
            "AQ-_",  // URL-safe alphabet
            "/x==",  // non-zero low four tail bits
            "AAB=",  // non-zero low two tail bits
            "AQI= ", // embedded whitespace
        ] {
            let json = format!("\"{encoded}\"");
            norito::json::from_str::<ProofAttachmentList>(&json)
                .expect_err("noncanonical base64 must fail bounded preflight");
            let value = norito::json::parse_value(&json).expect("valid generic JSON string");
            <ProofAttachmentList as norito::json::JsonDeserialize>::json_from_value(&value)
                .expect_err("Value preflight must reject the same noncanonical base64");
        }
        norito::json::from_str::<ProofAttachmentList>(r#""\/w==""#)
            .expect_err("escaped base64 spelling must not alias its canonical wire spelling");
    }
    #[cfg(feature = "json")]
    #[test]
    fn proof_attachment_list_json_rejects_over_limit_attachment_count() {
        use base64::Engine as _;
        let attachment = ProofAttachment::new_ref(
            "halo2/ipa".into(),
            ProofBox::new("halo2/ipa".into(), vec![1]),
            VerifyingKeyId::new("halo2/ipa", "vk_1"),
        );
        let list = ProofAttachmentList(vec![
            attachment;
            PROOF_ATTACHMENT_LIST_MAX_ATTACHMENTS_V1 + 1
        ]);
        let frame = norito::encode_canonical(&list).expect("encode over-count test frame");
        assert!(frame.len() < PROOF_ATTACHMENT_LIST_MAX_CANONICAL_FRAME_BYTES_V1);
        assert_eq!(
            proof_attachment_list_frame_attachment_count(&frame)
                .expect("inspect over-limit count before element allocation"),
            PROOF_ATTACHMENT_LIST_MAX_ATTACHMENTS_V1 + 1
        );
        let json = format!("\"{}\"", STANDARD.encode(frame));
        let error = norito::json::from_str::<ProofAttachmentList>(&json)
            .expect_err("canonical frame above the attachment-count limit must reject");
        assert!(error.to_string().contains("attachment count"));
    }
    #[cfg(feature = "json")]
    #[test]
    fn proof_attachment_list_json_rejects_forged_empty_frame() {
        use base64::Engine as _;
        // The private field prevents this value outside the defining module;
        // forge it here solely to exercise hostile wire input.
        let frame = norito::encode_canonical(&ProofAttachmentList(Vec::new()))
            .expect("encode forged empty attachment-list frame");
        let json = format!("\"{}\"", STANDARD.encode(frame));
        let error = norito::json::from_str::<ProofAttachmentList>(&json)
            .expect_err("empty attachment-list frame must reject");
        assert!(error.to_string().contains("must not be empty"));
    }
    #[cfg(feature = "json")]
    #[test]
    fn proof_attachment_json_accepts_reference_only_payload() {
        let json = r#"{
            "backend": "halo2/ipa",
            "proof": { "backend": "halo2/ipa", "bytes": [1, 2, 3] },
            "vk_ref": { "backend": "halo2/ipa", "name": "vk_1" },
            "vk_commitment": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 7]
        }"#;
        let attachment: ProofAttachment = norito::json::from_str(json).expect("reference JSON");
        assert_eq!(attachment.backend.as_str(), "halo2/ipa");
        assert_eq!(attachment.proof.bytes, vec![1, 2, 3]);
        assert_eq!(attachment.vk_ref.name.as_str(), "vk_1");
        assert_eq!(
            attachment.vk_commitment,
            Some({
                let mut commitment = [0u8; 32];
                commitment[31] = 7;
                commitment
            })
        );
        assert!(attachment.envelope_hash.is_none());
    }
    #[cfg(feature = "json")]
    #[test]
    fn proof_attachment_json_uses_canonical_proof_byte_array() {
        let json = r#"{
            "backend": "halo2/ipa",
            "proof": { "backend": "halo2/ipa", "bytes": [1, 2, 3] },
            "vk_ref": { "backend": "halo2/ipa", "name": "vk_1" }
        }"#;
        let attachment: ProofAttachment = norito::json::from_str(json).expect("canonical JSON");
        assert_eq!(attachment.proof.bytes, vec![1, 2, 3]);
        let canonical = norito::json::to_json(&attachment).expect("serialize canonical JSON");
        assert!(canonical.contains("\"bytes\":[1,2,3]"));
        assert!(!canonical.contains("bytes_b64"));
        assert!(!canonical.contains("vk_commitment"));
        assert!(!canonical.contains("envelope_hash"));
        assert!(!canonical.contains("lane_privacy"));
        let roundtrip: ProofAttachment =
            norito::json::from_str(&canonical).expect("canonical roundtrip JSON");
        assert_eq!(roundtrip, attachment);
        let value = norito::json::parse_value(&canonical).expect("canonical generic JSON value");
        let from_value =
            <ProofAttachment as norito::json::JsonDeserialize>::json_from_value(&value)
                .expect("canonical Value must use the streaming acceptance language");
        assert_eq!(from_value, attachment);
    }
    #[cfg(feature = "json")]
    #[test]
    fn proof_attachment_json_streaming_decoder_is_field_order_independent() {
        let json = r#"{
            "vk_ref": { "name": "vk_1", "backend": "halo2/ipa" },
            "proof": { "bytes": [1, 2, 3], "backend": "halo2/ipa" },
            "backend": "halo2/ipa"
        }"#;
        let attachment: ProofAttachment =
            norito::json::from_str(json).expect("reordered canonical attachment JSON");
        assert_eq!(attachment.backend, "halo2/ipa");
        assert_eq!(attachment.proof.bytes, [1, 2, 3]);
        assert_eq!(attachment.vk_ref.name, "vk_1");
    }
    #[cfg(feature = "json")]
    #[test]
    fn proof_attachment_json_proof_bytes_are_bounded_while_streaming() {
        // The production decoder uses the multi-million-byte V1 ceiling. A
        // small const-generic limit exercises the identical boundary without
        // constructing an adversarial 64 MiB fixture in a unit test.
        assert!(PROOF_BOX_MAX_ENCODED_BYTES_V1 > 1_000_000);
        let at_limit = norito::json::from_str::<ProofAttachmentJsonProofBoxV1<4>>(
            r#"{ "backend": "halo2/ipa", "bytes": [0, 1, 2, 3] }"#,
        )
        .expect("stream exactly at the test limit through the production proof decoder");
        assert_eq!(at_limit.backend, "halo2/ipa");
        assert_eq!(at_limit.bytes, [0, 1, 2, 3]);
        let error = norito::json::from_str::<ProofAttachmentJsonProofBoxV1<4>>(
            r#"{ "backend": "halo2/ipa", "bytes": [0, 1, 2, 3, 4] }"#,
        )
        .err()
        .expect("the fifth byte must be rejected before output growth");
        assert!(error.to_string().contains("4-byte streaming limit"));
        let error = norito::json::from_str::<ProofAttachmentJsonProofBoxV1<4>>(
            r#"{ "bytes": [0, 1, 2, 3, 4], "backend": "halo2/ipa" }"#,
        )
        .err()
        .expect("backend discovered after bytes must still bound the byte stream");
        assert!(error.to_string().contains("4-byte streaming limit"));
        let value = norito::json::parse_value(
            r#"{
                "backend": "halo2/ipa",
                "proof": { "bytes": [0, 1, 2, 3, 4], "backend": "halo2/ipa" },
                "vk_ref": { "backend": "halo2/ipa", "name": "vk_1" }
            }"#,
        )
        .expect("generic over-limit proof fixture");
        let error = proof_attachment_json_value_preflight::<4>(&value)
            .expect_err("borrowed Value preflight must reject the fifth byte");
        assert!(error.to_string().contains("proof.bytes"));
    }
    #[cfg(feature = "json")]
    #[test]
    fn proof_attachment_json_lane_path_is_bounded_while_streaming() {
        let sibling = norito::json::to_json(&canonical_lane_sibling(0x23))
            .expect("serialize canonical Merkle sibling");
        let at_limit = format!("[{sibling},{sibling}]");
        let decoded = norito::json::from_str::<ProofAttachmentJsonAuditPathV1<2>>(&at_limit)
            .expect("path exactly at the test limit");
        assert_eq!(decoded.0.len(), 2);
        let over_limit = format!("[{sibling},{sibling},{sibling}]");
        let error = norito::json::from_str::<ProofAttachmentJsonAuditPathV1<2>>(&over_limit)
            .err()
            .expect("third sibling must be rejected before output growth");
        assert!(error.to_string().contains("2-sibling limit"));
    }
    #[cfg(feature = "json")]
    #[test]
    fn proof_attachment_json_rejects_present_null_fields() {
        for json in [
            r#"{
                "backend": null,
                "proof": { "backend": "halo2/ipa", "bytes": [1] },
                "vk_ref": { "backend": "halo2/ipa", "name": "vk_1" }
            }"#,
            r#"{
                "backend": "halo2/ipa",
                "proof": null,
                "vk_ref": { "backend": "halo2/ipa", "name": "vk_1" }
            }"#,
            r#"{
                "backend": "halo2/ipa",
                "proof": { "backend": "halo2/ipa", "bytes": [1] },
                "vk_ref": null
            }"#,
            r#"{
                "backend": "halo2/ipa",
                "proof": { "backend": "halo2/ipa", "bytes": [1] },
                "vk_ref": { "backend": "halo2/ipa", "name": "vk_1" },
                "vk_commitment": null
            }"#,
            r#"{
                "backend": "halo2/ipa",
                "proof": { "backend": "halo2/ipa", "bytes": [1] },
                "vk_ref": { "backend": "halo2/ipa", "name": "vk_1" },
                "envelope_hash": null
            }"#,
            r#"{
                "backend": "halo2/ipa",
                "proof": { "backend": "halo2/ipa", "bytes": [1] },
                "vk_ref": { "backend": "halo2/ipa", "name": "vk_1" },
                "lane_privacy": null
            }"#,
        ] {
            assert!(
                norito::json::from_str::<ProofAttachment>(json).is_err(),
                "present null must not alias an absent first-release field: {json}"
            );
            let value = norito::json::parse_value(json).expect("valid generic JSON fixture");
            assert!(
                <ProofAttachment as norito::json::JsonDeserialize>::json_from_value(&value)
                    .is_err(),
                "json_from_value must enforce the same present-null rule: {json}"
            );
        }
    }
    #[cfg(feature = "json")]
    #[test]
    fn proof_attachment_json_value_preflight_rejects_wrong_shapes() {
        for json in [
            r#"[]"#,
            r#"{
                "backend": 7,
                "proof": { "backend": "halo2/ipa", "bytes": [1] },
                "vk_ref": { "backend": "halo2/ipa", "name": "vk_1" }
            }"#,
            r#"{
                "backend": "halo2/ipa",
                "proof": [],
                "vk_ref": { "backend": "halo2/ipa", "name": "vk_1" }
            }"#,
            r#"{
                "backend": "halo2/ipa",
                "proof": { "backend": "halo2/ipa", "bytes": "AQ==" },
                "vk_ref": { "backend": "halo2/ipa", "name": "vk_1" }
            }"#,
            r#"{
                "backend": "halo2/ipa",
                "proof": { "backend": "halo2/ipa", "bytes": [1] },
                "vk_ref": []
            }"#,
            r#"{
                "backend": "halo2/ipa",
                "proof": { "backend": "halo2/ipa", "bytes": [1] },
                "vk_ref": { "backend": "halo2/ipa", "name": "vk_1" },
                "vk_commitment": "not-a-byte-array"
            }"#,
            r#"{
                "backend": "halo2/ipa",
                "proof": { "backend": "halo2/ipa", "bytes": [1] },
                "vk_ref": { "backend": "halo2/ipa", "name": "vk_1" },
                "lane_privacy": []
            }"#,
        ] {
            let value = norito::json::parse_value(json).expect("valid generic JSON fixture");
            <ProofAttachment as norito::json::JsonDeserialize>::json_from_value(&value)
                .expect_err("borrowed preflight must reject wrong first-release shapes");
        }
    }
    #[cfg(feature = "json")]
    #[test]
    fn proof_attachment_json_rejects_oversized_identifier_fields() {
        let oversized = "a".repeat(VERIFYING_KEY_ID_MAX_FIELD_BYTES + 1);
        for json in [
            format!(
                r#"{{
                    "backend": "{oversized}",
                    "proof": {{ "backend": "{oversized}", "bytes": [1] }},
                    "vk_ref": {{ "backend": "{oversized}", "name": "vk_1" }}
                }}"#
            ),
            format!(
                r#"{{
                    "backend": "halo2/ipa",
                    "proof": {{ "backend": "halo2/ipa", "bytes": [1] }},
                    "vk_ref": {{ "backend": "halo2/ipa", "name": "{oversized}" }}
                }}"#
            ),
        ] {
            let error = norito::json::from_str::<ProofAttachment>(&json)
                .expect_err("oversized attachment identifiers must reject");
            assert!(error.to_string().contains("256-byte limit"));
            let value = norito::json::parse_value(&json).expect("valid generic JSON fixture");
            <ProofAttachment as norito::json::JsonDeserialize>::json_from_value(&value)
                .expect_err("borrowed Value preflight must reject oversized identifiers");
        }
    }
    #[cfg(feature = "json")]
    #[test]
    fn proof_attachment_json_rejects_trailing_commas() {
        for json in [
            r#"{
                "backend": "halo2/ipa",
                "proof": { "backend": "halo2/ipa", "bytes": [1] },
                "vk_ref": { "backend": "halo2/ipa", "name": "vk_1" },
            }"#,
            r#"{
                "backend": "halo2/ipa",
                "proof": { "backend": "halo2/ipa", "bytes": [1], },
                "vk_ref": { "backend": "halo2/ipa", "name": "vk_1" }
            }"#,
            r#"{
                "backend": "halo2/ipa",
                "proof": { "backend": "halo2/ipa", "bytes": [1,] },
                "vk_ref": { "backend": "halo2/ipa", "name": "vk_1" }
            }"#,
            r#"{
                "backend": "halo2/ipa",
                "proof": { "backend": "halo2/ipa", "bytes": [1] },
                "vk_ref": { "backend": "halo2/ipa", "name": "vk_1", }
            }"#,
        ] {
            assert!(
                norito::json::from_str::<ProofAttachment>(json).is_err(),
                "trailing comma must be rejected: {json}"
            );
        }
    }
    #[cfg(feature = "json")]
    #[test]
    fn proof_attachment_json_rejects_noncanonical_proof_byte_encodings() {
        for proof_json in [
            r#"{"backend":"halo2/ipa"}"#,
            r#"{"backend":"halo2/ipa","bytes_b64":"AQID"}"#,
            r#"{"backend":"halo2/ipa","bytes":[1,2,3],"bytes_b64":"AQID"}"#,
            r#"{"backend":"halo2/ipa","bytes":"AQID"}"#,
            r#"{"backend":"halo2/ipa","bytes":[1,256,3]}"#,
        ] {
            let json = format!(
                r#"{{"backend":"halo2/ipa","proof":{proof_json},"vk_ref":{{"backend":"halo2/ipa","name":"vk_1"}}}}"#
            );
            norito::json::from_str::<ProofAttachment>(&json)
                .expect_err("noncanonical proof bytes must fail");
            let value = norito::json::parse_value(&json).expect("valid generic JSON fixture");
            <ProofAttachment as norito::json::JsonDeserialize>::json_from_value(&value)
                .expect_err("Value preflight or strict re-entry must reject proof shape");
        }
    }
    #[cfg(feature = "json")]
    #[test]
    fn proof_attachment_json_accepts_matching_envelope_hash() {
        let proof_bytes = [1u8, 2, 3];
        let envelope_hash = proof_bytes_hash(&proof_bytes);
        let envelope_hash_json = hash_json(&envelope_hash);
        let json = format!(
            r#"{{
                "backend": "halo2/ipa",
                "proof": {{ "backend": "halo2/ipa", "bytes": [1, 2, 3] }},
                "vk_ref": {{ "backend": "halo2/ipa", "name": "vk_1" }},
                "envelope_hash": {envelope_hash_json}
            }}"#
        );
        let attachment: ProofAttachment =
            norito::json::from_str(&json).expect("matching envelope hash JSON");
        assert_eq!(attachment.envelope_hash, Some(envelope_hash));
    }
    #[cfg(feature = "json")]
    #[test]
    fn proof_attachment_json_rejects_retired_inline_vk_fields() {
        for field in [
            "vk_inline",
            "vkInline",
            "verifyingKeyInline",
            "verifying_key_inline",
        ] {
            let json = format!(
                r#"{{
                    "backend": "halo2/ipa",
                    "proof": {{ "backend": "halo2/ipa", "bytes": [1, 2, 3] }},
                    "vk_ref": {{ "backend": "halo2/ipa", "name": "vk_1" }},
                    "{field}": {{ "backend": "halo2/ipa", "bytes": [9, 9, 9] }}
                }}"#
            );
            let err = norito::json::from_str::<ProofAttachment>(&json)
                .expect_err("retired inline verifying key must be rejected");
            assert!(
                err.to_string()
                    .contains("retired inline verifying-key field")
            );
        }
    }
    #[cfg(feature = "json")]
    #[test]
    fn proof_attachment_json_rejects_unknown_members_at_every_declared_layer() {
        for json in [
            r#"{
                "backend": "halo2/ipa",
                "proof": { "backend": "halo2/ipa", "bytes": [1, 2, 3] },
                "vk_ref": { "backend": "halo2/ipa", "name": "vk_1" },
                "future_attachment_metadata": true
            }"#,
            r#"{
                "backend": "halo2/ipa",
                "proof": { "backend": "halo2/ipa", "bytes": [1, 2, 3], "future_proof_metadata": 7 },
                "vk_ref": { "backend": "halo2/ipa", "name": "vk_1" }
            }"#,
            r#"{
                "backend": "halo2/ipa",
                "proof": { "backend": "halo2/ipa", "bytes": [1, 2, 3] },
                "vk_ref": { "backend": "halo2/ipa", "name": "vk_1", "future_registry_metadata": 7 }
            }"#,
        ] {
            let error = norito::json::from_str::<ProofAttachment>(json)
                .expect_err("unknown first-release member must be rejected");
            assert!(error.to_string().contains("unknown fields"));
            let value = norito::json::parse_value(json).expect("valid generic JSON fixture");
            let error = <ProofAttachment as norito::json::JsonDeserialize>::json_from_value(&value)
                .expect_err("borrowed Value preflight must reject every unknown member");
            assert!(error.to_string().contains("unknown"));
        }
    }
    #[cfg(feature = "json")]
    #[test]
    fn proof_attachment_json_requires_exact_structural_lane_privacy() {
        let sibling = canonical_lane_sibling(0x22);
        let mut attachment = ProofAttachment::new_ref(
            "halo2/ipa".into(),
            ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]),
            VerifyingKeyId::new("halo2/ipa", "vk_1"),
        );
        attachment.lane_privacy = Some(lane_privacy_with_path(1, vec![Some(sibling)]));
        let canonical = norito::json::to_json(&attachment).expect("canonical lane attachment JSON");
        let decoded = norito::json::from_str::<ProofAttachment>(&canonical)
            .expect("canonical lane attachment JSON must decode");
        assert_eq!(decoded, attachment);
        let value = norito::json::parse_value(&canonical).expect("canonical lane Value");
        let decoded = <ProofAttachment as norito::json::JsonDeserialize>::json_from_value(&value)
            .expect("canonical lane Value must pass preflight and strict re-entry");
        assert_eq!(decoded, attachment);
        let unknown = canonical.replacen("\"leaf_index\":1", "\"shadow\":0,\"leaf_index\":1", 1);
        let error = norito::json::from_str::<ProofAttachment>(&unknown)
            .expect_err("unknown nested lane field must reject");
        assert!(error.to_string().contains("unknown fields"));
        let value = norito::json::parse_value(&unknown).expect("unknown nested lane Value");
        <ProofAttachment as norito::json::JsonDeserialize>::json_from_value(&value)
            .expect_err("borrowed preflight must reject unknown nested lane fields");
        let duplicate = canonical.replacen(
            "\"commitment_id\":[5]",
            "\"commitment_id\":[5],\"commitment_id\":[5]",
            1,
        );
        let error = norito::json::from_str::<ProofAttachment>(&duplicate)
            .expect_err("duplicate nested lane field must reject");
        assert!(error.to_string().contains("duplicate field"));
        for malformed in [
            lane_privacy_with_path(0, Vec::new()),
            lane_privacy_with_path(0, vec![None]),
            lane_privacy_with_path(2, vec![Some(sibling)]),
            lane_privacy_with_path(
                0,
                vec![Some(sibling); crate::nexus::LANE_PRIVACY_MAX_MERKLE_DEPTH_V1 + 1],
            ),
        ] {
            attachment.lane_privacy = Some(malformed);
            let json = norito::json::to_json(&attachment).expect("malformed lane JSON fixture");
            let error = norito::json::from_str::<ProofAttachment>(&json)
                .expect_err("malformed lane JSON must reject");
            assert!(error.to_string().contains("lane_privacy"));
            let value = norito::json::parse_value(&json).expect("malformed lane generic Value");
            <ProofAttachment as norito::json::JsonDeserialize>::json_from_value(&value)
                .expect_err("Value preflight or strict re-entry must reject malformed lane data");
        }
    }
    #[cfg(feature = "json")]
    #[test]
    fn proof_attachment_json_rejects_duplicate_declared_members() {
        for json in [
            r#"{
                "backend": "halo2/ipa",
                "backend": "halo2/ipa",
                "proof": { "backend": "halo2/ipa", "bytes": [1, 2, 3] },
                "vk_ref": { "backend": "halo2/ipa", "name": "vk_1" }
            }"#,
            r#"{
                "backend": "halo2/ipa",
                "proof": { "backend": "halo2/ipa", "bytes": [1], "bytes": [2] },
                "vk_ref": { "backend": "halo2/ipa", "name": "vk_1" }
            }"#,
            r#"{
                "backend": "halo2/ipa",
                "proof": { "backend": "halo2/ipa", "bytes": [1, 2, 3] },
                "vk_ref": { "backend": "halo2/ipa", "name": "vk_1", "name": "vk_2" }
            }"#,
        ] {
            let err = norito::json::from_str::<ProofAttachment>(json)
                .expect_err("duplicate declared member must fail");
            assert!(err.to_string().contains("duplicate field"));
        }
    }
    #[cfg(feature = "json")]
    #[test]
    fn proof_attachment_json_rejects_malformed_fixed_hashes() {
        for (json, expected) in [
            (
                r#"{
                    "backend": "halo2/ipa",
                    "proof": { "backend": "halo2/ipa", "bytes": [1, 2, 3] },
                    "vk_ref": { "backend": "halo2/ipa", "name": "vk_1" },
                    "vk_commitment": [0, 1, 2]
                }"#,
                "expected 32 bytes",
            ),
            (
                r#"{
                    "backend": "halo2/ipa",
                    "proof": { "backend": "halo2/ipa", "bytes": [1, 2, 3] },
                    "vk_ref": { "backend": "halo2/ipa", "name": "vk_1" },
                    "vk_commitment": [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1]
                }"#,
                "exactly 32 bytes",
            ),
        ] {
            let err = norito::json::from_str::<ProofAttachment>(json)
                .expect_err("malformed vk_commitment must be rejected");
            assert!(
                err.to_string().contains(expected),
                "unexpected error: {err}"
            );
        }
    }
    #[cfg(feature = "json")]
    #[test]
    fn proof_attachment_json_rejects_invalid_fixed_hash_byte() {
        let json = r#"{
            "backend": "halo2/ipa",
            "proof": { "backend": "halo2/ipa", "bytes": [1, 2, 3] },
            "vk_ref": { "backend": "halo2/ipa", "name": "vk_1" },
            "envelope_hash": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 300]
        }"#;
        let err = norito::json::from_str::<ProofAttachment>(json)
            .expect_err("out-of-range envelope_hash byte must be rejected");
        assert!(err.to_string().contains("not a valid u8"));
    }
    #[cfg(feature = "json")]
    #[test]
    fn proof_attachment_json_rejects_backend_mismatches() {
        let proof_backend_json = r#"{
            "backend": "halo2/ipa",
            "proof": { "backend": "stark/fri", "bytes": [1, 2, 3] },
            "vk_ref": { "backend": "halo2/ipa", "name": "vk_1" }
        }"#;
        let err = norito::json::from_str::<ProofAttachment>(proof_backend_json)
            .expect_err("proof backend mismatch must be rejected");
        assert!(err.to_string().contains("proof.backend"));
        let vk_backend_json = r#"{
            "backend": "halo2/ipa",
            "proof": { "backend": "halo2/ipa", "bytes": [1, 2, 3] },
            "vk_ref": { "backend": "stark/fri", "name": "vk_1" }
        }"#;
        let err = norito::json::from_str::<ProofAttachment>(vk_backend_json)
            .expect_err("vk_ref backend mismatch must be rejected");
        assert!(err.to_string().contains("vk_ref.backend"));
    }
    #[cfg(feature = "json")]
    #[test]
    fn proof_attachment_json_rejects_nested_retired_inline_vk_fields() {
        let proof_shadow_json = r#"{
            "backend": "halo2/ipa",
            "proof": { "backend": "halo2/ipa", "bytes": [1, 2, 3], "vk_inline": { "backend": "halo2/ipa", "bytes": [9] } },
            "vk_ref": { "backend": "halo2/ipa", "name": "vk_1" }
        }"#;
        let err = norito::json::from_str::<ProofAttachment>(proof_shadow_json)
            .expect_err("retired proof inline key must be rejected");
        assert!(err.to_string().contains("proof.vk_inline"));
        let vk_ref_shadow_json = r#"{
            "backend": "halo2/ipa",
            "proof": { "backend": "halo2/ipa", "bytes": [1, 2, 3] },
            "vk_ref": { "backend": "halo2/ipa", "name": "vk_1", "verifying_key_inline": "shadow" }
        }"#;
        let err = norito::json::from_str::<ProofAttachment>(vk_ref_shadow_json)
            .expect_err("retired vk_ref inline key must be rejected");
        assert!(err.to_string().contains("vk_ref.verifying_key_inline"));
    }
    #[cfg(feature = "json")]
    #[test]
    fn proof_attachment_json_rejects_blank_verifying_key_name() {
        let json = r#"{
            "backend": "halo2/ipa",
            "proof": { "backend": "halo2/ipa", "bytes": [1, 2, 3] },
            "vk_ref": { "backend": "halo2/ipa", "name": "   " }
        }"#;
        let err = norito::json::from_str::<ProofAttachment>(json)
            .expect_err("blank verifying key names must be rejected");
        assert!(err.to_string().contains("vk_ref.name"));
    }
    #[cfg(feature = "json")]
    #[test]
    fn proof_attachment_json_rejects_blank_backend_fields() {
        let cases = [
            (
                r#"{
                    "backend": "   ",
                    "proof": { "backend": "   ", "bytes": [1, 2, 3] },
                    "vk_ref": { "backend": "   ", "name": "vk_1" }
                }"#,
                "backend",
            ),
            (
                r#"{
                    "backend": "halo2/ipa",
                    "proof": { "backend": "   ", "bytes": [1, 2, 3] },
                    "vk_ref": { "backend": "halo2/ipa", "name": "vk_1" }
                }"#,
                "proof.backend",
            ),
            (
                r#"{
                    "backend": "halo2/ipa",
                    "proof": { "backend": "halo2/ipa", "bytes": [1, 2, 3] },
                    "vk_ref": { "backend": "   ", "name": "vk_1" }
                }"#,
                "vk_ref.backend",
            ),
        ];
        for (json, expected_field) in cases {
            let err = norito::json::from_str::<ProofAttachment>(json)
                .expect_err("blank backend fields must be rejected");
            assert!(
                err.to_string().contains(expected_field),
                "expected error to mention {expected_field}, got {err}"
            );
        }
    }
    #[cfg(feature = "json")]
    #[test]
    fn proof_attachment_json_rejects_nonportable_refs_empty_proofs_and_zero_hashes() {
        let zero_hash = "[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]";
        let mut forged_hash = proof_bytes_hash(&[1, 2, 3]);
        forged_hash[0] ^= 0x80;
        let forged_hash = hash_json(&forged_hash);
        let cases = [
            (
                r#"{
                    "backend": "Halo2/ipa",
                    "proof": { "backend": "Halo2/ipa", "bytes": [1, 2, 3] },
                    "vk_ref": { "backend": "Halo2/ipa", "name": "vk_1" }
                }"#
                .to_owned(),
                "vk_ref",
            ),
            (
                r#"{
                    "backend": "halo2/ipa",
                    "proof": { "backend": "halo2/ipa", "bytes": [1, 2, 3] },
                    "vk_ref": { "backend": "halo2/ipa", "name": "Vk_1" }
                }"#
                .to_owned(),
                "vk_ref",
            ),
            (
                r#"{
                    "backend": "halo2/ipa",
                    "proof": { "backend": "halo2/ipa", "bytes": [] },
                    "vk_ref": { "backend": "halo2/ipa", "name": "vk_1" }
                }"#
                .to_owned(),
                "proof.bytes",
            ),
            (
                format!(
                    r#"{{
                        "backend": "halo2/ipa",
                        "proof": {{ "backend": "halo2/ipa", "bytes": [1, 2, 3] }},
                        "vk_ref": {{ "backend": "halo2/ipa", "name": "vk_1" }},
                        "vk_commitment": {zero_hash}
                    }}"#
                ),
                "vk_commitment",
            ),
            (
                format!(
                    r#"{{
                        "backend": "halo2/ipa",
                        "proof": {{ "backend": "halo2/ipa", "bytes": [1, 2, 3] }},
                        "vk_ref": {{ "backend": "halo2/ipa", "name": "vk_1" }},
                        "envelope_hash": {zero_hash}
                    }}"#
                ),
                "envelope_hash",
            ),
            (
                format!(
                    r#"{{
                        "backend": "halo2/ipa",
                        "proof": {{ "backend": "halo2/ipa", "bytes": [1, 2, 3] }},
                        "vk_ref": {{ "backend": "halo2/ipa", "name": "vk_1" }},
                        "envelope_hash": {forged_hash}
                    }}"#
                ),
                "envelope_hash",
            ),
        ];
        for (json, expected_field) in cases {
            let err = norito::json::from_str::<ProofAttachment>(&json)
                .expect_err("malformed proof attachment JSON must be rejected");
            assert!(
                err.to_string().contains(expected_field),
                "expected JSON error to mention {expected_field}, got {err}"
            );
        }
    }
    #[test]
    fn proofed_committed_tx_roundtrip() {
        use crate::query::CommittedTransaction;
        use iroha_crypto::{Hash, HashOf};
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
        let authority = crate::account::AccountId::parse_encoded(
            "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE",
        )
        .expect("valid account id")
        .into_account_id();
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
            merge_inclusion: None,
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
    #[test]
    fn take_len_prefixed_slice_rejects_fields_beyond_cap() {
        let mut encoded = Vec::new();
        ncore::write_len_header_to_vec(&mut encoded, (MAX_BACKEND_FIELD_BYTES as u64) + 1);
        let mut offset = 0usize;
        let result = take_len_prefixed_slice(&encoded, &mut offset, MAX_BACKEND_FIELD_BYTES);
        assert!(matches!(result, Err(ncore::Error::LengthMismatch)));
    }
    #[test]
    fn proofbox_decode_rejects_oversized_len_prefixed_payloads() {
        let backend: iroha_schema::Ident = "halo2/ipa".into();
        let backend_bytes = norito::to_bytes(&backend).expect("encode backend");
        let mut encoded = Vec::new();
        ncore::write_len_header_to_vec(&mut encoded, backend_bytes.len() as u64);
        encoded.extend_from_slice(&backend_bytes);
        ncore::write_len_header_to_vec(&mut encoded, (MAX_LEN_PREFIXED_FIELD_BYTES as u64) + 1);
        let result = <ProofBox as ncore::DecodeFromSlice>::decode_from_slice(&encoded);
        assert!(matches!(result, Err(ncore::Error::LengthMismatch)));
    }
    #[test]
    fn verifying_key_box_decode_rejects_oversized_outer_field_before_decode() {
        let backend: iroha_schema::Ident = "halo2/ipa".into();
        let backend_bytes = norito::to_bytes(&backend).expect("encode backend");
        let mut encoded = Vec::new();
        ncore::write_len_header_to_vec(&mut encoded, backend_bytes.len() as u64);
        encoded.extend_from_slice(&backend_bytes);
        ncore::write_len_header_to_vec(
            &mut encoded,
            (VERIFYING_KEY_BOX_MAX_FIELD_BYTES_V1 as u64) + 1,
        );
        let result = <VerifyingKeyBox as ncore::DecodeFromSlice>::decode_from_slice(&encoded);
        assert!(matches!(result, Err(ncore::Error::LengthMismatch)));
    }
    #[test]
    fn verifying_key_box_decode_rejects_oversized_declared_vector_before_allocation() {
        let backend: iroha_schema::Ident = "halo2/ipa".into();
        let backend_bytes = norito::to_bytes(&backend).expect("encode backend");
        let mut vk_field = Vec::new();
        ncore::write_seq_len(
            &mut vk_field,
            (VERIFYING_KEY_BOX_MAX_PAYLOAD_BYTES_V1 as u64) + 1,
        )
        .expect("encode declared verifier-key length");
        let mut encoded = Vec::new();
        ncore::write_len_header_to_vec(&mut encoded, backend_bytes.len() as u64);
        encoded.extend_from_slice(&backend_bytes);
        ncore::write_len_header_to_vec(&mut encoded, vk_field.len() as u64);
        encoded.extend_from_slice(&vk_field);
        let result = <VerifyingKeyBox as ncore::DecodeFromSlice>::decode_from_slice(&encoded);
        assert!(matches!(result, Err(ncore::Error::LengthMismatch)));
    }
}
