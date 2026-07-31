//! Projection of stable, public runtime-provider bindings from node configuration.
//!
//! The collectors in this module deliberately receive only the validated
//! configuration and the payload-free binding catalog. They never resolve
//! credentials, private keys, tokens, or any other runtime-only secret.

use super::*;

/// Append every configured provider binding in the canonical projection order.
pub(super) fn collect_configured_bindings(
    config: &Config,
    bindings: &mut Vec<IrohaRuntimeProviderBindingV1>,
) -> Result<(), IrohaRuntimeProviderRegistryErrorV1> {
    collect_storage_security_bindings(config, bindings)?;
    collect_appeal_finance_bindings(config, bindings)?;
    collect_native_transaction_signer_bindings(config, bindings)?;
    collect_moderation_viewer_bindings(config, bindings)?;
    collect_pop_potr_gateway_bindings(config, bindings)?;
    collect_reputation_billing_bindings(config, bindings)?;
    collect_provider_ingest_bindings(config, bindings)?;
    collect_soracloud_runtime_signer_binding(config, bindings)?;
    collect_soracloud_hf_credential_provider_binding(config, bindings)
}

fn collect_soracloud_runtime_signer_binding(
    config: &Config,
    bindings: &mut Vec<IrohaRuntimeProviderBindingV1>,
) -> Result<(), IrohaRuntimeProviderRegistryErrorV1> {
    let slot = IrohaRuntimeProviderSlotV1::SoracloudRuntimeMutationSigner;
    match config.soracloud_runtime.submission.signer.as_ref() {
        Some(binding) => {
            bindings.push(IrohaRuntimeProviderBindingV1::try_new_soracloud_runtime_signer(binding)?)
        }
        None if config.soracloud_runtime.production_mode => {
            return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot));
        }
        None => {}
    }
    Ok(())
}

fn collect_soracloud_hf_credential_provider_binding(
    config: &Config,
    bindings: &mut Vec<IrohaRuntimeProviderBindingV1>,
) -> Result<(), IrohaRuntimeProviderRegistryErrorV1> {
    let slot = IrohaRuntimeProviderSlotV1::SoracloudHfInferenceCredentialProvider;
    match config
        .soracloud_runtime
        .hf
        .inference_credential_provider
        .as_ref()
    {
        Some(binding) => bindings.push(
            IrohaRuntimeProviderBindingV1::try_new_soracloud_hf_credential_provider(binding)?,
        ),
        None if config.soracloud_runtime.hf.allow_inference_bridge_fallback => {
            return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot));
        }
        None => {}
    }
    Ok(())
}

fn append_binding(
    bindings: &mut Vec<IrohaRuntimeProviderBindingV1>,
    slot: IrohaRuntimeProviderSlotV1,
    handle: &str,
    revision: Option<u64>,
    policy_digest: Option<[u8; 32]>,
) -> Result<(), IrohaRuntimeProviderRegistryErrorV1> {
    bindings.push(IrohaRuntimeProviderBindingV1::try_new(
        slot,
        handle,
        revision,
        policy_digest,
    )?);
    Ok(())
}

