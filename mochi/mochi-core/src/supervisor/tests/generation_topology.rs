fn peer_config_table(peer: &PeerHandle) -> toml::Table {
    toml::from_str(&fs::read_to_string(peer.config_path()).expect("read current peer config"))
        .expect("parse current peer config")
}

fn assert_shallow_network_overlay_preserves_managed_base(fields: &[(&str, i64)]) {
    let temp = tempfile::tempdir().expect("temp dir");
    let paths = NetworkPaths::from_root(
        temp.path(),
        &NetworkProfile::from_preset(ProfilePreset::FourPeerBft),
    );
    paths.ensure().expect("paths");
    let specs = (0_u16..4)
        .map(|index| {
            test_peer_spec(&paths, format!("peer{index}"), 8_080 + index, 1_337 + index)
                .expect("peer spec")
        })
        .collect::<Vec<_>>();
    let genesis = test_genesis_material(&paths);
    let spec = &specs[0];
    spec.write_config(
        "demo-chain",
        &genesis,
        &specs,
        &PeerConfigOverrides::default(),
        &[],
    )
    .expect("write base config");
    let base = toml::from_str::<toml::Table>(
        &fs::read_to_string(&spec.config_path).expect("read base config"),
    )
    .expect("parse base config");
    let base_network = base
        .get("network")
        .and_then(toml::Value::as_table)
        .cloned()
        .expect("base network table");
    let base_trusted = base.get("trusted_peers").cloned();

    let mut network = toml::Table::new();
    for (field, value) in fields {
        network.insert((*field).to_owned(), toml::Value::Integer(*value));
    }
    let mut overlay = toml::Table::new();
    overlay.insert("network".into(), toml::Value::Table(network));
    spec.write_config(
        "demo-chain",
        &genesis,
        &specs,
        &PeerConfigOverrides::default(),
        &[overlay],
    )
    .expect("write shallow network overlay");

    let overlaid = toml::from_str::<toml::Table>(
        &fs::read_to_string(&spec.config_path).expect("read overlaid config"),
    )
    .expect("parse overlaid config");
    let mut overlaid_network = overlaid
        .get("network")
        .and_then(toml::Value::as_table)
        .cloned()
        .expect("overlaid network table");
    for (field, value) in fields {
        assert_eq!(
            overlaid_network.remove(*field),
            Some(toml::Value::Integer(*value))
        );
    }
    assert_eq!(overlaid_network, base_network);
    assert_eq!(overlaid.get("trusted_peers").cloned(), base_trusted);

    let config = actual::Root::from_toml_source(
        TomlSource::from_file(&spec.config_path).expect("read generated peer config"),
    )
    .expect("parse generated peer config");
    validate_managed_peer_paths(&config, spec, specs.len())
        .expect("shallow network overlay keeps managed runtime paths");
    let expected_roster = specs
        .iter()
        .map(|candidate| {
            (
                candidate.keys.public_key.clone(),
                candidate.keys.pop.clone(),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    managed_paths::validate_candidate_peer_topology(&config, spec, &specs, &expected_roster)
        .expect("shallow network overlay keeps exact peer topology");
}

#[test]
fn shallow_latency_overlay_preserves_managed_network_and_topology() {
    assert_shallow_network_overlay_preserves_managed_base(&[
        ("block_gossip_period_ms", 1_250),
        ("transaction_gossip_period_ms", 1_250),
    ]);
}

#[test]
fn shallow_packet_loss_overlay_preserves_managed_network_and_topology() {
    assert_shallow_network_overlay_preserves_managed_base(&[
        ("debug_packet_loss_inbound_percent", 37),
        ("debug_packet_loss_outbound_percent", 37),
    ]);
}

#[test]
fn shallow_network_overlay_rejects_managed_pow_redirect() {
    let temp = tempfile::tempdir().expect("temp dir");
    let paths = NetworkPaths::from_root(
        temp.path(),
        &NetworkProfile::from_preset(ProfilePreset::FourPeerBft),
    );
    paths.ensure().expect("paths");
    let specs = (0_u16..4)
        .map(|index| {
            test_peer_spec(&paths, format!("peer{index}"), 8_080 + index, 1_337 + index)
                .expect("peer spec")
        })
        .collect::<Vec<_>>();
    let genesis = test_genesis_material(&paths);
    let spec = &specs[0];
    let mut pow = toml::Table::new();
    pow.insert(
        "revocation_store_path".into(),
        toml::Value::String("/tmp/shared-ticket-revocations.norito".to_owned()),
    );
    let mut handshake = toml::Table::new();
    handshake.insert("pow".into(), toml::Value::Table(pow));
    let mut network = toml::Table::new();
    network.insert("soranet_handshake".into(), toml::Value::Table(handshake));
    let mut overlay = toml::Table::new();
    overlay.insert("network".into(), toml::Value::Table(network));

    let error = spec
        .write_config(
            "demo-chain",
            &genesis,
            &specs,
            &PeerConfigOverrides::default(),
            &[overlay],
        )
        .expect_err("managed local PoW redirect must fail closed");
    assert!(
        matches!(
            error,
            SupervisorError::Config(ref message)
                if message.contains("full managed local network.soranet_handshake.pow")
        ),
        "unexpected managed PoW redirect error: {error:?}"
    );
}

#[test]
fn overlay_rejects_managed_network_bind_and_public_address_redirects() {
    if !ports_available("overlay_rejects_managed_network_bind_and_public_address_redirects") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _kagami = KagamiStub::install(temp.path());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");
    let selected_generation = supervisor.generation_id().to_owned();

    for (field, redirected) in [
        ("address", "127.0.0.1:65000"),
        ("public_address", "127.0.0.1:65001"),
    ] {
        let mut network = peer_config_table(&supervisor.peers()[0])
            .remove("network")
            .and_then(|value| value.as_table().cloned())
            .expect("network table");
        network.insert(
            field.to_owned(),
            toml::Value::String(
                socket_addr_literal(redirected, field).expect("render redirected address"),
            ),
        );
        let mut overlay = toml::Table::new();
        overlay.insert("network".into(), toml::Value::Table(network));

        let error = supervisor
            .restart_peer_with_extra_layers("peer0", &[overlay])
            .expect_err("managed network address redirect must fail before publication");
        assert!(
            matches!(
                error,
                SupervisorError::Config(ref message)
                    if message.contains(&format!("managed network.{field}"))
            ),
            "unexpected error for {field}: {error:?}"
        );
        assert_eq!(supervisor.generation_id(), selected_generation);
    }
}

#[test]
fn candidate_validation_rejects_network_address_redirects() {
    let temp = tempfile::tempdir().expect("temp dir");
    let paths = NetworkPaths::from_root(
        temp.path(),
        &NetworkProfile::from_preset(ProfilePreset::FourPeerBft),
    );
    paths.ensure().expect("paths");
    let specs = (0_u16..4)
        .map(|index| {
            test_peer_spec(&paths, format!("peer{index}"), 8_080 + index, 1_337 + index)
                .expect("peer spec")
        })
        .collect::<Vec<_>>();
    let genesis = test_genesis_material(&paths);
    let spec = &specs[0];
    spec.write_config(
        "demo-chain",
        &genesis,
        &specs,
        &PeerConfigOverrides::default(),
        &[],
    )
    .expect("write config");
    let expected_roster = specs
        .iter()
        .map(|candidate| {
            (
                candidate.keys.public_key.clone(),
                candidate.keys.pop.clone(),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let load = || {
        actual::Root::from_toml_source(
            TomlSource::from_file(&spec.config_path).expect("read generated peer config"),
        )
        .expect("parse generated peer config")
    };

    let mut redirected_bind = load();
    *redirected_bind.network.address.value_mut() =
        "127.0.0.1:65003".parse().expect("redirected bind");
    let error = managed_paths::validate_candidate_peer_topology(
        &redirected_bind,
        spec,
        &specs,
        &expected_roster,
    )
    .expect_err("redirected parsed bind address must fail validation");
    assert!(error.to_string().contains("network.address"));

    let mut redirected_public = load();
    *redirected_public.network.public_address.value_mut() = "127.0.0.1:65004"
        .parse()
        .expect("redirected public address");
    let error = managed_paths::validate_candidate_peer_topology(
        &redirected_public,
        spec,
        &specs,
        &expected_roster,
    )
    .expect_err("redirected parsed public address must fail validation");
    assert!(error.to_string().contains("network.public_address"));
}

#[test]
fn overlay_rejects_trusted_peer_address_substitution() {
    if !ports_available("overlay_rejects_trusted_peer_address_substitution") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _kagami = KagamiStub::install(temp.path());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");
    let selected_generation = supervisor.generation_id().to_owned();
    let mut trusted_peers = peer_config_table(&supervisor.peers()[0])
        .remove("trusted_peers")
        .and_then(|value| value.as_array().cloned())
        .expect("trusted_peers array");
    let substituted_key = supervisor.peers()[1].peer_id().to_string();
    let entry = trusted_peers
        .iter_mut()
        .find(|entry| {
            entry
                .as_str()
                .is_some_and(|value| value.starts_with(&substituted_key))
        })
        .expect("peer1 trusted entry");
    *entry = toml::Value::String(format!("{substituted_key}@127.0.0.1:65002"));
    let mut overlay = toml::Table::new();
    overlay.insert("trusted_peers".into(), toml::Value::Array(trusted_peers));

    let error = supervisor
        .restart_peer_with_extra_layers("peer0", &[overlay])
        .expect_err("trusted peer address substitution must fail before publication");
    assert!(
        matches!(
            error,
            SupervisorError::GenerationValidation(ref message)
                if message.contains("trusted PeerId/address topology")
        ),
        "unexpected trusted topology error: {error:?}"
    );
    assert_eq!(supervisor.generation_id(), selected_generation);
}
