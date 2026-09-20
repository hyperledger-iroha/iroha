//! Distinct final-promotion role, purpose and sole canonical wire admission.

use super::*;

#[test]
fn final_promotion_role_is_distinct_and_only_admits_ed25519() {
    let role = SignerRoleV1::FinalPromotionProvenance;
    assert_eq!(role as u8, 14);
    assert_eq!(role.as_str(), "final_promotion_provenance");
    assert_eq!(
        role.domain(),
        "sorafs.production-readiness.final-promotion-provenance.v1"
    );
    assert_eq!(role.to_string().parse::<SignerRoleV1>().unwrap(), role);
    assert!(role.allows_algorithm(SignerKeyAlgorithmV1::Ed25519));
    assert!(!role.allows_algorithm(SignerKeyAlgorithmV1::MlDsa));
    assert_ne!(role.domain(), SignerRoleV1::Promotion.domain());
    assert_ne!(role.domain(), SignerRoleV1::ReleaseManifest.domain());
    for alias in [
        "final-promotion",
        "FinalPromotionProvenance",
        "final_promotion",
        "final_promotion_provenance ",
    ] {
        assert!(alias.parse::<SignerRoleV1>().is_err());
    }
}

#[test]
fn final_promotion_purpose_rejects_every_other_role_and_malformed_deployment() {
    let purpose = SignerPurposeBindingV1::FinalPromotionProvenance {
        deployment_id: "production-primary".into(),
    };
    for role in [
        SignerRoleV1::ProofOutcome,
        SignerRoleV1::Repair,
        SignerRoleV1::Reserve,
        SignerRoleV1::Orderbook,
        SignerRoleV1::Promotion,
        SignerRoleV1::GovernanceDag,
        SignerRoleV1::PotrGateway,
        SignerRoleV1::PotrProvider,
        SignerRoleV1::BillingStatement,
        SignerRoleV1::EvidenceViewer,
        SignerRoleV1::StreamToken,
        SignerRoleV1::PopCredentials,
        SignerRoleV1::ReleaseManifest,
        SignerRoleV1::FinalPromotionProvenance,
        SignerRoleV1::FinalPromotionAccountTransaction,
    ] {
        assert_eq!(
            purpose.validates_role(role),
            role == SignerRoleV1::FinalPromotionProvenance
        );
    }
    assert!(
        !SignerPurposeBindingV1::NativeOrPromotion
            .validates_role(SignerRoleV1::FinalPromotionProvenance)
    );
    assert!(
        !SignerPurposeBindingV1::ReleaseManifest {
            deployment_id: "production-primary".into()
        }
        .validates_role(SignerRoleV1::FinalPromotionProvenance)
    );
    for deployment_id in [
        String::new(),
        "production/primary".into(),
        "test-primary".into(),
        "x".repeat(129),
    ] {
        assert!(
            !SignerPurposeBindingV1::FinalPromotionProvenance { deployment_id }
                .validates_role(SignerRoleV1::FinalPromotionProvenance)
        );
    }
}

#[test]
fn final_promotion_role_and_deployment_roundtrip_in_the_canonical_frame() {
    let role = SignerRoleV1::FinalPromotionProvenance;
    let purpose = SignerPurposeBindingV1::FinalPromotionProvenance {
        deployment_id: "production-primary".into(),
    };
    let other = SignerPurposeBindingV1::FinalPromotionProvenance {
        deployment_id: "production-secondary".into(),
    };
    let canonical = norito::encode_canonical(&purpose).unwrap();
    assert_ne!(canonical, norito::encode_canonical(&other).unwrap());
    assert_ne!(
        canonical,
        norito::encode_canonical(&SignerPurposeBindingV1::ReleaseManifest {
            deployment_id: "production-primary".into()
        })
        .unwrap()
    );
    for flags in crate::canonical_test_support::supported_layouts() {
        let _layout = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(norito::encode_canonical(&purpose).unwrap(), canonical);
        assert_eq!(
            norito::decode_canonical::<SignerPurposeBindingV1>(&canonical).unwrap(),
            purpose
        );
        assert_eq!(
            norito::decode_canonical::<SignerRoleV1>(&norito::encode_canonical(&role).unwrap())
                .unwrap(),
            role
        );
    }
}
