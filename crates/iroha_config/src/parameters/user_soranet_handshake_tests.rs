use super::*;

fn default_value<T>(name: &str, value: T) -> WithOrigin<T> {
    WithOrigin::new(
        value,
        ParameterOrigin::default(ParameterId::from(["network", "soranet_handshake", name])),
    )
}

fn test_handshake_config(kem_id: u8, sig_id: u8) -> SoranetHandshake {
    SoranetHandshake {
        descriptor_commit: default_value(
            "descriptor_commit",
            SoranetHandshake::default_descriptor_commit(),
        ),
        client_capabilities: default_value(
            "client_capabilities",
            SoranetHandshake::default_client_capabilities(),
        ),
        relay_capabilities: default_value(
            "relay_capabilities",
            SoranetHandshake::default_relay_capabilities(),
        ),
        trust_gossip: true,
        kem_id,
        kem_suite: None,
        sig_id,
        resume_hash: None,
        pow: SoranetHandshakePow {
            difficulty: SoranetHandshakePow::default_difficulty(),
            max_future_skew_secs: SoranetHandshakePow::default_max_future_skew(),
            min_ticket_ttl_secs: SoranetHandshakePow::default_min_ticket_ttl(),
            ticket_ttl_secs: SoranetHandshakePow::default_ticket_ttl(),
            outbound_mint_capacity: SoranetHandshakePow::default_puzzle_work_capacity(),
            inbound_verify_capacity: SoranetHandshakePow::default_puzzle_work_capacity(),
            revocation_store_capacity: SoranetHandshakePow::default_revocation_store_capacity(),
            revocation_store_ttl_secs: SoranetHandshakePow::default_revocation_store_ttl(),
            revocation_store_path: SoranetHandshakePow::default_revocation_store_path(),
            puzzle: SoranetHandshakePuzzle {
                memory_kib: SoranetHandshakePuzzle::default_memory_kib(),
                time_cost: SoranetHandshakePuzzle::default_time_cost(),
                lanes: SoranetHandshakePuzzle::default_lanes(),
            },
        },
    }
}

fn test_pow_config() -> SoranetHandshakePow {
    test_handshake_config(1, 1).pow
}

fn assert_pow_parse_error(config: SoranetHandshakePow, expected: &str) {
    let mut emitter = Emitter::new();
    let _ = config.parse(&mut emitter);
    let error = emitter
        .into_result()
        .expect_err("invalid SoraNet PoW configuration must be rejected");
    let rendered = format!("{error:?}");
    assert!(
        rendered.contains(expected),
        "expected error containing {expected:?}, got {rendered}"
    );
}

fn assert_puzzle_parse_error(config: SoranetHandshakePuzzle, expected: &str) {
    let mut emitter = Emitter::new();
    let _ = config.parse(&mut emitter);
    let error = emitter
        .into_result()
        .expect_err("invalid SoraNet puzzle configuration must be rejected");
    let rendered = format!("{error:?}");
    assert!(
        rendered.contains(expected),
        "expected error containing {expected:?}, got {rendered}"
    );
}

#[test]
fn parse_rejects_unsupported_handshake_suite_ids() {
    let mut kem_emitter = Emitter::new();
    let _ = test_handshake_config(0xFF, 1).parse(&mut kem_emitter);
    let kem_error = kem_emitter
        .into_result()
        .expect_err("unsupported KEM ID must fail configuration parsing");
    assert!(
        format!("{kem_error:?}").contains("network.soranet_handshake.kem_id 255 is unsupported")
    );

    let mut signature_emitter = Emitter::new();
    let _ = test_handshake_config(1, 0xFF).parse(&mut signature_emitter);
    let signature_error = signature_emitter
        .into_result()
        .expect_err("unsupported signature ID must fail configuration parsing");
    assert!(
        format!("{signature_error:?}")
            .contains("network.soranet_handshake.sig_id 255 is unsupported")
    );
}

#[test]
fn parse_accepts_supported_handshake_suite_ids() {
    let mut emitter = Emitter::new();
    let parsed = test_handshake_config(2, 1).parse(&mut emitter);
    emitter
        .into_result()
        .expect("supported suite IDs must parse");
    assert_eq!(parsed.kem_id, 2);
    assert_eq!(parsed.sig_id, 1);
    for capabilities in [
        parsed.client_capabilities.value(),
        parsed.relay_capabilities.value(),
    ] {
        let capabilities = iroha_crypto::soranet::handshake::parse_capabilities(capabilities)
            .expect("synchronized default capabilities must remain well formed");
        let kem = capabilities
            .iter()
            .find(|capability| capability.ty == 0x0101)
            .expect("default capabilities advertise snnet.pqkem");
        assert_eq!(kem.value, [2, 1]);
    }
}

