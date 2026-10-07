//! Initial credentials remain bound to the signed policy and actual wallet incarnation.

use super::owner::Error;
use crate::kagemusha_wallet_advance_v1::KagemushaWalletKeyProfileV1;
use iroha_data_model::kagemusha::*;

fn invalid() -> Error {
    Error::Original("initial enrollment credential binding")
}

pub(super) fn evidence_matches_policy(
    platform: KagemushaWalletEnrollmentPlatformV1,
    profile: KagemushaWalletKeyProfileV1,
    evidence: KagemushaWalletEvidenceKindV1,
) -> bool {
    use KagemushaWalletAndroidHardwareV1 as Hardware;
    use KagemushaWalletEnrollmentPlatformV1 as Platform;
    use KagemushaWalletEvidenceKindV1 as Evidence;
    use KagemushaWalletKeyProfileV1 as Profile;
    matches!(
        (platform, profile, evidence),
        (
            Platform::Android {
                hardware: Hardware::Tee,
                ..
            },
            Profile::AndroidTee,
            Evidence::AndroidKeyMintTee
        ) | (
            Platform::Android {
                hardware: Hardware::StrongBox,
                ..
            },
            Profile::SecureElement,
            Evidence::AndroidKeyMintStrongBox
        ) | (
            Platform::Android {
                hardware: Hardware::TeeOrStrongBox,
                ..
            },
            Profile::SecureElementOrTee,
            Evidence::AndroidKeyMintTee | Evidence::AndroidKeyMintStrongBox
        ) | (
            Platform::Apple { .. },
            Profile::SecureElement,
            Evidence::AppleAppAttest
        )
    )
}

