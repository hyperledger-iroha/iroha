fn persist_historical_atomic_temp_dependencies(kura: &Kura, payload: &LaneExecutablePayloadV1) {
    persist_historical_capacity_payload_fixture(kura, payload);
    let recovered = kura
        .recover_autonomous_lane_block_payload(
            &payload.origin_proposal,
            payload.chain_id_hash,
            payload.epoch,
        )
        .expect("recover historical atomic-temp execution input");
    kura.persist_lane_block_execution_input(&recovered)
        .expect("persist historical atomic-temp execution input");
}

fn write_historical_atomic_temp_fixture(
    directory: &Path,
    name: &str,
    record: &HistoricalAutonomousLaneRecoveryRecordV1,
) -> PathBuf {
    std::fs::create_dir_all(directory).expect("create historical atomic-temp directory");
    let path = directory.join(name);
    std::fs::write(&path, historical_autonomous_recovery_record_bytes(record))
        .expect("write historical atomic-temp fixture");
    path
}

#[test]
fn historical_atomic_temp_fault_and_legacy_residue_recover_before_startup_inventory() {
    let temp_dir = TempDir::new().expect("historical atomic-temp fault directory");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (_, _, first_payload) = historical_capacity_payload_for_kura(
        lane.lane_id,
        lane.dataspace_id,
        1,
        "atomic-temp-fault",
        &signer,
    );
    let (_, _, second_payload) = historical_capacity_payload_for_kura(
        lane.lane_id,
        lane.dataspace_id,
        2,
        "atomic-temp-legacy",
        &signer,
    );
    let first = historical_autonomous_recovery_record_for_kura(
        &first_payload,
        &signer,
        b"atomic-temp-fault",
    );
    let second = historical_autonomous_recovery_record_for_kura(
        &second_payload,
        &signer,
        b"atomic-temp-legacy",
    );
    let (kura, _) = Kura::new(&config, &lane_config).expect("historical atomic-temp Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &first_payload);
    persist_historical_atomic_temp_dependencies(&kura, &first_payload);
    persist_historical_atomic_temp_dependencies(&kura, &second_payload);

    kura.fail_next_atomic_write_after_temporary_sync_for_test();
    assert!(
        kura.persist_historical_autonomous_lane_recovery_records(std::slice::from_ref(&first))
            .is_err(),
        "fault after temporary fsync must retain the dedicated publication residue",
    );
    let directory = Kura::historical_autonomous_recovery_directory_for_entry(lane, temp_dir.path());
    let dedicated_temp = std::fs::read_dir(&directory)
        .expect("read dedicated historical residue directory")
        .map(|entry| entry.expect("read dedicated historical residue").path())
        .find(|path| {
            path.file_name()
                .and_then(std::ffi::OsStr::to_str)
                .is_some_and(|name| {
                    name.starts_with(HISTORICAL_AUTONOMOUS_RECOVERY_ATOMIC_TEMP_PREFIX)
                })
        })
        .expect("dedicated historical publication residue exists");
    let legacy_temp = write_historical_atomic_temp_fixture(
        &directory,
        &format!("{LEGACY_HISTORICAL_AUTONOMOUS_RECOVERY_ATOMIC_TEMP_PREFIX}legacy-fixture"),
        &second,
    );
    let first_stable = Kura::historical_autonomous_recovery_path_for_entry(
        lane,
        temp_dir.path(),
        first.recovery_id,
    );
    let second_stable = Kura::historical_autonomous_recovery_path_for_entry(
        lane,
        temp_dir.path(),
        second.recovery_id,
    );
    assert!(!first_stable.exists() && !second_stable.exists());
    drop(kura);

    let (reopened, _) = Kura::new(&config, &lane_config)
        .expect("startup authenticates and promotes dedicated plus legacy residues");
    assert_eq!(
        std::fs::read(&first_stable).expect("read recovered dedicated stable seal"),
        historical_autonomous_recovery_record_bytes(&first),
    );
    assert_eq!(
        std::fs::read(&second_stable).expect("read recovered legacy stable seal"),
        historical_autonomous_recovery_record_bytes(&second),
    );
    assert!(!dedicated_temp.exists() && !legacy_temp.exists());
    drop(reopened);

    let (reopened_again, _) = Kura::new(&config, &lane_config)
        .expect("historical atomic-temp recovery is restart-idempotent");
    assert!(
        reopened_again
            .historical_autonomous_lane_recovery_record_matches(&first)
            .expect("revalidate dedicated recovered record"),
    );
    assert!(
        reopened_again
            .historical_autonomous_lane_recovery_record_matches(&second)
            .expect("revalidate legacy recovered record"),
    );
}

#[test]
fn historical_atomic_temp_cleans_exact_duplicate_and_two_link_publication_retry() {
    let temp_dir = TempDir::new().expect("historical atomic duplicate directory");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (_, _, duplicate_payload) = historical_capacity_payload_for_kura(
        lane.lane_id,
        lane.dataspace_id,
        1,
        "atomic-temp-duplicate",
        &signer,
    );
    let (_, _, linked_payload) = historical_capacity_payload_for_kura(
        lane.lane_id,
        lane.dataspace_id,
        2,
        "atomic-temp-linked",
        &signer,
    );
    let duplicate = historical_autonomous_recovery_record_for_kura(
        &duplicate_payload,
        &signer,
        b"atomic-temp-duplicate",
    );
    let linked = historical_autonomous_recovery_record_for_kura(
        &linked_payload,
        &signer,
        b"atomic-temp-linked",
    );
    let (kura, _) = Kura::new(&config, &lane_config).expect("historical duplicate Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &duplicate_payload);
    persist_historical_atomic_temp_dependencies(&kura, &duplicate_payload);
    persist_historical_atomic_temp_dependencies(&kura, &linked_payload);
    kura.persist_historical_autonomous_lane_recovery_records(&[duplicate.clone(), linked.clone()])
        .expect("persist stable historical duplicate fixtures");
    drop(kura);

    let directory = Kura::historical_autonomous_recovery_directory_for_entry(lane, temp_dir.path());
    let duplicate_temp = write_historical_atomic_temp_fixture(
        &directory,
        &format!("{HISTORICAL_AUTONOMOUS_RECOVERY_ATOMIC_TEMP_PREFIX}separate-duplicate"),
        &duplicate,
    );
    let linked_stable = Kura::historical_autonomous_recovery_path_for_entry(
        lane,
        temp_dir.path(),
        linked.recovery_id,
    );
    let linked_temp = directory.join(format!(
        "{HISTORICAL_AUTONOMOUS_RECOVERY_ATOMIC_TEMP_PREFIX}linked-retry"
    ));
    std::fs::hard_link(&linked_stable, &linked_temp)
        .expect("create stable/temporary two-link publication boundary");

    let (reopened, _) = Kura::new(&config, &lane_config)
        .expect("startup cleans exact and two-link historical retries");
    assert!(!duplicate_temp.exists() && !linked_temp.exists());
    for record in [&duplicate, &linked] {
        assert!(
            reopened
                .historical_autonomous_lane_recovery_record_matches(record)
                .expect("revalidate historical duplicate cleanup"),
        );
    }
    assert!(Kura::sidecar_is_single_link(
        &std::fs::symlink_metadata(&linked_stable)
            .expect("read cleaned linked historical stable metadata"),
    ));
}

#[test]
fn historical_atomic_temp_whole_inventory_preflight_prevents_partial_promotion() {
    let temp_dir = TempDir::new().expect("historical whole-inventory directory");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (_, _, payload) = historical_capacity_payload_for_kura(
        lane.lane_id,
        lane.dataspace_id,
        1,
        "atomic-temp-whole-inventory",
        &signer,
    );
    let record = historical_autonomous_recovery_record_for_kura(
        &payload,
        &signer,
        b"atomic-temp-whole-inventory",
    );
    let (kura, _) = Kura::new(&config, &lane_config).expect("historical preflight Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    persist_historical_atomic_temp_dependencies(&kura, &payload);
    let directory = Kura::historical_autonomous_recovery_directory_for_entry(lane, temp_dir.path());
    let valid_temp = write_historical_atomic_temp_fixture(
        &directory,
        &format!("{HISTORICAL_AUTONOMOUS_RECOVERY_ATOMIC_TEMP_PREFIX}a-valid"),
        &record,
    );
    let malformed_temp = directory.join(format!(
        "{HISTORICAL_AUTONOMOUS_RECOVERY_ATOMIC_TEMP_PREFIX}z-malformed"
    ));
    std::fs::write(&malformed_temp, b"not historical recovery Norito")
        .expect("write late malformed historical residue");
    let valid_bytes = std::fs::read(&valid_temp).expect("snapshot valid historical residue");
    let stable = Kura::historical_autonomous_recovery_path_for_entry(
        lane,
        temp_dir.path(),
        record.recovery_id,
    );
    drop(kura);

    assert!(
        Kura::new(&config, &lane_config).is_err(),
        "a late malformed item must reject the complete startup reconciliation",
    );
    assert!(!stable.exists());
    assert_eq!(
        std::fs::read(&valid_temp).expect("valid residue remains after rejected preflight"),
        valid_bytes,
    );
    assert!(malformed_temp.exists());
}

#[test]
fn historical_atomic_temp_rejects_multiple_names_for_one_target_before_mutation() {
    let temp_dir = TempDir::new().expect("duplicate-temp historical residue directory");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (_, _, payload) = historical_capacity_payload_for_kura(
        lane.lane_id,
        lane.dataspace_id,
        1,
        "atomic-temp-duplicate-target",
        &signer,
    );
    let record = historical_autonomous_recovery_record_for_kura(
        &payload,
        &signer,
        b"atomic-temp-duplicate-target",
    );
    let (kura, _) = Kura::new(&config, &lane_config).expect("duplicate-temp residue Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    let directory = Kura::historical_autonomous_recovery_directory_for_entry(lane, temp_dir.path());
    let first = write_historical_atomic_temp_fixture(
        &directory,
        &format!("{HISTORICAL_AUTONOMOUS_RECOVERY_ATOMIC_TEMP_PREFIX}duplicate-target-a"),
        &record,
    );
    let second = write_historical_atomic_temp_fixture(
        &directory,
        &format!("{HISTORICAL_AUTONOMOUS_RECOVERY_ATOMIC_TEMP_PREFIX}duplicate-target-b"),
        &record,
    );
    let first_bytes = std::fs::read(&first).expect("snapshot first duplicate temp");
    let second_bytes = std::fs::read(&second).expect("snapshot second duplicate temp");
    assert!(
        kura.reconcile_historical_autonomous_recovery_atomic_temps_on_startup()
            .is_err(),
    );
    assert_eq!(
        std::fs::read(&first).expect("first duplicate temp remains"),
        first_bytes,
    );
    assert_eq!(
        std::fs::read(&second).expect("second duplicate temp remains"),
        second_bytes,
    );
    assert!(
        !Kura::historical_autonomous_recovery_path_for_entry(
            lane,
            temp_dir.path(),
            record.recovery_id,
        )
        .exists(),
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn historical_atomic_temp_rejects_oversize_symlink_and_extraneous_hardlinks() {
    {
        let temp_dir = TempDir::new().expect("oversized historical residue directory");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = two_lane_runtime_config();
        let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
        let (kura, _) = Kura::new(&config, &lane_config).expect("oversized residue Kura");
        let directory =
            Kura::historical_autonomous_recovery_directory_for_entry(lane, temp_dir.path());
        std::fs::create_dir_all(&directory).expect("create oversized residue directory");
        let oversized = directory.join(format!(
            "{HISTORICAL_AUTONOMOUS_RECOVERY_ATOMIC_TEMP_PREFIX}oversized"
        ));
        let file = std::fs::File::create(&oversized).expect("create oversized residue");
        file.set_len(
            u64::try_from(HISTORICAL_AUTONOMOUS_RECOVERY_RECORD_MAX_BYTES)
                .expect("historical record max fits u64")
                + 1,
        )
        .expect("size oversized residue");
        drop(file);
        drop(kura);
        assert!(Kura::new(&config, &lane_config).is_err());
        assert!(oversized.exists());
    }

    #[cfg(unix)]
    {
        let temp_dir = TempDir::new().expect("symlink historical residue directory");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = two_lane_runtime_config();
        let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
        let (kura, _) = Kura::new(&config, &lane_config).expect("symlink residue Kura");
        let directory =
            Kura::historical_autonomous_recovery_directory_for_entry(lane, temp_dir.path());
        std::fs::create_dir_all(&directory).expect("create symlink residue directory");
        let target = temp_dir.path().join("historical-symlink-target");
        std::fs::write(&target, b"forbidden symlink target").expect("write symlink target");
        let symlink = directory.join(format!(
            "{HISTORICAL_AUTONOMOUS_RECOVERY_ATOMIC_TEMP_PREFIX}symlink"
        ));
        std::os::unix::fs::symlink(&target, &symlink).expect("create historical residue symlink");
        drop(kura);
        assert!(Kura::new(&config, &lane_config).is_err());
        assert!(
            std::fs::symlink_metadata(&symlink)
                .expect("symlink residue retained")
                .file_type()
                .is_symlink(),
        );
    }

    {
        let temp_dir = TempDir::new().expect("hardlink historical residue directory");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = two_lane_runtime_config();
        let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
        let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
        let (_, _, payload) = historical_capacity_payload_for_kura(
            lane.lane_id,
            lane.dataspace_id,
            1,
            "atomic-temp-extraneous-hardlink",
            &signer,
        );
        let record = historical_autonomous_recovery_record_for_kura(
            &payload,
            &signer,
            b"atomic-temp-extraneous-hardlink",
        );
        let (kura, _) = Kura::new(&config, &lane_config).expect("hardlink residue Kura");
        install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
        let directory =
            Kura::historical_autonomous_recovery_directory_for_entry(lane, temp_dir.path());
        let first = write_historical_atomic_temp_fixture(
            &directory,
            &format!("{HISTORICAL_AUTONOMOUS_RECOVERY_ATOMIC_TEMP_PREFIX}hardlink-a"),
            &record,
        );
        let second = directory.join(format!(
            "{HISTORICAL_AUTONOMOUS_RECOVERY_ATOMIC_TEMP_PREFIX}hardlink-b"
        ));
        std::fs::hard_link(&first, &second).expect("create extraneous temporary hardlink pair");
        drop(kura);
        assert!(Kura::new(&config, &lane_config).is_err());
        assert!(first.exists() && second.exists());
    }
}

#[test]
fn historical_atomic_temp_rejects_collision_and_stale_incarnation_without_mutation() {
    {
        let temp_dir = TempDir::new().expect("conflicting historical residues directory");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = two_lane_runtime_config();
        let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
        let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
        let (_, _, first_payload) = historical_capacity_payload_for_kura(
            lane.lane_id,
            lane.dataspace_id,
            1,
            "atomic-temp-conflict-a",
            &signer,
        );
        let (_, _, second_payload) = historical_capacity_payload_for_kura(
            lane.lane_id,
            lane.dataspace_id,
            1,
            "atomic-temp-conflict-b",
            &signer,
        );
        let first = historical_autonomous_recovery_record_for_kura(
            &first_payload,
            &signer,
            b"atomic-temp-conflict-a",
        );
        let second = historical_autonomous_recovery_record_for_kura(
            &second_payload,
            &signer,
            b"atomic-temp-conflict-b",
        );
        let (kura, _) = Kura::new(&config, &lane_config).expect("conflicting residues Kura");
        install_autonomous_lane_marker_for_kura(&kura, &lane_config, &first_payload);
        let directory =
            Kura::historical_autonomous_recovery_directory_for_entry(lane, temp_dir.path());
        let first_temp = write_historical_atomic_temp_fixture(
            &directory,
            &format!("{HISTORICAL_AUTONOMOUS_RECOVERY_ATOMIC_TEMP_PREFIX}conflict-a"),
            &first,
        );
        let second_temp = write_historical_atomic_temp_fixture(
            &directory,
            &format!("{HISTORICAL_AUTONOMOUS_RECOVERY_ATOMIC_TEMP_PREFIX}conflict-b"),
            &second,
        );
        assert!(
            kura.reconcile_historical_autonomous_recovery_atomic_temps_on_startup()
                .is_err(),
        );
        assert!(first_temp.exists() && second_temp.exists());
        assert!(
            !Kura::historical_autonomous_recovery_path_for_entry(
                lane,
                temp_dir.path(),
                first.recovery_id,
            )
            .exists(),
        );
    }

    {
        let temp_dir = TempDir::new().expect("stale historical residue directory");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = two_lane_runtime_config();
        let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
        let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
        let (_, _, payload) = historical_capacity_payload_for_kura(
            lane.lane_id,
            lane.dataspace_id,
            1,
            "atomic-temp-stale-incarnation",
            &signer,
        );
        let record = historical_autonomous_recovery_record_for_kura(
            &payload,
            &signer,
            b"atomic-temp-stale-incarnation",
        );
        let (kura, _) = Kura::new(&config, &lane_config).expect("stale residue Kura");
        install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
        persist_historical_atomic_temp_dependencies(&kura, &payload);
        let directory =
            Kura::historical_autonomous_recovery_directory_for_entry(lane, temp_dir.path());
        let temporary = write_historical_atomic_temp_fixture(
            &directory,
            &format!("{HISTORICAL_AUTONOMOUS_RECOVERY_ATOMIC_TEMP_PREFIX}stale"),
            &record,
        );
        kura.install_lane_incarnation_marker_for_test(
            lane,
            Hash::new(b"historical-atomic-temp-recreated-incarnation"),
            0,
        )
        .expect("install recreated active incarnation marker");
        assert!(
            kura.reconcile_historical_autonomous_recovery_atomic_temps_on_startup()
                .is_err(),
        );
        assert!(temporary.exists());
        assert!(
            !Kura::historical_autonomous_recovery_path_for_entry(
                lane,
                temp_dir.path(),
                record.recovery_id,
            )
            .exists(),
        );
    }
}
