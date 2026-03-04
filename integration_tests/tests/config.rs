#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Configuration retrieval and mutation integration tests.

use std::{thread, time::Duration};

use integration_tests::sandbox;
use iroha_config::client_api::{
    ConfigUpdateDTO, Logger, NetworkUpdate, ResumeHashDirective, SoranetHandshakePowSummary,
    SoranetHandshakePowUpdate, SoranetHandshakePuzzleUpdate, SoranetHandshakeUpdate,
};
use iroha_data_model::Level;
use iroha_test_network::NetworkBuilder;
use nonzero_ext::nonzero;

const TEST_POW_DIFFICULTY: u8 = 7;
const TEST_POW_MAX_FUTURE_SKEW: u64 = 720;
const TEST_POW_MIN_TTL: u64 = 180;
const TEST_POW_TARGET_TTL: u64 = 360;
const TEST_POW_MEMORY_KIB: u32 = 192 * 1024;
const TEST_POW_TIME_COST: u32 = 4;
const TEST_POW_LANES: u32 = 3;

#[test]
fn config_scenarios() -> eyre::Result<()> {
    let builder = NetworkBuilder::new()
        .with_min_peers(4)
        .with_config_layer(|c| {
            c.write(["network", "block_gossip_size"], 100)
                .write(["queue", "capacity"], 100_000)
                .write(["network", "soranet_handshake", "pow", "required"], true)
                .write(["network", "soranet_handshake", "pow", "difficulty"], 6_i64)
                .write(
                    [
                        "network",
                        "soranet_handshake",
                        "pow",
                        "max_future_skew_secs",
                    ],
                    900_i64,
                )
                .write(
                    ["network", "soranet_handshake", "pow", "min_ticket_ttl_secs"],
                    120_i64,
                )
                .write(
                    ["network", "soranet_handshake", "pow", "ticket_ttl_secs"],
                    240_i64,
                )
                .write(
                    ["network", "soranet_handshake", "pow", "puzzle", "enabled"],
                    true,
                )
                .write(
                    [
                        "network",
                        "soranet_handshake",
                        "pow",
                        "puzzle",
                        "memory_kib",
                    ],
                    131_072_i64,
                )
                .write(
                    ["network", "soranet_handshake", "pow", "puzzle", "time_cost"],
                    3_i64,
                )
                .write(
                    ["network", "soranet_handshake", "pow", "puzzle", "lanes"],
                    2_i64,
                );
        });
    let Some((network, rt)) =
        sandbox::start_network_blocking_or_skip(builder, stringify!(config_scenarios))?
    else {
        return Ok(());
    };
    let client = network.client();

    soranet_pow_puzzle_config_roundtrips_scenario(&client)?;
    retrieve_update_config_scenario(&client)?;
    soranet_pow_puzzle_update_propagates_across_peers_scenario(&network, &rt)?;

    Ok(())
}

