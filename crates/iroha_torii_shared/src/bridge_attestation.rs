//! Bounded unsigned failure observations for the node-signed finality route.
//!
//! These observations never authenticate readiness. Only the successful attestation,
//! independently verified against the original node and genesis context, can do that.
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};

/// Maximum canonical error envelope accepted by a finality-attestation client.
pub const FINALITY_ATTESTATION_FAILURE_MAX_BYTES: usize = 2048;
/// Fixed error-envelope code carrying one exact finality failure detail.
pub const FINALITY_ATTESTATION_FAILURE_CODE: &str = "bridge_finality_attestation_failure";

/// Exact reason the requested node-signed finality statement cannot be produced.
///
/// JSON uses one case-sensitive scalar string naming the variant, such as
/// `"GenesisUncommitted"`. The CLI observation labels returned by [`Self::as_str`]
/// are a separate presentation field.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, NoritoDeserialize, NoritoSerialize, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_torii_shared::bridge_attestation::FinalityAttestationFailureReason")]
pub enum FinalityAttestationFailureReason {
    /// The consensus runtime has not published its initial status.
    ConsensusUninitialized,
    /// The immutable ledger view contains no committed genesis.
    GenesisUncommitted,
    /// The node requires recovery by restart.
    RestartRequired,
    /// The requested height differs from the committed durable tip.
    TipChanged,
    /// Committed state, runtime status or finality evidence disagree.
    ConflictingState,
    /// Required durable finality evidence is absent or unreadable.
    FinalityUnavailable,
    /// The node could not sign or represent the requested statement.
    InternalFailure,
}
impl FinalityAttestationFailureReason {
    const fn json_name(self) -> &'static str {
        match self {
            Self::ConsensusUninitialized => "ConsensusUninitialized",
            Self::GenesisUncommitted => "GenesisUncommitted",
            Self::RestartRequired => "RestartRequired",
            Self::TipChanged => "TipChanged",
            Self::ConflictingState => "ConflictingState",
            Self::FinalityUnavailable => "FinalityUnavailable",
            Self::InternalFailure => "InternalFailure",
        }
    }

    /// Required HTTP status for this exact failure; a different status is malformed.
    #[must_use]
    pub const fn http_status_code(self) -> u16 {
        match self {
            Self::ConsensusUninitialized
            | Self::GenesisUncommitted
            | Self::RestartRequired
            | Self::FinalityUnavailable => 503,
            Self::TipChanged | Self::ConflictingState => 409,
            Self::InternalFailure => 500,
        }
    }
    /// Stable bounded reason used by CLI observations.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ConsensusUninitialized => "consensus_uninitialized",
            Self::GenesisUncommitted => "genesis_uncommitted",
            Self::RestartRequired => "restart_required",
            Self::TipChanged => "tip_changed",
            Self::ConflictingState => "conflicting_state",
            Self::FinalityUnavailable => "finality_unavailable",
            Self::InternalFailure => "internal_failure",
        }
    }
    /// Readiness category. Only `pending` permits an overall-deadline-bounded retry.
    #[must_use]
    pub const fn readiness_state(self) -> &'static str {
        match self {
            Self::ConsensusUninitialized | Self::GenesisUncommitted => "pending",
            Self::RestartRequired => "restart_required",
            Self::TipChanged | Self::ConflictingState => "conflict",
            Self::FinalityUnavailable | Self::InternalFailure => "unavailable",
        }
    }
}

impl norito::json::JsonSerialize for FinalityAttestationFailureReason {
    fn json_serialize(&self, output: &mut String) {
        norito::json::write_json_string(self.json_name(), output);
    }

    fn json_serialize_to(
        &self,
        output: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        norito::json::write_json_string_to(self.json_name(), output)
    }
}

impl norito::json::JsonDeserialize for FinalityAttestationFailureReason {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        match parser.parse_string()?.as_str() {
            "ConsensusUninitialized" => Ok(Self::ConsensusUninitialized),
            "GenesisUncommitted" => Ok(Self::GenesisUncommitted),
            "RestartRequired" => Ok(Self::RestartRequired),
            "TipChanged" => Ok(Self::TipChanged),
            "ConflictingState" => Ok(Self::ConflictingState),
            "FinalityUnavailable" => Ok(Self::FinalityUnavailable),
            "InternalFailure" => Ok(Self::InternalFailure),
            _ => Err(norito::json::Error::InvalidField {
                field: "reason".to_owned(),
                message: "expected a canonical finality failure reason".to_owned(),
            }),
        }
    }
}

/// Canonical failure observation for one exact attestation request.
///
/// The echoed challenge and height reject stale/mixed responses; they are unsigned
/// and confer no node or consensus authority. This schema never carries a ready state.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    JsonDeserialize,
    JsonSerialize,
    NoritoDeserialize,
    NoritoSerialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_torii_shared::bridge_attestation::FinalityAttestationFailure")]
