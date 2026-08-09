struct CanonicalTerminalCapacityFixture {
    _temp_dir: TempDir,
    kura: Arc<Kura>,
    merge_entry: MergeLedgerEntry,
    carrier: Arc<SignedBlock>,
    outcome_paths: Vec<PathBuf>,
    route_identities: Vec<(LaneId, (u64, u64))>,
    reservation_groups: Vec<LaneQueueReservationGroupBindingV1>,
    pending_lengths: Vec<u64>,
    initial_disk_usage: u64,
    global_terminal_reservations: u64,
    reserved_post_wsv_bytes: u64,
    admitted_limit: u64,
}

fn assert_exact_canonical_terminal_publication(
    publication: AutonomousLifecycleCanonicalCarrierSourceOutcomePublication,
    entry: &MergeLedgerEntry,
    expected_groups: &[LaneQueueReservationGroupBindingV1],
) {
    let queue_authorizations = publication
        .consume_for_v2_apply(entry)
        .expect("capacity publication authenticates its exact committed carrier");
    assert_eq!(queue_authorizations.len(), expected_groups.len());
    for ((group, authorization), expected_group) in
        queue_authorizations.into_iter().zip(expected_groups)
    {
        assert_eq!(group, *expected_group);
        let (authorized_group, ordered_keys, source_outcome_hash) = authorization
            .consume_for_queue()
            .expect("capacity publication contains an exact Queue source authorization");
        assert_eq!(authorized_group, group);
        assert_eq!(
            lane_queue_reservation_group_binding_from_ordered_keys(ordered_keys.iter())
                .expect("capacity publication preserves FIFO reservation-key order"),
            group,
        );
        assert!(
            source_outcome_hash.as_ref().iter().any(|byte| *byte != 0),
            "capacity publication must bind a non-zero durable source-outcome hash",
        );
    }
}

