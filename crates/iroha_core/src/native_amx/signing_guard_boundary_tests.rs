#[cfg(unix)]
#[test]
fn signing_guard_durably_binds_full_source_session_and_participant_incarnation() {
    let root = tempfile::tempdir().expect("temp dir");
    let (_keypair, signer) = signing_guard_signer(0x6E);
    let validators = vec![signer.clone()];
    let request = full_plan_request(
        body_for_validator_set(NativeAmxPhase::Prepare, &validators),
        validators,
    );
    let base = request.body;
    let guard =
        open_signing_guard(root.path(), &base, signer.clone(), 32).expect("open signing guard");

    let mut wrong_coordinator_dataspace = request.clone();
    wrong_coordinator_dataspace.body.coordinator_dataspace_id = DataSpaceId::new(70);
    let mut wrong_participant_lane = request.clone();
    wrong_participant_lane.body.participant_lane_id = LaneId::new(20);
    let mut wrong_participant_dataspace = request.clone();
    wrong_participant_dataspace.body.participant_dataspace_id = DataSpaceId::new(80);
    let mut wrong_participant_incarnation = request.clone();
    wrong_participant_incarnation
        .body
        .participant_lane_incarnation = Hash::new(b"unbound participant incarnation");
    let mut wrong_plan = request.clone();
    wrong_plan.body.plan_digest = Hash::new(b"unbound full plan");
    for (label, invalid) in [
        ("coordinator dataspace", wrong_coordinator_dataspace),
        ("participant lane", wrong_participant_lane),
        ("participant dataspace", wrong_participant_dataspace),
        ("participant incarnation", wrong_participant_incarnation),
        ("plan digest", wrong_plan),
    ] {
        assert!(
            matches!(
                guard.record(&invalid),
                Err(NativeAmxSigningGuardError::InvalidInput(_))
            ),
            "the journal boundary must reject an unauthenticated {label} drift"
        );
        assert_eq!(
            guard.record_count_for_test(),
            0,
            "an unauthenticated {label} drift must not mutate the journal"
        );
    }

    guard
        .record(&request)
        .expect("record authenticated source-session claim");
    assert_eq!(guard.record_count_for_test(), 1);

    let mut divergent_plan = request.clone();
    let coordinator = RoutingDecision::new(base.coordinator_lane_id, base.coordinator_dataspace_id);
    let primary_participant =
        RoutingDecision::new(base.participant_lane_id, base.participant_dataspace_id);
    let extra_participant = RoutingDecision::new(LaneId::new(3), DataSpaceId::new(9));
    let plan = RoutingPlan::native_amx(
        coordinator,
        vec![
            RouteLeg::new(primary_participant, RouteLegRole::Participant),
            RouteLeg::new(extra_participant, RouteLegRole::Participant),
        ],
    );
    divergent_plan.plan_legs = plan.legs();
    divergent_plan.body.plan_digest = plan.digest();
    assert_eq!(divergent_plan.validate_plan_binding(), Ok(()));
    assert_eq!(
        guard.record(&divergent_plan),
        Err(NativeAmxSigningGuardError::PlanEquivocation),
        "a valid request may not move one source to a different full plan"
    );
    assert_eq!(
        guard.record_count_for_test(),
        1,
        "plan-only equivocation must be rejected before journal mutation"
    );
    drop(guard);

    let mut drifts = Vec::new();

    let mut entrypoint = base;
    entrypoint.phase = NativeAmxPhase::Commit;
    entrypoint.tx_entrypoint_hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(
        Hash::new(b"source-entrypoint-drift"),
    );
    drifts.push(entrypoint);

    let mut global_view = base;
    global_view.phase = NativeAmxPhase::Commit;
    global_view.round.view = global_view.round.view.saturating_add(1);
    drifts.push(global_view);

    let mut coordinator_route = base;
    coordinator_route.phase = NativeAmxPhase::Commit;
    coordinator_route.coordinator_lane_id = LaneId::new(9);
    drifts.push(coordinator_route);

    let mut coordinator_dataspace = base;
    coordinator_dataspace.phase = NativeAmxPhase::Commit;
    coordinator_dataspace.coordinator_dataspace_id = DataSpaceId::new(70);
    drifts.push(coordinator_dataspace);

    let mut coordinator_incarnation = base;
    coordinator_incarnation.phase = NativeAmxPhase::Commit;
    coordinator_incarnation.coordinator_lane_incarnation =
        Hash::new(b"coordinator-incarnation-drift");
    drifts.push(coordinator_incarnation);

    let mut planned_height = base;
    planned_height.phase = NativeAmxPhase::Commit;
    planned_height.planned_coordinator_block_height = planned_height
        .planned_coordinator_block_height
        .saturating_add(1);
    drifts.push(planned_height);

    let mut coordinator_view = base;
    coordinator_view.phase = NativeAmxPhase::Commit;
    coordinator_view.coordinator_lane_block_view = coordinator_view
        .coordinator_lane_block_view
        .saturating_add(1);
    drifts.push(coordinator_view);

    let mut coordinator_proposal = base;
    coordinator_proposal.phase = NativeAmxPhase::Commit;
    coordinator_proposal.coordinator_proposal_hash = Hash::new(b"coordinator-proposal-drift");
    drifts.push(coordinator_proposal);

    let mut participant_incarnation = base;
    participant_incarnation.phase = NativeAmxPhase::Commit;
    participant_incarnation.participant_lane_incarnation =
        Hash::new(b"participant-incarnation-drift");
    drifts.push(participant_incarnation);

    for drift in drifts {
        let restarted = open_signing_guard(root.path(), &base, signer.clone(), 32)
            .expect("restart signing guard");
        assert_eq!(
            restarted.record_body_for_test(&drift),
            Err(NativeAmxSigningGuardError::PlanEquivocation)
        );
        assert_eq!(
            restarted.record_count_for_test(),
            1,
            "rejected source-session drift must precede journal mutation"
        );
    }

    let mut round_context = base;
    round_context.phase = NativeAmxPhase::Commit;
    round_context.round.context_id = another_context(b"source-round-context-drift");
    let mut round_height = base;
    round_height.phase = NativeAmxPhase::Commit;
    round_height.round.height = round_height.round.height.saturating_add(1);
    let mut epoch = base;
    epoch.phase = NativeAmxPhase::Commit;
    epoch.epoch = epoch.epoch.saturating_add(1);
    let mut chain = base;
    chain.phase = NativeAmxPhase::Commit;
    chain.chain_id_hash = Hash::new(b"source-chain-drift");
    let mut authority_height = base;
    authority_height.phase = NativeAmxPhase::Commit;
    authority_height.authority_context_height =
        authority_height.authority_context_height.saturating_add(1);
    for (label, drift, expected) in [
        (
            "round context",
            round_context,
            NativeAmxSigningGuardError::ContextMismatch,
        ),
        (
            "round height",
            round_height,
            NativeAmxSigningGuardError::InvalidInput("malformed attestation body".to_owned()),
        ),
        ("epoch", epoch, NativeAmxSigningGuardError::ContextMismatch),
        ("chain", chain, NativeAmxSigningGuardError::ContextMismatch),
        (
            "authority height",
            authority_height,
            NativeAmxSigningGuardError::InvalidInput("malformed attestation body".to_owned()),
        ),
    ] {
        let restarted = open_signing_guard(root.path(), &base, signer.clone(), 32)
            .expect("restart signing guard");
        assert_eq!(
            restarted.record_body_for_test(&drift),
            Err(expected),
            "{label}"
        );
        assert_eq!(
            restarted.record_count_for_test(),
            1,
            "rejected {label} drift must precede journal mutation"
        );
    }

    let mut second_participant = base;
    second_participant.participant_lane_id = LaneId::new(3);
    second_participant.participant_dataspace_id = DataSpaceId::new(9);
    second_participant.participant_lane_incarnation =
        Hash::new(b"second-planned-participant-incarnation");
    second_participant.participant_proposal_hash =
        Hash::new(b"second-planned-participant-proposal");
    second_participant.participant_settlement_commitment = second_participant
        .computed_grouped_participant_settlement_commitment(&[second_participant.source_id])
        .expect("single-source test fixture settlement is valid");
    let restarted =
        open_signing_guard(root.path(), &base, signer.clone(), 32).expect("restart signing guard");
    restarted
        .record_body_for_test(&second_participant)
        .expect("same source may bind another planned participant route");
    assert_eq!(restarted.record_count_for_test(), 2);

    let mut second_source = base;
    second_source.source_id = [0xA4; Hash::LENGTH];
    second_source.tx_entrypoint_hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(
        Hash::new(b"second-source-entrypoint"),
    );
    second_source.participant_previous_block_height = 1;
    second_source.participant_previous_block_descriptor_hash =
        Some(Hash::new(b"second-source-participant-predecessor"));
    second_source.participant_lane_block_height = 2;
    second_source.participant_proposal_hash = Hash::new(b"second-source-participant-proposal");
    second_source.participant_settlement_commitment = second_source
        .computed_grouped_participant_settlement_commitment(&[second_source.source_id])
        .expect("single-source test fixture settlement is valid");
    restarted
        .record_body_for_test(&second_source)
        .expect("a distinct source owns an independent durable claim");
    assert_eq!(restarted.record_count_for_test(), 3);

    let mut second_source_conflict = second_source;
    second_source_conflict.phase = NativeAmxPhase::Commit;
    second_source_conflict.coordinator_dataspace_id = DataSpaceId::new(71);
    assert_eq!(
        restarted.record_body_for_test(&second_source_conflict),
        Err(NativeAmxSigningGuardError::PlanEquivocation),
        "the source-keyed map must retain an independent claim for each source"
    );
    assert_eq!(restarted.record_count_for_test(), 3);
}

