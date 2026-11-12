//! Committed content-bundle limits of a chain.
//!
//! [`ContentPolicyV1`] is the typed payload of the custom parameter
//! [`ContentPolicyV1::PARAMETER_ID_STR`]. It carries the limits and defaults
//! `PublishContentBundle` applies to an archive: the bundle, file-count, path
//! and retention caps, the ingestion chunk size, the cache defaults and the
//! default stripe layout.
//!
//! Recognizing the parameter is behaviour a release adds after the frozen
//! baseline of the live-upgrade foundation, so a root recognizes it only after
//! a governed change set activates the Parameter definition the release
//! publishes for it; activation writes [`ContentPolicyV1::protocol_default`].
//! From then on an authorized `SetParameter` changes it, with effect from the
//! block after the one that commits it. A root that does not recognize it
//! stores the identifier like any other custom parameter and applies the
//! protocol default, which equals the constants of the release.
//!
//! This module owns the type, the protocol default and the canonical
//! validation, so genesis tools and SDKs author and check a policy without a
//! node. Applying a policy to State is Core's job. Gateway rate limits,
//! response objectives and the fetch proof-of-work guard are local resources
//! of a node and are not part of this policy.
//!
//! Access control is not part of this policy either: the publisher allow-list
//! and the default read authorization mode name accounts and roles, and are not
//! limits.

use iroha_primitives::json::Json;

pub use crate::pipeline_validity_policy::PipelinePolicyJsonAdmissionError as ContentPolicyJsonAdmissionError;
use crate::{
    da::prelude::DaStripeLayout,
    parameter::custom::{CustomParameter, CustomParameterId},
};

/// Why a content policy value is not a valid chain policy.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ContentPolicyError {
    /// The committed payload does not decode as the exact V1 policy.
    #[error("the content policy payload is not a canonical V1 policy: {0}")]
    Malformed(String),
    /// A field that must be positive is zero.
    #[error("content policy field `{0}` must be non-zero")]
    Zero(&'static str),
    /// The default cache lifetime exceeds the cache lifetime cap.
    #[error("content policy `default_cache_max_age_secs` exceeds `max_cache_max_age_secs`")]
    DefaultCacheAboveCap,
}

crate::scalar_policy_record::scalar_policy_record! {
    /// The complete first-release content-bundle limits of a chain.
    ContentPolicyV1, "content policy", "iroha_data_model::content_policy::ContentPolicyV1",
    {
        /// Largest tarball accepted for a bundle, in bytes.
        max_bundle_bytes: u64,
        /// Largest number of files in an archive.
        max_files: u32,
        /// Largest path length of one archive entry, in bytes.
        max_path_len: u32,
        /// Largest retention window of an expiring bundle, in blocks.
        max_retention_blocks: u64,
        /// Chunk size used during ingestion, in bytes; non-zero.
        chunk_size_bytes: u32,
        /// Cache lifetime of a bundle that names none, in seconds; between 1
        /// and `max_cache_max_age_secs`.
        default_cache_max_age_secs: u32,
        /// Largest cache lifetime a bundle may name, in seconds; non-zero.
        max_cache_max_age_secs: u32,
        /// Whether every bundle is immutable.
        immutable_bundles: bool,
        /// Row stripes (data and column parity) of the default layout; non-zero.
        stripe_total_stripes: u32,
        /// Shards per stripe (data and row parity) of the default layout; non-zero.
        stripe_shards_per_stripe: u32,
        /// Column-parity stripes of the default layout.
        stripe_row_parity_stripes: u16,
    }
}

impl ContentPolicyV1 {
    /// Identifier of the committed custom parameter that carries the policy.
    pub const PARAMETER_ID_STR: &'static str = "content_policy_v1";

    /// The [`CustomParameterId`] of the committed policy.
    ///
    /// # Panics
    ///
    /// Never: the identifier is a valid parameter name.
    #[must_use]
    pub fn parameter_id() -> CustomParameterId {
        Self::PARAMETER_ID_STR
            .parse()
            .expect("the content policy parameter identifier is a valid name")
    }

