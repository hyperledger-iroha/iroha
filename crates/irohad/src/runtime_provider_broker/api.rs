//! Launcher-facing lifecycle, registry, backend-injection, and server boundary.
//!
//! This module owns only public provider bindings and injected runtime adapters;
//! it never loads credentials, private keys, or endpoint overrides.

#[cfg(any(target_os = "linux", target_os = "macos"))]
use super::protocol;
use crate::{
    IrohaRuntimeDeps,
    runtime_provider_registry::{
        IrohaRuntimeProviderBindingsV1, IrohaRuntimeProviderRegistryErrorV1,
        IrohaRuntimeProviderRegistryV1, IrohaRuntimeProviderSlotV1,
        resolve_runtime_deps_from_bindings,
    },
};
use std::{fmt, sync::Arc};

const BROKER_LIFECYCLE_STARTING_V1: u8 = 0;
#[cfg(any(target_os = "linux", target_os = "macos"))]
const BROKER_LIFECYCLE_READY_V1: u8 = 1;
const BROKER_LIFECYCLE_STOPPING_V1: u8 = 2;

/// One-shot lifecycle control shared by a broker launcher and serving thread.
///
/// Readiness publication and shutdown are linearized through a bounded
/// callback gate plus one atomic state. A shutdown request that wins while the
/// server is starting prevents the readiness callback and short-circuits
/// startup before backend qualification when it is already present at entry.
#[derive(Debug)]
pub struct RuntimeProviderBrokerLifecycleV1 {
    state: std::sync::atomic::AtomicU8,
    readiness_publication_gate: std::sync::Mutex<()>,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    active_provider_calls: std::sync::atomic::AtomicUsize,
}

impl RuntimeProviderBrokerLifecycleV1 {
    /// Construct a fresh one-shot lifecycle in the starting state.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: std::sync::atomic::AtomicU8::new(BROKER_LIFECYCLE_STARTING_V1),
            readiness_publication_gate: std::sync::Mutex::new(()),
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            active_provider_calls: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Request orderly shutdown without waiting for in-flight provider calls.
    ///
    /// The serving call closes accepted local transports and joins every
    /// session before it returns. A provider qualification already in progress
    /// or an operation already admitted when this method linearizes is allowed
    /// to finish because the synchronous V1 provider traits do not expose
    /// cancellation. Operation admission is the final atomic check immediately
    /// before dispatch; it can precede the actual trait-method call by a small
    /// in-process interval.
    ///
    /// This call waits for a readiness callback that already owns the bounded
    /// publication gate. The callback must therefore be bounded and must not
    /// call `request_shutdown` reentrantly.
    pub fn request_shutdown(&self) {
        let _publication = self
            .readiness_publication_gate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.state.store(
            BROKER_LIFECYCLE_STOPPING_V1,
            std::sync::atomic::Ordering::SeqCst,
        );
    }

