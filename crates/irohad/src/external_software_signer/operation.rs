//! Opaque hardware operations with authoritative reservation and signature-release fences.
//!
//! The operation provider has no key import, export, wrapping or generation API. An independently
//! configured state source authenticates custody state and owns durable exclusive reservations and
//! operation commits. It must not obtain its trust, clock or current head from the signing request.
//! Provider signatures remain internal until every sub-operation, durable commit and final state
//! observation succeeds. Failure leaves a recoverable reservation tombstone; it never frees an
//! operation id for an untracked retry. The source must recover committed operations explicitly.
//! This boundary verifies custody, ordering and completion, not application payload semantics.
//! Its crate-private producer must validate the canonical role/purpose request and construct
//! provenance binding the exact active custody record and current anchor before supplying bytes.
//! A nonzero request digest or a provider signature cannot replace those producer checks.
//!
//! Release manifests use the concrete purpose-bound producer and immutable private receipt journal
//! below; enrollment, renewal and terminal revocation use the authoritative control transitions.
//! TODO: Replace the remaining role service/journal private-key sites and migrate their runtime and
//! receipt consumers atomically. Real hardware and finalized state adapters remain required. This
//! module's injected test providers are race/failure simulations, never hardware qualification.

use super::protocol::{
    SIGNER_MAX_REQUEST_PAYLOAD_BYTES_V1, SIGNER_MAX_SIGNATURE_BYTES_V1, digest_canonical,
    digest_parts,
};
use iroha_crypto::Signature;
use sorafs_manifest::signer::{
    custody::{
        SignerCustodyBindingV1, SignerCustodyErrorV1, SignerCustodyTrustV1,
        SignerCustodyUseContextV1, VerifiedSignerCustodyV1, verify_signer_custody_use_v1,
    },
    protocol::{
        SignerKeyOperationPurposeV1, SignerOperationActionV1, SignerOperationAuditHeadV1,
        SignerOperationCommitmentV1, SignerOperationCustodyV1, SignerOperationIntentV1,
        SignerOperationReservationV1,
    },
};
use std::{fmt, sync::Arc};
use zeroize::Zeroizing;

/// Authoritative enrollment and terminal custody-control transitions.
pub mod control;
mod recovery;
/// Exact reviewed release-manifest producer with mandatory durable private receipt staging.
#[cfg(unix)]
pub mod release_manifest;
pub(super) use recovery::{RecoveredSignerOperationV1, RecoveredSignerSignatureV1};

const SIGNATURES_DOMAIN: &[u8] = b"iroha.sorafs.signer.operation.signatures.v1";
const MAX_RESERVATION_MS: u64 = 60_000;

/// Privately constructed request for one authoritative exclusive reservation.
pub struct SignerOperationReservationRequestV1<'a> {
    intent: &'a SignerOperationIntentV1,
    intent_digest: [u8; 32],
    custody: &'a VerifiedSignerCustodyV1,
}
impl SignerOperationReservationRequestV1<'_> {
    /// Exact action and journal predecessor to compare and reserve atomically.
    #[must_use]
    pub const fn intent(&self) -> &SignerOperationIntentV1 {
        self.intent
    }
    /// Canonical digest of the exact intent.
    #[must_use]
    pub const fn intent_digest(&self) -> [u8; 32] {
        self.intent_digest
    }
    /// Independently verified active generation and current custody control-state expectation.
    #[must_use]
    pub const fn custody(&self) -> &VerifiedSignerCustodyV1 {
        self.custody
    }
}

/// Privately constructed expectation for checking exclusive reservation ownership.
pub struct SignerOperationReservationCheckV1<'a> {
    request: SignerOperationReservationRequestV1<'a>,
    reservation: SignerOperationReservationV1,
}
impl SignerOperationReservationCheckV1<'_> {
    /// Exact intent and last verified custody state.
    #[must_use]
    pub const fn request(&self) -> &SignerOperationReservationRequestV1<'_> {
        &self.request
    }
    /// Exact unexpired reservation and fencing generation that must still be owned.
    #[must_use]
    pub const fn reservation(&self) -> SignerOperationReservationV1 {
        self.reservation
    }
}

