//! Separate account-custody role and exact first-release purpose framing.

use super::*;

const ACCOUNT: SignerRoleV1 = SignerRoleV1::FinalPromotionAccountTransaction;

#[test]
fn final_promotion_account_role_is_unique_ed25519_and_has_no_alias() {
    assert_eq!(ACCOUNT as u8, 15);
    assert_eq!(ACCOUNT.as_str(), "final_promotion_account_transaction");
    assert_eq!(
        ACCOUNT.domain(),
        "sorafs.native-transaction.final-promotion-account.v1"
    );
    assert_eq!(
        ACCOUNT.to_string().parse::<SignerRoleV1>().unwrap(),
        ACCOUNT
    );
    assert!(ACCOUNT.allows_algorithm(SignerKeyAlgorithmV1::Ed25519));
    assert!(!ACCOUNT.allows_algorithm(SignerKeyAlgorithmV1::MlDsa));
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
    ] {
        assert_ne!(ACCOUNT as u8, role as u8);
        assert_ne!(ACCOUNT.domain(), role.domain());
        assert_ne!(ACCOUNT.as_str(), role.as_str());
    }
    for alias in [
        "final_promotion_account",
        "final-promotion-account-transaction",
        "FinalPromotionAccountTransaction",
        "final_promotion_account_transaction ",
        " final_promotion_account_transaction",
        "final_promotion",
        "15",
    ] {
        assert!(alias.parse::<SignerRoleV1>().is_err(), "alias {alias:?}");
    }
}

#[test]
fn account_deployment_purpose_rejects_all_other_roles_purposes_and_invalid_identities() {
    let purpose = SignerPurposeBindingV1::FinalPromotionAccountTransaction {
        deployment_id: "production-primary".into(),
    };
    for label in [
        "proof_outcome",
        "repair",
        "reserve",
        "orderbook",
        "promotion",
        "governance_dag",
        "potr_gateway",
        "potr_provider",
        "billing_statement",
        "evidence_viewer",
        "stream_token",
        "pop_credentials",
        "release_manifest",
        "final_promotion_provenance",
        "final_promotion_account_transaction",
    ] {
        let role = label.parse::<SignerRoleV1>().unwrap();
        assert_eq!(purpose.validates_role(role), role == ACCOUNT);
    }
    for other in [
        SignerPurposeBindingV1::NativeOrPromotion,
        SignerPurposeBindingV1::ReleaseManifest {
            deployment_id: "production-primary".into(),
        },
        SignerPurposeBindingV1::GovernanceDag {
            publisher_peer_id: vec![1],
        },
        SignerPurposeBindingV1::PotrGateway { signer_id: [1; 32] },
        SignerPurposeBindingV1::PotrProvider {
            signer_id: [1; 32],
            provider_id: [2; 32],
        },
        SignerPurposeBindingV1::BillingStatement {
            signer_id: "billing-primary".into(),
        },
        SignerPurposeBindingV1::EvidenceViewer,
        SignerPurposeBindingV1::StreamToken {
            provider_id: [1; 32],
        },
        SignerPurposeBindingV1::PopCredentials {
            issuer_id: "issuer-primary".into(),
        },
        SignerPurposeBindingV1::FinalPromotionProvenance {
            deployment_id: "production-primary".into(),
        },
    ] {
        assert!(!other.validates_role(ACCOUNT), "wrong purpose {other:?}");
    }
    for deployment_id in [
        String::new(),
        "production/primary".into(),
        "production primary".into(),
        "production\0primary".into(),
        "生产".into(),
        "test-primary".into(),
        "TEST-primary".into(),
        "x".repeat(SIGNER_MAX_ID_BYTES_V1 + 1),
    ] {
        assert!(
            !SignerPurposeBindingV1::FinalPromotionAccountTransaction { deployment_id }
                .validates_role(ACCOUNT)
        );
    }
    assert!(
        SignerPurposeBindingV1::FinalPromotionAccountTransaction {
            deployment_id: "x".repeat(SIGNER_MAX_ID_BYTES_V1),
        }
        .validates_role(ACCOUNT)
    );
}

