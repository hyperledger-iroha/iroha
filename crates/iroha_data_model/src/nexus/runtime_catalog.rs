//! Additive, consensus-committed Nexus catalog requests and cumulative state.
//!
//! These types bind canonical data and bounded opaque manifest sources. The executor must also
//! authorize the request, compare all expected roots, preserve existing catalog entries, validate
//! the native manifest schema and validator authority, and install the complete update atomically.
use super::{
    DataSpaceCatalog, DataSpaceMetadata, LaneCatalog, LaneConfig, MAX_ACTIVE_EXECUTION_LANES,
};
use crate::{
    DeriveJsonDeserialize, DeriveJsonSerialize,
    consensus::MAX_LANE_CONSENSUS_VALIDATORS,
    parameter::{CustomParameter, CustomParameterId},
};
use iroha_crypto::Hash;
use iroha_model_base::topology::{DataSpaceId, LaneId};
use iroha_primitives::json::{Json, MAX_JSON_BYTES};
use iroha_schema::IntoSchema;
use norito::{
    codec::{Decode, Encode},
    json::{self, JsonDeserializeOwned, JsonSerialize},
};
use std::num::NonZeroU32;
use thiserror::Error;

/// Maximum entries of each kind in a transition or cumulative runtime overlay.
pub const MAX_NEXUS_RUNTIME_CATALOG_ENTRIES: usize = MAX_ACTIVE_EXECUTION_LANES;
/// Maximum canonical source bytes in one inline native lane manifest.
pub const MAX_NEXUS_RUNTIME_MANIFEST_BYTES: usize = 256 * 1024;
/// Maximum complete custom-parameter JSON, including its cumulative manifest sources.
pub const MAX_NEXUS_RUNTIME_CATALOG_BYTES: usize = MAX_JSON_BYTES;
const RUNTIME_CATALOG_DOMAIN: &[u8] = b"iroha:nexus:runtime-catalog:v1\0";
const MAX_ALIAS_BYTES: usize = 128;
const MAX_DESCRIPTION_BYTES: usize = 4 * 1024;
const DECODE_LIMITS: norito::DecodeLimits = norito::DecodeLimits::new(
    MAX_NEXUS_RUNTIME_CATALOG_ENTRIES,
    MAX_NEXUS_RUNTIME_CATALOG_BYTES,
    128 * 1024,
    16 * MAX_NEXUS_RUNTIME_CATALOG_BYTES,
    32,
);

/// One physical dataspace added without replacing an existing catalog entry.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::nexus::RuntimeDataSpaceAdditionV1")]
pub struct RuntimeDataSpaceAdditionV1 {
    /// Complete native descriptor whose identity is derived from `manifest_hash`.
    pub descriptor: DataSpaceMetadata,
    /// Stable identity hash; this is not a digest of the inline lane manifest JSON.
    pub manifest_hash: [u8; 32],
}

/// One exact native manifest source added for a lane.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::nexus::RuntimeLaneManifestV1")]
pub struct RuntimeLaneManifestV1 {
    /// Lane whose native manifest is being installed.
    pub lane_id: LaneId,
    /// Canonical, bounded [`super::NativeLaneManifestV1`] JSON. Core validates its schema and authority.
    pub manifest: Json,
}

/// Authorized request to atomically extend the runtime dataspace, lane and manifest catalogs.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::nexus::NexusCatalogTransitionV1")]
pub struct NexusCatalogTransitionV1 {
    /// Layout version, exactly [`Self::VERSION`].
    pub version: u8,
    /// Expected current lane catalog commitment.
    pub expected_catalog_hash: Hash,
    /// Expected current lane-incarnation commitment.
    pub expected_incarnation_root: Hash,
    /// Expected cumulative runtime overlay, or `None` only before the first overlay exists.
    #[norito(required)]
    pub expected_runtime_catalog_hash: Option<Hash>,
    /// New physical dataspaces, strictly ordered by their numeric identity.
    pub dataspace_additions: Vec<RuntimeDataSpaceAdditionV1>,
    /// New execution lanes, strictly ordered by lane ID.
    pub lane_additions: Vec<LaneConfig>,
    /// New exact manifest sources, strictly ordered by lane ID.
    pub manifest_additions: Vec<RuntimeLaneManifestV1>,
}

/// Protected cumulative overlay persisted through the ordinary World custom-parameter state.
// Lane descriptors/incarnations remain in their existing committed lane catalog. Their additions
// are carried by NexusCatalogTransitionV1 and are not independently duplicated in this overlay.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::nexus::NexusRuntimeCatalogV1")]
pub struct NexusRuntimeCatalogV1 {
    /// Layout version, exactly [`Self::VERSION`].
    pub version: u8,
    /// Commitment to the full immutable configured dataspace baseline.
    pub baseline_dataspaces_hash: Hash,
    /// Commitment to the immutable configured native manifest-source baseline.
    pub baseline_manifests_hash: Hash,
    /// All authenticated physical dataspace additions, strictly ordered by identity.
    pub dataspaces: Vec<RuntimeDataSpaceAdditionV1>,
    /// All authenticated inline manifest additions, strictly ordered by lane ID.
    pub manifests: Vec<RuntimeLaneManifestV1>,
}

