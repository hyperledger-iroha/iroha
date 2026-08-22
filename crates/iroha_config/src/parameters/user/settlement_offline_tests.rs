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
    assert!(actual.kagemusha_promotion_controller_public_key.is_none());
    assert!(
        actual
            .kagemusha_catalog_revalidation_authority_key_id
            .is_none()
    );
    assert!(
        actual
            .kagemusha_catalog_revalidation_authority_public_key
            .is_none()
    );
    assert!(actual.kagemusha_promotion_reservation_path.is_none());
    assert!(actual.kagemusha_validator_qualification_seal_path.is_none());
    assert_eq!(
        actual.kagemusha_max_decoded_bytes,
        defaults::settlement::offline::KAGEMUSHA_MAX_DECODED_BYTES
    );
}
fn promotion_controller(algorithm: Algorithm) -> PublicKey {
    KeyPair::from_seed(vec![0x51; 32], algorithm)
        .public_key()
        .clone()
}
fn catalog_revalidation_authority(algorithm: Algorithm) -> PublicKey {
    KeyPair::from_seed(vec![0x52; 32], algorithm)
        .public_key()
        .clone()
}
const CATALOG_REVALIDATION_AUTHORITY_KEY_ID: &str = "sora.catalog-authority_v1";

#[test]
fn offline_parse_preserves_complete_validator_qualification_configuration() {
    let controller = promotion_controller(Algorithm::Ed25519);
    let reservation = PathBuf::from("/Library/SORA/Kagemusha/reservation-v1.norito");
    let validator_seal = PathBuf::from("/Library/SORA/Kagemusha/validator-v1.norito");
    let mut emitter = Emitter::new();
    let actual = Offline {
        kagemusha_release_policy_path: Some("policy.norito".into()),
        kagemusha_artifact_dir: Some("artifacts".into()),
        kagemusha_catalog_qualification_seal_path: Some(
            "/Library/SORA/Kagemusha/catalog-v1.norito".into(),
        ),
        kagemusha_promotion_controller_public_key: Some(controller.clone()),
        kagemusha_catalog_revalidation_authority_key_id: Some(
            CATALOG_REVALIDATION_AUTHORITY_KEY_ID.to_owned(),
        ),
        kagemusha_catalog_revalidation_authority_public_key: Some(catalog_revalidation_authority(
            Algorithm::Ed25519,
        )),
        kagemusha_promotion_reservation_path: Some(reservation.clone()),
        kagemusha_validator_qualification_seal_path: Some(validator_seal.clone()),
        ..Offline::default()
    }
    .parse(&mut emitter);
    assert!(emitter.into_result().is_ok());
    assert_eq!(
        actual.kagemusha_promotion_controller_public_key,
        Some(controller)
    );
    assert_eq!(
        actual
            .kagemusha_catalog_revalidation_authority_key_id
            .as_deref(),
        Some(CATALOG_REVALIDATION_AUTHORITY_KEY_ID)
    );
    assert_eq!(
        actual.kagemusha_catalog_revalidation_authority_public_key,
        Some(catalog_revalidation_authority(Algorithm::Ed25519))
    );
    assert_eq!(
        actual.kagemusha_promotion_reservation_path,
        Some(reservation)
    );
    assert_eq!(
        actual.kagemusha_validator_qualification_seal_path,
        Some(validator_seal)
    );
}

#[test]
fn offline_parse_rejects_partial_or_unsealed_validator_qualification_configuration() {
    for offline in [
        Offline {
            kagemusha_promotion_controller_public_key: Some(promotion_controller(
                Algorithm::Ed25519,
            )),
            ..Offline::default()
        },
        Offline {
            kagemusha_catalog_revalidation_authority_key_id: Some(
                CATALOG_REVALIDATION_AUTHORITY_KEY_ID.to_owned(),
            ),
            ..Offline::default()
        },
        Offline {
            kagemusha_catalog_revalidation_authority_public_key: Some(
                catalog_revalidation_authority(Algorithm::Ed25519),
            ),
            ..Offline::default()
        },
        Offline {
            kagemusha_promotion_reservation_path: Some(
                "/Library/SORA/Kagemusha/reservation-v1.norito".into(),
            ),
            ..Offline::default()
        },
        Offline {
            kagemusha_validator_qualification_seal_path: Some(
                "/Library/SORA/Kagemusha/validator-v1.norito".into(),
            ),
            ..Offline::default()
        },
    ] {
        let mut emitter = Emitter::new();
        let _ = offline.parse(&mut emitter);
        assert!(emitter.into_result().is_err());
    }

    let mut emitter = Emitter::new();
    let _ = Offline {
        kagemusha_promotion_controller_public_key: Some(promotion_controller(Algorithm::Ed25519)),
        kagemusha_promotion_reservation_path: Some(
            "/Library/SORA/Kagemusha/reservation-v1.norito".into(),
        ),
        kagemusha_validator_qualification_seal_path: Some(
            "/Library/SORA/Kagemusha/validator-v1.norito".into(),
        ),
        ..Offline::default()
    }
    .parse(&mut emitter);
    assert!(emitter.into_result().is_err());
}

