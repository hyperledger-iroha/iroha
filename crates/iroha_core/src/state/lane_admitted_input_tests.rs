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
    let mut inconsistent = lane_context_verified_fixture();
    assert_eq!(inconsistent.block.execution_context().unwrap().queue_plan_admissions.len(), 1);
    // Authenticate an explicitly false source position: the real carrier has
    // index zero only, while its signed context witness claims index one.
    let mut contexts = inconsistent.state.lane_consensus_contexts.view().get().clone();
    contexts.contexts[0].admission_priority.admission_index = 1;
    let commitment = LaneConsensusContextsCommitmentV1::from_contexts(
        inconsistent.state.network_id, inconsistent.block.header().height().get(), &contexts,
    ).unwrap();
    inconsistent.witness.writes.iter_mut().find(|entry|
        entry.key == super::lane_consensus_state::LANE_CONSENSUS_CONTEXTS_WITNESS_KEY
    ).unwrap().value = norito::to_bytes(&commitment).unwrap();
    let mut cell = inconsistent.state.lane_consensus_contexts.block();
    *cell.get_mut() = contexts;
    cell.commit();
    let (artifact, receipt) = stage_lane_context_fixture_finality(
        &inconsistent.state, &inconsistent.block, inconsistent.opening, inconsistent.witness,
    );
    inconsistent.state.kura.promote_kagemusha_finality_sidecar(&artifact, &receipt).unwrap();
    let false_source = inconsistent.state.verified_lane_consensus_contexts().unwrap().unwrap();
    assert_eq!(inconsistent.state.first_lane_admitted_input(&false_source, &false_source.contexts()[0]).unwrap_err(),
        "first-admission canonical index is absent from its carrier",
        "signed context witness cannot supply an admission omitted from the actual carrier");
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

state_test! { sync canonical_queue_plan_input_reads_exact_first_carrier_pending_and_applied
    let (fixture, control) = first_lane_input_fixture(0x91);
    let state = &fixture.state;
    let hash = fixture.binding.entrypoint_hash;
    let before = exact_test_tree_fingerprint(&state.kura.store_root());
    let generation = state.state_view_generation();
    let first = state.canonical_queue_plan_admitted_input(hash).unwrap().unwrap();
    assert_eq!(norito::encode_canonical(first.input()).unwrap(), control);
    assert_eq!(first.certificate().certificate.binding, fixture.binding);
    assert_eq!(first.entrypoint().hash(), hash);
    assert_eq!(state.canonical_queue_plan_admitted_input(hash).unwrap(), Some(first.clone()));
    assert!(State::queue_plan_pending_binding_in_view(&state.view(), hash).unwrap().is_some());
    assert_eq!(state.state_view_generation(), generation);
    assert_eq!(exact_test_tree_fingerprint(&state.kura.store_root()), before);

    let absent = HashOf::from_untyped_unchecked(Hash::new(b"absent canonical admission"));
    assert_eq!(state.canonical_queue_plan_admitted_input(absent).unwrap(), None);
    // The actual first carrier remains the source after the existing explicit
    // fixture boundary resolves every pending marker and commits membership.
    // This does not claim transaction execution or a second finality decision.
    state.record_committed_queue_plan_entrypoints_for_tests(
        [hash], NonZeroUsize::new(state.committed_height()).unwrap(),
    ).unwrap();
    assert_eq!(State::queue_plan_pending_binding_in_view(&state.view(), hash).unwrap(), None);
    assert_eq!(state.canonical_queue_plan_admitted_input(hash).unwrap(), Some(first));
    assert_eq!(exact_test_tree_fingerprint(&state.kura.store_root()), before);
}

