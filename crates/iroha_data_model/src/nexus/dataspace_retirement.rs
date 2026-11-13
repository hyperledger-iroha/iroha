//! Owner-signed retirement of a catalogued physical dataspace from parent execution.
//!
//! Retirement removes effective execution authority only. The physical catalog keeps the
//! original dataspace descriptor and every lane descriptor, incarnation and storage binding;
//! native block history is never erased and a retired lane identity is never reused. A
//! retirement record is the sole authority that later lets the exact retiring SNS owner and
//! ownership generation re-admit the same identity as an independent private root.
//!
//! Both payloads travel as reserved `SetParameter` custom parameters with canonical JSON
//! payloads, so neither changes an instruction, query or wire-schema layout:
//!
//! - [`DataSpaceRetirementRequestV1`] (`nexus_dataspace_retirement_v1`) is a transient,
//!   owner-signed command. Core validates and consumes it; it is never stored.
//! - [`DataSpaceRetirementsV1`] (`nexus_dataspace_retirements_v1`) is the protected cumulative
//!   record. Only the typed request handler writes it; a direct `SetParameter` is refused.

use super::{
    NexusCatalogValidationError,
    runtime_catalog::{
        MAX_NEXUS_RUNTIME_CATALOG_BYTES, bounded_json, count, custom_parameter_from_validated_json,
        decode_payload, preflight, validate_alias, version,
    },
};
use crate::{
    DeriveJsonDeserialize, DeriveJsonSerialize,
    account::AccountId,
    parameter::{CustomParameter, CustomParameterId},
};
use iroha_model_base::topology::{DataSpaceId, LaneId};

/// Owner-signed request to retire one catalogued physical dataspace from parent execution.
///
/// The transaction authority must equal [`Self::owner`], the active SNS owner of the canonical
/// dataspace alias at [`Self::expected_ownership_generation`]. [`Self::lanes`] must name exactly
/// every physical catalog lane of the dataspace. Core additionally requires the native Sumeragi
/// lanes of the dataspace to be closed and removed (`sumeragi_lane_policy`) beforehand.
#[derive(Debug, Clone, PartialEq, Eq, DeriveJsonSerialize, DeriveJsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct DataSpaceRetirementRequestV1 {
    /// Layout version, exactly [`Self::VERSION`].
    pub version: u8,
    /// Catalogued physical dataspace whose parent execution authority is retired.
    pub dataspace_id: DataSpaceId,
    /// Exact canonical dataspace alias; equals the catalog descriptor and the paid SNS label.
    pub alias: String,
    /// Exact current SNS owner, also required to sign the transaction.
    pub owner: AccountId,
    /// Current SNS ownership generation of the alias.
    pub expected_ownership_generation: u64,
    /// Every physical lane of the dataspace, strictly ascending. Lane `0` is never retired.
    pub lanes: Vec<LaneId>,
}

/// One committed, owner-bound retirement; retained across replay and snapshots.
#[derive(Debug, Clone, PartialEq, Eq, DeriveJsonSerialize, DeriveJsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct RetiredDataSpaceV1 {
    /// Retired physical dataspace identity; its catalog descriptor stays in place.
    pub dataspace_id: DataSpaceId,
    /// Canonical alias at retirement.
    pub alias: String,
    /// SNS owner that signed the retirement.
    pub owner: AccountId,
    /// SNS ownership generation at retirement; a later re-admission must present it.
    pub ownership_generation: u64,
    /// Physical lanes whose execution authority was retired, strictly ascending.
    pub lanes: Vec<LaneId>,
    /// Height of the accepted retirement block, supplied solely by native execution.
    pub retirement_height: u64,
}

/// Protected cumulative retirement record, persisted as an ordinary World custom parameter.
#[derive(Debug, Clone, PartialEq, Eq, DeriveJsonSerialize, DeriveJsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct DataSpaceRetirementsV1 {
    /// Layout version, exactly [`Self::VERSION`].
    pub version: u8,
    /// Every retirement, strictly ordered by dataspace identity.
    pub retired: Vec<RetiredDataSpaceV1>,
}

