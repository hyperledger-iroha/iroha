//! Known-public synthetic ordinary enrollment with genuine threshold, Ed and P-256 signatures.
//!
//! Synthetic qualification reports and attestation originals exercise only model admission and
//! codecs. They do not establish physical attestation, proving-key qualification or live issuance.
//! This module exists only under the model's existing test-fixtures feature or unit tests.

use crate::kagemusha::kagemusha_release_v1::fixture_support as release_fixture;
use crate::{
    account::AccountId, asset::AssetDefinitionId, kagemusha::*, nexus::AxtAssetIncarnationV1,
};
use iroha_crypto::{Algorithm, Hash, KeyPair, Signature, SignatureOf};
use iroha_model_base::topology::DataSpaceId;
use p256::ecdsa::{Signature as P256Signature, SigningKey, signature::Signer as _};
use sha2::{Digest as _, Sha256};
use std::sync::Arc;

/// Sign an exact model issuer admission under a known-public synthetic authority seed61.
/// This fixture helper grants no production authority or physical qualification.
/// # Panics
/// Panics if the fixed signing model or fixture key shape changes.
pub fn ordinary_test_issuer_admission_v1(
    subject: KagemushaOrdinaryIssuerCircuitAdmissionSubjectV1,
) -> KagemushaOrdinaryIssuerCircuitAdmissionV1 {
    let mut hash = Sha256::new();
    hash.update(KAGEMUSHA_ORDINARY_ISSUER_P256_SEED_DOMAIN_V1);
    hash.update([61; 32]);
    let seed: [u8; 32] = hash.finalize().into();
    let key = SigningKey::from_bytes((&seed).into()).unwrap();
    let sig: P256Signature = key.sign(&subject.canonical_signing_bytes().unwrap());
    let sig = sig.normalize_s().unwrap_or(sig);
    KagemushaOrdinaryIssuerCircuitAdmissionV1 {
        subject,
        signature: KagemushaDeviceSignatureV1::from_raw_bytes(&sig.to_bytes()).unwrap(),
    }
}
/// Known-public independently selected circuit issuer point corresponding to seed61.
/// # Panics
/// Panics if the deterministic synthetic scalar is invalid.
pub fn ordinary_test_issuer_public_key_v1() -> KagemushaDevicePublicKeyV1 {
    let mut hash = Sha256::new();
    hash.update(KAGEMUSHA_ORDINARY_ISSUER_P256_SEED_DOMAIN_V1);
    hash.update([61; 32]);
    let seed: [u8; 32] = hash.finalize().into();
    let key = SigningKey::from_bytes((&seed).into()).unwrap();
    KagemushaDevicePublicKeyV1::from_sec1_bytes(
        key.verifying_key().to_encoded_point(false).as_bytes(),
    )
    .unwrap()
}

/// Complete public-key fixture originals; no software monetary owner is constructed.
pub struct KagemushaOrdinaryRetailEnrollmentFixtureV1 {
    /// Genuine threshold-authenticated release over explicit synthetic qualification reports.
    pub release: Arc<KagemushaAuthenticatedReleaseV1>,
    /// Exact selected ordinary trust policy original.
    pub trust: KagemushaOrdinaryAppTrustPolicyV1,
    /// Exact selected Ed app authority original.
    pub app_authority: KagemushaAppAttestationAuthorityPolicyV1,
    /// Exact selected FI issuer/runtime original.
    pub issuer_policy: KagemushaRetailEnrollmentIssuerPolicyV1,
    /// Independently selected owner, Core signer reference and signed preparation.
    pub selection: KagemushaOrdinaryRetailEnrollmentSelectionV1,
    /// Complete native challenge supplied to both possession signers.
    pub challenge: KagemushaOrdinaryRetailEnrollmentChallengeV1,
    /// Real wallet Ed and platform P-256 signatures under known-public test keys.
    pub proof: KagemushaOrdinaryRetailEnrollmentPossessionProofV1,
    /// Real FI Ed signature over the complete original ownership/issuance assertion.
    pub certificate: KagemushaOrdinaryRetailEnrollmentCertificateV1,
}
impl KagemushaOrdinaryRetailEnrollmentFixtureV1 {
    /// Construct independent Android or Apple model originals with real signatures.
    /// # Panics
    /// Panics if a maintained fixture schema/admission invariant changes.
    #[must_use]
    pub fn new(apple: bool) -> Self {
        Self::with_integrity(apple, false, [19; 32])
    }

    /// Construct an Android fixture with an explicit synthetic separate Integrity policy.
    /// # Panics
    /// Panics if a maintained native model invariant changes.
    #[must_use]
    pub fn android_with_integrity() -> Self {
        Self::with_integrity(false, true, [19; 32])
    }

