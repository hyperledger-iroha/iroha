#[test]
fn tracker_handles_failure_verdict() {
    let tracker = PorTracker::default();
    let mut challenge = sample_challenge();
    challenge.sample_count = 1;
    challenge.sample_indices = vec![0];
    tracker.record_challenge(&challenge).unwrap();
    let mut verdict = sample_verdict(&challenge, [1; 32]);
    verdict.proof_digest = None;
    verdict.outcome = AuditOutcomeV1::Failed;
    verdict.failure_reason = Some("timeout".to_string());
    verdict.decided_at = challenge.deadline_at;
    resign_sample_verdict(&mut verdict);
    let stats = tracker
        .record_verdict(&verdict, &sample_auditor_keys(), 1)
        .unwrap();
    assert_eq!(
        stats,
        PorVerdictStats {
            success_samples: 0,
            failed_samples: 1
        }
    );
}

#[test]
fn successful_and_repaired_verdicts_never_invoke_repair_handoff() {
    for (index, outcome) in [AuditOutcomeV1::Success, AuditOutcomeV1::Repaired]
        .into_iter()
        .enumerate()
    {
        let tracker = PorTracker::default();
        let challenge = next_challenge(&sample_challenge(), index as u64);
        let proof = sample_proof(&challenge);
        tracker.record_challenge(&challenge).unwrap();
        tracker
            .record_proof(&proof, &sample_provider_key())
            .unwrap();
        let mut verdict = sample_verdict(&challenge, proof.proof_digest());
        verdict.outcome = outcome;
        verdict.failure_reason = (outcome == AuditOutcomeV1::Repaired)
            .then(|| "repair verification succeeded".to_owned());
        resign_sample_verdict(&mut verdict);

        let transition = tracker
            .record_verdict_with(&verdict, &sample_auditor_keys(), 1, |_| {
                panic!("non-failed verdict must not invoke repair handoff")
            })
            .expect("non-failed terminal verdict");
        assert_eq!(transition.repair_task_id, None);
    }
}

#[test]
fn tracker_detects_mismatched_proof() {
    let tracker = PorTracker::default();
    let challenge = sample_challenge();
    tracker.record_challenge(&challenge).unwrap();
    let mut proof = sample_proof(&challenge);
    proof.manifest_digest = [99; 32];
    resign_sample_proof(&mut proof);
    let err = tracker
        .record_proof(&proof, &sample_provider_key())
        .unwrap_err();
    assert!(matches!(err, PorTrackerError::MismatchManifest));
    assert_eq!(tracker.proof_digest(&challenge.challenge_id), None);

    tracker
        .record_proof(&sample_proof(&challenge), &sample_provider_key())
        .expect("mismatched proof must not consume the challenge");
}

#[test]
fn tracker_rejects_wrong_sample_coverage_and_late_or_predated_proofs() {
    let challenge = sample_challenge();
    for mutation in 0..3 {
        let tracker = PorTracker::default();
        tracker.record_challenge(&challenge).unwrap();
        let mut proof = sample_proof(&challenge);
        match mutation {
            0 => proof.samples.swap(0, 1),
            1 => proof.submitted_at = challenge.issued_at - 1,
            2 => proof.submitted_at = challenge.deadline_at + 1,
            _ => unreachable!(),
        }
        resign_sample_proof(&mut proof);

        let error = tracker
            .record_proof(&proof, &sample_provider_key())
            .expect_err("adversarial proof must fail");
        assert!(
            matches!(
                (mutation, &error),
                (0, PorTrackerError::SampleIndicesMismatch)
                    | (1 | 2, PorTrackerError::ProofOutsideChallengeWindow { .. })
            ),
            "unexpected mutation result {mutation}: {error:?}"
        );
        assert_eq!(tracker.proof_digest(&challenge.challenge_id), None);
    }
}

#[test]
fn tracker_rejects_cross_bound_verdict_without_consuming_challenge() {
    let tracker = PorTracker::default();
    let challenge = sample_challenge();
    let proof = sample_proof(&challenge);
    tracker.record_challenge(&challenge).unwrap();
    tracker
        .record_proof(&proof, &sample_provider_key())
        .unwrap();
    let valid = sample_verdict(&challenge, proof.proof_digest());

    for mutation in 0..5 {
        let mut forged = valid.clone();
        match mutation {
            0 => forged.provider_id[0] ^= 1,
            1 => forged.manifest_digest[0] ^= 1,
            2 => forged.proof_digest = Some([0xEE; 32]),
            3 => forged.proof_digest = None,
            4 => forged.decided_at = proof.submitted_at - 1,
            _ => unreachable!(),
        }
        resign_sample_verdict(&mut forged);
        assert!(
            tracker
                .record_verdict(&forged, &sample_auditor_keys(), 1)
                .is_err()
        );
        assert!(
            tracker.contains_challenge(&challenge.challenge_id),
            "mutation {mutation} must not consume challenge state"
        );
        assert_eq!(
            tracker.proof_digest(&challenge.challenge_id),
            Some(proof.proof_digest())
        );
    }

    tracker
        .record_verdict(&valid, &sample_auditor_keys(), 1)
        .expect("valid verdict remains retryable after forged attempts");
    assert!(!tracker.contains_challenge(&challenge.challenge_id));
}

