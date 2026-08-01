//! Exact scope checks for deployment-resolved runtime dependencies.
//!
//! The registry accepts only the dependency roles requested by the sanitized
//! binding catalog. Shared provider objects remain valid only when every role
//! in their fixed V1 pair was requested.

use super::*;

/// Return whether the resolved dependency catalog contains the requested role.
pub(super) fn dependency_is_present(
    dependencies: &IrohaRuntimeDeps,
    slot: IrohaRuntimeProviderSlotV1,
) -> bool {
    use IrohaRuntimeProviderSlotV1 as Slot;

    let deps = dependencies;
    match slot {
        Slot::ModerationQuarantineKeyWrapper => deps.moderation_quarantine_key_wrapper.is_some(),
        Slot::PrivacyCyclePrfProvider => deps.privacy_cycle_prf_provider.is_some(),
        Slot::PrivacyReleaseAnchor => deps.privacy_release_anchor.is_some(),
        Slot::TransparencyLeaderLease => deps.transparency_leader_lease_provider.is_some(),
        Slot::FencedPrivacyPublisher => deps.sorafs_fenced_transparency_publisher.is_some(),
        Slot::FencedPrivacyHeadReader => deps.sorafs_fenced_transparency_head_reader.is_some(),
        Slot::GovernanceDagSigner => deps.sorafs_governance_dag_signer.is_some(),
        Slot::GovernanceDagIpfsAuthenticator => {
            deps.sorafs_governance_dag_ipfs_authenticator.is_some()
        }
        Slot::GovernanceDagHeadAuthenticator => {
            deps.sorafs_governance_dag_head_authenticator.is_some()
        }
        Slot::GovernanceDagCheckpointStore => deps.sorafs_governance_dag_checkpoint_store.is_some(),
        Slot::StreamTokenSigner => deps.sorafs_stream_token_signer.is_some(),
        Slot::StreamTokenGatewayAdmission => deps.sorafs_stream_token_gateway_admission.is_some(),
        Slot::AppealFinanceTransactionSigner => {
            deps.sorafs_appeal_finance_runtime_signers.is_some()
        }
        Slot::AppealFinanceCheckpoint => deps.sorafs_appeal_finance_checkpoint_runtime.is_some(),
        Slot::ProofOutcomeTransactionSigner => deps.sorafs_proof_outcome_signer.is_some(),
        Slot::RepairTransactionSigner => deps.sorafs_repair_transaction_signer.is_some(),
        Slot::ReserveTransactionSigner => deps.sorafs_reserve_transaction_signer.is_some(),
        Slot::OrderbookTransactionSigner => deps.sorafs_orderbook_transaction_signer.is_some(),
        Slot::ModerationTransactionSigner => deps.sorafs_moderation_transaction_signer.is_some(),
        Slot::ModerationSettlementHandoff => deps.sorafs_moderation_settlement_handoff.is_some(),
        Slot::ModerationPublicationHandoff => deps.sorafs_moderation_publication_handoff.is_some(),
        Slot::ModerationPanelNotification => deps.sorafs_moderation_panel_notification.is_some(),
        Slot::ModerationPanelNotificationArchive => {
            deps.sorafs_moderation_panel_notification_archive.is_some()
        }
        Slot::ModerationCheckpointStore => deps.sorafs_moderation_checkpoint_store.is_some(),
        Slot::EvidenceViewerWebAuthn => deps.sorafs_evidence_viewer_webauthn.is_some(),
        Slot::EvidenceViewerGrantAuthority => deps.sorafs_evidence_viewer_grants.is_some(),
        Slot::EvidenceViewerReceiptSigner => deps.sorafs_evidence_viewer_receipt_signer.is_some(),
        Slot::EvidenceViewerErasure => deps.sorafs_evidence_viewer_erasure.is_some(),
        Slot::EvidenceViewerCheckpointStore => {
            deps.sorafs_evidence_viewer_checkpoint_store.is_some()
        }
        Slot::PopCredentialProviderRegistry => {
            deps.sorafs_pop_credential_provider_registry.is_some()
        }
        Slot::PotrGatewaySigner | Slot::PotrProviderSigner => {
            deps.sorafs_potr_runtime_signer_roles.is_some()
        }
        Slot::GatewayAcmeClient => deps.sorafs_gateway_acme_client.is_some(),
        Slot::GatewayComplianceFeedTransport => {
            deps.sorafs_gateway_compliance_feed_transport.is_some()
        }
        Slot::ReputationJournalTransactionSubmitter => deps
            .sorafs_reputation_journal_transaction_submitter
            .is_some(),
        Slot::ReputationJournalCheckpoint => {
            deps.sorafs_reputation_journal_checkpoint_provider.is_some()
        }
        Slot::ReputationThresholdSigner => deps.sorafs_reputation_threshold_signer.is_some(),
        Slot::ReputationGovernanceDag => deps.sorafs_reputation_governance_dag.is_some(),
        Slot::ReputationFinalizedArchiveRetentionAuthority => {
            deps.sorafs_reputation_retention_authority.is_some()
        }
        Slot::BillingFinalizedQuery => deps.sorafs_hedging_billing_finalized_query.is_some(),
        Slot::BillingJournalVerifier => deps.sorafs_hedging_billing_journal_verifier.is_some(),
        Slot::BillingStatementSigner => deps.sorafs_billing_statement_signer.is_some(),
        Slot::BillingStatementPublisher => deps.sorafs_billing_statement_publisher.is_some(),
        Slot::BillingAcknowledgementAuthority => {
            deps.sorafs_billing_acknowledgement_authority.is_some()
        }
        Slot::BillingEpochWitnessStore => deps.sorafs_hedging_billing_epoch_witness_store.is_some(),
        Slot::ProviderIngestAuthenticatedSource => {
            deps.sorafs_provider_ingest_authenticated_source.is_some()
        }
        Slot::ProviderIngestCompletionSignerResolver | Slot::ProviderIngestCompletionSigner => {
            deps.sorafs_provider_ingest_signer_resolver.is_some()
        }
        Slot::ProviderIngestCheckpointStore => {
            deps.sorafs_provider_ingest_checkpoint_runtime.is_some()
        }
        Slot::ProviderIngestRetentionAuthority => {
            deps.sorafs_provider_ingest_retention_authority.is_some()
        }
        Slot::PorFinalizedReplayArchive => deps.sorafs_por_finalized_replay_archive.is_some(),
        Slot::EvidenceViewerCompactionArchive => {
            deps.sorafs_evidence_viewer_compaction_archive.is_some()
        }
        Slot::EvidenceViewerTransparencyPublisher => {
            deps.sorafs_evidence_viewer_transparency_publisher.is_some()
        }
        Slot::SoracloudRuntimeMutationSigner => deps.soracloud_runtime_mutation_signer.is_some(),
        Slot::SoracloudHfInferenceCredentialProvider => {
            deps.soracloud_hf_inference_credential_provider.is_some()
        }
    }
}

