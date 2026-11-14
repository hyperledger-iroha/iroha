//! Committed uploaded-model and DA ingest quotas of a chain.
//!
//! [`NexusQuotaPolicyV1`] is the typed payload of the custom parameter
//! [`NexusQuotaPolicyV1::PARAMETER_ID_STR`]. It carries the size and chunk
//! ceilings `RegisterSoracloudUploadedModelBundle` applies to one uploaded-model
//! bundle and the count and byte ceilings the DA ingest quota applies to one
//! account in one quota window.
//!
//! Recognizing the parameter is behaviour a release adds after the frozen
//! baseline of the live-upgrade foundation, so a root recognizes it only after
//! a governed change set activates the Parameter definition the release
//! publishes for it; activation writes [`NexusQuotaPolicyV1::protocol_default`].
//! From then on an authorized `SetParameter` changes it, with effect from the
//! block after the one that commits it. A root that does not recognize it
//! stores the identifier like any other custom parameter and applies the
//! protocol default, which equals the values every first-release network ran.
//!
//! This module owns the type, the protocol default and the canonical
//! validation, so genesis tools and SDKs author and check a policy without a
//! node. Applying a policy to State is Core's job. A node configuration holds
//! none of these values, and the length of the DA ingest quota window is not
//! part of this policy.

use iroha_primitives::json::Json;

use crate::parameter::custom::{CustomParameter, CustomParameterId};
pub use crate::pipeline_validity_policy::PipelinePolicyJsonAdmissionError as NexusQuotaPolicyJsonAdmissionError;

/// Why a quota policy value is not a valid chain policy.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NexusQuotaPolicyError {
    /// The committed payload does not decode as the exact V1 policy.
    #[error("the Nexus quota policy payload is not a canonical V1 policy: {0}")]
    Malformed(String),
    /// A field that must be positive is zero.
    #[error("Nexus quota policy field `{0}` must be non-zero")]
    Zero(&'static str),
}

crate::scalar_policy_record::scalar_policy_record! {
    /// The complete first-release uploaded-model and DA ingest quotas of a chain.
    NexusQuotaPolicyV1, "Nexus quota policy", "iroha_data_model::nexus_quota_policy::NexusQuotaPolicyV1",
    {
        /// Largest plaintext size one uploaded-model bundle may declare, in bytes.
        max_uploaded_model_plaintext_bytes: u64,
        /// Most encrypted chunks one uploaded-model bundle may declare.
        max_uploaded_model_chunks: u32,
        /// Most DA pin intents one account may have accepted in one ingest quota
        /// window; non-zero.
        max_da_ingests_per_account: u64,
        /// Most canonical DA payload bytes one account may have accepted in one
        /// ingest quota window; non-zero.
        max_da_ingest_bytes_per_account: u64,
    }
}

impl NexusQuotaPolicyV1 {
    /// Identifier of the committed custom parameter that carries the policy.
    pub const PARAMETER_ID_STR: &'static str = "nexus_quota_policy_v1";

    /// The [`CustomParameterId`] of the committed policy.
    ///
    /// # Panics
    ///
    /// Never: the identifier is a valid parameter name.
    #[must_use]
    pub fn parameter_id() -> CustomParameterId {
        Self::PARAMETER_ID_STR
            .parse()
            .expect("the Nexus quota policy parameter identifier is a valid name")
    }

    /// The policy of a chain that installs none: the protocol defaults of this
    /// release, identical in every binary.
    #[must_use]
    pub const fn protocol_default() -> Self {
        Self {
            max_uploaded_model_plaintext_bytes: 64 * 1024 * 1024 * 1024,
            max_uploaded_model_chunks: 16_384,
            max_da_ingests_per_account: 1_024,
            max_da_ingest_bytes_per_account: 64 << 30,
        }
    }

