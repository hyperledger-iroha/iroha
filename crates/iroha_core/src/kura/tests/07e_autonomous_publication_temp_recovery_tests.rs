fn autonomous_temp_recovery_catalog() -> LaneCatalog {
    let lane0 = ModelLaneConfig::default();
    let lane1 = ModelLaneConfig {
        id: LaneId::from(1),
        alias: "beta".to_owned(),
        ..ModelLaneConfig::default()
    };
    LaneCatalog::new(nonzero!(2_u32), vec![lane0, lane1]).expect("temp-recovery lane catalog")
}

fn open_authenticated_temp_recovery_kura(
    config: &KuraConfig,
    lane_config: &RuntimeLaneConfig,
    catalog: &LaneCatalog,
) -> Result<(Arc<Kura>, BlockCount)> {
    Kura::new_with_configured_lane_catalog(config, lane_config, catalog)
}

fn publish_temp_recovery_catalog_baseline(kura: &Kura, catalog: &LaneCatalog) {
    let lane_config = RuntimeLaneConfig::from_catalog(catalog);
    let mut incarnations = BTreeMap::new();
    let mut activation_heights = BTreeMap::new();
    for entry in lane_config.entries() {
        let (incarnation, activation_height) = kura
            .active_lane_incarnation_marker(entry)
            .expect("read configured temp-recovery lane marker");
        incarnations.insert(entry.lane_id, incarnation);
        activation_heights.insert(entry.lane_id, activation_height);
    }
    let baseline = LaneLifecycleParameterV1::catalog_hash(catalog);
    kura.establish_or_verify_configured_primary_geometry_anchor(
        lane_config.primary(),
        incarnations[&LaneId::SINGLE],
        baseline,
    )
    .expect("anchor temp-recovery configured primary");
    kura.mark_lane_geometry_catalog_published(
        &lane_config,
        &incarnations,
        &activation_heights,
        Some(baseline),
    )
    .expect("publish temp-recovery configured catalog baseline");
}

fn assert_retained_publication_quarantine(path: &Path, expected: &[u8]) {
    let metadata = fs::symlink_metadata(path).expect("stat retained publication quarantine");
    assert!(
        metadata.file_type().is_file()
            && !metadata.file_type().is_symlink()
            && Kura::sidecar_is_single_link(&metadata),
        "retained publication quarantine must be one regular single-link object",
    );
    assert_eq!(
        fs::read(path).expect("read retained publication quarantine"),
        expected,
    );
}

