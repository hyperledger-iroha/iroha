#[test]
fn pending_current_canonical_anchor_preserves_exact_in_memory_decision_owner() {
    let (mut adapter, keys) = fixture_at_height_inner(wire::ConsensusMode::Permissioned, 2, true);
    let (block, proposal) = planned_lane_candidate_block_at_view(&adapter, &keys, 0);
    let (round, subject) = mark_global_body_locked_for_block(&mut adapter, &block);
    assert_ne!(
        adapter.bind_locked_global_body(&block),
        V2LaneIngressOutcome::Rejected
    );
    adapter
        .kura
        .store_block(block.clone())
        .expect("interrupt actual Apply after body and raw ownership publication");
    assert!(
        adapter
            .kura
            .v2_finality_artifact(adapter.context.height)
            .expect("authenticate actual finality absence")
            .is_none()
    );
    assert!(
        adapter
            .kura
            .read_lane_block_artifact_read_only(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height,
            )
            .expect("the pending body has a real structurally valid raw slot")
            .is_some()
    );
    adapter
        .retain_merge_sidecars_for_global_view(round.view, Some(subject), Some(subject))
        .expect("retain the exact lock through Decision before finality publication");
    let proposals_before = adapter.lane_sessions.rollover_proposal_hashes();
    let qcs_before = adapter.lane_sessions.qcs_for_incomplete_sessions();
    let bindings_before = adapter.locally_bound_lane_proposals.clone();
    let effects_before = adapter
        .effects
        .iter()
        .map(lane_work_effect_key)
        .collect::<Vec<_>>();
    assert!(proposals_before.contains(&proposal.proposal_hash));
    assert_eq!(
        bindings_before.get(&proposal.proposal_hash),
        proposal.payload_block_hint.as_ref()
    );
    assert!(
        adapter
            .canonical_anchor_for_proposal(&proposal)
            .expect("pending publication is recoverable")
            .is_none(),
        "an unsigned current raw slot is not authenticated pruned-history authority"
    );
    assert!(
        adapter
            .proposal_is_bound_to_decided_carrier(&proposal)
            .expect("exact in-memory Decision binding remains authoritative")
    );
    assert_eq!(
        adapter.lane_sessions.rollover_proposal_hashes(),
        proposals_before
    );
    assert_eq!(
        adapter.lane_sessions.qcs_for_incomplete_sessions(),
        qcs_before
    );
    assert_eq!(adapter.locally_bound_lane_proposals, bindings_before);
    assert_eq!(
        adapter
            .effects
            .iter()
            .map(lane_work_effect_key)
            .collect::<Vec<_>>(),
        effects_before
    );
    assert!(!adapter.output_guard.restart_required());
    assert!(adapter.output_guard.acquire().is_some());

    let finality = verified_finality_artifact_for_block(&adapter, &keys, &block);
    assert!(
        adapter.prepare_canonical_lane_rollover(&finality).is_err(),
        "pending raw ownership must retain every rollover owner until finality is published"
    );
    assert!(!adapter.output_guard.restart_required());
    assert_eq!(
        adapter.lane_sessions.rollover_proposal_hashes(),
        proposals_before
    );
    assert_eq!(
        adapter.lane_sessions.qcs_for_incomplete_sessions(),
        qcs_before
    );
    assert_eq!(adapter.locally_bound_lane_proposals, bindings_before);
    let _ = adapter
        .kura
        .store_v2_finality_artifact(&finality)
        .expect("complete the actual finality publication cut");
    assert!(
        adapter
            .canonical_anchor_for_proposal(&proposal)
            .expect("signed exact ownership becomes canonical authority")
            .is_some()
    );
    assert!(!adapter.output_guard.restart_required());
}

