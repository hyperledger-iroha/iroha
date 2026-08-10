// Backpressure retention and adversarial vote-set tests.
//
// Included by `lane_consensus::tests` to preserve exact libtest names.

#[test]
fn lane_block_session_cache_preserves_undrained_committed_session_under_backpressure() {
    let keys = [checked_bls_keypair(1), checked_bls_keypair(2)];
    let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
    validator_set.sort();
    let proposal_a = lane_block_proposal_at_height(&validator_set, 13);
    let key_a = LaneBlockSessionKey::from_proposal(&proposal_a);
    let prepare_body = proposal_a.vote_body(CertPhase::Prepare);
    let prepare_vote_a = signed_vote(&prepare_body, &keys[0]);
    let prepare_vote_b = signed_vote(&prepare_body, &keys[1]);
    let prepare_qc = aggregate_lane_block_votes_to_qc(
        prepare_body,
        validator_set.clone(),
        &[prepare_vote_a, prepare_vote_b],
    )
    .expect("prepare QC");
    let commit_body = proposal_a.vote_body(CertPhase::Commit);
    let commit_vote_a = signed_vote(&commit_body, &keys[0]);
    let commit_vote_b = signed_vote(&commit_body, &keys[1]);
    let commit_qc = aggregate_lane_block_votes_to_qc(
        commit_body,
        validator_set.clone(),
        &[commit_vote_a, commit_vote_b],
    )
    .expect("commit QC");
    let proposal_b = lane_block_proposal_at_height(&validator_set, 14);
    let key_b = LaneBlockSessionKey::from_proposal(&proposal_b);
    let proposal_c = lane_block_proposal_at_height(&validator_set, 15);
    let key_c = LaneBlockSessionKey::from_proposal(&proposal_c);
    let pops = signer_pops(&keys);
    let mut cache = LaneBlockSessionCache::new(1);

    assert_eq!(
        cache.insert_proposal(proposal_a.clone()),
        Ok(LaneBlockSessionInsertOutcome::Inserted)
    );
    assert_eq!(
        cache.insert_qc_with_pops(prepare_qc.clone(), &pops),
        Ok(LaneBlockSessionInsertOutcome::Inserted)
    );
    assert_eq!(
        cache.insert_qc_with_pops(commit_qc.clone(), &pops),
        Ok(LaneBlockSessionInsertOutcome::Inserted)
    );
    assert_eq!(
        cache.insert_proposal(proposal_b.clone()),
        Ok(LaneBlockSessionInsertOutcome::Inserted)
    );
    assert!(
        cache.get(&key_a).is_some(),
        "undrained certified lane-block session must survive cache backpressure"
    );
    assert!(
        cache.get(&key_b).is_some(),
        "ordinary sessions may coexist while committed evidence is protected"
    );
    assert_eq!(
        cache.len(),
        2,
        "protected committed evidence may temporarily exceed the ordinary cache capacity"
    );

    let committed = cache.drain_committed_sessions();
    assert_eq!(committed.len(), 1);
    assert_eq!(committed[0].proposal, proposal_a);
    assert_eq!(committed[0].prepare_qc, prepare_qc);
    assert_eq!(committed[0].commit_qc, commit_qc);

    assert_eq!(
        cache.insert_proposal(proposal_c),
        Ok(LaneBlockSessionInsertOutcome::Inserted)
    );
    assert!(
        cache.get(&key_a).is_none(),
        "drained committed evidence should become evictable again"
    );
    assert!(
        cache.get(&key_b).is_none(),
        "old uncommitted sessions should remain bounded after protected evidence drains"
    );
    assert!(cache.get(&key_c).is_some());
    assert_eq!(cache.len(), 1);
}