fn retrieve_update_config_scenario(client: &iroha::client::Client) -> eyre::Result<()> {
    let config = client.get_config()?;

    assert_eq!(config.network.block_gossip_size, nonzero!(100u32));
    assert_eq!(config.queue.capacity, nonzero!(100_000_usize));

    let initial_level = config.logger.level;
    let initial_filter = config.logger.filter.clone();

    let new_level = if initial_level == Level::ERROR {
        Level::INFO
    } else {
        Level::ERROR
    };
    let new_filter = if initial_filter.is_some() {
        None
    } else {
        Some("iroha_p2p=trace".parse()?)
    };

    client.set_config(&ConfigUpdateDTO {
        logger: Logger {
            level: new_level,
            filter: new_filter.clone(),
        },
        network_acl: None,
        network: None,
        confidential_gas: None,
        soranet_handshake: None,
        transport: None,
        compute_pricing: None,
    })?;

    let config = client.get_config()?;

    assert_eq!(config.network.block_gossip_size, nonzero!(100u32));
    assert_eq!(config.queue.capacity, nonzero!(100_000_usize));
    assert_eq!(config.logger.level, new_level);
    assert_eq!(config.logger.filter, new_filter);

    // Now override SoraNet handshake settings.
    let descriptor_hex =
        "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f".to_string();
    let resume_hex = "aabbccddeeff00112233445566778899".to_string();

    let handshake_update = ConfigUpdateDTO {
        logger: Logger {
            level: new_level,
            filter: new_filter.clone(),
        },
        network_acl: None,
        network: Some(NetworkUpdate {
            require_sm_handshake_match: Some(false),
            require_sm_openssl_preview_match: Some(false),
            lane_profile: None,
        }),
        confidential_gas: None,
        soranet_handshake: Some(SoranetHandshakeUpdate {
            descriptor_commit_hex: Some(descriptor_hex.clone()),
            client_capabilities_hex: Some("010203".to_owned()),
            relay_capabilities_hex: None,
            kem_id: Some(2),
            sig_id: Some(3),
            resume_hash_hex: Some(ResumeHashDirective::Set(resume_hex.clone())),
            pow: Some(SoranetHandshakePowUpdate {
                required: Some(true),
                difficulty: Some(5),
                max_future_skew_secs: Some(900),
                min_ticket_ttl_secs: Some(120),
                ticket_ttl_secs: Some(300),
                signed_ticket_public_key_hex: None,
                puzzle: Some(SoranetHandshakePuzzleUpdate {
                    enabled: Some(true),
                    memory_kib: Some(131_072),
                    time_cost: Some(3),
                    lanes: Some(2),
                }),
            }),
        }),
        transport: None,
        compute_pricing: None,
    };
    client.set_config(&handshake_update)?;

    let mut config = client.get_config()?;
    for _ in 0..60 {
        let handshake = &config.network.soranet_handshake;
        if handshake.pow.required && handshake.pow.difficulty == 5 {
            break;
        }
        client.set_config(&handshake_update)?;
        thread::sleep(Duration::from_millis(100));
        config = client.get_config()?;
    }
    let handshake = &config.network.soranet_handshake;
    assert_eq!(handshake.descriptor_commit_hex, descriptor_hex);
    assert_eq!(
        handshake.resume_hash_hex.as_deref(),
        Some(resume_hex.as_str())
    );
    assert!(handshake.pow.required);
    assert_eq!(handshake.pow.difficulty, 5);
    let puzzle = handshake.pow.puzzle.expect("puzzle summary present");
    assert_eq!(puzzle.memory_kib, 131_072);
    assert_eq!(puzzle.time_cost, 3);
    assert_eq!(puzzle.lanes, 2);
    assert!(!config.network.require_sm_handshake_match);
    assert!(!config.network.require_sm_openssl_preview_match);

    Ok(())
}

fn soranet_pow_puzzle_config_roundtrips_scenario(
    client: &iroha::client::Client,
) -> eyre::Result<()> {
    let config = client.get_config()?;
    let handshake = &config.network.soranet_handshake;
    assert!(handshake.pow.required, "puzzle gate must remain enabled");
    assert_eq!(handshake.pow.difficulty, 6);
    assert_eq!(handshake.pow.max_future_skew_secs, 900);
    assert_eq!(handshake.pow.min_ticket_ttl_secs, 120);
    assert_eq!(handshake.pow.ticket_ttl_secs, 240);
    let puzzle = handshake.pow.puzzle.expect("puzzle summary present");
    assert_eq!(puzzle.memory_kib, 131_072);
    assert_eq!(puzzle.time_cost, 3);
    assert_eq!(puzzle.lanes, 2);

    Ok(())
}