/// Reject any resolved dependency that was not requested by exact binding.
pub(super) fn has_unrequested_dependency(
    bindings: &IrohaRuntimeProviderBindingsV1,
    dependencies: &IrohaRuntimeDeps,
) -> bool {
    has_unrequested_storage_security_dependency(bindings, dependencies)
        || has_unrequested_finance_native_dependency(bindings, dependencies)
        || has_unrequested_moderation_viewer_dependency(bindings, dependencies)
        || has_unrequested_pop_potr_gateway_dependency(bindings, dependencies)
        || has_unrequested_reputation_billing_dependency(bindings, dependencies)
        || has_unrequested_provider_ingest_dependency(bindings, dependencies)
        || dependency_is_unrequested(
            bindings,
            IrohaRuntimeProviderSlotV1::SoracloudRuntimeMutationSigner,
            dependencies.soracloud_runtime_mutation_signer.is_some(),
        )
        || dependency_is_unrequested(
            bindings,
            IrohaRuntimeProviderSlotV1::SoracloudHfInferenceCredentialProvider,
            dependencies
                .soracloud_hf_inference_credential_provider
                .is_some(),
        )
}

fn dependency_is_unrequested(
    bindings: &IrohaRuntimeProviderBindingsV1,
    slot: IrohaRuntimeProviderSlotV1,
    is_present: bool,
) -> bool {
    is_present && !bindings.iter().any(|binding| binding.slot() == slot)
}

fn paired_dependency_is_unrequested(
    bindings: &IrohaRuntimeProviderBindingsV1,
    first_slot: IrohaRuntimeProviderSlotV1,
    second_slot: IrohaRuntimeProviderSlotV1,
    is_present: bool,
) -> bool {
    is_present
        && ![first_slot, second_slot]
            .into_iter()
            .all(|slot| bindings.iter().any(|binding| binding.slot() == slot))
}