    /// The policy of a chain that installs none: the protocol defaults of this
    /// release, identical in every binary.
    #[must_use]
    pub const fn protocol_default() -> Self {
        Self {
            max_bundle_bytes: 1_048_576,
            max_files: 128,
            max_path_len: 256,
            max_retention_blocks: 10_000,
            chunk_size_bytes: 64 * 1024,
            default_cache_max_age_secs: 300,
            max_cache_max_age_secs: 86_400,
            immutable_bundles: true,
            stripe_total_stripes: 1,
            stripe_shards_per_stripe: 1,
            stripe_row_parity_stripes: 0,
        }
    }

    /// The default stripe layout applied to a bundle that names none.
    #[must_use]
    pub const fn stripe_layout(&self) -> DaStripeLayout {
        DaStripeLayout {
            total_stripes: self.stripe_total_stripes,
            shards_per_stripe: self.stripe_shards_per_stripe,
            row_parity_stripes: self.stripe_row_parity_stripes,
        }
    }

    /// Check every rule a committed policy must satisfy.
    ///
    /// The decision uses the policy value alone, so every validator reaches
    /// the same result.
    ///
    /// Rules: the chunk size, the cache lifetime cap, the default cache
    /// lifetime and both stripe counts are positive; the default cache
    /// lifetime is at most the cap. Every other value is admissible; a zero
    /// bundle, file-count or path cap refuses every archive.
    ///
    /// # Errors
    ///
    /// Returns the first rule that fails, in the fixed order of this function.
    pub fn validate(&self) -> Result<(), ContentPolicyError> {
        let non_zero: [(&'static str, u64); 5] = [
            ("chunk_size_bytes", u64::from(self.chunk_size_bytes)),
            (
                "max_cache_max_age_secs",
                u64::from(self.max_cache_max_age_secs),
            ),
            (
                "default_cache_max_age_secs",
                u64::from(self.default_cache_max_age_secs),
            ),
            ("stripe_total_stripes", u64::from(self.stripe_total_stripes)),
            (
                "stripe_shards_per_stripe",
                u64::from(self.stripe_shards_per_stripe),
            ),
        ];
        for (field, value) in non_zero {
            if value == 0 {
                return Err(ContentPolicyError::Zero(field));
            }
        }
        if self.default_cache_max_age_secs > self.max_cache_max_age_secs {
            return Err(ContentPolicyError::DefaultCacheAboveCap);
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
    ) -> Option<Result<Self, ContentPolicyError>> {
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
    pub fn from_payload(payload: &Json) -> Result<Self, ContentPolicyError> {
        let policy = payload
            .try_into_any_norito::<Self>()
            .map_err(|error| ContentPolicyError::Malformed(error.to_string()))?;
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
    ) -> Result<ContentPolicyV1, ContentPolicyJsonAdmissionError> {
        let mut parser = norito::json::Parser::new(source);
        parser.preflight_document()?;
        let value = ContentPolicyV1::try_decode_json(&mut parser, pool)?;
        parser.finish_document()?;
        Ok(value)
    }

    #[test]
    fn protocol_default_is_valid_and_round_trips_through_the_parameter() {
        let policy = ContentPolicyV1::protocol_default();
        policy.validate().expect("the protocol default is valid");
        let custom = policy.into_custom_parameter();
        assert_eq!(custom.id(), &ContentPolicyV1::parameter_id());
        assert_eq!(
            ContentPolicyV1::from_custom_parameter(&custom),
            Some(Ok(policy))
        );
        let other = CustomParameter::new(
            "another_parameter".parse().expect("valid name"),
            Json::new(1_u32),
        );
        assert!(ContentPolicyV1::from_custom_parameter(&other).is_none());
        let bytes = policy.encode();
        assert_eq!(
            ContentPolicyV1::decode(&mut bytes.as_slice()).expect("decodes"),
            policy
        );
        assert_eq!(
            policy.stripe_layout(),
            DaStripeLayout {
                total_stripes: 1,
                shards_per_stripe: 1,
                row_parity_stripes: 0,
            }
        );
    }

    #[test]
    fn field_names_match_the_committed_payload() {
        let names = ContentPolicyV1::FIELD_NAMES;
        assert_eq!(names.len(), 11);
        let value = norito::json::to_value(&ContentPolicyV1::protocol_default())
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
        type Edit = fn(&mut ContentPolicyV1);
        let cases: [(Edit, ContentPolicyError); 6] = [
            (
                |policy| policy.chunk_size_bytes = 0,
                ContentPolicyError::Zero("chunk_size_bytes"),
            ),
            (
                |policy| policy.max_cache_max_age_secs = 0,
                ContentPolicyError::Zero("max_cache_max_age_secs"),
            ),
            (
                |policy| policy.default_cache_max_age_secs = 0,
                ContentPolicyError::Zero("default_cache_max_age_secs"),
            ),
            (
                |policy| policy.stripe_total_stripes = 0,
                ContentPolicyError::Zero("stripe_total_stripes"),
            ),
            (
                |policy| policy.stripe_shards_per_stripe = 0,
                ContentPolicyError::Zero("stripe_shards_per_stripe"),
            ),
            (
                |policy| policy.default_cache_max_age_secs = policy.max_cache_max_age_secs + 1,
                ContentPolicyError::DefaultCacheAboveCap,
            ),
        ];
        for (edit, expected) in cases {
            let mut policy = ContentPolicyV1::protocol_default();
            edit(&mut policy);
            assert_eq!(policy.validate(), Err(expected));
        }
        // Limits may shrink while the rules hold, down to refusing every archive.
        let smaller = ContentPolicyV1 {
            max_bundle_bytes: 0,
            max_files: 0,
            max_path_len: 0,
            max_retention_blocks: 0,
            chunk_size_bytes: 1,
            default_cache_max_age_secs: 1,
            max_cache_max_age_secs: 1,
            immutable_bundles: false,
            stripe_total_stripes: 1,
            stripe_shards_per_stripe: 1,
            stripe_row_parity_stripes: 0,
        };
        smaller.validate().expect("a coherent smaller policy");
    }

    #[test]
    fn payload_with_an_unknown_missing_or_duplicate_field_is_malformed() {
        let default = ContentPolicyV1::protocol_default();
        let mut value = norito::json::to_value(&default).expect("the policy serializes");
        value
            .as_object_mut()
            .expect("a policy object")
            .insert("per_unknown".to_owned(), norito::json::Value::from(1_u64));
        assert!(matches!(
            ContentPolicyV1::from_payload(&Json::from(&value)),
            Err(ContentPolicyError::Malformed(_))
        ));
        for field in ContentPolicyV1::FIELD_NAMES {
            let mut value = norito::json::to_value(&default).expect("the policy serializes");
            assert!(
                value
                    .as_object_mut()
                    .expect("a policy object")
                    .remove(*field)
                    .is_some()
            );
            assert!(matches!(
                ContentPolicyV1::from_payload(&Json::from(&value)),
                Err(ContentPolicyError::Malformed(_))
            ));
        }
        let source = norito::json::to_json(&default).expect("the policy serializes");
        let duplicate = source.replacen('{', "{\"max_files\":128,", 1);
        assert!(norito::json::from_str::<ContentPolicyV1>(&duplicate).is_err());
        let invalid = ContentPolicyV1 {
            chunk_size_bytes: 0,
            ..default
        };
        assert_eq!(
            ContentPolicyV1::from_payload(&Json::new(invalid)),
            Err(ContentPolicyError::Zero("chunk_size_bytes"))
        );
    }

    #[test]
    fn admitted_decode_equals_ordinary_decode_and_retains_nothing() {
        let policy = ContentPolicyV1 {
            max_bundle_bytes: 2_097_152,
            max_files: 64,
            ..ContentPolicyV1::protocol_default()
        };
        let source = norito::json::to_json(&policy).expect("the policy serializes");
        let pool = AllocationBudget::new(0);
        assert_eq!(decode_admitted(&source, &pool).expect("decodes"), policy);
        assert_eq!(pool.reserved_bytes(), 0);
        assert_eq!(
            norito::json::from_str::<ContentPolicyV1>(&source).expect("decodes"),
            policy
        );
        // An escaped key needs scratch from the caller's pool and returns it.
        let escaped = source.replacen("\"max_files\"", "\"\\u006dax_files\"", 1);
        assert!(decode_admitted(&escaped, &pool).is_err());
        let funded = AllocationBudget::new(4096);
        assert_eq!(decode_admitted(&escaped, &funded).expect("decodes"), policy);
        assert_eq!(funded.reserved_bytes(), 0);
        let unknown = source.replacen('{', "{\"unknown\":1,", 1);
        assert!(decode_admitted(&unknown, &funded).is_err());
        assert_eq!(funded.reserved_bytes(), 0);
    }
}
