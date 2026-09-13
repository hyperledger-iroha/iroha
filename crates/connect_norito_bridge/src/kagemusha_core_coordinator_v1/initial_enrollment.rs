//! Test-only one-use native issuer enrollment before an account/device wallet-open ceremony.
//!
//! The native owner starts one suspend-inclusive deadline and creates its client nonce before
//! HTTP. Completing this object proves a recent response by the independently pinned issuer
//! and both possession signatures. It supplies neither a hardware commit clock nor monetary
//! authority. Consuming challenge and proof phases retain the selected qualification and derive
//! signing inputs locally; certificate completion cannot substitute another proof. See the
//! adjacent `initial_enrollment/README.md` for the integration and validation boundary.
//! TODO: connect the owned pending phase to the bounded revocable native registry
//! and sole public lifecycle ABI; there is no C/JNI policy installer or host approval fallback.

use std::{sync::Arc, time::Duration};

use iroha_core::zk::kagemusha_v1_state::KagemushaRecoveryEnrollmentBindingV1;
use iroha_crypto::{Algorithm, Signature, SignatureOf};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_DEVICE_RESPONSE_MAX_BYTES_V1, KagemushaAuthenticatedReleaseV1,
    KagemushaDevicePublicKeyV1, KagemushaDeviceQualificationReplyV1,
    KagemushaDeviceReadCredentialCommandV1, KagemushaRetailEnrollmentAccountProofV1,
    KagemushaRetailEnrollmentCertificateV1, KagemushaRetailEnrollmentChallengeV1,
    KagemushaRetailEnrollmentIssuerPolicyV1, KagemushaRetailEnrollmentOwnerV1,
    KagemushaRetailEnrollmentPossessionProofV1, KagemushaRetailEnrollmentSelectionV1,
    KagemushaVerifiedRetailEnrollmentIssuerEvidenceV1, kagemusha_verify_device_response_v1,
};
use rand::{TryRngCore as _, rngs::OsRng};

use super::native_deadline::NativeDeadlineV1;
use crate::kagemusha_device_bridge_v1::sender_payload::hardware_authorization_key_reference_v1;

const LIFETIME: Duration = Duration::from_secs(120);

/// Closed initial-ceremony failures; no rejected result exposes a partial admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum InitialEnrollmentErrorV1 {
    Encoding,
    Binding,
    Authority,
    RandomUnavailable,
    Expired,
}

type Result<T> = std::result::Result<T, InitialEnrollmentErrorV1>;

/// Rust-owned pending attempt with independent immutable issuer/catalog/Core-key pins.
/// It cannot be decoded, cloned or restored from a host cache after process restart.
pub(super) struct PendingIssuerEnrollmentV1 {
    policy: Arc<KagemushaRetailEnrollmentIssuerPolicyV1>,
    release: Arc<KagemushaAuthenticatedReleaseV1>,
    enrollment: KagemushaRecoveryEnrollmentBindingV1,
    native_authorization_public_key: KagemushaDevicePublicKeyV1,
    qualification: KagemushaDeviceQualificationReplyV1,
    canonical_qualification: Vec<u8>,
    client_nonce: [u8; 32],
    deadline: NativeDeadlineV1,
}

