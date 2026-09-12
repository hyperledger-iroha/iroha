//! Exact provider-scoped stream-token purpose and first-release wire regressions.

use super::*;

#[test]
fn stream_token_purpose_requires_one_nonzero_provider_and_exact_role() {
    let purpose = SignerPurposeBindingV1::StreamToken {
        provider_id: [0x71; 32],
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
    ] {
        assert_eq!(
            purpose.validates_role(role),
            role == SignerRoleV1::StreamToken
        );
    }
    assert!(
        !SignerPurposeBindingV1::StreamToken {
            provider_id: [0; 32],
        }
        .validates_role(SignerRoleV1::StreamToken)
    );
    assert!(!SignerPurposeBindingV1::NativeOrPromotion.validates_role(SignerRoleV1::StreamToken));
    assert!(SignerRoleV1::StreamToken.allows_algorithm(SignerKeyAlgorithmV1::Ed25519));
    assert!(!SignerRoleV1::StreamToken.allows_algorithm(SignerKeyAlgorithmV1::MlDsa));
}

#[test]
fn stream_token_purpose_provider_changes_one_canonical_binding_in_every_layout() {
    let purpose = SignerPurposeBindingV1::StreamToken {
        provider_id: [0x71; 32],
    };
    let other = SignerPurposeBindingV1::StreamToken {
        provider_id: [0x72; 32],
    };
    let canonical = norito::encode_canonical(&purpose).expect("canonical provider purpose");
    let other_canonical = norito::encode_canonical(&other).expect("other canonical provider");
    assert_ne!(canonical, other_canonical);
    let flags: Vec<_> = (0..=norito::core::supported_header_flags())
        .filter(|flag| norito::core::validate_header_flags(*flag).is_ok())
        .collect();
    assert_eq!(flags.len(), 10);
    for flags in flags {
        let _guard = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(norito::encode_canonical(&purpose).unwrap(), canonical);
        assert_eq!(norito::encode_canonical(&other).unwrap(), other_canonical);
        assert_eq!(
            norito::decode_canonical::<SignerPurposeBindingV1>(&canonical).unwrap(),
            purpose
        );
        let advertised = norito::to_bytes(&purpose).expect("advertised provider frame");
        assert_eq!(
            norito::decode_from_bytes::<SignerPurposeBindingV1>(&advertised).unwrap(),
            purpose
        );
    }
}

#[test]
fn stream_token_purpose_wire_requires_provider_under_current_schema() {
    // Serialize the actual prior unit shape under the CURRENT schema header. This isolates
    // missing provider admission from an unrelated type-name/schema-hash mismatch.
    #[derive(norito::SerializePayload)]
    enum UnitPurposeShape {
        NativeOrPromotion,
        ReleaseManifest {
            deployment_id: String,
        },
        GovernanceDag {
            publisher_peer_id: Vec<u8>,
        },
        PotrGateway {
            signer_id: [u8; 32],
        },
        PotrProvider {
            signer_id: [u8; 32],
            provider_id: [u8; 32],
        },
        BillingStatement {
            signer_id: String,
        },
        EvidenceViewer,
        StreamToken,
        PopCredentials {
            issuer_id: String,
        },
    }
    let cases = [
        (
            UnitPurposeShape::NativeOrPromotion,
            Some(SignerPurposeBindingV1::NativeOrPromotion),
        ),
        (
            UnitPurposeShape::ReleaseManifest {
                deployment_id: "primary".into(),
            },
            Some(SignerPurposeBindingV1::ReleaseManifest {
                deployment_id: "primary".into(),
            }),
        ),
        (
            UnitPurposeShape::GovernanceDag {
                publisher_peer_id: vec![1],
            },
            Some(SignerPurposeBindingV1::GovernanceDag {
                publisher_peer_id: vec![1],
            }),
        ),
        (
            UnitPurposeShape::PotrGateway { signer_id: [2; 32] },
            Some(SignerPurposeBindingV1::PotrGateway { signer_id: [2; 32] }),
        ),
        (
            UnitPurposeShape::PotrProvider {
                signer_id: [3; 32],
                provider_id: [4; 32],
            },
            Some(SignerPurposeBindingV1::PotrProvider {
                signer_id: [3; 32],
                provider_id: [4; 32],
            }),
        ),
        (
            UnitPurposeShape::BillingStatement {
                signer_id: "billing-primary".into(),
            },
            Some(SignerPurposeBindingV1::BillingStatement {
                signer_id: "billing-primary".into(),
            }),
        ),
        (
            UnitPurposeShape::EvidenceViewer,
            Some(SignerPurposeBindingV1::EvidenceViewer),
        ),
        (UnitPurposeShape::StreamToken, None),
        (
            UnitPurposeShape::PopCredentials {
                issuer_id: "issuer-primary".into(),
            },
            Some(SignerPurposeBindingV1::PopCredentials {
                issuer_id: "issuer-primary".into(),
            }),
        ),
    ];
    for flags in (0..=norito::core::supported_header_flags())
        .filter(|flags| norito::core::validate_header_flags(*flags).is_ok())
    {
        let _guard = norito::core::DecodeFlagsGuard::enter(flags);
        for (raw, expected) in &cases {
            let mut bytes = Vec::new();
            norito::SerializePayload::serialize(
                raw,
                &mut norito::core::Encoder::for_buffer(&mut bytes),
            )
            .expect("serialize actual enum payload shape");
            let frame =
                norito::core::frame_bare_with_header_flags::<SignerPurposeBindingV1>(&bytes, flags)
                    .expect("frame purpose under current schema");
            let decoded = norito::decode_from_bytes::<SignerPurposeBindingV1>(&frame);
            if let Some(expected) = expected {
                assert_eq!(
                    &decoded.expect("unchanged shape is a valid control"),
                    expected
                );
            } else {
                assert!(
                    decoded.is_err(),
                    "provider-less stream purpose at layout {flags:#04x}"
                );
            }
        }
    }
}
