//! Committed notice policy of governed runtime changes (`specs/live_runtime_upgrades.md` §6,
//! "Notice"; custom parameter `runtime_upgrade_policy_v1`).
//!
//! A change set becomes eligible only after both its block floor and its committed-time floor
//! have passed. Admission of a proposal freezes the policy in force into its record, so a later
//! parameter update never changes the notice of a change already proposed. Notice is a lower
//! bound: it never bypasses governance and never produces empty blocks.
//!
//! Missing or malformed committed state never falls back to these defaults: a root without the
//! committed parameter admits no change set, and
//! [`RuntimeUpgradePolicyV1::from_custom_parameter`] refuses another identity, an oversized,
//! malformed, non-canonical or invalid payload. The defaults are the production values the
//! design recommends; a testnet commits shorter values through the same parameter.

use iroha_primitives::json::Json;

use super::{CustomParameter, CustomParameterId};
use crate::runtime::lifecycle::RuntimeNoticeV1;

/// Why a committed `runtime_upgrade_policy_v1` parameter cannot be read or written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RuntimeUpgradePolicyErrorV1 {
    /// The custom parameter has another identity, an oversized or malformed payload, or a
    /// payload that is not the canonical JSON of its value.
    #[error("runtime upgrade policy parameter: {0}")]
    CustomParameter(&'static str),
    /// A disable notice is shorter than an activation notice.
    #[error("disabling must give at least the notice of an activation")]
    DisableShorterThanActivation,
}

/// Minimum notice of each kind of governed runtime change.
#[derive(
    Clone,
    Copy,
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
    name = "iroha_data_model::parameter::runtime_upgrade_policy::RuntimeUpgradePolicyV1"
)]
pub struct RuntimeUpgradePolicyV1 {
    /// Notice of a change that activates, binds, installs or rebinds definitions.
    pub activation: RuntimeNoticeV1,
    /// Notice of a change that disables or retires definitions.
    pub disable: RuntimeNoticeV1,
}

impl Default for RuntimeUpgradePolicyV1 {
    /// The production values of `live_runtime_upgrades.md` §6: 3,600 blocks and 14 days to
    /// activate, 3,600 blocks and 30 days to disable, and a 24-hour cushion after approval.
    fn default() -> Self {
        const DAY_MS: u64 = 24 * 60 * 60 * 1000;
        Self {
            activation: RuntimeNoticeV1 {
                blocks: 3_600,
                ms: 14 * DAY_MS,
                approval_cushion_ms: DAY_MS,
            },
            disable: RuntimeNoticeV1 {
                blocks: 3_600,
                ms: 30 * DAY_MS,
                approval_cushion_ms: DAY_MS,
            },
        }
    }
}

impl RuntimeUpgradePolicyV1 {
    /// Largest accepted JSON payload.
    pub const MAX_PAYLOAD_BYTES: usize = 1_024;

    /// The sole canonical custom parameter identity.
    #[must_use]
    pub fn parameter_id() -> &'static CustomParameterId {
        static ID: std::sync::OnceLock<CustomParameterId> = std::sync::OnceLock::new();
        ID.get_or_init(|| {
            "runtime_upgrade_policy_v1"
                .parse()
                .expect("static parameter identity")
        })
    }

    /// Check that disabling never gives less notice than activating.
    ///
    /// # Errors
    /// [`RuntimeUpgradePolicyErrorV1::DisableShorterThanActivation`].
    pub fn validate(&self) -> Result<(), RuntimeUpgradePolicyErrorV1> {
        if self.disable.blocks < self.activation.blocks
            || self.disable.ms < self.activation.ms
            || self.disable.approval_cushion_ms < self.activation.approval_cushion_ms
        {
            return Err(RuntimeUpgradePolicyErrorV1::DisableShorterThanActivation);
        }
        Ok(())
    }

    /// Decode and validate the exact committed parameter, without substituting defaults on
    /// failure.
    ///
    /// # Errors
    /// Another identity, more than [`Self::MAX_PAYLOAD_BYTES`] of JSON, a malformed or
    /// non-canonical payload, or a value that fails [`Self::validate`].
    pub fn from_custom_parameter(
        parameter: &CustomParameter,
    ) -> Result<Self, RuntimeUpgradePolicyErrorV1> {
        if parameter.id() != Self::parameter_id() {
            return Err(RuntimeUpgradePolicyErrorV1::CustomParameter(
                "another parameter identity",
            ));
        }
        let payload = parameter.payload();
        if payload.get().len() > Self::MAX_PAYLOAD_BYTES {
            return Err(RuntimeUpgradePolicyErrorV1::CustomParameter(
                "payload exceeds 1024 bytes",
            ));
        }
        let value: Self = norito::json::from_str(payload.get())
            .map_err(|_| RuntimeUpgradePolicyErrorV1::CustomParameter("malformed payload"))?;
        if Json::try_new(&value).ok().as_ref() != Some(payload) {
            return Err(RuntimeUpgradePolicyErrorV1::CustomParameter(
                "payload is not the canonical JSON of its value",
            ));
        }
        value.validate()?;
        Ok(value)
    }

    /// The committed custom parameter of a valid value, with its canonical JSON payload.
    ///
    /// # Errors
    /// A value that fails [`Self::validate`], or a payload that cannot be produced.
    pub fn to_custom_parameter(&self) -> Result<CustomParameter, RuntimeUpgradePolicyErrorV1> {
        self.validate()?;
        let payload = Json::try_new(self).map_err(|_| {
            RuntimeUpgradePolicyErrorV1::CustomParameter("payload cannot be serialized")
        })?;
        Ok(CustomParameter::new(Self::parameter_id().clone(), payload))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_policy_round_trips_through_its_parameter() {
        let policy = RuntimeUpgradePolicyV1::default();
        let parameter = policy.to_custom_parameter().expect("valid policy");
        assert_eq!(
            RuntimeUpgradePolicyV1::from_custom_parameter(&parameter),
            Ok(policy)
        );
    }

    #[test]
    fn disabling_never_gives_less_notice_than_activating() {
        let mut policy = RuntimeUpgradePolicyV1::default();
        policy.disable.ms = policy.activation.ms - 1;
        assert_eq!(
            policy.validate(),
            Err(RuntimeUpgradePolicyErrorV1::DisableShorterThanActivation)
        );
        assert!(policy.to_custom_parameter().is_err());
    }

    #[test]
    fn foreign_and_non_canonical_payloads_are_refused() {
        let policy = RuntimeUpgradePolicyV1::default();
        let other = CustomParameter::new(
            "parliament_lifecycle_v1".parse().unwrap(),
            Json::try_new(&policy).unwrap(),
        );
        assert!(matches!(
            RuntimeUpgradePolicyV1::from_custom_parameter(&other),
            Err(RuntimeUpgradePolicyErrorV1::CustomParameter(_))
        ));
        let malformed = CustomParameter::new(
            RuntimeUpgradePolicyV1::parameter_id().clone(),
            Json::from_raw_json("{\"activation\":1}".to_owned()).unwrap(),
        );
        assert!(matches!(
            RuntimeUpgradePolicyV1::from_custom_parameter(&malformed),
            Err(RuntimeUpgradePolicyErrorV1::CustomParameter(_))
        ));
    }
}