fn canonical_terminal_capacity_fixture() -> CanonicalTerminalCapacityFixture {
    let temp_dir = TempDir::new().expect("canonical terminal capacity temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let local_peer = PeerId::new(signer.public_key().clone());
    let height_context_id = HeightContextId(HashOf::<HeightContext>::from_untyped_unchecked(
        Hash::new(b"kura-canonical-terminal-capacity-context"),
    ));
    let lanes = [
        lane_config.primary(),
        lane_config.entry(LaneId::new(1)).expect("lane one"),
    ];
    let payloads = lanes
        .iter()
        .enumerate()
        .map(|(index, lane)| {
            canonical_terminal_payload_for_test(
                lane,
                height_context_id,
                &signer,
                u8::try_from(index + 11).expect("capacity fixture salt fits u8"),
            )
        })
        .collect::<Vec<_>>();
    let chain_id_hash = payloads[0].chain_id_hash;
    let epoch = payloads[0].epoch;
    let (mut kura, _) = Kura::new(&config, &lane_config).expect("canonical terminal capacity Kura");
    kura.bind_local_peer_id(local_peer.clone())
        .expect("bind canonical terminal capacity peer");
    let generation = kura
        .claim_autonomous_lifecycle_process_generation(chain_id_hash, &local_peer)
        .expect("claim canonical terminal capacity generation");

    let mut executions = Vec::new();
    let mut bindings = Vec::new();
    let mut reservation_groups = Vec::new();
    let mut outcome_paths = Vec::new();
    let mut route_identities = Vec::new();
    for (lane, payload) in lanes.iter().zip(&payloads) {
        install_autonomous_lane_marker_for_kura(&kura, &lane_config, payload);
        executions.push(canonical_terminal_merge_execution_for_test(
            &kura, payload, &signer,
        ));
        let (binding, reservation_group) = install_live_lifecycle_cursor_for_terminal_test(
            &kura,
            &generation,
            payload,
            height_context_id,
            &signer,
        );
        bindings.push(binding);
        reservation_groups.push(reservation_group);
        let descriptor = &payload.origin_proposal.descriptor;
        route_identities.push((
            descriptor.lane_id,
            (descriptor.lane_block_height, descriptor.proposal_height),
        ));
        outcome_paths.push(Kura::autonomous_lifecycle_terminal_outcome_path_for_entry(
            lane,
            temp_dir.path(),
            descriptor.lane_block_height,
            descriptor.proposal_height,
        ));
    }

    let mut blocks = DummyBlocks::new();
    let parent = blocks.next();
    let raw_carrier = blocks.next();
    let entrypoint_count = executions
        .iter()
        .map(|execution| u64::try_from(execution.entrypoints.len()).expect("entrypoint count fits"))
        .sum();
    let base_state_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"terminal capacity base state"));
    let write_set_root = Hash::new(b"terminal capacity write set");
    let mut batch = MergeExecutionBatch {
        version: 1,
        base_state_height: 1,
        base_state_hash,
        application_block_header: crate::merge::merge_application_header_from_carrier(
            &raw_carrier.header(),
        ),
        execution_root: crate::merge::merge_execution_root(&executions),
        entrypoint_count,
        entrypoint_merkle_root: crate::merge::merge_execution_entrypoint_merkle_root(&executions)
            .expect("capacity carrier has entrypoints"),
        result_merkle_root: crate::merge::merge_execution_result_merkle_root(&executions)
            .expect("capacity carrier has results"),
        lanes: executions,
        application_write_set_root: Hash::new(b"terminal capacity application writes"),
        write_set_root,
        expected_post_state_hash: crate::merge::merge_expected_post_state_hash(
            1,
            base_state_hash,
            write_set_root,
        ),
        batch_hash: Hash::prehashed([0; Hash::LENGTH]),
    };
    batch.batch_hash = crate::merge::merge_execution_batch_hash(&batch);
    let mut merge_entry = sample_merge_entry(epoch);
    merge_entry.epoch_id = epoch;
    merge_entry.execution_batch = Some(batch);
    let bound_carrier = bind_merge_entry_to_carrier(raw_carrier, &mut merge_entry);
    let mut executed_carrier = bound_carrier.as_ref().clone();
    attach_ok_results_to_block(&mut executed_carrier);
    let carrier = Arc::new(executed_carrier);
    let carrier_height = carrier.header().height().get();
    let carrier_hash = carrier.hash();
    kura.store_block(parent).expect("store capacity parent");
    let baseline_disk_usage = kura
        .refresh_disk_usage_bytes()
        .expect("measure pre-carrier capacity baseline");
    let global_terminal_reservations = kura
        .autonomous_global_terminal_outcome_reserved_bytes()
        .expect("account globally reserved terminal slots");
    let (persisted_count, unindexed_bytes) = kura
        .persisted_count_and_unindexed_bytes()
        .expect("measure pre-carrier durable frontier");
    let pending_canonical_bytes = kura
        .pending_block_bytes(persisted_count, unindexed_bytes)
        .expect("measure pre-carrier pending canonical bytes");
    let post_wsv_reservations = kura
        .post_wsv_lane_artifact_budget_reserved_bytes()
        .expect("measure pre-carrier post-WSV reservations");
    let certified_bundle_reservations = kura
        .certified_bundle_capacity_reserved_bytes()
        .expect("measure pre-carrier certified-bundle reservations");
    let carrier_required = kura
        .block_required_bytes_for_budget(carrier.as_ref(), Some(&merge_entry), u64::MAX)
        .expect("account exact carrier and post-WSV components");
    let merge_commit_required = kura
        .merge_commit_required_bytes(carrier.as_ref(), &merge_entry)
        .expect("account exact merge association");
    let association_stage_required = kura
        .canonical_association_stage_additional_bytes(carrier.as_ref(), Some(&merge_entry))
        .expect("account exact canonical association stage");
    let admitted_limit = baseline_disk_usage
        .checked_add(pending_canonical_bytes)
        .and_then(|bytes| bytes.checked_add(global_terminal_reservations))
        .and_then(|bytes| bytes.checked_add(post_wsv_reservations))
        .and_then(|bytes| bytes.checked_add(certified_bundle_reservations))
        .and_then(|bytes| {
            bytes.checked_add(Kura::canonical_prune_intent_maintenance_headroom_bytes())
        })
        .and_then(|bytes| bytes.checked_add(carrier_required))
        .and_then(|bytes| bytes.checked_add(merge_commit_required))
        .and_then(|bytes| bytes.checked_add(association_stage_required))
        .expect("original carrier capacity limit fits u64");
    Arc::get_mut(&mut kura)
        .expect("capacity Kura remains exclusive before carrier admission")
        .max_disk_usage_bytes = admitted_limit;
    kura.store_block_with_merge_entry(Arc::clone(&carrier), &merge_entry)
        .expect("store capacity carrier at its original exact limit");
    let _ = persist_v2_finality_chain_through(
        &kura,
        NonZeroUsize::new(usize::try_from(carrier_height).expect("carrier height fits"))
            .expect("carrier height is non-zero"),
    );
    kura.persist_merge_lane_block_application_receipts(&merge_entry, carrier_height, carrier_hash)
        .expect("persist capacity carrier receipts");

    let batch = merge_entry
        .execution_batch
        .as_ref()
        .expect("capacity fixture execution batch");
    let pending_lengths = bindings
        .iter()
        .zip(&batch.lanes)
        .map(|(binding, execution)| {
            let receipt = LaneBlockApplicationReceiptArtifact::new_merge_execution(
                &merge_entry,
                batch,
                execution,
                Kura::merge_lane_block_artifact(execution),
                carrier_height,
                carrier_hash,
            );
            let source = Kura::autonomous_lifecycle_terminal_source_from_merge_receipt(&receipt)
                .expect("derive capacity Pending source");
            let pending = AutonomousLifecycleTerminalOutcomeV1::pending(binding.clone(), source)
                .expect("construct capacity Pending outcome");
            u64::try_from(
                pending
                    .encode_framed()
                    .expect("encode capacity Pending outcome")
                    .len(),
            )
            .expect("capacity Pending length fits")
        })
        .collect::<Vec<_>>();
    let initial_disk_usage = kura
        .kura_disk_usage_bytes()
        .expect("read capacity fixture disk usage");
    let reserved_post_wsv_bytes = kura
        .post_wsv_lane_artifact_budget_reserved_bytes()
        .expect("read capacity fixture post-WSV envelope");
    assert_eq!(
        reserved_post_wsv_bytes,
        u64::try_from(BOUND_PROGRESS_APPEND_INTENT_MAX_BYTES)
            .expect("shared carrier transient fits u64"),
        "durable receipt/frontier components leave only the carrier shared transient",
    );
    CanonicalTerminalCapacityFixture {
        _temp_dir: temp_dir,
        kura,
        merge_entry,
        carrier,
        outcome_paths,
        route_identities,
        reservation_groups,
        pending_lengths,
        initial_disk_usage,
        global_terminal_reservations,
        reserved_post_wsv_bytes,
        admitted_limit,
    }
}

