// Recorder-policy cutover, immutable event intervals, and replay regressions.

#[test]
fn governed_token_appends_are_contiguous_and_exact_replays_are_idempotent() {
    let (mut state, authority, other, provider_id) = state_with_reputation_accounts();
    let initial_policy = policy(&authority);
    let policy_digest = initial_policy.canonical_digest().expect("policy digest");
    let first = token_entry(&authority, provider_id, policy_digest, 0x41);
    let mut retained_records = None;
    transact_test(&mut state, 1, TEST_NOW_MS, |transaction| {
        SetSorafsReputationJournalAuthorityPolicy::new(initial_policy)
            .execute(&authority, transaction)
            .expect("activate policy");
        AppendSorafsStreamTokenReputationJournalEntry::new(first.clone())
            .execute(&authority, transaction)
            .expect("append first token event");
        AppendSorafsStreamTokenReputationJournalEntry::new(first.clone())
            .execute(&authority, transaction)
            .expect("exact replay is idempotent");
        assert_eq!(
            read_journal_head(transaction.world())
                .expect("read journal head")
                .expect("journal head")
                .last_sequence,
            1
        );
        let replay_error = AppendSorafsStreamTokenReputationJournalEntry::new(first.clone())
            .execute(&other, transaction)
            .expect_err("another authority cannot replay the event");
        assert!(
            matches!(&replay_error, InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(message)) if message.contains("replay authority"))
        );
        let second = token_entry(&authority, provider_id, policy_digest, 0x51);
        AppendSorafsStreamTokenReputationJournalEntry::new(second)
            .execute(&authority, transaction)
            .expect("append second token event");
        let head = read_journal_head(transaction.world())
            .expect("read journal head")
            .expect("journal head");
        assert_eq!(head.last_sequence, 2);
        assert_eq!(head.last_event_index, 1);
        let first_record = read_event(transaction.world(), 1)
            .expect("read first event")
            .expect("first event");
        let second_record = read_event(transaction.world(), 2)
            .expect("read second event")
            .expect("second event");
        validate_event_successor(Some(&first_record), &second_record)
            .expect("events are globally contiguous");
        transaction
            .world
            .smart_contract_state
            .remove(event_key(first_record.sequence));
        assert!(
            validate_journal_head(transaction.world()).is_err(),
            "a journal with no global sequence one must fail closed"
        );
        transaction.world.smart_contract_state.insert(
            event_key(first_record.sequence),
            encode_state(&first_record, "restored first reputation event")
                .expect("encode restored first event"),
        );
        let forged_tail_key = event_key(3);
        transaction
            .world
            .smart_contract_state
            .insert(forged_tail_key.clone(), vec![0xFF]);
        assert!(
            validate_journal_head(transaction.world()).is_err(),
            "an event-prefixed key beyond the journal head must fail closed"
        );
        transaction
            .world
            .smart_contract_state
            .remove(forged_tail_key);
        let wrong_policy_entry = token_entry(&authority, provider_id, [0x99; 32], 0x61);
        AppendSorafsStreamTokenReputationJournalEntry::new(wrong_policy_entry)
            .execute(&authority, transaction)
            .expect_err("stale policy digest must fail");
        let wrong_source_family = token_entry(&authority, provider_id, policy_digest, 0x71);
        AppendSorafsPorReputationJournalEntry::new(wrong_source_family)
            .execute(&authority, transaction)
            .expect_err("PoR append must reject a stream-token source");
        assert_eq!(
            read_journal_head(transaction.world())
                .expect("read journal head")
                .expect("journal head")
                .last_sequence,
            2
        );
        retained_records = Some((first_record, second_record));
        Ok(())
    })
    .expect("commit contiguous token events");
    let (first_record, second_record) = retained_records.expect("retained event records");
    transact_test(&mut state, 2, TEST_NOW_MS + 1, |transaction| {
        let mut rotated_policy = policy(&authority);
        rotated_policy.revision = 2;
        rotated_policy.predecessor_policy_digest = Some(policy_digest);
        SetSorafsReputationJournalAuthorityPolicy::new(rotated_policy)
            .execute(&authority, transaction)
            .expect("rotate recorder policy");
        AppendSorafsStreamTokenReputationJournalEntry::new(first)
            .execute(&authority, transaction)
            .expect("exact historical entry replay remains idempotent after rotation");
        let stale_historical_entry = token_entry_at(
            &authority,
            provider_id,
            policy_digest,
            0x72,
            TEST_NOW_MS + 1,
        );
        AppendSorafsStreamTokenReputationJournalEntry::new(stale_historical_entry)
            .execute(&authority, transaction)
            .expect_err("new entries cannot use a superseded recorder policy");
        assert_eq!(
            read_journal_head(transaction.world())
                .expect("read journal after policy rotation")
                .expect("journal head")
                .last_sequence,
            2
        );
        let forged_cross_source_head = ReputationJournalSourceHeadV1 {
            source_kind: ReputationJournalSourceKindV1::StreamToken,
            source_revision: 2,
            event_id: second_record.entry.event_id,
            sequence: second_record.sequence,
        };
        transaction.world.smart_contract_state.insert(
            source_head_key(first_record.entry.source_id),
            encode_state(&forged_cross_source_head, "forged reputation source head")
                .expect("encode forged source head"),
        );
        assert!(
            validate_event_indexes(transaction.world(), &first_record).is_err(),
            "a source head must not recurse through an event from another source"
        );
        let restored_first_source_head = ReputationJournalSourceHeadV1 {
            source_kind: ReputationJournalSourceKindV1::StreamToken,
            source_revision: 1,
            event_id: first_record.entry.event_id,
            sequence: first_record.sequence,
        };
        transaction.world.smart_contract_state.insert(
            source_head_key(first_record.entry.source_id),
            encode_state(
                &restored_first_source_head,
                "restored reputation source head",
            )
            .expect("encode restored source head"),
        );
        let retained_head = read_journal_head(transaction.world())?.expect("valid retained head");
        transaction
            .world
            .smart_contract_state
            .remove(journal_head_key().clone());
        let orphan_replay = token_entry(&authority, provider_id, policy_digest, 0x41);
        let corruption = AppendSorafsStreamTokenReputationJournalEntry::new(orphan_replay)
            .execute(&authority, transaction)
            .expect_err("an orphaned journal index must fail closed on exact replay");
        assert!(matches!(
            corruption,
            InstructionExecutionError::InvariantViolation(_)
        ));
        transaction.world.smart_contract_state.insert(
            journal_head_key().clone(),
            encode_state(&retained_head, "restored replay fixture head")?,
        );
        validate_journal_head(transaction.world())?;
        Ok(())
    })
    .expect("commit rotated policy with the validated journal restored");
}

