/// Runtime-only daemon dependencies supplied by the deployment launcher.
///
/// Implementations of the moderation wrapper, privacy-cycle PRF provider, stream-token and native
/// proof/repair/reserve/orderbook/moderation signers, moderation durable handoffs, evidence-viewer
/// checkpoint authority, appeal-finance transaction signers, role-separated `PoTR` signers,
/// exact-view billing queries, threshold/HSM signers, immutable publication, acknowledgement,
/// sealed witness storage, authenticated Governance DAG publication/readback/head updates, sealed
/// monotonic Governance DAG checkpoints, externally sealed reputation journal checkpoints, the
/// Soracloud mutation/provenance signer, and the authenticated Hugging Face credential provider,
/// plus the reserved Musubi provider- attestation clock, approval signer, and authenticated
/// inventory, are the reference-node boundaries for ledger access, PKCS#11, managed-KMS, and
/// threshold services. Provider credentials, unwrapped keys, PRF shares, seeds, and outputs must
/// stay inside those implementations and must never be sourced from `iroha_config`.
#[derive(Clone, Default)]
pub struct IrohaRuntimeDeps {
    sumeragi_global_beacon_partial_signer:
        Option<Arc<dyn iroha_core::beacon::GlobalThresholdBeaconPartialSignerV1>>,
    parliament_tle_partial_release_signer:
        Option<Arc<dyn iroha_core::tle_release::TlePartialReleaseSignerV1>>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThresholdSignerStartupReadinessV1 {
    Ready,
    ParliamentTleShareUnavailable {
        key_session_id: iroha_core::governance::timed_ovn::TleKeySessionId,
    },
}

fn require_global_beacon_signer_for_local_seat_v1(
    local_has_committee_seat: bool,
    signer_is_resolved: bool,
) -> Result<(), &'static str> {
    if local_has_committee_seat && !signer_is_resolved {
        return Err("local global-beacon committee seat has no resolved partial signer");
    }
    Ok(())
}

fn parliament_tle_signer_readiness_for_local_seat_v1(
    key_session_id: iroha_core::governance::timed_ovn::TleKeySessionId,
    local_has_committee_seat: bool,
    signer_is_resolved: bool,
) -> ThresholdSignerStartupReadinessV1 {
    if local_has_committee_seat && !signer_is_resolved {
        ThresholdSignerStartupReadinessV1::ParliamentTleShareUnavailable { key_session_id }
    } else {
        ThresholdSignerStartupReadinessV1::Ready
    }
}