fn collect_storage_security_bindings(
    config: &Config,
    bindings: &mut Vec<IrohaRuntimeProviderBindingV1>,
) -> Result<(), IrohaRuntimeProviderRegistryErrorV1> {
    let storage = &config.torii.sorafs_storage;
    if let Some(binding) = storage.moderation_quarantine_key_provider.as_ref() {
        append_binding(
            bindings,
            IrohaRuntimeProviderSlotV1::ModerationQuarantineKeyWrapper,
            &binding.handle,
            Some(binding.revision),
            Some(binding.policy_digest),
        )?;
    }
    if let Some(archive) = storage.por_replay_archive.as_ref() {
        bindings.push(IrohaRuntimeProviderBindingV1::try_new_por_replay_archive(
            archive,
        )?);
    }
    if let Some(binding) = storage.privacy_aggregates.cycle_prf_provider.as_ref() {
        append_binding(
            bindings,
            IrohaRuntimeProviderSlotV1::PrivacyCyclePrfProvider,
            &binding.handle,
            Some(binding.revision),
            Some(binding.policy_digest),
        )?;
    }
    if let Some(binding) = storage.privacy_aggregates.release_anchor_provider.as_ref() {
        append_binding(
            bindings,
            IrohaRuntimeProviderSlotV1::PrivacyReleaseAnchor,
            &binding.handle,
            Some(binding.revision),
            Some(binding.policy_digest),
        )?;
    }
    if let Some(binding) = storage.privacy_aggregates.leader_lease_provider.as_ref() {
        append_binding(
            bindings,
            IrohaRuntimeProviderSlotV1::TransparencyLeaderLease,
            &binding.handle,
            Some(binding.revision),
            Some(binding.policy_digest),
        )?;
    }
    if let Some(binding) = storage.privacy_aggregates.fenced_privacy_publisher.as_ref() {
        for slot in [
            IrohaRuntimeProviderSlotV1::FencedPrivacyPublisher,
            IrohaRuntimeProviderSlotV1::FencedPrivacyHeadReader,
        ] {
            append_binding(
                bindings,
                slot,
                &binding.handle,
                Some(binding.revision),
                Some(binding.policy_digest),
            )?;
        }
    }
    // Repeat the parser's all-or-nothing qualification check because callers
    // can manually construct or mutate the public actual configuration.
    match (
        storage.governance_dag_publisher_peer_id.as_deref(),
        storage.governance_dag_signer_handle.as_deref(),
        storage.governance_dag_signer_revision,
        storage.governance_dag_signer_policy_digest,
        storage.governance_dag_publisher_public_key_hex.as_deref(),
    ) {
        (None, None, None, None, None) if storage.governance_dag_dir.is_none() => {}
        (
            Some(publisher_peer_id),
            Some(handle),
            Some(revision),
            Some(policy_digest),
            Some(publisher_public_key_hex),
        ) if storage.enabled && storage.governance_dag_dir.is_some() => {
            bindings.push(
                IrohaRuntimeProviderBindingV1::try_new_governance_dag_signer(
                    handle,
                    revision,
                    policy_digest,
                    publisher_peer_id,
                    publisher_public_key_hex,
                )?,
            );
        }
        _ => {
            return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                IrohaRuntimeProviderSlotV1::GovernanceDagSigner,
            ));
        }
    }
    let governance_service = &storage.governance_dag_service;
    if governance_service.enabled {
        append_required_governance_request_auth_binding(
            bindings,
            IrohaRuntimeProviderSlotV1::GovernanceDagIpfsAuthenticator,
            governance_service.ipfs_authenticator_handle.as_deref(),
            governance_service.ipfs_authenticator_revision,
            governance_service.ipfs_authenticator_policy_digest,
            governance_service.ipfs_request_auth_public_key,
            governance_service.max_request_bytes.0,
        )?;
        if governance_service.head_mode == "signed_http" {
            append_required_governance_request_auth_binding(
                bindings,
                IrohaRuntimeProviderSlotV1::GovernanceDagHeadAuthenticator,
                governance_service.head_authenticator_handle.as_deref(),
                governance_service.head_authenticator_revision,
                governance_service.head_authenticator_policy_digest,
                governance_service.head_request_auth_public_key,
                governance_service.max_request_bytes.0,
            )?;
        } else if governance_service.head_mode != "ipns"
            || governance_service.head_authenticator_handle.is_some()
            || governance_service.head_authenticator_revision.is_some()
            || governance_service
                .head_authenticator_policy_digest
                .is_some()
            || governance_service.head_request_auth_public_key.is_some()
        {
            return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                IrohaRuntimeProviderSlotV1::GovernanceDagHeadAuthenticator,
            ));
        }
    } else {
        if governance_service.ipfs_authenticator_handle.is_some()
            || governance_service.ipfs_authenticator_revision.is_some()
            || governance_service
                .ipfs_authenticator_policy_digest
                .is_some()
            || governance_service.ipfs_request_auth_public_key.is_some()
        {
            return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                IrohaRuntimeProviderSlotV1::GovernanceDagIpfsAuthenticator,
            ));
        }
        if governance_service.head_authenticator_handle.is_some()
            || governance_service.head_authenticator_revision.is_some()
            || governance_service
                .head_authenticator_policy_digest
                .is_some()
            || governance_service.head_request_auth_public_key.is_some()
        {
            return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                IrohaRuntimeProviderSlotV1::GovernanceDagHeadAuthenticator,
            ));
        }
    }
    let checkpoint_store_configured = governance_service.checkpoint_store_handle.is_some()
        || governance_service.checkpoint_store_revision.is_some()
        || governance_service.checkpoint_store_policy_digest.is_some();
    if governance_service.enabled || storage.governance_dag_dir.is_some() {
        append_required_governance_service_binding(
            bindings,
            IrohaRuntimeProviderSlotV1::GovernanceDagCheckpointStore,
            governance_service.checkpoint_store_handle.as_deref(),
            governance_service.checkpoint_store_revision,
            governance_service.checkpoint_store_policy_digest,
        )?;
    } else if checkpoint_store_configured {
        return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
            IrohaRuntimeProviderSlotV1::GovernanceDagCheckpointStore,
        ));
    }
    match (
        storage.stream_tokens.signer_handle.as_deref(),
        storage.stream_tokens.signer_public_key,
    ) {
        (Some(handle), Some(public_key)) => bindings.push(
            IrohaRuntimeProviderBindingV1::try_new_stream_token_signer(handle, public_key)?,
        ),
        (None, None) => {}
        _ => {
            return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                IrohaRuntimeProviderSlotV1::StreamTokenSigner,
            ));
        }
    }
    Ok(())
}