/// Privately constructed request for durable completion of the exact reserved aggregate action.
pub struct SignerOperationCommitRequestV1<'a> {
    check: SignerOperationReservationCheckV1<'a>,
    commitment: SignerOperationCommitmentV1,
    signatures_digest: [u8; 32],
    original_custody: SignerOperationCustodyV1,
}
impl SignerOperationCommitRequestV1<'_> {
    /// Exact reservation ownership and current custody state to compare atomically.
    #[must_use]
    pub const fn check(&self) -> &SignerOperationReservationCheckV1<'_> {
        &self.check
    }
    /// Exact durably persisted audit successor and response to commit.
    #[must_use]
    pub const fn commitment(&self) -> SignerOperationCommitmentV1 {
        self.commitment
    }
    /// Digest of every ordered purpose, exact signing message and provider signature.
    #[must_use]
    pub const fn signatures_digest(&self) -> [u8; 32] {
        self.signatures_digest
    }
    /// Exact original custody identity to persist and compare on every completed-row observation.
    #[must_use]
    pub const fn original_custody(&self) -> SignerOperationCustodyV1 {
        self.original_custody
    }
}

/// Independently configured authoritative custody and durable operation-state boundary.
///
/// Implementations authenticate finalized per-role custody control state, trust and current time
/// independently of candidate requests. Reservations/journal commits are outside the custody
/// control-state digest. Every CAS must compare the exact role, active record/generation/control
/// state, action, operation id, request digest, journal predecessor and reservation fence. Expired,
/// failed or abandoned reservations retain replay tombstones; they cannot silently become fresh
/// operations. No in-memory or software fallback is an acceptable production implementation.
pub trait SignerOperationStateSourceV1: Send + Sync {
    /// Read a fresh independently authenticated active-head, policy and revocation snapshot.
    ///
    /// # Errors
    /// Fails when a fresh authoritative snapshot cannot be authenticated.
    fn observe(
        &self,
        binding: &SignerCustodyBindingV1,
    ) -> Result<SignerCustodyUseContextV1, SignerOperationErrorV1>;

    /// Durably reserve the exact next journal slot under exclusive ownership using CAS.
    ///
    /// # Errors
    /// Fails on custody drift, journal drift, replay, conflict or inability to persist the fence.
    fn reserve(
        &self,
        request: &SignerOperationReservationRequestV1<'_>,
    ) -> Result<SignerOperationReservationV1, SignerOperationErrorV1>;

    /// Authenticate exact still-exclusive, unexpired ownership and return fresh custody state.
    ///
    /// # Errors
    /// Fails on a lost fence, changed predecessor, expiry, unavailable state or custody drift.
    fn observe_reserved(
        &self,
        check: &SignerOperationReservationCheckV1<'_>,
    ) -> Result<SignerCustodyUseContextV1, SignerOperationErrorV1>;

    /// Atomically compare all expectations, verify durable audit/response persistence and commit.
    ///
    /// The returned context is authenticated after the durable completion transaction. Success
    /// must not precede persistence or be synthesized from the request. This ordinary-use CAS
    /// cannot rotate or revoke custody; those are separate governed terminal transitions.
    ///
    /// # Errors
    /// Fails closed if the exact reservation and immutable successor cannot be committed.
    fn commit(
        &self,
        request: &SignerOperationCommitRequestV1<'_>,
    ) -> Result<SignerCustodyUseContextV1, SignerOperationErrorV1>;

    /// Independently authenticate the exact durable completed operation and fresh custody state.
    ///
    /// The stored completion must prove that the original exclusive reservation was committed
    /// before its expiry. Completed-response recovery may happen after that expiry; it never
    /// renews a reservation or authorizes another provider call. An abandoned reservation or a
    /// late/failed commit is not a completed operation and must remain ineligible for recovery.
    /// The original record/control-state digests must be stored and match `original_custody`;
    /// comparing only request/response/signature hashes cannot fence same-key custody renewal.
    ///
    /// # Errors
    /// Fails when completion changed, is not durable, is unavailable, or custody is ineligible.
    fn observe_committed(
        &self,
        request: &SignerOperationCommitRequestV1<'_>,
    ) -> Result<SignerCustodyUseContextV1, SignerOperationErrorV1>;

    /// Authenticate the next enrollment slot and governed expected successor configuration.
    ///
    /// # Errors
    /// Fails unless the independently configured successor exactly matches `binding`.
    fn observe_enrollment(
        &self,
        binding: &SignerCustodyBindingV1,
    ) -> Result<
        sorafs_manifest::signer::custody::SignerCustodyEnrollmentContextV1,
        SignerOperationErrorV1,
    >;

