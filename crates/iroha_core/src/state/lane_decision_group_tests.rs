// Consumer boundary tests use authentic first-carrier source, frozen committees
// and native signatures; they do not fabricate reducer Ready or economic Apply.

fn prepared_native_decision_group_for_test(
    fixture: &LaneContextVerifiedFixture,
) -> super::VerifiedLaneDecisionGroupV1 {
    let state = &fixture.state;
    let observed = state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &observed.contexts()[0];
    let FirstLaneAdmittedInputReadV1::Ready(source) =
        state.first_lane_admitted_input(&observed, lane).unwrap()
    else {
        panic!("source");
    };
    let LaneInputBodyPreparationV1::Ready(body) = state
        .prepare_lane_input_body(&observed, lane, &source)
        .unwrap()
    else {
        panic!("body");
    };
    let decisions = observed
        .contexts()
        .iter()
        .enumerate()
        .map(|(index, lane)| {
            sign_native_group_decision_for_test(
                lane,
                &fixture.validators,
                &body,
                index as u64,
                index as u64 + 2,
            )
        })
        .collect::<Vec<_>>();
    let super::LaneDecisionGroupPreparationV1::Ready(group) = state
        .prepare_lane_decision_group(&observed, lane, &source, &decisions)
        .unwrap()
    else {
        panic!("group");
    };
    group
}

fn resign_changed_native_group_payload_for_test(
    fixture: &LaneContextVerifiedFixture<impl std::borrow::Borrow<State>>,
    source: &mut iroha_data_model::block::lane_input::LaneDecisionGroupV1,
) {
    use iroha_data_model::block::{consensus_v2 as wire, lane_consensus::lane_availability_hash};
    let state: &State = std::borrow::Borrow::borrow(&fixture.state);
    let observed = state.verified_lane_consensus_contexts().unwrap().unwrap();
    let bytes = norito::encode_canonical(&source.payload).unwrap();
    for (decision, lane) in source.decisions.iter_mut().zip(observed.contexts()) {
        let manifest = &mut decision.manifest;
        let chunks = wire::encode_payload_chunks(manifest.layout, &bytes).unwrap();
        manifest.chunk_root =
            wire::payload_chunk_root(&chunks.iter().map(Hash::new).collect::<Vec<_>>()).unwrap();
        manifest.byte_len = bytes.len() as u64;
        manifest.chunk_count = chunks.len() as u32;
        manifest.value.descriptor_hash = source.payload.descriptor.canonical_hash().unwrap();
        manifest.value.payload_hash = Hash::new(&bytes);
        manifest.value.availability_hash = lane_availability_hash(
            manifest.layout,
            manifest.chunk_root,
            manifest.byte_len,
            manifest.chunk_count,
        )
        .unwrap();
        resign_native_group_decision_for_test(lane, &fixture.validators, decision);
        crate::sumeragi::v2_lane_wire::LaneAuthenticator::new(lane)
            .decision_certificate(decision)
            .unwrap();
    }
    source.validate_structure().unwrap();
}

state_test! { sync native_lane_decision_source_roundtrips_single_input_and_rechecks_current_authority
    use super::LaneDecisionGroupPreparationV1;
    use iroha_data_model::block::lane_input::LaneDecisionGroupV1;
    let fixture = all_route_input_fixture(false);
    let state = &fixture.state;
    let group = prepared_native_decision_group_for_test(&fixture);
    let wire = group.to_wire();
    let bytes = norito::encode_canonical(&wire).unwrap();
    let decoded = LaneDecisionGroupV1::decode_canonical(&bytes, bytes.len()).unwrap();
    let observed = state.verified_lane_consensus_contexts().unwrap().unwrap();
    let LaneDecisionGroupPreparationV1::Ready(imported) = state.import_lane_decision_group(&observed,&decoded).unwrap() else {panic!("authentic complete native source");};
    assert_eq!(imported.to_wire(),wire);
    assert_eq!(imported.contexts().len(),2);
    assert_eq!(imported.body().canonical_bytes(),group.body().canonical_bytes());
    assert_ne!(wire.decisions[0].commit_qc.statement.round.voting_view,wire.decisions[1].commit_qc.statement.round.voting_view);
    let overlay = state.merge_preexecution_block(empty_global_block_after(Some(&fixture.block)).header());
    overlay.preflight_lane_decision_execution_inputs(std::slice::from_ref(&imported)).unwrap();
    drop(overlay);
    assert_eq!(state.block_hashes.view().len() as u64,fixture.block.header().height().get(),"preflight cannot publish application");
    state.append_committed_block_header_for_tests(empty_global_block_after(Some(&fixture.block)).header());
    assert!(matches!(state.import_lane_decision_group(&observed,&decoded).unwrap(),LaneDecisionGroupPreparationV1::ObservationChanged));
}