    /// Check every rule a committed policy must satisfy.
    ///
    /// The decision uses the policy value alone, so every validator reaches
    /// the same result.
    ///
    /// Rules: both DA ingest ceilings are positive. Every other value is
    /// admissible; a zero uploaded-model ceiling refuses every bundle that
    /// declares any plaintext or chunk.
    ///
    /// # Errors
    ///
    /// Returns the first rule that fails, in the fixed order of this function.
    pub fn validate(&self) -> Result<(), NexusQuotaPolicyError> {
        let non_zero: [(&'static str, u64); 2] = [
            (
                "max_da_ingests_per_account",
                self.max_da_ingests_per_account,
            ),
            (
                "max_da_ingest_bytes_per_account",
                self.max_da_ingest_bytes_per_account,
            ),
        ];
        for (field, value) in non_zero {
            if value == 0 {
                return Err(NexusQuotaPolicyError::Zero(field));
            }
        }
        Ok(())
    }

    /// Wrap the policy as the committed custom parameter.
    #[must_use]
    pub fn into_custom_parameter(self) -> CustomParameter {
        CustomParameter::new(Self::parameter_id(), Json::new(self))
    }

    /// Decode and validate the policy carried by `custom`.
    ///
    /// Returns `None` when `custom` is another parameter.
    ///
    /// # Errors
    ///
    /// Returns an error when the payload is not the exact V1 policy or a
    /// value violates a rule of [`Self::validate`]. Every validator derives
    /// the same error from the same committed bytes.
    pub fn from_custom_parameter(
        custom: &CustomParameter,
    ) -> Option<Result<Self, NexusQuotaPolicyError>> {
        if custom.id() != &Self::parameter_id() {
            return None;
        }
        Some(Self::from_payload(custom.payload()))
    }

    /// Decode and validate a policy payload.
    ///
    /// # Errors
    ///
    /// Returns an error when the payload is not the exact V1 policy or a
    /// value violates a rule of [`Self::validate`].
    pub fn from_payload(payload: &Json) -> Result<Self, NexusQuotaPolicyError> {
        let policy = payload
            .try_into_any_norito::<Self>()
            .map_err(|error| NexusQuotaPolicyError::Malformed(error.to_string()))?;
        policy.validate()?;
        Ok(policy)
    }
}

#[cfg(test)]
mod tests {
    use iroha_allocation::AllocationBudget;
    use norito::codec::{Decode as _, Encode as _};

    use super::*;

    fn decode_admitted(
        source: &str,
        pool: &AllocationBudget,
    ) -> Result<NexusQuotaPolicyV1, NexusQuotaPolicyJsonAdmissionError> {
        let mut parser = norito::json::Parser::new(source);
        parser.preflight_document()?;
        let value = NexusQuotaPolicyV1::try_decode_json(&mut parser, pool)?;
        parser.finish_document()?;
        Ok(value)
    }

    #[test]
    fn protocol_default_is_valid_and_round_trips_through_the_parameter() {
        let policy = NexusQuotaPolicyV1::protocol_default();
        policy.validate().expect("the protocol default is valid");
        // The values every first-release network ran: 64 GiB and 16 384 chunks per
        // uploaded model, 1 024 ingests and 64 GiB per account and quota window.
        assert_eq!(policy.max_uploaded_model_plaintext_bytes, 68_719_476_736);
        assert_eq!(policy.max_uploaded_model_chunks, 16_384);
        assert_eq!(policy.max_da_ingests_per_account, 1_024);
        assert_eq!(policy.max_da_ingest_bytes_per_account, 68_719_476_736);
        let custom = policy.into_custom_parameter();
        assert_eq!(custom.id(), &NexusQuotaPolicyV1::parameter_id());
        assert_eq!(
            NexusQuotaPolicyV1::from_custom_parameter(&custom),
            Some(Ok(policy))
        );
        let other = CustomParameter::new(
            "another_parameter".parse().expect("valid name"),
            Json::new(1_u32),
        );
        assert!(NexusQuotaPolicyV1::from_custom_parameter(&other).is_none());
        let bytes = policy.encode();
        assert_eq!(
            NexusQuotaPolicyV1::decode(&mut bytes.as_slice()).expect("decodes"),
            policy
        );
    }

    #[test]
    fn field_names_match_the_committed_payload() {
        let names = NexusQuotaPolicyV1::FIELD_NAMES;
        assert_eq!(names.len(), 4);
        let value = norito::json::to_value(&NexusQuotaPolicyV1::protocol_default())
            .expect("the policy serializes");
        let mut payload_names: Vec<String> = value
            .as_object()
            .expect("a policy object")
            .keys()
            .cloned()
            .collect();
        payload_names.sort();
        let mut sorted: Vec<String> = names.iter().map(|name| (*name).to_owned()).collect();
        sorted.sort();
        assert_eq!(payload_names, sorted);
    }