    /// Use an independently derived known-public Native financial commitment for circuit tests.
    /// # Panics
    /// Panics if the selected commitment is zero or the maintained native fixture changes.
    #[must_use]
    pub fn with_financial_commitment(apple: bool, commitment: [u8; 32]) -> Self {
        assert_ne!(commitment, [0; 32]);
        Self::with_integrity(apple, false, commitment)
    }
    /// Construct fully signed synthetic originals for a measured Apple release and financial opening.
    /// This fixture grants no installed owner, physical evidence or monetary qualification.
    /// # Errors
    /// Rejects a category/version outside the maintained model release-digest policy.
    /// # Panics
    /// Panics for a zero financial commitment or another maintained native fixture invariant.
    pub fn measured_apple_with_financial_commitment(
        category: u32,
        version: &str,
        financial_commitment: [u8; 32],
    ) -> Result<Self, String> {
        assert_ne!(financial_commitment, [0; 32]);
        let release = crate::kagemusha::app_attest_release_extensions_digest(category, version)
            .map_err(|_| "fixture Apple release metadata differs".to_owned())?;
        Ok(Self::with_integrity_and_release(
            true,
            false,
            financial_commitment,
            release,
        ))
    }

    /// Build the actual first-release one-member W account ceremony with genuine signatures.
    /// This remains synthetic test evidence, never an installed Native or physical owner.
    /// # Panics
    /// Panics if a maintained model/signature invariant changes.
    #[must_use]
    pub fn with_single_member_wallet(
        apple: bool,
        integrity: bool,
        financial_commitment: [u8; 32],
    ) -> Self {
        Self::with_integrity_controller(apple, integrity, financial_commitment, true)
    }
    fn with_integrity(apple: bool, integrity: bool, financial_commitment: [u8; 32]) -> Self {
        Self::with_integrity_controller(apple, integrity, financial_commitment, false)
    }
    fn with_integrity_controller(
        apple: bool,
        integrity: bool,
        financial_commitment: [u8; 32],
        wallet_controller: bool,
    ) -> Self {
        Self::with_integrity_and_release_controller(
            apple,
            integrity,
            financial_commitment,
            [3; 32],
            wallet_controller,
        )
    }
    fn with_integrity_and_release(
        apple: bool,
        integrity: bool,
        financial_commitment: [u8; 32],
        app_release_digest: [u8; 32],
    ) -> Self {
        Self::with_integrity_and_release_controller(
            apple,
            integrity,
            financial_commitment,
            app_release_digest,
            false,
        )
    }
    fn with_integrity_and_release_controller(
        apple: bool,
        integrity: bool,
        financial_commitment: [u8; 32],
        app_release_digest: [u8; 32],
        wallet_controller: bool,
    ) -> Self {
        let issuer = KeyPair::from_seed(vec![61; 32], Algorithm::Ed25519);
        let wallet = KeyPair::from_seed(vec![62; 32], Algorithm::Ed25519);
        let app = SigningKey::from_bytes((&[7; 32]).into()).unwrap();
        let key = KagemushaDevicePublicKeyV1::from_sec1_bytes(
            app.verifying_key().to_encoded_point(false).as_bytes(),
        )
        .unwrap();
        let class = if apple {
            KagemushaHardwarePlatformClassV1::AppleAppAttest
        } else {
            KagemushaHardwarePlatformClassV1::AndroidKeyMint
        };
        let authority = KagemushaAppAttestationAuthorityPolicyV1 {
            authority_key: issuer.public_key().clone(),
            platform_class: class,
            app_signing_identity_digest: [2; 32],
            app_release_digest,
            maximum_lifetime_ms: 10000,
        };
        let trust = KagemushaOrdinaryAppTrustPolicyV1 {
            version: 1,
            app_authority_policy_digest: authority.canonical_digest().unwrap(),
            platform_class: class,
            allowed_android_security_levels: if apple {
                vec![]
            } else {
                vec![
                    KagemushaAppKeySecurityLevelV1::TrustedExecutionEnvironment,
                    KagemushaAppKeySecurityLevelV1::StrongBox,
                ]
            },
            play_integrity_policy: integrity.then_some(KagemushaPlayIntegrityPolicyV1 {
                policy_digest: [56; 32],
                maximum_evidence_age_ms: 1000,
                maximum_refresh_interval_ms: 1000,
                require_play_recognized: true,
                require_licensed: true,
                minimum_device_integrity: 1,
            }),
            maximum_credential_lifetime_ms: 10000,
        };
        let artifacts = release_fixture::artifacts();
        let mut receipt = release_fixture::receipt(&artifacts);
        let mut enabled = release_fixture::enabled_profile(
            7,
            receipt.profile_qualifications[0].profile.vk_digest,
        );
        enabled.hardware_profile.platform_class = class;
        enabled.hardware_profile.governance_credential_public_key =
            ordinary_test_issuer_public_key_v1();
        enabled.hardware_profile.capability_mask = class.required_guarantees();
        enabled.hardware_profile.firmware_policy_digest = trust.canonical_digest().unwrap();
        enabled
            .hardware_profile
            .app_attestation_authority_policy_digest = authority.canonical_digest().unwrap();
        enabled.hardware_profile = enabled.hardware_profile.seal_hardware_profile_id().unwrap();
        enabled.hardware_profile_id = enabled.hardware_profile.hardware_profile_id;
        receipt.profile_qualifications = vec![release_fixture::profile_qualification(
            &enabled,
            &artifacts,
            &receipt.helper_protocols,
            0x80,
        )];
        let profiles = vec![receipt.profile_qualifications[0].profile];
        receipt.hardware_policy_digest = kagemusha_hardware_policy_digest_v1(&profiles).unwrap();
        receipt.provider_policy = release_fixture::provider_policy(&profiles);
        // The ordinary profile pins the separately derived issuer point. Re-sign the
        // unchanged native provider-policy subject under that actual fixture key.
        let mut issuer_seed = Sha256::new();
        issuer_seed.update(KAGEMUSHA_ORDINARY_ISSUER_P256_SEED_DOMAIN_V1);
        issuer_seed.update([61; 32]);
        let issuer_seed: [u8; 32] = issuer_seed.finalize().into();
        let issuer_p256 = SigningKey::from_bytes((&issuer_seed).into()).unwrap();
        for entry in &mut receipt.provider_policy {
            let message = kagemusha_provider_policy_signing_bytes_v1(
                entry.hardware_profile_id,
                entry.provider_profile_index,
                entry.provider_authority_commitment,
            )
            .unwrap();
            let signature: P256Signature = issuer_p256.sign(&message);
            let signature = signature.normalize_s().unwrap_or(signature);
            entry.issuer_signature =
                KagemushaDeviceSignatureV1::from_raw_bytes(&signature.to_bytes()).unwrap();
        }
        receipt.provider_policy_root =
            kagemusha_provider_policy_root_v1(&profiles, &receipt.provider_policy).unwrap();
        let manifest = release_fixture::manifest(artifacts, &receipt);
        let release_keys = release_fixture::authority_keys();
        let release_policy = release_fixture::authority_policy(&release_keys, 2);
        let release_subject = manifest
            .release_attestation_subject(&receipt, &release_policy)
            .unwrap();
        let release_attestation = KagemushaReleaseAttestationV1 {
            version: 1,
            subject: release_subject,
            approvals: release_keys[..2]
                .iter()
                .map(|key| KagemushaReleaseApprovalV1 {
                    public_key: key.public_key().clone(),
                    signature: SignatureOf::try_new(
                        key.private_key(),
                        &release_subject.approval_payload(),
                    )
                    .unwrap(),
                })
                .collect(),
        };
        let release = Arc::new(
            manifest
                .authenticate(&receipt, &release_policy, &release_attestation)
                .unwrap(),
        );
        let runtime = KagemushaRetailEnrollmentRuntimeV1 {
            fi_id: "mibank".parse().unwrap(),
            ledger_dataspace_id: DataSpaceId::new(10),
            authentication_namespace: "mibank.bpng".parse().unwrap(),
            network_id: release.network_id(),
            asset: AssetDefinitionId::from_uuid_bytes([
                0x2f, 0x17, 0xc7, 0x24, 0x66, 0xf8, 0x4a, 0x4b, 0xb8, 0xa8, 0xe2, 0x48, 0x84, 0xfd,
                0xcd, 0x2f,
            ])
            .unwrap(),
            asset_incarnation: AxtAssetIncarnationV1::try_from_bytes(
                *Hash::new(b"ordinary-test-incarnation").as_ref(),
            )
            .unwrap(),
            scale: 2,
        };
        let owner = KagemushaRetailEnrollmentOwnerV1 {
            account_id: if wallet_controller {
                AccountId::new_multisig(
                    crate::account::MultisigPolicy::new(
                        1,
                        vec![
                            crate::account::MultisigMember::new(wallet.public_key().clone(), 1)
                                .unwrap(),
                        ],
                    )
                    .unwrap(),
                )
            } else {
                AccountId::new(wallet.public_key().clone())
            },
            runtime: runtime.clone(),
            lane_id: [16; 32],
        };
        let policy = KagemushaRetailEnrollmentIssuerPolicyV1 {
            version: 1,
            issuer_policy_id: [20; 32],
            issuer_public_key: issuer.public_key().clone(),
            issuer_audience: "test-ordinary-enrollment-service".parse().unwrap(),
            runtime,
            valid_from_ms: 1,
            expires_at_ms: 20000,
            maximum_certificate_lifetime_ms: 10000,
        };
        let preparation_subject = KagemushaOrdinaryAppEnrollmentChallengeV1 {
            version: 1,
            platform_class: class,
            enrollment_id: owner.enrollment_id().unwrap(),
            client_nonce: [13; 32],
            server_nonce: [14; 32],
            account_binding: kagemusha_ordinary_app_account_binding_v1(&owner.account_id),
            network_id: *release.network_id().as_bytes(),
            lane_id: owner.lane_id,
            release_id: release.release_id(),
            hardware_profile_id: enabled.hardware_profile_id,
            suite_id: enabled.suite_id,
            trust_policy_digest: trust.canonical_digest().unwrap(),
            app_authority_policy_digest: authority.canonical_digest().unwrap(),
            financial_authority_commitment: financial_commitment,
            issuer_policy_digest: kagemusha_ordinary_retail_issuer_policy_digest_v1(&policy)
                .unwrap(),
            policy_epoch: enabled.policy_epoch,
            hardware_epoch: 1,
            issued_at_ms: 100,
            expires_at_ms: 2000,
        };
        let preparation = KagemushaSignedOrdinaryAppEnrollmentChallengeV1 {
            challenge: preparation_subject,
            signature: Signature::try_new(
                issuer.private_key(),
                &preparation_subject.canonical_signing_bytes().unwrap(),
            )
            .unwrap(),
        };
        let raw_attestation =
            b"explicit-synthetic-platform-attestation-original-not-physical".to_vec();
        let possession_message = kagemusha_ordinary_app_enrollment_possession_message_v1(
            &preparation_subject,
            &key,
            Sha256::digest(&raw_attestation).into(),
        )
        .unwrap();
        let evidence = if apple {
            let mut auth = vec![2; 32];
            auth.push(0x40);
            auth.extend_from_slice(&11u32.to_be_bytes());
            let mut nonce = Sha256::new();
            nonce.update(&auth);
            nonce.update(Sha256::digest(&possession_message));
            let signature: P256Signature = app.sign(&nonce.finalize());
            let der = signature.to_der();
            let mut raw = vec![0xa2, 0x71];
            raw.extend_from_slice(b"authenticatorData");
            raw.extend_from_slice(&[0x58, 37]);
            raw.extend_from_slice(&auth);
            raw.push(0x69);
            raw.extend_from_slice(b"signature");
            raw.extend_from_slice(&[
                0x58,
                u8::try_from(der.as_bytes().len()).expect("P-256 DER signature fits one CBOR byte"),
            ]);
            raw.extend_from_slice(der.as_bytes());
            KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest { raw_assertion: raw }
        } else {
            let signature: P256Signature = app.sign(&possession_message);
            KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
                signature_der: signature.to_der().as_bytes().to_vec(),
            }
        };
        let raw_possession = match &evidence {
            KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore { signature_der } => {
                signature_der
            }
            KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest { raw_assertion } => {
                raw_assertion
            }
        };
        let credential_subject = KagemushaOrdinaryAppCredentialSubjectV1 {
            version: 1,
            platform_class: class,
            security_level: if apple {
                KagemushaAppKeySecurityLevelV1::AppleAppAttest
            } else {
                KagemushaAppKeySecurityLevelV1::StrongBox
            },
            enrollment_id: preparation_subject.enrollment_id,
            client_nonce: preparation_subject.client_nonce,
            server_nonce: preparation_subject.server_nonce,
            account_binding: preparation_subject.account_binding,
            network_id: preparation_subject.network_id,
            lane_id: preparation_subject.lane_id,
            release_id: preparation_subject.release_id,
            hardware_profile_id: preparation_subject.hardware_profile_id,
            suite_id: preparation_subject.suite_id,
            trust_policy_digest: preparation_subject.trust_policy_digest,
            app_authority_policy_digest: preparation_subject.app_authority_policy_digest,
            app_signing_identity_digest: authority.app_signing_identity_digest,
            app_release_digest: authority.app_release_digest,
            attested_key_id: Sha256::digest(key.as_sec1_bytes()).into(),
            app_key_reference: kagemusha_device_key_reference_v1(&key),
            financial_authority_commitment: preparation_subject.financial_authority_commitment,
            platform_evidence_digest: kagemusha_ordinary_app_enrollment_evidence_digest_v1(
                &raw_attestation,
                raw_possession,
            )
            .unwrap(),
            enrollment_challenge_digest: preparation_subject.attestation_challenge().unwrap(),
            app_public_key: key,
            policy_epoch: enabled.policy_epoch,
            hardware_epoch: 1,
            issued_at_ms: 200,
            expires_at_ms: 10200,
            app_attest_counter_floor: if apple { 11 } else { 0 },
            play_integrity: integrity.then_some(KagemushaPlayIntegrityBindingV1 {
                request_hash: preparation_subject
                    .play_integrity_request_hash(Sha256::digest(key.as_sec1_bytes()).into())
                    .unwrap(),
                evidence_digest: [58; 32],
                policy_digest: [56; 32],
                verified_at_ms: 200,
                refresh_before_ms: 1200,
            }),
        };
        let signature = Signature::try_new(
            issuer.private_key(),
            &credential_subject.canonical_signing_bytes().unwrap(),
        )
        .unwrap();
        let circuit_admission = ordinary_test_issuer_admission_v1(
            KagemushaOrdinaryAppCredentialV1::circuit_admission_subject_for(
                &credential_subject,
                &signature,
            )
            .unwrap(),
        );
        let credential = KagemushaOrdinaryAppCredentialV1 {
            subject: credential_subject,
            signature,
            circuit_admission,
        };
        let selection = KagemushaOrdinaryRetailEnrollmentSelectionV1 {
            owner,
            issuance: KagemushaOrdinaryRetailEnrollmentIssuanceV1 {
                release_id: release.release_id(),
                hardware_policy_digest: release.hardware_policy_digest(),
                core_authorization_key_reference: [30; 32],
                credential,
            },
            preparation,
        };
        let challenge = KagemushaOrdinaryRetailEnrollmentChallengeV1 {
            version: 1,
            owner: selection.owner.clone(),
            issuance: selection.issuance.clone(),
            preparation: selection.preparation.clone(),
            issued_at_ms: 250,
            expires_at_ms: 2000,
        };
        let proof = KagemushaOrdinaryRetailEnrollmentPossessionProofV1 {
            challenge: challenge.clone(),
            account_signature: SignatureOf::try_new(
                wallet.private_key(),
                &challenge.account_signing_payload().unwrap(),
            )
            .unwrap(),
            raw_attestation,
            app_possession: evidence,
        };
        let app_credential = selection
            .issuance
            .credential
            .authenticate(
                &release,
                &trust,
                &authority,
                &selection.preparation.challenge,
                &key,
                300,
            )
            .unwrap();
        let possession = proof
            .authenticate(
                &challenge,
                &selection,
                &policy,
                &release,
                &app_credential,
                if apple { Some(0) } else { None },
                300,
            )
            .unwrap();
        let subject = KagemushaOrdinaryRetailEnrollmentSubjectV1 {
            version: 1,
            enrollment_id: selection.owner.enrollment_id().unwrap(),
            issuer_policy_id: policy.issuer_policy_id,
            issuer_audience: policy.issuer_audience.clone(),
            owner: selection.owner.clone(),
            issuance: selection.issuance.clone(),
            challenge_evidence_digest: possession.evidence_digest(),
            ordinary_app_credential_digest: app_credential.digest(),
            issued_at_ms: 300,
            expires_at_ms: 9000,
        };
        let certificate = KagemushaOrdinaryRetailEnrollmentCertificateV1 {
            signature: SignatureOf::try_new(
                issuer.private_key(),
                &subject.approval_payload().unwrap(),
            )
            .unwrap(),
            subject,
        };
        let fixture = Self {
            release,
            trust,
            app_authority: authority,
            issuer_policy: policy,
            selection,
            challenge,
            proof,
            certificate,
        };
        fixture.verify(300).unwrap();
        fixture
    }
    /// Construct a separate genuine periodic lease for an Android Integrity fixture.
    /// Google evidence remains synthetic; actual Core, platform, Ed and governed P256 signatures run.
    /// # Panics
    /// Panics for a fixture without Integrity or when an actual model invariant changes.
    #[must_use]
    pub fn integrity_refresh_originals(
        &self,
    ) -> (
        KagemushaSignedPlayIntegrityRefreshChallengeV1,
        KagemushaPlayIntegrityRefreshLeaseV1,
    ) {
        let enrolled = self.verify(300).unwrap();
        let credential = enrolled.app_credential();
        let s = credential.subject();
        let issuer = KeyPair::from_seed(vec![61; 32], Algorithm::Ed25519);
        let c = KagemushaPlayIntegrityRefreshChallengeV1 {
            version: 1,
            credential_digest: credential.digest(),
            attested_key_id: s.attested_key_id,
            account_binding: s.account_binding,
            network_id: s.network_id,
            lane_id: s.lane_id,
            release_id: s.release_id,
            hardware_profile_id: s.hardware_profile_id,
            suite_id: s.suite_id,
            trust_policy_digest: s.trust_policy_digest,
            app_authority_policy_digest: s.app_authority_policy_digest,
            play_integrity_policy_digest: self.trust.play_integrity_policy.unwrap().policy_digest,
            nonce: [59; 32],
            original_enrollment_challenge_digest: s.enrollment_challenge_digest,
            policy_epoch: s.policy_epoch,
            hardware_epoch: s.hardware_epoch,
            issued_at_ms: 1300,
            expires_at_ms: 2000,
        };
        let signed = KagemushaSignedPlayIntegrityRefreshChallengeV1 {
            signature: Signature::new(issuer.private_key(), &c.canonical_signing_bytes().unwrap()),
            challenge: c,
        };
        let key = SigningKey::from_bytes((&[7; 32]).into()).unwrap();
        let signature: P256Signature = key.sign(&c.possession_signing_bytes().unwrap());
        let der = signature.to_der().as_bytes().to_vec();
        let subject = KagemushaPlayIntegrityRefreshLeaseSubjectV1 {
            version: 1,
            credential_digest: credential.digest(),
            challenge_digest: c.attempt_id().unwrap(),
            attested_key_id: c.attested_key_id,
            release_id: c.release_id,
            hardware_profile_id: c.hardware_profile_id,
            trust_policy_digest: c.trust_policy_digest,
            app_authority_policy_digest: c.app_authority_policy_digest,
            binding: KagemushaPlayIntegrityBindingV1 {
                request_hash: c.request_hash().unwrap(),
                evidence_digest: [60; 32],
                policy_digest: c.play_integrity_policy_digest,
                verified_at_ms: 1400,
                refresh_before_ms: 2400,
            },
            possession_original_digest: Sha256::digest(&der).into(),
            policy_epoch: c.policy_epoch,
            hardware_epoch: c.hardware_epoch,
            issued_at_ms: 1400,
            expires_at_ms: 2400,
        };
        let signature = Signature::new(
            issuer.private_key(),
            &subject.canonical_signing_bytes().unwrap(),
        );
        let app_possession =
            KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore { signature_der: der };
        let circuit_admission = ordinary_test_issuer_admission_v1(
            KagemushaPlayIntegrityRefreshLeaseV1::circuit_admission_subject_for(
                &subject,
                &signature,
                &app_possession,
            )
            .unwrap(),
        );
        let lease = KagemushaPlayIntegrityRefreshLeaseV1 {
            subject,
            signature,
            app_possession,
            circuit_admission,
        };
        lease
            .authenticate(
                credential,
                &self.release,
                &self.trust,
                &self.app_authority,
                &signed,
                issuer.public_key(),
                1500,
            )
            .unwrap();
        (signed, lease)
    }
    /// Run actual public current credential/dual-possession/FI certificate admission again.
    /// # Errors
    /// Rejects expired or modified fixture originals using the production model verifiers.
    pub fn verify(
        &self,
        now: u64,
    ) -> Result<KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1, String> {
        let credential = &self.selection.issuance.credential;
        let app = credential.authenticate(
            &self.release,
            &self.trust,
            &self.app_authority,
            &self.selection.preparation.challenge,
            &credential.subject.app_public_key,
            now,
        )?;
        let possession = self.proof.authenticate(
            &self.challenge,
            &self.selection,
            &self.issuer_policy,
            &self.release,
            &app,
            if credential.subject.platform_class == KagemushaHardwarePlatformClassV1::AppleAppAttest
            {
                Some(0)
            } else {
                None
            },
            now,
        )?;
        self.certificate.authenticate(
            &self.selection,
            &self.issuer_policy,
            &self.release,
            app,
            possession,
            now,
        )
    }
}

