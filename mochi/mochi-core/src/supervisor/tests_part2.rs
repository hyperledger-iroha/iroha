#[test]
fn restore_snapshot_replaces_storage_and_configs() {
    if !ports_available("restore_snapshot_replaces_storage_and_configs") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");

    let peer = &supervisor.peers()[0];
    let storage_dir = peer.storage_dir().to_path_buf();
    let snapshot_dir = peer.snapshot_dir().to_path_buf();
    let config_path = peer.config_path().to_path_buf();
    let log_path = peer.log_path().to_path_buf();
    let genesis_path = supervisor.genesis_manifest().to_path_buf();
    let genesis_block_path = supervisor.genesis_block_file().to_path_buf();

    fs::write(storage_dir.join("marker.txt"), b"snapshot-data").expect("write storage marker");
    fs::write(snapshot_dir.join("inner.txt"), b"snapshot-inner").expect("write snapshot file");
    let original_config = fs::read(&config_path).expect("read original config");
    if let Some(parent) = log_path.parent() {
        fs::create_dir_all(parent).expect("create log directory");
    }
    fs::write(&log_path, b"snapshot-log").expect("write log file");

    let snapshot_root = supervisor
        .export_snapshot(Some("Restore Demo 2026"))
        .expect("export snapshot");

    fs::write(storage_dir.join("marker.txt"), b"mutated-storage")
        .expect("overwrite storage marker");
    fs::remove_file(snapshot_dir.join("inner.txt")).expect("remove snapshot file");
    fs::write(&config_path, b"mutated-config").expect("mutate config");
    fs::write(&log_path, b"mutated-log").expect("mutate log");

    supervisor
        .restore_snapshot(&snapshot_root)
        .expect("restore snapshot by path");

    assert_eq!(
        fs::read(storage_dir.join("marker.txt")).expect("read storage marker after restore"),
        b"snapshot-data"
    );
    assert!(
        snapshot_dir.join("inner.txt").exists(),
        "snapshot directory should be restored"
    );
    assert_eq!(
        fs::read(&config_path).expect("read restored config"),
        original_config
    );
    assert_eq!(
        fs::read(&log_path).expect("read restored log"),
        b"snapshot-log"
    );
    assert_eq!(
        fs::read(&genesis_path).expect("read restored genesis"),
        fs::read(snapshot_root.join("genesis").join(GENESIS_FILE_NAME))
            .expect("read snapshot genesis")
    );
    assert_eq!(
        fs::read(&genesis_block_path).expect("read restored signed genesis"),
        fs::read(snapshot_root.join("genesis").join(GENESIS_SIGNED_FILE_NAME))
            .expect("read snapshot signed genesis")
    );

    let snapshot_name = snapshot_root
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    fs::write(storage_dir.join("marker.txt"), b"mutated-again").expect("mutate storage");
    supervisor
        .restore_snapshot(snapshot_name.as_str())
        .expect("restore snapshot by label");
    assert_eq!(
        fs::read(storage_dir.join("marker.txt")).expect("read storage marker"),
        b"snapshot-data"
    );
}

#[test]
fn restore_snapshot_rejects_genesis_hash_mismatch() {
    if !ports_available("restore_snapshot_rejects_genesis_hash_mismatch") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");

    let snapshot_root = supervisor
        .export_snapshot(Some("Genesis Hash Mismatch"))
        .expect("export snapshot");

    fs::write(supervisor.genesis_manifest(), b"mutated-genesis").expect("mutate genesis");

    let err = supervisor
        .restore_snapshot(&snapshot_root)
        .expect_err("restore should fail when genesis hash mismatches");
    match err {
        SupervisorError::Config(message) => assert!(
            message.contains("genesis hash"),
            "expected genesis hash mismatch message, got `{message}`"
        ),
        other => panic!("expected SupervisorError::Config, got {other:?}"),
    }
}

#[test]
fn restore_snapshot_rejects_kura_hash_tampering() {
    if !ports_available("restore_snapshot_rejects_kura_hash_tampering") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");

    let snapshot_root = supervisor
        .export_snapshot(Some("Kura Tamper"))
        .expect("export snapshot");
    let alias = supervisor.peers()[0].alias().to_owned();
    let storage_copy = snapshot_root.join("peers").join(&alias).join("storage");
    fs::write(storage_copy.join("tamper.bin"), b"tampered").expect("mutate snapshot storage");

    let err = supervisor
        .restore_snapshot(&snapshot_root)
        .expect_err("restore should fail when kura hash mismatches metadata");
    match err {
        SupervisorError::Config(message) => assert!(
            message.contains("integrity check") && message.contains(&alias),
            "expected kura integrity error mentioning alias; got `{message}`"
        ),
        other => panic!("expected SupervisorError::Config, got {other:?}"),
    }
}

#[test]
fn restore_snapshot_rejects_chain_mismatch() {
    if !ports_available("restore_snapshot_rejects_chain_mismatch") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");

    let snapshot_root = supervisor
        .export_snapshot(Some("Chain Mismatch"))
        .expect("export snapshot");
    let metadata_path = snapshot_root.join("metadata.json");
    let mut metadata: Value =
        norito::json::from_slice(&fs::read(&metadata_path).expect("read metadata"))
            .expect("parse metadata");
    metadata
        .as_object_mut()
        .expect("metadata should be an object")
        .insert("chain_id".into(), Value::String("other-chain".into()));
    fs::write(
        &metadata_path,
        json::to_vec_pretty(&metadata).expect("serialize metadata"),
    )
    .expect("write mutated metadata");

    fs::write(
        supervisor.peers()[0].storage_dir().join("marker.txt"),
        b"mutated",
    )
    .expect("mutate storage");

    let err = supervisor
        .restore_snapshot(&snapshot_root)
        .expect_err("restore should fail when chains mismatch");
    match err {
        SupervisorError::Config(message) => assert!(
            message.contains("other-chain"),
            "expected chain mismatch message, got `{message}`"
        ),
        other => panic!("expected SupervisorError::Config, got {other:?}"),
    }
}

#[test]
fn restore_snapshot_rejects_missing_storage_layout() {
    if !ports_available("restore_snapshot_rejects_missing_storage_layout") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");
    let snapshot_root = supervisor
        .export_snapshot(Some("Legacy Storage Layout"))
        .expect("export snapshot");
    let metadata_path = snapshot_root.join("metadata.json");
    let mut metadata: Value = json::from_slice(&fs::read(&metadata_path).expect("read metadata"))
        .expect("parse metadata");
    metadata
        .as_object_mut()
        .expect("metadata object")
        .remove("storage_layout");
    fs::write(
        &metadata_path,
        json::to_vec_pretty(&metadata).expect("serialize metadata"),
    )
    .expect("write metadata");
    let live_sentinel = supervisor.peers()[0]
        .storage_dir()
        .join("live-layout-sentinel.bin");
    fs::write(&live_sentinel, b"live-state").expect("write live sentinel");

    let err = supervisor
        .restore_snapshot(&snapshot_root)
        .expect_err("unversioned storage layout must fail closed");
    match err {
        SupervisorError::Config(message) => assert!(
            message.contains("missing `storage_layout`")
                && message.contains("cannot be restored safely"),
            "unexpected error: {message}"
        ),
        other => panic!("expected SupervisorError::Config, got {other:?}"),
    }
    assert_eq!(
        fs::read(live_sentinel).expect("read live sentinel"),
        b"live-state",
        "layout rejection must happen before live storage is mutated"
    );
}

