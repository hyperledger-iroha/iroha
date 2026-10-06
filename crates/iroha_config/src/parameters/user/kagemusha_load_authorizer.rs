//! File-only optional publisher settings. Secret contents are never embedded in diagnostics.
use super::*;

/// Optional online finalized-load publisher. No field accepts environment overrides.
#[derive(Debug, ReadConfig)]
pub struct KagemushaLoadAuthorizer {
    #[config(default)]
    enabled: bool,
    keyring_file: Option<WithOrigin<PathBuf>>,
    submitter_key_file: Option<WithOrigin<PathBuf>>,
    #[config(default = "defaults::kagemusha_load_authorizer::POLL_INTERVAL_MS")]
    poll_interval_ms: u64,
    #[config(default = "defaults::kagemusha_load_authorizer::PAGE_SIZE")]
    page_size: usize,
    #[config(default = "defaults::kagemusha_load_authorizer::BLOCK_BYTES")]
    block_bytes: usize,
    #[config(default = "defaults::kagemusha_load_authorizer::JOURNAL_BYTES")]
    journal_bytes: usize,
    #[config(default = "defaults::kagemusha_load_authorizer::BLOCK_COUNT")]
    block_count: usize,
    #[config(default = "defaults::kagemusha_load_authorizer::ALLOCATED_BYTES")]
    allocated_bytes: usize,
    #[config(default = "defaults::kagemusha_load_authorizer::TRANSACTION_TTL_MS")]
    transaction_ttl_ms: u64,
    #[config(default)]
    charge_limits: Vec<iroha_data_model::transaction::FeeChargeLimit>,
}
impl KagemushaLoadAuthorizer {
    pub(super) fn parse(
        self,
        files: &ConfigFiles<'_>,
        emitter: &mut Emitter<ParseError>,
    ) -> Option<actual::KagemushaLoadAuthorizer> {
        let mut emit = |message: &'static str| {
            emitter
                .emit(Report::new(ParseError::InvalidKagemushaLoadAuthorizerConfig).attach(message))
        };
        let limits = iroha_data_model::sumeragi::finality::NativeFinalityLimits {
            block_bytes: self.block_bytes,
            journal_bytes: self.journal_bytes,
            block_count: self.block_count,
            allocated_bytes: self.allocated_bytes,
        };
        if !(1..=60_000).contains(&self.poll_interval_ms)
            || !(1..=256).contains(&self.page_size)
            || !(1_000..=3_600_000).contains(&self.transaction_ttl_ms)
            || limits.validate().is_err()
            || self.charge_limits.len() > 16
            || iroha_data_model::transaction::FeePaymentIntent::authority(
                self.charge_limits.clone(),
                None,
            )
            .validate()
            .is_err()
        {
            emit(
                "kagemusha_load_authorizer requires finite valid finality limits, page_size 1..256, poll_interval_ms 1..60000, and transaction_ttl_ms 1000..3600000",
            );
            return None;
        }
        let custody = match (self.enabled, self.keyring_file, self.submitter_key_file) {
            (false, None, None) => None,
            (true, Some(keyring), Some(submitter)) => {
                let keyring = read_checked(
                    files,
                    &keyring.resolve_relative_path(),
                    ConfigFileRequest {
                        access: ConfigFileAccess::Private,
                        maximum: defaults::kagemusha_load_authorizer::KEYRING_MAX_BYTES,
                    },
                );
                let submitter = read_private_key_file(
                    submitter,
                    "kagemusha_load_authorizer.submitter_key_file",
                    files,
                );
                match (keyring, submitter) {
                    (Ok(keyring), Ok((secret, _))) if !keyring.is_empty() => {
                        match KeyPair::from_private_key(secret) {
                            Ok(submitter) => {
                                Some(actual::KagemushaLoadAuthorizerCustody { keyring, submitter })
                            }
                            Err(_) => {
                                emit("kagemusha_load_authorizer submitter key is invalid");
                                return None;
                            }
                        }
                    }
                    _ => {
                        emit(
                            "kagemusha_load_authorizer private custody files are absent, unavailable, unsafe, empty or oversized",
                        );
                        return None;
                    }
                }
            }
            _ => {
                emit(
                    "kagemusha_load_authorizer must be disabled without key files or enabled with both keyring_file and submitter_key_file",
                );
                return None;
            }
        };
        Some(actual::KagemushaLoadAuthorizer {
            custody,
            poll_interval: Duration::from_millis(self.poll_interval_ms),
            page_size: self.page_size,
            finality_limits: limits,
            transaction_ttl: Duration::from_millis(self.transaction_ttl_ms),
            charge_limits: self.charge_limits,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    struct Files {
        keyring: Vec<u8>,
        submitter: Vec<u8>,
        error: Option<io::ErrorKind>,
        calls: Cell<usize>,
    }
    impl ConfigFileSource for Files {
        fn read(
            &self,
            path: &Path,
            request: ConfigFileRequest,
        ) -> io::Result<zeroize::Zeroizing<Vec<u8>>> {
            self.calls.set(self.calls.get() + 1);
            assert_eq!(request.access, ConfigFileAccess::Private);
            if let Some(error) = self.error {
                return Err(error.into());
            }
            let bytes = if path == Path::new("keyring") {
                &self.keyring
            } else {
                assert_eq!(path, Path::new("submitter"));
                &self.submitter
            };
            Ok(zeroize::Zeroizing::new(bytes.clone()))
        }
    }
    fn config() -> KagemushaLoadAuthorizer {
        let defaults = actual::KagemushaLoadAuthorizer::default();
        KagemushaLoadAuthorizer {
            enabled: false,
            keyring_file: None,
            submitter_key_file: None,
            poll_interval_ms: defaults.poll_interval.as_millis().try_into().unwrap(),
            page_size: defaults.page_size,
            block_bytes: defaults.finality_limits.block_bytes,
            journal_bytes: defaults.finality_limits.journal_bytes,
            block_count: defaults.finality_limits.block_count,
            allocated_bytes: defaults.finality_limits.allocated_bytes,
            transaction_ttl_ms: defaults.transaction_ttl.as_millis().try_into().unwrap(),
            charge_limits: Vec::new(),
        }
    }
    fn files() -> Files {
        let fixture: toml::Table = include_str!("../../../tests/fixtures/base.toml")
            .parse()
            .unwrap();
        Files {
            keyring: vec![1, 2, 3],
            submitter: fixture["private_key"].as_str().unwrap().as_bytes().to_vec(),
            error: None,
            calls: Cell::new(0),
        }
    }
    fn enabled() -> KagemushaLoadAuthorizer {
        let mut config = config();
        config.enabled = true;
        config.keyring_file = Some(WithOrigin::inline(PathBuf::from("keyring")));
        config.submitter_key_file = Some(WithOrigin::inline(PathBuf::from("submitter")));
        config
    }
    #[test]
    fn disabled_default_reads_no_secret_and_enabled_custody_is_redacted() {
        let files = files();
        let mut emitter = Emitter::new();
        let default = config()
            .parse(&ConfigFiles::Supplied(&files), &mut emitter)
            .unwrap();
        emitter.into_result().unwrap();
        assert!(default.custody.is_none());
        assert_eq!(files.calls.get(), 0);
        let mut emitter = Emitter::new();
        let loaded = enabled()
            .parse(&ConfigFiles::Supplied(&files), &mut emitter)
            .unwrap();
        emitter.into_result().unwrap();
        assert_eq!(files.calls.get(), 2);
        assert_eq!(
            loaded.custody.as_ref().unwrap().keyring.as_slice(),
            &[1, 2, 3]
        );
        let debug = format!("{loaded:?}");
        assert!(!debug.contains("1, 2, 3"));
        assert!(!debug.contains(std::str::from_utf8(&files.submitter).unwrap()));
    }
    #[test]
    fn missing_unsafe_unavailable_oversized_or_partial_custody_is_rejected() {
        for error in [
            io::ErrorKind::NotFound,
            io::ErrorKind::PermissionDenied,
            io::ErrorKind::Other,
        ] {
            let mut files = files();
            files.error = Some(error);
            let mut emitter = Emitter::new();
            assert!(
                enabled()
                    .parse(&ConfigFiles::Supplied(&files), &mut emitter)
                    .is_none()
            );
            assert!(emitter.into_result().is_err());
        }
        for size in [
            0,
            defaults::kagemusha_load_authorizer::KEYRING_MAX_BYTES + 1,
        ] {
            let mut files = files();
            files.keyring = vec![0; size];
            let mut emitter = Emitter::new();
            assert!(
                enabled()
                    .parse(&ConfigFiles::Supplied(&files), &mut emitter)
                    .is_none()
            );
            assert!(emitter.into_result().is_err());
        }
        for missing_submitter in [false, true] {
            let mut config = enabled();
            if missing_submitter {
                config.submitter_key_file = None;
            } else {
                config.keyring_file = None;
            }
            let files = files();
            let mut emitter = Emitter::new();
            assert!(
                config
                    .parse(&ConfigFiles::Supplied(&files), &mut emitter)
                    .is_none()
            );
            assert!(emitter.into_result().is_err());
        }
        let mut disabled_with_keys = enabled();
        disabled_with_keys.enabled = false;
        let files = files();
        let mut emitter = Emitter::new();
        assert!(
            disabled_with_keys
                .parse(&ConfigFiles::Supplied(&files), &mut emitter)
                .is_none()
        );
        assert!(emitter.into_result().is_err());
    }
    #[test]
    fn zero_or_excessive_runtime_limits_are_rejected() {
        for index in 0..6 {
            let mut config = config();
            match index {
                0 => config.poll_interval_ms = 0,
                1 => config.page_size = 257,
                2 => config.block_bytes = 0,
                3 => config.journal_bytes = 1,
                4 => config.allocated_bytes = 0,
                _ => config.transaction_ttl_ms = 3_600_001,
            }
            let files = files();
            let mut emitter = Emitter::new();
            assert!(
                config
                    .parse(&ConfigFiles::Supplied(&files), &mut emitter)
                    .is_none()
            );
            assert!(emitter.into_result().is_err());
            assert_eq!(files.calls.get(), 0);
        }
    }
    #[test]
    fn toml_defaults_and_explicit_fee_caps_are_canonical() {
        use iroha_config_base::{read::ConfigReader, toml::TomlSource};
        let parsed = ConfigReader::new()
            .with_toml_source(TomlSource::inline(toml::Table::new()))
            .read_and_complete::<KagemushaLoadAuthorizer>()
            .unwrap();
        assert!(!parsed.enabled);
        assert!(parsed.charge_limits.is_empty());
        assert_eq!(
            parsed.page_size,
            defaults::kagemusha_load_authorizer::PAGE_SIZE
        );
        let asset = iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            iroha_model_base::domain::DomainId::parse_fully_qualified("issuer.sora").unwrap(),
            "fees".parse().unwrap(),
        );
        let limit = iroha_data_model::transaction::FeeChargeLimit {
            kind: iroha_data_model::transaction::FeeChargeKind::Nexus,
            asset_definition_id: asset,
            max_amount: iroha_primitives::numeric::Quantity::from_canonical_numeric(
                iroha_primitives::numeric::Numeric::new(10, 0),
            )
            .unwrap(),
        };
        for limits in [vec![limit.clone()], vec![limit.clone(), limit]] {
            let valid = limits.len() == 1;
            let mut config = config();
            config.charge_limits = limits.clone();
            let files = files();
            let mut emitter = Emitter::new();
            let result = config.parse(&ConfigFiles::Supplied(&files), &mut emitter);
            assert_eq!(result.is_some(), valid);
            assert_eq!(emitter.into_result().is_ok(), valid);
            if let Some(result) = result {
                assert_eq!(result.charge_limits, limits);
            }
        }
    }
}
