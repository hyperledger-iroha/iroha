// Native opening authority and admission-backed closed-lane regressions.

fn closed_lane_opening_authority_fixture(
    atomic: bool,
) -> (
    State,
    Vec<KeyPair>,
    crate::torii_proxy::QueuePlanAdmissionBindingV1,
    SignedBlock,
) {
    let (state, validators, _, parent) = configured_lane_context_queue_plan_state();
    let lane_id = LaneId::new(1);
    install_autoscale_elastic_catalog_for_test(
        &state,
        autoscale_elastic_catalog_lane_with_committee_for_test(lane_id, 1, &validators),
    );
    install_lane_manifest_registry_for_keypairs(&state, &[LaneId::SINGLE, lane_id], &validators);
    let closing_route = crate::queue::RoutingDecision::new(lane_id, DataSpaceId::UNIVERSAL);
    let plan = if atomic {
        crate::queue::RoutingPlan::native_amx(
            crate::queue::RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            vec![crate::queue::RouteLeg::new(
                closing_route,
                crate::queue::RouteLegRole::Participant,
            )],
        )
    } else {
        crate::queue::RoutingPlan::single(closing_route)
    };
    let (binding, certificate) = queue_plan_admission_certificate_for_state_test(
        &state,
        plan,
        &validators,
        parent.header().height().get(),
        0x71,
    );
    seed_exact_queue_plan_admission_state_for_test(&state, &certificate);
    let close = empty_global_block_after(Some(&parent));
    let mut overlay = state.block(close.header());
    let capacity = autoscale_default_route_capacity_lanes(
        &overlay.nexus.routing_policy,
        overlay.nexus.lane_catalog.lanes(),
        overlay.nexus.autoscale.min_lane_id.get(),
        overlay.nexus.autoscale.max_lane_id_exclusive.get(),
    );
    overlay
        .stage_autoscale_lane_drain_intent(lane_id, capacity, capacity, 0, 0)
        .expect("native staged close under the existing immutable pin");
    // The production autoscale driver records the carrier immediately after
    // staging the intent; the low-level staging helper deliberately leaves zero.
    overlay.record_autoscale_transition_height(close.header().height().get());
    state
        .validate_committed_autoscale_lane_lifecycle(
            overlay.pending_autoscale_lifecycle.as_ref().unwrap(),
            close.header().height().get(),
            close.header().hash(),
            None,
            &mut LaneLifecycleReleases::new(&state),
        )
        .expect("exact native drain staging must pass publication preflight");
    overlay.block_hashes.push(close.hash());
    insert_empty_transaction_block_for_state_commit(&mut overlay, &close);
    overlay
        .commit()
        .expect("commit exact native drain metadata");
    state
        .kura
        .store_block(Arc::new(close.clone()))
        .expect("store close carrier");
    (state, validators, binding, close)
}

state_test! { sync lane_opening_authority_uses_exact_staged_route_height_and_native_pops
    let (state, validators, _, parent) = configured_single_lane_queue_plan_state();
    let carrier = empty_global_block_after(Some(&parent));
    let height = carrier.header().height().get();
    let mut overlay = state.block(carrier.header());
    let lane = overlay.nexus.lane_catalog.lanes()[0].clone();
    let incarnation = overlay.lane_incarnations[&lane.id];
    let (committee, pops) = lane_consensus_authority::resolve_open_lane_authority(
        &overlay, &lane, incarnation, height,
    ).expect("completed staged live authority");
    let mut expected = validators.iter().map(|key| PeerId::new(key.public_key().clone())).collect::<Vec<_>>();
    expected.sort();
    assert_eq!(committee, expected);
    assert_eq!(pops.len(), 4);
    assert!(lane_consensus_authority::resolve_open_lane_authority(
        &overlay, &lane, incarnation, height + 1,
    ).is_err());
    assert!(lane_consensus_authority::resolve_open_lane_authority(
        &overlay, &lane, Hash::new(b"other incarnation"), height,
    ).is_err());
    let mut altered_lane = lane.clone();
    altered_lane.dataspace_id = DataSpaceId::new(99);
    assert!(lane_consensus_authority::resolve_open_lane_authority(
        &overlay, &altered_lane, incarnation, height,
    ).is_err());
    let id = derive_validator_key_id(committee[0].public_key());
    let mut record = overlay.world.consensus_keys.get(&id).unwrap().clone();
    record.pop = Some(vec![0; 96]);
    overlay.world.consensus_keys.insert(id, record);
    assert!(lane_consensus_authority::resolve_open_lane_authority(
        &overlay, &lane, incarnation, height,
    ).is_err(), "an unverified native PoP cannot enter the frozen context");
}

