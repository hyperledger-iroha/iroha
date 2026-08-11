struct EnvVarGuard {
    key: &'static str,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: &std::ffi::OsStr) -> Self {
        // SAFETY: tests serialize environment mutation within a single thread.
        unsafe {
            env::set_var(key, value);
        }
        Self { key }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        // SAFETY: matching set_var executed in a controlled test environment.
        unsafe {
            env::remove_var(self.key);
        }
    }
}

struct RestoringEnvVarGuard {
    key: &'static str,
    previous: Option<OsString>,
}

impl RestoringEnvVarGuard {
    fn set(key: &'static str, value: &std::ffi::OsStr) -> Self {
        let previous = env::var_os(key);
        // SAFETY: tests serialize environment mutation within a single thread.
        unsafe {
            env::set_var(key, value);
        }
        Self { key, previous }
    }
}

impl Drop for RestoringEnvVarGuard {
    fn drop(&mut self) {
        // SAFETY: matching set_var executed in a controlled test environment.
        unsafe {
            if let Some(previous) = self.previous.take() {
                env::set_var(self.key, previous);
            } else {
                env::remove_var(self.key);
            }
        }
    }
}

fn write_version_stub(root: &Path, name: &str, build_line: &str) -> PathBuf {
    let script_path = root.join(format!("{name}.sh"));
    let script = format!(
        r#"#!/bin/sh
case "$1" in
  --version)
    echo "{name} {build_line}"
    exit 0
    ;;
  *)
    exit 0
    ;;
esac
"#
    );
    fs::write(&script_path, script).expect("write version stub");
    #[cfg(unix)]
    {
        let mut perms = fs::metadata(&script_path)
            .expect("version stub metadata")
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).expect("set version stub permissions");
    }
    script_path
}

#[cfg(unix)]
fn write_long_running_irohad_stub(root: &Path) -> PathBuf {
    let script_path = root.join("iroha3d-long-running-stub.sh");
    let script = r#"#!/bin/sh
case "$1" in
  --version)
    echo "iroha3d-long-running-stub iroha3"
    exit 0
    ;;
esac
exec /usr/bin/tail -f /dev/null
"#;
    fs::write(&script_path, script).expect("write long-running iroha3d stub");
    let mut permissions = fs::metadata(&script_path)
        .expect("long-running iroha3d stub metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script_path, permissions)
        .expect("set long-running iroha3d stub permissions");
    script_path
}

#[cfg(unix)]
fn redirect_storage_generations_through_symlink(
    peer: &PeerHandle,
    attacker_root: &Path,
) -> PathBuf {
    let storage_generation = peer
        .storage_dir()
        .file_name()
        .expect("storage generation id")
        .to_owned();
    let storage_generations = peer
        .storage_dir()
        .parent()
        .expect("storage-generations directory");
    let original = storage_generations.with_file_name("storage-generations-original");
    fs::rename(storage_generations, &original).expect("move genuine storage hierarchy");

    let redirected_storage = attacker_root.join(storage_generation);
    fs::create_dir_all(
        redirected_storage
            .join("snapshot")
            .join(SNAPSHOT_GENERATIONS_DIR_NAME),
    )
    .expect("create redirected hierarchy");
    let sentinel = redirected_storage.join("attacker-sentinel");
    fs::write(&sentinel, b"must-not-touch").expect("write attacker sentinel");
    symlink(attacker_root, storage_generations).expect("redirect storage-generations");
    sentinel
}

#[cfg(unix)]
fn write_cargo_build_stub(root: &Path) -> (PathBuf, PathBuf) {
    let stub_path = root.join("cargo_stub.sh");
    let log_path = root.join("cargo_stub.log");
    let script = r#"#!/bin/sh
set -eu
LOG_FILE="${MOCHI_TEST_CARGO_LOG:-}"
if [ -n "$LOG_FILE" ]; then
  printf '%s\n' "$*" >> "$LOG_FILE"
fi
TARGET="${CARGO_TARGET_DIR:-target}"
/bin/mkdir -p "$TARGET/debug"
case "$*" in
  *"-p irohad --bin iroha3d"*)
    BIN="$TARGET/debug/iroha3d"
    ;;
  *"-p iroha_kagami"*)
    BIN="$TARGET/debug/kagami"
    ;;
  *"-p iroha_cli"*)
    BIN="$TARGET/debug/iroha3"
    ;;
  *)
    BIN="$TARGET/debug/unknown"
    ;;
esac
/bin/cat > "$BIN" <<'EOF'
#!/bin/sh
if [ "$1" = "--version" ]; then
  bin="${0##*/}"
  echo "$bin iroha3"
  exit 0
fi
exit 0
EOF
/bin/chmod 755 "$BIN"
exit 0
"#
    .to_string();
    fs::write(&stub_path, script).expect("write cargo stub");
    let mut perms = fs::metadata(&stub_path)
        .expect("cargo stub metadata")
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&stub_path, perms).expect("set cargo stub permissions");
    let _ = fs::File::create(&log_path);
    (stub_path, log_path)
}

#[cfg(unix)]
fn write_cargo_failure_stub(root: &Path) -> PathBuf {
    let stub_path = root.join("cargo_fail_stub.sh");
    let script = r#"#!/bin/sh
exit 1
"#;
    fs::write(&stub_path, script).expect("write cargo failure stub");
    let mut perms = fs::metadata(&stub_path)
        .expect("cargo failure stub metadata")
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&stub_path, perms).expect("set cargo failure stub permissions");
    stub_path
}

#[test]
fn snapshot_label_sanitization_behaves() {
    assert_eq!(
        sanitize_snapshot_label("My First Snapshot!").as_deref(),
        Some("my-first-snapshot")
    );
    assert_eq!(
        sanitize_snapshot_label("weird__Label!!").as_deref(),
        Some("weird-label")
    );
    assert!(sanitize_snapshot_label("%%%").is_none());
}

#[test]
fn snapshot_label_collapses_separators_and_clamps_length() {
    let label = "___  noisy--Label...with__mixed---separators   ";
    assert_eq!(
        sanitize_snapshot_label(label).as_deref(),
        Some("noisy-label-with-mixed-separators")
    );

    let long_label = "PREFIX".repeat(20);
    let sanitized = sanitize_snapshot_label(&long_label).expect("sanitized output");
    assert!(
        sanitized.len() <= SNAPSHOT_LABEL_MAX_LEN,
        "sanitized label should be clamped"
    );
    assert!(
        sanitized.starts_with("prefixprefixprefix"),
        "sanitized label should keep leading alphas in lower-case"
    );
}

#[test]
fn copy_dir_recursive_handles_missing_sources() {
    let temp = tempfile::tempdir().expect("tempdir");
    let missing = temp.path().join("missing");
    let dest = temp.path().join("out");
    copy_dir_recursive(&missing, &dest).expect("copy missing source");
    assert!(dest.exists(), "destination directory should be created");
    let mut iter = fs::read_dir(&dest).expect("read destination dir");
    assert!(iter.next().is_none(), "destination should remain empty");
}

#[test]
fn probe_version_output_parses_and_infers_build_line() {
    let temp = tempfile::tempdir().expect("tempdir");
    let script_path = temp.path().join("custom-bin.sh");
    let script = r#"#!/bin/sh
echo "custom-bin iroha3 3.2.1"
exit 0
"#;
    fs::write(&script_path, script).expect("write version script");
    #[cfg(unix)]
    {
        let mut perms = fs::metadata(&script_path)
            .expect("version script metadata")
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).expect("set version script perms");
    }

    let (raw, version) =
        probe_version_output(&script_path, "custom-bin").expect("version probe succeeds");
    assert_eq!(version.as_deref(), Some("custom-bin iroha3 3.2.1"));
    let build_line = infer_build_line("custom-bin", &script_path, raw.as_deref());
    assert_eq!(build_line, BuildLine::Iroha3);
}

#[test]
fn probe_version_output_honors_iroha2_output() {
    let temp = tempfile::tempdir().expect("tempdir");
    let script_path = temp.path().join("weird-name.sh");
    let script = r#"#!/bin/sh
echo "my-custom iroha2 build"
exit 0
"#;
    fs::write(&script_path, script).expect("write version script");
    #[cfg(unix)]
    {
        let mut perms = fs::metadata(&script_path)
            .expect("version script metadata")
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).expect("set version script perms");
    }

    let (raw, _) =
        probe_version_output(&script_path, "weird-name").expect("version probe succeeds");
    let build_line = infer_build_line("weird-name", &script_path, raw.as_deref());
    assert_eq!(build_line, BuildLine::Iroha2);
}

#[test]
fn compatibility_summary_includes_profile_and_fingerprint() {
    let report = CompatibilityReport {
        versions: Vec::new(),
        verify: Some(KagamiVerifyReport {
            profile: GenesisProfile::Iroha3Dev,
            chain_id: Some("test-chain".to_owned()),
            vrf_seed_hex: None,
            peers_with_pop: None,
            fingerprint: Some("fp123".to_owned()),
            raw_output: "fingerprint fp123".to_owned(),
        }),
        chain_id: "test-chain".to_owned(),
        profile: Some(GenesisProfile::Iroha3Dev),
    };

    let summary = report.summary_line();
    assert!(
        summary.contains("chain test-chain"),
        "summary should include chain id: {summary}"
    );
    assert!(
        summary.contains("profile iroha3-dev"),
        "summary should include profile slug: {summary}"
    );
    assert!(
        summary.contains("fingerprint fp123"),
        "summary should include verify fingerprint: {summary}"
    );
}

struct KagamiStub {
    _path_guard: EnvVarGuard,
    _log_guard: EnvVarGuard,
    _irohad_guard: EnvVarGuard,
    _iroha_cli_guard: EnvVarGuard,
    _signature_guard: EnvVarGuard,
    log_path: PathBuf,
}

