//! Full catalog/issuer/account/device verification under explicit fixed test keys.
//! Issuer times are deliberately historical; only the actual native clock bounds freshness.

use super::*;
use iroha_crypto::{Hash, HashOf, KeyPair, Signature, SignatureOf};
use iroha_data_model::{
    NetworkId, account::AccountId, asset::AssetDefinitionId, kagemusha::*,
    nexus::AxtAssetIncarnationV1,
};
use iroha_model_base::topology::DataSpaceId;
use p256::ecdsa::{SigningKey, signature::Signer as _};
use sha2::{Digest as _, Sha256};

mod catalog;

mod hardware_transactions;

mod challenge_phases;

struct Fixture {
    release: Arc<KagemushaAuthenticatedReleaseV1>,
    policy: Arc<KagemushaRetailEnrollmentIssuerPolicyV1>,
    issuer: KeyPair,
    account: KeyPair,
    device: SigningKey,
    native_key: KagemushaDevicePublicKeyV1,
    owner: KagemushaRetailEnrollmentOwnerV1,
    qualification: KagemushaDeviceQualificationReplyV1,
}

impl Fixture {
    fn new() -> Self {
        let release = catalog::authenticated_release();
        let issuer = KeyPair::from_seed(vec![81; 32], Algorithm::Ed25519);
        let account = KeyPair::from_seed(vec![12; 32], Algorithm::Ed25519);
        let device = SigningKey::from_bytes((&[3; 32]).into()).unwrap();
        let native_key = public(&SigningKey::from_bytes((&[4; 32]).into()).unwrap());
        let runtime = KagemushaRetailEnrollmentRuntimeV1 {
            fi_id: "mibank".parse().unwrap(),
            ledger_dataspace_id: DataSpaceId::new(8648377547929788715),
            authentication_namespace: "mibank.bpng".parse().unwrap(),
            network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
                b"native-enrollment-test-network",
            ))),
            asset: AssetDefinitionId::from_uuid_bytes([
                0x2f, 0x17, 0xc7, 0x24, 0x66, 0xf8, 0x4a, 0x4b, 0xb8, 0xa8, 0xe2, 0x48, 0x84, 0xfd,
                0xcd, 0x2f,
            ])
            .unwrap(),
            asset_incarnation: AxtAssetIncarnationV1::try_from_bytes(
                *Hash::new(b"native-enrollment-test-incarnation").as_ref(),
            )
            .unwrap(),
            scale: 2,
        };
        let owner = KagemushaRetailEnrollmentOwnerV1 {
            account_id: AccountId::new(account.public_key().clone()),
            runtime: runtime.clone(),
            lane_id: [32; 32],
        };
        let policy = Arc::new(KagemushaRetailEnrollmentIssuerPolicyV1 {
            version: 1,
            issuer_policy_id: [71; 32],
            issuer_public_key: issuer.public_key().clone(),
            issuer_audience: "mibank-retail-enrollment".parse().unwrap(),
            runtime,
            valid_from_ms: 100,
            expires_at_ms: 9000,
            maximum_certificate_lifetime_ms: 4000,
        });
        let enabled = release.enabled_profiles()[0];
        let profile = enabled.hardware_profile;
        let device_public_key = public(&device);
        let mut credential = KagemushaHardwareCredentialV1 {
            version: 1,
            credential_id: [0; 32],
            network_id: policy.runtime.network_id,
            hardware_profile_id: profile.hardware_profile_id,
            suite_id: enabled.suite_id,
            firmware_policy_digest: profile.firmware_policy_digest,
            policy_epoch: profile.policy_epoch,
            lane_commitment: owner.lane_id,
            hardware_epoch_id: [1; 32],
            hardware_epoch_generation: 1,
            device_public_key,
            device_key_reference: kagemusha_device_key_reference_v1(&device_public_key),
            issued_at_ms: 200,
            expires_at_ms: 9000,
            governance_signature: KagemushaDeviceSignatureV1::from_raw_bytes(&[1; 64]).unwrap(),
        }
        .seal_credential_id()
        .unwrap();
        let seed = profile.provider_id[0].wrapping_add(5);
        let governance = SigningKey::from_bytes((&[seed; 32]).into()).unwrap();
        let signature: p256::ecdsa::Signature =
            governance.sign(&credential.canonical_signing_bytes().unwrap());
        credential.governance_signature = KagemushaDeviceSignatureV1::from_raw_bytes(
            &signature.normalize_s().unwrap_or(signature).to_bytes(),
        )
        .unwrap();
        credential.validate_against_profile(&profile).unwrap();
        let qualification = KagemushaDeviceQualificationReplyV1 {
            version: 1,
            operation: 1,
            release_id: release.release_id(),
            hardware_policy_digest: release.hardware_policy_digest(),
            core_authorization_key_reference: hardware_authorization_key_reference_v1(&native_key),
            profile,
            credential,
        };
        Self {
            release,
            policy,
            issuer,
            account,
            device,
            native_key,
            owner,
            qualification,
        }
    }

    fn begin(&self) -> PendingIssuerEnrollmentV1 {
        PendingIssuerEnrollmentV1::begin(
            self.policy.clone(),
            self.release.clone(),
            self.owner.clone(),
            self.native_key,
            &norito::encode_canonical(&self.qualification).unwrap(),
        )
        .unwrap()
    }

    fn proof(&self, nonce: [u8; 32]) -> KagemushaRetailEnrollmentPossessionProofV1 {
        let challenge = KagemushaRetailEnrollmentChallengeV1 {
            version: 1,
            client_nonce: nonce,
            server_nonce: [93; 32],
            issuer_policy_id: self.policy.issuer_policy_id,
            issuer_audience: self.policy.issuer_audience.clone(),
            owner: self.owner.clone(),
            issuance: KagemushaRetailEnrollmentIssuanceV1 {
                release_id: self.qualification.release_id,
                hardware_policy_digest: self.qualification.hardware_policy_digest,
                core_authorization_key_reference: self
                    .qualification
                    .core_authorization_key_reference,
                credential: self.qualification.credential,
            },
            issued_at_ms: 1000,
            expires_at_ms: 2000,
        };
        self.sign_proof(challenge)
    }

    fn sign_proof(
        &self,
        challenge: KagemushaRetailEnrollmentChallengeV1,
    ) -> KagemushaRetailEnrollmentPossessionProofV1 {
        KagemushaRetailEnrollmentPossessionProofV1 {
            account_signature: SignatureOf::from_signature(
                Signature::try_new(
                    self.account.private_key(),
                    &challenge.account_signing_message().unwrap(),
                )
                .unwrap(),
            ),
            device_response: self.device_response(challenge.device_request_id().unwrap()),
            challenge,
        }
    }

    fn device_response(&self, nonce: [u8; 32]) -> Vec<u8> {
        let body = norito::encode_canonical(&self.qualification).unwrap();
        let command = KagemushaDeviceReadCredentialCommandV1::canonical_bytes().unwrap();
        let transcript = kagemusha_device_response_signing_bytes_v1(
            1,
            nonce,
            &command,
            &body,
            self.qualification.hardware_policy_digest,
            self.qualification.profile.qualification_report_digest,
        )
        .unwrap();
        let signature: p256::ecdsa::Signature = self.device.sign(&transcript);
        let signature = signature.normalize_s().unwrap_or(signature).to_bytes();
        let mut response = b"IKGMJRS1".to_vec();
        response.extend_from_slice(&1u16.to_le_bytes());
        response.extend_from_slice(&[1, 0]);
        response.extend_from_slice(&nonce);
        response.extend_from_slice(&u32::try_from(body.len()).unwrap().to_le_bytes());
        response.extend_from_slice(&64u32.to_le_bytes());
        response.extend_from_slice(&Sha256::digest(&body));
        response.extend_from_slice(&Sha256::digest(signature));
        response.extend_from_slice(&body);
        response.extend_from_slice(&signature);
        response
    }

    fn certificate(
        &self,
        proof: &KagemushaRetailEnrollmentPossessionProofV1,
    ) -> KagemushaRetailEnrollmentCertificateV1 {
        let subject = KagemushaRetailEnrollmentSubjectV1 {
            version: 1,
            enrollment_id: proof.challenge.owner.enrollment_id().unwrap(),
            issuer_policy_id: self.policy.issuer_policy_id,
            issuer_audience: self.policy.issuer_audience.clone(),
            owner: proof.challenge.owner.clone(),
            issuance: proof.challenge.issuance.clone(),
            challenge_evidence_digest: proof.canonical_evidence_digest().unwrap(),
            issued_at_ms: 1000,
            expires_at_ms: 3000,
        };
        KagemushaRetailEnrollmentCertificateV1 {
            signature: SignatureOf::try_new(
                self.issuer.private_key(),
                &subject.approval_payload().unwrap(),
            )
            .unwrap(),
            subject,
        }
    }
}

