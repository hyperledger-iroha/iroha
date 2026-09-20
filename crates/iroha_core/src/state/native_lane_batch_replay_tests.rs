// Inactive canonical inclusion and exact pre-State replay boundaries. Actual
// native economic publication/Apply remains disabled, including in these tests.

// Storage fixtures project actual prefix outputs only. This does not execute the
// common Time/final-witness tail, authorize State publication or mint native Apply.
fn attach_actual_native_prefix_results_for_test(
    carrier: &mut SignedBlock,
    prepared: &super::lane_decision_batch::PreparedLaneDecisionBatchV1<'_>,
) {
    let hashes = prepared
        .executions()
        .iter()
        .map(|execution| execution.source.payload.input.entrypoint.hash())
        .collect::<Vec<_>>();
    let results = prepared
        .executions()
        .iter()
        .map(|execution| execution.result.clone())
        .collect::<Vec<_>>();
    // Shared start hooks may own additional real protocol evidence. Native
    // extraction currently exports its rows separately; retained custody must
    // agree byte-for-byte when those rows also remain on this same overlay.
    let mut transcripts = prepared.overlay().fastpq_transcripts.clone();
    for execution in prepared.executions() {
        let input = &execution.source.payload.input.entrypoint;
        for bundle in &execution.fastpq_transcripts {
            assert_eq!(bundle.entry_hash, Hash::from(input.hash()));
            let call = Hash::from(input.execution_call_hash());
            assert!(!bundle.transcripts.is_empty());
            if let Some(retained) = transcripts.insert(call, bundle.transcripts.clone()) {
                assert_eq!(retained, bundle.transcripts);
            }
        }
    }
    let proposal_hash = carrier.hash();
    assert!(carrier.network_input_hashes().eq(hashes.iter().copied()));
    let outputs = results
        .into_iter()
        .enumerate()
        .map(|(index, result)| {
            iroha_data_model::block::execution_output::ExecutionOutputV1::Network(
                iroha_data_model::block::execution_output::NetworkExecutionOutputV1 {
                    input_index: u32::try_from(index).unwrap(),
                    result,
                    completions: Vec::new(),
                },
            )
        })
        .collect();
    carrier
        .set_execution_outputs(
            outputs,
            u64::try_from(prepared.overlay().committed_fragment_count()).unwrap(),
            transcripts,
            prepared.overlay().axt_envelopes().to_vec(),
            prepared.overlay().axt_policy_snapshot(),
            prepared.overlay().axt_authorization_transitioned().clone(),
            Vec::new(),
            &crate::execution_output_test_support::structural_output_limits(),
        )
        .expect("actual native outputs join their source positions");
    assert_eq!(carrier.hash(), proposal_hash);
    carrier.validate_native_lane_results().unwrap();
}

fn retained_native_batch_fixture() -> (
    Box<NativeEconomicFixture>,
    SignedBlock,
    crate::kura::FinalizedNativeLaneBatchV1,
) {
    retained_native_batch_fixture_with_substitution(false)
}

fn retained_native_batch_fixture_with_substitution(
    substitute: bool,
) -> (
    Box<NativeEconomicFixture>,
    SignedBlock,
    crate::kura::FinalizedNativeLaneBatchV1,
) {
    retained_native_batch_fixture_with_cases(&[NativeEconomicCase::Transfer(25)], substitute)
}

fn retained_native_batch_fixture_with_cases(
    cases: &[NativeEconomicCase],
    substitute: bool,
) -> (
    Box<NativeEconomicFixture>,
    SignedBlock,
    crate::kura::FinalizedNativeLaneBatchV1,
) {
    let fixture = native_economic_fixture_with_genesis_layout(
        cases,
        false,
        Some(DataAvailabilityLayout {
            encoding: PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 8192,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 2 * 1024 * 1024,
            max_chunk_count: 512,
        }),
    );
    let (carrier, included) = retain_native_batch_fixture(&fixture, substitute);
    (fixture, carrier, included)
}

