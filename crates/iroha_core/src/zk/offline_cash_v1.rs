//! Terminal admission boundary for the clean offline-cash V1 paired proof.

/// Authenticated Halo2 artifact-provider boundary.
mod artifacts;
/// Fail-closed first-party Halo2 verifier skeleton.
mod halo2_backend;
/// Strict Pasta IPA artifact, history, and augmented-proof primitives.
mod halo2_primitives;
/// Exact 184-word / 27-cell field-neutral helper public ABI.
mod helper_abi;
/// Fixed Eq/Fp and Ep/Fq helper public-binding circuit scaffolds.
mod helper_circuit;
/// Private move-only GuardUse, PlatformBind, KeyCert, and bundle relation.
mod helper_relation;
/// Private, non-authorizing bounded affine P-256 child prototype.
mod p256_affine_compact;
/// Private, non-authorizing compact P-256 arithmetic prototype.
mod p256_compact;
/// Exact Offline Cash V1 Halo2 profile and protocol identities.
mod protocol;
/// Private, non-authorizing SHA-256 inventory and fail-closed shape evidence.
mod sha256_compact;
/// Exact 229-word / 33-cell field-neutral STATE public-instance ABI.
mod state_abi;
/// Exact Eq/Fp and Ep/Fq STATE relation circuit scaffolds.
mod state_circuit;
/// Private canonical balance/credit head and conservation relation.
mod state_relation;
/// Private fixed-geometry SHA-256 bridge used only by the STATE relation.
mod state_sha;
/// Private move-only balance, pending-request, credit, and hardware-guard state machine.
pub(crate) mod state_transition;

pub(crate) use artifacts::{
    OfflineCashAuthenticatedVerifierArtifactsV1, OfflineCashHalo2ArtifactErrorV1,
    OfflineCashHalo2ArtifactManifestV1, OfflineCashHalo2ArtifactSourceV1,
};
#[cfg(test)]
pub(crate) use halo2_backend::OfflineCashHalo2VerifierBackendV1;
pub(crate) use protocol::OfflineCashHalo2ParityV1;
#[cfg(test)]
pub(crate) use protocol::{
    OfflineCashHalo2CircuitRoleV1, offline_cash_halo2_profile_digest_v1,
    offline_cash_halo2_protocol_identity_v1,
};

use iroha_data_model::{
    NetworkId,
    asset::AssetDefinitionId,
    offline::{
        OfflineCashAcknowledgementV1, OfflineCashArtifactBindingV1, OfflineCashArtifactRoleV1,
        OfflineCashAuthenticatedReleaseV1, OfflineCashPaymentRequestV1, OfflineCashPaymentV1,
    },
};
use sha2::{Digest as _, Sha256};

/// Exact verification stage that rejected a paired proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OfflineCashVerificationStageV1 {
    /// Eq/Fp current proof verification.
    EqCurrent,
    /// Ep/Fq current proof verification.
    EpCurrent,
    /// Eq/Fp delayed-history terminal decision.
    EqHistory,
    /// Ep/Fq delayed-history terminal decision.
    EpHistory,
}

/// Failure returned by the offline-cash terminal boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OfflineCashVerificationErrorV1 {
    /// Canonical request, response, or acknowledgement validation failed.
    InvalidWire,
    /// Request is not live at the authoritative handoff time.
    RequestNotLive,
    /// Payment selected a different authenticated release or artifact manifest.
    ReleaseMismatch,
    /// Payment selected a different compiled parity protocol.
    ProtocolMismatch,
    /// Authenticated release did not resolve the required parity verifying-key roles.
    ArtifactMismatch,
    /// A current proof or delayed history failed cryptographic verification.
    Cryptographic {
        /// Exact stage that failed.
        stage: OfflineCashVerificationStageV1,
        /// Backend diagnostic for operator logs; it is not a wire value.
        message: String,
    },
    /// A receiver acknowledgement does not identify the verified credit.
    AcknowledgementMismatch,
}