// A later monetary marker must still name the exact original wallet incarnation. Enrollment
// verification binds issuer/E1/key; these checks also bind the current durable marker's scope.
pub(super) fn verify_initial_credential_marker(
    credential: &KagemushaWalletCredentialV1,
    scheme: &KagemushaWalletSchemeV1,
    certificate: &KagemushaWalletSignerCertificateV1,
    challenge: &KagemushaWalletEnrollmentChallengeV1,
    marker: &KagemushaWalletMarkerV1,
) -> Result<(), Error> {
    if marker.scheme_id != challenge.scheme_id
        || marker.asset_digest != challenge.asset_digest
        || marker.wallet_id != challenge.wallet_id(&marker.payment_key)
        || marker.wallet_id != credential.body.wallet_id
    {
        return Err(invalid());
    }
    credential
        .verify_enrollment(scheme, certificate, challenge, &marker.payment_key)
        .map_err(|_| invalid())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_evidence_requires_the_exact_signed_platform_and_hardware_profile() {
        use KagemushaWalletAndroidHardwareV1 as Hardware;
        use KagemushaWalletEnrollmentPlatformV1 as Platform;
        use KagemushaWalletEvidenceKindV1 as Evidence;
        use KagemushaWalletKeyProfileV1 as Profile;
        let android = |hardware| Platform::Android {
            attestation_root_sha256: [1; 32],
            hardware,
            patch_floor_yyyymm: 202610,
            play_integrity_maximum_age_ms: 120_000,
            require_play_recognized: true,
            require_licensed: true,
            minimum_device_integrity: KagemushaWalletPlayIntegrityLevelV1::Device,
        };
        for (platform, profile, allowed) in [
            (
                android(Hardware::Tee),
                Profile::AndroidTee,
                vec![Evidence::AndroidKeyMintTee],
            ),
            (
                android(Hardware::StrongBox),
                Profile::SecureElement,
                vec![Evidence::AndroidKeyMintStrongBox],
            ),
            (
                android(Hardware::TeeOrStrongBox),
                Profile::SecureElementOrTee,
                vec![
                    Evidence::AndroidKeyMintTee,
                    Evidence::AndroidKeyMintStrongBox,
                ],
            ),
            (
                Platform::Apple {
                    attestation_root_sha256: [1; 32],
                },
                Profile::SecureElement,
                vec![Evidence::AppleAppAttest],
            ),
        ] {
            for evidence in Evidence::ALL {
                for offered in [
                    Profile::SecureElement,
                    Profile::SecureElementOrTee,
                    Profile::AndroidTee,
                ] {
                    assert_eq!(
                        evidence_matches_policy(platform, offered, evidence),
                        offered == profile && allowed.contains(&evidence),
                        "{platform:?}, {offered:?}, {evidence:?}",
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod credential_retry_tests {
    use super::*;
    use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};

    fn fixture<T>(name: &str) -> T
    where
        T: norito::NoritoSerialize,
        for<'de> T: norito::NoritoDeserialize<'de>,
    {
        let vectors: norito::json::Value = norito::json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/kagemusha/wallet_v1_vectors.json"
        )))
        .unwrap();
        let row = vectors["objects"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["type"].as_str() == Some(name))
            .unwrap();
        let bytes = hex::decode(row["canonical_hex"].as_str().unwrap()).unwrap();
        norito::decode_canonical_with_limits(&bytes, norito::canonical_decode_limits(bytes.len()))
            .unwrap()
    }

    fn enrollment_issuer(
        credential: &KagemushaWalletCredentialV1,
    ) -> KagemushaWalletSignerCertificateV1 {
        let vectors: norito::json::Value = norito::json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/kagemusha/wallet_v1_vectors.json"
        )))
        .unwrap();
        for row in vectors["envelopes"].as_array().unwrap() {
            let bytes = hex::decode(row["canonical_hex"].as_str().unwrap()).unwrap();
            let envelope: KagemushaWalletEnvelopeV1 = norito::decode_canonical_with_limits(
                &bytes,
                norito::canonical_decode_limits(bytes.len()),
            )
            .unwrap();
            let certificates = match envelope.message {
                KagemushaWalletMessageV1::Offer { offer } => offer.certificates,
                KagemushaWalletMessageV1::Request { request } => request.certificates,
                _ => continue,
            };
            if let Some(certificate) = certificates.certificates.iter().find(|certificate| {
                certificate.certificate_digest() == credential.body.issuer_certificate
            }) {
                return *certificate;
            }
        }
        panic!("original enrollment issuer fixture");
    }

    fn original_attempt() -> (
        KagemushaWalletSchemeV1,
        KagemushaWalletSignerCertificateV1,
        KagemushaWalletEnrollmentChallengeV1,
        KagemushaWalletCredentialV1,
        SigningKey,
    ) {
        // Public Model fixture keys sign test DATA only; no Native owner or financial proof
        // is constructed. The certificate is the original rooted Enrollment-role certificate.
        let scheme = fixture("KagemushaWalletSchemeV1");
        let original: KagemushaWalletCredentialV1 = fixture("KagemushaWalletCredentialV1");
        let issuer = enrollment_issuer(&original);
        let vectors: norito::json::Value = norito::json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/kagemusha/wallet_v1_vectors.json"
        )))
        .unwrap();
        let signing = vectors["keys"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                SigningKey::from_slice(&hex::decode(row["scalar_hex"].as_str().unwrap()).unwrap())
                    .unwrap()
            })
            .find(|key| {
                key.verifying_key().to_encoded_point(false).as_bytes()
                    == issuer.body.key.as_sec1_bytes()
            })
            .expect("public original Enrollment fixture signer");
        let challenge = KagemushaWalletEnrollmentChallengeV1 {
            version: KAGEMUSHA_WALLET_VERSION_V1,
            scheme_id: original.body.scheme_id,
            asset_digest: original.body.asset_digest,
            account_digest: original.body.account_digest,
            app_policy: original.body.app_policy,
            enrollment_policy: [0x71; 32],
            issuer_nonce: [0x72; 32],
        };
        let credential = signed_attempt(original.body, &issuer, &challenge, &signing);
        (scheme, issuer, challenge, credential, signing)
    }

    fn signed_attempt(
        mut body: KagemushaWalletCredentialBodyV1,
        issuer: &KagemushaWalletSignerCertificateV1,
        challenge: &KagemushaWalletEnrollmentChallengeV1,
        key: &SigningKey,
    ) -> KagemushaWalletCredentialV1 {
        body.enrollment_id = challenge.enrollment_id(&body.payment_key);
        body.wallet_id = challenge.wallet_id(&body.payment_key);
        let signature: Signature = key.sign(&body.signing_message());
        KagemushaWalletCredentialV1::sign(
            body,
            issuer,
            KagemushaWalletSignerOutputV1::Raw(signature.to_bytes().into()),
        )
        .unwrap()
    }

    #[test]
    fn retry_verifies_exact_rooted_credential_e1_and_current_marker_scope() {
        let (scheme, issuer, challenge, credential, signing) = original_attempt();
        let marker =
            KagemushaWalletMarkerV1::enrollment(&challenge, credential.body.payment_key).unwrap();
        assert!(
            verify_initial_credential_marker(&credential, &scheme, &issuer, &challenge, &marker)
                .is_ok()
        );
        for field in 0..4 {
            let mut changed = marker;
            match field {
                0 => changed.scheme_id[0] ^= 1,
                1 => changed.asset_digest[0] ^= 1,
                2 => changed.wallet_id[0] ^= 1,
                _ => {
                    let another = SigningKey::from_slice(&[0x73; 32]).unwrap();
                    changed.payment_key = KagemushaDevicePublicKeyV1::from_sec1_bytes(
                        another.verifying_key().to_encoded_point(false).as_bytes(),
                    )
                    .unwrap();
                }
            }
            assert!(
                verify_initial_credential_marker(
                    &credential,
                    &scheme,
                    &issuer,
                    &challenge,
                    &changed
                )
                .is_err()
            );
        }
        let mut other_e1 = challenge;
        other_e1.issuer_nonce[0] ^= 1;
        assert!(
            verify_initial_credential_marker(&credential, &scheme, &issuer, &other_e1, &marker)
                .is_err()
        );
        let other_credential = signed_attempt(credential.body, &issuer, &other_e1, &signing);
        other_credential.verify(&scheme, &issuer).unwrap();
        assert!(
            verify_initial_credential_marker(
                &other_credential,
                &scheme,
                &issuer,
                &challenge,
                &marker
            )
            .is_err()
        );
        let mut bad_signature = credential;
        bad_signature.body.account_digest[0] ^= 1;
        assert!(
            verify_initial_credential_marker(&bad_signature, &scheme, &issuer, &challenge, &marker)
                .is_err()
        );
    }
}