    /// Authoritatively enroll the initial independently qualified generation using durable CAS.
    ///
    /// # Errors
    /// Fails unless this is the exact initial predecessor slot and no generation is active.
    fn enroll_initial(
        &self,
        request: &control::SignerCustodyEnrollmentRequestV1<'_>,
    ) -> Result<SignerCustodyUseContextV1, SignerOperationErrorV1>;

    /// Atomically finalize the exact terminal audit before activating or revoking custody.
    ///
    /// The transaction compares the exact exclusive reservation, current control state, old active
    /// head, expected terminal intent and audit successor. Activation additionally compares the
    /// independently governed successor binding and exact enrollment sequence/predecessor. No
    /// response or further old-key signing is authorized after this transaction takes effect.
    ///
    /// # Errors
    /// Fails when audit durability, authoritative enrollment/control CAS or finality cannot be proved.
    fn commit_custody_transition(
        &self,
        request: &control::SignerCustodyTransitionRequestV1<'_>,
    ) -> Result<SignerCustodyUseContextV1, SignerOperationErrorV1>;
}

/// Privately constructed bounded exact-message request presented to an opaque key provider.
pub struct SignerKeyOperationRequestV1<'a> {
    check: SignerOperationReservationCheckV1<'a>,
    purpose: SignerKeyOperationPurposeV1,
    ordinal: u8,
    message: &'a [u8],
}
impl SignerKeyOperationRequestV1<'_> {
    /// Verified custody and exact authoritative operation reservation.
    #[must_use]
    pub const fn check(&self) -> &SignerOperationReservationCheckV1<'_> {
        &self.check
    }
    /// Exact sub-operation purpose, distinct from the custody role's application purpose.
    #[must_use]
    pub const fn purpose(&self) -> SignerKeyOperationPurposeV1 {
        self.purpose
    }
    /// One-based ordered sub-operation within this reservation; never retried by this boundary.
    #[must_use]
    pub const fn ordinal(&self) -> u8 {
        self.ordinal
    }
    /// Exact bounded signing bytes; never log or persist application payloads in provider errors.
    #[must_use]
    pub const fn message(&self) -> &[u8] {
        self.message
    }
}
impl fmt::Debug for SignerKeyOperationRequestV1<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SignerKeyOperationRequestV1")
            .field("purpose", &self.purpose)
            .field("ordinal", &self.ordinal)
            .finish_non_exhaustive()
    }
}

/// Deployment-injected non-exportable hardware signing operations, with no private-key API.
///
/// The adapter resolves the exact opaque handle from verified custody, verifies the hardware's
/// identity and honors the operation fence. It must never import, export or substitute a software
/// key. Vendor credentials stay inside the adapter; no candidate record selects an implementation.
pub trait SignerKeyOperationProviderV1: Send + Sync {
    /// Sign the exact authorized message with the exact independently qualified opaque key.
    ///
    /// # Errors
    /// Returns a fixed failure class when hardware, credentials or the operation fence fail.
    fn sign(
        &self,
        request: &SignerKeyOperationRequestV1<'_>,
    ) -> Result<Signature, SignerOperationErrorV1>;
}

/// Fixed secret-free operation failures; backend messages are never retained.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignerOperationErrorV1 {
    /// Invalid action, digest, message, bounds, signature order or successor.
    InvalidOperation,
    /// The independent custody verifier rejected the active record.
    Custody(SignerCustodyErrorV1),
    /// Active custody changed or an authoritative observation moved backwards.
    CustodyChanged,
    /// The state source could not authenticate fresh authoritative state.
    StateUnavailable,
    /// A reservation, journal predecessor, expiry or durable completion CAS failed.
    ReservationConflict,
    /// Hardware rejected or could not perform the operation.
    ProviderUnavailable,
    /// Hardware returned an invalid or incorrect-key/message signature.
    InvalidSignature,
    /// A previous failed sub-operation made this operation permanently unusable.
    Poisoned,
}
impl fmt::Display for SignerOperationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidOperation => "invalid signer operation",
            Self::Custody(_) => "signer operation custody rejected",
            Self::CustodyChanged => "signer operation custody changed",
            Self::StateUnavailable => "signer authoritative state unavailable",
            Self::ReservationConflict => "signer operation reservation conflict",
            Self::ProviderUnavailable => "signer hardware operation unavailable",
            Self::InvalidSignature => "signer hardware signature invalid",
            Self::Poisoned => "signer operation poisoned",
        })
    }
}
impl std::error::Error for SignerOperationErrorV1 {}

