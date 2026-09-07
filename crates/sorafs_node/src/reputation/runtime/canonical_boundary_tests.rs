// Fixed V1 runtime checkpoints, exact retention ceilings, and restart admission.

#[test]
fn journal_checkpoint_byte_ceiling_binary_search_preserves_exact_replay_tombstones() {
    let mut policy = producer_policy();
    policy.max_attempts = 1;
    policy.checkpoint_max_bytes = REPUTATION_RUNTIME_MIN_CHECKPOINT_BYTES_V1;
    let policy_digest = policy.digest().expect("producer policy digest");
    let (_temp, outbox) = initialized_outbox(policy.clone());
    let authority_policy = policy.authority_policy.clone();
    let observed_outcome = verified_por(0x31);
    let observed_entry = ReputationJournalEntryV1::try_new(
        provider(6),
        authority_policy
            .canonical_digest()
            .expect("authority policy digest"),
        authority_policy.por_recorder_authority,
        observed_outcome.decided_at_unix_ms,
        None,
        ReputationJournalPayloadV1::PorTerminal(observed_outcome),
    )
    .expect("observed journal entry");
    outbox
        .reconcile_finalized_journal_page(terminal_page(
            10,
            [0xD3; 32],
            FINALIZED_AT_MS.saturating_add(100),
            vec![finalized_event(1, 10, [0xD3; 32], 0, observed_entry)],
        ))
        .expect("retain observed tombstone");
    let token_producer = token_producer(Arc::clone(&outbox));
    let mut head_event_id = ReputationJournalEventIdV1::ZERO;
    for sequence in 1_u8..=17 {
        let mut token = counted_token(0x97, sequence);
        token.binding.gateway_sequence = u64::from(sequence);
        head_event_id = admission_event_id(
            &token_producer,
            token,
            "token admission",
            "unexpected token admission",
        );
    }
    let completed_outcome = shifted_por(0x32, 50, true);
    let before_source_check = outbox.state.lock().unwrap().checkpoint.clone();
    assert!(matches!(
        por_producer(Arc::clone(&outbox)).enqueue_terminal(provider(7), completed_outcome.clone()),
        Err(ReputationRuntimeError::FinalizedFork)
    ));
    assert_eq!(outbox.state.lock().unwrap().checkpoint, before_source_check);
    // The default source fixture uses a different hash and time at height 10.
    // Bind the real adapter to this fixture's observed anchor, then advance the
    // query to the block that commits the local completion before the next row.
    let por_producer = por_producer_for(
        Arc::clone(&outbox),
        source_query_script([
            Ok(source_view_at(10, [0xD3; 32], FINALIZED_AT_MS + 100, None)),
            Ok(source_view_at(11, [0xD4; 32], FINALIZED_AT_MS + 200, None)),
        ]),
    );
    let completed_event_id = por_event_id(&por_producer, 7, completed_outcome, "local PoR row");
    outbox
        .begin_submission(completed_event_id, finalized_id(10, [0xD3; 32]))
        .expect("begin local PoR submission");
    outbox
        .acknowledge_committed(completed_event_id, committed_identity(2, 11, [0xD4; 32], 0))
        .expect("retain completed tombstone");
    let dead_letter_event_id = por_event_id(
        &por_producer,
        8,
        shifted_por(0x33, 100, true),
        "dead-letter PoR row",
    );
    outbox
        .begin_submission(dead_letter_event_id, finalized_id(11, [0xD4; 32]))
        .expect("begin terminal PoR submission");
    assert!(matches!(
        outbox
            .record_not_submitted(dead_letter_event_id, [0xE3; 32])
            .expect("dead-letter failed PoR"),
        ReputationJournalDeliveryOutcomeV1::DeadLettered { attempts: 1 }
    ));
    let original = outbox
        .state
        .lock()
        .expect("producer state")
        .checkpoint
        .clone();
    assert_eq!(original.observed.len(), 1);
    assert_eq!(original.completed.len(), 1);
    assert_eq!(original.dead_letters.len(), 1);
    assert_eq!(original.stream_token_gateway_admissions.len(), 17);
    let original_pending = original.pending.clone();
    let original_completed = original.completed.clone();
    let original_observed = original.observed.clone();
    let original_dead_letters = original.dead_letters.clone();
    let original_heads = original.stream_token_gateway_heads.clone();
    // Derive the minimal fitting prefix independently from the production
    // search, plan, and eviction helpers. The fixture fixes sequences
    // 1..=16 as evictable oldest-to-newest and sequence 17 as the head.
    const EXPECTED_PREFIX: usize = 9;
    let expected_eviction_order = (1_u64..17)
        .map(|sequence| {
            original
                .stream_token_gateway_admissions
                .iter()
                .find(|admission| admission.binding.gateway_sequence == sequence)
                .expect("hard-coded non-head admission")
                .event_id
        })
        .collect::<Vec<_>>();
    let mut iterative = original.clone();
    let mut iterative_lengths = vec![canonical_test_frame(&iterative).len()];
    let mut expected = None;
    for (index, event_id) in expected_eviction_order.iter().copied().enumerate() {
        let position = iterative
            .stream_token_gateway_admissions
            .iter()
            .position(|admission| admission.event_id == event_id)
            .expect("hard-coded admission remains");
        iterative.stream_token_gateway_admissions.remove(position);
        iterative_lengths.push(canonical_test_frame(&iterative).len());
        if index + 1 == EXPECTED_PREFIX {
            expected = Some(iterative.clone());
        }
    }
    assert_eq!(iterative.stream_token_gateway_admissions.len(), 1);
    assert!(
        iterative_lengths
            .windows(2)
            .all(|adjacent| adjacent[0] > adjacent[1]),
        "each complete admission removal must strictly reduce the frame"
    );
    let expected = expected.expect("capture independently compacted checkpoint");
    let ceiling =
        u64::try_from(iterative_lengths[EXPECTED_PREFIX]).expect("fixture length fits u64");
    assert!(
        u64::try_from(iterative_lengths[EXPECTED_PREFIX - 1]).expect("fixture length fits u64")
            > ceiling,
        "the preceding prefix must remain over the selected ceiling"
    );
    let expected_bytes = canonical_test_frame(&expected);
    let expected_seal =
        ReputationJournalSealedCheckpointRecordV1::new(1, None, expected_bytes.clone())
            .expect("construct canonical checkpoint seal");
    let expected_seal_bytes = canonical_test_frame(&expected_seal);
    let original_bytes = canonical_test_frame(&original);
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"reputation-test-journal-checkpoint-v1");
    hasher.update(&u64::try_from(original_bytes.len()).unwrap().to_le_bytes());
    hasher.update(&original_bytes);
    let expected_hash = *hasher.finalize().as_bytes();
    let mut saw_alternate_frame = false;
    for flags in supported_layouts() {
        let _layout = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(
            hash_canonical(b"reputation-test-journal-checkpoint-v1", &original).unwrap(),
            expected_hash
        );
        let eviction_plan = stream_token_admission_eviction_plan(&original);
        assert_eq!(eviction_plan, expected_eviction_order);
        let search = smallest_stream_token_admission_eviction_prefix(
            &original,
            &eviction_plan,
            ceiling,
            iterative_lengths[0],
        )
        .expect("find smallest fitting admission prefix");
        assert_eq!(search.prefix, EXPECTED_PREFIX);
        let mut ceiling_log2 = 0;
        let mut covered = 1;
        while covered < eviction_plan.len() {
            covered *= 2;
            ceiling_log2 += 1;
        }
        assert!(
            search.probes <= ceiling_log2 + 1,
            "full-plan qualification plus binary search must be logarithmic"
        );
        let (bounded, bounded_bytes) =
            encode_bounded_journal_checkpoint(original.clone(), &policy, policy_digest, ceiling)
                .expect("evict the independently minimal admission prefix");
        assert_eq!(bounded, expected);
        assert_eq!(
            bounded_bytes.len(),
            norito::canonical_frame_len(&bounded).expect("measure exact bounded frame")
        );
        assert_eq!(bounded_bytes, expected_bytes, "caller layout {flags:#04x}");
        assert_eq!(bounded.pending, original_pending);
        assert_eq!(bounded.completed, original_completed);
        assert_eq!(bounded.observed, original_observed);
        assert_eq!(bounded.dead_letters, original_dead_letters);
        assert_eq!(bounded.stream_token_gateway_heads, original_heads);
        let head = bounded
            .stream_token_gateway_heads
            .first()
            .expect("gateway head");
        assert_eq!(head.event_id, head_event_id);
        assert!(
            bounded
                .stream_token_gateway_admissions
                .iter()
                .any(|admission| admission.binding == head.binding
                    && admission.event_id == head.event_id),
            "the canonical head admission must remain pinned"
        );
        assert_eq!(
            decode_journal_checkpoint(&bounded_bytes, &policy, policy_digest)
                .expect("decode bounded checkpoint"),
            bounded
        );
        let alternate_bytes = norito::to_bytes(&bounded).unwrap();
        if alternate_bytes != expected_bytes {
            saw_alternate_frame = true;
            assert!(matches!(
                decode_journal_checkpoint(&alternate_bytes, &policy, policy_digest),
                Err(ReputationRuntimeError::InvalidCheckpoint)
            ));
        }
        let seal = ReputationJournalSealedCheckpointRecordV1::new(1, None, bounded_bytes)
            .expect("seal the exact minimal retained prefix");
        assert_eq!(seal, expected_seal);
        assert_eq!(
            seal.to_canonical_bytes(ceiling).unwrap(),
            expected_seal_bytes
        );
        assert_eq!(
            ReputationJournalSealedCheckpointRecordV1::from_canonical_bytes(
                &expected_seal_bytes,
                ceiling,
            )
            .expect("reload the exact canonical external seal"),
            expected_seal
        );
        let alternate_seal = norito::to_bytes(&seal).unwrap();
        if alternate_seal != expected_seal_bytes {
            assert!(matches!(
                ReputationJournalSealedCheckpointRecordV1::from_canonical_bytes(
                    &alternate_seal,
                    ceiling,
                ),
                Err(ReputationRuntimeError::InvalidSealedCheckpoint)
            ));
        }
        assert_eq!(norito::core::get_decode_flags(), flags);
    }
    assert!(saw_alternate_frame);
    let irreducible = iterative;
    let mut irreducible_probe = irreducible.clone();
    assert!(!evict_oldest_non_head_stream_token_admission(
        &mut irreducible_probe
    ));
    assert_eq!(irreducible.pending, original_pending);
    assert_eq!(irreducible.completed, original_completed);
    assert_eq!(irreducible.observed, original_observed);
    assert_eq!(irreducible.dead_letters, original_dead_letters);
    assert_eq!(irreducible.stream_token_gateway_heads, original_heads);
    let irreducible_ceiling = u64::try_from(canonical_test_frame(&irreducible).len())
        .expect("fixture length fits u64")
        .saturating_sub(1);
    for flags in supported_layouts() {
        let _layout = norito::core::DecodeFlagsGuard::enter(flags);
        assert!(matches!(
            encode_bounded_journal_checkpoint(
                original.clone(),
                &policy,
                policy_digest,
                irreducible_ceiling,
            ),
            Err(ReputationRuntimeError::CheckpointTooLarge)
        ));
        assert_eq!(norito::core::get_decode_flags(), flags);
    }
    assert_eq!(original.completed, original_completed);
    assert_eq!(original.observed, original_observed);
}

