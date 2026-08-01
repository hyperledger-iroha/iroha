//! Strict, bounded first-release JSON decoding for proof attachments.

use base64::{Engine as _, engine::general_purpose as b64gp};
use iroha_crypto::privacy::LaneCommitmentId;
use iroha_data_model::{
    nexus::{LANE_PRIVACY_MAX_MERKLE_DEPTH_V1, LanePrivacyProof},
    proof::{
        PROOF_BOX_MAX_ENCODED_BYTES_V1, ProofAttachment, ProofBox,
        VERIFYING_KEY_ID_MAX_FIELD_BYTES, VerifyingKeyId, proof_box_max_proof_bytes_v1,
        verifying_key_id_field_is_portable,
    },
};
use libc::{c_char, c_ulong};
use norito::json::{
    Error as JsonError, JsonDeserialize, MapVisitor as JsonMapVisitor, Parser as JsonParser,
    SeqVisitor as JsonSeqVisitor,
};

use super::{BridgeError, BridgeResult};

pub(super) const PROOF_ATTACHMENT_JSON_MAX_BASE64_BYTES_V1: usize =
    PROOF_BOX_MAX_ENCODED_BYTES_V1.div_ceil(3) * 4;
// A maximum-depth lane witness needs less than 64 KiB in this JSON shape. Keep
// the structural allowance explicit and generous enough for field names,
// portable identifiers, fixed hashes, punctuation, and ordinary whitespace.
const PROOF_ATTACHMENT_JSON_STRUCTURAL_ALLOWANCE_BYTES_V1: usize = 128 * 1024;
pub(super) const PROOF_ATTACHMENT_JSON_MAX_BYTES_V1: usize =
    PROOF_ATTACHMENT_JSON_MAX_BASE64_BYTES_V1 + PROOF_ATTACHMENT_JSON_STRUCTURAL_ALLOWANCE_BYTES_V1;
const PROOF_ATTACHMENT_JSON_HASH_HEX_BYTES_V1: usize = 64;
const PROOF_ATTACHMENT_JSON_LANE_KIND_BYTES_V1: usize = 16;

/// A JSON string whose token and decoded value are checked before retaining
/// owned storage. This prevents small semantic fields from consuming the
/// entire proof-ingress allowance before their schema limit is enforced.
struct ProofAttachmentJsonBoundedStringV1<const MAX: usize>(String);

impl<const MAX: usize> JsonDeserialize for ProofAttachmentJsonBoundedStringV1<MAX> {
    fn json_deserialize(parser: &mut JsonParser<'_>) -> Result<Self, JsonError> {
        let mut probe = *parser;
        probe.skip_string_bounded(MAX)?;

        let value = String::json_deserialize(parser)?;
        if value.len() > MAX {
            return Err(JsonError::Message(format!(
                "proof attachment string exceeds the {MAX}-byte decoded limit"
            )));
        }
        Ok(Self(value))
    }
}

fn decode_exact_lower_hex_array<const N: usize>(hex_str: &str) -> BridgeResult<[u8; N]> {
    if hex_str.len() != N * 2
        || !hex_str.bytes().all(|byte| byte.is_ascii_hexdigit())
        || hex_str.bytes().any(|byte| byte.is_ascii_uppercase())
    {
        return Err(BridgeError::ProofAttachment);
    }
    let bytes = hex::decode(hex_str).map_err(|_| BridgeError::ProofAttachment)?;
    if bytes.len() != N {
        return Err(BridgeError::ProofAttachment);
    }
    let mut out = [0u8; N];
    out.copy_from_slice(&bytes);
    Ok(out)
}

