//! Enrollment borrows the actual exclusive provider; failures never move or drop its custody.
use super::*;
use crate::kagemusha_wallet_advance_v1::{
    KagemushaWalletChallengeLivenessV1, KagemushaWalletEnrollmentDatesV1,
    KagemushaWalletEnrollmentStepV1, KagemushaWalletKeyProfileV1, KagemushaWalletProviderV1,
    KagemushaWalletSlotIdV1, KagemushaWalletUnavailableV1,
};

fn invalid() -> ProviderError {
    ProviderError::Invalid {
        field: "native enrollment originals",
    }
}

fn evidence_matches_policy(
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
            Profile::TeeOnly,
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
fn verify_initial_credential_marker(
    credential: &KagemushaWalletCredentialV1,
    scheme: &KagemushaWalletSchemeV1,
    certificate: &KagemushaWalletSignerCertificateV1,
    challenge: &KagemushaWalletEnrollmentChallengeV1,
    marker: &KagemushaWalletMarkerV1,
) -> Result<(), ProviderError> {
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

/// Native-selected existing enrollment operation; its slot cannot be supplied by foreign callers.
/// Construction resolves the unique durable intent under the authenticated original selection.
#[expect(
    missing_copy_implementations,
    reason = "Retries borrow the same private Native-selected operation; this selection must not be duplicated"
)]
pub struct NativeEnrollmentOperationV1 {
    slot: KagemushaWalletSlotIdV1,
    challenge: KagemushaWalletEnrollmentChallengeV1,
    profile: KagemushaWalletKeyProfileV1,
    dates: KagemushaWalletEnrollmentDatesV1,
}