state_test! { sync lane_opening_authority_uses_completed_pending_manifest_projection
    // Autoscale's base profile is ungoverned. Bind its real alias and profile
    // to the validator rules before staging either block: the general autoscale
    // fixture installs a lane-0/parliament status which cannot survive this
    // catalog's exact lifecycle rebind.
    let primary = LaneConfig::default();
    let state = State::new_with_pre_genesis_nexus_for_testing(
        World::default(),
        autoscale_transition_test_nexus(vec![primary.clone()], 1, 3, 100),
        LiveQueryStore::start_test(),
    );
    let kura = Arc::clone(&state.kura);
    let keys = seed_autoscale_transport_peers_for_test(&state, 4);
    let validators = keys.iter()
        .map(|key| AccountId::new(key.public_key().clone())).collect::<Vec<_>>();
    let validator_bindings = validators.iter().zip(&keys)
        .map(|(validator, key)| ManifestValidatorBinding {
            validator: validator.clone(),
            peer_id: PeerId::new(key.public_key().clone()),
            torii_url: None,
        }).collect();
    state.install_lane_manifests(&Arc::new(LaneManifestRegistry::from_statuses(
        BTreeMap::from([(primary.id, LaneManifestStatus {
            lane: primary.id,
            alias: primary.alias.clone(),
            dataspace: primary.dataspace_id,
            visibility: primary.visibility,
            storage: primary.storage,
            governance: primary.governance.clone(),
            manifest_path: Some(PathBuf::from("/tmp/default-lane-authority.manifest.json")),
            governance_rules: Some(GovernanceRules {
                validators, validator_bindings, ..GovernanceRules::default()
            }),
            privacy_commitments: Vec::new(),
        })]),
    )));
    let directory = tempfile::tempdir().expect("isolated staged-authority cold store");
    let cold_root = directory.path().join("cold");
    *state.tiered_backend.lock() =
        TieredStateBackend::new(true, 0, 0, 0, Some(cold_root), None, 1, 0);
    let first = autoscale_signed_block_with_committed_fragments(None, 100, 0);
    let mut first_overlay = state.block(first.header());
    let committed_first = ValidBlock::new_unverified_for_tests(first.clone())
        .commit_unchecked().unpack(|_| {});
    let _ = first_overlay.apply_without_execution(&committed_first, Vec::new());
    first_overlay.commit().expect("commit native autoscale observation");
    store_committed_autoscale_history_block_for_test(&state, &kura, &first);
    let second = autoscale_signed_block_with_committed_fragments(Some(&first), 200, 20);
    let mut state_block = state.block(second.header());
    state_block.add_committed_fragments(20);
    let committed_second = ValidBlock::new_unverified_for_tests(second)
        .commit_unchecked().unpack(|_| {});
    let _ = state_block.apply_without_execution(&committed_second, Vec::new());
    assert!(state_block.pending_autoscale_lifecycle.is_some(),
        "native scale-out must stage the manifest projection under test");
    let lane = state_block.nexus.lane_catalog.lanes().iter()
        .find(|lane| lane.id == LaneId::SINGLE).unwrap().clone();
    let incarnation = state_block.lane_incarnations[&lane.id];
    let height = state_block._curr_block.height().get();
    let pending = state_block.pending_autoscale_lifecycle.as_ref().unwrap();
    // This existing fixture installs status-only manifests, which intentionally
    // claim no source/catalog binding. Require the exact lifecycle derivation
    // and active coverage instead of inventing a source snapshot for the test.
    let rebound = rebind_lane_manifests_for_lifecycle(
        state_block.lane_manifests.as_ref(), &state_block.nexus.lane_catalog,
        &state_block.nexus.governance,
    ).unwrap();
    assert_eq!(pending.updated_lane_manifests.consensus_policy_digest(),
        rebound.consensus_policy_digest());
    pending.updated_lane_manifests.validate_active_coverage_for_catalog(
        &state_block.nexus.lane_catalog,
    ).unwrap();
    let expected = lane_authority::resolve_from_sources(
        &state_block.world, &state_block.network_id,
        LaneAuthorityRoute::new(lane.id, lane.dataspace_id),
        &pending.updated_lane_manifests, &state_block.nexus, height,
    ).unwrap().into_validators();
    // The completed lifecycle projection owns policy; a stale pre-transition
    // handle must not be consulted instead of that exact pending projection.
    state_block.lane_manifests = Arc::new(LaneManifestRegistry::from_statuses(BTreeMap::new()));
    assert!(state_block.resolve_lane_committee_at_height(
        LaneAuthorityRoute::new(lane.id, lane.dataspace_id), height,
    ).is_err());
    assert_eq!(lane_consensus_authority::resolve_open_lane_authority(
        &state_block, &lane, incarnation, height,
    ).unwrap().0, expected);
}

