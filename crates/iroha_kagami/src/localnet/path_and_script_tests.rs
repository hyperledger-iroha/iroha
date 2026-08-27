#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the test validates one multi-file localnet path-normalization contract end to end"
)]
fn relative_out_dir_paths_are_absolute_in_configs() {
    struct DirGuard {
        prev: PathBuf,
    }

    impl Drop for DirGuard {
        fn drop(&mut self) {
            env::set_current_dir(&self.prev).expect("restore current dir");
        }
    }

    let base = tempfile::tempdir().expect("tmp dir");
    let previous = env::current_dir().expect("current dir");
    env::set_current_dir(base.path()).expect("chdir into temp");
    let _guard = DirGuard { prev: previous };

    let opts = LocalnetOptions {
        sora_profile: None,
        perf_profile: None,
        peers: NonZeroU16::new(4).unwrap(),
        seed: Some("absolute-paths".to_owned()),
        bind_host: DEFAULT_BIND_HOST.to_owned(),
        public_host: DEFAULT_PUBLIC_HOST.to_owned(),
        base_api_port: 19081,
        base_p2p_port: 23338,
        out_dir: PathBuf::from("localnet"),
        extra_accounts: 0,
        assets: Vec::new(),
        block_cadence_ms: None,
        consensus_mode: SumeragiConsensusMode::Npos,
    };

    generate_localnet(&opts, &mut BufWriter::new(Vec::new()))
        .expect("generate localnet with relative path");

    let out_dir = fs::canonicalize(base.path().join("localnet"))
        .expect("canonical generated localnet output directory");
    let peer_cfg = fs::read_to_string(out_dir.join("peer0.toml")).expect("read peer config");
    let parsed: toml::Value = toml::from_str(&peer_cfg).expect("parse peer config");
    let genesis_path = parsed
        .get("genesis")
        .and_then(toml::Value::as_table)
        .and_then(|t| t.get("file"))
        .and_then(toml::Value::as_str)
        .expect("genesis path");
    let kura_path = parsed
        .get("kura")
        .and_then(toml::Value::as_table)
        .and_then(|t| t.get("store_dir"))
        .and_then(toml::Value::as_str)
        .expect("kura store");
    let soracloud_runtime_path = parsed
        .get("soracloud_runtime")
        .and_then(toml::Value::as_table)
        .and_then(|t| t.get("state_dir"))
        .and_then(toml::Value::as_str)
        .expect("soracloud runtime state dir");
    let tiered_state = parsed
        .get("tiered_state")
        .and_then(toml::Value::as_table)
        .expect("tiered_state table");
    let tiered_root = tiered_state
        .get("cold_store_root")
        .and_then(toml::Value::as_str)
        .expect("tiered_state cold_store_root");
    let da_root = tiered_state
        .get("da_store_root")
        .and_then(toml::Value::as_str)
        .expect("tiered_state da_store_root");
    let rans_tables_path = parsed
        .get("streaming")
        .and_then(toml::Value::as_table)
        .and_then(|streaming| streaming.get("codec"))
        .and_then(toml::Value::as_table)
        .and_then(|codec| codec.get("rans_tables_path"))
        .and_then(toml::Value::as_str)
        .expect("streaming codec rANS tables path");
    assert!(
        Path::new(genesis_path).is_absolute(),
        "genesis path should be absolute"
    );
    assert!(
        Path::new(kura_path).is_absolute(),
        "kura store path should be absolute"
    );
    assert!(
        Path::new(soracloud_runtime_path).is_absolute(),
        "soracloud runtime state_dir should be absolute"
    );
    let peer_state_path = Path::new(kura_path)
        .parent()
        .and_then(Path::parent)
        .expect("Kura path lives below the localnet output root")
        .join("state")
        .join("peer0");
    assert!(
        Path::new(soracloud_runtime_path).starts_with(&peer_state_path)
            && !Path::new(soracloud_runtime_path).starts_with(Path::new(kura_path)),
        "soracloud runtime state_dir must remain outside the pristine Kura root"
    );
    assert!(
        Path::new(tiered_root).is_absolute(),
        "tiered_state cold_store_root should be absolute"
    );
    assert!(
        Path::new(da_root).is_absolute(),
        "tiered_state da_store_root should be absolute"
    );
    let expected_rans_tables_path =
        fs::canonicalize(out_dir.join(LOCALNET_RANS_TABLE_RELATIVE_PATH))
            .expect("canonical generated rANS table");
    assert!(
        Path::new(rans_tables_path).is_absolute(),
        "streaming codec rANS tables path should be absolute"
    );
    assert_eq!(
        Path::new(rans_tables_path),
        expected_rans_tables_path,
        "streaming codec must bind the rANS table emitted into its output directory"
    );
    assert!(
        Path::new(tiered_root).starts_with(&peer_state_path)
            && Path::new(da_root).starts_with(&peer_state_path)
            && !Path::new(tiered_root).starts_with(Path::new(kura_path))
            && !Path::new(da_root).starts_with(Path::new(kura_path)),
        "auxiliary state roots must remain outside the pristine Kura root"
    );
    for peer_index in 0..opts.peers.get() {
        let peer_config: toml::Value = toml::from_str(
            &fs::read_to_string(out_dir.join(format!("peer{peer_index}.toml")))
                .expect("read generated peer config"),
        )
        .expect("parse generated peer config");
        let expected_state = out_dir.join("state").join(format!("peer{peer_index}"));
        let streaming = peer_config
            .get("streaming")
            .and_then(toml::Value::as_table)
            .expect("streaming table");
        let session_store = streaming
            .get("session_store_dir")
            .and_then(toml::Value::as_str)
            .expect("streaming session store");
        let torii_data = peer_config
            .get("torii")
            .and_then(toml::Value::as_table)
            .and_then(|torii| torii.get("data_dir"))
            .and_then(toml::Value::as_str)
            .expect("Torii data directory");
        let torii_da = peer_config
            .get("torii")
            .and_then(toml::Value::as_table)
            .and_then(|torii| torii.get("da_ingest"))
            .and_then(toml::Value::as_table)
            .expect("Torii DA ingest table");
        let torii_da_replay = torii_da
            .get("replay_cache_store_dir")
            .and_then(toml::Value::as_str)
            .expect("Torii DA replay-cache directory");
        let torii_da_manifests = torii_da
            .get("manifest_store_dir")
            .and_then(toml::Value::as_str)
            .expect("Torii DA manifest directory");
        let sorafs_data = peer_config
            .get("sorafs")
            .and_then(toml::Value::as_table)
            .and_then(|sorafs| sorafs.get("storage"))
            .and_then(toml::Value::as_table)
            .and_then(|storage| storage.get("data_dir"))
            .and_then(toml::Value::as_str)
            .expect("SoraFS data directory");
        let sorafs_por_state = peer_config
            .get("sorafs")
            .and_then(toml::Value::as_table)
            .and_then(|sorafs| sorafs.get("por"))
            .and_then(toml::Value::as_table)
            .and_then(|por| por.get("state_dir"))
            .and_then(toml::Value::as_str)
            .expect("SoraFS PoR state directory");
        let soranet_ticket_revocations = peer_config
            .get("network")
            .and_then(toml::Value::as_table)
            .and_then(|network| network.get("soranet_handshake"))
            .and_then(toml::Value::as_table)
            .and_then(|handshake| handshake.get("pow"))
            .and_then(toml::Value::as_table)
            .and_then(|pow| pow.get("revocation_store_path"))
            .and_then(toml::Value::as_str)
            .expect("SoraNet ticket revocation store");
        assert_eq!(Path::new(session_store), expected_state.join("streaming"));
        assert_eq!(Path::new(torii_data), expected_state.join("torii"));
        assert_eq!(
            Path::new(torii_da_replay),
            expected_state.join("torii").join("da_replay")
        );
        assert_eq!(
            Path::new(torii_da_manifests),
            expected_state.join("torii").join("da_manifests")
        );
        assert_eq!(Path::new(sorafs_data), expected_state.join("sorafs"));
        assert_eq!(
            Path::new(sorafs_por_state),
            expected_state.join("sorafs").join("por")
        );
        assert_eq!(
            Path::new(soranet_ticket_revocations),
            expected_state
                .join("soranet")
                .join("ticket_revocations.norito")
        );
    }
    assert!(
        fs::read_dir(kura_path)
            .expect("read pristine Kura root")
            .next()
            .is_none(),
        "generated localnet must leave the Kura root pristine for catalog binding"
    );
}

