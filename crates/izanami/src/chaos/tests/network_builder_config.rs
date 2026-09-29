//! Tests for Izanami-generated network configuration layers.

use super::*;

#[test]
fn make_network_builder_emits_only_native_sumeragi_config() -> Result<()> {
    init_instruction_registry();
    let pipeline_time = Duration::from_millis(300);
    let config = ChaosConfig {
        pipeline_time: Some(pipeline_time),
        seed: Some(17),
        ..test_chaos_config()
    };
    let network = test_prepared_network_builder(&config)?.build();
    assert_eq!(network.block_cadence(), pipeline_time);
    let layers: Vec<Table> = network.config_layers().map(Cow::into_owned).collect();
    let lookup = |path: &[&str]| {
        layers.iter().rev().find_map(|layer| {
            let mut current = layer;
            for (idx, key) in path.iter().enumerate() {
                let value = current.get(*key)?;
                if idx + 1 == path.len() {
                    return Some(value);
                }
                current = value.as_table()?;
            }
            None
        })
    };
    assert!(lookup(&["sumeragi", "round_timeout_ms"]).is_none());
    assert_eq!(
        lookup(&["sumeragi", "role"]).and_then(TomlValue::as_str),
        Some("validator")
    );
    assert!(lookup(&["sumeragi", "block"]).is_none());
    assert!(lookup(&["sumeragi", "queues"]).is_none());
    assert_eq!(
        lookup(&["network", "max_total_connections"]).and_then(TomlValue::as_integer),
        Some(i64::try_from(IZANAMI_MAX_TOTAL_CONNECTIONS).expect("connection capacity fits TOML"))
    );
    assert_eq!(
        lookup(&["sumeragi", "keys", "allowed_algorithms"])
            .and_then(TomlValue::as_array)
            .and_then(|algorithms| algorithms.first())
            .and_then(TomlValue::as_str),
        Some("bls_normal")
    );
    for layer in &layers {
        let Some(sumeragi) = layer.get("sumeragi").and_then(TomlValue::as_table) else {
            continue;
        };
        for retired in [
            "consensus_mode",
            "protocol_version",
            "da",
            "advanced",
            "recovery",
            "gating",
            "collectors",
            "persistence",
        ] {
            assert!(
                !sumeragi.contains_key(retired),
                "retired sumeragi.{retired} must not be generated"
            );
        }
    }
    Ok(())
}
#[test]
fn make_network_builder_forwards_rust_log_and_sets_peer_base_level() -> Result<()> {
    init_instruction_registry();
    let _env_guard = EnvGuard::set("RUST_LOG", "iroha_p2p=debug,iroha_core=debug");
    let config = ChaosConfig {
        seed: Some(19),
        ..test_chaos_config()
    };
    let Some(network) = build_test_prepared_network(&config)? else {
        return Ok(());
    };
    let layers: Vec<Table> = network.config_layers().map(Cow::into_owned).collect();
    let read_str = |layer: &Table, path: &[&str]| -> Option<String> {
        let mut current = layer;
        for (idx, key) in path.iter().enumerate() {
            let value = current.get(*key)?;
            if idx + 1 == path.len() {
                return value.as_str().map(ToString::to_string);
            }
            current = value.as_table()?;
        }
        None
    };
    let filter = layers
        .iter()
        .rev()
        .find_map(|layer| read_str(layer, &["logger", "filter"]));
    assert_eq!(filter.as_deref(), Some("iroha_p2p=debug,iroha_core=debug"));
    let level = layers
        .iter()
        .rev()
        .find_map(|layer| read_str(layer, &["logger", "level"]));
    assert_eq!(level.as_deref(), Some(IZANAMI_PEER_LOG_BASE_LEVEL));
    Ok(())
}