state_test! { large_stack canonical_queue_plan_input_rejects_wrong_rank_claim_and_orphan
    let (fixture, _) = first_lane_input_fixture(0x92);
    let state = &fixture.state;
    let binding = &fixture.binding;
    let key = State::queue_plan_admission_registry_marker_key(&binding.registry_key()).unwrap();
    let original = state.world.smart_contract_state.view().get(&key).unwrap().clone();
    let record = State::decode_exact_queue_plan_admission_registry_record(&key, &original).unwrap();
    let before = exact_test_tree_fingerprint(&state.kura.store_root());
    for (height, index) in [
        (record.priority.carrier_height, 1),
        (record.priority.carrier_height + 1, 0),
        (record.priority.carrier_height - 1, 0),
    ] {
        let mut world = state.world.block();
        world.smart_contract_state.insert(key.clone(),
            State::queue_plan_admission_registry_marker_payload(
                &record.claim, QueuePlanAdmissionPriorityV1::new(height, index).unwrap(),
            ).unwrap());
        world.commit();
        assert!(state.canonical_queue_plan_admitted_input(binding.entrypoint_hash).is_err(),
            "wrong first-carrier position ({height}, {index}) cannot yield an input");
    }
    let mut other_claim = record.claim;
    other_claim.binding_hash = Hash::new(b"another canonical binding");
    let mut world = state.world.block();
    world.smart_contract_state.insert(key.clone(),
        State::queue_plan_admission_registry_marker_payload(&other_claim, record.priority).unwrap());
    world.commit();
    assert!(state.canonical_queue_plan_admitted_input(binding.entrypoint_hash).is_err());
    let mut world = state.world.block();
    world.smart_contract_state.remove(key.clone());
    world.commit();
    assert!(state.canonical_queue_plan_admitted_input(binding.entrypoint_hash).is_err(),
        "an orphaned pending obligation is never genuine registry absence");
    let mut world = state.world.block();
    world.smart_contract_state.insert(key, original);
    world.commit();
    assert!(state.canonical_queue_plan_admitted_input(binding.entrypoint_hash).unwrap().is_some());
    assert_eq!(exact_test_tree_fingerprint(&state.kura.store_root()), before);
}

state_test! { sync canonical_queue_plan_input_requires_original_finality_and_available_body
    let (fixture, _) = first_lane_input_fixture(0x93);
    let state = &fixture.state;
    let hash = fixture.binding.entrypoint_hash;
    let path = state.kura.v2_finality_artifact_path_for_testing(fixture.block.header().height().get());
    let exact = std::fs::read(&path).unwrap();
    let generation = state.state_view_generation();
    let block_hash = state.latest_block_hash_fast();
    std::fs::remove_file(&path).unwrap();
    assert!(state.canonical_queue_plan_admitted_input(hash).is_err());
    let mut corrupt = exact.clone();
    corrupt[0] ^= 1;
    std::fs::write(&path, &corrupt).unwrap();
    assert!(state.canonical_queue_plan_admitted_input(hash).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), corrupt, "read cannot repair occupied corruption");
    let (foreign, _) = first_lane_input_fixture(0x94);
    let foreign_path = foreign.state.kura.v2_finality_artifact_path_for_testing(
        foreign.block.header().height().get(),
    );
    let foreign_bytes = std::fs::read(foreign_path).unwrap();
    assert_ne!(foreign.block.hash(), fixture.block.hash());
    std::fs::write(&path, &foreign_bytes).unwrap();
    assert!(state.canonical_queue_plan_admitted_input(hash).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), foreign_bytes);
    std::fs::write(&path, exact).unwrap();
    assert!(state.canonical_queue_plan_admitted_input(hash).unwrap().is_some());
    state.kura.evict_first_admission_body_for_testing(
        NonZeroUsize::new(usize::try_from(fixture.block.header().height().get()).unwrap()).unwrap(),
        fixture.block.hash(),
    ).unwrap();
    assert_eq!(state.canonical_queue_plan_admitted_input(hash).unwrap_err(),
        "canonical QueuePlan first-carrier body requires authenticated recovery");
    assert_eq!(state.state_view_generation(), generation);
    assert_eq!(state.latest_block_hash_fast(), block_hash);
    assert_eq!(state.queue_plan_admission_binding_registry_match(&fixture.binding).unwrap(),
        QueuePlanAdmissionRegistryMatch::Exact);
}

