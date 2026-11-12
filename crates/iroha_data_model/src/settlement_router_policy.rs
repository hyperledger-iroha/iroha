//! Committed settlement router policy of a chain.
//!
//! [`SettlementRouterPolicyV1`] is the typed payload of the custom parameter
//! [`SettlementRouterPolicyV1::PARAMETER_ID_STR`]. It carries the inputs of the
//! deterministic XOR settlement router: the TWAP window, the base safety
//! margin, the buffer guard-rail thresholds and the buffer coverage horizon.
//!
//! Like `content_policy_v1`, a root recognizes it only after a governed change
//! set activates the Parameter definition the release publishes for it;
//! activation writes [`SettlementRouterPolicyV1::protocol_default`], and an
//! authorized `SetParameter` then changes it with effect from the next block.
//! A root that does not recognize it applies the protocol default.
//!
//! This module owns the type, the protocol default and the canonical
//! validation, so genesis tools and SDKs author and check a policy without a
//! node. Applying a policy to State is Core's job. A node configuration holds
//! no settlement router value.

use iroha_primitives::json::Json;

use crate::parameter::custom::{CustomParameter, CustomParameterId};
pub use crate::pipeline_validity_policy::PipelinePolicyJsonAdmissionError as SettlementRouterPolicyJsonAdmissionError;

/// Largest base safety margin, in basis points: the whole amount.
pub const MAX_EPSILON_BPS: u16 = 10_000;

/// Why a settlement router policy value is not a valid chain policy.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SettlementRouterPolicyError {
    /// The committed payload does not decode as the exact V1 policy.
    #[error("the settlement router policy payload is not a canonical V1 policy: {0}")]
    Malformed(String),
    /// A field that must be positive is zero.
    #[error("settlement router policy field `{0}` must be non-zero")]
    Zero(&'static str),
    /// A field exceeds its fixed protocol maximum.
    #[error("settlement router policy field `{0}` exceeds the protocol maximum of the field")]
    OutOfRange(&'static str),
    /// The buffer thresholds are not strictly ordered.
    #[error(
        "settlement router buffer thresholds must satisfy 100 >= alert > throttle > xor_only > halt"
    )]
    ThresholdOrder,
}

crate::scalar_policy_record::scalar_policy_record! {
    /// The complete first-release settlement router policy of a chain.
    SettlementRouterPolicyV1, "settlement router policy",
    "iroha_data_model::settlement_router_policy::SettlementRouterPolicyV1",
    {
        /// TWAP window used during settlement conversion, in whole seconds; non-zero.
        twap_window_seconds: u64,
        /// Base safety margin in basis points; at most [`MAX_EPSILON_BPS`].
        epsilon_bps: u16,
        /// Remaining-buffer percentage below which the router alerts; at most 100.
        buffer_alert_pct: u8,
        /// Remaining-buffer percentage below which the router throttles.
        buffer_throttle_pct: u8,
        /// Remaining-buffer percentage below which only XOR is accepted.
        buffer_xor_only_pct: u8,
        /// Remaining-buffer percentage below which the router halts.
        buffer_halt_pct: u8,
        /// Hours of spend the XOR buffer must cover; non-zero.
        buffer_horizon_hours: u16,
    }
}

impl SettlementRouterPolicyV1 {
    /// Identifier of the committed custom parameter that carries the policy.
    pub const PARAMETER_ID_STR: &'static str = "settlement_router_policy_v1";

    /// The [`CustomParameterId`] of the committed policy.
    ///
    /// # Panics
    ///
    /// Never: the identifier is a valid parameter name.
    #[must_use]
    pub fn parameter_id() -> CustomParameterId {
        Self::PARAMETER_ID_STR
            .parse()
            .expect("the settlement router policy parameter identifier is a valid name")
    }

    /// The policy of a chain that installs none: the protocol defaults of this
    /// release, identical in every binary.
    #[must_use]
    pub const fn protocol_default() -> Self {
        Self {
            twap_window_seconds: 60,
            epsilon_bps: 25,
            buffer_alert_pct: 75,
            buffer_throttle_pct: 25,
            buffer_xor_only_pct: 10,
            buffer_halt_pct: 2,
            buffer_horizon_hours: 72,
        }
    }

