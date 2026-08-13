// PoR reputation work and authenticated replay-archive regressions.
#[test]
fn reputation_work_and_ack_checkpoint_roundtrip_byte_identically() {
    let source = PorTracker::default();
    let first_challenge = sample_challenge();
    let first_proof = sample_proof(&first_challenge);
    source.record_challenge(&first_challenge).unwrap();
    source
        .record_proof(&first_proof, &sample_provider_key())
        .unwrap();
    let first = source
        .record_verdict_with(
            &sample_verdict(&first_challenge, first_proof.proof_digest()),
            &sample_auditor_keys(),
            1,
            |_| panic!("success must not invoke repair handoff"),
        )
        .unwrap()
        .reputation_work;
    source
        .acknowledge_reputation_terminal(first.sequence, first.work_digest)
        .unwrap();
    let second_challenge = next_challenge(&first_challenge, 1);
    let second_proof = sample_proof(&second_challenge);
    source.record_challenge(&second_challenge).unwrap();
    source
        .record_proof(&second_proof, &sample_provider_key())
        .unwrap();
    let second = source
        .record_verdict_with(
            &sample_verdict(&second_challenge, second_proof.proof_digest()),
            &sample_auditor_keys(),
            1,
            |_| panic!("success must not invoke repair handoff"),
        )
        .unwrap()
        .reputation_work;
    let checkpoint = source.checkpoint();
    let canonical = norito::to_bytes(&checkpoint).expect("encode source checkpoint");
    let restored = PorTracker::default();
    restored
        .restore_checkpoint(checkpoint)
        .expect("restore canonical checkpoint");
    assert_eq!(
        norito::to_bytes(&restored.checkpoint()).expect("encode restored checkpoint"),
        canonical
    );
    assert_eq!(
        restored
            .acknowledge_reputation_terminal(first.sequence, first.work_digest)
            .expect("latest acknowledgement remains exactly replayable"),
        PorReputationTerminalAckOutcomeV1::ExactReplay
    );
    assert_eq!(
        restored.next_reputation_terminal_work().unwrap(),
        Some(second)
    );
    assert_eq!(restored.pending_reputation_terminal_count(), 1);
}
#[test]
fn tracker_refuses_bounded_status_history_exhaustion() {
    let tracker = PorTracker::with_entry_limit(1);
    let first = sample_challenge();
    let second = next_challenge(&first, 1);
    tracker.record_challenge(&first).unwrap();
    assert!(matches!(
        tracker.record_challenge(&second),
        Err(PorTrackerError::PendingRetentionExhausted { limit: 1 })
    ));
    let proof = sample_proof(&first);
    tracker
        .record_proof(&proof, &sample_provider_key())
        .unwrap();
    tracker
        .record_verdict(
            &sample_verdict(&first, proof.proof_digest()),
            &sample_auditor_keys(),
            1,
        )
        .unwrap();
    assert!(matches!(
        tracker.record_challenge(&second),
        Err(PorTrackerError::PendingRetentionExhausted { limit: 1 })
    ));
    let retained = tracker
        .next_reputation_terminal_work()
        .unwrap()
        .expect("first finalized terminal remains retained");
    tracker
        .acknowledge_reputation_terminal(retained.sequence, retained.work_digest)
        .expect("acknowledge retained terminal");
    assert!(matches!(
        tracker.record_challenge(&second),
        Err(PorTrackerError::PendingRetentionExhausted { limit: 1 })
    ));
    assert!(
        !tracker.contains_challenge(&second.challenge_id),
        "acknowledgement does not erase bounded historical status"
    );
}
#[test]
fn status_generation_overflow_fails_before_each_lifecycle_mutation() {
    let challenge_tracker = PorTracker::default();
    challenge_tracker
        .inner
        .write()
        .expect("tracker lock")
        .status_generation = u64::MAX;
    let challenge = sample_challenge();
    assert!(matches!(
        challenge_tracker.record_challenge(&challenge),
        Err(PorTrackerError::StatusGenerationExhausted)
    ));
    assert!(!challenge_tracker.contains_challenge(&challenge.challenge_id));
    let proof_tracker = PorTracker::default();
    proof_tracker.record_challenge(&challenge).unwrap();
    proof_tracker
        .inner
        .write()
        .expect("tracker lock")
        .status_generation = u64::MAX;
    let proof = sample_proof(&challenge);
    assert!(matches!(
        proof_tracker.record_proof(&proof, &sample_provider_key()),
        Err(PorTrackerError::StatusGenerationExhausted)
    ));
    assert_eq!(proof_tracker.proof_digest(&challenge.challenge_id), None);
    let verdict_tracker = PorTracker::default();
    verdict_tracker.record_challenge(&challenge).unwrap();
    verdict_tracker
        .inner
        .write()
        .expect("tracker lock")
        .status_generation = u64::MAX;
    let mut verdict = sample_verdict(&challenge, [0; 32]);
    verdict.outcome = AuditOutcomeV1::Failed;
    verdict.proof_digest = None;
    verdict.failure_reason = Some("deadline elapsed".to_owned());
    verdict.decided_at = challenge.deadline_at;
    resign_sample_verdict(&mut verdict);
    assert!(matches!(
        verdict_tracker.record_verdict_durable(&verdict, &sample_auditor_keys(), 1),
        Err(PorTrackerError::StatusGenerationExhausted)
    ));
    assert!(verdict_tracker.contains_challenge(&challenge.challenge_id));
    assert!(
        verdict_tracker
            .next_pending_repair_work()
            .unwrap()
            .is_none()
    );
}
#[test]
fn tracker_checkpoint_preserves_pending_proofs_and_finalized_payloads() {
    let source = PorTracker::with_entry_limit(4);
    let finalized = sample_challenge();
    let finalized_proof = sample_proof(&finalized);
    source.record_challenge(&finalized).unwrap();
    source
        .record_proof(&finalized_proof, &sample_provider_key())
        .unwrap();
    source
        .record_verdict(
            &sample_verdict(&finalized, finalized_proof.proof_digest()),
            &sample_auditor_keys(),
            1,
        )
        .unwrap();
    let pending = next_challenge(&finalized, 1);
    let pending_proof = sample_proof(&pending);
    source.record_challenge(&pending).unwrap();
    source
        .record_proof(&pending_proof, &sample_provider_key())
        .unwrap();
    let checkpoint = source.checkpoint();
    let encoded = norito::to_bytes(&checkpoint).unwrap();
    let checkpoint = norito::decode_from_bytes(&encoded).unwrap();
    let restored = PorTracker::with_entry_limit(4);
    restored.restore_checkpoint(checkpoint).unwrap();
    restored
        .record_challenge(&finalized)
        .expect("restored finalized challenge is exactly idempotent");
    let mut conflicting = finalized.clone();
    conflicting.deadline_at = conflicting.deadline_at.saturating_add(1);
    assert!(matches!(
        restored.record_challenge(&conflicting),
        Err(PorTrackerError::ChallengeConflict)
    ));
    restored
        .record_verdict(
            &sample_verdict(&pending, pending_proof.proof_digest()),
            &sample_auditor_keys(),
            1,
        )
        .unwrap();
}
#[test]
fn tracker_projects_each_first_release_proof_lifecycle_stage() {
    let tracker = PorTracker::with_entry_limit(2);
    let challenge = sample_challenge();
    tracker.record_challenge(&challenge).unwrap();
    let awaiting = tracker.status_authority_snapshot().unwrap();
    assert_eq!(
        awaiting.statuses[0].status,
        PorChallengeOutcome::AwaitingProof
    );
    assert!(awaiting.statuses[0].proof_digest.is_none());
    let proof = sample_proof(&challenge);
    tracker
        .record_proof(&proof, &sample_provider_key())
        .unwrap();
    let submitted = tracker.status_authority_snapshot().unwrap();
    assert_eq!(
        submitted.statuses[0].status,
        PorChallengeOutcome::ProofSubmitted
    );
    assert_eq!(
        submitted.statuses[0].proof_digest,
        Some(proof.proof_digest())
    );
    let restored = PorTracker::with_entry_limit(2);
    restored.restore_checkpoint(tracker.checkpoint()).unwrap();
    assert_eq!(
        restored.status_authority_snapshot().unwrap().statuses[0].status,
        PorChallengeOutcome::ProofSubmitted
    );
    restored
        .record_verdict(
            &sample_verdict(&challenge, proof.proof_digest()),
            &sample_auditor_keys(),
            1,
        )
        .unwrap();
    assert_eq!(
        restored.status_authority_snapshot().unwrap().statuses[0].status,
        PorChallengeOutcome::Verified
    );
}
#[test]
fn tracker_checkpoint_rejects_forged_finalized_verdict_signature() {
    let source = PorTracker::default();
    let challenge = sample_challenge();
    let proof = sample_proof(&challenge);
    source.record_challenge(&challenge).unwrap();
    source.record_proof(&proof, &sample_provider_key()).unwrap();
    source
        .record_verdict(
            &sample_verdict(&challenge, proof.proof_digest()),
            &sample_auditor_keys(),
            1,
        )
        .unwrap();
    let mut checkpoint = source.checkpoint();
    checkpoint.finalized[0].verdict.auditor_signatures[0].signature[0] ^= 1;
    let restored = PorTracker::default();
    assert!(matches!(
        restored.restore_checkpoint(checkpoint),
        Err(PorTrackerError::VerdictSignatureInvalid(_))
    ));
}
#[test]
fn tracker_checkpoint_rejects_reputation_sequence_terminal_and_ack_tampering() {
    let source = PorTracker::default();
    let challenge = sample_challenge();
    let proof = sample_proof(&challenge);
    source.record_challenge(&challenge).unwrap();
    source.record_proof(&proof, &sample_provider_key()).unwrap();
    source
        .record_verdict(
            &sample_verdict(&challenge, proof.proof_digest()),
            &sample_auditor_keys(),
            1,
        )
        .unwrap();
    let work = source
        .next_reputation_terminal_work()
        .unwrap()
        .expect("retained terminal");
    source
        .acknowledge_reputation_terminal(work.sequence, work.work_digest)
        .unwrap();
    let checkpoint = source.checkpoint();
    for mutation in 0..3 {
        let mut tampered = checkpoint.clone();
        match mutation {
            0 => tampered.finalized[0].reputation_sequence = 2,
            1 => {
                tampered.finalized[0].reputation_terminal.status = PorTerminalStatusV1::Repaired;
            }
            2 => {
                tampered
                    .acknowledged_reputation_terminal
                    .as_mut()
                    .expect("retained acknowledgement")
                    .work_digest[0] ^= 0x80;
            }
            _ => unreachable!(),
        }
        assert!(
            matches!(
                PorTracker::default().restore_checkpoint(tampered),
                Err(PorTrackerError::InvalidCheckpoint(_))
            ),
            "tamper case {mutation} must fail closed"
        );
    }
}
#[test]
fn authenticated_archive_compaction_is_crash_replay_safe_and_preserves_conflicts() {
    let tracker = PorTracker::with_entry_limit(1);
    let challenge = sample_challenge();
    let proof = sample_proof(&challenge);
    let verdict = sample_verdict(&challenge, proof.proof_digest());
    tracker.record_challenge(&challenge).unwrap();
    tracker
        .record_proof(&proof, &sample_provider_key())
        .unwrap();
    tracker
        .record_verdict(&verdict, &sample_auditor_keys(), 1)
        .unwrap();
    let work = tracker
        .next_reputation_terminal_work()
        .unwrap()
        .expect("retained terminal");
    let archive = MemoryReplayArchive::new(0x91);
    let mut substituted_binding = archive.binding;
    substituted_binding.policy_digest[0] ^= 1;
    assert!(matches!(
        tracker.compact_acknowledged_with_replay_archive(&archive, substituted_binding, 1),
        Err(PorTrackerError::ReplayArchiveBindingMismatch)
    ));
    assert_eq!(
        tracker
            .compact_acknowledged_with_replay_archive(&archive, archive.binding, 1)
            .expect("unacknowledged records are never archived"),
        0
    );
    assert_eq!(archive.append_calls(), 0);
    tracker
        .acknowledge_reputation_terminal(work.sequence, work.work_digest)
        .unwrap();
    let before_compaction = tracker.checkpoint();
    assert_eq!(
        tracker
            .compact_acknowledged_with_replay_archive(&archive, archive.binding, 1)
            .expect("first authenticated compaction"),
        1
    );
    let after_compaction = tracker.checkpoint();
    assert!(after_compaction.finalized.is_empty());
    assert!(after_compaction.replay_archive_receipt.is_some());
    let status_after_compaction = tracker
        .status_authority_snapshot()
        .expect("compacted status remains queryable");
    assert_eq!(status_after_compaction.statuses.len(), 1);
    assert_eq!(
        status_after_compaction.statuses[0].status,
        PorChallengeOutcome::Verified
    );
    // Simulate a crash after the external append but before the node
    // checkpoint. Restoring the old checkpoint must drive an exact append
    // replay, not create a second archive record.
    tracker
        .restore_checkpoint(before_compaction)
        .expect("restore pre-commit node checkpoint");
    assert_eq!(
        tracker
            .compact_acknowledged_with_replay_archive(&archive, archive.binding, 1)
            .expect("retry exact authenticated compaction"),
        1
    );
    assert_eq!(archive.append_calls(), 2);
    assert_eq!(archive.retained_records(), 1);
    assert_eq!(
        norito::to_bytes(&tracker.checkpoint()).unwrap(),
        norito::to_bytes(&after_compaction).unwrap()
    );
    let restored = PorTracker::with_entry_limit(1);
    restored
        .restore_checkpoint(after_compaction.clone())
        .expect("restore compacted checkpoint");
    assert!(matches!(
        restored.record_challenge(&challenge),
        Err(PorTrackerError::ReplayArchiveRequired)
    ));
    restored
        .record_challenge_with_archive_and_bounds(
            &challenge,
            &archive,
            PorFinalizedReplayArchiveProofBoundsV1::production_default(),
        )
        .expect("archived challenge exact replay");
    let mut conflicting_challenge = challenge.clone();
    conflicting_challenge.deadline_at = conflicting_challenge.deadline_at.saturating_add(1);
    assert!(matches!(
        restored.record_challenge_with_archive_and_bounds(
            &conflicting_challenge,
            &archive,
            PorFinalizedReplayArchiveProofBoundsV1::production_default(),
        ),
        Err(PorTrackerError::ChallengeConflict)
    ));
    let replay = restored
        .record_verdict_with_archive_and_bounds(
            &verdict,
            &sample_auditor_keys(),
            1,
            &archive,
            PorFinalizedReplayArchiveProofBoundsV1::production_default(),
            |_| panic!("archived exact replay must not enqueue repair"),
        )
        .expect("archived verdict exact replay");
    assert!(!replay.newly_finalized);
    assert_eq!(replay.reputation_work, work);
    let mut conflicting_verdict = verdict.clone();
    conflicting_verdict.decided_at = conflicting_verdict.decided_at.saturating_add(1);
    resign_sample_verdict(&mut conflicting_verdict);
    assert!(matches!(
        restored.record_verdict_with_archive_and_bounds(
            &conflicting_verdict,
            &sample_auditor_keys(),
            1,
            &archive,
            PorFinalizedReplayArchiveProofBoundsV1::production_default(),
            |_| panic!("conflicting archived replay must not enqueue repair"),
        ),
        Err(PorTrackerError::VerdictConflict)
    ));
    assert!(matches!(
        restored.record_proof_with_archive_and_bounds(
            &proof,
            &sample_provider_key(),
            &archive,
            PorFinalizedReplayArchiveProofBoundsV1::production_default(),
        ),
        Ok(PorProofRecordOutcomeV1::ExactReplay(_))
    ));
    let fresh = next_challenge(&challenge, 1);
    assert_eq!(
        restored
            .record_challenge_with_archive_and_bounds(
                &fresh,
                &archive,
                PorFinalizedReplayArchiveProofBoundsV1::production_default(),
            )
            .expect("rolling retention admits a replacement"),
        PorChallengeRecordOutcomeV1::Inserted
    );
    let fresh_update = restored
        .status_authority_update(fresh.challenge_id)
        .expect("replacement has one exact authority delta");
    assert_eq!(
        fresh_update.removed_challenge_ids,
        vec![challenge.challenge_id]
    );
    assert_eq!(
        restored
            .status_authority_snapshot()
            .expect("bounded projection remains queryable")
            .statuses
            .iter()
            .map(|status| status.challenge_id)
            .collect::<Vec<_>>(),
        vec![fresh.challenge_id]
    );
    let archived_status = match restored
        .record_proof_with_archive_and_bounds(
            &proof,
            &sample_provider_key(),
            &archive,
            PorFinalizedReplayArchiveProofBoundsV1::production_default(),
        )
        .expect("retired archived proof remains exactly replayable")
    {
        PorProofRecordOutcomeV1::ExactReplay(status) => status,
        PorProofRecordOutcomeV1::Inserted => panic!("archived replay cannot insert"),
    };
    let replay_update = restored
        .status_authority_replay_update(archived_status)
        .expect("archived replay produces a same-generation projection no-op");
    assert_eq!(replay_update.generation, fresh_update.generation);
    assert!(replay_update.removed_challenge_ids.is_empty());
    let mut conflicting_proof = proof.clone();
    conflicting_proof.samples[0].chunk_digest[0] ^= 1;
    resign_sample_proof(&mut conflicting_proof);
    assert!(matches!(
        restored.record_proof_with_archive_and_bounds(
            &conflicting_proof,
            &sample_provider_key(),
            &archive,
            PorFinalizedReplayArchiveProofBoundsV1::production_default(),
        ),
        Err(PorTrackerError::DuplicateProof)
    ));
    let mut tampered_acknowledgement = after_compaction.clone();
    tampered_acknowledgement
        .acknowledged_reputation_terminal
        .as_mut()
        .expect("archived acknowledgement")
        .work_digest[0] ^= 1;
    assert!(matches!(
        PorTracker::with_entry_limit(1).restore_checkpoint(tampered_acknowledgement),
        Err(PorTrackerError::InvalidCheckpoint(_))
    ));
    let mut tampered = after_compaction;
    tampered
        .replay_archive_receipt
        .as_mut()
        .expect("archive receipt")
        .signature[0] ^= 1;
    assert!(matches!(
        PorTracker::with_entry_limit(1).restore_checkpoint(tampered),
        Err(PorTrackerError::InvalidReplayArchiveReceipt)
    ));
}
#[test]
fn archive_call_paths_reject_post_call_binding_drift() {
    let tracker = PorTracker::with_entry_limit(2);
    let challenge = sample_challenge();
    let proof = sample_proof(&challenge);
    let verdict = sample_verdict(&challenge, proof.proof_digest());
    tracker.record_challenge(&challenge).unwrap();
    tracker
        .record_proof(&proof, &sample_provider_key())
        .unwrap();
    tracker
        .record_verdict(&verdict, &sample_auditor_keys(), 1)
        .unwrap();
    let work = tracker
        .next_reputation_terminal_work()
        .unwrap()
        .expect("retained terminal");
    tracker
        .acknowledge_reputation_terminal(work.sequence, work.work_digest)
        .unwrap();
    let archive = MemoryReplayArchive::new(0x95);
    let before_compaction = tracker.checkpoint();
    let compaction_drift = BindingDriftReplayArchive::new(&archive);
    assert!(matches!(
        tracker.compact_acknowledged_with_replay_archive(&compaction_drift, archive.binding, 1,),
        Err(PorTrackerError::ReplayArchiveBindingMismatch)
    ));
    assert_eq!(
        tracker.checkpoint(),
        before_compaction,
        "post-append binding drift must roll local compaction state back"
    );
    assert_eq!(
        archive.retained_records(),
        1,
        "the exact external append remains retryable after local rollback"
    );
    tracker
        .compact_acknowledged_with_replay_archive(&archive, archive.binding, 1)
        .expect("exact append replay commits local compaction");
    let compacted = tracker.checkpoint();
    let fresh = next_challenge(&challenge, 1);
    let challenge_drift = BindingDriftReplayArchive::new(&archive);
    assert!(matches!(
        tracker.record_challenge_with_archive_and_bounds(
            &fresh,
            &challenge_drift,
            PorFinalizedReplayArchiveProofBoundsV1::production_default(),
        ),
        Err(PorTrackerError::ReplayArchiveBindingMismatch)
    ));
    assert_eq!(
        tracker.checkpoint(),
        compacted,
        "post-lookup binding drift must not admit an absent challenge"
    );
    let verdict_drift = BindingDriftReplayArchive::new(&archive);
    assert!(matches!(
        tracker.record_verdict_with_archive_and_bounds(
            &verdict,
            &sample_auditor_keys(),
            1,
            &verdict_drift,
            PorFinalizedReplayArchiveProofBoundsV1::production_default(),
            |_| panic!("archived exact replay must not enqueue repair"),
        ),
        Err(PorTrackerError::ReplayArchiveBindingMismatch)
    ));
    assert_eq!(
        tracker.checkpoint(),
        compacted,
        "post-lookup binding drift must not return an archived verdict"
    );
}
#[test]
fn archive_compaction_requires_authoritative_head_installation() {
    let tracker = PorTracker::with_entry_limit(1);
    let challenge = sample_challenge();
    let proof = sample_proof(&challenge);
    tracker.record_challenge(&challenge).unwrap();
    tracker
        .record_proof(&proof, &sample_provider_key())
        .unwrap();
    let work = tracker
        .record_verdict_with(
            &sample_verdict(&challenge, proof.proof_digest()),
            &sample_auditor_keys(),
            1,
            |_| panic!("success must not invoke repair handoff"),
        )
        .unwrap()
        .reputation_work;
    tracker
        .acknowledge_reputation_terminal(work.sequence, work.work_digest)
        .unwrap();
    let archive = MemoryReplayArchive::new(0x96);
    let stale_head_archive = StaleHeadReplayArchive { inner: &archive };
    let before_compaction = tracker.checkpoint();
    assert!(matches!(
        tracker.compact_acknowledged_with_replay_archive(&stale_head_archive, archive.binding, 1,),
        Err(PorTrackerError::ReplayArchiveHeadRollback)
    ));
    assert_eq!(
        tracker.checkpoint(),
        before_compaction,
        "a signed append without authoritative head readback must preserve local replay state"
    );
    assert_eq!(
        archive.retained_records(),
        1,
        "the externally committed record remains available for exact retry"
    );
    assert_eq!(
        tracker
            .compact_acknowledged_with_replay_archive(&archive, archive.binding, 1)
            .expect("authoritative exact-append retry"),
        1
    );
    assert!(tracker.checkpoint().finalized.is_empty());
    assert_eq!(
        tracker.checkpoint().replay_archive_receipt,
        archive.current_head().expect("authoritative archive head")
    );
}
#[test]
fn archive_readback_requires_successor_chain_to_checkpoint_head() {
    let tracker = PorTracker::with_entry_limit(2);
    let first_challenge = sample_challenge();
    let first_proof = sample_proof(&first_challenge);
    tracker.record_challenge(&first_challenge).unwrap();
    tracker
        .record_proof(&first_proof, &sample_provider_key())
        .unwrap();
    let first_work = tracker
        .record_verdict_with(
            &sample_verdict(&first_challenge, first_proof.proof_digest()),
            &sample_auditor_keys(),
            1,
            |_| panic!("success must not invoke repair handoff"),
        )
        .unwrap()
        .reputation_work;
    let second_challenge = next_challenge(&first_challenge, 1);
    let second_proof = sample_proof(&second_challenge);
    tracker.record_challenge(&second_challenge).unwrap();
    tracker
        .record_proof(&second_proof, &sample_provider_key())
        .unwrap();
    let second_work = tracker
        .record_verdict_with(
            &sample_verdict(&second_challenge, second_proof.proof_digest()),
            &sample_auditor_keys(),
            1,
            |_| panic!("success must not invoke repair handoff"),
        )
        .unwrap()
        .reputation_work;
    tracker
        .acknowledge_reputation_terminal(first_work.sequence, first_work.work_digest)
        .unwrap();
    tracker
        .acknowledge_reputation_terminal(second_work.sequence, second_work.work_digest)
        .unwrap();
    let archive = MemoryReplayArchive::new(0x92);
    let pre_archive_checkpoint = tracker.checkpoint();
    assert_eq!(
        tracker
            .compact_acknowledged_with_replay_archive(&archive, archive.binding, 1)
            .expect("archive first acknowledged record"),
        1
    );
    let first_archive_checkpoint = tracker.checkpoint();
    assert_eq!(
        tracker
            .compact_acknowledged_with_replay_archive(&archive, archive.binding, 1)
            .expect("archive second acknowledged record"),
        1
    );
    let fully_archived_checkpoint = tracker.checkpoint();
    let checkpoint_head = fully_archived_checkpoint
        .replay_archive_receipt
        .expect("checkpoint-pinned archive head");
    let PorFinalizedReplayArchiveLookupV1::Found(readback) = archive
        .lookup(
            first_challenge.challenge_id,
            checkpoint_head,
            PorFinalizedReplayArchiveProofBoundsV1::production_default(),
        )
        .expect("authenticated archive lookup")
    else {
        panic!("first archived record must be present");
    };
    assert_eq!(readback.successor_receipts.len(), 1);
    readback
        .record
        .validate()
        .expect("canonical archived record");
    readback
        .receipt
        .validate()
        .expect("canonical signed receipt");
    assert_eq!(readback.receipt.binding(), archive.binding);
    assert_eq!(readback.receipt.reputation_sequence(), first_work.sequence);
    assert_eq!(
        readback.receipt.challenge_id(),
        first_challenge.challenge_id
    );
    assert_eq!(
        readback.receipt.record_digest(),
        readback.record.record_digest().expect("record digest")
    );
    assert_eq!(
        readback.receipt.reputation_work_digest(),
        first_work.work_digest
    );
    assert_eq!(readback.receipt.previous_head_digest(), None);
    assert_ne!(readback.receipt.signature(), [0; 64]);
    readback
        .validate_at_checkpoint(
            archive.binding,
            checkpoint_head,
            PorFinalizedReplayArchiveProofBoundsV1::production_default(),
        )
        .expect("contiguous signed successor chain reaches pinned head");
    let mut truncated = readback.clone();
    truncated.successor_receipts.clear();
    assert!(matches!(
        truncated.validate_at_checkpoint(
            archive.binding,
            checkpoint_head,
            PorFinalizedReplayArchiveProofBoundsV1::production_default(),
        ),
        Err(PorTrackerError::InvalidReplayArchiveReceipt)
    ));
    let mut count_flood = readback.clone();
    count_flood
        .successor_receipts
        .push(*count_flood.successor_receipts.last().expect("successor"));
    assert!(matches!(
        count_flood.validate_at_checkpoint(
            archive.binding,
            checkpoint_head,
            PorFinalizedReplayArchiveProofBoundsV1::try_new(1, u64::MAX).expect("count bound"),
        ),
        Err(PorTrackerError::ReplayArchiveProofLimitExceeded)
    ));
    assert!(matches!(
        readback.validate_at_checkpoint(
            archive.binding,
            checkpoint_head,
            PorFinalizedReplayArchiveProofBoundsV1::try_new(2, 1).expect("byte bound"),
        ),
        Err(PorTrackerError::ReplayArchiveProofLimitExceeded)
    ));
    let framed_bounds =
        PorFinalizedReplayArchiveProofBoundsV1::try_new(1, 1).expect("framed bounds");
    framed_bounds
        .validate_framed_successor_shape(1, 1)
        .expect("declared transport frame is within both bounds");
    assert!(matches!(
        framed_bounds.validate_framed_successor_shape(2, 1),
        Err(PorTrackerError::ReplayArchiveProofLimitExceeded)
    ));
    assert!(matches!(
        framed_bounds.validate_framed_successor_shape(1, 2),
        Err(PorTrackerError::ReplayArchiveProofLimitExceeded)
    ));
    assert!(matches!(
        framed_bounds.validate_framed_successor_shape(1, 0),
        Err(PorTrackerError::ReplayArchiveProofLimitExceeded)
    ));
    assert_eq!(
        archive.lookup(
            first_challenge.challenge_id,
            checkpoint_head,
            PorFinalizedReplayArchiveProofBoundsV1::try_new(1, 1).expect("adapter byte bound"),
        ),
        Err(PorFinalizedReplayArchiveExternalErrorV1::Rejected),
        "the typed in-memory adapter must reject an oversized canonical proof before return"
    );
    let first_record = pre_archive_checkpoint
        .finalized
        .iter()
        .find(|finalized| finalized.reputation_sequence == first_work.sequence)
        .cloned()
        .map(PorFinalizedReplayArchiveRecordV1::from_finalized)
        .expect("first retained archive record");
    assert_eq!(
        archive
            .append(&first_record, None)
            .expect("historical exact append retry remains idempotent"),
        readback.receipt,
        "an exact retry must return its original receipt after successors exist"
    );
    assert_eq!(
        archive.current_head().expect("current archive head"),
        Some(checkpoint_head),
        "a historical exact retry must not roll the monotonic head back"
    );
    let absent_challenge_id = [0xFA; 32];
    let PorFinalizedReplayArchiveLookupV1::Absent(absence) = archive
        .lookup(
            absent_challenge_id,
            checkpoint_head,
            PorFinalizedReplayArchiveProofBoundsV1::production_default(),
        )
        .expect("signed absence at current head")
    else {
        panic!("unknown challenge must return signed absence");
    };
    absence
        .validate_at_checkpoint(archive.binding, absent_challenge_id, checkpoint_head)
        .expect("absence binds the exact checkpoint head");
    assert_eq!(absence.binding(), archive.binding);
    assert_eq!(absence.challenge_id(), absent_challenge_id);
    assert_eq!(absence.checkpoint_head(), checkpoint_head);
    assert_ne!(absence.signature(), [0; 64]);
    assert!(matches!(
        absence.validate_at_checkpoint(archive.binding, absent_challenge_id, readback.receipt,),
        Err(PorTrackerError::InvalidReplayArchiveAbsenceProof)
    ));
    assert_eq!(
        archive.lookup(
            absent_challenge_id,
            readback.receipt,
            PorFinalizedReplayArchiveProofBoundsV1::production_default(),
        ),
        Err(PorFinalizedReplayArchiveExternalErrorV1::Rejected),
        "the adapter must reject a stale expected checkpoint head"
    );
    let restored_ancestor = PorTracker::with_entry_limit(2);
    restored_ancestor
        .restore_checkpoint(first_archive_checkpoint.clone())
        .expect("restore first archive head");
    assert!(
        restored_ancestor
            .reconcile_restored_replay_archive_head(
                &archive,
                archive.binding,
                PorFinalizedReplayArchiveProofBoundsV1::production_default(),
            )
            .expect("current head proves and reconciles the exact retained successor")
    );
    assert_eq!(
        restored_ancestor.checkpoint(),
        fully_archived_checkpoint,
        "startup reconciliation must advance the local checkpoint to the proved prefix"
    );
    let restored_empty_head = PorTracker::with_entry_limit(2);
    restored_empty_head
        .restore_checkpoint(pre_archive_checkpoint)
        .expect("restore acknowledged local prefix before its first archive append");
    assert!(
        restored_empty_head
            .reconcile_restored_replay_archive_head(
                &archive,
                archive.binding,
                PorFinalizedReplayArchiveProofBoundsV1::production_default(),
            )
            .expect("live first-append crash window has an exact acknowledged local intent")
    );
    assert_eq!(
        restored_empty_head.checkpoint(),
        fully_archived_checkpoint,
        "the exact live prefix must be compacted locally without a format-level intent"
    );
    let mut insufficient_ack_checkpoint = first_archive_checkpoint;
    insufficient_ack_checkpoint.acknowledged_reputation_terminal =
        Some(PorReputationTerminalAckV1 {
            sequence: first_work.sequence,
            work_digest: readback.receipt.reputation_work_digest(),
        });
    let insufficient_ack = PorTracker::with_entry_limit(2);
    insufficient_ack
        .restore_checkpoint(insufficient_ack_checkpoint)
        .expect("restore a checkpoint acknowledging only its existing archive head");
    assert!(matches!(
        insufficient_ack.reconcile_restored_replay_archive_head(
            &archive,
            archive.binding,
            PorFinalizedReplayArchiveProofBoundsV1::production_default(),
        ),
        Err(PorTrackerError::ReplayArchiveHeadRollback)
    ));
    let fresh_tracker = PorTracker::with_entry_limit(2);
    assert!(matches!(
        fresh_tracker.reconcile_restored_replay_archive_head(
            &archive,
            archive.binding,
            PorFinalizedReplayArchiveProofBoundsV1::production_default(),
        ),
        Err(PorTrackerError::ReplayArchiveHeadRollback)
    ));
    assert!(
        !tracker
            .reconcile_restored_replay_archive_head(
                &archive,
                archive.binding,
                PorFinalizedReplayArchiveProofBoundsV1::production_default(),
            )
            .expect("an exact restored head needs no reconciliation")
    );
    let latest_head = archive
        .state
        .lock()
        .expect("archive state")
        .latest_head
        .expect("latest head");
    archive.state.lock().expect("archive state").latest_head = Some(readback.receipt.head_digest());
    assert!(matches!(
        tracker.reconcile_restored_replay_archive_head(
            &archive,
            archive.binding,
            PorFinalizedReplayArchiveProofBoundsV1::production_default(),
        ),
        Err(PorTrackerError::ReplayArchiveHeadRollback)
    ));
    archive.state.lock().expect("archive state").latest_head = None;
    assert!(matches!(
        tracker.reconcile_restored_replay_archive_head(
            &archive,
            archive.binding,
            PorFinalizedReplayArchiveProofBoundsV1::production_default(),
        ),
        Err(PorTrackerError::ReplayArchiveHeadRollback)
    ));
    archive.state.lock().expect("archive state").latest_head = Some(latest_head);
}
// Textual inclusion preserves the original PoR test-module paths.