fn validate_retired_identity(
    dataspace_id: DataSpaceId,
    alias: &str,
    ownership_generation: u64,
    lanes: &[LaneId],
) -> Result<(), NexusCatalogValidationError> {
    if dataspace_id == DataSpaceId::UNIVERSAL {
        return Err(NexusCatalogValidationError::InvalidDataSpace(
            "the universal dataspace cannot be retired",
        ));
    }
    if ownership_generation == 0 {
        return Err(NexusCatalogValidationError::InvalidDataSpace(
            "retirement requires a positive SNS ownership generation",
        ));
    }
    validate_alias(alias)?;
    if alias == "universal" {
        return Err(NexusCatalogValidationError::InvalidDataSpace(
            "reserved universal alias",
        ));
    }
    count(lanes.len())?;
    if lanes.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(NexusCatalogValidationError::NonCanonicalOrder(
            "retired lanes",
        ));
    }
    if lanes.contains(&LaneId::SINGLE) {
        return Err(NexusCatalogValidationError::InvalidLane(
            "lane 0 is never retired".into(),
        ));
    }
    Ok(())
}

impl DataSpaceRetirementRequestV1 {
    /// Supported layout version.
    pub const VERSION: u8 = 1;
    /// Reserved request parameter consumed by the native retirement handler.
    pub const PARAMETER_ID_STR: &'static str = "nexus_dataspace_retirement_v1";

    /// Validate the structural bounds; Core independently authenticates ownership and closure.
    ///
    /// # Errors
    /// Rejects an unsupported version, the universal dataspace or alias, a zero generation,
    /// a non-canonical alias, and non-canonical or primary lanes.
    pub fn validate_structure(&self) -> Result<(), NexusCatalogValidationError> {
        self.validated_json().map(|_| ())
    }

    fn validated_json(&self) -> Result<String, NexusCatalogValidationError> {
        version(self.version)?;
        validate_retired_identity(
            self.dataspace_id,
            &self.alias,
            self.expected_ownership_generation,
            &self.lanes,
        )?;
        let encoded = bounded_json(self)?;
        preflight(&encoded, MAX_NEXUS_RUNTIME_CATALOG_BYTES)?;
        Ok(encoded)
    }

    /// Return the exact request custom-parameter ID.
    #[must_use]
    pub fn parameter_id() -> CustomParameterId {
        Self::PARAMETER_ID_STR
            .parse()
            .expect("valid dataspace retirement request parameter ID")
    }

    /// Encode a structurally validated request without signing or executing it.
    ///
    /// # Errors
    /// Returns structural or bounded codec failures.
    pub fn into_custom_parameter(self) -> Result<CustomParameter, NexusCatalogValidationError> {
        let encoded = self.validated_json()?;
        custom_parameter_from_validated_json(Self::parameter_id(), &encoded)
    }

    /// Decode only this reserved parameter and validate its complete structure.
    ///
    /// # Errors
    /// A matching malformed or unsupported payload fails; an unrelated ID returns `None`.
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

impl RetiredDataSpaceV1 {
    /// Validate the retained structure, including its post-genesis height.
    ///
    /// # Errors
    /// Rejects a malformed identity or a genesis/pre-genesis retirement height.
    pub fn validate_structure(&self) -> Result<(), NexusCatalogValidationError> {
        validate_retired_identity(
            self.dataspace_id,
            &self.alias,
            self.ownership_generation,
            &self.lanes,
        )?;
        if self.retirement_height <= 1 {
            return Err(NexusCatalogValidationError::InvalidDataSpace(
                "retirement must be post-genesis",
            ));
        }
        Ok(())
    }

    /// Whether `lane` is one of this record's retired physical lanes.
    #[must_use]
    pub fn retires_lane(&self, lane: LaneId) -> bool {
        self.lanes.binary_search(&lane).is_ok()
    }
}

impl DataSpaceRetirementsV1 {
    /// Supported layout version.
    pub const VERSION: u8 = 1;
    /// Protected state key; a direct user `SetParameter` for this key must be rejected by Core.
    pub const PARAMETER_ID_STR: &'static str = "nexus_dataspace_retirements_v1";

    /// Validate canonical order, unique aliases and disjoint lanes of every record.
    ///
    /// # Errors
    /// Rejects an unsupported version, malformed records, duplicates and codec bounds.
    pub fn validate_structure(&self) -> Result<(), NexusCatalogValidationError> {
        self.validated_json().map(|_| ())
    }

