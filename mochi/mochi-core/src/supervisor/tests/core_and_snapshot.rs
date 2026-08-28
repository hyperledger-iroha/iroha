struct EnvVarGuard {
    key: &'static str,
}

#[test]
fn generated_genesis_record_reader_requires_exact_lf_framing() {
    let temp = tempfile::tempdir().expect("tempdir");
    let record = temp.path().join("record");
    fs::write(&record, b"value\n").expect("write exact record");
    assert_eq!(
        read_generated_genesis_record(&record, "test record").expect("read exact record"),
        "value\n"
    );

    fs::write(&record, b"value\r\n").expect("write CRLF record");
    assert_eq!(
        read_generated_genesis_record(&record, "test record")
            .expect_err("CRLF must fail closed")
            .kind(),
        ErrorKind::InvalidData
    );

    fs::write(&record, b"value").expect("write unterminated record");
    assert_eq!(
        read_generated_genesis_record(&record, "test record")
            .expect_err("unterminated record must fail closed")
            .kind(),
        ErrorKind::InvalidData
    );
}

#[test]
fn generated_genesis_record_reader_rejects_oversized_and_non_regular_inputs() {
    let temp = tempfile::tempdir().expect("tempdir");
    let oversized = temp.path().join("oversized");
    fs::write(
        &oversized,
        vec![b'a'; GENERATED_GENESIS_RECORD_MAX_BYTES_V1 + 1],
    )
    .expect("write oversized record");
    assert_eq!(
        read_generated_genesis_record(&oversized, "test record")
            .expect_err("oversized record must fail closed")
            .kind(),
        ErrorKind::InvalidData
    );

    let directory = temp.path().join("directory");
    fs::create_dir(&directory).expect("create directory");
    assert_eq!(
        read_generated_genesis_record(&directory, "test record")
            .expect_err("directory must fail closed")
            .kind(),
        ErrorKind::InvalidData
    );

    #[cfg(unix)]
    {
        let target = temp.path().join("target");
        let link = temp.path().join("link");
        fs::write(&target, b"value\n").expect("write symlink target");
        symlink(&target, &link).expect("create record symlink");
        assert_eq!(
            read_generated_genesis_record(&link, "test record")
                .expect_err("symlink must fail closed")
                .kind(),
            ErrorKind::InvalidData
        );
    }
}

#[test]
#[cfg(unix)]
fn generated_genesis_record_reader_rejects_raced_symlinks_and_fifos() {
    let temp = tempfile::tempdir().expect("tempdir");
    for replacement in ["symlink", "fifo"] {
        let path = temp.path().join(format!("record-{replacement}"));
        fs::write(&path, b"value\n").expect("write admitted record");
        let target = temp.path().join(format!("target-{replacement}"));
        fs::write(&target, b"replacement\n").expect("write replacement target");
        read_generated_genesis_record_inner(&path, "test record", || {
            fs::remove_file(&path).expect("remove admitted record path");
            if replacement == "symlink" {
                symlink(&target, &path).expect("install raced record symlink");
            } else {
                let result = Command::new("mkfifo")
                    .arg(&path)
                    .status()
                    .expect("run mkfifo");
                assert!(result.success(), "mkfifo failed");
            }
        })
        .expect_err("raced non-regular record path must fail closed");
    }
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
fn write_executable_stub(root: &Path, name: &str) -> PathBuf {
    let script_path = root.join(format!("{name}.sh"));
    fs::write(&script_path, "#!/bin/sh\nexit 0\n").expect("write executable stub");
    #[cfg(unix)]
    {
        let mut perms = fs::metadata(&script_path)
            .expect("executable stub metadata")
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).expect("set executable stub permissions");
    }
    script_path
}
#[cfg(unix)]
fn write_long_running_irohad_stub(root: &Path) -> PathBuf {
    let script_path = root.join("iroha3d-long-running-stub.sh");
    let script = r#"#!/bin/sh
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
  *)
    BIN="$TARGET/debug/unknown"
    ;;
esac
/bin/cat > "$BIN" <<'EOF'
#!/bin/sh
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
fn copy_dir_recursive_rejects_missing_and_unsupported_sources() {
    let temp = tempfile::tempdir().expect("tempdir");
    let missing = temp.path().join("missing");
    let dest = temp.path().join("out");
    assert_eq!(
        copy_dir_recursive(&missing, &dest)
            .expect_err("missing snapshot sources must fail")
            .kind(),
        ErrorKind::NotFound
    );
    assert!(!dest.exists());

    #[cfg(unix)]
    {
        let source = temp.path().join("source");
        fs::create_dir(&source).expect("create source");
        let outside = temp.path().join("outside");
        fs::write(&outside, b"outside").expect("write outside file");
        symlink(&outside, source.join("linked")) .expect("create source symlink");
        let destination = temp.path().join("rejected");
        assert_eq!(
            copy_dir_recursive(&source, &destination)
                .expect_err("symlinked snapshot entries must fail")
                .kind(),
            ErrorKind::InvalidData
        );
        assert!(!destination.exists(), "failed copies must be cleaned");
    }
}
#[test]
fn copy_dir_recursive_enforces_injected_tree_limits_and_cleans_partial_output() {
    let temp = tempfile::tempdir().expect("tempdir");
    let source = temp.path().join("source");
    fs::create_dir(&source).expect("create source");
    for name in ["a", "b", "c"] {
        fs::write(source.join(name), name).expect("write source file");
    }
    let nested = source.join("nested");
    fs::create_dir(&nested).expect("create nested source");
    fs::write(nested.join("value"), b"nested").expect("write nested source file");

    for (label, max_depth, directory_limit, tree_limit) in [
        ("depth", 0, 8, 8),
        ("directory", 8, 2, 8),
        ("tree", 8, 8, 2),
    ] {
        let destination = temp.path().join(format!("destination-{label}"));
        let mut remaining = tree_limit;
        let error = copy_dir_recursive_with_limits(
            &source,
            &destination,
            0,
            max_depth,
            directory_limit,
            &mut remaining,
        )
        .expect_err("an injected traversal limit must stop the copy");
        assert_eq!(error.kind(), ErrorKind::InvalidData);
        assert!(
            !destination.exists(),
            "a failed {label}-limited copy must remove its partial destination"
        );
    }
}
const KAGAMI_STUB_NPOS_PARAMETERS: &str = r#","parameters":{"custom":{"sumeragi_npos_parameters":{"id":"sumeragi_npos_parameters","payload":{"activation_lag_blocks":1,"epoch_length_blocks":3600,"epoch_seed":"4D5FC075E21E35B005F84FB9A3810B339776C77DC1027BEA6A14CB2D300C9AC9","evidence_horizon_blocks":7200,"finality_margin_blocks":8,"max_entity_correlation_pct":25,"max_nominator_concentration_pct":25,"max_validators":31,"min_nomination_bond":"1","min_self_bond":"1000","seat_band_pct":5,"slashing_delay_blocks":259200}}}}"#;