#[test]
fn account_role_and_purpose_have_one_canonical_tag_and_require_deployment_payload() {
    #[derive(norito::SerializePayload)]
    enum PurposeWire {
        #[codec(index = 10)]
        Account { deployment_id: String },
    }
    #[derive(norito::SerializePayload)]
    enum MissingDeployment {
        #[codec(index = 10)]
        Account,
    }
    #[derive(norito::SerializePayload)]
    enum AccountRoleWire {
        #[codec(index = 15)]
        Account,
    }
    #[derive(norito::SerializePayload)]
    enum UnknownRole {
        #[codec(index = 17)]
        Unknown,
    }
    let purpose = SignerPurposeBindingV1::FinalPromotionAccountTransaction {
        deployment_id: "production-primary".into(),
    };
    let canonical = norito::encode_canonical(&purpose).unwrap();
    let role_frame = norito::encode_canonical(&ACCOUNT).unwrap();
    assert_ne!(
        canonical,
        norito::encode_canonical(&SignerPurposeBindingV1::FinalPromotionProvenance {
            deployment_id: "production-primary".into(),
        })
        .unwrap()
    );
    assert_ne!(
        canonical,
        norito::encode_canonical(&SignerPurposeBindingV1::FinalPromotionAccountTransaction {
            deployment_id: "production-secondary".into(),
        })
        .unwrap()
    );
    for flags in crate::canonical_test_support::supported_layouts() {
        let _layout = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(norito::encode_canonical(&purpose).unwrap(), canonical);
        assert_eq!(norito::encode_canonical(&ACCOUNT).unwrap(), role_frame);
        assert_eq!(
            norito::decode_canonical::<SignerPurposeBindingV1>(&canonical).unwrap(),
            purpose
        );
        assert_eq!(
            norito::decode_canonical::<SignerRoleV1>(&role_frame).unwrap(),
            ACCOUNT
        );
        let mut body = Vec::new();
        norito::SerializePayload::serialize(
            &PurposeWire::Account {
                deployment_id: "production-primary".into(),
            },
            &mut norito::core::Encoder::for_buffer(&mut body),
        )
        .unwrap();
        let frame =
            norito::core::frame_bare_with_header_flags::<SignerPurposeBindingV1>(&body, flags)
                .unwrap();
        assert_eq!(
            norito::decode_from_bytes::<SignerPurposeBindingV1>(&frame).unwrap(),
            purpose
        );
        body.clear();
        norito::SerializePayload::serialize(
            &MissingDeployment::Account,
            &mut norito::core::Encoder::for_buffer(&mut body),
        )
        .unwrap();
        let frame =
            norito::core::frame_bare_with_header_flags::<SignerPurposeBindingV1>(&body, flags)
                .unwrap();
        assert!(norito::decode_from_bytes::<SignerPurposeBindingV1>(&frame).is_err());
        body.clear();
        norito::SerializePayload::serialize(
            &AccountRoleWire::Account,
            &mut norito::core::Encoder::for_buffer(&mut body),
        )
        .unwrap();
        let frame =
            norito::core::frame_bare_with_header_flags::<SignerRoleV1>(&body, flags).unwrap();
        assert_eq!(
            norito::decode_from_bytes::<SignerRoleV1>(&frame).unwrap(),
            ACCOUNT
        );
        body.clear();
        norito::SerializePayload::serialize(
            &UnknownRole::Unknown,
            &mut norito::core::Encoder::for_buffer(&mut body),
        )
        .unwrap();
        let frame =
            norito::core::frame_bare_with_header_flags::<SignerRoleV1>(&body, flags).unwrap();
        assert!(norito::decode_from_bytes::<SignerRoleV1>(&frame).is_err());
    }
}
