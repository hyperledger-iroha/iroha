fn config_test_args(config_path: PathBuf, genesis_manifest_json: Option<PathBuf>) -> Args {
    Args {
        config: Some(config_path),
        genesis_manifest_json,
        startup: StartupArgs {
            check_config: false,
            write_kagemusha_catalog_qualification_seal: None,
            write_kagemusha_validator_qualification_seal: None,
            trace_config: false,
            config_blake3: None,
        },
        terminal_colors: false,
        language: None,
        sora: false,
        #[cfg(feature = "test-network-parliament-signers")]
        test_network_parliament_beacon_signer_mode: TestNetworkParliamentBeaconSignerMode::Valid,
        fastpq_execution_mode: None,
        fastpq_poseidon_mode: None,
        fastpq_device_class: None,
        fastpq_chip_family: None,
        fastpq_gpu_kind: None,
    }
}

#[test]
fn integrity_bound_config_is_hashed_and_parsed_from_one_buffer() -> eyre::Result<()> {
    let genesis_key_pair = KeyPair::random();
    let config = config_factory(genesis_key_pair.public_key());
    let raw = toml::to_string(&config)?;
    let dir = tempfile::tempdir()?;
    let config_path = dir.path().join("config.toml");
    std::fs::write(&config_path, raw.as_bytes())?;
    let expected = blake3::hash(raw.as_bytes()).to_hex().to_string();
    let mut args = config_test_args(config_path.clone(), None);
    args.startup.config_blake3 = Some(expected);
    let (_config, _genesis, sources) = read_config_and_genesis_with_kagemusha_sources(&args)
        .map_err(|report| eyre::eyre!("valid integrity-bound config failed: {report:?}"))?;
    assert_eq!(sources.flattened_toml_config_source(), Some(raw.as_bytes()));
    std::fs::write(&config_path, format!("{raw}\n# changed after admission\n"))?;
    assert_eq!(
        sources.flattened_toml_config_source(),
        Some(raw.as_bytes()),
        "the same-read buffer must not follow a later path mutation"
    );
    let error = read_config_and_genesis(&args)
        .expect_err("changed integrity-bound config must fail closed");
    assert!(
        format!("{error:?}").contains("has BLAKE3"),
        "unexpected integrity error: {error:?}"
    );
    Ok(())
}

#[test]
fn integrity_bound_config_rejects_extends() -> eyre::Result<()> {
    let genesis_key_pair = KeyPair::random();
    let mut config = config_factory(genesis_key_pair.public_key());
    config.insert(
        "extends".to_owned(),
        toml::Value::String("base.toml".to_owned()),
    );
    let raw = toml::to_string(&config)?;
    let dir = tempfile::tempdir()?;
    let config_path = dir.path().join("config.toml");
    std::fs::write(&config_path, raw.as_bytes())?;
    let mut args = config_test_args(config_path, None);
    args.startup.config_blake3 = Some(blake3::hash(raw.as_bytes()).to_hex().to_string());
    let error = read_config_and_genesis(&args)
        .expect_err("integrity-bound config must not resolve external extends");
    assert!(
        format!("{error:?}").contains("must be flattened"),
        "unexpected extends error: {error:?}"
    );
    Ok(())
}

#[test]
fn relative_file_paths_resolution() -> eyre::Result<()> {
    // Given
    let genesis_key_pair = KeyPair::random();
    let raw = GenesisBuilder::new_without_executor(ChainId::from("chain"), ".").build_raw();
    iroha_genesis::init_instruction_registry();
    let proposal = raw
        .build_and_sign(&genesis_key_pair)
        .expect("build prepared genesis proposal");
    assert!(proposal.0.is_resultless_proposal());
    let mut config = config_factory(genesis_key_pair.public_key());
    iroha_config::base::toml::Writer::new(&mut config)
        .write(["genesis", "file"], "./genesis/genesis.proposal.nrt")
        .write(["kura", "store_dir"], "../storage")
        .write(["snapshot", "store_dir"], "../snapshots")
        .write(["dev_telemetry", "out_file"], "../logs/telemetry");
    let dir = tempfile::tempdir()?;
    let genesis_path = dir.path().join("config/genesis/genesis.proposal.nrt");
    let executor_path = dir.path().join("config/genesis/executor.to");
    let config_path = dir.path().join("config/config.toml");
    std::fs::create_dir(dir.path().join("config"))?;
    std::fs::create_dir(dir.path().join("config/genesis"))?;
    std::fs::write(config_path, toml::to_string(&config)?)?;
    let genesis_wire = proposal.0.encode_wire()?;
    std::fs::write(&genesis_path, &genesis_wire)?;
    std::fs::write(executor_path, "")?;
    let config_path = dir.path().join("config/config.toml");
    // When
    let (config, genesis, sources) = read_config_and_genesis_with_kagemusha_sources(&Args {
        config: Some(config_path),
        genesis_manifest_json: None,
        startup: StartupArgs {
            check_config: false,
            write_kagemusha_catalog_qualification_seal: None,
            write_kagemusha_validator_qualification_seal: None,
            trace_config: false,
            config_blake3: None,
        },
        terminal_colors: false,
        language: None,
        sora: false,
        #[cfg(feature = "test-network-parliament-signers")]
        test_network_parliament_beacon_signer_mode: TestNetworkParliamentBeaconSignerMode::Valid,
        fastpq_execution_mode: None,
        fastpq_poseidon_mode: None,
        fastpq_device_class: None,
        fastpq_chip_family: None,
        fastpq_gpu_kind: None,
    })
    .map_err(|report| eyre::eyre!("{report:?}"))?;
    validate_config(&config).map_err(|report| eyre::eyre!("{report:?}"))?;
    // Then
    // No need to check whether genesis.file is resolved - if not, genesis wouldn't be read
    assert!(genesis.is_some());
    assert_eq!(
        sources.signed_genesis_source(),
        Some(genesis_wire.as_slice())
    );
    assert!(
        genesis
            .as_ref()
            .is_some_and(|block| block.0.is_resultless_proposal())
    );
    assert_eq!(
        config.kura.store_dir.resolve_relative_path().absolutize()?,
        dir.path().join("storage")
    );
    assert_eq!(
        config
            .snapshot
            .store_dir
            .resolve_relative_path()
            .absolutize()?,
        dir.path().join("snapshots")
    );
    assert_eq!(
        config
            .dev_telemetry
            .out_file
            .expect("dev telemetry should be set")
            .resolve_relative_path()
            .absolutize()?,
        dir.path().join("logs/telemetry")
    );
    Ok(())
}
