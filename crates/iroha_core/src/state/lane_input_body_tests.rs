// Exact source/all-route materialization; these are State boundary fixtures,
// not claims that the production shared lane driver is already connected.

fn all_route_input_fixture(block_secondary: bool) -> LaneContextVerifiedFixture {
    let genesis = empty_global_block_after(None);
    let mut nexus = iroha_config::parameters::actual::Nexus::default();
    nexus.lane_catalog = LaneCatalog::new(
        nonzero!(2_u32),
        vec![
            LaneConfig::default(),
            LaneConfig {
                id: LaneId::new(1),
                alias: "input-secondary".into(),
                ..LaneConfig::default()
            },
        ],
    )
    .unwrap();
    let (state, kura) = State::new_with_chain_and_network_id_and_pre_genesis_nexus_for_testing(
        World::default(),
        nexus,
        LiveQueryStore::start_test(),
        (*DEFAULT_TEST_CHAIN_ID).clone(),
        iroha_data_model::NetworkId::from_genesis_hash(genesis.hash()),
    );
    let (ids, validators) = bls_accounts_in("validators", 4);
    seed_consensus_keys_with_pops(&state, &validators);
    install_lane_manifest_registry(
        &state,
        &[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL, ids.clone()),
            (LaneId::new(1), DataSpaceId::UNIVERSAL, ids),
        ],
    );
    let _ = configure_commit_topology_preserving_world_peers(&state, 1);
    kura.store_block(Arc::new(genesis.clone())).unwrap();
    commit_block_metadata_with_genesis_checkpoint_to_state(&state, &genesis);
    let parent = advance_queue_plan_fixture_to_beacon_parent(&state, genesis);
    let primary = crate::queue::RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    let secondary = crate::queue::RoutingDecision::new(LaneId::new(1), DataSpaceId::UNIVERSAL);
    let plan = crate::queue::RoutingPlan::native_amx(
        primary,
        vec![
            crate::queue::RouteLeg::new(primary, crate::queue::RouteLegRole::Participant),
            crate::queue::RouteLeg::new(secondary, crate::queue::RouteLegRole::Participant),
        ],
    );
    let (binding, complete) = queue_plan_admission_certificate_for_state_test(
        &state,
        plan,
        &validators,
        parent.header().height().get(),
        0x81,
    );
    let publish = |parent: &SignedBlock, controls: Vec<Vec<u8>>| {
        let mut block = empty_global_block_after(Some(parent));
        let mut execution = block.execution_context().cloned().unwrap_or_default();
        execution.queue_plan_admissions = controls.clone();
        block.set_execution_context(Some(execution));
        // Changing proposal controls invalidates its previous result attachment.
        // This carrier admits inputs only; their Network execution happens later.
        // Bind an empty structural result to the final control bytes before the
        // existing four-validator finality fixture authenticates this carrier.
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
        let opening = match state
            .kura
            .v2_finality_artifact(parent.header().height().get())
            .unwrap()
        {
            Some(exact_parent) => crate::sumeragi::v2_context::build_successor_height_context(
                &exact_parent,
                exact_parent.height_context.nexus_amx_context_hash,
                None,
            )
            .unwrap(),
            None => lane_opening_context_for_state_test(&state),
        };
        let mut overlay = state
            .block_with_queue_plan_admissions(block.header(), &controls)
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
            stage_lane_context_fixture_finality(&state, &block, opening.clone(), witness.clone());
        state
            .kura
            .promote_kagemusha_finality_sidecar(&artifact, &receipt)
            .unwrap();
        (block, opening, witness)
    };
    let parent = if block_secondary {
        let (_, earlier) = queue_plan_admission_certificate_for_state_test(
            &state,
            crate::queue::RoutingPlan::single(secondary),
            &validators,
            parent.header().height().get(),
            0x80,
        );
        publish(&parent, vec![earlier]).0
    } else {
        parent
    };
    let (block, opening, witness) = publish(&parent, vec![complete]);
    LaneContextVerifiedFixture {
        state,
        validators,
        binding,
        block,
        opening,
        witness,
    }
}