#[test]
fn canonical_pending_capacity_preflights_full_set_and_consumes_reserved_slots_idempotently() {
    let mut fixture = canonical_terminal_capacity_fixture();
    let budgets_before = fixture
        .route_identities
        .iter()
        .map(|(lane_id, identity)| {
            fixture
                .kura
                .autonomous_lifecycle_terminal_reservation_budget_for_tests(*lane_id, *identity)
                .expect("read pre-Pending reservation")
        })
        .collect::<Vec<_>>();
    assert!(budgets_before.iter().all(|budget| budget.0));
    assert!(fixture.outcome_paths.iter().all(|path| !path.exists()));

    let (persisted_count, unindexed_bytes) = fixture
        .kura
        .persisted_count_and_unindexed_bytes()
        .expect("measure pre-Pending durable frontier");
    let pending_canonical_bytes = fixture
        .kura
        .pending_block_bytes(persisted_count, unindexed_bytes)
        .expect("measure pre-Pending pending canonical bytes");
    let certified_bundle_reservations = fixture
        .kura
        .certified_bundle_capacity_reserved_bytes()
        .expect("measure pre-Pending certified-bundle reservations");
    let exact_steady_required = fixture
        .initial_disk_usage
        .checked_add(pending_canonical_bytes)
        .and_then(|bytes| bytes.checked_add(fixture.global_terminal_reservations))
        .and_then(|bytes| bytes.checked_add(fixture.reserved_post_wsv_bytes))
        .and_then(|bytes| bytes.checked_add(certified_bundle_reservations))
        .and_then(|bytes| {
            bytes.checked_add(Kura::canonical_prune_intent_maintenance_headroom_bytes())
        })
        .expect("exact pre-Pending steady capacity");
    assert!(
        exact_steady_required <= fixture.admitted_limit,
        "materialized receipt/frontier components must fit the original carrier admission limit",
    );
    Arc::get_mut(&mut fixture.kura)
        .expect("capacity fixture Kura is exclusive")
        .max_disk_usage_bytes = exact_steady_required - 1;
    let first_error = match fixture
        .kura
        .persist_autonomous_lifecycle_canonical_terminal_outcomes_pending(&fixture.merge_entry)
    {
        Ok(_) => panic!("one byte below steady reserved capacity must reject the full set"),
        Err(error) => error,
    };
    assert!(
        first_error
            .to_string()
            .contains("reserved terminal or carrier capacity")
    );
    assert!(fixture.outcome_paths.iter().all(|path| !path.exists()));

    Arc::get_mut(&mut fixture.kura)
        .expect("capacity fixture Kura remains exclusive")
        .max_disk_usage_bytes = fixture.admitted_limit;
    let publication = fixture
        .kura
        .persist_autonomous_lifecycle_canonical_terminal_outcomes_pending(&fixture.merge_entry)
        .expect("the original carrier admission limit admits the complete Pending set")
        .expect("execution-bearing carrier publishes Pending outcomes");
    assert_exact_canonical_terminal_publication(
        publication,
        &fixture.merge_entry,
        &fixture.reservation_groups,
    );
    assert!(fixture.outcome_paths.iter().all(|path| path.is_file()));

    let budgets_after = fixture
        .route_identities
        .iter()
        .zip(&fixture.pending_lengths)
        .zip(&budgets_before)
        .map(|(((lane_id, identity), pending_len), before)| {
            let after = fixture
                .kura
                .autonomous_lifecycle_terminal_reservation_budget_for_tests(*lane_id, *identity)
                .expect("read consumed Pending reservation");
            assert!(!after.0, "durable Pending consumes its conceptual slot");
            assert_eq!(after.1, before.1, "reservation consumption is file-neutral");
            assert_eq!(
                after.2,
                before.2 - AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_MAX_BYTES as u64 + pending_len,
                "exact Pending bytes replace the maximum conceptual reservation",
            );
            after
        })
        .collect::<Vec<_>>();
    let outcome_bytes = fixture
        .outcome_paths
        .iter()
        .map(|path| fs::read(path).expect("read durable capacity Pending"))
        .collect::<Vec<_>>();
    Arc::get_mut(&mut fixture.kura)
        .expect("capacity fixture Kura remains exclusive")
        .max_disk_usage_bytes = fixture.admitted_limit;
    let retry_publication = fixture
        .kura
        .persist_autonomous_lifecycle_canonical_terminal_outcomes_pending(&fixture.merge_entry)
        .expect("exact Pending retry remains admitted")
        .expect("execution-bearing retry returns publication");
    assert_exact_canonical_terminal_publication(
        retry_publication,
        &fixture.merge_entry,
        &fixture.reservation_groups,
    );
    for ((path, expected_bytes), ((lane_id, identity), expected_budget)) in fixture
        .outcome_paths
        .iter()
        .zip(&outcome_bytes)
        .zip(fixture.route_identities.iter().zip(&budgets_after))
    {
        assert_eq!(
            fs::read(path).expect("read retried Pending").as_slice(),
            expected_bytes.as_slice(),
        );
        assert_eq!(
            fixture
                .kura
                .autonomous_lifecycle_terminal_reservation_budget_for_tests(*lane_id, *identity)
                .expect("read idempotent reservation budget"),
            *expected_budget,
        );
    }
}

