//! Strict public binding and resource bounds for stream-token admission.
use super::*;
pub(super) fn decode_policy_digest(
    value: Option<&str>,
    field: &'static str,
    emitter: &mut Emitter<ParseError>,
) -> Option<[u8; 32]> {
    value.and_then(|value| {
        let canonical = value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
        if !canonical {
            emitter.emit(Report::new(ParseError::InvalidSorafsConfig).attach(format!(
                "sorafs.storage.stream_tokens.{field} must be exactly 64 lowercase hexadecimal characters"
            )));
            return None;
        }
        let digest: [u8; 32] = hex::decode(value)
            .expect("validated lowercase policy digest hex")
            .try_into()
            .expect("validated 32-byte policy digest");
        if digest == [0; 32] {
            emitter.emit(Report::new(ParseError::InvalidSorafsConfig).attach(format!(
                "sorafs.storage.stream_tokens.{field} must be non-zero"
            )));
            return None;
        }
        Some(digest)
    })
}
pub(super) fn validate_binding_and_bounds(
    config: &SorafsStreamTokenConfig,
    emitter: &mut Emitter<ParseError>,
) {
    if config.enabled {
        match config.admission_provider_handle.as_deref() {
            Some(handle) if is_production_runtime_handle(handle) => {}
            Some(_) => emitter.emit(Report::new(ParseError::InvalidSorafsConfig).attach(
                "sorafs.storage.stream_tokens.admission_provider_handle must be a canonical credential-free production runtime handle",
            )),
            None => emitter.emit(Report::new(ParseError::InvalidSorafsConfig).attach(
                "sorafs.storage.stream_tokens.admission_provider_handle is required when issuance is enabled",
            )),
        }
        match config.admission_provider_revision {
            Some(0) => emitter.emit(Report::new(ParseError::InvalidSorafsConfig).attach(
                "sorafs.storage.stream_tokens.admission_provider_revision must be non-zero",
            )),
            None => emitter.emit(Report::new(ParseError::InvalidSorafsConfig).attach(
                "sorafs.storage.stream_tokens.admission_provider_revision is required when issuance is enabled",
            )),
            Some(_) => {}
        }
        if config.admission_provider_policy_digest_hex.is_none() {
            emitter.emit(Report::new(ParseError::InvalidSorafsConfig).attach(
                "sorafs.storage.stream_tokens.admission_provider_policy_digest_hex is required when issuance is enabled",
            ));
        }
    }
    for (field, value, maximum) in [
        (
            "admission_max_pending",
            config.admission_max_pending,
            1_000_000,
        ),
        (
            "admission_max_tracked_tokens",
            config.admission_max_tracked_tokens,
            1_000_000,
        ),
        (
            "admission_reconcile_max_items",
            config.admission_reconcile_max_items,
            1_024,
        ),
    ] {
        if value == 0 || value > maximum {
            emitter.emit(Report::new(ParseError::InvalidSorafsConfig).attach(format!(
                "sorafs.storage.stream_tokens.{field} must be within 1..={maximum}"
            )));
        }
    }
    if !(1..=60_000).contains(&config.admission_reconcile_interval_ms) {
        emitter.emit(Report::new(ParseError::InvalidSorafsConfig).attach(
            "sorafs.storage.stream_tokens.admission_reconcile_interval_ms must be within 1..=60000",
        ));
    }
    if !(1..=60_000).contains(&config.admission_operation_timeout_ms) {
        emitter.emit(Report::new(ParseError::InvalidSorafsConfig).attach(
            "sorafs.storage.stream_tokens.admission_operation_timeout_ms must be within 1..=60000",
        ));
    }
    if config.admission_lease_ttl_ms == 0 || config.admission_lease_ttl_ms > 300_000 {
        emitter.emit(Report::new(ParseError::InvalidSorafsConfig).attach(
            "sorafs.storage.stream_tokens.admission_lease_ttl_ms must be within 1..=300000",
        ));
    }
}