#[test]
fn recorder_policy_cutover_preserves_retained_intervals_and_exact_replays() {
    let (mut state, authority, _other, provider_id) = state_with_reputation_accounts();
    let first_policy = policy(&authority);
    let first_digest = first_policy
        .canonical_digest()
        .expect("first policy digest");
    let mut successor = first_policy.clone();
    successor.revision = 2;
    successor.predecessor_policy_digest = Some(first_digest);
    let successor_digest = successor.canonical_digest().expect("successor digest");
    transact_test(&mut state, 1, TEST_NOW_MS - 10, |transaction| {
        SetSorafsReputationJournalAuthorityPolicy::new(first_policy.clone())
            .execute(&authority, transaction)
    })
    .expect("activate source-time policy");
    let first = token_entry_at(&authority, provider_id, first_digest, 0xD1, TEST_NOW_MS);
    let delayed = token_entry_at(&authority, provider_id, first_digest, 0xD2, TEST_NOW_MS - 5);
    transact_test(&mut state, 2, TEST_NOW_MS, |transaction| {
        for entry in [&first, &delayed] {
            AppendSorafsStreamTokenReputationJournalEntry::new(entry.clone())
                .execute(&authority, transaction)?;
        }
        let terminal = read_event(transaction.world(), 2)?.expect("delayed terminal event");
        assert!(terminal.entry.source_time_unix_ms < first.source_time_unix_ms);
        assert_eq!(terminal.recorded_at_unix_ms, TEST_NOW_MS);
        assert_rejected_policy_cutover_preserves_state(transaction, &authority, &successor)?;
        for entry in [&first, &delayed] {
            AppendSorafsStreamTokenReputationJournalEntry::new(entry.clone())
                .execute(&authority, transaction)?;
        }
        // Historical policy replay must bypass only new-cutover admission, even at the
        // same time as the latest event. It must neither rewrite activation nor emit.
        let before = reputation_policy_state_snapshot(transaction);
        let event_count = transaction.world.internal_event_buf.len();
        SetSorafsReputationJournalAuthorityPolicy::new(first_policy.clone())
            .execute(&authority, transaction)?;
        assert_eq!(reputation_policy_state_snapshot(transaction), before);
        assert_eq!(transaction.world.internal_event_buf.len(), event_count);
        assert_eq!(
            validate_journal_head(transaction.world())?
                .unwrap()
                .last_sequence,
            2
        );
        Ok(())
    })
    .expect("commit same-time events after rejecting the retroactive cutover");
    // A later block at the same timestamp cannot rewrite the already committed interval.
    transact_test(&mut state, 3, TEST_NOW_MS, |transaction| {
        assert_rejected_policy_cutover_preserves_state(transaction, &authority, &successor)
    })
    .expect("commit unchanged journal after the same-time cutover is rejected");
    transact_test(&mut state, 4, TEST_NOW_MS + 1, |transaction| {
        SetSorafsReputationJournalAuthorityPolicy::new(successor.clone())
            .execute(&authority, transaction)?;
        let active = read_active_policy(transaction.world())?.expect("successor active");
        assert_eq!(active.policy_digest, successor_digest);
        assert_eq!(active.activated_at_unix_ms, TEST_NOW_MS + 1);
        let before = reputation_policy_state_snapshot(transaction);
        let event_count = transaction.world.internal_event_buf.len();
        for entry in [&first, &delayed] {
            AppendSorafsStreamTokenReputationJournalEntry::new(entry.clone())
                .execute(&authority, transaction)?;
        }
        SetSorafsReputationJournalAuthorityPolicy::new(first_policy.clone())
            .execute(&authority, transaction)?;
        assert_eq!(reputation_policy_state_snapshot(transaction), before);
        assert_eq!(transaction.world.internal_event_buf.len(), event_count);
        let expired = token_entry_at(
            &authority, provider_id, first_digest, 0xD3, TEST_NOW_MS + 1,
        );
        let error = AppendSorafsStreamTokenReputationJournalEntry::new(expired)
            .execute(&authority, transaction)
            .expect_err("the exact cutover belongs to the successor");
        assert!(matches!(&error,
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(message))
                if message.contains("outside its recorder-policy activation interval")
        ), "{error:?}");
        assert_eq!(reputation_policy_state_snapshot(transaction), before);
        let current = token_entry_at(
            &authority, provider_id, successor_digest, 0xD4, TEST_NOW_MS + 1,
        );
        AppendSorafsStreamTokenReputationJournalEntry::new(current)
            .execute(&authority, transaction)?;
        assert_eq!(validate_journal_head(transaction.world())?.unwrap().last_sequence, 3);
        Ok(())
    })
    .expect("commit later cutover and preserve historical event replay");
    let page = FindSorafsReputationJournalEvents::new(None, None, 8)
        .execute(&state.view())
        .expect("query retained events after the committed cutover");
    assert_eq!(page.events.len(), 3);
    assert_eq!(page.events[0].entry, first);
    assert_eq!(page.events[1].entry, delayed);
    assert_eq!(
        page.events[2].entry.authority_policy_digest,
        successor_digest
    );
}