#[test]
fn tracker_enforces_provider_admission_and_auditor_threshold_at_commit_boundary() {
    let tracker = PorTracker::default();
    let challenge = sample_challenge();
    let proof = sample_proof(&challenge);
    tracker.record_challenge(&challenge).unwrap();

    assert!(matches!(
        tracker.record_proof(&proof, &[0xFE; 32]),
        Err(PorTrackerError::ProofSignatureInvalid(
            sorafs_manifest::por::PorSignatureVerificationError::ProviderSignerMismatch
        ))
    ));
    assert_eq!(tracker.proof_digest(&challenge.challenge_id), None);
    tracker
        .record_proof(&proof, &sample_provider_key())
        .expect("admitted provider proof");

    let verdict = sample_verdict(&challenge, proof.proof_digest());
    assert!(matches!(
        tracker.record_verdict(&verdict, &[vec![0xFD; 32]], 1),
        Err(PorTrackerError::VerdictSignatureInvalid(
            sorafs_manifest::por::PorSignatureVerificationError::UntrustedAuditorSigner
        ))
    ));
    let mut two_auditors = sample_auditor_keys();
    two_auditors.push(vec![0xFC; 32]);
    assert!(matches!(
            tracker.record_verdict(&verdict, &two_auditors, 2),
            Err(PorTrackerError::VerdictSignatureInvalid(
                sorafs_manifest::por::PorSignatureVerificationError::InsufficientTrustedAuditorSignatures {
                    actual: 1,
                    required: 2,
                }
            ))
        ));
    assert!(tracker.contains_challenge(&challenge.challenge_id));
    tracker
        .record_verdict(&verdict, &sample_auditor_keys(), 1)
        .expect("trusted auditor threshold");
}

#[test]
fn tracker_requires_proof_for_success_but_allows_failure_without_one() {
    let challenge = sample_challenge();
    let tracker = PorTracker::default();
    tracker.record_challenge(&challenge).unwrap();
    let success = sample_verdict(&challenge, [0x55; 32]);
    assert!(matches!(
        tracker.record_verdict(&success, &sample_auditor_keys(), 1),
        Err(PorTrackerError::UnexpectedVerdictProofDigest)
    ));
    assert!(tracker.contains_challenge(&challenge.challenge_id));

    let mut success_without_digest = success.clone();
    success_without_digest.proof_digest = None;
    resign_sample_verdict(&mut success_without_digest);
    assert!(matches!(
        tracker.record_verdict(&success_without_digest, &sample_auditor_keys(), 1),
        Err(PorTrackerError::MissingProofForSuccessfulVerdict)
    ));

    let mut failure = success_without_digest;
    failure.outcome = AuditOutcomeV1::Failed;
    failure.decided_at = challenge.deadline_at;
    failure.failure_reason = Some("provider missed deadline".to_owned());
    resign_sample_verdict(&mut failure);
    tracker
        .record_verdict(&failure, &sample_auditor_keys(), 1)
        .expect("failure without proof is a valid terminal transition");
}

#[test]
fn failed_verdict_commits_before_repair_handoff_and_remains_pending() {
    let tracker = PorTracker::default();
    let challenge = sample_challenge();
    let proof = sample_proof(&challenge);
    tracker.record_challenge(&challenge).unwrap();
    tracker
        .record_proof(&proof, &sample_provider_key())
        .unwrap();
    let mut verdict = sample_verdict(&challenge, proof.proof_digest());
    verdict.outcome = AuditOutcomeV1::Failed;
    verdict.failure_reason = Some("proof verification failed".to_owned());
    resign_sample_verdict(&mut verdict);

    let transition = tracker
        .record_verdict_durable(&verdict, &sample_auditor_keys(), 1)
        .expect("verdict commits independently of repair admission");
    assert!(!tracker.contains_challenge(&challenge.challenge_id));
    let pending = tracker
        .next_pending_repair_work()
        .expect("read durable repair work")
        .expect("failed verdict retains repair work");
    assert_eq!(Some(pending.repair_task_id), transition.repair_task_id);
    assert_eq!(pending.intent.challenge_id, challenge.challenge_id);
}

#[test]
fn failed_verdict_exact_replay_reuses_durable_pending_task() {
    let tracker = PorTracker::default();
    let challenge = sample_challenge();
    tracker.record_challenge(&challenge).unwrap();
    let mut verdict = sample_verdict(&challenge, [0x11; 32]);
    verdict.outcome = AuditOutcomeV1::Failed;
    verdict.proof_digest = None;
    verdict.decided_at = challenge.deadline_at;
    verdict.failure_reason = Some("provider missed the challenge".to_owned());
    resign_sample_verdict(&mut verdict);
    let first = tracker
        .record_verdict_durable(&verdict, &sample_auditor_keys(), 1)
        .expect("initial failed verdict");
    let replay = tracker
        .record_verdict_durable(&verdict, &sample_auditor_keys(), 1)
        .expect("exact failed verdict replay");

    assert!(first.newly_finalized);
    assert!(!replay.newly_finalized);
    assert_eq!(replay.repair_task_id, first.repair_task_id);
    assert_eq!(
        tracker
            .next_pending_repair_work()
            .expect("read pending repair")
            .map(|work| work.repair_task_id),
        first.repair_task_id
    );

    let mut conflicting = verdict.clone();
    conflicting.failure_reason = Some("different terminal reason".to_owned());
    resign_sample_verdict(&mut conflicting);
    assert!(matches!(
        tracker.record_verdict_durable(&conflicting, &sample_auditor_keys(), 1),
        Err(PorTrackerError::VerdictConflict)
    ));
}

