// Same-scope repair query regressions extracted to keep the parent source budget bounded.
#[test]
fn repair_task_byte_budget_returns_stable_continuation_cursor() {
    const TASK_COUNT: usize = 300;
    const EVIDENCE_PADDING_BYTES: usize = 30 * 1024;
    let mut state = make_state();
    let provider = ProviderId::new([0xC2; 32]);
    grant_repair_operator(&mut state, &alice(), provider);
    let evidence_json = format!(r#"{{"padding":"{}"}}"#, "x".repeat(EVIDENCE_PADDING_BYTES));
    transact_repair(&mut state, 1, 6_000_000, |transaction| {
        for index in 0..TASK_COUNT {
            let sequence = u16::try_from(index + 1).expect("bounded repair fixture sequence");
            let mut source_identity = [0u8; 32];
            source_identity[..2].copy_from_slice(&sequence.to_be_bytes());
            let mut manifest_digest = [0xC3; 32];
            manifest_digest[..2].copy_from_slice(&sequence.to_be_bytes());
            let mut report = repair_report(
                &format!("REP-BYTE-{sequence:03}"),
                provider,
                manifest_digest,
                &alice(),
                6_000,
            );
            report.evidence.evidence_json = Some(evidence_json.clone());
            let report_payload = to_bytes(&report).expect("encode large repair report");
            assert!(
                report_payload.len() <= REPAIR_LEDGER_MAX_CANONICAL_PAYLOAD_BYTES_V1,
                "large repair fixture must remain an admissible bounded payload"
            );
            SubmitSorafsRepairTask::new(source_identity, report_payload)
                .execute(&alice(), transaction)?;
        }
        Ok(())
    })
    .expect("commit large repair task index");
    let view = state.view();
    let first = FindSorafsRepairTasks::new(None, None, REPAIR_QUERY_MAX_ITEMS_V1)
        .execute(&view)
        .expect("query byte-bounded repair task page");
    assert!(first.has_more);
    assert!(first.tasks.len() < TASK_COUNT);
    let after = first
        .next_after_task_id
        .expect("byte-bounded task page has continuation cursor");
    assert_eq!(
        after,
        first
            .tasks
            .last()
            .expect("byte-bounded task page is non-empty")
            .task_id
    );
    let second = FindSorafsRepairTasks::new(
        Some(first.finalized_cursor),
        Some(after),
        REPAIR_QUERY_MAX_ITEMS_V1,
    )
    .execute(&view)
    .expect("continue byte-bounded repair task page");
    assert!(!second.has_more);
    assert_eq!(second.finalized_cursor, first.finalized_cursor);
    let mut task_ids = first
        .tasks
        .iter()
        .map(|task| task.task_id)
        .collect::<Vec<_>>();
    task_ids.extend(second.tasks.iter().map(|task| task.task_id));
    assert_eq!(task_ids.len(), TASK_COUNT);
    assert!(
        task_ids.windows(2).all(|pair| pair[0] < pair[1]),
        "byte-budget continuation must preserve strict task-id order"
    );
}
#[test]
fn repair_committed_event_queries_fail_closed_on_corrupt_journals() {
    let missing_head =
        committed_repair_fixture("REP-CORRUPT-HEAD", [0x61; 32], |_, transaction| {
            transaction
                .world
                .smart_contract_state
                .remove(repair_event_journal_head_key().clone());
            Ok(())
        });
    assert!(matches!(
        FindSorafsRepairEvents::new(None, None, 10).execute(&missing_head.view()),
        Err(QueryExecutionFail::Conversion(_))
    ));
    let malformed_event =
        committed_repair_fixture("REP-CORRUPT-BYTES", [0x62; 32], |_, transaction| {
            transaction
                .world
                .smart_contract_state
                .insert(repair_event_key(1), vec![0xFF; 16]);
            Ok(())
        });
    assert!(matches!(
        FindSorafsRepairEvents::new(None, None, 10).execute(&malformed_event.view()),
        Err(QueryExecutionFail::Conversion(_))
    ));
    let orphan_event =
        committed_repair_fixture("REP-CORRUPT-ORPHAN", [0x63; 32], |_, transaction| {
            let mut orphan = read_repair_persisted_event(transaction.world(), 1)?
                .expect("initial repair event exists");
            orphan.sequence = 2;
            orphan.event_index = 1;
            transaction.world.smart_contract_state.insert(
                repair_event_key(2),
                encode_repair_state(&orphan, "orphan repair event")?,
            );
            Ok(())
        });
    assert!(matches!(
        FindSorafsRepairEvents::new(None, None, 10).execute(&orphan_event.view()),
        Err(QueryExecutionFail::Conversion(_))
    ));
    let orphan_task = committed_repair_fixture("REP-CORRUPT-TASK", [0x64; 32], |_, transaction| {
        let mut orphan = read_repair_persisted_event(transaction.world(), 1)?
            .expect("initial repair event exists");
        orphan.event.ticket_id = "REP-MISSING-TASK".to_owned();
        transaction.world.smart_contract_state.insert(
            repair_event_key(1),
            encode_repair_state(&orphan, "orphan-task repair event")?,
        );
        Ok(())
    });
    assert!(matches!(
        FindSorafsRepairEvents::new(None, None, 10).execute(&orphan_task.view()),
        Err(QueryExecutionFail::Conversion(_))
    ));
    let missing_middle =
        committed_repair_fixture("REP-CORRUPT-GAP", [0x65; 32], |report, transaction| {
            ApplySorafsRepairTaskAction::new(
                report.ticket_id.0.clone(),
                1,
                SorafsRepairTaskActionV1::Claim(SorafsRepairClaimV1 {
                    lease_duration_ms: REPAIR_LEDGER_MIN_LEASE_MS_V1,
                    idempotency_key: "gap-claim".to_owned(),
                }),
            )
            .execute(&alice(), transaction)?;
            ApplySorafsRepairTaskAction::new(
                report.ticket_id.0.clone(),
                2,
                SorafsRepairTaskActionV1::Renew(SorafsRepairRenewV1 {
                    lease_generation: 1,
                    lease_duration_ms: REPAIR_LEDGER_MIN_LEASE_MS_V1,
                    idempotency_key: "gap-renew".to_owned(),
                }),
            )
            .execute(&alice(), transaction)?;
            transaction
                .world
                .smart_contract_state
                .remove(repair_event_key(2));
            Ok(())
        });
    assert!(matches!(
        FindSorafsRepairEvents::new(None, None, 10).execute(&missing_middle.view()),
        Err(QueryExecutionFail::Conversion(_))
    ));
    let nonfinalized =
        committed_repair_fixture("REP-CORRUPT-HEIGHT", [0x66; 32], |_, transaction| {
            let mut event = read_repair_persisted_event(transaction.world(), 1)?
                .expect("initial repair event exists");
            event.target_block_height = 2;
            let mut head = read_repair_event_journal_head(transaction.world())?
                .expect("repair event head exists");
            head.last_target_block_height = 2;
            transaction.world.smart_contract_state.insert(
                repair_event_key(1),
                encode_repair_state(&event, "non-finalized repair event")?,
            );
            transaction.world.smart_contract_state.insert(
                repair_event_journal_head_key().clone(),
                encode_repair_state(&head, "non-finalized repair event head")?,
            );
            Ok(())
        });
    assert!(matches!(
        FindSorafsRepairEvents::new(None, None, 10).execute(&nonfinalized.view()),
        Err(QueryExecutionFail::Conversion(_))
    ));
}
#[test]
fn repair_query_resource_and_encoded_budget_guards_fail_closed() {
    let oversized_record =
        committed_repair_fixture("REP-BUDGET-STATE", [0x67; 32], |_, transaction| {
            transaction.world.smart_contract_state.insert(
                repair_source_key([0x67; 32]),
                vec![0xFF; REPAIR_STATE_MAX_BYTES_V1 + 1],
            );
            Ok(())
        });
    assert!(matches!(
        FindSorafsRepairTasks::new(None, None, 1).execute(&oversized_record.view()),
        Err(QueryExecutionFail::Conversion(_))
    ));
    let mut inspected_records = 3usize;
    assert!(
        charge_repair_query_inspected_records(
            &mut inspected_records,
            1,
            3,
            "adversarial sparse repair projection",
        )
        .is_err()
    );
    let mut state_read_bytes = REPAIR_QUERY_MAX_EVENT_STATE_READ_BYTES_V1;
    assert!(
        charge_repair_query_state_bytes(
            &mut state_read_bytes,
            1,
            REPAIR_QUERY_MAX_EVENT_STATE_READ_BYTES_V1,
            "adversarial repair projection",
        )
        .is_err()
    );
    let state = committed_repair_fixture("REP-BUDGET-PAGE", [0x68; 32], |_, _| Ok(()));
    let view = state.view();
    let finalized_task = FindSorafsRepairTask::new("REP-BUDGET-PAGE".to_owned(), None)
        .execute(&view)
        .expect("query repair budget fixture task");
    let mut oversized_task = finalized_task.task;
    oversized_task.canonical_report = vec![0xA5; REPAIR_QUERY_MAX_TASK_PAGE_BYTES_V1 + 1];
    let oversized_task_page = RepairLedgerTaskPageV1 {
        finalized_cursor: finalized_task.finalized_cursor,
        tasks: vec![oversized_task],
        has_more: false,
        next_after_task_id: None,
    };
    assert!(
        ensure_repair_query_encoded_budget(
            &oversized_task_page,
            REPAIR_QUERY_MAX_TASK_PAGE_BYTES_V1,
            "adversarial repair task page",
        )
        .is_err()
    );
    let event_page = FindSorafsRepairEvents::new(Some(finalized_task.finalized_cursor), None, 1)
        .execute(&view)
        .expect("query repair budget fixture event");
    let event = event_page
        .events
        .into_iter()
        .next()
        .expect("repair budget fixture has one event");
    let mut oversized_event = event;
    oversized_event.event.ticket_id = "x".repeat(REPAIR_QUERY_MAX_EVENT_PAGE_BYTES_V1 + 1);
    let oversized_event_page = RepairFinalizedEventPageV1 {
        finalized_cursor: finalized_task.finalized_cursor,
        events: vec![oversized_event],
        has_more: false,
        next_after: None,
    };
    assert!(
        ensure_repair_query_encoded_budget(
            &oversized_event_page,
            REPAIR_QUERY_MAX_EVENT_PAGE_BYTES_V1,
            "adversarial repair event page",
        )
        .is_err()
    );
}
#[test]
fn repair_escalation_and_provider_appeal_are_atomic_and_idempotent() {
    let mut state = make_state();
    let provider = ProviderId::new([0xE1; 32]);
    grant_repair_operator(&mut state, &alice(), provider);
    state.world.provider_owners.insert(provider, bob());
    let report = repair_report("REP-SLASH-1", provider, [0xE2; 32], &alice(), 3_000);
    let header = repair_block_header(1, 3_000_000);
    let block_hash = iroha_crypto::HashOf::new(&header);
    let mut block = state.block(header);
    let mut transaction = block.transaction();
    SubmitSorafsRepairTask::new([0xE3; 32], to_bytes(&report).expect("encode repair report"))
        .execute(&alice(), &mut transaction)
        .expect("submit repair task");
    ApplySorafsRepairTaskAction::new(
        report.ticket_id.0.clone(),
        1,
        SorafsRepairTaskActionV1::Claim(SorafsRepairClaimV1 {
            lease_duration_ms: REPAIR_LEDGER_MIN_LEASE_MS_V1,
            idempotency_key: "claim-slash".to_owned(),
        }),
    )
    .execute(&alice(), &mut transaction)
    .expect("claim repair task");
    let slash = RepairSlashProposalV1 {
        version: REPAIR_SLASH_PROPOSAL_VERSION_V1,
        ticket_id: report.ticket_id.clone(),
        provider_id: *provider.as_bytes(),
        manifest_digest: report.evidence.manifest_digest,
        auditor_account: report.auditor_account.clone(),
        proposed_penalty: "0.000001".parse().expect("valid XOR quantity"),
        submitted_at_unix: report.submitted_at_unix,
        rationale: "repair SLA failed".to_owned(),
        approval: None,
    };
    ApplySorafsRepairTaskAction::new(
        report.ticket_id.0.clone(),
        2,
        SorafsRepairTaskActionV1::Escalate(SorafsRepairEscalateV1 {
            lease_generation: 1,
            slash_proposal_payload: to_bytes(&slash).expect("encode slash proposal"),
            idempotency_key: "escalate-1".to_owned(),
        }),
    )
    .execute(&alice(), &mut transaction)
    .expect("slash proposal and terminal escalation commit atomically");
    let appeal = SubmitSorafsRepairAppeal::new(
        report.ticket_id.0.clone(),
        3,
        [0xE4; 32],
        "provider counter-evidence".to_owned(),
        "appeal-1".to_owned(),
    );
    appeal
        .clone()
        .execute(&bob(), &mut transaction)
        .expect("provider owner appeals committed slash");
    appeal
        .execute(&bob(), &mut transaction)
        .expect("exact appeal replay is idempotent");
    let conflicting_replay = SubmitSorafsRepairAppeal::new(
        report.ticket_id.0.clone(),
        3,
        [0xE5; 32],
        "different evidence".to_owned(),
        "appeal-1".to_owned(),
    )
    .execute(&bob(), &mut transaction)
    .expect_err("appeal idempotency key cannot be rebound");
    assert!(smart_contract_error_message(&conflicting_replay).contains("different action"));
    let duplicate_appeal = SubmitSorafsRepairAppeal::new(
        report.ticket_id.0.clone(),
        4,
        [0xE6; 32],
        "second appeal".to_owned(),
        "appeal-2".to_owned(),
    )
    .execute(&bob(), &mut transaction)
    .expect_err("slash proposal permits only one appeal");
    assert!(smart_contract_error_message(&duplicate_appeal).contains("single appeal"));
    transaction.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("commit repair escalation block");
    state.push_block_hash_for_testing(block_hash);
    let view = state.view();
    let task = FindSorafsRepairTask::new(report.ticket_id.0, None)
        .execute(&view)
        .expect("typed finalized task query");
    assert!(task.task.slash.is_some());
    assert!(task.task.appeal.is_some());
    assert!(matches!(
        task.task.terminal_outcome,
        Some(RepairLedgerTerminalOutcomeV1 {
            kind: RepairLedgerTerminalKindV1::Escalated(_),
            ..
        })
    ));
    let status = FindSorafsRepairStatus::new(Some(task.finalized_cursor))
        .execute(&view)
        .expect("typed finalized status query");
    assert_eq!(status.status.escalated, 1);
    assert_eq!(status.status.slash_proposals, 1);
    assert_eq!(status.status.appeals, 1);
    let events = FindSorafsRepairEvents::new(Some(task.finalized_cursor), None, 10)
        .execute(&view)
        .expect("query escalation event journal");
    assert_eq!(events.events.len(), 4, "exact replays emit no journal rows");
    assert_eq!(
        events
            .events
            .iter()
            .map(|event| event.event.kind)
            .collect::<Vec<_>>(),
        vec![
            SorafsRepairLedgerEventKind::TaskSubmitted,
            SorafsRepairLedgerEventKind::LeaseClaimed,
            SorafsRepairLedgerEventKind::Escalated,
            SorafsRepairLedgerEventKind::Appealed,
        ]
    );
}