    fn validated_json(&self) -> Result<String, NexusCatalogValidationError> {
        version(self.version)?;
        count(self.retired.len())?;
        if self
            .retired
            .windows(2)
            .any(|pair| pair[0].dataspace_id >= pair[1].dataspace_id)
        {
            return Err(NexusCatalogValidationError::NonCanonicalOrder(
                "retired dataspaces",
            ));
        }
        for (index, record) in self.retired.iter().enumerate() {
            record.validate_structure()?;
            for previous in &self.retired[..index] {
                if previous.alias == record.alias {
                    return Err(NexusCatalogValidationError::DuplicateAlias(
                        record.alias.clone(),
                    ));
                }
                if record.lanes.iter().any(|lane| previous.retires_lane(*lane)) {
                    return Err(NexusCatalogValidationError::InvalidLane(
                        "a physical lane is retired by two dataspaces".into(),
                    ));
                }
            }
        }
        let encoded = bounded_json(self)?;
        preflight(&encoded, MAX_NEXUS_RUNTIME_CATALOG_BYTES)?;
        Ok(encoded)
    }

    /// The retirement of `dataspace_id`, if any.
    #[must_use]
    pub fn get(&self, dataspace_id: DataSpaceId) -> Option<&RetiredDataSpaceV1> {
        self.retired
            .binary_search_by_key(&dataspace_id, |record| record.dataspace_id)
            .ok()
            .map(|index| &self.retired[index])
    }

    /// Whether `dataspace_id` is retired from parent execution.
    #[must_use]
    pub fn is_retired(&self, dataspace_id: DataSpaceId) -> bool {
        self.get(dataspace_id).is_some()
    }

    /// The retirement that owns physical lane `lane`, if any.
    #[must_use]
    pub fn lane_owner(&self, lane: LaneId) -> Option<&RetiredDataSpaceV1> {
        self.retired.iter().find(|record| record.retires_lane(lane))
    }

    /// Return this record extended by one new, canonically placed retirement.
    ///
    /// # Errors
    /// Rejects an already retired identity, alias or lane, or an invalid successor.
    pub fn with_retirement(
        &self,
        record: RetiredDataSpaceV1,
    ) -> Result<Self, NexusCatalogValidationError> {
        if self.is_retired(record.dataspace_id) {
            return Err(NexusCatalogValidationError::InvalidDataSpace(
                "dataspace is already retired",
            ));
        }
        let mut next = self.clone();
        let index = next
            .retired
            .partition_point(|existing| existing.dataspace_id < record.dataspace_id);
        next.retired.insert(index, record);
        next.validate_structure()?;
        Ok(next)
    }

    /// Return the exact protected record custom-parameter ID.
    #[must_use]
    pub fn parameter_id() -> CustomParameterId {
        Self::PARAMETER_ID_STR
            .parse()
            .expect("valid dataspace retirement record parameter ID")
    }

    /// Encode validated protected state for use by the trusted retirement handler.
    ///
    /// # Errors
    /// Returns structural or bounded codec failures. This method grants no write authority.
    pub fn into_custom_parameter(self) -> Result<CustomParameter, NexusCatalogValidationError> {
        let encoded = self.validated_json()?;
        custom_parameter_from_validated_json(Self::parameter_id(), &encoded)
    }