fn has_unrequested_storage_security_dependency(
    bindings: &IrohaRuntimeProviderBindingsV1,
    dependencies: &IrohaRuntimeDeps,
) -> bool {
    use IrohaRuntimeProviderSlotV1 as Slot;

    dependency_is_unrequested(
        bindings,
        Slot::ModerationQuarantineKeyWrapper,
        dependencies.moderation_quarantine_key_wrapper.is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::PrivacyCyclePrfProvider,
        dependencies.privacy_cycle_prf_provider.is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::PrivacyReleaseAnchor,
        dependencies.privacy_release_anchor.is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::TransparencyLeaderLease,
        dependencies.transparency_leader_lease_provider.is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::FencedPrivacyPublisher,
        dependencies.sorafs_fenced_transparency_publisher.is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::FencedPrivacyHeadReader,
        dependencies
            .sorafs_fenced_transparency_head_reader
            .is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::GovernanceDagSigner,
        dependencies.sorafs_governance_dag_signer.is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::GovernanceDagIpfsAuthenticator,
        dependencies
            .sorafs_governance_dag_ipfs_authenticator
            .is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::GovernanceDagHeadAuthenticator,
        dependencies
            .sorafs_governance_dag_head_authenticator
            .is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::GovernanceDagCheckpointStore,
        dependencies
            .sorafs_governance_dag_checkpoint_store
            .is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::StreamTokenSigner,
        dependencies.sorafs_stream_token_signer.is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::StreamTokenGatewayAdmission,
        dependencies.sorafs_stream_token_gateway_admission.is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::PorFinalizedReplayArchive,
        dependencies.sorafs_por_finalized_replay_archive.is_some(),
    )
}

fn has_unrequested_finance_native_dependency(
    bindings: &IrohaRuntimeProviderBindingsV1,
    dependencies: &IrohaRuntimeDeps,
) -> bool {
    use IrohaRuntimeProviderSlotV1 as Slot;

    dependency_is_unrequested(
        bindings,
        Slot::AppealFinanceTransactionSigner,
        dependencies.sorafs_appeal_finance_runtime_signers.is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::AppealFinanceCheckpoint,
        dependencies
            .sorafs_appeal_finance_checkpoint_runtime
            .is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::ProofOutcomeTransactionSigner,
        dependencies.sorafs_proof_outcome_signer.is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::RepairTransactionSigner,
        dependencies.sorafs_repair_transaction_signer.is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::ReserveTransactionSigner,
        dependencies.sorafs_reserve_transaction_signer.is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::OrderbookTransactionSigner,
        dependencies.sorafs_orderbook_transaction_signer.is_some(),
    )
}

fn has_unrequested_moderation_viewer_dependency(
    bindings: &IrohaRuntimeProviderBindingsV1,
    dependencies: &IrohaRuntimeDeps,
) -> bool {
    use IrohaRuntimeProviderSlotV1 as Slot;

    dependency_is_unrequested(
        bindings,
        Slot::ModerationTransactionSigner,
        dependencies.sorafs_moderation_transaction_signer.is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::ModerationSettlementHandoff,
        dependencies.sorafs_moderation_settlement_handoff.is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::ModerationPublicationHandoff,
        dependencies.sorafs_moderation_publication_handoff.is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::ModerationPanelNotification,
        dependencies.sorafs_moderation_panel_notification.is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::ModerationPanelNotificationArchive,
        dependencies
            .sorafs_moderation_panel_notification_archive
            .is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::ModerationCheckpointStore,
        dependencies.sorafs_moderation_checkpoint_store.is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::EvidenceViewerWebAuthn,
        dependencies.sorafs_evidence_viewer_webauthn.is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::EvidenceViewerGrantAuthority,
        dependencies.sorafs_evidence_viewer_grants.is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::EvidenceViewerReceiptSigner,
        dependencies.sorafs_evidence_viewer_receipt_signer.is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::EvidenceViewerErasure,
        dependencies.sorafs_evidence_viewer_erasure.is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::EvidenceViewerCheckpointStore,
        dependencies
            .sorafs_evidence_viewer_checkpoint_store
            .is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::EvidenceViewerCompactionArchive,
        dependencies
            .sorafs_evidence_viewer_compaction_archive
            .is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::EvidenceViewerTransparencyPublisher,
        dependencies
            .sorafs_evidence_viewer_transparency_publisher
            .is_some(),
    )
}

fn has_unrequested_pop_potr_gateway_dependency(
    bindings: &IrohaRuntimeProviderBindingsV1,
    dependencies: &IrohaRuntimeDeps,
) -> bool {
    use IrohaRuntimeProviderSlotV1 as Slot;

    dependency_is_unrequested(
        bindings,
        Slot::PopCredentialProviderRegistry,
        dependencies
            .sorafs_pop_credential_provider_registry
            .is_some(),
    ) || paired_dependency_is_unrequested(
        bindings,
        Slot::PotrGatewaySigner,
        Slot::PotrProviderSigner,
        dependencies.sorafs_potr_runtime_signer_roles.is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::GatewayAcmeClient,
        dependencies.sorafs_gateway_acme_client.is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::GatewayComplianceFeedTransport,
        dependencies
            .sorafs_gateway_compliance_feed_transport
            .is_some(),
    )
}

