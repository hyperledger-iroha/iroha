// Explicit identities do not turn retired-layout rejection fixtures into current models.

#[test]
fn moderation_persisted_and_rejection_roots_have_distinct_explicit_names() {
    use crate::smartcontracts::isi::sorafs::schema_test_support::assert_identity;
    assert_identity::<AppealDepositBindingStateV1>(
        "iroha_core::smartcontracts::isi::sorafs_moderation::AppealDepositBindingStateV1",
    );
    assert_identity::<AppealProofTokenBindingStateV1>(
        "iroha_core::smartcontracts::isi::sorafs_moderation::AppealProofTokenBindingStateV1",
    );
    assert_identity::<ModerationSortitionAnchorScheduleV1>(
        "iroha_core::smartcontracts::isi::sorafs_moderation::ModerationSortitionAnchorScheduleV1",
    );
    assert_identity::<ModerationPersistedEventV1>(
        "iroha_core::smartcontracts::isi::sorafs_moderation::ModerationPersistedEventV1",
    );
    assert_identity::<ModerationEventJournalHeadV1>(
        "iroha_core::smartcontracts::isi::sorafs_moderation::ModerationEventJournalHeadV1",
    );
    assert_identity::<PreCutModerationLedgerPolicyRecord>(
        "iroha_core::smartcontracts::isi::sorafs_moderation::tests::PreCutModerationLedgerPolicyRecord",
    );
    assert_identity::<PreCutModerationCaseRecordV1>(
        "iroha_core::smartcontracts::isi::sorafs_moderation::tests::PreCutModerationCaseRecordV1",
    );
    assert_identity::<PreCutModerationAppealRecordV1>(
        "iroha_core::smartcontracts::isi::sorafs_moderation::tests::PreCutModerationAppealRecordV1",
    );
    assert_ne!(
        norito::schema::identity::frame_hash::<PreCutModerationLedgerPolicyRecord>(),
        norito::schema::identity::frame_hash::<ModerationLedgerPolicyRecord>()
    );
    assert_ne!(
        norito::schema::identity::frame_hash::<PreCutModerationCaseRecordV1>(),
        norito::schema::identity::frame_hash::<ModerationCaseRecordV1>()
    );
    assert_ne!(
        norito::schema::identity::frame_hash::<PreCutModerationAppealRecordV1>(),
        norito::schema::identity::frame_hash::<ModerationAppealRecordV1>()
    );
}

#[test]
fn moderation_persistence_replays_explicit_frames_and_rejects_wrong_roots() {
    use crate::smartcontracts::isi::sorafs::schema_test_support::assert_canonical_frame;
    let deposit = AppealDepositBindingStateV1 {
        deposit_lock_digest: [0x51; 32],
        case_id: "schema-case".to_owned(),
        round_id: "schema-round".to_owned(),
        intake_digest: [0x52; 32],
    };
    let bytes = assert_canonical_frame(
        &deposit,
        "iroha_core::smartcontracts::isi::sorafs_moderation::AppealDepositBindingStateV1",
    );
    assert_eq!(encode_state(&deposit, "schema deposit").unwrap(), bytes);
    assert_eq!(
        decode_state_with_current::<AppealDepositBindingStateV1>(&bytes, "schema deposit", None)
            .unwrap(),
        deposit
    );
    let proof = AppealProofTokenBindingStateV1 {
        proof_token_digest: deposit.deposit_lock_digest,
        case_id: deposit.case_id.clone(),
        round_id: deposit.round_id.clone(),
        intake_digest: deposit.intake_digest,
    };
    assert_canonical_frame(
        &proof,
        "iroha_core::smartcontracts::isi::sorafs_moderation::AppealProofTokenBindingStateV1",
    );
    // Equal field shapes still have separate canonical identities.
    assert!(matches!(
        decode_state_with_current::<AppealProofTokenBindingStateV1>(&bytes, "wrong binding", None),
        Err(InstructionExecutionError::InvariantViolation(_))
    ));
    let schedule = ModerationSortitionAnchorScheduleV1 {
        version: 1,
        entries: vec![ModerationSortitionAnchorScheduleEntryV1 {
            registration_deadline_unix_ms: 10_000,
            case_id: deposit.case_id,
            round_id: deposit.round_id,
            intake_digest: deposit.intake_digest,
        }],
    };
    let schedule_bytes = assert_canonical_frame(
        &schedule,
        "iroha_core::smartcontracts::isi::sorafs_moderation::ModerationSortitionAnchorScheduleV1",
    );
    assert_eq!(
        decode_state_with_current::<ModerationSortitionAnchorScheduleV1>(
            &schedule_bytes,
            "schema schedule",
            None
        )
        .unwrap(),
        schedule
    );
    let record = ModerationPersistedEventV1 {
        sequence: 1,
        target_block_height: 7,
        event_index: 0,
        event: SorafsModerationLedgerEvent {
            kind: SorafsModerationLedgerEventKind::PolicyActivated,
            case_id: None,
            round_id: None,
            authority: account(&keypair(0x11)),
            occurred_at_unix_ms: OPENED_AT,
        },
    };
    validate_persisted_event(&record, 1).expect("actual event policy");
    let record_bytes = assert_canonical_frame(
        &record,
        "iroha_core::smartcontracts::isi::sorafs_moderation::ModerationPersistedEventV1",
    );
    assert_eq!(
        decode_state_with_current::<ModerationPersistedEventV1>(
            &record_bytes,
            "schema event",
            None
        )
        .unwrap(),
        record
    );
    let head = ModerationEventJournalHeadV1 {
        last_sequence: 1,
        last_target_block_height: 7,
        last_event_index: 0,
    };
    assert_canonical_frame(
        &head,
        "iroha_core::smartcontracts::isi::sorafs_moderation::ModerationEventJournalHeadV1",
    );
    let mut invalid = record;
    invalid.event.case_id = Some("unexpected-case".to_owned());
    assert!(matches!(
        validate_persisted_event(&invalid, 1),
        Err(InstructionExecutionError::InvariantViolation(_))
    ));
}
