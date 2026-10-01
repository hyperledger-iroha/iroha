// Source-independent journal integrity and recorder-policy cutover through signed execution.
// Stream-token Append now requires a native delivery intent; its authority and exact-envelope
// coverage lives in sorafs_stream_token_gateway/delivery_tests.rs. PoR retains the generic journal
// path exercised here. Deliberate index corruption is confined to discarded read-only overlays.

#[test]
fn governed_por_appends_are_contiguous_and_exact_replays_are_idempotent() {
    let (mut chain, authority, other, provider_id) = certified_reputation_chain();
    let initial_policy = policy(&authority);
    let policy_digest = initial_policy.canonical_digest().expect("policy digest");
    commit_reputation_instruction(
        &mut chain,
        TEST_NOW_MS,
        1,
        SetSorafsReputationJournalAuthorityPolicy::new(initial_policy),
    )
    .expect("activate policy");
    let first = por_entry_at(
        &authority,
        provider_id,
        policy_digest,
        0x41,
        TEST_NOW_MS + 1_000,
    );
    let results = commit_reputation_instructions(
        &mut chain,
        TEST_NOW_MS + 1_000,
        vec![
            (
                1,
                AppendSorafsPorReputationJournalEntry::new(first.clone()).into(),
            ),
            (
                1,
                AppendSorafsPorReputationJournalEntry::new(first.clone()).into(),
            ),
            (
                2,
                AppendSorafsPorReputationJournalEntry::new(first.clone()).into(),
            ),
        ],
    );
    assert!(results[0].is_ok(), "{:?}", results[0]);
    assert!(results[1].is_ok(), "{:?}", results[1]);
    assert_instruction_message(
        results[2]
            .as_ref()
            .expect_err("another authority cannot replay the event"),
        "replay authority",
    );
    assert_eq!(
        read_journal_head(chain.state().view().world())
            .unwrap()
            .unwrap()
            .last_sequence,
        1
    );
    let second = por_entry_at(
        &authority,
        provider_id,
        policy_digest,
        0x51,
        TEST_NOW_MS + 1_000,
    );
    commit_reputation_instruction(
        &mut chain,
        TEST_NOW_MS + 2_000,
        1,
        AppendSorafsPorReputationJournalEntry::new(second),
    )
    .expect("append second event");
    let (first_record, second_record) = {
        let view = chain.state().view();
        let head = read_journal_head(view.world()).unwrap().unwrap();
        assert_eq!(head.last_sequence, 2);
        // The second block's first event starts at zero; same-block contiguity is checked below.
        assert_eq!(head.last_event_index, 0);
        let first = read_event(view.world(), 1).unwrap().unwrap();
        let second = read_event(view.world(), 2).unwrap().unwrap();
        validate_event_successor(Some(&first), &second).expect("events are globally contiguous");
        (first, second)
    };
    inspect_journal_overlay(&chain, |transaction| {
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
            encode_state(&first_record, "restored first reputation event").unwrap(),
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
        let wrong_policy_entry = por_entry_at(
            &authority,
            provider_id,
            [0x99; 32],
            0x61,
            TEST_NOW_MS + 1_000,
        );
        AppendSorafsPorReputationJournalEntry::new(wrong_policy_entry)
            .execute(&authority, transaction)
            .expect_err("stale policy digest must fail");
        let wrong_source_family = token_entry(&authority, provider_id, policy_digest, 0x71);
        AppendSorafsPorReputationJournalEntry::new(wrong_source_family)
            .execute(&authority, transaction)
            .expect_err("PoR append must reject a stream-token source");
        assert_eq!(
            read_journal_head(transaction.world())
                .unwrap()
                .unwrap()
                .last_sequence,
            2
        );
        assert_ne!(
            authority, other,
            "replay rejection used a distinct registered account"
        );
    });
    let mut rotated_policy = policy(&authority);
    rotated_policy.revision = 2;
    rotated_policy.predecessor_policy_digest = Some(policy_digest);
    commit_reputation_instruction(
        &mut chain,
        TEST_NOW_MS + 3_000,
        1,
        SetSorafsReputationJournalAuthorityPolicy::new(rotated_policy),
    )
    .expect("rotate recorder policy");
    let stale_historical_entry = por_entry_at(
        &authority,
        provider_id,
        policy_digest,
        0x72,
        TEST_NOW_MS + 3_000,
    );
    let results = commit_reputation_instructions(
        &mut chain,
        TEST_NOW_MS + 4_000,
        vec![
            (
                1,
                AppendSorafsPorReputationJournalEntry::new(first.clone()).into(),
            ),
            (
                1,
                AppendSorafsPorReputationJournalEntry::new(stale_historical_entry).into(),
            ),
        ],
    );
    assert!(
        results[0].is_ok(),
        "exact historical entry replay remains idempotent after rotation: {:?}",
        results[0]
    );
    assert!(
        results[1].is_err(),
        "new entries cannot use a superseded recorder policy"
    );
    assert_eq!(
        read_journal_head(chain.state().view().world())
            .unwrap()
            .unwrap()
            .last_sequence,
        2
    );
    inspect_journal_overlay(&chain, |transaction| {
        let forged_cross_source_head = ReputationJournalSourceHeadV1 {
            source_kind: ReputationJournalSourceKindV1::Por,
            source_revision: 2,
            event_id: second_record.entry.event_id,
            sequence: second_record.sequence,
        };
        transaction.world.smart_contract_state.insert(
            source_head_key(first_record.entry.source_id),
            encode_state(&forged_cross_source_head, "forged reputation source head").unwrap(),
        );
        assert!(
            validate_event_indexes(transaction.world(), &first_record).is_err(),
            "a source head must not recurse through an event from another source"
        );
        let restored = ReputationJournalSourceHeadV1 {
            source_kind: ReputationJournalSourceKindV1::Por,
            source_revision: 1,
            event_id: first_record.entry.event_id,
            sequence: first_record.sequence,
        };
        transaction.world.smart_contract_state.insert(
            source_head_key(first_record.entry.source_id),
            encode_state(&restored, "restored reputation source head").unwrap(),
        );
        let retained_head = read_journal_head(transaction.world()).unwrap().unwrap();
        transaction
            .world
            .smart_contract_state
            .remove(journal_head_key().clone());
        let corruption = AppendSorafsPorReputationJournalEntry::new(first)
            .execute(&authority, transaction)
            .expect_err("an orphaned journal index must fail closed on exact replay");
        assert!(matches!(
            corruption,
            InstructionExecutionError::InvariantViolation(_)
        ));
        transaction.world.smart_contract_state.insert(
            journal_head_key().clone(),
            encode_state(&retained_head, "restored replay fixture head").unwrap(),
        );
        validate_journal_head(transaction.world()).unwrap();
    });
}