fn publication_quarantines(parent: &Path, prefix: &str) -> Vec<PathBuf> {
    let mut paths = fs::read_dir(parent)
        .expect("inventory retained publication quarantines")
        .map(|entry| entry.expect("read retained quarantine entry").path())
        .filter(|path| {
            path.file_name()
                .and_then(std::ffi::OsStr::to_str)
                .is_some_and(|name| Kura::is_autonomous_publication_quarantine_name(name, prefix))
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

#[derive(Debug, PartialEq, Eq)]
struct ProcessGenerationClaimStateForTesting {
    disk_usage: DiskUsageAccountingSnapshotForTesting,
    accounting_generation: u64,
    mutations_in_flight: usize,
    stable_bytes: Option<Vec<u8>>,
    legacy_temp_bytes: Option<Vec<u8>>,
    unresolved_atomic_temps: Vec<PathBuf>,
    quarantines: Vec<(PathBuf, Vec<u8>)>,
    live_claim_present: bool,
}

fn process_generation_claim_state(kura: &Kura) -> ProcessGenerationClaimStateForTesting {
    let optional_bytes = |path: &Path| match fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => panic!(
            "read process-generation state at {}: {error}",
            path.display()
        ),
    };
    let (accounting_generation, mutations_in_flight) = {
        let accounting = kura.disk_usage_total_accounting.lock();
        (accounting.generation, accounting.mutations_in_flight)
    };
    let mut unresolved_atomic_temps = fs::read_dir(&kura.store_root)
        .expect("inventory process-generation atomic temporaries")
        .map(|entry| entry.expect("read process-generation root entry").path())
        .filter(|path| {
            path.file_name()
                .and_then(std::ffi::OsStr::to_str)
                .is_some_and(|name| {
                    Kura::is_unresolved_autonomous_publication_temporary_name(
                        name,
                        AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX,
                    )
                })
        })
        .collect::<Vec<_>>();
    unresolved_atomic_temps.sort();
    let quarantines = publication_quarantines(
        &kura.store_root,
        AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX,
    )
    .into_iter()
    .map(|path| {
        let bytes = fs::read(&path).expect("read process-generation quarantine snapshot");
        (path, bytes)
    })
    .collect();
    ProcessGenerationClaimStateForTesting {
        disk_usage: kura
            .disk_usage_accounting_snapshot_for_tests()
            .expect("snapshot process-generation disk accounting"),
        accounting_generation,
        mutations_in_flight,
        stable_bytes: optional_bytes(&Kura::autonomous_lifecycle_process_generation_path_for(
            &kura.store_root,
        )),
        legacy_temp_bytes: optional_bytes(
            &Kura::autonomous_lifecycle_process_generation_temp_path_for(&kura.store_root),
        ),
        unresolved_atomic_temps,
        quarantines,
        live_claim_present: kura
            .autonomous_lifecycle_process_generation_claim
            .get()
            .is_some(),
    }
}

fn assert_bootstrap_atomic_temp_recovery_controls(
    store_root: &Path,
    config: &KuraConfig,
    lane_config: &RuntimeLaneConfig,
    catalog: &LaneCatalog,
    bootstrap_path: &Path,
    bootstrap_bytes: &[u8],
) {
    let parent = bootstrap_path.parent().expect("bootstrap path has parent");
    let atomic_temp = parent.join(format!(
        "{AUTONOMOUS_LIFECYCLE_BOOTSTRAP_ATOMIC_TEMP_PREFIX}crash-residue"
    ));
    fs::remove_file(bootstrap_path).expect("model crash before bootstrap atomic rename");
    fs::write(&atomic_temp, bootstrap_bytes).expect("write bootstrap atomic temporary");
    drop(
        open_authenticated_temp_recovery_kura(config, lane_config, catalog)
            .expect("startup quarantines an authenticated pre-rename bootstrap temporary"),
    );
    assert!(!atomic_temp.exists() && !bootstrap_path.exists());

    let quarantine = parent.join(format!(
        "{AUTONOMOUS_LIFECYCLE_BOOTSTRAP_ATOMIC_TEMP_PREFIX}quarantine-{}",
        Hash::new(bootstrap_bytes)
    ));
    assert_retained_publication_quarantine(&quarantine, bootstrap_bytes);
    let quarantines =
        publication_quarantines(parent, AUTONOMOUS_LIFECYCLE_BOOTSTRAP_ATOMIC_TEMP_PREFIX);
    drop(
        open_authenticated_temp_recovery_kura(config, lane_config, catalog)
            .expect("startup revalidates the exact retained bootstrap quarantine"),
    );
    assert_eq!(
        publication_quarantines(parent, AUTONOMOUS_LIFECYCLE_BOOTSTRAP_ATOMIC_TEMP_PREFIX,),
        quarantines,
        "idempotent startup must not add or remove bootstrap tombstones",
    );
    assert_retained_publication_quarantine(&quarantine, bootstrap_bytes);
    assert!(!bootstrap_path.exists());

    let mut tampered_quarantine = bootstrap_bytes.to_vec();
    *tampered_quarantine
        .last_mut()
        .expect("bootstrap quarantine is non-empty") ^= 0x40;
    fs::write(&quarantine, &tampered_quarantine).expect("tamper retained bootstrap quarantine");
    assert!(
        open_authenticated_temp_recovery_kura(config, lane_config, catalog).is_err(),
        "retained quarantine bytes must remain bound to their digest name",
    );
    fs::write(&quarantine, bootstrap_bytes).expect("restore retained bootstrap quarantine");

    let malformed_quarantine = parent.join(format!(
        "{AUTONOMOUS_LIFECYCLE_BOOTSTRAP_ATOMIC_TEMP_PREFIX}quarantine-not-a-digest"
    ));
    fs::write(&malformed_quarantine, bootstrap_bytes)
        .expect("write malformed reserved bootstrap quarantine");
    assert!(
        open_authenticated_temp_recovery_kura(config, lane_config, catalog).is_err(),
        "reserved quarantine names must fail closed",
    );
    fs::remove_file(&malformed_quarantine).expect("remove malformed quarantine control");

    let mut malformed = bootstrap_bytes.to_vec();
    *malformed.last_mut().expect("bootstrap is non-empty") ^= 0x80;
    fs::write(&atomic_temp, malformed).expect("write malformed bootstrap temporary");
    assert!(
        open_authenticated_temp_recovery_kura(config, lane_config, catalog).is_err()
            && atomic_temp.exists()
    );
    fs::remove_file(&atomic_temp).expect("remove malformed bootstrap temporary");

    fs::File::create(&atomic_temp)
        .expect("create oversized bootstrap temporary")
        .set_len(AUTONOMOUS_LIFECYCLE_BOOTSTRAP_MAX_BYTES as u64 + 1)
        .expect("extend oversized bootstrap temporary");
    assert!(
        open_authenticated_temp_recovery_kura(config, lane_config, catalog).is_err()
            && atomic_temp.exists()
    );
    fs::remove_file(&atomic_temp).expect("remove oversized bootstrap temporary");

    let second = parent.join(format!(
        "{AUTONOMOUS_LIFECYCLE_BOOTSTRAP_ATOMIC_TEMP_PREFIX}second"
    ));
    fs::write(&atomic_temp, bootstrap_bytes).expect("write first bootstrap temporary");
    fs::write(&second, bootstrap_bytes).expect("write second bootstrap temporary");
    assert!(open_authenticated_temp_recovery_kura(config, lane_config, catalog).is_err());
    assert!(atomic_temp.exists() && second.exists());
    fs::remove_file(&atomic_temp).expect("remove first ambiguous bootstrap temporary");
    fs::remove_file(&second).expect("remove second ambiguous bootstrap temporary");

    fs::write(&atomic_temp, bootstrap_bytes).expect("write hard-linked bootstrap temporary");
    let hardlink = parent.join("bootstrap-temp-hardlink");
    fs::hard_link(&atomic_temp, &hardlink).expect("hard-link bootstrap temporary");
    assert!(open_authenticated_temp_recovery_kura(config, lane_config, catalog).is_err());
    assert!(atomic_temp.exists() && hardlink.exists());
    fs::remove_file(&hardlink).expect("remove bootstrap temporary hardlink");
    fs::remove_file(&atomic_temp).expect("remove hard-linked bootstrap temporary");

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let target = store_root.join("bootstrap-temp-symlink-target");
        fs::write(&target, bootstrap_bytes).expect("write bootstrap temporary symlink target");
        symlink(&target, &atomic_temp).expect("symlink bootstrap temporary");
        assert!(
            open_authenticated_temp_recovery_kura(config, lane_config, catalog).is_err()
                && atomic_temp.is_symlink()
        );
        fs::remove_file(&atomic_temp).expect("remove bootstrap temporary symlink");
        fs::remove_file(&target).expect("remove bootstrap temporary symlink target");
    }

    let bootstrap_lane_id = Kura::decode_autonomous_lifecycle_bootstrap(
        bootstrap_path,
        bootstrap_bytes,
    )
    .expect("decode bootstrap route for swapped-path control")
    .body
    .executable_payload
    .origin_proposal
    .descriptor
    .lane_id;
    let wrong_lane = lane_config
        .entries()
        .iter()
        .find(|entry| entry.lane_id != bootstrap_lane_id)
        .expect("bootstrap route-swap control requires another configured lane");
    let wrong_parent = Kura::lane_artifact_dir(&wrong_lane.blocks_dir(store_root));
    fs::create_dir_all(&wrong_parent).expect("create swapped bootstrap parent");
    let wrong_path = wrong_parent.join(format!(
        "{AUTONOMOUS_LIFECYCLE_BOOTSTRAP_ATOMIC_TEMP_PREFIX}wrong-route"
    ));
    fs::write(&wrong_path, bootstrap_bytes).expect("write route-swapped bootstrap temporary");
    assert!(
        open_authenticated_temp_recovery_kura(config, lane_config, catalog).is_err()
            && wrong_path.exists()
    );
    fs::remove_file(&wrong_path).expect("remove route-swapped bootstrap temporary");

    let wrong_quarantine = wrong_parent.join(
        quarantine
            .file_name()
            .expect("retained bootstrap quarantine has a name"),
    );
    fs::write(&wrong_quarantine, bootstrap_bytes)
        .expect("write route-swapped retained bootstrap quarantine");
    assert!(
        open_authenticated_temp_recovery_kura(config, lane_config, catalog).is_err()
            && wrong_quarantine.exists(),
        "a retained quarantine moved to another route must fail closed",
    );
    fs::remove_file(&wrong_quarantine).expect("remove route-swapped retained quarantine");

    fs::write(bootstrap_path, bootstrap_bytes).expect("restore stable bootstrap after temp tests");
}

#[test]
fn process_generation_atomic_temp_recovery_uses_the_real_writer_boundary() {
    let temp_dir = TempDir::new().expect("process-generation recovery temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let catalog = autonomous_temp_recovery_catalog();
    let lane_config = RuntimeLaneConfig::from_catalog(&catalog);
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let local_peer = PeerId::new(signer.public_key().clone());
    let network_id = test_network_id(b"process-generation-real-writer-crash-genesis");
    let stable_path = Kura::autonomous_lifecycle_process_generation_path_for(temp_dir.path());

    let (crashing, _) = open_authenticated_temp_recovery_kura(&config, &lane_config, &catalog)
        .expect("initialize authenticated unclaimed Kura root");
    publish_temp_recovery_catalog_baseline(&crashing, &catalog);
    crashing
        .bind_local_peer_id(local_peer.clone())
        .expect("bind local peer before injected publication crash");
    let usage_before_residue = crashing
        .kura_disk_usage_bytes()
        .expect("measure Kura before process-generation residue");
    crashing.fail_next_atomic_write_after_temporary_sync_for_test();
    assert!(
        crashing
            .claim_autonomous_lifecycle_process_generation(network_id, &local_peer)
            .is_err(),
        "the injected writer boundary must fail before process-generation rename",
    );
    let atomic_temps = fs::read_dir(temp_dir.path())
        .expect("inventory injected process-generation temporary")
        .map(|entry| entry.expect("read Kura-root entry").path())
        .filter(|path| {
            path.file_name()
                .and_then(std::ffi::OsStr::to_str)
                .is_some_and(|name| {
                    name.starts_with(AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX)
                })
        })
        .collect::<Vec<_>>();
    assert_eq!(atomic_temps.len(), 1);
    assert!(!stable_path.exists());
    let residue_bytes =
        fs::read(&atomic_temps[0]).expect("read injected process-generation residue");
    assert_eq!(
        Kura::autonomous_lifecycle_process_generation_publication_residue_bytes(temp_dir.path())
            .expect("account active process-generation residue"),
        residue_bytes.len() as u64,
    );
    assert_eq!(
        crashing
            .kura_disk_usage_bytes()
            .expect("measure Kura with active process-generation residue"),
        usage_before_residue.saturating_add(residue_bytes.len() as u64),
    );
    let retained_initial = temp_dir.path().join(format!(
        "{AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX}quarantine-{}",
        Hash::new(&residue_bytes)
    ));
    drop(crashing);

    assert!(Kura::new(&config, &lane_config).is_err());
    assert!(
        atomic_temps[0].exists(),
        "unauthenticated startup must retain the process-generation residue",
    );
    let (recovered_once, _) =
        open_authenticated_temp_recovery_kura(&config, &lane_config, &catalog)
            .expect("startup quarantines the exact unpublished process generation");
    drop(recovered_once);
    assert!(!atomic_temps[0].exists());
    assert_retained_publication_quarantine(&retained_initial, &residue_bytes);
    assert_eq!(
        Kura::autonomous_lifecycle_process_generation_publication_residue_bytes(temp_dir.path())
            .expect("account retained process-generation quarantine"),
        residue_bytes.len() as u64,
    );
    assert!(!stable_path.exists());

    let conflicting_initial = AutonomousLifecycleProcessGenerationRecordV1::new(
        test_network_id(b"conflicting-retained-initial-chain"),
        local_peer.clone(),
        1,
    )
    .expect("construct conflicting initial process generation")
    .encode_framed()
    .expect("encode conflicting initial process generation");
    let conflicting_initial_path = temp_dir.path().join(format!(
        "{AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX}quarantine-{}",
        Hash::new(&conflicting_initial),
    ));
    fs::write(&conflicting_initial_path, &conflicting_initial)
        .expect("write conflicting retained initial process generation");
    assert!(
        open_authenticated_temp_recovery_kura(&config, &lane_config, &catalog).is_err(),
        "retained generation-one quarantines must agree on chain and local peer",
    );
    fs::remove_file(&conflicting_initial_path)
        .expect("remove conflicting retained initial process generation");

    let quarantines = publication_quarantines(
        temp_dir.path(),
        AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX,
    );
    let (recovered, _) = open_authenticated_temp_recovery_kura(&config, &lane_config, &catalog)
        .expect("startup revalidates retained process-generation quarantine");
    assert_eq!(
        publication_quarantines(
            temp_dir.path(),
            AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX,
        ),
        quarantines,
        "idempotent startup must not add or remove process-generation tombstones",
    );
    recovered
        .bind_local_peer_id(local_peer.clone())
        .expect("bind local peer after cleanup");
    let claim = recovered
        .claim_autonomous_lifecycle_process_generation(network_id, &local_peer)
        .expect("retry initial process-generation publication");
    assert_eq!(claim.generation(), 1);
    let stable_bytes = fs::read(&stable_path).expect("read stable generation one");
    drop(recovered);

    let oversized = temp_dir.path().join(format!(
        "{AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX}oversized"
    ));
    fs::File::create(&oversized)
        .expect("create oversized atomic temporary")
        .set_len(AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_MAX_BYTES as u64 + 1)
        .expect("extend oversized atomic temporary");
    assert!(open_authenticated_temp_recovery_kura(&config, &lane_config, &catalog).is_err());
    assert!(oversized.exists());
    fs::remove_file(&oversized).expect("remove rejected oversized temporary");

    let successor =
        AutonomousLifecycleProcessGenerationRecordV1::new(network_id, local_peer.clone(), 2)
            .expect("construct exact generation-two successor")
            .encode_framed()
            .expect("encode exact generation-two successor");
    let quarantined_successor = temp_dir.path().join(format!(
        "{AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX}quarantine-{}",
        Hash::new(&successor)
    ));
    fs::write(&quarantined_successor, &successor)
        .expect("write crash-resident generation-two quarantine");
    assert_eq!(
        Kura::autonomous_lifecycle_process_generation_publication_residue_bytes(temp_dir.path())
            .expect("account both retained process-generation quarantines"),
        (residue_bytes.len() + successor.len()) as u64,
    );
    drop(
        open_authenticated_temp_recovery_kura(&config, &lane_config, &catalog)
            .expect("startup validates an exact generation-two quarantine"),
    );
    assert_retained_publication_quarantine(&quarantined_successor, &successor);
    let quarantines = publication_quarantines(
        temp_dir.path(),
        AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX,
    );
    drop(
        open_authenticated_temp_recovery_kura(&config, &lane_config, &catalog)
            .expect("startup idempotently revalidates all process-generation quarantines"),
    );
    assert_eq!(
        publication_quarantines(
            temp_dir.path(),
            AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX,
        ),
        quarantines,
    );

    let mut tampered_successor = successor.clone();
    *tampered_successor
        .last_mut()
        .expect("process-generation successor is non-empty") ^= 0x20;
    fs::write(&quarantined_successor, &tampered_successor)
        .expect("tamper retained process-generation quarantine");
    assert!(
        open_authenticated_temp_recovery_kura(&config, &lane_config, &catalog).is_err(),
        "retained process-generation bytes must stay bound to their quarantine name",
    );
    fs::write(&quarantined_successor, &successor)
        .expect("restore retained process-generation quarantine");

    let malformed_quarantine = temp_dir.path().join(format!(
        "{AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX}quarantine-malformed"
    ));
    fs::write(&malformed_quarantine, &successor)
        .expect("write malformed process-generation quarantine name");
    assert!(open_authenticated_temp_recovery_kura(&config, &lane_config, &catalog).is_err());
    fs::remove_file(&malformed_quarantine).expect("remove malformed process-generation quarantine");

    let skipped =
        AutonomousLifecycleProcessGenerationRecordV1::new(network_id, local_peer.clone(), 3)
            .expect("construct skipped process generation")
            .encode_framed()
            .expect("encode skipped process generation");
    let skipped_quarantine = temp_dir.path().join(format!(
        "{AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX}quarantine-{}",
        Hash::new(&skipped),
    ));
    fs::write(&skipped_quarantine, &skipped).expect("write skipped process-generation quarantine");
    assert!(
        open_authenticated_temp_recovery_kura(&config, &lane_config, &catalog).is_err(),
        "a retained quarantine cannot jump beyond the exact stable successor",
    );
    fs::remove_file(&skipped_quarantine).expect("remove skipped-generation quarantine");

    let first = temp_dir.path().join(format!(
        "{AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX}first"
    ));
    let second = temp_dir.path().join(format!(
        "{AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX}second"
    ));
    fs::write(&first, &successor).expect("write first process-generation temporary");
    fs::write(&second, &successor).expect("write second process-generation temporary");
    assert!(open_authenticated_temp_recovery_kura(&config, &lane_config, &catalog).is_err());
    assert!(first.exists() && second.exists());
    fs::remove_file(&first).expect("remove first ambiguous temporary");
    fs::remove_file(&second).expect("remove second ambiguous temporary");

    fs::write(&first, &successor).expect("write hard-linked process-generation temporary");
    let hardlink = temp_dir.path().join("process-generation-temp-hardlink");
    fs::hard_link(&first, &hardlink).expect("hard-link process-generation temporary");
    assert!(open_authenticated_temp_recovery_kura(&config, &lane_config, &catalog).is_err());
    assert!(first.exists() && hardlink.exists());
    fs::remove_file(&hardlink).expect("remove process-generation temporary hardlink");
    fs::remove_file(&first).expect("remove hard-linked process-generation temporary");

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        symlink(&stable_path, &first).expect("symlink process-generation temporary");
        assert!(open_authenticated_temp_recovery_kura(&config, &lane_config, &catalog).is_err());
        assert!(first.is_symlink());
        fs::remove_file(&first).expect("remove process-generation temporary symlink");
    }
    assert_eq!(
        fs::read(&stable_path).expect("stable generation survives negative controls"),
        stable_bytes,
    );
}

#[test]
fn retained_initial_process_generation_quarantine_constrains_first_durable_claim() {
    let temp_dir = TempDir::new().expect("retained initial authority temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let catalog = autonomous_temp_recovery_catalog();
    let lane_config = RuntimeLaneConfig::from_catalog(&catalog);
    let authority_signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let authority_peer = PeerId::new(authority_signer.public_key().clone());
    let conflicting_signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let conflicting_peer = PeerId::new(conflicting_signer.public_key().clone());
    let authority_chain = test_network_id(b"retained-initial-process-generation-authority");
    let conflicting_chain = test_network_id(b"conflicting-initial-process-generation-chain");
    let retained_record = AutonomousLifecycleProcessGenerationRecordV1::new(
        authority_chain,
        authority_peer.clone(),
        1,
    )
    .expect("construct retained generation-one authority");
    let retained_bytes = retained_record
        .encode_framed()
        .expect("encode retained generation-one authority");
    let quarantine_path = temp_dir.path().join(format!(
        "{AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX}quarantine-{}",
        Hash::new(&retained_bytes),
    ));

    let (initialized, _) = open_authenticated_temp_recovery_kura(&config, &lane_config, &catalog)
        .expect("initialize Kura root without a stable process generation");
    publish_temp_recovery_catalog_baseline(&initialized, &catalog);
    drop(initialized);
    fs::write(&quarantine_path, &retained_bytes)
        .expect("install authenticated retained generation-one quarantine");

    let (wrong_chain_kura, _) =
        open_authenticated_temp_recovery_kura(&config, &lane_config, &catalog)
            .expect("startup authenticates the lone retained generation-one authority");
    wrong_chain_kura
        .bind_local_peer_id(authority_peer.clone())
        .expect("bind retained authority peer for wrong-chain claim");
    let wrong_chain_before = process_generation_claim_state(&wrong_chain_kura);
    assert_eq!(
        wrong_chain_before.quarantines,
        vec![(quarantine_path.clone(), retained_bytes.clone())],
    );
    assert!(wrong_chain_before.stable_bytes.is_none());
    assert!(wrong_chain_before.legacy_temp_bytes.is_none());
    assert!(wrong_chain_before.unresolved_atomic_temps.is_empty());
    assert!(!wrong_chain_before.live_claim_present);
    assert_eq!(wrong_chain_before.mutations_in_flight, 0);
    let wrong_chain_error = wrong_chain_kura
        .claim_autonomous_lifecycle_process_generation(conflicting_chain, &authority_peer)
        .expect_err("retained generation one must reject chain identity drift");
    assert!(
        wrong_chain_error
            .to_string()
            .contains("conflicts with retained canonical authority")
    );
    assert_eq!(
        process_generation_claim_state(&wrong_chain_kura),
        wrong_chain_before,
        "a conflicting chain claim must fail before stable/temp, accounting, quarantine, or live-claim mutation",
    );
    drop(wrong_chain_kura);

    let (wrong_peer_kura, _) =
        open_authenticated_temp_recovery_kura(&config, &lane_config, &catalog)
            .expect("restart reauthenticates retained generation-one authority");
    wrong_peer_kura
        .bind_local_peer_id(conflicting_peer.clone())
        .expect("bind conflicting local peer");
    let wrong_peer_before = process_generation_claim_state(&wrong_peer_kura);
    let wrong_peer_error = wrong_peer_kura
        .claim_autonomous_lifecycle_process_generation(authority_chain, &conflicting_peer)
        .expect_err("retained generation one must reject local-peer identity drift");
    assert!(
        wrong_peer_error
            .to_string()
            .contains("conflicts with retained canonical authority")
    );
    assert_eq!(
        process_generation_claim_state(&wrong_peer_kura),
        wrong_peer_before,
        "a conflicting peer claim must fail before stable/temp, accounting, quarantine, or live-claim mutation",
    );
    drop(wrong_peer_kura);

    let (exact_kura, _) = open_authenticated_temp_recovery_kura(&config, &lane_config, &catalog)
        .expect("restart preserves exact retained generation-one authority");
    exact_kura
        .bind_local_peer_id(authority_peer.clone())
        .expect("bind exact retained authority peer");
    let exact_claim = exact_kura
        .claim_autonomous_lifecycle_process_generation(authority_chain, &authority_peer)
        .expect("exact retained authority publishes generation one");
    assert_eq!(exact_claim.generation(), 1);
    let published = process_generation_claim_state(&exact_kura);
    assert_eq!(published.stable_bytes, Some(retained_bytes.clone()));
    assert!(published.legacy_temp_bytes.is_none());
    assert!(published.unresolved_atomic_temps.is_empty());
    assert!(published.live_claim_present);
    assert_eq!(published.mutations_in_flight, 0);
    assert_eq!(
        published.quarantines,
        vec![(quarantine_path.clone(), retained_bytes.clone())],
        "the authenticated crash tombstone remains durable after exact reconciliation",
    );
    let repeated_claim = exact_kura
        .claim_autonomous_lifecycle_process_generation(authority_chain, &authority_peer)
        .expect("repeating the live exact claim is idempotent");
    assert_eq!(repeated_claim, exact_claim);
    assert_eq!(
        process_generation_claim_state(&exact_kura),
        published,
        "an idempotent live retry must not rewrite stable state or consume its quarantine",
    );
    drop(exact_kura);

    let (restarted, _) = open_authenticated_temp_recovery_kura(&config, &lane_config, &catalog)
        .expect("restart revalidates stable generation one and its retained quarantine");
    let restarted_state = process_generation_claim_state(&restarted);
    assert_eq!(restarted_state.stable_bytes, Some(retained_bytes.clone()));
    assert!(restarted_state.unresolved_atomic_temps.is_empty());
    assert_eq!(
        restarted_state.quarantines,
        vec![(quarantine_path, retained_bytes)],
    );
    assert!(!restarted_state.live_claim_present);
}

#[test]
fn provisional_catalog_open_detects_bootstrap_residue_without_mutating_it() {
    let temp_dir = TempDir::new().expect("provisional temp-recovery directory");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let catalog = configured_primary_catalog("provisional-temp-recovery");
    let lane_config = RuntimeLaneConfig::from_catalog(&catalog);
    let (kura, _) = Kura::new_with_configured_lane_catalog(&config, &lane_config, &catalog)
        .expect("establish provisional configured catalog");
    publish_configured_catalog_baseline(&kura, &catalog);
    drop(kura);

    let parent = Kura::lane_artifact_dir(&lane_config.primary().blocks_dir(temp_dir.path()));
    fs::create_dir_all(&parent).expect("create provisional bootstrap namespace");
    let residue = parent.join(format!(
        "{AUTONOMOUS_LIFECYCLE_BOOTSTRAP_ATOMIC_TEMP_PREFIX}provisional"
    ));
    let residue_bytes = b"provisional bootstrap residue remains read-only";
    fs::write(&residue, residue_bytes).expect("write provisional bootstrap residue");
    let policy = SnapshotBootstrapPolicy {
        enabled: true,
        audited_sha256: Some("00".repeat(32)),
        audited_height: Some(1),
    };
    Kura::new_with_configured_lane_catalog_and_snapshot_bootstrap(
        &config,
        &lane_config,
        &catalog,
        &policy,
    )
    .expect_err("provisional startup must detect but never clean atomic residue");
    assert_eq!(
        fs::read(&residue).expect("read retained provisional residue"),
        residue_bytes,
    );
}

#[test]
fn provisional_catalog_open_never_repairs_merge_tail_or_prune_intent() {
    let temp_dir = TempDir::new().expect("provisional merge/prune directory");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let catalog = configured_primary_catalog("provisional-merge-prune");
    let lane_config = RuntimeLaneConfig::from_catalog(&catalog);
    let (kura, _) = Kura::new_with_configured_lane_catalog(&config, &lane_config, &catalog)
        .expect("establish configured provisional fixture");
    publish_configured_catalog_baseline(&kura, &catalog);
    let block = DummyBlocks::new().next();
    let first_hash = block.hash();
    kura.store_block(Arc::clone(&block))
        .expect("store provisional fixture block");
    let snapshot_tail_hash =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xC7; Hash::LENGTH]));
    kura.extend_hash_only_suffix_from_verified_snapshot(&[first_hash, snapshot_tail_hash])
        .expect("publish verified hash-only tail");
    drop(kura);

    let merge_path = lane_config.primary().merge_log_path(temp_dir.path());
    let partial_tail = 73_u32.to_le_bytes();
    fs::OpenOptions::new()
        .append(true)
        .open(&merge_path)
        .and_then(|mut file| file.write_all(&partial_tail))
        .expect("append incomplete merge frame header");
    let merge_bytes = fs::read(&merge_path).expect("snapshot partial merge tail");
    let policy = SnapshotBootstrapPolicy {
        enabled: true,
        audited_sha256: Some("00".repeat(32)),
        audited_height: Some(2),
    };
    let (provisional, _) = Kura::new_with_configured_lane_catalog_and_snapshot_bootstrap(
        &config,
        &lane_config,
        &catalog,
        &policy,
    )
    .expect("provisional startup ignores the unauthenticated merge log");
    assert!(provisional.provisional_snapshot_bootstrap_pending());
    drop(provisional);
    assert_eq!(
        fs::read(&merge_path).expect("read retained partial merge tail"),
        merge_bytes,
    );

    let intent = seal_prune_intent_fixture(KuraPruneIntentV2 {
        version: 2,
        source_height: 2,
        source_tip_hash: Some(snapshot_tail_hash),
        target_height: 1,
        target_tip_hash: Some(first_hash),
        retained_merge_entries: 0,
        retained_merge_tip_hash: None,
        sidecar_rewrite: KuraPruneSidecarRewriteProjectionV2::none(),
        capacity: unsealed_prune_capacity_fixture(),
    });
    let intent_bytes = norito::to_bytes(&intent).expect("encode provisional prune intent");
    let intent_path = Kura::prune_intent_path_for(temp_dir.path());
    let temp_intent_path = Kura::prune_intent_temp_path_for(temp_dir.path());
    fs::write(&temp_intent_path, &intent_bytes).expect("write temporary prune intent");
    fs::hard_link(&temp_intent_path, &intent_path)
        .expect("construct canonical stable+temp prune publication crash window");
    Kura::new_with_configured_lane_catalog_and_snapshot_bootstrap(
        &config,
        &lane_config,
        &catalog,
        &policy,
    )
    .expect_err("provisional startup rejects pending prune recovery");
    assert_eq!(
        fs::read(&intent_path).expect("read retained canonical prune intent"),
        intent_bytes,
    );
    assert_eq!(
        fs::read(&temp_intent_path).expect("read retained temporary prune intent"),
        intent_bytes,
    );
    assert_eq!(
        fs::read(&merge_path).expect("read merge tail after prune rejection"),
        merge_bytes,
    );
}