#[test]
fn offline_parse_rejects_invalid_validator_qualification_identity_and_paths() {
    let complete = |controller, reservation: PathBuf, validator: PathBuf| Offline {
        kagemusha_release_policy_path: Some("policy.norito".into()),
        kagemusha_artifact_dir: Some("artifacts".into()),
        kagemusha_catalog_qualification_seal_path: Some(
            "/Library/SORA/Kagemusha/catalog-v1.norito".into(),
        ),
        kagemusha_promotion_controller_public_key: Some(controller),
        kagemusha_catalog_revalidation_authority_key_id: Some(
            CATALOG_REVALIDATION_AUTHORITY_KEY_ID.to_owned(),
        ),
        kagemusha_catalog_revalidation_authority_public_key: Some(catalog_revalidation_authority(
            Algorithm::Ed25519,
        )),
        kagemusha_promotion_reservation_path: Some(reservation),
        kagemusha_validator_qualification_seal_path: Some(validator),
        ..Offline::default()
    };
    for offline in [
        complete(
            promotion_controller(Algorithm::BlsNormal),
            "/Library/SORA/Kagemusha/reservation-v1.norito".into(),
            "/Library/SORA/Kagemusha/validator-v1.norito".into(),
        ),
        complete(
            promotion_controller(Algorithm::Ed25519),
            "reservation-v1.norito".into(),
            "/Library/SORA/Kagemusha/validator-v1.norito".into(),
        ),
        complete(
            promotion_controller(Algorithm::Ed25519),
            "/Library/SORA/Kagemusha/../reservation-v1.norito".into(),
            "/Library/SORA/Kagemusha/validator-v1.norito".into(),
        ),
        complete(
            promotion_controller(Algorithm::Ed25519),
            "/Library/SORA/Kagemusha/catalog-v1.norito".into(),
            "/Library/SORA/Kagemusha/validator-v1.norito".into(),
        ),
        Offline {
            kagemusha_catalog_revalidation_authority_key_id: Some("bad:key".to_owned()),
            ..complete(
                promotion_controller(Algorithm::Ed25519),
                "/Library/SORA/Kagemusha/reservation-v1.norito".into(),
                "/Library/SORA/Kagemusha/validator-v1.norito".into(),
            )
        },
        Offline {
            kagemusha_catalog_revalidation_authority_key_id: Some(String::new()),
            ..complete(
                promotion_controller(Algorithm::Ed25519),
                "/Library/SORA/Kagemusha/reservation-v1.norito".into(),
                "/Library/SORA/Kagemusha/validator-v1.norito".into(),
            )
        },
        Offline {
            kagemusha_catalog_revalidation_authority_key_id: Some(".bad-prefix".to_owned()),
            ..complete(
                promotion_controller(Algorithm::Ed25519),
                "/Library/SORA/Kagemusha/reservation-v1.norito".into(),
                "/Library/SORA/Kagemusha/validator-v1.norito".into(),
            )
        },
        Offline {
            kagemusha_catalog_revalidation_authority_key_id: Some(format!("a{}", "b".repeat(128))),
            ..complete(
                promotion_controller(Algorithm::Ed25519),
                "/Library/SORA/Kagemusha/reservation-v1.norito".into(),
                "/Library/SORA/Kagemusha/validator-v1.norito".into(),
            )
        },
        Offline {
            kagemusha_catalog_revalidation_authority_public_key: Some(
                catalog_revalidation_authority(Algorithm::BlsNormal),
            ),
            ..complete(
                promotion_controller(Algorithm::Ed25519),
                "/Library/SORA/Kagemusha/reservation-v1.norito".into(),
                "/Library/SORA/Kagemusha/validator-v1.norito".into(),
            )
        },
    ] {
        let mut emitter = Emitter::new();
        let _ = offline.parse(&mut emitter);
        assert!(emitter.into_result().is_err());
    }

    let controller = promotion_controller(Algorithm::Ed25519);
    let mut overlapping = complete(
        controller.clone(),
        "/Library/SORA/Kagemusha/reservation-v1.norito".into(),
        "/Library/SORA/Kagemusha/validator-v1.norito".into(),
    );
    overlapping.kagemusha_catalog_revalidation_authority_public_key = Some(controller);
    let mut emitter = Emitter::new();
    let _ = overlapping.parse(&mut emitter);
    assert!(emitter.into_result().is_err());
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