state_test! { sync native_lane_decision_source_rejects_resigned_first_carrier_and_certificate_substitution
    let fixture = all_route_input_fixture(false);
    let state = &fixture.state;
    let group = prepared_native_decision_group_for_test(&fixture);
    let observed = state.verified_lane_consensus_contexts().unwrap().unwrap();
    for mutation in 0..3 {
        let mut changed = group.to_wire();
        match mutation {
            0 => {changed.payload.descriptor.admission_carrier_hash=HashOf::from_untyped_unchecked(Hash::new(b"other finalized first carrier"));},
            1 => {changed.payload.descriptor.admission_priority.admission_index+=1;},
            2 => {
                let input=&changed.payload.input;
                let alternate=queue_plan_admission_certificate_bytes_for_signer_indices_state_test(&input.entrypoint,&input.certificate.binding,&fixture.validators,&[1,2]);
                let valid=crate::torii_proxy::decode_and_validate_lane_admitted_input_v1(&state.network_id,&alternate).unwrap();
                changed.payload.input=valid.input().clone();
                changed.payload.descriptor.admitted_input_hash=Hash::new(&alternate);
                assert_ne!(changed.payload.input,group.to_wire().payload.input,"a valid alternate durability subset is not the original carrier's bytes");
            },
            _=>unreachable!(),
        }
        resign_changed_native_group_payload_for_test(&fixture,&mut changed);
        assert!(state.import_lane_decision_group(&observed,&changed).is_err(),"valid native Commit signatures cannot substitute first-carrier source {mutation}");
    }
    let mut missing=group.to_wire();missing.decisions.pop();
    assert!(state.import_lane_decision_group(&observed,&missing).is_err());
    let mut reordered=group.to_wire();reordered.decisions.swap(0,1);
    assert!(state.import_lane_decision_group(&observed,&reordered).is_err());
    let (foreign,_)=first_lane_input_fixture(0xa2);
    let foreign_group=prepared_native_decision_group_for_test(&foreign);
    assert!(matches!(state.import_lane_decision_group(&observed,&foreign_group.to_wire()).unwrap(),super::LaneDecisionGroupPreparationV1::InstanceNotCurrent));
}

