// Generated-localnet rANS table copy, isolation, and path contract tests.
#[test]
fn copy_rans_tables_writes_seed_table() {
    let temp = tempfile::tempdir().expect("tmp dir");
    let emitted_path = copy_rans_tables(temp.path()).expect("copy rANS tables");
    let seed_path = temp
        .path()
        .join("codec")
        .join("rans")
        .join("tables")
        .join("rans_seed0.toml");
    assert!(emitted_path.is_absolute());
    assert_eq!(
        emitted_path,
        fs::canonicalize(&seed_path).expect("canonical emitted rANS table")
    );
    let bytes = fs::read(&emitted_path).expect("read rANS seed table");
    assert_eq!(bytes, RANS_SEED0_TABLE);
}
#[test]
fn generated_peer_configs_isolate_absolute_rans_tables_for_every_profile() {
    let temp = tempfile::tempdir().expect("tmp dir");
    let profiles = [
        ("generic", None, SumeragiConsensusMode::Permissioned),
        (
            "nexus",
            Some(SoraProfile::Nexus),
            SumeragiConsensusMode::Npos,
        ),
        (
            "paynet",
            Some(SoraProfile::Dataspace),
            SumeragiConsensusMode::Npos,
        ),
        (
            "sbp",
            Some(SoraProfile::PrivateSbp),
            SumeragiConsensusMode::Npos,
        ),
        (
            "cbuae",
            Some(SoraProfile::PrivateCbuae),
            SumeragiConsensusMode::Npos,
        ),
    ];
    let mut generated_paths = std::collections::BTreeSet::new();
    for (label, sora_profile, consensus_mode) in profiles {
        let out_dir = temp.path().join(label);
        let opts = LocalnetOptions {
            build_line: BuildLine::Iroha3,
            sora_profile,
            perf_profile: None,
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some(format!("rans-table-{label}")),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: 19080,
            base_p2p_port: 23337,
            out_dir: out_dir.clone(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: None,
            consensus_mode,
        };
        generate_localnet(&opts, &mut BufWriter::new(Vec::new()))
            .unwrap_or_else(|error| panic!("generate {label} localnet: {error:#}"));
        let start_script =
            fs::read_to_string(out_dir.join("start.sh")).expect("read generated start script");
        assert_eq!(
            start_script.contains("IROHA_SORA_MODE=\"1\""),
            sora_profile.is_some(),
            "{label} startup mode must follow explicit Sora profile selection"
        );
        let canonical_out_dir =
            fs::canonicalize(&out_dir).expect("canonical generated output directory");
        let expected_path =
            fs::canonicalize(canonical_out_dir.join(LOCALNET_RANS_TABLE_RELATIVE_PATH))
                .expect("canonical generated rANS table");
        assert!(
            expected_path.starts_with(&canonical_out_dir),
            "{label} rANS table must remain inside its generated output"
        );
        assert_eq!(
            fs::read(&expected_path).expect("read generated rANS table"),
            RANS_SEED0_TABLE
        );
        for peer_index in 0..opts.peers.get() {
            let config_path = out_dir.join(format!("peer{peer_index}.toml"));
            let peer_config: toml::Value = toml::from_str(
                &fs::read_to_string(&config_path).expect("read generated peer config"),
            )
            .expect("parse generated peer config");
            let configured_path = peer_config
                .get("streaming")
                .and_then(toml::Value::as_table)
                .and_then(|streaming| streaming.get("codec"))
                .and_then(toml::Value::as_table)
                .and_then(|codec| codec.get("rans_tables_path"))
                .and_then(toml::Value::as_str)
                .map(PathBuf::from)
                .expect("generated streaming codec rANS tables path");
            assert!(
                configured_path.is_absolute(),
                "{label} peer {peer_index} rANS table path must be absolute"
            );
            assert_eq!(
                configured_path, expected_path,
                "{label} peer {peer_index} must bind its own generated rANS table"
            );
            let sorafs_storage = peer_config
                .get("sorafs")
                .and_then(toml::Value::as_table)
                .and_then(|sorafs| sorafs.get("storage"))
                .and_then(toml::Value::as_table)
                .expect("every peer must reserve its own persistent SoraFS root");
            assert_eq!(
                sorafs_storage
                    .get("data_dir")
                    .and_then(toml::Value::as_str)
                    .map(PathBuf::from),
                Some(
                    canonical_out_dir
                        .join("state")
                        .join(format!("peer{peer_index}"))
                        .join("sorafs")
                ),
                "{label} peer {peer_index} must reserve its own SoraFS data root"
            );
            if sora_profile.is_some() {
                assert_eq!(
                    sorafs_storage
                        .get("enabled")
                        .and_then(toml::Value::as_bool),
                    Some(false),
                    "{label} must opt out of unprovisioned embedded SoraFS storage"
                );
            } else {
                assert!(
                    !sorafs_storage.contains_key("enabled"),
                    "{label} must not emit a Sora-only enabled override"
                );
            }
            let sorafs_por_state_dir = peer_config
                .get("sorafs")
                .and_then(toml::Value::as_table)
                .and_then(|sorafs| sorafs.get("por"))
                .and_then(toml::Value::as_table)
                .and_then(|por| por.get("state_dir"))
                .and_then(toml::Value::as_str)
                .map(PathBuf::from);
            assert_eq!(
                sorafs_por_state_dir,
                Some(
                    canonical_out_dir
                        .join("state")
                        .join(format!("peer{peer_index}"))
                        .join("sorafs")
                        .join("por")
                ),
                "{label} peer {peer_index} must reserve its own SoraFS PoR state root"
            );
        }
        assert!(
            generated_paths.insert(expected_path),
            "{label} must not reuse another generated network's rANS table"
        );
    }
    assert_eq!(generated_paths.len(), profiles.len());
}