#[test]
fn aggregate_lane_block_votes_rejects_adversarial_vote_sets() {
    let keys = [
        checked_bls_keypair(1),
        checked_bls_keypair(2),
        checked_bls_keypair(3),
    ];
    let outsider = checked_bls_keypair(9);
    let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
    validator_set.sort();
    let body = vote_body(&validator_set);
    let vote_a = signed_vote(&body, &keys[0]);
    let vote_b = signed_vote(&body, &keys[1]);

    assert_eq!(
        aggregate_lane_block_votes_to_qc(body.clone(), validator_set.clone(), &[]),
        Err(LaneBlockQcBuildError::EmptyVotes)
    );
    assert_eq!(
        aggregate_lane_block_votes_to_qc(
            body.clone(),
            validator_set.clone(),
            &[vote_a.clone(), vote_b.clone()]
        ),
        Err(LaneBlockQcBuildError::QuorumNotMet),
        "threshold minus one distinct signers must fail"
    );
    assert_eq!(
        aggregate_lane_block_votes_to_qc(
            body.clone(),
            validator_set.clone(),
            &[vote_a.clone(), vote_a.clone()],
        ),
        Err(LaneBlockQcBuildError::DuplicateSigner)
    );

    let outsider_vote = signed_vote(&body, &outsider);
    assert_eq!(
        aggregate_lane_block_votes_to_qc(
            body.clone(),
            validator_set.clone(),
            &[vote_a.clone(), outsider_vote],
        ),
        Err(LaneBlockQcBuildError::SignerNotInValidatorSet)
    );

    let mut body_drift = body.clone();
    body_drift.descriptor_hash = Hash::prehashed([0xFE; Hash::LENGTH]);
    let drift_vote = signed_vote(&body_drift, &keys[1]);
    assert_eq!(
        aggregate_lane_block_votes_to_qc(
            body.clone(),
            validator_set.clone(),
            &[vote_a.clone(), drift_vote],
        ),
        Err(LaneBlockQcBuildError::BodyMismatch)
    );

    let mut hash_drift = body.clone();
    hash_drift.validator_set_hash =
        HashOf::from_untyped_unchecked(Hash::prehashed([0xAA; Hash::LENGTH]));
    assert_eq!(
        aggregate_lane_block_votes_to_qc(
            hash_drift,
            validator_set.clone(),
            &[vote_a.clone(), vote_b.clone()]
        ),
        Err(LaneBlockQcBuildError::ValidatorSetHashMismatch)
    );

    let mut lowered_quorum = body.clone();
    lowered_quorum.min_quorum -= 1;
    let lowered_votes = keys
        .iter()
        .map(|key| signed_vote(&lowered_quorum, key))
        .collect::<Vec<_>>();
    assert_eq!(
        aggregate_lane_block_votes_to_qc(lowered_quorum, validator_set.clone(), &lowered_votes,),
        Err(LaneBlockQcBuildError::InvalidBody),
        "signed vote bodies must not lower the canonical committee threshold"
    );

    let mut reversed = validator_set;
    reversed.reverse();
    assert_eq!(
        aggregate_lane_block_votes_to_qc(body, reversed, &[vote_a, vote_b]),
        Err(LaneBlockQcBuildError::ValidatorSetNotCanonical)
    );
}

#[test]
fn compacted_new_view_checkpoint_rejects_forged_jump_and_replay_domains() {
    let keypairs = [
        checked_bls_keypair(11),
        checked_bls_keypair(12),
        checked_bls_keypair(13),
    ];
    let (network_id, epoch, payload) = autonomous_payload_fixture(&keypairs);
    let source = retarget_lane_block_proposal_exact_view(&payload.origin_proposal, 256)
        .expect("canonical checkpoint source");
    let target =
        retarget_lane_block_proposal_view(&source, 257).expect("canonical checkpoint target");
    let mut checkpoint = DurableLaneBlockViewCheckpointV1 {
        source_proposal: source.clone(),
        target_proposal: target,
        certificate: durable_new_view_certificate(&source, &payload, &keypairs, network_id, epoch),
    };

    checkpoint.source_proposal.descriptor.lane_block_view = 255;
    checkpoint.source_proposal.descriptor.descriptor_hash = checkpoint
        .source_proposal
        .descriptor
        .computed_descriptor_hash();
    checkpoint.source_proposal.proposal_hash = checkpoint.source_proposal.computed_proposal_hash();
    assert!(matches!(
        validate_lane_block_view_checkpoint(&checkpoint, &payload, network_id, epoch),
        Err(LaneAutonomousArtifactError::NewViewSourceMismatch)
    ));

    let valid_source = retarget_lane_block_proposal_exact_view(&payload.origin_proposal, 256)
        .expect("canonical checkpoint source");
    checkpoint.source_proposal = valid_source;
    assert!(matches!(
        validate_lane_block_view_checkpoint(
            &checkpoint,
            &payload,
            NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
                b"foreign-checkpoint-genesis",
            ))),
            epoch,
        ),
        Err(LaneAutonomousArtifactError::NetworkOrEpochMismatch)
    ));
    assert!(matches!(
        validate_lane_block_view_checkpoint(
            &checkpoint,
            &payload,
            network_id,
            epoch.saturating_add(1),
        ),
        Err(LaneAutonomousArtifactError::NetworkOrEpochMismatch)
    ));
}