/// Service-owned coordinator with separately injected hardware and authoritative-state owners.
pub struct SignerOperationCoordinatorV1 {
    binding: SignerCustodyBindingV1,
    record: Vec<u8>,
    trust: SignerCustodyTrustV1,
    provider: Arc<dyn SignerKeyOperationProviderV1>,
    source: Arc<dyn SignerOperationStateSourceV1>,
}
impl SignerOperationCoordinatorV1 {
    /// Verify independently supplied configuration and current state before constructing a signer.
    ///
    /// This constructor accepts only public custody metadata and opaque operations, never key
    /// material. Ordinary actions obtain new authoritative observations and reservations.
    ///
    /// # Errors
    /// Fails for invalid custody, unavailable state or untrusted expected configuration.
    pub fn new(
        binding: SignerCustodyBindingV1,
        record: Vec<u8>,
        trust: SignerCustodyTrustV1,
        provider: Arc<dyn SignerKeyOperationProviderV1>,
        source: Arc<dyn SignerOperationStateSourceV1>,
    ) -> Result<Self, SignerOperationErrorV1> {
        let coordinator = Self {
            binding,
            record,
            trust,
            provider,
            source,
        };
        coordinator.verify(&coordinator.source.observe(&coordinator.binding)?)?;
        Ok(coordinator)
    }

    fn verify(
        &self,
        context: &SignerCustodyUseContextV1,
    ) -> Result<VerifiedSignerCustodyV1, SignerOperationErrorV1> {
        verify_signer_custody_use_v1(&self.record, &self.binding, &self.trust, context)
            .map_err(SignerOperationErrorV1::Custody)
    }

    /// Begin only after the trusted service has validated the canonical role/purpose request.
    pub(super) fn begin(
        &self,
        intent: SignerOperationIntentV1,
    ) -> Result<SignerOperationV1<'_>, SignerOperationErrorV1> {
        let intent_digest = intent
            .digest()
            .map_err(|_| SignerOperationErrorV1::InvalidOperation)?;
        let custody = self.verify(&self.source.observe(&self.binding)?)?;
        let reservation = self.source.reserve(&SignerOperationReservationRequestV1 {
            intent: &intent,
            intent_digest,
            custody: &custody,
        })?;
        let mut operation = SignerOperationV1 {
            coordinator: self,
            intent,
            intent_digest,
            reservation,
            custody,
            signatures: Vec::new(),
            poisoned: false,
        };
        operation.validate_reservation()?;
        operation.refresh_reserved()?;
        Ok(operation)
    }
}
impl fmt::Debug for SignerOperationCoordinatorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SignerOperationCoordinatorV1")
            .finish_non_exhaustive()
    }
}