/// Append one required authenticated Governance DAG request binding.
pub(super) fn append_required_governance_request_auth_binding(
    bindings: &mut Vec<IrohaRuntimeProviderBindingV1>,
    slot: IrohaRuntimeProviderSlotV1,
    handle: Option<&str>,
    revision: Option<u64>,
    policy_digest: Option<[u8; 32]>,
    public_key: Option<[u8; 32]>,
    max_body_bytes: u64,
) -> Result<(), IrohaRuntimeProviderRegistryErrorV1> {
    let (Some(handle), Some(revision), Some(policy_digest), Some(public_key)) =
        (handle, revision, policy_digest, public_key)
    else {
        return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot));
    };
    bindings.push(
        IrohaRuntimeProviderBindingV1::try_new_governance_request_auth(
            slot,
            handle,
            revision,
            policy_digest,
            public_key,
            max_body_bytes,
        )?,
    );
    Ok(())
}

/// Append one required qualified Governance DAG service binding.
pub(super) fn append_required_governance_service_binding(
    bindings: &mut Vec<IrohaRuntimeProviderBindingV1>,
    slot: IrohaRuntimeProviderSlotV1,
    handle: Option<&str>,
    revision: Option<u64>,
    policy_digest: Option<[u8; 32]>,
) -> Result<(), IrohaRuntimeProviderRegistryErrorV1> {
    let (Some(handle), Some(revision), Some(policy_digest)) = (handle, revision, policy_digest)
    else {
        return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot));
    };
    append_binding(bindings, slot, handle, Some(revision), Some(policy_digest))
}

fn collect_appeal_finance_bindings(
    config: &Config,
    bindings: &mut Vec<IrohaRuntimeProviderBindingV1>,
) -> Result<(), IrohaRuntimeProviderRegistryErrorV1> {
    let appeal_finance = &config.torii.sorafs_appeal_finance_settlement;
    for binding in &appeal_finance.submitter_signers {
        bindings.push(IrohaRuntimeProviderBindingV1::try_new_appeal_finance_signer(binding)?);
    }
    if let Some(binding) = appeal_finance.checkpoint_provider.as_ref() {
        bindings.push(
            IrohaRuntimeProviderBindingV1::try_new_appeal_finance_checkpoint(
                binding,
                appeal_finance.worker_checkpoint_max_bytes,
            )?,
        );
    }
    Ok(())
}