fn kagami_stub_manifest_json(
    chain_id: &str,
    consensus_mode: &str,
    npos_parameters: &str,
    consensus_fingerprint: Option<&str>,
) -> String {
    let fingerprint = consensus_fingerprint
        .map(|value| format!(",\"consensus_fingerprint\":\"{value}\""))
        .unwrap_or_default();
    let chain_discriminant = iroha_data_model::account::address::chain_discriminant();
    format!(
        "{{\"chain\":\"{chain_id}\",\"chain_discriminant\":{chain_discriminant},\"ivm_dir\":\".\",\"consensus_mode\":\"{consensus_mode}\",\"wire_protocol_version\":4{fingerprint},\"sumeragi_v2\":{{\"da_layout\":{{\"encoding\":{{\"encoding\":\"reed_solomon16\",\"details\":null}},\"chunk_size_bytes\":262144,\"data_shards\":4,\"parity_shards\":2,\"max_payload_size_bytes\":16777216,\"max_chunk_count\":1024}},\"nexus_amx_context_hash\":\"6611CDC66348BEBFBD583F888864A747DCC828C5FE84F58DFB0346CCA27ABAF3\",\"execution_policy_hash\":\"3F947453758F8EE90B2C66437A128FC22D93C4D2E0CA60C261D828B7E0B897C3\"}},\"transactions\":[{{\"instructions\":[]{npos_parameters}}}]}}"
    )
}

fn kagami_stub_consensus_fingerprint(consensus_mode: SumeragiConsensusMode) -> String {
    let (mode, npos_parameters) = match consensus_mode {
        SumeragiConsensusMode::Permissioned => ("Permissioned", ""),
        SumeragiConsensusMode::Npos => ("Npos", KAGAMI_STUB_NPOS_PARAMETERS),
    };
    let manifest: RawGenesisTransaction = norito::json::from_str(&kagami_stub_manifest_json(
        "mochi-stub-chain",
        mode,
        npos_parameters,
        None,
    ))
    .expect("decode Kagami stub manifest");
    manifest
        .with_consensus_meta()
        .consensus_fingerprint()
        .expect("Kagami stub consensus fingerprint")
        .to_string()
}