#[test]
fn parse_synchronizes_default_relay_descriptor_capability() {
    let descriptor = vec![0xA5; iroha_crypto::Hash::LENGTH];
    let mut config = test_handshake_config(1, 1);
    config.descriptor_commit = WithOrigin::inline(HexBytes(descriptor.clone()));

    let mut emitter = Emitter::new();
    let parsed = config.parse(&mut emitter);
    emitter
        .into_result()
        .expect("a fixed-width descriptor override must parse");
    let capabilities =
        iroha_crypto::soranet::handshake::parse_capabilities(parsed.relay_capabilities.value())
            .expect("synchronized default relay capabilities must remain well formed");
    let transcript_commit = capabilities
        .iter()
        .find(|capability| capability.ty == 0x0103)
        .expect("default relay capabilities advertise snnet.transcript_commit");
    assert_eq!(transcript_commit.value, descriptor);
}

#[test]
fn parse_does_not_rewrite_operator_supplied_capability_vectors() {
    let mut config = test_handshake_config(2, 1);
    config.client_capabilities =
        WithOrigin::inline(SoranetHandshake::default_client_capabilities());

    let mut emitter = Emitter::new();
    let parsed = config.parse(&mut emitter);
    emitter
        .into_result()
        .expect("custom capability syntax is validated by the P2P preflight");
    let client =
        iroha_crypto::soranet::handshake::parse_capabilities(parsed.client_capabilities.value())
            .expect("operator vector must remain well formed");
    let relay =
        iroha_crypto::soranet::handshake::parse_capabilities(parsed.relay_capabilities.value())
            .expect("default relay vector must remain well formed");
    let selected_kem = |capabilities: &[iroha_crypto::soranet::handshake::CapabilityTlv]| {
        capabilities
            .iter()
            .find(|capability| capability.ty == 0x0101)
            .expect("snnet.pqkem must be advertised")
            .value[0]
    };
    assert_eq!(selected_kem(&client), 1);
    assert_eq!(selected_kem(&relay), 2);
}

#[test]
fn default_ticket_ttl_uses_full_future_skew_window() {
    assert_eq!(
        SoranetHandshakePow::default_ticket_ttl(),
        SoranetHandshakePow::default_max_future_skew()
    );
    assert!(
        SoranetHandshakePow::default_ticket_ttl() > SoranetHandshakePow::default_min_ticket_ttl()
    );
}