// Reserve carrier overlays only after economic State/genesis construction has
// returned; the retained input fixture remains on the heap throughout staging.
fn retain_native_batch_fixture(
    fixture: &NativeEconomicFixture,
    substitute: bool,
) -> (SignedBlock, crate::kura::FinalizedNativeLaneBatchV1) {
    use crate::kura::NativeLaneBatchCarrierReadV1;
    let state = &fixture.native.state;
    seed_autoscale_sample_history_for_snapshot_test(state);
    let groups = native_economic_groups(fixture);
    let mut carrier = empty_global_block_after(Some(&fixture.native.block));
    carrier.set_da_proof_policies(Some(crate::da::active_proof_policy_bundle_at_height(
        &state.nexus_snapshot(),
        carrier.header().height().get(),
    )));
    let mut batch = state.prepare_lane_decision_batch(&groups).unwrap();
    if substitute {
        let source = &mut batch.groups[0];
        source.payload.descriptor.admission_carrier_hash = HashOf::from_untyped_unchecked(
            Hash::new(b"another first carrier despite authentic native signatures"),
        );
        resign_changed_native_group_payload_for_test(&fixture.native, source);
    }
    carrier.set_execution_context(Some(
        iroha_data_model::block::BlockExecutionContextBundle::default()
            .with_native_lane_decisions(batch.clone()),
    ));
    let prepared = state
        .prepare_native_batch_on_carrier(carrier.header(), groups.clone())
        .unwrap();
    attach_actual_native_prefix_results_for_test(&mut carrier, &prepared);
    drop(prepared); // release the prefix before unrelated Kura/finality fixture work
    // This is a storage/finality fixture, not production acceptance of native
    // carriers. Their authenticated inputs are subsequently re-executed by the tested API.
    assert!(
        crate::block::ValidBlock::validate_inactive_native_carrier_for_test(&carrier)
            .unwrap_err()
            .to_string()
            .contains("not active")
    );
    state.kura.store_block(Arc::new(carrier.clone())).unwrap();
    let parent = state
        .kura
        .v2_finality_artifact(fixture.native.block.header().height().get())
        .unwrap()
        .unwrap();
    let opening = crate::sumeragi::v2_context::build_successor_height_context(
        &parent,
        parent.height_context.nexus_amx_context_hash,
        None,
    )
    .unwrap();
    let mut overlay = state.block(carrier.header());
    overlay
        .finalize_lane_consensus_contexts(&carrier, Some(&opening))
        .unwrap();
    let mut witness = ExecWitness::default();
    overlay
        .capture_lane_consensus_contexts(&mut witness)
        .unwrap();
    drop(overlay); // never publish a fake native application
    let (artifact, receipt) =
        stage_lane_context_fixture_finality(state, &carrier, opening, witness);
    state
        .kura
        .promote_kagemusha_finality_sidecar(&artifact, &receipt)
        .unwrap();
    let NativeLaneBatchCarrierReadV1::Ready(included) = state
        .kura
        .read_finalized_native_lane_batch(
            NonZeroUsize::new(carrier.header().height().get() as usize).unwrap(),
            carrier.hash(),
        )
        .unwrap()
    else {
        panic!("locally retained canonical carrier");
    };
    assert_eq!(included.batch(), &batch);
    (carrier, included)
}

