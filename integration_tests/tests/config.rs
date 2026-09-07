//! Four-validator retrieval of startup configuration and rejection of HTTP mutation.

use std::{io::Read as _, time::Duration};

use eyre::{Result, ensure, eyre};
use integration_tests::sandbox;
use iroha_config::client_api::ConfigGetDTO;
use iroha_data_model::Level;
use iroha_test_network::NetworkBuilder;
use nonzero_ext::nonzero;

fn assert_startup_configuration(config: &ConfigGetDTO) {
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
            let client = peer.client();
            let url = client.torii_url.join("/v1/configuration")?;
            let uri: iroha_torii::Uri = url.path().parse()?;
            let operator = client
                .operator_key_pair
                .as_ref()
                .ok_or_else(|| eyre!("test-network client is missing its operator key"))?;
            // Retain the exact signed GET carrier before exercising the native client's
            // decoder. A malformed server response must remain inspectable even when
            // Client::get_config correctly fails; this is never a fallback decoder.
            let read_headers = iroha_torii::operator_signed_request_headers(
                operator,
                &client.network_id,
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
            let before = client.get_config()?;
            assert_startup_configuration(&before);
            let body = norito::json::to_vec(&norito::json!({
                "logger": {"level": "ERROR"},
                "queue": {"capacity": 1}
            }))?;
            let headers = iroha_torii::operator_signed_request_headers(
                operator,
                &client.network_id,
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
            let after = client.get_config()?;
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