    /// Decode and validate this protected parameter without authenticating its origin.
    ///
    /// # Errors
    /// A matching malformed or unsupported payload fails; an unrelated ID returns `None`.
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

impl Default for DataSpaceRetirementsV1 {
    fn default() -> Self {
        Self {
            version: Self::VERSION,
            retired: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use norito::json;

    /// Canonical JSON custody for authored test text (derive field order is not canonical).
    fn canonical(text: &str) -> iroha_primitives::json::Json {
        let value: json::Value = json::from_str(text).unwrap();
        iroha_primitives::json::Json::from_norito_value_ref(&value).unwrap()
    }

    fn owner() -> AccountId {
        AccountId::new(
            iroha_crypto::KeyPair::from_seed(vec![7; 32], iroha_crypto::Algorithm::Ed25519)
                .public_key()
                .clone(),
        )
    }

    fn request() -> DataSpaceRetirementRequestV1 {
        DataSpaceRetirementRequestV1 {
            version: 1,
            dataspace_id: DataSpaceId::new(6_983_892_400_042_691_068),
            alias: "cbsi".into(),
            owner: owner(),
            expected_ownership_generation: 1,
            lanes: vec![LaneId::new(6)],
        }
    }

    fn record(id: u64, alias: &str, lanes: &[u32]) -> RetiredDataSpaceV1 {
        RetiredDataSpaceV1 {
            dataspace_id: DataSpaceId::new(id),
            alias: alias.into(),
            owner: owner(),
            ownership_generation: 1,
            lanes: lanes.iter().copied().map(LaneId::new).collect(),
            retirement_height: 12_700,
        }
    }

    #[test]
    fn taira_tenant_catalog_ids_are_the_hash_derived_sns_ids() {
        // Live Taira catalog ids of the four restricted tenant dataspaces and BPNG. A private
        // root may only claim `DataSpaceId::from_hash(selector.name_hash())`.
        for (alias, id) in [
            ("cbsi", 6_983_892_400_042_691_068_u64),
            ("dpn", 9_884_542_487_407_331_249),
            ("is2", 9_700_280_040_122_529_948),
            ("is", 6_647_857_470_246_403_404),
            ("bpng", 8_648_377_547_929_788_715),
        ] {
            assert_eq!(
                crate::sns::dataspace_id_for_alias(alias),
                Some(DataSpaceId::new(id)),
                "{alias}"
            );
        }
    }

    #[test]
    fn request_roundtrips_through_its_reserved_parameter_only() {
        let request = request();
        let custom = request.clone().into_custom_parameter().unwrap();
        assert_eq!(custom.id().name().as_ref(), "nexus_dataspace_retirement_v1");
        assert_eq!(
            DataSpaceRetirementRequestV1::from_custom_parameter(&custom).unwrap(),
            Some(request.clone())
        );
        assert!(
            DataSpaceRetirementsV1::from_custom_parameter(&custom)
                .unwrap()
                .is_none()
        );
        // The documented JSON shape: decimal dataspace id, string owner, numeric lanes.
        let value: json::Value = json::from_str(custom.payload().get()).unwrap();
        let object = value.as_object().unwrap();
        let mut keys: Vec<_> = object.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "alias",
                "dataspace_id",
                "expected_ownership_generation",
                "lanes",
                "owner",
                "version"
            ]
        );
        assert_eq!(
            object.get("dataspace_id").and_then(json::Value::as_u64),
            Some(6_983_892_400_042_691_068)
        );
        let lanes: Vec<_> = object
            .get("lanes")
            .and_then(json::Value::as_array)
            .unwrap()
            .iter()
            .map(json::Value::as_u64)
            .collect();
        assert_eq!(lanes, [Some(6)]);
        assert!(object.get("owner").unwrap().is_string());
    }

    #[test]
    fn documented_set_parameter_json_decodes_to_the_request() {
        // The runbook's `iroha parameter set` stdin shape for Taira's `cbsi` (lane 6).
        let owner_json = json::to_json(&owner()).unwrap();
        let text = format!(
            r#"{{"Custom":{{"id":"nexus_dataspace_retirement_v1","payload":{{"version":1,"dataspace_id":6983892400042691068,"alias":"cbsi","owner":{owner_json},"expected_ownership_generation":1,"lanes":[6]}}}}}}"#
        );
        let crate::parameter::Parameter::Custom(custom) =
            json::from_str::<crate::parameter::Parameter>(&text).unwrap()
        else {
            panic!("documented shape must decode as a custom parameter");
        };
        assert_eq!(
            DataSpaceRetirementRequestV1::from_custom_parameter(&custom).unwrap(),
            Some(request())
        );
        let record = DataSpaceRetirementsV1::default()
            .with_retirement(RetiredDataSpaceV1 {
                dataspace_id: request().dataspace_id,
                alias: "cbsi".into(),
                owner: owner(),
                ownership_generation: 1,
                lanes: vec![LaneId::new(6)],
                retirement_height: 12_700,
            })
            .unwrap()
            .into_custom_parameter()
            .unwrap();
        let expected = format!(
            r#"{{"retired":[{{"alias":"cbsi","dataspace_id":6983892400042691068,"lanes":[6],"owner":{owner_json},"ownership_generation":1,"retirement_height":12700}}],"version":1}}"#
        );
        assert_eq!(record.payload().get(), expected.as_str());
    }