fn reputation_policy_state_snapshot(
    transaction: &StateTransaction<'_, '_>,
) -> Vec<(StatePath, Vec<u8>)> {
    transaction
        .world
        .smart_contract_state
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

fn assert_rejected_policy_cutover_preserves_state(
    transaction: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    successor: &ReputationJournalAuthorityPolicyV1,
) -> Result<(), InstructionExecutionError> {
    let before = reputation_policy_state_snapshot(transaction);
    let event_count = transaction.world.internal_event_buf.len();
    let error = SetSorafsReputationJournalAuthorityPolicy::new(successor.clone())
        .execute(authority, transaction)
        .expect_err("same-time cutover must not invalidate an earlier event");
    assert!(
        matches!(&error,
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(message))
                if message == "reputation policy activation must follow every retained event commit time"
        ),
        "{error:?}"
    );
    assert_eq!(reputation_policy_state_snapshot(transaction), before);
    assert_eq!(transaction.world.internal_event_buf.len(), event_count);
    assert!(
        read_policy_history(transaction.world(), &successor.canonical_digest().unwrap())?.is_none()
    );
    for sequence in 1..=2 {
        let event = read_event(transaction.world(), sequence)?.expect("unchanged event");
        validate_event_indexes(transaction.world(), &event)?;
    }
    assert_eq!(
        validate_journal_head(transaction.world())?
            .unwrap()
            .last_sequence,
        2
    );
    Ok(())
}