/// Invalid or non-canonical additive Nexus catalog data.
#[derive(Debug, Error)]
pub enum NexusCatalogValidationError {
    /// Only the V1 layout is accepted.
    #[error("unsupported Nexus runtime catalog version {0}")]
    UnsupportedVersion(u8),
    /// A required baseline or expected-state commitment was an empty digest.
    #[error("Nexus catalog commitment must be non-zero: {0}")]
    ZeroCommitment(&'static str),
    /// No additions were supplied.
    #[error("Nexus catalog transition cannot be empty")]
    EmptyTransition,
    /// Canonical numeric ordering was violated or an ID repeated.
    #[error("Nexus catalog entries must have strictly increasing IDs: {0}")]
    NonCanonicalOrder(&'static str),
    /// Distinct entries reused an alias.
    #[error("duplicate Nexus catalog alias: {0}")]
    DuplicateAlias(String),
    /// An entry had an invalid native descriptor or identity binding.
    #[error("invalid Nexus runtime dataspace: {0}")]
    InvalidDataSpace(&'static str),
    /// A native lane descriptor was invalid.
    #[error("invalid Nexus runtime lane: {0}")]
    InvalidLane(String),
    /// Manifest input was not a bounded canonical object.
    #[error("invalid Nexus runtime manifest: {0}")]
    InvalidManifest(&'static str),
    /// A protocol byte/count bound was exceeded.
    #[error("Nexus runtime catalog exceeds {0}")]
    BoundExceeded(&'static str),
    /// Original typed JSON reader failure, including its decoder scope identity.
    #[error("Nexus runtime catalog codec: {0}")]
    Json(#[source] json::Error),
    /// Fixed protocol lexical bound or syntax failure; this is not local admission.
    #[error("Nexus runtime catalog codec: {0}")]
    Preflight(#[source] json::JsonPreflightError),
    /// Original counted JSON writer failure.
    #[error("Nexus runtime catalog codec: {0}")]
    JsonEncoding(#[source] json::BoundedJsonError),
    /// Original bounded canonical frame writer failure.
    #[error("Nexus runtime catalog codec: {0}")]
    BinaryEncoding(#[source] norito::core::BoundedEncodeError),
    /// Original canonical JSON custody or native codec failure.
    #[error("Nexus runtime catalog codec: {0}")]
    Codec(#[source] norito::Error),
}

impl NexusCatalogValidationError {
    /// Move an original decoder cause into its caller's active admission observer.
    ///
    /// This performs no classification and creates no observer. The caller must
    /// observe the actual operation before invoking this method. Scoped resource
    /// owners retain their exact provenance; fixed protocol bounds and malformed
    /// JSON stay in their original model variants instead of becoming local refusal.
    ///
    /// # Errors
    /// Returns this unchanged error when it is not a native decoder cause or a
    /// JSON resource report. No formatting or allocation occurs on that path.
    pub fn into_decode_error(self) -> Result<norito::Error, Self> {
        match self {
            Self::Json(
                original @ (json::Error::ScopedDecodeResource(_)
                | json::Error::DecodeResource(_)
                | json::Error::AllocationFailed),
            ) => Ok(original.into_core_error()),
            Self::JsonEncoding(
                original @ (json::BoundedJsonError::ScopedDecodeResource(_)
                | json::BoundedJsonError::DecodeResource(_)
                | json::BoundedJsonError::AllocationFailed),
            ) => Ok(original.into_core_error()),
            Self::Codec(original)
            | Self::BinaryEncoding(norito::core::BoundedEncodeError::Serialization(original)) => {
                Ok(original)
            }
            Self::BinaryEncoding(norito::core::BoundedEncodeError::AllocationFailed { bytes }) => {
                Ok(norito::Error::AllocationFailed {
                    bytes: bytes as u64,
                })
            }
            original => Err(original),
        }
    }
}

impl RuntimeDataSpaceAdditionV1 {
    /// Validate canonical descriptor spelling, native identity derivation and committee bounds.
    ///
    /// # Errors
    /// Rejects reserved IDs, mismatched hashes, invalid aliases, oversized descriptions or quorum
    /// geometry exceeding the native lane validator bound.
    pub fn validate_structure(&self) -> Result<(), NexusCatalogValidationError> {
        let descriptor = &self.descriptor;
        if self.manifest_hash == [0; 32]
            || descriptor.id == DataSpaceId::UNIVERSAL
            || DataSpaceId::from_hash(&self.manifest_hash) != descriptor.id
        {
            return Err(NexusCatalogValidationError::InvalidDataSpace(
                "hash/id binding",
            ));
        }
        validate_alias(&descriptor.alias)?;
        if descriptor.alias == "universal" {
            return Err(NexusCatalogValidationError::InvalidDataSpace(
                "reserved universal alias",
            ));
        }
        if descriptor
            .description
            .as_ref()
            .is_some_and(|value| value.len() > MAX_DESCRIPTION_BYTES)
        {
            return Err(NexusCatalogValidationError::BoundExceeded(
                "dataspace description bytes",
            ));
        }
        let committee = descriptor
            .fault_tolerance
            .checked_mul(3)
            .and_then(|value| value.checked_add(1));
        if descriptor.fault_tolerance == 0
            || committee.is_none_or(|size| size as usize > MAX_LANE_CONSENSUS_VALIDATORS)
        {
            return Err(NexusCatalogValidationError::InvalidDataSpace(
                "committee size",
            ));
        }
        Ok(())
    }
}

impl RuntimeLaneManifestV1 {
    /// Validate source bounds and object shape; Core owns all manifest schema/authority checks.
    ///
    /// # Errors
    /// Rejects a non-object, empty object, or source exceeding its byte/structure budget.
    pub fn validate_structure(&self) -> Result<(), NexusCatalogValidationError> {
        let raw = self.manifest.get();
        if raw.len() > MAX_NEXUS_RUNTIME_MANIFEST_BYTES {
            return Err(NexusCatalogValidationError::BoundExceeded(
                "individual manifest bytes",
            ));
        }
        preflight(raw, MAX_NEXUS_RUNTIME_MANIFEST_BYTES)?;
        if !raw.starts_with('{') || raw == "{}" {
            return Err(NexusCatalogValidationError::InvalidManifest(
                "nonempty native object required",
            ));
        }
        Ok(())
    }
}

impl NexusCatalogTransitionV1 {
    /// Supported layout version.
    pub const VERSION: u8 = 1;
    /// User-request parameter handled by the additive catalog executor.
    pub const PARAMETER_ID_STR: &'static str = "nexus_catalog_transition_v1";

    /// Validate canonical additive request structure without authorizing or applying it.
    ///
    /// # Errors
    /// Rejects unsupported versions, empty requests, non-canonical entries and codec bounds.
    pub fn validate_structure(&self) -> Result<(), NexusCatalogValidationError> {
        self.validated_json().map(|_| ())
    }

    // Keep the validated output for authoring instead of allocating and serializing it again.
    // This does not bypass lexical preflight or transfer ownership of the canonical Json backing.
    fn validated_json(&self) -> Result<String, NexusCatalogValidationError> {
        version(self.version)?;
        nonzero(self.expected_catalog_hash, "expected_catalog_hash")?;
        nonzero(self.expected_incarnation_root, "expected_incarnation_root")?;
        if let Some(hash) = self.expected_runtime_catalog_hash {
            nonzero(hash, "expected_runtime_catalog_hash")?;
        }
        if self.dataspace_additions.is_empty()
            && self.lane_additions.is_empty()
            && self.manifest_additions.is_empty()
        {
            return Err(NexusCatalogValidationError::EmptyTransition);
        }
        validate_dataspaces(&self.dataspace_additions)?;
        validate_lanes(&self.lane_additions)?;
        validate_manifests(&self.manifest_additions)?;
        let encoded = bounded_json(self)?;
        preflight(&encoded, MAX_NEXUS_RUNTIME_CATALOG_BYTES)?;
        Ok(encoded)
    }

    /// Return the exact transition custom-parameter ID.
    #[must_use]
    pub fn parameter_id() -> CustomParameterId {
        Self::PARAMETER_ID_STR
            .parse()
            .expect("valid transition parameter ID")
    }

    /// Encode a structurally validated request without signing or executing it.
    ///
    /// # Errors
    /// Returns structural or bounded codec failures.
    pub fn into_custom_parameter(self) -> Result<CustomParameter, NexusCatalogValidationError> {
        let encoded = self.validated_json()?;
        custom_parameter_from_validated_json(Self::parameter_id(), &encoded)
    }

    /// Decode only this reserved parameter and validate its complete bounded structure.
    ///
    /// # Errors
    /// A matching malformed/unsupported parameter fails; an unrelated ID returns `None`.
    pub fn from_custom_parameter(
        custom: &CustomParameter,
    ) -> Result<Option<Self>, NexusCatalogValidationError> {
        if custom.id() != &Self::parameter_id() {
            return Ok(None);
        }
        let value: Self = decode_payload(custom.payload())?;
        value.validate_structure()?;
        Ok(Some(value))
    }
}

impl NexusRuntimeCatalogV1 {
    /// Supported layout version.
    pub const VERSION: u8 = 1;
    /// Protected state key; a direct user `SetParameter` for this key must be rejected by Core.
    pub const PARAMETER_ID_STR: &'static str = "nexus_runtime_catalog_v1";

    /// Validate the canonical cumulative overlay; this does not authenticate state origin.
    ///
    /// # Errors
    /// Rejects unsupported versions, zero baselines, non-canonical entries and codec bounds.
    pub fn validate_structure(&self) -> Result<(), NexusCatalogValidationError> {
        self.validated_json().map(|_| ())
    }

    // Keep the validated output for authoring instead of allocating and serializing it again.
    // This does not bypass lexical preflight or transfer ownership of the canonical Json backing.
    fn validated_json(&self) -> Result<String, NexusCatalogValidationError> {
        version(self.version)?;
        nonzero(self.baseline_dataspaces_hash, "baseline_dataspaces_hash")?;
        nonzero(self.baseline_manifests_hash, "baseline_manifests_hash")?;
        validate_dataspaces(&self.dataspaces)?;
        validate_manifests(&self.manifests)?;
        let encoded = bounded_json(self)?;
        preflight(&encoded, MAX_NEXUS_RUNTIME_CATALOG_BYTES)?;
        Ok(encoded)
    }

    /// Compute the canonical domain-separated root of this complete validated overlay.
    ///
    /// # Errors
    /// Returns structural or bounded native encoding failures.
    pub fn canonical_hash(&self) -> Result<Hash, NexusCatalogValidationError> {
        self.validate_structure()?;
        streamed_catalog_hash(self, 2 * MAX_NEXUS_RUNTIME_CATALOG_BYTES)
    }

    /// Return the exact protected cumulative-state custom-parameter ID.
    #[must_use]
    pub fn parameter_id() -> CustomParameterId {
        Self::PARAMETER_ID_STR
            .parse()
            .expect("valid runtime catalog parameter ID")
    }

    /// Encode validated protected state for use by the trusted catalog executor.
    ///
    /// # Errors
    /// Returns structural or bounded codec failures. This method grants no write authority.
    pub fn into_custom_parameter(self) -> Result<CustomParameter, NexusCatalogValidationError> {
        let encoded = self.validated_json()?;
        custom_parameter_from_validated_json(Self::parameter_id(), &encoded)
    }

    /// Decode and validate this protected state parameter without authenticating its origin.
    ///
    /// # Errors
    /// A matching malformed/unsupported parameter fails; an unrelated ID returns `None`.
    pub fn from_custom_parameter(
        custom: &CustomParameter,
    ) -> Result<Option<Self>, NexusCatalogValidationError> {
        if custom.id() != &Self::parameter_id() {
            return Ok(None);
        }
        let value: Self = decode_payload(custom.payload())?;
        value.validate_structure()?;
        Ok(Some(value))
    }
}

/// Commit to the complete canonical native configured dataspace baseline, including descriptions.
#[must_use]
pub fn dataspace_catalog_hash(catalog: &DataSpaceCatalog) -> Hash {
    try_dataspace_catalog_hash(catalog).expect("canonical dataspace catalog encoding")
}

/// Hash the original catalog's canonical bare sequence without copying its graph or bytes.
///
/// # Errors
/// Returns the native serialization failure rather than producing a partial commitment.
pub fn try_dataspace_catalog_hash(catalog: &DataSpaceCatalog) -> Result<Hash, std::io::Error> {
    struct Entries<'a>(&'a [DataSpaceMetadata]);
    impl norito::SerializePayload for Entries<'_> {
        fn serialize(
            &self,
            out: &mut norito::core::Encoder<'_>,
        ) -> Result<(), norito::core::Error> {
            norito::core::write_element_sequence::<DataSpaceMetadata, _>(out, self.0.iter())
        }
    }
    Hash::new_from_writer(|out| {
        out.write_all(b"iroha:nexus:dataspace-catalog:v1\0")?;
        norito::codec::encode_adaptive_into(&Entries(catalog.entries()), &mut &mut *out)
            .map_err(std::io::Error::other)?;
        Ok(())
    })
}

fn version(value: u8) -> Result<(), NexusCatalogValidationError> {
    if value == 1 {
        Ok(())
    } else {
        Err(NexusCatalogValidationError::UnsupportedVersion(value))
    }
}
fn nonzero(hash: Hash, field: &'static str) -> Result<(), NexusCatalogValidationError> {
    // Native Hash adds its low-bit marker even to an all-zero prehashed digest. Reject that
    // canonical empty sentinel as well as raw zero bytes; checking only the latter misses it.
    if hash == Hash::prehashed([0; Hash::LENGTH]) || hash.as_ref().iter().all(|byte| *byte == 0) {
        Err(NexusCatalogValidationError::ZeroCommitment(field))
    } else {
        Ok(())
    }
}
fn validate_alias(alias: &str) -> Result<(), NexusCatalogValidationError> {
    if alias.is_empty()
        || alias.trim() != alias
        || alias.len() > MAX_ALIAS_BYTES
        || alias.chars().any(char::is_control)
    {
        return Err(NexusCatalogValidationError::InvalidDataSpace(
            "non-canonical alias",
        ));
    }
    Ok(())
}
fn validate_dataspaces(
    entries: &[RuntimeDataSpaceAdditionV1],
) -> Result<(), NexusCatalogValidationError> {
    count(entries.len())?;
    if entries
        .windows(2)
        .any(|pair| pair[0].descriptor.id >= pair[1].descriptor.id)
    {
        return Err(NexusCatalogValidationError::NonCanonicalOrder("dataspaces"));
    }
    for (index, entry) in entries.iter().enumerate() {
        entry.validate_structure()?;
        if entries[..index]
            .iter()
            .any(|previous| previous.descriptor.alias == entry.descriptor.alias)
        {
            return Err(NexusCatalogValidationError::DuplicateAlias(
                entry.descriptor.alias.clone(),
            ));
        }
    }
    Ok(())
}
fn validate_lanes(entries: &[LaneConfig]) -> Result<(), NexusCatalogValidationError> {
    count(entries.len())?;
    if entries.windows(2).any(|pair| pair[0].id >= pair[1].id) {
        return Err(NexusCatalogValidationError::NonCanonicalOrder("lanes"));
    }
    for entry in entries {
        validate_alias(&entry.alias)?;
        if entry
            .description
            .as_ref()
            .is_some_and(|value| value.len() > MAX_DESCRIPTION_BYTES)
        {
            return Err(NexusCatalogValidationError::BoundExceeded(
                "lane description bytes",
            ));
        }
    }
    if let Some(last) = entries.last() {
        let bound = last
            .id
            .as_u32()
            .checked_add(1)
            .and_then(NonZeroU32::new)
            .ok_or_else(|| {
                NexusCatalogValidationError::InvalidLane(
                    "lane ID cannot form exclusive bound".into(),
                )
            })?;
        LaneCatalog::validate_borrowed(bound, entries)
            .map_err(|error| NexusCatalogValidationError::InvalidLane(error.to_string()))?;
    }
    Ok(())
}
fn validate_manifests(
    entries: &[RuntimeLaneManifestV1],
) -> Result<(), NexusCatalogValidationError> {
    count(entries.len())?;
    if entries
        .windows(2)
        .any(|pair| pair[0].lane_id >= pair[1].lane_id)
    {
        return Err(NexusCatalogValidationError::NonCanonicalOrder("manifests"));
    }
    for entry in entries {
        entry.validate_structure()?;
    }
    Ok(())
}
fn count(len: usize) -> Result<(), NexusCatalogValidationError> {
    if len > MAX_NEXUS_RUNTIME_CATALOG_ENTRIES {
        Err(NexusCatalogValidationError::BoundExceeded("entry count"))
    } else {
        Ok(())
    }
}
fn preflight(raw: &str, max: usize) -> Result<(), NexusCatalogValidationError> {
    let limits = json::JsonPreflightLimits::new(
        max,
        128 * 1024,
        max,
        max,
        max,
        MAX_NEXUS_RUNTIME_CATALOG_ENTRIES,
        MAX_NEXUS_RUNTIME_CATALOG_ENTRIES,
        MAX_NEXUS_RUNTIME_CATALOG_ENTRIES,
        128 * 1024,
        32,
    );
    json::preflight_slice(raw.as_bytes(), limits)
        .map(|_| ())
        .map_err(NexusCatalogValidationError::Preflight)
}
fn bounded_json<T: JsonSerialize>(value: &T) -> Result<String, NexusCatalogValidationError> {
    json::to_json_bounded(value, MAX_NEXUS_RUNTIME_CATALOG_BYTES)
        .map_err(NexusCatalogValidationError::JsonEncoding)
}
// Retained only as an independent authoring/refusal oracle in the unit controls.
#[cfg(test)]
fn custom_parameter<T: JsonSerialize>(
    id: CustomParameterId,
    value: &T,
) -> Result<CustomParameter, NexusCatalogValidationError> {
    let encoded = bounded_json(value)?;
    preflight(&encoded, MAX_NEXUS_RUNTIME_CATALOG_BYTES)?;
    custom_parameter_from_validated_json(id, &encoded)
}

// Both production callers supply their exact validated_json result. Preserve the existing typed
// parse and canonical custody: derive field order is not canonical Json object-key order.
fn custom_parameter_from_validated_json(
    id: CustomParameterId,
    encoded: &str,
) -> Result<CustomParameter, NexusCatalogValidationError> {
    let payload = norito::with_decode_limits_scope(DECODE_LIMITS, || {
        let value: json::Value =
            json::from_str(encoded).map_err(NexusCatalogValidationError::Json)?;
        Json::from_norito_value_ref(&value).map_err(NexusCatalogValidationError::Codec)
    })?;
    Ok(CustomParameter::new(id, payload))
}
// The codec measures this exact borrowed value before invoking the prefix. It then enforces the
// same payload length, checksum and layout flags while streaming its second pass. The prefix emits
// no bytes and rejects an oversized complete frame before its header reaches the destination.
// Preserve active layout flags exactly as to_bytes_bounded did; forcing default flags would change
// existing commitments inside a non-default layout scope. Serializer-internal scratch remains its
// owner's responsibility: this removes only the complete frame destination.
fn write_catalog_frame<T: norito::core::NoritoSerialize>(
    value: &T,
    writer: &mut dyn std::io::Write,
    maximum: usize,
) -> Result<(), norito::core::BoundedEncodeError> {
    let mut bound = None;
    let result = norito::core::write_frame_with_prefix(
        value,
        &mut norito::core::Encoder::new(writer),
        |_, encoded_bytes| {
            if encoded_bytes > maximum {
                bound = Some(norito::core::BoundedEncodeError::FrameTooLarge {
                    encoded_bytes,
                    max_bytes: maximum,
                });
                return Err(norito::Error::LengthMismatch);
            }
            Ok(())
        },
    );
    match bound {
        Some(original) => Err(original),
        None => result.map_err(norito::core::BoundedEncodeError::Serialization),
    }
}

fn streamed_catalog_hash<T: norito::core::NoritoSerialize>(
    value: &T,
    maximum: usize,
) -> Result<Hash, NexusCatalogValidationError> {
    let mut codec = None;
    let hash = Hash::new_from_writer(|writer| {
        writer.write_all(RUNTIME_CATALOG_DOMAIN)?;
        write_catalog_frame(value, writer, maximum).map_err(|original| {
            codec = Some(original);
            // Keep the original typed cause outside the I/O bridge without formatting or boxing.
            std::io::Error::from(std::io::ErrorKind::Other)
        })
    });
    if let Some(original) = codec {
        return Err(NexusCatalogValidationError::BinaryEncoding(original));
    }
    hash.map_err(|original| {
        NexusCatalogValidationError::BinaryEncoding(
            norito::core::BoundedEncodeError::Serialization(norito::Error::Io(original)),
        )
    })
}

fn decode_payload<T: JsonDeserializeOwned>(
    payload: &Json,
) -> Result<T, NexusCatalogValidationError> {
    preflight(payload.get(), MAX_NEXUS_RUNTIME_CATALOG_BYTES)?;
    norito::with_decode_limits_scope(DECODE_LIMITS, || json::from_str(payload.get()))
        .map_err(NexusCatalogValidationError::Json)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dataspace(id: u64, alias: &str) -> RuntimeDataSpaceAdditionV1 {
        let mut hash = [0_u8; 32];
        hash[..8].copy_from_slice(&id.to_le_bytes());
        RuntimeDataSpaceAdditionV1 {
            descriptor: DataSpaceMetadata {
                id: DataSpaceId::new(id),
                alias: alias.into(),
                description: None,
                fault_tolerance: 1,
            },
            manifest_hash: hash,
        }
    }
    fn manifest(id: u32) -> RuntimeLaneManifestV1 {
        RuntimeLaneManifestV1 {
            lane_id: LaneId::new(id),
            manifest: Json::new(json::Value::Object(
                [("lane".into(), json::Value::String(format!("lane-{id}")))]
                    .into_iter()
                    .collect(),
            )),
        }
    }
    fn transition() -> NexusCatalogTransitionV1 {
        NexusCatalogTransitionV1 {
            version: 1,
            expected_catalog_hash: Hash::new(b"catalog"),
            expected_incarnation_root: Hash::new(b"incarnations"),
            expected_runtime_catalog_hash: None,
            dataspace_additions: vec![dataspace(9, "nine")],
            lane_additions: vec![LaneConfig {
                id: LaneId::new(7),
                dataspace_id: DataSpaceId::new(9),
                alias: "lane-7".into(),
                ..LaneConfig::default()
            }],
            manifest_additions: vec![manifest(7)],
        }
    }
    fn runtime() -> NexusRuntimeCatalogV1 {
        NexusRuntimeCatalogV1 {
            version: 1,
            baseline_dataspaces_hash: Hash::new(b"baseline-ds"),
            baseline_manifests_hash: Hash::new(b"baseline-manifests"),
            dataspaces: vec![dataspace(9, "nine")],
            manifests: vec![manifest(7)],
        }
    }
    #[test]
    fn additive_catalog_parameters_roundtrip_without_losing_canonical_identity() {
        let request = transition();
        let custom = request.clone().into_custom_parameter().unwrap();
        assert_eq!(
            NexusCatalogTransitionV1::from_custom_parameter(&custom).unwrap(),
            Some(request.clone())
        );
        assert!(
            NexusRuntimeCatalogV1::from_custom_parameter(&custom)
                .unwrap()
                .is_none()
        );
        let encoded = norito::to_bytes(&request).unwrap();
        let decoded: NexusCatalogTransitionV1 = norito::decode_from_bytes(&encoded).unwrap();
        assert_eq!(decoded, request);
        let state = runtime();
        let root = state.canonical_hash().unwrap();
        let custom = state.clone().into_custom_parameter().unwrap();
        let decoded = NexusRuntimeCatalogV1::from_custom_parameter(&custom)
            .unwrap()
            .unwrap();
        assert_eq!(decoded, state);
        assert_eq!(decoded.canonical_hash().unwrap(), root);
        assert!(
            NexusCatalogTransitionV1::from_custom_parameter(&custom)
                .unwrap()
                .is_none()
        );
    }
    #[test]
    fn additive_catalog_rejects_unknown_duplicate_fields_and_wrong_versions() {
        let text = json::to_json(&transition()).unwrap();
        for altered in [
            text.replacen('{', "{\"unknown\":0,", 1),
            text.replacen('{', "{\"version\":1,", 1),
        ] {
            assert!(json::from_str::<NexusCatalogTransitionV1>(&altered).is_err());
        }
        let omitted = text.replace("\"expected_runtime_catalog_hash\":null,", "");
        assert_ne!(omitted, text);
        assert!(json::from_str::<NexusCatalogTransitionV1>(&omitted).is_err());
        let state_text = json::to_json(&runtime()).unwrap();
        for altered in [
            state_text.replacen('{', "{\"unknown\":0,", 1),
            state_text.replacen('{', "{\"version\":1,", 1),
        ] {
            assert!(json::from_str::<NexusRuntimeCatalogV1>(&altered).is_err());
        }
        let descriptor_text = json::to_json(&dataspace(9, "nine").descriptor).unwrap();
        for altered in [
            descriptor_text.replacen('{', "{\"unknown\":0,", 1),
            descriptor_text.replacen('{', "{\"fault_tolerance\":1,", 1),
        ] {
            assert!(json::from_str::<DataSpaceMetadata>(&altered).is_err());
        }
        let mut request = transition();
        request.version = 2;
        assert!(request.clone().into_custom_parameter().is_err());
        let custom =
            CustomParameter::new(NexusCatalogTransitionV1::parameter_id(), Json::new(request));
        assert!(NexusCatalogTransitionV1::from_custom_parameter(&custom).is_err());
        let mut state = runtime();
        state.version = 2;
        assert!(state.into_custom_parameter().is_err());
    }
    #[test]
    fn additive_catalog_rejects_empty_noncanonical_and_duplicate_entries() {
        let mut request = transition();
        request.dataspace_additions.clear();
        request.lane_additions.clear();
        request.manifest_additions.clear();
        assert!(matches!(
            request.validate_structure(),
            Err(NexusCatalogValidationError::EmptyTransition)
        ));
        let mut request = transition();
        request.dataspace_additions.push(dataspace(8, "eight"));
        assert!(request.validate_structure().is_err());
        let mut request = transition();
        request.dataspace_additions.push(dataspace(10, "nine"));
        assert!(request.validate_structure().is_err());
        let mut request = transition();
        request
            .lane_additions
            .push(request.lane_additions[0].clone());
        assert!(request.validate_structure().is_err());
        let mut request = transition();
        request.manifest_additions.push(manifest(7));
        assert!(request.validate_structure().is_err());
        let mut state = runtime();
        state.dataspaces.reverse();
        state.manifests.push(manifest(6));
        assert!(state.validate_structure().is_err());
    }
    #[test]
    fn additive_catalog_binds_identity_hashes_and_committee_geometry() {
        let mut addition = dataspace(9, "nine");
        addition.manifest_hash[0] = 10;
        assert!(addition.validate_structure().is_err());
        for f in [0, u32::MAX, 43] {
            let mut addition = dataspace(9, "nine");
            addition.descriptor.fault_tolerance = f;
            assert!(addition.validate_structure().is_err());
        }
        assert!(dataspace(0, "universal").validate_structure().is_err());
        let empty_digest = Hash::prehashed([0; Hash::LENGTH]);
        assert_ne!(empty_digest.as_ref(), &[0; Hash::LENGTH]);
        for field in [
            "expected_catalog_hash",
            "expected_incarnation_root",
            "expected_runtime_catalog_hash",
        ] {
            let mut request = transition();
            match field {
                "expected_catalog_hash" => request.expected_catalog_hash = empty_digest,
                "expected_incarnation_root" => request.expected_incarnation_root = empty_digest,
                _ => request.expected_runtime_catalog_hash = Some(empty_digest),
            }
            assert!(matches!(request.validate_structure(),
                Err(NexusCatalogValidationError::ZeroCommitment(actual)) if actual == field));
            assert!(matches!(request.clone().into_custom_parameter(),
                Err(NexusCatalogValidationError::ZeroCommitment(actual)) if actual == field));
            let unchecked =
                CustomParameter::new(NexusCatalogTransitionV1::parameter_id(), Json::new(request));
            assert!(
                matches!(NexusCatalogTransitionV1::from_custom_parameter(&unchecked),
                Err(NexusCatalogValidationError::ZeroCommitment(actual)) if actual == field)
            );
        }
        for field in ["baseline_dataspaces_hash", "baseline_manifests_hash"] {
            let mut state = runtime();
            match field {
                "baseline_dataspaces_hash" => state.baseline_dataspaces_hash = empty_digest,
                _ => state.baseline_manifests_hash = empty_digest,
            }
            assert!(matches!(state.validate_structure(),
                Err(NexusCatalogValidationError::ZeroCommitment(actual)) if actual == field));
            assert!(matches!(state.canonical_hash(),
                Err(NexusCatalogValidationError::ZeroCommitment(actual)) if actual == field));
            assert!(matches!(state.clone().into_custom_parameter(),
                Err(NexusCatalogValidationError::ZeroCommitment(actual)) if actual == field));
            let unchecked =
                CustomParameter::new(NexusRuntimeCatalogV1::parameter_id(), Json::new(state));
            assert!(
                matches!(NexusRuntimeCatalogV1::from_custom_parameter(&unchecked),
                Err(NexusCatalogValidationError::ZeroCommitment(actual)) if actual == field)
            );
        }
    }
    #[test]
    fn additive_catalog_bounds_manifest_sources_and_counts() {
        for value in [
            Json::new(()),
            Json::new(json::Value::Object(std::collections::BTreeMap::default())),
        ] {
            let entry = RuntimeLaneManifestV1 {
                lane_id: LaneId::new(7),
                manifest: value,
            };
            assert!(entry.validate_structure().is_err());
        }
        let value = json::Value::Object(
            [(
                "body".into(),
                json::Value::String("x".repeat(MAX_NEXUS_RUNTIME_MANIFEST_BYTES)),
            )]
            .into_iter()
            .collect(),
        );
        let entry = RuntimeLaneManifestV1 {
            lane_id: LaneId::new(7),
            manifest: Json::new(value),
        };
        assert!(entry.validate_structure().is_err());
        let mut request = transition();
        request.manifest_additions = vec![manifest(7); MAX_NEXUS_RUNTIME_CATALOG_ENTRIES + 1];
        assert!(request.validate_structure().is_err());
        let mut state = runtime();
        state.manifests = (0..5)
            .map(|id| RuntimeLaneManifestV1 {
                lane_id: LaneId::new(id),
                manifest: Json::new(json::Value::Object(
                    [("body".into(), json::Value::String("x".repeat(230 * 1024)))]
                        .into_iter()
                        .collect(),
                )),
            })
            .collect();
        assert!(state.into_custom_parameter().is_err());
    }
    #[test]
    fn runtime_root_binds_baselines_descriptors_and_exact_manifest_content() {
        let original = runtime();
        let root = original.canonical_hash().unwrap();
        let mut changed = original.clone();
        changed.baseline_manifests_hash = Hash::new(b"other");
        assert_ne!(changed.canonical_hash().unwrap(), root);
        let mut changed = original.clone();
        changed.dataspaces[0].descriptor.description = Some("changed".into());
        assert_ne!(changed.canonical_hash().unwrap(), root);
        let mut changed = original;
        changed.manifests[0].manifest = Json::new(json::Value::Object(
            [("lane".into(), json::Value::String("different".into()))]
                .into_iter()
                .collect(),
        ));
        assert_ne!(changed.canonical_hash().unwrap(), root);
        let mut baseline = DataSpaceCatalog::default();
        let first = dataspace_catalog_hash(&baseline);
        let mut entry = baseline.entries()[0].clone();
        entry.description = Some("description is also bound".into());
        baseline = DataSpaceCatalog::new(vec![entry]).unwrap();
        assert_ne!(dataspace_catalog_hash(&baseline), first);
    }

    #[test]
    fn borrowed_dataspace_hash_preserves_exact_native_sequence_commitment() {
        let catalog = DataSpaceCatalog::new(vec![
            DataSpaceMetadata::default(),
            DataSpaceMetadata {
                id: DataSpaceId::new(17),
                alias: "borrowed".into(),
                description: Some("exact retained description ".repeat(4096)),
                fault_tolerance: 3,
            },
        ])
        .unwrap();
        // This is the original canonical commitment, built independently from the
        // same retained descriptors. The production writer must preserve every byte.
        let original = Hash::new_from_chunks(&[
            b"iroha:nexus:dataspace-catalog:v1\0",
            &catalog.entries().to_vec().encode(),
        ]);
        assert_eq!(try_dataspace_catalog_hash(&catalog).unwrap(), original);
        let tiny =
            norito::core::DecodeBudgetContext::new(norito::DecodeLimits::new(0, 0, 0, 0, 32));
        assert_eq!(
            tiny.with(|| try_dataspace_catalog_hash(&catalog)).unwrap(),
            original,
            "hashing borrowed native metadata cannot require another owned graph or buffer"
        );
        assert_eq!(tiny.consumed_allocated_bytes(), 0);
    }

    #[test]
    fn dataspace_catalog_uniqueness_preserves_original_validation() {
        let first = DataSpaceMetadata {
            id: DataSpaceId::new(17),
            alias: "seventeen".into(),
            description: None,
            fault_tolerance: 1,
        };
        let mut second = first.clone();
        second.alias = "another".into();
        assert!(matches!(DataSpaceCatalog::new(vec![first.clone(), second]),
            Err(super::super::DataSpaceCatalogError::DuplicateId(id)) if id == first.id));
        let mut second = first.clone();
        second.id = DataSpaceId::new(18);
        assert!(matches!(DataSpaceCatalog::new(vec![first.clone(), second]),
            Err(super::super::DataSpaceCatalogError::DuplicateAlias(alias)) if alias == first.alias));
        let mut second = first.clone();
        second.id = DataSpaceId::new(18);
        second.alias = "eighteen".into();
        let catalog = DataSpaceCatalog::new(vec![second, first]).unwrap();
        assert_eq!(catalog.entries()[0].id, DataSpaceId::new(17));
        assert_eq!(catalog.entries()[1].id, DataSpaceId::new(18));
    }

    // Independent pre-streaming oracle. It deliberately allocates a complete native frame so the
    // original destination refusal assertions remain exercised alongside the optimized path.
    fn materialized_runtime_hash(
        value: &NexusRuntimeCatalogV1,
    ) -> Result<Hash, NexusCatalogValidationError> {
        value.validate_structure()?;
        let encoded = norito::core::to_bytes_bounded(value, 2 * MAX_NEXUS_RUNTIME_CATALOG_BYTES)
            .map_err(NexusCatalogValidationError::BinaryEncoding)?;
        Ok(Hash::new_from_chunks(&[RUNTIME_CATALOG_DOMAIN, &encoded]))
    }

    fn assert_original_refusal(
        context: &norito::core::DecodeBudgetContext,
        operation: impl FnOnce() -> NexusCatalogValidationError,
    ) -> norito::core::DecodeAttemptError {
        let mut captured = None;
        let attempt = context
            .with(|| {
                norito::core::classify_decode_attempt(|| {
                    let original = operation()
                        .into_decode_error()
                        .expect("actual decoder cause is available to its enclosing observer");
                    if let norito::Error::ScopedDecodeResource(scope) = &original {
                        captured = Some(scope.clone());
                    }
                    Err::<(), _>(original)
                })
            })
            .unwrap_err();
        assert_eq!(
            attempt.kind(),
            norito::core::DecodeAttemptErrorKind::EnclosingLimit
        );
        assert!(attempt.local_refusal().is_some());
        let norito::Error::ScopedDecodeResource(original) = attempt.error() else {
            panic!("the actual operation must retain its original scoped refusal");
        };
        assert_eq!(Some(original), captured.as_ref());
        attempt
    }

    #[test]
    fn catalog_decode_preserves_actual_work_allocation_and_observer_identity() {
        type Read = fn(&CustomParameter) -> Result<(), NexusCatalogValidationError>;
        let sources: [(CustomParameter, Read); 2] = [
            (transition().into_custom_parameter().unwrap(), |source| {
                NexusCatalogTransitionV1::from_custom_parameter(source).map(|_| ())
            }),
            (runtime().into_custom_parameter().unwrap(), |source| {
                NexusRuntimeCatalogV1::from_custom_parameter(source).map(|_| ())
            }),
        ];
        for (source, read) in sources {
            let original = source.clone();
            for work_limit in [true, false] {
                let limits = norito::DecodeLimits::new(
                    usize::MAX,
                    usize::MAX,
                    if work_limit { 1 } else { usize::MAX },
                    if work_limit { usize::MAX } else { 0 },
                    usize::MAX,
                );
                let context = norito::core::DecodeBudgetContext::new(limits);
                let error = assert_original_refusal(&context, || read(&source).unwrap_err());
                let norito::Error::ScopedDecodeResource(_) = error.error() else {
                    unreachable!("checked original scope");
                };
                if work_limit {
                    assert!(
                        matches!(error.error().decode_resource_error().expect("original resource fields"),
                        norito::core::DecodeResourceError::TotalElementsExceeded { attempted, limit: 1 }
                        if attempted > 1)
                    );
                } else {
                    assert!(
                        matches!(error.error().decode_resource_error().expect("original resource fields"),
                        norito::core::DecodeResourceError::TotalAllocationExceeded { attempted, limit: 0 }
                        if attempted > 0)
                    );
                }
                // Even the same cumulative context cannot authorize replay in a sibling attempt.
                let replay = context
                    .with(|| {
                        norito::core::classify_decode_attempt(|| Err::<(), _>(error.into_error()))
                    })
                    .unwrap_err();
                assert_eq!(replay.kind(), norito::core::DecodeAttemptErrorKind::Invalid);
                assert!(replay.local_refusal().is_none());
                assert_eq!(source, original);
                assert!(source.payload().ptr_eq(original.payload()));
                read(&source)
                    .expect("unchanged original source retries outside the exhausted context");
            }
        }
    }

    #[test]
    fn catalog_authoring_preserves_writer_and_inner_canonical_custody_refusals() {
        let value = json::Value::Null;
        let zero = norito::core::DecodeBudgetContext::new(norito::DecodeLimits::new(
            usize::MAX,
            usize::MAX,
            usize::MAX,
            0,
            usize::MAX,
        ));
        assert_original_refusal(&zero, || {
            let error = bounded_json(&value).unwrap_err();
            assert!(matches!(
                &error,
                NexusCatalogValidationError::JsonEncoding(
                    json::BoundedJsonError::ScopedDecodeResource(_)
                )
            ));
            error
        });
        assert_eq!(bounded_json(&value).unwrap(), "null");

        // Measure the exact prefix before canonical Json custody. The tested operation
        // remains the complete original custom_parameter path, including its parse.
        let measured = norito::core::DecodeBudgetContext::new(norito::DecodeLimits::new(
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
        ));
        measured.with(|| {
            let encoded = bounded_json(&value).unwrap();
            preflight(&encoded, MAX_NEXUS_RUNTIME_CATALOG_BYTES).unwrap();
            norito::with_decode_limits_scope(DECODE_LIMITS, || {
                assert!(matches!(
                    json::from_str::<json::Value>(&encoded).unwrap(),
                    json::Value::Null
                ));
            });
        });
        let prefix_bytes = usize::try_from(measured.consumed_allocated_bytes()).unwrap();
        assert!(prefix_bytes > 0);
        let context = norito::core::DecodeBudgetContext::new(norito::DecodeLimits::new(
            usize::MAX,
            usize::MAX,
            usize::MAX,
            prefix_bytes,
            usize::MAX,
        ));
        let id = NexusCatalogTransitionV1::parameter_id();
        let expected = custom_parameter(id.clone(), &value).unwrap();
        assert_original_refusal(&context, || {
            let error = custom_parameter(id.clone(), &value).unwrap_err();
            assert!(
                matches!(
                    &error,
                    NexusCatalogValidationError::Codec(norito::Error::ScopedDecodeResource(_))
                ),
                "the refusal must be from original canonical custody after the measured prefix"
            );
            error
        });
        assert_eq!(context.consumed_allocated_bytes(), prefix_bytes as u64);
        assert_eq!(custom_parameter(id, &value).unwrap(), expected);
    }

    #[test]
    fn catalog_root_hash_preserves_original_binary_destination_refusal() {
        let value = runtime();
        let original = value.clone();
        let expected = value.canonical_hash().unwrap();
        let measured = norito::core::DecodeBudgetContext::new(norito::DecodeLimits::new(
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
        ));
        measured.with(|| value.validate_structure()).unwrap();
        let prefix_bytes = usize::try_from(measured.consumed_allocated_bytes()).unwrap();
        let context = norito::core::DecodeBudgetContext::new(norito::DecodeLimits::new(
            usize::MAX,
            usize::MAX,
            usize::MAX,
            prefix_bytes,
            usize::MAX,
        ));
        assert_original_refusal(&context, || {
            let error = materialized_runtime_hash(&value).unwrap_err();
            assert!(matches!(
                &error,
                NexusCatalogValidationError::BinaryEncoding(
                    norito::core::BoundedEncodeError::Serialization(
                        norito::Error::ScopedDecodeResource(_)
                    )
                )
            ));
            error
        });
        assert_eq!(context.consumed_allocated_bytes(), prefix_bytes as u64);
        let streaming = norito::core::DecodeBudgetContext::new(norito::DecodeLimits::new(
            usize::MAX,
            usize::MAX,
            usize::MAX,
            prefix_bytes,
            usize::MAX,
        ));
        assert_eq!(streaming.with(|| value.canonical_hash()).unwrap(), expected);
        assert_eq!(streaming.consumed_allocated_bytes(), prefix_bytes as u64);
        assert_eq!(value, original);
        assert_eq!(value.canonical_hash().unwrap(), expected);
    }

    #[test]
    fn catalog_invalid_input_and_protocol_bounds_never_gain_local_admission() {
        let invalid = preflight("{", MAX_NEXUS_RUNTIME_CATALOG_BYTES).unwrap_err();
        assert!(matches!(invalid.into_decode_error(),
            Err(NexusCatalogValidationError::Preflight(error)) if error.syntax_kind().is_some()));
        let fixed_bound = preflight("null", 3).unwrap_err();
        assert!(matches!(fixed_bound.into_decode_error(),
            Err(NexusCatalogValidationError::Preflight(error)) if error.resource_kind().is_some()));
        let malformed =
            CustomParameter::new(NexusCatalogTransitionV1::parameter_id(), Json::new(()));
        let error = NexusCatalogTransitionV1::from_custom_parameter(&malformed).unwrap_err();
        assert!(matches!(
            error.into_decode_error(),
            Err(NexusCatalogValidationError::Json(_))
        ));
        let mut wrong_version = transition();
        wrong_version.version = 2;
        assert!(matches!(
            wrong_version
                .validate_structure()
                .unwrap_err()
                .into_decode_error(),
            Err(NexusCatalogValidationError::UnsupportedVersion(2))
        ));
        let too_large = "x".repeat(MAX_NEXUS_RUNTIME_CATALOG_BYTES);
        assert!(matches!(
            bounded_json(&too_large).unwrap_err().into_decode_error(),
            Err(NexusCatalogValidationError::JsonEncoding(
                json::BoundedJsonError::BodyTooLarge
            ))
        ));
        let frame = norito::core::to_bytes_bounded(&runtime(), 0).unwrap_err();
        assert!(matches!(
            NexusCatalogValidationError::BinaryEncoding(frame).into_decode_error(),
            Err(NexusCatalogValidationError::BinaryEncoding(
                norito::core::BoundedEncodeError::FrameTooLarge { .. }
            ))
        ));
        let reconstructed = NexusCatalogValidationError::Json(json::Error::DecodeResource(
            norito::core::DecodeResourceError::TotalElementsExceeded {
                attempted: 2,
                limit: 1,
            },
        ));
        let context = norito::core::DecodeBudgetContext::new(norito::DecodeLimits::new(
            usize::MAX,
            usize::MAX,
            1,
            usize::MAX,
            usize::MAX,
        ));
        let attempt = context
            .with(|| {
                norito::core::classify_decode_attempt(|| {
                    Err::<(), _>(reconstructed.into_decode_error().unwrap())
                })
            })
            .unwrap_err();
        assert_eq!(
            attempt.kind(),
            norito::core::DecodeAttemptErrorKind::Invalid
        );
        assert!(attempt.local_refusal().is_none());
    }

    #[test]
    fn catalog_second_read_exhausts_original_cumulative_element_owner() {
        let expected = transition();
        let source = expected.clone().into_custom_parameter().unwrap();
        let original = source.clone();
        let unlimited =
            norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, usize::MAX, usize::MAX);
        let (measured, usage) = norito::core::with_decode_limits_measured(unlimited, || {
            NexusCatalogTransitionV1::from_custom_parameter(&source)
        });
        assert_eq!(measured.unwrap(), Some(expected.clone()));
        let work = usage.total_elements();
        assert!(work > 0);
        let limit = work.checked_mul(2).unwrap() - 1;
        let limits =
            norito::DecodeLimits::new(usize::MAX, usize::MAX, limit, usize::MAX, usize::MAX);
        let context = norito::core::DecodeBudgetContext::new(limits);
        let first = context
            .with(|| NexusCatalogTransitionV1::from_custom_parameter(&source))
            .unwrap();
        assert_eq!(first, Some(expected));
        let error = assert_original_refusal(&context, || {
            NexusCatalogTransitionV1::from_custom_parameter(&source).unwrap_err()
        });
        let norito::Error::ScopedDecodeResource(_) = error.error() else {
            unreachable!("checked original scoped work refusal");
        };
        assert!(
            matches!(error.error().decode_resource_error().expect("original resource fields"),
            norito::core::DecodeResourceError::TotalElementsExceeded { attempted, limit: actual }
            if actual == limit as u64 && attempted > actual)
        );
        assert_eq!(source, original);
        assert!(source.payload().ptr_eq(original.payload()));
        let fresh = norito::core::DecodeBudgetContext::new(limits);
        assert_eq!(
            fresh
                .with(|| NexusCatalogTransitionV1::from_custom_parameter(&source))
                .unwrap(),
            first
        );
    }

    #[test]
    fn catalog_real_json_and_binary_destination_denials_preserve_bytes_and_retry() {
        use crate::amx_prepare_streaming_allocations::with_nth_exact_allocation_denial;
        use std::alloc::Layout;

        let value = json::Value::Null;
        let expected = bounded_json(&value).unwrap();
        let bytes = expected.len();
        let layout = Layout::array::<u8>(bytes).unwrap();
        let (attempt, denied) = with_nth_exact_allocation_denial(layout, 0, || {
            norito::core::classify_decode_attempt(|| {
                let error = bounded_json(&value).unwrap_err();
                assert!(matches!(&error, NexusCatalogValidationError::JsonEncoding(
                    json::BoundedJsonError::DecodeResource(norito::core::DecodeResourceError::AllocationFailed { bytes: actual })
                ) if *actual == bytes as u64));
                Err::<(), _>(error.into_decode_error().unwrap())
            })
        });
        assert!(denied, "actual counted JSON destination must be denied");
        let error = attempt.unwrap_err();
        assert_eq!(
            error.kind(),
            norito::core::DecodeAttemptErrorKind::Allocator
        );
        assert!(
            matches!(error.error(), norito::Error::AllocationFailed { bytes: actual } if *actual == bytes as u64)
        );
        assert_eq!(
            error.local_refusal().unwrap().kind(),
            norito::core::DecodeLocalRefusalKind::Allocator
        );
        assert_eq!(bounded_json(&value).unwrap(), expected);

        let mut value = runtime();
        value.dataspaces.clear();
        value.manifests.clear();
        let original = value.clone();
        let expected = value.canonical_hash().unwrap();
        let bytes = norito::core::encoded_frame_len(&value).unwrap();
        // Empty collections need no prefix allocation; the only prior owned
        // serialization is the differently sized bounded JSON validation output.
        assert_ne!(bounded_json(&value).unwrap().len(), bytes);
        let layout = Layout::array::<u8>(bytes).unwrap();
        let (attempt, denied) = with_nth_exact_allocation_denial(layout, 0, || {
            norito::core::classify_decode_attempt(|| {
                let error = materialized_runtime_hash(&value).unwrap_err();
                assert!(
                    matches!(&error, NexusCatalogValidationError::BinaryEncoding(
                    norito::core::BoundedEncodeError::AllocationFailed { bytes: actual }
                ) if *actual == bytes)
                );
                Err::<(), _>(error.into_decode_error().unwrap())
            })
        });
        assert!(denied, "actual canonical binary destination must be denied");
        let error = attempt.unwrap_err();
        assert_eq!(
            error.kind(),
            norito::core::DecodeAttemptErrorKind::Allocator
        );
        assert!(
            matches!(error.error(), norito::Error::AllocationFailed { bytes: actual } if *actual == bytes as u64)
        );
        assert_eq!(
            error.local_refusal().unwrap().kind(),
            norito::core::DecodeLocalRefusalKind::Allocator
        );
        let (streamed, denied) =
            with_nth_exact_allocation_denial(layout, 0, || value.canonical_hash());
        assert!(
            !denied,
            "streaming never requests the removed frame destination"
        );
        assert_eq!(streamed.unwrap(), expected);
        assert_eq!(value, original);
        assert_eq!(value.canonical_hash().unwrap(), expected);
    }

    #[test]
    fn streamed_catalog_frames_preserve_exact_bytes_layouts_and_bounds() {
        let mut empty = runtime();
        empty.dataspaces.clear();
        empty.manifests.clear();
        let mut populated = runtime();
        populated.dataspaces[0].descriptor.description = Some("quoted \"line\"\n\\雪 ".repeat(64));
        populated.dataspaces.push(dataspace(17, "seventeen"));
        populated.manifests.push(RuntimeLaneManifestV1 {
            lane_id: LaneId::new(18),
            manifest: Json::new(json::Value::Object(
                [(
                    "escaped".into(),
                    json::Value::String("\"\\\n雪".repeat(128)),
                )]
                .into_iter()
                .collect(),
            )),
        });
        for value in [empty, runtime(), populated] {
            for flags in [0, norito::core::header_flags::COMPACT_LEN] {
                let _layout = norito::core::DecodeFlagsGuard::enter(flags);
                let original = value.clone();
                let frame =
                    norito::core::to_bytes_bounded(&value, 2 * MAX_NEXUS_RUNTIME_CATALOG_BYTES)
                        .unwrap();
                let expected = Hash::new_from_chunks(&[RUNTIME_CATALOG_DOMAIN, &frame]);
                let mut streamed = Vec::new();
                write_catalog_frame(&value, &mut streamed, frame.len()).unwrap();
                assert_eq!(
                    streamed, frame,
                    "header, padding, payload and checksum must match"
                );
                assert_eq!(
                    streamed_catalog_hash(&value, frame.len()).unwrap(),
                    expected
                );
                assert_eq!(value.canonical_hash().unwrap(), expected);
                let mut refused_output = Vec::new();
                let error =
                    write_catalog_frame(&value, &mut refused_output, frame.len() - 1).unwrap_err();
                assert!(matches!(error,
                    norito::core::BoundedEncodeError::FrameTooLarge { encoded_bytes, max_bytes }
                    if encoded_bytes == frame.len() && max_bytes == frame.len() - 1));
                assert!(
                    refused_output.is_empty(),
                    "over-limit frame must not emit its header"
                );
                assert!(matches!(
                    streamed_catalog_hash(&value, frame.len() - 1).unwrap_err().into_decode_error(),
                    Err(NexusCatalogValidationError::BinaryEncoding(
                        norito::core::BoundedEncodeError::FrameTooLarge { encoded_bytes, max_bytes }
                    )) if encoded_bytes == frame.len() && max_bytes == frame.len() - 1
                ));
                assert_eq!(value, original);
            }
        }
    }

    #[test]
    fn catalog_streaming_removes_only_frame_destination_and_keeps_actual_refusals() {
        use crate::amx_prepare_streaming_allocations::{
            measured, with_nth_exact_allocation_denial,
        };
        use std::alloc::Layout;

        let value = runtime();
        let original = value.clone();
        let expected = materialized_runtime_hash(&value).unwrap();
        // Warm process state and prepare every oracle/input before observing native requests.
        assert_eq!(value.canonical_hash().unwrap(), expected);
        let (validated, validation_requests) = measured(|| value.validate_structure());
        validated.unwrap();
        let (streamed, hash_requests) = measured(|| value.canonical_hash());
        assert_eq!(streamed.unwrap(), expected);
        assert_eq!(
            hash_requests, validation_requests,
            "hashing adds no native allocation request beyond existing JSON validation"
        );
        let (materialized, materialized_requests) = measured(|| materialized_runtime_hash(&value));
        assert_eq!(materialized.unwrap(), expected);
        assert_eq!(
            materialized_requests.iter().sum::<usize>(),
            hash_requests.iter().sum::<usize>() + 1,
            "the independent oracle still allocates exactly the removed complete frame"
        );

        let prefix = bounded_json(&value).unwrap().len();
        let context = norito::core::DecodeBudgetContext::new(norito::DecodeLimits::new(
            usize::MAX,
            usize::MAX,
            usize::MAX,
            prefix - 1,
            usize::MAX,
        ));
        assert_original_refusal(&context, || {
            let error = value.canonical_hash().unwrap_err();
            assert!(matches!(
                &error,
                NexusCatalogValidationError::JsonEncoding(
                    json::BoundedJsonError::ScopedDecodeResource(_)
                )
            ));
            error
        });
        // The JSON destination still really exists. Refusing its allocator request must retain
        // the actual allocator cause, even though the later binary destination has disappeared.
        let (attempt, denied) =
            with_nth_exact_allocation_denial(Layout::array::<u8>(prefix).unwrap(), 0, || {
                norito::core::classify_decode_attempt(|| {
                    let error = value.canonical_hash().unwrap_err();
                    assert!(matches!(&error, NexusCatalogValidationError::JsonEncoding(
                        json::BoundedJsonError::DecodeResource(
                            norito::core::DecodeResourceError::AllocationFailed { bytes }
                        )) if *bytes == prefix as u64));
                    Err::<(), _>(error.into_decode_error().unwrap())
                })
            });
        assert!(denied);
        let error = attempt.unwrap_err();
        assert_eq!(
            error.kind(),
            norito::core::DecodeAttemptErrorKind::Allocator
        );
        assert!(
            matches!(error.error(), norito::Error::AllocationFailed { bytes }
            if *bytes == prefix as u64)
        );
        assert_eq!(
            error.local_refusal().unwrap().kind(),
            norito::core::DecodeLocalRefusalKind::Allocator
        );
        assert_eq!(value, original);
        assert_eq!(value.canonical_hash().unwrap(), expected);
    }

    #[test]
    fn catalog_streaming_keeps_original_serializer_refusals_and_rejects_drift() {
        use std::{cell::Cell, io::Write};

        #[derive(norito::NoritoSchema)]
        #[norito_schema(name = "iroha_data_model.test.CatalogAllocatingSerializer")]
        struct Allocating;
        impl norito::SerializePayload for Allocating {
            fn serialize(
                &self,
                output: &mut norito::core::Encoder<'_>,
            ) -> Result<(), norito::Error> {
                // Exercise a real fallible codec destination inside the producer. No error fields
                // or observer provenance are fabricated by this control.
                let bytes =
                    norito::core::to_bytes_bounded(&7_u8, 1024).map_err(|error| match error {
                        norito::core::BoundedEncodeError::Serialization(original) => original,
                        norito::core::BoundedEncodeError::AllocationFailed { bytes } => {
                            norito::Error::AllocationFailed {
                                bytes: bytes as u64,
                            }
                        }
                        norito::core::BoundedEncodeError::FrameTooLarge { .. } => {
                            unreachable!("single byte fixture fits the fixed frame bound")
                        }
                    })?;
                output.write_all(&bytes)?;
                Ok(())
            }
        }
        let context = norito::core::DecodeBudgetContext::new(norito::DecodeLimits::new(
            usize::MAX,
            usize::MAX,
            usize::MAX,
            0,
            usize::MAX,
        ));
        assert_original_refusal(&context, || {
            let error = streamed_catalog_hash(&Allocating, 4096).unwrap_err();
            assert!(matches!(
                &error,
                NexusCatalogValidationError::BinaryEncoding(
                    norito::core::BoundedEncodeError::Serialization(
                        norito::Error::ScopedDecodeResource(_)
                    )
                )
            ));
            error
        });
        let bytes = norito::core::to_bytes_bounded(&Allocating, 4096).unwrap();
        assert_eq!(
            streamed_catalog_hash(&Allocating, 4096).unwrap(),
            Hash::new_from_chunks(&[RUNTIME_CATALOG_DOMAIN, &bytes])
        );

        #[derive(norito::NoritoSchema)]
        #[norito_schema(name = "iroha_data_model.test.CatalogDriftingSerializer")]
        struct Drifting {
            visits: Cell<usize>,
            length_changes: bool,
        }
        impl norito::SerializePayload for Drifting {
            fn serialize(
                &self,
                output: &mut norito::core::Encoder<'_>,
            ) -> Result<(), norito::Error> {
                let visit = self.visits.get();
                self.visits.set(visit + 1);
                if self.length_changes && visit != 0 {
                    output.write_all(&[1, 2])?;
                } else {
                    output.write_all(&[u8::from(visit != 0)])?;
                }
                Ok(())
            }
        }
        for length_changes in [false, true] {
            let value = Drifting {
                visits: Cell::new(0),
                length_changes,
            };
            let error = streamed_catalog_hash(&value, 4096).unwrap_err();
            assert!(matches!(error, NexusCatalogValidationError::BinaryEncoding(
                norito::core::BoundedEncodeError::Serialization(ref original)
            ) if if length_changes { matches!(original, norito::Error::LengthMismatch) }
                else { matches!(original, norito::Error::ChecksumMismatch) }));
            assert_eq!(
                value.visits.get(),
                2,
                "the actual two passes must detect drift"
            );
        }
    }

    #[test]
    fn catalog_authoring_reuses_validated_json_without_changing_custody_or_bounds() {
        fn check<T: JsonSerialize + Clone>(
            value: T,
            validate: fn(&T) -> Result<(), NexusCatalogValidationError>,
            author: fn(T) -> Result<CustomParameter, NexusCatalogValidationError>,
            id: fn() -> CustomParameterId,
        ) {
            let unlimited = norito::DecodeLimits::new(
                usize::MAX,
                usize::MAX,
                usize::MAX,
                usize::MAX,
                usize::MAX,
            );
            let prior = norito::core::DecodeBudgetContext::new(unlimited);
            let previous = prior
                .with(|| {
                    validate(&value)?;
                    custom_parameter(id(), &value)
                })
                .unwrap();
            let destination_bytes = bounded_json(&value).unwrap().len();
            let next_source = value.clone();
            let current = norito::core::DecodeBudgetContext::new(unlimited);
            let next = current.with(|| author(next_source)).unwrap();
            assert_eq!(next, previous);
            assert_eq!(next.payload().get(), previous.payload().get());
            assert_eq!(
                prior.consumed_allocated_bytes(),
                current.consumed_allocated_bytes() + destination_bytes as u64,
                "only the duplicate validated JSON destination charge is removed"
            );
            let exact = usize::try_from(current.consumed_allocated_bytes()).unwrap();
            assert!(exact > 0);
            let source = value.clone();
            let fits = norito::core::DecodeBudgetContext::new(norito::DecodeLimits::new(
                usize::MAX,
                usize::MAX,
                usize::MAX,
                exact,
                usize::MAX,
            ));
            assert_eq!(fits.with(|| author(source)).unwrap(), previous);
            assert_eq!(fits.consumed_allocated_bytes(), exact as u64);
            let source = value.clone();
            let short = norito::core::DecodeBudgetContext::new(norito::DecodeLimits::new(
                usize::MAX,
                usize::MAX,
                usize::MAX,
                exact - 1,
                usize::MAX,
            ));
            assert_original_refusal(&short, || author(source).unwrap_err());
            assert_eq!(author(value).unwrap(), previous);
        }
        check(
            transition(),
            NexusCatalogTransitionV1::validate_structure,
            NexusCatalogTransitionV1::into_custom_parameter,
            NexusCatalogTransitionV1::parameter_id,
        );
        check(
            runtime(),
            NexusRuntimeCatalogV1::validate_structure,
            NexusRuntimeCatalogV1::into_custom_parameter,
            NexusRuntimeCatalogV1::parameter_id,
        );
    }

    // Exact prior wrapper and independent prior owner; used only as a regression oracle.
    fn materialized_validate_lanes(
        entries: &[LaneConfig],
    ) -> Result<(), NexusCatalogValidationError> {
        count(entries.len())?;
        if entries.windows(2).any(|pair| pair[0].id >= pair[1].id) {
            return Err(NexusCatalogValidationError::NonCanonicalOrder("lanes"));
        }
        for entry in entries {
            validate_alias(&entry.alias)?;
            if entry
                .description
                .as_ref()
                .is_some_and(|value| value.len() > MAX_DESCRIPTION_BYTES)
            {
                return Err(NexusCatalogValidationError::BoundExceeded(
                    "lane description bytes",
                ));
            }
        }
        if let Some(last) = entries.last() {
            let bound = last
                .id
                .as_u32()
                .checked_add(1)
                .and_then(NonZeroU32::new)
                .ok_or_else(|| {
                    NexusCatalogValidationError::InvalidLane(
                        "lane ID cannot form exclusive bound".into(),
                    )
                })?;
            super::super::tests::original_lane_catalog_constructor(bound, entries.to_vec())
                .map_err(|error| NexusCatalogValidationError::InvalidLane(error.to_string()))?;
        }
        Ok(())
    }

    #[test]
    fn borrowed_runtime_lanes_preserve_outer_limits_order_and_owner_errors() {
        let lane = transition().lane_additions.remove(0);
        let mut cases = vec![Vec::new(), vec![lane.clone()]];
        let mut oversized = vec![lane.clone(); MAX_NEXUS_RUNTIME_CATALOG_ENTRIES + 1];
        oversized[0].alias.clear();
        assert!(matches!(
            validate_lanes(&oversized),
            Err(NexusCatalogValidationError::BoundExceeded("entry count"))
        ));
        cases.push(oversized);
        let mut reversed = vec![lane.clone(), lane.clone()];
        reversed[0].alias.clear();
        assert!(matches!(
            validate_lanes(&reversed),
            Err(NexusCatalogValidationError::NonCanonicalOrder("lanes"))
        ));
        cases.push(reversed);
        for alias in [
            String::new(),
            " padded ".into(),
            "control\n".into(),
            "x".repeat(MAX_ALIAS_BYTES + 1),
        ] {
            let mut invalid = lane.clone();
            invalid.alias = alias;
            invalid.id = LaneId::new(u32::MAX);
            assert!(matches!(
                validate_lanes(std::slice::from_ref(&invalid)),
                Err(NexusCatalogValidationError::InvalidDataSpace(
                    "non-canonical alias"
                ))
            ));
            cases.push(vec![invalid]);
        }
        let mut description = lane.clone();
        description.description = Some("d".repeat(MAX_DESCRIPTION_BYTES + 1));
        description.id = LaneId::new(u32::MAX);
        assert!(matches!(
            validate_lanes(std::slice::from_ref(&description)),
            Err(NexusCatalogValidationError::BoundExceeded(
                "lane description bytes"
            ))
        ));
        cases.push(vec![description]);
        let mut overflow = lane.clone();
        overflow.id = LaneId::new(u32::MAX);
        assert!(matches!(validate_lanes(std::slice::from_ref(&overflow)),
            Err(NexusCatalogValidationError::InvalidLane(message))
            if message == "lane ID cannot form exclusive bound"));
        cases.push(vec![overflow]);
        let mut duplicate = lane.clone();
        duplicate.id = LaneId::new(lane.id.as_u32() + 1);
        assert!(matches!(validate_lanes(&[lane.clone(), duplicate.clone()]),
            Err(NexusCatalogValidationError::InvalidLane(message))
            if message == super::super::LaneCatalogError::DuplicateLaneAlias(lane.alias.clone()).to_string()));
        cases.push(vec![lane.clone(), duplicate.clone()]);
        duplicate.alias = "distinct".into();
        duplicate.visibility = super::super::LaneVisibility::Restricted;
        assert!(matches!(validate_lanes(&[lane.clone(), duplicate.clone()]),
            Err(NexusCatalogValidationError::InvalidLane(message))
            if message == (super::super::LaneCatalogError::MixedDataspaceVisibility { dataspace: lane.dataspace_id }).to_string()));
        cases.push(vec![lane.clone(), duplicate]);
        let mut retired = lane.clone();
        retired
            .metadata
            .insert("da_shard_id".into(), "retired".into());
        assert!(matches!(validate_lanes(std::slice::from_ref(&retired)),
            Err(NexusCatalogValidationError::InvalidLane(message))
            if message == super::super::LaneCatalogError::RetiredShardIdMetadata(lane.id).to_string()));
        cases.push(vec![retired]);
        for entries in cases {
            let original = entries.clone();
            let expected =
                materialized_validate_lanes(&entries).map_err(|error| format!("{error:?}"));
            let actual = validate_lanes(&entries).map_err(|error| format!("{error:?}"));
            assert_eq!(
                actual, expected,
                "the wrapper must retain its exact variant and diagnostic"
            );
            assert_eq!(entries, original);
        }
        // Empty additions remain valid here; the complete owner constructor still rejects empty.
        validate_lanes(&[]).unwrap();
        assert_eq!(
            LaneCatalog::validate_borrowed(NonZeroU32::MIN, &[]),
            Err(super::super::LaneCatalogError::EmptyCatalog)
        );
    }

    #[test]
    fn borrowed_runtime_lanes_eliminate_graph_copy_at_exact_maximum_shape() {
        use crate::amx_prepare_streaming_allocations::measured;
        let lanes: Vec<_> = (0..MAX_NEXUS_RUNTIME_CATALOG_ENTRIES)
            .map(|index| LaneConfig {
                id: LaneId::new((index * 3 + 7) as u32),
                alias: format!("{}-{index:04}", "a".repeat(MAX_ALIAS_BYTES - 5)),
                description: Some("d".repeat(MAX_DESCRIPTION_BYTES)),
                dataspace_id: DataSpaceId::new(9),
                metadata: [("operator_note".into(), "retained input metadata".repeat(32))]
                    .into_iter()
                    .collect(),
                ..LaneConfig::default()
            })
            .collect();
        let original = lanes.clone();
        let aliases: Vec<_> = lanes.iter().map(|lane| lane.alias.as_ptr()).collect();
        let descriptions: Vec<_> = lanes
            .iter()
            .map(|lane| lane.description.as_ref().unwrap().as_ptr())
            .collect();
        validate_lanes(&lanes).unwrap();
        let (old, old_requests) = measured(|| materialized_validate_lanes(&lanes));
        old.unwrap();
        assert!(
            old_requests.iter().sum::<usize>() > 0,
            "the independent old wrapper must exercise real graph and owner allocations"
        );
        let (new, requests) = measured(|| validate_lanes(&lanes));
        new.unwrap();
        assert_eq!(
            requests, [0; 3],
            "no alloc, alloc_zeroed or realloc on the borrowed path"
        );
        assert_eq!(lanes, original);
        assert_eq!(
            lanes
                .iter()
                .map(|lane| lane.alias.as_ptr())
                .collect::<Vec<_>>(),
            aliases
        );
        assert_eq!(
            lanes
                .iter()
                .map(|lane| lane.description.as_ref().unwrap().as_ptr())
                .collect::<Vec<_>>(),
            descriptions
        );
        let mut over = lanes.clone();
        let mut last = lanes.last().unwrap().clone();
        last.id = LaneId::new(last.id.as_u32() + 1);
        over.push(last);
        assert!(matches!(
            validate_lanes(&over),
            Err(NexusCatalogValidationError::BoundExceeded("entry count"))
        ));
    }
}