state_test! { sync native_lane_decision_source_missing_first_body_retains_exact_global_recovery_requirement
    use super::LaneDecisionGroupPreparationV1;
    use crate::sumeragi::{v2_chunks,v2_transport};
    use iroha_data_model::block::consensus_v2 as wire;
    let fixture = all_route_input_fixture(false);
    let state = &fixture.state;
    let group = prepared_native_decision_group_for_test(&fixture);
    let source = group.to_wire();
    let observed = state.verified_lane_consensus_contexts().unwrap().unwrap();
    let height = std::num::NonZeroUsize::new(fixture.block.header().height().get() as usize).unwrap();
    state.kura.evict_first_admission_body_for_testing(height, fixture.block.hash()).unwrap();
    let LaneDecisionGroupPreparationV1::CanonicalBodyRecoveryRequired(required) = state.import_lane_decision_group(&observed,&source).unwrap() else {panic!("a supplied complete source cannot stand in for first-carrier proof");};
    assert_eq!(required.carrier_hash(),fixture.block.hash());
    assert_eq!(required.priority(),source.payload.descriptor.admission_priority);
    assert_eq!(required.finality().subject,group.body().source().source().finality().subject);
    assert_eq!(source,group.to_wire(),"waiting leaves every exact incoming decision with its caller");
    let finality=required.finality();
    let key=&fixture.validators[0];let peer=PeerId::new(key.public_key().clone());
    let mut request=wire::CertifiedBodyRequest {round:finality.commit_qc.proposal_round,subject:finality.subject,certificate:finality.commit_qc.clone(),requester:peer.clone(),signature:vec![]};
    request.signature=Signature::new(key.private_key(),&request.signature_preimage()).payload().to_vec();
    let request=v2_transport::authenticate_certified_body_request_with_validator_pops(&finality.height_context,&finality.validator_set_pops,request,&peer).unwrap();
    let body=fixture.block.canonical_resultless_proposal().encode_wire().unwrap();
    let (manifest,_)=v2_chunks::encode_payload(&finality.height_context,request.request().round,finality.subject,&body).unwrap().into_parts();
    let mut response=wire::CertifiedBodyResponse {request_hash:request.request_hash(),manifest,body,responder:peer.clone(),signature:vec![]};
    response.signature=Signature::new(key.private_key(),&response.signature_preimage()).payload().to_vec();
    let mut outstanding=v2_transport::OutstandingCertifiedBodyRequests::new(1).unwrap();
    outstanding.register(request.clone()).unwrap();
    let response=outstanding.authenticate_response(&finality.height_context,response,&peer).unwrap();
    let recovered=required.complete_from_authenticated_response(&request,&response).unwrap();
    let LaneDecisionGroupPreparationV1::Ready(imported)=state.import_recovered_lane_decision_group(&observed,&source,&recovered).unwrap() else {panic!("exact recovered source re-enters full import checks");};
    assert_eq!(imported.to_wire(),source);
    let mut substituted=source.clone();
    substituted.payload.descriptor.admission_carrier_hash=HashOf::from_untyped_unchecked(Hash::new(b"substituted after recovery"));
    resign_changed_native_group_payload_for_test(&fixture,&mut substituted);
    assert!(state.import_recovered_lane_decision_group(&observed,&substituted,&recovered).is_err());
    assert_eq!(outstanding.len(),1,"group validation does not acknowledge transport custody");
    let overlay=state.merge_preexecution_block(empty_global_block_after(Some(&fixture.block)).header());
    overlay.preflight_lane_decision_execution_inputs(std::slice::from_ref(&group)).unwrap();
    // A previously verified source remains immutable evidence; a fresh import
    // requires its own exact recovery. Neither route grants an Apply completion.
}

state_test! { sync native_lane_decision_execution_preflight_rejects_changed_frontier_closed_head_and_duplicate_batch
    let fixture = all_route_input_fixture(false);
    let state = &fixture.state;
    let group = prepared_native_decision_group_for_test(&fixture);
    let next=empty_global_block_after(Some(&fixture.block));
    {
        let overlay=state.merge_preexecution_block(next.header());
        overlay.preflight_lane_decision_execution_inputs(std::slice::from_ref(&group)).unwrap();
        assert!(overlay.preflight_lane_decision_execution_inputs(&[group.clone(),group.clone()]).is_err());
    }
    for mutation in 0..4 {
        let mut overlay=state.merge_preexecution_block(next.header());
        let slot=&group.body().payload().descriptor.slots[1];
        match mutation {
            0 => {overlay.lane_consensus_contexts.get_mut().contexts.pop();},
            1 => {
                let marker=super::AppliedMergeLaneFrontierMarker {version:1,lane_id:slot.route.lane_id,dataspace_id:slot.route.dataspace_id,lane_incarnation:slot.lane_incarnation,lane_block_height:slot.lane_height,lane_block_descriptor_hash:Hash::new(b"competing old writer"),applied_global_height:next.header().height().get()};
                let (key,value)=State::encode_merge_lane_frontier_marker(marker).unwrap();
                overlay.world.smart_contract_state.insert(key,value);
            },
            2 => {
                let input=&group.body().payload().input;
                overlay.resolve_required_queue_plan_pending_obligations(vec![(input.entrypoint.hash(),input.certificate.binding.canonical_hash())],BTreeSet::new()).unwrap();
            },
            3 => {overlay.block_hashes.push(next.hash());},
            _=>unreachable!(),
        }
        assert!(overlay.preflight_lane_decision_execution_inputs(std::slice::from_ref(&group)).is_err(),"stale private observation cannot authorize changed execution overlay {mutation}");
    }
    let overlay=state.merge_preexecution_block(next.header());
    overlay.preflight_lane_decision_execution_inputs(std::slice::from_ref(&group)).unwrap();
    assert_eq!(group.to_wire().decisions.len(),2,"rejection and dropped overlays preserve native source custody");
}

