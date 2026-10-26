//! Tests for Izanami-generated network configuration layers.

use super::*;

#[test]
fn selected_nondefault_staking_recipe_reaches_native_genesis_without_node_overrides() -> Result<()>
{
    use iroha_data_model::block::consensus::PublicLaneStakingPolicy;
    let mut profile = crate::config::NexusProfile::sora_defaults()?;
    profile.staking_policy.min_validator_stake = 37_u64.into();
    profile.staking_policy.max_stake_shares_per_validator = 13_u32.try_into().unwrap();
    profile.staking_policy.max_pending_unbonds_per_share = 9_u32.try_into().unwrap();
    profile.staking_policy.unbonding_delay_ms = 987_654;
    profile.staking_policy.max_slash_bps = 997;
    let expected = instructions::nexus_staking_policy(&profile)?;
    let config = ChaosConfig {
        seed: Some(37),
        nexus: Some(profile),
        ..test_chaos_config()
    };
    let network = test_prepared_network_builder(&config)?.build();
    let genesis = network.genesis();
    assert!(
        genesis
            .0
            .output_results()
            .all(|result| result.as_ref().is_ok())
    );
    let originals = genesis
        .0
        .external_transactions()
        .filter_map(|transaction| match transaction.instructions() {
            Executable::Instructions(instructions) => Some(instructions.iter()),
            _ => None,
        })
        .flatten()
        .filter_map(|instruction| instruction.as_any().downcast_ref::<SetParameter>())
        .filter_map(|set| match set.inner() {
            Parameter::Custom(custom)
                if custom.id() == &PublicLaneStakingPolicy::parameter_id() =>
            {
                Some(custom)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        originals.len(),
        1,
        "one authored policy across preview and final signing"
    );
    assert_eq!(
        PublicLaneStakingPolicy::from_custom_parameter(originals[0])?,
        expected
    );
    for layer in network.config_layers() {
        if let Some(staking) = layer
            .get("nexus")
            .and_then(TomlValue::as_table)
            .and_then(|nexus| nexus.get("staking"))
            .and_then(TomlValue::as_table)
        {
            for field in [
                "min_validator_stake",
                "max_stake_shares_per_validator",
                "max_pending_unbonds_per_share",
                "unbonding_delay_ms",
                "max_slash_bps",
                "stake_asset_id",
                "stake_escrow_account_id",
                "slash_sink_account_id",
            ] {
                assert!(
                    !staking.contains_key(field),
                    "retired node monetary field {field}"
                );
            }
        }
    }
    Ok(())
}

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
    // The explicit loopback bypass owns local load generation. Authority limits
    // retain the valid runtime defaults instead of emitting a retired zero sentinel.
    for limit in [
        "query_rate_per_authority_per_sec",
        "query_burst_per_authority",
        "tx_rate_per_authority_per_sec",
        "tx_burst_per_authority",
    ] {
        assert!(lookup(&["torii", limit]).is_none());
    }
    assert_eq!(
        lookup(&["torii", "api_rate_limit_bypass_cidrs"])
            .and_then(TomlValue::as_array)
            .map(|cidrs| cidrs
                .iter()
                .map(|cidr| cidr.as_str().unwrap())
                .collect::<Vec<_>>()),
        Some(vec!["127.0.0.0/8", "::1/128"])
    );
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