impl KagamiStub {
    fn install(root: &Path) -> Self {
        let script_path = root.join("kagami_stub.sh");
        let chain_discriminant = iroha_data_model::account::address::chain_discriminant();
        let manifest = format!(
            "{{\"chain\":\"00000000-0000-0000-0000-000000000000\",\"chain_discriminant\":{chain_discriminant},\"ivm_dir\":\".\",\"consensus_mode\":\"Permissioned\",\"wire_protocol_version\":4,\"sumeragi_v2\":{{\"da_layout\":{{\"encoding\":{{\"encoding\":\"reed_solomon16\",\"details\":null}},\"chunk_size_bytes\":262144,\"data_shards\":4,\"parity_shards\":2,\"max_payload_size_bytes\":16777216,\"max_chunk_count\":1024}},\"nexus_amx_context_hash\":\"6611CDC66348BEBFBD583F888864A747DCC828C5FE84F58DFB0346CCA27ABAF3\",\"execution_policy_hash\":\"3F947453758F8EE90B2C66437A128FC22D93C4D2E0CA60C261D828B7E0B897C3\"}},\"transactions\":[{{\"instructions\":[]}}]}}"
        );
        let script = format!(
            r#"#!/bin/sh
if [ -n "$MOCHI_KAGAMI_LOG" ]; then
  printf 'args:%s\n' "$*" >> "$MOCHI_KAGAMI_LOG"
fi
case "$1" in
  --version)
    echo "kagami-stub iroha3"
    exit 0
    ;;
  verify)
    exit 0
    ;;
  genesis)
    case "$2" in
      generate)
        cat <<'JSON'
{manifest}
JSON
        exit 0
        ;;
      sign)
        if [ "$MOCHI_KAGAMI_FAIL_SIGN" = "1" ]; then
          echo "requested kagami sign failure" >&2
          exit 23
        fi
        manifest_path="$3"
        shift 3
        while [ "$#" -gt 0 ]; do
          case "$1" in
            --out-file)
              out_file="$2"
              shift 2
              ;;
            --bound-manifest-out)
              bound_manifest_out="$2"
              shift 2
              ;;
            --expected-hash-out)
              expected_hash_out="$2"
              shift 2
              ;;
            --private-key-file)
              private_key_file="$2"
              shift 2
              ;;
            --config)
              config_file="$2"
              shift 2
              ;;
            *)
              shift
              ;;
          esac
        done
        test -s "$private_key_file"
        test -s "$config_file"
        config_mode="$(stat -f %Lp "$config_file" 2>/dev/null || stat -c %a "$config_file")"
        test "$config_mode" = "600"
        grep -F 'expected_hash = "REPLACE_WITH_GENESIS_EXPECTED_HASH"' "$config_file" >/dev/null
        printf 'stub-signed-genesis' > "$out_file"
        printf '0000000000000000000000000000000000000000000000000000000000000001\n' > "$expected_hash_out"
        if [ "$bound_manifest_out" != "$manifest_path" ]; then
          cp "$manifest_path" "$bound_manifest_out"
        fi
        exit 0
        ;;
      *)
        echo "unsupported kagami genesis command: $2" >&2
        exit 1
        ;;
    esac
    ;;
  *)
    echo "unsupported kagami stub command: $1" >&2
    exit 1
    ;;
esac
"#
        );
        fs::write(&script_path, script).expect("write kagami stub");
        #[cfg(unix)]
        {
            let mut perms = fs::metadata(&script_path)
                .expect("script metadata")
                .permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script_path, perms).expect("set script perms");
        }
        let log_path = root.join("kagami_stub.log");
        let iroha_stub = write_version_stub(root, "iroha-stub", "iroha3");
        let path_guard = EnvVarGuard::set("MOCHI_KAGAMI", script_path.as_os_str());
        let log_guard = EnvVarGuard::set("MOCHI_KAGAMI_LOG", log_path.as_os_str());
        let irohad_guard = EnvVarGuard::set("MOCHI_IROHAD", iroha_stub.as_os_str());
        let iroha_cli_guard = EnvVarGuard::set("MOCHI_IROHA_CLI", iroha_stub.as_os_str());
        let signature_guard = EnvVarGuard::set(
            TEST_FINALIZE_KAGAMI_STUB_SIGNATURE,
            std::ffi::OsStr::new("1"),
        );
        let _ = fs::File::create(&log_path);
        Self {
            _path_guard: path_guard,
            _log_guard: log_guard,
            _irohad_guard: irohad_guard,
            _iroha_cli_guard: iroha_cli_guard,
            _signature_guard: signature_guard,
            log_path,
        }
    }

    fn log_path(&self) -> &Path {
        &self.log_path
    }
}

struct StandaloneKagamiStub {
    script_path: PathBuf,
    log_path: PathBuf,
    _irohad_guard: EnvVarGuard,
    _iroha_cli_guard: EnvVarGuard,
}

impl StandaloneKagamiStub {
    fn create(root: &Path) -> Self {
        let script_path = root.join("kagami_override.sh");
        let log_path = root.join("kagami_override.log");
        let chain_discriminant = iroha_data_model::account::address::chain_discriminant();
        let script = format!(
            r#"#!/bin/sh
set -e
SCRIPT_DIR="$(CDPATH= cd -- "$(dirname "$0")" && pwd)"
printf '%s\n' "$@" >> "$SCRIPT_DIR/kagami_override.log"
case "$1" in
  --version)
    echo "kagami-override iroha3"
    exit 0
    ;;
  verify)
    exit 0
    ;;
  genesis)
    case "$2" in
      generate)
        cat <<'JSON'
{{"chain":"00000000-0000-0000-0000-000000000000","chain_discriminant":{chain_discriminant},"ivm_dir":".","consensus_mode":"Permissioned","wire_protocol_version":4,"sumeragi_v2":{{"da_layout":{{"encoding":{{"encoding":"reed_solomon16","details":null}},"chunk_size_bytes":262144,"data_shards":4,"parity_shards":2,"max_payload_size_bytes":16777216,"max_chunk_count":1024}},"nexus_amx_context_hash":"6611CDC66348BEBFBD583F888864A747DCC828C5FE84F58DFB0346CCA27ABAF3","execution_policy_hash":"3F947453758F8EE90B2C66437A128FC22D93C4D2E0CA60C261D828B7E0B897C3"}},"transactions":[{{"instructions":[]}}]}}
JSON
        exit 0
        ;;
      sign)
        manifest_path="$3"
        shift 3
        while [ "$#" -gt 0 ]; do
          case "$1" in
            --out-file)
              out_file="$2"
              shift 2
              ;;
            --bound-manifest-out)
              bound_manifest_out="$2"
              shift 2
              ;;
            --expected-hash-out)
              expected_hash_out="$2"
              shift 2
              ;;
            --private-key-file)
              private_key_file="$2"
              shift 2
              ;;
            --config)
              config_file="$2"
              shift 2
              ;;
            *)
              shift
              ;;
          esac
        done
        test -s "$private_key_file"
        test -s "$config_file"
        grep -F 'expected_hash = "REPLACE_WITH_GENESIS_EXPECTED_HASH"' "$config_file" >/dev/null
        printf 'stub-signed-genesis' > "$out_file"
        printf '0000000000000000000000000000000000000000000000000000000000000001\n' > "$expected_hash_out"
        if [ "$bound_manifest_out" != "$manifest_path" ]; then
          cp "$manifest_path" "$bound_manifest_out"
        fi
        exit 0
        ;;
      *)
        echo "unsupported kagami genesis command: $2" >&2
        exit 1
        ;;
    esac
    ;;
  *)
    echo "unsupported kagami stub command: $1" >&2
    exit 1
    ;;
esac
"#
        );
        fs::write(&script_path, script).expect("write standalone kagami stub");
        #[cfg(unix)]
        {
            let mut perms = fs::metadata(&script_path)
                .expect("standalone script metadata")
                .permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script_path, perms).expect("set standalone script permissions");
        }
        let iroha_stub = write_version_stub(root, "kagami-override-iroha", "iroha3");
        let irohad_guard = EnvVarGuard::set("MOCHI_IROHAD", iroha_stub.as_os_str());
        let iroha_cli_guard = EnvVarGuard::set("MOCHI_IROHA_CLI", iroha_stub.as_os_str());
        Self {
            script_path,
            log_path,
            _irohad_guard: irohad_guard,
            _iroha_cli_guard: iroha_cli_guard,
        }
    }

    fn script_path(&self) -> &Path {
        &self.script_path
    }

    fn log_path(&self) -> &Path {
        &self.log_path
    }
}

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn ports_available(context: &str) -> bool {
    match TcpListener::bind(("127.0.0.1", 0)) {
        Ok(listener) => {
            drop(listener);
            true
        }
        Err(err)
            if matches!(
                err.kind(),
                ErrorKind::PermissionDenied | ErrorKind::AddrNotAvailable
            ) =>
        {
            eprintln!("skipping {context}: {err}");
            false
        }
        Err(err) => panic!("{context}: {err}"),
    }
}

fn test_peer_spec(
    paths: &NetworkPaths,
    alias: String,
    torii_port: u16,
    p2p_port: u16,
) -> Result<PeerSpec> {
    let peer_dir = paths.peer_dir(&alias);
    fs::create_dir_all(&peer_dir)?;
    let storage_dir = fs::canonicalize(peer_dir)?.join("storage");
    PeerSpec::new_in_generation(paths.root(), storage_dir, alias, torii_port, p2p_port)
}

fn test_genesis_material(paths: &NetworkPaths) -> GenesisMaterial {
    let genesis_dir = paths.root().join("test-fixtures").join("genesis");
    let manifest_path = genesis_dir.join(GENESIS_FILE_NAME);
    let block_path = genesis_dir.join(GENESIS_SIGNED_FILE_NAME);
    let expected_hash_path = genesis_dir.join(GENESIS_EXPECTED_HASH_FILE_NAME);
    let public_key_path = genesis_dir.join(GENESIS_PUBLIC_KEY_FILE_NAME);
    let expected_hash = "0000000000000000000000000000000000000000000000000000000000000001"
        .parse::<HashOf<BlockHeader>>()
        .expect("test genesis hash");
    let key_pair = KeyPair::random();
    fs::create_dir_all(&genesis_dir).expect("genesis fixture dir");
    fs::write(&manifest_path, b"{}").expect("write manifest");
    fs::write(&block_path, b"norito-wire-stub").expect("write signed genesis");
    fs::write(&expected_hash_path, format!("{expected_hash}\n"))
        .expect("write genesis expected hash");
    fs::write(&public_key_path, format!("{}\n", key_pair.public_key()))
        .expect("write genesis public key");

    GenesisMaterial {
        generation_id: "00000000000000000000000000000000".to_owned(),
        key_pair,
        manifest_path,
        block_path,
        expected_hash_path,
        public_key_path,
        expected_hash: Some(expected_hash),
        chain_discriminant: iroha_data_model::account::address::chain_discriminant(),
        profile: None,
        vrf_seed_hex: None,
        verify_report: None,
        consensus_fingerprint: None,
    }
}

fn npos_preset_profile(preset: ProfilePreset) -> NetworkProfile {
    let mut profile = NetworkProfile::from_preset(preset);
    profile.consensus_mode = SumeragiConsensusMode::Npos;
    profile
}

#[test]
fn binary_paths_default_respects_env_override() {
    let temp = tempfile::NamedTempFile::new().expect("temp file");
    let override_path = temp.path().to_path_buf();
    let _guard = EnvVarGuard::set("MOCHI_IROHAD", override_path.as_os_str());
    let binaries = BinaryPaths::default();
    assert_eq!(binaries.irohad_executable(), override_path.as_path());
}