    /// Return whether orderly shutdown has been requested or begun.
    #[must_use]
    pub fn shutdown_requested(&self) -> bool {
        self.state.load(std::sync::atomic::Ordering::SeqCst) == BROKER_LIFECYCLE_STOPPING_V1
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    pub(super) fn publish_ready<R>(&self, on_ready: R) -> bool
    where
        R: FnOnce(),
    {
        let _publication = self
            .readiness_publication_gate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if self
            .state
            .compare_exchange(
                BROKER_LIFECYCLE_STARTING_V1,
                BROKER_LIFECYCLE_READY_V1,
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
            )
            .is_err()
        {
            return false;
        }
        on_ready();
        true
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    pub(super) fn try_begin_qualification(
        self: &Arc<Self>,
    ) -> Option<RuntimeProviderBrokerCallPermitV1> {
        if self.state.load(std::sync::atomic::Ordering::SeqCst) == BROKER_LIFECYCLE_STOPPING_V1 {
            return None;
        }
        self.active_provider_calls
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        if self.state.load(std::sync::atomic::Ordering::SeqCst) == BROKER_LIFECYCLE_STOPPING_V1 {
            self.active_provider_calls
                .fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
            return None;
        }
        Some(RuntimeProviderBrokerCallPermitV1 {
            lifecycle: Arc::clone(self),
        })
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    pub(super) fn try_begin_operation(
        self: &Arc<Self>,
    ) -> Option<RuntimeProviderBrokerCallPermitV1> {
        if self.state.load(std::sync::atomic::Ordering::SeqCst) != BROKER_LIFECYCLE_READY_V1 {
            return None;
        }
        self.active_provider_calls
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        if self.state.load(std::sync::atomic::Ordering::SeqCst) != BROKER_LIFECYCLE_READY_V1 {
            self.active_provider_calls
                .fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
            return None;
        }
        Some(RuntimeProviderBrokerCallPermitV1 {
            lifecycle: Arc::clone(self),
        })
    }

    #[cfg(all(any(target_os = "linux", target_os = "macos"), test))]
    pub(super) fn active_provider_call_count(&self) -> usize {
        self.active_provider_calls
            .load(std::sync::atomic::Ordering::Acquire)
    }
}

impl Default for RuntimeProviderBrokerLifecycleV1 {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(super) struct RuntimeProviderBrokerCallPermitV1 {
    lifecycle: Arc<RuntimeProviderBrokerLifecycleV1>,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl Drop for RuntimeProviderBrokerCallPermitV1 {
    fn drop(&mut self) {
        self.lifecycle
            .active_provider_calls
            .fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
    }
}

/// Stock platform-fixed runtime-provider registry used by `main_entry`.
///
/// Construction performs no I/O. The registry connects to the fixed local
/// endpoint only when the validated public binding catalog is non-empty.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct StockRuntimeProviderBrokerRegistryV1;

impl StockRuntimeProviderBrokerRegistryV1 {
    /// Construct the stock registry without connecting to the broker.
    #[must_use]
    pub(crate) const fn new() -> Self {
        Self
    }
}

const fn stock_runtime_provider_slot_is_supported(slot: IrohaRuntimeProviderSlotV1) -> bool {
    let wire_id = slot.wire_id();
    wire_id >= IrohaRuntimeProviderSlotV1::ModerationQuarantineKeyWrapper.wire_id()
        && wire_id <= IrohaRuntimeProviderSlotV1::ModerationPanelNotificationArchive.wire_id()
}

impl IrohaRuntimeProviderRegistryV1 for StockRuntimeProviderBrokerRegistryV1 {
    fn resolve(
        &self,
        bindings: &IrohaRuntimeProviderBindingsV1,
    ) -> Result<IrohaRuntimeDeps, IrohaRuntimeProviderRegistryErrorV1> {
        if bindings.is_empty() {
            return Ok(IrohaRuntimeDeps::default());
        }

        // The frozen V1 wire ids are a contiguous, exhaustively tested
        // whitelist. A future role outside that closed interval fails until
        // this registry and its bounded protocol surface are extended.
        if bindings
            .iter()
            .any(|binding| !stock_runtime_provider_slot_is_supported(binding.slot()))
        {
            return Err(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution);
        }

        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            protocol::resolve(bindings)
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = bindings;
            Err(IrohaRuntimeProviderRegistryErrorV1::Unavailable)
        }
    }
}

/// Stock broker-backed registry for the packaged Governance DAG service.
///
/// The chain identity is public and participates in the broker's exact-catalog
/// handshake. Construction performs no I/O and stores no credential or private
/// key material.
#[derive(Clone, Debug)]
pub struct StockGovernanceDagServiceRuntimeProviderRegistryV1 {
    chain_id: iroha_data_model::ChainId,
}

impl StockGovernanceDagServiceRuntimeProviderRegistryV1 {
    /// Construct a standalone-service registry for one canonical chain.
    #[must_use]
    pub const fn new(chain_id: iroha_data_model::ChainId) -> Self {
        Self { chain_id }
    }
}

impl sorafs_node::GovernanceDagServiceRuntimeProviderRegistryV1
    for StockGovernanceDagServiceRuntimeProviderRegistryV1
{
    fn resolve(
        &self,
        service: &sorafs_node::GovernanceDagServiceRuntimeProviderBindingsV1,
    ) -> Result<
        sorafs_node::GovernanceDagServiceRuntimeProviders,
        sorafs_node::GovernanceDagServiceRuntimeProviderRegistryErrorV1,
    > {
        let bindings = IrohaRuntimeProviderBindingsV1::try_from_governance_dag_service(
            &self.chain_id,
            service,
        )
        .map_err(map_governance_service_registry_error)?;
        let dependencies = resolve_runtime_deps_from_bindings(
            &bindings,
            Some(&StockRuntimeProviderBrokerRegistryV1::new()),
        )
        .map_err(map_governance_service_registry_error)?;
        let ipfs_authenticator = dependencies
            .sorafs_governance_dag_ipfs_authenticator
            .ok_or(
                sorafs_node::GovernanceDagServiceRuntimeProviderRegistryErrorV1::RejectedBindings,
            )?;
        let checkpoint_store = dependencies.sorafs_governance_dag_checkpoint_store.ok_or(
            sorafs_node::GovernanceDagServiceRuntimeProviderRegistryErrorV1::RejectedBindings,
        )?;
        let mut providers = sorafs_node::GovernanceDagServiceRuntimeProviders::default()
            .with_ipfs_authenticator(ipfs_authenticator)
            .with_checkpoint_store(checkpoint_store);
        if let Some(head_authenticator) = dependencies.sorafs_governance_dag_head_authenticator {
            providers = providers.with_head_authenticator(head_authenticator);
        }
        Ok(providers)
    }
}

fn map_governance_service_registry_error(
    error: IrohaRuntimeProviderRegistryErrorV1,
) -> sorafs_node::GovernanceDagServiceRuntimeProviderRegistryErrorV1 {
    use sorafs_node::GovernanceDagServiceRuntimeProviderRegistryErrorV1 as ServiceError;

    match error {
        IrohaRuntimeProviderRegistryErrorV1::Unavailable
        | IrohaRuntimeProviderRegistryErrorV1::MissingRegistry => ServiceError::Unavailable,
        IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked => ServiceError::StaleOrRevoked,
        IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(_)
        | IrohaRuntimeProviderRegistryErrorV1::BindingMismatch
        | IrohaRuntimeProviderRegistryErrorV1::TestProviderRejected
        | IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution
        | IrohaRuntimeProviderRegistryErrorV1::UnexpectedProviders => {
            ServiceError::RejectedBindings
        }
    }
}

#[cfg(test)]
mod governance_service_registry_tests {
    use super::*;
    use sorafs_node::GovernanceDagServiceRuntimeProviderRegistryErrorV1 as ServiceError;

    #[test]
    fn stock_registry_whitelists_every_frozen_v1_slot() {
        use IrohaRuntimeProviderSlotV1 as Slot;

        let slots = [
            Slot::ModerationQuarantineKeyWrapper,
            Slot::PrivacyCyclePrfProvider,
            Slot::PrivacyReleaseAnchor,
            Slot::TransparencyLeaderLease,
            Slot::FencedPrivacyPublisher,
            Slot::FencedPrivacyHeadReader,
            Slot::GovernanceDagSigner,
            Slot::GovernanceDagIpfsAuthenticator,
            Slot::GovernanceDagHeadAuthenticator,
            Slot::GovernanceDagCheckpointStore,
            Slot::StreamTokenSigner,
            Slot::AppealFinanceTransactionSigner,
            Slot::AppealFinanceCheckpoint,
            Slot::ProofOutcomeTransactionSigner,
            Slot::RepairTransactionSigner,
            Slot::ReserveTransactionSigner,
            Slot::OrderbookTransactionSigner,
            Slot::ModerationTransactionSigner,
            Slot::ModerationSettlementHandoff,
            Slot::ModerationPublicationHandoff,
            Slot::ModerationPanelNotification,
            Slot::EvidenceViewerWebAuthn,
            Slot::EvidenceViewerGrantAuthority,
            Slot::EvidenceViewerReceiptSigner,
            Slot::EvidenceViewerErasure,
            Slot::EvidenceViewerCheckpointStore,
            Slot::PopCredentialProviderRegistry,
            Slot::PotrGatewaySigner,
            Slot::PotrProviderSigner,
            Slot::GatewayAcmeClient,
            Slot::GatewayComplianceFeedTransport,
            Slot::ReputationJournalTransactionSubmitter,
            Slot::ReputationThresholdSigner,
            Slot::ReputationGovernanceDag,
            Slot::BillingFinalizedQuery,
            Slot::BillingJournalVerifier,
            Slot::BillingStatementSigner,
            Slot::BillingStatementPublisher,
            Slot::BillingAcknowledgementAuthority,
            Slot::BillingEpochWitnessStore,
            Slot::ProviderIngestAuthenticatedSource,
            Slot::ProviderIngestCompletionSignerResolver,
            Slot::ProviderIngestCompletionSigner,
            Slot::ProviderIngestCheckpointStore,
            Slot::ProviderIngestRetentionAuthority,
            Slot::PorFinalizedReplayArchive,
            Slot::EvidenceViewerCompactionArchive,
            Slot::ReputationFinalizedArchiveRetentionAuthority,
            Slot::SoracloudRuntimeMutationSigner,
            Slot::ReputationJournalCheckpoint,
            Slot::SoracloudHfInferenceCredentialProvider,
            Slot::ModerationCheckpointStore,
            Slot::EvidenceViewerTransparencyPublisher,
            Slot::StreamTokenGatewayAdmission,
            Slot::ModerationPanelNotificationArchive,
        ];
        assert_eq!(slots.len(), 55);
        for (index, slot) in slots.into_iter().enumerate() {
            assert_eq!(usize::from(slot.wire_id()), index + 1);
            assert!(stock_runtime_provider_slot_is_supported(slot));
        }
    }

    #[test]
    fn registry_errors_map_to_payload_free_service_categories() {
        for error in [
            IrohaRuntimeProviderRegistryErrorV1::Unavailable,
            IrohaRuntimeProviderRegistryErrorV1::MissingRegistry,
        ] {
            assert_eq!(
                map_governance_service_registry_error(error),
                ServiceError::Unavailable
            );
        }
        assert_eq!(
            map_governance_service_registry_error(
                IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked
            ),
            ServiceError::StaleOrRevoked
        );
        for error in [
            IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                IrohaRuntimeProviderSlotV1::GovernanceDagIpfsAuthenticator,
            ),
            IrohaRuntimeProviderRegistryErrorV1::BindingMismatch,
            IrohaRuntimeProviderRegistryErrorV1::TestProviderRejected,
            IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution,
            IrohaRuntimeProviderRegistryErrorV1::UnexpectedProviders,
        ] {
            assert_eq!(
                map_governance_service_registry_error(error),
                ServiceError::RejectedBindings
            );
        }
    }
}

/// Runtime-only backends injected into the stock local broker server.
///
/// The value contains no credential loader, key material, endpoint discovery,
/// or built-in implementation. A deployment-owned launcher must inject each
/// backend requested by its public binding catalog. Startup rejects missing,
/// extra, substituted, stale, revoked, and test-marked bindings.
#[derive(Clone, Default)]
pub struct RuntimeProviderBrokerBackendsV1 {
    pub(super) moderation_quarantine_key_wrapper:
        Option<Arc<dyn sorafs_node::ModerationQuarantineKeyWrapper>>,
    pub(super) privacy_cycle_prf_provider:
        Option<Arc<dyn sorafs_node::ProductionPrivacyCyclePrfProviderV1>>,
    pub(super) privacy_release_anchor: Option<Arc<dyn sorafs_node::ProductionPrivacyReleaseAnchorV1>>,
    pub(super) transparency_leader_lease_provider:
        Option<Arc<dyn sorafs_node::ProductionTransparencyLeaderLeaseProviderV1>>,
    pub(super) fenced_privacy_publisher: Option<Arc<dyn sorafs_node::FencedTransparencyPublisherV1>>,
    pub(super) fenced_privacy_head_reader:
        Option<Arc<dyn sorafs_node::FencedTransparencyAuthoritativeHeadReaderV1>>,
    pub(super) governance_dag_signer: Option<Arc<dyn sorafs_node::GovernanceDagRuntimeSigner>>,
    pub(super) governance_dag_ipfs_authenticator:
        Option<Arc<dyn sorafs_node::GovernanceDagRequestAuthenticator>>,
    pub(super) governance_dag_head_authenticator:
        Option<Arc<dyn sorafs_node::GovernanceDagRequestAuthenticator>>,
    pub(super) governance_dag_checkpoint_store:
        Option<Arc<dyn sorafs_node::GovernanceDagSealedCheckpointStore>>,
    pub(super) stream_token_signer: Option<Arc<dyn iroha_torii::sorafs::StreamTokenRuntimeSigner>>,
    pub(super) stream_token_gateway_admission:
        Option<Arc<dyn iroha_torii::sorafs::StreamTokenGatewayAdmissionProviderV1>>,
    pub(super) appeal_finance_transaction_signers:
        Vec<Arc<dyn iroha_torii::SoraFsAppealFinanceTransactionSigner>>,
    pub(super) appeal_finance_checkpoint:
        Option<
            Arc<
                dyn sorafs_node::appeal_finance_transaction_forwarder::
                    AppealFinanceCheckpointRuntime,
            >,
        >,
    pub(super) proof_outcome_transaction_signer:
        Option<Arc<dyn iroha_torii::SoraFsProofOutcomeTransactionSigner>>,
    pub(super) repair_transaction_signer: Option<Arc<dyn iroha_torii::SoraFsRepairTransactionSigner>>,
    pub(super) reserve_transaction_signer: Option<Arc<dyn iroha_torii::SoraFsReserveTransactionSigner>>,
    pub(super) orderbook_transaction_signer: Option<Arc<dyn iroha_torii::SoraFsOrderbookTransactionSigner>>,
    pub(super) moderation_transaction_signer: Option<
        Arc<
            dyn iroha_torii::sorafs::moderation_runtime::ModerationSignedTransactionSignerV1,
        >,
    >,
    pub(super) moderation_settlement_handoff: Option<
        Arc<dyn iroha_torii::sorafs::moderation_runtime::ModerationDurableHandoffBoundaryV1>,
    >,
    pub(super) moderation_publication_handoff: Option<
        Arc<dyn iroha_torii::sorafs::moderation_runtime::ModerationDurableHandoffBoundaryV1>,
    >,
    pub(super) moderation_panel_notification: Option<
        Arc<
            dyn iroha_torii::sorafs::moderation_runtime::
                ModerationDurablePanelNotificationBoundaryV1,
        >,
    >,
    pub(super) moderation_checkpoint_store:
        Option<Arc<dyn sorafs_node::moderation_orchestrator::ModerationCheckpointStoreV1>>,
    pub(super) moderation_panel_notification_archive: Option<
        Arc<
            dyn sorafs_node::moderation_orchestrator::
                ModerationPanelNotificationArchiveV1,
        >,
    >,
    pub(super) provider_ingest_authenticated_source: Option<
        Arc<
            dyn crate::sorafs_provider_ingest_runtime::
                ProviderIngestAuthenticatedSourceRuntimeV1,
        >,
    >,
    pub(super) provider_ingest_signer_resolver: Option<
        Arc<
            dyn crate::sorafs_provider_ingest_runtime::
                ProviderIngestGovernedSignerResolverRuntimeV1,
        >,
    >,
    pub(super) provider_ingest_checkpoint_store:
        Option<Arc<dyn sorafs_node::ProviderIngestCheckpointRuntimeV1>>,
    pub(super) provider_ingest_retention_authority: Option<
        Arc<
            dyn iroha_core::query::provider_ingest_finalized::
                ProviderIngestFinalizedArchiveRetentionAuthorityV1,
        >,
    >,
    pub(super) reputation_finalized_archive_retention_authority: Option<
        Arc<
            dyn iroha_core::query::reputation_finalized::
                ReputationFinalizedArchiveRetentionAuthorityV1,
        >,
    >,
    pub(super) reputation_journal_transaction_submitter: Option<
        Arc<dyn sorafs_node::reputation::runtime::ReputationJournalTransactionSubmitterV1>,
    >,
    pub(super) reputation_journal_checkpoint: Option<
        Arc<dyn sorafs_node::reputation::runtime::ReputationJournalCheckpointRuntimeV1>,
    >,
    pub(super) reputation_threshold_signer: Option<
        Arc<dyn sorafs_node::reputation::runtime::ReputationThresholdSignerClientV1>,
    >,
    pub(super) reputation_governance_dag: Option<
        Arc<dyn sorafs_node::reputation::runtime::ReputationGovernanceDagClientV1>,
    >,
    pub(super) billing_finalized_query:
        Option<Arc<dyn sorafs_node::hedging_billing_service::HedgingBillingFinalizedQuery>>,
    pub(super) billing_journal_verifier:
        Option<Arc<dyn sorafs_node::hedging_billing_service::HedgingBillingJournalVerifier>>,
    pub(super) billing_statement_signer:
        Option<Arc<dyn sorafs_node::hedging_billing_service::BillingStatementRuntimeSigner>>,
    pub(super) billing_statement_publisher:
        Option<Arc<dyn sorafs_node::hedging_billing_service::BillingStatementPublisher>>,
    pub(super) billing_acknowledgement_authority: Option<
        Arc<
            dyn sorafs_node::hedging_billing_service::
                BillingStatementAcknowledgementAuthority,
        >,
    >,
    pub(super) billing_epoch_witness_store:
        Option<Arc<dyn sorafs_node::hedging_billing_service::HedgingBillingEpochWitnessStore>>,
    pub(super) pop_credential_provider_registry: Option<
        Arc<
            dyn iroha_torii::sorafs::pop_api::PopCredentialRuntimeProviderRegistryV1,
        >,
    >,
    pub(super) potr_gateway_signer: Option<Arc<dyn iroha_torii::sorafs::PotrGatewaySignerV1>>,
    pub(super) potr_provider_signer: Option<Arc<dyn iroha_torii::sorafs::PotrProviderSignerV1>>,
    pub(super) gateway_acme_client:
        Option<Arc<dyn iroha_torii::sorafs::gateway::AcmeClient>>,
    pub(super) gateway_compliance_feed_transport: Option<
        Arc<
            dyn iroha_torii::sorafs::gateway::
                GatewayComplianceFeedTransport,
        >,
    >,
    pub(super) por_finalized_replay_archive:
        Option<Arc<dyn sorafs_node::PorFinalizedReplayArchiveV1>>,
    pub(super) evidence_viewer_webauthn:
        Option<Arc<dyn sorafs_node::evidence_viewer::EvidenceViewerWebAuthnBoundaryV1>>,
    pub(super) evidence_viewer_grants:
        Option<Arc<dyn sorafs_node::evidence_viewer::EvidenceViewerGrantBoundaryV1>>,
    pub(super) evidence_viewer_receipt_signer:
        Option<Arc<dyn sorafs_node::evidence_viewer::EvidenceViewerReceiptSignerV1>>,
    pub(super) evidence_viewer_erasure:
        Option<Arc<dyn sorafs_node::evidence_viewer::EvidenceViewerErasureBoundaryV1>>,
    pub(super) evidence_viewer_checkpoint_store:
        Option<Arc<dyn sorafs_node::evidence_viewer::EvidenceViewerCheckpointStoreV1>>,
    pub(super) evidence_viewer_compaction_archive:
        Option<Arc<dyn sorafs_node::evidence_viewer::EvidenceViewerCompactionArchiveV1>>,
    pub(super) evidence_viewer_transparency_publisher: Option<
        Arc<
            dyn sorafs_node::evidence_viewer::transparency_producer::
                EvidenceViewerTransparencyPublisherV1,
        >,
    >,
    pub(super) soracloud_runtime_mutation_signer:
        Option<Arc<dyn crate::soracloud_runtime_signer::SoracloudRuntimeMutationSignerV1>>,
    pub(super) soracloud_hf_inference_credential_provider: Option<
        Arc<dyn crate::soracloud_hf_credential::SoracloudHfInferenceCredentialProviderV1>,
    >,
}

impl fmt::Debug for RuntimeProviderBrokerBackendsV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeProviderBrokerBackendsV1")
            .field(
                "moderation_quarantine_key_wrapper",
                &self.moderation_quarantine_key_wrapper.is_some(),
            )
            .field(
                "privacy_cycle_prf_provider",
                &self.privacy_cycle_prf_provider.is_some(),
            )
            .field(
                "privacy_release_anchor",
                &self.privacy_release_anchor.is_some(),
            )
            .field(
                "transparency_leader_lease_provider",
                &self.transparency_leader_lease_provider.is_some(),
            )
            .field(
                "fenced_privacy_publisher",
                &self.fenced_privacy_publisher.is_some(),
            )
            .field(
                "fenced_privacy_head_reader",
                &self.fenced_privacy_head_reader.is_some(),
            )
            .field(
                "governance_dag_signer",
                &self.governance_dag_signer.is_some(),
            )
            .field(
                "governance_dag_ipfs_authenticator",
                &self.governance_dag_ipfs_authenticator.is_some(),
            )
            .field(
                "governance_dag_head_authenticator",
                &self.governance_dag_head_authenticator.is_some(),
            )
            .field(
                "governance_dag_checkpoint_store",
                &self.governance_dag_checkpoint_store.is_some(),
            )
            .field("stream_token_signer", &self.stream_token_signer.is_some())
            .field(
                "stream_token_gateway_admission",
                &self.stream_token_gateway_admission.is_some(),
            )
            .field(
                "appeal_finance_transaction_signer_count",
                &self.appeal_finance_transaction_signers.len(),
            )
            .field(
                "appeal_finance_checkpoint",
                &self.appeal_finance_checkpoint.is_some(),
            )
            .field(
                "proof_outcome_transaction_signer",
                &self.proof_outcome_transaction_signer.is_some(),
            )
            .field(
                "repair_transaction_signer",
                &self.repair_transaction_signer.is_some(),
            )
            .field(
                "reserve_transaction_signer",
                &self.reserve_transaction_signer.is_some(),
            )
            .field(
                "orderbook_transaction_signer",
                &self.orderbook_transaction_signer.is_some(),
            )
            .field(
                "moderation_transaction_signer",
                &self.moderation_transaction_signer.is_some(),
            )
            .field(
                "moderation_settlement_handoff",
                &self.moderation_settlement_handoff.is_some(),
            )
            .field(
                "moderation_publication_handoff",
                &self.moderation_publication_handoff.is_some(),
            )
            .field(
                "moderation_panel_notification",
                &self.moderation_panel_notification.is_some(),
            )
            .field(
                "moderation_checkpoint_store",
                &self.moderation_checkpoint_store.is_some(),
            )
            .field(
                "moderation_panel_notification_archive",
                &self.moderation_panel_notification_archive.is_some(),
            )
            .field(
                "provider_ingest_authenticated_source",
                &self.provider_ingest_authenticated_source.is_some(),
            )
            .field(
                "provider_ingest_signer_resolver",
                &self.provider_ingest_signer_resolver.is_some(),
            )
            .field(
                "provider_ingest_checkpoint_store",
                &self.provider_ingest_checkpoint_store.is_some(),
            )
            .field(
                "provider_ingest_retention_authority",
                &self.provider_ingest_retention_authority.is_some(),
            )
            .field(
                "reputation_finalized_archive_retention_authority",
                &self
                    .reputation_finalized_archive_retention_authority
                    .is_some(),
            )
            .field(
                "reputation_journal_transaction_submitter",
                &self.reputation_journal_transaction_submitter.is_some(),
            )
            .field(
                "reputation_journal_checkpoint",
                &self.reputation_journal_checkpoint.is_some(),
            )
            .field(
                "reputation_threshold_signer",
                &self.reputation_threshold_signer.is_some(),
            )
            .field(
                "reputation_governance_dag",
                &self.reputation_governance_dag.is_some(),
            )
            .field(
                "billing_finalized_query",
                &self.billing_finalized_query.is_some(),
            )
            .field(
                "billing_journal_verifier",
                &self.billing_journal_verifier.is_some(),
            )
            .field(
                "billing_statement_signer",
                &self.billing_statement_signer.is_some(),
            )
            .field(
                "billing_statement_publisher",
                &self.billing_statement_publisher.is_some(),
            )
            .field(
                "billing_acknowledgement_authority",
                &self.billing_acknowledgement_authority.is_some(),
            )
            .field(
                "billing_epoch_witness_store",
                &self.billing_epoch_witness_store.is_some(),
            )
            .field(
                "pop_credential_provider_registry",
                &self.pop_credential_provider_registry.is_some(),
            )
            .field("potr_gateway_signer", &self.potr_gateway_signer.is_some())
            .field("potr_provider_signer", &self.potr_provider_signer.is_some())
            .field("gateway_acme_client", &self.gateway_acme_client.is_some())
            .field(
                "gateway_compliance_feed_transport",
                &self.gateway_compliance_feed_transport.is_some(),
            )
            .field(
                "por_finalized_replay_archive",
                &self.por_finalized_replay_archive.is_some(),
            )
            .field(
                "evidence_viewer_webauthn",
                &self.evidence_viewer_webauthn.is_some(),
            )
            .field(
                "evidence_viewer_grants",
                &self.evidence_viewer_grants.is_some(),
            )
            .field(
                "evidence_viewer_receipt_signer",
                &self.evidence_viewer_receipt_signer.is_some(),
            )
            .field(
                "evidence_viewer_erasure",
                &self.evidence_viewer_erasure.is_some(),
            )
            .field(
                "evidence_viewer_checkpoint_store",
                &self.evidence_viewer_checkpoint_store.is_some(),
            )
            .field(
                "evidence_viewer_compaction_archive",
                &self.evidence_viewer_compaction_archive.is_some(),
            )
            .field(
                "evidence_viewer_transparency_publisher",
                &self.evidence_viewer_transparency_publisher.is_some(),
            )
            .field(
                "soracloud_runtime_mutation_signer",
                &self.soracloud_runtime_mutation_signer.is_some(),
            )
            .field(
                "soracloud_hf_inference_credential_provider",
                &self.soracloud_hf_inference_credential_provider.is_some(),
            )
            .finish()
    }
}

impl RuntimeProviderBrokerBackendsV1 {
    /// Construct an empty injection set.
    ///
    /// The server accepts this only for an empty catalog. Every requested
    /// production role must be attached explicitly before serving.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            moderation_quarantine_key_wrapper: None,
            privacy_cycle_prf_provider: None,
            privacy_release_anchor: None,
            transparency_leader_lease_provider: None,
            fenced_privacy_publisher: None,
            fenced_privacy_head_reader: None,
            governance_dag_signer: None,
            governance_dag_ipfs_authenticator: None,
            governance_dag_head_authenticator: None,
            governance_dag_checkpoint_store: None,
            stream_token_signer: None,
            stream_token_gateway_admission: None,
            appeal_finance_transaction_signers: Vec::new(),
            appeal_finance_checkpoint: None,
            proof_outcome_transaction_signer: None,
            repair_transaction_signer: None,
            reserve_transaction_signer: None,
            orderbook_transaction_signer: None,
            moderation_transaction_signer: None,
            moderation_settlement_handoff: None,
            moderation_publication_handoff: None,
            moderation_panel_notification: None,
            moderation_checkpoint_store: None,
            moderation_panel_notification_archive: None,
            provider_ingest_authenticated_source: None,
            provider_ingest_signer_resolver: None,
            provider_ingest_checkpoint_store: None,
            provider_ingest_retention_authority: None,
            reputation_finalized_archive_retention_authority: None,
            reputation_journal_transaction_submitter: None,
            reputation_journal_checkpoint: None,
            reputation_threshold_signer: None,
            reputation_governance_dag: None,
            billing_finalized_query: None,
            billing_journal_verifier: None,
            billing_statement_signer: None,
            billing_statement_publisher: None,
            billing_acknowledgement_authority: None,
            billing_epoch_witness_store: None,
            pop_credential_provider_registry: None,
            potr_gateway_signer: None,
            potr_provider_signer: None,
            gateway_acme_client: None,
            gateway_compliance_feed_transport: None,
            por_finalized_replay_archive: None,
            evidence_viewer_webauthn: None,
            evidence_viewer_grants: None,
            evidence_viewer_receipt_signer: None,
            evidence_viewer_erasure: None,
            evidence_viewer_checkpoint_store: None,
            evidence_viewer_compaction_archive: None,
            evidence_viewer_transparency_publisher: None,
            soracloud_runtime_mutation_signer: None,
            soracloud_hf_inference_credential_provider: None,
        }
    }

    /// Attach the deployment-owned Soracloud transaction and provenance signer.
    #[must_use]
    pub fn with_soracloud_runtime_mutation_signer(
        mut self,
        signer: Arc<dyn crate::soracloud_runtime_signer::SoracloudRuntimeMutationSignerV1>,
    ) -> Self {
        self.soracloud_runtime_mutation_signer = Some(signer);
        self
    }

    /// Attach the deployment-owned authenticated HF credential provider.
    #[must_use]
    pub fn with_soracloud_hf_inference_credential_provider(
        mut self,
        provider: Arc<dyn crate::soracloud_hf_credential::SoracloudHfInferenceCredentialProviderV1>,
    ) -> Self {
        self.soracloud_hf_inference_credential_provider = Some(provider);
        self
    }

    /// Attach the deployment-owned PKCS#11/KMS quarantine-DEK wrapper.
    #[must_use]
    pub fn with_moderation_quarantine_key_wrapper(
        mut self,
        key_wrapper: Arc<dyn sorafs_node::ModerationQuarantineKeyWrapper>,
    ) -> Self {
        self.moderation_quarantine_key_wrapper = Some(key_wrapper);
        self
    }

    /// Attach the deployment-owned threshold-PRF provider used for privacy cycles.
    #[must_use]
    pub fn with_privacy_cycle_prf_provider(
        mut self,
        provider: Arc<dyn sorafs_node::ProductionPrivacyCyclePrfProviderV1>,
    ) -> Self {
        self.privacy_cycle_prf_provider = Some(provider);
        self
    }

    /// Attach the independently administered finalized privacy-release anchor.
    #[must_use]
    pub fn with_privacy_release_anchor(
        mut self,
        anchor: Arc<dyn sorafs_node::ProductionPrivacyReleaseAnchorV1>,
    ) -> Self {
        self.privacy_release_anchor = Some(anchor);
        self
    }

    /// Attach the external sealed-CAS transparency leader-lease provider.
    #[must_use]
    pub fn with_transparency_leader_lease_provider(
        mut self,
        provider: Arc<dyn sorafs_node::ProductionTransparencyLeaderLeaseProviderV1>,
    ) -> Self {
        self.transparency_leader_lease_provider = Some(provider);
        self
    }

    /// Attach the deployment-owned fused privacy Governance target writer.
    #[must_use]
    pub fn with_fenced_privacy_publisher(
        mut self,
        publisher: Arc<dyn sorafs_node::FencedTransparencyPublisherV1>,
    ) -> Self {
        self.fenced_privacy_publisher = Some(publisher);
        self
    }

    /// Attach the authenticated fused-privacy authoritative-head reader.
    #[must_use]
    pub fn with_fenced_privacy_head_reader(
        mut self,
        reader: Arc<dyn sorafs_node::FencedTransparencyAuthoritativeHeadReaderV1>,
    ) -> Self {
        self.fenced_privacy_head_reader = Some(reader);
        self
    }

    /// Attach the deployment-owned Governance DAG HSM/KMS signer.
    #[must_use]
    pub fn with_governance_dag_signer(
        mut self,
        signer: Arc<dyn sorafs_node::GovernanceDagRuntimeSigner>,
    ) -> Self {
        self.governance_dag_signer = Some(signer);
        self
    }

    /// Attach the deployment-owned Governance DAG IPFS request-auth HSM.
    #[must_use]
    pub fn with_governance_dag_ipfs_authenticator(
        mut self,
        authenticator: Arc<dyn sorafs_node::GovernanceDagRequestAuthenticator>,
    ) -> Self {
        self.governance_dag_ipfs_authenticator = Some(authenticator);
        self
    }

    /// Attach the independently administered signed-head request-auth HSM.
    #[must_use]
    pub fn with_governance_dag_head_authenticator(
        mut self,
        authenticator: Arc<dyn sorafs_node::GovernanceDagRequestAuthenticator>,
    ) -> Self {
        self.governance_dag_head_authenticator = Some(authenticator);
        self
    }

    /// Attach the deployment-owned Governance DAG sealed checkpoint store.
    #[must_use]
    pub fn with_governance_dag_checkpoint_store(
        mut self,
        store: Arc<dyn sorafs_node::GovernanceDagSealedCheckpointStore>,
    ) -> Self {
        self.governance_dag_checkpoint_store = Some(store);
        self
    }

    /// Attach the deployment-owned stream-token Ed25519 signer.
    #[must_use]
    pub fn with_stream_token_signer(
        mut self,
        signer: Arc<dyn iroha_torii::sorafs::StreamTokenRuntimeSigner>,
    ) -> Self {
        self.stream_token_signer = Some(signer);
        self
    }

    /// Attach the deployment-owned stream-token quota, sealed-sequence, and
    /// ordered callback-outbox provider.
    #[must_use]
    pub fn with_stream_token_gateway_admission(
        mut self,
        provider: Arc<dyn iroha_torii::sorafs::StreamTokenGatewayAdmissionProviderV1>,
    ) -> Self {
        self.stream_token_gateway_admission = Some(provider);
        self
    }

    /// Attach one independently administered appeal-finance transaction signer.
    ///
    /// Call this once for every configured signer handle. Server startup
    /// rejects duplicates, missing providers, and extras.
    #[must_use]
    pub fn with_appeal_finance_transaction_signer(
        mut self,
        signer: Arc<dyn iroha_torii::SoraFsAppealFinanceTransactionSigner>,
    ) -> Self {
        self.appeal_finance_transaction_signers.push(signer);
        self
    }

    /// Attach the appeal-finance HSM signer and sealed monotonic checkpoint store.
    #[must_use]
    pub fn with_appeal_finance_checkpoint(
        mut self,
        checkpoint: Arc<
            dyn sorafs_node::appeal_finance_transaction_forwarder::AppealFinanceCheckpointRuntime,
        >,
    ) -> Self {
        self.appeal_finance_checkpoint = Some(checkpoint);
        self
    }

    /// Attach the independently administered proof-outcome transaction signer.
    #[must_use]
    pub fn with_proof_outcome_transaction_signer(
        mut self,
        signer: Arc<dyn iroha_torii::SoraFsProofOutcomeTransactionSigner>,
    ) -> Self {
        self.proof_outcome_transaction_signer = Some(signer);
        self
    }

    /// Attach the independently administered native repair transaction signer.
    #[must_use]
    pub fn with_repair_transaction_signer(
        mut self,
        signer: Arc<dyn iroha_torii::SoraFsRepairTransactionSigner>,
    ) -> Self {
        self.repair_transaction_signer = Some(signer);
        self
    }

    /// Attach the independently administered reserve/rent transaction signer.
    #[must_use]
    pub fn with_reserve_transaction_signer(
        mut self,
        signer: Arc<dyn iroha_torii::SoraFsReserveTransactionSigner>,
    ) -> Self {
        self.reserve_transaction_signer = Some(signer);
        self
    }

    /// Attach the independently administered orderbook transaction signer.
    #[must_use]
    pub fn with_orderbook_transaction_signer(
        mut self,
        signer: Arc<dyn iroha_torii::SoraFsOrderbookTransactionSigner>,
    ) -> Self {
        self.orderbook_transaction_signer = Some(signer);
        self
    }

    /// Attach the independently administered moderation transaction signer.
    #[must_use]
    pub fn with_moderation_transaction_signer(
        mut self,
        signer: Arc<
            dyn iroha_torii::sorafs::moderation_runtime::ModerationSignedTransactionSignerV1,
        >,
    ) -> Self {
        self.moderation_transaction_signer = Some(signer);
        self
    }

    /// Attach the durable exactly-once moderation settlement boundary.
    #[must_use]
    pub fn with_moderation_settlement_handoff(
        mut self,
        boundary: Arc<
            dyn iroha_torii::sorafs::moderation_runtime::ModerationDurableHandoffBoundaryV1,
        >,
    ) -> Self {
        self.moderation_settlement_handoff = Some(boundary);
        self
    }

    /// Attach the durable exactly-once moderation publication boundary.
    #[must_use]
    pub fn with_moderation_publication_handoff(
        mut self,
        boundary: Arc<
            dyn iroha_torii::sorafs::moderation_runtime::ModerationDurableHandoffBoundaryV1,
        >,
    ) -> Self {
        self.moderation_publication_handoff = Some(boundary);
        self
    }

    /// Attach the durable payload-free moderation panel notification boundary.
    #[must_use]
    pub fn with_moderation_panel_notification(
        mut self,
        boundary: Arc<
            dyn iroha_torii::sorafs::moderation_runtime::
                ModerationDurablePanelNotificationBoundaryV1,
        >,
    ) -> Self {
        self.moderation_panel_notification = Some(boundary);
        self
    }

    /// Attach the deployment-owned sealed monotonic moderation checkpoint store.
    #[must_use]
    pub fn with_moderation_checkpoint_store(
        mut self,
        store: Arc<dyn sorafs_node::moderation_orchestrator::ModerationCheckpointStoreV1>,
    ) -> Self {
        self.moderation_checkpoint_store = Some(store);
        self
    }

    /// Attach the authenticated governed provider-ingest source pool.
    #[must_use]
    pub fn with_provider_ingest_authenticated_source(
        mut self,
        source: Arc<
            dyn crate::sorafs_provider_ingest_runtime::ProviderIngestAuthenticatedSourceRuntimeV1,
        >,
    ) -> Self {
        self.provider_ingest_authenticated_source = Some(source);
        self
    }

    /// Attach the governed provider-ingest completion-signer resolver.
    #[must_use]
    pub fn with_provider_ingest_signer_resolver(
        mut self,
        resolver: Arc<
            dyn crate::sorafs_provider_ingest_runtime::
                ProviderIngestGovernedSignerResolverRuntimeV1,
        >,
    ) -> Self {
        self.provider_ingest_signer_resolver = Some(resolver);
        self
    }

    /// Attach the provider-ingest sealed monotonic checkpoint store.
    #[must_use]
    pub fn with_provider_ingest_checkpoint_store(
        mut self,
        store: Arc<dyn sorafs_node::ProviderIngestCheckpointRuntimeV1>,
    ) -> Self {
        self.provider_ingest_checkpoint_store = Some(store);
        self
    }

    /// Attach the provider-ingest finalized-archive retention authority.
    #[must_use]
    pub fn with_provider_ingest_retention_authority(
        mut self,
        authority: Arc<
            dyn iroha_core::query::provider_ingest_finalized::
                ProviderIngestFinalizedArchiveRetentionAuthorityV1,
        >,
    ) -> Self {
        self.provider_ingest_retention_authority = Some(authority);
        self
    }

    /// Attach the reputation finalized-archive sealed retention authority.
    #[must_use]
    pub fn with_reputation_finalized_archive_retention_authority(
        mut self,
        authority: Arc<
            dyn iroha_core::query::reputation_finalized::
                ReputationFinalizedArchiveRetentionAuthorityV1,
        >,
    ) -> Self {
        self.reputation_finalized_archive_retention_authority = Some(authority);
        self
    }

    /// Attach the runtime-only native reputation-journal transaction submitter.
    #[must_use]
    pub fn with_reputation_journal_transaction_submitter(
        mut self,
        submitter: Arc<
            dyn sorafs_node::reputation::runtime::ReputationJournalTransactionSubmitterV1,
        >,
    ) -> Self {
        self.reputation_journal_transaction_submitter = Some(submitter);
        self
    }

    /// Attach the externally sealed monotonic reputation-journal checkpoint provider.
    #[must_use]
    pub fn with_reputation_journal_checkpoint(
        mut self,
        checkpoint: Arc<dyn sorafs_node::reputation::runtime::ReputationJournalCheckpointRuntimeV1>,
    ) -> Self {
        self.reputation_journal_checkpoint = Some(checkpoint);
        self
    }

    /// Attach the independently administered reputation threshold signer.
    #[must_use]
    pub fn with_reputation_threshold_signer(
        mut self,
        signer: Arc<dyn sorafs_node::reputation::runtime::ReputationThresholdSignerClientV1>,
    ) -> Self {
        self.reputation_threshold_signer = Some(signer);
        self
    }

    /// Attach the authenticated reputation Governance DAG publication/readback provider.
    #[must_use]
    pub fn with_reputation_governance_dag(
        mut self,
        governance_dag: Arc<dyn sorafs_node::reputation::runtime::ReputationGovernanceDagClientV1>,
    ) -> Self {
        self.reputation_governance_dag = Some(governance_dag);
        self
    }

    /// Attach the immutable finalized-ledger billing query.
    #[must_use]
    pub fn with_billing_finalized_query(
        mut self,
        query: Arc<dyn sorafs_node::hedging_billing_service::HedgingBillingFinalizedQuery>,
    ) -> Self {
        self.billing_finalized_query = Some(query);
        self
    }

    /// Attach the consensus billing-journal proof verifier.
    #[must_use]
    pub fn with_billing_journal_verifier(
        mut self,
        verifier: Arc<dyn sorafs_node::hedging_billing_service::HedgingBillingJournalVerifier>,
    ) -> Self {
        self.billing_journal_verifier = Some(verifier);
        self
    }

    /// Attach the independently administered billing statement HSM/KMS signer.
    #[must_use]
    pub fn with_billing_statement_signer(
        mut self,
        signer: Arc<dyn sorafs_node::hedging_billing_service::BillingStatementRuntimeSigner>,
    ) -> Self {
        self.billing_statement_signer = Some(signer);
        self
    }

    /// Attach the immutable billing statement publication/readback provider.
    #[must_use]
    pub fn with_billing_statement_publisher(
        mut self,
        publisher: Arc<dyn sorafs_node::hedging_billing_service::BillingStatementPublisher>,
    ) -> Self {
        self.billing_statement_publisher = Some(publisher);
        self
    }

    /// Attach the authenticated acknowledgement/reconciliation authority.
    #[must_use]
    pub fn with_billing_acknowledgement_authority(
        mut self,
        authority: Arc<
            dyn sorafs_node::hedging_billing_service::BillingStatementAcknowledgementAuthority,
        >,
    ) -> Self {
        self.billing_acknowledgement_authority = Some(authority);
        self
    }

    /// Attach the sealed monotonic billing epoch-witness store.
    #[must_use]
    pub fn with_billing_epoch_witness_store(
        mut self,
        store: Arc<dyn sorafs_node::hedging_billing_service::HedgingBillingEpochWitnessStore>,
    ) -> Self {
        self.billing_epoch_witness_store = Some(store);
        self
    }

    /// Attach the deployment-owned PoP private-runtime provider registry.
    #[must_use]
    pub fn with_pop_credential_provider_registry(
        mut self,
        registry: Arc<dyn iroha_torii::sorafs::pop_api::PopCredentialRuntimeProviderRegistryV1>,
    ) -> Self {
        self.pop_credential_provider_registry = Some(registry);
        self
    }

    /// Attach the independently administered PoTR gateway Ed25519 signer.
    #[must_use]
    pub fn with_potr_gateway_signer(
        mut self,
        signer: Arc<dyn iroha_torii::sorafs::PotrGatewaySignerV1>,
    ) -> Self {
        self.potr_gateway_signer = Some(signer);
        self
    }

    /// Attach the independently administered PoTR provider ML-DSA-65 signer.
    #[must_use]
    pub fn with_potr_provider_signer(
        mut self,
        signer: Arc<dyn iroha_torii::sorafs::PotrProviderSignerV1>,
    ) -> Self {
        self.potr_provider_signer = Some(signer);
        self
    }

    /// Attach the deployment-owned authenticated ACME client.
    #[must_use]
    pub fn with_gateway_acme_client(
        mut self,
        client: Arc<dyn iroha_torii::sorafs::gateway::AcmeClient>,
    ) -> Self {
        self.gateway_acme_client = Some(client);
        self
    }

    /// Attach the deployment-owned pinned DNS/HTTPS compliance-feed transport.
    #[must_use]
    pub fn with_gateway_compliance_feed_transport(
        mut self,
        transport: Arc<dyn iroha_torii::sorafs::gateway::GatewayComplianceFeedTransport>,
    ) -> Self {
        self.gateway_compliance_feed_transport = Some(transport);
        self
    }

    /// Attach the deployment-owned authenticated finalized-PoR replay archive.
    #[must_use]
    pub fn with_por_finalized_replay_archive(
        mut self,
        archive: Arc<dyn sorafs_node::PorFinalizedReplayArchiveV1>,
    ) -> Self {
        self.por_finalized_replay_archive = Some(archive);
        self
    }

    /// Attach the deployment-owned evidence-viewer WebAuthn boundary.
    #[must_use]
    pub fn with_evidence_viewer_webauthn(
        mut self,
        boundary: Arc<dyn sorafs_node::evidence_viewer::EvidenceViewerWebAuthnBoundaryV1>,
    ) -> Self {
        self.evidence_viewer_webauthn = Some(boundary);
        self
    }

    /// Attach the deployment-owned evidence-viewer rotating-grant authority.
    #[must_use]
    pub fn with_evidence_viewer_grants(
        mut self,
        boundary: Arc<dyn sorafs_node::evidence_viewer::EvidenceViewerGrantBoundaryV1>,
    ) -> Self {
        self.evidence_viewer_grants = Some(boundary);
        self
    }

    /// Attach the deployment-owned evidence-viewer receipt signer.
    #[must_use]
    pub fn with_evidence_viewer_receipt_signer(
        mut self,
        signer: Arc<dyn sorafs_node::evidence_viewer::EvidenceViewerReceiptSignerV1>,
    ) -> Self {
        self.evidence_viewer_receipt_signer = Some(signer);
        self
    }

    /// Attach the deployment-owned evidence-viewer erasure boundary.
    #[must_use]
    pub fn with_evidence_viewer_erasure(
        mut self,
        boundary: Arc<dyn sorafs_node::evidence_viewer::EvidenceViewerErasureBoundaryV1>,
    ) -> Self {
        self.evidence_viewer_erasure = Some(boundary);
        self
    }

    /// Attach the deployment-owned evidence-viewer authoritative checkpoint store.
    #[must_use]
    pub fn with_evidence_viewer_checkpoint_store(
        mut self,
        store: Arc<dyn sorafs_node::evidence_viewer::EvidenceViewerCheckpointStoreV1>,
    ) -> Self {
        self.evidence_viewer_checkpoint_store = Some(store);
        self
    }

    /// Attach the deployment-owned evidence-viewer immutable compaction archive.
    #[must_use]
    pub fn with_evidence_viewer_compaction_archive(
        mut self,
        archive: Arc<dyn sorafs_node::evidence_viewer::EvidenceViewerCompactionArchiveV1>,
    ) -> Self {
        self.evidence_viewer_compaction_archive = Some(archive);
        self
    }

    /// Attach the deployment-owned immutable moderation notification archive.
    #[must_use]
    pub fn with_moderation_panel_notification_archive(
        mut self,
        archive: Arc<
            dyn sorafs_node::moderation_orchestrator::ModerationPanelNotificationArchiveV1,
        >,
    ) -> Self {
        self.moderation_panel_notification_archive = Some(archive);
        self
    }

    /// Attach the deployment-owned signed monotonic evidence transparency publisher.
    #[must_use]
    pub fn with_evidence_viewer_transparency_publisher(
        mut self,
        publisher: Arc<
            dyn sorafs_node::evidence_viewer::transparency_producer::
                EvidenceViewerTransparencyPublisherV1,
        >,
    ) -> Self {
        self.evidence_viewer_transparency_publisher = Some(publisher);
        self
    }
}