impl PendingIssuerEnrollmentV1 {
    /// Only the qualified Rust backend supplies policy, release and native Core key pins.
    /// `owner` is initially an untrusted selector; creating this attempt grants it no authority.
    /// The backend/registry must separately reject an initial attempt for an existing Core owner.
    pub(super) fn begin(
        policy: Arc<KagemushaRetailEnrollmentIssuerPolicyV1>,
        release: Arc<KagemushaAuthenticatedReleaseV1>,
        owner: KagemushaRetailEnrollmentOwnerV1,
        native_authorization_public_key: KagemushaDevicePublicKeyV1,
        canonical_qualification: &[u8],
    ) -> Result<Self> {
        let deadline =
            NativeDeadlineV1::start(LIFETIME).map_err(|_| InitialEnrollmentErrorV1::Expired)?;
        policy
            .validate()
            .map_err(|_| InitialEnrollmentErrorV1::Authority)?;
        native_authorization_public_key
            .validate()
            .map_err(|_| InitialEnrollmentErrorV1::Authority)?;
        if owner.runtime != policy.runtime
            || owner
                .account_id
                .controller()
                .single_signatory()
                .is_none_or(|key| key.algorithm() != Algorithm::Ed25519)
        {
            return Err(InitialEnrollmentErrorV1::Binding);
        }
        let enrollment_id = owner
            .enrollment_id()
            .map_err(|_| InitialEnrollmentErrorV1::Binding)?;
        // This governed projection selects the expected device; it is not evidence of
        // current device possession. The exact command-bound response is checked later.
        let qualification =
            KagemushaDeviceQualificationReplyV1::decode_canonical_exact(canonical_qualification)
                .map_err(|_| InitialEnrollmentErrorV1::Encoding)?;
        let enabled = release
            .enabled_profile(qualification.credential.hardware_profile_id)
            .ok_or(InitialEnrollmentErrorV1::Binding)?;
        if qualification.release_id != release.release_id()
            || qualification.hardware_policy_digest != release.hardware_policy_digest()
            || qualification.core_authorization_key_reference
                != hardware_authorization_key_reference_v1(&native_authorization_public_key)
            || qualification.profile != enabled.hardware_profile
            || qualification.credential.suite_id != enabled.suite_id
            || qualification.credential.network_id != owner.runtime.network_id
            || qualification.credential.lane_commitment != owner.lane_id
        {
            return Err(InitialEnrollmentErrorV1::Binding);
        }
        let mut client_nonce = [0; 32];
        OsRng
            .try_fill_bytes(&mut client_nonce)
            .map_err(|_| InitialEnrollmentErrorV1::RandomUnavailable)?;
        if client_nonce == [0; 32] {
            return Err(InitialEnrollmentErrorV1::RandomUnavailable);
        }
        deadline
            .check()
            .map_err(|_| InitialEnrollmentErrorV1::Expired)?;
        Ok(Self {
            policy,
            release,
            enrollment: KagemushaRecoveryEnrollmentBindingV1 {
                enrollment_id,
                owner,
            },
            native_authorization_public_key,
            qualification,
            canonical_qualification: canonical_qualification.to_vec(),
            client_nonce,
            deadline,
        })
    }

    /// Read only the original native nonce; reading it never starts a new lifetime.
    pub(super) fn client_nonce(&self) -> Result<[u8; 32]> {
        self.require_unexpired()?;
        Ok(self.client_nonce)
    }

    /// Exact selected qualification body for the start request; reading never renews time.
    pub(super) fn canonical_qualification(&self) -> Result<&[u8]> {
        self.require_unexpired()?;
        Ok(&self.canonical_qualification)
    }

    pub(super) fn deadline(&self) -> Result<NativeDeadlineV1> {
        self.require_unexpired()?;
        Ok(self.deadline.clone())
    }

    fn require_unexpired(&self) -> Result<()> {
        self.deadline
            .check()
            .map(|_| ())
            .map_err(|_| InitialEnrollmentErrorV1::Expired)
    }

    /// Consume the start attempt before inspecting the issuer response. Projections supplied
    /// by HTTP are checked against locally derived bytes and never authorize signing.
    /// Native C/JNI must copy bounded inputs and consume its exact registry ticket first.
    pub(super) fn accept_challenge(
        self,
        canonical_challenge: &[u8],
        projection: IssuerChallengeProjectionV1<'_>,
    ) -> Result<AcceptedIssuerChallengeV1> {
        self.require_unexpired()?;
        let challenge =
            KagemushaRetailEnrollmentChallengeV1::decode_canonical_exact(canonical_challenge)
                .map_err(|_| InitialEnrollmentErrorV1::Encoding)?;
        self.require_challenge_binding(&challenge)?;
        let account_signing_message = challenge
            .account_signing_message()
            .map_err(|_| InitialEnrollmentErrorV1::Encoding)?;
        let device_request_id = challenge
            .device_request_id()
            .map_err(|_| InitialEnrollmentErrorV1::Encoding)?;
        let canonical_device_command = KagemushaDeviceReadCredentialCommandV1::canonical_bytes()
            .map_err(|_| InitialEnrollmentErrorV1::Encoding)?;
        if projection.challenge_id != device_request_id
            || projection.device_request_id != device_request_id
            || projection.account_signing_message != account_signing_message
            || projection.canonical_device_command != canonical_device_command
            || projection.expires_at_ms != challenge.expires_at_ms
        {
            return Err(InitialEnrollmentErrorV1::Binding);
        }
        self.require_unexpired()?;
        Ok(AcceptedIssuerChallengeV1 {
            pending: self,
            challenge,
            account_signing_message,
            device_request_id,
            canonical_device_command,
        })
    }

