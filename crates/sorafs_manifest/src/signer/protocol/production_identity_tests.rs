//! Every textual purpose uses the same bounded identity grammar without changing exact bytes.
use super::*;

fn textual_purposes(identity: &str) -> [(SignerRoleV1, SignerPurposeBindingV1); 5] {
    use SignerPurposeBindingV1 as Purpose;
    use SignerRoleV1 as Role;
    [
        (
            Role::ReleaseManifest,
            Purpose::ReleaseManifest {
                deployment_id: identity.into(),
            },
        ),
        (
            Role::FinalPromotionProvenance,
            Purpose::FinalPromotionProvenance {
                deployment_id: identity.into(),
            },
        ),
        (
            Role::FinalPromotionAccountTransaction,
            Purpose::FinalPromotionAccountTransaction {
                deployment_id: identity.into(),
            },
        ),
        (
            Role::BillingStatement,
            Purpose::BillingStatement {
                signer_id: identity.into(),
            },
        ),
        (
            Role::PopCredentials,
            Purpose::PopCredentials {
                issuer_id: identity.into(),
            },
        ),
    ]
}

#[test]
fn every_textual_purpose_accepts_real_words_and_preserves_exact_identity_bytes() {
    for identity in ["account-attester", "attestation", "latest", "contest"] {
        for (role, purpose) in textual_purposes(identity) {
            assert!(purpose.validates_role(role), "{role:?}: {identity}");
            let frame = norito::encode_canonical(&purpose).unwrap();
            let decoded: SignerPurposeBindingV1 = norito::decode_canonical(&frame).unwrap();
            assert_eq!(decoded, purpose);
            let (_, uppercase) = textual_purposes(&identity.to_ascii_uppercase())
                .into_iter()
                .find(|(candidate, _)| *candidate == role)
                .unwrap();
            assert!(uppercase.validates_role(role));
            assert_ne!(frame, norito::encode_canonical(&uppercase).unwrap());
        }
    }
}

#[test]
fn every_textual_purpose_rejects_reserved_components_and_retains_exact_byte_ceiling() {
    for reserved in [
        "null",
        "mock",
        "test",
        "dev",
        "demo",
        "fake",
        "dummy",
        "placeholder",
    ] {
        for (role, purpose) in textual_purposes(&format!(
            "production:{}:primary",
            reserved.to_ascii_uppercase()
        )) {
            assert!(!purpose.validates_role(role), "{role:?}: {reserved}");
        }
    }
    for length in [SIGNER_MAX_ID_BYTES_V1, SIGNER_MAX_ID_BYTES_V1 + 1] {
        for (role, purpose) in textual_purposes(&"a".repeat(length)) {
            assert_eq!(
                purpose.validates_role(role),
                length == SIGNER_MAX_ID_BYTES_V1
            );
        }
    }
}
