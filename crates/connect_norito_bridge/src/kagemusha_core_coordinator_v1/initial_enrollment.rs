//! One-use native issuer enrollment before an account/device wallet-open ceremony.
//!
//! The native owner starts one suspend-inclusive deadline and creates its client nonce before
//! HTTP. Completing this object proves a recent response by the independently pinned issuer
//! and both possession signatures. It supplies neither a hardware commit clock nor monetary
//! authority. TODO: connect the owned pending phase to the bounded revocable native registry
//! and sole public lifecycle ABI; there is no C/JNI policy installer or host approval fallback.

use std::{sync::Arc, time::Duration};

use iroha_core::zk::kagemusha_v1_state::KagemushaRecoveryEnrollmentBindingV1;
use iroha_crypto::Algorithm;
use iroha_data_model::kagemusha::{
    KagemushaAuthenticatedReleaseV1, KagemushaDevicePublicKeyV1,
    KagemushaRetailEnrollmentCertificateV1, KagemushaRetailEnrollmentIssuerPolicyV1,
    KagemushaRetailEnrollmentOwnerV1, KagemushaRetailEnrollmentPossessionProofV1,
    KagemushaRetailEnrollmentSelectionV1, KagemushaVerifiedRetailEnrollmentIssuerEvidenceV1,
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
            client_nonce,
            deadline,
        })
    }

    /// Read only the original native nonce; reading it never starts a new lifetime.
    pub(super) fn client_nonce(&self) -> Result<[u8; 32]> {
        self.require_unexpired()?;
        Ok(self.client_nonce)
    }

    pub(super) fn enrollment_binding(&self) -> &KagemushaRecoveryEnrollmentBindingV1 {
        &self.enrollment
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

    /// Consume one pending attempt before cryptographic work. Native C/JNI must first copy
    /// bounded input buffers and consume its exact registry ticket; those boundaries may not
    /// retry a rejected proof against this same instance or replace its retained selector.
    pub(super) fn complete(
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
        let issuance = &proof.challenge.issuance;
        if proof.challenge.client_nonce != self.client_nonce
            || proof.challenge.owner != self.enrollment.owner
            || issuance.release_id != self.release.release_id()
            || issuance.hardware_policy_digest != self.release.hardware_policy_digest()
            || issuance.core_authorization_key_reference
                != hardware_authorization_key_reference_v1(&self.native_authorization_public_key)
        {
            return Err(InitialEnrollmentErrorV1::Binding);
        }
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
