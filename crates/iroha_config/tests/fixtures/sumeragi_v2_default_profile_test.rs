#[test]
fn sumeragi_seed_custody_and_local_overrides_are_validated_together() {
    use iroha_config::parameters::user::Root as User;

    let parse = |role: &str, descriptor: u16| {
        let overrides = format!(
            "[sumeragi]\nrole = {role:?}\nmint_finality_seed_fd = {descriptor}\nview_timeout_base_ms = 250\n"
        )
        .parse::<Table>()
        .expect("custody and timing overrides");
        ConfigReader::new()
            .read_toml_with_extends(fixtures_dir().join("base.toml"))
            .expect("base fixture")
            .with_toml_source(TomlSource::inline(overrides))
            .read_and_complete::<User>()
            .expect("user config")
            .parse()
    };
    let config = parse("validator", 199).expect("validator custody and override");
    assert_eq!(config.sumeragi.mint_finality_seed_fd, Some(199));
    assert_eq!(
        config.sumeragi.local.t_base,
        Some(Duration::from_millis(250))
    );

    let error = parse("validator", 198).expect_err("wrong private descriptor");
    assert!(format!("{error:?}").contains("fixed private descriptor 199"));

    let error = parse("observer", 199).expect_err("observer cannot hold a validator seed");
    assert!(format!("{error:?}").contains("observer must not configure a mint-finality seed"));
}

#[test]
fn sumeragi_v2_defaults_match_fresh_network_profile() {
    use defaults::sumeragi::npos;
    use iroha_config::parameters::{actual::Root as Actual, user::Root as User};
    use iroha_config_base::read::ConfigReader;
    assert_eq!(defaults::sumeragi::PROTOCOL_VERSION, 8);
    assert_eq!(
        defaults::sumeragi::PROTOCOL_VERSION,
        u32::from(iroha_data_model::sumeragi::PROTOCOL_VERSION)
    );
    assert_eq!(defaults::sumeragi::BLOCK_CADENCE_MS, 1_000);
    assert_eq!(defaults::sumeragi::ROUND_TIMEOUT_CADENCE_MULTIPLIER, 10);
    assert_eq!(defaults::sumeragi::RETRANSMIT_DIVISOR, 5);
    assert_eq!(defaults::sumeragi::BLOCK_MAX_TRANSACTIONS.get(), 512);
    assert_eq!(
        defaults::sumeragi::BLOCK_MAX_PAYLOAD_BYTES.get(),
        16 * 1024 * 1024,
    );
    assert_eq!(defaults::sumeragi::QUEUE_COMMAND_CAPACITY.get(), 1_024);
    assert_eq!(
        defaults::sumeragi::QUEUE_AUTHENTICATED_NON_VALIDATOR_SOURCE_CAPACITY.get(),
        2
    );
    assert_eq!(defaults::sumeragi::QUEUE_BODY_CAPACITY.get(), 161);
    assert_eq!(
        defaults::sumeragi::QUEUE_BODY_CAPACITY.get(),
        5 * iroha_data_model::block::consensus_v2::MAX_VALIDATORS_PER_HEIGHT
            + 3 * defaults::sumeragi::QUEUE_AUTHENTICATED_NON_VALIDATOR_SOURCE_CAPACITY.get()
    );
    assert_eq!(
        defaults::sumeragi::QUEUE_BODY_BYTES.get(),
        1122 * 1024 * 1024
    );
    assert_eq!(
        defaults::sumeragi::QUEUE_BODY_SOURCE_BYTES.get(),
        34 * 1024 * 1024
    );
    assert_eq!(defaults::sumeragi::BODY_ENVELOPE_HEADROOM_BYTES, 64 * 1024);
    assert_eq!(defaults::sumeragi::TIMEOUT_VOTE_RESERVE_BYTES, 64 * 1024);
    assert_eq!(
        defaults::sumeragi::CERTIFIED_FENCE_ESCAPE_RESERVE_BYTES,
        1024 * 1024
    );
    assert_eq!(defaults::sumeragi::QUEUE_CHUNK_CAPACITY.get(), 2_048);
    assert_eq!(defaults::sumeragi::QUEUE_READY_BODY_CAPACITY.get(), 128);
    assert_eq!(npos::EPOCH_LENGTH_BLOCKS, 3_600);
    let cfg: Actual = ConfigReader::new()
        .read_toml_with_extends(fixtures_dir().join("base.toml"))
        .expect("base file should be valid")
        .read_and_complete::<User>()
        .expect("user config")
        .parse()
        .expect("actual config");
    assert_eq!(cfg.sumeragi.block.max_transactions.get(), 512);
    assert_eq!(cfg.sumeragi.block.max_payload_bytes.get(), 16 * 1024 * 1024);
    assert_eq!(cfg.sumeragi.queues.commands.get(), 1_024);
    assert_eq!(
        cfg.sumeragi
            .queues
            .authenticated_non_validator_sources
            .get(),
        2
    );
    assert_eq!(cfg.sumeragi.queues.bodies.get(), 161);
    assert_eq!(cfg.sumeragi.queues.body_bytes.get(), 1122 * 1024 * 1024);
    assert_eq!(
        cfg.sumeragi.queues.body_source_bytes.get(),
        34 * 1024 * 1024
    );
    assert_eq!(cfg.sumeragi.queues.chunks.get(), 2_048);
    assert_eq!(cfg.sumeragi.queues.ready_bodies.get(), 128);
    cfg.sumeragi
        .v2_config(
            Duration::from_secs(1),
            iroha_data_model::block::consensus_v2::ConsensusMode::Permissioned,
        )
        .expect("default parsed configuration must satisfy the v2 contract");
}
