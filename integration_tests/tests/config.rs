//! Operator configuration readback and node-local settings across a validator restart.
use eyre::{Result, WrapErr as _, ensure, eyre};
use integration_tests::sandbox;
use iroha_data_model::Level;
use iroha_primitives::nonzero;
use iroha_test_network::NetworkBuilder;
use iroha_torii_shared::configuration::{Configuration, SoranetHandshakeSummary};
use std::borrow::Cow;
use std::{io::Read as _, time::Duration};

#[test]
fn configuration_readback_and_node_local_restart() -> eyre::Result<()> {
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_config_table(toml::toml! {
            [logger]
            level = "INFO"
            filter = "iroha_p2p=info"
            [nexus.storage]
            local_budget_bytes = 1_073_741_824
            [network]
            block_gossip_size = 100
            [queue]
            capacity = 100_000
            [network.soranet_handshake.pow]
            difficulty = 6
            max_future_skew_secs = 900
            min_ticket_ttl_secs = 120
            ticket_ttl_secs = 240
            [network.soranet_handshake.pow.puzzle]
            memory_kib = 8192
            time_cost = 1
            lanes = 1
        });
    let Some((network, runtime)) = sandbox::start_network_blocking_or_skip(
        builder,
        stringify!(configuration_readback_and_node_local_restart),
    )?
    else {
        return Ok(());
    };
    runtime.block_on(network.ensure_blocks(1))?;
    assert_eq!(network.peers().len(), 4);
    let operators = network
        .peers()
        .iter()
        .map(|peer| {
            let client = peer.client();
            let key_pair = client
                .client()
                .operator_key_pair()
                .cloned()
                .ok_or_else(|| eyre!("test-network client is missing its operator key"))?;
            client.operator_client(key_pair)
        })
        .collect::<eyre::Result<Vec<_>>>()?;
    let baseline = operators
        .iter()
        .map(|operator| operator.configuration().get())
        .collect::<iroha::Result<Vec<_>>>()?;
    for config in &baseline {
        assert_shared_settings(config);
        assert_eq!(config.logger.level, Level::INFO);
        assert_eq!(config.logger.filter.as_deref(), Some("iroha_p2p=info"));
        assert_pow(config, (6, 900, 120, 240, 8192, 1, 1));
    }

    // One validator receives a different local configuration on restart. Readback
    // must retain every setting independently of the other validators.
    let restarted_peer = &network.peers()[0];
    let mut layers: Vec<_> = network.config_layers_for_peer(restarted_peer).collect();
    layers.push(Cow::Owned(toml::toml! {
        [logger]
        level = "DEBUG"
        filter = "iroha_p2p=trace"
        [network.soranet_handshake.pow]
        difficulty = 7
        max_future_skew_secs = 720
        min_ticket_ttl_secs = 180
        ticket_ttl_secs = 360
        [network.soranet_handshake.pow.puzzle]
        memory_kib = 12288
        time_cost = 3
        lanes = 3
    }));
    runtime.block_on(async {
        restarted_peer.shutdown().await;
        tokio::time::timeout(
            network.peer_startup_timeout(),
            restarted_peer.start_checked(layers.iter(), None),
        )
        .await
        .wrap_err("configured validator restart exceeded its startup deadline")??;
        Ok::<_, eyre::Report>(())
    })?;

    let restarted = operators[0].configuration().get()?;
    assert_shared_settings(&restarted);
    assert_eq!(restarted.logger.level, Level::DEBUG);
    assert_eq!(restarted.logger.filter.as_deref(), Some("iroha_p2p=trace"));
    assert_pow(&restarted, (7, 720, 180, 360, 12288, 3, 3));
    assert_eq!(
        handshake_identity(&restarted.network.soranet_handshake),
        handshake_identity(&baseline[0].network.soranet_handshake),
        "local proof-of-work policy must preserve the handshake identity and negotiated suite",
    );

    // Tickets are minted with the sender's local work factors and verified with
    // the receiver's policy; unequal difficulty is rejected before hashing. Keep
    // the changed timing policy and logger, but restore shared work factors before
    // requiring all four validators to exchange and apply the next block.
    layers.push(Cow::Owned(toml::toml! {
        [network.soranet_handshake.pow]
        difficulty = 6
        [network.soranet_handshake.pow.puzzle]
        memory_kib = 8192
        time_cost = 1
        lanes = 1
    }));
    runtime.block_on(async {
        restarted_peer.shutdown().await;
        tokio::time::timeout(
            network.peer_startup_timeout(),
            restarted_peer.start_checked(layers.iter(), None),
        )
        .await
        .wrap_err("compatible-work validator restart exceeded its startup deadline")??;
        Ok::<_, eyre::Report>(())
    })?;
    let compatible = operators[0].configuration().get()?;
    assert_shared_settings(&compatible);
    assert_pow(&compatible, (6, 720, 180, 360, 8192, 1, 1));
    assert_eq!(compatible.logger.level, Level::DEBUG);
    assert_eq!(compatible.logger.filter.as_deref(), Some("iroha_p2p=trace"));
    assert_eq!(
        handshake_identity(&compatible.network.soranet_handshake),
        handshake_identity(&baseline[0].network.soranet_handshake),
    );

    let height = network.peers()[1].client().status().get()?.blocks;
    network.peers()[1].client().submit(
        iroha_data_model::isi::Log::new(Level::INFO, "node-local configuration isolation".into()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    runtime.block_on(network.ensure_blocks(height + 1))?;
    for (index, operator) in operators.iter().enumerate().skip(1) {
        let config = operator.configuration().get()?;
        assert_shared_settings(&config);
        assert_eq!(config.logger.level, baseline[index].logger.level);
        assert_eq!(config.logger.filter, baseline[index].logger.filter);
        assert_pow(&config, (6, 900, 120, 240, 8192, 1, 1));
        assert_eq!(
            handshake_identity(&config.network.soranet_handshake),
            handshake_identity(&baseline[index].network.soranet_handshake),
        );
    }
    let restarted = operators[0].configuration().get()?;
    assert_pow(&restarted, (6, 720, 180, 360, 8192, 1, 1));
    assert_eq!(restarted.logger.level, Level::DEBUG);
    runtime.block_on(network.shutdown());
    Ok(())
}

fn assert_shared_settings(config: &Configuration) {
    assert_eq!(config.network.block_gossip_size, nonzero!(100u32));
    assert_eq!(config.queue.capacity, nonzero!(100_000_usize));
    assert!(config.network.require_sm_handshake_match);
    assert!(config.network.require_sm_openssl_preview_match);
}

fn assert_pow(config: &Configuration, expected: (u8, u64, u64, u64, u32, u32, u32)) {
    let pow = config.network.soranet_handshake.pow;
    assert_eq!(
        (
            pow.difficulty,
            pow.max_future_skew_secs,
            pow.min_ticket_ttl_secs,
            pow.ticket_ttl_secs,
            pow.puzzle.memory_kib,
            pow.puzzle.time_cost,
            pow.puzzle.lanes,
        ),
        expected,
    );
}

fn handshake_identity(value: &SoranetHandshakeSummary) -> (&str, &str, &str, u8, u8, Option<&str>) {
    (
        &value.descriptor_commit_hex,
        &value.client_capabilities_hex,
        &value.relay_capabilities_hex,
        value.kem_id,
        value.sig_id,
        value.resume_hash_hex.as_deref(),
    )
}

#[test]
fn restart_readback_policy_requires_compatible_handshake_work() {
    use iroha_crypto::soranet::{pow, puzzle};
    use std::num::NonZeroU32;
    use std::time::UNIX_EPOCH;

    let policy = |memory, time, lanes, difficulty, skew, minimum| {
        puzzle::Parameters::try_new(
            NonZeroU32::new(memory).unwrap(),
            NonZeroU32::new(time).unwrap(),
            NonZeroU32::new(lanes).unwrap(),
            difficulty,
            Duration::from_secs(skew),
            Duration::from_secs(minimum),
        )
        .unwrap()
    };
    let baseline = policy(8192, 1, 1, 6, 900, 120);
    let higher_work = policy(12288, 3, 3, 7, 720, 180);
    let compatible = policy(8192, 1, 1, 6, 720, 180);
    let binding = puzzle::ChallengeBinding::new(&[0x31; 32], &[0x42; 32], &[0x53; 32]);
    // A fixed public ticket keeps verification independent of wall-clock time and search cost.
    let ticket = pow::Ticket {
        version: pow::Ticket::VERSION,
        difficulty: 6,
        expires_at: 1_793_798_821,
        client_nonce: [
            138, 54, 128, 39, 175, 88, 190, 69, 234, 40, 12, 96, 126, 7, 85, 57, 223, 84, 88, 221,
            115, 204, 76, 11, 161, 182, 241, 7, 156, 237, 123, 240,
        ],
        solution: [
            218, 6, 165, 2, 77, 228, 142, 120, 147, 61, 34, 137, 135, 155, 136, 167, 20, 113, 255,
            62, 174, 56, 111, 149, 140, 104, 196, 14, 9, 9, 95, 112,
        ],
    };
    for ttl in [240, 360] {
        let now = UNIX_EPOCH + Duration::from_secs(ticket.expires_at - ttl);
        assert!(puzzle::verify_at(&ticket, &binding, &baseline, now).is_ok());
        assert!(puzzle::verify_at(&ticket, &binding, &compatible, now).is_ok());
        assert!(matches!(
            puzzle::verify_at(&ticket, &binding, &higher_work, now),
            Err(puzzle::Error::DifficultyMismatch {
                ticket: 6,
                required: 7
            })
        ));
    }
}

fn assert_startup_configuration(config: &Configuration) {
    assert_eq!(config.network.block_gossip_size, nonzero!(100_u32));
    assert_eq!(config.queue.capacity, nonzero!(100_000_usize));
    assert_eq!(config.logger.level, Level::INFO);
    assert_eq!(
        config
            .logger
            .filter
            .as_ref()
            .map(ToString::to_string)
            .as_deref(),
        Some("iroha_p2p=warn")
    );
    let pow = &config.network.soranet_handshake.pow;
    assert_eq!(pow.difficulty, 6);
    assert_eq!(pow.max_future_skew_secs, 900);
    assert_eq!(pow.min_ticket_ttl_secs, 120);
    assert_eq!(pow.ticket_ttl_secs, 240);
    assert_eq!(pow.puzzle.memory_kib, 8 * 1024);
    assert_eq!(pow.puzzle.time_cost, 1);
    assert_eq!(pow.puzzle.lanes, 1);
    assert!(config.network.require_sm_handshake_match);
    assert!(config.network.require_sm_openssl_preview_match);
}

#[test]
fn startup_configuration_is_read_only_on_four_validators() -> Result<()> {
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_config_layer(|config| {
            config
                .write(
                    ["nexus", "storage", "local_budget_bytes"],
                    1_073_741_824_i64,
                )
                .write(["network", "block_gossip_size"], 100)
                .write(["queue", "capacity"], 100_000)
                .write(["logger", "level"], "INFO")
                .write(["logger", "filter"], "iroha_p2p=warn")
                .write(["network", "soranet_handshake", "pow", "difficulty"], 6)
                .write(
                    [
                        "network",
                        "soranet_handshake",
                        "pow",
                        "max_future_skew_secs",
                    ],
                    900,
                )
                .write(
                    ["network", "soranet_handshake", "pow", "min_ticket_ttl_secs"],
                    120,
                )
                .write(
                    ["network", "soranet_handshake", "pow", "ticket_ttl_secs"],
                    240,
                )
                .write(
                    [
                        "network",
                        "soranet_handshake",
                        "pow",
                        "puzzle",
                        "memory_kib",
                    ],
                    8 * 1024,
                )
                .write(
                    ["network", "soranet_handshake", "pow", "puzzle", "time_cost"],
                    1,
                )
                .write(
                    ["network", "soranet_handshake", "pow", "puzzle", "lanes"],
                    1,
                );
        });
    let Some((network, runtime)) = sandbox::start_network_blocking_or_skip(
        builder,
        stringify!(startup_configuration_is_read_only_on_four_validators),
    )?
    else {
        return Ok(());
    };
    let result = (|| {
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        for peer in network.peers() {
            let blocking_client = peer.client();
            let client = blocking_client.client();
            let url = client.endpoint().join("/v1/configuration")?;
            let uri: iroha_torii::Uri = url.path().parse()?;
            let operator = client
                .operator_key_pair()
                .ok_or_else(|| eyre!("test-network client is missing its operator key"))?;
            // Retain the exact signed GET carrier before exercising the native client's
            // decoder. A malformed server response must remain inspectable even when
            // configuration().get() correctly fails; this is never a fallback decoder.
            let read_headers = iroha_torii::operator_signed_request_headers(
                operator,
                client.network_id(),
                &iroha_torii::Method::GET,
                &uri,
                &[],
            )?;
            let read_response = http
                .get(url.clone())
                .headers(read_headers)
                .header("Accept", "application/json")
                .send()?;
            ensure!(
                read_response.status() == reqwest::StatusCode::OK,
                "signed configuration GET returned {}",
                read_response.status()
            );
            let mut carrier = Vec::new();
            read_response
                .take(512 * 1024 + 1)
                .read_to_end(&mut carrier)?;
            ensure!(
                carrier.len() <= 512 * 1024,
                "configuration carrier exceeds test bound"
            );
            let storage = peer.kura_store_dir();
            let evidence = storage
                .parent()
                .ok_or_else(|| eyre!("peer storage has no evidence parent"))?
                .join("configuration-response.json");
            std::fs::write(evidence, &carrier)?;
            // Native clients sign GET with the explicitly configured peer operator key.
            let operator_client = blocking_client.operator_client(operator.clone())?;
            let before = operator_client.configuration().get()?;
            assert_startup_configuration(&before);
            let body = norito::json::to_vec(&norito::json!({
                "logger": {"level": "ERROR"},
                "queue": {"capacity": 1}
            }))?;
            let headers = iroha_torii::operator_signed_request_headers(
                operator,
                client.network_id(),
                &iroha_torii::Method::POST,
                &uri,
                &body,
            )?;
            let response = http
                .post(url)
                .headers(headers)
                .header("Content-Type", "application/json")
                .header("Accept", "application/json")
                .body(body)
                .send()?;
            ensure!(
                response.status() == reqwest::StatusCode::METHOD_NOT_ALLOWED,
                "configuration mutation must be rejected by method admission, got {}",
                response.status()
            );
            let mut bytes = Vec::new();
            response.take(16 * 1024 + 1).read_to_end(&mut bytes)?;
            ensure!(
                bytes.len() <= 16 * 1024,
                "method rejection exceeds its test carrier bound"
            );
            let error: norito::json::Value = norito::json::from_slice(&bytes)?;
            ensure!(
                error["code"].as_str() == Some("method_not_allowed"),
                "configuration POST returned the wrong rejection"
            );
            let after = operator_client.configuration().get()?;
            assert_startup_configuration(&after);
            assert_eq!(
                norito::json::to_vec(&before)?,
                norito::json::to_vec(&after)?,
                "unsupported POST changed effective configuration"
            );
        }
        Ok::<(), eyre::Report>(())
    })();
    runtime.block_on(network.shutdown());
    result
}