#[test]
fn binary_paths_default_respects_cli_env_override() {
    let temp = tempfile::NamedTempFile::new().expect("temp file");
    let override_path = temp.path().to_path_buf();
    let _guard = EnvVarGuard::set("MOCHI_IROHA_CLI", override_path.as_os_str());
    let binaries = BinaryPaths::default();
    assert_eq!(binaries.iroha_cli_executable(), override_path.as_path());
}

#[cfg(unix)]
#[test]
fn binary_paths_auto_builds_when_enabled() {
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let empty_path_dir = temp.path().join("empty-path");
    fs::create_dir_all(&empty_path_dir).expect("create empty path dir");
    let target_dir = temp.path().join("target");
    fs::create_dir_all(&target_dir).expect("create target dir");

    let (cargo_stub, cargo_log) = write_cargo_build_stub(temp.path());
    let _cargo_guard = RestoringEnvVarGuard::set("CARGO", cargo_stub.as_os_str());
    let _target_guard = RestoringEnvVarGuard::set("CARGO_TARGET_DIR", target_dir.as_os_str());
    let _path_guard = RestoringEnvVarGuard::set("PATH", empty_path_dir.as_os_str());
    let _log_guard = RestoringEnvVarGuard::set("MOCHI_TEST_CARGO_LOG", cargo_log.as_os_str());

    let mut binaries = BinaryPaths::default().allow_auto_builds(true);
    binaries.irohad = PathBuf::from("iroha3d");
    binaries.irohad_verified = false;
    binaries.irohad_build_attempted = false;
    binaries.irohad_auto = true;
    binaries.irohad_source = BinarySource::AutoDefault;
    binaries.kagami = PathBuf::from("kagami");
    binaries.kagami_verified = false;
    binaries.kagami_build_attempted = false;
    binaries.kagami_auto = true;
    binaries.kagami_source = BinarySource::AutoDefault;
    binaries.iroha_cli = PathBuf::from("iroha_cli");
    binaries.iroha_cli_verified = false;
    binaries.iroha_cli_build_attempted = false;
    binaries.iroha_cli_auto = true;
    binaries.iroha_cli_source = BinarySource::AutoDefault;

    let versions = binaries
        .probe_versions()
        .expect("probe versions should succeed");
    assert_eq!(versions.len(), 3);
    assert!(
        versions
            .iter()
            .all(|info| info.build_line == BuildLine::Iroha3),
        "auto-built binaries should report iroha3 build-line"
    );

    let log = fs::read_to_string(&cargo_log).expect("read cargo log");
    assert!(
        log.lines()
            .any(|line| line == "build -p irohad --bin iroha3d"),
        "daemon build must select only the iroha3d target: {log}"
    );
    assert_eq!(
        log.lines().count(),
        3,
        "expected one cargo invocation per binary build"
    );

    let _ = binaries
        .probe_versions()
        .expect("second probe should succeed");
    let log = fs::read_to_string(&cargo_log).expect("read cargo log");
    assert_eq!(
        log.lines().count(),
        3,
        "second probe should not trigger additional cargo builds"
    );
}

#[cfg(unix)]
#[test]
fn binary_paths_auto_build_failure_surfaces_error() {
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let empty_path_dir = temp.path().join("empty-path");
    fs::create_dir_all(&empty_path_dir).expect("create empty path dir");
    let target_dir = temp.path().join("target");
    fs::create_dir_all(&target_dir).expect("create target dir");

    let cargo_stub = write_cargo_failure_stub(temp.path());
    let _cargo_guard = RestoringEnvVarGuard::set("CARGO", cargo_stub.as_os_str());
    let _target_guard = RestoringEnvVarGuard::set("CARGO_TARGET_DIR", target_dir.as_os_str());
    let _path_guard = RestoringEnvVarGuard::set("PATH", empty_path_dir.as_os_str());

    let mut binaries = BinaryPaths::default().allow_auto_builds(true);
    binaries.irohad = PathBuf::from("iroha3d");
    binaries.irohad_verified = false;
    binaries.irohad_build_attempted = false;
    binaries.irohad_auto = true;
    binaries.irohad_source = BinarySource::AutoDefault;

    let err = binaries
        .ensure_irohad_ready()
        .expect_err("auto-build should surface failure");
    match err {
        SupervisorError::BinaryUnavailable { binary, message } => {
            assert_eq!(binary, "iroha3d");
            assert!(
                message.contains("cargo build"),
                "expected cargo build context, got `{message}`"
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[cfg(unix)]
#[test]
fn binary_paths_resolve_iroha_cli_alias_without_building() {
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let empty_target_dir = temp.path().join("target");
    fs::create_dir_all(&empty_target_dir).expect("create target dir");
    let path_dir = temp.path().join("bin");
    fs::create_dir_all(&path_dir).expect("create bin dir");
    let iroha_stub_script = write_version_stub(&path_dir, "iroha", "iroha3");
    let iroha_stub = path_dir.join(format!("iroha{}", env::consts::EXE_SUFFIX));
    fs::copy(&iroha_stub_script, &iroha_stub).expect("copy iroha alias stub");
    #[cfg(unix)]
    {
        let mut perms = fs::metadata(&iroha_stub)
            .expect("iroha alias metadata")
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&iroha_stub, perms).expect("set iroha alias permissions");
    }
    let cargo_stub = write_cargo_failure_stub(temp.path());
    let _cargo_guard = RestoringEnvVarGuard::set("CARGO", cargo_stub.as_os_str());
    let _target_guard = RestoringEnvVarGuard::set("CARGO_TARGET_DIR", empty_target_dir.as_os_str());
    let _path_guard = RestoringEnvVarGuard::set("PATH", path_dir.as_os_str());

    let mut binaries = BinaryPaths::default().allow_auto_builds(true);
    binaries.iroha_cli = PathBuf::from("iroha_cli");
    binaries.iroha_cli_verified = false;
    binaries.iroha_cli_build_attempted = false;
    binaries.iroha_cli_auto = true;
    binaries.iroha_cli_source = BinarySource::AutoDefault;

    assert_eq!(
        resolve_name_on_path(OsStr::new("iroha")).as_deref(),
        Some(iroha_stub.as_path())
    );
    let (alias_path, alias_source) =
        resolve_iroha_cli_alias().expect("iroha alias should be discoverable");
    assert_eq!(alias_path, iroha_stub);
    assert_eq!(alias_source, BinarySource::PathSearch);

    let resolved = binaries
        .ensure_iroha_cli_ready()
        .expect("iroha alias should resolve without cargo build");
    assert_eq!(resolved, iroha_stub.as_path());
    assert_eq!(binaries.iroha_cli_source, BinarySource::PathSearch);
    assert!(!binaries.iroha_cli_build_attempted);
}

#[cfg(unix)]
#[test]
fn binary_paths_rejects_build_line_mismatch() {
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let iroha2_stub = write_version_stub(temp.path(), "iroha2-stub", "iroha2");
    let iroha3_stub = write_version_stub(temp.path(), "iroha3-stub", "iroha3");

    let mut binaries = BinaryPaths::default()
        .irohad(iroha2_stub)
        .kagami(iroha3_stub.clone())
        .iroha_cli(iroha3_stub);

    let err = binaries
        .verify_build_line(BuildLine::Iroha3)
        .expect_err("iroha2 binary should fail build-line verification");
    match err {
        SupervisorError::BuildLineMismatch {
            binary,
            expected,
            found,
        } => {
            assert_eq!(binary, "iroha3d");
            assert_eq!(expected, BuildLine::Iroha3);
            assert_eq!(found, BuildLine::Iroha2);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn builder_creates_peer_configs() {
    if !ports_available("builder_creates_peer_configs") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
        .data_root(temp.path())
        .chain_id("test-chain")
        .torii_base_port(9000)
        .p2p_base_port(19000)
        .build()
        .expect("build supervisor");

    assert_eq!(supervisor.chain_id(), "test-chain");
    assert_eq!(supervisor.peers().len(), 4);
    let network_id = supervisor
        .network_id()
        .expect("validated genesis hash yields an exact network id");
    let mut transport_public_keys = HashSet::new();
    for peer in supervisor.peers() {
        assert_eq!(
            peer.torii_client()
                .expect("managed config yields a network-bound Torii client")
                .network_id(),
            Some(network_id)
        );
        KeyPair::new(
            peer.spec.keys.soranet_transport_public_key.clone(),
            peer.spec.keys.soranet_transport_private_key.0.clone(),
        )
        .expect("Mochi transport public/private key pair must match");
        assert_eq!(
            peer.spec.keys.soranet_transport_public_key.algorithm(),
            Algorithm::Ed25519
        );
        assert_ne!(
            peer.spec.keys.soranet_transport_public_key, peer.spec.keys.public_key,
            "transport and consensus signing identities must differ"
        );
        assert_ne!(
            peer.spec.keys.soranet_transport_public_key, peer.spec.keys.identity_public_key,
            "transport and streaming identities must differ"
        );
        assert!(
            transport_public_keys.insert(peer.spec.keys.soranet_transport_public_key.clone()),
            "Mochi peers must not share a transport identity"
        );
    }

    #[cfg(unix)]
    for peer in supervisor.peers() {
        let metadata = fs::symlink_metadata(peer.config_path()).expect("peer config metadata");
        assert!(metadata.file_type().is_file());
        assert!(!metadata.file_type().is_symlink());
        assert_eq!(metadata.uid(), fs::metadata(temp.path()).unwrap().uid());
        assert_eq!(metadata.nlink(), 1);
        assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
    }

    let peer = &supervisor.peers()[0];
    let config_path = peer.config_path().to_path_buf();
    let contents = fs::read_to_string(config_path).expect("config readable");
    let value: toml::Table = toml::from_str(&contents).expect("valid toml");
    let expected_torii = "0.0.0.0:9000"
        .parse::<iroha_primitives::addr::SocketAddr>()
        .expect("torii addr literal")
        .to_literal();
    let expected_public = "127.0.0.1:19000"
        .parse::<iroha_primitives::addr::SocketAddr>()
        .expect("public addr literal")
        .to_literal();

    assert_eq!(
        value.get("chain").and_then(toml::Value::as_str),
        Some("test-chain")
    );
    assert_eq!(
        value
            .get("soranet_transport_public_key")
            .and_then(toml::Value::as_str),
        Some(
            peer.spec
                .keys
                .soranet_transport_public_key
                .to_string()
                .as_str()
        )
    );
    assert_eq!(
        value
            .get("soranet_transport_private_key")
            .and_then(toml::Value::as_str),
        Some(
            peer.spec
                .keys
                .soranet_transport_private_key
                .to_string()
                .as_str()
        )
    );
    assert_eq!(
        value
            .get("genesis")
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get("expected_hash"))
            .and_then(toml::Value::as_str),
        Some("hash:0000000000000000000000000000000000000000000000000000000000000001#C50E")
    );
    assert!(!contents.contains(GENESIS_EXPECTED_HASH_PLACEHOLDER));
    assert!(
        value
            .get("sumeragi")
            .and_then(toml::Value::as_table)
            .is_none_or(|table| !table.contains_key("consensus_mode")),
        "consensus mode is signed-genesis state, not a mutable peer setting"
    );
    assert_eq!(
        value
            .get("confidential")
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get("enabled"))
            .and_then(toml::Value::as_bool),
        Some(true)
    );
    assert!(
        value
            .get("settlement")
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get("offline"))
            .is_none(),
        "offline protocol support is universal and must not be represented as a Mochi opt-in"
    );
    assert_eq!(
        value
            .get("torii")
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get("address"))
            .and_then(toml::Value::as_str),
        Some(expected_torii.as_str())
    );
    assert_eq!(
        value
            .get("torii")
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get("mcp"))
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get("enabled"))
            .and_then(toml::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        value
            .get("torii")
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get("transport"))
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get("norito_rpc"))
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get("enabled"))
            .and_then(toml::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        value
            .get("torii")
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get("transport"))
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get("norito_rpc"))
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get("stage"))
            .and_then(toml::Value::as_str),
        Some(LOCAL_NORITO_RPC_STAGE)
    );
    assert_eq!(
        value
            .get("network")
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get("public_address"))
            .and_then(toml::Value::as_str),
        Some(expected_public.as_str())
    );
    let expected_kura = peer.storage_dir().join("kura");
    let expected_snapshot = peer.storage_dir().join("snapshot");
    let expected_torii_data = peer.storage_dir().join("torii");
    assert_eq!(peer.kura_store_dir(), expected_kura);
    assert_eq!(
        value
            .get("kura")
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get("store_dir"))
            .and_then(toml::Value::as_str),
        Some(expected_kura.to_string_lossy().as_ref())
    );
    assert_eq!(
        value
            .get("snapshot")
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get("store_dir"))
            .and_then(toml::Value::as_str),
        Some(expected_snapshot.to_string_lossy().as_ref())
    );
    assert_eq!(
        value
            .get("torii")
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get("data_dir"))
            .and_then(toml::Value::as_str),
        Some(expected_torii_data.to_string_lossy().as_ref())
    );
    assert!(
        !expected_kura.exists(),
        "Mochi must leave a new Kura root absent so Kura can establish its authenticated catalog"
    );
    let snapshot_generations = expected_snapshot.join(SNAPSHOT_GENERATIONS_DIR_NAME);
    assert!(
        snapshot_generations.is_dir(),
        "an explicit snapshot root must contain its authenticated generations directory"
    );
    assert!(
        fs::read_dir(snapshot_generations)
            .expect("snapshot generations directory")
            .next()
            .is_none(),
        "new snapshot generations directory should be empty"
    );
    let streaming = value
        .get("streaming")
        .and_then(toml::Value::as_table)
        .expect("streaming config");
    let identity_public = streaming
        .get("identity_public_key")
        .and_then(toml::Value::as_str)
        .expect("identity public key");
    let identity_private = streaming
        .get("identity_private_key")
        .and_then(toml::Value::as_str)
        .expect("identity private key");
    assert!(
        identity_public
            .parse::<PublicKey>()
            .expect("identity public key should parse")
            .algorithm()
            == Algorithm::Ed25519,
        "expected Ed25519 identity public key, got {identity_public}"
    );
    assert!(
        !identity_private.is_empty(),
        "identity private key should be populated"
    );
    let operator_public_keys = value
        .get("torii")
        .and_then(toml::Value::as_table)
        .and_then(|torii| torii.get("operator_signatures"))
        .and_then(toml::Value::as_table)
        .and_then(|operator| operator.get("allowed_public_keys"))
        .and_then(toml::Value::as_array)
        .expect("managed operator public keys");
    assert!(
        operator_public_keys
            .iter()
            .any(|value| value.as_str() == Some(identity_public)),
        "managed streaming identity must be allow-listed for exact-network operator reads"
    );
}