fn sign_native_group_decision_for_test(
    lane: &VerifiedLaneContext,
    validators: &[KeyPair],
    body: &VerifiedLaneInputBodyV1,
    origin_view: u64,
    voting_view: u64,
) -> iroha_data_model::block::lane_consensus::LaneDecisionV1 {
    use iroha_data_model::block::lane_consensus::{
        LaneDecisionV1, LanePhaseV1, LaneQcV1, LaneRoundV1, LaneVoteStatementV1,
    };
    let manifest = *crate::sumeragi::v2_lane_payload::encode_lane_input(lane, body, origin_view)
        .unwrap()
        .manifest();
    let mut decision = LaneDecisionV1 {
        manifest,
        commit_qc: LaneQcV1 {
            statement: LaneVoteStatementV1 {
                round: LaneRoundV1 {
                    instance_id: manifest.value.instance_id,
                    lane_height: lane.frozen().next_lane_height,
                    voting_view,
                },
                phase: LanePhaseV1::Commit,
                value: manifest.value,
            },
            shares: Vec::new(),
        },
    };
    resign_native_group_decision_for_test(lane, validators, &mut decision);
    decision
}

fn resign_native_group_decision_for_test(
    lane: &VerifiedLaneContext,
    validators: &[KeyPair],
    decision: &mut iroha_data_model::block::lane_consensus::LaneDecisionV1,
) {
    use iroha_data_model::block::lane_consensus::LaneSignatureShareV1;
    let statement = &mut decision.commit_qc.statement;
    statement.value = decision.manifest.value;
    let preimage = statement.signature_preimage().unwrap();
    let quorum = 2 * ((lane.frozen().committee.len() - 1) / 3) + 1;
    decision.commit_qc.shares = (0..quorum)
        .map(|index| {
            let key = validators
                .iter()
                .find(|key| key.public_key() == lane.frozen().committee[index].public_key())
                .unwrap();
            LaneSignatureShareV1 {
                signer: index as u32,
                signature: Signature::try_new(key.private_key(), &preimage)
                    .unwrap()
                    .payload()
                    .to_vec(),
            }
        })
        .collect();
}

state_test! { sync native_lane_decision_group_joins_exact_routes_without_a_shared_view_or_duplicate_roles
    use super::LaneDecisionGroupPreparationV1;
    let fixture = all_route_input_fixture(false);
    let state = &fixture.state;
    let observed = state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &observed.contexts()[0];
    let FirstLaneAdmittedInputReadV1::Ready(source) = state.first_lane_admitted_input(&observed, lane).unwrap() else { panic!("source"); };
    let LaneInputBodyPreparationV1::Ready(body) = state.prepare_lane_input_body(&observed, lane, &source).unwrap() else { panic!("body"); };
    let first = sign_native_group_decision_for_test(lane, &fixture.validators, &body, 0, 1);
    let second = sign_native_group_decision_for_test(&observed.contexts()[1], &fixture.validators, &body, 2, 5);
    let received = vec![second.clone(), first.clone()];
    let LaneDecisionGroupPreparationV1::Ready(group) = state.prepare_lane_decision_group(&observed, lane, &source, &received).unwrap() else { panic!("exact group"); };
    assert_eq!(group.body().canonical_bytes(), body.canonical_bytes());
    assert_eq!(group.body().source().carrier_hash(), fixture.block.hash());
    assert_eq!(group.decisions(), &[first.clone(), second.clone()], "arrival order cannot alter canonical route order");
    assert_eq!(group.decisions().len(), 2, "coordinator+participant on one route needs one Decision");
    assert_eq!(fixture.binding.admission_context.route_incarnations.len(), 3);
    assert_eq!(received, vec![second, first], "preparation consumes no instance custody");
    let parent=state.kura.v2_finality_artifact(fixture.block.header().height().get()).unwrap().unwrap();
    let opening=crate::sumeragi::v2_context::build_successor_height_context(&parent,parent.height_context.nexus_amx_context_hash,None).unwrap();
    let later=empty_global_block_after(Some(&fixture.block));
    let mut overlay=state.block(later.header());
    overlay.finalize_lane_consensus_contexts(&later,Some(&opening)).unwrap();
    let mut witness=ExecWitness::default();
    overlay.capture_lane_consensus_contexts(&mut witness).unwrap();
    overlay.block_hashes.push(later.hash());
    insert_empty_transaction_block_for_state_commit(&mut overlay,&later);
    overlay.commit().unwrap();
    state.kura.store_block(Arc::new(later.clone())).unwrap();
    let (artifact,receipt)=stage_lane_context_fixture_finality(state,&later,opening,witness);
    state.kura.promote_kagemusha_finality_sidecar(&artifact,&receipt).unwrap();
    assert!(matches!(state.prepare_lane_decision_group(&observed,lane,&source,&received).unwrap(),LaneDecisionGroupPreparationV1::ObservationChanged));
    let current=state.verified_lane_consensus_contexts().unwrap().unwrap();
    let LaneDecisionGroupPreparationV1::Ready(rechecked)=state.prepare_lane_decision_group(&current,&current.contexts()[0],&source,&received).unwrap() else {panic!("same group survives unrelated global advancement");};
    assert_eq!(rechecked.body().canonical_bytes(),group.body().canonical_bytes());
    assert_eq!(rechecked.decisions(),group.decisions());
}