impl core::fmt::Display for OfflineCashVerificationErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidWire => formatter.write_str("invalid offline-cash wire value"),
            Self::RequestNotLive => formatter.write_str("offline-cash request is not live"),
            Self::ReleaseMismatch => {
                formatter.write_str("offline-cash payment selected a different release")
            }
            Self::ProtocolMismatch => {
                formatter.write_str("offline-cash payment selected a different protocol")
            }
            Self::ArtifactMismatch => {
                formatter.write_str("offline-cash release has invalid state verifying-key roles")
            }
            Self::Cryptographic { stage, message } => {
                write!(
                    formatter,
                    "offline-cash {stage:?} verification failed: {message}"
                )
            }
            Self::AcknowledgementMismatch => {
                formatter.write_str("offline-cash acknowledgement does not match verified credit")
            }
        }
    }
}

impl std::error::Error for OfflineCashVerificationErrorV1 {}

/// Cryptographic implementation used by the role-safe terminal boundary.
///
/// Current-proof verification and delayed-history decisions are deliberately
/// separate calls. A backend cannot make a payment authoritative by verifying
/// only the newest proof or only one Pasta parity.
pub(crate) trait OfflineCashPairedProofVerifierV1: paired_verifier_sealed::Sealed {
    /// Verify the Eq/Fp current proof and its constrained public history bytes.
    fn verify_eq_current(
        &self,
        verifying_key: OfflineCashArtifactBindingV1,
        protocol_digest: [u8; 32],
        semantic_digest: [u8; 32],
        proof: &[u8],
        history: &[u8],
    ) -> Result<(), String>;

    /// Verify the Ep/Fq current proof and its constrained public history bytes.
    fn verify_ep_current(
        &self,
        verifying_key: OfflineCashArtifactBindingV1,
        protocol_digest: [u8; 32],
        semantic_digest: [u8; 32],
        proof: &[u8],
        history: &[u8],
    ) -> Result<(), String>;

    /// Decide the Eq/Fp delayed-history accumulator.
    fn decide_eq_history(
        &self,
        verifying_key: OfflineCashArtifactBindingV1,
        protocol_digest: [u8; 32],
        history: &[u8],
    ) -> Result<(), String>;

    /// Decide the Ep/Fq delayed-history accumulator.
    fn decide_ep_history(
        &self,
        verifying_key: OfflineCashArtifactBindingV1,
        protocol_digest: [u8; 32],
        history: &[u8],
    ) -> Result<(), String>;
}

pub(crate) mod paired_verifier_sealed {
    /// Prevents any crate outside first-party Core from minting acceptance.
    pub(crate) trait Sealed {}
}

/// Move-only receipt proving that both current proofs and histories were decided.
#[derive(Debug)]
#[must_use]
pub(crate) struct VerifiedOfflineCashCreditV1 {
    release_id: [u8; 32],
    request_digest: [u8; 32],
    payment_digest: [u8; 32],
    network_id: NetworkId,
    asset: AssetDefinitionId,
    scale: u32,
    amount: u128,
    receiver_before: [u8; 32],
    recipient_key_reference: [u8; 32],
    credit_commitment: [u8; 32],
    transition_digest: [u8; 32],
    encrypted_credit_digest: [u8; 32],
}

impl VerifiedOfflineCashCreditV1 {
    /// Return the authenticated release identifier.
    #[must_use]
    pub(crate) const fn release_id(&self) -> [u8; 32] {
        self.release_id
    }

    /// Return the verified receiver-request digest.
    #[must_use]
    pub(crate) const fn request_digest(&self) -> [u8; 32] {
        self.request_digest
    }

    /// Return the verified sender-response digest.
    #[must_use]
    pub(crate) const fn payment_digest(&self) -> [u8; 32] {
        self.payment_digest
    }

    /// Return the exact network identity.
    #[must_use]
    pub(crate) const fn network_id(&self) -> &NetworkId {
        &self.network_id
    }