pub struct FinalityAttestationFailure {
    /// Nonzero challenge from the validated request header.
    pub challenge: [u8; 32],
    /// Exact requested durable-tip height.
    pub height: u64,
    /// Closed non-success reason.
    pub reason: FinalityAttestationFailureReason,
    /// Exact selector and observed heights, present only for `TipChanged`.
    pub tip_mismatch: Option<crate::bridge_finality::BridgeFinalityAttestationTipMismatchV1>,
}

impl FinalityAttestationFailure {
    /// Check the closed reason/payload shape without granting node authority.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.height > 0
            && self.challenge != [0; 32]
            && match (&self.reason, &self.tip_mismatch) {
                (FinalityAttestationFailureReason::TipChanged, Some(progress)) => {
                    progress.is_valid()
                        && progress.requested_height == self.height
                        && progress.challenge == self.challenge
                }
                (FinalityAttestationFailureReason::TipChanged, None) | (_, Some(_)) => false,
                (_, None) => true,
            }
    }

    /// Place this observation in the canonical HTTP error boundary's sole detail.
    #[must_use]
    pub fn into_error_envelope(self) -> crate::ErrorEnvelope {
        crate::ErrorEnvelope::new(
            FINALITY_ATTESTATION_FAILURE_CODE,
            "The requested finality attestation is unavailable.",
        )
        .with_details(crate::ErrorDetails {
            finality_attestation_failure: Some(self),
            ..crate::ErrorDetails::default()
        })
    }

    /// Validate this unsigned observation against the request and selected identity.
    ///
    /// Only exact changing-tip observations carry progress. Every other reason
    /// must omit that payload; neither shape authenticates successful finality.
    #[must_use]
    pub fn matches(
        &self,
        height: u64,
        challenge: [u8; 32],
        node_id: &iroha_model_base::peer::PeerId,
        network_id: iroha_data_model::NetworkId,
    ) -> bool {
        self.is_valid()
            && self.height == height
            && height > 0
            && self.challenge == challenge
            && challenge != [0; 32]
            && match (&self.reason, &self.tip_mismatch) {
                (FinalityAttestationFailureReason::TipChanged, Some(progress)) => {
                    progress.matches(height, challenge, node_id, network_id)
                }
                (FinalityAttestationFailureReason::TipChanged, None) | (_, Some(_)) => false,
                (_, None) => true,
            }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failure_reason_json_is_one_closed_scalar_in_both_writers() {
        use FinalityAttestationFailureReason::*;
        for (reason, name) in [
            (ConsensusUninitialized, "ConsensusUninitialized"),
            (GenesisUncommitted, "GenesisUncommitted"),
            (RestartRequired, "RestartRequired"),
            (TipChanged, "TipChanged"),
            (ConflictingState, "ConflictingState"),
            (FinalityUnavailable, "FinalityUnavailable"),
            (InternalFailure, "InternalFailure"),
        ] {
            let expected = format!("\"{name}\"");
            assert_eq!(norito::json::to_json(&reason).unwrap(), expected);
            assert_eq!(
                norito::json::to_json_bounded(&reason, expected.len()).unwrap(),
                expected
            );
            assert!(norito::json::to_json_bounded(&reason, expected.len() - 1).is_err());
            assert_eq!(
                norito::json::from_str::<FinalityAttestationFailureReason>(&expected).unwrap(),
                reason
            );
        }
        for invalid in [
            r#""Ready""#,
            r#""genesis_uncommitted""#,
            r#""genesisuncommitted""#,
            r#"" GenesisUncommitted""#,
            r#"{"reason":"GenesisUncommitted"}"#,
            r#"{"GenesisUncommitted":null}"#,
            r#"["GenesisUncommitted"]"#,
            "null",
            "0",
            "false",
        ] {
            assert!(
                norito::json::from_str::<FinalityAttestationFailureReason>(invalid).is_err(),
                "unexpected reason representation: {invalid}"
            );
        }
    }

    fn tip_progress() -> crate::bridge_finality::BridgeFinalityAttestationTipMismatchV1 {
        let key =
            iroha_crypto::KeyPair::try_from_seed(vec![71; 32], iroha_crypto::Algorithm::BlsNormal)
                .unwrap();
        crate::bridge_finality::BridgeFinalityAttestationTipMismatchV1 {
            requested_height: 1,
            applied_height: 2,
            status_height: 2,
            challenge: [71; 32],
            node_id: iroha_model_base::peer::PeerId::new(key.public_key().clone()),
            network_id: iroha_data_model::NetworkId::from_genesis_hash(
                iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::new(
                    b"failure bindings",
                )),
            ),
        }
    }
    #[test]
    fn failure_binding_requires_exact_selector_and_exclusive_progress() {
        let progress = tip_progress();
        let valid = FinalityAttestationFailure {
            height: 1,
            challenge: [71; 32],
            reason: FinalityAttestationFailureReason::TipChanged,
            tip_mismatch: Some(progress.clone()),
        };
        assert!(valid.matches(1, [71; 32], &progress.node_id, progress.network_id));
        assert!(!valid.matches(2, [71; 32], &progress.node_id, progress.network_id));
        assert!(!valid.matches(1, [72; 32], &progress.node_id, progress.network_id));
        let mut missing = valid.clone();
        missing.tip_mismatch = None;
        assert!(!missing.matches(1, [71; 32], &progress.node_id, progress.network_id));
        let mut mixed = valid.clone();
        mixed.reason = FinalityAttestationFailureReason::GenesisUncommitted;
        assert!(!mixed.matches(1, [71; 32], &progress.node_id, progress.network_id));
        mixed.tip_mismatch = None;
        assert!(mixed.matches(1, [71; 32], &progress.node_id, progress.network_id));
        for (height, challenge) in [(0, [71; 32]), (1, [0; 32])] {
            let invalid = FinalityAttestationFailure {
                height,
                challenge,
                ..mixed.clone()
            };
            assert!(!invalid.matches(height, challenge, &progress.node_id, progress.network_id));
        }
    }
    #[test]
    fn failure_contract_preserves_every_non_retryable_reason() {
        use FinalityAttestationFailureReason::*;
        let cases = [
            (
                ConsensusUninitialized,
                503,
                "consensus_uninitialized",
                "pending",
            ),
            (GenesisUncommitted, 503, "genesis_uncommitted", "pending"),
            (RestartRequired, 503, "restart_required", "restart_required"),
            (TipChanged, 409, "tip_changed", "conflict"),
            (ConflictingState, 409, "conflicting_state", "conflict"),
            (
                FinalityUnavailable,
                503,
                "finality_unavailable",
                "unavailable",
            ),
            (InternalFailure, 500, "internal_failure", "unavailable"),
        ];
        for (reason, status, name, state) in cases {
            assert_eq!(reason.http_status_code(), status);
            assert_eq!(reason.as_str(), name);
            assert_eq!(reason.readiness_state(), state);
            let value = FinalityAttestationFailure {
                challenge: [71; 32],
                height: 1,
                reason,
                tip_mismatch: (reason == TipChanged).then(tip_progress),
            };
            let envelope = value.clone().into_error_envelope();
            let json = norito::json::to_json(&envelope).expect("failure envelope JSON");
            assert_eq!(
                norito::json::to_json_bounded(&envelope, json.len()).unwrap(),
                json
            );
            assert!(norito::json::to_json_bounded(&envelope, json.len() - 1).is_err());
            let json_envelope: crate::ErrorEnvelope = norito::json::from_str(&json).unwrap();
            assert_eq!(json_envelope.code(), FINALITY_ATTESTATION_FAILURE_CODE);
            let mut json_details = json_envelope.details.unwrap();
            assert_eq!(
                json_details.finality_attestation_failure.take(),
                Some(value.clone())
            );
            assert!(json_details.is_empty());
            let wire = norito::to_bytes(&envelope).expect("canonical failure envelope");
            assert!(wire.len() <= FINALITY_ATTESTATION_FAILURE_MAX_BYTES);
            let decoded: crate::ErrorEnvelope = norito::decode_canonical_with_limits(
                &wire,
                norito::canonical_decode_limits(wire.len()),
            )
            .expect("failure roundtrip");
            assert_eq!(decoded.code(), FINALITY_ATTESTATION_FAILURE_CODE);
            let mut details = decoded.details.unwrap();
            assert_eq!(details.finality_attestation_failure.take(), Some(value));
            assert!(details.is_empty());
        }
    }
    #[test]
    fn failure_json_rejects_unknown_reason_fields_and_missing_binding() {
        let value = FinalityAttestationFailure {
            challenge: [91; 32],
            height: 1,
            reason: FinalityAttestationFailureReason::GenesisUncommitted,
            tip_mismatch: None,
        };
        let wire = norito::json::to_json(&value).expect("current failure JSON");
        assert!(wire.contains("GenesisUncommitted"));
        let unknown_reason = wire.replace("GenesisUncommitted", "Ready");
        assert!(norito::json::from_str::<FinalityAttestationFailure>(&unknown_reason).is_err());
        let with_extra = format!("{},\"extra\":true}}", wire.strip_suffix('}').unwrap());
        assert!(norito::json::from_str::<FinalityAttestationFailure>(&with_extra).is_err());
        let missing_height = wire.replace("\"height\":1,", "");
        assert_ne!(missing_height, wire);
        assert!(norito::json::from_str::<FinalityAttestationFailure>(&missing_height).is_err());
    }
}