fn public(key: &SigningKey) -> KagemushaDevicePublicKeyV1 {
    KagemushaDevicePublicKeyV1::from_sec1_bytes(
        key.verifying_key().to_encoded_point(false).as_bytes(),
    )
    .unwrap()
}

#[test]
fn initial_admission_authenticates_full_catalog_and_three_signatures_and_retains_exact_bytes() {
    let f = Fixture::new();
    let pending = f.begin();
    let nonce = pending.client_nonce().unwrap();
    assert_ne!(nonce, [0; 32]);
    let proof = f.proof(nonce);
    let certificate = f.certificate(&proof);
    let proof_bytes = proof.canonical_bytes().unwrap();
    let certificate_bytes = certificate.canonical_bytes().unwrap();
    let admission = pending.complete(&proof_bytes, &certificate_bytes).unwrap();
    assert_eq!(admission.evidence().certificate(), &certificate);
    assert_eq!(admission.evidence().client_nonce(), nonce);
    assert_eq!(admission.canonical_proof(), proof_bytes);
    assert_eq!(admission.canonical_certificate(), certificate_bytes);
    assert_eq!(admission.enrollment_binding().owner, f.owner);
    assert_eq!(admission.native_authorization_public_key(), &f.native_key);
    assert_eq!(admission.issuer_policy(), f.policy.as_ref());
    assert_eq!(admission.release().release_id(), f.release.release_id());
    admission.deadline().unwrap().check().unwrap();
}