state_test! { large_stack lane_opening_authority_finishes_preclose_admissions_without_reopening_ingress
    let (state, _, binding, close) = closed_lane_opening_authority_fixture(false);
    let carrier = empty_global_block_after(Some(&close));
    let height = carrier.header().height().get();
    let mut overlay = state.block(carrier.header());
    let lane = overlay.nexus.lane_catalog.lanes().iter()
        .find(|lane| lane.id == LaneId::new(1)).unwrap().clone();
    let incarnation = overlay.lane_incarnations[&lane.id];
    let pin = decode_autoscale_lane_committee(&lane).unwrap().unwrap();
    assert_eq!(binding.admission_context.proposal_height, close.header().height().get());
    assert!(overlay.resolve_lane_committee_at_height(
        LaneAuthorityRoute::new(lane.id, lane.dataspace_id), height,
    ).is_err(), "generic new-ingress authority stays closed");
    assert_eq!(lane_consensus_authority::resolve_open_lane_authority(
        &overlay, &lane, incarnation, height,
    ).unwrap(), (pin.validator_set.clone(), pin.validator_pops.clone()));
    let ranked_key = State::queue_plan_admission_registry_marker_key(&binding.registry_key()).unwrap();
    let original_ranked = overlay.world.smart_contract_state.get(&ranked_key).unwrap().clone();
    overlay.world.smart_contract_state.insert(ranked_key.clone(),
        State::queue_plan_admission_registry_marker_payload(&binding.registry_value(),
            QueuePlanAdmissionPriorityV1::new(height, 0).unwrap(),
        ).unwrap());
    assert!(lane_consensus_authority::resolve_open_lane_authority(
        &overlay, &lane, incarnation, height,
    ).is_err(), "an old signed proposal height cannot hide first admission after close");
    overlay.world.smart_contract_state.insert(ranked_key, original_ranked);
    let original_catalog = overlay.nexus.lane_catalog.clone();
    let drain = decode_autoscale_lane_drain_state(&lane).unwrap().unwrap();
    let mut foreign_network = drain.clone();
    foreign_network.intent.network_id = iroha_data_model::NetworkId::from_genesis_hash(
        HashOf::from_untyped_unchecked(Hash::new(b"foreign opening network")),
    );
    let mut foreign_incarnation = drain;
    foreign_incarnation.intent.lane_incarnation = Hash::new(b"foreign drain incarnation");
    foreign_incarnation.intent.initial_frontier.lane_incarnation =
        foreign_incarnation.intent.lane_incarnation;
    for changed in [foreign_network, foreign_incarnation] {
        let mut lanes = original_catalog.lanes().to_vec();
        let changed_lane = lanes.iter_mut().find(|candidate| candidate.id == lane.id).unwrap();
        changed_lane.metadata.insert(
            AUTOSCALE_META_DRAIN_STATE.to_owned(),
            encode_autoscale_lane_drain_state(&changed).unwrap(),
        );
        let changed_lane = changed_lane.clone();
        overlay.nexus.lane_catalog = LaneCatalog::new(original_catalog.lane_count(), lanes).unwrap();
        assert!(lane_consensus_authority::resolve_open_lane_authority(
            &overlay, &changed_lane, incarnation, height,
        ).is_err(), "a structurally valid drain must bind this network and incarnation");
    }
    overlay.nexus.lane_catalog = original_catalog;
    for peer in &pin.validator_set {
        overlay.world.consensus_keys.remove(derive_validator_key_id(peer.public_key()));
        overlay.world.consensus_keys_by_pk.remove(peer.public_key().to_string());
    }
    assert_eq!(lane_consensus_authority::resolve_open_lane_authority(
        &overlay, &lane, incarnation, height,
    ).unwrap(), (pin.validator_set, pin.validator_pops),
        "drain uses the verified immutable pin, not a reconstructed live key cache");
    let registry_key = State::queue_plan_admission_registry_marker_key(&binding.registry_key()).unwrap();
    overlay.world.smart_contract_state.remove(registry_key);
    assert!(lane_consensus_authority::resolve_open_lane_authority(
        &overlay, &lane, incarnation, height,
    ).is_err(), "route-member bytes alone cannot authenticate admission custody");
}

