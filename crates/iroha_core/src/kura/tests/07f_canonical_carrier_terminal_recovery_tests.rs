#[test]
fn canonical_carrier_terminal_recovery_materializes_and_partitions_the_full_lane_set() {
    let temp_dir = TempDir::new().expect("canonical terminal temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let local_peer = PeerId::new(signer.public_key().clone());
    let height_context_id = HeightContextId(HashOf::<HeightContext>::from_untyped_unchecked(
        Hash::new(b"kura-canonical-terminal-height-context"),
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
                u8::try_from(index + 1).expect("fixture lane salt fits u8"),
            )
        })
        .collect::<Vec<_>>();
    assert_ne!(
        payloads[0].reservation_keys[0].signed_transaction_hash,
        payloads[1].reservation_keys[0].signed_transaction_hash,
        "carrier members require distinct transaction identities",
    );
    let chain_id_hash = payloads[0].chain_id_hash;
    let epoch = payloads[0].epoch;
    let (kura, _) = Kura::new(&config, &lane_config).expect("canonical terminal Kura");
    kura.bind_local_peer_id(local_peer.clone())
        .expect("bind canonical terminal local peer");
    let generation = kura
        .claim_autonomous_lifecycle_process_generation(chain_id_hash, &local_peer)
        .expect("claim canonical terminal process generation");

    let mut executions = Vec::new();
    let mut groups = Vec::new();
    let mut outcome_paths = Vec::new();
    executions
        .try_reserve_exact(payloads.len())
        .expect("reserve canonical terminal executions");
    groups
        .try_reserve_exact(payloads.len())
        .expect("reserve canonical terminal groups");
    outcome_paths
        .try_reserve_exact(payloads.len())
        .expect("reserve canonical terminal paths");
    for (lane, payload) in lanes.iter().zip(&payloads) {
        install_autonomous_lane_marker_for_kura(&kura, &lane_config, payload);
        executions.push(canonical_terminal_merge_execution_for_test(
            &kura, payload, &signer,
        ));
        let (_, group) = install_live_lifecycle_cursor_for_terminal_test(
            &kura,
            &generation,
            payload,
            height_context_id,
            &signer,
        );
        groups.push(group);
        outcome_paths.push(Kura::autonomous_lifecycle_terminal_outcome_path_for_entry(
            lane,
            temp_dir.path(),
            payload.origin_proposal.descriptor.lane_block_height,
            payload.origin_proposal.descriptor.proposal_height,
        ));
    }

    let mut blocks = DummyBlocks::new();
    let parent = blocks.next();
    let raw_carrier = blocks.next();
    let entrypoint_count = executions
        .iter()
        .try_fold(0_u64, |count, execution| {
            count.checked_add(u64::try_from(execution.entrypoints.len()).ok()?)
        })
        .expect("canonical terminal entrypoint count fits u64");
    let base_state_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"canonical terminal base state"));
    let write_set_root = Hash::new(b"canonical terminal write set");
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
            .expect("canonical terminal carrier has entrypoints"),
        result_merkle_root: crate::merge::merge_execution_result_merkle_root(&executions)
            .expect("canonical terminal carrier has results"),
        lanes: executions,
        application_write_set_root: Hash::new(b"canonical terminal application writes"),
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
    assert_eq!(
        merge_entry
            .execution_batch
            .as_ref()
            .expect("canonical terminal execution batch")
            .application_block_header,
        crate::merge::merge_application_header_from_carrier(&carrier.header()),
    );
    let carrier_height = carrier.header().height().get();
    let carrier_hash = carrier.hash();
    kura.store_block(parent)
        .expect("store canonical terminal carrier parent");
    kura.store_block_with_merge_entry(Arc::clone(&carrier), &merge_entry)
        .expect("store canonical terminal merge carrier");
    let _ = persist_v2_finality_chain_through(
        &kura,
        NonZeroUsize::new(usize::try_from(carrier_height).expect("carrier height fits usize"))
            .expect("carrier height is non-zero"),
    );
    kura.persist_merge_lane_block_application_receipts(&merge_entry, carrier_height, carrier_hash)
        .expect("persist canonical terminal application receipts");

    assert!(
        outcome_paths.iter().all(|path| !path.exists()),
        "zero-file crash boundary starts without a terminal outcome seed",
    );
    let publication = kura
        .reconstruct_autonomous_lifecycle_canonical_carrier_source_outcomes_for_group(&groups[0])
        .expect("reconstruct zero-file canonical carrier source-outcome set");
    assert_eq!(
        publication.entry_hash(),
        crate::merge::merge_ledger_entry_hash(&merge_entry),
    );
    assert_eq!(
        publication
            .consume_for_v2_apply(&merge_entry)
            .expect("consume exact reconstructed carrier publication")
            .len(),
        2,
    );
    assert!(outcome_paths.iter().all(|path| path.is_file()));

    let initial_recoveries = kura
        .pending_autonomous_lifecycle_terminal_outcome_inventory()
        .expect("inventory complete Pending carrier handoff");
    assert_eq!(initial_recoveries.len(), 1);
    let expected_groups = initial_recoveries[0]
        .pending_reservation_groups()
        .expect("complete Pending carrier exposes exact expected groups");
    assert_eq!(expected_groups.len(), 2);
    let initial_stages = kura
        .verify_expected_autonomous_lifecycle_terminal_outcome_stages(
            chain_id_hash,
            &expected_groups,
        )
        .expect("directly prove both Pending carrier members");
    assert!(initial_stages.iter().all(|stage| {
        stage.source_kind() == AutonomousLifecycleTerminalOutcomeSourceKind::CanonicalCarrier
            && stage.stage() == AutonomousLifecycleTerminalOutcomeDurableStage::Pending
    }));

    fs::remove_file(&outcome_paths[1]).expect("remove strict-prefix second outcome");
    assert!(
        kura.verify_expected_autonomous_lifecycle_terminal_outcome_stages(
            chain_id_hash,
            &expected_groups,
        )
        .is_err(),
        "direct expected-stage proof must reject a deleted handoff member without reconstruction",
    );
    let prefix_recoveries = kura
        .pending_autonomous_lifecycle_terminal_outcome_inventory()
        .expect("strict-prefix inventory reconstructs every missing carrier member");
    assert_eq!(prefix_recoveries.len(), 1);
    assert_eq!(prefix_recoveries[0].pending_outcome_count(), 2);
    assert_eq!(prefix_recoveries[0].route_identities().len(), 2);
    assert!(outcome_paths[1].is_file());

    let second_bytes = fs::read(&outcome_paths[1]).expect("read second Pending outcome");
    fs::write(&outcome_paths[1], [0xFF]).expect("corrupt later carrier member");
    assert!(
        kura.pending_autonomous_lifecycle_terminal_outcome_inventory()
            .is_err(),
        "a malformed later carrier member must prevent every recovery token from returning",
    );
    fs::write(&outcome_paths[1], &second_bytes).expect("restore second Pending outcome");

    let first_bytes = fs::read(&outcome_paths[0]).expect("read first Pending outcome");
    let first_pending =
        Kura::decode_autonomous_lifecycle_terminal_outcome(&outcome_paths[0], &first_bytes)
            .expect("decode first Pending outcome");
    kura.complete_autonomous_lifecycle_terminal_outcome(
        groups[0],
        canonical_terminal_projection_for_test(groups[0]),
        true,
        first_pending.outcome_hash,
    )
    .expect("complete first canonical carrier member");
    let mixed = kura
        .pending_autonomous_lifecycle_terminal_outcome_inventory()
        .expect("inventory mixed Pending and Complete carrier members");
    assert_eq!(mixed.len(), 1);
    assert_eq!(mixed[0].pending_outcome_count(), 1);
    assert_eq!(mixed[0].route_identities().len(), 2);
    let pending_groups = mixed[0]
        .pending_reservation_groups()
        .expect("mixed carrier exposes its exact Pending group");
    assert_eq!(pending_groups.len(), 1);
    assert_eq!(pending_groups[0].binding(), groups[1]);
    assert_eq!(
        pending_groups[0].ordered_keys(),
        payloads[1].reservation_keys.as_slice(),
    );
    let mixed_stages = kura
        .verify_expected_autonomous_lifecycle_terminal_outcome_stages(
            chain_id_hash,
            &expected_groups,
        )
        .expect("directly prove mixed Complete/Pending carrier stages");
    assert_eq!(
        mixed_stages
            .iter()
            .map(|stage| stage.stage())
            .collect::<Vec<_>>(),
        vec![
            AutonomousLifecycleTerminalOutcomeDurableStage::Complete,
            AutonomousLifecycleTerminalOutcomeDurableStage::Pending,
        ],
    );
    let AutonomousLifecyclePendingTerminalOutcomeRecovery::Canonical(recovery) =
        mixed.into_iter().next().expect("mixed carrier recovery")
    else {
        panic!("mixed carrier recovery must remain canonical")
    };
    let (pending, complete, _, recovered_entry, _, _, recovered_chain) = recovery
        .consume_for_v2_apply()
        .expect("consume mixed carrier recovery partition");
    assert_eq!(pending.len(), 1);
    assert_eq!(complete.len(), 1);
    assert_eq!(recovered_entry, merge_entry);
    assert_eq!(recovered_chain, chain_id_hash);
    assert!(
        Kura::decode_autonomous_lifecycle_terminal_outcome(
            &outcome_paths[0],
            &fs::read(&outcome_paths[0]).expect("read Complete first outcome"),
        )
        .expect("decode Complete first outcome")
        .is_complete(),
    );

    let second_pending = Kura::decode_autonomous_lifecycle_terminal_outcome(
        &outcome_paths[1],
        &fs::read(&outcome_paths[1]).expect("read second Pending outcome for completion"),
    )
    .expect("decode second Pending outcome for completion");
    kura.complete_autonomous_lifecycle_terminal_outcome(
        groups[1],
        canonical_terminal_projection_for_test(groups[1]),
        true,
        second_pending.outcome_hash,
    )
    .expect("complete second canonical carrier member");
    let complete_stages = kura
        .verify_expected_autonomous_lifecycle_terminal_outcome_stages(
            chain_id_hash,
            &expected_groups,
        )
        .expect("directly prove both completed carrier members");
    assert!(complete_stages.iter().all(|stage| {
        stage.source_kind() == AutonomousLifecycleTerminalOutcomeSourceKind::CanonicalCarrier
            && stage.stage() == AutonomousLifecycleTerminalOutcomeDurableStage::Complete
    }));
}