#[test]
fn a_saved_issuer_response_cannot_complete_a_new_native_attempt() {
    let f = Fixture::new();
    let old = f.begin();
    let new = f.begin();
    assert_ne!(old.client_nonce().unwrap(), new.client_nonce().unwrap());
    let proof = f.proof(old.client_nonce().unwrap());
    let certificate = f.certificate(&proof);
    assert_eq!(
        new.complete(
            &proof.canonical_bytes().unwrap(),
            &certificate.canonical_bytes().unwrap()
        )
        .err(),
        Some(InitialEnrollmentErrorV1::Binding)
    );
    old.complete(
        &proof.canonical_bytes().unwrap(),
        &certificate.canonical_bytes().unwrap(),
    )
    .unwrap();
}

#[test]
fn changed_native_owner_release_policy_or_core_key_cannot_be_supplied_by_the_issuer() {
    let f = Fixture::new();
    for mutation in 0..5 {
        let pending = f.begin();
        let mut challenge = f.proof(pending.client_nonce().unwrap()).challenge;
        match mutation {
            0 => challenge.owner.account_id = AccountId::new(f.issuer.public_key().clone()),
            1 => challenge.issuance.release_id[0] ^= 1,
            2 => challenge.issuance.hardware_policy_digest[0] ^= 1,
            3 => challenge.issuance.core_authorization_key_reference[0] ^= 1,
            _ => challenge.owner.runtime.authentication_namespace = "other.fi".parse().unwrap(),
        }
        // Re-sign all account and issuer commitments. Retained native pins must still win.
        let proof = f.sign_proof(challenge);
        let certificate = f.certificate(&proof);
        assert_eq!(
            pending
                .complete(
                    &proof.canonical_bytes().unwrap(),
                    &certificate.canonical_bytes().unwrap()
                )
                .err(),
            Some(InitialEnrollmentErrorV1::Binding),
            "mutation {mutation}"
        );
    }
}