/// Internal in-progress action. Staged signatures never cross the public release boundary.
pub(super) struct SignerOperationV1<'a> {
    coordinator: &'a SignerOperationCoordinatorV1,
    intent: SignerOperationIntentV1,
    intent_digest: [u8; 32],
    reservation: SignerOperationReservationV1,
    custody: VerifiedSignerCustodyV1,
    signatures: Vec<StagedSignature>,
    poisoned: bool,
}
struct StagedSignature {
    purpose: SignerKeyOperationPurposeV1,
    message_digest: [u8; 32],
    signature: Zeroizing<Vec<u8>>,
}
impl SignerOperationV1<'_> {
    fn check(&self) -> SignerOperationReservationCheckV1<'_> {
        SignerOperationReservationCheckV1 {
            request: SignerOperationReservationRequestV1 {
                intent: &self.intent,
                intent_digest: self.intent_digest,
                custody: &self.custody,
            },
            reservation: self.reservation,
        }
    }
    fn validate_reservation(&self) -> Result<(), SignerOperationErrorV1> {
        let now = self.custody.verified_at_unix_ms();
        if self.reservation.reservation_id == [0; 32]
            || self.reservation.fence == 0
            || self.reservation.expires_at_unix_ms <= now
            || self.reservation.expires_at_unix_ms > self.custody.statement().expires_at_unix_ms
            || self.reservation.expires_at_unix_ms.saturating_sub(now) > MAX_RESERVATION_MS
        {
            return Err(SignerOperationErrorV1::ReservationConflict);
        }
        Ok(())
    }
    fn accept_context(
        &mut self,
        context: &SignerCustodyUseContextV1,
    ) -> Result<(), SignerOperationErrorV1> {
        let current = self.coordinator.verify(context)?;
        if !current.continues_active_state(&self.custody) {
            return Err(SignerOperationErrorV1::CustodyChanged);
        }
        self.custody = current;
        self.validate_reservation()
    }
    fn refresh_reserved(&mut self) -> Result<(), SignerOperationErrorV1> {
        let context = self.coordinator.source.observe_reserved(&self.check())?;
        self.accept_context(&context)
    }
    fn required_purposes(&self) -> &'static [SignerKeyOperationPurposeV1] {
        use SignerKeyOperationPurposeV1::{AuditRecord, Provenance, Response, RolePayload};
        match self.intent.action {
            SignerOperationActionV1::Sign => &[RolePayload, AuditRecord, Provenance, Response],
            SignerOperationActionV1::Qualify | SignerOperationActionV1::Status => {
                &[AuditRecord, Provenance, Response]
            }
            SignerOperationActionV1::ActivateCustody | SignerOperationActionV1::RevokeCustody => {
                &[AuditRecord]
            }
        }
    }
    /// Prepare a sub-signature solely for internal canonical journal/response construction.
    ///
    /// The trusted producer owns exact role-payload authorization and canonical custody-bound
    /// provenance construction. This method checks purpose order, bounded bytes, cryptography and
    /// current custody; it does not derive application intent from the supplied message.
    pub(super) fn sign(
        &mut self,
        purpose: SignerKeyOperationPurposeV1,
        message: &[u8],
    ) -> Result<&[u8], SignerOperationErrorV1> {
        if self.poisoned {
            return Err(SignerOperationErrorV1::Poisoned);
        }
        // Poison first: every early error, including invalid ordering, permanently fences retries.
        self.poisoned = true;
        if self.required_purposes().get(self.signatures.len()) != Some(&purpose)
            || message.is_empty()
            || message.len() > SIGNER_MAX_REQUEST_PAYLOAD_BYTES_V1
            || (purpose != SignerKeyOperationPurposeV1::RolePayload && message.len() != 32)
        {
            return Err(SignerOperationErrorV1::InvalidOperation);
        }
        self.refresh_reserved()?;
        let ordinal = u8::try_from(self.signatures.len() + 1)
            .map_err(|_| SignerOperationErrorV1::InvalidOperation)?;
        let signature = Zeroizing::new(self.coordinator.provider.sign(
            &SignerKeyOperationRequestV1 {
                check: self.check(),
                purpose,
                ordinal,
                message,
            },
        )?);
        if signature.payload().len() > SIGNER_MAX_SIGNATURE_BYTES_V1
            || signature
                .verify(&self.coordinator.binding.public_key, message)
                .is_err()
        {
            return Err(SignerOperationErrorV1::InvalidSignature);
        }
        self.refresh_reserved()?;
        self.signatures.push(StagedSignature {
            purpose,
            message_digest: digest_parts(b"iroha.sorafs.signer.operation.message.v1", &[message]),
            signature: Zeroizing::new(signature.payload().to_vec()),
        });
        self.poisoned = false;
        self.signatures
            .last()
            .map(|signature| signature.signature.as_slice())
            .ok_or(SignerOperationErrorV1::InvalidSignature)
    }
    /// Commit the aggregate action and release signatures only after a fresh committed-state fence.
    pub(super) fn finish(
        mut self,
        commitment: SignerOperationCommitmentV1,
    ) -> Result<CompletedSignerOperationV1, SignerOperationErrorV1> {
        if self.poisoned {
            return Err(SignerOperationErrorV1::Poisoned);
        }
        if matches!(
            self.intent.action,
            SignerOperationActionV1::ActivateCustody | SignerOperationActionV1::RevokeCustody
        ) {
            return Err(SignerOperationErrorV1::InvalidOperation);
        }
        if self.signatures.len() != self.required_purposes().len()
            || commitment.audit.sequence != self.intent.previous_audit.sequence + 1
            || commitment.audit.digest == [0; 32]
            || commitment.audit.digest == self.intent.previous_audit.digest
            || commitment.response_digest == [0; 32]
        {
            return Err(SignerOperationErrorV1::InvalidOperation);
        }
        for (purpose, message) in [
            (
                SignerKeyOperationPurposeV1::AuditRecord,
                commitment.audit.signing_message(),
            ),
            (
                SignerKeyOperationPurposeV1::Response,
                commitment.response_signing_message(),
            ),
        ] {
            let digest = digest_parts(b"iroha.sorafs.signer.operation.message.v1", &[&message]);
            if !self
                .signatures
                .iter()
                .any(|signature| signature.purpose == purpose && signature.message_digest == digest)
            {
                return Err(SignerOperationErrorV1::InvalidOperation);
            }
        }
        self.refresh_reserved()?;
        // Commit exact signature bytes through a domain-separated digest, without making a
        // second unzeroized copy of any unreleased signature for canonical serialization.
        let encoded_signatures = Zeroizing::new(
            norito::encode_canonical(
                &self
                    .signatures
                    .iter()
                    .map(|signature| {
                        (
                            signature.purpose,
                            signature.message_digest,
                            digest_parts(
                                b"iroha.sorafs.signer.operation.signature.v1",
                                &[signature.signature.as_slice()],
                            ),
                        )
                    })
                    .collect::<Vec<_>>(),
            )
            .map_err(|_| SignerOperationErrorV1::InvalidOperation)?,
        );
        let signatures_digest = digest_parts(SIGNATURES_DOMAIN, &[encoded_signatures.as_slice()]);
        let commit = SignerOperationCommitRequestV1 {
            check: self.check(),
            commitment,
            signatures_digest,
            original_custody: SignerOperationCustodyV1::from_verified(&self.custody),
        };
        let committed = self.coordinator.source.commit(&commit)?;
        self.accept_context(&committed)?;
        let commit = SignerOperationCommitRequestV1 {
            check: self.check(),
            commitment,
            signatures_digest,
            original_custody: SignerOperationCustodyV1::from_verified(&self.custody),
        };
        let final_context = self.coordinator.source.observe_committed(&commit)?;
        self.accept_context(&final_context)?;
        Ok(CompletedSignerOperationV1 {
            original_custody: SignerOperationCustodyV1::from_verified(&self.custody),
            custody: self.custody,
            reservation: self.reservation,
            commitment,
            intent_digest: self.intent_digest,
            signatures_digest,
            signatures: self.signatures,
        })
    }
}