fn collect_native_transaction_signer_bindings(
    config: &Config,
    bindings: &mut Vec<IrohaRuntimeProviderBindingV1>,
) -> Result<(), IrohaRuntimeProviderRegistryErrorV1> {
    use iroha_torii::SorafsNativeTransactionSignerRoleV1 as Role;

    let configured = &config.torii.sorafs_storage.native_transaction_signers;
    for (slot, role, binding) in [
        (
            IrohaRuntimeProviderSlotV1::ProofOutcomeTransactionSigner,
            Role::ProofOutcome,
            configured.proof_outcome.as_ref(),
        ),
        (
            IrohaRuntimeProviderSlotV1::RepairTransactionSigner,
            Role::Repair,
            configured.repair.as_ref(),
        ),
        (
            IrohaRuntimeProviderSlotV1::ReserveTransactionSigner,
            Role::Reserve,
            configured.reserve.as_ref(),
        ),
        (
            IrohaRuntimeProviderSlotV1::OrderbookTransactionSigner,
            Role::Orderbook,
            configured.orderbook.as_ref(),
        ),
    ] {
        let Some(binding) = binding else {
            continue;
        };
        if !matches!(
            binding.public_key.try_algorithm(),
            Ok(algorithm) if algorithm == binding.algorithm
        ) {
            return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot));
        }
        let qualification = iroha_torii::SorafsNativeTransactionSignerQualificationV1::new(
            binding.revision,
            binding.policy_digest,
        );
        let exact = iroha_torii::SorafsNativeTransactionSignerBindingV1::try_new(
            role,
            binding.handle.clone(),
            binding.authority.clone(),
            binding.public_key.clone(),
            qualification,
        )
        .map_err(|_| IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(slot))?;
        bindings.push(IrohaRuntimeProviderBindingV1::try_new_native_signer(
            slot, exact,
        )?);
    }
    Ok(())
}

fn collect_moderation_viewer_bindings(
    config: &Config,
    bindings: &mut Vec<IrohaRuntimeProviderBindingV1>,
) -> Result<(), IrohaRuntimeProviderRegistryErrorV1> {
    let storage = &config.torii.sorafs_storage;
    if let Some(runtime) = storage.moderation_orchestrator.as_ref() {
        validate_moderation_strict_ingress_binding(runtime)?;
        bindings.push(IrohaRuntimeProviderBindingV1::try_new_moderation_checkpoint_store(runtime)?);
        for (slot, handle, revision, policy_digest) in [
            (
                IrohaRuntimeProviderSlotV1::ModerationTransactionSigner,
                runtime.transaction_signer_handle.as_str(),
                runtime.transaction_signer_revision,
                runtime.transaction_signer_policy_digest,
            ),
            (
                IrohaRuntimeProviderSlotV1::ModerationSettlementHandoff,
                runtime.settlement_handoff_handle.as_str(),
                runtime.settlement_handoff_revision,
                runtime.settlement_handoff_policy_digest,
            ),
            (
                IrohaRuntimeProviderSlotV1::ModerationPublicationHandoff,
                runtime.publication_handoff_handle.as_str(),
                runtime.publication_handoff_revision,
                runtime.publication_handoff_policy_digest,
            ),
            (
                IrohaRuntimeProviderSlotV1::ModerationPanelNotification,
                runtime.panel_notification_handle.as_str(),
                runtime.panel_notification_revision,
                runtime.panel_notification_policy_digest,
            ),
        ] {
            append_binding(bindings, slot, handle, Some(revision), Some(policy_digest))?;
        }
    }
    if let Some(viewer) = storage.evidence_viewer.as_ref() {
        bindings.push(IrohaRuntimeProviderBindingV1::try_new_evidence_viewer_webauthn(viewer)?);
        bindings.push(IrohaRuntimeProviderBindingV1::try_new_evidence_viewer_grants(viewer)?);
        bindings
            .push(IrohaRuntimeProviderBindingV1::try_new_evidence_viewer_receipt_signer(viewer)?);
        append_binding(
            bindings,
            IrohaRuntimeProviderSlotV1::EvidenceViewerErasure,
            viewer.erasure_handle.as_str(),
            Some(viewer.erasure_revision),
            Some(viewer.erasure_policy_digest),
        )?;
        bindings
            .push(IrohaRuntimeProviderBindingV1::try_new_evidence_viewer_checkpoint_store(viewer)?);
        bindings.push(IrohaRuntimeProviderBindingV1::try_new_evidence_viewer_archive(viewer)?);
        bindings.push(
            IrohaRuntimeProviderBindingV1::try_new_evidence_viewer_transparency_publisher(viewer)?,
        );
    }
    Ok(())
}