#[test]
fn canonical_anchor_rejects_coherent_raw_rewrite_bound_to_unchanged_signed_carrier() {
    let (adapter, _, block, proposal) = strict_current_lane_owner_fixture();
    let original = adapter
        .canonical_anchor_for_proposal(&proposal)
        .expect("healthy signed canonical ownership")
        .expect("the exact canonical anchor is published");
    let mut rewritten = original.clone();
    rewritten.ownership.proposal_view += 1;
    rewritten.ownership.lane_block_view += 1;
    let replay = rewritten
        .ownership
        .compute_replay_hashes()
        .expect("derive coherent ownership for another view");
    rewritten.ownership.subject_hash = replay.subject_hash;
    rewritten.ownership.payload_ownership_hash = replay.payload_ownership_hash;
    rewritten.ownership.rbc_instance_hash = replay.rbc_instance_hash;
    rewritten.ownership.lane_block_descriptor_hash = Some(replay.lane_block_descriptor_hash);
    rewritten
        .ownership
        .validate_replay_material()
        .expect("the replacement is structurally valid, with recomputed replay bindings");
    let candidate = proposal_from_ownership(&rewritten.ownership, rewritten.proposal_block_hash)
        .expect("reconstruct the competing candidate");
    crate::lane_consensus::validate_lane_block_proposal(&candidate)
        .expect("a different candidate is structurally valid");
    assert!(
        adapter
            .canonical_anchor_for_proposal(&candidate)
            .expect("a competing candidate is not local corruption")
            .is_none()
    );
    assert!(!adapter.output_guard.restart_required());
    assert_eq!(rewritten.proposal_block_hash, block.hash());

    let blocks = adapter
        .state
        .nexus_snapshot()
        .lane_config
        .entry(proposal.descriptor.lane_id)
        .expect("configured lane")
        .blocks_dir(adapter.kura.store_root());
    let data_path = blocks.join("lane_artifacts/ownerships.norito");
    let index_path = blocks.join("lane_artifacts/ownerships.index");
    let index_before = std::fs::read(&index_path).expect("retain the actual indexed framing");
    let original_frame = original
        .encode_framed()
        .expect("encode exact original raw record");
    let replacement_frame = rewritten
        .encode_framed()
        .expect("encode coherent replacement");
    assert_eq!(original_frame.len(), replacement_frame.len());
    let mut data = std::fs::read(&data_path).expect("read actual occupied raw data");
    let positions = data
        .windows(original_frame.len())
        .enumerate()
        .filter_map(|(position, bytes)| (bytes == original_frame).then_some(position))
        .collect::<Vec<_>>();
    assert_eq!(
        positions.len(),
        1,
        "the exact indexed fixture record occurs once"
    );
    let start = positions[0];
    data[start..start + replacement_frame.len()].copy_from_slice(&replacement_frame);
    std::fs::write(&data_path, &data).expect("rewrite only the complete same-size raw record");
    assert_eq!(
        adapter
            .kura
            .read_lane_block_artifact_read_only(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height,
            )
            .expect("the unchanged index and coherent raw replay bindings remain valid"),
        Some(rewritten),
        "this counterexample must reach the signed ownership join"
    );
    let height =
        NonZeroUsize::new(usize::try_from(block.header().height().get()).unwrap()).unwrap();
    assert_eq!(
        adapter
            .kura
            .read_block_body(height)
            .expect("signed carrier is intact")
            .as_deref(),
        Some(&block)
    );
    let proposals_before = adapter.lane_sessions.rollover_proposal_hashes();
    let qcs_before = adapter.lane_sessions.qcs_for_incomplete_sessions();
    let bindings_before = adapter.locally_bound_lane_proposals.clone();
    assert!(!adapter.output_guard.restart_required());
    let error = adapter
        .canonical_anchor_for_proposal(&candidate)
        .expect_err("same carrier hash cannot authenticate another raw ownership");
    assert!(matches!(error, V2LaneWorkError::Persistence(message)
        if message.contains("signed carrier ownership")));
    assert!(adapter.output_guard.restart_required());
    assert!(adapter.output_guard.acquire().is_none());
    assert_eq!(
        adapter.lane_sessions.rollover_proposal_hashes(),
        proposals_before
    );
    assert_eq!(
        adapter.lane_sessions.qcs_for_incomplete_sessions(),
        qcs_before
    );
    assert_eq!(adapter.locally_bound_lane_proposals, bindings_before);
    assert_eq!(std::fs::read(data_path).unwrap(), data);
    assert_eq!(std::fs::read(index_path).unwrap(), index_before);
}