#[test]
fn invalid_catalog_fails_before_process_residue_cleanup() {
    let temp_dir = TempDir::new().expect("invalid-catalog temp-recovery directory");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let accepted = configured_primary_catalog("temp-recovery-accepted");
    let accepted_config = RuntimeLaneConfig::from_catalog(&accepted);
    let (kura, _) = Kura::new_with_configured_lane_catalog(&config, &accepted_config, &accepted)
        .expect("establish accepted configured catalog");
    publish_configured_catalog_baseline(&kura, &accepted);
    drop(kura);

    let residue = temp_dir.path().join(format!(
        "{AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX}invalid-catalog"
    ));
    let residue_bytes = b"catalog-rejected process residue remains untouched";
    fs::write(&residue, residue_bytes).expect("write process residue before catalog rejection");
    let rejected = configured_primary_catalog("temp-recovery-rejected");
    let rejected_config = RuntimeLaneConfig::from_catalog(&rejected);
    Kura::new_with_configured_lane_catalog(&config, &rejected_config, &rejected)
        .expect_err("configured catalog drift must fail before residue cleanup");
    assert_eq!(
        fs::read(&residue).expect("read retained catalog-rejected residue"),
        residue_bytes,
    );
}

#[test]
fn debug_block_dump_is_pinned_single_link_and_counted() {
    let temp_dir = TempDir::new().expect("bound debug-dump temp dir");
    let mut config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    config.debug_output_new_blocks = true;
    let (kura, _) = Kura::new(&config, &RuntimeLaneConfig::default())
        .expect("initialize bound debug-dump Kura");
    let path = kura
        .block_plain_text_path
        .lock()
        .clone()
        .expect("debug block dump path");
    let parent = path.parent().expect("debug block dump parent");
    let bytes = b"{\"debug\":true}\n";
    assert_eq!(Kura::blocks_root_debug_file_bytes(parent).unwrap(), 0);
    assert_eq!(
        kura.append_bound_debug_block_dump(&path, bytes)
            .expect("create bound debug block dump"),
        (0, bytes.len() as u64),
    );
    assert_eq!(
        Kura::blocks_root_debug_file_bytes(parent).expect("count debug block dump"),
        bytes.len() as u64,
    );

    let hardlink = parent.join("blocks-jsonl-hardlink");
    fs::hard_link(&path, &hardlink).expect("hard-link debug block dump");
    assert!(
        Kura::blocks_root_debug_file_bytes(parent).is_err()
            && kura.append_bound_debug_block_dump(&path, bytes).is_err(),
        "multiply linked debug output must fail closed",
    );
    assert_eq!(fs::read(&path).expect("read unchanged linked dump"), bytes);
    fs::remove_file(&hardlink).expect("remove debug dump hardlink");
    fs::remove_file(&path).expect("remove regular debug dump");

    fs::create_dir(&path).expect("replace debug dump with directory");
    assert!(
        Kura::blocks_root_debug_file_bytes(parent).is_err()
            && kura.append_bound_debug_block_dump(&path, bytes).is_err(),
        "non-regular debug output must fail closed",
    );
    fs::remove_dir(&path).expect("remove debug dump directory");

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let target = parent.join("blocks-jsonl-symlink-target");
        fs::write(&target, b"victim").expect("write debug symlink target");
        symlink(&target, &path).expect("symlink debug block dump");
        assert!(
            Kura::blocks_root_debug_file_bytes(parent).is_err()
                && kura.append_bound_debug_block_dump(&path, bytes).is_err(),
            "symlinked debug output must fail closed",
        );
        assert_eq!(
            fs::read(&target).expect("read untouched symlink target"),
            b"victim"
        );
    }
}

