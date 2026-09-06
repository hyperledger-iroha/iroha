//! Operator configuration readback and node-local settings across a validator restart.
use eyre::WrapErr as _;
use integration_tests::sandbox;
use iroha_data_model::Level;
use iroha_test_network::NetworkBuilder;
use iroha_torii_shared::configuration::{Configuration, SoranetHandshakeSummary};
use nonzero_ext::nonzero;
use std::borrow::Cow;

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
    let baseline = network
        .peers()
        .iter()
        .map(|peer| peer.client().get_config())
        .collect::<eyre::Result<Vec<_>>>()?;
    for config in &baseline {
        assert_shared_settings(config);
        assert_eq!(config.logger.level, Level::INFO);
        assert_eq!(config.logger.filter.as_deref(), Some("iroha_p2p=info"));
        assert_pow(config, (6, 900, 120, 240, 8192, 1, 1));
    }

    // One validator receives a different local configuration on restart. Its
    // peers retain their own settings while the same four-validator committee
    // continues to exchange and apply blocks.
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

    let restarted = restarted_peer.client().get_config()?;
    assert_shared_settings(&restarted);
    assert_eq!(restarted.logger.level, Level::DEBUG);
    assert_eq!(restarted.logger.filter.as_deref(), Some("iroha_p2p=trace"));
    assert_pow(&restarted, (7, 720, 180, 360, 12288, 3, 3));
    assert_eq!(
        handshake_identity(&restarted.network.soranet_handshake),
        handshake_identity(&baseline[0].network.soranet_handshake),
        "local proof-of-work policy must preserve the handshake identity and negotiated suite",
    );

    let height = network.peers()[1].client().get_status()?.blocks;
    network.peers()[1].client().submit_blocking(
        iroha_data_model::isi::Log::new(Level::INFO, "node-local configuration isolation".into()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )?;
    runtime.block_on(network.ensure_blocks(height + 1))?;
    for (index, peer) in network.peers().iter().enumerate().skip(1) {
        let config = peer.client().get_config()?;
        assert_shared_settings(&config);
        assert_eq!(config.logger.level, baseline[index].logger.level);
        assert_eq!(config.logger.filter, baseline[index].logger.filter);
        assert_pow(&config, (6, 900, 120, 240, 8192, 1, 1));
        assert_eq!(
            handshake_identity(&config.network.soranet_handshake),
            handshake_identity(&baseline[index].network.soranet_handshake),
        );
    }
    let restarted = restarted_peer.client().get_config()?;
    assert_pow(&restarted, (7, 720, 180, 360, 12288, 3, 3));
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
