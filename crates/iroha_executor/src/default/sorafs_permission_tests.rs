#[cfg(test)]
mod sorafs_permission_tests {
    use super::*;
    use crate::{Iroha, prelude, tests::with_mock_permissions};
    use core::num::NonZeroU64;
    use iroha_crypto::PublicKey;
    use iroha_data_model::{
        account::AccountId,
        block::BlockHeader,
        isi::sorafs::{
            AcceptSorafsModerationJurorAssignment, ActivateSorafsModerationCase,
            AppendSorafsPorReputationJournalEntry, AppendSorafsStreamTokenReputationJournalEntry,
            ApprovePinManifest, BindManifestAlias, CommitSorafsPopCredentialBatch,
            CompleteReplicationOrder, ExpireReplicationOrder, ExpireSorafsModerationChallenge,
            FinalizeSorafsModerationCase, FinalizeSorafsModerationSortition, IssueReplicationOrder,
            PublishSorafsPopRevocationList, RaiseSorafsModerationChallenge,
            RecordCapacityTelemetry, RegisterCapacityDeclaration, RegisterCapacityDispute,
            RegisterPinManifest, RegisterProviderOwner, RegisterSorafsModerationJurorEligibility,
            ResolveSorafsCapacityDispute, ResolveSorafsModerationChallenge, RetirePinManifest,
            ReviseReplicationOrderAssignments, RevokeProviderIngestCompletionAuthority,
            SetPricingSchedule, SetProviderIngestCompletionAuthority, SetSorafsModerationPolicy,
            SetSorafsPopIssuerPolicy, SetSorafsReputationJournalAuthorityPolicy,
            SubmitSorafsModerationAppeal, SubmitSorafsModerationCommit,
            SubmitSorafsModerationReveal, UnregisterProviderOwner, UpsertProviderCredit,
        },
        metadata::Metadata,
        permission::Permission as PermissionObject,
        prelude::{Quantity, ValidationFail},
        query::sorafs::prelude::{
            FindSorafsModerationAppeal, FindSorafsModerationEvents,
            FindSorafsModerationJurorEligibility, FindSorafsModerationPolicy,
            FindSorafsModerationSnapshot, FindSorafsModerationStatus,
            FindSorafsOrderbookCancellationByOrderId, FindSorafsOrderbookChannelById,
            FindSorafsOrderbookChannels, FindSorafsOrderbookEvents, FindSorafsOrderbookOrderById,
            FindSorafsOrderbookOrders, FindSorafsOrderbookPolicy, FindSorafsOrderbookReceiptById,
            FindSorafsOrderbookReceipts, FindSorafsOrderbookStatus, FindSorafsOrderbookTradeById,
            FindSorafsOrderbookTrades, FindSorafsPopAuditDigestBySequence,
            FindSorafsPopCommitmentRootByVersion, FindSorafsPopCredentialCommitmentByDigest,
            FindSorafsPopIssuerPolicy, FindSorafsPopRegistryStatus,
            FindSorafsPopRevocationByNonceCommitment, FindSorafsPopRevocationPublicationByVersion,
            FindSorafsReputationJournalAuthorityPolicy, FindSorafsReputationJournalEventBySourceId,
            FindSorafsReputationJournalEvents, FindSorafsReserveEvents,
        },
        sorafs::{
            capacity::{
                CapacityDeclarationRecord, CapacityDisputeEvidence, CapacityDisputeId,
                CapacityDisputeOutcome, CapacityDisputeRecord, CapacityTelemetryRecord, ProviderId,
            },
            moderation_ledger::{
                MODERATION_APPEAL_INTAKE_VERSION_V1, MODERATION_LEDGER_POLICY_VERSION_V1,
                ModerationAppealIntakeV1, ModerationChallengeDecisionV1, ModerationChallengeKindV1,
                ModerationFinalizedCursorV1, ModerationLedgerPolicyV1,
            },
            pin_registry::{
                ManifestAliasBinding, ManifestDigest, ProviderIngestCompletionAuthorityV1,
                ProviderIngestCompletionSignerPolicyV1, ProviderIngestFinalizedAnchorV1,
                ReplicationOrderId,
            },
            pop_registry::{POP_ISSUER_POLICY_VERSION_V1, PopIssuerPolicyV1},
            pricing::{PricingScheduleRecord, ProviderCreditRecord},
            reputation::{
                PorTerminalOutcomeV1, PorTerminalStatusV1,
                REPUTATION_JOURNAL_AUTHORITY_POLICY_VERSION_V1, ReputationJournalAuthorityPolicyV1,
                ReputationJournalEntryV1, ReputationJournalFinalizedCursorV1,
                ReputationJournalPayloadV1, ReputationJournalSourceIdV1,
                StreamTokenValidationBindingV1, StreamTokenValidationOutcomeV1,
                StreamTokenValidationStatusV1,
            },
            reserve::ReserveFinalizedCursorV1,
        },
    };
    use iroha_executor_data_model::permission::sorafs::{
        CanBindSorafsAlias, CanCompleteSorafsReplicationOrder, CanFileSorafsCapacityDispute,
        CanIssueSorafsReplicationOrder, CanManageSorafsModeration, CanManageSorafsPopRegistry,
        CanManageSorafsReputationJournalPolicy, CanOperateSorafsPopIssuer,
        CanRecordSorafsReputationJournal, CanResolveSorafsCapacityDispute, CanSetSorafsPricing,
        CanSetSorafsReservePolicy, CanUpsertSorafsProviderCredit,
    };
    use iroha_executor_data_model::permission::{
        domain::CanRegisterDomain,
        parameter::{CanSetHijiriParameters, CanSetParameters},
        sccp::CanManageSccpGovernance,
    };
    const AUTHORITY_PUBLIC_KEY: &str =
        "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245";
    const OWNER_PUBLIC_KEY: &str =
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03";
    fn account_id_from_public_key_hex(hex_literal: &str) -> AccountId {
        let public_key: PublicKey = hex_literal
            .parse()
            .expect("test public key literal should parse");
        AccountId::new(public_key)
    }
    fn authority_account_id() -> AccountId {
        account_id_from_public_key_hex(AUTHORITY_PUBLIC_KEY)
    }
    fn owner_account_id() -> AccountId {
        account_id_from_public_key_hex(OWNER_PUBLIC_KEY)
    }
    fn authority_public_key_bytes() -> [u8; 32] {
        let authority = authority_account_id();
        let (_, bytes) = authority
            .expect_single_signatory()
            .try_to_bytes()
            .expect("authority public key bytes");
        bytes.try_into().expect("Ed25519 public key length")
    }
    #[derive(Debug, iroha_executor_derive::Visit)]
    struct MockExecutor {
        host: Iroha,
        ctx: prelude::Context,
        verdict: crate::data_model::executor::Result<(), ValidationFail>,
    }
    impl MockExecutor {
        fn new(genesis: bool) -> Self {
            let height = if genesis { 1 } else { 2 };
            let header = BlockHeader::new(
                NonZeroU64::new(height).expect("nonzero height"),
                None,
                None,
                None,
                0,
                0,
            );
            let authority = authority_account_id();
            Self {
                host: Iroha,
                ctx: prelude::Context {
                    authority,
                    curr_block: header,
                },
                verdict: Ok(()),
            }
        }
    }
    impl Execute for MockExecutor {
        fn host(&self) -> &Iroha {
            &self.host
        }
        fn context(&self) -> &prelude::Context {
            &self.ctx
        }
        fn context_mut(&mut self) -> &mut prelude::Context {
            &mut self.ctx
        }
        fn verdict(&self) -> &crate::data_model::executor::Result<(), ValidationFail> {
            &self.verdict
        }
        fn deny(&mut self, reason: ValidationFail) {
            self.verdict = Err(reason);
        }
    }
    fn assert_denied_without_permission<T: Clone>(
        instruction: T,
        visit: impl Fn(&mut MockExecutor, &T),
    ) {
        with_mock_permissions(Vec::new(), || {
            let mut executor = MockExecutor::new(false);
            visit(&mut executor, &instruction);
            assert!(
                executor.verdict().is_err(),
                "expected denial without permission"
            );
        });
    }
    fn assert_allowed_without_permission<T: Clone>(
        instruction: T,
        visit: impl Fn(&mut MockExecutor, &T),
    ) {
        let mut executor = MockExecutor::new(false);
        visit(&mut executor, &instruction);
        assert!(
            executor.verdict().is_ok(),
            "expected instruction to be permitted without permission"
        );
    }
    fn assert_allowed_with_permission<T: Clone>(
        instruction: T,
        permission: PermissionObject,
        visit: impl Fn(&mut MockExecutor, &T),
    ) {
        with_mock_permissions(vec![permission], || {
            let mut executor = MockExecutor::new(false);
            visit(&mut executor, &instruction);
            assert!(
                executor.verdict().is_ok(),
                "expected instruction to be permitted with permission"
            );
        });
    }
    fn assert_denied_with_permission<T: Clone>(
        instruction: T,
        permission: PermissionObject,
        visit: impl Fn(&mut MockExecutor, &T),
    ) {
        with_mock_permissions(vec![permission], || {
            let mut executor = MockExecutor::new(false);
            visit(&mut executor, &instruction);
            assert!(
                executor.verdict().is_err(),
                "expected instruction to remain denied with unrelated permission"
            );
        });
    }
    fn sample_provider_id() -> ProviderId {
        ProviderId::new([0xAB; 32])
    }
    fn sample_manifest_digest() -> ManifestDigest {
        ManifestDigest::new([0xCD; 32])
    }
    fn register_pin_manifest() -> RegisterPinManifest {
        RegisterPinManifest::new(
            include_bytes!("../../../../fixtures/sorafs_gateway/1.0.0/manifest_v1.to").to_vec(),
            None,
            None,
        )
    }
    fn approve_pin_manifest() -> ApprovePinManifest {
        ApprovePinManifest::new(sample_manifest_digest(), None, None)
    }
    fn retire_pin_manifest() -> RetirePinManifest {
        RetirePinManifest::new(sample_manifest_digest(), None)
    }
    fn bind_manifest_alias() -> BindManifestAlias {
        BindManifestAlias::new(
            sample_manifest_digest(),
            ManifestAliasBinding {
                name: "docs".to_owned(),
                namespace: "sora".to_owned(),
                proof: Vec::new(),
            },
            4,
            5,
        )
    }
    fn set_pop_issuer_policy() -> SetSorafsPopIssuerPolicy {
        SetSorafsPopIssuerPolicy::new(PopIssuerPolicyV1 {
            version: POP_ISSUER_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            issuer_id: "pop-issuer-sora-foundation".to_owned(),
            issuer_account: authority_account_id(),
            issuer_public_key: authority_public_key_bytes(),
            max_credentials_per_batch: 16,
            max_revocations_per_publication: 16,
            max_credential_lifetime_secs: 86_400,
            max_future_clock_skew_secs: 30,
            paused: false,
        })
    }
    fn register_capacity_declaration() -> RegisterCapacityDeclaration {
        RegisterCapacityDeclaration::new(CapacityDeclarationRecord::new(
            sample_provider_id(),
            vec![0xAA],
            100,
            1,
            1,
            2,
            Metadata::default(),
        ))
    }
    fn record_capacity_telemetry() -> RecordCapacityTelemetry {
        RecordCapacityTelemetry::new(
            CapacityTelemetryRecord::new(
                sample_provider_id(),
                1,
                2,
                100,
                90,
                80,
                1,
                1,
                10_000,
                10_000,
                0,
                0,
                0,
                0,
                0,
            )
            .with_nonce(0),
        )
    }
    fn register_capacity_dispute() -> RegisterCapacityDispute {
        RegisterCapacityDispute::new(CapacityDisputeRecord::new_pending(
            CapacityDisputeId::new([0x01; 32]),
            sample_provider_id(),
            [0x02; 32],
            None,
            0,
            1,
            "desc".to_owned(),
            None,
            CapacityDisputeEvidence {
                digest: [0x03; 32],
                media_type: None,
                uri: None,
                size_bytes: None,
            },
            vec![0x04],
        ))
    }
    fn reputation_policy() -> ReputationJournalAuthorityPolicyV1 {
        let authority = authority_account_id();
        ReputationJournalAuthorityPolicyV1 {
            version: REPUTATION_JOURNAL_AUTHORITY_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            por_recorder_authority: authority.clone(),
            dispute_recorder_authority: authority.clone(),
            token_recorder_authority: authority,
            max_source_age_ms: 24 * 60 * 60 * 1_000,
        }
    }
    fn por_reputation_entry() -> ReputationJournalEntryV1 {
        let policy = reputation_policy();
        ReputationJournalEntryV1::try_new(
            sample_provider_id(),
            policy.canonical_digest().expect("reputation policy digest"),
            authority_account_id(),
            1_700_000_000_000,
            None,
            ReputationJournalPayloadV1::PorTerminal(PorTerminalOutcomeV1 {
                challenge_id: [0x31; 32],
                manifest_digest: [0x32; 32],
                epoch_id: 1,
                drand_round: 2,
                forced: false,
                sample_count: 4,
                failed_samples: 0,
                issued_at_unix_ms: 1_699_999_998_000,
                deadline_at_unix_ms: 1_700_000_000_000,
                responded_at_unix_ms: Some(1_699_999_999_000),
                decided_at_unix_ms: 1_700_000_000_000,
                proof_digest: Some([0x33; 32]),
                repair_task_id: None,
                verifier_latency_ms: Some(7),
                status: PorTerminalStatusV1::Verified,
            }),
        )
        .expect("canonical PoR reputation fixture")
    }
    fn token_reputation_entry() -> ReputationJournalEntryV1 {
        let policy = reputation_policy();
        ReputationJournalEntryV1::try_new(
            sample_provider_id(),
            policy.canonical_digest().expect("reputation policy digest"),
            authority_account_id(),
            1_700_000_000_000,
            None,
            ReputationJournalPayloadV1::StreamTokenValidation(StreamTokenValidationOutcomeV1 {
                binding: StreamTokenValidationBindingV1 {
                    gateway_id: [0x41; 32],
                    gateway_sequence: 1,
                    request_context_digest: [0x42; 32],
                },
                token_body_digest: Some([0x43; 32]),
                token_key_version: Some(1),
                validated_at_unix_ms: 1_700_000_000_000,
                status: StreamTokenValidationStatusV1::Accepted,
            }),
        )
        .expect("canonical reputation fixture")
    }
    fn resolve_capacity_dispute() -> ResolveSorafsCapacityDispute {
        ResolveSorafsCapacityDispute::new(
            CapacityDisputeId::new([0x01; 32]),
            reputation_policy()
                .canonical_digest()
                .expect("reputation policy digest"),
            CapacityDisputeOutcome::Upheld,
            [0x44; 32],
            Some("upheld".to_owned()),
        )
    }
    fn issue_replication_order() -> IssueReplicationOrder {
        IssueReplicationOrder::new(ReplicationOrderId::new([0x11; 32]), vec![0x22], 1, 2)
    }
    fn provider_ingest_completion_authority() -> ProviderIngestCompletionAuthorityV1 {
        ProviderIngestCompletionAuthorityV1::new(
            owner_account_id(),
            ProviderIngestCompletionSignerPolicyV1 {
                policy_id: [0x13; 32],
                revision: 1,
                predecessor_digest: None,
                policy_digest: [0x14; 32],
            },
        )
    }
    fn complete_replication_order() -> CompleteReplicationOrder {
        CompleteReplicationOrder::new(
            ReplicationOrderId::new([0x11; 32]),
            ProviderId::new([0x12; 32]),
            3,
            provider_ingest_completion_authority(),
            1,
            ProviderIngestFinalizedAnchorV1 {
                height: 2,
                block_hash: [0x15; 32],
            },
        )
    }
    fn revise_replication_order_assignments() -> ReviseReplicationOrderAssignments {
        ReviseReplicationOrderAssignments::new(
            ReplicationOrderId::new([0x11; 32]),
            1,
            2,
            Vec::new(),
        )
    }
    fn set_provider_ingest_completion_authority() -> SetProviderIngestCompletionAuthority {
        SetProviderIngestCompletionAuthority::new(
            ProviderId::new([0x12; 32]),
            None,
            provider_ingest_completion_authority(),
        )
    }
    fn revoke_provider_ingest_completion_authority() -> RevokeProviderIngestCompletionAuthority {
        RevokeProviderIngestCompletionAuthority::new(
            ProviderId::new([0x12; 32]),
            provider_ingest_completion_authority(),
        )
    }
    fn expire_replication_order() -> ExpireReplicationOrder {
        ExpireReplicationOrder::new(ReplicationOrderId::new([0x11; 32]), 4)
    }
    fn set_pricing_schedule() -> SetPricingSchedule {
        SetPricingSchedule::new(PricingScheduleRecord::launch_default())
    }
    fn upsert_provider_credit() -> UpsertProviderCredit {
        UpsertProviderCredit::new(ProviderCreditRecord::new(
            sample_provider_id(),
            Quantity::from(1_u32),
            Quantity::zero(),
            Quantity::zero(),
            Quantity::zero(),
            0,
            0,
            Metadata::default(),
        ))
    }
    fn set_moderation_policy() -> SetSorafsModerationPolicy {
        SetSorafsModerationPolicy::new(ModerationLedgerPolicyV1 {
            version: MODERATION_LEDGER_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            challenge_voting_asset_id:
                iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                    iroha_data_model::domain::DomainId::try_new("sora", "universal")
                        .expect("governance domain"),
                    "xor".parse().expect("governance asset name"),
                ),
            challenge_bond_amount: Quantity::from(
                iroha_data_model::sorafs::moderation_ledger::MODERATION_CHALLENGE_BOND_AMOUNT_V1,
            ),
            challenge_escrow_account: authority_account_id(),
            challenge_slash_receiver_account: authority_account_id(),
            challenge_rejected_slash_bps:
                iroha_data_model::sorafs::moderation_ledger::MODERATION_CHALLENGE_REJECTED_SLASH_BPS_V1,
            challenge_resolution_grace_ms:
                iroha_data_model::sorafs::moderation_ledger::MODERATION_CHALLENGE_RESOLUTION_GRACE_MS_V1,
            max_panel_size: 8,
            max_candidate_pool_size: 32,
            max_waitlist_size: 8,
            max_exclusions_per_case: 16,
            max_total_window_ms: 90_000_000,
            max_challenges_per_case: 4,
            missing_commit_penalty_points: 10,
            unrevealed_commit_penalty_points: 20,
        })
    }
    fn moderation_appeal_intake() -> ModerationAppealIntakeV1 {
        let appellant = authority_account_id();
        ModerationAppealIntakeV1 {
            version: MODERATION_APPEAL_INTAKE_VERSION_V1,
            case_id: "appeal-case".to_owned(),
            round_id: "round-1".to_owned(),
            appellant: appellant.clone(),
            appealed_decision_digest: [0x11; 32],
            proof_token_digest: [0x12; 32],
            evidence_bundle_digest: [0x13; 32],
            appeal_deposit_lock_digest: [0x14; 32],
            appeal_finance_config_version: "finance-v1".to_owned(),
            policy_reference: "moderation-v1".to_owned(),
            evidence_uri: None,
            panel_size: 3,
            waitlist_size: 2,
            quorum: 2,
            exclusions: vec![appellant],
            registration_deadline_unix_ms: 1_000,
            acceptance_deadline_unix_ms: 2_000,
            commit_deadline_unix_ms: 3_000,
            challenge_submission_deadline_unix_ms: 4_000,
            challenge_resolution_deadline_unix_ms: 86_404_000,
            reveal_deadline_unix_ms: 86_405_000,
            policy_digest: [0x15; 32],
        }
    }
    fn register_provider_owner() -> RegisterProviderOwner {
        RegisterProviderOwner::new(sample_provider_id(), owner_account_id())
    }
    fn unregister_provider_owner() -> UnregisterProviderOwner {
        UnregisterProviderOwner::new(sample_provider_id())
    }
    macro_rules! sorafs_permission_case {
        ($name:ident, $instruction:expr, $permission:expr, $visitor:path) => {
            #[test]
            fn $name() {
                let instruction = $instruction;
                assert_denied_without_permission(instruction.clone(), $visitor);
                assert_denied_with_permission(
                    instruction.clone(),
                    PermissionObject::from(CanRegisterDomain),
                    $visitor,
                );
                assert_allowed_with_permission(
                    instruction,
                    PermissionObject::from($permission),
                    $visitor,
                );
            }
        };
    }
    #[test]
    fn register_pin_manifest_is_public() {
        assert_allowed_without_permission(
            register_pin_manifest(),
            sorafs::visit_register_pin_manifest,
        );
    }
    #[test]
    fn approve_pin_manifest_relays_governed_envelopes_without_permission() {
        assert_allowed_without_permission(
            approve_pin_manifest(),
            sorafs::visit_approve_pin_manifest,
        );
    }
    #[test]
    fn retire_pin_manifest_defers_exact_owner_check_to_core() {
        assert_allowed_without_permission(retire_pin_manifest(), sorafs::visit_retire_pin_manifest);
    }
    sorafs_permission_case!(
        bind_manifest_alias_requires_permission,
        bind_manifest_alias(),
        CanBindSorafsAlias,
        sorafs::visit_bind_manifest_alias
    );
    #[test]
    fn register_capacity_declaration_is_public() {
        assert_allowed_without_permission(
            register_capacity_declaration(),
            sorafs::visit_register_capacity_declaration,
        );
    }
    #[test]
    fn record_capacity_telemetry_is_public() {
        assert_allowed_without_permission(
            record_capacity_telemetry(),
            sorafs::visit_record_capacity_telemetry,
        );
    }
    sorafs_permission_case!(
        record_capacity_dispute_requires_permission,
        register_capacity_dispute(),
        CanFileSorafsCapacityDispute,
        sorafs::visit_register_capacity_dispute
    );
    sorafs_permission_case!(
        resolve_capacity_dispute_requires_permission,
        resolve_capacity_dispute(),
        CanResolveSorafsCapacityDispute,
        sorafs::visit_resolve_capacity_dispute
    );
    sorafs_permission_case!(
        set_reputation_policy_requires_permission,
        SetSorafsReputationJournalAuthorityPolicy::new(reputation_policy()),
        CanManageSorafsReputationJournalPolicy,
        sorafs::visit_set_reputation_journal_authority_policy
    );
    sorafs_permission_case!(
        append_por_reputation_requires_permission,
        AppendSorafsPorReputationJournalEntry::new(por_reputation_entry()),
        CanRecordSorafsReputationJournal,
        sorafs::visit_append_por_reputation_journal_entry
    );
    sorafs_permission_case!(
        append_stream_token_reputation_requires_permission,
        AppendSorafsStreamTokenReputationJournalEntry::new(token_reputation_entry()),
        CanRecordSorafsReputationJournal,
        sorafs::visit_append_stream_token_reputation_journal_entry
    );
    sorafs_permission_case!(
        issue_replication_order_requires_permission,
        issue_replication_order(),
        CanIssueSorafsReplicationOrder,
        sorafs::visit_issue_replication_order
    );
    sorafs_permission_case!(
        complete_replication_order_requires_permission,
        complete_replication_order(),
        CanCompleteSorafsReplicationOrder,
        sorafs::visit_complete_replication_order
    );
    sorafs_permission_case!(
        revise_replication_order_assignments_requires_permission,
        revise_replication_order_assignments(),
        CanIssueSorafsReplicationOrder,
        sorafs::visit_revise_replication_order_assignments
    );
    sorafs_permission_case!(
        expire_replication_order_requires_permission,
        expire_replication_order(),
        CanIssueSorafsReplicationOrder,
        sorafs::visit_expire_replication_order
    );
    #[test]
    fn retired_direct_provider_owner_instructions_reach_core_for_uniform_rejection() {
        assert_allowed_without_permission(
            register_provider_owner(),
            sorafs::visit_register_provider_owner,
        );
        assert_allowed_without_permission(
            unregister_provider_owner(),
            sorafs::visit_unregister_provider_owner,
        );
    }
    #[test]
    fn completion_authority_instructions_reach_core_owner_check() {
        assert_allowed_without_permission(
            set_provider_ingest_completion_authority(),
            sorafs::visit_set_provider_ingest_completion_authority,
        );
        assert_allowed_without_permission(
            revoke_provider_ingest_completion_authority(),
            sorafs::visit_revoke_provider_ingest_completion_authority,
        );
    }
    sorafs_permission_case!(
        set_pricing_schedule_requires_permission,
        set_pricing_schedule(),
        CanSetSorafsPricing,
        sorafs::visit_set_pricing_schedule
    );
    sorafs_permission_case!(
        upsert_provider_credit_requires_permission,
        upsert_provider_credit(),
        CanUpsertSorafsProviderCredit,
        sorafs::visit_upsert_provider_credit
    );
    sorafs_permission_case!(
        set_moderation_policy_requires_permission,
        set_moderation_policy(),
        CanManageSorafsModeration,
        sorafs::visit_set_moderation_policy
    );
    #[test]
    fn moderation_appeal_eligibility_and_acceptance_are_public_at_executor_layer() {
        assert_allowed_without_permission(
            SubmitSorafsModerationAppeal::new(moderation_appeal_intake()),
            sorafs::visit_submit_moderation_appeal,
        );
        assert_allowed_without_permission(
            RegisterSorafsModerationJurorEligibility::new(
                "appeal-case".to_owned(),
                "round-1".to_owned(),
                vec![0x01],
            ),
            sorafs::visit_register_moderation_juror_eligibility,
        );
        assert_allowed_without_permission(
            AcceptSorafsModerationJurorAssignment::new(
                "appeal-case".to_owned(),
                "round-1".to_owned(),
                [0x02; 32],
            ),
            sorafs::visit_accept_moderation_juror_assignment,
        );
    }
    sorafs_permission_case!(
        finalize_moderation_sortition_requires_permission,
        FinalizeSorafsModerationSortition::new(
            "appeal-case".to_owned(),
            "round-1".to_owned(),
            [0x03; 32],
            [0x04; 32],
            vec![authority_account_id()],
            Vec::new(),
        ),
        CanManageSorafsModeration,
        sorafs::visit_finalize_moderation_sortition
    );
    sorafs_permission_case!(
        activate_moderation_case_requires_permission,
        ActivateSorafsModerationCase::new(
            "appeal-case".to_owned(),
            "round-1".to_owned(),
            [0x04; 32],
        ),
        CanManageSorafsModeration,
        sorafs::visit_activate_moderation_case
    );
    sorafs_permission_case!(
        set_pop_issuer_policy_requires_permission,
        set_pop_issuer_policy(),
        CanManageSorafsPopRegistry,
        sorafs::visit_set_pop_issuer_policy
    );
    sorafs_permission_case!(
        commit_pop_credential_batch_requires_permission,
        CommitSorafsPopCredentialBatch::new(vec![0x01]),
        CanOperateSorafsPopIssuer,
        sorafs::visit_commit_pop_credential_batch
    );
    sorafs_permission_case!(
        publish_pop_revocations_requires_permission,
        PublishSorafsPopRevocationList::new(vec![0x01], [1; 32]),
        CanOperateSorafsPopIssuer,
        sorafs::visit_publish_pop_revocation_list
    );
    #[test]
    fn moderation_commit_submission_is_public_at_executor_layer() {
        assert_allowed_without_permission(
            SubmitSorafsModerationCommit::new(vec![0x01]),
            sorafs::visit_submit_moderation_commit,
        );
    }
    #[test]
    fn moderation_challenge_raise_expiry_and_reveal_are_public_at_executor_layer() {
        assert_allowed_without_permission(
            RaiseSorafsModerationChallenge::new(
                "appeal-case".to_owned(),
                "round-1".to_owned(),
                "challenge-1".to_owned(),
                ModerationChallengeKindV1::Other,
                None,
                [0x31; 32],
                "public challenge".to_owned(),
            ),
            sorafs::visit_raise_moderation_challenge,
        );
        assert_allowed_without_permission(
            ExpireSorafsModerationChallenge::new(
                "appeal-case".to_owned(),
                "round-1".to_owned(),
                "challenge-1".to_owned(),
            ),
            sorafs::visit_expire_moderation_challenge,
        );
        assert_allowed_without_permission(
            SubmitSorafsModerationReveal::new(vec![0x01]),
            sorafs::visit_submit_moderation_reveal,
        );
    }
    sorafs_permission_case!(
        resolve_moderation_challenge_requires_permission,
        ResolveSorafsModerationChallenge::new(
            "appeal-case".to_owned(),
            "round-1".to_owned(),
            "challenge-1".to_owned(),
            ModerationChallengeDecisionV1::Accepted,
        ),
        CanManageSorafsModeration,
        sorafs::visit_resolve_moderation_challenge
    );
    sorafs_permission_case!(
        finalize_moderation_case_requires_permission,
        FinalizeSorafsModerationCase::new("appeal-case".to_owned(), "round-1".to_owned()),
        CanManageSorafsModeration,
        sorafs::visit_finalize_moderation_case
    );
    #[test]
    fn moderation_transparency_queries_are_public() {
        assert_allowed_without_permission(
            FindSorafsModerationPolicy,
            sorafs::visit_find_sorafs_moderation_policy,
        );
        assert_allowed_without_permission(
            FindSorafsModerationStatus,
            sorafs::visit_find_sorafs_moderation_status,
        );
        assert_allowed_without_permission(
            FindSorafsModerationAppeal::new("appeal-case".to_owned(), "round-1".to_owned()),
            sorafs::visit_find_sorafs_moderation_appeal,
        );
        assert_allowed_without_permission(
            FindSorafsModerationEvents::new(
                ModerationFinalizedCursorV1 {
                    height: 7,
                    block_hash: [0x44; 32],
                },
                None,
                16,
            ),
            sorafs::visit_find_sorafs_moderation_events,
        );
    }
    #[test]
    fn reputation_journal_query_is_public_transparency_state() {
        let cursor = ReputationJournalFinalizedCursorV1 {
            height: 7,
            block_hash: [0x45; 32],
            finalized_at_unix_ms: 1_700_000_000_000,
        };
        assert_allowed_without_permission(
            FindSorafsReputationJournalEvents::new(Some(cursor), None, 16),
            sorafs::visit_find_sorafs_reputation_journal_events,
        );
        assert_allowed_without_permission(
            FindSorafsReputationJournalEventBySourceId::new(
                ReputationJournalSourceIdV1([0x46; 32]),
                Some(cursor),
            ),
            super::visit_find_sorafs_reputation_journal_event_by_source_id,
        );
    }
    #[test]
    fn reputation_journal_authority_policy_query_requires_operator_permission() {
        let query = FindSorafsReputationJournalAuthorityPolicy;
        assert_denied_without_permission(
            query,
            sorafs::visit_find_sorafs_reputation_journal_authority_policy,
        );
        assert_allowed_with_permission(
            query,
            PermissionObject::from(CanManageSorafsReputationJournalPolicy),
            sorafs::visit_find_sorafs_reputation_journal_authority_policy,
        );
        assert_allowed_with_permission(
            query,
            PermissionObject::from(CanRecordSorafsReputationJournal),
            sorafs::visit_find_sorafs_reputation_journal_authority_policy,
        );
        assert_allowed_with_permission(
            query,
            PermissionObject::from(CanResolveSorafsCapacityDispute),
            sorafs::visit_find_sorafs_reputation_journal_authority_policy,
        );
    }
    #[test]
    fn complete_moderation_snapshot_is_manager_only() {
        let query = FindSorafsModerationSnapshot::new(8, 16);
        assert_denied_without_permission(query, sorafs::visit_find_sorafs_moderation_snapshot);
        assert_allowed_with_permission(
            query,
            PermissionObject::from(CanManageSorafsModeration),
            sorafs::visit_find_sorafs_moderation_snapshot,
        );
    }
    #[test]
    fn moderation_eligibility_query_is_self_or_manager_only() {
        assert_allowed_without_permission(
            FindSorafsModerationJurorEligibility::new(
                "appeal-case".to_owned(),
                "round-1".to_owned(),
                authority_account_id(),
            ),
            sorafs::visit_find_sorafs_moderation_juror_eligibility,
        );
        let other_juror = FindSorafsModerationJurorEligibility::new(
            "appeal-case".to_owned(),
            "round-1".to_owned(),
            owner_account_id(),
        );
        assert_denied_without_permission(
            other_juror.clone(),
            sorafs::visit_find_sorafs_moderation_juror_eligibility,
        );
        assert_allowed_with_permission(
            other_juror,
            PermissionObject::from(CanManageSorafsModeration),
            sorafs::visit_find_sorafs_moderation_juror_eligibility,
        );
    }
    #[test]
    fn derived_default_visit_dispatches_private_juror_eligibility_query() {
        with_mock_permissions(vec![PermissionObject::from(CanBindSorafsAlias)], || {
            let query = iroha_smart_contract::data_model::query::AnyQueryBox::Singular(
                FindSorafsModerationJurorEligibility::new(
                    "appeal-case".to_owned(),
                    "round-1".to_owned(),
                    owner_account_id(),
                )
                .into(),
            );
            let mut executor = MockExecutor::new(false);
            executor.visit_query(&query);
            assert!(
                executor.verdict().is_err(),
                "derived default Visit dispatch must not bypass foreign juror privacy"
            );
        });
    }
    fn orderbook_page_queries() -> Vec<iroha_smart_contract::data_model::query::AnyQueryBox> {
        [
            FindSorafsOrderbookTrades::new(None, None, 10).into(),
            FindSorafsOrderbookChannels::new(None, None, None, 10).into(),
            FindSorafsOrderbookEvents::new(None, None, 10).into(),
        ]
        .into_iter()
        .map(iroha_smart_contract::data_model::query::AnyQueryBox::Singular)
        .collect()
    }
    #[test]
    fn derived_default_visit_dispatches_orderbook_pages_through_permission_checks() {
        with_mock_permissions(vec![PermissionObject::from(CanBindSorafsAlias)], || {
            for query in orderbook_page_queries() {
                let mut executor = MockExecutor::new(false);
                executor.visit_query(&query);
                assert!(
                    executor.verdict().is_err(),
                    "derived dispatch must reject an unrelated SoraFS permission"
                );
            }
        });
        for permission in [
            PermissionObject::from(CanSetSorafsPricing),
            PermissionObject::from(CanCompleteSorafsReplicationOrder),
        ] {
            with_mock_permissions(vec![permission], || {
                for query in orderbook_page_queries() {
                    let mut executor = MockExecutor::new(false);
                    executor.visit_query(&query);
                    assert!(
                        executor.verdict().is_ok(),
                        "derived dispatch must accept an orderbook operator permission"
                    );
                }
            });
        }
    }
    include!("sccp_route_governance_permission_tests.rs");
    include!("governance_query_tail_tests.rs");
    include!("stream_token_custody_permission_tests.rs");
}