#[test]
fn debug_block_dump_reserves_capacity_before_first_creation() {
    let temp_dir = TempDir::new().expect("debug capacity temp dir");
    let mut config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    config.debug_output_new_blocks = true;
    let (mut kura, _) =
        Kura::new(&config, &RuntimeLaneConfig::default()).expect("initialize debug capacity Kura");
    let baseline = kura
        .kura_disk_usage_bytes()
        .expect("measure debug baseline");
    let path = kura
        .block_plain_text_path
        .lock()
        .clone()
        .expect("debug capacity path");
    Arc::get_mut(&mut kura)
        .expect("exclusive debug capacity Kura")
        .max_disk_usage_bytes = baseline;

    kura.append_debug_block_dump(&DummyBlocks::new().next());
    assert!(
        !path.exists(),
        "debug output must not be created without capacity for its first line",
    );
    assert_eq!(
        kura.kura_disk_usage_bytes()
            .expect("remeasure debug capacity baseline"),
        baseline,
    );
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "ios",
    target_os = "tvos",
    target_os = "visionos",
    target_os = "watchos",
    target_os = "redox",
))]
#[test]
fn exact_object_quarantine_retains_a_swapped_entry_instead_of_deleting_it() {
    let temp_dir = TempDir::new().expect("exact-object quarantine temp dir");
    let expected = b"classified atomic publication residue";
    let replacement = b"path-swapped replacement";
    let path = temp_dir.path().join(format!(
        "{AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX}classified"
    ));
    fs::write(&path, expected).expect("write classified residue");
    let (namespace, file, metadata, bound) = Kura::bind_autonomous_publication_temporary(
        temp_dir.path(),
        &path,
        4 * 1024,
        "test atomic temporary",
    )
    .expect("bind classified residue");
    let displaced = temp_dir.path().join("displaced-classified-residue");
    fs::rename(&path, &displaced).expect("displace classified residue after binding");
    fs::write(&path, replacement).expect("install path-swapped replacement");

    assert!(
        Kura::quarantine_and_retain_bound_autonomous_publication_temporary(
            temp_dir.path(),
            &namespace,
            &path,
            &file,
            &metadata,
            &bound,
            4 * 1024,
            AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX,
            "test atomic temporary",
        )
        .is_err(),
        "a quarantined inode mismatch must fail closed",
    );
    let quarantine = temp_dir.path().join(format!(
        "{AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX}quarantine-{}",
        Hash::new(&bound)
    ));
    assert_eq!(
        fs::read(&quarantine).expect("replacement remains quarantined for inspection"),
        replacement,
    );
    assert_eq!(
        fs::read(&displaced).expect("classified held object survives mismatch"),
        expected,
    );
    assert!(!path.exists());
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "ios",
    target_os = "tvos",
    target_os = "visionos",
    target_os = "watchos",
    target_os = "redox",
))]
#[test]
fn exact_object_quarantine_uses_the_pinned_parent_and_preserves_replacement_paths() {
    let temp_dir = TempDir::new().expect("pinned-parent quarantine temp dir");
    let parent = temp_dir.path().join("bound-parent");
    fs::create_dir(&parent).expect("create bound parent");
    let expected = b"classified residue in bound parent";
    let victim = b"replacement-path victim";
    let path = parent.join(format!(
        "{AUTONOMOUS_LIFECYCLE_BOOTSTRAP_ATOMIC_TEMP_PREFIX}classified"
    ));
    fs::write(&path, expected).expect("write classified residue");
    let (namespace, file, metadata, bound) = Kura::bind_autonomous_publication_temporary(
        temp_dir.path(),
        &path,
        4 * 1024,
        "test bootstrap temporary",
    )
    .expect("bind classified residue and parent");
    let displaced_parent = temp_dir.path().join("displaced-bound-parent");
    fs::rename(&parent, &displaced_parent).expect("displace bound parent after binding");
    fs::create_dir(&parent).expect("create replacement parent path");
    fs::write(&path, victim).expect("install replacement-path victim");

    assert!(
        Kura::quarantine_and_retain_bound_autonomous_publication_temporary(
            temp_dir.path(),
            &namespace,
            &path,
            &file,
            &metadata,
            &bound,
            4 * 1024,
            AUTONOMOUS_LIFECYCLE_BOOTSTRAP_ATOMIC_TEMP_PREFIX,
            "test bootstrap temporary",
        )
        .is_err(),
        "a replaced parent path must fail closed after pinned quarantine",
    );
    let quarantine = displaced_parent.join(format!(
        "{AUTONOMOUS_LIFECYCLE_BOOTSTRAP_ATOMIC_TEMP_PREFIX}quarantine-{}",
        Hash::new(&bound)
    ));
    assert_eq!(
        fs::read(&quarantine).expect("classified object remains in pinned quarantine"),
        expected,
    );
    assert_eq!(
        fs::read(&path).expect("replacement path is never deleted"),
        victim,
    );
}