state_test! { sync lane_input_body_uses_one_exact_input_and_one_slot_per_distinct_route
    use super::{lane_admitted_input::FirstLaneAdmittedInputReadV1, lane_input_body::LaneInputBodyPreparationV1};
    let fixture = all_route_input_fixture(false);
    let state = &fixture.state;
    let observed = state.verified_lane_consensus_contexts().unwrap().unwrap();
    assert_eq!(observed.contexts().len(), 2);
    assert_eq!(fixture.binding.admission_context.route_incarnations.len(), 3,
        "the coordinator is also a participant but occupies one slot");
    let mut canonical = None;
    for lane in observed.contexts() {
        let FirstLaneAdmittedInputReadV1::Ready(source) = state.first_lane_admitted_input(&observed, lane).unwrap() else { panic!("actual original source"); };
        let LaneInputBodyPreparationV1::Ready(body) = state.prepare_lane_input_body(&observed, lane, &source).unwrap() else { panic!("every route has this exact oldest group"); };
        let payload = body.payload();
        assert_eq!(payload.descriptor.slots.len(), 2);
        assert_eq!(payload.descriptor.admission_carrier_hash, fixture.block.hash());
        assert_eq!(payload.descriptor.admission_priority, source.priority());
        assert_eq!(payload.descriptor.admitted_input_hash, source.canonical_control_hash());
        assert_eq!(body.source().canonical_control_bytes(), source.canonical_control_bytes());
        assert_eq!(body.kind(), iroha_data_model::block::lane_consensus::LaneValueKindV1::AtomicGroup);
        assert_eq!(norito::encode_canonical(&payload.input).unwrap(), source.canonical_control_bytes());
        assert_eq!(norito::encode_canonical(payload).unwrap(), body.canonical_bytes());
        for (slot, current) in payload.descriptor.slots.iter().zip(observed.contexts()) {
            assert_eq!(slot.instance_id, Hash::from(current.instance_id().0));
            assert_eq!(slot.lane_height, current.frozen().next_lane_height);
            assert_eq!(slot.lane_incarnation, current.frozen().lane_incarnation);
        }
        if let Some(bytes) = &canonical { assert_eq!(body.canonical_bytes(), bytes); }
        else { canonical = Some(body.canonical_bytes().to_vec()); }
    }
}

state_test! { sync lane_input_body_waits_only_for_an_exact_earlier_route_head
    use super::{lane_admitted_input::FirstLaneAdmittedInputReadV1, lane_input_body::LaneInputBodyPreparationV1};
    let fixture = all_route_input_fixture(true);
    let state = &fixture.state;
    let observed = state.verified_lane_consensus_contexts().unwrap().unwrap();
    let main = observed.contexts().iter().find(|lane| lane.frozen().lane_id == LaneId::SINGLE).unwrap();
    let earlier = observed.contexts().iter().find(|lane| lane.frozen().lane_id == LaneId::new(1)).unwrap();
    let FirstLaneAdmittedInputReadV1::Ready(source) = state.first_lane_admitted_input(&observed, main).unwrap() else { panic!("actual main source"); };
    let LaneInputBodyPreparationV1::BlockedByEarlierInputs(wait) = state.prepare_lane_input_body(&observed, main, &source).unwrap() else { panic!("secondary route retains its earlier obligation"); };
    assert_eq!(wait.len(), 1);
    assert_eq!(wait[0].instance_id, Hash::from(earlier.instance_id().0));
    assert_eq!(wait[0].binding_hash, earlier.frozen().admitted_binding_hash);
    assert_eq!(wait[0].route, (LaneId::new(1), DataSpaceId::UNIVERSAL));
    assert_eq!(wait[0].priority, earlier.frozen().admission_priority);
    assert!(wait[0].priority < source.priority());
    assert!(wait[0].priority.carrier_height < source.priority().carrier_height);
    assert!(earlier.frozen().opening_global_height < main.frozen().opening_global_height,
        "the earlier frozen owner survives the later admission carrier");
    let FirstLaneAdmittedInputReadV1::Ready(earlier_source) = state.first_lane_admitted_input(&observed, earlier).unwrap() else { panic!("actual earlier source"); };
    assert!(matches!(state.prepare_lane_input_body(&observed, earlier, &earlier_source).unwrap(), LaneInputBodyPreparationV1::Ready(_)),
        "the earlier owner remains runnable without depending on the later group");
    assert!(state.prepare_lane_input_body(&observed, main, &earlier_source).is_err(),
        "an authenticated other input cannot be substituted for the target");
}

