// SoraFS discovery advert-key mismatch regression.

#[test]
fn disk_fixtures_detect_advert_key_mismatch() {
    let fixtures = [("advert_v1.to", "envelope_v1.to")];

    for (advert_path, envelope_path) in fixtures {
        let fixture = fixture_from_disk(advert_path, envelope_path);
        let registry = admission_registry_from_fixtures(std::slice::from_ref(&fixture));
        let mut cache = ProviderAdvertCache::new(
            [
                CapabilityType::ToriiGateway,
                CapabilityType::ChunkRangeFetch,
            ],
            registry,
        );

        let now = fixture
            .advert
            .issued_at
            .saturating_add(30)
            .min(fixture.advert.expires_at.saturating_sub(1))
            .max(fixture.advert.issued_at);
        let err = cache
            .ingest(fixture.advert.clone(), now)
            .expect_err("fixture ingestion must fail due to advert key mismatch");
        match err {
            AdvertError::AdmissionFailed { error, .. } => {
                assert!(
                    matches!(error, AdmissionCheckError::AdvertKeyMismatch),
                    "expected advert key mismatch, got {error:?}"
                );
            }
            other => panic!("expected admission failure, got {other:?}"),
        }
    }
}
