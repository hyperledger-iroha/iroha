//! Production registry revocation, prepare/commit fencing and durable replay-floor tests.
use super::*;
use ed25519_dalek::{Signer, SigningKey};
use iroha_core::smartcontracts::isi::sorafs_provider_admission::test_fixture::ProviderAdmissionTestFixtureV1;
use sorafs_manifest::AdvertSignature;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn live_finalized_revocation_fences_prepared_adverts_and_survives_registry_restart() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let mut native = ProviderAdmissionTestFixtureV1::new_at(now.saturating_sub(10));
    native.admit();
    let registry = Arc::new(AdmissionRegistry::from_state(Arc::clone(native.state())));
    let provider = *native.provider().as_bytes();
    assert!(registry.entry(&provider).is_some());
    assert!(registry.council_policy().is_some());
    let mut advert: ProviderAdvertV1 = norito::decode_from_bytes(include_bytes!(
        "../../../../../fixtures/sorafs_manifest/provider_admission/advert_v1.to"
    ))
    .unwrap();
    let key = SigningKey::from_bytes(&[0x21; 32]);
    advert.network_id = *registry.network_id();
    advert.body = native.envelope().advert_body.clone();
    advert.issued_at = now - 1;
    advert.expires_at = now + 60;
    advert.signature = AdvertSignature {
        algorithm: SignatureAlgorithm::Ed25519,
        public_key: key.verifying_key().as_bytes().to_vec(),
        signature: vec![0; 64],
    };
    advert.signature.signature = key
        .sign(&advert.signature_payload_bytes().unwrap())
        .to_bytes()
        .to_vec();
    let capabilities = advert
        .body
        .capabilities
        .iter()
        .map(|capability| capability.cap_type)
        .collect::<Vec<_>>();
    let root = std::env::temp_dir().canonicalize().unwrap();
    let temp = tempfile::tempdir_in(root).unwrap();
    let checkpoint = temp.path().join("replay.to");
    let mut cache = ProviderAdvertCache::new_persistent(
        capabilities.clone(),
        Arc::clone(&registry),
        checkpoint.clone(),
        NonZeroUsize::new(4096).unwrap(),
    )
    .unwrap();
    let prepared = cache
        .validation_policy()
        .prepare(advert.clone(), now)
        .unwrap();
    cache.commit_prepared(prepared, now).unwrap();
    let in_flight = cache
        .validation_policy()
        .prepare(advert.clone(), now)
        .unwrap();
    native.revoke();
    assert!(registry.entry(&provider).is_none());
    assert!(matches!(
        cache.commit_prepared(in_flight, now),
        Err(AdvertError::AdmissionMissing { .. })
    ));
    assert_eq!(cache.prune_stale(now), 1);
    assert!(cache.replay_high_water.contains_key(&provider));
    drop(cache);
    drop(registry);
    let restarted = Arc::new(AdmissionRegistry::from_state(Arc::clone(native.state())));
    let cache = ProviderAdvertCache::new_persistent(
        capabilities,
        restarted.clone(),
        checkpoint,
        NonZeroUsize::new(4096).unwrap(),
    )
    .unwrap();
    assert!(restarted.entry(&provider).is_none());
    assert!(restarted.retains_identity(&provider));
    assert!(cache.replay_high_water.contains_key(&provider));
    assert!(cache.validation_policy().prepare(advert, now).is_err());
}

#[test]
fn production_registry_cannot_reload_local_authority() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let mut native = ProviderAdmissionTestFixtureV1::new_at(now.saturating_sub(10));
    native.admit();
    let mut registry = AdmissionRegistry::from_state(Arc::clone(native.state()));
    let policy = registry.council_policy().unwrap();
    let directory = tempfile::tempdir().unwrap();
    assert!(matches!(
        registry.reload_from_dir(directory.path(), (*policy).clone()),
        Err(super::super::admission::AdmissionRegistryError::FinalizedAuthority)
    ));
    assert!(registry.entry(native.provider().as_bytes()).is_some());
}