state_test! { sync lane_input_body_rechecks_full_set_and_refuses_foreign_instance
    use super::{lane_admitted_input::FirstLaneAdmittedInputReadV1, lane_input_body::LaneInputBodyPreparationV1};
    let (fixture, _) = first_lane_input_fixture(0x82);
    let state = &fixture.state;
    let observed = state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &observed.contexts()[0];
    let FirstLaneAdmittedInputReadV1::Ready(source) = state.first_lane_admitted_input(&observed, lane).unwrap() else { panic!("actual source"); };
    let LaneInputBodyPreparationV1::Ready(body) = state.prepare_lane_input_body(&observed, lane, &source).unwrap() else { panic!("single route ready"); };
    assert_eq!(body.kind(), iroha_data_model::block::lane_consensus::LaneValueKindV1::Execution);
    let (foreign, _) = first_lane_input_fixture(0x83);
    let foreign_observed = foreign.state.verified_lane_consensus_contexts().unwrap().unwrap();
    assert!(matches!(state.prepare_lane_input_body(&observed, &foreign_observed.contexts()[0], &source).unwrap(), LaneInputBodyPreparationV1::InstanceNotCurrent));
    state.append_committed_block_header_for_tests(empty_global_block_after(Some(&fixture.block)).header());
    assert!(matches!(state.prepare_lane_input_body(&observed, lane, &source).unwrap(), LaneInputBodyPreparationV1::ObservationChanged),
        "retained source/body evidence is never a lease on a later State publication");
}

state_test! { sync lane_input_body_slot_selection_rejects_inconsistent_complete_sets
    use super::lane_input_body::{select_input_slots, SlotSelection};
    let fixture = all_route_input_fixture(false);
    let observed = fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();
    let priority = observed.contexts()[0].frozen().admission_priority;
    let exact = observed.contexts().iter().map(|lane|
        (lane.frozen().clone(), Hash::from(lane.instance_id().0))).collect::<Vec<_>>();
    let select = |values: &Vec<(FrozenLaneConsensusContextV1, Hash)>| {
        select_input_slots(&fixture.binding, priority, values.iter().map(|(frozen, id)| (frozen, *id)))
    };
    assert!(matches!(select(&exact).unwrap(), SlotSelection::Ready(_)));
    for mutation in 0..6 {
        let mut changed = exact.clone();
        match mutation {
            0 => { changed.pop(); },
            1 => { changed.push(changed[0].clone()); },
            2 => { changed[1].0.admitted_binding_hash = Hash::new(b"foreign equal-rank head"); },
            3 => {
                changed[1].0.admitted_binding_hash = Hash::new(b"foreign newer head");
                changed[1].0.admission_priority.carrier_height += 1;
            },
            4 => { changed[1].0.lane_incarnation = Hash::new(b"another incarnation"); },
            5 => { changed[1].0.admission_priority.admission_index += 1; },
            _ => unreachable!(),
        }
        assert!(select(&changed).is_err(), "structural mutation {mutation} cannot grant body authority");
    }
    let mut conflicting = fixture.binding.clone();
    let duplicate = conflicting.admission_context.route_incarnations.iter_mut().find(|route|
        route.leg.route.lane_id == LaneId::SINGLE && route.leg.role == crate::queue::RouteLegRole::Participant).unwrap();
    duplicate.lane_incarnation = Hash::new(b"conflicting role on the same route");
    assert!(select_input_slots(&conflicting, priority, exact.iter().map(|(frozen, id)| (frozen, *id))).is_err());
    // These mutated raw values never construct a VerifiedLaneContext/Contexts token.
}
