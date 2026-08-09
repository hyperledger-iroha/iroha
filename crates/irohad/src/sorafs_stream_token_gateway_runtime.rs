//! Qualified stream-token gateway admission and durable callback reconciliation.
//!
//! The external provider owns atomic quota admission, sealed monotonic gateway
//! sequencing, active concurrency leases, and the ordered callback outbox. This
//! module only derives the public launch binding, performs an exact startup
//! readback, and supervises replay into the committed reputation runtime.

use std::{fmt, sync::Arc, time::Duration};

use iroha_config::parameters::actual::SorafsTokenConfig;
use iroha_data_model::{ChainId, sorafs::reputation::derive_stream_token_gateway_id_v1};
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal};
use iroha_torii::sorafs::{
    StreamTokenAdmissionCaptureV1, StreamTokenGatewayAdmissionErrorV1,
    StreamTokenGatewayAdmissionProviderV1, StreamTokenGatewayAdmissionQualificationV1,
};
use sorafs_node::reputation::runtime::ReputationNativeOutcomeAdmissionApiV1;

const SHUTDOWN_WAIT: Duration = Duration::from_secs(2);

/// Fail-closed launcher error without runtime credentials or evidence payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StreamTokenGatewayRuntimeErrorV1 {
    /// A provider was injected while stream-token issuance is disabled.
    UnexpectedProvider,
    /// Enabled issuance has no deployment-owned admission provider.
    MissingProvider,
    /// Enabled issuance has no active committed reputation callback.
    MissingReputationCallback,
    /// The configured compliance gateway identity is absent or malformed.
    InvalidGatewayIdentity,
    /// The configured public provider binding is incomplete.
    IncompleteBinding,
    /// Provider qualification, readback, or callback admission failed.
    Admission(StreamTokenGatewayAdmissionErrorV1),
    /// The reconciliation cadence is zero.
    InvalidReconcileInterval,
}

impl fmt::Display for StreamTokenGatewayRuntimeErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnexpectedProvider => {
                "disabled stream-token issuance rejects an unexpected gateway admission provider"
            }
            Self::MissingProvider => {
                "enabled stream-token issuance requires a deployment-owned gateway admission provider"
            }
            Self::MissingReputationCallback => {
                "enabled stream-token issuance requires an active committed reputation callback"
            }
            Self::InvalidGatewayIdentity => {
                "enabled stream-token issuance requires a canonical compliance gateway identity"
            }
            Self::IncompleteBinding => {
                "enabled stream-token issuance has an incomplete gateway admission binding"
            }
            Self::Admission(_) => {
                "stream-token gateway admission qualification or durable reconciliation failed"
            }
            Self::InvalidReconcileInterval => {
                "stream-token gateway reconciliation interval must be non-zero"
            }
        })
    }
}

impl std::error::Error for StreamTokenGatewayRuntimeErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Admission(error) => Some(error),
            _ => None,
        }
    }
}

impl From<StreamTokenGatewayAdmissionErrorV1> for StreamTokenGatewayRuntimeErrorV1 {
    fn from(error: StreamTokenGatewayAdmissionErrorV1) -> Self {
        Self::Admission(error)
    }
}