#[test]
fn parse_accepts_first_release_pow_boundaries() {
    let mut lower = test_pow_config();
    lower.difficulty = NonZeroU16::new(1).expect("non-zero difficulty");
    lower.max_future_skew_secs = 2;
    lower.min_ticket_ttl_secs = 1;
    lower.ticket_ttl_secs = 2;
    lower.outbound_mint_capacity = nonzero!(1usize);
    lower.inbound_verify_capacity = nonzero!(1usize);
    lower.revocation_store_capacity = 1;
    lower.revocation_store_ttl_secs = 2;
    lower.puzzle = SoranetHandshakePuzzle {
        memory_kib: iroha_crypto::soranet::puzzle::MIN_MEMORY_KIB,
        time_cost: 1,
        lanes: 1,
    };
    let mut lower_emitter = Emitter::new();
    let lower = lower.parse(&mut lower_emitter);
    lower_emitter
        .into_result()
        .expect("lower first-release boundaries must parse");
    assert_eq!(lower.difficulty, 1);
    assert_eq!(lower.max_future_skew, Duration::from_secs(2));
    assert_eq!(lower.min_ticket_ttl, Duration::from_secs(1));
    assert_eq!(lower.ticket_ttl, Duration::from_secs(2));
    assert_eq!(lower.outbound_mint_capacity.get(), 1);
    assert_eq!(lower.inbound_verify_capacity.get(), 1);
    assert_eq!(lower.revocation_store_capacity, 1);
    assert_eq!(lower.revocation_max_ttl, Duration::from_secs(2));
    assert_eq!(
        lower.puzzle.memory_kib.get(),
        iroha_crypto::soranet::puzzle::MIN_MEMORY_KIB
    );
    assert_eq!(lower.puzzle.time_cost.get(), 1);
    assert_eq!(lower.puzzle.lanes.get(), 1);

    let mut upper = test_pow_config();
    upper.difficulty = NonZeroU16::new(u16::from(iroha_crypto::soranet::puzzle::MAX_DIFFICULTY))
        .expect("non-zero difficulty");
    upper.outbound_mint_capacity =
        NonZeroUsize::new(actual::SoranetPow::MAX_PUZZLE_WORK_CAPACITY_PER_DIRECTION)
            .expect("non-zero work capacity");
    upper.inbound_verify_capacity = upper.outbound_mint_capacity;
    upper.revocation_store_capacity =
        u64::try_from(iroha_crypto::soranet::pow::TICKET_REVOCATION_STORE_MAX_ENTRIES_V1)
            .expect("first-release revocation capacity fits u64");
    upper.puzzle = SoranetHandshakePuzzle {
        memory_kib: iroha_crypto::soranet::puzzle::MAX_MEMORY_KIB,
        time_cost: iroha_crypto::soranet::puzzle::MAX_TIME_COST,
        lanes: iroha_crypto::soranet::puzzle::MAX_LANES,
    };
    let mut upper_emitter = Emitter::new();
    let upper = upper.parse(&mut upper_emitter);
    upper_emitter
        .into_result()
        .expect("upper first-release boundaries must parse");
    assert_eq!(
        upper.difficulty,
        iroha_crypto::soranet::puzzle::MAX_DIFFICULTY
    );
    assert_eq!(
        upper.outbound_mint_capacity.get(),
        actual::SoranetPow::MAX_PUZZLE_WORK_CAPACITY_PER_DIRECTION
    );
    assert_eq!(
        upper.inbound_verify_capacity.get(),
        actual::SoranetPow::MAX_PUZZLE_WORK_CAPACITY_PER_DIRECTION
    );
    assert_eq!(
        upper.revocation_store_capacity,
        iroha_crypto::soranet::pow::TICKET_REVOCATION_STORE_MAX_ENTRIES_V1
    );
    assert_eq!(
        upper.puzzle.memory_kib.get(),
        iroha_crypto::soranet::puzzle::MAX_MEMORY_KIB
    );
    assert_eq!(
        upper.puzzle.time_cost.get(),
        iroha_crypto::soranet::puzzle::MAX_TIME_COST
    );
    assert_eq!(
        upper.puzzle.lanes.get(),
        iroha_crypto::soranet::puzzle::MAX_LANES
    );
}

#[test]
fn parse_rejects_difficulty_above_first_release_maximum() {
    let mut config = test_pow_config();
    config.difficulty =
        NonZeroU16::new(u16::from(iroha_crypto::soranet::puzzle::MAX_DIFFICULTY + 1))
            .expect("non-zero difficulty");
    assert_pow_parse_error(
        config,
        "network.soranet_handshake.pow.difficulty 33 exceeds the supported maximum 32",
    );
}

#[test]
fn parse_rejects_invalid_ticket_and_revocation_windows() {
    let mut zero_minimum = test_pow_config();
    zero_minimum.min_ticket_ttl_secs = 0;
    assert_pow_parse_error(
        zero_minimum,
        "network.soranet_handshake.pow.min_ticket_ttl_secs must be greater than zero",
    );

    let mut closed_window = test_pow_config();
    closed_window.max_future_skew_secs = closed_window.min_ticket_ttl_secs;
    assert_pow_parse_error(
        closed_window,
        "network.soranet_handshake.pow.max_future_skew_secs 30 must exceed min_ticket_ttl_secs 30",
    );

    let mut short_ticket = test_pow_config();
    short_ticket.ticket_ttl_secs = short_ticket.min_ticket_ttl_secs;
    assert_pow_parse_error(
        short_ticket,
        "network.soranet_handshake.pow.ticket_ttl_secs 30 must exceed min_ticket_ttl_secs 30",
    );

    let mut long_ticket = test_pow_config();
    long_ticket.ticket_ttl_secs = long_ticket.max_future_skew_secs + 1;
    assert_pow_parse_error(
        long_ticket,
        "network.soranet_handshake.pow.ticket_ttl_secs 301 must not exceed max_future_skew_secs 300",
    );

    let mut zero_revocation_ttl = test_pow_config();
    zero_revocation_ttl.revocation_store_ttl_secs = 0;
    assert_pow_parse_error(
        zero_revocation_ttl,
        "network.soranet_handshake.pow.revocation_store_ttl_secs must be greater than zero",
    );

    let mut short_revocation_ttl = test_pow_config();
    short_revocation_ttl.revocation_store_ttl_secs = short_revocation_ttl.max_future_skew_secs - 1;
    assert_pow_parse_error(
        short_revocation_ttl,
        "network.soranet_handshake.pow.revocation_store_ttl_secs 299 must cover max_future_skew_secs 300",
    );
}

