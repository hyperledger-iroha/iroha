/// Native repair admission retains Core's scoped authority and idempotency checks.
mod sorafs_repair_admission {
    use super::*;
    use crate::executor::Executor;
    use crate::smartcontracts::ValidSingularQuery;
    use iroha_data_model::{
        isi::{
            error::{InstructionExecutionError, InvalidParameterError},
            sorafs::{
                ApplySorafsRepairTaskAction, SorafsRepairClaimV1, SorafsRepairCompleteV1,
                SorafsRepairEscalateV1, SorafsRepairFailV1, SorafsRepairTaskActionV1,
                SubmitSorafsRepairAppeal, SubmitSorafsRepairTask,
            },
        },
        query::sorafs::prelude::{
            FindSorafsRepairEvents, FindSorafsRepairStatus, FindSorafsRepairTask,
        },
        sorafs::{
            capacity::ProviderId,
            moderation_ledger::{REPAIR_LEDGER_MIN_LEASE_MS_V1, RepairLedgerTerminalKindV1},
        },
    };
    use iroha_executor_data_model::permission::sorafs::CanOperateSorafsRepair;
    use sorafs_manifest::repair::{
        REPAIR_EVIDENCE_VERSION_V1, REPAIR_REPORT_VERSION_V1, REPAIR_SLASH_PROPOSAL_VERSION_V1,
        RepairCauseV1, RepairEvidenceV1, RepairManualCauseV1, RepairReportV1,
        RepairSlashProposalV1, RepairTicketId,
    };

    const TICKET: &str = "REP-INITIAL-1";
    const PROVIDER: [u8; 32] = [0xD1; 32];
    const MANIFEST: [u8; 32] = [0xD2; 32];
    const SOURCE: [u8; 32] = [0xD3; 32];
    const EVIDENCE: [u8; 32] = [0xD4; 32];

    fn permission(provider: [u8; 32]) -> Permission {
        CanOperateSorafsRepair {
            provider_id: ProviderId::new(provider),
        }
        .into()
    }

    fn fixture(bob_provider: [u8; 32]) -> State {
        let mut world = World::with(
            [],
            [
                Account::new(ALICE_ID.clone()).build(&ALICE_ID),
                Account::new(BOB_ID.clone()).build(&BOB_ID),
            ],
            [],
        );
        world
            .account_permissions
            .insert(ALICE_ID.clone(), BTreeSet::from([permission(PROVIDER)]));
        world
            .account_permissions
            .insert(BOB_ID.clone(), BTreeSet::from([permission(bob_provider)]));
        world
            .provider_owners
            .insert(ProviderId::new(PROVIDER), BOB_ID.clone());
        state_after_genesis(world)
    }

    fn submission(auditor: &AccountId) -> InstructionBox {
        SubmitSorafsRepairTask::new(
            SOURCE,
            norito::to_bytes(&RepairReportV1 {
                version: REPAIR_REPORT_VERSION_V1,
                ticket_id: RepairTicketId(TICKET.to_owned()),
                auditor_account: auditor.to_string(),
                submitted_at_unix: 1,
                evidence: RepairEvidenceV1 {
                    version: REPAIR_EVIDENCE_VERSION_V1,
                    manifest_digest: MANIFEST,
                    provider_id: PROVIDER,
                    por_history_id: None,
                    cause: RepairCauseV1::Manual(RepairManualCauseV1 {
                        reason: "native Initial executor regression".to_owned(),
                    }),
                    evidence_json: None,
                    notes: None,
                },
                notes: None,
            })
            .expect("canonical repair report"),
        )
        .into()
    }

    fn claim(revision: u64, key: &str) -> InstructionBox {
        ApplySorafsRepairTaskAction::new(
            TICKET.to_owned(),
            revision,
            SorafsRepairTaskActionV1::Claim(SorafsRepairClaimV1 {
                lease_duration_ms: REPAIR_LEDGER_MIN_LEASE_MS_V1,
                idempotency_key: key.to_owned(),
            }),
        )
        .into()
    }

    fn complete(revision: u64, generation: u64, key: &str) -> InstructionBox {
        ApplySorafsRepairTaskAction::new(
            TICKET.to_owned(),
            revision,
            SorafsRepairTaskActionV1::Complete(SorafsRepairCompleteV1 {
                lease_generation: generation,
                evidence_digest: EVIDENCE,
                idempotency_key: key.to_owned(),
            }),
        )
        .into()
    }

    fn stored_state(transaction: &StateTransaction<'_, '_>) -> Vec<(String, Vec<u8>)> {
        transaction
            .world
            .smart_contract_state
            .iter()
            .map(|(key, value)| (key.to_string(), value.clone()))
            .collect()
    }