#[test]
fn builder_reserves_unique_ports_across_torii_and_p2p() {
    if !ports_available("builder_reserves_unique_ports_across_torii_and_p2p") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let base_port = 32000;
    let supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
        .data_root(temp.path())
        .torii_base_port(base_port)
        .p2p_base_port(base_port)
        .build()
        .expect("build supervisor");

    let mut seen_ports = HashSet::new();
    for peer in supervisor.peers() {
        for addr in [peer.torii_address(), peer.p2p_address()] {
            let port = addr
                .parse::<std::net::SocketAddr>()
                .map(|socket| socket.port())
                .expect("address contains a port");
            assert!(
                seen_ports.insert(port),
                "port {port} should be unique across Torii and P2P assignments"
            );
        }
    }
}

#[test]
fn relative_data_root_renders_cwd_independent_peer_paths() {
    fn configured_path<'a>(config: &'a toml::Table, keys: &[&str]) -> &'a Path {
        let label = keys.join(".");
        let mut value = config
            .get(keys[0])
            .unwrap_or_else(|| panic!("missing `{label}`"));
        for key in &keys[1..] {
            value = value
                .as_table()
                .and_then(|table| table.get(*key))
                .unwrap_or_else(|| panic!("missing `{label}`"));
        }
        Path::new(
            value
                .as_str()
                .unwrap_or_else(|| panic!("`{label}` must be a path string")),
        )
    }

    fn resolve_from_peer_cwd(peer_cwd: &Path, path: &Path) -> PathBuf {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            peer_cwd.join(path)
        }
    }

    if !ports_available("relative_data_root_renders_cwd_independent_peer_paths") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let caller_cwd = env::current_dir().expect("caller current directory");
    let data_root = tempfile::Builder::new()
        .prefix(".mochi-relative-data-root-")
        .tempdir_in(&caller_cwd)
        .expect("relative data-root fixture");
    let relative_data_root = data_root
        .path()
        .strip_prefix(&caller_cwd)
        .expect("fixture must be beneath caller cwd");
    assert!(relative_data_root.is_relative());
    assert_eq!(
        resolve_data_root(relative_data_root).expect("resolve relative data root"),
        data_root.path()
    );
    assert_eq!(
        resolve_data_root(data_root.path()).expect("preserve absolute data root"),
        data_root.path()
    );

    let _stub = KagamiStub::install(data_root.path());
    let profile = NetworkProfile::from_preset(ProfilePreset::FourPeerBft);
    let expected_network_root = data_root.path().join(profile.slug());
    let supervisor = SupervisorBuilder::with_profile(profile)
        .data_root(relative_data_root)
        .torii_base_port(30_000)
        .p2p_base_port(31_000)
        .build()
        .expect("build supervisor from relative data root");

    assert_eq!(supervisor.paths().root(), expected_network_root);
    assert!(supervisor.genesis_manifest().is_absolute());
    assert!(supervisor.genesis_manifest().is_file());
    assert!(supervisor.genesis_block_file().is_absolute());
    assert!(supervisor.genesis_block_file().is_file());

    for peer in supervisor.peers() {
        assert!(peer.config_path().is_absolute());
        assert!(peer.config_path().is_file());
        #[cfg(unix)]
        assert_eq!(
            fs::metadata(peer.config_path())
                .expect("peer config metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600,
            "peer config contains private keys and must never be group/world-readable"
        );
        assert!(peer.log_path().is_absolute());
        let peer_cwd = peer
            .config_path()
            .parent()
            .expect("peer config parent is the isolated child cwd");
        let config: toml::Table = toml::from_str(
            &fs::read_to_string(peer.config_path()).expect("read generated peer config"),
        )
        .expect("parse generated peer config");

        let genesis_file = configured_path(&config, &["genesis", "file"]);
        let genesis_manifest = configured_path(&config, &["genesis", "manifest_json"]);
        let rans_tables = configured_path(&config, &["streaming", "codec", "rans_tables_path"]);
        for file in [genesis_file, genesis_manifest, rans_tables] {
            assert!(file.is_absolute(), "{} must be absolute", file.display());
            let resolved = resolve_from_peer_cwd(peer_cwd, file);
            assert_eq!(resolved, file);
            assert!(
                resolved.is_file(),
                "{} must remain readable from peer cwd {}",
                resolved.display(),
                peer_cwd.display()
            );
        }
        assert_eq!(genesis_file, supervisor.genesis_block_file());
        assert_eq!(genesis_manifest, supervisor.genesis_manifest());
        assert!(rans_tables.starts_with(peer_cwd));

        for keys in [
            &["kura", "store_dir"][..],
            &["snapshot", "store_dir"][..],
            &["sorafs", "storage", "data_dir"][..],
            &["streaming", "session_store_dir"][..],
            &["streaming", "soranet", "provision_spool_dir"][..],
            &["streaming", "soravpn", "provision_spool_dir"][..],
            &["torii", "data_dir"][..],
            &["torii", "da_ingest", "replay_cache_store_dir"][..],
            &["torii", "da_ingest", "manifest_store_dir"][..],
            &[
                "network",
                "soranet_handshake",
                "pow",
                "revocation_store_path",
            ][..],
        ] {
            let state_path = configured_path(&config, keys);
            assert!(
                state_path.is_absolute(),
                "{} must be absolute",
                keys.join(".")
            );
            assert_eq!(resolve_from_peer_cwd(peer_cwd, state_path), state_path);
            assert!(
                state_path.starts_with(peer.storage_dir()),
                "{} must remain inside mutable storage for {}",
                state_path.display(),
                peer.alias()
            );
            assert!(
                !state_path.starts_with(peer_cwd),
                "mutable state {} must remain outside immutable config generation {}",
                state_path.display(),
                peer_cwd.display()
            );
        }
    }
}

#[test]
fn multi_peer_trusted_peers_list_everyone() {
    if !ports_available("multi_peer_trusted_peers_list_everyone") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
        .data_root(temp.path())
        .torii_base_port(8000)
        .p2p_base_port(17000)
        .build()
        .expect("build supervisor");

    assert_eq!(supervisor.peers().len(), 4);

    let first_config =
        fs::read_to_string(supervisor.peers()[0].config_path()).expect("config readable");
    let value: toml::Table = toml::from_str(&first_config).expect("valid toml");
    let trusted = value
        .get("trusted_peers")
        .and_then(toml::Value::as_array)
        .expect("array");
    assert_eq!(trusted.len(), 4);
}

