use mv::storage::StorageReadOnly as _;

/// Runtime-only daemon dependencies supplied by the deployment launcher.
///
/// Implementations of the moderation wrapper, privacy-cycle PRF provider, stream-token and native
/// proof/repair/reserve/orderbook/moderation signers, moderation durable handoffs, evidence-viewer
/// checkpoint authority, appeal-finance transaction signers, role-separated `PoTR` signers,
/// exact-view billing queries, threshold/HSM signers, immutable publication, acknowledgement,
/// sealed witness storage, authenticated Governance DAG publication/readback/head updates, sealed
/// monotonic Governance DAG checkpoints, externally sealed reputation journal checkpoints, the
/// Soracloud mutation/provenance signer, plus the reserved Musubi provider-attestation clock,
/// approval signer, and authenticated
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
    sorafs_musubi_provider_attestation_clock_seal:
        Option<Arc<dyn sorafs_node::MusubiProviderAttestationClockSealV1>>,
    sorafs_musubi_provider_attestation_approval_signer:
        Option<Arc<dyn sorafs_node::MusubiProviderAttestationSignerV1>>,
    sorafs_musubi_provider_attestation_inventory:
        Option<Arc<dyn sorafs_node::MusubiProviderAttestationInventoryRuntimeV1>>,
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

fn require_parliament_tle_signer_for_local_seat_v1(
    local_has_committee_seat: bool,
    signer_is_resolved: bool,
) -> Result<(), &'static str> {
    if local_has_committee_seat && !signer_is_resolved {
        return Err("local Parliament TLE committee seat has no resolved partial-release signer");
    }
    Ok(())
}

fn require_parliament_tle_session_startup_binding_v1(
    session_network_id: [u8; 32],
    session_roster_hash: [u8; 32],
    session_committee_size: u16,
    startup_network_id: [u8; 32],
    startup_roster_hash: [u8; 32],
    startup_committee_size: usize,
) -> Result<(), &'static str> {
    if session_network_id != startup_network_id {
        return Err("active Parliament TLE key session belongs to another network");
    }
    if session_roster_hash != startup_roster_hash
        || usize::from(session_committee_size) != startup_committee_size
    {
        return Err("active Parliament TLE key session is not bound to the startup roster");
    }
    Ok(())
}

fn parliament_tle_local_participant_index_v1(
    frozen_roster: &[PeerId],
    local_peer: &PeerId,
) -> Result<Option<u16>, &'static str> {
    let mut local_index = None;
    for (index, peer) in frozen_roster.iter().enumerate() {
        if frozen_roster[..index].contains(peer) {
            return Err("Parliament TLE key-session roster contains a duplicate peer");
        }
        if peer == local_peer {
            let index = u16::try_from(index + 1)
                .map_err(|_| "Parliament TLE key-session participant index exceeds u16")?;
            local_index = Some(index);
        }
    }
    Ok(local_index)
}

fn require_parliament_tle_capability_for_local_seat_v1(
    local_participant_index: Option<u16>,
    signer: Option<&dyn iroha_core::tle_release::TlePartialReleaseSignerV1>,
    session: &iroha_core::tle_release::ValidatedTleKeySessionV1,
) -> Result<(), &'static str> {
    require_parliament_tle_signer_for_local_seat_v1(
        local_participant_index.is_some(),
        signer.is_some(),
    )?;
    let (Some(participant_index), Some(signer)) = (local_participant_index, signer) else {
        return Ok(());
    };
    let attestation = signer
        .attest_partial_release_capability(session, participant_index)
        .map_err(
            |_| "local Parliament TLE committee seat has no exact runtime custody attestation",
        )?;
    if !attestation.matches(session, participant_index) {
        return Err(
            "local Parliament TLE committee seat returned a mismatched runtime custody attestation",
        );
    }
    Ok(())
}