/// Build and live-qualify the exact capture boundary requested by config.
///
/// The first pending readback is reconciled synchronously. Consequently Torii
/// cannot begin serving while a stale, substituted, or malformed durable
/// callback prefix exists.
pub(crate) fn prepare_capture(
    chain_id: &ChainId,
    tokens: &SorafsTokenConfig,
    compliance_gateway_id: Option<&str>,
    provider: Option<Arc<dyn StreamTokenGatewayAdmissionProviderV1>>,
    reputation: Option<Arc<dyn ReputationNativeOutcomeAdmissionApiV1>>,
) -> Result<Option<Arc<StreamTokenAdmissionCaptureV1>>, StreamTokenGatewayRuntimeErrorV1> {
    if !tokens.enabled {
        return if provider.is_none() {
            Ok(None)
        } else {
            Err(StreamTokenGatewayRuntimeErrorV1::UnexpectedProvider)
        };
    }
    let provider = provider.ok_or(StreamTokenGatewayRuntimeErrorV1::MissingProvider)?;
    let reputation =
        reputation.ok_or(StreamTokenGatewayRuntimeErrorV1::MissingReputationCallback)?;
    let handle = tokens
        .admission_provider_handle
        .as_ref()
        .ok_or(StreamTokenGatewayRuntimeErrorV1::IncompleteBinding)?;
    let revision = tokens
        .admission_provider_revision
        .ok_or(StreamTokenGatewayRuntimeErrorV1::IncompleteBinding)?;
    let policy_digest = tokens
        .admission_provider_policy_digest
        .ok_or(StreamTokenGatewayRuntimeErrorV1::IncompleteBinding)?;
    let gateway_id = derive_stream_token_gateway_id_v1(
        chain_id,
        compliance_gateway_id.ok_or(StreamTokenGatewayRuntimeErrorV1::InvalidGatewayIdentity)?,
    )
    .map_err(|_| StreamTokenGatewayRuntimeErrorV1::InvalidGatewayIdentity)?;
    let qualification = StreamTokenGatewayAdmissionQualificationV1 {
        gateway_id,
        revision,
        policy_digest,
        max_pending: tokens.admission_max_pending,
        max_tracked_tokens: tokens.admission_max_tracked_tokens,
        lease_ttl_ms: tokens.admission_lease_ttl_ms,
    };
    let capture = Arc::new(StreamTokenAdmissionCaptureV1::try_new(
        handle.clone(),
        qualification,
        tokens.admission_reconcile_max_items,
        provider,
        reputation,
    )?);
    capture.reconcile_pending()?;
    Ok(Some(capture))
}

/// Start bounded replay of externally durable callbacks after startup.
pub(crate) fn start_reconciler(
    capture: Arc<StreamTokenAdmissionCaptureV1>,
    poll_interval: Duration,
    shutdown_signal: ShutdownSignal,
) -> Result<Child, StreamTokenGatewayRuntimeErrorV1> {
    if poll_interval.is_zero() {
        return Err(StreamTokenGatewayRuntimeErrorV1::InvalidReconcileInterval);
    }
    let task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(poll_interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        interval.tick().await;
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let tick_capture = Arc::clone(&capture);
                    match tokio::task::spawn_blocking(move || tick_capture.reconcile_pending()).await {
                        Ok(Ok(_)) => {}
                        Ok(Err(error)) if is_transient(error) => {
                            iroha_logger::warn!(
                                ?error,
                                "stream-token gateway callback reconciliation is temporarily unavailable"
                            );
                        }
                        Ok(Err(error)) => {
                            iroha_logger::error!(
                                ?error,
                                "stream-token gateway callback reconciliation failed closed"
                            );
                            shutdown_signal.send();
                            return;
                        }
                        Err(error) => {
                            iroha_logger::error!(
                                ?error,
                                "stream-token gateway callback reconciliation task panicked"
                            );
                            shutdown_signal.send();
                            return;
                        }
                    }
                }
                () = shutdown_signal.receive() => return,
            }
        }
    });
    Ok(Child::new(task, OnShutdown::Wait(SHUTDOWN_WAIT)))
}

const fn is_transient(error: StreamTokenGatewayAdmissionErrorV1) -> bool {
    matches!(
        error,
        StreamTokenGatewayAdmissionErrorV1::Unavailable
            | StreamTokenGatewayAdmissionErrorV1::Ambiguous
            | StreamTokenGatewayAdmissionErrorV1::ReputationCallback
    )
}