#[test]
fn restore_snapshot_rejects_unknown_storage_layout() {
    if !ports_available("restore_snapshot_rejects_unknown_storage_layout") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");
    let snapshot_root = supervisor
        .export_snapshot(Some("Unknown Storage Layout"))
        .expect("export snapshot");
    let metadata_path = snapshot_root.join("metadata.json");
    let mut metadata: Value = json::from_slice(&fs::read(&metadata_path).expect("read metadata"))
        .expect("parse metadata");
    metadata.as_object_mut().expect("metadata object").insert(
        "storage_layout".into(),
        Value::String("future-layout-v99".into()),
    );
    fs::write(
        &metadata_path,
        json::to_vec_pretty(&metadata).expect("serialize metadata"),
    )
    .expect("write metadata");
    let live_sentinel = supervisor.peers()[0]
        .storage_dir()
        .join("live-layout-sentinel.bin");
    fs::write(&live_sentinel, b"live-state").expect("write live sentinel");

    let err = supervisor
        .restore_snapshot(&snapshot_root)
        .expect_err("unknown storage layout must fail closed");
    match err {
        SupervisorError::Config(message) => assert!(
            message.contains("unsupported storage layout `future-layout-v99`")
                && message.contains(SNAPSHOT_STORAGE_LAYOUT),
            "unexpected error: {message}"
        ),
        other => panic!("expected SupervisorError::Config, got {other:?}"),
    }
    assert_eq!(
        fs::read(live_sentinel).expect("read live sentinel"),
        b"live-state",
        "layout rejection must happen before live storage is mutated"
    );
}

#[test]
fn supervisor_respects_explicit_kagami_override() {
    if !ports_available("supervisor_respects_explicit_kagami_override") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let stub = StandaloneKagamiStub::create(temp.path());
    SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .kagami_path(stub.script_path())
        .build()
        .expect("build supervisor with explicit kagami path");

    let log = fs::read_to_string(stub.log_path()).expect("explicit kagami log");
    assert!(
        log.contains("--genesis-public-key"),
        "expected explicit kagami stub to capture genesis args, got `{log}`"
    );
}

#[test]
fn supervisor_runs_kagami_verify_for_profile() {
    if !ports_available("supervisor_runs_kagami_verify_for_profile") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let stub = StandaloneKagamiStub::create(temp.path());
    let _guard = EnvVarGuard::set("MOCHI_KAGAMI", stub.script_path().as_os_str());

    let _supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .genesis_profile(GenesisProfile::Iroha3Dev)
        .build()
        .expect("build supervisor with kagami verify");

    let log = fs::read_to_string(stub.log_path()).expect("read kagami log");
    let lines: Vec<_> = log.lines().collect();
    assert!(
        lines.contains(&"genesis"),
        "expected kagami genesis invocation, got `{log}`"
    );
    assert!(
        lines.contains(&"generate"),
        "expected kagami generate invocation, got `{log}`"
    );
    assert!(
        lines.contains(&"verify"),
        "expected kagami verify invocation, got `{log}`"
    );
}

#[test]
fn existing_peer_directories_are_cleaned_before_build() {
    if !ports_available("existing_peer_directories_are_cleaned_before_build") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");

    let slug = NetworkProfile::from_preset(ProfilePreset::SinglePeer).slug();
    let root = temp.path().join(slug);
    let peer_dir = root.join("peers").join("peer0");
    let stale_file = peer_dir.join("stale.bin");
    fs::create_dir_all(&peer_dir).expect("create stale peer dir");
    fs::write(&stale_file, b"leftover").expect("write stale file");
    assert!(
        stale_file.exists(),
        "stale file should exist before supervisor build"
    );

    let _stub = KagamiStub::install(temp.path());
    let supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");

    assert!(
        !stale_file.exists(),
        "stale peer artefacts should be removed during build"
    );

    let rebuilt_storage = supervisor.peers()[0].spec.storage_dir.clone();
    assert!(
        rebuilt_storage.exists(),
        "storage directory should be recreated after cleanup"
    );
}

#[test]
fn wipe_and_regenerate_resets_storage_and_genesis() {
    if !ports_available("wipe_and_regenerate_resets_storage_and_genesis") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");

    for (idx, peer) in supervisor.peers().iter().enumerate() {
        let storage_file = peer.storage_dir().join(format!("leftover-{idx}.bin"));
        fs::write(&storage_file, b"stale").expect("write stale storage file");
        assert!(
            storage_file.exists(),
            "stale storage file should exist before wipe"
        );

        let snapshot_file = peer.snapshot_dir().join("stale.txt");
        fs::write(&snapshot_file, b"stale-snapshot").expect("write stale snapshot file");
        assert!(
            snapshot_file.exists(),
            "stale snapshot file should exist before wipe"
        );
    }

    let genesis_path = supervisor.genesis_manifest().to_path_buf();
    fs::write(&genesis_path, b"not-json").expect("corrupt genesis manifest");

    supervisor
        .wipe_and_regenerate()
        .expect("wipe and regenerate should succeed");

    let manifest_bytes = fs::read(&genesis_path).expect("read regenerated genesis");
    let manifest: Value =
        norito::json::from_slice(&manifest_bytes).expect("genesis should be valid JSON");
    assert_eq!(
        manifest
            .get("chain")
            .and_then(Value::as_str)
            .expect("chain field present"),
        supervisor.chain_id(),
        "regenerated genesis should carry supervisor chain id"
    );

    for (idx, peer) in supervisor.peers().iter().enumerate() {
        let storage_file = peer.storage_dir().join(format!("leftover-{idx}.bin"));
        assert!(
            !storage_file.exists(),
            "wipe should remove stale storage file for peer {}",
            peer.alias()
        );
        let snapshot_file = peer.snapshot_dir().join("stale.txt");
        assert!(
            !snapshot_file.exists(),
            "wipe should remove stale snapshot file for peer {}",
            peer.alias()
        );
        let generations = peer.snapshot_dir().join(SNAPSHOT_GENERATIONS_DIR_NAME);
        assert!(
            generations.is_dir(),
            "wipe should recreate the snapshot generations directory for peer {}",
            peer.alias()
        );
        assert!(
            fs::read_dir(generations)
                .expect("snapshot generations directory")
                .next()
                .is_none(),
            "wipe should leave snapshot generations empty for peer {}",
            peer.alias()
        );
    }
}