fn has_unrequested_reputation_billing_dependency(
    bindings: &IrohaRuntimeProviderBindingsV1,
    dependencies: &IrohaRuntimeDeps,
) -> bool {
    use IrohaRuntimeProviderSlotV1 as Slot;

    dependency_is_unrequested(
        bindings,
        Slot::ReputationJournalCheckpoint,
        dependencies
            .sorafs_reputation_journal_checkpoint_provider
            .is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::ReputationJournalTransactionSubmitter,
        dependencies
            .sorafs_reputation_journal_transaction_submitter
            .is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::ReputationThresholdSigner,
        dependencies.sorafs_reputation_threshold_signer.is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::ReputationGovernanceDag,
        dependencies.sorafs_reputation_governance_dag.is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::ReputationFinalizedArchiveRetentionAuthority,
        dependencies.sorafs_reputation_retention_authority.is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::BillingFinalizedQuery,
        dependencies
            .sorafs_hedging_billing_finalized_query
            .is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::BillingJournalVerifier,
        dependencies
            .sorafs_hedging_billing_journal_verifier
            .is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::BillingStatementSigner,
        dependencies.sorafs_billing_statement_signer.is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::BillingStatementPublisher,
        dependencies.sorafs_billing_statement_publisher.is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::BillingAcknowledgementAuthority,
        dependencies
            .sorafs_billing_acknowledgement_authority
            .is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::BillingEpochWitnessStore,
        dependencies
            .sorafs_hedging_billing_epoch_witness_store
            .is_some(),
    )
}

fn has_unrequested_provider_ingest_dependency(
    bindings: &IrohaRuntimeProviderBindingsV1,
    dependencies: &IrohaRuntimeDeps,
) -> bool {
    use IrohaRuntimeProviderSlotV1 as Slot;

    dependency_is_unrequested(
        bindings,
        Slot::ProviderIngestAuthenticatedSource,
        dependencies
            .sorafs_provider_ingest_authenticated_source
            .is_some(),
    ) || paired_dependency_is_unrequested(
        bindings,
        Slot::ProviderIngestCompletionSignerResolver,
        Slot::ProviderIngestCompletionSigner,
        dependencies
            .sorafs_provider_ingest_signer_resolver
            .is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::ProviderIngestCheckpointStore,
        dependencies
            .sorafs_provider_ingest_checkpoint_runtime
            .is_some(),
    ) || dependency_is_unrequested(
        bindings,
        Slot::ProviderIngestRetentionAuthority,
        dependencies
            .sorafs_provider_ingest_retention_authority
            .is_some(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bindings_for(slots: &[IrohaRuntimeProviderSlotV1]) -> IrohaRuntimeProviderBindingsV1 {
        let bindings = slots
            .iter()
            .copied()
            .enumerate()
            .map(|(index, slot)| {
                IrohaRuntimeProviderBindingV1::try_new(
                    slot,
                    format!("hsm://sorafs/runtime-scope-{index}"),
                    Some(1),
                    Some([u8::try_from(index + 1).expect("small fixture index"); 32]),
                )
                .expect("scope fixture binding must be production-shaped")
            })
            .collect();
        IrohaRuntimeProviderBindingsV1 {
            chain_id: "runtime-scope-chain".to_owned(),
            bindings,
        }
    }

    #[test]
    fn single_dependency_scope_requires_the_exact_requested_slot() {
        use IrohaRuntimeProviderSlotV1 as Slot;

        let bindings = bindings_for(&[Slot::GatewayAcmeClient]);
        assert!(!dependency_is_unrequested(
            &bindings,
            Slot::GatewayAcmeClient,
            true,
        ));
        assert!(dependency_is_unrequested(
            &bindings,
            Slot::GatewayComplianceFeedTransport,
            true,
        ));
        assert!(!dependency_is_unrequested(
            &bindings,
            Slot::GatewayComplianceFeedTransport,
            false,
        ));
    }

    #[test]
    fn shared_dependency_scope_requires_the_complete_role_pair() {
        use IrohaRuntimeProviderSlotV1 as Slot;

        let gateway_only = bindings_for(&[Slot::PotrGatewaySigner]);
        assert!(paired_dependency_is_unrequested(
            &gateway_only,
            Slot::PotrGatewaySigner,
            Slot::PotrProviderSigner,
            true,
        ));

        let complete = bindings_for(&[Slot::PotrGatewaySigner, Slot::PotrProviderSigner]);
        assert!(!paired_dependency_is_unrequested(
            &complete,
            Slot::PotrGatewaySigner,
            Slot::PotrProviderSigner,
            true,
        ));
        assert!(!paired_dependency_is_unrequested(
            &gateway_only,
            Slot::PotrGatewaySigner,
            Slot::PotrProviderSigner,
            false,
        ));
    }
}