#[cfg(test)]
mod tests {
    use iroha_data_model::sorafs::{
        capacity::ProviderId,
        reputation::{PorTerminalOutcomeV1, StreamTokenValidationOutcomeV1},
    };
    use iroha_torii::sorafs::{
        StreamTokenGatewayAdmissionAckV1, StreamTokenGatewayAdmissionReadbackV1,
        StreamTokenGatewayAdmissionRecordV1, StreamTokenGatewayAdmissionRequestV1,
        StreamTokenGatewayAdmissionResultV1,
    };
    use sorafs_node::reputation::runtime::{
        ReputationJournalEnqueueOutcomeV1, ReputationNativeOutcomeAdmissionStateV1,
        ReputationRuntimeError, StreamTokenReputationAdmissionOutcomeV1,
    };

    use super::*;

    const HANDLE: &str = "sealed://sorafs/stream-admission/eu-1";

    fn token_config() -> SorafsTokenConfig {
        SorafsTokenConfig {
            enabled: true,
            signer_handle: Some("pkcs11:prod/sorafs/stream-token/eu-1".to_owned()),
            signer_public_key: Some([
                0x15, 0x09, 0xA6, 0x11, 0xAD, 0x6D, 0x97, 0xB0, 0x1D, 0x87, 0x1E, 0x58, 0xED, 0x00,
                0xC8, 0xFD, 0x7C, 0x39, 0x17, 0xB6, 0xCA, 0x61, 0xA8, 0xC2, 0x83, 0x3A, 0x19, 0xE0,
                0x00, 0xAA, 0xC2, 0xE4,
            ]),
            signer_revision: Some(4),
            signer_policy_digest: Some([0xb4; 32]),
            admission_provider_handle: Some(HANDLE.to_owned()),
            admission_provider_revision: Some(7),
            admission_provider_policy_digest: Some([0x42; 32]),
            admission_max_pending: 64,
            admission_max_tracked_tokens: 32,
            admission_reconcile_max_items: 16,
            admission_lease_ttl_ms: 120_000,
            ..SorafsTokenConfig::default()
        }
    }

    fn qualification(revision: u64) -> StreamTokenGatewayAdmissionQualificationV1 {
        StreamTokenGatewayAdmissionQualificationV1 {
            gateway_id: derive_stream_token_gateway_id_v1(
                &ChainId::from("iroha3-taira"),
                "gateway.dxb-1",
            )
            .expect("canonical gateway identity"),
            revision,
            policy_digest: [0x42; 32],
            max_pending: 64,
            max_tracked_tokens: 32,
            lease_ttl_ms: 120_000,
        }
    }

    #[derive(Debug)]
    struct ProviderProbe {
        qualification: StreamTokenGatewayAdmissionQualificationV1,
        readback: StreamTokenGatewayAdmissionReadbackV1,
    }

    impl StreamTokenGatewayAdmissionProviderV1 for ProviderProbe {
        fn handle(&self) -> &str {
            HANDLE
        }

        fn qualification(
            &self,
        ) -> Result<StreamTokenGatewayAdmissionQualificationV1, StreamTokenGatewayAdmissionErrorV1>
        {
            Ok(self.qualification)
        }

        fn admit(
            &self,
            _request: &StreamTokenGatewayAdmissionRequestV1,
        ) -> Result<StreamTokenGatewayAdmissionResultV1, StreamTokenGatewayAdmissionErrorV1>
        {
            Err(StreamTokenGatewayAdmissionErrorV1::Rejected)
        }

        fn pending(
            &self,
            _max_items: u32,
        ) -> Result<StreamTokenGatewayAdmissionReadbackV1, StreamTokenGatewayAdmissionErrorV1>
        {
            Ok(self.readback.clone())
        }

        fn acknowledge(
            &self,
            _record: StreamTokenGatewayAdmissionRecordV1,
        ) -> Result<StreamTokenGatewayAdmissionAckV1, StreamTokenGatewayAdmissionErrorV1> {
            Err(StreamTokenGatewayAdmissionErrorV1::Rejected)
        }

        fn release_lease(
            &self,
            _record: StreamTokenGatewayAdmissionRecordV1,
        ) -> Result<StreamTokenGatewayAdmissionAckV1, StreamTokenGatewayAdmissionErrorV1> {
            Err(StreamTokenGatewayAdmissionErrorV1::Rejected)
        }
    }

