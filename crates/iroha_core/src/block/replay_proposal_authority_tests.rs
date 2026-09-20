// Historical validation uses exact finality and replicated State, never the live slot map.
fn replay_anchor_with_verified_finality() -> (
    AutonomousAnchorFixture,
    SignedBlock,
    VerifiedV2FinalityArtifact,
) {
    let fixture = autonomous_anchor_fixture_with_replay_lane(None, 0, None, Some(LaneId::new(3)));
    let (executed, verified) = authenticate_replay_fixture_block(&fixture, fixture.block.clone());
    (fixture, executed, verified)
}

fn authenticate_replay_fixture_block(
    fixture: &AutonomousAnchorFixture,
    mut executed: SignedBlock,
) -> (SignedBlock, VerifiedV2FinalityArtifact) {
    use iroha_data_model::block::consensus_v2 as wire;
    executed
        .set_transaction_results_with_transcripts(
            Vec::new(),
            &[],
            Vec::new(),
            BTreeMap::new(),
            Vec::new(),
            fixture.state.block(executed.header()).axt_policy_snapshot(),
        )
        .expect("retain exact control-only execution result");
    let context = fixture
        .profile
        .v2_context()
        .unwrap()
        .authenticated_height_context()
        .expect("fixture independently derives its parent-certified context")
        .clone();
    let subject = wire::BlockSubject {
        parent_block_hash: executed.header().prev_block_hash(),
        block_hash: executed.hash(),
        payload_hash: executed.canonical_proposal_wire_hash().unwrap(),
    };
    let bytes = executed.encode_wire().unwrap();
    let execution_commitment =
        wire::ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
            Hash::new(b"replay-slot-fixture-pre-state"),
            Hash::new(b"replay-slot-fixture-post-state"),
            Hash::new(b"replay-slot-fixture-writes"),
            bytes.len().try_into().unwrap(),
            Hash::new(&bytes),
        );
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: executed.header().view_change_index(),
    };
    let vote = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject,
        execution_commitment,
        signer: 0,
        signature: Vec::new(),
    };
    let keys = context
        .roster
        .iter()
        .map(|member| {
            fixture
                .validator_keys
                .iter()
                .find(|key| key.public_key() == member.validator.public_key())
                .unwrap()
        })
        .collect::<Vec<_>>();
    let shares = keys[..3]
        .iter()
        .map(|key| {
            Signature::new(key.private_key(), &vote.signature_preimage())
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    let artifact = wire::finality::V2FinalityArtifact::new(
        context,
        subject,
        wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(
                &shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
            )
            .unwrap(),
        },
        keys.iter()
            .map(|key| iroha_crypto::bls_normal_pop_prove(key.private_key()).unwrap())
            .collect(),
    );
    let verified = VerifiedV2FinalityArtifact::verify(artifact)
        .expect("actual four-validator q3 BLS finality");
    (executed, verified)
}

fn historical_profile(
    executed: &SignedBlock,
    verified: &VerifiedV2FinalityArtifact,
) -> ConsensusValidationProfile {
    ConsensusValidationProfile::VerifiedReplay {
        block_cadence: Duration::from_millis(1),
        authority: VerifiedReplayProposal::new(executed, verified, None)
            .expect("exact verified proposal"),
    }
}

#[test]
fn authenticated_replay_added_lane_uses_replicated_frontier_without_published_storage() {
    let (fixture, executed, verified) = replay_anchor_with_verified_finality();
    let baseline = LaneCatalog::new(
        nonzero!(1_u32),
        vec![iroha_data_model::nexus::LaneConfig::default()],
    )
    .unwrap();
    fixture.state.kura().replace_lane_storage_entries_for_test(
        &iroha_config::parameters::actual::LaneConfig::from_catalog(&baseline),
    );
    assert!(
        fixture
            .state
            .nexus_snapshot()
            .lane_catalog
            .lanes()
            .iter()
            .any(|lane| lane.id == LaneId::new(3))
    );
    let before = crate::snapshot::canonical_state_snapshot_hash(&fixture.state);
    let candidate = executed.canonical_resultless_proposal();
    ValidBlock::validate_execution_context_with_state(
        &candidate,
        &fixture.topology,
        &fixture.state.query_view(),
        historical_profile(&executed, &verified),
    )
    .expect("verified historical added-lane anchor uses exact replicated predecessor");
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&fixture.state),
        before
    );
    let error = validate_autonomous_anchor_fixture(
        &fixture,
        &candidate,
        candidate.execution_context().unwrap(),
    )
    .expect_err("live admission still requires the published lane storage map");
    assert!(
        matches!(error, BlockValidationError::ExecutionContextInvalid(message)
        if message.contains("local autonomous slot is unreadable") && message.contains("no Kura storage segment"))
    );
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&fixture.state),
        before
    );
}