state_test! { sync historical_native_batch_replays_only_on_exact_pre_state_without_publication
    use super::NativeLaneBatchReplayV1;
    let (fixture, carrier, included) = retained_native_batch_fixture();
    let state = &fixture.native.state;
    let before = crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot");
    let NativeLaneBatchReplayV1::Ready(replayed) =
        state.replay_finalized_native_lane_batch(&included, &[]).unwrap()
        else { panic!("exact private sources re-enter economic replay"); };
    assert_eq!(replayed.batch(), included.batch());
    assert_eq!(replayed.overlay().world.assets.get(&fixture.source).unwrap().0, Quantity::from(75u32));
    assert_eq!(replayed.overlay().world.assets.get(&fixture.destination).unwrap().0, Quantity::from(25u32));
    drop(replayed);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot"), before);
    let other = native_economic_fixture(&[NativeEconomicCase::Transfer(30)], false);
    assert!(other.native.state.replay_finalized_native_lane_batch(&included, &[]).is_err(),
        "same-looking roster or height is not the exact first source/base");
    assert!(state.kura.read_finalized_native_lane_batch(
        NonZeroUsize::new(carrier.header().height().get() as usize).unwrap(), fixture.native.block.hash(),
    ).is_err(), "a supplied hash cannot redirect canonical inclusion");
    let path = state.kura.v2_finality_artifact_path_for_testing(carrier.header().height().get());
    let bytes = std::fs::read(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    assert!(state.kura.read_finalized_native_lane_batch(
        NonZeroUsize::new(carrier.header().height().get() as usize).unwrap(), carrier.hash(),
    ).is_err(), "missing authentic finality cannot become body recovery");
    std::fs::write(path, bytes).unwrap();
}

state_test! { sync historical_native_batch_first_source_recovery_is_typed_and_rechecked
    use super::NativeLaneBatchReplayV1;
    use crate::sumeragi::{v2_chunks, v2_transport};
    use iroha_data_model::block::consensus_v2 as wire;
    let (fixture, _, included) = retained_native_batch_fixture();
    let state = &fixture.native.state;
    let first = &fixture.native.block;
    state.kura.evict_first_admission_body_for_testing(
        NonZeroUsize::new(first.header().height().get() as usize).unwrap(), first.hash(),
    ).unwrap();
    let NativeLaneBatchReplayV1::FirstInputRecoveryRequired { execution_index, source } =
        state.replay_finalized_native_lane_batch(&included, &[]).unwrap()
        else { panic!("included raw full input cannot replace first-carrier evidence"); };
    assert_eq!(execution_index, 0);
    assert_eq!(source.carrier_hash(), first.hash());
    let finality = source.finality();
    let key = &fixture.native.validators[0]; let peer = PeerId::new(key.public_key().clone());
    let mut request = wire::CertifiedBodyRequest { round: finality.commit_qc.proposal_round,
        subject: finality.subject, certificate: finality.commit_qc.clone(), requester: peer.clone(), signature: vec![] };
    request.signature = Signature::new(key.private_key(), &request.signature_preimage()).payload().to_vec();
    let request = v2_transport::authenticate_certified_body_request_with_validator_pops(
        &finality.height_context, &finality.validator_set_pops, request, &peer,
    ).unwrap();
    let body = first.canonical_resultless_proposal().encode_wire().unwrap();
    let (manifest, _) = v2_chunks::encode_payload(&finality.height_context, request.request().round,
        finality.subject, &body).unwrap().into_parts();
    let mut response = wire::CertifiedBodyResponse {request_hash: request.request_hash(), manifest,
        body, responder: peer.clone(), signature: vec![]};
    response.signature = Signature::new(key.private_key(), &response.signature_preimage()).payload().to_vec();
    let mut outstanding = v2_transport::OutstandingCertifiedBodyRequests::new(1).unwrap();
    outstanding.register(request.clone()).unwrap();
    let response = outstanding.authenticate_response(&finality.height_context, response, &peer).unwrap();
    let recovered = source.complete_from_authenticated_response(&request, &response).unwrap();
    assert!(state.replay_finalized_native_lane_batch(&included, &[(1, recovered.clone())]).is_err());
    assert!(state.replay_finalized_native_lane_batch(&included, &[(0, recovered.clone()), (0, recovered.clone())]).is_err());
    let NativeLaneBatchReplayV1::Ready(replayed) = state.replay_finalized_native_lane_batch(
        &included, &[(0, recovered)],
    ).unwrap() else { panic!("authentic completion rejoins every exact source and replay check"); };
    assert_eq!(replayed.batch(), included.batch());
    drop(replayed);
    assert_eq!(outstanding.len(), 1, "economic projection cannot acknowledge transport custody");
}

state_test! { sync historical_native_batch_rechecks_even_finalized_resigned_source_substitution
    let (fixture, _, included) = retained_native_batch_fixture_with_substitution(true);
    let state = &fixture.native.state;
    let before = crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot");
    // The storage fixture's canonical CommitQC really contains this substituted
    // group, whose native shares were also re-signed. Inclusion is not execution.
    assert!(state.replay_finalized_native_lane_batch(&included, &[]).is_err());
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot"), before);
    assert!(state.kura.read_finalized_native_lane_batch(
        NonZeroUsize::new(fixture.native.block.header().height().get() as usize).unwrap(),
        fixture.native.block.hash(),
    ).is_err(), "an admission-only carrier contains no native economic batch");
}

// Outline the actual snapshot decoder so the test does not reserve a second
// whole State alongside both block overlays on its ordinary test-thread stack.
fn restore_native_batch_pre_state_for_test(state: &State) -> Box<State> {
    let mut restored = deserialize_state_snapshot_value_with_kura(
        norito::json::to_value(state).unwrap(),
        Arc::clone(&state.kura),
    )
    .unwrap();
    // Process settings are not snapshot state. Reinstall the same test startup
    // settings before the original configured Nexus policy, just as the source
    // fixture did; the resulting prefix must retain its exact authenticated root.
    restored.configure_test_runtime_defaults();
    // The snapshot authenticates the applying prefix, not process configuration.
    // As at daemon startup, reinstall the same static Nexus policy before replay;
    // otherwise this zero-charge fixture silently picks up default nonzero fees.
    restored
        .set_nexus_from_config(state.nexus_snapshot())
        .expect("restore original startup policy on the authenticated pre-State");
    // Bare snapshots intentionally start with no configured manifest authority.
    // The daemon installs its frozen manifests/compliance before replay too.
    restored.install_lane_manifests(&state.lane_manifests.read().clone());
    restored.install_lane_compliance_engine(state.lane_compliance_engine());
    assert_eq!(
        restored.execution_policy_digest_v1().unwrap(),
        state.execution_policy_digest_v1().unwrap(),
        "historical replay requires the same authenticated execution policy, not only the WSV root",
    );
    assert_eq!(
        restored
            .lane_execution_state_hash()
            .expect("stable valid fixture snapshot"),
        state
            .lane_execution_state_hash()
            .expect("stable valid fixture snapshot"),
        "installing static policy must preserve the authenticated applying prefix",
    );
    restored
}

fn advance_native_batch_fixture_membership_for_test(
    state: &State,
    carrier: &SignedBlock,
    included: &crate::kura::FinalizedNativeLaneBatchV1,
) {
    let mut overlay = Box::new(state.block(carrier.header()));
    overlay
        .finalize_lane_consensus_contexts(carrier, Some(&included.finality().height_context))
        .unwrap();
    overlay.block_hashes.push(carrier.hash());
    insert_empty_transaction_block_for_state_commit(&mut overlay, carrier);
    overlay.commit().unwrap();
}

fn close_native_batch_fixture_membership_for_test(
    fixture: &NativeEconomicFixture,
    carrier: &SignedBlock,
    included: &crate::kura::FinalizedNativeLaneBatchV1,
) {
    let state = &fixture.native.state;
    // Advance only authenticated membership metadata. This is NOT economic
    // Apply: the production native gate stays closed and assets remain unchanged.
    advance_native_batch_fixture_membership_for_test(state, carrier, included);
    let close = empty_global_block_after(Some(carrier));
    let context = crate::sumeragi::v2_context::build_successor_height_context(
        included.finality(),
        included.finality().height_context.nexus_amx_context_hash,
        None,
    )
    .unwrap();
    let mut overlay = Box::new(state.block(close.header()));
    assert!(
        State::resolve_queue_plan_pending_obligation_in_storage(
            &mut overlay.world.smart_contract_state,
            fixture.native.binding.network_id_digest,
            fixture.native.binding.entrypoint_hash,
        )
        .unwrap()
    );
    overlay
        .finalize_lane_consensus_contexts(&close, Some(&context))
        .unwrap();
    let mut witness = ExecWitness::default();
    overlay
        .capture_lane_consensus_contexts(&mut witness)
        .unwrap();
    overlay.block_hashes.push(close.hash());
    insert_empty_transaction_block_for_state_commit(&mut overlay, &close);
    overlay.commit().unwrap();
    state.kura.store_block(Arc::new(close.clone())).unwrap();
    let (artifact, receipt) = stage_lane_context_fixture_finality(state, &close, context, witness);
    state
        .kura
        .promote_kagemusha_finality_sidecar(&artifact, &receipt)
        .unwrap();
}

state_test! { sync historical_native_batch_inclusion_survives_authenticated_closure_but_post_state_cannot_replay
    use super::NativeLaneBatchReplayV1;
    use crate::kura::NativeLaneBatchCarrierReadV1;
    let (fixture, carrier, included) = retained_native_batch_fixture();
    let state = &fixture.native.state;
    let pre_state = restore_native_batch_pre_state_for_test(state);
    assert_eq!(pre_state.lane_execution_state_hash().expect("stable valid fixture snapshot"), included.batch().base_state_hash);
    close_native_batch_fixture_membership_for_test(&fixture, &carrier, &included);
    assert!(state.verified_lane_consensus_contexts().unwrap().unwrap().contexts().is_empty());
    let NativeLaneBatchCarrierReadV1::Ready(retained) = state.kura.read_finalized_native_lane_batch(
        NonZeroUsize::new(carrier.header().height().get() as usize).unwrap(), carrier.hash(),
    ).unwrap() else { panic!("closure does not erase canonical inclusion"); };
    assert_eq!(retained.batch(), included.batch());
    assert!(state.replay_finalized_native_lane_batch(&retained, &[]).is_err(),
        "post-closure membership can never be reinterpreted as the applying prefix");
    let NativeLaneBatchReplayV1::Ready(replayed) = pre_state.replay_finalized_native_lane_batch(
        &retained, &[],
    ).unwrap() else { panic!("explicit preserved prefix reauthenticates its own historical inputs"); };
    assert_eq!(replayed.batch(), included.batch());
    drop(replayed);
    assert_eq!(pre_state.lane_execution_state_hash().expect("stable valid fixture snapshot"), included.batch().base_state_hash);
}

fn authenticated_native_batch_body_response_for_test(
    key: &KeyPair,
    finality: &V2FinalityArtifact,
    block: &SignedBlock,
) -> (
    crate::sumeragi::v2_transport::AuthenticatedCertifiedBodyRequest,
    crate::sumeragi::v2_transport::AuthenticatedCertifiedBodyResponse,
    crate::sumeragi::v2_transport::OutstandingCertifiedBodyRequests,
) {
    use crate::sumeragi::{v2_chunks, v2_transport};
    use iroha_data_model::block::consensus_v2 as wire;
    let peer = PeerId::new(key.public_key().clone());
    let mut request = wire::CertifiedBodyRequest {
        round: finality.commit_qc.proposal_round,
        subject: finality.subject,
        certificate: finality.commit_qc.clone(),
        requester: peer.clone(),
        signature: vec![],
    };
    request.signature = Signature::new(key.private_key(), &request.signature_preimage())
        .payload()
        .to_vec();
    let request = v2_transport::authenticate_certified_body_request_with_validator_pops(
        &finality.height_context,
        &finality.validator_set_pops,
        request,
        &peer,
    )
    .unwrap();
    let body = block.canonical_resultless_proposal().encode_wire().unwrap();
    assert!(
        body.len() <= finality.height_context.da_layout.max_payload_size_bytes as usize,
        "the complete actual carrier must fit its immutable signed geometry"
    );
    let (manifest, _) = v2_chunks::encode_payload(
        &finality.height_context,
        request.request().round,
        finality.subject,
        &body,
    )
    .unwrap()
    .into_parts();
    let mut response = wire::CertifiedBodyResponse {
        request_hash: request.request_hash(),
        manifest,
        body,
        responder: peer.clone(),
        signature: vec![],
    };
    response.signature = Signature::new(key.private_key(), &response.signature_preimage())
        .payload()
        .to_vec();
    let mut outstanding = v2_transport::OutstandingCertifiedBodyRequests::new(1).unwrap();
    outstanding.register(request.clone()).unwrap();
    let response = outstanding
        .authenticate_response(&finality.height_context, response, &peer)
        .unwrap();
    (request, response, outstanding)
}

state_test! { sync historical_native_batch_carrier_recovery_retains_exact_resultless_custody
    use crate::kura::NativeLaneBatchCarrierReadV1;
    use super::NativeLaneBatchReplayV1;
    let (fixture, carrier, included) = retained_native_batch_fixture();
    let state = &fixture.native.state;
    let height = NonZeroUsize::new(carrier.header().height().get() as usize).unwrap();
    state.kura.evict_first_admission_body_for_testing(height, carrier.hash()).unwrap();
    let NativeLaneBatchCarrierReadV1::CanonicalBodyRecoveryRequired(requirement) =
        state.kura.read_finalized_native_lane_batch(height, carrier.hash()).unwrap()
        else { panic!("authenticated eviction requires the exact existing global source owner"); };
    assert_eq!(requirement.finality(), included.finality());
    let (request, response, outstanding) = authenticated_native_batch_body_response_for_test(
        &fixture.native.validators[0], requirement.finality(), &carrier,
    );
    let first = &fixture.native.block;
    let first_finality = state.kura.v2_finality_artifact(first.header().height().get()).unwrap().unwrap();
    let (foreign_request, foreign_response, foreign_outstanding) = authenticated_native_batch_body_response_for_test(
        &fixture.native.validators[0], &first_finality, first,
    );
    assert!(requirement.complete_from_authenticated_response(&foreign_request, &foreign_response).is_err(),
        "a genuine body/QC for another canonical carrier is not this pending completion");
    assert_eq!(foreign_outstanding.len(), 1);
    let recovered = requirement.complete_from_authenticated_response(&request, &response).unwrap();
    assert_eq!(recovered.batch(), included.batch());
    assert_eq!(included.carrier_header(), &carrier.header());
    assert!(carrier.output_merkle_commitment().is_some());
    assert_eq!(recovered.carrier_header(), &carrier.canonical_resultless_proposal().header());
    assert_eq!(included.carrier_header(), recovered.carrier_header(), "execution output attachment preserves the immutable proposal header");
    assert_eq!(recovered.carrier_header().hash(), included.carrier_header().hash());
    assert_eq!(recovered.finality(), included.finality());
    let NativeLaneBatchReplayV1::Ready(replayed) =
        state.replay_finalized_native_lane_batch(&recovered, &[]).unwrap()
        else { panic!("recovered inclusion must re-enter source, context and economic replay"); };
    assert_eq!(replayed.batch(), included.batch());
    drop(replayed);
    assert!(matches!(state.kura.read_finalized_native_lane_batch(height, carrier.hash()).unwrap(),
        NativeLaneBatchCarrierReadV1::CanonicalBodyRecoveryRequired(_)),
        "resultless completion must never populate executed-wire storage");
    let mut tampered = response.response().clone();
    tampered.body[0] ^= 1;
    let peer = PeerId::new(fixture.native.validators[0].public_key().clone());
    assert!(outstanding.authenticate_response(&requirement.finality().height_context, tampered, &peer).is_err());
    assert_eq!(outstanding.len(), 1, "neither projection nor malformed completion acknowledges the transport owner");
}

state_test! { sync historical_native_batch_multiple_missing_inputs_retain_prior_completions
    use super::NativeLaneBatchReplayV1;
    let (fixture, _, included) = retained_native_batch_fixture_with_cases(
        &[NativeEconomicCase::Transfer(25), NativeEconomicCase::Transfer(30)], false,
    );
    let state = &fixture.native.state;
    let before = crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot");
    let first = &fixture.native.block;
    state.kura.evict_first_admission_body_for_testing(
        NonZeroUsize::new(first.header().height().get() as usize).unwrap(), first.hash(),
    ).unwrap();
    let NativeLaneBatchReplayV1::FirstInputRecoveryRequired { execution_index, source } =
        state.replay_finalized_native_lane_batch(&included, &[]).unwrap()
        else { panic!("the first missing source owns a recovery requirement"); };
    assert_eq!(execution_index, 0);
    let (request, response, outstanding) = authenticated_native_batch_body_response_for_test(
        &fixture.native.validators[0], source.finality(), first,
    );
    let first_input = source.complete_from_authenticated_response(&request, &response).unwrap();
    let retained = vec![(0, first_input)];
    let NativeLaneBatchReplayV1::FirstInputRecoveryRequired { execution_index, source } =
        state.replay_finalized_native_lane_batch(&included, &retained).unwrap()
        else { panic!("the second source must not restart the already completed first source"); };
    assert_eq!(execution_index, 1);
    // Both admissions share one canonical first carrier, but each input token
    // authenticates its own exact certificate/body join. Shared transport custody
    // does not collapse the two original execution positions.
    let second_input = source.complete_from_authenticated_response(&request, &response).unwrap();
    assert!(state.replay_finalized_native_lane_batch(&included, &[(0, second_input.clone())]).is_err());
    let mut retained = retained;
    retained.push((1, second_input));
    let NativeLaneBatchReplayV1::Ready(replayed) =
        state.replay_finalized_native_lane_batch(&included, &retained).unwrap()
        else { panic!("both retained completions rejoin exactly once"); };
    assert_eq!(replayed.batch(), included.batch());
    assert_eq!(replayed.overlay().world.assets.get(&fixture.source).unwrap().0, Quantity::from(45u32));
    assert_eq!(replayed.overlay().world.assets.get(&fixture.destination).unwrap().0, Quantity::from(55u32));
    drop(replayed);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot"), before);
    assert_eq!(outstanding.len(), 1, "economic replay cannot discharge the shared transport owner");
}

// The complete inputs, Decisions and outputs here come from the actual Core
// fixture and executor. Wire proofs still do not authorize global acceptance.
fn assert_actual_native_prefix_wire_for_test(
    fixture: &NativeEconomicFixture,
    expected_success: &[bool],
) {
    let state = &fixture.native.state;
    let before = crate::snapshot::canonical_state_snapshot_hash(state)
        .expect("stable valid fixture snapshot");
    let groups = native_economic_groups(fixture);
    let mut carrier =
        empty_global_block_after(Some(&fixture.native.block)).canonical_resultless_proposal();
    let batch = state.prepare_lane_decision_batch(&groups).unwrap();
    carrier.set_execution_context(Some(
        BlockExecutionContextBundle::default().with_native_lane_decisions(batch),
    ));
    let prepared = state
        .prepare_native_batch_on_carrier(carrier.header(), groups.clone())
        .unwrap();
    assert_eq!(
        prepared
            .executions()
            .iter()
            .map(|execution| execution.result.is_ok())
            .collect::<Vec<_>>(),
        expected_success
    );
    let proposal_hash = carrier.hash();
    attach_actual_native_prefix_results_for_test(&mut carrier, &prepared);
    assert_eq!(carrier.external_entrypoint_count(), 0);
    assert_eq!(carrier.network_entrypoint_count(), groups.len());
    assert_eq!(
        carrier.committed_fragment_count(),
        Some(u64::try_from(prepared.overlay().committed_fragment_count()).unwrap())
    );
    assert_eq!(
        carrier.output_results().cloned().collect::<Vec<_>>(),
        prepared
            .executions()
            .iter()
            .map(|execution| execution.result.clone())
            .collect::<Vec<_>>()
    );
    let wire = carrier.encode_wire().unwrap();
    let decoded = iroha_data_model::block::decode_framed_signed_block(&wire).unwrap();
    assert_eq!(decoded, carrier);
    decoded.validate_native_lane_results().unwrap();
    assert_eq!(
        decoded.canonical_resultless_proposal().hash(),
        proposal_hash
    );
    for (index, actual) in prepared.executions().iter().enumerate() {
        let input = &actual.source.payload.input.entrypoint;
        assert_eq!(decoded.network_entrypoint_at(index), Some(input));
        let input_index = u32::try_from(index).unwrap();
        let input_proof = decoded.network_input_proof(input_index).unwrap();
        let input_commitment = decoded.network_input_merkle_commitment().unwrap();
        assert!(input_proof.verify(&input.hash(), &input_commitment));
        let foreign_input = HashOf::from_untyped_unchecked(Hash::new(b"foreign native input"));
        assert!(!input_proof.verify(&foreign_input, &input_commitment));
        let (output_index, row) = decoded.network_output_at(input_index).unwrap();
        assert_eq!(row.input_index, input_index);
        assert_eq!(row.result, actual.result);
        let output_proof = decoded.output_proof(output_index).unwrap();
        let output_commitment = decoded.output_merkle_commitment().unwrap();
        let output_hash = HashOf::new(&decoded.execution_outputs()[output_index as usize]);
        assert!(output_proof.verify(&output_hash, &output_commitment));
        let foreign_output = HashOf::from_untyped_unchecked(Hash::new(b"foreign native output"));
        assert!(!output_proof.verify(&foreign_output, &output_commitment));
        if let TransactionEntrypoint::SealedReveal(_) = input {
            assert_ne!(
                Hash::from(input.hash()),
                Hash::from(input.execution_call_hash())
            );
            assert!(
                decoded
                    .network_input_hashes()
                    .all(|hash| Hash::from(hash) != Hash::from(input.execution_call_hash())),
                "the signed replay alias is not a second canonical input"
            );
            assert!(
                !decoded
                    .fastpq_transcripts()
                    .contains_key(&Hash::from(input.hash()))
            );
        }
    }
    drop(prepared);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(state)
            .expect("stable valid fixture snapshot"),
        before
    );
}

state_test! { sync native_actual_prefix_results_roundtrip_transfer_rejection_and_sealed_owner
    for (case, success) in [(NativeEconomicCase::Transfer(25), true),
        (NativeEconomicCase::BadSignature, false), (NativeEconomicCase::Reveal(0), true)] {
        let fixture = native_economic_fixture(&[case], false);
        assert_actual_native_prefix_wire_for_test(&fixture, &[success]);
    }
}

state_test! { sync native_actual_prefix_results_preserve_nonzero_fee_and_terminal_failure
    let fixture = native_economic_fixture_with_fee_policy(
        &[NativeEconomicCase::Transfer(25), NativeEconomicCase::Transfer(30)],
        false, None, Some(NativeEconomicDirectFee { funding: 7, signed_max: 5 }),
    );
    assert_actual_native_prefix_wire_for_test(&fixture, &[true, false]);
}
