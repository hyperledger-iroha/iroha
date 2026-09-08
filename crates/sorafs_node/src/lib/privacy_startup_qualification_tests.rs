// Privacy runtime qualification failures must precede all checkpoint persistence.
#[test]
fn privacy_cycle_prf_startup_requires_runtime_provider() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let root = temp_dir.path().canonicalize().expect("canonical temp dir");
    let cfg = privacy_aggregate_storage_config(&root);
    let error = NodeHandle::try_new_with_runtime_deps(
        cfg,
        with_fresh_test_fenced_privacy_runtime(NodeRuntimeDeps::default()),
    )
    .expect_err("configured threshold-PRF provider is required");
    assert!(
        matches!(
            error,
            NodeInitError::PrivacyCyclePrfProviderQualification {
                error: TransparencyRuntimeProviderQualificationErrorV1::MissingProvider,
            }
        ),
        "unexpected startup error: {error:?}"
    );
    assert!(!root.join("storage").exists());
}
#[test]
fn privacy_cycle_prf_qualification_fails_before_persistence() {
    let cases: [(
        Arc<dyn ProductionPrivacyCyclePrfProviderV1>,
        TransparencyRuntimeProviderQualificationErrorV1,
    ); 5] = [
        (
            Arc::new(TestPrivacyCyclePrfProvider::with_qualification(
                "threshold-prf:transparency:secondary",
                1,
                [0xC7; 32],
                false,
            )),
            TransparencyRuntimeProviderQualificationErrorV1::SubstitutedProvider,
        ),
        (
            Arc::new(TestPrivacyCyclePrfProvider::with_qualification(
                TEST_PRIVACY_CYCLE_PRF_PROVIDER_HANDLE,
                2,
                [0xC7; 32],
                false,
            )),
            TransparencyRuntimeProviderQualificationErrorV1::ConfiguredQualificationMismatch,
        ),
        (
            Arc::new(TestPrivacyCyclePrfProvider::with_qualification(
                TEST_PRIVACY_CYCLE_PRF_PROVIDER_HANDLE,
                1,
                [0xC8; 32],
                false,
            )),
            TransparencyRuntimeProviderQualificationErrorV1::ConfiguredQualificationMismatch,
        ),
        (
            Arc::new(TestPrivacyCyclePrfProvider::with_qualification(
                "threshold-prf:test:primary",
                1,
                [0xC7; 32],
                false,
            )),
            TransparencyRuntimeProviderQualificationErrorV1::TestMarkedProviderHandle,
        ),
        (
            Arc::new(TestPrivacyCyclePrfProvider::with_qualification(
                TEST_PRIVACY_CYCLE_PRF_PROVIDER_HANDLE,
                1,
                [0xC7; 32],
                true,
            )),
            TransparencyRuntimeProviderQualificationErrorV1::UnavailableOrStale,
        ),
    ];
    for (provider, expected) in cases {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let root = temp_dir.path().canonicalize().expect("canonical temp dir");
        let cfg = privacy_aggregate_storage_config(&root);
        let data_dir = cfg.data_dir().clone();
        assert!(!data_dir.exists());
        let error = NodeHandle::try_new_with_runtime_deps(
            cfg,
            with_fresh_test_fenced_privacy_runtime(privacy_runtime_deps(
                provider,
                test_privacy_release_anchor(),
            )),
        )
        .expect_err("invalid production threshold-PRF qualification must fail startup");
        assert!(
            matches!(
                &error,
                NodeInitError::PrivacyCyclePrfProviderQualification { error }
                    if *error == expected
            ),
            "unexpected startup error: {error:?}"
        );
        assert!(!error.to_string().contains("must-never-escape"));
        assert!(!format!("{error:?}").contains("must-never-escape"));
        assert!(
            !data_dir.exists(),
            "provider qualification must complete before persistence opens"
        );
    }
}
#[test]
fn differential_privacy_startup_requires_finalized_release_anchor() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let root = temp_dir.path().canonicalize().expect("canonical temp dir");
    let cfg = privacy_aggregate_storage_config(&root);
    let error = NodeHandle::try_new_with_runtime_deps(
        cfg,
        with_fresh_test_fenced_privacy_runtime(
            NodeRuntimeDeps::default()
                .with_privacy_cycle_prf_provider(test_privacy_cycle_prf_provider()),
        ),
    )
    .expect_err("configured finalized release anchor is required");
    assert!(
        matches!(
            error,
            NodeInitError::PrivacyReleaseAnchorQualification {
                error: TransparencyRuntimeProviderQualificationErrorV1::MissingProvider,
            }
        ),
        "unexpected startup error: {error:?}"
    );
    assert!(!root.join("storage").exists());
}
#[test]
fn differential_privacy_startup_requires_transparency_leader_lease_provider() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let root = temp_dir.path().canonicalize().expect("canonical temp dir");
    let cfg = privacy_aggregate_storage_config(&root);
    let error = NodeHandle::try_new_with_runtime_deps(
        cfg,
        with_fresh_test_fenced_privacy_runtime(
            NodeRuntimeDeps::default()
                .with_privacy_cycle_prf_provider(test_privacy_cycle_prf_provider())
                .with_privacy_release_anchor(test_privacy_release_anchor()),
        ),
    )
    .expect_err("configured transparency leader lease provider is required");
    assert!(
        matches!(
            error,
            NodeInitError::TransparencyLeaderLeaseProviderQualification {
                error: TransparencyLeaderLeaseErrorV1::ProviderQualification(
                    TransparencyRuntimeProviderQualificationErrorV1::MissingProvider
                ),
            }
        ),
        "unexpected startup error: {error:?}"
    );
    assert!(!root.join("storage").exists());
}