#[test]
fn genesis_topology_matches_peer_configuration_across_presets() {
    if !ports_available("genesis_topology_matches_peer_configuration_across_presets") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());

    for preset in [ProfilePreset::SinglePeer, ProfilePreset::FourPeerBft] {
        let supervisor = SupervisorBuilder::new(preset)
            .data_root(temp.path())
            .build()
            .expect("build supervisor");

        let bytes = fs::read(supervisor.genesis_manifest()).expect("genesis manifest readable");
        let manifest: norito::json::Value =
            norito::json::from_slice(&bytes).expect("parse genesis json");
        let transactions = manifest
            .get("transactions")
            .and_then(norito::json::Value::as_array)
            .expect("transactions array");
        let topology = transactions
            .iter()
            .filter_map(|tx| tx.get("topology").and_then(norito::json::Value::as_array))
            .find(|entries| !entries.is_empty())
            .expect("non-empty topology transaction present");

        let actual_peer_ids: Vec<PeerId> = topology
            .iter()
            .map(|entry| {
                let topology_entry: GenesisTopologyEntry =
                    norito::json::from_value(entry.clone()).expect("topology entry should decode");
                topology_entry.peer
            })
            .collect();
        let expected_peer_ids: Vec<PeerId> = supervisor
            .peers()
            .iter()
            .map(|peer| peer.peer_id())
            .collect();

        assert_eq!(
            actual_peer_ids, expected_peer_ids,
            "topology should mirror prepared peers for preset {preset:?}"
        );

        let chain = manifest
            .get("chain")
            .and_then(norito::json::Value::as_str)
            .expect("chain field");
        assert_eq!(
            chain,
            supervisor.chain_id(),
            "manifest chain id should match supervisor for preset {preset:?}"
        );
    }
}

#[test]
fn peer_spec_peer_id_roundtrip() {
    let temp = tempfile::tempdir().expect("tempdir");
    let paths = NetworkPaths::from_root(temp.path(), &NetworkProfile::default());
    paths.ensure().expect("paths");
    let spec = PeerSpec::new(&paths, "peer0".into(), 8080, 1337).expect("peer spec");
    let peer_id = spec.peer_id();
    let parsed: PublicKey = peer_id.public_key().clone();
    assert_eq!(parsed, spec.keys.public_key);
}

#[test]
fn normalize_peer_config_overrides_sets_lane_count_and_local_services() {
    let mut nexus = toml::Table::new();
    nexus.insert("enabled".into(), toml::Value::Boolean(true));
    let mut lane0 = toml::Table::new();
    lane0.insert("alias".into(), toml::Value::String("core".into()));
    lane0.insert("index".into(), toml::Value::Integer(0));
    let mut lane1 = toml::Table::new();
    lane1.insert("alias".into(), toml::Value::String("governance".into()));
    lane1.insert("index".into(), toml::Value::Integer(1));
    nexus.insert(
        "lane_catalog".into(),
        toml::Value::Array(vec![toml::Value::Table(lane0), toml::Value::Table(lane1)]),
    );
    let mut nexus = Some(nexus);
    let mut sumeragi = None;
    let mut torii = None;

    normalize_peer_config_overrides(&mut nexus, &mut sumeragi, &mut torii)
        .expect("normalize overrides");

    let nexus = nexus.expect("nexus config");
    assert_eq!(
        nexus.get("lane_count").and_then(toml::Value::as_integer),
        Some(2)
    );
    assert!(sumeragi.is_none());
    let torii = torii.expect("torii config");
    let mcp = torii
        .get("mcp")
        .and_then(toml::Value::as_table)
        .expect("mcp table");
    assert!(matches!(
        mcp.get("enabled"),
        Some(toml::Value::Boolean(true))
    ));
    assert!(matches!(
        mcp.get("profile"),
        Some(toml::Value::String(value)) if value == LOCAL_MCP_PROFILE
    ));
}

#[test]
fn normalize_peer_config_overrides_rejects_disabled_nexus_with_lanes() {
    let mut nexus = toml::Table::new();
    nexus.insert("enabled".into(), toml::Value::Boolean(false));
    nexus.insert("lane_count".into(), toml::Value::Integer(3));
    let mut nexus = Some(nexus);
    let mut sumeragi = None;
    let mut torii = None;

    let err = normalize_peer_config_overrides(&mut nexus, &mut sumeragi, &mut torii)
        .expect_err("disabled nexus should fail");
    match err {
        SupervisorError::Config(message) => assert!(
            message.contains("nexus.enabled = false"),
            "unexpected error: {message}"
        ),
        other => panic!("expected SupervisorError::Config, got {other:?}"),
    }
}

#[test]
fn supervisor_defaults_nexus_disabled_for_local_permissioned_profiles() {
    if !ports_available("supervisor_defaults_nexus_disabled_for_local_permissioned_profiles") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("temp dir");
    let _stub = KagamiStub::install(temp.path());

    let supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");

    let nexus = supervisor
        .nexus_config_overrides()
        .expect("default nexus overrides");
    assert!(matches!(
        nexus.get("enabled"),
        Some(toml::Value::Boolean(false))
    ));
}

#[test]
fn supervisor_rejects_enabled_nexus_without_npos_consensus() {
    if !ports_available("supervisor_rejects_enabled_nexus_without_npos_consensus") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("temp dir");
    let _stub = KagamiStub::install(temp.path());

    let err = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .nexus_enabled(true)
        .build()
        .expect_err("permissioned localnet should reject nexus");
    match err {
        SupervisorError::Config(message) => assert!(
            message.contains("NPoS signed-genesis consensus mode"),
            "unexpected error: {message}"
        ),
        other => panic!("expected SupervisorError::Config, got {other:?}"),
    }
}

#[test]
fn supervisor_exposes_config_overrides() {
    if !ports_available("supervisor_exposes_config_overrides") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("temp dir");
    let _stub = KagamiStub::install(temp.path());

    let mut nexus = toml::Table::new();
    nexus.insert("enabled".into(), toml::Value::Boolean(true));
    let mut sumeragi = toml::Table::new();
    sumeragi.insert("msg_channel_cap_votes".into(), toml::Value::Integer(16));
    let mut torii = toml::Table::new();
    torii.insert(
        "address".into(),
        toml::Value::String("127.0.0.1:8080".to_owned()),
    );

    let supervisor =
        SupervisorBuilder::with_profile(npos_preset_profile(ProfilePreset::SinglePeer))
            .data_root(temp.path())
            .nexus_config(nexus)
            .sumeragi_config(sumeragi)
            .torii_config(torii)
            .build()
            .expect("build supervisor");

    let nexus = supervisor
        .nexus_config_overrides()
        .expect("nexus overrides");
    assert!(matches!(
        nexus.get("enabled"),
        Some(toml::Value::Boolean(true))
    ));
    assert_eq!(
        supervisor
            .sumeragi_config_overrides()
            .and_then(|table| table.get("msg_channel_cap_votes"))
            .and_then(toml::Value::as_integer),
        Some(16)
    );
    let torii = supervisor
        .torii_config_overrides()
        .expect("torii overrides");
    assert!(matches!(
        torii.get("address"),
        Some(toml::Value::String(value)) if value == "127.0.0.1:8080"
    ));
}

#[test]
fn lane_slug_sanitizes_alias() {
    assert_eq!(lane_slug("Core Lane", 0), "core_lane");
    assert_eq!(lane_slug("Gov+Ops", 2), "gov_ops");
    assert_eq!(lane_slug("---", 3), "lane3");
}

