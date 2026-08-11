/// Runtime-only daemon dependencies supplied by the deployment launcher.
///
/// Implementations of the moderation wrapper, privacy-cycle PRF provider,
/// stream-token and native proof/repair/reserve/orderbook/moderation signers,
/// moderation durable handoffs, evidence-viewer checkpoint authority,
/// appeal-finance transaction signers,
/// role-separated `PoTR` signers, exact-view billing queries, threshold/HSM
/// signers, immutable publication, acknowledgement, sealed witness storage,
/// authenticated Governance DAG publication/readback/head updates, sealed
/// monotonic Governance DAG checkpoints, externally sealed reputation journal
/// checkpoints, the Soracloud mutation/provenance signer, and the authenticated
/// Hugging Face credential provider, plus the reserved Musubi provider-
/// attestation clock, approval signer, and authenticated inventory, are the
/// reference-node boundaries for
/// ledger access, PKCS#11, managed-KMS, and threshold services. Provider
/// credentials, unwrapped keys, PRF shares, seeds, and outputs must stay inside
/// those implementations and must never be sourced from `iroha_config`.
#[derive(Clone, Default)]
pub struct IrohaRuntimeDeps {
    bootle_lantern_issuance_provider_registry: Option<
        Arc<
            dyn iroha_torii::privacy_issuance_api::BootleLanternIssuanceRuntimeProviderRegistryV1,
        >,
    >,
    moderation_quarantine_key_wrapper: Option<Arc<dyn sorafs_node::ModerationQuarantineKeyWrapper>>,
    privacy_cycle_prf_provider:
        Option<Arc<dyn sorafs_node::ProductionPrivacyCyclePrfProviderV1>>,
    privacy_release_anchor: Option<Arc<dyn sorafs_node::ProductionPrivacyReleaseAnchorV1>>,
    transparency_leader_lease_provider:
        Option<Arc<dyn sorafs_node::ProductionTransparencyLeaderLeaseProviderV1>>,
    sorafs_fenced_transparency_publisher:
        Option<Arc<dyn sorafs_node::FencedTransparencyPublisherV1>>,
    sorafs_fenced_transparency_head_reader:
        Option<Arc<dyn sorafs_node::FencedTransparencyAuthoritativeHeadReaderV1>>,
    sorafs_governance_dag_signer: Option<Arc<dyn sorafs_node::GovernanceDagRuntimeSigner>>,
    sorafs_governance_dag_ipfs_authenticator:
        Option<Arc<dyn sorafs_node::GovernanceDagRequestAuthenticator>>,
    sorafs_governance_dag_head_authenticator:
        Option<Arc<dyn sorafs_node::GovernanceDagRequestAuthenticator>>,
    sorafs_governance_dag_checkpoint_store:
        Option<Arc<dyn sorafs_node::GovernanceDagSealedCheckpointStore>>,
    sorafs_stream_token_signer: Option<Arc<dyn iroha_torii::sorafs::StreamTokenRuntimeSigner>>,
    sorafs_stream_token_gateway_admission:
        Option<Arc<dyn iroha_torii::sorafs::StreamTokenGatewayAdmissionProviderV1>>,
    sorafs_appeal_finance_runtime_signers:
        Option<Arc<iroha_torii::SoraFsAppealFinanceRuntimeSignersV1>>,
    sorafs_appeal_finance_checkpoint_runtime: Option<
        Arc<dyn sorafs_node::appeal_finance_transaction_forwarder::AppealFinanceCheckpointRuntime>,
    >,
    sorafs_proof_outcome_signer: Option<Arc<dyn iroha_torii::SoraFsProofOutcomeTransactionSigner>>,
    sorafs_repair_transaction_signer: Option<Arc<dyn iroha_torii::SoraFsRepairTransactionSigner>>,
    sorafs_reserve_transaction_signer: Option<Arc<dyn iroha_torii::SoraFsReserveTransactionSigner>>,
    sorafs_orderbook_transaction_signer:
        Option<Arc<dyn iroha_torii::SoraFsOrderbookTransactionSigner>>,
    sorafs_moderation_transaction_signer: Option<
        Arc<dyn iroha_torii::sorafs::moderation_runtime::ModerationSignedTransactionSignerV1>,
    >,
    sorafs_moderation_settlement_handoff: Option<
        Arc<dyn iroha_torii::sorafs::moderation_runtime::ModerationDurableHandoffBoundaryV1>,
    >,
    sorafs_moderation_publication_handoff: Option<
        Arc<dyn iroha_torii::sorafs::moderation_runtime::ModerationDurableHandoffBoundaryV1>,
    >,
    sorafs_moderation_panel_notification: Option<
        Arc<
            dyn iroha_torii::sorafs::moderation_runtime::ModerationDurablePanelNotificationBoundaryV1,
        >,
    >,
    sorafs_moderation_panel_notification_archive: Option<
        Arc<
            dyn sorafs_node::moderation_orchestrator::ModerationPanelNotificationArchiveV1,
        >,
    >,
    sorafs_moderation_checkpoint_store:
        Option<Arc<dyn sorafs_node::moderation_orchestrator::ModerationCheckpointStoreV1>>,
    sorafs_evidence_viewer_webauthn:
        Option<Arc<dyn sorafs_node::evidence_viewer::EvidenceViewerWebAuthnBoundaryV1>>,
    sorafs_evidence_viewer_grants:
        Option<Arc<dyn sorafs_node::evidence_viewer::EvidenceViewerGrantBoundaryV1>>,
    sorafs_evidence_viewer_receipt_signer:
        Option<Arc<dyn sorafs_node::evidence_viewer::EvidenceViewerReceiptSignerV1>>,
    sorafs_evidence_viewer_erasure:
        Option<Arc<dyn sorafs_node::evidence_viewer::EvidenceViewerErasureBoundaryV1>>,
    sorafs_evidence_viewer_checkpoint_store:
        Option<Arc<dyn sorafs_node::evidence_viewer::EvidenceViewerCheckpointStoreV1>>,
    sorafs_evidence_viewer_compaction_archive:
        Option<Arc<dyn sorafs_node::evidence_viewer::EvidenceViewerCompactionArchiveV1>>,
    sorafs_evidence_viewer_transparency_publisher: Option<
        Arc<
            dyn sorafs_node::evidence_viewer::transparency_producer::
                EvidenceViewerTransparencyPublisherV1,
        >,
    >,
    sorafs_pop_credential_provider_registry:
        Option<Arc<dyn iroha_torii::sorafs::pop_api::PopCredentialRuntimeProviderRegistryV1>>,
    sorafs_potr_runtime_signer_roles: Option<Arc<iroha_torii::sorafs::PotrRuntimeSignerRolesV1>>,
    sorafs_gateway_acme_client: Option<Arc<dyn iroha_torii::sorafs::gateway::AcmeClient>>,
    sorafs_gateway_compliance_feed_transport:
        Option<Arc<dyn iroha_torii::sorafs::gateway::GatewayComplianceFeedTransport>>,
    sorafs_reputation_journal_checkpoint_provider: Option<
        Arc<dyn sorafs_node::reputation::runtime::ReputationJournalCheckpointRuntimeV1>,
    >,
    sorafs_reputation_journal_transaction_submitter:
        Option<Arc<dyn sorafs_node::reputation::runtime::ReputationJournalTransactionSubmitterV1>>,
    sorafs_reputation_threshold_signer:
        Option<Arc<dyn sorafs_node::reputation::runtime::ReputationThresholdSignerClientV1>>,
    sorafs_reputation_governance_dag:
        Option<Arc<dyn sorafs_node::reputation::runtime::ReputationGovernanceDagClientV1>>,
    sorafs_reputation_retention_authority: Option<
        Arc<
            dyn iroha_core::query::reputation_finalized::ReputationFinalizedArchiveRetentionAuthorityV1,
        >,
    >,
    sorafs_hedging_billing_finalized_query:
        Option<Arc<dyn sorafs_node::hedging_billing_service::HedgingBillingFinalizedQuery>>,
    sorafs_hedging_billing_journal_verifier:
        Option<Arc<dyn sorafs_node::hedging_billing_service::HedgingBillingJournalVerifier>>,
    sorafs_billing_statement_signer:
        Option<Arc<dyn sorafs_node::hedging_billing_service::BillingStatementRuntimeSigner>>,
    sorafs_billing_statement_publisher:
        Option<Arc<dyn sorafs_node::hedging_billing_service::BillingStatementPublisher>>,
    sorafs_billing_acknowledgement_authority: Option<
        Arc<dyn sorafs_node::hedging_billing_service::BillingStatementAcknowledgementAuthority>,
    >,
    sorafs_hedging_billing_epoch_witness_store:
        Option<Arc<dyn sorafs_node::hedging_billing_service::HedgingBillingEpochWitnessStore>>,
    sorafs_provider_ingest_authenticated_source:
        Option<Arc<dyn sorafs_provider_ingest_runtime::ProviderIngestAuthenticatedSourceRuntimeV1>>,
    sorafs_provider_ingest_signer_resolver: Option<
        Arc<dyn sorafs_provider_ingest_runtime::ProviderIngestGovernedSignerResolverRuntimeV1>,
    >,
    sorafs_provider_ingest_checkpoint_runtime:
        Option<Arc<dyn sorafs_node::ProviderIngestCheckpointRuntimeV1>>,
    sorafs_provider_ingest_retention_authority: Option<
        Arc<
            dyn iroha_core::query::provider_ingest_finalized::ProviderIngestFinalizedArchiveRetentionAuthorityV1,
        >,
    >,
    sorafs_por_finalized_replay_archive:
        Option<Arc<dyn sorafs_node::PorFinalizedReplayArchiveV1>>,
    soracloud_runtime_mutation_signer:
        Option<Arc<dyn soracloud_runtime_signer::SoracloudRuntimeMutationSignerV1>>,
    soracloud_hf_inference_credential_provider:
        Option<Arc<dyn soracloud_hf_credential::SoracloudHfInferenceCredentialProviderV1>>,
    sorafs_musubi_provider_attestation_clock_seal:
        Option<Arc<dyn sorafs_node::MusubiProviderAttestationClockSealV1>>,
    sorafs_musubi_provider_attestation_approval_signer:
        Option<Arc<dyn sorafs_node::MusubiProviderAttestationSignerV1>>,
    sorafs_musubi_provider_attestation_inventory:
        Option<Arc<dyn sorafs_node::MusubiProviderAttestationInventoryRuntimeV1>>,
}
impl IrohaRuntimeDeps {
    /// Return whether no deployment-owned runtime dependency is attached.
    ///
    /// The standard launcher uses this to reject a registry that returns
    /// process-local authority when configuration requested no provider.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bootle_lantern_issuance_provider_registry.is_none()
            && self.moderation_quarantine_key_wrapper.is_none()
            && self.privacy_cycle_prf_provider.is_none()
            && self.privacy_release_anchor.is_none()
            && self.transparency_leader_lease_provider.is_none()
            && self.sorafs_fenced_transparency_publisher.is_none()
            && self.sorafs_fenced_transparency_head_reader.is_none()
            && self.sorafs_governance_dag_signer.is_none()
            && self.sorafs_governance_dag_ipfs_authenticator.is_none()
            && self.sorafs_governance_dag_head_authenticator.is_none()
            && self.sorafs_governance_dag_checkpoint_store.is_none()
            && self.sorafs_stream_token_signer.is_none()
            && self.sorafs_stream_token_gateway_admission.is_none()
            && self.sorafs_appeal_finance_runtime_signers.is_none()
            && self.sorafs_appeal_finance_checkpoint_runtime.is_none()
            && self.sorafs_proof_outcome_signer.is_none()
            && self.sorafs_repair_transaction_signer.is_none()
            && self.sorafs_reserve_transaction_signer.is_none()
            && self.sorafs_orderbook_transaction_signer.is_none()
            && self.sorafs_moderation_transaction_signer.is_none()
            && self.sorafs_moderation_settlement_handoff.is_none()
            && self.sorafs_moderation_publication_handoff.is_none()
            && self.sorafs_moderation_panel_notification.is_none()
            && self.sorafs_moderation_panel_notification_archive.is_none()
            && self.sorafs_moderation_checkpoint_store.is_none()
            && self.sorafs_evidence_viewer_webauthn.is_none()
            && self.sorafs_evidence_viewer_grants.is_none()
            && self.sorafs_evidence_viewer_receipt_signer.is_none()
            && self.sorafs_evidence_viewer_erasure.is_none()
            && self.sorafs_evidence_viewer_checkpoint_store.is_none()
            && self.sorafs_evidence_viewer_compaction_archive.is_none()
            && self.sorafs_evidence_viewer_transparency_publisher.is_none()
            && self.sorafs_pop_credential_provider_registry.is_none()
            && self.sorafs_potr_runtime_signer_roles.is_none()
            && self.sorafs_gateway_acme_client.is_none()
            && self.sorafs_gateway_compliance_feed_transport.is_none()
            && self.sorafs_reputation_journal_checkpoint_provider.is_none()
            && self
                .sorafs_reputation_journal_transaction_submitter
                .is_none()
            && self.sorafs_reputation_threshold_signer.is_none()
            && self.sorafs_reputation_governance_dag.is_none()
            && self.sorafs_reputation_retention_authority.is_none()
            && self.sorafs_hedging_billing_finalized_query.is_none()
            && self.sorafs_hedging_billing_journal_verifier.is_none()
            && self.sorafs_billing_statement_signer.is_none()
            && self.sorafs_billing_statement_publisher.is_none()
            && self.sorafs_billing_acknowledgement_authority.is_none()
            && self.sorafs_hedging_billing_epoch_witness_store.is_none()
            && self.sorafs_provider_ingest_authenticated_source.is_none()
            && self.sorafs_provider_ingest_signer_resolver.is_none()
            && self.sorafs_provider_ingest_checkpoint_runtime.is_none()
            && self.sorafs_provider_ingest_retention_authority.is_none()
            && self.sorafs_por_finalized_replay_archive.is_none()
            && self.soracloud_runtime_mutation_signer.is_none()
            && self.soracloud_hf_inference_credential_provider.is_none()
            && self.sorafs_musubi_provider_attestation_clock_seal.is_none()
            && self
                .sorafs_musubi_provider_attestation_approval_signer
                .is_none()
            && self.sorafs_musubi_provider_attestation_inventory.is_none()
    }

    /// Attach the deployment-owned Bootle/Lantern issuer and authentication registry.
    #[must_use]
    pub fn with_bootle_lantern_issuance_provider_registry(
        mut self,
        registry: Arc<
            dyn iroha_torii::privacy_issuance_api::BootleLanternIssuanceRuntimeProviderRegistryV1,
        >,
    ) -> Self {
        self.bootle_lantern_issuance_provider_registry = Some(registry);
        self
    }

    /// Attach the production PKCS#11/KMS wrapper for moderation quarantine
    /// object data keys.
    #[must_use]
    pub fn with_moderation_quarantine_key_wrapper(
        mut self,
        key_wrapper: Arc<dyn sorafs_node::ModerationQuarantineKeyWrapper>,
    ) -> Self {
        self.moderation_quarantine_key_wrapper = Some(key_wrapper);
        self
    }

    /// Attach the production threshold-PRF provider for differential-privacy
    /// publication cycles.
    #[must_use]
    pub fn with_privacy_cycle_prf_provider(
        mut self,
        provider: Arc<dyn sorafs_node::ProductionPrivacyCyclePrfProviderV1>,
    ) -> Self {
        self.privacy_cycle_prf_provider = Some(provider);
        self
    }

    /// Attach the independently administered finalized privacy-release head.
    #[must_use]
    pub fn with_privacy_release_anchor(
        mut self,
        anchor: Arc<dyn sorafs_node::ProductionPrivacyReleaseAnchorV1>,
    ) -> Self {
        self.privacy_release_anchor = Some(anchor);
        self
    }

    /// Attach the production external sealed-CAS transparency leader lease.
    #[must_use]
    pub fn with_transparency_leader_lease_provider(
        mut self,
        provider: Arc<dyn sorafs_node::ProductionTransparencyLeaderLeaseProviderV1>,
    ) -> Self {
        self.transparency_leader_lease_provider = Some(provider);
        self
    }

    /// Attach the deployment-owned fused privacy Governance target writer.
    ///
    /// Enabled privacy publication requires this writer and an authenticated
    /// head reader. Both roles must expose the exact configured handle,
    /// revision, and policy digest; partial or mismatched pairs fail startup.
    #[must_use]
    pub fn with_sorafs_fenced_transparency_publisher(
        mut self,
        publisher: Arc<dyn sorafs_node::FencedTransparencyPublisherV1>,
    ) -> Self {
        self.sorafs_fenced_transparency_publisher = Some(publisher);
        self
    }

    /// Attach the authenticated authoritative-head reader paired with the
    /// fused privacy target writer.
    ///
    /// Enabled privacy publication requires both roles to expose the exact
    /// configured handle, revision, and policy digest; partial or mismatched
    /// pairs fail startup.
    #[must_use]
    pub fn with_sorafs_fenced_transparency_head_reader(
        mut self,
        reader: Arc<dyn sorafs_node::FencedTransparencyAuthoritativeHeadReaderV1>,
    ) -> Self {
        self.sorafs_fenced_transparency_head_reader = Some(reader);
        self
    }

    /// Attach the production HSM/KMS signer for the embedded `SoraFS`
    /// Governance DAG publisher.
    #[must_use]
    pub fn with_sorafs_governance_dag_signer(
        mut self,
        signer: Arc<dyn sorafs_node::GovernanceDagRuntimeSigner>,
    ) -> Self {
        self.sorafs_governance_dag_signer = Some(signer);
        self
    }

    /// Attach the production Kubo/IPFS/IPNS request authenticator for the
    /// supervised Governance DAG service.
    #[must_use]
    pub fn with_sorafs_governance_dag_ipfs_authenticator(
        mut self,
        authenticator: Arc<dyn sorafs_node::GovernanceDagRequestAuthenticator>,
    ) -> Self {
        self.sorafs_governance_dag_ipfs_authenticator = Some(authenticator);
        self
    }

    /// Attach the production signed-head compare-and-swap authenticator for
    /// the supervised Governance DAG service.
    #[must_use]
    pub fn with_sorafs_governance_dag_head_authenticator(
        mut self,
        authenticator: Arc<dyn sorafs_node::GovernanceDagRequestAuthenticator>,
    ) -> Self {
        self.sorafs_governance_dag_head_authenticator = Some(authenticator);
        self
    }

    /// Attach the sealed monotonic checkpoint and publish-intent store for the
    /// supervised Governance DAG service.
    #[must_use]
    pub fn with_sorafs_governance_dag_checkpoint_store(
        mut self,
        checkpoint_store: Arc<dyn sorafs_node::GovernanceDagSealedCheckpointStore>,
    ) -> Self {
        self.sorafs_governance_dag_checkpoint_store = Some(checkpoint_store);
        self
    }

    /// Attach the production HSM/KMS signer for `SoraFS` stream-token issuance.
    #[must_use]
    pub fn with_sorafs_stream_token_signer(
        mut self,
        signer: Arc<dyn iroha_torii::sorafs::StreamTokenRuntimeSigner>,
    ) -> Self {
        self.sorafs_stream_token_signer = Some(signer);
        self
    }

    /// Attach the deployment-owned atomic stream-token quota, sealed sequence,
    /// and ordered callback-outbox provider.
    #[must_use]
    pub fn with_sorafs_stream_token_gateway_admission(
        mut self,
        provider: Arc<dyn iroha_torii::sorafs::StreamTokenGatewayAdmissionProviderV1>,
    ) -> Self {
        self.sorafs_stream_token_gateway_admission = Some(provider);
        self
    }

    /// Attach runtime-only HSM/KMS providers for appeal-finance lock,
    /// disbursement, and refund transactions.
    #[must_use]
    pub fn with_sorafs_appeal_finance_runtime_signers(
        mut self,
        signers: Arc<iroha_torii::SoraFsAppealFinanceRuntimeSignersV1>,
    ) -> Self {
        self.sorafs_appeal_finance_runtime_signers = Some(signers);
        self
    }

    /// Attach the HSM/KMS-authenticated monotonic checkpoint boundary for the
    /// appeal-finance transaction forwarder.
    #[must_use]
    pub fn with_sorafs_appeal_finance_checkpoint_runtime(
        mut self,
        runtime: Arc<
            dyn sorafs_node::appeal_finance_transaction_forwarder::AppealFinanceCheckpointRuntime,
        >,
    ) -> Self {
        self.sorafs_appeal_finance_checkpoint_runtime = Some(runtime);
        self
    }

    /// Attach a raw runtime-only signer for authoritative proof-outcome
    /// transactions.
    ///
    /// The deployment registry resolver replaces this provider with an
    /// immutable facade qualified against the exact configured role, authority,
    /// algorithm, key, revision, and policy digest.
    #[must_use]
    pub fn with_sorafs_proof_outcome_signer(
        mut self,
        signer: Arc<dyn iroha_torii::SoraFsProofOutcomeTransactionSigner>,
    ) -> Self {
        self.sorafs_proof_outcome_signer = Some(signer);
        self
    }

    /// Attach a raw runtime-only signer for native repair transactions.
    ///
    /// The deployment registry resolver replaces this provider with an
    /// immutable facade qualified against the exact configured role, authority,
    /// algorithm, key, revision, and policy digest.
    #[must_use]
    pub fn with_sorafs_repair_transaction_signer(
        mut self,
        signer: Arc<dyn iroha_torii::SoraFsRepairTransactionSigner>,
    ) -> Self {
        self.sorafs_repair_transaction_signer = Some(signer);
        self
    }

    /// Attach a raw runtime-only signer for native reserve/rent transactions.
    ///
    /// The deployment registry resolver replaces this provider with an
    /// immutable facade qualified against the exact configured role, authority,
    /// algorithm, key, revision, and policy digest.
    #[must_use]
    pub fn with_sorafs_reserve_transaction_signer(
        mut self,
        signer: Arc<dyn iroha_torii::SoraFsReserveTransactionSigner>,
    ) -> Self {
        self.sorafs_reserve_transaction_signer = Some(signer);
        self
    }

    /// Attach a raw runtime-only signer for native orderbook transactions.
    ///
    /// The deployment registry resolver replaces this provider with an
    /// immutable facade qualified against the exact configured role, authority,
    /// algorithm, key, revision, and policy digest.
    #[must_use]
    pub fn with_sorafs_orderbook_transaction_signer(
        mut self,
        signer: Arc<dyn iroha_torii::SoraFsOrderbookTransactionSigner>,
    ) -> Self {
        self.sorafs_orderbook_transaction_signer = Some(signer);
        self
    }

    /// Attach the raw deployment-owned Soracloud transaction and provenance signer.
    ///
    /// The runtime-provider registry replaces this provider with an immutable
    /// facade qualified against the exact configured handle, authority, key,
    /// revision, policy digest, active posture, and non-test posture.
    #[must_use]
    pub fn with_soracloud_runtime_mutation_signer(
        mut self,
        signer: Arc<dyn soracloud_runtime_signer::SoracloudRuntimeMutationSignerV1>,
    ) -> Self {
        self.soracloud_runtime_mutation_signer = Some(signer);
        self
    }

    /// Attach the raw deployment-owned authenticated HF credential provider.
    ///
    /// The registry resolver replaces this provider with an immutable facade
    /// qualified against the exact configured handle, revision, policy digest,
    /// active posture, and non-test posture. Bearer credentials remain inside
    /// the provider.
    #[must_use]
    pub fn with_soracloud_hf_inference_credential_provider(
        mut self,
        provider: Arc<dyn soracloud_hf_credential::SoracloudHfInferenceCredentialProviderV1>,
    ) -> Self {
        self.soracloud_hf_inference_credential_provider = Some(provider);
        self
    }

    /// Attach the runtime-only HSM/KMS signer for exact moderation native
    /// transaction envelopes.
    #[must_use]
    pub fn with_sorafs_moderation_transaction_signer(
        mut self,
        signer: Arc<
            dyn iroha_torii::sorafs::moderation_runtime::ModerationSignedTransactionSignerV1,
        >,
    ) -> Self {
        self.sorafs_moderation_transaction_signer = Some(signer);
        self
    }

    /// Attach the durable appeal-finance boundary for finalized moderation
    /// settlement handoffs.
    #[must_use]
    pub fn with_sorafs_moderation_settlement_handoff(
        mut self,
        boundary: Arc<
            dyn iroha_torii::sorafs::moderation_runtime::ModerationDurableHandoffBoundaryV1,
        >,
    ) -> Self {
        self.sorafs_moderation_settlement_handoff = Some(boundary);
        self
    }

    /// Attach the durable governance/transparency boundary for finalized
    /// moderation publication handoffs.
    #[must_use]
    pub fn with_sorafs_moderation_publication_handoff(
        mut self,
        boundary: Arc<
            dyn iroha_torii::sorafs::moderation_runtime::ModerationDurableHandoffBoundaryV1,
        >,
    ) -> Self {
        self.sorafs_moderation_publication_handoff = Some(boundary);
        self
    }

    /// Attach the durable payload-free juror-notification boundary.
    #[must_use]
    pub fn with_sorafs_moderation_panel_notification(
        mut self,
        boundary: Arc<
            dyn iroha_torii::sorafs::moderation_runtime::ModerationDurablePanelNotificationBoundaryV1,
        >,
    ) -> Self {
        self.sorafs_moderation_panel_notification = Some(boundary);
        self
    }

    /// Attach the immutable authenticated moderation notification-receipt archive.
    #[must_use]
    pub fn with_sorafs_moderation_panel_notification_archive(
        mut self,
        archive: Arc<
            dyn sorafs_node::moderation_orchestrator::ModerationPanelNotificationArchiveV1,
        >,
    ) -> Self {
        self.sorafs_moderation_panel_notification_archive = Some(archive);
        self
    }

    /// Attach the deployment-owned sealed monotonic moderation checkpoint authority.
    #[must_use]
    pub fn with_sorafs_moderation_checkpoint_store(
        mut self,
        checkpoint_store: Arc<
            dyn sorafs_node::moderation_orchestrator::ModerationCheckpointStoreV1,
        >,
    ) -> Self {
        self.sorafs_moderation_checkpoint_store = Some(checkpoint_store);
        self
    }

    /// Attach the production `WebAuthn` verifier for evidence-viewer sessions.
    #[must_use]
    pub fn with_sorafs_evidence_viewer_webauthn(
        mut self,
        boundary: Arc<dyn sorafs_node::evidence_viewer::EvidenceViewerWebAuthnBoundaryV1>,
    ) -> Self {
        self.sorafs_evidence_viewer_webauthn = Some(boundary);
        self
    }

    /// Attach the finalized assignment/role grant authority for evidence
    /// viewing.
    #[must_use]
    pub fn with_sorafs_evidence_viewer_grants(
        mut self,
        boundary: Arc<dyn sorafs_node::evidence_viewer::EvidenceViewerGrantBoundaryV1>,
    ) -> Self {
        self.sorafs_evidence_viewer_grants = Some(boundary);
        self
    }

    /// Attach the HSM-backed signer for hash-chained evidence access receipts.
    #[must_use]
    pub fn with_sorafs_evidence_viewer_receipt_signer(
        mut self,
        signer: Arc<dyn sorafs_node::evidence_viewer::EvidenceViewerReceiptSignerV1>,
    ) -> Self {
        self.sorafs_evidence_viewer_receipt_signer = Some(signer);
        self
    }

    /// Attach the authenticated evidence erasure boundary. Its implementation
    /// owns KMS/storage credentials and must honor stable operation IDs.
    #[must_use]
    pub fn with_sorafs_evidence_viewer_erasure(
        mut self,
        boundary: Arc<dyn sorafs_node::evidence_viewer::EvidenceViewerErasureBoundaryV1>,
    ) -> Self {
        self.sorafs_evidence_viewer_erasure = Some(boundary);
        self
    }

    /// Attach the deployment-owned linearizable evidence-viewer checkpoint
    /// authority. Its implementation owns all CAS credentials and sealed
    /// persistence state.
    #[must_use]
    pub fn with_sorafs_evidence_viewer_checkpoint_store(
        mut self,
        checkpoint_store: Arc<dyn sorafs_node::evidence_viewer::EvidenceViewerCheckpointStoreV1>,
    ) -> Self {
        self.sorafs_evidence_viewer_checkpoint_store = Some(checkpoint_store);
        self
    }

    /// Attach the authenticated immutable evidence-viewer compaction archive.
    ///
    /// Archive credentials and its Ed25519 private signing key remain inside
    /// the deployment-owned implementation.
    #[must_use]
    pub fn with_sorafs_evidence_viewer_compaction_archive(
        mut self,
        archive: Arc<dyn sorafs_node::evidence_viewer::EvidenceViewerCompactionArchiveV1>,
    ) -> Self {
        self.sorafs_evidence_viewer_compaction_archive = Some(archive);
        self
    }

    /// Attach the deployment-owned signed monotonic evidence transparency publisher.
    ///
    /// Publisher credentials and the Ed25519 private signing key remain inside
    /// the deployment-owned implementation.
    #[must_use]
    pub fn with_sorafs_evidence_viewer_transparency_publisher(
        mut self,
        publisher: Arc<
            dyn sorafs_node::evidence_viewer::transparency_producer::
                EvidenceViewerTransparencyPublisherV1,
        >,
    ) -> Self {
        self.sorafs_evidence_viewer_transparency_publisher = Some(publisher);
        self
    }

    /// Attach the deployment-owned registry for all runtime-only `PoP`
    /// enrollment, issuer, finalized-query, wallet, and authentication
    /// providers.
    #[must_use]
    pub fn with_sorafs_pop_credential_provider_registry(
        mut self,
        provider_registry: Arc<
            dyn iroha_torii::sorafs::pop_api::PopCredentialRuntimeProviderRegistryV1,
        >,
    ) -> Self {
        self.sorafs_pop_credential_provider_registry = Some(provider_registry);
        self
    }

    /// Attach independently administered runtime HSM services for the `SoraFS`
    /// `PoTR` gateway Ed25519 and provider ML-DSA-65 receipt roles.
    ///
    /// Torii binds these roles to its own authoritative finalized state after
    /// state and the council-verified admission registry are available.
    #[must_use]
    pub fn with_sorafs_potr_runtime_signer_roles(
        mut self,
        roles: Arc<iroha_torii::sorafs::PotrRuntimeSignerRolesV1>,
    ) -> Self {
        self.sorafs_potr_runtime_signer_roles = Some(roles);
        self
    }

    /// Attach the runtime-owned ACME client used by the `SoraFS` regional gateway.
    ///
    /// Account and DNS-provider credentials remain inside the implementation
    /// and never enter resolved configuration or Torii state.
    #[must_use]
    pub fn with_sorafs_gateway_acme_client(
        mut self,
        client: Arc<dyn iroha_torii::sorafs::gateway::AcmeClient>,
    ) -> Self {
        self.sorafs_gateway_acme_client = Some(client);
        self
    }

    /// Attach the authenticated, address-pinned `SoraFS` compliance feed transport.
    ///
    /// Bearer tokens, client identities, DNS credentials, and TLS key material
    /// remain owned by the deployment adapter.
    #[must_use]
    pub fn with_sorafs_gateway_compliance_feed_transport(
        mut self,
        transport: Arc<dyn iroha_torii::sorafs::gateway::GatewayComplianceFeedTransport>,
    ) -> Self {
        self.sorafs_gateway_compliance_feed_transport = Some(transport);
        self
    }

    /// Attach a runtime-only identity-matching signer and normal-queue
    /// submitter for native `PoR` and stream-token reputation journal entries.
    #[must_use]
    pub fn with_sorafs_reputation_journal_transaction_submitter(
        mut self,
        submitter: Arc<
            dyn sorafs_node::reputation::runtime::ReputationJournalTransactionSubmitterV1,
        >,
    ) -> Self {
        self.sorafs_reputation_journal_transaction_submitter = Some(submitter);
        self
    }

    /// Attach the externally sealed monotonic checkpoint provider for the
    /// native reputation journal outbox.
    #[must_use]
    pub fn with_sorafs_reputation_journal_checkpoint_provider(
        mut self,
        provider: Arc<dyn sorafs_node::reputation::runtime::ReputationJournalCheckpointRuntimeV1>,
    ) -> Self {
        self.sorafs_reputation_journal_checkpoint_provider = Some(provider);
        self
    }

    /// Attach the external threshold-signing service for exact committed
    /// reputation material.
    #[must_use]
    pub fn with_sorafs_reputation_threshold_signer(
        mut self,
        signer: Arc<dyn sorafs_node::reputation::runtime::ReputationThresholdSignerClientV1>,
    ) -> Self {
        self.sorafs_reputation_threshold_signer = Some(signer);
        self
    }

    /// Attach the authenticated Governance DAG publication/readback service for
    /// committed reputation snapshots.
    #[must_use]
    pub fn with_sorafs_reputation_governance_dag(
        mut self,
        governance_dag: Arc<dyn sorafs_node::reputation::runtime::ReputationGovernanceDagClientV1>,
    ) -> Self {
        self.sorafs_reputation_governance_dag = Some(governance_dag);
        self
    }

    /// Attach the separate sealed monotonic finalized-reputation archive
    /// retention authority.
    #[must_use]
    pub fn with_sorafs_reputation_retention_authority(
        mut self,
        authority: Arc<
            dyn iroha_core::query::reputation_finalized::ReputationFinalizedArchiveRetentionAuthorityV1,
        >,
    ) -> Self {
        self.sorafs_reputation_retention_authority = Some(authority);
        self
    }

    /// Attach the identity-pinned finalized billing query, including typed
    /// consensus-authenticated period-close records.
    #[must_use]
    pub fn with_sorafs_hedging_billing_finalized_query(
        mut self,
        query: Arc<dyn sorafs_node::hedging_billing_service::HedgingBillingFinalizedQuery>,
    ) -> Self {
        self.sorafs_hedging_billing_finalized_query = Some(query);
        self
    }

    /// Attach the consensus billing-journal inclusion/finality verifier.
    #[must_use]
    pub fn with_sorafs_hedging_billing_journal_verifier(
        mut self,
        verifier: Arc<dyn sorafs_node::hedging_billing_service::HedgingBillingJournalVerifier>,
    ) -> Self {
        self.sorafs_hedging_billing_journal_verifier = Some(verifier);
        self
    }

    /// Attach the runtime-only HSM/KMS billing statement signer.
    #[must_use]
    pub fn with_sorafs_billing_statement_signer(
        mut self,
        signer: Arc<dyn sorafs_node::hedging_billing_service::BillingStatementRuntimeSigner>,
    ) -> Self {
        self.sorafs_billing_statement_signer = Some(signer);
        self
    }

    /// Attach the authenticated immutable billing statement publisher.
    #[must_use]
    pub fn with_sorafs_billing_statement_publisher(
        mut self,
        publisher: Arc<dyn sorafs_node::hedging_billing_service::BillingStatementPublisher>,
    ) -> Self {
        self.sorafs_billing_statement_publisher = Some(publisher);
        self
    }

    /// Attach the authoritative billing statement acknowledgement service.
    #[must_use]
    pub fn with_sorafs_billing_acknowledgement_authority(
        mut self,
        authority: Arc<
            dyn sorafs_node::hedging_billing_service::BillingStatementAcknowledgementAuthority,
        >,
    ) -> Self {
        self.sorafs_billing_acknowledgement_authority = Some(authority);
        self
    }

    /// Attach the authenticated monotonic sealed billing epoch witness store.
    #[must_use]
    pub fn with_sorafs_hedging_billing_epoch_witness_store(
        mut self,
        store: Arc<dyn sorafs_node::hedging_billing_service::HedgingBillingEpochWitnessStore>,
    ) -> Self {
        self.sorafs_hedging_billing_epoch_witness_store = Some(store);
        self
    }

    /// Attach the authenticated governed source-fetch boundary used by local
    /// finalized replication ingest.
    #[must_use]
    pub fn with_sorafs_provider_ingest_authenticated_source(
        mut self,
        source: Arc<dyn sorafs_provider_ingest_runtime::ProviderIngestAuthenticatedSourceRuntimeV1>,
    ) -> Self {
        self.sorafs_provider_ingest_authenticated_source = Some(source);
        self
    }

    /// Attach the governance-aware runtime HSM/KMS completion-signer resolver.
    #[must_use]
    pub fn with_sorafs_provider_ingest_signer_resolver(
        mut self,
        resolver: Arc<
            dyn sorafs_provider_ingest_runtime::ProviderIngestGovernedSignerResolverRuntimeV1,
        >,
    ) -> Self {
        self.sorafs_provider_ingest_signer_resolver = Some(resolver);
        self
    }

    /// Attach the sealed monotonic provider-ingest checkpoint authority.
    #[must_use]
    pub fn with_sorafs_provider_ingest_checkpoint_runtime(
        mut self,
        runtime: Arc<dyn sorafs_node::ProviderIngestCheckpointRuntimeV1>,
    ) -> Self {
        self.sorafs_provider_ingest_checkpoint_runtime = Some(runtime);
        self
    }

    /// Attach the separate sealed monotonic finalized-archive retention authority.
    #[must_use]
    pub fn with_sorafs_provider_ingest_retention_authority(
        mut self,
        authority: Arc<
            dyn iroha_core::query::provider_ingest_finalized::ProviderIngestFinalizedArchiveRetentionAuthorityV1,
        >,
    ) -> Self {
        self.sorafs_provider_ingest_retention_authority = Some(authority);
        self
    }

    /// Attach the authenticated immutable finalized-PoR replay archive.
    ///
    /// Archive credentials and the Ed25519 private signing key remain inside
    /// the deployment-owned implementation.
    #[must_use]
    pub fn with_sorafs_por_finalized_replay_archive(
        mut self,
        archive: Arc<dyn sorafs_node::PorFinalizedReplayArchiveV1>,
    ) -> Self {
        self.sorafs_por_finalized_replay_archive = Some(archive);
        self
    }

    /// Attach the rollback-resistant monotonic clock seal reserved for the
    /// supervised Musubi provider-attestation journal.
    #[must_use]
    pub fn with_sorafs_musubi_provider_attestation_clock_seal(
        mut self,
        seal: Arc<dyn sorafs_node::MusubiProviderAttestationClockSealV1>,
    ) -> Self {
        self.sorafs_musubi_provider_attestation_clock_seal = Some(seal);
        self
    }

    /// Attach the approval-only HSM/KMS or threshold signer reserved for the
    /// supervised Musubi provider-attestation journal.
    #[must_use]
    pub fn with_sorafs_musubi_provider_attestation_approval_signer(
        mut self,
        signer: Arc<dyn sorafs_node::MusubiProviderAttestationSignerV1>,
    ) -> Self {
        self.sorafs_musubi_provider_attestation_approval_signer = Some(signer);
        self
    }

    /// Attach the authenticated coordinator inventory reserved for the
    /// supervised Musubi provider-attestation journal.
    #[must_use]
    pub fn with_sorafs_musubi_provider_attestation_inventory(
        mut self,
        inventory: Arc<dyn sorafs_node::MusubiProviderAttestationInventoryRuntimeV1>,
    ) -> Self {
        self.sorafs_musubi_provider_attestation_inventory = Some(inventory);
        self
    }
}