struct KagamiStub {
    _path_guard: EnvVarGuard,
    _log_guard: EnvVarGuard,
    _irohad_guard: EnvVarGuard,
    _signature_guard: EnvVarGuard,
    log_path: PathBuf,
}
impl KagamiStub {
    fn install(root: &Path) -> Self {
        let script_path = root.join("kagami_stub.sh");
        let manifest = kagami_stub_manifest_json(
            "$chain_id",
            "$consensus_mode",
            "$npos_parameters",
            Some("$consensus_fingerprint"),
        );
        let npos_parameters = KAGAMI_STUB_NPOS_PARAMETERS;
        let permissioned_fingerprint =
            kagami_stub_consensus_fingerprint(SumeragiConsensusMode::Permissioned);
        let npos_fingerprint = kagami_stub_consensus_fingerprint(SumeragiConsensusMode::Npos);
        let script = format!(
            r#"#!/bin/sh
if [ -n "$MOCHI_KAGAMI_LOG" ]; then
  printf 'args:%s\n' "$*" >> "$MOCHI_KAGAMI_LOG"
fi
case "$1" in
  verify)
    exit 0
    ;;
  genesis)
    case "$2" in
      generate)
        shift 2
        chain_id=
        consensus_mode=
        while [ "$#" -gt 0 ]; do
          case "$1" in
            --chain-id)
              chain_id="$2"
              shift 2
              ;;
            --consensus-mode)
              case "$2" in
                permissioned)
                  consensus_mode=Permissioned
                  npos_parameters=
                  consensus_fingerprint='{permissioned_fingerprint}'
                  ;;
                npos)
                  consensus_mode=Npos
                  npos_parameters='{npos_parameters}'
                  consensus_fingerprint='{npos_fingerprint}'
                  ;;
                *) exit 1 ;;
              esac
              shift 2
              ;;
            *)
              shift
              ;;
          esac
        done
        test -n "$chain_id"
        test -n "$consensus_mode"
        cat <<JSON
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
              echo "unsupported kagami genesis sign argument: $1" >&2
              exit 1
              ;;
          esac
        done
        test -s "$private_key_file"
        test -s "$config_file"
        config_mode="$(stat -f %Lp "$config_file" 2>/dev/null || stat -c %a "$config_file")"
        test "$config_mode" = "600"
        grep -F 'expected_hash = "REPLACE_WITH_GENESIS_EXPECTED_HASH"' "$config_file" >/dev/null
        printf 'stub-signed-genesis' > "$out_file"
        printf 'hash:0000000000000000000000000000000000000000000000000000000000000001#C50E\n' > "$expected_hash_out"
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
        let irohad_stub = write_executable_stub(root, "iroha3d-stub");
        let path_guard = EnvVarGuard::set("MOCHI_KAGAMI", script_path.as_os_str());
        let log_guard = EnvVarGuard::set("MOCHI_KAGAMI_LOG", log_path.as_os_str());
        let irohad_guard = EnvVarGuard::set("MOCHI_IROHAD", irohad_stub.as_os_str());
        let signature_guard = EnvVarGuard::set(
            TEST_FINALIZE_KAGAMI_STUB_SIGNATURE,
            std::ffi::OsStr::new("1"),
        );
        let _ = fs::File::create(&log_path);
        Self {
            _path_guard: path_guard,
            _log_guard: log_guard,
            _irohad_guard: irohad_guard,
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
    _signature_guard: EnvVarGuard,
}
impl StandaloneKagamiStub {
    fn create(root: &Path) -> Self {
        let script_path = root.join("kagami_override.sh");
        let log_path = root.join("kagami_override.log");
        let manifest = kagami_stub_manifest_json(
            "$chain_id",
            "$consensus_mode",
            "$npos_parameters",
            Some("$consensus_fingerprint"),
        );
        let npos_parameters = KAGAMI_STUB_NPOS_PARAMETERS;
        let permissioned_fingerprint =
            kagami_stub_consensus_fingerprint(SumeragiConsensusMode::Permissioned);
        let npos_fingerprint = kagami_stub_consensus_fingerprint(SumeragiConsensusMode::Npos);
        let script = format!(
            r#"#!/bin/sh
set -e
SCRIPT_DIR="$(CDPATH= cd -- "$(dirname "$0")" && pwd)"
printf '%s\n' "$@" >> "$SCRIPT_DIR/kagami_override.log"
case "$1" in
  verify)
    exit 0
    ;;
  genesis)
    case "$2" in
      generate)
        shift 2
        chain_id=
        consensus_mode=
        while [ "$#" -gt 0 ]; do
          case "$1" in
            --chain-id)
              chain_id="$2"
              shift 2
              ;;
            --consensus-mode)
              case "$2" in
                permissioned)
                  consensus_mode=Permissioned
                  npos_parameters=
                  consensus_fingerprint='{permissioned_fingerprint}'
                  ;;
                npos)
                  consensus_mode=Npos
                  npos_parameters='{npos_parameters}'
                  consensus_fingerprint='{npos_fingerprint}'
                  ;;
                *) exit 1 ;;
              esac
              shift 2
              ;;
            *)
              shift
              ;;
          esac
        done
        test -n "$chain_id"
        test -n "$consensus_mode"
        cat <<JSON
{manifest}
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
              echo "unsupported kagami genesis sign argument: $1" >&2
              exit 1
              ;;
          esac
        done
        test -s "$private_key_file"
        test -s "$config_file"
        grep -F 'expected_hash = "REPLACE_WITH_GENESIS_EXPECTED_HASH"' "$config_file" >/dev/null
        printf 'stub-signed-genesis' > "$out_file"
        printf 'hash:0000000000000000000000000000000000000000000000000000000000000001#C50E\n' > "$expected_hash_out"
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
        let irohad_stub = write_executable_stub(root, "kagami-override-iroha3d");
        let irohad_guard = EnvVarGuard::set("MOCHI_IROHAD", irohad_stub.as_os_str());
        let signature_guard = EnvVarGuard::set(
            TEST_FINALIZE_KAGAMI_STUB_SIGNATURE,
            std::ffi::OsStr::new("1"),
        );
        Self {
            script_path,
            log_path,
            _irohad_guard: irohad_guard,
            _signature_guard: signature_guard,
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
    fs::write(
        &expected_hash_path,
        format!("{}\n", NetworkId::from_genesis_hash(expected_hash)),
    )
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
    assert_eq!(binaries.irohad, override_path);
}
#[cfg(unix)]
#[test]
fn binary_paths_do_not_replace_missing_explicit_paths_from_path() {
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let path_dir = temp.path().join("path");
    fs::create_dir(&path_dir).expect("create PATH directory");
    for name in ["iroha3d", "kagami"] {
        let executable = path_dir.join(name);
        fs::write(&executable, "#!/bin/sh\nexit 0\n").expect("write PATH executable");
        let mut permissions = fs::metadata(&executable)
            .expect("PATH executable metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&executable, permissions).expect("set PATH executable permissions");
    }
    let _path_guard = RestoringEnvVarGuard::set("PATH", path_dir.as_os_str());
    let missing_dir = temp.path().join("missing");
    let missing_irohad = missing_dir.join("iroha3d");
    let missing_kagami = missing_dir.join("kagami");
    let mut binaries = BinaryPaths::default()
        .allow_auto_builds(false)
        .irohad(&missing_irohad)
        .kagami(&missing_kagami);

    let irohad_error = binaries
        .ensure_irohad_ready()
        .expect_err("an explicit iroha3d path must fail closed");
    assert!(
        irohad_error
            .to_string()
            .contains(&missing_irohad.display().to_string())
    );
    let kagami_error = binaries
        .ensure_kagami_ready()
        .expect_err("an explicit kagami path must fail closed");
    assert!(
        kagami_error
            .to_string()
            .contains(&missing_kagami.display().to_string())
    );
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
    binaries.irohad_build_attempted = false;
    binaries.irohad_auto = true;
    binaries.kagami = PathBuf::from("kagami");
    binaries.kagami_build_attempted = false;
    binaries.kagami_auto = true;
    binaries
        .ensure_irohad_ready()
        .expect("iroha3d auto-build should succeed");
    binaries
        .ensure_kagami_ready()
        .expect("Kagami auto-build should succeed");
    let log = fs::read_to_string(&cargo_log).expect("read cargo log");
    assert!(
        log.lines()
            .any(|line| line == "build -p irohad --bin iroha3d"),
        "daemon build must select only the iroha3d target: {log}"
    );
    assert_eq!(
        log.lines().count(),
        2,
        "expected one cargo invocation per binary build"
    );
    binaries
        .ensure_irohad_ready()
        .expect("resolved iroha3d should remain ready");
    binaries
        .ensure_kagami_ready()
        .expect("resolved Kagami should remain ready");
    let log = fs::read_to_string(&cargo_log).expect("read cargo log");
    assert_eq!(
        log.lines().count(),
        2,
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
    binaries.irohad_build_attempted = false;
    binaries.irohad_auto = true;
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
    let genesis = value
        .get("genesis")
        .and_then(toml::Value::as_table)
        .expect("generated genesis config");
    assert_eq!(
        genesis
            .get("expected_hash_file")
            .and_then(toml::Value::as_str),
        Some(
            supervisor
                .genesis
                .expected_hash_path
                .to_string_lossy()
                .as_ref()
        )
    );
    assert!(!genesis.contains_key("expected_hash"));
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
        SupervisorBuilder::with_profile(profile).profile_preset(ProfilePreset::FourPeerBft);
    assert_eq!(builder.profile().preset, Some(ProfilePreset::FourPeerBft));
    assert_eq!(builder.profile().topology.peer_count, 4);
    assert_eq!(
        builder.profile().consensus_mode,
        SumeragiConsensusMode::Npos
    );
}
#[test]
fn genesis_profile_and_explicit_chain_are_order_independent() {
    let expected_chain = GenesisProfile::Iroha3Dev.defaults().chain_id;
    let chain_then_profile = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
        .chain_id(expected_chain)
        .genesis_profile(GenesisProfile::Iroha3Dev);
    let profile_then_chain = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
        .genesis_profile(GenesisProfile::Iroha3Dev)
        .chain_id(expected_chain);
    assert_eq!(chain_then_profile.chain_id, expected_chain);
    assert_eq!(profile_then_chain.chain_id, expected_chain);

    let temp = tempfile::tempdir().expect("tempdir");
    for (name, builder) in [
        (
            "chain-then-profile",
            SupervisorBuilder::new(ProfilePreset::FourPeerBft)
                .chain_id("different.local")
                .genesis_profile(GenesisProfile::Iroha3Dev),
        ),
        (
            "profile-then-chain",
            SupervisorBuilder::new(ProfilePreset::FourPeerBft)
                .genesis_profile(GenesisProfile::Iroha3Dev)
                .chain_id("different.local"),
        ),
    ] {
        let data_root = temp.path().join(name);
        let error = builder
            .data_root(&data_root)
            .build()
            .expect_err("a profile/chain mismatch must fail");
        assert!(
            error.to_string().contains("requires chain id"),
            "unexpected error: {error}"
        );
        assert!(
            !data_root.exists(),
            "invalid inputs must not create the data root"
        );
    }
}
#[test]
fn invalid_first_release_inputs_fail_before_creating_the_data_root() {
    let temp = tempfile::tempdir().expect("tempdir");
    let valid_seed = "ab".repeat(32);

    let mut zero_queues = toml::Table::new();
    zero_queues.insert("body_bytes".to_owned(), toml::Value::Integer(0));
    let mut invalid_sumeragi = toml::Table::new();
    invalid_sumeragi.insert("queues".to_owned(), toml::Value::Table(zero_queues));

    let mut managed_onboarding = toml::Table::new();
    managed_onboarding.insert(
        "account_onboarding".to_owned(),
        toml::Value::Table(toml::Table::new()),
    );

    let mut disabled_mcp = toml::Table::new();
    disabled_mcp.insert("enabled".to_owned(), toml::Value::Boolean(false));
    let mut invalid_mcp = toml::Table::new();
    invalid_mcp.insert("mcp".to_owned(), toml::Value::Table(disabled_mcp));

    let mut lane_without_metadata = toml::Table::new();
    lane_without_metadata.insert("index".to_owned(), toml::Value::Integer(0));
    let mut invalid_nexus = toml::Table::new();
    invalid_nexus.insert(
        "lane_catalog".to_owned(),
        toml::Value::Array(vec![toml::Value::Table(lane_without_metadata)]),
    );

    let cases = [
        (
            "invalid-chain",
            SupervisorBuilder::new(ProfilePreset::FourPeerBft).chain_id(""),
            "invalid chain id",
        ),
        (
            "missing-required-seed",
            SupervisorBuilder::new(ProfilePreset::FourPeerBft)
                .genesis_profile(GenesisProfile::Iroha3Taira),
            "requires a 32-byte hexadecimal VRF seed",
        ),
        (
            "short-seed",
            SupervisorBuilder::new(ProfilePreset::FourPeerBft)
                .genesis_profile(GenesisProfile::Iroha3Dev)
                .vrf_seed_hex("ab"),
            "exactly 32 hexadecimal bytes",
        ),
        (
            "seed-without-profile",
            SupervisorBuilder::new(ProfilePreset::FourPeerBft).vrf_seed_hex(valid_seed),
            "requires a genesis profile",
        ),
        (
            "zero-queue-capacity",
            SupervisorBuilder::new(ProfilePreset::FourPeerBft)
                .sumeragi_config(invalid_sumeragi),
            "must be a positive integer",
        ),
        (
            "managed-onboarding-override",
            SupervisorBuilder::new(ProfilePreset::FourPeerBft)
                .torii_config(managed_onboarding),
            "account_onboarding is managed by Mochi",
        ),
        (
            "disabled-mcp",
            SupervisorBuilder::new(ProfilePreset::FourPeerBft).torii_config(invalid_mcp),
            "torii.mcp.enabled must be true",
        ),
        (
            "lane-without-metadata",
            SupervisorBuilder::new(ProfilePreset::FourPeerBft).nexus_config(invalid_nexus),
            "lane_catalog[0].metadata must be an explicit table",
        ),
    ];
    for (name, builder, expected_error) in cases {
        let data_root = temp.path().join(name);
        let error = builder
            .data_root(&data_root)
            .build()
            .expect_err("invalid first-release input must fail");
        assert!(
            error.to_string().contains(expected_error),
            "unexpected error for {name}: {error}"
        );
        assert!(
            !data_root.exists(),
            "invalid inputs must not create `{}`",
            data_root.display()
        );
    }
}
#[test]
fn kagami_manifest_chain_must_be_present_canonical_and_requested() {
    let expected = GenesisProfile::Iroha3Taira.defaults().chain_id;
    let uppercase = expected.to_ascii_uppercase();
    validate_kagami_manifest_chain(&norito::json!({"chain": expected}), expected)
        .expect("canonical requested chain must pass");

    for value in [
        norito::json!({}),
        norito::json!({"chain": "different.local"}),
        norito::json!({"chain": uppercase}),
    ] {
        validate_kagami_manifest_chain(&value, expected)
            .expect_err("missing, mismatched, or non-canonical Kagami chain must fail");
    }
}
#[test]
fn toml_secret_zeroizer_clears_nested_strings() {
    let mut nested = toml::Table::new();
    nested.insert(
        "private_key".to_owned(),
        toml::Value::String("nested-secret".to_owned()),
    );
    let mut root = toml::Table::new();
    root.insert(
        "values".to_owned(),
        toml::Value::Array(vec![
            toml::Value::String("array-secret".to_owned()),
            toml::Value::Table(nested),
        ]),
    );

    zeroize_toml_table(&mut root);

    let values = root
        .get("values")
        .and_then(toml::Value::as_array)
        .expect("array remains structured");
    assert_eq!(values[0].as_str(), Some(""));
    assert_eq!(
        values[1]
            .as_table()
            .and_then(|table| table.get("private_key"))
            .and_then(toml::Value::as_str),
        Some("")
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
    let supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
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
    let supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
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
fn generated_genesis_uses_first_release_block_cadence() {
    if !ports_available("generated_genesis_uses_first_release_block_cadence") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let preset = ProfilePreset::FourPeerBft;
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
        1_000,
        "{} must sign the first-release local cadence",
        preset.slug()
    );
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
    let error = match SupervisorBuilder::new(ProfilePreset::FourPeerBft)
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
    let seed = "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20";
    SupervisorBuilder::new(ProfilePreset::FourPeerBft)
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
    let supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
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
    let supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
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
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
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
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
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
    let mut lane0 = toml::Table::new();
    lane0.insert("alias".into(), toml::Value::String("core".into()));
    lane0.insert("index".into(), toml::Value::Integer(0));
    lane0.insert("dataspace".into(), toml::Value::String("universal".into()));
    lane0.insert("metadata".into(), toml::Value::Table(toml::Table::new()));
    let mut lane1 = toml::Table::new();
    lane1.insert("alias".into(), toml::Value::String("governance".into()));
    lane1.insert("index".into(), toml::Value::Integer(1));
    lane1.insert("dataspace".into(), toml::Value::String("universal".into()));
    lane1.insert("metadata".into(), toml::Value::Table(toml::Table::new()));
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
        assert!(!nexus_table.contains_key("enabled"));
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
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
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
#[cfg(unix)]
#[test]
fn restore_snapshot_swap_failure_rolls_back_every_peer_and_running_state() {
    if !ports_available("restore_snapshot_swap_failure_rolls_back_every_peer_and_running_state") {
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
    for (index, peer) in supervisor.peers().iter().enumerate() {
        fs::write(
            peer.storage_dir().join("restore-state.bin"),
            format!("snapshot-{index}"),
        )
        .expect("write snapshot state");
    }
    let snapshot = supervisor
        .export_snapshot(Some("Transactional Restore"))
        .expect("export snapshot");
    for (index, peer) in supervisor.peers().iter().enumerate() {
        fs::write(
            peer.storage_dir().join("restore-state.bin"),
            format!("live-{index}"),
        )
        .expect("write live state");
    }
    supervisor.start_peer("peer0").expect("start peer0");
    supervisor.start_peer("peer2").expect("start peer2");
    for peer in supervisor.peers() {
        use std::io::Write as _;
        let mut log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(peer.log_path())
            .expect("open live log");
        writeln!(log, "live-log-sentinel-{}", peer.alias()).expect("append live log sentinel");
    }

    let error = supervisor
        .restore_snapshot_with_swap_hook(&snapshot, |installed| {
            if installed == 1 {
                Err(SupervisorError::Config(
                    "injected restore swap failure".to_owned(),
                ))
            } else {
                Ok(())
            }
        })
        .expect_err("injected swap failure must abort restore");
    assert!(error.to_string().contains("injected restore swap failure"));
    assert_eq!(
        supervisor.running_peer_aliases(),
        vec!["peer0".to_owned(), "peer2".to_owned()]
    );
    for (index, peer) in supervisor.peers().iter().enumerate() {
        assert_eq!(
            fs::read_to_string(peer.storage_dir().join("restore-state.bin"))
                .expect("read rolled-back live state"),
            format!("live-{index}")
        );
        assert!(
            fs::read_to_string(peer.log_path())
                .expect("read rolled-back live log")
                .contains(&format!("live-log-sentinel-{}", peer.alias()))
        );
        let storage_parent = peer.storage_dir().parent().expect("storage parent");
        assert!(
            fs::read_dir(storage_parent)
                .expect("read storage parent")
                .all(|entry| !entry
                    .expect("storage parent entry")
                    .file_name()
                    .to_string_lossy()
                    .contains(".mochi-restore-")),
            "restore transaction artifacts must be cleaned"
        );
    }
}
#[cfg(unix)]
#[test]
fn restore_snapshot_restart_failure_rolls_back_before_restarting_original_peers() {
    if !ports_available(
        "restore_snapshot_restart_failure_rolls_back_before_restarting_original_peers",
    ) {
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
    for (index, peer) in supervisor.peers().iter().enumerate() {
        fs::write(
            peer.storage_dir().join("restart-state.bin"),
            format!("snapshot-{index}"),
        )
        .expect("write snapshot state");
    }
    let snapshot = supervisor
        .export_snapshot(Some("Restart Rollback"))
        .expect("export snapshot");
    for (index, peer) in supervisor.peers().iter().enumerate() {
        fs::write(
            peer.storage_dir().join("restart-state.bin"),
            format!("live-{index}"),
        )
        .expect("write original live state");
    }
    supervisor.start_peer("peer0").expect("start peer0");
    supervisor.start_peer("peer2").expect("start peer2");
    for peer in supervisor.peers() {
        use std::io::Write as _;
        let mut log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(peer.log_path())
            .expect("open original live log");
        writeln!(log, "original-log-sentinel-{}", peer.alias())
            .expect("append original log sentinel");
    }

    let error = supervisor
        .restore_snapshot_with_restart_hook(&snapshot, |supervisor, aliases| {
            supervisor.restore_captured_running_peers(&aliases[..1])?;
            Err(SupervisorError::Config(
                "injected restored-peer restart failure".to_owned(),
            ))
        })
        .expect_err("restart failure must abort and roll back restore");

    assert!(
        error
            .to_string()
            .contains("injected restored-peer restart failure")
    );
    assert_eq!(
        supervisor.running_peer_aliases(),
        vec!["peer0".to_owned(), "peer2".to_owned()]
    );
    for (index, peer) in supervisor.peers().iter().enumerate() {
        assert_eq!(
            fs::read_to_string(peer.storage_dir().join("restart-state.bin"))
                .expect("read original state after rollback"),
            format!("live-{index}")
        );
        assert!(
            fs::read_to_string(peer.log_path())
                .expect("read original log after rollback")
                .contains(&format!("original-log-sentinel-{}", peer.alias()))
        );
    }
    supervisor.stop_all().expect("stop restored original peers");
}
#[cfg(unix)]
#[test]
fn restore_snapshot_post_spawn_exit_rolls_back_before_commit() {
    if !ports_available("restore_snapshot_post_spawn_exit_rolls_back_before_commit") {
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
    for (index, peer) in supervisor.peers().iter().enumerate() {
        fs::write(
            peer.storage_dir().join("survival-state.bin"),
            format!("snapshot-{index}"),
        )
        .expect("write snapshot state");
    }
    let snapshot = supervisor
        .export_snapshot(Some("Post Spawn Exit"))
        .expect("export snapshot");
    for (index, peer) in supervisor.peers().iter().enumerate() {
        fs::write(
            peer.storage_dir().join("survival-state.bin"),
            format!("live-{index}"),
        )
        .expect("write original live state");
    }
    supervisor.start_peer("peer0").expect("start peer0");
    supervisor.start_peer("peer2").expect("start peer2");

    let error = supervisor
        .restore_snapshot_with_restart_hook(&snapshot, |supervisor, aliases| {
            supervisor.restore_captured_running_peers(aliases)?;
            let peer = supervisor
                .peers
                .iter_mut()
                .find(|peer| peer.alias() == "peer0")
                .expect("peer0 exists");
            peer.stop()?;
            peer.process = Some(
                Command::new("/usr/bin/false")
                    .spawn()
                    .expect("spawn immediately exiting restored peer"),
            );
            peer.state = PeerState::Running;
            Ok(())
        })
        .expect_err("an immediately exiting restored peer must abort the transaction");

    assert!(error.to_string().contains("post-spawn survival check"));
    assert_eq!(
        supervisor.running_peer_aliases(),
        vec!["peer0".to_owned(), "peer2".to_owned()]
    );
    for (index, peer) in supervisor.peers().iter().enumerate() {
        assert_eq!(
            fs::read_to_string(peer.storage_dir().join("survival-state.bin"))
                .expect("read original state after survival rollback"),
            format!("live-{index}")
        );
    }
    assert!(!
        supervisor
            .paths()
            .root()
            .join(SNAPSHOT_RESTORE_COMMIT_FILE_NAME)
            .exists()
    );
    supervisor.stop_all().expect("stop restored original peers");
}
#[cfg(unix)]
#[test]
fn restore_snapshot_commit_publication_uncertainty_preserves_restored_state_for_recovery() {
    if !ports_available(
        "restore_snapshot_commit_publication_uncertainty_preserves_restored_state_for_recovery",
    ) {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");
    for (index, peer) in supervisor.peers().iter().enumerate() {
        fs::write(
            peer.storage_dir().join("publication-state.bin"),
            format!("snapshot-{index}"),
        )
        .expect("write snapshot state");
    }
    let snapshot = supervisor
        .export_snapshot(Some("Commit Publication Uncertainty"))
        .expect("export snapshot");
    for (index, peer) in supervisor.peers().iter().enumerate() {
        fs::write(
            peer.storage_dir().join("publication-state.bin"),
            format!("live-{index}"),
        )
        .expect("write original live state");
    }
    let network_root = supervisor.paths().root().to_path_buf();

    let error = supervisor
        .restore_snapshot_with_commit_hook(&snapshot, |transaction| {
            write_restore_commit_marker_with(
                &transaction.commit_marker_path,
                &transaction.network_root,
                |_| Err(io::Error::other("injected commit marker removal failure")),
                |_| Err(io::Error::other("injected commit marker directory sync failure")),
            )
        })
        .expect_err("ambiguous commit marker publication must fail closed");

    assert!(error.to_string().contains("publication"));
    assert!(error.to_string().contains("uncertain"));
    assert!(network_root.join(SNAPSHOT_RESTORE_JOURNAL_FILE_NAME).is_file());
    assert!(network_root.join(SNAPSHOT_RESTORE_COMMIT_FILE_NAME).is_file());
    for (index, peer) in supervisor.peers().iter().enumerate() {
        assert_eq!(
            fs::read_to_string(peer.storage_dir().join("publication-state.bin"))
                .expect("read retained restored state"),
            format!("snapshot-{index}")
        );
        let backup = fs::read_dir(peer.storage_dir().parent().expect("storage parent"))
            .expect("read storage parent")
            .map(|entry| entry.expect("storage entry").path())
            .find(|path| {
                path.file_name()
                    .and_then(OsStr::to_str)
                    .is_some_and(|name| name.contains(".mochi-restore-backup-storage."))
            })
            .expect("original storage backup retained");
        assert_eq!(
            fs::read_to_string(backup.join("publication-state.bin"))
                .expect("read retained original backup"),
            format!("live-{index}")
        );
    }

    recover_snapshot_restore_if_needed(&network_root)
        .expect("startup recovery commits a surviving valid marker");
    for (index, peer) in supervisor.peers().iter().enumerate() {
        assert_eq!(
            fs::read_to_string(peer.storage_dir().join("publication-state.bin"))
                .expect("read recovered restored state"),
            format!("snapshot-{index}")
        );
    }
    assert!(!network_root.join(SNAPSHOT_RESTORE_JOURNAL_FILE_NAME).exists());
    assert!(!network_root.join(SNAPSHOT_RESTORE_COMMIT_FILE_NAME).exists());
}
fn snapshot_restore_recovery_fixture(network_root: &Path) -> SnapshotRestoreTransaction {
    snapshot_restore_recovery_fixture_with_generation(
        network_root,
        "0123456789abcdef0123456789abcdef",
    )
}
fn snapshot_restore_recovery_fixture_with_generation(
    network_root: &Path,
    generation_id: &str,
) -> SnapshotRestoreTransaction {
    let storage_parent = network_root
        .join("peers")
        .join("peer0")
        .join("storage-generations");
    let logs = network_root.join("logs");
    fs::create_dir_all(&storage_parent).expect("create storage parent");
    fs::create_dir_all(&logs).expect("create log parent");
    let live_storage = storage_parent.join(generation_id);
    let staged_storage = storage_parent.join(format!(
        ".{generation_id}.mochi-restore-staged-storage.test.1"
    ));
    let backup_storage = storage_parent.join(format!(
        ".{generation_id}.mochi-restore-backup-storage.test.1"
    ));
    fs::create_dir(&live_storage).expect("create original storage");
    fs::write(live_storage.join("state"), b"original").expect("write original state");
    fs::create_dir(&staged_storage).expect("create staged storage");
    fs::write(staged_storage.join("state"), b"restored").expect("write restored state");
    let live_log = logs.join("peer0.log");
    let staged_log = logs.join(".peer0.log.mochi-restore-staged-log.test.1");
    let backup_log = logs.join(".peer0.log.mochi-restore-backup-log.test.1");
    fs::write(&live_log, b"original-log").expect("write original log");
    fs::write(&staged_log, b"restored-log").expect("write staged log");
    let transaction = SnapshotRestoreTransaction {
        network_root: network_root.to_path_buf(),
        journal_path: network_root.join(SNAPSHOT_RESTORE_JOURNAL_FILE_NAME),
        commit_marker_path: network_root.join(SNAPSHOT_RESTORE_COMMIT_FILE_NAME),
        peers: vec![StagedPeerRestore {
            alias: "peer0".to_owned(),
            live_storage,
            staged_storage,
            backup_storage,
            live_log,
            staged_log: Some(staged_log),
            backup_log,
            original_log_present: true,
            log_touched: false,
            storage_backed_up: false,
            storage_installed: false,
            log_backed_up: false,
            log_installed: false,
        }],
        committed: false,
        preserve_backups: false,
    };
    write_pending_restore_journal(&transaction).expect("write pending restore journal");
    transaction
}
#[test]
fn pending_snapshot_restore_journal_recovers_each_swap_boundary() {
    for completed_swaps in 0..=2 {
        let temp = tempfile::tempdir().expect("tempdir");
        let transaction = snapshot_restore_recovery_fixture(temp.path());
        let peer = &transaction.peers[0];
        if completed_swaps >= 1 {
            fs::rename(&peer.live_storage, &peer.backup_storage)
                .expect("backup original storage");
            fs::rename(&peer.live_log, &peer.backup_log).expect("backup original log");
        }
        if completed_swaps == 2 {
            fs::rename(&peer.staged_storage, &peer.live_storage)
                .expect("install restored storage");
            fs::rename(peer.staged_log.as_ref().expect("staged log"), &peer.live_log)
                .expect("install restored log");
        }

        recover_snapshot_restore_if_needed(temp.path()).expect("recover pending restore");

        assert_eq!(
            fs::read(peer.live_storage.join("state")).expect("read recovered state"),
            b"original"
        );
        assert_eq!(
            fs::read(&peer.live_log).expect("read recovered log"),
            b"original-log"
        );
        assert!(!peer.staged_storage.exists());
        assert!(!peer.backup_storage.exists());
        assert!(!transaction.journal_path.exists());
    }
}
#[test]
fn committed_snapshot_restore_journal_keeps_restored_state_and_finishes_cleanup() {
    let temp = tempfile::tempdir().expect("tempdir");
    let transaction = snapshot_restore_recovery_fixture(temp.path());
    let peer = &transaction.peers[0];
    fs::rename(&peer.live_storage, &peer.backup_storage).expect("backup original storage");
    fs::rename(&peer.staged_storage, &peer.live_storage).expect("install restored storage");
    fs::rename(&peer.live_log, &peer.backup_log).expect("backup original log");
    fs::rename(peer.staged_log.as_ref().expect("staged log"), &peer.live_log)
        .expect("install restored log");
    write_restore_commit_marker(&transaction.commit_marker_path, temp.path())
        .expect("write commit marker");

    recover_snapshot_restore_if_needed(temp.path()).expect("finish committed restore cleanup");

    assert_eq!(
        fs::read(peer.live_storage.join("state")).expect("read committed state"),
        b"restored"
    );
    assert_eq!(
        fs::read(&peer.live_log).expect("read committed log"),
        b"restored-log"
    );
    assert!(!peer.backup_storage.exists());
    assert!(!peer.backup_log.exists());
    assert!(!transaction.journal_path.exists());
    assert!(!transaction.commit_marker_path.exists());
}
#[test]
fn committed_snapshot_restore_keeps_marker_when_journal_cleanup_fails() {
    let temp = tempfile::tempdir().expect("tempdir");
    let transaction = snapshot_restore_recovery_fixture(temp.path());
    write_restore_commit_marker(&transaction.commit_marker_path, temp.path())
        .expect("write commit marker");
    let mut attempts = Vec::new();

    transaction.remove_committed_journal_with(|path| {
        attempts.push(path.to_path_buf());
        if path == transaction.journal_path {
            Err(io::Error::other("injected journal cleanup failure"))
        } else {
            fs::remove_file(path)
        }
    });

    assert_eq!(
        attempts.as_slice(),
        std::slice::from_ref(&transaction.journal_path)
    );
    assert!(transaction.journal_path.exists());
    assert!(
        transaction.commit_marker_path.exists(),
        "a committed journal must never be left without its marker"
    );
}
#[test]
fn restore_commit_marker_durable_cleanup_is_unambiguously_precommit() {
    let temp = tempfile::tempdir().expect("tempdir");
    let marker = temp.path().join(SNAPSHOT_RESTORE_COMMIT_FILE_NAME);
    let mut sync_attempts = 0_usize;

    let error = write_restore_commit_marker_with(
        &marker,
        temp.path(),
        |path| fs::remove_file(path),
        |path| {
            sync_attempts += 1;
            if sync_attempts == 1 {
                Err(io::Error::other("injected publication sync failure"))
            } else {
                sync_managed_directory(path)
            }
        },
    )
    .expect_err("publication failure must be reported");

    assert!(matches!(
        error,
        SnapshotRestoreCommitFailure::NotPublished { .. }
    ));
    assert_eq!(sync_attempts, 2);
    assert!(!marker.exists());
}
#[test]
fn snapshot_restore_recovery_rejects_noncanonical_generation_id_before_mutation() {
    let temp = tempfile::tempdir().expect("tempdir");
    let transaction =
        snapshot_restore_recovery_fixture_with_generation(temp.path(), "generation-1");
    let peer = &transaction.peers[0];

    let error = recover_snapshot_restore_if_needed(temp.path())
        .expect_err("noncanonical live generation ids must fail closed");

    assert!(error.to_string().contains("invalid storage paths"));
    assert_eq!(
        fs::read(peer.live_storage.join("state")).expect("read untouched live state"),
        b"original"
    );
    assert_eq!(
        fs::read(peer.staged_storage.join("state")).expect("read untouched staged state"),
        b"restored"
    );
    assert!(transaction.journal_path.exists());
}
#[cfg(unix)]
#[test]
fn snapshot_restore_recovery_rejects_symlinked_storage_ancestor_before_mutation() {
    let temp = tempfile::tempdir().expect("tempdir");
    let transaction = snapshot_restore_recovery_fixture(temp.path());
    let peer = &transaction.peers[0];
    let storage_parent = peer
        .live_storage
        .parent()
        .expect("storage-generations parent");
    let escaped_storage = temp.path().join("escaped-storage-generations");
    fs::rename(storage_parent, &escaped_storage).expect("move storage outside managed hierarchy");
    symlink(&escaped_storage, storage_parent).expect("redirect storage ancestor");
    let escaped_staged = escaped_storage.join(
        peer.staged_storage
            .file_name()
            .expect("staged storage basename"),
    );

    let error = recover_snapshot_restore_if_needed(temp.path())
        .expect_err("symlinked recovery ancestors must fail closed");

    assert!(error.to_string().contains("storage-generations ancestor"));
    assert_eq!(
        fs::read(escaped_staged.join("state")).expect("read escaped staged sentinel"),
        b"restored"
    );
    assert!(transaction.journal_path.exists());
}
#[test]
fn restore_snapshot_rejects_genesis_hash_mismatch() {
    if !ports_available("restore_snapshot_rejects_genesis_hash_mismatch") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
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
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
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
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
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
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
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
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
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
fn restore_snapshot_rejects_unknown_metadata_and_extra_peer_hashes_before_mutation() {
    if !ports_available(
        "restore_snapshot_rejects_unknown_metadata_and_extra_peer_hashes_before_mutation",
    ) {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
        .data_root(temp.path())
        .build()
        .expect("build supervisor");
    let snapshot_root = supervisor
        .export_snapshot(Some("Strict Metadata"))
        .expect("export snapshot");
    let metadata_path = snapshot_root.join("metadata.json");
    let original: Value = json::from_slice(&fs::read(&metadata_path).expect("read metadata"))
        .expect("parse metadata");
    let live_sentinel = supervisor.peers()[0]
        .storage_dir()
        .join("strict-metadata-live-sentinel.bin");
    fs::write(&live_sentinel, b"live-state").expect("write live sentinel");

    let mut unknown = original.clone();
    unknown
        .as_object_mut()
        .expect("metadata object")
        .insert("legacy".into(), Value::Bool(true));
    fs::write(
        &metadata_path,
        json::to_vec_pretty(&unknown).expect("serialize unknown metadata"),
    )
    .expect("write unknown metadata");
    let error = supervisor
        .restore_snapshot(&snapshot_root)
        .expect_err("unknown V1 metadata field must fail closed");
    assert!(error.to_string().contains("unknown V1 field"), "{error}");
    assert_eq!(fs::read(&live_sentinel).expect("read sentinel"), b"live-state");

    let mut extra_alias = original;
    let hashes = extra_alias
        .as_object_mut()
        .and_then(|metadata| metadata.get_mut("kura_hashes"))
        .and_then(Value::as_object_mut)
        .expect("kura hashes");
    let hash = hashes.values().next().expect("peer hash").clone();
    hashes.insert("retired-peer".into(), hash);
    fs::write(
        &metadata_path,
        json::to_vec_pretty(&extra_alias).expect("serialize extra alias metadata"),
    )
    .expect("write extra alias metadata");
    let error = supervisor
        .restore_snapshot(&snapshot_root)
        .expect_err("extra V1 peer hash alias must fail closed");
    assert!(
        error.to_string().contains("exactly the managed peer aliases"),
        "{error}"
    );
    assert_eq!(fs::read(&live_sentinel).expect("read sentinel"), b"live-state");
}
#[test]
fn restore_snapshot_rejects_missing_storage_layout() {
    if !ports_available("restore_snapshot_rejects_missing_storage_layout") {
        return;
    }
    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let _stub = KagamiStub::install(temp.path());
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
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
            message.contains("missing required `storage_layout` string")
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
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
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
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
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
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
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
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
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
    let mut supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
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