#[test]
fn multi_peer_configs_bound_soranet_pow_for_local_full_mesh() {
    if !ports_available("multi_peer_configs_bound_soranet_pow_for_local_full_mesh") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
        .data_root(temp.path().join("four-peer"))
        .torii_base_port(28000)
        .p2p_base_port(29000)
        .build()
        .expect("build four-peer supervisor");
    let mut revocation_paths = HashSet::new();

    for (index, peer) in supervisor.peers().iter().enumerate() {
        let config: toml::Table =
            toml::from_str(&fs::read_to_string(peer.config_path()).expect("peer config readable"))
                .expect("valid peer config");
        let pow = config
            .get("network")
            .and_then(toml::Value::as_table)
            .and_then(|network| network.get("soranet_handshake"))
            .and_then(toml::Value::as_table)
            .and_then(|handshake| handshake.get("pow"))
            .and_then(toml::Value::as_table)
            .expect("multi-peer PoW override");
        assert_eq!(
            pow.get("ticket_ttl_secs").and_then(toml::Value::as_integer),
            Some(LOCAL_MULTI_PEER_POW_TICKET_TTL_SECS),
            "peer{index} must allow local full-mesh handshakes to finish"
        );
        assert_eq!(
            pow.get("difficulty").and_then(toml::Value::as_integer),
            Some(LOCAL_MULTI_PEER_POW_DIFFICULTY),
            "peer{index} must bound local full-mesh hashcash work"
        );
        let revocation_path = PathBuf::from(
            pow.get("revocation_store_path")
                .and_then(toml::Value::as_str)
                .expect("peer-local revocation ledger path"),
        );
        assert!(
            revocation_path.is_absolute(),
            "peer{index} revocation ledger must not depend on the launcher cwd"
        );
        assert!(
            revocation_paths.insert(revocation_path),
            "peer{index} must own a unique revocation ledger lock"
        );
        let puzzle = pow
            .get("puzzle")
            .and_then(toml::Value::as_table)
            .expect("multi-peer puzzle override");
        assert_eq!(
            puzzle.get("memory_kib").and_then(toml::Value::as_integer),
            Some(LOCAL_MULTI_PEER_POW_PUZZLE_MEMORY_KIB)
        );
        assert_eq!(
            puzzle.get("time_cost").and_then(toml::Value::as_integer),
            Some(LOCAL_MULTI_PEER_POW_PUZZLE_TIME_COST)
        );
        assert_eq!(
            puzzle.get("lanes").and_then(toml::Value::as_integer),
            Some(LOCAL_MULTI_PEER_POW_PUZZLE_LANES)
        );
    }
    assert_eq!(revocation_paths.len(), 4);

    let legacy = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path().join("single-peer"))
        .torii_base_port(30000)
        .p2p_base_port(31000)
        .build()
        .expect("build historical profile alias");
    assert_eq!(
        legacy.peers().len(),
        4,
        "the historical profile name must still launch a safe committee"
    );
}

#[test]
fn custom_profile_supports_seven_peers_with_unique_ports() {
    if !ports_available("custom_profile_supports_seven_peers_with_unique_ports") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let profile = NetworkProfile::custom(7, SumeragiConsensusMode::Permissioned).expect("profile");
    let supervisor = SupervisorBuilder::with_profile(profile)
        .data_root(temp.path())
        .torii_base_port(8100)
        .p2p_base_port(17100)
        .build()
        .expect("build supervisor");

    assert_eq!(supervisor.peers().len(), 7);

    let mut seen_ports = HashSet::new();
    for peer in supervisor.peers() {
        for addr in [peer.torii_address(), peer.p2p_address()] {
            let port = addr
                .parse::<std::net::SocketAddr>()
                .map(|socket| socket.port())
                .expect("address contains a port");
            assert!(
                seen_ports.insert(port),
                "port {port} should be unique across Torii and P2P assignments"
            );
        }
    }

    let first_config =
        fs::read_to_string(supervisor.peers()[0].config_path()).expect("config readable");
    let value: toml::Table = toml::from_str(&first_config).expect("valid toml");
    let trusted = value
        .get("trusted_peers")
        .and_then(toml::Value::as_array)
        .expect("array");
    assert_eq!(trusted.len(), 7);
}

#[test]
fn profile_preset_preserves_consensus_mode() {
    let profile = NetworkProfile::custom(7, SumeragiConsensusMode::Npos).expect("profile");
    let builder =
        SupervisorBuilder::with_profile(profile).profile_preset(ProfilePreset::SinglePeer);
    assert_eq!(builder.profile().preset, Some(ProfilePreset::SinglePeer));
    assert_eq!(builder.profile().topology.peer_count, 4);
    assert_eq!(
        builder.profile().consensus_mode,
        SumeragiConsensusMode::Npos
    );
}

#[test]
fn build_rejects_genesis_profile_without_npos() {
    let temp = tempfile::tempdir().expect("tempdir");
    let profile = NetworkProfile::custom(4, SumeragiConsensusMode::Permissioned).expect("profile");
    let builder = SupervisorBuilder::with_profile(profile)
        .data_root(temp.path())
        .genesis_profile(GenesisProfile::Iroha3Dev)
        .set_profile(
            NetworkProfile::custom(4, SumeragiConsensusMode::Permissioned).expect("profile"),
        );

    let err = builder
        .build()
        .expect_err("expected consensus mode mismatch");
    assert!(
        err.to_string()
            .contains("genesis_profile requires consensus_mode npos"),
        "unexpected error: {err}"
    );
}

#[test]
fn genesis_includes_topology() {
    if !ports_available("genesis_includes_topology") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");

    let bytes = fs::read(supervisor.genesis_manifest()).expect("genesis manifest readable");
    let manifest: norito::json::Value =
        norito::json::from_slice(&bytes).expect("parse genesis json");
    let transactions = manifest
        .get("transactions")
        .and_then(norito::json::Value::as_array)
        .expect("transactions array");
    let contains_topology = transactions.iter().any(|tx| {
        tx.get("topology")
            .and_then(norito::json::Value::as_array)
            .map(|entries| !entries.is_empty())
            .unwrap_or(false)
    });
    assert!(
        contains_topology,
        "genesis manifest should include topology transaction"
    );
}

#[test]
fn genesis_generation_invokes_kagami() {
    if !ports_available("genesis_generation_invokes_kagami") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let stub = KagamiStub::install(temp.path());
    let supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");

    let log = fs::read_to_string(stub.log_path()).expect("kagami invocation log");
    assert!(
        log.contains("genesis") && log.contains("generate"),
        "expected kagami invocation to record subcommand, got `{log}`"
    );
    assert!(
        log.contains("--genesis-public-key"),
        "expected kagami invocation to record genesis public key argument"
    );
    assert!(
        log.contains("--consensus-mode") && log.contains("permissioned"),
        "expected permissioned consensus mode to be pinned for kagami: {log}"
    );
    assert!(
        log.contains("genesis sign")
            && log.contains("--config")
            && log.contains("--bound-manifest-out")
            && log.contains("--private-key-file"),
        "expected config-bound kagami signing with persisted manifest metadata: {log}"
    );
    assert!(
        !log.split_whitespace()
            .any(|argument| argument == "--private-key"),
        "the genesis private key must never be exposed on the kagami command line: {log}"
    );
    let genesis_dir = supervisor
        .genesis_manifest()
        .parent()
        .expect("genesis directory");
    assert!(
        fs::read_dir(genesis_dir)
            .expect("read genesis directory")
            .all(|entry| {
                !entry
                    .expect("genesis entry")
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".mochi-genesis-signing-key-")
            }),
        "temporary genesis signing keys must be removed after kagami exits"
    );
}

#[test]
fn generated_genesis_binds_topology_specific_block_cadence() {
    if !ports_available("generated_genesis_binds_topology_specific_block_cadence") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());

    for (preset, expected_cadence_ms) in [
        (ProfilePreset::SinglePeer, 1_000),
        (ProfilePreset::FourPeerBft, 1_000),
    ] {
        let supervisor = SupervisorBuilder::new(preset)
            .data_root(temp.path().join(format!("cadence-{}", preset.slug())))
            .build()
            .expect("build supervisor");
        let manifest = RawGenesisTransaction::from_path(supervisor.genesis_manifest())
            .expect("load generated genesis manifest");
        let actual_cadence_ms = manifest
            .effective_parameters()
            .expect("derive effective genesis parameters")
            .sumeragi()
            .block_cadence_ms()
            .get();

        assert_eq!(
            actual_cadence_ms,
            expected_cadence_ms,
            "{} must sign the topology-appropriate local cadence",
            preset.slug()
        );
    }
}

#[cfg(unix)]
#[test]
fn temporary_genesis_key_file_is_owner_only_and_removed_on_drop() {
    let temp = tempfile::tempdir().expect("tempdir");
    let key_pair = KeyPair::random();
    let key_file =
        TemporaryGenesisKeyFile::create(temp.path(), &key_pair).expect("create key file");
    let path = key_file.path().to_path_buf();
    let metadata = fs::metadata(&path).expect("temporary key metadata");
    assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
    assert_eq!(
        fs::read_to_string(&path).expect("read temporary key"),
        format!("{}\n", ExposedPrivateKey(key_pair.private_key().clone()))
    );

    drop(key_file);
    assert!(!path.exists(), "temporary key should be removed on drop");
}

#[cfg(unix)]
#[test]
fn temporary_genesis_key_file_resolves_symlinked_directory_components() {
    let temp = tempfile::tempdir().expect("tempdir");
    let real_genesis_dir = temp.path().join("real-genesis");
    fs::create_dir(&real_genesis_dir).expect("create real genesis directory");
    let linked_genesis_dir = temp.path().join("linked-genesis");
    symlink(&real_genesis_dir, &linked_genesis_dir).expect("link genesis directory");

    let key_file = TemporaryGenesisKeyFile::create(&linked_genesis_dir, &KeyPair::random())
        .expect("create key through symlinked directory");
    assert!(
        key_file
            .path()
            .starts_with(fs::canonicalize(&real_genesis_dir).expect("canonical genesis dir")),
        "private key path must contain no symlinked directory component: {}",
        key_file.path().display()
    );
}

#[test]
fn kagami_sign_failure_is_reported_and_removes_temporary_key() {
    if !ports_available("kagami_sign_failure_is_reported_and_removes_temporary_key") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let _failure = EnvVarGuard::set("MOCHI_KAGAMI_FAIL_SIGN", OsStr::new("1"));

    let error = match SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
    {
        Ok(_) => panic!("requested kagami sign failure should fail supervisor build"),
        Err(error) => error,
    };
    assert!(
        matches!(error, SupervisorError::KagamiInvocation(ref message) if message.contains("`kagami genesis sign`") && message.contains("exit status: 23") && message.contains("requested kagami sign failure")),
        "unexpected signing failure: {error}"
    );
    let mut files = Vec::new();
    collect_files_recursive(temp.path(), &mut files).expect("collect temporary files");
    assert!(
        files.iter().all(|path| {
            !path.file_name().is_some_and(|name| {
                name.to_string_lossy()
                    .starts_with(".mochi-genesis-signing-key-")
            })
        }),
        "temporary genesis signing key leaked after failure: {files:?}"
    );
}

