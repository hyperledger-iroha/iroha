// Test body included from the parent module to keep its production source budget bounded.
use super::*;
use crate::{
    incentive_log::{INCENTIVE_MAX_TRUSTED_VERIFIERS_V1, IncentiveLogError},
    incentives::{
        INCENTIVE_DEFAULT_ACTIVE_EPOCHS, INCENTIVE_DEFAULT_MEASUREMENTS_PER_EPOCH,
        INCENTIVE_MAX_ACTIVE_EPOCHS_V1, INCENTIVE_MAX_RETAINED_MEASUREMENTS_V1,
    },
    vpn::VpnOverlay,
};
use hex::FromHex;
use iroha_crypto::KeyPair;
use iroha_data_model::account::AccountId;
use std::collections::BTreeSet;
use tempfile::{NamedTempFile, TempDir};
macro_rules! config_fixture {
    ($name:literal) => {
        concat!(
            include_str!(concat!("config_tests/fixtures/", $name)),
            "        "
        )
    };
}
fn write_config(json: &str) -> NamedTempFile {
    let file = NamedTempFile::new_in(std::env::current_dir().expect("current directory"))
        .expect("create temp file");
    std::fs::write(file.path(), json).expect("write config");
    file
}
fn write_manifest(json: &str) -> NamedTempFile {
    let file = NamedTempFile::new_in(std::env::current_dir().expect("current directory"))
        .expect("create manifest file");
    std::fs::write(file.path(), json).expect("write manifest");
    file
}
fn fixture_mldsa65_private_key_hex() -> &'static str {
    static PRIVATE_KEY_HEX: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    PRIVATE_KEY_HEX.get_or_init(|| {
        let key_pair = KeyPair::try_from_seed(
            b"soranet-relay-strict-manifest-test-mldsa65".to_vec(),
            Algorithm::MlDsa,
        )
        .expect("derive fixture ML-DSA-65 keypair");
        let (algorithm, mut private_key) = key_pair.private_key().to_bytes();
        assert_eq!(algorithm, Algorithm::MlDsa);
        assert_eq!(private_key.len(), MlDsaSuite::MlDsa65.secret_key_len());
        let encoded = hex::encode(&private_key);
        zeroize::Zeroize::zeroize(&mut private_key);
        encoded
    })
}
fn descriptor_manifest_identity_json(ed25519: &str, mldsa65: &str) -> String {
    format!(r#"{{"ed25519_private_key_hex":"{ed25519}","mldsa65_private_key_hex":"{mldsa65}"}}"#)
}
fn descriptor_manifest_json_from_hex(ed25519: &str, mldsa65: &str) -> String {
    let identity = descriptor_manifest_identity_json(ed25519, mldsa65);
    format!(r#"{{"version":1,"identity":{identity}}}"#)
}
fn valid_descriptor_manifest_json(ed25519_seed: [u8; ED25519_IDENTITY_SEED_LEN_V1]) -> String {
    descriptor_manifest_json_from_hex(
        &hex::encode(ed25519_seed),
        fixture_mldsa65_private_key_hex(),
    )
}
fn descriptor_manifest_error_message(json: &str) -> String {
    let manifest = write_manifest(json);
    let policy = HandshakePolicy {
        descriptor_manifest_path: Some(manifest.path().to_path_buf()),
        ..HandshakePolicy::default()
    };
    match policy
        .manifest_secrets()
        .expect_err("descriptor manifest must fail closed")
    {
        ConfigError::DescriptorManifest { message, .. } => message,
        other => panic!("unexpected descriptor manifest error: {other:?}"),
    }
}
fn write_vpn_issuer_public_key(byte: u8) -> NamedTempFile {
    let file = NamedTempFile::new_in(std::env::current_dir().expect("current directory"))
        .expect("create VPN issuer public-key file");
    let key_pair = KeyPair::try_from_seed(vec![byte; 32], Algorithm::Ed25519)
        .expect("derive VPN issuer keypair");
    let (_, payload) = key_pair
        .public_key()
        .try_to_bytes()
        .expect("encode VPN issuer public key");
    std::fs::write(file.path(), hex::encode(payload)).expect("write VPN issuer public-key file");
    file
}
fn write_vpn_secret(byte: u8) -> NamedTempFile {
    let file = NamedTempFile::new_in(std::env::current_dir().expect("current directory"))
        .expect("create VPN secret file");
    std::fs::write(file.path(), hex::encode([byte; 32])).expect("write VPN secret file");
    file
}
struct VpnConfigCredentials {
    _issuer: NamedTempFile,
    _receipt_spool: TempDir,
}
fn vpn_config_with_credentials(byte: u8) -> (VpnConfig, VpnConfigCredentials) {
    let file = write_vpn_issuer_public_key(byte);
    let receipt_spool = tempfile::tempdir_in(std::env::current_dir().expect("current directory"))
        .expect("create VPN receipt spool directory");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(receipt_spool.path(), std::fs::Permissions::from_mode(0o700))
            .expect("protect VPN receipt spool directory");
    }
    let backend_socket = receipt_spool.path().join("backend.sock");
    let config = VpnConfig {
        enabled: true,
        helper_ticket_issuer_public_key_path: Some(file.path().to_path_buf()),
        backend_endpoint: Some(format!("unix:{}", backend_socket.display())),
        backend_expected_uid: Some(0),
        backend_expected_gid: Some(0),
        backend_bootstrap_secret_path: Some(file.path().to_path_buf()),
        receipt_spool_dir: Some(receipt_spool.path().to_path_buf()),
        ..VpnConfig::default()
    };
    (
        config,
        VpnConfigCredentials {
            _issuer: file,
            _receipt_spool: receipt_spool,
        },
    )
}
fn assert_config_json_admission_rejected(bytes: &[u8]) {
    let file = NamedTempFile::new_in(std::env::current_dir().expect("current directory"))
        .expect("create admission input");
    std::fs::write(file.path(), bytes).expect("write admission input");
    let error = RelayConfig::load(file.path()).expect_err("JSON admission must reject input");
    assert!(
        matches!(error, ConfigError::JsonAdmission(_)),
        "unexpected error: {error:?}"
    );
}
#[test]
fn relay_config_file_limit_accepts_exact_and_rejects_plus_one() {
    let exact = NamedTempFile::new_in(std::env::current_dir().expect("current directory"))
        .expect("create exact config");
    let mut valid = br#"{"mode":"Entry","listen":"127.0.0.1:0"}"#.to_vec();
    valid.resize(RELAY_CONFIG_JSON_MAX_BYTES_V1, b' ');
    std::fs::write(exact.path(), &valid).expect("write exact config");
    let loaded = RelayConfig::load(exact.path()).expect("exact-limit config must load");
    assert_eq!(loaded.mode, RelayMode::Entry);
    let plus_one = NamedTempFile::new_in(std::env::current_dir().expect("current directory"))
        .expect("create oversized config");
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
    let directory = tempfile::tempdir_in(std::env::current_dir().expect("current directory"))
        .expect("create temp directory");
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
    let directory = tempfile::tempdir_in(std::env::current_dir().expect("current directory"))
        .expect("create temp directory");
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
fn relay_config_requires_trusted_leaf_mode_and_single_link() {
    use std::os::unix::fs::PermissionsExt as _;

    let directory = tempfile::tempdir_in(std::env::current_dir().expect("current directory"))
        .expect("create temp directory");
    let configured = directory.path().join("relay.json");
    let hardlink = directory.path().join("relay-hardlink.json");
    std::fs::write(&configured, br#"{"mode":"Entry","listen":"127.0.0.1:0"}"#)
        .expect("write config");
    std::fs::set_permissions(&configured, std::fs::Permissions::from_mode(0o664))
        .expect("make config group-writable");
    let error = RelayConfig::load(&configured).expect_err("writable config must fail closed");
    assert!(
        matches!(error, ConfigError::Io(ref source) if source.kind() == std::io::ErrorKind::PermissionDenied)
    );

    std::fs::set_permissions(&configured, std::fs::Permissions::from_mode(0o644))
        .expect("make config read-only to non-owner");
    RelayConfig::load(&configured).expect("non-writable 0644 config must be accepted");
    std::fs::hard_link(&configured, &hardlink).expect("create hard link");
    let error =
        RelayConfig::load(&configured).expect_err("multiply linked config must fail closed");
    assert!(
        matches!(error, ConfigError::Io(ref source) if source.kind() == std::io::ErrorKind::PermissionDenied)
    );
}
#[cfg(unix)]
#[test]
fn relay_config_rejects_relative_path() {
    let error = RelayConfig::load(std::path::Path::new("relative-relay-config.json"))
        .expect_err("relative config path must fail closed before open");
    assert!(
        matches!(error, ConfigError::Io(ref source) if source.kind() == std::io::ErrorKind::PermissionDenied),
        "unexpected error: {error:?}"
    );
}

#[cfg(unix)]
#[test]
fn relay_config_rejects_group_writable_parent() {
    use std::os::unix::fs::PermissionsExt as _;

    let directory = tempfile::tempdir_in(std::env::current_dir().expect("current directory"))
        .expect("create temp directory");
    let unsafe_parent = directory.path().join("unsafe-parent");
    std::fs::create_dir(&unsafe_parent).expect("create unsafe parent");
    let configured = unsafe_parent.join("relay.json");
    std::fs::write(&configured, br#"{"mode":"Entry","listen":"127.0.0.1:0"}"#)
        .expect("write config");
    std::fs::set_permissions(&unsafe_parent, std::fs::Permissions::from_mode(0o770))
        .expect("make parent group-writable");
    let error = RelayConfig::load(&configured).expect_err("writable parent must fail closed");
    assert!(
        matches!(error, ConfigError::Io(ref source) if source.kind() == std::io::ErrorKind::PermissionDenied),
        "unexpected error: {error:?}"
    );
    std::fs::set_permissions(&unsafe_parent, std::fs::Permissions::from_mode(0o700))
        .expect("restore parent custody");
}
#[cfg(unix)]
#[test]
fn relay_config_pins_canonical_parent_before_opening_through_alias() {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir_in(std::env::current_dir().expect("current directory"))
        .expect("create temp directory");
    let real_parent = directory.path().join("real-parent");
    std::fs::create_dir(&real_parent).expect("create real parent");
    let configured = real_parent.join("relay.json");
    std::fs::write(&configured, br#"{"mode":"Entry","listen":"127.0.0.1:0"}"#)
        .expect("write config");
    let alias = directory.path().join("alias-parent");
    symlink(&real_parent, &alias).expect("create parent symlink");
    let config = RelayConfig::load(alias.join("relay.json"))
        .expect("stable canonical parent must be accepted");
    assert_eq!(config.mode, RelayMode::Entry);
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
#[allow(unsafe_code)]
fn bounded_reader_sensitive_buffer_can_be_explicitly_cleared() {
    let mut allocation = Vec::with_capacity(64);
    allocation.resize(allocation.capacity(), 0xA5);
    allocation.truncate(17);
    let capacity = allocation.capacity();
    let mut buffer = SensitiveReadBuffer(allocation);
    buffer.clear();
    assert_eq!(buffer.0.len(), 17);
    assert!(buffer.0.iter().all(|byte| *byte == 0));
    // SAFETY: `SensitiveReadBuffer::clear` initializes and wipes every byte
    // through the allocation's capacity before restoring its logical length.
    unsafe { buffer.0.set_len(capacity) };
    assert!(buffer.0.iter().all(|byte| *byte == 0));
    buffer.0.truncate(17);

    let mut probe = SensitiveReadProbe([0xA5]);
    probe.clear();
    assert_eq!(probe.0, [0]);
}
#[test]
#[allow(unsafe_code)]
fn sensitive_byte_scrubber_overwrites_spare_capacity() {
    let mut bytes = Vec::with_capacity(64);
    bytes.resize(bytes.capacity(), 0xA5);
    bytes.truncate(17);
    let capacity = bytes.capacity();

    clear_sensitive_bytes(&mut bytes);

    assert_eq!(bytes.len(), 17);
    assert!(bytes.iter().all(|byte| *byte == 0));
    // SAFETY: `clear_sensitive_bytes` initializes and overwrites every byte
    // through the allocation's capacity before restoring its logical length.
    unsafe { bytes.set_len(capacity) };
    assert!(bytes.iter().all(|byte| *byte == 0));
    bytes.truncate(17);

    let mut owner = PrivateFileBytes::from(vec![0xA5; 17]);
    assert_eq!(format!("{owner:?}"), "<redacted private file bytes>");
    owner.clear();
    assert!(owner.is_empty());
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
    let mut manifest =
        valid_descriptor_manifest_json([0x11; ED25519_IDENTITY_SEED_LEN_V1]).into_bytes();
    manifest.resize(DESCRIPTOR_MANIFEST_JSON_MAX_BYTES_V1, b' ');
    std::fs::write(exact.path(), manifest).expect("write exact manifest");
    let mut policy = HandshakePolicy {
        descriptor_manifest_path: Some(exact.path().to_path_buf()),
        ..HandshakePolicy::default()
    };
    let secrets = policy
        .manifest_secrets()
        .expect("exact-limit manifest must load");
    let (ed25519, mldsa65) = secrets.into_private_keys();
    assert_eq!(ed25519, [0x11; ED25519_IDENTITY_SEED_LEN_V1]);
    assert_eq!(mldsa65.len(), MlDsaSuite::MlDsa65.secret_key_len());
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
fn descriptor_manifest_is_required_by_first_release_loader() {
    let error = HandshakePolicy::default()
        .manifest_secrets()
        .expect_err("first-release relay identity manifest must be required");
    assert!(
        matches!(error, ConfigError::Handshake(ref message) if message.contains("descriptor_manifest_path")),
        "unexpected error: {error:?}"
    );
}
#[test]
fn descriptor_manifest_preflight_bounds_nesting() {
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
        valid_descriptor_manifest_json([0x11; ED25519_IDENTITY_SEED_LEN_V1]),
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
fn certificate_bundle_is_required_by_first_release_loader() {
    let error = HandshakePolicy::default()
        .load_certificate_bundle_at(0)
        .expect_err("first-release relay certificate bundle must be required");
    assert!(
        matches!(error, ConfigError::Handshake(ref message) if message.contains("handshake.certificate")),
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
fn strict_identity_manifest_loads_exact_v1_key_material() {
    let manifest_json = valid_descriptor_manifest_json([0x11; ED25519_IDENTITY_SEED_LEN_V1]);
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
    let secrets = policy.manifest_secrets().expect("manifest secrets");
    let (ed25519, mldsa65) = secrets.into_private_keys();
    assert_eq!(ed25519, [0x11; ED25519_IDENTITY_SEED_LEN_V1]);
    assert_eq!(
        mldsa65,
        hex::decode(fixture_mldsa65_private_key_hex()).unwrap()
    );
}
#[test]
fn strict_identity_manifest_rejects_aliases_nesting_versions_types_and_unknown_fields() {
    let seed = "22".repeat(ED25519_IDENTITY_SEED_LEN_V1);
    let mldsa65 = fixture_mldsa65_private_key_hex();
    let identity = descriptor_manifest_identity_json(&seed, mldsa65);
    let cases = [
        ("[]".to_owned(), "root must be a JSON object"),
        (
            format!(r#"{{"identity":{identity}}}"#),
            "missing required field `version`",
        ),
        (
            format!(r#"{{"version":2,"identity":{identity}}}"#),
            "`version` must be the integer 1",
        ),
        (
            format!(r#"{{"version":"1","identity":{identity}}}"#),
            "`version` must be the integer 1",
        ),
        (
            format!(r#"{{"version":1,"identity":{identity},"metadata":{{}}}}"#),
            "root contains unknown field `metadata`",
        ),
        (
            r#"{"version":1,"identity":[]}"#.to_owned(),
            "`identity` must be a JSON object",
        ),
        (
            format!(
                r#"{{"version":1,"identity":{{"ed25519_private_key_hex":"{seed}","mldsa65_private_key_hex":"{mldsa65}","rotation":1}}}}"#
            ),
            "identity contains unknown field `rotation`",
        ),
        (
            format!(r#"{{"version":1,"relay":{{"identity":{identity}}}}}"#),
            "root contains unknown field `relay`",
        ),
        (
            format!(
                r#"{{"version":1,"identity":{{"private_key_hex":"{seed}","mldsa65_private_key_hex":"{mldsa65}"}}}}"#
            ),
            "identity contains unknown field `private_key_hex`",
        ),
        (
            format!(r#"{{"version":1,"identity_private_key_hex":"{seed}","identity":{identity}}}"#),
            "root contains unknown field `identity_private_key_hex`",
        ),
        (
            format!(
                r#"{{"version":1,"identity":{{"ed25519_private_key_hex":7,"mldsa65_private_key_hex":"{mldsa65}"}}}}"#
            ),
            "`identity.ed25519_private_key_hex` must be a string",
        ),
        (
            format!(
                r#"{{"version":1,"identity":{{"ed25519_private_key_hex":"{seed}","mldsa65_private_key_hex":7}}}}"#
            ),
            "`identity.mldsa65_private_key_hex` must be a string",
        ),
        (
            format!(
                r#"{{"version":1,"identity":{{"ed25519_private_key_hex":"{seed}","mldsa65_private_key_hex":"{mldsa65}","ml_kem_private_key_hex":"11"}}}}"#
            ),
            "identity contains unknown field `ml_kem_private_key_hex`",
        ),
        (
            format!(
                r#"{{"version":1,"identity":{{"ed25519_private_key_hex":"{seed}","mldsa65_private_key_hex":"{mldsa65}","ml_kem_public_hex":"11"}}}}"#
            ),
            "identity contains unknown field `ml_kem_public_hex`",
        ),
    ];
    for (manifest, expected) in cases {
        let message = descriptor_manifest_error_message(&manifest);
        assert!(
            message.contains(expected),
            "expected `{expected}` in `{message}`"
        );
    }
}
#[test]
fn strict_identity_manifest_rejects_noncanonical_or_inert_private_material() {
    let seed = "ab".repeat(ED25519_IDENTITY_SEED_LEN_V1);
    let mldsa65 = fixture_mldsa65_private_key_hex();
    let uppercase_seed = seed.to_ascii_uppercase();
    let uppercase_mldsa65 = mldsa65.to_ascii_uppercase();
    let cases = [
        (
            descriptor_manifest_json_from_hex(&"00".repeat(ED25519_IDENTITY_SEED_LEN_V1), mldsa65),
            "ed25519_private_key_hex must not be all zero",
        ),
        (
            descriptor_manifest_json_from_hex(&uppercase_seed, mldsa65),
            "canonical lowercase hexadecimal",
        ),
        (
            descriptor_manifest_json_from_hex("11", mldsa65),
            "ed25519_private_key_hex must contain exactly 64",
        ),
        (
            descriptor_manifest_json_from_hex(
                &seed,
                &"00".repeat(MlDsaSuite::MlDsa65.secret_key_len()),
            ),
            "mldsa65_private_key_hex must not be all zero",
        ),
        (
            descriptor_manifest_json_from_hex(&seed, &uppercase_mldsa65),
            "canonical lowercase hexadecimal",
        ),
        (
            descriptor_manifest_json_from_hex(&seed, "11"),
            "mldsa65_private_key_hex must contain exactly 8064",
        ),
        (
            descriptor_manifest_json_from_hex(
                &seed,
                &"11".repeat(MlDsaSuite::MlDsa65.secret_key_len()),
            ),
            "does not encode a valid ML-DSA-65 private key",
        ),
    ];
    for (manifest, expected) in cases {
        let message = descriptor_manifest_error_message(&manifest);
        assert!(
            message.contains(expected),
            "expected `{expected}` in `{message}`"
        );
    }
}
#[test]
fn manifest_secret_debug_is_redacted_and_private_material_can_be_cleared() {
    let mut secrets = ManifestSecrets {
        ed25519_private_key: [171; ED25519_IDENTITY_SEED_LEN_V1],
        mldsa65_private_key: vec![205; 4],
    };
    let rendered = format!("{secrets:?}");
    assert!(rendered.contains("<redacted>"));
    assert!(!rendered.contains("171"));
    assert!(!rendered.contains("205"));
    secrets.clear_private_material();
    assert_eq!(
        secrets.ed25519_private_key,
        [0; ED25519_IDENTITY_SEED_LEN_V1]
    );
    assert_eq!(secrets.mldsa65_private_key, vec![0; 4]);
}
#[test]
fn load_minimal_structural_config_in_test_build() {
    let json = config_fixture!("self_signed.json");
    let path = write_config(json);
    let cfg = RelayConfig::load(path).expect("load config");
    assert_eq!(cfg.mode, RelayMode::Entry);
    assert_eq!(cfg.listen_addr().unwrap().port(), 0);
    assert_eq!(cfg.pow_config().difficulty, 18);
    assert_eq!(cfg.pow_config().max_future_skew_secs, 300);
    assert_eq!(cfg.pow_config().min_ticket_ttl_secs, 30);
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
    let mut config: RelayConfig =
        norito::json::from_str(config_json).expect("parse incomplete VPN trust fixture");
    // These cases exercise relay-level VPN trust requirements. Keep the VPN
    // component itself valid so newly mandatory backend custody fields do not
    // mask the cross-field error under test.
    let (vpn, _credentials) = vpn_config_with_credentials(0xAB);
    config.vpn = Some(vpn);
    let error = config
        .validate()
        .expect_err("incomplete VPN trust must fail closed");
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
                    "helper_ticket_issuer_public_key_path": "/run/secrets/vpn-helper-ticket-issuer-public-key.hex",
                    "backend_bootstrap_secret_path": "/run/secrets/vpn-backend-bootstrap.hex"
                }
            }"#;
    assert_vpn_config_error(entry, "relay mode Exit");
    let missing_tls = r#"{
                "mode": "Exit",
                "listen": "127.0.0.1:0",
                "vpn": {
                    "enabled": true,
                    "helper_ticket_issuer_public_key_path": "/run/secrets/vpn-helper-ticket-issuer-public-key.hex",
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
                    "helper_ticket_issuer_public_key_path": "/run/secrets/vpn-helper-ticket-issuer-public-key.hex",
                    "backend_bootstrap_secret_path": "/run/secrets/vpn-backend-bootstrap.hex"
                }
            }"#;
    assert_vpn_config_error(missing_certificate, "verified handshake.certificate");
}
#[test]
fn vpn_requires_persistent_identity_and_authenticated_directory() {
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
                    "helper_ticket_issuer_public_key_path": "/run/secrets/vpn-helper-ticket-issuer-public-key.hex",
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
                    "helper_ticket_issuer_public_key_path": "/run/secrets/vpn-helper-ticket-issuer-public-key.hex",
                    "backend_bootstrap_secret_path": "/run/secrets/vpn-backend-bootstrap.hex"
                }}
            }}"#,
    );
    assert_vpn_config_error(&missing_directory, "authenticated guard_directory");
}
#[test]
fn every_guard_directory_rejects_the_retired_missing_entry_bypass() {
    for value in [false, true] {
        let config = format!(
            r#"{{
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "guard_directory": {{
                    "snapshot_path": "/run/secrets/guard-directory.norito",
                    "expected_snapshot_digest_hex": "{}",
                    "allow_missing_entry": {value}
                }}
            }}"#,
            "ee".repeat(32),
        );
        assert_config_json_admission_rejected(config.as_bytes());
    }
}
#[test]
fn relay_config_rejects_retired_self_signed_transport_field() {
    let config = br#"{
        "mode": "Entry",
        "listen": "127.0.0.1:0",
        "tls": { "self_signed_subject": "attacker-controlled.example" }
    }"#;
    assert_config_json_admission_rejected(config);
}
#[test]
fn production_transport_validation_rejects_incomplete_trust_chain() {
    let path = write_config(r#"{"mode":"Entry","listen":"127.0.0.1:0"}"#);
    let config = RelayConfig::load(path).expect("test-only structural config admission");
    let error = config
        .validate_production_transport()
        .expect_err("production transport cannot fall back to a self-signed identity");
    assert!(
        matches!(error, ConfigError::TlsPaths(ref message) if message.contains("production relays require")),
        "unexpected error: {error:?}"
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
                "pow": {{ "difficulty": 18 }},
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
fn constant_rate_capability_rejects_strict_mode_without_silent_downgrade() {
    let strict = ConstantRateCapabilityConfig {
        enabled: true,
        version: 1,
        strict: true,
    };
    let error = strict
        .validate()
        .expect_err("strict mode remains gated on bounded DATAGRAM entry accounting");
    assert!(
        error
            .to_string()
            .contains("Quinn 0.11.9 / quinn-proto 0.11.15")
            && error
                .to_string()
                .contains("payload bytes instead of entries"),
        "unexpected strict-mode rejection: {error}"
    );
    // Keep the requested value observable; validation must reject it rather
    // than silently advertising the weaker best-effort mode.
    assert_eq!(strict.capability().mode, ConstantRateMode::Strict);

    let best_effort = ConstantRateCapabilityConfig {
        enabled: true,
        version: 1,
        strict: false,
    };
    best_effort
        .validate()
        .expect("best-effort cover traffic remains available");
    assert_eq!(best_effort.capability().mode, ConstantRateMode::BestEffort);
}
#[test]
fn strict_constant_rate_requires_core_profile_before_dependency_requalification() {
    for profile in [ConstantRateProfileName::Home, ConstantRateProfileName::Null] {
        let json = format!(
            r#"{{
                "mode": "Entry",
                "listen": "127.0.0.1:0",
                "pow": {{ "difficulty": 18 }},
                "constant_rate_capability": {{ "enabled": true, "strict": true }},
                "constant_rate_profile": "{}"
            }}"#,
            profile.as_str()
        );
        let error = RelayConfig::load(write_config(&json))
            .expect_err("a non-Core strict profile must fail before dependency qualification");
        match error {
            ConfigError::ConstantRateCapability(message) => {
                assert!(
                    message.contains("constant_rate_profile `core`"),
                    "{message}"
                );
                assert!(message.contains("5 ms"), "{message}");
                assert!(message.contains(profile.as_str()), "{message}");
            }
            other => panic!("unexpected error: {other:?}"),
        }
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
    let verifier = KeyPair::try_from_seed(vec![0x91; 32], Algorithm::Ed25519)
        .expect("derive trusted incentive verifier");
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
            trusted_verifier_ids: BTreeSet::from([AccountId::new(verifier.public_key().clone())]),
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
        trusted_verifier_ids: BTreeSet::new(),
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
fn enabled_incentive_ingestion_rejects_an_empty_verifier_roster() {
    let mut incentives = IncentiveLogConfig {
        enable: true,
        spool_dir: None,
        max_active_epochs: 1,
        max_measurements_per_epoch: 1,
        trusted_verifier_ids: BTreeSet::new(),
    };
    assert!(matches!(
        incentives.validate(),
        Err(IncentiveLogError::Config(message))
            if message.contains("trusted_verifier_ids") && message.contains("at least one")
    ));
}

#[test]
fn incentive_verifier_roster_accepts_sixty_four_and_rejects_sixty_five() {
    let verifier_id = |index: usize| {
        let seed = u8::try_from(index + 1).expect("fixture verifier index fits one byte");
        let verifier = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive deterministic incentive verifier");
        AccountId::new(verifier.public_key().clone())
    };
    let trusted_verifier_ids = (0..INCENTIVE_MAX_TRUSTED_VERIFIERS_V1)
        .map(verifier_id)
        .collect();
    let mut exact = IncentiveLogConfig {
        enable: true,
        spool_dir: None,
        max_active_epochs: 1,
        max_measurements_per_epoch: 1,
        trusted_verifier_ids,
    };
    exact.validate().expect("the exact verifier-roster limit");

    let mut overflow = exact;
    assert!(
        overflow
            .trusted_verifier_ids
            .insert(verifier_id(INCENTIVE_MAX_TRUSTED_VERIFIERS_V1))
    );
    assert!(matches!(
        overflow.validate(),
        Err(IncentiveLogError::Config(message))
            if message.contains("trusted_verifier_ids")
                && message.contains("first-release limit is 64")
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
                message.contains("wss://"),
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
                message.contains("wss://"),
                "unexpected routing error message: {message}"
            );
        }
        other => panic!("unexpected error {other:?}"),
    }
}
#[test]
fn exit_routing_validation_pins_canonical_wss_origins() {
    validate_wss_endpoint_v1("kaigi_stream.hub_ws_url", "wss://kaigi.example:443/hub")
        .expect("canonical TLS WebSocket endpoint");
    validate_wss_endpoint_v1("kaigi_stream.hub_ws_url", "wss://[2001:db8::1]:443/hub")
        .expect("canonical IPv6 TLS WebSocket endpoint");

    for hostile in [
        "ws://kaigi.example/hub",
        "wss://viewer:secret@kaigi.example/hub",
        "wss://kaigi.example/hub?access_token=secret",
        "wss://kaigi.example/hub#redirect",
        "wss://127.1/hub",
        "wss://KAIGI.example/hub",
        "wss://kaigi%2eexample/hub",
        "wss://kaigi.example:0/hub",
        "wss://kaigi.example:0443/hub",
        " wss://kaigi.example/hub",
        "wss://kaigi.example\\internal/hub",
    ] {
        assert!(
            validate_wss_endpoint_v1("kaigi_stream.hub_ws_url", hostile).is_err(),
            "hostile or ambiguous endpoint must fail closed: {hostile}"
        );
    }
}
#[test]
fn pow_defaults_match_first_release_admission_policy() {
    let pow = PowConfig::default();
    assert_eq!(pow.difficulty, u32::from(puzzle::DEFAULT_DIFFICULTY));
    assert_eq!(pow.puzzle, PuzzleConfig::default());
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
    assert_eq!(
        config.pow_config().difficulty,
        u32::from(puzzle::DEFAULT_DIFFICULTY)
    );
    assert_eq!(config.pow_config().puzzle, PuzzleConfig::default());
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
fn pow_config_rejects_retired_required_key_as_unknown() {
    let err = norito::json::from_str::<PowConfig>(r#"{"required":false}"#)
        .expect_err("the retired admission toggle must fail closed");
    let message = err.to_string();
    assert!(
        message.contains("unknown field") && message.contains("required"),
        "unexpected error: {message}"
    );
}
#[test]
fn pow_config_rejects_retired_adaptive_key_as_unknown() {
    let err = norito::json::from_str::<PowConfig>(r#"{"adaptive":{"enabled":true}}"#)
        .expect_err("the retired adaptive schema must fail closed");
    let message = err.to_string();
    assert!(
        message.contains("unknown field") && message.contains("adaptive"),
        "unexpected error: {message}"
    );
}
#[test]
fn puzzle_config_rejects_retired_enabled_key_as_unknown() {
    let err = norito::json::from_str::<PuzzleConfig>(r#"{"enabled":false}"#)
        .expect_err("the retired puzzle toggle must fail closed");
    let message = err.to_string();
    assert!(
        message.contains("unknown field") && message.contains("enabled"),
        "unexpected error: {message}"
    );
}
#[test]
fn quotas_for_mode_honours_overrides() {
    let mut pow = PowConfig {
        quotas: QuotaConfig {
            per_remote_burst: 100,
            per_remote_window_secs: 45,
            cooldown_secs: 15,
            max_entries: 2048,
        },
        quotas_per_mode: Some(HopQuotaOverrides {
            entry: Some(QuotaConfig {
                per_remote_burst: 5,
                per_remote_window_secs: 30,
                cooldown_secs: 9,
                max_entries: 1024,
            }),
            middle: None,
            exit: Some(QuotaConfig {
                per_remote_burst: 70,
                per_remote_window_secs: 0,
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
    assert_eq!(exit.cooldown_secs, QuotaConfig::default_cooldown_secs());
    assert_eq!(exit.max_entries, QuotaConfig::default_max_entries());
}
#[test]
fn quota_config_rejects_retired_descriptor_keys_as_unknown() {
    for field in ["per_descriptor_burst", "per_descriptor_window_secs"] {
        let document = format!(r#"{{"{field}":1}}"#);
        let error = norito::json::from_str::<QuotaConfig>(&document)
            .expect_err("retired descriptor quota key must fail closed");
        let message = error.to_string();
        assert!(
            message.contains("unknown field") && message.contains(field),
            "unexpected error for {field}: {message}"
        );
    }
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
fn quota_duration_horizon_accepts_boundary_and_rejects_overflow() {
    let exact = QuotaConfig {
        per_remote_window_secs: u64::MAX - 20,
        cooldown_secs: 20,
        ..QuotaConfig::default()
    };
    exact
        .validate()
        .expect("an exactly representable quota horizon must validate");

    let overflow = QuotaConfig {
        per_remote_window_secs: u64::MAX,
        cooldown_secs: 1,
        ..QuotaConfig::default()
    };
    let error = overflow
        .validate()
        .expect_err("an overflowing quota horizon must fail validation");
    assert!(
        matches!(error, ConfigError::Quota(ref message) if message.contains("quotas.per_remote_window_secs") && message.contains("overflow")),
        "unexpected error: {error:?}"
    );

    let mut per_mode = PowConfig {
        quotas_per_mode: Some(HopQuotaOverrides {
            entry: Some(QuotaConfig {
                per_remote_window_secs: u64::MAX,
                cooldown_secs: 1,
                ..QuotaConfig::default()
            }),
            ..HopQuotaOverrides::default()
        }),
        ..PowConfig::default()
    };
    let error = per_mode
        .apply_defaults()
        .expect_err("an overflowing per-mode quota horizon must fail validation");
    assert!(
        matches!(error, ConfigError::Quota(ref message) if message.contains("quotas_per_mode.entry.per_remote_window_secs") && message.contains("overflow")),
        "unexpected error: {error:?}"
    );
}
#[test]
fn puzzle_config_rejects_invalid_values() {
    let mut pow = PowConfig {
        puzzle: PuzzleConfig {
            memory_kib: 1024,
            time_cost: 0,
            lanes: 0,
        },
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
fn puzzle_parameters_reject_inverted_ticket_timing_without_panic() {
    let pow = PowConfig {
        max_future_skew_secs: 10,
        min_ticket_ttl_secs: 30,
        ..PowConfig::default()
    };
    match pow.puzzle_parameters() {
        Err(ConfigError::Puzzle(message)) => assert!(
            message.contains("invalid pow.puzzle timing parameters"),
            "unexpected puzzle timing error: {message}"
        ),
        other => panic!("expected puzzle timing error, got {other:?}"),
    }
    let mut pow = pow;
    match pow.apply_defaults() {
        Err(ConfigError::Puzzle(message)) => assert!(
            message.contains("invalid pow.puzzle timing parameters"),
            "unexpected puzzle defaults error: {message}"
        ),
        other => panic!("expected puzzle defaults error, got {other:?}"),
    }
}
#[test]
fn puzzle_config_builds_parameters() {
    let mut pow = PowConfig {
        difficulty: 12,
        max_future_skew_secs: 45,
        min_ticket_ttl_secs: 15,
        puzzle: PuzzleConfig {
            memory_kib: 32 * 1024,
            time_cost: 3,
            lanes: 2,
        },
        ..PowConfig::default()
    };
    pow.apply_defaults().expect("defaults");
    let params = pow.puzzle_parameters().expect("parameters");
    assert_eq!(params.memory_kib().get(), 32 * 1024);
    assert_eq!(params.time_cost().get(), 3);
    assert_eq!(params.lanes().get(), 2);
    assert_eq!(params.difficulty(), 12);
}
#[test]
fn relay_config_rejects_retired_descriptor_replay_filter() {
    let json = config_fixture!("replay_filter.json");
    let path = write_config(json);
    let error = RelayConfig::load(path).expect_err("unsafe static-descriptor filter must fail");
    assert!(
        matches!(error, ConfigError::Json(_)),
        "unexpected error: {error:?}"
    );
    let message = error.to_string();
    assert!(message.contains("unknown field") && message.contains("replay_filter"));
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

    let link_directory = tempfile::tempdir_in(std::env::current_dir().expect("current directory"))
        .expect("create salt symlink directory");
    let link_path = link_directory.path().join("salt-link");
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
                    "ed25519_private_key_hex": "{seed_hex}",
                    "mldsa65_private_key_hex": "{}"
                }}
            }}"#,
        fixture_mldsa65_private_key_hex(),
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
    let secrets = config
        .handshake_policy()
        .manifest_secrets()
        .expect("manifest parsing");
    let (seed, mldsa65) = secrets.into_private_keys();
    let expected_bytes = hex::decode(seed_hex).expect("valid hex");
    let mut expected = [0u8; ED25519_IDENTITY_SEED_LEN_V1];
    expected.copy_from_slice(&expected_bytes);
    assert_eq!(seed, expected);
    assert_eq!(mldsa65.len(), MlDsaSuite::MlDsa65.secret_key_len());
}
#[test]
fn identity_manifest_missing_key_errors() {
    let seed = "44".repeat(ED25519_IDENTITY_SEED_LEN_V1);
    let mldsa65 = fixture_mldsa65_private_key_hex();
    let cases = [
        (
            format!(r#"{{"version":1,"identity":{{"mldsa65_private_key_hex":"{mldsa65}"}}}}"#),
            "ed25519_private_key_hex",
        ),
        (
            format!(r#"{{"version":1,"identity":{{"ed25519_private_key_hex":"{seed}"}}}}"#),
            "mldsa65_private_key_hex",
        ),
    ];
    for (manifest, field) in cases {
        let message = descriptor_manifest_error_message(&manifest);
        assert!(
            message.contains(&format!("missing required field `{field}`")),
            "unexpected message for `{field}`: {message}"
        );
    }
}
#[test]
fn deploy_sample_config_validates() {
    let json = include_str!("../deploy/config/relay.entry.json");
    let mut cfg: RelayConfig = norito::json::from_str(json).expect("parse sample config");
    cfg.validate().expect("sample config validates");
    assert_eq!(cfg.mode, RelayMode::Entry);
    let certificate = cfg
        .handshake_policy()
        .certificate
        .as_ref()
        .expect("production sample pins a certificate bundle");
    assert_eq!(
        certificate.bundle_path,
        PathBuf::from("/etc/soranet/relay/secrets/relay-certificate.cbor")
    );
    assert_eq!(certificate.issuer_ed25519_hex.len(), 64);
    assert!(
        certificate
            .issuer_ed25519_hex
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    );
    assert_eq!(
        certificate.issuer_mldsa_hex.len(),
        MlDsaSuite::MlDsa65.public_key_len() * 2
    );
    assert!(
        certificate
            .issuer_mldsa_hex
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    );
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
        "/var/lib/soranet-relay/audit-spool"
    );
    assert_eq!(
        cfg.guard_directory_config()
            .expect("guard directory")
            .pinning_proof_path()
            .expect("guard pinning proof path"),
        Path::new("/var/lib/soranet-relay/guard-pinning-proofs/relay.json")
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
fn deployment_secret_samples_expose_only_the_exact_v1_identity_schema() {
    let manifest: norito::json::Value = norito::json::from_str(include_str!(
        "../deploy/config/relay-descriptor-manifest.sample.json"
    ))
    .expect("parse deployment descriptor manifest sample");
    let root = manifest.as_object().expect("manifest root object");
    assert_eq!(root.len(), 2);
    assert_eq!(root["version"].as_u64(), Some(1));
    let identity = root["identity"].as_object().expect("identity object");
    let mut fields = identity
        .keys()
        .map(|field| field.as_str())
        .collect::<Vec<_>>();
    fields.sort_unstable();
    assert_eq!(
        fields,
        ["ed25519_private_key_hex", "mldsa65_private_key_hex"]
    );

    let kubernetes = include_str!("../deploy/kubernetes/soranet-relay.yaml");
    for required in [
        "descriptor_manifest_path",
        "bundle_path",
        "issuer_ed25519_hex",
        "issuer_mldsa_hex",
        "ed25519_private_key_hex",
        "mldsa65_private_key_hex",
        "relay-certificate.cbor",
    ] {
        assert!(
            kubernetes.contains(required),
            "Kubernetes deployment sample is missing `{required}`"
        );
    }
    for retired in ["ml_kem_private_key_hex", "ml_kem_public_hex"] {
        assert!(
            !kubernetes.contains(retired),
            "Kubernetes deployment sample retains retired manifest field `{retired}`"
        );
    }
}
#[test]
fn deployment_samples_materialize_direct_config_and_persist_audit_state() {
    let kubernetes = include_str!("../deploy/kubernetes/soranet-relay.yaml");
    for required in [
        "cp /var/run/soranet-relay-config-source/relay.json /config/relay.json",
        "snapshot_source=/var/run/soranet-relay-snapshot-source/current_snapshot.norito",
        "cp \"$snapshot_source\" /private/current_snapshot.norito",
        "chmod 0400 /config/relay.json",
        "refusing symbolic-link persistence path",
        "chmod 0700 \"$path\"",
        "pinning_proof_path\": \"/var/lib/soranet-relay/guard-pinning-proofs/relay.json",
        "automountServiceAccountToken: false",
        "claimName: soranet-relay-guard-snapshot",
        "claimName: soranet-relay-state",
        "claimName: soranet-relay-audit-spool",
        "claimName: soranet-relay-compliance-logs",
    ] {
        assert!(
            kubernetes.contains(required),
            "Kubernetes deployment sample is missing `{required}`"
        );
    }
    let images = kubernetes
        .lines()
        .filter_map(|line| line.trim().strip_prefix("image:").map(str::trim))
        .collect::<Vec<_>>();
    assert!(!images.is_empty(), "deployment sample must declare images");
    for image in images {
        let (repository, digest) = image
            .rsplit_once("@sha256:")
            .unwrap_or_else(|| panic!("image `{image}` must use an immutable sha256 digest"));
        assert!(
            !repository.is_empty() && !repository.contains('@'),
            "image `{image}` has a malformed repository or multiple digest selectors"
        );
        assert_eq!(
            digest.len(),
            64,
            "image `{image}` must have exactly 64 digest characters"
        );
        assert!(
            digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
            "image `{image}` digest must be lowercase hexadecimal"
        );
    }

    let systemd = include_str!("../deploy/systemd/soranet-relay.service");
    for required in [
        "StateDirectoryMode=0700",
        "LogsDirectory=soranet",
        "LogsDirectoryMode=0700",
        "UMask=0077",
        "ExecStartPre=/usr/bin/test ! -L",
        "/var/lib/soranet-relay/audit-spool",
        "/var/lib/soranet-relay/guard-pinning-proofs",
    ] {
        assert!(
            systemd.contains(required),
            "systemd deployment sample is missing `{required}`"
        );
    }
    for forbidden in [
        "ExecReload=",
        "CAP_NET_BIND_SERVICE",
        "AmbientCapabilities=",
    ] {
        assert!(
            !systemd.contains(forbidden),
            "systemd deployment sample must not contain `{forbidden}`"
        );
    }
}
#[test]
fn kubernetes_deployment_preserves_private_custody_without_fs_group() {
    let kubernetes = include_str!("../deploy/kubernetes/soranet-relay.yaml");
    for forbidden in ["fsGroup:", "fsGroupChangePolicy:"] {
        assert!(
            !kubernetes.contains(forbidden),
            "pod-level `{forbidden}` can widen persistent private files"
        );
    }
    for required in [
        "for path in /persistent/audit /persistent/logs /persistent/state /persistent/state/guard-pinning-proofs; do",
        "chown 65532:65532 \"$path\"",
        "chmod 0700 \"$path\"",
        "chown 65532:65532 /private/*",
        "chmod 0400 /private/*",
        "chown 0:0 /config",
        "chmod 0755 /config",
        "chown 0:0 /private",
        "chmod 0755 /private",
    ] {
        assert!(
            kubernetes.contains(required),
            "Kubernetes custody contract is missing `{required}`"
        );
    }
    let init_container = kubernetes
        .split("      initContainers:\n")
        .nth(1)
        .and_then(|source| source.split("\n      containers:\n").next())
        .expect("init-container source contract");
    let exact_capability_contract = concat!(
        "            capabilities:\n",
        "              drop:\n",
        "                - ALL\n",
        "              add:\n",
        "                - CHOWN\n",
        "                - DAC_OVERRIDE\n",
        "                - FOWNER\n",
        "          volumeMounts:"
    );
    assert!(
        init_container.contains(exact_capability_contract),
        "root init container must drop all capabilities and add only CHOWN, DAC_OVERRIDE, and FOWNER"
    );
}
#[test]
fn kubernetes_persistent_claims_are_single_pod_writer_volumes() {
    let kubernetes = include_str!("../deploy/kubernetes/soranet-relay.yaml");
    let claims = kubernetes
        .split("\n---\n")
        .filter(|document| document.contains("\nkind: PersistentVolumeClaim\n"))
        .collect::<Vec<_>>();
    assert_eq!(claims.len(), 4, "deployment must declare four custody PVCs");
    for claim in claims {
        assert_eq!(
            claim.matches("    - ReadWriteOncePod").count(),
            1,
            "every custody PVC must enforce cluster-wide single-pod mounting"
        );
        assert!(
            !claim.contains("    - ReadWriteOnce\n"),
            "node-scoped ReadWriteOnce is insufficient for relay custody"
        );
    }
    let readme = include_str!("../deploy/README.md");
    for required in [
        "selected CSI driver and cluster must support it",
        "adds only `CHOWN`, `FOWNER`, and `DAC_OVERRIDE`",
        "storage access mode is not a substitute",
    ] {
        assert!(
            readme.contains(required),
            "single-writer/capability documentation is missing `{required}`"
        );
    }
}
#[test]
fn kubernetes_guard_snapshot_uses_a_dedicated_volume_instead_of_a_secret() {
    let kubernetes = include_str!("../deploy/kubernetes/soranet-relay.yaml");
    let secret = kubernetes
        .split("\n---\n")
        .find(|document| document.contains("\nkind: Secret\n"))
        .expect("deployment Secret document");
    for required in [
        "relay-descriptor-manifest.json",
        "server.crt",
        "server.key",
        "relay-certificate.cbor",
    ] {
        assert!(
            secret.contains(required),
            "private identity/certificate Secret is missing `{required}`"
        );
    }
    assert!(
        !secret.contains("current_snapshot.norito"),
        "a valid 5 MiB guard snapshot cannot be delivered through a Kubernetes Secret"
    );
    for required in [
        "name: soranet-relay-guard-snapshot",
        "mountPath: /var/run/soranet-relay-snapshot-source",
        "readOnly: true",
        "guard snapshot exceeds the 5 MiB first-release limit",
    ] {
        assert!(
            kubernetes.contains(required),
            "dedicated guard snapshot volume contract is missing `{required}`"
        );
    }
    let readme = include_str!("../deploy/README.md");
    for required in [
        "Scale `deployment/soranet-relay` to zero",
        "atomically rename it to",
        "`/current_snapshot.norito`",
        "Never overwrite the live inode in place",
    ] {
        assert!(
            readme.contains(required),
            "guard snapshot replacement documentation is missing `{required}`"
        );
    }
}
#[test]
fn runtime_shutdown_contract_handles_sigterm_and_keeps_quic_close() {
    let runtime = include_str!("runtime.rs");
    for required in [
        "async fn shutdown_signal()",
        "tokio::signal::ctrl_c()",
        "SignalKind::terminate()",
        "endpoint.close(0u32.into(), b\"shutdown\")",
    ] {
        assert!(
            runtime.contains(required),
            "runtime shutdown contract is missing `{required}`"
        );
    }
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
fn vpn_route_push_rejects_host_bits() {
    let mut cfg = VpnConfig {
        enabled: true,
        route_push: vec!["10.1.2.3/24".to_string()],
        ..VpnConfig::default()
    };
    let err = cfg
        .validate()
        .expect_err("route prefixes with host bits must fail");
    match err {
        ConfigError::Vpn(message) => {
            assert!(message.contains("host bits"), "unexpected error: {message}");
            assert!(
                message.contains("10.1.2.0/24"),
                "unexpected error: {message}"
            );
        }
        other => panic!("unexpected error {other:?}"),
    }
}
#[test]
fn vpn_route_push_rejects_semantic_duplicates() {
    let mut cfg = VpnConfig {
        enabled: true,
        route_push: vec!["2001:0db8::/64".to_string(), "2001:db8::/64".to_string()],
        ..VpnConfig::default()
    };
    let err = cfg
        .validate()
        .expect_err("semantically duplicate route prefixes must fail");
    assert!(
        matches!(&err, ConfigError::Vpn(message) if message.contains("duplicate")),
        "unexpected error: {err:?}"
    );
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
fn vpn_dns_override_rejects_non_unicast_addresses() {
    for dns in [
        "0.0.0.0",
        "224.0.0.1",
        "255.255.255.255",
        "::",
        "ff02::1",
        "::ffff:0.0.0.0",
        "::ffff:224.0.0.1",
        "::ffff:255.255.255.255",
    ] {
        let mut cfg = VpnConfig {
            enabled: true,
            dns_overrides: vec![dns.to_string()],
            ..VpnConfig::default()
        };
        let err = cfg
            .validate()
            .expect_err("non-unicast DNS overrides must fail");
        assert!(
            matches!(&err, ConfigError::Vpn(message) if message.contains("unicast")),
            "unexpected error for {dns}: {err:?}"
        );
    }
}
#[test]
fn vpn_dns_override_rejects_semantic_duplicates() {
    let mut cfg = VpnConfig {
        enabled: true,
        dns_overrides: vec!["2001:0db8::1".to_string(), "2001:db8::1".to_string()],
        ..VpnConfig::default()
    };
    let err = cfg
        .validate()
        .expect_err("semantically duplicate DNS overrides must fail");
    assert!(
        matches!(&err, ConfigError::Vpn(message) if message.contains("duplicate")),
        "unexpected error: {err:?}"
    );
}
#[test]
fn vpn_dns_override_rejects_mapped_ipv4_duplicates() {
    let mut cfg = VpnConfig {
        enabled: true,
        dns_overrides: vec!["1.1.1.1".to_string(), "::ffff:1.1.1.1".to_string()],
        ..VpnConfig::default()
    };
    let err = cfg
        .validate()
        .expect_err("mapped and native IPv4 DNS overrides must be semantic duplicates");
    assert!(
        matches!(&err, ConfigError::Vpn(message) if message.contains("duplicate")),
        "unexpected error: {err:?}"
    );
}
#[test]
fn vpn_cover_ratio_allows_zero_when_enabled() {
    let (mut cfg, _issuer_key) = vpn_config_with_credentials(0xAB);
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
fn vpn_helper_ticket_issuer_public_key_loads_private_file() {
    let (mut cfg, _issuer_key_file) = vpn_config_with_credentials(0xAB);
    cfg.validate().expect("vpn config should validate");
    let expected = KeyPair::try_from_seed(vec![0xAB; 32], Algorithm::Ed25519)
        .expect("derive expected issuer keypair")
        .public_key()
        .clone();
    assert_eq!(
        cfg.try_helper_ticket_issuer_public_key()
            .expect("read helper-ticket issuer public key"),
        Some(expected)
    );
}
#[test]
fn vpn_helper_ticket_replay_store_defaults_are_mandatory() {
    let (mut cfg, _issuer_key) = vpn_config_with_credentials(0xAB);
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
    let (mut cfg, _issuer_key) = vpn_config_with_credentials(0xAB);
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
fn vpn_helper_ticket_issuer_public_key_rejects_short_file() {
    let file = NamedTempFile::new_in(std::env::current_dir().expect("current directory"))
        .expect("create short VPN secret");
    std::fs::write(file.path(), "ab".repeat(16)).expect("write short VPN secret");
    let cfg = VpnConfig {
        helper_ticket_issuer_public_key_path: Some(file.path().to_path_buf()),
        ..VpnConfig::default()
    };
    let err = cfg
        .try_helper_ticket_issuer_public_key()
        .expect_err("short helper-ticket issuer public key must fail");
    assert!(
        matches!(err, ConfigError::Vpn(message) if message.contains("64 lowercase hexadecimal"))
    );
}
#[test]
fn vpn_helper_ticket_issuer_public_key_requires_canonical_nonzero_encoding() {
    for (contents, expected) in [
        ("AB".repeat(32), "lowercase"),
        (format!("{}\n", "ab".repeat(32)), "no newline"),
        ("00".repeat(32), "all-zero"),
    ] {
        let file = NamedTempFile::new_in(std::env::current_dir().expect("current directory"))
            .expect("create noncanonical VPN secret");
        std::fs::write(file.path(), contents).expect("write noncanonical VPN secret");
        let config = VpnConfig {
            helper_ticket_issuer_public_key_path: Some(file.path().to_path_buf()),
            ..VpnConfig::default()
        };
        let error = config
            .try_helper_ticket_issuer_public_key()
            .expect_err("noncanonical VPN issuer public key must fail closed");
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
    let (mut cfg, credentials) = vpn_config_with_credentials(0xAB);
    let socket = credentials._receipt_spool.path().join("normalized.sock");
    cfg.backend_endpoint = Some(format!(" unix:{} ", socket.display()));
    cfg.validate().expect("vpn config should validate");
    let canonical_socket = std::fs::canonicalize(socket.parent().expect("socket parent"))
        .expect("canonical socket parent")
        .join("normalized.sock");
    assert_eq!(
        cfg.backend_endpoint,
        Some(format!("unix:{}", canonical_socket.display()))
    );
    assert_eq!(
        cfg.backend_endpoint()
            .map(|endpoint| endpoint.path().to_path_buf()),
        Some(canonical_socket)
    );
}
#[cfg(unix)]
#[test]
fn vpn_backend_endpoint_rejects_unsafely_writable_parent() {
    use std::os::unix::fs::PermissionsExt as _;

    let (mut cfg, _credentials) = vpn_config_with_credentials(0xAB);
    let parent = tempfile::tempdir_in(std::env::current_dir().expect("current directory"))
        .expect("create backend socket parent");
    std::fs::set_permissions(parent.path(), std::fs::Permissions::from_mode(0o777))
        .expect("make backend socket parent unsafe");
    cfg.backend_endpoint = Some(format!(
        "unix:{}",
        parent.path().join("backend.sock").display()
    ));
    let error = cfg
        .validate()
        .expect_err("writable backend socket parent must fail closed");
    assert!(
        matches!(error, ConfigError::Vpn(message) if message.contains("not be unsafely writable"))
    );
}
#[test]
fn vpn_backend_endpoint_requires_secret_for_unix_socket() {
    let (mut cfg, _issuer_key) = vpn_config_with_credentials(0xAB);
    cfg.backend_bootstrap_secret_path = None;
    let unix_error = cfg
        .validate()
        .expect_err("Unix endpoint must require bootstrap authentication");
    assert!(
        matches!(unix_error, ConfigError::Vpn(message) if message.contains("backend_bootstrap_secret_path"))
    );
    let bootstrap_secret = write_vpn_secret(0xCD);
    cfg.backend_bootstrap_secret_path = Some(bootstrap_secret.path().to_path_buf());
    cfg.validate().expect("Unix endpoint with secret");
    assert!(
        cfg.backend_endpoint()
            .is_some_and(|endpoint| endpoint.path().is_absolute())
    );
    assert_eq!(
        cfg.try_backend_bootstrap_secret_bytes()
            .expect("read bootstrap secret"),
        Some([0xCD; 32])
    );
}
#[test]
fn vpn_backend_tcp_endpoints_are_rejected() {
    let (mut cfg, _issuer_key) = vpn_config_with_credentials(0xAB);
    let bootstrap_secret = write_vpn_secret(0xCD);
    cfg.backend_bootstrap_secret_path = Some(bootstrap_secret.path().to_path_buf());
    for endpoint in ["tcp://127.0.0.1:19090", "tcp://192.0.2.1:19090"] {
        cfg.backend_endpoint = Some(endpoint.to_owned());
        let err = cfg
            .validate()
            .expect_err("every TCP backend must fail closed");
        assert!(
            matches!(&err, ConfigError::Vpn(message) if message.contains("TCP")),
            "unexpected backend error for {endpoint}: {err:?}"
        );
    }
}
#[test]
fn vpn_backend_peer_uid_and_gid_are_mandatory() {
    let (mut cfg, _credentials) = vpn_config_with_credentials(0xAB);
    cfg.backend_expected_uid = None;
    let uid_error = cfg
        .validate()
        .expect_err("VPN backend UID must be explicitly pinned");
    assert!(
        matches!(uid_error, ConfigError::Vpn(message) if message.contains("backend_expected_uid"))
    );

    cfg.backend_expected_uid = Some(0);
    cfg.backend_expected_gid = None;
    let gid_error = cfg
        .validate()
        .expect_err("VPN backend GID must be explicitly pinned");
    assert!(
        matches!(gid_error, ConfigError::Vpn(message) if message.contains("backend_expected_gid"))
    );
}
#[test]
fn vpn_fallible_accessors_decode_valid_private_files() {
    let (mut cfg, credentials) = vpn_config_with_credentials(0xAB);
    let bootstrap_secret = write_vpn_secret(0xCD);
    cfg.backend_endpoint = Some(format!(
        " unix:{} ",
        credentials
            ._receipt_spool
            .path()
            .join("accessor.sock")
            .display()
    ));
    cfg.backend_bootstrap_secret_path = Some(bootstrap_secret.path().to_path_buf());
    cfg.billing = VpnBillingConfig {
        meter_hash_hex: "ef".repeat(32),
        ..VpnBillingConfig::default()
    };
    cfg.validate().expect("vpn config validates");
    assert_eq!(cfg.try_meter_hash_bytes().expect("meter hash"), [0xEF; 32]);
    let expected_issuer = KeyPair::try_from_seed(vec![0xAB; 32], Algorithm::Ed25519)
        .expect("derive expected issuer keypair")
        .public_key()
        .clone();
    assert_eq!(
        cfg.try_helper_ticket_issuer_public_key()
            .expect("helper-ticket issuer public key"),
        Some(expected_issuer)
    );
    assert!(
        cfg.try_backend_endpoint()
            .expect("backend endpoint")
            .is_some_and(|endpoint| endpoint.path().is_absolute())
    );
    assert_eq!(cfg.backend_expected_peer_ids(), Some((0, 0)));
    assert_eq!(
        cfg.try_backend_bootstrap_secret_bytes()
            .expect("bootstrap secret"),
        Some([0xCD; 32])
    );
    let overlay = VpnOverlay::try_from_config(cfg).expect("vpn overlay");
    assert_eq!(overlay.meter_hash(), [0xEF; 32]);
}
#[test]
fn vpn_overlay_try_from_config_rejects_invalid_issuer_key_without_panic() {
    let file = NamedTempFile::new_in(std::env::current_dir().expect("current directory"))
        .expect("create invalid issuer key");
    std::fs::write(file.path(), "not-hex").expect("write invalid issuer key");
    let (mut cfg, _credentials) = vpn_config_with_credentials(0xA9);
    cfg.helper_ticket_issuer_public_key_path = Some(file.path().to_path_buf());
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
    let (mut cfg, _issuer_key) = vpn_config_with_credentials(0xAB);
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
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt as _;
    let (mut cfg, _issuer_key) = vpn_config_with_credentials(0xAB);
    let spool = tempfile::tempdir_in(std::env::current_dir().expect("current directory"))
        .expect("create receipt spool directory");
    #[cfg(unix)]
    std::fs::set_permissions(spool.path(), std::fs::Permissions::from_mode(0o700))
        .expect("protect receipt spool directory");
    cfg.receipt_spool_dir = Some(spool.path().to_path_buf());
    cfg.validate().expect("vpn config should validate");
    let canonical_spool = std::fs::canonicalize(spool.path()).expect("canonical spool path");
    assert_eq!(
        cfg.receipt_spool_dir.as_deref(),
        Some(canonical_spool.as_path())
    );
}
#[test]
fn enabled_vpn_requires_durable_receipt_spool() {
    let (mut cfg, _credentials) = vpn_config_with_credentials(0xAE);
    cfg.receipt_spool_dir = None;
    let error = cfg
        .validate()
        .expect_err("enabled VPN must not discard settlement artifacts");
    assert!(
        matches!(error, ConfigError::Vpn(message) if message.contains("receipt_spool_dir must be set"))
    );
}
#[cfg(unix)]
#[test]
fn vpn_receipt_spool_dir_rejects_relative_or_permissive_paths() {
    use std::os::unix::fs::PermissionsExt as _;

    let (mut cfg, _issuer_key) = vpn_config_with_credentials(0xAC);
    cfg.receipt_spool_dir = Some(PathBuf::from("relative-vpn-receipts"));
    let relative_error = cfg.validate().expect_err("relative spool must fail");
    assert!(
        matches!(relative_error, ConfigError::Vpn(message) if message.contains("must be absolute"))
    );

    let spool = tempfile::tempdir_in(std::env::current_dir().expect("current directory"))
        .expect("create receipt spool directory");
    std::fs::set_permissions(spool.path(), std::fs::Permissions::from_mode(0o755))
        .expect("make receipt spool permissive");
    let (mut cfg, _issuer_key) = vpn_config_with_credentials(0xAD);
    cfg.receipt_spool_dir = Some(spool.path().to_path_buf());
    let mode_error = cfg.validate().expect_err("permissive spool must fail");
    assert!(matches!(mode_error, ConfigError::Vpn(message) if message.contains("mode 0700")));
}
#[test]
fn vpn_control_plane_threads_routes_and_dns() {
    let (mut cfg, _issuer_key) = vpn_config_with_credentials(0xAB);
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