#[test]
fn authenticated_replay_authority_rejects_different_proposal_wire_and_state_prefix() {
    let (fixture, executed, verified) = replay_anchor_with_verified_finality();
    let authority = VerifiedReplayProposal::new(&executed, &verified, None).unwrap();
    let candidate = executed.canonical_resultless_proposal();
    authority
        .validate(&candidate, &fixture.state.query_view())
        .unwrap();
    let mut different = candidate.clone();
    let mut header = different.header();
    header.creation_time_ms = header.creation_time_ms.checked_add(1).unwrap();
    different.replace_header_for_testing(header);
    assert!(
        matches!(authority.validate(&different, &fixture.state.query_view()),
        Err(BlockValidationError::ExecutionContextInvalid(message)) if message.contains("verified authority"))
    );
    assert!(VerifiedReplayProposal::new(&different, &verified, None).is_err());
    let unrelated = State::new_for_testing(
        World::new(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    assert!(
        authority
            .validate(&candidate, &unrelated.query_view())
            .is_err()
    );
    let mut corrupt = verified.artifact().clone();
    corrupt.commit_qc.aggregate_signature[0] ^= 1;
    assert!(VerifiedV2FinalityArtifact::verify(corrupt).is_err());
}

#[test]
fn authenticated_replay_lane_predecessor_requires_exact_replicated_height_and_hash() {
    let (fixture, executed, verified) = replay_anchor_with_verified_finality();
    let payload = crate::lane_consensus::decode_autonomous_lane_payload_envelope(
        &fixture.bundle.autonomous_lane_payloads[0],
        fixture.state.network_id,
        verified.height_context.epoch,
    )
    .unwrap();
    let descriptor = &payload.origin_proposal.descriptor;
    State::validate_merge_execution_predecessor_against_frontier(
        fixture.state.query_view().world(),
        descriptor,
    )
    .expect("first lane slot extends exact zero frontier");
    let mut skipped = descriptor.clone();
    skipped.previous_lane_block_height = 1;
    skipped.previous_lane_block_descriptor_hash = Some(Hash::new(b"invented predecessor"));
    skipped.lane_block_height = 2;
    assert!(
        State::validate_merge_execution_predecessor_against_frontier(
            fixture.state.query_view().world(),
            &skipped
        )
        .is_err()
    );
    let mut wrong_hash = descriptor.clone();
    wrong_hash.previous_lane_block_descriptor_hash =
        Some(Hash::new(b"invented zero-height predecessor"));
    assert!(
        State::validate_merge_execution_predecessor_against_frontier(
            fixture.state.query_view().world(),
            &wrong_hash
        )
        .is_err()
    );
    assert!(
        !historical_profile(&executed, &verified).persist_pipeline_recovery_sidecar(),
        "replay cannot publish pipeline sidecars while prevalidating"
    );
}

#[test]
fn authenticated_replay_ordinary_and_native_amx_keep_exact_predecessors_without_live_slots() {
    let (fixture, executed, verified) = replay_anchor_with_verified_finality();
    let baseline = LaneCatalog::new(
        nonzero!(1_u32),
        vec![iroha_data_model::nexus::LaneConfig::default()],
    )
    .unwrap();
    fixture.state.kura().replace_lane_storage_entries_for_test(
        &iroha_config::parameters::actual::LaneConfig::from_catalog(&baseline),
    );
    let before = crate::snapshot::canonical_state_snapshot_hash(&fixture.state);
    let payload = crate::lane_consensus::decode_autonomous_lane_payload_envelope(
        &fixture.bundle.autonomous_lane_payloads[0],
        fixture.state.network_id,
        verified.height_context.epoch,
    )
    .unwrap();
    let authority = VerifiedReplayProposal::new(&executed, &verified, None).unwrap();
    let view = fixture.state.query_view();
    let adapter = authority.native_amx_authority(&view).unwrap();
    assert!(
        adapter
            .native_amx_participant_predecessor_is_current(&payload.origin_proposal, None)
            .unwrap()
    );
    assert!(
        !adapter
            .native_amx_participant_predecessor_is_current(
                &payload.origin_proposal,
                Some(HashOf::from_untyped_unchecked(Hash::new(
                    b"invented previous Native settlement"
                ))),
            )
            .unwrap()
    );
    let mut skipped = payload.origin_proposal.clone();
    skipped.descriptor.previous_lane_block_height = 1;
    skipped.descriptor.previous_lane_block_descriptor_hash =
        Some(Hash::new(b"invented shared predecessor"));
    skipped.descriptor.lane_block_height = 2;
    assert!(
        adapter
            .native_amx_participant_predecessor_is_current(&skipped, None)
            .is_err()
    );

    drop(adapter);
    drop(view);

    // Exercise the ordinary ownership storage boundary separately: intrinsic
    // payload/routing validation remains in the common caller, while this check
    // must use the same exact proposal capability and replicated predecessor.
    let descriptor = &payload.origin_proposal.descriptor;
    let ownership = sample_lane_payload_ownership_for_context_at_slot(
        executed.header().height().get(),
        executed.header().view_change_index(),
        descriptor.lane_id,
        descriptor.dataspace_id,
        descriptor.lane_incarnation,
        1,
        0,
        vec![0],
        vec![Hash::new(b"ordinary owned entrypoint")],
        &descriptor.validator_set,
    );
    let mut bundle = BlockExecutionContextBundle::new(Vec::new());
    bundle.lane_payload_ownerships.push(ownership);
    let mut ordinary = fixture.block.clone();
    ordinary.set_execution_context(Some(bundle.clone()));
    let (ordinary, ordinary_verified) = authenticate_replay_fixture_block(&fixture, ordinary);
    let proposal = ordinary.canonical_resultless_proposal();
    let view = fixture.state.query_view();
    ValidBlock::validate_execution_context_lane_payload_artifacts(
        &proposal,
        &view,
        &bundle,
        &historical_profile(&ordinary, &ordinary_verified),
    )
    .expect("ordinary replay uses exact State frontier without a current lane segment");
    assert!(
        ValidBlock::validate_execution_context_lane_payload_artifacts(
            &proposal,
            &view,
            &bundle,
            &fixture.profile,
        )
        .is_err(),
        "live ordinary ownership still requires readable current storage"
    );
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&fixture.state),
        before
    );
}