fn validate_moderation_strict_ingress_binding(
    runtime: &iroha_config::parameters::actual::SorafsModerationOrchestrator,
) -> Result<(), IrohaRuntimeProviderRegistryErrorV1> {
    use sorafs_node::moderation_orchestrator::{
        ModerationRuntimeProviderQualificationErrorV1 as Error,
        ModerationRuntimeProviderQualificationV1,
    };

    let configured_qualification = ModerationRuntimeProviderQualificationV1::new(
        runtime.strict_ingress_revision,
        runtime.strict_ingress_policy_digest,
    );
    iroha_torii::sorafs::moderation_runtime::qualify_torii_moderation_strict_ingress_binding_v1(
        &runtime.strict_ingress_handle,
        configured_qualification,
    )
    .map_err(|error| match error {
        Error::TestMarkedConfiguredHandle | Error::TestMarkedProviderHandle => {
            IrohaRuntimeProviderRegistryErrorV1::TestProviderRejected
        }
        Error::InvalidConfiguredHandle
        | Error::InvalidProviderHandle
        | Error::SubstitutedProvider => IrohaRuntimeProviderRegistryErrorV1::BindingMismatch,
        Error::InvalidConfiguredQualification
        | Error::UnavailableOrStale
        | Error::InvalidQualification
        | Error::QualificationMismatch
        | Error::IdentityOrPolicyChanged => IrohaRuntimeProviderRegistryErrorV1::StaleOrRevoked,
    })
}

fn collect_pop_potr_gateway_bindings(
    config: &Config,
    bindings: &mut Vec<IrohaRuntimeProviderBindingV1>,
) -> Result<(), IrohaRuntimeProviderRegistryErrorV1> {
    if let Some(pop) = config.torii.sorafs_storage.pop_credentials.as_ref() {
        bindings.push(IrohaRuntimeProviderBindingV1::try_new_pop_credential_registry(pop)?);
    }
    if let Some(potr) = config.torii.sorafs_por.potr_runtime.as_ref() {
        for slot in [
            IrohaRuntimeProviderSlotV1::PotrGatewaySigner,
            IrohaRuntimeProviderSlotV1::PotrProviderSigner,
        ] {
            bindings.push(IrohaRuntimeProviderBindingV1::try_new_potr_signer(
                slot, potr,
            )?);
        }
    }
    if let Some(binding) = config.torii.sorafs_gateway.acme.provider.as_ref() {
        append_binding(
            bindings,
            IrohaRuntimeProviderSlotV1::GatewayAcmeClient,
            &binding.provider_handle,
            Some(binding.revision),
            Some(binding.policy_digest),
        )?;
    }
    if let Some(compliance) = config.torii.sorafs_gateway.compliance.as_ref() {
        let binding = &compliance.feed_transport_provider;
        append_binding(
            bindings,
            IrohaRuntimeProviderSlotV1::GatewayComplianceFeedTransport,
            &binding.provider_handle,
            Some(binding.revision),
            Some(binding.policy_digest),
        )?;
    }
    Ok(())
}