/// Validate runtime custody for every active or deadline-retained threshold session assigned to
/// the local peer.
///
/// Private timed-OVN ballots have no plaintext or manual-opening fallback, so a local seat in the
/// frozen roster of any required Parliament TLE session is not operational without an exact live
/// capability lookup in its runtime partial-release signer.
fn validate_threshold_signer_startup_readiness_v1(
    state: &iroha_core::state::State,
    local_peer: &PeerId,
    runtime_deps: &IrohaRuntimeDeps,
) -> Result<(), &'static str> {
    use iroha_core::state::WorldReadOnly as _;

    let world = state.world_view();
    let topology = state.commit_topology_snapshot();
    let topology_roster_hash =
        iroha_core::beacon::global_threshold_beacon_roster_hash_v1(&topology);
    let committed_height = u64::try_from(state.committed_height()).unwrap_or(u64::MAX);
    let active_session_id =
        iroha_core::beacon::active_global_threshold_beacon_session_id_v1(&world)
            .map_err(|_| "active global-beacon key-session storage is noncanonical")?;
    if let Some(active_session_id) = active_session_id {
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

    let next_height = committed_height.checked_add(1).unwrap_or(committed_height);
    let active_tle_key_session_id =
        world.selectable_tle_key_session_for_fresh_ballot_at(next_height);
    let required_tle_key_sessions = world
        .tle_key_sessions_required_for_runtime_custody_v1(committed_height)
        .map_err(|_| "committed Parliament state is invalid for TLE custody readiness")?;
    for tle_key_session_id in required_tle_key_sessions {
        let public_session = world
            .tle_key_sessions()
            .get(&tle_key_session_id)
            .ok_or("required Parliament TLE key session is absent")?;
        let session = public_session
            .clone()
            .validate()
            .map_err(|_| "required Parliament TLE key session is invalid")?;
        if public_session.network_id != *state.network_id_ref().as_bytes() {
            return Err("required Parliament TLE key session belongs to another network");
        }
        let frozen_roster = world
            .tle_key_session_rosters()
            .get(&tle_key_session_id)
            .ok_or("required Parliament TLE key session has no frozen roster binding")?;
        if usize::from(public_session.committee_size) != frozen_roster.len()
            || public_session.roster_hash
                != iroha_core::beacon::global_threshold_beacon_roster_hash_v1(frozen_roster)
        {
            return Err("required Parliament TLE key session has an invalid frozen roster binding");
        }
        if active_tle_key_session_id == Some(tle_key_session_id) {
            require_parliament_tle_session_startup_binding_v1(
                public_session.network_id,
                public_session.roster_hash,
                public_session.committee_size,
                *state.network_id_ref().as_bytes(),
                topology_roster_hash,
                topology.len(),
            )?;
            if frozen_roster.as_slice() != topology.as_slice() {
                return Err(
                    "active Parliament TLE key session frozen roster differs from the startup topology",
                );
            }
        }
        let local_participant_index =
            parliament_tle_local_participant_index_v1(frozen_roster, local_peer)?;
        require_parliament_tle_capability_for_local_seat_v1(
            local_participant_index,
            runtime_deps
                .parliament_tle_partial_release_signer
                .as_deref(),
            &session,
        )?;
    }
    Ok(())
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
            && self.sorafs_musubi_provider_attestation_clock_seal.is_none()
            && self
                .sorafs_musubi_provider_attestation_approval_signer
                .is_none()
            && self.sorafs_musubi_provider_attestation_inventory.is_none()
    }

    /// Build the opaque Core coordinator used by Torii's authenticated local
    /// Parliament partial-release route.
    ///
    /// Ordinary startup rejects a missing provider when the committed active
    /// TLE roster assigns this node a seat. A non-seated node still receives a
    /// fail-closed coordinator. The provider trait object is cloned only as an
    /// `Arc`; secret-share material remains inside the runtime implementation
    /// and is never projected into daemon configuration or route state.
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
    use iroha_core::{
        smartcontracts::Execute as _,
        state::{
            StateTransaction, THRESHOLD_KEY_LIFECYCLE_CERTIFICATE_VERSION_V1,
            threshold_key_lifecycle_certificate_preimage_v1,
        },
        tle_release::{TleKeySessionPublicStateV1, ValidatedTleKeySessionV1},
    };
    use iroha_crypto::{
        Algorithm, Hash, HashOf, KeyPair, Signature,
        threshold_bls::{
            AdaptiveThresholdBlsParameters, DasRenDealerSecret, ThresholdBlsSession,
            TleReleasePurpose,
        },
    };
    use iroha_data_model::{
        block::BlockHeader,
        governance::types::{
            AbiVersion, ContractAbiHash, ContractCodeHash, DeployContractProposal, ProposalKind,
            TleKeySessionId,
        },
        isi::consensus_keys::{
            ApplyThresholdKeyLifecycleCertificateV1, ThresholdKeyLifecycleActionV1,
            ThresholdKeyLifecycleCertificateV1, ThresholdKeyLifecycleSignatureV1,
        },
    };
    use rand::{SeedableRng as _, rngs::StdRng};
    use std::sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    };

    struct UnavailableSigner;

    impl iroha_core::tle_release::TlePartialReleaseSignerV1 for UnavailableSigner {
        fn attest_partial_release_capability(
            &self,
            _session: &iroha_core::tle_release::ValidatedTleKeySessionV1,
            _expected_participant_index: u16,
        ) -> Result<
            iroha_core::tle_release::TlePartialReleaseCapabilityAttestationV1,
            iroha_core::tle_release::TlePartialReleaseCapabilityErrorV1,
        > {
            Err(iroha_core::tle_release::TlePartialReleaseCapabilityErrorV1::Unavailable)
        }

        fn sign_partial_release(
            &self,
            _context: &iroha_core::tle_release::AuthorizedTleReleaseContextV1,
        ) -> Result<iroha_core::tle_release::TlePartialReleaseShareV1, String> {
            Err("unavailable".to_owned())
        }
    }

    #[derive(Clone, Copy)]
    enum CapabilityMode {
        Exact,
        MismatchedSeat,
        Rejected,
    }

    struct CapabilityProbeSigner {
        mode: CapabilityMode,
        attestation_calls: Mutex<Vec<(TleKeySessionId, u16)>>,
        sign_calls: AtomicUsize,
    }

    impl CapabilityProbeSigner {
        fn new(mode: CapabilityMode) -> Self {
            Self {
                mode,
                attestation_calls: Mutex::new(Vec::new()),
                sign_calls: AtomicUsize::new(0),
            }
        }

        fn attestation_calls(&self) -> Vec<(TleKeySessionId, u16)> {
            self.attestation_calls
                .lock()
                .expect("capability call journal lock")
                .clone()
        }
    }

    impl iroha_core::tle_release::TlePartialReleaseSignerV1 for CapabilityProbeSigner {
        fn attest_partial_release_capability(
            &self,
            session: &iroha_core::tle_release::ValidatedTleKeySessionV1,
            expected_participant_index: u16,
        ) -> Result<
            iroha_core::tle_release::TlePartialReleaseCapabilityAttestationV1,
            iroha_core::tle_release::TlePartialReleaseCapabilityErrorV1,
        > {
            self.attestation_calls
                .lock()
                .expect("capability call journal lock")
                .push((
                    session.public_state().key_session_id,
                    expected_participant_index,
                ));
            match self.mode {
                CapabilityMode::Exact => {
                    iroha_core::tle_release::TlePartialReleaseCapabilityAttestationV1::for_validated_session(
                        session,
                        expected_participant_index,
                    )
                }
                CapabilityMode::MismatchedSeat => {
                    let mismatched = if expected_participant_index == 1 { 2 } else { 1 };
                    iroha_core::tle_release::TlePartialReleaseCapabilityAttestationV1::for_validated_session(
                        session,
                        mismatched,
                    )
                }
                CapabilityMode::Rejected => {
                    Err(iroha_core::tle_release::TlePartialReleaseCapabilityErrorV1::NotOwned)
                }
            }
        }

        fn sign_partial_release(
            &self,
            _context: &iroha_core::tle_release::AuthorizedTleReleaseContextV1,
        ) -> Result<iroha_core::tle_release::TlePartialReleaseShareV1, String> {
            self.sign_calls.fetch_add(1, Ordering::AcqRel);
            Err("the readiness path must never invoke signing".to_owned())
        }
    }

    fn tle_public_session_fixture_v1(
        network_id: [u8; 32],
        session_byte: u8,
        roster_hash: [u8; 32],
    ) -> TleKeySessionPublicStateV1 {
        let session = ThresholdBlsSession::<TleReleasePurpose>::new(
            network_id,
            [session_byte; 32],
            roster_hash,
            4,
            2,
        )
        .expect("construct TLE session fixture");
        let parameters =
            AdaptiveThresholdBlsParameters::derive(&session).expect("derive TLE parameters");
        let mut rng = StdRng::from_seed([session_byte.wrapping_add(5); 32]);
        let mut dealers = Vec::new();
        for participant_index in 1_u16..=3 {
            let (_, dealer) =
                DasRenDealerSecret::generate_with_rng(&parameters, participant_index, &mut rng)
                    .expect("generate TLE dealer fixture");
            dealers.push(dealer);
        }
        ValidatedTleKeySessionV1::from_qualified_dealers(session, &dealers, &[1, 2, 3], [4; 32])
            .expect("validate TLE session fixture")
            .public_state()
            .clone()
    }

    fn certified_tle_install_v1(
        state_transaction: &StateTransaction<'_, '_>,
        validator_keys: &[KeyPair],
        public_state: &TleKeySessionPublicStateV1,
    ) -> ApplyThresholdKeyLifecycleCertificateV1 {
        let ordered_roster = state_transaction.commit_topology().get();
        assert_eq!(ordered_roster.len(), validator_keys.len());
        for (peer, key) in ordered_roster.iter().zip(validator_keys) {
            assert_eq!(peer.public_key(), key.public_key());
        }
        let committee_size = u16::try_from(ordered_roster.len()).expect("small test roster");
        let quorum =
            u16::try_from((ordered_roster.len() - 1) / 3 * 2 + 1).expect("small test quorum");
        let mut certificate = ThresholdKeyLifecycleCertificateV1 {
            version: THRESHOLD_KEY_LIFECYCLE_CERTIFICATE_VERSION_V1,
            action: ThresholdKeyLifecycleActionV1::InstallParliamentTleKey,
            expected_active_session_id: state_transaction
                .world
                .active_tle_key_session()
                .map(|session_id| *session_id.as_bytes()),
            effective_height: state_transaction.block_height(),
            network_id: state_transaction.network_id,
            roster_hash: iroha_core::beacon::global_threshold_beacon_roster_hash_v1(ordered_roster),
            committee_size,
            quorum,
            session_id: *public_state.key_session_id.as_bytes(),
            transcript_hash: public_state.transcript_hash,
            public_state: norito::encode_canonical(public_state)
                .expect("encode canonical TLE public state"),
            signatures: Vec::new(),
        };
        let preimage = threshold_key_lifecycle_certificate_preimage_v1(&certificate)
            .expect("derive threshold-key lifecycle preimage");
        certificate.signatures = validator_keys
            .iter()
            .take(usize::from(quorum))
            .enumerate()
            .map(|(index, key)| ThresholdKeyLifecycleSignatureV1 {
                signer_index: u16::try_from(index).expect("small signer index"),
                signature: Signature::try_new(key.private_key(), &preimage)
                    .expect("sign threshold-key lifecycle certificate"),
            })
            .collect();
        ApplyThresholdKeyLifecycleCertificateV1 { certificate }
    }

    struct ThresholdSignerReadinessFixture {
        state: State,
        local_peer: PeerId,
        retained_key_session_id: TleKeySessionId,
        active_key_session_id: TleKeySessionId,
        retained_participant_index: u16,
        active_participant_index: u16,
    }

    fn threshold_signer_readiness_fixture_v1(
        committed_height: u64,
    ) -> ThresholdSignerReadinessFixture {
        const RETAINED_SESSION_BYTE: u8 = 0xD1;
        const ACTIVE_SESSION_BYTE: u8 = 0xE1;
        const RETENTION_DEADLINE_HEIGHT: u64 = 13;

        let validator_keys = (0_u8..4)
            .map(|index| {
                KeyPair::try_from_seed(vec![0x41_u8.saturating_add(index); 32], Algorithm::Ed25519)
                    .expect("derive deterministic validator key")
            })
            .collect::<Vec<_>>();
        let retained_roster = validator_keys
            .iter()
            .map(|key| PeerId::new(key.public_key().clone()))
            .collect::<Vec<_>>();
        let local_peer = retained_roster[1].clone();
        let mut active_validator_keys = validator_keys.clone();
        active_validator_keys.reverse();
        let active_roster = active_validator_keys
            .iter()
            .map(|key| PeerId::new(key.public_key().clone()))
            .collect::<Vec<_>>();
        let retained_participant_index = 2;
        let active_participant_index = 3;
        assert_eq!(
            retained_roster[usize::from(retained_participant_index - 1)],
            local_peer
        );
        assert_eq!(
            active_roster[usize::from(active_participant_index - 1)],
            local_peer
        );

        let network_id = NetworkId::from_genesis_hash(
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x31; Hash::LENGTH])),
        );
        let state = State::new_with_chain_and_network_id_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            "threshold-readiness-test"
                .parse()
                .expect("fixture chain id"),
            network_id,
        );
        let header = BlockHeader::new(
            NonZeroU64::new(1).expect("non-zero fixture height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut state_transaction = block.transaction();
        *state_transaction.commit_topology.get_mut() = retained_roster.clone();
        let retained_public_session = tle_public_session_fixture_v1(
            *network_id.as_bytes(),
            RETAINED_SESSION_BYTE,
            iroha_core::beacon::global_threshold_beacon_roster_hash_v1(&retained_roster),
        );
        let retained_key_session_id = retained_public_session.key_session_id;
        certified_tle_install_v1(
            &state_transaction,
            &validator_keys,
            &retained_public_session,
        )
        .execute(
            &AccountId::new(validator_keys[0].public_key().clone()),
            &mut state_transaction,
        )
        .expect("install deadline-retained TLE session");

        *state_transaction.commit_topology.get_mut() = active_roster.clone();
        let active_public_session = tle_public_session_fixture_v1(
            *network_id.as_bytes(),
            ACTIVE_SESSION_BYTE,
            iroha_core::beacon::global_threshold_beacon_roster_hash_v1(&active_roster),
        );
        let active_key_session_id = active_public_session.key_session_id;
        certified_tle_install_v1(
            &state_transaction,
            &active_validator_keys,
            &active_public_session,
        )
        .execute(
            &AccountId::new(active_validator_keys[0].public_key().clone()),
            &mut state_transaction,
        )
        .expect("install active TLE session");

        let proposal = ProposalKind::DeployContract(DeployContractProposal {
            contract_address: "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw"
                .parse()
                .expect("canonical contract address"),
            code_hash: ContractCodeHash::new([0x29; 32]),
            abi_hash: ContractAbiHash::new([0x2A; 32]),
            abi_version: AbiVersion::new(1),
            manifest_provenance: None,
        });
        let candidates = (0_u8..3)
            .map(|index| {
                let key = KeyPair::try_from_seed(
                    vec![0x61_u8.saturating_add(index); 32],
                    Algorithm::Ed25519,
                )
                .expect("derive deterministic Parliament candidate key");
                AccountId::new(key.public_key().clone())
            })
            .collect();
        let attempt = iroha_core::governance::parliament::enacted_parliament_attempt_for_testing(
            &proposal,
            candidates,
            &network_id,
            RETENTION_DEADLINE_HEIGHT,
        );
        let attempt_id = attempt.attempt().id;
        state_transaction
            .world
            .put_parliament_attempt_for_testing(attempt_id, attempt)
            .expect("persist deadline-retaining Parliament attempt");
        state_transaction.apply();
        block
            .commit_world_overlay_for_testing()
            .expect("commit startup-readiness fixture");

        {
            let mut topology = state.commit_topology.block();
            *topology.get_mut() = active_roster.clone();
            topology.commit();
        }

        let mut block_hashes = state.block_hashes.block();
        while u64::try_from(block_hashes.len()).unwrap_or(u64::MAX) < committed_height {
            let marker = u8::try_from(block_hashes.len() + 1).expect("small fixture height");
            block_hashes.push_for_tests(HashOf::<BlockHeader>::from_untyped_unchecked(
                Hash::prehashed([marker; Hash::LENGTH]),
            ));
        }
        block_hashes.commit_for_tests();

        assert_eq!(
            state.commit_topology_snapshot(),
            active_roster,
            "startup topology must match the active TLE certificate roster in this fixture",
        );

        assert_eq!(
            retained_key_session_id,
            TleKeySessionId::new([RETAINED_SESSION_BYTE; 32]),
            "the public Parliament fixture binds hidden ballots to this historical session"
        );
        ThresholdSignerReadinessFixture {
            state,
            local_peer,
            retained_key_session_id,
            active_key_session_id,
            retained_participant_index,
            active_participant_index,
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
    fn local_threshold_committee_seats_fail_closed_without_runtime_signers() {
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

        assert_eq!(
            require_parliament_tle_signer_for_local_seat_v1(true, false),
            Err("local Parliament TLE committee seat has no resolved partial-release signer")
        );
        assert_eq!(
            require_parliament_tle_signer_for_local_seat_v1(true, true),
            Ok(())
        );
        assert_eq!(
            require_parliament_tle_signer_for_local_seat_v1(false, false),
            Ok(())
        );
    }

    #[test]
    fn active_parliament_tle_session_must_match_the_exact_startup_context() {
        let network_id = [0x11; 32];
        let roster_hash = [0x22; 32];
        assert_eq!(
            require_parliament_tle_session_startup_binding_v1(
                network_id,
                roster_hash,
                4,
                network_id,
                roster_hash,
                4,
            ),
            Ok(())
        );
        assert_eq!(
            require_parliament_tle_session_startup_binding_v1(
                network_id,
                roster_hash,
                4,
                [0x33; 32],
                roster_hash,
                4,
            ),
            Err("active Parliament TLE key session belongs to another network")
        );
        assert_eq!(
            require_parliament_tle_session_startup_binding_v1(
                network_id,
                roster_hash,
                4,
                network_id,
                [0x44; 32],
                4,
            ),
            Err("active Parliament TLE key session is not bound to the startup roster")
        );
        assert_eq!(
            require_parliament_tle_session_startup_binding_v1(
                network_id,
                roster_hash,
                4,
                network_id,
                roster_hash,
                7,
            ),
            Err("active Parliament TLE key session is not bound to the startup roster")
        );
    }

    #[cfg(unix)]
    #[test]
    fn parliament_tle_startup_requires_exact_non_signing_custody_attestation() {
        let fixture =
            crate::external_software_signer::consensus_threshold_tle_broker_test_fixture_v1();
        let session = fixture.session;

        let exact = CapabilityProbeSigner::new(CapabilityMode::Exact);
        assert_eq!(
            require_parliament_tle_capability_for_local_seat_v1(Some(1), Some(&exact), &session,),
            Ok(())
        );
        assert_eq!(exact.attestation_calls().len(), 1);
        assert_eq!(exact.sign_calls.load(Ordering::Acquire), 0);

        let mismatched = CapabilityProbeSigner::new(CapabilityMode::MismatchedSeat);
        assert_eq!(
            require_parliament_tle_capability_for_local_seat_v1(
                Some(1),
                Some(&mismatched),
                &session,
            ),
            Err(
                "local Parliament TLE committee seat returned a mismatched runtime custody attestation"
            )
        );
        assert_eq!(mismatched.sign_calls.load(Ordering::Acquire), 0);

        let rejected = CapabilityProbeSigner::new(CapabilityMode::Rejected);
        assert_eq!(
            require_parliament_tle_capability_for_local_seat_v1(Some(1), Some(&rejected), &session,),
            Err("local Parliament TLE committee seat has no exact runtime custody attestation")
        );
        assert_eq!(rejected.sign_calls.load(Ordering::Acquire), 0);

        assert_eq!(
            require_parliament_tle_capability_for_local_seat_v1(
                Some(1),
                Some(&UnavailableSigner),
                &session,
            ),
            Err("local Parliament TLE committee seat has no exact runtime custody attestation")
        );
        assert_eq!(
            require_parliament_tle_capability_for_local_seat_v1(None, None, &session),
            Ok(())
        );
    }

    #[test]
    fn frozen_parliament_tle_roster_derives_one_exact_local_seat() {
        fn peer(tag: u8) -> PeerId {
            let key = iroha_crypto::KeyPair::try_from_seed(
                vec![tag; 32],
                iroha_crypto::Algorithm::Ed25519,
            )
            .expect("derive deterministic peer key");
            PeerId::new(key.public_key().clone())
        }

        let first = peer(0x41);
        let second = peer(0x42);
        assert_eq!(
            parliament_tle_local_participant_index_v1(&[first.clone(), second.clone()], &second),
            Ok(Some(2))
        );
        assert_eq!(
            parliament_tle_local_participant_index_v1(&[first.clone()], &second),
            Ok(None)
        );
        assert_eq!(
            parliament_tle_local_participant_index_v1(&[first.clone(), first], &second),
            Err("Parliament TLE key-session roster contains a duplicate peer")
        );
    }

    #[test]
    fn threshold_signer_startup_readiness_scans_active_and_deadline_retained_frozen_rosters() {
        let fixture = threshold_signer_readiness_fixture_v1(13);
        let signer = Arc::new(CapabilityProbeSigner::new(CapabilityMode::Exact));
        let runtime_deps =
            IrohaRuntimeDeps::default().with_parliament_tle_partial_release_signer(signer.clone());

        validate_threshold_signer_startup_readiness_v1(
            &fixture.state,
            &fixture.local_peer,
            &runtime_deps,
        )
        .expect("active and deadline-retained frozen seats have exact runtime custody");

        let mut calls = signer.attestation_calls();
        calls.sort_unstable();
        let mut expected = vec![
            (
                fixture.retained_key_session_id,
                fixture.retained_participant_index,
            ),
            (
                fixture.active_key_session_id,
                fixture.active_participant_index,
            ),
        ];
        expected.sort_unstable();
        assert_eq!(calls, expected);
        assert_eq!(signer.sign_calls.load(Ordering::Acquire), 0);
    }

    #[test]
    fn threshold_signer_startup_readiness_skips_expired_history_and_rejects_mismatch() {
        let fixture = threshold_signer_readiness_fixture_v1(14);
        let exact_signer = Arc::new(CapabilityProbeSigner::new(CapabilityMode::Exact));
        let exact_runtime_deps = IrohaRuntimeDeps::default()
            .with_parliament_tle_partial_release_signer(exact_signer.clone());

        validate_threshold_signer_startup_readiness_v1(
            &fixture.state,
            &fixture.local_peer,
            &exact_runtime_deps,
        )
        .expect("expired historical custody is skipped while the active seat remains ready");
        assert_eq!(
            exact_signer.attestation_calls(),
            vec![(
                fixture.active_key_session_id,
                fixture.active_participant_index,
            )]
        );
        assert_eq!(exact_signer.sign_calls.load(Ordering::Acquire), 0);

        let mismatched_signer =
            Arc::new(CapabilityProbeSigner::new(CapabilityMode::MismatchedSeat));
        let mismatched_runtime_deps = IrohaRuntimeDeps::default()
            .with_parliament_tle_partial_release_signer(mismatched_signer.clone());
        assert_eq!(
            validate_threshold_signer_startup_readiness_v1(
                &fixture.state,
                &fixture.local_peer,
                &mismatched_runtime_deps,
            ),
            Err(
                "local Parliament TLE committee seat returned a mismatched runtime custody attestation"
            )
        );
        assert_eq!(
            mismatched_signer.attestation_calls(),
            vec![(
                fixture.active_key_session_id,
                fixture.active_participant_index,
            )]
        );
        assert_eq!(mismatched_signer.sign_calls.load(Ordering::Acquire), 0);
    }

    #[test]
    fn threshold_signer_preflight_rejects_before_consensus_startup() {
        let startup = include_str!("../main.rs")
            .split_once("let sumeragi = if emergency_fast")
            .expect("consensus startup branch")
            .1
            .split_once("let tx_gossiper = if emergency_fast")
            .expect("consensus startup boundary")
            .0;
        let preflight = startup
            .find("validate_threshold_signer_startup_readiness_v1")
            .expect("threshold-signer startup preflight");
        let consensus_start = startup.find("SumeragiStartArgs").expect("Sumeragi startup");
        let guarded_preflight: String = startup[preflight..consensus_start]
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();

        assert!(preflight < consensus_start);
        assert!(
            guarded_preflight
                .contains(".map_err(|message|Report::new(StartError::StartP2p).attach(message))?;")
        );
        assert!(!startup.contains("ParliamentTleShareUnavailable"));
        assert!(!startup.contains("local Parliament TLE committee seat is not operational"));
    }
}
