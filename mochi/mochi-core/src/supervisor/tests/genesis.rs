//! Genesis manifests, Kagami test doubles, and bootstrap contract tests.
use super::*;
use crate::genesis as sandbox_genesis;

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

fn kagami_stub_authority() -> KagemushaMintFinalityGenesisParametersV1 {
    let validators = (0x20_u8..0x24).map(|seed| {
        PeerId::new(
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("derive deterministic stub validator")
                .public_key()
                .clone(),
        )
    });
    sandbox_genesis::dev_sandbox_kagemusha_mint_finality_parameters(
        "mochi-stub-chain",
        "stub-template",
        validators,
    )
    .expect("derive canonical stub authority template")
}

const KAGAMI_STUB_EPOCH_SEED: [u8; 32] = [
    0x4d, 0x5f, 0xc0, 0x75, 0xe2, 0x1e, 0x35, 0xb0, 0x05, 0xf8, 0x4f, 0xb9, 0xa3, 0x81, 0x0b, 0x33,
    0x97, 0x76, 0xc7, 0x7d, 0xc1, 0x02, 0x7b, 0xea, 0x6a, 0x14, 0xcb, 0x2d, 0x30, 0x0c, 0x9a, 0xc9,
];

fn kagami_stub_manifest_template(consensus_mode: SumeragiConsensusMode) -> String {
    use iroha_data_model::{
        block::consensus_v2::SumeragiV2GenesisContextParameters,
        parameter::{Parameter, system::SumeragiNposParameters},
    };
    let authority = kagami_stub_authority();
    let chain_placeholder: ChainId = "mochi-stub-chain-placeholder"
        .parse()
        .expect("canonical stub chain placeholder");
    let chain_json =
        norito::json::to_json(&chain_placeholder).expect("serialize stub chain placeholder");
    let mut builder = iroha_genesis::GenesisBuilder::new_without_executor(chain_placeholder, ".")
        .with_sumeragi_v2_context_parameters(SumeragiV2GenesisContextParameters::recommended())
        .with_kagemusha_mint_finality_genesis_parameters(authority.clone());
    if consensus_mode == SumeragiConsensusMode::Npos {
        let parameters = SumeragiNposParameters {
            epoch_seed: KAGAMI_STUB_EPOCH_SEED,
            ..SumeragiNposParameters::default()
        };
        builder = builder.append_parameter(Parameter::Custom(parameters.into_custom_parameter()));
    }
    let manifest = builder
        .build_raw()
        .expect("build complete typed stub genesis")
        .with_consensus_mode(consensus_mode);
    manifest
        .validate_mode_specific_consensus_parameters()
        .expect("stub consensus parameters match their selected mode");
    let serialized = norito::json::to_json(&manifest).expect("serialize typed stub manifest");
    let authority_json = norito::json::to_json(&authority).expect("serialize stub authority");
    assert_eq!(serialized.matches(authority_json.as_str()).count(), 1);
    assert_eq!(serialized.matches(chain_json.as_str()).count(), 1);
    // Materialize the supplied chain and public parameters only after typed serialization.
    // The existing signing owner binds topology and recomputes consensus metadata.
    let serialized = serialized
        .replacen(authority_json.as_str(), "$kagemusha_mint_finality", 1)
        .replacen(chain_json.as_str(), r#""$chain_id""#, 1);
    if consensus_mode == SumeragiConsensusMode::Npos {
        let seed_json = norito::json::to_json(&KAGAMI_STUB_EPOCH_SEED)
            .expect("serialize the default stub epoch seed");
        let seed_field = format!(r#""epoch_seed":{seed_json}"#);
        assert_eq!(serialized.matches(seed_field.as_str()).count(), 1);
        serialized.replacen(seed_field.as_str(), r#""epoch_seed":"$vrf_seed_hex""#, 1)
    } else {
        serialized
    }
}

pub(super) struct KagamiStub {
    _path_guard: EnvVarGuard,
    _log_guard: EnvVarGuard,
    _irohad_guard: EnvVarGuard,
    _signature_guard: EnvVarGuard,
    log_path: PathBuf,
}
impl KagamiStub {
    pub(super) fn install(root: &Path) -> Self {
        let script_path = root.join("kagami_stub.sh");
        let permissioned_manifest =
            kagami_stub_manifest_template(SumeragiConsensusMode::Permissioned);
        let npos_manifest = kagami_stub_manifest_template(SumeragiConsensusMode::Npos);
        let default_seed_hex: String = KAGAMI_STUB_EPOCH_SEED
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
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
        vrf_seed_hex={default_seed_hex}
        kagemusha_mint_finality_file=
        while [ "$#" -gt 0 ]; do
          case "$1" in
            --chain-id)
              chain_id="$2"
              shift 2
              ;;
            --vrf-seed-hex)
              vrf_seed_hex="$2"
              shift 2
              ;;
            --kagemusha-mint-finality-parameters)
              kagemusha_mint_finality_file="$2"
              shift 2
              ;;
            --consensus-mode)
              case "$2" in
                permissioned)
                  consensus_mode=Permissioned
                  ;;
                npos)
                  consensus_mode=Npos
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
        test -n "$chain_id" || exit 1
        test -n "$consensus_mode" || exit 1
        test -s "$kagemusha_mint_finality_file" || exit 1
        kagemusha_mint_finality="$(cat "$kagemusha_mint_finality_file")" || exit 1
        case "$consensus_mode" in
          Permissioned)
            cat <<JSON
{permissioned_manifest}
JSON
            ;;
          Npos)
            cat <<JSON
{npos_manifest}
JSON
            ;;
        esac
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
    pub(super) fn log_path(&self) -> &Path {
        &self.log_path
    }
}
pub(super) struct StandaloneKagamiStub {
    script_path: PathBuf,
    log_path: PathBuf,
    _irohad_guard: EnvVarGuard,
    _signature_guard: EnvVarGuard,
}
impl StandaloneKagamiStub {
    pub(super) fn create(root: &Path) -> Self {
        let script_path = root.join("kagami_override.sh");
        let log_path = root.join("kagami_override.log");
        let permissioned_manifest =
            kagami_stub_manifest_template(SumeragiConsensusMode::Permissioned);
        let npos_manifest = kagami_stub_manifest_template(SumeragiConsensusMode::Npos);
        let default_seed_hex: String = KAGAMI_STUB_EPOCH_SEED
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
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
        vrf_seed_hex={default_seed_hex}
        kagemusha_mint_finality_file=
        while [ "$#" -gt 0 ]; do
          case "$1" in
            --chain-id)
              chain_id="$2"
              shift 2
              ;;
            --vrf-seed-hex)
              vrf_seed_hex="$2"
              shift 2
              ;;
            --kagemusha-mint-finality-parameters)
              kagemusha_mint_finality_file="$2"
              shift 2
              ;;
            --consensus-mode)
              case "$2" in
                permissioned)
                  consensus_mode=Permissioned
                  ;;
                npos)
                  consensus_mode=Npos
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
        test -n "$chain_id" || exit 1
        test -n "$consensus_mode" || exit 1
        test -s "$kagemusha_mint_finality_file" || exit 1
        kagemusha_mint_finality="$(cat "$kagemusha_mint_finality_file")" || exit 1
        case "$consensus_mode" in
          Permissioned)
            cat <<JSON
{permissioned_manifest}
JSON
            ;;
          Npos)
            cat <<JSON
{npos_manifest}
JSON
            ;;
        esac
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
    pub(super) fn script_path(&self) -> &Path {
        &self.script_path
    }
    pub(super) fn log_path(&self) -> &Path {
        &self.log_path
    }
}

