//! Validation helpers for manifests destined for the Pin Registry.
#![allow(unexpected_cfgs)]
use std::collections::BTreeSet;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use crate::{
    ChunkingProfileV1, EMPTY_POR_ROOT_V1, GovernanceProofs, MANIFEST_VERSION_V1, ManifestV1,
    PinPolicy, ProfileId, StorageClass, chunker_registry,
};
/// Maximum canonical Norito manifest size admitted by first-release nodes.
pub const MAX_MANIFEST_ENCODED_BYTES: usize = 512 * 1024;
/// Exact binary length of the canonical first-release manifest root CID.
///
/// The layout is CIDv1 + dag-cbor + BLAKE3-256 multihash, whose four
/// single-byte canonical varints precede a 32-byte digest.
pub const MAX_MANIFEST_ROOT_CID_BYTES: usize = 36;
/// Maximum alias claims carried by one manifest.
pub const MAX_MANIFEST_ALIAS_CLAIMS: usize = 64;
/// Maximum aggregate alias-proof bytes carried by one manifest.
pub const MAX_MANIFEST_ALIAS_PROOF_BYTES: usize = 256 * 1024;
/// Maximum metadata entries carried by one manifest.
pub const MAX_MANIFEST_METADATA_ENTRIES: usize = 128;
/// Maximum aggregate metadata key/value bytes carried by one manifest.
pub const MAX_MANIFEST_METADATA_BYTES: usize = 128 * 1024;
/// Maximum council signatures carried by one manifest.
pub const MAX_MANIFEST_COUNCIL_SIGNATURES: usize = 64;
const MAX_MANIFEST_TEXT_FIELD_BYTES: usize = 128;
const MAX_MANIFEST_METADATA_VALUE_BYTES: usize = 4096;
const MAX_MANIFEST_DECODE_SEQUENCE_ELEMENTS: usize = MAX_MANIFEST_ALIAS_PROOF_BYTES;
const MAX_MANIFEST_DECODE_TOTAL_ELEMENTS: usize = MAX_MANIFEST_ENCODED_BYTES * 2;
const MAX_MANIFEST_DECODE_ALLOCATED_BYTES: usize = MAX_MANIFEST_ENCODED_BYTES * 4;
const MAX_MANIFEST_DECODE_DEPTH: usize = 64;
const MAX_MANIFEST_BASE64_BYTES: usize = MAX_MANIFEST_ENCODED_BYTES.div_ceil(3) * 4;
/// Errors emitted while decoding an attacker-controlled manifest payload.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ManifestDecodeError {
    /// The base64 text exceeds the largest canonical spelling of a V1 manifest.
    #[error("manifest base64 payload has {found} bytes; maximum is {maximum}")]
    Base64PayloadTooLarge { found: usize, maximum: usize },
    /// The manifest text was not valid padded standard base64.
    #[error("failed to decode manifest base64 payload: {reason}")]
    Base64Decode { reason: String },
    /// The manifest used an alternate base64 spelling.
    #[error("manifest payload is not exact canonical padded standard base64")]
    NonCanonicalBase64,
    /// The wire payload exceeds the first-release manifest byte ceiling.
    #[error("manifest payload has {found} bytes; maximum is {maximum}")]
    PayloadTooLarge { found: usize, maximum: usize },
    /// Norito rejected the payload under the manifest resource budget.
    #[error("failed to decode bounded ManifestV1 payload: {reason}")]
    Decode { reason: String },
    /// The manifest could not be encoded canonically.
    #[error("failed to encode canonical ManifestV1 payload: {reason}")]
    CanonicalEncoding { reason: String },
    /// The input contained a non-canonical or trailing-byte encoding.
    #[error("manifest payload is not the exact canonical Norito encoding")]
    NonCanonicalEncoding,
}
/// Decode one exact canonical manifest under first-release resource limits.
///
/// This helper is intended for every untrusted manifest byte boundary. It
/// applies limits before allocation and rejects alternate encodings and
/// trailing bytes. This verifies the wire representation only; callers must
/// still use [`validate_manifest`] for semantic and policy validation.
pub fn decode_manifest_v1_canonical(bytes: &[u8]) -> Result<ManifestV1, ManifestDecodeError> {
    if bytes.len() > MAX_MANIFEST_ENCODED_BYTES {
        return Err(ManifestDecodeError::PayloadTooLarge {
            found: bytes.len(),
            maximum: MAX_MANIFEST_ENCODED_BYTES,
        });
    }
    let limits = norito::DecodeLimits::new(
        MAX_MANIFEST_DECODE_SEQUENCE_ELEMENTS,
        MAX_MANIFEST_ENCODED_BYTES,
        MAX_MANIFEST_DECODE_TOTAL_ELEMENTS,
        MAX_MANIFEST_DECODE_ALLOCATED_BYTES,
        MAX_MANIFEST_DECODE_DEPTH,
    );
    norito::decode_canonical_with_limits(bytes, limits).map_err(|error| match error {
        norito::Error::NonCanonicalEncoding => ManifestDecodeError::NonCanonicalEncoding,
        error => ManifestDecodeError::Decode {
            reason: error.to_string(),
        },
    })
}
/// Decode one exact canonical padded-base64 V1 manifest under resource limits.
///
/// The encoded length is checked before base64 allocation. The decoded bytes
/// then pass through [`decode_manifest_v1_canonical`], so alternate Norito
/// layouts and trailing bytes are rejected as well. This verifies the wire
/// representation only; callers must still use [`validate_manifest`] for
/// semantic and policy validation.
pub fn decode_manifest_v1_base64_canonical(
    encoded: &str,
) -> Result<ManifestV1, ManifestDecodeError> {
    if encoded.len() > MAX_MANIFEST_BASE64_BYTES {
        return Err(ManifestDecodeError::Base64PayloadTooLarge {
            found: encoded.len(),
            maximum: MAX_MANIFEST_BASE64_BYTES,
        });
    }
    let bytes = BASE64_STANDARD
        .decode(encoded.as_bytes())
        .map_err(|error| ManifestDecodeError::Base64Decode {
            reason: error.to_string(),
        })?;
    if BASE64_STANDARD.encode(&bytes) != encoded {
        return Err(ManifestDecodeError::NonCanonicalBase64);
    }
    decode_manifest_v1_canonical(&bytes)
}
/// Encode a V1 manifest as exact canonical padded standard base64.
///
/// This does not perform semantic or policy validation; callers that admit the
/// resulting manifest must use [`validate_manifest`] as well.
pub fn encode_manifest_v1_base64_canonical(
    manifest: &ManifestV1,
) -> Result<String, ManifestDecodeError> {
    let bytes = norito::encode_canonical(manifest).map_err(|error| {
        ManifestDecodeError::CanonicalEncoding {
            reason: error.to_string(),
        }
    })?;
    if bytes.len() > MAX_MANIFEST_ENCODED_BYTES {
        return Err(ManifestDecodeError::PayloadTooLarge {
            found: bytes.len(),
            maximum: MAX_MANIFEST_ENCODED_BYTES,
        });
    }
    Ok(BASE64_STANDARD.encode(bytes))
}
/// Constraints applied to the pin policy during manifest validation.
#[derive(Debug, Clone)]
pub struct PinPolicyConstraints {
    /// Minimum number of replicas that governance policy requires.
    pub min_replicas_floor: u16,
    /// Optional maximum replicas ceiling.
    pub max_replicas_ceiling: Option<u16>,
    /// Optional upper bound for retention epoch (inclusive).
    pub max_retention_epoch: Option<u64>,
    /// Allowed storage classes. When omitted, any storage class is accepted.
    pub allowed_storage_classes: Option<BTreeSet<StorageClass>>,
    /// Whether a manifest must carry council signatures.
    pub require_council_signatures: bool,
}
impl Default for PinPolicyConstraints {
    fn default() -> Self {
        Self {
            min_replicas_floor: 1,
            max_replicas_ceiling: None,
            max_retention_epoch: None,
            allowed_storage_classes: None,
            require_council_signatures: false,
        }
    }
}
/// Errors surfaced while validating a manifest for registry submission.
#[derive(Debug, thiserror::Error)]
pub enum ManifestValidationError {
    #[error("unsupported manifest version {found}; expected {expected}")]
    UnsupportedVersion { expected: u8, found: u8 },
    #[error("chunker profile id {profile_id} is not registered")]
    UnknownChunkerProfile { profile_id: u32 },
    #[error("chunker descriptor mismatch for field {field}: expected {expected}, found {found}")]
    ChunkerDescriptorMismatch {
        field: &'static str,
        expected: String,
        found: String,
    },
    #[error("manifest advertises unexpected chunker alias `{alias}`")]
    UnknownChunkerAlias { alias: String },
    #[error("manifest chunker aliases are missing canonical handle `{canonical}`")]
    MissingCanonicalAlias { canonical: String },
    #[error("manifest chunker has {found} aliases; maximum is {maximum}")]
    TooManyChunkerAliases { found: usize, maximum: usize },
    #[error("manifest chunker field {field} has {found} bytes; maximum is {maximum}")]
    ChunkerTextTooLong {
        field: &'static str,
        found: usize,
        maximum: usize,
    },
    #[error("pin policy requires at least {required} replicas but manifest specifies {found}")]
    MinReplicasTooLow { required: u16, found: u16 },
    #[error("pin policy exceeds maximum replicas {maximum}; manifest specifies {found}")]
    MaxReplicasExceeded { maximum: u16, found: u16 },
    #[error("pin retention epoch must be <= {maximum}; manifest specifies {found}")]
    RetentionEpochExceeded { maximum: u64, found: u64 },
    #[error("storage class `{found:?}` is not permitted by policy")]
    StorageClassNotAllowed { found: StorageClass },
    #[error("manifest must include at least one council signature")]
    MissingCouncilSignature,
    #[error("manifest root CID must contain exactly {maximum} bytes (found {found})")]
    InvalidRootCidLength { found: usize, maximum: usize },
    #[error("manifest root CID must not be all zero")]
    InertRootCid,
    #[error("manifest root CID version must be 1 (found {found})")]
    InvalidRootCidVersion { found: u8 },
    #[error("manifest root CID codec mismatch: expected {expected:#x}, found {found:#x}")]
    RootCidCodecMismatch { expected: u64, found: u64 },
    #[error("manifest root CID multihash mismatch: expected {expected:#x}, found {found:#x}")]
    RootCidMultihashMismatch { expected: u64, found: u64 },
    #[error("manifest root CID digest length must be {expected} bytes (found {found})")]
    InvalidRootCidDigestLength { expected: u8, found: u8 },
    #[error("manifest root CID digest must not be all zero")]
    InertRootCidDigest,
    #[error("manifest chunk-plan SHA3-256 digest must not be zero")]
    InertChunkDigest,
    #[error("non-empty manifest content requires a non-zero PoR root")]
    InertPorRoot,
    #[error("empty manifest content requires the canonical zero PoR root")]
    NonCanonicalEmptyPorRoot,
    #[error("manifest DAG codec must be non-zero")]
    InvalidDagCodec,
    #[error("unsupported manifest DAG codec {found:#x}; expected dag-cbor {expected:#x}")]
    UnsupportedDagCodec { expected: u64, found: u64 },
    #[error("manifest CAR digest must not be zero")]
    InertCarDigest,
    #[error("manifest CAR size must be positive")]
    InvalidCarSize,
    #[error("manifest CAR size {car_size} is smaller than content length {content_length}")]
    CarSmallerThanContent { car_size: u64, content_length: u64 },
    #[error("pin retention epoch must be positive")]
    InvalidRetentionEpoch,
    #[error("manifest has {found} alias claims; maximum is {maximum}")]
    TooManyAliasClaims { found: usize, maximum: usize },
    #[error("manifest alias claim {index} has invalid {field}: {reason}")]
    InvalidAliasClaim {
        index: usize,
        field: &'static str,
        reason: String,
    },
    #[error("manifest repeats alias claim `{namespace}/{name}`")]
    DuplicateAliasClaim { namespace: String, name: String },
    #[error("manifest alias proofs contain {found} bytes; maximum is {maximum}")]
    AliasProofBytesExceeded { found: usize, maximum: usize },
    #[error("manifest has {found} metadata entries; maximum is {maximum}")]
    TooManyMetadataEntries { found: usize, maximum: usize },
    #[error("manifest metadata entry {index} has invalid {field}: {reason}")]
    InvalidMetadataEntry {
        index: usize,
        field: &'static str,
        reason: String,
    },
    #[error("manifest repeats metadata key `{key}`")]
    DuplicateMetadataKey { key: String },
    #[error("manifest metadata contains {found} bytes; maximum is {maximum}")]
    MetadataBytesExceeded { found: usize, maximum: usize },
    #[error("manifest has {found} council signatures; maximum is {maximum}")]
    TooManyCouncilSignatures { found: usize, maximum: usize },
    #[error("manifest council signer at index {index} is invalid: {reason}")]
    InvalidCouncilSigner { index: usize, reason: String },
    #[error("manifest council signers must be distinct and strictly ascending (index {index})")]
    NonCanonicalCouncilSignerOrder { index: usize },
    #[error("manifest council signature at index {index} has {found} bytes; expected 64")]
    InvalidCouncilSignatureLength { index: usize, found: usize },
    #[error("manifest council signature at index {index} is invalid: {reason}")]
    InvalidCouncilSignature { index: usize, reason: String },
    #[error("manifest council signature at index {index} failed verification: {reason}")]
    CouncilSignatureVerificationFailed { index: usize, reason: String },
    #[error("failed to encode manifest for validation: {reason}")]
    ManifestEncoding { reason: String },
    #[error("canonical manifest encoding has {found} bytes; maximum is {maximum}")]
    ManifestTooLarge { found: usize, maximum: usize },
}
/// Validates the manifest according to registry policy.
pub fn validate_manifest(
    manifest: &ManifestV1,
    policy: &PinPolicyConstraints,
) -> Result<(), ManifestValidationError> {
    if manifest.version != MANIFEST_VERSION_V1 {
        return Err(ManifestValidationError::UnsupportedVersion {
            expected: MANIFEST_VERSION_V1,
            found: manifest.version,
        });
    }
    validate_manifest_geometry(manifest)?;
    validate_registered_chunker_profile(&manifest.chunking)?;
    validate_pin_policy(&manifest.pin_policy, policy)?;
    validate_alias_claims(manifest)?;
    validate_metadata(manifest)?;
    validate_governance(manifest, policy.require_council_signatures)?;
    let encoded = manifest
        .encode()
        .map_err(|error| ManifestValidationError::ManifestEncoding {
            reason: error.to_string(),
        })?;
    if encoded.len() > MAX_MANIFEST_ENCODED_BYTES {
        return Err(ManifestValidationError::ManifestTooLarge {
            found: encoded.len(),
            maximum: MAX_MANIFEST_ENCODED_BYTES,
        });
    }
    Ok(())
}
/// Validate an entire registered chunker snapshot, including immutable geometry.
///
/// A registered profile identifier is a commitment to every chunk-boundary
/// parameter. Callers admitting a full manifest must use this helper rather
/// than validating only the human-readable handle.
pub fn validate_registered_chunker_profile(
    profile: &ChunkingProfileV1,
) -> Result<&'static chunker_registry::ChunkerProfileDescriptor, ManifestValidationError> {
    for (field, value) in [
        ("namespace", profile.namespace.as_str()),
        ("name", profile.name.as_str()),
        ("semver", profile.semver.as_str()),
    ] {
        if value.len() > MAX_MANIFEST_TEXT_FIELD_BYTES {
            return Err(ManifestValidationError::ChunkerTextTooLong {
                field,
                found: value.len(),
                maximum: MAX_MANIFEST_TEXT_FIELD_BYTES,
            });
        }
    }
    if profile.aliases.len() > 16 {
        return Err(ManifestValidationError::TooManyChunkerAliases {
            found: profile.aliases.len(),
            maximum: 16,
        });
    }
    for alias in &profile.aliases {
        if alias.len() > MAX_MANIFEST_TEXT_FIELD_BYTES {
            return Err(ManifestValidationError::ChunkerTextTooLong {
                field: "aliases",
                found: alias.len(),
                maximum: MAX_MANIFEST_TEXT_FIELD_BYTES,
            });
        }
    }
    let descriptor = validate_chunker_handle(
        profile.profile_id,
        &profile.namespace,
        &profile.name,
        &profile.semver,
        profile.multihash_code,
        Some(&profile.aliases),
    )?;
    if usize::try_from(profile.min_size) != Ok(descriptor.profile.min_size) {
        return Err(ManifestValidationError::ChunkerDescriptorMismatch {
            field: "min_size",
            expected: descriptor.profile.min_size.to_string(),
            found: profile.min_size.to_string(),
        });
    }
    if usize::try_from(profile.target_size) != Ok(descriptor.profile.target_size) {
        return Err(ManifestValidationError::ChunkerDescriptorMismatch {
            field: "target_size",
            expected: descriptor.profile.target_size.to_string(),
            found: profile.target_size.to_string(),
        });
    }
    if usize::try_from(profile.max_size) != Ok(descriptor.profile.max_size) {
        return Err(ManifestValidationError::ChunkerDescriptorMismatch {
            field: "max_size",
            expected: descriptor.profile.max_size.to_string(),
            found: profile.max_size.to_string(),
        });
    }
    if u64::from(profile.break_mask) != descriptor.profile.break_mask {
        return Err(ManifestValidationError::ChunkerDescriptorMismatch {
            field: "break_mask",
            expected: descriptor.profile.break_mask.to_string(),
            found: profile.break_mask.to_string(),
        });
    }
    if profile.aliases.len() != descriptor.aliases.len()
        || profile
            .aliases
            .iter()
            .zip(descriptor.aliases)
            .any(|(provided, expected)| provided != expected)
    {
        return Err(ManifestValidationError::ChunkerDescriptorMismatch {
            field: "aliases",
            expected: descriptor.aliases.join(","),
            found: profile.aliases.join(","),
        });
    }
    Ok(descriptor)
}
/// Validates chunker metadata against the registry.
#[allow(clippy::needless_pass_by_value)]
pub fn validate_chunker_handle(
    profile_id: ProfileId,
    namespace: &str,
    name: &str,
    semver: &str,
    multihash_code: u64,
    aliases: Option<&[String]>,
) -> Result<&'static chunker_registry::ChunkerProfileDescriptor, ManifestValidationError> {
    let descriptor = chunker_registry::lookup(profile_id).ok_or(
        ManifestValidationError::UnknownChunkerProfile {
            profile_id: profile_id.0,
        },
    )?;
    if namespace != descriptor.namespace {
        return Err(ManifestValidationError::ChunkerDescriptorMismatch {
            field: "namespace",
            expected: descriptor.namespace.to_owned(),
            found: namespace.to_owned(),
        });
    }
    if name != descriptor.name {
        return Err(ManifestValidationError::ChunkerDescriptorMismatch {
            field: "name",
            expected: descriptor.name.to_owned(),
            found: name.to_owned(),
        });
    }
    if semver != descriptor.semver {
        return Err(ManifestValidationError::ChunkerDescriptorMismatch {
            field: "semver",
            expected: descriptor.semver.to_owned(),
            found: semver.to_owned(),
        });
    }
    if multihash_code != descriptor.multihash_code {
        return Err(ManifestValidationError::ChunkerDescriptorMismatch {
            field: "multihash_code",
            expected: descriptor.multihash_code.to_string(),
            found: multihash_code.to_string(),
        });
    }
    if let Some(aliases) = aliases {
        let expected_aliases: BTreeSet<String> = descriptor
            .aliases
            .iter()
            .map(|value| value.to_string())
            .collect();
        let provided_aliases: BTreeSet<String> = aliases.iter().cloned().collect();
        for alias in &provided_aliases {
            if !expected_aliases.contains(alias) {
                return Err(ManifestValidationError::UnknownChunkerAlias {
                    alias: alias.clone(),
                });
            }
        }
        let canonical = format!(
            "{}.{}@{}",
            descriptor.namespace, descriptor.name, descriptor.semver
        );
        if !provided_aliases.contains(&canonical) {
            return Err(ManifestValidationError::MissingCanonicalAlias { canonical });
        }
    }
    Ok(descriptor)
}
fn validate_manifest_geometry(manifest: &ManifestV1) -> Result<(), ManifestValidationError> {
    validate_manifest_root_cid(
        &manifest.root_cid,
        manifest.dag_codec.0,
        manifest.chunking.multihash_code,
    )?;
    if manifest.chunk_digest_sha3_256.iter().all(|byte| *byte == 0) {
        return Err(ManifestValidationError::InertChunkDigest);
    }
    match (
        manifest.content_length == 0,
        manifest.por_root == EMPTY_POR_ROOT_V1,
    ) {
        (false, true) => return Err(ManifestValidationError::InertPorRoot),
        (true, false) => return Err(ManifestValidationError::NonCanonicalEmptyPorRoot),
        (false, false) | (true, true) => {}
    }
    if manifest.car_digest.iter().all(|byte| *byte == 0) {
        return Err(ManifestValidationError::InertCarDigest);
    }
    if manifest.car_size == 0 {
        return Err(ManifestValidationError::InvalidCarSize);
    }
    if manifest.car_size < manifest.content_length {
        return Err(ManifestValidationError::CarSmallerThanContent {
            car_size: manifest.car_size,
            content_length: manifest.content_length,
        });
    }
    Ok(())
}
/// Validates the canonical first-release binary manifest-root CID.
///
/// Only CIDv1 with the dag-cbor codec and BLAKE3-256 multihash is accepted.
/// The version, codec, hash code, and digest length are single-byte canonical
/// varints, so the exact accepted wire length is 36 bytes.
pub fn validate_manifest_root_cid(
    root_cid: &[u8],
    dag_codec: u64,
    multihash_code: u64,
) -> Result<(), ManifestValidationError> {
    if root_cid.len() != MAX_MANIFEST_ROOT_CID_BYTES {
        return Err(ManifestValidationError::InvalidRootCidLength {
            found: root_cid.len(),
            maximum: MAX_MANIFEST_ROOT_CID_BYTES,
        });
    }
    if root_cid.iter().all(|byte| *byte == 0) {
        return Err(ManifestValidationError::InertRootCid);
    }
    if dag_codec == 0 {
        return Err(ManifestValidationError::InvalidDagCodec);
    }
    if dag_codec != chunker_registry::MANIFEST_DAG_CODEC {
        return Err(ManifestValidationError::UnsupportedDagCodec {
            expected: chunker_registry::MANIFEST_DAG_CODEC,
            found: dag_codec,
        });
    }
    if root_cid[0] != 1 {
        return Err(ManifestValidationError::InvalidRootCidVersion { found: root_cid[0] });
    }
    if u64::from(root_cid[1]) != dag_codec {
        return Err(ManifestValidationError::RootCidCodecMismatch {
            expected: dag_codec,
            found: u64::from(root_cid[1]),
        });
    }
    if u64::from(root_cid[2]) != multihash_code {
        return Err(ManifestValidationError::RootCidMultihashMismatch {
            expected: multihash_code,
            found: u64::from(root_cid[2]),
        });
    }
    if root_cid[3] != 32 {
        return Err(ManifestValidationError::InvalidRootCidDigestLength {
            expected: 32,
            found: root_cid[3],
        });
    }
    if root_cid[4..].iter().all(|byte| *byte == 0) {
        return Err(ManifestValidationError::InertRootCidDigest);
    }
    Ok(())
}
fn validate_manifest_label(value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err("must not be empty".to_owned());
    }
    if value.len() > MAX_MANIFEST_TEXT_FIELD_BYTES {
        return Err(format!(
            "contains {} bytes; maximum is {MAX_MANIFEST_TEXT_FIELD_BYTES}",
            value.len()
        ));
    }
    if value != value.trim() {
        return Err("must not contain surrounding whitespace".to_owned());
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b".-_".contains(&byte))
    {
        return Err("must use lowercase ASCII letters, digits, '.', '-', or '_'".to_owned());
    }
    Ok(())
}
fn validate_alias_claims(manifest: &ManifestV1) -> Result<(), ManifestValidationError> {
    if manifest.alias_claims.len() > MAX_MANIFEST_ALIAS_CLAIMS {
        return Err(ManifestValidationError::TooManyAliasClaims {
            found: manifest.alias_claims.len(),
            maximum: MAX_MANIFEST_ALIAS_CLAIMS,
        });
    }
    let mut aliases = BTreeSet::new();
    let mut proof_bytes = 0usize;
    for (index, claim) in manifest.alias_claims.iter().enumerate() {
        validate_manifest_label(&claim.namespace).map_err(|reason| {
            ManifestValidationError::InvalidAliasClaim {
                index,
                field: "namespace",
                reason,
            }
        })?;
        validate_manifest_label(&claim.name).map_err(|reason| {
            ManifestValidationError::InvalidAliasClaim {
                index,
                field: "name",
                reason,
            }
        })?;
        if claim.proof.is_empty() {
            return Err(ManifestValidationError::InvalidAliasClaim {
                index,
                field: "proof",
                reason: "must not be empty".to_owned(),
            });
        }
        proof_bytes = proof_bytes.checked_add(claim.proof.len()).ok_or(
            ManifestValidationError::AliasProofBytesExceeded {
                found: usize::MAX,
                maximum: MAX_MANIFEST_ALIAS_PROOF_BYTES,
            },
        )?;
        if proof_bytes > MAX_MANIFEST_ALIAS_PROOF_BYTES {
            return Err(ManifestValidationError::AliasProofBytesExceeded {
                found: proof_bytes,
                maximum: MAX_MANIFEST_ALIAS_PROOF_BYTES,
            });
        }
        if !aliases.insert((claim.namespace.clone(), claim.name.clone())) {
            return Err(ManifestValidationError::DuplicateAliasClaim {
                namespace: claim.namespace.clone(),
                name: claim.name.clone(),
            });
        }
    }
    Ok(())
}
fn validate_metadata(manifest: &ManifestV1) -> Result<(), ManifestValidationError> {
    if manifest.metadata.len() > MAX_MANIFEST_METADATA_ENTRIES {
        return Err(ManifestValidationError::TooManyMetadataEntries {
            found: manifest.metadata.len(),
            maximum: MAX_MANIFEST_METADATA_ENTRIES,
        });
    }
    let mut keys = BTreeSet::new();
    let mut total_bytes = 0usize;
    for (index, entry) in manifest.metadata.iter().enumerate() {
        validate_manifest_label(&entry.key).map_err(|reason| {
            ManifestValidationError::InvalidMetadataEntry {
                index,
                field: "key",
                reason,
            }
        })?;
        if entry.value.len() > MAX_MANIFEST_METADATA_VALUE_BYTES
            || entry.value.chars().any(char::is_control)
        {
            return Err(ManifestValidationError::InvalidMetadataEntry {
                index,
                field: "value",
                reason: format!(
                    "must be control-free UTF-8 of at most {MAX_MANIFEST_METADATA_VALUE_BYTES} bytes"
                ),
            });
        }
        total_bytes = total_bytes
            .checked_add(entry.key.len())
            .and_then(|total| total.checked_add(entry.value.len()))
            .ok_or(ManifestValidationError::MetadataBytesExceeded {
                found: usize::MAX,
                maximum: MAX_MANIFEST_METADATA_BYTES,
            })?;
        if total_bytes > MAX_MANIFEST_METADATA_BYTES {
            return Err(ManifestValidationError::MetadataBytesExceeded {
                found: total_bytes,
                maximum: MAX_MANIFEST_METADATA_BYTES,
            });
        }
        if !keys.insert(entry.key.clone()) {
            return Err(ManifestValidationError::DuplicateMetadataKey {
                key: entry.key.clone(),
            });
        }
    }
    Ok(())
}
pub fn validate_pin_policy(
    pin_policy: &PinPolicy,
    constraints: &PinPolicyConstraints,
) -> Result<(), ManifestValidationError> {
    if pin_policy.retention_epoch == 0 {
        return Err(ManifestValidationError::InvalidRetentionEpoch);
    }
    if pin_policy.min_replicas < constraints.min_replicas_floor {
        return Err(ManifestValidationError::MinReplicasTooLow {
            required: constraints.min_replicas_floor,
            found: pin_policy.min_replicas,
        });
    }
    if let Some(maximum) = constraints.max_replicas_ceiling
        && pin_policy.min_replicas > maximum
    {
        return Err(ManifestValidationError::MaxReplicasExceeded {
            maximum,
            found: pin_policy.min_replicas,
        });
    }
    if let Some(maximum) = constraints.max_retention_epoch
        && pin_policy.retention_epoch > maximum
    {
        return Err(ManifestValidationError::RetentionEpochExceeded {
            maximum,
            found: pin_policy.retention_epoch,
        });
    }
    if constraints
        .allowed_storage_classes
        .as_ref()
        .is_some_and(|allowed| !allowed.contains(&pin_policy.storage_class))
    {
        return Err(ManifestValidationError::StorageClassNotAllowed {
            found: pin_policy.storage_class,
        });
    }
    Ok(())
}
fn validate_governance(
    manifest: &ManifestV1,
    require_council_signatures: bool,
) -> Result<(), ManifestValidationError> {
    let proofs: &GovernanceProofs = &manifest.governance;
    if require_council_signatures && proofs.council_signatures.is_empty() {
        return Err(ManifestValidationError::MissingCouncilSignature);
    }
    if proofs.council_signatures.len() > MAX_MANIFEST_COUNCIL_SIGNATURES {
        return Err(ManifestValidationError::TooManyCouncilSignatures {
            found: proofs.council_signatures.len(),
            maximum: MAX_MANIFEST_COUNCIL_SIGNATURES,
        });
    }
    if proofs.council_signatures.is_empty() {
        return Ok(());
    }
    let mut previous_signer = None;
    for (index, signature) in proofs.council_signatures.iter().enumerate() {
        if previous_signer.is_some_and(|previous| previous >= signature.signer) {
            return Err(ManifestValidationError::NonCanonicalCouncilSignerOrder { index });
        }
        previous_signer = Some(signature.signer);
        crate::checked_ed25519_verifying_key_from_bytes(&signature.signer)
            .map_err(|reason| ManifestValidationError::InvalidCouncilSigner { index, reason })?;
        let signature_bytes: [u8; ed25519_dalek::SIGNATURE_LENGTH] =
            signature.signature.as_slice().try_into().map_err(|_| {
                ManifestValidationError::InvalidCouncilSignatureLength {
                    index,
                    found: signature.signature.len(),
                }
            })?;
        crate::checked_ed25519_signature_from_bytes(&signature_bytes)
            .map_err(|reason| ManifestValidationError::InvalidCouncilSignature { index, reason })?;
    }
    let mut unsigned = manifest.clone();
    unsigned.governance.council_signatures.clear();
    let unsigned_bytes =
        unsigned
            .encode()
            .map_err(|error| ManifestValidationError::ManifestEncoding {
                reason: error.to_string(),
            })?;
    let digest = blake3::hash(&unsigned_bytes);
    for (index, signature) in proofs.council_signatures.iter().enumerate() {
        let verifying_key = crate::checked_ed25519_verifying_key_from_bytes(&signature.signer)
            .map_err(|reason| ManifestValidationError::InvalidCouncilSigner { index, reason })?;
        let signature_bytes: [u8; ed25519_dalek::SIGNATURE_LENGTH] =
            signature.signature.as_slice().try_into().map_err(|_| {
                ManifestValidationError::InvalidCouncilSignatureLength {
                    index,
                    found: signature.signature.len(),
                }
            })?;
        let signature = crate::checked_ed25519_signature_from_bytes(&signature_bytes)
            .map_err(|reason| ManifestValidationError::InvalidCouncilSignature { index, reason })?;
        verifying_key
            .verify_strict(digest.as_bytes(), &signature)
            .map_err(
                |error| ManifestValidationError::CouncilSignatureVerificationFailed {
                    index,
                    reason: error.to_string(),
                },
            )?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use base64::Engine as _;
    use ed25519_dalek::{Signer as _, SigningKey};
    use super::*;
    use crate::{AliasClaim, CouncilSignature, GovernanceProofs, ManifestBuilder, MetadataEntry};
    use sorafs_chunker::ChunkProfile;
    fn manifest_with_defaults() -> ManifestV1 {
        let mut manifest = ManifestBuilder::new()
            .root_cid(crate::canonical_manifest_root_cid([0xAA; 32]))
            .dag_codec(crate::DagCodecId(chunker_registry::MANIFEST_DAG_CODEC))
            .chunking_from_registry(chunker_registry::default_descriptor().id)
            .chunk_digest_sha3_256([0xAC; 32])
            .por_root([0xAD; 32])
            .content_length(1_048_576)
            .car_digest([0xAB; 32])
            .car_size(1_100_000)
            .pin_policy(PinPolicy {
                min_replicas: 3,
                storage_class: StorageClass::Hot,
                retention_epoch: 10,
            })
            .build()
            .expect("manifest");
        let signing_key = SigningKey::from_bytes(&[0x11; 32]);
        let digest = manifest.digest().expect("unsigned manifest digest");
        manifest.governance = GovernanceProofs {
            council_signatures: vec![CouncilSignature {
                signer: signing_key.verifying_key().to_bytes(),
                signature: signing_key.sign(digest.as_bytes()).to_bytes().to_vec(),
            }],
        };
        manifest
    }
    fn default_constraints() -> PinPolicyConstraints {
        PinPolicyConstraints {
            min_replicas_floor: 1,
            max_replicas_ceiling: Some(5),
            max_retention_epoch: Some(20),
            allowed_storage_classes: None,
            require_council_signatures: false,
        }
    }
    #[test]
    fn validates_manifest_successfully() {
        let manifest = manifest_with_defaults();
        let constraints = default_constraints();
        let result = validate_manifest(&manifest, &constraints);
        assert!(result.is_ok());
    }
    #[test]
    fn bounded_manifest_decoder_accepts_only_exact_canonical_bytes() {
        let manifest = manifest_with_defaults();
        let canonical = norito::encode_canonical(&manifest).expect("canonical manifest");
        assert_eq!(
            decode_manifest_v1_canonical(&canonical).expect("canonical manifest decodes"),
            manifest
        );
        let mut with_trailing_bytes = canonical;
        with_trailing_bytes.extend_from_slice(&[0x00, 0xA5]);
        assert!(matches!(
            decode_manifest_v1_canonical(&with_trailing_bytes),
            Err(ManifestDecodeError::NonCanonicalEncoding)
                | Err(ManifestDecodeError::Decode { .. })
        ));
    }
    #[test]
    fn bounded_manifest_decoder_rejects_oversized_input_before_decode() {
        let oversized = vec![0_u8; MAX_MANIFEST_ENCODED_BYTES + 1];
        assert_eq!(
            decode_manifest_v1_canonical(&oversized),
            Err(ManifestDecodeError::PayloadTooLarge {
                found: MAX_MANIFEST_ENCODED_BYTES + 1,
                maximum: MAX_MANIFEST_ENCODED_BYTES,
            })
        );
    }
    #[test]
    fn canonical_base64_manifest_codec_round_trips_and_rejects_malformed_text() {
        let manifest = manifest_with_defaults();
        let encoded =
            encode_manifest_v1_base64_canonical(&manifest).expect("encode canonical base64");
        assert_eq!(
            decode_manifest_v1_base64_canonical(&encoded)
                .expect("decode canonical base64 manifest"),
            manifest
        );
        assert!(matches!(
            decode_manifest_v1_base64_canonical("%%%="),
            Err(ManifestDecodeError::Base64Decode { .. })
        ));
        assert!(matches!(
            decode_manifest_v1_base64_canonical("AA"),
            Err(ManifestDecodeError::Base64Decode { .. })
        ));
        assert!(matches!(
            decode_manifest_v1_base64_canonical("AB=="),
            Err(ManifestDecodeError::Base64Decode { .. })
        ));
    }
    #[test]
    fn canonical_base64_manifest_codec_rejects_alternate_norito_layout() {
        let manifest = manifest_with_defaults();
        let canonical = norito::encode_canonical(&manifest).expect("canonical manifest");
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let alternate = {
            let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            norito::to_bytes(&manifest).expect("alternate manifest")
        };
        assert_ne!(alternate, canonical);
        assert_eq!(
            decode_manifest_v1_base64_canonical(&BASE64_STANDARD.encode(alternate)),
            Err(ManifestDecodeError::NonCanonicalEncoding)
        );
    }
    #[test]
    fn canonical_base64_manifest_encoder_ignores_ambient_norito_layout() {
        let manifest = manifest_with_defaults();
        let expected = BASE64_STANDARD
            .encode(norito::encode_canonical(&manifest).expect("canonical manifest"));
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let actual = {
            let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            encode_manifest_v1_base64_canonical(&manifest).expect("canonical base64 manifest")
        };
        assert_eq!(actual, expected);
    }
    #[test]
    fn canonical_base64_manifest_decoder_rejects_oversized_text_before_decode() {
        let oversized = "A".repeat(MAX_MANIFEST_BASE64_BYTES + 1);
        assert_eq!(
            decode_manifest_v1_base64_canonical(&oversized),
            Err(ManifestDecodeError::Base64PayloadTooLarge {
                found: MAX_MANIFEST_BASE64_BYTES + 1,
                maximum: MAX_MANIFEST_BASE64_BYTES,
            })
        );
    }
    #[test]
    fn canonical_base64_manifest_decoder_checks_decoded_size_at_equal_text_bound() {
        let oversized_bytes = vec![0_u8; MAX_MANIFEST_ENCODED_BYTES + 1];
        let encoded = BASE64_STANDARD.encode(oversized_bytes);
        assert_eq!(encoded.len(), MAX_MANIFEST_BASE64_BYTES);
        assert_eq!(
            decode_manifest_v1_base64_canonical(&encoded),
            Err(ManifestDecodeError::PayloadTooLarge {
                found: MAX_MANIFEST_ENCODED_BYTES + 1,
                maximum: MAX_MANIFEST_ENCODED_BYTES,
            })
        );
    }
    #[test]
    fn canonical_base64_manifest_encoder_enforces_wire_size_bound() {
        let mut manifest = manifest_with_defaults();
        manifest.metadata = vec![MetadataEntry {
            key: "oversized".to_owned(),
            value: "A".repeat(MAX_MANIFEST_ENCODED_BYTES),
        }];
        assert!(matches!(
            encode_manifest_v1_base64_canonical(&manifest),
            Err(ManifestDecodeError::PayloadTooLarge {
                found,
                maximum: MAX_MANIFEST_ENCODED_BYTES,
            }) if found > MAX_MANIFEST_ENCODED_BYTES
        ));
    }
    #[test]
    fn rejects_inline_chunker_profile_manifest() {
        let mut manifest = manifest_with_defaults();
        let profile = ChunkProfile {
            min_size: 8,
            target_size: 8,
            max_size: 8,
            break_mask: 1,
        };
        manifest.chunking =
            crate::ChunkingProfileV1::from_profile(profile, crate::BLAKE3_256_MULTIHASH_CODE);
        let error = validate_manifest(&manifest, &default_constraints())
            .expect_err("first release accepts only registered profiles");
        assert!(matches!(
            error,
            ManifestValidationError::UnknownChunkerProfile { profile_id: 0 }
        ));
    }
    #[test]
    fn inline_profile_cannot_bypass_registry_by_changing_aliases() {
        let mut manifest = manifest_with_defaults();
        let profile = ChunkProfile {
            min_size: 8,
            target_size: 8,
            max_size: 8,
            break_mask: 1,
        };
        manifest.chunking =
            crate::ChunkingProfileV1::from_profile(profile, crate::BLAKE3_256_MULTIHASH_CODE);
        manifest.chunking.aliases.clear();
        let err = validate_manifest(&manifest, &default_constraints()).expect_err("should fail");
        assert!(matches!(
            err,
            ManifestValidationError::UnknownChunkerProfile { profile_id: 0 }
        ));
    }
    #[test]
    fn rejects_unknown_chunker_profile() {
        let mut manifest = manifest_with_defaults();
        manifest.chunking.profile_id = crate::ProfileId(u32::MAX);
        let err = validate_manifest(&manifest, &default_constraints()).expect_err("should fail");
        assert!(matches!(
            err,
            ManifestValidationError::UnknownChunkerProfile { profile_id }
            if profile_id == u32::MAX
        ));
    }
    #[test]
    fn rejects_chunker_metadata_mismatch() {
        let mut manifest = manifest_with_defaults();
        manifest.chunking.semver = "2.0.0".to_string();
        let err = validate_manifest(&manifest, &default_constraints()).expect_err("should fail");
        assert!(matches!(
            err,
            ManifestValidationError::ChunkerDescriptorMismatch {
                field,
                ..
            } if field == "semver"
        ));
    }
    #[test]
    fn rejects_registered_chunker_geometry_substitution() {
        for field in ["min_size", "target_size", "max_size", "break_mask"] {
            let mut manifest = manifest_with_defaults();
            match field {
                "min_size" => manifest.chunking.min_size += 1,
                "target_size" => manifest.chunking.target_size += 1,
                "max_size" => manifest.chunking.max_size += 1,
                "break_mask" => manifest.chunking.break_mask += 1,
                _ => unreachable!("fixed adversarial field list"),
            }
            let error = validate_manifest(&manifest, &default_constraints())
                .expect_err("registered geometry substitution must fail");
            assert!(matches!(
                error,
                ManifestValidationError::ChunkerDescriptorMismatch {
                    field: rejected,
                    ..
                } if rejected == field
            ));
        }
    }
    #[test]
    fn rejects_alias_without_canonical() {
        let mut manifest = manifest_with_defaults();
        manifest.chunking.aliases.clear();
        manifest.chunking.aliases.push("sorafs-sf1".to_string());
        let err = validate_manifest(&manifest, &default_constraints()).expect_err("should fail");
        assert!(matches!(
            err,
            ManifestValidationError::MissingCanonicalAlias { .. }
        ));
    }
    #[test]
    fn enforces_pin_policy_constraints() {
        let mut manifest = manifest_with_defaults();
        manifest.pin_policy.min_replicas = 0;
        let err = validate_manifest(
            &manifest,
            &PinPolicyConstraints {
                min_replicas_floor: 1,
                max_replicas_ceiling: Some(5),
                max_retention_epoch: Some(20),
                allowed_storage_classes: None,
                require_council_signatures: false,
            },
        )
        .expect_err("should fail");
        assert!(matches!(
            err,
            ManifestValidationError::MinReplicasTooLow { required, found }
            if required == 1 && found == 0
        ));
    }
    #[test]
    fn enforces_pin_policy_ceiling_retention_and_storage_class() {
        let mut manifest = manifest_with_defaults();
        manifest.pin_policy.min_replicas = 6;
        let err = validate_manifest(&manifest, &default_constraints()).expect_err("should fail");
        assert!(matches!(
            err,
            ManifestValidationError::MaxReplicasExceeded {
                maximum: 5,
                found: 6
            }
        ));
        let mut manifest = manifest_with_defaults();
        manifest.pin_policy.retention_epoch = 21;
        let err = validate_manifest(&manifest, &default_constraints()).expect_err("should fail");
        assert!(matches!(
            err,
            ManifestValidationError::RetentionEpochExceeded {
                maximum: 20,
                found: 21
            }
        ));
        let mut manifest = manifest_with_defaults();
        manifest.pin_policy.storage_class = StorageClass::Cold;
        let mut constraints = default_constraints();
        constraints.allowed_storage_classes = Some(BTreeSet::from([StorageClass::Hot]));
        let err = validate_manifest(&manifest, &constraints).expect_err("should fail");
        assert!(matches!(
            err,
            ManifestValidationError::StorageClassNotAllowed {
                found: StorageClass::Cold
            }
        ));
    }
    #[test]
    fn accepts_missing_signatures_by_default() {
        let mut manifest = manifest_with_defaults();
        manifest.governance.council_signatures.clear();
        validate_manifest(&manifest, &default_constraints())
            .expect("should allow permissionless pins");
    }
    #[test]
    fn rejects_missing_signatures_when_required() {
        let mut manifest = manifest_with_defaults();
        manifest.governance.council_signatures.clear();
        let mut constraints = default_constraints();
        constraints.require_council_signatures = true;
        let err = validate_manifest(&manifest, &constraints).expect_err("should fail");
        assert!(matches!(
            err,
            ManifestValidationError::MissingCouncilSignature
        ));
    }
    #[test]
    fn chunker_handle_valid_without_aliases() {
        let descriptor = chunker_registry::default_descriptor();
        validate_chunker_handle(
            descriptor.id,
            descriptor.namespace,
            descriptor.name,
            descriptor.semver,
            descriptor.multihash_code,
            None,
        )
        .expect("handle validates");
    }
    #[test]
    fn pin_policy_helper_enforces_constraints() {
        let policy = PinPolicy {
            min_replicas: 0,
            storage_class: StorageClass::Hot,
            retention_epoch: 1,
        };
        let err = validate_pin_policy(
            &policy,
            &PinPolicyConstraints {
                min_replicas_floor: 1,
                max_replicas_ceiling: Some(5),
                max_retention_epoch: None,
                allowed_storage_classes: None,
                require_council_signatures: false,
            },
        )
        .expect_err("policy should fail");
        assert!(matches!(
            err,
            ManifestValidationError::MinReplicasTooLow { .. }
        ));
    }
    #[test]
    fn pin_policy_rejects_zero_retention() {
        let mut policy = manifest_with_defaults().pin_policy;
        policy.retention_epoch = 0;
        assert!(matches!(
            validate_pin_policy(&policy, &default_constraints()),
            Err(ManifestValidationError::InvalidRetentionEpoch)
        ));
    }
    #[test]
    fn rejects_inert_or_inconsistent_manifest_geometry() {
        let mut empty_root = manifest_with_defaults();
        empty_root.root_cid.clear();
        assert!(matches!(
            validate_manifest(&empty_root, &default_constraints()),
            Err(ManifestValidationError::InvalidRootCidLength { .. })
        ));
        let mut zero_root = manifest_with_defaults();
        zero_root.root_cid.fill(0);
        assert!(matches!(
            validate_manifest(&zero_root, &default_constraints()),
            Err(ManifestValidationError::InertRootCid)
        ));
        let mut wrong_version = manifest_with_defaults();
        wrong_version.root_cid[0] = 2;
        assert!(matches!(
            validate_manifest(&wrong_version, &default_constraints()),
            Err(ManifestValidationError::InvalidRootCidVersion { found: 2 })
        ));
        let mut wrong_codec = manifest_with_defaults();
        wrong_codec.root_cid[1] = 0x55;
        assert!(matches!(
            validate_manifest(&wrong_codec, &default_constraints()),
            Err(ManifestValidationError::RootCidCodecMismatch { .. })
        ));
        let mut unsupported_codec = manifest_with_defaults();
        unsupported_codec.dag_codec.0 = 0x55;
        unsupported_codec.root_cid[1] = 0x55;
        assert!(matches!(
            validate_manifest(&unsupported_codec, &default_constraints()),
            Err(ManifestValidationError::UnsupportedDagCodec { .. })
        ));
        let mut wrong_multihash = manifest_with_defaults();
        wrong_multihash.root_cid[2] ^= 1;
        assert!(matches!(
            validate_manifest(&wrong_multihash, &default_constraints()),
            Err(ManifestValidationError::RootCidMultihashMismatch { .. })
        ));
        let mut wrong_digest_length = manifest_with_defaults();
        wrong_digest_length.root_cid[3] = 31;
        assert!(matches!(
            validate_manifest(&wrong_digest_length, &default_constraints()),
            Err(ManifestValidationError::InvalidRootCidDigestLength { .. })
        ));
        let mut zero_digest = manifest_with_defaults();
        zero_digest.root_cid[4..].fill(0);
        assert!(matches!(
            validate_manifest(&zero_digest, &default_constraints()),
            Err(ManifestValidationError::InertRootCidDigest)
        ));
        let mut trailing_data = manifest_with_defaults();
        trailing_data.root_cid.push(0);
        assert!(matches!(
            validate_manifest(&trailing_data, &default_constraints()),
            Err(ManifestValidationError::InvalidRootCidLength { .. })
        ));
        let mut zero_codec = manifest_with_defaults();
        zero_codec.dag_codec.0 = 0;
        assert!(matches!(
            validate_manifest(&zero_codec, &default_constraints()),
            Err(ManifestValidationError::InvalidDagCodec)
        ));
        let mut zero_chunk_digest = manifest_with_defaults();
        zero_chunk_digest.chunk_digest_sha3_256.fill(0);
        assert!(matches!(
            validate_manifest(&zero_chunk_digest, &default_constraints()),
            Err(ManifestValidationError::InertChunkDigest)
        ));
        let mut zero_por_root = manifest_with_defaults();
        zero_por_root.por_root = EMPTY_POR_ROOT_V1;
        assert!(matches!(
            validate_manifest(&zero_por_root, &default_constraints()),
            Err(ManifestValidationError::InertPorRoot)
        ));
        let mut noncanonical_empty_por_root = manifest_with_defaults();
        noncanonical_empty_por_root.content_length = 0;
        assert!(matches!(
            validate_manifest(&noncanonical_empty_por_root, &default_constraints()),
            Err(ManifestValidationError::NonCanonicalEmptyPorRoot)
        ));
        let mut canonical_empty_por_root = manifest_with_defaults();
        canonical_empty_por_root.content_length = 0;
        canonical_empty_por_root.por_root = EMPTY_POR_ROOT_V1;
        assert!(validate_manifest_geometry(&canonical_empty_por_root).is_ok());
        let mut zero_car_digest = manifest_with_defaults();
        zero_car_digest.car_digest.fill(0);
        assert!(matches!(
            validate_manifest(&zero_car_digest, &default_constraints()),
            Err(ManifestValidationError::InertCarDigest)
        ));
        let mut undersized_car = manifest_with_defaults();
        undersized_car.car_size = undersized_car.content_length - 1;
        assert!(matches!(
            validate_manifest(&undersized_car, &default_constraints()),
            Err(ManifestValidationError::CarSmallerThanContent { .. })
        ));
    }
    #[test]
    fn rejects_alias_claim_resource_and_ambiguity_attacks() {
        let mut duplicate = manifest_with_defaults();
        duplicate.alias_claims = vec![
            AliasClaim {
                name: "docs".to_owned(),
                namespace: "sora".to_owned(),
                proof: vec![1],
            },
            AliasClaim {
                name: "docs".to_owned(),
                namespace: "sora".to_owned(),
                proof: vec![2],
            },
        ];
        assert!(matches!(
            validate_manifest(&duplicate, &default_constraints()),
            Err(ManifestValidationError::DuplicateAliasClaim { .. })
        ));
        let mut oversized = manifest_with_defaults();
        oversized.alias_claims.push(AliasClaim {
            name: "docs".to_owned(),
            namespace: "sora".to_owned(),
            proof: vec![0xA5; MAX_MANIFEST_ALIAS_PROOF_BYTES + 1],
        });
        assert!(matches!(
            validate_manifest(&oversized, &default_constraints()),
            Err(ManifestValidationError::AliasProofBytesExceeded { .. })
        ));
        let mut invalid_label = manifest_with_defaults();
        invalid_label.alias_claims.push(AliasClaim {
            name: "Docs".to_owned(),
            namespace: "sora".to_owned(),
            proof: vec![1],
        });
        assert!(matches!(
            validate_manifest(&invalid_label, &default_constraints()),
            Err(ManifestValidationError::InvalidAliasClaim { field: "name", .. })
        ));
    }
    #[test]
    fn rejects_metadata_resource_and_duplicate_key_attacks() {
        let mut duplicate = manifest_with_defaults();
        duplicate.metadata = vec![
            MetadataEntry {
                key: "build".to_owned(),
                value: "one".to_owned(),
            },
            MetadataEntry {
                key: "build".to_owned(),
                value: "two".to_owned(),
            },
        ];
        assert!(matches!(
            validate_manifest(&duplicate, &default_constraints()),
            Err(ManifestValidationError::DuplicateMetadataKey { .. })
        ));
        let mut flood = manifest_with_defaults();
        flood.metadata = (0..=MAX_MANIFEST_METADATA_ENTRIES)
            .map(|index| MetadataEntry {
                key: format!("key{index}"),
                value: String::new(),
            })
            .collect();
        assert!(matches!(
            validate_manifest(&flood, &default_constraints()),
            Err(ManifestValidationError::TooManyMetadataEntries { .. })
        ));
        let mut control = manifest_with_defaults();
        control.metadata.push(MetadataEntry {
            key: "note".to_owned(),
            value: "line\nbreak".to_owned(),
        });
        assert!(matches!(
            validate_manifest(&control, &default_constraints()),
            Err(ManifestValidationError::InvalidMetadataEntry { field: "value", .. })
        ));
    }
    #[test]
    fn verifies_manifest_council_signatures_and_rejects_replay_shapes() {
        let mut invalid = manifest_with_defaults();
        invalid.governance.council_signatures[0].signature[0] ^= 1;
        assert!(matches!(
            validate_manifest(&invalid, &default_constraints()),
            Err(ManifestValidationError::CouncilSignatureVerificationFailed { .. })
                | Err(ManifestValidationError::InvalidCouncilSignature { .. })
        ));
        let mut duplicate = manifest_with_defaults();
        duplicate
            .governance
            .council_signatures
            .push(duplicate.governance.council_signatures[0].clone());
        assert!(matches!(
            validate_manifest(&duplicate, &default_constraints()),
            Err(ManifestValidationError::NonCanonicalCouncilSignerOrder { .. })
        ));
        let mut flood = manifest_with_defaults();
        flood.governance.council_signatures = (0..=MAX_MANIFEST_COUNCIL_SIGNATURES)
            .map(|index| CouncilSignature {
                signer: [u8::try_from(index + 1).expect("fixture index fits u8"); 32],
                signature: vec![1; 64],
            })
            .collect();
        assert!(matches!(
            validate_manifest(&flood, &default_constraints()),
            Err(ManifestValidationError::TooManyCouncilSignatures { .. })
        ));
    }
}