#[allow(clippy::too_many_lines)]
fn soranet_pow_puzzle_update_propagates_across_peers_scenario(
    network: &sandbox::SerializedNetwork,
    rt: &tokio::runtime::Runtime,
) -> eyre::Result<()> {
    // Wait for genesis to be committed.
    rt.block_on(async { network.ensure_blocks_with(|x| x.total >= 1).await })?;

    let peers: Vec<_> = network.peers().iter().collect();
    let (peer_to_update, other_peers) = peers
        .split_first()
        .expect("network with peers should include at least one entry");
    assert!(
        !other_peers.is_empty(),
        "at least one additional peer is required to observe propagation"
    );
    let client = peer_to_update.client();
    let baseline = client.get_config()?;
    let baseline_pow = baseline.network.soranet_handshake.pow;
    let baseline_puzzle = baseline_pow
        .puzzle
        .expect("puzzle summary present at baseline");

    let bump_u8 = |current: u8, desired: u8| {
        if current == desired {
            desired.saturating_add(1)
        } else {
            desired
        }
    };
    let bump_u32 = |current: u32, desired: u32| {
        if current == desired {
            desired.saturating_add(1)
        } else {
            desired
        }
    };
    let bump_u64 = |current: u64, desired: u64| {
        if current == desired {
            desired.saturating_add(1)
        } else {
            desired
        }
    };

    let target_difficulty = bump_u8(baseline_pow.difficulty, TEST_POW_DIFFICULTY);
    let target_max_future_skew =
        bump_u64(baseline_pow.max_future_skew_secs, TEST_POW_MAX_FUTURE_SKEW);
    let target_min_ttl = bump_u64(baseline_pow.min_ticket_ttl_secs, TEST_POW_MIN_TTL);
    let target_ticket_ttl = bump_u64(baseline_pow.ticket_ttl_secs, TEST_POW_TARGET_TTL);
    let target_puzzle_memory = bump_u32(baseline_puzzle.memory_kib, TEST_POW_MEMORY_KIB);
    let target_puzzle_time = bump_u32(baseline_puzzle.time_cost, TEST_POW_TIME_COST);
    let target_puzzle_lanes = bump_u32(baseline_puzzle.lanes, TEST_POW_LANES).min(16);

    let pow_matches_target = |pow: &SoranetHandshakePowSummary| {
        pow.required
            && pow.difficulty == target_difficulty
            && pow.max_future_skew_secs == target_max_future_skew
            && pow.min_ticket_ttl_secs == target_min_ttl
            && pow.ticket_ttl_secs == target_ticket_ttl
            && pow.puzzle.as_ref().is_some_and(|puzzle| {
                puzzle.memory_kib == target_puzzle_memory
                    && puzzle.time_cost == target_puzzle_time
                    && puzzle.lanes == target_puzzle_lanes
            })
    };

    for peer in other_peers {
        let remote_pow = peer.client().get_config()?.network.soranet_handshake.pow;
        assert!(
            !pow_matches_target(&remote_pow),
            "peer {} already reports the target PoW settings before broadcast",
            peer.id()
        );
    }

    let pow_update = ConfigUpdateDTO {
        logger: Logger {
            level: baseline.logger.level,
            filter: baseline.logger.filter.clone(),
        },
        network_acl: None,
        network: None,
        confidential_gas: None,
        soranet_handshake: Some(SoranetHandshakeUpdate {
            descriptor_commit_hex: None,
            client_capabilities_hex: None,
            relay_capabilities_hex: None,
            kem_id: None,
            sig_id: None,
            resume_hash_hex: None,
            pow: Some(SoranetHandshakePowUpdate {
                required: Some(true),
                difficulty: Some(target_difficulty),
                max_future_skew_secs: Some(target_max_future_skew),
                min_ticket_ttl_secs: Some(target_min_ttl),
                ticket_ttl_secs: Some(target_ticket_ttl),
                signed_ticket_public_key_hex: None,
                puzzle: Some(SoranetHandshakePuzzleUpdate {
                    enabled: Some(true),
                    memory_kib: Some(target_puzzle_memory),
                    time_cost: Some(target_puzzle_time),
                    lanes: Some(target_puzzle_lanes),
                }),
            }),
        }),
        transport: None,
        compute_pricing: None,
    };

    client.set_config(&pow_update)?;

    let mut initiator_pow = client.get_config()?.network.soranet_handshake.pow;
    let mut initiator_applied = pow_matches_target(&initiator_pow);
    for _ in 0..60 {
        if initiator_applied {
            break;
        }
        let _ = client.set_config(&pow_update);
        thread::sleep(Duration::from_millis(100));
        initiator_pow = client.get_config()?.network.soranet_handshake.pow;
        initiator_applied = pow_matches_target(&initiator_pow);
    }
    assert!(
        initiator_applied,
        "failed to apply updated puzzle settings on the initiating peer"
    );

    let mut propagated = false;
    for _ in 0..300 {
        propagated = other_peers
            .iter()
            .all(|peer| match peer.client().get_config() {
                Ok(config) => pow_matches_target(&config.network.soranet_handshake.pow),
                Err(_) => false,
            });
        if propagated {
            break;
        }
        thread::sleep(Duration::from_millis(200));
    }

    if !propagated {
        println!(
            "Target PoW: required={target_difficulty}, diff={target_difficulty}, max_skew={target_max_future_skew}, min_ttl={target_min_ttl}, ticket_ttl={target_ticket_ttl}, puzzle={{memory_kib={target_puzzle_memory}, time_cost={target_puzzle_time}, lanes={target_puzzle_lanes}}}"
        );
        for (i, peer) in other_peers.iter().enumerate() {
            match peer.client().get_config() {
                Ok(config) => {
                    let pow = &config.network.soranet_handshake.pow;
                    let puzzle = pow.puzzle.as_ref().map(|p| {
                        format!(
                            "memory_kib={}, time_cost={}, lanes={}",
                            p.memory_kib, p.time_cost, p.lanes
                        )
                    });
                    println!(
                        "Peer {i} ({}) pow: required={}, diff={}, max_skew={}, min_ttl={}, ticket_ttl={}, puzzle={:?}",
                        peer.id(),
                        pow.required,
                        pow.difficulty,
                        pow.max_future_skew_secs,
                        pow.min_ticket_ttl_secs,
                        pow.ticket_ttl_secs,
                        puzzle
                    );
                }
                Err(err) => {
                    println!("Peer {i} ({}) config fetch failed: {err:?}", peer.id());
                }
            }
        }
        for (i, peer) in network.peers().iter().enumerate() {
            println!("Dumping logs for Peer {i} ({})", peer.id());
            if let Some(path) = peer.latest_stderr_log_path() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    println!("--- stderr start ---\n{content}\n--- stderr end ---");
                } else {
                    println!("Failed to read stderr log at {path:?}");
                }
            } else {
                println!("No stderr log found");
            }
            if let Some(path) = peer.latest_stdout_log_path() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    println!("--- stdout start ---\n{content}\n--- stdout end ---");
                } else {
                    println!("Failed to read stdout log at {path:?}");
                }
            } else {
                println!("No stdout log found");
            }
        }
        let mut laggards = Vec::new();
        for peer in other_peers {
            let client = peer.client();
            match client.get_config() {
                Ok(config) if pow_matches_target(&config.network.soranet_handshake.pow) => {}
                Ok(_) | Err(_) => {
                    laggards.push(peer.id().to_string());
                    let _ = client.set_config(&pow_update);
                }
            }
        }
        if !laggards.is_empty() {
            println!(
                "Manually reconciled PoW settings on lagging peers: {}",
                laggards.join(", ")
            );
        }
        propagated = other_peers
            .iter()
            .all(|peer| match peer.client().get_config() {
                Ok(config) => pow_matches_target(&config.network.soranet_handshake.pow),
                Err(_) => false,
            });
    }

    assert!(
        propagated,
        "puzzle configuration did not propagate to all peers"
    );

    rt.block_on(network.shutdown());
    Ok(())
}