/// Explicit native gateway transaction custody and independent clock policy.
#[derive(Debug, Default, ReadConfig, Clone, norito::JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct SorafsStreamTokenGatewayNativeConfig {
    /// Direct Ed25519 operator account selected by the governed gateway policy.
    pub operator: Option<AccountId>,
    /// Absolute owner-only canonical operator private-key credential path.
    pub operator_credential: Option<PathBuf>,
    /// Independent direct Ed25519 observer selected by the governed gateway policy.
    pub observer: Option<AccountId>,
    /// Distinct absolute owner-only observer private-key credential path.
    pub observer_credential: Option<PathBuf>,
    /// Independent direct Ed25519 recorder for consensus-owned reputation Append intents.
    pub reputation_recorder: Option<AccountId>,
    /// Distinct absolute owner-only recorder private-key credential path.
    pub reputation_recorder_credential: Option<PathBuf>,
    /// Explicit Norito JSON fee intent for operator/observer actions; Append fees are governed.
    pub fee_payment_json: Option<String>,
    /// Closed uncertainty on either side of a fresh UTC sample, within 0..=5000 ms.
    pub clock_uncertainty_ms: Option<u64>,
}
impl SorafsStreamTokenGatewayNativeConfig {
    fn is_configured(&self) -> bool {
        self.operator.is_some()
            || self.operator_credential.is_some()
            || self.observer.is_some()
            || self.observer_credential.is_some()
            || self.reputation_recorder.is_some()
            || self.reputation_recorder_credential.is_some()
            || self.fee_payment_json.is_some()
            || self.clock_uncertainty_ms.is_some()
    }
    pub(super) fn parse(
        &self,
        enabled: bool,
        emitter: &mut Emitter<ParseError>,
    ) -> Option<actual::SorafsStreamTokenGatewayNativeConfig> {
        if !enabled {
            if self.is_configured() {
                emitter.emit(Report::new(ParseError::InvalidSorafsConfig).attach(
                    "sorafs.storage.stream_tokens.admission_native is forbidden while issuance is disabled",
                ));
            }
            return None;
        }
        let valid = || {
            let operator = self.operator.as_ref()?;
            let observer = self.observer.as_ref()?;
            let reputation_recorder = self.reputation_recorder.as_ref()?;
            let recorder_key = reputation_recorder.try_signatory()?;
            let operator_key = operator.try_signatory()?;
            let observer_key = observer.try_signatory()?;
            if operator_key == recorder_key
                || observer_key == recorder_key
                || recorder_key.algorithm() != iroha_crypto::Algorithm::Ed25519
                || operator_key == observer_key
                || operator_key.algorithm() != iroha_crypto::Algorithm::Ed25519
                || observer_key.algorithm() != iroha_crypto::Algorithm::Ed25519
            {
                return None;
            }
            let operator_credential = self.operator_credential.as_ref()?;
            let observer_credential = self.observer_credential.as_ref()?;
            let reputation_recorder_credential = self.reputation_recorder_credential.as_ref()?;
            if operator_credential == reputation_recorder_credential
                || observer_credential == reputation_recorder_credential
                || operator_credential == observer_credential
                || [
                    operator_credential,
                    observer_credential,
                    reputation_recorder_credential,
                ]
                .iter()
                .any(|path| {
                    !path.is_absolute()
                        || path.components().any(|component| {
                            matches!(
                                component,
                                std::path::Component::CurDir | std::path::Component::ParentDir
                            )
                        })
                })
            {
                return None;
            }
            let fee_payment: iroha_data_model::transaction::FeePaymentIntent =
                norito::json::from_str(self.fee_payment_json.as_deref()?).ok()?;
            fee_payment.validate().ok()?;
            // Explicit even when zero: the daemon must not silently claim a perfect host clock.
            let clock_uncertainty_ms = self.clock_uncertainty_ms?;
            if clock_uncertainty_ms > 5_000 {
                return None;
            }
            Some(actual::SorafsStreamTokenGatewayNativeConfig {
                operator: operator.clone(),
                operator_credential: operator_credential.clone(),
                observer: observer.clone(),
                observer_credential: observer_credential.clone(),
                reputation_recorder: reputation_recorder.clone(),
                reputation_recorder_credential: reputation_recorder_credential.clone(),
                fee_payment,
                clock_uncertainty_ms,
            })
        };
        let result = valid();
        if result.is_none() {
            emitter.emit(Report::new(ParseError::InvalidSorafsConfig).attach(
                "sorafs.storage.stream_tokens.admission_native is required when issuance is enabled and requires independent direct Ed25519 operator, observer and reputation recorder accounts, distinct absolute credential paths, explicit valid fee intent, and explicit clock uncertainty within 0..=5000 milliseconds",
            ));
        }
        result
    }
}