#[cfg(unix)]
#[test]
fn signing_guard_durably_rejects_same_source_plan_only_equivocation_after_restart() {
    let root = tempfile::tempdir().expect("temp dir");
    let (_keypair, signer) = signing_guard_signer(0x72);
    let body = body(NativeAmxPhase::Prepare);
    let guard =
        open_signing_guard(root.path(), &body, signer.clone(), 8).expect("open signing guard");
    guard
        .record_body_for_test(&body)
        .expect("record source-plan claim");

    let mut conflicting_plan = body;
    conflicting_plan.phase = NativeAmxPhase::Commit;
    conflicting_plan.plan_digest = Hash::new(b"conflicting durable native AMX plan");
    assert_eq!(
        guard.record_body_for_test(&conflicting_plan),
        Err(NativeAmxSigningGuardError::PlanEquivocation)
    );
    assert_eq!(guard.record_count_for_test(), 1);
    drop(guard);

    let restarted =
        open_signing_guard(root.path(), &body, signer, 8).expect("restart signing guard");
    assert_eq!(
        restarted.record_body_for_test(&conflicting_plan),
        Err(NativeAmxSigningGuardError::PlanEquivocation)
    );
    assert_eq!(restarted.record_count_for_test(), 1);
}

#[cfg(unix)]
fn write_unpublished_signing_tail(
    guard: &NativeAmxSigningGuard,
    body: &NativeAmxAttestationBodyV2,
    signer: &PeerId,
) -> PathBuf {
    let anchor = guard.inner.lock().anchor.clone();
    let sequence = anchor
        .record_count
        .checked_add(1)
        .expect("fixture tail sequence");
    let tail = NativeAmxSigningRecordV2::from_body(sequence, anchor.head_hash, body, signer)
        .expect("build canonical unpublished signing tail");
    let path = NativeAmxSigningGuard::record_path(&guard.directory, &tail);
    write_secure_new(
        &path,
        &norito::encode_canonical(&tail).expect("encode canonical unpublished signing tail"),
    );
    path
}