#[cfg(unix)]
#[test]
#[allow(clippy::too_many_lines)]
fn start_and_stop_scripts_are_executable() {
    let temp = tempfile::tempdir().expect("tmp dir");
    let client_account_literal = localnet_client_account_literal(None);
    let fee_asset_definition_id = localnet_fee_asset_literal();
    write_scripts(
        temp.path(),
        1,
        false,
        false,
        &client_account_literal,
        &fee_asset_definition_id,
    )
    .expect("write scripts");

    let start_path = temp.path().join("start.sh");
    let stop_path = temp.path().join("stop.sh");
    let start_mode = fs::metadata(&start_path)
        .expect("start metadata")
        .permissions()
        .mode();
    let stop_mode = fs::metadata(&stop_path)
        .expect("stop metadata")
        .permissions()
        .mode();
    assert_ne!(
        start_mode & 0o111,
        0,
        "start script should be marked executable"
    );
    assert_ne!(
        stop_mode & 0o111,
        0,
        "stop script should be marked executable"
    );

    let start_contents = fs::read_to_string(&start_path).expect("read start script");
    assert_eq!(
        start_contents.lines().take(3).collect::<Vec<_>>(),
        ["#!/usr/bin/env bash", "set -euo pipefail", "umask 077"],
        "generated startup must keep logs, pidfiles, and runtime directories owner-only",
    );
    let (debug_path, release_path) = default_irohad_bin_paths(false);
    let expected_debug = format!(
        "DEFAULT_IROHAD_BIN_DEBUG={}",
        shell_quote_path(&debug_path).expect("quote debug path")
    );
    let expected_release = format!(
        "DEFAULT_IROHAD_BIN_RELEASE={}",
        shell_quote_path(&release_path).expect("quote release path")
    );
    assert!(
        start_contents.lines().any(|line| line == expected_debug),
        "start script should set debug default"
    );
    assert!(
        start_contents.lines().any(|line| line == expected_release),
        "start script should set release default"
    );
    assert!(
        start_contents.contains("if [ -x \"$DEFAULT_IROHAD_BIN_DEBUG\" ]; then"),
        "start script should prefer the debug iroha3d for local contract development"
    );
    assert!(
        start_contents.contains("elif [ -x \"$DEFAULT_IROHAD_BIN_RELEASE\" ]; then"),
        "start script should fall back to the release iroha3d when no debug binary exists"
    );
    assert!(
        start_contents.contains("DEFAULT_IROHA_CLI_RELEASE="),
        "start script should also wire the iroha CLI defaults"
    );
    assert!(
        start_contents.contains("FAUCET_RESERVE_TARGET="),
        "start script should declare a faucet reserve target"
    );
    assert!(
        start_contents.contains("FAUCET_RESERVE_RETRIES="),
        "start script should make faucet reserve retries configurable"
    );
    assert!(
        start_contents.contains("ledger asset mint --definition \"$FAUCET_ASSET_DEFINITION_ID\""),
        "start script should mint the fee asset back to the faucet when reserve is low"
    );
    assert!(
        start_contents.contains("--fee-payer authority --output-format json"),
        "start script should explicitly select authority-paid typed fees for faucet reserve top-ups"
    );
    assert!(!start_contents.contains("faucet-topup.metadata.json"));
    assert!(!start_contents.contains("gas_asset_id"));
    assert!(
        start_contents.contains("start_new_session=True"),
        "start script should detach peers into a new session when python3 is available"
    );
    assert!(
        start_contents.contains("nohup env SNAPSHOT_STORE_DIR="),
        "start script should keep a nohup fallback for minimal shells"
    );
    assert!(
        start_contents.contains("SNAPSHOT_STORE_DIR=\"$DIR/state/peer${i}/snapshot\""),
        "snapshot state must remain outside the pristine Kura root"
    );
    assert!(
        start_contents.contains("mkdir -p \"$SNAPSHOT_STORE_DIR/generations\""),
        "start script should create the snapshot generations directory"
    );
    assert!(
        start_contents.contains("peer$i already running with pid $existing_pid"),
        "start script should refuse to overwrite live pidfiles"
    );
    assert!(
        start_contents.contains("pid_is_running()")
            && start_contents.contains("pid_is_running \"$existing_pid\""),
        "start script should probe pid liveness without null signals"
    );
    assert!(
        start_contents.contains("command -v ps >/dev/null 2>&1 || return 0"),
        "start script should treat missing ps as live rather than stale"
    );
    assert!(
        !start_contents.contains("kill -0"),
        "start script should not use null-signal pid probes"
    );
    assert!(
        start_contents.contains("rm -f \"$PIDFILE\""),
        "start script should clear stale pidfiles before relaunch"
    );
    let stop_contents = fs::read_to_string(&stop_path).expect("read stop script");
    assert_eq!(
        stop_contents.lines().take(3).collect::<Vec<_>>(),
        ["#!/usr/bin/env bash", "set -euo pipefail", "umask 077"],
        "generated shutdown must preserve owner-only runtime custody",
    );
    assert!(
        stop_contents.contains("pid_matches_peer()"),
        "stop script should validate pid ownership before signaling"
    );
    assert!(
        stop_contents.contains("pid_is_running()"),
        "stop script should probe pid liveness without null signals"
    );
    assert!(
        stop_contents.contains("command -v ps >/dev/null 2>&1 || return 0"),
        "stop script should treat missing ps as live rather than stale"
    );
    assert!(
        !stop_contents.contains("kill -0"),
        "stop script should not use null-signal pid probes"
    );
    assert!(
        stop_contents.contains("grep -F -- \"--config $config\""),
        "stop script should bind live pid checks to the peer config path"
    );
    assert!(
        stop_contents.contains("live pid $pid does not match $config"),
        "stop script should leave reused pidfiles untouched"
    );
    assert!(
        !stop_contents.contains("kill -9 \"$pid\""),
        "stop script should not escalate to SIGKILL"
    );
    assert!(
        stop_contents.contains("localnet peer $peer_name pid $pid is still running"),
        "stop script should leave still-running owned peers visible"
    );
    assert!(
        stop_contents.contains("rm -f \"$pidfile\""),
        "stop script should clean pidfiles after shutdown"
    );
}