#[test]
fn genesis_profile_and_seed_forward_to_kagami() {
    if !ports_available("genesis_profile_and_seed_forward_to_kagami") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let stub = KagamiStub::install(temp.path());
    let seed = "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";

    SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .genesis_profile(GenesisProfile::Iroha3Dev)
        .vrf_seed_hex(seed)
        .build()
        .expect("build supervisor");

    let log = fs::read_to_string(stub.log_path()).expect("kagami invocation log");
    assert!(
        log.contains("--profile") && log.contains("iroha3-dev"),
        "profile should be forwarded to kagami: {log}"
    );
    assert!(
        log.contains("--vrf-seed-hex") && log.contains(seed),
        "vrf seed should be forwarded to kagami: {log}"
    );
    assert!(
        log.contains("--consensus-mode") && log.contains("npos"),
        "npos mode should be pinned when a genesis profile is used: {log}"
    );
}

#[test]
fn peer_config_records_chain_and_fingerprint_header() {
    if !ports_available("peer_config_records_chain_and_fingerprint_header") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .genesis_profile(GenesisProfile::Iroha3Dev)
        .build()
        .expect("build supervisor");

    let manifest =
        RawGenesisTransaction::from_path(supervisor.genesis_manifest()).expect("genesis");
    let fingerprint = manifest
        .consensus_fingerprint()
        .map(|value| value.to_string())
        .or_else(|| {
            let normalized = manifest.clone().with_consensus_meta();
            normalized
                .consensus_fingerprint()
                .map(|value| value.to_string())
        })
        .expect("consensus fingerprint");

    let peer = supervisor.peers().first().expect("peer");
    let config_text = fs::read_to_string(peer.config_path()).expect("read config");
    let expected_chain = format!("# mochi.chain_id = {}", supervisor.chain_id());
    let expected_fingerprint = format!("# mochi.consensus_fingerprint = {fingerprint}");

    assert!(
        config_text.contains(&expected_chain),
        "config should record chain id header"
    );
    assert!(
        config_text.contains(&expected_fingerprint),
        "config should record consensus fingerprint header"
    );
}

#[test]
fn readiness_smoke_plan_uses_primary_signer_and_unique_nonces() {
    if !ports_available("readiness_smoke_plan_uses_primary_signer_and_unique_nonces") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");

    let plan = supervisor
        .readiness_smoke_plan_with_offset(3, 2)
        .expect("build readiness plan");
    assert_eq!(plan.transactions.len(), 3);

    let expected_authority = supervisor
        .readiness_smoke_signer()
        .expect("readiness signer available")
        .account_id()
        .clone();
    let mut nonces = HashSet::new();
    for (idx, tx) in plan.transactions.iter().enumerate() {
        assert_eq!(tx.authority(), &expected_authority);
        let nonce = tx.nonce().expect("nonce present");
        let nonce_value = u32::from(nonce);
        assert!(nonces.insert(nonce_value), "nonce should be unique");
        assert_eq!(
            nonce_value,
            (idx as u32) + 3,
            "nonce should incorporate offset"
        );
    }
}

#[test]
fn export_snapshot_captures_storage_and_metadata() {
    if !ports_available("export_snapshot_captures_storage_and_metadata") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");

    let mut peer_aliases = Vec::new();
    let mut expected_storage = Vec::new();
    for (idx, peer) in supervisor.peers().iter().enumerate() {
        let storage_file = peer.storage_dir().join(format!("sentinel-{idx}.bin"));
        let storage_contents = format!("peer-storage-{idx}");
        fs::write(&storage_file, &storage_contents).expect("write storage sentinel");

        let snapshot_marker = peer.snapshot_dir().join("marker.txt");
        fs::write(&snapshot_marker, b"snapshot-marker").expect("write snapshot marker");

        let log_path = peer.log_path();
        if let Some(parent) = log_path.parent() {
            fs::create_dir_all(parent).expect("create logs directory");
        }
        fs::write(log_path, format!("log-entry-{idx}")).expect("write peer log");
        peer_aliases.push(peer.alias().to_owned());
        expected_storage.push(storage_contents.into_bytes());
    }

    let snapshot_root = supervisor
        .export_snapshot(Some("Smoke Snapshot 2026"))
        .expect("export snapshot");

    assert_eq!(
        snapshot_root.file_name().unwrap(),
        std::ffi::OsStr::new("smoke-snapshot-2026"),
        "label should be sanitized before export"
    );
    assert!(snapshot_root.exists(), "snapshot directory should exist");

    let metadata_path = snapshot_root.join("metadata.json");
    let metadata_bytes = fs::read(&metadata_path).expect("read metadata");
    let metadata: Value =
        norito::json::from_slice(&metadata_bytes).expect("parse snapshot metadata");
    assert_eq!(
        metadata
            .get("chain_id")
            .and_then(Value::as_str)
            .expect("chain id present"),
        supervisor.chain_id()
    );
    assert_eq!(
        metadata
            .get("peer_count")
            .and_then(Value::as_u64)
            .expect("peer count present"),
        peer_aliases.len() as u64
    );
    assert!(
        metadata.get("created_at_ms").is_some(),
        "metadata should include creation timestamp"
    );
    assert_eq!(
        metadata.get("storage_layout").and_then(Value::as_str),
        Some(SNAPSHOT_STORAGE_LAYOUT),
        "metadata should pin the Kura subdirectory layout"
    );
    let genesis_hash = metadata
        .get("genesis_hash")
        .and_then(Value::as_str)
        .expect("genesis hash present");
    let actual_genesis_hash =
        Hash::new(fs::read(supervisor.genesis_manifest()).expect("read genesis"));
    assert_eq!(
        genesis_hash,
        actual_genesis_hash.to_string(),
        "metadata should record the current genesis hash"
    );
    let kura_hashes = metadata
        .get("kura_hashes")
        .and_then(Value::as_object)
        .expect("kura hashes map present");
    assert_eq!(
        kura_hashes.len(),
        peer_aliases.len(),
        "kura hashes should track every peer"
    );

    for (idx, alias) in peer_aliases.iter().enumerate() {
        let alias_root = snapshot_root.join("peers").join(alias);
        let storage_copy = alias_root
            .join("storage")
            .join(format!("sentinel-{idx}.bin"));
        let snapshot_copy = alias_root
            .join("storage")
            .join("snapshot")
            .join("marker.txt");
        let config_copy = alias_root.join("config.toml");
        let log_copy = alias_root.join("latest.log");

        assert_eq!(
            fs::read(&storage_copy).expect("storage copy should exist"),
            expected_storage[idx].as_slice(),
            "storage sentinel should be copied for peer {alias}"
        );
        assert_eq!(
            fs::read(&snapshot_copy).expect("snapshot copy should exist"),
            b"snapshot-marker",
            "snapshot marker should be copied for peer {alias}"
        );
        assert!(
            config_copy.exists(),
            "config should be exported for peer {alias}"
        );
        assert!(log_copy.exists(), "log should be exported for peer {alias}");
        let expected_hash =
            hash_directory(&alias_root.join("storage")).expect("hash copied storage");
        let recorded_hash = kura_hashes
            .get(alias)
            .and_then(Value::as_str)
            .expect("kura hash entry present");
        assert_eq!(
            recorded_hash,
            expected_hash.to_string(),
            "kura hash should match exported storage contents"
        );
    }

    let genesis_copy = snapshot_root.join("genesis").join(GENESIS_FILE_NAME);
    let signed_genesis_copy = snapshot_root.join("genesis").join(GENESIS_SIGNED_FILE_NAME);
    assert!(
        genesis_copy.exists(),
        "snapshot should include the current genesis manifest"
    );
    assert!(
        signed_genesis_copy.exists(),
        "snapshot should include the signed genesis wire file"
    );
}

#[test]
fn snapshot_export_and_restore_hold_shared_selection_lease() {
    if !ports_available("snapshot_export_and_restore_hold_shared_selection_lease") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");
    let root = supervisor.paths().root().to_path_buf();
    let expected = Some(supervisor.generation_id().to_owned());
    let storage = supervisor.peers()[0].storage_dir().to_path_buf();
    fs::write(storage.join("lease-marker"), b"snapshot").expect("write snapshot marker");

    let snapshot = supervisor
        .export_snapshot_with_selection_hook(Some("Selection Lease"), || {
            let error = GenerationTransaction::begin_replacing(&root, expected.clone())
                .expect_err("export lease must block an exclusive writer");
            assert!(matches!(error, SupervisorError::GenerationLocked { .. }));
        })
        .expect("export snapshot under lease");
    fs::write(storage.join("lease-marker"), b"mutated").expect("mutate storage marker");
    supervisor
        .restore_snapshot_with_selection_hook(&snapshot, || {
            let error = GenerationTransaction::begin_replacing(&root, expected.clone())
                .expect_err("restore lease must block an exclusive writer");
            assert!(matches!(error, SupervisorError::GenerationLocked { .. }));
        })
        .expect("restore snapshot under lease");
    assert_eq!(
        fs::read(storage.join("lease-marker")).expect("read restored marker"),
        b"snapshot"
    );
    GenerationTransaction::begin_replacing(&root, expected)
        .expect("writer succeeds after snapshot operation releases its lease");
}

