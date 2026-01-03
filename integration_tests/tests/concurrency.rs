//! Concurrency integration tests: verify that configured scheduler limits are applied.

use std::fs;

use integration_tests::sandbox;
use iroha_test_network::{NetworkBuilder, init_instruction_registry};

fn skip_if_sandboxed(context: &str) -> bool {
    match std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)) {
        Ok(listener) => {
            drop(listener);
            false
        }
        Err(err) => {
            if err.kind() == std::io::ErrorKind::PermissionDenied
                || sandbox::sandbox_reason(&eyre::Report::new(err)).is_some()
            {
                eprintln!(
                    "sandboxed network restriction detected while running {context}; skipping"
                );
                true
            } else {
                false
            }
        }
    }
}

#[test]
fn config_layer_overrides_concurrency_settings() {
    if skip_if_sandboxed("config_layer_overrides_concurrency_settings") {
        return;
    }
    let network = NetworkBuilder::new()
        .with_config_layer(|c| {
            c.write(["concurrency", "scheduler_min_threads"], 3i64)
                .write(["concurrency", "scheduler_max_threads"], 7i64)
                .write(["logger", "level"], "TRACE");
        })
        .build();

    let mut layers = network.config_layers();

    // Trusted peers list is injected automatically as the first layer.
    let trusted = layers
        .next()
        .expect("trusted peers layer must be present")
        .into_owned();
    assert!(trusted.contains_key("trusted_peers"));

    // Base configuration built by the harness should retain default logger level.
    let base = layers
        .next()
        .expect("base config layer must be present")
        .into_owned();
    assert_eq!(
        base.get("logger")
            .and_then(toml::Value::as_table)
            .and_then(|logger| logger.get("level"))
            .and_then(toml::Value::as_str),
        Some("INFO")
    );

    // User layer should override concurrency and logger settings.
    let overrides = layers
        .next()
        .expect("user overrides layer must be present")
        .into_owned();
    let concurrency = overrides
        .get("concurrency")
        .and_then(toml::Value::as_table)
        .expect("concurrency table must exist");
    assert_eq!(
        concurrency
            .get("scheduler_min_threads")
            .and_then(toml::Value::as_integer),
        Some(3)
    );
    assert_eq!(
        concurrency
            .get("scheduler_max_threads")
            .and_then(toml::Value::as_integer),
        Some(7)
    );
    assert_eq!(
        overrides
            .get("logger")
            .and_then(toml::Value::as_table)
            .and_then(|logger| logger.get("level"))
            .and_then(toml::Value::as_str),
        Some("TRACE")
    );

    assert!(layers.next().is_none(), "no extra config layers expected");
}

#[test]
fn scheduler_limits_reflected_in_banner() -> eyre::Result<()> {
    init_instruction_registry();
    // Configure the node to use exactly 2 scheduler threads and start a 1‑peer network.
    let builder = NetworkBuilder::new().with_config_layer(|c| {
        c.write(["concurrency", "scheduler_min_threads"], 2i64)
            .write(["concurrency", "scheduler_max_threads"], 2i64)
            // Keep a small stdout log for verification
            .write(["logger", "level"], "INFO");
    });
    let Some((network, _rt)) = sandbox::start_network_blocking_or_skip(
        builder,
        stringify!(scheduler_limits_reflected_in_banner),
    )?
    else {
        return Ok(());
    };

    // Read the peer stdout log and assert the IVM banner reports the configured cores.
    let peer = network.peer();
    let log_path = peer
        .latest_stdout_log_path()
        .expect("stdout log file must exist after startup");
    let stdout = fs::read_to_string(log_path)?;
    assert!(
        stdout.contains("Using 2 cores"),
        "IVM banner should reflect configured scheduler cores"
    );

    Ok(())
}

#[test]
fn scheduler_limits_one_core_reflected_in_banner() -> eyre::Result<()> {
    init_instruction_registry();
    // Configure the node to use exactly 1 scheduler thread and start a 1‑peer network.
    let builder = NetworkBuilder::new().with_config_layer(|c| {
        c.write(["concurrency", "scheduler_min_threads"], 1i64)
            .write(["concurrency", "scheduler_max_threads"], 1i64)
            .write(["logger", "level"], "INFO");
    });
    let Some((network, _rt)) = sandbox::start_network_blocking_or_skip(
        builder,
        stringify!(scheduler_limits_one_core_reflected_in_banner),
    )?
    else {
        return Ok(());
    };

    let peer = network.peer();
    let log_path = peer
        .latest_stdout_log_path()
        .expect("stdout log file must exist after startup");
    let stdout = fs::read_to_string(log_path)?;
    assert!(
        stdout.contains("Using 1 core"),
        "IVM banner should reflect configured single-core scheduler"
    );

    Ok(())
}