#[test]
fn parse_rejects_out_of_range_work_and_revocation_capacities() {
    let oversized_work_capacity =
        NonZeroUsize::new(actual::SoranetPow::MAX_PUZZLE_WORK_CAPACITY_PER_DIRECTION + 1)
            .expect("non-zero work capacity");
    let mut outbound = test_pow_config();
    outbound.outbound_mint_capacity = oversized_work_capacity;
    assert_pow_parse_error(
        outbound,
        "network.soranet_handshake.pow.outbound_mint_capacity 9 exceeds the supported maximum 8",
    );

    let mut inbound = test_pow_config();
    inbound.inbound_verify_capacity = oversized_work_capacity;
    assert_pow_parse_error(
        inbound,
        "network.soranet_handshake.pow.inbound_verify_capacity 9 exceeds the supported maximum 8",
    );

    let mut zero_revocation_capacity = test_pow_config();
    zero_revocation_capacity.revocation_store_capacity = 0;
    assert_pow_parse_error(
        zero_revocation_capacity,
        "network.soranet_handshake.pow.revocation_store_capacity 0 must be in 1..=65536",
    );

    let mut oversized_revocation_capacity = test_pow_config();
    oversized_revocation_capacity.revocation_store_capacity =
        u64::try_from(iroha_crypto::soranet::pow::TICKET_REVOCATION_STORE_MAX_ENTRIES_V1)
            .expect("first-release revocation capacity fits u64")
            + 1;
    assert_pow_parse_error(
        oversized_revocation_capacity,
        "network.soranet_handshake.pow.revocation_store_capacity 65537 must be in 1..=65536",
    );
}

#[test]
fn parse_rejects_out_of_range_argon2_resource_costs() {
    let valid = || SoranetHandshakePuzzle {
        memory_kib: SoranetHandshakePuzzle::default_memory_kib(),
        time_cost: SoranetHandshakePuzzle::default_time_cost(),
        lanes: SoranetHandshakePuzzle::default_lanes(),
    };

    let mut memory_below_minimum = valid();
    memory_below_minimum.memory_kib = iroha_crypto::soranet::puzzle::MIN_MEMORY_KIB - 1;
    assert_puzzle_parse_error(
        memory_below_minimum,
        "network.soranet_handshake.pow.puzzle.memory_kib 4095 must be in 4096..=131072",
    );

    let mut memory_above_maximum = valid();
    memory_above_maximum.memory_kib = iroha_crypto::soranet::puzzle::MAX_MEMORY_KIB + 1;
    assert_puzzle_parse_error(
        memory_above_maximum,
        "network.soranet_handshake.pow.puzzle.memory_kib 131073 must be in 4096..=131072",
    );

    let mut zero_time_cost = valid();
    zero_time_cost.time_cost = 0;
    assert_puzzle_parse_error(
        zero_time_cost,
        "network.soranet_handshake.pow.puzzle.time_cost 0 must be in 1..=8",
    );

    let mut excessive_time_cost = valid();
    excessive_time_cost.time_cost = iroha_crypto::soranet::puzzle::MAX_TIME_COST + 1;
    assert_puzzle_parse_error(
        excessive_time_cost,
        "network.soranet_handshake.pow.puzzle.time_cost 9 must be in 1..=8",
    );

    let mut zero_lanes = valid();
    zero_lanes.lanes = 0;
    assert_puzzle_parse_error(
        zero_lanes,
        "network.soranet_handshake.pow.puzzle.lanes 0 must be in 1..=16",
    );

    let mut excessive_lanes = valid();
    excessive_lanes.lanes = iroha_crypto::soranet::puzzle::MAX_LANES + 1;
    assert_puzzle_parse_error(
        excessive_lanes,
        "network.soranet_handshake.pow.puzzle.lanes 17 must be in 1..=16",
    );
}