state_test! { sync lane_opening_authority_rejects_any_postclose_member_and_empty_drain
    let (state, validators, mut late_binding, close) = closed_lane_opening_authority_fixture(false);
    late_binding.admission_context.authority_height = close.header().height().get();
    late_binding.admission_context.proposal_height = close.header().height().get() + 1;
    late_binding.admission_context.predecessor_block_hash = Some(close.hash());
    let late_entrypoint = queue_plan_entrypoint_for_state_test(&state, 0x72);
    let plan = crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
        LaneId::new(1), DataSpaceId::UNIVERSAL,
    ));
    let late_binding = crate::torii_proxy::new_queue_plan_admission_binding(
        &state.network_id, &late_entrypoint, &plan, late_binding.admission_context, 901,
    ).unwrap();
    let certificate = queue_plan_admission_certificate_bytes_for_state_test(&late_entrypoint, &late_binding, &validators);
    seed_exact_queue_plan_admission_state_for_test(&state, &certificate);
    let carrier = empty_global_block_after(Some(&close));
    let height = carrier.header().height().get();
    let mut overlay = state.block(carrier.header());
    let lane = overlay.nexus.lane_catalog.lanes().iter()
        .find(|lane| lane.id == LaneId::new(1)).unwrap().clone();
    let incarnation = overlay.lane_incarnations[&lane.id];
    let route = QueuePlanPendingObligationRouteV1 {
        version: QUEUE_PLAN_PENDING_OBLIGATION_VERSION_V1,
        lane_id: lane.id, dataspace_id: lane.dataspace_id, lane_incarnation: incarnation,
    };
    let members = State::queue_plan_pending_route_members_from_storage(
        overlay.world.smart_contract_state(), route,
    ).unwrap();
    assert_eq!(members.len(), 2, "both signed admissions are present; only one is pre-close");
    assert!(lane_consensus_authority::resolve_open_lane_authority(
        &overlay, &lane, incarnation, height,
    ).is_err(), "one valid pre-close admission cannot hide a later member");
    for (key, _) in members {
        overlay.world.smart_contract_state.remove(key);
    }
    assert!(lane_consensus_authority::resolve_open_lane_authority(
        &overlay, &lane, incarnation, height,
    ).is_err(), "no pending member can open a closed route");
}

