// Commit-request shape and QC replay-validation tests.
//
// Included by `native_amx::tests` to preserve exact libtest names.
#[test]
fn commit_request_shape_binds_the_exact_round_and_epoch() {
    let keys = [checked_bls_keypair(0x41), checked_bls_keypair(0x42)];
    let mut validators = keys
        .iter()
        .map(|key| PeerId::new(key.public_key().clone()))
        .collect::<Vec<_>>();
    validators.sort();
    let prepare_request = full_plan_request(
        body_for_validator_set(NativeAmxPhase::Prepare, &validators),
        validators.clone(),
    );
    let prepare_body = prepare_request.body;
    let validator_set_pops = aligned_pops(&validators, &keys);
    let votes = keys
        .iter()
        .map(|key| signed_vote(&prepare_body, key))
        .collect::<Vec<_>>();
    let prepare_qc = aggregate_votes_to_qc(
        prepare_body,
        validators.clone(),
        validator_set_pops,
        &votes,
        2,
    )
    .expect("prepare QC");
    let mut commit_request = prepare_request;
    commit_request.body.phase = NativeAmxPhase::Commit;
    let request = NativeAmxCommitRequestV2 {
        request: commit_request,
        prepare_qc: prepare_qc.clone(),
    };
    assert_eq!(request.validate_shape(), Ok(()));
    let mut replayed_view = request.clone();
    replayed_view.request.body.round.view = replayed_view.request.body.round.view.saturating_add(1);
    replayed_view.request.body.coordinator_lane_block_view = replayed_view
        .request
        .body
        .coordinator_lane_block_view
        .saturating_add(1);
    replayed_view
        .request
        .coordinator_proposal
        .descriptor
        .lane_block_view = replayed_view.request.body.coordinator_lane_block_view;
    replayed_view
        .request
        .coordinator_proposal
        .descriptor
        .descriptor_hash = replayed_view
        .request
        .coordinator_proposal
        .descriptor
        .computed_descriptor_hash();
    replayed_view.request.coordinator_proposal.proposal_hash = replayed_view
        .request
        .coordinator_proposal
        .computed_proposal_hash();
    replayed_view.request.body.coordinator_proposal_hash =
        replayed_view.request.coordinator_proposal.proposal_hash;
    assert_eq!(
        replayed_view.validate_shape(),
        Err(NativeAmxCommitRequestError::LegMismatch)
    );
    let mut replayed_epoch = request;
    replayed_epoch.request.body.epoch = replayed_epoch.request.body.epoch.saturating_add(1);
    assert_eq!(
        replayed_epoch.validate_shape(),
        Err(NativeAmxCommitRequestError::LegMismatch)
    );
}
#[test]
fn qc_validation_rejects_context_replay_and_missing_pop() {
    let keys = [
        checked_bls_keypair(0x51),
        checked_bls_keypair(0x52),
        checked_bls_keypair(0x53),
    ];
    let mut validators = keys
        .iter()
        .map(|key| PeerId::new(key.public_key().clone()))
        .collect::<Vec<_>>();
    validators.sort();
    let body = body_for_validator_set(NativeAmxPhase::Prepare, &validators);
    let validator_set_pops = aligned_pops(&validators, &keys);
    let votes = keys
        .iter()
        .map(|key| signed_vote(&body, key))
        .collect::<Vec<_>>();
    let qc = aggregate_votes_to_qc(body, validators.clone(), validator_set_pops, &votes, 3)
        .expect("aggregate exact QC");
    let pops = keys
        .iter()
        .map(|key| {
            (
                key.public_key().clone(),
                iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("prove PoP"),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        validate_native_amx_qc(&qc, &body, &validators, 3, &pops),
        Ok(())
    );
    let mut another_context = body;
    another_context.round.context_id = HeightContextId(
        HashOf::<HeightContext>::from_untyped_unchecked(Hash::new(b"replayed-context")),
    );
    assert_eq!(
        validate_native_amx_qc(&qc, &another_context, &validators, 3, &pops),
        Err(NativeAmxQcValidationError::BodyMismatch)
    );
    let mut missing_pop = pops;
    missing_pop.remove(keys[0].public_key());
    assert_eq!(
        validate_native_amx_qc(&qc, &body, &validators, 3, &missing_pop),
        Err(NativeAmxQcValidationError::InvalidProofOfPossession)
    );
}
#[test]
fn vote_signature_rejects_same_label_foreign_genesis_network() {
    let shared_label_a: iroha_data_model::ChainId =
        "shared-display-label".parse().expect("valid display label");
    let shared_label_b: iroha_data_model::ChainId =
        "shared-display-label".parse().expect("valid display label");
    assert_eq!(shared_label_a, shared_label_b);
    let keypair = checked_bls_keypair(0xE7);
    let local_body = body(NativeAmxPhase::Prepare);
    let mut replayed = signed_vote(&local_body, &keypair);
    replayed.body.network_id = network_id(b"foreign-genesis-with-shared-label");
    assert_ne!(
        local_body.signature_preimage(),
        replayed.body.signature_preimage(),
    );
    assert_eq!(
        replayed.verify_signature(),
        Err(NativeAmxVoteIngressError::InvalidSignature),
    );
}