#[cfg(unix)]
#[test]
fn signing_guard_restart_rejects_source_and_slot_equivocating_unpublished_tails() {
    for (seed, slot_conflict) in [(0x94, false), (0x95, true)] {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(seed);
        let base = body(NativeAmxPhase::Prepare);
        let guard =
            open_signing_guard(root.path(), &base, signer.clone(), 8).expect("open signing guard");
        guard
            .record_body_for_test(&base)
            .expect("record anchored source and slot claims");

        let mut conflicting_tail = base;
        if slot_conflict {
            conflicting_tail.source_id = [0xE5; Hash::LENGTH];
            conflicting_tail.tx_entrypoint_hash =
                HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::prehashed(
                    conflicting_tail.source_id,
                ));
            conflicting_tail.participant_settlement_commitment =
                Hash::new(b"unpublished tail slot settlement conflict");
        } else {
            // A different phase gives the tail a fresh signing key and slot,
            // leaving the source-session claim as the only conflict.
            conflicting_tail.phase = NativeAmxPhase::Commit;
            conflicting_tail.coordinator_proposal_hash =
                Hash::new(b"unpublished tail source-session conflict");
        }
        assert!(native_amx_body_shape_valid(&conflicting_tail));
        assert_ne!(
            NativeAmxSigningKeyV2::from_body(&base, &signer),
            NativeAmxSigningKeyV2::from_body(&conflicting_tail, &signer),
            "the unpublished tail must not collide with the anchored signing key"
        );
        let anchored_slot = NativeAmxSigningSlotV3::from_body(&base, &signer);
        let tail_slot = NativeAmxSigningSlotV3::from_body(&conflicting_tail, &signer);
        if slot_conflict {
            assert_ne!(conflicting_tail.source_id, base.source_id);
            assert_eq!(tail_slot, anchored_slot);
            assert_ne!(
                NativeAmxSigningSlotClaimV3::from_body(&conflicting_tail),
                NativeAmxSigningSlotClaimV3::from_body(&base),
                "the distinct source must conflict only with the anchored slot claim"
            );
        } else {
            assert_eq!(conflicting_tail.source_id, base.source_id);
            assert_ne!(tail_slot, anchored_slot);
            assert_ne!(
                NativeAmxSourceSessionClaimV4::from_body(&conflicting_tail),
                NativeAmxSourceSessionClaimV4::from_body(&base),
                "the fresh slot must conflict only with the anchored source-session claim"
            );
        }
        let tail_path = write_unpublished_signing_tail(&guard, &conflicting_tail, &signer);
        drop(guard);

        assert!(matches!(
            open_signing_guard(root.path(), &base, signer, 8),
            Err(NativeAmxSigningGuardError::UnsafeJournal(message))
                if message.contains("invalid unpublished tail record")
        ));
        assert!(
            tail_path.exists(),
            "an equivocation-shaped unpublished tail must fail closed, not be reconciled away"
        );
    }
}