#[cfg(unix)]
#[test]
fn snapshot_operations_restore_exact_partial_running_set() {
    if !ports_available("snapshot_operations_restore_exact_partial_running_set") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let irohad = write_long_running_irohad_stub(temp.path());
    let _irohad = EnvVarGuard::set("MOCHI_IROHAD", irohad.as_os_str());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");
    supervisor.start_peer("peer0").expect("start peer0");
    supervisor.start_peer("peer2").expect("start peer2");

    let snapshot = supervisor
        .export_snapshot(Some("Partial Running Set"))
        .expect("export snapshot");
    let running_after_export = supervisor
        .peers()
        .iter()
        .filter(|peer| peer.is_running())
        .map(|peer| peer.alias().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(running_after_export, vec!["peer0", "peer2"]);

    supervisor
        .restore_snapshot(&snapshot)
        .expect("restore snapshot");
    let running_after_restore = supervisor
        .peers()
        .iter()
        .filter(|peer| peer.is_running())
        .map(|peer| peer.alias().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(running_after_restore, vec!["peer0", "peer2"]);
    supervisor.stop_all().expect("stop partial running set");
}

#[cfg(unix)]
#[test]
fn export_snapshot_failure_restores_full_running_set() {
    if !ports_available("export_snapshot_failure_restores_full_running_set") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let irohad = write_long_running_irohad_stub(temp.path());
    let _irohad = EnvVarGuard::set("MOCHI_IROHAD", irohad.as_os_str());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");
    supervisor.start_all().expect("start full running set");
    supervisor
        .export_snapshot(Some("Duplicate Snapshot"))
        .expect("create first snapshot");
    assert!(supervisor.peers()[0].is_running());

    let error = supervisor
        .export_snapshot(Some("Duplicate Snapshot"))
        .expect_err("duplicate snapshot must fail after peers stop");
    assert!(matches!(error, SupervisorError::SnapshotExists { .. }));
    assert!(
        supervisor.peers()[0].is_running(),
        "full running set must be restored on export failure"
    );
    supervisor.stop_all().expect("stop restored peer");
}

#[test]
fn export_snapshot_preserves_multilane_catalog_and_ports() {
    if !ports_available("export_snapshot_preserves_multilane_catalog_and_ports") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());

    let mut nexus = toml::Table::new();
    nexus.insert("enabled".into(), toml::Value::Boolean(true));
    let mut lane0 = toml::Table::new();
    lane0.insert("alias".into(), toml::Value::String("core".into()));
    lane0.insert("index".into(), toml::Value::Integer(0));
    lane0.insert("dataspace".into(), toml::Value::String("universal".into()));
    let mut lane1 = toml::Table::new();
    lane1.insert("alias".into(), toml::Value::String("governance".into()));
    lane1.insert("index".into(), toml::Value::Integer(1));
    lane1.insert("dataspace".into(), toml::Value::String("universal".into()));
    nexus.insert(
        "lane_catalog".into(),
        toml::Value::Array(vec![toml::Value::Table(lane0), toml::Value::Table(lane1)]),
    );
    let mut dataspace = toml::Table::new();
    dataspace.insert("alias".into(), toml::Value::String("universal".into()));
    dataspace.insert("id".into(), toml::Value::Integer(0));
    nexus.insert(
        "dataspace_catalog".into(),
        toml::Value::Array(vec![toml::Value::Table(dataspace)]),
    );

    let mut supervisor =
        SupervisorBuilder::with_profile(npos_preset_profile(ProfilePreset::FourPeerBft))
            .data_root(temp.path())
            .nexus_config(nexus)
            .build()
            .expect("build supervisor");

    let snapshot_root = supervisor
        .export_snapshot(Some("Multilane Snapshot"))
        .expect("export snapshot");

    let genesis_bytes = fs::read(snapshot_root.join("genesis").join(GENESIS_FILE_NAME))
        .expect("read snapshot genesis");
    let manifest: Value = norito::json::from_slice(&genesis_bytes).expect("parse genesis json");
    let chain = manifest
        .get("chain")
        .and_then(Value::as_str)
        .expect("chain field");
    assert_eq!(chain, supervisor.chain_id());

    let transactions = manifest
        .get("transactions")
        .and_then(Value::as_array)
        .expect("transactions array");
    let topology = transactions
        .iter()
        .filter_map(|tx| tx.get("topology").and_then(Value::as_array))
        .find(|entries| !entries.is_empty())
        .expect("non-empty topology transaction present");

    let actual_peer_ids: Vec<PeerId> = topology
        .iter()
        .map(|entry| {
            let decoded: GenesisTopologyEntry =
                norito::json::from_value(entry.clone()).expect("topology entry should decode");
            decoded.peer
        })
        .collect();
    let expected_peer_ids: Vec<PeerId> = supervisor
        .peers()
        .iter()
        .map(|peer| peer.peer_id())
        .collect();
    assert_eq!(
        actual_peer_ids, expected_peer_ids,
        "snapshot genesis should preserve topology"
    );

    let peers_root = snapshot_root.join("peers");
    let mut seen_ports = HashSet::new();
    let mut genesis_files = HashSet::new();

    for peer in supervisor.peers() {
        let config_path = peers_root.join(peer.alias()).join("config.toml");
        let contents = fs::read_to_string(&config_path).expect("read snapshot config");
        let value: toml::Table = toml::from_str(&contents).expect("valid toml");

        let torii_addr = value
            .get("torii")
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get("address"))
            .and_then(toml::Value::as_str)
            .expect("torii address");
        let network_addr = value
            .get("network")
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get("address"))
            .and_then(toml::Value::as_str)
            .expect("network address");
        for addr in [torii_addr, network_addr] {
            let body = norito::literal::parse("addr", addr).expect("valid addr literal");
            let port = body
                .parse::<iroha_primitives::addr::SocketAddr>()
                .map(|socket| socket.port())
                .expect("address contains a port");
            assert!(
                seen_ports.insert(port),
                "port {port} should be unique across exported configs"
            );
        }

        let genesis_file = value
            .get("genesis")
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get("file"))
            .and_then(toml::Value::as_str)
            .expect("genesis file path");
        genesis_files.insert(genesis_file.to_owned());

        let nexus_table = value
            .get("nexus")
            .and_then(toml::Value::as_table)
            .expect("nexus config");
        assert_eq!(
            nexus_table.get("enabled").and_then(toml::Value::as_bool),
            Some(true)
        );
        assert_eq!(
            nexus_table
                .get("lane_count")
                .and_then(toml::Value::as_integer),
            Some(2)
        );
        let lane_catalog = nexus_table
            .get("lane_catalog")
            .and_then(toml::Value::as_array)
            .expect("lane catalog");
        assert_eq!(lane_catalog.len(), 2);
        let lane0 = lane_catalog[0].as_table().expect("lane0 table");
        assert_eq!(
            lane0.get("alias").and_then(toml::Value::as_str),
            Some("core")
        );
        assert_eq!(
            lane0.get("index").and_then(toml::Value::as_integer),
            Some(0)
        );
        let lane1 = lane_catalog[1].as_table().expect("lane1 table");
        assert_eq!(
            lane1.get("alias").and_then(toml::Value::as_str),
            Some("governance")
        );
        assert_eq!(
            lane1.get("index").and_then(toml::Value::as_integer),
            Some(1)
        );
        let dataspace_catalog = nexus_table
            .get("dataspace_catalog")
            .and_then(toml::Value::as_array)
            .expect("dataspace catalog");
        assert_eq!(dataspace_catalog.len(), 1);
        let dataspace = dataspace_catalog[0].as_table().expect("dataspace table");
        assert_eq!(
            dataspace.get("alias").and_then(toml::Value::as_str),
            Some("universal")
        );
        assert_eq!(
            dataspace.get("id").and_then(toml::Value::as_integer),
            Some(0)
        );
    }

    assert_eq!(
        genesis_files.len(),
        1,
        "all peers should share the same genesis manifest path"
    );
}

#[test]
fn restore_snapshot_replaces_only_mutable_runtime_state() {
    if !ports_available("restore_snapshot_replaces_only_mutable_runtime_state") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");

    let peer = &supervisor.peers()[0];
    let storage_dir = peer.storage_dir().to_path_buf();
    let snapshot_dir = peer.snapshot_dir().to_path_buf();
    let config_path = peer.config_path().to_path_buf();
    let log_path = peer.log_path().to_path_buf();
    let genesis_path = supervisor.genesis_manifest().to_path_buf();
    let genesis_block_path = supervisor.genesis_block_file().to_path_buf();

    fs::write(storage_dir.join("marker.txt"), b"snapshot-data").expect("write storage marker");
    fs::write(snapshot_dir.join("inner.txt"), b"snapshot-inner").expect("write snapshot file");
    let original_config = fs::read(&config_path).expect("read original config");
    if let Some(parent) = log_path.parent() {
        fs::create_dir_all(parent).expect("create log directory");
    }
    fs::write(&log_path, b"snapshot-log").expect("write log file");

    let snapshot_root = supervisor
        .export_snapshot(Some("Restore Demo 2026"))
        .expect("export snapshot");

    fs::write(storage_dir.join("marker.txt"), b"mutated-storage")
        .expect("overwrite storage marker");
    fs::remove_file(snapshot_dir.join("inner.txt")).expect("remove snapshot file");
    fs::write(&log_path, b"mutated-log").expect("mutate log");

    supervisor
        .restore_snapshot(&snapshot_root)
        .expect("restore snapshot by path");

    assert_eq!(
        fs::read(storage_dir.join("marker.txt")).expect("read storage marker after restore"),
        b"snapshot-data"
    );
    assert!(
        snapshot_dir.join("inner.txt").exists(),
        "snapshot directory should be restored"
    );
    assert_eq!(
        fs::read(&config_path).expect("read restored config"),
        original_config
    );
    assert_eq!(
        fs::read(&log_path).expect("read restored log"),
        b"snapshot-log"
    );
    assert_eq!(
        fs::read(&genesis_path).expect("read restored genesis"),
        fs::read(snapshot_root.join("genesis").join(GENESIS_FILE_NAME))
            .expect("read snapshot genesis")
    );
    assert_eq!(
        fs::read(&genesis_block_path).expect("read restored signed genesis"),
        fs::read(snapshot_root.join("genesis").join(GENESIS_SIGNED_FILE_NAME))
            .expect("read snapshot signed genesis")
    );

    let snapshot_name = snapshot_root
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    fs::write(storage_dir.join("marker.txt"), b"mutated-again").expect("mutate storage");
    supervisor
        .restore_snapshot(snapshot_name.as_str())
        .expect("restore snapshot by label");
    assert_eq!(
        fs::read(storage_dir.join("marker.txt")).expect("read storage marker"),
        b"snapshot-data"
    );
}

#[test]
fn restore_snapshot_rejects_genesis_hash_mismatch() {
    if !ports_available("restore_snapshot_rejects_genesis_hash_mismatch") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");

    let snapshot_root = supervisor
        .export_snapshot(Some("Genesis Hash Mismatch"))
        .expect("export snapshot");

    fs::write(
        snapshot_root.join("genesis").join(GENESIS_FILE_NAME),
        b"mutated-genesis",
    )
    .expect("mutate snapshot genesis");

    let err = supervisor
        .restore_snapshot(&snapshot_root)
        .expect_err("restore should fail when genesis hash mismatches");
    match err {
        SupervisorError::Config(message) => assert!(
            message.contains("genesis hash"),
            "expected genesis hash mismatch message, got `{message}`"
        ),
        other => panic!("expected SupervisorError::Config, got {other:?}"),
    }
}

