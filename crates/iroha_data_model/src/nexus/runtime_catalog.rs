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
use std::{collections::BTreeSet, num::NonZeroU32};
use thiserror::Error;

/// Maximum entries of each kind in a transition or cumulative runtime overlay.
pub const MAX_NEXUS_RUNTIME_CATALOG_ENTRIES: usize = MAX_ACTIVE_EXECUTION_LANES;
/// Maximum canonical source bytes in one inline native lane manifest.
pub const MAX_NEXUS_RUNTIME_MANIFEST_BYTES: usize = 256 * 1024;
/// Maximum complete custom-parameter JSON, including its cumulative manifest sources.
pub const MAX_NEXUS_RUNTIME_CATALOG_BYTES: usize = MAX_JSON_BYTES;
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
    /// Canonical, bounded native `ManifestFile` JSON. Core validates its schema and authority.
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
#[derive(Debug, Clone, PartialEq, Eq, Error)]
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
    /// Strict codec validation or bounded encoding failed.
    #[error("Nexus runtime catalog codec: {0}")]
    Codec(String),
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
        preflight(&bounded_json(self)?, MAX_NEXUS_RUNTIME_CATALOG_BYTES)
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
        self.validate_structure()?;
        custom_parameter(Self::parameter_id(), &self)
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
    /// Protected state key; a direct user SetParameter for this key must be rejected by Core.
    pub const PARAMETER_ID_STR: &'static str = "nexus_runtime_catalog_v1";

    /// Validate the canonical cumulative overlay; this does not authenticate state origin.
    ///
    /// # Errors
    /// Rejects unsupported versions, zero baselines, non-canonical entries and codec bounds.
    pub fn validate_structure(&self) -> Result<(), NexusCatalogValidationError> {
        version(self.version)?;
        nonzero(self.baseline_dataspaces_hash, "baseline_dataspaces_hash")?;
        nonzero(self.baseline_manifests_hash, "baseline_manifests_hash")?;
        validate_dataspaces(&self.dataspaces)?;
        validate_manifests(&self.manifests)?;
        preflight(&bounded_json(self)?, MAX_NEXUS_RUNTIME_CATALOG_BYTES)
    }

    /// Compute the canonical domain-separated root of this complete validated overlay.
    ///
    /// # Errors
    /// Returns structural or bounded native encoding failures.
    pub fn canonical_hash(&self) -> Result<Hash, NexusCatalogValidationError> {
        self.validate_structure()?;
        let encoded = norito::core::to_bytes_bounded(self, 2 * MAX_NEXUS_RUNTIME_CATALOG_BYTES)
            .map_err(|error| NexusCatalogValidationError::Codec(error.to_string()))?;
        Ok(Hash::new_from_chunks(&[
            b"iroha:nexus:runtime-catalog:v1\0",
            &encoded,
        ]))
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
        self.validate_structure()?;
        custom_parameter(Self::parameter_id(), &self)
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
    let encoded = catalog.entries().to_vec().encode();
    Hash::new_from_chunks(&[b"iroha:nexus:dataspace-catalog:v1\0", &encoded])
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
    let mut aliases = BTreeSet::new();
    for entry in entries {
        entry.validate_structure()?;
        if !aliases.insert(entry.descriptor.alias.as_str()) {
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
        LaneCatalog::new(bound, entries.to_vec())
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
        .map_err(|error| NexusCatalogValidationError::Codec(error.to_string()))
}
fn bounded_json<T: JsonSerialize>(value: &T) -> Result<String, NexusCatalogValidationError> {
    json::to_json_bounded(value, MAX_NEXUS_RUNTIME_CATALOG_BYTES)
        .map_err(|error| NexusCatalogValidationError::Codec(error.to_string()))
}
fn custom_parameter<T: JsonSerialize>(
    id: CustomParameterId,
    value: &T,
) -> Result<CustomParameter, NexusCatalogValidationError> {
    let encoded = bounded_json(value)?;
    preflight(&encoded, MAX_NEXUS_RUNTIME_CATALOG_BYTES)?;
    let payload = norito::with_decode_limits_scope(DECODE_LIMITS, || {
        let value: json::Value = json::from_str(&encoded)?;
        Json::from_norito_value_ref(&value).map_err(|error| json::Error::Message(error.to_string()))
    })
    .map_err(|error| NexusCatalogValidationError::Codec(error.to_string()))?;
    Ok(CustomParameter::new(id, payload))
}
fn decode_payload<T: JsonDeserializeOwned>(
    payload: &Json,
) -> Result<T, NexusCatalogValidationError> {
    preflight(payload.get(), MAX_NEXUS_RUNTIME_CATALOG_BYTES)?;
    norito::with_decode_limits_scope(DECODE_LIMITS, || json::from_str(payload.get()))
        .map_err(|error| NexusCatalogValidationError::Codec(error.to_string()))
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
        assert_eq!(
            request.validate_structure(),
            Err(NexusCatalogValidationError::EmptyTransition)
        );
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
            let expected = NexusCatalogValidationError::ZeroCommitment(field);
            assert_eq!(request.validate_structure(), Err(expected.clone()));
            assert_eq!(
                request.clone().into_custom_parameter(),
                Err(expected.clone())
            );
            let unchecked =
                CustomParameter::new(NexusCatalogTransitionV1::parameter_id(), Json::new(request));
            assert_eq!(
                NexusCatalogTransitionV1::from_custom_parameter(&unchecked),
                Err(expected)
            );
        }
        for field in ["baseline_dataspaces_hash", "baseline_manifests_hash"] {
            let mut state = runtime();
            match field {
                "baseline_dataspaces_hash" => state.baseline_dataspaces_hash = empty_digest,
                _ => state.baseline_manifests_hash = empty_digest,
            }
            let expected = NexusCatalogValidationError::ZeroCommitment(field);
            assert_eq!(state.validate_structure(), Err(expected.clone()));
            assert_eq!(state.canonical_hash(), Err(expected.clone()));
            assert_eq!(state.clone().into_custom_parameter(), Err(expected.clone()));
            let unchecked =
                CustomParameter::new(NexusRuntimeCatalogV1::parameter_id(), Json::new(state));
            assert_eq!(
                NexusRuntimeCatalogV1::from_custom_parameter(&unchecked),
                Err(expected)
            );
        }
    }
    #[test]
    fn additive_catalog_bounds_manifest_sources_and_counts() {
        for value in [
            Json::new(()),
            Json::new(json::Value::Object(Default::default())),
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
}