#[test]
fn valid_issuer_signature_cannot_replace_account_or_device_possession() {
    let f = Fixture::new();
    for account in [false, true] {
        let pending = f.begin();
        let mut proof = f.proof(pending.client_nonce().unwrap());
        if account {
            proof.account_signature = SignatureOf::try_new(
                f.issuer.private_key(),
                &proof.challenge.account_signing_payload().unwrap(),
            )
            .unwrap();
        } else {
            // Fresh valid frame signature over another request is still the wrong proof.
            proof.device_response = f.device_response([82; 32]);
        }
        let certificate = f.certificate(&proof);
        assert_eq!(
            pending
                .complete(
                    &proof.canonical_bytes().unwrap(),
                    &certificate.canonical_bytes().unwrap()
                )
                .err(),
            Some(InitialEnrollmentErrorV1::Authority)
        );
    }
}

#[test]
fn wrong_issuer_and_changed_proof_commitment_are_rejected() {
    let f = Fixture::new();
    for issuer in [false, true] {
        let pending = f.begin();
        let proof = f.proof(pending.client_nonce().unwrap());
        let mut certificate = f.certificate(&proof);
        if issuer {
            certificate.signature = SignatureOf::try_new(
                f.account.private_key(),
                &certificate.subject.approval_payload().unwrap(),
            )
            .unwrap();
        } else {
            certificate.subject.challenge_evidence_digest[0] ^= 1;
            certificate.signature = SignatureOf::try_new(
                f.issuer.private_key(),
                &certificate.subject.approval_payload().unwrap(),
            )
            .unwrap();
        }
        assert_eq!(
            pending
                .complete(
                    &proof.canonical_bytes().unwrap(),
                    &certificate.canonical_bytes().unwrap()
                )
                .err(),
            Some(InitialEnrollmentErrorV1::Authority)
        );
    }
}

#[test]
fn expired_native_attempt_is_rejected_before_proof_parsing_or_clock_renewal() {
    let f = Fixture::new();
    let mut pending = f.begin();
    pending.deadline = NativeDeadlineV1::expired_for_test();
    assert_eq!(
        pending.client_nonce().err(),
        Some(InitialEnrollmentErrorV1::Expired)
    );
    assert_eq!(
        pending.deadline().err(),
        Some(InitialEnrollmentErrorV1::Expired)
    );
    assert_eq!(
        pending.complete(&[], &[]).err(),
        Some(InitialEnrollmentErrorV1::Expired)
    );
}

#[test]
fn canonical_input_bounds_and_trailing_bytes_are_rejected() {
    let f = Fixture::new();
    for mutation in 0..4 {
        let pending = f.begin();
        let proof = f.proof(pending.client_nonce().unwrap());
        let mut p = proof.canonical_bytes().unwrap();
        let mut c = f.certificate(&proof).canonical_bytes().unwrap();
        match mutation {
            0 => p.push(0),
            1 => c.push(0),
            2 => p = vec![0; KAGEMUSHA_RETAIL_ENROLLMENT_PROOF_MAX_BYTES_V1 + 1],
            _ => c = vec![0; 16385],
        }
        assert_eq!(
            pending.complete(&p, &c).err(),
            Some(InitialEnrollmentErrorV1::Encoding)
        );
    }
}