#[test]
fn canonical_anchor_errors_preserve_rollover_owners_and_reject_only_corruption() {
    let (mut adapter, keys, block, proposal) = strict_current_lane_owner_fixture();
    let session = committed_lane_session(&proposal, &keys);
    assert!(
        adapter
            .session_has_canonical_anchor(&session)
            .expect("exact ordinary anchor")
    );
    assert!(
        !adapter
            .canonical_autonomous_anchor_matches_kura(&proposal)
            .expect("an ordinary carrier is healthy without an autonomous payload")
    );
    let mut unanchored = proposal.clone();
    unanchored.payload_block_hint = None;
    assert!(
        !adapter
            .canonical_autonomous_anchor_matches_kura(&unanchored)
            .expect("an unanchored candidate has no canonical autonomous authority")
    );
    let mut losing = proposal.clone();
    losing
        .payload_block_hint
        .as_mut()
        .expect("candidate hint")
        .proposal_view += 1;
    losing.proposal_hash = losing.computed_proposal_hash();
    assert!(
        !adapter
            .canonical_autonomous_anchor_matches_kura(&losing)
            .expect("a competing hint is not corruption of the valid canonical body")
    );
    assert!(!adapter.output_guard.restart_required());

    let finality = verified_finality_artifact_for_block(&adapter, &keys, &block);
    let proposals_before = adapter.lane_sessions.rollover_proposal_hashes();
    let qcs_before = adapter.lane_sessions.qcs_for_incomplete_sessions();
    let bindings_before = adapter.locally_bound_lane_proposals.clone();
    let blocks = adapter
        .state
        .nexus_snapshot()
        .lane_config
        .primary()
        .blocks_dir(adapter.kura.store_root());
    let body_path = blocks.join("blocks.data");
    corrupt_durable_file_for_test(&body_path);
    let damaged = std::fs::read(&body_path).expect("read injected body corruption");
    assert!(!adapter.output_guard.restart_required());
    assert!(
        matches!(
            adapter.prepare_canonical_lane_rollover(&finality),
            Err(V2LaneWorkError::Persistence(_))
        ),
        "rollover must independently authenticate the canonical body before pruning owners"
    );
    assert!(
        adapter
            .canonical_autonomous_anchor_matches_kura(&proposal)
            .is_err()
    );
    assert!(adapter.output_guard.restart_required());
    assert!(adapter.session_has_canonical_anchor(&session).is_err());
    assert!(adapter.prepare_canonical_lane_rollover(&finality).is_err());
    assert_eq!(
        adapter.lane_sessions.rollover_proposal_hashes(),
        proposals_before
    );
    assert_eq!(
        adapter.lane_sessions.qcs_for_incomplete_sessions(),
        qcs_before
    );
    assert_eq!(adapter.locally_bound_lane_proposals, bindings_before);
    assert_eq!(
        std::fs::read(body_path).expect("retained corruption"),
        damaged
    );
}

#[test]
fn occupied_receipt_corruption_never_reopens_fresh_lane_signing() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let (_, proposal) = planned_lane_candidate_block_at_view(&adapter, &keys, 0);
    adapter
        .lane_sessions
        .insert_recovered_proposal_replacing_uncommitted_conflict(proposal.clone())
        .expect("retain the exact unsigned lane session");
    adapter.locally_bound_lane_proposals.insert(
        proposal.proposal_hash,
        proposal.payload_block_hint.expect("planned candidate hint"),
    );
    assert!(
        adapter
            .proposal_can_progress(&proposal)
            .expect("prove the genuinely absent receipt slot")
    );
    assert!(
        adapter
            .lane_sessions
            .local_prepare_vote_needed_for(&proposal, &adapter.local_peer)
    );
    let _ = adapter.drain_effects(usize::MAX);
    let proposals_before = adapter.lane_sessions.rollover_proposal_hashes();
    let qcs_before = adapter.lane_sessions.qcs_for_incomplete_sessions();
    let source = adapter
        .state
        .nexus_snapshot()
        .lane_config
        .entry(proposal.descriptor.lane_id)
        .expect("configured lane")
        .blocks_dir(adapter.kura.store_root())
        .join("lane_artifacts");
    std::fs::create_dir_all(&source).expect("create the active receipt namespace");
    let data_path = source.join("application_receipts.norito");
    let index_path = source.join("application_receipts.index");
    assert!(
        !data_path.exists() && !index_path.exists(),
        "the control must begin with real absence"
    );
    std::fs::write(&data_path, b"occupied damaged receipt").expect("occupy the receipt data file");
    std::fs::write(&index_path, b"damaged index").expect("occupy the receipt index file");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o700))
            .expect("private receipt namespace");
        for path in [&data_path, &index_path] {
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
                .expect("private damaged receipt fixture");
        }
    }
    let before = (
        std::fs::read(&data_path).unwrap(),
        std::fs::read(&index_path).unwrap(),
    );

    assert!(matches!(
        adapter.sign_lane_vote(&proposal, CertPhase::Prepare),
        Err(V2LaneWorkError::Persistence(_))
    ));
    assert!(adapter.output_guard.restart_required());
    assert!(adapter.proposal_can_progress(&proposal).is_err());
    adapter.drive_lane_sessions();
    assert!(
        adapter
            .lane_sessions
            .local_prepare_vote_needed_for(&proposal, &adapter.local_peer)
    );
    assert_eq!(
        adapter.lane_sessions.rollover_proposal_hashes(),
        proposals_before
    );
    assert_eq!(
        adapter.lane_sessions.qcs_for_incomplete_sessions(),
        qcs_before
    );
    assert!(
        adapter.effects.is_empty(),
        "the failed read cannot produce fresh output"
    );
    assert_eq!(
        (
            std::fs::read(data_path).unwrap(),
            std::fs::read(index_path).unwrap()
        ),
        before,
        "the signer must not repair or overwrite occupied corruption"
    );
}