    #[test]
    fn request_structure_refuses_universal_zero_generation_and_noncanonical_lanes() {
        request().validate_structure().unwrap();
        for invalid in [
            DataSpaceRetirementRequestV1 {
                version: 2,
                ..request()
            },
            DataSpaceRetirementRequestV1 {
                dataspace_id: DataSpaceId::UNIVERSAL,
                ..request()
            },
            DataSpaceRetirementRequestV1 {
                alias: "universal".into(),
                ..request()
            },
            DataSpaceRetirementRequestV1 {
                alias: " cbsi".into(),
                ..request()
            },
            DataSpaceRetirementRequestV1 {
                expected_ownership_generation: 0,
                ..request()
            },
            DataSpaceRetirementRequestV1 {
                lanes: vec![LaneId::new(6), LaneId::new(6)],
                ..request()
            },
            DataSpaceRetirementRequestV1 {
                lanes: vec![LaneId::new(7), LaneId::new(6)],
                ..request()
            },
            DataSpaceRetirementRequestV1 {
                lanes: vec![LaneId::SINGLE, LaneId::new(6)],
                ..request()
            },
        ] {
            assert!(invalid.validate_structure().is_err(), "{invalid:?}");
            assert!(invalid.into_custom_parameter().is_err());
        }
        let text = json::to_json(&request()).unwrap();
        let unknown = text.replacen('{', "{\"unknown\":0,", 1);
        let custom = CustomParameter::new(
            DataSpaceRetirementRequestV1::parameter_id(),
            canonical(&unknown),
        );
        assert!(DataSpaceRetirementRequestV1::from_custom_parameter(&custom).is_err());
        let mut value: json::Value = json::from_str(&text).unwrap();
        value.as_object_mut().unwrap().remove("lanes");
        assert!(json::from_value::<DataSpaceRetirementRequestV1>(value).is_err());
    }

    #[test]
    fn record_is_canonical_append_only_and_lane_disjoint() {
        let empty = DataSpaceRetirementsV1::default();
        empty.validate_structure().unwrap();
        let one = empty.with_retirement(record(9, "nine", &[9])).unwrap();
        let two = one.with_retirement(record(3, "three", &[3, 4])).unwrap();
        assert_eq!(
            two.retired
                .iter()
                .map(|record| record.dataspace_id)
                .collect::<Vec<_>>(),
            [DataSpaceId::new(3), DataSpaceId::new(9)]
        );
        assert!(two.is_retired(DataSpaceId::new(3)));
        assert!(!two.is_retired(DataSpaceId::new(4)));
        assert_eq!(
            two.lane_owner(LaneId::new(4))
                .map(|record| record.dataspace_id),
            Some(DataSpaceId::new(3))
        );
        assert!(two.lane_owner(LaneId::new(5)).is_none());
        for duplicate in [
            record(9, "other", &[10]),
            record(10, "nine", &[10]),
            record(10, "ten", &[4]),
        ] {
            assert!(two.with_retirement(duplicate).is_err());
        }
        assert!(
            empty
                .with_retirement(RetiredDataSpaceV1 {
                    retirement_height: 1,
                    ..record(9, "nine", &[9])
                })
                .is_err()
        );
        let custom = two.clone().into_custom_parameter().unwrap();
        assert_eq!(
            DataSpaceRetirementsV1::from_custom_parameter(&custom).unwrap(),
            Some(two.clone())
        );
        let unordered = DataSpaceRetirementsV1 {
            version: 1,
            retired: vec![record(9, "nine", &[9]), record(3, "three", &[3])],
        };
        assert!(unordered.validate_structure().is_err());
        let forged = CustomParameter::new(
            DataSpaceRetirementsV1::parameter_id(),
            canonical(&json::to_json(&unordered).unwrap()),
        );
        assert!(DataSpaceRetirementsV1::from_custom_parameter(&forged).is_err());
    }
}
