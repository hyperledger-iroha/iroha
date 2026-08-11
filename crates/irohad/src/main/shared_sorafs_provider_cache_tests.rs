#[cfg(test)]
mod shared_sorafs_provider_cache_tests {
    use std::{
        fs,
        num::NonZeroUsize,
        path::{Path, PathBuf},
    };

    use iroha_config::{
        base::read::ConfigReader,
        parameters::{actual::SorafsAdmission, user::Root as UserConfig},
    };
    use iroha_config_base::toml::TomlSource;
    use iroha_crypto::{Algorithm, PrivateKey, PublicKey, Signature};
    use iroha_torii::sorafs::{ReplayCheckpointError, discovery::AdvertError};
    use sorafs_manifest::{ProviderAdmissionCouncilPolicyError, ProviderAdvertV1};
    use tempfile::TempDir;

    use super::*;

    fn base_config() -> Config {
        let table = toml::toml! {
            chain = "00000000-0000-0000-0000-000000000000"
            public_key = "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2"
            private_key = "8926201CA347641228C3B79AA43839DEDC85FA51C0E8B9B6A00F6B0D6B0423E902973F"

            [network]
            address = "addr:127.0.0.1:1337#8F78"
            public_address = "addr:127.0.0.1:1337#8F78"

            [torii]
            address = "addr:127.0.0.1:8080#8942"

            [genesis]
            public_key = "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
            expected_hash = "hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"

            [streaming]
            identity_public_key = "ed01208BA62848CF767D72E7F7F4B9D2D7BA07FEE33760F79ABE5597A51520E292A0CB"
            identity_private_key = "8026208F4C15E5D664DA3F13778801D23D4E89B76E94C1B94B389544168B6CB894F84F"
        };

        ConfigReader::new()
            .with_toml_source(TomlSource::inline(table))
            .read_and_complete::<UserConfig>()
            .expect("shared provider-cache test config must be readable")
            .parse()
            .expect("shared provider-cache test config must parse")
    }

    fn ed25519_public_key(seed: u8) -> PublicKey {
        let private = PrivateKey::from_bytes(Algorithm::Ed25519, &[seed; 32])
            .expect("fixture Ed25519 seed must be valid");
        PublicKey::from(private)
    }

    fn configure_discovery(config: &mut Config, temp: &TempDir) -> PathBuf {
        let root = temp
            .path()
            .canonicalize()
            .expect("canonical temporary provider-cache root");
        let admission_dir = root.join("admission");
        fs::create_dir_all(&admission_dir).expect("create fixture admission directory");
        config.torii.data_dir = root.join("torii-data");
        config.torii.sorafs_discovery.discovery_enabled = true;
        config.torii.sorafs_discovery.known_capabilities =
            vec!["torii_gateway".to_owned(), "chunk_range_fetch".to_owned()];
        config.torii.sorafs_discovery.replay_checkpoint_path =
            PathBuf::from("discovery/provider-advert-replay.to");
        config.torii.sorafs_discovery.replay_checkpoint_max_entries =
            NonZeroUsize::new(8).expect("non-zero bound");
        config.torii.sorafs_discovery.admission = Some(SorafsAdmission {
            envelopes_dir: admission_dir.clone(),
            trusted_council_keys: vec![ed25519_public_key(0x45)],
            signature_threshold: NonZeroUsize::new(1).expect("non-zero threshold"),
        });
        admission_dir
    }