/// Emit exact native codecs with genuine known-public Ed/P256 signatures on both platforms.
/// Raw attestation and Google evidence are inert synthetic originals, never qualification.
/// # Panics
/// Panics if actual model admission, original layout or a maintained codec invariant changes.
pub fn kagemusha_ordinary_enrollment_public_codec_golden_v1() -> Vec<u8> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let mut vectors = Vec::new();
    for (name, f) in [
        (
            "android_keymint",
            KagemushaOrdinaryRetailEnrollmentFixtureV1::android_with_integrity(),
        ),
        (
            "apple_app_attest",
            KagemushaOrdinaryRetailEnrollmentFixtureV1::new(true),
        ),
    ] {
        let verified = f.verify(300).unwrap();
        let prep = &f.selection.preparation;
        let c = &prep.challenge;
        let credential = &f.selection.issuance.credential;
        let subject = &credential.subject;
        credential.original_preimage_layout().unwrap();
        let raw_sha: [u8; 32] = Sha256::digest(&f.proof.raw_attestation).into();
        let e = KagemushaAppEnrollmentPossessionChallengeV1::from_original_enrollment(
            c,
            &subject.app_public_key,
            raw_sha,
        )
        .unwrap();
        let e_signing = e.canonical_signing_bytes().unwrap();
        assert_eq!(
            e_signing,
            kagemusha_ordinary_app_enrollment_possession_message_v1(
                c,
                &subject.app_public_key,
                raw_sha
            )
            .unwrap()
        );
        let raw_subject = KagemushaRawAppAttestationAdmissionSubjectV1 {
            version: 1,
            enrollment_challenge_digest: c.attestation_challenge().unwrap(),
            authority_policy_digest: f.app_authority.canonical_digest().unwrap(),
            platform_class: subject.platform_class,
            security_level: subject.security_level,
            app_public_key: subject.app_public_key,
            attested_key_id: subject.attested_key_id,
            raw_platform_evidence_digest: raw_sha,
            app_signing_identity_digest: f.app_authority.app_signing_identity_digest,
            original_app_attest_counter: 0,
            issued_at_ms: c.issued_at_ms,
            expires_at_ms: c.expires_at_ms,
        };
        let issuer = KeyPair::from_seed(vec![61; 32], Algorithm::Ed25519);
        let raw_admission = KagemushaRawAppAttestationAdmissionV1 {
            signature: Signature::new(
                issuer.private_key(),
                &raw_subject.canonical_signing_bytes().unwrap(),
            ),
            subject: raw_subject,
        };
        raw_admission
            .authenticate(&f.release, &f.trust, &f.app_authority, c, 300)
            .unwrap();
        let raw = raw_admission.to_transport_bytes().unwrap();
        let pop = match &f.proof.app_possession {
            KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore { signature_der } => {
                norito::json!({"platform":"android_keystore", "signature_der_base64":(STANDARD.encode(signature_der))})
            }
            KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest { raw_assertion } => {
                norito::json!({"platform":"apple_app_attest", "raw_assertion_base64":(STANDARD.encode(raw_assertion))})
            }
        };
        let operation = hex::encode(c.attestation_challenge().unwrap());
        let prep_bytes = prep.to_transport_bytes().unwrap();
        let credential_bytes = credential.canonical_bytes().unwrap();
        let certificate_bytes = f.certificate.canonical_bytes().unwrap();
        let raw_request = norito::json!({
            "schema":"iroha.kagemusha.ordinary-app-raw-admission-request.v1", "operation":"issue",
            "operation_id":(operation.clone()), "signed_preparation_base64":(STANDARD.encode(&prep_bytes)),
            "attested_public_key_sec1_base64":(STANDARD.encode(subject.app_public_key.as_sec1_bytes())),
            "raw_attestation_base64":(STANDARD.encode(&f.proof.raw_attestation))});
        let certificate_request = norito::json!({
            "schema":"iroha.kagemusha.ordinary-app-credential-request.v1", "operation":"issue",
            "operation_id":(operation.clone()), "signed_preparation_base64":(STANDARD.encode(&prep_bytes)),
            "attested_public_key_sec1_base64":(STANDARD.encode(subject.app_public_key.as_sec1_bytes())),
            "raw_attestation_base64":(STANDARD.encode(&f.proof.raw_attestation)),
            "app_possession":pop, "play_integrity_token":(if name=="android_keymint" { Some("synthetic-google-evidence-no-live-verdict") } else {None})});
        let integrity_refresh = if name == "android_keymint" {
            let (challenge, lease) = f.integrity_refresh_originals();
            let c = &challenge.challenge;
            Some(norito::json!({
                "signed_refresh_challenge_base64":(STANDARD.encode(challenge.to_transport_bytes().unwrap())),
                "refresh_signing_message_base64":(STANDARD.encode(c.canonical_signing_bytes().unwrap())),
                "operation_id":(hex::encode(c.attempt_id().unwrap())),
                "play_integrity_request_hash_hex":(hex::encode(c.request_hash().unwrap())),
                "possession_signing_message_base64":(STANDARD.encode(c.possession_signing_bytes().unwrap())),
                "lease_signing_body_base64":(STANDARD.encode(&lease.subject.canonical_signing_bytes().unwrap()[lease.subject.canonical_signing_bytes().unwrap().len()-402..])),
                "lease_base64":(STANDARD.encode(lease.canonical_bytes().unwrap())),
                "lease_ed_original_base64":(STANDARD.encode(lease.ed_only_canonical_bytes().unwrap())),
                "lease_issuer_admission_base64":(STANDARD.encode(lease.circuit_admission.to_transport_bytes().unwrap())),
                "possession_der_base64":(STANDARD.encode(match &lease.app_possession{KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore{signature_der}=>signature_der,_=>unreachable!()}))
            }))
        } else {
            None
        };
        vectors.push(norito::json!({
                "platform":name, "operation_id":operation, "integrity_refresh":integrity_refresh,
                "signed_preparation_base64":(STANDARD.encode(&prep_bytes)),
                "preparation_signing_message_base64":(STANDARD.encode(c.canonical_signing_bytes().unwrap())),
                "attestation_challenge_hex":(hex::encode(c.attestation_challenge().unwrap())),
                "play_integrity_request_hash_hex":(hex::encode(c.play_integrity_request_hash(subject.attested_key_id).unwrap())),
                "app_public_key_sec1_base64":(STANDARD.encode(subject.app_public_key.as_sec1_bytes())),
                "ordinary_issuer_public_key_sec1_base64":(STANDARD.encode(verified.app_credential().circuit_admission().public_key().as_sec1_bytes())),
                "ordinary_credential_ed_original_base64":(STANDARD.encode(credential.ed_only_canonical_bytes().unwrap())),
                "ordinary_credential_issuer_admission_base64":(STANDARD.encode(credential.circuit_admission.to_transport_bytes().unwrap())),
                "attested_key_id_hex":(hex::encode(subject.attested_key_id)),
                "raw_attestation_base64":(STANDARD.encode(&f.proof.raw_attestation)),
                "raw_attestation_sha256_hex":(hex::encode(raw_sha)),
                "enrollment_possession_signing_message_base64":(STANDARD.encode(e_signing)),
                "enrollment_possession_canonical_base64":(STANDARD.encode(norito::encode_canonical(&KagemushaAppEnrollmentPossessionV1 {challenge:e,evidence:f.proof.app_possession.clone()}).unwrap())),
                "ordinary_credential_base64":(STANDARD.encode(&credential_bytes)),
                "ordinary_credential_digest_hex":(hex::encode(verified.app_credential().digest())),
                "ordinary_credential_signing_body_base64":(STANDARD.encode(&subject.canonical_signing_bytes().unwrap()[KAGEMUSHA_ORDINARY_APP_CREDENTIAL_DOMAIN_V1.len()+8..])),
                "canonical_retail_challenge_base64":(STANDARD.encode(f.challenge.canonical_bytes().unwrap())),
                "account_signing_message_base64":(STANDARD.encode(f.challenge.account_signing_message().unwrap())),
                "raw_request":raw_request,
                "raw_response":{"raw_admission_base64":(STANDARD.encode(&raw)),"raw_admission_sha256_hex":(hex::encode(Sha256::digest(&raw)))},
                "certificate_request":certificate_request,
                "certificate_response":{"certificate_base64":(STANDARD.encode(&credential_bytes)),"certificate_sha256_hex":(hex::encode(Sha256::digest(&credential_bytes)))},
                "prepare_response":{"operation_id":(hex::encode(c.attestation_challenge().unwrap())),"signed_preparation_base64":(STANDARD.encode(&prep_bytes)),"attestation_challenge_base64":(STANDARD.encode(c.attestation_challenge().unwrap())),"expires_at_ms":(c.expires_at_ms)},
                "start_request":{"signed_preparation_base64":(STANDARD.encode(prep_bytes)),"app_certificate_base64":(STANDARD.encode(credential_bytes))},
                "start_response":{"challenge_id":(hex::encode(c.attestation_challenge().unwrap())),"canonical_challenge_base64":(STANDARD.encode(f.challenge.canonical_bytes().unwrap())),"account_signing_message_base64":(STANDARD.encode(f.challenge.account_signing_message().unwrap())),"expires_at_ms":(f.challenge.expires_at_ms)},
                "finish_request":{"challenge_id":(hex::encode(c.attestation_challenge().unwrap())),"account_signature_base64":(STANDARD.encode(f.proof.account_signature.payload()))},
                "finish_response":{"challenge_id":(hex::encode(c.attestation_challenge().unwrap())),"enrollment_id_hex":(hex::encode(f.certificate.subject.owner.enrollment_id().unwrap())),"canonical_certificate_base64":(STANDARD.encode(certificate_bytes))}
            }));
    }
    let document = norito::json!({
        "schema":"iroha.kagemusha.ordinary-app-enrollment-public-codec-fixture.v1",
        "scope":"known-public synthetic signatures; raw attestation and Google evidence are inert; no physical qualification",
        "authority":false,"wallet_issuer_seed_hex":(hex::encode([62;32])),
        "app_authority_seed_hex":(hex::encode([61;32])),"platform_p256_secret_hex":(hex::encode([7;32])),
        "vectors":vectors
    });
    norito::json::to_vec(&document).unwrap()
}
