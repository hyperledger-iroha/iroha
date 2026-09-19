// Actual State admission staging and finalized-carrier source authority.

fn first_lane_input_fixture(seed: u8) -> (Box<LaneContextVerifiedFixture>, Vec<u8>) {
    let (state, validators, _, parent) = configured_lane_context_queue_plan_state();
    let (binding, control) = queue_plan_admission_certificate_for_state_test(
        &state,
        crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
            LaneId::SINGLE,
            DataSpaceId::UNIVERSAL,
        )),
        &validators,
        parent.header().height().get(),
        seed,
    );
    let (block, opening, witness) = publish_first_lane_input_fixture(&state, &parent, &control);
    (
        Box::new(LaneContextVerifiedFixture {
            state,
            validators,
            binding,
            block,
            opening,
            witness,
        }),
        control,
    )
}

// Keep the admission overlay and finality temporaries outside State/genesis
// construction's call stack. The completed fixture also stays on the heap when
// a reader test retains more than one independent State.
fn publish_first_lane_input_fixture(
    state: &State,
    parent: &SignedBlock,
    control: &[u8],
) -> (
    SignedBlock,
    iroha_data_model::block::consensus_v2::HeightContext,
    ExecWitness,
) {
    // Admission may be delayed after its signed proposal height. Preserve the
    // original binding while advancing the actual canonical source position.
    let advance = empty_global_block_after(Some(parent));
    commit_block_metadata_to_state(state, &advance);
    state.kura.store_block(Arc::new(advance.clone())).unwrap();
    let mut block = empty_global_block_after(Some(&advance));
    let admissions = vec![control.to_vec()];
    let mut context = block.execution_context().cloned().unwrap_or_default();
    context.queue_plan_admissions = admissions.clone();
    block.set_execution_context(Some(context));
    // Final admission controls change the proposal commitment and invalidate the
    // earlier result attachment. This carrier only admits inputs; retain an
    // explicit zero-work result and a real signature over the final proposal.
    assert_eq!(block.network_entrypoint_count(), 0);
    block
        .set_execution_outputs(
            Vec::new(),
            0,
            BTreeMap::new(),
            Vec::new(),
            AxtPolicySnapshot::default(),
            BTreeSet::new(),
            Vec::new(),
            &crate::execution_output_test_support::structural_output_limits(),
        )
        .unwrap();
    let carrier_key = merge_carrier_finality_fixture_keypair();
    block
        .replace_signatures(BTreeSet::from([
            iroha_data_model::block::BlockSignature::new(
                0,
                iroha_crypto::SignatureOf::from_hash(carrier_key.private_key(), block.hash()),
            ),
        ]))
        .unwrap();
    block.validate_proposal_commitments().unwrap();
    block.validate_execution_result_structure().unwrap();
    let opening = lane_opening_context_for_state_test(state);
    let mut overlay = state
        .block_with_queue_plan_admissions(block.header(), &admissions)
        .unwrap();
    overlay
        .finalize_lane_consensus_contexts(&block, Some(&opening))
        .unwrap();
    let mut witness = ExecWitness::default();
    overlay
        .capture_lane_consensus_contexts(&mut witness)
        .unwrap();
    overlay
        .stage_autoscale_sample_record_for_count(&block, 0)
        .expect("admission fixture retains its actual runtime predecessor");
    overlay.block_hashes.push(block.hash());
    insert_empty_transaction_block_for_state_commit(&mut overlay, &block);
    overlay.commit().unwrap();
    state.kura.store_block(Arc::new(block.clone())).unwrap();
    let (artifact, receipt) =
        stage_lane_context_fixture_finality(state, &block, opening.clone(), witness.clone());
    state
        .kura
        .promote_kagemusha_finality_sidecar(&artifact, &receipt)
        .unwrap();
    (block, opening, witness)
}