state_test! { sync native_lane_decision_group_names_missing_instances_and_rejects_duplicate_foreign_quorums
    use super::LaneDecisionGroupPreparationV1;
    use iroha_data_model::block::lane_consensus::{LanePhaseV1, LaneSignatureShareV1};
    let fixture = all_route_input_fixture(false);
    let state = &fixture.state;
    let observed = state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &observed.contexts()[0];
    let FirstLaneAdmittedInputReadV1::Ready(source) = state.first_lane_admitted_input(&observed, lane).unwrap() else { panic!("source"); };
    let LaneInputBodyPreparationV1::Ready(body) = state.prepare_lane_input_body(&observed, lane, &source).unwrap() else { panic!("body"); };
    let first = sign_native_group_decision_for_test(lane, &fixture.validators, &body, 0, 0);
    let second = sign_native_group_decision_for_test(&observed.contexts()[1], &fixture.validators, &body, 0, 0);
    let expected = body.payload().descriptor.slots.iter().map(|slot|slot.instance_id).collect::<Vec<_>>();
    let LaneDecisionGroupPreparationV1::MissingDecisions(missing) = state.prepare_lane_decision_group(&observed,lane,&source,&[]).unwrap() else { panic!("missing group"); };
    assert_eq!(missing,expected);
    let LaneDecisionGroupPreparationV1::MissingDecisions(missing) = state.prepare_lane_decision_group(&observed,lane,&source,std::slice::from_ref(&second)).unwrap() else { panic!("missing exact first instance"); };
    assert_eq!(missing,vec![expected[0]]);
    assert!(state.prepare_lane_decision_group(&observed,lane,&source,&[first.clone(),first.clone()]).is_err());
    assert!(state.prepare_lane_decision_group(&observed,lane,&source,&vec![first.clone();iroha_data_model::block::lane_input::MAX_LANE_INPUT_ROUTE_SLOTS+1]).is_err());
    for mutation in 0..6 {
        let mut changed=first.clone();
        match mutation {
            0 => { changed.manifest.value.instance_id=Hash::new(b"another live or closed slot"); },
            1 => { changed.commit_qc.shares.pop(); },
            2 => {
                let index=3;let key=fixture.validators.iter().find(|key|key.public_key()==lane.frozen().committee[index].public_key()).unwrap();
                changed.commit_qc.shares.push(LaneSignatureShareV1 {signer:index as u32,signature:Signature::try_new(key.private_key(),&changed.commit_qc.statement.signature_preimage().unwrap()).unwrap().payload().to_vec()});
            },
            3 => { changed.commit_qc.shares[0].signature[0]^=1; },
            4 => { changed.commit_qc.statement.phase=LanePhaseV1::Prepare;resign_native_group_decision_for_test(lane,&fixture.validators,&mut changed); },
            5 => { changed.manifest.value.admitted_binding_hash=Hash::new(b"another immutable admission");resign_native_group_decision_for_test(lane,&fixture.validators,&mut changed); },
            _ => unreachable!(),
        }
        assert!(state.prepare_lane_decision_group(&observed,lane,&source,&[changed,second.clone()]).is_err(),"wrong instance, non-exact quorum, invalid signature, Prepare or foreign binding cannot decide: {mutation}");
    }
}