    #[derive(Debug)]
    struct ReputationProbe;

    impl ReputationNativeOutcomeAdmissionApiV1 for ReputationProbe {
        fn activation_state(
            &self,
        ) -> Result<ReputationNativeOutcomeAdmissionStateV1, ReputationRuntimeError> {
            Ok(ReputationNativeOutcomeAdmissionStateV1::Active)
        }

        fn record_por_terminal(
            &self,
            _provider_id: ProviderId,
            _outcome: PorTerminalOutcomeV1,
        ) -> Result<ReputationJournalEnqueueOutcomeV1, ReputationRuntimeError> {
            Err(ReputationRuntimeError::InvalidRuntimePolicy)
        }

        fn record_authenticated_stream_token_validation(
            &self,
            _provider_id: ProviderId,
            _outcome: StreamTokenValidationOutcomeV1,
        ) -> Result<StreamTokenReputationAdmissionOutcomeV1, ReputationRuntimeError> {
            Err(ReputationRuntimeError::InvalidRuntimePolicy)
        }
    }

    fn provider(
        revision: u64,
        acknowledged_through_sequence: u64,
        high_water_sequence: u64,
    ) -> Arc<dyn StreamTokenGatewayAdmissionProviderV1> {
        Arc::new(ProviderProbe {
            qualification: qualification(revision),
            readback: StreamTokenGatewayAdmissionReadbackV1 {
                acknowledged_through_sequence,
                high_water_sequence,
                records: Vec::new(),
            },
        })
    }

    fn reputation() -> Arc<dyn ReputationNativeOutcomeAdmissionApiV1> {
        Arc::new(ReputationProbe)
    }

    #[test]
    fn exact_configured_provider_is_live_qualified_before_launch() {
        let capture = prepare_capture(
            &ChainId::from("iroha3-taira"),
            &token_config(),
            Some("gateway.dxb-1"),
            Some(provider(7, 0, 0)),
            Some(reputation()),
        )
        .expect("qualified capture")
        .expect("enabled capture");
        capture
            .validate_expected_binding(HANDLE, qualification(7), 16)
            .expect("exact binding");
    }

    #[test]
    fn provider_revision_substitution_fails_startup() {
        let error = prepare_capture(
            &ChainId::from("iroha3-taira"),
            &token_config(),
            Some("gateway.dxb-1"),
            Some(provider(8, 0, 0)),
            Some(reputation()),
        )
        .expect_err("substituted revision must fail");
        assert_eq!(
            error,
            StreamTokenGatewayRuntimeErrorV1::Admission(
                StreamTokenGatewayAdmissionErrorV1::BindingMismatch
            )
        );
    }

    #[test]
    fn malformed_pending_readback_fails_before_torii_launch() {
        let error = prepare_capture(
            &ChainId::from("iroha3-taira"),
            &token_config(),
            Some("gateway.dxb-1"),
            Some(provider(7, 0, 1)),
            Some(reputation()),
        )
        .expect_err("omitted pending row must fail startup");
        assert_eq!(
            error,
            StreamTokenGatewayRuntimeErrorV1::Admission(
                StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome
            )
        );
    }

    #[test]
    fn disabled_service_rejects_injected_provider() {
        let error = prepare_capture(
            &ChainId::from("iroha3-taira"),
            &SorafsTokenConfig::default(),
            None,
            Some(provider(7, 0, 0)),
            None,
        )
        .expect_err("disabled provider injection must fail");
        assert_eq!(error, StreamTokenGatewayRuntimeErrorV1::UnexpectedProvider);
    }

    #[test]
    fn enabled_service_requires_active_reputation_callback() {
        let error = prepare_capture(
            &ChainId::from("iroha3-taira"),
            &token_config(),
            Some("gateway.dxb-1"),
            Some(provider(7, 0, 0)),
            None,
        )
        .expect_err("missing callback must fail");
        assert_eq!(
            error,
            StreamTokenGatewayRuntimeErrorV1::MissingReputationCallback
        );
    }
}
