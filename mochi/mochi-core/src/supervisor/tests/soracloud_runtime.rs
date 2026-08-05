#[test]
fn peer_specs_write_distinct_absolute_managed_soracloud_state_roots() {
    let temp = tempfile::tempdir().expect("temp dir");
    let profile = NetworkProfile::from_preset(ProfilePreset::FourPeerBft);
    let paths = NetworkPaths::from_root(temp.path(), &profile);
    paths.ensure().expect("paths");
    let specs = (0_u16..4)
        .map(|index| {
            test_peer_spec(&paths, format!("peer{index}"), 8_080 + index, 1_337 + index)
                .expect("peer spec")
        })
        .collect::<Vec<_>>();
    let genesis = test_genesis_material(&paths);

    let mut configured_roots = HashSet::new();
    for spec in &specs {
        spec.write_config(
            "demo-chain",
            &genesis,
            &specs,
            &PeerConfigOverrides::default(),
            &[],
        )
        .expect("write config");
        let contents = fs::read_to_string(&spec.config_path).expect("read config");
        let value: toml::Table = toml::from_str(&contents).expect("parse config");
        let configured = value
            .get("soracloud_runtime")
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get("state_dir"))
            .and_then(toml::Value::as_str)
            .expect("managed Soracloud runtime state directory");
        let expected = spec
            .storage_dir
            .canonicalize()
            .expect("canonical storage root")
            .join("soracloud_runtime");
        assert!(Path::new(configured).is_absolute());
        assert_eq!(Path::new(configured), expected);
        assert!(
            configured_roots.insert(configured.to_owned()),
            "each peer must own a distinct Soracloud runtime state root"
        );
    }
    assert_eq!(configured_roots.len(), specs.len());
}

#[test]
fn peer_spec_preserves_managed_soracloud_state_root_across_overlay() {
    let temp = tempfile::tempdir().expect("temp dir");
    let paths = NetworkPaths::from_root(temp.path(), &NetworkProfile::default());
    paths.ensure().expect("paths");
    let spec = test_peer_spec(&paths, "peer0".into(), 8080, 1337).expect("peer spec");
    let genesis = test_genesis_material(&paths);
    let mut runtime = toml::Table::new();
    runtime.insert("reconcile_interval_ms".into(), toml::Value::Integer(3_210));
    let mut overlay = toml::Table::new();
    overlay.insert("soracloud_runtime".into(), toml::Value::Table(runtime));

    spec.write_config(
        "demo-chain",
        &genesis,
        std::slice::from_ref(&spec),
        &PeerConfigOverrides::default(),
        &[overlay],
    )
    .expect("write config");

    let contents = fs::read_to_string(&spec.config_path).expect("read config");
    let value: toml::Table = toml::from_str(&contents).expect("parse config");
    let runtime = value
        .get("soracloud_runtime")
        .and_then(toml::Value::as_table)
        .expect("Soracloud runtime config");
    assert_eq!(
        runtime
            .get("reconcile_interval_ms")
            .and_then(toml::Value::as_integer),
        Some(3_210)
    );
    let expected = spec
        .storage_dir
        .canonicalize()
        .expect("canonical storage root")
        .join("soracloud_runtime");
    assert_eq!(
        runtime.get("state_dir").and_then(toml::Value::as_str),
        Some(expected.to_string_lossy().as_ref())
    );
}

#[test]
fn peer_spec_rejects_soracloud_state_root_override() {
    let temp = tempfile::tempdir().expect("temp dir");
    let paths = NetworkPaths::from_root(temp.path(), &NetworkProfile::default());
    paths.ensure().expect("paths");
    let spec = test_peer_spec(&paths, "peer0".into(), 8080, 1337).expect("peer spec");
    let genesis = test_genesis_material(&paths);
    let mut runtime = toml::Table::new();
    runtime.insert(
        "state_dir".into(),
        toml::Value::String("/tmp/shared-soracloud-runtime".to_owned()),
    );
    let mut overlay = toml::Table::new();
    overlay.insert("soracloud_runtime".into(), toml::Value::Table(runtime));

    let error = spec
        .write_config(
            "demo-chain",
            &genesis,
            std::slice::from_ref(&spec),
            &PeerConfigOverrides::default(),
            &[overlay],
        )
        .expect_err("Soracloud runtime state root override must fail closed");
    match error {
        SupervisorError::Config(message) => assert!(
            message.contains("must preserve Mochi's managed Soracloud runtime state root")
                && message.contains(spec.storage_dir.to_string_lossy().as_ref()),
            "unexpected error: {message}"
        ),
        other => panic!("expected SupervisorError::Config, got {other:?}"),
    }
}