#[test]
fn lane_path_comments_include_default_aliases_for_multilane() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut nexus = toml::Table::new();
    nexus.insert("enabled".into(), toml::Value::Boolean(true));
    nexus.insert("lane_count".into(), toml::Value::Integer(3));

    let comments = lane_path_comments(temp.path(), Some(&nexus));
    assert!(
        comments
            .iter()
            .any(|line| line.contains("mochi.lane[0].alias = default"))
    );
    assert!(
        comments
            .iter()
            .any(|line| line.contains("mochi.lane[1].alias = lane1"))
    );
    assert!(
        comments
            .iter()
            .any(|line| line.contains("mochi.lane[2].alias = lane2"))
    );
}

#[test]
fn peer_spec_writes_nexus_and_always_on_da_storage() {
    let temp = tempfile::tempdir().expect("temp dir");
    let profile = NetworkProfile::default();
    let paths = NetworkPaths::from_root(temp.path(), &profile);
    paths.ensure().expect("paths");
    let spec = PeerSpec::new(&paths, "peer0".into(), 8080, 1337).expect("peer spec");
    let genesis = test_genesis_material(&paths);

    let mut nexus = toml::Table::new();
    nexus.insert("enabled".into(), toml::Value::Boolean(true));
    nexus.insert("lane_count".into(), toml::Value::Integer(1));
    let overrides = PeerConfigOverrides {
        nexus: Some(nexus),
        sumeragi: None,
        torii: None,
    };
    let specs = vec![spec.clone()];
    spec.write_config("demo-chain", &genesis, &specs, &overrides, &[])
        .expect("write config");

    let contents = fs::read_to_string(&spec.config_path).expect("read config");
    let value: toml::Table = toml::from_str(&contents).expect("parse config");
    let nexus = value
        .get("nexus")
        .and_then(toml::Value::as_table)
        .expect("nexus table");
    assert!(matches!(
        nexus.get("enabled"),
        Some(toml::Value::Boolean(true))
    ));
    let torii = value
        .get("torii")
        .and_then(toml::Value::as_table)
        .expect("torii table");
    let mcp = torii
        .get("mcp")
        .and_then(toml::Value::as_table)
        .expect("mcp table");
    assert!(matches!(
        mcp.get("enabled"),
        Some(toml::Value::Boolean(true))
    ));
    assert!(matches!(
        mcp.get("profile"),
        Some(toml::Value::String(value)) if value == LOCAL_MCP_PROFILE
    ));
    let expected_torii_dir = spec.storage_dir.join("torii").display().to_string();
    assert_eq!(
        torii.get("data_dir").and_then(toml::Value::as_str),
        Some(expected_torii_dir.as_str())
    );
    let da_ingest = torii
        .get("da_ingest")
        .and_then(toml::Value::as_table)
        .expect("da_ingest table");
    let expected_replay = spec
        .storage_dir
        .join("torii")
        .join("da_replay")
        .display()
        .to_string();
    assert_eq!(
        da_ingest
            .get("replay_cache_store_dir")
            .and_then(toml::Value::as_str),
        Some(expected_replay.as_str())
    );
    let expected_manifest = spec
        .storage_dir
        .join("torii")
        .join("da_manifests")
        .display()
        .to_string();
    assert_eq!(
        da_ingest
            .get("manifest_store_dir")
            .and_then(toml::Value::as_str),
        Some(expected_manifest.as_str())
    );
}

#[test]
fn peer_specs_write_distinct_managed_sorafs_state_roots() {
    let temp = tempfile::tempdir().expect("temp dir");
    let profile = NetworkProfile::from_preset(ProfilePreset::FourPeerBft);
    let paths = NetworkPaths::from_root(temp.path(), &profile);
    paths.ensure().expect("paths");
    let specs = (0_u16..4)
        .map(|index| {
            PeerSpec::new(&paths, format!("peer{index}"), 8_080 + index, 1_337 + index)
                .expect("peer spec")
        })
        .collect::<Vec<_>>();
    let genesis = test_genesis_material(&paths);

    for spec in &specs {
        spec.write_config(
            "demo-chain",
            &genesis,
            &specs,
            &PeerConfigOverrides::default(),
            &[],
        )
        .expect("write config");
    }

    let mut configured_roots = HashSet::new();
    for spec in &specs {
        let contents = fs::read_to_string(&spec.config_path).expect("read config");
        let value: toml::Table = toml::from_str(&contents).expect("parse config");
        let configured = value
            .get("sorafs")
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get("storage"))
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get("data_dir"))
            .and_then(toml::Value::as_str)
            .expect("managed SoraFS data directory");
        let expected = spec.storage_dir.join("sorafs").display().to_string();
        assert_eq!(configured, expected);
        assert!(
            configured_roots.insert(configured.to_owned()),
            "each peer must own a distinct SoraFS checkpoint root"
        );
    }
    assert_eq!(configured_roots.len(), specs.len());
}

#[test]
fn peer_spec_preserves_managed_sorafs_root_when_overlay_enables_storage() {
    let temp = tempfile::tempdir().expect("temp dir");
    let profile = NetworkProfile::default();
    let paths = NetworkPaths::from_root(temp.path(), &profile);
    paths.ensure().expect("paths");
    let spec = PeerSpec::new(&paths, "peer0".into(), 8080, 1337).expect("peer spec");
    let genesis = test_genesis_material(&paths);
    let mut storage = toml::Table::new();
    storage.insert("enabled".into(), toml::Value::Boolean(true));
    let mut sorafs = toml::Table::new();
    sorafs.insert("storage".into(), toml::Value::Table(storage));
    let mut overlay = toml::Table::new();
    overlay.insert("sorafs".into(), toml::Value::Table(sorafs));

    spec.write_config(
        "demo-chain",
        &genesis,
        std::slice::from_ref(&spec),
        &PeerConfigOverrides::default(),
        &[overlay],
    )
    .expect("write config");

    let contents = fs::read_to_string(&spec.config_path).expect("read config");
    let value: toml::Table = toml::from_str(&contents).expect("parse config");
    let storage = value
        .get("sorafs")
        .and_then(toml::Value::as_table)
        .and_then(|table| table.get("storage"))
        .and_then(toml::Value::as_table)
        .expect("SoraFS storage config");
    assert_eq!(
        storage.get("enabled").and_then(toml::Value::as_bool),
        Some(true)
    );
    let expected = spec.storage_dir.join("sorafs").display().to_string();
    assert_eq!(
        storage.get("data_dir").and_then(toml::Value::as_str),
        Some(expected.as_str())
    );
}

#[test]
fn peer_spec_rejects_sorafs_state_root_override() {
    let temp = tempfile::tempdir().expect("temp dir");
    let profile = NetworkProfile::default();
    let paths = NetworkPaths::from_root(temp.path(), &profile);
    paths.ensure().expect("paths");
    let spec = PeerSpec::new(&paths, "peer0".into(), 8080, 1337).expect("peer spec");
    let genesis = test_genesis_material(&paths);
    let mut storage = toml::Table::new();
    storage.insert(
        "data_dir".into(),
        toml::Value::String("/tmp/shared-sorafs".to_owned()),
    );
    let mut sorafs = toml::Table::new();
    sorafs.insert("storage".into(), toml::Value::Table(storage));
    let mut overlay = toml::Table::new();
    overlay.insert("sorafs".into(), toml::Value::Table(sorafs));

    let err = spec
        .write_config(
            "demo-chain",
            &genesis,
            std::slice::from_ref(&spec),
            &PeerConfigOverrides::default(),
            &[overlay],
        )
        .expect_err("SoraFS root override must fail closed");
    match err {
        SupervisorError::Config(message) => assert!(
            message.contains("must preserve Mochi's managed SoraFS root")
                && message.contains(spec.storage_dir.to_string_lossy().as_ref()),
            "unexpected error: {message}"
        ),
        other => panic!("expected SupervisorError::Config, got {other:?}"),
    }
}