fn validate_threshold_signer_startup_readiness_v1(
    state: &iroha_core::state::State,
    local_peer: &PeerId,
    runtime_deps: &IrohaRuntimeDeps,
) -> Result<ThresholdSignerStartupReadinessV1, &'static str> {
    use iroha_core::state::{GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, WorldReadOnly as _};

    let world = state.world_view();
    let topology = state.commit_topology_snapshot();
    let topology_roster_hash =
        iroha_core::beacon::global_threshold_beacon_roster_hash_v1(&topology);
    let committed_height = u64::try_from(state.committed_height()).unwrap_or(u64::MAX);
    if let Some(active_session_id) = world
        .global_beacon_active_session()
        .get(&GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY)
        .copied()
    {
        let record = world
            .global_beacon_key_sessions()
            .get(&active_session_id)
            .ok_or("active global-beacon key session is absent")?;
        record
            .validate()
            .map_err(|_| "active global-beacon key session is invalid")?;
        if record.session.network_id != *state.network_id_ref()
            || record.session.roster_hash != topology_roster_hash
            || usize::from(record.session.committee_size) != topology.len()
            || !record.is_active_at(committed_height.saturating_add(1))
        {
            return Err("active global-beacon key session is not bound to the startup roster");
        }
        require_global_beacon_signer_for_local_seat_v1(
            topology.iter().any(|peer| peer == local_peer),
            runtime_deps.sumeragi_global_beacon_partial_signer.is_some(),
        )?;
    }

    let Some(tle_key_session_id) = world.active_tle_key_session() else {
        return Ok(ThresholdSignerStartupReadinessV1::Ready);
    };
    let tle_session = world
        .tle_key_sessions()
        .get(&tle_key_session_id)
        .ok_or("active Parliament TLE key session is absent")?;
    tle_session
        .clone()
        .validate()
        .map_err(|_| "active Parliament TLE key session is invalid")?;
    if tle_session.network_id != *state.network_id_ref().as_bytes() {
        return Err("active Parliament TLE key session belongs to another network");
    }
    Ok(parliament_tle_signer_readiness_for_local_seat_v1(
        tle_key_session_id,
        tle_session.roster_hash == topology_roster_hash
            && topology.iter().any(|peer| peer == local_peer),
        runtime_deps.parliament_tle_partial_release_signer.is_some(),
    ))
}
macro_rules! define_runtime_dep_setters_v1 {
    (
        $(
            $(#[$attribute:meta])*
            $name:ident($argument:ident: $dependency:ty $(,)?) => $field:ident;
        )+
    ) => {
        $(
            $(#[$attribute])*
            #[must_use]
            pub fn $name(mut self, $argument: $dependency) -> Self {
                self.$field = Some($argument);
                self
            }
        )+
    };
}

impl IrohaRuntimeDeps {
    /// Return whether no deployment-owned runtime dependency is attached.
    ///
    /// The standard launcher uses this to reject a registry that returns
    /// process-local authority when configuration requested no provider.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sumeragi_global_beacon_partial_signer.is_none()
            && self.parliament_tle_partial_release_signer.is_none()
            && self.bootle_lantern_issuance_provider_registry.is_none()
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

    /// Build the opaque Core coordinator used by Torii's authenticated local
    /// Parliament partial-release route.
    ///
    /// A missing deployment provider produces a fail-closed coordinator. The
    /// provider trait object is cloned only as an `Arc`; secret-share material
    /// remains inside the runtime implementation and is never projected into
    /// daemon configuration or route state.
    pub(crate) fn parliament_tle_release_coordinator(
        &self,
    ) -> Arc<iroha_core::tle_release::TleReleaseCoordinatorV1> {
        Arc::new(
            self.parliament_tle_partial_release_signer
                .clone()
                .map_or_else(
                    iroha_core::tle_release::TleReleaseCoordinatorV1::without_signer,
                    iroha_core::tle_release::TleReleaseCoordinatorV1::from_signer,
                ),
        )
    }
    define_runtime_dep_setters_v1! {
        /// Attach the runtime-only adaptive threshold-beacon signing-share owner.
        with_sumeragi_global_beacon_partial_signer(
            signer: Arc<dyn iroha_core::beacon::GlobalThresholdBeaconPartialSignerV1>,
        ) => sumeragi_global_beacon_partial_signer;
        /// Attach the runtime-only adaptive Parliament TLE signing-share owner.
        ///
        /// Private DKG components remain inside this provider. They are never
        /// read from configuration, serialized, or logged by the daemon. A
        /// production provider owns active/retiring session selection and keeps
        /// old shares available through all committed opening deadlines.
        with_parliament_tle_partial_release_signer(
            signer: Arc<dyn iroha_core::tle_release::TlePartialReleaseSignerV1>,
        ) => parliament_tle_partial_release_signer;
        /// Attach the deployment-owned Bootle/Lantern issuer and authentication registry.
        with_bootle_lantern_issuance_provider_registry(
            registry: Arc<
                dyn iroha_torii::privacy_issuance_api::BootleLanternIssuanceRuntimeProviderRegistryV1,
            >,
        ) => bootle_lantern_issuance_provider_registry;
        /// Attach the production PKCS#11/KMS wrapper for moderation quarantine object data keys.
        with_moderation_quarantine_key_wrapper(
            key_wrapper: Arc<dyn sorafs_node::ModerationQuarantineKeyWrapper>,
        ) => moderation_quarantine_key_wrapper;
        /// Attach the production threshold-PRF provider for differential-privacy publication cycles.
        with_privacy_cycle_prf_provider(
            provider: Arc<dyn sorafs_node::ProductionPrivacyCyclePrfProviderV1>,
        ) => privacy_cycle_prf_provider;
        /// Attach the independently administered finalized privacy-release head.
        with_privacy_release_anchor(
            anchor: Arc<dyn sorafs_node::ProductionPrivacyReleaseAnchorV1>,
        ) => privacy_release_anchor;
        /// Attach the production external sealed-CAS transparency leader lease.
        with_transparency_leader_lease_provider(
            provider: Arc<dyn sorafs_node::ProductionTransparencyLeaderLeaseProviderV1>,
        ) => transparency_leader_lease_provider;
        /// Attach the deployment-owned fused privacy Governance target writer.
        ///
        /// Enabled privacy publication requires this writer and an authenticated
        /// head reader. Both roles must expose the exact configured handle,
        /// revision, and policy digest; partial or mismatched pairs fail startup.
        with_sorafs_fenced_transparency_publisher(
            publisher: Arc<dyn sorafs_node::FencedTransparencyPublisherV1>,
        ) => sorafs_fenced_transparency_publisher;
        /// Attach the authenticated authoritative-head reader paired with the
        /// fused privacy target writer.
        ///
        /// Enabled privacy publication requires both roles to expose the exact configured handle,
        /// revision, and policy digest; partial or mismatched pairs fail startup.
        with_sorafs_fenced_transparency_head_reader(
            reader: Arc<dyn sorafs_node::FencedTransparencyAuthoritativeHeadReaderV1>,
        ) => sorafs_fenced_transparency_head_reader;
        /// Attach the production HSM/KMS signer for the embedded `SoraFS` Governance DAG publisher.
        with_sorafs_governance_dag_signer(
            signer: Arc<dyn sorafs_node::GovernanceDagRuntimeSigner>,
        ) => sorafs_governance_dag_signer;
        /// Attach the production Kubo/IPFS/IPNS request authenticator for the
        /// supervised Governance DAG service.
        with_sorafs_governance_dag_ipfs_authenticator(
            authenticator: Arc<dyn sorafs_node::GovernanceDagRequestAuthenticator>,
        ) => sorafs_governance_dag_ipfs_authenticator;
        /// Attach the production signed-head compare-and-swap authenticator for
        /// the supervised Governance DAG service.
        with_sorafs_governance_dag_head_authenticator(
            authenticator: Arc<dyn sorafs_node::GovernanceDagRequestAuthenticator>,
        ) => sorafs_governance_dag_head_authenticator;
        /// Attach the sealed monotonic checkpoint and publish-intent store for the
        /// supervised Governance DAG service.
        with_sorafs_governance_dag_checkpoint_store(
            checkpoint_store: Arc<dyn sorafs_node::GovernanceDagSealedCheckpointStore>,
        ) => sorafs_governance_dag_checkpoint_store;
        /// Attach the production HSM/KMS signer for `SoraFS` stream-token issuance.
        with_sorafs_stream_token_signer(
            signer: Arc<dyn iroha_torii::sorafs::StreamTokenRuntimeSigner>,
        ) => sorafs_stream_token_signer;
        /// Attach the deployment-owned atomic stream-token quota, sealed sequence,
        /// and ordered callback-outbox provider.
        with_sorafs_stream_token_gateway_admission(
            provider: Arc<dyn iroha_torii::sorafs::StreamTokenGatewayAdmissionProviderV1>,
        ) => sorafs_stream_token_gateway_admission;
        /// Attach runtime-only HSM/KMS providers for appeal-finance lock,
        /// disbursement, and refund transactions.
        with_sorafs_appeal_finance_runtime_signers(
            signers: Arc<iroha_torii::SoraFsAppealFinanceRuntimeSignersV1>,
        ) => sorafs_appeal_finance_runtime_signers;
        /// Attach the HSM/KMS-authenticated monotonic checkpoint boundary for the
        /// appeal-finance transaction forwarder.
        with_sorafs_appeal_finance_checkpoint_runtime(
            runtime: Arc<
                dyn sorafs_node::appeal_finance_transaction_forwarder::AppealFinanceCheckpointRuntime,
            >,
        ) => sorafs_appeal_finance_checkpoint_runtime;
        /// Attach a raw runtime-only signer for authoritative proof-outcome transactions.
        ///
        /// The deployment registry resolver replaces this provider with an immutable facade qualified
        /// against the exact configured role, authority, algorithm, key, revision, and policy digest.
        with_sorafs_proof_outcome_signer(
            signer: Arc<dyn iroha_torii::SoraFsProofOutcomeTransactionSigner>,
        ) => sorafs_proof_outcome_signer;
        /// Attach a raw runtime-only signer for native repair transactions.
        ///
        /// The deployment registry resolver replaces this provider with an immutable facade qualified
        /// against the exact configured role, authority, algorithm, key, revision, and policy digest.
        with_sorafs_repair_transaction_signer(
            signer: Arc<dyn iroha_torii::SoraFsRepairTransactionSigner>,
        ) => sorafs_repair_transaction_signer;
        /// Attach a raw runtime-only signer for native reserve/rent transactions.
        ///
        /// The deployment registry resolver replaces this provider with an immutable facade qualified
        /// against the exact configured role, authority, algorithm, key, revision, and policy digest.
        with_sorafs_reserve_transaction_signer(
            signer: Arc<dyn iroha_torii::SoraFsReserveTransactionSigner>,
        ) => sorafs_reserve_transaction_signer;
        /// Attach a raw runtime-only signer for native orderbook transactions.
        ///
        /// The deployment registry resolver replaces this provider with an immutable facade qualified
        /// against the exact configured role, authority, algorithm, key, revision, and policy digest.
        with_sorafs_orderbook_transaction_signer(
            signer: Arc<dyn iroha_torii::SoraFsOrderbookTransactionSigner>,
        ) => sorafs_orderbook_transaction_signer;
        /// Attach the raw deployment-owned Soracloud transaction and provenance signer.
        ///
        /// The runtime-provider registry replaces this provider with an immutable
        /// facade qualified against the exact configured handle, authority, key,
        /// revision, policy digest, active posture, and non-test posture.
        with_soracloud_runtime_mutation_signer(
            signer: Arc<dyn soracloud_runtime_signer::SoracloudRuntimeMutationSignerV1>,
        ) => soracloud_runtime_mutation_signer;
        /// Attach the raw deployment-owned authenticated HF credential provider.
        ///
        /// The registry resolver replaces this provider with an immutable facade qualified against the
        /// exact configured handle, revision, policy digest, active posture, and non-test posture.
        /// Bearer credentials remain inside the provider.
        with_soracloud_hf_inference_credential_provider(
            provider: Arc<dyn soracloud_hf_credential::SoracloudHfInferenceCredentialProviderV1>,
        ) => soracloud_hf_inference_credential_provider;
        /// Attach the runtime-only HSM/KMS signer for exact moderation native transaction envelopes.
        with_sorafs_moderation_transaction_signer(
            signer: Arc<
                dyn iroha_torii::sorafs::moderation_runtime::ModerationSignedTransactionSignerV1,
            >,
        ) => sorafs_moderation_transaction_signer;
        /// Attach the durable appeal-finance boundary for finalized moderation settlement handoffs.
        with_sorafs_moderation_settlement_handoff(
            boundary: Arc<
                dyn iroha_torii::sorafs::moderation_runtime::ModerationDurableHandoffBoundaryV1,
            >,
        ) => sorafs_moderation_settlement_handoff;
        /// Attach the durable governance/transparency boundary for finalized
        /// moderation publication handoffs.
        with_sorafs_moderation_publication_handoff(
            boundary: Arc<
                dyn iroha_torii::sorafs::moderation_runtime::ModerationDurableHandoffBoundaryV1,
            >,
        ) => sorafs_moderation_publication_handoff;
        /// Attach the durable payload-free juror-notification boundary.
        with_sorafs_moderation_panel_notification(
            boundary: Arc<
                dyn iroha_torii::sorafs::moderation_runtime::ModerationDurablePanelNotificationBoundaryV1,
            >,
        ) => sorafs_moderation_panel_notification;
        /// Attach the immutable authenticated moderation notification-receipt archive.
        with_sorafs_moderation_panel_notification_archive(
            archive: Arc<
                dyn sorafs_node::moderation_orchestrator::ModerationPanelNotificationArchiveV1,
            >,
        ) => sorafs_moderation_panel_notification_archive;
        /// Attach the deployment-owned sealed monotonic moderation checkpoint authority.
        with_sorafs_moderation_checkpoint_store(
            checkpoint_store: Arc<
                dyn sorafs_node::moderation_orchestrator::ModerationCheckpointStoreV1,
            >,
        ) => sorafs_moderation_checkpoint_store;
        /// Attach the production `WebAuthn` verifier for evidence-viewer sessions.
        with_sorafs_evidence_viewer_webauthn(
            boundary: Arc<dyn sorafs_node::evidence_viewer::EvidenceViewerWebAuthnBoundaryV1>,
        ) => sorafs_evidence_viewer_webauthn;
        /// Attach the finalized assignment/role grant authority for evidence viewing.
        with_sorafs_evidence_viewer_grants(
            boundary: Arc<dyn sorafs_node::evidence_viewer::EvidenceViewerGrantBoundaryV1>,
        ) => sorafs_evidence_viewer_grants;
        /// Attach the HSM-backed signer for hash-chained evidence access receipts.
        with_sorafs_evidence_viewer_receipt_signer(
            signer: Arc<dyn sorafs_node::evidence_viewer::EvidenceViewerReceiptSignerV1>,
        ) => sorafs_evidence_viewer_receipt_signer;
        /// Attach the authenticated evidence erasure boundary. Its implementation
        /// owns KMS/storage credentials and must honor stable operation IDs.
        with_sorafs_evidence_viewer_erasure(
            boundary: Arc<dyn sorafs_node::evidence_viewer::EvidenceViewerErasureBoundaryV1>,
        ) => sorafs_evidence_viewer_erasure;
        /// Attach the deployment-owned linearizable evidence-viewer checkpoint authority. Its
        /// implementation owns all CAS credentials and sealed persistence state.
        with_sorafs_evidence_viewer_checkpoint_store(
            checkpoint_store: Arc<dyn sorafs_node::evidence_viewer::EvidenceViewerCheckpointStoreV1>,
        ) => sorafs_evidence_viewer_checkpoint_store;
        /// Attach the authenticated immutable evidence-viewer compaction archive.
        ///
        /// Archive credentials and its Ed25519 private signing key remain inside
        /// the deployment-owned implementation.
        with_sorafs_evidence_viewer_compaction_archive(
            archive: Arc<dyn sorafs_node::evidence_viewer::EvidenceViewerCompactionArchiveV1>,
        ) => sorafs_evidence_viewer_compaction_archive;
        /// Attach the deployment-owned signed monotonic evidence transparency publisher.
        ///
        /// Publisher credentials and the Ed25519 private signing key remain inside
        /// the deployment-owned implementation.
        with_sorafs_evidence_viewer_transparency_publisher(
            publisher: Arc<
                dyn sorafs_node::evidence_viewer::transparency_producer::
                    EvidenceViewerTransparencyPublisherV1,
            >,
        ) => sorafs_evidence_viewer_transparency_publisher;
        /// Attach the deployment-owned registry for all runtime-only `PoP` enrollment, issuer,
        /// finalized-query, wallet, and authentication providers.
        with_sorafs_pop_credential_provider_registry(
            provider_registry: Arc<
                dyn iroha_torii::sorafs::pop_api::PopCredentialRuntimeProviderRegistryV1,
            >,
        ) => sorafs_pop_credential_provider_registry;
        /// Attach independently administered runtime HSM services for the `SoraFS`
        /// `PoTR` gateway Ed25519 and provider ML-DSA-65 receipt roles.
        ///
        /// Torii binds these roles to its own authoritative finalized state after
        /// state and the council-verified admission registry are available.
        with_sorafs_potr_runtime_signer_roles(
            roles: Arc<iroha_torii::sorafs::PotrRuntimeSignerRolesV1>,
        ) => sorafs_potr_runtime_signer_roles;
        /// Attach the runtime-owned ACME client used by the `SoraFS` regional gateway.
        ///
        /// Account and DNS-provider credentials remain inside the implementation
        /// and never enter resolved configuration or Torii state.
        with_sorafs_gateway_acme_client(
            client: Arc<dyn iroha_torii::sorafs::gateway::AcmeClient>,
        ) => sorafs_gateway_acme_client;
        /// Attach the authenticated, address-pinned `SoraFS` compliance feed transport.
        ///
        /// Bearer tokens, client identities, DNS credentials, and TLS key material
        /// remain owned by the deployment adapter.
        with_sorafs_gateway_compliance_feed_transport(
            transport: Arc<dyn iroha_torii::sorafs::gateway::GatewayComplianceFeedTransport>,
        ) => sorafs_gateway_compliance_feed_transport;
        /// Attach a runtime-only identity-matching signer and normal-queue
        /// submitter for native `PoR` and stream-token reputation journal entries.
        with_sorafs_reputation_journal_transaction_submitter(
            submitter: Arc<
                dyn sorafs_node::reputation::runtime::ReputationJournalTransactionSubmitterV1,
            >,
        ) => sorafs_reputation_journal_transaction_submitter;
        /// Attach the externally sealed monotonic checkpoint provider for the
        /// native reputation journal outbox.
        with_sorafs_reputation_journal_checkpoint_provider(
            provider: Arc<dyn sorafs_node::reputation::runtime::ReputationJournalCheckpointRuntimeV1>,
        ) => sorafs_reputation_journal_checkpoint_provider;
        /// Attach the external threshold-signing service for exact committed reputation material.
        with_sorafs_reputation_threshold_signer(
            signer: Arc<dyn sorafs_node::reputation::runtime::ReputationThresholdSignerClientV1>,
        ) => sorafs_reputation_threshold_signer;
        /// Attach the authenticated Governance DAG publication/readback service for
        /// committed reputation snapshots.
        with_sorafs_reputation_governance_dag(
            governance_dag: Arc<dyn sorafs_node::reputation::runtime::ReputationGovernanceDagClientV1>,
        ) => sorafs_reputation_governance_dag;
        /// Attach the separate sealed monotonic finalized-reputation archive retention authority.
        with_sorafs_reputation_retention_authority(
            authority: Arc<
                dyn iroha_core::query::reputation_finalized::ReputationFinalizedArchiveRetentionAuthorityV1,
            >,
        ) => sorafs_reputation_retention_authority;
        /// Attach the identity-pinned finalized billing query, including typed
        /// consensus-authenticated period-close records.
        with_sorafs_hedging_billing_finalized_query(
            query: Arc<dyn sorafs_node::hedging_billing_service::HedgingBillingFinalizedQuery>,
        ) => sorafs_hedging_billing_finalized_query;
        /// Attach the consensus billing-journal inclusion/finality verifier.
        with_sorafs_hedging_billing_journal_verifier(
            verifier: Arc<dyn sorafs_node::hedging_billing_service::HedgingBillingJournalVerifier>,
        ) => sorafs_hedging_billing_journal_verifier;
        /// Attach the runtime-only HSM/KMS billing statement signer.
        with_sorafs_billing_statement_signer(
            signer: Arc<dyn sorafs_node::hedging_billing_service::BillingStatementRuntimeSigner>,
        ) => sorafs_billing_statement_signer;
        /// Attach the authenticated immutable billing statement publisher.
        with_sorafs_billing_statement_publisher(
            publisher: Arc<dyn sorafs_node::hedging_billing_service::BillingStatementPublisher>,
        ) => sorafs_billing_statement_publisher;
        /// Attach the authoritative billing statement acknowledgement service.
        with_sorafs_billing_acknowledgement_authority(
            authority: Arc<
                dyn sorafs_node::hedging_billing_service::BillingStatementAcknowledgementAuthority,
            >,
        ) => sorafs_billing_acknowledgement_authority;
        /// Attach the authenticated monotonic sealed billing epoch witness store.
        with_sorafs_hedging_billing_epoch_witness_store(
            store: Arc<dyn sorafs_node::hedging_billing_service::HedgingBillingEpochWitnessStore>,
        ) => sorafs_hedging_billing_epoch_witness_store;
        /// Attach the authenticated governed source-fetch boundary used by local
        /// finalized replication ingest.
        with_sorafs_provider_ingest_authenticated_source(
            source: Arc<dyn sorafs_provider_ingest_runtime::ProviderIngestAuthenticatedSourceRuntimeV1>,
        ) => sorafs_provider_ingest_authenticated_source;
        /// Attach the governance-aware runtime HSM/KMS completion-signer resolver.
        with_sorafs_provider_ingest_signer_resolver(
            resolver: Arc<
                dyn sorafs_provider_ingest_runtime::ProviderIngestGovernedSignerResolverRuntimeV1,
            >,
        ) => sorafs_provider_ingest_signer_resolver;
        /// Attach the sealed monotonic provider-ingest checkpoint authority.
        with_sorafs_provider_ingest_checkpoint_runtime(
            runtime: Arc<dyn sorafs_node::ProviderIngestCheckpointRuntimeV1>,
        ) => sorafs_provider_ingest_checkpoint_runtime;
        /// Attach the separate sealed monotonic finalized-archive retention authority.
        with_sorafs_provider_ingest_retention_authority(
            authority: Arc<
                dyn iroha_core::query::provider_ingest_finalized::ProviderIngestFinalizedArchiveRetentionAuthorityV1,
            >,
        ) => sorafs_provider_ingest_retention_authority;
        /// Attach the authenticated immutable finalized-PoR replay archive.
        ///
        /// Archive credentials and the Ed25519 private signing key remain inside
        /// the deployment-owned implementation.
        with_sorafs_por_finalized_replay_archive(
            archive: Arc<dyn sorafs_node::PorFinalizedReplayArchiveV1>,
        ) => sorafs_por_finalized_replay_archive;
        /// Attach the rollback-resistant monotonic clock seal reserved for the
        /// supervised Musubi provider-attestation journal.
        with_sorafs_musubi_provider_attestation_clock_seal(
            seal: Arc<dyn sorafs_node::MusubiProviderAttestationClockSealV1>,
        ) => sorafs_musubi_provider_attestation_clock_seal;
        /// Attach the approval-only HSM/KMS or threshold signer reserved for the
        /// supervised Musubi provider-attestation journal.
        with_sorafs_musubi_provider_attestation_approval_signer(
            signer: Arc<dyn sorafs_node::MusubiProviderAttestationSignerV1>,
        ) => sorafs_musubi_provider_attestation_approval_signer;
        /// Attach the authenticated coordinator inventory reserved for the
        /// supervised Musubi provider-attestation journal.
        with_sorafs_musubi_provider_attestation_inventory(
            inventory: Arc<dyn sorafs_node::MusubiProviderAttestationInventoryRuntimeV1>,
        ) => sorafs_musubi_provider_attestation_inventory;
    }
}

#[cfg(test)]
mod parliament_tle_release_tests {
    use super::*;

    struct UnavailableSigner;

    impl iroha_core::tle_release::TlePartialReleaseSignerV1 for UnavailableSigner {
        fn sign_partial_release(
            &self,
            _context: &iroha_core::tle_release::AuthorizedTleReleaseContextV1,
        ) -> Result<iroha_core::tle_release::TlePartialReleaseShareV1, String> {
            Err("unavailable".to_owned())
        }
    }

    #[test]
    fn parliament_tle_coordinator_is_fail_closed_or_runtime_injected() {
        let absent = IrohaRuntimeDeps::default().parliament_tle_release_coordinator();
        assert!(!absent.signer_is_available());

        let injected = IrohaRuntimeDeps::default()
            .with_parliament_tle_partial_release_signer(Arc::new(UnavailableSigner))
            .parliament_tle_release_coordinator();
        assert!(injected.signer_is_available());
    }

    #[test]
    fn local_threshold_committee_seats_surface_missing_runtime_signers() {
        assert_eq!(
            require_global_beacon_signer_for_local_seat_v1(true, false),
            Err("local global-beacon committee seat has no resolved partial signer")
        );
        assert_eq!(
            require_global_beacon_signer_for_local_seat_v1(true, true),
            Ok(())
        );
        assert_eq!(
            require_global_beacon_signer_for_local_seat_v1(false, false),
            Ok(())
        );

        let session_id = iroha_core::governance::timed_ovn::TleKeySessionId::new([0x71; 32]);
        assert_eq!(
            parliament_tle_signer_readiness_for_local_seat_v1(session_id, true, false),
            ThresholdSignerStartupReadinessV1::ParliamentTleShareUnavailable {
                key_session_id: session_id,
            }
        );
        assert_eq!(
            parliament_tle_signer_readiness_for_local_seat_v1(session_id, true, true),
            ThresholdSignerStartupReadinessV1::Ready
        );
    }
}
