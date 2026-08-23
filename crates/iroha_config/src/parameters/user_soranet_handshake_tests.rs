use super::*;

fn test_handshake_config(kem_id: u8, sig_id: u8) -> SoranetHandshake {
    SoranetHandshake {
        descriptor_commit: WithOrigin::inline(SoranetHandshake::default_descriptor_commit()),
        client_capabilities: WithOrigin::inline(SoranetHandshake::default_client_capabilities()),
        relay_capabilities: WithOrigin::inline(SoranetHandshake::default_relay_capabilities()),
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