#[test]
fn peer_specs_write_distinct_managed_streaming_state_roots() {
    let temp = tempfile::tempdir().expect("temp dir");
    let profile = NetworkProfile::from_preset(ProfilePreset::FourPeerBft);
    let paths = NetworkPaths::from_root(temp.path(), &profile);
    paths.ensure().expect("paths");
    let specs = (0_u16..4)
        .map(|index| {
            PeerSpec::new(&paths, format!("peer{index}"), 8_080 + index, 1_337 + index)
                .expect("peer spec")
        })
        .collect::<Vec<_>>();
    let genesis = test_genesis_material(&paths);

    for spec in &specs {
        spec.write_config(
            "demo-chain",
            &genesis,
            &specs,
            &PeerConfigOverrides::default(),
            &[],
        )
        .expect("write config");
    }

    let mut session_roots = HashSet::new();
    let mut soranet_roots = HashSet::new();
    let mut soravpn_roots = HashSet::new();
    for spec in &specs {
        let contents = fs::read_to_string(&spec.config_path).expect("read config");
        let value: toml::Table = toml::from_str(&contents).expect("parse config");
        let streaming = value
            .get("streaming")
            .and_then(toml::Value::as_table)
            .expect("streaming config");
        let session = streaming
            .get("session_store_dir")
            .and_then(toml::Value::as_str)
            .expect("managed streaming session directory");
        let soranet = streaming
            .get("soranet")
            .and_then(toml::Value::as_table)
            .expect("SoraNet streaming config");
        let soranet_spool = soranet
            .get("provision_spool_dir")
            .and_then(toml::Value::as_str)
            .expect("managed SoraNet spool directory");
        let soravpn_spool = streaming
            .get("soravpn")
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get("provision_spool_dir"))
            .and_then(toml::Value::as_str)
            .expect("managed SoraVPN spool directory");
        let expected = spec
            .storage_dir
            .canonicalize()
            .expect("storage root")
            .join("streaming");
        assert_eq!(Path::new(session), expected);
        assert_eq!(Path::new(soranet_spool), expected.join("soranet_routes"));
        assert_eq!(Path::new(soravpn_spool), expected.join("soravpn_routes"));
        assert!(Path::new(session).is_absolute());
        assert!(Path::new(soranet_spool).is_absolute());
        assert!(Path::new(soravpn_spool).is_absolute());
        assert_eq!(
            soranet.get("enabled").and_then(toml::Value::as_bool),
            Some(false)
        );
        for required in [
            "exit_multiaddr",
            "padding_budget_ms",
            "access_kind",
            "channel_salt",
            "provision_spool_max_bytes",
            "provision_window_segments",
            "provision_queue_capacity",
        ] {
            assert!(
                soranet.contains_key(required),
                "generated streaming.soranet is missing required field {required}"
            );
        }
        let soravpn = streaming
            .get("soravpn")
            .and_then(toml::Value::as_table)
            .expect("SoraVPN streaming config");
        assert!(soravpn.contains_key("provision_spool_max_bytes"));
        assert!(session_roots.insert(session.to_owned()));
        assert!(soranet_roots.insert(soranet_spool.to_owned()));
        assert!(soravpn_roots.insert(soravpn_spool.to_owned()));
    }
    assert_eq!(session_roots.len(), specs.len());
    assert_eq!(soranet_roots.len(), specs.len());
    assert_eq!(soravpn_roots.len(), specs.len());
}

#[test]
fn peer_specs_stage_distinct_rans_tables_and_write_absolute_paths() {
    let temp = tempfile::tempdir().expect("temp dir");
    let profile = NetworkProfile::from_preset(ProfilePreset::FourPeerBft);
    let paths = NetworkPaths::from_root(temp.path(), &profile);
    paths.ensure().expect("paths");
    let specs = (0_u16..4)
        .map(|index| {
            PeerSpec::new(&paths, format!("peer{index}"), 8_080 + index, 1_337 + index)
                .expect("peer spec")
        })
        .collect::<Vec<_>>();
    let genesis = test_genesis_material(&paths);

    for spec in &specs {
        spec.write_config(
            "demo-chain",
            &genesis,
            &specs,
            &PeerConfigOverrides::default(),
            &[],
        )
        .expect("write config");
    }

    let mut configured_paths = HashSet::new();
    for spec in &specs {
        let contents = fs::read_to_string(&spec.config_path).expect("read config");
        let value: toml::Table = toml::from_str(&contents).expect("parse config");
        let codec = value
            .get("streaming")
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get("codec"))
            .and_then(toml::Value::as_table)
            .expect("streaming codec config");
        assert_eq!(
            codec.get("cabac_mode").and_then(toml::Value::as_str),
            Some("disabled")
        );
        assert!(
            codec
                .get("trellis_blocks")
                .and_then(toml::Value::as_array)
                .is_some_and(|blocks| blocks.is_empty())
        );
        assert_eq!(
            codec.get("entropy_mode").and_then(toml::Value::as_str),
            Some("rans_bundled")
        );
        assert_eq!(
            codec.get("bundle_width").and_then(toml::Value::as_integer),
            Some(2)
        );
        assert_eq!(
            codec.get("bundle_accel").and_then(toml::Value::as_str),
            Some("none")
        );
        let configured = codec
            .get("rans_tables_path")
            .and_then(toml::Value::as_str)
            .expect("managed rANS tables path");
        let configured = Path::new(configured);

        assert!(configured.is_absolute());
        assert_eq!(configured, spec.rans_tables_path);
        assert!(configured.is_file());
        assert_eq!(
            fs::read(configured).expect("read staged rANS tables"),
            MANAGED_RANS_SEED0_TABLE
        );
        let tables = norito::streaming::codec::load_bundle_tables_from_toml(configured)
            .expect("parse staged SignedRansTablesV1");
        assert!(tables.max_width() >= 2);
        assert!(
            configured_paths.insert(configured.to_path_buf()),
            "each peer must reference a distinct staged rANS table"
        );
    }
    assert_eq!(configured_paths.len(), specs.len());
}