#[test]
fn begin_rejects_a_different_runtime_or_non_ed25519_account() {
    let f = Fixture::new();
    for algorithm in [false, true] {
        let mut owner = f.owner.clone();
        if algorithm {
            owner.account_id = AccountId::new(
                KeyPair::from_seed(vec![1; 32], Algorithm::Secp256k1)
                    .public_key()
                    .clone(),
            );
        } else {
            owner.runtime.fi_id = "other".parse().unwrap();
        }
        assert_eq!(
            PendingIssuerEnrollmentV1::begin(
                f.policy.clone(),
                f.release.clone(),
                owner,
                f.native_key,
                &norito::encode_canonical(&f.qualification).unwrap(),
            )
            .err(),
            Some(InitialEnrollmentErrorV1::Binding)
        );
    }
}

#[test]
fn initial_possession_preserves_exact_issuer_admission_and_requires_a_new_device_nonce() {
    use crate::kagemusha_core_coordinator_v1::enrolled_open::PendingEnrolledOpenV1;
    let f = Fixture::new();
    let enrollment = f.begin();
    let proof = f.proof(enrollment.client_nonce().unwrap());
    let certificate = f.certificate(&proof);
    let proof_bytes = proof.canonical_bytes().unwrap();
    let certificate_bytes = certificate.canonical_bytes().unwrap();
    let admission = enrollment
        .complete(&proof_bytes, &certificate_bytes)
        .unwrap();
    let open = PendingEnrolledOpenV1::from_fresh_issuer_admission(admission).unwrap();
    assert_ne!(open.nonce(), proof.challenge.device_request_id().unwrap());
    assert_eq!(open.enrollment_binding().owner, f.owner);
    let account_signature =
        Signature::try_new(f.account.private_key(), &open.account_signing_message()).unwrap();
    let response = f.device_response(open.nonce());
    let completed = open
        .complete(account_signature.payload(), &response)
        .unwrap();
    let original = completed.evidence().initial_enrollment().unwrap();
    assert_eq!(original.canonical_proof(), proof_bytes);
    assert_eq!(original.canonical_certificate(), certificate_bytes);
    assert_eq!(original.evidence().certificate().subject.issued_at_ms, 1000);
    assert_eq!(original.evidence().certificate(), &certificate);
    completed.into_parts().unwrap();
}

#[test]
fn transitioning_to_possession_does_not_restart_the_native_deadline() {
    use crate::kagemusha_core_coordinator_v1::enrolled_open::{
        EnrolledOpenErrorV1, PendingEnrolledOpenV1,
    };
    let f = Fixture::new();
    let pending = f.begin();
    let proof = f.proof(pending.client_nonce().unwrap());
    let certificate = f.certificate(&proof);
    let mut admission = pending
        .complete(
            &proof.canonical_bytes().unwrap(),
            &certificate.canonical_bytes().unwrap(),
        )
        .unwrap();
    // An actual short native clock interval catches an accidental new 120-second lease.
    let original = NativeDeadlineV1::start(Duration::from_millis(250)).unwrap();
    admission.pending.deadline = original.clone();
    let open = PendingEnrolledOpenV1::from_fresh_issuer_admission(admission).unwrap();
    while original.check().is_ok() {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        open.require_unexpired().err(),
        Some(EnrolledOpenErrorV1::Expired)
    );
}

#[test]
fn admission_expiry_between_issuer_verification_and_possession_start_is_rejected() {
    use crate::kagemusha_core_coordinator_v1::enrolled_open::{
        EnrolledOpenErrorV1, PendingEnrolledOpenV1,
    };
    let f = Fixture::new();
    let pending = f.begin();
    let proof = f.proof(pending.client_nonce().unwrap());
    let certificate = f.certificate(&proof);
    let mut admission = pending
        .complete(
            &proof.canonical_bytes().unwrap(),
            &certificate.canonical_bytes().unwrap(),
        )
        .unwrap();
    admission.pending.deadline = NativeDeadlineV1::expired_for_test();
    assert_eq!(
        PendingEnrolledOpenV1::from_fresh_issuer_admission(admission).err(),
        Some(EnrolledOpenErrorV1::Expired)
    );
}