#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "ios",
    target_os = "tvos",
    target_os = "visionos",
    target_os = "watchos",
    target_os = "redox",
))]
#[test]
fn exact_object_quarantine_retains_bounded_repeated_identical_residues() {
    let temp_dir = TempDir::new().expect("repeated quarantine temp dir");
    let bytes = b"the same publication residue from repeated crashes";
    let current_name =
        format!("{AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX}repeated");
    let path = temp_dir.path().join(&current_name);

    for _ in 0..2 {
        fs::write(&path, bytes).expect("recreate identical publication residue");
        let (namespace, file, metadata, bound) = Kura::bind_autonomous_publication_temporary(
            temp_dir.path(),
            &path,
            4 * 1024,
            "repeated test temporary",
        )
        .expect("bind repeated publication residue");
        Kura::retain_bound_autonomous_publication_temporary_as_quarantine(
            temp_dir.path(),
            &namespace,
            &path,
            file,
            &metadata,
            &bound,
            4 * 1024,
            AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX,
            "repeated test temporary",
        )
        .expect("retain repeated publication residue");
    }

    let primary_name = format!(
        "{AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX}quarantine-{}",
        Hash::new(bytes),
    );
    let secondary_name = format!("{primary_name}-{}", Hash::new(current_name.as_bytes()));
    let quarantines = publication_quarantines(
        temp_dir.path(),
        AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX,
    );
    assert_eq!(
        quarantines,
        vec![
            temp_dir.path().join(primary_name),
            temp_dir.path().join(secondary_name),
        ],
    );
    for quarantine in &quarantines {
        assert_retained_publication_quarantine(quarantine, bytes);
    }

    fs::write(&path, bytes).expect("recreate residue beyond bounded multiplicity");
    let (namespace, file, metadata, bound) = Kura::bind_autonomous_publication_temporary(
        temp_dir.path(),
        &path,
        4 * 1024,
        "bounded repeated test temporary",
    )
    .expect("bind residue beyond bounded multiplicity");
    assert!(
        Kura::retain_bound_autonomous_publication_temporary_as_quarantine(
            temp_dir.path(),
            &namespace,
            &path,
            file,
            &metadata,
            &bound,
            4 * 1024,
            AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX,
            "bounded repeated test temporary",
        )
        .is_err(),
        "bounded multiplicity exhaustion must fail closed",
    );
    assert!(path.exists());
    assert_eq!(
        publication_quarantines(
            temp_dir.path(),
            AUTONOMOUS_LIFECYCLE_PROCESS_GENERATION_ATOMIC_TEMP_PREFIX,
        ),
        quarantines,
    );
}