/// Payload-free stock broker-server startup or transport failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RuntimeProviderBrokerServerErrorV1 {
    /// The catalog requests a role the bounded V1 server does not implement.
    UnsupportedRole,
    /// A requested backend is absent or an unrequested backend was injected.
    BackendSetMismatch,
    /// A backend's live public identity or qualification is not exact.
    BindingMismatch,
    /// The fixed service-UID-owned local endpoint could not be secured.
    EndpointUnavailable,
    /// The broker could not prove and remove its bound endpoint entry without
    /// risking a path-substitution unlink.
    EndpointCleanupFailed,
    /// A canonical protocol or authenticated peer invariant failed.
    Protocol,
    /// This platform lacks the authenticated V1 local transport.
    UnsupportedPlatform,
}

impl fmt::Display for RuntimeProviderBrokerServerErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedRole => "runtime-provider broker role is unsupported",
            Self::BackendSetMismatch => "runtime-provider broker backend set is incomplete",
            Self::BindingMismatch => "runtime-provider broker binding is not qualified",
            Self::EndpointUnavailable => "runtime-provider broker endpoint is unavailable",
            Self::EndpointCleanupFailed => {
                "runtime-provider broker endpoint cleanup could not be completed safely"
            }
            Self::Protocol => "runtime-provider broker protocol failed",
            Self::UnsupportedPlatform => {
                "runtime-provider broker transport is unsupported on this platform"
            }
        })
    }
}

