#[test]
fn autonomous_claim_initial_staging_preflights_the_whole_named_temp_set() {
    let temp_dir = TempDir::new().expect("claim staging capacity temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (_, _, payload) = two_reservation_autonomous_lane_payload_for_kura(
        lane.lane_id,
        lane.dataspace_id,
        1,
        &signer,
    );
    let (mut kura, _) = Kura::new(&config, &lane_config).expect("claim staging Kura");
    let staged_bytes = payload
        .entrypoint_hashes
        .iter()
        .map(|entrypoint_hash| {
            u64::try_from(
                norito::encode_canonical(&AutonomousLaneEntrypointClaimV3::new(
                    &payload,
                    *entrypoint_hash,
                ))
                .expect("encode staged claim")
                .len(),
            )
            .expect("staged claim length fits u64")
        })
        .try_fold(0_u64, u64::checked_add)
        .expect("staged claim set length fits u64");
    let used = kura
        .refresh_disk_usage_bytes()
        .expect("measure staging baseline");
    let (persisted_count, unindexed_bytes) = kura
        .persisted_count_and_unindexed_bytes()
        .expect("measure staging durable frontier");
    let pending_canonical_bytes = kura
        .pending_block_bytes(persisted_count, unindexed_bytes)
        .expect("measure staging pending canonical bytes");
    let terminal = kura
        .autonomous_global_terminal_outcome_reserved_bytes()
        .expect("measure terminal reservations");
    let post_wsv = kura
        .post_wsv_lane_artifact_budget_reserved_bytes()
        .expect("measure post-WSV reservations");
    let certified = kura
        .certified_bundle_capacity_reserved_bytes()
        .expect("measure certified-bundle reservations");
    let exact_peak = used
        .checked_add(pending_canonical_bytes)
        .and_then(|bytes| bytes.checked_add(terminal))
        .and_then(|bytes| bytes.checked_add(post_wsv))
        .and_then(|bytes| bytes.checked_add(certified))
        .and_then(|bytes| {
            bytes.checked_add(Kura::canonical_prune_intent_maintenance_headroom_bytes())
        })
        .and_then(|bytes| bytes.checked_add(staged_bytes))
        .expect("claim staging peak fits u64");
    Arc::get_mut(&mut kura)
        .expect("exclusive staging Kura")
        .max_disk_usage_bytes = exact_peak - 1;
    let claim_paths = payload
        .entrypoint_hashes
        .iter()
        .map(|entrypoint_hash| {
            Kura::autonomous_lane_entrypoint_claim_path(
                temp_dir.path(),
                &payload.network_id,
                entrypoint_hash,
            )
        })
        .collect::<Vec<_>>();
    let error = {
        let _prune_guard = kura.prune_lock.lock();
        let _canonical_chain_guard = kura.canonical_chain_lock.lock();
        let pending_canonical_bytes = kura
            .pending_canonical_capacity_bytes_under_prune_and_canonical_guards()
            .expect("snapshot staged-claim pending canonical bytes");
        let _geometry_guard = kura.lane_geometry_lock.lock();
        let _sidecar_guard = kura.sidecar_lock.lock();
        kura.prepare_autonomous_lane_entrypoint_claims_locked(pending_canonical_bytes, &payload)
            .expect_err("one byte below the whole staged set must reject")
    };
    assert!(matches!(error, Error::IO(_, _)));
    for path in &claim_paths {
        assert!(!path.exists());
        assert!(!Kura::autonomous_lane_entrypoint_claim_temp_path(path).exists());
        assert!(
            !path.parent().expect("claim path has a shard").exists(),
            "whole-set capacity rejection must not create a claim shard",
        );
    }
    assert_eq!(
        kura.kura_disk_usage_bytes()
            .expect("measure rejected staging"),
        used,
        "whole-set capacity rejection must precede the first shard or claim mutation",
    );
}
#[test]
fn autonomous_claim_release_cas_preflights_promotions_removals_and_atomic_peak() {
    let temp_dir = TempDir::new().expect("claim release capacity temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, payload) = two_reservation_autonomous_lane_payload_for_kura(
        lane.lane_id,
        lane.dataspace_id,
        1,
        &signer,
    );
    let (mut kura, _) = Kura::new(&config, &lane_config).expect("claim release Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    kura.persist_lane_executable_payload(&payload, network_id, epoch)
        .expect("persist release payload");
    let retirement = AutonomousLaneSlotRetirementV1::from_payload(&payload);
    let retirement_hash = retirement.digest().expect("retirement digest");
    let claim_paths = payload
        .entrypoint_hashes
        .iter()
        .map(|entrypoint_hash| {
            Kura::autonomous_lane_entrypoint_claim_path(
                temp_dir.path(),
                &network_id,
                entrypoint_hash,
            )
        })
        .collect::<Vec<_>>();
    let temp_paths = claim_paths
        .iter()
        .map(|path| Kura::autonomous_lane_entrypoint_claim_temp_path(path))
        .collect::<Vec<_>>();
    // Exercise both recovery forms in the same bounded group: the first owner
    // exists only as a named temp and the second has a redundant exact temp.
    fs::rename(&claim_paths[0], &temp_paths[0]).expect("stage first exact owner");
    fs::copy(&claim_paths[1], &temp_paths[1]).expect("stage redundant second owner");
    kura.refresh_disk_usage_bytes()
        .expect("refresh claim temp accounting");
    let main_before = claim_paths
        .iter()
        .map(|path| fs::read(path).ok())
        .collect::<Vec<_>>();
    let temp_before = temp_paths
        .iter()
        .map(|path| fs::read(path).expect("read staged claim"))
        .collect::<Vec<_>>();
    let mut current_delta = 0_i128;
    let mut peak_delta = 0_i128;
    for (index, (_path, entrypoint_hash)) in claim_paths
        .iter()
        .zip(&payload.entrypoint_hashes)
        .enumerate()
    {
        let named_temp_len = u64::try_from(temp_before[index].len()).expect("temp length fits");
        let main_len = main_before[index].as_ref().map_or(0, |bytes| {
            u64::try_from(bytes.len()).expect("main length fits")
        });
        if index == 0 {
            current_delta -= i128::from(main_len);
        } else {
            current_delta -= i128::from(named_temp_len);
        }
        let replacement = AutonomousLaneEntrypointClaimV3::release_pending_for_payload(
            &payload,
            *entrypoint_hash,
            retirement_hash,
        );
        let replacement_len = u64::try_from(
            norito::encode_canonical(&replacement)
                .expect("encode ReleasePending claim")
                .len(),
        )
        .expect("replacement length fits");
        let stable_len = if index == 0 { named_temp_len } else { main_len };
        current_delta += i128::from(replacement_len);
        peak_delta = peak_delta.max(current_delta);
        current_delta -= i128::from(stable_len);
    }
    let release_peak = u64::try_from(peak_delta).expect("release peak fits u64");
    let used = kura
        .kura_disk_usage_bytes()
        .expect("measure release baseline");
    let (persisted_count, unindexed_bytes) = kura
        .persisted_count_and_unindexed_bytes()
        .expect("measure release durable frontier");
    let pending_canonical_bytes = kura
        .pending_block_bytes(persisted_count, unindexed_bytes)
        .expect("measure release pending canonical bytes");
    let terminal = kura
        .autonomous_global_terminal_outcome_reserved_bytes()
        .expect("measure terminal reservations");
    let post_wsv = kura
        .post_wsv_lane_artifact_budget_reserved_bytes()
        .expect("measure post-WSV reservations");
    let certified = kura
        .certified_bundle_capacity_reserved_bytes()
        .expect("measure certified-bundle reservations");
    let exact_peak = used
        .checked_add(pending_canonical_bytes)
        .and_then(|bytes| bytes.checked_add(terminal))
        .and_then(|bytes| bytes.checked_add(post_wsv))
        .and_then(|bytes| bytes.checked_add(certified))
        .and_then(|bytes| {
            bytes.checked_add(Kura::canonical_prune_intent_maintenance_headroom_bytes())
        })
        .and_then(|bytes| bytes.checked_add(release_peak))
        .expect("claim release peak fits u64");
    Arc::get_mut(&mut kura)
        .expect("exclusive release Kura")
        .max_disk_usage_bytes = exact_peak - 1;
    let error = {
        let _prune_guard = kura.prune_lock.lock();
        let _canonical_chain_guard = kura.canonical_chain_lock.lock();
        let pending_canonical_bytes = kura
            .pending_canonical_capacity_bytes_under_prune_and_canonical_guards()
            .expect("snapshot claim-release pending canonical bytes");
        let _geometry_guard = kura.lane_geometry_lock.lock();
        let _sidecar_guard = kura.sidecar_lock.lock();
        kura.prepare_autonomous_lane_entrypoint_claim_release_locked(
            pending_canonical_bytes,
            &payload,
            &retirement,
        )
        .expect_err("one byte below the ordered release CAS peak must reject")
    };
    assert!(matches!(error, Error::IO(_, _)));
    assert_eq!(
        claim_paths
            .iter()
            .map(|path| fs::read(path).ok())
            .collect::<Vec<_>>(),
        main_before,
        "capacity rejection must not promote, replace, or create a main claim",
    );
    assert_eq!(
        temp_paths
            .iter()
            .map(|path| fs::read(path).expect("read retained staged claim"))
            .collect::<Vec<_>>(),
        temp_before,
        "capacity rejection must not remove or rewrite a named claim temp",
    );
}