    fn fixture_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/sorafs_manifest/provider_admission")
            .join(name)
    }

    fn install_admission_fixture(admission_dir: &Path) {
        fs::copy(
            fixture_path("envelope_v1.to"),
            admission_dir.join("envelope_v1.to"),
        )
        .expect("copy canonical provider admission fixture");
    }

    fn load_advert_fixture() -> ProviderAdvertV1 {
        let bytes =
            fs::read(fixture_path("advert_v1.to")).expect("read canonical provider advert fixture");
        norito::decode_from_bytes(&bytes).expect("decode canonical provider advert fixture")
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
        let mut config = base_config();
        config.torii.data_dir = temp.path().join("must-not-exist");
        config.torii.sorafs_discovery.discovery_enabled = false;
        config.torii.sorafs_discovery.known_capabilities = vec!["unknown".to_owned()];
        config.torii.sorafs_discovery.admission = None;

        let cache = build_shared_sorafs_provider_cache(&config)
            .expect("disabled discovery must not validate unused configuration");

        assert!(cache.is_none());
        assert!(!config.torii.data_dir.exists());
    }

    #[test]
    fn enabled_discovery_requires_admission_without_panicking() {
        let mut config = base_config();
        config.torii.sorafs_discovery.discovery_enabled = true;
        config.torii.sorafs_discovery.admission = None;

        let error = build_shared_sorafs_provider_cache(&config)
            .expect_err("enabled discovery without admission must fail closed");

        assert!(matches!(
            error,
            SharedSoraFsProviderCacheError::AdmissionPolicyRequired
        ));
    }

    #[test]
    fn malformed_capability_lists_are_typed_startup_errors() {
        let temp = tempfile::tempdir().expect("temporary provider-cache root");
        let mut config = base_config();
        configure_discovery(&mut config, &temp);
        config.torii.sorafs_discovery.known_capabilities = vec!["not-a-capability".to_owned()];

        let error = build_shared_sorafs_provider_cache(&config)
            .expect_err("unknown capability must fail closed");
        assert!(matches!(
            error,
            SharedSoraFsProviderCacheError::UnknownCapability(name)
                if name == "not-a-capability"
        ));

        config.torii.sorafs_discovery.known_capabilities =
            vec!["torii".to_owned(), "torii_gateway".to_owned()];
        let error = build_shared_sorafs_provider_cache(&config)
            .expect_err("duplicate capability aliases must fail closed");
        assert!(matches!(
            error,
            SharedSoraFsProviderCacheError::DuplicateCapability(name)
                if name == "torii_gateway"
        ));

        config.torii.sorafs_discovery.known_capabilities.clear();
        let error = build_shared_sorafs_provider_cache(&config)
            .expect_err("empty capability list must fail closed");
        assert!(matches!(
            error,
            SharedSoraFsProviderCacheError::EmptyCapabilities
        ));
    }

    #[test]
    fn malformed_admission_policies_are_typed_startup_errors() {
        let temp = tempfile::tempdir().expect("temporary provider-cache root");
        let mut config = base_config();
        configure_discovery(&mut config, &temp);
        let duplicate = ed25519_public_key(0x45);
        config
            .torii
            .sorafs_discovery
            .admission
            .as_mut()
            .expect("admission policy")
            .trusted_council_keys = vec![duplicate.clone(), duplicate];

        let error = build_shared_sorafs_provider_cache(&config)
            .expect_err("duplicate council key must fail closed");
        assert!(matches!(
            error,
            SharedSoraFsProviderCacheError::InvalidCouncilPolicy(
                ProviderAdmissionCouncilPolicyError::DuplicateSigner { .. }
            )
        ));

        let secp_private = PrivateKey::from_bytes(Algorithm::Secp256k1, &[0x31; 32])
            .expect("fixture secp256k1 seed must be valid");
        config
            .torii
            .sorafs_discovery
            .admission
            .as_mut()
            .expect("admission policy")
            .trusted_council_keys = vec![PublicKey::from(secp_private)];
        let error = build_shared_sorafs_provider_cache(&config)
            .expect_err("non-Ed25519 council key must fail closed");
        assert!(matches!(
            error,
            SharedSoraFsProviderCacheError::UnsupportedCouncilKeyAlgorithm {
                algorithm: Algorithm::Secp256k1,
                ..
            }
        ));
    }

    #[test]
    fn malformed_replay_checkpoint_is_a_typed_startup_error() {
        let temp = tempfile::tempdir().expect("temporary provider-cache root");
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

        let error = build_shared_sorafs_provider_cache(&config)
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
        let mut config = base_config();
        configure_discovery(&mut config, &temp);
        config.torii.sorafs_discovery.replay_checkpoint_max_entries =
            NonZeroUsize::new(usize::MAX).expect("maximum usize is non-zero");

        let error = build_shared_sorafs_provider_cache(&config)
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
        let mut config = base_config();
        let admission_dir = configure_discovery(&mut config, &temp);
        install_admission_fixture(&admission_dir);
        let checkpoint = config
            .torii
            .data_dir
            .join(&config.torii.sorafs_discovery.replay_checkpoint_path);

        let original = load_advert_fixture();
        let mut latest = original.clone();
        latest.issued_at = latest.issued_at.saturating_add(1);
        resign_advert(&mut latest);

        let cache = build_shared_sorafs_provider_cache(&config)
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

        let restarted = build_shared_sorafs_provider_cache(&config)
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