impl std::error::Error for RuntimeProviderBrokerServerErrorV1 {}

/// Serve the exact catalog on the platform-fixed service-UID-owned endpoint.
///
/// This is the packaged launcher boundary for deployment-owned broker
/// executables. It blocks in the authenticated accept loop and never loads
/// credentials, private keys, environment overrides, or test backends.
///
/// # Errors
///
/// Fails before accepting clients if the catalog/backend set is incomplete or
/// any live public binding is missing, substituted, stale, revoked, or
/// test-marked. It also fails when the fixed endpoint cannot be created with
/// the required ownership and mode.
pub fn serve_runtime_provider_broker_v1(
    bindings: &IrohaRuntimeProviderBindingsV1,
    backends: RuntimeProviderBrokerBackendsV1,
) -> Result<(), RuntimeProviderBrokerServerErrorV1> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        protocol::serve(bindings, backends)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (bindings, backends);
        Err(RuntimeProviderBrokerServerErrorV1::UnsupportedPlatform)
    }
}

/// Serve the exact catalog until the caller requests an orderly shutdown.
///
/// The caller retains a clone of `lifecycle` and requests shutdown through
/// [`RuntimeProviderBrokerLifecycleV1::request_shutdown`]. `on_ready` runs
/// exactly once, on the serving thread, after all requested backends have
/// passed live qualification, the fixed endpoint has been securely bound, and
/// the complete backend catalog has passed an immediate second qualification.
/// A bounded gate linearizes the complete callback against shutdown: a
/// shutdown that wins suppresses the callback, while a shutdown that loses
/// waits for the callback to finish before returning.
///
/// The callback must be bounded and infallible, and it must not call
/// [`RuntimeProviderBrokerLifecycleV1::request_shutdown`] reentrantly. If it
/// panics, endpoint cleanup is attempted while the panic unwinds.
///
/// After shutdown, the server closes every accepted transport and joins every
/// session before returning. Synchronous deployment-owned provider methods do
/// not expose cancellation or a uniform deadline, so a qualification call
/// already in progress or an operation already admitted can delay this return;
/// deployments must enforce their advertised bounds inside each provider
/// adapter. Admission is the final atomic check immediately before dispatch
/// and can precede entry into the trait method by a small in-process interval.
/// No operation is admitted after the shutdown transition.
///
/// Startup binds an unpredictable staging name in a pinned parent directory,
/// establishes the socket identity guard before permission changes, then
/// atomically promotes that entry to the canonical name without replacement.
/// Portable Linux/macOS pathname APIs do not provide an atomic
/// “unlink-if-device-and-inode-match” operation. Cleanup resolves and unlinks
/// relative to the pinned directory descriptor, checks the socket identity
/// immediately before that unlink, and reports substitution instead of
/// knowingly removing a different entry. These pathname APIs still leave
/// check/use intervals around mode changes and cleanup, so the service-owned
/// runtime directory must exclude untrusted same-UID pathname mutators. If the
/// broker cannot establish the staging entry's identity immediately after a
/// successful bind, it closes the listener, reports
/// [`RuntimeProviderBrokerServerErrorV1::EndpointCleanupFailed`], and leaves
/// that unpredictable staging entry for operator inspection rather than
/// unlinking an unproven replacement.
///
/// # Errors
///
/// Fails before readiness if the catalog/backend set is incomplete, any live
/// public binding is missing, substituted, stale, revoked, or test-marked, or
/// the fixed endpoint cannot be created with the required ownership and mode.
pub fn serve_runtime_provider_broker_with_lifecycle_v1<R>(
    bindings: &IrohaRuntimeProviderBindingsV1,
    backends: RuntimeProviderBrokerBackendsV1,
    lifecycle: Arc<RuntimeProviderBrokerLifecycleV1>,
    on_ready: R,
) -> Result<(), RuntimeProviderBrokerServerErrorV1>
where
    R: FnOnce(),
{
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        protocol::serve_with_lifecycle(bindings, backends, lifecycle, on_ready)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        lifecycle.request_shutdown();
        let _ = (bindings, backends, on_ready);
        Err(RuntimeProviderBrokerServerErrorV1::UnsupportedPlatform)
    }
}