impl<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1, S: OriginalSourceV1 + Send>
    NativeWalletRuntimeV1<F, P, S>
{
    fn enrollment_provider(
        &mut self,
    ) -> Result<&mut KagemushaWalletProviderV1<F, P>, ProviderError> {
        match &mut self.custody {
            RuntimeCustodyV1::Exclusive(provider) => Ok(provider),
            _ => Err(ProviderError::Unavailable(
                KagemushaWalletUnavailableV1::Busy,
            )),
        }
    }
    fn enrollment_intent(
        &mut self,
        operation: &NativeEnrollmentOperationV1,
    ) -> Result<(), ProviderError> {
        let provider = self.enrollment_provider()?;
        let intent = provider.read_intent(&operation.slot)?.ok_or_else(invalid)?;
        if intent.slot != operation.slot.0
            || intent.challenge != operation.challenge
            || intent.profile != operation.profile.tag()
            || intent.dates != operation.dates
            || intent.challenge.scheme_id != *provider.scheme_id()
        {
            return Err(invalid());
        }
        Ok(())
    }

    /// Resolve the unique actual intent, never create a slot or generation grant.
    /// # Errors
    /// Missing/ambiguous intent, changed original selection or unavailable custody.
    pub fn enrollment_operation(
        &mut self,
        challenge: &KagemushaWalletEnrollmentChallengeV1,
        profile: KagemushaWalletKeyProfileV1,
        dates: KagemushaWalletEnrollmentDatesV1,
    ) -> Result<NativeEnrollmentOperationV1, ProviderError> {
        let slot = self
            .enrollment_provider()?
            .enrollment_slot(challenge, profile, dates)?
            .ok_or_else(invalid)?;
        Ok(NativeEnrollmentOperationV1 {
            slot,
            challenge: *challenge,
            profile,
            dates,
        })
    }

    /// Read only the actual current enrollment marker's payment public key.
    /// # Errors
    /// Changed intent, non-enrollment marker, missing or unavailable key.
    pub fn enrollment_payment_key(
        &mut self,
        operation: &NativeEnrollmentOperationV1,
    ) -> Result<KagemushaDevicePublicKeyV1, ProviderError> {
        self.enrollment_intent(operation)?;
        let provider = self.enrollment_provider()?;
        let SlotStatus::Enrollment(marker) = provider.status(&operation.slot)? else {
            return Err(invalid());
        };
        match provider.probe_payment_key(&operation.slot)? {
            crate::kagemusha_wallet_advance_v1::KagemushaWalletProbeV1::Present(key)
                if key == *marker.payment_key() =>
            {
                Ok(key)
            }
            crate::kagemusha_wallet_advance_v1::KagemushaWalletProbeV1::Unavailable(reason) => {
                Err(ProviderError::Unavailable(reason))
            }
            _ => Err(ProviderError::KeyLost),
        }
    }

    /// Read bounded original Android evidence from this operation's actual retained platform.
    /// # Errors
    /// Changed intent, marker/key mismatch or unavailable/oversized attestation chain.
    pub fn enrollment_attestation_chain(
        &mut self,
        operation: &NativeEnrollmentOperationV1,
    ) -> Result<Vec<Vec<u8>>, ProviderError> {
        self.enrollment_intent(operation)?;
        self.enrollment_provider()?
            .enrollment_attestation_chain(&operation.slot)
    }

    /// Recover the exact durable request without rereading or replacing platform evidence.
    /// # Errors
    /// Changed intent or unavailable/invalid retained request.
    pub fn enrollment_request(
        &mut self,
        operation: &NativeEnrollmentOperationV1,
    ) -> Result<Option<Vec<u8>>, ProviderError> {
        self.enrollment_intent(operation)?;
        self.enrollment_provider()?
            .recover_enrollment_request(&operation.slot)
    }

    /// Begin or resume the same exact E1 under Native-authenticated enrollment policy.
    /// # Errors
    /// Nonexclusive custody, invalid E1, unavailable storage or an interrupted provider step.
    pub fn begin_enrollment(
        &mut self,
        challenge: &KagemushaWalletEnrollmentChallengeV1,
        profile: KagemushaWalletKeyProfileV1,
        dates: KagemushaWalletEnrollmentDatesV1,
    ) -> Result<KagemushaWalletEnrollmentStepV1, ProviderError> {
        self.enrollment_provider()?
            .begin_or_resume_enrollment(challenge, profile, dates)
    }

    /// Resume only the slot whose durable intent contains the exact retained E1 and profile.
    /// A loaded intent cannot recreate the private original fresh-generation grant.
    /// # Errors
    /// Changed intent, completed enrollment, nonexclusive custody or provider unavailability.
    pub fn resume_enrollment(
        &mut self,
        challenge: &KagemushaWalletEnrollmentChallengeV1,
        profile: KagemushaWalletKeyProfileV1,
        dates: KagemushaWalletEnrollmentDatesV1,
    ) -> Result<KagemushaWalletEnrollmentStepV1, ProviderError> {
        let operation = self.enrollment_operation(challenge, profile, dates)?;
        self.enrollment_provider()?
            .resume_enrollment(&operation.slot, KagemushaWalletChallengeLivenessV1::Live)
    }

    /// Retain the exact transport request before sending; an earlier retained request wins.
    /// This is durable DATA, not issuer or account authorization.
    /// # Errors
    /// Changed intent, invalid/unbounded request or provider publication failure.
    pub fn retain_enrollment_request(
        &mut self,
        operation: &NativeEnrollmentOperationV1,
        request: &[u8],
    ) -> Result<Vec<u8>, ProviderError> {
        self.enrollment_intent(operation)?;
        self.enrollment_provider()?
            .retain_enrollment_request(&operation.slot, request)
    }

    /// Verify the genuine initial credential against the actual enrollment marker and
    /// rooted issuer originals before the provider publishes its immutable credential.
    /// After monetary advance only the already-retained identical initial credential may be
    /// retried; the current marker and payment key remain unchanged.
    /// # Errors
    /// Changed issuer/E1/key/regulator, missing retained request or publication failure.
    pub fn store_enrollment_credential(
        &mut self,
        operation: &NativeEnrollmentOperationV1,
        policy: &KagemushaWalletEnrollmentPolicyV1,
        original: &[u8],
        certificates: &[u8],
    ) -> Result<(), ProviderError> {
        self.enrollment_intent(operation)?;
        let challenge = &operation.challenge;
        let slot = &operation.slot;
        policy.validate().map_err(|_| invalid())?;
        if policy.scheme_id != challenge.scheme_id
            || policy.asset_digest != challenge.asset_digest
            || policy.app_policy != challenge.app_policy
            || policy.policy_digest().map_err(|_| invalid())? != challenge.enrollment_policy
        {
            return Err(invalid());
        }
        if certificates.is_empty() || certificates.len() > KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1 {
            return Err(invalid());
        }
        let credential =
            KagemushaWalletCredentialV1::decode_canonical(original, &challenge.scheme_id)
                .map_err(|_| invalid())?;
        let certificates: KagemushaWalletCertificateSetV1 = norito::decode_canonical_with_limits(
            certificates,
            norito::canonical_decode_limits(certificates.len()),
        )
        .map_err(|_| invalid())?;
        let scheme = *self.installed.verifier().scheme();
        certificates.verify(&scheme).map_err(|_| invalid())?;
        if certificates.certificates.len() != 1
            || credential.body.regulatory_policy != policy.regulatory_policy
            || !evidence_matches_policy(
                policy.platform,
                operation.profile,
                credential.body.evidence_kind,
            )
            || !(operation.dates.issued_at_ms..operation.dates.expires_at_ms)
                .contains(&credential.body.issued_at_ms)
            || !(operation.dates.issued_at_ms..operation.dates.expires_at_ms)
                .contains(&credential.body.enrollment_evidence.time_ms)
        {
            return Err(invalid());
        }
        let certificate = certificates
            .certificate(
                &credential.body.issuer_certificate,
                KagemushaWalletSignerRoleV1::Enrollment,
            )
            .map_err(|_| invalid())?;
        let provider = self.enrollment_provider()?;
        let marker = match provider.status(slot)? {
            SlotStatus::Enrollment(marker) => marker,
            SlotStatus::Pending(marker) | SlotStatus::Released(marker) => {
                // Monetary use already required credential zero. Reopening may retry its
                // exact publication, but cannot replace or reconstruct missing custody data.
                match provider.credential(slot, 0)? {
                    Some(retained) if retained == original => {}
                    Some(_) => return Err(invalid()),
                    None => {
                        return Err(ProviderError::UnavailableCustodyData {
                            object: "initial enrollment credential",
                        });
                    }
                }
                marker
            }
            SlotStatus::Terminal(_) => return Err(ProviderError::Terminal),
            _ => return Err(invalid()),
        };
        verify_initial_credential_marker(
            &credential,
            &scheme,
            certificate,
            challenge,
            marker.marker(),
        )?;
        match provider.probe_payment_key(slot)? {
            crate::kagemusha_wallet_advance_v1::KagemushaWalletProbeV1::Present(key)
                if key == *marker.payment_key() => {}
            crate::kagemusha_wallet_advance_v1::KagemushaWalletProbeV1::Unavailable(reason) => {
                return Err(ProviderError::Unavailable(reason));
            }
            _ => return Err(ProviderError::KeyLost),
        }
        provider.store_credential(slot, 0, original)
    }
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
                Profile::TeeOnly,
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
                    Profile::TeeOnly,
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

    fn original_attempt() -> (
        KagemushaWalletSchemeV1,
        KagemushaWalletSignerCertificateV1,
        KagemushaWalletEnrollmentChallengeV1,
        KagemushaWalletCredentialV1,
        SigningKey,
    ) {
        // Public Model fixture keys sign test DATA only; no Native owner or financial proof
        // is constructed. The certificate is the original rooted Enrollment-role certificate.
        use crate::kagemusha_wallet_state_v1::tests::{enrollment_issuer, fixture};
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
        assert_eq!(
            verify_initial_credential_marker(&credential, &scheme, &issuer, &challenge, &marker),
            Ok(())
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