    #[test]
    fn rules_reject_each_invalid_value() {
        type Edit = fn(&mut NexusQuotaPolicyV1);
        let cases: [(Edit, NexusQuotaPolicyError); 2] = [
            (
                |policy| policy.max_da_ingests_per_account = 0,
                NexusQuotaPolicyError::Zero("max_da_ingests_per_account"),
            ),
            (
                |policy| policy.max_da_ingest_bytes_per_account = 0,
                NexusQuotaPolicyError::Zero("max_da_ingest_bytes_per_account"),
            ),
        ];
        for (edit, expected) in cases {
            let mut policy = NexusQuotaPolicyV1::protocol_default();
            edit(&mut policy);
            assert_eq!(policy.validate(), Err(expected));
        }
        // The uploaded-model ceilings may shrink down to refusing every bundle,
        // and the DA ceilings down to one ingest of one byte.
        let smallest = NexusQuotaPolicyV1 {
            max_uploaded_model_plaintext_bytes: 0,
            max_uploaded_model_chunks: 0,
            max_da_ingests_per_account: 1,
            max_da_ingest_bytes_per_account: 1,
        };
        smallest.validate().expect("a coherent smaller policy");
        let largest = NexusQuotaPolicyV1 {
            max_uploaded_model_plaintext_bytes: u64::MAX,
            max_uploaded_model_chunks: u32::MAX,
            max_da_ingests_per_account: u64::MAX,
            max_da_ingest_bytes_per_account: u64::MAX,
        };
        largest.validate().expect("no ceiling has an upper rule");
    }

    #[test]
    fn payload_with_an_unknown_missing_or_duplicate_field_is_malformed() {
        let default = NexusQuotaPolicyV1::protocol_default();
        let mut value = norito::json::to_value(&default).expect("the policy serializes");
        value
            .as_object_mut()
            .expect("a policy object")
            .insert("per_unknown".to_owned(), norito::json::Value::from(1_u64));
        assert!(matches!(
            NexusQuotaPolicyV1::from_payload(&Json::from(&value)),
            Err(NexusQuotaPolicyError::Malformed(_))
        ));
        for field in NexusQuotaPolicyV1::FIELD_NAMES {
            let mut value = norito::json::to_value(&default).expect("the policy serializes");
            assert!(
                value
                    .as_object_mut()
                    .expect("a policy object")
                    .remove(*field)
                    .is_some()
            );
            assert!(matches!(
                NexusQuotaPolicyV1::from_payload(&Json::from(&value)),
                Err(NexusQuotaPolicyError::Malformed(_))
            ));
        }
        let source = norito::json::to_json(&default).expect("the policy serializes");
        let duplicate = source.replacen('{', "{\"max_uploaded_model_chunks\":16384,", 1);
        assert!(norito::json::from_str::<NexusQuotaPolicyV1>(&duplicate).is_err());
        let invalid = NexusQuotaPolicyV1 {
            max_da_ingests_per_account: 0,
            ..default
        };
        assert_eq!(
            NexusQuotaPolicyV1::from_payload(&Json::new(invalid)),
            Err(NexusQuotaPolicyError::Zero("max_da_ingests_per_account"))
        );
    }

    #[test]
    fn admitted_decode_equals_ordinary_decode_and_retains_nothing() {
        let policy = NexusQuotaPolicyV1 {
            max_uploaded_model_chunks: 64,
            max_da_ingests_per_account: 8,
            ..NexusQuotaPolicyV1::protocol_default()
        };
        let source = norito::json::to_json(&policy).expect("the policy serializes");
        let pool = AllocationBudget::new(0);
        assert_eq!(decode_admitted(&source, &pool).expect("decodes"), policy);
        assert_eq!(pool.reserved_bytes(), 0);
        assert_eq!(
            norito::json::from_str::<NexusQuotaPolicyV1>(&source).expect("decodes"),
            policy
        );
        // An escaped key needs scratch from the caller's pool and returns it.
        let escaped = source.replacen(
            "\"max_uploaded_model_chunks\"",
            "\"\\u006dax_uploaded_model_chunks\"",
            1,
        );
        assert!(decode_admitted(&escaped, &pool).is_err());
        let funded = AllocationBudget::new(4096);
        assert_eq!(decode_admitted(&escaped, &funded).expect("decodes"), policy);
        assert_eq!(funded.reserved_bytes(), 0);
        let unknown = source.replacen('{', "{\"unknown\":1,", 1);
        assert!(decode_admitted(&unknown, &funded).is_err());
        assert_eq!(funded.reserved_bytes(), 0);
    }
}