#[test]
fn canonical_complete_releases_shared_transient_and_store_retry_stutters() {
    let mut fixture = canonical_terminal_capacity_fixture();
    Arc::get_mut(&mut fixture.kura)
        .expect("complete fixture Kura is exclusive")
        .max_disk_usage_bytes = fixture.admitted_limit;
    let publication = fixture
        .kura
        .persist_autonomous_lifecycle_canonical_terminal_outcomes_pending(&fixture.merge_entry)
        .expect("publish exact canonical Pending set")
        .expect("execution carrier has a Pending set");
    assert_exact_canonical_terminal_publication(
        publication,
        &fixture.merge_entry,
        &fixture.reservation_groups,
    );
    for (path, group) in fixture
        .outcome_paths
        .iter()
        .zip(&fixture.reservation_groups)
    {
        let bytes = fs::read(path).expect("read exact canonical Pending");
        let pending = Kura::decode_autonomous_lifecycle_terminal_outcome(path, &bytes)
            .expect("decode exact canonical Pending");
        fixture
            .kura
            .complete_autonomous_lifecycle_terminal_outcome(
                *group,
                canonical_terminal_projection_for_test(*group),
                true,
                pending.outcome_hash,
            )
            .expect("Complete must fit the original carrier admission limit");
    }
    assert_eq!(
        fixture
            .kura
            .post_wsv_lane_artifact_budget_reserved_bytes()
            .expect("read pre-release carrier transient"),
        fixture.reserved_post_wsv_bytes,
        "the shared carrier transient remains until explicit all-Complete release",
    );
    fixture
        .kura
        .release_post_wsv_lane_artifact_budget_reservation(
            &fixture.merge_entry,
            fixture.carrier.header().height().get(),
            fixture.carrier.hash(),
        )
        .expect("all exact Complete members release the carrier envelope");
    assert_eq!(
        fixture
            .kura
            .post_wsv_lane_artifact_budget_reserved_bytes()
            .expect("read released carrier envelope"),
        0,
    );

    fixture
        .kura
        .post_wsv_lane_artifact_budget_reservations
        .lock()
        .clear();
    fixture
        .kura
        .store_block_with_merge_entry(Arc::clone(&fixture.carrier), &fixture.merge_entry)
        .expect("exact store retry authenticates all-Complete tombstones");
    assert_eq!(
        fixture
            .kura
            .post_wsv_lane_artifact_budget_reserved_bytes()
            .expect("read store-retry reservation state"),
        0,
        "all-Complete store retry must not reinstall a full or shared envelope",
    );
    assert!(
        fixture
            .kura
            .pending_autonomous_lifecycle_terminal_outcome_inventory()
            .expect("all-Complete inventory readback")
            .is_empty(),
    );
    assert_eq!(
        fixture
            .kura
            .post_wsv_lane_artifact_budget_reserved_bytes()
            .expect("read post-inventory reservation state"),
        0,
        "all-Complete recovery inventory must stutter without a reservation map",
    );
}