    /// Check every rule a committed policy must satisfy.
    ///
    /// The decision uses the policy value alone and fixed protocol constants,
    /// so every validator reaches the same result.
    ///
    /// Rules: the TWAP window and the buffer horizon are positive; the safety
    /// margin is at most [`MAX_EPSILON_BPS`]; the buffer thresholds satisfy
    /// `100 >= alert > throttle > xor_only > halt`.
    ///
    /// # Errors
    ///
    /// Returns the first rule that fails, in the fixed order of this function.
    pub fn validate(&self) -> Result<(), SettlementRouterPolicyError> {
        if self.twap_window_seconds == 0 {
            return Err(SettlementRouterPolicyError::Zero("twap_window_seconds"));
        }
        if self.epsilon_bps > MAX_EPSILON_BPS {
            return Err(SettlementRouterPolicyError::OutOfRange("epsilon_bps"));
        }
        if self.buffer_horizon_hours == 0 {
            return Err(SettlementRouterPolicyError::Zero("buffer_horizon_hours"));
        }
        if self.buffer_alert_pct > 100
            || self.buffer_throttle_pct >= self.buffer_alert_pct
            || self.buffer_xor_only_pct >= self.buffer_throttle_pct
            || self.buffer_halt_pct >= self.buffer_xor_only_pct
        {
            return Err(SettlementRouterPolicyError::ThresholdOrder);
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
    ) -> Option<Result<Self, SettlementRouterPolicyError>> {
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
    pub fn from_payload(payload: &Json) -> Result<Self, SettlementRouterPolicyError> {
        let policy = payload
            .try_into_any_norito::<Self>()
            .map_err(|error| SettlementRouterPolicyError::Malformed(error.to_string()))?;
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
    ) -> Result<SettlementRouterPolicyV1, SettlementRouterPolicyJsonAdmissionError> {
        let mut parser = norito::json::Parser::new(source);
        parser.preflight_document()?;
        let value = SettlementRouterPolicyV1::try_decode_json(&mut parser, pool)?;
        parser.finish_document()?;
        Ok(value)
    }

    #[test]
    fn protocol_default_is_valid_and_round_trips_through_the_parameter() {
        let policy = SettlementRouterPolicyV1::protocol_default();
        policy.validate().expect("the protocol default is valid");
        let custom = policy.into_custom_parameter();
        assert_eq!(custom.id(), &SettlementRouterPolicyV1::parameter_id());
        assert_eq!(
            SettlementRouterPolicyV1::from_custom_parameter(&custom),
            Some(Ok(policy))
        );
        let other = CustomParameter::new(
            "another_parameter".parse().expect("valid name"),
            Json::new(1_u32),
        );
        assert!(SettlementRouterPolicyV1::from_custom_parameter(&other).is_none());
        let bytes = policy.encode();
        assert_eq!(
            SettlementRouterPolicyV1::decode(&mut bytes.as_slice()).expect("decodes"),
            policy
        );
    }

    #[test]
    fn field_names_match_the_committed_payload() {
        let names = SettlementRouterPolicyV1::FIELD_NAMES;
        assert_eq!(names.len(), 7);
        let value = norito::json::to_value(&SettlementRouterPolicyV1::protocol_default())
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
        type Edit = fn(&mut SettlementRouterPolicyV1);
        let cases: [(Edit, SettlementRouterPolicyError); 8] = [
            (
                |policy| policy.twap_window_seconds = 0,
                SettlementRouterPolicyError::Zero("twap_window_seconds"),
            ),
            (
                |policy| policy.epsilon_bps = MAX_EPSILON_BPS + 1,
                SettlementRouterPolicyError::OutOfRange("epsilon_bps"),
            ),
            (
                |policy| policy.buffer_horizon_hours = 0,
                SettlementRouterPolicyError::Zero("buffer_horizon_hours"),
            ),
            (
                |policy| policy.buffer_alert_pct = 101,
                SettlementRouterPolicyError::ThresholdOrder,
            ),
            (
                |policy| policy.buffer_throttle_pct = policy.buffer_alert_pct,
                SettlementRouterPolicyError::ThresholdOrder,
            ),
            (
                |policy| policy.buffer_xor_only_pct = policy.buffer_throttle_pct,
                SettlementRouterPolicyError::ThresholdOrder,
            ),
            (
                |policy| policy.buffer_halt_pct = policy.buffer_xor_only_pct,
                SettlementRouterPolicyError::ThresholdOrder,
            ),
            (
                |policy| {
                    policy.buffer_alert_pct = 3;
                    policy.buffer_throttle_pct = 2;
                    policy.buffer_xor_only_pct = 1;
                    policy.buffer_halt_pct = 1;
                },
                SettlementRouterPolicyError::ThresholdOrder,
            ),
        ];
        for (edit, expected) in cases {
            let mut policy = SettlementRouterPolicyV1::protocol_default();
            edit(&mut policy);
            assert_eq!(policy.validate(), Err(expected));
        }
        let mut boundary = SettlementRouterPolicyV1::protocol_default();
        boundary.epsilon_bps = MAX_EPSILON_BPS;
        boundary.buffer_alert_pct = 100;
        boundary.buffer_throttle_pct = 2;
        boundary.buffer_xor_only_pct = 1;
        boundary.buffer_halt_pct = 0;
        boundary
            .validate()
            .expect("the boundary values are accepted");
    }

    #[test]
    fn payload_with_an_unknown_missing_or_duplicate_field_is_malformed() {
        let default = SettlementRouterPolicyV1::protocol_default();
        let mut value = norito::json::to_value(&default).expect("the policy serializes");
        value
            .as_object_mut()
            .expect("a policy object")
            .insert("per_unknown".to_owned(), norito::json::Value::from(1_u64));
        assert!(matches!(
            SettlementRouterPolicyV1::from_payload(&Json::from(&value)),
            Err(SettlementRouterPolicyError::Malformed(_))
        ));
        for field in SettlementRouterPolicyV1::FIELD_NAMES {
            let mut value = norito::json::to_value(&default).expect("the policy serializes");
            assert!(
                value
                    .as_object_mut()
                    .expect("a policy object")
                    .remove(*field)
                    .is_some()
            );
            assert!(matches!(
                SettlementRouterPolicyV1::from_payload(&Json::from(&value)),
                Err(SettlementRouterPolicyError::Malformed(_))
            ));
        }
        let source = norito::json::to_json(&default).expect("the policy serializes");
        let duplicate = source.replacen('{', "{\"epsilon_bps\":25,", 1);
        assert!(norito::json::from_str::<SettlementRouterPolicyV1>(&duplicate).is_err());
        let invalid = SettlementRouterPolicyV1 {
            twap_window_seconds: 0,
            ..default
        };
        assert_eq!(
            SettlementRouterPolicyV1::from_payload(&Json::new(invalid)),
            Err(SettlementRouterPolicyError::Zero("twap_window_seconds"))
        );
    }

    #[test]
    fn admitted_decode_equals_ordinary_decode_and_retains_nothing() {
        let policy = SettlementRouterPolicyV1 {
            twap_window_seconds: 90,
            epsilon_bps: 40,
            ..SettlementRouterPolicyV1::protocol_default()
        };
        let source = norito::json::to_json(&policy).expect("the policy serializes");
        let pool = AllocationBudget::new(0);
        assert_eq!(decode_admitted(&source, &pool).expect("decodes"), policy);
        assert_eq!(pool.reserved_bytes(), 0);
        assert_eq!(
            norito::json::from_str::<SettlementRouterPolicyV1>(&source).expect("decodes"),
            policy
        );
        // An escaped key needs scratch from the caller's pool and returns it.
        let escaped = source.replacen("\"epsilon_bps\"", "\"\\u0065psilon_bps\"", 1);
        assert!(decode_admitted(&escaped, &pool).is_err());
        let funded = AllocationBudget::new(4096);
        assert_eq!(decode_admitted(&escaped, &funded).expect("decodes"), policy);
        assert_eq!(funded.reserved_bytes(), 0);
        let unknown = source.replacen('{', "{\"unknown\":1,", 1);
        assert!(decode_admitted(&unknown, &funded).is_err());
        assert_eq!(funded.reserved_bytes(), 0);
    }
}