state_test! { sync queue_plan_pending_route_authority_distinguishes_active_absent_and_applied
    let (state, validators, _, parent) = configured_lane_context_queue_plan_state();
    let (binding, certificate) = queue_plan_admission_certificate_for_state_test(
        &state,
        crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
            LaneId::SINGLE, DataSpaceId::UNIVERSAL,
        )),
        &validators, parent.header().height().get(), 0x81,
    );
    assert!(matches!(
        State::queue_plan_pending_route_authority_in_view(&state.view(), &binding).unwrap(),
        None,
    ), "an authenticated certificate without canonical pending custody grants no continuation");
    let admission_carrier = lane_context_admission_carrier_for_test(&parent, &certificate);
    let opening = lane_opening_context_for_state_test(&state);
    {
        let mut admission = state.block_with_queue_plan_admissions(
            admission_carrier.header(), &[certificate],
        ).unwrap();
        admission.finalize_lane_consensus_contexts(&admission_carrier, Some(&opening)).unwrap();
        admission.stage_autoscale_sample_record_for_count(&admission_carrier, 0).unwrap();
        admission.block_hashes.push(admission_carrier.hash());
        insert_empty_transaction_block_for_state_commit(&mut admission, &admission_carrier);
        admission.commit().expect("commit actual ranked admission carrier");
    }
    state.kura.store_block(Arc::new(admission_carrier.clone())).unwrap();
    assert!(matches!(
        State::queue_plan_pending_route_authority_in_view(&state.view(), &binding).unwrap(),
        Some(QueuePlanPendingRouteAuthority::Active),
    ));
    let carrier = empty_global_block_after(Some(&admission_carrier));
    let mut overlay = state.block(carrier.header());
    assert!(State::resolve_queue_plan_pending_obligation_in_storage(
        &mut overlay.world.smart_contract_state, binding.network_id_digest,
        binding.entrypoint_hash,
    ).unwrap());
    overlay.transactions.insert_block_with_single_tx(
        binding.entrypoint_hash,
        NonZeroUsize::new(usize::try_from(carrier.header().height().get()).unwrap()).unwrap(),
    );
    assert_eq!(
        State::queue_plan_binding_application_evidence_in_view(&overlay, &binding).unwrap(),
        QueuePlanBindingApplicationEvidence::AppliedDirect,
    );
    assert!(matches!(
        State::queue_plan_pending_route_authority_in_view(&overlay, &binding).unwrap(),
        None,
    ), "terminal application must not grant another pending execution");
    drop(overlay);
    assert!(matches!(
        State::queue_plan_pending_route_authority_in_view(&state.view(), &binding).unwrap(),
        Some(QueuePlanPendingRouteAuthority::Active),
    ), "the inspection and aborted terminal overlay publish nothing");
}

state_test! { large_stack queue_plan_pending_route_authority_keeps_closed_owner_under_immutable_pin
    let (state, _, binding, close) = closed_lane_opening_authority_fixture(false);
    assert!(matches!(
        State::queue_plan_pending_route_authority_in_view(&state.view(), &binding).unwrap(),
        Some(QueuePlanPendingRouteAuthority::Draining),
    ));
    let carrier = empty_global_block_after(Some(&close));
    let mut overlay = state.block(carrier.header());
    let lane = overlay.nexus.lane_catalog.lanes().iter()
        .find(|lane| lane.id == LaneId::new(1)).unwrap().clone();
    assert!(overlay.resolve_lane_committee_at_height(
        LaneAuthorityRoute::new(lane.id, lane.dataspace_id), carrier.header().height().get(),
    ).is_err(), "fresh admission remains closed");
    let pin = decode_autoscale_lane_committee(&lane).unwrap().unwrap();
    for peer in &pin.validator_set {
        overlay.world.consensus_keys.remove(derive_validator_key_id(peer.public_key()));
        overlay.world.consensus_keys_by_pk.remove(peer.public_key().to_string());
    }
    assert!(matches!(
        State::queue_plan_pending_route_authority_in_view(&overlay, &binding).unwrap(),
        Some(QueuePlanPendingRouteAuthority::Draining),
    ), "accepted work retains the original pin after live-key churn");
    assert_eq!(State::queue_plan_pending_binding_in_view(
        &overlay, binding.entrypoint_hash,
    ).unwrap(), Some(binding.clone()));
    drop(overlay);
    assert_eq!(state.latest_block_hash_fast(), Some(close.hash()));
}

