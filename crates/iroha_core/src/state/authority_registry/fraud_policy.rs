//! Exact transaction-admission projection of the configured fraud policy.
//!
//! Transport endpoints and request timeouts only obtain external assessments.
//! Consensus admission reads the fields encoded here from signed transaction
//! metadata in `tx::enforce_fraud_policy`. The projection preserves attester
//! order because the first matching engine identity supplies the verifying key.

use iroha_config::parameters::actual::{FraudMonitoring, FraudRiskBand};
use iroha_crypto::PublicKey;
use norito::{Decode, Encode, NoritoSchema};

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, NoritoSchema)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha:state:fraud-monitoring-attester:v1")]
struct FraudAdmissionAttesterV1 {
    engine_id: String,
    public_key: PublicKey,
}

/// Canonical first-release fraud policy fields that can change transaction validity.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, NoritoSchema)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha:state:fraud_monitoring:v1")]
pub(super) struct FraudAdmissionPolicyV1 {
    enabled: bool,
    required_minimum_band: Option<u8>,
    missing_assessment_permitted: bool,
    attesters: Vec<FraudAdmissionAttesterV1>,
}

impl FraudAdmissionPolicyV1 {
    /// Borrow only fields read for deterministic admission by the transaction owner.
    pub(super) fn from_actual(config: &FraudMonitoring) -> Self {
        Self {
            enabled: config.enabled,
            required_minimum_band: config.required_minimum_band.map(|band| match band {
                FraudRiskBand::Low => 1,
                FraudRiskBand::Medium => 2,
                FraudRiskBand::High => 3,
                FraudRiskBand::Critical => 4,
            }),
            missing_assessment_permitted: !config.missing_assessment_grace.is_zero(),
            attesters: config
                .attesters
                .iter()
                .map(|attester| FraudAdmissionAttesterV1 {
                    engine_id: attester.engine_id.clone(),
                    public_key: attester.public_key.clone(),
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_config::parameters::actual::FraudAttester;
    use iroha_crypto::{Algorithm, KeyPair};
    use std::time::Duration;

    fn policy() -> FraudMonitoring {
        let first = KeyPair::from_seed(b"fraud-state-first".to_vec(), Algorithm::Ed25519);
        let second = KeyPair::from_seed(b"fraud-state-second".to_vec(), Algorithm::Ed25519);
        FraudMonitoring {
            enabled: true,
            required_minimum_band: Some(FraudRiskBand::Medium),
            missing_assessment_grace: Duration::ZERO,
            attesters: vec![
                FraudAttester {
                    engine_id: "risk-a".to_owned(),
                    public_key: first.public_key().clone(),
                },
                FraudAttester {
                    engine_id: "risk-b".to_owned(),
                    public_key: second.public_key().clone(),
                },
            ],
            ..FraudMonitoring::default()
        }
    }

    fn frame(policy: &FraudMonitoring) -> Vec<u8> {
        norito::encode_canonical(&FraudAdmissionPolicyV1::from_actual(policy)).unwrap()
    }

    #[test]
    fn fraud_admission_policy_roundtrips_with_explicit_v1_identity() {
        let projected = FraudAdmissionPolicyV1::from_actual(&policy());
        assert_eq!(
            FraudAdmissionPolicyV1::nominal_name(),
            "iroha:state:fraud_monitoring:v1"
        );
        let encoded = norito::encode_canonical(&projected).unwrap();
        assert_eq!(
            norito::decode_canonical::<FraudAdmissionPolicyV1>(&encoded).unwrap(),
            projected
        );
        let _ambient = norito::core::DecodeFlagsGuard::enter(0);
        assert_eq!(norito::encode_canonical(&projected).unwrap(), encoded);
    }

    #[test]
    fn fraud_admission_mutations_change_projection_but_transport_does_not() {
        let baseline = policy();
        let encoded = frame(&baseline);

        let mut changed = baseline.clone();
        changed.enabled = false;
        assert_ne!(frame(&changed), encoded);

        let mut changed = baseline.clone();
        changed.required_minimum_band = Some(FraudRiskBand::High);
        assert_ne!(frame(&changed), encoded);

        let mut changed = baseline.clone();
        changed.missing_assessment_grace = Duration::from_secs(1);
        assert_ne!(frame(&changed), encoded);
        changed.missing_assessment_grace = Duration::from_secs(9);
        assert_eq!(
            frame(&changed),
            frame(&FraudMonitoring {
                missing_assessment_grace: Duration::from_secs(1),
                ..baseline.clone()
            })
        );

        let mut changed = baseline.clone();
        changed.attesters[0].engine_id = "risk-c".to_owned();
        assert_ne!(frame(&changed), encoded);

        let mut changed = baseline.clone();
        changed.attesters[0].public_key = changed.attesters[1].public_key.clone();
        assert_ne!(frame(&changed), encoded);

        let mut changed = baseline.clone();
        changed.attesters.reverse();
        assert_ne!(frame(&changed), encoded);

        let mut transport = baseline;
        transport
            .service_endpoints
            .push("https://fraud.example/".parse().unwrap());
        transport.connect_timeout += Duration::from_secs(1);
        transport.request_timeout += Duration::from_secs(1);
        assert_eq!(frame(&transport), encoded);
    }
}