pub(super) fn decode_canonical_bounded_base64(
    value: &str,
    max_decoded: usize,
) -> BridgeResult<Vec<u8>> {
    let max_encoded = max_decoded
        .checked_add(2)
        .and_then(|value| value.checked_div(3))
        .and_then(|value| value.checked_mul(4))
        .ok_or(BridgeError::ProofAttachment)?;
    if value.len() > max_encoded {
        return Err(BridgeError::ProofAttachment);
    }
    if !is_canonical_standard_base64(value.as_bytes()) {
        return Err(BridgeError::ProofAttachment);
    }
    let bytes = b64gp::STANDARD
        .decode(value.as_bytes())
        .map_err(|_| BridgeError::ProofAttachment)?;
    if bytes.len() > max_decoded {
        return Err(BridgeError::ProofAttachment);
    }
    Ok(bytes)
}

fn standard_base64_sextet(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

fn is_canonical_standard_base64(value: &[u8]) -> bool {
    if !value.len().is_multiple_of(4) {
        return false;
    }
    if value.is_empty() {
        return true;
    }

    let padding = if value.ends_with(b"==") {
        2
    } else if value.ends_with(b"=") {
        1
    } else {
        0
    };
    let payload_len = value.len() - padding;
    if value[..payload_len]
        .iter()
        .any(|byte| standard_base64_sextet(*byte).is_none())
        || value[payload_len..].iter().any(|byte| *byte != b'=')
    {
        return false;
    }

    match padding {
        0 => true,
        1 => {
            payload_len % 4 == 3
                && standard_base64_sextet(value[payload_len - 1])
                    .is_some_and(|sextet| sextet & 0b11 == 0)
        }
        2 => {
            payload_len % 4 == 2
                && standard_base64_sextet(value[payload_len - 1])
                    .is_some_and(|sextet| sextet & 0b1111 == 0)
        }
        _ => false,
    }
}

struct ProofAttachmentJsonBytes32V1([u8; 32]);

impl JsonDeserialize for ProofAttachmentJsonBytes32V1 {
    fn json_deserialize(parser: &mut JsonParser<'_>) -> Result<Self, JsonError> {
        let mut sequence = JsonSeqVisitor::new(parser)?;
        let mut bytes = [0_u8; 32];
        for byte in &mut bytes {
            *byte = sequence.next_element::<u8>()?.ok_or_else(|| {
                JsonError::Message("proof attachment byte array must contain 32 bytes".into())
            })?;
        }
        if sequence.next_element::<u8>()?.is_some() {
            return Err(JsonError::Message(
                "proof attachment byte array must contain exactly 32 bytes".into(),
            ));
        }
        sequence.finish()?;
        Ok(Self(bytes))
    }
}

struct ProofAttachmentJsonAuditPathV1(Vec<[u8; 32]>);

impl JsonDeserialize for ProofAttachmentJsonAuditPathV1 {
    fn json_deserialize(parser: &mut JsonParser<'_>) -> Result<Self, JsonError> {
        let mut sequence = JsonSeqVisitor::new(parser)?;
        let mut path = Vec::with_capacity(LANE_PRIVACY_MAX_MERKLE_DEPTH_V1);
        while !sequence.is_finished() {
            if path.len() == LANE_PRIVACY_MAX_MERKLE_DEPTH_V1 {
                return Err(JsonError::Message(
                    "proof attachment audit path exceeds the V1 depth limit".into(),
                ));
            }
            let sibling = sequence
                .next_element::<ProofAttachmentJsonBytes32V1>()?
                .ok_or_else(|| JsonError::Message("expected audit-path sibling".into()))?;
            path.push(sibling.0);
        }
        sequence.finish()?;
        if path.is_empty() {
            return Err(JsonError::Message(
                "proof attachment audit path must not be empty".into(),
            ));
        }
        Ok(Self(path))
    }
}

struct ProofAttachmentJsonVerifyingKeyRefV1 {
    backend: String,
    name: String,
}

fn mark_proof_attachment_json_field(
    seen: &mut u8,
    field_bit: u8,
    field: &str,
) -> Result<(), JsonError> {
    if *seen & field_bit != 0 {
        return Err(JsonError::duplicate_field(field));
    }
    *seen |= field_bit;
    Ok(())
}

impl JsonDeserialize for ProofAttachmentJsonVerifyingKeyRefV1 {
    fn json_deserialize(parser: &mut JsonParser<'_>) -> Result<Self, JsonError> {
        const BACKEND: u8 = 1 << 0;
        const NAME: u8 = 1 << 1;

        let mut object = JsonMapVisitor::new(parser)?;
        let mut seen = 0_u8;
        let mut backend = None;
        let mut name = None;
        while let Some(field) = object.next_key()? {
            match field.as_str() {
                "backend" => {
                    mark_proof_attachment_json_field(&mut seen, BACKEND, "backend")?;
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
                    mark_proof_attachment_json_field(&mut seen, NAME, "name")?;
                    name =
                        Some(
                            object
                                .parse_value::<ProofAttachmentJsonBoundedStringV1<
                                    VERIFYING_KEY_ID_MAX_FIELD_BYTES,
                                >>()?
                                .0,
                        );
                }
                field => return Err(JsonError::unknown_field(field)),
            }
        }
        object.finish()?;
        Ok(Self {
            backend: backend.ok_or_else(|| JsonError::missing_field("backend"))?,
            name: name.ok_or_else(|| JsonError::missing_field("name"))?,
        })
    }
}

struct ProofAttachmentJsonLaneMerkleProofV1 {
    leaf_index: u32,
    audit_path: ProofAttachmentJsonAuditPathV1,
}

impl JsonDeserialize for ProofAttachmentJsonLaneMerkleProofV1 {
    fn json_deserialize(parser: &mut JsonParser<'_>) -> Result<Self, JsonError> {
        const LEAF_INDEX: u8 = 1 << 0;
        const AUDIT_PATH: u8 = 1 << 1;

        let mut object = JsonMapVisitor::new(parser)?;
        let mut seen = 0_u8;
        let mut leaf_index = None;
        let mut audit_path = None;
        while let Some(field) = object.next_key()? {
            match field.as_str() {
                "leaf_index" => {
                    mark_proof_attachment_json_field(&mut seen, LEAF_INDEX, "leaf_index")?;
                    leaf_index = Some(object.parse_value::<u32>()?);
                }
                "audit_path" => {
                    mark_proof_attachment_json_field(&mut seen, AUDIT_PATH, "audit_path")?;
                    audit_path = Some(object.parse_value::<ProofAttachmentJsonAuditPathV1>()?);
                }
                field => return Err(JsonError::unknown_field(field)),
            }
        }
        object.finish()?;
        Ok(Self {
            leaf_index: leaf_index.ok_or_else(|| JsonError::missing_field("leaf_index"))?,
            audit_path: audit_path.ok_or_else(|| JsonError::missing_field("audit_path"))?,
        })
    }
}

struct ProofAttachmentJsonLaneMerklePayloadV1 {
    leaf: ProofAttachmentJsonBytes32V1,
    proof: ProofAttachmentJsonLaneMerkleProofV1,
}

impl JsonDeserialize for ProofAttachmentJsonLaneMerklePayloadV1 {
    fn json_deserialize(parser: &mut JsonParser<'_>) -> Result<Self, JsonError> {
        const LEAF: u8 = 1 << 0;
        const PROOF: u8 = 1 << 1;

        let mut object = JsonMapVisitor::new(parser)?;
        let mut seen = 0_u8;
        let mut leaf = None;
        let mut proof = None;
        while let Some(field) = object.next_key()? {
            match field.as_str() {
                "leaf" => {
                    mark_proof_attachment_json_field(&mut seen, LEAF, "leaf")?;
                    leaf = Some(object.parse_value::<ProofAttachmentJsonBytes32V1>()?);
                }
                "proof" => {
                    mark_proof_attachment_json_field(&mut seen, PROOF, "proof")?;
                    proof = Some(object.parse_value::<ProofAttachmentJsonLaneMerkleProofV1>()?);
                }
                field => return Err(JsonError::unknown_field(field)),
            }
        }
        object.finish()?;
        Ok(Self {
            leaf: leaf.ok_or_else(|| JsonError::missing_field("leaf"))?,
            proof: proof.ok_or_else(|| JsonError::missing_field("proof"))?,
        })
    }
}

struct ProofAttachmentJsonLaneWitnessV1 {
    kind: String,
    payload: ProofAttachmentJsonLaneMerklePayloadV1,
}

impl JsonDeserialize for ProofAttachmentJsonLaneWitnessV1 {
    fn json_deserialize(parser: &mut JsonParser<'_>) -> Result<Self, JsonError> {
        const KIND: u8 = 1 << 0;
        const PAYLOAD: u8 = 1 << 1;

        let mut object = JsonMapVisitor::new(parser)?;
        let mut seen = 0_u8;
        let mut kind = None;
        let mut payload = None;
        while let Some(field) = object.next_key()? {
            match field.as_str() {
                "kind" => {
                    mark_proof_attachment_json_field(&mut seen, KIND, "kind")?;
                    kind = Some(
                        object
                            .parse_value::<ProofAttachmentJsonBoundedStringV1<
                                PROOF_ATTACHMENT_JSON_LANE_KIND_BYTES_V1,
                            >>()?
                            .0,
                    );
                }
                "payload" => {
                    mark_proof_attachment_json_field(&mut seen, PAYLOAD, "payload")?;
                    payload = Some(object.parse_value::<ProofAttachmentJsonLaneMerklePayloadV1>()?);
                }
                field => return Err(JsonError::unknown_field(field)),
            }
        }
        object.finish()?;
        Ok(Self {
            kind: kind.ok_or_else(|| JsonError::missing_field("kind"))?,
            payload: payload.ok_or_else(|| JsonError::missing_field("payload"))?,
        })
    }
}

struct ProofAttachmentJsonLanePrivacyV1 {
    commitment_id: u16,
    witness: ProofAttachmentJsonLaneWitnessV1,
}

impl JsonDeserialize for ProofAttachmentJsonLanePrivacyV1 {
    fn json_deserialize(parser: &mut JsonParser<'_>) -> Result<Self, JsonError> {
        const COMMITMENT_ID: u8 = 1 << 0;
        const WITNESS: u8 = 1 << 1;

        let mut object = JsonMapVisitor::new(parser)?;
        let mut seen = 0_u8;
        let mut commitment_id = None;
        let mut witness = None;
        while let Some(field) = object.next_key()? {
            match field.as_str() {
                "commitment_id" => {
                    mark_proof_attachment_json_field(&mut seen, COMMITMENT_ID, "commitment_id")?;
                    commitment_id = Some(object.parse_value::<u16>()?);
                }
                "witness" => {
                    mark_proof_attachment_json_field(&mut seen, WITNESS, "witness")?;
                    witness = Some(object.parse_value::<ProofAttachmentJsonLaneWitnessV1>()?);
                }
                field => return Err(JsonError::unknown_field(field)),
            }
        }
        object.finish()?;
        Ok(Self {
            commitment_id: commitment_id
                .ok_or_else(|| JsonError::missing_field("commitment_id"))?,
            witness: witness.ok_or_else(|| JsonError::missing_field("witness"))?,
        })
    }
}

impl ProofAttachmentJsonLanePrivacyV1 {
    fn into_lane_privacy_proof(self) -> BridgeResult<LanePrivacyProof> {
        if self.witness.kind != "merkle" {
            return Err(BridgeError::ProofAttachment);
        }
        let ProofAttachmentJsonLaneMerklePayloadV1 { leaf, proof } = self.witness.payload;
        let ProofAttachmentJsonLaneMerkleProofV1 {
            leaf_index,
            audit_path,
        } = proof;
        if audit_path.0.len() < u32::BITS as usize
            && u64::from(leaf_index) >= 1_u64 << audit_path.0.len()
        {
            return Err(BridgeError::ProofAttachment);
        }
        LanePrivacyProof::merkle_from_raw_path(
            LaneCommitmentId::new(self.commitment_id),
            leaf.0,
            leaf_index,
            audit_path.0.into_iter().map(Some).collect(),
        )
        .map_err(|_| BridgeError::ProofAttachment)
    }
}

struct ProofAttachmentJsonV1 {
    backend: String,
    proof_b64: String,
    vk_ref: ProofAttachmentJsonVerifyingKeyRefV1,
    vk_commitment_hex: Option<String>,
    envelope_hash_hex: String,
    lane_privacy: Option<ProofAttachmentJsonLanePrivacyV1>,
}

impl JsonDeserialize for ProofAttachmentJsonV1 {
    fn json_deserialize(parser: &mut JsonParser<'_>) -> Result<Self, JsonError> {
        const BACKEND: u8 = 1 << 0;
        const PROOF_B64: u8 = 1 << 1;
        const VK_REF: u8 = 1 << 2;
        const VK_COMMITMENT_HEX: u8 = 1 << 3;
        const ENVELOPE_HASH_HEX: u8 = 1 << 4;
        const LANE_PRIVACY: u8 = 1 << 5;

        let mut object = JsonMapVisitor::new(parser)?;
        let mut seen = 0_u8;
        let mut backend = None;
        let mut proof_b64 = None;
        let mut vk_ref = None;
        let mut vk_commitment_hex = None;
        let mut envelope_hash_hex = None;
        let mut lane_privacy = None;
        while let Some(field) = object.next_key()? {
            match field.as_str() {
                "backend" => {
                    mark_proof_attachment_json_field(&mut seen, BACKEND, "backend")?;
                    backend =
                        Some(
                            object
                                .parse_value::<ProofAttachmentJsonBoundedStringV1<
                                    VERIFYING_KEY_ID_MAX_FIELD_BYTES,
                                >>()?
                                .0,
                        );
                }
                "proof_b64" => {
                    mark_proof_attachment_json_field(&mut seen, PROOF_B64, "proof_b64")?;
                    proof_b64 = Some(
                        object
                            .parse_value::<ProofAttachmentJsonBoundedStringV1<
                                PROOF_ATTACHMENT_JSON_MAX_BASE64_BYTES_V1,
                            >>()?
                            .0,
                    );
                }
                "vk_ref" => {
                    mark_proof_attachment_json_field(&mut seen, VK_REF, "vk_ref")?;
                    vk_ref = Some(object.parse_value::<ProofAttachmentJsonVerifyingKeyRefV1>()?);
                }
                "vk_commitment_hex" => {
                    mark_proof_attachment_json_field(
                        &mut seen,
                        VK_COMMITMENT_HEX,
                        "vk_commitment_hex",
                    )?;
                    // Parse the string directly so a present null/object/array fails
                    // before a generic JSON value can be materialized.
                    vk_commitment_hex = Some(
                        object
                            .parse_value::<ProofAttachmentJsonBoundedStringV1<
                                PROOF_ATTACHMENT_JSON_HASH_HEX_BYTES_V1,
                            >>()?
                            .0,
                    );
                }
                "envelope_hash_hex" => {
                    mark_proof_attachment_json_field(
                        &mut seen,
                        ENVELOPE_HASH_HEX,
                        "envelope_hash_hex",
                    )?;
                    envelope_hash_hex = Some(
                        object
                            .parse_value::<ProofAttachmentJsonBoundedStringV1<
                                PROOF_ATTACHMENT_JSON_HASH_HEX_BYTES_V1,
                            >>()?
                            .0,
                    );
                }
                "lane_privacy" => {
                    mark_proof_attachment_json_field(&mut seen, LANE_PRIVACY, "lane_privacy")?;
                    // Parse the object directly; present null is not equivalent to
                    // the field being absent in the first-release schema.
                    lane_privacy = Some(object.parse_value::<ProofAttachmentJsonLanePrivacyV1>()?);
                }
                field => return Err(JsonError::unknown_field(field)),
            }
        }
        object.finish()?;
        Ok(Self {
            backend: backend.ok_or_else(|| JsonError::missing_field("backend"))?,
            proof_b64: proof_b64.ok_or_else(|| JsonError::missing_field("proof_b64"))?,
            vk_ref: vk_ref.ok_or_else(|| JsonError::missing_field("vk_ref"))?,
            vk_commitment_hex,
            envelope_hash_hex: envelope_hash_hex
                .ok_or_else(|| JsonError::missing_field("envelope_hash_hex"))?,
            lane_privacy,
        })
    }
}

fn parse_proof_attachment_json_v1(value: ProofAttachmentJsonV1) -> BridgeResult<ProofAttachment> {
    if !verifying_key_id_field_is_portable(&value.backend) {
        return Err(BridgeError::ProofAttachment);
    }
    let maximum_proof_bytes =
        proof_box_max_proof_bytes_v1(&value.backend).ok_or(BridgeError::ProofAttachment)?;
    let proof_bytes = decode_canonical_bounded_base64(&value.proof_b64, maximum_proof_bytes)?;

    if !verifying_key_id_field_is_portable(&value.vk_ref.backend)
        || value.vk_ref.backend != value.backend
        || !verifying_key_id_field_is_portable(&value.vk_ref.name)
    {
        return Err(BridgeError::ProofAttachment);
    }
    let proof = ProofBox::new(value.backend.clone(), proof_bytes);
    let vk_ref = VerifyingKeyId::new(value.vk_ref.backend, value.vk_ref.name);
    let mut attachment = ProofAttachment::new_ref(value.backend, proof, vk_ref);
    attachment.vk_commitment = value
        .vk_commitment_hex
        .map(|commitment| decode_exact_lower_hex_array(&commitment))
        .transpose()?;
    attachment.envelope_hash = Some(decode_exact_lower_hex_array(&value.envelope_hash_hex)?);
    attachment.lane_privacy = value
        .lane_privacy
        .map(ProofAttachmentJsonLanePrivacyV1::into_lane_privacy_proof)
        .transpose()?;
    if attachment.structural_error().is_some() {
        return Err(BridgeError::ProofAttachment);
    }
    Ok(attachment)
}

pub(super) fn parse_proof_attachment_from_json_bytes(
    ptr: *const c_char,
    len: c_ulong,
) -> BridgeResult<ProofAttachment> {
    let length = usize::try_from(len).map_err(|_| BridgeError::ProofAttachment)?;
    if ptr.is_null() || !proof_attachment_json_length_is_valid(length) {
        return Err(BridgeError::ProofAttachment);
    }
    // SAFETY: the caller's bridge contract provides `length` readable bytes;
    // null and the global input ceiling were checked before constructing it.
    let slice = unsafe { std::slice::from_raw_parts(ptr.cast::<u8>(), length) };
    parse_proof_attachment_from_json_slice(slice)
}

pub(super) const fn proof_attachment_json_length_is_valid(length: usize) -> bool {
    length > 0 && length <= PROOF_ATTACHMENT_JSON_MAX_BYTES_V1
}

pub(super) fn parse_proof_attachment_from_json_slice(
    slice: &[u8],
) -> BridgeResult<ProofAttachment> {
    if !proof_attachment_json_length_is_valid(slice.len()) {
        return Err(BridgeError::ProofAttachment);
    }
    let value = norito::json::from_slice::<ProofAttachmentJsonV1>(slice)
        .map_err(|_| BridgeError::ProofAttachment)?;
    parse_proof_attachment_json_v1(value)
}