fn collect_reputation_billing_bindings(
    config: &Config,
    bindings: &mut Vec<IrohaRuntimeProviderBindingV1>,
) -> Result<(), IrohaRuntimeProviderRegistryErrorV1> {
    let storage = &config.torii.sorafs_storage;
    if let Some(reputation) = storage.reputation_runtime.as_ref() {
        let checkpoint_slot = IrohaRuntimeProviderSlotV1::ReputationJournalCheckpoint;
        sorafs_node::reputation::runtime::ReputationJournalCheckpointSealingPolicyV1::try_new(
            reputation.journal_checkpoint_provider_handle.clone(),
            reputation.journal_checkpoint_provider_revision,
            reputation.journal_checkpoint_provider_policy_digest,
        )
        .map_err(|_| IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(checkpoint_slot))?;
        for (slot, handle, revision, policy_digest) in [
            (
                checkpoint_slot,
                reputation.journal_checkpoint_provider_handle.as_str(),
                reputation.journal_checkpoint_provider_revision,
                reputation.journal_checkpoint_provider_policy_digest,
            ),
            (
                IrohaRuntimeProviderSlotV1::ReputationJournalTransactionSubmitter,
                reputation.journal_transaction_submitter_handle.as_str(),
                reputation.journal_transaction_submitter_revision,
                reputation.journal_transaction_submitter_policy_digest,
            ),
            (
                IrohaRuntimeProviderSlotV1::ReputationThresholdSigner,
                reputation.threshold_signer_handle.as_str(),
                reputation.threshold_signer_revision,
                reputation.threshold_signer_policy_digest,
            ),
            (
                IrohaRuntimeProviderSlotV1::ReputationGovernanceDag,
                reputation.governance_dag_handle.as_str(),
                reputation.governance_dag_revision,
                reputation.governance_dag_policy_digest,
            ),
        ] {
            append_binding(bindings, slot, handle, Some(revision), Some(policy_digest))?;
        }
        if let Some(retention) = reputation.finalized_archive_retention_authority.as_ref() {
            append_binding(
                bindings,
                IrohaRuntimeProviderSlotV1::ReputationFinalizedArchiveRetentionAuthority,
                &retention.handle,
                Some(retention.revision),
                Some(retention.policy_digest),
            )?;
        }
    }
    if let Some(billing) = storage.hedging_billing_runtime.as_ref() {
        for (slot, handle, revision, policy_digest) in [
            (
                IrohaRuntimeProviderSlotV1::BillingFinalizedQuery,
                billing.finalized_query_handle.as_str(),
                billing.finalized_query_revision,
                billing.finalized_query_policy_digest,
            ),
            (
                IrohaRuntimeProviderSlotV1::BillingJournalVerifier,
                billing.journal_verifier_handle.as_str(),
                billing.journal_verifier_revision,
                billing.journal_verifier_policy_digest,
            ),
            (
                IrohaRuntimeProviderSlotV1::BillingStatementSigner,
                billing.statement_signer_handle.as_str(),
                billing.statement_signer_revision,
                billing.statement_signer_policy_digest,
            ),
            (
                IrohaRuntimeProviderSlotV1::BillingStatementPublisher,
                billing.statement_publisher_handle.as_str(),
                billing.statement_publisher_revision,
                billing.statement_publisher_policy_digest,
            ),
            (
                IrohaRuntimeProviderSlotV1::BillingAcknowledgementAuthority,
                billing.acknowledgement_authority_handle.as_str(),
                billing.acknowledgement_authority_revision,
                billing.acknowledgement_authority_policy_digest,
            ),
            (
                IrohaRuntimeProviderSlotV1::BillingEpochWitnessStore,
                billing.epoch_witness_store_handle.as_str(),
                billing.epoch_witness_store_revision,
                billing.epoch_witness_store_policy_digest,
            ),
        ] {
            append_binding(bindings, slot, handle, Some(revision), Some(policy_digest))?;
        }
    }
    Ok(())
}