state_test! { sync canonical_queue_plan_input_releases_state_guards_and_rejoins_original_registry
    let (fixture, _) = first_lane_input_fixture(0x95);
    let state = Arc::new(fixture.state);
    let hash = fixture.binding.entrypoint_hash;
    let key = State::queue_plan_admission_registry_marker_key(&fixture.binding.registry_key()).unwrap();
    let original = state.world.smart_contract_state.view().get(&key).unwrap().clone();
    let record = State::decode_exact_queue_plan_admission_registry_record(&key, &original).unwrap();
    let successor = empty_global_block_after(Some(&fixture.block));
    let writer = Arc::clone(&state);
    let result = super::lane_consensus_verified::io_observer::observe(move || {
        let writer = Arc::clone(&writer);
        let header = successor.header();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let task = std::thread::spawn(move || {
            let _lease = writer.consensus_publication_lease();
            writer.append_committed_block_header_for_tests(header);
            let world = writer.world.block();
            world.commit();
            let _ = done_tx.send(());
        });
        done_rx.recv_timeout(Duration::from_secs(5)).expect("no State guards survive into Kura I/O");
        task.join().unwrap();
    }, || state.canonical_queue_plan_admitted_input(hash));
    assert!(result.unwrap().is_some(), "unrelated publication preserves the original source");
    let changed = Arc::clone(&state);
    let changed_key = key.clone();
    let result = crate::torii_proxy::observe_queue_plan_authentication_for_test(move || {
        // This hook runs after the actual Kura read, while the original body is
        // authenticated. Its mutation must be seen by the final registry join.
        let mut world = changed.world.block();
        world.smart_contract_state.insert(changed_key.clone(),
            State::queue_plan_admission_registry_marker_payload(&record.claim,
                QueuePlanAdmissionPriorityV1::new(record.priority.carrier_height, 1).unwrap(),
            ).unwrap());
        world.commit();
    }, || state.canonical_queue_plan_admitted_input(hash));
    assert_eq!(result.unwrap_err(), "canonical QueuePlan registry source changed during carrier read");
    let mut world = state.world.block();
    world.smart_contract_state.insert(key, original);
    world.commit();
    assert!(state.canonical_queue_plan_admitted_input(hash).unwrap().is_some());
}

state_test! { sync canonical_queue_plan_input_component_carrier_requires_separate_finality
    let (state, validators, _, parent) = configured_lane_context_queue_plan_state();
    let (binding, control) = queue_plan_admission_certificate_for_state_test(
        &state,
        crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
            LaneId::SINGLE, DataSpaceId::UNIVERSAL,
        )),
        &validators, parent.header().height().get(), 0x96,
    );
    // This component helper accepts admission-only metadata. The generic
    // lane-opening fixture also owns DA policies, so build this narrower real
    // proposal from the same parent and complete input instead.
    let header = BlockHeader::new(
        parent.header().height().checked_add(1).unwrap(),
        Some(parent.hash()), None, parent.header().creation_time_ms + 1, 0,
    );
    let mut builder = iroha_data_model::block::builder::BlockBuilder::new(header);
    builder.set_execution_context(Some(
        iroha_data_model::block::BlockExecutionContextBundle::default()
            .with_queue_plan_admissions(vec![control.clone()]),
    ));
    let key = merge_carrier_finality_fixture_keypair();
    let mut carrier = builder.build_with_signature(0, key.private_key());
    carrier.set_execution_outputs(
        Vec::new(), 0, BTreeMap::new(), Vec::new(), AxtPolicySnapshot::default(),
        Default::default(), Vec::new(),
        &crate::execution_output_test_support::structural_output_limits(),
    ).unwrap();
    carrier.validate_proposal_commitments().unwrap();
    carrier.validate_execution_result_structure().unwrap();
    assert_eq!(carrier.header().prev_block_hash(), Some(parent.hash()));
    assert!(carrier.da_proof_policies().is_none());
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    assert!(state.commit_queue_plan_admission_carrier_for_testing(
        &carrier.canonical_resultless_proposal(),
    ).is_err(), "the component helper cannot manufacture a result-bearing image");
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(), before);
    state.commit_queue_plan_admission_carrier_for_testing(&carrier).unwrap();
    assert_eq!(state.latest_block_hash_fast(), Some(carrier.hash()));
    assert_eq!(state.queue_plan_admission_binding_registry_match(&binding).unwrap(),
        QueuePlanAdmissionRegistryMatch::Exact);
    assert!(state.canonical_queue_plan_admitted_input(binding.entrypoint_hash).is_err(),
        "component State staging alone is not authenticated body/finality custody");
    state.kura.store_block(Arc::new(carrier.clone())).unwrap();
    assert!(state.canonical_queue_plan_admitted_input(binding.entrypoint_hash).is_err(),
        "an exact stored body alone is not finality");
    let mut previous = None;
    for height in 1..=state.committed_height() {
        let block = state.kura.get_block(NonZeroUsize::new(height).unwrap()).unwrap();
        let artifact = merge_carrier_finality_artifact_with_network(
            &block, previous.as_ref(), state.network_id,
        );
        let _receipt = state.kura.store_v2_finality_artifact(&artifact).unwrap();
        previous = Some(artifact);
    }
    let admitted = state.canonical_queue_plan_admitted_input(binding.entrypoint_hash).unwrap().unwrap();
    assert_eq!(norito::encode_canonical(admitted.input()).unwrap(), control);
    assert_eq!(admitted.certificate().certificate.binding, binding);
}