    fn reject_unchanged(
        transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        instruction: InstructionBox,
        marker: &str,
    ) {
        let before = stored_state(transaction);
        let error = Executor::Initial
            .execute_instruction(transaction, authority, instruction)
            .expect_err("native repair validation must reject the instruction");
        assert!(
            matches!(
                error,
                ValidationFail::InstructionFailed(InstructionExecutionError::InvalidParameter(
                    InvalidParameterError::SmartContract(ref message)
                )) if message.contains(marker)
            ),
            "expected native repair rejection containing {marker:?}, got {error:?}"
        );
        assert_eq!(
            stored_state(transaction),
            before,
            "rejection changed repair state"
        );
    }

    fn replay_unchanged(
        transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        instruction: InstructionBox,
    ) {
        let before = stored_state(transaction);
        Executor::Initial
            .execute_instruction(transaction, authority, instruction)
            .expect("exact authorized action replay reaches the native idempotency receipt");
        assert_eq!(
            stored_state(transaction),
            before,
            "replay changed repair state"
        );
    }

    #[test]
    fn initial_executor_repair_preserves_provider_scope_and_authority_bound_replay() {
        let mut state = fixture([0xE1; 32]);
        let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 2_000, 0);
        let block_hash = HashOf::new(&header);
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        reject_unchanged(
            &mut transaction,
            &BOB_ID,
            submission(&BOB_ID),
            "provider-scoped",
        );
        Executor::Initial
            .execute_instruction(&mut transaction, &ALICE_ID, submission(&ALICE_ID))
            .expect("provider worker submits through the native Initial executor");
        replay_unchanged(&mut transaction, &ALICE_ID, submission(&ALICE_ID));
        reject_unchanged(
            &mut transaction,
            &BOB_ID,
            claim(1, "bob-claim"),
            "provider-scoped",
        );
        let claimed = claim(1, "alice-claim");
        Executor::Initial
            .execute_instruction(&mut transaction, &ALICE_ID, claimed.clone())
            .expect("authorized worker claims through the native Initial executor");
        replay_unchanged(&mut transaction, &ALICE_ID, claimed.clone());
        reject_unchanged(&mut transaction, &BOB_ID, claimed, "different action");
        reject_unchanged(
            &mut transaction,
            &ALICE_ID,
            claim(2, "alice-claim"),
            "different action",
        );
        let completion = complete(2, 1, "complete");
        Executor::Initial
            .execute_instruction(&mut transaction, &ALICE_ID, completion.clone())
            .expect("lease owner completes through the native Initial executor");
        replay_unchanged(&mut transaction, &ALICE_ID, completion.clone());
        reject_unchanged(&mut transaction, &BOB_ID, completion, "different action");
        reject_unchanged(
            &mut transaction,
            &ALICE_ID,
            ApplySorafsRepairTaskAction::new(
                TICKET.to_owned(),
                3,
                SorafsRepairTaskActionV1::Fail(SorafsRepairFailV1 {
                    lease_generation: 1,
                    failure_digest: EVIDENCE,
                    idempotency_key: "second-terminal".to_owned(),
                }),
            )
            .into(),
            "terminal outcome",
        );
        transaction.apply();
        block
            .commit_world_overlay_for_testing()
            .expect("commit repair state");
        state.push_block_hash_for_testing(block_hash);
        let view = state.view();
        let task = FindSorafsRepairTask::new(TICKET.to_owned(), None)
            .execute(&view)
            .expect("read finalized task");
        assert_eq!(task.task.revision, 3);
        let terminal = task.task.terminal_outcome.expect("completed task");
        assert_eq!(terminal.finalized_by, *ALICE_ID);
        assert!(
            matches!(terminal.kind, RepairLedgerTerminalKindV1::Completed(result) if result.evidence_digest == EVIDENCE)
        );
        let status = FindSorafsRepairStatus::new(Some(task.finalized_cursor))
            .execute(&view)
            .expect("read counters");
        assert_eq!(status.status.completed, 1);
        assert_eq!(status.status.terminal_outcomes, 1);
        let events = FindSorafsRepairEvents::new(Some(task.finalized_cursor), None, 10)
            .execute(&view)
            .expect("read journal");
        assert_eq!(events.events.len(), 3);
    }

    #[test]
    fn initial_executor_repair_revocation_and_reauthorization_do_not_restore_a_stale_lease() {
        let state = fixture(PROVIDER);
        let mut block = state.block(BlockHeader::new(
            nonzero!(2_u64),
            None,
            None,
            None,
            2_000,
            0,
        ));
        let mut transaction = block.transaction();
        for instruction in [submission(&ALICE_ID), claim(1, "alice-claim")] {
            Executor::Initial
                .execute_instruction(&mut transaction, &ALICE_ID, instruction)
                .expect("seed native lease");
        }
        Executor::Initial
            .execute_instruction(
                &mut transaction,
                &ALICE_ID,
                Revoke::account_permission(permission(PROVIDER), ALICE_ID.clone()).into(),
            )
            .expect("exact holder self-revokes through native permission policy");
        replay_unchanged(&mut transaction, &ALICE_ID, claim(1, "alice-claim"));
        reject_unchanged(
            &mut transaction,
            &ALICE_ID,
            complete(2, 1, "revoked-complete"),
            "provider-scoped",
        );
        Executor::Initial
            .execute_instruction(&mut transaction, &BOB_ID, claim(2, "bob-reclaim"))
            .expect("reclaim revoked owner's unexpired lease");
        Executor::Initial
            .execute_instruction(
                &mut transaction,
                &BOB_ID,
                Grant::account_permission(permission(PROVIDER), ALICE_ID.clone()).into(),
            )
            .expect("remaining exact holder reauthorizes the old worker");
        reject_unchanged(
            &mut transaction,
            &ALICE_ID,
            complete(3, 1, "stale-owner"),
            "different account",
        );
        reject_unchanged(
            &mut transaction,
            &BOB_ID,
            complete(3, 1, "stale-generation"),
            "generation mismatch",
        );
        Executor::Initial
            .execute_instruction(&mut transaction, &BOB_ID, complete(3, 2, "bob-complete"))
            .expect("replacement lease retains terminal authority");
    }

    #[test]
    fn initial_executor_repair_appeal_requires_provider_owner_and_rejects_rebound_replay() {
        let mut state = fixture([0xE1; 32]);
        let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 2_000, 0);
        let block_hash = HashOf::new(&header);
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        let proposal = RepairSlashProposalV1 {
            version: REPAIR_SLASH_PROPOSAL_VERSION_V1,
            ticket_id: RepairTicketId(TICKET.to_owned()),
            provider_id: PROVIDER,
            manifest_digest: MANIFEST,
            auditor_account: ALICE_ID.to_string(),
            proposed_penalty: "0.000001".parse().expect("positive XOR penalty"),
            submitted_at_unix: 1,
            rationale: "repair SLA failed".to_owned(),
            approval: None,
        };
        let escalation = ApplySorafsRepairTaskAction::new(
            TICKET.to_owned(),
            2,
            SorafsRepairTaskActionV1::Escalate(SorafsRepairEscalateV1 {
                lease_generation: 1,
                slash_proposal_payload: norito::to_bytes(&proposal).expect("canonical proposal"),
                idempotency_key: "escalate".to_owned(),
            }),
        )
        .into();
        for instruction in [submission(&ALICE_ID), claim(1, "claim"), escalation] {
            Executor::Initial
                .execute_instruction(&mut transaction, &ALICE_ID, instruction)
                .expect("native repair escalation");
        }
        let appeal: InstructionBox = SubmitSorafsRepairAppeal::new(
            TICKET.to_owned(),
            3,
            EVIDENCE,
            "provider counter-evidence".to_owned(),
            "appeal".to_owned(),
        )
        .into();
        reject_unchanged(&mut transaction, &ALICE_ID, appeal.clone(), "owned by");
        Executor::Initial
            .execute_instruction(&mut transaction, &BOB_ID, appeal.clone())
            .expect("governed owner appeals without worker permission");
        replay_unchanged(&mut transaction, &BOB_ID, appeal.clone());
        reject_unchanged(&mut transaction, &ALICE_ID, appeal, "different action");
        reject_unchanged(
            &mut transaction,
            &BOB_ID,
            SubmitSorafsRepairAppeal::new(
                TICKET.to_owned(),
                3,
                [0xE2; 32],
                "changed evidence".to_owned(),
                "appeal".to_owned(),
            )
            .into(),
            "different action",
        );
        reject_unchanged(
            &mut transaction,
            &BOB_ID,
            SubmitSorafsRepairAppeal::new(
                TICKET.to_owned(),
                4,
                EVIDENCE,
                "second appeal".to_owned(),
                "appeal-2".to_owned(),
            )
            .into(),
            "single appeal",
        );
        transaction.apply();
        block
            .commit_world_overlay_for_testing()
            .expect("commit appealed repair");
        state.push_block_hash_for_testing(block_hash);
        let view = state.view();
        let task = FindSorafsRepairTask::new(TICKET.to_owned(), None)
            .execute(&view)
            .expect("read appealed task");
        assert_eq!(task.task.revision, 4);
        let appeal = task.task.appeal.expect("one provider appeal");
        assert_eq!(appeal.appellant, *BOB_ID);
        assert_eq!(appeal.evidence_digest, EVIDENCE);
        let status = FindSorafsRepairStatus::new(Some(task.finalized_cursor))
            .execute(&view)
            .expect("read appeal counters");
        assert_eq!(status.status.appeals, 1);
        assert_eq!(status.status.escalated, 1);
        assert_eq!(status.status.terminal_outcomes, 1);
        let events = FindSorafsRepairEvents::new(Some(task.finalized_cursor), None, 10)
            .execute(&view)
            .expect("read appeal journal");
        assert_eq!(events.events.len(), 4);
    }
}