#[test]
fn peer_spec_preserves_managed_streaming_roots_with_shallow_opt_in_overlay() {
    let temp = tempfile::tempdir().expect("temp dir");
    let profile = NetworkProfile::default();
    let paths = NetworkPaths::from_root(temp.path(), &profile);
    paths.ensure().expect("paths");
    let spec = PeerSpec::new(&paths, "peer0".into(), 8080, 1337).expect("peer spec");
    let genesis = test_genesis_material(&paths);

    let mut soranet = toml::Table::new();
    soranet.insert("enabled".into(), toml::Value::Boolean(true));
    soranet.insert(
        "exit_multiaddr".into(),
        toml::Value::String("/dns/example.test/tcp/443".to_owned()),
    );
    let mut soravpn = toml::Table::new();
    soravpn.insert(
        "provision_spool_max_bytes".into(),
        toml::Value::Integer(4096),
    );
    let mut codec = toml::Table::new();
    codec.insert(
        "cabac_mode".into(),
        toml::Value::String("adaptive".to_owned()),
    );
    codec.insert(
        "trellis_blocks".into(),
        toml::Value::Array(vec![toml::Value::Integer(16), toml::Value::Integer(32)]),
    );
    codec.insert(
        "entropy_mode".into(),
        toml::Value::String("rans-bundled".to_owned()),
    );
    codec.insert("bundle_width".into(), toml::Value::Integer(3));
    codec.insert(
        "bundle_accel".into(),
        toml::Value::String("cpu_simd".to_owned()),
    );
    let mut streaming = toml::Table::new();
    streaming.insert("feature_bits".into(), toml::Value::Integer(7));
    streaming.insert("codec".into(), toml::Value::Table(codec));
    streaming.insert("soranet".into(), toml::Value::Table(soranet));
    streaming.insert("soravpn".into(), toml::Value::Table(soravpn));
    let mut overlay = toml::Table::new();
    overlay.insert("streaming".into(), toml::Value::Table(streaming));

    spec.write_config(
        "demo-chain",
        &genesis,
        std::slice::from_ref(&spec),
        &PeerConfigOverrides::default(),
        &[overlay],
    )
    .expect("write config");

    let contents = fs::read_to_string(&spec.config_path).expect("read config");
    let value: toml::Table = toml::from_str(&contents).expect("parse config");
    let streaming = value
        .get("streaming")
        .and_then(toml::Value::as_table)
        .expect("streaming config");
    let expected = spec
        .storage_dir
        .canonicalize()
        .expect("storage root")
        .join("streaming");
    assert_eq!(
        streaming
            .get("session_store_dir")
            .and_then(toml::Value::as_str),
        Some(expected.to_string_lossy().as_ref())
    );
    assert_eq!(
        streaming
            .get("feature_bits")
            .and_then(toml::Value::as_integer),
        Some(7)
    );
    assert!(streaming.contains_key("identity_public_key"));
    assert!(streaming.contains_key("identity_private_key"));
    let codec = streaming
        .get("codec")
        .and_then(toml::Value::as_table)
        .expect("streaming codec config");
    assert_eq!(
        codec.get("cabac_mode").and_then(toml::Value::as_str),
        Some("adaptive")
    );
    let trellis_blocks = codec
        .get("trellis_blocks")
        .and_then(toml::Value::as_array)
        .expect("trellis block override");
    assert_eq!(trellis_blocks.len(), 2);
    assert_eq!(trellis_blocks[0].as_integer(), Some(16));
    assert_eq!(trellis_blocks[1].as_integer(), Some(32));
    assert_eq!(
        codec.get("entropy_mode").and_then(toml::Value::as_str),
        Some("rans-bundled")
    );
    assert_eq!(
        codec.get("bundle_width").and_then(toml::Value::as_integer),
        Some(3)
    );
    assert_eq!(
        codec.get("bundle_accel").and_then(toml::Value::as_str),
        Some("cpu_simd")
    );
    let soranet = streaming
        .get("soranet")
        .and_then(toml::Value::as_table)
        .expect("SoraNet config");
    assert_eq!(
        soranet.get("enabled").and_then(toml::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        soranet.get("exit_multiaddr").and_then(toml::Value::as_str),
        Some("/dns/example.test/tcp/443")
    );
    assert_eq!(
        soranet
            .get("provision_spool_dir")
            .and_then(toml::Value::as_str),
        Some(expected.join("soranet_routes").to_string_lossy().as_ref())
    );
    let soravpn = streaming
        .get("soravpn")
        .and_then(toml::Value::as_table)
        .expect("SoraVPN config");
    assert_eq!(
        soravpn
            .get("provision_spool_max_bytes")
            .and_then(toml::Value::as_integer),
        Some(4096)
    );
    assert_eq!(
        soravpn
            .get("provision_spool_dir")
            .and_then(toml::Value::as_str),
        Some(expected.join("soravpn_routes").to_string_lossy().as_ref())
    );
}

#[test]
fn peer_spec_rejects_managed_streaming_state_redirects() {
    let temp = tempfile::tempdir().expect("temp dir");
    let profile = NetworkProfile::default();
    let paths = NetworkPaths::from_root(temp.path(), &profile);
    paths.ensure().expect("paths");
    let spec = PeerSpec::new(&paths, "peer0".into(), 8080, 1337).expect("peer spec");
    let genesis = test_genesis_material(&paths);

    for (key, nested, expected_error) in [
        ("session_store_dir", None, "managed streaming session root"),
        (
            "provision_spool_dir",
            Some("soranet"),
            "managed SoraNet provision spool",
        ),
        (
            "provision_spool_dir",
            Some("soravpn"),
            "managed SoraVPN provision spool",
        ),
    ] {
        let redirect = toml::Value::String("/tmp/shared-streaming-state".to_owned());
        let mut streaming = toml::Table::new();
        if let Some(section) = nested {
            let mut table = toml::Table::new();
            table.insert(key.into(), redirect);
            streaming.insert(section.into(), toml::Value::Table(table));
        } else {
            streaming.insert(key.into(), redirect);
        }
        let mut overlay = toml::Table::new();
        overlay.insert("streaming".into(), toml::Value::Table(streaming));
        let err = spec
            .write_config(
                "demo-chain",
                &genesis,
                std::slice::from_ref(&spec),
                &PeerConfigOverrides::default(),
                &[overlay],
            )
            .expect_err("managed streaming redirect must be rejected");
        match err {
            SupervisorError::Config(message) => assert!(
                message.contains(expected_error)
                    && message.contains(spec.storage_dir.to_string_lossy().as_ref()),
                "unexpected error: {message}"
            ),
            other => panic!("expected SupervisorError::Config, got {other:?}"),
        }
    }
}

#[test]
fn peer_spec_config_honors_torii_da_ingest_overrides() {
    let temp = tempfile::tempdir().expect("temp dir");
    let profile = NetworkProfile::default();
    let paths = NetworkPaths::from_root(temp.path(), &profile);
    paths.ensure().expect("paths");
    let spec = PeerSpec::new(&paths, "peer0".into(), 8080, 1337).expect("peer spec");
    let genesis = test_genesis_material(&paths);

    let mut da_ingest = toml::Table::new();
    da_ingest.insert(
        "replay_cache_store_dir".into(),
        toml::Value::String("/custom/replay".to_owned()),
    );
    da_ingest.insert(
        "manifest_store_dir".into(),
        toml::Value::String("/custom/manifests".to_owned()),
    );
    let mut torii = toml::Table::new();
    torii.insert("da_ingest".into(), toml::Value::Table(da_ingest));
    let overrides = PeerConfigOverrides {
        nexus: None,
        sumeragi: None,
        torii: Some(torii),
    };
    let specs = vec![spec.clone()];
    spec.write_config("demo-chain", &genesis, &specs, &overrides, &[])
        .expect("write config");

    let contents = fs::read_to_string(&spec.config_path).expect("read config");
    let value: toml::Table = toml::from_str(&contents).expect("parse config");
    let torii = value
        .get("torii")
        .and_then(toml::Value::as_table)
        .expect("torii table");
    let da_ingest = torii
        .get("da_ingest")
        .and_then(toml::Value::as_table)
        .expect("da_ingest table");
    assert_eq!(
        da_ingest
            .get("replay_cache_store_dir")
            .and_then(toml::Value::as_str),
        Some("/custom/replay")
    );
    assert_eq!(
        da_ingest
            .get("manifest_store_dir")
            .and_then(toml::Value::as_str),
        Some("/custom/manifests")
    );
}

#[test]
fn peer_spec_rejects_kura_store_override() {
    let temp = tempfile::tempdir().expect("temp dir");
    let profile = NetworkProfile::default();
    let paths = NetworkPaths::from_root(temp.path(), &profile);
    paths.ensure().expect("paths");
    let spec = PeerSpec::new(&paths, "peer0".into(), 8080, 1337).expect("peer spec");
    let genesis = test_genesis_material(&paths);
    let mut kura = toml::Table::new();
    kura.insert(
        "store_dir".into(),
        toml::Value::String("/tmp/unmanaged-kura".into()),
    );
    let mut overlay = toml::Table::new();
    overlay.insert("kura".into(), toml::Value::Table(kura));

    let err = spec
        .write_config(
            "demo-chain",
            &genesis,
            std::slice::from_ref(&spec),
            &PeerConfigOverrides::default(),
            &[overlay],
        )
        .expect_err("Kura root override must fail closed");
    match err {
        SupervisorError::Config(message) => assert!(
            message.contains("must preserve Mochi's managed Kura root")
                && message.contains(spec.kura_dir.to_string_lossy().as_ref()),
            "unexpected error: {message}"
        ),
        other => panic!("expected SupervisorError::Config, got {other:?}"),
    }
}

#[test]
fn peer_spec_rejects_non_string_kura_store_overlay() {
    let temp = tempfile::tempdir().expect("temp dir");
    let profile = NetworkProfile::default();
    let paths = NetworkPaths::from_root(temp.path(), &profile);
    paths.ensure().expect("paths");
    let spec = PeerSpec::new(&paths, "peer0".into(), 8080, 1337).expect("peer spec");
    let genesis = test_genesis_material(&paths);
    let mut kura = toml::Table::new();
    kura.insert("store_dir".into(), toml::Value::Integer(7));
    let mut overlay = toml::Table::new();
    overlay.insert("kura".into(), toml::Value::Table(kura));

    let err = spec
        .write_config(
            "demo-chain",
            &genesis,
            std::slice::from_ref(&spec),
            &PeerConfigOverrides::default(),
            &[overlay],
        )
        .expect_err("malformed Kura root override must fail closed");
    match err {
        SupervisorError::Config(message) => assert!(
            message.contains("must preserve Mochi's managed Kura root"),
            "unexpected error: {message}"
        ),
        other => panic!("expected SupervisorError::Config, got {other:?}"),
    }
}

#[test]
fn peer_spec_config_header_includes_lane_paths() {
    let temp = tempfile::tempdir().expect("temp dir");
    let profile = NetworkProfile::default();
    let paths = NetworkPaths::from_root(temp.path(), &profile);
    paths.ensure().expect("paths");
    let spec = PeerSpec::new(&paths, "peer0".into(), 8080, 1337).expect("peer spec");
    let genesis = test_genesis_material(&paths);

    let mut lane0 = toml::Table::new();
    lane0.insert("alias".into(), toml::Value::String("Core Lane".into()));
    lane0.insert("index".into(), toml::Value::Integer(0));
    let mut lane1 = toml::Table::new();
    lane1.insert("alias".into(), toml::Value::String("Gov+Ops".into()));
    lane1.insert("index".into(), toml::Value::Integer(1));
    let mut nexus = toml::Table::new();
    nexus.insert("enabled".into(), toml::Value::Boolean(true));
    nexus.insert("lane_count".into(), toml::Value::Integer(2));
    nexus.insert(
        "lane_catalog".into(),
        toml::Value::Array(vec![toml::Value::Table(lane0), toml::Value::Table(lane1)]),
    );
    let overrides = PeerConfigOverrides {
        nexus: Some(nexus),
        sumeragi: None,
        torii: None,
    };
    let specs = vec![spec.clone()];
    spec.write_config("demo-chain", &genesis, &specs, &overrides, &[])
        .expect("write config");

    let contents = fs::read_to_string(&spec.config_path).expect("read config");
    let lane0_slug = lane_slug("Core Lane", 0);
    let lane1_slug = lane_slug("Gov+Ops", 1);
    let lane0_blocks = spec
        .kura_dir
        .join("blocks")
        .join(format!("lane_000_{lane0_slug}"))
        .display()
        .to_string();
    let lane1_blocks = spec
        .kura_dir
        .join("blocks")
        .join(format!("lane_001_{lane1_slug}"))
        .display()
        .to_string();
    let lane0_merge = spec
        .kura_dir
        .join("merge_ledger")
        .join(format!("lane_000_{lane0_slug}_merge.log"))
        .display()
        .to_string();
    let lane1_merge = spec
        .kura_dir
        .join("merge_ledger")
        .join(format!("lane_001_{lane1_slug}_merge.log"))
        .display()
        .to_string();

    assert!(contents.contains("# mochi.lane[0].alias = Core Lane"));
    assert!(contents.contains("# mochi.lane[1].alias = Gov+Ops"));
    assert!(contents.contains(&format!("# mochi.lane[0].blocks_dir = {lane0_blocks}")));
    assert!(contents.contains(&format!("# mochi.lane[1].blocks_dir = {lane1_blocks}")));
    assert!(contents.contains(&format!("# mochi.lane[0].merge_log = {lane0_merge}")));
    assert!(contents.contains(&format!("# mochi.lane[1].merge_log = {lane1_merge}")));
}

#[test]
fn supervisor_session_info_reports_workspace_and_mcp_urls() {
    if !ports_available("supervisor_session_info_reports_workspace_and_mcp_urls") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("temp dir");
    let workspace_root = temp.path().join("workspace");
    let sandbox_root = workspace_root.join(".mochi").join("sandbox");
    let _stub = KagamiStub::install(temp.path());

    let supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(&sandbox_root)
        .build()
        .expect("build supervisor");

    let info = supervisor.session_info().expect("session info");
    assert_eq!(
        info.workspace_root.as_deref(),
        Some(workspace_root.as_path())
    );
    assert!(info.sandbox_root.ends_with(Path::new("single-peer")));
    assert_eq!(info.torii_url, "http://127.0.0.1:8080");
    assert_eq!(info.mcp_url, "http://127.0.0.1:8080/v1/mcp");
    assert!(info.account_id.is_some());
    assert!(info.private_key.is_some());
}

#[test]
fn managed_block_stream_unknown_peer_errors() {
    if !ports_available("managed_block_stream_unknown_peer_errors") {
        return;
    }
    let runtime = Runtime::new().expect("runtime");
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");

    let err = supervisor
        .managed_block_stream("missing", runtime.handle())
        .expect_err("unknown peer should fail");
    matches!(err, SupervisorError::PeerUnknown { .. });
}

#[test]
fn managed_block_stream_returns_handle_for_peer() {
    if !ports_available("managed_block_stream_returns_handle_for_peer") {
        return;
    }
    let runtime = Runtime::new().expect("runtime");
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");

    let stream = supervisor
        .managed_block_stream("peer0", runtime.handle())
        .expect("managed stream handle");

    assert_eq!(stream.alias(), "peer0");
    stream.abort();
}

#[test]
fn restart_policy_backoff_scales() {
    let policy = RestartPolicy::OnFailure {
        max_restarts: 5,
        backoff: Duration::from_millis(500),
    };
    assert_eq!(policy.backoff_for(1), Duration::from_millis(500));
    assert_eq!(policy.backoff_for(2), Duration::from_millis(1000));
    assert_eq!(policy.backoff_for(3), Duration::from_millis(2000));
    assert_eq!(policy.backoff_for(6), Duration::from_millis(8000));
}

#[test]
fn restart_policy_rejects_zero_attempt() {
    let policy = RestartPolicy::default();
    assert!(!policy.should_retry(0));
    assert_eq!(policy.backoff_for(0), Duration::ZERO);
}

#[cfg(unix)]
#[test]
fn managed_peer_process_uses_its_peer_directory_as_cwd() {
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let profile = NetworkProfile::default();
    let paths = NetworkPaths::from_root(temp.path(), &profile);
    paths.ensure().expect("paths");
    let spec = PeerSpec::new(&paths, "peer0".into(), 8080, 1337).expect("peer spec");
    fs::write(&spec.config_path, "chain = \"cwd-test\"\n").expect("write config");

    let cwd_capture = temp.path().join("peer-cwd.txt");
    let stub = temp.path().join("irohad-cwd-stub.sh");
    fs::write(
        &stub,
        "#!/bin/sh\n/bin/pwd > \"$MOCHI_TEST_PEER_CWD\"\nexit 0\n",
    )
    .expect("write irohad stub");
    let mut perms = fs::metadata(&stub).expect("stub metadata").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&stub, perms).expect("set stub permissions");
    let _capture_guard = EnvVarGuard::set("MOCHI_TEST_PEER_CWD", cwd_capture.as_os_str());

    let expected = spec
        .config_path
        .canonicalize()
        .expect("canonical config")
        .parent()
        .expect("peer directory")
        .to_path_buf();
    let logs_dir = temp.path().join("logs");
    let mut peer = PeerHandle::prepared(spec, logs_dir, RestartPolicy::Never);
    peer.start(&stub, StartReason::Manual).expect("start peer");
    for _ in 0..50 {
        if cwd_capture.exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let captured = fs::read_to_string(&cwd_capture).expect("captured peer cwd");
    assert_eq!(Path::new(captured.trim()), expected);
    if let Some(child) = peer.process.as_mut() {
        child.wait().expect("wait for peer stub");
    }
}

#[test]
fn manual_stop_cancels_pending_restart() {
    if !ports_available("manual_stop_cancels_pending_restart") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _kagami = KagamiStub::install(temp.path());

    let irohad_stub = temp.path().join("irohad_stub.sh");
    let stub_script = r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo "iroha-stub iroha3"
  exit 0
fi
exit 1
"#;
    fs::write(&irohad_stub, stub_script).expect("write irohad stub");
    #[cfg(unix)]
    {
        let mut perms = fs::metadata(&irohad_stub)
            .expect("stub metadata")
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&irohad_stub, perms).expect("set stub perms");
    }
    let _irohad_guard = EnvVarGuard::set("MOCHI_IROHAD", irohad_stub.as_os_str());

    let mut supervisor = match SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .restart_policy(RestartPolicy::OnFailure {
            max_restarts: 2,
            backoff: Duration::from_millis(200),
        })
        .build()
    {
        Ok(supervisor) => supervisor,
        Err(SupervisorError::Config(message))
            if message.contains("failed to allocate Torii port") =>
        {
            eprintln!("skipping manual_stop_cancels_pending_restart: {message}");
            return;
        }
        Err(err) => panic!("build supervisor: {err}"),
    };

    supervisor.start_peer("peer0").expect("start peer");
    // Stub exits immediately; refresh to observe the failure and schedule a restart.
    std::thread::sleep(Duration::from_millis(10));
    supervisor.refresh_peer_states();

    let peer = &supervisor.peers()[0];
    assert!(
        matches!(peer.state, PeerState::Restarting | PeerState::Stopped),
        "peer should schedule a restart after failure"
    );
    assert!(
        peer.next_restart_at.is_some(),
        "failure should set a restart timer"
    );

    supervisor
        .stop_peer("peer0")
        .expect("manual stop should succeed");

    let peer = &supervisor.peers()[0];
    assert!(
        peer.next_restart_at.is_none(),
        "restart timer should be cleared"
    );
    assert_eq!(peer.restart_attempts, 0);
    assert!(matches!(peer.state, PeerState::Stopped));

    // Allow enough time for the original backoff to elapse and confirm no restart occurs.
    std::thread::sleep(Duration::from_millis(250));
    supervisor.refresh_peer_states();

    let peer = &supervisor.peers()[0];
    assert!(
        peer.process.is_none(),
        "manual stop should keep the peer offline"
    );
    assert!(peer.next_restart_at.is_none());
    assert_eq!(peer.restart_attempts, 0);
    assert!(matches!(peer.state, PeerState::Stopped));
}

