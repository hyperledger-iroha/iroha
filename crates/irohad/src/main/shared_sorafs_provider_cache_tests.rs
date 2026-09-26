#[cfg(test)]
mod shared_sorafs_provider_cache_tests {
    use super::*;
    use iroha_config::parameters::actual::SorafsAdmission;
    use iroha_config_base::toml::TomlSource;
    use iroha_core::smartcontracts::isi::sorafs_provider_admission::test_fixture::ProviderAdmissionTestFixtureV1;
    use iroha_crypto::{Algorithm, PrivateKey, PublicKey, Signature};
    use iroha_torii::sorafs::{ReplayCheckpointError, discovery::AdvertError};
    use sorafs_manifest::ProviderAdvertV1;
    use std::{
        fs,
        num::NonZeroUsize,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };
    use tempfile::TempDir;
    fn base_config() -> Config {
        Config::from_toml_source(TomlSource::inline(
            crate::config_tests::minimal_config_table(),
        ))
        .expect("shared provider-cache test config must parse")
    }
    fn native_fixture() -> ProviderAdmissionTestFixtureV1 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let mut fixture = ProviderAdmissionTestFixtureV1::new_at(now - 10);
        fixture.admit();
        fixture
    }
    fn configure_discovery(config: &mut Config, temp: &TempDir) {
        let root = temp
            .path()
            .canonicalize()
            .expect("canonical temporary provider-cache root");
        config.torii.data_dir = root.join("torii-data");
        config.torii.sorafs_discovery.discovery_enabled = true;
        config.torii.sorafs_discovery.known_capabilities =
            vec!["torii_gateway".to_owned(), "chunk_range_fetch".to_owned()];
        config.torii.sorafs_discovery.replay_checkpoint_path =
            PathBuf::from("discovery/provider-advert-replay.to");
        config.torii.sorafs_discovery.replay_checkpoint_max_entries =
            NonZeroUsize::new(8).expect("non-zero bound");
        config.torii.sorafs_discovery.admission = Some(SorafsAdmission);
    }
    fn fixture_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/sorafs_manifest/provider_admission")
            .join(name)
    }
    fn load_advert_fixture(native: &ProviderAdmissionTestFixtureV1) -> ProviderAdvertV1 {
        let bytes =
            fs::read(fixture_path("advert_v1.to")).expect("read canonical provider advert fixture");
        let mut advert: ProviderAdvertV1 =
            norito::decode_from_bytes(&bytes).expect("decode canonical provider advert fixture");
        advert.network_id = native.envelope().network_id;
        advert.body = native.envelope().advert_body.clone();
        advert.issued_at = native.envelope().issued_at + 2;
        advert.expires_at = advert.issued_at + 60;
        resign_advert(&mut advert);
        advert
    }
    fn resign_advert(advert: &mut ProviderAdvertV1) {
        let private = PrivateKey::from_bytes(Algorithm::Ed25519, &[0x21; 32])
            .expect("fixture provider Ed25519 seed must be valid");
        let public = PublicKey::from(private.clone());
        let (_, public_payload) = public
            .try_to_bytes()
            .expect("fixture provider public key must be well formed");
        advert.signature.public_key = public_payload.to_vec();
        advert.signature.signature = vec![0; 64];
        let payload = advert
            .signature_payload_bytes()
            .expect("encode advert signature payload");
        advert.signature.signature = Signature::try_new(&private, &payload)
            .expect("sign provider advert fixture")
            .payload()
            .to_vec();
    }
    #[test]
    fn disabled_discovery_is_side_effect_free_even_with_poisonous_config() {
        let temp = tempfile::tempdir().expect("temporary provider-cache root");
        let native = native_fixture();
        let mut config = base_config();
        config.torii.data_dir = temp.path().join("must-not-exist");
        config.torii.sorafs_discovery.discovery_enabled = false;
        config.torii.sorafs_discovery.known_capabilities = vec!["unknown".to_owned()];
        config.torii.sorafs_discovery.admission = None;
        let cache = build_shared_sorafs_provider_cache(&config, Arc::clone(native.state()))
            .expect("disabled discovery must not validate unused configuration");
        assert!(cache.is_none());
        assert!(!config.torii.data_dir.exists());
    }
    #[test]
    fn enabled_discovery_uses_native_authority_without_a_local_admission_marker() {
        let temp = tempfile::tempdir().expect("temporary provider-cache root");
        let native = native_fixture();
        let mut config = base_config();
        configure_discovery(&mut config, &temp);
        config.torii.sorafs_discovery.admission = None;
        let cache = build_shared_sorafs_provider_cache(&config, Arc::clone(native.state()))
            .expect("native authority needs no local trust policy")
            .unwrap();
        let advert = load_advert_fixture(&native);
        assert!(
            cache
                .try_read()
                .unwrap()
                .validation_policy()
                .prepare(advert.clone(), advert.issued_at + 1)
                .is_ok()
        );
    }
    #[test]
    fn shared_cache_rejects_foreign_network_advert() {
        let temp = tempfile::tempdir().unwrap();
        let native = native_fixture();
        let mut config = base_config();
        configure_discovery(&mut config, &temp);
        let cache = build_shared_sorafs_provider_cache(&config, Arc::clone(native.state()))
            .unwrap()
            .unwrap();
        let mut advert = load_advert_fixture(&native);
        advert.network_id = [0xb3; 32];
        resign_advert(&mut advert);
        assert!(
            cache
                .try_read()
                .unwrap()
                .validation_policy()
                .prepare(advert.clone(), advert.issued_at + 1)
                .is_err()
        );
    }
    #[test]
    fn malformed_capability_lists_are_typed_startup_errors() {
        let temp = tempfile::tempdir().expect("temporary provider-cache root");
        let native = native_fixture();
        let mut config = base_config();
        configure_discovery(&mut config, &temp);
        config.torii.sorafs_discovery.known_capabilities = vec!["not-a-capability".to_owned()];
        let error = build_shared_sorafs_provider_cache(&config, Arc::clone(native.state()))
            .expect_err("unknown capability must fail closed");
        assert!(matches!(
            error,
            SharedSoraFsProviderCacheError::UnknownCapability(name)
                if name == "not-a-capability"
        ));
        config.torii.sorafs_discovery.known_capabilities =
            vec!["torii".to_owned(), "torii_gateway".to_owned()];
        let error = build_shared_sorafs_provider_cache(&config, Arc::clone(native.state()))
            .expect_err("retired capability aliases must fail closed");
        assert!(matches!(
            error,
            SharedSoraFsProviderCacheError::UnknownCapability(name) if name == "torii"
        ));
        config.torii.sorafs_discovery.known_capabilities =
            vec!["torii_gateway".to_owned(), "torii_gateway".to_owned()];
        let error = build_shared_sorafs_provider_cache(&config, Arc::clone(native.state()))
            .expect_err("duplicate canonical capabilities must fail closed");
        assert!(matches!(
            error,
            SharedSoraFsProviderCacheError::DuplicateCapability(name)
                if name == "torii_gateway"
        ));
        config.torii.sorafs_discovery.known_capabilities.clear();
        let error = build_shared_sorafs_provider_cache(&config, Arc::clone(native.state()))
            .expect_err("empty capability list must fail closed");
        assert!(matches!(
            error,
            SharedSoraFsProviderCacheError::EmptyCapabilities
        ));
    }
    #[test]
    fn malformed_replay_checkpoint_is_a_typed_startup_error() {
        let temp = tempfile::tempdir().expect("temporary provider-cache root");
        let native = native_fixture();
        let mut config = base_config();
        configure_discovery(&mut config, &temp);
        let checkpoint = config
            .torii
            .data_dir
            .join(&config.torii.sorafs_discovery.replay_checkpoint_path);
        fs::create_dir_all(checkpoint.parent().expect("checkpoint parent"))
            .expect("create checkpoint parent");
        fs::write(&checkpoint, b"not canonical Norito").expect("write corrupt checkpoint");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&checkpoint, fs::Permissions::from_mode(0o600))
                .expect("set private checkpoint permissions");
        }
        let error = build_shared_sorafs_provider_cache(&config, Arc::clone(native.state()))
            .expect_err("corrupt checkpoint must fail startup");
        assert!(matches!(
            error,
            SharedSoraFsProviderCacheError::ReplayCheckpoint {
                path,
                source: ReplayCheckpointError::Codec(_),
            } if path == checkpoint
        ));
    }
    #[test]
    fn configured_replay_bound_is_enforced_by_shared_cache_startup() {
        let temp = tempfile::tempdir().expect("temporary provider-cache root");
        let native = native_fixture();
        let mut config = base_config();
        configure_discovery(&mut config, &temp);
        config.torii.sorafs_discovery.replay_checkpoint_max_entries =
            NonZeroUsize::new(usize::MAX).expect("maximum usize is non-zero");
        let error = build_shared_sorafs_provider_cache(&config, Arc::clone(native.state()))
            .expect_err("unsafe replay checkpoint bound must fail startup");
        assert!(matches!(
            error,
            SharedSoraFsProviderCacheError::ReplayCheckpoint {
                source: ReplayCheckpointError::ConfiguredLimitTooLarge {
                    configured: usize::MAX,
                    ..
                },
                ..
            }
        ));
    }
    #[test]
    fn shared_cache_persists_replay_rejection_across_irohad_restart() {
        let temp = tempfile::tempdir().expect("temporary provider-cache root");
        let native = native_fixture();
        let mut config = base_config();
        configure_discovery(&mut config, &temp);
        let checkpoint = config
            .torii
            .data_dir
            .join(&config.torii.sorafs_discovery.replay_checkpoint_path);
        let original = load_advert_fixture(&native);
        let mut latest = original.clone();
        latest.issued_at = latest.issued_at.saturating_add(1);
        resign_advert(&mut latest);
        let cache = build_shared_sorafs_provider_cache(&config, Arc::clone(native.state()))
            .expect("initialize persistent shared cache")
            .expect("enabled discovery cache");
        {
            let mut cache = cache.try_write().expect("exclusive cache guard");
            let original_now = original.issued_at.saturating_add(1);
            let prepared = cache
                .validation_policy()
                .prepare(original.clone(), original_now)
                .expect("prepare original provider advert");
            cache
                .commit_prepared(prepared, original_now)
                .expect("persist original provider advert");
            let latest_now = latest.issued_at.saturating_add(1);
            let prepared = cache
                .validation_policy()
                .prepare(latest.clone(), latest_now)
                .expect("prepare latest provider advert");
            cache
                .commit_prepared(prepared, latest_now)
                .expect("persist latest provider advert high-water mark");
        }
        drop(cache);
        assert!(
            checkpoint.exists(),
            "relative replay path must resolve beneath Torii data_dir"
        );
        let restarted = build_shared_sorafs_provider_cache(&config, Arc::clone(native.state()))
            .expect("restart with canonical replay checkpoint")
            .expect("enabled discovery cache after restart");
        let mut restarted = restarted.try_write().expect("exclusive restarted guard");
        let stale_now = latest.issued_at.saturating_add(1);
        let prepared = restarted
            .validation_policy()
            .prepare(original, stale_now)
            .expect("stale advert remains otherwise authentic");
        let stale_error = restarted
            .commit_prepared(prepared, stale_now)
            .expect_err("restart must preserve stale-advert rejection");
        assert!(matches!(
            stale_error,
            AdvertError::NonMonotonicIssuedAt {
                current_issued_at,
                incoming_issued_at,
                ..
            } if current_issued_at == latest.issued_at
                && incoming_issued_at < current_issued_at
        ));
        let mut conflicting = latest.clone();
        conflicting.allow_unknown_capabilities = !conflicting.allow_unknown_capabilities;
        resign_advert(&mut conflicting);
        let conflict_now = latest.issued_at.saturating_add(1);
        let prepared = restarted
            .validation_policy()
            .prepare(conflicting, conflict_now)
            .expect("conflicting advert remains otherwise authentic");
        let conflict_error = restarted
            .commit_prepared(prepared, conflict_now)
            .expect_err("restart must preserve conflicting same-timestamp rejection");
        assert!(matches!(
            conflict_error,
            AdvertError::NonMonotonicIssuedAt {
                current_issued_at,
                incoming_issued_at,
                ..
            } if current_issued_at == latest.issued_at
                && incoming_issued_at == current_issued_at
        ));
    }
}