/// Privately constructed result of exact durable completion and final custody revalidation.
pub struct CompletedSignerOperationV1 {
    original_custody: SignerOperationCustodyV1,
    custody: VerifiedSignerCustodyV1,
    reservation: SignerOperationReservationV1,
    commitment: SignerOperationCommitmentV1,
    intent_digest: [u8; 32],
    signatures_digest: [u8; 32],
    signatures: Vec<StagedSignature>,
}
impl CompletedSignerOperationV1 {
    /// Exact original record/control-state identity authenticated by durable completion.
    #[must_use]
    pub const fn original_custody(&self) -> SignerOperationCustodyV1 {
        self.original_custody
    }
    /// Fresh verified active record and current finalized anchor at release.
    #[must_use]
    pub const fn custody(&self) -> &VerifiedSignerCustodyV1 {
        &self.custody
    }
    /// Exact completed reservation coordinates.
    #[must_use]
    pub const fn reservation(&self) -> SignerOperationReservationV1 {
        self.reservation
    }
    /// Exact immutable audit successor and response committed by authoritative CAS.
    #[must_use]
    pub const fn commitment(&self) -> SignerOperationCommitmentV1 {
        self.commitment
    }
    /// Canonical digest of the exact completed intent.
    #[must_use]
    pub const fn intent_digest(&self) -> [u8; 32] {
        self.intent_digest
    }
    /// Commitment to all ordered exact-message signatures.
    #[must_use]
    pub const fn signatures_digest(&self) -> [u8; 32] {
        self.signatures_digest
    }
    /// Release one signature only from a successfully completed and fenced aggregate action.
    #[must_use]
    pub fn signature(&self, purpose: SignerKeyOperationPurposeV1) -> Option<&[u8]> {
        self.signatures
            .iter()
            .find(|signature| signature.purpose == purpose)
            .map(|signature| signature.signature.as_slice())
    }
}
impl fmt::Debug for CompletedSignerOperationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompletedSignerOperationV1")
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests;