state_test! { large_stack canonical_queue_plan_input_retains_first_carrier_after_real_lane_close
    let (state, validators, _, parent) = configured_lane_context_queue_plan_state();
    let lane_id = LaneId::new(1);
    install_autoscale_elastic_catalog_for_test(&state,
        autoscale_elastic_catalog_lane_with_committee_for_test(lane_id, 1, &validators));
    install_lane_manifest_registry_for_keypairs(&state, &[LaneId::SINGLE, lane_id], &validators);
    let plan = crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
        lane_id, DataSpaceId::UNIVERSAL,
    ));
    let (binding, control) = queue_plan_admission_certificate_for_state_test(
        &state, plan.clone(), &validators, parent.header().height().get(), 0x97,
    );
    let (first, _, _) = publish_first_lane_input_fixture(&state, &parent, &control);
    let original = state.canonical_queue_plan_admitted_input(binding.entrypoint_hash).unwrap().unwrap();
    let close = empty_global_block_after(Some(&first));
    let mut overlay = state.block(close.header());
    let capacity = autoscale_default_route_capacity_lanes(
        &overlay.nexus.routing_policy, overlay.nexus.lane_catalog.lanes(),
        overlay.nexus.autoscale.min_lane_id.get(), overlay.nexus.autoscale.max_lane_id_exclusive.get(),
    );
    overlay.stage_autoscale_lane_drain_intent(lane_id, capacity, capacity, 0, 0).unwrap();
    overlay.record_autoscale_transition_height(close.header().height().get());
    state.validate_committed_autoscale_lane_lifecycle(
        overlay.pending_autoscale_lifecycle.as_ref().unwrap(),
        close.header().height().get(), close.hash(), None,
    ).unwrap();
    overlay.block_hashes.push(close.hash());
    insert_empty_transaction_block_for_state_commit(&mut overlay, &close);
    overlay.commit().unwrap();
    state.kura.store_block(Arc::new(close.clone())).unwrap();
    assert!(crate::queue::queue_plan_authoritative_peers_in_view_at_height(
        &state.view(), plan.coordinator_route(), close.header().height().get() + 1,
    ).is_err(), "fresh admission is closed");
    assert_eq!(State::queue_plan_pending_route_authority_in_view(&state.view(), &binding).unwrap(),
        Some(QueuePlanPendingRouteAuthority::Draining));
    let before = exact_test_tree_fingerprint(&state.kura.store_root());
    assert_eq!(state.canonical_queue_plan_admitted_input(binding.entrypoint_hash).unwrap(), Some(original));
    assert_eq!(state.latest_block_hash_fast(), Some(close.hash()));
    assert_eq!(exact_test_tree_fingerprint(&state.kura.store_root()), before);
}

state_test! { sync canonical_queue_plan_input_enforces_cumulative_read_budget
    let (fixture, control) = first_lane_input_fixture(0x98);
    let state = &fixture.state;
    let hash = fixture.binding.entrypoint_hash;
    let budget = State::canonical_queue_plan_input_decode_limits().unwrap();
    let body = norito::canonical_decode_limits(
        usize::try_from(iroha_data_model::block::consensus_v2::MAX_EXECUTED_BLOCK_WIRE_BYTES).unwrap(),
    );
    let kura = crate::kura::canonical_admission_read_decode_limits().unwrap();
    assert!(kura.max_total_allocated_bytes() > body.max_total_allocated_bytes(),
        "valid maximum-body allocations retain their complete allowance after metadata reads");
    assert!(budget.max_total_allocated_bytes() > kura.max_total_allocated_bytes(),
        "pre-read and post-read State observations have separate cumulative allowances");
    assert!(State::canonical_queue_plan_input_read_working_set_bytes().unwrap()
        > budget.max_total_allocated_bytes());
    let generation = state.state_view_generation();
    let before = exact_test_tree_fingerprint(&state.kura.store_root());
    let refused = norito::with_decode_limits_scope(
        norito::DecodeLimits::new(
            budget.max_sequence_elements(), budget.max_field_bytes(),
            budget.max_total_elements(), 0, budget.max_nesting_depth(),
        ),
        || state.canonical_queue_plan_admitted_input(hash),
    );
    assert!(refused.is_err(), "a nested reader cannot raise the original allocator budget");
    let admitted = state.canonical_queue_plan_admitted_input(hash).unwrap().unwrap();
    assert_eq!(norito::encode_canonical(admitted.input()).unwrap(), control,
        "a refused scoped decode must release its budget before the exact retry");
    assert_eq!(state.state_view_generation(), generation);
    assert_eq!(exact_test_tree_fingerprint(&state.kura.store_root()), before);
}