state_test! { sync queue_plan_pending_route_authority_rejects_rank_and_marker_corruption
    let (state, _, binding, close) = closed_lane_opening_authority_fixture(false);
    let carrier = empty_global_block_after(Some(&close));
    let mut overlay = state.block(carrier.header());
    let key = State::queue_plan_admission_registry_marker_key(&binding.registry_key()).unwrap();
    let original = overlay.world.smart_contract_state.get(&key).unwrap().clone();
    let source_height = binding.admission_context.proposal_height;
    assert!(source_height > 1);
    for invalid_rank in [source_height - 1, close.header().height().get() + 1] {
        overlay.world.smart_contract_state.insert(key.clone(),
            State::queue_plan_admission_registry_marker_payload(&binding.registry_value(),
                QueuePlanAdmissionPriorityV1::new(invalid_rank, 0).unwrap(),
            ).unwrap());
        assert!(State::queue_plan_pending_route_authority_in_view(&overlay, &binding).is_err(),
            "rank {invalid_rank} must not precede the signed source or follow the close");
    }
    overlay.world.smart_contract_state.insert(key.clone(), vec![0xFF]);
    assert!(State::queue_plan_pending_route_authority_in_view(&overlay, &binding).is_err(),
        "corrupt ranked custody is not absence");
    overlay.world.smart_contract_state.remove(key.clone());
    assert!(State::queue_plan_pending_route_authority_in_view(&overlay, &binding).is_err(),
        "pending obligation and route members cannot survive a missing registry owner");
    overlay.world.smart_contract_state.insert(key, original);
    assert!(matches!(
        State::queue_plan_pending_route_authority_in_view(&overlay, &binding).unwrap(),
        Some(QueuePlanPendingRouteAuthority::Draining),
    ));
}

state_test! { large_stack queue_plan_pending_route_authority_rejects_drain_identity_pin_and_commitment
    let (state, _, binding, close) = closed_lane_opening_authority_fixture(false);
    let carrier = empty_global_block_after(Some(&close));
    let mut overlay = state.block(carrier.header());
    let catalog = overlay.nexus.lane_catalog.clone();
    let lane = catalog.lanes().iter().find(|lane| lane.id == LaneId::new(1)).unwrap();
    let original_drain = decode_autoscale_lane_drain_state(lane).unwrap().unwrap();
    let mut foreign_network = original_drain.clone();
    foreign_network.intent.network_id = iroha_data_model::NetworkId::from_genesis_hash(
        HashOf::from_untyped_unchecked(Hash::new(b"foreign pending-route network")),
    );
    let mut foreign_incarnation = original_drain.clone();
    foreign_incarnation.intent.lane_incarnation = Hash::new(b"foreign pending-route incarnation");
    foreign_incarnation.intent.initial_frontier.lane_incarnation =
        foreign_incarnation.intent.lane_incarnation;
    let mut completed = original_drain.clone();
    completed.commitment = Some(iroha_data_model::merge::LaneDrainCommitmentV1 {
        version: iroha_data_model::merge::LaneDrainCommitmentV1::VERSION,
        certificate_hash: HashOf::from_untyped_unchecked(Hash::new(b"already-carried drain certificate")),
        merge_entry_hash: HashOf::from_untyped_unchecked(Hash::new(b"already-carried drain entry")),
        carrier_height: carrier.header().height().get(),
        frontier: original_drain.intent.initial_frontier,
    });
    for changed in [foreign_network, foreign_incarnation, completed] {
        let mut lanes = catalog.lanes().to_vec();
        let target = lanes.iter_mut().find(|lane| lane.id == LaneId::new(1)).unwrap();
        target.metadata.insert(AUTOSCALE_META_DRAIN_STATE.to_owned(),
            encode_autoscale_lane_drain_state(&changed).unwrap());
        assert!(decode_autoscale_lane_drain_state(target).unwrap().is_some(),
            "negative metadata remains structurally well formed");
        overlay.nexus.lane_catalog = LaneCatalog::new(catalog.lane_count(), lanes).unwrap();
        assert!(State::queue_plan_pending_route_authority_in_view(&overlay, &binding).is_err(),
            "wrong drain identity or an already certified drain cannot authorize pending work");
    }
    let mut lanes = catalog.lanes().to_vec();
    let target = lanes.iter_mut().find(|lane| lane.id == LaneId::new(1)).unwrap();
    attach_synthetic_autoscale_committee_for_test(target);
    let foreign_pin = decode_autoscale_lane_committee(target).unwrap().unwrap();
    assert_ne!(foreign_pin.validator_set, binding.admission_context.route_incarnations[0].validator_set);
    let mut changed = original_drain;
    changed.intent.validator_set_hash_version = foreign_pin.validator_set_hash_version;
    changed.intent.validator_set_hash = foreign_pin.validator_set_hash;
    changed.intent.validator_set = foreign_pin.validator_set;
    changed.intent.validator_count = foreign_pin.validator_count;
    changed.intent.min_quorum = foreign_pin.min_quorum;
    target.metadata.insert(AUTOSCALE_META_DRAIN_STATE.to_owned(),
        encode_autoscale_lane_drain_state(&changed).unwrap());
    assert!(decode_autoscale_lane_drain_state(target).unwrap().is_some());
    overlay.nexus.lane_catalog = LaneCatalog::new(catalog.lane_count(), lanes).unwrap();
    assert!(State::queue_plan_pending_route_authority_in_view(&overlay, &binding).is_err(),
        "a coherent replacement pin/drain pair cannot replace the original admission committee");
    overlay.nexus.lane_catalog = catalog;
    assert!(matches!(
        State::queue_plan_pending_route_authority_in_view(&overlay, &binding).unwrap(),
        Some(QueuePlanPendingRouteAuthority::Draining),
    ));
}