#[test]
fn failed_verdict_rejects_mismatched_handoff_acknowledgement() {
    let tracker = PorTracker::default();
    let challenge = sample_challenge();
    tracker.record_challenge(&challenge).unwrap();
    let mut verdict = sample_verdict(&challenge, [0x11; 32]);
    verdict.outcome = AuditOutcomeV1::Failed;
    verdict.proof_digest = None;
    verdict.decided_at = challenge.deadline_at;
    verdict.failure_reason = Some("provider missed the challenge".to_owned());
    resign_sample_verdict(&mut verdict);

    tracker
        .record_verdict_durable(&verdict, &sample_auditor_keys(), 1)
        .expect("failed verdict commits with pending repair work");
    assert!(matches!(
        tracker.acknowledge_repair_handoff(challenge.challenge_id, [0xFF; 32]),
        Err(PorTrackerError::RepairTaskIdMismatch)
    ));
    assert!(tracker.next_pending_repair_work().unwrap().is_some());
}

#[test]
fn failed_verdict_cannot_compact_before_repair_handoff_acknowledgement() {
    let tracker = PorTracker::default();
    let challenge = sample_challenge();
    tracker.record_challenge(&challenge).unwrap();
    let mut verdict = sample_verdict(&challenge, [0x11; 32]);
    verdict.outcome = AuditOutcomeV1::Failed;
    verdict.proof_digest = None;
    verdict.decided_at = challenge.deadline_at;
    verdict.failure_reason = Some("provider missed the challenge".to_owned());
    resign_sample_verdict(&mut verdict);
    let transition = tracker
        .record_verdict_durable(&verdict, &sample_auditor_keys(), 1)
        .expect("commit failed verdict");
    tracker
        .acknowledge_reputation_terminal(
            transition.reputation_work.sequence,
            transition.reputation_work.work_digest,
        )
        .expect("acknowledge reputation terminal");
    let archive = MemoryReplayArchive::new(0xA1);
    assert!(matches!(
        tracker.compact_acknowledged_with_replay_archive(&archive, archive.binding, 1),
        Err(PorTrackerError::RepairHandoffPendingCompaction)
    ));
    let repair = tracker
        .next_pending_repair_work()
        .unwrap()
        .expect("repair handoff remains pending");
    tracker
        .acknowledge_repair_handoff(challenge.challenge_id, repair.repair_task_id)
        .expect("acknowledge exact repair handoff");
    assert_eq!(
        tracker
            .compact_acknowledged_with_replay_archive(&archive, archive.binding, 1)
            .expect("compact after all handoffs are durable"),
        1
    );
    let statuses = tracker.status_authority_snapshot().unwrap().statuses;
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].status, PorChallengeOutcome::Failed);
    assert_eq!(statuses[0].repair_task_id, Some(repair.repair_task_id));
}

#[test]
fn por_repair_source_and_report_are_canonical_and_payload_free() {
    let intent = PorFailedRepairIntentV1 {
        manifest_digest: [0x11; 32],
        provider_id: [0x22; 32],
        challenge_id: [0x33; 32],
        failed_samples: 7,
        proof_digest: Some([0x44; 32]),
        decided_at_unix: 1_700_000_400,
    };
    let report = canonical_por_failure_repair_report_v1(intent, "repair@sora")
        .expect("canonical failed PoR repair report");

    assert_eq!(
        report.ticket_id.0,
        format!("POR-{}", hex::encode_upper(intent.challenge_id))
    );
    assert_eq!(report.auditor_account, "repair@sora");
    assert_eq!(report.evidence.por_history_id, None);
    assert_eq!(report.evidence.evidence_json, None);
    assert_eq!(report.evidence.notes, None);
    assert_eq!(report.notes, None);
    assert!(matches!(
        report.evidence.cause,
        RepairCauseV1::PorFailure(RepairPorFailureCauseV1 {
            challenge_id,
            failed_samples: 7,
            proof_digest: Some(proof_digest),
        }) if challenge_id == intent.challenge_id && proof_digest == [0x44; 32]
    ));
    assert_eq!(
        intent.repair_task_id(),
        sorafs_repair_task_id_v1(por_repair_source_identity_v1(intent.challenge_id))
    );
    assert_ne!(
        por_repair_source_identity_v1(intent.challenge_id),
        por_repair_source_identity_v1([0x34; 32])
    );
}
