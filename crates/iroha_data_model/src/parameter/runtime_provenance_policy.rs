//! Committed change-set provenance policy of the universal root (`specs/live_runtime_upgrades.md`
//! §6, "Who governs"; custom parameter `runtime_provenance_policy_v1`).
//!
//! Every rule that decides whether a change set is valid comes from committed State. While this
//! parameter is committed, SORA Parliament admits a proposal that schedules a change set only
//! with endorsements of the change set's identity from `threshold` distinct trusted release
//! signers, who vouch for the build, SBOM and SLSA evidence of what it publishes. A root without
//! the parameter requires no endorsement; a malformed committed value never falls back to a
//! default.

use iroha_crypto::PublicKey;
use iroha_primitives::json::Json;

use super::{CustomParameter, CustomParameterId};

/// Why a committed `runtime_provenance_policy_v1` parameter cannot be read or written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RuntimeProvenancePolicyErrorV1 {
    /// The custom parameter has another identity, an oversized or malformed payload, or a
    /// payload that is not the canonical JSON of its value.
    #[error("runtime provenance policy parameter: {0}")]
    CustomParameter(&'static str),
    /// The signers are empty, unsorted or repeated.
    #[error("the trusted signers must be non-empty, sorted and unique")]
    Signers,
    /// The threshold is zero or exceeds the number of signers.
    #[error("the threshold must be at least one and at most the number of signers")]
    Threshold,
}

/// The release signers whose endorsements a scheduled change set needs.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::codec::Encode,
    norito::codec::Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(
    name = "iroha_data_model::parameter::runtime_provenance_policy::RuntimeProvenancePolicyV1"
)]
pub struct RuntimeProvenancePolicyV1 {
    /// Trusted release signers, sorted by key.
    pub signers: Vec<PublicKey>,
    /// Distinct trusted signers that must endorse a change set.
    pub threshold: u32,
}

impl RuntimeProvenancePolicyV1 {
    /// Largest accepted JSON payload.
    pub const MAX_PAYLOAD_BYTES: usize = 16 * 1024;

    /// The sole canonical custom parameter identity.
    #[must_use]
    pub fn parameter_id() -> &'static CustomParameterId {
        static ID: std::sync::OnceLock<CustomParameterId> = std::sync::OnceLock::new();
        ID.get_or_init(|| {
            "runtime_provenance_policy_v1"
                .parse()
                .expect("static parameter identity")
        })
    }

    /// Check the signers and the threshold.
    ///
    /// # Errors
    /// The first rule the policy violates.
    pub fn validate(&self) -> Result<(), RuntimeProvenancePolicyErrorV1> {
        if self.signers.is_empty() || !self.signers.windows(2).all(|pair| pair[0] < pair[1]) {
            return Err(RuntimeProvenancePolicyErrorV1::Signers);
        }
        let threshold = usize::try_from(self.threshold)
            .map_err(|_| RuntimeProvenancePolicyErrorV1::Threshold)?;
        if threshold == 0 || threshold > self.signers.len() {
            return Err(RuntimeProvenancePolicyErrorV1::Threshold);
        }
        Ok(())
    }

    /// Decode and validate the exact committed parameter, without substituting defaults.
    ///
    /// # Errors
    /// Another identity, more than [`Self::MAX_PAYLOAD_BYTES`] of JSON, a malformed or
    /// non-canonical payload, or a value that fails [`Self::validate`].
    pub fn from_custom_parameter(
        parameter: &CustomParameter,
    ) -> Result<Self, RuntimeProvenancePolicyErrorV1> {
        if parameter.id() != Self::parameter_id() {
            return Err(RuntimeProvenancePolicyErrorV1::CustomParameter(
                "another parameter identity",
            ));
        }
        let payload = parameter.payload();
        if payload.get().len() > Self::MAX_PAYLOAD_BYTES {
            return Err(RuntimeProvenancePolicyErrorV1::CustomParameter(
                "payload exceeds 16384 bytes",
            ));
        }
        let value: Self = norito::json::from_str(payload.get())
            .map_err(|_| RuntimeProvenancePolicyErrorV1::CustomParameter("malformed payload"))?;
        if Json::try_new(&value).ok().as_ref() != Some(payload) {
            return Err(RuntimeProvenancePolicyErrorV1::CustomParameter(
                "payload is not the canonical JSON of its value",
            ));
        }
        value.validate()?;
        Ok(value)
    }

    /// The committed custom parameter of a valid policy, with its canonical JSON payload.
    ///
    /// # Errors
    /// A policy that fails [`Self::validate`], or a payload that cannot be produced.
    pub fn to_custom_parameter(&self) -> Result<CustomParameter, RuntimeProvenancePolicyErrorV1> {
        self.validate()?;
        let payload = Json::try_new(self).map_err(|_| {
            RuntimeProvenancePolicyErrorV1::CustomParameter("payload cannot be serialized")
        })?;
        Ok(CustomParameter::new(Self::parameter_id().clone(), payload))
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair};

    use super::*;

    fn signers(count: u8) -> Vec<PublicKey> {
        let mut keys: Vec<PublicKey> = (1..=count)
            .map(|seed| {
                KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
                    .public_key()
                    .clone()
            })
            .collect();
        keys.sort();
        keys
    }

    #[test]
    fn a_valid_policy_round_trips_through_its_parameter() {
        let policy = RuntimeProvenancePolicyV1 {
            signers: signers(3),
            threshold: 2,
        };
        let parameter = policy.to_custom_parameter().expect("valid policy");
        assert_eq!(
            RuntimeProvenancePolicyV1::from_custom_parameter(&parameter),
            Ok(policy)
        );
    }

    #[test]
    fn signers_are_canonical_and_the_threshold_is_reachable() {
        let mut unsorted = RuntimeProvenancePolicyV1 {
            signers: signers(3),
            threshold: 2,
        };
        unsorted.signers.swap(0, 1);
        assert_eq!(
            unsorted.validate(),
            Err(RuntimeProvenancePolicyErrorV1::Signers)
        );
        let empty = RuntimeProvenancePolicyV1 {
            signers: Vec::new(),
            threshold: 1,
        };
        assert_eq!(
            empty.validate(),
            Err(RuntimeProvenancePolicyErrorV1::Signers)
        );
        for threshold in [0, 4] {
            let policy = RuntimeProvenancePolicyV1 {
                signers: signers(3),
                threshold,
            };
            assert_eq!(
                policy.validate(),
                Err(RuntimeProvenancePolicyErrorV1::Threshold),
                "{threshold}"
            );
        }
    }
}