state_test! { large_stack queue_plan_pending_route_authority_checks_every_atomic_leg
    let (state, _, binding, close) = closed_lane_opening_authority_fixture(true);
    assert_eq!(binding.admission_context.route_incarnations.len(), 2);
    assert_eq!(binding.admission_context.route_incarnations[0].leg.route.lane_id, LaneId::SINGLE);
    assert_eq!(binding.admission_context.route_incarnations[1].leg.route.lane_id, LaneId::new(1));
    assert!(matches!(
        State::queue_plan_pending_route_authority_in_view(&state.view(), &binding).unwrap(),
        Some(QueuePlanPendingRouteAuthority::Draining),
    ), "an active coordinator cannot hide its draining participant");
    let carrier = empty_global_block_after(Some(&close));
    let mut overlay = state.block(carrier.header());
    for route in &binding.admission_context.route_incarnations {
        let lane_id = route.leg.route.lane_id;
        let original = overlay.lane_incarnations[&lane_id];
        overlay.lane_incarnations.insert(lane_id, Hash::new(b"recreated atomic member"));
        assert!(State::queue_plan_pending_route_authority_in_view(&overlay, &binding).is_err(),
            "every active or draining participant must retain the exact incarnation");
        overlay.lane_incarnations.insert(lane_id, original);
    }
    let obligation = State::queue_plan_pending_obligation_from_binding(&binding).unwrap();
    for route in &obligation.routes {
        let member = State::queue_plan_pending_route_member_from_obligation(&obligation, *route).unwrap();
        let key = State::queue_plan_pending_route_member_marker_key(*route, member.member_identity).unwrap();
        let original = overlay.world.smart_contract_state.get(&key).unwrap().clone();
        overlay.world.smart_contract_state.remove(key.clone());
        assert!(State::queue_plan_pending_route_authority_in_view(&overlay, &binding).is_err(),
            "a partial atomic pending index cannot authorize either leg");
        overlay.world.smart_contract_state.insert(key, original);
    }
    assert!(matches!(
        State::queue_plan_pending_route_authority_in_view(&overlay, &binding).unwrap(),
        Some(QueuePlanPendingRouteAuthority::Draining),
    ));
}
