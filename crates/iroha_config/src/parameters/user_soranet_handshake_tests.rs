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
            signed_ticket_public_key_hex: None,
            puzzle: SoranetHandshakePuzzle {
                memory_kib: SoranetHandshakePuzzle::default_memory_kib(),
                time_cost: SoranetHandshakePuzzle::default_time_cost(),
                lanes: SoranetHandshakePuzzle::default_lanes(),
            },
        },
    }
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
        assert_eq!(kem.value, [2]);
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
fn puzzle_work_capacity_is_nonzero_and_bounded() {
    assert_eq!(
        SoranetHandshakePow::default_puzzle_work_capacity(),
        actual::SoranetPow::DEFAULT_PUZZLE_WORK_CAPACITY_PER_DIRECTION
    );
    assert_eq!(
        SoranetHandshakePow::bound_puzzle_work_capacity(nonzero!(usize::MAX)).get(),
        actual::SoranetPow::MAX_PUZZLE_WORK_CAPACITY_PER_DIRECTION
    );
}

#[test]
fn parse_bounds_argon2_resource_costs() {
    let upper = SoranetHandshakePuzzle {
        memory_kib: u32::MAX,
        time_cost: u32::MAX,
        lanes: u32::MAX,
    }
    .parse();
    assert_eq!(
        upper.memory_kib.get(),
        iroha_crypto::soranet::puzzle::MAX_MEMORY_KIB
    );
    assert_eq!(
        upper.time_cost.get(),
        iroha_crypto::soranet::puzzle::MAX_TIME_COST
    );
    assert_eq!(upper.lanes.get(), iroha_crypto::soranet::puzzle::MAX_LANES);
    let lower = SoranetHandshakePuzzle {
        memory_kib: 0,
        time_cost: 0,
        lanes: 0,
    }
    .parse();
    assert_eq!(
        lower.memory_kib.get(),
        iroha_crypto::soranet::puzzle::MIN_MEMORY_KIB
    );
    assert_eq!(lower.time_cost.get(), 1);
    assert_eq!(lower.lanes.get(), 1);
}
