//! KAGEMUSHA V1 command configuration tests.

use super::*;
use iroha_crypto::ExposedPrivateKey;
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(unix)]
use std::{io::Write as _, os::unix::fs::OpenOptionsExt as _};
static NEXT_KEY_FILE: AtomicU64 = AtomicU64::new(0);
struct TestKeyFile(PathBuf);
impl TestKeyFile {
    fn unique_path() -> PathBuf {
        let sequence = NEXT_KEY_FILE.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "iroha-config-kagemusha-v1-{}-{sequence}.key",
            std::process::id()
        ))
    }
    fn create(contents: &str) -> Self {
        let path = Self::unique_path();
        #[cfg(unix)]
        {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&path)
                .expect("create owner-only KAGEMUSHA V1 key file");
            file.write_all(contents.as_bytes())
                .expect("write KAGEMUSHA V1 key file");
        }
        #[cfg(not(unix))]
        fs::write(&path, contents).expect("write KAGEMUSHA V1 key file");
        Self(path)
    }
    fn missing() -> Self {
        let path = Self::unique_path();
        let _ = fs::remove_file(&path);
        Self(path)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for TestKeyFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
fn sample() -> ToriiKagemushaV1Commands {
    let key_pair = KeyPair::from_seed(vec![0x41; 32], Algorithm::Ed25519);
    ToriiKagemushaV1Commands {
        redemption_private_key: Some(key_pair.private_key().clone()),
        redemption_private_key_file: None,
        redemption_minimum_xor_balance: Some(Quantity::from(25_u32)),
        max_tx_value: defaults::torii::kagemusha_v1_commands::max_tx_value(),
        operation_registry_max_entries:
            defaults::torii::kagemusha_v1_commands::OPERATION_REGISTRY_MAX_ENTRIES,
        operation_registry_max_bytes:
            defaults::torii::kagemusha_v1_commands::OPERATION_REGISTRY_MAX_BYTES,
    }
}
fn parse_valid(config: ToriiKagemushaV1Commands) -> actual::ToriiKagemushaV1Commands {
    let mut emitter = Emitter::new();
    let parsed = config
        .parse(&mut emitter)
        .expect("valid KAGEMUSHA V1 command configuration");
    emitter
        .into_result()
        .expect("valid KAGEMUSHA V1 command configuration must not emit errors");
    parsed
}
fn rejection_report(config: ToriiKagemushaV1Commands) -> String {
    let mut emitter = Emitter::new();
    assert!(config.parse(&mut emitter).is_none());
    let error = emitter
        .into_result()
        .expect_err("invalid KAGEMUSHA V1 command configuration must emit an error");
    format!("{error:?}")
}
fn assert_rejected(config: ToriiKagemushaV1Commands) {
    let _ = rejection_report(config);
}
#[test]
fn parses_kagemusha_v1_redemption_authority() {
    let parsed = parse_valid(sample());
    assert_eq!(
        defaults::torii::kagemusha_v1_commands::OPERATION_REGISTRY_ACCOUNTED_BYTES_PER_ENTRY,
        145
    );
    assert_eq!(
        defaults::torii::kagemusha_v1_commands::OPERATION_REGISTRY_MAX_BYTES,
        593_920
    );
    assert_eq!(
        parsed.operation_registry_max_entries.get(),
        defaults::torii::kagemusha_v1_commands::OPERATION_REGISTRY_MAX_ENTRIES
    );
    assert_eq!(parsed.operation_registry_max_bytes.get(), 593_920);
    let issuer = parsed
        .redemption_issuer
        .expect("configured redemption issuer");
    assert_eq!(issuer.minimum_xor_balance, Quantity::from(25_u32));
    assert!(!parsed.max_tx_value.is_zero());
}

#[test]
fn payer_signed_top_up_admission_does_not_require_a_redemption_key() {
    let mut config = sample();
    config.redemption_private_key = None;
    config.redemption_minimum_xor_balance = None;
    let parsed = parse_valid(config);
    assert!(parsed.redemption_issuer.is_none());
    assert!(!parsed.max_tx_value.is_zero());
}
#[test]
fn parses_owner_held_kagemusha_v1_redemption_key() {
    let key_pair = KeyPair::from_seed(vec![0x42; 32], Algorithm::Ed25519);
    let key_file = TestKeyFile::create(&format!(
        "{}\n",
        ExposedPrivateKey(key_pair.private_key().clone())
    ));
    let mut config = sample();
    config.redemption_private_key = None;
    config.redemption_private_key_file = Some(WithOrigin::inline(key_file.path().to_path_buf()));
    let parsed = parse_valid(config);
    let issuer = parsed
        .redemption_issuer
        .expect("configured redemption issuer");
    assert_eq!(
        issuer.authority,
        AccountId::new(key_pair.public_key().clone())
    );
}
#[test]
fn rejects_duplicate_kagemusha_v1_redemption_private_key_sources() {
    let key_file = TestKeyFile::create("");
    let mut config = sample();
    config.redemption_private_key_file = Some(WithOrigin::inline(key_file.path().to_path_buf()));
    let report = rejection_report(config);
    assert!(
        report.contains("torii.kagemusha_v1_commands.redemption_private_key")
            && report.contains("torii.kagemusha_v1_commands.redemption_private_key_file"),
        "unexpected diagnostic: {report}"
    );
}
#[test]
fn rejects_minimum_balance_without_a_redemption_key() {
    let mut config = sample();
    config.redemption_private_key = None;
    let report = rejection_report(config);
    assert!(
        report.contains("torii.kagemusha_v1_commands.redemption_minimum_xor_balance"),
        "unexpected diagnostic: {report}"
    );
}

#[test]
fn rejects_redemption_key_without_a_minimum_balance() {
    let mut config = sample();
    config.redemption_minimum_xor_balance = None;
    let report = rejection_report(config);
    assert!(
        report.contains("torii.kagemusha_v1_commands.redemption_minimum_xor_balance"),
        "unexpected diagnostic: {report}"
    );
}
#[test]
fn rejects_missing_or_malformed_kagemusha_v1_redemption_private_key_file() {
    for key_file in [
        TestKeyFile::missing(),
        TestKeyFile::create("not-a-private-key\n"),
    ] {
        let mut config = sample();
        config.redemption_private_key = None;
        config.redemption_private_key_file =
            Some(WithOrigin::inline(key_file.path().to_path_buf()));
        assert_rejected(config);
    }
}
#[test]
fn rejects_unsupported_kagemusha_v1_redemption_private_key_algorithm() {
    let key_pair = KeyPair::from_seed(vec![0x43; 32], Algorithm::BlsNormal);
    let mut config = sample();
    config.redemption_private_key = Some(key_pair.private_key().clone());
    assert_rejected(config);
}
#[test]
fn rejects_zero_redemption_minimum_xor_balance() {
    let mut config = sample();
    config.redemption_minimum_xor_balance = Some(Quantity::zero());
    let report = rejection_report(config);
    assert!(
        report.contains("torii.kagemusha_v1_commands.redemption_minimum_xor_balance"),
        "unexpected diagnostic: {report}"
    );
}
#[test]
fn rejects_zero_operation_registry_limits() {
    let mut config = sample();
    config.operation_registry_max_entries = 0;
    assert_rejected(config);
}
#[test]
fn rejects_zero_operation_registry_byte_limit() {
    let mut config = sample();
    config.operation_registry_max_bytes = 0;
    assert_rejected(config);
}
#[test]
fn rejects_operation_registry_byte_limit_below_one_entry() {
    let minimum =
        defaults::torii::kagemusha_v1_commands::OPERATION_REGISTRY_ACCOUNTED_BYTES_PER_ENTRY;
    assert!(
        minimum > 1,
        "accounted entry size must have a positive predecessor"
    );
    let mut config = sample();
    config.operation_registry_max_bytes = minimum - 1;
    assert_rejected(config);
}
#[test]
fn rejects_zero_maximum_transaction_value() {
    let mut config = sample();
    config.max_tx_value = Quantity::zero();
    assert_rejected(config);
}