#[test]
fn restore_snapshot_rejects_tampered_signed_genesis_before_storage_mutation() {
    if !ports_available("restore_snapshot_rejects_tampered_signed_genesis_before_storage_mutation")
    {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");
    let live_sentinel = supervisor.peers()[0]
        .storage_dir()
        .join("signed-genesis-live-sentinel.bin");
    fs::write(&live_sentinel, b"snapshot-state").expect("write snapshot state");
    let snapshot_root = supervisor
        .export_snapshot(Some("Signed Genesis Tamper"))
        .expect("export snapshot");
    fs::write(&live_sentinel, b"live-state").expect("write live state");
    fs::write(
        snapshot_root.join("genesis").join(GENESIS_SIGNED_FILE_NAME),
        b"tampered-signed-genesis",
    )
    .expect("tamper snapshot signed genesis");

    let error = supervisor
        .restore_snapshot(&snapshot_root)
        .expect_err("tampered signed genesis must fail before restore");
    match error {
        SupervisorError::Config(message) => assert!(
            message.contains("signed genesis") && message.contains("byte-for-byte"),
            "unexpected signed-genesis rejection: {message}"
        ),
        other => panic!("expected SupervisorError::Config, got {other:?}"),
    }
    assert_eq!(
        fs::read(&live_sentinel).expect("read live sentinel"),
        b"live-state",
        "signed-genesis rejection must precede mutable storage replacement"
    );
}

#[test]
fn restore_snapshot_rejects_tampered_peer_config_before_storage_mutation() {
    if !ports_available("restore_snapshot_rejects_tampered_peer_config_before_storage_mutation") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");
    let alias = supervisor.peers()[0].alias().to_owned();
    let live_sentinel = supervisor.peers()[0]
        .storage_dir()
        .join("config-live-sentinel.bin");
    fs::write(&live_sentinel, b"snapshot-state").expect("write snapshot state");
    let snapshot_root = supervisor
        .export_snapshot(Some("Peer Config Tamper"))
        .expect("export snapshot");
    fs::write(&live_sentinel, b"live-state").expect("write live state");
    fs::write(
        snapshot_root.join("peers").join(&alias).join("config.toml"),
        b"chain = \"tampered\"\n",
    )
    .expect("tamper snapshot peer config");

    let error = supervisor
        .restore_snapshot(&snapshot_root)
        .expect_err("tampered peer config must fail before restore");
    match error {
        SupervisorError::Config(message) => assert!(
            message.contains(&format!("config for peer `{alias}`"))
                && message.contains("byte-for-byte"),
            "unexpected peer-config rejection: {message}"
        ),
        other => panic!("expected SupervisorError::Config, got {other:?}"),
    }
    assert_eq!(
        fs::read(&live_sentinel).expect("read live sentinel"),
        b"live-state",
        "peer-config rejection must precede mutable storage replacement"
    );
}

#[test]
fn restore_snapshot_rejects_kura_hash_tampering() {
    if !ports_available("restore_snapshot_rejects_kura_hash_tampering") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");

    let snapshot_root = supervisor
        .export_snapshot(Some("Kura Tamper"))
        .expect("export snapshot");
    let alias = supervisor.peers()[0].alias().to_owned();
    let storage_copy = snapshot_root.join("peers").join(&alias).join("storage");
    fs::write(storage_copy.join("tamper.bin"), b"tampered").expect("mutate snapshot storage");

    let err = supervisor
        .restore_snapshot(&snapshot_root)
        .expect_err("restore should fail when kura hash mismatches metadata");
    match err {
        SupervisorError::Config(message) => assert!(
            message.contains("integrity check") && message.contains(&alias),
            "expected kura integrity error mentioning alias; got `{message}`"
        ),
        other => panic!("expected SupervisorError::Config, got {other:?}"),
    }
}

#[test]
fn restore_snapshot_rejects_chain_mismatch() {
    if !ports_available("restore_snapshot_rejects_chain_mismatch") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");

    let snapshot_root = supervisor
        .export_snapshot(Some("Chain Mismatch"))
        .expect("export snapshot");
    let metadata_path = snapshot_root.join("metadata.json");
    let mut metadata: Value =
        norito::json::from_slice(&fs::read(&metadata_path).expect("read metadata"))
            .expect("parse metadata");
    metadata
        .as_object_mut()
        .expect("metadata should be an object")
        .insert("chain_id".into(), Value::String("other-chain".into()));
    fs::write(
        &metadata_path,
        json::to_vec_pretty(&metadata).expect("serialize metadata"),
    )
    .expect("write mutated metadata");

    fs::write(
        supervisor.peers()[0].storage_dir().join("marker.txt"),
        b"mutated",
    )
    .expect("mutate storage");

    let err = supervisor
        .restore_snapshot(&snapshot_root)
        .expect_err("restore should fail when chains mismatch");
    match err {
        SupervisorError::Config(message) => assert!(
            message.contains("other-chain"),
            "expected chain mismatch message, got `{message}`"
        ),
        other => panic!("expected SupervisorError::Config, got {other:?}"),
    }
}

#[test]
fn restore_snapshot_rejects_missing_storage_layout() {
    if !ports_available("restore_snapshot_rejects_missing_storage_layout") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");
    let snapshot_root = supervisor
        .export_snapshot(Some("Legacy Storage Layout"))
        .expect("export snapshot");
    let metadata_path = snapshot_root.join("metadata.json");
    let mut metadata: Value = json::from_slice(&fs::read(&metadata_path).expect("read metadata"))
        .expect("parse metadata");
    metadata
        .as_object_mut()
        .expect("metadata object")
        .remove("storage_layout");
    fs::write(
        &metadata_path,
        json::to_vec_pretty(&metadata).expect("serialize metadata"),
    )
    .expect("write metadata");
    let live_sentinel = supervisor.peers()[0]
        .storage_dir()
        .join("live-layout-sentinel.bin");
    fs::write(&live_sentinel, b"live-state").expect("write live sentinel");

    let err = supervisor
        .restore_snapshot(&snapshot_root)
        .expect_err("unversioned storage layout must fail closed");
    match err {
        SupervisorError::Config(message) => assert!(
            message.contains("missing `storage_layout`")
                && message.contains("cannot be restored safely"),
            "unexpected error: {message}"
        ),
        other => panic!("expected SupervisorError::Config, got {other:?}"),
    }
    assert_eq!(
        fs::read(live_sentinel).expect("read live sentinel"),
        b"live-state",
        "layout rejection must happen before live storage is mutated"
    );
}

#[test]
fn restore_snapshot_rejects_unknown_storage_layout() {
    if !ports_available("restore_snapshot_rejects_unknown_storage_layout") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");
    let snapshot_root = supervisor
        .export_snapshot(Some("Unknown Storage Layout"))
        .expect("export snapshot");
    let metadata_path = snapshot_root.join("metadata.json");
    let mut metadata: Value = json::from_slice(&fs::read(&metadata_path).expect("read metadata"))
        .expect("parse metadata");
    metadata.as_object_mut().expect("metadata object").insert(
        "storage_layout".into(),
        Value::String("future-layout-v99".into()),
    );
    fs::write(
        &metadata_path,
        json::to_vec_pretty(&metadata).expect("serialize metadata"),
    )
    .expect("write metadata");
    let live_sentinel = supervisor.peers()[0]
        .storage_dir()
        .join("live-layout-sentinel.bin");
    fs::write(&live_sentinel, b"live-state").expect("write live sentinel");

    let err = supervisor
        .restore_snapshot(&snapshot_root)
        .expect_err("unknown storage layout must fail closed");
    match err {
        SupervisorError::Config(message) => assert!(
            message.contains("unsupported storage layout `future-layout-v99`")
                && message.contains(SNAPSHOT_STORAGE_LAYOUT),
            "unexpected error: {message}"
        ),
        other => panic!("expected SupervisorError::Config, got {other:?}"),
    }
    assert_eq!(
        fs::read(live_sentinel).expect("read live sentinel"),
        b"live-state",
        "layout rejection must happen before live storage is mutated"
    );
}

#[cfg(unix)]
#[test]
fn start_rejects_intermediate_storage_generations_symlink_before_spawn() {
    if !ports_available("start_rejects_intermediate_storage_generations_symlink_before_spawn") {
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
    let attacker = temp.path().join("attacker-start-storage");
    let sentinel = redirect_storage_generations_through_symlink(&supervisor.peers()[0], &attacker);

    let error = supervisor
        .start_all()
        .expect_err("intermediate storage symlink must fail closed");
    assert!(matches!(error, SupervisorError::GenerationValidation(_)));
    assert!(!supervisor.is_any_running(), "no peer may be spawned");
    assert_eq!(
        fs::read(sentinel).expect("read sentinel"),
        b"must-not-touch"
    );
}

#[cfg(unix)]
#[test]
fn start_rejects_managed_kura_symlink_before_spawn() {
    if !ports_available("start_rejects_managed_kura_symlink_before_spawn") {
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
    let attacker = temp.path().join("attacker-kura");
    fs::create_dir(&attacker).expect("create attacker directory");
    let sentinel = attacker.join("sentinel");
    fs::write(&sentinel, b"must-not-touch").expect("write attacker sentinel");
    symlink(&attacker, supervisor.peers()[0].storage_dir().join("kura"))
        .expect("redirect managed Kura directory");

    let error = supervisor
        .start_all()
        .expect_err("managed Kura symlink must fail closed");
    assert!(matches!(error, SupervisorError::GenerationValidation(_)));
    assert!(!supervisor.is_any_running(), "no peer may be spawned");
    assert_eq!(
        fs::read(sentinel).expect("read sentinel"),
        b"must-not-touch"
    );
}

#[cfg(unix)]
#[test]
fn export_rejects_intermediate_storage_generations_symlink_before_copy() {
    if !ports_available("export_rejects_intermediate_storage_generations_symlink_before_copy") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _kagami = KagamiStub::install(temp.path());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");
    let attacker = temp.path().join("attacker-export-storage");
    let sentinel = redirect_storage_generations_through_symlink(&supervisor.peers()[0], &attacker);

    let error = supervisor
        .export_snapshot(Some("Must Not Exist"))
        .expect_err("intermediate storage symlink must fail closed");
    assert!(matches!(error, SupervisorError::GenerationValidation(_)));
    assert!(
        !supervisor
            .paths()
            .snapshots_dir()
            .join("must-not-exist")
            .exists()
    );
    assert_eq!(
        fs::read(sentinel).expect("read sentinel"),
        b"must-not-touch"
    );
}

#[cfg(unix)]
#[test]
fn restore_rejects_intermediate_storage_generations_symlink_before_wipe() {
    if !ports_available("restore_rejects_intermediate_storage_generations_symlink_before_wipe") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _kagami = KagamiStub::install(temp.path());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::SinglePeer)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");
    let snapshot = supervisor
        .export_snapshot(Some("Symlink Restore"))
        .expect("export baseline snapshot");
    let attacker = temp.path().join("attacker-restore-storage");
    let sentinel = redirect_storage_generations_through_symlink(&supervisor.peers()[0], &attacker);

    let error = supervisor
        .restore_snapshot(&snapshot)
        .expect_err("intermediate storage symlink must fail before wipe");
    assert!(matches!(error, SupervisorError::GenerationValidation(_)));
    assert_eq!(
        fs::read(sentinel).expect("read sentinel"),
        b"must-not-touch"
    );
}
