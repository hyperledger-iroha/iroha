#[test]
fn private_dataspace_peer_configs_use_direct_fee_settlement() {
    let temp = tempfile::tempdir().expect("tmp dir");
    for (profile, label, base_port) in [
        (SoraProfile::PrivateSbp, "sbp", 28_080_u16),
        (SoraProfile::PrivateCbuae, "cbuae", 29_080_u16),
    ] {
        let out_dir = temp.path().join(label);
        let opts = LocalnetOptions {
            sora_profile: Some(profile),
            perf_profile: None,
            peers: NonZeroU16::new(4).expect("non-zero"),
            seed: Some(format!("private-direct-settlement-{label}")),
            bind_host: DEFAULT_BIND_HOST.to_owned(),
            public_host: DEFAULT_PUBLIC_HOST.to_owned(),
            base_api_port: base_port,
            base_p2p_port: base_port.saturating_add(257),
            out_dir: out_dir.clone(),
            extra_accounts: 0,
            assets: Vec::new(),
            block_cadence_ms: None,
            consensus_mode: SumeragiConsensusMode::Npos,
        };
        generate_localnet(&opts, &mut BufWriter::new(Vec::new()))
            .unwrap_or_else(|error| panic!("generate {label} localnet: {error:#}"));
        for peer in 0..4 {
            let peer_cfg: toml::Value = toml::from_str(
                &fs::read_to_string(out_dir.join(format!("peer{peer}.toml")))
                    .expect("read generated peer config"),
            )
            .expect("parse peer config");
            let fees = peer_cfg
                .get("nexus")
                .and_then(toml::Value::as_table)
                .and_then(|nexus| nexus.get("fees"))
                .and_then(toml::Value::as_table)
                .expect("nexus fees table");
            assert_eq!(
                fees.get("per_gas_unit_fee").and_then(toml::Value::as_str),
                Some("0.00005")
            );
            assert_eq!(
                fees.get("settlement_mode").and_then(toml::Value::as_str),
                Some("direct"),
                "private {label} peer {peer} must use genesis-compatible direct fee settlement"
            );
        }
    }
}
#[test]
fn account_id_raw_string_parses_as_account_id() {
    let seed_bytes = Some(b"localnet-gas-parse".as_slice());
    let (genesis_public_key, _) = generate_genesis_key_pair(seed_bytes, GENESIS_SEED)
        .expect("test localnet genesis key generation should succeed");
    let gas_account_id = localnet_gas_account_id(&genesis_public_key)
        .expect("test localnet gas account derivation should succeed");
    let encoded = account_id_raw_string(&gas_account_id);
    let parsed = AccountId::parse_encoded(&encoded).expect("account id parse");
    assert_eq!(parsed, gas_account_id);
}
