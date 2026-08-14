use super::*;
#[test]
fn offline_parse_defaults_to_universal_capability_without_operator_catalog() {
    let mut emitter = Emitter::new();
    let actual = Offline::default().parse(&mut emitter);
    assert!(emitter.into_result().is_ok());
    assert!(actual.escrow_accounts.is_empty());
    assert!(actual.kagemusha_release_policy_path.is_none());
    assert!(actual.kagemusha_artifact_dir.is_none());
    assert!(actual.kagemusha_catalog_qualification_seal_path.is_none());
    assert_eq!(
        actual.kagemusha_max_decoded_bytes,
        defaults::settlement::offline::KAGEMUSHA_MAX_DECODED_BYTES
    );
}
#[test]
fn offline_parse_requires_release_paths_as_a_pair() {
    for offline in [
        Offline {
            kagemusha_release_policy_path: Some("policy.norito".into()),
            ..Offline::default()
        },
        Offline {
            kagemusha_artifact_dir: Some("artifacts".into()),
            ..Offline::default()
        },
    ] {
        let mut emitter = Emitter::new();
        let _ = offline.parse(&mut emitter);
        assert!(emitter.into_result().is_err());
    }
}
#[test]
fn offline_parse_preserves_paired_release_paths() {
    let policy = PathBuf::from("policy.norito");
    let artifacts = PathBuf::from("artifacts");
    let seal = PathBuf::from("/var/lib/iroha/kagemusha/catalog-seal.norito");
    let mut emitter = Emitter::new();
    let actual = Offline {
        kagemusha_release_policy_path: Some(policy.clone()),
        kagemusha_artifact_dir: Some(artifacts.clone()),
        kagemusha_catalog_qualification_seal_path: Some(seal.clone()),
        ..Offline::default()
    }
    .parse(&mut emitter);
    assert!(emitter.into_result().is_ok());
    assert_eq!(actual.kagemusha_release_policy_path, Some(policy));
    assert_eq!(actual.kagemusha_artifact_dir, Some(artifacts));
    assert_eq!(actual.kagemusha_catalog_qualification_seal_path, Some(seal));
}
#[test]
fn offline_parse_rejects_qualification_seal_without_release_paths() {
    let mut emitter = Emitter::new();
    let _ = Offline {
        kagemusha_catalog_qualification_seal_path: Some(
            "/var/lib/iroha/kagemusha/catalog-seal.norito".into(),
        ),
        ..Offline::default()
    }
    .parse(&mut emitter);
    assert!(emitter.into_result().is_err());
}
#[test]
fn offline_parse_rejects_relative_qualification_seal_path() {
    let mut emitter = Emitter::new();
    let _ = Offline {
        kagemusha_release_policy_path: Some("policy.norito".into()),
        kagemusha_artifact_dir: Some("artifacts".into()),
        kagemusha_catalog_qualification_seal_path: Some("catalog-seal.norito".into()),
        ..Offline::default()
    }
    .parse(&mut emitter);
    assert!(emitter.into_result().is_err());
}
#[test]
fn offline_parse_rejects_empty_release_paths() {
    let mut emitter = Emitter::new();
    let _ = Offline {
        kagemusha_release_policy_path: Some(PathBuf::new()),
        kagemusha_artifact_dir: Some(PathBuf::from("artifacts")),
        ..Offline::default()
    }
    .parse(&mut emitter);
    assert!(emitter.into_result().is_err());
}
#[test]
fn offline_parse_rejects_empty_qualification_seal_path() {
    let mut emitter = Emitter::new();
    let _ = Offline {
        kagemusha_release_policy_path: Some(PathBuf::from("policy.norito")),
        kagemusha_artifact_dir: Some(PathBuf::from("artifacts")),
        kagemusha_catalog_qualification_seal_path: Some(PathBuf::new()),
        ..Offline::default()
    }
    .parse(&mut emitter);
    assert!(emitter.into_result().is_err());
}
#[test]
fn offline_parse_rejects_zero_kagemusha_decoded_budget() {
    let mut emitter = Emitter::new();
    let actual = Offline {
        kagemusha_max_decoded_bytes: 0,
        ..Offline::default()
    }
    .parse(&mut emitter);
    assert!(emitter.into_result().is_err());
    assert_eq!(
        actual.kagemusha_max_decoded_bytes,
        defaults::settlement::offline::KAGEMUSHA_MAX_DECODED_BYTES
    );
}
#[test]
fn offline_parse_rejects_kagemusha_budget_above_safety_ceiling() {
    let mut emitter = Emitter::new();
    let actual = Offline {
        kagemusha_max_decoded_bytes: defaults::settlement::offline::KAGEMUSHA_MAX_DECODED_BYTES + 1,
        ..Offline::default()
    }
    .parse(&mut emitter);
    assert!(emitter.into_result().is_err());
    assert_eq!(
        actual.kagemusha_max_decoded_bytes,
        defaults::settlement::offline::KAGEMUSHA_MAX_DECODED_BYTES
    );
}
#[test]
fn offline_parse_allows_lower_kagemusha_budget() {
    let lower = defaults::settlement::offline::KAGEMUSHA_MAX_DECODED_BYTES / 2;
    let mut emitter = Emitter::new();
    let actual = Offline {
        kagemusha_max_decoded_bytes: lower,
        ..Offline::default()
    }
    .parse(&mut emitter);
    assert!(emitter.into_result().is_ok());
    assert_eq!(actual.kagemusha_max_decoded_bytes, lower);
}
