#[test]
fn eviction_requires_distinct_matching_replica_adverts() {
    let temp_dir = TempDir::new().unwrap();
    populate_store(&temp_dir, 4);
    let config = kura_config_for_dir(&temp_dir, NonZeroUsize::new(1).expect("non-zero"));
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura init");
    let height = nonzero!(2_usize);
    finalize_chain_through_for_eviction(&kura, height);
    let (block_hash, payload_len) = advertised_block_metadata(&kura, height);
    let wrong_hash = DummyBlocks::new().next().hash();
    assert_ne!(wrong_hash, block_hash);
    let repeated_peer = checked_peer_id();
    for _ in 0..EVICTION_REQUIRED_REPLICAS.get() {
        kura.record_block_replica_advert(
            repeated_peer.clone(),
            height.get() as u64,
            block_hash,
            payload_len,
        );
    }
    for _ in 0..EVICTION_REQUIRED_REPLICAS.get() {
        let peer = checked_peer_id();
        kura.record_block_replica_advert(
            peer,
            height.get() as u64,
            block_hash,
            payload_len.saturating_add(1),
        );
    }
    for _ in 0..EVICTION_REQUIRED_REPLICAS.get() {
        let peer = checked_peer_id();
        kura.record_block_replica_advert(peer, height.get() as u64, wrong_hash, payload_len);
    }
    let freed = kura
        .evict_block_bodies(payload_len)
        .expect("attempt eviction with bad adverts");
    assert_eq!(
        freed, 0,
        "duplicate peers, wrong hashes, and wrong lengths must not satisfy replica quorum"
    );
    let index = {
        let mut store = kura.block_store.lock();
        store.read_block_index(1).expect("block index")
    };
    assert!(!index.is_evicted());
    assert_eq!(
        kura.advertise_required_replicas_for_bench(height),
        Some(payload_len)
    );
    let freed = kura
        .evict_block_bodies(payload_len)
        .expect("evict with enough matching adverts");
    assert!(freed >= payload_len);
    let index = {
        let mut store = kura.block_store.lock();
        store.read_block_index(1).expect("block index")
    };
    assert!(index.is_evicted());
}
#[test]
fn deterministic_commit_qc_keepers_use_f_plus_one_and_pin_a_local_keeper() {
    let temp_dir = TempDir::new().unwrap();
    populate_store(&temp_dir, 4);
    let config = kura_config_for_dir(&temp_dir, NonZeroUsize::new(1).expect("non-zero"));
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura init");
    let height = nonzero!(2_usize);
    finalize_chain_through_for_eviction(&kura, height);
    let artifact = kura
        .v2_finality_artifact(height.get() as u64)
        .expect("read exact fixture finality")
        .expect("fixture finality exists");
    let first = kura.deterministic_kura_replica_keepers(&artifact);
    let second = kura.deterministic_kura_replica_keepers(&artifact);
    assert_eq!(first, second, "keeper selection must be deterministic");
    let quorum = usize::try_from(artifact.height_context.quorum.min_signers)
        .expect("fixture quorum count fits usize");
    let expected = artifact
        .height_context
        .roster
        .len()
        .checked_sub(quorum)
        .expect("valid quorum does not exceed roster")
        .saturating_add(1)
        .max(config.replica_advert.eviction_required_replicas.get());
    assert_eq!(first.len(), expected);
    assert!(first.iter().all(|(index, peer)| {
        artifact.commit_qc.signers.contains(index)
            && artifact
                .height_context
                .roster
                .get(usize::try_from(*index).expect("keeper index fits usize"))
                .is_some_and(|validator| &validator.validator == peer)
    }));
    let local_keeper = first
        .first()
        .expect("fixture must select at least one keeper")
        .1
        .clone();
    kura.bind_local_peer_id(local_keeper)
        .expect("bind selected local keeper");
    let (_, payload_len) = advertised_block_metadata(&kura, height);
    assert_eq!(
        kura.advertise_required_replicas_for_bench(height),
        Some(payload_len)
    );
    assert_eq!(
        kura.evict_block_bodies(payload_len)
            .expect("attempt eviction while local peer is a keeper"),
        0,
        "a selected local keeper must pin its exact body even with every advert present"
    );
}
#[test]
fn nonkeeper_replica_advert_probe_never_reads_the_complete_body() {
    let temp_dir = TempDir::new().unwrap();
    populate_store(&temp_dir, 4);
    let config = kura_config_for_dir(&temp_dir, NonZeroUsize::new(1).expect("non-zero"));
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura init");
    let height = nonzero!(2_usize);
    finalize_chain_through_for_eviction(&kura, height);
    let artifact = kura
        .v2_finality_artifact(height.get() as u64)
        .expect("read fixture finality")
        .expect("fixture finality exists");
    let selected = kura.deterministic_kura_replica_keepers(&artifact);
    let keys = v2_finality_fixture_keys();
    let nonkeeper_key = keys
        .iter()
        .find(|key| {
            let peer = PeerId::new(key.public_key().clone());
            selected.iter().all(|(_, selected)| selected != &peer)
        })
        .expect("fixture has a deterministic nonkeeper");
    kura.bind_local_peer_id(PeerId::new(nonkeeper_key.public_key().clone()))
        .expect("bind nonkeeper local peer");
    assert!(
        kura.probe_kura_replica_advert_source(height.get() as u64, nonkeeper_key)
            .expect("probe nonkeeper authority")
            .is_none()
    );
    assert_eq!(
        kura.kura_replica_advert_body_reads_for_tests(),
        0,
        "a nonkeeper height must advance without complete-body I/O"
    );
}
#[test]
fn selected_keeper_invalid_index_missing_body_and_corrupt_body_fail_closed() {
    let selected_fixture = || {
        let temp_dir = TempDir::new().expect("temporary selected-keeper Kura");
        populate_store(&temp_dir, 4);
        let config = kura_config_for_dir(&temp_dir, NonZeroUsize::new(1).expect("non-zero"));
        let (kura, _) = Kura::open_test_kura_with_configured_lane_config(
            &config,
            &RuntimeLaneConfig::default(),
        )
        .expect("kura init");
        let height = nonzero!(2_usize);
        finalize_chain_through_for_eviction(&kura, height);
        let artifact = kura
            .v2_finality_artifact(height.get() as u64)
            .expect("read fixture finality")
            .expect("fixture finality exists");
        let (keeper_index, keeper) = kura
            .deterministic_kura_replica_keepers(&artifact)
            .first()
            .expect("fixture selected keeper")
            .clone();
        let keys = v2_finality_fixture_keys();
        let keeper_key = keys[usize::try_from(keeper_index).expect("keeper index fits")].clone();
        assert_eq!(PeerId::new(keeper_key.public_key().clone()), keeper);
        kura.bind_local_peer_id(keeper)
            .expect("bind selected keeper");
        (temp_dir, kura, height, keeper_key)
    };
    let (_temp_dir, kura, height, keeper_key) = selected_fixture();
    let index = kura
        .block_store
        .lock()
        .read_block_index(
            u64::try_from(height.get().saturating_sub(1)).expect("height index fits u64"),
        )
        .expect("read selected body index");
    kura.block_store
        .lock()
        .write_block_index(
            u64::try_from(height.get().saturating_sub(1)).expect("height index fits u64"),
            index.start,
            0,
        )
        .expect("write invalid selected body index");
    assert!(matches!(
        kura.probe_kura_replica_advert_source(height.get() as u64, &keeper_key),
        Err(Error::InvalidKuraReplicaAdvert(_))
    ));
    assert_eq!(kura.kura_replica_advert_body_reads_for_tests(), 0);
    let (_temp_dir, kura, height, keeper_key) = selected_fixture();
    let source = kura
        .probe_kura_replica_advert_source(height.get() as u64, &keeper_key)
        .expect("probe selected keeper")
        .expect("selected keeper source");
    let index = kura
        .block_store
        .lock()
        .read_block_index(
            u64::try_from(height.get().saturating_sub(1)).expect("height index fits u64"),
        )
        .expect("read selected body index");
    kura.block_store
        .lock()
        .write_block_index(
            u64::try_from(height.get().saturating_sub(1)).expect("height index fits u64"),
            EVICTED_BLOCK_START,
            index.length,
        )
        .expect("remove selected keeper inline body");
    assert!(matches!(
        kura.build_signed_kura_replica_advert_from_source(&source, &keeper_key),
        Err(Error::InvalidKuraReplicaAdvert(_))
    ));
    assert_eq!(kura.kura_replica_advert_body_reads_for_tests(), 1);
    let (_temp_dir, kura, height, keeper_key) = selected_fixture();
    let source = kura
        .probe_kura_replica_advert_source(height.get() as u64, &keeper_key)
        .expect("probe selected keeper")
        .expect("selected keeper source");
    let index = kura
        .block_store
        .lock()
        .read_block_index(
            u64::try_from(height.get().saturating_sub(1)).expect("height index fits u64"),
        )
        .expect("read selected body index");
    kura.block_store
        .lock()
        .write_block_data(index.start, &[0xFF])
        .expect("corrupt selected keeper body");
    assert!(
        kura.build_signed_kura_replica_advert_from_source(&source, &keeper_key)
            .is_err(),
        "selected keeper corrupt body must be fatal"
    );
    assert_eq!(kura.kura_replica_advert_body_reads_for_tests(), 1);
}
#[test]
fn authenticated_replica_admission_rejects_forgery_non_qc_peer_and_alternate_finality() {
    let temp_dir = TempDir::new().unwrap();
    populate_store(&temp_dir, 4);
    let config = kura_config_for_dir(&temp_dir, NonZeroUsize::new(1).expect("non-zero"));
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura init");
    let height = nonzero!(2_usize);
    finalize_chain_through_for_eviction(&kura, height);
    let artifact = kura
        .v2_finality_artifact(height.get() as u64)
        .expect("read fixture finality")
        .expect("fixture finality exists");
    let selected = kura.deterministic_kura_replica_keepers(&artifact);
    let (keeper_index, keeper) = selected
        .first()
        .expect("fixture has a selected keeper")
        .clone();
    let keys = v2_finality_fixture_keys();
    let keeper_key = &keys[usize::try_from(keeper_index).expect("keeper index fits")];
    assert_eq!(PeerId::new(keeper_key.public_key().clone()), keeper);
    kura.bind_local_peer_id(keeper.clone())
        .expect("bind exact selected keeper");
    let source = kura
        .probe_kura_replica_advert_source(height.get() as u64, keeper_key)
        .expect("probe exact advert authority")
        .expect("selected keeper has an advert source");
    assert_eq!(source.height(), height.get() as u64);
    assert_eq!(
        kura.kura_replica_advert_body_reads_for_tests(),
        0,
        "keeper selection must precede every complete body read"
    );
    let advert = kura
        .build_signed_kura_replica_advert_from_source(&source, keeper_key)
        .expect("build exact advert");
    assert_eq!(
        kura.kura_replica_advert_body_reads_for_tests(),
        1,
        "initial selected-keeper build must read the complete body exactly once"
    );
    let expected_key = BlockReplicaKey {
        height: advert.height,
        block_hash: advert.block_hash,
        finality_artifact_hash: advert.finality_artifact_hash,
        executed_block_wire_len: advert.executed_block_wire_len,
        executed_block_wire_hash: advert.executed_block_wire_hash,
    };
    let conflicting_key = BlockReplicaKey {
        finality_artifact_hash: HashOf::from_untyped_unchecked(Hash::new(
            b"conflicting-process-local-finality",
        )),
        ..expected_key
    };
    kura.replica_registry.lock().insert(
        conflicting_key,
        BTreeMap::from([(
            keeper.clone(),
            BlockReplicaAdvert {
                keeper_index,
                observed_at: Instant::now(),
            },
        )]),
    );
    kura.admit_kura_replica_advert(&advert)
        .expect("exact advert is admitted");
    let same_height_keys = kura
        .replica_registry
        .lock()
        .keys()
        .filter(|key| key.height == advert.height)
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(
        same_height_keys,
        [expected_key],
        "authenticated admission must replace every conflicting process-local key at the same canonical height",
    );
    kura.revalidate_local_kura_replica_advert(&advert, keeper_key)
        .expect("exact retained advert revalidates");
    kura.revalidate_kura_replica_advert_source(&advert)
        .expect("exact retained advert has non-mutating rollover authority");
    let mut forged_wire = advert.clone();
    forged_wire.executed_block_wire_hash = Hash::new(b"forged-replica-wire");
    assert!(matches!(
        kura.admit_kura_replica_advert(&forged_wire),
        Err(Error::InvalidKuraReplicaAdvert(_))
    ));
    assert!(matches!(
        kura.revalidate_kura_replica_advert_source(&forged_wire),
        Err(Error::InvalidKuraReplicaAdvert(_))
    ));
    let non_qc_index = artifact
        .height_context
        .roster
        .iter()
        .enumerate()
        .find(|(index, _)| {
            u32::try_from(*index)
                .ok()
                .is_some_and(|index| !artifact.commit_qc.signers.contains(&index))
        })
        .map(|(index, _)| index)
        .expect("fixture has a non-QC roster member");
    let non_qc_key = &keys[non_qc_index];
    let mut non_qc = advert.clone();
    non_qc.keeper_index = u32::try_from(non_qc_index).expect("fixture index fits u32");
    non_qc.keeper = PeerId::new(non_qc_key.public_key().clone());
    non_qc.signature = Signature::new(non_qc_key.private_key(), &non_qc.signature_preimage())
        .payload()
        .to_vec();
    assert!(matches!(
        kura.admit_kura_replica_advert(&non_qc),
        Err(Error::InvalidKuraReplicaAdvert(_))
    ));
    let mut alternate_finality = advert;
    alternate_finality.finality_artifact_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"alternate-finality-identity"));
    alternate_finality.signature = Signature::new(
        keeper_key.private_key(),
        &alternate_finality.signature_preimage(),
    )
    .payload()
    .to_vec();
    assert!(matches!(
        kura.admit_kura_replica_advert(&alternate_finality),
        Err(Error::InvalidKuraReplicaAdvert(_))
    ));
}
#[test]
fn authenticated_replica_admission_rejects_outside_the_active_horizon_before_mutation() {
    let temp_dir = TempDir::new().unwrap();
    populate_store(&temp_dir, 4);
    let mut config = kura_config_for_dir(&temp_dir, NonZeroUsize::new(1).expect("non-zero"));
    config.replica_advert.evictable_window = NonZeroUsize::new(1).expect("non-zero");
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura init");
    let height = nonzero!(2_usize);
    finalize_chain_through_for_eviction(&kura, height);
    let artifact = kura
        .v2_finality_artifact(height.get() as u64)
        .expect("read fixture finality")
        .expect("fixture finality exists");
    let (keeper_index, keeper) = kura
        .deterministic_kura_replica_keepers(&artifact)
        .first()
        .expect("fixture has a selected keeper")
        .clone();
    let keys = v2_finality_fixture_keys();
    let keeper_key = &keys[usize::try_from(keeper_index).expect("keeper index fits")];
    kura.bind_local_peer_id(keeper.clone())
        .expect("bind exact selected keeper");
    let advert = kura
        .build_signed_kura_replica_advert(height.get() as u64, keeper_key)
        .expect("build exact advert")
        .expect("selected keeper has a durable body");
    let sentinel_key = BlockReplicaKey {
        height: 4,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"registry-sentinel-block")),
        finality_artifact_hash: HashOf::from_untyped_unchecked(Hash::new(
            b"registry-sentinel-finality",
        )),
        executed_block_wire_len: 1,
        executed_block_wire_hash: Hash::new(b"registry-sentinel-wire"),
    };
    kura.replica_registry.lock().insert(
        sentinel_key,
        BTreeMap::from([(
            keeper,
            BlockReplicaAdvert {
                keeper_index,
                observed_at: Instant::now(),
            },
        )]),
    );
    let before = kura
        .replica_registry
        .lock()
        .keys()
        .copied()
        .collect::<Vec<_>>();
    let error = kura
        .admit_kura_replica_advert(&advert)
        .expect_err("height two is outside the active 3..=4 horizon");
    assert!(
        matches!(error, Error::InvalidKuraReplicaAdvert(ref message) if message.contains("outside the active replica registry horizon 3..=4")),
        "{error:?}",
    );
    assert_eq!(
        kura.replica_registry
            .lock()
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        before,
        "rejected admissions must not prune or otherwise mutate the registry",
    );
}
#[test]
fn eviction_query_prunes_expired_and_out_of_horizon_replica_observations() {
    let temp_dir = TempDir::new().unwrap();
    populate_store(&temp_dir, 4);
    let mut config = kura_config_for_dir(&temp_dir, NonZeroUsize::new(1).expect("non-zero"));
    config.replica_advert.evictable_window = NonZeroUsize::new(1).expect("non-zero");
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura init");
    finalize_chain_through_for_eviction(&kura, nonzero!(2_usize));
    let peer = checked_peer_id();
    let fresh = Instant::now();
    let expired = fresh
        .checked_sub(kura.replica_advert_ttl() + Duration::from_secs(1))
        .expect("expired observation instant");
    let key_at = |height, label: &[u8]| BlockReplicaKey {
        height,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(label)),
        finality_artifact_hash: HashOf::from_untyped_unchecked(Hash::new(label)),
        executed_block_wire_len: 1,
        executed_block_wire_hash: Hash::new(label),
    };
    kura.replica_registry.lock().extend([
        (
            key_at(2, b"out-of-horizon"),
            BTreeMap::from([(
                peer.clone(),
                BlockReplicaAdvert {
                    keeper_index: 0,
                    observed_at: fresh,
                },
            )]),
        ),
        (
            key_at(3, b"expired-in-horizon"),
            BTreeMap::from([(
                peer,
                BlockReplicaAdvert {
                    keeper_index: 0,
                    observed_at: expired,
                },
            )]),
        ),
    ]);
    assert_eq!(
        kura.evict_block_bodies(1).expect("bounded eviction query"),
        0,
        "unverified observations must not authorize eviction",
    );
    assert!(
        kura.replica_registry.lock().is_empty(),
        "the query boundary must prune both expired and out-of-horizon observations",
    );
}
#[test]
fn replica_adverts_ignore_zero_height_and_payload_len() {
    let temp_dir = TempDir::new().unwrap();
    populate_store(&temp_dir, 4);
    let config = kura_config_for_dir(&temp_dir, NonZeroUsize::new(1).expect("non-zero"));
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura init");
    let height = nonzero!(2_usize);
    finalize_chain_through_for_eviction(&kura, height);
    let (block_hash, payload_len) = advertised_block_metadata(&kura, height);
    for _ in 0..EVICTION_REQUIRED_REPLICAS.get() {
        let peer = checked_peer_id();
        kura.record_block_replica_advert(peer, 0, block_hash, payload_len);
    }
    for _ in 0..EVICTION_REQUIRED_REPLICAS.get() {
        let peer = checked_peer_id();
        kura.record_block_replica_advert(peer, height.get() as u64, block_hash, 0);
    }
    let freed = kura
        .evict_block_bodies(payload_len)
        .expect("attempt eviction with ignored adverts");
    assert_eq!(freed, 0);
    let index = {
        let mut store = kura.block_store.lock();
        store.read_block_index(1).expect("block index")
    };
    assert!(
        !index.is_evicted(),
        "invalid replica adverts must not make a body evictable"
    );
}
#[test]
fn replica_registry_capacity_preserves_the_configured_evictable_height_window() {
    let tail = NonZeroUsize::new(8).expect("non-zero protected tail");
    let kura = Kura::blank_kura_for_testing_with_blocks_in_memory(tail);
    let capacity = kura.replica_registry_capacity();
    assert_eq!(
        capacity,
        tail.get() + kura.replica_advert_evictable_window().get(),
    );
    assert_eq!(
        capacity - kura.blocks_in_memory().get(),
        kura.replica_advert_evictable_window().get(),
        "the protected tail must not consume the body-evictable advert horizon",
    );
    let canonical_tip = u64::try_from(capacity + 37).expect("fixture tip fits u64");
    let (minimum_height, maximum_height) = kura.replica_registry_height_horizon(canonical_tip);
    assert_eq!(maximum_height, canonical_tip);
    assert_eq!(
        maximum_height - minimum_height + 1,
        u64::try_from(capacity).expect("registry capacity fits u64"),
    );
}
#[test]
fn invalid_replica_advert_runtime_geometry_fails_before_store_creation() {
    fn assert_rejected(config: KuraConfig, store_root: &Path, expected: &str) {
        let error = Kura::open_test_kura_with_configured_lane_config(
            &config,
            &RuntimeLaneConfig::default(),
        )
        .expect_err("invalid replica-advert runtime geometry must fail");
        assert!(
            matches!(
                error,
                Error::InvalidKuraReplicaAdvertConfiguration(ref message)
                    if message.contains(expected)
            ),
            "{error:?}",
        );
        assert!(
            !store_root.exists(),
            "invalid replica-advert configuration must fail before creating Kura storage",
        );
    }
    let parent = TempDir::new().expect("temporary parent");
    let ttl_root = parent.path().join("invalid-ttl");
    let mut invalid_ttl = kura_config_for_path(&ttl_root, BLOCKS_IN_MEMORY);
    invalid_ttl.replica_advert.ttl = Duration::from_nanos(1);
    assert_rejected(invalid_ttl, &ttl_root, "TTL 1ns is below the 2 ms minimum");
    let refresh_root = parent.path().join("invalid-refresh");
    let mut invalid_refresh = kura_config_for_path(&refresh_root, BLOCKS_IN_MEMORY);
    invalid_refresh.replica_advert.refresh_interval = Duration::ZERO;
    assert_rejected(
        invalid_refresh,
        &refresh_root,
        "refresh interval 0ns is below the 1 ms minimum",
    );
    let submillisecond_refresh_root = parent.path().join("submillisecond-refresh");
    let mut submillisecond_refresh =
        kura_config_for_path(&submillisecond_refresh_root, BLOCKS_IN_MEMORY);
    submillisecond_refresh.replica_advert.refresh_interval = Duration::from_nanos(1);
    assert_rejected(
        submillisecond_refresh,
        &submillisecond_refresh_root,
        "refresh interval 1ns is below the 1 ms minimum",
    );
    let minimum_refresh_root = parent.path().join("minimum-refresh");
    let mut minimum_refresh = kura_config_for_path(&minimum_refresh_root, BLOCKS_IN_MEMORY);
    minimum_refresh.replica_advert.refresh_interval = Duration::from_millis(1);
    let _ = Kura::open_test_kura_with_configured_lane_config(
        &minimum_refresh,
        &RuntimeLaneConfig::default(),
    )
    .expect("the exact one-millisecond refresh floor must be accepted");
    let floor_root = parent.path().join("invalid-floor");
    let mut invalid_floor = kura_config_for_path(&floor_root, BLOCKS_IN_MEMORY);
    invalid_floor.replica_advert.eviction_required_replicas = NonZeroUsize::new(
        iroha_config::parameters::actual::KURA_REPLICA_ADVERT_KEEPERS_PER_KEY_LIMIT + 1,
    )
    .expect("invalid floor remains non-zero");
    assert_rejected(
        invalid_floor,
        &floor_root,
        "eviction replica floor 129 exceeds the protocol validator limit 128",
    );
    let capacity_root = parent.path().join("invalid-capacity");
    let mut invalid_capacity = kura_config_for_path(&capacity_root, BLOCKS_IN_MEMORY);
    invalid_capacity.blocks_in_memory =
        NonZeroUsize::new(usize::MAX).expect("usize maximum is non-zero");
    invalid_capacity.replica_advert.evictable_window =
        NonZeroUsize::new(1).expect("one is non-zero");
    assert_rejected(
        invalid_capacity,
        &capacity_root,
        "plus replica-advert evictable window 1 exceeds",
    );
}
#[test]
fn nonselected_or_wrong_length_replica_observations_do_not_count() {
    let temp_dir = TempDir::new().unwrap();
    populate_store(&temp_dir, 4);
    let config = kura_config_for_dir(&temp_dir, NonZeroUsize::new(1).expect("non-zero"));
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura init");
    let height = nonzero!(2_usize);
    finalize_chain_through_for_eviction(&kura, height);
    let (block_hash, payload_len) = advertised_block_metadata(&kura, height);
    let overwritten_peer = checked_peer_id();
    kura.record_block_replica_advert(
        overwritten_peer.clone(),
        height.get() as u64,
        block_hash,
        payload_len,
    );
    kura.record_block_replica_advert(
        overwritten_peer,
        height.get() as u64,
        block_hash,
        payload_len.saturating_add(1),
    );
    for _ in 1..EVICTION_REQUIRED_REPLICAS.get() {
        let peer = checked_peer_id();
        kura.record_block_replica_advert(peer, height.get() as u64, block_hash, payload_len);
    }
    let freed = kura
        .evict_block_bodies(payload_len)
        .expect("attempt eviction with one overwritten peer");
    assert_eq!(
        freed, 0,
        "nonselected peers and wrong-length observations must not satisfy keeper authority"
    );
    assert_eq!(
        kura.advertise_required_replicas_for_bench(height),
        Some(payload_len)
    );
    let freed = kura
        .evict_block_bodies(payload_len)
        .expect("evict after restoring distinct matching quorum");
    assert!(freed >= payload_len);
}
#[test]
fn expired_replica_adverts_do_not_allow_eviction() {
    let temp_dir = TempDir::new().unwrap();
    populate_store(&temp_dir, 4);
    let config = kura_config_for_dir(&temp_dir, NonZeroUsize::new(1).expect("non-zero"));
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura init");
    let height = nonzero!(2_usize);
    let (block_hash, payload_len) = advertised_block_metadata(&kura, height);
    let expired_at = Instant::now()
        .checked_sub(kura.replica_advert_ttl() + Duration::from_secs(1))
        .expect("expired instant");
    {
        let mut registry = kura.replica_registry.lock();
        let key = BlockReplicaKey {
            height: height.get() as u64,
            block_hash,
            finality_artifact_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"expired-replica-finality",
            )),
            executed_block_wire_len: payload_len,
            executed_block_wire_hash: Hash::new(b"expired-replica-wire"),
        };
        for _ in 0..EVICTION_REQUIRED_REPLICAS.get() {
            let peer = checked_peer_id();
            registry.entry(key).or_default().insert(
                peer,
                BlockReplicaAdvert {
                    keeper_index: 0,
                    observed_at: expired_at,
                },
            );
        }
    }
    let freed = kura
        .evict_block_bodies(payload_len)
        .expect("attempt eviction with expired adverts");
    assert_eq!(
        freed, 0,
        "expired adverts must not satisfy the remote replica quorum"
    );
    assert!(
        kura.replica_registry.lock().is_empty(),
        "eviction check should prune expired replica adverts"
    );
    let index = {
        let mut store = kura.block_store.lock();
        store.read_block_index(1).expect("block index")
    };
    assert!(!index.is_evicted());
    advertise_required_replicas(&kura, height);
    let freed = kura
        .evict_block_bodies(payload_len)
        .expect("evict with fresh adverts");
    assert!(freed >= payload_len);
}
#[test]
fn replica_adverts_expiring_during_compaction_block_stage_publication() {
    let temp_dir = TempDir::new().unwrap();
    populate_store(&temp_dir, 4);
    let config = kura_config_for_dir(&temp_dir, NonZeroUsize::new(1).expect("non-zero"));
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura init");
    let height = nonzero!(2_usize);
    let (_block_hash, payload_len) = advertise_required_replicas(&kura, height);
    kura.pause_next_eviction_before_stage_publication_for_tests();
    let evict_kura = Arc::clone(&kura);
    let handle = thread::spawn(move || {
        evict_kura
            .evict_block_bodies(payload_len)
            .expect("eviction freshness recheck")
    });
    let deadline = Instant::now() + Duration::from_secs(2);
    while !kura.eviction_paused_before_stage_publication_for_tests() {
        assert!(
            !handle.is_finished(),
            "eviction completed before its final freshness boundary"
        );
        assert!(
            Instant::now() < deadline,
            "eviction did not reach its final freshness boundary"
        );
        thread::yield_now();
    }
    let expired_at = Instant::now()
        .checked_sub(kura.replica_advert_ttl() + Duration::from_secs(1))
        .expect("expired instant");
    {
        let mut registry = kura.replica_registry.lock();
        assert!(
            !registry.is_empty(),
            "the fixture must install exact selected-keeper adverts"
        );
        for adverts in registry.values_mut() {
            for advert in adverts.values_mut() {
                advert.observed_at = expired_at;
            }
        }
    }
    kura.resume_eviction_before_stage_publication_for_tests();
    let freed = handle.join().expect("eviction thread");
    assert_eq!(
        freed, 0,
        "adverts expiring after compaction must not authorize stage publication"
    );
    assert!(
        kura.replica_registry.lock().is_empty(),
        "the final freshness check must prune expired adverts"
    );
    let index = {
        let mut store = kura.block_store.lock();
        store.read_block_index(1).expect("block index")
    };
    assert!(
        !index.is_evicted(),
        "the canonical body must remain inline when final keeper authority expires"
    );
    let blocks_dir = primary_blocks_dir(&temp_dir);
    for name in [
        EVICTION_COMPACTION_STAGE_FILE_NAME,
        EVICTION_COMPACTION_DATA_FILE_NAME,
        EVICTION_COMPACTION_INDEX_FILE_NAME,
    ] {
        assert!(
            !blocks_dir.join(name).exists(),
            "failed eviction authority must not leave `{name}` publishable"
        );
    }
}
#[test]
fn evict_block_bodies_zero_request_is_noop() {
    let temp_dir = TempDir::new().unwrap();
    populate_store(&temp_dir, 4);
    let config = kura_config_for_dir(&temp_dir, NonZeroUsize::new(1).expect("non-zero"));
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura init");
    advertise_required_replicas(&kura, nonzero!(2_usize));
    let before_indices = {
        let mut store = kura.block_store.lock();
        let mut indices = vec![BlockIndex::default(); 4];
        store
            .read_block_indices(0, &mut indices)
            .expect("read indices before zero eviction");
        indices
    };
    let freed = kura
        .evict_block_bodies(0)
        .expect("zero-byte eviction request");
    assert_eq!(freed, 0);
    let after_indices = {
        let mut store = kura.block_store.lock();
        let mut indices = vec![BlockIndex::default(); 4];
        store
            .read_block_indices(0, &mut indices)
            .expect("read indices after zero eviction");
        indices
    };
    assert_eq!(after_indices, before_indices);
}
#[test]
fn evict_block_bodies_is_idempotent_for_already_evicted_body() {
    let temp_dir = TempDir::new().unwrap();
    populate_store(&temp_dir, 4);
    let config = kura_config_for_dir(&temp_dir, NonZeroUsize::new(1).expect("non-zero"));
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura init");
    let height = nonzero!(2_usize);
    let (_block_hash, payload_len) = advertise_required_replicas(&kura, height);
    let first = kura
        .evict_block_bodies(payload_len)
        .expect("first eviction");
    assert!(first >= payload_len);
    let second = kura
        .evict_block_bodies(payload_len)
        .expect("repeat eviction");
    assert_eq!(
        second, 0,
        "already-evicted bodies should not be counted as newly freed"
    );
    let index = {
        let mut store = kura.block_store.lock();
        store.read_block_index(1).expect("block index")
    };
    assert!(index.is_evicted());
}
#[test]
fn remote_only_status_requires_canonical_hash_and_exact_length() {
    let temp_dir = TempDir::new().unwrap();
    populate_store(&temp_dir, 4);
    let config = kura_config_for_dir(&temp_dir, NonZeroUsize::new(1).expect("non-zero"));
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura init");
    let height = nonzero!(2_usize);
    let (block_hash, payload_len) = advertise_required_replicas(&kura, height);
    let freed = kura
        .evict_block_bodies(payload_len)
        .expect("evict block body");
    assert!(freed >= payload_len);
    {
        let store = kura.block_store.lock();
        store
            .remove_da_block_file(height.get() as u64)
            .expect("remove sidecar to exercise remote-only status");
    }
    kura.replica_registry.lock().clear();
    assert_eq!(
        kura.block_body_status_by_hash(block_hash),
        Some(BlockBodyStatus::Missing),
        "evicted block should be missing without fresh matching replica evidence"
    );
    let wrong_hash = {
        let block: SignedBlock = ValidBlock::new_dummy(checked_keypair().private_key()).into();
        block.hash()
    };
    assert_ne!(wrong_hash, block_hash);
    for _ in 0..EVICTION_REQUIRED_REPLICAS.get() {
        let peer = checked_peer_id();
        kura.record_block_replica_advert(peer, height.get() as u64, wrong_hash, payload_len);
    }
    assert_eq!(
        kura.block_body_status_by_hash(block_hash),
        Some(BlockBodyStatus::Missing),
        "replica evidence for a different hash must not satisfy the canonical block"
    );
    for _ in 0..EVICTION_REQUIRED_REPLICAS.get() {
        let peer = checked_peer_id();
        kura.record_block_replica_advert(
            peer,
            height.get() as u64,
            block_hash,
            payload_len.saturating_add(1),
        );
    }
    assert_eq!(
        kura.block_body_status_by_hash(block_hash),
        Some(BlockBodyStatus::Missing),
        "replica evidence with the wrong payload length must not mark the body remote-only"
    );
    assert_eq!(
        kura.advertise_required_replicas_for_bench(height),
        Some(payload_len)
    );
    assert_eq!(
        kura.block_body_status_by_hash(block_hash),
        Some(BlockBodyStatus::RemoteOnly {
            replicas: EVICTION_REQUIRED_REPLICAS.get()
        })
    );
}
#[test]
fn eviction_keeps_genesis_and_retained_tail_inline() {
    let temp_dir = TempDir::new().unwrap();
    populate_store(&temp_dir, 3);
    let config = kura_config_for_dir(&temp_dir, NonZeroUsize::new(1).expect("non-zero"));
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura init");
    for height in [nonzero!(1_usize), nonzero!(2_usize), nonzero!(3_usize)] {
        advertise_required_replicas(&kura, height);
    }
    let requested = {
        let mut store = kura.block_store.lock();
        (0..3)
            .map(|idx| store.read_block_index(idx).expect("block index").length)
            .sum::<u64>()
    };
    let freed = kura
        .evict_block_bodies(requested)
        .expect("attempt full eviction");
    assert!(freed > 0);
    let indices = {
        let mut store = kura.block_store.lock();
        let mut indices = vec![BlockIndex::default(); 3];
        store
            .read_block_indices(0, &mut indices)
            .expect("read block indices");
        indices
    };
    assert!(
        !indices[0].is_evicted(),
        "genesis body must remain inline even if advertised"
    );
    assert!(indices[1].is_evicted(), "eligible middle body should evict");
    assert!(
        !indices[2].is_evicted(),
        "recent retained tail body must remain inline"
    );
}
#[test]
fn bench_eviction_helper_requires_a_finalized_remote_eviction_fixture() {
    let temp_dir = TempDir::new().unwrap();
    let config = kura_config_for_dir(&temp_dir, NonZeroUsize::new(1).expect("non-zero"));
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura init");
    store_dummy_block_arcs(&kura, 3);
    let (_, payload_len) = advertise_required_replicas(&kura, nonzero!(2_usize));
    let freed = kura
        .evict_block_bodies_for_bench(payload_len)
        .expect("evict benchmark block body");
    assert_eq!(freed, payload_len);
}
#[test]
fn canonical_rewrite_purges_equal_length_stale_sidecar_before_reeviction() {
    let temp_dir = TempDir::new().unwrap();
    let config = kura_config_for_dir(&temp_dir, nonzero!(1_usize));
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura init");
    let blocks = store_dummy_block_arcs(&kura, 3);
    let height = nonzero!(2_usize);
    let original_sidecar = blocks[1].encode_wire().expect("encode original block wire");
    let da_path = {
        let store = kura.block_store.lock();
        store
            .write_da_block_bytes(2, &original_sidecar)
            .expect("plant stale cache candidate beside the inline body");
        store.da_block_path(2)
    };
    kura.prune_to_height(2)
        .expect("remove the tail before replacing height two");
    // Rebuild the canonical transaction payload with a different fixed-width
    // header context so the hostile sidecar and replacement retain the same
    // complete-wire length without synthetic padding.
    let replacement_transactions = blocks[1]
        .external_transactions()
        .cloned()
        .map(|transaction| AcceptedTransaction::new_unchecked(Cow::Owned(transaction)))
        .collect();
    let mut replacement: SignedBlock = BlockBuilder::new(replacement_transactions)
        .chain(
            blocks[1].header().view_change_index().saturating_add(1),
            Some(blocks[0].as_ref()),
        )
        .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
        .unpack(|_| {})
        .into();
    if let Some(external) = replacement
        .execution_context()
        .map(|context| context.external.clone())
    {
        replacement.set_execution_context(Some(BlockExecutionContextBundle::new(external)));
    }
    let replacement = Arc::new(replacement);
    let replacement_wire = replacement.encode_wire().expect("replacement wire");
    assert_eq!(
        replacement_wire.len(),
        original_sidecar.len(),
        "fixture must exercise the same-length stale-sidecar hazard"
    );
    assert_ne!(replacement_wire, original_sidecar);
    kura.replace_top_block(Arc::clone(&replacement))
        .expect("replace the unfinalized canonical top inline");
    assert!(
        !da_path.exists(),
        "an inline canonical rewrite must purge the old height sidecar"
    );
    let tail: Arc<SignedBlock> = Arc::new(
        ValidBlock::new_dummy_and_modify_header(checked_keypair().private_key(), |header| {
            header.set_height(nonzero!(3_u64));
            header.set_prev_block_hash(Some(replacement.hash()));
        })
        .into(),
    );
    kura.store_block(tail)
        .expect("append replacement successor");
    let (_, advertised_len) = advertise_required_replicas(&kura, height);
    assert_eq!(
        advertised_len,
        u64::try_from(replacement_wire.len()).unwrap()
    );
    assert!(
        kura.evict_block_bodies(advertised_len)
            .expect("re-evict replacement body")
            >= advertised_len
    );
    assert_eq!(
        std::fs::read(&da_path).expect("read replacement DA sidecar"),
        replacement_wire,
        "re-eviction must publish the current canonical wire even at equal length"
    );
    assert_eq!(
        kura.get_block(height).as_deref(),
        Some(replacement.as_ref())
    );
}
#[test]
fn eviction_flushes_pending_fsync_before_rewrite() {
    let temp_dir = TempDir::new().unwrap();
    let config = KuraConfig { init_mode: iroha_config::kura::InitMode::Strict, store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
        max_disk_usage_bytes: iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
        blocks_in_memory: NonZeroUsize::new(1).expect("non-zero"),
        debug_output_new_blocks: false,
        merge_ledger_cache_capacity: MERGE_LEDGER_CACHE_CAPACITY,
        fsync_mode: FsyncMode::Batched,
        fsync_interval: Duration::from_secs(3600),
        lane_history_retention: LANE_HISTORY_RETENTION,
        replica_advert: iroha_config::parameters::defaults::kura::REPLICA_ADVERT_POLICY,
    };
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura init");
    let mut blocks = DummyBlocks::new();
    for _ in 0..3 {
        let block = blocks.next();
        kura.store_block(block)
            .expect("append block through the live canonical image");
    }
    {
        let mut store = kura.block_store.lock();
        store
            .flush_pending_fsync(true)
            .expect("flush pending fsync");
    }
    let block = blocks.next();
    kura.block_store
        .lock()
        .append_block_to_chain(block.as_ref())
        .expect("append block");
    {
        // The direct BlockStore append is used only to retain a pending
        // batched-fsync boundary. Mirror the accepted block into the live
        // canonical image; the ordinary Kura path would force the marker.
        let mut block_data = kura.block_data.lock();
        block_data.push((block.hash(), Some(block)));
        Kura::drop_persisted_blocks(&mut block_data, 4, kura.blocks_in_memory.get());
    }
    let (durable_before, index_before) = {
        let mut store = kura.block_store.lock();
        let durable = store.read_durable_index_count().expect("durable count");
        let index = store.read_index_count().expect("index count");
        (durable, index)
    };
    assert_eq!(durable_before, 3);
    assert_eq!(index_before, 4);
    advertise_required_replicas(&kura, nonzero!(2_usize));
    let evict_len = {
        let mut store = kura.block_store.lock();
        store.read_block_index(1).expect("block index").length
    };
    kura.evict_block_bodies(evict_len)
        .expect("evict block bodies");
    let index_after = {
        let mut store = kura.block_store.lock();
        store
            .read_index_count()
            .expect("index count after eviction")
    };
    assert_eq!(
        index_after, 4,
        "eviction should not truncate pending index entries"
    );
}
#[test]
fn evict_block_bodies_releases_block_store_lock_while_compacting() {
    let temp_dir = TempDir::new().unwrap();
    populate_store(&temp_dir, 4);
    let config = kura_config_for_dir(&temp_dir, NonZeroUsize::new(1).expect("non-zero"));
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura init");
    let (_block_hash, evict_len) = advertise_required_replicas(&kura, nonzero!(2_usize));
    kura.pause_next_eviction_after_snapshot_for_tests();
    let evict_kura = Arc::clone(&kura);
    let handle = thread::spawn(move || {
        evict_kura
            .evict_block_bodies(evict_len)
            .expect("evict block bodies")
    });
    let deadline = Instant::now() + Duration::from_secs(2);
    while !kura.eviction_paused_after_snapshot_for_tests() {
        assert!(
            !handle.is_finished(),
            "eviction completed before reaching snapshot pause"
        );
        assert!(
            Instant::now() < deadline,
            "eviction did not reach snapshot pause"
        );
        thread::yield_now();
    }
    let block_store_available = kura.block_store.try_lock().is_some();
    kura.resume_eviction_after_snapshot_for_tests();
    let freed = handle.join().expect("eviction thread");
    assert!(
        block_store_available,
        "block_store lock should be released during eviction temp-file compaction"
    );
    assert!(freed >= evict_len, "eviction should still reclaim the body");
}
#[test]
fn evicted_block_caches_after_remote_rehydrate() {
    let temp_dir = TempDir::new().unwrap();
    populate_store(&temp_dir, 4);
    let (kura, _) = Kura::open_test_kura_with_configured_lane_config(
        &KuraConfig { init_mode: iroha_config::kura::InitMode::Strict, store_dir: WithOrigin::inline(temp_dir.path().to_str().unwrap().into()),
            max_disk_usage_bytes: iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
            blocks_in_memory: NonZeroUsize::new(1).expect("non-zero"),
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity: MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: FsyncMode::Batched,
            fsync_interval: FSYNC_INTERVAL,
            lane_history_retention: LANE_HISTORY_RETENTION,
            replica_advert: iroha_config::parameters::defaults::kura::REPLICA_ADVERT_POLICY,
        },
        &RuntimeLaneConfig::default(),
    )
    .expect("kura init");
    let height = NonZeroUsize::new(2).expect("non-zero");
    let block = kura
        .get_block(height)
        .expect("inline block before eviction");
    let (block_hash, _) = advertise_required_replicas(&kura, height);
    let evict_len = {
        let mut store = kura.block_store.lock();
        store.read_block_index(1).expect("block index").length
    };
    let freed = kura
        .evict_block_bodies(evict_len)
        .expect("evict block bodies");
    assert!(
        freed >= evict_len,
        "expected eviction to free at least one block"
    );
    let (evicted_index, da_path) = {
        let mut store = kura.block_store.lock();
        (
            store.read_block_index(1).expect("block index"),
            store.da_block_path(2),
        )
    };
    assert!(evicted_index.is_evicted());
    assert!(
        da_path.exists(),
        "eviction should create a local DA sidecar"
    );
    assert_eq!(
        kura.block_body_status_by_hash(block_hash),
        Some(BlockBodyStatus::LocalSidecar)
    );
    kura.cache_block_body(block.as_ref())
        .expect("recache sidecar block");
    let rehydrated = kura.get_block(height).expect("rehydrated block");
    assert_eq!(rehydrated.hash(), block_hash);
}
#[test]
fn evicted_block_status_becomes_local_sidecar_after_cache() {
    let temp_dir = TempDir::new().unwrap();
    populate_store(&temp_dir, 4);
    let config = kura_config_for_dir(&temp_dir, NonZeroUsize::new(1).expect("non-zero"));
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura init");
    let height = nonzero!(2_usize);
    let block = kura
        .get_block(height)
        .expect("inline block before eviction");
    let block_hash = block.hash();
    let (advertised_hash, payload_len) = advertise_required_replicas(&kura, height);
    assert_eq!(advertised_hash, block_hash);
    let freed = kura
        .evict_block_bodies(payload_len)
        .expect("evict block body");
    assert!(freed >= payload_len);
    assert_eq!(
        kura.block_body_status_by_hash(block_hash),
        Some(BlockBodyStatus::LocalSidecar)
    );
    assert_eq!(
        kura.durable_block_payload_len_by_hash(block_hash),
        Some((height.get() as u64, payload_len)),
        "durable metadata must still expose payload length for sidecar bodies"
    );
    kura.cache_block_body(block.as_ref())
        .expect("cache rehydrated block body");
    assert_eq!(
        kura.block_body_status_by_hash(block_hash),
        Some(BlockBodyStatus::LocalSidecar)
    );
    assert!(kura.block_payload_available_by_hash(block_hash));
}
#[test]
fn inline_body_status_is_available_after_memory_eviction() {
    let temp_dir = TempDir::new().unwrap();
    let config = kura_config_for_dir(&temp_dir, NonZeroUsize::new(1).expect("non-zero"));
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura init");
    let blocks = store_dummy_block_arcs(&kura, 3);
    let height = nonzero!(2_usize);
    let block_hash = blocks[1].hash();
    assert_eq!(
        kura.block_body_status_by_hash(block_hash),
        Some(BlockBodyStatus::Inline),
        "old persisted bodies outside the memory window should still be inline before eviction"
    );
    assert!(kura.block_payload_available_by_hash(block_hash));
    let block = kura.get_block(height).expect("read inline body from disk");
    assert_eq!(block.hash(), block_hash);
    assert_eq!(
        kura.block_body_status_by_hash(block_hash),
        Some(BlockBodyStatus::Inline),
        "deep inline reads should not pin the body in memory outside the retention window"
    );
}
#[test]
fn zero_length_index_entry_makes_payload_unavailable() {
    let temp_dir = TempDir::new().unwrap();
    let config = kura_config_for_dir(&temp_dir, NonZeroUsize::new(1).expect("non-zero"));
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura init");
    let blocks = store_dummy_block_arcs(&kura, 3);
    let height = nonzero!(2_usize);
    let block_hash = blocks[1].hash();
    {
        let mut store = kura.block_store.lock();
        store
            .write_block_index(1, 0, 0)
            .expect("zero block index length");
    }
    assert_eq!(
        kura.block_body_status_by_hash(block_hash),
        Some(BlockBodyStatus::Missing)
    );
    assert!(!kura.block_payload_available_by_hash(block_hash));
    assert_eq!(kura.durable_block_payload_len_by_hash(block_hash), None);
    assert!(
        kura.get_block(height).is_none(),
        "zero-length block index entries must not be decoded"
    );
}
#[test]
fn evicted_body_without_hash_metadata_is_missing_even_with_adverts() {
    let temp_dir = TempDir::new().unwrap();
    let config = kura_config_for_dir(&temp_dir, NonZeroUsize::new(1).expect("non-zero"));
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura init");
    let blocks = store_dummy_block_arcs(&kura, 4);
    let height = nonzero!(2_usize);
    let block_hash = blocks[1].hash();
    let (_hash, payload_len) = advertise_required_replicas(&kura, height);
    let freed = kura
        .evict_block_bodies(payload_len)
        .expect("evict block body");
    assert!(freed >= payload_len);
    {
        let mut store = kura.block_store.lock();
        store
            .remove_da_block_file(height.get() as u64)
            .expect("remove sidecar to isolate missing hash metadata");
        store
            .truncate_hashes_to_count(1)
            .expect("remove hash metadata for evicted body");
    }
    advertise_required_replicas(&kura, height);
    assert_eq!(
        kura.block_body_status_by_hash(block_hash),
        Some(BlockBodyStatus::Missing),
        "remote adverts must not make a body available when Kura lacks durable hash metadata"
    );
    assert!(!kura.block_payload_available_by_hash(block_hash));
    assert_eq!(kura.durable_block_payload_len_by_hash(block_hash), None);
}
#[test]
fn get_block_rejects_hash_mismatched_local_sidecar() {
    let temp_dir = TempDir::new().unwrap();
    let config = kura_config_for_dir(&temp_dir, NonZeroUsize::new(1).expect("non-zero"));
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura init");
    let blocks = store_dummy_block_arcs(&kura, 4);
    let height = nonzero!(2_usize);
    let block_hash = blocks[1].hash();
    let conflicting = Arc::clone(&blocks[2]);
    assert_ne!(block_hash, conflicting.hash());
    let (_hash, payload_len) = advertise_required_replicas(&kura, height);
    let freed = kura
        .evict_block_bodies(payload_len)
        .expect("evict block body");
    assert!(freed >= payload_len);
    let (frame, _versioned) = conflicting
        .canonical_wire()
        .expect("encode conflicting sidecar")
        .into_parts();
    {
        let store = kura.block_store.lock();
        store
            .write_da_block_bytes(height.get() as u64, &frame)
            .expect("write conflicting sidecar");
    }
    assert!(
        kura.get_block(height).is_none(),
        "Kura must reject a sidecar whose decoded hash differs from canonical metadata"
    );
}
#[test]
fn recent_disk_loaded_body_is_cached_after_read() {
    let temp_dir = TempDir::new().unwrap();
    let config = kura_config_for_dir(&temp_dir, NonZeroUsize::new(2).expect("non-zero"));
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura init");
    let blocks = store_dummy_block_arcs(&kura, 4);
    let height = nonzero!(3_usize);
    let block_hash = blocks[2].hash();
    {
        let mut data = kura.block_data.lock();
        data[height.get() - 1].1 = None;
    }
    assert_eq!(
        kura.block_body_status_by_hash(block_hash),
        Some(BlockBodyStatus::Inline)
    );
    let loaded = kura.get_block(height).expect("read recent inline body");
    assert_eq!(loaded.hash(), block_hash);
    assert_eq!(
        kura.block_body_status_by_hash(block_hash),
        Some(BlockBodyStatus::Cached),
        "recent disk-loaded bodies should be cached in memory after read"
    );
}
#[test]
fn concurrent_index_eviction_cannot_reinsert_inline_body_into_memory_cache() {
    let temp_dir = TempDir::new().expect("create Kura root");
    let config = kura_config_for_dir(&temp_dir, nonzero!(4_usize));
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("open Kura");
    let blocks = store_dummy_block_arcs(&kura, 2);
    let canonical = Arc::clone(&blocks[1]);
    let height = nonzero!(2_usize);
    kura.block_data.lock()[1].1 = None;
    kura.pause_next_block_read_before_cache_recheck_for_tests();
    let read_kura = Arc::clone(&kura);
    let reader = thread::spawn(move || read_kura.get_block(height));
    let deadline = Instant::now() + Duration::from_secs(5);
    while !kura.block_read_paused_before_cache_recheck_for_tests() {
        assert!(
            Instant::now() < deadline,
            "block read did not reach the cache-publication race barrier"
        );
        thread::yield_now();
    }
    // Reproduce eviction's publication order: the durable index becomes evicted and the old
    // memory slot is cleared while the block-store guard remains held. The reader already
    // decoded the inline bytes, but has not yet attempted its opportunistic cache fill.
    {
        let _write_guard = kura.block_store_write_lock.lock();
        let mut store = kura.block_store.lock();
        let original = store.read_block_index(1).expect("read inline index");
        assert!(!original.is_evicted());
        assert!(original.length > 0);
        store
            .write_block_index(1, EVICTED_BLOCK_START, original.length)
            .expect("publish evicted index");
        kura.block_data.lock()[1].1 = None;
    }
    kura.resume_block_read_before_cache_recheck_for_tests();
    assert_eq!(
        reader.join().expect("join concurrent reader").as_deref(),
        Some(canonical.as_ref()),
        "the read may linearize before eviction publication"
    );
    assert!(
        kura.block_data.lock()[1].1.is_none(),
        "a reader must not reinsert an inline body after its index became evicted"
    );
    assert!(
        kura.block_store
            .lock()
            .read_block_index(1)
            .expect("reread evicted index")
            .is_evicted()
    );
}
#[test]
fn cache_block_body_is_idempotent_for_existing_local_sidecar() {
    let temp_dir = TempDir::new().unwrap();
    let config = kura_config_for_dir(&temp_dir, NonZeroUsize::new(1).expect("non-zero"));
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura init");
    let blocks = store_dummy_block_arcs(&kura, 4);
    let height = nonzero!(2_usize);
    let block = Arc::clone(&blocks[1]);
    let block_hash = block.hash();
    let (_hash, payload_len) = advertise_required_replicas(&kura, height);
    let freed = kura
        .evict_block_bodies(payload_len)
        .expect("evict block body");
    assert!(freed >= payload_len);
    kura.cache_block_body(block.as_ref())
        .expect("cache rehydrated body");
    let (da_path, first_len, first_usage) = {
        let mut store = kura.block_store.lock();
        let da_path = store.da_block_path(2);
        let first_len = std::fs::metadata(&da_path).expect("sidecar metadata").len();
        let usage = Kura::block_store_tracked_bytes(&mut store).expect("tracked bytes");
        (da_path, first_len, usage)
    };
    kura.cache_block_body(block.as_ref())
        .expect("cache same body again");
    let (second_len, second_usage) = {
        let mut store = kura.block_store.lock();
        let second_len = std::fs::metadata(&da_path).expect("sidecar metadata").len();
        let usage = Kura::block_store_tracked_bytes(&mut store).expect("tracked bytes");
        (second_len, usage)
    };
    assert_eq!(second_len, first_len);
    assert_eq!(
        second_usage, first_usage,
        "rewriting the same sidecar should not inflate tracked storage"
    );
    assert_eq!(
        kura.block_body_status_by_hash(block_hash),
        Some(BlockBodyStatus::LocalSidecar)
    );
    let loaded = kura.get_block(height).expect("read cached sidecar");
    assert_eq!(loaded.hash(), block_hash);
}
#[test]
fn cache_block_body_is_noop_for_inline_block() {
    let temp_dir = TempDir::new().unwrap();
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("kura init");
    let block = store_dummy_block_arcs(&kura, 1)
        .pop()
        .expect("stored block");
    let block_hash = block.hash();
    let da_path = {
        let store = kura.block_store.lock();
        store.da_block_path(1)
    };
    kura.cache_block_body(block.as_ref())
        .expect("inline cache should be a no-op");
    assert!(
        !da_path.exists(),
        "inline blocks must not be duplicated into the sidecar cache"
    );
    assert_eq!(
        kura.block_body_status_by_hash(block_hash),
        Some(BlockBodyStatus::Cached)
    );
    assert!(kura.block_payload_available_by_hash(block_hash));
    let total_before = kura
        .refresh_total_disk_usage_bytes()
        .expect("cache total bytes before inline-sidecar rejection");
    assert!(
        kura.remove_evicted_block_sidecar_for_testing(nonzero!(1_usize))
            .is_err(),
        "the remote-only test hook must reject a still-inline body"
    );
    assert_eq!(
        kura.refresh_total_disk_usage_bytes()
            .expect("scan total bytes after inline-sidecar rejection"),
        total_before,
        "rejected test-hook removal must not perturb disk accounting"
    );
}