    fn require_challenge_binding(
        &self,
        challenge: &KagemushaRetailEnrollmentChallengeV1,
    ) -> Result<()> {
        let issuance = &challenge.issuance;
        if challenge.client_nonce != self.client_nonce
            || challenge.owner != self.enrollment.owner
            || challenge.issuer_policy_id != self.policy.issuer_policy_id
            || challenge.issuer_audience != self.policy.issuer_audience
            || issuance.release_id != self.qualification.release_id
            || issuance.hardware_policy_digest != self.qualification.hardware_policy_digest
            || issuance.core_authorization_key_reference
                != self.qualification.core_authorization_key_reference
            || issuance.credential != self.qualification.credential
            || challenge.issued_at_ms < self.policy.valid_from_ms
            || challenge.expires_at_ms > self.policy.expires_at_ms
            || challenge.issued_at_ms < self.qualification.credential.issued_at_ms
            || challenge.expires_at_ms > self.qualification.credential.expires_at_ms
        {
            return Err(InitialEnrollmentErrorV1::Binding);
        }
        // These are interval-shape checks only. No unsigned challenge timestamp becomes
        // trusted UTC; freshness remains the original native continuous deadline. Only
        // the later authenticated issuer decision checks historical issuance validity.
        Ok(())
    }

    /// Internal final step. Only a prepared proof reaches this through the callable phase
    /// API; the host cannot replace proof bytes when supplying the issuer certificate.
    fn complete(
        self,
        canonical_proof: &[u8],
        canonical_certificate: &[u8],
    ) -> Result<FreshIssuerAdmissionV1> {
        self.require_unexpired()?;
        let proof =
            KagemushaRetailEnrollmentPossessionProofV1::decode_canonical_exact(canonical_proof)
                .map_err(|_| InitialEnrollmentErrorV1::Encoding)?;
        let certificate =
            KagemushaRetailEnrollmentCertificateV1::decode_canonical_exact(canonical_certificate)
                .map_err(|_| InitialEnrollmentErrorV1::Encoding)?;
        self.require_challenge_binding(&proof.challenge)?;
        let issuance = &proof.challenge.issuance;
        // Initial credential identity comes from both possession proofs under the enabled
        // governed profile. This is allowed only before an existing Core/registry owner exists;
        // recovery must instead retain its opaque Core source and stronger credential floor.
        let expected = KagemushaRetailEnrollmentSelectionV1 {
            enrollment_id: self.enrollment.enrollment_id,
            account_id: self.enrollment.owner.account_id.clone(),
            lane_id: self.enrollment.owner.lane_id,
            issuance: issuance.clone(),
        };
        let evidence = proof
            .authenticate_issuer_evidence(
                &certificate,
                &self.policy,
                &self.release,
                &expected,
                self.client_nonce,
            )
            .map_err(|_| InitialEnrollmentErrorV1::Authority)?;
        let canonical_proof = canonical_proof.to_vec();
        let canonical_certificate = canonical_certificate.to_vec();
        self.require_unexpired()?;
        Ok(FreshIssuerAdmissionV1 {
            pending: self,
            evidence,
            canonical_proof,
            canonical_certificate,
        })
    }
}

/// Untrusted response projections, decoded from the strict HTTP response without authority.
/// The native phase compares both independently supplied identifiers with its own derivation.
pub(super) struct IssuerChallengeProjectionV1<'a> {
    pub(super) challenge_id: [u8; 32],
    pub(super) account_signing_message: [u8; 32],
    pub(super) device_request_id: [u8; 32],
    pub(super) canonical_device_command: &'a [u8],
    pub(super) expires_at_ms: u64,
}

/// One retained signing request, not an authenticated issuer decision or device admission.
/// This state cannot be cloned, deserialized, or supplied a replacement challenge.
pub(super) struct AcceptedIssuerChallengeV1 {
    pending: PendingIssuerEnrollmentV1,
    challenge: KagemushaRetailEnrollmentChallengeV1,
    account_signing_message: [u8; 32],
    device_request_id: [u8; 32],
    canonical_device_command: Vec<u8>,
}

impl AcceptedIssuerChallengeV1 {
    pub(super) fn account_signing_message(&self) -> Result<[u8; 32]> {
        self.pending.require_unexpired()?;
        Ok(self.account_signing_message)
    }

    pub(super) fn device_request_id(&self) -> Result<[u8; 32]> {
        self.pending.require_unexpired()?;
        Ok(self.device_request_id)
    }

    pub(super) fn canonical_device_command(&self) -> Result<&[u8]> {
        self.pending.require_unexpired()?;
        Ok(&self.canonical_device_command)
    }