state_test! { sync first_lane_input_reader_authenticates_original_carrier_and_transport_completion
    use crate::sumeragi::{v2_chunks, v2_transport};
    use super::lane_admitted_input::FirstLaneAdmittedInputReadV1;
    let (fixture, control) = first_lane_input_fixture(0x71);
    let state = &fixture.state;
    let observed = state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &observed.contexts()[0];
    let FirstLaneAdmittedInputReadV1::Ready(local) = state.first_lane_admitted_input(&observed, lane).unwrap() else {
        panic!("actual finalized admission is locally readable");
    };
    assert_eq!(local.canonical_control_bytes(), control);
    assert_eq!(local.canonical_control_hash(), Hash::new(&control));
    assert_eq!(local.carrier_hash(), fixture.block.hash());
    assert_eq!(local.priority().carrier_height, fixture.block.header().height().get());
    assert_eq!(local.priority().admission_index, 0);
    assert_ne!(local.priority().carrier_height, fixture.binding.admission_context.proposal_height,
        "the signed ingress proposal height is not the actual admission rank");
    assert_eq!(local.validated_input().certificate().binding_hash, fixture.binding.canonical_hash());
    let finality = local.source().finality();
    let requester_key = &fixture.validators[0];
    let requester = PeerId::new(requester_key.public_key().clone());
    let mut request = iroha_data_model::block::consensus_v2::CertifiedBodyRequest {
        round: finality.commit_qc.proposal_round, subject: finality.subject,
        certificate: finality.commit_qc.clone(), requester: requester.clone(), signature: vec![],
    };
    request.signature = Signature::new(requester_key.private_key(), &request.signature_preimage()).payload().to_vec();
    let request = v2_transport::authenticate_certified_body_request_with_validator_pops(
        &finality.height_context, &finality.validator_set_pops, request, &requester,
    ).unwrap();
    let body = fixture.block.canonical_resultless_proposal().encode_wire().unwrap();
    assert!(fixture.block.has_results(), "local source is the executed image");
    assert_ne!(body, fixture.block.encode_wire().unwrap());
    let (manifest, _) = v2_chunks::encode_payload(
        &finality.height_context, request.request().round, finality.subject, &body,
    ).unwrap().into_parts();
    let mut response = iroha_data_model::block::consensus_v2::CertifiedBodyResponse {
        request_hash: request.request_hash(), manifest, body, responder: requester.clone(), signature: vec![],
    };
    response.signature = Signature::new(requester_key.private_key(), &response.signature_preimage()).payload().to_vec();
    let mut outstanding = v2_transport::OutstandingCertifiedBodyRequests::new(1).unwrap();
    outstanding.register(request.clone()).unwrap();
    let authenticated = outstanding.authenticate_response(&finality.height_context, response.clone(), &requester).unwrap();
    let recovered = local.source().complete_from_authenticated_response(&request, &authenticated).unwrap();
    assert_eq!(recovered.canonical_control_bytes(), local.canonical_control_bytes());
    assert_eq!(recovered.carrier_hash(), local.carrier_hash());
    assert_eq!(recovered.priority(), local.priority());
    assert_eq!(outstanding.len(), 1, "projection cannot acknowledge or drop request custody");
    let (other, _) = first_lane_input_fixture(0x72);
    let other_observed = other.state.verified_lane_consensus_contexts().unwrap().unwrap();
    let FirstLaneAdmittedInputReadV1::Ready(other_source) = other.state.first_lane_admitted_input(
        &other_observed, &other_observed.contexts()[0],
    ).unwrap() else { panic!("second actual source"); };
    assert!(other_source.source().complete_from_authenticated_response(&request, &authenticated).is_err(),
        "a valid response for another source cannot cross the private source binding");
    response.body[0] ^= 1;
    assert!(outstanding.authenticate_response(&finality.height_context, response, &requester).is_err());
    assert_eq!(outstanding.len(), 1, "malformed response preserves exact request ownership");
}

state_test! { sync first_lane_input_reader_rejects_missing_proof_and_false_source_index
    use super::lane_admitted_input::FirstLaneAdmittedInputReadV1;
    let (fixture, _) = first_lane_input_fixture(0x73);
    let state = &fixture.state;
    let observed = state.verified_lane_consensus_contexts().unwrap().unwrap();
    let path = state.kura.v2_finality_artifact_path_for_testing(fixture.block.header().height().get());
    let exact = std::fs::read(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    assert!(state.first_lane_admitted_input(&observed, &observed.contexts()[0]).is_err(),
        "missing required historical finality is not a body-recovery retry");
    std::fs::write(&path, &exact).unwrap();
    let mut corrupt = exact.clone();
    corrupt[0] ^= 1;
    std::fs::write(&path, &corrupt).unwrap();
    assert!(state.first_lane_admitted_input(&observed, &observed.contexts()[0]).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), corrupt, "read cannot repair occupied corruption");
    std::fs::write(&path, exact).unwrap();
    let inconsistent = lane_context_verified_fixture();
    let (artifact, receipt) = stage_lane_context_fixture_finality(
        &inconsistent.state, &inconsistent.block, inconsistent.opening, inconsistent.witness,
    );
    inconsistent.state.kura.promote_kagemusha_finality_sidecar(&artifact, &receipt).unwrap();
    let false_source = inconsistent.state.verified_lane_consensus_contexts().unwrap().unwrap();
    assert!(inconsistent.state.first_lane_admitted_input(&false_source, &false_source.contexts()[0]).is_err(),
        "fixture-only raw registry seeding cannot supply an admission omitted from the actual carrier");
    assert!(matches!(state.first_lane_admitted_input(&observed, &false_source.contexts()[0]).unwrap(),
        FirstLaneAdmittedInputReadV1::InstanceNotCurrent));
}

state_test! { sync first_lane_input_reader_drops_state_guards_and_rejects_changed_publication
    use super::lane_admitted_input::FirstLaneAdmittedInputReadV1;
    let (fixture, _) = first_lane_input_fixture(0x74);
    let state = Arc::new(fixture.state);
    let observed = state.verified_lane_consensus_contexts().unwrap().unwrap();
    let change = Arc::clone(&state);
    let successor = empty_global_block_after(Some(&fixture.block));
    let result = super::lane_consensus_verified::io_observer::observe(move || {
        let writer = Arc::clone(&change);
        let header = successor.header();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let task = std::thread::spawn(move || {
            let _lease = writer.consensus_publication_lease();
            writer.append_committed_block_header_for_tests(header);
            let world = writer.world.block();
            world.commit();
            done_tx.send(()).unwrap();
        });
        done_rx.recv_timeout(Duration::from_secs(5)).expect("source read retained no State/MV guards");
        task.join().unwrap();
    }, || state.first_lane_admitted_input(&observed, &observed.contexts()[0]));
    assert!(matches!(result.unwrap(), FirstLaneAdmittedInputReadV1::ObservationChanged));
}