#[cfg(unix)]
#[test]
fn shell_assignment_quoting_preserves_metacharacters_as_data() {
    let input = "target dir/it's-$(printf injected)-`printf other`";
    let quoted = shell_single_quote(input).expect("quote shell value");
    let command = format!("value={quoted}; printf '%s' \"$value\"");
    let output = std::process::Command::new("bash")
        .arg("-c")
        .arg(command)
        .output()
        .expect("run generated assignment");
    assert!(output.status.success());
    assert_eq!(output.stdout, input.as_bytes());
    assert!(shell_single_quote("line one\nline two").is_err());
}

#[cfg(unix)]
#[test]
fn start_script_skips_zero_faucet_retries_before_seq() {
    let temp = tempfile::tempdir().expect("tmp dir");
    write_scripts(
        temp.path(),
        1,
        false,
        false,
        &localnet_client_account_literal(None),
        &localnet_fee_asset_literal(),
    )
    .expect("write scripts");
    let start = fs::read_to_string(temp.path().join("start.sh")).expect("read start script");
    let zero_retry_guard = start
        .find("[ \"$FAUCET_RESERVE_RETRIES\" != \"0\" ] ||")
        .expect("explicit zero-retry guard");
    let reserve_loop = start
        .find("for _ in $(seq 1 \"$FAUCET_RESERVE_RETRIES\"); do")
        .expect("faucet reserve retry loop");
    assert!(
        zero_retry_guard < reserve_loop,
        "zero retries must return before invoking platform-dependent seq"
    );
}
