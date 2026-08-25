#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one linear test covers participant, incarnation, corrupt conflict, and malformed durable evidence"
)]
fn pending_queue_plan_evidence_blocks_every_bound_route_and_classifies_losers() {
    let (state, validator_keypairs, _, parent) = configured_two_lane_merge_state();
    let participant_lane = LaneId::new(1);
    let_row! { routing_plan = crate::queue::RoutingPlan::native_amx( crate::queue::RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL), vec![crate::queue::RouteLeg::new( crate::queue::RoutingDecision::new(participant_lane, DataSpaceId::UNIVERSAL), crate::queue::RouteLegRole::Participant, )], ) };
    let_row! { (binding, certificate) = queue_plan_admission_certificate_for_state_test( &state, routing_plan, &validator_keypairs, queue_plan_authority_height_for_state_test(&state), 0x61, ) };
    let proposal_height = binding.admission_context.proposal_height;
    let_row! { pending_hash = state .kura .persist_pending_queue_plan_admission_certificate(&certificate) .expect("persist participant-bound QueuePlan certificate") };
    let_row! { participant_incarnation = state .lane_incarnation(participant_lane) .expect("participant lane incarnation") };
    assert!(
        state.lane_has_drain_blocking_evidence(
            participant_lane,
            DataSpaceId::UNIVERSAL,
            participant_incarnation,
        ),
        "a durable pending participant claim must block its exact lane incarnation"
    );
    assert!(
        !state.lane_has_drain_blocking_evidence(
            participant_lane,
            DataSpaceId::UNIVERSAL,
            Hash::new(b"unrelated-recreated-incarnation"),
        ),
        "pending evidence from another incarnation must not block a recreated lane"
    );
    state
        .kura
        .remove_pending_queue_plan_admission_certificate(pending_hash)
        .expect("remove first pending fixture");
    let replacement_incarnation = Hash::new(b"replacement-participant-incarnation");
    let_row! { _ = state .lane_incarnations .write() .insert(participant_lane, replacement_incarnation) };
    let_row! { stale_hash = state .kura .persist_pending_queue_plan_admission_certificate(&certificate) .expect("persist authenticated stale QueuePlan certificate") };
    assert_eq!(
        state
            .classify_pending_queue_plan_admission(&certificate, proposal_height)
            .expect("authenticated stale evidence is classifiable")
            .1,
        PendingQueuePlanAdmissionDisposition::Stale
    );
    assert!(
        queue_plan_admission_registry_match(
            &state.view(),
            binding.entrypoint_hash.clone(),
            binding.canonical_hash(),
        )
        .is_err(),
        "Queue recovery must not reacquire an exact owner bound to incarnation A"
    );
    assert!(
        state
            .queue_plan_admission_binding_registry_match(&binding)
            .is_err(),
        "public binding acknowledgement must report a stale pending incarnation"
    );
    assert!(
        !state.lane_has_drain_blocking_evidence(
            participant_lane,
            DataSpaceId::UNIVERSAL,
            replacement_incarnation,
        ),
        "an authenticated stale claim is a definitive loser, not a drain blocker"
    );
    state
        .kura
        .remove_pending_queue_plan_admission_certificate(stale_hash)
        .expect("remove stale pending fixture");
    let_row! { _ = state .lane_incarnations .write() .insert(participant_lane, participant_incarnation) };
    let_row! { registry_key = State::queue_plan_admission_registry_marker_key(&binding.registry_key()) .expect("fixture registry key") };
    let_row! { conflicting_value = crate::torii_proxy::QueuePlanAdmissionRegistryValueV2 { version: crate::torii_proxy::QUEUE_PLAN_ADMISSION_BINDING_VERSION_V2, binding_hash: Hash::new(b"different-immutable-queue-plan-binding"), } };
    let_row! { conflicting_payload = State::queue_plan_admission_registry_marker_payload(&conflicting_value) .expect("fixture conflicting registry value") };
    {
        let mut world = state.world.block();
        world
            .smart_contract_state
            .insert(registry_key.clone(), conflicting_payload);
        world.commit();
    }
    assert!(
        state
            .classify_pending_queue_plan_admission(&certificate, proposal_height)
            .is_err(),
        "a conflicting registry hash without pending-or-applied owner evidence is corrupt"
    );
    assert!(
        state
            .queue_plan_admission_binding_registry_match(&binding)
            .is_err(),
        "a partial conflicting owner must not be reported as definitive"
    );
    let_row! { conflict_hash = state .kura .persist_pending_queue_plan_admission_certificate(&certificate) .expect("persist definitive conflict fixture") };
    assert!(
        state.lane_has_drain_blocking_evidence(
            participant_lane,
            DataSpaceId::UNIVERSAL,
            participant_incarnation,
        ),
        "corrupt conflicting ownership must fail closed for drain"
    );
    state
        .kura
        .remove_pending_queue_plan_admission_certificate(conflict_hash)
        .expect("remove conflict pending fixture");
    seed_exact_queue_plan_admission_state_for_test(&state, &certificate);
    assert_eq!(
        state
            .queue_plan_admission_binding_registry_match(&binding)
            .expect("exact registry marker"),
        QueuePlanAdmissionRegistryMatch::Exact
    );
    let_row! { obligation = State::queue_plan_pending_obligation_from_binding(&binding) .expect("fixture exact pending obligation") };
    let_row! { participant_route = *obligation .routes .iter() .find(|route| route.lane_id == participant_lane) .expect("fixture exact participant route") };
    let_row! { participant_member_identity = State::queue_plan_pending_route_member_identity(&obligation, participant_route) .expect("fixture exact participant member identity") };
    let_row! { incarnation_a_member_key = State::queue_plan_pending_route_member_marker_key( participant_route, participant_member_identity, ) .expect("fixture incarnation-A member key") };
    let_row! { incarnation_b_route = QueuePlanPendingObligationRouteV1 { lane_incarnation: replacement_incarnation, ..participant_route } };
    let_row! { (incarnation_b_member_prefix, _) = State::queue_plan_pending_route_member_marker_prefix(incarnation_b_route) .expect("fixture incarnation-B member prefix") };
    assert!(
        !incarnation_a_member_key
            .as_ref()
            .starts_with(&incarnation_b_member_prefix)
    );
    let_row! { _ = state .lane_incarnations .write() .insert(participant_lane, replacement_incarnation) };
    {
        let world = state.world.view();
        assert!(
            world
                .smart_contract_state()
                .get(&incarnation_a_member_key)
                .is_some()
        );
        assert!(
            State::queue_plan_pending_route_members_from_storage(
                world.smart_contract_state(),
                incarnation_b_route,
            )
            .expect("inspect exact incarnation-B member roster")
            .is_empty()
        );
    }
    assert!(
        !state.lane_has_drain_blocking_evidence(
            participant_lane,
            DataSpaceId::UNIVERSAL,
            replacement_incarnation,
        ),
        "incarnation-A WSV witnesses must not drain-block same-ID incarnation B"
    );
    assert_eq!(
        state
            .classify_pending_queue_plan_admission(&certificate, proposal_height)
            .expect("delayed incarnation-A evidence remains classifiable")
            .1,
        PendingQueuePlanAdmissionDisposition::Stale
    );
    let lifecycle = state.lane_consensus_lifecycle_snapshot();
    let_row! { active_lanes = lifecycle .nexus .lane_catalog .lanes() .iter() .map(|lane| MergeLaneBinding { lane_id: lane.id, dataspace_id: lane.dataspace_id, lane_config_hash: merge_lane_config_hash(lane), incarnation: lifecycle.incarnations[&lane.id], activation_height: lifecycle.activation_heights[&lane.id].saturating_add(1), }) .collect::<Vec<_>>() };
    let carrier = empty_global_block_after(Some(&parent));
    let mut delayed_block = state.block(carrier.header());
    let delayed_write_set_before = delayed_block.merge_execution_write_set_root();
    assert!(
        delayed_block
            .stage_queue_plan_admissions(
                &[certificate.clone()],
                &active_lanes,
                carrier.header().height().get(),
            )
            .is_err(),
        "the production StateBlock boundary must reject delayed incarnation-A evidence"
    );
    assert_eq!(
        delayed_block.merge_execution_write_set_root(),
        delayed_write_set_before,
        "stale evidence rejection must not publish incarnation-B WSV writes"
    );
    drop(delayed_block);
    let_row! { _ = state .lane_incarnations .write() .insert(participant_lane, participant_incarnation) };
    {
        let mut world = state.world.block();
        world.smart_contract_state.insert(registry_key, vec![0x00]);
        world.commit();
    }
    assert!(
        state
            .queue_plan_admission_binding_registry_match(&binding)
            .is_err(),
        "malformed WSV registry markers must be reported as errors"
    );
    let_row! { malformed_hash = state .kura .persist_pending_queue_plan_admission_certificate(&[0xFF]) .expect("persist malformed pending evidence") };
    assert!(
        state.lane_has_drain_blocking_evidence(
            participant_lane,
            DataSpaceId::UNIVERSAL,
            participant_incarnation,
        ),
        "malformed durable pending evidence must fail closed for drain"
    );
    state
        .kura
        .remove_pending_queue_plan_admission_certificate(malformed_hash)
        .expect("remove malformed pending fixture");
}
fn next_relay_merge_entry(
    state: &State,
    lane_height: u64,
    validator_keypairs: &[KeyPair],
    commit_keypairs: &[KeyPair],
) -> MergeLedgerEntry {
    next_relay_merge_entry_for_lane(
        state,
        lane_height,
        LaneId::SINGLE,
        validator_keypairs,
        commit_keypairs,
    )
}
fn next_relay_merge_entry_for_lane(
    state: &State,
    lane_height: u64,
    lane_id: LaneId,
    validator_keypairs: &[KeyPair],
    commit_keypairs: &[KeyPair],
) -> MergeLedgerEntry {
    let_row! { envelope = seed_effect_authenticated_relay_for_merge_test( state, sample_lane_relay_envelope_for_state(state, lane_height, lane_id, validator_keypairs), ) };
    state
        .record_lane_relay(&envelope)
        .expect("record next contiguous merge relay");
    let_row! { candidate = state .merge_entry_candidates_from_lane_relays() .into_iter() .next() .expect("next relay merge candidate") };
    let qc = merge_qc_for_candidate(state, &candidate, commit_keypairs, &[0]);
    merge_entry_from_candidate(candidate, qc)
}
fn store_and_commit_exact_merge_carrier(
    state: &State,
    previous: &SignedBlock,
    entry: &MergeLedgerEntry,
) -> SignedBlock {
    let carrier = certified_merge_carrier_after(previous, entry);
    state
        .kura
        .store_block_with_merge_entry(Arc::new(carrier.clone()), entry)
        .expect("store exact merge carrier and full entry");
    commit_exact_merge_carrier_to_state(state, &carrier, entry);
    carrier
}
fn persist_merge_entry_with_exact_carrier(mut entry: MergeLedgerEntry) -> Arc<Kura> {
    let kura = Kura::blank_kura_for_testing();
    let parent = empty_global_block_after(None);
    let carrier = empty_global_block_after(Some(&parent));
    entry.merge_qc.carrier_height = carrier.header().height().get();
    entry.merge_qc.carrier_parent_hash = carrier
        .header()
        .prev_block_hash()
        .expect("non-genesis carrier has a parent");
    entry.merge_qc.view = carrier.header().view_change_index();
    let carrier = certified_merge_carrier_after(&parent, &entry);
    kura.store_block(Arc::new(parent.clone()))
        .expect("store invalid-history carrier parent");
    kura.store_block_with_merge_entry(Arc::new(carrier.clone()), &entry)
        .expect("store exact invalid-history carrier fixture");
    persist_merge_carrier_finality_for_state_test(&kura, &carrier);
    kura
}
fn panic_payload_text(payload: Box<dyn core::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<String>() {
        return message.clone();
    }
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        return (*message).to_owned();
    }
    "non-string panic payload".to_owned()
}
state_test! { sync staged_merge_missing_transaction_block_mutates_nothing
    let (state, validator_keypairs, commit_keypairs, parent) = configured_single_lane_merge_state();
    let entry = next_relay_merge_entry(&state, 1, &validator_keypairs, &commit_keypairs);
    let carrier = certified_merge_carrier_after(&parent, &entry);
    state
        .kura
        .store_block_with_merge_entry(Arc::new(carrier.clone()), &entry)
        .expect("store exact merge carrier before State commit");
    let committed_height_before = state.committed_height();
    let state_hash_before = state.lane_execution_state_hash();
    let roots_before = state.world.merge_hint_roots.view().clone();
    let global_root_before = *state.world.merge_global_state_root.view();
    let cache_before = state.merge_ledger.snapshot();
    let_row! { mut state_block = state .block_with_certified_merge_entry(carrier.header().clone(), &entry, ConsensusMode::Permissioned) .expect("stage exact certified merge entry") };
    state_block.block_hashes.push(carrier.hash());
    let_row! { error = state_block .commit() .expect_err("missing transaction membership must abort the staged merge") };
    assert!(matches!(error, TransactionsBlockError::MissingInsertBlock));
    assert_eq!(state.committed_height(), committed_height_before);
    assert_eq!(state.lane_execution_state_hash(), state_hash_before);
    assert_eq!(*state.world.merge_hint_roots.view(), roots_before);
    assert_eq!(
        *state.world.merge_global_state_root.view(),
        global_root_before
    );
    assert_eq!(state.merge_ledger.snapshot(), cache_before);
    let admission = state.merge_admission.read();
    assert!(admission.latest_entry().is_none());
    assert!(admission.latest_lane_snapshots.is_empty());
    assert!(admission.latest_execution_heights.is_empty());
}
state_test! { sync durable_kura_carrier_requires_exact_committed_state_carrier_before_publication
    let (state, validator_keypairs, commit_keypairs, parent) = configured_single_lane_merge_state();
    let entry = next_relay_merge_entry(&state, 1, &validator_keypairs, &commit_keypairs);
    let carrier = certified_merge_carrier_after(&parent, &entry);
    state
        .kura
        .store_block_with_merge_entry(Arc::new(carrier.clone()), &entry)
        .expect("store exact Kura carrier");
    persist_merge_carrier_finality_for_state_test(&state.kura, &carrier);
    let state_hash_before = state.lane_execution_state_hash();
    let cache_before = state.merge_ledger.snapshot();
    let_row! { error = state .record_globally_committed_merge_entry(&entry, MergeLedgerPublicationMode::LiveCommit) .expect_err("Kura durability alone must not publish merge admission") };
    assert!(matches!(
        error,
        MergeLedgerCommitError::ExecutionStatePublication(reason)
            if reason.contains("absent from committed State history")
    ));
    assert_eq!(state.lane_execution_state_hash(), state_hash_before);
    assert_eq!(state.merge_ledger.snapshot(), cache_before);
    {
        let admission = state.merge_admission.read();
        assert!(admission.latest_entry().is_none());
        assert!(admission.latest_lane_snapshots.is_empty());
        assert!(admission.latest_execution_heights.is_empty());
    }
    commit_exact_merge_carrier_to_state(&state, &carrier, &entry);
    assert_eq!(state.merge_ledger.snapshot().len(), 1);
    assert_eq!(state.merge_ledger.latest().as_deref(), Some(&entry));
    {
        let admission = state.merge_admission.read();
        assert_eq!(admission.latest_entry(), Some(&entry));
        assert_eq!(admission.expected_epoch(), 2);
        assert_eq!(admission.latest_lane_snapshots.len(), 1);
    }
    let_row! { (stored, event) = state .record_globally_committed_merge_entry(&entry, MergeLedgerPublicationMode::LiveCommit) .expect("publication retry for the exact State carrier is idempotent") };
    assert_eq!(stored.as_ref(), &entry);
    assert!(event.is_none());
    assert_eq!(state.merge_ledger.snapshot().len(), 1);
}
state_test! { sync stale_staged_merge_fails_before_wsv_when_admission_advances
    let (state, validator_keypairs, commit_keypairs, parent) = configured_single_lane_merge_state();
    let first = next_relay_merge_entry(&state, 1, &validator_keypairs, &commit_keypairs);
    let first_carrier = store_and_commit_exact_merge_carrier(&state, &parent, &first);
    let second = next_relay_merge_entry(&state, 2, &validator_keypairs, &commit_keypairs);
    let second_carrier = certified_merge_carrier_after(&first_carrier, &second);
    state
        .kura
        .store_block_with_merge_entry(Arc::new(second_carrier.clone()), &second)
        .expect("store second exact Kura carrier");
    let_row! { mut stale_block = state .block_with_certified_merge_entry(second_carrier.header().clone(), &second, ConsensusMode::Permissioned) .expect("stage epoch two before the competing admission publication") };
    stale_block.block_hashes.push(second_carrier.hash());
    insert_empty_transaction_block_for_state_commit(&mut stale_block, &second_carrier);
    {
        let mut admission = state.merge_admission.write();
        admission
            .validate_next(&second)
            .expect("competing epoch two admission is initially valid");
        admission.record(&second);
    }
    let committed_height_before = state.committed_height();
    let state_hash_before = state.lane_execution_state_hash();
    let roots_before = state.world.merge_hint_roots.view().clone();
    let global_root_before = *state.world.merge_global_state_root.view();
    let cache_before = state.merge_ledger.snapshot();
    let_row! { error = stale_block .commit() .expect_err("stale staged epoch two must fail admission preflight") };
    assert!(matches!(error, TransactionsBlockError::MergeAdmission));
    assert_eq!(state.committed_height(), committed_height_before);
    assert_eq!(state.lane_execution_state_hash(), state_hash_before);
    assert_eq!(*state.world.merge_hint_roots.view(), roots_before);
    assert_eq!(
        *state.world.merge_global_state_root.view(),
        global_root_before
    );
    assert_eq!(state.merge_ledger.snapshot(), cache_before);
    assert_eq!(state.merge_ledger.latest().as_deref(), Some(&first));
}
state_test! { sync same_block_merge_and_lane_replacement_preserves_history_and_prunes_old_progress
    let replaced_lane = LaneId::new(1);
    let (state, validator_keypairs, commit_keypairs, parent) = configured_two_lane_merge_state();
    let_row! { entry = next_relay_merge_entry_for_lane( &state, 1, replaced_lane, &validator_keypairs, &commit_keypairs, ) };
    let carrier = certified_merge_carrier_after(&parent, &entry);
    state
        .kura
        .store_block_with_merge_entry(Arc::new(carrier.clone()), &entry)
        .expect("store merge+lifecycle carrier");
    let_row! { old_incarnation = state .lane_incarnation(replaced_lane) .expect("replaceable lane has an incarnation") };
    let catalog = state.nexus_snapshot().lane_catalog;
    let_row! { canonical_incarnations = iroha_data_model::nexus::LaneLifecycleParameterV1::canonical_incarnations( &catalog, &state.lane_incarnations_snapshot(), ) .expect("current lane incarnation set is canonical") };
    let_row! { plan = iroha_data_model::nexus::LaneLifecyclePlan { additions: vec![ catalog .lanes() .iter() .find(|lane| lane.id == replaced_lane) .expect("replaceable lane is in the catalog") .clone(), ], retire: vec![replaced_lane], } };
    let_row! { payload = iroha_data_model::nexus::LaneLifecycleParameterV1::new( &catalog, &canonical_incarnations, plan, ) .expect("same-id lifecycle replacement payload is canonical") };
    let_row! { mut state_block = state .block_with_certified_merge_entry(carrier.header().clone(), &entry, ConsensusMode::Permissioned) .expect("stage merge before the same-block lane replacement") };
    {
        let mut transaction = state_block.transaction();
        transaction
            .stage_consensus_lane_lifecycle(&payload)
            .expect("stage same-block lane replacement after merge certification");
        transaction.apply();
    }
    assert!(state_block.pending_autoscale_lifecycle.is_some());
    state_block.block_hashes.push(carrier.hash());
    insert_empty_transaction_block_for_state_commit(&mut state_block, &carrier);
    state_block
        .commit()
        .expect("merge-covered lane progress must not block same-block replacement");
    let_row! { new_incarnation = state .lane_incarnation(replaced_lane) .expect("replacement lane has a fresh incarnation") };
    assert_ne!(new_incarnation, old_incarnation);
    let admission = state.merge_admission.read();
    assert_eq!(admission.latest_entry(), Some(&entry));
    assert!(
        admission
            .binding_history
            .historical_incarnations
            .contains(&old_incarnation),
        "the globally committed old binding must remain in authenticated history"
    );
    assert!(
        admission
            .latest_lane_snapshots
            .keys()
            .all(|(lane_id, _, _)| *lane_id != replaced_lane),
        "replacement must prune old-incarnation relay tips"
    );
    assert!(
        admission
            .latest_execution_heights
            .keys()
            .all(|(lane_id, _, _)| *lane_id != replaced_lane),
        "replacement must prune old-incarnation execution tips"
    );
    let_row! { next_lane_height = admission .latest_lane_snapshots .get(&(replaced_lane, DataSpaceId::UNIVERSAL, new_incarnation)) .map_or(1, |snapshot| snapshot.lane_block_height.saturating_add(1)) };
    assert_eq!(
        next_lane_height, 1,
        "fresh incarnation must restart at lane h1"
    );
}
state_test! { sync empty_and_zero_activation_merge_entries_fail_live_and_recovery_with_same_rule
    let_row! { empty = MergeLedgerEntry { version: MergeLedgerEntry::VERSION, epoch_id: 1, lane_catalog_hash: Hash::new(b"catalog"), active_lanes: Vec::new(), incarnation_root: Hash::new(b"incarnations"), activation_root: Hash::new(b"activations"), lane_snapshots: Vec::new(), execution_batch: None, lane_drain_certificates: Vec::new(), global_state_root: Hash::new(b"root"), merge_qc: dummy_merge_qc(), } };
    let_row! { mut zero_activation = merge_entry_from_candidate(merge_candidate_with_lanes(1, 1), dummy_merge_qc()) };
    zero_activation.active_lanes[0].activation_height = 0;
    for (label, entry, expected_live, expected_recovery) in [
        (
            "empty",
            empty,
            "merge ledger entry must include a lane snapshot, execution batch, or drain certificate",
            "EmptyEntry",
        ),
        (
            "zero activation",
            zero_activation,
            "zero activation height",
            "zero activation height",
        ),
    ] {
        let_row! { live_state = State::new_for_testing( World::default(), Kura::blank_kura_for_testing(), LiveQueryStore::start_test(), ) };
        let_row! { live_error = live_state .validate_certified_merge_entry_for_global_order(&entry, ConsensusMode::Permissioned) .expect_err("malformed live merge entry must fail closed") };
        assert!(
            live_error.to_string().contains(expected_live),
            "{label} live rejection used the wrong rule: {live_error}"
        );
        let kura = persist_merge_entry_with_exact_carrier(entry);
        let_row! { recovery = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| { State::new_for_testing(World::default(), kura, LiveQueryStore::start_test()) })) };
        let_row! { panic = match recovery { Ok(_) => panic!("malformed globally carried history must abort recovery"), Err(payload) => panic_payload_text(payload), } };
        assert!(
            panic.contains(expected_recovery),
            "{label} recovery rejection used the wrong rule: {panic}"
        );
    }
}
state_test! { sync merge_consensus_snapshot_never_mixes_lifecycle_replacement_with_old_admission_tip
    let replaced_lane = LaneId::new(1);
    let (state, _validator_keypairs, _commit_keypairs, _parent) = configured_two_lane_merge_state();
    let state = Arc::new(state);
    let_row! { old_incarnation = state .lane_incarnation(replaced_lane) .expect("replaceable lane has an incarnation") };
    let_row! { old_activation = state.lane_incarnation_activation_heights_snapshot()[&replaced_lane].saturating_add(1) };
    let_row! { settlement = empty_merge_settlement(replaced_lane, old_incarnation, DataSpaceId::UNIVERSAL, 1) };
    let_row! { old_tip = MergeLaneSnapshot { lane_id: replaced_lane, lane_incarnation: old_incarnation, incarnation_activation_height: old_activation, proposal_height: old_activation, dataspace_id: DataSpaceId::UNIVERSAL, lane_block_height: 1, tip_hash: HashOf::from_untyped_unchecked(Hash::new(b"old-tip")), merge_hint_root: Hash::new(b"old-hint"), settlement_hash: canonical_merge_settlement_hash(&settlement) .expect("old tip settlement hashes canonically"), settlement_commitment: settlement, relay_envelope: None, } };
    let old_tip_key = (replaced_lane, DataSpaceId::UNIVERSAL, old_incarnation);
    state
        .merge_admission
        .write()
        .latest_lane_snapshots
        .insert(old_tip_key, old_tip);
    let catalog = state.nexus_snapshot().lane_catalog;
    let_row! { replacement = iroha_data_model::nexus::LaneLifecyclePlan { additions: vec![ catalog .lanes() .iter() .find(|lane| lane.id == replaced_lane) .expect("replaceable lane is in the catalog") .clone(), ], retire: vec![replaced_lane], } };
    let admission_guard = state.merge_admission.write();
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let reader_state = Arc::clone(&state);
    let_row! { reader = std::thread::spawn(move || { started_tx.send(()).expect("signal snapshot reader start"); let snapshot = reader_state.merge_consensus_snapshot(); ( snapshot.lifecycle.incarnations[&replaced_lane], snapshot .admission .latest_lane_snapshots .contains_key(&old_tip_key), ) }) };
    started_rx.recv().expect("snapshot reader started");
    let writer_state = Arc::clone(&state);
    let writer = std::thread::spawn(move || writer_state.apply_lane_lifecycle_shared(&replacement));
    let write_generation_deadline = Instant::now() + Duration::from_secs(5);
    let_row! { observed_write_generation = loop { if state.state_view_generation() % 2 == 1 { break true; } if Instant::now() >= write_generation_deadline { break false; } std::thread::sleep(Duration::from_millis(1)); } };
    drop(admission_guard);
    writer
        .join()
        .expect("lifecycle writer thread must not panic")
        .expect("same-id lifecycle replacement must succeed");
    assert!(
        observed_write_generation,
        "test must overlap the snapshot with an active lifecycle write generation"
    );
    let_row! { (observed_incarnation, observed_old_tip) = reader.join().expect("snapshot reader must not panic") };
    let_row! { new_incarnation = state .lane_incarnation(replaced_lane) .expect("replacement lane has a current incarnation") };
    assert_ne!(new_incarnation, old_incarnation);
    assert_eq!(observed_incarnation, new_incarnation);
    assert!(
        !observed_old_tip,
        "a stable post-replacement consensus snapshot must not retain the old tip"
    );
}
state_test! { sync apply_without_execution_updates_commit_topology_from_world_peers
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), kura, query);
    let keypairs = configure_commit_topology(&state, 4);
    let_row! { base_topology: Vec<_> = keypairs .iter() .map(|kp| PeerId::new(kp.public_key().clone())) .collect() };
    let_row! { new_peer = PeerId::new( crate::state::checked_keypair_with_algorithm(Algorithm::BlsNormal) .public_key() .clone(), ) };
    {
        let mut world_block = state.world.block();
        {
            let mut peers = world_block.peers_mut_for_testing().transaction();
            peers.clear();
            peers.extend(base_topology.clone());
            peers.push(new_peer.clone());
            peers.apply();
        }
        world_block.commit();
    }
    let_row! { block = BlockBuilder::new(vec![dummy_accepted_transaction()]) .chain(0, None) .sign(keypairs[0].private_key()) .unpack(|_| {}) };
    let signed_block: SignedBlock = block.into();
    let mut state_block = state.block(signed_block.header());
    let valid = ValidBlock::validate_unchecked(signed_block, &mut state_block).unpack(|_| {});
    let committed = valid.commit_unchecked().unpack(|_| {});
    let prev_hash = committed.as_ref().hash();
    let _ = state_block.apply_without_execution(&committed, base_topology.clone());
    state_block.commit().expect("commit state block");
    let mut expected_topology = Topology::new(base_topology.clone());
    let mut world_peers = base_topology.clone();
    world_peers.push(new_peer);
    expected_topology.block_committed(world_peers, prev_hash);
    let expected = expected_topology.as_ref().to_vec();
    let view = state.view();
    let actual: Vec<_> = view.commit_topology().iter().cloned().collect();
    assert_eq!(actual, expected);
    let prev: Vec<_> = view.prev_commit_topology().iter().cloned().collect();
    assert_eq!(prev, base_topology);
}
state_test! { sync v2_authority_preserves_topology_transition
    let kura = Kura::blank_kura_for_testing();
    let state = blank_test_state_from_kura(&kura);
    let keypairs = configure_commit_topology(&state, 4);
    let_row! { base_topology = keypairs .iter() .map(|keypair| PeerId::new(keypair.public_key().clone())) .collect::<Vec<_>>() };
    let_row! { block: SignedBlock = BlockBuilder::new(vec![dummy_accepted_transaction()]) .chain(0, None) .sign(keypairs[0].private_key()) .unpack(|_| {}) .into() };
    let mut state_block = state.block(block.header());
    let valid = ValidBlock::validate_unchecked(block, &mut state_block).unpack(|_| {});
    let artifact = merge_carrier_finality_artifact_with_network(
        valid.as_ref(),
        None,
        *state.network_id_ref(),
    );
    let_row! { exact_roster = artifact .height_context .roster .iter() .map(|entry| entry.validator.clone()) .collect::<Vec<_>>() };
    let_row! { verified_artifact = crate::block::VerifiedV2FinalityArtifact::verify(artifact.clone()) .expect("fixture finality verifies once") };
    let committed = valid
        .commit_with_verified_v2_artifact(
            verified_artifact,
            artifact.commit_qc.execution_commitment,
        )
        .unpack(|_| {})
        .expect("fixture block binds exact v2 finality");
    let block_hash = committed.as_ref().hash();
    let_row! { _ = state_block .apply_without_execution_with_verified_v2_finality(&committed) .expect("ordinary v2 carrier metadata remains valid") };
    state_block
        .commit()
        .expect("commit v2-authorized state block");
    let mut expected_topology = Topology::new(base_topology.clone());
    expected_topology.block_committed(exact_roster, block_hash);
    assert_eq!(
        state.commit_topology_snapshot(),
        expected_topology.as_ref().to_vec(),
        "v2 authority must rotate from the exact authenticated roster without world-peer append"
    );
    assert_eq!(state.prev_commit_topology_snapshot(), base_topology);
}
state_test! { sync v2_authority_requires_exact_context_before_post_execution_mutation
    let kura = Kura::blank_kura_for_testing();
    let state = blank_test_state_from_kura(&kura);
    let keypairs = configure_commit_topology(&state, 4);
    let_row! { genesis: SignedBlock = BlockBuilder::new(vec![dummy_accepted_transaction()]) .chain(0, None) .sign(keypairs[0].private_key()) .unpack(|_| {}) .into() };
    let_row! { block: SignedBlock = BlockBuilder::new(vec![dummy_accepted_transaction()]) .chain(0, Some(&genesis)) .sign(keypairs[0].private_key()) .unpack(|_| {}) .into() };
    let genesis_artifact = merge_carrier_finality_artifact_with_network(
        &genesis,
        None,
        *state.network_id_ref(),
    );
    let block_hash = block.hash();
    let mut state_block = state.block(block.header());
    let valid = ValidBlock::validate_unchecked(block, &mut state_block).unpack(|_| {});
    let artifact = merge_carrier_finality_artifact_with_network(
        valid.as_ref(),
        Some(&genesis_artifact),
        *state.network_id_ref(),
    );
    let_row! { verified_artifact = crate::block::VerifiedV2FinalityArtifact::verify(artifact.clone()) .expect("fixture finality verifies once") };
    let committed = valid
        .commit_with_verified_v2_artifact(
            verified_artifact,
            artifact.commit_qc.execution_commitment,
        )
        .unpack(|_| {})
        .expect("fixture block binds exact v2 finality");
    let_row! { error = state_block .apply_without_execution_with_verified_v2_finality(&committed) .expect_err("v2 apply without durable exact context must fail closed") };
    assert!(
        error.to_string().contains("missing exact v2 finality context"),
        "unexpected v2 context failure: {error}"
    );
    assert!(
        state_block
            .block_hashes
            .iter()
            .all(|hash| *hash != block_hash),
        "failed v2 pre-apply validation must not publish the block hash"
    );
    assert!(
        !state_block.transactions.has_staged_block(),
        "failed v2 pre-apply validation must not publish transaction history"
    );
}
state_test! { sync v2_state_apply_rejects_ordinary_commit_capability
    let kura = Kura::blank_kura_for_testing();
    let state = blank_test_state_from_kura(&kura);
    let keypairs = configure_commit_topology(&state, 4);
    let_row! { block: SignedBlock = BlockBuilder::new(vec![dummy_accepted_transaction()]) .chain(0, None) .sign(keypairs[0].private_key()) .unpack(|_| {}) .into() };
    let block_hash = block.hash();
    let mut state_block = state.block(block.header());
    let valid = ValidBlock::validate_unchecked(block, &mut state_block).unpack(|_| {});
    let committed = valid.commit_unchecked().unpack(|_| {});
    let_row! { error = state_block .apply_without_execution_with_verified_v2_finality(&committed) .expect_err("ordinary commit must not authorize production State apply") };
    assert!(
        error
            .to_string()
            .contains("requires a block committed with verified Sumeragi-v2 finality"),
        "unexpected authority failure: {error}"
    );
    assert!(
        state_block
            .block_hashes
            .iter()
            .all(|hash| *hash != block_hash),
        "failed capability validation must precede block-hash publication"
    );
    assert!(
        !state_block.transactions.has_staged_block(),
        "failed capability validation must precede transaction-history publication"
    );
}
state_test! { sync height_mismatch_does_not_publish_staged_commit_topology
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), kura, query);
    let keypairs = configure_commit_topology(&state, 4);
    let_row! { base_topology: Vec<_> = keypairs .iter() .map(|kp| PeerId::new(kp.public_key().clone())) .collect() };
    assert_eq!(state.commit_topology_snapshot(), base_topology);
    assert!(
        state.prev_commit_topology_snapshot().is_empty(),
        "test setup should start with no previous commit topology"
    );
    let_row! { new_peer = PeerId::new( crate::state::checked_keypair_with_algorithm(Algorithm::BlsNormal) .public_key() .clone(), ) };
    {
        let mut world_block = state.world.block();
        {
            let mut peers = world_block.peers_mut_for_testing().transaction();
            peers.clear();
            peers.extend(base_topology.clone());
            peers.push(new_peer);
            peers.apply();
        }
        world_block.commit();
    }
    let_row! { first_block: SignedBlock = BlockBuilder::new(vec![dummy_accepted_transaction()]) .chain(0, None) .sign(keypairs[0].private_key()) .unpack(|_| {}) .into() };
    let_row! { second_block: SignedBlock = BlockBuilder::new(vec![dummy_accepted_transaction()]) .chain(0, Some(&first_block)) .sign(keypairs[0].private_key()) .unpack(|_| {}) .into() };
    let mut state_block = state.block(second_block.header());
    let valid = ValidBlock::validate_unchecked(second_block, &mut state_block).unpack(|_| {});
    let committed = valid.commit_unchecked().unpack(|_| {});
    let _ = state_block.apply_without_execution(&committed, base_topology.clone());
    let_row! { err = state_block .commit() .expect_err("height mismatch must abort staged topology updates") };
    assert!(matches!(
        err,
        TransactionsBlockError::HeightMismatch {
            expected_current_height: 1,
            actual_current_height: 2
        }
    ));
    assert_eq!(
        state.commit_topology_snapshot(),
        base_topology,
        "height mismatch must not publish staged commit topology updates"
    );
    assert!(
        state.prev_commit_topology_snapshot().is_empty(),
        "height mismatch must not publish staged previous-topology updates"
    );
}
state_test! { sync apply_without_execution_keeps_world_peer_append_scoped_to_checkpoint_lanes
    use iroha_config::parameters::actual::LaneValidatorMode;
    use iroha_data_model::nexus::{LaneCatalog, LaneConfig as CatalogLaneConfig, LaneVisibility};
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::default(), kura, query);
    {
        let_row! { lane_catalog = LaneCatalog::new( NonZeroU32::new(2).expect("nonzero lane count"), vec![ CatalogLaneConfig::default(), CatalogLaneConfig { id: LaneId::new(1), alias: "restricted".to_string(), visibility: LaneVisibility::Restricted, ..CatalogLaneConfig::default() }, ], ) .expect("lane catalog") };
        let nexus = state.nexus.get_mut();
        nexus.lane_catalog = lane_catalog.clone();
        nexus.lane_config =
            iroha_config::parameters::actual::LaneConfig::from_catalog(&lane_catalog);
        nexus.staking.public_validator_mode = LaneValidatorMode::StakeElected;
        nexus.staking.restricted_validator_mode = LaneValidatorMode::StakeElected;
        nexus.staking.min_validator_stake = 100_u64.into();
    }
    let_row! { public_keypairs: Vec<_> = (0..2) .map(|_| crate::state::checked_keypair_with_algorithm(Algorithm::BlsNormal)) .collect() };
    let_row! { restricted_keypairs: Vec<_> = (0..2) .map(|_| crate::state::checked_keypair_with_algorithm(Algorithm::BlsNormal)) .collect() };
    let_row! { base_topology: Vec<_> = public_keypairs .iter() .map(|kp| PeerId::new(kp.public_key().clone())) .collect() };
    {
        let mut topo = state.commit_topology.block();
        topo.clear();
        for peer in &base_topology {
            topo.push(peer.clone());
        }
        topo.commit();
    }
    {
        let mut world_block = state.world.block();
        {
            let mut peers = world_block.peers_mut_for_testing().transaction();
            peers.clear();
            peers.extend(base_topology.clone());
            peers.extend(
                restricted_keypairs
                    .iter()
                    .map(|kp| PeerId::new(kp.public_key().clone())),
            );
            peers.apply();
        }
        for kp in &public_keypairs {
            let validator = AccountId::new(kp.public_key().clone());
            world_block.public_lane_validators.insert(
                (LaneId::SINGLE, validator.clone()),
                PublicLaneValidatorRecord {
                    lane_id: LaneId::SINGLE,
                    validator: validator.clone(),
                    peer_id: PeerId::new(kp.public_key().clone()),
                    stake_account: validator,
                    total_stake: iroha_primitives::numeric::Quantity::from(1_000_u32),
                    self_stake: iroha_primitives::numeric::Quantity::from(1_000_u32),
                    metadata: Metadata::default(),
                    status: PublicLaneValidatorStatus::Active,
                    activation_epoch: None,
                    activation_height: None,
                    last_reward_epoch: None,
                },
            );
        }
        for kp in &restricted_keypairs {
            let validator = AccountId::new(kp.public_key().clone());
            world_block.public_lane_validators.insert(
                (LaneId::new(1), validator.clone()),
                PublicLaneValidatorRecord {
                    lane_id: LaneId::new(1),
                    validator: validator.clone(),
                    peer_id: PeerId::new(kp.public_key().clone()),
                    stake_account: validator,
                    total_stake: iroha_primitives::numeric::Quantity::from(1_000_u32),
                    self_stake: iroha_primitives::numeric::Quantity::from(1_000_u32),
                    metadata: Metadata::default(),
                    status: PublicLaneValidatorStatus::Active,
                    activation_epoch: None,
                    activation_height: None,
                    last_reward_epoch: None,
                },
            );
        }
        world_block.commit();
    }
    seed_consensus_keys_with_pops(
        &state,
        &public_keypairs
            .iter()
            .chain(restricted_keypairs.iter())
            .cloned()
            .collect::<Vec<_>>(),
    );
    let_row! { block = BlockBuilder::new(vec![dummy_accepted_transaction()]) .chain(0, None) .sign(public_keypairs[0].private_key()) .unpack(|_| {}) };
    let signed_block: SignedBlock = block.into();
    let mut state_block = state.block(signed_block.header());
    let valid = ValidBlock::validate_unchecked(signed_block, &mut state_block).unpack(|_| {});
    let committed = valid.commit_unchecked().unpack(|_| {});
    let prev_hash = committed.as_ref().hash();
    let _ = state_block.apply_without_execution(&committed, base_topology.clone());
    state_block.commit().expect("commit state block");
    let mut expected_topology = Topology::new(base_topology.clone());
    expected_topology.block_committed(base_topology.clone(), prev_hash);
    let expected = expected_topology.as_ref().to_vec();
    let view = state.view();
    let actual: Vec<_> = view.commit_topology().iter().cloned().collect();
    assert_eq!(actual, expected);
    let prev: Vec<_> = view.prev_commit_topology().iter().cloned().collect();
    assert_eq!(prev, base_topology);
}
state_test! { sync apply_without_execution_keeps_npos_commit_topology_without_world_peer_append
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), kura, query);
    {
        let mut params = state.world.parameters.block();
        params.set_parameter(Parameter::Custom(
            SumeragiNposParameters::default().into_custom_parameter(),
        ));
        params.commit();
    }
    let keypairs = configure_commit_topology(&state, 4);
    let_row! { base_topology: Vec<_> = keypairs .iter() .map(|kp| PeerId::new(kp.public_key().clone())) .collect() };
    let_row! { new_peer = PeerId::new( crate::state::checked_keypair_with_algorithm(Algorithm::BlsNormal) .public_key() .clone(), ) };
    let_row! { block = BlockBuilder::new(vec![dummy_accepted_transaction()]) .chain(0, None) .sign(keypairs[0].private_key()) .unpack(|_| {}) };
    let signed_block: SignedBlock = block.into();
    let mut state_block = state.block(signed_block.header());
    {
        let mut peers = state_block.world.peers_mut_for_testing().transaction();
        peers.clear();
        peers.extend(base_topology.clone());
        peers.push(new_peer);
        peers.apply();
    }
    let valid = ValidBlock::validate_unchecked(signed_block, &mut state_block).unpack(|_| {});
    let committed = valid.commit_unchecked().unpack(|_| {});
    let prev_hash = committed.as_ref().hash();
    let _ = state_block.apply_without_execution(&committed, base_topology.clone());
    state_block.commit().expect("commit state block");
    let mut expected_topology = Topology::new(base_topology.clone());
    expected_topology.block_committed(base_topology.clone(), prev_hash);
    let expected = expected_topology.as_ref().to_vec();
    let view = state.view();
    let actual: Vec<_> = view.commit_topology().iter().cloned().collect();
    assert_eq!(actual, expected);
    let prev: Vec<_> = view.prev_commit_topology().iter().cloned().collect();
    assert_eq!(prev, base_topology);
}
state_test! { sync apply_without_execution_widens_npos_commit_topology_with_active_public_validator
    use iroha_config::parameters::actual::LaneValidatorMode;
    use iroha_data_model::parameter::system::{Parameter, SumeragiNposParameters};
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::default(), kura, query);
    {
        let mut params = state.world.parameters.block();
        params.set_parameter(Parameter::Custom(
            SumeragiNposParameters::default().into_custom_parameter(),
        ));
        params.commit();
    }
    {
        let nexus = state.nexus.get_mut();
        nexus.staking.public_validator_mode = LaneValidatorMode::StakeElected;
        nexus.staking.min_validator_stake = 100_u64.into();
    }
    let keypairs = configure_commit_topology(&state, 3);
    let missing_keypair = crate::state::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let_row! { base_topology: Vec<_> = keypairs .iter() .map(|kp| PeerId::new(kp.public_key().clone())) .collect() };
    let missing_peer = PeerId::new(missing_keypair.public_key().clone());
    {
        let mut world_block = state.world.block();
        {
            let mut peers = world_block.peers_mut_for_testing().transaction();
            peers.clear();
            peers.extend(base_topology.clone());
            peers.push(missing_peer.clone());
            peers.apply();
        }
        for keypair in keypairs.iter().chain(core::iter::once(&missing_keypair)) {
            let validator = AccountId::new(keypair.public_key().clone());
            world_block.public_lane_validators.insert(
                (LaneId::SINGLE, validator.clone()),
                PublicLaneValidatorRecord {
                    lane_id: LaneId::SINGLE,
                    validator: validator.clone(),
                    peer_id: PeerId::new(keypair.public_key().clone()),
                    stake_account: validator,
                    total_stake: iroha_primitives::numeric::Quantity::from(1_000_u32),
                    self_stake: iroha_primitives::numeric::Quantity::from(1_000_u32),
                    metadata: Metadata::default(),
                    status: PublicLaneValidatorStatus::Active,
                    activation_epoch: None,
                    activation_height: None,
                    last_reward_epoch: None,
                },
            );
        }
        world_block.commit();
    }
    seed_consensus_keys_with_pops(
        &state,
        &keypairs
            .iter()
            .chain(core::iter::once(&missing_keypair))
            .cloned()
            .collect::<Vec<_>>(),
    );
    let_row! { block = BlockBuilder::new(vec![dummy_accepted_transaction()]) .chain(0, None) .sign(keypairs[0].private_key()) .unpack(|_| {}) };
    let signed_block: SignedBlock = block.into();
    let mut state_block = state.block(signed_block.header());
    let valid = ValidBlock::validate_unchecked(signed_block, &mut state_block).unpack(|_| {});
    let committed = valid.commit_unchecked().unpack(|_| {});
    let prev_hash = committed.as_ref().hash();
    let _ = state_block.apply_without_execution(&committed, base_topology.clone());
    state_block.commit().expect("commit state block");
    let mut expected_topology = Topology::new(base_topology.clone());
    let mut widened_roster = base_topology.clone();
    widened_roster.push(missing_peer.clone());
    expected_topology.block_committed(widened_roster, prev_hash);
    let expected = expected_topology.as_ref().to_vec();
    let view = state.view();
    let actual: Vec<_> = view.commit_topology().iter().cloned().collect();
    assert_eq!(actual, expected);
    assert!(actual.contains(&missing_peer));
    let prev: Vec<_> = view.prev_commit_topology().iter().cloned().collect();
    assert_eq!(prev, base_topology);
}
state_test! { sync apply_without_execution_uses_npos_parameters_for_commit_topology
    use iroha_data_model::parameter::system::{Parameter, SumeragiNposParameters};
    // Simulate stale status metadata (permissioned tag) while NPoS parameters are present.
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), kura, query);
    {
        let mut params = state.world.parameters.block();
        params.set_parameter(Parameter::Custom(
            SumeragiNposParameters::default().into_custom_parameter(),
        ));
        params.commit();
    }
    let keypairs = configure_commit_topology(&state, 4);
    let_row! { base_topology: Vec<_> = keypairs .iter() .map(|kp| PeerId::new(kp.public_key().clone())) .collect() };
    let_row! { new_peer = PeerId::new( crate::state::checked_keypair_with_algorithm(Algorithm::BlsNormal) .public_key() .clone(), ) };
    let_row! { block = BlockBuilder::new(vec![dummy_accepted_transaction()]) .chain(0, None) .sign(keypairs[0].private_key()) .unpack(|_| {}) };
    let signed_block: SignedBlock = block.into();
    let mut state_block = state.block(signed_block.header());
    {
        let mut peers = state_block.world.peers_mut_for_testing().transaction();
        peers.clear();
        peers.extend(base_topology.clone());
        peers.push(new_peer);
        peers.apply();
    }
    let valid = ValidBlock::validate_unchecked(signed_block, &mut state_block).unpack(|_| {});
    let committed = valid.commit_unchecked().unpack(|_| {});
    let prev_hash = committed.as_ref().hash();
    let _ = state_block.apply_without_execution(&committed, base_topology.clone());
    state_block.commit().expect("commit state block");
    let mut expected_topology = Topology::new(base_topology.clone());
    expected_topology.block_committed(base_topology.clone(), prev_hash);
    let expected = expected_topology.as_ref().to_vec();
    let view = state.view();
    let actual: Vec<_> = view.commit_topology().iter().cloned().collect();
    assert_eq!(actual, expected);
    let prev: Vec<_> = view.prev_commit_topology().iter().cloned().collect();
    assert_eq!(prev, base_topology);
}
state_test! { sync apply_without_execution_derives_commit_topology_when_roster_missing
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), kura, query);
    let keypairs = configure_commit_topology(&state, 4);
    let_row! { base_topology: Vec<_> = keypairs .iter() .map(|kp| PeerId::new(kp.public_key().clone())) .collect() };
    let_row! { new_peer = PeerId::new( crate::state::checked_keypair_with_algorithm(Algorithm::BlsNormal) .public_key() .clone(), ) };
    let_row! { block = BlockBuilder::new(vec![dummy_accepted_transaction()]) .chain(0, None) .sign(keypairs[0].private_key()) .unpack(|_| {}) };
    let signed_block: SignedBlock = block.into();
    let mut state_block = state.block(signed_block.header());
    {
        let mut peers = state_block.world.peers_mut_for_testing().transaction();
        peers.clear();
        peers.extend(base_topology.clone());
        peers.push(new_peer.clone());
        peers.apply();
    }
    let valid = ValidBlock::validate_unchecked(signed_block, &mut state_block).unpack(|_| {});
    let committed = valid.commit_unchecked().unpack(|_| {});
    let block_hash = committed.as_ref().hash();
    let _ = state_block.apply_without_execution(&committed, Vec::new());
    state_block.commit().expect("commit state block");
    let mut expected_topology = Topology::new(base_topology.clone());
    let mut world_peers = base_topology.clone();
    world_peers.push(new_peer);
    world_peers.sort();
    expected_topology.block_committed(world_peers, block_hash);
    let expected = expected_topology.as_ref().to_vec();
    let view = state.view();
    let actual: Vec<_> = view.commit_topology().iter().cloned().collect();
    assert_eq!(actual, expected);
    let prev: Vec<_> = view.prev_commit_topology().iter().cloned().collect();
    assert_eq!(prev, base_topology);
}
state_test! { sync apply_without_execution_prefers_checkpoint_topology_when_world_peers_incomplete
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), kura, query);
    let keypairs = configure_commit_topology(&state, 4);
    let_row! { base_topology: Vec<_> = keypairs .iter() .map(|kp| PeerId::new(kp.public_key().clone())) .collect() };
    let_row! { block = BlockBuilder::new(vec![dummy_accepted_transaction()]) .chain(0, None) .sign(keypairs[0].private_key()) .unpack(|_| {}) };
    let signed_block: SignedBlock = block.into();
    let mut state_block = state.block(signed_block.header());
    {
        let mut peers = state_block.world.peers_mut_for_testing().transaction();
        peers.clear();
        peers.push(base_topology[0].clone());
        peers.apply();
    }
    let valid = ValidBlock::validate_unchecked(signed_block, &mut state_block).unpack(|_| {});
    let committed = valid.commit_unchecked().unpack(|_| {});
    let block_hash = committed.as_ref().hash();
    let _ = state_block.apply_without_execution(&committed, base_topology.clone());
    state_block.commit().expect("commit state block");
    let mut expected_topology = Topology::new(base_topology.clone());
    expected_topology.block_committed(base_topology.clone(), block_hash);
    let expected = expected_topology.as_ref().to_vec();
    let view = state.view();
    let actual: Vec<_> = view.commit_topology().iter().cloned().collect();
    assert_eq!(actual, expected);
    let prev: Vec<_> = view.prev_commit_topology().iter().cloned().collect();
    assert_eq!(prev, base_topology);
}
fn merge_signers_bitmap(
    signers: &BTreeSet<iroha_data_model::block::consensus::ValidatorIndex>,
    roster_len: usize,
) -> Vec<u8> {
    if roster_len == 0 {
        return Vec::new();
    }
    let mut bitmap = vec![0u8; roster_len.div_ceil(8)];
    for signer in signers {
        let_row! { Ok(idx) = usize::try_from(*signer) else { continue; } };
        if idx >= roster_len {
            continue;
        }
        let byte = idx / 8;
        let bit = idx % 8;
        bitmap[byte] |= 1u8 << bit;
    }
    bitmap
}
fn merge_qc_for_candidate(
    state: &State,
    candidate: &crate::merge::MergeLedgerCandidate,
    keypairs: &[KeyPair],
    signers: &[usize],
) -> MergeQuorumCertificate {
    if state.latest_block_hash_fast().is_none() {
        let mut block_hashes = state.block_hashes.block();
        block_hashes.push_for_tests(candidate.carrier_parent_hash);
        block_hashes.commit_for_tests();
    }
    let_row! { validator_set = keypairs .iter() .map(|keypair| PeerId::new(keypair.public_key().clone())) .collect::<Vec<_>>() };
    let validator_set_hash = HashOf::new(&validator_set);
    let_row! { message_digest = crate::merge::merge_qc_message_digest( state.network_id_ref(), candidate, VALIDATOR_SET_HASH_VERSION_V1, validator_set_hash, ) };
    let mut signers_set = BTreeSet::new();
    let mut signature_payloads = Vec::with_capacity(signers.len());
    let mut signer_proofs = Vec::with_capacity(signers.len());
    for idx in signers {
        let idx_u32 = u32::try_from(*idx).expect("signer index fits in u32");
        signers_set.insert(idx_u32);
        let_row! { signature = Signature::try_new(keypairs[*idx].private_key(), message_digest.as_ref()) .expect("test fixture signing should succeed") };
        signature_payloads.push(signature.payload().to_vec());
        signer_proofs.push(iroha_data_model::merge::MergeSignerProof {
            signer: idx_u32,
            proof_of_possession: iroha_crypto::bls_normal_pop_prove(keypairs[*idx].private_key())
                .expect("test signer PoP"),
        });
    }
    let signature_refs: Vec<&[u8]> = signature_payloads.iter().map(Vec::as_slice).collect();
    let_row! { aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(&signature_refs) .expect("aggregate merge signatures") };
    let signers_bitmap = merge_signers_bitmap(&signers_set, keypairs.len());
    MergeQuorumCertificate::new(
        candidate.view,
        candidate.epoch_id,
        candidate.carrier_height,
        candidate.carrier_parent_hash,
        *state.network_id_ref(),
        VALIDATOR_SET_HASH_VERSION_V1,
        validator_set_hash,
        validator_set,
        signers_bitmap,
        signer_proofs,
        aggregate_signature,
        message_digest,
    )
}
fn setup_nexus_fee_merge_state(
    sponsor_balance: Quantity,
    fee_amount: Quantity,
    source_id: [u8; 32],
) -> (State, AccountId, AssetDefinitionId, Vec<KeyPair>) {
    let (sponsor_id, _sponsor_kp) = gen_account_in("wonderland");
    let (custody_id, _custody_kp) = gen_account_in("wonderland");
    let_row! { program_id = FeeSponsorProgramId::new( sponsor_id.clone(), "merge-receipts".parse().expect("program name"), ) };
    let lease_id = Hash::new(b"merge-fee-sponsor-spend-lease");
    let domain_id = DomainId::try_new("wonderland", "universal").expect("domain id");
    let domain = Domain::new(domain_id.clone()).build(&sponsor_id);
    let sponsor = Account::new(sponsor_id.clone()).build(&sponsor_id);
    let custody = Account::new(custody_id.clone()).build(&sponsor_id);
    let fee_asset_selector = iroha_config::parameters::defaults::nexus::fees::fee_asset_id();
    let_row! { asset_def_id = AssetDefinitionId::parse_address_literal(&fee_asset_selector) .expect("default Nexus XOR fee asset id must be canonical") };
    let_row! { mut asset_definition = AssetDefinition::numeric( asset_def_id.clone(), "xor", iroha_data_model::asset::AssetBalancePolicy::Global, None, ) .build(&sponsor_id) };
    asset_definition.total_quantity = sponsor_balance.clone();
    let_row! { custody_asset = Asset::new( AssetId::of(asset_def_id.clone(), custody_id.clone()), sponsor_balance.clone(), ) };
    let_row! { mut world = World::with_assets( [domain], [sponsor, custody], [asset_definition], [custody_asset], [], ) };
    let_row! { fee_asset_alias: AssetDefinitionAlias = "xor#universal".parse().expect("canonical fee asset alias") };
    world.asset_definition_aliases =
        std::iter::once((fee_asset_alias.clone(), asset_def_id.clone())).collect();
    world.asset_definition_alias_bindings = std::iter::once((
        asset_def_id.clone(),
        AssetDefinitionAliasBindingRecord {
            alias: fee_asset_alias,
            lease_expiry_ms: None,
            grace_until_ms: None,
            bound_at_ms: 0,
        },
    ))
    .collect();
    let_row! { allocation = VerifiedFeeSponsorVaultAllocation::new( program_id.clone(), 1, asset_def_id.clone(), Quantity::from(1_000_000_u32), DataSpaceId::UNIVERSAL, 1, Hash::new(b"merge-fee-sponsor-source-state"), 100, lease_id, Hash::new(b"merge-fee-sponsor-proof"), *Hash::new(b"merge-fee-sponsor-statement").as_ref(), Hash::new(b"merge-fee-sponsor-proof-digest"), 1, *Hash::new(b"merge-fee-sponsor-manifest").as_ref(), AxtFastpqBinding { parameter: "fastpq-lane-balanced".to_owned(), source_dsid: DataSpaceId::UNIVERSAL.as_u64(), source_dataspace: "universal".to_owned(), source_receipt_id: "merge-fee-sponsor-receipt".to_owned(), source_tx_commitment: "aa".repeat(32), claim_type: "fee_sponsor_vault_allocation".to_owned(), claim_digest: "bb".repeat(32), witness_commitment: "cc".repeat(32), policy_commitment: "dd".repeat(32), verified_effect_type: "fee_sponsor_vault_allocation".to_owned(), corridor: "fee-sponsor".to_owned(), verifier_id: "fastpq".to_owned(), verifier_version: "v1".to_owned(), target_dsids: vec![DataSpaceId::UNIVERSAL.as_u64()], effect_binding: None, remote_spend_intent_commitments: Vec::new(), }, ) };
    let_row! { allocation_key: StatePath = VerifiedFeeSponsorVaultAllocation::state_key_for(&program_id, &asset_def_id, &lease_id) .parse() .expect("verified fee allocation state key") };
    world.smart_contract_state_mut_for_testing().insert(
        allocation_key,
        norito::to_bytes(&Json::try_new(allocation).expect("verified fee allocation JSON"))
            .expect("verified fee allocation state"),
    );
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(world, kura, query);
    assert_eq!(
        state
            .view()
            .world()
            .asset_definition(&asset_def_id)
            .expect("fee asset definition exists")
            .total_quantity(),
        &sponsor_balance,
        "fee fixture supply must equal its seeded sponsor balance"
    );
    let_row! { nexus = iroha_config::parameters::actual::Nexus { fees: iroha_config::parameters::actual::NexusFees { fee_asset_id: "xor#universal".to_owned(), settlement_mode: iroha_config::parameters::actual::NexusFeeSettlementMode::LaneRelayBurn, sponsor_vault_custody_account_id: custody_id.clone(), ..iroha_config::parameters::actual::NexusFees::default() }, ..iroha_config::parameters::actual::Nexus::default() } };
    state.set_nexus(nexus).expect("apply Nexus config");
    let (validator_ids, validator_keypairs) = bls_accounts_in("validators", 4);
    seed_consensus_keys_with_pops(&state, &validator_keypairs);
    install_lane_manifest_registry(
        &state,
        &[(LaneId::new(0), DataSpaceId::UNIVERSAL, validator_ids)],
    );
    let commit_keypairs = configure_commit_topology_preserving_world_peers(&state, 1);
    let_row! { mut envelope = sample_lane_relay_envelope_for_state(&state, 1, LaneId::new(0), &validator_keypairs) };
    let mut settlement = envelope.settlement_commitment.clone();
    // The fee receipt belongs to the same transaction as the base
    // settlement receipt. Bind both receipt categories to the same source
    // so `tx_count` continues to describe one distinct transaction.
    settlement.receipts[0].source_id = source_id;
    settlement.nexus_fee_receipts = vec![NexusFeeReceipt {
        version: NexusFeeReceipt::VERSION,
        source_id,
        dataspace_id: envelope.dataspace_id,
        lane_id: envelope.lane_id,
        block_height: envelope.block_height,
        debit_source: FeeDebitSource::SponsorProgram(program_id),
        fee_asset_id: asset_def_id.clone(),
        program_revision: Some(1),
        lease_id: Some(lease_id),
        fee_amount: fee_amount.clone(),
        schedule: NexusFeeScheduleInputs {
            tx_bytes_len: 0,
            instruction_count: 0,
            gas_used: 0,
            base_fee: fee_amount,
            per_byte_fee: Quantity::zero(),
            per_instruction_fee: Quantity::zero(),
            per_gas_unit_fee: Quantity::zero(),
        },
    }];
    envelope.settlement_commitment = settlement;
    envelope.settlement_hash =
        iroha_data_model::nexus::compute_settlement_hash(&envelope.settlement_commitment)
            .expect("fee relay settlement hash");
    envelope
        .verify()
        .expect("fee relay envelope remains structurally valid");
    resign_lane_relay_for_state_test(&state, &mut envelope, &validator_keypairs);
    // The governed proof is verified at proposal height 1, so publish the
    // corresponding committed carrier before the record is admitted.
    ensure_merge_carrier_parent_for_test(&state);
    seed_verified_lane_relay_record(&state, &envelope);
    state.record_lane_relay(&envelope).expect("relay accepted");
    if state.latest_block_hash_fast().is_none() {
        let mut block_hashes = state.block_hashes.block();
        block_hashes.push_for_tests(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"test-merge-parent",
        )));
        block_hashes.commit_for_tests();
    }
    (state, custody_id, asset_def_id, commit_keypairs)
}
fn account_asset_balance(
    state: &State,
    asset_def_id: &AssetDefinitionId,
    account_id: &AccountId,
) -> Quantity {
    state
        .view()
        .world()
        .assets()
        .get(&AssetId::of(asset_def_id.clone(), account_id.clone()))
        .expect("account asset exists")
        .0
        .clone()
}
state_test! { sync sponsored_fee_receipt_validates_lease_at_authenticated_authority_height
    let (sponsor, _) = gen_account_in("fee-sponsor-authority-height");
    let program_id = FeeSponsorProgramId::new(sponsor, "lane-relay".parse().expect("program name"));
    let_row! { asset_definition_id = AssetDefinitionId::parse_address_literal( &iroha_config::parameters::defaults::nexus::fees::fee_asset_id(), ) .expect("default Nexus fee asset is canonical") };
    let dataspace_id = DataSpaceId::new(7);
    let lease_id = Hash::new(b"fee-sponsor-authority-height-lease");
    let_row! { binding = AxtFastpqBinding { parameter: "fastpq-lane-balanced".to_owned(), source_dsid: dataspace_id.as_u64(), source_dataspace: "authority-height-test".to_owned(), source_receipt_id: "authority-height-receipt".to_owned(), source_tx_commitment: "aa".repeat(32), claim_type: "fee_sponsor_vault_allocation".to_owned(), claim_digest: "bb".repeat(32), witness_commitment: "cc".repeat(32), policy_commitment: "dd".repeat(32), verified_effect_type: "fee_sponsor_vault_allocation".to_owned(), corridor: "fee-sponsor".to_owned(), verifier_id: "fastpq".to_owned(), verifier_version: "v1".to_owned(), target_dsids: vec![DataSpaceId::UNIVERSAL.as_u64()], effect_binding: None, remote_spend_intent_commitments: Vec::new(), } };
    let_row! { record = VerifiedFeeSponsorVaultAllocation::new( program_id.clone(), 3, asset_definition_id.clone(), Quantity::from(10_u32), dataspace_id, 40, Hash::new(b"fee-sponsor-authority-height-state"), 50, lease_id, Hash::new(b"fee-sponsor-authority-height-proof"), *Hash::new(b"fee-sponsor-authority-height-statement").as_ref(), Hash::new(b"fee-sponsor-authority-height-proof-digest"), 41, *Hash::new(b"fee-sponsor-authority-height-manifest").as_ref(), binding, ) };
    let_row! { key: StatePath = VerifiedFeeSponsorVaultAllocation::state_key_for( &program_id, &asset_definition_id, &lease_id, ) .parse() .expect("verified allocation state key") };
    let_row! { payload = norito::to_bytes(&Json::try_new(record.clone()).expect("verified allocation JSON")) .expect("verified allocation state") };
    let mut world = World::default();
    world
        .smart_contract_state_mut_for_testing()
        .insert(key, payload);
    let world = world.block();
    let_row! { receipt = NexusFeeReceipt { version: NexusFeeReceipt::VERSION, source_id: [0xA5; 32], dataspace_id, lane_id: LaneId::new(2), block_height: 3, debit_source: FeeDebitSource::SponsorProgram(program_id), fee_asset_id: asset_definition_id, program_revision: Some(3), lease_id: Some(lease_id), fee_amount: Quantity::from(2_u32), schedule: NexusFeeScheduleInputs { tx_bytes_len: 0, instruction_count: 0, gas_used: 0, base_fee: Quantity::from(2_u32), per_byte_fee: Quantity::zero(), per_instruction_fee: Quantity::zero(), per_gas_unit_fee: Quantity::zero(), }, } };
    assert_eq!(
        State::verified_fee_sponsor_allocation_for_receipt(&world, &receipt, 45)
            .expect("authority height is within the lease")
            .expect("sponsored receipt resolves its lease"),
        record
    );
    assert!(
        State::verified_fee_sponsor_allocation_for_receipt(&world, &receipt, 39).is_err(),
        "allocation cannot be consumed before its source state exists"
    );
    assert!(
        State::verified_fee_sponsor_allocation_for_receipt(&world, &receipt, 51).is_err(),
        "allocation cannot be consumed after its global-height expiry"
    );
}
state_test! { sync lane_relay_fee_receipt_rejects_unauthenticated_authority_debit
    let (authority, _) = gen_account_in("authority-receipt");
    let_row! { asset_definition_id = AssetDefinitionId::parse_address_literal( &iroha_config::parameters::defaults::nexus::fees::fee_asset_id(), ) .expect("default Nexus fee asset is canonical") };
    let_row! { receipt = NexusFeeReceipt { version: NexusFeeReceipt::VERSION, source_id: [0xA6; 32], dataspace_id: DataSpaceId::UNIVERSAL, lane_id: LaneId::SINGLE, block_height: 1, debit_source: FeeDebitSource::Account(authority), fee_asset_id: asset_definition_id.clone(), program_revision: None, lease_id: None, fee_amount: Quantity::from(1_u32), schedule: NexusFeeScheduleInputs { tx_bytes_len: 0, instruction_count: 0, gas_used: 0, base_fee: Quantity::from(1_u32), per_byte_fee: Quantity::zero(), per_instruction_fee: Quantity::zero(), per_gas_unit_fee: Quantity::zero(), }, } };
    let_row! { error = State::validate_nexus_fee_receipt( &receipt, LaneId::SINGLE, DataSpaceId::UNIVERSAL, 1, &asset_definition_id, ) .expect_err("receipt settlement must require an authenticated sponsor spend lease") };
    assert!(matches!(
        error,
        MergeLedgerCommitError::InvalidNexusFeeReceipt(reason)
            if reason.contains("authority spend lease")
    ));
}
state_test! { sync commit_merge_entry_burns_nexus_fee_receipts_once
    let_row! { (state, sponsor_id, asset_def_id, commit_keypairs) = setup_nexus_fee_merge_state(Quantity::from(10_u32), Quantity::from(3_u32), [0x42; 32]) };
    let_row! { candidate = state .merge_entry_candidates_from_lane_relays() .into_iter() .next() .expect("merge candidate") };
    let qc = merge_qc_for_candidate(&state, &candidate, &commit_keypairs, &[0]);
    let entry = merge_entry_from_candidate(candidate.clone(), qc);
    state
        .commit_merge_entry(entry.clone())
        .expect("merge settlement burns fee");
    assert_eq!(
        account_asset_balance(&state, &asset_def_id, &sponsor_id),
        Quantity::from(7_u32)
    );
    let_row! { err = state .commit_merge_entry(entry) .expect_err("replayed merge entry must be rejected") };
    assert!(matches!(
        err,
        MergeLedgerCommitError::NonMonotonicEpoch { .. }
            | MergeLedgerCommitError::DuplicateNexusFeeReceipt(_)
    ));
    assert_eq!(
        account_asset_balance(&state, &asset_def_id, &sponsor_id),
        Quantity::from(7_u32)
    );
    state.settled_nexus_fee_receipts.write().clear();
    let_row! { duplicate_candidate = crate::merge::MergeLedgerCandidate { epoch_id: 2, ..candidate.clone() } };
    let duplicate_qc = merge_qc_for_candidate(&state, &duplicate_candidate, &commit_keypairs, &[0]);
    let duplicate_entry = merge_entry_from_candidate(duplicate_candidate, duplicate_qc);
    let_row! { err = state .commit_merge_entry(duplicate_entry) .expect_err("higher-epoch replay must be rejected before a second burn") };
    assert!(matches!(
        err,
        MergeLedgerCommitError::NonMonotonicLaneSnapshot { .. }
            | MergeLedgerCommitError::DuplicateNexusFeeReceipt(_)
    ));
    assert_eq!(
        account_asset_balance(&state, &asset_def_id, &sponsor_id),
        Quantity::from(7_u32)
    );
}
state_test! { sync commit_merge_entry_rejects_tampered_canonical_settlement_hash
    let_row! { (state, sponsor_id, asset_def_id, commit_keypairs) = setup_nexus_fee_merge_state(Quantity::from(10_u32), Quantity::from(3_u32), [0x46; 32]) };
    let_row! { mut candidate = state .merge_entry_candidates_from_lane_relays() .into_iter() .next() .expect("merge candidate") };
    candidate
        .lane_snapshots
        .first_mut()
        .expect("candidate lane snapshot")
        .settlement_hash = HashOf::from_untyped_unchecked(Hash::new(b"tampered-settlement-hash"));
    let qc = merge_qc_for_candidate(&state, &candidate, &commit_keypairs, &[0]);
    let_row! { err = state .commit_merge_entry(merge_entry_from_candidate(candidate, qc)) .expect_err("tampered canonical settlement hash must be rejected") };
    assert!(matches!(
        err,
        MergeLedgerCommitError::SettlementCommitmentMismatch { .. }
    ));
    assert_eq!(
        account_asset_balance(&state, &asset_def_id, &sponsor_id),
        Quantity::from(10_u32),
        "rejected settlement must not burn the sponsor balance"
    );
    assert!(state.kura.merge_ledger_snapshot().is_empty());
    assert!(state.settled_nexus_fee_receipts.read().is_empty());
}
state_test! { sync commit_merge_entry_rejects_commitment_changed_after_hashing
    let_row! { (state, sponsor_id, asset_def_id, commit_keypairs) = setup_nexus_fee_merge_state(Quantity::from(10_u32), Quantity::from(3_u32), [0x47; 32]) };
    let_row! { mut candidate = state .merge_entry_candidates_from_lane_relays() .into_iter() .next() .expect("merge candidate") };
    let_row! { settlement = &mut candidate .lane_snapshots .first_mut() .expect("candidate lane snapshot") .settlement_commitment };
    settlement.total_local_amount = settlement
        .total_local_amount
        .try_add(&Quantity::from(1_u32))
        .expect("small test mutation must fit Quantity");
    let qc = merge_qc_for_candidate(&state, &candidate, &commit_keypairs, &[0]);
    let_row! { err = state .commit_merge_entry(merge_entry_from_candidate(candidate, qc)) .expect_err("commitment mutation after hashing must be rejected") };
    assert!(matches!(
        err,
        MergeLedgerCommitError::SettlementCommitmentMismatch { .. }
    ));
    assert_eq!(
        account_asset_balance(&state, &asset_def_id, &sponsor_id),
        Quantity::from(10_u32),
        "rejected settlement must not burn the sponsor balance"
    );
    assert!(state.kura.merge_ledger_snapshot().is_empty());
    assert!(state.settled_nexus_fee_receipts.read().is_empty());
}
state_test! { sync commit_merge_entry_rejects_insufficient_nexus_fee_balance_without_partial_burn
    let_row! { (state, sponsor_id, asset_def_id, commit_keypairs) = setup_nexus_fee_merge_state(Quantity::from(1_u32), Quantity::from(3_u32), [0x43; 32]) };
    let_row! { candidate = state .merge_entry_candidates_from_lane_relays() .into_iter() .next() .expect("merge candidate") };
    let qc = merge_qc_for_candidate(&state, &candidate, &commit_keypairs, &[0]);
    let entry = merge_entry_from_candidate(candidate, qc);
    let_row! { err = state .commit_merge_entry(entry) .expect_err("insufficient fee balance rejects settlement") };
    assert!(matches!(
        err,
        MergeLedgerCommitError::InsufficientNexusFeeBalance { .. }
    ));
    assert_eq!(
        account_asset_balance(&state, &asset_def_id, &sponsor_id),
        Quantity::from(1_u32)
    );
}
state_test! { sync merge_relay_candidate_signing_rejects_insufficient_nexus_fee_balance
    let_row! { (state, _sponsor_id, _asset_def_id, _commit_keypairs) = setup_nexus_fee_merge_state(Quantity::from(1_u32), Quantity::from(3_u32), [0x53; 32]) };
    let_row! { candidate = state .merge_entry_candidates_from_lane_relays() .into_iter() .next() .expect("merge candidate") };
    let_row! { parent = state .latest_block_header_fast() .expect("merge candidate requires a committed parent") };
    let_row! { err = state .validate_merge_relay_candidate_for_round(&candidate, &parent, candidate.view, ConsensusMode::Permissioned) .expect_err("honest validators must reject an unpayable candidate before signing") };
    assert!(matches!(
        err,
        MergeLedgerCommitError::InsufficientNexusFeeBalance { .. }
    ));
}
state_test! { sync merge_append_failure_does_not_mutate_fee_state_or_replay_markers
    let_row! { (state, sponsor_id, asset_def_id, commit_keypairs) = setup_nexus_fee_merge_state(Quantity::from(10_u32), Quantity::from(3_u32), [0x44; 32]) };
    let_row! { candidate = state .merge_entry_candidates_from_lane_relays() .into_iter() .next() .expect("merge candidate") };
    let qc = merge_qc_for_candidate(&state, &candidate, &commit_keypairs, &[0]);
    let entry = merge_entry_from_candidate(candidate, qc);
    state.kura.fail_next_merge_append_for_test();
    state
        .commit_merge_entry(entry)
        .expect_err("durable append failure must abort before settlement");
    assert_eq!(
        account_asset_balance(&state, &asset_def_id, &sponsor_id),
        Quantity::from(10_u32)
    );
    assert!(state.kura.merge_ledger_snapshot().is_empty());
    assert!(state.settled_nexus_fee_receipts.read().is_empty());
}
state_test! { sync staged_fee_merge_kura_failure_publishes_no_burn_or_receipt_cache
    let source_id = [0x47; 32];
    let_row! { (state, sponsor_id, asset_def_id, commit_keypairs) = setup_nexus_fee_merge_state(Quantity::from(10_u32), Quantity::from(3_u32), source_id) };
    let_row! { candidate = state .merge_entry_candidates_from_lane_relays() .into_iter() .next() .expect("fee merge candidate") };
    let qc = merge_qc_for_candidate(&state, &candidate, &commit_keypairs, &[0]);
    let entry = merge_entry_from_candidate(candidate, qc);
    let_row! { parent = state .kura .get_block(NonZeroUsize::new(1).expect("non-zero parent height")) .expect("fee merge carrier parent") };
    let carrier = certified_merge_carrier_after(&parent, &entry);
    let entry_hash = entry.canonical_hash();
    let_row! { state_block = state .block_with_certified_merge_entry(carrier.header().clone(), &entry, ConsensusMode::Permissioned) .expect("stage fee merge before Kura publication") };
    state.kura.fail_next_merge_append_for_test();
    state
        .kura
        .store_block_with_merge_entry(Arc::new(carrier), &entry)
        .expect_err("Kura publication failure must abort before State publication");
    drop(state_block);
    let_row! { receipt_marker = State::nexus_fee_receipt_marker_key(&source_id).expect("fee receipt marker key") };
    assert_eq!(state.committed_height(), 1);
    assert_eq!(
        account_asset_balance(&state, &asset_def_id, &sponsor_id),
        Quantity::from(10_u32)
    );
    assert!(
        matches!(
            state.kura.exact_durable_blocks_count(),
            Err(crate::kura::Error::CanonicalStoragePoisoned)
        ),
        "a post-commit association failure must poison-gate exact live Kura reads"
    );
    assert_eq!(
        state
            .kura
            .get_durable_block_hash(NonZeroUsize::new(2).expect("carrier height")),
        None,
        "poisoned Kura must not expose the committed carrier before restart recovery"
    );
    assert!(state.kura.merge_ledger_snapshot().is_empty());
    assert_eq!(
        state
            .kura
            .merge_carrier_for_entry(entry_hash)
            .expect("read sparse carrier index"),
        None,
        "the failed append must not expose a partial merge association"
    );
    assert!(
        matches!(
            state.kura.merge_entry_by_hash(entry_hash),
            Err(crate::kura::Error::CanonicalStoragePoisoned)
        ),
        "the exact pending retry must remain inaccessible until restart recovery"
    );
    assert!(state.settled_nexus_fee_receipts.read().is_empty());
    assert!(
        state
            .world
            .view()
            .smart_contract_state()
            .get(&receipt_marker)
            .is_none(),
        "Kura failure must publish no durable receipt marker"
    );
    assert!(state.merge_ledger().is_empty());
}
#[test]
fn staged_fee_merge_missing_transaction_membership_publishes_no_burn_or_receipt_cache() {
    let source_id = [0x46; 32];
    let_row! { (state, sponsor_id, asset_def_id, commit_keypairs) = setup_nexus_fee_merge_state(Quantity::from(10_u32), Quantity::from(3_u32), source_id) };
    let_row! { candidate = state .merge_entry_candidates_from_lane_relays() .into_iter() .next() .expect("fee merge candidate") };
    let qc = merge_qc_for_candidate(&state, &candidate, &commit_keypairs, &[0]);
    let entry = merge_entry_from_candidate(candidate, qc);
    let_row! { parent = state .kura .get_block(NonZeroUsize::new(1).expect("non-zero parent height")) .expect("fee merge carrier parent") };
    let carrier = certified_merge_carrier_after(&parent, &entry);
    state
        .kura
        .store_block_with_merge_entry(Arc::new(carrier.clone()), &entry)
        .expect("persist exact fee merge carrier");
    let_row! { receipt_marker = State::nexus_fee_receipt_marker_key(&source_id).expect("fee receipt marker key") };
    let_row! { mut state_block = state .block_with_certified_merge_entry(carrier.header().clone(), &entry, ConsensusMode::Permissioned) .expect("stage exact fee merge carrier") };
    state_block.block_hashes.push(carrier.hash());
    let_row! { error = state_block .commit() .expect_err("missing transaction membership must abort fee merge publication") };
    assert!(matches!(error, TransactionsBlockError::MissingInsertBlock));
    assert_eq!(state.committed_height(), 1);
    assert_eq!(
        account_asset_balance(&state, &asset_def_id, &sponsor_id),
        Quantity::from(10_u32)
    );
    assert!(state.settled_nexus_fee_receipts.read().is_empty());
    assert!(
        state
            .world
            .view()
            .smart_contract_state()
            .get(&receipt_marker)
            .is_none(),
        "aborted fee merge must publish no durable receipt marker"
    );
    assert!(state.merge_ledger().is_empty());
}
state_test! { sync restart_rejects_orphan_merge_sidecar_without_burning_or_truncating
    let source_id = [0x45; 32];
    let_row! { (state, sponsor_id, asset_def_id, commit_keypairs) = setup_nexus_fee_merge_state(Quantity::from(10_u32), Quantity::from(3_u32), source_id) };
    let_row! { candidate = state .merge_entry_candidates_from_lane_relays() .into_iter() .next() .expect("merge candidate") };
    let qc = merge_qc_for_candidate(&state, &candidate, &commit_keypairs, &[0]);
    let entry = merge_entry_from_candidate(candidate, qc);
    // A sidecar without its exact globally committed carrier is not
    // authenticated history and must never be replayed or deleted.
    state
        .kura
        .append_merge_entry(&entry)
        .expect("persist merge entry without settlement");
    let_row! { recovery = state .recover_merge_ledger_from_kura() .expect_err("orphan sidecar recovery must fail closed") };
    assert!(
        matches!(
            &recovery,
            MergeLedgerCommitError::ExecutionStatePublication(message)
                if message.contains("has no exact global carrier")
        ),
        "orphan sidecar recovery returned an unexpected error: {recovery}"
    );
    assert_eq!(
        account_asset_balance(&state, &asset_def_id, &sponsor_id),
        Quantity::from(10_u32)
    );
    assert!(state.settled_nexus_fee_receipts.read().is_empty());
    assert_eq!(
        state
            .kura
            .merge_ledger_all_entries()
            .expect("orphan sidecar remains inspectable"),
        vec![entry]
    );
}
state_test! { sync exact_merge_carrier_replay_burns_settlement_once
    let source_id = [0x45; 32];
    let_row! { (state, sponsor_id, asset_def_id, commit_keypairs) = setup_nexus_fee_merge_state(Quantity::from(10_u32), Quantity::from(3_u32), source_id) };
    let_row! { candidate = state .merge_entry_candidates_from_lane_relays() .into_iter() .next() .expect("merge candidate") };
    let qc = merge_qc_for_candidate(&state, &candidate, &commit_keypairs, &[0]);
    let entry = merge_entry_from_candidate(candidate, qc);
    let carrier = store_merge_carrier_without_state_publication_for_test(&state, &entry);
    state
        .recover_merge_ledger_from_kura()
        .expect("authenticate future exact merge carrier during recovery");
    assert!(state.merge_ledger().snapshot().is_empty());
    assert_eq!(
        account_asset_balance(&state, &asset_def_id, &sponsor_id),
        Quantity::from(10_u32)
    );
    let reference = iroha_data_model::block::CertifiedMergeLedgerReference::new(&entry);
    let_row! { mut state_block = state .block_with_certified_merge_reference(carrier.as_ref().header().clone(), &reference, ConsensusMode::Permissioned) .expect("exact durable carrier reference stages settlement") };
    let _ = state_block.apply_without_execution(&carrier, Vec::new());
    state_block
        .commit()
        .expect("commit exact carrier WSV effects");
    state
        .record_globally_committed_merge_entry(&entry, MergeLedgerPublicationMode::LiveCommit)
        .expect("publish exact carrier cache");
    state
        .replay_persisted_merge_settlements()
        .expect("already-applied exact carrier settlement is recognized");
    assert_eq!(
        account_asset_balance(&state, &asset_def_id, &sponsor_id),
        Quantity::from(7_u32)
    );
    assert!(state.settled_nexus_fee_receipts.read().contains(&source_id));
    state
        .replay_persisted_merge_settlements()
        .expect("marker-backed replay is idempotent");
    assert_eq!(
        account_asset_balance(&state, &asset_def_id, &sponsor_id),
        Quantity::from(7_u32)
    );
}
state_test! { sync restart_replays_durable_merge_settlement_exactly_once
    let source_id = [0x45; 32];
    let_row! { (state, sponsor_id, asset_def_id, commit_keypairs) = setup_nexus_fee_merge_state(Quantity::from(10_u32), Quantity::from(3_u32), source_id) };
    let_row! { candidate = state .merge_entry_candidates_from_lane_relays() .into_iter() .next() .expect("merge candidate") };
    let qc = merge_qc_for_candidate(&state, &candidate, &commit_keypairs, &[0]);
    let entry = merge_entry_from_candidate(candidate, qc);
    // Model a process dying after Kura durably stores the exact carrier but
    // before that carrier is replayed into State. Recovery authenticates the
    // future carrier without publishing or settling it early.
    let_row! { parent = state .kura .get_block(core::num::NonZeroUsize::new(1).expect("non-zero parent height")) .expect("fee merge carrier parent is durable") };
    let carrier = certified_merge_carrier_after(&parent, &entry);
    state
        .kura
        .store_block_with_merge_entry(Arc::new(carrier.clone()), &entry)
        .expect("persist exact merge carrier without State settlement");
    persist_merge_carrier_finality_for_state_test(&state.kura, &carrier);
    state
        .recover_merge_ledger_from_kura()
        .expect("authenticate durable exact merge carrier during recovery");
    assert!(state.merge_ledger().snapshot().is_empty());
    assert_eq!(
        account_asset_balance(&state, &asset_def_id, &sponsor_id),
        Quantity::from(10_u32)
    );
    state
        .replay_persisted_merge_settlements()
        .expect("future carrier must not settle before exact State replay");
    assert_eq!(
        account_asset_balance(&state, &asset_def_id, &sponsor_id),
        Quantity::from(10_u32)
    );
    commit_exact_merge_carrier_to_state(&state, &carrier, &entry);
    assert_eq!(
        account_asset_balance(&state, &asset_def_id, &sponsor_id),
        Quantity::from(7_u32)
    );
    assert!(state.settled_nexus_fee_receipts.read().contains(&source_id));
    state
        .replay_persisted_merge_settlements()
        .expect("marker-backed replay is idempotent");
    assert_eq!(
        account_asset_balance(&state, &asset_def_id, &sponsor_id),
        Quantity::from(7_u32)
    );
}
state_test! { sync commit_merge_entry_rejects_replayed_lane_snapshot_at_higher_epoch
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::default(), kura, query);
    let_row! { (candidate, commit_keypairs, _) = record_commit_ready_merge_candidate_with_lanes(&mut state, 1, 1) };
    let qc = merge_qc_for_candidate(&state, &candidate, &commit_keypairs, &[0]);
    state
        .commit_merge_entry(merge_entry_from_candidate(candidate.clone(), qc))
        .expect("initial merge entry commits");
    let_row! { replay_candidate = crate::merge::MergeLedgerCandidate { epoch_id: candidate.epoch_id.saturating_add(1), ..candidate } };
    let qc = merge_qc_for_candidate(&state, &replay_candidate, &commit_keypairs, &[0]);
    let_row! { err = state .commit_merge_entry(merge_entry_from_candidate(replay_candidate, qc)) .expect_err("higher-epoch lane snapshot replay must be rejected") };
    assert!(
        matches!(
            err,
            MergeLedgerCommitError::NonMonotonicLaneSnapshot {
                lane_id,
                dataspace_id,
                latest_height: 1,
                attempted_height: 1,
            } if lane_id == LaneId::new(0) && dataspace_id == DataSpaceId::UNIVERSAL
        ),
        "unexpected replay rejection: {err:?}"
    );
    assert_eq!(state.merge_ledger().len(), 1);
}
state_test! { sync commit_merge_entry_rejects_headerless_settlement_hash
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::default(), kura, query);
    let_row! { (mut candidate, commit_keypairs, _) = record_commit_ready_merge_candidate_with_lanes(&mut state, 1, 1) };
    let settlement = candidate.lane_snapshots[0].settlement_commitment.clone();
    let canonical_hash = candidate.lane_snapshots[0].settlement_hash;
    let headerless_hash = HashOf::new(&settlement);
    assert_ne!(
        headerless_hash, canonical_hash,
        "test requires framed and headerless Norito hashes to remain distinct"
    );
    candidate.lane_snapshots[0].settlement_hash = headerless_hash;
    let qc = merge_qc_for_candidate(&state, &candidate, &commit_keypairs, &[0]);
    let_row! { err = state .commit_merge_entry(merge_entry_from_candidate(candidate, qc)) .expect_err("merge admission must reject the non-protocol headerless hash") };
    assert!(matches!(
        err,
        MergeLedgerCommitError::SettlementCommitmentMismatch { lane_id }
            if lane_id == LaneId::SINGLE
    ));
    assert!(state.merge_ledger().is_empty());
}
state_test! { sync live_merge_rejects_historical_incarnation_reuse_beyond_rolling_cache
    let first = merge_entry_from_candidate(merge_candidate_with_lanes(1, 2), dummy_merge_qc());
    let gap = merge_entry_from_candidate(merge_candidate_with_lanes(2, 1), dummy_merge_qc());
    let replay = merge_entry_from_candidate(merge_candidate_with_lanes(3, 2), dummy_merge_qc());
    let mut history = MergeBindingHistory::default();
    history
        .validate_next(&first)
        .expect("initial binding history is valid");
    history.record(&first);
    history.validate_next(&gap).expect("lane omission is valid");
    history.record(&gap);
    let_row! { shared_err = history .validate_next(&replay) .expect_err("a retired incarnation must never become active again") };
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), Arc::clone(&kura), query);
    state.set_merge_ledger_cache_capacity(1);
    state.merge_ledger.replace(vec![first, gap]);
    assert_eq!(
        state.merge_ledger().len(),
        1,
        "the rolling query cache intentionally forgot the first incarnation"
    );
    state.merge_admission.write().binding_history = history;
    let_row! { live_err = state .validate_certified_merge_entry_for_global_order(&replay, ConsensusMode::Permissioned) .expect_err("live admission must consult the full binding history") };
    assert_eq!(live_err.to_string(), shared_err.to_string());
    assert!(matches!(
        live_err,
        MergeLedgerCommitError::IncarnationContext(reason)
            if reason.contains("reuses a historical incarnation")
    ));
    assert!(kura.merge_ledger_snapshot().is_empty());
}
state_test! { sync merge_binding_history_accepts_fresh_same_config_recreation
    let first = merge_entry_from_candidate(merge_candidate_with_lanes(1, 1), dummy_merge_qc());
    let mut history = MergeBindingHistory::default();
    history
        .validate_next(&first)
        .expect("initial binding is valid");
    history.record(&first);
    let historical_incarnation = first.active_lanes[0].incarnation;
    let fresh_incarnation = Hash::new(b"fresh-same-config-incarnation");
    let mut candidate = merge_candidate_with_lanes(2, 1);
    assert_eq!(candidate.lane_catalog_hash, first.lane_catalog_hash);
    candidate.active_lanes[0].incarnation = fresh_incarnation;
    candidate.active_lanes[0].activation_height =
        first.active_lanes[0].activation_height.saturating_add(1);
    let recreated = merge_entry_from_candidate(candidate, dummy_merge_qc());
    history
        .validate_next(&recreated)
        .expect("fresh later activation may replace an identical lane configuration");
    history.record(&recreated);
    assert!(
        history
            .historical_incarnations
            .contains(&historical_incarnation)
    );
    assert!(history.historical_incarnations.contains(&fresh_incarnation));
}
state_test! { sync merge_binding_history_rejects_config_drift_under_same_catalog_hash
    let first = merge_entry_from_candidate(merge_candidate_with_lanes(1, 1), dummy_merge_qc());
    let_row! { history = MergeBindingHistory::from_entries(std::slice::from_ref(&first)) .expect("initial binding history") };
    let mut candidate = merge_candidate_with_lanes(2, 1);
    assert_eq!(candidate.lane_catalog_hash, first.lane_catalog_hash);
    candidate.active_lanes[0].lane_config_hash = Hash::new(b"uncommitted-config-drift");
    candidate.active_lanes[0].incarnation = Hash::new(b"fresh-config-drift-incarnation");
    candidate.active_lanes[0].activation_height =
        first.active_lanes[0].activation_height.saturating_add(1);
    let attempted = merge_entry_from_candidate(candidate, dummy_merge_qc());
    let_row! { err = history .validate_next(&attempted) .expect_err("unchanged catalog hash must reject lane configuration drift") };
    assert!(matches!(
        err,
        MergeLedgerCommitError::IncarnationContext(reason)
            if reason.contains("unchanged catalog hash changed its active lane configuration")
    ));
}
state_test! { sync merge_binding_history_rejects_fresh_incarnation_without_later_activation
    let first = merge_entry_from_candidate(merge_candidate_with_lanes(1, 2), dummy_merge_qc());
    let gap = merge_entry_from_candidate(merge_candidate_with_lanes(2, 1), dummy_merge_qc());
    let_row! { history = MergeBindingHistory::from_entries(&[first, gap]) .expect("valid prefix builds binding history") };
    let historical_before = history.historical_incarnations.clone();
    let activations_before = history.latest_activation_by_lane.clone();
    let latest_epoch_before = history.latest_entry.as_ref().map(|entry| entry.epoch_id);
    let mut candidate = merge_candidate_with_lanes(3, 2);
    candidate.active_lanes[1].incarnation = Hash::new(b"fresh-but-not-later");
    let attempted = merge_entry_from_candidate(candidate, dummy_merge_qc());
    let_row! { err = history .validate_next(&attempted) .expect_err("reactivation must advance its authenticated activation height") };
    assert!(matches!(
        err,
        MergeLedgerCommitError::IncarnationContext(reason)
            if reason.contains("does not advance its historical activation height")
    ));
    assert_eq!(history.historical_incarnations, historical_before);
    assert_eq!(history.latest_activation_by_lane, activations_before);
    assert_eq!(
        history.latest_entry.as_ref().map(|entry| entry.epoch_id),
        latest_epoch_before
    );
}
state_test! { sync merge_snapshot_rejects_proposal_height_after_carrier
    let mut candidate = merge_candidate_with_lanes(1, 1);
    let invalid_proposal_height = candidate.carrier_height.saturating_add(1);
    candidate.lane_snapshots[0].proposal_height = invalid_proposal_height;
    let_row! { err = validate_merge_snapshot_carrier_bounds(candidate.carrier_height, &candidate.lane_snapshots) .expect_err("a merge carrier cannot authenticate a later relay proposal") };
    assert!(matches!(
        err,
        MergeLedgerCommitError::ExecutionBatchInvalid(reason)
            if reason.contains("relay proposal height")
                && reason.contains("after merge carrier height")
    ));
}
state_test! { sync validate_merge_quorum_certificate_rejects_unbound_live_carrier
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), kura, query);
    let commit_keypairs = configure_commit_topology(&state, 1);
    let candidate = merge_candidate_with_lanes(1, 1);
    let qc = merge_qc_for_candidate(&state, &candidate, &commit_keypairs, &[0]);
    let entry = merge_entry_from_candidate(candidate, qc);
    let block_hashes = state.block_hashes.block_and_revert();
    block_hashes.commit_for_tests();
    assert!(
        state.latest_block_hash_fast().is_none(),
        "the live carrier parent must be absent for this fixture"
    );
    let_row! { err = state .validate_merge_quorum_certificate(&entry, true, true) .expect_err("live carrier binding must reject an absent synthetic parent") };
    assert!(matches!(
        err,
        MergeLedgerCommitError::ExecutionBatchInvalid(reason)
            if reason == "merge QC is bound to a stale or future global carrier"
    ));
    assert!(state.merge_ledger().is_empty());
}
state_test! { sync commit_merge_entry_rejects_non_contiguous_lane_snapshot
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), kura, query);

    let commit_keypairs = configure_commit_topology(&state, 1);
    let (validator_ids, validator_keypairs) = bls_accounts_in("validators", 4);
    seed_consensus_keys_with_pops(&state, &validator_keypairs);
    install_lane_manifest_registry(
        &state,
        &[(LaneId::new(0), DataSpaceId::UNIVERSAL, validator_ids)],
    );
    let_row! { skipped = sample_lane_relay_envelope_for_state(&state, 2, LaneId::new(0), &validator_keypairs) };
    state
        .lane_relays
        .write()
        .insert(skipped.clone())
        .expect("seed skipped relay");
    ensure_merge_carrier_parent_for_test(&state);
    let regressed_candidate = merge_candidate_from_relay(&state, 1, &skipped);
    let regressed_qc = merge_qc_for_candidate(&state, &regressed_candidate, &commit_keypairs, &[0]);
    let_row! { err = state .commit_merge_entry(merge_entry_from_candidate( regressed_candidate, regressed_qc, )) .expect_err("merge commit must reject a skipped lane height") };
    assert!(
        matches!(
            err,
            MergeLedgerCommitError::NonContiguousLaneSnapshot {
                lane_id,
                dataspace_id,
                expected_height: 1,
                attempted_height: 2,
            } if lane_id == LaneId::new(0) && dataspace_id == DataSpaceId::UNIVERSAL
        ),
        "unexpected skipped-snapshot rejection: {err:?}"
    );
    assert!(state.merge_ledger().is_empty());
}
state_test! { sync commit_merge_entry_rejects_replay_for_lane_omitted_from_latest_active_entry
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::default(), kura, query);
    let_row! { (first_candidate, commit_keypairs, validator_keypairs) = record_commit_ready_merge_candidate_with_lanes(&mut state, 2, 1) };
    let replay_context = first_candidate.clone();
    let_row! { lane1_snapshot = first_candidate .lane_snapshots .iter() .cloned() .find(|snapshot| snapshot.lane_id == LaneId::new(1)) .expect("first merge includes lane1") };
    let first_qc = merge_qc_for_candidate(&state, &first_candidate, &commit_keypairs, &[0]);
    let_row! { first_stored = state .commit_merge_entry(merge_entry_from_candidate(first_candidate, first_qc)) .expect("initial two-lane merge commits") };
    publish_committed_merge_carrier_for_test(&state, first_stored.as_ref());
    let_row! { lane0_h2 = seed_effect_authenticated_relay_for_merge_test( &state, sample_lane_relay_envelope_for_state(&state, 2, LaneId::new(0), &validator_keypairs), ) };
    state
        .lane_relays
        .write()
        .insert(lane0_h2.clone())
        .expect("seed newer lane0 relay");
    ensure_merge_carrier_parent_for_test(&state);
    let second_candidate = merge_candidate_from_relay(&state, 2, &lane0_h2);
    let second_qc = merge_qc_for_candidate(&state, &second_candidate, &commit_keypairs, &[0]);
    state
        .commit_merge_entry(merge_entry_from_candidate(second_candidate, second_qc))
        .expect("lane0-only active merge commits");
    let replay_roots = vec![lane1_snapshot.merge_hint_root];
    let_row! { replay_candidate = crate::merge::MergeLedgerCandidate { epoch_id: 3, view: 0, lane_snapshots: vec![lane1_snapshot], global_state_root: crate::merge::reduce_merge_hint_roots(&replay_roots), ..replay_context } };
    let replay_qc = merge_qc_for_candidate(&state, &replay_candidate, &commit_keypairs, &[0]);
    let_row! { err = state .commit_merge_entry(merge_entry_from_candidate(replay_candidate, replay_qc)) .expect_err("lane replay omitted from latest active entry must be rejected") };
    assert!(matches!(
        err,
        MergeLedgerCommitError::NonMonotonicLaneSnapshot {
            lane_id,
            dataspace_id,
            latest_height: 1,
            attempted_height: 1,
        } if lane_id == LaneId::new(1) && dataspace_id == DataSpaceId::UNIVERSAL
    ));
    assert_eq!(state.merge_ledger().len(), 2);
}
state_test! { sync commit_merge_entry_rejects_unknown_catalog_lane
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), kura, query);

    let commit_keypairs = configure_commit_topology(&state, 1);
    let (_, validator_keypair) = bls_account_in("validators");
    let signers = [&validator_keypair];
    let_row! { envelope = sample_lane_relay_envelope( 2, LaneId::new(1), &signers, full_signer_bitmap(signers.len()), ) };
    state
        .lane_relays
        .write()
        .insert(envelope.clone())
        .expect("seed stale relay cache");
    ensure_merge_carrier_parent_for_test(&state);
    let candidate = merge_candidate_from_relay(&state, 1, &envelope);
    let qc = merge_qc_for_candidate(&state, &candidate, &commit_keypairs, &[0]);
    let entry = merge_entry_from_candidate(candidate, qc);
    let_row! { err = state .commit_merge_entry(entry) .expect_err("merge commit must reject snapshots outside the active lane catalog") };
    assert!(matches!(
        err,
        MergeLedgerCommitError::UnknownLane {
            lane_id
        } if lane_id == LaneId::new(1)
    ));
    assert!(state.merge_ledger().is_empty());
}
state_test! { sync commit_merge_entry_rejects_stale_geometry_for_removed_catalog_lane
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), kura, query);
    let stale_lane = LaneId::new(1);
    let_row! { stale_geometry_catalog = LaneCatalog::new( nonzero!(2_u32), vec![ LaneConfig::default(), LaneConfig { id: stale_lane, alias: "stale-merge".to_owned(), ..LaneConfig::default() }, ], ) .expect("stale lane geometry") };
    {
        let mut nexus = state.nexus.write();
        install_test_nexus_lane_catalog(
            &mut nexus,
            LaneCatalog::new(nonzero!(1_u32), vec![LaneConfig::default()])
                .expect("authoritative lane catalog"),
        );
        nexus.lane_config = RuntimeLaneConfig::from_catalog(&stale_geometry_catalog);
        assert!(
            nexus.lane_config.entry(stale_lane).is_some(),
            "test must seed derived geometry for the removed lane"
        );
        assert!(
            nexus
                .lane_catalog
                .lanes()
                .iter()
                .all(|lane| lane.id != stale_lane),
            "test must keep stale lane out of the authoritative catalog"
        );
    }
    let commit_keypairs = configure_commit_topology(&state, 1);
    let (_, validator_keypair) = bls_account_in("validators");
    let signers = [&validator_keypair];
    let_row! { envelope = sample_lane_relay_envelope(2, stale_lane, &signers, full_signer_bitmap(signers.len())) };
    state
        .lane_relays
        .write()
        .insert(envelope.clone())
        .expect("seed stale relay cache");
    ensure_merge_carrier_parent_for_test(&state);
    let candidate = merge_candidate_from_relay(&state, 1, &envelope);
    let qc = merge_qc_for_candidate(&state, &candidate, &commit_keypairs, &[0]);
    let entry = merge_entry_from_candidate(candidate, qc);
    let_row! { err = state .commit_merge_entry(entry) .expect_err("stale geometry must not make a removed lane merge-active") };
    assert!(matches!(
        err,
        MergeLedgerCommitError::UnknownLane { lane_id } if lane_id == stale_lane
    ));
    assert!(state.merge_ledger().is_empty());
}
state_test! { sync commit_merge_entry_rejects_future_created_autoscale_lane_snapshot
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), kura, query);
    let future_lane = LaneId::new(1);
    install_autoscale_elastic_catalog_for_test(
        &state,
        autoscale_elastic_catalog_lane_for_test(future_lane, 7),
    );
    let commit_keypairs = configure_commit_topology(&state, 1);
    let (_, validator_keypair) = bls_account_in("validators");
    let signers = [&validator_keypair];
    let_row! { incarnation = state .lane_incarnation(future_lane) .expect("future-created catalog lane has a committed incarnation") };
    let_row! { stale_relay = sample_lane_relay_envelope_with_network_dataspace_view_and_incarnation( 1, future_lane, DataSpaceId::UNIVERSAL, state.network_id_ref(), 0, incarnation, &signers, full_signer_bitmap(signers.len()), ) };
    state
        .lane_relays
        .write()
        .insert(stale_relay.clone())
        .expect("seed stale future-created relay cache");
    ensure_merge_carrier_parent_for_test(&state);
    let candidate = merge_candidate_from_relay(&state, 1, &stale_relay);
    let qc = merge_qc_for_candidate(&state, &candidate, &commit_keypairs, &[0]);
    let entry = merge_entry_from_candidate(candidate, qc);
    let_row! { err = state .commit_merge_entry(entry) .expect_err("merge commit must reject future-created autoscale lane snapshots") };
    assert!(
        matches!(
            err,
            MergeLedgerCommitError::UnknownLane { lane_id } if lane_id == future_lane
        ),
        "future-created merge returned unexpected error: {err:?}"
    );
    assert!(state.merge_ledger().is_empty());
}
state_test! { sync commit_merge_entry_rejects_catalog_dataspace_mismatch
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), kura, query);

    let commit_keypairs = configure_commit_topology(&state, 1);
    let (_, validator_keypair) = bls_account_in("validators");
    let signers = [&validator_keypair];
    let unexpected_dataspace = DataSpaceId::new(99);
    let_row! { envelope = sample_lane_relay_envelope_with_dataspace( 2, LaneId::new(0), unexpected_dataspace, &signers, full_signer_bitmap(signers.len()), ) };
    state
        .lane_relays
        .write()
        .insert(envelope.clone())
        .expect("seed stale relay cache");
    let candidate = merge_candidate_from_relay(&state, 1, &envelope);
    let qc = merge_qc_for_candidate(&state, &candidate, &commit_keypairs, &[0]);
    let entry = merge_entry_from_candidate(candidate, qc);
    let_row! { err = state .commit_merge_entry(entry) .expect_err("merge commit must reject stale dataspace bindings") };
    assert!(matches!(
        err,
        MergeLedgerCommitError::DataspaceMismatch {
            lane_id,
            expected,
            actual
        } if lane_id == LaneId::new(0)
            && expected == DataSpaceId::UNIVERSAL
            && actual == unexpected_dataspace
    ));
    assert!(state.merge_ledger().is_empty());
}
state_test! { sync commit_merge_entry_rejects_unknown_dataspace_catalog_entry
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), kura, query);
    {
        let mut nexus = state.nexus.write();
        nexus.dataspace_catalog =
            DataSpaceCatalog::new(Vec::new()).expect("empty dataspace catalog");
    }
    let commit_keypairs = configure_commit_topology(&state, 1);
    let (_, validator_keypair) = bls_account_in("validators");
    let signers = [&validator_keypair];
    let_row! { envelope = sample_lane_relay_envelope_with_state_incarnation_unchecked( &state, 1, LaneId::new(0), &signers, full_signer_bitmap(signers.len()), ) };
    state
        .lane_relays
        .write()
        .insert(envelope.clone())
        .expect("seed stale relay cache");
    let candidate = merge_candidate_from_relay(&state, 1, &envelope);
    let qc = merge_qc_for_candidate(&state, &candidate, &commit_keypairs, &[0]);
    let entry = merge_entry_from_candidate(candidate, qc);
    let_row! { err = state .commit_merge_entry(entry) .expect_err("merge commit must reject snapshots for missing dataspace catalog entries") };
    assert!(matches!(
        err,
        MergeLedgerCommitError::UnknownDataspace {
            dataspace_id
        } if dataspace_id == DataSpaceId::UNIVERSAL
    ));
    assert!(state.merge_ledger().is_empty());
}
state_test! { sync commit_merge_entry_rejects_empty_entry
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new(World::default(), kura, query);
    let_row! { entry = MergeLedgerEntry { version: MergeLedgerEntry::VERSION, epoch_id: 1, lane_catalog_hash: Hash::new(b"catalog"), active_lanes: Vec::new(), incarnation_root: Hash::new(b"incarnations"), activation_root: Hash::new(b"activations"), lane_snapshots: Vec::new(), execution_batch: None, lane_drain_certificates: Vec::new(), global_state_root: iroha_crypto::Hash::new(b"root"), merge_qc: dummy_merge_qc(), } };
    let_row! { err = state .commit_merge_entry(entry) .expect_err("empty merge entry must be rejected") };
    assert!(matches!(err, MergeLedgerCommitError::EmptyEntry));
}
state_test! { sync commit_merge_entry_rejects_unsorted_lane_snapshots
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new(World::default(), kura, query);
    let candidate = merge_candidate_with_lanes(1, 2);
    let mut entry = merge_entry_from_candidate(candidate, dummy_merge_qc());
    entry.lane_snapshots.reverse();
    let_row! { err = state .commit_merge_entry(entry) .expect_err("unsorted lane snapshots must be rejected") };
    assert!(matches!(
        err,
        MergeLedgerCommitError::LaneSnapshotOrderViolation { .. }
    ));
}
state_test! { sync commit_merge_entry_rejects_qc_digest_mismatch
    setup_merge_qc_test!(kura, query, state, keypairs, candidate);
    let mut qc = merge_qc_for_candidate(&state, &candidate, &keypairs, &[0]);
    qc.message_digest = Hash::new(b"wrong-digest");
    let entry = merge_entry_from_candidate(candidate, qc);
    let_row! { err = state .commit_merge_entry(entry) .expect_err("digest mismatch must be rejected") };
    assert!(matches!(
        err,
        MergeLedgerCommitError::MergeQCDigestMismatch { .. }
    ));
}
state_test! { sync commit_merge_entry_rejects_qc_signer_superset
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new(World::default(), kura, query);
    let keypairs = configure_commit_topology(&state, 4);
    let candidate = merge_candidate_with_lanes(1, 1);
    let qc = merge_qc_for_candidate(&state, &candidate, &keypairs, &[0, 1, 2, 3]);
    let entry = merge_entry_from_candidate(candidate, qc);
    let_row! { err = state .commit_merge_entry(entry) .expect_err("signer superset must be rejected") };
    assert!(matches!(
        err,
        MergeLedgerCommitError::MergeQCSignerCountMismatch { observed: 4, required: 3 }
    ));
}
state_test! { sync commit_merge_entry_rejects_invalid_aggregate_signature
    setup_merge_qc_test!(kura, query, state, keypairs, candidate);
    let mut qc = merge_qc_for_candidate(&state, &candidate, &keypairs, &[0]);
    if let Some(first) = qc.aggregate_signature.first_mut() {
        *first ^= 0xFF;
    }
    let entry = merge_entry_from_candidate(candidate, qc);
    let_row! { err = state .commit_merge_entry(entry) .expect_err("invalid aggregate signature must be rejected") };
    assert!(matches!(
        err,
        MergeLedgerCommitError::MergeQCAggregateSignatureInvalid
    ));
}
state_test! { sync commit_merge_entry_rejects_qc_for_another_network
    setup_merge_qc_test!(kura, query, state, keypairs, candidate);
    let mut qc = merge_qc_for_candidate(&state, &candidate, &keypairs, &[0]);
    qc.network_id = crate::sumeragi::synthetic_network_id("foreign-merge-qc-genesis");
    let_row! { err = state .commit_merge_entry(merge_entry_from_candidate(candidate, qc)) .expect_err("cross-network QC must fail closed") };
    assert!(matches!(
        err,
        MergeLedgerCommitError::MergeQCNetworkIdMismatch { .. }
    ));
}
state_test! { sync commit_merge_entry_rejects_duplicate_historical_validator
    setup_merge_qc_test!(kura, query, state, keypairs, candidate);
    let mut qc = merge_qc_for_candidate(&state, &candidate, &keypairs, &[0]);
    qc.validator_set.push(qc.validator_set[0].clone());
    qc.validator_set_hash = HashOf::new(&qc.validator_set);
    let_row! { err = state .commit_merge_entry(merge_entry_from_candidate(candidate, qc)) .expect_err("duplicated historical roster entries must fail closed") };
    assert!(matches!(
        err,
        MergeLedgerCommitError::MergeQCDuplicateValidator(_)
    ));
}
state_test! { sync commit_merge_entry_rejects_nonzero_signer_bitmap_padding
    setup_merge_qc_test!(kura, query, state, keypairs, candidate);
    let mut qc = merge_qc_for_candidate(&state, &candidate, &keypairs, &[0]);
    qc.signers_bitmap[0] |= 0x80;
    let_row! { err = state .commit_merge_entry(merge_entry_from_candidate(candidate, qc)) .expect_err("bitmap padding is malleable and must be rejected") };
    assert!(matches!(err, MergeLedgerCommitError::MergeQCBitmapPadding));
}
state_test! { sync commit_merge_entry_rejects_invalid_historical_signer_pop
    setup_merge_qc_test!(kura, query, state, keypairs, candidate);
    let mut qc = merge_qc_for_candidate(&state, &candidate, &keypairs, &[0]);
    qc.signer_proofs[0].proof_of_possession[0] ^= 0x80;
    let_row! { err = state .commit_merge_entry(merge_entry_from_candidate(candidate, qc)) .expect_err("invalid historical PoP must fail closed") };
    assert!(matches!(
        err,
        MergeLedgerCommitError::MergeQCSignerPopInvalid { signer: 0 }
    ));
}
state_test! { sync commit_merge_entry_rejects_short_merge_qc_signature
    setup_merge_qc_test!(kura, query, state, keypairs, candidate);
    let mut qc = merge_qc_for_candidate(&state, &candidate, &keypairs, &[0]);
    qc.aggregate_signature
        .truncate(MERGE_QC_BLS_PROOF_BYTES - 1);
    let_row! { err = state .commit_merge_entry(merge_entry_from_candidate(candidate, qc)) .expect_err("short BLS aggregate signature must fail closed") };
    assert!(
        matches!(err, MergeLedgerCommitError::ExecutionBatchInvalid(_)),
        "unexpected short aggregate-signature error: {err:?}"
    );
}
state_test! { sync commit_merge_entry_rejects_long_merge_qc_signature
    setup_merge_qc_test!(kura, query, state, keypairs, candidate);
    let mut qc = merge_qc_for_candidate(&state, &candidate, &keypairs, &[0]);
    qc.aggregate_signature.push(0);
    let_row! { err = state .commit_merge_entry(merge_entry_from_candidate(candidate, qc)) .expect_err("long BLS aggregate signature must fail before cryptography") };
    assert!(matches!(
        err,
        MergeLedgerCommitError::ExecutionBatchInvalid(_)
    ));
}
state_test! { sync commit_merge_entry_rejects_short_merge_qc_pop
    setup_merge_qc_test!(kura, query, state, keypairs, candidate);
    let mut qc = merge_qc_for_candidate(&state, &candidate, &keypairs, &[0]);
    qc.signer_proofs[0]
        .proof_of_possession
        .truncate(MERGE_QC_BLS_PROOF_BYTES - 1);
    let_row! { err = state .commit_merge_entry(merge_entry_from_candidate(candidate, qc)) .expect_err("short BLS proof of possession must fail closed") };
    assert!(
        matches!(err, MergeLedgerCommitError::ExecutionBatchInvalid(_)),
        "unexpected short signer-PoP error: {err:?}"
    );
}
state_test! { sync commit_merge_entry_rejects_long_merge_qc_pop
    setup_merge_qc_test!(kura, query, state, keypairs, candidate);
    let mut qc = merge_qc_for_candidate(&state, &candidate, &keypairs, &[0]);
    qc.signer_proofs[0].proof_of_possession.push(0);
    let_row! { err = state .commit_merge_entry(merge_entry_from_candidate(candidate, qc)) .expect_err("long BLS proof of possession must fail before cryptography") };
    assert!(matches!(
        err,
        MergeLedgerCommitError::ExecutionBatchInvalid(_)
    ));
}
state_test! { sync commit_merge_entry_rejects_tampered_incarnation_context
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new(World::default(), kura, query);
    let candidate = merge_candidate_with_lanes(1, 1);
    let mut entry = merge_entry_from_candidate(candidate, dummy_merge_qc());
    entry.activation_root = Hash::new(b"tampered-activation-root");
    let_row! { err = state .commit_merge_entry(entry) .expect_err("tampered historical activation root must fail closed") };
    assert!(matches!(err, MergeLedgerCommitError::IncarnationContext(_)));
}
state_test! { sync commit_merge_entry_updates_cache
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new(World::default(), kura, query);
    let (candidate, keypairs, _) = record_commit_ready_merge_candidate_with_lanes(&mut state, 2, 1);
    let qc = merge_qc_for_candidate(&state, &candidate, &keypairs, &[0]);
    let entry = merge_entry_from_candidate(candidate, qc);
    let epoch_id = entry.epoch_id;
    let_row! { stored = state .commit_merge_entry(entry) .expect("merge entry commit succeeds") };
    assert_eq!(stored.epoch_id, epoch_id);
    let stored_ref = stored.as_ref();
    let stored_merge_hint_roots = stored_ref.merge_hint_roots();
    let roots_view = state.world.merge_hint_roots.view();
    assert_eq!(&*roots_view, &stored_merge_hint_roots);
    let global_view = state.world.merge_global_state_root.view();
    assert_eq!(global_view.as_ref(), Some(&stored_ref.global_state_root));
    let_row! { latest = state .merge_ledger() .latest() .expect("latest merge entry present") };
    assert!(Arc::ptr_eq(&stored, &latest));
    assert_eq!(state.merge_ledger().len(), 1);
    let events = state.world.external_event_buf.view();
    assert!(matches!(
        events.last(),
        Some(EventBox::Pipeline(PipelineEventBox::Merge(_)))
    ));
}
state_test! { sync globally_carried_merge_publication_is_durable_idempotent_and_replay_silent
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new(World::default(), Arc::clone(&kura), query);
    let_row! { genesis = new_dummy_block_with_payload(|header| { header.set_height(nonzero!(1_u64)); header.set_prev_block_hash(None); }) };
    let genesis = Arc::new(genesis.as_ref().clone());
    kura.store_block(Arc::clone(&genesis))
        .expect("store fixture genesis");
    state.push_block_hash_for_testing(genesis.hash());
    let_row! { carrier_fixture = |parent: &SignedBlock, epoch: u64| { let parent_hash = parent.hash(); let carrier = new_dummy_block_with_payload(|header| { header.set_height( NonZeroU64::new(parent.header().height().get().saturating_add(1)) .expect("carrier height is nonzero"), ); header.set_prev_block_hash(Some(parent_hash)); }); let mut entry = merge_entry_from_candidate(merge_candidate_with_lanes(epoch, 1), dummy_merge_qc()); entry.merge_qc.epoch_id = epoch; entry.merge_qc.view = carrier.as_ref().header().view_change_index(); entry.merge_qc.carrier_height = carrier.as_ref().header().height().get(); entry.merge_qc.carrier_parent_hash = parent_hash; let mut signed = carrier.as_ref().clone(); let context = signed .execution_context() .cloned() .unwrap_or_else(|| BlockExecutionContextBundle::new(Vec::new())) .with_merge_entry(iroha_data_model::block::CertifiedMergeLedgerReference::new( &entry, )); signed.set_execution_context(Some(context)); (Arc::new(signed), entry) } };
    let (carrier_one, entry_one) = carrier_fixture(&genesis, 1);
    let_row! { err = state .record_globally_committed_merge_entry(&entry_one, MergeLedgerPublicationMode::LiveCommit) .expect_err("uncommitted future entry must not publish or emit") };
    assert!(matches!(
        err,
        MergeLedgerCommitError::Persistence(crate::kura::Error::MergeCarrierConflict(_))
    ));
    assert!(state.merge_ledger().snapshot().is_empty());
    kura.store_block_with_merge_entry(Arc::clone(&carrier_one), &entry_one)
        .expect("store first durable carrier and sidecar");
    persist_merge_carrier_finality_for_state_test(&kura, carrier_one.as_ref());
    state.push_block_hash_for_testing(carrier_one.hash());
    let_row! { (stored_one, first_event) = state .record_globally_committed_merge_entry(&entry_one, MergeLedgerPublicationMode::LiveCommit) .expect("publish first live merge") };
    assert_eq!(stored_one.as_ref(), &entry_one);
    let_row! { Some(PipelineEventBox::Merge(first_event)) = first_event else { panic!("first live publication must return its merge event"); } };
    assert_eq!(first_event.entry, entry_one);
    let_row! { (_, duplicate_event) = state .record_globally_committed_merge_entry(&entry_one, MergeLedgerPublicationMode::LiveCommit) .expect("idempotent live retry") };
    assert!(
        duplicate_event.is_none(),
        "idempotent live retry must not emit twice"
    );
    let (carrier_two, entry_two) = carrier_fixture(&carrier_one, 2);
    let_row! { err = state .record_globally_committed_merge_entry(&entry_two, MergeLedgerPublicationMode::Replay) .expect_err("replay must not publish a future carrier") };
    assert!(matches!(
        err,
        MergeLedgerCommitError::Persistence(crate::kura::Error::MergeCarrierConflict(_))
    ));
    assert_eq!(state.merge_ledger().snapshot().len(), 1);
    let carrier_two_hash = carrier_two.hash();
    kura.store_block_with_merge_entry(Arc::clone(&carrier_two), &entry_two)
        .expect("store second durable carrier and sidecar");
    persist_merge_carrier_finality_for_state_test(&kura, carrier_two.as_ref());
    state.push_block_hash_for_testing(carrier_two_hash);
    let_row! { (stored_two, replay_event) = state .record_globally_committed_merge_entry(&entry_two, MergeLedgerPublicationMode::Replay) .expect("replay durable second merge") };
    assert_eq!(stored_two.as_ref(), &entry_two);
    assert!(replay_event.is_none(), "replay must remain event-silent");
    let_row! { (_, post_replay_event) = state .record_globally_committed_merge_entry(&entry_two, MergeLedgerPublicationMode::LiveCommit) .expect("live retry after replay is idempotent") };
    assert!(
        post_replay_event.is_none(),
        "replayed history must never be re-emitted as a live merge"
    );
    assert_eq!(state.merge_ledger().snapshot().len(), 2);
    assert!(
        state.world.external_event_buf.view().is_empty(),
        "publication helper must return live events to the commit pipeline, not leak them into replay state"
    );
}
state_test! { sync commit_merge_entry_persists_to_kura
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new(World::default(), Arc::clone(&kura), query);
    let (candidate, keypairs, _) = record_commit_ready_merge_candidate_with_lanes(&mut state, 3, 1);
    let qc = merge_qc_for_candidate(&state, &candidate, &keypairs, &[0]);
    let entry = merge_entry_from_candidate(candidate, qc);
    let epoch = entry.epoch_id;
    state
        .commit_merge_entry(entry)
        .expect("merge entry commit persists");
    let persisted = kura.merge_ledger_snapshot();
    assert_eq!(persisted.len(), 1, "kura stores committed entry");
    assert_eq!(persisted[0].epoch_id, epoch);
}
state_test! { sync apply_without_execution_refreshes_merge_metadata
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new(World::default(), kura, query);
    let (candidate, keypairs, _) = record_commit_ready_merge_candidate_with_lanes(&mut state, 2, 1);
    let qc = merge_qc_for_candidate(&state, &candidate, &keypairs, &[0]);
    let entry = merge_entry_from_candidate(candidate, qc);
    let_row! { stored = state .commit_merge_entry(entry) .expect("merge entry commit persists") };
    let stored_ref = stored.as_ref();
    let stored_merge_hint_roots = stored_ref.merge_hint_roots();
    {
        let mut roots_block = state.world.merge_hint_roots.block();
        {
            let mut tx = roots_block.transaction();
            tx.clear();
            tx.apply();
        }
        roots_block.commit();
    }
    {
        let mut block = state.world.merge_global_state_root.block();
        let mut tx = block.transaction();
        *tx = None;
        tx.apply();
        block.commit();
    }
    let_row! { block = new_dummy_block_with_payload(|header| { header.set_height(nonzero!(1_u64)); }) };
    let mut state_block = state.block(block.as_ref().header());
    let _ = state_block.apply_without_execution(&block, Vec::new());
    state_block.commit().expect("commit apply block");
    assert_eq!(
        &*state.world.merge_hint_roots.view(),
        &stored_merge_hint_roots
    );
    assert_eq!(
        state.world.merge_global_state_root.view().as_ref(),
        Some(&stored_ref.global_state_root)
    );
}
state_test! { sync state_rehydrates_merge_ledger_from_kura_snapshot
    let_row! { (original, validator_keypairs, commit_keypairs, parent) = configured_single_lane_merge_state() };
    let kura = Arc::clone(&original.kura);
    let first = next_relay_merge_entry(&original, 1, &validator_keypairs, &commit_keypairs);
    let first_carrier = store_and_commit_exact_merge_carrier(&original, &parent, &first);
    let second = next_relay_merge_entry(&original, 2, &validator_keypairs, &commit_keypairs);
    store_and_commit_exact_merge_carrier(&original, &first_carrier, &second);
    drop(original);
    let query = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::default(), Arc::clone(&kura), query);
    assert!(
        state.merge_ledger().is_empty(),
        "durable future carriers must not publish before restored State history reaches them"
    );
    for height in 1..=kura.blocks_count() {
        let_row! { block = kura .get_block(NonZeroUsize::new(height).expect("restored height is non-zero")) .expect("durable restored block is available") };
        state.push_block_hash_for_testing(block.hash());
    }
    state
        .recover_merge_ledger_from_kura()
        .expect("rehydrate merge ledger from restored Kura history");
    let snapshot = state.merge_ledger().snapshot();
    assert_eq!(snapshot.len(), 2, "state seeds merge cache from kura");
    assert_eq!(snapshot[0].as_ref(), &first);
    assert_eq!(snapshot[1].as_ref(), &second);
    let kura_snapshot = kura.merge_ledger_snapshot();
    assert_eq!(kura_snapshot, vec![first, second]);
}
state_test! { sync state_rehydrates_multi_lane_merge_ledger_from_kura_snapshot
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut original = State::new_for_testing(World::default(), Arc::clone(&kura), query);
    let_row! { (first_candidate, first_keypairs, _) = record_commit_ready_merge_candidate_with_lanes(&mut original, 2, 1) };
    let first_qc = merge_qc_for_candidate(&original, &first_candidate, &first_keypairs, &[0]);
    let first = merge_entry_from_candidate(first_candidate, first_qc);
    let_row! { first_stored = original .commit_merge_entry(first.clone()) .expect("commit first authenticated multi-lane merge entry") };
    publish_committed_merge_carrier_for_test(&original, first_stored.as_ref());
    let_row! { (second_candidate, second_keypairs, _) = record_commit_ready_merge_candidate_with_lanes(&mut original, 2, 2) };
    let second_qc = merge_qc_for_candidate(&original, &second_candidate, &second_keypairs, &[0]);
    let second = merge_entry_from_candidate(second_candidate, second_qc);
    let_row! { second_stored = original .commit_merge_entry(second.clone()) .expect("commit second authenticated multi-lane merge entry") };
    publish_committed_merge_carrier_for_test(&original, second_stored.as_ref());
    drop(original);
    let state = blank_test_state_from_kura(&kura);
    assert!(
        state.merge_ledger().is_empty(),
        "durable multi-lane carriers must remain unpublished until State history catches up"
    );
    ensure_merge_carrier_parent_for_test(&state);
    state
        .recover_merge_ledger_from_kura()
        .expect("rehydrate multi-lane merge ledger from Kura history");
    let snapshot = state.merge_ledger().snapshot();
    assert_eq!(snapshot.len(), 2, "state seeds both multi-lane epochs");
    assert_eq!(snapshot[0].as_ref(), &first);
    assert_eq!(snapshot[1].as_ref(), &second);
    assert_eq!(
        kura.merge_ledger_snapshot(),
        vec![first, second],
        "recovery must not rewrite durable multi-lane history"
    );
}
state_test! { sync restart_rejects_invalid_historical_merge_qc_without_truncation
    let_row! { (original, validator_keypairs, commit_keypairs, parent) = configured_single_lane_merge_state() };
    let kura = Arc::clone(&original.kura);
    let first = next_relay_merge_entry(&original, 1, &validator_keypairs, &commit_keypairs);
    let first_carrier = store_and_commit_exact_merge_carrier(&original, &parent, &first);
    let mut invalid = next_relay_merge_entry(&original, 2, &validator_keypairs, &commit_keypairs);
    invalid.merge_qc.aggregate_signature[0] ^= 0x80;
    let invalid_carrier = certified_merge_carrier_after(&first_carrier, &invalid);
    kura.store_block_with_merge_entry(Arc::new(invalid_carrier.clone()), &invalid)
        .expect("store invalid entry in an exact global carrier");
    persist_merge_carrier_finality_for_state_test(&kura, &invalid_carrier);
    let_row! { durable_entries_before = kura .merge_ledger_all_entries() .expect("durable entries before restart") };
    let_row! { durable_carriers_before = kura .merge_carrier_records() .expect("durable carriers before restart") };
    drop(original);
    let_row! { recovery = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| { State::new_for_testing( World::default(), Arc::clone(&kura), LiveQueryStore::start_test(), ) })) };
    let_row! { panic = match recovery { Ok(_) => panic!("invalid globally carried QC suffix must abort recovery"), Err(payload) => panic_payload_text(payload), } };
    assert!(
        panic.contains("aggregate signature")
            || panic.contains("BLS")
            || panic.contains("MergeQCAggregateSignatureInvalid"),
        "unexpected strict recovery rejection: {panic}"
    );
    assert_eq!(
        kura.merge_ledger_all_entries().expect("durable history"),
        vec![first.clone(), invalid.clone()],
        "fixture and post-recovery history must retain the authenticated prefix and invalid carried suffix"
    );
    assert_eq!(
        kura.merge_ledger_all_entries()
            .expect("durable entries remain unchanged"),
        durable_entries_before
    );
    assert_eq!(
        kura.merge_carrier_records()
            .expect("durable carriers remain unchanged"),
        durable_carriers_before,
        "strict recovery must never rewrite exact carrier bindings"
    );
    assert_eq!(
        invalid_carrier.header().height().get(),
        invalid.merge_qc.carrier_height,
        "invalid QC fixture must remain bound to its exact carrier height"
    );
}
state_test! { sync merge_ledger_cache_reconfigures_capacity
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new(World::default(), kura, query);
    state.set_merge_ledger_cache_capacity(1);
    let (candidate, keypairs, _) = record_commit_ready_merge_candidate_with_lanes(&mut state, 1, 1);
    let qc = merge_qc_for_candidate(&state, &candidate, &keypairs, &[0]);
    state
        .commit_merge_entry(merge_entry_from_candidate(candidate, qc))
        .expect("commit first");
    let (candidate, keypairs, _) = record_commit_ready_merge_candidate_with_lanes(&mut state, 1, 2);
    let qc = merge_qc_for_candidate(&state, &candidate, &keypairs, &[0]);
    state
        .commit_merge_entry(merge_entry_from_candidate(candidate, qc))
        .expect("commit second");
    let snapshot = state.merge_ledger().snapshot();
    assert_eq!(snapshot.len(), 1);
    assert_eq!(snapshot[0].epoch_id, 2);
    state.set_merge_ledger_cache_capacity(0);
    assert_eq!(
        state.merge_ledger().cache_capacity(),
        MergeLedgerStore::DEFAULT_CACHE_DEPTH
    );
}