fn collect_provider_ingest_bindings(
    config: &Config,
    bindings: &mut Vec<IrohaRuntimeProviderBindingV1>,
) -> Result<(), IrohaRuntimeProviderRegistryErrorV1> {
    let Some(ingest) = config.torii.sorafs_storage.provider_ingest_runtime.as_ref() else {
        return Ok(());
    };
    if ingest.outbox.max_signed_transaction_bytes.0
        < provider_ingest_outbox_defaults::MAX_SIGNED_TRANSACTION_BYTES_MIN
        || ingest.outbox.max_signed_transaction_bytes.0
            > provider_ingest_outbox_defaults::MAX_SIGNED_TRANSACTION_BYTES_LIMIT
    {
        return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
            IrohaRuntimeProviderSlotV1::ProviderIngestCompletionSignerResolver,
        ));
    }
    if ingest.outbox.checkpoint_max_bytes.0 == 0
        || ingest.outbox.checkpoint_max_bytes.0
            > provider_ingest_outbox_defaults::CHECKPOINT_MAX_BYTES_LIMIT
    {
        return Err(IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
            IrohaRuntimeProviderSlotV1::ProviderIngestCheckpointStore,
        ));
    }
    sorafs_node::ProviderIngestOutboxPolicyV1 {
        max_active_entries: ingest.outbox.max_active_entries,
        max_terminal_entries: ingest.outbox.max_terminal_entries,
        max_attempts: ingest.outbox.max_attempts,
        checkpoint_max_bytes: ingest.outbox.checkpoint_max_bytes.0,
        checkpoint_operation_timeout_ms: ingest.outbox.checkpoint_operation_timeout_ms,
        source_lease_ttl_ms: ingest.outbox.source_lease_ttl_ms,
        retry_base_delay_ms: ingest.outbox.retry_base_delay_ms,
        retry_max_delay_ms: ingest.outbox.retry_max_delay_ms,
        terminal_retention_blocks: ingest.outbox.terminal_retention_blocks,
        max_signed_transaction_bytes: ingest.outbox.max_signed_transaction_bytes.0,
        max_status_page_size: ingest.outbox.max_status_page_size,
    }
    .validate()
    .map_err(|_| {
        IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
            IrohaRuntimeProviderSlotV1::ProviderIngestCheckpointStore,
        )
    })?;
    let max_source_providers = u32::try_from(ingest.max_source_providers).map_err(|_| {
        IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
            IrohaRuntimeProviderSlotV1::ProviderIngestAuthenticatedSource,
        )
    })?;
    let max_concurrent_streams = u32::try_from(config.torii.sorafs_storage.max_parallel_fetches)
        .map_err(|_| {
            IrohaRuntimeProviderRegistryErrorV1::InvalidBinding(
                IrohaRuntimeProviderSlotV1::ProviderIngestAuthenticatedSource,
            )
        })?;
    bindings.push(
        IrohaRuntimeProviderBindingV1::try_new_provider_ingest_source(
            ingest.authenticated_source_fetch_handle.clone(),
            ingest.authenticated_source_fetch_revision,
            ingest.authenticated_source_fetch_policy_digest,
            ProviderIngestSourceLimitsV1 {
                operation_timeout_ms: ingest.source_operation_timeout_ms,
                max_content_bytes: config.torii.sorafs_storage.max_capacity_bytes.0,
                max_source_providers,
                max_concurrent_streams,
            },
        )?,
    );
    let signer_binding = sorafs_node::ProviderIngestCompletionSignerBindingV1::new(
        ingest.completion_signer_handle.clone(),
        sorafs_node::ProviderIngestCompletionSignerQualificationV1::new(
            ingest.completion_signer_adapter_revision,
            ingest.completion_signer_policy,
            ingest.completion_signer_algorithm,
            ingest.completion_signer_public_key.clone(),
        ),
    );
    bindings.push(
        IrohaRuntimeProviderBindingV1::try_new_provider_ingest_signer(
            IrohaRuntimeProviderSlotV1::ProviderIngestCompletionSignerResolver,
            ingest.completion_signer_resolver_handle.clone(),
            ingest.completion_signer_resolver_revision,
            ingest.completion_signer_resolver_policy_digest,
            signer_binding.clone(),
            ingest.outbox.max_signed_transaction_bytes.0,
        )?,
    );
    bindings.push(
        IrohaRuntimeProviderBindingV1::try_new_provider_ingest_signer(
            IrohaRuntimeProviderSlotV1::ProviderIngestCompletionSigner,
            ingest.completion_signer_handle.clone(),
            ingest.completion_signer_adapter_revision,
            ingest.completion_signer_policy.policy_digest,
            signer_binding,
            ingest.outbox.max_signed_transaction_bytes.0,
        )?,
    );
    bindings.push(
        IrohaRuntimeProviderBindingV1::try_new_provider_ingest_checkpoint(
            ingest.checkpoint_store_handle.clone(),
            ingest.checkpoint_store_revision,
            ingest.checkpoint_store_policy_digest,
            ingest.outbox.checkpoint_max_bytes.0,
        )?,
    );
    if let Some(retention) = ingest.finalized_archive.retention_authority.as_ref() {
        append_binding(
            bindings,
            IrohaRuntimeProviderSlotV1::ProviderIngestRetentionAuthority,
            &retention.handle,
            Some(retention.revision),
            Some(retention.policy_digest),
        )?;
    }
    Ok(())
}