#[test]
fn committed_snapshot_history_is_bounded_ordered_and_restart_safe() {
    let trust = trust_policy();
    let policy = publication_policy(&trust);
    let policy_digest = policy.digest().expect("publication policy digest");
    let mut checkpoint = ReputationPublicationCheckpointV1::empty(policy_digest);
    let mut previous_snapshot_id = None;
    let mut previous_governance_readback = None;
    for offset in 0_u8..3 {
        let snapshot_id = [0xC0 + offset; 16];
        let signed = signed_snapshot(
            &trust,
            snapshot_id,
            previous_snapshot_id,
            FINALIZED_AT_MS / 1_000 + u64::from(offset),
        );
        let material_digest = [0xD0 + offset; 32];
        let signed_result_digest = signed_result_digest(&signed).expect("signed result digest");
        let sequence = u64::from(offset) + 1;
        let (acknowledgement, governance_readback) = readback_after(
            &policy,
            sequence,
            material_digest,
            signed_result_digest,
            &signed,
            previous_governance_readback.as_ref(),
        );
        let current_governance_readback = governance_readback
            .reconstruct_readback(&signed)
            .expect("reconstruct Governance DAG readback");
        let committed = ReputationCommittedSnapshotV1 {
            sequence,
            material_digest,
            signed_result_digest,
            signed_result: signed,
            governance_acknowledgement: acknowledgement,
        };
        checkpoint
            .commit_authoritative_with_retention_limit(
                committed,
                &policy,
                &trust,
                governance_readback,
                2,
            )
            .expect("commit bounded authoritative snapshot");
        previous_snapshot_id = Some(snapshot_id);
        previous_governance_readback = Some(current_governance_readback);
    }
    assert_eq!(checkpoint.committed_snapshots.len(), 2);
    assert_eq!(checkpoint.committed_governance_readbacks.len(), 2);
    assert_eq!(
        checkpoint
            .committed_snapshots
            .iter()
            .map(|committed| committed.signed_result.snapshot.snapshot_id)
            .collect::<Vec<_>>(),
        vec![[0xC1; 16], [0xC2; 16]]
    );
    assert_eq!(
        checkpoint
            .committed_read
            .events
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>(),
        vec![2, 3]
    );
    assert_eq!(
        checkpoint
            .committed_read
            .latest
            .as_ref()
            .map(|committed| committed.signed_result.snapshot.snapshot_id),
        Some([0xC2; 16])
    );
    assert!(
        checkpoint
            .committed_snapshots
            .iter()
            .all(|committed| committed.signed_result.snapshot.snapshot_id != [0xC0; 16]),
        "the oldest identifier must be unavailable after bounded eviction"
    );
    validate_publication_checkpoint(&checkpoint, &policy, policy_digest, &trust)
        .expect("validate bounded history");
    let mut expected_byte_bounded = checkpoint.clone();
    assert!(expected_byte_bounded.evict_oldest_committed());
    let expected_byte_bounded_bytes = canonical_test_frame(&expected_byte_bounded);
    let byte_ceiling =
        u64::try_from(expected_byte_bounded_bytes.len()).expect("fixture length fits u64");
    let canonical = canonical_test_frame(&checkpoint);
    let mut saw_alternate_frame = false;
    for flags in supported_layouts() {
        let _layout = norito::core::DecodeFlagsGuard::enter(flags);
        let (byte_bounded, byte_bounded_bytes) = encode_bounded_publication_checkpoint(
            checkpoint.clone(),
            &policy,
            policy_digest,
            &trust,
            byte_ceiling,
        )
        .expect("evict the oldest snapshot to meet the byte ceiling");
        assert_eq!(byte_bounded, expected_byte_bounded);
        assert_eq!(byte_bounded_bytes, expected_byte_bounded_bytes);
        assert_eq!(
            decode_publication_checkpoint(&byte_bounded_bytes, &policy, policy_digest, &trust,)
                .expect("restore byte-bounded history"),
            expected_byte_bounded
        );
        assert!(matches!(
            encode_bounded_publication_checkpoint(
                checkpoint.clone(),
                &policy,
                policy_digest,
                &trust,
                byte_ceiling - 1,
            ),
            Err(ReputationRuntimeError::CheckpointTooLarge)
        ));
        assert_eq!(
            decode_publication_checkpoint(&canonical, &policy, policy_digest, &trust)
                .expect("restore bounded history"),
            checkpoint
        );
        let (uncompacted, uncompacted_bytes) = encode_bounded_publication_checkpoint(
            checkpoint.clone(),
            &policy,
            policy_digest,
            &trust,
            u64::try_from(canonical.len()).unwrap(),
        )
        .expect("canonical full history fits its exact byte ceiling");
        assert_eq!(uncompacted, checkpoint);
        assert_eq!(uncompacted_bytes, canonical);
        let alternate_bytes = norito::to_bytes(&checkpoint).unwrap();
        if alternate_bytes != canonical {
            saw_alternate_frame = true;
            assert!(matches!(
                decode_publication_checkpoint(&alternate_bytes, &policy, policy_digest, &trust),
                Err(ReputationRuntimeError::InvalidCheckpoint)
            ));
        }
        assert_eq!(norito::core::get_decode_flags(), flags);
    }
    assert!(saw_alternate_frame);
}