    /// Return the transferred asset definition.
    #[must_use]
    pub(crate) const fn asset(&self) -> &AssetDefinitionId {
        &self.asset
    }

    /// Return the authoritative asset scale.
    #[must_use]
    pub(crate) const fn scale(&self) -> u32 {
        self.scale
    }

    /// Return the exact positive credit amount.
    #[must_use]
    pub(crate) const fn amount(&self) -> u128 {
        self.amount
    }

    /// Return the receiver head consumed by `ReceiveFold`.
    #[must_use]
    pub(crate) const fn receiver_before(&self) -> [u8; 32] {
        self.receiver_before
    }

    /// Return the exact receiver decryption-key reference named by the request.
    #[must_use]
    pub(crate) const fn recipient_key_reference(&self) -> [u8; 32] {
        self.recipient_key_reference
    }

    /// Return the proof-bound credit commitment.
    #[must_use]
    pub(crate) const fn credit_commitment(&self) -> [u8; 32] {
        self.credit_commitment
    }

    /// Return the common sender-remainder/receiver-credit transition digest.
    #[must_use]
    pub(crate) const fn transition_digest(&self) -> [u8; 32] {
        self.transition_digest
    }

    /// Return the SHA-256 of the receiver-only encrypted opening.
    #[must_use]
    pub(crate) const fn encrypted_credit_digest(&self) -> [u8; 32] {
        self.encrypted_credit_digest
    }
}

/// Role-safe verifier bound to one authenticated offline-cash release.
pub(crate) struct OfflineCashTerminalVerifierV1<'a, V> {
    release: &'a OfflineCashAuthenticatedReleaseV1,
    backend: &'a V,
}