#[test]
fn retired_release_pending_and_complete_progress_at_the_original_exact_limit() {
    let temp_dir = TempDir::new().expect("release capacity temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane = lane_config.primary();
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let local_peer = PeerId::new(signer.public_key().clone());
    let height_context_id = HeightContextId(HashOf::<HeightContext>::from_untyped_unchecked(
        Hash::new(b"release-terminal-capacity-context"),
    ));
    let payload = canonical_terminal_payload_for_test(&lane, height_context_id, &signer, 0x51);
    let chain_id_hash = payload.chain_id_hash;
    let epoch = payload.epoch;
    let (mut kura, _) = Kura::new(&config, &lane_config).expect("release capacity Kura");
    kura.bind_local_peer_id(local_peer.clone())
        .expect("bind release capacity peer");
    let generation = kura
        .claim_autonomous_lifecycle_process_generation(chain_id_hash, &local_peer)
        .expect("claim release capacity generation");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    kura.persist_lane_executable_payload(&payload, chain_id_hash, epoch)
        .expect("persist release capacity payload");
    let (_, group) = install_live_lifecycle_cursor_for_terminal_test(
        &kura,
        &generation,
        &payload,
        height_context_id,
        &signer,
    );
    let retirement = AutonomousLaneSlotRetirementV1::from_payload(&payload);
    kura.persist_autonomous_lane_slot_retirement(&retirement, chain_id_hash, epoch)
        .expect("persist release capacity retirement");
    let barrier = retirement
        .queue_release_barrier()
        .expect("derive release capacity barrier");
    kura.finalize_autonomous_lane_slot_release(&retirement, &barrier, chain_id_hash, epoch)
        .expect("durably release exact Queue claims");

    let physical = kura
        .kura_disk_usage_bytes()
        .expect("measure pre-Pending release bytes");
    let global_reservations = kura
        .autonomous_global_terminal_outcome_reserved_bytes()
        .expect("measure release terminal slot and shared transient");
    let (persisted_count, unindexed_bytes) = kura
        .persisted_count_and_unindexed_bytes()
        .expect("measure release durable frontier");
    let pending_canonical_bytes = kura
        .pending_block_bytes(persisted_count, unindexed_bytes)
        .expect("measure release pending canonical bytes");
    let post_wsv_reservations = kura
        .post_wsv_lane_artifact_budget_reserved_bytes()
        .expect("measure release post-WSV reservations");
    let certified_bundle_reservations = kura
        .certified_bundle_capacity_reserved_bytes()
        .expect("measure release certified-bundle reservations");
    let exact_limit = physical
        .checked_add(pending_canonical_bytes)
        .and_then(|bytes| bytes.checked_add(global_reservations))
        .and_then(|bytes| bytes.checked_add(post_wsv_reservations))
        .and_then(|bytes| bytes.checked_add(certified_bundle_reservations))
        .and_then(|bytes| {
            bytes.checked_add(Kura::canonical_prune_intent_maintenance_headroom_bytes())
        })
        .expect("release exact limit fits u64");
    Arc::get_mut(&mut kura)
        .expect("release Kura remains exclusive")
        .max_disk_usage_bytes = exact_limit - 1;
    assert!(
        kura.persist_autonomous_lifecycle_release_terminal_outcome_pending(
            &retirement,
            chain_id_hash,
            epoch,
        )
        .is_err(),
        "one byte below the admitted global slot must reject before Pending",
    );
    let path = Kura::autonomous_lifecycle_terminal_outcome_path_for_entry(
        &lane,
        temp_dir.path(),
        payload.origin_proposal.descriptor.lane_block_height,
        payload.origin_proposal.descriptor.proposal_height,
    );
    assert!(!path.exists());

    Arc::get_mut(&mut kura)
        .expect("release Kura remains exclusive after rejection")
        .max_disk_usage_bytes = exact_limit;
    let source_authorization = kura
        .persist_autonomous_lifecycle_release_terminal_outcome_pending(
            &retirement,
            chain_id_hash,
            epoch,
        )
        .expect("release Pending fits its original exact reservation");
    let pending_bytes = fs::read(&path).expect("read exact release Pending");
    let pending = Kura::decode_autonomous_lifecycle_terminal_outcome(&path, &pending_bytes)
        .expect("decode release Pending");
    assert_eq!(
        source_authorization.consume_for_queue(&barrier),
        Some(pending.outcome_hash),
        "the move-only Queue authorization must name the exact durable Pending outcome",
    );
    kura.complete_autonomous_lifecycle_terminal_outcome(
        group,
        release_terminal_projection_for_test(&kura, &payload, &retirement, &barrier),
        false,
        pending.outcome_hash,
    )
    .expect("release Complete fits the same original exact limit");
    let complete_bytes = fs::read(&path).expect("read exact release Complete");
    assert_eq!(complete_bytes.len(), pending_bytes.len());
    assert_eq!(
        kura.autonomous_global_terminal_outcome_reserved_bytes()
            .expect("read post-Complete global reservation"),
        0,
        "Complete consumes the final global stable slot and shared CAS transient",
    );
}