#[test]
fn managed_peer_path_validation_rejects_soracloud_state_root_redirect() {
    let temp = tempfile::tempdir().expect("temp dir");
    let paths = NetworkPaths::from_root(temp.path(), &NetworkProfile::default());
    paths.ensure().expect("paths");
    let spec = test_peer_spec(&paths, "peer0".into(), 8080, 1337).expect("peer spec");
    let genesis = test_genesis_material(&paths);
    spec.write_config(
        "demo-chain",
        &genesis,
        std::slice::from_ref(&spec),
        &PeerConfigOverrides::default(),
        &[],
    )
    .expect("write config");
    let mut config = actual::Root::from_toml_source(
        TomlSource::from_file(&spec.config_path).expect("read generated peer config"),
    )
    .expect("parse generated peer config");
    config.soracloud_runtime.state_dir = temp.path().join("redirected-soracloud-runtime");

    let error = validate_managed_peer_paths(&config, &spec, 1)
        .expect_err("redirected Soracloud runtime state root must fail validation");
    assert!(error.to_string().contains("soracloud_runtime.state_dir"));
}

#[cfg(unix)]
#[test]
fn start_rejects_managed_soracloud_runtime_symlink_before_spawn() {
    if !ports_available("start_rejects_managed_soracloud_runtime_symlink_before_spawn") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _kagami = KagamiStub::install(temp.path());
    let irohad = write_long_running_irohad_stub(temp.path());
    let _irohad = EnvVarGuard::set("MOCHI_IROHAD", irohad.as_os_str());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");
    let selected_generation = supervisor.generation_id().to_owned();
    let attacker = temp.path().join("attacker-soracloud-runtime");
    fs::create_dir(&attacker).expect("create attacker directory");
    let sentinel = attacker.join("sentinel");
    fs::write(&sentinel, b"must-not-touch").expect("write attacker sentinel");
    symlink(
        &attacker,
        supervisor.peers()[0]
            .storage_dir()
            .join("soracloud_runtime"),
    )
    .expect("redirect managed Soracloud runtime directory");

    let error = supervisor
        .start_all()
        .expect_err("managed Soracloud runtime symlink must fail closed");
    assert!(matches!(error, SupervisorError::GenerationValidation(_)));
    assert_eq!(supervisor.generation_id(), selected_generation);
    assert!(!supervisor.is_any_running(), "no peer may be spawned");
    assert_eq!(
        fs::read(sentinel).expect("read sentinel"),
        b"must-not-touch"
    );
}

#[cfg(unix)]
#[test]
fn launched_soracloud_writer_leaves_generation_inventory_immutable() {
    if !ports_available("launched_soracloud_writer_leaves_generation_inventory_immutable") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _kagami = KagamiStub::install(temp.path());
    let irohad = temp.path().join("irohad-soracloud-writer-stub.sh");
    let script = r#"#!/bin/sh
case "$1" in
  --version)
    echo "irohad-soracloud-writer-stub iroha3"
    exit 0
    ;;
esac
config=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --config)
      config="$2"
      shift 2
      ;;
    *)
      shift
      ;;
  esac
done
state_dir=$(awk '
  $0 == "[soracloud_runtime]" { in_runtime = 1; next }
  in_runtime && substr($0, 1, 1) == "[" { exit }
  in_runtime && $1 == "state_dir" && $2 == "=" {
    value = $0
    sub(/^[^=]*= "/, "", value)
    sub(/"$/, "", value)
    print value
    exit
  }
' "$config")
[ "$state_dir" = "$MOCHI_TEST_SORACLOUD_STATE_DIR" ] || exit 71
mkdir -p "$state_dir" || exit 72
printf '{"schema":1}\n' > "$state_dir/runtime_snapshot.json" || exit 73
"#;
    fs::write(&irohad, script).expect("write Soracloud writer stub");
    let mut permissions = fs::metadata(&irohad).expect("stub metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&irohad, permissions).expect("set stub permissions");
    let _irohad_guard = EnvVarGuard::set("MOCHI_IROHAD", irohad.as_os_str());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");
    let expected_state_dir = supervisor.peers()[0]
        .storage_dir()
        .canonicalize()
        .expect("canonical storage root")
        .join("soracloud_runtime");
    let _state_guard = EnvVarGuard::set(
        "MOCHI_TEST_SORACLOUD_STATE_DIR",
        expected_state_dir.as_os_str(),
    );

    supervisor.start_peer("peer0").expect("start peer writer");
    let status = supervisor.peers[0]
        .process
        .as_mut()
        .expect("running peer writer")
        .wait()
        .expect("wait for peer writer");
    assert!(
        status.success(),
        "Soracloud writer stub must exit successfully"
    );
    assert_eq!(
        fs::read(expected_state_dir.join("runtime_snapshot.json"))
            .expect("read mutable Soracloud snapshot"),
        b"{\"schema\":1}\n"
    );
    let immutable_default = supervisor.peers()[0]
        .config_path()
        .parent()
        .expect("generation peer directory")
        .join("storage/soracloud_runtime/runtime_snapshot.json");
    assert!(
        !immutable_default.exists(),
        "runtime state must not be written below the inventoried generation"
    );
    supervisor
        .session_info()
        .expect("runtime writes must leave the selected generation inventory valid");
}