#[test]
fn recorder_policy_cutover_preserves_retained_intervals_and_exact_replays() {
    let (mut chain, authority, _other, provider_id) = certified_reputation_chain();
    let first_policy = policy(&authority);
    let first_digest = first_policy
        .canonical_digest()
        .expect("first policy digest");
    let mut successor = first_policy.clone();
    successor.revision = 2;
    successor.predecessor_policy_digest = Some(first_digest);
    let successor_digest = successor.canonical_digest().expect("successor digest");
    commit_reputation_instruction(
        &mut chain,
        TEST_NOW_MS - 1_000,
        1,
        SetSorafsReputationJournalAuthorityPolicy::new(first_policy.clone()),
    )
    .expect("activate source-time policy");
    chain.take_events().unwrap();
    let first = por_entry_at(&authority, provider_id, first_digest, 0xD1, TEST_NOW_MS);
    let delayed = por_entry_at(
        &authority,
        provider_id,
        first_digest,
        0xD2,
        TEST_NOW_MS - 500,
    );
    // Distinct sole-instruction envelopes execute in one certified block. This tests two
    // successive same-time cutover refusals without fabricating a non-increasing block clock.
    let results = commit_reputation_instructions(
        &mut chain,
        TEST_NOW_MS,
        vec![
            (
                1,
                AppendSorafsPorReputationJournalEntry::new(first.clone()).into(),
            ),
            (
                1,
                AppendSorafsPorReputationJournalEntry::new(delayed.clone()).into(),
            ),
            (
                1,
                SetSorafsReputationJournalAuthorityPolicy::new(successor.clone()).into(),
            ),
            (
                1,
                SetSorafsReputationJournalAuthorityPolicy::new(first_policy.clone()).into(),
            ),
            (
                1,
                SetSorafsReputationJournalAuthorityPolicy::new(successor.clone()).into(),
            ),
            (
                1,
                AppendSorafsPorReputationJournalEntry::new(first.clone()).into(),
            ),
            (
                1,
                AppendSorafsPorReputationJournalEntry::new(delayed.clone()).into(),
            ),
        ],
    );
    for index in [0, 1, 3, 5, 6] {
        assert!(results[index].is_ok(), "{index}: {:?}", results[index]);
    }
    for index in [2, 4] {
        assert_instruction_message(
            results[index]
                .as_ref()
                .expect_err("same-time cutover must preserve earlier events"),
            "reputation policy activation must follow every retained event commit time",
        );
    }
    assert_eq!(
        take_policy_activation_count(&mut chain),
        0,
        "rejections and historical replay emit no activation"
    );
    {
        let view = chain.state().view();
        assert_eq!(
            read_active_policy(view.world())
                .unwrap()
                .unwrap()
                .policy_digest,
            first_digest
        );
        assert!(
            read_policy_history(view.world(), &successor_digest)
                .unwrap()
                .is_none()
        );
        let terminal = read_event(view.world(), 2).unwrap().unwrap();
        assert!(terminal.entry.source_time_unix_ms < first.source_time_unix_ms);
        assert_eq!(terminal.recorded_at_unix_ms, TEST_NOW_MS);
        let head = validate_journal_head(view.world()).unwrap().unwrap();
        assert_eq!(head.last_sequence, 2);
        assert_eq!(
            head.last_event_index, 1,
            "same-block events remain contiguous"
        );
        for sequence in 1..=2 {
            let event = read_event(view.world(), sequence).unwrap().unwrap();
            validate_event_indexes(view.world(), &event).unwrap();
        }
    }
    // A rejected current-time fork is a separate signed block and must preserve all journal rows.
    let before = reputation_policy_state_snapshot(chain.state().view().world());
    let mut fork = successor.clone();
    fork.predecessor_policy_digest = Some([0xA9; 32]);
    commit_reputation_instruction(
        &mut chain,
        TEST_NOW_MS + 1_000,
        1,
        SetSorafsReputationJournalAuthorityPolicy::new(fork),
    )
    .expect_err("fork must not partially publish policy rows");
    assert_eq!(
        reputation_policy_state_snapshot(chain.state().view().world()),
        before
    );
    assert_eq!(take_policy_activation_count(&mut chain), 0);
    commit_reputation_instruction(
        &mut chain,
        TEST_NOW_MS + 2_000,
        1,
        SetSorafsReputationJournalAuthorityPolicy::new(successor.clone()),
    )
    .expect("activate later cutover");
    let active = read_active_policy(chain.state().view().world())
        .unwrap()
        .unwrap();
    assert_eq!(active.policy_digest, successor_digest);
    assert_eq!(active.activated_at_unix_ms, TEST_NOW_MS + 2_000);
    assert_eq!(take_policy_activation_count(&mut chain), 1);
    let before = reputation_policy_state_snapshot(chain.state().view().world());
    let expired = por_entry_at(
        &authority,
        provider_id,
        first_digest,
        0xD3,
        TEST_NOW_MS + 2_000,
    );
    let results = commit_reputation_instructions(
        &mut chain,
        TEST_NOW_MS + 3_000,
        vec![
            (
                1,
                AppendSorafsPorReputationJournalEntry::new(first.clone()).into(),
            ),
            (
                1,
                AppendSorafsPorReputationJournalEntry::new(delayed.clone()).into(),
            ),
            (
                1,
                SetSorafsReputationJournalAuthorityPolicy::new(first_policy).into(),
            ),
            (
                1,
                AppendSorafsPorReputationJournalEntry::new(expired).into(),
            ),
        ],
    );
    assert!(results[..3].iter().all(Result::is_ok), "{results:?}");
    assert_instruction_message(
        results[3]
            .as_ref()
            .expect_err("exact cutover belongs to successor"),
        "outside its recorder-policy activation interval",
    );
    assert_eq!(
        reputation_policy_state_snapshot(chain.state().view().world()),
        before
    );
    assert_eq!(
        take_policy_activation_count(&mut chain),
        0,
        "historical replay neither rewrites nor emits"
    );
    let current = por_entry_at(
        &authority,
        provider_id,
        successor_digest,
        0xD4,
        TEST_NOW_MS + 2_000,
    );
    commit_reputation_instruction(
        &mut chain,
        TEST_NOW_MS + 4_000,
        1,
        AppendSorafsPorReputationJournalEntry::new(current),
    )
    .expect("append successor material");
    assert_eq!(
        validate_journal_head(chain.state().view().world())
            .unwrap()
            .unwrap()
            .last_sequence,
        3
    );
    let page = FindSorafsReputationJournalEvents::new(None, None, 8)
        .execute(&chain.state().view())
        .expect("query retained events after committed cutover");
    assert_eq!(page.events.len(), 3);
    assert_eq!(page.events[0].entry, first);
    assert_eq!(page.events[1].entry, delayed);
    assert_eq!(
        page.events[2].entry.authority_policy_digest,
        successor_digest
    );
}

fn reputation_policy_state_snapshot(world: &impl WorldReadOnly) -> Vec<(StatePath, Vec<u8>)> {
    world
        .smart_contract_state()
        .iter()
        .filter(|(key, _)| key.to_string().starts_with("sorafs_reputation_"))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

fn take_policy_activation_count(chain: &mut CertifiedTestChain) -> usize {
    chain
        .take_events()
        .expect("native event delivery")
        .iter()
        .filter(|event| {
            matches!(event, iroha_data_model::events::EventBox::Data(data)
            if matches!(data.as_ref(), DataEvent::Sorafs(SorafsGatewayEvent::ReputationJournal(
                SorafsReputationJournalEvent::PolicyActivated(_)))))
        })
        .count()
}