#[test]
fn kagami_stubs_materialize_complete_manifests_with_exact_supplied_authority() {
    use iroha_data_model::{
        block::consensus_v2::SumeragiV2GenesisContextParameters,
        parameter::system::SumeragiNposParameters,
    };

    let _env = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("stub fixture directory");
    let placeholder = kagami_stub_authority();
    let authority = sandbox_genesis::dev_sandbox_kagemusha_mint_finality_parameters(
        "materialized-stub-chain",
        "supplied-generation",
        placeholder
            .epoch_roster
            .validators
            .iter()
            .map(|keys| keys.validator.clone()),
    )
    .expect("derive distinct supplied authority");
    assert_ne!(authority, placeholder);
    let authority_path = temp.path().join("supplied-authority.json");
    fs::write(
        &authority_path,
        norito::json::to_vec_pretty(&authority).expect("serialize supplied authority"),
    )
    .expect("write supplied authority");
    let check = |script_path: &Path| {
        for (argument, mode, seed) in [
            ("permissioned", SumeragiConsensusMode::Permissioned, None),
            ("npos", SumeragiConsensusMode::Npos, None),
            ("npos", SumeragiConsensusMode::Npos, Some([0x42_u8; 32])),
        ] {
            let mut command = Command::new(script_path);
            command.args(["genesis", "generate"]);
            if let Some(seed) = seed {
                let hex_seed: String = seed.iter().map(|byte| format!("{byte:02x}")).collect();
                command.args(["--vrf-seed-hex", &hex_seed]);
            }
            let output = command
                .args([
                    "--chain-id",
                    "materialized-stub-chain",
                    "--consensus-mode",
                    argument,
                    "--kagemusha-mint-finality-parameters",
                ])
                .arg(&authority_path)
                .output()
                .expect("run fixture generator");
            assert!(output.status.success(), "stub generator failed: {output:?}");
            let manifest: RawGenesisTransaction = norito::json::from_slice(&output.stdout)
                .expect("stub emits the complete current typed manifest");
            assert_eq!(manifest.chain_id().as_ref(), "materialized-stub-chain");
            assert_eq!(manifest.consensus_mode(), mode);
            assert_eq!(
                manifest.wire_protocol_version(),
                iroha_config::parameters::defaults::sumeragi::PROTOCOL_VERSION
            );
            assert_eq!(
                manifest.sumeragi_v2_context_parameters(),
                SumeragiV2GenesisContextParameters::recommended()
            );
            assert_eq!(
                manifest.kagemusha_mint_finality_genesis_parameters(),
                &authority,
                "the passed generation authority must replace the template roster"
            );
            manifest
                .validate_mode_specific_consensus_parameters()
                .expect("canonical mode-specific manifest");
            let parameters = manifest
                .effective_parameters()
                .expect("effective parameters");
            let npos = parameters
                .custom()
                .get(&SumeragiNposParameters::parameter_id())
                .and_then(SumeragiNposParameters::from_custom_parameter);
            if mode == SumeragiConsensusMode::Npos {
                assert_eq!(
                    npos.expect("typed NPoS parameters").epoch_seed(),
                    seed.unwrap_or(KAGAMI_STUB_EPOCH_SEED),
                    "the stub must materialize the exact supplied or default VRF seed"
                );
            } else {
                assert!(npos.is_none());
            }
            assert!(manifest.consensus_fingerprint().is_none());
            assert!(
                manifest
                    .with_consensus_meta()
                    .consensus_fingerprint()
                    .is_some(),
                "the signing owner can bind the materialized consensus parameters"
            );
            let mut missing: Value =
                norito::json::from_slice(&output.stdout).expect("complete JSON positive control");
            assert!(
                missing
                    .as_object_mut()
                    .expect("manifest object")
                    .remove("kagemusha_mint_finality")
                    .is_some()
            );
            assert!(matches!(
                norito::json::from_value::<RawGenesisTransaction>(missing),
                Err(norito::json::Error::MissingField { field })
                    if field == "kagemusha_mint_finality"
            ));
        }
        let missing = Command::new(script_path)
            .args([
                "genesis",
                "generate",
                "--chain-id",
                "materialized-stub-chain",
                "--consensus-mode",
                "permissioned",
            ])
            .output()
            .expect("run generator without authority");
        assert!(!missing.status.success());
        assert!(
            missing.stdout.is_empty(),
            "missing authority must not emit a manifest"
        );
    };
    {
        let _stub = KagamiStub::install(temp.path());
        let script = env::var_os("MOCHI_KAGAMI").expect("installed stub path");
        check(Path::new(&script));
    }
    {
        let stub = StandaloneKagamiStub::create(temp.path());
        check(stub.script_path());
    }
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
            SupervisorBuilder::new(ProfilePreset::FourPeerBft).sumeragi_config(invalid_sumeragi),
            "must be a positive integer",
        ),
        (
            "managed-onboarding-override",
            SupervisorBuilder::new(ProfilePreset::FourPeerBft).torii_config(managed_onboarding),
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
    let supervisor = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
        .data_root(temp.path())
        .genesis_profile(GenesisProfile::Iroha3Dev)
        .vrf_seed_hex(seed)
        .build()
        .expect("build supervisor");
    let manifest = RawGenesisTransaction::from_path(supervisor.genesis_manifest())
        .expect("load bound profiled manifest");
    let parameters = manifest
        .effective_parameters()
        .expect("effective parameters");
    let npos = parameters
        .custom()
        .get(&iroha_data_model::parameter::system::SumeragiNposParameters::parameter_id())
        .and_then(
            iroha_data_model::parameter::system::SumeragiNposParameters::from_custom_parameter,
        )
        .expect("profile requires typed NPoS parameters");
    assert_eq!(
        npos.epoch_seed(),
        std::array::from_fn(|index| u8::try_from(index + 1).expect("32 seed bytes fit u8")),
        "the forwarded seed must reach the persisted signed-genesis manifest"
    );
    let fingerprint = manifest
        .consensus_fingerprint()
        .expect("the signing owner must publish a bound manifest");
    assert_eq!(
        Some(fingerprint),
        manifest.with_consensus_meta().consensus_fingerprint(),
        "persisted metadata must bind the actual supplied seed"
    );
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
        .expect("persisted bound consensus fingerprint");
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
