//! Owner-bound physical retirement requests and immutable storage history.
//!
//! Retirement removes effective execution authority. The authenticated catalog
//! retains the original descriptors and storage bindings permanently; it never
//! reuses a retired physical lane identity or erases its native block history.

use super::{DataSpaceId, LaneConfig, LaneId, NexusCatalogValidationError};
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize, account::AccountId};
use iroha_crypto::Hash;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

/// The exact paid namespace ownership required to retire one physical dataspace.
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
#[norito_schema(name = "iroha_data_model::nexus::RuntimeDataSpaceRetirementV1")]
pub struct RuntimeDataSpaceRetirementV1 {
    /// Existing physical dataspace whose execution authority is being retired.
    pub dataspace_id: DataSpaceId,
    /// Exact canonical paid SNS label; the retained lease is never removed.
    pub alias: String,
    /// Exact current namespace owner, also required to sign the transition.
    pub owner: AccountId,
    /// Independently selected current SNS ownership generation.
    pub expected_ownership_generation: u64,
}

impl RuntimeDataSpaceRetirementV1 {
    /// Validate the structural bounds; Core independently authenticates ownership.
    ///
    /// # Errors
    /// Rejects the universal dataspace, malformed alias, or absent generation.
    pub fn validate_structure(&self) -> Result<(), NexusCatalogValidationError> {
        if self.dataspace_id == DataSpaceId::UNIVERSAL || self.expected_ownership_generation == 0 {
            return Err(NexusCatalogValidationError::InvalidDataSpace(
                "retirement identity or ownership generation",
            ));
        }
        super::runtime_catalog::validate_alias(&self.alias)
    }
}

/// Original storage binding retained after its physical execution lane retires.
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
#[norito_schema(name = "iroha_data_model::nexus::RuntimeLaneRetirementV1")]
pub struct RuntimeLaneRetirementV1 {
    /// Complete original lane descriptor, including its dataspace and alias.
    pub lane: LaneConfig,
    /// Exact incarnation authenticated by the retiring catalog commitment.
    pub incarnation: Hash,
    /// Original activation height, preserved for native storage identity.
    pub activation_height: u64,
    /// Height of the committed physical retirement; always post-genesis.
    pub retirement_height: u64,
}

impl RuntimeLaneRetirementV1 {
    /// Validate the historical binding without authorizing any storage operation.
    ///
    /// # Errors
    /// Rejects primary/universal retirement, zero incarnation, or inverted heights.
    pub fn validate_structure(&self) -> Result<(), NexusCatalogValidationError> {
        if self.lane.id == LaneId::SINGLE
            || self.lane.dataspace_id == DataSpaceId::UNIVERSAL
            || self.incarnation == Hash::prehashed([0; 32])
            || self.incarnation.as_ref() == &[0; 32]
            || self.retirement_height <= 1
            || self.activation_height > self.retirement_height
        {
            return Err(NexusCatalogValidationError::InvalidLane(
                "invalid retained retirement binding".into(),
            ));
        }
        super::runtime_catalog::validate_lanes(std::slice::from_ref(&self.lane))
    }
}

/// A committed owner-bound retirement, retained across replay and snapshots.
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
#[norito_schema(name = "iroha_data_model::nexus::RuntimeDataSpaceRetirementRecordV1")]
pub struct RuntimeDataSpaceRetirementRecordV1 {
    /// Original exact paid-namespace retirement request.
    pub retirement: RuntimeDataSpaceRetirementV1,
    /// Actual accepted block height, supplied solely by native execution.
    pub retirement_height: u64,
}

impl RuntimeDataSpaceRetirementRecordV1 {
    /// Validate the retained native structure, including its post-genesis height.
    ///
    /// # Errors
    /// Rejects a malformed request or a genesis/pre-genesis retirement height.
    pub fn validate_structure(&self) -> Result<(), NexusCatalogValidationError> {
        self.retirement.validate_structure()?;
        if self.retirement_height <= 1 {
            return Err(NexusCatalogValidationError::InvalidDataSpace(
                "retirement must be post-genesis",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner() -> AccountId {
        AccountId::new(iroha_crypto::KeyPair::random().public_key().clone())
    }

    #[test]
    fn owner_generation_and_native_retirement_height_are_required() {
        let request = RuntimeDataSpaceRetirementV1 {
            dataspace_id: DataSpaceId::new(7),
            alias: "is".into(),
            owner: owner(),
            expected_ownership_generation: 1,
        };
        request.validate_structure().unwrap();
        for invalid in [
            RuntimeDataSpaceRetirementV1 {
                dataspace_id: DataSpaceId::UNIVERSAL,
                ..request.clone()
            },
            RuntimeDataSpaceRetirementV1 {
                expected_ownership_generation: 0,
                ..request.clone()
            },
        ] {
            assert!(invalid.validate_structure().is_err());
        }
        let record = RuntimeDataSpaceRetirementRecordV1 {
            retirement: request,
            retirement_height: 8,
        };
        record.validate_structure().unwrap();
        assert!(
            RuntimeDataSpaceRetirementRecordV1 {
                retirement_height: 1,
                ..record
            }
            .validate_structure()
            .is_err()
        );
    }

    #[test]
    fn historical_lane_binding_preserves_exact_incarnation_and_activation() {
        let record = RuntimeLaneRetirementV1 {
            lane: LaneConfig {
                id: LaneId::new(7),
                dataspace_id: DataSpaceId::new(7),
                alias: "is".into(),
                ..LaneConfig::default()
            },
            incarnation: Hash::new(b"original retired incarnation"),
            activation_height: 3,
            retirement_height: 8,
        };
        record.validate_structure().unwrap();
        let bytes = norito::to_bytes(&record).unwrap();
        assert_eq!(
            norito::decode_from_bytes::<RuntimeLaneRetirementV1>(&bytes).unwrap(),
            record
        );
        for invalid in [
            RuntimeLaneRetirementV1 {
                incarnation: Hash::prehashed([0; 32]),
                ..record.clone()
            },
            RuntimeLaneRetirementV1 {
                activation_height: 9,
                ..record.clone()
            },
            RuntimeLaneRetirementV1 {
                lane: LaneConfig {
                    id: LaneId::SINGLE,
                    ..record.lane.clone()
                },
                ..record.clone()
            },
        ] {
            assert!(invalid.validate_structure().is_err());
        }
    }
}