impl<'a, V> OfflineCashTerminalVerifierV1<'a, V>
where
    V: OfflineCashPairedProofVerifierV1,
{
    /// Bind a cryptographic backend to an already authenticated release.
    #[must_use]
    pub(crate) const fn new(
        release: &'a OfflineCashAuthenticatedReleaseV1,
        backend: &'a V,
    ) -> Self {
        Self { release, backend }
    }

    /// Verify and decide both current proofs and both delayed histories.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid wire values, stale requests, release or
    /// protocol substitution, or any failed current/history decision.
    pub(crate) fn verify_payment(
        &self,
        request: &OfflineCashPaymentRequestV1,
        payment: &OfflineCashPaymentV1,
        now_ms: u64,
    ) -> Result<VerifiedOfflineCashCreditV1, OfflineCashVerificationErrorV1> {
        payment
            .validate_against(request)
            .map_err(|_| OfflineCashVerificationErrorV1::InvalidWire)?;
        if now_ms < request.issued_at_ms || now_ms >= request.expires_at_ms {
            return Err(OfflineCashVerificationErrorV1::RequestNotLive);
        }
        if request.release_id != self.release.release_id()
            || payment.statement.release_id != self.release.release_id()
            || payment.artifact_manifest_digest != self.release.manifest_digest()
        {
            return Err(OfflineCashVerificationErrorV1::ReleaseMismatch);
        }
        if payment.proof.eq_protocol_digest != self.release.eq_protocol_digest()
            || payment.proof.ep_protocol_digest != self.release.ep_protocol_digest()
        {
            return Err(OfflineCashVerificationErrorV1::ProtocolMismatch);
        }
        let eq_verifying_key = self.release.artifact(OfflineCashArtifactRoleV1::StateVkEq);
        let ep_verifying_key = self.release.artifact(OfflineCashArtifactRoleV1::StateVkEp);
        if eq_verifying_key.role != OfflineCashArtifactRoleV1::StateVkEq
            || ep_verifying_key.role != OfflineCashArtifactRoleV1::StateVkEp
        {
            return Err(OfflineCashVerificationErrorV1::ArtifactMismatch);
        }

        let semantic_digest = payment
            .statement
            .canonical_digest()
            .map_err(|_| OfflineCashVerificationErrorV1::InvalidWire)?;
        self.backend
            .verify_eq_current(
                eq_verifying_key,
                self.release.eq_protocol_digest(),
                semantic_digest,
                &payment.proof.eq_proof,
                &payment.proof.eq_history,
            )
            .map_err(|message| OfflineCashVerificationErrorV1::Cryptographic {
                stage: OfflineCashVerificationStageV1::EqCurrent,
                message,
            })?;
        self.backend
            .verify_ep_current(
                ep_verifying_key,
                self.release.ep_protocol_digest(),
                semantic_digest,
                &payment.proof.ep_proof,
                &payment.proof.ep_history,
            )
            .map_err(|message| OfflineCashVerificationErrorV1::Cryptographic {
                stage: OfflineCashVerificationStageV1::EpCurrent,
                message,
            })?;
        self.backend
            .decide_eq_history(
                eq_verifying_key,
                self.release.eq_protocol_digest(),
                &payment.proof.eq_history,
            )
            .map_err(|message| OfflineCashVerificationErrorV1::Cryptographic {
                stage: OfflineCashVerificationStageV1::EqHistory,
                message,
            })?;
        self.backend
            .decide_ep_history(
                ep_verifying_key,
                self.release.ep_protocol_digest(),
                &payment.proof.ep_history,
            )
            .map_err(|message| OfflineCashVerificationErrorV1::Cryptographic {
                stage: OfflineCashVerificationStageV1::EpHistory,
                message,
            })?;

        let encrypted_credit_digest = Sha256::digest(&payment.encrypted_credit).into();
        Ok(VerifiedOfflineCashCreditV1 {
            release_id: self.release.release_id(),
            request_digest: request
                .canonical_digest()
                .map_err(|_| OfflineCashVerificationErrorV1::InvalidWire)?,
            payment_digest: payment
                .canonical_digest_against(request)
                .map_err(|_| OfflineCashVerificationErrorV1::InvalidWire)?,
            network_id: payment.statement.network_id.clone(),
            asset: payment.statement.asset.clone(),
            scale: payment.statement.scale,
            amount: payment.statement.amount,
            receiver_before: payment.statement.receiver_before,
            recipient_key_reference: request.recipient_key_reference,
            credit_commitment: payment.statement.credit_commitment,
            transition_digest: payment.statement.transition_digest,
            encrypted_credit_digest,
        })
    }

    /// Validate a receiver acknowledgement against one verified credit.
    ///
    /// # Errors
    ///
    /// Returns an error when the acknowledgement signature or identity binding fails.
    pub(crate) fn verify_acknowledgement(
        &self,
        request: &OfflineCashPaymentRequestV1,
        payment: &OfflineCashPaymentV1,
        acknowledgement: &OfflineCashAcknowledgementV1,
        receipt: &VerifiedOfflineCashCreditV1,
    ) -> Result<(), OfflineCashVerificationErrorV1> {
        acknowledgement
            .validate_against(request, payment)
            .map_err(|_| OfflineCashVerificationErrorV1::InvalidWire)?;
        if receipt.release_id != self.release.release_id()
            || acknowledgement.release_id != receipt.release_id
            || acknowledgement.request_digest != receipt.request_digest
            || acknowledgement.payment_digest != receipt.payment_digest
        {
            return Err(OfflineCashVerificationErrorV1::AcknowledgementMismatch);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "offline_cash_v1/terminal_tests.rs"]
mod terminal_tests;

#[cfg(test)]
#[path = "offline_cash_v1/halo2_tests.rs"]
mod halo2_tests;

#[cfg(test)]
#[path = "offline_cash_v1/halo2_primitives_tests.rs"]
mod halo2_primitives_tests;

#[cfg(test)]
#[path = "offline_cash_v1/state_circuit_tests.rs"]
mod state_circuit_tests;

#[cfg(test)]
#[path = "offline_cash_v1/helper_tests.rs"]
mod helper_tests;