#[cfg(unix)]
#[test]
fn signing_guard_restart_rejects_truncated_and_oversized_records_and_anchors() {
    fn assert_record_corruption_rejected(seed: u8, oversized: bool) {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(seed);
        let body = body(NativeAmxPhase::Prepare);
        let guard =
            open_signing_guard(root.path(), &body, signer.clone(), 8).expect("open signing guard");
        guard
            .record_body_for_test(&body)
            .expect("record anchored body");
        let record_path = signing_record_paths(&guard)
            .into_iter()
            .next()
            .expect("anchored signing record");
        let max_record_bytes = guard.limits.max_record_bytes.get();
        drop(guard);

        if oversized {
            fs::write(
                &record_path,
                vec![0xA5; max_record_bytes.checked_add(1).expect("oversize length")],
            )
            .expect("write oversized record");
        } else {
            let mut bytes = fs::read(&record_path).expect("read canonical record");
            bytes.pop().expect("canonical record is non-empty");
            fs::write(&record_path, bytes).expect("write truncated record");
        }

        assert!(matches!(
            open_signing_guard(root.path(), &body, signer, 8),
            Err(NativeAmxSigningGuardError::UnsafeJournal(_))
        ));
    }

    fn assert_anchor_corruption_rejected(seed: u8, oversized: bool) {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(seed);
        let body = body(NativeAmxPhase::Prepare);
        let guard =
            open_signing_guard(root.path(), &body, signer.clone(), 8).expect("open signing guard");
        guard
            .record_body_for_test(&body)
            .expect("record anchored body");
        let anchor_path = NativeAmxSigningGuard::anchor_path(&guard.directory);
        let max_anchor_bytes = guard.limits.max_anchor_bytes.get();
        drop(guard);

        if oversized {
            fs::write(
                &anchor_path,
                vec![0x5A; max_anchor_bytes.checked_add(1).expect("oversize length")],
            )
            .expect("write oversized anchor");
        } else {
            let mut bytes = fs::read(&anchor_path).expect("read canonical anchor");
            bytes.pop().expect("canonical anchor is non-empty");
            fs::write(&anchor_path, bytes).expect("write truncated anchor");
        }

        assert!(matches!(
            open_signing_guard(root.path(), &body, signer, 8),
            Err(NativeAmxSigningGuardError::UnsafeJournal(_))
        ));
    }

    assert_record_corruption_rejected(0x96, false);
    assert_record_corruption_rejected(0x97, true);
    assert_anchor_corruption_rejected(0x98, false);
    assert_anchor_corruption_rejected(0x99, true);
}

#[cfg(unix)]
#[test]
fn signing_guard_restart_rejects_duplicate_record_sequence() {
    let root = tempfile::tempdir().expect("temp dir");
    let (_keypair, signer) = signing_guard_signer(0x9A);
    let base = body(NativeAmxPhase::Prepare);
    let guard =
        open_signing_guard(root.path(), &base, signer.clone(), 8).expect("open signing guard");
    guard
        .record_body_for_test(&base)
        .expect("record anchored body");
    let existing_path = signing_record_paths(&guard)
        .into_iter()
        .next()
        .expect("anchored signing record");
    let anchor = guard.inner.lock().anchor.clone();

    let mut duplicate_body = base;
    duplicate_body.source_id = [0xE6; Hash::LENGTH];
    duplicate_body.tx_entrypoint_hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(
        Hash::prehashed(duplicate_body.source_id),
    );
    duplicate_body.participant_settlement_commitment = duplicate_body
        .computed_grouped_participant_settlement_commitment(&[duplicate_body.source_id])
        .expect("duplicate-sequence fixture settlement");
    let duplicate = NativeAmxSigningRecordV2::from_body(
        1,
        anchor
            .binding
            .genesis_head()
            .expect("derive anchored genesis head"),
        &duplicate_body,
        &signer,
    )
    .expect("build duplicate-sequence record");
    let duplicate_path = NativeAmxSigningGuard::record_path(&guard.directory, &duplicate);
    assert_ne!(duplicate_path, existing_path);
    write_secure_new(
        &duplicate_path,
        &norito::encode_canonical(&duplicate).expect("encode duplicate-sequence record"),
    );
    drop(guard);

    assert!(matches!(
        open_signing_guard(root.path(), &base, signer, 8),
        Err(NativeAmxSigningGuardError::UnsafeJournal(message))
            if message.contains("duplicate record sequence")
    ));
}