state_test! { sync native_lane_decision_group_rejects_authentic_qc_over_a_substituted_input_or_codeword
    use iroha_data_model::block::lane_consensus::lane_availability_hash;
    let fixture = all_route_input_fixture(false);
    let state = &fixture.state;
    let observed = state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &observed.contexts()[0];
    let FirstLaneAdmittedInputReadV1::Ready(source) = state.first_lane_admitted_input(&observed, lane).unwrap() else { panic!("source"); };
    let LaneInputBodyPreparationV1::Ready(body) = state.prepare_lane_input_body(&observed, lane, &source).unwrap() else { panic!("body"); };
    let first = sign_native_group_decision_for_test(lane,&fixture.validators,&body,0,0);
    let second = sign_native_group_decision_for_test(&observed.contexts()[1],&fixture.validators,&body,0,0);
    for mutation in 0..4 {
        let mut changed=first.clone();
        match mutation {
            0 => { changed.manifest.value.descriptor_hash=Hash::new(b"another group route descriptor"); },
            1 => { changed.manifest.value.payload_hash=Hash::new(b"different canonical input"); },
            2 => {
                changed.manifest.chunk_root=Hash::new(b"different RS16 codeword");
                changed.manifest.value.availability_hash=lane_availability_hash(changed.manifest.layout,changed.manifest.chunk_root,changed.manifest.byte_len,changed.manifest.chunk_count).unwrap();
            },
            3 => { changed.manifest.value.kind=iroha_data_model::block::lane_consensus::LaneValueKindV1::Execution; },
            _ => unreachable!(),
        }
        resign_native_group_decision_for_test(lane,&fixture.validators,&mut changed);
        crate::sumeragi::v2_lane_wire::LaneAuthenticator::new(lane).decision_certificate(&changed).expect("cryptographically valid exact quorum is insufficient without input/codeword equality");
        assert!(state.prepare_lane_decision_group(&observed,lane,&source,&[changed,second.clone()]).is_err(),"must reject exact signed substitution {mutation}");
    }
}

state_test! { sync native_lane_decision_group_preserves_earlier_head_wait_and_fences_changed_observation
    use super::LaneDecisionGroupPreparationV1;
    let fixture = all_route_input_fixture(true);
    let state = &fixture.state;
    let observed=state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane=observed.contexts().iter().find(|lane|lane.frozen().lane_id==LaneId::SINGLE).unwrap();
    let FirstLaneAdmittedInputReadV1::Ready(source)=state.first_lane_admitted_input(&observed,lane).unwrap() else {panic!("source");};
    let LaneDecisionGroupPreparationV1::BlockedByEarlierInputs(deps)=state.prepare_lane_decision_group(&observed,lane,&source,&[]).unwrap() else {panic!("earlier owner remains serviceable");};
    assert_eq!(deps.len(),1);assert!(deps[0].priority<source.priority());
    let (foreign,_)=first_lane_input_fixture(0x94);
    let foreign_observed=foreign.state.verified_lane_consensus_contexts().unwrap().unwrap();
    let FirstLaneAdmittedInputReadV1::Ready(foreign_source)=foreign.state.first_lane_admitted_input(&foreign_observed,&foreign_observed.contexts()[0]).unwrap() else {panic!("authentic foreign source");};
    assert!(state.prepare_lane_decision_group(&observed,lane,&foreign_source,&[]).is_err(),"an authentic other first-carrier source cannot replace this group");
    assert!(matches!(state.prepare_lane_decision_group(&observed,&foreign_observed.contexts()[0],&source,&[]).unwrap(),LaneDecisionGroupPreparationV1::InstanceNotCurrent));
    state.append_committed_block_header_for_tests(empty_global_block_after(Some(&fixture.block)).header());
    assert!(matches!(state.prepare_lane_decision_group(&observed,lane,&source,&[]).unwrap(),LaneDecisionGroupPreparationV1::ObservationChanged));
}
