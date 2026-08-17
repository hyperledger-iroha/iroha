#[test]
fn sumeragi_v2_defaults_match_fresh_network_profile() {
    use defaults::sumeragi::npos;
    use iroha_config::parameters::{actual::Root as Actual, user::Root as User};
    use iroha_config_base::read::ConfigReader;
    assert_eq!(defaults::sumeragi::PROTOCOL_VERSION, 4);
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
        defaults::network::lane_profile::CORE_MAX_TOTAL_CONNECTIONS,
        99,
        "the default core reply-source bound must fit maximum-roster lifecycle geometry"
    );
    assert_eq!(
        defaults::sumeragi::QUEUE_AUTHENTICATED_NON_VALIDATOR_SOURCE_CAPACITY.get(),
        2
    );
    assert_eq!(defaults::sumeragi::QUEUE_BODY_CAPACITY.get(), 163);
    assert_eq!(
        defaults::sumeragi::QUEUE_BODY_CAPACITY.get(),
        5 * iroha_data_model::block::consensus_v2::MAX_VALIDATORS_PER_HEIGHT
            + 3 * defaults::sumeragi::QUEUE_AUTHENTICATED_NON_VALIDATOR_SOURCE_CAPACITY.get()
            + 2
    );
    assert_eq!(
        defaults::sumeragi::QUEUE_BODY_BYTES.get(),
        231 * 1024 * 1024
    );
    assert_eq!(
        defaults::sumeragi::QUEUE_BODY_SOURCE_BYTES.get(),
        33 * 1024 * 1024
    );
    assert_eq!(defaults::sumeragi::BODY_ENVELOPE_HEADROOM_BYTES, 64 * 1024);
    assert_eq!(defaults::sumeragi::TIMEOUT_VOTE_RESERVE_BYTES, 64 * 1024);
    assert_eq!(
        defaults::sumeragi::CERTIFIED_FENCE_ESCAPE_RESERVE_BYTES,
        64 * 1024
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
    assert_eq!(cfg.sumeragi.queues.bodies.get(), 163);
    assert_eq!(cfg.sumeragi.queues.body_bytes.get(), 231 * 1024 * 1024);
    assert_eq!(
        cfg.sumeragi.queues.body_source_bytes.get(),
        33 * 1024 * 1024
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