#[test]
fn supervisor_exposes_log_stream() {
    if !ports_available("supervisor_exposes_log_stream") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");

    let stream = supervisor
        .log_stream("peer0")
        .expect("log stream should be available");
    assert_eq!(stream.alias(), "peer0");
}

#[test]
fn start_peer_unknown_alias_errors() {
    if !ports_available("start_peer_unknown_alias_errors") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let mut supervisor = match SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .torii_base_port(20000)
        .p2p_base_port(30000)
        .build()
    {
        Ok(supervisor) => supervisor,
        Err(SupervisorError::Config(message))
            if message.contains("failed to allocate Torii port") =>
        {
            eprintln!("skipping start_peer_unknown_alias_errors: {message}");
            return;
        }
        Err(err) => panic!("build supervisor: {err}"),
    };

    let err = supervisor
        .start_peer("missing-peer")
        .expect_err("unknown peer should fail");
    assert!(matches!(err, SupervisorError::PeerUnknown { .. }));
}

#[test]
fn stop_peer_unknown_alias_errors() {
    if !ports_available("stop_peer_unknown_alias_errors") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let mut supervisor = match SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .torii_base_port(20000)
        .p2p_base_port(30000)
        .build()
    {
        Ok(supervisor) => supervisor,
        Err(SupervisorError::Config(message))
            if message.contains("failed to allocate Torii port") =>
        {
            eprintln!("skipping stop_peer_unknown_alias_errors: {message}");
            return;
        }
        Err(err) => panic!("build supervisor: {err}"),
    };

    let err = supervisor
        .stop_peer("missing-peer")
        .expect_err("unknown peer should fail");
    assert!(matches!(err, SupervisorError::PeerUnknown { .. }));
}