    /// Verify the account signature and complete command-bound device frame, then retain
    /// the sole canonical proof for exact HTTP retries. No host time is used or returned.
    pub(super) fn prepare_proof(
        self,
        raw_account_signature: &[u8],
        device_response: &[u8],
    ) -> Result<PreparedIssuerProofV1> {
        self.pending.require_unexpired()?;
        if raw_account_signature.len() != 64
            || device_response.is_empty()
            || device_response.len() > KAGEMUSHA_DEVICE_RESPONSE_MAX_BYTES_V1
        {
            return Err(InitialEnrollmentErrorV1::Encoding);
        }
        let account_signature: SignatureOf<KagemushaRetailEnrollmentAccountProofV1> =
            SignatureOf::from_signature(
                Signature::try_from_bytes(raw_account_signature)
                    .map_err(|_| InitialEnrollmentErrorV1::Authority)?,
            );
        let account_key = self
            .pending
            .enrollment
            .owner
            .account_id
            .controller()
            .single_signatory()
            .ok_or(InitialEnrollmentErrorV1::Authority)?;
        account_signature
            .verify(
                account_key,
                &self
                    .challenge
                    .account_signing_payload()
                    .map_err(|_| InitialEnrollmentErrorV1::Encoding)?,
            )
            .map_err(|_| InitialEnrollmentErrorV1::Authority)?;
        let expected = &self.pending.qualification;
        let response = kagemusha_verify_device_response_v1(
            device_response,
            &self.canonical_device_command,
            1,
            self.device_request_id,
            expected.hardware_policy_digest,
            expected.profile.qualification_report_digest,
            &expected.credential.device_public_key,
        )
        .map_err(|_| InitialEnrollmentErrorV1::Authority)?;
        let qualification =
            KagemushaDeviceQualificationReplyV1::decode_canonical_exact(response.payload)
                .map_err(|_| InitialEnrollmentErrorV1::Authority)?;
        if qualification != *expected {
            return Err(InitialEnrollmentErrorV1::Binding);
        }
        let proof = KagemushaRetailEnrollmentPossessionProofV1 {
            challenge: self.challenge,
            account_signature,
            device_response: device_response.to_vec(),
        };
        let canonical_proof = proof
            .canonical_bytes()
            .map_err(|_| InitialEnrollmentErrorV1::Encoding)?;
        self.pending.require_unexpired()?;
        Ok(PreparedIssuerProofV1 {
            pending: self.pending,
            challenge_id: self.device_request_id,
            canonical_proof,
        })
    }
}

/// Exact verified possession signatures awaiting the issuer decision. It confers no issuer
/// approval, trusted UTC or monetary authority. Retrying reads these same immutable bytes.
pub(super) struct PreparedIssuerProofV1 {
    pending: PendingIssuerEnrollmentV1,
    challenge_id: [u8; 32],
    canonical_proof: Vec<u8>,
}

impl PreparedIssuerProofV1 {
    pub(super) fn challenge_id(&self) -> Result<[u8; 32]> {
        self.pending.require_unexpired()?;
        Ok(self.challenge_id)
    }

    pub(super) fn canonical_proof(&self) -> Result<&[u8]> {
        self.pending.require_unexpired()?;
        Ok(&self.canonical_proof)
    }

    /// Consume this exact proof and the original native deadline before issuer admission.
    pub(super) fn complete(self, canonical_certificate: &[u8]) -> Result<FreshIssuerAdmissionV1> {
        self.pending
            .complete(&self.canonical_proof, canonical_certificate)
    }
}

/// Recent nonce-bound issuer response, not current UTC or a monetary bootstrap grant.
/// Construction consumes a native pending attempt and checks its original clock before and
/// after all three signatures. Registry selection/current-owner checks remain mandatory.
/// No constructor, deserializer or Clone permits a host to manufacture or duplicate this value.
pub(super) struct FreshIssuerAdmissionV1 {
    pending: PendingIssuerEnrollmentV1,
    evidence: KagemushaVerifiedRetailEnrollmentIssuerEvidenceV1,
    canonical_proof: Vec<u8>,
    canonical_certificate: Vec<u8>,
}

impl FreshIssuerAdmissionV1 {
    pub(super) fn evidence(&self) -> &KagemushaVerifiedRetailEnrollmentIssuerEvidenceV1 {
        &self.evidence
    }
    pub(super) fn release(&self) -> &KagemushaAuthenticatedReleaseV1 {
        &self.pending.release
    }
    pub(super) fn issuer_policy(&self) -> &KagemushaRetailEnrollmentIssuerPolicyV1 {
        &self.pending.policy
    }
    pub(super) fn native_authorization_public_key(&self) -> &KagemushaDevicePublicKeyV1 {
        &self.pending.native_authorization_public_key
    }
    pub(super) fn canonical_proof(&self) -> &[u8] {
        &self.canonical_proof
    }
    pub(super) fn canonical_certificate(&self) -> &[u8] {
        &self.canonical_certificate
    }
    pub(super) fn enrollment_binding(&self) -> &KagemushaRecoveryEnrollmentBindingV1 {
        &self.pending.enrollment
    }
    pub(super) fn deadline(&self) -> Result<NativeDeadlineV1> {
        self.pending.deadline()
    }
}

#[cfg(test)]
mod tests;
