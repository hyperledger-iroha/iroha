// Test body included from the parent module to keep its production source budget bounded.
use super::*;
use crate::{
    incentive_log::IncentiveLogError,
    incentives::{
        INCENTIVE_DEFAULT_ACTIVE_EPOCHS, INCENTIVE_DEFAULT_MEASUREMENTS_PER_EPOCH,
        INCENTIVE_MAX_ACTIVE_EPOCHS_V1, INCENTIVE_MAX_RETAINED_MEASUREMENTS_V1,
    },
    vpn::VpnOverlay,
};
use hex::FromHex;
use std::time::Duration;
use tempfile::NamedTempFile;
macro_rules! config_fixture {
    ($name:literal) => {
        concat!(
            include_str!(concat!("config_tests/fixtures/", $name)),
            "        "
        )
    };
}
fn write_config(json: &str) -> PathBuf {
    let file = NamedTempFile::new().expect("create temp file");
    std::fs::write(file.path(), json).expect("write config");
    file.into_temp_path().keep().expect("persist temp file")
}
fn write_manifest(json: &str) -> NamedTempFile {
    let file = NamedTempFile::new_in(std::env::current_dir().expect("current directory"))
        .expect("create manifest file");
    std::fs::write(file.path(), json).expect("write manifest");
    file
}
fn write_vpn_secret(byte: u8) -> NamedTempFile {
    let file = NamedTempFile::new_in(std::env::current_dir().expect("current directory"))
        .expect("create VPN secret file");
    std::fs::write(file.path(), hex::encode([byte; 32])).expect("write VPN secret file");
    file
}
fn vpn_config_with_secret(byte: u8) -> (VpnConfig, NamedTempFile) {
    let file = write_vpn_secret(byte);
    let config = VpnConfig {
        enabled: true,
        helper_ticket_secret_path: Some(file.path().to_path_buf()),
        backend_bootstrap_secret_path: Some(file.path().to_path_buf()),
        ..VpnConfig::default()
    };
    (config, file)
}
fn assert_config_json_admission_rejected(bytes: &[u8]) {
    let file = NamedTempFile::new().expect("create admission input");
    std::fs::write(file.path(), bytes).expect("write admission input");
    let error = RelayConfig::load(file.path()).expect_err("JSON admission must reject input");
    assert!(
        matches!(error, ConfigError::JsonAdmission(_)),
        "unexpected error: {error:?}"
    );
}
#[test]
fn relay_config_file_limit_accepts_exact_and_rejects_plus_one() {
    let exact = NamedTempFile::new().expect("create exact config");
    let mut valid = br#"{"mode":"Entry","listen":"127.0.0.1:0"}"#.to_vec();
    valid.resize(RELAY_CONFIG_JSON_MAX_BYTES_V1, b' ');
    std::fs::write(exact.path(), &valid).expect("write exact config");
    let loaded = RelayConfig::load(exact.path()).expect("exact-limit config must load");
    assert_eq!(loaded.mode, RelayMode::Entry);
    let plus_one = NamedTempFile::new().expect("create oversized config");
    plus_one
        .as_file()
        .set_len(
            u64::try_from(RELAY_CONFIG_JSON_MAX_BYTES_V1 + 1).expect("fixed config limit fits u64"),
        )
        .expect("size oversized config");
    let error = RelayConfig::load(plus_one.path()).expect_err("limit + 1 must fail");
    assert!(
        matches!(error, ConfigError::Io(ref source) if source.kind() == std::io::ErrorKind::InvalidData),
        "unexpected error: {error:?}"
    );
}
#[cfg(unix)]
#[test]
fn relay_config_rejects_symlink_input() {
    use std::os::unix::fs::symlink;
    let directory = tempfile::tempdir().expect("create temp directory");
    let target = directory.path().join("relay-target.json");
    let link = directory.path().join("relay-link.json");
    std::fs::write(&target, br#"{"mode":"Entry","listen":"127.0.0.1:0"}"#).expect("write target");
    symlink(&target, &link).expect("create symlink");
    let error = RelayConfig::load(&link).expect_err("symlink config must fail");
    assert!(
        matches!(error, ConfigError::Io(ref source) if source.kind() == std::io::ErrorKind::InvalidData),
        "unexpected error: {error:?}"
    );
}
#[cfg(unix)]
#[test]
fn relay_config_rejects_path_replacement_race() {
    let directory = tempfile::tempdir().expect("create temp directory");
    let configured = directory.path().join("relay.json");
    let replacement = directory.path().join("replacement.json");
    std::fs::write(&configured, br#"{"mode":"Entry","listen":"127.0.0.1:0"}"#)
        .expect("write configured file");
    std::fs::write(&replacement, br#"{"mode":"Middle","listen":"127.0.0.1:1"}"#)
        .expect("write replacement file");
    *BOUNDED_FILE_READ_REPLACEMENT
        .lock()
        .expect("race hook lock") = Some((configured.clone(), replacement));
    let error = RelayConfig::load(&configured).expect_err("path replacement must fail");
    assert!(
        matches!(error, ConfigError::Io(ref source) if source.kind() == std::io::ErrorKind::InvalidData),
        "unexpected error: {error:?}"
    );
}
#[cfg(unix)]
#[test]
fn private_file_reader_rejects_group_permissions_inside_the_identity_chain() {
    use std::os::unix::fs::PermissionsExt as _;
    let file = NamedTempFile::new_in(std::env::current_dir().expect("current directory"))
        .expect("create private input");
    std::fs::write(file.path(), b"private material").expect("write private input");
    std::fs::set_permissions(file.path(), std::fs::Permissions::from_mode(0o640))
        .expect("set unsafe private input permissions");
    let error = read_bounded_private_regular_file(file.path(), 64, "private test input")
        .expect_err("group-readable private input must fail in the bounded reader");
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
}
#[cfg(unix)]
#[test]
fn private_file_reader_rejects_unsafe_parent_custody() {
    use std::os::unix::fs::PermissionsExt as _;
    let directory = tempfile::Builder::new()
        .prefix("relay-unsafe-private-parent-")
        .tempdir_in(std::env::current_dir().expect("current directory"))
        .expect("create private input directory");
    let path = directory.path().join("secret");
    std::fs::write(&path, b"private material").expect("write private input");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
        .expect("protect private input");
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o777))
        .expect("make parent unsafe");
    let error = read_bounded_private_regular_file(&path, 64, "private test input")
        .expect_err("other-writable parent must fail");
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
}
#[cfg(unix)]
#[test]
fn private_file_ancestor_policy_accepts_root_sticky_boundary_only() {
    let effective_uid = 1_000;
    assert!(trusted_private_ancestor(
        effective_uid,
        0o700,
        effective_uid
    ));
    assert!(trusted_private_ancestor(0, 0o1777, effective_uid));
    assert!(!trusted_private_ancestor(2_000, 0o755, effective_uid));
    assert!(!trusted_private_ancestor(
        effective_uid,
        0o777,
        effective_uid
    ));
}
#[cfg(unix)]
#[test]
fn private_file_reader_rejects_hard_linked_secret() {
    let directory = tempfile::Builder::new()
        .prefix("relay-linked-private-file-")
        .tempdir_in(std::env::current_dir().expect("current directory"))
        .expect("create private input directory");
    let path = directory.path().join("secret");
    let alias = directory.path().join("secret-alias");
    std::fs::write(&path, b"private material").expect("write private input");
    std::fs::hard_link(&path, &alias).expect("create hard link");
    let error = read_bounded_private_regular_file(&path, 64, "private test input")
        .expect_err("hard-linked private input must fail");
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
}
#[cfg(not(unix))]
#[test]
fn private_file_reader_fails_closed_without_unix_custody_checks() {
    let error = trusted_private_file_path(Path::new("secret"), "private test input")
        .expect_err("platform without an equivalent private ACL policy must fail closed");
    assert_eq!(error.kind(), std::io::ErrorKind::Unsupported);
}
#[test]
fn manifest_json_string_scrubber_overwrites_nested_values() {
    let mut value = norito::json!({
        "secret": "identity-secret",
        "nested": ["kem-secret", { "public": "public-material" }],
        "number": 7,
    });
    clear_manifest_json_strings(&mut value);
    let object = value.as_object().expect("manifest object");
    assert_eq!(
        object["secret"].as_str(),
        Some("\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0")
    );
    let nested = object["nested"].as_array().expect("nested manifest array");
    assert_eq!(nested[0].as_str(), Some("\0\0\0\0\0\0\0\0\0\0"));
    assert!(
        nested[1].as_object().expect("nested object")["public"]
            .as_str()
            .expect("scrubbed public value")
            .bytes()
            .all(|byte| byte == 0)
    );
    assert_eq!(object["number"].as_u64(), Some(7));
}
#[test]
fn bounded_reader_sensitive_buffer_can_be_explicitly_cleared() {
    let mut buffer = SensitiveReadBuffer(vec![0xA5; 32]);
    buffer.clear();
    assert!(buffer.0.iter().all(|byte| *byte == 0));

    let mut probe = SensitiveReadProbe([0xA5]);
    probe.clear();
    assert_eq!(probe.0, [0]);
}
#[test]
fn relay_config_preflight_rejects_depth_count_and_string_budgets() {
    let mut deep = "[".repeat(RELAY_CONFIG_JSON_MAX_DEPTH_V1 + 1);
    deep.push('0');
    deep.push_str(&"]".repeat(RELAY_CONFIG_JSON_MAX_DEPTH_V1 + 1));
    assert_config_json_admission_rejected(deep.as_bytes());
    let too_many = format!(
        "[{}]",
        std::iter::repeat_n("0", RELAY_CONFIG_JSON_MAX_SEQUENCE_ELEMENTS_V1 + 1)
            .collect::<Vec<_>>()
            .join(",")
    );
    assert_config_json_admission_rejected(too_many.as_bytes());
    let oversized_string = format!(
        "\"{}\"",
        "a".repeat(RELAY_CONFIG_JSON_MAX_FIELD_BYTES_V1 + 1)
    );
    assert_config_json_admission_rejected(oversized_string.as_bytes());
    let aggregate_strings = format!(
        "[{}]",
        std::iter::repeat_n(
            format!("\"{}\"", "b".repeat(97)),
            RELAY_CONFIG_JSON_MAX_SEQUENCE_ELEMENTS_V1,
        )
        .collect::<Vec<_>>()
        .join(",")
    );
    assert!(aggregate_strings.len() < RELAY_CONFIG_JSON_MAX_BYTES_V1);
    assert_config_json_admission_rejected(aggregate_strings.as_bytes());
}
#[test]
fn descriptor_manifest_file_limit_accepts_exact_and_rejects_plus_one() {
    let exact = NamedTempFile::new_in(std::env::current_dir().expect("current directory"))
        .expect("create exact manifest");
    let mut manifest = format!(
        r#"{{"identity":{{"ed25519_private_key_hex":"{}"}}}}"#,
        "11".repeat(32)
    )
    .into_bytes();
    manifest.resize(DESCRIPTOR_MANIFEST_JSON_MAX_BYTES_V1, b' ');
    std::fs::write(exact.path(), manifest).expect("write exact manifest");
    let mut policy = HandshakePolicy {
        descriptor_manifest_path: Some(exact.path().to_path_buf()),
        ..HandshakePolicy::default()
    };
    assert_eq!(
        policy
            .identity_private_key_from_manifest()
            .expect("exact-limit manifest must load"),
        Some([0x11; 32])
    );
    let plus_one = NamedTempFile::new_in(std::env::current_dir().expect("current directory"))
        .expect("create oversized manifest");
    plus_one
        .as_file()
        .set_len(
            u64::try_from(DESCRIPTOR_MANIFEST_JSON_MAX_BYTES_V1 + 1)
                .expect("fixed manifest limit fits u64"),
        )
        .expect("size oversized manifest");
    policy.descriptor_manifest_path = Some(plus_one.path().to_path_buf());
    let error = policy
        .manifest_secrets()
        .expect_err("manifest limit + 1 must fail");
    assert!(
        matches!(error, ConfigError::DescriptorManifest { ref message, .. } if message.contains("first-release limit")),
        "unexpected error: {error:?}"
    );
}
#[test]
fn descriptor_manifest_preflight_bounds_recursive_lookup() {
    let mut deep = "[".repeat(DESCRIPTOR_MANIFEST_JSON_MAX_DEPTH_V1 + 1);
    deep.push_str("null");
    deep.push_str(&"]".repeat(DESCRIPTOR_MANIFEST_JSON_MAX_DEPTH_V1 + 1));
    let manifest = write_manifest(&deep);
    let policy = HandshakePolicy {
        descriptor_manifest_path: Some(manifest.path().to_path_buf()),
        ..HandshakePolicy::default()
    };
    let error = policy
        .manifest_secrets()
        .expect_err("deep manifest must fail before Value allocation");
    assert!(
        matches!(error, ConfigError::DescriptorManifest { ref message, .. } if message.contains("JSON admission failed")),
        "unexpected error: {error:?}"
    );
}
#[cfg(unix)]
#[test]
fn descriptor_manifest_requires_private_direct_file() {
    use std::os::unix::fs::{PermissionsExt as _, symlink};
    let directory = tempfile::Builder::new()
        .prefix("relay-private-manifest-")
        .tempdir_in(std::env::current_dir().expect("current directory"))
        .expect("create manifest directory");
    let target = directory.path().join("relay-manifest.json");
    let link = directory.path().join("relay-manifest.link");
    std::fs::write(
        &target,
        format!(
            r#"{{"identity":{{"ed25519_private_key_hex":"{}"}}}}"#,
            "11".repeat(32)
        ),
    )
    .expect("write manifest");
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o640))
        .expect("set group-readable permissions");
    let mut policy = HandshakePolicy {
        descriptor_manifest_path: Some(target.clone()),
        ..HandshakePolicy::default()
    };
    let error = policy
        .manifest_secrets()
        .expect_err("group-readable manifest must fail closed");
    assert!(
        matches!(error, ConfigError::DescriptorManifest { ref message, .. } if message.contains("group or other")),
        "unexpected error: {error:?}"
    );

    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o600))
        .expect("protect manifest");
    symlink(&target, &link).expect("create manifest symlink");
    policy.descriptor_manifest_path = Some(link);
    let error = policy
        .manifest_secrets()
        .expect_err("manifest symlink must fail closed");
    assert!(
        matches!(error, ConfigError::DescriptorManifest { ref message, .. } if message.contains("regular file")),
        "unexpected error: {error:?}"
    );
}
#[test]
fn certificate_bundle_protocol_limit_accepts_exact_and_rejects_plus_one() {
    let exact = NamedTempFile::new().expect("create exact bundle");
    exact
        .as_file()
        .set_len(u64::try_from(SRC_V2_MAX_BUNDLE_BYTES).expect("SRCv2 limit fits u64"))
        .expect("size exact bundle");
    let certificate = CertificateConfig {
        bundle_path: exact.path().to_path_buf(),
        issuer_ed25519_hex: "11".repeat(32),
        issuer_mldsa_hex: "22".repeat(MlDsaSuite::MlDsa65.public_key_len()),
    };
    let exact_error = certificate
        .load_bundle()
        .expect_err("zero-filled exact bundle is not CBOR");
    assert!(
        matches!(exact_error, ConfigError::Certificate { ref message, .. } if message.contains("failed to parse")),
        "exact limit should reach the parser: {exact_error:?}"
    );
    let plus_one = NamedTempFile::new().expect("create oversized bundle");
    plus_one
        .as_file()
        .set_len(u64::try_from(SRC_V2_MAX_BUNDLE_BYTES + 1).expect("SRCv2 limit + 1 fits u64"))
        .expect("size oversized bundle");
    let oversized = CertificateConfig {
        bundle_path: plus_one.path().to_path_buf(),
        ..certificate
    };
    let error = oversized
        .load_bundle()
        .expect_err("bundle limit + 1 must fail");
    assert!(
        matches!(error, ConfigError::Certificate { ref message, .. } if message.contains("first-release limit")),
        "unexpected error: {error:?}"
    );
}
#[test]
fn certificate_issuer_fields_are_length_checked_before_hex_decode() {
    let mut certificate = CertificateConfig {
        bundle_path: PathBuf::from("unused.cbor"),
        issuer_ed25519_hex: "11".repeat(32),
        issuer_mldsa_hex: "22".repeat(MlDsaSuite::MlDsa65.public_key_len()),
    };
    certificate
        .validate()
        .expect("exact protocol key lengths must validate");
    certificate.issuer_ed25519_hex.push_str("00");
    let error = certificate
        .validate()
        .expect_err("Ed25519 hex length + 1 byte must fail");
    assert!(
        matches!(error, ConfigError::Handshake(ref message) if message.contains("issuer_ed25519_hex") && message.contains("exactly")),
        "unexpected error: {error:?}"
    );
    certificate.issuer_ed25519_hex.truncate(64);
    certificate.issuer_mldsa_hex.push_str("00");
    let error = certificate
        .validate()
        .expect_err("ML-DSA hex length + 1 byte must fail");
    assert!(
        matches!(error, ConfigError::Handshake(ref message) if message.contains("issuer_mldsa_hex") && message.contains("exactly")),
        "unexpected error: {error:?}"
    );
}
#[test]
fn handshake_validation_enforces_producer_collection_limit() {
    let mut policy = HandshakePolicy {
        grease: std::iter::repeat_with(|| GreasePolicyEntry {
            typ: 0x7F10,
            value_hex: String::new(),
        })
        .take(RELAY_CONFIG_JSON_MAX_SEQUENCE_ELEMENTS_V1)
        .collect(),
        ..HandshakePolicy::default()
    };
    let error = policy
        .validate()
        .expect_err("wire semantics must reject an aggregate above the capability limit");
    assert!(
        matches!(error, ConfigError::Handshake(ref message) if message.contains("capability vector")),
        "unexpected error: {error:?}"
    );
    policy.grease.push(GreasePolicyEntry {
        typ: 0x7F10,
        value_hex: String::new(),
    });
    let error = policy
        .validate()
        .expect_err("producer list limit + 1 must fail");
    assert!(
        matches!(error, ConfigError::Handshake(ref message) if message.contains("first-release limit")),
        "unexpected error: {error:?}"
    );
}
#[test]
fn handshake_validation_bounds_worst_case_relay_capability_vector() {
    // The canonical worst-case v1 response occupies 73 bytes before GREASE:
    // KEM, signature, descriptor, role, padding, constant-rate, and two suites.
    // One GREASE TLV adds a four-byte header, leaving 4,019 value bytes.
    let mut policy = HandshakePolicy {
        grease: vec![GreasePolicyEntry {
            typ: 0x7F10,
            value_hex: "aa".repeat(4_019),
        }],
        ..HandshakePolicy::default()
    };
    policy
        .validate()
        .expect("capability vector at the exact wire limit must validate");

    policy.grease[0].value_hex.push_str("aa");
    let error = policy
        .validate()
        .expect_err("capability vector one byte above the wire limit must fail");
    assert!(
        matches!(error, ConfigError::Handshake(ref message)
            if message.contains("4097 bytes") && message.contains("4096 bytes")),
        "unexpected error: {error:?}"
    );
}
#[test]
fn manifest_with_ml_kem_keys_loads() {
    let private_hex = "aa".repeat(ML_KEM_768_SECRET_LEN);
    let public_hex = "bb".repeat(ML_KEM_768_PUBLIC_LEN);
    let manifest_json = format!(
        r#"{{
                "version": 1,
                "identity": {{
                    "ed25519_private_key_hex": "{seed}",
                    "ml_kem_private_key_hex": "{private_hex}",
                    "ml_kem_public_hex": "{public_hex}"
                }}
            }}"#,
        seed = "11".repeat(32),
        private_hex = private_hex,
        public_hex = public_hex,
    );
    let manifest = write_manifest(&manifest_json);
    let manifest_path = manifest.path().display().to_string();
    let manifest_json_path = format!("{manifest_path:?}");
    let config_json = format!(
        r#"{{
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "handshake": {{ "descriptor_manifest_path": {manifest_json_path} }}
            }}"#
    );
    let config_path = write_config(&config_json);
    let cfg = RelayConfig::load(config_path).expect("load config");
    let policy = cfg.handshake_policy();
    let secrets = policy
        .manifest_secrets()
        .expect("manifest secrets")
        .expect("secrets expected");
    assert!(secrets.identity_private_key.is_some());
    assert_eq!(
        secrets.ml_kem_private_key.as_ref().map(Vec::len),
        Some(ML_KEM_768_SECRET_LEN)
    );
    assert_eq!(
        secrets.ml_kem_public_key.as_ref().map(Vec::len),
        Some(ML_KEM_768_PUBLIC_LEN)
    );
    let ml_kem = policy
        .ml_kem_keys_from_manifest()
        .expect("ml-kem keys")
        .expect("ml-kem keypair expected");
    assert_eq!(ml_kem.private.len(), ML_KEM_768_SECRET_LEN);
    assert_eq!(ml_kem.public.len(), ML_KEM_768_PUBLIC_LEN);
}
#[test]
fn manifest_requires_complete_ml_kem_pair() {
    let manifest_json = format!(
        r#"{{
                "version": 1,
                "identity": {{
                    "ed25519_private_key_hex": "{seed}",
                    "ml_kem_public_hex": "{public_hex}"
                }}
            }}"#,
        seed = "22".repeat(32),
        public_hex = "cc".repeat(ML_KEM_768_PUBLIC_LEN),
    );
    let manifest = write_manifest(&manifest_json);
    let manifest_path = manifest.path().display().to_string();
    let manifest_json_path = format!("{manifest_path:?}");
    let config_json = format!(
        r#"{{
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "handshake": {{ "descriptor_manifest_path": {manifest_json_path} }}
            }}"#
    );
    let config_path = write_config(&config_json);
    let cfg = RelayConfig::load(config_path).expect("load config");
    let policy = cfg.handshake_policy();
    let err = policy
        .ml_kem_keys_from_manifest()
        .expect_err("expected manifest error");
    match err {
        ConfigError::DescriptorManifest { message, .. } => {
            assert!(
                message.contains("ml_kem_private_key_hex") && message.contains("ml_kem_public_hex"),
                "unexpected error message: {message}"
            );
        }
        other => panic!("unexpected error {other:?}"),
    }
}
#[test]
fn manifest_secret_debug_is_redacted_and_private_material_can_be_cleared() {
    let mut ml_kem = MlKemKeys {
        public: vec![1, 2, 3],
        private: vec![222; 4],
    };
    let rendered = format!("{ml_kem:?}");
    assert!(rendered.contains("<redacted>"));
    assert!(!rendered.contains("222"));
    ml_kem.clear_private_material();
    assert_eq!(ml_kem.private, vec![0; 4]);

    let mut secrets = ManifestSecrets {
        identity_private_key: Some([171; 32]),
        ml_kem_private_key: Some(vec![205; 4]),
        ml_kem_public_key: Some(vec![7; 5]),
    };
    let rendered = format!("{secrets:?}");
    assert!(rendered.contains("<redacted>"));
    assert!(!rendered.contains("171"));
    assert!(!rendered.contains("205"));
    secrets.clear_private_material();
    assert_eq!(secrets.identity_private_key, Some([0; 32]));
    assert_eq!(secrets.ml_kem_private_key, Some(vec![0; 4]));
    assert_eq!(secrets.ml_kem_public_key, Some(vec![7; 5]));
}
#[test]
fn load_self_signed_config() {
    let json = config_fixture!("self_signed.json");
    let path = write_config(json);
    let cfg = RelayConfig::load(path).expect("load config");
    assert_eq!(cfg.mode, RelayMode::Entry);
    assert_eq!(cfg.listen_addr().unwrap().port(), 0);
    assert!(cfg.pow_config().required);
    assert_eq!(cfg.pow_config().difficulty, 18);
    assert_eq!(cfg.pow_config().max_future_skew_secs, 300);
    assert_eq!(cfg.pow_config().min_ticket_ttl_secs, 30);
    assert_eq!(cfg.self_signed_subject(), DEFAULT_SELF_SIGNED_SUBJECT);
    assert_eq!(cfg.padding_config().cell_size, 1024);
    assert_eq!(cfg.padding_config().max_idle_millis, 150);
    assert_eq!(
        cfg.padding_config().global_rate_limit_bytes_per_sec,
        PaddingConfig::default_global_rate_limit_bytes_per_sec()
    );
    assert_eq!(
        cfg.padding_config().burst_bytes,
        PaddingConfig::default_burst_bytes()
    );
    assert_eq!(cfg.congestion_config().max_circuits_per_client, 8);
    assert_eq!(cfg.congestion_config().max_active_circuits, 4_096);
    assert_eq!(cfg.congestion_config().handshake_cooldown_millis, 200);
    assert!(!cfg.compliance_config().enable);
    assert_eq!(cfg.compliance_config().max_log_bytes, 64 * 1024 * 1024);
    assert_eq!(cfg.compliance_config().max_backup_files, 5);
    assert!(cfg.compliance_config().pipeline_spool_dir().is_none());
    assert!(!cfg.incentive_log_config().enable);
    assert!(cfg.incentive_log_config().spool_dir.is_none());
    assert_eq!(
        cfg.incentive_log_config().max_active_epochs,
        INCENTIVE_DEFAULT_ACTIVE_EPOCHS
    );
    assert_eq!(
        cfg.incentive_log_config().max_measurements_per_epoch,
        INCENTIVE_DEFAULT_MEASUREMENTS_PER_EPOCH
    );
}
fn assert_vpn_config_error(config_json: &str, expected: &str) {
    let path = write_config(config_json);
    let error = RelayConfig::load(path).expect_err("incomplete VPN trust must fail closed");
    assert!(
        matches!(error, ConfigError::Vpn(ref message) if message.contains(expected)),
        "expected VPN error containing {expected:?}, got {error:?}"
    );
}
#[test]
fn vpn_requires_exit_role_and_persistent_transport_trust() {
    let entry = r#"{
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "vpn": {
                    "enabled": true,
                    "helper_ticket_secret_path": "/run/secrets/vpn-helper-ticket.hex",
                    "backend_bootstrap_secret_path": "/run/secrets/vpn-backend-bootstrap.hex"
                }
            }"#;
    assert_vpn_config_error(entry, "relay mode Exit");
    let missing_tls = r#"{
                "mode": "Exit",
                "listen": "127.0.0.1:0",
                "vpn": {
                    "enabled": true,
                    "helper_ticket_secret_path": "/run/secrets/vpn-helper-ticket.hex",
                    "backend_bootstrap_secret_path": "/run/secrets/vpn-backend-bootstrap.hex"
                }
            }"#;
    assert_vpn_config_error(missing_tls, "persistent tls.certificate_path");
    let missing_certificate = r#"{
                "mode": "Exit",
                "listen": "127.0.0.1:0",
                "tls": {
                    "certificate_path": "/run/secrets/relay-cert.pem",
                    "private_key_path": "/run/secrets/relay-key.pem"
                },
                "vpn": {
                    "enabled": true,
                    "helper_ticket_secret_path": "/run/secrets/vpn-helper-ticket.hex",
                    "backend_bootstrap_secret_path": "/run/secrets/vpn-backend-bootstrap.hex"
                }
            }"#;
    assert_vpn_config_error(missing_certificate, "verified handshake.certificate");
}
#[test]
fn vpn_requires_persistent_identity_and_strict_authenticated_directory() {
    let issuer_mldsa = "bb".repeat(MlDsaSuite::MlDsa65.public_key_len());
    let certificate = format!(
        r#"{{
                "bundle_path": "/run/secrets/relay-certificate.cbor",
                "issuer_ed25519_hex": "{}",
                "issuer_mldsa_hex": "{issuer_mldsa}"
            }}"#,
        "cc".repeat(32),
    );
    let missing_identity = format!(
        r#"{{
                "mode": "Exit",
                "listen": "127.0.0.1:0",
                "tls": {{
                    "certificate_path": "/run/secrets/relay-cert.pem",
                    "private_key_path": "/run/secrets/relay-key.pem"
                }},
                "handshake": {{ "certificate": {certificate} }},
                "vpn": {{
                    "enabled": true,
                    "helper_ticket_secret_path": "/run/secrets/vpn-helper-ticket.hex",
                    "backend_bootstrap_secret_path": "/run/secrets/vpn-backend-bootstrap.hex"
                }}
            }}"#
    );
    assert_vpn_config_error(&missing_identity, "persistent relay identity key");
    let missing_directory = format!(
        r#"{{
                "mode": "Exit",
                "listen": "127.0.0.1:0",
                "tls": {{
                    "certificate_path": "/run/secrets/relay-cert.pem",
                    "private_key_path": "/run/secrets/relay-key.pem"
                }},
                "handshake": {{
                    "descriptor_manifest_path": "/run/secrets/relay-descriptor-manifest.json",
                    "certificate": {certificate}
                }},
                "vpn": {{
                    "enabled": true,
                    "helper_ticket_secret_path": "/run/secrets/vpn-helper-ticket.hex",
                    "backend_bootstrap_secret_path": "/run/secrets/vpn-backend-bootstrap.hex"
                }}
            }}"#,
    );
    assert_vpn_config_error(&missing_directory, "authenticated guard_directory");
    let permissive_directory = format!(
        r#"{{
                "mode": "Exit",
                "listen": "127.0.0.1:0",
                "tls": {{
                    "certificate_path": "/run/secrets/relay-cert.pem",
                    "private_key_path": "/run/secrets/relay-key.pem"
                }},
                "handshake": {{
                    "descriptor_manifest_path": "/run/secrets/relay-descriptor-manifest.json",
                    "certificate": {certificate}
                }},
                "guard_directory": {{
                    "snapshot_path": "/run/secrets/guard-directory.norito",
                    "expected_snapshot_digest_hex": "{}",
                    "allow_missing_entry": true
                }},
                "vpn": {{
                    "enabled": true,
                    "helper_ticket_secret_path": "/run/secrets/vpn-helper-ticket.hex",
                    "backend_bootstrap_secret_path": "/run/secrets/vpn-backend-bootstrap.hex"
                }}
            }}"#,
        "ee".repeat(32),
    );
    assert_vpn_config_error(
        &permissive_directory,
        "forbids guard_directory.allow_missing_entry",
    );
}
#[test]
fn non_loopback_admin_listener_is_rejected() {
    let json = config_fixture!("admin_nonloopback.json");
    let path = write_config(json);
    let err = RelayConfig::load(path).expect_err("remote admin listener must be protected");
    match err {
        ConfigError::Admin(message) => {
            assert!(message.contains("loopback"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
#[test]
fn non_loopback_admin_listener_remains_rejected_with_token_file() {
    let json = config_fixture!("admin_nonloopback_with_token.json");
    let path = write_config(json);
    let err = RelayConfig::load(path).expect_err("admin listener must remain local");
    assert!(matches!(err, ConfigError::Admin(message) if message.contains("loopback")));
}
#[test]
fn loopback_admin_listener_requires_token_file() {
    let json = config_fixture!("admin_loopback_without_token.json");
    let path = write_config(json);
    let err = RelayConfig::load(path).expect_err("local admin listener still requires auth");
    assert!(
        matches!(err, ConfigError::Admin(message) if message.contains("admin_auth_token_path"))
    );
}
#[test]
fn padding_cell_size_must_be_non_zero() {
    let json = config_fixture!("zero_padding_cell.json");
    let path = write_config(json);
    let err = RelayConfig::load(path).expect_err("validation must fail");
    match err {
        ConfigError::Padding(message) => {
            assert!(
                message.contains("non-zero"),
                "unexpected padding error: {message}"
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
#[test]
fn padding_cell_size_must_fit_ipv6_mtu() {
    let max = PaddingConfig::max_cell_size_bytes();
    let invalid = u32::from(max) + 1;
    let json = format!(
        r#"{{
                "mode": "Middle",
                "listen": "127.0.0.1:0",
                "pow": {{ "required": true, "difficulty": 18 }},
                "padding": {{ "cell_size": {invalid}, "max_idle_millis": 200 }}
            }}"#
    );
    let path = write_config(&json);
    let err = RelayConfig::load(path).expect_err("validation must fail");
    match err {
        ConfigError::Padding(message) => {
            assert!(
                message.contains("MTU-safe"),
                "unexpected padding error: {message}"
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
#[test]
fn padding_cell_size_clamps_to_limit() {
    let max = PaddingConfig::max_cell_size_bytes();
    assert_eq!(PaddingConfig::clamp_cell_size(max), max);
    let invalid = max.saturating_add(1);
    assert_eq!(PaddingConfig::clamp_cell_size(invalid), max);
    assert_eq!(PaddingConfig::clamp_cell_size(0), 0);
}
#[test]
fn constant_rate_capability_rejects_future_versions() {
    let json = config_fixture!("future_constant_rate.json");
    let path = write_config(json);
    let err = RelayConfig::load(path).expect_err("validation must fail");
    match err {
        ConfigError::ConstantRateCapability(message) => {
            assert!(
                message.contains("version"),
                "unexpected constant-rate capability error: {message}"
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
#[test]
fn constant_rate_capability_returns_none_when_disabled() {
    let json = config_fixture!("disabled_constant_rate.json");
    let path = write_config(json);
    let cfg = RelayConfig::load(path).expect("load config");
    assert!(cfg.constant_rate_capability().is_none());
}
#[test]
fn incentive_log_defaults_when_enabled() {
    let mut cfg = RelayConfig {
        mode: RelayMode::Entry,
        listen: "127.0.0.1:0".to_owned(),
        admin_listen: None,
        admin_auth_token_path: None,
        tls: None,
        pow: None,
        padding: None,
        handshake: None,
        congestion: None,
        compliance: None,
        incentives: Some(IncentiveLogConfig {
            enable: true,
            spool_dir: None,
            max_active_epochs: 0,
            max_measurements_per_epoch: 0,
        }),
        exit_routing: ExitRoutingConfig::default(),
        vpn: None,
        privacy: None,
        guard_directory: None,
        constant_rate_capability: None,
        constant_rate_profile: ConstantRateProfileName::Core,
    };
    cfg.validate().expect("validate config");
    let incentives = cfg.incentive_log_config();
    assert!(incentives.enable);
    assert!(incentives.spool_dir.is_some());
    assert_eq!(
        incentives.max_active_epochs,
        INCENTIVE_DEFAULT_ACTIVE_EPOCHS
    );
    assert_eq!(
        incentives.max_measurements_per_epoch,
        INCENTIVE_DEFAULT_MEASUREMENTS_PER_EPOCH
    );
}
#[test]
fn incentive_memory_geometry_accepts_exact_aggregate_and_rejects_max_plus_one() {
    let mut exact = IncentiveLogConfig {
        enable: false,
        spool_dir: None,
        max_active_epochs: INCENTIVE_MAX_ACTIVE_EPOCHS_V1,
        max_measurements_per_epoch: INCENTIVE_MAX_RETAINED_MEASUREMENTS_V1
            / INCENTIVE_MAX_ACTIVE_EPOCHS_V1,
    };
    exact.validate().expect("exact aggregate limit");
    let mut overflow = IncentiveLogConfig {
        max_measurements_per_epoch: exact.max_measurements_per_epoch + 1,
        ..exact
    };
    assert!(matches!(
        overflow.validate(),
        Err(IncentiveLogError::Config(message)) if message.contains("aggregate")
    ));
}
#[test]
fn exit_routing_validation_rejects_plain_http() {
    let mut routing = ExitRoutingConfig {
        norito_stream: Some(NoritoStreamRoutingConfig {
            torii_ws_url: "http://localhost:8080/ws".into(),
            connect_timeout_millis: 0,
            padding_target_millis: 0,
            gar_category_read_only: None,
            gar_category_authenticated: None,
            spool_dir: None,
            route_refresh_secs: 0,
        }),
        ..ExitRoutingConfig::default()
    };
    let err = routing.validate().expect_err("validation must fail");
    match err {
        ConfigError::Routing(message) => {
            assert!(
                message.contains("ws://"),
                "unexpected routing error message: {message}"
            );
        }
        other => panic!("unexpected error {other:?}"),
    }
}
#[test]
fn exit_routing_validation_requires_bounded_canonical_gar_categories() {
    assert!(is_canonical_gar_category_v1("stream.norito.read_only"));
    assert!(!is_canonical_gar_category_v1("Stream.Norito.ReadOnly"));

    let mut routing = ExitRoutingConfig {
        norito_stream: Some(NoritoStreamRoutingConfig {
            torii_ws_url: "wss://localhost:8080/ws".into(),
            connect_timeout_millis: 0,
            padding_target_millis: 0,
            gar_category_read_only: Some("Stream.Norito.ReadOnly".into()),
            gar_category_authenticated: None,
            spool_dir: None,
            route_refresh_secs: 0,
        }),
        ..ExitRoutingConfig::default()
    };
    let error = routing.validate().expect_err("mixed-case label must fail");
    assert!(
        matches!(error, ConfigError::Routing(message) if message.contains("canonical lowercase ASCII"))
    );

    routing
        .norito_stream
        .as_mut()
        .expect("route retained")
        .gar_category_read_only = Some("a".repeat(GAR_CATEGORY_MAX_BYTES_V1 + 1));
    let error = routing.validate().expect_err("oversized label must fail");
    assert!(
        matches!(error, ConfigError::Routing(message) if message.contains("canonical lowercase ASCII"))
    );
}
#[test]
fn exit_routing_validation_rejects_kaigi_plain_http() {
    let mut routing = ExitRoutingConfig {
        kaigi_stream: Some(KaigiStreamRoutingConfig {
            hub_ws_url: "http://localhost:9090/ws".into(),
            connect_timeout_millis: 0,
            gar_category_public: None,
            gar_category_authenticated: None,
            spool_dir: None,
            route_refresh_secs: 0,
        }),
        ..ExitRoutingConfig::default()
    };
    let err = routing.validate().expect_err("validation must fail");
    match err {
        ConfigError::Routing(message) => {
            assert!(
                message.contains("ws://"),
                "unexpected routing error message: {message}"
            );
        }
        other => panic!("unexpected error {other:?}"),
    }
}
#[test]
fn pow_defaults_match_first_release_admission_policy() {
    let pow = PowConfig::default();
    assert!(pow.required);
    assert_eq!(pow.difficulty, u32::from(puzzle::DEFAULT_DIFFICULTY));
    assert!(
        pow.puzzle.as_ref().is_some_and(|puzzle| puzzle.enabled),
        "the default Argon2 gate must be enabled"
    );
}
#[test]
fn pow_revocation_store_capacity_enforces_first_release_ceiling() {
    let mut exact = PowConfig {
        revocation_store_capacity: u64::try_from(pow::TICKET_REVOCATION_STORE_MAX_ENTRIES_V1)
            .expect("fixed limit fits u64"),
        ..PowConfig::default()
    };
    exact
        .apply_defaults()
        .expect("exact first-release capacity validates");
    let mut excessive = PowConfig {
        revocation_store_capacity: u64::try_from(pow::TICKET_REVOCATION_STORE_MAX_ENTRIES_V1 + 1)
            .expect("fixed limit plus one fits u64"),
        ..PowConfig::default()
    };
    assert!(matches!(
        excessive.apply_defaults(),
        Err(ConfigError::TicketReplayStore(message))
            if message.contains("revocation_store_capacity")
                && message.contains("first-release limit")
    ));
}
#[test]
fn omitted_pow_policy_fields_use_secure_first_release_defaults() {
    let json = config_fixture!("pow_defaults.json");
    let path = write_config(json);
    let config = RelayConfig::load(path).expect("load config with secure PoW defaults");
    assert!(config.pow_config().required);
    assert_eq!(
        config.pow_config().difficulty,
        u32::from(puzzle::DEFAULT_DIFFICULTY)
    );
    assert!(
        config
            .pow_config()
            .puzzle
            .as_ref()
            .is_some_and(|puzzle| puzzle.enabled)
    );
}
#[test]
fn pow_config_rejects_zero_difficulty() {
    let mut pow = PowConfig {
        difficulty: 0,
        ..PowConfig::default()
    };
    let err = pow
        .apply_defaults()
        .expect_err("zero work factor must fail");
    assert!(
        matches!(err, ConfigError::Puzzle(ref message) if message.contains("difficulty")),
        "unexpected error: {err:?}"
    );
}
#[test]
fn pow_config_rejects_difficulty_above_supported_corridor() {
    let mut pow = PowConfig {
        difficulty: u32::from(puzzle::MAX_DIFFICULTY) + 1,
        ..PowConfig::default()
    };
    let err = pow
        .apply_defaults()
        .expect_err("oversized work factor must fail");
    assert!(
        matches!(err, ConfigError::Puzzle(ref message) if message.contains("difficulty")),
        "unexpected error: {err:?}"
    );
}
#[test]
fn pow_config_rejects_optional_admission() {
    let mut pow = PowConfig {
        required: false,
        ..PowConfig::default()
    };
    let err = pow
        .apply_defaults()
        .expect_err("optional admission must fail");
    assert!(
        matches!(err, ConfigError::Puzzle(ref message) if message.contains("required")),
        "unexpected error: {err:?}"
    );
}
#[test]
fn pow_config_rejects_zero_adaptive_difficulty_floor() {
    let mut pow = PowConfig {
        adaptive: AdaptiveDifficultyConfig {
            min_difficulty: 0,
            ..AdaptiveDifficultyConfig::default()
        },
        ..PowConfig::default()
    };
    let err = pow
        .apply_defaults()
        .expect_err("zero adaptive difficulty floor must fail");
    assert!(
        matches!(err, ConfigError::Puzzle(ref message) if message.contains("min_difficulty")),
        "unexpected error: {err:?}"
    );
}
#[test]
fn pow_config_rejects_adaptive_difficulty_above_supported_corridor() {
    let mut pow = PowConfig {
        adaptive: AdaptiveDifficultyConfig {
            max_difficulty: puzzle::MAX_DIFFICULTY + 1,
            ..AdaptiveDifficultyConfig::default()
        },
        ..PowConfig::default()
    };
    let err = pow
        .apply_defaults()
        .expect_err("oversized adaptive difficulty ceiling must fail");
    assert!(
        matches!(err, ConfigError::Puzzle(ref message) if message.contains("max_difficulty")),
        "unexpected error: {err:?}"
    );
}
#[test]
fn puzzle_config_disabled_is_rejected() {
    let mut pow = PowConfig {
        puzzle: Some(PuzzleConfig {
            enabled: false,
            memory_kib: 0,
            time_cost: 0,
            lanes: 0,
        }),
        ..PowConfig::default()
    };
    let err = pow.apply_defaults().expect_err("disabled puzzle must fail");
    assert!(
        matches!(err, ConfigError::Puzzle(ref message) if message.contains("enabled")),
        "unexpected error: {err:?}"
    );
}
#[test]
fn quotas_for_mode_honours_overrides() {
    let mut pow = PowConfig {
        quotas: QuotaConfig {
            per_remote_burst: 100,
            per_remote_window_secs: 45,
            per_descriptor_burst: 80,
            per_descriptor_window_secs: 35,
            cooldown_secs: 15,
            max_entries: 2048,
        },
        quotas_per_mode: Some(HopQuotaOverrides {
            entry: Some(QuotaConfig {
                per_remote_burst: 5,
                per_remote_window_secs: 30,
                per_descriptor_burst: 0,
                per_descriptor_window_secs: 0,
                cooldown_secs: 9,
                max_entries: 1024,
            }),
            middle: None,
            exit: Some(QuotaConfig {
                per_remote_burst: 70,
                per_remote_window_secs: 0,
                per_descriptor_burst: 20,
                per_descriptor_window_secs: 0,
                cooldown_secs: 0,
                max_entries: 0,
            }),
        }),
        ..PowConfig::default()
    };
    pow.apply_defaults().expect("pow defaults");
    let entry = pow.quotas_for_mode(RelayMode::Entry);
    assert_eq!(entry.per_remote_burst, 5);
    assert_eq!(entry.per_remote_window_secs, 30);
    assert_eq!(entry.per_descriptor_burst, 0);
    assert_eq!(entry.cooldown_secs, 9);
    assert_eq!(entry.max_entries, 1024);
    let middle = pow.quotas_for_mode(RelayMode::Middle);
    assert_eq!(middle.per_remote_burst, 100);
    assert_eq!(middle.per_remote_window_secs, 45);
    assert_eq!(middle.cooldown_secs, 15);
    assert_eq!(middle.max_entries, 2048);
    let exit = pow.quotas_for_mode(RelayMode::Exit);
    assert_eq!(exit.per_remote_burst, 70);
    assert_eq!(
        exit.per_remote_window_secs,
        QuotaConfig::default_per_remote_window_secs()
    );
    assert_eq!(
        exit.per_descriptor_window_secs,
        QuotaConfig::default_per_descriptor_window_secs()
    );
    assert_eq!(exit.cooldown_secs, QuotaConfig::default_cooldown_secs());
    assert_eq!(exit.max_entries, QuotaConfig::default_max_entries());
    assert_eq!(exit.per_descriptor_burst, 20);
}
#[test]
fn quota_tracker_capacity_accepts_exact_limit_and_rejects_plus_one() {
    let exact = QuotaConfig {
        max_entries: QUOTA_TRACKER_MAX_ENTRIES_V1,
        ..QuotaConfig::default()
    };
    exact.validate().expect("exact tracker limit must validate");
    let mut base_overflow = PowConfig {
        quotas: QuotaConfig {
            max_entries: QUOTA_TRACKER_MAX_ENTRIES_V1 + 1,
            ..QuotaConfig::default()
        },
        ..PowConfig::default()
    };
    let error = base_overflow
        .apply_defaults()
        .expect_err("base tracker limit + 1 must fail");
    assert!(
        matches!(error, ConfigError::Quota(ref message) if message.contains("quotas.max_entries")),
        "unexpected error: {error:?}"
    );
    let mut override_overflow = PowConfig {
        quotas_per_mode: Some(HopQuotaOverrides {
            middle: Some(QuotaConfig {
                max_entries: QUOTA_TRACKER_MAX_ENTRIES_V1 + 1,
                ..QuotaConfig::default()
            }),
            ..HopQuotaOverrides::default()
        }),
        ..PowConfig::default()
    };
    let error = override_overflow
        .apply_defaults()
        .expect_err("per-mode tracker limit + 1 must fail");
    assert!(
        matches!(error, ConfigError::Quota(ref message) if message.contains("quotas_per_mode.middle.max_entries")),
        "unexpected error: {error:?}"
    );
}
#[test]
fn puzzle_config_rejects_invalid_values() {
    let mut pow = PowConfig {
        puzzle: Some(PuzzleConfig {
            enabled: true,
            memory_kib: 1024,
            time_cost: 0,
            lanes: 0,
        }),
        ..PowConfig::default()
    };
    let err = pow.apply_defaults().expect_err("invalid puzzle config");
    match err {
        ConfigError::Puzzle(message) => {
            assert!(message.contains("memory_kib"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
#[test]
fn pow_config_parameters_reject_inverted_ticket_timing_without_panic() {
    let pow = PowConfig {
        max_future_skew_secs: 10,
        min_ticket_ttl_secs: 30,
        ..PowConfig::default()
    };
    match pow.parameters() {
        Err(ConfigError::Puzzle(message)) => assert!(
            message.contains("invalid pow ticket timing parameters"),
            "unexpected pow timing error: {message}"
        ),
        other => panic!("expected pow timing error, got {other:?}"),
    }
    let mut pow = pow;
    match pow.apply_defaults() {
        Err(ConfigError::Puzzle(message)) => assert!(
            message.contains("invalid pow ticket timing parameters"),
            "unexpected pow defaults error: {message}"
        ),
        other => panic!("expected pow defaults error, got {other:?}"),
    }
}
#[test]
fn puzzle_config_builds_parameters() {
    let mut pow = PowConfig {
        difficulty: 12,
        max_future_skew_secs: 45,
        min_ticket_ttl_secs: 15,
        puzzle: Some(PuzzleConfig {
            enabled: true,
            memory_kib: 32 * 1024,
            time_cost: 3,
            lanes: 2,
        }),
        ..PowConfig::default()
    };
    pow.apply_defaults().expect("defaults");
    let base = pow::Parameters::new(12, Duration::from_secs(45), Duration::from_secs(15));
    let params = pow
        .puzzle_parameters(&base)
        .expect("parameters")
        .expect("enabled puzzle");
    assert_eq!(params.memory_kib().get(), 32 * 1024);
    assert_eq!(params.time_cost().get(), 3);
    assert_eq!(params.lanes().get(), 2);
    assert_eq!(params.difficulty(), 12);
}
#[test]
fn replay_filter_defaults_and_rounds_parameters() {
    let mut cfg = ReplayFilterConfig {
        enabled: true,
        bits: 1_000,
        hash_functions: 0,
        ttl_secs: 0,
    };
    cfg.apply_defaults().expect("defaults");
    assert_eq!(cfg.bits, 1_024);
    assert_eq!(
        cfg.hash_functions,
        ReplayFilterConfig::default_hash_functions()
    );
    assert_eq!(cfg.ttl_secs, ReplayFilterConfig::default_ttl_secs());
}
#[test]
fn replay_filter_rejects_invalid_parameters() {
    let mut too_many_bits = ReplayFilterConfig {
        enabled: true,
        bits: (1 << 24) + 1,
        hash_functions: 4,
        ttl_secs: 10,
    };
    let err = too_many_bits
        .apply_defaults()
        .expect_err("bits exceeding limit should fail");
    assert!(
        matches!(err, ConfigError::ReplayFilter(ref message) if message.contains("bits")),
        "unexpected error: {err:?}"
    );
    let mut overflowing_bits = ReplayFilterConfig {
        enabled: true,
        bits: u32::MAX,
        hash_functions: 4,
        ttl_secs: 10,
    };
    let err = overflowing_bits
        .apply_defaults()
        .expect_err("overflowing bit count should fail");
    assert!(
        matches!(err, ConfigError::ReplayFilter(ref message) if message.contains("bits")),
        "unexpected error: {err:?}"
    );
    let mut too_many_hashes = ReplayFilterConfig {
        enabled: true,
        bits: 256,
        hash_functions: 17,
        ttl_secs: 10,
    };
    let err = too_many_hashes
        .apply_defaults()
        .expect_err("hash functions exceeding limit should fail");
    assert!(
        matches!(err, ConfigError::ReplayFilter(ref message) if message.contains("hash_functions")),
        "unexpected error: {err:?}"
    );
}
#[test]
fn relay_config_loads_replay_filter_settings() {
    let json = config_fixture!("replay_filter.json");
    let path = write_config(json);
    let cfg = RelayConfig::load(path).expect("load config");
    let filter = cfg.pow_config().replay_filter();
    assert!(filter.is_enabled());
    assert_eq!(filter.bits_usize(), 4_096);
    assert_eq!(filter.hash_count(), 3);
    assert_eq!(filter.ttl().as_secs(), 45);
}
#[test]
fn rejects_partial_tls_paths() {
    let json = config_fixture!("partial_tls.json");
    let path = write_config(json);
    let err = RelayConfig::load(path).expect_err("config should fail");
    match err {
        ConfigError::TlsPaths(message) => assert!(message.contains("private key path")),
        other => panic!("unexpected error variant: {other:?}"),
    }
}
#[test]
fn rejects_bad_address() {
    let json = config_fixture!("bad_address.json");
    let path = write_config(json);
    let err = RelayConfig::load(path).expect_err("config should fail");
    match err {
        ConfigError::InvalidAddress(field, value) => {
            assert_eq!(field, "listen");
            assert_eq!(value, "invalid:address");
        }
        other => panic!("unexpected error variant: {other:?}"),
    }
}
#[test]
fn handshake_descriptor_commit_is_decoded() {
    let json = config_fixture!("descriptor_commit.json");
    let path = write_config(json);
    let cfg = RelayConfig::load(path).expect("load config");
    let commit = cfg
        .handshake_policy()
        .descriptor_commit_bytes()
        .expect("decode commit")
        .expect("commit present");
    assert_eq!(
        commit,
        [
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
            0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B,
            0x1C, 0x1D, 0x1E, 0x1F
        ]
    );
}
#[test]
fn rejects_unknown_kem_identifier() {
    let json = config_fixture!("unknown_kem.json");
    let path = write_config(json);
    let err = RelayConfig::load(path).expect_err("config should fail");
    match err {
        ConfigError::Handshake(message) => {
            assert!(message.contains("unknown KEM identifier"));
        }
        other => panic!("unexpected error variant: {other:?}"),
    }
}
#[test]
fn kem_zero_wire_id_is_ml_kem_512_and_classic_alias_is_rejected() {
    assert_eq!(
        parse_kem_id("ml-kem-512"),
        Some(capability::KemId::MlKem512)
    );
    assert_eq!(capability::KemId::MlKem512.code(), 0x00);
    assert_eq!(
        capability::KemId::from_code(0x00),
        Some(capability::KemId::MlKem512)
    );
    assert_eq!(capability::KemId::MlKem512.to_string(), "ml-kem-512");
    assert_eq!(parse_kem_id("classic"), None);
}
#[test]
fn only_dilithium3_is_accepted_as_a_transcript_signature() {
    assert_eq!(
        parse_signature_id("dilithium3"),
        Some(capability::SignatureId::Dilithium3)
    );
    assert_eq!(capability::SignatureId::Dilithium3.code(), 0x01);
    assert_eq!(
        capability::SignatureId::from_code(0x01),
        Some(capability::SignatureId::Dilithium3)
    );
    assert_eq!(capability::SignatureId::from_code(0x00), None);
    assert_eq!(capability::SignatureId::from_code(0x02), None);
    assert_eq!(parse_signature_id("ed25519"), None);
    assert_eq!(parse_signature_id("falcon512"), None);
}
#[test]
fn handshake_policy_rejects_duplicate_algorithm_identifiers() {
    let mut duplicate_kem = HandshakePolicy::default();
    duplicate_kem.kem.push(duplicate_kem.kem[0].clone());
    assert!(matches!(
        duplicate_kem.validate(),
        Err(ConfigError::Handshake(message)) if message.contains("duplicate KEM identifier")
    ));

    let mut duplicate_signature = HandshakePolicy::default();
    duplicate_signature
        .signatures
        .push(duplicate_signature.signatures[0].clone());
    assert!(matches!(
        duplicate_signature.validate(),
        Err(ConfigError::Handshake(message)) if message.contains("duplicate signature identifier")
    ));
}
#[test]
fn rejects_oversized_handshake_grease_value() {
    let value_hex = "aa".repeat(usize::from(u16::MAX) + 1);
    let json = format!(
        r#"{{
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "handshake": {{
                    "grease": [
                        {{ "typ": 32528, "value_hex": "{value_hex}" }}
                    ]
                }}
            }}"#
    );
    let path = write_config(&json);
    let err = RelayConfig::load(path).expect_err("oversized GREASE value should fail");
    match err {
        ConfigError::Handshake(message) => {
            assert!(message.contains("value length"));
            assert!(message.contains("exceeds u16::MAX"));
        }
        other => panic!("unexpected error variant: {other:?}"),
    }
}
#[test]
fn pow_defaults_populate_missing_fields() {
    let json = config_fixture!("pow_zero_defaults.json");
    let path = write_config(json);
    let cfg = RelayConfig::load(path).expect("load config");
    assert_eq!(cfg.pow_config().max_future_skew_secs, 300);
    assert_eq!(cfg.pow_config().min_ticket_ttl_secs, 30);
}
#[test]
fn rejects_removed_inline_identity_private_key_field() {
    let json = config_fixture!("short_identity_key.json");
    let path = write_config(json);
    RelayConfig::load(path).expect_err("removed private-key field must be unknown");
}
#[test]
fn custom_congestion_config_validates() {
    let json = config_fixture!("custom_congestion.json");
    let path = write_config(json);
    let cfg = RelayConfig::load(path).expect("load config");
    assert_eq!(cfg.congestion_config().max_circuits_per_client, 4);
    assert_eq!(cfg.congestion_config().max_active_circuits, 32);
    assert_eq!(cfg.congestion_config().handshake_cooldown_millis, 750);
}
#[test]
fn congestion_capacity_accepts_exact_limit_and_rejects_overflow() {
    let mut exact = CongestionConfig {
        max_circuits_per_client: 8,
        max_active_circuits: CONGESTION_MAX_ACTIVE_CIRCUITS_V1,
        handshake_cooldown_millis: 200,
    };
    exact.validate().expect("exact global limit must validate");
    let mut overflow = CongestionConfig {
        max_active_circuits: CONGESTION_MAX_ACTIVE_CIRCUITS_V1 + 1,
        ..CongestionConfig::default()
    };
    assert!(matches!(
        overflow.validate(),
        Err(ConfigError::Congestion(message)) if message.contains("max_active_circuits")
    ));
    let mut inconsistent = CongestionConfig {
        max_circuits_per_client: 9,
        max_active_circuits: 8,
        handshake_cooldown_millis: 200,
    };
    assert!(matches!(
        inconsistent.validate(),
        Err(ConfigError::Congestion(message)) if message.contains("max_circuits_per_client")
    ));
}
#[test]
fn compliance_requires_log_path_when_enabled() {
    let json = config_fixture!("compliance_without_log.json");
    let path = write_config(json);
    let err = RelayConfig::load(path).expect_err("config should fail");
    match err {
        ConfigError::Compliance(message) => {
            assert!(message.contains("log_path"));
        }
        other => panic!("expected compliance error, got {other:?}"),
    }
}
#[test]
fn compliance_salt_decodes() {
    let salt_hex = "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20";
    let salt_file = write_manifest(salt_hex);
    let json = format!(
        r#"{{
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "compliance": {{
                    "enable": true,
                    "log_path": "/tmp/logger.jsonl",
                    "hash_salt_path": "{}",
                    "max_log_bytes": 123456,
                    "max_backup_files": 3,
                    "pipeline_spool_dir": "/tmp/spool"
                }}
            }}"#,
        salt_file.path().display()
    );
    let path = write_config(&json);
    let cfg = RelayConfig::load(path).expect("load config");
    let salt = cfg
        .compliance_config()
        .hash_salt_bytes()
        .expect("salt decode")
        .expect("salt present");
    assert_eq!(salt, <[u8; 32]>::from_hex(salt_hex).expect("hex to bytes"));
    assert_eq!(cfg.compliance_config().max_log_bytes, 123456);
    assert_eq!(cfg.compliance_config().max_backup_files, 3);
    assert_eq!(
        cfg.compliance_config()
            .pipeline_spool_dir()
            .expect("spool dir")
            .display()
            .to_string(),
        "/tmp/spool"
    );
}
#[test]
fn compliance_enabled_requires_private_salt_path_and_debug_redacts_it() {
    let mut config = ComplianceConfig {
        enable: true,
        log_path: Some("/var/log/soranet/compliance.jsonl".into()),
        ..ComplianceConfig::default()
    };
    let error = config
        .apply_defaults()
        .expect_err("enumerable unsalted endpoint hashes must fail closed");
    assert!(
        matches!(error, ConfigError::Compliance(message) if message.contains("hash_salt_path"))
    );

    config.hash_salt_path = Some("/run/secrets/compliance-hash-salt.hex".into());
    let rendered = format!("{config:?}");
    assert!(rendered.contains("<redacted>"));
    assert!(!rendered.contains("compliance-hash-salt.hex"));
}
#[test]
fn compliance_rejects_retired_inline_hash_salt() {
    let json = r#"{
        "mode": "Entry",
        "listen": "127.0.0.1:0",
        "compliance": {
            "enable": false,
            "hash_salt_hex": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        }
    }"#;
    let path = write_config(json);
    RelayConfig::load(path).expect_err("inline compliance secrets must not be accepted");
}
#[cfg(unix)]
#[test]
fn compliance_salt_rejects_permissions_symlinks_and_noncanonical_encoding() {
    use std::os::unix::fs::{PermissionsExt as _, symlink};

    let canonical = "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20";
    let salt_file = write_manifest(canonical);
    let mut config = ComplianceConfig {
        hash_salt_path: Some(salt_file.path().to_path_buf()),
        ..ComplianceConfig::default()
    };
    assert_eq!(
        config.hash_salt_bytes().expect("private salt"),
        Some(<[u8; 32]>::from_hex(canonical).expect("canonical salt"))
    );
    std::fs::set_permissions(salt_file.path(), std::fs::Permissions::from_mode(0o640))
        .expect("make salt group-readable");
    assert!(config.hash_salt_bytes().is_err());
    std::fs::set_permissions(salt_file.path(), std::fs::Permissions::from_mode(0o600))
        .expect("restore private salt permissions");

    let link_path = salt_file.path().with_extension("link");
    symlink(salt_file.path(), &link_path).expect("create salt symlink");
    config.hash_salt_path = Some(link_path);
    assert!(config.hash_salt_bytes().is_err());

    let uppercase = write_manifest(&canonical.to_ascii_uppercase());
    config.hash_salt_path = Some(uppercase.path().to_path_buf());
    let error = config
        .hash_salt_bytes()
        .expect_err("uppercase salt encoding is not canonical");
    assert!(matches!(error, ConfigError::Compliance(message) if message.contains("lowercase")));

    for degenerate in ["00".repeat(32), "ab".repeat(32)] {
        let file = write_manifest(&degenerate);
        config.hash_salt_path = Some(file.path().to_path_buf());
        let error = config
            .hash_salt_bytes()
            .expect_err("degenerate salt key must fail closed");
        assert!(
            matches!(error, ConfigError::Compliance(message) if message.contains("degenerate"))
        );
    }
}
#[test]
fn identity_manifest_is_loaded() {
    let seed_hex = "abf17b54402f71fbb8ce1b716e2fdd9e7e1825cfb64fe0d4a1cfae3d6458f207";
    let manifest = write_manifest(&format!(
        r#"{{
                "version": 1,
                "identity": {{
                    "ed25519_private_key_hex": "{seed_hex}"
                }}
            }}"#
    ));
    let manifest_path = manifest.path().to_str().expect("manifest path utf-8");
    let json = format!(
        r#"{{
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "handshake": {{
                    "descriptor_manifest_path": "{manifest_path}"
                }}
            }}"#
    );
    let config_path = write_config(&json);
    let config = RelayConfig::load(config_path).expect("load config");
    let seed = config
        .handshake_policy()
        .identity_private_key_from_manifest()
        .expect("manifest parsing")
        .expect("seed present");
    let expected_bytes = hex::decode(seed_hex).expect("valid hex");
    let mut expected = [0u8; 32];
    expected.copy_from_slice(&expected_bytes);
    assert_eq!(seed, expected);
}
#[test]
fn identity_manifest_missing_key_errors() {
    let manifest = write_manifest(r#"{ "version": 1, "identity": { "metadata": "placeholder" } }"#);
    let manifest_path = manifest.path().to_str().expect("manifest path utf-8");
    let json = format!(
        r#"{{
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "handshake": {{
                    "descriptor_manifest_path": "{manifest_path}"
                }}
            }}"#
    );
    let config_path = write_config(&json);
    let config = RelayConfig::load(config_path).expect("load config");
    match config
        .handshake_policy()
        .identity_private_key_from_manifest()
    {
        Err(ConfigError::DescriptorManifest { message, .. }) => {
            assert!(message.contains("missing"), "unexpected message: {message}");
        }
        other => panic!("expected manifest error, got {other:?}"),
    }
}
#[test]
fn deploy_sample_config_validates() {
    let json = include_str!("../deploy/config/relay.entry.json");
    let mut cfg: RelayConfig = norito::json::from_str(json).expect("parse sample config");
    cfg.validate().expect("sample config validates");
    assert_eq!(cfg.mode, RelayMode::Entry);
    assert_eq!(
        cfg.handshake_policy()
            .descriptor_manifest_path()
            .expect("manifest path set")
            .display()
            .to_string(),
        "/etc/soranet/relay/secrets/relay-descriptor-manifest.json"
    );
    assert!(cfg.compliance_config().enable);
    assert_eq!(
        cfg.compliance_config()
            .log_path()
            .expect("log path")
            .display()
            .to_string(),
        "/var/log/soranet/relay_compliance.jsonl"
    );
    assert_eq!(
        cfg.compliance_config().hash_salt_path.as_deref(),
        Some(Path::new(
            "/etc/soranet/relay/secrets/compliance-hash-salt.hex"
        ))
    );
    assert_eq!(cfg.compliance_config().max_log_bytes, 67_108_864);
    assert_eq!(cfg.compliance_config().max_backup_files, 7);
    assert_eq!(
        cfg.compliance_config()
            .pipeline_spool_dir()
            .expect("spool dir")
            .display()
            .to_string(),
        "/var/spool/soranet/audit"
    );
    assert_eq!(cfg.constant_rate_profile(), ConstantRateProfileName::Core);
    assert_eq!(
        cfg.privacy_config().bucket_secs,
        DEFAULT_PRIVACY_BUCKET_SECS
    );
    assert_eq!(
        cfg.privacy_config().min_handshakes,
        DEFAULT_PRIVACY_MIN_HANDSHAKES
    );
    assert_eq!(
        cfg.privacy_config().flush_delay_buckets,
        DEFAULT_PRIVACY_FLUSH_DELAY_BUCKETS
    );
    assert_eq!(
        cfg.privacy_config().force_flush_buckets,
        DEFAULT_PRIVACY_FORCE_FLUSH_BUCKETS
    );
    assert_eq!(
        cfg.privacy_config().max_completed_buckets,
        DEFAULT_PRIVACY_MAX_COMPLETED_BUCKETS
    );
}
#[test]
fn token_config_merges_inline_and_file_revocations() {
    use iroha_crypto::soranet::token::compute_issuer_fingerprint;
    use soranet_pq::{MlDsaSuite, generate_mldsa_keypair_from_os as generate_mldsa_keypair};
    let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44).expect("generate keypair");
    let issuer_hex = hex::encode(keypair.public_key());
    let file_ids = [hex::encode([0x11; 32]), hex::encode([0x22; 32])];
    let json_value = norito::json::Value::Array(
        file_ids
            .iter()
            .cloned()
            .map(norito::json::Value::String)
            .collect(),
    );
    let json = norito::json::to_string_pretty(&json_value).expect("serialise revocations");
    let file = NamedTempFile::new().expect("revocation file");
    std::fs::write(file.path(), format!("{json}\n")).expect("write revocation file");
    let mut cfg = TokenConfig {
        enabled: true,
        issuer_public_key_hex: Some(issuer_hex),
        max_ttl_secs: 300,
        clock_skew_secs: 5,
        replay_store_path: file.path().with_extension("replays.norito"),
        revocation_list_hex: vec![hex::encode([0x33; 32])],
        revocation_list_path: Some(file.path().to_path_buf()),
        ..TokenConfig::default()
    };
    cfg.apply_defaults();
    cfg.validate().expect("token config validates");
    let policy = cfg
        .build_policy()
        .expect("build policy")
        .expect("policy enabled");
    assert_eq!(policy.revocations.len(), 3);
    let expected_fp = compute_issuer_fingerprint(keypair.public_key());
    assert_eq!(policy.verifier.issuer_fingerprint(), &expected_fp);
}
#[test]
fn certificate_config_rejects_all_zero_issuer_ed25519_key_material() {
    let config = CertificateConfig {
        bundle_path: PathBuf::from("relay.cbor"),
        issuer_ed25519_hex: hex::encode([0u8; 32]),
        issuer_mldsa_hex: hex::encode(vec![0x55; MlDsaSuite::MlDsa65.public_key_len()]),
    };
    match config.parse_issuer_ed25519() {
        Err(ConfigError::Certificate { message, .. }) => assert!(
            message.contains("all zero"),
            "unexpected certificate config error: {message}"
        ),
        other => panic!("expected certificate config error, got {other:?}"),
    }
}
#[test]
fn certificate_config_rejects_small_order_issuer_ed25519_key_material() {
    const SMALL_ORDER_ED25519_POINT: [u8; 32] = [
        1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];
    let config = CertificateConfig {
        bundle_path: PathBuf::from("relay.cbor"),
        issuer_ed25519_hex: hex::encode(SMALL_ORDER_ED25519_POINT),
        issuer_mldsa_hex: hex::encode(vec![0x55; MlDsaSuite::MlDsa65.public_key_len()]),
    };
    match config.parse_issuer_ed25519() {
        Err(ConfigError::Certificate { message, .. }) => assert!(
            message.contains("small-order"),
            "unexpected certificate config error: {message}"
        ),
        other => panic!("expected certificate config error, got {other:?}"),
    }
}
#[test]
fn certificate_config_requires_mldsa65_issuer_key() {
    let config = CertificateConfig {
        bundle_path: PathBuf::from("relay.cbor"),
        issuer_ed25519_hex: hex::encode([0x44; 32]),
        issuer_mldsa_hex: String::new(),
    };
    let err = config
        .validate()
        .expect_err("first-release certificate policy must require ML-DSA-65");
    assert!(
        matches!(&err, ConfigError::Handshake(message) if message.contains("dual-signature policy")),
        "unexpected certificate configuration error: {err:?}"
    );
}
#[test]
fn token_config_rejects_invalid_issuer_key_without_panic() {
    let replay_file = NamedTempFile::new().expect("replay path");
    let replay_store_path = replay_file.path().with_extension("replays.norito");
    let mut cfg = TokenConfig {
        enabled: true,
        issuer_public_key_hex: Some(hex::encode([0x42; 32])),
        replay_store_path,
        ..TokenConfig::default()
    };
    cfg.apply_defaults();
    cfg.validate().expect("token config validates");
    match cfg.build_policy() {
        Err(ConfigError::Token(message)) => assert!(
            message.contains("invalid pow.token.issuer_public_key_hex verifier key"),
            "unexpected token config error: {message}"
        ),
        other => panic!("expected token config error, got {other:?}"),
    }
}
#[test]
fn token_config_rejects_overflowing_replay_retention_window() {
    let cfg = TokenConfig {
        enabled: true,
        issuer_public_key_hex: Some(hex::encode([0x42; 32])),
        max_ttl_secs: u64::MAX,
        clock_skew_secs: 1,
        replay_store_capacity: 1,
        replay_store_path: PathBuf::from("unused.norito"),
        ..TokenConfig::default()
    };
    match cfg.build_policy() {
        Err(ConfigError::Token(message)) => {
            assert!(message.contains("max_ttl_secs + 2 * clock_skew_secs"));
            assert!(message.contains("overflow"));
        }
        other => panic!("expected token retention overflow, got {other:?}"),
    }
}
#[test]
fn token_replay_store_capacity_enforces_first_release_ceiling() {
    let exact = TokenConfig {
        replay_store_capacity: TOKEN_STORE_MAX_ENTRIES_V1,
        ..TokenConfig::default()
    };
    exact
        .validate()
        .expect("exact first-release capacity validates");
    let excessive = TokenConfig {
        replay_store_capacity: TOKEN_STORE_MAX_ENTRIES_V1 + 1,
        ..TokenConfig::default()
    };
    assert!(matches!(
        excessive.validate(),
        Err(ConfigError::Token(message))
            if message.contains("replay_store_capacity")
                && message.contains("first-release limit")
    ));
}
#[test]
fn token_replay_retention_covers_both_clock_skew_edges() {
    let cfg = TokenConfig {
        max_ttl_secs: 900,
        clock_skew_secs: 5,
        ..TokenConfig::default()
    };
    assert_eq!(cfg.replay_retention_secs().expect("retention"), 910);
}
#[test]
fn token_policy_rejects_missing_issuer_key_without_panic() {
    let cfg = TokenConfig {
        enabled: true,
        ..TokenConfig::default()
    };
    match cfg.build_policy() {
        Err(ConfigError::Token(message)) => assert!(
            message.contains("pow.token.issuer_public_key_hex must be set"),
            "unexpected token config error: {message}"
        ),
        other => panic!("expected token config error, got {other:?}"),
    }
}
#[test]
fn privacy_config_overrides_are_applied() {
    let json = config_fixture!("privacy_overrides.json");
    let path = write_config(json);
    let cfg = RelayConfig::load(path).expect("load config");
    let privacy = cfg.privacy_config();
    assert_eq!(privacy.bucket_secs, 90);
    assert_eq!(privacy.min_handshakes, 8);
    assert_eq!(privacy.flush_delay_buckets, 2);
    assert_eq!(privacy.force_flush_buckets, 4);
    assert_eq!(privacy.max_completed_buckets, 12);
    assert_eq!(privacy.expected_shares, 3);
    assert_eq!(privacy.event_buffer_capacity, 2_048);
}
#[test]
fn privacy_config_validates_force_flush_ordering() {
    let json = config_fixture!("privacy_flush_order.json");
    let path = write_config(json);
    let err = RelayConfig::load(path).expect_err("expected privacy validation error");
    match err {
        ConfigError::Privacy(message) => {
            assert!(
                message.contains("force_flush_buckets"),
                "unexpected privacy error: {message}"
            );
        }
        other => panic!("unexpected error {other:?}"),
    }
}

#[test]
fn privacy_config_rejects_zero_force_flush_window() {
    let mut config = PrivacyTelemetryConfig {
        flush_delay_buckets: 0,
        force_flush_buckets: 0,
        ..PrivacyTelemetryConfig::default()
    };
    assert!(matches!(
        config.apply_defaults(),
        Err(ConfigError::Privacy(message))
            if message == "privacy.force_flush_buckets must be greater than zero"
    ));
}
#[test]
fn privacy_config_enforces_first_release_memory_limits() {
    let mut exact = PrivacyTelemetryConfig {
        flush_delay_buckets: PRIVACY_MAX_OPEN_BUCKETS_V1,
        force_flush_buckets: PRIVACY_MAX_OPEN_BUCKETS_V1,
        max_completed_buckets: PRIVACY_MAX_COMPLETED_BUCKETS_V1,
        expected_shares: PRIVACY_MAX_EXPECTED_SHARES_V1,
        event_buffer_capacity: PRIVACY_EVENT_BUFFER_MAX_CAPACITY_V1,
        ..PrivacyTelemetryConfig::default()
    };
    exact.apply_defaults().expect("exact privacy limits");
    let mut overflow = exact.clone();
    overflow.max_completed_buckets = PRIVACY_MAX_COMPLETED_BUCKETS_V1 + 1;
    assert!(matches!(
        overflow.apply_defaults(),
        Err(ConfigError::Privacy(message)) if message.contains("max_completed_buckets")
    ));
    let mut overflow = exact.clone();
    overflow.event_buffer_capacity = PRIVACY_EVENT_BUFFER_MAX_CAPACITY_V1 + 1;
    assert!(matches!(
        overflow.apply_defaults(),
        Err(ConfigError::Privacy(message)) if message.contains("event_buffer_capacity")
    ));
    let mut overflow = exact.clone();
    overflow.expected_shares = PRIVACY_MAX_EXPECTED_SHARES_V1 + 1;
    assert!(matches!(
        overflow.apply_defaults(),
        Err(ConfigError::Privacy(message)) if message.contains("expected_shares")
    ));
    let mut overflow = exact;
    overflow.force_flush_buckets = PRIVACY_MAX_OPEN_BUCKETS_V1 + 1;
    assert!(matches!(
        overflow.apply_defaults(),
        Err(ConfigError::Privacy(message)) if message.contains("flush windows")
    ));
}
#[test]
fn vpn_route_push_rejects_invalid_cidr() {
    let mut cfg = VpnConfig {
        enabled: true,
        route_push: vec!["not-a-cidr".to_string()],
        ..VpnConfig::default()
    };
    let err = cfg
        .validate()
        .expect_err("expected CIDR validation failure");
    match err {
        ConfigError::Vpn(message) => {
            assert!(
                message.contains("CIDR"),
                "unexpected vpn route error: {message}"
            );
        }
        other => panic!("unexpected error {other:?}"),
    }
}
#[test]
fn vpn_dns_override_rejects_non_ip() {
    let mut cfg = VpnConfig {
        enabled: true,
        dns_overrides: vec!["example.com".to_string()],
        ..VpnConfig::default()
    };
    let err = cfg.validate().expect_err("expected dns validation failure");
    match err {
        ConfigError::Vpn(message) => {
            assert!(
                message.contains("dns_overrides"),
                "unexpected vpn dns error: {message}"
            );
        }
        other => panic!("unexpected error {other:?}"),
    }
}
#[test]
fn vpn_cover_ratio_allows_zero_when_enabled() {
    let (mut cfg, _helper_secret) = vpn_config_with_secret(0xAB);
    cfg.cover = VpnCoverTrafficConfig {
        enabled: true,
        cover_to_data_per_mille: 0,
        heartbeat_ms: 10,
        max_cover_burst: 1,
        max_jitter_millis: 1,
    };
    cfg.validate().expect("vpn config should validate");
    assert_eq!(cfg.cover.cover_to_data_per_mille, 0);
}
#[test]
fn vpn_helper_ticket_secret_loads_private_file() {
    let (mut cfg, _helper_secret) = vpn_config_with_secret(0xAB);
    cfg.validate().expect("vpn config should validate");
    assert_eq!(
        cfg.try_helper_ticket_secret_bytes()
            .expect("read helper secret"),
        Some([0xAB; 32])
    );
}
#[test]
fn vpn_helper_ticket_replay_store_defaults_are_mandatory() {
    let (mut cfg, _helper_secret) = vpn_config_with_secret(0xAB);
    cfg.helper_ticket_replay_store_capacity = 0;
    cfg.helper_ticket_replay_store_path = PathBuf::new();
    cfg.validate().expect("VPN replay-store defaults validate");
    assert_eq!(
        cfg.helper_ticket_replay_store_capacity,
        DEFAULT_VPN_HELPER_TICKET_REPLAY_STORE_CAPACITY
    );
    assert_eq!(
        cfg.helper_ticket_replay_store_path,
        PathBuf::from("./storage/soranet/vpn_helper_ticket_replays.norito")
    );
}
#[test]
fn vpn_helper_ticket_replay_store_preserves_operator_settings() {
    let (mut cfg, _helper_secret) = vpn_config_with_secret(0xAB);
    cfg.helper_ticket_replay_store_capacity = 32_768;
    cfg.helper_ticket_replay_store_path =
        PathBuf::from("/var/lib/soranet/vpn-helper-replays.norito");
    cfg.validate().expect("custom VPN replay store validates");
    assert_eq!(cfg.helper_ticket_replay_store_capacity, 32_768);
    assert_eq!(
        cfg.helper_ticket_replay_store_path,
        PathBuf::from("/var/lib/soranet/vpn-helper-replays.norito")
    );
}
#[test]
fn vpn_helper_ticket_replay_capacity_enforces_first_release_ceiling() {
    let mut exact = VpnConfig {
        helper_ticket_replay_store_capacity: REPLAY_LEDGER_MAX_ENTRIES_V1,
        ..VpnConfig::default()
    };
    exact
        .validate()
        .expect("exact first-release capacity validates");
    let mut excessive = VpnConfig {
        helper_ticket_replay_store_capacity: REPLAY_LEDGER_MAX_ENTRIES_V1 + 1,
        ..VpnConfig::default()
    };
    assert!(matches!(
        excessive.validate(),
        Err(ConfigError::Vpn(message))
            if message.contains("helper_ticket_replay_store_capacity")
                && message.contains("first-release limit")
    ));
}
#[test]
fn vpn_helper_ticket_secret_rejects_short_file() {
    let file = NamedTempFile::new_in(std::env::current_dir().expect("current directory"))
        .expect("create short VPN secret");
    std::fs::write(file.path(), "ab".repeat(16)).expect("write short VPN secret");
    let cfg = VpnConfig {
        helper_ticket_secret_path: Some(file.path().to_path_buf()),
        ..VpnConfig::default()
    };
    let err = cfg
        .try_helper_ticket_secret_bytes()
        .expect_err("short helper ticket secret must fail");
    assert!(
        matches!(err, ConfigError::Vpn(message) if message.contains("64 lowercase hexadecimal"))
    );
}
#[test]
fn vpn_shared_secrets_require_canonical_nonzero_encoding() {
    for (contents, expected) in [
        ("AB".repeat(32), "lowercase"),
        (format!("{}\n", "ab".repeat(32)), "no newline"),
        ("00".repeat(32), "all-zero"),
    ] {
        let file = NamedTempFile::new_in(std::env::current_dir().expect("current directory"))
            .expect("create noncanonical VPN secret");
        std::fs::write(file.path(), contents).expect("write noncanonical VPN secret");
        let config = VpnConfig {
            helper_ticket_secret_path: Some(file.path().to_path_buf()),
            ..VpnConfig::default()
        };
        let error = config
            .try_helper_ticket_secret_bytes()
            .expect_err("noncanonical VPN secret must fail closed");
        assert!(
            error.to_string().contains(expected),
            "unexpected error for {expected}: {error}"
        );
    }
}
#[test]
fn vpn_rejects_retired_inline_secrets() {
    for field in ["helper_ticket_secret_hex", "backend_bootstrap_secret_hex"] {
        let json = format!(
            r#"{{
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "vpn": {{ "{field}": "{}" }}
            }}"#,
            "ab".repeat(32)
        );
        let path = write_config(&json);
        RelayConfig::load(path).expect_err("removed inline VPN secret field must be unknown");
    }
}
#[test]
fn vpn_backend_endpoint_normalizes_unix_endpoint() {
    let (mut cfg, _helper_secret) = vpn_config_with_secret(0xAB);
    cfg.backend_endpoint = Some(" unix:/run/sora-vpn-backend.sock ".to_string());
    cfg.validate().expect("vpn config should validate");
    assert_eq!(
        cfg.backend_endpoint,
        Some("unix:/run/sora-vpn-backend.sock".to_string())
    );
    assert_eq!(
        cfg.backend_endpoint(),
        Some(VpnBackendEndpoint::Unix(PathBuf::from(
            "/run/sora-vpn-backend.sock"
        )))
    );
}
#[test]
fn vpn_backend_endpoint_requires_secret_for_unix_and_tcp() {
    let (mut cfg, _helper_secret) = vpn_config_with_secret(0xAB);
    cfg.backend_bootstrap_secret_path = None;
    let unix_error = cfg
        .validate()
        .expect_err("Unix endpoint must require bootstrap authentication");
    assert!(
        matches!(unix_error, ConfigError::Vpn(message) if message.contains("backend_bootstrap_secret_path"))
    );
    cfg.backend_endpoint = Some("tcp://127.0.0.1:19090".to_string());
    let err = cfg
        .validate()
        .expect_err("expected tcp bootstrap secret validation failure");
    assert!(
        matches!(err, ConfigError::Vpn(message) if message.contains("backend_bootstrap_secret_path"))
    );
    let bootstrap_secret = write_vpn_secret(0xCD);
    cfg.backend_bootstrap_secret_path = Some(bootstrap_secret.path().to_path_buf());
    cfg.validate().expect("tcp endpoint with secret");
    assert_eq!(
        cfg.backend_endpoint(),
        Some(VpnBackendEndpoint::Tcp("127.0.0.1:19090".to_string()))
    );
    assert_eq!(
        cfg.try_backend_bootstrap_secret_bytes()
            .expect("read bootstrap secret"),
        Some([0xCD; 32])
    );
}
#[test]
fn vpn_backend_tcp_endpoint_rejects_non_loopback_transport() {
    let (mut cfg, _helper_secret) = vpn_config_with_secret(0xAB);
    let bootstrap_secret = write_vpn_secret(0xCD);
    cfg.backend_endpoint = Some("tcp://192.0.2.1:19090".to_string());
    cfg.backend_bootstrap_secret_path = Some(bootstrap_secret.path().to_path_buf());
    let err = cfg
        .validate()
        .expect_err("remote TCP backend must fail closed");
    assert!(
        matches!(&err, ConfigError::Vpn(message) if message.contains("loopback")),
        "unexpected remote backend error: {err:?}"
    );
}
#[test]
fn vpn_fallible_accessors_decode_valid_private_files() {
    let (mut cfg, _helper_secret) = vpn_config_with_secret(0xAB);
    let bootstrap_secret = write_vpn_secret(0xCD);
    cfg.backend_endpoint = Some(" tcp://127.0.0.1:19090 ".to_string());
    cfg.backend_bootstrap_secret_path = Some(bootstrap_secret.path().to_path_buf());
    cfg.billing = VpnBillingConfig {
        meter_hash_hex: "ef".repeat(32),
        ..VpnBillingConfig::default()
    };
    cfg.validate().expect("vpn config validates");
    assert_eq!(cfg.try_meter_hash_bytes().expect("meter hash"), [0xEF; 32]);
    assert_eq!(
        cfg.try_helper_ticket_secret_bytes().expect("helper secret"),
        Some([0xAB; 32])
    );
    assert_eq!(
        cfg.try_backend_endpoint().expect("backend endpoint"),
        Some(VpnBackendEndpoint::Tcp("127.0.0.1:19090".to_string()))
    );
    assert_eq!(
        cfg.try_backend_bootstrap_secret_bytes()
            .expect("bootstrap secret"),
        Some([0xCD; 32])
    );
    let overlay = VpnOverlay::try_from_config(cfg).expect("vpn overlay");
    assert_eq!(overlay.meter_hash(), [0xEF; 32]);
}
#[test]
fn vpn_overlay_try_from_config_rejects_invalid_helper_secret_without_panic() {
    let file = NamedTempFile::new_in(std::env::current_dir().expect("current directory"))
        .expect("create invalid helper secret");
    std::fs::write(file.path(), "not-hex").expect("write invalid helper secret");
    let cfg = VpnConfig {
        enabled: true,
        helper_ticket_secret_path: Some(file.path().to_path_buf()),
        backend_bootstrap_secret_path: Some(file.path().to_path_buf()),
        ..VpnConfig::default()
    };
    match VpnOverlay::try_from_config(cfg) {
        Err(ConfigError::Vpn(message)) => assert!(
            message.contains("64 lowercase hexadecimal"),
            "unexpected vpn config error: {message}"
        ),
        other => panic!("expected vpn config error, got {other:?}"),
    }
}
#[test]
fn vpn_backend_endpoint_rejects_invalid_endpoint() {
    let (mut cfg, _helper_secret) = vpn_config_with_secret(0xAB);
    cfg.backend_endpoint = Some("not-a-socket".to_string());
    let err = cfg
        .validate()
        .expect_err("expected backend endpoint validation failure");
    match err {
        ConfigError::Vpn(message) => {
            assert!(
                message.contains("backend_endpoint"),
                "unexpected vpn backend endpoint error: {message}"
            );
        }
        other => panic!("unexpected error {other:?}"),
    }
}
#[test]
fn vpn_receipt_spool_dir_preserves_operator_path() {
    let (mut cfg, _helper_secret) = vpn_config_with_secret(0xAB);
    cfg.receipt_spool_dir = Some(PathBuf::from("/var/spool/soranet/vpn-receipts"));
    cfg.validate().expect("vpn config should validate");
    assert_eq!(
        cfg.receipt_spool_dir.as_deref(),
        Some(Path::new("/var/spool/soranet/vpn-receipts"))
    );
}
#[test]
fn vpn_control_plane_threads_routes_and_dns() {
    let (mut cfg, _helper_secret) = vpn_config_with_secret(0xAB);
    cfg.lease_secs = 45;
    cfg.route_push = vec!["10.0.0.0/24".to_string()];
    cfg.dns_overrides = vec!["1.1.1.1".to_string()];
    cfg.validate().expect("vpn config validates");
    let overlay = VpnOverlay::from_config(cfg);
    let entry_guard = [0xAA; 32];
    let exit_guard = [0xBB; 32];
    let envelope = overlay.control_plane_envelope(entry_guard, exit_guard);
    assert_eq!(envelope.entry_guard, entry_guard);
    assert_eq!(envelope.exit_guard, exit_guard);
    assert_eq!(envelope.lease_seconds, 45);
    assert_eq!(envelope.exit_class, VpnExitClassV1::Standard);
    assert_eq!(envelope.dns_servers, vec!["1.1.1.1".to_string()]);
    assert_eq!(envelope.routes.len(), 1);
    assert_eq!(envelope.routes[0].cidr, "10.0.0.0/24");
}
